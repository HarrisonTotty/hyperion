/**
 * The reference tracer's spectral cases (plan R08, R08.T17; Verification's "Spectral error",
 * Design note 5): `earth`'s and `mars`'s media, figures and geometries traced in the 15 bake bins,
 * so that the light the fitted channels draw is held to the light of the spectrum.
 *
 * @remarks
 * Each case is its three-channel case of `cases.ts` with every term's optics taken at the centres of
 * `medium.ts`'s {@link BAKE_WAVELENGTHS_NM} from the source behind it
 * (`thick/spectralColumn.ts`'s {@link spectralOptics}), as the spectral bakes take them:
 *
 * - the molecular term at each bin's own σ_mix(λ) and ρ_mix(λ);
 * - Mars's dust at its mode's optics and √θ table at each bin, with its matrix, its `asymmetry` the
 *   mode's g;
 * - Earth's aerosol, whose source is its three channels alone, at R05's own Ångström law
 *   ({@link aerosolExtinctionPerM}), its single-scattering albedo and Cornette–Shanks g held, rather
 *   than the bake's ground rule, which holds the end channels' values beyond them (21% above the
 *   law in the last bin, at 747 nm, and 12% below it in the first, at 393 nm);
 * - Earth's ozone by its curves of growth in each bin under the reference Sun (`absorbers.ts`'s
 *   {@link absorberBinCurves}), the weights S alone, as Design note 5 has the reference's
 *   15-wavelength runs take them;
 * - the ground's albedo by the bake's ground rule ({@link binGroundAlbedo}), grey in both.
 *
 * The sun's irradiance in each bin is the reference Sun's: R06's bake spectrum times its photopic
 * illuminance at 1 au, W m⁻² nm⁻¹, so that the radiances come out in W m⁻² nm⁻¹ sr⁻¹. The
 * geometries are the three-channel case's but its twilight image, `ground-s95` (the sun 5° below
 * the horizon), whose every view `earth`'s three-channel reference refuses as too noisy at
 * 560,128 samples (R08.T12.b.1's record: it would need 6.8 × 10⁶ to 5.4 × 10⁷); the aggregates are
 * the case's. Each body is traced in two parts ({@link SpectralPart}), each a case of its own seed,
 * so that each trace stays under the four hours a pod's run is checkpointed by and each file under
 * the added-file hook.
 *
 * Each geometry's count ({@link spectralSampleCounts}) is set from the three-channel reference's
 * own standard error, which a pilot of the same paths gives as well: the count that puts the noisiest
 * channel's σ at {@link SPECTRAL_RELATIVE_SIGMA} of its radiance, 1.5% with the sun 10° up and 2% on
 * the horizon, times {@link SPECTRAL_COUNT_MARGIN} for the bins beyond the channels, in whole
 * blocks of 1,024 and at least 4,096. The targets are above Design note 10's σ ≤ T ÷ 8 (0.625% of
 * L at its 5%), so its metric refuses many of the spectral readings' comparisons: the check is read
 * in u′v′ beside each reading's own noise, and the cap's counts were projected from pod pilots at
 * about 300 CPU-h, five times the four parts' 56.
 */

import type { Rgb } from "../../photometry/toneCurve";
import { type AbsorberBinCurves, absorberBinCurves } from "../absorbers";
import {
  AEROSOL_ASYMMETRY,
  AEROSOL_SINGLE_SCATTERING_ALBEDO,
  aerosolExtinctionPerM,
  EARTH_REFERENCE_SOURCES,
  R06_SOLAR_BAKE_SPECTRUM,
} from "../earth";
import { aerosolModeOptics } from "../aerosol";
import { BAKE_WAVELENGTHS_NM, type DensityProfile, type PhaseTable } from "../medium";
import { type BakeSpectrum, luminanceOfRgb } from "../spectralColour";
import { binGroundAlbedo } from "../thick/bake";
import { type SpectralSource, spectralColumnOf, spectralOptics } from "../thick/spectralColumn";
import { EARTH_CASE, MARS_CASE, MARS_REFERENCE_SOURCES, type ReferenceCase, seedOf } from "./cases";
import type { ReferenceAggregate, ReferenceGeometry } from "./geometries";
import type { ReferenceFile } from "./references";

/** A spectral case's phase function over its wavelengths. */
export type SpectralCasePhase =
  | { readonly kind: "rayleigh"; readonly depolarisation: ReadonlyArray<number> }
  | { readonly kind: "cornette-shanks"; readonly asymmetry: number }
  | {
      readonly kind: "tabulated";
      /** One a wavelength, its channel 0 the one read (`spectralColumn.ts`'s `SpectralPhase`). */
      readonly tables: ReadonlyArray<PhaseTable>;
      /** The source's mean cosine at each wavelength, the mode's g. */
      readonly asymmetry: ReadonlyArray<number>;
    }
  | { readonly kind: "none" };

/** An absorber term's amount and its curves in each bin under the case's sun. */
export type SpectralCaseAbsorber =
  | {
      readonly kind: "column";
      readonly numberDensityPerM3: number;
      readonly curves: AbsorberBinCurves;
    }
  | {
      readonly kind: "pair";
      readonly pairDensityPerM6: number;
      readonly curves: AbsorberBinCurves;
    };

/** One term of a spectral case, at relative density 1, one entry a wavelength. */
export interface SpectralCaseTerm {
  readonly name: string;
  readonly density: DensityProfile;
  readonly scatteringPerM: ReadonlyArray<number>;
  readonly absorptionPerM: ReadonlyArray<number>;
  readonly phase: SpectralCasePhase;
  readonly absorber?: SpectralCaseAbsorber;
}

/** One spectral case of the reference tracer (the module's remarks). */
export interface SpectralReferenceCase {
  /** The case's name: its files are `<name>.case.json` and `<name>.reference.json`. */
  readonly name: string;
  /** The three-channel case whose medium, figure and geometries it traces. */
  readonly base: ReferenceCase;
  /** Which of the base's geometries it traces. */
  readonly part: SpectralPart;
  readonly seed: number;
  /** The bins' centres, nm. */
  readonly wavelengthsNm: ReadonlyArray<number>;
  /** The star's spectrum the bins' irradiance and curves are of, W m⁻² nm⁻¹ per lux. */
  readonly spectrum: BakeSpectrum;
  readonly terms: ReadonlyArray<SpectralCaseTerm>;
  /** The ground's Lambertian albedo at each wavelength. */
  readonly groundAlbedo: ReadonlyArray<number>;
  /** The sun's spectral irradiance normal to its beam at the top at each wavelength. */
  readonly sunIrradianceWPerM2Nm: ReadonlyArray<number>;
  readonly geometries: ReadonlyArray<ReferenceGeometry>;
  readonly aggregates: ReadonlyArray<ReferenceAggregate>;
}

/**
 * Which of its three-channel case's geometries a spectral case traces (the module's remarks):
 * `day` those under a sun higher than {@link SPECTRAL_LOW_SUN_ZENITH_DEG}, with every aggregate,
 * and `low` the rest but the twilight image, so that each part's trace and file stay small.
 */
export type SpectralPart = "day" | "low";

/** The sun's zenith angle from which a geometry is the `low` part's, degrees: 80°, 10° up. */
export const SPECTRAL_LOW_SUN_ZENITH_DEG = 80;

/** The image a spectral case leaves out: the sun 5° below the horizon (the module's remarks). */
export const SPECTRAL_OMITTED_IMAGE = "ground-s95";

/** The relative standard error a geometry's count is set for by day (the module's remarks). */
export const SPECTRAL_RELATIVE_SIGMA = 0.01;

/**
 * The relative standard error at a low sun, by its zenith angle, degrees: 1.5% at 80° and 2% at 90°
 * and below the horizon, where the light is dimmest and longest-pathed (the module's remarks).
 */
export const SPECTRAL_LOW_SUN_SIGMA: ReadonlyArray<
  readonly [sunZenithDeg: number, relativeSigma: number]
> = [
  [90, 0.02],
  [80, 0.015],
];

/** The factor on each count for the bins beyond the channels' wavelengths: 1.5. */
export const SPECTRAL_COUNT_MARGIN = 1.5;

/** The tracer's block of samples, which every count is a whole number of. */
export const SPECTRAL_COUNT_BLOCK = 1024;

/** The least count of a geometry or aggregate: 4,096, the committed references' `--samples`. */
export const SPECTRAL_LEAST_COUNT = 4096;

/** A geometry's relative-σ target at its sun's zenith angle (the module's remarks). */
export function spectralSigmaTarget(sunZenithDeg: number): number {
  const low = SPECTRAL_LOW_SUN_SIGMA.find(([fromZenithDeg]) => sunZenithDeg >= fromZenithDeg);
  return low === undefined ? SPECTRAL_RELATIVE_SIGMA : low[1];
}

/** A term's optics at one wavelength, at relative density 1, for a channel source's override. */
export interface SpectralTermAt {
  readonly scatteringPerM: number;
  readonly absorptionPerM: number;
  /** The same at every wavelength. */
  readonly phase: SpectralCasePhase;
}

/** A channel source's optics at any wavelength, by its term's name (the module's remarks). */
export type ChannelTermAt = (wavelengthNm: number) => SpectralTermAt;

/**
 * Earth's aerosol at a wavelength: R05's Ångström law of τ(550) 0.1 and α 1.3 over its 1.2 km,
 * ω₀ 0.92 and Cornette–Shanks g 0.584 at every wavelength (`earth.ts`).
 */
export function earthAerosolAt(wavelengthNm: number): SpectralTermAt {
  const extinction = aerosolExtinctionPerM(wavelengthNm);
  return {
    scatteringPerM: extinction * AEROSOL_SINGLE_SCATTERING_ALBEDO,
    absorptionPerM: extinction * (1 - AEROSOL_SINGLE_SCATTERING_ALBEDO),
    phase: { kind: "cornette-shanks", asymmetry: AEROSOL_ASYMMETRY },
  };
}

/** One term's optics at every wavelength from its source, `override` for a channel source. */
function spectralTerm(
  base: ReferenceCase,
  index: number,
  source: SpectralSource,
  optics: ReturnType<typeof spectralOptics>,
  wavelengthsNm: ReadonlyArray<number>,
  spectrum: BakeSpectrum,
  override: ChannelTermAt | undefined,
): SpectralCaseTerm {
  const term = base.medium.terms[index];
  if (term === undefined) {
    throw new Error(`${base.name} has no term ${index}`);
  }
  if (source.kind === "channels") {
    if (override === undefined) {
      throw new RangeError(`${base.name}'s term ${term.name} has no spectrum of its own`);
    }
    const each = wavelengthsNm.map(override);
    const [first] = each;
    if (first === undefined) {
      throw new RangeError("a spectral case has wavelengths");
    }
    return {
      name: term.name,
      density: term.density,
      scatteringPerM: each.map((t) => t.scatteringPerM),
      absorptionPerM: each.map((t) => t.absorptionPerM),
      phase: first.phase,
    };
  }
  const at = optics.map((row) => {
    const o = row[index];
    if (o === undefined) {
      throw new Error(`${base.name}'s optics lack term ${term.name}`);
    }
    return o;
  });
  let phase: SpectralCasePhase;
  switch (source.kind) {
    case "molecular":
    case "thomson":
      phase = {
        kind: "rayleigh",
        depolarisation: at.map((o) => {
          if (o.phase.kind !== "rayleigh") {
            throw new Error(`${term.name}'s phase is Rayleigh's`);
          }
          return o.phase.depolarisation;
        }),
      };
      break;
    case "aerosol":
      phase = {
        kind: "tabulated",
        tables: at.map((o) => {
          if (o.phase.kind !== "tabulated") {
            throw new Error(`${term.name}'s phase is its mode's table`);
          }
          return o.phase.table;
        }),
        asymmetry: wavelengthsNm.map((nm) => aerosolModeOptics(source.particles, nm).asymmetry),
      };
      break;
    case "absorber":
      phase = { kind: "none" };
      break;
  }
  let absorber: SpectralCaseAbsorber | undefined;
  if (term.absorber !== undefined) {
    const curves = absorberBinCurves(term.absorber.spec, spectrum);
    absorber =
      term.absorber.kind === "column"
        ? { kind: "column", numberDensityPerM3: term.absorber.numberDensityPerM3, curves }
        : { kind: "pair", pairDensityPerM6: term.absorber.pairDensityPerM6, curves };
  }
  return {
    name: term.name,
    density: term.density,
    scatteringPerM: at.map((o) => o.scatteringPerM),
    absorptionPerM: at.map((o) => o.absorptionPerM),
    phase,
    ...(absorber === undefined ? {} : { absorber }),
  };
}

/**
 * A spectral case of `base` (the module's remarks), its terms from `sources`, a channel source's
 * from `overrides` by its term's name.
 *
 * @throws RangeError for a channel source with no override, or as `spectralOptics` refuses a source.
 */
export function spectralCase(
  name: string,
  base: ReferenceCase,
  sources: ReadonlyArray<SpectralSource>,
  part: SpectralPart,
  overrides: Readonly<Record<string, ChannelTermAt>> = {},
): SpectralReferenceCase {
  const wavelengthsNm = BAKE_WAVELENGTHS_NM;
  const spectrum = R06_SOLAR_BAKE_SPECTRUM;
  const column = spectralColumnOf(base.medium, sources);
  const optics = spectralOptics(column, wavelengthsNm);
  const terms = sources.map((source, i) =>
    spectralTerm(
      base,
      i,
      source,
      optics,
      wavelengthsNm,
      spectrum,
      overrides[base.medium.terms[i]?.name ?? ""],
    ),
  );
  const illuminanceLx = luminanceOfRgb(base.sunIlluminanceLx);
  const albedo: Rgb = base.medium.groundAlbedo;
  const traced = base.geometries.filter((g) => g.image !== SPECTRAL_OMITTED_IMAGE);
  const low = (g: ReferenceGeometry): boolean => g.sun.zenithDeg >= SPECTRAL_LOW_SUN_ZENITH_DEG;
  return {
    name,
    base,
    part,
    seed: seedOf(name),
    wavelengthsNm,
    spectrum,
    terms,
    groundAlbedo: wavelengthsNm.map((nm) => binGroundAlbedo(albedo, nm)),
    sunIrradianceWPerM2Nm: spectrum.map((s) => s * illuminanceLx),
    geometries: traced.filter((g) => (part === "low") === low(g)),
    aggregates: part === "day" ? base.aggregates : [],
  };
}

/** `earth-spectral-day`: {@link EARTH_CASE}'s day, orbit and aerial images, and its aggregates. */
export const EARTH_SPECTRAL_DAY_CASE: SpectralReferenceCase = spectralCase(
  "earth-spectral-day",
  EARTH_CASE,
  EARTH_REFERENCE_SOURCES,
  "day",
  { aerosol: earthAerosolAt },
);

/** `earth-spectral-low`: {@link EARTH_CASE}'s images of a sun 10° up and on the horizon. */
export const EARTH_SPECTRAL_LOW_CASE: SpectralReferenceCase = spectralCase(
  "earth-spectral-low",
  EARTH_CASE,
  EARTH_REFERENCE_SOURCES,
  "low",
  { aerosol: earthAerosolAt },
);

/** `mars-spectral-day`: {@link MARS_CASE}'s day, orbit and aerial images, and its aggregates. */
export const MARS_SPECTRAL_DAY_CASE: SpectralReferenceCase = spectralCase(
  "mars-spectral-day",
  MARS_CASE,
  MARS_REFERENCE_SOURCES,
  "day",
);

/**
 * `mars-spectral-low`: {@link MARS_CASE}'s images of a sun 10° up and on the horizon, and its
 * aureole under the sun 5° up.
 */
export const MARS_SPECTRAL_LOW_CASE: SpectralReferenceCase = spectralCase(
  "mars-spectral-low",
  MARS_CASE,
  MARS_REFERENCE_SOURCES,
  "low",
);

/** The spectral cases, in the order their files are listed. */
export const SPECTRAL_CASES: ReadonlyArray<SpectralReferenceCase> = [
  EARTH_SPECTRAL_DAY_CASE,
  EARTH_SPECTRAL_LOW_CASE,
  MARS_SPECTRAL_DAY_CASE,
  MARS_SPECTRAL_LOW_CASE,
];

/** A count in whole blocks, at least {@link SPECTRAL_LEAST_COUNT}. */
function blocks(count: number): number {
  return Math.max(
    SPECTRAL_LEAST_COUNT,
    Math.ceil(count / SPECTRAL_COUNT_BLOCK) * SPECTRAL_COUNT_BLOCK,
  );
}

/** The largest of σ ÷ x over the channels, where x is above 0. */
function largestRelative(sigma: ReadonlyArray<number>, value: ReadonlyArray<number>): number {
  return Math.max(
    0,
    ...sigma.map((s, c) => {
      const v = value[c] ?? 0;
      return v > 0 ? s / v : 0;
    }),
  );
}

/** A spectral case's own counts (the module's remarks), by geometry and aggregate name. */
export interface SpectralCounts {
  readonly geometries: Readonly<Record<string, number>>;
  readonly aggregates: Readonly<Record<string, number>>;
}

/**
 * A spectral case's counts from its three-channel case's committed reference (the module's
 * remarks): an aggregate's for σ_F ≤ 1% of its flux, direct and diffuse at the ground and upwelling
 * at the top.
 *
 * @throws Error for a geometry or aggregate the reference does not hold.
 */
export function spectralSampleCounts(
  spectral: SpectralReferenceCase,
  baseReference: ReferenceFile,
): SpectralCounts {
  const traced = new Map(baseReference.geometries.map((g) => [g.name, g] as const));
  const geometries: Record<string, number> = {};
  for (const g of spectral.geometries) {
    const reading = traced.get(g.name);
    if (reading === undefined) {
      throw new Error(`${baseReference.case} holds no geometry ${g.name}`);
    }
    const relative = largestRelative(reading.standardError, reading.radiance);
    const target = spectralSigmaTarget(g.sun.zenithDeg);
    geometries[g.name] = blocks(SPECTRAL_COUNT_MARGIN * reading.samples * (relative / target) ** 2);
  }
  const fluxes = new Map(baseReference.aggregates.map((a) => [a.name, a] as const));
  const aggregates: Record<string, number> = {};
  for (const a of spectral.aggregates) {
    const reading = fluxes.get(a.name);
    if (reading === undefined) {
      throw new Error(`${baseReference.case} holds no aggregate ${a.name}`);
    }
    const ground = reading.diffuseGround.map((d, c) => d + (reading.directGround[c] ?? 0));
    const relative = Math.max(
      largestRelative(reading.diffuseGroundStandardError, ground),
      largestRelative(reading.upwellingTopStandardError, reading.upwellingTop),
    );
    aggregates[a.name] = blocks(
      SPECTRAL_COUNT_MARGIN * reading.samples * (relative / SPECTRAL_RELATIVE_SIGMA) ** 2,
    );
  }
  return { geometries, aggregates };
}
