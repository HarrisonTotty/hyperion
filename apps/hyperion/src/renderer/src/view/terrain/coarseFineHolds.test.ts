import { describe, expect, it } from "vitest";

import {
  CoarseFineHolds,
  coarseFineHolds,
  coarseFineMorph,
  CORNER_HOLD_BITS,
  EDGE_HOLD_BITS,
  FADE_STEPS,
  fadeByte,
  fadedMorph,
  fadeFloor,
  type FadeFootprint,
  TO_PARENT_SHIFT,
  TransitionFades,
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

  it("hold a sub-patch as the level it is drawn at, beside patches of its own area's level", () => {
    // (0, 0) at level 2 is drawn as a part of its level-1 parent: beside it, (1, 0) along its edge
    // x = 64, (0, 1) along its edge y = 64 and (1, 1) at its corner (64, 64) are the finer side.
    const keys = [on2(2, 0, 0), on2(2, 1, 0), on2(2, 1, 1), on2(2, 0, 1)];
    const holds = new CoarseFineHolds(4);
    expect([[...holds.update(keys, [1, 2, 2, 2])], [...holds.update(keys)]]).toEqual([
      [U_MAX | V_MAX | CORNER_64_64, toParent(U_MIN), toParent(CORNER_00), toParent(V_MIN)],
      [0, 0, 0, 0],
    ]);
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

/** `bytes` as a fades word, byte n the nth. */
function word(bytes: ReadonlyArray<number>): number {
  return bytes.reduce((acc, byte, n) => (acc | (byte << (8 * n))) >>> 0, 0);
}

describe("the fades' floor on the morph (R10.T16.i)", () => {
  it("takes a fade as its nearest byte of 255 steps", () => {
    expect([fadeByte(0), fadeByte(1), fadeByte(0.5), fadeByte(-1), fadeByte(2)]).toEqual([
      0,
      FADE_STEPS,
      128,
      0,
      FADE_STEPS,
    ]);
  });

  it("floors a vertex at the largest of its instance's own fade and its edges' and corner's", () => {
    const edges = word([10, 20, 30, 40]);
    const corners = word([50, 60, 70, 80]);
    expect([
      fadeFloor(edges, corners, 5, 32, 32),
      fadeFloor(edges, corners, 5, 0, 32),
      fadeFloor(edges, corners, 5, 64, 32),
      fadeFloor(edges, corners, 5, 32, 64),
      fadeFloor(edges, corners, 5, 64, 64),
      fadeFloor(edges, corners, 90, 0, 0),
    ]).toEqual([5, 10, 20, 40, 70, 90].map((b) => b / FADE_STEPS));
  });

  it("raises CDLOD's k to the floor, but not where a hold at k = 0 alone covers the vertex", () => {
    const own = FADE_STEPS;
    expect([
      fadedMorph(0, 0, 0, own, 32, 32, 0.25),
      fadedMorph(U_MAX, 0, 0, own, 64, 32, 0.25),
      fadedMorph(U_MAX | toParent(U_MAX), 0, 0, own, 64, 32, 0.25),
      fadedMorph(U_MAX, 0, 0, own, 32, 32, 0.25),
      fadedMorph(0, 0, 0, 0, 32, 32, 0.25),
    ]).toEqual([1, 0, 1, 1, 0.25]);
  });

  it("is the holds' morph, bit for bit, with no fade", () => {
    const holds = V_MIN | toParent(U_MIN) | CORNER_64_64;
    const ks = [0, 0.1, 0.5, 0.999, 1];
    const places = [0, 1, 32, 63, 64].flatMap((x) => [0, 32, 64].map((y) => [x, y] as const));
    expect(
      places.flatMap(([x, y]) =>
        ks.filter(
          (k) => !Object.is(fadedMorph(holds, 0, 0, 0, x, y, k), coarseFineMorph(holds, x, y, k)),
        ),
      ),
    ).toEqual([]);
  });
});

describe("the shader's fades, as the twin reads them (R10.T16.i)", () => {
  it("decodes byte n of a fades word in steps of the twin's", () => {
    expect(terrainPatchWgsl).toContain(
      `return f32((word >> (8u * n)) & 0xffu) / ${String(FADE_STEPS)}.0;`,
    );
  });

  it("takes each edge's fade at the twin's edge bit and each corner's at its corner bit", () => {
    expect([
      EDGE_HOLD_BITS.every((bit, n) => bit === 1 << n),
      CORNER_HOLD_BITS.every((bit, n) => bit === 16 << n),
      terrainPatchWgsl.includes("if ((at & (1u << n)) != 0u) {"),
      terrainPatchWgsl.includes("if ((at & (16u << n)) != 0u) {"),
      terrainPatchWgsl.includes("var floor = fadeOf(inst.sub.w, 0u);"),
    ]).toEqual([true, true, true, true, true]);
  });

  it("floors k but where a hold at k = 0 alone covers the vertex, as the twin does", () => {
    expect(terrainPatchWgsl).toContain(
      `let toOwnAlone = ((inst.parentQuadrant >> 2u) & 0xffu & at) != 0u && ((inst.parentQuadrant >> ${String(2 + TO_PARENT_SHIFT)}u) & 0xffu & at) == 0u;`,
    );
  });
});

describe("the fades of a draw set from its transitions (R10.T16.i)", () => {
  it("gives an instance in a footprint its fade, and one beside it of its level or finer, on that edge or corner", () => {
    // The footprint is (1, 1) at level 1: (2, 2) at level 2 lies in it; (2, 4) at level 2 lies
    // beside its edge y = 64 and, at its own corner (64, 0), its corner; (4, 1) at level 2 beside
    // its corner (64, 0), at its own (0, 64); (0, 0) and (1, 2) at level 1, its level, beside its
    // corner (0, 0) and its edge y = 64.
    const areas = [on2(2, 2, 2), on2(2, 2, 4), on2(2, 4, 1), on2(1, 0, 0), on2(1, 1, 2)];
    const fades = new TransitionFades(8);
    fades.update(areas, [{ area: on2(1, 1, 1), fade: 0.5 }]);
    const h = fadeByte(0.5);
    expect([
      [...fades.own.subarray(0, 5)],
      [...fades.edges.subarray(0, 5)],
      [...fades.corners.subarray(0, 5)],
    ]).toEqual([
      [h, 0, 0, 0, 0],
      [word([0, h, 0, h]), word([0, 0, h, 0]), 0, 0, word([0, 0, h, 0])],
      [word([0, 0, h, 0]), word([0, h, 0, 0]), word([0, 0, 0, h]), word([0, 0, h, 0]), 0],
    ]);
  });

  it("gives an instance coarser than a footprint beside it no fade", () => {
    const fades = new TransitionFades(2);
    fades.update([on2(1, 0, 0)], [{ area: on2(2, 2, 0), fade: 1 }]);
    expect([fades.own[0], fades.edges[0], fades.corners[0]]).toEqual([0, 0, 0]);
  });

  it("computes one morph at every vertex two instances share, over a refine beside a same-level, a coarser and a finer neighbour", () => {
    // A stand-in, (2, 2) at level 2 of face 2, refined into its four children; beside it, (3, 4)
    // and (3, 5) at level 3 on its left (its children's level), (2, 1) at level 2 below it
    // (coarser than them), and (12, 8) to (12, 11) at level 4 on its right (finer).
    const children = [on2(3, 4, 4), on2(3, 5, 4), on2(3, 4, 5), on2(3, 5, 5)];
    const neighbours = [
      on2(3, 3, 4),
      on2(3, 3, 5),
      on2(2, 2, 1),
      ...[8, 9, 10, 11].map((j) => on2(4, 12, j)),
    ];
    const areas = [...children, ...neighbours];
    const holds = coarseFineHolds(areas);
    const placesAt = (fade: number): string[] => {
      const footprints: FadeFootprint[] = children.map((area) => ({ area, fade }));
      const fades = new TransitionFades(areas.length);
      fades.update(areas, footprints);
      return sharedVertexDisagreements(areas, holds, fades);
    };
    // The one place the holds themselves leave open, with or without a fade: the corner where the
    // finer neighbour, a child and the coarser one meet, two levels apart, where the fine side's
    // step is the one closed (coarseFineHolds.ts' remarks).
    expect([1, 0.75, 0.5, 0.25, 0].map(placesAt)).toEqual(
      Array.from({ length: 5 }, () => ["768 512"]),
    );
  });
});

/**
 * The places, in the finest level's quads, where two of `areas` share a vertex and do not lie on
 * one surface there: patches of one level at different morph factors, or a finer one not at k = 1
 * or a coarser one not at k = 0, at a CDLOD k of 0.3 for both, with the holds and fades given.
 */
function sharedVertexDisagreements(
  areas: ReadonlyArray<PatchKey>,
  holds: Uint16Array,
  fades: TransitionFades,
): string[] {
  const finest = Math.max(...areas.map((a) => a.level));
  const at = new Map<string, Array<{ readonly n: number; readonly k: number }>>();
  areas.forEach((area, n) => {
    const scale = 2 ** (finest - area.level);
    for (let x = 0; x <= 64; x += 1) {
      for (let y = 0; y <= 64; y += 1) {
        if (x !== 0 && x !== 64 && y !== 0 && y !== 64) {
          continue;
        }
        const k = fadedMorph(
          holds[n] ?? 0,
          fades.edges[n] ?? 0,
          fades.corners[n] ?? 0,
          fades.own[n] ?? 0,
          x,
          y,
          0.3,
        );
        const place = `${(64 * area.i + x) * scale} ${(64 * area.j + y) * scale}`;
        const list = at.get(place) ?? [];
        list.push({ n, k });
        at.set(place, list);
      }
    }
  });
  const out = new Set<string>();
  for (const [place, sharing] of at) {
    for (const a of sharing) {
      for (const b of sharing) {
        const [ka, kb] = [areas[a.n]?.level ?? 0, areas[b.n]?.level ?? 0];
        const agree = ka === kb ? a.k === b.k : ka > kb ? a.k === 1 && b.k === 0 : true;
        if (!agree) {
          out.add(place);
        }
      }
    }
  }
  return [...out];
}
