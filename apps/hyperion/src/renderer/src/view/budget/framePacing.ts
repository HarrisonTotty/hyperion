/**
 * The several views' frames (plan R07, T19; Design note 14, decision-r07-t18 item 1): which
 * animation frames each view draws in at its budget's rate, the GPU time of each of the primary
 * view's frames with every view's passes in it, and the primary's internal scale from its budget
 * and its resolution controller.
 *
 * @remarks
 * A 60 Hz view draws in every animation frame and a 30 Hz view in every second one, counted from
 * the frame the views started in, so that a 30 Hz primary draws on every second vsync as R05
 * Design note 21 paces the low setting, and an instrument, at 30 Hz, draws only in frames the
 * primary draws in too. The instruments' cost so lands in the primary's frame, whose GPU time the
 * controller holds to the budget: {@link PrimaryFrameTimes} gives each primary frame every resolve
 * submitted from its start to the next one's, since R01 numbers resolves (`PassTimes.frame`, read
 * as `RenderEngine.passTimesFrame`), one a submission, not animation frames.
 *
 * That counting is the launch's `display` pacing (`--frame-pacing`), at up to the display's rate:
 * on a 120 Hz panel a 60 Hz view draws at 120 Hz and a 30 Hz one at 60. Under `rate` each view
 * draws at its design rate by time, on a whole number of the measured vsync period
 * (`lib/vsyncPacing.ts`; the owner's frame rate). {@link FramePacer} holds either for a loop.
 */

import type { FramePacingName } from "../../../../preload/api";
import { drawsNow, VsyncClock, vsyncsPerDraw } from "../../lib/vsyncPacing";
import type { PassTimes } from "../engine/types";
import { gpuTimeMs, ResolutionController, type ScaleBounds } from "./resolutionController";
import type { ViewBudget } from "./viewBudget";

/** The animation frames a view's rate spans: 1 at 60 Hz, 2 at 30 Hz. */
function frameDivisor(rateHz: 60 | 30): number {
  return 60 / rateHz;
}

/**
 * Whether a view at `rateHz` draws in animation frame `frame`, counted from 0: every frame at
 * 60 Hz, every second one at 30 Hz.
 */
export function drawsInFrame(frame: number, rateHz: 60 | 30): boolean {
  return frame % frameDivisor(rateHz) === 0;
}

/** One rate's beat under `rate`: the frame it was last asked in, its answer, and its last draw. */
interface Beat {
  readonly frame: number;
  readonly draws: boolean;
  /** The timestamp of its last draw, ms, or `null` before its first. */
  readonly drawnMs: number | null;
}

/**
 * Paces a loop's views at their rates in its animation frames (R07.T19; the owner's frame rate):
 * by counting frames under `display` ({@link drawsInFrame}), by time under `rate`
 * (`lib/vsyncPacing.ts`' `drawsNow`, on its `VsyncClock`'s measured period).
 *
 * @remarks
 * Every view at one rate draws in the same frames, its rate's beat: so the 30 Hz instruments draw
 * together, in frames the primary draws (they are asked only there, `withinHz` the primary's
 * rate, so that their beat keeps to the primary's), and the readouts' publish, asked at their rate,
 * falls in a frame every instrument draws, as counted frames had it. A beat is asked any number of
 * times in a frame with one answer, the first ask's, and moves on only in a frame it is asked.
 *
 * @example
 * ```ts
 * const pacer = new FramePacer(window.hyperion.framePacing);
 * const tick = (nowMs: number): void => {
 *   pacer.frame(nowMs);
 *   if (pacer.draws(primaryBudget.rateHz)) {
 *     // draw the primary, then each instrument where
 *     // `pacer.draws(budget.rateHz, primaryBudget.rateHz)`
 *   }
 * };
 * ```
 */
export class FramePacer {
  readonly #mode: FramePacingName;
  readonly #clock = new VsyncClock();
  readonly #beats = new Map<number, Beat>();
  #frame = -1;
  #nowMs = 0;

  constructor(mode: FramePacingName) {
    this.#mode = mode;
  }

  /** The pacing it keeps. */
  get mode(): FramePacingName {
    return this.#mode;
  }

  /** The vsync period the loop's frames measure, ms (`VsyncClock`), whatever the mode. */
  get periodMs(): number {
    return this.#clock.periodMs;
  }

  /** Starts an animation frame at its timestamp, ms: once a callback, before any {@link draws}. */
  frame(nowMs: number): void {
    this.#frame += 1;
    this.#nowMs = nowMs;
    this.#clock.note(nowMs);
  }

  /**
   * Whether the views at `rateHz` draw in this frame.
   *
   * @param withinHz - The rate of the views in whose draws alone these are asked, so that their
   *   beat keeps to those draws' (the instruments in the primary's frames); none where they are
   *   asked in every frame.
   */
  draws(rateHz: 60 | 30, withinHz?: 60 | 30): boolean {
    if (this.#mode === "display") {
      return drawsInFrame(this.#frame, rateHz);
    }
    const beat = this.#beats.get(rateHz);
    if (beat?.frame === this.#frame) {
      return beat.draws;
    }
    const drawnMs = beat?.drawnMs ?? null;
    const periodMs = this.#clock.periodMs;
    const draws = drawsNow(
      drawnMs === null ? Number.POSITIVE_INFINITY : this.#nowMs - drawnMs,
      rateHz,
      periodMs,
      withinHz === undefined ? 1 : vsyncsPerDraw(withinHz, periodMs),
    );
    this.#beats.set(rateHz, {
      frame: this.#frame,
      draws,
      drawnMs: draws ? this.#nowMs : drawnMs,
    });
    return draws;
  }
}

/**
 * The primary frames whose resolves are awaited at most, a bound on memory: a group is discarded
 * anyway once a later one completes, so this matters only while no report arrives at all.
 */
export const PENDING_PRIMARY_FRAMES = 8;

/** A primary frame's group, awaited: its first and last resolve, and what has come of them. */
interface PendingFrame {
  readonly first: number;
  readonly last: number;
  received: number;
  /** The GPU time of the resolves received, ms. */
  ms: number;
}

/**
 * Groups R01's pass times by the primary view's frame (decision-r07-t19, item 1): a frame's group
 * is the resolves numbered from its start to the next frame's, every view's and target's, and it is
 * reported once all of them have arrived.
 *
 * @remarks
 * A group with no resolve gives no entry (an absent timer, nothing timed, views detached during a
 * loss). A group still awaited when a later one completes is discarded without an entry: one of its
 * resolves was dropped, or its read failed, and a frame short of part of its time would read as
 * cheaper than it was. A late report of a discarded group is ignored.
 *
 * @example
 * ```ts
 * const times = new PrimaryFrameTimes(engine.passTimesFrame);
 * engine.onPassTimes((t) => times.passTimes(t));
 * engine.onRestored(() => times.reset(engine.passTimesFrame));
 * // At the start of each primary frame: the previous frame's group ends here.
 * times.startFrame(engine.passTimesFrame);
 * controller.update(timer === "absent" ? undefined : times.take(), intervalMs);
 * ```
 */
export class PrimaryFrameTimes {
  #pending: PendingFrame[] = [];
  /** Reports of the frame still being drawn, whose group is not yet ended. */
  #early: PassTimes[] = [];
  #mark: number;
  /** Whether a primary frame has started since the last mark was set without one. */
  #started = false;
  #done: number[] = [];

  /** @param mark - `RenderEngine.passTimesFrame` now: earlier resolves belong to no frame. */
  constructor(mark = 0) {
    this.#mark = mark;
  }

  /**
   * Starts a primary frame at `mark` (`RenderEngine.passTimesFrame` now): the previous frame's
   * group is the resolves after the previous mark, up to this one. A count that went back (a new
   * timer) starts again from it.
   */
  startFrame(mark: number): void {
    if (mark < this.#mark) {
      this.reset(mark);
    }
    const ended = this.#started && mark > this.#mark ? { first: this.#mark + 1, last: mark } : null;
    this.#started = true;
    this.#mark = mark;
    if (ended !== null) {
      this.#pending.push({ ...ended, received: 0, ms: 0 });
      while (this.#pending.length > PENDING_PRIMARY_FRAMES) {
        this.#pending.shift();
      }
      const early = this.#early;
      this.#early = [];
      for (const times of early) {
        this.passTimes(times);
      }
    }
  }

  /** Takes one resolve's times (`RenderEngine.onPassTimes`). */
  passTimes(times: PassTimes): void {
    if (times.frame > this.#mark) {
      this.#early.push(times);
      return;
    }
    const index = this.#pending.findIndex(
      ({ first, last }) => times.frame >= first && times.frame <= last,
    );
    const group = this.#pending[index];
    if (group === undefined) {
      // Before the first frame (a warm-up), or of a group already discarded.
      return;
    }
    group.received += 1;
    group.ms += gpuTimeMs([times]);
    if (group.received >= group.last - group.first + 1) {
      // Complete: reported, and every group before it still awaited is discarded.
      this.#done.push(group.ms);
      this.#pending.splice(0, index + 1);
    }
  }

  /**
   * The GPU time of each primary frame whose resolves have all arrived since the last call, ms
   * ({@link gpuTimeMs}), oldest first; none on some calls, two on others.
   */
  take(): ReadonlyArray<number> {
    const done = this.#done;
    this.#done = [];
    return done;
  }

  /**
   * Discards every group and its mark, as at a restore, whose new timer numbers its resolves from
   * 1 again: the next primary frame starts the first group after `mark`.
   */
  reset(mark = 0): void {
    this.#pending = [];
    this.#early = [];
    this.#done = [];
    this.#started = false;
    this.#mark = mark;
  }
}

function sameBounds(a: ScaleBounds, b: ScaleBounds): boolean {
  return a[0] === b[0] && a[1] === b[1];
}

/**
 * How a frame-rate floor's air comes back to its setting's sizes (the owner's report, 2026-11-01):
 * once its scale is back at its max, its frames' GPU time must stay under this share of the floor's
 * budget (without a timer, its intervals under {@link CALM_INTERVAL_SHARE} of its period) for
 * {@link CALM_FRAMES} frames in a row, so that the air drawn at the low setting's sizes, whose
 * frames are cheaper, returns only where its own may fit.
 */
export const CALM_SHARE = 0.5;

/**
 * Without a timer, the share of the floor's period under which a frame's interval is calm: frames
 * drawn at a 60 Hz display's every vsync, 16.7 ms against the floor's 33.3, are; at its 30 Hz not.
 */
export const CALM_INTERVAL_SHARE = 0.6;

/** The frames in a row a floor's air waits through under {@link CALM_SHARE} before it returns. */
export const CALM_FRAMES = 60;

/**
 * The primary view's internal scale from its budget: the budget's `renderScale`, or, while its
 * `control` is set, the scale of a {@link ResolutionController} made from it when it first
 * appears, retargeted when its target changes and dropped when it returns to `null`.
 *
 * @remarks
 * Under its frame-rate floor (`ScaleControl.kind` `floor`; the owner's report, 2026-11-01) it also
 * holds whether the view's air is stepped to the low setting's sizes ({@link
 * BudgetedScale.airStepped}): from the frame its scale first drops below its max, until its scale
 * is back at its max and its frames have stayed calm, under {@link CALM_SHARE} of the budget, for
 * {@link CALM_FRAMES} frames in a row.
 */
export class BudgetedScale {
  #controller: ResolutionController | null = null;
  #bounds: ScaleBounds | null = null;
  #airStepped = false;
  #lowest = false;
  #calm = 0;

  /** Whether the frame-rate floor has stepped the view's air to the low setting's sizes. */
  get airStepped(): boolean {
    return this.#airStepped;
  }

  /**
   * Whether the frame-rate floor holds the view's scale at its lowest step, its bounds' min, when
   * its large discs' limb pixels take `PhotorealSettings.floorLimbSamples` cells (R07.T8.i, the
   * owner's priority).
   */
  get atLowestStep(): boolean {
    return this.#lowest;
  }

  /** The controller while the budget controls the scale, else `null`. */
  get controller(): ResolutionController | null {
    return this.#controller;
  }

  /**
   * The scale of the primary's next frame, from its budget and the measurements of the frames
   * since the last call (once a primary frame).
   *
   * @param gpuFramesMs - {@link PrimaryFrameTimes.take}, or `undefined` while
   *   `GraphicsStatus.timer` is `absent`.
   * @param intervalMs - The interval between the primary's last two frames, ms; 0 for the first.
   */
  update(
    budget: ViewBudget,
    gpuFramesMs: ReadonlyArray<number> | undefined,
    intervalMs: number,
  ): number {
    const { control } = budget;
    if (control === null) {
      this.#controller = null;
      this.#bounds = null;
      this.#airStepped = false;
      this.#lowest = false;
      return budget.renderScale;
    }
    if (
      this.#controller === null ||
      this.#bounds === null ||
      !sameBounds(this.#bounds, control.bounds)
    ) {
      this.#controller = new ResolutionController(control.bounds, control.target);
      this.#bounds = control.bounds;
    } else {
      this.#controller.retarget(control.target);
    }
    const scale = this.#controller.update(gpuFramesMs, intervalMs);
    this.#stepAir(control, scale, gpuFramesMs, intervalMs);
    this.#lowest = control.kind === "floor" && scale <= control.bounds[0];
    return scale;
  }

  /** Steps the floor's air down as its scale drops, and back once its frames have stayed calm. */
  #stepAir(
    control: NonNullable<ViewBudget["control"]>,
    scale: number,
    gpuFramesMs: ReadonlyArray<number> | undefined,
    intervalMs: number,
  ): void {
    if (control.kind !== "floor") {
      this.#airStepped = false;
      return;
    }
    if (scale < control.bounds[1]) {
      this.#airStepped = true;
      this.#calm = 0;
      return;
    }
    if (!this.#airStepped) {
      return;
    }
    const measured =
      gpuFramesMs === undefined
        ? intervalMs > 0
          ? [intervalMs <= CALM_INTERVAL_SHARE * control.target.periodMs]
          : []
        : gpuFramesMs.map((ms) => ms <= CALM_SHARE * control.target.gpuBudgetMs);
    for (const calm of measured) {
      this.#calm = calm ? this.#calm + 1 : 0;
    }
    if (this.#calm >= CALM_FRAMES) {
      this.#airStepped = false;
      this.#calm = 0;
    }
  }
}
