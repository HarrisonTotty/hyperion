/**
 * R10's rebake rule, which the holder of a body's baked patches applies at each block of its field
 * (plan R10, Design note 16 as built by T6.b and R10.T8.b; `hyperion_surface::worker`'s module
 * documentation).
 *
 * @remarks
 * Cells arrive exact and whole with their margins, so a block changes no value at a vertex the
 * field held before it, and a resolution code alone changes no byte. A patch baked before a block
 * is baked again after it exactly where
 *
 * 1. its bake was not complete (a vertex took the reference surface, or has no weights, because
 *    the field lacked a cell its reads reach, which any block may bring);
 * 2. the block surveys a cell holding one of its vertices outside the survey, so that its survey
 *    mask changes; or
 * 3. the block brings a cell, surveyed or margin, that its horizon map's far field was refused for:
 *    a refused height names the first cell its reads lacked, every read before it held and final,
 *    so it can change only once that cell is held (decision-r10-t8a §3).
 *
 * `BodyBake::needs_rebake` is the same rule in the crate, whose tests hold that a bake the rule
 * passes over bakes the same bytes after the block, its horizon map's included.
 */

import type { FieldBlockAnswer, TerrainLayers } from "./messages";

/**
 * Whether `runs`, a block's cells as flat [first, past last) pairs, rising and disjoint, hold any
 * of `cells`, rising cell indices.
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
  change: Pick<FieldBlockAnswer, "newlySurveyed" | "newlyHeld">,
): boolean {
  if (layers === null) {
    return false;
  }
  return (
    !layers.complete ||
    surveysAny(change.newlySurveyed, layers.unsurveyedCells) ||
    surveysAny(change.newlyHeld, layers.horizonRefusedCells)
  );
}

/**
 * Whether a body's bake made while its field's blocks were being posted is baked again, before the
 * blocks' changes are known: unless it is complete, its horizon map is complete, and its mask is
 * whole (R10's rebake rule taken conservatively, decision-r10-t8a §3). A bake with no layers is
 * baked again.
 */
export function staleInFlight(layers: TerrainLayers | null): boolean {
  return (
    layers === null ||
    !layers.complete ||
    !layers.horizonComplete ||
    layers.unsurveyedCells.length > 0
  );
}
