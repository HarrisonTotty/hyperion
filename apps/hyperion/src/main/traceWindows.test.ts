import { describe, expect, it } from "vitest";

import { PROFILED, recordingOf, UNPROFILED, windowFile, windowTrace } from "./fixtures/traces";
import { measured, missing } from "./measured";
import {
  CPU_PROFILER_CATEGORY,
  mergeTraceWindows,
  type PooledTraceFigures,
  PROFILER_OFF_REASON,
  TRACE_BOUNDARY_GUARD_S,
  traceSettingsOf,
  type TraceWindowsReport,
} from "./traceWindows";

/**
 * Two windows of a 20 s script that starts at 1,000 ms: the first from −0.1 s to its stop request
 * at 10.0 s, the second from its start at 10.5 s to 20.0 s. The boundary's exclusion is
 * [10.0, 11.5) s. A frame every 0.25 s, each 250 ms after the one before, but the one at 10.5 s,
 * after the gap, 750 ms.
 */
function report(): TraceWindowsReport {
  const scriptTimesS = Array.from({ length: 81 }, (_, i) => i / 4);
  return {
    scriptStartMs: 1000,
    traceWindows: [
      { startedMs: 900, stopRequestedMs: 11_000 },
      { startedMs: 11_500, stopRequestedMs: 21_000 },
    ],
    warmupS: 1,
    segments: [
      { name: "a", startS: 0, endS: 10 },
      { name: "b", startS: 10, endS: 20 },
    ],
    frames: {
      scriptTimesS,
      rafIntervalsMs: scriptTimesS.map((t) => (t === 0 ? 0 : t === 10.5 ? 750 : 250)),
    },
  };
}

/** Page ms at script time `s`. */
function at(s: number): number {
  return 1000 + 1000 * s;
}

/** The two windows' traces, each on its own clock. */
function twoWindows() {
  return [
    windowTrace({
      offsetUs: 7e9,
      fromMs: at(-0.1),
      toMs: at(10),
      // 0.5 s is in the warm-up, so the interval from it is not kept.
      presentedMs: [0.5, 2, 3, 4.5].map(at),
      droppedMs: [0.2, 5].map(at),
      busyMs: 100,
      ourCodeMs: 50,
      engine: { selfMs: 10, sampledMs: 40 },
      gc: { count: 3, totalMs: 6, maxMs: 3 },
      gpuBusyMs: 20,
      gpuSlice: { count: 2, totalMs: 4, maxMs: 3 },
    }),
    windowTrace({
      offsetUs: 9e9,
      fromMs: at(10.5),
      toMs: at(20),
      // 11 s is in the boundary's exclusion, so the interval from it is not kept.
      presentedMs: [11, 12, 12.25, 14].map(at),
      droppedMs: [10.8, 15].map(at),
      busyMs: 200,
      ourCodeMs: 70,
      engine: { selfMs: 5, sampledMs: 60 },
      gc: { count: 2, totalMs: 10, maxMs: 8 },
      gpuBusyMs: 30,
      gpuSlice: { count: 5, totalMs: 7, maxMs: 2.5 },
    }),
  ];
}

function pooled(figures: ReturnType<typeof mergeTraceWindows>["figures"]): PooledTraceFigures {
  if (figures.value === null) {
    throw new Error(`no pooled figures: ${figures.reason}`);
  }
  return figures.value;
}

describe("the trace's windows, merged", () => {
  it("place each window in script time, with one boundary and its exclusion between them", () => {
    const merged = mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), report());
    const run = merged.run.value;
    expect(run?.windows.map(({ index, fromS, toS }) => [index, fromS, toS])).toEqual([
      [0, -0.1, 10],
      [1, 10.5, 20],
    ]);
    expect(run?.guardS).toBe(TRACE_BOUNDARY_GUARD_S);
    // Frames at 10, 10.25, …, 11.25 s; the stall is the frame after the gap.
    expect(run?.boundaries).toEqual([
      {
        afterWindow: 0,
        stopRequestedS: 10,
        resumedS: 10.5,
        excludedToS: 11.5,
        excludedFrames: 6,
        maxRafIntervalMs: 750,
      },
    ]);
    expect(merged.exclusions).toEqual([{ fromS: 10, toS: 11.5 }]);
  });

  it("counts the traced time after the warm-up and outside the exclusion", () => {
    const merged = mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), report());
    // [1, 10) of the first window and [11.5, 20) of the second.
    expect(merged.run.value?.tracedS).toBeCloseTo(17.5, 9);
  });

  it("records each window's span, file size and buffer use", () => {
    const merged = mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), report());
    expect(merged.run.value?.windows.map(({ figures }) => figures)).toEqual([
      measured({ spanMs: 10_100, bytes: 1_000_000, bufferPercent: 10 }),
      measured({ spanMs: 9500, bytes: 1_000_000, bufferPercent: 10 }),
    ]);
  });

  it("form presentation intervals within a window, never across its gap", () => {
    const figures = pooled(
      mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), report()).figures,
    );
    // The first window's 2 → 3 and 3 → 4.5 s, the second's 12 → 12.25 and 12.25 → 14 s; not
    // 4.5 → 12 s across the gap, nor those from the warm-up or the exclusion.
    expect(figures.presentationMs.map((ms) => Math.round(ms))).toEqual([1000, 1500, 250, 1750]);
  });

  it("assigns a presentation to the segment of its end's script time", () => {
    const figures = pooled(
      mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), report()).figures,
    );
    const rounded = (name: string): number[] =>
      (figures.presentationBySegmentMs.get(name) ?? []).map((ms) => Math.round(ms));
    expect(rounded("a")).toEqual([1000, 1500]);
    expect(rounded("b")).toEqual([250, 1750]);
  });

  it("counts the frames dropped after the warm-up and outside the exclusion", () => {
    const figures = pooled(
      mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), report()).figures,
    );
    // 0.2 s is in the warm-up and 10.8 s in the exclusion.
    expect(figures.dropped).toBe(2);
  });

  it("sums busy times, GC pauses and the GPU process's slices, and takes their largest", () => {
    const figures = pooled(
      mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), report()).figures,
    );
    expect(figures.split).toEqual(
      measured({
        pid: 1,
        tid: 2,
        wallMs: 10_100 + 9500,
        busyMs: 300,
        ourCodeMs: 120,
        idleMs: 10_000 + 9300,
      }),
    );
    expect(figures.gc).toEqual([
      { process: "Renderer", thread: "CrRendererMain", count: 5, totalMs: 16, maxMs: 8 },
    ]);
    expect(figures.gpuProcess.value?.busyMs).toBe(50);
    expect(figures.gpuProcess.value?.slices.find(({ name }) => name === "WebGPU")).toEqual({
      name: "WebGPU",
      count: 7,
      totalMs: 11,
      maxMs: 3,
    });
  });

  it("sums the engine's sampled time in a profiled run, and leaves it out of an unprofiled one", () => {
    const traces = twoWindows().map(windowFile);
    expect(
      pooled(mergeTraceWindows(recordingOf(traces, PROFILED), report()).figures).engine,
    ).toEqual(measured({ selfMs: 15, sampledMs: 100 }));
    expect(
      pooled(mergeTraceWindows(recordingOf(traces, UNPROFILED), report()).figures).engine,
    ).toEqual(missing(PROFILER_OFF_REASON));
  });

  it("says which window of a profiled run has no CPU profile", () => {
    const [first] = twoWindows();
    const second = windowTrace({ offsetUs: 9e9, fromMs: at(10.5), toMs: at(20), engine: null });
    if (first === undefined) {
      throw new Error("no first window");
    }
    const engine = pooled(
      mergeTraceWindows(recordingOf([first, second].map(windowFile), PROFILED), report()).figures,
    ).engine;
    expect(engine).toEqual(
      missing("trace window 2 of 2: the trace has no CPU profile of the renderer's main thread"),
    );
  });

  it("is one window and no boundary before the trace is cycled", () => {
    const one: TraceWindowsReport = {
      ...report(),
      traceWindows: [{ startedMs: 900, stopRequestedMs: 21_100 }],
    };
    const trace = windowTrace({ offsetUs: 7e9, fromMs: at(-0.1), toMs: at(20.1) });
    const merged = mergeTraceWindows(recordingOf([windowFile(trace)]), one);
    expect(merged.run.value?.boundaries).toEqual([]);
    expect(merged.exclusions).toEqual([]);
    expect(merged.run.value?.tracedS).toBeCloseTo(19, 9);
  });

  it("has no trace when the renderer's windows and the main process's files disagree", () => {
    const trace = windowTrace({ offsetUs: 7e9, fromMs: at(-0.1), toMs: at(10) });
    const merged = mergeTraceWindows(recordingOf([windowFile(trace)]), report());
    const reason = "the renderer reported 2 trace windows, and the main process wrote 1";
    expect(merged).toEqual({ run: missing(reason), exclusions: [], figures: missing(reason) });
  });

  it("has no trace when the renderer reports no window", () => {
    const reason = "the renderer reported no trace window";
    expect(
      mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), {
        ...report(),
        traceWindows: [],
      }),
    ).toEqual({ run: missing(reason), exclusions: [], figures: missing(reason) });
  });

  it("passes on why there is no trace at all", () => {
    expect(mergeTraceWindows(missing("the trace was not recorded"), report())).toEqual({
      run: missing("the trace was not recorded"),
      exclusions: [],
      figures: missing("the trace was not recorded"),
    });
  });
});

describe("a run's trace settings", () => {
  it("call a run profiled when the CPU profiler's category is recorded", () => {
    const settings = traceSettingsOf({
      recording_mode: "record-until-full",
      trace_buffer_size_in_kb: 786_432,
      included_categories: ["gpu", CPU_PROFILER_CATEGORY],
    });
    expect(settings).toEqual({
      profiled: true,
      categories: ["gpu", CPU_PROFILER_CATEGORY],
      recordingMode: "record-until-full",
      bufferKb: 786_432,
    });
    expect(traceSettingsOf({ included_categories: ["gpu"] }).profiled).toBe(false);
  });
});
