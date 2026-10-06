import type { SpikeFrameSeries } from "../../preload/api";
import { type DurationSummary, recordedGpuSlices, type TraceFigures } from "../reduceTrace";
import { type Measured, measured } from "../measured";
import type { TraceRecording, TraceSettings, TraceWindowFile } from "../traceWindows";

/** An unprofiled run's settings, as T14.g's default trace records them: no `gpu`, no profiler. */
export const UNPROFILED: TraceSettings = {
  format: "json",
  profiled: false,
  categories: [
    "devtools.timeline",
    "disabled-by-default-devtools.timeline",
    "disabled-by-default-devtools.timeline.frame",
    "disabled-by-default-v8.gc",
    "blink.user_timing",
  ],
  recordingMode: "record-until-full",
  bufferKb: 786_432,
};

/** A profiled run's: the same, with `gpu` and V8's CPU profiler. */
export const PROFILED: TraceSettings = {
  ...UNPROFILED,
  profiled: true,
  categories: [...UNPROFILED.categories, "gpu", "disabled-by-default-v8.cpu_profiler"],
};

/** The renderer's frames a window's `spike.frame` spans are made from: a report's. */
export interface FrameSource {
  readonly frames: Pick<SpikeFrameSeries, "ourCodeMs" | "callbackStartsMs">;
}

/** How long after its frame's `requestAnimationFrame` time a test report's callback starts, ms. */
export const CALLBACK_LAG_MS = 0.1;

/**
 * A test report's callback starts: each {@link CALLBACK_LAG_MS} after its frame's
 * `requestAnimationFrame` time, `scriptStartMs` + 1000 × its script time.
 */
export function callbackStartsOf(
  scriptStartMs: number,
  scriptTimesS: ReadonlyArray<number>,
): number[] {
  return scriptTimesS.map((t) => scriptStartMs + 1000 * t + CALLBACK_LAG_MS);
}

/** What a test gives of one window's reduced trace. */
export interface WindowTraceOptions {
  /** The trace's clock less the page's, µs. */
  readonly offsetUs: number;
  /** The span's first and last events, in the page's `performance.now()` ms. */
  readonly fromMs: number;
  readonly toMs: number;
  /** Presentations' and dropped frames' ends, page ms. */
  readonly presentedMs?: ReadonlyArray<number>;
  readonly droppedMs?: ReadonlyArray<number>;
  /** The main thread's busy and our-code time, ms. */
  readonly busyMs?: number;
  readonly ourCodeMs?: number;
  /** The engine chunk's and the profile's sampled self time, ms; `null` without a profile. */
  readonly engine?: { readonly selfMs: number; readonly sampledMs: number } | null;
  /** The main thread's GC pauses. */
  readonly gc?: DurationSummary;
  /** The GPU process's busy time, and its `GPUTask` slices, ms. */
  readonly gpuBusyMs?: number;
  readonly gpuSlice?: DurationSummary;
  /** The categories recorded, which list the GPU process's slices; {@link UNPROFILED}'s by default. */
  readonly categories?: ReadonlyArray<string>;
  /**
   * The frames whose `spike.frame` spans the window holds: one for each frame whose callback starts
   * in `[fromMs, toMs]`, from its callback start (its `args.startTime`) for its `ourCodeMs`. None
   * without.
   */
  readonly frames?: FrameSource;
}

/** The frames of `source` whose callbacks start in `[fromMs, toMs]`, as their spans: page ms. */
export function frameSpansOf(
  source: FrameSource,
  fromMs: number,
  toMs: number,
): Array<{ readonly startMs: number; readonly durationMs: number }> {
  const { callbackStartsMs, ourCodeMs } = source.frames;
  return callbackStartsMs.flatMap((startMs, i) =>
    startMs >= fromMs && startMs <= toMs ? [{ startMs, durationMs: ourCodeMs[i] ?? 0 }] : [],
  );
}

/**
 * A window's reduced trace: renderer 1's main thread 2 and compositor 1, the GPU process 3's main
 * thread 4, its times on a clock `offsetUs` ahead of the page's.
 */
export function windowTrace(options: WindowTraceOptions): TraceFigures {
  const us = (ms: number): number => Math.round(options.offsetUs + 1000 * ms);
  const spans =
    options.frames === undefined ? [] : frameSpansOf(options.frames, options.fromMs, options.toMs);
  const presentedAtUs = (options.presentedMs ?? []).map(us);
  const droppedAtUs = (options.droppedMs ?? []).map(us);
  const wallMs = options.toMs - options.fromMs;
  const busyMs = options.busyMs ?? 100;
  const gc = options.gc ?? { count: 3, totalMs: 6, maxMs: 3 };
  const engine = options.engine === undefined ? { selfMs: 10, sampledMs: 40 } : options.engine;
  return {
    span: { firstUs: us(options.fromMs), lastUs: us(options.toMs) },
    clockOffsetUs: options.offsetUs,
    frames: {
      pid: 1,
      layerTreeHostId: 1,
      presentedAtUs,
      intervalsMs: presentedAtUs.slice(1).map((end, i) => (end - (presentedAtUs[i] ?? end)) / 1000),
      presented: presentedAtUs.length,
      dropped: droppedAtUs.length,
      droppedAtUs,
      noUpdate: 0,
    },
    mainThread: {
      pid: 1,
      tid: 2,
      wallMs,
      busyMs,
      ourCodeMs: options.ourCodeMs ?? 50,
      frameSpans: {
        startsUs: spans.map(({ startMs }) => us(startMs)),
        durationsMs: spans.map(({ durationMs }) => durationMs),
        startTimesMs: spans.map(({ startMs }) => startMs),
      },
      engineSelfMs: engine?.selfMs ?? null,
      sampledMs: engine?.sampledMs ?? null,
      idleMs: wallMs - busyMs,
    },
    gpuProcess: {
      pid: 3,
      tid: 4,
      busyMs: options.gpuBusyMs ?? 20,
      slices: recordedGpuSlices(options.categories ?? UNPROFILED.categories).map((name) =>
        Object.assign(
          { name },
          name === "GPUTask"
            ? (options.gpuSlice ?? { count: 2, totalMs: 4, maxMs: 3 })
            : { count: 0, totalMs: 0, maxMs: 0 },
        ),
      ),
    },
    threads: [
      { pid: 1, tid: 2, process: "Renderer", thread: "CrRendererMain", busyMs, gc },
      {
        pid: 3,
        tid: 4,
        process: "GPU Process",
        thread: "CrGpuMain",
        busyMs: 20,
        gc: { count: 0, totalMs: 0, maxMs: 0 },
      },
    ],
    userTiming: [],
  };
}

/** A window's file holding `trace`, of 1 MB with a tenth of its buffer used. */
export function windowFile(trace: TraceFigures): TraceWindowFile {
  return { trace: measured(trace), bytes: 1_000_000, bufferPercent: 10 };
}

/** A recording of `windows`, unprofiled unless `settings` says otherwise. */
export function recordingOf(
  windows: ReadonlyArray<TraceWindowFile>,
  settings: TraceSettings = UNPROFILED,
): Measured<TraceRecording> {
  return measured({ settings, windows });
}
