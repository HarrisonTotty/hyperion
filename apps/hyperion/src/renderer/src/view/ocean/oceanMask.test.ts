import { describe, expect, it } from "vitest";

import { oceanAnswerOf, oceanCellCentre } from "../../test/oceanFixtures";
import {
  OPEN_WATER_MAX_ICE,
  oceanCellKindAt,
  oceanCellKindOf,
  oceanMaskIndexAt,
  oceanMaskOf,
} from "./oceanMask";

describe("the ocean mask (R11.T8.b)", () => {
  it("is open water where the water surface is above the coarse ground and never under ice", () => {
    expect(oceanCellKindOf(250, 0)).toBe("open-water");
    expect(oceanCellKindOf(0.001, OPEN_WATER_MAX_ICE)).toBe("open-water");
    // The classifier's majority: a point under water lies in its cell's ice past half its share.
    expect(oceanCellKindOf(250, OPEN_WATER_MAX_ICE + 1)).toBe("sea-ice");
    expect(oceanCellKindOf(250, 255)).toBe("sea-ice");
    // Its ground at or above its water surface: dry, whatever the ice.
    expect(oceanCellKindOf(0, 0)).toBe("dry");
    expect(oceanCellKindOf(0, 255)).toBe("dry");
    expect(OPEN_WATER_MAX_ICE / 255).toBeLessThanOrEqual(0.5);
    expect((OPEN_WATER_MAX_ICE + 1) / 255).toBeGreaterThan(0.5);
  });

  it("decides each surveyed cell of an answer, and no sea over the cells it does not name", () => {
    const answer = oceanAnswerOf([
      { face: 1, i: 3, j: 4, depthM: 3800, levelM: 0, ice: 0 },
      { face: 1, i: 4, j: 4, depthM: 3600, levelM: 0, ice: 200 },
      { face: 1, i: 5, j: 4, depthM: 0, levelM: 410, ice: 0 },
      { face: 4, i: 30, j: 1, depthM: 12, levelM: 0, ice: 127 },
    ]);
    const mask = oceanMaskOf(answer);
    expect([mask.level, mask.faceTexels, mask.openCells]).toEqual([5, 32, 2]);
    expect(oceanCellKindAt(mask, answer, oceanCellCentre(1, 3, 4))).toBe("open-water");
    expect(oceanCellKindAt(mask, answer, oceanCellCentre(1, 4, 4))).toBe("sea-ice");
    expect(oceanCellKindAt(mask, answer, oceanCellCentre(1, 5, 4))).toBe("dry");
    expect(oceanCellKindAt(mask, answer, oceanCellCentre(4, 30, 1))).toBe("open-water");
    // Its neighbour, deep water by its surroundings, is unsurveyed: no coastline is claimed there.
    expect(oceanCellKindAt(mask, answer, oceanCellCentre(1, 3, 5))).toBe("unsurveyed");
    const open = [...mask.open.entries()].filter(([, v]) => v === 1).map(([at]) => at);
    expect(open).toEqual(
      [oceanCellCentre(1, 3, 4), oceanCellCentre(4, 30, 1)]
        .map((d) => oceanMaskIndexAt(mask, d))
        .toSorted((a, b) => a - b),
    );
    // The coverage mask's layout: (face · n + j) · n + i.
    expect(oceanMaskIndexAt(mask, oceanCellCentre(1, 3, 4))).toBe((1 * 32 + 4) * 32 + 3);
  });

  it("is empty for an answer of no cell, an unsurveyed body", () => {
    const mask = oceanMaskOf(oceanAnswerOf([]));
    expect(mask.openCells).toBe(0);
    expect(mask.open.every((v) => v === 0)).toBe(true);
    expect(mask.places.every((p) => p === -1)).toBe(true);
  });

  it("refuses an answer at a level R09 does not serve, or of arrays that disagree", () => {
    const answer = oceanAnswerOf([{ face: 0, i: 0, j: 0, depthM: 1, levelM: 0, ice: 0 }]);
    expect(() => oceanMaskOf({ ...answer, level: 4 })).toThrow(RangeError);
    expect(() => oceanMaskOf({ ...answer, ice: new Uint8Array() })).toThrow(RangeError);
    expect(() => oceanMaskOf({ ...answer, cells: Uint32Array.of(6 * 4 ** 5) })).toThrow(RangeError);
  });
});
