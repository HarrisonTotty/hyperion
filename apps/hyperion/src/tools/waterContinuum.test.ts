import { describe, expect, it } from "vitest";

import crossSections from "../renderer/src/view/atmosphere/absorbers/crossSections.json" with { type: "json" };
import { LADDER_SOURCES, LINE_SOURCES } from "./kDistributionsCli";
import type { LineList } from "./molecularLines";
import {
  CKD_FROM_NM,
  CKD_REFERENCE_PER_CM3,
  ckdCellMean,
  lineWingCellMean,
  type LowtranContinuum,
  parseLowtranContinuum,
  radiationTerm,
  reduceWaterContinuum,
} from "./waterContinuum";

/**
 * `lowtran7.f`'s values about three cells (SHA-256 `25e83d94…59ac`; `BLOCK DATA SF296`, `SF260` and
 * `BFH2O`, in their units of 10⁻²⁰ cm³ a molecule): [ν cm⁻¹, C_s(296 K), C_s(260 K), C_f(296 K)].
 */
const LOWTRAN_POINTS: Readonly<
  Record<number, ReadonlyArray<readonly [number, number, number, number]>>
> = {
  590: [
    [16_930, 6.8451e-10, 1.0849e-9, 8.6104e-11],
    [16_940, 6.9255e-10, 1.0929e-9, 8.8516e-11],
    [16_950, 6.9923e-10, 1.0971e-9, 9.0814e-11],
    [16_960, 7.0396e-10, 1.0969e-9, 9.2522e-11],
    [16_970, 6.7715e-10, 1.046e-9, 8.8913e-11],
  ],
  694: [
    [14_390, 1.4946e-9, 2.3259e-9, 1.9931e-10],
    [14_400, 1.4322e-9, 2.1833e-9, 1.9079e-10],
    [14_410, 1.321e-9, 1.9785e-9, 1.7478e-10],
    [14_420, 1.1749e-9, 1.7308e-9, 1.5259e-10],
  ],
  720: [
    [13_870, 6.2478e-9, 1.0027e-8, 7.7503e-10],
    [13_880, 6.8291e-9, 1.0876e-8, 8.8152e-10],
    [13_890, 7.4528e-9, 1.1744e-8, 9.9022e-10],
    [13_900, 7.6078e-9, 1.1853e-8, 1.0229e-9],
  ],
};

/** A LOWTRAN table's values, each v. */
function filled(v: number): Float64Array {
  return new Float64Array(2_003).fill(v);
}

/** A table holding only some points, 0 elsewhere, on LOWTRAN's grid. */
function sparseTable(
  points: ReadonlyArray<readonly [number, number, number, number]>,
): LowtranContinuum {
  const table = {
    firstPerCm: -20,
    stepPerCm: 10,
    self296Cm3: filled(0),
    self260Cm3: filled(0),
    foreign296Cm3: filled(0),
  };
  for (const [nu, s296, s260, f296] of points) {
    const i = (nu + 20) / 10;
    table.self296Cm3[i] = s296 * 1e-20;
    table.self260Cm3[i] = s260 * 1e-20;
    table.foreign296Cm3[i] = f296 * 1e-20;
  }
  return table;
}

/** A committed H₂O continuum pair's rows. */
function committed(pair: string): {
  readonly temperaturesK: ReadonlyArray<number>;
  readonly rows: ReadonlyArray<ReadonlyArray<number>>;
} {
  const table = crossSections.pairs.find((p) => p.pair === pair);
  if (table === undefined) {
    throw new Error(`crossSections.json holds no ${pair}`);
  }
  return table;
}

/** A committed cell's σ at the reference density, cm² a molecule. */
function atReference(pair: string, nm: number, temperatureK: number): number {
  const table = committed(pair);
  const row = table.rows.find((r) => r[0] === nm);
  const t = table.temperaturesK.indexOf(temperatureK);
  return ((row?.[t + 1] ?? Number.NaN) / 1e-10) * CKD_REFERENCE_PER_CM3;
}

/** A BLOCK DATA unit of `n` values in LOWTRAN's fixed form. */
function block(name: string, first: number, n: number): string {
  const rows: string[] = [];
  for (let k = 0; k < n; k += 5) {
    const values = Array.from(
      { length: Math.min(5, n - k) },
      (_, j) => `${(k + j + 1).toFixed(4)}E-01`,
    );
    rows.push(`     C ${values.join(", ")}${k + 5 >= n ? "/" : ","}`);
  }
  return [
    `      BLOCK DATA ${name}`,
    "C>    BLOCK DATA",
    `      COMMON /X/ V1,V2,DV,NPT,S(${n})`,
    "       DATA V1,V2,DV,NPT /",
    `     1      ${first.toFixed(1)},     20000.0,       10.0,  ${n}/`,
    "      DATA S0001/",
    ...rows,
    `      END BLOCKDATA ${name}`,
  ].join("\n");
}

describe("LOWTRAN 7's continuum tables", () => {
  it("reads each unit's 2,003 values in order, in cm³ a molecule", () => {
    const text = ["SF296", "SF260", "BFH2O"].map((n) => block(n, -20, 2_003)).join("\n");
    const table = parseLowtranContinuum(text);
    expect([table.self296Cm3.length, table.self296Cm3[0], table.foreign296Cm3[2_002]]).toEqual([
      2_003,
      expect.closeTo(0.1e-20, 30),
      expect.closeTo(200.3e-20, 28),
    ]);
  });

  it("refuses a unit on another grid", () => {
    const text = [
      block("SF296", -10, 2_003),
      block("SF260", -20, 2_003),
      block("BFH2O", -20, 2_003),
    ].join("\n");
    expect(() => parseLowtranContinuum(text)).toThrow("SF296");
  });

  it("takes the radiation term as ν tanh(hcν ÷ 2kT), ν itself in the visible", () => {
    expect(radiationTerm(14_400, 296) / 14_400).toBeCloseTo(1, 12);
    expect(radiationTerm(100, 296)).toBeCloseTo(100 * Math.tanh((1.438_776_877 * 100) / 592), 12);
  });
});

describe("H₂O's continuum in crossSections.json", () => {
  it.each([590, 694, 720])(
    "equals LOWTRAN 7's tables with the radiation term in the %s nm cell, at 296 and 260 K",
    (nm) => {
      const table = sparseTable(LOWTRAN_POINTS[nm] ?? []);
      // Six significant figures as written.
      expect(
        atReference("H2O+H2O", nm, 296) / ckdCellMean(table, table.self296Cm3, nm, 296),
      ).toBeCloseTo(1, 5);
      expect(
        atReference("H2O+H2O", nm, 260) / ckdCellMean(table, table.self260Cm3, nm, 260),
      ).toBeCloseTo(1, 5);
      expect(
        atReference("H2O+air", nm, 296) / ckdCellMean(table, table.foreign296Cm3, nm, 296),
      ).toBeCloseTo(1, 5);
    },
  );

  it("equals RRTMG_SW's band-24 CKD_2.4 means over 12,850–16,000 cm⁻¹ within 1%", () => {
    // AER's RRTMG_SW (BSD-3), data/rrtmg_sw.nc at 286e84ed (SHA-256 6bbdcaf3…e717): selfref at
    // 296 K and forref's 296 K set in band 24, weighted by rrtmg_sw_init.f90's 16 g-weights,
    // × 10⁻²⁰: self 1.1781 × 10⁻²⁵ and foreign 1.3876 × 10⁻²⁶ cm² at the reference density.
    const bandMean = (pair: string): number => {
      let sum = 0;
      let width = 0;
      for (let nm = 620; nm <= 780; nm += 1) {
        const low = Math.max(1e7 / (nm + 0.5), 12_850);
        const high = Math.min(1e7 / (nm - 0.5), 16_000);
        if (high > low) {
          sum += atReference(pair, nm, 296) * (high - low);
          width += high - low;
        }
      }
      return sum / width;
    };
    expect(bandMean("H2O+H2O") / 1.1781e-25).toBeCloseTo(1, 2);
    expect(bandMean("H2O+air") / 1.3876e-26).toBeCloseTo(1, 2);
  });

  it("is measured with N₂ and O₂ over 260–296 K from 567 nm, and every other partner by its ratio", () => {
    const self = crossSections.pairs.find((p) => p.pair === "H2O+H2O");
    const foreign = crossSections.pairs.find((p) => p.pair === "H2O+air");
    expect([
      self?.temperatureLaw,
      self?.measuredRangeK,
      self?.lawRangeK,
      self?.estimatedBelowNm,
    ]).toEqual(["lnLnLinear", [260, 296], [150, 500], CKD_FROM_NM]);
    expect([
      foreign?.temperatureLaw,
      foreign?.members,
      foreign?.measuredPartners,
      foreign?.estimatedBelowNm,
    ]).toEqual(["constant", ["H2O", "air"], ["N2", "O2"], CKD_FROM_NM]);
    expect(foreign?.partners).toMatchObject({ N2: 1, O2: 1 });
  });
});

/** Toy H₂O lines: S 10⁻²² cm, γ_air 0.08 and γ_self 0.4 cm⁻¹. */
function toyLines(centres: ReadonlyArray<number>): LineList {
  const fill = (v: number): Float64Array => new Float64Array(centres.length).fill(v);
  return {
    isotopologues: new Uint16Array(centres.length).fill(1),
    wavenumbersPerCm: Float64Array.from(centres),
    intensitiesCm: fill(1e-22),
    lowerEnergiesPerCm: fill(0),
    airHalfWidthsPerCmAtm: fill(0.08),
    airExponents: fill(0.7),
    selfHalfWidthsPerCmAtm: fill(0.4),
    foreignHalfWidthsPerCmAtm: {
      H2: fill(Number.NaN),
      He: fill(Number.NaN),
      CO2: fill(Number.NaN),
      H2O: fill(Number.NaN),
    },
    selfExponents: fill(Number.NaN),
    foreignExponents: {
      H2: fill(Number.NaN),
      He: fill(Number.NaN),
      CO2: fill(Number.NaN),
      H2O: fill(Number.NaN),
    },
  };
}

describe("the extension below 567 nm", () => {
  it("sums each line's Lorentz wing beyond 25 cm⁻¹ and its pedestal, as a brute-force mean", () => {
    const cellNm = 550;
    const [low, high] = [1e7 / 550.5, 1e7 / 549.5];
    const lines = toyLines([low + 5, high + 20, low - 400, high + 1_500]);
    let brute = 0;
    const steps = 200_000;
    for (let k = 0; k < steps; k += 1) {
      const nu = low + ((k + 0.5) * (high - low)) / steps;
      for (const centre of [low + 5, high + 20, low - 400]) {
        const d = Math.max(Math.abs(nu - centre), 25);
        brute += (1e-22 * 0.08) / Math.PI / (d * d + 0.08 * 0.08);
      }
    }
    // The fourth line lies beyond 1,000 cm⁻¹ and is not summed.
    expect(
      lineWingCellMean(lines, lines.airHalfWidthsPerCmAtm, cellNm) / (brute / steps),
    ).toBeCloseTo(1, 6);
  });

  it("is CKD's ratio to the lines over 567–600 nm times the lines' sum, the self law the same cells'", () => {
    const table: LowtranContinuum = {
      firstPerCm: -20,
      stepPerCm: 10,
      self296Cm3: filled(2e-29),
      self260Cm3: filled(3e-29),
      foreign296Cm3: filled(4e-30),
    };
    const lines = toyLines(Array.from({ length: 200 }, (_, k) => 15_500 + 50 * k));
    const reduced = reduceWaterContinuum(table, lines, {});
    let ckd = 0;
    let wing = 0;
    for (let nm = 567; nm <= 600; nm += 1) {
      ckd += ckdCellMean(table, table.foreign296Cm3, nm, 296);
      wing += lineWingCellMean(lines, lines.airHalfWidthsPerCmAtm, nm);
    }
    expect(reduced.foreignRatio).toBeCloseTo(ckd / wing, 10);
    const row = reduced.foreign.rows.find((r) => r[0] === 520);
    const expected =
      (reduced.foreignRatio * lineWingCellMean(lines, lines.airHalfWidthsPerCmAtm, 520)) /
      CKD_REFERENCE_PER_CM3;
    expect((row?.[1] ?? Number.NaN) / 1e-10 / expected).toBeCloseTo(1, 5);
    expect(reduced.selfColdRatio).toBeCloseTo(1.5, 2);
  });
});

describe("the lines' pedestal (R08.T4.d)", () => {
  it("is subtracted for exactly the species whose continuum is drawn: H₂O", () => {
    // The continuum contains each line's Lorentz value at its 25 cm⁻¹ bound (FASCODE's line
    // function), so a species drawn with a CKD pair takes its lines less it, and no other does.
    const continuumSpecies = [
      ...new Set(
        crossSections.pairs
          .filter((pair) => pair.source.startsWith("Clough-Kneizys-Davies"))
          .map((pair) => pair.members[0]),
      ),
    ];
    const pedestalSpecies = Object.values({ ...LINE_SOURCES, ...LADDER_SOURCES })
      .filter((source) => source.policy.pedestal)
      .map((source) => source.species);
    expect([continuumSpecies, pedestalSpecies]).toEqual([["H2O"], ["H2O"]]);
  });
});
