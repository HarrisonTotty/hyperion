/**
 * Which thick worlds the atmosphere cache routes to their bake (plan R08, R08.T14.c and
 * R08.T14.d.4; Design note 9's routing): a regime routes to its bake only where its gate passes,
 * R08.T14.d's for `thickScattering` and R08.T15's for `cloudDeck`, and since R08.T14.d.4 a
 * `thickScattering` world only within the range its passing gates cover ({@link THICK_ROUTE_RANGE};
 * main's rule on routing by the depth range the passing cases cover,
 * `decision-r08-t13c2-classes.md` §6). Elsewhere a thick world is drawn by the analytic term under
 * `ATMOSPHERE: APPROXIMATE`. A Titan-class haze's range opens on `titan-class`'s gate alone, once
 * its recorded verdict passes ({@link HAZE_GATE_TALLIES}, R08.T14.d.4), which it does not yet. A
 * `cloudDeck` world routes to its deck's bake only within the range its
 * passing deck gates cover ({@link DECK_ROUTE_RANGE}, R08.T15.b.2), which none does yet: a gas
 * envelope's opens on `giant-deck`'s envelope variant alone, once its recorded verdict passes
 * ({@link ENVELOPE_GATE_TALLIES}, R08.T16.a.2). Tests route by injection (`AtmosphereCache`'s
 * `routes`).
 *
 * And whether it asks a world's polarisation correction (R08.T14.f; Design note 10): turned on with
 * the route ({@link CORRECTION_ROUTED}), for thin worlds and routed thick worlds alone (main, with
 * T14.d's split: an unrouted thick world takes none), its tests asking by injection
 * (`AtmosphereCache`'s `corrects`).
 */

import type { AtmosphereMedium } from "../medium";
import { correctionBySources } from "./columnLayout";
import type { DeckFloor } from "./deck";
import { upperMediumOf } from "./deckRead";
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
 * Titan-class columns (τ 7–77): Venus's is a deck's (R08.T15.b.2), and Titan's has a range of its
 * own, {@link HAZE_ROUTE_RANGE}, closed while its gate misses. Neither the absorbers' depth, which
 * the channels leave out (only `earth`'s ozone is gated), nor the radius over the scale height, on
 * which the pseudo-spherical beam's low sun depends, is bounded.
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
 * `thickScattering`, and only where its medium lies within one of `ranges`. `thin` takes none, and
 * `cloudDeck`'s bake is R08.T15's deck split, which {@link deckRouted} routes.
 *
 * @param ranges - The ranges its gates pass over: {@link THICK_ROUTE_RANGES}, or a test's; none
 *   routes nothing.
 */
export function thickBakeRouted(
  regime: AtmosphereRegime,
  medium: AtmosphereMedium,
  ranges: ReadonlyArray<ThickRouteRange> = THICK_ROUTE_RANGES,
): boolean {
  let routed: boolean;
  switch (regime.kind) {
    case "thin":
    case "cloudDeck":
      routed = false;
      break;
    case "thickScattering":
      routed = ranges.some((range) => withinRouteRange(medium, range));
      break;
  }
  return routed;
}

/**
 * The ranges over which the cloud-deck split's gates pass (R08.T15.b.2; Design note 9's routing), by
 * the split's floor (`thick/deck.ts`'s `DeckFloor`), each as {@link withinRouteRange} reads it, or
 * `null` where that floor routes nothing:
 *
 * - `ground`, a deck over a surface (Venus-class), over the whole medium's channels, once both of
 *   R08.T15's gates pass: above the deck, T15.a.2's `giant-deck`, and below it, T15.b.2's
 *   Venus-class cases;
 * - `envelope`, a gas envelope split at its floor, which has no view from below, over the channels
 *   of its medium above the floor, the only part drawn (R08.T16.a.2; below it the deck bakes to
 *   τ 1,000 over a black floor), on `giant-deck`'s envelope variant alone
 *   (`reference/vectorGate.json`'s `deck.asAnEnvelope`; research's amendment of 2026-11-01), never
 *   on Venus's gate.
 */
export type DeckRouteRange = Readonly<Record<DeckFloor, ThickRouteRange | null>>;

/**
 * One image's recorded verdict in a deck gate's tallies (`reference/vectorGate.json`'s
 * `deck.<variant>.tallies.images`): its comparisons, and how many of them pass, miss and are
 * refused (a refusal, noise past the reference's σ, neither passes nor misses).
 */
export interface GateImageTally {
  readonly image: string;
  readonly comparisons: number;
  readonly passes: number;
  readonly misses: number;
  readonly refusals: number;
}

/**
 * `giant-deck`'s envelope variant's recorded verdict by image (R08.T16.a.2): `vectorGate.json`'s
 * `deck.asAnEnvelope.tallies.images`, the case split at its envelope floor (21,958 m) over a black
 * floor, through the route an envelope takes, held to the file by the test. Against the re-traced
 * reference (R08.T12.h's), 27 of its 30 values pass and 3 miss, on the disc at p75 (red, 0.92 of
 * the reference) and p85 (red and green, from 0.85), so the route stays closed
 * ({@link DECK_ROUTE_RANGE}).
 */
export const ENVELOPE_GATE_TALLIES: ReadonlyArray<GateImageTally> = [
  { image: "disc-p0", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p30", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p60", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p75", comparisons: 3, passes: 2, misses: 1, refusals: 0 },
  { image: "disc-p85", comparisons: 3, passes: 1, misses: 2, refusals: 0 },
  { image: "limb", comparisons: 15, passes: 15, misses: 0, refusals: 0 },
];

/**
 * Whether a deck gate's recorded tallies pass (R08.T16.a.2; lean, 2026-11-01): every image passes
 * somewhere and none misses, so that a route opens only on passes (research-leans-16, item 5,
 * condition 2), and a gate whose limb misses, or is refused throughout, keeps it closed.
 */
export function deckGatePasses(tallies: ReadonlyArray<GateImageTally>): boolean {
  return tallies.length > 0 && tallies.every((image) => image.passes > 0 && image.misses === 0);
}

/**
 * `titan-class`'s recorded verdict by image through its route (R08.T14.d.4): `vectorGate.json`'s
 * `titanClass.routed.images`, its whole column baked at m ≤ 2 with its correction as a haze route
 * bakes it, its green and blue at their own wavelengths, its diffuse streams on their curved paths
 * and its rows of a low sun with the horizon's first-order terms (`bake.ts`'s "Own wavelengths",
 * "Curved streams" and "The horizon's first-order terms"), against the re-traced reference's
 * vector I (R08.T12.h's), held to the file by the test. 182 of its 360 values pass, 3 miss and 175
 * are refused, so the route stays closed ({@link THICK_ROUTE_RANGES}); every miss is red and bright:
 *
 * - the sky at s30 (v30, a45 and a180: 1.07 and 1.10 of the reference), within the gate at m < 8;
 * - the disc at p60 (1.07), mode-free, the red still the bins' (its methane's curve at the floor).
 *
 * The horizon's 21 views at s90, dim in red at 0.76–0.88 without the terms, read 0.99–1.04.
 */
export const HAZE_GATE_TALLIES: ReadonlyArray<GateImageTally> = [
  { image: "ground-s0", comparisons: 12, passes: 12, misses: 0, refusals: 0 },
  { image: "ground-s30", comparisons: 60, passes: 48, misses: 2, refusals: 10 },
  { image: "ground-s60", comparisons: 60, passes: 36, misses: 0, refusals: 24 },
  { image: "ground-s80", comparisons: 63, passes: 34, misses: 0, refusals: 29 },
  { image: "ground-s90", comparisons: 63, passes: 21, misses: 0, refusals: 42 },
  { image: "ground-s95", comparisons: 63, passes: 0, misses: 0, refusals: 63 },
  { image: "disc-p0", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p30", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p60", comparisons: 3, passes: 2, misses: 1, refusals: 0 },
  { image: "disc-p75", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p85", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "limb", comparisons: 15, passes: 12, misses: 0, refusals: 3 },
  { image: "aerial", comparisons: 9, passes: 5, misses: 0, refusals: 4 },
];

/**
 * The gate's images whose sun stands below the horizon, s95 (`reference/cases.ts`'s sphere set): a
 * routed body's view there reads `ATMOSPHERE: APPROXIMATE` with {@link TWILIGHT_REASON}, past the
 * pseudo-spherical model's limit (main's ruling with T14.d.2's step 3), so no haze gate is judged
 * on them.
 */
export const BELOW_THE_HORIZON_IMAGES: ReadonlyArray<string> = ["ground-s95"];

/**
 * Whether a haze gate's recorded tallies pass (R08.T14.d.4; lean, 2026-11-01): the deck's rule
 * ({@link deckGatePasses}), every image passing somewhere and none missing, over the images whose
 * sun stands at or above the horizon ({@link BELOW_THE_HORIZON_IMAGES}).
 */
export function hazeGatePasses(tallies: ReadonlyArray<GateImageTally>): boolean {
  return deckGatePasses(tallies.filter((image) => !BELOW_THE_HORIZON_IMAGES.includes(image.image)));
}

/**
 * The range a Titan-class haze routes over once `titan-class`'s gate passes (R08.T14.d.4; lean,
 * 2026-11-01): the case's channels over `regimeChannels`, the range its gate covers, as
 * {@link THICK_ROUTE_RANGE} is the kept cases'.
 *
 * @remarks
 * Each bound is the case's extreme, rounded outward at its third figure (the test holds them to the
 * case): τ to 13.07 (blue, the tholin's most of it), ω̄ from 0.9143 (blue), ḡ to 0.7332 (red),
 * τ ḡ to 7.849 and τ (1 − ω̄) to 1.120 (blue), and the ground to 0.3. A Mars-class dust lies
 * outside it by its blue alone (ḡ 0.762, ω̄ 0.829), and a Venus-class column by its depth.
 */
export const HAZE_ROUTE_RANGE: ThickRouteRange = {
  mostOpticalDepth: 13.1,
  leastSingleScatteringAlbedo: 0.914,
  mostAsymmetry: 0.734,
  mostGroundAlbedo: 0.3,
  mostForwardDepth: 7.85,
  mostAbsorbedDepth: 1.12,
};

/**
 * The ranges a `thickScattering` world routes over, given `tallies`, the haze gate's recorded
 * verdict (R08.T14.d.4): {@link THICK_ROUTE_RANGE}, and `haze` beside it where the gate passes
 * ({@link hazeGatePasses}).
 */
export function thickRouteRangesOf(
  tallies: ReadonlyArray<GateImageTally>,
  haze: ThickRouteRange = HAZE_ROUTE_RANGE,
): ReadonlyArray<ThickRouteRange> {
  return hazeGatePasses(tallies) ? [THICK_ROUTE_RANGE, haze] : [THICK_ROUTE_RANGE];
}

/**
 * The ranges a `thickScattering` world routes over (R08.T14.d.4): {@link THICK_ROUTE_RANGE} alone,
 * since `titan-class`'s gate misses ({@link HAZE_GATE_TALLIES}). A Titan-class world stays drawn by
 * the analytic term under `ATMOSPHERE: APPROXIMATE` with `THICK_REASON`, its bake unasked. Its
 * opening also waits on the owner's RTX run of a routed Titan's frame against
 * `decision-r08-t11-frame.md`'s 16.67 ms.
 */
export const THICK_ROUTE_RANGES: ReadonlyArray<ThickRouteRange> =
  thickRouteRangesOf(HAZE_GATE_TALLIES);

/**
 * Whether a body routed `regime` is baked as a haze route's bake is (R08.T14.d.4): the channels its
 * lights allow at their own wavelengths, its diffuse streams' curved paths and the horizon's
 * first-order terms (`bake.ts`'s remarks, "Own wavelengths", "Curved streams" and "The horizon's
 * first-order terms"). A `thickScattering` world that `ranges` route outside
 * {@link THICK_ROUTE_RANGE}, by the haze's range, whose gate was baked so; never one
 * {@link THICK_ROUTE_RANGE} holds, whose kept cases' gates were baked without them.
 *
 * @param ranges - The ranges its gates pass over: {@link THICK_ROUTE_RANGES}, or a test's.
 */
export function bakedAsHaze(
  regime: AtmosphereRegime,
  medium: AtmosphereMedium,
  ranges: ReadonlyArray<ThickRouteRange> = THICK_ROUTE_RANGES,
): boolean {
  return thickBakeRouted(regime, medium, ranges) && !withinRouteRange(medium, THICK_ROUTE_RANGE);
}

/**
 * The range a gas envelope routes over once `giant-deck`'s envelope variant passes (R08.T16.a.2;
 * lean, 2026-11-01): the channels of the case's medium above its envelope floor, the range its
 * gate covers, as {@link THICK_ROUTE_RANGE} is its passing cases'.
 *
 * @remarks
 * Each bound is the case's extreme, rounded outward at its third figure (the test holds them to the
 * case): τ to 10.04 (blue), ḡ to 0.7323 (red), τ ḡ to 7.331 (red); its deck is conservative, so
 * ω̄ is 1 and τ (1 − ω̄) 0, and an envelope's floor is black. A Jupiter-class envelope lies outside
 * it, its chromophore absorbing (the hand Jupiter's τ (1 − ω̄) 0.065–0.49 and blue τ 17.0 above
 * its floor), and stays `ATMOSPHERE: APPROXIMATE` until a gate covering it passes.
 */
export const ENVELOPE_ROUTE_RANGE: ThickRouteRange = {
  mostOpticalDepth: 10.1,
  leastSingleScatteringAlbedo: 1,
  mostAsymmetry: 0.733,
  mostGroundAlbedo: 0,
  mostForwardDepth: 7.34,
  mostAbsorbedDepth: 0,
};

/**
 * The deck routes `tallies`, an envelope gate's recorded verdict, open (R08.T16.a.2): the envelope's
 * `range` where the gate passes ({@link deckGatePasses}), and `null`, none, where it does not; a
 * deck over a surface routes on no gate yet (lean, 2026-11-01, R08.T15.b.2's: once both of
 * R08.T15's gates pass, as the plan says).
 */
export function deckRouteRangeOf(
  tallies: ReadonlyArray<GateImageTally>,
  range: ThickRouteRange = ENVELOPE_ROUTE_RANGE,
): DeckRouteRange | null {
  return deckGatePasses(tallies) ? { ground: null, envelope: range } : null;
}

/**
 * The range the cloud-deck split's passing gates cover (R08.T15.b.2, R08.T16.a.2): none yet. No
 * deck case passes: the Venus-class cases' scalar gate refuses nearly every value below the deck at
 * their references' 1,024 samples, `giant-deck` below its split misses 42 of its twilight values
 * (s90, its plane-parallel table dark there), and its envelope variant misses 3 of its 30 values
 * on the disc ({@link ENVELOPE_GATE_TALLIES}). A `cloudDeck` world stays drawn by the analytic term
 * under `ATMOSPHERE: APPROXIMATE`, its deck's bake unasked. Routing a gas envelope changes its draw
 * path, so its opening also waits on the owner's RTX run of its frame against
 * `decision-r08-t11-frame.md`'s 16.67 ms.
 */
export const DECK_ROUTE_RANGE: DeckRouteRange | null = deckRouteRangeOf(ENVELOPE_GATE_TALLIES);

/**
 * Whether a body routed `regime` takes its deck's bake (Design note 9's split; the atmosphere
 * cache's deck path, `thick/deck.ts`'s `bakeDeck`): only `cloudDeck`, at a split inside the medium,
 * and only where `range` opens its floor and the medium, its whole over a surface and its part above
 * the split for an envelope ({@link DeckRouteRange}), lies within that floor's range. `thin` and
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
      const splitM = regime.deckAltitudeM;
      routed =
        splitM > 0 &&
        splitM < medium.topHeightM &&
        bounds !== null &&
        withinRouteRange(envelope ? upperMediumOf(medium, splitM) : medium, bounds);
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
