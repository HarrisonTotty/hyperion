/**
 * The large-particle limit of a sphere's optics (plan R08, R08.T16.a): Fraunhofer's diffraction by
 * its disc and geometric optics at its surface, which `sizeDistribution.ts` sums in place of Mie for
 * the part of a size distribution past {@link GEOMETRIC_OPTICS_SIZE_PARAMETER}.
 *
 * @remarks
 * - **Extinction.** Q_ext = 2: the sphere takes its own area out of the beam and diffracts as much
 *   again, the extinction paradox (H. C. van de Hulst 1957, "Light Scattering by Small Particles",
 *   Wiley, ch. 8; C. F. Bohren and D. R. Huffman 1983, "Absorption and Scattering of Light by Small
 *   Particles", Wiley, ch. 4). Mie's lies above it by about 2 x^(−2/3), the edge's share (measured
 *   in `geometricOptics.test.ts`).
 * - **Diffraction** carries Q = 1 in Fraunhofer's pattern of the projected disc,
 *   (x² ÷ 4π)[2J₁(xθ) ÷ (xθ)]² sr⁻¹ (van de Hulst ch. 8; M. Born and E. Wolf, "Principles of
 *   Optics", 7th ed., §8.5.2), with no part in absorption. Past xθ = {@link AIRY_RING_AVERAGE_FROM}
 *   its rings are averaged by J₁'s asymptote, J₁(v) ≈ √(2 ÷ πv) cos(v − 3π ÷ 4) (M. Abramowitz and
 *   I. A. Stegun 1964, "Handbook of Mathematical Functions", 9.2.1), to 4 ÷ (π (xθ)³), so that a sum
 *   over sizes does not alias them, and scaled so that the pattern's power stays 1: Rayleigh's
 *   encircled energy, 1 − J₀² − J₁² (Born and Wolf §8.5.2), leaves J₀(v)² + J₁(v)² past v, where the
 *   average leaves 2 ÷ πv. A pattern whose first dark ring lies inside the phase table's forward
 *   delta ({@link DIFFRACTION_DELTA_SIZE_PARAMETER}) is that delta.
 * - **Geometric optics** carries the other Q = 1 (van de Hulst ch. 12). Rays meet the sphere at
 *   incidence i, the incident power through the annulus at impact parameter a sin i being
 *   sin 2i di of the disc's. Order p = 0 is reflected at Fresnel's coefficients (Born and Wolf §1.5.2)
 *   for the complex index m = n − ik, in `mie.ts`'s convention; order p ≥ 1 enters with 1 − R at m,
 *   crosses p chords of length 2a cos r, r Snell's angle at n, each passing exp(−4kx cos r) of its
 *   power (the intensity's decay exp(−4πk s ÷ λ)), is reflected p − 1 times inside at n's
 *   coefficients, and leaves deviated by Θ_p = 2i − 2pr + (p − 1)π, folded into [0, π].
 * - **The matrix** sums the rays incoherently, as a sum over sizes averages their interference, in
 *   `sizeDistribution.ts`'s elements (S₁ perpendicular): a₁ = a₂ ∝ ½(E⊥ + E∥), b₁ ∝ ½(E∥ − E⊥),
 *   a₃ = a₄ ∝ r⊥r∥ for the reflection (Re r⊥r∥* at m) and (1 − r⊥²)(1 − r∥²)(r⊥r∥)^(p−1) for a
 *   refracted ray, and b₂ ∝ −Im r⊥r∥* for the reflection and 0 for the refracted rays, so that a₃ is
 *   a₁ forward and −a₁ backward, as a sphere's is. Diffraction's matrix is the identity's.
 * - **The angles.** Each ray's share is laid on the phase table's u grid by linear weights on the
 *   two entries about it, each divided by its own hat's solid angle, so that the table as read
 *   (linear in u) holds every ray's power; other cosines read it linearly in u.
 * - **Absorption** is taken by groups: the rays fall in {@link RAY_GROUPS} groups even in sin²i, and
 *   each group's order is attenuated at the group's mean chord, exact for k = 0 and in the limits of
 *   weak and strong absorption. Past {@link RAY_ORDERS}, the orders are one bucket, attenuated as
 *   order {@link RAY_ORDERS} + 1, and past {@link RAY_TRACED} what is left of a ray, its series' sum,
 *   is laid at the next order's angle, so that a lossless sphere's rays keep all their power.
 */

import { gaussLegendre } from "../lighting/quadrature";
import { FORWARD_PEAK_ENTRIES, FORWARD_PEAK_GRID_ENTRIES } from "./forwardPeak";
import type { ComplexIndex } from "./mie";
import { acos, asin, cos, exp, sin } from "../../lib/pinnedMath";

/**
 * The size parameter past which a sphere takes the large-particle limit in place of Mie, chosen by
 * measured agreement (`geometricOptics.test.ts` prints it): the least of the test's grid, 50, 75,
 * 100, 150 and 200, at which, averaged over ±10% in x at every term wavelength for the volatile
 * decks' indices, the limit's Q_ext lies within {@link CROSSOVER_EXTINCTION_TOLERANCE} of Mie's, its
 * ω within {@link CROSSOVER_ALBEDO_TOLERANCE} and its g within {@link CROSSOVER_ASYMMETRY_TOLERANCE}.
 *
 * @remarks
 * The extinction's tolerance sets it between the cost and the committed references (lean,
 * 2026-10-31). Mie's matrix at the phase table's 256 angles costs about 0.1 ms a sphere at x 100
 * (pod, 16 vCPU), and a deck's quadrature takes 16 to 64 spheres a unit of x below the crossover:
 * a 10 µm water deck's term costs 0.9 s at 100, 2.5 s at 150 and 23 s in Mie alone. At 125 and
 * below, the Venus deck's mode 3, which reaches x 338 by its r⁶ tail, moves the traced references'
 * beams by 10⁻⁵ of τ, past their check (`reference/references.test.ts`); at 150 it does not. A
 * deck's own optics lie closer to Mie's than the tolerances, by its share past the crossover.
 */
export const GEOMETRIC_OPTICS_SIZE_PARAMETER = 150;

/**
 * The crossover's tolerance on Q_ext, relative: the limit's 2 against Mie's, which lies above it by
 * about 2x^(−2/3), 3.4% at 150 and 4.6% at 100.
 */
export const CROSSOVER_EXTINCTION_TOLERANCE = 0.04;

/** The crossover's tolerance on ω, absolute. */
export const CROSSOVER_ALBEDO_TOLERANCE = 0.005;

/** The crossover's tolerance on g, absolute: the limit's runs high by about 2 ÷ x. */
export const CROSSOVER_ASYMMETRY_TOLERANCE = 0.02;

/** j₁,₁, the first zero of J₁ (Abramowitz and Stegun 1964, Table 9.5): Fraunhofer's first dark ring. */
export const BESSEL_J1_FIRST_ZERO = 3.831705970207512;

/**
 * The size parameter past which a sphere's diffraction is the phase table's forward delta: where
 * its first dark ring, xθ = {@link BESSEL_J1_FIRST_ZERO}, falls inside the delta's
 * {@link FORWARD_PEAK_ENTRIES} entries, θ < π(4 ÷ 255)², about 4,957.
 */
export const DIFFRACTION_DELTA_SIZE_PARAMETER =
  BESSEL_J1_FIRST_ZERO / (Math.PI * (FORWARD_PEAK_ENTRIES / (FORWARD_PEAK_GRID_ENTRIES - 1)) ** 2);

/** xθ past which the Airy pattern's rings are averaged: 4π, where cos²(v − 3π ÷ 4) is ½. */
export const AIRY_RING_AVERAGE_FROM = 4 * Math.PI;

/** The rays in incidence angle, even in i over [0, π ÷ 2] at their midpoints. */
export const RAY_SAMPLES = 4096;

/** The groups the rays fall in by sin²i, each attenuated at its mean chord. */
export const RAY_GROUPS = 16;

/** The orders kept apart: p = 0 … RAY_ORDERS, the rest one bucket. */
export const RAY_ORDERS = 10;

/** The orders traced one by one; past them a ray's series' sum is laid at the next order's angle. */
export const RAY_TRACED = 16;

/** The orders a group holds: p = 0 … {@link RAY_ORDERS}, then the bucket of the rest. */
const ORDER_SLOTS = RAY_ORDERS + 2;

/** The phase table's entries, `sizeDistribution.ts`'s `PHASE_TABLE_SIZE` (a test holds them equal). */
const ENTRIES = FORWARD_PEAK_GRID_ENTRIES;

/** Bessel's integral's rule (order 32), on [0, π]. */
const BESSEL_RULE = gaussLegendre(32);

/**
 * Jₙ(v) by Bessel's integral, (1 ÷ π) ∫₀^π cos(nτ − v sin τ) dτ (Abramowitz and Stegun 1964,
 * 9.1.21), by Gauss–Legendre of order 32: to rounding for v up to about 20.
 */
function besselJ(order: 0 | 1, v: number): number {
  let sum = 0;
  for (let q = 0; q < BESSEL_RULE.x.length; q += 1) {
    const tau = 0.5 * Math.PI * (1 + (BESSEL_RULE.x[q] ?? Number.NaN));
    sum += (BESSEL_RULE.w[q] ?? Number.NaN) * cos(order * tau - v * sin(tau));
  }
  return 0.5 * sum;
}

/**
 * J₁(v) by Bessel's integral ({@link besselJ}'s rule): to rounding for v up to about 20.
 *
 * @param v - The argument, finite and ≥ 0.
 */
export function besselJ1(v: number): number {
  return besselJ(1, v);
}

let ringAverageScale: number | undefined;

/**
 * The ring average's scale, (J₀(4π)² + J₁(4π)²) ÷ (2 ÷ 4π²), which gives the pattern past 4π its
 * exact power (the module's remarks).
 */
function ringAverageScaleOf(): number {
  if (ringAverageScale === undefined) {
    const v = AIRY_RING_AVERAGE_FROM;
    const j0 = besselJ(0, v);
    const j1 = besselJ(1, v);
    ringAverageScale = (j0 * j0 + j1 * j1) / (2 / (Math.PI * v));
  }
  return ringAverageScale;
}

/** The Airy table's intervals over [0, {@link AIRY_RING_AVERAGE_FROM}]. */
const AIRY_INTERVALS = 4096;

let airyTable: Float64Array | undefined;

/** [2J₁(v) ÷ v]² at the table's nodes, made once. */
function airyNodes(): Float64Array {
  if (airyTable === undefined) {
    airyTable = Float64Array.from({ length: AIRY_INTERVALS + 1 }, (_, q) => {
      const v = (q * AIRY_RING_AVERAGE_FROM) / AIRY_INTERVALS;
      const ratio = q === 0 ? 1 : (2 * besselJ1(v)) / v;
      return ratio * ratio;
    });
  }
  return airyTable;
}

/**
 * Fraunhofer's pattern of a disc relative to its centre, [2J₁(v) ÷ v]², linear between 4,097 nodes
 * to {@link AIRY_RING_AVERAGE_FROM}, and ring-averaged past it, 4 ÷ (π v³) scaled to the pattern's
 * power there (the module's remarks).
 *
 * @param v - xθ, ≥ 0.
 */
export function airyPattern(v: number): number {
  if (v >= AIRY_RING_AVERAGE_FROM) {
    return (ringAverageScaleOf() * 4) / (Math.PI * v * v * v);
  }
  const nodes = airyNodes();
  const at = (v * AIRY_INTERVALS) / AIRY_RING_AVERAGE_FROM;
  const q = Math.floor(at);
  const t = at - q;
  const a = nodes[q] ?? Number.NaN;
  return a + t * ((nodes[q + 1] ?? Number.NaN) - a);
}

let diffractionSpread: number | undefined;

/**
 * D in 1 − g = D ÷ x, diffraction's mean cosine over the sphere: (2c ÷ π) ∫₀^π (1 − cos θ) sin θ ÷ θ³ dθ,
 * c the ring average's scale, the ring-averaged pattern's, which holds all of it but O(x⁻²); about
 * 0.51.
 */
function diffractionSpreadOf(): number {
  if (diffractionSpread === undefined) {
    const rule = gaussLegendre(64);
    let sum = 0;
    for (let q = 0; q < rule.x.length; q += 1) {
      const theta = 0.5 * Math.PI * (1 + (rule.x[q] ?? Number.NaN));
      sum +=
        ((rule.w[q] ?? Number.NaN) * ((1 - cos(theta)) * sin(theta))) / (theta * theta * theta);
    }
    diffractionSpread = ((2 * ringAverageScaleOf()) / Math.PI) * 0.5 * Math.PI * sum;
  }
  return diffractionSpread;
}

/** Diffraction's asymmetry at size parameter x: 1 − D ÷ x ({@link diffractionSpreadOf}). */
export function diffractionAsymmetry(sizeParameter: number): number {
  return 1 - diffractionSpreadOf() / sizeParameter;
}

// ---------------------------------------------------------------------------------------------
// Fresnel's coefficients.

/** The reflection at the complex index: |r⊥|², |r∥|², and Re and −Im of r⊥r∥*. */
interface Reflection {
  readonly perp: number;
  readonly par: number;
  readonly cross: number;
  readonly crossIm: number;
}

/**
 * Fresnel's reflection at m = n − ik for incidence of sine² `t` and cosine `cosI`:
 * r⊥ = (cos i − w) ÷ (cos i + w) and r∥ = (m² cos i − w) ÷ (m² cos i + w), w = √(m² − sin² i) on the
 * principal branch (Re w ≥ 0), which for n real are Born and Wolf's (§1.5.2) with w = n cos r.
 */
function reflection(t: number, cosI: number, index: ComplexIndex): Reflection {
  const { n, k } = index;
  const m2Re = n * n - k * k;
  const m2Im = -2 * n * k;
  const zRe = m2Re - t;
  const zIm = m2Im;
  const modulus = Math.hypot(zRe, zIm);
  const wRe = Math.sqrt(0.5 * (modulus + zRe));
  const wIm = (zIm < 0 ? -1 : 1) * Math.sqrt(Math.max(0, 0.5 * (modulus - zRe)));
  const perp = quotient(cosI - wRe, -wIm, cosI + wRe, wIm);
  const par = quotient(m2Re * cosI - wRe, m2Im * cosI - wIm, m2Re * cosI + wRe, m2Im * cosI + wIm);
  return {
    perp: perp.re * perp.re + perp.im * perp.im,
    par: par.re * par.re + par.im * par.im,
    // r⊥ r∥* = (a + ib)(c − id) = (ac + bd) + i(bc − ad).
    cross: perp.re * par.re + perp.im * par.im,
    crossIm: -(perp.im * par.re - perp.re * par.im),
  };
}

/** (a + ib) ÷ (c + id). */
function quotient(
  a: number,
  b: number,
  c: number,
  d: number,
): { readonly re: number; readonly im: number } {
  const norm = c * c + d * d;
  return { re: (a * c + b * d) / norm, im: (b * c - a * d) / norm };
}

// ---------------------------------------------------------------------------------------------
// The limit at one index.

/** A matrix's four distinct elements on the phase table's grid, group-order-major. */
interface GridMatrix {
  readonly a1: Float64Array;
  readonly b1: Float64Array;
  readonly a3: Float64Array;
  readonly b2: Float64Array;
}

/**
 * A sphere's geometric optics at one index, made once a wavelength and summed over its sizes
 * ({@link geometricLimit}): each group's orders' power, unattenuated, and, where angles are asked,
 * their matrices.
 */
export interface GeometricLimit {
  readonly index: ComplexIndex;
  /** ½(E⊥ + E∥) of each group's order at [group × slots + order], a share of the disc's power. */
  readonly power: Float64Array;
  /** The same, times cos Θ. */
  readonly cosinePower: Float64Array;
  /** Each group's mean chord cosine, cos r, over its refracted rays; 1 where none is. */
  readonly chords: Float64Array;
  /** Each group's order's matrix on the phase table's grid, sr⁻¹; undefined where no angles are asked. */
  readonly grid: GridMatrix | undefined;
}

let hatSolidAngles: Float64Array | undefined;

/**
 * Each grid entry's hat, linear in u from its neighbours to it, integrated over the sphere:
 * 2π ∫ hat(u) sin(πu²) 2πu du, by Gauss–Legendre of order 8 on each of its two intervals.
 */
function hatSolidAnglesOf(): Float64Array {
  if (hatSolidAngles === undefined) {
    const rule = gaussLegendre(8);
    const step = 1 / (ENTRIES - 1);
    hatSolidAngles = Float64Array.from({ length: ENTRIES }, (_, j) => {
      let sum = 0;
      for (const side of [-1, 1]) {
        const from = j * step;
        const to = from + side * step;
        if (to < 0 || to > 1 + 1e-12) {
          continue;
        }
        for (let q = 0; q < rule.x.length; q += 1) {
          const s = 0.5 * (1 + (rule.x[q] ?? Number.NaN));
          const u = from + s * (to - from);
          sum +=
            0.5 *
            step *
            (rule.w[q] ?? Number.NaN) *
            (1 - s) *
            sin(Math.PI * u * u) *
            2 *
            Math.PI *
            u;
        }
      }
      return 2 * Math.PI * sum;
    });
  }
  return hatSolidAngles;
}

/**
 * A sphere's geometric optics at one index (the module's remarks): its rays traced once, by group
 * and order, for {@link geometricSphere} and {@link addGeometricSphere} to attenuate by size.
 *
 * @param angles - Whether the matrix on the phase table's grid is made: the efficiencies need none.
 * @throws RangeError for an index whose n is not finite and > 0 or whose k is not finite and ≥ 0.
 */
export function geometricLimit(index: ComplexIndex, angles: boolean): GeometricLimit {
  const { n, k } = index;
  if (!(n > 0 && Number.isFinite(n) && k >= 0 && Number.isFinite(k))) {
    throw new RangeError(`refractive index ${n} + ${k}i needs a finite n > 0 and a finite k ≥ 0`);
  }
  const power = new Float64Array(RAY_GROUPS * ORDER_SLOTS);
  const cosinePower = new Float64Array(RAY_GROUPS * ORDER_SLOTS);
  const chordSums = new Float64Array(RAY_GROUPS);
  const chordWeights = new Float64Array(RAY_GROUPS);
  const size = RAY_GROUPS * ORDER_SLOTS * ENTRIES;
  const grid: GridMatrix | undefined = angles
    ? {
        a1: new Float64Array(size),
        b1: new Float64Array(size),
        a3: new Float64Array(size),
        b2: new Float64Array(size),
      }
    : undefined;
  const step = (0.5 * Math.PI) / RAY_SAMPLES;
  // The midpoints' weights sin 2i di, made to sum to 1.
  let total = 0;
  for (let s = 0; s < RAY_SAMPLES; s += 1) {
    total += sin(2 * (s + 0.5) * step) * step;
  }
  const lay = (
    slot: number,
    weight: number,
    perp: number,
    par: number,
    cross: number,
    crossIm: number,
    deviation: number,
  ): void => {
    const e = weight * 0.5 * (perp + par);
    const cosine = cos(deviation);
    power[slot] = (power[slot] ?? 0) + e;
    cosinePower[slot] = (cosinePower[slot] ?? 0) + e * cosine;
    if (grid === undefined) {
      return;
    }
    const u = Math.sqrt(acos(Math.min(Math.max(cosine, -1), 1)) / Math.PI);
    const at = u * (ENTRIES - 1);
    const j = Math.min(Math.floor(at), ENTRIES - 2);
    const f = at - j;
    const base = slot * ENTRIES + j;
    const put = (values: Float64Array, v: number): void => {
      values[base] = (values[base] ?? 0) + v * (1 - f);
      values[base + 1] = (values[base + 1] ?? 0) + v * f;
    };
    put(grid.a1, e);
    put(grid.b1, weight * 0.5 * (par - perp));
    put(grid.a3, weight * cross);
    put(grid.b2, weight * crossIm);
  };
  for (let s = 0; s < RAY_SAMPLES; s += 1) {
    const i = (s + 0.5) * step;
    const sinI = sin(i);
    const cosI = cos(i);
    const t = sinI * sinI;
    const weight = (sin(2 * i) * step) / total;
    const group = Math.min(Math.floor(t * RAY_GROUPS), RAY_GROUPS - 1);
    const slots = group * ORDER_SLOTS;
    const outer = reflection(t, cosI, index);
    lay(slots, weight, outer.perp, outer.par, outer.cross, outer.crossIm, Math.PI - 2 * i);
    const sinR = sinI / n;
    if (sinR >= 1) {
      continue;
    }
    const cosR = Math.sqrt(1 - sinR * sinR);
    const r = asin(sinR);
    chordSums[group] = (chordSums[group] ?? 0) + weight * cosR;
    chordWeights[group] = (chordWeights[group] ?? 0) + weight;
    const rPerp = (cosI - n * cosR) / (cosI + n * cosR);
    const rPar = (n * cosI - cosR) / (n * cosI + cosR);
    const reflectPerp = rPerp * rPerp;
    const reflectPar = rPar * rPar;
    const product = rPerp * rPar;
    let perp = (1 - outer.perp) * (1 - reflectPerp);
    let par = (1 - outer.par) * (1 - reflectPar);
    let sign = 1;
    for (let p = 1; p <= RAY_TRACED; p += 1) {
      const slot = slots + Math.min(p, RAY_ORDERS + 1);
      lay(
        slot,
        weight,
        perp,
        par,
        sign * Math.sqrt(perp * par),
        0,
        2 * i - 2 * p * r + (p - 1) * Math.PI,
      );
      perp *= reflectPerp;
      par *= reflectPar;
      sign = product < 0 ? -sign : sign;
    }
    // What is left, the series Σ_q E r^(2q), at the next order's angle.
    const p = RAY_TRACED + 1;
    lay(
      slots + RAY_ORDERS + 1,
      weight,
      perp / (1 - reflectPerp),
      par / (1 - reflectPar),
      (sign * Math.sqrt(perp * par)) / (1 - product),
      0,
      2 * i - 2 * p * r + (p - 1) * Math.PI,
    );
  }
  if (grid !== undefined) {
    const hats = hatSolidAnglesOf();
    for (const values of [grid.a1, grid.b1, grid.a3, grid.b2]) {
      for (let q = 0; q < values.length; q += 1) {
        values[q] = (values[q] ?? 0) / (hats[q % ENTRIES] ?? Number.NaN);
      }
    }
  }
  const chords = chordSums.map((sum, g) => {
    const w = chordWeights[g] ?? 0;
    return w > 0 ? sum / w : 1;
  });
  return { index, power, cosinePower, chords, grid };
}

/**
 * The share of each order a group passes at size parameter x: its chord's exp(−4kx cos r) to the
 * order's power, the bucket's at {@link RAY_ORDERS} + 1.
 */
function passes(limit: GeometricLimit, sizeParameter: number, group: number): number {
  const { k } = limit.index;
  return k > 0 ? exp(-4 * k * sizeParameter * (limit.chords[group] ?? 1)) : 1;
}

/** A sphere's large-particle efficiencies, per unit of its geometric cross-section. */
export interface GeometricSphere {
  /** Q_ext, 2. */
  readonly qExt: number;
  /** Q_sca: diffraction's 1 and the rays' power that leaves. */
  readonly qSca: number;
  /** Q_sca × g. */
  readonly qScaAsymmetry: number;
}

/**
 * One sphere's large-particle efficiencies at size parameter x (the module's remarks).
 *
 * @param sizeParameter - x, > 0.
 */
export function geometricSphere(limit: GeometricLimit, sizeParameter: number): GeometricSphere {
  let scattered = 0;
  let cosine = 0;
  for (let g = 0; g < RAY_GROUPS; g += 1) {
    const pass = passes(limit, sizeParameter, g);
    let share = 1;
    for (let p = 0; p < ORDER_SLOTS; p += 1) {
      const slot = g * ORDER_SLOTS + p;
      scattered += share * (limit.power[slot] ?? 0);
      cosine += share * (limit.cosinePower[slot] ?? 0);
      share *= pass;
    }
  }
  return {
    qExt: 2,
    qSca: 1 + scattered,
    qScaAsymmetry: diffractionAsymmetry(sizeParameter) + cosine,
  };
}

/**
 * The matrix sums of a distribution's large spheres at a set of cosines, as {@link addGeometricSphere}
 * adds them: each group's order's weight, the diffraction at the cosines, and the weight whose
 * diffraction is the forward delta.
 */
export interface GeometricMatrixSums {
  readonly limit: GeometricLimit;
  /** The cosines' angles, θ. */
  readonly theta: Float64Array;
  /** Σ w × the share each group's order passes, at [group × slots + order]. */
  readonly orderWeights: Float64Array;
  /** Σ w (x² ÷ 4π) A(xθ) at the cosines, over the spheres whose diffraction is drawn. */
  readonly diffraction: Float64Array;
  /** Σ w over the spheres whose diffraction is the forward delta. */
  forwardPeakWeight: number;
}

/**
 * Empty matrix sums at the cosines `mu`.
 *
 * @throws RangeError for a limit made without angles.
 */
export function geometricMatrixSums(limit: GeometricLimit, mu: Float64Array): GeometricMatrixSums {
  if (limit.grid === undefined) {
    throw new RangeError("a large-particle matrix needs a limit made with its angles");
  }
  return {
    limit,
    theta: mu.map((c) => acos(Math.min(Math.max(c, -1), 1))),
    orderWeights: new Float64Array(RAY_GROUPS * ORDER_SLOTS),
    diffraction: new Float64Array(mu.length),
    forwardPeakWeight: 0,
  };
}

/**
 * Adds one sphere of size parameter x and weight w to a distribution's matrix sums, w in the units
 * of the sphere's geometric cross-section that `sizeDistribution.ts`'s area weights are.
 */
export function addGeometricSphere(
  sums: GeometricMatrixSums,
  sizeParameter: number,
  weight: number,
): void {
  for (let g = 0; g < RAY_GROUPS; g += 1) {
    const pass = passes(sums.limit, sizeParameter, g);
    let share = weight;
    for (let p = 0; p < ORDER_SLOTS; p += 1) {
      const slot = g * ORDER_SLOTS + p;
      sums.orderWeights[slot] = (sums.orderWeights[slot] ?? 0) + share;
      share *= pass;
    }
  }
  if (sizeParameter > DIFFRACTION_DELTA_SIZE_PARAMETER) {
    sums.forwardPeakWeight += weight;
    return;
  }
  const peak = (weight * sizeParameter * sizeParameter) / (4 * Math.PI);
  for (let q = 0; q < sums.theta.length; q += 1) {
    sums.diffraction[q] =
      (sums.diffraction[q] ?? 0) + peak * airyPattern(sizeParameter * (sums.theta[q] ?? 0));
  }
}

/** A distribution's large spheres' matrix at its sums' cosines, in Mie's sums' units. */
export interface GeometricMatrix {
  readonly a1: Float64Array;
  readonly b1: Float64Array;
  readonly a3: Float64Array;
  readonly b2: Float64Array;
}

/**
 * The matrix the sums hold at their cosines, in the units of `sizeDistribution.ts`'s Mie sums,
 * whose a₁ adds w ÷ x² × ½(|S₁|² + |S₂|²) a sphere and so π w Q_sca over the sphere: π w times each
 * part's sr⁻¹. The forward delta's spheres' diffraction is left out (`forwardPeakWeight`).
 */
export function geometricMatrix(sums: GeometricMatrixSums): GeometricMatrix {
  const { grid } = sums.limit;
  if (grid === undefined) {
    throw new RangeError("a large-particle matrix needs a limit made with its angles");
  }
  const onGrid = (values: Float64Array): Float64Array => {
    const out = new Float64Array(ENTRIES);
    for (let slot = 0; slot < sums.orderWeights.length; slot += 1) {
      const w = sums.orderWeights[slot] ?? 0;
      if (w === 0) {
        continue;
      }
      const base = slot * ENTRIES;
      for (let j = 0; j < ENTRIES; j += 1) {
        out[j] = (out[j] ?? 0) + w * (values[base + j] ?? 0);
      }
    }
    return out;
  };
  const read = (values: Float64Array): Float64Array =>
    sums.theta.map((theta) => {
      const at = Math.sqrt(theta / Math.PI) * (ENTRIES - 1);
      const j = Math.min(Math.floor(at), ENTRIES - 2);
      const f = at - j;
      const a = values[j] ?? Number.NaN;
      return Math.PI * (a + f * ((values[j + 1] ?? Number.NaN) - a));
    });
  const diffraction = sums.diffraction.map((v) => Math.PI * v);
  const a1 = read(onGrid(grid.a1));
  const a3 = read(onGrid(grid.a3));
  return {
    a1: a1.map((v, q) => v + (diffraction[q] ?? 0)),
    b1: read(onGrid(grid.b1)),
    a3: a3.map((v, q) => v + (diffraction[q] ?? 0)),
    b2: read(onGrid(grid.b2)),
  };
}

/**
 * The power a large convex particle in random orientation reflects from its outer surface, a share
 * of its geometric cross-section: ∫ ½(R⊥ + R∥) sin 2i di at the complex index, a sphere's, which any
 * convex particle's random orientation shares (van de Hulst 1957, ch. 8).
 *
 * @throws RangeError as {@link geometricLimit}.
 */
export function meanReflectance(index: ComplexIndex): number {
  const { n, k } = index;
  if (!(n > 0 && Number.isFinite(n) && k >= 0 && Number.isFinite(k))) {
    throw new RangeError(`refractive index ${n} + ${k}i needs a finite n > 0 and a finite k ≥ 0`);
  }
  const step = (0.5 * Math.PI) / RAY_SAMPLES;
  let total = 0;
  let reflected = 0;
  for (let s = 0; s < RAY_SAMPLES; s += 1) {
    const i = (s + 0.5) * step;
    const sinI = sin(i);
    const weight = sin(2 * i) * step;
    const outer = reflection(sinI * sinI, cos(i), index);
    total += weight;
    reflected += weight * 0.5 * (outer.perp + outer.par);
  }
  return reflected / total;
}
