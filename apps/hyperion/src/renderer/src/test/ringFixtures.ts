/**
 * A ring for the view's tests (plan R11, R11.T9): Saturn-like, of porous ice, with one gap, about a
 * body, its geometry from the caller.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import type { Vec3 } from "../geometry/vec3";
import type { ViewRing } from "../view/scene/model";

/** A massive ring of porous ice about `body`, τ 1.5, one gap 0.8 of the way out. */
export function aViewRing(
  body: BodyIdHex,
  innerRadiusM: number,
  outerRadiusM: number,
  normal: Vec3,
): ViewRing {
  return {
    body,
    ringKind: "massive",
    material: "porous_ice",
    innerRadiusM,
    outerRadiusM,
    opticalDepth: 1.5,
    gapRadiiM: [innerRadiusM + 0.8 * (outerRadiusM - innerRadiusM)],
    normal,
  };
}
