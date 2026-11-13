/**
 * A body's survey as selection reads it (plan R10, R10.T16.b; decision-r10-live-path, D6, and
 * decision-r10-t16-contracts §2–3): which patches have a surveyed coarse cell under them, so that
 * selection leaves the bare ground to the reference surface (R10 Design note 12).
 *
 * @remarks
 * A bake's survey mask reads, at each vertex, the coarse cell holding the vertex's direction
 * (`hyperion_surface`'s `PatchKey::containing`), and a vertex on a cell's edge or corner reads one
 * of the cells meeting there as its rounding falls. So the cells a patch's vertices can read are
 * those its closed area meets: for a patch no finer than the field, the cells under it and the ring
 * about them; for a finer one, the cell holding it and those across the edges and corners of that
 * cell it touches. The lookup answers whether any of them is surveyed, never less: no patch it calls
 * bare bakes a vertex inside the survey.
 *
 * For the first case it keeps the surveyed cells dilated by one cell, across face edges and at cube
 * corners (`stepCellInto`), in a pyramid of every level up to the field's: a patch no finer than
 * the field holds a dilated cell exactly where its area or its ring holds a surveyed one, since a
 * cell's neighbours are its neighbours' neighbours. At level 8, R09's deepest field, that is about
 * 0.9 MB a body, the cells' own bytes and the pyramid's.
 */

import type { CoverageView } from "./coverage";
import { fieldCellAt, fieldCellCount, isFieldLevel } from "./coverage";
import { type CellOut, type PatchKey, stepCellInto } from "./patchKey";
import type { SurveyLookup } from "./select";
import type { FieldBlockAnswer } from "./workers/messages";

/** The steps (di, dj) to a cell's eight neighbours. */
const NEIGHBOURS: ReadonlyArray<readonly [number, number]> = [
  [-1, 0],
  [1, 0],
  [0, -1],
  [0, 1],
  [-1, -1],
  [1, -1],
  [-1, 1],
  [1, 1],
];

/** A cell's index in a level's face-major rows: face, then row j, then column i. */
function rowIndex(face: number, level: number, i: number, j: number): number {
  const n = 2 ** level;
  return (face * n + j) * n + i;
}

/**
 * The cells of one body's field surveyed so far, from the coarse-field store's coverage and the
 * blocks the height workers took, as selection's {@link SurveyLookup}.
 *
 * @remarks
 * Cells are only added, as a coverage only grows (`CoverageView.merged`); {@link SurveyedCells.clear}
 * forgets them all, for a field the workers dropped. {@link SurveyedCells.version} rises with every
 * cell added, so that a terrain pass selects again.
 */
export class SurveyedCells implements SurveyLookup {
  #level: number | null = null;
  /** The surveyed cells at the field's level, by {@link rowIndex}. */
  #cells: Uint8Array | null = null;
  /** At each level up to the field's, whether a dilated cell lies under each patch. */
  #pyramid: Uint8Array[] = [];
  #version = 0;
  readonly #step: CellOut = { face: 0, i: 0, j: 0 };
  /**
   * The field-level cell under a finer patch, rewritten in place by each lookup, so that selection's
   * hot path allocates nothing (the batch review's C9).
   */
  readonly #under: { face: PatchKey["face"]; level: number; i: number; j: number } = {
    face: 0,
    level: 0,
    i: 0,
    j: 0,
  };

  /** A count of the changes: every cell added, and every clear. */
  get version(): number {
    return this.#version;
  }

  /** The field's level, 5 to 8, or `null` before any cell. */
  get level(): number | null {
    return this.#level;
  }

  /**
   * Adds the cells a block surveyed anew (`FieldBlockAnswer.newlySurveyed`'s runs of cell indices
   * at `fieldLevel`); cells of another level than those held replace them.
   *
   * @throws RangeError for a level R09 does not serve, or a run that is not one of its cells.
   */
  add(change: Pick<FieldBlockAnswer, "fieldLevel" | "newlySurveyed">): void {
    const { fieldLevel: level, newlySurveyed: runs } = change;
    if (!isFieldLevel(level)) {
      throw new RangeError(`a coarse field is at level 5 to 8, not ${String(level)}`);
    }
    const count = fieldCellCount(level);
    for (let r = 0; r + 1 < runs.length; r += 2) {
      const start = runs[r] ?? 0;
      const end = runs[r + 1] ?? 0;
      if (!(start < end && end <= count)) {
        throw new RangeError(
          `cells ${String(start)}..${String(end)} are not a run of level ${String(level)}'s`,
        );
      }
    }
    for (let r = 0; r + 1 < runs.length; r += 2) {
      for (let index = runs[r] ?? 0; index < (runs[r + 1] ?? 0); index += 1) {
        this.#addCell(fieldCellAt(level, index));
      }
    }
  }

  /** Adds every surveyed cell of a coverage (the store's answers). */
  merge(coverage: CoverageView): void {
    const level = coverage.level;
    const texels = coverage.maskTexels();
    const n = 2 ** level;
    for (let at = 0; at < texels.length; at += 1) {
      if ((texels[at] ?? 0) === 0) {
        continue;
      }
      const i = at % n;
      const j = Math.floor(at / n) % n;
      const face = Math.floor(at / (n * n));
      if (this.#cells?.[rowIndex(face, level, i, j)] === 1 && this.#level === level) {
        continue;
      }
      this.#addCell({ face: faceOf(face), level, i, j });
    }
  }

  /** Forgets every cell: the workers dropped the body's field. */
  clear(): void {
    if (this.#level === null) {
      return;
    }
    this.#level = null;
    this.#cells = null;
    this.#pyramid = [];
    this.#version += 1;
  }

  /**
   * Whether a vertex of patch `key` can read a surveyed cell (see the module's remarks); `false`
   * with no cell held.
   */
  surveyedUnder(key: PatchKey): boolean {
    const level = this.#level;
    const cells = this.#cells;
    if (level === null || cells === null) {
      return false;
    }
    if (key.level <= level) {
      return this.#pyramid[key.level]?.[rowIndex(key.face, key.level, key.i, key.j)] === 1;
    }
    const shift = key.level - level;
    const cell = this.#under;
    cell.face = key.face;
    cell.level = level;
    cell.i = key.i >> shift;
    cell.j = key.j >> shift;
    if (cells[rowIndex(cell.face, level, cell.i, cell.j)] === 1) {
      return true;
    }
    // The cells across the edges and corners of its cell that the patch touches.
    const last = 2 ** shift - 1;
    const withinI = key.i & last;
    const withinJ = key.j & last;
    const di = withinI === 0 ? -1 : withinI === last ? 1 : 0;
    const dj = withinJ === 0 ? -1 : withinJ === last ? 1 : 0;
    // A patch as wide as its cell is not finer than it, so it touches one side of each axis at most.
    return (
      (di !== 0 && this.#surveyedAt(cell, di, 0)) ||
      (dj !== 0 && this.#surveyedAt(cell, 0, dj)) ||
      (di !== 0 && dj !== 0 && this.#surveyedAt(cell, di, dj))
    );
  }

  /** Whether the cell (`di`, `dj`) from `cell`, folded across a face's edge, is surveyed. */
  #surveyedAt(cell: PatchKey, di: number, dj: number): boolean {
    const out = this.#step;
    return (
      stepCellInto(out, cell, di, dj) &&
      this.#cells?.[rowIndex(out.face, cell.level, out.i, out.j)] === 1
    );
  }

  /** Adds one surveyed cell, dilating it into the pyramid. */
  #addCell(cell: PatchKey): void {
    const level = cell.level;
    if (this.#level !== level) {
      this.#level = level;
      this.#cells = new Uint8Array(fieldCellCount(level));
      this.#pyramid = Array.from({ length: level + 1 }, (_, k) => new Uint8Array(6 * 4 ** k));
    }
    const cells = this.#cells;
    if (cells === null) {
      throw new Error("the survey's cells were not made with its level");
    }
    const at = rowIndex(cell.face, level, cell.i, cell.j);
    if (cells[at] === 1) {
      return;
    }
    cells[at] = 1;
    this.#version += 1;
    this.#dilate(cell.face, cell.i, cell.j);
    const out = this.#step;
    for (const [di, dj] of NEIGHBOURS) {
      if (stepCellInto(out, cell, di, dj)) {
        this.#dilate(out.face, out.i, out.j);
      }
    }
  }

  /** Marks the field-level cell (`face`, `i`, `j`) dilated, and the patches above it. */
  #dilate(face: number, i: number, j: number): void {
    const level = this.#level;
    if (level === null) {
      return;
    }
    for (let k = level; k >= 0; k -= 1) {
      const shift = level - k;
      const flags = this.#pyramid[k];
      const at = rowIndex(face, k, i >> shift, j >> shift);
      if (flags === undefined || flags[at] === 1) {
        // Everything above a marked patch is marked already.
        return;
      }
      flags[at] = 1;
    }
  }
}

/** A face index of a coverage's texels, 0 to 5. */
function faceOf(n: number): PatchKey["face"] {
  switch (n) {
    case 0:
    case 1:
    case 2:
    case 3:
    case 4:
    case 5:
      return n;
    default:
      throw new Error(`a coverage's texels name face ${String(n)}, not 0 to 5`);
  }
}
