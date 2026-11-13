/**
 * Where the ocean is drawn (plan R11, R11.T8.b; Design note 11): open water decided per coarse cell
 * from the height workers' field, as the classifier decides it, never again by a shader.
 *
 * @remarks
 * - **Under water** is the record's water surface above its coarse ground: a worker's
 *   `OceanMapAnswer.depthM` above 0, exactly `SynthesisCell::is_under_water`, the classifier's
 *   condition for `Seabed` and `SeaIce` (`material/classify.rs`).
 * - **Open water** is under water and not in the cell's ice: a point under water lies in the ice
 *   where the cell's ice share is above ½ (`classify.rs`' `ice_placement`, code 255 always, code 0
 *   never), so the sea is open exactly where the ice code is at most {@link OPEN_WATER_MAX_ICE}.
 *   Where the ice covers it the ocean gives way to R10's `SeaIce`, "with no glint and no waves".
 * - **A closed basin** is drawn at its own level, the cell's water surface
 *   (`OceanMapAnswer.levelM`, against the sea), not the sea's.
 * - **No sea over unsurveyed ground**: a cell the answer does not name is not water, since no
 *   coastline is claimed there (Design note 11).
 *
 * The mask's layout is the GPU coverage mask's (`terrain/coverage.ts`): six faces of 2ᴸ × 2ᴸ cells,
 * cell (face, L, i, j) at `(face · n + j) · n + i`, which `cubeTexel.wgsl`'s `class_map_texel` and
 * its twin `classMapTexelOf` find from a direction.
 */
import { classMapTexelOf } from "../bodies/discSurface";
import { fieldCellAt, fieldCellCount, isFieldLevel } from "../terrain/coverage";
import type { Xyz } from "../terrain/cube";
import type { OceanMapAnswer } from "../terrain/workers/messages";

/**
 * The largest ice code of open water: 127, since a point under water lies in its cell's ice where
 * the ice share, code ÷ 255, is above ½ (`classify.rs`' `ice_placement`), from code 128.
 */
export const OPEN_WATER_MAX_ICE = 127;

/** What a cell of a body's field is to the ocean. */
export type OceanCellKind =
  /** Under water and free of ice: the ocean is drawn. */
  | "open-water"
  /** Under water and in its ice: R10's `SeaIce`, no ocean. */
  | "sea-ice"
  /** Its ground above its water surface: no water. */
  | "dry"
  /** Not surveyed, or not held: no coastline is claimed, no sea is drawn. */
  | "unsurveyed";

/**
 * A surveyed cell's kind from its water depth (m, its water surface less its ground) and its ice
 * code: the classifier's rule (see the module's remarks).
 */
export function oceanCellKindOf(depthM: number, ice: number): Exclude<OceanCellKind, "unsurveyed"> {
  if (!(depthM > 0)) {
    return "dry";
  }
  return ice <= OPEN_WATER_MAX_ICE ? "open-water" : "sea-ice";
}

/** A body's ocean mask: each cell of its field's level, open water or not. */
export interface OceanMask {
  /** The field's level, 5 to 8. */
  readonly level: number;
  /** n = 2ᴸ, the cells along a face's side. */
  readonly faceTexels: number;
  /**
   * Each cell's place in the answer (`OceanMapAnswer.cells`' order), −1 for a cell it does not
   * name, at `(face · n + j) · n + i`.
   */
  readonly places: Int32Array;
  /** 1 for open water and 0 for any other cell, laid out as {@link OceanMask.places}. */
  readonly open: Uint8Array;
  /** How many cells are open water. */
  readonly openCells: number;
}

/**
 * The mask of a worker's answer: each surveyed cell it names decided by {@link oceanCellKindOf},
 * every other cell not water.
 *
 * @throws RangeError for a level R09 does not serve, arrays that disagree in length, or a cell
 *   index past the level's cells.
 */
export function oceanMaskOf(
  answer: Pick<OceanMapAnswer, "level" | "cells" | "depthM" | "ice">,
): OceanMask {
  const { level, cells, depthM, ice } = answer;
  if (!isFieldLevel(level)) {
    throw new RangeError(`a coarse field is at level 5 to 8, not ${String(level)}`);
  }
  if (depthM.length !== cells.length || ice.length !== cells.length) {
    throw new RangeError(
      `an ocean map of ${String(cells.length)} cells has ${String(depthM.length)} depths and ${String(ice.length)} ice codes`,
    );
  }
  const count = fieldCellCount(level);
  const n = 2 ** level;
  const places = new Int32Array(count).fill(-1);
  const open = new Uint8Array(count);
  let openCells = 0;
  for (let k = 0; k < cells.length; k += 1) {
    const index = cells[k] ?? -1;
    if (!(Number.isInteger(index) && index >= 0 && index < count)) {
      throw new RangeError(`an ocean map at level ${String(level)} has no cell ${String(index)}`);
    }
    const cell = fieldCellAt(level, index);
    const at = (cell.face * n + cell.j) * n + cell.i;
    places[at] = k;
    if (oceanCellKindOf(depthM[k] ?? 0, ice[k] ?? 0) === "open-water") {
      open[at] = 1;
      openCells += 1;
    }
  }
  return { level, faceTexels: n, places, open, openCells };
}

/** The mask's layout index of the cell holding a body-fixed direction (it need not be unit). */
export function oceanMaskIndexAt(mask: Pick<OceanMask, "faceTexels">, direction: Xyz): number {
  const n = mask.faceTexels;
  const { face, i, j } = classMapTexelOf(direction, n);
  return (face * n + j) * n + i;
}

/**
 * What the cell holding a body-fixed direction is to the ocean: its kind, from `answer`'s record of
 * it, or `unsurveyed` where the answer does not name it.
 */
export function oceanCellKindAt(
  mask: OceanMask,
  answer: Pick<OceanMapAnswer, "depthM" | "ice">,
  direction: Xyz,
): OceanCellKind {
  const place = mask.places[oceanMaskIndexAt(mask, direction)] ?? -1;
  if (place < 0) {
    return "unsurveyed";
  }
  return oceanCellKindOf(answer.depthM[place] ?? 0, answer.ice[place] ?? 0);
}
