/**
 * The morph holds at the drawn patches' coarse–fine edges and corners (plan R10, R10.T8.c;
 * decision-r10-t16-leans §9): where drawn patches of two levels meet, the coarser one's vertices
 * there are held at morph factor k = 0 and the finer one's at k = 1, so that both lie on the
 * coarser level's surface in every view, whatever its own morph bands give there.
 *
 * @remarks
 * CDLOD's seam needs the fine side of a coarse–fine edge at k = 1 (its odd vertices on its
 * parent's mesh, its even ones at its parent's heights) and the coarse side at k = 0 (its own
 * level's mesh). Each view's bands come from its own optics and τ alone (`morphRangeM`), never from
 * what is drawn, and selection's tolerance and its 8-neighbour balance, not the bands, choose the
 * levels. So a view that drove no split sees a coarse leaf beyond its band start beside a finer
 * one (an instrument a quarter as wide, or four times as high: k = 1 on the coarse side at every
 * such edge), the balance's rings of splits carry coarse–fine edges past the coarse side's band
 * start in a single view too (k up to 0.92 at 1,000 km over the pole), and a stand-in, an ancestor
 * drawn for missing patches, borders finer resident patches short of their band end. The holds are
 * made over the draw set as resolved, stand-ins included, once a selection, and are the same for
 * every view: a vertex of a patch that touches a finer drawn patch is held at 0, one that touches
 * a coarser drawn patch at 1, and one touching both (two levels between its neighbours, only beside
 * a stand-in) at 1, since the fine side's step is the one seen. A neighbour two or more levels
 * coarser or finer is held as one a level apart: the fine side at k = 1 reaches only its parent's
 * level, so the step is not closed there, only smaller (a stand-in's, under `TERRAIN: STREAMING`).
 *
 * The holds are whole edge rows and single corners, never a ramp: every patch that shares a vertex
 * with a patch of another level has that vertex held, and patches of one level that share a vertex
 * hold it alike, so their seam stays shut; a ramp would part them at the perpendicular edges'
 * corners. What is left is a fold one row wide, of at most about τ pixels where the bands would
 * have morphed the row anyway. A corner bit is needed beside the edge bits because the balance lets
 * a finer patch touch a coarser one at a corner alone, and the morph moves even vertices too.
 *
 * Each drawn patch's holds are 16 bits: bits 0 to 3 hold the edges x = 0, x = 64, y = 0 and y = 64
 * of its 65 × 65 grid (`EDGES`' order: UMin, UMax, VMin, VMax) at k = 0, bits 4 to 7 its corners
 * (0, 0), (64, 0), (64, 64) and (0, 64) (`cornerNeighbours`' order) at k = 0, and bits 8 to 15 the
 * same edges and corners at k = 1. The instance record carries them above its quadrant
 * (`gpu/uniforms.ts`' `InstanceRecords`), and `shaders/terrainPatch.wgsl`'s `coarseFineMorph`
 * applies them, whose twin is {@link coarseFineMorph}.
 */

import {
  type CellOut,
  type CubePoint,
  type Face,
  faceCoords,
  facePoint,
  MAX_LEVEL,
  type PatchKey,
  stepCellInto,
} from "./patchKey";
import { PatchKeyTable } from "./patchKeyTable";

/**
 * The holds' bit of each of a patch's edges at k = 0, in `EDGES`' order: x = 0, x = 64, y = 0,
 * y = 64.
 */
export const EDGE_HOLD_BITS: readonly [number, number, number, number] = [1, 2, 4, 8];

/**
 * The holds' bit of each of a patch's corners at k = 0, in `cornerNeighbours`' order: (0, 0),
 * (64, 0), (64, 64), (0, 64).
 */
export const CORNER_HOLD_BITS: readonly [number, number, number, number] = [16, 32, 64, 128];

/** The shift from an edge's or corner's hold at k = 0 to its hold at k = 1. */
export const TO_PARENT_SHIFT = 8;

/** The largest holds value: 16 bits. */
export const MAX_MORPH_HOLDS = 0xffff;

/** The corners' bits of a patch's holds at k = 0. */
const CORNER_HOLDS = 0xf0;

/** The last vertex of a patch's row. */
const LAST_VERTEX = 64;

/** An edge's or corner's step to the cell beside it, its hold bit at k = 0, and a vertex on it. */
interface Probe {
  readonly di: number;
  readonly dj: number;
  readonly bit: number;
  /** A vertex on it: an edge's middle, which lies inside a coarser neighbour's edge, or the corner. */
  readonly x: number;
  readonly y: number;
}

/** The edges' and then the corners' probes. */
const PROBES: ReadonlyArray<Probe> = [
  { di: -1, dj: 0, bit: EDGE_HOLD_BITS[0], x: 0, y: 32 },
  { di: 1, dj: 0, bit: EDGE_HOLD_BITS[1], x: 64, y: 32 },
  { di: 0, dj: -1, bit: EDGE_HOLD_BITS[2], x: 32, y: 0 },
  { di: 0, dj: 1, bit: EDGE_HOLD_BITS[3], x: 32, y: 64 },
  { di: -1, dj: -1, bit: CORNER_HOLD_BITS[0], x: 0, y: 0 },
  { di: 1, dj: -1, bit: CORNER_HOLD_BITS[1], x: 64, y: 0 },
  { di: 1, dj: 1, bit: CORNER_HOLD_BITS[2], x: 64, y: 64 },
  { di: -1, dj: 1, bit: CORNER_HOLD_BITS[3], x: 0, y: 64 },
];

/**
 * The hold bits at k = 0 of the place (`x`, `y`) of a patch's grid, each 0 to 64 and not
 * necessarily whole: the bit of each edge it lies on, and its corner's where it is one. The
 * shader's `holdBitsAt`.
 */
export function vertexHoldBits(x: number, y: number): number {
  const edges =
    (x === 0 ? EDGE_HOLD_BITS[0] : 0) |
    (x === LAST_VERTEX ? EDGE_HOLD_BITS[1] : 0) |
    (y === 0 ? EDGE_HOLD_BITS[2] : 0) |
    (y === LAST_VERTEX ? EDGE_HOLD_BITS[3] : 0);
  switch (edges) {
    case EDGE_HOLD_BITS[0] | EDGE_HOLD_BITS[2]:
      return edges | CORNER_HOLD_BITS[0];
    case EDGE_HOLD_BITS[1] | EDGE_HOLD_BITS[2]:
      return edges | CORNER_HOLD_BITS[1];
    case EDGE_HOLD_BITS[1] | EDGE_HOLD_BITS[3]:
      return edges | CORNER_HOLD_BITS[2];
    case EDGE_HOLD_BITS[0] | EDGE_HOLD_BITS[3]:
      return edges | CORNER_HOLD_BITS[3];
    default:
      return edges;
  }
}

/**
 * The morph factor at vertex (`x`, `y`) of a patch with holds `holds`, from CDLOD's `k`: 1 where a
 * hold at k = 1 covers the vertex, else 0 where a hold at k = 0 does, else `k`. The shader's
 * `coarseFineMorph`, which a grounded contact's hold then caps (`morphFactor`).
 */
export function coarseFineMorph(holds: number, x: number, y: number, k: number): number {
  const at = vertexHoldBits(x, y);
  if (((holds >>> TO_PARENT_SHIFT) & at) !== 0) {
    return 1;
  }
  return (holds & at) !== 0 ? 0 : k;
}

/** A patch key whose fields are rewritten in place. */
interface ScratchKey {
  face: Face;
  level: number;
  i: number;
  j: number;
}

/**
 * The bits at k = 0 of `coarse`'s grid at vertex (`x`, `y`) of `fine`, a finer patch whose place
 * there lies on `coarse`'s boundary: the corner's bit alone where the place is a corner of
 * `coarse`, else the bit of the edge it lies on.
 */
function coarseBitsAt(fine: PatchKey, x: number, y: number, coarse: PatchKey): number {
  let xc: number;
  let yc: number;
  if (fine.face === coarse.face) {
    // One face's axes: the place in `coarse`'s quads, exact in f64.
    const cells = 2 ** (fine.level - coarse.level);
    xc = (64 * fine.i + x) / cells - 64 * coarse.i;
    yc = (64 * fine.j + y) / cells - 64 * coarse.j;
  } else {
    // Across a face's edge, by `sampleDir`'s integer lattice: `fine`'s vertices on the cube of
    // half-width 64 · 2^level, scaled to `coarse`'s, which halves it a level; exact in f64.
    const quads = 64 * 2 ** fine.level;
    const p = facePoint(
      fine.face,
      2 * (64 * fine.i + x) - quads,
      2 * (64 * fine.j + y) - quads,
      quads,
    );
    const scale = 2 ** (coarse.level - fine.level);
    const scaled: CubePoint = [p[0] * scale, p[1] * scale, p[2] * scale];
    const [u, v] = faceCoords(coarse.face, scaled);
    const coarseQuads = 64 * 2 ** coarse.level;
    xc = (u + coarseQuads) / 2 - 64 * coarse.i;
    yc = (v + coarseQuads) / 2 - 64 * coarse.j;
  }
  const at = vertexHoldBits(xc, yc);
  return (at & CORNER_HOLDS) !== 0 ? at & CORNER_HOLDS : at;
}

/**
 * The morph holds of a draw set's patches, made once and refilled each selection, with nothing
 * allocated after warm-up but where a patch meets a coarser one across a face's edge.
 *
 * @remarks
 * Each patch looks up the cell beside each of its edges and corners, and, where that is not drawn,
 * its ancestors down to the coarsest level drawn: some 8 to 16 lookups a patch, and up to 8 more a
 * level between it and the coarsest drawn patch where its neighbour is finer or undrawn: 3.2 ms
 * for 2,560 patches, every other level-6 patch of a 32 × 32 block split, under vitest at a load of
 * 21 on the shared machine (provisional), once a selection.
 */
export class CoarseFineHolds {
  /** The holds of the patches last given, by their index there. */
  readonly holds: Uint16Array;
  /** The patches being held, to their index; empty between calls. */
  readonly #drawn = new PatchKeyTable<number>();
  readonly #cell: CellOut = { face: 0, i: 0, j: 0 };
  /** The coarser patch `#coarserBeside` last found. */
  readonly #coarse: ScratchKey = { face: 0, level: 0, i: 0, j: 0 };
  /** The coarsest level of the patches being held. */
  #coarsest = 0;

  /** @param capacity - The most patches a draw set holds: the cache's slot count. */
  constructor(capacity: number) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError(`morph holds need a whole, positive capacity, not ${capacity}`);
    }
    this.holds = new Uint16Array(capacity);
  }

  /**
   * Fills {@link holds}' first `keys.length` entries for the drawn patches `keys`, which do not
   * overlap: each one's at k = 0 where a finer one of `keys` touches it, at k = 1 where a coarser
   * one does.
   *
   * @returns {@link holds}.
   * @throws RangeError for more patches than the capacity.
   */
  update(keys: ReadonlyArray<PatchKey>): Uint16Array {
    const { holds } = this;
    if (keys.length > holds.length) {
      throw new RangeError(`${keys.length} drawn patches exceed ${holds.length} holds`);
    }
    const drawn = this.#drawn;
    let coarsest = MAX_LEVEL;
    for (let n = 0; n < keys.length; n += 1) {
      const key = keys[n];
      if (key !== undefined) {
        drawn.set(key, n);
        coarsest = Math.min(coarsest, key.level);
      }
      holds[n] = 0;
    }
    this.#coarsest = coarsest;
    // From each patch's side, the coarser drawn patch across each edge and corner, if any: the
    // patch's own vertices there are held at 1, and that coarser patch's at 0.
    const coarse = this.#coarse;
    for (let n = 0; n < keys.length; n += 1) {
      const key = keys[n];
      if (key === undefined) {
        continue;
      }
      for (const probe of PROBES) {
        const at = this.#coarserBeside(key, probe);
        if (at >= 0) {
          holds[n] = (holds[n] ?? 0) | (probe.bit << TO_PARENT_SHIFT);
          holds[at] = (holds[at] ?? 0) | coarseBitsAt(key, probe.x, probe.y, coarse);
        }
      }
    }
    for (const key of keys) {
      drawn.delete(key);
    }
    return holds;
  }

  /**
   * The index of the drawn patch coarser than `key` covering the cell of `key`'s level a step
   * `probe` away, its key left in `#coarse`; −1 where that cell is drawn itself, lies past a cube
   * corner, or no drawn ancestor covers it (it is finer, or not drawn).
   */
  #coarserBeside(key: PatchKey, probe: Probe): number {
    const cell = this.#cell;
    const drawn = this.#drawn;
    if (
      !stepCellInto(cell, key, probe.di, probe.dj) ||
      drawn.getAt(cell.face, key.level, cell.i, cell.j) !== undefined
    ) {
      return -1;
    }
    for (let level = key.level - 1; level >= this.#coarsest; level -= 1) {
      const cells = 2 ** (key.level - level);
      const i = Math.floor(cell.i / cells);
      const j = Math.floor(cell.j / cells);
      const n = drawn.getAt(cell.face, level, i, j);
      if (n !== undefined) {
        const coarse = this.#coarse;
        coarse.face = cell.face;
        coarse.level = level;
        coarse.i = i;
        coarse.j = j;
        return n;
      }
    }
    return -1;
  }
}

/**
 * The morph holds of the drawn patches `keys`, once, through a {@link CoarseFineHolds} made for the
 * call; a terrain pass keeps one a device instead.
 */
export function coarseFineHolds(keys: ReadonlyArray<PatchKey>): Uint16Array {
  return new CoarseFineHolds(Math.max(1, keys.length)).update(keys).slice(0, keys.length);
}
