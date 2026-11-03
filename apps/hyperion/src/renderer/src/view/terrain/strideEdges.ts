/**
 * The shared stride bands of a view's drawn patches' edges (plan R14, R14.T3.c;
 * fix-r14-t3c-shared-bands, research-leans-35's ruling, 2026-11-03; `clusters/strides.ts`' remarks):
 * at each block along each edge, the band of the edge's vertices there, the largest need of every
 * block that holds them, its own and the neighbour's adjacent ones, so that both patches place
 * every vertex they share alike.
 *
 * @remarks
 * Each patch looks up the drawn patch across each of its edges, of its level or a coarser one, as
 * `CoarseFineHolds` does, across a cube face's edge too, and finds the neighbour's block at each of
 * its own by the place of the block's stretch's middle in the neighbour's grid (`coarsePlaceOf`):
 * - **Of its level**: at each block along the edge and each stride, the larger of its block's need
 *   and the neighbour's adjacent block's; each side forms the same.
 * - **Coarser by one level**: a fine patch's vertex of morph level j + 1 on the edge is the coarse
 *   one's of level j, and a coarse block's stretch the fine patch's two blocks'. So at each coarse
 *   block along it the band at the coarse levels 0 and 1 is the largest of the coarse block's need
 *   there and the two fine blocks' at levels 1 and 2, which the fine side takes at its levels 1 and
 *   2 for both its blocks; the coarse edge's level-2 vertices, the fine blocks' corners, never move
 *   (none), and the fine edge's level-0 ones, at the coarse grid's midpoints, are held on its chords
 *   (their own block's need, which the coarse–fine hold at k = 1 overrides).
 * - **Not a measured whole patch** (a sub-patch, an unmeasured bake), on either side of an edge of
 *   one level or one apart: the measured side takes the larger of its own need and its level's
 *   largest e_s's, and the other's vertices never move: they part by at most that e_s, under τ′
 *   there, which the skirts close. A sub-patch is keyed by its area, finer than the level it is
 *   drawn at, so a patch of that level beside it is given its share from the sub-patch's side.
 * - **Two or more levels apart** (only beside a stand-in): neither side's edge moves (none).
 *
 * An edge with no drawn patch beside it keeps its own blocks' needs. The corners lie at morph level
 * 3 on every side and never move.
 */

import {
  blockErrorM,
  EDGE_BANDS,
  edgeBandIndex,
  NO_STRIDE_BAND,
  sharedStrideBandM,
  type StrideErrorsM,
  type StrideScale,
  strideNeedM,
} from "../clusters/strides";
import { coarsePlaceOf } from "./coarseFineHolds";
import { type CellOut, type Face, MAX_LEVEL, type PatchKey, stepCellInto } from "./patchKey";
import { PatchKeyTable } from "./patchKeyTable";

/** An edge's step to the cell beside it, in `EDGES`' order: x = 0, x = 64, y = 0, y = 64. */
const EDGE_STEPS: ReadonlyArray<readonly [di: number, dj: number]> = [
  [-1, 0],
  [1, 0],
  [0, -1],
  [0, 1],
];

/** The strides an edge's band is held for, its morph levels 0 to 2. */
const LEVELS = 3;

/** What lies beside a coarse block along an edge, finer by one level or more (`#finerKind`). */
const NO_FINER = 0;
const MEASURED_FINER = 1;
const UNMEASURED_FINER = 2;
const FAR_FINER = 3;

/** One drawn patch as the edges take it. */
export interface StrideEdgePatch {
  /** Its area, as the draw set draws it (a sub-patch's the missing patch's). */
  readonly key: PatchKey;
  /** The level it is drawn at: a sub-patch's its ancestor's. */
  readonly level: number;
  /** Its scale and its bake's clusters' errors where it is a measured whole patch; else `null`. */
  readonly measured: {
    readonly scale: StrideScale;
    readonly clusterErrors: ArrayLike<number>;
  } | null;
}

/** A patch key whose fields are rewritten in place. */
interface ScratchKey {
  face: Face;
  level: number;
  i: number;
  j: number;
}

/** Block `t` along edge `edge` of a patch's 8 × 8. */
function edgeBlock(edge: number, t: number): number {
  switch (edge) {
    case 0:
      return 8 * t;
    case 1:
      return 8 * t + 7;
    case 2:
      return t;
    default:
      return 56 + t;
  }
}

/** The middle of block `t`'s stretch of edge `edge`, a grid vertex of its patch. */
function stretchMiddle(edge: number, t: number): readonly [number, number] {
  const along = 8 * t + 4;
  switch (edge) {
    case 0:
      return [0, along];
    case 1:
      return [64, along];
    case 2:
      return [along, 0];
    default:
      return [along, 64];
  }
}

/** The edge and the block along it of the place (x, y) of a patch's grid on its boundary. */
function edgeAt(x: number, y: number): readonly [edge: number, t: number] {
  if (x === 0 || x === 64) {
    return [x === 0 ? 0 : 1, Math.min(7, Math.floor(y / 8))];
  }
  return [y === 0 ? 2 : 3, Math.min(7, Math.floor(x / 8))];
}

/**
 * The shared bands of a view's drawn patches' edges (see the module's remarks), refilled each
 * frame, with nothing allocated after warm-up but where a patch meets another across a face's edge.
 */
export class StrideEdges {
  /** Each patch's edges' bands, m: {@link EDGE_BANDS} a patch by its index, at `edgeBandIndex`. */
  readonly bandsM: Float64Array;
  /** The patches, to their index; empty between calls. */
  readonly #drawn = new PatchKeyTable<number>();
  readonly #cell: CellOut = { face: 0, i: 0, j: 0 };
  readonly #beside: ScratchKey = { face: 0, level: 0, i: 0, j: 0 };
  /**
   * Each coarse block along an edge's shared bands at its levels 0 and 1 where a measured patch one
   * level finer is beside it, two a block by (patch, edge, block), and what is beside it, one a
   * block: `NO_FINER`, `MEASURED_FINER`, `UNMEASURED_FINER` (its own vertices never move), or
   * `FAR_FINER` (two levels or more apart, neither side's moving).
   */
  readonly #finer: Float64Array;
  readonly #finerKind: Uint8Array;
  #coarsest = 0;

  /** @param capacity - The most patches a view draws: the cache's slot count. */
  constructor(capacity: number) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError(`stride edges need a whole, positive capacity, not ${String(capacity)}`);
    }
    this.bandsM = new Float64Array(capacity * EDGE_BANDS);
    this.#finer = new Float64Array(capacity * 32 * 2);
    this.#finerKind = new Uint8Array(capacity * 32);
  }

  /**
   * Fills {@link bandsM} for `patches`, the areas a view draws, which do not overlap.
   *
   * @param levelLargestM - Each level's largest e₂, e₄ and e₈ over the view's measured patches, for
   *   an edge beside one that is not; `null` for a level with none.
   * @throws RangeError for more patches than the capacity.
   */
  update(
    patches: ReadonlyArray<StrideEdgePatch>,
    levelLargestM: (level: number) => StrideErrorsM | null,
  ): Float64Array {
    const count = patches.length;
    if (count * EDGE_BANDS > this.bandsM.length) {
      throw new RangeError(`${String(count)} drawn patches exceed the stride edges' capacity`);
    }
    const out = this.bandsM;
    const finer = this.#finer;
    const kinds = this.#finerKind;
    const drawn = this.#drawn;
    const need = (n: number, block: number, level: number): number => {
      const measured = patches[n]?.measured ?? null;
      return measured === null
        ? NO_STRIDE_BAND
        : strideNeedM(measured.scale, blockErrorM(measured.clusterErrors, block, level));
    };
    const fallback = (n: number, level: number): number => {
      const patch = patches[n];
      const measured = patch?.measured ?? null;
      const largest = patch === undefined ? null : levelLargestM(patch.level);
      return measured === null || largest === null
        ? NO_STRIDE_BAND
        : strideNeedM(measured.scale, largest[level] ?? Number.POSITIVE_INFINITY);
    };
    let coarsest = MAX_LEVEL;
    for (let n = 0; n < count; n += 1) {
      const patch = patches[n];
      if (patch !== undefined) {
        drawn.set(patch.key, n);
        coarsest = Math.min(coarsest, patch.key.level);
      }
      for (let e = 0; e < 4; e += 1) {
        for (let t = 0; t < 8; t += 1) {
          for (let l = 0; l < LEVELS; l += 1) {
            out[n * EDGE_BANDS + edgeBandIndex(e, t, l)] = need(n, edgeBlock(e, t), l);
          }
          kinds[32 * n + 8 * e + t] = NO_FINER;
        }
      }
    }
    this.#coarsest = coarsest;
    // From each patch's side, the drawn patch across each edge, of its level or coarser.
    for (let n = 0; n < count; n += 1) {
      const patch = patches[n];
      if (patch !== undefined) {
        EDGE_STEPS.forEach(([di, dj], e) => {
          this.#edge(patches, n, patch, e, di, dj, need, fallback);
        });
      }
    }
    // Each coarse block with a finer patch beside it: the shared bands at its levels 0 and 1, the
    // larger of its own need and its level's largest beside an unmeasured one, none two levels
    // apart, and none at level 2.
    for (let n = 0; n < count; n += 1) {
      for (let e = 0; e < 4; e += 1) {
        for (let t = 0; t < 8; t += 1) {
          const link = 32 * n + 8 * e + t;
          const kind = kinds[link] ?? NO_FINER;
          if (kind === NO_FINER) {
            continue;
          }
          const base = n * EDGE_BANDS;
          for (let l = 0; l < 2; l += 1) {
            out[base + edgeBandIndex(e, t, l)] =
              kind === MEASURED_FINER
                ? (finer[2 * link + l] ?? NO_STRIDE_BAND)
                : kind === UNMEASURED_FINER
                  ? sharedStrideBandM(need(n, edgeBlock(e, t), l), fallback(n, l))
                  : NO_STRIDE_BAND;
          }
          out[base + edgeBandIndex(e, t, 2)] = NO_STRIDE_BAND;
        }
      }
    }
    for (const patch of patches) {
      drawn.delete(patch.key);
    }
    return out;
  }

  /** Patch `n`'s edges' bands from the last {@link StrideEdges.update}, m: a view of {@link bandsM}. */
  edgesOf(n: number): Float64Array {
    return this.bandsM.subarray(n * EDGE_BANDS, (n + 1) * EDGE_BANDS);
  }

  /** Patch `n`'s edge `e`, a step (`di`, `dj`) to the cell beside: see the module's remarks. */
  #edge(
    patches: ReadonlyArray<StrideEdgePatch>,
    n: number,
    patch: StrideEdgePatch,
    e: number,
    di: number,
    dj: number,
    need: (n: number, block: number, level: number) => number,
    fallback: (n: number, level: number) => number,
  ): void {
    const out = this.bandsM;
    const finer = this.#finer;
    const kinds = this.#finerKind;
    const at = this.#besideOf(patch.key, di, dj);
    const other = at < 0 ? undefined : patches[at];
    if (other === undefined || other.level > patch.level) {
      // Undrawn, finer (which has this edge from its own side), or a sub-patch's finer neighbour.
      return;
    }
    const own = patch.measured !== null;
    const theirs = other.measured !== null;
    const step = patch.level - other.level;
    const base = n * EDGE_BANDS;
    // A sub-patch beside a patch of the level it is drawn at, keyed finer than it, which the patch
    // beside does not find from its own side: this side gives it its share.
    const subBeside = !own && theirs && patch.key.level > this.#beside.level;
    for (let t = 0; t < 8; t += 1) {
      const [x, y] = stretchMiddle(e, t);
      const [xo, yo] = coarsePlaceOf(patch.key, x, y, this.#beside);
      const [eo, to] = edgeAt(xo, yo);
      if (step === 0) {
        for (let l = 0; l < LEVELS; l += 1) {
          const mine = need(n, edgeBlock(e, t), l);
          out[base + edgeBandIndex(e, t, l)] = sharedStrideBandM(
            mine,
            theirs ? need(at, edgeBlock(eo, to), l) : fallback(n, l),
          );
          if (subBeside) {
            out[at * EDGE_BANDS + edgeBandIndex(eo, to, l)] = sharedStrideBandM(
              need(at, edgeBlock(eo, to), l),
              fallback(at, l),
            );
          }
        }
        continue;
      }
      const link = 32 * at + 8 * eo + to;
      if (step > 1) {
        // Two levels apart: neither side moves.
        for (let l = 0; l < LEVELS; l += 1) {
          out[base + edgeBandIndex(e, t, l)] = NO_STRIDE_BAND;
        }
        kinds[link] = FAR_FINER;
        continue;
      }
      if (!own) {
        // An unmeasured finer patch beside the coarse block: its own vertices never move.
        kinds[link] = UNMEASURED_FINER;
        continue;
      }
      // The coarse block's stretch holds this block and its sibling along the edge, at the fine
      // levels 1 and 2, the coarse block's 0 and 1.
      const sibling = edgeBlock(e, t ^ 1);
      for (let lc = 0; lc < 2; lc += 1) {
        const fine = sharedStrideBandM(need(n, edgeBlock(e, t), lc + 1), need(n, sibling, lc + 1));
        const shared = sharedStrideBandM(
          fine,
          theirs ? need(at, edgeBlock(eo, to), lc) : fallback(n, lc + 1),
        );
        out[base + edgeBandIndex(e, t, lc + 1)] = shared;
        finer[2 * link + lc] = shared;
      }
      if (theirs && kinds[link] === NO_FINER) {
        kinds[link] = MEASURED_FINER;
      }
    }
  }

  /**
   * The index of the drawn patch covering the cell of `key`'s level a step (`di`, `dj`) away, that
   * cell or a coarser one, its key left in `#beside`; −1 where none of its level or coarser covers
   * it (it is finer, or not drawn) or the step leaves the cube's corner (`CoarseFineHolds`').
   */
  #besideOf(key: PatchKey, di: number, dj: number): number {
    const cell = this.#cell;
    if (!stepCellInto(cell, key, di, dj)) {
      return -1;
    }
    for (let level = key.level; level >= this.#coarsest; level -= 1) {
      const cells = 2 ** (key.level - level);
      const i = Math.floor(cell.i / cells);
      const j = Math.floor(cell.j / cells);
      const n = this.#drawn.getAt(cell.face, level, i, j);
      if (n !== undefined) {
        const beside = this.#beside;
        beside.face = cell.face;
        beside.level = level;
        beside.i = i;
        beside.j = j;
        return n;
      }
    }
    return -1;
  }
}
