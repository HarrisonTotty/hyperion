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
 */

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
 * The primary view's internal scale from its budget: the budget's `renderScale`, or, while its
 * `control` is set, the scale of a {@link ResolutionController} made from it when it first
 * appears, retargeted when its target changes and dropped when it returns to `null`.
 */
export class BudgetedScale {
  #controller: ResolutionController | null = null;
  #bounds: ScaleBounds | null = null;

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
    return this.#controller.update(gpuFramesMs, intervalMs);
  }
}
