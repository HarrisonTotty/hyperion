/**
 * One perf repetition's measurements in the renderer (plan R12, R12.T4.c; Design notes 4, 6, 7 and
 * 13), over R05's metrics (`spike/metrics.ts`): its frames, its passes by role and label to the
 * budget's rows, its timer, its releases and its streaming, which `endRepetition` hands over.
 *
 * @remarks
 * As R05's metrics do, a frame owns the engine's resolves numbered after the frame before it, up to
 * its own last (`FrameSample.engineFrame`, on R05's `ResolveCounter`), and each resolve's
 * `PassTimes` are matched to their frame when they arrive: resolves numbered before the
 * repetition's first frame are the warm-up's or an earlier repetition's and are left out, as are
 * those after its last frame. A pass's percentiles are over the frames that timed it; a row's and
 * the timed passes' sum over the frames whose every resolve reported (a dropped resolve is a gap),
 * each such frame's sum of the row's passes, 0 where none ran. Percentiles are by nearest rank.
 * The timer is the worst any resolve reported, and `quantized` where it reported `full` but every
 * pass time is a whole multiple of 65,536 ns, Dawn's quantum (Design note 4; R12.T4.a's record).
 */

import type {
  BudgetRow,
  PassFigures,
  PassKey,
  PerfTimer,
  RowFigures,
  StreamingFigures,
} from "@hyperion/perf";

import type {
  PerfRepetitionResult,
  PerfTallyRelease,
  SpikeSegmentSpan,
} from "../../../../preload/api";
import type { PassTimes } from "../engine/types";
import type { FrameSample, PatchEvent } from "../spike/metrics";
import { attributePasses, type PassRoles } from "./passRows";

/** A frame as a repetition's metrics read it: the fields of R05's `FrameSample` they need. */
export type PerfFrameSample = Pick<
  FrameSample,
  "engineFrame" | "scriptTimeS" | "rafTimestampMs" | "callbackStartMs" | "callbackMs" | "streaming"
>;

/** Dawn's timestamp quantum, ns (`kTimestampQuantizationMask`, Design note 4). */
export const TIMESTAMP_QUANTUM_NS = 65_536;

/** The least release the kernel's settle window is told of, bytes: 16 MiB (R12.T3.b's record). */
export const RELEASE_MIN_BYTES = 16 * 2 ** 20;

/** The timer states from best to worst; a repetition reports the worst it saw. */
const TIMER_RANK: Readonly<Record<PerfTimer, number>> = { full: 0, quantized: 1, absent: 2 };

/** What a {@link RepetitionMetrics} is told at its start. */
export interface RepetitionMetricsOptions {
  /** The roles the scene's resolves are given (`passRolesOf`). */
  readonly roles: PassRoles;
  /** The script's segments, s from the span's start. */
  readonly segments: ReadonlyArray<SpikeSegmentSpan>;
  /** R05's predicted demand of a segment, patches a second, or `null` where it is not had. */
  readonly demandPerS: (segment: string) => number | null;
  /**
   * The run's resolve count before the repetition's first frame (`ResolveCounter.value`): every
   * resolve numbered at or before it is an earlier span's.
   */
  readonly firstEngineFrame: number;
  /** The `requestAnimationFrame` timestamp of the last frame before the span's, ms, or `null`. */
  readonly previousRafMs: number | null;
}

/** What a repetition's result takes from outside its metrics. */
export interface RepetitionExtras {
  /** The span flown, s. */
  readonly spanS: number;
  /** The page's `performance.timeOrigin`, ms since the epoch. */
  readonly timeOriginMs: number;
  readonly memory: PerfRepetitionResult["memory"];
  /** Pipelines the shim counted after the warm-up within the repetition. */
  readonly latePipelines: number;
  readonly internalScale: PerfRepetitionResult["internalScale"];
}

/** The value at fraction `p` of `sorted` (ascending, not empty), by nearest rank. */
function nearestRank(sorted: ReadonlyArray<number>, p: number): number {
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))] ?? 0;
}

/** The mean and the nearest-rank p50, p95 and p99 of `values`, ms; `null`s for none. */
function figuresOf(values: ReadonlyArray<number>): {
  readonly meanMs: number | null;
  readonly p50Ms: number | null;
  readonly p95Ms: number | null;
  readonly p99Ms: number | null;
} {
  if (values.length === 0) {
    return { meanMs: null, p50Ms: null, p95Ms: null, p99Ms: null };
  }
  const sorted = values.toSorted((a, b) => a - b);
  return {
    meanMs: values.reduce((sum, v) => sum + v, 0) / values.length,
    p50Ms: nearestRank(sorted, 0.5),
    p95Ms: nearestRank(sorted, 0.95),
    p99Ms: nearestRank(sorted, 0.99),
  };
}

/** A pass's GPU time in each frame that timed it, ms, by frame index. */
interface PassSeries {
  readonly row: BudgetRow;
  readonly msByFrame: Map<number, number>;
}

/** Gathers one repetition's figures, frame by frame, into its {@link PerfRepetitionResult}. */
export class RepetitionMetrics {
  readonly #options: RepetitionMetricsOptions;
  readonly #scriptTimesS: number[] = [];
  readonly #intervalsMs: number[] = [];
  readonly #callbackStartsMs: number[] = [];
  readonly #callbackMs: number[] = [];
  /** Each frame's last engine frame number, ascending. */
  readonly #lastEngineFrame: number[] = [];
  /** Each frame's resolves whose pass times have arrived. */
  readonly #reported: number[] = [];
  readonly #passes = new Map<PassKey, PassSeries>();
  readonly #releases: PerfTallyRelease[] = [];
  readonly #resident = new Map<string, number>();
  readonly #streamingS = new Map<string, number>();
  #lastRafMs: number | null;
  #lastScriptS = 0;
  #timer: PerfTimer | null = null;
  /** Whether every pass time so far is a whole multiple of the quantum. */
  #multiples = true;
  /** Whether any pass time so far is not zero. */
  #nonZero = false;

  constructor(options: RepetitionMetricsOptions) {
    this.#options = options;
    this.#lastRafMs = options.previousRafMs;
  }

  /** The frames recorded so far. */
  get frames(): number {
    return this.#scriptTimesS.length;
  }

  /** Records one frame of the span. */
  frame(sample: PerfFrameSample): void {
    this.#intervalsMs.push(
      this.#lastRafMs === null ? 0 : Math.max(0, sample.rafTimestampMs - this.#lastRafMs),
    );
    this.#scriptTimesS.push(sample.scriptTimeS);
    this.#callbackStartsMs.push(sample.callbackStartMs);
    this.#callbackMs.push(Math.max(0, sample.callbackMs));
    this.#lastEngineFrame.push(sample.engineFrame);
    this.#reported.push(0);
    if (sample.streaming && this.#scriptTimesS.length > 1) {
      this.#addStreaming(this.#lastScriptS, sample.scriptTimeS);
    }
    this.#lastRafMs = sample.rafTimestampMs;
    this.#lastScriptS = sample.scriptTimeS;
  }

  /**
   * Records one resolve's pass times, numbered on the run's count (`ResolveCounter.runFrame`): a
   * resolve of no frame of the span is left out.
   *
   * @throws `UnmappedPass` for a pass with no row on its role (Design note 4).
   */
  passTimes(times: PassTimes): void {
    const index = this.#frameOfEngine(times.frame);
    if (index === undefined) {
      return;
    }
    const attributed = attributePasses(times, this.#options.roles);
    if (this.#timer === null || TIMER_RANK[times.timer] > TIMER_RANK[this.#timer]) {
      this.#timer = times.timer;
    }
    this.#reported[index] = (this.#reported[index] ?? 0) + 1;
    for (const { key, row, ns } of attributed) {
      this.#multiples &&= ns % TIMESTAMP_QUANTUM_NS === 0;
      this.#nonZero ||= ns !== 0;
      let series = this.#passes.get(key);
      if (series === undefined) {
        series = { row, msByFrame: new Map() };
        this.#passes.set(key, series);
      }
      // A key a frame times twice (the two instruments' `view:wireframe`) is summed, as R05's is.
      series.msByFrame.set(index, (series.msByFrame.get(index) ?? 0) + ns / 1e6);
    }
  }

  /** Records a patch's progress at the last frame's script time; `resident` alone is counted. */
  patch(event: PatchEvent): void {
    if (event !== "resident") {
      return;
    }
    const segment = this.#segmentAt(this.#lastScriptS);
    if (segment !== undefined) {
      this.#resident.set(segment, (this.#resident.get(segment) ?? 0) + 1);
    }
  }

  /** Records a release of the tally's within the span. */
  release(release: PerfTallyRelease): void {
    this.#releases.push(release);
  }

  /** The repetition's result for `endRepetition`. */
  result(extras: RepetitionExtras): PerfRepetitionResult {
    const complete = this.#completeFrames();
    const passes: PassFigures[] = [];
    const rowsSeen = new Set<BudgetRow>();
    for (const [key, { row, msByFrame }] of this.#passes) {
      rowsSeen.add(row);
      const { p50Ms, p95Ms, p99Ms } = figuresOf([...msByFrame.values()]);
      passes.push({ key, row, frames: msByFrame.size, p50Ms, p95Ms, p99Ms });
    }
    const rows: RowFigures[] = [...rowsSeen].map((row) => {
      const series = [...this.#passes.values()].filter((s) => s.row === row);
      const sums = complete.map((i) =>
        series.reduce((sum, { msByFrame }) => sum + (msByFrame.get(i) ?? 0), 0),
      );
      const { meanMs, p50Ms, p95Ms, p99Ms } = figuresOf(sums);
      return { row, frames: complete.length, meanMs, p50Ms, p95Ms, p99Ms };
    });
    const allSeries = [...this.#passes.values()];
    const timed = figuresOf(
      complete.map((i) =>
        allSeries.reduce((sum, { msByFrame }) => sum + (msByFrame.get(i) ?? 0), 0),
      ),
    );
    return {
      spanS: extras.spanS,
      timeOriginMs: extras.timeOriginMs,
      frames: {
        intervalsMs: [...this.#intervalsMs],
        callbackStartsMs: [...this.#callbackStartsMs],
        callbackMs: [...this.#callbackMs],
        scriptTimesS: [...this.#scriptTimesS],
      },
      segments: this.#options.segments.map(({ name, startS, endS }) => ({ name, startS, endS })),
      passes,
      rows,
      timedPassesPerFrameMs: timed.meanMs,
      timer: this.#timerOf(),
      memory: extras.memory,
      releases: [...this.#releases],
      streaming: this.#streaming(),
      latePipelines: extras.latePipelines,
      heightPointUs: null,
      internalScale: extras.internalScale,
      arrival: null,
    };
  }

  /** The worst timer reported, `quantized` where every time was a multiple of the quantum. */
  #timerOf(): PerfTimer {
    const timer = this.#timer ?? "absent";
    return timer === "full" && this.#multiples && this.#nonZero ? "quantized" : timer;
  }

  /** The indices of the frames whose every resolve reported its pass times. */
  #completeFrames(): ReadonlyArray<number> {
    const complete: number[] = [];
    let before = this.#options.firstEngineFrame;
    this.#lastEngineFrame.forEach((last, i) => {
      const numbered = Math.max(0, last - before);
      before = Math.max(before, last);
      if (numbered > 0 && (this.#reported[i] ?? 0) >= numbered) {
        complete.push(i);
      }
    });
    return complete;
  }

  /** The frame owning an engine frame number: the first whose last is at or after it. */
  #frameOfEngine(engineFrame: number): number | undefined {
    const lasts = this.#lastEngineFrame;
    if (engineFrame <= this.#options.firstEngineFrame || lasts.length === 0) {
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

  #segmentAt(scriptTimeS: number): string | undefined {
    return this.#options.segments.find(
      ({ startS, endS }) => scriptTimeS >= startS && scriptTimeS < endS,
    )?.name;
  }

  /** Shares the streaming interval `[fromS, toS)` among the segments it overlaps, as R05's does. */
  #addStreaming(fromS: number, toS: number): void {
    for (const { name, startS, endS } of this.#options.segments) {
      const overlapS = Math.min(toS, endS) - Math.max(fromS, startS);
      if (overlapS > 0) {
        this.#streamingS.set(name, (this.#streamingS.get(name) ?? 0) + overlapS);
      }
    }
  }

  /** Each segment's patches made resident a second, its predicted demand and its streaming time. */
  #streaming(): ReadonlyArray<StreamingFigures> {
    return this.#options.segments.map(({ name, startS, endS }) => {
      const seconds = endS - startS;
      return {
        segment: name,
        patchesPerS: seconds > 0 ? (this.#resident.get(name) ?? 0) / seconds : null,
        demandPerS: this.#options.demandPerS(name),
        streamingSeconds: this.#streamingS.get(name) ?? 0,
      };
    });
  }
}
