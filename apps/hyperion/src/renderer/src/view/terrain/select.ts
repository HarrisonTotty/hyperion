/**
 * Patch selection: which patches each view needs, as a pure function of the views, the quality
 * setting and the grounded bodies (plan R05, T7.b–T7.d, Design note 7).
 */

import type { PatchBounds } from "./bounds";
import type { PatchKey } from "./patchKey";

/** A patch the selection asks to draw. */
export interface SelectedPatch {
  readonly key: PatchKey;
  readonly bounds: PatchBounds;
  /** Whether it lies in a grounded body's forced region, which the cache never evicts. */
  readonly forced: boolean;
}

/** A request to bake a patch, with its streaming priority (Design note 24). */
export interface PatchRequest {
  readonly key: PatchKey;
  /** Higher is sooner: the largest over the views of w_view × (ρ ÷ τ). */
  readonly priority: number;
  /** Whether it lies in a grounded body's forced region, which outranks everything. */
  readonly forced: boolean;
}

/** What selection returns: the patches to draw, by `patchKeyString`, and the demand to bake. */
export interface Selection {
  readonly patches: ReadonlyMap<string, SelectedPatch>;
  readonly demand: ReadonlyArray<PatchRequest>;
}
