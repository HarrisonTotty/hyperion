import { add, dot, scale, vec3, type Vec3 } from "./vec3";

/**
 * The named directions at a point, as galactic unit vectors: coreward, spinward and north.
 *
 * @remarks
 * `(spinward, coreward, north)` is a right-handed triple. Coreward points towards the galactic
 * axis, spinward along the direction of rotation, and north along +z, from which the galaxy
 * rotates counter-clockwise. See "Voice and nomenclature" in `docs/frontend/ux-guidelines.md`.
 */
export interface LocalFrame {
  readonly coreward: Vec3;
  readonly spinward: Vec3;
  readonly north: Vec3;
  /**
   * True when the point is on the galactic axis, where coreward and spinward are undefined and
   * the frame falls back to coreward −x and spinward +y; the display must say so.
   */
  readonly onAxis: boolean;
}

/** A vector's components along a {@link LocalFrame}'s directions. */
export interface LocalComponents {
  readonly coreward: number;
  readonly spinward: number;
  readonly north: number;
}

/** A position in galactic cylindrical coordinates, as the `GALACTIC` frame names them. */
export interface Cylindrical {
  /** Distance from the galactic axis. */
  readonly radiusLy: number;
  /** Angle about the axis from +x towards +y, counter-clockwise seen from the north, in [0, 360). */
  readonly angleDeg: number;
  /** Height along +z. */
  readonly heightLy: number;
}

/** Distance from the galactic axis inside which the named directions are taken as undefined. */
export const AXIS_TOLERANCE_LY = 1e-6;

const NORTH = vec3(0, 0, 1);

const AXIS_FRAME: LocalFrame = {
  coreward: vec3(-1, 0, 0),
  spinward: vec3(0, 1, 0),
  north: NORTH,
  onAxis: true,
};

/**
 * The local frame at a galactic position.
 *
 * @remarks
 * With R = √(x² + y²): coreward = (−x, −y, 0) ÷ R, spinward = (−y, x, 0) ÷ R, north = +z.
 * Within {@link AXIS_TOLERANCE_LY} of the axis the directions are undefined and the frame falls
 * back to coreward = −x and spinward = +y, flagged by `onAxis` (plan 05, design note D11).
 */
export function localFrameAt(positionLy: Vec3): LocalFrame {
  const radiusLy = Math.hypot(positionLy.x, positionLy.y);
  if (!(radiusLy > AXIS_TOLERANCE_LY)) {
    return AXIS_FRAME;
  }
  return {
    coreward: vec3(-positionLy.x / radiusLy, -positionLy.y / radiusLy, 0),
    spinward: vec3(-positionLy.y / radiusLy, positionLy.x / radiusLy, 0),
    north: NORTH,
    onAxis: false,
  };
}

/** The components of a galactic vector along the frame's coreward, spinward and north. */
export function toLocal(frame: LocalFrame, vector: Vec3): LocalComponents {
  return {
    coreward: dot(vector, frame.coreward),
    spinward: dot(vector, frame.spinward),
    north: dot(vector, frame.north),
  };
}

/** The galactic vector with the given components along the frame's directions. */
export function fromLocal(frame: LocalFrame, components: LocalComponents): Vec3 {
  return add(
    add(scale(frame.coreward, components.coreward), scale(frame.spinward, components.spinward)),
    scale(frame.north, components.north),
  );
}

/** A galactic position's radius, angle and height. On the axis the angle is 0°. */
export function cylindrical(positionLy: Vec3): Cylindrical {
  const radiusLy = Math.hypot(positionLy.x, positionLy.y);
  const angleDeg = (Math.atan2(positionLy.y, positionLy.x) * 180) / Math.PI;
  // atan2 gives (−180°, 180°]; −0 and anything that rounds to 360 both land on 0.
  const wrapped = angleDeg < 0 ? angleDeg + 360 : angleDeg;
  return {
    radiusLy,
    angleDeg: wrapped >= 360 || Object.is(wrapped, -0) ? 0 : wrapped,
    heightLy: positionLy.z,
  };
}
