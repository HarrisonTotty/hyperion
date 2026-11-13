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
 * instead, each fragment by its own offset across the stroke: to the depth that the plane through
 * the stroke's point, parallel to the datum's tangent plane there, has at the fragment's pixel,
 * where that is nearer than the stroke's, and by {@link GROUND_PULL_PX} of the plane's depth slope
 * besides ({@link groundMarkDepth}). The ground beneath the stroke's nearer half stays behind it
 * as far as it lies along that plane, at every grazing angle. Until then every fragment was pulled
 * by the list's `occluderSlopePx` pixels of the slope, 5 at the views' stroke scale of 2, and a far
 * edge showed through a nearer ridge by as much, some 23 km at 2° from 10 km up; now its centre
 * line shows through by a fifth of that, its 2 px stroke by half, and no fragment by more than it
 * did. Every other mark is hidden by the ground in front of it to within the constant fraction:
 * 4.4 m 287 km out.
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
 * The pull every fragment of a mark on the ground takes beside its own offset's, px of the datum's
 * depth slope: 1.
 *
 * @remarks
 * It is for the edge's rim, drawn a little off the drawn ground: its chords between the rim's
 * points stand within `SURVEY_EDGE_SAG_PX` of its curve on the view, and the rim reads its bake's
 * own level's heights, not the morph the terrain draws (`residentHeights.ts`), while the terrain's
 * push is some 0.004 px of the slope 7 km out. With no such pull the line's centre gave way along
 * a tenth of its pixels on TERRAIN TEST's 248 s look, its chords then sagging under the drawn
 * triangles by up to 0.9 px at the 99th percentile; with 1 px, a hundredth (lean, 2026-11-02).
 */
export const GROUND_PULL_PX = 1;

/** A vector on the view, device px: x to the right, y down. */
export interface ViewPx {
  readonly xPx: number;
  readonly yPx: number;
}

/** A gradient of reversed depth on the view, per device px along x to the right and y down. */
export interface DepthGradientPerPx {
  readonly perXPx: number;
  readonly perYPx: number;
}

/**
 * The screen gradient of the reversed depth over the plane through a point parallel to the tangent
 * plane of a body's datum there.
 *
 * @remarks
 * It is n ÷ (Q · N) times the plane's tilt into the screen's two axes, scaled to pixels, y down.
 * Under the reversed, infinite projection the depth over a plane of unit normal N through Q is
 * n (r · N) ÷ (Q · N) along the ray r, whose view-axis component is 1, so it is affine in r, and so
 * in the pixel.
 *
 * @param pointM - The point, m from the camera along the camera frame's axes.
 * @param centreM - The body's centre from the camera, likewise; the plane's normal is the point's
 *   direction from it.
 */
export function groundDepthGradientPerPx(
  pointM: Vec3,
  centreM: Vec3,
  camera: ProjectionCamera,
  viewport: Viewport,
): DepthGradientPerPx {
  const q = toViewAxes(pointM, camera.orientation);
  const n = toViewAxes(normalise(sub(pointM, centreM)), camera.orientation);
  const qn = dot(q, n);
  const heightM = (qn >= 0 ? 1 : -1) * Math.max(Math.abs(qn), 1e-6);
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const aspect = viewport.widthPx / viewport.heightPx;
  return {
    perXPx: ((NEAR_PLANE_M / heightM) * (n.x * 2)) / (viewport.widthPx * s),
    perYPx: ((NEAR_PLANE_M / heightM) * (-n.y * 2)) / (viewport.heightPx * s * aspect),
  };
}

/**
 * The depth a fragment of a mark on the ground is tested at, pulled towards the camera.
 *
 * @remarks
 * Its own, `depth`, the stroke's at the point of its centre line it lies across from, pulled by
 * the plane's depth across `offsetPx`, the fragment's offset from that point, where that is
 * nearer, and by {@link GROUND_PULL_PX} of the plane's depth slope
 * ({@link groundDepthGradientPerPx}), at most the near plane's 1.
 */
export function groundMarkDepth(
  depth: number,
  gradient: DepthGradientPerPx,
  offsetPx: ViewPx,
): number {
  const across = gradient.perXPx * offsetPx.xPx + gradient.perYPx * offsetPx.yPx;
  const slope = Math.hypot(gradient.perXPx, gradient.perYPx);
  return Math.min(depth + GROUND_PULL_PX * slope + Math.max(across, 0), 1);
}
