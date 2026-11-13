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
 * its own by the place of the block's stretch's middle in the neighbour's grid (`coarsePlaceInto`):
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
 *
 * **Its cost** (research-next-work-40): an update forms each patch's own needs along its edges once,
 * where both sides of every edge read them, and a frame whose inputs are the last's, field by field
 * (each patch's key, level and scale, its errors by identity, and its level's largest errors), takes
 * the last update's bands back as it left them, the same to the bit: the drawn set and the view's
 * slewed τ′ hold still for most frames (74% of low's on the seamless probe).
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
import { coarsePlaceInto, type PlaceOut } from "./coarseFineHolds";
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

/**
 * The numbers of a patch's inputs `StrideEdges` compares from one update to the next: its key's
 * four, its level, its scale's two and its level's largest three errors.
 */
const INPUT_WORDS = 10;

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

/** The middle of block `t`'s stretch of edge `edge`, a grid vertex of its patch: its x. */
function stretchMiddleX(edge: number, t: number): number {
  return edge === 0 ? 0 : edge === 1 ? 64 : 8 * t + 4;
}

/** The middle of block `t`'s stretch of edge `edge`: its y. */
function stretchMiddleY(edge: number, t: number): number {
  return edge === 2 ? 0 : edge === 3 ? 64 : 8 * t + 4;
}

/** The edge the place (x, y) of a patch's grid on its boundary lies on. */
function edgeOfPlace(x: number, y: number): number {
  return x === 0 ? 0 : x === 64 ? 1 : y === 0 ? 2 : 3;
}

/** The block along its edge (`edgeOfPlace`) of the place (x, y) on a patch's boundary. */
function blockOfPlace(x: number, y: number): number {
  return Math.min(7, Math.floor((x === 0 || x === 64 ? y : x) / 8));
}

/** The shared bands of a view's drawn patches' edges (see the module's remarks), refilled each frame. */
export class StrideEdges {
  /** Each patch's edges' bands, m: {@link EDGE_BANDS} a patch by its index, at `edgeBandIndex`. */
  readonly bandsM: Float64Array;
  /** The patches, to their index; empty between calls. */
  readonly #drawn = new PatchKeyTable<number>();
  readonly #cell: CellOut = { face: 0, i: 0, j: 0 };
  readonly #beside: ScratchKey = { face: 0, level: 0, i: 0, j: 0 };
  readonly #place: PlaceOut = { x: 0, y: 0 };
  /**
   * Each coarse block along an edge's shared bands at its levels 0 and 1 where a measured patch one
   * level finer is beside it, two a block by (patch, edge, block), and what is beside it, one a
   * block: `NO_FINER`, `MEASURED_FINER`, `UNMEASURED_FINER` (its own vertices never move), or
   * `FAR_FINER` (two levels or more apart, neither side's moving).
   */
  readonly #finer: Float64Array;
  readonly #finerKind: Uint8Array;
  /**
   * Each patch's own needs along its edges, at `edgeBandIndex` as {@link bandsM}, and its level's
   * largest errors' needs beside a patch that is not measured, three a patch: each formed once an
   * update, where the edges read them often.
   */
  readonly #needs: Float64Array;
  readonly #fallbacks: Float64Array;
  #coarsest = 0;
  /**
   * The last update's inputs and its {@link bandsM} as it left them, before any caller's writes:
   * a frame of the same inputs takes them back, the same bands to the bit, as the drawn set and
   * the view's τ′ hold still for many frames at a time. Each patch's key and level, its scale's two
   * parts and its level's largest errors, `NaN` for none ({@link StrideEdges.update}), seven
   * numbers and its errors a patch.
   */
  readonly #lastInputs: Float64Array;
  readonly #lastErrors: Array<ArrayLike<number> | null>;
  readonly #lastBands: Float64Array;
  #lastCount = -1;
  #reused = false;

  /** @param capacity - The most patches a view draws: the cache's slot count. */
  constructor(capacity: number) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError(`stride edges need a whole, positive capacity, not ${String(capacity)}`);
    }
    this.bandsM = new Float64Array(capacity * EDGE_BANDS);
    this.#finer = new Float64Array(capacity * 32 * 2);
    this.#finerKind = new Uint8Array(capacity * 32);
    this.#needs = new Float64Array(capacity * EDGE_BANDS);
    this.#fallbacks = new Float64Array(capacity * LEVELS);
    this.#lastInputs = new Float64Array(capacity * INPUT_WORDS);
    this.#lastErrors = Array.from({ length: capacity }, () => null);
    this.#lastBands = new Float64Array(capacity * EDGE_BANDS);
  }

  /** Whether the last {@link StrideEdges.update} took its bands back from the one before. */
  get reused(): boolean {
    return this.#reused;
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
    // The same inputs as the last update's: its bands again, as it left them.
    this.#reused = this.#sameInputs(patches, levelLargestM);
    if (this.#reused) {
      out.set(this.#lastBands.subarray(0, count * EDGE_BANDS));
      return out;
    }
    const finer = this.#finer;
    const kinds = this.#finerKind;
    const drawn = this.#drawn;
    const needs = this.#needs;
    const fallbacks = this.#fallbacks;
    let coarsest = MAX_LEVEL;
    for (let n = 0; n < count; n += 1) {
      const patch = patches[n];
      const measured = patch?.measured ?? null;
      if (patch !== undefined) {
        drawn.set(patch.key, n);
        coarsest = Math.min(coarsest, patch.key.level);
      }
      for (let e = 0; e < 4; e += 1) {
        for (let t = 0; t < 8; t += 1) {
          const block = edgeBlock(e, t);
          for (let l = 0; l < LEVELS; l += 1) {
            const at = n * EDGE_BANDS + edgeBandIndex(e, t, l);
            const own =
              measured === null
                ? NO_STRIDE_BAND
                : strideNeedM(measured.scale, blockErrorM(measured.clusterErrors, block, l));
            needs[at] = own;
            out[at] = own;
          }
          kinds[32 * n + 8 * e + t] = NO_FINER;
        }
      }
      const largest = patch === undefined ? null : levelLargestM(patch.level);
      for (let l = 0; l < LEVELS; l += 1) {
        fallbacks[LEVELS * n + l] =
          measured === null || largest === null
            ? NO_STRIDE_BAND
            : strideNeedM(measured.scale, largest[l] ?? Number.POSITIVE_INFINITY);
      }
    }
    this.#coarsest = coarsest;
    // From each patch's side, the drawn patch across each edge, of its level or coarser.
    for (let n = 0; n < count; n += 1) {
      const patch = patches[n];
      if (patch !== undefined) {
        for (let e = 0; e < 4; e += 1) {
          const [di, dj] = EDGE_STEPS[e] ?? [0, 0];
          this.#edge(patches, n, patch, e, di, dj);
        }
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
                  ? sharedStrideBandM(
                      needs[base + edgeBandIndex(e, t, l)] ?? NO_STRIDE_BAND,
                      fallbacks[LEVELS * n + l] ?? NO_STRIDE_BAND,
                    )
                  : NO_STRIDE_BAND;
          }
          out[base + edgeBandIndex(e, t, 2)] = NO_STRIDE_BAND;
        }
      }
    }
    for (const patch of patches) {
      drawn.delete(patch.key);
    }
    this.#lastBands.set(out.subarray(0, count * EDGE_BANDS));
    return out;
  }

  /**
   * Whether `patches` and their levels' largest errors are the last update's, field by field and
   * their errors by identity, and records them as the last where they are not.
   */
  #sameInputs(
    patches: ReadonlyArray<StrideEdgePatch>,
    levelLargestM: (level: number) => StrideErrorsM | null,
  ): boolean {
    const inputs = this.#lastInputs;
    const errors = this.#lastErrors;
    let same = patches.length === this.#lastCount;
    const note = (at: number, word: number): void => {
      same &&= Object.is(inputs[at], word);
      inputs[at] = word;
    };
    for (let n = 0; n < patches.length; n += 1) {
      const patch = patches[n];
      const measured = patch?.measured ?? null;
      const largest = patch === undefined ? null : levelLargestM(patch.level);
      const at = INPUT_WORDS * n;
      note(at, patch?.key.face ?? Number.NaN);
      note(at + 1, patch?.key.level ?? Number.NaN);
      note(at + 2, patch?.key.i ?? Number.NaN);
      note(at + 3, patch?.key.j ?? Number.NaN);
      note(at + 4, patch?.level ?? Number.NaN);
      note(at + 5, measured?.scale.statedM ?? Number.NaN);
      note(at + 6, measured?.scale.perErrorM ?? Number.NaN);
      note(at + 7, largest?.[0] ?? Number.NaN);
      note(at + 8, largest?.[1] ?? Number.NaN);
      note(at + 9, largest?.[2] ?? Number.NaN);
      const own = measured?.clusterErrors ?? null;
      same &&= errors[n] === own;
      errors[n] = own;
    }
    this.#lastCount = patches.length;
    return same;
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
  ): void {
    const out = this.bandsM;
    const finer = this.#finer;
    const kinds = this.#finerKind;
    const needs = this.#needs;
    const fallbacks = this.#fallbacks;
    // Patch `m`'s own need at block `t` of edge `edge` at level `level`, and its fallback's there.
    const need = (m: number, edge: number, t: number, level: number): number =>
      needs[m * EDGE_BANDS + edgeBandIndex(edge, t, level)] ?? NO_STRIDE_BAND;
    const fallback = (m: number, level: number): number =>
      fallbacks[LEVELS * m + level] ?? NO_STRIDE_BAND;
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
      coarsePlaceInto(
        this.#place,
        patch.key,
        stretchMiddleX(e, t),
        stretchMiddleY(e, t),
        this.#beside,
      );
      const eo = edgeOfPlace(this.#place.x, this.#place.y);
      const to = blockOfPlace(this.#place.x, this.#place.y);
      if (step === 0) {
        for (let l = 0; l < LEVELS; l += 1) {
          const mine = need(n, e, t, l);
          out[base + edgeBandIndex(e, t, l)] = sharedStrideBandM(
            mine,
            theirs ? need(at, eo, to, l) : fallback(n, l),
          );
          if (subBeside) {
            out[at * EDGE_BANDS + edgeBandIndex(eo, to, l)] = sharedStrideBandM(
              need(at, eo, to, l),
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
      for (let lc = 0; lc < 2; lc += 1) {
        const fine = sharedStrideBandM(need(n, e, t, lc + 1), need(n, e, t ^ 1, lc + 1));
        const shared = sharedStrideBandM(fine, theirs ? need(at, eo, to, lc) : fallback(n, lc + 1));
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
