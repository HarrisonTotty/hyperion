/**
 * The render channels' wavelengths, fitted (plan R08, Design note 5; R08.T4.a): the case family,
 * its objective and the search, which `channels.test.ts` runs and records in `channels.json`.
 *
 * @remarks
 * The per-frame tables are three-channel (Hillaire 2020; R08's budget): a smooth term (Rayleigh, an
 * aerosol) is evaluated at three wavelengths and multiplied by the star's linear Rec. 709 colour.
 * That is exact only for light whose spectrum the air changes by a constant factor across each
 * channel's matching function, so the wavelengths are a fit, not the code constant of Bruneton's
 * `precomputed_atmospheric_scattering` (680, 550, 440 nm, `atmosphere/model.h`). In this module's
 * pipeline (the star's colour times R at the three wavelengths, as R08.T6.d draws; not Bruneton
 * 2017's own conversion, §14.3 eq. 2, which R05's `solar.ts` keeps), that constant draws Earth's Sun
 * 13% too bright 5° above the horizon, 41% at 2° and 3.8 times at the horizon (Design note 5's "up
 * to 60% too bright"). The fit is a minimax of Δu′v′
 * (CIE 1976 UCS, CIE 015:2018 §8.1) between the three-channel colour and the spectral one over a
 * stated family of cases ({@link channelFitCases}): Design note 5's comparison of three-sample
 * rendering with a spectral reference (Bruneton 2017, arXiv 1612.04336, §14.3; Elek and Kmoch
 * 2010, "Real-time spectral scattering in large-scale natural participating media", SCCG 2010),
 * made the objective. The family is the stars of R06's table, not the Sun alone, and Earth, Mars
 * and every gas with Rayleigh optics here, not Earth's air alone ({@link channelFitAirs}), since
 * one triple serves every world. Both colours are `spectralColour.ts`'s.
 *
 * The module is the fit's, for tests: the renderer reads the fitted triple from
 * `fittedChannels.ts`, and the pipeline from `spectralColour.ts`.
 */

import type { Rgb } from "../photometry/toneCurve";
import { type AtmosphereColumn, hydrostaticColumn } from "./column";
import {
  AEROSOL_ANGSTROM_EXPONENT,
  AEROSOL_ASYMMETRY,
  AEROSOL_OPTICAL_DEPTH_550,
  AEROSOL_SCALE_HEIGHT_M,
  AEROSOL_SINGLE_SCATTERING_ALBEDO,
} from "./earth";
import { BAKE_RANGE_NM, columnLengthM, densityAt, type DensityProfile, phaseAt } from "./medium";
import { type LevelSpheroid, referenceGravity } from "./oblate";
import { distanceToTopM, intersectsGround, localRadiusM, type Shell } from "./opticalDepth";
import { ESTIMATED_RAYLEIGH, type GasFractions, GASES, molecularMixture } from "./rayleigh";
import { type AerosolMode, modeOptics } from "./sizeDistribution";
import {
  type BakeSpectrum,
  deltaUv,
  luminanceOfRgb,
  PIPELINE_WAVELENGTHS_NM,
  REC709_TO_XYZ,
  type SunWeights,
  sunWeights,
  uvOfRgb,
  weightedRgb,
} from "./spectralColour";

/** One term of an atmosphere at any wavelength: R08's medium term, spectrally. */
export interface SpectralAirTerm {
  readonly name: string;
  /** The relative density over height above the ground. */
  readonly density: DensityProfile;
  /** β_sca at relative density 1, m⁻¹, at a vacuum wavelength in nm. */
  readonly scatteringPerM: (wavelengthNm: number) => number;
  /** β_abs at relative density 1, m⁻¹, at a vacuum wavelength in nm. */
  readonly absorptionPerM: (wavelengthNm: number) => number;
  /**
   * The phase function, sr⁻¹, at the cosine between the light's directions of travel before and
   * after scattering, and a vacuum wavelength in nm.
   */
  readonly phasePerSr: (cosTheta: number, wavelengthNm: number) => number;
}

/** An atmosphere on a spherical shell, spectrally. */
export interface SpectralAir {
  readonly name: string;
  readonly groundRadiusM: number;
  readonly topHeightM: number;
  readonly terms: ReadonlyArray<SpectralAirTerm>;
}

/** Where a case looks from the ground. */
export type CaseView =
  /** The star's disc: its direct beam. */
  | { readonly kind: "sun"; readonly sunZenithDeg: number }
  /** The sky in one direction, singly scattered. */
  | {
      readonly kind: "sky";
      readonly sunZenithDeg: number;
      /** The view's elevation above the horizon, degrees. */
      readonly elevationDeg: number;
      /** The view's azimuth from the star's, degrees. */
      readonly azimuthFromSunDeg: number;
    };

/** A star, named, with its spectrum in the bake bins. */
export interface CaseSun {
  readonly name: string;
  readonly spectrum: BakeSpectrum;
}

/** One case of the fit: a star's light through an atmosphere, seen one way. */
export interface ChannelCase {
  readonly name: string;
  readonly sun: CaseSun;
  readonly air: SpectralAir;
  readonly view: CaseView;
}

/**
 * The steps of a column's quadrature to the top of the air: midpoints evenly spaced in u, the
 * distance s = L (e^(a u) − 1) ÷ (e^a − 1), so that the steps grow geometrically from
 * {@link FIRST_STEP_M}-scale at the ground, where the density is highest, to kilometres near the
 * top. On Earth's column it gives the vertical and the horizontal columns within 10⁻⁴ of a
 * 4,096-step sum (`channels.test.ts`).
 */
const COLUMN_STEPS = 128;

/**
 * The points of a sky view's single-scattering sum along the line of sight, on the same spacing.
 * The spectral and the three-channel colours are summed over the same points, so the quadrature's
 * error is common to both and the fit compares like with like.
 */
const VIEW_STEPS = 64;

/** The distance scale of a ray's first steps, m. */
const FIRST_STEP_M = 20;

/** A ray's quadrature: each midpoint's distance and step length, m. */
function raySteps(
  lengthM: number,
  steps: number,
): { readonly sM: Float64Array; readonly dsM: Float64Array } {
  const a = Math.log1p(lengthM / FIRST_STEP_M);
  const scale = lengthM / Math.expm1(a);
  const sM = new Float64Array(steps);
  const dsM = new Float64Array(steps);
  for (let i = 0; i < steps; i += 1) {
    const u = (i + 0.5) / steps;
    sM[i] = scale * Math.expm1(a * u);
    dsM[i] = (scale * a * Math.exp(a * u)) / steps;
  }
  return { sM, dsM };
}

/** Each term's column along a ray from radius `rM` at zenith cosine `mu` to the top, m. */
function columnsToTopM(air: SpectralAir, shell: Shell, rM: number, mu: number): Float64Array {
  const columns = new Float64Array(air.terms.length);
  const { sM, dsM } = raySteps(distanceToTopM(shell, rM, mu), COLUMN_STEPS);
  for (const [i, s] of sM.entries()) {
    const heightM = localRadiusM(rM, mu, s) - shell.bottomRadiusM;
    for (const [t, term] of air.terms.entries()) {
      columns[t] = (columns[t] ?? 0) + densityAt(term.density, heightM) * (dsM[i] ?? 0);
    }
  }
  return columns;
}

/**
 * A view's geometry, independent of wavelength: for the direct beam, each term's slant column; for
 * the sky, each quadrature point's step, local densities and columns (to the eye plus to the star),
 * and the scattering angle's cosine.
 */
type CaseGeometry =
  | { readonly kind: "sun"; readonly columnsM: Float64Array }
  | {
      readonly kind: "sky";
      readonly cosTheta: number;
      /** Per point and term, `t + terms × i`: the relative density times the step, m. */
      readonly weightsM: Float64Array;
      /** Per point and term: the column from the eye to the point and from it to the star, m. */
      readonly columnsM: Float64Array;
      readonly points: number;
    };

const DEG = Math.PI / 180;

function geometryOf(air: SpectralAir, view: CaseView): CaseGeometry {
  const shell: Shell = {
    bottomRadiusM: air.groundRadiusM,
    topRadiusM: air.groundRadiusM + air.topHeightM,
  };
  const groundM = air.groundRadiusM;
  const sun: readonly [number, number, number] = [
    Math.sin(view.sunZenithDeg * DEG),
    Math.cos(view.sunZenithDeg * DEG),
    0,
  ];
  if (view.kind === "sun") {
    return { kind: "sun", columnsM: columnsToTopM(air, shell, groundM, sun[1]) };
  }
  const elevation = view.elevationDeg * DEG;
  const azimuth = view.azimuthFromSunDeg * DEG;
  const look: readonly [number, number, number] = [
    Math.cos(elevation) * Math.cos(azimuth),
    Math.sin(elevation),
    Math.cos(elevation) * Math.sin(azimuth),
  ];
  const terms = air.terms.length;
  const { sM, dsM } = raySteps(distanceToTopM(shell, groundM, look[1]), VIEW_STEPS);
  const weightsM = new Float64Array(terms * VIEW_STEPS);
  const columnsM = new Float64Array(terms * VIEW_STEPS);
  const toEye = new Float64Array(terms);
  for (const [i, s] of sM.entries()) {
    const position = [s * look[0], groundM + s * look[1], s * look[2]] as const;
    const rM = Math.hypot(position[0], position[1], position[2]);
    const heightM = rM - groundM;
    const muSun = (position[0] * sun[0] + position[1] * sun[1] + position[2] * sun[2]) / rM;
    const lit = !intersectsGround(shell, rM, muSun);
    const toSun = lit ? columnsToTopM(air, shell, rM, muSun) : undefined;
    const ds = dsM[i] ?? 0;
    for (const [t, term] of air.terms.entries()) {
      // Each midpoint stands for its step, so the column from the eye reaches the steps before
      // it and half its own.
      const density = densityAt(term.density, heightM);
      weightsM[t + terms * i] = lit ? density * ds : 0;
      columnsM[t + terms * i] = (toEye[t] ?? 0) + (density * ds) / 2 + (toSun?.[t] ?? 0);
      toEye[t] = (toEye[t] ?? 0) + density * ds;
    }
  }
  const cosTheta = look[0] * sun[0] + look[1] * sun[1] + look[2] * sun[2];
  return { kind: "sky", cosTheta, weightsM, columnsM, points: VIEW_STEPS };
}

/** Every term's coefficients at one wavelength, m⁻¹ at relative density 1. */
interface AirOptics {
  readonly scattering: Float64Array;
  readonly extinction: Float64Array;
}

function opticsAt(air: SpectralAir, wavelengthNm: number): AirOptics {
  const scattering = new Float64Array(air.terms.length);
  const extinction = new Float64Array(air.terms.length);
  for (const [t, term] of air.terms.entries()) {
    const sca = term.scatteringPerM(wavelengthNm);
    scattering[t] = sca;
    extinction[t] = sca + term.absorptionPerM(wavelengthNm);
  }
  return { scattering, extinction };
}

/** R(λ) for a geometry: the direct beam's transmittance, or the sky's singly scattered radiance per unit irradiance, sr⁻¹. */
function responseOf(
  air: SpectralAir,
  geometry: CaseGeometry,
  optics: AirOptics,
  wavelengthNm: number,
): number {
  const terms = air.terms.length;
  if (geometry.kind === "sun") {
    let depth = 0;
    for (let t = 0; t < terms; t += 1) {
      depth += (optics.extinction[t] ?? 0) * (geometry.columnsM[t] ?? 0);
    }
    return Math.exp(-depth);
  }
  let radiance = 0;
  const phase = air.terms.map((term) => term.phasePerSr(geometry.cosTheta, wavelengthNm));
  for (let i = 0; i < geometry.points; i += 1) {
    let depth = 0;
    let source = 0;
    for (let t = 0; t < terms; t += 1) {
      depth += (optics.extinction[t] ?? 0) * (geometry.columnsM[t + terms * i] ?? 0);
      source +=
        (optics.scattering[t] ?? 0) * (phase[t] ?? 0) * (geometry.weightsM[t + terms * i] ?? 0);
    }
    radiance += source * Math.exp(-depth);
  }
  return radiance;
}

/**
 * R(λ) of one view through one atmosphere, as the fit computes it: the direct beam's
 * transmittance, or the sky's singly scattered radiance per unit irradiance normal to the beam,
 * sr⁻¹ (the star's irradiance at the top, with no ground and no multiple scattering).
 */
export function viewResponse(air: SpectralAir, view: CaseView, wavelengthNm: number): number {
  return responseOf(air, geometryOf(air, view), opticsAt(air, wavelengthNm), wavelengthNm);
}

/**
 * A well-mixed gas on its column as a spectral atmosphere: one Rayleigh term, n_s σ_mix(λ) at
 * relative density 1 on the column's density, with ρ_mix(λ) in its phase (R08.T3.c's
 * `molecularMixture`), over a sphere of the column's R_ref up to its top.
 *
 * @param scale - A factor on the gas's coefficients, 1 for the column's own: a case family sets
 *   another gas's optical depth equal to a reference's with it.
 * @throws RangeError as `molecularMixture`.
 */
export function molecularAir(
  name: string,
  column: AtmosphereColumn,
  fractions: GasFractions,
  scale: number,
): SpectralAir {
  const mixture = molecularMixture(fractions);
  const perM = column.surfaceNumberDensityPerM3 * scale;
  // ρ is asked for at every view's few wavelengths, and costs every species' King factor.
  const depolarisation = new Map<number, number>();
  return {
    name,
    groundRadiusM: column.referenceRadiusM,
    topHeightM: column.topHeightM,
    terms: [
      {
        name: "rayleigh",
        density: column.density,
        scatteringPerM: (nm) => perM * mixture.crossSectionM2(nm),
        absorptionPerM: () => 0,
        phasePerSr: (cosTheta, nm) => {
          const rho = depolarisation.get(nm) ?? mixture.depolarisation(nm);
          depolarisation.set(nm, rho);
          return phaseAt({ kind: "rayleigh", depolarisation: [rho, rho, rho] }, cosTheta)[0];
        },
      },
    ],
  };
}

/**
 * R05's Earth aerosol (`earth.ts`) at any wavelength: τ(550) {@link AEROSOL_OPTICAL_DEPTH_550}
 * with Ångström's law of exponent {@link AEROSOL_ANGSTROM_EXPONENT}, single-scattering albedo
 * {@link AEROSOL_SINGLE_SCATTERING_ALBEDO} and the Cornette–Shanks phase of
 * {@link AEROSOL_ASYMMETRY} at every wavelength, on an exponential of
 * {@link AEROSOL_SCALE_HEIGHT_M}: the smooth term beside the molecular one in the Earth R08.T6.d
 * rebuilds.
 */
export const EARTH_AEROSOL_TERM: SpectralAirTerm = {
  name: "aerosol",
  density: { kind: "exponential", scaleHeightM: AEROSOL_SCALE_HEIGHT_M },
  scatteringPerM: (nm) => AEROSOL_SINGLE_SCATTERING_ALBEDO * earthAerosolExtinctionPerM(nm),
  absorptionPerM: (nm) => (1 - AEROSOL_SINGLE_SCATTERING_ALBEDO) * earthAerosolExtinctionPerM(nm),
  phasePerSr: (cosTheta) =>
    phaseAt({ kind: "cornette-shanks", asymmetry: AEROSOL_ASYMMETRY }, cosTheta)[0],
};

/** R05's aerosol extinction at the ground, m⁻¹: τ(λ) ÷ H, as `earth.ts` evaluates it per channel. */
function earthAerosolExtinctionPerM(wavelengthNm: number): number {
  return (
    (AEROSOL_OPTICAL_DEPTH_550 * (wavelengthNm / 550) ** -AEROSOL_ANGSTROM_EXPONENT) /
    AEROSOL_SCALE_HEIGHT_M
  );
}

/**
 * Dry air by volume, as fractions: N₂ 78.084, O₂ 20.946 and Ar 0.934 percent (Bodhaine et al.
 * 1999, J. Atmos. Oceanic Technol. 16, 1854, eq. 23) and Peck and Reeder's 330 ppm of CO₂
 * (Bodhaine §1), over their sum of 99.997: `rayleigh.test.ts`'s dry air, which gives Earth's 4.85,
 * 11.5 and 28.7 × 10⁻⁶ m⁻¹ through R08.T3.c's molecular term.
 */
export const EARTH_DRY_AIR: GasFractions = (() => {
  const percent = [
    ["N2", 78.084],
    ["O2", 20.946],
    ["Ar", 0.934],
    ["CO2", 0.033],
  ] as const;
  const total = percent.reduce((sum, [, p]) => sum + p, 0);
  return percent.map(([species, p]) => ({ species, moleFraction: p / total }));
})();

/**
 * WGS 84's level spheroid (NIMA TR8350.2, Third Edition, Amendment 1, Table 3.1), as R08.T3.d's
 * tests take it.
 */
const WGS84: LevelSpheroid = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257_223_563),
  gmM3S2: 3.986_004_418e14,
  angularVelocityRadS: 7.292_115e-5,
};

/**
 * Earth's column for the fit: the U.S. Standard Atmosphere 1976's sea level, 101,325 Pa and
 * 288.15 K, isothermal (R08.T3.a's seam), with dry air's 28.9644 g mol⁻¹ (USSA 1976, Table 3's
 * M₀), at WGS 84's g_ref = √(γ_e γ_p) (R08.T3.d) on R05's table sphere, (2a + c) ÷ 3.
 */
export const EARTH_FIT_COLUMN: AtmosphereColumn = hydrostaticColumn({
  surfacePa: 101_325,
  temperature: { kind: "isothermal", temperatureK: 288.15 },
  meanMolarMassGPerMol: 28.964_4,
  referenceGravityMS2: referenceGravity(WGS84),
  referenceRadiusM: (2 * WGS84.equatorialRadiusM + WGS84.polarRadiusM) / 3,
});

/** Earth for the fit: dry air on {@link EARTH_FIT_COLUMN} and {@link EARTH_AEROSOL_TERM}. */
export function earthFitAir(): SpectralAir {
  const air = molecularAir("Earth", EARTH_FIT_COLUMN, EARTH_DRY_AIR, 1);
  return { ...air, terms: [...air.terms, EARTH_AEROSOL_TERM] };
}

/**
 * Mars's air by volume, as fractions: CO₂ 95.1, N₂ 2.59, Ar 1.94, O₂ 0.16 and CO 0.06 percent
 * (NASA's Mars Fact Sheet, D. R. Williams, NSSDCA, last updated 19 May 2025), over their sum of
 * 99.85.
 */
export const MARS_AIR: GasFractions = (() => {
  const percent = [
    ["CO2", 95.1],
    ["N2", 2.59],
    ["Ar", 1.94],
    ["O2", 0.16],
    ["CO", 0.06],
  ] as const;
  const total = percent.reduce((sum, [, p]) => sum + p, 0);
  return percent.map(([species, p]) => ({ species, moleFraction: p / total }));
})();

/**
 * Mars's column for the fit, from NASA's Mars Fact Sheet (last updated 19 May 2025): 6.36 mbar at
 * the mean radius; the average ~214 K held isothermal (R08.T3.a's seam); the mean molecular weight
 * 43.49 g mol⁻¹, which {@link MARS_AIR}'s own mixture gives to 0.01%; g_ref the geometric mean of
 * the sheet's equatorial and polar surface accelerations, √(3.69 × 3.73) = 3.71 m s⁻², as
 * {@link EARTH_FIT_COLUMN} takes √(γ_e γ_p); on the volumetric mean radius, 3,389.5 km.
 */
export const MARS_FIT_COLUMN: AtmosphereColumn = hydrostaticColumn({
  surfacePa: 636,
  temperature: { kind: "isothermal", temperatureK: 214 },
  meanMolarMassGPerMol: 43.49,
  referenceGravityMS2: Math.sqrt(3.69 * 3.73),
  referenceRadiusM: 3_389_500,
});

/**
 * Mars's dust for the fit: R08.T5.b's `mars_dust` index file in a gamma distribution of r_eff
 * 1.5 µm and v_eff 0.3, the mode `science-r08-nonspherical.md` §2.1 takes for Mars (within Lemmon
 * et al. 2015's 1.5–1.65 µm and 0.2–0.5, §4.2).
 *
 * @remarks
 * The file's indices are Wolff et al. 2009's CRISM retrievals from 440 nm; below that they are
 * interpolated towards the compilation's 321 nm row (`science-r08-nonspherical.md` §3.2).
 */
export const MARS_DUST_MODE: AerosolMode = {
  material: "mars_dust",
  sizes: { kind: "gamma", effectiveRadiusUm: 1.5, effectiveVariance: 0.3 },
};

/**
 * Mars's dust optical depth for the fit, vertical, at 550 nm: 0.5.
 *
 * @remarks
 * That is τ(880) ≈ 0.54 for {@link MARS_DUST_MODE}, whose extinction at 880 nm is 7.5% above its
 * 550 nm value (sphere Mie of the source's 800 and 900 nm rows, the science check of 2026-10-10;
 * Lemmon et al. 2015, Icarus 251, 96, §4.2, find such dust 7–11% less opaque at 440 nm than at
 * 880 nm). It lies inside the range the rovers' Pancams recorded at 880 nm outside dust storms
 * over five Mars years: below about 0.3 (Spirit) and 0.5 (Opportunity) in the clear season, and
 * about 1 in the dusty season's background (their §4.1).
 */
export const MARS_DUST_OPTICAL_DEPTH_550 = 0.5;

/** The spacing of {@link DUST_GRID_NM}, nm. */
const DUST_GRID_STEP_NM = 20;

/**
 * The wavelengths at which the fit evaluates the dust's Mie optics, nm: every
 * {@link DUST_GRID_STEP_NM} over the bake range. Its extinction and albedo are linear between
 * them; a Mie sum over the size distribution costs about 47 ms a wavelength, and the 20 nm grid
 * keeps the fit's case family to about a second.
 */
const DUST_GRID_NM: Float64Array = Float64Array.from(
  { length: (BAKE_RANGE_NM[1] - BAKE_RANGE_NM[0]) / DUST_GRID_STEP_NM + 1 },
  (_, i) => BAKE_RANGE_NM[0] + DUST_GRID_STEP_NM * i,
);

/** Linear interpolation of values on {@link DUST_GRID_NM}, held at its ends. */
function onDustGrid(values: Float64Array, wavelengthNm: number): number {
  const x = Math.min(
    Math.max((wavelengthNm - BAKE_RANGE_NM[0]) / DUST_GRID_STEP_NM, 0),
    DUST_GRID_NM.length - 1,
  );
  const k = Math.min(Math.floor(x), DUST_GRID_NM.length - 2);
  const t = x - k;
  return (values[k] ?? Number.NaN) * (1 - t) + (values[k + 1] ?? Number.NaN) * t;
}

/**
 * The Henyey–Greenstein phase, sr⁻¹: (1 − g²) ÷ (4π (1 + g² − 2g cos θ)^(3/2)) (Henyey and
 * Greenstein 1941, ApJ 93, 70).
 */
function henyeyGreenstein(asymmetry: number, cosTheta: number): number {
  const g = asymmetry;
  return (1 - g * g) / (4 * Math.PI * (1 + g * g - 2 * g * cosTheta) ** 1.5);
}

/**
 * Mars for the fit: {@link MARS_AIR} on {@link MARS_FIT_COLUMN}, with {@link MARS_DUST_MODE} mixed
 * with the gas at {@link MARS_DUST_OPTICAL_DEPTH_550}.
 *
 * @remarks
 * Mars dust is a `nonSphericalMineral`, so R08.T5.b's sphere Mie gives its cross-sections, and its
 * albedo and asymmetry only as the spheres' values (Design note 6). Its phase here is the
 * Henyey–Greenstein of that asymmetry at each wavelength, the interim R08.T5.b names for
 * R08.T5.c's `aerosolTerm` until R08.T5.c's TAMUdust2020 tables land; the fit is blessed again
 * then. The dust is well mixed, its density the gas's: the slow-settling limit of Conrath's
 * profile (B. J. Conrath, "Thermal structure of the Martian atmosphere during the dissipation of
 * the dust storm of 1971", Icarus 24 (1975) 36–46), and what Spirit's sky shows, dust "well mixed
 * with the atmosphere in the bottom 10-20 km" (Lemmon et al. 2015, conclusion 5).
 */
export function marsFitAir(): SpectralAir {
  const air = molecularAir("Mars", MARS_FIT_COLUMN, MARS_AIR, 1);
  const optics = Array.from(DUST_GRID_NM, (nm) => modeOptics(MARS_DUST_MODE, nm, NO_ANGLES));
  const extinction = Float64Array.from(optics, (o) => o.extinctionCrossSectionM2);
  const albedo = Float64Array.from(optics, (o) => o.singleScatteringAlbedo);
  const asymmetry = Float64Array.from(optics, (o) => o.asymmetry);
  const at550 = onDustGrid(extinction, 550);
  const columnM = columnLengthM(MARS_FIT_COLUMN.density, MARS_FIT_COLUMN.topHeightM);
  const extinctionPerM = (nm: number): number =>
    (MARS_DUST_OPTICAL_DEPTH_550 / columnM) * (onDustGrid(extinction, nm) / at550);
  const dust: SpectralAirTerm = {
    name: "dust",
    density: MARS_FIT_COLUMN.density,
    scatteringPerM: (nm) => onDustGrid(albedo, nm) * extinctionPerM(nm),
    absorptionPerM: (nm) => (1 - onDustGrid(albedo, nm)) * extinctionPerM(nm),
    phasePerSr: (cosTheta, nm) => henyeyGreenstein(onDustGrid(asymmetry, nm), cosTheta),
  };
  return { ...air, terms: [...air.terms, dust] };
}

/** No scattering angles: Mie then sums the efficiencies alone. */
const NO_ANGLES = new Float64Array(0);

/** Whether a formula names a single atom: one element symbol and no count, as "Fe" or "H". */
function isAtom(formula: string): boolean {
  return /^[A-Z][a-z]?$/u.test(formula);
}

/**
 * The gases the fit holds the triple over besides Earth and Mars, by formula: each of R08.T3.b's
 * measured {@link GASES}, and each of R08.T3.c's estimated rows ({@link ESTIMATED_RAYLEIGH}) that
 * is a molecule; a species added to either joins it.
 *
 * @remarks
 * SiO, TiO, VO and FeH are hot vapours rather than bulk gases, but stay: their estimates are a
 * static polarisability, a plain λ⁻⁴, and change nothing (0.0072 at the fitted triple).
 *
 * The estimated atoms (H, O, Na, K, Fe, Mg, Si, Ca, Ti) are left out, on plausibility: none is an
 * atmosphere's bulk gas at an Earth-like Rayleigh depth. They are the dissociation and vapour
 * products of hot envelopes and lava worlds, beside the bulk gas, and Na's, K's, Ca's and Ti's
 * visible optics are their resonance lines (R08.T4.c). For all but two the omission changes
 * nothing: each alone at dry air's τ(550) gives 0.0075 (H) or 0.0072 (O, Na, K, Si, Ca, Ti) at the
 * fitted triple, under the objective. Mg's and Fe's one oscillators, at 285.3 and 248.4 nm, steepen
 * σ to λ^−5.6 and λ^−5.1 over 450–650 nm, and would give 0.020 and 0.014: out of the family's
 * range, and recorded (R08's Risks, T4.a).
 */
export const CHANNEL_FIT_GASES: ReadonlyArray<string> = [
  ...GASES,
  ...ESTIMATED_RAYLEIGH.map((row) => row.species).filter((species) => !isAtom(species)),
];

/**
 * The fit's atmospheres: {@link earthFitAir}, {@link marsFitAir}, and each of
 * {@link CHANNEL_FIT_GASES} alone on {@link EARTH_FIT_COLUMN}, its coefficients scaled to dry
 * air's Rayleigh optical depth at 550 nm, so that the gases differ in their spectral shape alone.
 *
 * @remarks
 * A mixture's σ(λ) is its species' sum weighted by their fractions, so its shape lies among
 * theirs. A deeper column is not in the family: the error grows with the slant optical depth
 * (Risks, R08.T4.a's record), and the thick regime is the spectral bakes' (Design notes 5 and 9).
 */
export function channelFitAirs(): SpectralAir[] {
  const reference = molecularMixture(EARTH_DRY_AIR).crossSectionM2(550);
  return [
    earthFitAir(),
    marsFitAir(),
    ...CHANNEL_FIT_GASES.map((species) => {
      const alone: GasFractions = [{ species, moleFraction: 1 }];
      const scale = reference / molecularMixture(alone).crossSectionM2(550);
      return molecularAir(species, EARTH_FIT_COLUMN, alone, scale);
    }),
  ];
}

/** The sun zenith angles of the fit's views, degrees: noon to a low sun 5° up. */
export const FIT_SUN_ZENITHS_DEG: ReadonlyArray<number> = [0, 45, 60, 70, 75, 80, 85];

/**
 * The fit's sky directions, as elevation and azimuth from the star's, degrees: the zenith, the
 * sky opposite the star at 30°, and the low sky towards, across from and away from it at 10°.
 */
export const FIT_SKY_DIRECTIONS_DEG: ReadonlyArray<readonly [number, number]> = [
  [90, 0],
  [30, 180],
  [10, 0],
  [10, 90],
  [10, 180],
];

/**
 * Every case of a family: each star through each atmosphere, seen as its direct beam and as the
 * sky in each of {@link FIT_SKY_DIRECTIONS_DEG}, at each of {@link FIT_SUN_ZENITHS_DEG}.
 */
export function channelFitCases(
  suns: ReadonlyArray<CaseSun>,
  airs: ReadonlyArray<SpectralAir>,
): ChannelCase[] {
  const cases: ChannelCase[] = [];
  for (const air of airs) {
    for (const sun of suns) {
      for (const sunZenithDeg of FIT_SUN_ZENITHS_DEG) {
        const at = `${air.name}, ${sun.name}, sun at ${sunZenithDeg}°`;
        cases.push({ name: `${at}: the sun`, sun, air, view: { kind: "sun", sunZenithDeg } });
        for (const [elevationDeg, azimuthFromSunDeg] of FIT_SKY_DIRECTIONS_DEG) {
          cases.push({
            name: `${at}: the sky at ${elevationDeg}°, ${azimuthFromSunDeg}° from it`,
            sun,
            air,
            view: { kind: "sky", sunZenithDeg, elevationDeg, azimuthFromSunDeg },
          });
        }
      }
    }
  }
  return cases;
}

/** One way of looking through one atmosphere, made ready: its geometry, and R on the pipeline's rows. */
interface PreparedView {
  readonly air: SpectralAir;
  readonly geometry: CaseGeometry;
  readonly spectral: Float64Array;
}

/** A case made ready: its view, the star's own colour, and the light's spectral colour. */
interface PreparedCase {
  readonly name: string;
  readonly view: number;
  readonly starColour: Rgb;
  readonly spectral: Rgb;
  /** The spectral colour's (u′, v′). */
  readonly spectralUv: readonly [number, number];
  /**
   * The three-channel colour's XYZ per unit R in each channel: the star's colour in that channel
   * times the column of {@link REC709_TO_XYZ}, red's X, Y, Z first, so that the search's worst case
   * is a sum and allocates nothing.
   */
  readonly drawnBasis: readonly [
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
    number,
  ];
}

/** A case's {@link PreparedCase.drawnBasis}. */
function drawnBasisOf(
  star: Rgb,
): readonly [number, number, number, number, number, number, number, number, number] {
  const [x, y, z] = REC709_TO_XYZ;
  return [
    x[0] * star[0],
    y[0] * star[0],
    z[0] * star[0],
    x[1] * star[1],
    y[1] * star[1],
    z[1] * star[1],
    x[2] * star[2],
    y[2] * star[2],
    z[2] * star[2],
  ];
}

/** A case family made ready for {@link channelObjective}. */
export interface PreparedFamily {
  readonly views: ReadonlyArray<PreparedView>;
  readonly cases: ReadonlyArray<PreparedCase>;
}

/**
 * Makes a case family ready: each atmosphere's optics on the pipeline's 1 nm rows, each view's
 * geometry and spectral response, which every star shares, and each case's spectral colour.
 */
export function prepareFamily(cases: ReadonlyArray<ChannelCase>): PreparedFamily {
  const opticsByAir = new Map<SpectralAir, AirOptics[]>();
  const viewIndex = new Map<SpectralAir, Map<string, number>>();
  const views: PreparedView[] = [];
  const weightsBySun = new Map<BakeSpectrum, SunWeights>();
  // A geometry depends on the shell and the terms' densities alone, so atmospheres that differ
  // only in their optics (each gas on one column) share it.
  const densityIds = new Map<DensityProfile, number>();
  const geometries = new Map<string, CaseGeometry>();
  const geometryKey = (air: SpectralAir, view: CaseView): string => {
    const ids = air.terms.map((term) => {
      const id = densityIds.get(term.density) ?? densityIds.size;
      densityIds.set(term.density, id);
      return id;
    });
    return JSON.stringify([air.groundRadiusM, air.topHeightM, ids, view]);
  };
  const prepared = cases.map((c): PreparedCase => {
    const optics =
      opticsByAir.get(c.air) ?? Array.from(PIPELINE_WAVELENGTHS_NM, (nm) => opticsAt(c.air, nm));
    opticsByAir.set(c.air, optics);
    const byView = viewIndex.get(c.air) ?? new Map<string, number>();
    viewIndex.set(c.air, byView);
    const key = JSON.stringify(c.view);
    let view = byView.get(key);
    if (view === undefined) {
      const shared = geometryKey(c.air, c.view);
      const geometry = geometries.get(shared) ?? geometryOf(c.air, c.view);
      geometries.set(shared, geometry);
      const spectral = PIPELINE_WAVELENGTHS_NM.map((nm, i) => {
        const at = optics[i];
        return at === undefined ? Number.NaN : responseOf(c.air, geometry, at, nm);
      });
      view = views.length;
      views.push({ air: c.air, geometry, spectral });
      byView.set(key, view);
    }
    const weights = weightsBySun.get(c.sun.spectrum) ?? sunWeights(c.sun.spectrum);
    weightsBySun.set(c.sun.spectrum, weights);
    const spectralColour = weightedRgb(weights, views[view]?.spectral ?? new Float64Array(0));
    return {
      name: c.name,
      view,
      starColour: weights.rgb,
      spectral: spectralColour,
      spectralUv: uvOfRgb(spectralColour),
      drawnBasis: drawnBasisOf(weights.rgb),
    };
  });
  return { views, cases: prepared };
}

/** Each view's R at one wavelength, with each atmosphere's optics evaluated there once. */
function viewResponses(family: PreparedFamily, wavelengthNm: number): Float64Array {
  const optics = new Map<SpectralAir, AirOptics>();
  return Float64Array.from(family.views, (view) => {
    const at = optics.get(view.air) ?? opticsAt(view.air, wavelengthNm);
    optics.set(view.air, at);
    return responseOf(view.air, view.geometry, at, wavelengthNm);
  });
}

/** One case's error under a triple: the three-channel colour's from the spectral one. */
export interface CaseError {
  readonly name: string;
  /** Δu′v′ between the two. */
  readonly deltaUv: number;
  /** The three-channel colour's luminance over the spectral's. */
  readonly luminanceRatio: number;
}

/** Every case's error, from each view's R at the red, green and blue wavelengths. */
function errorsOf(
  family: PreparedFamily,
  red: Float64Array,
  green: Float64Array,
  blue: Float64Array,
): CaseError[] {
  return family.cases.map((c) => {
    const drawn: Rgb = [
      c.starColour[0] * (red[c.view] ?? Number.NaN),
      c.starColour[1] * (green[c.view] ?? Number.NaN),
      c.starColour[2] * (blue[c.view] ?? Number.NaN),
    ];
    return {
      name: c.name,
      deltaUv: deltaUv(drawn, c.spectral),
      luminanceRatio: luminanceOfRgb(drawn) / luminanceOfRgb(c.spectral),
    };
  });
}

/** The largest Δu′v′ over the cases, a NaN counting as infinite: {@link errorsOf}'s, unallocated. */
function worstDeltaUv(
  family: PreparedFamily,
  red: Float64Array,
  green: Float64Array,
  blue: Float64Array,
): number {
  let worst = 0;
  for (const c of family.cases) {
    const r = red[c.view] ?? Number.NaN;
    const g = green[c.view] ?? Number.NaN;
    const b = blue[c.view] ?? Number.NaN;
    const k = c.drawnBasis;
    const x = r * k[0] + g * k[3] + b * k[6];
    const y = r * k[1] + g * k[4] + b * k[7];
    const z = r * k[2] + g * k[5] + b * k[8];
    const denominator = x + 15 * y + 3 * z;
    const d = Math.hypot(
      (4 * x) / denominator - c.spectralUv[0],
      (9 * y) / denominator - c.spectralUv[1],
    );
    worst = Math.max(worst, Number.isNaN(d) ? Number.POSITIVE_INFINITY : d);
  }
  return worst;
}

/** Every case's error under a triple of channel wavelengths, nm (red, green, blue). */
export function caseErrors(family: PreparedFamily, wavelengthsNm: Rgb): CaseError[] {
  return errorsOf(
    family,
    viewResponses(family, wavelengthsNm[0]),
    viewResponses(family, wavelengthsNm[1]),
    viewResponses(family, wavelengthsNm[2]),
  );
}

/** The fit's objective under a triple: the largest Δu′v′ over the family. */
export function channelObjective(family: PreparedFamily, wavelengthsNm: Rgb): number {
  return worstDeltaUv(
    family,
    viewResponses(family, wavelengthsNm[0]),
    viewResponses(family, wavelengthsNm[1]),
    viewResponses(family, wavelengthsNm[2]),
  );
}

/** How the fit searches. */
export interface ChannelSearch {
  /** The triple it starts from, nm. */
  readonly startNm: Rgb;
  /** Its first step, nm. */
  readonly firstStepNm: number;
  /** The step below which it stops, nm. */
  readonly lastStepNm: number;
}

/** Where the fit starts: Design note 5's research triple, (620, 540, 445) nm. */
export const CHANNEL_FIT_START_NM: Rgb = [620, 540, 445];

/**
 * The fit's search: from {@link CHANNEL_FIT_START_NM} in steps of 8 nm, halved down to 0.125 nm,
 * a tenth of the matching functions' 1 nm rows.
 */
export const CHANNEL_FIT_SEARCH: ChannelSearch = {
  startNm: CHANNEL_FIT_START_NM,
  firstStepNm: 8,
  lastStepNm: 0.125,
};

/** What the fit found. */
export interface ChannelFit {
  readonly wavelengthsNm: Rgb;
  /** The largest Δu′v′ over the family at them. */
  readonly objective: number;
}

/**
 * A triple at a local minimum of the family's largest Δu′v′, from a start: a pattern search (Hooke
 * and Jeeves 1961, J. ACM 8, 212; Kolda, Lewis and Torczon 2003, SIAM Rev. 45, 385, §3) that polls
 * the 26 neighbours of the {−1, 0, 1}³ lattice at the current step, moves to the best that
 * improves, and halves the step when none does.
 *
 * @remarks
 * A minimax objective has kinks where its largest cases cross, at which no move along one axis
 * improves while a diagonal one does, so the poll takes every lattice direction and not only the
 * six of a compass. Even so, a pattern search need not reach a stationary point of a nonsmooth
 * objective (Kolda, Lewis and Torczon 2003, §6.1), and the family's minimum is a shallow valley
 * along which all three wavelengths rise together: from (620, 540, 445) the search stops 0.24%
 * above a lower point other starts reach, with red fixed to about ±6 nm (R08's Risks, T4.a). The
 * result is the search's from its start, which `channels.json` records with the start. The
 * search is deterministic, ties keeping the earlier direction, and every point
 * it visits lies on the start's lattice of the steps' powers of two, so the triple is exact in
 * binary. The triple keeps red above green above blue, within the bake range. Each channel's
 * response depends on its own wavelength alone, so the views' responses are kept by wavelength and a
 * poll computes at most six new ones.
 *
 * @throws RangeError unless 0 < `lastStepNm` ≤ `firstStepNm`, both finite, and the start keeps red
 *   above green above blue within {@link BAKE_RANGE_NM}.
 */
export function fitChannels(family: PreparedFamily, search: ChannelSearch): ChannelFit {
  const [low, high] = BAKE_RANGE_NM;
  const valid = (t: Rgb): boolean => t[0] > t[1] && t[1] > t[2] && t[2] >= low && t[0] <= high;
  const { firstStepNm, lastStepNm, startNm } = search;
  if (
    !(Number.isFinite(firstStepNm) && lastStepNm > 0 && lastStepNm <= firstStepNm) ||
    !valid(startNm)
  ) {
    throw new RangeError(
      `a channel search needs 0 < last step ≤ first step and an ordered start in ${low}–${high} nm, got steps ${firstStepNm} and ${lastStepNm} nm from [${startNm.join(", ")}] nm`,
    );
  }
  const directions: Array<readonly [number, number, number]> = [];
  for (const a of [-1, 0, 1]) {
    for (const b of [-1, 0, 1]) {
      for (const c of [-1, 0, 1]) {
        if (a !== 0 || b !== 0 || c !== 0) {
          directions.push([a, b, c]);
        }
      }
    }
  }
  const responses = new Map<number, Float64Array>();
  const responsesAt = (nm: number): Float64Array => {
    const known = responses.get(nm) ?? viewResponses(family, nm);
    responses.set(nm, known);
    return known;
  };
  const objective = (t: Rgb): number =>
    worstDeltaUv(family, responsesAt(t[0]), responsesAt(t[1]), responsesAt(t[2]));
  let best: Rgb = startNm;
  let bestValue = objective(best);
  let step = firstStepNm;
  while (step >= lastStepNm) {
    let moved: Rgb | undefined;
    let movedValue = bestValue;
    for (const [a, b, c] of directions) {
      const trial: Rgb = [best[0] + a * step, best[1] + b * step, best[2] + c * step];
      if (valid(trial)) {
        const value = objective(trial);
        if (value < movedValue) {
          moved = trial;
          movedValue = value;
        }
      }
    }
    if (moved === undefined) {
      step /= 2;
    } else {
      best = moved;
      bestValue = movedValue;
    }
  }
  return { wavelengthsNm: best, objective: bestValue };
}
