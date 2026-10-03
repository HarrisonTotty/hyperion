/**
 * The descent spike's per-frame measurements in the renderer (plan R05, T14.a, Design note 18),
 * gathered into the report the main process turns into the results file (T14.c's
 * `DescentSpikeReport`).
 *
 * @remarks
 * The renderer keeps raw series and counts; the percentiles, missed frames and hitches are the
 * main process's (`main/results.ts`), so that one convention serves the presentation times from
 * the trace and the series here. Each segment is marked in the trace with one
 * `performance.measure` span named {@link SEGMENT_MEASURE_PREFIX}`<name>`, by which the main
 * process splits presentation intervals.
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

/** The `performance.measure` name prefix of a segment's span (`main/results.ts` reads it). */
export const SEGMENT_MEASURE_PREFIX = "spike.segment:";

/** The passes R01's timer times in a frame; more are untimed (Consumes, "As built"). */
export const TIMED_PASSES_A_FRAME = 64;

/** One frame as the spike's loop sees it. */
export interface FrameSample {
  /** The engine's frame number, which its `PassTimes.frame` carries. */
  readonly engineFrame: number;
  readonly scriptTimeS: number;
  /** The `requestAnimationFrame` timestamp, ms. */
  readonly rafTimestampMs: number;
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
  readonly predicted: (segment: string) => {
    readonly hardPerS: number;
    readonly calibratedPerS: number;
  };
}

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

/** Gathers a run's figures, frame by frame, into a {@link DescentSpikeReport}. */
export class SpikeMetrics {
  readonly #options: SpikeMetricsOptions;
  readonly #scriptTimesS: number[] = [];
  readonly #rafIntervalsMs: number[] = [];
  readonly #ourCodeMs: number[] = [];
  /** The frame index of each engine frame number. */
  readonly #frameOf = new Map<number, number>();
  readonly #passes = new Map<string, Array<number | null>>();
  readonly #segments = new Map<string, SegmentTally>();
  #lastRafMs: number | undefined;
  #lastScriptS: number | undefined;
  #timer: GpuTimer = "absent";
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
    const index = this.#scriptTimesS.length;
    this.#frameOf.set(sample.engineFrame, index);
    this.#scriptTimesS.push(sample.scriptTimeS);
    this.#rafIntervalsMs.push(
      this.#lastRafMs === undefined ? 0 : sample.rafTimestampMs - this.#lastRafMs,
    );
    this.#ourCodeMs.push(sample.callbackMs);
    for (const series of this.#passes.values()) {
      series.push(null);
    }
    this.#untimedPasses += Math.max(0, sample.passesSubmitted - TIMED_PASSES_A_FRAME);
    const tally = this.#tallyAt(sample.scriptTimeS);
    if (tally !== undefined) {
      tally.frames += 1;
      tally.patchesHard += sample.patchesHard;
      tally.patchesCalibrated += sample.patchesCalibrated;
      if (sample.streaming && this.#lastScriptS !== undefined) {
        tally.streamingS += sample.scriptTimeS - this.#lastScriptS;
      }
    }
    this.#lastRafMs = sample.rafTimestampMs;
    this.#lastScriptS = sample.scriptTimeS;
  }

  /** Records a frame's pass times, which R01 delivers after the frame. */
  passTimes(times: PassTimes): void {
    this.#timer = times.timer;
    const index = this.#frameOf.get(times.frame);
    if (index === undefined) {
      return;
    }
    for (const { label, ns } of times.passes) {
      let series = this.#passes.get(label);
      if (series === undefined) {
        series = Array.from({ length: this.#scriptTimesS.length }, () => null);
        this.#passes.set(label, series);
      }
      series[index] = ns / 1e6;
    }
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

  #tallyAt(scriptTimeS: number): SegmentTally | undefined {
    const name = this.segmentAt(scriptTimeS);
    return name === undefined ? undefined : this.#segments.get(name);
  }

  /** The run's report. */
  report(extra: {
    readonly latePipelines: ReadonlyArray<SpikeLatePipeline>;
    readonly adapterPeakBytes: number;
    readonly canvas: DescentSpikeReport["canvas"];
  }): DescentSpikeReport {
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
