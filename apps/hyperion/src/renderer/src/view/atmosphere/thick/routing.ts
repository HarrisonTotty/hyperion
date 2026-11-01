/**
 * Which thick worlds the atmosphere cache routes to their bake (plan R08, R08.T14.c and
 * R08.T14.d.4; Design note 9's routing): a regime routes to its bake only where its gate passes,
 * R08.T14.d's for `thickScattering` and R08.T15's for `cloudDeck`, and since R08.T14.d.4 a
 * `thickScattering` world only within the range its passing gates cover ({@link THICK_ROUTE_RANGE};
 * main's rule on routing by the depth range the passing cases cover,
 * `decision-r08-t13c2-classes.md` §6). Elsewhere a thick world is drawn by the analytic term under
 * `ATMOSPHERE: APPROXIMATE`. A `cloudDeck` world routes to its deck's bake only within the range its
 * passing deck gates cover ({@link DECK_ROUTE_RANGE}, R08.T15.b.2), which none does yet. Tests route
 * by injection (`AtmosphereCache`'s `routes`).
 *
 * And whether it asks a world's polarisation correction (R08.T14.f; Design note 10): turned on with
 * the route ({@link CORRECTION_ROUTED}), for thin worlds and routed thick worlds alone (main, with
 * T14.d's split: an unrouted thick world takes none), its tests asking by injection
 * (`AtmosphereCache`'s `corrects`).
 */

import type { AtmosphereMedium } from "../medium";
import { correctionBySources } from "./columnLayout";
import type { DeckFloor } from "./deck";
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
 * `cloudDeck`'s bake is R08.T15's deck split, which {@link deckRouted} routes.
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
 * The ranges over which the cloud-deck split's gates pass (R08.T15.b.2; Design note 9's routing), by
 * the split's floor (`thick/deck.ts`'s `DeckFloor`), each over the whole medium's channels as
 * {@link withinRouteRange} reads them, or `null` where that floor routes nothing:
 *
 * - `ground`, a deck over a surface (Venus-class), once both of R08.T15's gates pass: above the
 *   deck, T15.a.2's `giant-deck`, and below it, T15.b.2's Venus-class cases;
 * - `envelope`, a gas envelope split at its floor, which has no view from below, on `giant-deck`'s
 *   envelope variant alone (`reference/vectorGate.json`'s `deck.asAnEnvelope`; R08.T16.a.2's
 *   opening, research's amendment of 2026-11-01), never on Venus's gate.
 */
export type DeckRouteRange = Readonly<Record<DeckFloor, ThickRouteRange | null>>;

/**
 * The range the cloud-deck split's passing gates cover (R08.T15.b.2): none. No deck case passes:
 * the Venus-class cases' scalar gate refuses nearly every value below the deck at their references'
 * 1,024 samples, and `giant-deck`'s envelope variant refuses all 30 of its gated values (lean,
 * 2026-11-01: a deck over a surface routes only once both of R08.T15's gates pass, as the plan says,
 * and a gas envelope's opening is R08.T16.a.2's, through this hook). A `cloudDeck` world stays
 * drawn by the analytic term under `ATMOSPHERE: APPROXIMATE`, its deck's bake unasked.
 */
export const DECK_ROUTE_RANGE: DeckRouteRange | null = null;

/**
 * Whether a body routed `regime` takes its deck's bake (Design note 9's split; the atmosphere
 * cache's deck path, `thick/deck.ts`'s `bakeDeck`): only `cloudDeck`, at a split above the ground,
 * and only where `range` opens its floor and its medium lies within that floor's range. `thin` and
 * `thickScattering` take none ({@link thickBakeRouted} routes the latter).
 *
 * @param envelope - Whether the body's cover is a gas envelope's, which splits at its floor.
 * @param range - The ranges its gates pass over: {@link DECK_ROUTE_RANGE}, or a test's.
 */
export function deckRouted(
  regime: AtmosphereRegime,
  medium: AtmosphereMedium,
  envelope: boolean,
  range: DeckRouteRange | null = DECK_ROUTE_RANGE,
): boolean {
  let routed: boolean;
  switch (regime.kind) {
    case "thin":
    case "thickScattering":
      routed = false;
      break;
    case "cloudDeck": {
      const bounds = range?.[envelope ? "envelope" : "ground"] ?? null;
      routed = regime.deckAltitudeM > 0 && bounds !== null && withinRouteRange(medium, bounds);
      break;
    }
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
