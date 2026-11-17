/**
 * A perf run's record, assembled in the main process from its samples and the renderer's
 * repetitions (plan R12, R12.T4.b; Design notes 4 to 7 and 13), validated by `@hyperion/perf`'s
 * `validatePerfRecord` before it is appended.
 *
 * @remarks
 * Frame intervals are R05's (`main/results.ts`' `frameStats`): presentation times from the trace
 * where its windows hold at least two presentations within the span, else the renderer's
 * `requestAnimationFrame` intervals; a missed frame is an interval above 1.5 T and a hitch one above
 * 3 T, T being the display's vsync period times the scene's pacing (one vsync or two), or the
 * primary view's rate's period where the display's is unknown (a hidden run). A presentation is
 * counted in the segment of the last frame the renderer began at or before it. GPU busy time per
 * frame is the GPU process's `drm-engine-render` time over the span divided by its frames, and the
 * `compositing` row is that less the timed passes' mean (Design note 6), every figure `null` where
 * the busy time is unread (NVIDIA, hardware item 2), so that the table reads `NOT AVAILABLE`.
 */

import {
  type FrameFigures,
  type MachineRecord,
  PERF_RECORD_VERSION,
  type PerfRepetition,
  type PerfRunRecord,
  type PerfTimer,
  type PowerThermalRecord,
  perfRecordId,
  type RowFigures,
  type SegmentFigures,
  type ServerRecord,
  type ThrottleFigures,
  validatePerfRecord,
} from "@hyperion/perf";

import type {
  GraphicsLaunchMode,
  PerfLaunch,
  PerfRepetitionResult,
  PerfRunDescription,
} from "../../preload/api";
import { frameStats, nearestRank } from "../results";
import {
  displayServerFiguresOf,
  type DrmSpanSummary,
  gpuMemoryHeadline,
  type NvidiaSpanSummary,
} from "./samplers";
import type { TrendFlag } from "./steadiness";

/** The main process's measurements of one repetition's span, on the runner's clock (epoch ms). */
export interface SpanMeasurements {
  /** The span's start and end, ms since the epoch. */
  readonly fromMs: number;
  readonly toMs: number;
  /**
   * The trace's presentation times by window, the page's `performance.now()` ms, ascending; `null`
   * without a trace.
   */
  readonly presentedWindowsMs: ReadonlyArray<ReadonlyArray<number>> | null;
  readonly throttle: ThrottleFigures;
  /** The GPU process's DRM samples over the span, or `null` where none was read. */
  readonly drm: DrmSpanSummary | null;
  readonly nvidia: NvidiaSpanSummary | null;
  /** Xorg's DRM samples over the span, or `null` where it was not read. */
  readonly displayServer: DrmSpanSummary | null;
  /** The peak of every process's working set, bytes, or `null` where unread. */
  readonly appPeakBytes: number | null;
}

/** One repetition: the renderer's result and the main process's measurements of its span. */
export interface RepetitionInputs {
  readonly result: PerfRepetitionResult;
  readonly span: SpanMeasurements;
}

/** What the harness section of the record is made from, besides the repetitions. */
export interface HarnessInputs {
  readonly gpuTiming: boolean;
  readonly switches: ReadonlyArray<string>;
  readonly launchMode: GraphicsLaunchMode;
  readonly warmUp: { readonly seconds: number; readonly steady: boolean };
  readonly server: ServerRecord;
  /** The display's vsync period, ms, or `null` where it is not known (a hidden run). */
  readonly vsyncPeriodMs: number | null;
  /** `virtual` for a hidden run, whose frames Chromium's timer paces (Design note 11). */
  readonly display: "physical" | "virtual";
}

/** Everything a run's record is assembled from. */
export interface RunInputs {
  readonly launch: PerfLaunch;
  /** The run's start, ms since the epoch. */
  readonly startedAtMs: number;
  readonly commit: string;
  readonly dirty: boolean;
  readonly generatorVersion: number;
  readonly description: PerfRunDescription;
  readonly machine: MachineRecord;
  readonly state: PowerThermalRecord;
  /** The preconditions that failed, which `--force` overrode (Design note 7). */
  readonly forcedReasons: ReadonlyArray<string>;
  readonly harness: HarnessInputs;
  readonly repetitions: ReadonlyArray<RepetitionInputs>;
  /** What the record's `notes` say, in order: the run's own, then the main process's. */
  readonly notes: ReadonlyArray<string>;
}

/** The timer states from best to worst; a run reports the worst any repetition saw. */
const TIMER_RANK: Readonly<Record<PerfTimer, number>> = { full: 0, quantized: 1, absent: 2 };

/** The mean of `values`, or `null` for none. */
function mean(values: ReadonlyArray<number>): number | null {
  return values.length === 0 ? null : values.reduce((sum, v) => sum + v, 0) / values.length;
}

/**
 * T, ms: the display's vsync period times the pacing's vsyncs, or the primary view's rate's period
 * where the display's is unknown; `null` with neither.
 */
export function periodOf(
  vsyncPeriodMs: number | null,
  pacing: PerfRunDescription["pacing"],
  primaryRateHz: number | null,
): number | null {
  const vsyncs = pacing === "every-second-vsync" ? 2 : 1;
  if (vsyncPeriodMs !== null && vsyncPeriodMs > 0) {
    return vsyncPeriodMs * vsyncs;
  }
  return primaryRateHz !== null && primaryRateHz > 0 ? 1000 / primaryRateHz : null;
}

/** Frame figures of `intervalsMs` against T, R05's `frameStats` with the mean. */
export function frameFiguresOf(
  source: FrameFigures["source"],
  intervalsMs: ReadonlyArray<number>,
  periodMs: number | null,
): FrameFigures {
  const stats = frameStats(intervalsMs, periodMs);
  return {
    source,
    count: intervalsMs.length,
    meanMs: mean(intervalsMs),
    p50Ms: stats?.p50Ms ?? null,
    p95Ms: stats?.p95Ms ?? null,
    p99Ms: stats?.p99Ms ?? null,
    maxMs: stats?.maxMs ?? null,
    missedFraction: stats?.missedFraction ?? null,
    hitches: stats?.hitches ?? null,
  };
}

/** A frame's intervals, each with the script time it was counted at. */
interface CountedIntervals {
  readonly source: FrameFigures["source"];
  readonly intervalsMs: ReadonlyArray<number>;
  readonly scriptTimesS: ReadonlyArray<number>;
}

/**
 * The script time of the last frame the renderer began at or before `atMs` (page ms), or `null`
 * before its first.
 */
function scriptTimeAt(result: PerfRepetitionResult, atMs: number): number | null {
  const starts = result.frames.callbackStartsMs;
  let lo = 0;
  let hi = starts.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if ((starts[mid] ?? Infinity) <= atMs) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  return lo === 0 ? null : (result.frames.scriptTimesS[lo - 1] ?? null);
}

/** How long after the span's last frame began its presentation may come, ms. */
const LAST_PRESENTATION_MS = 1000;

/**
 * The repetition's intervals: the trace's presentations within the renderer's frames, within each
 * window, where at least two are, else the renderer's own; each interval counted at the script time
 * of the frame that ends it.
 */
export function countedIntervals(
  result: PerfRepetitionResult,
  presentedWindowsMs: ReadonlyArray<ReadonlyArray<number>> | null,
): CountedIntervals {
  const first = result.frames.callbackStartsMs[0];
  const last = result.frames.callbackStartsMs.at(-1);
  const intervalsMs: number[] = [];
  const scriptTimesS: number[] = [];
  if (presentedWindowsMs !== null && first !== undefined && last !== undefined) {
    for (const window of presentedWindowsMs) {
      const presented = window.filter((ms) => ms >= first && ms <= last + LAST_PRESENTATION_MS);
      for (let i = 1; i < presented.length; i += 1) {
        const at = presented[i] ?? 0;
        intervalsMs.push(at - (presented[i - 1] ?? at));
        scriptTimesS.push(scriptTimeAt(result, at) ?? 0);
      }
    }
  }
  if (intervalsMs.length > 0) {
    return { source: "presentation", intervalsMs, scriptTimesS };
  }
  // The first frame's interval is from the warm-up's last frame, or from the reset before the span.
  return {
    source: "raf",
    intervalsMs: result.frames.intervalsMs.slice(1),
    scriptTimesS: result.frames.scriptTimesS.slice(1),
  };
}

/** The rows of a repetition: the renderer's, and the `compositing` row from the busy time. */
function rowsOf(
  result: PerfRepetitionResult,
  frames: number,
  gpuBusyPerFrameMs: number | null,
): ReadonlyArray<RowFigures> {
  const compositingMs =
    gpuBusyPerFrameMs === null || result.timedPassesPerFrameMs === null
      ? null
      : Math.max(0, gpuBusyPerFrameMs - result.timedPassesPerFrameMs);
  return [
    ...result.rows.filter(({ row }) => row !== "compositing"),
    { row: "compositing", frames, meanMs: compositingMs, p50Ms: null, p95Ms: null, p99Ms: null },
  ];
}

/** A repetition of the record. */
function repetitionOf(
  index: number,
  inputs: RepetitionInputs,
  periodMs: number | null,
): { readonly repetition: PerfRepetition; readonly intervalsMs: ReadonlyArray<number> } {
  const { result, span } = inputs;
  const counted = countedIntervals(result, span.presentedWindowsMs);
  const frames = frameFiguresOf(counted.source, counted.intervalsMs, periodMs);
  const segments: SegmentFigures[] = result.segments.map(({ name, startS, endS }) => ({
    segment: name,
    frames: frameFiguresOf(
      counted.source,
      counted.intervalsMs.filter((_, i) => {
        const t = counted.scriptTimesS[i] ?? -1;
        return t >= startS && t < endS;
      }),
      periodMs,
    ),
  }));
  const busyNs = span.drm?.busyNs ?? null;
  const gpuBusyPerFrameMs =
    busyNs === null || result.frames.callbackMs.length === 0
      ? null
      : busyNs / 1e6 / result.frames.callbackMs.length;
  const tallyPeakBytes = result.memory.tallyPeakBytes;
  return {
    intervalsMs: counted.intervalsMs,
    repetition: {
      index,
      startedAt: new Date(span.fromMs).toISOString(),
      frames,
      segments,
      gpuBusyPerFrameMs,
      mainThreadPerFrameMs: mean(result.frames.callbackMs),
      rows: rowsOf(result, frames.count, gpuBusyPerFrameMs),
      passes: result.passes,
      memory: {
        itemPeakBytes: result.memory.itemPeakBytes,
        tallyPeakBytes,
        gpu: gpuMemoryHeadline(span.drm, span.nvidia, tallyPeakBytes),
        appPeakBytes: span.appPeakBytes,
        workerFieldBytes: result.memory.workerFieldBytes,
        workerWasmBytes: result.memory.workerWasmBytes,
        nvidiaDeviceLessBaselinePeakBytes: span.nvidia?.deviceLessBaselinePeakBytes ?? null,
        displayServer:
          span.displayServer === null ? null : displayServerFiguresOf(span.displayServer),
      },
      streaming: result.streaming,
      throttle: span.throttle,
      latePipelines: result.latePipelines,
      heightPointUs: result.heightPointUs,
    },
  };
}

/** The trend screen's line for the record's notes, or `null` where the screen flags nothing. */
export function trendNote(flag: TrendFlag | null): string | null {
  return flag === null
    ? null
    : `TREND: the repetitions' mean frame times are monotone, ${flag.direction} (Mann–Kendall S = ${String(flag.s)}; Design note 7's drift screen)`;
}

/**
 * The run's record from its inputs, its ID numbered 1 (the store renumbers it on append).
 *
 * @throws `@hyperion/perf`'s `PerfRecordError` if the record does not validate, or Error with no
 *   repetition, since a record holds at least one.
 */
export function assemblePerfRecord(inputs: RunInputs): PerfRunRecord {
  const { description, harness, launch } = inputs;
  if (inputs.repetitions.length === 0) {
    throw new Error("a perf record holds at least one repetition");
  }
  const primary = description.views.find(({ role }) => role !== "instrument") ?? null;
  const periodMs = periodOf(harness.vsyncPeriodMs, description.pacing, primary?.rateHz ?? null);
  const built = inputs.repetitions.map((repetition, i) =>
    repetitionOf(i + 1, repetition, periodMs),
  );
  const pooled = built.flatMap(({ intervalsMs }) => intervalsMs).toSorted((a, b) => a - b);
  const timer = inputs.repetitions
    .map(({ result }) => result.timer)
    .reduce<PerfTimer>((worst, t) => (TIMER_RANK[t] > TIMER_RANK[worst] ? t : worst), "full");
  const scales = description.views.map((_, v) => {
    const each = inputs.repetitions.flatMap(({ result }) => result.internalScale[v] ?? []);
    const medians = each.map(({ median: m }) => m).toSorted((a, b) => a - b);
    return {
      min: each.length === 0 ? 1 : Math.min(...each.map(({ min }) => min)),
      median: medians.length === 0 ? 1 : nearestRank(medians, 0.5),
    };
  });
  const date = new Date(inputs.startedAtMs).toISOString();
  const spans = inputs.repetitions.map(({ result }) => result.spanS).toSorted((a, b) => a - b);
  const record: PerfRunRecord = {
    version: PERF_RECORD_VERSION,
    id: perfRecordId(date, inputs.machine.name, launch.scene, 1),
    date,
    commit: inputs.commit,
    dirty: inputs.dirty,
    generatorVersion: inputs.generatorVersion,
    source: "perf-run",
    scene: { id: launch.scene, ...description.scene },
    setting: { quality: launch.setting, values: description.settingValues },
    views: description.views.map((view, v) => ({
      ...view,
      internalScale: scales[v] ?? { min: 1, median: 1 },
    })),
    machine: inputs.machine,
    state: inputs.state,
    harness: {
      gpuTiming: harness.gpuTiming,
      timer,
      bracketedPasses: [],
      switches: harness.switches,
      launchMode: harness.launchMode,
      repetitions: built.length,
      repetitionSeconds: description.repetitionSeconds ?? nearestRank(spans, 0.5),
      warmUp: harness.warmUp,
      forced: inputs.forcedReasons.length > 0,
      forcedReasons: inputs.forcedReasons,
      pacing: description.pacing,
      vsyncPeriodMs: harness.vsyncPeriodMs,
      server: harness.server,
      heightWorkers: description.heightWorkers,
      display: harness.display,
    },
    runs: built.map(({ repetition }) => repetition),
    pooledFrames: {
      count: pooled.length,
      p99Ms: pooled.length === 0 ? null : nearestRank(pooled, 0.99),
    },
    arrival:
      inputs.repetitions.find(({ result }) => result.arrival !== null)?.result.arrival ?? null,
    notes: inputs.notes.filter((note) => note.length > 0).join(" "),
  };
  return validatePerfRecord(record);
}
