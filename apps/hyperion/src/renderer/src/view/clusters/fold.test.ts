/**
 * Folded dispatches (plan R14, R14.T5.d): a fold's indices a bijection onto the workgroups asked
 * for, within WebGPU's 65,535 a dimension, and the GPU's fold of a count its CPU's.
 */

import { describe, expect, it } from "vitest";

import {
  FOLDED_GROUP_WGSL,
  foldedIndex,
  foldedWorkgroups,
  MAX_FOLDED_WORKGROUPS,
  MAX_WORKGROUPS_PER_DIMENSION,
} from "./fold";
import instanceFoldWgsl from "./instanceFold.wgsl?raw";

/** `instanceFold.wgsl`'s `fold_of` in `u32` arithmetic, as WGSL forms it. */
function gpuFoldOf(n: number): readonly [number, number, number] {
  const most = MAX_WORKGROUPS_PER_DIMENSION;
  const y = Math.max(Math.floor(((n + most - 1) >>> 0) / most), 1);
  return [Math.floor(((n + y - 1) >>> 0) / y), y, 1];
}

/**
 * Whether the fold of `n` takes each index below `n` once, by its workgroups row by row, and leaves
 * the rest past it.
 */
function foldIsBijection(n: number): boolean {
  const [x, y] = foldedWorkgroups(n);
  const seen = new Uint8Array(n);
  let past = 0;
  for (let gy = 0; gy < y; gy += 1) {
    for (let gx = 0; gx < x; gx += 1) {
      const index = foldedIndex(gx, gy, x);
      if (index >= n) {
        past += 1;
      } else if (seen[index] === 1) {
        return false;
      } else {
        seen[index] = 1;
      }
    }
  }
  return seen.every((s) => s === 1) && past === x * y - n;
}

/** Counts across the fold's edges: one row, its end, a second row, many rows. */
const EDGES = [
  0,
  1,
  2,
  63,
  64,
  65_534,
  65_535,
  65_536,
  65_537,
  131_070,
  131_071,
  131_072,
  15_625,
  1_000_000,
  4_194_241,
  2 ** 24,
] as const;

describe("folded dispatches (plan R14, R14.T5.d)", () => {
  it("takes each workgroup asked for exactly once, row by row, at every count across the fold's edges", () => {
    expect(EDGES.filter((n) => !foldIsBijection(n))).toEqual([]);
  });

  it("keeps each dimension within 65,535, covers the count and leaves fewer than its rows past it", () => {
    const counts = [
      ...EDGES,
      MAX_FOLDED_WORKGROUPS,
      MAX_FOLDED_WORKGROUPS - 1,
      ...Array.from({ length: 1000 }, (_, k) => ((k + 1) * 2_654_435_761) % MAX_FOLDED_WORKGROUPS),
    ];
    const broken = counts.filter((n) => {
      const [x, y] = foldedWorkgroups(n);
      return !(
        x <= MAX_WORKGROUPS_PER_DIMENSION &&
        y >= 1 &&
        y <= MAX_WORKGROUPS_PER_DIMENSION &&
        x * y >= n &&
        x * y - n < y
      );
    });
    expect(broken).toEqual([]);
  });

  it("folds a run of at most 65,535 as one row, its indices the unfolded dispatch's", () => {
    expect([foldedWorkgroups(0), foldedWorkgroups(1), foldedWorkgroups(65_535)]).toEqual([
      [0, 1, 1],
      [1, 1, 1],
      [65_535, 1, 1],
    ]);
    expect(foldedWorkgroups(65_536)).toEqual([32_768, 2, 1]);
  });

  it("refuses a count past 65,535 rows of 65,535, or not a whole number", () => {
    expect(() => foldedWorkgroups(MAX_FOLDED_WORKGROUPS + 1)).toThrow(RangeError);
    expect(() => foldedWorkgroups(-1)).toThrow(RangeError);
    expect(() => foldedWorkgroups(1.5)).toThrow(RangeError);
  });

  it("forms the GPU's fold of a count as the CPU's, in u32 arithmetic", () => {
    const counts = [
      ...EDGES,
      ...Array.from({ length: 1000 }, (_, k) => ((k + 1) * 40_503) % 2 ** 24),
    ];
    expect(counts.filter((n) => gpuFoldOf(n).join() !== foldedWorkgroups(n).join())).toEqual([]);
    expect(instanceFoldWgsl).toContain(
      `const MAX_WORKGROUPS_PER_DIMENSION : u32 = ${String(MAX_WORKGROUPS_PER_DIMENSION)}u;`,
    );
    expect(instanceFoldWgsl).toContain(
      "let y = max((n + MAX_WORKGROUPS_PER_DIMENSION - 1u) / MAX_WORKGROUPS_PER_DIMENSION, 1u);",
    );
    expect(instanceFoldWgsl).toContain("return vec3u((n + y - 1u) / y, y, 1u);");
  });

  it("gives the kernels the index the CPU's fold takes, row by row", () => {
    expect(FOLDED_GROUP_WGSL).toContain("return group.y * groups.x + group.x;");
  });
});
