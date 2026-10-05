import { describe, expect, it, vi } from "vitest";

import type { BufferHandle, ComputeHandle, TextureHandle } from "../engine/types";
import type { BufferSpec } from "../engine/memory";
import {
  binCentreLuminance,
  cpuHistogram,
  HISTOGRAM_BINS,
  HISTOGRAM_KERNEL,
  HISTOGRAM_RING,
  histogramBin,
  histogramParams,
  histogramWorkgroups,
  meterClassOf,
  HistogramReader,
  type Histogram,
  type HistogramEngine,
} from "./histogram";
import { METER_CLASS, meterWeights, type MeterMode } from "./meter";

/** RGBA texels of grey `luminance` and meter class `meterClass`, one per entry. */
function texels(entries: ReadonlyArray<readonly [number, number]>): Float32Array {
  const out = new Float32Array(4 * entries.length);
  entries.forEach(([luminance, meterClass], i) => {
    out.set([luminance, luminance, luminance, meterClass], 4 * i);
  });
  return out;
}

describe("histogramBin", () => {
  it("puts zeros, NaN and everything below 2⁻¹⁴ in bin 0", () => {
    expect(histogramBin(0)).toBe(0);
    expect(histogramBin(Number.NaN)).toBe(0);
    expect(histogramBin(2 ** -15)).toBe(0);
    expect(histogramBin(2 ** -14)).toBe(1);
  });

  it("spans 30 stops in 255 bins, 0.118 stop each", () => {
    expect(histogramBin(2 ** -14 * 2 ** (1 / 8.5) * 1.0001)).toBe(2);
    expect(histogramBin(1)).toBe(1 + 14 * 8.5);
    expect(histogramBin(65_504)).toBe(255);
    expect(histogramBin(1e30)).toBe(255);
    expect(histogramBin(Infinity)).toBe(255);
  });

  it("maps each bin's centre back to that bin", () => {
    expect(histogramBin(binCentreLuminance(77))).toBe(77);
    for (let bin = 1; bin < HISTOGRAM_BINS; bin += 1) {
      expect(histogramBin(binCentreLuminance(bin))).toBe(bin);
    }
    expect(binCentreLuminance(0)).toBe(0);
  });
});

describe("meterWeights", () => {
  it("weighs every class but the host disc under AVG, only lit or unlit bodies under LIT or DARK", () => {
    expect(meterWeights("average")).toEqual([0, 1, 1, 1]);
    expect(meterWeights("lit")).toEqual([0, 0, 1, 0]);
    expect(meterWeights("dark")).toEqual([0, 0, 0, 1]);
  });
});

/** The sum of a histogram's counts. */
function total(bins: Uint32Array): number {
  return bins.reduce((sum, n) => sum + n, 0);
}

describe("meterClassOf", () => {
  it("rounds halves to even, as WGSL's round does, within [0, 3]", () => {
    expect([0.5, 1.5, 2.5, 2.4, -1, 7].map(meterClassOf)).toEqual([0, 2, 2, 2, 0, 3]);
  });
});

describe("cpuHistogram", () => {
  const size = { widthPx: 4, heightPx: 1 };
  const frame = texels([
    [binCentreLuminance(200), METER_CLASS.hostDisc],
    [binCentreLuminance(100), METER_CLASS.other],
    [binCentreLuminance(150), METER_CLASS.litBody],
    [0, METER_CLASS.unlitBody],
  ]);

  it("never counts host-disc pixels", () => {
    for (const mode of ["average", "lit", "dark"] as const) {
      expect(cpuHistogram(frame, size, mode, 1)[200]).toBe(0);
    }
  });

  it("counts each meter's classes only, and a black pixel in bin 0", () => {
    const expected: Readonly<Record<MeterMode, ReadonlyArray<number>>> = {
      average: [0, 100, 150],
      lit: [150],
      dark: [0],
    };
    for (const mode of ["average", "lit", "dark"] as const) {
      const bins = cpuHistogram(frame, size, mode, 1);
      const filled = [...bins.keys()].filter((bin) => (bins[bin] ?? 0) > 0);
      expect(filled).toEqual(expected[mode]);
    }
  });

  it("reads every other texel on each axis at stride 2", () => {
    const quad = { widthPx: 4, heightPx: 4 };
    const all = texels(Array.from({ length: 16 }, () => [1, METER_CLASS.other] as const));
    expect(total(cpuHistogram(all, quad, "average", 1))).toBe(16);
    expect(total(cpuHistogram(all, quad, "average", 2))).toBe(4);
  });
});

describe("the kernel's inputs", () => {
  it("is bit-exact with no subgroup twin", () => {
    expect(HISTOGRAM_KERNEL.readback).toBe("bit-exact");
    expect(HISTOGRAM_KERNEL.subgroup).toBeNull();
  });

  it("lays out Params as weights, size, stride", () => {
    expect([...histogramParams({ widthPx: 640, heightPx: 360 }, "lit", 2)]).toEqual([
      0, 0, 1, 0, 640, 360, 2, 0,
    ]);
  });

  it("covers the input with 16 × 16 workgroups", () => {
    expect(histogramWorkgroups({ widthPx: 1920, heightPx: 1080 }, 1)).toEqual([120, 68, 1]);
    expect(histogramWorkgroups({ widthPx: 1280, heightPx: 720 }, 2)).toEqual([40, 23, 1]);
  });
});

/** An engine that records the reader's calls and settles each read when the test says. */
class ReadbackEngine implements HistogramEngine {
  readonly created: BufferHandle[] = [];
  readonly pending: Array<{
    buffer: BufferHandle;
    settle: (bytes: ArrayBuffer) => void;
    reject: (error: Error) => void;
  }> = [];
  readonly dispatched: BufferHandle[] = [];
  readonly released: BufferHandle[] = [];
  readonly violations: string[] = [];

  createBuffer(spec: BufferSpec): BufferHandle {
    const handle: BufferHandle = { kind: "buffer", name: spec.name, bytes: spec.bytes };
    this.created.push(handle);
    return handle;
  }
  writeBuffer(buffer: BufferHandle): void {
    if (this.pending.some((read) => read.buffer === buffer)) {
      this.violations.push(`zeroed ${buffer.name} while it was being read`);
    }
  }
  dispatch(_kernel: ComputeHandle, bindings: Parameters<HistogramEngine["dispatch"]>[1]): void {
    const buffer = bindings.buffers["bins"];
    if (buffer === undefined) {
      throw new Error("no bins bound");
    }
    if (this.pending.some((read) => read.buffer === buffer)) {
      this.violations.push(`wrote ${buffer.name} while it was being read`);
    }
    this.dispatched.push(buffer);
  }
  readBuffer(buffer: BufferHandle): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) => {
      this.pending.push({ buffer, settle: resolve, reject });
    });
  }
  releaseBuffer(buffer: BufferHandle): void {
    this.released.push(buffer);
  }
  /** Fails the read at `index` of those pending. */
  fail(index: number): void {
    const [read] = this.pending.splice(index, 1);
    read?.reject(new Error("lost"));
  }
  /** Settles the read at `index` of those pending with a histogram whose bin 1 holds `count`. */
  settle(index: number, count: number): void {
    const [read] = this.pending.splice(index, 1);
    const bins = new Uint32Array(HISTOGRAM_BINS);
    bins[1] = count;
    read?.settle(bins.buffer);
  }
}

const KERNEL: ComputeHandle = { kind: "compute", name: "exposure histogram", path: "reference" };
const HDR: TextureHandle = { kind: "texture", name: "view:hdr" };
const REQUEST = {
  hdrColour: HDR,
  size: { widthPx: 64, heightPx: 32 },
  mode: "average",
  stride: 1,
  preExposure: 0.5,
} as const;

/** Lets every settled promise's callbacks run. */
async function flush(): Promise<void> {
  // The reader's read settles, its `finally` runs, then its delivery: a few microtask turns.
  await Promise.resolve()
    .then(() => undefined)
    .then(() => undefined)
    .then(() => undefined)
    .then(() => undefined);
}

describe("HistogramReader", () => {
  it("keeps at most three reads in flight and never touches a buffer being read", async () => {
    const engine = new ReadbackEngine();
    const reader = new HistogramReader(engine, KERNEL, "view", () => undefined);
    expect(engine.created).toHaveLength(HISTOGRAM_RING);
    const taken = [1, 2, 3, 4, 5].map(() => reader.measure(REQUEST));
    expect(taken).toEqual([true, true, true, false, false]);
    expect(reader.inFlight).toBe(3);
    engine.settle(1, 7);
    await flush();
    expect(reader.inFlight).toBe(2);
    expect(reader.measure(REQUEST)).toBe(true);
    expect(engine.dispatched.at(-1)).toBe(engine.created[1]);
    expect(engine.violations).toEqual([]);
  });

  it("delivers histograms in order of measurement with their pre-exposure", async () => {
    const engine = new ReadbackEngine();
    const seen: Histogram[] = [];
    const reader = new HistogramReader(engine, KERNEL, "view", (h) => seen.push(h));
    reader.measure(REQUEST);
    reader.measure(REQUEST);
    engine.settle(1, 2);
    await flush();
    engine.settle(0, 1);
    await flush();
    expect(seen.map((h) => h.bins[1])).toEqual([2]);
    expect(seen[0]?.preExposure).toBe(0.5);
  });

  it("frees a slot whose read fails, warning only while live", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const engine = new ReadbackEngine();
    const reader = new HistogramReader(engine, KERNEL, "view", () => undefined);
    for (let i = 0; i < HISTOGRAM_RING; i += 1) {
      reader.measure(REQUEST);
    }
    engine.fail(0);
    await flush();
    expect(reader.inFlight).toBe(2);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(reader.measure(REQUEST)).toBe(true);
    reader.dispose();
    engine.fail(0);
    await flush();
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it("delivers nothing once disposed", async () => {
    const engine = new ReadbackEngine();
    const seen: Histogram[] = [];
    const reader = new HistogramReader(engine, KERNEL, "view", (h) => seen.push(h));
    reader.measure(REQUEST);
    reader.dispose();
    engine.settle(0, 3);
    await flush();
    expect(seen).toEqual([]);
    expect(reader.measure(REQUEST)).toBe(false);
  });

  it("releases its ring's buffers once when disposed (R07.T8.a)", () => {
    const engine = new ReadbackEngine();
    const reader = new HistogramReader(engine, KERNEL, "view", () => undefined);
    reader.dispose();
    reader.dispose();
    expect(engine.released).toEqual(engine.created);
  });
});
