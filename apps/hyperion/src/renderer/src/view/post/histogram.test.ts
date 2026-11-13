import { describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { perspectiveReversedInfinite, viewRotation4 } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import type { ViewRole } from "../camera/state";
import type { BufferHandle, ComputeHandle, TextureHandle } from "../engine/types";
import type { BufferSpec } from "../engine/memory";
import { ev100FromAverageLuminance, exposureScale } from "../photometry/exposure";
import { meteredLuminance } from "./autoExposure";
import { type GlareSource, glareSourceVeilOf, glareSpreadTerms } from "./glare";
import {
  binCentreLuminance,
  cpuHistogram,
  HISTOGRAM_BINS,
  HISTOGRAM_KERNEL,
  HISTOGRAM_RING,
  histogramBin,
  histogramGlareUniform,
  histogramParams,
  histogramWorkgroups,
  meterClassOf,
  HistogramReader,
  type Histogram,
  type HistogramEngine,
  type MeteredGlare,
  meteredVeil,
} from "./histogram";
import { type HistogramMeter, METER_CLASS, meterLuma, meterWeights } from "./meter";

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
    const expected: Readonly<Record<HistogramMeter, ReadonlyArray<number>>> = {
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

const EYE = { ageYears: 25, pigmentation: 0.5 };

/** The Sun at 1 au (`sunLikeHostDisc`'s mean luminance, 1.964 × 10⁹ cd/m²), at a view direction. */
function sunAt(direction: {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}): GlareSource {
  const luminance = 1.964e9;
  return {
    direction: vec3(direction.x, direction.y, direction.z),
    angularRadiusRad: Math.asin(6.957e8 / 1.495978707e11),
    // The part an EV100 −10 target cannot hold, which the image spreads in closed form.
    excessLuminance: [0.96 * luminance, 0.96 * luminance, 0.96 * luminance],
    luminance: [luminance, luminance, luminance],
  };
}

/** 60° across at 480 × 270 px: the Sun at 1 au 1.93 px in radius at the centre. */
const FRAME = { widthPx: 480, heightPx: 270 };

function frameGlare(
  role: ViewRole,
  ev100: number,
  sources: ReadonlyArray<GlareSource>,
): MeteredGlare {
  return {
    sources,
    preExposure: exposureScale(ev100),
    viewRotation: viewRotation4(IDENTITY_QUATERNION),
    projection: perspectiveReversedInfinite(Math.PI / 3, FRAME.widthPx / FRAME.heightPx, 0.1),
    terms: glareSpreadTerms(role, EYE),
  };
}

/** The angle of texel (x, y)'s centre from the frame's centre, rad. */
function fromCentreRad(x: number, y: number): number {
  const s = Math.tan(Math.PI / 6);
  const v = [
    (((x + 0.5) / FRAME.widthPx) * 2 - 1) * s,
    (1 - ((y + 0.5) / FRAME.heightPx) * 2) * (s / (FRAME.widthPx / FRAME.heightPx)),
    -1,
  ] as const;
  return Math.acos(1 / Math.hypot(...v));
}

/**
 * A frame of black sky with the star's disc at its centre, and a lit planet of `planetCdM2` 60 px
 * in radius 20° to the right, or none; pre-exposed by `preExposure`.
 */
function starFrame(preExposure: number, planetCdM2: number | null, star: boolean): Float32Array {
  const rho = Math.asin(6.957e8 / 1.495978707e11);
  const rgba = new Float32Array(4 * FRAME.widthPx * FRAME.heightPx);
  for (let y = 0; y < FRAME.heightPx; y += 1) {
    for (let x = 0; x < FRAME.widthPx; x += 1) {
      const at = 4 * (y * FRAME.widthPx + x);
      if (star && fromCentreRad(x, y) < rho) {
        rgba.set([65_504, 65_504, 65_504, METER_CLASS.hostDisc], at);
      } else if (planetCdM2 !== null && Math.hypot(x + 0.5 - 400, y + 0.5 - 135) < 60) {
        const value = planetCdM2 * preExposure;
        rgba.set([value, value, value, METER_CLASS.litBody], at);
      } else {
        rgba.set([0, 0, 0, METER_CLASS.other], at);
      }
    }
  }
  return rgba;
}

/** The EV100 a frame pre-exposed as `glare` meters at under `mode`, the veil in it or not. */
function meteredEv(
  rgba: Float32Array,
  mode: HistogramMeter,
  glare: MeteredGlare,
  veil = true,
): number {
  const bins = cpuHistogram(rgba, FRAME, mode, 1, veil ? glare : undefined);
  return ev100FromAverageLuminance(meteredLuminance({ bins, preExposure: glare.preExposure }));
}

describe("the meter with the glare sources' veil (fix-view-light)", () => {
  const centre = vec3(0, 0, -1);

  it.each(["eye", "camera"] as const)(
    "meters a star on black within 1% of the veil summed over the frame outside its disc (%s)",
    (role) => {
      const glare = frameGlare(role, 15, [sunAt(centre)]);
      const source = glare.sources[0];
      if (source === undefined) {
        throw new Error("no source");
      }
      const whole = { ...source, excessLuminance: source.luminance };
      const rgba = starFrame(glare.preExposure, null, true);
      let sum = 0;
      let count = 0;
      for (let y = 0; y < FRAME.heightPx; y += 1) {
        for (let x = 0; x < FRAME.widthPx; x += 1) {
          if ((rgba[4 * (y * FRAME.widthPx + x) + 3] ?? 0) === METER_CLASS.hostDisc) {
            continue;
          }
          sum += meterLuma(glareSourceVeilOf(whole, fromCentreRad(x, y), glare.terms));
          count += 1;
        }
      }
      const bins = cpuHistogram(rgba, FRAME, "average", 1, glare);
      const metered = meteredLuminance({ bins, preExposure: glare.preExposure });
      expect(Math.abs(metered / (sum / count) - 1)).toBeLessThan(0.01);
    },
  );

  it("weighs nothing of the disc's own rgba, whatever the veil there", () => {
    const glare = frameGlare("eye", 15, [sunAt(centre)]);
    const disc = new Float32Array([65_504, 65_504, 65_504, METER_CLASS.hostDisc]);
    const size = { widthPx: 1, heightPx: 1 };
    const one = { ...glare, projection: perspectiveReversedInfinite(1e-3, 1, 0.1) };
    expect(meteredVeil(one, size, 0, 0)).toBeGreaterThan(1e3);
    expect(cpuHistogram(disc, size, "average", 1, one).every((n) => n === 0)).toBe(true);
  });

  it("is the same light at every pre-exposure in range, so the meter cannot chase its own exposure", () => {
    // The image's veil is the excess's alone, which vanishes at the exposure the whole light calls
    // for (the Sun's disc fits in the target from EV100 about 14.9): a meter of it would hunt.
    const evs = [13, 15, 17, 19].map((ev100) => {
      const glare = frameGlare("eye", ev100, [sunAt(centre)]);
      return meteredEv(starFrame(glare.preExposure, null, true), "average", glare);
    });
    for (const ev of evs) {
      expect(Math.abs(ev - (evs[1] ?? 0))).toBeLessThan(0.05);
    }
  });

  // The research's figures are 15.0 and 15.4 for the camera, 17.8 and 17.9 for the eye, from the
  // spread function at the point form; an exact convolution over the disc gives the eye 18.22,
  // and the closed form's own +7% to +13% near the limb (`glareSourceVeilOf`) the rest of a
  // +0.55 EV finding, pinned as it stands.
  it.each([
    { role: "camera", alone: 14.8, withPlanet: 15.22 },
    { role: "eye", alone: 18.35, withPlanet: 18.39 },
  ] as const)(
    "puts AVG at EV100 $alone with the Sun alone at the centre of a 60° frame ($role)",
    ({ role, alone, withPlanet }) => {
      // A planet adding 1,200 cd/m² to the frame's mean, as the research's figures take it.
      const planet = 1200 / ((Math.PI * 60 * 60) / (FRAME.widthPx * FRAME.heightPx));
      const glare = frameGlare(role, 15, [sunAt(centre)]);
      const starAlone = meteredEv(starFrame(glare.preExposure, null, true), "average", glare);
      const both = meteredEv(starFrame(glare.preExposure, planet, true), "average", glare);
      expect([starAlone, both].map((ev) => Math.round(ev * 100) / 100)).toEqual([
        alone,
        withPlanet,
      ]);
    },
  );

  it.each(["camera", "eye"] as const)(
    "leaves LIT at the lit planet's own EV100 as the Sun enters the frame 20° from it (%s)",
    (role) => {
      const planet = 1200 / ((Math.PI * 60 * 60) / (FRAME.widthPx * FRAME.heightPx));
      const glare = frameGlare(role, 16, [sunAt(centre)]);
      const own = meteredEv(starFrame(glare.preExposure, planet, false), "lit", glare, false);
      const withStar = meteredEv(starFrame(glare.preExposure, planet, true), "lit", glare);
      expect(withStar - own).toBeGreaterThan(0);
      expect(withStar - own).toBeLessThan(0.3);
    },
  );
});

describe("the kernel's inputs", () => {
  it("is bit-exact with no subgroup twin", () => {
    expect(HISTOGRAM_KERNEL.readback).toBe("bit-exact");
    expect(HISTOGRAM_KERNEL.subgroup).toBeNull();
  });

  it("lays out Params as weights, size, stride and the glare sources' count", () => {
    expect([...histogramParams({ widthPx: 640, heightPx: 360 }, "lit", 2, 3)]).toEqual([
      0, 0, 1, 0, 640, 360, 2, 3,
    ]);
  });

  it("lays out MeteredGlare as the view rotation, the projection and GlareTerms", () => {
    const rotation = viewRotation4(IDENTITY_QUATERNION);
    const projection = perspectiveReversedInfinite(Math.PI / 3, 16 / 9, 0.1);
    const terms = glareSpreadTerms("camera", EYE);
    const packed = histogramGlareUniform({ viewRotation: rotation, projection, terms });
    expect([packed.length, ...packed.subarray(0, 16)]).toEqual([52, ...rotation]);
    expect([...packed.subarray(16, 32)]).toEqual([...projection]);
    // The camera's Lorentz tail, then its Gaussian core.
    expect([...packed.subarray(44, 46)]).toEqual(
      [terms.lorentz[0]?.amplitude, terms.lorentz[0]?.scaleRad].map((v) => Math.fround(v ?? 0)),
    );
    expect(packed[50]).toBe(Math.fround(terms.gaussian.amplitude));
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
  glare: {
    sources: { kind: "buffer", name: "view glare sources", bytes: 48 },
    count: 0,
    viewRotation: viewRotation4(IDENTITY_QUATERNION),
    projection: perspectiveReversedInfinite(Math.PI / 3, 2, 0.1),
    terms: glareSpreadTerms("eye", { ageYears: 25, pigmentation: 0.5 }),
  },
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

  it("drops the reads in flight at a cut, delivering the next one measured (fix-view-light)", async () => {
    const engine = new ReadbackEngine();
    const seen: Histogram[] = [];
    const reader = new HistogramReader(engine, KERNEL, "view", (h) => seen.push(h));
    reader.measure(REQUEST);
    reader.measure(REQUEST);
    reader.dropInFlight();
    reader.measure(REQUEST);
    engine.settle(0, 1);
    engine.settle(0, 2);
    await flush();
    engine.settle(0, 3);
    await flush();
    expect(seen.map((h) => h.bins[1])).toEqual([3]);
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
