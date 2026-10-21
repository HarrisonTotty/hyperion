import { describe, expect, it } from "vitest";

import {
  CoarseFineHolds,
  coarseFineHolds,
  coarseFineMorph,
  CORNER_HOLD_BITS,
  EDGE_HOLD_BITS,
  TO_PARENT_SHIFT,
  vertexHoldBits,
} from "./coarseFineHolds";
import { NO_PARENT_SLOT } from "./gpu/uniforms";
import { EDGES, edgeNeighbourAndBack, parentKey, type PatchKey } from "./patchKey";
import terrainPatchWgsl from "./shaders/terrainPatch.wgsl?raw";

/** Patch (`i`, `j`) of `level` on face 2. */
function on2(level: number, i: number, j: number): PatchKey {
  return { face: 2, level, i, j };
}

/** A hold at k = 1 of the edge or corner whose hold at k = 0 is `bit`. */
function toParent(bit: number): number {
  return bit << TO_PARENT_SHIFT;
}

const [U_MIN, U_MAX, V_MIN, V_MAX] = EDGE_HOLD_BITS;
const [CORNER_00, CORNER_64_0, CORNER_64_64, CORNER_0_64] = CORNER_HOLD_BITS;

describe("the morph holds of a draw set (R10.T8.c; decision-r10-t16-leans §9)", () => {
  it("hold nothing among patches of one level", () => {
    const block = [on2(2, 1, 1), on2(2, 2, 1), on2(2, 1, 2), on2(2, 2, 2)];
    expect([...coarseFineHolds(block)]).toEqual([0, 0, 0, 0]);
  });

  it("hold a coarse patch's vertices at k = 0 and its finer neighbours' at k = 1 where they meet, edges and corners", () => {
    // Face 2 at level 1 with its patch (1, 0) split: (0, 0) meets two of the children along its
    // edge x = 64, (1, 1) two along its edge y = 0, and (0, 1) one at its corner (64, 0) alone.
    const keys = [
      on2(1, 0, 0),
      on2(1, 0, 1),
      on2(1, 1, 1),
      on2(2, 2, 0),
      on2(2, 3, 0),
      on2(2, 2, 1),
      on2(2, 3, 1),
    ];
    expect([...coarseFineHolds(keys)]).toEqual([
      U_MAX,
      CORNER_64_0,
      V_MIN,
      toParent(U_MIN | CORNER_0_64),
      0,
      toParent(U_MIN | V_MAX | CORNER_00 | CORNER_64_64 | CORNER_0_64),
      toParent(V_MAX | CORNER_0_64),
    ]);
  });

  it("hold a corner alone where a finer patch touches a coarser one at a corner alone", () => {
    expect([...coarseFineHolds([on2(1, 0, 1), on2(2, 2, 1)])]).toEqual([
      CORNER_64_0,
      toParent(CORNER_0_64),
    ]);
  });

  it("hold the coarse patch's own edge across a face's edge, in its own face's axes", () => {
    const fine = on2(2, 0, 1);
    const [across, back] = edgeNeighbourAndBack(fine, "UMin");
    const coarse = parentKey(across);
    if (coarse === null) {
      throw new Error("a level-2 patch has a parent");
    }
    const [coarseHolds, fineHolds] = coarseFineHolds([coarse, fine]);
    expect([across.face !== fine.face, coarseHolds, (fineHolds ?? 0) & 0xff]).toEqual([
      true,
      EDGE_HOLD_BITS[EDGES.indexOf(back)],
      0,
    ]);
    // Its edge x = 0 at k = 1, and at most a corner beside it.
    expect(((fineHolds ?? 0) >>> TO_PARENT_SHIFT) & 0x0f).toBe(U_MIN);
  });

  it("hold a neighbour two levels coarser as one a level coarser", () => {
    expect([...coarseFineHolds([on2(1, 0, 0), on2(3, 4, 0)])]).toEqual([
      U_MAX,
      toParent(U_MIN | CORNER_0_64),
    ]);
  });

  it("leave nothing of one call's patches in the next", () => {
    const holds = new CoarseFineHolds(4);
    holds.update([on2(1, 0, 0), on2(2, 2, 0)]);
    expect([...holds.update([on2(2, 1, 0), on2(2, 2, 0)]).subarray(0, 2)]).toEqual([0, 0]);
  });

  it("refuses more patches than its capacity", () => {
    expect(() => new CoarseFineHolds(1).update([on2(1, 0, 0), on2(1, 1, 0)])).toThrow(RangeError);
  });
});

describe("the morph factor at held vertices, the shader's `coarseFineMorph`", () => {
  it("names a place's edges and its corner", () => {
    expect([
      vertexHoldBits(0, 0),
      vertexHoldBits(64, 64),
      vertexHoldBits(32, 0),
      vertexHoldBits(0.5, 0),
      vertexHoldBits(31, 33),
    ]).toEqual([U_MIN | V_MIN | CORNER_00, U_MAX | V_MAX | CORNER_64_64, V_MIN, V_MIN, 0]);
  });

  it("holds an edge's whole row at k = 0, and nothing beside it", () => {
    expect([0, 32, 64].map((y) => coarseFineMorph(U_MAX, 64, y, 0.5))).toEqual([0, 0, 0]);
    expect(coarseFineMorph(U_MAX, 63, 32, 0.5)).toBe(0.5);
  });

  it("holds a corner at its vertex alone", () => {
    expect([
      coarseFineMorph(CORNER_64_0, 64, 0, 0.5),
      coarseFineMorph(CORNER_64_0, 64, 1, 0.5),
      coarseFineMorph(CORNER_64_0, 63, 0, 0.5),
    ]).toEqual([0, 0.5, 0.5]);
  });

  it("holds at k = 1 where a coarser patch touches, over a hold at k = 0 at the same vertex", () => {
    const holds = V_MIN | toParent(U_MIN);
    expect([
      coarseFineMorph(holds, 0, 32, 0.25),
      coarseFineMorph(holds, 0, 0, 0.25),
      coarseFineMorph(holds, 32, 0, 0.25),
    ]).toEqual([1, 1, 0]);
  });
});

describe("the shader's holds, as the twin reads them", () => {
  it("decodes the holds at k = 0 from bit 2 and those at k = 1 from bit 2 + TO_PARENT_SHIFT", () => {
    expect([
      terrainPatchWgsl.includes("let toOwn = ((word >> 2u) & 0xffu & at) != 0u;"),
      terrainPatchWgsl.includes(
        `let toParent = ((word >> ${String(2 + TO_PARENT_SHIFT)}u) & 0xffu & at) != 0u;`,
      ),
    ]).toEqual([true, true]);
  });

  it("gives each edge and corner the twin's bit", () => {
    const [x0, x1, y0, y1] = EDGE_HOLD_BITS;
    const [c00, c10, c11, c01] = CORNER_HOLD_BITS;
    const edges =
      `let edges = select(0u, ${String(x0)}u, x == 0u) | select(0u, ${String(x1)}u, x == last) | ` +
      `select(0u, ${String(y0)}u, y == 0u) | select(0u, ${String(y1)}u, y == last);`;
    const corners =
      `let corner = select(0u, ${String(c00)}u, edges == ${String(x0 | y0)}u) | ` +
      `select(0u, ${String(c10)}u, edges == ${String(x1 | y0)}u) | ` +
      `select(0u, ${String(c11)}u, edges == ${String(x1 | y1)}u) | ` +
      `select(0u, ${String(c01)}u, edges == ${String(x0 | y1)}u);`;
    expect([terrainPatchWgsl.includes(edges), terrainPatchWgsl.includes(corners)]).toEqual([
      true,
      true,
    ]);
  });

  it("names no parent by uniforms.ts' NO_PARENT_SLOT", () => {
    expect(terrainPatchWgsl).toContain(
      `const NO_PARENT_SLOT : u32 = 0x${NO_PARENT_SLOT.toString(16)}u;`,
    );
  });
});
