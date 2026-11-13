import { describe, expect, it } from "vitest";

import type { TerrainLayers } from "./messages";
import { needsRebake, staleInFlight, surveysAny } from "./rebake";

/** Layers of a bake, complete or not, with `unsurveyed` cells and a map refused for `refused`. */
function layers(
  complete: boolean,
  unsurveyed: readonly number[],
  refused: readonly number[] = [],
): TerrainLayers {
  return {
    palette: new Uint8Array(16),
    weights: new Uint8Array(),
    surveyMask: new Uint8Array(),
    fieldLevel: 8,
    boundM: 1,
    sigmaM: 0,
    complete,
    unsurveyedCells: Uint32Array.from(unsurveyed),
    photometry: new Float32Array(40),
    horizonNear: new Uint32Array(),
    horizonFar: new Uint32Array(),
    horizonComplete: refused.length === 0,
    horizonRefusedCells: Uint32Array.from(refused),
  };
}

/** A change surveying `runs` and bringing `held`, each run [first, past last). */
function change(
  runs: ReadonlyArray<readonly [number, number]>,
  held: ReadonlyArray<readonly [number, number]> = runs,
): { newlySurveyed: Uint32Array; newlyHeld: Uint32Array } {
  return { newlySurveyed: Uint32Array.from(runs.flat()), newlyHeld: Uint32Array.from(held.flat()) };
}

describe("R10's rebake rule", () => {
  it("finds a cell in a block's runs, ends excluded", () => {
    const runs = Uint32Array.from([3, 6, 9, 10, 11, 13]);
    const cells = Array.from({ length: 15 }, (_, cell) => cell);
    expect(cells.filter((cell) => surveysAny(runs, Uint32Array.of(cell)))).toEqual([
      3, 4, 5, 9, 11, 12,
    ]);
    expect(surveysAny(runs, Uint32Array.from([0, 6, 10, 13]))).toBe(false);
    expect(surveysAny(runs, Uint32Array.from([0, 1, 12]))).toBe(true);
    expect(surveysAny(new Uint32Array(), Uint32Array.from([1, 2]))).toBe(false);
    expect(surveysAny(runs, new Uint32Array())).toBe(false);
  });

  it("bakes a complete patch again only where a block surveys a cell its mask holds unsurveyed", () => {
    expect(
      needsRebake(
        layers(true, [7, 40]),
        change([
          [0, 7],
          [41, 50],
        ]),
      ),
    ).toBe(false);
    expect(needsRebake(layers(true, [7, 40]), change([[0, 8]]))).toBe(true);
    expect(needsRebake(layers(true, [7, 40]), change([[40, 41]]))).toBe(true);
    expect(needsRebake(layers(true, []), change([[0, 100]]))).toBe(false);
  });

  it("bakes an incomplete patch again on any block, a margin's alone among them", () => {
    expect(needsRebake(layers(false, []), change([]))).toBe(true);
    expect(needsRebake(layers(false, [3]), change([[10, 11]]))).toBe(true);
  });

  it("bakes a patch again on a block that brings a cell its horizon map was refused for", () => {
    // Brought as margin alone: not surveyed, but held anew.
    expect(needsRebake(layers(true, [], [12, 30]), change([], [[29, 31]]))).toBe(true);
    // Brought surveyed.
    expect(needsRebake(layers(true, [], [12, 30]), change([[12, 13]]))).toBe(true);
    // A block that brings none of them passes it over, its map incomplete or not.
    expect(
      needsRebake(
        layers(true, [], [12, 30]),
        change(
          [[0, 12]],
          [
            [13, 30],
            [31, 40],
          ],
        ),
      ),
    ).toBe(false);
    // A cell surveyed anew that was held as margin is not brought anew.
    expect(needsRebake(layers(true, [], [12]), change([[12, 13]], []))).toBe(false);
  });

  it("never bakes the test planet's patches again, which read no field", () => {
    expect(needsRebake(null, change([[0, 100]]))).toBe(false);
  });

  it("bakes a bake in flight again unless it is complete, its map complete and its mask whole", () => {
    expect(staleInFlight(layers(true, []))).toBe(false);
    expect(staleInFlight(layers(false, []))).toBe(true);
    expect(staleInFlight(layers(true, [4]))).toBe(true);
    expect(staleInFlight(layers(true, [], [9]))).toBe(true);
    expect(staleInFlight(null)).toBe(true);
  });
});
