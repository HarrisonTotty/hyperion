import { describe, expect, it } from "vitest";

import type { PerfRepetitionResult } from "../../preload/api";
import { countedIntervals, frameFiguresOf, periodOf } from "./assemble";

/** A repetition's result of `frames` frames a `frameMs` apart, from page time 1,000 ms. */
function result(frames: number, frameMs: number): PerfRepetitionResult {
  return {
    spanS: 10,
    timeOriginMs: 0,
    frames: {
      intervalsMs: Array.from({ length: frames }, () => frameMs),
      callbackStartsMs: Array.from({ length: frames }, (_, i) => 1000 + i * frameMs),
      callbackMs: Array.from({ length: frames }, () => 2),
      scriptTimesS: Array.from({ length: frames }, (_, i) => (i * frameMs) / 1000),
    },
    segments: [],
    passes: [],
    rows: [],
    timedPassesPerFrameMs: null,
    timer: "full",
    memory: { itemPeakBytes: {}, tallyPeakBytes: null, workerFieldBytes: [], workerWasmBytes: [] },
    releases: [],
    streaming: [],
    latePipelines: 0,
    heightPointUs: null,
    internalScale: [],
    arrival: null,
  };
}

describe("the record's frames", () => {
  it("take T from the display's vsync and the pacing, or the primary's rate unknown", () => {
    expect(periodOf(16.68, "every-second-vsync", 30)).toBeCloseTo(33.36, 10);
    expect(periodOf(16.68, "every-vsync", 60)).toBe(16.68);
    expect(periodOf(null, "every-second-vsync", 30)).toBeCloseTo(33.333, 3);
    expect(periodOf(null, "every-vsync", null)).toBeNull();
  });

  it("count missed frames above 1.5 T and hitches above 3 T, R05's convention", () => {
    const figures = frameFiguresOf("raf", [16, 17, 26, 51, 16], 16.68);
    expect(figures.count).toBe(5);
    expect(figures.missedFraction).toBeCloseTo(2 / 5, 10);
    expect(figures.hitches).toBe(1);
    expect(figures.meanMs).toBeCloseTo(25.2, 10);
  });

  it("are the renderer's intervals less the first, without a trace", () => {
    const counted = countedIntervals(result(5, 20), null);
    expect(counted.source).toBe("raf");
    expect(counted.intervalsMs).toEqual([20, 20, 20, 20]);
  });

  it("are the trace's presentations within each window, never across a cycle's gap", () => {
    const counted = countedIntervals(result(10, 16), [
      [1000, 1016, 1033],
      [1100, 1116],
    ]);
    expect(counted.source).toBe("presentation");
    expect(counted.intervalsMs).toEqual([16, 17, 16]);
    // Each presentation is counted at the script time of the last frame begun by then.
    expect(counted.scriptTimesS).toEqual([0.016, 0.032, 0.112]);
  });

  it("leave out presentations from outside the renderer's frames", () => {
    const counted = countedIntervals(result(3, 16), [[400, 500, 1000, 1016, 9000]]);
    expect(counted.intervalsMs).toEqual([16]);
  });
});
