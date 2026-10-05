/**
 * The descent spike's trace in windows of script time (plan R05, T14.d;
 * decision-r05-trace-windows.md): each window's reduced trace placed in script time, the frames
 * left out at its boundaries, and the windows' figures pooled into the results file's.
 *
 * @remarks
 * Chromium keeps one trace session at a time, and its tracing service crashed reading out a whole
 * descent's trace at the stop, so the trace is stopped and started again at fixed boundaries of
 * script time (T14.e). Each window is written to its own file and reduced alone after the run. A
 * boundary's gap runs from the stop request to the next start resolving, and its excluded interval
 * from the stop request to {@link TRACE_BOUNDARY_GUARD_S} after that: the frames in it are left out
 * of every per-frame figure, as the warm-up's are, and counted. Until T14.e drives the windows, a
 * run is one window with no boundary.
 *
 * The windows' figures pool by the ruling's merge rules:
 *
 * - a presentation interval lies between consecutive presentations of one window, never across a
 *   gap. It is placed in script time by its window's clock offset, kept when both its ends are
 *   after the warm-up and outside every exclusion, and assigned to the segment of its end;
 * - a dropped frame counts where its end is after the warm-up and outside every exclusion;
 * - times and counts are summed and maxima are the largest. The percentiles are taken over the
 *   pooled intervals (`results.ts`), never averaged over windows;
 * - one failed window makes every figure read from the trace missing with "trace window k of n:
 *   <reason>": a file that is missing or not a trace, a trace with no event or no clock offset, a
 *   window that filled its buffer, or one that shows another renderer than the others.
 */

import type { TraceConfig } from "electron";

import type { DescentSpikeReport, SpikeFrameSeries, SpikeTraceWindow } from "../preload/api";
import { type Measured, measured, missing } from "./measured";
import type { GpuProcessFigures, MainThreadFigures, TraceFigures } from "./reduceTrace";

/**
 * How long after a window's start resolves its boundary's frames are still left out, s
 * (`TRACE_BOUNDARY_GUARD_S`): the start's own pause. Provisional until T14.f's runs confirm it.
 */
export const TRACE_BOUNDARY_GUARD_S = 1;

/** V8's CPU profiler's trace category, recorded only in a profiled run (`--trace-profile on`). */
export const CPU_PROFILER_CATEGORY = "disabled-by-default-v8.cpu_profiler";

/** Why an unprofiled run has no engine figure. */
export const PROFILER_OFF_REASON =
  "the CPU profiler is off (--trace-profile off; decision-r05-trace-windows.md)";

/**
 * Why a window whose trace has no timed event measured nothing.
 *
 * @remarks
 * On 2026-10-04 Chromium's tracing service crashed while writing a 1.71 GB trace and left the file
 * without events. Its frame drops and GC pauses are then missing, not zero.
 */
export const EMPTY_TRACE_REASON = "the trace has no timed event";

/** Why a window's trace cannot be placed in script time. */
export const NO_CLOCK_OFFSET_REASON =
  "the trace has no performance.measure span on the renderer's main thread to set its clock by";

/** A window whose span is under this share of the time it recorded filled its buffer. */
export const SHORT_SPAN_FRACTION = 0.95;

/** A window whose buffer was used to this share or more, %, filled it. */
export const FULL_BUFFER_PERCENT = 99;

/** How a run's trace was recorded: one configuration for every window. */
export interface TraceSettings {
  /**
   * Whether {@link CPU_PROFILER_CATEGORY} is recorded. A profiled run is a diagnostic, never
   * judged: its memory includes the profiler's samples.
   */
  readonly profiled: boolean;
  /** The categories recorded. */
  readonly categories: ReadonlyArray<string>;
  /** Chromium's recording mode, `record-until-full`. */
  readonly recordingMode: NonNullable<TraceConfig["recording_mode"]>;
  /** Each window's buffer ceiling, KiB (`trace_buffer_size_in_kb`). */
  readonly bufferKb: number;
}

/** A run's trace settings from the configuration its trace was started with. */
export function traceSettingsOf(config: TraceConfig): TraceSettings {
  const categories = config.included_categories ?? [];
  return {
    profiled: categories.includes(CPU_PROFILER_CATEGORY),
    categories: [...categories],
    // Chromium's default when none is given.
    recordingMode: config.recording_mode ?? "record-until-full",
    bufferKb: config.trace_buffer_size_in_kb ?? 0,
  };
}

/** One window's file as the main process reduced it. */
export interface TraceWindowFile {
  /** The reduced trace, or why the file gave none (missing, unreadable or not a trace). */
  readonly trace: Measured<TraceFigures>;
  /** The file's size, bytes, or `null` when it could not be read. */
  readonly bytes: number | null;
  /**
   * The buffer's use just before the stop (`getTraceBufferUsage`), %, or `null` when Electron
   * reports none.
   */
  readonly bufferPercent: number | null;
}

/** A run's trace: how it was recorded, and its windows' files in order. */
export interface TraceRecording {
  readonly settings: TraceSettings;
  readonly windows: ReadonlyArray<TraceWindowFile>;
}

/** What a window's file held. */
export interface TraceWindowFigures {
  /** From the trace's first event to its last, ms. */
  readonly spanMs: number;
  /** The file's size, bytes. */
  readonly bytes: number;
  /** The buffer's use before the stop, %, or `null` when Electron reports none. */
  readonly bufferPercent: number | null;
}

/** One window of a run's trace, as the results file records it. */
export interface TraceWindowRecord {
  readonly index: number;
  /** Script time when its recording began, s; negative for the first, started before the script. */
  readonly fromS: number;
  /** Script time of its stop request, s. */
  readonly toS: number;
  /** What its file held, or why the window failed. */
  readonly figures: Measured<TraceWindowFigures>;
}

/** The gap between two windows and the stretch of frames left out around it. */
export interface TraceBoundary {
  /** The window before it. */
  readonly afterWindow: number;
  /** Script time of that window's stop request, s: the exclusion's start. */
  readonly stopRequestedS: number;
  /** Script time when the next window's start resolved, s. */
  readonly resumedS: number;
  /** `resumedS` plus the guard, s: the exclusion's end. */
  readonly excludedToS: number;
  /** Frames after the warm-up in `[stopRequestedS, excludedToS)`, left out of every figure. */
  readonly excludedFrames: number;
  /** Their largest `requestAnimationFrame` interval, ms: the stall's size; `null` for none. */
  readonly maxRafIntervalMs: number | null;
}

/** The results file's `run.trace` (results schema version 3). */
export interface TraceRun extends TraceSettings {
  /** {@link TRACE_BOUNDARY_GUARD_S} as the run used it, s. */
  readonly guardS: number;
  readonly windows: ReadonlyArray<TraceWindowRecord>;
  /** One between each pair of windows. */
  readonly boundaries: ReadonlyArray<TraceBoundary>;
  /** Script time after the warm-up, to the script's end, that is traced and not excluded, s. */
  readonly tracedS: number;
}

/** A stretch of script time, `[fromS, toS)`. */
export interface ScriptInterval {
  readonly fromS: number;
  readonly toS: number;
}

/** The renderer's main thread's time, summed over the windows. */
export type MainThreadSplit = Omit<MainThreadFigures, "engineSelfMs" | "sampledMs">;

/** The engine adapter's sampled self time, from a profiled run's CPU profile. */
export interface EngineFigures {
  /** Sampled self time in the engine's chunk, ms. */
  readonly selfMs: number;
  /** Sampled self time of the whole profile, ms. */
  readonly sampledMs: number;
}

/** One thread's GC pauses over the windows. */
export interface GcFigures {
  readonly process: string | null;
  readonly thread: string | null;
  readonly count: number;
  readonly totalMs: number;
  readonly maxMs: number;
}

/** The windows' figures, pooled. */
export interface PooledTraceFigures {
  /** The presentation intervals kept, ms, in window and time order. */
  readonly presentationMs: ReadonlyArray<number>;
  /** The same, by the segment of each interval's end. */
  readonly presentationBySegmentMs: ReadonlyMap<string, ReadonlyArray<number>>;
  /** Frames Chromium dropped, after the warm-up and outside every exclusion. */
  readonly dropped: number;
  readonly gpuProcess: Measured<GpuProcessFigures>;
  readonly split: Measured<MainThreadSplit>;
  /** Missing with {@link PROFILER_OFF_REASON} in an unprofiled run. */
  readonly engine: Measured<EngineFigures>;
  /** Threads with a GC pause, by process and thread ID. */
  readonly gc: ReadonlyArray<GcFigures>;
}

/** A run's windows, merged. */
export interface MergedTrace {
  /** The results file's `run.trace`, or why the run has no trace. */
  readonly run: Measured<TraceRun>;
  /** The boundaries' excluded intervals, in order; none without a trace. */
  readonly exclusions: ReadonlyArray<ScriptInterval>;
  /** The pooled figures, or why every figure read from the trace is missing. */
  readonly figures: Measured<PooledTraceFigures>;
}

/** What the merge reads of the renderer's report. */
export interface TraceWindowsReport extends Pick<
  DescentSpikeReport,
  "scriptStartMs" | "traceWindows" | "warmupS" | "segments"
> {
  readonly frames: Pick<SpikeFrameSeries, "scriptTimesS" | "rafIntervalsMs">;
}

/** A window that passed its checks: its trace, the trace's clock offset, and its file's figures. */
interface GoodWindow {
  readonly trace: TraceFigures;
  /** The trace's {@link TraceFigures.clockOffsetUs}, known present. */
  readonly offsetUs: number;
  readonly figures: TraceWindowFigures;
  readonly failure: null;
}

/** A window that failed, and why. */
interface FailedWindow {
  readonly failure: string;
}

/** A window's file checked. */
type Checked = GoodWindow | FailedWindow;

function failed(failure: string): FailedWindow {
  return { failure };
}

function isGood(window: Checked): window is GoodWindow {
  return window.failure === null;
}

function seconds(ms: number): string {
  return (ms / 1000).toFixed(1);
}

/** A window's file against the time the renderer recorded it. */
function checkWindow(file: TraceWindowFile, time: SpikeTraceWindow): Checked {
  const trace = file.trace.value;
  if (trace === null) {
    return failed(file.trace.reason);
  }
  if (trace.span === null) {
    return failed(EMPTY_TRACE_REASON);
  }
  if (trace.clockOffsetUs === null) {
    return failed(NO_CLOCK_OFFSET_REASON);
  }
  if (file.bytes === null) {
    return failed("its file's size could not be read");
  }
  const spanMs = (trace.span.lastUs - trace.span.firstUs) / 1000;
  const recordedMs = time.stopRequestedMs - time.startedMs;
  if (spanMs < SHORT_SPAN_FRACTION * recordedMs) {
    return failed(
      `it filled its buffer: it spans ${seconds(spanMs)} s of the ${seconds(recordedMs)} s recorded`,
    );
  }
  if (file.bufferPercent !== null && file.bufferPercent >= FULL_BUFFER_PERCENT) {
    return failed(`it filled its buffer: ${file.bufferPercent.toFixed(0)} % of it was used`);
  }
  return {
    trace,
    offsetUs: trace.clockOffsetUs,
    figures: { spanMs, bytes: file.bytes, bufferPercent: file.bufferPercent },
    failure: null,
  };
}

/** The renderer, main thread and compositor a window's frames and split are read from. */
function rendererOf(trace: TraceFigures): string {
  return `renderer ${String(trace.frames.pid)}, main thread ${String(trace.mainThread?.tid ?? null)}, compositor ${String(trace.frames.layerTreeHostId)}`;
}

/** The windows with the renderer check added: every good window must show the first good one's. */
function sameRenderer(checked: ReadonlyArray<Checked>): Checked[] {
  const first = checked.findIndex(isGood);
  const reference = checked[first];
  if (reference === undefined || !isGood(reference)) {
    return [...checked];
  }
  const expected = rendererOf(reference.trace);
  return checked.map((window) => {
    if (!isGood(window)) {
      return window;
    }
    const shown = rendererOf(window.trace);
    return shown === expected
      ? window
      : failed(
          `it shows another renderer than trace window ${first + 1}: ${shown}, against ${expected}`,
        );
  });
}

/** A run with no trace, every figure read from one missing for `reason`. */
function noTrace(reason: string): MergedTrace {
  return { run: missing(reason), exclusions: [], figures: missing(reason) };
}

/** The length of `[fromS, toS)` within `[lowS, highS)`, s. */
function within(fromS: number, toS: number, lowS: number, highS: number): number {
  return Math.max(0, Math.min(toS, highS) - Math.max(fromS, lowS));
}

/**
 * The run's trace windows merged: each placed in script time, the frames at its boundaries left
 * out, and its figures pooled.
 *
 * @param recording - Its windows in the order the renderer's `report.traceWindows` lists them.
 * @param guardS - How long after each window's start its boundary's frames are still left out, s.
 */
export function mergeTraceWindows(
  recording: Measured<TraceRecording>,
  report: TraceWindowsReport,
  guardS: number = TRACE_BOUNDARY_GUARD_S,
): MergedTrace {
  if (recording.value === null) {
    return noTrace(recording.reason);
  }
  const { settings, windows: files } = recording.value;
  const times = report.traceWindows;
  if (times.length === 0) {
    return noTrace("the renderer reported no trace window");
  }
  if (times.length !== files.length) {
    return noTrace(
      `the renderer reported ${times.length} trace windows, and the main process wrote ${files.length}`,
    );
  }
  const scriptS = (perfMs: number): number => (perfMs - report.scriptStartMs) / 1000;
  const checked = sameRenderer(
    times.map((time, i) =>
      checkWindow(
        files[i] ?? { trace: missing("no trace file"), bytes: null, bufferPercent: null },
        time,
      ),
    ),
  );
  const windows: TraceWindowRecord[] = times.map((time, index) => {
    const window = checked[index] ?? failed("no trace file");
    return {
      index,
      fromS: scriptS(time.startedMs),
      toS: scriptS(time.stopRequestedMs),
      figures: isGood(window) ? measured(window.figures) : missing(window.failure),
    };
  });
  const { warmupS } = report;
  const { scriptTimesS, rafIntervalsMs } = report.frames;
  const boundaries: TraceBoundary[] = windows.slice(1).map((next, k) => {
    const stopRequestedS = windows[k]?.toS ?? next.fromS;
    const excludedToS = next.fromS + guardS;
    let excludedFrames = 0;
    let maxRafIntervalMs: number | null = null;
    for (const [i, t] of scriptTimesS.entries()) {
      if (t >= warmupS && t >= stopRequestedS && t < excludedToS) {
        excludedFrames += 1;
        maxRafIntervalMs = Math.max(maxRafIntervalMs ?? 0, rafIntervalsMs[i] ?? 0);
      }
    }
    return {
      afterWindow: k,
      stopRequestedS,
      resumedS: next.fromS,
      excludedToS,
      excludedFrames,
      maxRafIntervalMs,
    };
  });
  const exclusions = boundaries.map(({ stopRequestedS, excludedToS }) => ({
    fromS: stopRequestedS,
    toS: excludedToS,
  }));
  const scriptEndS = Math.max(0, ...report.segments.map(({ endS }) => endS));
  let tracedS = 0;
  for (const window of windows) {
    tracedS += within(window.fromS, window.toS, warmupS, scriptEndS);
    for (const { fromS, toS } of exclusions) {
      tracedS -= within(
        Math.max(fromS, window.fromS),
        Math.min(toS, window.toS),
        warmupS,
        scriptEndS,
      );
    }
  }
  const run: TraceRun = { ...settings, guardS, windows, boundaries, tracedS };
  const n = windows.length;
  const firstFailed = checked.findIndex((window) => !isGood(window));
  const failure = checked[firstFailed];
  const good = checked.filter(isGood);
  const figures =
    failure === undefined
      ? measured(pool(good, settings, report, exclusions))
      : missing(`trace window ${firstFailed + 1} of ${n}: ${failure.failure}`);
  return { run: measured(run), exclusions, figures };
}

/** Every window's figures pooled, each placed in script time by its clock offset. */
function pool(
  windows: ReadonlyArray<GoodWindow>,
  settings: TraceSettings,
  report: TraceWindowsReport,
  exclusions: ReadonlyArray<ScriptInterval>,
): PooledTraceFigures {
  const traces = windows.map(({ trace }) => trace);
  const n = traces.length;
  const ofWindow = (k: number, reason: string): string =>
    `trace window ${k + 1} of ${n}: ${reason}`;
  const kept = (t: number): boolean =>
    t >= report.warmupS && !exclusions.some(({ fromS, toS }) => t >= fromS && t < toS);
  const segmentAt = (t: number): string | undefined =>
    report.segments.find(({ startS, endS }) => t >= startS && t < endS)?.name;

  const presentationMs: number[] = [];
  const bySegment = new Map<string, number[]>();
  let dropped = 0;
  for (const { trace, offsetUs } of windows) {
    const scriptS = (us: number): number => ((us - offsetUs) / 1000 - report.scriptStartMs) / 1000;
    const presented = trace.frames.presentedAtUs;
    for (let i = 1; i < presented.length; i += 1) {
      const startUs = presented[i - 1] ?? 0;
      const endUs = presented[i] ?? 0;
      const endS = scriptS(endUs);
      if (!kept(scriptS(startUs)) || !kept(endS)) {
        continue;
      }
      const intervalMs = (endUs - startUs) / 1000;
      presentationMs.push(intervalMs);
      const segment = segmentAt(endS);
      if (segment !== undefined) {
        const list = bySegment.get(segment) ?? [];
        list.push(intervalMs);
        bySegment.set(segment, list);
      }
    }
    dropped += trace.frames.droppedAtUs.filter((us) => kept(scriptS(us))).length;
  }

  const noGpu = traces.findIndex(({ gpuProcess }) => gpuProcess === null);
  const gpus = traces.flatMap(({ gpuProcess }) => (gpuProcess === null ? [] : [gpuProcess]));
  const [firstGpu] = gpus;
  const gpuProcess: Measured<GpuProcessFigures> =
    noGpu >= 0 || firstGpu === undefined
      ? missing(ofWindow(Math.max(noGpu, 0), "the trace has no GPU process"))
      : measured({
          // A GPU process restarted after a crash sums with the first; its IDs are the first's.
          pid: firstGpu.pid,
          tid: firstGpu.tid,
          busyMs: gpus.reduce((sum, gpu) => sum + gpu.busyMs, 0),
          slices: firstGpu.slices.map(({ name }) => {
            const of = gpus.flatMap(({ slices }) => slices.filter((slice) => slice.name === name));
            return {
              name,
              count: of.reduce((sum, slice) => sum + slice.count, 0),
              totalMs: of.reduce((sum, slice) => sum + slice.totalMs, 0),
              maxMs: of.reduce((max, slice) => Math.max(max, slice.maxMs), 0),
            };
          }),
        });

  const noMain = traces.findIndex(({ mainThread }) => mainThread === null);
  const mains = traces.flatMap(({ mainThread }) => (mainThread === null ? [] : [mainThread]));
  const [firstMain] = mains;
  const sum = (pickMs: (main: MainThreadFigures) => number): number =>
    mains.reduce((total, main) => total + pickMs(main), 0);
  const split: Measured<MainThreadSplit> =
    noMain >= 0 || firstMain === undefined
      ? missing(ofWindow(Math.max(noMain, 0), "the trace has no renderer main thread"))
      : measured({
          pid: firstMain.pid,
          tid: firstMain.tid,
          wallMs: sum(({ wallMs }) => wallMs),
          busyMs: sum(({ busyMs }) => busyMs),
          ourCodeMs: sum(({ ourCodeMs }) => ourCodeMs),
          idleMs: sum(({ idleMs }) => idleMs),
        });
  const unprofiled = traces.findIndex(
    ({ mainThread }) =>
      mainThread === null || mainThread.engineSelfMs === null || mainThread.sampledMs === null,
  );
  const profiles = mains.flatMap(({ engineSelfMs, sampledMs }) =>
    engineSelfMs === null || sampledMs === null ? [] : [{ engineSelfMs, sampledMs }],
  );
  const engine: Measured<EngineFigures> = !settings.profiled
    ? missing(PROFILER_OFF_REASON)
    : split.value === null
      ? missing(split.reason)
      : unprofiled >= 0 || profiles.length !== n
        ? missing(
            ofWindow(
              Math.max(unprofiled, 0),
              "the trace has no CPU profile of the renderer's main thread",
            ),
          )
        : measured({
            selfMs: profiles.reduce((total, { engineSelfMs }) => total + engineSelfMs, 0),
            sampledMs: profiles.reduce((total, { sampledMs }) => total + sampledMs, 0),
          });

  const gcByThread = new Map<string, GcFigures & { readonly pid: number; readonly tid: number }>();
  for (const trace of traces) {
    for (const { pid, tid, process, thread, gc } of trace.threads) {
      if (gc.count === 0) {
        continue;
      }
      const key = `${pid}:${tid}`;
      const before = gcByThread.get(key);
      gcByThread.set(key, {
        pid,
        tid,
        process: before?.process ?? process,
        thread: before?.thread ?? thread,
        count: (before?.count ?? 0) + gc.count,
        totalMs: (before?.totalMs ?? 0) + gc.totalMs,
        maxMs: Math.max(before?.maxMs ?? 0, gc.maxMs),
      });
    }
  }
  const gc = [...gcByThread.values()]
    .toSorted((a, b) => a.pid - b.pid || a.tid - b.tid)
    .map(({ process, thread, count, totalMs, maxMs }) => ({
      process,
      thread,
      count,
      totalMs,
      maxMs,
    }));

  return {
    presentationMs,
    presentationBySegmentMs: bySegment,
    dropped,
    gpuProcess,
    split,
    engine,
    gc,
  };
}
