import { describe, expect, it } from "vitest";

import { EARTH_REFERENCE } from "./earth";
import {
  columnLengthM,
  type DensityProfile,
  densityAt,
  extinction,
  type MediumTerm,
  phaseAt,
  type PhaseMatrixElement,
  type PhaseMatrixTable,
  phaseTable,
  tabulatedDensity,
  termNamed,
} from "./medium";

const TENT: DensityProfile = { kind: "tent", bottomM: 10_000, peakM: 25_000, topM: 40_000 };
const EXP: DensityProfile = { kind: "exponential", scaleHeightM: 8_000 };
/** A layer from the ground: 1, 0.5 at 1 km, 0.75 at 3 km, 0 at 4 km. */
const TABLE: DensityProfile = tabulatedDensity(
  Float64Array.of(0, 1_000, 3_000, 4_000),
  Float64Array.of(1, 0.5, 0.75, 0),
);
/** A table whose last level is not 0, ending at 2 km. */
const CUT: DensityProfile = tabulatedDensity(Float64Array.of(0, 2_000), Float64Array.of(1, 0.5));
/** A layer above the ground: 0.2 from the ground to 1 km, rising to 0.6 at 3 km. */
const RAISED: DensityProfile = tabulatedDensity(
  Float64Array.of(1_000, 3_000),
  Float64Array.of(0.2, 0.6),
);

/** The midpoint rule over [0, top] in 1 m steps. */
function numericColumnM(profile: DensityProfile, topM: number): number {
  let sum = 0;
  for (let h = 0.5; h < topM; h += 1) {
    sum += densityAt(profile, h);
  }
  return sum;
}

describe("densityAt", () => {
  it("is 1 at the ground and 1/e at one scale height for an exponential", () => {
    expect(densityAt(EXP, 0)).toBe(1);
    expect(densityAt(EXP, 8_000)).toBeCloseTo(Math.exp(-1), 12);
  });

  it("is zero outside a tent, 1 at its peak and half way up each side", () => {
    expect(densityAt(TENT, 5_000)).toBe(0);
    expect(densityAt(TENT, 45_000)).toBe(0);
    expect(densityAt(TENT, 25_000)).toBe(1);
    expect(densityAt(TENT, 17_500)).toBeCloseTo(0.5, 12);
    expect(densityAt(TENT, 32_500)).toBeCloseTo(0.5, 12);
  });
});

describe("a tabulated density", () => {
  it("takes each level's value", () => {
    expect([0, 1_000, 3_000, 4_000].map((h) => densityAt(TABLE, h))).toEqual([1, 0.5, 0.75, 0]);
  });

  it.each([
    [500, 0.75],
    [2_000, 0.625],
    [3_500, 0.375],
  ])("is linear between levels: at %s m, %s", (heightM, expected) => {
    expect(densityAt(TABLE, heightM)).toBeCloseTo(expected, 12);
  });

  it("is the first level's below it, as R08.T12.a's tracer reads it", () => {
    expect([densityAt(TABLE, -10), densityAt(RAISED, 0), densityAt(RAISED, 500)]).toEqual([
      1, 0.2, 0.2,
    ]);
  });

  it("is the last level's above it, as R08.T12.a's tracer reads it", () => {
    expect([densityAt(CUT, 2_000), densityAt(CUT, 9_000)]).toEqual([0.5, 0.5]);
  });

  it.each<[string, DensityProfile, number, number]>([
    ["every level", TABLE, 10_000, 750 + 1_250 + 375],
    ["part of a level", TABLE, 2_000, 750 + 562.5],
    ["the last level's value above it", CUT, 5_000, 1_500 + 0.5 * 3_000],
    ["the first level's value below it", RAISED, 2_000, 200 + 300],
    ["nothing at the ground", TABLE, 0, 0],
  ])("integrates %s", (_, profile, topM, expected) => {
    expect(columnLengthM(profile, topM)).toBeCloseTo(expected, 9);
  });

  it.each<[string, DensityProfile]>([
    ["a layer from the ground", TABLE],
    ["a table cut short", CUT],
    ["a layer above the ground", RAISED],
  ])("agrees with a numerical integral of %s at partial heights", (_, profile) => {
    for (const top of [700, 2_500, 3_900, 6_000]) {
      expect(columnLengthM(profile, top)).toBeCloseTo(numericColumnM(profile, top), 6);
    }
  });

  it.each<[string, Float64Array, Float64Array]>([
    ["one level", Float64Array.of(0), Float64Array.of(1)],
    ["more heights than densities", Float64Array.of(0, 1, 2), Float64Array.of(1, 0.5)],
    ["heights that do not ascend", Float64Array.of(0, 0), Float64Array.of(1, 0.5)],
    ["a height that is not a number", Float64Array.of(0, Number.NaN), Float64Array.of(1, 0.5)],
    ["a negative density", Float64Array.of(0, 1), Float64Array.of(1, -0.1)],
  ])("refuses %s", (_, altitudesM, relative) => {
    expect(() => tabulatedDensity(altitudesM, relative)).toThrow(RangeError);
  });
});

describe("columnLengthM", () => {
  it("gives 15 km for the full tent, as Bruneton's ozone layer integrates", () => {
    expect(columnLengthM(TENT, 100_000)).toBeCloseTo(15_000, 9);
  });

  it("agrees with a numerical integral of the profile at partial heights", () => {
    for (const top of [12_000, 25_000, 31_000, 60_000]) {
      expect(columnLengthM(TENT, top)).toBeCloseTo(numericColumnM(TENT, top), 2);
      expect(columnLengthM(EXP, top)).toBeCloseTo(numericColumnM(EXP, top), 2);
    }
  });
});

describe("terms", () => {
  it("sums scattering and absorption into extinction", () => {
    const term: MediumTerm = {
      name: "t",
      density: EXP,
      scattering: [1, 2, 3],
      absorption: [0.5, 0.25, 0],
      phase: { kind: "none" },
    };
    expect(extinction(term)).toEqual([1.5, 2.25, 3]);
  });

  it("finds a term by name and refuses a missing one", () => {
    expect(termNamed(EARTH_REFERENCE, "ozone").name).toBe("ozone");
    expect(() => termNamed(EARTH_REFERENCE, "dust")).toThrow(/no term dust/);
  });
});

describe("phaseAt", () => {
  it("gives Rayleigh's 3 ÷ (16π) × (1 + cos²θ) in every channel", () => {
    expect(phaseAt({ kind: "rayleigh" }, 0.5)).toEqual(
      Array.from({ length: 3 }, () => (3 / (16 * Math.PI)) * 1.25),
    );
  });

  it("gives Cornette–Shanks's form, which is Rayleigh's at g = 0", () => {
    const [atZero] = phaseAt({ kind: "cornette-shanks", asymmetry: 0 }, 0.3);
    expect(atZero).toBeCloseTo((3 / (16 * Math.PI)) * 1.09, 15);
    const g = 0.584;
    const k = ((3 / (8 * Math.PI)) * (1 - g * g)) / (2 + g * g);
    const [forward] = phaseAt({ kind: "cornette-shanks", asymmetry: g }, 1);
    expect(forward).toBeCloseTo((2 * k) / (1 - g) ** 3, 12);
  });

  it("gives an absorbing-only term no phase", () => {
    expect(phaseAt({ kind: "none" }, 0.2)).toEqual([0, 0, 0]);
  });
});

/** A scattering-matrix element of `n` entries a channel, each `value`. */
function element(n: number, value = 0.5): PhaseMatrixElement {
  return [
    new Float64Array(n).fill(value),
    new Float64Array(n).fill(value),
    new Float64Array(n).fill(value),
  ];
}

/** A scattering matrix of `n` entries a channel, its b₁ given. */
function matrixOf(n: number, b1 = element(n)): PhaseMatrixTable {
  return { a2: element(n), a3: element(n), a4: element(n), b1, b2: element(n) };
}

/** A two-entry phase of 1 in every channel. */
const ONES = [Float64Array.of(1, 1), Float64Array.of(1, 1), Float64Array.of(1, 1)] as const;

describe("a tabulated phase", () => {
  /** Three entries at θ = 0, π ÷ 4 and π: u = 0, ½ and 1. */
  const ENTRIES = phaseTable(Float64Array.of(0, 0.5, 1), [
    Float64Array.of(1, 0.5, 0.25),
    Float64Array.of(2, 1, 0.5),
    Float64Array.of(4, 2, 1),
  ]);
  const PHASE = { kind: "tabulated", table: ENTRIES } as const;

  it("takes each entry's value per channel", () => {
    expect(phaseAt(PHASE, 1)).toEqual([1, 2, 4]);
    expect(phaseAt(PHASE, -1)).toEqual([0.25, 0.5, 1]);
    const [r, g, b] = phaseAt(PHASE, Math.cos(Math.PI / 4));
    expect(r).toBeCloseTo(0.5, 12);
    expect(g).toBeCloseTo(1, 12);
    expect(b).toBeCloseTo(2, 12);
  });

  it("is linear in u = √(θ ÷ π) between entries, not in θ or cos θ", () => {
    // θ = π ÷ 16 is u = ¼, half way between the first two entries.
    const [r] = phaseAt(PHASE, Math.cos(Math.PI / 16));
    expect(r).toBeCloseTo(0.75, 12);
  });

  it.each<[string, Float64Array, readonly [Float64Array, Float64Array, Float64Array]]>([
    ["one entry", Float64Array.of(0), [Float64Array.of(1), Float64Array.of(1), Float64Array.of(1)]],
    [
      "a channel of another length",
      Float64Array.of(0, 1),
      [Float64Array.of(1, 1), Float64Array.of(1), Float64Array.of(1, 1)],
    ],
    [
      "a u that does not start at 0",
      Float64Array.of(0.1, 1),
      [Float64Array.of(1, 1), Float64Array.of(1, 1), Float64Array.of(1, 1)],
    ],
    [
      "a u that does not end at 1",
      Float64Array.of(0, 0.9),
      [Float64Array.of(1, 1), Float64Array.of(1, 1), Float64Array.of(1, 1)],
    ],
    [
      "a u that does not ascend",
      Float64Array.of(0, 0.5, 0.5, 1),
      [Float64Array.of(1, 1, 1, 1), Float64Array.of(1, 1, 1, 1), Float64Array.of(1, 1, 1, 1)],
    ],
    [
      "a negative value",
      Float64Array.of(0, 1),
      [Float64Array.of(1, 1), Float64Array.of(1, -1), Float64Array.of(1, 1)],
    ],
    [
      "a value that is not a number",
      Float64Array.of(0, 1),
      [Float64Array.of(1, 1), Float64Array.of(1, 1), Float64Array.of(Number.NaN, 1)],
    ],
  ])("refuses %s", (_, u, values) => {
    expect(() => phaseTable(u, values)).toThrow(RangeError);
  });

  it("is a total depolariser with no matrix, and keeps a matrix given", () => {
    expect(phaseTable(Float64Array.of(0, 1), ONES).matrix).toBeUndefined();
    const matrix = matrixOf(2, element(2, -0.2));
    expect(phaseTable(Float64Array.of(0, 1), ONES, matrix).matrix).toBe(matrix);
  });

  it("refuses a matrix element of another length or not finite", () => {
    expect(() => phaseTable(Float64Array.of(0, 1), ONES, matrixOf(3))).toThrow(RangeError);
    const notFinite = matrixOf(2, element(2, Number.POSITIVE_INFINITY));
    expect(() => phaseTable(Float64Array.of(0, 1), ONES, notFinite)).toThrow(/b1/);
  });
});
