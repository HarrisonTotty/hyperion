/**
 * Which thick regimes the atmosphere cache routes to their bake (plan R08, R08.T14.c; Design note
 * 9's routing): a regime routes to its bake only once its gate passes, R08.T14.d's for
 * `thickScattering` and R08.T15's for `cloudDeck`. Until then a thick world is drawn by the
 * analytic term under `ATMOSPHERE: APPROXIMATE`, so T14.c builds the whole route with no regime
 * routed in the product, and its tests route by injection (`AtmosphereCache`'s `routes`).
 */

import type { AtmosphereRegime } from "./regime";

/**
 * The regimes whose gate has passed, so that the cache routes them to their bake: none yet.
 * R08.T14.d adds `thickScattering` when its gates pass; `cloudDeck`'s bake is R08.T15's, never
 * routed by {@link thickBakeRouted}.
 */
export const GATED_REGIMES: ReadonlySet<AtmosphereRegime["kind"]> = new Set<
  AtmosphereRegime["kind"]
>();

/**
 * Whether a body routed `regime` takes its thick bake (Design note 9's routing): only
 * `thickScattering`, and only where `gated` holds it. `thin` takes none, and `cloudDeck`'s bake is
 * R08.T15's deck split, which this route does not make.
 *
 * @param gated - The regimes whose gate has passed: {@link GATED_REGIMES}, or a test's.
 */
export function thickBakeRouted(
  regime: AtmosphereRegime,
  gated: ReadonlySet<AtmosphereRegime["kind"]> = GATED_REGIMES,
): boolean {
  let routed: boolean;
  switch (regime.kind) {
    case "thin":
    case "cloudDeck":
      routed = false;
      break;
    case "thickScattering":
      routed = gated.has("thickScattering");
      break;
  }
  return routed;
}
