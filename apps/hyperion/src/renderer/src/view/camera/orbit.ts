/**
 * The `ORBIT` camera (plan R15, T4): held about one body, from far out down to its terrain.
 *
 * @remarks
 * - **Its pose** is held in the body's frame, whose origin is the body's centre and whose axes are
 *   the galactic axes, so that the orbit's centre is the frame's origin.
 * - **A drag** turns the camera about the centre, its look with it: the ground under the pointer
 *   follows it. The turn is the drag's orbit turn scaled by the altitude over the distance from the
 *   centre, so that far out a drag swings the camera round the body and close in it pans the
 *   camera over the ground.
 * - **Its altitude** is measured above the body's reference spheroid, its datum, along the camera's
 *   line from the centre ({@link orbitRadiusM}), so that an oblate body's ground is reached at every
 *   latitude: the radius each function takes is the datum's along that line.
 * - **Zoom** steps the altitude above the datum by {@link ORBIT_ZOOM_FACTOR} a step, between the
 *   floor ({@link orbitFloorM}: {@link ORBIT_FLOOR_M} above the drawn surface under the camera, its
 *   ground or a sea's surface over it, or above the datum where none is drawn) and
 *   {@link ORBIT_CEILING_RADII} radii, along the line from the centre; the look is kept.
 * - **The floor holds** each frame, the zoom's floor at no step: a camera that ground drawn since
 *   stands over, or that a drag or the arrows bring over higher ground, is lifted to the floor at
 *   once along its line from the centre, its look kept, since a camera inside rock is worse than a
 *   jump.
 * - **Tilt and heading** turn the look about the camera's right axis and the local vertical, the
 *   tilt held between straight down at the centre and the horizon, so that close in the camera
 *   looks out along the terrain.
 * - **The ground turns with it**: each frame the camera is turned with the body's rotation over
 *   the frame (`coRotated`), so that it hovers over one place on the ground as the body spins.
 */
import { cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { type Rotation3, rotateToBody, rotateToBodyFixed } from "../coords/rotation";
import type { ViewTurn } from "./look";
import type { CameraPose, Quaternion } from "./pose";
import { lookAlong, multiply, quaternionFromAxisAngle, rotate } from "./quaternion";

/** The factor one zoom step scales the altitude by: a wheel's notch, 1.25. */
export const ORBIT_ZOOM_FACTOR = 1.25;

/**
 * The least height above the drawn surface under the camera, m: 10, the guide's lowest look at the
 * ground; the surface is the drawn ground, or a sea's where the ground lies under it, and the datum
 * where none is drawn ({@link orbitFloorM}).
 */
export const ORBIT_FLOOR_M = 10;

/**
 * The floor's altitude above the datum, m, over a drawn surface `surfaceAboveDatumM` above it (0,
 * the datum, where none is drawn): {@link ORBIT_FLOOR_M} above it.
 */
export function orbitFloorM(surfaceAboveDatumM: number): number {
  return surfaceAboveDatumM + ORBIT_FLOOR_M;
}

/** A body's reference spheroid as `ORBIT` reads it: its radii and its pole along the galactic axes. */
export interface OrbitDatum {
  /** The equatorial radius, m, positive. */
  readonly equatorialRadiusM: number;
  /** The polar radius, m, positive and at most the equatorial. */
  readonly polarRadiusM: number;
  /** The spheroid's axis, a unit vector along the galactic axes; `null` for a sphere's. */
  readonly pole: Vec3 | null;
}

/**
 * The datum's radius along the line from the centre through `positionM`, m: where that line meets
 * the spheroid, 1 ÷ √(cos²φ ÷ a² + sin²φ ÷ c²) at the line's latitude φ; the equatorial radius
 * at the centre itself or for a body with no pole.
 */
export function orbitRadiusM(datum: OrbitDatum, positionM: Vec3): number {
  const distanceM = norm(positionM);
  if (datum.pole === null || !(distanceM > 0)) {
    return datum.equatorialRadiusM;
  }
  const sin = dot(positionM, datum.pole) / distanceM;
  const cos2 = Math.max(1 - sin * sin, 0);
  return 1 / Math.sqrt(cos2 / datum.equatorialRadiusM ** 2 + (sin * sin) / datum.polarRadiusM ** 2);
}

/** The greatest distance from the centre, in body radii: 50, where a body still fills a few px. */
export const ORBIT_CEILING_RADII = 50;

/** The distance from the centre a camera starts its orbit at where it stood inside it: 3 radii. */
export const ORBIT_START_RADII = 3;

/** The camera's own axes: +x right, +y up, −z forward, the hull convention. */
const CAMERA_RIGHT = vec3(1, 0, 0);
const CAMERA_UP = vec3(0, 1, 0);
const CAMERA_FORWARD = vec3(0, 0, -1);

/** The tilt from the downward vertical the look is held within, rad: up to the horizon. */
const MAX_TILT_RAD = Math.PI / 2;

/** Galactic north, the up that stands in where the look is along the vertical. */
const NORTH = vec3(0, 0, 1);

/** `pose`'s distance from the orbit's centre, m. */
export function orbitDistanceM(pose: CameraPose): number {
  return norm(pose.positionM);
}

/** `pose`'s altitude above a body of `radiusM`, m. */
export function orbitAltitudeM(pose: CameraPose, radiusM: number): number {
  return norm(pose.positionM) - radiusM;
}

/** A unit vector across `v`, for a look that stands along the vertical. */
function across(v: Vec3): Vec3 {
  const trial = Math.abs(dot(normalise(v), NORTH)) < 0.9 ? NORTH : vec3(1, 0, 0);
  return normalise(cross(v, trial));
}

/**
 * The orbit's start from `pose`, in the frame `frame` of a body of `radiusM`: where it stands,
 * looking at the centre with its up kept as near as it can be, or at {@link ORBIT_START_RADII}
 * radii along its line from the centre where it stood inside {@link ORBIT_FLOOR_M} of the surface,
 * and within {@link ORBIT_CEILING_RADII} radii.
 */
export function orbitStart(
  pose: CameraPose,
  frame: CameraPose["frame"],
  radiusM: number,
): CameraPose {
  const distanceM = norm(pose.positionM);
  const outward = distanceM > 0 ? scale(pose.positionM, 1 / distanceM) : vec3(1, 0, 0);
  const held =
    distanceM < radiusM + ORBIT_FLOOR_M
      ? ORBIT_START_RADII * radiusM
      : Math.min(distanceM, ORBIT_CEILING_RADII * radiusM);
  const positionM = scale(outward, held);
  const up = rotate(pose.orientation, CAMERA_UP);
  const upAcross = sub(up, scale(outward, dot(up, outward)));
  return {
    frame,
    positionM,
    orientation: lookAlong(
      scale(outward, -1),
      norm(upAcross) > 1e-9 ? normalise(upAcross) : across(outward),
    ),
  };
}

/** `pose` turned about the orbit's centre by `rotation`: its position and its look together. */
function turnedAboutCentre(pose: CameraPose, rotation: Quaternion): CameraPose {
  return {
    ...pose,
    positionM: rotate(rotation, pose.positionM),
    orientation: multiply(rotation, pose.orientation),
  };
}

/**
 * `pose` after a drag's orbit turn `turn` (`orbitTurn`) about a body of `radiusM`: turned about the
 * centre on the camera's up and right axes by the turn scaled by its altitude over its distance, so
 * that the ground follows the pointer close in and the body swings round far out.
 */
export function orbitDragged(pose: CameraPose, turn: ViewTurn, radiusM: number): CameraPose {
  const distanceM = norm(pose.positionM);
  if (!(distanceM > 0)) {
    return pose;
  }
  const share = Math.max(distanceM - radiusM, 0) / distanceM;
  const up = rotate(pose.orientation, CAMERA_UP);
  const right = rotate(pose.orientation, CAMERA_RIGHT);
  // A drag to the right swings the camera left round the centre, so that the ground moves with it.
  const yaw = quaternionFromAxisAngle(up, turn.yawRad * share);
  const pitch = quaternionFromAxisAngle(right, turn.pitchRad * share);
  return turnedAboutCentre(pose, multiply(pitch, yaw));
}

/**
 * `pose` zoomed `steps` steps about a body of `radiusM` over a drawn surface `surfaceM` above the
 * datum (0 where none is drawn): its altitude above the datum, or above that surface where it lies
 * below the datum, scaled by {@link ORBIT_ZOOM_FACTOR} to the `-steps` power, in nearer for a
 * positive step, held between the floor ({@link orbitFloorM}) and the ceiling, along its line from
 * the centre; at no step, lifted to the floor where it stands under it.
 *
 * @remarks
 * A scale of the altitude above the datum comes down towards the datum, never under it, so over a
 * dry basin it scales the height above the basin's floor instead, and comes down to its floor
 * there too; over the datum and higher ground it scales the altitude above the datum as before.
 */
export function orbitZoomed(
  pose: CameraPose,
  steps: number,
  radiusM: number,
  surfaceM: number,
): CameraPose {
  const distanceM = norm(pose.positionM);
  if (!(distanceM > 0)) {
    return pose;
  }
  const baseM = Math.min(surfaceM, 0);
  const floorM = orbitFloorM(surfaceM) - baseM;
  const heightM = Math.max(distanceM - radiusM - baseM, floorM);
  const next = Math.min(
    Math.max(heightM * ORBIT_ZOOM_FACTOR ** -steps, floorM),
    ORBIT_CEILING_RADII * radiusM - radiusM - baseM,
  );
  return { ...pose, positionM: scale(pose.positionM, (radiusM + baseM + next) / distanceM) };
}

/** The look's tilt from straight down at the centre, rad. */
export function orbitTiltRad(pose: CameraPose): number {
  const down = normalise(scale(pose.positionM, -1));
  const forward = rotate(pose.orientation, CAMERA_FORWARD);
  return Math.acos(Math.min(Math.max(dot(forward, down), -1), 1));
}

/**
 * `pose` with its look tilted by `tiltRad` (positive towards the horizon) about its right axis and
 * turned by `headingRad` about the local vertical, where it stands; a tilt that would pass the
 * horizon or straight down is held at it.
 */
export function orbitLooked(pose: CameraPose, tiltRad: number, headingRad: number): CameraPose {
  const distanceM = norm(pose.positionM);
  if (!(distanceM > 0)) {
    return pose;
  }
  const vertical = scale(pose.positionM, 1 / distanceM);
  const turned = multiply(quaternionFromAxisAngle(vertical, headingRad), pose.orientation);
  const tilt = orbitTiltRad({ ...pose, orientation: turned });
  const held = Math.min(Math.max(tilt + tiltRad, 0), MAX_TILT_RAD) - tilt;
  const right = rotate(turned, CAMERA_RIGHT);
  return {
    ...pose,
    orientation: multiply(quaternionFromAxisAngle(right, held), turned),
  };
}

/**
 * `pose` turned with its body's spin from `before` to `after`, the body's rotations from body-fixed
 * to body axes at the last frame and at this one: the camera hovers over one place of the ground as
 * the body turns, its look turned with it.
 */
export function coRotated(pose: CameraPose, before: Rotation3, after: Rotation3): CameraPose {
  const carried = (v: Vec3): Vec3 => rotateToBody(after, rotateToBodyFixed(before, v));
  return {
    ...pose,
    positionM: carried(pose.positionM),
    orientation: lookAlong(
      carried(rotate(pose.orientation, CAMERA_FORWARD)),
      carried(rotate(pose.orientation, CAMERA_UP)),
    ),
  };
}
