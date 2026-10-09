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
 * CO₂ and Xe, 0.5% for Kr and at most 0.15% for the other gases. The dense-gas fluctuation factor,
 * about 1.06 at Venus's surface, is a recorded omission (Design note 4).
 *
 * Wavelengths are in vacuum, nm. Where one exists, a formula's machine-readable copy is
 * refractiveindex.info's database (Polyanskiy, Sci. Data 11 (2024) 94; CC0), named in
 * {@link Dispersion.dataFile}. The coefficients were checked against the paper as printed
 * wherever the paper could be read, as `rayleigh.test.ts` records. The tutorials' 5.8, 13.5 and 33.1 × 10⁻⁶ m⁻¹ appear nowhere. They
 * are Riley et al. 2004's, which Bruneton and Neyret 2008 took (§2), a pure λ⁻⁴ law with no King
 * factor.
 */

/**
 * Every gas with Rayleigh optics here, by formula. Plan 14's nine come first, in `Gas::ALL`'s order
 * (`planetary::derive::atmosphere::Gas`, `Hydrogen` … `Argon`), then Ne, Kr, Xe and N₂O.
 *
 * @remarks
 * The set is open: a species is added as data, by its formula here and an entry in each of
 * {@link GAS_DISPERSION}, {@link GAS_KING_FACTOR} and {@link GAS_MOLAR_MASS_G_PER_MOL}, each with
 * its sources; the compiler names any record that lacks it. Gases a generator could plausibly
 * produce whose optics have no source checked here yet (CO, SO₂, H₂S, HCN, O₃, C₂H₆, C₂H₄ and
 * C₂H₂) are listed in R08's Risks for the composition audit.
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
   * The vacuum wavelengths the formula was fitted over, nm. Between these and
   * {@link RAYLEIGH_WAVELENGTH_RANGE_NM}'s ends the formula is extrapolated.
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
 * The vacuum wavelengths the formulas are evaluated over, nm: 300 to 1,000. The range holds the
 * bake bins (380–760 nm, Design note 5), the channels and the CIE functions' visible span.
 *
 * @remarks
 * Every formula's poles lie outside it, the nearest being dry air's at 160 nm and CO₂'s infrared
 * term at 4.14 µm. A wavelength outside the range is a caller's unit error, and is refused.
 */
export const RAYLEIGH_WAVELENGTH_RANGE_NM: readonly [number, number] = [300, 1_000];

/**
 * The Boltzmann constant, J K⁻¹, exact in the 2019 SI. It is the sim's
 * `hyperion_base::units::consts::BOLTZMANN_CONSTANT`.
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

/** σ², the squared vacuum wavenumber, µm⁻². */
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
 * σ is the vacuum wavenumber in µm⁻¹ and ν in cm⁻¹. Z is from the NIST Chemistry WebBook (SRD 69,
 * Lemmon, McLinden and Friend), Z = p ÷ (ρRT) from its density at the state.
 * - H₂: Peck and Huang 1977, 10⁶(n − 1) = 14,895.6 ÷ (180.7 − σ²) + 4,903.7 ÷ (92 − σ²), at 0 °C
 *   and 760 torr. Z = 1.000 624.
 * - He: Mansfield and Peck 1969, 10⁵(n − 1) = 1,470.091 ÷ (423.98 − σ²), at 0 °C and 760 torr.
 *   Z = 1.000 532.
 * - H₂O: Ciddor 1996, eq. 3, pure water vapour at 20 °C and 1,333 Pa, 10⁸(n − 1) = c_f (w₀ + w₁σ² +
 *   w₂σ⁴ + w₃σ⁶) with w = 295.235, 2.6422, −0.032 380, 0.004 028 and c_f = 1.022 (as NIST's
 *   Engineering Metrology Toolbox reproduces them). Z = 0.999 237.
 * - CH₄: He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 10,
 *   10⁸(n − 1) = 3,603.09 + 4.403 62 × 10¹⁴ ÷ (1.1741 × 10¹⁰ − ν²), at 288.15 K and 1013.25 hPa,
 *   fitted over 264–671 nm. Design note 4's Sneep and Ubachs 2005 eq. 18, a fit to Hohm 1993's
 *   polarisabilities, runs 13% high in n − 1, against Loria 1909 and Wilmouth and Sayres 2019 (R08's
 *   Risks). He et al. derive n from measured cross-sections through the ideal N = 2.546 899 ×
 *   10¹⁹ cm⁻³, so Z = 1 returns those cross-sections.
 * - NH₃: C. and M. Cuthbertson 1914, n − 1 = 0.032 953 ÷ (90.392 − λ⁻²), λ in µm, at 0 °C and 760
 *   mm, over the cadmium-to-lithium lines (air wavelengths; the difference from vacuum is 2 × 10⁻⁵
 *   of n − 1). Z = 0.984 798; whether the paper reduced its readings with the real or the ideal gas
 *   is unread, ±3% in σ.
 * - N₂: Peck and Khanna 1966's own 15 °C form from 468 nm, 10⁸(n − 1) = 6,497.378 + 3,073,864.9 ÷
 *   (144 − σ²). Sneep and Ubachs's eq. 10 (6,498.2 + …) is the 0 °C form scaled by the ideal gas,
 *   1.5 × 10⁻⁴ higher. Below 468 nm, Bates 1984's ultraviolet branch, 10⁸(n − 1) = 5,677.465 +
 *   318.818 74 × 10¹² ÷ (14.4 × 10⁹ − ν²), an interpolation to Abjean, Mehu and Johannin-Gilles 1970
 *   (Sneep and Ubachs 2005, eq. 11). Both at 15 °C and 101,325 Pa. Z = 0.999 715.
 * - O₂: Křen 2011's refit of Zhang, Lu and Wang 2008's data with others, n − 1 = 1.181 494 × 10⁻⁴ +
 *   9.708 931 × 10⁻³ ÷ (75.4 − σ²), at 20 °C and 101,325 Pa. Zhang's own eq. 20 holds only over
 *   740–860 nm. Křen's comment was not read (Optica, closed); the coefficients are
 *   refractiveindex.info's. Z = 0.999 282.
 * - CO₂: Bideau-Mehu et al. 1973 as Sneep and Ubachs 2005 (eq. 13) correct it, at 0 °C and 760 torr,
 *   not 15 °C (the trap of Design note 4). Sneep and Ubachs's 15 °C coefficients are these times
 *   273.15 ÷ 288.15 to 4 × 10⁻⁵; their printed last numerator, 0.121 814 5 × 10⁻⁴, is 10⁴ too small.
 *   Z = 0.993 265.
 * - Ar: Peck and Fisher 1964's 15 °C form, 10⁷(n − 1) = 643.2135 + 286,060.21 ÷ (144 − σ²), at 760
 *   torr. Z = 0.999 260.
 * - Ne, Kr and Xe: Börzsönyi, Heiner, Kalashnikov, Kovács and Osvay, Appl. Opt. 47 (2008) 4856,
 *   Sellmeier forms at 0 °C and 1,000 mbar fitted over 400–1,000 nm (refractiveindex.info's
 *   copies; the paper was not read, and Xe's C₁ takes the database's correction of the paper's
 *   12.75 × 10⁻⁶ to 12.75 × 10⁻³ µm²). The same paper's Ar, He and N₂ forms agree with Peck and
 *   Fisher, Mansfield and Peck, and Peck and Khanna to 0.3%, and its Ne, Kr and Xe with C. and M.
 *   Cuthbertson's (Proc. R. Soc. A 135 (1932) 40 for Ne, 84 (1910) 13 for Kr and Xe) to 0.5%.
 *   Z = 1.000 483, 0.997 282 and 0.993 245.
 * - N₂O: He et al. 2021, eq. 9, 10⁸(n − 1) = 22,095 + 1.662 91 × 10¹⁴ ÷ (6.752 26 × 10⁹ − ν²), at
 *   288.15 K and 1013.25 hPa, fitted over 307–725 nm, derived like their CH₄ through the ideal N,
 *   so Z = 1.
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
 * 10⁸(n − 1) = 8,060.51 + 2,480,990 ÷ (132.274 − σ²) + 17,455.7 ÷ (39.329 57 − σ²), σ the vacuum
 * wavenumber in µm⁻¹, valid down to about 230 nm (the paper's abstract), as Bodhaine et al., J.
 * Atmos. Oceanic Technol. 16 (1999) 1854, eq. 4, quote it. Peck and Reeder's standard air carries
 * 330 ppm of CO₂ (Bodhaine §1); Bodhaine applies the formula as 300 ppm air, as R05's `earth.ts`
 * and {@link dryAirKingFactor} do, a 3 × 10⁻⁵ difference in σ. refractiveindex.info's copy
 * (`other/mixed gases/air/nk/Peck.yml`) has the same coefficients but labels the air with 450 ppm,
 * Ciddor's later standard. Z = 0.999 59 is CIPM-2007's for dry air at the state (Picard, Davis,
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
  compressibility: 0.999_59,
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
 * - N₂: 1.034 + 3.17 × 10⁻⁴ λ⁻², and O₂: 1.096 + 1.385 × 10⁻³ λ⁻² + 1.448 × 10⁻⁴ λ⁻⁴, λ in µm (Bates,
 *   Planet. Space Sci. 32 (1984) 785, as Bodhaine et al. 1999, eqs. 5 and 6, quote them).
 * - CO₂: 1.1364 + 25.3 × 10⁻¹² ν², ν in cm⁻¹ (Sneep and Ubachs 2005, eq. 14, fitted to Alms et al.
 *   1975's depolarisation over about 476–625 nm).
 * - N₂O: ρₚ = 0.0577 + 11.8 × 10⁻¹² ν² (Sneep and Ubachs 2005, eq. 19, Alms et al. 1975), 1.225 at
 *   532.2 nm as their Table 2 has it.
 * - Ar, He, Ne, Kr and Xe: 1, atoms; CH₄: 1, a spherical top (Sneep and Ubachs 2005, §5.2: below
 *   1.0007).
 * - H₂O: 1.001, from Murphy, J. Chem. Phys. 67 (1977) 5877's ρₚ = (3.0 ± 1.4) × 10⁻⁴ at 514.5 nm:
 *   1.0010 ± 0.0005.
 * - H₂: 1.0312 + 3.09 × 10⁻⁴ λ⁻², λ in µm: 1 + (2/9)(γ ÷ ᾱ)² for the v = 0, J = 0 averages of Raj,
 *   Hamaguchi and Witek, J. Chem. Phys. 148 (2018) 104308's ab initio α∥(r, ω) and α⊥(r, ω), fitted
 *   here over 380–800 nm to 2 × 10⁻⁵ (1.0322 at 550 nm). Design note 4's "Hohm 1993" (Mol. Phys.
 *   78, 929) is a hydrocarbon mean-polarisability paper; Hohm 1994 (Chem. Phys. 179, 533) holds
 *   the anisotropies and was not read.
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
 * abridged), copied so that the client's column and the sim's agree. Ne (20.180), Kr (83.798) and
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
 * The Rayleigh cross-section of a molecule from a dispersion and a King factor, m²: 24π³ ÷ (λ⁴
 * N_ref²) × ((n² − 1) ÷ (n² + 2))² × F_K, with N_ref the dispersion's reference density.
 *
 * @param king - The King factor at the same wavelength, at least 1.
 * @param wavelengthNm - A vacuum wavelength within {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 * @throws RangeError if the wavelength is outside the range, a caller's unit error.
 */
export function crossSectionFromDispersionM2(
  dispersion: Dispersion,
  king: number,
  wavelengthNm: number,
): number {
  checkWavelength(wavelengthNm);
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
 * {@link GAS_DISPERSION} at its reference state with its {@link kingFactor}.
 *
 * @param wavelengthNm - A vacuum wavelength within {@link RAYLEIGH_WAVELENGTH_RANGE_NM}.
 * @throws RangeError if the wavelength is outside the range, a caller's unit error.
 */
export function rayleighCrossSectionM2(gas: Gas, wavelengthNm: number): number {
  return crossSectionFromDispersionM2(
    GAS_DISPERSION[gas],
    kingFactor(gas, wavelengthNm),
    wavelengthNm,
  );
}

/** @throws RangeError if a wavelength is not a number of nm within the range. */
function checkWavelength(wavelengthNm: number): void {
  const [low, high] = RAYLEIGH_WAVELENGTH_RANGE_NM;
  if (!(wavelengthNm >= low && wavelengthNm <= high)) {
    throw new RangeError(
      `Rayleigh wavelength ${wavelengthNm} nm is outside ${low}–${high} nm (vacuum, in nm)`,
    );
  }
}
