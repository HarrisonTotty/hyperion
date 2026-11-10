import { describe, expect, it } from "vitest";

import { fieldCover } from "../../test/coarseFieldFixtures";
import { CoverageView, fieldCellIndex } from "./coverage";
import { type CellOut, type PatchKey, stepCellInto } from "./patchKey";
import { SurveyedCells } from "./surveyLookup";

/** The field's level in these tests. */
const LEVEL = 6;

/** A field-level cell of face 1. */
function cell(i: number, j: number, face: PatchKey["face"] = 1): PatchKey {
  return { face, level: LEVEL, i, j };
}

/** A block's change surveying `cells`, as the workers report one. */
function surveying(cells: ReadonlyArray<PatchKey>): {
  readonly fieldLevel: number;
  readonly newlySurveyed: Uint32Array;
} {
  const indices = cells.map(fieldCellIndex).toSorted((a, b) => a - b);
  return {
    fieldLevel: LEVEL,
    newlySurveyed: Uint32Array.from(indices.flatMap((n) => [n, n + 1])),
  };
}

/** A survey holding `cells`. */
function surveyOf(cells: ReadonlyArray<PatchKey>): SurveyedCells {
  const survey = new SurveyedCells();
  survey.add(surveying(cells));
  return survey;
}

describe("a body's survey as selection reads it (R10.T16.b)", () => {
  it("calls nothing surveyed before any cell", () => {
    const survey = new SurveyedCells();
    expect([survey.level, survey.surveyedUnder(cell(10, 10))]).toEqual([null, false]);
    expect(survey.surveyedUnder({ face: 1, level: 0, i: 0, j: 0 })).toBe(false);
  });

  it("calls a cell's own patch, its ancestors and the ring about it surveyed, and nothing beyond", () => {
    const survey = surveyOf([cell(10, 10)]);
    const at = (key: PatchKey): boolean => survey.surveyedUnder(key);
    expect([at(cell(10, 10)), at(cell(11, 10)), at(cell(9, 9)), at(cell(11, 11))]).toEqual([
      true,
      true,
      true,
      true,
    ]);
    expect([at(cell(12, 10)), at(cell(10, 12)), at(cell(10, 10, 0))]).toEqual([
      false,
      false,
      false,
    ]);
    expect(at({ face: 1, level: 5, i: 5, j: 5 })).toBe(true);
    // Level 5's (6, 5) holds cells 12 to 13 and 10 to 11: beyond the ring.
    expect(at({ face: 1, level: 5, i: 6, j: 5 })).toBe(false);
    expect([at({ face: 1, level: 0, i: 0, j: 0 }), at({ face: 0, level: 0, i: 0, j: 0 })]).toEqual([
      true,
      false,
    ]);
  });

  it("calls a finer patch surveyed in a surveyed cell, and in a neighbour where it touches their shared edge or corner", () => {
    const survey = surveyOf([cell(10, 10)]);
    // Level 8: four patches a cell along each side.
    const fine = (i: number, j: number): boolean =>
      survey.surveyedUnder({ face: 1, level: LEVEL + 2, i, j });
    // Inside the surveyed cell, at its middle and at its far corner.
    expect([fine(41, 41), fine(43, 43)]).toEqual([true, true]);
    // In the bare cell (11, 10): touching the shared edge, and one patch in from it.
    expect([fine(44, 41), fine(45, 41)]).toEqual([true, false]);
    // In the bare cell (11, 11): at the corner it shares, and along an edge it shares with a bare cell.
    expect([fine(44, 44), fine(44, 45)]).toEqual([true, false]);
  });

  it("folds its ring across a face's edge", () => {
    const edgeCell = cell(2 ** LEVEL - 1, 20);
    const survey = surveyOf([edgeCell]);
    const across: CellOut = { face: 0, i: 0, j: 0 };
    expect(stepCellInto(across, edgeCell, 1, 0)).toBe(true);
    expect(across.face).not.toBe(edgeCell.face);
    expect(
      survey.surveyedUnder({ face: across.face, level: LEVEL, i: across.i, j: across.j }),
    ).toBe(true);
    expect(survey.surveyedUnder({ face: across.face, level: 0, i: 0, j: 0 })).toBe(true);
  });

  it("takes a coverage's surveyed cells as a block's, its margin cells not", () => {
    const cells = [cell(10, 10), cell(11, 10)];
    const indices = cells.map(fieldCellIndex);
    const from = Math.min(...indices);
    const merged = CoverageView.empty(LEVEL).merged(
      fieldCover([from, from + 1, 30], [from + 1, from + 2, 0]),
      1,
    );
    if (!merged.ok) {
      throw new Error(merged.message);
    }
    const covered = new SurveyedCells();
    covered.merge(merged.coverage);
    const blocked = surveyOf([cells[0] ?? cell(0, 0)]);
    const keys = [cell(9, 10), cell(10, 10), cell(11, 10), cell(12, 10), cell(13, 10)];
    expect(keys.map((k) => covered.surveyedUnder(k))).toEqual(
      keys.map((k) => blocked.surveyedUnder(k)),
    );
  });

  it("counts each cell added and the clear as changes, and forgets every cell when cleared", () => {
    const survey = surveyOf([cell(10, 10)]);
    const once = survey.version;
    survey.add(surveying([cell(10, 10)]));
    expect(survey.version).toBe(once);
    survey.add(surveying([cell(30, 30)]));
    expect(survey.version).toBe(once + 1);
    survey.clear();
    expect([survey.version, survey.level, survey.surveyedUnder(cell(10, 10))]).toEqual([
      once + 2,
      null,
      false,
    ]);
  });

  it("holds another level's cells in place of those held", () => {
    const survey = surveyOf([cell(10, 10)]);
    survey.add({
      fieldLevel: 7,
      newlySurveyed: Uint32Array.of(
        fieldCellIndex({ face: 2, level: 7, i: 5, j: 5 }),
        fieldCellIndex({ face: 2, level: 7, i: 5, j: 5 }) + 1,
      ),
    });
    expect([survey.level, survey.surveyedUnder(cell(10, 10))]).toEqual([7, false]);
    expect(survey.surveyedUnder({ face: 2, level: 7, i: 5, j: 5 })).toBe(true);
  });

  it.each([
    ["past a level-6 field's last cell", 6, Uint32Array.of(0, 6 * 4 ** 6 + 1)],
    ["of a level below any field's", 4, Uint32Array.of(0, 1)],
  ])("refuses a run of cells %s", (_case, fieldLevel, newlySurveyed) => {
    expect(() => {
      new SurveyedCells().add({ fieldLevel, newlySurveyed });
    }).toThrow(RangeError);
  });
});

/** A level's patches, face by face, row by row. */
function patchesAt(level: number): PatchKey[] {
  const n = 2 ** level;
  const keys: PatchKey[] = [];
  for (const face of [0, 1, 2, 3, 4, 5] as const) {
    for (let j = 0; j < n; j += 1) {
      for (let i = 0; i < n; i += 1) {
        keys.push({ face, level, i, j });
      }
    }
  }
  return keys;
}

describe("the survey's merge, cell by cell (perf-long-tasks-2)", () => {
  it("steps an interior cell to its neighbours on its own face, as stepCellInto does", () => {
    const out: CellOut = { face: 0, i: 0, j: 0 };
    for (const level of [1, 2, 3, 4, 5]) {
      const last = 2 ** level - 1;
      for (const key of patchesAt(level)) {
        if (key.i === 0 || key.i === last || key.j === 0 || key.j === last) {
          continue;
        }
        for (const [di, dj] of [-1, 0, 1].flatMap((a) => [-1, 0, 1].map((b) => [a, b] as const))) {
          expect(stepCellInto(out, key, di, dj)).toBe(true);
          expect(out).toStrictEqual({ face: key.face, i: key.i + di, j: key.j + dj });
        }
      }
    }
  });

  it("marks every patch at or above the field whose area or ring holds a surveyed cell, as stepping each cell's ring does", () => {
    const level = 5;
    const n = 2 ** level;
    const texels = new Uint8Array(6 * n * n);
    // A fixed scatter, every face's edges and corners among it, and one face whole.
    let seed = 12345;
    for (let at = 0; at < texels.length; at += 1) {
      seed = (seed * 1103515245 + 12345) % 2 ** 31;
      texels[at] = seed % 7 === 0 || Math.floor(at / (n * n)) === 4 ? 1 : 0;
    }
    const survey = new SurveyedCells();
    survey.merge({ level, maskTexels: () => texels });
    // The reference: each surveyed cell and its ring, stepped by stepCellInto alone.
    const dilated = new Set<string>();
    const out: CellOut = { face: 0, i: 0, j: 0 };
    let surveyed = 0;
    for (const key of patchesAt(level)) {
      if (texels[(key.face * n + key.j) * n + key.i] !== 1) {
        continue;
      }
      surveyed += 1;
      dilated.add(`${String(key.face)} ${String(key.i)} ${String(key.j)}`);
      for (const [di, dj] of [-1, 0, 1].flatMap((a) => [-1, 0, 1].map((b) => [a, b] as const))) {
        if (stepCellInto(out, key, di, dj)) {
          dilated.add(`${String(out.face)} ${String(out.i)} ${String(out.j)}`);
        }
      }
    }
    for (let k = 0; k <= level; k += 1) {
      const shift = level - k;
      const marked = new Set(
        [...dilated].map((text) => {
          const [face, i, j] = text.split(" ").map(Number);
          return `${String(face)} ${String((i ?? 0) >> shift)} ${String((j ?? 0) >> shift)}`;
        }),
      );
      for (const key of patchesAt(k)) {
        expect(survey.surveyedUnder(key)).toBe(
          marked.has(`${String(key.face)} ${String(key.i)} ${String(key.j)}`),
        );
      }
    }
    expect(survey.version).toBe(surveyed);
  });
});
