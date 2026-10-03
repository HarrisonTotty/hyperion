/**
 * A TypeScript emulation of `terrain.wgsl`'s `FaceDifferences` vertex arithmetic (plan R05,
 * R05.T11.b, Design note 4): the same `f32` operations in the same order, each rounded by
 * `Math.fround`, so that a test holds the shader's formula to T4.b's Rust `f32` reference through
 * `vertex_f32.golden`.
 *
 * @remarks
 * `Math.fround` of a sum, difference, product, quotient or square root of two `f32` values is the
 * correctly rounded `f32` result, since a double holds the exact result to more than twice `f32`'s
 * precision plus two bits (Figueroa 1995, "When is double rounding innocuous?"). So the emulation
 * checks the formula under IEEE-rounded operations, and it matches the Rust reference bit for bit.
 * A GPU need not: WGSL lets an implementation contract `a * b + c` into a fused multiply-add, allows
 * 2.5 ULP in an `f32` division, and takes `sqrt`'s accuracy from `inverseSqrt` (W3C WGSL §15.5.1,
 * "Floating Point Accuracy"). Every inexact quotient here is a small quantity, so those last bits
 * stay far inside T4.b's 1 mm bound.
 */

import type { Vec3 } from "../../../geometry/vec3";

const f = Math.fround;

/** A slot record's `f32` terms, as `terrain.wgsl`'s `SlotRecord` holds them (uniforms.ts). */
export interface SlotTermsF32 {
  readonly axisA: Vec3;
  readonly axisE1: Vec3;
  readonly axisE2: Vec3;
  readonly s0: number;
  readonly t0: number;
  readonly u0: number;
  readonly v0: number;
  readonly step: number;
  /** M's diagonal, metres. */
  readonly scale: Vec3;
  /** m₀ = M⁻¹ d₀, per metre. */
  readonly m0: Vec3;
  readonly nu0: Vec3;
  /** h₀, metres. */
  readonly h0M: number;
  readonly straddles: boolean;
}

function stToUv(s: number): number {
  if (s >= 0.5) {
    return f(f(f(f(4 * s) * s) - 1) / 3);
  }
  const r = f(1 - s);
  return f(f(1 - f(f(4 * r) * r)) / 3);
}

function warpDifference(s0: number, ds: number, straddles: boolean): number {
  const s = f(s0 + ds);
  if (straddles) {
    return f(stToUv(s) - stToUv(s0));
  }
  if (s0 >= 0.5) {
    return f(f(f(4 * ds) * f(s + s0)) / 3);
  }
  return f(f(f(4 * ds) * f(f(2 - s) - s0)) / 3);
}

function add(a: Vec3, b: Vec3): Vec3 {
  return { x: f(a.x + b.x), y: f(a.y + b.y), z: f(a.z + b.z) };
}

function scaled(v: Vec3, k: number): Vec3 {
  return { x: f(v.x * k), y: f(v.y * k), z: f(v.z * k) };
}

function dot(a: Vec3, b: Vec3): number {
  return f(f(f(a.x * b.x) + f(a.y * b.y)) + f(a.z * b.z));
}

function unitDifference(p0: Vec3, delta: Vec3): Vec3 {
  const dotP = dot(p0, delta);
  const dd = dot(delta, delta);
  const len0Sq = dot(p0, p0);
  const len0 = f(Math.sqrt(len0Sq));
  const len = f(Math.sqrt(f(f(len0Sq + f(2 * dotP)) + dd)));
  const diff = f(-f(f(2 * dotP) + dd));
  const factor = f(diff / f(f(len * len0) * f(len + len0)));
  return {
    x: f(f(delta.x / len) + f(p0.x * factor)),
    y: f(f(delta.y / len) + f(p0.y * factor)),
    z: f(f(delta.z / len) + f(p0.z * factor)),
  };
}

/** d − d₀ of vertex (`x`, `y`), the shader's `directionDifference`. */
export function directionDifferenceF32(rec: SlotTermsF32, x: number, y: number): Vec3 {
  const du = warpDifference(rec.s0, f((x - 32) * rec.step), rec.straddles);
  const dv = warpDifference(rec.t0, f((y - 32) * rec.step), rec.straddles);
  const n0 = add(add(rec.axisA, scaled(rec.axisE1, rec.u0)), scaled(rec.axisE2, rec.v0));
  const delta = add(scaled(rec.axisE1, du), scaled(rec.axisE2, dv));
  return unitDifference(n0, delta);
}

/** ν − ν₀ from d − d₀, the shader's `normalDifference`. */
export function normalDifferenceF32(rec: SlotTermsF32, dd: Vec3): Vec3 {
  const dm = { x: f(dd.x / rec.scale.x), y: f(dd.y / rec.scale.y), z: f(dd.z / rec.scale.z) };
  return unitDifference(rec.m0, dm);
}

/**
 * P − P₀ of vertex (`x`, `y`) at height `hM` metres, body-fixed metres: the shader's
 * `faceDifferencePosition`, Rust's `face_difference_position_f32`.
 */
export function faceDifferencePositionF32(
  rec: SlotTermsF32,
  x: number,
  y: number,
  hM: number,
): Vec3 {
  const dd = directionDifferenceF32(rec, x, y);
  const dnu = normalDifferenceF32(rec, dd);
  const dh = f(hM - rec.h0M);
  const m = { x: f(rec.scale.x * dd.x), y: f(rec.scale.y * dd.y), z: f(rec.scale.z * dd.z) };
  return add(add(m, scaled(dnu, hM)), scaled(rec.nu0, dh));
}

/**
 * The morph target's position of vertex (`x`, `y`): at an even vertex, and on a level-0 patch,
 * the formula at its morph height; at an odd one the mean of the formula at its two even
 * neighbours on the parent mesh's diagonal, at their morph heights `morphHeightM(x, y)`. The
 * shader's `morphOffset` on the `FaceDifferences` path; Rust's `face_difference_morph_f32`.
 */
export function faceDifferenceMorphF32(
  rec: SlotTermsF32,
  x: number,
  y: number,
  morphHeightM: (x: number, y: number) => number,
): Vec3 {
  const at = (px: number, py: number): Vec3 =>
    faceDifferencePositionF32(rec, px, py, morphHeightM(px, py));
  const oddX = x % 2 === 1;
  const oddY = y % 2 === 1;
  if (rec.straddles || (!oddX && !oddY)) {
    return at(x, y);
  }
  const [a, b] =
    oddX && !oddY
      ? [at(x - 1, y), at(x + 1, y)]
      : !oddX
        ? [at(x, y - 1), at(x, y + 1)]
        : [at(x - 1, y - 1), at(x + 1, y + 1)];
  return { x: f(0.5 * f(a.x + b.x)), y: f(0.5 * f(a.y + b.y)), z: f(0.5 * f(a.z + b.z)) };
}
