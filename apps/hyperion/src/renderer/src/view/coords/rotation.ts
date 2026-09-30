import { cross, dot, type Vec3 } from "../../geometry/vec3";

/**
 * The largest departure from orthonormality {@link rotation3FromRows} accepts: every entry of
 * R Rᵀ within this of the identity's, as the simulation's `ROTATION_ORTHONORMAL_TOLERANCE`.
 */
export const ROTATION_ORTHONORMAL_TOLERANCE = 1e-12;

/**
 * A body's rotation from its body-fixed axes to its non-rotating body frame, at one moment: the
 * client's mirror of the simulation's `coords::BodyFixedRotation`.
 *
 * @remarks
 * Row-major `f64` rows; its columns are the body-fixed axes expressed in the body frame, so
 * {@link rotateToBody} is R · p and {@link rotateToBodyFixed} is Rᵀ · p. It is built only by
 * {@link rotation3FromRows}, which checks that it is a rotation, and carries that proof in its
 * brand.
 */
export interface Rotation3 {
  /** The three rows, each a {@link Vec3}. */
  readonly rows: readonly [Vec3, Vec3, Vec3];
  /** The brand that only {@link rotation3FromRows} gives. */
  readonly __rotation3: true;
}

/**
 * Builds a {@link Rotation3} from its rows, checking that it is a proper rotation.
 *
 * @throws RangeError if an entry is not finite, if some entry of R Rᵀ departs from the identity's
 * by more than {@link ROTATION_ORTHONORMAL_TOLERANCE}, or if the determinant is below zero (a
 * reflection).
 */
export function rotation3FromRows(rows: readonly [Vec3, Vec3, Vec3]): Rotation3 {
  for (const row of rows) {
    if (!(Number.isFinite(row.x) && Number.isFinite(row.y) && Number.isFinite(row.z))) {
      throw new RangeError("rotation matrix has an entry that is not finite");
    }
  }
  for (const [i, a] of rows.entries()) {
    for (const [j, b] of rows.entries()) {
      const expected = i === j ? 1 : 0;
      if (Math.abs(dot(a, b) - expected) > ROTATION_ORTHONORMAL_TOLERANCE) {
        throw new RangeError(
          `rotation matrix is not orthonormal to ${String(ROTATION_ORTHONORMAL_TOLERANCE)}`,
        );
      }
    }
  }
  if (dot(rows[0], cross(rows[1], rows[2])) < 0) {
    throw new RangeError("rotation matrix is a reflection (determinant below 0)");
  }
  return { rows, __rotation3: true };
}

/** No rotation: the body-fixed axes are the body frame's. */
export const IDENTITY_ROTATION: Rotation3 = rotation3FromRows([
  { x: 1, y: 0, z: 0 },
  { x: 0, y: 1, z: 0 },
  { x: 0, y: 0, z: 1 },
]);

/** R · p: a body-fixed vector's components in the body frame, each a row's dot product in x, y, z order. */
export function rotateToBody(rotation: Rotation3, p: Vec3): Vec3 {
  const [r0, r1, r2] = rotation.rows;
  return {
    x: r0.x * p.x + r0.y * p.y + r0.z * p.z,
    y: r1.x * p.x + r1.y * p.y + r1.z * p.z,
    z: r2.x * p.x + r2.y * p.y + r2.z * p.z,
  };
}

/** Rᵀ · p: a body-frame vector's components along the body-fixed axes, in x, y, z order. */
export function rotateToBodyFixed(rotation: Rotation3, p: Vec3): Vec3 {
  const [r0, r1, r2] = rotation.rows;
  return {
    x: r0.x * p.x + r1.x * p.y + r2.x * p.z,
    y: r0.y * p.x + r1.y * p.y + r2.y * p.z,
    z: r0.z * p.x + r1.z * p.y + r2.z * p.z,
  };
}
