/**
 * Which thick worlds the atmosphere cache routes to their bake (plan R08, R08.T14.c and
 * R08.T14.d.4; Design note 9's routing): a regime routes to its bake only where its gate passes,
 * R08.T14.d's for `thickScattering` and R08.T15's for `cloudDeck`, and since R08.T14.d.4 a
 * `thickScattering` world only within the range its passing gates cover ({@link THICK_ROUTE_RANGE};
 * main's rule on routing by the depth range the passing cases cover,
 * `decision-r08-t13c2-classes.md` §6). Elsewhere a thick world is drawn by the analytic term under
 * `ATMOSPHERE: APPROXIMATE`. Tests route by injection (`AtmosphereCache`'s `routes`).
 *
 * And whether it asks a world's polarisation correction (R08.T14.f; Design note 10): turned on with
 * the route ({@link CORRECTION_ROUTED}), for thin worlds and routed thick worlds alone (main, with
 * T14.d's split: an unrouted thick world takes none), its tests asking by injection
 * (`AtmosphereCache`'s `corrects`).
 */

import type { AtmosphereMedium } from "../medium";
import { correctionBySources } from "./columnLayout";
import { type AtmosphereRegime, regimeChannels } from "./regime";
import type { SpectralColumn } from "./spectralColumn";

/**
 * A range of `regime.ts`'s `regimeChannels` over which a regime's gates pass: every channel's
 * optical depth τ at most `mostOpticalDepth`, its single-scattering albedo ω̄ at least
 * `leastSingleScatteringAlbedo`, its asymmetry ḡ at most `mostAsymmetry`, and the ground's albedo
 * in it at most `mostGroundAlbedo`; and jointly, so that the box's corners, each bound from another
 * case, are not taken for gated, τ ḡ at most `mostForwardDepth` (the forward-peaked source of
 * R08.T14.d's precondition 1) and τ (1 − ω̄) at most `mostAbsorbedDepth`.
 */
export interface ThickRouteRange {
  readonly mostOpticalDepth: number;
  readonly leastSingleScatteringAlbedo: number;
  readonly mostAsymmetry: number;
  readonly mostGroundAlbedo: number;
  readonly mostForwardDepth: number;
  readonly mostAbsorbedDepth: number;
}

/**
 * The range the `thickScattering` regime's passing gates cover (R08.T14.d.4): the channels of the
 * kept cases whose route passes R08.T14.d's vector gate (`reference/vectorGate.json`), `earth`,
 * `earth-ozone-slab` and `saturn-oblate`, but for twilight below the horizon, which s95 bounds as
 * the pseudo-spherical model's limit (main's ruling; {@link TWILIGHT_REASON}); `earth`'s two s90
 * views and its disc's red at p0 and p30, some 0.2% past the edge since the ground's light reads
 * the thick table, are recorded misses, and whether `earth` still bounds the range is main's to
 * rule.
 *
 * @remarks
 * Each bound is the cases' extreme, rounded outward at its third figure (the test holds them to the
 * cases): τ to 0.7279 (`saturn-oblate`'s blue the deepest), ω̄ from 0.9507 and ḡ to 0.3871
 * (`earth`'s red), the ground to 0.3, τ ḡ to 0.0837 and τ (1 − ω̄) to 0.0109 (`earth`'s blue). Out
 * of it stand a Mars-class dust's forward peak (ḡ 0.71–0.76, τ ḡ 0.36–0.38), which stays
 * `ATMOSPHERE: APPROXIMATE` (precondition 2, main's ruling with the split), and the Venus- and
 * Titan-class columns (τ 7–77), whose gates pass no depth until the re-traced references land.
 * Neither the absorbers' depth, which the channels leave out (only `earth`'s ozone is gated), nor
 * the radius over the scale height, on which the pseudo-spherical beam's low sun depends, is
 * bounded.
 */
export const THICK_ROUTE_RANGE: ThickRouteRange = {
  mostOpticalDepth: 0.728,
  leastSingleScatteringAlbedo: 0.95,
  mostAsymmetry: 0.388,
  mostGroundAlbedo: 0.3,
  mostForwardDepth: 0.0838,
  mostAbsorbedDepth: 0.0109,
};

/** Whether every channel of `medium`, as `classifyRegime` reads it, lies within `range`. */
export function withinRouteRange(medium: AtmosphereMedium, range: ThickRouteRange): boolean {
  return regimeChannels(medium).every(
    (channel) =>
      channel.opticalDepth <= range.mostOpticalDepth &&
      channel.singleScatteringAlbedo >= range.leastSingleScatteringAlbedo &&
      channel.asymmetry <= range.mostAsymmetry &&
      channel.groundAlbedo <= range.mostGroundAlbedo &&
      channel.opticalDepth * channel.asymmetry <= range.mostForwardDepth &&
      channel.opticalDepth * (1 - channel.singleScatteringAlbedo) <= range.mostAbsorbedDepth,
  );
}

/**
 * Whether a body routed `regime` takes its thick bake (Design note 9's routing): only
 * `thickScattering`, and only where its medium lies within `range`. `thin` takes none, and
 * `cloudDeck`'s bake is R08.T15's deck split, which this route does not make.
 *
 * @param range - The range its gates pass over: {@link THICK_ROUTE_RANGE}, or `null` for none.
 */
export function thickBakeRouted(
  regime: AtmosphereRegime,
  medium: AtmosphereMedium,
  range: ThickRouteRange | null = THICK_ROUTE_RANGE,
): boolean {
  let routed: boolean;
  switch (regime.kind) {
    case "thin":
    case "cloudDeck":
      routed = false;
      break;
    case "thickScattering":
      routed = range !== null && withinRouteRange(medium, range);
      break;
  }
  return routed;
}

/**
 * Why a routed thick world carries `atmosphereApproximate` where its brightest light stands below
 * the camera's local horizontal, the camera in its air (R08.T14.d.4), as player text in
 * `THICK_REASON`'s form: the bake's twilight is checked to the horizon, s90, and s95 lies past the
 * pseudo-spherical model's limit (main's ruling with T14.d.2's step 3). Its words await the owner's
 * UX sign-off.
 */
export const TWILIGHT_REASON =
  "multiple scattering: computed beyond its checked range, the brightest star below the local horizontal";

/**
 * Whether the product asks a world with air its polarisation correction (plan R08, R08.T14.f): on
 * since R08.T14.d.4's route, by main's ruling with T14.d's split, for thin worlds and routed thick
 * worlds alone, which the atmosphere cache decides with the route (an unrouted thick world takes
 * none: question 3). Every such world draws by the thick kernels' read, which `hillaire.ts`'s
 * callers make ahead. The guide's rows for its `ATMOSPHERE: COMPUTING` and its failure await the
 * owner's UX sign-off, and its frame against `decision-r08-t11-frame.md`'s 16.67 ms the owner's RTX
 * run.
 */
export const CORRECTION_ROUTED = true;

/**
 * Whether a body drawn from `column` takes its polarisation correction: where `routed`, every column
 * `columnLayout.ts`'s `correctionBySources` does not rule out (one whose aerosols alone could tell
 * is asked, the bake telling). The atmosphere cache asks it of thin worlds and routed thick worlds
 * alone.
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
