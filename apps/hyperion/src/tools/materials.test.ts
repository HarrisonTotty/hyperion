import { describe, expect, it } from "vitest";

import {
  cauchyIndex,
  cellMolarVolumeCm3PerMol,
  CO2_ICE_FIT,
  DERIVED_MATERIALS,
  FETCHED_MATERIALS,
  IDEAL_MOLAR_VOLUME_STP_CM3_PER_MOL,
  lorentzLorenzIndex,
  MATERIAL_GRID_NM,
  NH3_REFRACTIVITY,
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

  it("estimate NH₄SH's index as science-r08-nonspherical.md §3.1 gives it", () => {
    // The ruling: 1.685 at 380 nm, 1.648 at 550 nm and 1.632 at 780 nm at δ = +4.5%.
    expect(Math.abs(nh4shIndex(380) - 1.685)).toBeLessThan(5e-4);
    expect(Math.abs(nh4shIndex(550) - 1.648)).toBeLessThan(5e-4);
    expect(Math.abs(nh4shIndex(780) - 1.632)).toBeLessThan(5e-4);
  });

  it("keep NH₄SH's estimate within +0.065 and −0.04 over the increment's band", () => {
    const [low, high] = NH4SH_IONIC_INCREMENT_BAND;
    for (const nm of MATERIAL_GRID_NM) {
      const n = nh4shIndex(nm);
      expect(nh4shIndex(nm, high) - n).toBeLessThan(0.065);
      expect(n - nh4shIndex(nm, low)).toBeLessThan(0.04);
    }
  });

  it("would need a 23% increment to reach the withdrawn 1.80 at 550 nm", () => {
    expect(nh4shIndex(550, 0.22)).toBeLessThan(1.8);
    expect(nh4shIndex(550, 0.24)).toBeGreaterThan(1.8);
  });

  it("give NH₃ ice the ruling's 1.458, the method's check on a measured molecular solid", () => {
    // Olovsson and Templeton's cubic cell (Acta Cryst. 12 (1959) 832; COD 2310927), a = 5.138 Å with
    // four molecules; Martonchik et al. 1984 (Appl. Opt. 23, 541) measure 1.436 at 550 nm.
    const n = lorentzLorenzIndex(
      [NH3_REFRACTIVITY],
      550,
      cellMolarVolumeCm3PerMol(5.138 ** 3, 4),
      0,
    );
    expect(Math.abs(n - 1.458)).toBeLessThan(5e-4);
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
