/**
 * The resolution controller (plan R07, T18; Design note 14): it holds the photorealistic view's
 * internal scale between the setting's bounds so that the frame meets its budget while
 * instruments are open.
 *
 * @remarks
 * It reads the GPU time of the frame's passes from R01's `onPassTimes` ({@link gpuTimeMs}) where
 * `GraphicsStatus.timer` is not `absent`, and the frame interval otherwise.
 *
 * - **From GPU time** the frame's cost is taken to grow as the square of the scale, all of it
 *   pixels. The scale drops once two of the last four frames are over the budget, to where the
 *   worst of them would take {@link CONTROLLER_TUNING}'s `aim` of it. It rises once a run of frames
 *   has stayed under `raiseBelow` of the budget, to where the worst of the run would take `aim`.
 *   Fixed costs make a frame cheaper than the square predicts after a rise and dearer after a
 *   drop, so a rise never overshoots (it may land short of the top) and a drop may take a second
 *   step. Between `raiseBelow` and the budget the scale holds: that band, about 25% wide, is far
 *   wider than the timer's 65,536 ns quantum, ±0.4% of a 16 ms frame for each timestamp.
 * - **From the frame interval**, which says only whether a frame met its vsync, a frame is missed
 *   when its interval is over 1.5 periods (R05 Design note 21's missed frame). Two misses in four
 *   frames drop the scale by a fixed step; a long run with none raises it by a small one, a probe,
 *   and a probe that misses a frame returns at once to the scale before it.
 *
 * Hysteresis: after each change the controller ignores a few updates, since pass times resolve a
 * frame or more late and the new scale takes effect on the next frame. A rise followed by a drop
 * within `raiseWatchFactor` times the first wait failed: the next rise waits twice as long (up to
 * `holdGrowthLimit` times the first wait), and no drop after a failed rise goes above the scale
 * before it. A rise that holds through the watch resets the wait. A load that keeps returning over
 * the budget therefore settles rather than cycles. Runs and windows count frames measured (one an
 * update from the interval, none to two from GPU time); the settles and the watch count updates,
 * one per frame the view draws.
 */

import type { PassTimes } from "../engine/types";

/** The least and greatest internal scale, as `ViewSettings.internalScaleBounds` gives them. */
export type ScaleBounds = readonly [min: number, max: number];

/** What the controller holds a frame to (from the view's budget, `viewBudgets`). */
export interface ResolutionTarget {
  /** The view's frame period, ms: 1,000 ÷ its rate. */
  readonly periodMs: number;
  /** The GPU time the frame's passes may take, ms, every view's on the device included. */
  readonly gpuBudgetMs: number;
}

/** The controller's tuning, as fractions of the budget and counts of updates or frames. */
export const CONTROLLER_TUNING = {
  /** The fraction of the budget above which a frame is over it. */
  dropAbove: 1,
  /** The fraction of the budget below which a run of frames lets the scale rise. */
  raiseBelow: 0.75,
  /** The fraction of the budget a change aims the worst frame at. */
  aim: 0.9,
  /** The last frames a drop looks at. */
  dropWindow: 4,
  /** The frames over the budget, or missed, among the last `dropWindow` that drop the scale. */
  dropCount: 2,
  /** Updates ignored after a change from GPU time, whose pass times resolve a frame or so late. */
  settleUpdates: 6,
  /** Updates ignored after a change from the interval, which reports the frame just presented. */
  probeSettleUpdates: 2,
  /** The first run of frames under `raiseBelow` that raises the scale: 0.5 s at 60 Hz. */
  raiseHoldFrames: 30,
  /** The first run of frames without a miss that probes upward from the interval: 2 s at 60 Hz. */
  probeHoldFrames: 120,
  /** How far failed rises lengthen the wait: at most this many times the first. */
  holdGrowthLimit: 16,
  /** How long after a rise, in updates, a drop counts as its failure: this many first runs. */
  raiseWatchFactor: 8,
  /** A frame interval over this many periods is a missed frame (R05 Design note 21). */
  missedPeriods: 1.5,
  /** The scale's factor at a drop from missed frames. */
  intervalDrop: 0.85,
  /** The scale's factor at a probe. */
  intervalRaise: 1.1,
} as const;

/** The input an update uses: the GPU's time, or the frame interval where there is none. */
type Source = "gpu" | "interval";

/**
 * The GPU time of one frame's passes, ms: the sum over the resolves of R01's `onPassTimes` that
 * belong to the frame.
 *
 * @remarks
 * Each view's and target's render resolves its own, and `PassTimes.frame` counts resolves, not
 * animation frames, so the caller groups a frame's resolves (as the descent spike's metrics do)
 * and calls this once the last of them has arrived.
 */
export function gpuTimeMs(times: ReadonlyArray<PassTimes>): number {
  let ns = 0;
  for (const resolve of times) {
    for (const pass of resolve.passes) {
      ns += pass.ns;
    }
  }
  return ns / 1e6;
}

/**
 * Holds a view's internal scale between its bounds from the frames' GPU time, or their interval.
 *
 * @example
 * ```ts
 * const control = new ResolutionController(budget.control.bounds, budget.control.target);
 * // Each frame the view draws: the GPU times of the frames whose pass times finished resolving
 * // since the last update, each frame once, or `undefined` where `GraphicsStatus.timer` is
 * // `absent`:
 * const scale = control.update(timer === "absent" ? undefined : resolvedFramesMs, intervalMs);
 * ```
 */
export class ResolutionController {
  readonly #bounds: ScaleBounds;
  #target: ResolutionTarget;
  #scale: number;
  #source: Source | null = null;
  /** Whether each of the last few frames was over the budget or missed, and its GPU time, ms. */
  #recent: Array<{ readonly over: boolean; readonly ms: number }> = [];
  /** Frames in the current run under `raiseBelow` (GPU) or without a miss (interval). */
  #run = 0;
  /** The worst GPU time of the run, ms. */
  #runMaxMs = 0;
  #settle = 0;
  /** How many times failed rises have doubled the wait before the next. */
  #doublings = 0;
  /** Updates since the last rise, while it is watched; `null` when no rise is. */
  #sinceRaise: number | null = null;
  #beforeRaise = 0;

  /**
   * @param bounds - `ViewSettings.internalScaleBounds` or a narrower pair: 0 < min ≤ max.
   * @param target - The view's budget; the scale starts at the bounds' max.
   * @throws RangeError for bounds or a target that are not finite and positive, or min > max.
   */
  constructor(bounds: ScaleBounds, target: ResolutionTarget) {
    const [min, max] = bounds;
    if (!(Number.isFinite(min) && Number.isFinite(max) && min > 0 && min <= max)) {
      throw new RangeError(
        `internal scale bounds must satisfy 0 < min <= max, not [${min}, ${max}]`,
      );
    }
    checkTarget(target);
    this.#bounds = [min, max];
    this.#target = target;
    this.#scale = max;
  }

  /** The internal scale, within the bounds. */
  get scale(): number {
    return this.#scale;
  }

  /**
   * Gives the controller a new budget (an instrument opened or closed, the setting changed, a
   * measured vsync period): it keeps the scale and starts its windows and its waits again, since
   * the frames it holds and the rises that failed were judged against the old budget. A target
   * equal to the present one changes nothing; a caller tracking a measured period retargets only
   * when its estimate moves (rounded, say), since every new target restarts the windows.
   *
   * @throws RangeError for a target that is not finite and positive.
   */
  retarget(target: ResolutionTarget): void {
    checkTarget(target);
    if (
      target.periodMs === this.#target.periodMs &&
      target.gpuBudgetMs === this.#target.gpuBudgetMs
    ) {
      return;
    }
    this.#target = target;
    this.#doublings = 0;
    this.#sinceRaise = null;
    this.#restartWindows();
  }

  /**
   * Takes one drawn frame's measurements and returns the internal scale for the next frame.
   *
   * @param gpuFramesMs - The GPU time, ms ({@link gpuTimeMs}), of each frame whose pass times
   *   finished resolving since the previous update, oldest first, each frame given once (none on
   *   some updates, two on others, since times resolve a frame or more late); or `undefined`
   *   where `GraphicsStatus.timer` is `absent`, in which case the interval is used. Where GPU
   *   times are given the interval is not read: a missed frame whose GPU time is within the
   *   budget is not the resolution's to mend. A time that is not finite or is negative is ignored.
   * @param intervalMs - The time since the view's previous frame, ms; ignored unless positive.
   * @returns The scale, a fraction of the view's render resolution on each axis.
   */
  update(gpuFramesMs: ReadonlyArray<number> | undefined, intervalMs: number): number {
    const source: Source = gpuFramesMs === undefined ? "interval" : "gpu";
    if (source !== this.#source) {
      this.#source = source;
      this.#doublings = 0;
      this.#sinceRaise = null;
      this.#restartWindows();
    }
    this.#watchRaise(source);
    if (this.#settle > 0) {
      // Frames resolving now were drawn at, or just after, the previous scale.
      this.#settle -= 1;
      return this.#scale;
    }
    if (gpuFramesMs === undefined) {
      if (Number.isFinite(intervalMs) && intervalMs > 0) {
        this.#fromInterval(intervalMs);
      }
      return this.#scale;
    }
    for (const ms of gpuFramesMs) {
      // After a change, the frames still to come were drawn at the old scale.
      if (Number.isFinite(ms) && ms >= 0 && this.#fromGpu(ms)) {
        break;
      }
    }
    return this.#scale;
  }

  /** Takes one frame's GPU time; returns whether the scale changed. */
  #fromGpu(ms: number): boolean {
    const t = CONTROLLER_TUNING;
    const budget = this.#target.gpuBudgetMs;
    if (this.#push(ms > t.dropAbove * budget, ms)) {
      const worst = Math.max(...this.#recent.map((frame) => frame.ms));
      let next = this.#scale * Math.sqrt((t.aim * budget) / worst);
      if (this.#sinceRaise !== null) {
        next = Math.min(next, this.#beforeRaise);
      }
      return this.#drop(next, "gpu");
    }
    if (ms < t.raiseBelow * budget) {
      this.#run += 1;
      this.#runMaxMs = Math.max(this.#runMaxMs, ms);
    } else {
      this.#run = 0;
      this.#runMaxMs = 0;
    }
    if (this.#run >= this.#hold("gpu") && this.#scale < this.#bounds[1]) {
      // A run of empty frames (0 ms) has no cost to scale from: rise to the top.
      const ratio = this.#runMaxMs > 0 ? Math.sqrt((t.aim * budget) / this.#runMaxMs) : Infinity;
      return this.#raise(this.#scale * ratio, "gpu");
    }
    return false;
  }

  #fromInterval(intervalMs: number): void {
    const t = CONTROLLER_TUNING;
    const missed = intervalMs > t.missedPeriods * this.#target.periodMs;
    // A probe that misses a frame failed: back to the scale that met every vsync before it.
    if (missed && this.#sinceRaise !== null) {
      this.#drop(this.#beforeRaise, "interval");
      return;
    }
    if (this.#push(missed, 0)) {
      this.#drop(this.#scale * t.intervalDrop, "interval");
      return;
    }
    this.#run = missed ? 0 : this.#run + 1;
    if (this.#run >= this.#hold("interval") && this.#scale < this.#bounds[1]) {
      this.#raise(this.#scale * t.intervalRaise, "interval");
    }
  }

  /** The run a rise waits for: the first wait, doubled by each failed rise, up to the limit. */
  #hold(source: Source): number {
    return baseHold(source) * Math.min(2 ** this.#doublings, CONTROLLER_TUNING.holdGrowthLimit);
  }

  /** Records a frame; returns whether the recent frames call for a drop. */
  #push(over: boolean, ms: number): boolean {
    const t = CONTROLLER_TUNING;
    this.#recent.push({ over, ms });
    if (this.#recent.length > t.dropWindow) {
      this.#recent.shift();
    }
    return this.#recent.filter((frame) => frame.over).length >= t.dropCount;
  }

  #drop(scale: number, source: Source): boolean {
    if (this.#sinceRaise !== null) {
      // A drop soon after a rise is the rise's failure: the next rise waits longer.
      if (2 ** this.#doublings < CONTROLLER_TUNING.holdGrowthLimit) {
        this.#doublings += 1;
      }
      this.#sinceRaise = null;
    }
    return this.#change(scale, source);
  }

  #raise(scale: number, source: Source): boolean {
    const before = this.#scale;
    const changed = this.#change(scale, source);
    if (changed) {
      this.#sinceRaise = 0;
      this.#beforeRaise = before;
    }
    return changed;
  }

  /** Moves to a scale within the bounds; returns whether it changed. */
  #change(scale: number, source: Source): boolean {
    const [min, max] = this.#bounds;
    const next = Math.min(max, Math.max(min, scale));
    if (next === this.#scale) {
      return false;
    }
    this.#scale = next;
    this.#restartWindows();
    this.#settle =
      source === "interval"
        ? CONTROLLER_TUNING.probeSettleUpdates
        : CONTROLLER_TUNING.settleUpdates;
    return true;
  }

  /** Counts the updates since a rise; one that held through the watch resets the wait. */
  #watchRaise(source: Source): void {
    if (this.#sinceRaise === null) {
      return;
    }
    this.#sinceRaise += 1;
    if (this.#sinceRaise > CONTROLLER_TUNING.raiseWatchFactor * baseHold(source)) {
      this.#sinceRaise = null;
      this.#doublings = 0;
    }
  }

  #restartWindows(): void {
    this.#recent = [];
    this.#run = 0;
    this.#runMaxMs = 0;
  }
}

/** The first run of frames a rise waits for, for each source. */
function baseHold(source: Source): number {
  return source === "gpu" ? CONTROLLER_TUNING.raiseHoldFrames : CONTROLLER_TUNING.probeHoldFrames;
}

function checkTarget(target: ResolutionTarget): void {
  const { periodMs, gpuBudgetMs } = target;
  if (!(
    Number.isFinite(periodMs) &&
    periodMs > 0 &&
    Number.isFinite(gpuBudgetMs) &&
    gpuBudgetMs > 0
  )) {
    throw new RangeError(
      `a resolution target needs a positive period and GPU budget, ` +
        `not ${periodMs} and ${gpuBudgetMs} ms`,
    );
  }
}
