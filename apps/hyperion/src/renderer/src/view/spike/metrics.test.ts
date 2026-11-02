import { describe, expect, it } from "vitest";

import type { PassTimes } from "../engine/types";
import { SETTINGS } from "../quality/qualitySetting";
import {
  type FrameSample,
  SpikeMetrics,
  type SpikeMetricsOptions,
  TIMED_PASSES_A_FRAME,
} from "./metrics";
import type { SpikeGeometrySample } from "./spikeRun";

const OPTIONS: SpikeMetricsOptions = {
  warmupS: 10,
  segments: [
    { name: "orbit coast", startS: 0, endS: 2 },
    { name: "descent arc", startS: 2, endS: 4 },
  ],
  levels: [{ level: 19, epsilonM: 0.02, k: 5 }],
  rowOf: (label) =>
    label.startsWith("terrain")
      ? "terrain"
      : label.startsWith("atmosphere")
        ? "atmosphere"
        : "other",
  predicted: (segment) =>
    segment === "orbit coast"
      ? { hardPerS: 4, calibratedPerS: 2 }
      : { hardPerS: 16, calibratedPerS: 8 },
};

const EXTRA = {
  inFlightResolves: [],
  latePipelines: [],
  adapterPeakBytes: 1024,
  canvas: { widthPx: 1280, heightPx: 720 },
};

function sample(
  engineFrame: number,
  scriptTimeS: number,
  overrides: Partial<FrameSample> = {},
): FrameSample {
  return {
    engineFrame,
    scriptTimeS,
    rafTimestampMs: 1000 + scriptTimeS * 1000,
    callbackStartMs: 1000.1 + scriptTimeS * 1000,
    callbackMs: 3,
    passesSubmitted: 4,
    patchesHard: 100,
    patchesCalibrated: 40,
    streaming: false,
    ...overrides,
  };
}

function times(frame: number, passes: ReadonlyArray<readonly [string, number]>): PassTimes {
  return {
    frame,
    timer: "full",
    passes: passes.map(([label, ns]) => ({ label, ns, bracketed: false })),
  };
}

describe("the spike's metrics", () => {
  it("records each frame's script time, rAF interval and callback time in order", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(7, 0));
    metrics.frame(sample(8, 0.5, { callbackMs: 5, callbackStartMs: 1502.3 }));
    metrics.frame(sample(9, 1.25));
    const { frames } = metrics.report(EXTRA);
    expect(frames.scriptTimesS).toEqual([0, 0.5, 1.25]);
    expect(frames.rafIntervalsMs).toEqual([0, 500, 750]);
    expect(frames.ourCodeMs).toEqual([3, 5, 3]);
    expect(frames.callbackStartsMs).toEqual([1000.1, 1502.3, 2250.1]);
  });

  it("aligns pass times with their frame by the engine's frame number, ns to ms", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(7, 0));
    metrics.frame(sample(8, 0.5));
    // R01 delivers a frame's times after later frames have been recorded.
    metrics.passTimes(times(8, [["terrain", 4_000_000]]));
    metrics.passTimes(
      times(7, [
        ["terrain", 3_500_000],
        ["atmosphere.sky", 500_000],
      ]),
    );
    metrics.passTimes(times(99, [["terrain", 1]]));
    const { frames, timer } = metrics.report(EXTRA);
    expect(timer).toBe("full");
    expect(frames.passes).toEqual([
      { label: "terrain", row: "terrain", gpuMs: [3.5, 4] },
      { label: "atmosphere.sky", row: "atmosphere", gpuMs: [0.5, null] },
    ]);
  });

  it("gives a frame every engine frame after the last one's, summing a label timed twice", () => {
    const metrics = new SpikeMetrics({ ...OPTIONS, firstEngineFrame: 2 });
    // The first frame resolved engine frames 3 to 5, the second 6 and 7.
    metrics.frame(sample(5, 0));
    metrics.frame(sample(7, 0.5));
    metrics.passTimes(times(2, [["terrain", 9_000_000]]));
    metrics.passTimes(times(3, [["terrain", 1_000_000]]));
    metrics.passTimes(times(4, [["view:wireframe", 250_000]]));
    metrics.passTimes(times(5, [["view:wireframe", 500_000]]));
    metrics.passTimes(times(7, [["terrain", 2_000_000]]));
    expect(metrics.report(EXTRA).frames.passes).toEqual([
      { label: "terrain", row: "terrain", gpuMs: [1, 2] },
      { label: "view:wireframe", row: "other", gpuMs: [0.75, null] },
    ]);
  });

  it("leaves a pass's later frames null until their times arrive", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(1, 0));
    metrics.passTimes(times(1, [["tone", 250_000]]));
    metrics.frame(sample(2, 0.1));
    expect(metrics.report(EXTRA).frames.passes).toEqual([
      { label: "tone", row: "other", gpuMs: [0.25, null] },
    ]);
  });

  it("counts each frame's resolves whose times never arrived, 0 for a complete frame", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    // Five resolves a frame: engine frames 1 to 5, 6 to 10 and 11 to 15.
    metrics.frame(sample(5, 0));
    metrics.frame(sample(10, 0.5));
    metrics.frame(sample(15, 1));
    for (const n of [1, 2, 3, 4, 5, 6, 7, 9, 10]) {
      metrics.passTimes(times(n, [["terrain", 1_000_000]]));
    }
    const { frames } = metrics.report(EXTRA);
    // The second frame lost one of its five (partial), the third all five (dropped).
    expect(frames.missingResolves).toEqual([0, 1, 5]);
    expect(frames.passes).toEqual([{ label: "terrain", row: "terrain", gpuMs: [5, 4, null] }]);
  });

  it("counts each frame's missing resolves still being read at the report", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(5, 0));
    metrics.frame(sample(10, 0.5));
    metrics.frame(sample(15, 1));
    for (const n of [1, 2, 3, 4, 5, 6, 7, 9, 10]) {
      metrics.passTimes(times(n, [["terrain", 1_000_000]]));
    }
    // 8 and 13 to 15 are still being read; 9 is listed but reported, and 16 follows every frame.
    const { frames } = metrics.report({ ...EXTRA, inFlightResolves: [8, 9, 13, 14, 15, 16] });
    expect(frames.missingResolves).toEqual([0, 1, 5]);
    expect(frames.inFlightResolves).toEqual([0, 1, 3]);
  });

  it("counts no missing resolve for a frame that numbered none", () => {
    const metrics = new SpikeMetrics({ ...OPTIONS, firstEngineFrame: 2 });
    metrics.frame(sample(3, 0));
    metrics.frame(sample(3, 0.5));
    metrics.frame(sample(4, 1));
    // Resolve 2 is the warm-up's, and resolve 9 follows every frame: neither is a frame's.
    for (const n of [2, 3, 9]) {
      metrics.passTimes(times(n, [["terrain", 1_000_000]]));
    }
    expect(metrics.report(EXTRA).frames.missingResolves).toEqual([0, 0, 1]);
  });

  it("counts passes beyond the timer's 64 a frame as untimed", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(1, 0, { passesSubmitted: TIMED_PASSES_A_FRAME + 3 }));
    metrics.frame(sample(2, 0.1, { passesSubmitted: TIMED_PASSES_A_FRAME }));
    expect(metrics.report(EXTRA).untimedPasses).toBe(3);
  });

  it("counts no untimed pass of a frame that counts none, and records the frame", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(1, 0, { passesSubmitted: null }));
    metrics.frame(sample(2, 0.1, { passesSubmitted: TIMED_PASSES_A_FRAME + 2 }));
    const report = metrics.report(EXTRA);
    expect(report.untimedPasses).toBe(2);
    expect(report.frames.scriptTimesS).toEqual([0, 0.1]);
  });

  it("keeps each segment's streaming figures to its own frames and events", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(1, 0.0, { patchesHard: 100, patchesCalibrated: 40 }));
    metrics.frame(sample(2, 1.0, { patchesHard: 200, patchesCalibrated: 60, streaming: true }));
    metrics.frame(sample(3, 2.5, { patchesHard: 900, patchesCalibrated: 300, streaming: true }));
    metrics.patches("requested", 8, 0.5);
    metrics.patches("baked", 6, 1.5);
    metrics.patches("resident", 6, 1.9);
    metrics.patches("requested", 40, 3.0);
    metrics.patches("baked", 2, 9.0);
    const [coast, arc] = metrics.report(EXTRA).streaming;
    expect(coast).toEqual({
      segment: "orbit coast",
      requestedPerS: 4,
      bakedPerS: 3,
      residentPerS: 3,
      predictedHardPerS: 4,
      predictedCalibratedPerS: 2,
      patchesHard: 150,
      patchesCalibrated: 50,
      // Streaming from 0 s to 1 s, then from 1 s to the boundary at 2 s.
      streamingS: 2,
    });
    expect(arc).toMatchObject({
      segment: "descent arc",
      requestedPerS: 20,
      bakedPerS: 0,
      patchesHard: 900,
      patchesCalibrated: 300,
      predictedHardPerS: 16,
      predictedCalibratedPerS: 8,
      streamingS: 0.5,
    });
  });

  it("shares a streaming interval that spans a boundary between the two segments", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(1, 1.5));
    metrics.frame(sample(2, 2.25, { streaming: true }));
    const [coast, arc] = metrics.report(EXTRA).streaming;
    expect(coast?.streamingS).toBeCloseTo(0.5, 12);
    expect(arc?.streamingS).toBeCloseTo(0.25, 12);
  });

  it("reports the worst pass timer it saw", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.frame(sample(1, 0));
    metrics.passTimes(times(1, []));
    metrics.passTimes({ ...times(1, []), timer: "quantized" });
    metrics.passTimes(times(1, []));
    expect(metrics.report(EXTRA).timer).toBe("quantized");
  });

  it("tallies upload bytes from the engine's uploaded events alone", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    metrics.allocation({ kind: "uploaded", name: "patch slots", bytes: 4096 });
    metrics.allocation({ kind: "created", name: "patch slots", bytes: 1 << 20, category: "other" });
    metrics.allocation({ kind: "uploaded", name: "normals", bytes: 1000 });
    expect(metrics.report(EXTRA).uploadBytes).toBe(5096);
  });

  it("finds the segment of a script time, and none past the script's end", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    expect(metrics.segmentAt(0)).toBe("orbit coast");
    expect(metrics.segmentAt(2)).toBe("descent arc");
    expect(metrics.segmentAt(4)).toBeUndefined();
  });

  it("carries the run's description into the report", () => {
    const report = new SpikeMetrics(OPTIONS).report(EXTRA);
    expect(report).toMatchObject({
      warmupS: 10,
      segments: OPTIONS.segments,
      levels: OPTIONS.levels,
      timer: "absent",
      adapterPeakBytes: 1024,
      canvas: { widthPx: 1280, heightPx: 720 },
      latePipelines: [],
    });
  });

  it("gives a generated world's geometry by segment, each count's median and largest, null where no frame had it (plan R14, R14.T1.f)", () => {
    const metrics = new SpikeMetrics(OPTIONS);
    const settings = SETTINGS.low.geometry;
    const counts = (
      interior: number,
      drawnTriangles: number,
      overlayTriangles: number | null,
    ): SpikeGeometrySample => ({
      settings,
      counts: {
        patches: { interior, edge: 2 },
        drawnTriangles,
        litClusterTriangles: drawnTriangles - 1,
        casterTriangles: null,
        overlayTriangles,
      },
    });
    metrics.frame(sample(1, 0, { geometry: counts(10, 100, null) }));
    metrics.frame(sample(2, 0.5, { geometry: counts(12, 300, 90) }));
    metrics.frame(sample(3, 1, { geometry: counts(11, 200, 80) }));
    metrics.frame(sample(4, 2.5, { geometry: { settings, counts: null } }));
    const geometry = metrics.report(EXTRA).geometry;
    expect(geometry).toEqual({
      settings,
      segments: [
        {
          segment: "orbit coast",
          interiorPatches: { frames: 3, p50: 11, max: 12 },
          edgePatches: { frames: 3, p50: 2, max: 2 },
          drawnTriangles: { frames: 3, p50: 200, max: 300 },
          litClusterTriangles: { frames: 3, p50: 199, max: 299 },
          casterTriangles: null,
          overlayTriangles: { frames: 2, p50: 85, max: 90 },
        },
        {
          segment: "descent arc",
          interiorPatches: null,
          edgePatches: null,
          drawnTriangles: null,
          litClusterTriangles: null,
          casterTriangles: null,
          overlayTriangles: null,
        },
      ],
    });
    // The test planet's frames carry none, and its report no geometry.
    const planet = new SpikeMetrics(OPTIONS);
    planet.frame(sample(1, 0));
    expect(planet.report(EXTRA).geometry).toBeUndefined();
  });
});
