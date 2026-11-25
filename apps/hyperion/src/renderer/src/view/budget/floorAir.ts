/**
 * A frame-rate floor's air (the owner's report, 2026-11-01; lane fix-floor-calm): when the floor
 * steps the primary's air to the low setting's sizes, what that step saved, and when its own air
 * comes back.
 *
 * @remarks
 * Under the pass timer:
 *
 * - **Down, the resolution first.** The air steps only once the controller's drop aims below its
 *   bounds' min (`ResolutionController.wantedBelowMin`): its frames are over the budget at the
 *   floor's lowest step, or over it by so much that the drop's own aim passes that step, when it
 *   steps in the same frame as the scale. A milder overload drops the scale alone, as Unity's
 *   Adaptive Performance reduces its scalers of low visual impact first, its resolution's among
 *   them ("changing the resolution has less impact on how the game is perceived visually": the
 *   package's 4.0 user guide, "Visual Impact" and "Adaptive Resolution").
 * - **What the step saved.** The own air's frames that drove the drop, carried to the lowest step
 *   by the controller's square of the scale, against the stepped frames there once they have
 *   settled: their medians' ratio less 1 is the air's cost as a share of a stepped frame,
 *   {@link FloorAir.costRatio}, kept as a ratio so that it carries from one view of a world to
 *   another and across scales. A pair counts only where neither window spreads over
 *   {@link AIR_TUNING}'s `pairSpread`; until one has, the share is `unmeasuredCostRatio`.
 * - **Up, the air first.** Each stepped frame is carried to the lowest step and given the air's
 *   cost, and is over where that is over the controller's `aim` of the budget. The air returns once
 *   `returnFrames` frames in a row have had no two over in four, the controller's own drop rule: a
 *   load whose own air fits at the floor's lowest step, with the controller's 10% to spare, gets it
 *   back at whatever scale it stands, and the controller then raises the scale as far as it holds.
 *   So the air no longer waits for the scale's max, which the M5's orbit never reaches again (its
 *   rise lands at about 0.97, inside the controller's band).
 * - **The guard.** A step within `returnWatchUpdates` of a return is the return's failure: the next
 *   return waits twice as many frames, up to `returnGrowthLimit` times; a return that holds through
 *   the watch resets the wait. A step within `quickFailUpdates` failed on the frames the return was
 *   judged on (lean, 2026-11-19): the square of the scale carries a frame's fixed cost to the lowest
 *   step as if it were pixels, and so under-reads it there. No return is tried again while the
 *   stepped frames drawn at the scale it was tried at (within `failedScaleBand`) stay within
 *   `failedBand` of those it was judged on; two in four heavier by more, as the drop rule takes
 *   frames over the budget, are another view, and lift the hold.
 *   A load at the boundary so changes the air at most once in 30 s, and one whose fixed cost misled
 *   the return fails it once at a scale.
 *
 * A frame is read only once the scale and the air have stood for the controller's
 * `settleUpdates`, since its pass times resolve a frame or more late; until then it was drawn at
 * what they were before. The air returns only in an update that read frames at the scale it stands
 * at and leaves it there, so that its own frames are first drawn at the scale they were judged at.
 *
 * Without a timer the air steps as the scale drops below its max and returns once the scale is back
 * at its max and {@link CALM_FRAMES} intervals in a row are under {@link CALM_INTERVAL_SHARE} of
 * the floor's period, as before.
 */

import { CONTROLLER_TUNING, type ResolutionTarget, type ScaleBounds } from "./resolutionController";

/**
 * Without a timer, the share of the floor's period under which a frame's interval is calm: frames
 * drawn at a 60 Hz display's every vsync, 16.7 ms against the floor's 33.3, are; at its 30 Hz not.
 */
export const CALM_INTERVAL_SHARE = 0.6;

/** Without a timer, the calm intervals in a row a floor's air waits through before it returns. */
export const CALM_FRAMES = 60;

/** The floor's air's tuning under the pass timer (the module's remarks). */
export const AIR_TUNING = {
  /** The frames in a row with no two over in four that a return first waits for: 1 s at 60 Hz. */
  returnFrames: 60,
  /**
   * The air's cost as a share of a stepped frame before a step has measured it: about 1. On the
   * RTX 3080 high's march and composite took 3.45 times low's a pixel (R08's run W5, R05's Earth
   * from 400 km: 7.14 ms at 1920 × 1080 against 0.92 ms at 1280 × 720 with the march at ½), and
   * on the M5 the stepped air is about 44% of a stepped frame's union (the 2026-11-08 Mac batch,
   * high-dpr1: 7.42 of 16.89 ms at orbit, 12.95 of 29.01 closed in), so (3.45 − 1) × 0.44 ≈ 1.1.
   */
  unmeasuredCostRatio: 1,
  /** The largest share a step's measure is taken at. */
  maxCostRatio: 3,
  /** The most a measure's window may spread, its slowest frame over its fastest. */
  pairSpread: 1.5,
  /** The stepped frames at the lowest step a measure needs at least. */
  afterFramesMin: 8,
  /** The stepped frames at the lowest step a measure takes at most. */
  afterFramesMax: 30,
  /** The updates after a return within which a step is its failure: 30 s at 60 Hz. */
  returnWatchUpdates: 1_800,
  /** How far failed returns lengthen the wait: at most this many times the first. */
  returnGrowthLimit: 16,
  /** The updates after a return within which a step failed on its own frames: 0.5 s at 60 Hz. */
  quickFailUpdates: 30,
  /** How near the frames a failed return was judged on the stepped frames hold the next. */
  failedBand: 0.1,
  /** How near the scale a return failed at a frame's is for the hold to read it. */
  failedScaleBand: 0.02,
} as const;

/** The median of `values` (not empty). */
function median(values: ReadonlyArray<number>): number {
  const sorted = values.toSorted((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 === 1
    ? (sorted[mid] ?? 0)
    : ((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2;
}

/** Whether `values`' slowest is within `AIR_TUNING.pairSpread` of their fastest, which is not 0. */
function tight(values: ReadonlyArray<number>): boolean {
  const least = Math.min(...values);
  return least > 0 && Math.max(...values) <= AIR_TUNING.pairSpread * least;
}

/** What one update tells the floor's air: the frames resolved and the controller after its update. */
export interface FloorAirUpdate {
  /**
   * The GPU time of each frame resolved since the last update, ms (`PrimaryFrameTimes.take`),
   * drawn at the scale and the air of before it; `undefined` while the timer is absent.
   */
  readonly framesMs: ReadonlyArray<number> | undefined;
  /** The interval between the primary's last two frames, ms; 0 for the first. */
  readonly intervalMs: number;
  /** The controller's scale for the next frame. */
  readonly scale: number;
  /** `ResolutionController.wantedBelowMin` after its update. */
  readonly wantedBelowMin: boolean;
  readonly bounds: ScaleBounds;
  readonly target: ResolutionTarget;
}

/** A frame-rate floor's air: whether it is stepped, and when it steps and returns (the module's remarks). */
export class FloorAir {
  #stepped = false;
  #costRatio: number = AIR_TUNING.unmeasuredCostRatio;
  /** The scale of the last update, which the next update's frames were drawn at; `null` before one. */
  #lastScale: number | null = null;
  /** Updates since the scale or the air last changed; a frame is read once they reach the settle. */
  #sinceChange: number = CONTROLLER_TUNING.settleUpdates;
  /** The own air's last frames read: each carried to the lowest step, and whether over the budget. */
  #recent: Array<{ readonly atMinMs: number; readonly over: boolean }> = [];
  /** The own air's frames over the budget at the last step, carried to the lowest; `null` for none. */
  #before: ReadonlyArray<number> | null = null;
  /** The stepped frames read at the lowest step since the last step; `null` once measured. */
  #after: number[] | null = null;
  /** Whether each of the last few stepped frames, given the air's cost, was over the line. */
  #window: boolean[] = [];
  /** The last few stepped frames read, ms, and the scale each was drawn at. */
  #judged: Array<{ readonly ms: number; readonly scale: number }> = [];
  /** The scale the last return was tried at and its stepped frames' median there, ms. */
  #returnedAt: { readonly scale: number; readonly ms: number } | null = null;
  /** The same of a return that failed on its own frames, which holds the next back; else `null`. */
  #failedAt: { readonly scale: number; readonly ms: number } | null = null;
  /** Whether each of the last few frames read at the held scale was heavier than its band. */
  #heavier: boolean[] = [];
  /** Whether the last update was without a timer, whose path keeps none of the timer's windows. */
  #onInterval = false;
  /** Stepped frames in a row with no two over in four. */
  #run = 0;
  /** How many times failed returns have doubled the wait. */
  #doublings = 0;
  /** Updates since the last return, while it is watched; `null` when none is. */
  #sinceReturn: number | null = null;
  /** Without a timer, the calm intervals in a row at the scale's max. */
  #calm = 0;

  /** Whether the floor has stepped the view's air to the low setting's sizes. */
  get stepped(): boolean {
    return this.#stepped;
  }

  /** The air's cost as a share of a stepped frame: the last step's measure, or its assumed value. */
  get costRatio(): number {
    return this.#costRatio;
  }

  /**
   * Takes one update of the floor's view and returns whether its air changed under the pass timer,
   * after which the caller restarts its controller (`ResolutionController.restart`): the frames it
   * holds are of the air before. Without a timer it returns `false`, the air changing as before.
   */
  update(update: FloorAirUpdate): boolean {
    if (update.framesMs === undefined) {
      this.#fromInterval(update);
      return false;
    }
    this.#onInterval = false;
    const { framesMs, scale, bounds, target } = update;
    const [min] = bounds;
    const drawnAt = this.#lastScale ?? scale;
    const settled = this.#sinceChange >= CONTROLLER_TUNING.settleUpdates;
    if (settled) {
      for (const ms of framesMs) {
        if (Number.isFinite(ms) && ms >= 0) {
          this.#read(ms, drawnAt, min, target.gpuBudgetMs);
        }
      }
    }
    const scaleChanged = this.#lastScale !== null && scale !== this.#lastScale;
    if (scaleChanged) {
      // The stepped frames at the lowest step end with it.
      this.#measure();
    }
    let changed = false;
    if (!this.#stepped && update.wantedBelowMin) {
      this.#step();
      changed = true;
    } else if (this.#stepped && settled && !scaleChanged && this.#run >= this.#returnFrames()) {
      this.#return();
      changed = true;
    }
    this.#sinceChange = changed || scaleChanged ? 0 : this.#sinceChange + 1;
    this.#lastScale = scale;
    if (this.#sinceReturn !== null && !changed) {
      this.#sinceReturn += 1;
      if (this.#sinceReturn > AIR_TUNING.returnWatchUpdates) {
        // The return held: the next waits only the first wait.
        this.#sinceReturn = null;
        this.#doublings = 0;
      }
    }
    return changed;
  }

  /**
   * Starts every window again, keeping whether the air is stepped and what it costs: for a new
   * controller, whose scale the frames in flight were not drawn at.
   */
  restartWindows(): void {
    this.#clearTimerWindows();
    this.#calm = 0;
  }

  /** Brings the air back at once and starts every window again: for a view with no floor. */
  leave(): void {
    this.#stepped = false;
    this.restartWindows();
  }

  /** Starts the pass timer's windows and waits again, keeping the air and what it costs. */
  #clearTimerWindows(): void {
    this.#lastScale = null;
    this.#sinceChange = 0;
    this.#recent = [];
    this.#before = null;
    this.#after = null;
    this.#window = [];
    this.#judged = [];
    this.#run = 0;
    this.#doublings = 0;
    this.#sinceReturn = null;
    this.#returnedAt = null;
    this.#failedAt = null;
    this.#heavier = [];
  }

  /** Reads one frame drawn at `drawnAt` and the air before this update. */
  #read(ms: number, drawnAt: number, min: number, budgetMs: number): void {
    const atMinMs = ms * (min / drawnAt) ** 2;
    if (!this.#stepped) {
      this.#recent.push({ atMinMs, over: ms > CONTROLLER_TUNING.dropAbove * budgetMs });
      if (this.#recent.length > CONTROLLER_TUNING.dropWindow) {
        this.#recent.shift();
      }
      return;
    }
    if (this.#after !== null && drawnAt === min) {
      this.#after.push(ms);
      if (this.#after.length >= AIR_TUNING.afterFramesMax) {
        this.#measure();
      }
    }
    this.#judged.push({ ms, scale: drawnAt });
    if (this.#judged.length > CONTROLLER_TUNING.dropWindow) {
      this.#judged.shift();
    }
    let over = (1 + this.#costRatio) * atMinMs > CONTROLLER_TUNING.aim * budgetMs;
    const failed = this.#failedAt;
    if (failed !== null && Math.abs(drawnAt / failed.scale - 1) <= AIR_TUNING.failedScaleBand) {
      // Read at that scale alone, where no carry by the square of the scale misleads it.
      this.#heavier.push(ms > (1 + AIR_TUNING.failedBand) * failed.ms);
      if (this.#heavier.length > CONTROLLER_TUNING.dropWindow) {
        this.#heavier.shift();
      }
      if (this.#heavier.filter((each) => each).length >= CONTROLLER_TUNING.dropCount) {
        // Heavier than the frames the failed return was judged on: another view, judged afresh.
        this.#failedAt = null;
        this.#heavier = [];
      } else if (ms >= (1 - AIR_TUNING.failedBand) * failed.ms) {
        over = true;
      }
    }
    this.#window.push(over);
    if (this.#window.length > CONTROLLER_TUNING.dropWindow) {
      this.#window.shift();
    }
    this.#run =
      this.#window.filter((each) => each).length >= CONTROLLER_TUNING.dropCount ? 0 : this.#run + 1;
  }

  /** The frames a return waits for: the first wait, doubled by each failed return, up to the limit. */
  #returnFrames(): number {
    return AIR_TUNING.returnFrames * Math.min(2 ** this.#doublings, AIR_TUNING.returnGrowthLimit);
  }

  #step(): void {
    if (
      this.#sinceReturn !== null &&
      this.#sinceReturn <= AIR_TUNING.quickFailUpdates &&
      this.#returnedAt !== null
    ) {
      // It failed on the frames it was judged on: hold the next back until they change.
      this.#failedAt = this.#returnedAt;
      this.#heavier = [];
    }
    const over = this.#recent.filter((frame) => frame.over).map((frame) => frame.atMinMs);
    this.#before = over.length >= CONTROLLER_TUNING.dropCount ? over : null;
    this.#after = [];
    this.#stepped = true;
    this.#recent = [];
    this.#window = [];
    this.#run = 0;
    if (this.#sinceReturn !== null) {
      // A step soon after a return is the return's failure: the next waits longer.
      if (2 ** this.#doublings < AIR_TUNING.returnGrowthLimit) {
        this.#doublings += 1;
      }
      this.#sinceReturn = null;
    }
  }

  #return(): void {
    this.#measure();
    const last = this.#judged.at(-1);
    this.#returnedAt =
      last === undefined
        ? null
        : {
            scale: last.scale,
            ms: median(this.#judged.map(({ ms, scale }) => ms * (last.scale / scale) ** 2)),
          };
    this.#failedAt = null;
    this.#heavier = [];
    this.#judged = [];
    this.#stepped = false;
    this.#window = [];
    this.#run = 0;
    this.#sinceReturn = 0;
  }

  /** Takes the air's cost from the last step's pair, where both its windows are clean; ends them. */
  #measure(): void {
    const before = this.#before;
    const after = this.#after;
    if (
      before !== null &&
      after !== null &&
      after.length >= AIR_TUNING.afterFramesMin &&
      tight(before) &&
      tight(after)
    ) {
      this.#costRatio = Math.min(
        AIR_TUNING.maxCostRatio,
        Math.max(0, median(before) / median(after) - 1),
      );
    }
    this.#before = null;
    this.#after = null;
  }

  /** Without a timer: steps as the scale drops below its max, and back once its intervals are calm. */
  #fromInterval({ scale, bounds, target, intervalMs }: FloorAirUpdate): void {
    if (!this.#onInterval) {
      // The timer's frames, when they come back, start its windows and waits again.
      this.#clearTimerWindows();
      this.#onInterval = true;
    }
    this.#lastScale = scale;
    this.#sinceChange = 0;
    if (scale < bounds[1]) {
      this.#stepped = true;
      this.#calm = 0;
      return;
    }
    if (!this.#stepped) {
      return;
    }
    if (intervalMs > 0) {
      this.#calm = intervalMs <= CALM_INTERVAL_SHARE * target.periodMs ? this.#calm + 1 : 0;
    }
    if (this.#calm >= CALM_FRAMES) {
      this.#stepped = false;
      this.#calm = 0;
    }
  }
}
