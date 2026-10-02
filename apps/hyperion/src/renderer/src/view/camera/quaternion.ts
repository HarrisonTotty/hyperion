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

/**
 * The unit quaternion of a rotation matrix given by its rows (Shepperd's method, which divides by
 * the largest of the four squared components so that no branch loses precision).
 *
 * @remarks
 * The inverse of {@link rotationRows}. The rows must form a proper rotation; the result is
 * normalised, so rounding in an orthonormal input is absorbed.
 */
export function quaternionFromRows(rows: readonly [Vec3, Vec3, Vec3]): Quaternion {
  const [r0, r1, r2] = rows;
  const trace = r0.x + r1.y + r2.z;
  if (trace >= r0.x && trace >= r1.y && trace >= r2.z) {
    const s = 2 * Math.sqrt(1 + trace);
    return quaternion(s / 4, (r2.y - r1.z) / s, (r0.z - r2.x) / s, (r1.x - r0.y) / s);
  }
  if (r0.x >= r1.y && r0.x >= r2.z) {
    const s = 2 * Math.sqrt(1 + r0.x - r1.y - r2.z);
    return quaternion((r2.y - r1.z) / s, s / 4, (r0.y + r1.x) / s, (r0.z + r2.x) / s);
  }
  if (r1.y >= r2.z) {
    const s = 2 * Math.sqrt(1 + r1.y - r0.x - r2.z);
    return quaternion((r0.z - r2.x) / s, (r0.y + r1.x) / s, s / 4, (r1.z + r2.y) / s);
  }
  const s = 2 * Math.sqrt(1 + r2.z - r0.x - r1.y);
  return quaternion((r1.x - r0.y) / s, (r0.z + r2.x) / s, (r1.z + r2.y) / s, s / 4);
}

/** The unit vector perpendicular to `hint` and `back`, `hint × back` normalised, or `null`. */
function rightOf(hint: Vec3, back: Vec3): Vec3 | null {
  const x = {
    x: hint.y * back.z - hint.z * back.y,
    y: hint.z * back.x - hint.x * back.z,
    z: hint.x * back.y - hint.y * back.x,
  };
  const length = Math.hypot(x.x, x.y, x.z);
  return length > 1e-9 ? { x: x.x / length, y: x.y / length, z: x.z / length } : null;
}

/**
 * The orientation of a camera looking along `forward` with its up axis as near `up` as the
 * forward direction allows.
 *
 * @remarks
 * The camera looks down its own −z with +y up (plan R02, Design note 4), so the result's z column
 * is −forward, its x column forward × up normalised, and its y column z × x. Where `up` is
 * parallel to `forward`, the frame's +z, or failing that its +y, stands in for it.
 *
 * @throws RangeError if `forward` has no length.
 */
export function lookAlong(forward: Vec3, up: Vec3): Quaternion {
  const f = Math.hypot(forward.x, forward.y, forward.z);
  if (!(Number.isFinite(f) && f > 0)) {
    throw new RangeError("a look direction must have a length");
  }
  const z = { x: -forward.x / f, y: -forward.y / f, z: -forward.z / f };
  const x = rightOf(up, z) ?? rightOf({ x: 0, y: 0, z: 1 }, z) ?? rightOf({ x: 0, y: 1, z: 0 }, z);
  if (x === null) {
    throw new RangeError("no up axis is perpendicular to the look direction");
  }
  const y = { x: z.y * x.z - z.z * x.y, y: z.z * x.x - z.x * x.z, z: z.x * x.y - z.y * x.x };
  return quaternionFromRows([
    { x: x.x, y: y.x, z: z.x },
    { x: x.y, y: y.y, z: z.y },
    { x: x.z, y: y.z, z: z.z },
  ]);
}

/**
 * The rotation part of the way from `a` to `b` along the shorter arc (spherical linear
 * interpolation), `t` 0 giving `a` and 1 giving `b`.
 */
export function slerp(a: Quaternion, b: Quaternion, t: number): Quaternion {
  const dotAB = a.w * b.w + a.x * b.x + a.y * b.y + a.z * b.z;
  // q and −q are one rotation; b is flipped onto a's hemisphere so that the arc is the shorter.
  const sign = dotAB < 0 ? -1 : 1;
  const cosine = Math.min(1, sign * dotAB);
  if (cosine > 1 - 1e-12) {
    return quaternion(
      a.w + (sign * b.w - a.w) * t,
      a.x + (sign * b.x - a.x) * t,
      a.y + (sign * b.y - a.y) * t,
      a.z + (sign * b.z - a.z) * t,
    );
  }
  const angle = Math.acos(cosine);
  const wa = Math.sin((1 - t) * angle) / Math.sin(angle);
  const wb = (sign * Math.sin(t * angle)) / Math.sin(angle);
  return quaternion(
    wa * a.w + wb * b.w,
    wa * a.x + wb * b.x,
    wa * a.y + wb * b.y,
    wa * a.z + wb * b.z,
  );
}
