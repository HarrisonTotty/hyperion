/**
 * The coarse field's cells by their indices (plan R10, T7): R09's numbering of a level's 6 · 4ᴸ
 * cells, face-major, then the Morton order of (i, j) within the face, and its inverse, as plain
 * arithmetic with no engine, so that the height workers decide by it too (`ocean/oceanCells.ts`;
 * perf-long-tasks-5). `coverage.ts` re-exports it, as it held it before.
 */
import type { Face, PatchKey } from "./patchKey";

/** The shallowest coarse-field level R09 serves (`SurfaceHeaderDto.level`). */
export const MIN_FIELD_LEVEL = 5;

/** The deepest coarse-field level R09 serves, an Earth's 40 km cells (`SurfaceHeaderDto.level`). */
export const MAX_FIELD_LEVEL = 8;

/** The cells of a coarse-field level: 6 · 4ᴸ (`hyperion_surface::field::cell_count`). */
export function fieldCellCount(level: number): number {
  return 6 * 4 ** level;
}

/** Whether `level` is one R09 serves a field at. */
export function isFieldLevel(level: number): boolean {
  return Number.isInteger(level) && level >= MIN_FIELD_LEVEL && level <= MAX_FIELD_LEVEL;
}

/** The low 16 bits of `x` spread to the even bits, as R09's `spread`. */
function spread(x: number): number {
  let s = x & 0x0000_ffff;
  s = (s | (s << 8)) & 0x00ff_00ff;
  s = (s | (s << 4)) & 0x0f0f_0f0f;
  s = (s | (s << 2)) & 0x3333_3333;
  return (s | (s << 1)) & 0x5555_5555;
}

/** The even bits of `x` gathered into the low 16, the inverse of {@link spread}, as R09's `compact`. */
export function compact(x: number): number {
  let c = x & 0x5555_5555;
  c = (c | (c >>> 1)) & 0x3333_3333;
  c = (c | (c >>> 2)) & 0x0f0f_0f0f;
  c = (c | (c >>> 4)) & 0x00ff_00ff;
  return (c | (c >>> 8)) & 0x0000_ffff;
}

/**
 * A coarse cell's index among its level's cells, the client's twin of R09's `cell_index`: the
 * face × 4ᴸ plus the cell's Morton code, whose bit 2k is bit k of i and bit 2k + 1 bit k of j.
 *
 * @throws Error for a cell outside the levels R09 serves.
 */
export function fieldCellIndex(cell: PatchKey): number {
  if (!isFieldLevel(cell.level)) {
    throw new Error(`a coarse cell is at level 5 to 8, not ${cell.level}`);
  }
  return cell.face * 4 ** cell.level + (spread(cell.i) | (spread(cell.j) << 1));
}

/** The coarse cell of `level` whose {@link fieldCellIndex} is `index`, R09's `cell_at_index`. */
export function fieldCellAt(level: number, index: number): PatchKey {
  const perFace = 4 ** level;
  const face = Math.floor(index / perFace);
  const morton = index - face * perFace;
  return { face: faceOfIndex(face), level, i: compact(morton), j: compact(morton >>> 1) };
}

/**
 * The coarse cell of `level` whose {@link fieldCellIndex} is `index`, as its place on the faces'
 * grid, (face · 2ᴸ + j) · 2ᴸ + i: {@link fieldCellAt}'s cell with no object made, for the loops over
 * a map's cells (lane perf-main-thread).
 *
 * @throws Error for an index past the level's six faces, as {@link fieldCellAt}.
 */
export function fieldCellGridIndex(level: number, index: number): number {
  const perFace = 4 ** level;
  const face = faceOfIndex(Math.floor(index / perFace));
  const morton = index - face * perFace;
  const n = 2 ** level;
  return (face * n + compact(morton >>> 1)) * n + compact(morton);
}

/** {@link mortonCells}' table, made at its first use. */
let MORTON_CELLS: Uint16Array | null = null;

/**
 * The cell of each Morton code below 2¹⁶, a face's codes at the levels R09 serves, as j · 256 + i:
 * {@link fieldCellAt}'s (i, j) by a look-up, for the loops over a field's cells (lane
 * perf-main-thread). Made once, 128 KiB, and read-only.
 */
export function mortonCells(): ArrayLike<number> {
  if (MORTON_CELLS === null) {
    MORTON_CELLS = new Uint16Array(2 ** 16);
    for (let morton = 0; morton < MORTON_CELLS.length; morton += 1) {
      MORTON_CELLS[morton] = compact(morton >>> 1) * 256 + compact(morton);
    }
  }
  return MORTON_CELLS;
}

function faceOfIndex(n: number): Face {
  switch (n) {
    case 0:
    case 1:
    case 2:
    case 3:
    case 4:
    case 5:
      return n;
    default:
      throw new Error(`a cell index names face ${n}, not 0 to 5`);
  }
}
