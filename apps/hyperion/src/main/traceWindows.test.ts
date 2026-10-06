import { describe, expect, it } from "vitest";

import {
  callbackStartsOf,
  PROFILED,
  recordingOf,
  UNPROFILED,
  windowFile,
  windowTrace,
} from "./fixtures/traces";
import { measured, missing } from "./measured";
import type { TraceFigures } from "./reduceTrace";
import {
  checkFrameSpans,
  EMPTY_TRACE_REASON,
  frameSpanFailure,
  LOST_DATA_REASON,
  mergeTraceWindows,
  NO_CLOCK_OFFSET_REASON,
  type PooledTraceFigures,
  PROFILER_OFF_REASON,
  type TraceWindowsReport,
  windowFileFailure,
} from "./traceWindows";

/**
 * Two windows of a 20 s script that starts at 1,000 ms: the first from −0.1 s to its stop request
 * at 10.0 s, the second from its start at 10.5 s to 20.0 s. The boundary's exclusion is
 * [10.0, 11.5) s. A frame every 0.25 s, each 250 ms after the one before, but the one at 10.5 s,
 * after the gap, 750 ms, and each with its callback starting 0.1 ms after its rAF time and 3 ms of
 * our code.
 */
function report(): TraceWindowsReport {
  const scriptTimesS = Array.from({ length: 81 }, (_, i) => i / 4);
  return {
    scriptStartMs: 1000,
    traceWindows: [
      { startedMs: 900, stopRequestedMs: 11_000, failure: null },
      { startedMs: 11_500, stopRequestedMs: 21_000, failure: null },
    ],
    traceGuardS: 1,
    warmupS: 1,
    segments: [
      { name: "a", startS: 0, endS: 10 },
      { name: "b", startS: 10, endS: 20 },
    ],
    frames: {
      scriptTimesS,
      rafIntervalsMs: scriptTimesS.map((t) => (t === 0 ? 0 : t === 10.5 ? 750 : 250)),
      ourCodeMs: scriptTimesS.map(() => 3),
      callbackStartsMs: callbackStartsOf(1000, scriptTimesS),
    },
  };
}

/** Page ms at script time `s`. */
function at(s: number): number {
  return 1000 + 1000 * s;
}

/** The two windows' traces, each on its own clock, with a frame span for each of their frames. */
function twoWindows() {
  return [
    windowTrace({
      offsetUs: 7e9,
      fromMs: at(-0.1),
      toMs: at(10),
      frames: report(),
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
      frames: report(),
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
    expect(run?.guardS).toBe(1);
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

  it("counts the frames each window's span check checked, for the run's log", () => {
    const merged = mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), report());
    // Callbacks at 1.5, 1.75, …, 10.25 s in the first window's interior [1.4, 10.5] s, and at
    // 12, 12.25, …, 20.25 s in the second's [12, 20.5] s.
    expect(merged.checkedFrames).toEqual([36, 34]);
  });

  it("fails a window that lost data, giving it no count", () => {
    const [first, second] = twoWindows().map(windowFile);
    if (first === undefined || second === undefined) {
      throw new Error("no two windows");
    }
    const merged = mergeTraceWindows(recordingOf([first, { ...second, lostData: true }]), report());
    expect(merged.figures.reason).toBe(`trace window 2 of 2: ${LOST_DATA_REASON}`);
    expect(merged.run.value?.windows[1]?.figures.reason).toBe(LOST_DATA_REASON);
    expect(merged.checkedFrames).toEqual([36, null]);
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
    // An unprofiled run records no `gpu`, so its slices are GPUTask's alone.
    expect(figures.gpuProcess.value?.slices).toEqual([
      { name: "GPUTask", count: 7, totalMs: 11, maxMs: 3 },
    ]);
  });

  it("lists all three GPU-process slices in a profiled run, which records gpu", () => {
    const traces = twoWindows().map((trace) =>
      windowFile({
        ...trace,
        gpuProcess:
          trace.gpuProcess === null
            ? null
            : {
                ...trace.gpuProcess,
                slices: [
                  { name: "WebGPU", count: 1, totalMs: 2, maxMs: 2 },
                  ...trace.gpuProcess.slices,
                  { name: "VulkanQueueSubmitHook", count: 3, totalMs: 0.3, maxMs: 0.1 },
                ],
              },
      }),
    );
    const figures = pooled(mergeTraceWindows(recordingOf(traces, PROFILED), report()).figures);
    expect(figures.gpuProcess.value?.slices).toEqual([
      { name: "WebGPU", count: 2, totalMs: 4, maxMs: 2 },
      { name: "GPUTask", count: 7, totalMs: 11, maxMs: 3 },
      { name: "VulkanQueueSubmitHook", count: 6, totalMs: 0.6, maxMs: 0.1 },
    ]);
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
    const second = windowTrace({
      offsetUs: 9e9,
      fromMs: at(10.5),
      toMs: at(20),
      engine: null,
      frames: report(),
    });
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
      traceWindows: [{ startedMs: 900, stopRequestedMs: 21_100, failure: null }],
    };
    const trace = windowTrace({ offsetUs: 7e9, fromMs: at(-0.1), toMs: at(20.1), frames: one });
    const merged = mergeTraceWindows(recordingOf([windowFile(trace)]), one);
    expect(merged.run.value?.boundaries).toEqual([]);
    expect(merged.exclusions).toEqual([]);
    expect(merged.run.value?.tracedS).toBeCloseTo(19, 9);
  });

  it("has no trace when the renderer's windows and the main process's files disagree", () => {
    const trace = windowTrace({ offsetUs: 7e9, fromMs: at(-0.1), toMs: at(10) });
    const merged = mergeTraceWindows(recordingOf([windowFile(trace)]), report());
    const reason = "the renderer reported 2 trace windows, and the main process wrote 1";
    expect(merged).toEqual({
      run: missing(reason),
      checkedFrames: [],
      exclusions: [],
      figures: missing(reason),
    });
  });

  it("has no trace when the renderer reports no window", () => {
    const reason = "the renderer reported no trace window";
    expect(
      mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), {
        ...report(),
        traceWindows: [],
      }),
    ).toEqual({
      run: missing(reason),
      checkedFrames: [],
      exclusions: [],
      figures: missing(reason),
    });
  });

  it("passes on why there is no trace at all", () => {
    expect(mergeTraceWindows(missing("the trace was not recorded"), report())).toEqual({
      run: missing("the trace was not recorded"),
      checkedFrames: [],
      exclusions: [],
      figures: missing("the trace was not recorded"),
    });
  });
});

describe("a trace the renderer ended early", () => {
  /** The second window failed by the renderer: its cycle at 10 s was refused. */
  function ended(): TraceWindowsReport {
    const base = report();
    const [first] = base.traceWindows;
    if (first === undefined) {
      throw new Error("no first window");
    }
    return {
      ...base,
      traceWindows: [
        first,
        { startedMs: 11_500, stopRequestedMs: 21_000, failure: "the trace's cycle at 10 s failed" },
      ],
    };
  }

  it("fails every trace figure with the reason, its last window needing no file", () => {
    const [first] = twoWindows();
    if (first === undefined) {
      throw new Error("no first window");
    }
    const merged = mergeTraceWindows(recordingOf([windowFile(first)]), ended());
    const reason = "trace window 2 of 2: the trace's cycle at 10 s failed";
    expect(merged.figures).toEqual(missing(reason));
    expect(merged.run.value?.windows.map(({ figures }) => figures.reason)).toEqual([
      null,
      "the trace's cycle at 10 s failed",
    ]);
    // The boundary's frames are still left out; only the reduced window counts as traced.
    expect(merged.exclusions).toEqual([{ fromS: 10, toS: 11.5 }]);
    expect(merged.run.value?.tracedS).toBeCloseTo(9, 9);
  });

  it("ignores a file the main process still wrote for the failed window", () => {
    const merged = mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), ended());
    expect(merged.figures.reason).toBe("trace window 2 of 2: the trace's cycle at 10 s failed");
  });

  it("has no trace when a window the renderer did not fail has no file", () => {
    const [first] = twoWindows();
    if (first === undefined) {
      throw new Error("no first window");
    }
    const merged = mergeTraceWindows(recordingOf([windowFile(first)]), report());
    expect(merged.run.reason).toBe(
      "the renderer reported 2 trace windows, and the main process wrote 1",
    );
  });
});

describe("the boundaries' guard", () => {
  it("is the report's", () => {
    const merged = mergeTraceWindows(recordingOf(twoWindows().map(windowFile)), {
      ...report(),
      traceGuardS: 2,
    });
    expect(merged.run.value?.guardS).toBe(2);
    expect(merged.exclusions).toEqual([{ fromS: 10, toS: 12.5 }]);
  });
});

/** One frame span: its start in the trace's clock, its duration, and its `args.startTime`. */
interface Span {
  readonly startUs: number;
  readonly durationMs: number;
  readonly startMs: number | null;
}

/** `trace` with its main thread's frame spans edited. */
function withSpans(trace: TraceFigures, edit: (spans: Span[]) => Span[]): TraceFigures {
  const main = trace.mainThread;
  if (main === null) {
    throw new Error("the window has no main thread");
  }
  const { startsUs, durationsMs, startTimesMs } = main.frameSpans;
  const edited = edit(
    startsUs.map((startUs, i) => ({
      startUs,
      durationMs: durationsMs[i] ?? 0,
      startMs: startTimesMs[i] ?? null,
    })),
  ).toSorted((a, b) => a.startUs - b.startUs);
  return {
    ...trace,
    mainThread: {
      ...main,
      frameSpans: {
        startsUs: edited.map(({ startUs }) => startUs),
        durationsMs: edited.map(({ durationMs }) => durationMs),
        startTimesMs: edited.map(({ startMs }) => startMs),
      },
    },
  };
}

/** The first window's span of the frame at 5 s: the 21st, its frames being 0, 0.25, …, 10 s. */
const AT_5_S = 20;

/** A span on the first window's clock of a callback starting at page ms `startMs`. */
function spanAt(startMs: number, durationMs = 3): Span {
  return { startUs: 7e9 + 1000 * startMs, durationMs, startMs };
}

/**
 * The first window's frames checked: those whose callbacks start 0.5 s inside [−0.1, 10.0] s, at
 * 0.5 s to 9.25 s; the one at 9.5 s starts 0.1 ms too late.
 */
const CHECKED = 36;

/** The reason a window fails when its spans disagree with `m` of its `n` frames' and `strays`. */
function disagreement(m: number, strays = 0, n = CHECKED): string {
  const stray =
    strays === 0 ? "" : `; ${strays} ${strays === 1 ? "span matches" : "spans match"} no frame`;
  return `the trace's frame spans disagree with the renderer's (${m} of ${n} frames${stray})`;
}

/** The second window, to `at(21)`, with its spans edited, and a report whose last window ends there. */
function endingAt21(edit: (spans: Span[]) => Span[]): {
  readonly windows: ReadonlyArray<TraceFigures>;
  readonly report: TraceWindowsReport;
} {
  const [first] = twoWindows();
  if (first === undefined) {
    throw new Error("no first window");
  }
  const base = report();
  const second = withSpans(
    windowTrace({ offsetUs: 9e9, fromMs: at(10.5), toMs: at(21), frames: base }),
    edit,
  );
  return {
    windows: [first, second],
    report: {
      ...base,
      traceWindows: [
        { startedMs: 900, stopRequestedMs: 11_000, failure: null },
        { startedMs: 11_500, stopRequestedMs: at(21), failure: null },
      ],
    },
  };
}

describe("the frame-span check, by identity", () => {
  /** The first window with its spans edited, merged with the second as it is. */
  function mergedWith(
    edit: (spans: Span[]) => Span[],
    base: TraceWindowsReport = report(),
  ): ReturnType<typeof mergeTraceWindows> {
    const [, second] = twoWindows();
    const first = windowTrace({ offsetUs: 7e9, fromMs: at(-0.1), toMs: at(10), frames: base });
    if (second === undefined) {
      throw new Error("no two windows");
    }
    return mergeTraceWindows(recordingOf([withSpans(first, edit), second].map(windowFile)), base);
  }

  it("passes windows with one span a frame, of its frame's ourCodeMs", () => {
    const merged = mergedWith((spans) => spans);
    expect(merged.figures.reason).toBeNull();
    expect(merged.run.value?.windows.map(({ figures }) => figures.reason)).toEqual([null, null]);
  });

  it("passes a span 0.24 ms off its frame's ourCodeMs", () => {
    const merged = mergedWith((spans) =>
      spans.map((span, k) => (k === AT_5_S ? { ...span, durationMs: 3.24 } : span)),
    );
    expect(merged.figures.reason).toBeNull();
  });

  it.each<readonly [string, (spans: Span[]) => Span[], string]>([
    ["a missing span", (spans) => spans.filter((_, k) => k !== AT_5_S), disagreement(1)],
    [
      "a duplicated span",
      (spans) => [...spans, ...spans.filter((_, k) => k === AT_5_S)],
      disagreement(1),
    ],
    [
      "a span whose duration is off by 0.3 ms",
      (spans) =>
        spans.map((span, k) =>
          k === AT_5_S ? { ...span, durationMs: span.durationMs + 0.3 } : span,
        ),
      disagreement(1),
    ],
    [
      "an unmatched span inside the interior",
      (spans) => [...spans, spanAt(at(5) + 2)],
      disagreement(0, 1),
    ],
    [
      "a span whose startTime is 2·10⁻⁶ ms off its frame's callback start",
      (spans) =>
        spans.map((span, k) =>
          k === AT_5_S && span.startMs !== null ? { ...span, startMs: span.startMs + 2e-6 } : span,
        ),
      disagreement(1, 1),
    ],
    [
      "a span without a startTime",
      (spans) => [...spans, { startUs: 7e9 + 1000 * at(5.1), durationMs: 3, startMs: null }],
      disagreement(0, 1),
    ],
  ])("fails the window for %s, and so every trace figure", (_case, edit, reason) => {
    const merged = mergedWith(edit);
    expect(merged.figures).toEqual(missing(`trace window 1 of 2: ${reason}`));
    expect(merged.run.value?.windows.map(({ figures }) => figures.reason)).toEqual([reason, null]);
  });

  it("passes a callback that began after the next frame's rAF time", () => {
    // The frame at 5 s waits until 0.05 ms after the next frame's rAF time, 5.25 s, and runs its
    // 3 ms; the next frame's callback starts after it.
    const base = report();
    const late = base.frames.callbackStartsMs.map((ms, i) =>
      i === AT_5_S ? at(5.25) + 0.05 : i === AT_5_S + 1 ? at(5.25) + 3.2 : ms,
    );
    const merged = mergedWith((spans) => spans, {
      ...base,
      frames: { ...base.frames, callbackStartsMs: late },
    });
    expect(merged.figures.reason).toBeNull();
  });

  it("checks only the frames whose callbacks start at least 0.5 s inside their window", () => {
    // Without the spans of 0.25 s and 9.5 s, just outside the checked interior, it passes; without
    // the one of 0.5 s, inside it, it fails.
    expect(
      mergedWith((spans) => spans.filter((_, k) => k !== 1 && k !== 38)).figures.reason,
    ).toBeNull();
    expect(mergedWith((spans) => spans.filter((_, k) => k !== 2)).figures.reason).toBe(
      `trace window 1 of 2: ${disagreement(1)}`,
    );
  });

  it("checks the series' last frame", () => {
    // The second window stops 1 s after the last frame, at 20 s, so that frame is 0.5 s inside it.
    const whole = endingAt21((spans) => spans);
    expect(
      mergeTraceWindows(recordingOf(whole.windows.map(windowFile)), whole.report).figures.reason,
    ).toBeNull();
    const without = endingAt21((spans) => spans.slice(0, -1));
    expect(
      mergeTraceWindows(recordingOf(without.windows.map(windowFile)), without.report).figures
        .reason,
    ).toMatch(/^trace window 2 of 2: the trace's frame spans disagree with the renderer's \(1 of /);
  });

  it("ignores the spans that start after the series' last callback, which the page draws on", () => {
    const after = endingAt21((spans) => [
      ...spans,
      ...[20.25, 20.5].map((t) => ({
        startUs: 9e9 + 1000 * (at(t) + 0.1),
        durationMs: 3,
        startMs: at(t) + 0.1,
      })),
    ]);
    expect(
      mergeTraceWindows(recordingOf(after.windows.map(windowFile)), after.report).figures.reason,
    ).toBeNull();
  });

  it("counts the frames it checked, those that disagree and the spans that match none", () => {
    const [first] = twoWindows();
    const time = report().traceWindows[0];
    if (first === undefined || time === undefined) {
      throw new Error("no first window");
    }
    expect(checkFrameSpans(first, time, report())).toEqual({
      checked: 36,
      disagreeing: 0,
      strays: 0,
    });
    const stray = withSpans(first, (spans) => [
      ...spans.filter((_, k) => k !== AT_5_S),
      spanAt(at(5) + 7),
    ]);
    expect(checkFrameSpans(stray, time, report())).toEqual({
      checked: 36,
      disagreeing: 1,
      strays: 1,
    });
  });

  it("is checked against a window's own times", () => {
    const [first] = twoWindows();
    const time = report().traceWindows[0];
    if (first === undefined || time === undefined) {
      throw new Error("no first window");
    }
    const none = withSpans(first, () => []);
    expect(frameSpanFailure(none, time, report())).toBe(disagreement(CHECKED));
    // A window under a second long holds no frame 0.5 s inside it.
    const short = { ...time, startedMs: at(9.6), stopRequestedMs: at(10) };
    expect(frameSpanFailure(none, short, report())).toBeNull();
  });
});

describe("a window's file alone", () => {
  it("fails without a trace, an event, a clock offset or a size, or with a full buffer or lost data", () => {
    const trace = windowTrace({ offsetUs: 7e9, fromMs: at(0), toMs: at(3) });
    const good = windowFile(trace);
    expect(windowFileFailure(good)).toBeNull();
    expect(windowFileFailure({ ...good, trace: missing("its file could not be read: x") })).toBe(
      "its file could not be read: x",
    );
    expect(windowFileFailure(windowFile({ ...trace, span: null }))).toBe(EMPTY_TRACE_REASON);
    expect(windowFileFailure(windowFile({ ...trace, clockOffsetUs: null }))).toBe(
      NO_CLOCK_OFFSET_REASON,
    );
    expect(windowFileFailure({ ...good, bytes: null })).toBe("its file's size could not be read");
    expect(windowFileFailure({ ...good, bufferPercent: 99 })).toBe(
      "it filled its buffer: 99 % of it was used",
    );
    expect(windowFileFailure({ ...good, lostData: true })).toBe(LOST_DATA_REASON);
  });
});
