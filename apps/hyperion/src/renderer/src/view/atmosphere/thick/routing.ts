/**
 * Which thick regimes the atmosphere cache routes to their bake (plan R08, R08.T14.c; Design note
 * 9's routing): a regime routes to its bake only once its gate passes, R08.T14.d's for
 * `thickScattering` and R08.T15's for `cloudDeck`. Until then a thick world is drawn by the
 * analytic term under `ATMOSPHERE: APPROXIMATE`, so T14.c builds the whole route with no regime
 * routed in the product, and its tests route by injection (`AtmosphereCache`'s `routes`).
 *
 * And whether it asks a world's polarisation correction (R08.T14.f; Design note 10): built whole,
 * and held in the product ({@link CORRECTION_ROUTED}) until main turns it on, its tests asking by
 * injection (`AtmosphereCache`'s `corrects`).
 */

import { correctionBySources } from "./columnLayout";
import type { AtmosphereRegime } from "./regime";
import type { SpectralColumn } from "./spectralColumn";

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

/**
 * Whether the product asks every world with air its polarisation correction (plan R08, R08.T14.f):
 * not yet. Turning it on changes every such world's draw path, to the thick kernels' read beside
 * Ψ_ms, as R08.T14.d's routing changes a thick world's, so it waits on what R08.T14.d's does (plan
 * R08's Risks, "Preconditions of the correction's route"): the owner's RTX run of the frame timing's
 * (M) and (N) against `decision-r08-t11-frame.md`'s 16.67 ms, the guide's `ATMOSPHERE: COMPUTING`
 * and `ATMOSPHERE: APPROXIMATE` rows for it signed off, and Design note 11's 2 MB row counting it.
 * `hillaire.ts`'s callers make the thick kernels ahead once it is on.
 */
export const CORRECTION_ROUTED = false;

/**
 * Whether a body drawn from `column` takes its polarisation correction: where `routed`, every column
 * `columnLayout.ts`'s `correctionBySources` does not rule out (every world with air, thin or thick;
 * one whose aerosols alone could tell is asked, the bake telling).
 *
 * @param routed - Whether the correction is routed: {@link CORRECTION_ROUTED}, or a test's.
 */
export function correctionRouted(
  column: SpectralColumn,
  routed: boolean = CORRECTION_ROUTED,
): boolean {
  return routed && correctionBySources(column) !== false;
}

/**
 * Whether a body drawn from `column` takes its disc's reflectance table (plan R08, R08.T16.b),
 * which R07's disc reads beyond the 10⁹ m boundary: every world with air whose column has a term
 * (lean, 2026-10-30: asked for every drawn body, near or far, so that a body's table is held before
 * it crosses the boundary; until it lands the full passes draw it at every distance).
 */
export function discTableRouted(column: SpectralColumn): boolean {
  return column.terms.length > 0;
}
