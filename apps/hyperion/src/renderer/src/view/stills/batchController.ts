/**
 * The batch sizing of a still image job (plan R11, R11.T10.a, Design note 17): how much of the
 * job's work each animation frame submits, so that no batch holds the GPU long enough to stall the
 * live view.
 *
 * @remarks
 * Pure arithmetic over times fed in: no GPU, no clock, no module state. A still's job is a queue of
 * **units**, each one whole pass (timestamps exist only at pass boundaries), named by the label its
 * cost is learnt under. {@link BatchController.nextBatch} takes the latest measurement and proposes
 * the next batch, a run of the queue's units. With timestamps (`GpuTimer` `quantized` or `full`) it
 * predicts each unit from its label's measured pass time and fills the batch to the cap less two
 * quanta. Without them (`absent`) it estimates the last batch's GPU time as its submit round trip
 * less the empty-submit round trip, takes the cost per unit as that over the batch's units, and
 * grows the batch by at most 1.5 times a step, halving it after an overrun.
 *
 * Calling the engine, gating each batch on `onSubmittedWorkDone()` and the suspension rule after a
 * device loss are R11.T10.b's (`stillJob.ts`); `maxUnits` is what the loss rule's growth cap sets.
 */

import type { GpuTimer } from "../engine/status";
import type { QualitySetting } from "../quality/qualitySetting";

/**
 * The timestamp quantum, ns: Dawn rounds every timestamp to a multiple of 65,536 ns in shipping
 * launches (plan R01, Design note 4, found by probe on 2026-09-29: "every timestamp is a multiple
 * of 65,536 ns"; `GpuTimer`'s `quantized`). Defined here rather than under `engine/`.
 */
export const TIMESTAMP_QUANTUM_NS = 65_536;

/**
 * The margin a batch sized from timestamps keeps below its cap, in quanta: two (Design note 17,
 * researched 2026-09-29). It is kept under `full` timestamps too, the conservative reading.
 */
const TIMESTAMP_MARGIN_QUANTA = 2;

/**
 * The low setting's batch cap, ns: about 6 ms of GPU time on the UHD 620's low setting (Design note
 * 17, researched 2026-09-29; the roadmap's Brainstorm corrections, "Still images"). It is a fixed
 * figure that does not scale with the frame interval.
 */
const LOW_BATCH_CAP_NS = 6_000_000;

/**
 * The target's batch cap as a fraction of the measured frame interval: a quarter of the frame
 * (Design note 17), the display's measured vsync period T, not 1 ÷ 60 s (main's ruling (a) for
 * R11.T10.a, 2026-10-20; the roadmap's gate defines T so).
 */
const TARGET_FRAME_FRACTION = 1 / 4;

/** The most a batch sized without timestamps grows by in a step, ×: 1.5 (Design note 17). */
const GROWTH_FACTOR = 1.5;

/** What a batch sized without timestamps shrinks by after an overrun, ×: ½ (Design note 17). */
const OVERRUN_FACTOR = 1 / 2;

/**
 * The empty submits whose round trips' median is the baseline without timestamps, measured at the
 * job's start: five (Design note 17).
 */
export const EMPTY_SUBMIT_SAMPLES = 5;

/**
 * A batch's cap, ns of GPU time: 6 ms on the low setting, and a quarter of the measured frame
 * interval T on the high setting, the target (Design note 17; main's ruling (a)).
 *
 * @remarks
 * At T = 16.683 ms, the 59.94 Hz projector mode, the high cap is 4.171 ms.
 *
 * @param frameIntervalNs - T, the display's measured vsync period, ns; checked on both settings.
 * @throws RangeError if `frameIntervalNs` is not positive and finite.
 */
export function batchCapNs(setting: QualitySetting, frameIntervalNs: number): number {
  if (!(Number.isFinite(frameIntervalNs) && frameIntervalNs > 0)) {
    throw new RangeError("a frame interval must be positive and finite");
  }
  let capNs: number;
  switch (setting) {
    case "low":
      capNs = LOW_BATCH_CAP_NS;
      break;
    case "high":
      capNs = frameIntervalNs * TARGET_FRAME_FRACTION;
      break;
  }
  return capNs;
}

/**
 * The GPU time a batch sized from timestamps is filled to, ns: {@link batchCapNs} less two
 * {@link TIMESTAMP_QUANTUM_NS} quanta (Design note 17), 5.868928 ms on the low setting.
 *
 * @throws RangeError as {@link batchCapNs} does.
 */
export function timestampBudgetNs(setting: QualitySetting, frameIntervalNs: number): number {
  return batchCapNs(setting, frameIntervalNs) - TIMESTAMP_MARGIN_QUANTA * TIMESTAMP_QUANTUM_NS;
}

/**
 * The most GPU time a pass measured at `measuredNs` can have taken, ns, under `timer`.
 *
 * @remarks
 * A `quantized` pass time is the difference of two timestamps, each a multiple of the quantum q, so
 * it is off the pass's true time by less than one quantum either way: the true time is under
 * measured + q, which is what this returns, never 0. A `full` pass time is taken as it is.
 *
 * @throws RangeError if `measuredNs` is negative or not finite.
 */
export function passTimeBoundNs(measuredNs: number, timer: Exclude<GpuTimer, "absent">): number {
  if (!(Number.isFinite(measuredNs) && measuredNs >= 0)) {
    throw new RangeError("a pass time must be finite and not negative");
  }
  let boundNs: number;
  switch (timer) {
    case "quantized":
      boundNs = measuredNs + TIMESTAMP_QUANTUM_NS;
      break;
    case "full":
      boundNs = measuredNs;
      break;
  }
  return boundNs;
}

/**
 * The median of `samplesNs`, ns: the middle value, or the mean of the two middle values of an even
 * count. Without timestamps, the median of {@link EMPTY_SUBMIT_SAMPLES} empty submits' round trips
 * is the baseline a batch's round trip is taken less (Design note 17).
 *
 * @throws RangeError if there are no samples or one is not finite.
 */
export function medianNs(samplesNs: ReadonlyArray<number>): number {
  if (samplesNs.length === 0 || !samplesNs.every((ns) => Number.isFinite(ns))) {
    throw new RangeError("a median needs finite samples");
  }
  const sorted = samplesNs.toSorted((a, b) => a - b);
  const upper = sorted[Math.floor(sorted.length / 2)] ?? Number.NaN;
  if (sorted.length % 2 === 1) {
    return upper;
  }
  const lower = sorted[sorted.length / 2 - 1] ?? Number.NaN;
  return (lower + upper) / 2;
}

/** How the job's GPU time is measured: by pass timestamps, or by submit round trips. */
export type BatchTiming =
  | { readonly timer: Exclude<GpuTimer, "absent"> }
  | {
      readonly timer: "absent";
      /**
       * {@link EMPTY_SUBMIT_SAMPLES} round trips of an empty submit, ns, measured at the job's
       * start (Design note 17); their median is the baseline.
       */
      readonly emptyRoundTripsNs: ReadonlyArray<number>;
    };

/** What a {@link BatchController} sizes the batches of one still for. */
export interface BatchControllerOptions {
  readonly setting: QualitySetting;
  /** T, the display's measured vsync period, ns ({@link batchCapNs}). */
  readonly frameIntervalNs: number;
  /** From `GraphicsStatus.timer`, for the whole job. */
  readonly timing: BatchTiming;
  /**
   * The job's units in order, each one whole pass, by the label its cost is learnt under. Units of
   * one label are predicted at one cost, so a label names the pass and the tile: a tile of sky
   * costs less than one of ground.
   */
  readonly units: ReadonlyArray<string>;
  /**
   * The most units a batch may hold, a positive integer; absent, no limit. R11.T10.b's re-request
   * after a device loss sets it to half the batch that was in flight.
   */
  readonly maxUnits?: number;
}

/** One pass's measured GPU time: an entry of R01's `PassTimes.passes`. */
export interface MeasuredPass {
  readonly label: string;
  readonly ns: number;
}

/**
 * What arrived since the last {@link BatchController.nextBatch}:
 *
 * - `none`: nothing, as at the job's start, or while a batch's pass times have yet to resolve;
 * - `pass-times`: with timestamps, the passes resolved since the last call, of any earlier batches;
 *   passes whose labels are not the job's, and negative times, are ignored;
 * - `round-trip`: without timestamps, the last batch's submit round trip, ns, from its submit to
 *   its `onSubmittedWorkDone()`.
 */
export type BatchMeasurement =
  | { readonly kind: "none" }
  | { readonly kind: "pass-times"; readonly passes: ReadonlyArray<MeasuredPass> }
  | { readonly kind: "round-trip"; readonly roundTripNs: number };

/** A batch the controller proposes: the units [first, first + count) of the job's queue. */
export interface Batch {
  readonly first: number;
  /** At least 1. */
  readonly count: number;
  /** The batch's predicted GPU time, ns, or `null` while a unit's cost has not been measured. */
  readonly predictedNs: number | null;
  /** The GPU time the batch was sized to, ns: {@link BatchController.budgetNs}. */
  readonly budgetNs: number;
  /**
   * Whether the batch is a single unit predicted over `budgetNs`: it cannot be smaller, so it is
   * proposed, and flagged rather than planned silently.
   */
  readonly overBudget: boolean;
}

/** The most whole units of `costNs` each that fit in `budgetNs`; unbounded at no cost. */
function unitsWithin(budgetNs: number, costNs: number): number {
  if (!(costNs > 0)) {
    return Number.POSITIVE_INFINITY;
  }
  const units = Math.floor(budgetNs / costNs);
  // The division can round up onto an integer: keep the product within the budget.
  return units * costNs > budgetNs ? units - 1 : units;
}

/**
 * The most units a batch sized without timestamps may hold after a batch of `units`: half as many
 * after an overrun, rounded down; otherwise 1.5 times as many, rounded down, or two after one,
 * since whole units cannot grow from one by less than double.
 */
function sizeAfter(units: number, overrun: boolean): number {
  if (overrun) {
    return Math.floor(units * OVERRUN_FACTOR);
  }
  return units < 2 ? 2 : Math.floor(units * GROWTH_FACTOR);
}

/**
 * Sizes the batches of one still's job (Design note 17), one call per animation frame.
 *
 * @remarks
 * A class, since the sizing is a function of everything measured so far: with timestamps, each
 * label's latest pass time; without, the last batch's size and cost per unit.
 *
 * **With timestamps.** Each unit is predicted at its label's latest measured time, the largest of a
 * measurement's passes of that label, bounded as {@link passTimeBoundNs} does: under `quantized`,
 * measured + q per pass, so that the prediction bounds the batch's true time whatever the
 * quantisation, and the two-quantum margin is kept below the cap for what no timestamp covers (the
 * variation between measurements and the gaps between passes). A batch is the longest run of units
 * whose predictions fit {@link timestampBudgetNs}. A unit whose label has not been measured, as at
 * the job's start or while pass times are still resolving, is proposed alone (`predictedNs`
 * `null`), so a batch never grows on missing data; a unit predicted over the budget alone is
 * proposed alone and flagged `overBudget`.
 *
 * **Without timestamps.** The first batch is one unit. Each round trip, less the baseline (the
 * median of the {@link EMPTY_SUBMIT_SAMPLES} empty submits) and at least 0, is the last batch's GPU
 * time g, and g over its units the cost per unit; Design note 17's probe found the estimate over
 * the true time by 1.5–5 ms (provisional), so it cannot undershoot the cap. The next batch is at
 * most ⌊1.5 n⌋ units after a batch of n ≥ 2 (two after a batch of one, the only way whole units can
 * grow from one), at most ⌊n ÷ 2⌋ after an overrun (g over the cap), and never more than fit
 * {@link batchCapNs} at the cost per unit measured. Without a round trip it repeats the last size.
 */
export class BatchController {
  /** The GPU time each batch is sized to, ns: the cap, less two quanta with timestamps. */
  readonly budgetNs: number;
  private readonly timing: BatchTiming;
  private readonly units: ReadonlyArray<string>;
  private readonly labels: ReadonlySet<string>;
  private readonly maxUnits: number;
  /** Without timestamps: the empty-submit baseline, ns. */
  private readonly baselineNs: number;
  /** With timestamps: each label's latest measured pass time, ns. */
  private readonly passNs = new Map<string, number>();
  /** The index of the next unit to propose. */
  private cursor = 0;
  /** Without timestamps: the last batch's units, 0 before the first. */
  private lastCount = 0;
  /** Without timestamps: the latest cost per unit, ns, `null` before any round trip. */
  private costPerUnitNs: number | null = null;

  /**
   * A controller for one still's job, its first batch proposed by the first
   * {@link BatchController.nextBatch}.
   *
   * @throws RangeError if the frame interval is not positive and finite, `maxUnits` is not a
   *   positive integer, or, without timestamps, there are not {@link EMPTY_SUBMIT_SAMPLES} finite,
   *   non-negative round trips.
   */
  constructor(options: BatchControllerOptions) {
    this.timing = options.timing;
    this.units = options.units;
    this.labels = new Set(options.units);
    const maxUnits = options.maxUnits ?? Number.POSITIVE_INFINITY;
    if (options.maxUnits !== undefined && !(Number.isSafeInteger(maxUnits) && maxUnits > 0)) {
      throw new RangeError("a batch's most units must be a positive integer");
    }
    this.maxUnits = maxUnits;
    switch (options.timing.timer) {
      case "quantized":
      case "full":
        this.budgetNs = timestampBudgetNs(options.setting, options.frameIntervalNs);
        this.baselineNs = 0;
        break;
      case "absent": {
        const trips = options.timing.emptyRoundTripsNs;
        if (
          trips.length !== EMPTY_SUBMIT_SAMPLES ||
          !trips.every((ns) => Number.isFinite(ns) && ns >= 0)
        ) {
          throw new RangeError(
            `a baseline needs ${EMPTY_SUBMIT_SAMPLES} finite, non-negative round trips`,
          );
        }
        this.budgetNs = batchCapNs(options.setting, options.frameIntervalNs);
        this.baselineNs = medianNs(trips);
        break;
      }
    }
  }

  /**
   * The next batch, from what was measured since the last call, or `null` once every unit has been
   * proposed.
   *
   * @throws Error if `measured` is of the other timing's kind, or a round trip arrives before any
   *   batch was proposed: both are the caller's bugs.
   * @throws RangeError if a pass time is not finite, or a round trip is negative or not finite.
   */
  nextBatch(measured: BatchMeasurement): Batch | null {
    const timing = this.timing;
    let batch: Batch | null;
    switch (timing.timer) {
      case "quantized":
      case "full":
        batch = this.timedBatch(measured, timing.timer);
        break;
      case "absent":
        batch = this.roundTripBatch(measured);
        break;
    }
    return batch;
  }

  private timedBatch(measured: BatchMeasurement, timer: Exclude<GpuTimer, "absent">): Batch | null {
    switch (measured.kind) {
      case "none":
        break;
      case "pass-times":
        this.learn(measured.passes);
        break;
      case "round-trip":
        throw new Error("a controller timed by timestamps was given a round trip");
    }
    const first = this.cursor;
    const firstLabel = this.units[first];
    if (firstLabel === undefined) {
      return null;
    }
    const firstNs = this.passNs.get(firstLabel);
    if (firstNs === undefined) {
      return this.propose(first, 1, null, false);
    }
    let predictedNs = passTimeBoundNs(firstNs, timer);
    if (predictedNs > this.budgetNs) {
      return this.propose(first, 1, predictedNs, true);
    }
    let count = 1;
    while (count < this.maxUnits) {
      const label = this.units[first + count];
      const ns = label === undefined ? undefined : this.passNs.get(label);
      if (ns === undefined) {
        break;
      }
      const unitNs = passTimeBoundNs(ns, timer);
      if (predictedNs + unitNs > this.budgetNs) {
        break;
      }
      predictedNs += unitNs;
      count += 1;
    }
    return this.propose(first, count, predictedNs, false);
  }

  /**
   * Each of the job's labels takes the largest of its passes in `passes`. A negative pass time,
   * which a reset of the timestamp counter gives, says nothing and is skipped, the label keeping
   * its last time: the WebGPU specification's note on timestamp queries calls such values rare and
   * safe to ignore.
   */
  private learn(passes: ReadonlyArray<MeasuredPass>): void {
    const latest = new Map<string, number>();
    for (const pass of passes) {
      if (!Number.isFinite(pass.ns)) {
        throw new RangeError("a pass time must be finite");
      }
      if (pass.ns >= 0 && this.labels.has(pass.label)) {
        latest.set(pass.label, Math.max(latest.get(pass.label) ?? 0, pass.ns));
      }
    }
    for (const [label, ns] of latest) {
      this.passNs.set(label, ns);
    }
  }

  private roundTripBatch(measured: BatchMeasurement): Batch | null {
    let count: number;
    switch (measured.kind) {
      case "none":
        count = Math.max(this.lastCount, 1);
        break;
      case "pass-times":
        throw new Error("a controller timed by round trips was given pass times");
      case "round-trip": {
        if (this.lastCount === 0) {
          throw new Error("a round trip arrived before any batch was proposed");
        }
        const { roundTripNs } = measured;
        if (!(Number.isFinite(roundTripNs) && roundTripNs >= 0)) {
          throw new RangeError("a round trip must be finite and not negative");
        }
        const gpuNs = Math.max(0, roundTripNs - this.baselineNs);
        const costNs = gpuNs / this.lastCount;
        this.costPerUnitNs = costNs;
        const target = sizeAfter(this.lastCount, gpuNs > this.budgetNs);
        count = Math.max(1, Math.min(target, unitsWithin(this.budgetNs, costNs)));
        break;
      }
    }
    const first = this.cursor;
    count = Math.min(count, this.maxUnits, this.units.length - first);
    if (count < 1) {
      return null;
    }
    this.lastCount = count;
    const costNs = this.costPerUnitNs;
    const predictedNs = costNs === null ? null : count * costNs;
    return this.propose(
      first,
      count,
      predictedNs,
      predictedNs !== null && predictedNs > this.budgetNs,
    );
  }

  private propose(
    first: number,
    count: number,
    predictedNs: number | null,
    overBudget: boolean,
  ): Batch {
    this.cursor = first + count;
    return { first, count, predictedNs, budgetNs: this.budgetNs, overBudget };
  }
}
