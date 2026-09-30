/**
 * A vector in three dimensions, in whatever unit and frame its user states.
 *
 * @remarks
 * The spatial view knows nothing about stars: callers name the unit (`relLy`, `positionKm`).
 */
export interface Vec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/** Builds a {@link Vec3} from its components. */
export function vec3(x: number, y: number, z: number): Vec3 {
  return { x, y, z };
}

/** The sum `a + b`. */
export function add(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z };
}

/** The difference `a − b`. */
export function sub(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z };
}

/** The vector `v` multiplied by the number `factor`. */
export function scale(v: Vec3, factor: number): Vec3 {
  return { x: v.x * factor, y: v.y * factor, z: v.z * factor };
}

/** The dot product `a · b`. */
export function dot(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

/** The cross product `a × b`, right-handed. */
export function cross(a: Vec3, b: Vec3): Vec3 {
  return {
    x: a.y * b.z - a.z * b.y,
    y: a.z * b.x - a.x * b.z,
    z: a.x * b.y - a.y * b.x,
  };
}

/** The Euclidean length of `v`. */
export function norm(v: Vec3): number {
  return Math.hypot(v.x, v.y, v.z);
}

/**
 * The unit vector along `v`.
 *
 * @throws RangeError when `v` has no length, which has no direction.
 */
export function normalise(v: Vec3): Vec3 {
  const length = norm(v);
  if (!(length > 0) || !Number.isFinite(length)) {
    throw new RangeError(`cannot normalise a vector of length ${String(length)}`);
  }
  return scale(v, 1 / length);
}
