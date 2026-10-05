/**
 * How each lit body is drawn on a view: a point, an analytic disc or a smooth mesh (plan R07,
 * Design notes 1 and 2).
 *
 * @remarks
 * A body is a `point` while its angular diameter is under {@link POINT_BELOW_PX} of the view's own
 * pixels (brainstorm, "The scales the view spans"), a `disc` above, and a `mesh` only where depth
 * against other geometry requires it (Design note 2's promotion) or R10 hands it to terrain. Each
 * threshold has {@link REGIME_HYSTERESIS}: a point becomes a disc at 3.3 px and a disc a point at
 * 2.7 px, so a body at the boundary does not flip every frame. The set is a function of the bodies,
 * the camera, the viewport and the previous set only, each body's regime its own, so input order
 * cannot change it. An oblate body is sized on its equatorial radius.
 */
import type { BodyIdHex } from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import {
  NEAR_PLANE_M,
  type ProjectionCamera,
  toViewAxes,
  type Viewport,
} from "../camera/projection";
import { angularDiameterPx } from "../wireframe/bodies";
import { sphereScreenRect } from "../wireframe/submit";

/** How a lit body is drawn. */
export type LitRegime = "point" | "disc" | "mesh";

/** Below this angular diameter, px, a body is a point (brainstorm, "The scales the view spans"). */
export const POINT_BELOW_PX = 3;

/**
 * The distance inside which R08 runs a gas giant's full atmosphere passes, m (Design note 1): ten
 * scale heights spanning 2 px, 2.2 × 10⁸ m for Jupiter and 4.9 × 10⁸ m for Saturn at 1080p across
 * 60° at R02's centre-pixel scale (2.5 and 5.5 × 10⁸ m at the brainstorm's width ÷ field), rounded
 * up to one boundary.
 */
export const GAS_GIANT_FULL_PASS_BOUNDARY_M = 1e9;

/** Each threshold's hysteresis, a fraction of it (Design note 1). */
export const REGIME_HYSTERESIS = 0.1;

/** A lit body as the regime and the painter order see it. */
export interface LitSphere {
  readonly id: BodyIdHex;
  /** Its centre from the camera, m (`f64`). */
  readonly centreM: Vec3;
  /** Its equatorial radius, m: the bounding sphere of an oblate body. */
  readonly radiusM: number;
}

/** The point–disc regime of one body from its diameter and its previous regime. */
function pointOrDisc(diameterPx: number, previous: LitRegime | undefined): "point" | "disc" {
  const wasPoint = previous === undefined || previous === "point";
  const threshold = POINT_BELOW_PX * (wasPoint ? 1 + REGIME_HYSTERESIS : 1 - REGIME_HYSTERESIS);
  return diameterPx < threshold ? "point" : "disc";
}

/**
 * Each body's point or disc regime on a view, before Design note 2's promotion to `mesh`.
 *
 * @remarks
 * A body seen from inside its sphere is a disc. A body that was a `mesh` keeps the disc side of the
 * hysteresis; whether it stays a mesh is {@link promoteOverlapping}'s to decide each frame.
 *
 * @param previous - Each body's regime on the previous frame; a body absent from it starts as a
 *   point, so that it must clear 3.3 px to become a disc.
 */
export function litRegimes(
  bodies: ReadonlyArray<LitSphere>,
  camera: ProjectionCamera,
  viewport: Viewport,
  previous: ReadonlyMap<BodyIdHex, LitRegime>,
): Map<BodyIdHex, LitRegime> {
  const regimes = new Map<BodyIdHex, LitRegime>();
  for (const body of bodies) {
    const diameterPx = angularDiameterPx(body.centreM, body.radiusM, camera, viewport);
    regimes.set(body.id, pointOrDisc(diameterPx, previous.get(body.id)));
  }
  return regimes;
}

/** A screen footprint, px: a circle about the projected centre bounding the drawn body. */
export interface ScreenCircle {
  readonly xPx: number;
  readonly yPx: number;
  readonly radiusPx: number;
}

/**
 * A sphere's footprint on a view: the circle about its screen rectangle (`sphereScreenRect`, its
 * silhouette's bound with a margin, clamped to the view) through the rectangle's corners, or `null`
 * where it is off the view or wholly behind the near plane (where `sphereScreenRect` gives the
 * whole view). It bounds the silhouette, so that promotion may promote a disc that overlaps
 * nothing, never miss one that does (T5 as built).
 *
 * @param centreM - The sphere's centre from the camera, m (`f64`).
 */
export function sphereFootprint(
  centreM: Vec3,
  radiusM: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): ScreenCircle | null {
  if (-toViewAxes(centreM, camera.orientation).z + radiusM < NEAR_PLANE_M) {
    return null;
  }
  const rect = sphereScreenRect(centreM, radiusM, camera, viewport);
  if (rect === null) {
    return null;
  }
  return {
    xPx: (rect.leftPx + rect.rightPx) / 2,
    yPx: (rect.topPx + rect.bottomPx) / 2,
    radiusPx: Math.hypot(rect.rightPx - rect.leftPx, rect.bottomPx - rect.topPx) / 2,
  };
}

/** Whether two footprints overlap. */
function overlaps(a: ScreenCircle, b: ScreenCircle): boolean {
  return Math.hypot(a.xPx - b.xPx, a.yPx - b.yPx) < a.radiusPx + b.radiusPx;
}

/**
 * Promotes to `mesh` every disc whose footprint overlaps geometry that writes depth: a mesh body,
 * R10's terrain, a craft (Design note 2), so that the depth test orders them there.
 *
 * @remarks
 * Promotion spreads: a disc overlapping a newly promoted mesh is promoted too, until nothing
 * changes. The result depends on the sets only, not their order.
 *
 * @param footprints - Each disc's and mesh's footprint on the view.
 * @param depthWriters - The footprints of the view's other depth-writing geometry.
 */
export function promoteOverlapping(
  regimes: ReadonlyMap<BodyIdHex, LitRegime>,
  footprints: ReadonlyMap<BodyIdHex, ScreenCircle>,
  depthWriters: ReadonlyArray<ScreenCircle>,
): Map<BodyIdHex, LitRegime> {
  const promoted = new Map(regimes);
  let changed = true;
  while (changed) {
    changed = false;
    const writers = [
      ...depthWriters,
      ...[...promoted]
        .filter(([, regime]) => regime === "mesh")
        .flatMap(([id]) => {
          const footprint = footprints.get(id);
          return footprint === undefined ? [] : [footprint];
        }),
    ];
    for (const [id, regime] of promoted) {
      const footprint = footprints.get(id);
      if (regime !== "disc" || footprint === undefined) {
        continue;
      }
      if (writers.some((writer) => overlaps(footprint, writer))) {
        promoted.set(id, "mesh");
        changed = true;
      }
    }
  }
  return promoted;
}
