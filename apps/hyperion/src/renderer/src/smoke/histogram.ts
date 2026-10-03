/**
 * R07.T12's smoke checks: the exposure-histogram kernel against its CPU twin, bin for bin, on a
 * synthetic `rgba16float` target, under each meter and both strides, and through the reader's ring.
 *
 * @remarks
 * The target is the test's own (decision 2026-10-02, item 1: R07.T7 makes the views' scene
 * targets). Every luminance sits at a bin's centre in log₂, 0.059 stop from either edge, so the
 * GPU's `f32` luminance and `log2` cannot move a texel across a bin.
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { ComputeHandle, RenderEngine, TextureHandle, ViewSize } from "../view/engine/types";
import {
  binCentreLuminance,
  cpuHistogram,
  HISTOGRAM_BINS,
  HISTOGRAM_KERNEL,
  HISTOGRAM_PASS,
  HISTOGRAM_RING,
  histogramParams,
  histogramWorkgroups,
  HistogramReader,
  type Histogram,
} from "../view/post/histogram";
import type { MeterMode } from "../view/post/meter";
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

/** Runs the kernel once into a fresh buffer and reads it back. */
async function gpuHistogram(
  engine: RenderEngine,
  kernel: ComputeHandle,
  hdr: TextureHandle,
  mode: MeterMode,
  stride: 1 | 2,
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
      uniforms: { params: histogramParams(SIZE, mode, stride) },
      buffers: { bins },
      sampled: { hdr },
      storage: {},
    },
    histogramWorkgroups(SIZE, stride),
    HISTOGRAM_PASS,
  );
  return new Uint32Array(await engine.readBuffer(bins));
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
  const hdr = engine.createTexture({
    name: "smoke histogram input",
    size: [SIZE.widthPx, SIZE.heightPx],
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeTexture(
    hdr,
    { x: 0, y: 0 },
    { width: SIZE.widthPx, height: SIZE.heightPx },
    frame.bits,
  );
  for (const mode of ["average", "lit", "dark"] as const) {
    for (const stride of [1, 2] as const) {
      // Each case reads back before the next dispatches, as the harness's checks do.
      // oxlint-disable-next-line no-await-in-loop
      const gpu = await gpuHistogram(engine, kernel, hdr, mode, stride);
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

  const delivered: Histogram[] = [];
  const request = {
    hdrColour: hdr,
    size: SIZE,
    mode: "average",
    stride: 1,
    preExposure: 1,
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
  const cpu = cpuHistogram(frame.values, SIZE, "average", 1);
  checks.check(
    "R07.T12 the reader keeps at most three reads in flight and delivers intact histograms",
    taken.filter(Boolean).length === HISTOGRAM_RING &&
      delivered.length >= 1 &&
      delivered.every((histogram) => firstDifference(histogram.bins, cpu) === null),
    `taken ${JSON.stringify(taken)}, ${delivered.length} delivered`,
  );
}
