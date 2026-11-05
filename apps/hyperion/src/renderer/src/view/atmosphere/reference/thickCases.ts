/**
 * The reference tracer's cloudy and hazy cases (plan R08, R08.T12.b.2; Design notes 10 and 16;
 * `decision-r08-t12b-scope.md`, ruling 1): `venus-92bar`, `venus-58bar`, `titan-class` and
 * `giant-deck`, each a medium on a sphere from the client's own optics, for `hyperion-fit
 * atmosphere-reference` to trace and R08.T13–T15 to hold the CPU twin to.
 *
 * @remarks
 * Their decks' and hazes' Mie and mean-field optics run for about a minute, so the cases are built
 * on first need ({@link thickReferenceCases}), in a test or a worker, never on the render thread
 * (`reference/cases.ts`'s cloud-free cases are cheap). Like those, they are R08.T10's fixtures
 * (ruling 3), whose media `ATMOSPHERE TEST`'s thick bodies are to read, once a worker builds them
 * for it (not yet: R08's records, T12.b.2). This module imports nothing from the tests.
 *
 * - `venus-92bar` and `venus-58bar`: a Venus-class air, the real 92 bar and plan 14's 58
 *   (Design note 16), under P14.T24.g's case-1 deck of four sulphuric-acid modes, mode 2
 *   `acid_uv_absorber` (`science-r08-unowned-absorbers.md`; R08.T5.e fits its absorber on
 *   `venus-92bar`), with its SO₂, on a sphere of Venus's radius.
 * - `titan-class`: a Titan-class N₂–CH₄ air with its methane and a tholin haze, the thick bake's
 *   Titan-class fixture's medium.
 * - `giant-deck`: a Saturn-class air, `saturn-oblate`'s, with its methane, its H₂–He and H₂–H₂
 *   pairs and a thick NH₃ ice deck over a black ground, on a sphere of Saturn's mean radius.
 *
 * Every case traces at the fitted channels under the reference Sun's per-channel illuminance at
 * 1 au, as the cloud-free cases do, at a seed of its own, and writes its terms to 9 significant
 * figures (`ReferenceCase.significantFigures`), so that Venus's four tabulated phases stay under the
 * added-file hook. Their geometries are Design note 10's sphere set, its limb at the medium's own
 * scale height at the ground (ruling 7), and their aggregates its four suns and, for the Venus-class
 * sanity range, the Pioneer Venus large probe's own; Titan's range reads its 60° sun's.
 *
 * Left out, stated (no file would hold them): the line absorbers' lines. Venus's CO₂ and the
 * giant's H₂ have committed k-distribution tables, which only the optics worker loads, and a curve
 * with their pressure rows (30 rows on Venus) holds about 22,000 values, past the hook beside the
 * deck's tables. In Venus's channels CO₂'s lines take a vertical depth of about 7 × 10⁻³ (red), and
 * their effect lies in the deep views, which the traces cannot hold to their σ yet.
 */

import type { Rgb } from "../../photometry/toneCurve";
import {
  type AbsorberCurve,
  type AbsorberLayer,
  absorberCurve,
  absorberSpecs,
  absorberTerm,
} from "../absorbers";
import {
  type AerosolModeSpec,
  type AerosolParticles,
  type AerosolTerm,
  aerosolTerm,
  aerosolTermOptics,
  type AerosolTermOptics,
} from "../aerosol";
import {
  type AtmosphereColumn,
  dryAdiabatExponent,
  gasProperties,
  hydrostaticColumn,
  meanMolarMassGPerMol,
} from "../column";
import { R06_SOLAR_BAKE_SPECTRUM } from "../earth";
import { type AtmosphereMedium, columnLengthM, type MediumTerm } from "../medium";
import { tableRadiusM } from "../oblate";
import type { GasFractions } from "../rayleigh";
import { molecularTerm } from "../rayleigh";
import type { BodyCover } from "../thick/regime";
import { absorberSourceOf, type SpectralSource } from "../thick/spectralColumn";
import {
  type ReferenceCase,
  SATURN_OBLATE_AIR,
  SATURN_OBLATE_COLUMN,
  SATURN_OBLATE_SPHEROID,
  sphereCase,
} from "./cases";
import type { ReferenceAggregate } from "./geometries";
import { pow } from "../../../lib/pinnedMath";

/** The significant figures the thick cases' files write their terms to (the module's remarks). */
export const THICK_CASE_SIGNIFICANT_FIGURES = 9;

/** One term of a thick case with its source, and its source's mean cosine where it is a table. */
interface SourcedTerm {
  readonly term: MediumTerm;
  readonly source: SpectralSource;
  readonly asymmetry?: Rgb;
}

/** The molecular term of `air` on `column`, and its source. */
function molecular(column: AtmosphereColumn, air: GasFractions): SourcedTerm {
  return {
    term: molecularTerm(column, air).term,
    source: {
      kind: "molecular",
      fractions: air,
      numberDensityPerM3: column.surfaceNumberDensityPerM3,
    },
  };
}

/**
 * The absorber terms `absorberSpecs` draws of `layers` on `column`, with the reference Sun's curves,
 * the order kept.
 */
function absorbers(
  layers: ReadonlyArray<AbsorberLayer>,
  column: AtmosphereColumn,
): { readonly terms: ReadonlyArray<SourcedTerm>; readonly curves: ReadonlyArray<AbsorberCurve> } {
  const { specs } = absorberSpecs(layers, column);
  const terms = specs.map((spec) => {
    const term = absorberTerm(spec, column);
    return { term, source: absorberSourceOf(term) };
  });
  return { terms, curves: specs.map((spec) => absorberCurve(spec, R06_SOLAR_BAKE_SPECTRUM)) };
}

/** A mode's term on `column` from its optics, its source and its mean cosine per channel. */
function mode(
  spec: AerosolModeSpec,
  column: AtmosphereColumn,
  optics: AerosolTermOptics,
): SourcedTerm {
  const built: AerosolTerm = aerosolTerm(spec, column, optics);
  return {
    term: built.term,
    source: {
      kind: "aerosol",
      particles: spec.particles,
      extinction550PerM: built.extinction550PerM,
    },
    asymmetry: [
      built.channels[0].asymmetry,
      built.channels[1].asymmetry,
      built.channels[2].asymmetry,
    ],
  };
}

/** A column's vertical number column, molecules m⁻². */
function moleculesPerM2(column: AtmosphereColumn): number {
  return column.surfaceNumberDensityPerM3 * columnLengthM(column.density, column.topHeightM);
}

/** A column's datum pressure, Pa. */
function datumPa(column: AtmosphereColumn): number {
  return column.pressuresPa[0] ?? Number.NaN;
}

/**
 * A thick case on a sphere of `radiusM`: its terms in their order, its curves, its tables' source
 * means, the thick cases' figures, and `extra` aggregates beside Design note 10's.
 */
function thickCase(
  name: string,
  medium: Omit<AtmosphereMedium, "terms">,
  terms: ReadonlyArray<SourcedTerm>,
  curves: ReadonlyArray<AbsorberCurve>,
  radiusM: number,
  extra: ReadonlyArray<ReferenceAggregate> = [],
): ReferenceCase {
  const full: AtmosphereMedium = { ...medium, terms: terms.map((t) => t.term) };
  const phaseAsymmetry = Object.fromEntries(
    terms.flatMap(({ term, asymmetry }) =>
      asymmetry === undefined ? [] : [[term.name, asymmetry]],
    ),
  );
  const base = sphereCase(
    name,
    full,
    terms.map((t) => t.source),
    radiusM,
    curves,
    phaseAsymmetry,
  );
  return {
    ...base,
    aggregates: [...base.aggregates, ...extra],
    significantFigures: THICK_CASE_SIGNIFICANT_FIGURES,
  };
}

// --- Venus -----------------------------------------------------------------------------------

/** Venus's volumetric mean radius, m: 6,051.8 km (NASA's Venus Fact Sheet, D. R. Williams, NSSDCA). */
export const VENUS_RADIUS_M = 6_051_800;

/** Venus's surface gravity, m s⁻²: 8.87 (NASA's Venus Fact Sheet), its column's g_ref. */
export const VENUS_GRAVITY_MS2 = 8.87;

/** A Venus-class air by volume: 96.5% CO₂ and 3.5% N₂ (NASA's Venus Fact Sheet). */
export const VENUS_AIR = [
  { species: "CO2", moleFraction: 0.965 },
  { species: "N2", moleFraction: 0.035 },
] as const satisfies GasFractions;

/** Venus's mean surface temperature, K: 737 (NASA's Venus Fact Sheet). */
export const VENUS_SURFACE_TEMPERATURE_K = 737;

/** Venus's black-body temperature, K: 226.6 (NASA's Venus Fact Sheet), its T_eq. */
export const VENUS_BLACK_BODY_TEMPERATURE_K = 226.6;

/**
 * α, the dry adiabat's scale for a dry or CO₂ air: 0.88, Design note 3's refit on Venus's measured
 * state (Robinson and Catling 2012's 0.8 took γ 1.3).
 */
export const VENUS_ADIABAT_SCALE = 0.88;

/** The Venus-class cases' surface pressures, Pa: the real 92 bar and plan 14's 58 (Design note 16). */
export const VENUS_SURFACE_PRESSURES_PA = { "venus-92bar": 92e5, "venus-58bar": 58e5 } as const;

/** The Venus-class ground's grey Lambertian albedo: 0.15, a hand value (`venusClassMedium`'s). */
export const VENUS_GROUND_ALBEDO = 0.15;

/**
 * A Venus-class column at `surfacePa`: Design note 3's profile, T = max(T_ad, T_skin), the adiabat
 * from {@link VENUS_SURFACE_TEMPERATURE_K} at β = α R ÷ c_p ({@link VENUS_ADIABAT_SCALE} of the air's
 * dry-adiabat exponent, a constant c_p, as `column.ts` takes it), the skin 2^(−1/4) T_eq
 * ({@link VENUS_BLACK_BODY_TEMPERATURE_K}), on Venus's radius at {@link VENUS_GRAVITY_MS2}.
 */
export function venusColumn(surfacePa: number): AtmosphereColumn {
  const mixture = VENUS_AIR.map(({ species, moleFraction }) => ({
    properties: gasProperties(species),
    moleFraction,
  }));
  return hydrostaticColumn({
    surfacePa,
    temperature: {
      kind: "radiativeConvective",
      surfaceK: VENUS_SURFACE_TEMPERATURE_K,
      surfacePa,
      beta: VENUS_ADIABAT_SCALE * dryAdiabatExponent(mixture),
      skinK: pow(2, -0.25) * VENUS_BLACK_BODY_TEMPERATURE_K,
    },
    meanMolarMassGPerMol: meanMolarMassGPerMol(mixture),
    referenceGravityMS2: VENUS_GRAVITY_MS2,
    referenceRadiusM: VENUS_RADIUS_M,
  });
}

/**
 * P14.T24.g's case-1 deck on Venus: its base and top, Pa, VIRA's 48 and 70 km (A. Seiff et al.,
 * Adv. Space Res. 5(11) (1985) 3–58; its rows read through Basilevsky and Head 2003's table, Rep.
 * Prog. Phys. 66, 1699, by the science check): 1.38 bar, P14.T24.g's, hydrostatic between the
 * 1.979 and 1.066 bar of the 45 and 50 km rows (1.375 bar), and 0.0369 bar, the 70 km row, where
 * P14.T24.g's "0.037, from memory" rounds it; and its vertical optical depth at 550 nm, 34.5, the
 * figure P14.T24.g fixes σ_d on, from Haus et al. 2016's standard cloud model (R. Haus et al.,
 * Icarus 272 (2016) 178–205, as P14.T24.g reads it; not re-read here). Both Venus cases take it,
 * at the same pressures.
 */
export const VENUS_DECK = { basePa: 1.38e5, topPa: 3.69e3, opticalDepth550: 34.5 } as const;

/** One of the deck's modes, as P14.T24.g tabulates it. */
export interface VenusDeckMode {
  /** Its term's name. */
  readonly name: string;
  /** Its registry material: `H2SO4` (75 wt%) or `acid_uv_absorber`. */
  readonly material: string;
  /** r_eff, µm, and v_eff of its log-normal distribution. */
  readonly effectiveRadiusUm: number;
  readonly effectiveVariance: number;
  /** The span of the deck's log-pressure fraction u = ln(p_base ÷ p) ÷ ln(p_base ÷ p_top) it fills. */
  readonly u: readonly [number, number];
  /** Its share of the deck's τ(550). */
  readonly share: number;
}

/**
 * P14.T24.g's four modes (`science-r08-unowned-absorbers.md` §1.4; the shares Mie at 550 nm over
 * Haus et al. 2016's columns), each well mixed across its span: 1, `H2SO4`, r_eff 0.49 µm, v_eff
 * 0.21, u 0–1, 0.15 of τ; 2, `acid_uv_absorber`, 1.17 µm, 0.064, u 0.55–1, 0.18; 2′, `H2SO4`,
 * 1.56 µm, 0.045, u 0–0.5, 0.27; 3, `H2SO4`, 4.27 µm, 0.064, u 0–0.33, 0.39. The shares sum to 0.99
 * as tabled, and each mode takes its share of 34.5, so the deck's τ(550) is 34.155.
 *
 * @remarks
 * r_eff and v_eff are derived, r_eff = r_g e^(2.5 ln²σ_g) and v_eff = e^(ln²σ_g) − 1 (J. E. Hansen
 * and L. D. Travis, Space Sci. Rev. 16 (1974) 527, eq. 2.60, r_g the number median), from Pollack
 * et al. 1993's r_g of 0.30, 1.00, 1.40 and 3.65 µm (J. B. Pollack et al., Icarus 103 (1993) 1, as
 * the plan reads it, not re-read here) at ln σ_g rounded to 0.44, 0.25, 0.21 and 0.25, as
 * P14.T49.e keeps them; Pollack's own σ_g would move r_eff by at most 0.6% and v_eff by up to 5%.
 */
export const VENUS_DECK_MODES: ReadonlyArray<VenusDeckMode> = [
  {
    name: "aerosol:H2SO4",
    material: "H2SO4",
    effectiveRadiusUm: 0.49,
    effectiveVariance: 0.21,
    u: [0, 1],
    share: 0.15,
  },
  {
    name: "aerosol:acid_uv_absorber",
    material: "acid_uv_absorber",
    effectiveRadiusUm: 1.17,
    effectiveVariance: 0.064,
    u: [0.55, 1],
    share: 0.18,
  },
  {
    name: "aerosol:H2SO4:2",
    material: "H2SO4",
    effectiveRadiusUm: 1.56,
    effectiveVariance: 0.045,
    u: [0, 0.5],
    share: 0.27,
  },
  {
    name: "aerosol:H2SO4:3",
    material: "H2SO4",
    effectiveRadiusUm: 4.27,
    effectiveVariance: 0.064,
    u: [0, 0.33],
    share: 0.39,
  },
];

/** A deck mode's particles: liquid spheres of its material on its log-normal distribution. */
export function venusModeParticles(deckMode: VenusDeckMode): AerosolParticles {
  return {
    kind: "distribution",
    mode: {
      material: deckMode.material,
      form: { phase: "liquid" },
      shape: "sphere",
      sizes: {
        kind: "logNormal",
        effectiveRadiusUm: deckMode.effectiveRadiusUm,
        effectiveVariance: deckMode.effectiveVariance,
      },
    },
  };
}

/**
 * A Venus-class medium's deck as a body's cover gives it (R08.T15.b.2): its τ(550), each mode's
 * share of {@link VENUS_DECK}'s 34.5 (34.155), under a cloud fraction of 1, over its modes' span on
 * the ground, each from the level below where its tabulated density first rises from 0 to the one
 * above where it last falls to it, so that `classifyRegime` splits the medium at the deck's top over
 * the ground (lean, 2026-11-01, as `giant-deck`'s): the cover the Venus-class gate reads the cases
 * under (`vectorGate.test.ts`), and `ATMOSPHERE TEST` draws `TEST VENUS` and `TEST VENUS 5.80 MPa`
 * under.
 *
 * @throws Error for a medium without the deck's modes as tabulated terms.
 */
export function venusDeckCover(medium: AtmosphereMedium): BodyCover {
  let baseAltitudeM = Number.POSITIVE_INFINITY;
  let topAltitudeM = 0;
  for (const deckMode of VENUS_DECK_MODES) {
    const term = medium.terms.find((t) => t.name === deckMode.name);
    if (term?.density.kind !== "tabulated") {
      throw new Error(`${medium.name} has no tabulated deck mode ${deckMode.name}`);
    }
    const { altitudesM, relative } = term.density;
    const first = relative.findIndex((d) => d > 0);
    const last = relative.findLastIndex((d) => d > 0);
    baseAltitudeM = Math.min(baseAltitudeM, altitudesM[Math.max(first - 1, 0)] ?? Number.NaN);
    topAltitudeM = Math.max(
      topAltitudeM,
      altitudesM[Math.min(last + 1, altitudesM.length - 1)] ?? Number.NaN,
    );
  }
  const opticalDepth550 = VENUS_DECK_MODES.reduce(
    (sum, deckMode) => sum + deckMode.share * VENUS_DECK.opticalDepth550,
    0,
  );
  return {
    cloudFraction: 1,
    decks: [{ opticalDepth550, baseAltitudeM, topAltitudeM }],
    envelope: false,
  };
}

/** The pressure at a deck fraction u, Pa: p_base (p_top ÷ p_base)^u. */
export function venusDeckPressurePa(u: number): number {
  return VENUS_DECK.basePa * pow(VENUS_DECK.topPa / VENUS_DECK.basePa, u);
}

/** A deck mode's spec: its span's pressures, its share of the deck's τ(550), well mixed. */
export function venusModeSpec(deckMode: VenusDeckMode): AerosolModeSpec {
  return {
    name: deckMode.name,
    particles: venusModeParticles(deckMode),
    opticalDepth550: deckMode.share * VENUS_DECK.opticalDepth550,
    basePa: venusDeckPressurePa(deckMode.u[0]),
    topPa: venusDeckPressurePa(deckMode.u[1]),
    mixingExponent: 0,
  };
}

/**
 * Venus's SO₂ by volume below its clouds: 130 ppm, the figure P14.T24.g holds a measured Venus to
 * (E. Marcq et al., JGR 113 (2008) E00B07: 130 ± 50 ppmv at 30–40 km), from the ground to the deck's
 * top, as P14.T24.g publishes its layer.
 */
export const VENUS_SO2_MOLE_FRACTION = 1.3e-4;

/** The four modes' optics, computed once for both Venus cases. */
function venusModeOptics(): ReadonlyArray<AerosolTermOptics> {
  return VENUS_DECK_MODES.map((m) => aerosolTermOptics(venusModeParticles(m)));
}

/** A Venus-class case at `name`'s pressure, its modes' `optics` given. */
function venusCase(
  name: keyof typeof VENUS_SURFACE_PRESSURES_PA,
  optics: ReadonlyArray<AerosolTermOptics>,
): ReferenceCase {
  const column = venusColumn(VENUS_SURFACE_PRESSURES_PA[name]);
  const so2 = absorbers(
    [
      {
        species: "SO2",
        columnPerM2: VENUS_SO2_MOLE_FRACTION * moleculesPerM2(column),
        basePa: datumPa(column),
        topPa: VENUS_DECK.topPa,
      },
    ],
    column,
  );
  const deck = VENUS_DECK_MODES.map((m, k) => {
    const modeOptics = optics[k];
    if (modeOptics === undefined) {
      throw new Error(`the deck's mode ${m.name} has its optics`);
    }
    return mode(venusModeSpec(m), column, modeOptics);
  });
  return thickCase(
    name,
    { name, topHeightM: column.topHeightM, groundAlbedo: grey(VENUS_GROUND_ALBEDO) },
    [molecular(column, VENUS_AIR), ...so2.terms, ...deck],
    so2.curves,
    VENUS_RADIUS_M,
    [VENUS_PROBE_AGGREGATE],
  );
}

/**
 * The sun of the Pioneer Venus large probe's solar flux radiometer (LSFR) as it descended: 65.7°
 * from the zenith at its entry site, 4.4°N 304.0°E (NASA's Pioneer Venus probes table, D. R.
 * Williams, NSSDCA; M. G. Tomasko and L. R. Doose, NASA-CR-169874, 1982: "at the large probe entry
 * site where the solar zenith angle was 65.7°"; the probe's results M. G. Tomasko et al., JGR 85
 * (1980) 8167–8186), the Venus-class cases' sanity range's aggregate.
 */
export const VENUS_PROBE_SUN_ZENITH_DEG = 65.7;

/** The Venus-class cases' probe aggregate, at {@link VENUS_PROBE_SUN_ZENITH_DEG}. */
const VENUS_PROBE_AGGREGATE: ReferenceAggregate = {
  name: "flux-lsfr",
  latitudeDeg: 0,
  sun: { zenithDeg: VENUS_PROBE_SUN_ZENITH_DEG, azimuthDeg: 0 },
};

// --- Titan -----------------------------------------------------------------------------------

/** Titan's mean radius, m: 2,574.73 km (H. A. Zebker et al., Science 324 (2009) 921). */
export const TITAN_RADIUS_M = 2_574_730;

/** Titan's GM, m³ s⁻²: 8,978.1356 km³ s⁻² (R. A. Jacobson et al., AJ 132 (2006) 2520, Table 3). */
export const TITAN_GM_M3S2 = 8.978_135_6e12;

/**
 * Titan's surface: 1,467 hPa and 93.65 K (M. Fulchignoni et al., Nature 438 (2005) 785, Huygens'
 * HASI).
 */
export const TITAN_SURFACE = { pressurePa: 146_700, temperatureK: 93.65 } as const;

/**
 * A Titan-class air by volume: methane at its near-surface 5.65% (H. B. Niemann et al., JGR 115
 * (2010) E12006, Huygens' GCMS), held through the column, nitrogen the rest.
 */
export const TITAN_AIR = [
  { species: "N2", moleFraction: 0.9435 },
  { species: "CH4", moleFraction: 0.0565 },
] as const satisfies GasFractions;

/** The Titan-class ground's grey Lambertian albedo: 0.3, a hand value (the thick bake's fixture's). */
export const TITAN_GROUND_ALBEDO = 0.3;

/** The Titan-class column: isothermal at the surface's temperature, at GM ÷ R² on Titan's radius. */
export function titanColumn(): AtmosphereColumn {
  const mixture = TITAN_AIR.map(({ species, moleFraction }) => ({
    properties: gasProperties(species),
    moleFraction,
  }));
  return hydrostaticColumn({
    surfacePa: TITAN_SURFACE.pressurePa,
    temperature: { kind: "isothermal", temperatureK: TITAN_SURFACE.temperatureK },
    meanMolarMassGPerMol: meanMolarMassGPerMol(mixture),
    referenceGravityMS2: TITAN_GM_M3S2 / TITAN_RADIUS_M ** 2,
    referenceRadiusM: TITAN_RADIUS_M,
  });
}

/**
 * The Titan-class haze: tholin (`materials/tholin.json`) as aggregates of 3,000 monomers of
 * 0.05 µm at D_f 2, after Tomasko et al. 2008's model of Titan's haze (M. G. Tomasko et al.,
 * Planet. Space Sci. 56 (2008) 669–707), from the ground to 1 Pa, of vertical optical depth 8.1 at
 * 550 nm, the column Huygens' DISR measured (Tomasko et al. 2008's Fig. 47 as A. Bazzon, H. M.
 * Schmid and E. Buenzli, A&A 572 (2014) A6, "HST observations of the limb polarization of Titan",
 * eqs. A.8–A.10, fit it: 3.940 above 80 km, 2.793 from 30 to 80 km and 1.372 below): the thick
 * bake's Titan-class fixture's.
 *
 * @remarks
 * Its mixing ratio rises aloft as (p ÷ p_base)^−0.83, the exponent fitted to DISR's layers on
 * {@link titanColumn} (plan R08, R08.T17; research-leans-36 §1): τ(550) 3.93 above 80 km, 2.29
 * from 30 to 80 km and 1.88 below. A single power law cannot match all three layers; it matches
 * the upper one, which sets the disc's colour, its haze absorbing the blue before N₂'s Rayleigh
 * scattering beneath sends it back. Mixed with the gas from the ground (exponent 0), 2.6% of the
 * haze's τ lay above 80 km, and the disc read grey: I/F red over green 0.79 and blue over green
 * 0.87 against Titan's geometric albedo's 1.40 and 0.55 at 645, 544 and 434 nm (0.284, 0.202 and
 * 0.111: E. Karkoschka, Icarus 133 (1998) 134, as A. Payne et al. 2025, arXiv:2508.13368, compile it).
 */
export const TITAN_HAZE: AerosolModeSpec = {
  name: "aerosol:tholin",
  particles: {
    kind: "aggregate",
    mode: {
      material: "tholin",
      form: { phase: "solid" },
      monomerRadiusUm: 0.05,
      monomerCount: 3_000,
      fractalDimension: 2,
    },
  },
  opticalDepth550: 8.1,
  basePa: TITAN_SURFACE.pressurePa,
  topPa: 1,
  mixingExponent: -0.83,
};

/** `titan-class`: the N₂–CH₄ air, its methane and its haze on {@link titanColumn}. */
function titanCase(): ReferenceCase {
  const column = titanColumn();
  const methane = absorbers(
    TITAN_AIR.map(({ species, moleFraction }) => ({
      species,
      columnPerM2: moleFraction * moleculesPerM2(column),
      basePa: datumPa(column),
      topPa: 0,
    })),
    column,
  );
  return thickCase(
    "titan-class",
    {
      name: "titan-class",
      topHeightM: column.topHeightM,
      groundAlbedo: grey(TITAN_GROUND_ALBEDO),
    },
    [
      molecular(column, TITAN_AIR),
      ...methane.terms,
      mode(TITAN_HAZE, column, aerosolTermOptics(TITAN_HAZE.particles)),
    ],
    methane.curves,
    TITAN_RADIUS_M,
  );
}

// --- The giant's deck ------------------------------------------------------------------------

/**
 * The giant's NH₃ ice deck: NH₃ ice crystals (`materials/ammonia-ice.json`) on a gamma distribution
 * of r_eff 2 µm and v_eff 0.1, a hand mode, well mixed from the ground at 1 bar to 0.3 bar, of
 * vertical optical depth 20 at 550 nm, past `CLOUD_DECK_SPLIT_OPTICAL_DEPTH`'s 10, so that R08.T15
 * splits the medium at it.
 */
export const GIANT_DECK: AerosolModeSpec = {
  name: "aerosol:NH3",
  particles: {
    kind: "distribution",
    mode: {
      material: "NH3",
      form: { phase: "solid" },
      sizes: { kind: "gamma", effectiveRadiusUm: 2, effectiveVariance: 0.1 },
    },
  },
  opticalDepth550: 20,
  basePa: 1e5,
  topPa: 3e4,
  mixingExponent: 0,
};

/** The giant's ground under its deck: black, so that what lies below the deck adds nothing. */
export const GIANT_GROUND_ALBEDO = 0;

/** The giant's sphere: R05's `tableRadiusM` of Saturn's figure at 1 bar, (2a + c) ÷ 3. */
export const GIANT_RADIUS_M = tableRadiusM(SATURN_OBLATE_SPHEROID);

/**
 * `giant-deck`: `saturn-oblate`'s Saturn-class air on its column (`SATURN_OBLATE_COLUMN`, isothermal
 * at 134 K from 1 bar), its gases' absorbers and pairs (CH₄'s bands, H₂–He and H₂–H₂), and
 * {@link GIANT_DECK}, on {@link GIANT_RADIUS_M}.
 */
function giantCase(): ReferenceCase {
  const column = SATURN_OBLATE_COLUMN;
  const gases = absorbers(
    SATURN_OBLATE_AIR.map(({ species, moleFraction }) => ({
      species,
      columnPerM2: moleFraction * moleculesPerM2(column),
      basePa: datumPa(column),
      topPa: 0,
    })),
    column,
  );
  return thickCase(
    "giant-deck",
    { name: "giant-deck", topHeightM: column.topHeightM, groundAlbedo: grey(GIANT_GROUND_ALBEDO) },
    [
      molecular(column, SATURN_OBLATE_AIR),
      ...gases.terms,
      mode(GIANT_DECK, column, aerosolTermOptics(GIANT_DECK.particles)),
    ],
    gases.curves,
    GIANT_RADIUS_M,
  );
}

// --- The cases -------------------------------------------------------------------------------

/** A grey albedo. */
function grey(albedo: number): Rgb {
  return [albedo, albedo, albedo];
}

/** The thick cases, by name. */
export interface ThickReferenceCases {
  readonly "venus-92bar": ReferenceCase;
  readonly "venus-58bar": ReferenceCase;
  readonly "titan-class": ReferenceCase;
  readonly "giant-deck": ReferenceCase;
}

/** The cover each thick case's body is drawn under, by case name; absent, a clear sky. */
export type ThickCaseCovers = Readonly<Partial<Record<keyof ThickReferenceCases, BodyCover>>>;

/**
 * The covers `ATMOSPHERE TEST` draws the thick cases' bodies under (R08.T15.b.2): each Venus-class
 * case's deck ({@link venusDeckCover}), so that the router splits it at the deck's top and routes it
 * through its deck as its gate reads it; the others a clear sky, as before.
 */
export function thickCaseCovers(cases: ThickReferenceCases): ThickCaseCovers {
  return {
    "venus-92bar": venusDeckCover(cases["venus-92bar"].medium),
    "venus-58bar": venusDeckCover(cases["venus-58bar"].medium),
  };
}

/** The names of the thick cases, in the order their files are listed. */
export const THICK_CASE_NAMES: ReadonlyArray<keyof ThickReferenceCases> = [
  "venus-92bar",
  "venus-58bar",
  "titan-class",
  "giant-deck",
];

/** The cases once built, for the module's life. */
let built: ThickReferenceCases | undefined;

/** `giant-deck` once built, for the module's life: the one {@link thickReferenceCases} holds. */
let builtGiantDeck: ReferenceCase | undefined;

/** `titan-class` once built, for the module's life: the one {@link thickReferenceCases} holds. */
let builtTitanClass: ReferenceCase | undefined;

/**
 * `giant-deck` alone (the module's remarks), built on the first call without the Venus-class
 * cases' Mie, and held: the case {@link thickReferenceCases} gives, for a reader of that case alone
 * (R08.T16.a.2's envelope range, `thick/routing.test.ts`).
 */
export function giantDeckReferenceCase(): ReferenceCase {
  builtGiantDeck ??= giantCase();
  return builtGiantDeck;
}

/**
 * `titan-class` alone (the module's remarks), built on the first call without the Venus-class
 * cases' Mie, its haze's aggregate optics some 20 s (pod, 16 vCPU), and held: the case
 * {@link thickReferenceCases} gives, for a reader of that case alone (R08.T14.d.4's Titan-class
 * gate, `vectorGate.test.ts` and `thick/routing.test.ts`).
 */
export function titanClassReferenceCase(): ReferenceCase {
  builtTitanClass ??= titanCase();
  return builtTitanClass;
}

/**
 * The thick cases (the module's remarks), built on the first call, about a minute of Mie, and held:
 * each a pure function of the committed optics, so that every caller reads one medium.
 *
 * @throws RangeError as the optics refuse their inputs: a bug, every input here being fixed.
 */
export function thickReferenceCases(): ThickReferenceCases {
  if (built === undefined) {
    const optics = venusModeOptics();
    built = {
      "venus-92bar": venusCase("venus-92bar", optics),
      "venus-58bar": venusCase("venus-58bar", optics),
      "titan-class": titanClassReferenceCase(),
      "giant-deck": giantDeckReferenceCase(),
    };
  }
  return built;
}
