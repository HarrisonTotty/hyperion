import { describe, expect, it } from "vitest";

import {
  allardProfilePerPerturber,
  type AllardTable,
  allardWing,
  impactWing,
  PAIR_GRID,
  pairCellMeansM5,
  parseAllardTable,
  parseAllardWing,
} from "./alkaliPairs";
import { atomicLineSet, atomicLineStrengthCm, perturberScale } from "./atomicKDistributions";
import { lineAt } from "./atomicLinesToys";
import { SPEED_OF_LIGHT_M_PER_S } from "./molecularLines";

/** An expansion table in Allard et al.'s layout (their README's lines 1–16), its values made up. */
const TOY_TABLE = [
  "Date of calculation: Tue Jan  2 14:53:20 2018",
  "TK: 1000.00",
  "lambda: 5891.58",
  "absorption oscillator strength fabs     0.64 cgs",
  " r0 and fabs and pi*r0*fabs      0.28000E-12    0.64080E+00    0.56368E-12",
  " radiator perturber mass:   23.00    2.00",
  " perturber density [cm-3]  0.10000E+22",
  "omega cm-1 1st order.. until      3order",
  "end",
  "  5891.582  1.E+21  7.60885796  5.6367712E-13",
  " 3 3",
  "  21.9821136  1.69550044",
  " -100.  1.0E-03  5.0E-03",
  "  1.0E-02",
  "  100.  2.0E-03  4.0E-03  8.0E-03",
  "  200. -1.0E-04  1.0E-03  1.0E-03",
  "",
].join("\n");

describe("Allard et al.'s expansion tables", () => {
  const table: AllardTable = parseAllardTable(TOY_TABLE);

  it("read the data header and each detuning's first-order coefficient, its lines run over", () => {
    expect([
      table.temperatureK,
      table.referencePerCm3,
      table.normalisationCm,
      table.impactHwhmPerCm,
      table.detuningsPerCm,
      table.firstOrder,
    ]).toEqual([1_000, 1e21, 5.636_771_2e-13, 21.982_113_6, [-100, 100, 200], [1e-3, 2e-3, -1e-4]]);
  });

  it("give the one-perturber profile c₁ ÷ n_ref, linear between detunings and 0 where c₁ falls below 0", () => {
    expect(
      [-100, 0, 200, 300].map((d) => allardProfilePerPerturber(table, undefined, d) * 1e24),
    ).toEqual([expect.closeTo(1, 12), expect.closeTo(1.5, 12), 0, 0]);
  });

  it("take the red wing beyond the table from its wing file, σ ÷ (π r₀ f n_ref), ln σ linear between points", () => {
    const wing = parseAllardWing(
      " -0.3000000E+03  0.4000000E-16\n -0.2000000E+03  0.1000000E-15\n",
    );
    const expected = Math.sqrt(4e-17 * 1e-16) / (5.636_771_2e-13 * 1e21);
    expect(allardProfilePerPerturber(table, wing, -250) / expected).toBeCloseTo(1, 9);
    expect(allardProfilePerPerturber(table, wing, -400)).toBe(0);
  });

  it("refuse a table that is not as the README gives it", () => {
    expect(() => parseAllardTable("TK: 1000\nend\n")).toThrow("README");
  });

  it("refuse a table whose header holds a field that is not a number", () => {
    const broken = TOY_TABLE.replace("  21.9821136  1.69550044", "  width  1.69550044");
    expect(() => parseAllardTable(broken)).toThrow("README");
  });
});

describe("a line's pair wings", () => {
  // K I 766.7 nm's toy: Kurucz's van der Waals field log Γ −7.3 per H atom at 10,000 K, K's mass,
  // Q 2 at every temperature.
  const line = { ...lineAt(766.7, 0.13, 0), logGammaVanDerWaalsPerHydrogen: -7.3 };
  const set = atomicLineSet(
    { key: "K", symbol: "K", stage: 0 },
    19,
    39.0983,
    4.340_664,
    [line],
    [],
    () => 2,
  );
  const strength = atomicLineStrengthCm(line, 1_500, 2);

  it("take the impact wing S γ_p ÷ (π Δ²), γ_p the line's half width per perturber", () => {
    // Kurucz's full width per H atom at 1,500 K, cm³ s⁻¹, scaled to H₂ by Unsöld's rule, as a
    // half width per cm⁻³: ÷ (4π c), c in cm s⁻¹.
    const fullCm3PerS = 10 ** -7.3 * (1_500 / 1e4) ** 0.3 * perturberScale("H2", 39.0983, 0.3);
    const halfWidthPerCm = fullCm3PerS / (4 * Math.PI * SPEED_OF_LIGHT_M_PER_S * 100);
    const expected = (strength * halfWidthPerCm) / (Math.PI * 100 ** 2);
    expect(impactWing(set, 0, "H2", 1_500)(100) / expected).toBeCloseTo(1, 12);
  });

  it("take Allard et al.'s wing as the line's strength times their one-perturber profile", () => {
    const table = parseAllardTable(TOY_TABLE);
    const expected = strength * allardProfilePerPerturber(table, undefined, 0);
    expect(allardWing(set, 0, table, undefined, 1_500)(0) / expected).toBeCloseTo(1, 12);
  });
});

describe("a pair's cell means", () => {
  it("leave out each line's 25 cm⁻¹ core, which the atom's own table holds", () => {
    // A flat wing of 1 cm⁵ about a line at 17,000 cm⁻¹ (588.24 nm): the pair takes only the points
    // beyond 25 cm⁻¹, red of 589.098 nm, none of the 588 nm cell and 40.2% of the 589 nm one.
    const means = pairCellMeansM5([{ centrePerCm: 17_000, wing: () => 1 }]);
    const at = (nm: number): number => means[nm - PAIR_GRID.firstNm] ?? Number.NaN;
    expect([at(588), at(589) / 1e-10, at(590)]).toEqual([0, expect.closeTo(0.402, 2), 1e-10]);
  });
});
