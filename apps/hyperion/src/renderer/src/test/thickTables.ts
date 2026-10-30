/**
 * Synthetic thick bakes and polarisation corrections for the read path's tests and the smoke page's
 * checks (plan R08, R08.T14.c and R08.T14.f): `bake.ts`'s `MsSourceTable` and `polarisation.ts`'s
 * `PolarisationTable` shapes on the bake's grid, with values set so that the packed table's J⁰, G
 * and G₂ (`thick/read.ts`) follow a pattern in every axis, without running a solve. The values are a
 * test's, not a world's.
 */

import type { AtmosphereMedium } from "../view/atmosphere/medium";
import type { TablesBand } from "../view/atmosphere/tables";
import type { MsSourceSlot, MsSourceTable } from "../view/atmosphere/thick/bake";
import type { PolarisationSlot, PolarisationTable } from "../view/atmosphere/thick/polarisation";
import {
  THICK_ALTITUDES,
  THICK_CORRECTION_MODES,
  THICK_SOURCE_MODES,
  THICK_SUN_COSINES,
  THICK_TRAVEL_COSINES,
  thickSunCosineAt,
} from "../view/atmosphere/thick/read";

/** J⁰ and G at a point of the pattern, per channel, sr⁻¹ per unit of scattering. */
export interface ThickPatternPoint {
  readonly level: readonly [number, number, number];
  readonly slope: readonly [number, number, number];
}

/**
 * The pattern's J⁰ and G at altitude `a`, sun cosine `s`, travel cosine `v`, band `b` and light slot
 * `k`: smooth in each, J⁰ in [0.01, 0.07] sr⁻¹ (of Ψ_ms's order), G a fifth of it with either sign.
 */
export function thickPatternAt(
  a: number,
  s: number,
  v: number,
  b: number,
  k: number,
): ThickPatternPoint {
  const base = (c: number): number =>
    0.04 +
    0.012 * Math.cos(0.17 * a + 0.3 * c + 0.4 * b) +
    0.01 * Math.sin(0.21 * s + 0.5 * k) +
    0.008 * Math.cos(0.37 * v + 0.2 * c);
  const slope = (c: number): number => 0.2 * base(c) * Math.sin(0.29 * v + 0.13 * s - 0.4 * c + b);
  return { level: [base(0), base(1), base(2)], slope: [slope(0), slope(1), slope(2)] };
}

/**
 * The pattern's grid: altitudes evenly from the ground to the medium's top, and the bake's cosines,
 * its sun cosines below the horizontal among them (R08.T14.d.2).
 */
function patternGrid(medium: AtmosphereMedium): {
  readonly altitudesM: Float64Array;
  readonly sunCosines: Float64Array;
  readonly travelCosines: Float64Array;
} {
  const top = medium.topHeightM;
  return {
    altitudesM: Float64Array.from(
      { length: THICK_ALTITUDES },
      (_, a) => (top * a) / (THICK_ALTITUDES - 1),
    ),
    sunCosines: Float64Array.from({ length: THICK_SUN_COSINES }, (_, s) => thickSunCosineAt(s)),
    travelCosines: Float64Array.from(
      { length: THICK_TRAVEL_COSINES },
      (_, v) => (v + 0.5) / (THICK_TRAVEL_COSINES / 2) - 1,
    ),
  };
}

/**
 * A bake whose packed J⁰ and G are {@link thickPatternAt}'s and whose m = 2 is 0, one table a band
 * of `bands` (a tables plan's, in its order), with `slots` light slots, on the bake's grid:
 * altitudes evenly from the ground to `medium`'s top, the table's 46 μ₀ (`thick/read.ts`'s
 * `thickSunCosineAt`, the rows below the horizontal with the same pattern as those above) and
 * μ_v = (v + ½) ÷ 8 − 1.
 * Its J is in sr⁻¹ per unit of the kernels' scattering, as the bake's (`bake.ts`'s `MsSourceSlot`),
 * so that the packed table is the pattern over any medium.
 */
export function patternedThickBake(
  bands: ReadonlyArray<TablesBand>,
  medium: AtmosphereMedium,
  slots: number,
): MsSourceTable[] {
  const { altitudesM, sunCosines, travelCosines } = patternGrid(medium);
  return bands.map((band, b): MsSourceTable => {
    const slotTables = Array.from({ length: slots }, (_, k): MsSourceSlot => {
      const values = new Float32Array(
        THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * THICK_SOURCE_MODES * 3,
      );
      for (let a = 0; a < THICK_ALTITUDES; a += 1) {
        for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
          for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
            const point = thickPatternAt(a, s, v, b, k);
            const mu0 = sunCosines[s] ?? Number.NaN;
            const muV = travelCosines[v] ?? Number.NaN;
            const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
            const at =
              ((a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * THICK_SOURCE_MODES * 3;
            for (let c = 0; c < 3; c += 1) {
              values[at + c] = point.level[c] ?? Number.NaN;
              values[at + 3 + c] = (point.slope[c] ?? Number.NaN) * sines;
            }
          }
        }
      }
      return {
        values,
        bins: [],
        solves: 0,
        doubleForwardShare: 0,
        flooredSlantFactors: 0,
        largestBeamDeficit: 0,
      };
    });
    return {
      band: {
        gravityRatio: band.gravityRatio,
        latitudeRad: band.latitudeRad,
        bottomRadiusM: band.bottomRadiusM,
        coefficientFactor: band.coefficientFactor,
      },
      altitudesM,
      sunCosines,
      travelCosines,
      modes: THICK_SOURCE_MODES,
      verticalOpticalDepth: new Float64Array(15),
      slots: slotTables,
    };
  });
}

/** ΔJ_0, ΔJ_1 ÷ (sin θ_v sin θ₀) and ΔJ_2 ÷ (sin θ_v sin θ₀)² at a point, per channel, sr⁻¹. */
export interface ThickCorrectionPoint {
  readonly level: readonly [number, number, number];
  readonly slope: readonly [number, number, number];
  readonly curve: readonly [number, number, number];
}

/**
 * The correction's pattern at altitude `a`, sun cosine `s`, travel cosine `v`, band `b` and light slot
 * `k` (R08.T14.f): smooth in each and of either sign, ΔJ_0 at most a tenth of
 * {@link thickPatternAt}'s J⁰, ΔJ_1's quotient 8% of it and ΔJ_2's 5%.
 */
export function thickCorrectionPatternAt(
  a: number,
  s: number,
  v: number,
  b: number,
  k: number,
): ThickCorrectionPoint {
  const { level: base } = thickPatternAt(a, s, v, b, k);
  const at = (c: number): number => base[c] ?? Number.NaN;
  const level = (c: number): number =>
    0.1 * at(c) * Math.sin(0.23 * a - 0.31 * s + 0.17 * v + 0.5 * c + 0.7 * b + 0.3 * k);
  const slope = (c: number): number =>
    0.08 * at(c) * Math.cos(0.19 * a + 0.27 * v - 0.4 * c + b - 0.2 * k);
  const curve = (c: number): number =>
    0.05 * at(c) * Math.sin(0.33 * s + 0.41 * v + 0.6 * c - 0.5 * b - k);
  return {
    level: [level(0), level(1), level(2)],
    slope: [slope(0), slope(1), slope(2)],
    curve: [curve(0), curve(1), curve(2)],
  };
}

/**
 * A polarisation correction whose ΔJ is {@link thickCorrectionPatternAt}'s (R08.T14.f), one table a
 * band of `bands`, with `slots` light slots, on {@link patternedThickBake}'s grid for `medium`. ΔJ is
 * per unit of the whole σ_s, so that over a medium with no forward delta the packed correction is
 * the pattern itself.
 */
export function patternedCorrection(
  bands: ReadonlyArray<TablesBand>,
  medium: AtmosphereMedium,
  slots: number,
): PolarisationTable[] {
  const { altitudesM, sunCosines, travelCosines } = patternGrid(medium);
  return bands.map((band, b): PolarisationTable => {
    const slotTables = Array.from({ length: slots }, (_, k): PolarisationSlot => {
      const values = new Float32Array(
        THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * THICK_CORRECTION_MODES * 3,
      );
      for (let a = 0; a < THICK_ALTITUDES; a += 1) {
        for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
          for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
            const point = thickCorrectionPatternAt(a, s, v, b, k);
            const mu0 = sunCosines[s] ?? Number.NaN;
            const muV = travelCosines[v] ?? Number.NaN;
            const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
            const at =
              ((a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * THICK_CORRECTION_MODES * 3;
            for (let c = 0; c < 3; c += 1) {
              values[at + c] = point.level[c] ?? Number.NaN;
              values[at + 3 + c] = (point.slope[c] ?? Number.NaN) * sines;
              values[at + 6 + c] = (point.curve[c] ?? Number.NaN) * sines * sines;
            }
          }
        }
      }
      return {
        values,
        solves: 0,
        complexPairs: 0,
        imaginaryShares: [Number.POSITIVE_INFINITY, 0],
        nonsymmetricLayers: 0,
      };
    });
    return {
      band: {
        gravityRatio: band.gravityRatio,
        latitudeRad: band.latitudeRad,
        bottomRadiusM: band.bottomRadiusM,
        coefficientFactor: band.coefficientFactor,
      },
      altitudesM,
      sunCosines,
      travelCosines,
      modes: THICK_CORRECTION_MODES,
      slots: slotTables,
    };
  });
}

/** {@link leastZerothModes}'s least J⁰: above the horizontal, sr⁻¹, and below it, of the largest. */
export interface ZerothModesLeast {
  readonly above: number;
  readonly belowShare: number;
}

/**
 * A bake's least J⁰ (its m = 0, every channel; `bake.ts`'s `[altitude][μ₀][μ_v][m][RGB]`) over the
 * rows above the horizontal, and over those below it as a share of its largest J⁰ (R08.T14.d.2).
 * Each bin's J⁰ is at least 0, but a channel's is the bins' converted by signed matching functions
 * (`binConversion.ts`), so a near-dark twilight row's steep spectrum can take it a hair below 0:
 * −1.8 × 10⁻⁹ sr⁻¹ in TEST EARTH's blue at μ₀ −0.164, 3 × 10⁻⁸ of its largest, which the read holds
 * at 0.
 */
export function leastZerothModes(
  values: ArrayLike<number>,
  sunCosines: Float64Array,
): ZerothModesLeast {
  const perSun = THICK_TRAVEL_COSINES * THICK_SOURCE_MODES * 3;
  let above = Number.POSITIVE_INFINITY;
  let below = Number.POSITIVE_INFINITY;
  let largest = 0;
  for (let i = 0; i < values.length; i += 1) {
    if (Math.floor(i / 3) % THICK_SOURCE_MODES !== 0) {
      continue;
    }
    const value = values[i] ?? Number.NaN;
    largest = Math.max(largest, value);
    if ((sunCosines[Math.floor(i / perSun) % sunCosines.length] ?? Number.NaN) > 0) {
      above = Math.min(above, value);
    } else {
      below = Math.min(below, value);
    }
  }
  return { above, belowShare: below / largest };
}
