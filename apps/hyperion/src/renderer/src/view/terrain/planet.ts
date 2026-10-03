/**
 * The planet's geometry as selection reads it (plan R05, T7.a).
 */

import type { Vec3 } from "../../geometry/vec3";

/**
 * A vector in a body's rotating, body-fixed axes (z along the pole), in metres unless its name says
 * otherwise; positions reach R02 as a `ViewPosition` of kind `body_fixed`.
 */
export type BodyFixedVec3 = Vec3;
