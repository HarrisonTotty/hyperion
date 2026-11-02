/**
 * The exposure histogram: its kernel, its CPU twin, and the reader that keeps at most three
 * histograms in flight to the CPU (plan R07, T12, Design note 10).
 *
 * @remarks
 * The kernel takes the HDR colour as a texture argument; R07.T7 makes each photorealistic view's
 * scene target and wires it here (decision 2026-10-02, item 1). R01's `readBuffer` copies into a
 * staging buffer of its own on every call, so no mapped buffer is ever reused; the reader's ring
 * of three storage buffers bounds the reads in flight and keeps a histogram being written from
 * one being copied.
 */

import { BUFFER_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type {
  BufferHandle,
  ComputeHandle,
  RenderEngine,
  TextureHandle,
  ViewSize,
} from "../engine/types";
import { packGlareTerms } from "./bloomChain";
import { type GlareSource, glareSourcePerUnit, type GlareSpreadTerms } from "./glare";
import GLARE_WGSL from "./glare.wgsl?raw";
import HISTOGRAM_WGSL from "./histogram.wgsl?raw";
import { type HistogramMeter, meterLuma, meterWeights } from "./meter";

/** The histogram's bin count. */
export const HISTOGRAM_BINS = 256;

/** log₂ of the smallest pre-exposed luminance with a bin of its own: 2⁻¹⁴, `rgba16float`'s least normal. */
export const HISTOGRAM_MIN_LOG2 = -14;

/** log₂ of the top of the binned range: 2¹⁶, above `rgba16float`'s largest value. */
export const HISTOGRAM_MAX_LOG2 = 16;

/** Bins per stop above bin 0: 255 ÷ 30 = 8.5, each bin 0.118 stop wide. */
export const HISTOGRAM_BINS_PER_STOP =
  (HISTOGRAM_BINS - 1) / (HISTOGRAM_MAX_LOG2 - HISTOGRAM_MIN_LOG2);

/** The kernel's workgroup side, texels: 16 × 16 invocations. */
export const HISTOGRAM_WORKGROUP = 16;

export { METER_LUMA } from "./meter";

/** One frame's histogram, as read back. */
export interface Histogram {
  /** {@link HISTOGRAM_BINS} weighted counts; bin 0 holds everything below 2⁻¹⁴, zeros included. */
  readonly bins: Uint32Array;
  /**
   * The pre-exposure scale of the target it was taken from, 1 ÷ (cd/m²): a bin's luminance is
   * 2^(bin centre) ÷ this.
   */
  readonly preExposure: number;
}

/**
 * The exposure-histogram kernel: no subgroup twin, and `bit-exact` since its integer adds give the
 * same bytes in any order.
 */
export const HISTOGRAM_KERNEL: KernelPair = {
  name: "exposure histogram",
  reference: `${GLARE_WGSL}\n${HISTOGRAM_WGSL}`,
  subgroup: null,
  readback: "bit-exact",
};

/**
 * The bin of a pre-exposed luminance, as the kernel computes it: 0 below 2⁻¹⁴ (and for NaN), 255
 * from 2¹⁶ (and for +∞, which a pass writing above 65,504 stores), else 1 + ⌊(log₂ L + 14) × 8.5⌋.
 */
export function histogramBin(luminance: number): number {
  if (!(luminance >= 2 ** HISTOGRAM_MIN_LOG2)) {
    return 0;
  }
  if (!(luminance < 2 ** HISTOGRAM_MAX_LOG2)) {
    return HISTOGRAM_BINS - 1;
  }
  const position = (Math.log2(luminance) - HISTOGRAM_MIN_LOG2) * HISTOGRAM_BINS_PER_STOP;
  return Math.min(1 + Math.floor(position), HISTOGRAM_BINS - 1);
}

/** The pre-exposed luminance at a bin's centre in log₂; bin 0 is luminance 0 (Design note 10). */
export function binCentreLuminance(bin: number): number {
  if (bin <= 0) {
    return 0;
  }
  return 2 ** (HISTOGRAM_MIN_LOG2 + (bin - 0.5) / HISTOGRAM_BINS_PER_STOP);
}

/**
 * The meter class a texel's alpha holds, as the kernel reads it: rounded half to even, as WGSL's
 * `round` does, in [0, 3].
 */
export function meterClassOf(alpha: number): number {
  const a = Math.max(alpha, 0);
  const floor = Math.floor(a);
  const rest = a - floor;
  const rounded = rest > 0.5 || (rest === 0.5 && floor % 2 === 1) ? floor + 1 : floor;
  return Math.min(rounded, 3);
}

/**
 * The frame's glare sources as the histogram weighs them (`histogram.wgsl`): the view they are
 * seen from, its spread function, and the sources, each spreading its whole light.
 */
export interface MeteredGlare {
  readonly sources: ReadonlyArray<GlareSource>;
  /** The target's pre-exposure scale, 1 ÷ (cd/m²). */
  readonly preExposure: number;
  /** The frame's view rotation, sixteen `f32` column-major (`viewRotation4`). */
  readonly viewRotation: Float32Array;
  /** The frame's projection, sixteen `f32` column-major (`perspectiveReversedInfinite`). */
  readonly projection: Float32Array;
  /** The view's spread function (`glareSpreadTerms`). */
  readonly terms: GlareSpreadTerms;
}

/** A column-major 4 × 4 matrix times a direction (w = 0). */
function rotated(
  m: Float32Array,
  v: { x: number; y: number; z: number },
): [number, number, number] {
  const at = (i: number): number => m[i] ?? 0;
  return [
    at(0) * v.x + at(4) * v.y + at(8) * v.z,
    at(1) * v.x + at(5) * v.y + at(9) * v.z,
    at(2) * v.x + at(6) * v.y + at(10) * v.z,
  ];
}

/** A vector's unit, or itself where it has no length. */
function unit(v: readonly [number, number, number]): [number, number, number] {
  const length = Math.hypot(v[0], v[1], v[2]);
  return length > 0 ? [v[0] / length, v[1] / length, v[2] / length] : [v[0], v[1], v[2]];
}

/**
 * The veil of every source of `glare` at texel (x, y) of a target of `size`, through Rec. 709's
 * weights, pre-exposed: the kernel's `meteredVeil`, its pixel's direction as the tone-mapping
 * pass's `glare_pixel_direction` takes it.
 */
export function meteredVeil(glare: MeteredGlare, size: ViewSize, xPx: number, yPx: number): number {
  const p = glare.projection;
  const ndcX = ((xPx + 0.5) / size.widthPx) * 2 - 1;
  const ndcY = 1 - ((yPx + 0.5) / size.heightPx) * 2;
  const pixel = unit([ndcX / (p[0] ?? 1), ndcY / (p[5] ?? 1), -1]);
  let veil = 0;
  for (const source of glare.sources) {
    const d = unit(rotated(glare.viewRotation, source.direction));
    const cross = Math.hypot(
      d[1] * pixel[2] - d[2] * pixel[1],
      d[2] * pixel[0] - d[0] * pixel[2],
      d[0] * pixel[1] - d[1] * pixel[0],
    );
    const theta = Math.atan2(cross, d[0] * pixel[0] + d[1] * pixel[1] + d[2] * pixel[2]);
    veil +=
      meterLuma(source.luminance) *
      glare.preExposure *
      glareSourcePerUnit(source, theta, glare.terms);
  }
  return veil;
}

/**
 * The CPU twin of the kernel over RGBA texels, row-major.
 *
 * @param rgba - Four values per texel: pre-exposed linear red, green, blue and the meter class.
 * @param stride - 1, or 2 for the low setting's quarter-resolution input.
 * @param glare - The frame's glare sources, whose veil each weighed texel adds; none where absent.
 */
export function cpuHistogram(
  rgba: Float32Array,
  size: ViewSize,
  mode: HistogramMeter,
  stride: 1 | 2,
  glare?: MeteredGlare,
): Uint32Array {
  const bins = new Uint32Array(HISTOGRAM_BINS);
  const weights = meterWeights(mode);
  for (let y = 0; y < size.heightPx; y += stride) {
    for (let x = 0; x < size.widthPx; x += stride) {
      const at = 4 * (y * size.widthPx + x);
      const weight = weights[meterClassOf(rgba[at + 3] ?? 0)] ?? 0;
      if (weight === 0) {
        continue;
      }
      const stored = meterLuma([rgba[at] ?? 0, rgba[at + 1] ?? 0, rgba[at + 2] ?? 0]);
      const veil = glare === undefined ? 0 : meteredVeil(glare, size, x, y);
      const bin = histogramBin(stored + veil);
      bins[bin] = (bins[bin] ?? 0) + weight;
    }
  }
  return bins;
}

/**
 * The kernel's uniform `Params`: the weights, the size, the stride and the glare sources' count,
 * as `u32`.
 */
export function histogramParams(
  size: ViewSize,
  mode: HistogramMeter,
  stride: 1 | 2,
  sourceCount: number,
): Uint32Array {
  return new Uint32Array([...meterWeights(mode), size.widthPx, size.heightPx, stride, sourceCount]);
}

/**
 * The kernel's uniform `MeteredGlare`: the view rotation and projection, then the spread function
 * as the tone-mapping pass's `GlareTerms` takes it (`packGlareTerms`), 52 `f32`.
 */
export function histogramGlareUniform(
  glare: Pick<MeteredGlare, "viewRotation" | "projection" | "terms">,
): Float32Array {
  const terms = packGlareTerms(glare.terms);
  const out = new Float32Array(52);
  out.set(glare.viewRotation.subarray(0, 16), 0);
  out.set(glare.projection.subarray(0, 16), 16);
  ["glarePoisson0", "glarePoisson1", "glarePoisson2", "glareBroad", "glareMisc"].forEach(
    (name, i) => {
      out.set(terms[name] ?? new Float32Array(4), 32 + 4 * i);
    },
  );
  return out;
}

/** The workgroups that cover `size` read at `stride`. */
export function histogramWorkgroups(
  size: ViewSize,
  stride: 1 | 2,
): readonly [number, number, number] {
  return [
    Math.ceil(Math.ceil(size.widthPx / stride) / HISTOGRAM_WORKGROUP),
    Math.ceil(Math.ceil(size.heightPx / stride) / HISTOGRAM_WORKGROUP),
    1,
  ];
}

/** The engine calls the reader makes. */
export type HistogramEngine = Pick<
  RenderEngine,
  "createBuffer" | "writeBuffer" | "dispatch" | "readBuffer" | "releaseBuffer"
>;

/** The reads a {@link HistogramReader} keeps in flight at most. */
export const HISTOGRAM_RING = 3;

/** The pass label the histogram's dispatch is timed under (R07's `PHOTOREAL_PASS_LABELS`). */
export const HISTOGRAM_PASS = "histogram";

/** One request to {@link HistogramReader.measure}. */
export interface HistogramRequest {
  /** The view's HDR colour, `rgba16float` with the meter class in alpha. */
  readonly hdrColour: TextureHandle;
  readonly size: ViewSize;
  readonly mode: HistogramMeter;
  /** 1 on the high setting, 2 on the low (Design note 18). */
  readonly stride: 1 | 2;
  /** The scale the target was pre-exposed with, 1 ÷ (cd/m²). */
  readonly preExposure: number;
  /** The frame's glare sources, as the tone-mapping pass reads them. */
  readonly glare: HistogramGlare;
}

/** The frame's glare sources as the kernel binds them: the tone-mapping pass's own buffer. */
export interface HistogramGlare {
  /** The sources as `packGlareSources` packs them, at the frame's pre-exposure. */
  readonly sources: BufferHandle;
  /** The sources in the buffer. */
  readonly count: number;
  readonly viewRotation: Float32Array;
  readonly projection: Float32Array;
  readonly terms: GlareSpreadTerms;
}

/**
 * Takes a view's histogram each frame and delivers each one once read back, one to three frames
 * late, never more than {@link HISTOGRAM_RING} at a time.
 *
 * @remarks
 * Each slot of the ring is a storage buffer that a measurement zeroes, the kernel fills and
 * `readBuffer` copies; a slot is reused only once its read has settled. A frame finding every slot
 * in flight takes no histogram. A result older than one already delivered is dropped. After a
 * device loss the owner makes a new reader in `onRestored`, as for every other handle.
 */
export class HistogramReader {
  readonly #engine: HistogramEngine;
  readonly #kernel: ComputeHandle;
  readonly #slots: ReadonlyArray<BufferHandle>;
  readonly #busy: boolean[];
  readonly #zeros = new Uint32Array(HISTOGRAM_BINS);
  readonly #onHistogram: (histogram: Histogram) => void;
  #issued = 0;
  #delivered = -1;
  #disposed = false;

  /**
   * Makes the ring's three buffers, labelled after `name`.
   *
   * @param kernel - {@link HISTOGRAM_KERNEL}, made by `createComputeAsync`.
   * @param name - The view's name, which labels the buffers.
   * @param onHistogram - Called with each histogram as it arrives, in order of measurement.
   */
  constructor(
    engine: HistogramEngine,
    kernel: ComputeHandle,
    name: string,
    onHistogram: (histogram: Histogram) => void,
  ) {
    this.#engine = engine;
    this.#kernel = kernel;
    this.#onHistogram = onHistogram;
    const slots: BufferHandle[] = [];
    for (let i = 0; i < HISTOGRAM_RING; i += 1) {
      slots.push(
        engine.createBuffer({
          name: `${name} histogram ${i}`,
          bytes: HISTOGRAM_BINS * 4,
          usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC | BUFFER_USAGE.COPY_DST,
          category: "other",
        }),
      );
    }
    this.#slots = slots;
    this.#busy = slots.map(() => false);
  }

  /** The reads now in flight. */
  get inFlight(): number {
    return this.#busy.filter(Boolean).length;
  }

  /**
   * Takes this frame's histogram.
   *
   * @returns Whether it was taken: `false` when every slot is still being read.
   */
  measure(request: HistogramRequest): boolean {
    const slot = this.#busy.indexOf(false);
    const buffer = this.#slots[slot];
    if (this.#disposed || buffer === undefined) {
      return false;
    }
    this.#busy[slot] = true;
    const sequence = this.#issued;
    this.#issued += 1;
    this.#engine.writeBuffer(buffer, 0, this.#zeros);
    const { glare } = request;
    this.#engine.dispatch(
      this.#kernel,
      {
        uniforms: {
          params: histogramParams(request.size, request.mode, request.stride, glare.count),
          glare: histogramGlareUniform(glare),
        },
        buffers: { bins: buffer, glareSources: glare.sources },
        sampled: { hdr: request.hdrColour },
        storage: {},
      },
      histogramWorkgroups(request.size, request.stride),
      HISTOGRAM_PASS,
    );
    void this.#read(buffer, slot, sequence, request.preExposure).catch((error: unknown) => {
      if (!this.#disposed) {
        console.warn(`histogram read ${sequence} failed:`, error);
      }
    });
    return true;
  }

  /** Reads one slot back, frees it, and delivers the histogram unless a later one came first. */
  async #read(
    buffer: BufferHandle,
    slot: number,
    sequence: number,
    preExposure: number,
  ): Promise<void> {
    let bytes: ArrayBuffer;
    try {
      bytes = await this.#engine.readBuffer(buffer);
    } finally {
      this.#busy[slot] = false;
    }
    if (this.#disposed || sequence < this.#delivered) {
      return;
    }
    this.#delivered = sequence;
    this.#onHistogram({ bins: new Uint32Array(bytes), preExposure });
  }

  /**
   * Drops the reads now in flight: none of them is delivered, so that the next histogram delivered
   * is of an image measured after the call (a cut's, `AutoExposure.cut`).
   */
  dropInFlight(): void {
    this.#delivered = this.#issued;
  }

  /**
   * Stops delivering and releases the ring's buffers; reads in flight settle unseen (each read
   * copies into its own staging buffer, and the device destroys a released buffer only once its
   * work is done).
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    for (const slot of this.#slots) {
      this.#engine.releaseBuffer(slot);
    }
  }
}
