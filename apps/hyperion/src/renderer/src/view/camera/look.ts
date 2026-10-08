/**
 * Turning a view's line of sight (plan R07, T19.f): the turn a drag or the arrows ask for, and the
 * look offset that turns a `SEAT` or `CHASE` camera from its preset without rolling it.
 */

import { norm, scale, vec3 } from "../../geometry/vec3";
import type { CameraPose, Quaternion } from "./pose";
import { conjugate, multiply, quaternionFromAxisAngle, rotate } from "./quaternion";

/**
 * A turn of a view's line of sight, rad, about the camera's own axes: `yawRad` about its +y,
 * positive to the left, and `pitchRad` about its +x, positive up (the free camera's convention).
 */
export interface ViewTurn {
  readonly yawRad: number;
  readonly pitchRad: number;
}

/** No turn. */
export const NO_TURN: ViewTurn = { yawRad: 0, pitchRad: 0 };

/** Two turns one after the other, to first order: their yaws and pitches added. */
export function addTurns(a: ViewTurn, b: ViewTurn): ViewTurn {
  return { yawRad: a.yawRad + b.yawRad, pitchRad: a.pitchRad + b.pitchRad };
}

/** Whether a turn turns anything. */
export function isTurn(turn: ViewTurn): boolean {
  return turn.yawRad !== 0 || turn.pitchRad !== 0;
}

/**
 * A `SEAT` or `CHASE` camera's look offset from its preset's line of sight: an azimuth about the
 * hull's up, positive to the left, then an elevation about the turned right axis, positive up, rad.
 *
 * @remarks
 * Turned this way the camera's right axis stays in the hull's plane, so the camera never rolls
 * from the hull's up and stays right way up. The azimuth wraps to (−π, π]; the elevation is held
 * where the line of sight would pass the vertical ({@link turnedOffset}).
 */
export interface LookOffset {
  readonly azimuthRad: number;
  readonly elevationRad: number;
}

/** The preset's own line of sight. */
export const NO_LOOK_OFFSET: LookOffset = { azimuthRad: 0, elevationRad: 0 };

/** Whether an offset turns the camera from its preset at all. */
export function hasLookOffset(offset: LookOffset): boolean {
  return offset.azimuthRad !== 0 || offset.elevationRad !== 0;
}

const UP = vec3(0, 1, 0);
const RIGHT = vec3(1, 0, 0);
const FORWARD = vec3(0, 0, -1);
const BACKWARD = vec3(0, 0, 1);
const HALF_TURN_RAD = Math.PI;
const QUARTER_TURN_RAD = Math.PI / 2;

/** An angle wrapped to (−π, π]. */
function wrapRad(angleRad: number): number {
  const wrapped = angleRad - 2 * Math.PI * Math.floor((angleRad + HALF_TURN_RAD) / (2 * Math.PI));
  return wrapped === -HALF_TURN_RAD ? HALF_TURN_RAD : wrapped;
}

/** An orientation's line of sight as an azimuth and elevation from the hull's forward, rad. */
function hullAngles(
  orientation: Quaternion,
  attitude: Quaternion,
): { readonly azimuthRad: number; readonly elevationRad: number } {
  const forward = rotate(multiply(conjugate(attitude), orientation), FORWARD);
  return {
    azimuthRad: Math.atan2(-forward.x, -forward.z),
    elevationRad: Math.asin(Math.min(1, Math.max(-1, forward.y))),
  };
}

/**
 * An offset's elevation held so that the line of sight stays within ±90° of the hull's plane, from
 * a preset whose own line of sight stands `baseElevationRad` above it, and within ±90° of the
 * preset's own.
 */
function heldElevation(elevationRad: number, baseElevationRad: number): number {
  const lowest = Math.max(-QUARTER_TURN_RAD - baseElevationRad, -QUARTER_TURN_RAD);
  const highest = Math.min(QUARTER_TURN_RAD - baseElevationRad, QUARTER_TURN_RAD);
  return Math.min(highest, Math.max(lowest, elevationRad));
}

/**
 * A look offset turned by `turn`: its yaw added to the azimuth, which wraps, and its pitch to the
 * elevation, which is held where the line of sight would pass the vertical.
 *
 * @param base - The preset's own orientation, in the ship's frame axes.
 * @param attitude - The own ship's attitude, whose up the offset turns about.
 */
export function turnedOffset(
  offset: LookOffset,
  turn: ViewTurn,
  base: Quaternion,
  attitude: Quaternion,
): LookOffset {
  const { elevationRad } = hullAngles(base, attitude);
  return {
    azimuthRad: wrapRad(offset.azimuthRad + turn.yawRad),
    elevationRad: heldElevation(offset.elevationRad + turn.pitchRad, elevationRad),
  };
}

/**
 * A preset's pose turned by its look offset (plan R07, T19.f): the line of sight turned from the
 * preset's own by the offset's azimuth about the hull's up and its elevation, with no roll from
 * the hull's up. `orbit` (the `CHASE` camera) also swings the camera about its frame's origin, the
 * own ship, at the distance it stands, so that the ship stays at the view's centre; otherwise (the
 * `SEAT` camera) it turns where it stands. Without an offset the pose is returned as it is.
 *
 * @remarks
 * Every preset's own orientation has no roll from the hull's up (`lookAlong` with the hull's up),
 * so it is the hull's attitude turned by its own azimuth and then its own elevation, to which the
 * offset's are added.
 *
 * @param attitude - The own ship's attitude: the rotation from hull axes to the frame's axes.
 */
export function offsetPose(
  base: CameraPose,
  attitude: Quaternion,
  offset: LookOffset,
  orbit: boolean,
): CameraPose {
  if (!hasLookOffset(offset)) {
    return base;
  }
  const own = hullAngles(base.orientation, attitude);
  const elevationRad = own.elevationRad + heldElevation(offset.elevationRad, own.elevationRad);
  const orientation = multiply(
    attitude,
    multiply(
      quaternionFromAxisAngle(UP, own.azimuthRad + offset.azimuthRad),
      quaternionFromAxisAngle(RIGHT, elevationRad),
    ),
  );
  return {
    frame: base.frame,
    positionM: orbit ? scale(rotate(orientation, BACKWARD), norm(base.positionM)) : base.positionM,
    orientation,
  };
}
