import { describe, expect, it } from "vitest";

import bakeSpectra from "../../../../../../../packages/protocol/fixtures/bake_spectra.json" with { type: "json" };
import type { Rgb } from "../photometry/toneCurve";
import { EARTH_FIT_COLUMN, earthFitAir, molecularAir, type SpectralAir } from "./channels";
import { EARTH_DRY_AIR, RAYLEIGH_SCALE_HEIGHT_M } from "./earth";
import { CHANNEL_WAVELENGTHS_NM, densityAt } from "./medium";
import { distanceToTopM, intersectsGround, localRadiusM, type Shell } from "./opticalDepth";
import {
  bakeSpectrumOf,
  deltaUv,
  PIPELINE_WAVELENGTHS_NM,
  type SunWeights,
  sunWeights,
  weightedRgb,
} from "./spectralColour";

/**
 * The limb from orbit in R08.T4.a's colour pipeline (plan R08, R08.T8: "the fitted triple's u′v′
 * error on the limb seen from orbit … its direct beam and singly scattered light, as the channel
 * fit's objective models them"). Views from orbit are not in the fit's family (`channels.ts`'s
 * `channelFitCases`, which looks from the ground), so the geometry is the chord's own here, and
 * the physics the objective's: the direct beam e^(−Σ β_ext N), and the single scattering
 * Σ β_sca P ρ Δs e^(−Σ β_ext (N_eye + N_sun)) with the ground's shadow, no ground and no multiple
 * scattering, the spectral colour on the pipeline's 1 nm rows and the three-channel one at the
 * fitted triple (`CHANNEL_WAVELENGTHS_NM`), each the worst over R06's twelve suns, and the direct
 * beam's under the Sun alone too.
 */

/** The limb set of Design note 10: tangent heights in Rayleigh scale heights of R05's Earth. */
const LIMB_SCALE_HEIGHTS = [0.1, 0.3, 1, 3, 10] as const;

/** The view chord's steps on each side of the tangent point, placed quadratically toward it. */
const HALF_CHORD_STEPS = 512;

/** The steps of each sample's column to the top toward the sun: the objective's `COLUMN_STEPS`. */
const SUN_COLUMN_STEPS = 128;

/** The distance scale of a sun column's first steps, m: the objective's `FIRST_STEP_M`. */
const FIRST_STEP_M = 20;

/**
 * Where the sun stands for the singly scattered light, 90° from the chord at the tangent point:
 * overhead there, or on its horizon across the chord (the terminator); `behind` for the direct
 * beam, the sun seen through the limb, which takes no column toward it.
 */
type LimbSun = "overhead" | "across" | "behind";

/** One point of a chord's quadrature: its height, its step and each term's column toward the sun. */
interface ChordPoint {
  readonly heightM: number;
  readonly stepM: number;
  /**
   * Per term, the column from the point to the top toward the sun, m; `null` in the ground's
   * shadow, or for the direct beam's chord.
   */
  readonly toSunM: Float64Array | null;
}

/** Each term's column along a ray from radius `rM` at zenith cosine `mu` to the top, m (the objective's). */
function columnsToTopM(air: SpectralAir, shell: Shell, rM: number, mu: number): Float64Array {
  const lengthM = distanceToTopM(shell, rM, mu);
  const a = Math.log1p(lengthM / FIRST_STEP_M);
  const scaleM = lengthM / Math.expm1(a);
  const columns = new Float64Array(air.terms.length);
  for (let i = 0; i < SUN_COLUMN_STEPS; i += 1) {
    const u = (i + 0.5) / SUN_COLUMN_STEPS;
    const sM = scaleM * Math.expm1(a * u);
    const dsM = (scaleM * a * Math.exp(a * u)) / SUN_COLUMN_STEPS;
    const heightM = localRadiusM(rM, mu, sM) - shell.bottomRadiusM;
    for (const [t, term] of air.terms.entries()) {
      columns[t] = (columns[t] ?? 0) + densityAt(term.density, heightM) * dsM;
    }
  }
  return columns;
}

/**
 * The chord through `air` whose tangent point is `tangentM` above its ground, as seen from outside
 * the air (from 400 km, above the fit column's top at 138.9 km): its points from the entry at the
 * top, in order, and each one's column toward the sun.
 */
function chordOf(
  air: SpectralAir,
  tangentM: number,
  sun: LimbSun,
  halfSteps: number,
): ChordPoint[] {
  const shell: Shell = {
    bottomRadiusM: air.groundRadiusM,
    topRadiusM: air.groundRadiusM + air.topHeightM,
  };
  const tangentRadiusM = air.groundRadiusM + tangentM;
  const halfM = Math.sqrt(shell.topRadiusM ** 2 - tangentRadiusM ** 2);
  // The chord along +x at y = tangentRadiusM; the sun overhead at the tangent point is +y, and on
  // its horizon across the chord +z.
  const offsets: Array<{ readonly xM: number; readonly stepM: number }> = [];
  for (let k = 0; k < halfSteps; k += 1) {
    const a = halfM * (k / halfSteps) ** 2;
    const b = halfM * ((k + 1) / halfSteps) ** 2;
    offsets.push({ xM: (a + b) / 2, stepM: b - a }, { xM: -(a + b) / 2, stepM: b - a });
  }
  offsets.sort((p, q) => p.xM - q.xM);
  return offsets.map(({ xM, stepM }) => {
    const rM = Math.hypot(xM, tangentRadiusM);
    const mu = sun === "overhead" ? tangentRadiusM / rM : 0;
    const shadowed = sun === "behind" || intersectsGround(shell, rM, mu);
    return {
      heightM: rM - air.groundRadiusM,
      stepM,
      toSunM: shadowed ? null : columnsToTopM(air, shell, rM, mu),
    };
  });
}

/** R(λ) of the direct beam through the whole chord: its transmittance. */
function beamResponse(
  air: SpectralAir,
  chord: readonly ChordPoint[],
  wavelengthNm: number,
): number {
  let depth = 0;
  for (const term of air.terms) {
    const extinctionPerM = term.scatteringPerM(wavelengthNm) + term.absorptionPerM(wavelengthNm);
    for (const point of chord) {
      depth += extinctionPerM * densityAt(term.density, point.heightM) * point.stepM;
    }
  }
  return Math.exp(-depth);
}

/**
 * R(λ) of the singly scattered light along the chord toward the eye, per unit irradiance normal to
 * the sun's beam, sr⁻¹, at 90° from the sun (the objective's sky case: each point stands for its
 * step, the column from the eye reaching the steps before it and half its own).
 */
function scatteredResponse(
  air: SpectralAir,
  chord: readonly ChordPoint[],
  wavelengthNm: number,
): number {
  const terms = air.terms.map((term) => ({
    term,
    scatteringPerM: term.scatteringPerM(wavelengthNm),
    extinctionPerM: term.scatteringPerM(wavelengthNm) + term.absorptionPerM(wavelengthNm),
    phasePerSr: term.phasePerSr(0, wavelengthNm),
  }));
  const toEye = new Float64Array(terms.length);
  let radiance = 0;
  for (const point of chord) {
    let depth = 0;
    let source = 0;
    for (const [t, { term, scatteringPerM, extinctionPerM, phasePerSr }] of terms.entries()) {
      const density = densityAt(term.density, point.heightM);
      const toSun = point.toSunM?.[t];
      depth += extinctionPerM * ((toEye[t] ?? 0) + (density * point.stepM) / 2 + (toSun ?? 0));
      if (toSun !== undefined) {
        source += scatteringPerM * phasePerSr * density * point.stepM;
      }
      toEye[t] = (toEye[t] ?? 0) + density * point.stepM;
    }
    radiance += source * Math.exp(-depth);
  }
  return radiance;
}

/** The suns' colour weights: R06's twelve bake spectra. */
const TWELVE_SUNS = bakeSpectra.suns.map((row) => sunWeights(bakeSpectrumOf(row.bake_spectrum)));

/** The Sun's: the 5,772 K main-sequence row, R06's `solar_colour()` (`channels.test.ts`'s `THE_SUN`). */
const THE_SUN = ((): readonly SunWeights[] => {
  const row = bakeSpectra.suns.find((s) => s.teff_k === 5_772 && s.grid === "MainSequence");
  if (row === undefined) {
    throw new Error("bake_spectra.json holds the Sun's row");
  }
  return [sunWeights(bakeSpectrumOf(row.bake_spectrum))];
})();

/** The worst Δu′v′ over `stars` between a response's spectral and three-channel colours. */
function worstDeltaUv(
  response: (wavelengthNm: number) => number,
  stars: readonly SunWeights[],
): number {
  const spectral = PIPELINE_WAVELENGTHS_NM.map(response);
  const channels: Rgb = [
    response(CHANNEL_WAVELENGTHS_NM[0]),
    response(CHANNEL_WAVELENGTHS_NM[1]),
    response(CHANNEL_WAVELENGTHS_NM[2]),
  ];
  return Math.max(
    ...stars.map((weights) =>
      deltaUv(weightedRgb(weights, spectral), [
        weights.rgb[0] * channels[0],
        weights.rgb[1] * channels[1],
        weights.rgb[2] * channels[2],
      ]),
    ),
  );
}

/**
 * Each tangent height's worst Δu′v′ over `stars` of the light through the limb of `air`: the
 * direct beam with the sun behind the limb, or the singly scattered light with the sun overhead or
 * across.
 */
function limbErrors(
  air: SpectralAir,
  sun: LimbSun,
  halfSteps: number,
  stars: readonly SunWeights[],
): number[] {
  return LIMB_SCALE_HEIGHTS.map((k) => {
    const chord = chordOf(air, k * RAYLEIGH_SCALE_HEIGHT_M, sun, halfSteps);
    return worstDeltaUv(
      (nm) => (sun === "behind" ? beamResponse(air, chord, nm) : scatteredResponse(air, chord, nm)),
      stars,
    );
  });
}

/** Dry air on R08.T4.a's fit column, and the same with R05's aerosol (`channels.ts`). */
const DRY_AIR = molecularAir("Earth", EARTH_FIT_COLUMN, EARTH_DRY_AIR, 1);
const HAZY_AIR = earthFitAir();

/** Each figure over its recorded value. */
function overRecorded(measured: readonly number[], recorded: readonly number[]): number[] {
  return measured.map((value, i) => value / (recorded[i] ?? Number.NaN));
}

/**
 * Every figure within 0.5% of its recorded value, so that a change shows: the figures are
 * deterministic, and within 10⁻⁴ of themselves when the chord's steps are doubled (3 × 10⁻⁵
 * measured; the last test; each sun column's 128 steps are not doubled).
 */
const AS_RECORDED = LIMB_SCALE_HEIGHTS.map(() => expect.closeTo(1, 2));

// Measured 2026-10-21 and recorded in plan R08's Risks ("Deviations in T8.a, as built"), at tangent
// heights of 0.1, 0.3, 1, 3 and 10 of R05's Rayleigh scale heights (8,434.5 m). A limb chord at
// 0.1 H carries 1.8 times the horizon's molecular column and 0.99 times its aerosol (τ(550) 15.1
// against 12.5 with the haze, 6.05 against 3.34 dry). Its direct beam is off by 0.052 dry and 0.105
// hazy, worst over R06's twelve suns (the 2,300 K star), against the horizon's 0.072 over the same
// suns (0.022 under the Sun; R08.T4.a), and by 0.011 dry and 0.059 hazy under the Sun alone; the
// singly scattered light stays within 0.0073 over the twelve.
describe("the fitted triple's u′v′ on the limb seen from orbit (R08.T8, recorded)", () => {
  it("records the direct beam's error through the limb of dry air: 0.052 at 0.1 H, 5 × 10⁻⁶ at 10 H", () => {
    const recorded = [0.05214, 0.03672, 0.01255, 0.004107, 4.795e-6];
    expect(
      overRecorded(limbErrors(DRY_AIR, "behind", HALF_CHORD_STEPS, TWELVE_SUNS), recorded),
    ).toEqual(AS_RECORDED);
  }, 60_000);

  it("records the direct beam's error with R05's aerosol: 0.105 at 0.1 H", () => {
    const recorded = [0.1046, 0.05041, 0.01256, 0.004107, 4.795e-6];
    expect(
      overRecorded(limbErrors(HAZY_AIR, "behind", HALF_CHORD_STEPS, TWELVE_SUNS), recorded),
    ).toEqual(AS_RECORDED);
  }, 60_000);

  it("records dry air's scattered light's error with the sun overhead at the tangent point: under 0.0073", () => {
    const recorded = [8.536e-4, 0.001452, 0.003431, 0.006148, 0.007206];
    expect(
      overRecorded(limbErrors(DRY_AIR, "overhead", HALF_CHORD_STEPS, TWELVE_SUNS), recorded),
    ).toEqual(AS_RECORDED);
  }, 60_000);

  it("records it with R05's aerosol and the sun overhead: under 0.0073", () => {
    const recorded = [0.001945, 0.001688, 0.003412, 0.006148, 0.007206];
    expect(
      overRecorded(limbErrors(HAZY_AIR, "overhead", HALF_CHORD_STEPS, TWELVE_SUNS), recorded),
    ).toEqual(AS_RECORDED);
  }, 60_000);

  it("records dry air's scattered light's error with the sun on the tangent point's horizon: under 0.0073", () => {
    const recorded = [0.003621, 0.003221, 0.00281, 0.005464, 0.007203];
    expect(
      overRecorded(limbErrors(DRY_AIR, "across", HALF_CHORD_STEPS, TWELVE_SUNS), recorded),
    ).toEqual(AS_RECORDED);
  }, 60_000);

  it("records it with R05's aerosol and the sun on the horizon: under 0.0073", () => {
    const recorded = [0.001119, 0.00105, 0.002783, 0.005464, 0.007203];
    expect(
      overRecorded(limbErrors(HAZY_AIR, "across", HALF_CHORD_STEPS, TWELVE_SUNS), recorded),
    ).toEqual(AS_RECORDED);
  }, 60_000);

  it("records the direct beam's error under the Sun alone, dry and with R05's aerosol: 0.011 and 0.059 at 0.1 H", () => {
    const dry = [0.01128, 0.01049, 0.007567, 0.003024, 4.642e-6];
    const hazy = [0.05867, 0.01066, 0.007595, 0.003024, 4.642e-6];
    expect({
      dry: overRecorded(limbErrors(DRY_AIR, "behind", HALF_CHORD_STEPS, THE_SUN), dry),
      hazy: overRecorded(limbErrors(HAZY_AIR, "behind", HALF_CHORD_STEPS, THE_SUN), hazy),
    }).toEqual({ dry: AS_RECORDED, hazy: AS_RECORDED });
  }, 60_000);

  it("has its quadrature converged: twice the chord's steps moves no figure by 10⁻⁴ of itself", () => {
    const cases = [
      ["dry air", DRY_AIR],
      ["R05's aerosol", HAZY_AIR],
    ] as const;
    const moved: string[] = [];
    for (const [name, air] of cases) {
      for (const [sun, stars, over] of [
        ["behind", TWELVE_SUNS, "twelve suns"],
        ["overhead", TWELVE_SUNS, "twelve suns"],
        ["across", TWELVE_SUNS, "twelve suns"],
        ["behind", THE_SUN, "the Sun"],
      ] as const) {
        const coarse = limbErrors(air, sun, HALF_CHORD_STEPS, stars);
        const fine = limbErrors(air, sun, 2 * HALF_CHORD_STEPS, stars);
        for (const [i, e] of coarse.entries()) {
          if (!(Math.abs(e / (fine[i] ?? Number.NaN) - 1) < 1e-4)) {
            moved.push(`${name}, sun ${sun}, ${over}, ${String(LIMB_SCALE_HEIGHTS[i])} H`);
          }
        }
      }
    }
    expect(moved).toEqual([]);
  }, 120_000);
});
