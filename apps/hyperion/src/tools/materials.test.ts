import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import type { CiaSet } from "./atmosphereData";
import {
  type AbsorptionRow,
  absorptionIndex,
  ACID_GRID_NM,
  ACID_UV_DECAY_NM,
  ACID_UV_K365,
  acidUvAbsorberK,
  ammoniaLiquidLevelled,
  AR_DISPERSION,
  AR_ICE_CELL,
  AR_ICE_MEASURED_546,
  AR_LIQUID_DENSITY,
  AR_LIQUID_FACTOR,
  AR_LIQUID_ROWS,
  arWorstRow,
  AUGUSTIN_N2,
  averagedUniaxial,
  BLEEKRODE_CO2,
  C2H6_LIQUID_FIT,
  C2H6_LIQUID_MEASURED,
  cauchyIndex,
  cauchyResidual,
  cellMolarVolumeCm3PerMol,
  CH4_LIQUID_670_NM_N,
  checkedRows,
  CO2_CRITICAL_MOL_PER_L,
  CO2_DISPERSION,
  CO2_ICE_FIT,
  CO2_LIQUID_DENSITY,
  CO2_LIQUID_FACTOR,
  CO2_MOLAR_MASS_G_PER_MOL,
  co2CarryMiss,
  co2DensityGPerCm3,
  CO_DISPERSION,
  COMBINED_MATERIALS,
  GAGNON_COMBINED_ERROR,
  gagnonSlopeExcess,
  gasRowMolarRefractionCm3PerMol,
  KR_DISPERSION,
  KR_LIQUID_DENSITY,
  KR_LIQUID_ROWS,
  KR_LIQUID_TAIL,
  leastSquaresFactor,
  liquidOxygenK,
  LIVEING_DEWAR_1892_N2O,
  LIVEING_DEWAR_1892_N2O_LITHIUM,
  LIVEING_DEWAR_PRISM_ARCMIN,
  type MaterialHeader,
  molarRefractionRatio,
  molecularIceIndex,
  N2O_DISPERSION,
  N2O_LIQUID_DENSITY,
  N2O_LIQUID_FACTOR,
  O2_DISPERSION,
  O2_LIQUID_DENSITY,
  O2_LIQUID_FACTOR,
  O2_LIQUID_NUMBER_DENSITY_PER_CM3,
  OXYGEN_PAIR_CROSS_SECTIONS_CM5,
  OXYGEN_PAIR_LONGEST_NM,
  OXYGEN_PAIR_ZERO_NM,
  oxygenPairAbsorptionPerM,
  oxygenPairCrossSectionCm5,
  oxygenPairK,
  PHILLIPS_1920,
  phillipsFactor,
  prismApexArcmin,
  prismIndex,
  redTailIndex,
  redTailStep,
  redTailWorst,
  scaledLiquidIndex,
  scaledShapeMiss,
  SO2_LIQUID_CHECKS,
  SO2_LIQUID_FACTOR,
  T5K_LIQUID_RECORD,
  tableWithRedTail,
  THALMAN_VOLKAMER_REFERENCE,
  XE_DISPERSION,
  XE_LIQUID_DENSITY,
  XE_LIQUID_ROWS,
  XE_LIQUID_TAIL,
  DAIMON_24C,
  daimonExcess,
  DERIVED_MATERIALS,
  ethaneBurtonIndex,
  ethaneLiquidEstimate,
  ethaneLiquidLevelled,
  FETCHED_MATERIALS,
  fitCauchy,
  formulaIndex,
  formulaOnGrid,
  H2S_FILM,
  h2sFilmEstimate,
  HCL_REFRACTIVITY,
  iapwsWaterIndex,
  iapwsWaterN,
  ICE_ESTIMATES,
  iceIndex,
  IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL,
  indexOfDielectric,
  ionicIncrement,
  KOU_1993_ABSORPTION_PER_M,
  LIQUID_CHECKS,
  liquidCheckExcess,
  logLinearAbsorptionPerM,
  lorentzLorenzIndex,
  MARTONCHIK_TABLE_I_ROWS,
  martonchikUltravioletLineK,
  MASON_2016_ABSORPTION_PER_M,
  T5B_AMMONIA_ICE_550,
  MASON_DISSERTATION_ROWS,
  MASON_ROWS,
  MATERIAL_GRID_NM,
  methaneLiquidAt94K,
  N2_ICE_CELL,
  N2_LIQUID_FIT,
  NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL,
  NH3_LIQUID_FIT,
  NH3_LIQUID_MEASURED,
  NH3_REFRACTIVITY,
  NH4CL_CALIBRATION,
  NH4CL_IONIC_INCREMENT,
  NH4CL_MOLAR_VOLUME_CM3_PER_MOL,
  nh4clIndex,
  NH4SH_IONIC_INCREMENT,
  NH4SH_IONIC_INCREMENT_BAND,
  nh4shIndex,
  O2_ICE_CELL,
  parseAria,
  parseBraudeChromophore,
  parseDraineDielectric,
  parseHitranTable,
  parseIrwinAerosol3,
  parseKitzmannHeng,
  parseLnk,
  parseMartikainenK,
  parseOmlcAbsorption,
  parseRefractiveIndexInfo,
  parseRefractiveIndexInfoFormula,
  parseRefractiveIndexInfoN,
  parseWavelengthTable,
  parseWoppMason,
  POPE_FRY_ABSORPTION_PER_M,
  POPE_FRY_ROWS,
  QUAN_FRY_COEFFICIENTS,
  quanFryIndex,
  quanFrySalinityIncrement,
  reduceSource,
  resample,
  S8_MOLTEN_INDEX_D,
  S8_PRINCIPAL_INDICES,
  S8_STAND_IN_INDEX,
  SEAWATER,
  seawaterN,
  sourceOptions,
  T5G_WATER_K,
  T5G_WATER_N,
  titanMixtureResidual,
  titanRow,
  uniaxialFormulasOnGrid,
  WATER_DENSITY_KG_PER_M3,
  WATER_K_SPANS_NM,
  WATER_TEMPERATURE_K,
  waterIndex,
  wavenumberFill,
  wavenumberFraction,
  withoutVisibleAbsorption,
} from "./materials";

const MATERIALS = new URL("../renderer/src/view/atmosphere/materials/", import.meta.url);

/** A refractiveindex.info formula entry's text. */
function formula(type: number, range: string, coefficients: string): string {
  return [
    "DATA:",
    `  - type: formula ${type}`,
    `    wavelength_range: ${range}`,
    `    coefficients: ${coefficients}`,
  ].join("\n");
}

/** One row of a dielectric function at Draine's 550.06 nm. */
function draineRow(epsilonRe: number, epsilonIm: number) {
  return [{ wavelengthUm: 0.5500621, epsilonRe, epsilonIm }];
}

/** One row of an Irwin et al. table at 0.4 µm, its Aerosol-3 "Mean 1" k × 10³ as given. */
function irwinRow(k: string): string {
  return `0.4 & 1 & 2 & 3 & 4 & 5 & 6 & ${k}$\\times 10^{-3}$ & 8 & 9 \\\\`;
}

/** An Irwin et al. table of that one row, by its label. */
function irwinTable(label: string, k: string): string {
  return ["\\begin{table*}", irwinRow(k), `\\label{${label}}`, "\\end{table*}"].join("\n");
}

/** ρ = M ÷ V_m of an ice's cell, g cm⁻³. */
function cellDensity(
  molarMass: number,
  cell: { cellVolumeA3: number; formulaUnits: number },
): number {
  return molarMass / cellMolarVolumeCm3PerMol(cell.cellVolumeA3, cell.formulaUnits);
}

/** A committed material file's grid, n and k. */
function committed(file: string): {
  readonly wavelengthsNm: number[];
  readonly n: number[];
  readonly k: number[];
} {
  const raw: unknown = JSON.parse(readFileSync(fileURLToPath(new URL(file, MATERIALS)), "utf8"));
  const field = (name: string): number[] => {
    const v: unknown =
      typeof raw === "object" && raw !== null && name in raw ? Reflect.get(raw, name) : undefined;
    if (!Array.isArray(v)) {
      throw new TypeError(`${file}: no array ${name}`);
    }
    return v.map(Number);
  };
  return { wavelengthsNm: field("wavelengthsNm"), n: field("n"), k: field("k") };
}

/** A committed material file's text field. */
function committedText(file: string, field: string): string {
  const raw: unknown = JSON.parse(readFileSync(fileURLToPath(new URL(file, MATERIALS)), "utf8"));
  const value: unknown =
    typeof raw === "object" && raw !== null && field in raw ? Reflect.get(raw, field) : undefined;
  if (typeof value !== "string") {
    throw new TypeError(`${file}: no text ${field}`);
  }
  return value;
}

describe("parsing", () => {
  it("reads a refractiveindex.info tabulated block, in µm", () => {
    const text = [
      "REFERENCES: |",
      "    someone",
      "DATA:",
      "  - type: tabulated nk",
      "    data: |",
      "        0.400 1.339 1.86E-9",
      "        0.425 1.338 1.30E-9",
      "CONDITIONS:",
      "    temperature: 298",
    ].join("\n");
    expect(parseRefractiveIndexInfo(text)).toEqual([
      { wavelengthUm: 0.4, n: 1.339, k: 1.86e-9 },
      { wavelengthUm: 0.425, n: 1.338, k: 1.3e-9 },
    ]);
  });

  it("reads ARIA's wavenumbers as µm and keeps its NaN", () => {
    const rows = parseAria("#FORMAT=WAVN N K\n14250\t1.428\t2.07E-008\n18000\t1.431\tNaN\n");
    expect(rows[0]?.wavelengthUm).toBeCloseTo(1e4 / 14250, 15);
    expect(rows[1]?.k).toBeNaN();
  });

  it("reads an optool lnk file and checks its row count", () => {
    expect(parseLnk("# comment\n2 0.75\n0.30 1.5 1e-6\n0.32 1.49 2e-6\n")).toHaveLength(2);
    expect(() => parseLnk("# comment\n3 0.75\n0.30 1.5 1e-6\n")).toThrow(/count/u);
  });

  it("reads HITRAN-RI's four columns and a three-column table, skipping their headers", () => {
    const hitran =
      " Reference: x\n       cm-1    microns       real    imaginary\n   2.5813e+04 3.8740e-01     1.6600     0.0910\n";
    expect(parseHitranTable(hitran)).toEqual([{ wavelengthUm: 0.3874, n: 1.66, k: 0.091 }]);
    const table = "     WL(microns)      Nr         Ni\n     0.440000      1.49915   0.00767169\n";
    expect(parseWavelengthTable(table)).toEqual([
      { wavelengthUm: 0.44, n: 1.49915, k: 0.00767169 },
    ]);
  });
});

describe("resampling onto 380–780 nm", () => {
  const flat = [
    { wavelengthUm: 0.3, n: 1.5, k: 1e-6 },
    { wavelengthUm: 0.9, n: 1.2, k: 1e-4 },
  ];

  it("interpolates n linearly and k linearly in log k", () => {
    const { n, k } = resample(flat, { holdNm: 0 });
    const i600 = MATERIAL_GRID_NM.indexOf(600);
    expect(n[i600]).toBeCloseTo(1.35, 12);
    expect(k[i600]).toBeCloseTo(1e-5, 15);
  });

  it("interpolates k linearly where either end is 0", () => {
    const rows = [
      { wavelengthUm: 0.3, n: 1.5, k: 0 },
      { wavelengthUm: 0.9, n: 1.2, k: 2e-6 },
    ];
    expect(resample(rows, { holdNm: 0 }).k[MATERIAL_GRID_NM.indexOf(600)]).toBeCloseTo(1e-6, 15);
  });

  it("holds an end value only as far as allowed", () => {
    const short = [
      { wavelengthUm: 0.4, n: 1.3, k: 0 },
      { wavelengthUm: 1.0, n: 1.29, k: 0 },
    ];
    expect(() => resample(short, { holdNm: 0 })).toThrow(/beyond the source/u);
    expect(resample(short, { holdNm: 20 }).n[0]).toBe(1.3);
  });

  it("drops the rows below a cut before interpolating", () => {
    const edge = [
      { wavelengthUm: 0.1337, n: 1.704, k: 0.18 },
      { wavelengthUm: 0.4, n: 1.3, k: 0 },
      { wavelengthUm: 1.0, n: 1.288, k: 0 },
    ];
    expect(resample(edge, { ignoreBelowUm: 0.4, holdNm: 20 }).n[0]).toBe(1.3);
  });

  it("sets an unmeasured k from the shortest measured wavelength", () => {
    const acid = [
      { wavelengthUm: 0.3597, n: 1.452, k: Number.NaN },
      { wavelengthUm: 0.5556, n: 1.431, k: Number.NaN },
      { wavelengthUm: 0.7018, n: 1.428, k: 2.07e-8 },
      { wavelengthUm: 0.82, n: 1.427, k: 8e-8 },
    ];
    const { k } = resample(acid, { holdNm: 0, unmeasuredK: "shortestMeasured" });
    expect(k[0]).toBe(2.07e-8);
    expect(k[MATERIAL_GRID_NM.indexOf(700)]).toBe(2.07e-8);
  });

  it("refuses two values at one wavelength near the grid, and ignores them far from it", () => {
    const clash = [
      { wavelengthUm: 0.3, n: 1.5, k: 0 },
      { wavelengthUm: 0.5, n: 1.4, k: 0 },
      { wavelengthUm: 0.5, n: 1.41, k: 0 },
      { wavelengthUm: 0.9, n: 1.3, k: 0 },
    ];
    expect(() => resample(clash, { holdNm: 0 })).toThrow(/two rows/u);
    const far = [
      ...flat,
      { wavelengthUm: 2.969, n: 1.6, k: 0.5 },
      { wavelengthUm: 2.969, n: 1.7, k: 0.6 },
    ];
    expect(() => resample(far, { holdNm: 0 })).not.toThrow();
  });
});

describe("the derived files", () => {
  it("evaluate CO₂ ice's Cauchy fit as the science note gives it", () => {
    // n(λ) = 1.3994 + 0.004312 µm² ÷ λ² (science-r08-sulphur-co2ice.md); Warren 1986's Table I has
    // 1.413 at 0.553 µm and 1.404 at 1.000 µm.
    expect(cauchyIndex(CO2_ICE_FIT, 553).n).toBeCloseTo(1.4135, 4);
    expect(cauchyIndex(CO2_ICE_FIT, 1000).n).toBeCloseTo(1.4037, 4);
    expect(cauchyIndex(CO2_ICE_FIT, 550).k).toBeCloseTo((1e-2 * 550e-9) / (4 * Math.PI), 20);
  });

  it("take the ideal gas's molar volume at 0 °C and 1 atm from the exact SI constants", () => {
    // CODATA 2018: 22.413 969 54 × 10⁻³ m³ mol⁻¹.
    expect(IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL).toBeCloseTo(22_413.969_54, 4);
  });

  it("read NH₃'s formula at the paper's theoretic density, Z = 0.984 798 × 0.7708 ÷ 0.7605", () => {
    // Cuthbertson and Cuthbertson 1914, p. 21: the real gas's refractivity at 0 °C and 760 mm times
    // 0.7605 ÷ 0.7708 g L⁻¹; 0.984 798 is the NIST Chemistry WebBook's NH₃ there.
    expect(NH3_REFRACTIVITY.compressibility).toBeCloseTo(0.998_136, 6);
  });

  it("re-derive NH₄Cl's increment, +2.68%, from NH₃ and HCl in the same paper", () => {
    expect(NH4CL_IONIC_INCREMENT).toBeCloseTo(0.026_8, 4);
  });

  it("take NH₄Cl's n_D, 1.6391, from Gopalakrishnan 1957's fit to Poinsot and Mathieu", () => {
    // science-r08-gases-2.md §2.1: the Handbook of Mineralogy's sal ammoniac gives n = 1.639(1), and
    // NBS Circular 539 (vol. 1, p. 61) measured 1.641 on its own sample.
    const indexD = nh4clIndex(589.3);
    expect(Math.abs(indexD - 1.639_1)).toBeLessThan(1e-4);
    expect(Math.abs(indexD - 1.639)).toBeLessThan(0.001);
    expect(Math.abs(indexD - 1.641)).toBeLessThan(0.002);
  });

  it("evaluate NH₄Cl's fit over its stated 3130–7682 Å and refuse a wavelength outside it", () => {
    expect(() => nh4clIndex(313)).not.toThrow();
    expect(() => nh4clIndex(768.2)).not.toThrow();
    expect(() => nh4clIndex(312)).toThrow(RangeError);
    expect(() => nh4clIndex(769)).toThrow(RangeError);
    expect(() => nh4clIndex(Number.NaN)).toThrow(RangeError);
  });

  it("take NH₄Cl's density, 1.5258 g cm⁻³, from NBS Circular 539's cell, a = 3.8756 Å", () => {
    expect(NH4CL_MOLAR_VOLUME_CM3_PER_MOL).toBeCloseTo(35.056, 3);
    expect(
      Math.abs(NH4CL_CALIBRATION.molarMassGPerMol / NH4CL_MOLAR_VOLUME_CM3_PER_MOL - 1.525_8),
    ).toBeLessThan(1e-4);
  });

  it("build NH₄Cl's calibration from the fit at the D line and the cell", () => {
    expect(NH4CL_CALIBRATION.indexD).toBe(nh4clIndex(589.3));
    expect(NH4CL_CALIBRATION.molarVolumeCm3PerMol).toBe(NH4CL_MOLAR_VOLUME_CM3_PER_MOL);
    expect(NH4CL_CALIBRATION.densityGPerCm3).toBe(
      NH4CL_CALIBRATION.molarMassGPerMol / NH4CL_MOLAR_VOLUME_CM3_PER_MOL,
    );
  });

  it("find NH₄Cl's increment flat to 10⁻⁴ over 486.1–656.3 nm with the dispersion", () => {
    const increments = [486.1, 546.1, 589.3, 656.3].map((nm) =>
      ionicIncrement(
        [NH3_REFRACTIVITY, HCL_REFRACTIVITY],
        nm,
        NH4CL_MOLAR_VOLUME_CM3_PER_MOL,
        nh4clIndex(nm),
      ),
    );
    expect(Math.max(...increments) - Math.min(...increments)).toBeLessThan(1e-4);
  });

  it("take NH₄SH's increment as NH₄Cl's to 0.1%", () => {
    expect(Math.abs(NH4SH_IONIC_INCREMENT - NH4CL_IONIC_INCREMENT)).toBeLessThan(1e-3);
  });

  it("would give +4.7%, the top of the ruling's +4.3% to +4.7%, with its own inputs", () => {
    // The ruling's derivation (science-r08-nonspherical.md §3.1): HCl's n − 1 = 4.456 × 10⁻⁴ at the
    // D line, from a secondary source, with its Z read between 0.9924 and 1 (+4.7% to +4.3%).
    const realNh3 = { ...NH3_REFRACTIVITY, compressibility: 0.984_798 };
    const secondaryHcl = { aPerUm2: 4.456e-4 * (118.49 - 1 / 0.5893 ** 2), bPerUm2: 118.49 };
    // Its NH₄Cl inputs, through Wikipedia: n_D 1.642 (Chemister.ru) and 1.519 g cm⁻³ (the CRC
    // Handbook), at M = 53.489 g mol⁻¹.
    const increment = ionicIncrement(
      [realNh3, { ...secondaryHcl, compressibility: 0.992_4, source: "HCl" }],
      589.3,
      53.489 / 1.519,
      1.642,
    );
    expect(increment).toBeCloseTo(0.047, 3);
  });

  // science-r08-nonspherical.md §3.1's method; its 1.685, 1.648 and 1.632 at δ = +4.5% took NH₃'s
  // real-gas Z (R08's Risks, "Deviations in T3.b's follow-up, as built") and NH₄Cl's inputs through
  // Wikipedia (science-r08-gases-2.md §2).
  it.each([
    [380, 1.674_4],
    [550, 1.638_2],
    [780, 1.622_8],
  ] as const)("estimate NH₄SH's index at %i nm as %f, with the +2.7% increment", (nm, index) => {
    expect(Math.abs(nh4shIndex(nm) - index)).toBeLessThan(5e-4);
  });

  it("keep NH₄SH's estimate within +0.082 and −0.024 over the increment's band", () => {
    const [low, high] = NH4SH_IONIC_INCREMENT_BAND;
    for (const nm of MATERIAL_GRID_NM) {
      const n = nh4shIndex(nm);
      expect(nh4shIndex(nm, high) - n).toBeLessThan(0.082);
      expect(n - nh4shIndex(nm, low)).toBeLessThan(0.024);
    }
  });

  it("state NH₄SH's band over the grid, +0.081 and -0.022, in its file", () => {
    // The foot is 0.02247: science-r08-gases-2.md §2.2's −0.023 rounded its 4-figure 0.0225.
    const nh4sh = DERIVED_MATERIALS.find((file) => file.key === "NH4SH");
    expect(nh4sh?.standIn).toBe(
      "a non-absorbing particle whose real index is a Lorentz–Lorenz estimate, since no visible optical constants of NH4SH are published: 1.674 at 380 nm, 1.638 at 550 nm and 1.623 at 780 nm, within +0.081 and -0.022",
    );
    expect(nh4sh?.source).toContain(
      "1.80 would need a molar refraction 22% above the additive one.",
    );
  });

  it("read HCl's row as the paper prints it, with n = 3 × 10¹⁰ ÷ λ, to 2 × 10⁻⁵", () => {
    // Cuthbertson and Cuthbertson 1914, p. 12: (μ − 1) D ÷ (d₀76) = 4.6425 × 10²⁷ ÷ (10,664 × 10²⁷ −
    // n²); its Table V gives 44,803 × 10⁻⁸ at 5460.7 Å, calculated.
    const nm = 546.07;
    const frequencyPerS = 3e10 / (nm * 1e-7);
    const printed = 4.6425e27 / (10_664e27 - frequencyPerS * frequencyPerS);
    const row = HCL_REFRACTIVITY.aPerUm2 / (HCL_REFRACTIVITY.bPerUm2 - (1000 / nm) ** 2);
    expect(Math.abs(row / printed - 1)).toBeLessThan(2e-5);
  });

  it("reproduce Table V's calculated 44,803 × 10⁻⁸ for HCl at 5460.7 Å", () => {
    const nm = 546.07;
    const row = HCL_REFRACTIVITY.aPerUm2 / (HCL_REFRACTIVITY.bPerUm2 - (1000 / nm) ** 2);
    expect(Math.abs(row / 44_803e-8 - 1)).toBeLessThan(5e-5);
  });

  it("would need a 22% increment to reach the withdrawn 1.80 at 550 nm", () => {
    expect(nh4shIndex(550, 0.22)).toBeLessThan(1.8);
    expect(nh4shIndex(550, 0.23)).toBeGreaterThan(1.8);
  });

  it("give NH₃ ice 1.465, the method's check on a measured molecular solid", () => {
    // Olovsson and Templeton's cubic cell (Acta Cryst. 12 (1959) 832; COD 2310927), a = 5.138 Å with
    // four molecules; Martonchik et al. 1984 (Appl. Opt. 23, 541) measure 1.436 at 550 nm, 0.029
    // below. The ruling's 1.458 took NH₃'s real-gas Z.
    const n = lorentzLorenzIndex([NH3_REFRACTIVITY], 550, NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL, 0);
    expect(Math.abs(n - 1.464_8)).toBeLessThan(5e-4);
  });

  it("refuse a wavelength at or past a dispersion's pole", () => {
    // NH₃'s pole is at λ⁻² = 90.392 µm⁻², about 105 nm.
    expect(() => nh4shIndex(100)).toThrow(RangeError);
  });

  it("name a file for every material, each key's phase and variant once", () => {
    const files = [...FETCHED_MATERIALS, ...COMBINED_MATERIALS].map((spec) => spec.file);
    expect(new Set(files).size).toBe(files.length);
    const identities = [
      ...FETCHED_MATERIALS.map((s) => s.header),
      ...COMBINED_MATERIALS.map((s) => s.header),
      ...DERIVED_MATERIALS,
    ].map((h) => `${h.key}/${h.phase}/${h.variant ?? ""}`);
    expect(new Set(identities).size).toBe(identities.length);
  });

  it("ask each source's path once, the 75 wt% acid's for both files made from it", () => {
    const options = sourceOptions().map((input) => input.option);
    expect(new Set(options).size).toBe(options.length);
    expect(FETCHED_MATERIALS.filter((spec) => spec.option === "h2so4-75")).toHaveLength(2);
  });
});

describe("R08.T5.d's parsers", () => {
  it("reads a refractiveindex.info tabulated n block with k = 0", () => {
    const text = [
      "DATA:",
      "  - type: tabulated n",
      "    data: |",
      "        0.3612 1.2773",
      "        0.6439 1.2693",
      "CONDITIONS:",
    ].join("\n");
    expect(parseRefractiveIndexInfoN(text)).toEqual([
      { wavelengthUm: 0.3612, n: 1.2773, k: 0 },
      { wavelengthUm: 0.6439, n: 1.2693, k: 0 },
    ]);
  });

  it("evaluates a formula 1 entry: Malitson 1965's fused silica, 1.4585 at the helium d line", () => {
    // I. H. Malitson, J. Opt. Soc. Am. 55 (1965) 1205: n = 1.45846 at 587.56 nm.
    const silica = parseRefractiveIndexInfoFormula(
      formula(1, "0.21 6.7", "0 0.6961663 0.0684043 0.4079426 0.1162414 0.8974794 9.896161"),
    );
    expect(silica.type).toBe(1);
    expect(formulaIndex(silica, 0.58756)).toBeCloseTo(1.45846, 4);
  });

  it("evaluates a formula 2 entry: Ghosh 1999's quartz, n_o 1.5443 at 589.3 nm", () => {
    // Ghosh 1999's ordinary ray; quartz's n_ω at the sodium D lines is 1.5443.
    const quartz = parseRefractiveIndexInfoFormula(
      formula(2, "0.198 2.0531", "0.28604141 1.07044083 1.00585997e-2 1.10202242 100"),
    );
    expect(formulaIndex(quartz, 0.5893)).toBeCloseTo(1.5443, 3);
  });

  it("evaluates a formula 4 entry as DeVore 1951's rutile Sellmeier form", () => {
    const rutile = parseRefractiveIndexInfoFormula(
      formula(4, "0.43 1.53", "5.913 0.2441 0 0.0803 1 0 0 0 1"),
    );
    for (const um of [0.45, 0.55, 0.75]) {
      expect(formulaIndex(rutile, um)).toBeCloseTo(
        Math.sqrt(5.913 + 0.2441 / (um * um - 0.0803)),
        12,
      );
    }
  });

  it("refuses a wavelength outside a formula's stated range", () => {
    const rutile = parseRefractiveIndexInfoFormula(
      formula(4, "0.43 1.53", "5.913 0.2441 0 0.0803 1 0 0 0 1"),
    );
    expect(() => formulaIndex(rutile, 0.38)).toThrow(RangeError);
  });

  it("reads ARIA's wavelength format, NaN in n or k kept", () => {
    const rows = parseAria("#FORMAT=WAVL N K\n0.38\tNaN\t0.00087\n0.5\t1.52\t0.00092\n");
    expect(rows[0]?.wavelengthUm).toBe(0.38);
    expect(rows[0]?.n).toBeNaN();
    expect(rows[1]).toEqual({ wavelengthUm: 0.5, n: 1.52, k: 0.00092 });
  });

  it("reads Kitzmann and Heng's three header lines and checks the count", () => {
    const head = "#Refractive index of X[s]; someone\n#Data points: 2\n#Wavelength (micron) n  k\n";
    expect(parseKitzmannHeng(`${head}0.46\t1.31\t1.33\n0.61\t1.48\t1.48\n`)).toHaveLength(2);
    expect(() => parseKitzmannHeng(`${head}0.46\t1.31\t1.33\n`)).toThrow(/2 rows/u);
  });

  it("reads Draine's dielectric columns", () => {
    const text = [
      " ICOMP=18: Draine 2003 graphite E parallel to c",
      "  wave(um)      eps_1-1    eps_2      Re(n)-1     Im(n)",
      "5.500621E-01  4.4513E+00 1.1933E+00  1.3486E+00 2.5405E-01",
    ].join("\n");
    expect(parseDraineDielectric(text)).toEqual([
      { wavelengthUm: 0.5500621, epsilonRe: 5.4513, epsilonIm: 1.1933 },
    ]);
  });

  it("averages graphite's two axes as Kitzmann and Heng's C.dat: n 2.567, k 1.189 at 550 nm", () => {
    // Draine 2003's callindex.out_CpeD03_0.01 and _CpaD03_0.01 at 550.06 nm: ε_⊥ − 1 = 4.0395,
    // ε₂ = 8.5592; ε_∥ − 1 = 4.4513, ε₂ = 1.1933 (asserted values).
    const [row] = averagedUniaxial(
      draineRow(5.0395, 8.5592),
      draineRow(5.4513, 1.1933),
      [550, 550],
    );
    expect(row?.n).toBeCloseTo(2.567, 3);
    expect(row?.k).toBeCloseTo(1.189, 3);
  });

  it("takes n + ik = √ε with both parts positive", () => {
    const { n, k } = indexOfDielectric(-3, 4);
    expect(n * n - k * k).toBeCloseTo(-3, 12);
    expect(2 * n * k).toBeCloseTo(4, 12);
    expect(n).toBeGreaterThan(0);
  });

  it("refuses two axes on different wavelengths", () => {
    const one = [{ wavelengthUm: 0.55, epsilonRe: 2, epsilonIm: 0 }];
    const other = [{ wavelengthUm: 0.56, epsilonRe: 2, epsilonIm: 0 }];
    expect(() => averagedUniaxial(one, other, [550, 550])).toThrow(/differ/u);
  });

  it("reads one sample's k from Martikainen et al.'s tables, with n their fixed 1.5", () => {
    const text = [
      "Table 6 ",
      "",
      "lambda (nm)\tk _JSC1 (GRS2)\tk _MMS2 (GRS2)\tk _MGS1 (GRS2)\t",
      "650\t0.000643 +or- 0.00013\t0.000338 +or- 0.00009\t0.000433 +or- 0.00010\t",
    ].join("\n");
    expect(parseMartikainenK(text, "MGS1")).toEqual([{ wavelengthUm: 0.65, n: 1.5, k: 0.000433 }]);
    expect(() => parseMartikainenK(text, "XYZ")).toThrow(/XYZ/u);
  });

  it("reads Braude et al.'s first solution, with n the paper's 1.4", () => {
    const tex = [
      "\\begin{table}",
      "\\begin{tabular}{|c|c|c|c|c|}",
      "Wavelength (nm) & \\multicolumn{4}{c|}{$k_{c}$}\\tabularnewline",
      "450 & $0.033\\pm0.003$ & $0.038\\pm0.003$ & $0.048\\pm0.005$ & $0.045\\pm0.004$\\tabularnewline",
      "\\end{tabular}",
      "\\label{imagritable}",
      "\\end{table}",
    ].join("\n");
    expect(parseBraudeChromophore(tex)).toEqual([{ wavelengthUm: 0.45, n: 1.4, k: 0.033 }]);
  });

  it("takes the geometric mean of Irwin et al.'s two planets' Aerosol-3 Mean 1", () => {
    const rows = parseIrwinAerosol3(
      `${irwinTable("tab:refuranus", "1.50")}\n${irwinTable("tab:refneptune", "10.1")}`,
    );
    expect(rows).toHaveLength(1);
    expect(rows[0]?.n).toBe(1.4);
    expect(rows[0]?.k).toBeCloseTo(Math.sqrt(1.5e-3 * 10.1e-3), 15);
  });
});

describe("resampling on a file's own grid and its ends (decision-r08-t5d ruling 1)", () => {
  // The 75 wt% acid's rows near the visible, as in the T5.b test above.
  const acid = [
    { wavelengthUm: 0.3597, n: 1.452, k: Number.NaN },
    { wavelengthUm: 0.4082, n: 1.44, k: Number.NaN },
    { wavelengthUm: 0.5556, n: 1.431, k: Number.NaN },
    { wavelengthUm: 0.7018, n: 1.428, k: 2.07e-8 },
    { wavelengthUm: 0.82, n: 1.427, k: 8e-8 },
  ];

  it("reads the 359.7 nm row on the 360 nm grid and gives the 380 nm grid's values from 380 nm", () => {
    const options = { holdNm: 0, unmeasuredK: "shortestMeasured" } as const;
    const onAcidGrid = resample(acid, { ...options, grid: ACID_GRID_NM });
    const onGrid = resample(acid, options);
    expect(onAcidGrid.n).toHaveLength(ACID_GRID_NM.length);
    expect(onAcidGrid.n.slice(4)).toEqual(onGrid.n);
    expect(onAcidGrid.k.slice(4)).toEqual(onGrid.k);
    expect(onAcidGrid.n[0]).toBeCloseTo(1.452 + ((0.36 - 0.3597) / (0.4082 - 0.3597)) * -0.012, 5);
  });

  it("reads n and k from their own rows where the source gives each at some rows only", () => {
    const rows = [
      { wavelengthUm: 0.27, n: 1.52, k: Number.NaN },
      { wavelengthUm: 0.32, n: Number.NaN, k: 0.00097 },
      { wavelengthUm: 0.5, n: 1.52, k: 0.00092 },
      { wavelengthUm: 0.8, n: Number.NaN, k: 0.0013 },
      { wavelengthUm: 1, n: 1.51, k: 0.0016 },
    ];
    const { n, k } = resample(rows, { holdNm: 0, separateNK: true });
    expect(n[MATERIAL_GRID_NM.indexOf(500)]).toBe(1.52);
    expect(n[MATERIAL_GRID_NM.indexOf(750)]).toBeCloseTo(1.515, 6);
    expect(k[MATERIAL_GRID_NM.indexOf(500)]).toBe(0.00092);
  });

  it("extrapolates k linearly in log k beyond the source's end, n held, when asked", () => {
    const rows = [
      { wavelengthUm: 0.45, n: 1.4, k: 0.033 },
      { wavelengthUm: 0.5, n: 1.4, k: 0.023 },
      { wavelengthUm: 0.95, n: 1.4, k: 0.007 },
    ];
    const held = resample(rows, { holdNm: 70 });
    const extrapolated = resample(rows, { holdNm: 70, endK: "extrapolate" });
    expect(held.k[0]).toBe(0.033);
    expect(extrapolated.k[0]).toBeCloseTo(0.033 * (0.023 / 0.033) ** -1.4, 6);
    expect(extrapolated.n[0]).toBe(1.4);
  });
});

describe("acid_uv_absorber's k_UA (science-r08-unowned-absorbers; decision-r08-t5d ruling 4)", () => {
  it("holds the a-priori k₃₆₅, 1.2 × 10⁻³", () => {
    expect(ACID_UV_K365).toBe(1.2e-3);
  });

  it("holds L at Spacek et al.'s bound, 26.9 nm", () => {
    // Their absorption coefficients, 1278 cm⁻¹ at 375 nm and under 54 cm⁻¹ at 455 nm, bound
    // k = αλ ÷ 4π's decay length by 80 nm ÷ ln((1278 × 375) ÷ (54 × 455)); the ruling's
    // shorthand, ln(1278 ÷ 54), would give 25.3 nm.
    expect(ACID_UV_DECAY_NM).toBeCloseTo(80 / Math.log((1278 * 375) / (54 * 455)), 1);
  });

  it("is k₃₆₅ at 365 nm", () => {
    expect(acidUvAbsorberK(365)).toBe(1.2e-3);
  });

  it("decays to its value at 500 nm", () => {
    expect(acidUvAbsorberK(500)).toBeCloseTo(1.2e-3 * Math.exp(-135 / 26.9), 18);
  });

  it("joins its taper continuously at 500 nm", () => {
    expect(acidUvAbsorberK(500 + 1e-9)).toBeCloseTo(acidUvAbsorberK(500), 15);
  });

  it("is half the decay at 525 nm, midway down the taper", () => {
    expect(acidUvAbsorberK(525)).toBeCloseTo(0.5 * 1.2e-3 * Math.exp(-160 / 26.9), 18);
  });

  it("is exactly 0 from 550 nm", () => {
    expect([550, 551, 780].map(acidUvAbsorberK)).toEqual([0, 0, 0]);
  });

  it("is the committed file's k less the 75 wt% file's, to six figures, and adds nothing to n", () => {
    const absorber = committed("sulphuric-acid-uv-absorber.json");
    const acid = committed("sulphuric-acid-75.json");
    expect(absorber.wavelengthsNm).toEqual([...ACID_GRID_NM]);
    expect(absorber.n).toEqual(acid.n);
    for (const [i, nm] of ACID_GRID_NM.entries()) {
      // Six significant figures: half a unit in the sixth, at most 5 × 10⁻⁶ of the value.
      const k = absorber.k[i] ?? Number.NaN;
      expect(Math.abs(k - (acid.k[i] ?? Number.NaN) - acidUvAbsorberK(nm))).toBeLessThanOrEqual(
        5e-6 * k,
      );
    }
  });
});

/** A committed material file's value at a wavelength on its grid, nm. */
function committedAt(file: string, field: "n" | "k", nm: number): number {
  const values = committed(file);
  return values[field][values.wavelengthsNm.indexOf(nm)] ?? Number.NaN;
}

/** The normal-incidence reflectance of a surface of index n + ik under vacuum. */
function normalReflectance(n: number, k: number): number {
  return ((n - 1) ** 2 + k ** 2) / ((n + 1) ** 2 + k ** 2);
}

/** The absorption length λ ÷ 4πk, µm, at a wavelength, nm. */
function absorptionLengthUm(k: number, nm: number): number {
  return (nm * 1e-3) / (4 * Math.PI * k);
}

/** A row of Martonchik et al.'s Table I by its wavenumber, cm⁻¹. */
function tableIRow(wavenumberPerCm: number): (typeof MARTONCHIK_TABLE_I_ROWS)[number] {
  const row = MARTONCHIK_TABLE_I_ROWS.find((r) => r.wavenumberPerCm === wavenumberPerCm);
  if (row === undefined) {
    throw new Error(`no Table I row at ${wavenumberPerCm} cm⁻¹`);
  }
  return row;
}

describe("the solids' visible absorption (R08.T5.i, decision-p14-t49c-split §5.3)", () => {
  const ammonia = FETCHED_MATERIALS.find((spec) => spec.file === "ammonia-ice.json");
  if (ammonia === undefined) {
    throw new Error("the tool has no ammonia-ice.json entry");
  }
  const lo = tableIRow(6840);
  const hi = tableIRow(31_250);
  const visible = [tableIRow(14_976.7), tableIRow(23_113.3)];

  it("sets k to 0 and keeps n as reduced", () => {
    const reduced = { n: [1.46729, 1.43606], k: [7.82362e-6, 2.88714e-5] };
    expect(withoutVisibleAbsorption(reduced)).toEqual({ n: reduced.n, k: [0, 0] });
  });

  it("gives NH3 ice k = 0 at every grid point and the reduced n, as a stand-in", () => {
    // Table I's rows at 320, 433, 668 and 1,462 nm, in µm as optool's copy lists them.
    const lnk =
      "# rows\n4 0.75\n0.32 1.488 2.546e-6\n0.43265 1.449 2.109e-5\n0.6677 1.423 3.963e-5\n1.46199 1.408 5.817e-5\n";
    const file = reduceSource(ammonia, lnk);
    expect(file.k).toEqual(MATERIAL_GRID_NM.map(() => 0));
    expect(file.n).toEqual(resample(parseLnk(lnk), ammonia.resample).n);
    expect(file.provenance).toBe("standIn");
  });

  it("finds Table I's 668 and 433 nm rows at one and two thirds of the 1,462–320 nm interval", () => {
    expect(visible.map((row) => wavenumberFraction(lo, hi, row.wavenumberPerCm))).toEqual([
      expect.closeTo(1 / 3, 5),
      expect.closeTo(2 / 3, 5),
    ]);
  });

  it("finds those rows' k the linear fill in k to four figures, not the fill in log k", () => {
    const fills = visible.map((row) => wavenumberFill(lo, hi, row.wavenumberPerCm));
    expect(fills.map((fill) => Number(fill.kLinear.toPrecision(4)))).toEqual(
      visible.map((row) => row.k),
    );
    expect(fills.map((fill) => Number(fill.kLogLinear.toPrecision(3)))).toEqual([2.05e-5, 7.22e-6]);
  });

  it("finds Table I's rows from 200 to 320 nm on one straight line in wavelength, to their last figure", () => {
    const ultraviolet = MARTONCHIK_TABLE_I_ROWS.filter((row) => row.wavenumberPerCm >= 31_250);
    expect(ultraviolet).toHaveLength(8);
    // k is printed to four figures, a unit of 10⁻⁹ here; the 210 nm row lies half a unit off,
    // the line's 1.6715 × 10⁻⁶ printed 1.671.
    const off = ultraviolet.filter(
      (row) => Math.abs(row.k - martonchikUltravioletLineK(1e7 / row.wavenumberPerCm)) >= 1e-9,
    );
    expect(off).toEqual([]);
  });

  it("finds the visible rows' n off the straight line in n, 1.4347 and 1.4613, so n is no fill", () => {
    const line = visible.map((row) => wavenumberFill(lo, hi, row.wavenumberPerCm).nLinear);
    expect([line.map((n) => n.toFixed(4)), visible.map((row) => row.n)]).toEqual([
      ["1.4347", "1.4613"],
      [1.423, 1.449],
    ]);
  });

  it("states the fill's figures, the line and T5.b's absorption length in ammonia-ice.json", () => {
    const length = absorptionLengthUm(T5B_AMMONIA_ICE_550.k, 550);
    expect(length / 1000).toBeCloseTo(1.516, 3);
    const reduction = committedText("ammonia-ice.json", "reduction");
    for (const text of [
      "3.9629 × 10⁻⁵ and 2.1087 × 10⁻⁵",
      "2.05 × 10⁻⁵ and 7.22 × 10⁻⁶",
      "k = 1.592 × 10⁻⁶ + 7.95 × 10⁻⁹ (λ − 200 nm)",
      "1.4347 and 1.4613",
      `k was 2.88714 × 10⁻⁵ at 550 nm, an absorption length of ${(length / 1000).toFixed(1)} mm`,
    ]) {
      expect(reduction).toContain(text);
    }
  });

  it("states CaTiO3's figures from its own index: reflectance blind to its k, a section not colourless", () => {
    const file = "calcium-titanate.json";
    const [n, k] = [committedAt(file, "n", 550), committedAt(file, "k", 550)];
    const length = [450, 550].map((nm) => absorptionLengthUm(committedAt(file, "k", nm), nm));
    const through30 = length.map((um) => Math.exp(-30 / um));
    const figures = [
      normalReflectance(n, 0).toFixed(7),
      normalReflectance(n, k).toFixed(7),
      Number((normalReflectance(n, k) - normalReflectance(n, 0)).toPrecision(2)),
      `${(100 * (through30[0] ?? Number.NaN)).toFixed(1)}% at 450 nm and ${(100 * (through30[1] ?? Number.NaN)).toFixed(0)}% at 550 nm`,
      `${(length[0] ?? Number.NaN).toFixed(1)} and ${(length[1] ?? Number.NaN).toFixed(0)} µm`,
    ];
    expect(figures).toEqual([
      "0.1598728",
      "0.1598730",
      2.5e-7,
      "2.1% at 450 nm and 29% at 550 nm",
      "7.8 and 24 µm",
    ]);
    const reduction = committedText(file, "reduction");
    for (const text of [
      "0.1598728 with k = 0 and 0.1598730",
      "2.5 × 10⁻⁷",
      figures[3],
      figures[4],
    ]) {
      expect(reduction).toContain(text);
    }
  });

  it("states CaTiO3's visible span below its band gap: 3.26–1.59 eV", () => {
    // hc = 1239.84198 eV nm (CODATA 2018's exact h, c and e).
    const ev = [380, 780].map((nm) => (1239.841_98 / nm).toFixed(2));
    expect(ev).toEqual(["3.26", "1.59"]);
    expect(committedText("calcium-titanate.json", "reduction")).toContain("3.26–1.59 eV");
  });

  it("states Na2S's figures from its own index: a flat k, a 30 µm crystal not clear", () => {
    const file = "sodium-sulphide.json";
    const length = absorptionLengthUm(committedAt(file, "k", 550), 550);
    const { wavelengthsNm, k } = committed(file);
    const red = k.filter((_, i) => (wavelengthsNm[i] ?? 0) >= 600);
    expect([
      Math.exp(-30 / length).toExponential(1),
      length.toFixed(1),
      committedAt(file, "k", 550).toPrecision(3),
      Math.min(...red).toPrecision(3),
      Math.max(...red).toPrecision(3),
    ]).toEqual(["4.4e-5", "3.0", "0.0146", "0.0126", "0.0127"]);
    const reduction = committedText(file, "reduction");
    for (const text of [
      "1.46 × 10⁻² at 550 nm and 1.26–1.27 × 10⁻² over 600–780 nm",
      "would transmit 4.4 × 10⁻⁵ at 550 nm, an absorption length of 3.0 µm",
    ]) {
      expect(reduction).toContain(text);
    }
  });
});

describe("the fetched files' headers", () => {
  const FIELDS = [
    "name",
    "provenance",
    "paper",
    "source",
    "licence",
    "reduction",
    "standIn",
  ] as const;

  it.each(FETCHED_MATERIALS.map((spec) => [spec.file, spec] as const))(
    "write %s's header as the tool states it",
    (file, spec) => {
      // The by-hand regeneration's text half, which needs no fetched source.
      const raw: unknown = JSON.parse(
        readFileSync(fileURLToPath(new URL(file, MATERIALS)), "utf8"),
      );
      const header = (field: (typeof FIELDS)[number]): unknown =>
        typeof raw === "object" && raw !== null ? Reflect.get(raw, field) : undefined;
      expect(FIELDS.map(header)).toEqual(FIELDS.map((field) => spec.header[field]));
    },
  );
});

describe("the molecular ices' estimates (decision-r08-t5d rulings 3 and 5)", () => {
  it("take each cell's density from its paper, as the database states it", () => {
    // ρ = M ÷ V_m: β-N2 0.992 g cm⁻³ (Schuch and Mills 1970), γ-O2 1.364 (Cox et al. 1973), the
    // CIFs' _exptl_crystal_density_diffrn.
    expect(cellDensity(28.014, N2_ICE_CELL)).toBeCloseTo(0.992, 3);
    expect(cellDensity(31.998, O2_ICE_CELL)).toBeCloseTo(1.364, 3);
  });

  it("hold Yarnall and Hudson's amorphous H2S film: n 1.407 at 670 nm, 0.944 g cm⁻³", () => {
    // Yarnall and Hudson 2022 (Astrophys. J. Lett. 931, L4), Table 1, for ice at 19 K.
    expect(H2S_FILM).toEqual({ wavelengthNm: 670, index: 1.407, densityGPerCm3: 0.944 });
  });

  it("estimate amorphous H2S ice at its film's density above the film's measured n", () => {
    expect(h2sFilmEstimate()).toBeGreaterThan(H2S_FILM.index);
  });

  it("estimate amorphous H2S ice at its film's density within 0.04 of the film's measured n", () => {
    expect(h2sFilmEstimate() - H2S_FILM.index).toBeLessThan(0.04);
  });

  it("estimate solid Ar within 0.015 above Sinnock and Smith's measured 1.2708 at 546.1 nm", () => {
    const argon = iceIndex(AR_DISPERSION, AR_ICE_CELL, 546.1);
    expect(argon - AR_ICE_MEASURED_546).toBeGreaterThan(0);
    expect(argon - AR_ICE_MEASURED_546).toBeLessThan(0.015);
  });

  it("estimate every ice the registry names without an open measurement", () => {
    expect(ICE_ESTIMATES.map((e) => e.key)).toEqual(["H2S", "N2", "CO", "O2"]);
  });

  it("file each estimate as a non-absorbing crystal stand-in", () => {
    const filed = ICE_ESTIMATES.map((estimate) => {
      const file = DERIVED_MATERIALS.find((f) => f.key === estimate.key);
      return [file?.provenance, file?.shape, file?.k.every((k) => k === 0)];
    });
    expect(filed).toEqual(ICE_ESTIMATES.map(() => ["standIn", "crystal", true]));
  });

  it("state each cell's own terms in its file's licence", () => {
    for (const estimate of ICE_ESTIMATES) {
      const file = DERIVED_MATERIALS.find((f) => f.key === estimate.key);
      expect(file?.licence).toContain(estimate.cell.terms);
    }
  });

  it("take β-N2's and γ-O2's cells on the American Mineralogist Crystal Structure Database's terms", () => {
    expect([N2_ICE_CELL.terms, O2_ICE_CELL.terms].map((t) => t.split(",")[0])).toEqual([
      "from the American Mineralogist Crystal Structure Database",
      "from the American Mineralogist Crystal Structure Database",
    ]);
  });

  it("state each estimate's own reason in its file's standIn", () => {
    for (const estimate of ICE_ESTIMATES) {
      const file = DERIVED_MATERIALS.find((f) => f.key === estimate.key);
      expect(file?.standIn).toContain(`since ${estimate.standInReason}:`);
    }
  });
});

describe("S8's constant index (decision-r08-t5d ruling 5)", () => {
  it("averages the principal indices in the dielectric function: 2.0838", () => {
    expect(S8_PRINCIPAL_INDICES).toEqual([1.9579, 2.0377, 2.2452]);
    expect(S8_STAND_IN_INDEX).toBeCloseTo(2.0838, 4);
  });
});

/**
 * The published dispersion formulas of the four formula files, as refractiveindex.info (CC0)
 * gives each entry: Li 1976's KCl and NaCl (formula 1), Ghosh 1999's quartz (formula 2) and
 * Malitson and Dodge 1972's sapphire (formula 1), each ray.
 */
const FORMULAS = {
  kcl: formula(1, "0.18 35", "0.26486 0.30523 0.100 0.41620 0.131 0.18870 0.162 2.6200 70.42"),
  nacl: formula(
    1,
    "0.2 30",
    "0.00055 0.19800 0.050 0.48398 0.100 0.38696 0.128 0.25998 0.158 0.08796 40.50 3.17064 60.98 0.30038 120.34",
  ),
  quartzO: formula(2, "0.198 2.0531", "0.28604141 1.07044083 1.00585997e-2 1.10202242 100"),
  quartzE: formula(2, "0.198 2.0531", "0.28851804 1.09509924 1.02101864e-2 1.15662475 100"),
  sapphireO: formula(
    1,
    "0.20 5.0",
    "0 1.4313493 0.0726631 0.65054713 0.1193242 5.3414021 18.028251",
  ),
  sapphireE: formula(
    1,
    "0.20 5.0",
    "0 1.5039759 0.0740288 0.55069141 0.1216529 6.5927379 20.072248",
  ),
} as const;

describe("the formula files", () => {
  const i550 = MATERIAL_GRID_NM.indexOf(550);

  it("evaluate a formula at every grid wavelength, to six figures, with k = 0", () => {
    const { n, k } = formulaOnGrid(FORMULAS.kcl);
    expect(n).toHaveLength(MATERIAL_GRID_NM.length);
    expect(k.every((v) => v === 0)).toBe(true);
    // Li's KCl at 550 nm.
    expect(n[i550]).toBe(1.49272);
  });

  it("average a uniaxial crystal's two rays in ε, n² = (2n_o² + n_e²) ÷ 3", () => {
    const { n } = uniaxialFormulasOnGrid(FORMULAS.quartzO, FORMULAS.quartzE);
    const ordinary = formulaIndex(parseRefractiveIndexInfoFormula(FORMULAS.quartzO), 0.55);
    const extraordinary = formulaIndex(parseRefractiveIndexInfoFormula(FORMULAS.quartzE), 0.55);
    expect(n[i550]).toBeCloseTo(
      Math.sqrt((2 * ordinary * ordinary + extraordinary * extraordinary) / 3),
      5,
    );
    // Quartz's n_ω 1.5460 and n_ε 1.5551 at 550 nm average to 1.5490.
    expect(n[i550]).toBeCloseTo(1.549, 4);
  });

  it.each([
    ["potassium-chloride.json", () => formulaOnGrid(FORMULAS.kcl)],
    ["sodium-chloride.json", () => formulaOnGrid(FORMULAS.nacl)],
    ["silicon-dioxide.json", () => uniaxialFormulasOnGrid(FORMULAS.quartzO, FORMULAS.quartzE)],
    ["aluminium-oxide.json", () => uniaxialFormulasOnGrid(FORMULAS.sapphireO, FORMULAS.sapphireE)],
  ])("hold the committed %s to its formula", (file, reduce) => {
    const { n, k } = reduce();
    expect(committed(file).n).toEqual([...n]);
    expect(committed(file).k).toEqual([...k]);
  });
});

/** An omlc.org table of Pope and Fry's shape: 380–727.5 nm every 2.5 nm, a in cm⁻¹. */
function omlcTable(perCm: (nm: number) => number, rows = POPE_FRY_ROWS): string {
  const lines = Array.from({ length: rows }, (_, i) => {
    const nm = 380 + 2.5 * i;
    return `${nm}\t${perCm(nm)}`;
  });
  return [
    "R. M. Pope and E. S. Fry, a header line",
    "",
    "lambda\t absorption",
    " (nm) \t   (1/cm)   ",
    ...lines,
  ].join("\n");
}

/** A value on {@link MATERIAL_GRID_NM} at a wavelength, nm. */
function gridValue(values: ReadonlyArray<number>, nm: number): number {
  return values[MATERIAL_GRID_NM.indexOf(nm)] ?? Number.NaN;
}

/** Dry air's index, Peck and Reeder 1972, as `view/atmosphere/rayleigh.ts`'s `DRY_AIR_DISPERSION` holds it. */
function dryAirIndex(wavelengthNm: number): number {
  const s2 = (1_000 / wavelengthNm) ** 2;
  return 1 + (8_060.51 + 2_480_990 / (132.274 - s2) + 17_455.7 / (39.329_57 - s2)) * 1e-8;
}

/** A WOPP v3 table of the file's shape: comment lines, then a, m⁻¹, every 2 nm from 300 nm, and past 510. */
function woppTable(perM: (nm: number) => number, rows = MASON_ROWS): string {
  const lines = Array.from({ length: rows }, (_, i) => {
    const nm = 300 + 2 * i;
    return `${nm}\t${perM(nm)}\t0\t0\t0.0003\t0\t0`;
  });
  return [
    "%\tCombined\tdata\tfor\tpure\twater",
    "%Wavelength\ta\t(m-1),\t20degreeC",
    ...lines,
    "512\t0.5\t0\t0\t0.001\t0\t0",
  ].join("\n");
}

/** A committed file's k at a grid wavelength, nm, as an absorption coefficient a = 4πk ÷ λ, m⁻¹. */
function absorptionAt(file: { wavelengthsNm: number[]; k: number[] }, nm: number): number {
  return ((file.k[file.wavelengthsNm.indexOf(nm)] ?? Number.NaN) * 4 * Math.PI) / (nm * 1e-9);
}

/** An absorption coefficient, m⁻¹, doubling every 10 nm from 10⁻³ at 300 nm: log-linear rows keep it exactly. */
function risingAbsorption(nm: number): number {
  return 1e-3 * 2 ** ((nm - 300) / 10);
}

/** Whether a reduced value lies within the reduction's six significant figures of another. */
function sixFigures(got: number, want: number): boolean {
  return Math.abs(got / want - 1) < 5e-6;
}

/** A two-column table's value at a wavelength, nm, or NaN. */
function tableAt(rows: ReadonlyArray<AbsorptionRow>, nm: number): number {
  return rows.find(([at]) => at === nm)?.[1] ?? Number.NaN;
}

/** Pope and Fry's absorption over Mason et al.'s at a compared wavelength, nm. */
function popeFryOverMason(nm: number): number {
  return tableAt(POPE_FRY_ABSORPTION_PER_M, nm) / tableAt(MASON_2016_ABSORPTION_PER_M, nm);
}

describe("the water files' sources (R08.T5.g, R08.T5.h)", () => {
  it("reads omlc.org's absorption as k = aλ ÷ 4π, with n NaN", () => {
    const rows = parseOmlcAbsorption(
      omlcTable(() => 6.35e-5),
      POPE_FRY_ROWS,
    );
    expect(rows).toHaveLength(POPE_FRY_ROWS);
    const at440 = rows.find((row) => row.wavelengthUm === 0.44);
    expect(at440?.k).toBeCloseTo(absorptionIndex(0.00635, 440), 20);
    expect(Number.isNaN(at440?.n)).toBe(true);
    expect(absorptionIndex(0.00635, 440)).toBeCloseTo(2.2234e-10, 14);
  });

  it("refuses a table of another length or with a wavelength twice", () => {
    expect(() =>
      parseOmlcAbsorption(
        omlcTable(() => 1e-4, 139),
        POPE_FRY_ROWS,
      ),
    ).toThrow(/140 rows/u);
    const doubled = `${omlcTable(() => 1e-4, 139)}\n380\t1e-4`;
    expect(() => parseOmlcAbsorption(doubled, POPE_FRY_ROWS)).toThrow(/twice/u);
  });

  it("reads WOPP v3's Mason rows over 300–510 nm, every 2 nm, the dissertation's two in place", () => {
    const rows = parseWoppMason(woppTable(() => 0.005));
    expect(rows).toHaveLength(MASON_ROWS);
    expect([rows[0]?.[0], rows.at(-1)?.[0]]).toEqual([300, 510]);
    expect(rows.filter(([, perM]) => perM !== 0.005)).toEqual(MASON_DISSERTATION_ROWS);
  });

  it("takes the dissertation's 0.00321 m⁻¹ at 422 nm and 0.00690 at 446 nm", () => {
    expect(MASON_DISSERTATION_ROWS).toEqual([
      [422, 0.00321],
      [446, 0.0069],
    ]);
  });

  it("refuses a WOPP v3 table short of Mason's 106 rows", () => {
    expect(() => parseWoppMason(woppTable(() => 0.005, MASON_ROWS - 1))).toThrow(/106 rows/u);
  });

  it("interpolates an absorption coefficient linearly in log a", () => {
    const rows = [
      [400, 0.001],
      [402, 0.004],
    ] as const;
    expect(logLinearAbsorptionPerM(rows, 401)).toBeCloseTo(0.002, 15);
    expect(logLinearAbsorptionPerM(rows, 400)).toBe(0.001);
  });

  it("refuses an absorption coefficient past or before the rows, or between rows not both positive", () => {
    const rows = [
      [400, 0.001],
      [402, 0.004],
    ] as const;
    expect(() => logLinearAbsorptionPerM(rows, 403)).toThrow(/beyond/u);
    expect(() => logLinearAbsorptionPerM(rows, 399)).toThrow(/before/u);
    const zero = [
      [400, 0],
      [402, 0.004],
    ] as const;
    expect(() => logLinearAbsorptionPerM(zero, 401)).toThrow(/positive/u);
  });

  it("splits water's k at 495/500 nm and 725/730 nm", () => {
    expect(WATER_K_SPANS_NM).toEqual({ mason: [380, 495], popeFry: [500, 725], kou: [730, 780] });
  });

  it("takes water's n from IAPWS R9-97 at every grid point", () => {
    const { n } = waterIndex(
      omlcTable(() => 1e-4),
      woppTable(risingAbsorption),
    );
    expect(n).toEqual(MATERIAL_GRID_NM.map(iapwsWaterN));
  });

  it("takes k from Mason et al. to 495 nm, Pope and Fry to 725 nm and Kou et al. from 730 nm", () => {
    const { k } = waterIndex(
      omlcTable(() => 1e-4),
      woppTable(risingAbsorption),
    );
    expect(sixFigures(gridValue(k, 380), absorptionIndex(risingAbsorption(380), 380))).toBe(true);
    expect(sixFigures(gridValue(k, 495), absorptionIndex(risingAbsorption(495), 495))).toBe(true);
    // 445 nm lies halfway between the 444 nm row and the dissertation's 0.00690 at 446 nm.
    const at445 = absorptionIndex(Math.sqrt(risingAbsorption(444) * 0.0069), 445);
    expect(sixFigures(gridValue(k, 445), at445)).toBe(true);
    expect(sixFigures(gridValue(k, 500), absorptionIndex(0.01, 500))).toBe(true);
    expect(sixFigures(gridValue(k, 725), absorptionIndex(0.01, 725))).toBe(true);
    for (const [nm, perM] of KOU_1993_ABSORPTION_PER_M) {
      expect(sixFigures(gridValue(k, nm), absorptionIndex(perM, nm))).toBe(true);
    }
  });

  it("holds Kou et al.'s values as IOCCG 2018's Table 1.1 lists them, 730–780 nm every 5 nm", () => {
    expect(KOU_1993_ABSORPTION_PER_M).toEqual([
      [730, 1.97],
      [735, 2.51],
      [740, 2.78],
      [745, 2.83],
      [750, 2.85],
      [755, 2.88],
      [760, 2.86],
      [765, 2.86],
      [770, 2.82],
      [775, 2.76],
      [780, 2.69],
    ]);
  });

  it("reproduces IAPWS R9-97's Table 3 at 0 °C and 0.1 MPa", () => {
    // ρ = 999.8431 kg m⁻³, the NIST Chemistry WebBook's IAPWS-95 at 273.16 K and 0.1 MPa, its nearest
    // state to 0 °C (ρ moves by under 10⁻⁶ of itself over 0.01 K).
    expect(iapwsWaterIndex(273.15, 999.8431, 226.5)).toBeCloseTo(1.394_527, 6);
    expect(iapwsWaterIndex(273.15, 999.8431, 589)).toBeCloseTo(1.334_344, 6);
    expect(iapwsWaterIndex(273.15, 999.8431, 1_013.98)).toBeCloseTo(1.326_135, 6);
  });

  it("holds Daimon and Masumura's 24 °C formula as refractiveindex.info gives it", () => {
    const text = formula(
      2,
      "0.182 1.129",
      "0 5.666959820E-1 5.084151894E-3 1.731900098E-1 1.818488474E-2 2.095951857E-2 2.625439472E-2 1.125228406E-1 1.073842352E1",
    );
    expect(parseRefractiveIndexInfoFormula(text)).toEqual(DAIMON_24C);
  });
});

describe("the committed water.json (R08.T5.h, decision-r08-t5g)", () => {
  const water = committed("water.json");
  const text = committedText("water.json", "reduction");

  it("takes n from IAPWS R9-97 at 25 °C within 10⁻⁵ at every grid point: 1.34511, 1.33419, 1.32848", () => {
    for (const [i, nm] of water.wavelengthsNm.entries()) {
      const iapws = iapwsWaterIndex(WATER_TEMPERATURE_K, WATER_DENSITY_KG_PER_M3, nm);
      expect(Math.abs((water.n[i] ?? Number.NaN) - iapws)).toBeLessThan(1e-5);
      expect(water.n[i]).toBe(iapwsWaterN(nm));
    }
    expect([380, 550, 780].map((nm) => gridValue(water.n, nm))).toEqual([
      1.34511, 1.33419, 1.32848,
    ]);
  });

  it.each([
    [380, 0.00143],
    [400, 0.00222],
    [420, 0.00312],
    [440, 0.00522],
  ])("gives k at %d nm as Mason, Cone and Fry's a, %f m⁻¹, to six figures", (nm, perM) => {
    expect(MASON_2016_ABSORPTION_PER_M).toContainEqual([nm, perM]);
    expect(Math.abs(absorptionAt(water, nm) / perM - 1)).toBeLessThan(5e-6);
  });

  it("gives k over 500–725 nm as Pope and Fry's rows, 0.0565 m⁻¹ at 550 nm, to six figures", () => {
    const rows = POPE_FRY_ABSORPTION_PER_M.filter(([nm]) => nm >= 500 && nm <= 725);
    expect(rows).toContainEqual([550, 0.0565]);
    for (const [nm, perM] of rows) {
      expect(Math.abs(absorptionAt(water, nm) / perM - 1)).toBeLessThan(5e-6);
    }
  });

  it("gives k over 730–780 nm as Kou et al.'s, 2.78, 2.85 and 2.69 m⁻¹ at 740, 750 and 780 nm", () => {
    for (const [nm, perM] of KOU_1993_ABSORPTION_PER_M) {
      expect(Math.abs(absorptionAt(water, nm) / perM - 1)).toBeLessThan(5e-6);
    }
  });

  it("meets Pope and Fry at the seam: a at 495 nm within 3% of their 0.0173 m⁻¹", () => {
    expect(POPE_FRY_ABSORPTION_PER_M).toContainEqual([495, 0.0173]);
    expect(Math.abs(absorptionAt(water, 495) / 0.0173 - 1)).toBeLessThan(0.03);
  });

  it("records Pope and Fry's ratio to Mason et al., 7.95 at 380 nm to 0.98 at 500 nm, and 1.04–1.00 over 520–550 nm", () => {
    expect(
      [380, 400, 420, 440, 460, 480, 500].map((nm) => popeFryOverMason(nm).toFixed(2)),
    ).toEqual(["7.95", "2.99", "1.46", "1.22", "1.08", "1.05", "0.98"]);
    expect([520, 540, 550].every((nm) => Math.abs(popeFryOverMason(nm) - 1) < 0.05)).toBe(true);
    expect(text).toContain("runs 7.95 at 380 nm, 2.99 at 400 nm, 1.46 at 420 nm");
    expect(text).toContain("1.04 at 520 nm, 1.00 at 540 nm, 1.00 at 550 nm times Mason et al.'s");
  });

  it("states the change from R08.T5.g's file: n +0.00451, +0.00119 and −0.00132, and k's factors", () => {
    expect(text).toContain("+0.00451 at 380 nm, +0.00119 at 550 nm and −0.00132 at 780 nm");
    const factor = (nm: number, fall: boolean): string => {
      const was = tableAt(T5G_WATER_K, nm);
      const now = gridValue(water.k, nm);
      return `${(fall ? was / now : now / was).toFixed(2)} at ${nm} nm`;
    };
    const falls = [380, 400, 440].map((nm) => factor(nm, true));
    const rises = [740, 750, 780].map((nm) => factor(nm, false));
    expect(falls).toEqual(["7.95 at 380 nm", "2.99 at 400 nm", "1.22 at 440 nm"]);
    expect(text).toContain(
      `k falls by a factor of ${falls.join(", ")}, and rises by ${rises.join(", ")}`,
    );
  });

  it("holds R08.T5.g's record to its sources: Pope and Fry's k to 550 nm, Hale and Querry's at 750 nm", () => {
    for (const nm of [380, 400, 440, 550]) {
      const perM = POPE_FRY_ABSORPTION_PER_M.find(([at]) => at === nm)?.[1] ?? Number.NaN;
      const k = T5G_WATER_K.find(([at]) => at === nm)?.[1] ?? Number.NaN;
      expect(Math.abs(k / absorptionIndex(perM, nm) - 1)).toBeLessThan(5e-6);
    }
    expect(T5G_WATER_K).toContainEqual([750, 1.56e-7]);
    expect(T5G_WATER_N).toContainEqual([550, 1.333]);
  });

  it("finds Daimon and Masumura's 24 °C water within 6 × 10⁻⁶ of IAPWS R9-97 at 24 °C, as the file states", () => {
    for (const nm of [380, 550, 780]) {
      expect(Math.abs(daimonExcess(nm))).toBeLessThan(6e-6);
    }
    expect(text).toContain(
      "lies −5.6 × 10⁻⁶ at 380 nm, +1.1 × 10⁻⁷ at 550 nm, +2.6 × 10⁻⁶ at 780 nm",
    );
  });

  it("states how far reading λ as the air wavelength would lower n: at most 1.2 × 10⁻⁵, under 5 × 10⁻⁶ from 595 nm", () => {
    const fall = (nm: number): number =>
      iapwsWaterIndex(WATER_TEMPERATURE_K, WATER_DENSITY_KG_PER_M3, nm) -
      iapwsWaterIndex(WATER_TEMPERATURE_K, WATER_DENSITY_KG_PER_M3, nm * dryAirIndex(nm));
    const falls = MATERIAL_GRID_NM.map(fall);
    expect([Math.max(...falls).toExponential(1), fall(380)]).toEqual([
      "1.2e-5",
      Math.max(...falls),
    ]);
    const under = MATERIAL_GRID_NM.filter((nm) => fall(nm) < 5e-6);
    expect([under[0], under.length]).toEqual([595, MATERIAL_GRID_NM.length - 43]);
    expect(text).toContain(
      "read as the air wavelength, n would fall by at most 1.2 × 10⁻⁵, at 380 nm, and by under 5 × 10⁻⁶ from 595 nm",
    );
  });

  it("finds Quan and Fry's pure water, times air's index, above IAPWS R9-97 as the file states", () => {
    const excess = (nm: number): number =>
      quanFryIndex(0, 25, nm) * dryAirIndex(nm) -
      iapwsWaterIndex(WATER_TEMPERATURE_K, WATER_DENSITY_KG_PER_M3, nm);
    const inRange = MATERIAL_GRID_NM.filter((nm) => nm >= 400 && nm <= 700).map(excess);
    // The file states each to two figures, ×10⁻⁵.
    const [low, high, at380, at780] = [
      Math.min(...inRange),
      Math.max(...inRange),
      excess(380),
      excess(780),
    ].map((value) => (value * 1e5).toFixed(1));
    expect(text).toContain(
      `lies above it by ${low} × 10⁻⁵ to ${high} × 10⁻⁵ over 400–700 nm, and by ${at380} × 10⁻⁵ at 380 nm and ${at780} × 10⁻⁵ at 780 nm`,
    );
  });

  it("names its sources: IAPWS with its attribution, Mason et al., Pope and Fry, Kou et al. and IOCCG, and not Hale and Querry", () => {
    const paper = committedText("water.json", "paper");
    for (const name of [
      "IAPWS R9-97",
      "J. D. Mason, M. T. Cone and E. S. Fry",
      "R. M. Pope and E. S. Fry",
      "L. Kou, D. Labrie and P. Chýlek",
      "IOCCG Protocol Series",
    ]) {
      expect(paper).toContain(name);
    }
    expect(paper).not.toContain("Hale");
    expect(committedText("water.json", "licence")).toContain(
      "attribution is given to the International Association for the Properties of Water and Steam",
    );
  });
});

describe("R08.T5.g's seawater", () => {
  it("holds Quan and Fry's ten coefficients as the Ocean Optics Web Book gives them", () => {
    expect(QUAN_FRY_COEFFICIENTS).toEqual([
      1.31405, 1.779e-4, -1.05e-6, 1.6e-8, -2.02e-6, 15.868, 0.01155, -0.00423, -4382, 1.1455e6,
    ]);
  });

  it("adds the salinity terms alone, (n₁ + n₂T + n₃T²)S + n₆S ÷ λ", () => {
    const [, n1, n2, n3, , , n6] = QUAN_FRY_COEFFICIENTS;
    const { practicalSalinity: s, temperatureC: t } = SEAWATER;
    for (const nm of [380, 550, 780]) {
      expect(quanFrySalinityIncrement(s, t, nm)).toBeCloseTo(
        (n1 + n2 * t + n3 * t * t) * s + (n6 * s) / nm,
        12,
      );
    }
    expect(quanFrySalinityIncrement(s, t, 550)).toBeCloseTo(0.006_393, 6);
  });

  it("holds the committed seawater.json to water.json's n plus the terms, and water.json's k", () => {
    const water = committed("water.json");
    const sea = committed("seawater.json");
    expect(sea.n).toEqual([...seawaterN(water.n)]);
    expect(sea.k).toEqual(water.k);
    expect(gridValue(sea.n, 550)).toBe(1.34058);
  });

  it("lies within 5 × 10⁻⁵ of Quan and Fry's whole equation at S 35 and 25 °C, times dry air's index, over 400–700 nm", () => {
    const sea = committed("seawater.json");
    const departures = sea.wavelengthsNm
      .map((nm, i) => ({ nm, n: sea.n[i] ?? Number.NaN }))
      .filter(({ nm }) => nm >= 400 && nm <= 700)
      .map(({ nm, n }) => Math.abs(n - quanFryIndex(35, 25, nm) * dryAirIndex(nm)));
    expect(departures).toHaveLength(61);
    expect(Math.max(...departures)).toBeLessThan(5e-5);
  });
});

describe("R08.T5.g's derived liquids", () => {
  it("fits a Cauchy line exactly to points on one, and refuses a single wavelength", () => {
    const points = [0.4, 0.5, 0.7].map((um) => ({ wavelengthUm: um, n: 1.3 + 0.004 / (um * um) }));
    expect(fitCauchy(points)).toEqual({ a: 1.3, bUm2: 0.004 });
    expect(() => fitCauchy([{ wavelengthUm: 0.5, n: 1.3 }])).toThrow(/two distinct/u);
  });

  it("fit liquid N2 to Augustin 1915 within 2.2 × 10⁻⁴, the 656.3 nm value the largest residual", () => {
    const residuals = AUGUSTIN_N2.map(
      (p) => cauchyIndex(N2_LIQUID_FIT, p.wavelengthUm * 1000).n - p.n,
    );
    expect(Math.max(...residuals.map(Math.abs))).toBeLessThan(2.2e-4);
    expect(Math.abs(residuals[0] ?? 0)).toBe(Math.max(...residuals.map(Math.abs)));
  });

  it("meet Wong and Anderson's 1.360 at 632.8 nm for liquid NH3, within 4.5 × 10⁻⁴ of the shifted estimate", () => {
    expect(cauchyIndex(NH3_LIQUID_FIT, NH3_LIQUID_MEASURED.wavelengthNm).n).toBeCloseTo(1.36, 5);
    expect(cauchyResidual(NH3_LIQUID_FIT, ammoniaLiquidLevelled)).toBeLessThan(4.5e-4);
    expect([NH3_LIQUID_FIT.fittedFromUm, NH3_LIQUID_FIT.fittedToUm]).toEqual([0.6328, 0.6328]);
  });

  it("bound liquid NH3's k by Robertson and Williams's α, 6.7 × 10⁻⁷ at 400 nm and 2.8 × 10⁻⁷ at 654 nm, and keep k = 0", () => {
    // Their 0.212 and 0.054 cm⁻¹, Napierian, as k = αλ ÷ 4π.
    expect(absorptionIndex(21.2, 400).toExponential(1)).toBe("6.7e-7");
    expect(absorptionIndex(5.4, 654).toExponential(1)).toBe("2.8e-7");
    expect(committedText("ammonia-liquid.json", "source")).toContain(
      "the table bounds k by 6.7 × 10⁻⁷ at 400 nm and 2.8 × 10⁻⁷ at 654 nm",
    );
    expect(committed("ammonia-liquid.json").k.every((k) => k === 0)).toBe(true);
  });

  it("level liquid C2H6 to Hagermann et al.'s 1.3887 at 670 nm", () => {
    expect(C2H6_LIQUID_MEASURED).toEqual({ wavelengthNm: 670, n: 1.3887, temperatureK: 94 });
    expect(Math.abs(cauchyIndex(C2H6_LIQUID_FIT, 670).n - 1.3887)).toBeLessThan(1e-5);
    expect([C2H6_LIQUID_FIT.fittedFromUm, C2H6_LIQUID_FIT.fittedToUm]).toEqual([0.67, 0.67]);
  });

  it("file liquid C2H6 as a derived Cauchy line with no stand-in and k = 0", () => {
    const ethane = DERIVED_MATERIALS.find((f) => f.key === "C2H6");
    expect([ethane?.provenance, ethane?.fit, ethane?.standIn, ethane?.name]).toEqual([
      "derived",
      C2H6_LIQUID_FIT,
      null,
      "liquid C2H6 at 94 K",
    ]);
    expect(ethane?.k.every((k) => k === 0)).toBe(true);
  });

  it("keep the estimate's slope, within 3.5 × 10⁻⁵ of the estimate shifted to 1.3887, which ran 0.0077 high", () => {
    expect(ethaneLiquidEstimate(550)).toBeCloseTo(1.4, 2);
    expect(ethaneLiquidEstimate(670) - 1.3887).toBeCloseTo(0.0077, 4);
    expect(cauchyResidual(C2H6_LIQUID_FIT, ethaneLiquidLevelled)).toBeLessThan(3.5e-5);
    expect([380, 550, 780].map((nm) => cauchyIndex(C2H6_LIQUID_FIT, nm).n.toFixed(5))).toEqual([
      "1.40798",
      "1.39312",
      "1.38630",
    ]);
  });

  it("check Hagermann et al.'s scale: their pure methane's 1.2872 against methane-liquid.json carried to 94 K, 1.2869", () => {
    const methane = committed("methane-liquid.json");
    expect(gridValue(methane.n, 670)).toBe(CH4_LIQUID_670_NM_N);
    expect(methaneLiquidAt94K()).toBeCloseTo(1.2869, 4);
    expect(titanRow(0, 1, 0).n).toBe(1.2872);
  });

  it("pin the level by ideal mixing of Table 2's rows only to about ±0.004", () => {
    const estimate = ethaneLiquidEstimate(670);
    expect(titanRow(0.96, 0, 0.04).n).toBe(1.3848);
    expect(titanMixtureResidual(titanRow(0.96, 0, 0.04), 1.3887)).toBeCloseTo(-0.0018, 4);
    expect(titanMixtureResidual(titanRow(0.53, 0.47, 0), 1.3887)).toBeCloseTo(-0.0055, 4);
    expect(titanMixtureResidual(titanRow(0.96, 0, 0.04), estimate)).toBeCloseTo(0.0056, 4);
    expect(titanMixtureResidual(titanRow(0.53, 0.47, 0), estimate)).toBeCloseTo(-0.001, 4);
    // The ethane-free row misses as far: part of the spread is the mixing rule's own.
    expect(titanMixtureResidual(titanRow(0, 0.83, 0.17), 1.3887)).toBeCloseTo(-0.0042, 4);
    expect(Math.abs(titanMixtureResidual(titanRow(0, 1, 0), 1.3887))).toBeLessThan(5e-4);
  });

  it("find Burton's Lorentz–Lorenz function 0.0086 above the estimate at 632.8 nm", () => {
    expect(ethaneBurtonIndex()).toBeCloseTo(1.4061, 4);
    expect(ethaneBurtonIndex() - ethaneLiquidEstimate(632.8)).toBeCloseTo(0.0086, 4);
  });

  it("state that Burton's gas value does not reach the liquid: 1.410 at the 96/0/4 row's density, which implies 0.3587", () => {
    expect(committedText("ethane-liquid.json", "source")).toContain(
      "at the 96/0/4 row's 0.6530 g cm⁻³ it would give 1.410, above every measurement, where that row's own index implies 0.3587 cm³ g⁻¹",
    );
  });

  it("find the method above every measured liquid, by 0.0019 to 0.0074", () => {
    const excesses = LIQUID_CHECKS.map(liquidCheckExcess);
    expect(excesses.every((e) => e > 0)).toBe(true);
    expect(Math.min(...excesses)).toBeCloseTo(0.0019, 4);
    expect(Math.max(...excesses)).toBeCloseTo(0.0074, 4);
  });

  it.each([
    ["nitrogen-liquid.json", "N2"],
    ["ammonia-liquid.json", "NH3"],
    ["ethane-liquid.json", "C2H6"],
    ["sulphur-liquid.json", "S8"],
  ])("hold the committed %s to its derivation", (file, key) => {
    const derived = DERIVED_MATERIALS.find((f) => f.key === key && f.phase === "liquid");
    expect(committed(file).n).toEqual(derived?.n);
    expect(committed(file).k).toEqual(derived?.k);
  });

  it("give molten S8 Becquerel 1877's n_D, 1.929, at every wavelength", () => {
    expect(S8_MOLTEN_INDEX_D).toBe(1.929);
    expect(new Set(committed("sulphur-liquid.json").n)).toEqual(new Set([1.929]));
  });

  it("ask for Pope and Fry's table and WOPP v3's once each, for both water files, and Hale and Querry's for none", () => {
    for (const option of ["pope-fry", "mason"]) {
      expect(sourceOptions().filter((input) => input.option === option)).toHaveLength(1);
      expect(
        COMBINED_MATERIALS.filter((spec) => spec.inputs.some((i) => i.option === option)),
      ).toHaveLength(2);
    }
    expect(sourceOptions().some((input) => input.option === "water")).toBe(false);
  });
});

/** T5.k's nine files, by name, with their keys. */
const T5K_FILES = [
  ["argon-liquid.json", "Ar"],
  ["krypton-liquid.json", "Kr"],
  ["xenon-liquid.json", "Xe"],
  ["oxygen-liquid.json", "O2"],
  ["nitrous-oxide-liquid.json", "N2O"],
  ["sulphur-dioxide-liquid.json", "SO2"],
  ["carbon-dioxide-liquid.json", "CO2"],
  ["carbon-monoxide-liquid.json", "CO"],
  ["hydrogen-sulphide-liquid.json", "H2S"],
] as const;

/** The squared misses of Liveing and Dewar's five N₂O lines by the gas scaled by c. */
function n2oMisses(c: number): number {
  return LIVEING_DEWAR_1892_N2O.reduce(
    (sum, r) =>
      sum + (scaledLiquidIndex(N2O_DISPERSION, r.wavelengthUm * 1_000, 27.957_49, c) - r.n) ** 2,
    0,
  );
}

/** Liquid CO₂'s scaled index at 546.1 nm and its file's density, at a factor. */
function co2At546(c: number): number {
  return scaledLiquidIndex(CO2_DISPERSION, 546.1, CO2_LIQUID_DENSITY.molPerL, c);
}

/** A refractiveindex.info `tabulated n` file's text around its rows. */
function tabulatedN(rows: string): string {
  return `DATA:\n  - type: tabulated n\n    data: |\n${rows}\nCONDITIONS:\n`;
}

/**
 * A HITRAN collision-induced absorption file of one O₂–O₂ set, in the format `parseCia` reads (its
 * header's fixed columns, then a wavenumber and a coefficient a line), cm⁻¹ and cm⁵.
 */
function oneSetCia(
  temperatureK: number,
  reference: string,
  points: ReadonlyArray<readonly [number, number]>,
): string {
  const low = points[0]?.[0] ?? 0;
  const high = points.at(-1)?.[0] ?? 0;
  const header =
    "O2-O2".padStart(20) +
    low.toFixed(3).padStart(10) +
    high.toFixed(3).padStart(10) +
    String(points.length).padStart(7) +
    temperatureK.toFixed(1).padStart(7) +
    "1.000E-45".padStart(10) +
    "-.999".padStart(6) +
    "".padStart(21) +
    reference.padStart(3);
  return [
    header,
    ...points.map(([nu, value]) => `${nu.toFixed(3)} ${value.toExponential(4)}`),
  ].join("\n");
}

/** A scaled gas's n on the grid, to six figures. */
function scaledGrid(
  gas: typeof AR_DISPERSION,
  molPerL: number,
  factor: number,
): ReadonlyArray<number> {
  return MATERIAL_GRID_NM.map((nm) =>
    Number(scaledLiquidIndex(gas, nm, molPerL, factor).toPrecision(6)),
  );
}

/** A T5.k file's header as the tool states it: a combined spec's, or a derived liquid's. */
function t5kHeader(file: string, key: string): MaterialHeader {
  const combined = COMBINED_MATERIALS.find((spec) => spec.file === file)?.header;
  const derived = DERIVED_MATERIALS.find((f) => f.key === key && f.phase === "liquid");
  const header = combined ?? derived;
  if (header === undefined) {
    throw new Error(`the tool states no header for ${file}`);
  }
  return header;
}

describe("R08.T5.k's liquids (decision-p14-t49h-gas-index)", () => {
  it("scale a gas's molar refraction by c as the molecular estimate at V_m ÷ c", () => {
    for (const nm of [380, 546.1, 780]) {
      expect(scaledLiquidIndex(O2_DISPERSION, nm, 35.662_63, 0.97)).toBe(
        molecularIceIndex(O2_DISPERSION, nm, 1_000 / 35.662_63 / 0.97),
      );
    }
    expect(scaledLiquidIndex(CO_DISPERSION, 550, 30.329_71, 1)).toBe(
      molecularIceIndex(CO_DISPERSION, 550, 1_000 / 30.329_71),
    );
  });

  it("form the gas row's molar refraction as the molecular estimate does: x = R ÷ V_m", () => {
    const r = gasRowMolarRefractionCm3PerMol(AR_DISPERSION, 546.1);
    const x = r / 28.2;
    expect(molecularIceIndex(AR_DISPERSION, 546.1, 28.2)).toBe(Math.sqrt((1 + 2 * x) / (1 - x)));
  });

  it("find the least-squares factor: exactly on points made at one, and at a minimum of the misses", () => {
    const made = [400, 500, 650].map((nm) => ({
      wavelengthUm: nm / 1_000,
      n: scaledLiquidIndex(N2O_DISPERSION, nm, 27.957_49, 0.97),
    }));
    expect(leastSquaresFactor(N2O_DISPERSION, 27.957_49, made)).toBeCloseTo(0.97, 12);
    const c = leastSquaresFactor(N2O_DISPERSION, 27.957_49, LIVEING_DEWAR_1892_N2O);
    expect(n2oMisses(c)).toBeLessThan(n2oMisses(c + 1e-6));
    expect(n2oMisses(c)).toBeLessThan(n2oMisses(c - 1e-6));
  });

  it("refuse a least-squares factor with no points", () => {
    expect(() => leastSquaresFactor(N2O_DISPERSION, 27.957_49, [])).toThrow(/no points/u);
  });

  it("set O2's factor to meet Johns and Wilhelm's 1.2242 at 546.1 nm: 0.986155", () => {
    expect(O2_LIQUID_FACTOR).toBe(0.986_155);
    const n = scaledLiquidIndex(O2_DISPERSION, 546.1, O2_LIQUID_DENSITY.molPerL, O2_LIQUID_FACTOR);
    expect(Math.abs(n - 1.2242)).toBeLessThan(1e-6);
    expect(O2_LIQUID_DENSITY.molPerL).toBe(35.662_63);
  });

  it("fit N2O's factor to Liveing and Dewar 1892's five lines: 0.969096, every line within 5 × 10⁻⁴", () => {
    expect(N2O_LIQUID_FACTOR).toBe(0.969_096);
    for (const r of LIVEING_DEWAR_1892_N2O) {
      const n = scaledLiquidIndex(
        N2O_DISPERSION,
        r.wavelengthUm * 1_000,
        N2O_LIQUID_DENSITY.molPerL,
        N2O_LIQUID_FACTOR,
      );
      expect(Math.abs(n - r.n)).toBeLessThan(5e-4);
    }
  });

  it("find Liveing and Dewar's lithium line's printed 1.3257 against its own deviation, which gives 1.3287", () => {
    // Their prism, 59° 15′ (p. 208), at the lithium line's 22° 52′; the five other lines' printed
    // pairs imply that angle to 0.015° (0.9′), the lithium line's 59.68°.
    const fromDeviation = prismIndex(
      LIVEING_DEWAR_PRISM_ARCMIN,
      LIVEING_DEWAR_1892_N2O_LITHIUM.deviationArcmin,
    );
    expect(fromDeviation.toFixed(4)).toBe("1.3287");
    expect(LIVEING_DEWAR_1892_N2O_LITHIUM.n).toBe(1.3257);
    for (const r of LIVEING_DEWAR_1892_N2O) {
      expect(Math.abs(prismApexArcmin(r) - LIVEING_DEWAR_PRISM_ARCMIN)).toBeLessThan(0.9);
      expect(
        Math.abs(prismIndex(LIVEING_DEWAR_PRISM_ARCMIN, r.deviationArcmin) - r.n),
      ).toBeLessThan(1.5e-4);
    }
    expect(prismApexArcmin(LIVEING_DEWAR_1892_N2O_LITHIUM) / 60).toBeCloseTo(59.68, 2);
  });

  it("set SO2's factor to the mean of the three molar-refraction ratios: +0.13%, −0.73% and −1.10%, 0.994335", () => {
    expect(
      SO2_LIQUID_CHECKS.map((check) => (100 * (molarRefractionRatio(check) - 1)).toFixed(2)),
    ).toEqual(["-1.10", "-0.73", "0.13"]);
    expect(SO2_LIQUID_FACTOR).toBe(0.994_335);
  });

  it("set CO2's factor by Phillips's law at 1.17843 g cm⁻³: 0.976672, −0.0072 in n at 546.1 nm", () => {
    expect(co2DensityGPerCm3(CO2_LIQUID_DENSITY.molPerL).toFixed(5)).toBe("1.17843");
    expect(CO2_LIQUID_FACTOR).toBe(0.976_672);
    expect((co2At546(CO2_LIQUID_FACTOR) - co2At546(1)).toFixed(4)).toBe("-0.0072");
  });

  it("leave the gas's molar refraction unscaled at Phillips's zero density", () => {
    expect(phillipsFactor(0)).toBe(1);
  });

  it("find Phillips's ρ → 0 limit, 6.687 cm³ mol⁻¹, within 0.1% of the gas row's at 546.1 nm", () => {
    const limit = CO2_MOLAR_MASS_G_PER_MOL / PHILLIPS_1920.limitGPerCm3;
    expect(limit.toFixed(3)).toBe("6.687");
    expect(
      Math.abs(limit / gasRowMolarRefractionCm3PerMol(CO2_DISPERSION, 546.1) - 1),
    ).toBeLessThan(1e-3);
  });

  it("find the sim's carry of liquid CO2 at constant Lorentz–Lorenz function within 0.003 of the form at warm states", () => {
    for (const molPerL of [...BLEEKRODE_CO2.map((m) => m.molPerL), CO2_CRITICAL_MOL_PER_L]) {
      expect(Math.abs(co2CarryMiss(molPerL, 589.3))).toBeLessThan(3e-3);
    }
    // At the file's own density the carry is the file, the form there unrounded: c's sixth figure.
    expect(Math.abs(co2CarryMiss(CO2_LIQUID_DENSITY.molPerL, 589.3))).toBeLessThan(1e-6);
  });

  it("fit Ar's factor to Sinnock and Smith's nine rows: 0.990053, the worst row 1.51 × 10⁻⁴ at 578.0 nm", () => {
    expect(AR_LIQUID_FACTOR).toBe(0.990_053);
    const misses = AR_LIQUID_ROWS.map((row) =>
      Math.abs(
        scaledLiquidIndex(
          AR_DISPERSION,
          row.wavelengthUm * 1_000,
          AR_LIQUID_DENSITY.molPerL,
          AR_LIQUID_FACTOR,
        ) - row.n,
      ),
    );
    expect(Math.max(...misses)).toBeCloseTo(1.51e-4, 6);
    expect(misses.indexOf(Math.max(...misses))).toBe(7);
    expect(arWorstRow().wavelengthNm).toBeCloseTo(578, 9);
    expect(arWorstRow().miss).toBe(Math.max(...misses));
  });

  it.each([
    ["Kr", KR_LIQUID_ROWS, KR_LIQUID_TAIL, 1.295_77, 0.002_208_01, 1.2994],
    ["Xe", XE_LIQUID_ROWS, XE_LIQUID_TAIL, 1.377_05, 0.004_346_56, 1.3842],
  ] as const)(
    "fit %s's red tail to its rows from 435.8 nm, each within 3 × 10⁻⁴ and a step under 3 × 10⁻⁴ at 643.9 nm",
    (_, rows, tail, a, b, at780) => {
      expect(tail).toEqual({ a, bUm2: b });
      expect(redTailWorst(rows, tail)).toBeLessThan(3e-4);
      expect(Math.abs(redTailStep(rows, tail))).toBeLessThan(3e-4);
      expect(redTailIndex(tail, 780).toFixed(4)).toBe(at780.toFixed(4));
    },
  );

  it("find Kr's and Xe's tails' worst rows 1.1 × 10⁻⁴ and 1.8 × 10⁻⁴, their steps −4.5 × 10⁻⁶ and −6.6 × 10⁻⁵", () => {
    expect(redTailWorst(KR_LIQUID_ROWS, KR_LIQUID_TAIL).toExponential(1)).toBe("1.1e-4");
    expect(redTailWorst(XE_LIQUID_ROWS, XE_LIQUID_TAIL).toExponential(1)).toBe("1.8e-4");
    expect(redTailStep(KR_LIQUID_ROWS, KR_LIQUID_TAIL).toExponential(1)).toBe("-4.5e-6");
    expect(redTailStep(XE_LIQUID_ROWS, XE_LIQUID_TAIL).toExponential(1)).toBe("-6.6e-5");
  });

  it("find the gas's scaled shape missing Kr's and Xe's tables by over 1 × 10⁻³, so their rows are taken", () => {
    expect(
      scaledShapeMiss(KR_DISPERSION, KR_LIQUID_DENSITY.molPerL, KR_LIQUID_ROWS),
    ).toBeGreaterThan(1e-3);
    expect(
      scaledShapeMiss(XE_DISPERSION, XE_LIQUID_DENSITY.molPerL, XE_LIQUID_ROWS),
    ).toBeGreaterThan(1e-3);
    expect(scaledShapeMiss(AR_DISPERSION, AR_LIQUID_DENSITY.molPerL, AR_LIQUID_ROWS)).toBeLessThan(
      1.6e-4,
    );
  });

  it("put a measured table on the grid linear between its rows and its red tail beyond, with k = 0", () => {
    const { n, k } = tableWithRedTail(KR_LIQUID_ROWS);
    expect(n).toHaveLength(MATERIAL_GRID_NM.length);
    // 545 nm lies between the 508.6 and 546.1 nm rows, 1.3042 and 1.3032.
    const between = 1.3042 + ((545 - 508.6) / (546.1 - 508.6)) * (1.3032 - 1.3042);
    expect(Math.abs(gridValue(n, 545) - between)).toBeLessThan(1e-6);
    expect(gridValue(n, 645)).toBe(Number(redTailIndex(KR_LIQUID_TAIL, 645).toPrecision(6)));
    expect(k.every((v) => v === 0)).toBe(true);
  });

  it("refuse a liquid table whose rows differ from the rows the tool holds", () => {
    const held = KR_LIQUID_ROWS.map((r) => `        ${r.wavelengthUm} ${r.n}`).join("\n");
    expect(checkedRows(tabulatedN(held), KR_LIQUID_ROWS)).toBe(KR_LIQUID_ROWS);
    expect(() => checkedRows(tabulatedN(held.replace("1.3032", "1.3033")), KR_LIQUID_ROWS)).toThrow(
      /differ/u,
    );
  });

  it("read the method's record at the WebBook's densities, as the files state it", () => {
    // decision-p14-t49h-gas-index §2.3, recomputed (A.2): the unscaled estimate less each
    // measurement, in the record's order.
    expect(T5K_LIQUID_RECORD.map((check) => liquidCheckExcess(check).toFixed(4))).toEqual([
      "0.0025",
      "-0.0027",
      "-0.0018",
      "0.0033",
      "0.0113",
      "0.0115",
      "0.0118",
      "0.0124",
      "0.0118",
      "0.0042",
      "0.0028",
      "-0.0005",
      "0.0002",
      "0.0045",
      "-0.0026",
      "0.0076",
      "0.0142",
      "-0.0100",
      "-0.0212",
    ]);
    expect(
      T5K_LIQUID_RECORD.slice(0, 4).map((check) =>
        (100 * (molarRefractionRatio(check) - 1)).toFixed(2),
      ),
    ).toEqual(["-1.01", "0.82", "0.40", "-1.38"]);
  });

  it("find Gagnon et al.'s two CO values 0.0065 apart beyond the gas's slope, about twice their combined error", () => {
    expect(gagnonSlopeExcess().toFixed(4)).toBe("0.0065");
    expect(GAGNON_COMBINED_ERROR.toFixed(4)).toBe("0.0034");
  });

  it("give liquid O2 k = aλ ÷ 4π with a = σN²: 55.0 and 39.9 m⁻¹ at 575 and 630 nm", () => {
    expect(O2_LIQUID_NUMBER_DENSITY_PER_CM3.toPrecision(6)).toBe("2.14765e+22");
    const [at575, at630] = OXYGEN_PAIR_CROSS_SECTIONS_CM5.map(([nm, sigma]) => ({
      a: oxygenPairAbsorptionPerM(sigma).toFixed(1),
      k: Number(oxygenPairK(sigma, nm).toPrecision(6)),
    }));
    expect([at575, at630]).toEqual([
      { a: "55.0", k: 2.515_26e-6 },
      { a: "39.9", k: 1.998_58e-6 },
    ]);
    const committedK = committed("oxygen-liquid.json").k;
    expect([gridValue(committedK, 575), gridValue(committedK, 630)]).toEqual([
      2.515_26e-6, 1.998_58e-6,
    ]);
  });

  it("give liquid O2 k = 0 where the coldest sets' σ is at or below 0, and from 655 nm, past them", () => {
    const k = committed("oxygen-liquid.json").k;
    const zeros = MATERIAL_GRID_NM.filter((nm) => nm < 655 && gridValue(k, nm) === 0);
    expect(zeros).toEqual([...OXYGEN_PAIR_ZERO_NM]);
    expect(MATERIAL_GRID_NM.filter((nm) => nm >= 655 && gridValue(k, nm) !== 0)).toEqual([]);
    expect(Math.floor(OXYGEN_PAIR_LONGEST_NM)).toBe(654);
  });

  it("read an O2–O2 set's σ linearly in wavenumber, and none where no set at its temperature reaches", () => {
    const sets: ReadonlyArray<CiaSet> = [
      {
        pair: "O2-O2",
        minPerCm: 16_000,
        maxPerCm: 16_002,
        temperatureK: 203,
        reference: THALMAN_VOLKAMER_REFERENCE,
        points: [
          [16_000, 1e-46],
          [16_001, 3e-46],
          [16_002, 5e-46],
        ],
      },
    ];
    const midway = oxygenPairCrossSectionCm5(sets, 1e7 / 16_000.5) ?? Number.NaN;
    expect(Math.abs(midway / 2e-46 - 1)).toBeLessThan(1e-9);
    const onPoint = oxygenPairCrossSectionCm5(sets, 1e7 / 16_001) ?? Number.NaN;
    expect(Math.abs(onPoint / 3e-46 - 1)).toBeLessThan(1e-9);
    expect(oxygenPairCrossSectionCm5(sets, 1e7 / 16_003)).toBeUndefined();
    expect(oxygenPairCrossSectionCm5(sets, 1e7 / 16_001, 293)).toBeUndefined();
  });

  it("refuse an O2–O2 file whose coldest Thalman and Volkamer set is not at 203 K", () => {
    const text = oneSetCia(293, THALMAN_VOLKAMER_REFERENCE, [
      [15_000, 1e-46],
      [30_000, 1e-46],
    ]);
    expect(() => liquidOxygenK(text)).toThrow(/coldest .* 293 K/u);
  });

  it("refuse an O2–O2 file whose σ at 575 nm is not the one oxygen-liquid.json states", () => {
    // 575 nm is 17,391.3 cm⁻¹: a 203 K set across it at another σ.
    const text = oneSetCia(203, THALMAN_VOLKAMER_REFERENCE, [
      [17_390, 1e-45],
      [17_392, 1e-45],
    ]);
    expect(() => liquidOxygenK(text)).toThrow(/σ at 575 nm/u);
  });

  it.each(T5K_FILES)("write %s's header as the tool states it", (file, key) => {
    // The by-hand regeneration's text half, which needs no fetched source.
    const raw: unknown = JSON.parse(readFileSync(fileURLToPath(new URL(file, MATERIALS)), "utf8"));
    const header = t5kHeader(file, key);
    const fields = [
      "key",
      "phase",
      "name",
      "temperatureK",
      "provenance",
      "paper",
      "source",
      "licence",
      "reduction",
      "standIn",
      "fit",
    ] as const;
    const read = (field: (typeof fields)[number]): unknown =>
      typeof raw === "object" && raw !== null ? Reflect.get(raw, field) : undefined;
    expect(fields.map(read)).toEqual(fields.map((field) => header[field]));
  });

  it.each([
    ["nitrous-oxide-liquid.json", "N2O"],
    ["sulphur-dioxide-liquid.json", "SO2"],
    ["carbon-dioxide-liquid.json", "CO2"],
    ["carbon-monoxide-liquid.json", "CO"],
    ["hydrogen-sulphide-liquid.json", "H2S"],
  ])("hold the committed %s to its derivation", (file, key) => {
    const derived = DERIVED_MATERIALS.find((f) => f.key === key && f.phase === "liquid");
    expect(committed(file).n).toEqual(derived?.n);
    expect(committed(file).k).toEqual(derived?.k);
  });

  it("hold the committed liquid Kr and Xe to their tables, and liquid Ar and O2's n to their factors", () => {
    expect(committed("krypton-liquid.json").n).toEqual(tableWithRedTail(KR_LIQUID_ROWS).n);
    expect(committed("xenon-liquid.json").n).toEqual(tableWithRedTail(XE_LIQUID_ROWS).n);
    expect(committed("argon-liquid.json").n).toEqual(
      scaledGrid(AR_DISPERSION, AR_LIQUID_DENSITY.molPerL, AR_LIQUID_FACTOR),
    );
    expect(committed("oxygen-liquid.json").n).toEqual(
      scaledGrid(O2_DISPERSION, O2_LIQUID_DENSITY.molPerL, O2_LIQUID_FACTOR),
    );
  });

  it("ask for each of T5.k's four inputs once: the three liquid tables and HITRAN2024's O2–O2 file", () => {
    const options = sourceOptions().map((input) => input.option);
    for (const option of ["argon-liquid", "krypton-liquid", "xenon-liquid", "o2o2"]) {
      expect(options.filter((o) => o === option)).toHaveLength(1);
    }
  });
});
