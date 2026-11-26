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
import type { Xyz } from "../terrain/cube";
import type { OceanMapAnswer } from "../terrain/workers/messages";
import { type OceanCellKind, type OceanMask, oceanCellKindOf } from "./oceanCells";

export {
  OPEN_WATER_MAX_ICE,
  type OceanCellKind,
  oceanCellKindOf,
  type OceanMask,
  oceanMaskOf,
} from "./oceanCells";

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
