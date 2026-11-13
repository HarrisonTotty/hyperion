import { describe, expect, it } from "vitest";

import bounds from "../renderer/src/view/atmosphere/absorbers/bounds.json" with { type: "json" };
import { cellOf, cellWidthPerCm } from "./atomicLines";
import { type BoundEntry } from "./atomicLinesCli";
import {
  HCL_LEVELS_PER_CM,
  HCL_OVERTONE_NODES_K,
  hclOvertoneCellMeansM2,
  type HclOvertoneLines,
  withBoundEntry,
} from "./hclOvertones";
import { saturationPressurePa } from "./kDistributions";
import { HCL_TABLE_BELOW_PER_CM, LINE_SOURCES } from "./kDistributionsCli";
import { type LineList, linesBeyond } from "./molecularLines";

/** A toy line list of H³⁵Cl lines at wavenumbers, each S(296) 10⁻²⁷ cm from E″ = 0. */
function toyLines(wavenumbersPerCm: ReadonlyArray<number>): LineList {
  const n = wavenumbersPerCm.length;
  const filled = (v: number): Float64Array => new Float64Array(n).fill(v);
  const foreign = (): LineList["foreignHalfWidthsPerCmAtm"] => ({
    H2: filled(Number.NaN),
    He: filled(Number.NaN),
    CO2: filled(Number.NaN),
    H2O: filled(Number.NaN),
  });
  return {
    isotopologues: new Uint16Array(n).fill(52),
    wavenumbersPerCm: Float64Array.from(wavenumbersPerCm),
    intensitiesCm: filled(1e-27),
    lowerEnergiesPerCm: filled(0),
    airHalfWidthsPerCmAtm: filled(0.05),
    airExponents: filled(0.7),
    selfHalfWidthsPerCmAtm: filled(0.2),
    foreignHalfWidthsPerCmAtm: foreign(),
    selfExponents: filled(Number.NaN),
    foreignExponents: foreign(),
  };
}

/** Partition sums equal at every temperature, so that S(T) is S(296) but for its emission factor. */
const FLAT_SUMS = { firstK: 1, values: new Float64Array(1_000).fill(100) };

describe("a line list cut at a wavenumber", () => {
  it("keeps the lines below on one side and those at or above on the other", () => {
    const lines = toyLines([19_000, 19_029.5, 19_100]);
    expect([
      Array.from(linesBeyond(lines, 19_029.5, "below").wavenumbersPerCm),
      Array.from(linesBeyond(lines, 19_029.5, "above").wavenumbersPerCm),
    ]).toEqual([[19_000], [19_029.5, 19_100]]);
  });

  it("cuts HCl's table at the 525/526 nm cells' edge, the 8-0 band above", () => {
    expect([HCL_TABLE_BELOW_PER_CM, LINE_SOURCES["HCl"]?.tableBelowPerCm]).toEqual([
      1e7 / 525.5,
      1e7 / 525.5,
    ]);
  });
});

describe("HCl's saturation", () => {
  const curve = LINE_SOURCES["HCl"]?.saturation;

  it("is its triple point's 13.8 kPa at 159.07 K", () => {
    expect(curve === undefined ? Number.NaN : saturationPressurePa(curve, 159.07)).toBeCloseTo(
      13_800,
      6,
    );
  });

  it("gives 2.1 Pa at 100 K and 103.4 kPa at the 188.16 K boiling point, 2% over 1 atm", () => {
    const at = (t: number): number =>
      curve === undefined ? Number.NaN : saturationPressurePa(curve, t);
    expect([at(100), at(188.16)]).toEqual([expect.closeTo(2.08, 2), expect.closeTo(103_400, -2)]);
  });
});

describe("HCl's overtones part", () => {
  const data: HclOvertoneLines = {
    lines: toyLines([19_500]),
    partitionSums: new Map([[52, FLAT_SUMS]]),
  };
  const means = hclOvertoneCellMeansM2(data, 296);

  it("puts the 8-0 line in its own cell, S over the cell's width, m²", () => {
    const cell = cellOf(1e7 / 19_500);
    expect(means[cell]).toBeCloseTo((1e-27 / cellWidthPerCm(cell)) * 1e-4, 40);
  });

  it("moves it up H³⁵Cl's ladder for 9-0 to 11-0, a tenth for each quantum", () => {
    const ratios = ([9, 10, 11] as const).map((v) => {
      const cell = cellOf(1e7 / (19_500 + HCL_LEVELS_PER_CM[v] - HCL_LEVELS_PER_CM[8]));
      return ((means[cell] ?? 0) * cellWidthPerCm(cell)) / 1e-31;
    });
    expect(ratios).toEqual([
      expect.closeTo(0.1, 6),
      expect.closeTo(0.01, 6),
      expect.closeTo(0.001, 6),
    ]);
  });

  it("is in the client's bounds file, estimated, for R08.T4.f, at the table's grid", () => {
    const entry = bounds.bounds.find((b) => b.key === "HCl" && b.part === "overtones");
    expect([entry?.task, entry?.basis, entry?.temperaturesK]).toEqual([
      "R08.T4.f",
      "estimated",
      [...HCL_OVERTONE_NODES_K],
    ]);
  });
});

/** A toy bounds entry with a key, task and part. */
function toyEntry(key: string, part: string | undefined, value: number): BoundEntry {
  return {
    key,
    kind: "species",
    task: "R08.T4.f",
    ...(part === undefined ? {} : { part }),
    basis: "estimated",
    temperaturesK: [296],
    source: "a test",
    method: "none",
    cellMeansM2: [[value]],
  };
}

/** A written file as JSON of unknown shape, for the matchers to check. */
function entriesOf(text: string): unknown {
  const parsed: unknown = JSON.parse(text);
  return parsed;
}

describe("writing an entry into a bounds file", () => {
  const file = `{\n  "description": "toy",\n  "bounds": [\n    {\n      "key": "CO",\n      "kind": "species",\n      "task": "R08.T4.f",\n      "part": "overtones",\n      "basis": "estimated",\n      "temperaturesK": [296],\n      "source": "a",\n      "method": "b",\n      "cellMeansM2": [\n        [1]\n      ]\n    }\n  ]\n}\n`;

  it("adds an entry at the end, keeping the others byte for byte", () => {
    const written = withBoundEntry(file, toyEntry("HCl", "overtones", 2));
    expect(written.startsWith(file.slice(0, file.lastIndexOf("\n  ]")))).toBe(true);
    expect(entriesOf(written)).toMatchObject({
      bounds: [{ key: "CO" }, { key: "HCl", part: "overtones" }],
    });
  });

  it("replaces the entry of its key, task and part", () => {
    const once = withBoundEntry(file, toyEntry("HCl", "overtones", 2));
    const twice = withBoundEntry(once, toyEntry("HCl", "overtones", 3));
    expect(entriesOf(twice)).toMatchObject({
      bounds: [{ cellMeansM2: [[1]] }, { key: "HCl", cellMeansM2: [[3]] }],
    });
  });

  it("replaces an entry with no part in place, never beside one with a part", () => {
    const once = withBoundEntry(file, toyEntry("CO", undefined, 2));
    const twice = withBoundEntry(once, toyEntry("CO", undefined, 3));
    expect(entriesOf(twice)).toEqual({
      description: "toy",
      bounds: [
        expect.objectContaining({ key: "CO", part: "overtones", cellMeansM2: [[1]] }),
        expect.not.objectContaining({ part: expect.anything() }),
      ],
    });
    expect(entriesOf(twice)).toMatchObject({ bounds: [{}, { cellMeansM2: [[3]] }] });
  });
});
