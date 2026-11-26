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
 * **The middle step** (lane perf-air-mac; `--floor-air middle`, a lever whose default is the
 * floor's own, `low`): the floor's air takes two steps, high's own to the middle step's sizes
 * (`hillaire.ts`'s `FLOOR_MIDDLE_SIZES`: high's counts with the march at half resolution), then to
 * the low setting's, each down by the rule above from the step it stands at, and comes back the
 * same way a step at a time, each return judged by its own step's measured share: from low's to
 * the middle step's where the middle step's frames would fit at the floor's lowest scale, and from
 * there to high's own where those would. Each step's share is measured from its own pair, its prior
 * {@link AIR_TUNING}'s `unmeasuredMiddleCostRatios`. A step down from the middle step waits until
 * the frames drawn at it have settled, as a return does. The guard's watch and its doubled waits
 * are the floor's, whatever step a return crosses; a return held back on its own frames is held
 * at the step it was tried from alone. As the middle step's frames stand in the controller's band,
 * at a scale that differs a little from one return to the next, where the floor's own stepped
 * frames rise to the bounds' max, a return that fails on its own frames there also takes its
 * step's share up to where the frames it was judged on would have been over by the hold's band,
 * and its step's pair is not measured, both misled by the same carry (lean, 2026-11-26). Without
 * a timer the air steps as before, straight to low's.
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

import type { FloorAirName } from "../../../../preload/api";
import { CONTROLLER_TUNING, type ResolutionTarget, type ScaleBounds } from "./resolutionController";

/**
 * The sizes a frame-rate floor draws its view's air at (lane perf-air-mac): high's own, the
 * floor's middle step's (`hillaire.ts`'s `FLOOR_MIDDLE_SIZES`) or the low setting's.
 */
export type FloorAirStep = "own" | "middle" | "low";

/**
 * How a floor steps its air down (lane perf-air-mac): `low`, straight to the low setting's sizes,
 * the floor's own; `middle`, by its middle step first.
 */
export type FloorAirKind = "low" | "middle";

/**
 * What a `--floor-air` name asks of the floor: that it step its air by its kind, or a probe's hold
 * of the air at one step whatever its frames.
 */
export type FloorAirMode =
  | { readonly kind: "floor"; readonly steps: FloorAirKind }
  | { readonly kind: "held"; readonly step: FloorAirStep };

/** Each name's mode. */
const FLOOR_AIR_MODES: Readonly<Record<FloorAirName, FloorAirMode>> = {
  low: { kind: "floor", steps: "low" },
  middle: { kind: "floor", steps: "middle" },
  "held-own": { kind: "held", step: "own" },
  "held-middle": { kind: "held", step: "middle" },
  "held-low": { kind: "held", step: "low" },
};

/** What `name` asks of the floor ({@link FloorAirMode}). */
export function floorAirModeOf(name: FloorAirName): FloorAirMode {
  return FLOOR_AIR_MODES[name];
}

/** The step a probe of `name` holds the air at whatever its frames, or `null` where none holds it. */
export function heldStepOf(name: FloorAirName): FloorAirStep | null {
  const mode = floorAirModeOf(name);
  return mode.kind === "held" ? mode.step : null;
}

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
  /**
   * Under the middle step (lane perf-air-mac), each step's share before its own pair has measured
   * it: a frame with high's own air over one at the middle step's sizes, then the middle step's
   * over one at low's. Their product's less 1 is `unmeasuredCostRatio`, the whole way down,
   * (1 + 0.75) × (1 + 0.15) ≈ 2, split as the pod measured them from orbit at the 2026-11-26
   * batch's body (SwiftShader, each frame over the rest of its passes: the middle step's frame 1.25
   * times low's, high's own 2.59 times the middle step's), in proportion to their logarithms
   * (lane perf-air-mac's `results.md`).
   */
  unmeasuredMiddleCostRatios: [0.75, 0.15],
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

/** The steps a floor of each kind takes its air down: high's own first, then each below it. */
const STEPS_OF: Readonly<Record<FloorAirKind, ReadonlyArray<FloorAirStep>>> = {
  low: ["own", "low"],
  middle: ["own", "middle", "low"],
};

/** A step's prior shares, its boundaries' from the top down, before a pair has measured them. */
function priorCostRatios(kind: FloorAirKind): number[] {
  return kind === "low"
    ? [AIR_TUNING.unmeasuredCostRatio]
    : [...AIR_TUNING.unmeasuredMiddleCostRatios];
}

/** A frame-rate floor's air: whether it is stepped, and when it steps and returns (the module's remarks). */
export class FloorAir {
  /** The floor's kind: straight to low's sizes, or by the middle step (`--floor-air`). */
  #kind: FloorAirKind = "low";
  /** The step the air stands at, an index of {@link STEPS_OF}'s: 0, high's own. */
  #level = 0;
  /**
   * The air's cost at each step as a share of a frame at the step below it, from the top down:
   * each its last pair's measure, or its prior.
   */
  #costRatios: number[] = priorCostRatios("low");
  /** The scale of the last update, which the next update's frames were drawn at; `null` before one. */
  #lastScale: number | null = null;
  /** Updates since the scale or the air last changed; a frame is read once they reach the settle. */
  #sinceChange: number = CONTROLLER_TUNING.settleUpdates;
  /** The air's last frames read above the lowest step: each carried to the lowest scale, and whether over the budget. */
  #recent: Array<{ readonly atMinMs: number; readonly over: boolean }> = [];
  /** The frames over the budget at the last step, carried to the lowest; `null` for none. */
  #before: ReadonlyArray<number> | null = null;
  /** The frames read at the lowest scale since the last step; `null` once measured. */
  #after: number[] | null = null;
  /** The boundary the pending pair measures, the step it went down from; `null` for none. */
  #measuring: number | null = null;
  /** Whether each of the last few stepped frames, given the air's cost, was over the line. */
  #window: boolean[] = [];
  /** The last few stepped frames read, ms, and the scale each was drawn at. */
  #judged: Array<{ readonly ms: number; readonly scale: number }> = [];
  /**
   * The scale the last return was tried at, its frames' median there and carried to the lowest
   * scale, ms, and the step it was tried from.
   */
  #returnedAt: {
    readonly scale: number;
    readonly ms: number;
    readonly atMinMs: number;
    readonly level: number;
  } | null = null;
  /** The same of a return that failed on its own frames, which holds the next back; else `null`. */
  #failedAt: {
    readonly scale: number;
    readonly ms: number;
    readonly atMinMs: number;
    readonly level: number;
  } | null = null;
  /** The lowest scale and the budget of the last update, ms. */
  #min = 1;
  #budgetMs = 0;
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

  /** Whether the floor has stepped the view's air down from its own sizes. */
  get stepped(): boolean {
    return this.#level > 0;
  }

  /** The sizes the floor draws the view's air at. */
  get step(): FloorAirStep {
    return STEPS_OF[this.#kind][this.#level] ?? "low";
  }

  /**
   * The air's cost as a share of a stepped frame: the last step's measure, or its assumed value;
   * under the middle step the share the next return is judged by, the step's above the one the
   * air stands at (high's own over the middle step's while it stands at its own).
   */
  get costRatio(): number {
    return this.#costRatios[Math.max(0, this.#level - 1)] ?? AIR_TUNING.unmeasuredCostRatio;
  }

  /** Each step's share, from the top down ({@link FloorAir.costRatio}'s). */
  get costRatios(): ReadonlyArray<number> {
    return this.#costRatios;
  }

  /**
   * Takes one update of the floor's view and returns whether its air changed under the pass timer,
   * after which the caller restarts its controller (`ResolutionController.restart`): the frames it
   * holds are of the air before. Without a timer it returns `false`, the air changing as before.
   *
   * @param kind - How it steps its air (`--floor-air`): `low`, straight to low's sizes, the
   *   floor's own; or `middle`, by the middle step. A new kind takes its priors, and a stepped air
   *   back to high's own.
   */
  update(update: FloorAirUpdate, kind: FloorAirKind = "low"): boolean {
    if (kind !== this.#kind) {
      this.#kind = kind;
      this.#costRatios = priorCostRatios(kind);
      if (this.#level > 0) {
        this.leave();
      }
    }
    if (update.framesMs === undefined) {
      this.#fromInterval(update);
      return false;
    }
    this.#onInterval = false;
    const { framesMs, scale, bounds, target } = update;
    const [min] = bounds;
    this.#min = min;
    this.#budgetMs = target.gpuBudgetMs;
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
    const lowest = STEPS_OF[this.#kind].length - 1;
    let changed = false;
    if (this.#level < lowest && update.wantedBelowMin && (this.#level === 0 || settled)) {
      // From the middle step, once its own frames are read: a drop judged on the frames drawn
      // before it took effect would pass it by.
      this.#step();
      changed = true;
    } else if (this.#level > 0 && settled && !scaleChanged && this.#run >= this.#returnFrames()) {
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
    this.#level = 0;
    this.restartWindows();
  }

  /** Starts the pass timer's windows and waits again, keeping the air and what it costs. */
  #clearTimerWindows(): void {
    this.#lastScale = null;
    this.#sinceChange = 0;
    this.#recent = [];
    this.#before = null;
    this.#after = null;
    this.#measuring = null;
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
    if (this.#level < STEPS_OF[this.#kind].length - 1) {
      // Frames that may take the air a step down.
      this.#recent.push({ atMinMs, over: ms > CONTROLLER_TUNING.dropAbove * budgetMs });
      if (this.#recent.length > CONTROLLER_TUNING.dropWindow) {
        this.#recent.shift();
      }
    }
    if (this.#level === 0) {
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
    let over = (1 + this.costRatio) * atMinMs > CONTROLLER_TUNING.aim * budgetMs;
    const failed = this.#failedAt;
    if (
      failed !== null &&
      failed.level === this.#level &&
      Math.abs(drawnAt / failed.scale - 1) <= AIR_TUNING.failedScaleBand
    ) {
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
    // A pair the step from above left unmeasured ends here, with the frames it has.
    this.#measure();
    // Whether the middle step's quick failure took the share, whose pair the carry would mislead.
    let quick = false;
    if (
      this.#sinceReturn !== null &&
      this.#sinceReturn <= AIR_TUNING.quickFailUpdates &&
      this.#returnedAt !== null
    ) {
      // It failed on the frames it was judged on: hold the next back until they change.
      const failed = this.#returnedAt;
      this.#failedAt = failed;
      this.#heavier = [];
      if (this.#kind === "middle") {
        // Its step's share, too, to where the frames it was judged on would have been over by the
        // hold's band: the middle step's frames stand at a scale that differs from one return to
        // the next, where the hold, read at one scale, would let each misjudge its return once.
        const boundary = failed.level - 1;
        this.#costRatios[boundary] = Math.min(
          AIR_TUNING.maxCostRatio,
          Math.max(
            this.#costRatios[boundary] ?? 0,
            ((1 + AIR_TUNING.failedBand) * CONTROLLER_TUNING.aim * this.#budgetMs) /
              failed.atMinMs -
              1,
          ),
        );
        quick = true;
      }
    } else if (this.#failedAt !== null && this.#failedAt.level !== this.#level + 1) {
      // A hold of the step above, which a step down leaves.
      this.#failedAt = null;
      this.#heavier = [];
    }
    const over = this.#recent.filter((frame) => frame.over).map((frame) => frame.atMinMs);
    this.#before = !quick && over.length >= CONTROLLER_TUNING.dropCount ? over : null;
    this.#after = [];
    this.#measuring = this.#level;
    this.#level += 1;
    this.#recent = [];
    this.#window = [];
    this.#judged = [];
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
    if (last === undefined) {
      this.#returnedAt = null;
    } else {
      const ms = median(this.#judged.map((each) => each.ms * (last.scale / each.scale) ** 2));
      this.#returnedAt = {
        scale: last.scale,
        ms,
        atMinMs: ms * (this.#min / last.scale) ** 2,
        level: this.#level,
      };
    }
    this.#failedAt = null;
    this.#heavier = [];
    this.#judged = [];
    this.#level -= 1;
    this.#recent = [];
    this.#window = [];
    this.#run = 0;
    this.#sinceReturn = 0;
  }

  /** Takes the air's cost from the last step's pair, where both its windows are clean; ends them. */
  #measure(): void {
    const before = this.#before;
    const after = this.#after;
    const boundary = this.#measuring;
    if (
      boundary !== null &&
      before !== null &&
      after !== null &&
      after.length >= AIR_TUNING.afterFramesMin &&
      tight(before) &&
      tight(after)
    ) {
      this.#costRatios[boundary] = Math.min(
        AIR_TUNING.maxCostRatio,
        Math.max(0, median(before) / median(after) - 1),
      );
    }
    this.#before = null;
    this.#after = null;
    this.#measuring = null;
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
      // Straight to the lowest step, whatever the floor's kind: no frame says what a step saves.
      this.#level = STEPS_OF[this.#kind].length - 1;
      this.#calm = 0;
      return;
    }
    if (this.#level === 0) {
      return;
    }
    if (intervalMs > 0) {
      this.#calm = intervalMs <= CALM_INTERVAL_SHARE * target.periodMs ? this.#calm + 1 : 0;
    }
    if (this.#calm >= CALM_FRAMES) {
      this.#level = 0;
      this.#calm = 0;
    }
  }
}
