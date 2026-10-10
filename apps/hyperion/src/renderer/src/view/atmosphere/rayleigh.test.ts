import { describe, expect, it } from "vitest";

import { type AtmosphereColumn, hydrostaticColumn } from "./column";
import { CHANNEL_WAVELENGTHS_NM } from "./medium";
import {
  BOLTZMANN_J_PER_K,
  crossSectionFromDispersionM2,
  depolarisationOfKingFactor,
  type DispersionEstimate,
  DRY_AIR_DISPERSION,
  dryAirKingFactor,
  ESTIMATED_RAYLEIGH,
  type Gas,
  GAS_DISPERSION,
  GAS_KING_FACTOR,
  GAS_MOLAR_MASS_G_PER_MOL,
  type GasFractions,
  GASES,
  kingFactor,
  molecularMixture,
  molecularTerm,
  polarisabilityCrossSectionM2,
  type PolarisabilityEstimate,
  RAYLEIGH_WAVELENGTH_RANGE_NM,
  rayleighCrossSectionM2,
  rayleighOf,
  referenceNumberDensityPerM3,
} from "./rayleigh";

/** Square centimetres in one square metre. */
const CM2_PER_M2 = 1e4;

/** The relative difference of a value from a reference. */
function relative(value: number, reference: number): number {
  return Math.abs(value / reference - 1);
}

/** Wavelengths from `from` to `to` nm inclusive, `step` apart. */
function wavelengthsNm(from: number, to: number, step: number): number[] {
  const out: number[] = [];
  for (let nm = from; nm <= to + 1e-9; nm += step) {
    out.push(nm);
  }
  return out;
}

/** Dry air's cross-section, m²: Peck and Reeder's refractivity and Bodhaine's mixed King factor. */
function dryAirCrossSectionM2(wavelengthNm: number): number {
  return crossSectionFromDispersionM2(DRY_AIR_DISPERSION, dryAirKingFactor, wavelengthNm);
}

/**
 * Dry air by volume, percent: N₂ 78.084, O₂ 20.946, Ar 0.934 (Bodhaine et al. 1999, eq. 23), and
 * Peck and Reeder's 330 ppm of CO₂ (Bodhaine §1).
 */
const DRY_AIR_PERCENT: ReadonlyArray<readonly [Gas, number]> = [
  ["N2", 78.084],
  ["O2", 20.946],
  ["Ar", 0.934],
  ["CO2", 0.033],
];

/** Dry air as fractions summing to 1: {@link DRY_AIR_PERCENT} over its 99.997. */
const DRY_AIR: GasFractions = DRY_AIR_PERCENT.map(([species, percent]) => ({
  species,
  moleFraction: percent / DRY_AIR_PERCENT.reduce((sum, [, p]) => sum + p, 0),
}));

/** Sea level's real number density, m⁻³: 288.15 K and 101,325 Pa (US Standard Atmosphere 1976). */
const SEA_LEVEL_PER_M3 = referenceNumberDensityPerM3(DRY_AIR_DISPERSION);

/** The channels with their index: 680, 550 and 440 nm. */
const CHANNELS = [...CHANNEL_WAVELENGTHS_NM.entries()];

/** Earth's dry-air coefficients at 680, 550 and 440 nm, m⁻¹ (Design note 4; R05's `earth.ts`). */
const EARTH_RAYLEIGH_PER_M = [4.848e-6, 11.487e-6, 28.71e-6] as const;

/**
 * The tutorials' coefficients at 680, 550 and 440 nm, m⁻¹: Bruneton 2017's `kRayleigh` (5.802,
 * 13.558, 33.1 × 10⁻⁶), the 5.8, 13.5 and 33.1 of Riley et al. 2004 that Bruneton and Neyret 2008
 * (§2) take.
 */
const TUTORIAL_RAYLEIGH_PER_M = [5.802e-6, 13.558e-6, 33.1e-6] as const;

/** Sneep and Ubachs 2005's laser line, 18,788.4 cm⁻¹, nm (vacuum). */
const SNEEP_NM = 1e7 / 18_788.4;

/** A gas's cross-section at Sneep and Ubachs's line, cm². */
function sneepCrossSectionCm2(gas: Gas): number {
  return rayleighCrossSectionM2(gas, SNEEP_NM) * CM2_PER_M2;
}

describe("the gases", () => {
  it("start with plan 14's nine, in the sim's Gas::ALL order", () => {
    expect(GASES.slice(0, 9)).toEqual(["H2", "He", "H2O", "CH4", "NH3", "N2", "O2", "CO2", "Ar"]);
  });

  it.each([
    ["GAS_DISPERSION", Object.keys(GAS_DISPERSION)],
    ["GAS_KING_FACTOR", Object.keys(GAS_KING_FACTOR)],
    ["GAS_MOLAR_MASS_G_PER_MOL", Object.keys(GAS_MOLAR_MASS_G_PER_MOL)],
  ])("each have one entry in %s, and nothing else does", (_name, keys) => {
    expect(keys.toSorted()).toEqual(GASES.toSorted());
  });

  it("carry the sim's molar masses for plan 14's nine (`Gas::molar_mass_g_per_mol`)", () => {
    // crates/hyperion-sim/src/planetary/derive/atmosphere.rs, IUPAC 2021 abridged.
    expect(GAS_MOLAR_MASS_G_PER_MOL).toMatchObject({
      H2: 2.016,
      He: 4.003,
      H2O: 18.015,
      CH4: 16.043,
      NH3: 17.031,
      N2: 28.014,
      O2: 31.998,
      CO2: 44.009,
      Ar: 39.95,
    });
  });

  it("build N₂O's molar mass from IUPAC 2021's abridged atomic weights", () => {
    const nitrogen = 14.007;
    const oxygen = 15.999;
    expect(GAS_MOLAR_MASS_G_PER_MOL.N2O).toBeCloseTo(2 * nitrogen + oxygen, 9);
  });
});

describe("each formula's reference state", () => {
  /**
   * The state each source states, and Z there: the NIST Chemistry WebBook's density (SRD 69) as
   * p ÷ (ρRT); 1 for He et al. 2021's CH₄ and N₂O, whose n is defined through the ideal N.
   */
  const STATES: ReadonlyArray<readonly [Gas, number, number, number]> = [
    ["H2", 273.15, 101_325, 1.000_624], // Peck and Huang 1977: 0 °C, 760 torr
    ["He", 273.15, 101_325, 1.000_532], // Mansfield and Peck 1969: 0 °C, 760 torr
    ["H2O", 293.15, 1_333, 0.999_237], // Ciddor 1996, eq. 3: 20 °C, 1,333 Pa
    ["CH4", 288.15, 101_325, 1], // He et al. 2021: 288.15 K, 1013.25 hPa
    ["NH3", 273.15, 101_325, 0.984_798], // Cuthbertson and Cuthbertson 1914: 0 °C, 760 mm
    ["N2", 288.15, 101_325, 0.999_715], // Peck and Khanna 1966: 15 °C, 760 torr
    ["O2", 293.15, 101_325, 0.999_282], // Zhang et al. 2008 and Křen 2011: 20 °C, 101,325 Pa
    ["CO2", 273.15, 101_325, 0.993_265], // Bideau-Mehu et al. 1973: 0 °C, 760 torr
    ["Ar", 288.15, 101_325, 0.999_26], // Peck and Fisher 1964: 15 °C, 760 torr
    ["Ne", 273.15, 100_000, 1.000_483], // Börzsönyi et al. 2008: 0 °C, 1,000 mbar
    ["Kr", 273.15, 100_000, 0.997_282], // Börzsönyi et al. 2008: 0 °C, 1,000 mbar
    ["Xe", 273.15, 100_000, 0.993_245], // Börzsönyi et al. 2008: 0 °C, 1,000 mbar
    ["N2O", 288.15, 101_325, 1], // He et al. 2021: 288.15 K, 1013.25 hPa
  ];

  it("lists every gas", () => {
    expect(STATES.map(([gas]) => gas)).toEqual(GASES);
  });

  it.each(STATES)("pins %s's (T, p, Z) to its source", (gas, kelvin, pascals, z) => {
    const { referenceK, referencePa, compressibility } = GAS_DISPERSION[gas];
    expect([referenceK, referencePa, compressibility]).toEqual([kelvin, pascals, z]);
  });

  it.each(STATES)("computes %s's N_ref as p ÷ (Z k_B T)", (gas, kelvin, pascals, z) => {
    const perM3 = pascals / (z * BOLTZMANN_J_PER_K * kelvin);
    expect(relative(referenceNumberDensityPerM3(GAS_DISPERSION[gas]), perM3)).toBeLessThan(1e-12);
  });

  it("gives the ideal N = 2.546 899 × 10²⁵ m⁻³ for CH₄, as He et al. 2021's Table 1 states", () => {
    // The 7 × 10⁻⁶ between them is CODATA 1986's k_B, 1.380 658 × 10⁻²³ J K⁻¹, which the
    // literature's N carries.
    expect(relative(referenceNumberDensityPerM3(GAS_DISPERSION.CH4), 2.546_899e25)).toBeLessThan(
      1e-5,
    );
  });
});

describe("each dispersion formula, against its source", () => {
  it("reproduces Mansfield and Peck's He index, 3.4950 × 10⁻⁵ at 5462.258 Å, 0 °C", () => {
    expect(relative(GAS_DISPERSION.He.nMinusOne(546.225_8), 3.495e-5)).toBeLessThan(2e-5);
  });

  it("reproduces Peck and Huang's H₂ index, 139.304 × 10⁻⁶ at 0.546 225 2 µm, 0 °C", () => {
    expect(relative(GAS_DISPERSION.H2.nMinusOne(546.225_2), 139.304e-6)).toBeLessThan(3e-5);
  });

  // Table 2's (n − 1) × 10⁶ at 18,788.4 cm⁻¹, "at 15 °C and 101 325 Pa", scaled by them as an
  // ideal gas (their §4.3).
  it.each([
    ["Ar", 268],
    ["N2", 284],
    ["CO2", 427],
  ] as const)("reproduces Sneep and Ubachs's Table 2 index at 15 °C for %s", (gas, perMillion) => {
    const dispersion = GAS_DISPERSION[gas];
    const at15 = dispersion.nMinusOne(SNEEP_NM) * (dispersion.referenceK / 288.15);
    expect(Math.abs(at15 * 1e6 - perMillion)).toBeLessThan(0.5);
  });

  it("joins N₂'s two branches at 468 nm to 10⁻⁶", () => {
    const n2 = GAS_DISPERSION.N2.nMinusOne;
    expect(relative(n2(468), n2(468 - 1e-9))).toBeLessThan(1e-6);
  });

  it("matches Zhang et al. 2008's own O₂ formula over its 740–860 nm to 0.1%", () => {
    // Zhang, Lu and Wang 2008, Table 1 (eq. 20): 10⁸(n − 1) = 15,532.45 + 456,402.97 ÷ (50.0 −
    // λ⁻²), at 20 °C and 101,325 Pa.
    for (const nm of wavelengthsNm(740, 860, 5)) {
      const zhang = (15_532.45 + 456_402.97 / (50 - (1_000 / nm) ** 2)) * 1e-8;
      expect(relative(GAS_DISPERSION.O2.nMinusOne(nm), zhang)).toBeLessThan(1e-3);
    }
  });

  it("matches Bates 1984's O₂ formula over 300–546 nm to 0.2%", () => {
    // Bates 1984 as Sneep and Ubachs 2005, eq. 23, print it: 10⁸(n − 1) = 20,564.8 + 24.808 99 ×
    // 10¹² ÷ (4.09 × 10⁹ − ν²), at 0 °C (He et al. 2021, Table 1).
    for (const nm of wavelengthsNm(300, 546, 2)) {
      const bates = (20_564.8 + 24.808_99e12 / (4.09e9 - (1e7 / nm) ** 2)) * 1e-8;
      const at0 = GAS_DISPERSION.O2.nMinusOne(nm) * (293.15 / 273.15);
      expect(relative(at0, bates)).toBeLessThan(2e-3);
    }
  });

  it("matches Loria 1909's CH₄ over his 529–658 nm to 0.5%", () => {
    // Loria 1909 (refractiveindex.info, organic/CH4 - methane/nk/Loria.yml): n = 1.000 426 07 +
    // 6.139 6687 × 10⁻⁶ λ⁻², λ in µm, at 0 °C and 760 torr.
    for (const nm of wavelengthsNm(530, 658, 4)) {
      const loria = 4.260_7e-4 + 6.139_668_7e-6 * (1_000 / nm) ** 2;
      const at0 = GAS_DISPERSION.CH4.nMinusOne(nm) * (288.15 / 273.15);
      expect(relative(at0, loria)).toBeLessThan(5e-3);
    }
  });

  it("is not Sneep and Ubachs's CH₄ eq. 18, which runs over 10% high", () => {
    // Sneep and Ubachs 2005, eq. 18: 10⁸(n − 1) = 46,662 + 4.02 × 10⁻⁶ ν², at 15 °C.
    const nm = 550;
    const sneep = (46_662 + 4.02e-6 * (1e7 / nm) ** 2) * 1e-8;
    expect(sneep / GAS_DISPERSION.CH4.nMinusOne(nm) - 1).toBeGreaterThan(0.1);
  });

  // refractiveindex.info's main/{Ne,Kr,Xe}/nk/Cuthbertson.yml, n − 1 = A ÷ (B − λ⁻²), λ in µm, at
  // 0 °C and 760 torr: Ne (Proc. R. Soc. A 135 (1932) 40) over 289–546 nm, Kr and Xe (Proc. R. Soc.
  // A 84 (1910) 13) over 480–671 nm, scaled to Börzsönyi's 1,000 mbar. Cuthbertson's Kr sits a flat
  // 0.47% below Börzsönyi's, where Koch 1949's (Leonard 1974) agrees with Börzsönyi to 0.1%.
  it.each([
    ["Ne", 0.029_073_88, 435.713_76, 400, 546, 1e-3],
    ["Kr", 0.059_467_251, 142.062_05, 480, 670, 5e-3],
    ["Xe", 0.068_104_197, 99.892_276, 480, 670, 3e-3],
  ] as const)(
    "matches C. and M. Cuthbertson's %s over the span both cover",
    (gas, a, b, fromNm, toNm, tolerance) => {
      for (const nm of wavelengthsNm(fromNm, toNm, 2)) {
        const theirs = (a / (b - (1_000 / nm) ** 2)) * (100_000 / 101_325);
        expect(relative(GAS_DISPERSION[gas].nMinusOne(nm), theirs)).toBeLessThan(tolerance);
      }
    },
  );

  it("matches Sneep and Ubachs's N₂O eq. 20 (Alms et al. 1975) over 458–647 nm to 1%", () => {
    // Sneep and Ubachs 2005, eq. 20: 10⁸(n − 1) = 46,890 + 4.12 × 10⁻⁶ ν², at 15 °C.
    for (const nm of wavelengthsNm(458, 647, 3)) {
      const sneep = (46_890 + 4.12e-6 * (1e7 / nm) ** 2) * 1e-8;
      expect(relative(GAS_DISPERSION.N2O.nMinusOne(nm), sneep)).toBeLessThan(0.01);
    }
  });
});

describe("the King factors", () => {
  // Sneep and Ubachs 2005, Table 2, F_k at 18,788.4 cm⁻¹.
  it.each([
    ["Ar", 1],
    ["N2", 1.035],
    ["CO2", 1.145],
    ["CH4", 1],
    ["N2O", 1.225],
  ] as const)("gives %s's as Sneep and Ubachs's Table 2 does at 532.2 nm", (gas, printed) => {
    expect(Math.abs(kingFactor(gas, SNEEP_NM) - printed)).toBeLessThan(5e-4);
  });

  it("gives H₂'s 1.0322 at 550 nm, from Raj, Hamaguchi and Witek 2018's anisotropy", () => {
    expect(Math.abs(kingFactor("H2", 550) - 1.032_2)).toBeLessThan(1e-4);
  });

  it("gives H₂O's 1.001 from Murphy 1977's depolarisation, (3 + 6ρ) ÷ (3 − 4ρ)", () => {
    const rho = 3e-4;
    expect(Math.abs(kingFactor("H2O", 550) - (3 + 6 * rho) / (3 - 4 * rho))).toBeLessThan(1e-5);
  });
});

describe("the CO₂ trap (Design note 4)", () => {
  const misread = { ...GAS_DISPERSION.CO2, referenceK: 288.15 };

  it.each(CHANNELS)(
    "raises σ at channel %i by at least 10% when Bideau-Mehu's 0 °C formula is read at 288.15 K",
    (_channel, nm) => {
      const wrong = crossSectionFromDispersionM2(misread, GAS_KING_FACTOR.CO2.factor, nm);
      expect(wrong / rayleighCrossSectionM2("CO2", nm) - 1).toBeGreaterThan(0.1);
    },
  );
});

describe("dry air, the Earth check", () => {
  it("recomputes Bucholtz 1995's 4.51 × 10⁻²⁷ cm² at 550 nm to 1%", () => {
    expect(relative(dryAirCrossSectionM2(550) * CM2_PER_M2, 4.51e-27)).toBeLessThan(0.01);
  });

  it("recomputes Bodhaine et al. 1999's Table 3, 4.5105 × 10⁻²⁷ cm² at 550 nm, to 0.2%", () => {
    expect(relative(dryAirCrossSectionM2(550) * CM2_PER_M2, 4.510_5e-27)).toBeLessThan(2e-3);
  });

  it.each(CHANNELS)(
    "gives Earth's coefficient at channel %i (4.85, 11.5, 28.7 × 10⁻⁶ m⁻¹) to 1%",
    (channel, nm) => {
      const reference = EARTH_RAYLEIGH_PER_M[channel] ?? Number.NaN;
      expect(relative(dryAirCrossSectionM2(nm) * SEA_LEVEL_PER_M3, reference)).toBeLessThan(0.01);
    },
  );

  it("is what the per-gas route gives for dry air's composition, to 0.1% over 380–760 nm", () => {
    for (const nm of wavelengthsNm(380, 760, 5)) {
      const mixed = molecularMixture(DRY_AIR).crossSectionM2(nm);
      expect(relative(mixed, dryAirCrossSectionM2(nm))).toBeLessThan(1e-3);
    }
  });
});

describe("the ab initio cross-checks", () => {
  it("puts H₂ above Dalgarno and Williams 1962, which runs low, by under 8%", () => {
    // Dalgarno and Williams, ApJ 136 (1962) 690, eq. 3: σ = 8.14 × 10⁻¹³ λ⁻⁴ + 1.28 × 10⁻⁶ λ⁻⁶ +
    // 1.61 λ⁻⁸ cm², λ in Å.
    for (const nm of wavelengthsNm(380, 760, 20)) {
      const angstrom = nm * 10;
      const dalgarnoCm2 = 8.14e-13 / angstrom ** 4 + 1.28e-6 / angstrom ** 6 + 1.61 / angstrom ** 8;
      const excess = (rayleighCrossSectionM2("H2", nm) * CM2_PER_M2) / dalgarnoCm2 - 1;
      expect(excess).toBeGreaterThan(0);
      expect(excess).toBeLessThan(0.08);
    }
  });

  it("puts He within 1% of Dalgarno's cross-section", () => {
    // Kurucz, SAO Spec. Rep. 309 (1970), §5.8, "from Dalgarno (1962)": σ = 5.484 × 10⁻¹⁴ λ⁻⁴
    // [1 + 2.44 × 10⁵ λ⁻² + 5.94 × 10¹⁰ ÷ (λ²(λ² − 2.90 × 10⁵))]² cm², λ in Å (the report's
    // "5.94E−10" is a misprint for ATLAS's 5.94E10). Chan and Dalgarno 1965 was not read.
    for (const nm of wavelengthsNm(380, 760, 20)) {
      const a2 = (nm * 10) ** 2;
      const dalgarnoCm2 =
        (5.484e-14 / (a2 * a2)) * (1 + 2.44e5 / a2 + 5.94e10 / (a2 * (a2 - 2.9e5))) ** 2;
      expect(relative(rayleighCrossSectionM2("He", nm) * CM2_PER_M2, dalgarnoCm2)).toBeLessThan(
        0.01,
      );
    }
  });
});

describe("Sneep and Ubachs 2005's measurements at 532.2 nm (their Table 2)", () => {
  // Measured and n-based cross-sections, cm², with the measurement's 1σ.
  const TABLE_2 = [
    ["Ar", 4.45e-27, 0.3e-27, 4.56e-27],
    ["N2", 5.1e-27, 0.24e-27, 5.3e-27],
    ["CO2", 12.4e-27, 0.8e-27, 13.29e-27],
  ] as const;

  it.each(TABLE_2)(
    "pins %s to Sneep and Ubachs's n-based value, which takes the ideal N, times Z²",
    (gas, _measuredCm2, _errorCm2, nBasedCm2) => {
      const z = GAS_DISPERSION[gas].compressibility;
      expect(relative(sneepCrossSectionCm2(gas), nBasedCm2 * z * z)).toBeLessThan(2e-3);
    },
  );

  // CO₂'s formula runs 6% above its measurement, 0.9σ (the plan's "may, by 7%").
  it.each(TABLE_2)(
    "agrees with their %s measurement within its error",
    (gas, measuredCm2, errorCm2) => {
      expect(Math.abs(sneepCrossSectionCm2(gas) - measuredCm2)).toBeLessThan(errorCm2);
    },
  );

  it("agrees with their O₂ value, (4.50 ± 0.15) × 10⁻²⁷ cm² from a three-component fit, within its error", () => {
    expect(Math.abs(sneepCrossSectionCm2("O2") - 4.5e-27)).toBeLessThan(0.15e-27);
  });

  it("puts CH₄'s scattering below their measured extinction, which carries CH₄'s absorption", () => {
    // 12.47 ± 0.23 is a cavity ring-down extinction; He et al. 2021 (§3.4) find it agrees with
    // their extinction, which absorption dominates over parts of 400–725 nm. Sneep and Ubachs's
    // own n-based 14.69 is 18% above it.
    expect(sneepCrossSectionCm2("CH4")).toBeLessThan(12.47e-27 - 2 * 0.23e-27);
  });

  it("puts CH₄'s scattering within 15% of that extinction", () => {
    expect(sneepCrossSectionCm2("CH4")).toBeGreaterThan(0.85 * 12.47e-27);
  });
});

describe("a Venus-class column (Design note 4)", () => {
  /** The atomic mass constant, kg (CODATA 2022; the sim's `ATOMIC_MASS_CONSTANT_KG`). */
  const ATOMIC_MASS_KG = 1.660_539_068_92e-27;
  /** CO₂ molecules per m² over 92 bar at 8.87 m s⁻² (NASA's Venus fact sheet). */
  const COLUMN_PER_M2 = 9.2e6 / (GAS_MOLAR_MASS_G_PER_MOL.CO2 * ATOMIC_MASS_KG * 8.87);

  it("gives a 92-bar CO₂ column τ_R(550 nm) of 16 ± 1", () => {
    expect(Math.abs(rayleighCrossSectionM2("CO2", 550) * COLUMN_PER_M2 - 16)).toBeLessThan(1);
  });

  it("gives it about 41 at 440 nm, the blue channel", () => {
    expect(Math.abs(rayleighCrossSectionM2("CO2", 440) * COLUMN_PER_M2 - 41)).toBeLessThan(1);
  });
});

describe("the tutorial set", () => {
  it.each(CHANNELS)(
    "is 15–20% above Earth's coefficient from the formulas at channel %i",
    (channel, nm) => {
      const tutorial = TUTORIAL_RAYLEIGH_PER_M[channel] ?? Number.NaN;
      const excess = tutorial / (dryAirCrossSectionM2(nm) * SEA_LEVEL_PER_M3) - 1;
      expect(excess).toBeGreaterThan(0.15);
      expect(excess).toBeLessThan(0.2);
    },
  );

  it.each(GASES)("is equalled by %s at sea level's density in no channel, to 1%", (gas) => {
    for (const [channel, nm] of CHANNELS) {
      const tutorial = TUTORIAL_RAYLEIGH_PER_M[channel] ?? Number.NaN;
      const beta = rayleighCrossSectionM2(gas, nm) * SEA_LEVEL_PER_M3;
      expect(relative(beta, tutorial)).toBeGreaterThan(0.01);
    }
  });
});

describe("every formula over the wavelength range", () => {
  const [low, high] = RAYLEIGH_WAVELENGTH_RANGE_NM;
  const span = wavelengthsNm(low, high, 1);
  /** Consecutive (shorter, longer) wavelengths 1 nm apart across the range. */
  const steps = span.slice(1).map((longer, i) => [span[i] ?? Number.NaN, longer] as const);

  it.each(GASES)("keeps %s's n − 1 positive", (gas) => {
    const { nMinusOne } = GAS_DISPERSION[gas];
    expect(span.every((nm) => nMinusOne(nm) > 0)).toBe(true);
  });

  it.each(GASES)("lowers %s's n − 1 as the wavelength grows (normal dispersion)", (gas) => {
    const { nMinusOne } = GAS_DISPERSION[gas];
    expect(steps.every(([shorter, longer]) => nMinusOne(longer) < nMinusOne(shorter))).toBe(true);
  });

  it.each(GASES)("keeps %s's King factor at least 1", (gas) => {
    expect(span.every((nm) => kingFactor(gas, nm) >= 1)).toBe(true);
  });

  it.each(GASES)("keeps %s's King factor from rising with the wavelength", (gas) => {
    const king = (nm: number): number => kingFactor(gas, nm);
    expect(steps.every(([shorter, longer]) => king(longer) <= king(shorter))).toBe(true);
  });

  it.each(GASES)("lowers %s's cross-section as the wavelength grows", (gas) => {
    const sigma = (nm: number): number => rayleighCrossSectionM2(gas, nm);
    expect(steps.every(([shorter, longer]) => sigma(longer) < sigma(shorter))).toBe(true);
  });

  /** Wavelengths a caller in µm, in Å or with no number at all would pass. */
  const UNIT_ERRORS = [0.55, 5_500, Number.NaN];

  it.each(UNIT_ERRORS)("refuses %s nm in rayleighCrossSectionM2, a unit error", (nm) => {
    expect(() => rayleighCrossSectionM2("N2", nm)).toThrow(RangeError);
  });

  it.each(UNIT_ERRORS)("refuses %s nm in kingFactor, a unit error", (nm) => {
    expect(() => kingFactor("O2", nm)).toThrow(RangeError);
  });

  it.each(UNIT_ERRORS)("refuses %s nm in crossSectionFromDispersionM2, a unit error", (nm) => {
    expect(() => crossSectionFromDispersionM2(DRY_AIR_DISPERSION, () => 1, nm)).toThrow(RangeError);
  });

  it.each([low, high])("accepts the range's end, %i nm", (nm) => {
    expect(rayleighCrossSectionM2("N2", nm)).toBeGreaterThan(0);
  });

  it.each(GASES)("fits %s's formula over a span that reaches into the bake bins", (gas) => {
    const [fromNm, toNm] = GAS_DISPERSION[gas].measuredNm;
    expect(fromNm < toNm && fromNm < 760 && toNm > 380).toBe(true);
  });
});

/** Sea level's column: 288.15 K isothermal at 101,325 Pa, μ 28.97, WGS 84's g_ref and R. */
const SEA_LEVEL_COLUMN: AtmosphereColumn = hydrostaticColumn({
  surfacePa: 101_325,
  temperature: { kind: "isothermal", temperatureK: 288.15 },
  meanMolarMassGPerMol: 28.97,
  referenceGravityMS2: 9.806_2,
  referenceRadiusM: 6_371_000,
});

/** The estimated rows from a measured dispersion. */
const DISPERSION_ROWS = ESTIMATED_RAYLEIGH.filter(
  (row): row is DispersionEstimate => row.kind === "dispersion",
);

/** The estimated rows from a polarisability. */
const POLARISABILITY_ROWS = ESTIMATED_RAYLEIGH.filter(
  (row): row is PolarisabilityEstimate => row.kind === "polarisability",
);

/** The polarisability rows with no oscillator: the plan's static rule. */
const STATIC_ROWS = POLARISABILITY_ROWS.filter((row) => row.resonanceNm === undefined);

/** One species alone. */
function alone(species: string): GasFractions {
  return [{ species, moleFraction: 1 }];
}

/** A key no registry holds, as a newer server might send. */
const UNKNOWN = "Xx9";

/** A term's scattering at a channel, m⁻¹. */
function scatteringAt(fractions: GasFractions, channel: number): number {
  return molecularTerm(SEA_LEVEL_COLUMN, fractions).term.scattering[channel] ?? Number.NaN;
}

describe("rayleighOf", () => {
  it.each(GASES)("calls %s's optics measured", (gas) => {
    expect(rayleighOf(gas).provenance).toBe("measured");
  });

  it.each(GASES)("gives %s its measured cross-section", (gas) => {
    const optics = rayleighOf(gas);
    for (const [, nm] of CHANNELS) {
      expect(optics.crossSectionM2(nm)).toBe(rayleighCrossSectionM2(gas, nm));
    }
  });

  it.each(GASES)("gives %s its King factor's ρ", (gas) => {
    const optics = rayleighOf(gas);
    for (const [, nm] of CHANNELS) {
      expect(optics.depolarisation(nm)).toBe(depolarisationOfKingFactor(kingFactor(gas, nm)));
    }
  });

  it.each(GASES)("cites %s's dispersion", (gas) => {
    expect(rayleighOf(gas).source).toContain(GAS_DISPERSION[gas].source);
  });

  it.each(ESTIMATED_RAYLEIGH)("calls $species's optics estimated", (row) => {
    expect(rayleighOf(row.species).provenance).toBe("estimated");
  });

  it.each(ESTIMATED_RAYLEIGH)("says why $species's optics are estimated", (row) => {
    expect(rayleighOf(row.species).source).toContain(row.basis);
  });

  it.each(DISPERSION_ROWS)("gives $species its measured dispersion's σ", (row) => {
    const optics = rayleighOf(row.species);
    for (const [, nm] of CHANNELS) {
      expect(optics.crossSectionM2(nm)).toBe(
        crossSectionFromDispersionM2(row.dispersion, row.kingFactor.factor, nm),
      );
    }
  });

  it.each(DISPERSION_ROWS)("gives $species its King factor's ρ", (row) => {
    const optics = rayleighOf(row.species);
    for (const [, nm] of CHANNELS) {
      expect(optics.depolarisation(nm)).toBe(depolarisationOfKingFactor(row.kingFactor.factor(nm)));
    }
  });

  it.each(STATIC_ROWS)("gives $species the static rule's σ", (row) => {
    const optics = rayleighOf(row.species);
    for (const [, nm] of CHANNELS) {
      expect(optics.crossSectionM2(nm)).toBe(
        polarisabilityCrossSectionM2(row.polarisabilityM3, nm),
      );
    }
  });

  it.each(POLARISABILITY_ROWS)("gives $species ρ = 0", (row) => {
    const optics = rayleighOf(row.species);
    expect(CHANNEL_WAVELENGTHS_NM.map((nm) => optics.depolarisation(nm))).toEqual([0, 0, 0]);
  });

  it("calls a key it does not know none", () => {
    expect(rayleighOf(UNKNOWN).provenance).toBe("none");
  });

  it("gives a key it does not know no cross-section", () => {
    const optics = rayleighOf(UNKNOWN);
    expect(CHANNEL_WAVELENGTHS_NM.map((nm) => optics.crossSectionM2(nm))).toEqual([0, 0, 0]);
  });

  it("gives a key it does not know ρ = 0", () => {
    const optics = rayleighOf(UNKNOWN);
    expect(CHANNEL_WAVELENGTHS_NM.map((nm) => optics.depolarisation(nm))).toEqual([0, 0, 0]);
  });

  // A measured gas, a dispersion row, a static row, an oscillator row and an unknown key.
  it.each(["N2", "CO", "HCN", "H", UNKNOWN])(
    "refuses a cross-section at a wavelength in µm for %s",
    (species) => {
      expect(() => rayleighOf(species).crossSectionM2(0.55)).toThrow(RangeError);
    },
  );

  it.each(["N2", "CO", "HCN", "H", UNKNOWN])(
    "refuses a ρ at a wavelength in µm for %s",
    (species) => {
      expect(() => rayleighOf(species).depolarisation(0.55)).toThrow(RangeError);
    },
  );

  it("refuses polarisabilityCrossSectionM2 a wavelength in µm", () => {
    expect(() => polarisabilityCrossSectionM2(1e-30, 0.55)).toThrow(RangeError);
  });
});

describe("ρ from a King factor", () => {
  it("is 0 for an isotropic molecule, F_K = 1", () => {
    expect(depolarisationOfKingFactor(1)).toBe(0);
  });

  // Sneep and Ubachs 2005, eq. 7: F_K = (6 + 3ρ) ÷ (6 − 7ρ) for ρ in natural light.
  it.each([0.003, 0.0279, 0.0475, 0.1])("inverts eq. 7's F_K at ρ = %f", (rho) => {
    const king = (6 + 3 * rho) / (6 - 7 * rho);
    expect(relative(depolarisationOfKingFactor(king), rho)).toBeLessThan(1e-12);
  });
});

describe("the estimated rows", () => {
  /** The rows' species. */
  const ESTIMATED_SPECIES = ESTIMATED_RAYLEIGH.map((row) => row.species);

  it("name no measured gas", () => {
    expect(ESTIMATED_SPECIES.filter((key) => GASES.some((gas) => gas === key))).toEqual([]);
  });

  it("name each species once", () => {
    expect(new Set(ESTIMATED_SPECIES).size).toBe(ESTIMATED_SPECIES.length);
  });

  it.each(ESTIMATED_RAYLEIGH)("state why $species is an estimate", (row) => {
    expect(row.basis.length).toBeGreaterThan(0);
  });

  it.each(ESTIMATED_RAYLEIGH)(
    "state $species's uncertainty as a factor above 1, or none",
    (row) => {
      expect(row.uncertaintyFactor === undefined || row.uncertaintyFactor > 1).toBe(true);
    },
  );

  it.each(POLARISABILITY_ROWS)("give $species a positive α", (row) => {
    expect(row.polarisabilityM3).toBeGreaterThan(0);
  });

  it.each(POLARISABILITY_ROWS)("put $species's oscillator, if any, below 300 nm", (row) => {
    expect(row.resonanceNm === undefined || row.resonanceNm < RAYLEIGH_WAVELENGTH_RANGE_NM[0]).toBe(
      true,
    );
  });

  it.each(DISPERSION_ROWS)("cite $species's dispersion and King factor", (row) => {
    expect(Math.min(row.dispersion.source.length, row.kingFactor.source.length)).toBeGreaterThan(0);
  });

  it.each(DISPERSION_ROWS)("keep $species's n − 1 positive and falling over the range", (row) => {
    const span = wavelengthsNm(...RAYLEIGH_WAVELENGTH_RANGE_NM, 1).map((nm) =>
      row.dispersion.nMinusOne(nm),
    );
    expect(span.every((delta, i) => delta > 0 && (i === 0 || delta < (span[i - 1] ?? 0)))).toBe(
      true,
    );
  });

  it.each(DISPERSION_ROWS)("fit $species's dispersion over a span inside the bake bins", (row) => {
    const [fromNm, toNm] = row.dispersion.measuredNm;
    expect(fromNm < toNm && fromNm < 760 && toNm > 380).toBe(true);
  });

  it("give H its exact 9/2 a₀³, 0.666 83 Å³", () => {
    const h = POLARISABILITY_ROWS.find((row) => row.species === "H");
    expect(relative((h?.polarisabilityM3 ?? Number.NaN) / 1e-30, 0.666_831)).toBeLessThan(1e-6);
  });

  // Lee and Kim, MNRAS 347 (2004) 802, Table 1's exact series for atomic hydrogen, cm² (the science
  // check's evaluation of it): the static rule runs 5.5%, 8.7% and 14% below them.
  it.each([
    [680, 2.865_2e-28],
    [550, 6.893_3e-28],
    [440, 1.766_8e-27],
  ] as const)("put H within 0.1% of its exact σ at %i nm", (nm, exactCm2) => {
    expect(relative(rayleighOf("H").crossSectionM2(nm) * CM2_PER_M2, exactCm2)).toBeLessThan(1e-3);
  });

  it("put CO within Sneep and Ubachs's measured (6.19 ± 0.40) × 10⁻²⁷ cm² at 532.2 nm", () => {
    // Sneep and Ubachs 2005, Table 2: a cavity ring-down extinction, CO having no visible absorption.
    // The dispersion gives 6.59, 0.99σ above: their measurements run 2–6% below the n-based values
    // for every gas (R08's Risks, T3.b's table), so the margin is the data's, not a tolerance.
    expect(
      Math.abs(rayleighOf("CO").crossSectionM2(SNEEP_NM) * CM2_PER_M2 - 6.19e-27),
    ).toBeLessThan(0.4e-27);
  });

  it("put (128π⁵ ÷ 3) α² ÷ λ⁴ equal to the dispersion route for a dilute gas", () => {
    // (n² − 1) ÷ (n² + 2) = (4π ÷ 3) N α: a molecule of N₂'s refractivity at 550 nm with F_K = 1.
    const dispersion = GAS_DISPERSION.N2;
    const perM3 = referenceNumberDensityPerM3(dispersion);
    const delta = dispersion.nMinusOne(550);
    const lorentzLorenz = (delta * (2 + delta)) / (delta * (2 + delta) + 3);
    const alphaM3 = (3 * lorentzLorenz) / (4 * Math.PI * perM3);
    const viaDispersion = crossSectionFromDispersionM2(dispersion, () => 1, 550);
    expect(relative(polarisabilityCrossSectionM2(alphaM3, 550), viaDispersion)).toBeLessThan(1e-12);
  });
});

/** A measured gas's σ over the estimate's from a polarisability of `angstrom3` Å³, at a wavelength. */
function measuredOverEstimate(gas: Gas, angstrom3: number, nm: number): number {
  return rayleighCrossSectionM2(gas, nm) / polarisabilityCrossSectionM2(angstrom3 * 1e-30, nm);
}

describe("the estimate's route, on three measured gases", () => {
  /**
   * α₀, Å³, by the molecules' rule: the λ → ∞ limit of C. and M. Cuthbertson's visible dispersions
   * at 0 °C and 760 mm (N₂, Proc. R. Soc. Lond. A 83 (1910) 151, μ∞ − 1 = 2.9450 × 10⁻⁴; CO₂ and
   * CH₄, Proc. R. Soc. Lond. A 97 (1920) 152), as the science check reduced them.
   */
  const ROUTE = [
    ["N2", 1.744_4],
    ["CO2", 2.593_1],
    ["CH4", 2.547_6],
  ] as const;

  // A check on the route, not a tolerance on the data: 1.066, 1.194 and 1.071 measured.
  it.each(ROUTE)("gives %s's σ(550) within a factor of 1.3", (gas, angstrom3) => {
    const ratio = measuredOverEstimate(gas, angstrom3, 550);
    expect(ratio).toBeLessThan(1.3);
    expect(ratio).toBeGreaterThan(1 / 1.3);
  });

  it.each(ROUTE)("runs low for %s, and lower towards the violet", (gas, angstrom3) => {
    const ratios = wavelengthsNm(400, 700, 10).map((nm) =>
      measuredOverEstimate(gas, angstrom3, nm),
    );
    expect(ratios.every((ratio) => ratio > 1)).toBe(true);
    expect(ratios.slice(1).every((ratio, i) => ratio < (ratios[i] ?? Number.NaN))).toBe(true);
  });
});

describe("the molecular term", () => {
  it.each(CHANNELS)("scatters as N₂ alone at channel %i for a pure N₂ column", (channel, nm) => {
    const expected = SEA_LEVEL_COLUMN.surfaceNumberDensityPerM3 * rayleighCrossSectionM2("N2", nm);
    expect(relative(scatteringAt(alone("N2"), channel), expected)).toBeLessThan(1e-15);
  });

  it("takes N₂'s own ρ for a pure N₂ column", () => {
    expect(molecularTerm(SEA_LEVEL_COLUMN, alone("N2")).term.phase).toEqual({
      kind: "rayleigh",
      depolarisation: CHANNEL_WAVELENGTHS_NM.map((nm) =>
        depolarisationOfKingFactor(kingFactor("N2", nm)),
      ),
    });
  });

  it("reports no approximation for a measured gas", () => {
    expect(molecularTerm(SEA_LEVEL_COLUMN, alone("N2")).approximations).toEqual([]);
  });

  it("lies on the column's density", () => {
    expect(molecularTerm(SEA_LEVEL_COLUMN, DRY_AIR).term.density).toBe(SEA_LEVEL_COLUMN.density);
  });

  it("absorbs nothing", () => {
    expect(molecularTerm(SEA_LEVEL_COLUMN, DRY_AIR).term.absorption).toEqual([0, 0, 0]);
  });

  it("is named as R05's Rayleigh term is, `rayleigh`", () => {
    expect(molecularTerm(SEA_LEVEL_COLUMN, DRY_AIR).term.name).toBe("rayleigh");
  });

  it.each([0, 0.1, 0.5, 0.9, 1])("is linear in the fractions: N₂ at %f with CO₂", (nitrogen) => {
    const fractions: GasFractions = [
      { species: "N2", moleFraction: nitrogen },
      { species: "CO2", moleFraction: 1 - nitrogen },
    ];
    for (const [channel] of CHANNELS) {
      const expected =
        nitrogen * scatteringAt(alone("N2"), channel) +
        (1 - nitrogen) * scatteringAt(alone("CO2"), channel);
      expect(relative(scatteringAt(fractions, channel), expected)).toBeLessThan(1e-14);
    }
  });

  it("refuses fractions that sum to 1 + 2 × 10⁻⁹", () => {
    const fractions: GasFractions = [
      { species: "N2", moleFraction: 0.5 },
      { species: "O2", moleFraction: 0.5 + 2e-9 },
    ];
    expect(() => molecularTerm(SEA_LEVEL_COLUMN, fractions)).toThrow(RangeError);
  });

  it("takes fractions that sum to 1 within 10⁻⁹", () => {
    const fractions: GasFractions = [
      { species: "N2", moleFraction: 0.5 },
      { species: "O2", moleFraction: 0.5 - 5e-10 },
    ];
    expect(() => molecularTerm(SEA_LEVEL_COLUMN, fractions)).not.toThrow();
  });

  it.each<readonly [string, GasFractions]>([
    ["no species", []],
    [
      "a negative fraction",
      [
        { species: "N2", moleFraction: 1.5 },
        { species: "O2", moleFraction: -0.5 },
      ],
    ],
    ["a NaN fraction", [{ species: "N2", moleFraction: Number.NaN }]],
    [
      "a species listed twice",
      [
        { species: "N2", moleFraction: 0.5 },
        { species: "N2", moleFraction: 0.5 },
      ],
    ],
  ])("refuses %s", (_case, fractions) => {
    expect(() => molecularMixture(fractions)).toThrow(RangeError);
  });

  it.each(CHANNELS)(
    "gives Earth's coefficient at channel %i (4.85, 11.5, 28.7 × 10⁻⁶ m⁻¹) to 1% from dry air's gases",
    (channel) => {
      const reference = EARTH_RAYLEIGH_PER_M[channel] ?? Number.NaN;
      expect(relative(scatteringAt(DRY_AIR, channel), reference)).toBeLessThan(0.01);
    },
  );

  it("weights the King factors by scattering, Σxᵢσᵢ ÷ Σxᵢ(σᵢ ÷ Fᵢ)", () => {
    const mixture = molecularMixture(DRY_AIR);
    for (const nm of wavelengthsNm(380, 760, 20)) {
      let scattered = 0;
      let isotropic = 0;
      for (const [gas, percent] of DRY_AIR_PERCENT) {
        const sigma = percent * rayleighCrossSectionM2(gas, nm);
        scattered += sigma;
        isotropic += sigma / kingFactor(gas, nm);
      }
      expect(relative(mixture.kingFactor(nm), scattered / isotropic)).toBeLessThan(1e-14);
    }
  });

  describe("with an unknown key", () => {
    const WITH_UNKNOWN: GasFractions = [
      { species: "N2", moleFraction: 0.75 },
      { species: UNKNOWN, moleFraction: 0.25 },
    ];

    it.each(CHANNELS)("adds nothing to the scattering at channel %i", (channel) => {
      const expected = 0.75 * scatteringAt(alone("N2"), channel);
      expect(relative(scatteringAt(WITH_UNKNOWN, channel), expected)).toBeLessThan(1e-15);
    });

    it("leaves the known gas's ρ as it is", () => {
      expect(molecularTerm(SEA_LEVEL_COLUMN, WITH_UNKNOWN).term.phase).toEqual(
        molecularTerm(SEA_LEVEL_COLUMN, alone("N2")).term.phase,
      );
    });

    it("reports the key as having no optics", () => {
      expect(molecularTerm(SEA_LEVEL_COLUMN, WITH_UNKNOWN).approximations).toEqual([
        { species: UNKNOWN, provenance: "none" },
      ]);
    });

    it("scatters nothing when it is the only species", () => {
      expect(molecularTerm(SEA_LEVEL_COLUMN, alone(UNKNOWN)).term.scattering).toEqual([0, 0, 0]);
    });

    it("takes ρ = 0 when it is the only species", () => {
      expect(molecularTerm(SEA_LEVEL_COLUMN, alone(UNKNOWN)).term.phase).toEqual({
        kind: "rayleigh",
        depolarisation: [0, 0, 0],
      });
    });
  });

  it("puts dry air's ρ_mix 3.5–4.5% below Bates's factors mixed by volume, over 380–760 nm", () => {
    // Bodhaine et al. 1999's eq. 23 averages Bates's F by volume (`dryAirKingFactor`), 0.19% above
    // the σ-weighted F_mix; ρ, proportional to F − 1, moves 21 times as far, 3.9% (R08's Risks).
    const mixture = molecularMixture(DRY_AIR);
    for (const nm of wavelengthsNm(380, 760, 20)) {
      const bates = depolarisationOfKingFactor(dryAirKingFactor(nm));
      const below = 1 - mixture.depolarisation(nm) / bates;
      expect(below).toBeGreaterThan(0.035);
      expect(below).toBeLessThan(0.045);
    }
  });

  it("puts dry air's ρ_mix within 4% of Young's F(air) = 1.0480, ρ = 0.0279, over 440–680 nm", () => {
    // Young, J. Appl. Meteor. 20 (1981) 328, as Bodhaine et al. 1999 quote it: the one
    // published air value seen (not read in its primary). ρ_mix runs 2.3% below it at 550 nm.
    const young = depolarisationOfKingFactor(1.048);
    const mixture = molecularMixture(DRY_AIR);
    for (const nm of wavelengthsNm(440, 680, 20)) {
      expect(relative(mixture.depolarisation(nm), young)).toBeLessThan(0.04);
    }
  });
});

/**
 * The substance registry's gas rows, in its order (decision-composition §1.1): P14.T49.a's nine,
 * then P14.T49.b's measured four and the gases of the chemistry. Until P14.T49.a writes
 * `packages/protocol/fixtures/substances.json` this list stands for its gas rows, and R08.T19's
 * parity check then reads the fixture itself. e⁻ and H⁻ are left out: their scattering and
 * absorption are R08.T4.c's continua (Thomson; bound–free and free–free), not a Rayleigh row.
 */
const REGISTRY_GAS_ROWS = [
  ...GASES,
  "CO",
  "SO2",
  "H2S",
  "O3",
  "HCN",
  "C2H2",
  "C2H4",
  "C2H6",
  "PH3",
  "CH3OH",
  "H",
  "O",
  "Na",
  "K",
  "Fe",
  "Mg",
  "Si",
  "Ca",
  "Ti",
  "SiO",
  "TiO",
  "VO",
  "FeH",
] as const;

describe("the registry's gas rows", () => {
  it.each(REGISTRY_GAS_ROWS)("give %s measured or estimated optics", (species) => {
    expect(rayleighOf(species).provenance).not.toBe("none");
  });

  it("are the measured gases and the estimated rows, in the registry's order", () => {
    expect(REGISTRY_GAS_ROWS).toEqual([...GASES, ...ESTIMATED_RAYLEIGH.map((row) => row.species)]);
  });

  /** Every row in equal parts. */
  const EVERY_ROW: GasFractions = REGISTRY_GAS_ROWS.map((species) => ({
    species,
    moleFraction: 1 / REGISTRY_GAS_ROWS.length,
  }));

  it("give a term, mixed in equal parts, that scatters in each channel", () => {
    const { term } = molecularTerm(SEA_LEVEL_COLUMN, EVERY_ROW);
    expect(term.scattering.every((perM) => perM > 0)).toBe(true);
  });

  it("give a term, mixed in equal parts, that reports every estimated row", () => {
    expect(molecularTerm(SEA_LEVEL_COLUMN, EVERY_ROW).approximations).toEqual(
      ESTIMATED_RAYLEIGH.map(({ species }) => ({ species, provenance: "estimated" })),
    );
  });

  it.each(REGISTRY_GAS_ROWS)("give %s alone a term that scatters in each channel", (species) => {
    const { term } = molecularTerm(SEA_LEVEL_COLUMN, alone(species));
    expect(term.scattering.every((perM) => perM > 0)).toBe(true);
  });
});
