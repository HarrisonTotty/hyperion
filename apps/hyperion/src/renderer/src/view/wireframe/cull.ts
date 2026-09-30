import { dot, norm, type Vec3 } from "../../geometry/vec3";
import {
  NEAR_PLANE_M,
  type ProjectionCamera,
  toViewAxes,
  type Viewport,
} from "../camera/projection";

/**
 * Whether a sphere can show on a view: it is not wholly outside one of the frustum's side planes or
 * behind the near plane (plan R02, Design note 8: culling is ours, in `f64`).
 *
 * @remarks
 * The far plane of the infinite reversed projection is degenerate, so it is not tested. The test
 * is per plane and conservative: a sphere larger than the frustum, or one the camera is inside, is
 * kept, as the brainstorm's "Culling" asks.
 *
 * @param centreM - The sphere's centre from the camera, m along the camera frame's axes.
 * @param radiusM - Its radius, m (a body's radius, or a Hill sphere, or a mark's bound).
 */
export function sphereInFrustum(
  centreM: Vec3,
  radiusM: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): boolean {
  if (norm(centreM) <= radiusM) {
    return true;
  }
  const v = toViewAxes(centreM, camera.orientation);
  const halfX = camera.fovXRad / 2;
  const halfY = Math.atan(Math.tan(halfX) / (viewport.widthPx / viewport.heightPx));
  // Inward normals of the side planes in view space, where the camera looks down −z.
  const planes: readonly Vec3[] = [
    { x: Math.cos(halfX), y: 0, z: -Math.sin(halfX) },
    { x: -Math.cos(halfX), y: 0, z: -Math.sin(halfX) },
    { x: 0, y: Math.cos(halfY), z: -Math.sin(halfY) },
    { x: 0, y: -Math.cos(halfY), z: -Math.sin(halfY) },
  ];
  if (-v.z < NEAR_PLANE_M - radiusM) {
    return false;
  }
  return planes.every((n) => dot(n, v) >= -radiusM);
}

/** The fraction of the way to a point within which a sphere it lies on does not hide it: 10⁻⁹. */
export const SURFACE_TOLERANCE = 1e-9;

/**
 * Whether a point lies behind a body's limb as seen from the camera: the line of sight to it meets
 * the body's sphere before reaching it (the horizon test of Design note 8). A point on the near
 * surface itself (a landing site) is not behind it.
 *
 * @param pointM - The point from the camera, m.
 * @param centreM - The body's centre from the camera, m.
 * @param radiusM - The body's radius, m.
 */
export function behindLimb(pointM: Vec3, centreM: Vec3, radiusM: number): boolean {
  // |t p − c|² = r²: t² p·p − 2t p·c + c·c − r² = 0, solved for the nearer root in (0, 1).
  const pp = dot(pointM, pointM);
  const pc = dot(pointM, centreM);
  const cc = dot(centreM, centreM) - radiusM * radiusM;
  const discriminant = pc * pc - pp * cc;
  if (!(pp > 0) || discriminant <= 0) {
    return false;
  }
  // The nearer root, in the form that loses no precision when the camera is near the surface.
  const tNear = pc > 0 ? cc / (pc + Math.sqrt(discriminant)) : (pc - Math.sqrt(discriminant)) / pp;
  // A point on the near surface itself meets the sphere at t = 1; rounding must not hide it.
  return tNear > 0 && tNear < 1 - SURFACE_TOLERANCE;
}
