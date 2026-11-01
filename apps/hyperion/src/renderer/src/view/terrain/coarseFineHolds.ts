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
 * A sub-patch, the part of a resident ancestor drawn over a missing selected patch's area alone
 * (`cache.ts`' `DrawnPatch.within`), is held as its ancestor's level over that area: its edges and
 * corners are the missing patch's, and its surface the ancestor's, so a neighbour of the missing
 * patch's own level is the finer side there, held at k = 1, and the sub-patch's edge at k = 0.
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

/**
 * The steps of a fade (plan R10, R10.T16.i; the seamless lane's time morph): a fade is a byte, 0 to
 * {@link FADE_STEPS}, for 0 to 1.
 */
export const FADE_STEPS = 255;

/**
 * A fade of 0 to 1 as its byte ({@link FADE_STEPS}): rounded to the nearest step, so that every
 * instance given the same fade at the same time writes the same byte.
 */
export function fadeByte(fade: number): number {
  return Math.round(Math.min(1, Math.max(0, fade)) * FADE_STEPS);
}

/**
 * The fade floor at vertex (`x`, `y`) of an instance (plan R10, R10.T16.i): the largest of its own
 * fade and the fades of the edges and the corner the vertex lies on, from the bytes of
 * `edgeFades` (`EDGES`' order, byte n edge n), `cornerFades` (`cornerNeighbours`' order) and
 * `ownFade`'s low byte, each 0 to 1. The shader's `fadeFloor`.
 */
export function fadeFloor(
  edgeFades: number,
  cornerFades: number,
  ownFade: number,
  x: number,
  y: number,
): number {
  const at = vertexHoldBits(x, y);
  let floor = ownFade & 0xff;
  for (let n = 0; n < 4; n += 1) {
    if ((at & (1 << n)) !== 0) {
      floor = Math.max(floor, (edgeFades >>> (8 * n)) & 0xff);
    }
    if ((at & (16 << n)) !== 0) {
      floor = Math.max(floor, (cornerFades >>> (8 * n)) & 0xff);
    }
  }
  return floor / FADE_STEPS;
}

/**
 * The morph factor at vertex (`x`, `y`) of an instance with holds `holds` and fades (plan R10,
 * R10.T16.i): {@link coarseFineMorph}'s, raised to its {@link fadeFloor} but where a hold at k = 0
 * alone covers the vertex, so that a coarse side touching a finer patch stays on its own mesh
 * whatever fades (R05 Design note 6). The shader's `morphFactor` before the grounded contacts'
 * hold; with no fade, {@link coarseFineMorph}'s, bit for bit.
 */
export function fadedMorph(
  holds: number,
  edgeFades: number,
  cornerFades: number,
  ownFade: number,
  x: number,
  y: number,
  k: number,
): number {
  const held = coarseFineMorph(holds, x, y, k);
  const at = vertexHoldBits(x, y);
  const toOwnAlone = (holds & at) !== 0 && ((holds >>> TO_PARENT_SHIFT) & at) === 0;
  return toOwnAlone ? held : Math.max(held, fadeFloor(edgeFades, cornerFades, ownFade, x, y));
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
   * Fills {@link holds}' first `keys.length` entries for the drawn patches `keys`, the areas they
   * draw, which do not overlap: each one's at k = 0 where a finer one of `keys` touches it, at
   * k = 1 where a coarser one does, by `levels`.
   *
   * @param levels - The level each of `keys` is drawn at, in the same order: a sub-patch's its
   *   ancestor's, coarser than its key's; absent, or where shorter, each key's own.
   * @returns {@link holds}.
   * @throws RangeError for more patches than the capacity.
   */
  update(keys: ReadonlyArray<PatchKey>, levels?: ArrayLike<number>): Uint16Array {
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
    // From each patch's side, the drawn patch across each edge and corner of its own area's level
    // or a coarser one, if any: where it is drawn coarser, the patch's own vertices there are held
    // at 1 and its at 0; where finer (a sub-patch's area beside a patch drawn at it), the patch's at
    // 0 and its at 1.
    const coarse = this.#coarse;
    for (let n = 0; n < keys.length; n += 1) {
      const key = keys[n];
      if (key === undefined) {
        continue;
      }
      const own = levels?.[n] ?? key.level;
      for (const probe of PROBES) {
        const at = this.#beside(key, probe);
        if (at < 0) {
          continue;
        }
        const other = levels?.[at] ?? coarse.level;
        if (other < own) {
          holds[n] = (holds[n] ?? 0) | (probe.bit << TO_PARENT_SHIFT);
          holds[at] = (holds[at] ?? 0) | coarseBitsAt(key, probe.x, probe.y, coarse);
        } else if (other > own) {
          holds[n] = (holds[n] ?? 0) | probe.bit;
          holds[at] =
            (holds[at] ?? 0) | (coarseBitsAt(key, probe.x, probe.y, coarse) << TO_PARENT_SHIFT);
        }
      }
    }
    for (const key of keys) {
      drawn.delete(key);
    }
    return holds;
  }

  /**
   * The index of the drawn patch covering the cell of `key`'s level a step `probe` away, that cell
   * or a coarser one, its key left in `#coarse`; −1 where the cell lies past a cube corner or no
   * drawn patch of its level or coarser covers it (it is finer, or not drawn).
   */
  #beside(key: PatchKey, probe: Probe): number {
    const cell = this.#cell;
    const drawn = this.#drawn;
    if (!stepCellInto(cell, key, probe.di, probe.dj)) {
      return -1;
    }
    for (let level = key.level; level >= this.#coarsest; level -= 1) {
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
 * A transition of the drawn ground in progress over an area at a frame (plan R10, R10.T16.i): the
 * area, its footprint, and its fade there, 0 to 1.
 */
export interface FadeFootprint {
  readonly area: PatchKey;
  readonly fade: number;
}

/**
 * The fades of a draw set's instances from the transitions in progress (plan R10, R10.T16.i), made
 * once a frame, with nothing allocated after warm-up.
 *
 * @remarks
 * An instance whose area lies in a footprint takes its fade as its own; one outside it, of the
 * footprint's level or finer, takes it on each edge and corner whose cell beside, of the instance's
 * level, lies in the footprint. So every vertex on a footprint's boundary takes its fade from every
 * side of that level or finer, and two instances that share a vertex compute one morph factor
 * there (R05 Design note 6's crack rule). An instance coarser than a footprint touching it is the
 * coarse side of a coarse–fine edge there, held at k = 0, which a fade does not raise
 * ({@link fadedMorph}). The bytes are {@link fadeByte}'s, laid out as the instance record carries
 * them: {@link TransitionFades.edges} byte n for edge n in `EDGES`' order,
 * {@link TransitionFades.corners} byte n for corner n in `cornerNeighbours`' order, and
 * {@link TransitionFades.own}.
 */
export class TransitionFades {
  readonly edges: Uint32Array;
  readonly corners: Uint32Array;
  readonly own: Uint8Array;
  /** The footprints being applied, to their fades' bytes; empty between calls. */
  readonly #footprints = new PatchKeyTable<number>();
  readonly #cell: CellOut = { face: 0, i: 0, j: 0 };
  /** The footprints' levels, each once, coarsest first. */
  readonly #levels: number[] = [];

  /** @param capacity - The most instances a draw set holds: the cache's slot count. */
  constructor(capacity: number) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError(`transition fades need a whole, positive capacity, not ${capacity}`);
    }
    this.edges = new Uint32Array(capacity);
    this.corners = new Uint32Array(capacity);
    this.own = new Uint8Array(capacity);
  }

  /**
   * Fills the first `areas.length` entries for the drawn instances' areas `areas`, which do not
   * overlap, from `footprints`, which do not overlap either.
   *
   * @throws RangeError for more instances than the capacity.
   */
  update(areas: ReadonlyArray<PatchKey>, footprints: ReadonlyArray<FadeFootprint>): void {
    if (areas.length > this.own.length) {
      throw new RangeError(`${areas.length} instances exceed ${this.own.length} fades`);
    }
    const table = this.#footprints;
    const levels = this.#levels;
    levels.length = 0;
    for (const { area, fade } of footprints) {
      const byte = fadeByte(fade);
      if (byte > 0) {
        table.set(area, Math.max(byte, table.get(area) ?? 0));
        if (!levels.includes(area.level)) {
          levels.push(area.level);
        }
      }
    }
    levels.sort((a, b) => a - b);
    const cell = this.#cell;
    for (let n = 0; n < areas.length; n += 1) {
      const area = areas[n];
      let own = 0;
      let edges = 0;
      let corners = 0;
      if (area !== undefined && levels.length > 0) {
        own = this.#fadeOver(area.face, area.level, area.i, area.j);
        for (let p = 0; p < PROBES.length; p += 1) {
          const probe = PROBES[p];
          if (probe === undefined || !stepCellInto(cell, area, probe.di, probe.dj)) {
            continue;
          }
          const byte = this.#fadeOver(cell.face, area.level, cell.i, cell.j);
          if (p < 4) {
            edges |= byte << (8 * p);
          } else {
            corners |= byte << (8 * (p - 4));
          }
        }
      }
      this.own[n] = own;
      this.edges[n] = edges >>> 0;
      this.corners[n] = corners >>> 0;
    }
    for (const { area } of footprints) {
      table.delete(area);
    }
  }

  /** The largest fade's byte of the footprints holding cell (`i`, `j`) of `level` on `face`. */
  #fadeOver(face: Face, level: number, i: number, j: number): number {
    let byte = 0;
    for (const footprint of this.#levels) {
      if (footprint > level) {
        break;
      }
      const cells = 2 ** (level - footprint);
      const found = this.#footprints.getAt(
        face,
        footprint,
        Math.floor(i / cells),
        Math.floor(j / cells),
      );
      byte = Math.max(byte, found ?? 0);
    }
    return byte;
  }
}

/**
 * The morph holds of the drawn patches `keys`, once, through a {@link CoarseFineHolds} made for the
 * call; a terrain pass keeps one a device instead.
 */
export function coarseFineHolds(keys: ReadonlyArray<PatchKey>): Uint16Array {
  return new CoarseFineHolds(Math.max(1, keys.length)).update(keys).slice(0, keys.length);
}
