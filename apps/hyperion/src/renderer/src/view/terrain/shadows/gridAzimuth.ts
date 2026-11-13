/**
 * The grid azimuth of a direction at a point of a face (plan R10, R10.T8.b; decision-r10-t8a §2):
 * the TypeScript twin of `hyperion_surface::bake::horizon::grid_azimuth`, in the same operations in
 * `f64`, which the horizon map's azimuths are laid out by and `shadows/horizon.wgsl`'s
 * `horizon_grid_azimuth` computes for the sun.
 *
 * @remarks
 * A direction's grid azimuth at p on a face is atan2(b, a) of its part across the ground,
 * a ∂p/∂x + b ∂p/∂y, on S2's grid tangents ∂p/∂x ∝ (U − p (p · U)) du/ds and
 * ∂p/∂y ∝ (V − p (p · V)) dv/dt, U and V the face's axes and du/ds the derivative of S2's quadratic
 * warp; horizon azimuth k lies at k × 45° of it, along the patch grid's step `AZIMUTH_STEPS[k]`.
 * It is not R08's azimuth from local north, which R08's lookups keep. A point may lie just past its
 * face's edges (a skirt's or an edge's fragment), where the face's coordinates extend.
 */

import { faceUvToXyz, uvToSt, type Xyz } from "../cube";
import { type Face, unreachable } from "../patchKey";

/**
 * The share of a direction's length under which its part across the ground is taken as none, so
 * that it has no grid azimuth: `hyperion_surface`'s `ZENITH_TOLERANCE`, 10⁻¹².
 */
export const ZENITH_TOLERANCE = 1e-12;

function dot(a: Xyz, b: Xyz): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

function sub(a: Xyz, b: Xyz): Xyz {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}

function scale(a: Xyz, k: number): Xyz {
  return [a[0] * k, a[1] * k, a[2] * k];
}

/** `p`'s coordinates (u, v) on `face` by central projection, `cube::uv_on_face`'s. */
export function uvOnFace(face: Face, p: Xyz): readonly [number, number] {
  const [x, y, z] = p;
  switch (face) {
    case 0:
      return [y / x, z / x];
    case 1:
      return [-x / y, z / y];
    case 2:
      return [-x / z, -y / z];
    case 3:
      return [z / x, y / x];
    case 4:
      return [z / y, -x / y];
    case 5:
      return [-y / z, -x / z];
  }
  return unreachable(face);
}

/** du/ds of S2's quadratic warp at s: 8s ÷ 3 for s at least ½, 8 (1 − s) ÷ 3 below. */
export function warpRate(s: number): number {
  return s >= 0.5 ? (8 * s) / 3 : (8 * (1 - s)) / 3;
}

/**
 * The grid azimuth of `towards` at `at`, both body-fixed, on `face`'s grid, radians in [0, 2π);
 * `null` where `towards` has no part across the ground, that part under {@link ZENITH_TOLERANCE} of
 * its length (the zenith, the nadir, or a zero `towards`).
 *
 * @param at - The point, any length, in front of `face`.
 * @throws Error if `at` is not in front of `face`.
 */
export function gridAzimuthRad(face: Face, at: Xyz, towards: Xyz): number | null {
  const centre = faceUvToXyz(face, 0, 0);
  if (!(dot(at, centre) > 0)) {
    throw new Error(`(${at.join(", ")}) is not in front of face ${String(face)}`);
  }
  const length = Math.sqrt(dot(at, at));
  const p: Xyz = [at[0] / length, at[1] / length, at[2] / length];
  const across = sub(towards, scale(p, dot(towards, p)));
  if (Math.sqrt(dot(across, across)) <= ZENITH_TOLERANCE * Math.sqrt(dot(towards, towards))) {
    return null;
  }
  const [u, v] = uvOnFace(face, at);
  const axis = (au: number, av: number): Xyz => sub(faceUvToXyz(face, au, av), centre);
  const tangent = (a: Xyz, rate: number): Xyz => scale(sub(a, scale(p, dot(p, a))), rate);
  const gx = tangent(axis(1, 0), warpRate(uvToSt(u)));
  const gy = tangent(axis(0, 1), warpRate(uvToSt(v)));
  const xx = dot(gx, gx);
  const xy = dot(gx, gy);
  const yy = dot(gy, gy);
  const wx = dot(gx, across);
  const wy = dot(gy, across);
  // The solve of [xx xy; xy yy] (a, b) = (wx, wy), times its positive determinant.
  const a = yy * wx - xy * wy;
  const b = xx * wy - xy * wx;
  const phi = Math.atan2(b, a);
  return phi < 0 ? phi + 2 * Math.PI : phi;
}
