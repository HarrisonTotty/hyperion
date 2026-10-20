/**
 * One contour interval for a pose (plan R10, R10.T12.b; decision-r10-view-block-room §10): the
 * guide's "It is set by the camera's height above the ground beneath it, so that every view in one
 * pose states the same one", held across a stage's views.
 *
 * @remarks
 * Each view's interval steps by Design note 14's ±20% hysteresis from the one it held
 * (`contourInterval`), so two views that reached one point from different heights, or an
 * instrument opened where the primary holds the step below inside its band, could state intervals a
 * step apart. So the views of a stage state theirs through one holder: a view whose camera stands
 * within {@link POSE_TOLERANCE_M} of a point another view stated its interval at, over the same
 * body, takes that view's interval where it was stated in the same frame, and else steps its own
 * hysteresis from the latest interval held there, its own or another's; elsewhere each view's
 * hysteresis is its own. The primary's frame draws
 * before its instruments', so the instruments at its pose take its interval.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import { norm, sub, type Vec3 } from "../../geometry/vec3";
import type { ViewId } from "../camera/state";

/**
 * How near two cameras stand to share a pose's interval, m: 1 m, the ruling's bound. Two views of
 * one seat, or an instrument opened at the primary's pose, stand at one point to rounding; a chase
 * camera's offset or another craft's seat stands metres or more away, and keeps its own.
 */
export const POSE_TOLERANCE_M = 1;

/** Where a view stated its interval: the body and its camera's point, body-fixed, m. */
export interface PosePoint {
  /** The body its camera stands lowest over, whose ground sets the interval. */
  readonly body: BodyIdHex;
  /** The camera's position from the body's centre in the body's own axes, m. */
  readonly cameraBodyFixedM: Vec3;
}

/** An interval another view holds at a point. */
export interface HeldInterval {
  readonly intervalM: number;
  /** Whether it was stated in the frame asked about: the same frame's time. */
  readonly thisFrame: boolean;
}

interface Held {
  readonly point: PosePoint;
  readonly intervalM: number;
  /** The frame's time it was stated at, ms. */
  readonly nowMs: number;
}

/** A stage's contour intervals, each as its view last stated it, at the point it stood. */
export class PoseIntervals {
  readonly #held = new Map<ViewId, Held>();

  /**
   * The latest interval held at `point`, by any view, `view`'s own included, or `null` with none:
   * `thisFrame` where another view stated it in this frame, at `nowMs`, so that `view` takes it as
   * it is; else it is the interval `view`'s hysteresis steps from, so that a view's own newer
   * interval is never stepped back from another's older one.
   */
  at(view: ViewId, point: PosePoint, nowMs: number): HeldInterval | null {
    let latest: { readonly by: ViewId; readonly held: Held } | null = null;
    for (const [by, held] of this.#held) {
      if (
        held.point.body === point.body &&
        norm(sub(held.point.cameraBodyFixedM, point.cameraBodyFixedM)) <= POSE_TOLERANCE_M &&
        (latest === null || held.nowMs > latest.held.nowMs)
      ) {
        latest = { by, held };
      }
    }
    return latest === null
      ? null
      : {
          intervalM: latest.held.intervalM,
          thisFrame: latest.by !== view && latest.held.nowMs === nowMs,
        };
  }

  /** Records the interval `view` states at `point` in the frame at `nowMs`. */
  hold(view: ViewId, point: PosePoint, intervalM: number, nowMs: number): void {
    this.#held.set(view, { point, intervalM, nowMs });
  }

  /** Forgets `view`'s interval: it states none, or has gone. */
  forget(view: ViewId): void {
    this.#held.delete(view);
  }
}

/**
 * The interval `view` states at `point` in this frame: another view's at that point where it was
 * stated in this frame, else `step` of the latest interval held there, its own included, or of
 * `ownPreviousM` with none; recorded in `intervals` for the views after it.
 *
 * @param step - The view's hysteresis from an interval held, `contourInterval` at its height.
 */
export function poseInterval(
  intervals: PoseIntervals,
  view: ViewId,
  point: PosePoint,
  nowMs: number,
  ownPreviousM: number | null,
  step: (previousM: number | null) => number,
): number {
  const held = intervals.at(view, point, nowMs);
  const intervalM =
    held !== null && held.thisFrame ? held.intervalM : step(held?.intervalM ?? ownPreviousM);
  intervals.hold(view, point, intervalM, nowMs);
  return intervalM;
}
