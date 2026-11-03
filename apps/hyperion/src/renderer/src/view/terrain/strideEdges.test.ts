import { describe, expect, it } from "vitest";

import { seededRandom } from "../../test/seededRandom";
import {
  edgeBandIndex,
  heldEdgeBandsM,
  NO_STRIDE_BAND as NONE,
  NO_STRIDE_SCALE,
  type StrideErrorsM,
  strideMorphLevel,
  type StrideScale,
  vertexStrideBandM,
} from "../clusters/strides";
import { type CellOut, childKeys, FACES, facePoint, type PatchKey, stepCellInto } from "./patchKey";
import { type StrideEdgePatch, StrideEdges } from "./strideEdges";

/** A scale whose need is a cluster's error itself, so that the bands read as the errors. */
const UNIT: StrideScale = { statedM: 0, perErrorM: 1 };

/** A bake's errors: every block's e₂, e₄ and e₈ `errors`, and `blocks`' their own. */
function errorsOf(
  errors: StrideErrorsM,
  blocks: ReadonlyArray<readonly [block: number, errors: StrideErrorsM]> = [],
): Float16Array {
  const out = new Float16Array(4 * 64);
  for (let block = 0; block < 64; block += 1) {
    out.set([0, ...errors], 4 * block);
  }
  for (const [block, own] of blocks) {
    out.set([0, ...own], 4 * block);
  }
  return out;
}

/** A measured whole patch `key` of the unit scale and `clusterErrors`. */
function measured(key: PatchKey, clusterErrors: Float16Array): StrideEdgePatch {
  return { key, level: key.level, measured: { scale: UNIT, clusterErrors } };
}

/** An unmeasured patch `key`, drawn at `level`: a sub-patch's its ancestor's. */
function unmeasured(key: PatchKey, level = key.level): StrideEdgePatch {
  return { key, level, measured: null };
}

/** `patches`' edges' bands, each level's largest errors `largest`. */
function edgesOf(
  patches: ReadonlyArray<StrideEdgePatch>,
  largest: (level: number) => StrideErrorsM | null = () => null,
): StrideEdges {
  const edges = new StrideEdges(16);
  edges.update(patches, largest);
  return edges;
}

/** Patch `n`'s bands of edge `edge` at its block `t`, towards strides 2, 4 and 8. */
function bandsAt(edges: StrideEdges, n: number, edge: number, t: number): number[] {
  const of = edges.edgesOf(n);
  return [0, 1, 2].map((l) => of[edgeBandIndex(edge, t, l)] ?? Number.NaN);
}

/** Patch `n`'s bands of edge `edge` at each of its blocks. */
function edgeOf(edges: StrideEdges, n: number, edge: number): number[][] {
  return Array.from({ length: 8 }, (_, t) => bandsAt(edges, n, edge, t));
}

/** Level 7's largest errors alone. */
function LEVEL_7_LARGEST(level: number): StrideErrorsM | null {
  return level === 7 ? [21, 41, 51] : null;
}

const A: StrideErrorsM = [10, 20, 40];
const B: StrideErrorsM = [15, 18, 50];
const C: StrideErrorsM = [12, 30, 60];

describe("the edges' shared stride bands (plan R14, R14.T3.c's shared bands)", () => {
  /** Two level-6 patches side by side, the right's block on its x = 0 at y ÷ 8 = 3 its own. */
  function sideBySide(): StrideEdges {
    return edgesOf([
      measured({ face: 0, level: 6, i: 10, j: 20 }, errorsOf(A)),
      measured({ face: 0, level: 6, i: 11, j: 20 }, errorsOf(B, [[24, [30, 5, 60]]])),
    ]);
  }

  it("shares a same-level edge block by block as the larger of both blocks' needs", () => {
    const edges = sideBySide();
    const shared = Array.from({ length: 8 }, (_, t) => (t === 3 ? [30, 20, 60] : [15, 20, 50]));
    expect([edgeOf(edges, 0, 1), edgeOf(edges, 1, 0)]).toEqual([shared, shared]);
  });

  it("keeps a patch's own needs on an edge with no drawn patch beside it", () => {
    expect(bandsAt(sideBySide(), 0, 0, 5)).toEqual([...A]);
  });

  it("shares a same-level edge beside a patch that is not measured as the larger of its own need and its level's largest error's, the other's none", () => {
    const edges = edgesOf(
      [
        measured({ face: 0, level: 6, i: 10, j: 20 }, errorsOf(A)),
        unmeasured({ face: 0, level: 6, i: 10, j: 21 }),
      ],
      (level) => (level === 6 ? [12, 25, 35] : null),
    );
    expect([bandsAt(edges, 0, 3, 2), bandsAt(edges, 1, 2, 2), bandsAt(edges, 1, 0, 0)]).toEqual([
      [12, 25, 40],
      [NONE, NONE, NONE],
      [NONE, NONE, NONE],
    ]);
  });

  it("shares a coarse block's levels 0 and 1 with its two finer blocks' levels 1 and 2 along it, its level 2 none", () => {
    const coarse = { face: 0, level: 6, i: 10, j: 20 } as const;
    // The two children of the cell at i + 1 that touch the coarse patch's x = 64; the lower's block
    // at its x = 0 and y ÷ 8 = 1 its own.
    const lower = { face: 0, level: 7, i: 22, j: 40 } as const;
    const upper = { face: 0, level: 7, i: 22, j: 41 } as const;
    const edges = edgesOf([
      measured(coarse, errorsOf(C)),
      measured(lower, errorsOf(A, [[8, [10, 45, 40]]])),
      measured(upper, errorsOf(B)),
    ]);
    expect([edgeOf(edges, 0, 1), edgeOf(edges, 1, 0), edgeOf(edges, 2, 0)]).toEqual([
      [
        [45, 40, NONE],
        [20, 40, NONE],
        [20, 40, NONE],
        [20, 40, NONE],
        [18, 50, NONE],
        [18, 50, NONE],
        [18, 50, NONE],
        [18, 50, NONE],
      ],
      [[10, 45, 40], [10, 45, 40], ...Array.from({ length: 6 }, () => [10, 20, 40])],
      Array.from({ length: 8 }, () => [15, 18, 50]),
    ]);
  });

  it("shares a coarse block beside a finer patch that is not measured as the larger of its own need and its level's largest error's, the finer one's none", () => {
    const edges = edgesOf(
      [
        measured({ face: 0, level: 6, i: 10, j: 20 }, errorsOf(C)),
        measured({ face: 0, level: 7, i: 22, j: 40 }, errorsOf(A)),
        unmeasured({ face: 0, level: 7, i: 22, j: 41 }),
      ],
      (level) => (level === 6 ? [13, 35, 70] : [21, 41, 51]),
    );
    expect([bandsAt(edges, 0, 1, 0), bandsAt(edges, 0, 1, 5), edgeOf(edges, 2, 0)]).toEqual([
      [20, 40, NONE],
      [13, 35, NONE],
      Array.from({ length: 8 }, () => [NONE, NONE, NONE]),
    ]);
  });

  it("shares a finer patch's levels 1 and 2 beside a coarser one that is not measured with its level's largest errors there, the coarser one's none", () => {
    const edges = edgesOf(
      [
        unmeasured({ face: 0, level: 6, i: 10, j: 20 }),
        measured({ face: 0, level: 7, i: 22, j: 40 }, errorsOf(A)),
      ],
      LEVEL_7_LARGEST,
    );
    expect([bandsAt(edges, 1, 0, 3), bandsAt(edges, 0, 1, 1)]).toEqual([
      [10, 41, 51],
      [NONE, NONE, NONE],
    ]);
  });

  it("shares a patch's levels 1 and 2 beside a sub-patch of its key's level, drawn a level coarser, with its level's largest errors there", () => {
    // A level-7 area drawn by its level-6 ancestor's slot beside a level-7 patch.
    const edges = edgesOf(
      [
        unmeasured({ face: 0, level: 7, i: 20, j: 40 }, 6),
        measured({ face: 0, level: 7, i: 21, j: 40 }, errorsOf(A)),
      ],
      LEVEL_7_LARGEST,
    );
    expect(bandsAt(edges, 1, 0, 6)).toEqual([10, 41, 51]);
  });

  it("shares a patch's edge beside a sub-patch keyed finer than it, drawn at its level, as the larger of its own need and its level's largest error's", () => {
    // A level-7 area drawn by its level-6 ancestor's slot, (11, 20), beside the level-6 patch
    // (10, 20)'s x = 64 at its lower half.
    const edges = edgesOf(
      [
        measured({ face: 0, level: 6, i: 10, j: 20 }, errorsOf(A)),
        unmeasured({ face: 0, level: 7, i: 22, j: 40 }, 6),
      ],
      (level) => (level === 6 ? [12, 25, 35] : null),
    );
    expect([bandsAt(edges, 0, 1, 1), bandsAt(edges, 0, 1, 6), bandsAt(edges, 1, 0, 1)]).toEqual([
      [12, 25, 40],
      [...A],
      [NONE, NONE, NONE],
    ]);
  });

  it("gives none to both sides of an edge two levels apart", () => {
    const edges = edgesOf([
      measured({ face: 0, level: 6, i: 10, j: 20 }, errorsOf(A)),
      measured({ face: 0, level: 8, i: 44, j: 80 }, errorsOf(B)),
    ]);
    expect([bandsAt(edges, 0, 1, 0), bandsAt(edges, 1, 0, 0)]).toEqual([
      [NONE, NONE, NONE],
      [NONE, NONE, NONE],
    ]);
  });

  it("shares an edge across a cube face's edge alike from both sides", () => {
    const own = { face: 0, level: 3, i: 7, j: 3 } as const;
    const cell: CellOut = { face: 0, i: 0, j: 0 };
    if (!stepCellInto(cell, own, 1, 0)) {
      throw new Error("the step crosses a face's edge, not a corner");
    }
    const beside = { face: cell.face, level: 3, i: cell.i, j: cell.j };
    const edges = edgesOf([measured(own, errorsOf(A)), measured(beside, errorsOf(B))]);
    const sharedOn = (n: number): number =>
      [0, 1, 2, 3].filter((e) => edgeOf(edges, n, e).every((b) => b.join() === "15,20,50")).length;
    expect([beside.face !== own.face, sharedOn(0), sharedOn(1)]).toEqual([true, 1, 1]);
  });
});

/** A drawn patch of the random sets: its key, the level it is drawn at, and its strides. */
interface SetPatch {
  readonly patch: StrideEdgePatch;
  readonly scale: StrideScale;
  readonly clusterErrors: Float16Array;
  readonly sub: boolean;
}

/** A finest level the random sets reach, past which their lattice is exact. */
const SET_FINEST = 5;

/**
 * A random set of drawn patches over the whole cube from `seed`: the 24 level-1 cells split at
 * random down to level 5, a sixth of the leaves not measured (a stand-in or an unmeasured bake) and
 * a twentieth sub-patches drawn at their parents' level, each measured one of a random scale and
 * random errors block by block.
 */
function randomSet(seed: number): SetPatch[] {
  const random = seededRandom(seed);
  const leaves: PatchKey[] = [];
  const split = (key: PatchKey): void => {
    if (key.level < SET_FINEST && random() < 0.55 - 0.08 * key.level) {
      for (const child of childKeys(key)) {
        split(child);
      }
    } else {
      leaves.push(key);
    }
  };
  for (const face of FACES) {
    for (let i = 0; i < 2; i += 1) {
      for (let j = 0; j < 2; j += 1) {
        split({ face, level: 1, i, j });
      }
    }
  }
  return leaves.map((key) => {
    const roll = random();
    const scale: StrideScale = { statedM: 5 + 30 * random(), perErrorM: 20 + 40 * random() };
    const clusterErrors = new Float16Array(4 * 64);
    for (let block = 0; block < 64; block += 1) {
      const two = 0.2 + 3 * random();
      const four = two * (1.1 + 1.5 * random());
      clusterErrors.set([two / 2, two, four, four * (1.1 + 1.5 * random())], 4 * block);
    }
    if (roll < 0.05 && key.level > 1) {
      return {
        patch: unmeasured(key, key.level - 1),
        scale: NO_STRIDE_SCALE,
        clusterErrors,
        sub: true,
      };
    }
    if (roll < 0.2) {
      return { patch: unmeasured(key), scale: NO_STRIDE_SCALE, clusterErrors, sub: false };
    }
    return {
      patch: { key, level: key.level, measured: { scale, clusterErrors } },
      scale,
      clusterErrors,
      sub: false,
    };
  });
}

/** Vertex (x, y) of `key`'s grid on the cube at {@link SET_FINEST}'s lattice, as a string. */
function cubeKeyOf(key: PatchKey, x: number, y: number): string {
  const quads = 64 * 2 ** key.level;
  const scale = 2 ** (SET_FINEST - key.level);
  const p = facePoint(key.face, 2 * (64 * key.i + x) - quads, 2 * (64 * key.j + y) - quads, quads);
  return p.map((c) => c * scale).join(",");
}

describe("a vertex two patches share (plan R14, R14.T3.c's shared bands)", () => {
  it.each([0x5e7a, 0x5e7b, 0x5e7c, 0x5e7d])(
    "takes one band from both, held as their records hold it, over a random set %#: coarse–fine edges, face edges, stand-ins and sub-patches among them",
    (seed) => {
      const set = randomSet(seed);
      const largest = new Map<number, [number, number, number]>();
      for (const { patch, clusterErrors } of set) {
        if (patch.measured !== null) {
          const most = largest.get(patch.level) ?? [0, 0, 0];
          for (let block = 0; block < 64; block += 1) {
            for (let l = 0; l < 3; l += 1) {
              most[l] = Math.max(most[l] ?? 0, clusterErrors[4 * block + l + 1] ?? 0);
            }
          }
          largest.set(patch.level, most);
        }
      }
      const edges = new StrideEdges(set.length);
      const bandsM = edges.update(
        set.map(({ patch }) => patch),
        (level) => largest.get(level) ?? null,
      );
      // Each patch's boundary vertices by their place on the cube, the band each takes there.
      const at = new Map<string, Array<{ readonly n: number; readonly band: number }>>();
      set.forEach(({ patch, scale, clusterErrors, sub }, n) => {
        const held = heldEdgeBandsM(edges.edgesOf(n));
        // A sub-patch draws its ancestor's grid, every second vertex of its area's.
        const step = sub ? 2 : 1;
        for (let s = step; s < 64; s += step) {
          for (const [x, y] of [
            [0, s],
            [64, s],
            [s, 0],
            [s, 64],
          ] as const) {
            const level = strideMorphLevel(x, y) - (sub ? 1 : 0);
            const band =
              level >= 3 ? NONE : vertexStrideBandM(scale, clusterErrors, held, x, y, level);
            const list = at.get(cubeKeyOf(patch.key, x, y)) ?? [];
            list.push({ n, band });
            at.set(cubeKeyOf(patch.key, x, y), list);
          }
        }
      });
      let shared = 0;
      let striding = 0;
      let coarseFine = 0;
      let acrossFaces = 0;
      let beside = 0;
      const unlike: string[] = [];
      for (const [place, list] of at) {
        const [a, b] = list;
        if (a === undefined || b === undefined) {
          continue;
        }
        const pa = set[a.n];
        const pb = set[b.n];
        if (pa === undefined || pb === undefined) {
          throw new Error("the set's patches");
        }
        const apart = Math.abs(pa.patch.level - pb.patch.level);
        const both = pa.patch.measured !== null && pb.patch.measured !== null;
        if (!both) {
          beside += 1;
          // A side that is not measured never moves.
          for (const [p, side] of [
            [pa, a],
            [pb, b],
          ] as const) {
            if (p.patch.measured === null && side.band !== NONE) {
              unlike.push(`${place}: an unmeasured side's band ${String(side.band)}`);
            }
          }
          continue;
        }
        shared += 1;
        striding += a.band > 0 ? 1 : 0;
        coarseFine += apart === 1 ? 1 : 0;
        acrossFaces += pa.patch.key.face === pb.patch.key.face ? 0 : 1;
        const expected = apart > 1 ? [NONE, NONE] : [a.band, a.band];
        if (a.band !== expected[0] || b.band !== expected[1]) {
          unlike.push(
            `${place}: levels ${String(pa.patch.level)} and ${String(pb.patch.level)}, bands ${String(a.band)} and ${String(b.band)}`,
          );
        }
      }
      expect(unlike.slice(0, 5)).toEqual([]);
      expect(bandsM.every((band) => band === NONE || (band > 0 && Number.isFinite(band)))).toBe(
        true,
      );
      expect(shared).toBeGreaterThan(2_000);
      expect(striding).toBeGreaterThan(1_000);
      expect(coarseFine).toBeGreaterThan(200);
      expect(acrossFaces).toBeGreaterThan(100);
      expect(beside).toBeGreaterThan(200);
    },
  );
});
