import { describe, expect, it } from "vitest";

import { CHANNEL_WAVELENGTHS_NM } from "./medium";
import {
  BOLTZMANN_J_PER_K,
  crossSectionFromDispersionM2,
  DRY_AIR_DISPERSION,
  dryAirKingFactor,
  type Gas,
  GAS_DISPERSION,
  GAS_KING_FACTOR,
  GAS_MOLAR_MASS_G_PER_MOL,
  GASES,
  kingFactor,
  RAYLEIGH_WAVELENGTH_RANGE_NM,
  rayleighCrossSectionM2,
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

/** Dry air's cross-section by Peck and Reeder's refractivity and Bodhaine's mixed King factor, m². */
function dryAirCrossSectionM2(wavelengthNm: number): number {
  return crossSectionFromDispersionM2(
    DRY_AIR_DISPERSION,
    dryAirKingFactor(wavelengthNm),
    wavelengthNm,
  );
}

/** Σxᵢσᵢ ÷ Σxᵢ over a composition, m² (Design note 4's mixture, which R08.T3.c builds). */
function mixtureCrossSectionM2(
  composition: ReadonlyArray<readonly [Gas, number]>,
  wavelengthNm: number,
): number {
  let sum = 0;
  let total = 0;
  for (const [gas, fraction] of composition) {
    sum += fraction * rayleighCrossSectionM2(gas, wavelengthNm);
    total += fraction;
  }
  return sum / total;
}

/**
 * Dry air by volume, percent: N₂ 78.084, O₂ 20.946, Ar 0.934 (Bodhaine et al. 1999, eq. 23), and
 * Peck and Reeder's 330 ppm of CO₂ (Bodhaine §1).
 */
const DRY_AIR_COMPOSITION: ReadonlyArray<readonly [Gas, number]> = [
  ["N2", 78.084],
  ["O2", 20.946],
  ["Ar", 0.934],
  ["CO2", 0.033],
];

/** Sea level's real number density, m⁻³: 288.15 K and 101,325 Pa (US Standard Atmosphere 1976). */
const SEA_LEVEL_PER_M3 = referenceNumberDensityPerM3(DRY_AIR_DISPERSION);

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

describe("the gases", () => {
  it("start with plan 14's nine, in the sim's Gas::ALL order", () => {
    expect(GASES.slice(0, 9)).toEqual(["H2", "He", "H2O", "CH4", "NH3", "N2", "O2", "CO2", "Ar"]);
  });

  it("each have a dispersion, a King factor and a molar mass, and nothing else does", () => {
    const gases = GASES.toSorted();
    expect(Object.keys(GAS_DISPERSION).toSorted()).toEqual(gases);
    expect(Object.keys(GAS_KING_FACTOR).toSorted()).toEqual(gases);
    expect(Object.keys(GAS_MOLAR_MASS_G_PER_MOL).toSorted()).toEqual(gases);
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
   * p ÷ (ρRT); 1 for He et al. 2021's CH₄, whose n is defined through the ideal N.
   */
  const STATES: Readonly<Record<Gas, readonly [number, number, number]>> = {
    H2: [273.15, 101_325, 1.000_624], // Peck and Huang 1977: 0 °C, 760 torr
    He: [273.15, 101_325, 1.000_532], // Mansfield and Peck 1969: 0 °C, 760 torr
    H2O: [293.15, 1_333, 0.999_237], // Ciddor 1996, eq. 3: 20 °C, 1,333 Pa
    CH4: [288.15, 101_325, 1], // He et al. 2021: 288.15 K, 1013.25 hPa
    NH3: [273.15, 101_325, 0.984_798], // Cuthbertson and Cuthbertson 1914: 0 °C, 760 mm
    N2: [288.15, 101_325, 0.999_715], // Peck and Khanna 1966: 15 °C, 760 torr
    O2: [293.15, 101_325, 0.999_282], // Zhang et al. 2008 and Křen 2011: 20 °C, 101,325 Pa
    CO2: [273.15, 101_325, 0.993_265], // Bideau-Mehu et al. 1973: 0 °C, 760 torr
    Ar: [288.15, 101_325, 0.999_26], // Peck and Fisher 1964: 15 °C, 760 torr
    Ne: [273.15, 100_000, 1.000_483], // Börzsönyi et al. 2008: 0 °C, 1,000 mbar
    Kr: [273.15, 100_000, 0.997_282], // Börzsönyi et al. 2008: 0 °C, 1,000 mbar
    Xe: [273.15, 100_000, 0.993_245], // Börzsönyi et al. 2008: 0 °C, 1,000 mbar
    N2O: [288.15, 101_325, 1], // He et al. 2021: 288.15 K, 1013.25 hPa
  };

  it("pins (T, p, Z, N_ref) to each paper", () => {
    for (const gas of GASES) {
      const dispersion = GAS_DISPERSION[gas];
      const [kelvin, pascals, z] = STATES[gas];
      expect([dispersion.referenceK, dispersion.referencePa, dispersion.compressibility]).toEqual([
        kelvin,
        pascals,
        z,
      ]);
      const perM3 = pascals / (z * BOLTZMANN_J_PER_K * kelvin);
      expect(relative(referenceNumberDensityPerM3(dispersion), perM3)).toBeLessThan(1e-12);
    }
  });

  it("gives the ideal N = 2.546 899 × 10²⁵ m⁻³ for CH₄, as He et al. 2021's Table 1 states", () => {
    // The 7 × 10⁻⁶ between them is CODATA 1986's k_B, 1.380 658 × 10⁻²³ J K⁻¹, which the
    // literature's N carries.
    expect(relative(referenceNumberDensityPerM3(GAS_DISPERSION.CH4), 2.546_899e25)).toBeLessThan(
      1e-5,
    );
  });

  it("reproduces the indices Mansfield and Peck, and Peck and Huang, print at 546.2 nm", () => {
    // Mansfield and Peck 1969's abstract: 3.4950 × 10⁻⁵ at 5462.258 Å, 760 torr, 0 °C.
    expect(relative(GAS_DISPERSION.He.nMinusOne(546.225_8), 3.495e-5)).toBeLessThan(2e-5);
    // Peck and Huang 1977's abstract: 139.30 × 10⁻⁶ at 0.546 225 2 µm, standard conditions.
    expect(relative(GAS_DISPERSION.H2.nMinusOne(546.225_2), 139.3e-6)).toBeLessThan(1e-4);
  });

  it("reproduces Sneep and Ubachs 2005's Table 2 indices at 15 °C for Ar, N₂ and CO₂", () => {
    // Table 2's (n − 1) × 10⁶ at 18,788.4 cm⁻¹, "at 15 °C and 101 325 Pa", scaled by them as an
    // ideal gas (their §4.3): 268, 284 and 427.
    const printed: ReadonlyArray<readonly [Gas, number]> = [
      ["Ar", 268],
      ["N2", 284],
      ["CO2", 427],
    ];
    for (const [gas, perMillion] of printed) {
      const dispersion = GAS_DISPERSION[gas];
      const at15 = dispersion.nMinusOne(SNEEP_NM) * (dispersion.referenceK / 288.15);
      expect(Math.abs(at15 * 1e6 - perMillion)).toBeLessThan(0.5);
    }
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

  it("matches Bates 1984's O₂ formula over its 288–546 nm to 0.2%", () => {
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

  it("matches C. and M. Cuthbertson's Ne, Kr and Xe over the spans both cover, to 0.5%", () => {
    // refractiveindex.info's main/{Ne,Kr,Xe}/nk/Cuthbertson.yml, n − 1 = A ÷ (B − λ⁻²), λ in µm, at
    // 0 °C and 760 torr: Ne (Proc. R. Soc. A 135 (1932) 40) over 289–546 nm, Kr and Xe (Proc. R.
    // Soc. A 84 (1910) 13) over 480–671 nm. Scaled to Börzsönyi's 1,000 mbar.
    const cuthbertson: ReadonlyArray<readonly [Gas, number, number, number, number]> = [
      ["Ne", 0.029_073_88, 435.713_76, 400, 546],
      ["Kr", 0.059_467_251, 142.062_05, 480, 670],
      ["Xe", 0.068_104_197, 99.892_276, 480, 670],
    ];
    for (const [gas, a, b, from, to] of cuthbertson) {
      for (const nm of wavelengthsNm(from, to, 2)) {
        const theirs = (a / (b - (1_000 / nm) ** 2)) * (100_000 / 101_325);
        expect(relative(GAS_DISPERSION[gas].nMinusOne(nm), theirs)).toBeLessThan(5e-3);
      }
    }
  });

  it("matches Sneep and Ubachs's N₂O eq. 20 (Alms et al. 1975) over 458–647 nm to 1%", () => {
    // Sneep and Ubachs 2005, eq. 20: 10⁸(n − 1) = 46,890 + 4.12 × 10⁻⁶ ν², at 15 °C.
    for (const nm of wavelengthsNm(458, 647, 3)) {
      const sneep = (46_890 + 4.12e-6 * (1e7 / nm) ** 2) * 1e-8;
      expect(relative(GAS_DISPERSION.N2O.nMinusOne(nm), sneep)).toBeLessThan(0.01);
    }
  });

  it("gives N₂O's King factor as Sneep and Ubachs's Table 2 does, 1.225 at 532.2 nm", () => {
    expect(Math.abs(kingFactor("N2O", SNEEP_NM) - 1.225)).toBeLessThan(5e-4);
  });

  it("is not Sneep and Ubachs's CH₄ eq. 18, which runs over 10% high", () => {
    // Sneep and Ubachs 2005, eq. 18: 10⁸(n − 1) = 46,662 + 4.02 × 10⁻⁶ ν², at 15 °C.
    const nm = 550;
    const sneep = (46_662 + 4.02e-6 * (1e7 / nm) ** 2) * 1e-8;
    expect(sneep / GAS_DISPERSION.CH4.nMinusOne(nm) - 1).toBeGreaterThan(0.1);
  });
});

describe("the CO₂ trap (Design note 4)", () => {
  it("raises σ by at least 10% when Bideau-Mehu's 0 °C formula is read at 288.15 K", () => {
    const misread = { ...GAS_DISPERSION.CO2, referenceK: 288.15 };
    for (const nm of CHANNEL_WAVELENGTHS_NM) {
      const wrong = crossSectionFromDispersionM2(misread, kingFactor("CO2", nm), nm);
      expect(wrong / rayleighCrossSectionM2("CO2", nm) - 1).toBeGreaterThan(0.1);
    }
  });
});

describe("dry air, the Earth check", () => {
  it("recomputes Bucholtz 1995's 4.51 × 10⁻²⁷ cm² at 550 nm to 1%", () => {
    expect(relative(dryAirCrossSectionM2(550) * CM2_PER_M2, 4.51e-27)).toBeLessThan(0.01);
  });

  it("recomputes Bodhaine et al. 1999's Table 3, 4.5105 × 10⁻²⁷ cm² at 550 nm, to 0.2%", () => {
    expect(relative(dryAirCrossSectionM2(550) * CM2_PER_M2, 4.510_5e-27)).toBeLessThan(2e-3);
  });

  it("gives Earth's 4.85, 11.5 and 28.7 × 10⁻⁶ m⁻¹ at 680, 550 and 440 nm to 1%", () => {
    for (const [c, nm] of CHANNEL_WAVELENGTHS_NM.entries()) {
      const reference = EARTH_RAYLEIGH_PER_M[c] ?? Number.NaN;
      expect(relative(dryAirCrossSectionM2(nm) * SEA_LEVEL_PER_M3, reference)).toBeLessThan(0.01);
      const mixed = mixtureCrossSectionM2(DRY_AIR_COMPOSITION, nm) * SEA_LEVEL_PER_M3;
      expect(relative(mixed, reference)).toBeLessThan(0.01);
    }
  });

  it("is what the per-gas route gives for dry air's composition, to 0.1% over 380–760 nm", () => {
    for (const nm of wavelengthsNm(380, 760, 5)) {
      const mixed = mixtureCrossSectionM2(DRY_AIR_COMPOSITION, nm);
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
      const dalgarno = 8.14e-13 / angstrom ** 4 + 1.28e-6 / angstrom ** 6 + 1.61 / angstrom ** 8;
      const ratio = (rayleighCrossSectionM2("H2", nm) * CM2_PER_M2) / dalgarno;
      expect(ratio).toBeGreaterThan(1);
      expect(ratio - 1).toBeLessThan(0.08);
    }
  });

  it("puts He within 1% of Dalgarno's cross-section", () => {
    // Kurucz, SAO Spec. Rep. 309 (1970), §5.8, "from Dalgarno (1962)": σ = 5.484 × 10⁻¹⁴ λ⁻⁴
    // [1 + 2.44 × 10⁵ λ⁻² + 5.94 × 10¹⁰ ÷ (λ²(λ² − 2.90 × 10⁵))]² cm², λ in Å (the report's
    // "5.94E−10" is a misprint for ATLAS's 5.94E10). Chan and Dalgarno 1965 was not read.
    for (const nm of wavelengthsNm(380, 760, 20)) {
      const a2 = (nm * 10) ** 2;
      const dalgarno =
        (5.484e-14 / (a2 * a2)) * (1 + 2.44e5 / a2 + 5.94e10 / (a2 * (a2 - 2.9e5))) ** 2;
      expect(relative(rayleighCrossSectionM2("He", nm) * CM2_PER_M2, dalgarno)).toBeLessThan(0.01);
    }
  });
});

describe("Sneep and Ubachs 2005's measurements at 532.2 nm (their Table 2)", () => {
  // Measured and n-based cross-sections, 10⁻²⁷ cm², with the measurement's 1σ.
  const TABLE_2: ReadonlyArray<readonly [Gas, number, number, number]> = [
    ["Ar", 4.45, 0.3, 4.56],
    ["N2", 5.1, 0.24, 5.3],
    ["CO2", 12.4, 0.8, 13.29],
  ];

  it("pins Ar, N₂ and CO₂ to Sneep and Ubachs's n-based values, which take the ideal N, times Z²", () => {
    for (const [gas, , , nBased] of TABLE_2) {
      const z = GAS_DISPERSION[gas].compressibility;
      const sigma = rayleighCrossSectionM2(gas, SNEEP_NM) * CM2_PER_M2 * 1e27;
      expect(relative(sigma, nBased * z * z)).toBeLessThan(2e-3);
    }
  });

  it("agrees with their Ar, N₂ and CO₂ measurements within 1.5 of their errors", () => {
    // CO₂'s formula runs 6% above its measurement, 0.9σ (the plan's "may, by 7%").
    for (const [gas, measured, error] of TABLE_2) {
      const sigma = rayleighCrossSectionM2(gas, SNEEP_NM) * CM2_PER_M2 * 1e27;
      expect(Math.abs(sigma - measured)).toBeLessThan(1.5 * error);
    }
  });

  it("agrees with their O₂ value, (4.50 ± 0.15) × 10⁻²⁷ cm² from a three-component fit, within 1.5σ", () => {
    const sigma = rayleighCrossSectionM2("O2", SNEEP_NM) * CM2_PER_M2 * 1e27;
    expect(Math.abs(sigma - 4.5)).toBeLessThan(1.5 * 0.15);
  });

  it("puts CH₄'s scattering below their measured extinction, which carries CH₄'s absorption", () => {
    // 12.47 ± 0.23 is a cavity ring-down extinction; He et al. 2021 (§3.4) find it agrees with
    // their extinction, which absorption dominates over parts of 400–725 nm. Sneep and Ubachs's
    // own n-based 14.69 is 18% above it.
    const sigma = rayleighCrossSectionM2("CH4", SNEEP_NM) * CM2_PER_M2 * 1e27;
    expect(sigma).toBeLessThan(12.47 - 2 * 0.23);
    expect(sigma).toBeGreaterThan(0.85 * 12.47);
  });
});

describe("a Venus-class column (Design note 4)", () => {
  /** CO₂ molecules per m² over 92 bar at 8.87 m s⁻² (NASA's Venus fact sheet), 44.009 u (IUPAC). */
  const COLUMN_PER_M2 = 9.2e6 / (44.009 * 1.660_539_068_92e-27 * 8.87);

  it("gives a 92-bar CO₂ column τ_R(550 nm) of 16 ± 1", () => {
    expect(Math.abs(rayleighCrossSectionM2("CO2", 550) * COLUMN_PER_M2 - 16)).toBeLessThan(1);
  });

  it("gives it about 41 at 440 nm, the blue channel", () => {
    const tau = rayleighCrossSectionM2("CO2", 440) * COLUMN_PER_M2;
    expect(tau).toBeGreaterThan(40);
    expect(tau).toBeLessThan(42);
  });
});

describe("the tutorial set", () => {
  it("is 15–20% above Earth's coefficients from the formulas", () => {
    for (const [c, nm] of CHANNEL_WAVELENGTHS_NM.entries()) {
      const tutorial = TUTORIAL_RAYLEIGH_PER_M[c] ?? Number.NaN;
      const ratio = tutorial / (dryAirCrossSectionM2(nm) * SEA_LEVEL_PER_M3);
      expect(ratio).toBeGreaterThan(1.15);
      expect(ratio).toBeLessThan(1.2);
    }
  });

  it("is equalled by no gas at sea level's density, to 1%", () => {
    for (const [c, nm] of CHANNEL_WAVELENGTHS_NM.entries()) {
      const tutorial = TUTORIAL_RAYLEIGH_PER_M[c] ?? Number.NaN;
      for (const gas of GASES) {
        const beta = rayleighCrossSectionM2(gas, nm) * SEA_LEVEL_PER_M3;
        expect(relative(beta, tutorial)).toBeGreaterThan(0.01);
      }
    }
  });
});

describe("every formula over the wavelength range", () => {
  const [low, high] = RAYLEIGH_WAVELENGTH_RANGE_NM;
  const span = wavelengthsNm(low, high, 1);

  it("disperses normally: n − 1, F_K and σ fall as the wavelength grows", () => {
    for (const gas of GASES) {
      const { nMinusOne } = GAS_DISPERSION[gas];
      for (let i = 1; i < span.length; i++) {
        const shorter = span[i - 1] ?? Number.NaN;
        const longer = span[i] ?? Number.NaN;
        expect(nMinusOne(longer)).toBeGreaterThan(0);
        expect(nMinusOne(longer)).toBeLessThan(nMinusOne(shorter));
        expect(kingFactor(gas, longer)).toBeGreaterThanOrEqual(1);
        expect(kingFactor(gas, longer)).toBeLessThanOrEqual(kingFactor(gas, shorter));
        expect(rayleighCrossSectionM2(gas, longer)).toBeLessThan(
          rayleighCrossSectionM2(gas, shorter),
        );
      }
    }
  });

  it("refuses a wavelength outside 300–1,000 nm, a unit error", () => {
    expect(() => rayleighCrossSectionM2("N2", 0.55)).toThrow(RangeError);
    expect(() => rayleighCrossSectionM2("N2", 5_500)).toThrow(RangeError);
    expect(() => kingFactor("O2", Number.NaN)).toThrow(RangeError);
    expect(rayleighCrossSectionM2("N2", low)).toBeGreaterThan(0);
    expect(rayleighCrossSectionM2("N2", high)).toBeGreaterThan(0);
  });

  it("measures every formula over a span that reaches into the bake bins", () => {
    for (const gas of GASES) {
      const [from, to] = GAS_DISPERSION[gas].measuredNm;
      expect(from).toBeLessThan(to);
      expect(from).toBeLessThan(760);
      expect(to).toBeGreaterThan(380);
    }
  });
});
