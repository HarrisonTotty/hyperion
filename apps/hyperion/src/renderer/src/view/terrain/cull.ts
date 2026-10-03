/**
 * The culling predicates of patch selection: the view frustum and the horizon (plan R05, T7.a,
 * Design note 8).
 *
 * @remarks
 * Both run on the CPU in `f64` on camera-relative bounds, the body-fixed differences
 * {@link relativeBounds} forms. Both are conservative: a patch is culled only when no part of its
 * bounding volume can be seen.
 */

import { dot, type Vec3, vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { NEAR_PLANE_M } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import type { ViewSize } from "../engine/types";
import { boxCorners, type CameraRelativeBounds } from "./bounds";
import { type BodyFixedVec3, lowestHeightM, type PlanetGeometry } from "./planet";

/** A plane `dot(normal, p) + offsetM ≥ 0` on its inner side, p from the camera, metres. */
export interface Plane {
  /** The inward unit normal, body-fixed. */
  readonly normal: Vec3;
  readonly offsetM: number;
}

/**
 * A view's frustum in camera-relative body-fixed axes: the four side planes and the near plane.
 * The far plane of the reversed infinite projection is degenerate and omitted.
 */
export interface Frustum {
  readonly planes: readonly Plane[];
}

/** A camera as selection sees it: its orientation in the body-fixed axes, field of view and size. */
export interface FrustumCamera {
  /** Rotates camera axes (looking down −z, +y up) into the body-fixed axes. */
  readonly orientation: Quaternion;
  /** The horizontal field of view, rad, in (0, π). */
  readonly fovXRad: number;
  /** The presented size in device pixels. */
  readonly viewport: ViewSize;
}

/** The frustum of `camera`, in the body-fixed axes about the camera. */
export function frustumOf(camera: FrustumCamera): Frustum {
  const halfX = camera.fovXRad / 2;
  const aspect = camera.viewport.widthPx / camera.viewport.heightPx;
  const halfY = Math.atan(Math.tan(halfX) / aspect);
  const inView: readonly Vec3[] = [
    vec3(Math.cos(halfX), 0, -Math.sin(halfX)),
    vec3(-Math.cos(halfX), 0, -Math.sin(halfX)),
    vec3(0, Math.cos(halfY), -Math.sin(halfY)),
    vec3(0, -Math.cos(halfY), -Math.sin(halfY)),
  ];
  const sides = inView.map((n) => ({ normal: rotate(camera.orientation, n), offsetM: 0 }));
  const forward = rotate(camera.orientation, vec3(0, 0, -1));
  return { planes: [...sides, { normal: forward, offsetM: -NEAR_PLANE_M }] };
}

/**
 * Whether a patch can show in the frustum: its bounding sphere, then its oriented box, are not
 * wholly outside any plane. A patch larger than the frustum, and one the camera is inside, pass.
 */
export function inFrustum(b: CameraRelativeBounds, f: Frustum): boolean {
  if (dot(b.centreM, b.centreM) <= b.radiusM * b.radiusM) {
    return true;
  }
  for (const plane of f.planes) {
    if (dot(plane.normal, b.centreM) + plane.offsetM < -b.radiusM) {
      return false;
    }
  }
  const { centre, axes, halfExtentsM } = b.box;
  for (const plane of f.planes) {
    let reach = dot(plane.normal, centre) + plane.offsetM;
    axes.forEach((axis, k) => {
      reach += Math.abs(dot(plane.normal, axis)) * (halfExtentsM[k] ?? 0);
    });
    if (reach < 0) {
      return false;
    }
  }
  return true;
}

/**
 * The horizon's occluder as seen from one camera: the sphere of radius R_occ = c + h_min, the
 * spheroid's polar radius plus the planet's lowest possible height, which lies inside every
 * possible surface (Design note 8; Ring 2013's horizon culling).
 */
export interface HorizonCone {
  /** The camera from the body's centre, body-fixed metres. */
  readonly cameraM: BodyFixedVec3;
  /** R_occ, metres. */
  readonly occluderRadiusM: number;
}

/** The horizon occluder of `planet` for a camera at `cameraM` from the body's centre. */
export function horizonCone(planet: PlanetGeometry, cameraM: BodyFixedVec3): HorizonCone {
  return {
    cameraM,
    occluderRadiusM: planet.figure.polarRadiusM + lowestHeightM(planet),
  };
}

/**
 * Whether a patch can show above the horizon: culled only if all eight corners of its box, which
 * reaches its maximum height, are occluded, which is exact for a convex box (Cozzi and Ring 2011;
 * the Cesium horizon-culling method). Exact tangency is visible, and a camera at or below the
 * occluder's radius disables the test.
 */
export function aboveHorizon(b: CameraRelativeBounds, h: HorizonCone): boolean {
  const r = h.occluderRadiusM;
  // Space scaled by 1 ÷ R_occ, so that the occluder is the unit sphere.
  const c = vec3(h.cameraM.x / r, h.cameraM.y / r, h.cameraM.z / r);
  const horizonSq = dot(c, c) - 1;
  if (!(horizonSq > 0)) {
    return true;
  }
  for (const corner of boxCorners(b.box)) {
    // The corner from the camera, scaled; the difference was formed once, in f64, unscaled.
    const vt = vec3(corner.x / r, corner.y / r, corner.z / r);
    const vtDotVc = -dot(vt, c);
    const occluded = vtDotVc > horizonSq && (vtDotVc * vtDotVc) / dot(vt, vt) > horizonSq;
    if (!occluded) {
      return true;
    }
  }
  return false;
}
