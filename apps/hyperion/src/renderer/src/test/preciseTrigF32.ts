/**
 * An `f32` emulation of `view/shaders/preciseTrig.wgsl`, operation for operation in the WGSL's
 * order, and of the arithmetic WGSL allows around it (W3C WGSL, CRD 2026-09-21, §15.7.4 and
 * §15.7.4.1), shared by R08's `atmosphere/precision.test.ts` and R07's
 * `lighting/horizonPrecision.test.ts`.
 *
 * @remarks
 * `Math.fround` of a sum, difference, product, quotient or square root of two `f32` values is the
 * correctly rounded `f32` result (Figueroa 1995, "When is double rounding innocuous?"), which is
 * {@link ROUNDED}. WGSL allows more: it specifies no rounding mode, so a correctly rounded sum,
 * difference or product may be either neighbour of the exact value; a division is within 2.5 ULP
 * for a divisor in [2⁻¹²⁶, 2¹²⁶]; and `sqrt` is inherited from 1 ÷ `inverseSqrt`, whose bound is
 * 2 ULP. {@link atWgslBounds} takes each to a corner of those bounds.
 */

import { seededRandom } from "./seededRandom";

const f = Math.fround;

const BITS = new Uint32Array(1);
const FLOAT = new Float32Array(BITS.buffer);

/** `x`, an `f32`, moved `ulps` steps towards +∞ (towards −∞ for negative `ulps`); 0 stays 0. */
export function nudge(x: number, ulps: number): number {
  if (x === 0 || ulps === 0 || !Number.isFinite(x)) {
    return x;
  }
  FLOAT[0] = x;
  BITS[0] = (BITS[0] ?? 0) + (x > 0 ? ulps : -ulps);
  return FLOAT[0];
}

/** The arithmetic of a WGSL implementation on `f32` operands: one rounding of each result. */
export interface Arithmetic {
  readonly add: (a: number, b: number) => number;
  readonly sub: (a: number, b: number) => number;
  readonly mul: (a: number, b: number) => number;
  readonly div: (a: number, b: number) => number;
  readonly sqrt: (a: number) => number;
}

/** IEEE arithmetic, every result rounded to nearest. */
export const ROUNDED: Arithmetic = {
  add: (a, b) => f(a + b),
  sub: (a, b) => f(a - b),
  mul: (a, b) => f(a * b),
  div: (a, b) => f(a / b),
  sqrt: (a) => f(Math.sqrt(a)),
};

/**
 * `exact`, an `f64` value, rounded to the `f32` neighbour above it when `up` and below it
 * otherwise; an `f32` value is itself. A sum or difference of two `f32` values is exact in `f64`
 * unless their exponents differ by more than 29, and a product always is.
 */
function directed(exact: number, up: boolean): number {
  const nearest = f(exact);
  if (nearest === exact || !Number.isFinite(nearest)) {
    return nearest;
  }
  if (up) {
    return nearest > exact ? nearest : nudge(nearest, 1);
  }
  return nearest < exact ? nearest : nudge(nearest, -1);
}

/**
 * Arithmetic at a corner of WGSL's bounds, each operation pushed the way `sign()` says (1 up, −1
 * down): a sum, difference or product rounded to that neighbour; a quotient 2 ULP off the rounded
 * one (within 2.5 of the exact); and `sqrt` as 1 ÷ `inverseSqrt`, the reciprocal root 2 ULP off its
 * rounded value and the quotient 2 more, so up to about 6.5 ULP in all.
 */
export function atWgslBounds(sign: () => 1 | -1): Arithmetic {
  const div = (a: number, b: number): number => nudge(f(a / b), 2 * sign());
  return {
    add: (a, b) => directed(a + b, sign() > 0),
    sub: (a, b) => directed(a - b, sign() > 0),
    mul: (a, b) => directed(a * b, sign() > 0),
    div,
    sqrt: (a) => div(1, nudge(f(1 / Math.sqrt(a)), -2 * sign())),
  };
}

/** Correctly rounded arithmetic, then WGSL's bounds pushed up, down, and by three seeded signs. */
export function wgslArithmetics(): ReadonlyArray<readonly [string, Arithmetic]> {
  const seeded = (seed: number): Arithmetic => {
    const random = seededRandom(seed);
    return atWgslBounds(() => (random() < 0.5 ? -1 : 1));
  };
  return [
    ["rounded", ROUNDED],
    ["bounds up", atWgslBounds(() => 1)],
    ["bounds down", atWgslBounds(() => -1)],
    ["bounds seeded 1", seeded(1)],
    ["bounds seeded 2", seeded(2)],
    ["bounds seeded 3", seeded(3)],
  ];
}

// The WGSL's literals, 3.14159265358979 and 1.57079632679490, round to the same f32 as π and π/2.
/** `TRIG_PI`, f32. */
export const TRIG_PI = f(Math.PI);
/** `TRIG_HALF_PI`, f32. */
export const TRIG_HALF_PI = f(Math.PI / 2);

const SIN_SERIES: ReadonlyArray<number> = [
  -1 / 6,
  1 / 120,
  -1 / 5040,
  1 / 362_880,
  -1 / 39_916_800,
  1 / 6_227_020_800,
  -1 / 1_307_674_368_000,
].map(f);
const COS_SERIES: ReadonlyArray<number> = [
  -1 / 2,
  1 / 24,
  -1 / 720,
  1 / 40_320,
  -1 / 3_628_800,
  1 / 479_001_600,
  -1 / 87_178_291_200,
].map(f);
const ATAN_SERIES: ReadonlyArray<number> = [
  -1 / 3,
  1 / 5,
  -1 / 7,
  1 / 9,
  -1 / 11,
  1 / 13,
  -1 / 15,
].map(f);

/** 1 + x2 (c₁ + x2 (c₂ + … + x2 cₙ)), innermost first, as the WGSL nests it. */
function series(x2: number, coefficients: ReadonlyArray<number>, ops: Arithmetic): number {
  let acc = ops.mul(x2, coefficients.at(-1) ?? Number.NaN);
  for (let k = coefficients.length - 2; k >= 0; k -= 1) {
    acc = ops.mul(x2, ops.add(coefficients[k] ?? Number.NaN, acc));
  }
  return ops.add(1, acc);
}

/** `preciseSinCos`: (sin θ, cos θ) for θ in [0, π]. */
export function preciseSinCos(theta: number, ops: Arithmetic = ROUNDED): readonly [number, number] {
  const y = ops.mul(0.5, theta);
  const y2 = ops.mul(y, y);
  const s = ops.mul(y, series(y2, SIN_SERIES, ops));
  const c = series(y2, COS_SERIES, ops);
  return [ops.mul(ops.mul(2, s), c), ops.mul(ops.sub(c, s), ops.add(c, s))];
}

/** `preciseAtanUnit`: atan x for x in [0, 1]. */
export function preciseAtanUnit(x: number, ops: Arithmetic = ROUNDED): number {
  let t = ops.div(x, ops.add(1, ops.sqrt(ops.add(1, ops.mul(x, x)))));
  t = ops.div(t, ops.add(1, ops.sqrt(ops.add(1, ops.mul(t, t)))));
  const t2 = ops.mul(t, t);
  return ops.mul(4, ops.mul(t, series(t2, ATAN_SERIES, ops)));
}

/** `preciseAtan`: atan x for x ≥ 0. */
export function preciseAtan(x: number, ops: Arithmetic = ROUNDED): number {
  return x > 1
    ? ops.sub(TRIG_HALF_PI, preciseAtanUnit(ops.div(1, x), ops))
    : preciseAtanUnit(x, ops);
}

/** `preciseAtan2`: the angle in [0, π] whose sine goes as y ≥ 0 and cosine as x. */
export function preciseAtan2(y: number, x: number, ops: Arithmetic = ROUNDED): number {
  const ax = Math.abs(x);
  const a = preciseAtanUnit(ops.div(Math.min(y, ax), Math.max(y, ax)), ops);
  const b = y > ax ? ops.sub(TRIG_HALF_PI, a) : a;
  return x < 0 ? ops.sub(TRIG_PI, b) : b;
}
