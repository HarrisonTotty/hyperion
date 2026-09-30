import type { Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "./pose";
import { rotationRows } from "./quaternion";

/**
 * The near plane, m: 0.1 (plan R02, Design note 4).
 *
 * @remarks
 * It keeps a hull plate at 1 m well inside the frustum and costs nothing far away: under the
 * reversed, infinite projection the depth at 1 au is 6.7 × 10⁻¹³, a normal `f32` spaced at about
 * 1.2 × 10⁻⁷ of itself (Reed 2015, "Depth Precision Visualized").
 */
export const NEAR_PLANE_M = 0.1;

/** The horizontal fields of view the camera steps through, degrees (plan R02, R02.T7.a). */
export const FOV_STEPS_DEG = [10, 20, 30, 45, 60, 90, 120] as const;

/** The default horizontal field of view, degrees: 60° across, the brainstorm's convention. */
export const DEFAULT_FOV_DEG = 60;

/** A 3 × 3 matrix as nine `f32`, column-major (WGSL's `mat3x3f` order). */
export type Mat3F32 = Float32Array;

/** A 4 × 4 matrix as sixteen `f32`, column-major (WGSL's `mat4x4f` order). */
export type Mat4F32 = Float32Array;

/** A view's drawing surface, in device pixels. */
export interface Viewport {
  /** Width, px, positive. */
  readonly widthPx: number;
  /** Height, px, positive. */
  readonly heightPx: number;
}

/** What projecting needs of a camera: which way it looks and how wide. */
export interface ProjectionCamera {
  /** The camera's orientation in its frame. */
  readonly orientation: Quaternion;
  /** The horizontal field of view, rad, in (0, π). */
  readonly fovXRad: number;
}

/**
 * The rotation from a camera's frame axes to its view axes, as nine `f32` column-major: the
 * transpose of the orientation's matrix, with no translation (plan R02, Design note 3).
 *
 * @remarks
 * Every position reaching the GPU is already relative to the camera, so the view matrix carries
 * no translation and no large number: composing the camera's position into an `f32` matrix would
 * put a point 1 km from a camera 1 au out on a 16 km grid.
 */
export function viewRotation(orientation: Quaternion): Mat3F32 {
  const [r0, r1, r2] = rotationRows(orientation);
  // The view rotation is Rᵀ; its column j is R's row j, so Rᵀ column-major is R row-major.
  return new Float32Array([r0.x, r0.y, r0.z, r1.x, r1.y, r1.z, r2.x, r2.y, r2.z]);
}

/**
 * {@link viewRotation} as the 4 × 4 that R01's `FrameSubmission.viewRotation` takes: column-major,
 * with a zero translation column and a unit `w`.
 */
export function viewRotation4(orientation: Quaternion): Mat4F32 {
  const [r0, r1, r2] = rotationRows(orientation);
  // Column j of Rᵀ is row j of R.
  return new Float32Array([
    r0.x,
    r0.y,
    r0.z,
    0,
    r1.x,
    r1.y,
    r1.z,
    0,
    r2.x,
    r2.y,
    r2.z,
    0,
    0,
    0,
    0,
    1,
  ]);
}

/**
 * The reversed-Z, infinite-far perspective projection for WebGPU's `[0, 1]` depth range, as
 * sixteen `f32` column-major (plan R02, Design note 4).
 *
 * @remarks
 * For a right-handed view space looking down −z, with s = 1 ÷ tan(fovX ÷ 2), a the aspect ratio
 * and n the near plane, the rows are `[s, 0, 0, 0]`, `[0, s·a, 0, 0]`, `[0, 0, 0, n]` and
 * `[0, 0, −1, 0]`: depth = n ÷ (−z), 1 at the near plane and falling to 0 at infinity, compared
 * `greater-equal` against a buffer cleared to 0. The field of view is horizontal and pixels are
 * square. The matrix is handed to the engine frozen (R01, Design note 18).
 *
 * @param fovXRad - The horizontal field of view, rad, in (0, π).
 * @param aspect - Width ÷ height, positive.
 * @param nearM - The near plane, m, positive.
 * @throws RangeError if an argument is out of its range.
 */
export function perspectiveReversedInfinite(
  fovXRad: number,
  aspect: number,
  nearM: number,
): Mat4F32 {
  if (!(fovXRad > 0 && fovXRad < Math.PI) || !(aspect > 0) || !(nearM > 0)) {
    throw new RangeError(
      "a projection needs a field of view in (0, π) and a positive aspect and near plane",
    );
  }
  const s = 1 / Math.tan(fovXRad / 2);
  return new Float32Array([s, 0, 0, 0, 0, s * aspect, 0, 0, 0, 0, 0, -1, 0, 0, nearM, 0]);
}

/** Where a camera-relative point lands on a view. */
export interface Projected {
  /** Horizontal pixel coordinate from the left edge, px. */
  readonly xPx: number;
  /** Vertical pixel coordinate from the top edge, px. */
  readonly yPx: number;
  /** The reversed depth, n ÷ distance along the view axis: 1 at the near plane, → 0 far away. */
  readonly depth: number;
  /** Whether the point is in front of the near plane (only then are the coordinates meaningful). */
  readonly inFront: boolean;
}

/** `v`, relative to the camera along its frame's axes, in the camera's view axes. */
export function toViewAxes(v: Vec3, orientation: Quaternion): Vec3 {
  const [r0, r1, r2] = rotationRows(orientation);
  // Rᵀ · v: the dot products of v with R's columns.
  return {
    x: r0.x * v.x + r1.x * v.y + r2.x * v.z,
    y: r0.y * v.x + r1.y * v.y + r2.y * v.z,
    z: r0.z * v.x + r1.z * v.y + r2.z * v.z,
  };
}

/**
 * Projects a camera-relative `f64` vector to pixel coordinates and reversed depth, in `f64`, by the
 * same matrices the GPU is given.
 *
 * @param v - The point relative to the camera, m along the camera's frame axes (from
 * `relativeToCamera`).
 */
export function project(v: Vec3, camera: ProjectionCamera, viewport: Viewport): Projected {
  const view = toViewAxes(v, camera.orientation);
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const aspect = viewport.widthPx / viewport.heightPx;
  const w = -view.z;
  const ndcX = (s * view.x) / w;
  const ndcY = (s * aspect * view.y) / w;
  return {
    xPx: ((ndcX + 1) / 2) * viewport.widthPx,
    yPx: ((1 - ndcY) / 2) * viewport.heightPx,
    depth: NEAR_PLANE_M / w,
    inFront: w >= NEAR_PLANE_M,
  };
}

/**
 * The solid angle of the pixel a direction falls in, sr: Ω_centre × cos³θ, with θ the angle off the
 * view axis (plan R02, Design note 10).
 *
 * @remarks
 * A pixel of a perspective view subtends Ω_centre = (2 tan(fovX ÷ 2) ÷ width)² on the axis and
 * cos³θ of that off it. A 1920 px, 60° view's centre pixel subtends 3.62 × 10⁻⁷ sr and a 16:9
 * corner pixel, at θ = 33.5°, 0.580 of that. Point sources brighten with resolution by it.
 *
 * @param dir - The direction, along the camera's frame axes; any length.
 */
export function pixelSolidAngle(dir: Vec3, camera: ProjectionCamera, viewport: Viewport): number {
  const view = toViewAxes(dir, camera.orientation);
  const length = Math.hypot(view.x, view.y, view.z);
  const cosTheta = -view.z / length;
  const centre = ((2 * Math.tan(camera.fovXRad / 2)) / viewport.widthPx) ** 2;
  return centre * Math.max(cosTheta, 0) ** 3;
}
