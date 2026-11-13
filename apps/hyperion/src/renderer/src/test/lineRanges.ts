/**
 * The line tables' range test (plan R08, `decision-r08-t4d-kdist.md` §3.1, as R08.T4.g built it and
 * R08.T4.h reuses it): a table starts where a pure vapour's saturated column is negligible, R08.T10.a's
 * Δ_c under 10⁻³ at k = 100 over 40 p_sat ÷ (m g) at g = 0.5 m s⁻², for T4.b's three test suns.
 */

import bakeSpectra from "../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import {
  cellMeanSigma,
  type KDistributionTable,
  kDistributionAt,
} from "../view/atmosphere/absorbers/kDistribution";
import { BAKE_RANGE_NM } from "../view/atmosphere/medium";
import {
  type BakeSpectrum,
  bakeSpectrumAt,
  bakeSpectrumOf,
  matchingFunctionsAt,
} from "../view/atmosphere/spectralColour";
import { planckBakeSpectrum } from "./atmosphereFixtures";

/** The Sun: R06's `solar_colour()`, the 5,772 K main-sequence row of `bake_spectra.json`. */
export const THE_SUN: BakeSpectrum = (() => {
  const row = bakeSpectra.suns.find((s) => s.teff_k === 5_772 && s.grid === "MainSequence");
  if (row === undefined) {
    throw new Error("bake_spectra.json holds no 5,772 K main-sequence sun");
  }
  return bakeSpectrumOf(row.bake_spectrum);
})();

/** T4.b's test suns: the Sun and the 2,500 K and 30,000 K Planck suns. */
export const RANGE_SUNS: ReadonlyArray<readonly [string, BakeSpectrum]> = [
  ["the Sun", THE_SUN],
  ["a 2,500 K Planck sun", planckBakeSpectrum(2_500)],
  ["a 30,000 K Planck sun", planckBakeSpectrum(30_000)],
];

/** The atomic mass constant m_u, kg (CODATA 2018). */
const ATOMIC_MASS_KG = 1.660_539_066_6e-27;

/**
 * The saturated column of a pure vapour over a grazing path, m⁻²: 40 p_sat ÷ (m g) at
 * g = 0.5 m s⁻² (`decision-r08-t4d-kdist.md` §3.1), for p_sat in Pa and m in Da.
 */
export function saturatedColumnPerM2(saturationPa: number, molecularMassDa: number): number {
  return (40 * saturationPa) / (molecularMassDa * ATOMIC_MASS_KG * 0.5);
}

/** Each 1 nm cell's channel weights c̄_c S over 380–760 nm for a sun, by T4.b's weighting rule. */
export function cellWeights(sun: BakeSpectrum): ReadonlyArray<Float64Array> {
  const [low, high] = BAKE_RANGE_NM;
  const cells = high - low + 1;
  const weights = [new Float64Array(cells), new Float64Array(cells), new Float64Array(cells)];
  for (let c = 0; c < cells; c += 1) {
    for (let j = 0; j < 20; j += 1) {
      const nm = low + c - 0.5 + (j + 0.5) / 20;
      const s = bakeSpectrumAt(sun, Math.min(Math.max(nm, low), high)) / 20;
      const m = matchingFunctionsAt(Math.min(Math.max(nm, low), high));
      for (const [k, row] of weights.entries()) {
        row[c] = (row[c] ?? 0) + (m[k] ?? 0) * s;
      }
    }
  }
  return weights;
}

/**
 * R08.T10.a's Δ_c, the most a pending species' cells could move a channel's light, at k = 100
 * (follow-up 4, ruling 3): Σ |c̄_c| S min(1, k ⟨σ⟩ u) ÷ Σ c̄_c S, the largest over the channels.
 */
export function largestChange(
  weights: ReadonlyArray<Float64Array>,
  cellMeansM2: Float64Array,
  columnPerM2: number,
): number {
  return Math.max(
    ...weights.map((row) => {
      let changed = 0;
      let total = 0;
      for (let c = 0; c < row.length; c += 1) {
        const w = row[c] ?? 0;
        changed += Math.abs(w) * Math.min(1, 100 * (cellMeansM2[c] ?? 0) * columnPerM2);
        total += w;
      }
      return changed / total;
    }),
  );
}

/** A table's cell means at a temperature, m², on 380–760 nm, from its 10 Pa row. */
export function cellMeansAt(table: KDistributionTable, temperatureK: number): Float64Array {
  const kd = kDistributionAt(table, temperatureK);
  const out = new Float64Array(BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0] + 1);
  for (const cell of kd.cells) {
    const g = cell.weights.length;
    out[cell.cellNm - BAKE_RANGE_NM[0]] = cellMeanSigma(cell.weights, cell.sigmas.slice(0, g));
  }
  return out;
}

/**
 * Where Δ_c at k = 100 reaches 10⁻³ for a sun, K, by 20 halvings of the table's span (to
 * 4.3 × 10⁻⁴ K over 450 K): the saturation pressure a function of the temperature, Pa.
 */
export function rangeCrossingK(
  table: KDistributionTable,
  sun: BakeSpectrum,
  saturationPa: (temperatureK: number) => number,
  molecularMassDa: number,
): number {
  const weights = cellWeights(sun);
  let lo = table.temperaturesK[0] ?? Number.NaN;
  let hi = table.temperaturesK.at(-1) ?? Number.NaN;
  for (let k = 0; k < 20; k += 1) {
    const mid = 0.5 * (lo + hi);
    const column = saturatedColumnPerM2(saturationPa(mid), molecularMassDa);
    if (largestChange(weights, cellMeansAt(table, mid), column) < 1e-3) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  return lo;
}
