/**
 * Earth's reference atmosphere (plan R05, Design note 16, R05.T12.a): Rayleigh, aerosol and ozone
 * terms at Earth's measured values, each cited, and Hillaire 2020's reference medium beside it for
 * comparison against his published images.
 */

import type { Rgb } from "../photometry/toneCurve";
import type { AtmosphereMedium, MediumTerm } from "./medium";

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

const AEROSOL_EXTINCTION_PER_M: Rgb = [
  aerosolExtinctionPerM(680),
  aerosolExtinctionPerM(550),
  aerosolExtinctionPerM(440),
];

const scaled = (v: Rgb, k: number): Rgb => [v[0] * k, v[1] * k, v[2] * k];

/** Ozone's tent, zero below 10 km and above 40 km, 1 at 25 km (Bruneton 2017's demo). */
const OZONE_TERM: MediumTerm = {
  name: "ozone",
  density: { kind: "tent", bottomM: 10_000, peakM: 25_000, topM: 40_000 },
  scattering: [0, 0, 0],
  absorption: OZONE_ABSORPTION_PER_M,
  phase: { kind: "none" },
};

/**
 * Earth's reference atmosphere: dry-air Rayleigh scattering, a continental aerosol and the ozone
 * layer, at Earth's measured values (Design note 16).
 */
export const EARTH_REFERENCE: AtmosphereMedium = {
  name: "earth",
  topHeightM: EARTH_TOP_HEIGHT_M,
  groundAlbedo: [EARTH_GROUND_ALBEDO, EARTH_GROUND_ALBEDO, EARTH_GROUND_ALBEDO],
  terms: [
    {
      name: "rayleigh",
      density: { kind: "exponential", scaleHeightM: RAYLEIGH_SCALE_HEIGHT_M },
      scattering: RAYLEIGH_SCATTERING_PER_M,
      absorption: [0, 0, 0],
      phase: { kind: "rayleigh" },
    },
    {
      name: "aerosol",
      density: { kind: "exponential", scaleHeightM: AEROSOL_SCALE_HEIGHT_M },
      scattering: scaled(AEROSOL_EXTINCTION_PER_M, AEROSOL_SINGLE_SCATTERING_ALBEDO),
      absorption: scaled(AEROSOL_EXTINCTION_PER_M, 1 - AEROSOL_SINGLE_SCATTERING_ALBEDO),
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
      phase: { kind: "rayleigh" },
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
