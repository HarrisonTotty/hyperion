/**
 * Rayleigh scattering by each gas from its measured dispersion (plan R08, Design note 4,
 * R08.T3.b): the gas's refractivity at the reference state its source states, Lorentz–Lorenz to
 * that state's number density, and the gas's King factor.
 *
 * @remarks
 * The cross-section of one molecule is
 *
 * σ(λ) = 24π³ ÷ (λ⁴ N_ref²) × ((n² − 1) ÷ (n² + 2))² × F_K(λ),
 *
 * with n the gas's index at its formula's reference state (T_ref, p_ref), N_ref that state's number
 * density and F_K the King factor, (6 + 3ρ) ÷ (6 − 7ρ) for a depolarisation ratio ρ in natural
 * light, which corrects for the molecule's anisotropy. For a dilute gas (n² − 1) ÷ (n² + 2) is
 * proportional to N, so σ belongs to the molecule, not the state, provided N_ref is the state the
 * formula was measured at. Reading a 0 °C formula at 15 °C's density raises σ by
 * (288.15 ÷ 273.15)² − 1 = 11% (the CO₂ trap of Design note 4).
 *
 * N_ref is the real gas's, p_ref ÷ (Z k_B T_ref), with Z the compressibility factor at the state
 * ({@link Dispersion.compressibility}). The literature's tables (Bodhaine et al. 1999, Sneep and
 * Ubachs 2005) take the ideal p ÷ (k_B T), which leaves σ high by 1 ÷ Z²: 3.1% for NH₃, 1.4% for
 * CO₂ and Xe, 0.5% for Kr and at most 0.15% for the other gases. The dense gas's own fluctuation
 * factor at a body's surface is a recorded omission (Design note 4). Design note 4 gives about
 * 1.06 at Venus's surface; NIST's CO₂ there gives ρk_BTκ_T = 0.985, about 1.025 with the local
 * field (R08's Risks).
 *
 * Wavelengths are in vacuum, nm. In the formulas below λ is in µm and ν, the vacuum wavenumber,
 * in cm⁻¹. Where one exists, a formula's machine-readable copy is refractiveindex.info's database
 * (Polyanskiy, Sci. Data 11 (2024) 94; CC0), named in {@link Dispersion.dataFile}. The
 * coefficients were checked against the paper as printed wherever the paper could be read, as
 * `rayleigh.test.ts` records. No formula here gives the tutorials' 5.8, 13.5 and 33.1 × 10⁻⁶ m⁻¹:
 * those are Riley et al. 2004's, which Bruneton and Neyret 2008 took (§2), a pure λ⁻⁴ law with no
 * King factor.
 *
 * Any keyed species has Rayleigh optics through {@link rayleighOf}, measured, estimated or none,
 * and a well-mixed gas of any species is one medium term, {@link molecularTerm} (R08.T3.c,
 * decision-composition §1.9).
 */

import type { Rgb } from "../photometry/toneCurve";
import type { AtmosphereColumn } from "./column";
import { CHANNEL_WAVELENGTHS_NM, type MediumTerm } from "./medium";

/**
 * Every gas with a measured dispersion here, by formula.
 *
 * @remarks
 * Plan 14's nine come first, in `Gas::ALL`'s order (`planetary::derive::atmosphere::Gas`,
 * `Hydrogen` … `Argon`), then Ne, Kr, Xe and N₂O.
 *
 * Its keys are plan 14's substance keys (P14.T49.a; a definite species by its formula in chemical
 * case), so these tables are keyed as the wire is. A wire key outside this list is not a `Gas`:
 * R08.T3.c's {@link rayleighOf} gives it an estimate or none, with its provenance, and the body is
 * labelled; `Gas` is never widened to admit an unmeasured species.
 *
 * A measured species is added as data, by its formula here and an entry in each of
 * {@link GAS_DISPERSION}, {@link GAS_KING_FACTOR} and {@link GAS_MOLAR_MASS_G_PER_MOL}, each with
 * its sources; the compiler names any record that lacks it. The gases a generator could plausibly
 * produce whose dispersion has no source checked here (CO, SO₂, H₂S, HCN, O₃, C₂H₆, C₂H₄ and
 * C₂H₂ among them) are R08.T3.c's estimated rows, {@link ESTIMATED_RAYLEIGH}.
 */
export const GASES = [
  "H2",
  "He",
  "H2O",
  "CH4",
  "NH3",
  "N2",
  "O2",
  "CO2",
  "Ar",
  "Ne",
  "Kr",
  "Xe",
  "N2O",
] as const;

/** A gas with Rayleigh optics here: one of {@link GASES}. */
export type Gas = (typeof GASES)[number];

/** A gas's measured refractivity, at the reference state its source states. */
export interface Dispersion {
  /**
   * n − 1 at the reference state, at a vacuum wavelength in nm within
   * {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
   */
  readonly nMinusOne: (wavelengthNm: number) => number;
  /** The temperature the formula is stated at, K. */
  readonly referenceK: number;
  /** The pressure the formula is stated at, Pa. */
  readonly referencePa: number;
  /**
   * The compressibility factor Z = p ÷ (N k_B T) of the gas the formula describes, at the
   * reference state: the real gas's number density is p ÷ (Z k_B T).
   */
  readonly compressibility: number;
  /** The primary source, and each branch's where a formula has two. */
  readonly source: string;
  /**
   * The refractiveindex.info database file (CC0) that carries the same formula, under
   * `database/data/`, or `undefined` where the database has none.
   */
  readonly dataFile: string | undefined;
  /**
   * The vacuum wavelengths the formula was fitted over, nm.
   *
   * @remarks
   * Between these and {@link RAYLEIGH_WAVELENGTH_RANGE_NM}'s ends the formula is extrapolated.
   */
  readonly measuredNm: readonly [number, number];
}

/** A gas's King factor and its source. */
export interface KingFactor {
  /**
   * F_K at a vacuum wavelength in nm within {@link RAYLEIGH_WAVELENGTH_RANGE_NM}, at least 1.
   */
  readonly factor: (wavelengthNm: number) => number;
  /** The primary source of the factor or of the depolarisation it comes from. */
  readonly source: string;
}

/**
 * The vacuum wavelengths the formulas are evaluated over, nm: 300 to 1,000.
 *
 * @remarks
 * The range holds the bake bins (380–760 nm, Design note 5), the channels and the CIE functions'
 * visible span. Every formula's poles lie outside it, the nearest being dry air's at 160 nm and
 * CO₂'s infrared term at 4.14 µm. A wavelength outside the range is a caller's unit error, and is
 * refused.
 */
export const RAYLEIGH_WAVELENGTH_RANGE_NM: readonly [number, number] = [300, 1_000];

/**
 * The Boltzmann constant, J K⁻¹, exact in the 2019 SI.
 *
 * @remarks
 * The sim's `hyperion_base::units::consts::BOLTZMANN_CONSTANT`. R08.T3.a's column imports it from
 * here; `lib/system/constants.ts`, which holds G (R08.T3.d), is where it may move once both land.
 */
export const BOLTZMANN_J_PER_K = 1.380_649e-23;

/** 0 °C, K: the reference temperature of most refractivity formulas. */
const ZERO_CELSIUS_K = 273.15;
/** 15 °C, K: Edlén's and Peck's "standard air" temperature, and Sneep and Ubachs's. */
const FIFTEEN_CELSIUS_K = 288.15;
/** 20 °C, K: Zhang et al.'s, Křen's and Ciddor's water vapour's temperature. */
const TWENTY_CELSIUS_K = 293.15;
/** One standard atmosphere, 760 torr, Pa (exact by definition). */
const STANDARD_ATMOSPHERE_PA = 101_325;
/** 1,000 mbar, Pa: Börzsönyi et al. 2008's reference pressure. */
const THOUSAND_MILLIBAR_PA = 100_000;

/**
 * Where N₂'s two branches meet, nm. 468 is Peck and Khanna's shortest measured line, below which
 * Bates 1984's ultraviolet branch takes over (Sneep and Ubachs's 21,360 cm⁻¹ is 468.2 nm). The
 * branches agree there to 4 × 10⁻⁷ of n − 1.
 */
const N2_BRANCH_NM = 468;

/** The noble gases' source. */
const BORZSONYI_2008 =
  "Börzsönyi, Heiner, Kalashnikov, Kovács and Osvay, Appl. Opt. 47 (2008) 4856";

/** λ⁻², the squared vacuum wavenumber, µm⁻². */
function wavenumberSquaredPerUm2(wavelengthNm: number): number {
  const perUm = 1_000 / wavelengthNm;
  return perUm * perUm;
}

/** ν², the squared vacuum wavenumber, cm⁻². */
function wavenumberSquaredPerCm2(wavelengthNm: number): number {
  const perCm = 1e7 / wavelengthNm;
  return perCm * perCm;
}

/**
 * n − 1 from a two-term Sellmeier form, n² − 1 = B₁λ² ÷ (λ² − C₁) + B₂λ² ÷ (λ² − C₂), λ in µm and
 * C in µm², written as (n² − 1) ÷ (n + 1) so that no difference of two numbers near 1 is taken.
 */
function sellmeierNMinusOne(
  wavelengthNm: number,
  b1: number,
  c1Um2: number,
  b2: number,
  c2Um2: number,
): number {
  const l2 = (wavelengthNm / 1_000) ** 2;
  const nSquaredMinusOne = (b1 * l2) / (l2 - c1Um2) + (b2 * l2) / (l2 - c2Um2);
  return nSquaredMinusOne / (Math.sqrt(1 + nSquaredMinusOne) + 1);
}

/**
 * Each gas's measured refractivity and the reference state its source states (Design note 4's
 * table, with the corrections recorded in R08's Risks).
 *
 * @remarks
 * λ is in µm and ν in cm⁻¹. Z is from the NIST Chemistry WebBook (SRD 69,
 * Lemmon, McLinden and Friend), Z = p ÷ (ρRT) from its density at the state.
 * - H₂: Peck and Huang 1977, 10⁶(n − 1) = 14,895.6 ÷ (180.7 − λ⁻²) + 4,903.7 ÷ (92 − λ⁻²), at 0 °C
 *   and 760 torr. Z = 1.000 624.
 * - He: Mansfield and Peck 1969, 10⁵(n − 1) = 1,470.091 ÷ (423.98 − λ⁻²), at 0 °C and 760 torr.
 *   Z = 1.000 532.
 * - H₂O: Ciddor 1996, eq. 3, pure water vapour at 20 °C and 1,333 Pa, 10⁸(n − 1) = c_f (w₀ +
 *   w₁λ⁻² + w₂λ⁻⁴ + w₃λ⁻⁶) with w = 295.235, 2.6422, −0.032 380, 0.004 028 and c_f = 1.022 (as
 *   NIST's Engineering Metrology Toolbox reproduces them; refractiveindex.info has no copy).
 *   Z = 0.999 237.
 * - CH₄: He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 10,
 *   10⁸(n − 1) = 3,603.09 + 4.403 62 × 10¹⁴ ÷ (1.1741 × 10¹⁰ − ν²), at 288.15 K and
 *   1013.25 hPa, fitted over 264–671 nm. Design note 4's Sneep and Ubachs 2005 eq. 18, a fit to
 *   Hohm 1993's polarisabilities, runs 13% high in n − 1, against Loria 1909 and Wilmouth and
 *   Sayres 2019 (R08's Risks). He et al. derive n from measured cross-sections through the ideal
 *   N = 2.546 899 × 10¹⁹ cm⁻³, so Z = 1 returns those cross-sections.
 * - NH₃: C. and M. Cuthbertson 1914, n − 1 = 0.032 953 ÷ (90.392 − λ⁻²), at 0 °C and 760 mm,
 *   over the cadmium-to-lithium lines (air wavelengths; the difference from vacuum is 2 × 10⁻⁵
 *   of n − 1). Z = 0.984 798; whether the paper reduced its readings with the real or the ideal
 *   gas is unread, ±3% in the cross-section.
 * - N₂: Peck and Khanna 1966's own 15 °C form from 468 nm, 10⁸(n − 1) = 6,497.378 +
 *   3,073,864.9 ÷ (144 − λ⁻²). Sneep and Ubachs's eq. 10 (6,498.2 + …) is the 0 °C form scaled by
 *   the ideal gas, 1.5 × 10⁻⁴ higher. Below 468 nm, Bates 1984's ultraviolet branch,
 *   10⁸(n − 1) = 5,677.465 + 318.818 74 × 10¹² ÷ (14.4 × 10⁹ − ν²), an interpolation to Abjean,
 *   Mehu and Johannin-Gilles 1970 (Sneep and Ubachs 2005, eq. 11). Both at 15 °C and 101,325 Pa.
 *   Z = 0.999 715.
 * - O₂: Křen 2011's refit of Zhang, Lu and Wang 2008's data with others, n − 1 =
 *   1.181 494 × 10⁻⁴ + 9.708 931 × 10⁻³ ÷ (75.4 − λ⁻²), at 20 °C and 101,325 Pa. Zhang's own
 *   eq. 20 holds only over 740–860 nm. Křen's comment was not read (Optica, closed); the
 *   coefficients are refractiveindex.info's. Z = 0.999 282.
 * - CO₂: Bideau-Mehu et al. 1973 as Sneep and Ubachs 2005 (eq. 13) correct it, at 0 °C and 760
 *   torr, not 15 °C (the trap of Design note 4). Sneep and Ubachs's 15 °C coefficients are these
 *   times 273.15 ÷ 288.15 to 4 × 10⁻⁵ once their two misprints are read right: the prefactor
 *   printed 1.1427 × 10⁶ is 10³ too large, and the last numerator printed 0.121 814 5 × 10⁻⁴ is 10⁴
 *   too small. Z = 0.993 265.
 * - Ar: Peck and Fisher 1964's 15 °C form, 10⁷(n − 1) = 643.2135 + 286,060.21 ÷ (144 − λ⁻²), at
 *   760 torr. Z = 0.999 260.
 * - Ne, Kr and Xe: Börzsönyi, Heiner, Kalashnikov, Kovács and Osvay, Appl. Opt. 47 (2008) 4856,
 *   Sellmeier forms at 0 °C and 1,000 mbar (their Table 2). They measured the phase at 800 nm
 *   and joined it to the literature's ultraviolet and visible indices; `measuredNm` is
 *   refractiveindex.info's validity range, 400–1,000 nm. The coefficients are the database's
 *   copies, since the paper was not read, and Xe's C₁ takes the database's correction of the
 *   paper's 12.75 × 10⁻⁶ to 12.75 × 10⁻³ µm². The same paper's Ar, He and N₂ forms agree with Peck
 *   and Fisher, Mansfield and Peck, and Peck and Khanna to 0.3%, and its Ne and Xe with C. and M.
 *   Cuthbertson's (Proc. R. Soc. A 135 (1932) 40 for Ne, 84 (1910) 13 for Xe) to 0.1% and 0.3%.
 *   Their Kr sits 0.5% lower, where Koch 1949's (as Leonard 1974 compiles it) agrees with
 *   Börzsönyi to 0.1%. Z = 1.000 483, 0.997 282 and 0.993 245; it is open whether the literature
 *   the fit rests on reduced as a real gas (±1.4% in Xe's cross-section, ±0.6% in Kr's).
 * - N₂O: He et al. 2021, eq. 9, 10⁸(n − 1) = 22,095 + 1.662 91 × 10¹⁴ ÷ (6.752 26 × 10⁹ − ν²), at
 *   288.15 K and 1013.25 hPa, fitted over 307–725 nm, derived like their CH₄ through the ideal N,
 *   so Z = 1. Whether that N was the ideal one at their measuring state (about 295 K and
 *   1020 hPa) is unstated; if it was, both gases' cross-sections run high by 1 ÷ Z there, 0.57%
 *   for N₂O and 0.18% for CH₄.
 */
export const GAS_DISPERSION: Readonly<Record<Gas, Dispersion>> = {
  H2: {
    nMinusOne: (wavelengthNm) => {
      const s2 = wavenumberSquaredPerUm2(wavelengthNm);
      return (14_895.6 / (180.7 - s2) + 4_903.7 / (92 - s2)) * 1e-6;
    },
    referenceK: ZERO_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 1.000_624,
    source: "Peck and Huang, J. Opt. Soc. Am. 67 (1977) 1550",
    dataFile: "main/H2/nk/Peck.yml",
    measuredNm: [168, 1_694.5],
  },
  He: {
    nMinusOne: (wavelengthNm) =>
      (1_470.091 / (423.98 - wavenumberSquaredPerUm2(wavelengthNm))) * 1e-5,
    referenceK: ZERO_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 1.000_532,
    source: "Mansfield and Peck, J. Opt. Soc. Am. 59 (1969) 199",
    dataFile: "main/He/nk/Mansfield.yml",
    measuredNm: [480.1, 2_058.6],
  },
  H2O: {
    nMinusOne: (wavelengthNm) => {
      const s2 = wavenumberSquaredPerUm2(wavelengthNm);
      return 1.022 * (295.235 + s2 * (2.642_2 + s2 * (-0.032_38 + s2 * 0.004_028))) * 1e-8;
    },
    referenceK: TWENTY_CELSIUS_K,
    referencePa: 1_333,
    compressibility: 0.999_237,
    source: "Ciddor, Appl. Opt. 35 (1996) 1566, eq. 3",
    dataFile: undefined,
    measuredNm: [300, 1_690],
  },
  CH4: {
    nMinusOne: (wavelengthNm) =>
      (3_603.09 + 4.403_62e14 / (1.174_1e10 - wavenumberSquaredPerCm2(wavelengthNm))) * 1e-8,
    referenceK: FIFTEEN_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 1,
    source: "He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 10",
    dataFile: undefined,
    measuredNm: [264, 671],
  },
  NH3: {
    nMinusOne: (wavelengthNm) => 0.032_953 / (90.392 - wavenumberSquaredPerUm2(wavelengthNm)),
    referenceK: ZERO_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 0.984_798,
    source: "C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1",
    dataFile: "main/NH3/nk/Cuthbertson.yml",
    measuredNm: [480, 670.8],
  },
  N2: {
    nMinusOne: (wavelengthNm) => {
      if (wavelengthNm >= N2_BRANCH_NM) {
        return (6_497.378 + 3_073_864.9 / (144 - wavenumberSquaredPerUm2(wavelengthNm))) * 1e-8;
      }
      return (5_677.465 + 318.818_74e12 / (14.4e9 - wavenumberSquaredPerCm2(wavelengthNm))) * 1e-8;
    },
    referenceK: FIFTEEN_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 0.999_715,
    source:
      "Peck and Khanna, J. Opt. Soc. Am. 56 (1966) 1059, from 468 nm; below, Bates, Planet. Space Sci. 32 (1984) 785, as Sneep and Ubachs, J. Quant. Spectrosc. Radiat. Transfer 92 (2005) 293, eq. 11, print it",
    dataFile: "main/N2/nk/Peck-15C.yml",
    measuredNm: [254, 2_058.6],
  },
  O2: {
    nMinusOne: (wavelengthNm) =>
      1.181_494e-4 + 9.708_931e-3 / (75.4 - wavenumberSquaredPerUm2(wavelengthNm)),
    referenceK: TWENTY_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 0.999_282,
    source:
      "Křen, Appl. Opt. 50 (2011) 6484, refitting Zhang, Lu and Wang, Appl. Opt. 47 (2008) 3143",
    dataFile: "main/O2/nk/Zhang.yml",
    measuredNm: [400, 1_800],
  },
  CO2: {
    nMinusOne: (wavelengthNm) => {
      const s2 = wavenumberSquaredPerUm2(wavelengthNm);
      return (
        6.991_00e-2 / (166.175 - s2) +
        1.447_20e-3 / (79.609 - s2) +
        6.429_41e-5 / (56.306_4 - s2) +
        5.213_06e-5 / (46.019_6 - s2) +
        1.468_47e-6 / (0.058_473_8 - s2)
      );
    },
    referenceK: ZERO_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 0.993_265,
    source:
      "Bideau-Mehu, Guern, Abjean and Johannin-Gilles, Opt. Commun. 9 (1973) 432, as corrected by Sneep and Ubachs 2005, eq. 13",
    dataFile: "main/CO2/nk/Bideau-Mehu.yml",
    measuredNm: [180.7, 1_694.5],
  },
  Ar: {
    nMinusOne: (wavelengthNm) =>
      (643.213_5 + 286_060.21 / (144 - wavenumberSquaredPerUm2(wavelengthNm))) * 1e-7,
    referenceK: FIFTEEN_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 0.999_26,
    source: "Peck and Fisher, J. Opt. Soc. Am. 54 (1964) 1362",
    dataFile: "main/Ar/nk/Peck-15C.yml",
    measuredNm: [467.9, 2_058.6],
  },
  Ne: {
    nMinusOne: (wavelengthNm) =>
      sellmeierNMinusOne(wavelengthNm, 9_154.48e-8, 656.97e-6, 4_018.63e-8, 5.728e-3),
    referenceK: ZERO_CELSIUS_K,
    referencePa: THOUSAND_MILLIBAR_PA,
    compressibility: 1.000_483,
    source: BORZSONYI_2008,
    dataFile: "main/Ne/nk/Borzsonyi.yml",
    measuredNm: [400, 1_000],
  },
  Kr: {
    nMinusOne: (wavelengthNm) =>
      sellmeierNMinusOne(wavelengthNm, 26_102.88e-8, 2.01e-6, 56_946.82e-8, 10.043e-3),
    referenceK: ZERO_CELSIUS_K,
    referencePa: THOUSAND_MILLIBAR_PA,
    compressibility: 0.997_282,
    source: BORZSONYI_2008,
    dataFile: "main/Kr/nk/Borzsonyi.yml",
    measuredNm: [400, 1_000],
  },
  Xe: {
    nMinusOne: (wavelengthNm) =>
      sellmeierNMinusOne(wavelengthNm, 103_701.61e-8, 12.75e-3, 31_228.61e-8, 0.561e-3),
    referenceK: ZERO_CELSIUS_K,
    referencePa: THOUSAND_MILLIBAR_PA,
    compressibility: 0.993_245,
    source: BORZSONYI_2008,
    dataFile: "main/Xe/nk/Borzsonyi.yml",
    measuredNm: [400, 1_000],
  },
  N2O: {
    nMinusOne: (wavelengthNm) =>
      (22_095 + 1.662_91e14 / (6.752_26e9 - wavenumberSquaredPerCm2(wavelengthNm))) * 1e-8,
    referenceK: FIFTEEN_CELSIUS_K,
    referencePa: STANDARD_ATMOSPHERE_PA,
    compressibility: 1,
    source: "He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 9",
    dataFile: undefined,
    measuredNm: [307, 725],
  },
};

/**
 * Dry air's refractivity, for the Earth check only: Peck and Reeder, J. Opt. Soc. Am. 62 (1972)
 * 958, standard air (dry, 15 °C, 101,325 Pa).
 *
 * @remarks
 * 10⁸(n − 1) = 8,060.51 + 2,480,990 ÷ (132.274 − λ⁻²) + 17,455.7 ÷ (39.329 57 − λ⁻²), λ the vacuum
 * wavelength in µm, valid down to about 230 nm (the paper's abstract), as Bodhaine et al., J.
 * Atmos. Oceanic Technol. 16 (1999) 1854, eq. 4, quote it. Peck and Reeder's standard air carries
 * 330 ppm of CO₂ (Bodhaine §1); Bodhaine applies the formula as 300 ppm air, as R05's `earth.ts`
 * and {@link dryAirKingFactor} do, a 3 × 10⁻⁵ difference in σ. refractiveindex.info's copy
 * (`other/mixed gases/air/nk/Peck.yml`) has the same coefficients but labels the air with 450 ppm,
 * Ciddor's later standard. Z = 0.999 592 is CIPM-2007's for dry air at the state (Picard, Davis,
 * Gläser and Fujii, Metrologia 45 (2008) 149, the BIPM form Ciddor 1996 uses). Plan 14's
 * atmospheres are mixed from {@link GAS_DISPERSION} instead (R08.T3.c), which reproduces this
 * to 0.1%.
 */
export const DRY_AIR_DISPERSION: Dispersion = {
  nMinusOne: (wavelengthNm) => {
    const s2 = wavenumberSquaredPerUm2(wavelengthNm);
    return (8_060.51 + 2_480_990 / (132.274 - s2) + 17_455.7 / (39.329_57 - s2)) * 1e-8;
  },
  referenceK: FIFTEEN_CELSIUS_K,
  referencePa: STANDARD_ATMOSPHERE_PA,
  compressibility: 0.999_592,
  source: "Peck and Reeder, J. Opt. Soc. Am. 62 (1972) 958",
  dataFile: "other/mixed gases/air/nk/Peck.yml",
  measuredNm: [230, 1_690],
};

/** λ², µm², for the King factors' λ⁻² forms. */
function wavelengthSquaredUm2(wavelengthNm: number): number {
  return (wavelengthNm / 1_000) ** 2;
}

/** An isotropic molecule's King factor: 1, whatever the wavelength. */
function isotropic(source: string): KingFactor {
  return { factor: () => 1, source };
}

/**
 * Each gas's King factor F_K, the anisotropy correction to its cross-section, with its source.
 *
 * @remarks
 * F_K = (6 + 3ρₙ) ÷ (6 − 7ρₙ) = (3 + 6ρₚ) ÷ (3 − 4ρₚ) for a depolarisation ratio ρₙ in natural
 * light or ρₚ in linearly polarised light (Sneep and Ubachs 2005, eqs. 5 and 7).
 * - N₂: 1.034 + 3.17 × 10⁻⁴ λ⁻², and O₂: 1.096 + 1.385 × 10⁻³ λ⁻² + 1.448 × 10⁻⁴ λ⁻⁴ (Bates,
 *   Planet. Space Sci. 32 (1984) 785, as Bodhaine et al. 1999, eqs. 5 and 6, quote them).
 * - CO₂: 1.1364 + 25.3 × 10⁻¹² ν² (Sneep and Ubachs 2005, eq. 14, fitted to the depolarisation
 *   Alms, Burnham and Flygare, J. Chem. Phys. 63 (1975) 3321, measured at 457.9–647.1 nm).
 * - N₂O: ρₚ = 0.0577 + 11.8 × 10⁻¹² ν² (Sneep and Ubachs 2005, eq. 19, from Alms et al. 1975 over
 *   the same 457.9–647.1 nm), 1.225 at 532.2 nm as their Table 2 has it.
 * - Ar, He, Ne, Kr and Xe: 1, atoms; CH₄: 1, a spherical top (Sneep and Ubachs 2005, §5.2: below
 *   1.0007).
 * - H₂O: 1.001, from Murphy, J. Chem. Phys. 67 (1977) 5877's ρₚ = (3.0 ± 1.4) × 10⁻⁴ at 514.5 nm:
 *   1.0010 ± 0.0005.
 * - H₂: 1.0312 + 3.09 × 10⁻⁴ λ⁻²: 1 + (2/9)(γ ÷ ᾱ)² for the v = 0, J = 0 averages of Raj,
 *   Hamaguchi and Witek, J. Chem. Phys. 148 (2018) 104308's ab initio α∥(r, ω) and α⊥(r, ω),
 *   fitted here over 380–800 nm to 2 × 10⁻⁵ (1.0322 at 550 nm). Design note 4's "Hohm 1993" (Mol.
 *   Phys. 78, 929) is a hydrocarbon mean-polarisability paper; Hohm 1994 (Chem. Phys. 179, 533)
 *   holds the anisotropies and was not read.
 * - NH₃: 1, provisional, until Hohm 1994's anisotropy is read (R08's Risks); NH₃'s small
 *   anisotropy leaves it about 1% low.
 */
export const GAS_KING_FACTOR: Readonly<Record<Gas, KingFactor>> = {
  H2: {
    factor: (wavelengthNm) => 1.031_2 + 3.09e-4 / wavelengthSquaredUm2(wavelengthNm),
    source:
      "1 + (2/9)(γ/ᾱ)² from Raj, Hamaguchi and Witek, J. Chem. Phys. 148 (2018) 104308, v = 0, J = 0",
  },
  He: isotropic("an atom"),
  H2O: {
    factor: () => 1.001,
    source: "Murphy, J. Chem. Phys. 67 (1977) 5877: ρₚ = (3.0 ± 1.4) × 10⁻⁴",
  },
  CH4: isotropic("a spherical top (Sneep and Ubachs 2005, §5.2)"),
  NH3: isotropic("provisional: Hohm, Chem. Phys. 179 (1994) 533 not read"),
  N2: {
    factor: (wavelengthNm) => 1.034 + 3.17e-4 / wavelengthSquaredUm2(wavelengthNm),
    source: "Bates, Planet. Space Sci. 32 (1984) 785 (Bodhaine et al. 1999, eq. 5)",
  },
  O2: {
    factor: (wavelengthNm) => {
      const um2 = wavelengthSquaredUm2(wavelengthNm);
      return 1.096 + 1.385e-3 / um2 + 1.448e-4 / (um2 * um2);
    },
    source: "Bates, Planet. Space Sci. 32 (1984) 785 (Bodhaine et al. 1999, eq. 6)",
  },
  CO2: {
    factor: (wavelengthNm) => 1.136_4 + 25.3e-12 * wavenumberSquaredPerCm2(wavelengthNm),
    source: "Sneep and Ubachs 2005, eq. 14 (Alms et al. 1975)",
  },
  Ar: isotropic("an atom"),
  Ne: isotropic("an atom"),
  Kr: isotropic("an atom"),
  Xe: isotropic("an atom"),
  N2O: {
    factor: (wavelengthNm) => {
      const rho = 0.057_7 + 11.8e-12 * wavenumberSquaredPerCm2(wavelengthNm);
      return (3 + 6 * rho) / (3 - 4 * rho);
    },
    source: "Sneep and Ubachs 2005, eq. 19 (Alms et al. 1975)",
  },
};

/**
 * Each gas's molar mass, g mol⁻¹, the column's mean molecular mass's terms (R08.T3.a).
 *
 * @remarks
 * Plan 14's nine are the sim's `Gas::molar_mass_g_per_mol` (IUPAC 2021 standard atomic weights,
 * abridged; the sim rounds He's 4.0026 to 4.003), copied so that the client's column and the sim's
 * agree. Ne (20.180), Kr (83.798) and
 * Xe (131.29) are IUPAC 2021's abridged atomic weights, and N₂O is 2 × 14.007 + 15.999 from them.
 */
export const GAS_MOLAR_MASS_G_PER_MOL: Readonly<Record<Gas, number>> = {
  H2: 2.016,
  He: 4.003,
  H2O: 18.015,
  CH4: 16.043,
  NH3: 17.031,
  N2: 28.014,
  O2: 31.998,
  CO2: 44.009,
  Ar: 39.95,
  Ne: 20.18,
  Kr: 83.798,
  Xe: 131.29,
  N2O: 44.013,
};

/**
 * A gas's King factor at a vacuum wavelength: its {@link GAS_KING_FACTOR}.
 *
 * @param wavelengthNm - A vacuum wavelength within {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 * @throws RangeError if the wavelength is outside the range, a caller's unit error.
 */
export function kingFactor(gas: Gas, wavelengthNm: number): number {
  checkWavelength(wavelengthNm);
  return GAS_KING_FACTOR[gas].factor(wavelengthNm);
}

/**
 * Dry air's King factor at 300 ppm of CO₂, the Earth check's pair to {@link DRY_AIR_DISPERSION}.
 *
 * @remarks
 * Bates's N₂ and O₂ factors ({@link kingFactor}), with Ar's 1 and CO₂'s 1.15 (Bates 1984), mixed
 * by volume as Bodhaine et al. 1999, eq. 23, does: (78.084 F_N₂ + 20.946 F_O₂ + 0.934 + 0.03 ×
 * 1.15) ÷ 99.994, the composition in percent.
 *
 * @param wavelengthNm - A vacuum wavelength within {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 * @throws RangeError if the wavelength is outside the range, a caller's unit error.
 */
export function dryAirKingFactor(wavelengthNm: number): number {
  const co2Percent = 0.03;
  return (
    (78.084 * kingFactor("N2", wavelengthNm) +
      20.946 * kingFactor("O2", wavelengthNm) +
      0.934 +
      co2Percent * 1.15) /
    (78.084 + 20.946 + 0.934 + co2Percent)
  );
}

/** The real gas's number density at a formula's reference state, m⁻³: p ÷ (Z k_B T). */
export function referenceNumberDensityPerM3(dispersion: Dispersion): number {
  return (
    dispersion.referencePa /
    (dispersion.compressibility * BOLTZMANN_J_PER_K * dispersion.referenceK)
  );
}

/**
 * The Rayleigh cross-section of a molecule from a dispersion and a King factor, m².
 *
 * @remarks
 * 24π³ ÷ (λ⁴ N_ref²) × ((n² − 1) ÷ (n² + 2))² × F_K, with N_ref the dispersion's reference
 * density and both n and F_K taken at the wavelength.
 *
 * @param kingFactorAt - F_K at a wavelength in nm, at least 1, called only within the range.
 * @param wavelengthNm - A vacuum wavelength within {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 * @throws RangeError if the wavelength is outside the range, a caller's unit error.
 */
export function crossSectionFromDispersionM2(
  dispersion: Dispersion,
  kingFactorAt: (wavelengthNm: number) => number,
  wavelengthNm: number,
): number {
  checkWavelength(wavelengthNm);
  const king = kingFactorAt(wavelengthNm);
  const delta = dispersion.nMinusOne(wavelengthNm);
  // n² − 1 = δ(2 + δ), so that no difference of two numbers near 1 is taken.
  const nSquaredMinusOne = delta * (2 + delta);
  const lorentzLorenz = nSquaredMinusOne / (nSquaredMinusOne + 3);
  const wavelengthM = wavelengthNm * 1e-9;
  const lambdaSquared = wavelengthM * wavelengthM;
  const density = referenceNumberDensityPerM3(dispersion);
  return (
    ((24 * Math.PI ** 3) / (lambdaSquared * lambdaSquared * density * density)) *
    lorentzLorenz *
    lorentzLorenz *
    king
  );
}

/**
 * A gas's Rayleigh cross-section per molecule, m², at a vacuum wavelength: its
 * {@link GAS_DISPERSION} at its reference state with its {@link GAS_KING_FACTOR}.
 *
 * @param wavelengthNm - A vacuum wavelength within {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 * @throws RangeError if the wavelength is outside the range, a caller's unit error.
 */
export function rayleighCrossSectionM2(gas: Gas, wavelengthNm: number): number {
  return crossSectionFromDispersionM2(
    GAS_DISPERSION[gas],
    GAS_KING_FACTOR[gas].factor,
    wavelengthNm,
  );
}

/**
 * How far a mixture's mole fractions may sum from 1: 10⁻⁹, plan 14's tolerance for P14.T24.a's
 * gas fractions, by which {@link molecularMixture} and R08.T3.a's `column.ts` mixtures refuse a
 * mixture.
 */
export const MOLE_FRACTION_SUM_TOLERANCE = 1e-9;

/**
 * One species of a well-mixed gas, by the registry key the wire sends (`gases[].species`;
 * P14.T49.a), never narrowed to {@link Gas}.
 */
export interface GasFraction {
  /** The species' substance key, a formula in chemical case (`N2`, `CO`, `H2S`), or any key. */
  readonly species: string;
  /** x, in [0, 1]. */
  readonly moleFraction: number;
}

/**
 * A well-mixed mixture as the record carries it: largest first, summing to 1 within
 * {@link MOLE_FRACTION_SUM_TOLERANCE}. A species is listed once.
 *
 * @remarks
 * It is the wire's `gases: [{ species, mole_fraction }]` in camelCase (P14.T35.e), so the
 * record's list passes to {@link molecularTerm} unconverted (decision-composition §1.9).
 */
export type GasFractions = ReadonlyArray<GasFraction>;

/**
 * Where a species' optics come from.
 *
 * - `measured`: a measured dispersion and King factor ({@link GASES}).
 * - `estimated`: a stated estimate with its sources and uncertainty ({@link ESTIMATED_RAYLEIGH}).
 * - `none`: nothing known here, such as a key a newer server sent. It contributes nothing.
 */
export type OpticsProvenance = "measured" | "estimated" | "none";

/** A species' Rayleigh scattering per molecule, and where it comes from. */
export interface SpeciesRayleigh {
  readonly species: string;
  readonly provenance: OpticsProvenance;
  /**
   * σ per molecule, m², at a vacuum wavelength in nm within {@link RAYLEIGH_WAVELENGTH_RANGE_NM};
   * 0 for `none`.
   */
  readonly crossSectionM2: (wavelengthNm: number) => number;
  /**
   * ρ, the depolarisation ratio in natural light, 6(F_K − 1) ÷ (3 + 7F_K), at a vacuum wavelength in
   * nm within the range; 0 for a polarisability estimate (F_K = 1) and for `none`.
   */
  readonly depolarisation: (wavelengthNm: number) => number;
  /** The sources of the cross-section and the King factor; for `none`, why there are none. */
  readonly source: string;
}

/**
 * A gas's Rayleigh scattering where {@link GASES} holds no vetted measurement: an estimate, with its
 * sources and its uncertainty (decision-composition §1.9).
 *
 * - `dispersion`: a measured visible dispersion, read as {@link GAS_DISPERSION}'s are, with a King
 *   factor where a measured one was found in a secondary source and F_K = 1 where none was:
 *   {@link crossSectionFromDispersionM2}.
 * - `polarisability`: a mean dipole polarisability α with F_K = 1, σ = (128π⁵ ÷ 3) α² ÷ λ⁴
 *   ({@link polarisabilityCrossSectionM2}), α static or dispersed by one oscillator.
 */
export type EstimatedRayleigh = DispersionEstimate | PolarisabilityEstimate;

/** An estimate from a measured dispersion whose King factor, or its vetting, is not to hand. */
export interface DispersionEstimate {
  readonly kind: "dispersion";
  /** The species' substance key. */
  readonly species: string;
  readonly dispersion: Dispersion;
  readonly kingFactor: KingFactor;
  /** {@link PolarisabilityEstimate.uncertaintyFactor}'s meaning. */
  readonly uncertaintyFactor: number | undefined;
  /** Why the row is an estimate, and how its uncertainty is reckoned. */
  readonly basis: string;
}

/** An estimate from a mean polarisability, with F_K = 1. */
export interface PolarisabilityEstimate {
  readonly kind: "polarisability";
  /** The species' substance key. */
  readonly species: string;
  /**
   * α(0), the static electronic polarisability volume, the SI polarisability ÷ 4πε₀, m³
   * (1 Å³ = 10⁻³⁰ m³).
   */
  readonly polarisabilityM3: number;
  /**
   * The vacuum wavelength λ_r of one oscillator that disperses α, α(λ) = α(0) ÷ (1 − (λ_r ÷ λ)²),
   * nm, below {@link RAYLEIGH_WAVELENGTH_RANGE_NM}; or `undefined` for the static rule, α(λ) = α(0).
   */
  readonly resonanceNm: number | undefined;
  /**
   * The factor, either way, within which the true σ lies of the estimate over 440–680 nm, the
   * render channels' span: α's own uncertainty (twice over, in σ), the King factor the rule leaves
   * out, and the dispersion it omits. `undefined` where no bound is known: the species' lines or
   * bands lie inside that span, where no polarisability rule holds.
   */
  readonly uncertaintyFactor: number | undefined;
  /** α's primary source, its uncertainty, and the rule where α is not measured for the species. */
  readonly basis: string;
}

/** Å³, m³. */
const CUBIC_ANGSTROM_M3 = 1e-30;

/**
 * The atomic unit of polarisability volume, a₀³, m³: the Bohr radius 5.291 772 105 44 × 10⁻¹¹ m
 * cubed (CODATA 2022), 0.148 184 7 Å³.
 */
const BOHR_CUBED_M3 = 5.291_772_105_44e-11 ** 3;

/** 25 °C, K: Ramaswamy's reference temperature. */
const TWENTY_FIVE_CELSIUS_K = 298.15;

/** The atoms' compilation. */
const SCHWERDTFEGER_NAGLE =
  "Schwerdtfeger and Nagle, Mol. Phys. 117 (2019) 1200, the 2018 table of the neutral elements' static dipole polarisabilities";

/** Olney et al.'s sum-rule polarisabilities, read through CCCBDB, a finding aid. */
const OLNEY_1997 =
  "Olney, Cann, Cooper and Brion, Chem. Phys. 223 (1997) 59, a dipole-oscillator-strength sum (as NIST's CCCBDB lists it; the paper was not read)";

/** NIST's own computed polarisabilities, the only computations found for these radicals. */
const CCCBDB_COMPUTED =
  "NIST CCCBDB, Release 22 (Johnson, ed., NIST SRD 101, 2022), its own calculations";

/** The Cuthbertsons' 1910 paper on SO₂ and H₂S. */
const CUTHBERTSON_1910 = "C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171";

/**
 * n − 1 = A ÷ (B − λ⁻²), λ in µm and A and B in µm⁻²: the Cuthbertsons' and Ramaswamy's one-term
 * form.
 */
function oneTermNMinusOne(aPerUm2: number, bPerUm2: number): (wavelengthNm: number) => number {
  return (wavelengthNm) => aPerUm2 / (bPerUm2 - wavenumberSquaredPerUm2(wavelengthNm));
}

/** F_K = 1, no King factor found. */
function noKingFactor(why: string): KingFactor {
  return { factor: () => 1, source: `1, taken: ${why}` };
}

/**
 * The registry's gases outside {@link GASES}, each an estimate (decision-composition §1.9), in the
 * registry's order (§1.1). Their keys are plan 14's substance keys, as {@link GASES}' are.
 *
 * @remarks
 * Where a measured visible dispersion exists, the row takes it (the brainstorm's "Atmosphere": the
 * optics registry holds measured dispersions, and an estimate only where no measurement exists):
 * CO, SO₂, H₂S and O₃ from C. and M. Cuthbertson, C₂H₆ from Loria and CH₃OH from Ramaswamy. Each
 * stays `estimated` because its King factor is not measured, or was found only in a secondary
 * source, and none has been vetted to {@link GAS_DISPERSION}'s standard (its reference state read in
 * the paper, its Z from NIST's equation of state). Promoting one to {@link GASES} is a ruling's
 * (R08's Risks, "Deviations in T3.c, as built").
 *
 * The rest take a polarisability: the static electronic α, never a dielectric value, which holds
 * vibrational polarisability; for the atoms, Schwerdtfeger and Nagle's recommended values. H, Mg
 * and Fe, whose dispersion one line or continuum carries, take one oscillator, H's from its exact
 * dynamic polarisability (Lee and Kim, MNRAS 347 (2004) 802), Mg's and Fe's from their resonance
 * lines (NIST ASD). NIST's CCCBDB served as a finding aid, and its own calculations are cited only
 * where no other value was found (SiO, TiO, VO and, through TiH, FeH).
 *
 * A static α leaves out the dispersion and the King factor, so for a molecule whose resonances lie
 * in the ultraviolet the estimate runs low, and lower towards the violet: for HCN, C₂H₂, C₂H₄ and
 * PH₃ by about 8–16% at 550 nm. For an atom or radical with lines in or near the visible (Na, K,
 * Ca, Ti, TiO and VO) no polarisability rule is meaningful near them: α changes sign across each
 * line, so σ diverges at the line and vanishes between lines, and a static α is many times wrong
 * near one (K's 5× high at 440 nm, Na's 46× low at 550 nm, Ca's 150–170× low at 440 nm). With
 * solar abundances in the gas phase their true far-wing scattering reaches about a third of a hot
 * giant's Rayleigh optical depth at 440 nm (Ca) and a tenth at 550 nm (Na), which these estimates
 * put at under 1.2% (the science check's budget at 2,000 K and 0.1 bar). Their lines are their
 * visible opacity, and R08.T4.c's absorbers.
 */
export const ESTIMATED_RAYLEIGH: ReadonlyArray<EstimatedRayleigh> = [
  {
    kind: "dispersion",
    species: "CO",
    dispersion: {
      nMinusOne: oneTermNMinusOne(0.040_508, 123.77),
      referenceK: ZERO_CELSIUS_K,
      referencePa: STANDARD_ATMOSPHERE_PA,
      compressibility: 1,
      source:
        "C. and M. Cuthbertson, Proc. R. Soc. Lond. A 97 (1920) 152, Table II: n − 1 = 0.040 508 ÷ (123.77 − λ⁻²), at 0 °C and 760 mm, reduced as an ideal gas",
      dataFile: undefined,
      measuredNm: [480, 670.8],
    },
    kingFactor: {
      factor: () => (3 + 6 * 0.004_8) / (3 - 4 * 0.004_8),
      source:
        "ρₚ = 0.0048 at 632.8 nm (Bridge and Buckingham, Proc. R. Soc. Lond. A 295 (1966) 334, as Sneep and Ubachs 2005, §5.1, quote it)",
    },
    uncertaintyFactor: 1.07,
    basis:
      "the King factor from a secondary source; Sneep and Ubachs 2005's measured (6.19 ± 0.40) × 10⁻²⁷ cm² at 532.2 nm (Table 2) is the 1σ check",
  },
  {
    kind: "dispersion",
    species: "SO2",
    dispersion: {
      nMinusOne: oneTermNMinusOne(0.063_733, 99.349),
      referenceK: ZERO_CELSIUS_K,
      referencePa: STANDARD_ATMOSPHERE_PA,
      compressibility: 1,
      source: `${CUTHBERTSON_1910}: n − 1 = 0.063 733 ÷ (99.349 − λ⁻²) per molecule at 0 °C and 760 mm, its scale from Cuthbertson and Metcalfe, Proc. R. Soc. Lond. A 80 (1908) 406 (660.86 × 10⁻⁶ at 589.3 nm)`,
      dataFile: undefined,
      measuredNm: [500, 670],
    },
    kingFactor: noKingFactor(
      "SO₂'s depolarisation (Bogaard et al. 1978; Baas and van den Hout 1979) was not read, and F_K lies near 1.02–1.07",
    ),
    uncertaintyFactor: 1.08,
    basis: "no King factor",
  },
  {
    kind: "dispersion",
    species: "H2S",
    dispersion: {
      nMinusOne: oneTermNMinusOne(0.053_785, 86.876),
      referenceK: ZERO_CELSIUS_K,
      referencePa: STANDARD_ATMOSPHERE_PA,
      compressibility: 1,
      source: `${CUTHBERTSON_1910}: n − 1 = 0.053 785 ÷ (86.876 − λ⁻²) per molecule at 0 °C and 760 mm`,
      dataFile: undefined,
      measuredNm: [486.1, 656.3],
    },
    kingFactor: noKingFactor(
      "ρ ≤ 0.003 in natural light (Ananthakrishnan, Proc. Indian Acad. Sci. A 2 (1935) 153), so F_K ≤ 1.005",
    ),
    uncertaintyFactor: 1.03,
    basis: "an upper bound on the King factor",
  },
  {
    kind: "dispersion",
    species: "O3",
    dispersion: {
      nMinusOne: oneTermNMinusOne(0.022_714, 46.968),
      referenceK: ZERO_CELSIUS_K,
      referencePa: STANDARD_ATMOSPHERE_PA,
      compressibility: 1,
      source:
        "C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1, Tables IX–XI: n − 1 = 0.022 714 ÷ (46.968 − λ⁻²) at 0 °C and 760 mm, a two-point fit, measured in O₂ with about 6% O₃, which the authors call unsafe beyond the second figure",
      dataFile: undefined,
      measuredNm: [480, 671],
    },
    kingFactor: noKingFactor("no measured depolarisation was found"),
    uncertaintyFactor: 1.15,
    basis:
      "a two-point dispersion and no King factor; the Chappuis band's absorption over about 400–850 nm outweighs this scattering by about 10⁵",
  },
  {
    kind: "polarisability",
    species: "HCN",
    polarisabilityM3: 2.59 * CUBIC_ANGSTROM_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 1.27,
    basis:
      "Landolt–Börnstein, 6th ed., I/3 (1951) 509, as NIST's CCCBDB lists it (±5%; its primary, and whether it is electronic, were not read); F_K about 1.05, a linear molecule, not read",
  },
  {
    kind: "polarisability",
    species: "C2H2",
    polarisabilityM3: 3.49 * CUBIC_ANGSTROM_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 1.37,
    basis: `${OLNEY_1997}, 3.487 Å³ (±6%); Loria, Ann. Phys. 334 (1909) 605's dispersion sits 8.5% lower and Mascart 1878's index 7% higher, so neither is used; F_K about 1.06, not read`,
  },
  {
    kind: "polarisability",
    species: "C2H4",
    polarisabilityM3: 4.19 * CUBIC_ANGSTROM_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 1.34,
    basis: `${OLNEY_1997}, 4.188 Å³ (±6%); Loria 1909's dispersion sits 12% lower, so it is not used; F_K about 1.035, not read`,
  },
  {
    kind: "dispersion",
    species: "C2H6",
    dispersion: {
      nMinusOne: (wavelengthNm) => 7.33e-4 * (1 + 9.308e-3 * wavenumberSquaredPerUm2(wavelengthNm)),
      referenceK: ZERO_CELSIUS_K,
      referencePa: STANDARD_ATMOSPHERE_PA,
      compressibility: 1 / 1.014_8,
      source:
        "Loria, Ann. Phys. 334 (1909) 605, Table IX: n − 1 = a(1 + b ÷ λ²) at 0 °C and 760 mm with refractiveindex.info's refit a = 7.330 × 10⁻⁴ (the printed 7.365 × 10⁻⁴ is 0.5% above his own table) and b = 9.308 × 10⁻³ µm²; he reduced his readings at 610–755 mm as an ideal gas, and NIST's ethane has dρ ÷ dp 1.0148 times the ideal near 291 K, so Z is taken as 1 ÷ 1.0148",
      dataFile: undefined,
      measuredNm: [523, 667.7],
    },
    kingFactor: {
      factor: () => 1.006_6,
      source:
        "1 + (2/9)(Δα ÷ ᾱ)² with Δα ÷ ᾱ = 5.2 ÷ 30.2 a.u. at 632.8 nm (Bridge and Buckingham 1966, as van Gisbergen, Snijders and Baerends, J. Chem. Phys. 103 (1995) 9347, Table II, quote it)",
    },
    uncertaintyFactor: 1.06,
    basis:
      "the King factor from a secondary source, and Loria's α 2.2% below Hohm, Chem. Phys. 179 (1994) 533's static 29.54 a.u.",
  },
  {
    kind: "polarisability",
    species: "PH3",
    polarisabilityM3: 4.24 * CUBIC_ANGSTROM_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 1.23,
    basis: `${OLNEY_1997}, 4.24 Å³ (±5%, low confidence); the dielectric value near 4.8 Å³ holds vibrational polarisability`,
  },
  {
    kind: "dispersion",
    species: "CH3OH",
    dispersion: {
      nMinusOne: oneTermNMinusOne(0.064_052, 127.53),
      referenceK: TWENTY_FIVE_CELSIUS_K,
      referencePa: STANDARD_ATMOSPHERE_PA,
      compressibility: 1,
      source:
        "Ramaswamy, Proc. Indian Acad. Sci. A 4 (1936) 675, Table I: n − 1 = 0.064 052 ÷ (127.53 − λ⁻²) at 25 °C and 760 mm, stated as for an ideal gas after his own compressibility correction",
      dataFile: undefined,
      measuredNm: [436, 644],
    },
    kingFactor: noKingFactor("no measured depolarisation was found; F_K lies near 1.005–1.01"),
    uncertaintyFactor: 1.03,
    basis: "no King factor",
  },
  {
    kind: "polarisability",
    species: "H",
    polarisabilityM3: 4.5 * BOHR_CUBED_M3,
    resonanceNm: 110.74,
    uncertaintyFactor: 1.004,
    basis: `9/2 a₀³, exact for an infinitely heavy nucleus (${SCHWERDTFEGER_NAGLE}, whose ¹H value, 4.507 11, scatters 0.3% more); λ_r = 110.74 nm, the Lyman limit 91.1267 nm ÷ √(2c₀ ÷ c₁) of Lee and Kim, MNRAS 347 (2004) 802, Table 1, which holds their exact series to 0.06% over 440–1,000 nm and 0.4% at 300 nm`,
  },
  {
    kind: "polarisability",
    species: "O",
    polarisabilityM3: 5.3 * BOHR_CUBED_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 1.3,
    basis: `5.3 ± 0.2 a.u. (${SCHWERDTFEGER_NAGLE}; Das and Thakkar 1998's CCSD(T) 5.24 ± 0.04, Alpher and White 1959's measured 5.2 ± 0.4); its resonances lie in the far ultraviolet, and the static rule runs 4–20% low at 440 nm`,
  },
  {
    kind: "polarisability",
    species: "Na",
    polarisabilityM3: 162.7 * BOHR_CUBED_M3,
    resonanceNm: undefined,
    uncertaintyFactor: undefined,
    basis: `162.7 ± 0.5 a.u., measured (Ekstrom et al., Phys. Rev. A 51 (1995) 3883; ${SCHWERDTFEGER_NAGLE}); the D lines at 589.16 and 589.76 nm (vacuum; NIST ASD) lie inside 440–680 nm, and the true scattering is 46 times the estimate at 550 nm`,
  },
  {
    kind: "polarisability",
    species: "K",
    polarisabilityM3: 289.7 * BOHR_CUBED_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 13,
    basis: `289.7 ± 0.3 a.u., measured (Gregoire et al., Phys. Rev. A 92 (2015) 052513; ${SCHWERDTFEGER_NAGLE}); the resonance lines at 766.70 and 770.11 nm (vacuum; NIST ASD) put the true scattering at 0.2 times the estimate at 440 nm and 13 times at 680 nm`,
  },
  {
    kind: "polarisability",
    species: "Fe",
    polarisabilityM3: 62 * BOHR_CUBED_M3,
    resonanceNm: 248.4,
    uncertaintyFactor: 1.5,
    basis: `62 ± 4 a.u., computed (${SCHWERDTFEGER_NAGLE}; Pou-Amérigo et al. 1995's MCPF 63.9, Calaminici 2004's 62.65); λ_r = 248.40 nm, its strongest resonance line (vacuum, f = 0.543; NIST ASD), which leaves σ 21–24% low at 440 nm against the sum over its lines, where its 372 and 386 nm lines add`,
  },
  {
    kind: "polarisability",
    species: "Mg",
    polarisabilityM3: 71.2 * BOHR_CUBED_M3,
    resonanceNm: 285.3,
    uncertaintyFactor: 1.02,
    basis: `71.2 ± 0.4 a.u. (${SCHWERDTFEGER_NAGLE}; Thakkar and Lupinetti 2006's CCSD(T) 71.22 ± 0.36); λ_r = 285.30 nm, its resonance line (vacuum, f = 1.80; NIST ASD), which carries 99% of α(0) and matches the sum over its lines to 0.5%`,
  },
  {
    kind: "polarisability",
    species: "Si",
    polarisabilityM3: 37.3 * BOHR_CUBED_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 2.4,
    basis: `37.3 ± 0.7 a.u., computed (Thierfelder et al., Phys. Rev. A 78 (2008) 052506; ${SCHWERDTFEGER_NAGLE}); its 251–253 nm lines (NIST ASD) put the true scattering 1.1–2.2 times above the estimate`,
  },
  {
    kind: "polarisability",
    species: "Ca",
    polarisabilityM3: 160.8 * BOHR_CUBED_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 180,
    basis: `160.8 ± 4.0 a.u. (${SCHWERDTFEGER_NAGLE}; Chattopadhyay et al., Phys. Rev. A 89 (2014) 022506; Porsev and Derevianko 2006 give 157.1 ± 1.3); its resonance line at 422.79 nm (vacuum, f = 1.75; NIST ASD) puts the true scattering 2.5 times above the estimate at 680 nm and 150–170 times at 440 nm`,
  },
  {
    kind: "polarisability",
    species: "Ti",
    polarisabilityM3: 100 * BOHR_CUBED_M3,
    resonanceNm: undefined,
    uncertaintyFactor: undefined,
    basis: `100 ± 10 a.u., computed (${SCHWERDTFEGER_NAGLE}, from Kłos 2005's MRCI; Eustice et al., Phys. Rev. A 107 (2023) L051102, give 100.4 ± 1.8 for the ³F₄ level), where Ma et al., Phys. Rev. A 91 (2015) 010501, measured 63.4 ± 3.4; its ground-level lines at 465.8, 501.6 and 517.5 nm (vacuum; NIST ASD) lie inside 440–680 nm, where α passes through zero near 463 nm and the true scattering is 29–73 times the estimate near the lines`,
  },
  {
    kind: "polarisability",
    species: "SiO",
    polarisabilityM3: 4.5 * CUBIC_ANGSTROM_M3,
    resonanceNm: undefined,
    uncertaintyFactor: 2.2,
    basis: `${CCCBDB_COMPUTED}: B3LYP 4.43 and MP2 4.61 Å³ (aug-cc-pVQZ), 4.5 ± 0.25 Å³ (Maroulis et al., Mol. Phys. 98 (2000) 481, not read); its A–X band near 234 nm may raise the true scattering up to 1.9 times`,
  },
  {
    kind: "polarisability",
    species: "TiO",
    polarisabilityM3: 13.5 * CUBIC_ANGSTROM_M3,
    resonanceNm: undefined,
    uncertaintyFactor: undefined,
    basis: `${CCCBDB_COMPUTED}: B3LYP 12.52, MP2 14.55 and HF 15.0 Å³, 13.5 ± 1.5 Å³, no published value found; its electronic bands lie across the visible, where the estimate is not meaningful`,
  },
  {
    kind: "polarisability",
    species: "VO",
    polarisabilityM3: 10.3 * CUBIC_ANGSTROM_M3,
    resonanceNm: undefined,
    uncertaintyFactor: undefined,
    basis: `${CCCBDB_COMPUTED}: B3LYP 10.75, MP2 8.92 and HF 11.53 Å³, 10.3 ± 1.5 Å³, no published value found; its electronic bands lie in the visible and the near infrared, where the estimate is not meaningful`,
  },
  {
    kind: "polarisability",
    species: "FeH",
    polarisabilityM3: 8.2 * CUBIC_ANGSTROM_M3,
    resonanceNm: undefined,
    uncertaintyFactor: undefined,
    basis: `an analogy, α(Fe) times α(TiH) ÷ α(Ti), 0.89–1.02 with TiH's B3LYP 13.19 and MP2 15.15 Å³ (${CCCBDB_COMPUTED}), 8.2 ± 2.2 Å³, no published value found; its bands' share of α (the Wing–Ford band near 990 nm) is not known`,
  },
];

/**
 * σ = (128π⁵ ÷ 3) α² ÷ λ⁴, m², the Rayleigh cross-section of a molecule of polarisability volume α
 * with no anisotropy (F_K = 1).
 *
 * @remarks
 * It is {@link crossSectionFromDispersionM2}'s form for a dilute gas, whose Lorentz–Lorenz
 * (n² − 1) ÷ (n² + 2) is (4π ÷ 3) N α. A static α leaves out the dispersion, so for a molecule
 * whose resonances lie in the ultraviolet the cross-section runs low towards the violet: for N₂,
 * with the Cuthbertsons' static α and no King factor, by 5% at 700 nm, 7% at 550 nm and 10% at
 * 400 nm ({@link ESTIMATED_RAYLEIGH}).
 *
 * @param polarisabilityM3 - α, m³, finite and positive.
 * @param wavelengthNm - A vacuum wavelength within {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 * @throws RangeError if the wavelength is outside the range, a caller's unit error.
 */
export function polarisabilityCrossSectionM2(
  polarisabilityM3: number,
  wavelengthNm: number,
): number {
  checkWavelength(wavelengthNm);
  const wavelengthM = wavelengthNm * 1e-9;
  const lambdaSquared = wavelengthM * wavelengthM;
  return (
    ((128 / 3) * Math.PI ** 5 * polarisabilityM3 * polarisabilityM3) /
    (lambdaSquared * lambdaSquared)
  );
}

/**
 * ρ, the depolarisation ratio in natural light, from a King factor: 6(F_K − 1) ÷ (3 + 7F_K), the
 * inverse of F_K = (6 + 3ρ) ÷ (6 − 7ρ) (Sneep and Ubachs 2005, eq. 7).
 */
export function depolarisationOfKingFactor(king: number): number {
  return (6 * (king - 1)) / (3 + 7 * king);
}

/** A species' Rayleigh optics with its King factor, which the mixture weights by. */
interface ScatteringSpecies {
  readonly rayleigh: SpeciesRayleigh;
  /** F_K at a vacuum wavelength in nm, checked: 1 for a polarisability estimate and for `none`. */
  readonly kingFactor: (wavelengthNm: number) => number;
}

/** F_K = 1, checking the wavelength as every other optic does. */
function isotropicAt(wavelengthNm: number): number {
  checkWavelength(wavelengthNm);
  return 1;
}

/** 0, a σ or a ρ, checking the wavelength. */
function zeroAt(wavelengthNm: number): number {
  checkWavelength(wavelengthNm);
  return 0;
}

/** {@link GASES}' entry for a key, or `undefined`. */
function measuredGas(species: string): Gas | undefined {
  return GASES.find((gas) => gas === species);
}

/** {@link ESTIMATED_RAYLEIGH} by species. */
const ESTIMATED_BY_SPECIES: ReadonlyMap<string, EstimatedRayleigh> = new Map(
  ESTIMATED_RAYLEIGH.map((row) => [row.species, row]),
);

/** An estimate's uncertainty in words, for its {@link SpeciesRayleigh.source}. */
function uncertaintyOf(estimate: EstimatedRayleigh): string {
  const factor = estimate.uncertaintyFactor;
  return factor === undefined
    ? "no bound known over 440–680 nm"
    : `within a factor of ${factor} either way over 440–680 nm`;
}

/**
 * α at a wavelength, m³: α(0), or α(0) ÷ (1 − (λ_r ÷ λ)²) with one oscillator at λ_r.
 *
 * @throws RangeError if the wavelength is outside {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 */
function polarisabilityAtM3(estimate: PolarisabilityEstimate, wavelengthNm: number): number {
  checkWavelength(wavelengthNm);
  const { polarisabilityM3, resonanceNm } = estimate;
  return resonanceNm === undefined
    ? polarisabilityM3
    : polarisabilityM3 / (1 - (resonanceNm / wavelengthNm) ** 2);
}

/** An estimated row's Rayleigh optics. */
function estimatedSpecies(estimate: EstimatedRayleigh): ScatteringSpecies {
  const { species } = estimate;
  let optics: ScatteringSpecies;
  switch (estimate.kind) {
    case "dispersion": {
      const { dispersion, kingFactor: king } = estimate;
      const kingAt = (wavelengthNm: number): number => {
        checkWavelength(wavelengthNm);
        return king.factor(wavelengthNm);
      };
      optics = {
        rayleigh: {
          species,
          provenance: "estimated",
          crossSectionM2: (wavelengthNm) =>
            crossSectionFromDispersionM2(dispersion, king.factor, wavelengthNm),
          depolarisation: (wavelengthNm) => depolarisationOfKingFactor(kingAt(wavelengthNm)),
          source: `estimated, ${uncertaintyOf(estimate)} (${estimate.basis}): n − 1: ${dispersion.source}; F_K: ${king.source}`,
        },
        kingFactor: kingAt,
      };
      break;
    }
    case "polarisability": {
      const rule =
        estimate.resonanceNm === undefined
          ? "(128π⁵ ÷ 3) α² ÷ λ⁴ with F_K = 1"
          : `(128π⁵ ÷ 3) α(λ)² ÷ λ⁴ with α(λ) = α(0) ÷ (1 − (${estimate.resonanceNm} nm ÷ λ)²) and F_K = 1`;
      optics = {
        rayleigh: {
          species,
          provenance: "estimated",
          crossSectionM2: (wavelengthNm) =>
            polarisabilityCrossSectionM2(polarisabilityAtM3(estimate, wavelengthNm), wavelengthNm),
          depolarisation: zeroAt,
          source: `estimated, ${rule}, ${uncertaintyOf(estimate)}: α from ${estimate.basis}`,
        },
        kingFactor: isotropicAt,
      };
      break;
    }
  }
  return optics;
}

/** A species' Rayleigh optics: measured, estimated or none. */
function scatteringSpecies(species: string): ScatteringSpecies {
  const gas = measuredGas(species);
  if (gas !== undefined) {
    const dispersion = GAS_DISPERSION[gas];
    const king = GAS_KING_FACTOR[gas];
    return {
      rayleigh: {
        species,
        provenance: "measured",
        crossSectionM2: (wavelengthNm) => rayleighCrossSectionM2(gas, wavelengthNm),
        depolarisation: (wavelengthNm) => depolarisationOfKingFactor(kingFactor(gas, wavelengthNm)),
        source: `n − 1: ${dispersion.source}; F_K: ${king.source}`,
      },
      kingFactor: (wavelengthNm) => kingFactor(gas, wavelengthNm),
    };
  }
  const estimate = ESTIMATED_BY_SPECIES.get(species);
  if (estimate !== undefined) {
    return estimatedSpecies(estimate);
  }
  return {
    rayleigh: {
      species,
      provenance: "none",
      crossSectionM2: zeroAt,
      depolarisation: zeroAt,
      source: "none: no Rayleigh optics for this key here",
    },
    kingFactor: isotropicAt,
  };
}

/**
 * A species' Rayleigh scattering by its substance key, with its provenance (decision-composition
 * §1.9).
 *
 * @remarks
 * - `measured` for {@link GASES}: {@link rayleighCrossSectionM2} and {@link kingFactor}'s ρ.
 * - `estimated` for {@link ESTIMATED_RAYLEIGH}'s rows: a measured dispersion with its King factor's
 *   ρ (1 and so ρ = 0 where none was found), or a polarisability with ρ = 0.
 * - `none` for any other key, such as one a newer server sent: σ = 0 and ρ = 0, so it adds nothing
 *   to a mixture, which reports it ({@link molecularMixture}).
 *
 * No key is refused: a species is never dropped, only labelled.
 */
export function rayleighOf(species: string): SpeciesRayleigh {
  return scatteringSpecies(species).rayleigh;
}

/** A species of a mixture whose Rayleigh optics are not measured, for `atmosphereApproximate`. */
export interface RayleighApproximation {
  readonly species: string;
  readonly provenance: Exclude<OpticsProvenance, "measured">;
}

/**
 * A well-mixed gas's Rayleigh scattering per molecule of the mixture, at any wavelength (R08 Design
 * note 4).
 */
export interface MolecularMixture {
  /** σ_mix = Σ xᵢσᵢ, m², at a vacuum wavelength in nm within the range. */
  readonly crossSectionM2: (wavelengthNm: number) => number;
  /**
   * F_mix = Σ xᵢσᵢ ÷ Σ xᵢ(σᵢ ÷ Fᵢ), the σ-weighted King factor, at a vacuum wavelength in nm; 1 for a
   * mixture that scatters nothing.
   */
  readonly kingFactor: (wavelengthNm: number) => number;
  /** ρ_mix = 6(F_mix − 1) ÷ (3 + 7F_mix), at a vacuum wavelength in nm. */
  readonly depolarisation: (wavelengthNm: number) => number;
  /** Each species whose optics are estimated or none, in the mixture's order. */
  readonly approximations: ReadonlyArray<RayleighApproximation>;
}

/**
 * A well-mixed gas's Rayleigh scattering from its fractions, over any keyed species: the
 * number-fraction mixture, F_mix and ρ_mix of R08 Design note 4.
 *
 * @remarks
 * Each species' F_K is 1 + 2(γᵢ ÷ 3ᾱᵢ)² (Sneep and Ubachs 2005, eq. 7) and its σ is proportional
 * to ᾱᵢ² Fᵢ. Molecules of an ideal gas scatter independently, so Σ xᵢᾱᵢ² and Σ xᵢγᵢ² add, and
 * F_mix = Σ xᵢσᵢ ÷ Σ xᵢ(σᵢ ÷ Fᵢ) is exact. Averaging Fᵢ by volume, as Bodhaine et al. 1999's eq. 23
 * does for air, weights each gas by its fraction rather than its scattering: for air the two
 * differ by 0.2% in F and 4% in ρ.
 *
 * @throws RangeError for a fraction that is not finite in [0, 1], a species listed twice, or
 *   fractions that do not sum to 1 within {@link MOLE_FRACTION_SUM_TOLERANCE}.
 */
export function molecularMixture(fractions: GasFractions): MolecularMixture {
  const seen = new Set<string>();
  let total = 0;
  const species: Array<{ readonly fraction: number; readonly optics: ScatteringSpecies }> = [];
  for (const { species: key, moleFraction } of fractions) {
    if (!(Number.isFinite(moleFraction) && moleFraction >= 0 && moleFraction <= 1)) {
      throw new RangeError(`${key}'s mole fraction lies in [0, 1], got ${moleFraction}`);
    }
    if (seen.has(key)) {
      throw new RangeError(`a mixture lists each species once, and ${key} is listed twice`);
    }
    seen.add(key);
    total += moleFraction;
    species.push({ fraction: moleFraction, optics: scatteringSpecies(key) });
  }
  if (!(Math.abs(total - 1) <= MOLE_FRACTION_SUM_TOLERANCE)) {
    throw new RangeError(
      `a mixture's mole fractions sum to 1 within ${MOLE_FRACTION_SUM_TOLERANCE}, got ${total}`,
    );
  }
  const crossSectionM2 = (wavelengthNm: number): number => {
    checkWavelength(wavelengthNm);
    let sum = 0;
    for (const { fraction, optics } of species) {
      sum += fraction * optics.rayleigh.crossSectionM2(wavelengthNm);
    }
    return sum;
  };
  const kingFactorAt = (wavelengthNm: number): number => {
    checkWavelength(wavelengthNm);
    let scattered = 0;
    let isotropicPart = 0;
    for (const { fraction, optics } of species) {
      const sigma = fraction * optics.rayleigh.crossSectionM2(wavelengthNm);
      scattered += sigma;
      isotropicPart += sigma / optics.kingFactor(wavelengthNm);
    }
    return scattered > 0 ? scattered / isotropicPart : 1;
  };
  const approximations: RayleighApproximation[] = [];
  for (const { optics } of species) {
    const { provenance } = optics.rayleigh;
    if (provenance !== "measured") {
      approximations.push({ species: optics.rayleigh.species, provenance });
    }
  }
  return {
    crossSectionM2,
    kingFactor: kingFactorAt,
    depolarisation: (wavelengthNm) => depolarisationOfKingFactor(kingFactorAt(wavelengthNm)),
    approximations,
  };
}

/** A well-mixed gas's one Rayleigh term, and the species it carries by estimate or not at all. */
export interface MolecularTerm {
  /**
   * The term `rayleigh`: the column's density, n_s σ_mix per channel at relative density 1, no
   * absorption, and the Rayleigh phase with ρ_mix per channel.
   */
  readonly term: MediumTerm;
  /** {@link MolecularMixture.approximations}, which R08.T10.a turns into `atmosphereApproximate`. */
  readonly approximations: ReadonlyArray<RayleighApproximation>;
}

/**
 * A well-mixed gas's molecular scattering as one medium term on its column (R08 Design notes 2
 * and 4): {@link molecularMixture} at {@link CHANNEL_WAVELENGTHS_NM}.
 *
 * @remarks
 * The gases are mixed at every height, so the term's density is the column's n ÷ n_s and its
 * coefficient at relative density 1 is n_s σ_mix, n_s the column's ideal-gas number density at the
 * datum. σᵢ belongs to the molecule, at its formula's reference state (the real gas's N_ref), and
 * does not depend on the column's state.
 *
 * @throws RangeError as {@link molecularMixture}.
 */
export function molecularTerm(column: AtmosphereColumn, fractions: GasFractions): MolecularTerm {
  const mixture = molecularMixture(fractions);
  const numberDensity = column.surfaceNumberDensityPerM3;
  const [red, green, blue] = CHANNEL_WAVELENGTHS_NM;
  const scattering: Rgb = [
    numberDensity * mixture.crossSectionM2(red),
    numberDensity * mixture.crossSectionM2(green),
    numberDensity * mixture.crossSectionM2(blue),
  ];
  const depolarisation: Rgb = [
    mixture.depolarisation(red),
    mixture.depolarisation(green),
    mixture.depolarisation(blue),
  ];
  return {
    term: {
      name: "rayleigh",
      density: column.density,
      scattering,
      absorption: [0, 0, 0],
      phase: { kind: "rayleigh", depolarisation },
    },
    approximations: mixture.approximations,
  };
}

/**
 * Refuses a wavelength outside {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 *
 * @throws RangeError if the wavelength is not a number of nm within the range, NaN included.
 */
function checkWavelength(wavelengthNm: number): void {
  const [low, high] = RAYLEIGH_WAVELENGTH_RANGE_NM;
  if (!(wavelengthNm >= low && wavelengthNm <= high)) {
    throw new RangeError(
      `Rayleigh wavelength ${wavelengthNm} nm is outside ${low}–${high} nm (vacuum, in nm)`,
    );
  }
}
