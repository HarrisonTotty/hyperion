/**
 * R10's rebake rule, which the holder of a body's baked patches applies at each block of its field
 * (plan R10, Design note 16 as built by T6.b; `hyperion_surface::worker`'s module documentation).
 *
 * @remarks
 * Cells arrive exact and whole with their margins, so a block changes no value at a vertex the
 * field held before it, and a resolution code alone changes no byte. A patch baked before a block
 * is baked again after it exactly where its bake was not complete (a vertex took the reference
 * surface, or has no weights, because the field lacked a cell its reads reach, which any block may
 * bring), or where the block surveys a cell holding one of its vertices outside the survey, so that
 * its survey mask changes. `BodyBake::needs_rebake` is the same rule in the crate, whose tests hold
 * that a bake the rule passes over bakes the same bytes after the block.
 */

import type { FieldBlockAnswer, TerrainLayers } from "./messages";

/**
 * Whether `runs`, a block's newly surveyed cells as flat [first, past last) pairs, rising and
 * disjoint, hold any of `cells`, rising cell indices.
 */
export function surveysAny(runs: Uint32Array, cells: Uint32Array): boolean {
  let r = 0;
  for (const cell of cells) {
    let end = runs[r + 1];
    while (end !== undefined && end <= cell) {
      r += 2;
      end = runs[r + 1];
    }
    const start = runs[r];
    if (start === undefined || end === undefined) {
      return false;
    }
    if (start <= cell) {
      return true;
    }
  }
  return false;
}

/**
 * Whether a body's patch baked with `layers` is to be baked again after the block that made
 * `change` (the module documentation's rule). A test planet's bake carries no layers and reads no
 * field, so no block makes it stale.
 */
export function needsRebake(
  layers: TerrainLayers | null,
  change: Pick<FieldBlockAnswer, "newlySurveyed">,
): boolean {
  if (layers === null) {
    return false;
  }
  return !layers.complete || surveysAny(change.newlySurveyed, layers.unsurveyedCells);
}
