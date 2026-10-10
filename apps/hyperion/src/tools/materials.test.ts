import { describe, expect, it } from "vitest";

import {
  cauchyIndex,
  CO2_ICE_FIT,
  DERIVED_MATERIALS,
  FETCHED_MATERIALS,
  HCL_REFRACTIVITY,
  IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL,
  ionicIncrement,
  lorentzLorenzIndex,
  MATERIAL_GRID_NM,
  NH3_ICE_MOLAR_VOLUME_CM3_PER_MOL,
  NH3_REFRACTIVITY,
  NH4CL_CALIBRATION,
  NH4CL_IONIC_INCREMENT,
  NH4SH_IONIC_INCREMENT,
  NH4SH_IONIC_INCREMENT_BAND,
  nh4shIndex,
  parseAria,
  parseHitranTable,
  parseLnk,
  parseRefractiveIndexInfo,
  parseWavelengthTable,
  resample,
} from "./materials";

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

  it("re-derive NH₄Cl's increment, +3.51%, from NH₃ and HCl in the same paper", () => {
    expect(NH4CL_IONIC_INCREMENT).toBeCloseTo(0.035_1, 4);
  });

  it("take NH₄SH's increment as NH₄Cl's to 0.1%", () => {
    expect(Math.abs(NH4SH_IONIC_INCREMENT - NH4CL_IONIC_INCREMENT)).toBeLessThan(1e-3);
  });

  it("would give +4.7%, the top of the ruling's +4.3% to +4.7%, with its own inputs", () => {
    // The ruling's derivation (science-r08-nonspherical.md §3.1): HCl's n − 1 = 4.456 × 10⁻⁴ at the
    // D line, from a secondary source, with its Z read between 0.9924 and 1 (+4.7% to +4.3%).
    const realNh3 = { ...NH3_REFRACTIVITY, compressibility: 0.984_798 };
    const secondaryHcl = { aPerUm2: 4.456e-4 * (118.49 - 1 / 0.5893 ** 2), bPerUm2: 118.49 };
    const increment = ionicIncrement(
      [realNh3, { ...secondaryHcl, compressibility: 0.992_4, source: "HCl" }],
      589.3,
      NH4CL_CALIBRATION.molarVolumeCm3PerMol,
      NH4CL_CALIBRATION.indexD,
    );
    expect(increment).toBeCloseTo(0.047, 3);
  });

  // science-r08-nonspherical.md §3.1's method; its 1.685, 1.648 and 1.632 at δ = +4.5% took NH₃'s
  // real-gas Z (R08's Risks, "Deviations in T3.b's follow-up, as built").
  it.each([
    [380, 1.681_2],
    [550, 1.644_4],
    [780, 1.628_9],
  ] as const)("estimate NH₄SH's index at %i nm as %f, with the +3.5% increment", (nm, index) => {
    expect(Math.abs(nh4shIndex(nm) - index)).toBeLessThan(5e-4);
  });

  it("keep NH₄SH's estimate within +0.075 and −0.03 over the increment's band", () => {
    const [low, high] = NH4SH_IONIC_INCREMENT_BAND;
    for (const nm of MATERIAL_GRID_NM) {
      const n = nh4shIndex(nm);
      expect(nh4shIndex(nm, high) - n).toBeLessThan(0.075);
      expect(n - nh4shIndex(nm, low)).toBeLessThan(0.03);
    }
  });

  it("state NH₄SH's band over the grid, +0.074 and −0.029, in its file", () => {
    const nh4sh = DERIVED_MATERIALS.find((file) => file.key === "NH4SH");
    expect(nh4sh?.standIn).toContain("within +0.074 and −0.029");
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
    const names = new Set(FETCHED_MATERIALS.map((spec) => spec.file));
    expect(names.size).toBe(FETCHED_MATERIALS.length);
    const identities = [...FETCHED_MATERIALS.map((s) => s.header), ...DERIVED_MATERIALS].map(
      (h) => `${h.key}/${h.phase}/${h.variant ?? ""}`,
    );
    expect(new Set(identities).size).toBe(identities.length);
  });
});
