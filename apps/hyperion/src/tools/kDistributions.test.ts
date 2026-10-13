import { describe, expect, it } from "vitest";

import {
  broadeningExponentOffsets,
  broadeningRatios,
  CHI_CO2_CO2,
  CHI_CO2_N2,
  chiFactor,
  chiReachPerCm,
  chiWingFactor,
  cubicInLogPressure,
  cubicInTemperature,
  CURVE_DEPTH_MAX,
  decodeCell,
  encodeCell,
  errorColumns,
  intervalMean,
  intervalPoints,
  intervalTransmittance,
  cellBytes,
  kDistributionMetaText,
  kDistributionPart,
  lineShape,
  reduceCell,
  columnCapCm2,
  gatedColumnCm2,
  linearCell,
  rayleighCapCm2,
  STORED_PRESSURES_PA,
  saturationPressurePa,
  type ShapePolicy,
  type SortedNode,
  sortedNode,
  VERIFY_COLUMNS_PER_DECADE,
} from "./kDistributions";
import { LINE_SOURCES, refinedTemperatures, sortFailures } from "./kDistributionsCli";
import type { LineList } from "./molecularLines";

/**
 * A toy cell's spectrum: σ log-uniform over eight decades on 4,096 points, a quarter of them 0 (a
 * cell's points beyond every line's cut-off), as a k-distribution of lines and gaps spans.
 */
function toyNode(shift = 0): SortedNode {
  return sortedNode(
    Float64Array.from({ length: 4_096 }, (_, i) =>
      i % 4 === 0 ? 0 : 10 ** (-28 + 8 * ((i * 0.618_034) % 1) + shift),
    ),
  );
}

/** A node's mean transmittance through u, from its points. */
function exactTransmittance(node: SortedNode, u: number): number {
  return intervalTransmittance(node, 0, 1, u);
}

describe("a node's sorted cross-sections", () => {
  it("give an interval's mean, its ends weighted by their overlap", () => {
    const node = sortedNode(Float64Array.from([4, 1, 3, 2]));
    expect([
      intervalMean(node, 0, 1),
      intervalMean(node, 0, 0.5),
      intervalMean(node, 0.125, 0.375),
      intervalMean(node, 0.6, 0.7),
    ]).toEqual([2.5, 1.5, 1.5, 3]);
  });

  it("give an interval two points that keep its mean", () => {
    const node = toyNode();
    for (const [a, b] of [
      [0, 1],
      [0.3, 0.9],
      [0.99, 1],
    ] as const) {
      const [s1, s2] = intervalPoints(node, a, b);
      expect((s1 + s2) / 2 / intervalMean(node, a, b)).toBeCloseTo(1, 12);
    }
  });
});

describe("a cell's reduction", () => {
  const target = 2e-4;
  const nodes = [toyNode(), toyNode(1.3), toyNode(-0.7)];
  const reduced = reduceCell(nodes, target);

  it("holds every node's curve of growth to the target on the verification's columns", () => {
    for (const [n, node] of nodes.entries()) {
      const values = reduced.values[n] ?? new Float64Array(0);
      for (const u of errorColumns(node, target, VERIFY_COLUMNS_PER_DECADE, Infinity)) {
        let t = 0;
        for (const [g, w] of reduced.weights.entries()) {
          t += w * Math.exp(-(values[g] ?? 0) * u);
        }
        expect(Math.abs(t - exactTransmittance(node, u))).toBeLessThanOrEqual(target);
      }
    }
  });

  it("gives weights that sum to 1 and keep each node's mean cross-section", () => {
    expect(reduced.weights.reduce((s, w) => s + w, 0)).toBeCloseTo(1, 12);
    for (const [n, node] of nodes.entries()) {
      const values = reduced.values[n] ?? new Float64Array(0);
      let mean = 0;
      for (const [g, w] of reduced.weights.entries()) {
        mean += w * (values[g] ?? 0);
      }
      expect(mean / intervalMean(node, 0, 1)).toBeCloseTo(1, 10);
    }
  });

  it("gives one interval to a cell of a single cross-section, which has no error", () => {
    const flat = sortedNode(new Float64Array(512).fill(3e-27));
    expect(reduceCell([flat], target).weights.length).toBe(2);
  });
});

/** Numbers as a Float64Array. */
function of(values: ReadonlyArray<number>): Float64Array {
  return Float64Array.from(values);
}

/** e to a cubic in T, which a cubic on ln σ reproduces exactly. */
function cubicExp(x: number): number {
  return Math.exp(-50 + 0.01 * x - 1e-5 * x * x + 2e-8 * x ** 3);
}

/** A depth cubic in ln p, which the read reproduces exactly. */
function cubicDepth(x: number): number {
  return 2 + 0.3 * x - 0.2 * x * x + 0.05 * x ** 3;
}

/** A failing chain test at a temperature. */
function failureAt(t: number): {
  error: number;
  temperatureK: number;
  pressurePa: number;
  columnCm2: number;
} {
  return { error: 3e-3, temperatureK: t, pressurePa: 1e5, columnCm2: 1e24 };
}

describe("the broadening ratios", () => {
  const n = 2;
  const lines: LineList = {
    isotopologues: new Uint16Array(n).fill(36),
    wavenumbersPerCm: of([13_200, 13_300]),
    intensitiesCm: of([3e-24, 1e-24]),
    lowerEnergiesPerCm: of([0, 0]),
    airHalfWidthsPerCmAtm: of([0.05, 0.1]),
    airExponents: of([0.7, 0.7]),
    selfHalfWidthsPerCmAtm: of([0.1, 0.1]),
    foreignHalfWidthsPerCmAtm: {
      H2: of([Number.NaN, Number.NaN]),
      He: of([Number.NaN, 0.05]),
      CO2: of([0.06, 0.12]),
      H2O: of([Number.NaN, Number.NaN]),
    },
    selfExponents: of([0.6, Number.NaN]),
    foreignExponents: {
      H2: of([Number.NaN, Number.NaN]),
      He: of([Number.NaN, 0.5]),
      CO2: of([0.8, 0.8]),
      H2O: of([Number.NaN, Number.NaN]),
    },
  };

  it("are Σ S γ_j ÷ Σ S γ_air over the lines HITRAN gives γ_j for", () => {
    // self: (3 × 0.1 + 1 × 0.1) ÷ (3 × 0.05 + 1 × 0.1); He over the second line alone: 0.05 ÷ 0.1;
    // CO2: (3 × 0.06 + 1 × 0.12) ÷ 0.25. H2 and H2O, given for no line, are left out.
    const r = broadeningRatios(lines);
    expect(Object.keys(r)).toEqual(["self", "He", "CO2"]);
    expect(r["self"]).toBeCloseTo(1.6, 12);
    expect(r["He"]).toBeCloseTo(0.5, 12);
    expect(r["CO2"]).toBeCloseTo(1.2, 12);
  });

  it("carry each gas's S γ-weighted exponent over air's, n_air standing in where none is given", () => {
    // self: (0.3 × 0.6 + 0.1 × 0.7) ÷ 0.4 − 0.7; He over the second line: 0.5 − 0.7; CO2: 0.8 − 0.7.
    const d = broadeningExponentOffsets(lines);
    expect([Object.keys(d), d["self"], d["He"], d["CO2"]]).toEqual([
      ["self", "He", "CO2"],
      expect.closeTo(-0.075, 12),
      expect.closeTo(-0.2, 12),
      expect.closeTo(0.1, 12),
    ]);
  });

  it("take an offset under 10⁻⁶ as 0: a gas whose widths are air's to their printed digits", () => {
    // n_air on every line for H₂, its γ_H2 air's but for a digit: the means differ by 4.8 × 10⁻⁸.
    const near: LineList = {
      ...lines,
      airExponents: of([0.7, 0.71]),
      foreignHalfWidthsPerCmAtm: {
        ...lines.foreignHalfWidthsPerCmAtm,
        H2: of([0.050_000_01, 0.1]),
      },
    };
    expect(broadeningExponentOffsets(near)["H2"]).toBe(0);
  });
});

describe("the client's chain", () => {
  it("takes ln σ cubic in T, exact for a cubic, and linear in σ where a node is 0", () => {
    const t = [100, 150, 200, 250, 300];
    expect(cubicInTemperature(t, t.map(cubicExp), 175) / cubicExp(175)).toBeCloseTo(1, 12);
    expect(cubicInTemperature(t, [0, 2, 4, 6, 8], 175)).toBeCloseTo(3, 12);
  });

  it("reads τ cubic in ln p, exact for a cubic and not held between the rows about it", () => {
    const logP = [0, 1, 2, 3, 4].map((k) => k * 0.5756);
    expect(cubicInLogPressure(logP, logP.map(cubicDepth), 1.2)).toBeCloseTo(cubicDepth(1.2), 12);
    // A curve that dips between rows 1 and 2 is followed, not clamped to them.
    const dip = [1, 0.5, 0.5, 1, 2];
    expect(cubicInLogPressure(logP, dip, 0.5 * (logP[1] ?? 0) + 0.5 * (logP[2] ?? 0))).toBeLessThan(
      0.5,
    );
  });

  it("clamps the read's τ to [0, −ln 10⁻⁶] and holds the end rows outside them", () => {
    const logP = [0, 1, 2, 3];
    expect(cubicInLogPressure(logP, [0, 0, 1, 0], 0.5)).toBe(0);
    expect(cubicInLogPressure(logP, [20, 20, 9, 20], 0.5)).toBe(CURVE_DEPTH_MAX);
    expect(cubicInLogPressure(logP, [1, 2, 3, 4], -5)).toBe(1);
    expect(cubicInLogPressure(logP, [1, 2, 3, 4], 9)).toBe(4);
  });
});

describe("CO₂'s far-wing factors", () => {
  it("are SpeCT's ν₃ sets, CO₂–N₂'s B₂ α as its code reads it, digit for digit", () => {
    expect(CHI_CO2_CO2).toEqual({
      pair: "CO2-CO2",
      wingShape: "quantumAsymmetric",
      measuredRangeK: [218, 751],
      breaksPerCm: [3, 50, 220],
      alphaCm: [0.055, 0.021, 0.01],
      betaCm: [-0.11, 0.08, 0],
      epsilonPerK: [0.008, 0.0095, 0],
    });
    expect(CHI_CO2_N2).toEqual({
      pair: "CO2-N2",
      wingShape: "quantumAsymmetric",
      measuredRangeK: [295, 773],
      breaksPerCm: [3, 10, 70],
      alphaCm: [0.416, 0.00167, 0.019],
      betaCm: [-0.354, 0.0421, 0],
      epsilonPerK: [0.00386, 0.00248, 0],
    });
  });

  it("follow the piecewise form, 1 within 3 cm⁻¹", () => {
    // At 296 K, CO₂–CO₂'s B₁ = 0.055 − 0.11 e^(−2.368) and CO₂–N₂'s = 0.416 − 0.354 e^(−1.14256).
    const b1Self = 0.055 - 0.11 * Math.exp(-0.008 * 296);
    const b1N2 = 0.416 - 0.354 * Math.exp(-0.00386 * 296);
    expect([
      chiFactor(CHI_CO2_CO2, 296, 2.9),
      chiFactor(CHI_CO2_CO2, 296, 10),
      chiFactor(CHI_CO2_N2, 296, -10),
    ]).toEqual([
      1,
      expect.closeTo(Math.exp(-b1Self * 7), 12),
      expect.closeTo(Math.exp(-b1N2 * 7), 12),
    ]);
  });

  it("hold their coefficients outside their fits' temperatures", () => {
    expect([
      chiFactor(CHI_CO2_CO2, 120, 50),
      chiFactor(CHI_CO2_N2, 150, 200),
      chiFactor(CHI_CO2_N2, 900, 200),
    ]).toEqual([
      chiFactor(CHI_CO2_CO2, 218, 50),
      chiFactor(CHI_CO2_N2, 295, 200),
      chiFactor(CHI_CO2_N2, 773, 200),
    ]);
  });

  it("reach under 10⁻³ at a fixed distance over the temperatures used", () => {
    const reach = chiReachPerCm(CHI_CO2_CO2, 100, 500);
    for (const t of [100, 200, 296, 400, 500]) {
      expect(chiFactor(CHI_CO2_CO2, t, reach)).toBeLessThan(1e-3);
    }
    expect(chiFactor(CHI_CO2_CO2, 500, reach - 1)).toBeGreaterThan(1e-3);
  });

  it("reach 293 cm⁻¹ over the tables' 100–500 K, set by CO₂–CO₂ (§13, ruling 6)", () => {
    expect([chiReachPerCm(CHI_CO2_CO2, 100, 500), chiReachPerCm(CHI_CO2_N2, 100, 500)]).toEqual([
      293, 256,
    ]);
  });

  it("multiply a Lorentz-fitted set's profile by χ alone", () => {
    const set = { ...CHI_CO2_N2, wingShape: "lorentz" as const };
    expect(chiWingFactor(set, 296, -40, 14_000)).toBe(chiFactor(set, 296, 40));
  });

  it("give SpeCT's sets their Eq. 11 factors beyond σ₁, and χ alone within it", () => {
    const c2 = 1.438_776_877;
    const x = (c2 * 100) / 296;
    const balance =
      (14_100 * -Math.expm1((-c2 * 14_100) / 296)) / (14_000 * -Math.expm1((-c2 * 14_000) / 296));
    expect([
      chiWingFactor(CHI_CO2_CO2, 296, 2, 14_000),
      chiWingFactor(CHI_CO2_CO2, 296, 100, 14_000) / chiFactor(CHI_CO2_CO2, 296, 100),
    ]).toEqual([1, expect.closeTo((x / -Math.expm1(-x)) * balance, 12)]);
  });

  it("raise SpeCT's blue wing above its red one", () => {
    expect(chiWingFactor(CHI_CO2_CO2, 296, 100, 14_000)).toBeGreaterThan(
      chiWingFactor(CHI_CO2_CO2, 296, -100, 14_000),
    );
  });

  it("mix linearly in the width share, one place setting the shape", () => {
    const policy: ShapePolicy = {
      variants: [
        { broadener: "self", chi: CHI_CO2_CO2 },
        { broadener: "air", chi: CHI_CO2_N2 },
      ],
      cutoffPerCm: 250,
      pedestal: false,
      estimatedAboveHalfWidthPerCm: 0.25,
    };
    const mixed = lineShape(policy, 296, [0.3, 0.7]).farWing?.factor(20, 14_000) ?? Number.NaN;
    expect(mixed).toBeCloseTo(
      0.3 * chiWingFactor(CHI_CO2_CO2, 296, 20, 14_000) +
        0.7 * chiWingFactor(CHI_CO2_N2, 296, 20, 14_000),
      12,
    );
    expect(lineShape(policy, 296, [1, 0]).farWing?.breaksPerCm).toEqual([3, 50, 220]);
    expect(
      lineShape({ ...policy, variants: [{ broadener: "air" }] }, 296, [1]).farWing,
    ).toBeUndefined();
  });
});

describe("the cold end's column cap", () => {
  const o2 = LINE_SOURCES["O2"]?.saturation;

  it("takes O₂'s saturation 30.8 Pa at 50 K (decision-r08-t4d-kdist-chain.md §4.1) and none above its critical point", () => {
    if (o2 === undefined) {
      throw new Error("O₂ has no saturation curve");
    }
    expect(saturationPressurePa(o2, 50)).toBeCloseTo(30.8, 1);
    expect(saturationPressurePa(o2, 54.36)).toBeCloseTo(146.25, 6);
    expect(saturationPressurePa(o2, 160)).toBe(Infinity);
  });

  it("takes CO₂'s from Span and Wagner's equations: 1 atm at 194.7 K, its triple point, none past 304.13 K", () => {
    const co2 = LINE_SOURCES["CO2"]?.saturation;
    if (co2 === undefined) {
      throw new Error("CO₂ has no saturation curve");
    }
    expect(saturationPressurePa(co2, 194.7) / 101_325).toBeCloseTo(1, 2);
    // The two equations meet at the triple point to 2.3 × 10⁻⁵.
    expect(saturationPressurePa(co2, 216.591_999) / 517_950).toBeCloseTo(1, 6);
    expect(saturationPressurePa(co2, 216.592) / 517_950).toBeCloseTo(1, 4);
    expect(saturationPressurePa(co2, 305)).toBe(Infinity);
  });

  it("is ten times the saturated column 40 p_sat ÷ (m g) at 0.5 m s⁻² where that is the lower", () => {
    if (o2 === undefined) {
      throw new Error("O₂ has no saturation curve");
    }
    // 400 × 30.8 Pa ÷ (31.99 × 1.6605 × 10⁻²⁷ kg × 0.5 m s⁻²) m⁻², in cm⁻²: 4.6 × 10²⁵.
    const cap = columnCapCm2(o2, 50, 1e5, 31.98983);
    expect(
      cap / ((400 * saturationPressurePa(o2, 50)) / (31.98983 * 1.660_539_066_6e-27 * 0.5) / 1e4),
    ).toBeCloseTo(1, 12);
    expect(cap).toBeGreaterThan(4.5e25);
    expect(cap).toBeLessThan(4.7e25);
  });

  it("is 400 p_eff ÷ (m g) at 0.5 m s⁻² for every species, the lower of the two", () => {
    const byPressure = (400 * 1e3) / (31.98983 * 1.660_539_066_6e-27 * 0.5) / 1e4;
    expect(columnCapCm2(undefined, 300, 1e3, 31.98983) / byPressure).toBeCloseTo(1, 12);
    // Above O₂'s critical point only the pressure cap stops the columns.
    expect(
      columnCapCm2(LINE_SOURCES["O2"]?.saturation, 160, 1e3, 31.98983) / byPressure,
    ).toBeCloseTo(1, 12);
  });
});

describe("the Rayleigh cap and linear cells (decision-r08-t4d-kdist-chain.md §§9, 10 and 12)", () => {
  it("stop the columns at −ln 10⁻⁶ ÷ helium's Rayleigh cross-section: 5.1e28, 2.3e29 and 8.3e29 cm⁻²", () => {
    // The plan's figures (§10, ruling 4; §12, ruling 7), each to half its last digit.
    expect(Math.abs(rayleighCapCm2(380) / 5.1e28 - 1)).toBeLessThan(0.05 / 5.1);
    expect(Math.abs(rayleighCapCm2(550) / 2.3e29 - 1)).toBeLessThan(0.05 / 2.3);
    expect(Math.abs(rayleighCapCm2(760) / 8.3e29 - 1)).toBeLessThan(0.05 / 8.3);
    expect(gatedColumnCm2(undefined, 550, 300, 1e7, 31.98983)).toBe(rayleighCapCm2(550));
  });

  it("store a cell whose peak takes at most 0.02 at its cap as its mean, exact to 2 × 10⁻⁴", () => {
    const node = sortedNode(Float64Array.from({ length: 512 }, (_, i) => 1e-30 * (1 + (i % 7))));
    const cap = 0.02 / (node.sorted.at(-1) ?? 1);
    const reduced = linearCell([node, node], [cap, cap]);
    expect([reduced.weights.length, reduced.values[0]?.[0]]).toEqual([1, intervalMean(node, 0, 1)]);
    expect(reduced.worstError).toBeLessThanOrEqual(2e-4);
  });
});

describe("the stored pressures", () => {
  it("run 4 a decade from 10 Pa to 1 MPa, 8 a decade to 10 MPa, and one node more at 13.3 MPa", () => {
    expect(STORED_PRESSURES_PA).toHaveLength(30);
    expect(STORED_PRESSURES_PA[20]).toBeCloseTo(1e6, 6);
    expect((STORED_PRESSURES_PA[21] ?? 0) / 1e6).toBeCloseTo(10 ** (1 / 8), 12);
    expect(STORED_PRESSURES_PA[28]).toBeCloseTo(1e7, 4);
    expect((STORED_PRESSURES_PA[29] ?? 0) / 1e7).toBeCloseTo(10 ** (1 / 8), 12);
  });
});

describe("the temperature refinement", () => {
  const nodes = [50, 100, 150, 162.5, 175];

  it("halves an interval wider than 12.5 K whose midpoint fails, and records one at 12.5 K", () => {
    const sorted = sortFailures(nodes, [failureAt(75), failureAt(168.75), failureAt(75)]);
    expect(sorted.refine).toEqual([0]);
    expect(sorted.estimated).toEqual([[162.5, 175]]);
    expect(sorted.atNodes).toEqual([]);
    expect(refinedTemperatures(nodes, sorted.refine)).toEqual([50, 75, 100, 150, 162.5, 175]);
  });

  it("keeps a failure at a node's temperature apart, which refinement cannot reach", () => {
    expect(sortFailures(nodes, [failureAt(100)]).atNodes).toHaveLength(1);
  });
});

describe("a cell's storage", () => {
  it("keeps every σ to half a 16-bit step of its ln σ, and 0 as 0", () => {
    const values = Float64Array.from({ length: 64 }, (_, i) =>
      i === 3 ? 0 : 10 ** (-30 + (i * 7) / 63),
    );
    const none = { error: 0, temperatureK: 0, pressurePa: 0, columnCm2: 0 };
    const cell = {
      cellNm: 690,
      weights: Float64Array.of(0.5, 0.5),
      values,
      worstNodeError: 0,
      worstChain: none,
      worstBeyondCap: none,
      failures: [],
      measuredShapeTo: Int32Array.of(0),
      beyondCutoffShare: Float64Array.of(0),
      nearWingShare: Float64Array.of(0),
    };
    const encoded = encodeCell(cell);
    const decoded = decodeCell(encoded);
    expect(decoded[3]).toBe(0);
    const worst = Math.max(
      ...Array.from(values, (v, i) => (v > 0 ? Math.abs(Math.log((decoded[i] ?? 0) / v)) : 0)),
    );
    expect(worst).toBeLessThanOrEqual(0.5 * encoded.lnStep * (1 + 1e-9));
  });
});

describe("a k-distribution table's files", () => {
  const meta = {
    species: "O2",
    source: "s",
    url: "u",
    sha256: "h",
    fetched: "f",
    terms: "t",
    reduction: "r",
    basis: "measured" as const,
    measuredRangeK: [100, 296] as const,
    measuredBelowGrid: true,
    measuredShapeTo: [1, 0],
    estimatedTemperaturesK: [[50, 62.5] as const],
    broadening: { self: 1.005 },
    broadeningExponents: { self: -0.1 },
    variants: ["self" as const, "air" as const],
    chiHeld: [
      [true, true],
      [false, false],
    ],
    beyondCutoffShare: [1e-4, 1e-3, 2e-4, 2e-3],
    nearWingShare: [1e-3, 1e-2, 2e-3, 2e-2],
    worstNodeError: 9e-4,
    worstChainError: 1.5e-3,
    worstBeyondCapError: 3e-3,
    mixing: {
      shares: [0.05, 0.25, 0.5, 0.75, 0.95],
      temperaturesK: [100, 198, 296],
      worstErrors: [4e-3, 1e-2, 8e-3, 4e-3, 1e-3],
      limitColumnsCm2: [
        [
          [7e28, Infinity, Infinity],
          [1e29, Infinity, Infinity],
          [1.5e29, Infinity, Infinity],
          [Infinity, Infinity, Infinity],
          [Infinity, Infinity, Infinity],
        ],
      ],
    },
    worstQuantisationError: 1e-4,
    linearCells: 0,
  };
  const grid = { temperaturesK: [100, 296], pressuresPa: [10, 100] };

  it("write a sidecar of the grid, the records and the parts", () => {
    const file: unknown = JSON.parse(
      kDistributionMetaText(meta, grid, [
        { file: "O2-1.bin", cells: 1, bytes: 100, sha256: "abc" },
      ]),
    );
    expect(file).toMatchObject({
      species: "O2",
      temperaturesK: [100, 296],
      pressuresPa: [10, 100],
      measuredShapeTo: [1, 0],
      estimatedTemperaturesK: [[50, 62.5]],
      broadening: { self: 1.005 },
      variants: ["self", "air"],
      chiHeld: [
        [true, true],
        [false, false],
      ],
      broadeningExponents: { self: -0.1 },
      beyondCutoffShare: [1e-4, 1e-3, 2e-4, 2e-3],
      mixing: {
        shares: [0.05, 0.25, 0.5, 0.75, 0.95],
        temperaturesK: [100, 198, 296],
        worstErrors: [4e-3, 1e-2, 8e-3, 4e-3, 1e-3],
        limitColumnsCm2: [
          [
            [7e28, null, null],
            [1e29, null, null],
            [1.5e29, null, null],
            [null, null, null],
            [null, null, null],
          ],
        ],
      },
      parts: [{ file: "O2-1.bin", cells: 1, bytes: 100, sha256: "abc" }],
    });
  });

  it("write a binary part: its header, then each cell's nm, G, offset, step, weights and codes", () => {
    const codes = Uint16Array.from({ length: 2 * 2 * 2 * 2 }, (_, i) => i + 1);
    const cell = {
      cellNm: 690,
      weights: Float64Array.of(0.25, 0.75),
      lnOffset: -62,
      lnStep: 1e-3,
      codes,
    };
    const bytes = kDistributionPart(grid, 2, [cell]);
    const view = new DataView(bytes.buffer);
    expect(String.fromCharCode(...bytes.subarray(0, 4))).toBe("HYKD");
    expect([1, 2, 3, 4, 5, 6].map((k) => view.getUint16(2 + 2 * k, true))).toEqual([
      1, 1, 2, 2, 2, 0,
    ]);
    expect([view.getUint16(16, true), view.getUint16(18, true)]).toEqual([690, 2]);
    expect([view.getFloat64(20, true), view.getFloat64(28, true)]).toEqual([-62, 1e-3]);
    expect([view.getFloat64(36, true), view.getFloat64(44, true)]).toEqual([0.25, 0.75]);
    expect([view.getUint16(52, true), view.getUint16(52 + 2 * 15, true)]).toEqual([1, 16]);
    expect(bytes.length).toBe(16 + cellBytes(cell));
  });

  it("refuse a cell whose codes do not fill the grid", () => {
    const cell = {
      cellNm: 690,
      weights: Float64Array.of(1),
      lnOffset: 0,
      lnStep: 1,
      codes: new Uint16Array(3),
    };
    expect(() => kDistributionPart(grid, 1, [cell])).toThrow("codes");
  });
});
