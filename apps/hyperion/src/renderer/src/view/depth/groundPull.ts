/**
 * The depth a mark on a body's ground is tested at (plan R10, R10.T16.c's grazing fix):
 * `lineStroke.wgsl`'s `line_depth`, its twin in `f64`.
 *
 * @remarks
 * The terrain's depth, in the symbology's pass over the photorealistic image (`terrainDepth.wgsl`)
 * and in the wireframe (`terrainWire.wgsl`), is pushed away from the camera by
 * {@link TERRAIN_DEPTH_FRACTION} of itself alone. Until this fix it was pushed as the occluders'
 * is (R07.T16.d), by `occluderSlopePx` pixels of its screen slope as well. Over grazing ground that
 * slope term grows as the cotangent of the grazing angle, about 1.8 × 10⁻³ cot θ of the range at
 * 1080p across 60°: 14.8 km 287 km out from 10 km up at 2° over a plane. An orbit, a graticule or a
 * survey edge that far behind a ridge showed through it.
 *
 * A metre bound on that term could not stand instead (lean, 2026-11-01). A stroke is drawn at one
 * depth across its width, and over grazing ground the ground beneath its nearer half stands nearer
 * by about its half-width times R α ÷ sin θ (R the range, α a pixel's angle). That is 160 m for the
 * survey edge's 2.5 px 10 km out from 1 km up at 1080p, and some 10 km 287 km out from 10 km up.
 * Any bound small enough to keep a mark 1 km behind a ridge hidden hides that half of the edge.
 *
 * So the one mark drawn on the ground, the survey edge, pulls its own depth towards the camera
 * instead. It is pulled by `occluderSlopePx` pixels of the depth slope the datum's tangent plane
 * has at its point, which is the push the ground under it had, and the edge stays whole at every
 * grazing angle. Every other mark is hidden by the ground in front of it to within the constant
 * fraction: 4.4 m 287 km out.
 */

import { dot, normalise, sub, type Vec3 } from "../../geometry/vec3";
import {
  NEAR_PLANE_M,
  type ProjectionCamera,
  toViewAxes,
  type Viewport,
} from "../camera/projection";

/**
 * The terrain's push away from the camera, a fraction of its depth: 2⁻¹⁶, the occluders' constant
 * (`occluder.wgsl`'s DEPTH_FRACTION; `terrainDepth.wgsl`'s and `terrainWire.wgsl`'s), now its whole
 * push.
 */
export const TERRAIN_DEPTH_FRACTION = 2 ** -16;

/**
 * The screen slope, per device px, of the reversed depth over the tangent plane of a body's datum
 * at a point: n ÷ |Q · N| times the plane's tilt into the screen's two axes, scaled to pixels.
 *
 * @remarks
 * Under the reversed, infinite projection the depth over a plane of unit normal N through Q is
 * n (r · N) ÷ (Q · N) along the ray r, whose view-axis component is 1, so it is affine in r.
 *
 * @param pointM - The point, m from the camera along the camera frame's axes.
 * @param centreM - The body's centre from the camera, likewise; the plane's normal is the point's
 *   direction from it.
 */
export function groundDepthSlopePerPx(
  pointM: Vec3,
  centreM: Vec3,
  camera: ProjectionCamera,
  viewport: Viewport,
): number {
  const q = toViewAxes(pointM, camera.orientation);
  const n = toViewAxes(normalise(sub(pointM, centreM)), camera.orientation);
  const heightM = Math.max(Math.abs(dot(q, n)), 1e-6);
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const aspect = viewport.widthPx / viewport.heightPx;
  const tiltX = (n.x * 2) / (viewport.widthPx * s);
  const tiltY = (n.y * 2) / (viewport.heightPx * s * aspect);
  return (NEAR_PLANE_M / heightM) * Math.hypot(tiltX, tiltY);
}

/**
 * The depth a fragment of a mark on the ground is tested at: its own, `depth`, pulled towards the
 * camera by `occluderSlopePx` pixels of the ground's depth slope ({@link groundDepthSlopePerPx}),
 * at most the near plane's 1.
 */
export function groundMarkDepth(
  depth: number,
  slopePerPx: number,
  occluderSlopePx: number,
): number {
  return Math.min(depth + occluderSlopePx * slopePerPx, 1);
}
