/**
 * Synthetic thick bakes for the read path's tests and the smoke page's checks (plan R08,
 * R08.T14.c): `bake.ts`'s `MsSourceTable` shapes on the bake's grid, with values set so that the
 * packed table's J⁰ and G (`thick/read.ts`) follow a pattern in every axis, without running a solve.
 * The values are a test's, not a world's.
 */

import { type AtmosphereMedium, densityAt } from "../view/atmosphere/medium";
import type { TablesBand } from "../view/atmosphere/tables";
import type { MsSourceSlot, MsSourceTable } from "../view/atmosphere/thick/bake";
import {
  THICK_ALTITUDES,
  THICK_MODES,
  THICK_SUN_COSINES,
  THICK_TRAVEL_COSINES,
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
 * A bake whose packed J⁰ and G are {@link thickPatternAt}'s, one table a band of `bands` (a
 * tables plan's, in its order), with `slots` light slots, on the bake's grid: altitudes evenly from
 * the ground to the medium's top, μ₀ = (s + ½) ÷ 32 and μ_v = (v + ½) ÷ 8 − 1. E is formed from
 * the pattern by the kernels' scattering at each altitude, so that the medium given is the one the
 * kernels read (`tables.ts`'s `kernelMedium`, or the drawn medium where no term has a forward delta).
 */
export function patternedThickBake(
  bands: ReadonlyArray<TablesBand>,
  kernelMedium: AtmosphereMedium,
  slots: number,
): MsSourceTable[] {
  const top = kernelMedium.topHeightM;
  const altitudesM = Float64Array.from(
    { length: THICK_ALTITUDES },
    (_, a) => (top * a) / (THICK_ALTITUDES - 1),
  );
  const sunCosines = Float64Array.from(
    { length: THICK_SUN_COSINES },
    (_, s) => (s + 0.5) / THICK_SUN_COSINES,
  );
  const travelCosines = Float64Array.from(
    { length: THICK_TRAVEL_COSINES },
    (_, v) => (v + 0.5) / (THICK_TRAVEL_COSINES / 2) - 1,
  );
  const scattering = Array.from(altitudesM, (h) =>
    [0, 1, 2].map((c) =>
      kernelMedium.terms.reduce(
        (sum, term) => sum + (term.scattering[c] ?? 0) * densityAt(term.density, h),
        0,
      ),
    ),
  );
  return bands.map((band, b): MsSourceTable => {
    const slotTables = Array.from({ length: slots }, (_, k): MsSourceSlot => {
      const values = new Float32Array(
        THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * THICK_MODES * 3,
      );
      for (let a = 0; a < THICK_ALTITUDES; a += 1) {
        for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
          for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
            const point = thickPatternAt(a, s, v, b, k);
            const mu0 = sunCosines[s] ?? Number.NaN;
            const muV = travelCosines[v] ?? Number.NaN;
            const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
            const at = ((a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * THICK_MODES * 3;
            for (let c = 0; c < 3; c += 1) {
              const sigma = scattering[a]?.[c] ?? Number.NaN;
              values[at + c] = (point.level[c] ?? Number.NaN) * sigma;
              values[at + 3 + c] = (point.slope[c] ?? Number.NaN) * sigma * sines;
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
      modes: THICK_MODES,
      verticalOpticalDepth: new Float64Array(15),
      slots: slotTables,
    };
  });
}
