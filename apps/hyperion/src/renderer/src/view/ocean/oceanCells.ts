/**
 * Open water decided per coarse cell (plan R11, R11.T8.b; Design note 11), `oceanMask.ts`' rule as
 * plain arithmetic with no engine, so that a height worker decides a map's mask beside its answer
 * (perf-long-tasks-5); `oceanMask.ts` re-exports it, with the rule's remarks.
 */
import { fieldCellCount, fieldCellGridIndex, isFieldLevel } from "../terrain/fieldCells";
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
 * code: the classifier's rule (see the remarks of `oceanMask.ts`).
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
    const at = fieldCellGridIndex(level, index);
    places[at] = k;
    if (oceanCellKindOf(depthM[k] ?? 0, ice[k] ?? 0) === "open-water") {
      open[at] = 1;
      openCells += 1;
    }
  }
  return { level, faceTexels: n, places, open, openCells };
}
