import type { Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "./pose";

/**
 * Builds a unit {@link Quaternion} from its components, normalising them.
 *
 * @throws RangeError if the components are not finite or are all zero, which is no rotation.
 */
export function quaternion(w: number, x: number, y: number, z: number): Quaternion {
  const length = Math.hypot(w, x, y, z);
  if (!(Number.isFinite(length) && length > 0)) {
    throw new RangeError(`cannot normalise a quaternion of length ${String(length)}`);
  }
  return { w: w / length, x: x / length, y: y / length, z: z / length };
}

/** No rotation: the camera's axes are its frame's. */
export const IDENTITY_QUATERNION: Quaternion = { w: 1, x: 0, y: 0, z: 0 };

/**
 * The rotation by `angleRad` about `axis`, right-handed (counter-clockwise looking down the axis).
 *
 * @throws RangeError if `axis` has no length.
 */
export function quaternionFromAxisAngle(axis: Vec3, angleRad: number): Quaternion {
  const length = Math.hypot(axis.x, axis.y, axis.z);
  if (!(Number.isFinite(length) && length > 0)) {
    throw new RangeError("a rotation axis must have a length");
  }
  const s = Math.sin(angleRad / 2) / length;
  return quaternion(Math.cos(angleRad / 2), axis.x * s, axis.y * s, axis.z * s);
}

/** The product `a · b`: the rotation `b` followed by `a`. */
export function multiply(a: Quaternion, b: Quaternion): Quaternion {
  return quaternion(
    a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
    a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
    a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
  );
}

/** The inverse rotation of a unit quaternion. */
export function conjugate(q: Quaternion): Quaternion {
  return { w: q.w, x: -q.x, y: -q.y, z: -q.z };
}

/**
 * The rotation matrix of `q`, row-major: its columns are the rotated x, y and z axes.
 *
 * @remarks
 * For a camera's orientation the columns are the camera's right, up and backward axes in its
 * frame, since the camera looks down its own −z.
 */
export function rotationRows(q: Quaternion): readonly [Vec3, Vec3, Vec3] {
  const { w, x, y, z } = q;
  return [
    { x: 1 - 2 * (y * y + z * z), y: 2 * (x * y - w * z), z: 2 * (x * z + w * y) },
    { x: 2 * (x * y + w * z), y: 1 - 2 * (x * x + z * z), z: 2 * (y * z - w * x) },
    { x: 2 * (x * z - w * y), y: 2 * (y * z + w * x), z: 1 - 2 * (x * x + y * y) },
  ];
}

/** `v` rotated by `q`: a camera-axes vector's components in the camera's frame. */
export function rotate(q: Quaternion, v: Vec3): Vec3 {
  const [r0, r1, r2] = rotationRows(q);
  return {
    x: r0.x * v.x + r0.y * v.y + r0.z * v.z,
    y: r1.x * v.x + r1.y * v.y + r1.z * v.z,
    z: r2.x * v.x + r2.y * v.y + r2.z * v.z,
  };
}
