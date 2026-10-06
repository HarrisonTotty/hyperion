/**
 * The descent spike's per-frame measurements in the renderer (plan R05, T14.a, Design note 18),
 * gathered into the report the main process turns into the results file (T14.c's
 * `DescentSpikeReport`, less what `SpikeController` adds).
 *
 * @remarks
 * The renderer keeps raw series and counts; the percentiles, missed frames and hitches are the
 * main process's (`main/results.ts`), so that one convention serves the presentation times from
 * the trace and the series here. Each segment is marked in the trace with one
 * `performance.measure` span named {@link SEGMENT_MEASURE_PREFIX}`<name>`, for reading the trace
 * by eye: the main process assigns presentations to segments by their script time
 * (`main/traceWindows.ts`, T14.d), not by these marks.
 */

import type {
  DescentSpikeReport,
  SpikeLatePipeline,
  SpikePassRow,
  SpikeSegmentSpan,
  SpikeStreamingSegment,
} from "../../../../preload/api";
import type { GpuTimer } from "../engine/status";
import type { AllocationEvent } from "../engine/memory";
import type { PassTimes } from "../engine/types";

/** The `performance.measure` name prefix of a segment's span, which marks it in the trace. */
export const SEGMENT_MEASURE_PREFIX = "spike.segment:";

/** The passes R01's timer times in a frame; more are untimed (Consumes, "As built"). */
export const TIMED_PASSES_A_FRAME = 64;

/** One frame as the spike's loop sees it. */
export interface FrameSample {
  /**
   * The engine's last timing frame number by the end of this frame, which its `PassTimes.frame`
   * carries. R01 numbers each resolve of its timer, one a `renderFrame`, so a spike frame of
   * several views spans several: this frame owns those after the previous frame's last.
   */
  readonly engineFrame: number;
  readonly scriptTimeS: number;
  /** The `requestAnimationFrame` timestamp, ms. */
  readonly rafTimestampMs: number;
  /**
   * The frame callback's start, `performance.now()` ms: its `spike.frame` span's start, which
   * the trace carries as the span's `args.startTime`.
   */
  readonly callbackStartMs: number;
  /** The main thread's time in the frame callback, engine submission included, ms. */
  readonly callbackMs: number;
  /** Passes the frame submitted. */
  readonly passesSubmitted: number;
  /** Patches selected this frame under the hard bound and under min(hard, 4σ). */
  readonly patchesHard: number;
  readonly patchesCalibrated: number;
  /** Whether `TERRAIN: STREAMING` shows. */
  readonly streaming: boolean;
}

/** A patch event's kind (Design note 18: requested, baked, made resident). */
export type PatchEvent = "requested" | "baked" | "resident";

/** What {@link SpikeMetrics} is told at its start. */
export interface SpikeMetricsOptions {
  readonly warmupS: number;
  readonly segments: ReadonlyArray<SpikeSegmentSpan>;
  readonly levels: DescentSpikeReport["levels"];
  /** Which of Design note 21's GPU rows a pass label counts towards. */
  readonly rowOf: (label: string) => SpikePassRow;
  /** The predicted demand of a segment, a second, under each bound (Design note 19, T13.a). */
  readonly predicted: (segment: string) => SegmentPrediction;
  /**
   * The engine's last timing frame number before the run's first frame: earlier times (the
   * materials' warm-up, a mip build) belong to no frame. 0 by default.
   */
  readonly firstEngineFrame?: number;
}

/** A segment's predicted patch demand under the hard bound and under min(hard, 4σ). */
export interface SegmentPrediction {
  readonly hardPerS: number;
  readonly calibratedPerS: number;
}

/** What the report takes from outside the metrics: the shim's, the tally's and the view's. */
export interface SpikeReportExtras {
  readonly latePipelines: ReadonlyArray<SpikeLatePipeline>;
  /** The adapter's peak bytes, from T11.a's `allocationTally`. */
  readonly adapterPeakBytes: number;
  readonly canvas: DescentSpikeReport["canvas"];
}

/**
 * The report as the metrics give it: all of it but where script time starts, the trace's windows
 * and their boundary guard, which the run's control adds (`SpikeController`).
 */
export type SpikeMetricsReport = Omit<
  DescentSpikeReport,
  "scriptStartMs" | "traceWindows" | "traceGuardS"
>;

/** The timer states from best to worst; a run reports the worst it saw. */
const TIMER_RANK: Readonly<Record<GpuTimer, number>> = { full: 0, quantized: 1, absent: 2 };

/** Per-segment tallies. */
interface SegmentTally {
  requested: number;
  baked: number;
  resident: number;
  patchesHard: number;
  patchesCalibrated: number;
  frames: number;
  streamingS: number;
}

/** Gathers a run's figures, frame by frame, into a {@link SpikeMetricsReport}. */
export class SpikeMetrics {
  readonly #options: SpikeMetricsOptions;
  readonly #scriptTimesS: number[] = [];
  readonly #rafIntervalsMs: number[] = [];
  readonly #ourCodeMs: number[] = [];
  readonly #callbackStartsMs: number[] = [];
  /** Each frame's last engine frame number, ascending. */
  readonly #lastEngineFrame: number[] = [];
  readonly #passes = new Map<string, Array<number | null>>();
  readonly #segments = new Map<string, SegmentTally>();
  #lastRafMs: number | undefined;
  #lastScriptS: number | undefined;
  #timer: GpuTimer = "absent";
  /** Whether any pass times have arrived. */
  #timed = false;
  #untimedPasses = 0;
  #uploadBytes = 0;

  constructor(options: SpikeMetricsOptions) {
    this.#options = options;
    for (const { name } of options.segments) {
      this.#segments.set(name, {
        requested: 0,
        baked: 0,
        resident: 0,
        patchesHard: 0,
        patchesCalibrated: 0,
        frames: 0,
        streamingS: 0,
      });
    }
  }

  /** The segment at a script time, or `undefined` past the script's end. */
  segmentAt(scriptTimeS: number): string | undefined {
    return this.#options.segments.find(
      ({ startS, endS }) => scriptTimeS >= startS && scriptTimeS < endS,
    )?.name;
  }

  /** Records one frame. */
  frame(sample: FrameSample): void {
    this.#lastEngineFrame.push(sample.engineFrame);
    this.#scriptTimesS.push(sample.scriptTimeS);
    this.#rafIntervalsMs.push(
      this.#lastRafMs === undefined ? 0 : sample.rafTimestampMs - this.#lastRafMs,
    );
    this.#ourCodeMs.push(sample.callbackMs);
    this.#callbackStartsMs.push(sample.callbackStartMs);
    for (const series of this.#passes.values()) {
      series.push(null);
    }
    this.#untimedPasses += Math.max(0, sample.passesSubmitted - TIMED_PASSES_A_FRAME);
    const tally = this.#tallyAt(sample.scriptTimeS);
    if (tally !== undefined) {
      tally.frames += 1;
      tally.patchesHard += sample.patchesHard;
      tally.patchesCalibrated += sample.patchesCalibrated;
    }
    if (sample.streaming && this.#lastScriptS !== undefined) {
      this.#addStreaming(this.#lastScriptS, sample.scriptTimeS);
    }
    this.#lastRafMs = sample.rafTimestampMs;
    this.#lastScriptS = sample.scriptTimeS;
  }

  /** Records a frame's pass times, which R01 delivers after the frame. */
  passTimes(times: PassTimes): void {
    if (!this.#timed || TIMER_RANK[times.timer] > TIMER_RANK[this.#timer]) {
      this.#timer = times.timer;
    }
    this.#timed = true;
    const index = this.#frameOfEngine(times.frame);
    if (index === undefined) {
      return;
    }
    for (const { label, ns } of times.passes) {
      let series = this.#passes.get(label);
      if (series === undefined) {
        series = Array.from({ length: this.#scriptTimesS.length }, () => null);
        this.#passes.set(label, series);
      }
      // A label a frame times twice (the two instruments' `view:wireframe`) is summed.
      series[index] = (series[index] ?? 0) + ns / 1e6;
    }
  }

  /** The frame owning an engine frame number: the first whose last is at or after it. */
  #frameOfEngine(engineFrame: number): number | undefined {
    const lasts = this.#lastEngineFrame;
    if (engineFrame <= (this.#options.firstEngineFrame ?? 0) || lasts.length === 0) {
      return undefined;
    }
    let lo = 0;
    let hi = lasts.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if ((lasts[mid] ?? Infinity) < engineFrame) {
        lo = mid + 1;
      } else {
        hi = mid;
      }
    }
    return lo < lasts.length ? lo : undefined;
  }

  /** Records an engine allocation event; uploads are tallied. */
  allocation(event: AllocationEvent): void {
    if (event.kind === "uploaded") {
      this.#uploadBytes += event.bytes;
    }
  }

  /** Records patch events at a script time. */
  patches(kind: PatchEvent, count: number, scriptTimeS: number): void {
    const tally = this.#tallyAt(scriptTimeS);
    if (tally !== undefined) {
      tally[kind] += count;
    }
  }

  /** Shares the streaming interval `[fromS, toS)` among the segments it overlaps. */
  #addStreaming(fromS: number, toS: number): void {
    for (const { name, startS, endS } of this.#options.segments) {
      const overlapS = Math.min(toS, endS) - Math.max(fromS, startS);
      const tally = this.#segments.get(name);
      if (overlapS > 0 && tally !== undefined) {
        tally.streamingS += overlapS;
      }
    }
  }

  #tallyAt(scriptTimeS: number): SegmentTally | undefined {
    const name = this.segmentAt(scriptTimeS);
    return name === undefined ? undefined : this.#segments.get(name);
  }

  /** The run's report. */
  report(extra: SpikeReportExtras): SpikeMetricsReport {
    const streaming: SpikeStreamingSegment[] = this.#options.segments.map(
      ({ name, startS, endS }) => {
        const tally = this.#segments.get(name);
        const seconds = Math.max(endS - startS, Number.EPSILON);
        const frames = Math.max(tally?.frames ?? 0, 1);
        const predicted = this.#options.predicted(name);
        return {
          segment: name,
          requestedPerS: (tally?.requested ?? 0) / seconds,
          bakedPerS: (tally?.baked ?? 0) / seconds,
          residentPerS: (tally?.resident ?? 0) / seconds,
          predictedHardPerS: predicted.hardPerS,
          predictedCalibratedPerS: predicted.calibratedPerS,
          patchesHard: (tally?.patchesHard ?? 0) / frames,
          patchesCalibrated: (tally?.patchesCalibrated ?? 0) / frames,
          streamingS: tally?.streamingS ?? 0,
        };
      },
    );
    return {
      warmupS: this.#options.warmupS,
      segments: this.#options.segments,
      levels: this.#options.levels,
      timer: this.#timer,
      untimedPasses: this.#untimedPasses,
      frames: {
        scriptTimesS: [...this.#scriptTimesS],
        rafIntervalsMs: [...this.#rafIntervalsMs],
        ourCodeMs: [...this.#ourCodeMs],
        callbackStartsMs: [...this.#callbackStartsMs],
        passes: [...this.#passes].map(([label, gpuMs]) => ({
          label,
          row: this.#options.rowOf(label),
          gpuMs: [...gpuMs],
        })),
      },
      streaming,
      uploadBytes: this.#uploadBytes,
      latePipelines: extra.latePipelines,
      adapterPeakBytes: extra.adapterPeakBytes,
      canvas: extra.canvas,
    };
  }
}
