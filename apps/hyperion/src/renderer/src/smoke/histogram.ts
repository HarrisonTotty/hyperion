/**
 * R07.T12's smoke checks: the exposure-histogram kernel against its CPU twin, bin for bin, on a
 * synthetic `rgba16float` target, under each meter and both strides, with no glare source and with
 * one in the frame, and through the reader's ring.
 *
 * @remarks
 * The target is the test's own (decision 2026-10-02, item 1: R07.T7 makes the views' scene
 * targets). Every luminance sits at a bin's centre in log₂, 0.059 stop from either edge, so the
 * GPU's `f32` luminance and `log2` cannot move a texel across a bin. With the glare source each
 * texel stores a bin's centre less the CPU's veil there, so that the stored value and the veil
 * together sit at the centre, and a black texel only where the veil alone sits within a bin.
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { ComputeHandle, RenderEngine, TextureHandle, ViewSize } from "../view/engine/types";
import { vec3 } from "../geometry/vec3";
import { perspectiveReversedInfinite, viewRotation4 } from "../view/camera/projection";
import { quaternionFromAxisAngle, rotate } from "../view/camera/quaternion";
import { GLARE_SOURCE_BYTES, packGlareSources } from "../view/post/bloomChain";
import { type GlareSource, glareSpreadTerms } from "../view/post/glare";
import {
  binCentreLuminance,
  cpuHistogram,
  HISTOGRAM_BINS,
  HISTOGRAM_BINS_PER_STOP,
  HISTOGRAM_KERNEL,
  HISTOGRAM_MIN_LOG2,
  HISTOGRAM_PASS,
  HISTOGRAM_RING,
  histogramBin,
  histogramGlareUniform,
  histogramParams,
  histogramWorkgroups,
  HistogramReader,
  type Histogram,
  type HistogramGlare,
  type MeteredGlare,
  meteredVeil,
} from "../view/post/histogram";
import { type HistogramMeter, METER_CLASS } from "../view/post/meter";
import { type Checks, halfBits } from "./harness";

/** The synthetic target's size: not a multiple of the 16 × 16 workgroup on either axis. */
const SIZE: ViewSize = { widthPx: 70, heightPx: 45 };

/** A half float from its bits, positive values only. */
function halfValue(bits: number): number {
  const exponent = bits >> 10;
  const mantissa = bits & 0x3ff;
  if (exponent === 0x1f) {
    return Infinity;
  }
  return exponent === 0 ? mantissa * 2 ** -24 : (1 + mantissa / 1024) * 2 ** (exponent - 15);
}

/** A half float's +∞, which a pass writing above 65,504 stores. */
const HALF_INFINITY = 0x7c00;

/**
 * The synthetic frame: grey texels at bin centres across the whole range, every meter class, one
 * in seven black and one in eleven infinite, as half-float bits and as the values the target holds.
 */
function syntheticFrame(): { readonly bits: Uint16Array; readonly values: Float32Array } {
  const count = SIZE.widthPx * SIZE.heightPx;
  const bits = new Uint16Array(4 * count);
  const values = new Float32Array(4 * count);
  for (let i = 0; i < count; i += 1) {
    const bin = 1 + ((i * 37) % (HISTOGRAM_BINS - 1));
    const grey = i % 7 === 0 ? 0 : i % 11 === 5 ? HALF_INFINITY : halfBits(binCentreLuminance(bin));
    const meterClass = halfBits(Math.floor(i / 3) % 4);
    bits.set([grey, grey, grey, meterClass], 4 * i);
    const g = halfValue(grey);
    values.set([g, g, g, halfValue(meterClass)], 4 * i);
  }
  return { bits, values };
}

/** The glare frame's field across its width, rad. */
const GLARE_FOV_X_RAD = (60 * Math.PI) / 180;

/** The glare frame's view: turned 20° about an oblique axis, so that its rotation is exercised. */
const GLARE_ORIENTATION = quaternionFromAxisAngle(vec3(0.36, 0.48, 0.8), (20 * Math.PI) / 180);

/**
 * The frame's one glare source: an eye view's Sun-like disc 2° in radius at pixel (24, 19), its
 * narrow terms over its rectangle, its whole light putting the veil from about 10⁻¹ to 10⁶ in the
 * target's units across the frame.
 */
function glareFrameSource(): MeteredGlare {
  const projection = perspectiveReversedInfinite(
    GLARE_FOV_X_RAD,
    SIZE.widthPx / SIZE.heightPx,
    0.1,
  );
  const ndcX = (24.5 / SIZE.widthPx) * 2 - 1;
  const ndcY = 1 - (19.5 / SIZE.heightPx) * 2;
  const view = vec3(ndcX / (projection[0] ?? 1), ndcY / (projection[5] ?? 1), -1);
  const length = Math.hypot(view.x, view.y, view.z);
  const direction = rotate(
    GLARE_ORIENTATION,
    vec3(view.x / length, view.y / length, view.z / length),
  );
  const source: GlareSource = {
    direction,
    angularRadiusRad: (2 * Math.PI) / 180,
    excessLuminance: [3e5, 2e5, 1e5],
    luminance: [4e5, 3e5, 2e5],
  };
  return {
    sources: [source],
    preExposure: 1,
    viewRotation: viewRotation4(GLARE_ORIENTATION),
    projection,
    terms: glareSpreadTerms("eye", { ageYears: 25, pigmentation: 0.5 }),
  };
}

/** Where `luminance` lies within its bin, as a fraction of the bin's width; 0.5 at its centre. */
function binFraction(luminance: number): number {
  const position = (Math.log2(luminance) - HISTOGRAM_MIN_LOG2) * HISTOGRAM_BINS_PER_STOP;
  return position - Math.floor(position);
}

/**
 * The synthetic frame under `glare`: each texel a bin's centre above its veil less the veil, so that
 * the two together sit at the centre; black where the veil alone sits well inside a bin, or below
 * the range; infinite in one texel in eleven and where the veil nears the top; every meter class,
 * and the host disc's inside its disc.
 */
function glareFrame(glare: MeteredGlare): {
  readonly bits: Uint16Array;
  readonly values: Float32Array;
} {
  const count = SIZE.widthPx * SIZE.heightPx;
  const bits = new Uint16Array(4 * count);
  const values = new Float32Array(4 * count);
  for (let i = 0; i < count; i += 1) {
    const x = i % SIZE.widthPx;
    const y = Math.floor(i / SIZE.widthPx);
    const veil = meteredVeil(glare, SIZE, x, y);
    const inDisc = Math.hypot(x - 24, y - 19) <= 2;
    const veilBin = histogramBin(veil);
    const blackFits =
      veil < 2 ** (HISTOGRAM_MIN_LOG2 - 1) ||
      (veilBin < HISTOGRAM_BINS - 1 && Math.abs(binFraction(veil) - 0.5) < 0.3);
    let grey: number;
    // Within two bins of the top the veil alone may meet the top bin's edge: the texel is made
    // infinite, the top bin's in both.
    if (i % 11 === 5 || veilBin >= HISTOGRAM_BINS - 3) {
      grey = HALF_INFINITY;
    } else if (i % 7 === 0 && blackFits) {
      grey = 0;
    } else {
      // A bin above the veil's, so that the stored value is at least 8% of the sum.
      const bin = Math.min(HISTOGRAM_BINS - 2, veilBin + 2 + ((i * 37) % 40));
      grey = halfBits(Math.max(0, binCentreLuminance(bin) - veil));
    }
    const meterClass = halfBits(inDisc ? METER_CLASS.hostDisc : Math.floor(i / 3) % 4);
    bits.set([grey, grey, grey, meterClass], 4 * i);
    const g = halfValue(grey);
    values.set([g, g, g, halfValue(meterClass)], 4 * i);
  }
  return { bits, values };
}

/** Runs the kernel once into a fresh buffer and reads it back. */
async function gpuHistogram(
  engine: RenderEngine,
  kernel: ComputeHandle,
  hdr: TextureHandle,
  mode: HistogramMeter,
  stride: 1 | 2,
  glare: HistogramGlare,
): Promise<Uint32Array> {
  const bins = engine.createBuffer({
    name: `smoke histogram ${mode} ${stride}`,
    bytes: HISTOGRAM_BINS * 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(bins, 0, new Uint32Array(HISTOGRAM_BINS));
  engine.dispatch(
    kernel,
    {
      uniforms: {
        params: histogramParams(SIZE, mode, stride, glare.count),
        glare: histogramGlareUniform(glare),
      },
      buffers: { bins, glareSources: glare.sources },
      sampled: { hdr },
      storage: {},
    },
    histogramWorkgroups(SIZE, stride),
    HISTOGRAM_PASS,
  );
  return new Uint32Array(await engine.readBuffer(bins));
}

/** An `rgba16float` input of `SIZE` holding `bits`. */
function inputTexture(engine: RenderEngine, name: string, bits: Uint16Array): TextureHandle {
  const texture = engine.createTexture({
    name,
    size: [SIZE.widthPx, SIZE.heightPx],
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(
    texture,
    { x: 0, y: 0 },
    { width: SIZE.widthPx, height: SIZE.heightPx },
    bits,
  );
  return texture;
}

/** The first bin where two histograms differ, or `null`. */
function firstDifference(a: Uint32Array, b: Uint32Array): string | null {
  for (let bin = 0; bin < HISTOGRAM_BINS; bin += 1) {
    if (a[bin] !== b[bin]) {
      return `bin ${bin}: GPU ${a[bin]}, CPU ${b[bin]}`;
    }
  }
  return null;
}

/** R07.T12: the kernel equals its twin bin for bin, and the reader delivers through its ring. */
export async function checkHistogram(engine: RenderEngine, checks: Checks): Promise<void> {
  const kernel = await engine.createComputeAsync(HISTOGRAM_KERNEL);
  checks.check(
    "R07.T12 the histogram runs on the reference path",
    kernel.path === "reference",
    kernel.path,
  );
  const frame = syntheticFrame();
  const hdr = inputTexture(engine, "smoke histogram input", frame.bits);
  const glare = glareFrameSource();
  const sources = engine.createBuffer({
    name: "smoke histogram glare sources",
    bytes: GLARE_SOURCE_BYTES,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(sources, 0, packGlareSources(glare.sources, glare.preExposure, glare.terms));
  const bound = (count: number): HistogramGlare => ({ ...glare, sources, count });
  for (const mode of ["average", "lit", "dark"] as const) {
    for (const stride of [1, 2] as const) {
      // Each case reads back before the next dispatches, as the harness's checks do.
      // oxlint-disable-next-line no-await-in-loop
      const gpu = await gpuHistogram(engine, kernel, hdr, mode, stride, bound(0));
      const cpu = cpuHistogram(frame.values, SIZE, mode, stride);
      const difference = firstDifference(gpu, cpu);
      const counted = gpu.reduce((sum, n) => sum + n, 0);
      checks.check(
        `R07.T12 the GPU histogram equals the CPU's bin for bin (${mode}, stride ${stride})`,
        difference === null && counted > 0,
        difference ?? `${counted} counts, bin 0 ${gpu[0]}, host-disc texels uncounted`,
      );
    }
  }
  const lit = glareFrame(glare);
  const litHdr = inputTexture(engine, "smoke histogram glare input", lit.bits);
  for (const mode of ["average", "lit"] as const) {
    for (const stride of [1, 2] as const) {
      // Each case reads back before the next dispatches, as the harness's checks do.
      // oxlint-disable-next-line no-await-in-loop
      const gpu = await gpuHistogram(engine, kernel, litHdr, mode, stride, bound(1));
      const cpu = cpuHistogram(lit.values, SIZE, mode, stride, glare);
      const difference = firstDifference(gpu, cpu);
      const counted = gpu.reduce((sum, n) => sum + n, 0);
      checks.check(
        `fix-view-light the GPU histogram with a glare source's veil equals the CPU's bin for bin (${mode}, stride ${stride})`,
        difference === null && counted > 0,
        difference ?? `${counted} counts over ${String(cpu.filter((n) => n > 0).length)} bins`,
      );
    }
  }

  const delivered: Histogram[] = [];
  const request = {
    hdrColour: hdr,
    size: SIZE,
    mode: "average",
    stride: 1,
    preExposure: 1,
    glare: bound(0),
  } as const;
  let finish: (() => void) | null = null;
  const reader = new HistogramReader(engine, kernel, "smoke", (histogram) => {
    delivered.push(histogram);
    if (delivered.length === HISTOGRAM_RING) {
      finish?.();
    }
  });
  let timer: ReturnType<typeof setTimeout> | undefined;
  const settled = new Promise<void>((resolve) => {
    finish = resolve;
    // A dropped or failed read must not hang the harness.
    timer = setTimeout(resolve, 10_000);
  });
  const taken = Array.from({ length: HISTOGRAM_RING + 2 }, () => reader.measure(request));
  await settled;
  clearTimeout(timer);
  reader.dispose();
  engine.releaseBuffer(sources);
  const cpu = cpuHistogram(frame.values, SIZE, "average", 1);
  checks.check(
    "R07.T12 the reader keeps at most three reads in flight and delivers intact histograms",
    taken.filter(Boolean).length === HISTOGRAM_RING &&
      delivered.length >= 1 &&
      delivered.every((histogram) => firstDifference(histogram.bins, cpu) === null),
    `taken ${JSON.stringify(taken)}, ${delivered.length} delivered`,
  );
}
