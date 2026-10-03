/**
 * Streaming priority across views (plan R05, T7.d, Design note 24).
 *
 * @remarks
 * Demand is the union of every streaming view's selection, each patch requested once. A request's
 * priority is the largest over the views that want it of w_view × (ρ ÷ τ), the screen-space error
 * excess of the patch drawn in its place, with w_view 1 for the primary view and 0.25 for a
 * secondary, so that a second streaming camera shares the workers at lower priority. Patches in a
 * grounded body's forced region outrank everything.
 */

import { patchKeyString } from "./patchKey";
import type { PatchRequest } from "./select";

/** The streaming weight of a view: 1 for the primary view (Design note 24). */
export const PRIMARY_VIEW_WEIGHT = 1;

/** The streaming weight of a secondary view: 0.25 (Design note 24). */
export const SECONDARY_VIEW_WEIGHT = 0.25;

/**
 * Orders requests for the pool: forced first, then the higher priority, then by `patchKeyString`,
 * so that equal priorities keep one order however the views were given.
 */
export function compareRequests(a: PatchRequest, b: PatchRequest): number {
  if (a.forced !== b.forced) {
    return a.forced ? -1 : 1;
  }
  if (a.priority !== b.priority) {
    return b.priority - a.priority;
  }
  const ka = patchKeyString(a.key);
  const kb = patchKeyString(b.key);
  return ka < kb ? -1 : ka > kb ? 1 : 0;
}
