import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  ACID_GRID_NM,
  ACID_UV_DECAY_NM,
  ACID_UV_K365,
  acidUvAbsorberK,
  AR_DISPERSION,
  AR_ICE_CELL,
  AR_ICE_MEASURED_546,
  averagedUniaxial,
  cauchyIndex,
  cellMolarVolumeCm3PerMol,
  CO2_ICE_FIT,
  COMBINED_MATERIALS,
  DERIVED_MATERIALS,
  FETCHED_MATERIALS,
  formulaIndex,
  formulaOnGrid,
  H2S_FILM,
  h2sFilmEstimate,
  HCL_REFRACTIVITY,
  ICE_ESTIMATES,
  iceIndex,
  IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL,
  indexOfDielectric,
  ionicIncrement,
  lorentzLorenzIndex,
  MATERIAL_GRID_NM,
  N2_ICE_CELL,
  NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL,
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
  parseRefractiveIndexInfo,
  parseRefractiveIndexInfoFormula,
  parseRefractiveIndexInfoN,
  parseWavelengthTable,
  resample,
  S8_PRINCIPAL_INDICES,
  S8_STAND_IN_INDEX,
  sourceOptions,
  uniaxialFormulasOnGrid,
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
