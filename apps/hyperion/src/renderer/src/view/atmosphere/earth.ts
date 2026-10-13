/**
 * Earth's reference atmospheres: R08.T6.d's Earth rebuilt (plan R08, R08.T6.d), from R08.T3's
 * column and molecular term, R05's aerosol, R08.T4.b's ozone curve of growth and R08.T4.a's fitted
 * channels, under R06's Sun; R05's constants (plan R05, Design note 16, R05.T12.a), Rayleigh,
 * aerosol and ozone terms at Earth's measured values, each cited, kept as a second medium for
 * comparison; and Hillaire 2020's reference medium beside them for comparison against his
 * published images.
 *
 * @remarks
 * The rebuilt Earth's ozone is an absorber term (`absorbers.ts`'s `absorberTerm`), whose
 * absorption is each sun's curve of growth: {@link EARTH_REFERENCE_CURVES} are the reference Sun's,
 * which every table of {@link EARTH_REFERENCE} is built with. This module therefore takes the
 * absorbers' cross-sections into whatever bundle reads it, and builds the curve when it loads: the
 * descent spike's chunk, which `main.tsx` loads only for a `--descent-spike` launch, and the
 * smoke's, until R08.T10.a's optics worker builds each body's medium and curves off the render
 * thread.
 */

import type { HostDiscDto } from "@hyperion/protocol";

import { CHANNEL_LUMINANCE, starIlluminance } from "../lighting/illuminance";
import { frozenHostDisc, sunLikeHostDisc } from "../lighting/hostDisc";
import type { Rgb } from "../photometry/toneCurve";
import { AU_M } from "../scenes/kept";
import { type AbsorberSpec, absorberCurve, absorberSpecs, absorberTerm } from "./absorbers";
import {
  type AtmosphereColumn,
  dryAdiabatExponent,
  gasProperties,
  hydrostaticColumn,
  meanMolarMassGPerMol,
  type MixtureComponent,
} from "./column";
import {
  type AtmosphereMedium,
  CHANNEL_WAVELENGTHS_NM,
  type MediumTerm,
  type PhaseFunction,
} from "./medium";
import { type LevelSpheroid, referenceGravity } from "./oblate";
import { type GasFractions, molecularTerm } from "./rayleigh";
import { bakeSpectrumOf } from "./spectralColour";
import type { AbsorberCurveSlots } from "./tables";

/**
 * Dry air's Rayleigh scattering coefficients at sea level, m⁻¹, at 680, 550 and 440 nm:
 * 4.848, 11.487 and 28.71 × 10⁻⁶.
 *
 * @remarks
 * β = σ N with σ = 24π³ (n² − 1)² ÷ (λ⁴ N_s² (n² + 2)²) × F_K: the refractivity of standard air
 * from Peck and Reeder, J. Opt. Soc. Am. 62 (1972) 958 (15 °C, 101,325 Pa, 0.03% CO₂); the King
 * factor of air from Bates, Planet. Space Sci. 32 (1984) 785, mixed by volume as Bucholtz, Appl.
 * Opt. 34 (1995) 2765 does (both as Bodhaine et al., J. Atmos. Oceanic Technol. 16 (1999) 1854,
 * eqs. 4–6, quote them); and N = N_s = p ÷ (k_B T) = 2.547 × 10²⁵ m⁻³ at the US Standard
 * Atmosphere 1976's sea level, 288.15 K and 101,325 Pa. The 550 nm cross-section, 4.510 × 10⁻²⁷
 * cm², is Bucholtz 1995's 4.51 × 10⁻²⁷ cm² (cf. Bodhaine et al. 1999, Table 3, 4.5105 × 10⁻²⁷ cm²
 * at 360 ppm CO₂). `earth.test.ts` recomputes all three. The 5.8, 13.5, 33.1 × 10⁻⁶ that tutorials
 * copy (a pure λ⁻⁴ law, no King factor) are not used (the brainstorm's "Atmosphere").
 */
export const RAYLEIGH_SCATTERING_PER_M: Rgb = [4.848e-6, 11.487e-6, 28.71e-6];

/**
 * The Rayleigh term's scale height, m: 8,434.5, the US Standard Atmosphere 1976's at sea level.
 *
 * @remarks
 * R*T₀ ÷ (M₀g₀), with R* = 8.31432 J mol⁻¹ K⁻¹, M₀ = 28.9644 g mol⁻¹, g₀ = 9.80665 m s⁻² and
 * T₀ = 288.15 K (US Standard Atmosphere 1976): the height at which the sea-level N_s above carries
 * the column p₀ ÷ (m̄g₀). τ_R(550 nm) is then 0.0969, against Bodhaine et al. 1999's ≈ 0.097; an
 * 8 km height (Bruneton and Neyret 2008, Hillaire 2020) leaves the column 5% short
 * (decisions-r05.md item 3). Above about 20 km the exponential overstates the density; R08's
 * tabulated profile replaces it.
 */
export const RAYLEIGH_SCALE_HEIGHT_M = 8_434.5;

/**
 * The aerosol's optical depth at 550 nm, vertical, from the ground: 0.1.
 *
 * @remarks
 * Between the median (about 0.05) and the mean (0.15–0.19) of the MODIS aerosol optical depth over
 * land at 550 nm: Remer et al., J. Geophys. Res. 113 (2008) D14S07 (Collection 5, land 0.19); Levy
 * et al., Atmos. Meas. Tech. 6 (2013) 2989, §4.6 (Collection 6). Hillaire's reference (5.3 × 10⁻³
 * at every wavelength) is 20–40 times cleaner (Design note 16).
 */
export const AEROSOL_OPTICAL_DEPTH_550 = 0.1;

/**
 * The aerosol's Ångström exponent, α in τ(λ) = τ(550 nm) × (λ ÷ 550 nm)^(−α): 1.3.
 *
 * @remarks
 * Ångström's classical continental mean (Ångström, Geogr. Ann. 11 (1929) 156), within the 1.0–2.5
 * that AERONET retrieves for urban–industrial and biomass-burning aerosol (Dubovik et al., J. Atmos.
 * Sci. 59 (2002) 590, Table 1).
 */
export const AEROSOL_ANGSTROM_EXPONENT = 1.3;

/**
 * The aerosol's single-scattering albedo, the scattered fraction of its extinction: 0.92, taken
 * constant across the three channels.
 *
 * @remarks
 * Between the ω₀ ≈ 0.90 and 0.95 continental fine-mode models of Levy, Remer and Dubovik, J.
 * Geophys. Res. 112 (2007) D13210; cf. the urban–industrial ω₀ of Dubovik et al. 2002, Table 1,
 * 0.88–0.98 over 440–670 nm.
 */
export const AEROSOL_SINGLE_SCATTERING_ALBEDO = 0.92;

/**
 * The aerosol's Cornette–Shanks parameter g: 0.584, whose mean cosine 3g(4 + g²) ÷ (5(2 + g²)) is
 * 0.650.
 *
 * @remarks
 * 0.650 is AERONET's asymmetry at 550 nm for continental fine-mode aerosol: GSFC 0.68/0.59,
 * Crete–Paris and Mexico City 0.68/0.61 at 440/670 nm, interpolated linearly in λ (Dubovik et al.,
 * J. Atmos. Sci. 59 (2002) 590, Table 1). Bruneton and Neyret 2008's 0.76 (mean cosine 0.81),
 * fitted with a far thinner aerosol, is not used (decisions-r05.md item 1);
 * {@link HILLAIRE_REFERENCE} keeps Hillaire's 0.8 for the comparison with his images.
 */
export const AEROSOL_ASYMMETRY = 0.584;

/**
 * The aerosol's scale height, m: 1.2 km.
 *
 * @remarks
 * Bruneton and Neyret 2008, §2, eq. 3 ("H_M ≃ 1.2 km"), and Hillaire 2020 (the brainstorm's "near
 * 1.2 km").
 */
export const AEROSOL_SCALE_HEIGHT_M = 1_200;

/**
 * The ozone column, Dobson units: 300, Earth's global average (WMO/UNEP, _Twenty Questions and
 * Answers About the Ozone Layer: 2018 Update_, Q3), Bruneton 2017's value.
 */
export const OZONE_COLUMN_DU = 300;

/**
 * Ozone's absorption coefficients at the layer's peak, m⁻¹, at 680, 550 and 440 nm:
 * 0.650, 1.881 and 0.085 × 10⁻⁶.
 *
 * @remarks
 * σ × n_peak, with n_peak = 300 DU × 2.687 × 10²⁰ m⁻² ÷ 15 km = 5.374 × 10¹⁸ m⁻³ (the Dobson unit
 * from the WMO/UNEP 2018 glossary; the tent profile below integrates to 15 km), and σ Serdyuchenko
 * et al., Atmos. Meas. Tech. 7 (2014) 625, at 233 K, averaged over each 10 nm bin from the
 * channel's wavelength up, as Bruneton 2017 bins them (precomputed_atmospheric_scattering,
 * `atmosphere/demo/demo.cc`, `kOzoneCrossSection`): 1.209, 3.5 and 0.1582 × 10⁻²⁵ m².
 * `earth.test.ts` recomputes them. Bins centred on the wavelengths would give +10%, −6% and −19%
 * at 680, 550 and 440 nm; the plan keeps Bruneton's and sebh's values, so that both media share
 * Hillaire's ozone (decisions-r05.md item 2).
 */
export const OZONE_ABSORPTION_PER_M: Rgb = [0.65e-6, 1.881e-6, 0.085e-6];

/**
 * The top of the atmosphere above the ground, m: 100 km, the Kármán line, above which lies about
 * 3 × 10⁻⁷ of the air (US Standard Atmosphere 1976: 3.20 × 10⁻² Pa at 100 km) and Hillaire's and
 * sebh's top.
 */
export const EARTH_TOP_HEIGHT_M = 100_000;

/**
 * The ground's albedo, Lambertian, grey: 0.1, Bruneton 2017's demo value (`kGroundAlbedo`) and the
 * ᾱ with which Bruneton and Neyret 2008 (Fig. 6) reproduce the CIE clear sky.
 */
export const EARTH_GROUND_ALBEDO = 0.1;

/** The aerosol's extinction at the ground per channel, m⁻¹: τ(λ) ÷ H with the Ångström law. */
function aerosolExtinctionPerM(wavelengthNm: number): number {
  const opticalDepth =
    AEROSOL_OPTICAL_DEPTH_550 * (wavelengthNm / 550) ** -AEROSOL_ANGSTROM_EXPONENT;
  return opticalDepth / AEROSOL_SCALE_HEIGHT_M;
}

/** R05's aerosol extinction at Bruneton's 680, 550 and 440 nm, where R05's constants stand. */
const R05_AEROSOL_EXTINCTION_PER_M: Rgb = [
  aerosolExtinctionPerM(680),
  aerosolExtinctionPerM(550),
  aerosolExtinctionPerM(440),
];

const scaled = (v: Rgb, k: number): Rgb => [v[0] * k, v[1] * k, v[2] * k];

/**
 * The phase R05's Rayleigh terms are drawn with: ρ = 0 in every channel, 3 ÷ (16π) × (1 + cos²θ),
 * the form R05's kernels give every Rayleigh term (`source.wgsl`'s `phaseOf`). Air's own ρ is
 * 0.027–0.028 over 440–680 nm by the σ-weighted mixture of Bates 1984's King factors (R08 Design
 * note 4); R08.T6.d rebuilds Earth from R08.T3.c's molecular term, which carries it.
 */
const R05_RAYLEIGH_PHASE: PhaseFunction = { kind: "rayleigh", depolarisation: [0, 0, 0] };

/** Ozone's tent, zero below 10 km and above 40 km, 1 at 25 km (Bruneton 2017's demo). */
const OZONE_TERM: MediumTerm = {
  name: "ozone",
  density: { kind: "tent", bottomM: 10_000, peakM: 25_000, topM: 40_000 },
  scattering: [0, 0, 0],
  absorption: OZONE_ABSORPTION_PER_M,
  phase: { kind: "none" },
};

/**
 * R05's Earth (plan R05, Design note 16): dry-air Rayleigh scattering on an exponential of the US
 * Standard Atmosphere's 8,434.5 m, a continental aerosol and Bruneton's ozone tent, at Earth's
 * measured values evaluated at Bruneton's 680, 550 and 440 nm. Kept as a second medium for
 * comparison beside {@link HILLAIRE_REFERENCE} (plan R08, R08.T6.d), which R05's gates and
 * recorded figures were measured on; {@link EARTH_REFERENCE} is drawn.
 */
export const R05_EARTH_REFERENCE: AtmosphereMedium = {
  name: "earth (R05)",
  topHeightM: EARTH_TOP_HEIGHT_M,
  groundAlbedo: [EARTH_GROUND_ALBEDO, EARTH_GROUND_ALBEDO, EARTH_GROUND_ALBEDO],
  terms: [
    {
      name: "rayleigh",
      density: { kind: "exponential", scaleHeightM: RAYLEIGH_SCALE_HEIGHT_M },
      scattering: RAYLEIGH_SCATTERING_PER_M,
      absorption: [0, 0, 0],
      phase: R05_RAYLEIGH_PHASE,
    },
    {
      name: "aerosol",
      density: { kind: "exponential", scaleHeightM: AEROSOL_SCALE_HEIGHT_M },
      scattering: scaled(R05_AEROSOL_EXTINCTION_PER_M, AEROSOL_SINGLE_SCATTERING_ALBEDO),
      absorption: scaled(R05_AEROSOL_EXTINCTION_PER_M, 1 - AEROSOL_SINGLE_SCATTERING_ALBEDO),
      phase: { kind: "cornette-shanks", asymmetry: AEROSOL_ASYMMETRY },
    },
    OZONE_TERM,
  ],
};

/**
 * Hillaire 2020's reference medium, for comparison with his published images (Design note 16):
 * sebh's `SetupEarthAtmosphere` (UnrealEngineSkyAtmosphere, `Application/SkyAtmosphereCommon.cpp`,
 * MIT), converted from km⁻¹ to m⁻¹.
 *
 * @remarks
 * Rayleigh 5.802, 13.558 and 33.1 × 10⁻⁶ m⁻¹ with an 8 km scale height; an aerosol of scattering
 * 3.996 × 10⁻⁶ and extinction 4.44 × 10⁻⁶ m⁻¹ at every wavelength, 1.2 km, g = 0.8; Bruneton's
 * ozone; a black ground. Hillaire's planet is a 6,360 km sphere with a 100 km atmosphere; here the
 * medium sits on whatever figure the tables are given.
 */
export const HILLAIRE_REFERENCE: AtmosphereMedium = {
  name: "hillaire-2020",
  topHeightM: 100_000,
  groundAlbedo: [0, 0, 0],
  terms: [
    {
      name: "rayleigh",
      density: { kind: "exponential", scaleHeightM: 8_000 },
      scattering: [5.802e-6, 13.558e-6, 33.1e-6],
      absorption: [0, 0, 0],
      phase: R05_RAYLEIGH_PHASE,
    },
    {
      name: "aerosol",
      density: { kind: "exponential", scaleHeightM: 1_200 },
      scattering: [3.996e-6, 3.996e-6, 3.996e-6],
      absorption: [0.444e-6, 0.444e-6, 0.444e-6],
      phase: { kind: "cornette-shanks", asymmetry: 0.8 },
    },
    OZONE_TERM,
  ],
};

// --- Earth rebuilt (plan R08, R08.T6.d) -----------------------------------------------------------

/**
 * Dry air by volume, as fractions: N₂ 78.084, O₂ 20.946 and Ar 0.934 percent (Bodhaine et al.
 * 1999, J. Atmos. Oceanic Technol. 16, 1854, eq. 23) and Peck and Reeder's 330 ppm of CO₂
 * (Bodhaine §1), over their sum of 99.997: `rayleigh.test.ts`'s dry air, which gives Earth's 4.85,
 * 11.5 and 28.7 × 10⁻⁶ m⁻¹ at 680, 550 and 440 nm through R08.T3.c's molecular term, and
 * `channels.ts`'s fit air.
 */
export const EARTH_DRY_AIR_COMPONENTS = [
  ["N2", 78.084],
  ["O2", 20.946],
  ["Ar", 0.934],
  ["CO2", 0.033],
] as const;

const DRY_AIR_PERCENT = EARTH_DRY_AIR_COMPONENTS.reduce((sum, [, percent]) => sum + percent, 0);

/** {@link EARTH_DRY_AIR_COMPONENTS} as the molecular term takes it. */
export const EARTH_DRY_AIR: GasFractions = EARTH_DRY_AIR_COMPONENTS.map(([species, percent]) => ({
  species,
  moleFraction: percent / DRY_AIR_PERCENT,
}));

/** {@link EARTH_DRY_AIR_COMPONENTS} as the column takes it: each gas's molar mass and c_p. */
const DRY_AIR_MIXTURE: ReadonlyArray<MixtureComponent> = EARTH_DRY_AIR_COMPONENTS.map(
  ([gas, percent]) => ({ properties: gasProperties(gas), moleFraction: percent / DRY_AIR_PERCENT }),
);

/**
 * WGS 84's level spheroid (NIMA TR8350.2, Third Edition, Amendment 1, Table 3.1): a = 6,378,137 m,
 * 1 ÷ f = 298.257 223 563, GM = 3.986 004 418 × 10¹⁴ m³ s⁻² and ω = 7.292 115 × 10⁻⁵ rad s⁻¹.
 */
export const WGS84_LEVEL_SPHEROID: LevelSpheroid = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257_223_563),
  gmM3S2: 3.986_004_418e14,
  angularVelocityRadS: 7.292_115e-5,
};

/**
 * The latent heat's flattening of Earth's lapse rate, α in β = α R ÷ c_p: 0.6, Design note 3's for a
 * world with a water ocean (Robinson and Catling 2014, Nature Geoscience 7, 12, Table 1).
 */
export const EARTH_LAPSE_FLATTENING = 0.6;

/**
 * Earth's isothermal skin, K: 214.4, 2^(−1/4) × 255 K, the τ → 0 limit of a grey Eddington
 * atmosphere at Earth's equilibrium temperature, Design note 3's Earth check (tropopause 0.179 bar
 * at 214 K against about 0.16–0.23 bar and 217 K observed; US Standard Atmosphere 1976, 0.226 bar
 * at 216.65 K). On {@link EARTH_COLUMN}, whose sea level is 288.15 K, it is reached at 0.181 bar,
 * 12.59 km.
 */
export const EARTH_SKIN_K = 214.4;

/**
 * Earth's column (plan R08, Design note 3, R08.T3.a; R08.T6.d): the US Standard Atmosphere 1976's
 * sea level, 101,325 Pa and 288.15 K, on Design note 3's scaled dry adiabat, β =
 * {@link EARTH_LAPSE_FLATTENING} × R ÷ c_p of {@link EARTH_DRY_AIR_COMPONENTS} (0.172), up to
 * {@link EARTH_SKIN_K}, with dry air's molar mass from its gases (28.96 g mol⁻¹), at WGS 84's
 * g_ref = √(γ_e γ_p) (R08.T3.d) on R05's table sphere, (2a + c) ÷ 3.
 *
 * @remarks
 * Its scale height at the ground is kT ÷ (μ m_u g_ref), 8.43 km, so its column is R05's
 * exponential's and Bodhaine et al. 1999's: τ_R(550 nm) = 0.097 through dry air (`earth.test.ts`).
 * It differs from R05's in its vertical distribution: the adiabat thins the air above the
 * ground more slowly than an exponential of the ground's scale height, and the 214 K skin
 * faster, at about 6.3 km. The skin has no ozone heating (Design note 3's stated limit), so the
 * stratosphere is about 60 K colder than the real one's top at 50 km.
 */
export const EARTH_COLUMN: AtmosphereColumn = hydrostaticColumn({
  surfacePa: 101_325,
  temperature: {
    kind: "radiativeConvective",
    surfaceK: 288.15,
    surfacePa: 101_325,
    beta: EARTH_LAPSE_FLATTENING * dryAdiabatExponent(DRY_AIR_MIXTURE),
    skinK: EARTH_SKIN_K,
  },
  meanMolarMassGPerMol: meanMolarMassGPerMol(DRY_AIR_MIXTURE),
  referenceGravityMS2: referenceGravity(WGS84_LEVEL_SPHEROID),
  referenceRadiusM:
    (2 * WGS84_LEVEL_SPHEROID.equatorialRadiusM + WGS84_LEVEL_SPHEROID.polarRadiusM) / 3,
});

/** One Dobson unit, molecules m⁻²: 2.687 × 10²⁰ (WMO/UNEP 2018, _Twenty Questions_, glossary). */
export const DOBSON_UNIT_PER_M2 = 2.687e20;

/**
 * The ozone layer's base and top, Pa: 70 and 3 hPa, 18.6–38.5 km on {@link EARTH_COLUMN}, where
 * Earth's measured ozone mostly lies (about 15–35 km; WMO/UNEP 2018, _Twenty Questions_, Q1). Its
 * column-weighted mean height is 24.0 km, beside R05's tent's 25 km (`earth.test.ts`).
 *
 * @remarks
 * A layer of constant mixing ratio (`absorbers.ts`'s `absorberSpecs`), its number density the air's
 * times a constant, so densest at its base. A ray tangent to its base crosses about √(2πR ÷ H) ≈ 80
 * of its vertical columns (Chapman's grazing incidence; 81.6 at the transmittance table's deepest
 * texel), under where the red curve of growth leaves its last node above the 10⁻⁶ floor under
 * R06's Sun, 94.4 columns: the red channel leaves Rec. 709 at about 99 columns (29,700 DU), and the
 * read depth climbs from that node to the floor at d ln τ ÷ d ln u = 15, against 4.3 for the
 * spectral curve at 95 columns. A thin slab would not keep under it: galaxy plan 14's "layer near
 * 10–30 mbar" for an oxygen-bearing world's ozone (P14.T24.c) gives rays tangent near its base
 * 95–145 columns, where the table's own 4,096-step value cannot be met
 * to `decision-r08-t6b-floor.md` §9.2's bound, nor the stored column's half-float rounding (plan
 * R08's Risks, "Deviations in T6.d, as built", for "main").
 */
export const EARTH_OZONE_LAYER_PA: readonly [number, number] = [7_000, 300];

/** Earth's ozone, {@link OZONE_COLUMN_DU} in {@link EARTH_OZONE_LAYER_PA} on its column. */
export const EARTH_OZONE: AbsorberSpec = ((): AbsorberSpec => {
  const [basePa, topPa] = EARTH_OZONE_LAYER_PA;
  const { specs } = absorberSpecs(
    [{ species: "O3", columnPerM2: OZONE_COLUMN_DU * DOBSON_UNIT_PER_M2, basePa, topPa }],
    EARTH_COLUMN,
  );
  const [ozone] = specs;
  if (specs.length !== 1 || ozone === undefined) {
    throw new Error(`Earth's ozone is one absorber profile, not ${specs.length}`);
  }
  return ozone;
})();

/** R05's aerosol at {@link CHANNEL_WAVELENGTHS_NM}, the fitted channels. */
const EARTH_AEROSOL_EXTINCTION_PER_M: Rgb = [
  aerosolExtinctionPerM(CHANNEL_WAVELENGTHS_NM[0]),
  aerosolExtinctionPerM(CHANNEL_WAVELENGTHS_NM[1]),
  aerosolExtinctionPerM(CHANNEL_WAVELENGTHS_NM[2]),
];

/**
 * Earth's reference atmosphere, rebuilt (plan R08, R08.T6.d): R08.T3.c's molecular term of
 * {@link EARTH_DRY_AIR} on {@link EARTH_COLUMN} (`rayleigh`, tabulated, with air's depolarisation
 * per channel), R05's continental aerosol, and {@link EARTH_OZONE} as an absorber term
 * (`absorber:O3`), whose absorption is each sun's curve of growth (R08.T4.b), every smooth term at
 * the fitted channels, {@link CHANNEL_WAVELENGTHS_NM}. The top and the ground's albedo are R05's.
 * Its tables are built with {@link EARTH_REFERENCE_CURVES}.
 */
export const EARTH_REFERENCE: AtmosphereMedium = {
  name: "earth",
  topHeightM: EARTH_TOP_HEIGHT_M,
  groundAlbedo: [EARTH_GROUND_ALBEDO, EARTH_GROUND_ALBEDO, EARTH_GROUND_ALBEDO],
  terms: [
    molecularTerm(EARTH_COLUMN, EARTH_DRY_AIR).term,
    {
      name: "aerosol",
      density: { kind: "exponential", scaleHeightM: AEROSOL_SCALE_HEIGHT_M },
      scattering: scaled(EARTH_AEROSOL_EXTINCTION_PER_M, AEROSOL_SINGLE_SCATTERING_ALBEDO),
      absorption: scaled(EARTH_AEROSOL_EXTINCTION_PER_M, 1 - AEROSOL_SINGLE_SCATTERING_ALBEDO),
      phase: { kind: "cornette-shanks", asymmetry: AEROSOL_ASYMMETRY },
    },
    absorberTerm(EARTH_OZONE, EARTH_COLUMN),
  ],
};

/**
 * R06's solar colour, linear Rec. 709 at unit luminance: `hyperion_sim::sky::colour::solar_colour()`,
 * the colour table (`tables/star_colour.rs`, ATLAS9, Castelli and Kurucz 2003) at 5,772 K and
 * log g 4.438 by `star_colour`'s bilinear rule, its red and green 1.0844255 and 0.98155972, and its
 * blue from the luminance row R06 builds the channels with (R07's `CHANNEL_LUMINANCE`), 0.934006,
 * as R06's `blue()` gives it.
 *
 * @remarks
 * The same spectrum through the bake bins, {@link R06_SOLAR_BAKE_SPECTRUM} under the matching
 * functions (`spectralColour.ts`'s `sunWeights`), lies 1.2 × 10⁻⁴ from it in u′v′ (`earth.test.ts`),
 * the bins' interpolation of a spectrum R06 integrates at 1 nm.
 */
export const R06_SOLAR_COLOUR_RGB: Rgb = ((): Rgb => {
  const red = 1.084_425_5;
  const green = 0.981_559_72;
  const [yr, yg, yb] = CHANNEL_LUMINANCE;
  return [red, green, (1 - yr * red - yg * green) / yb];
})();

/**
 * R06's photopic illuminance of a V = 0 star of the Sun's spectrum over 2.54 µlx: 1.036747,
 * `solar_colour().lux_per_v0()` to seven figures, which puts the Sun at 1.3346 × 10⁵ lx (133,462 lx)
 * at 1 au, E-490's own 133,318 lx to 0.11%, where R07's stand-in takes 1 (`hostDisc.ts`).
 */
export const R06_SOLAR_LUX_PER_V0 = 1.036_747;

/**
 * R06's solar bake spectrum: `solar_colour().bake_spectrum()`, the Sun's mean spectral irradiance
 * over the 15 bake bins per lux, W m⁻² nm⁻¹, as `packages/protocol/fixtures/bake_spectra.json`
 * holds it (held there by `hyperion-fit`'s `star_colour` test, and here by `earth.test.ts`).
 */
export const R06_SOLAR_BAKE_SPECTRUM = bakeSpectrumOf([
  8.960915207343157e-6, 1.2362490278026274e-5, 1.3612987280831015e-5, 1.5485020042778694e-5,
  1.4690473858970552e-5, 1.3958072723148161e-5, 1.4076616230965863e-5, 1.381707381874496e-5,
  1.3363324889932911e-5, 1.2474684459518065e-5, 1.1775456367565396e-5, 1.1371777803239652e-5,
  1.0711433533074792e-5, 1.001879082420593e-5, 9.483862860417408e-6,
]);

/**
 * The reference Sun's disc as R06's `host_discs` would send it (plan R08, R08.T6.d): R07's
 * `sunLikeHostDisc` (M_V 4.81, R☉ 6.957 × 10⁸ m, the V-band limb darkening) with R06's solar
 * colour, photopic factor and bake spectrum in place of its stand-ins. The descent spike, the smoke
 * and the tests light the reference Earth with it; generated bodies take their stars' own from
 * the wire.
 */
export const REFERENCE_SUN: HostDiscDto = frozenHostDisc(
  sunLikeHostDisc({
    colourRgb: R06_SOLAR_COLOUR_RGB,
    lux_per_v0: R06_SOLAR_LUX_PER_V0,
    bake_spectrum: [...R06_SOLAR_BAKE_SPECTRUM],
  }),
);

/**
 * The reference Sun's illuminance per channel at a distance, lx: R07's `starIlluminance` of
 * {@link REFERENCE_SUN}, what a `SunState` carries.
 *
 * @param distanceM - From the Sun's centre, m; 1 au, 149,597,870,700 m (IAU 2012 Resolution B2), by
 *   default.
 */
export function referenceSunIlluminanceLx(distanceM: number = AU_M): Rgb {
  return starIlluminance(REFERENCE_SUN, distanceM);
}

/**
 * The reference Earth's curves of growth: light slot 0 the reference Sun's, R08.T4.b's
 * `absorberCurve` of {@link EARTH_OZONE} under {@link R06_SOLAR_BAKE_SPECTRUM}, which every table
 * of {@link EARTH_REFERENCE} is built with (`HillaireAtmosphere`'s and `AtmosphereTables`'s
 * `curves`).
 */
export const EARTH_REFERENCE_CURVES: AbsorberCurveSlots = [
  [absorberCurve(EARTH_OZONE, R06_SOLAR_BAKE_SPECTRUM)],
];
