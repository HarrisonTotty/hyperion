/**
 * Streaming priority across views (plan R05, T7.d, Design note 24).
 *
 * @remarks
 * Demand is the union of every streaming view's selection, each patch requested once. A request's
 * priority is the largest over the views that want it of w_view × (ρ ÷ τ), the screen-space error
 * excess of the patch drawn in its place, with w_view 1 for the primary view and 0.25 for a
 * secondary, so that a second streaming camera shares the workers at lower priority. Patches in a
 * grounded body's forced region outrank everything.
 *
 * Each terrain pass bakes through its own lane of one pool, which shares its workers by the passes'
 * holes (plan R10, R10.T14.b; lean, 2026-11-01): a pass with a hole in its drawn set, a request
 * that {@link fillsHole}, takes the next free worker ahead of another pass's refinements, until it
 * has none. Without it, a second pass attached with nothing resident draws nothing while the
 * first's demand outweighs its own, as the first's does on a generated world.
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

/**
 * Whether a request fills a hole in its pass's drawn set: a patch some view sees with nothing
 * resident over it, which the pool bakes ahead of another pass's refinements (plan R10,
 * R10.T14.b; lean, 2026-11-01).
 *
 * @remarks
 * The demand asks for a patch that is not forced only under a baked parent, roots apart (the
 * patch-demand ruling's breadth-first rule, `selectPatches`' demand), so a pass's coarsest wanted
 * level with a hole is always its roots', and a request fills one exactly where it is a root's and
 * not forced. A forced patch may lie under nothing resident too, but forced requests outrank every
 * other already. A root's rebake, a resident root asked again, is taken for one as well: at most
 * the six roots a pass, while its body's blocks change them.
 */
export function fillsHole(request: PatchRequest): boolean {
  return !request.forced && request.key.level === 0;
}
