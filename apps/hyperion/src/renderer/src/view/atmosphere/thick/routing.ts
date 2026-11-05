/**
 * Which thick worlds the atmosphere cache routes to their bake (plan R08, R08.T14.c and
 * R08.T14.d.4; Design note 9's routing): a regime routes to its bake only where its gate passes,
 * R08.T14.d's for `thickScattering` and R08.T15's for `cloudDeck`, and since R08.T14.d.4 a
 * `thickScattering` world only within the range its passing gates cover ({@link THICK_ROUTE_RANGE};
 * main's rule on routing by the depth range the passing cases cover,
 * `decision-r08-t13c2-classes.md` §6). Elsewhere a thick world is drawn by the analytic term under
 * `ATMOSPHERE: APPROXIMATE`. A Titan-class haze's range opens on `titan-class`'s gate alone, its
 * recorded verdict passing with its named exceptions ({@link HAZE_GATE_TALLIES},
 * {@link HAZE_GATE_EXCEPTIONS}, R08.T14.d.4), which since R08.T17's haze rising aloft it does not:
 * a Titan-class world is drawn by the analytic term. A
 * `cloudDeck` world routes to its deck's bake only within the range its
 * passing deck gates cover ({@link DECK_ROUTE_RANGE}, R08.T15.b.2): a gas envelope's, open on
 * `giant-deck`'s envelope variant alone on the `closed` giant route, whose recorded verdict passes
 * since the march steps a ray onto an envelope's floor from the camera
 * ({@link ENVELOPE_GATE_TALLIES}, R08.T16.a.2), and widened to the cool giants'
 * ({@link COOL_GIANT_ROUTE_RANGE}) on the `envelope` route, both settings', once `jupiter-envelope`'s
 * gate passes with its named exceptions ({@link giantDeckRouteRange}); a deck over
 * a surface's, open on R08.T15's two gates, `giant-deck`'s above its split and the Venus-class
 * cases' ({@link ABOVE_DECK_GATE_TALLIES}, {@link SURFACE_DECK_GATE_TALLIES}). Tests route by
 * injection (`AtmosphereCache`'s `routes`).
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
import { type AtmosphereRegime, type ChannelRegime, regimeChannels } from "./regime";
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
 * own, {@link HAZE_ROUTE_RANGE}, which its gate opens with its named exceptions (closed since R08.T17). Neither the absorbers' depth, which
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
  return regimeChannels(medium).every((channel) => channelWithinRouteRange(channel, range));
}

/** Whether one channel, as `regime.ts`'s `regimeChannels` gives it, lies within `range`. */
export function channelWithinRouteRange(
  channel: Pick<
    ChannelRegime,
    "opticalDepth" | "singleScatteringAlbedo" | "asymmetry" | "groundAlbedo"
  >,
  range: ThickRouteRange,
): boolean {
  return (
    channel.opticalDepth <= range.mostOpticalDepth &&
    channel.singleScatteringAlbedo >= range.leastSingleScatteringAlbedo &&
    channel.asymmetry <= range.mostAsymmetry &&
    channel.groundAlbedo <= range.mostGroundAlbedo &&
    channel.opticalDepth * channel.asymmetry <= range.mostForwardDepth &&
    channel.opticalDepth * (1 - channel.singleScatteringAlbedo) <= range.mostAbsorbedDepth
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
 *   R08.T15's gates pass: above the deck, T15.a.2's `giant-deck`, and the Venus-class cases above
 *   and below it, T15.b.2's ({@link SURFACE_DECK_GATE_TALLIES});
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
 * reference (R08.T12.h's) all 30 of its values pass, the disc at p85 in red the closest, 0.93 of
 * the reference at 0.96 of the gate's edge, so the route opens ({@link DECK_ROUTE_RANGE}). It
 * missed three on the disc at p75 and p85 (red from 0.85) until the march stepped a ray onto an
 * envelope's floor from the camera (`thick/deckRead.ts`, "An envelope's floor").
 */
export const ENVELOPE_GATE_TALLIES: ReadonlyArray<GateImageTally> = [
  { image: "disc-p0", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p30", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p60", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p75", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p85", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "limb", comparisons: 15, passes: 15, misses: 0, refusals: 0 },
];

/**
 * `jupiter-envelope`'s recorded verdict by image (R08.T16.a.2): `vectorGate.json`'s
 * `jupiterEnvelope.tallies.images`, the hand Jupiter's medium split at its envelope floor through
 * the route an envelope takes, its chromophore absorbing above the floor, against the vector I of
 * its reference at seed 15, held to the file by the test. Retraced on the hand Jupiter's NH₃ haze
 * and chromophore under the sim's rules at generator version 30 (`test/giantFixtures.ts`'s
 * `coldGiantHazes`, 2026-11-05), of its 24 values 2 pass, none misses and 22 are refused:
 *
 * - the twin reads 0.98–1.10 of the reference on the disc, its blue the highest (1.08–1.10 at
 *   p0–p60), and 1.00–1.05 on the limb; at 27's media 4 missed (the disc's red at p75, its blue at
 *   p60, the limb's blue at 0.3 H and 1 H);
 * - the refusals are the vector I's but for the limb's scalar at 0.3 H and 1 H: at the counts of
 *   the earlier pilots its σ passes T ÷ 8 almost everywhere (`refusals.json`).
 *
 * It passes on its 2 judged values, its four images refused throughout passed over as
 * {@link deckGatePasses} passes them ({@link chromophoreGatePasses}), and so opens a cool giant's
 * range ({@link COOL_GIANT_ROUTE_RANGE}) on the `envelope` setting. The analytic path, the one a
 * closed envelope is drawn by, reads 0.61–0.89 of the same reference, but for the disc's green at
 * p85 (0.47, the reference's σ there 24% of it). A re-pilot of the vector I's counts (ruling 8) is
 * pending.
 */
export const CHROMOPHORE_GATE_TALLIES: ReadonlyArray<GateImageTally> = [
  { image: "disc-p0", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
  { image: "disc-p30", comparisons: 3, passes: 1, misses: 0, refusals: 2 },
  { image: "disc-p60", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
  { image: "disc-p75", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
  { image: "disc-p85", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
  { image: "limb", comparisons: 9, passes: 1, misses: 0, refusals: 8 },
];

/**
 * `jupiter-envelope`'s misses through the envelope's route (R08.T16.a.2): `vectorGate.json`'s
 * `jupiterEnvelope.geometries`, every channel that misses, held to the file by the test.
 */
export const CHROMOPHORE_GATE_MISSES: ReadonlyArray<GateMiss> = [];

/**
 * The chromophore gate's named exceptions (R08.T16.a.2; main's ruling on research-leans-43, item
 * 1, 2026-11-05), each held to its recorded miss's bound as the haze gate's are
 * ({@link HAZE_GATE_EXCEPTIONS}): none, since the verdict retraced on generator version 30's media
 * misses nowhere. `jupiter-envelope`'s four misses on its 26 medium (bounds 2.17, 1.55, 1.34 and
 * 2.32) went with that medium. A miss to be named must read no closer to the reference through the
 * analytic path than through the route. A new miss, or a named one past its bound, closes the cool
 * giants' range ({@link coolGiantRouteRangeOf}).
 */
export const CHROMOPHORE_GATE_EXCEPTIONS: ReadonlyArray<GateException> = [];

/**
 * Whether a deck gate's recorded tallies pass (R08.T16.a.2; R08.T15.b.2, Titan's rule, main's ruling
 * of 2026-11-02): no image misses, and every image that holds a value not refused passes, so that a
 * route opens only on passes (research-leans-16, item 5, condition 2). An image refused throughout
 * is passed over, its gate's record naming it; a gate that misses anywhere, or is refused
 * throughout, keeps it closed.
 */
export function deckGatePasses(tallies: ReadonlyArray<GateImageTally>): boolean {
  const judged = tallies.filter((image) => image.refusals < image.comparisons);
  return (
    judged.length > 0 &&
    tallies.every((image) => image.misses === 0) &&
    judged.every((image) => image.passes > 0)
  );
}

/**
 * `giant-deck`'s recorded verdict by image above its split (R08.T15.a.2): `vectorGate.json`'s
 * `deck.routed.images`, the upper medium over the deck, through the route a deck takes against the
 * vector I, held to the file by the test. All 30 of its values pass, the limb at 10 H in red the
 * closest, 0.79 of the gate's edge: the gate above a deck that a deck over a surface routes on with
 * {@link SURFACE_DECK_GATE_TALLIES} below it (R08.T15.b.2, the plan's "once both gates pass").
 */
export const ABOVE_DECK_GATE_TALLIES: ReadonlyArray<GateImageTally> = [
  { image: "disc-p0", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p30", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p60", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p75", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "disc-p85", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "limb", comparisons: 15, passes: 15, misses: 0, refusals: 0 },
];

/**
 * The Venus-class cases' recorded verdicts by image through their deck (R08.T15.b.2):
 * `vectorGate.json`'s `deck.venusClass.cases.<case>.routed.images`, each case split at its deck's
 * top over its ground, the uncorrected twin against the scalar I, held to the file by the test.
 * `venus-92bar` passes 70 of its 360 values and `venus-58bar` 69, and neither misses one: their
 * night under the deck (s95), one value at the horizon's sun (s90) and the limb above the top. Every
 * value of `ground-s0`, `ground-s30`, `ground-s60`, `ground-s80`, the disc's five images and
 * `aerial` is refused, noise past the references' σ at their 1,024 samples, so that
 * {@link deckGatePasses} passes those images over, drawn unlabelled by day once the deck routes
 * (main's ruling of 2026-11-02, research-rm4-rm5-close-out §1.1), where the haze's gate passes over
 * images by their sun and labels them ({@link HAZE_UNJUDGED_IMAGES}). The analytic path reads worse
 * on the same values (a probe of 2026-11-02, not kept): it misses 63 and 43 of the night's (its
 * twilight's light where the references read none), and `venus-92bar`'s ground's flux beneath the
 * deck at the probe's sun (`flux-lsfr`) is 9 × 10⁻⁵ of the reference's in red, 2 × 10⁻⁶ in green
 * and 3 × 10⁻¹⁹ in blue, where the route's reads −1.5%, −0.8% and +19.9% of them (`venus-92bar`) and −2.0%, +3.1%
 * and +5.1% (`venus-58bar`), every one refused.
 */
export const SURFACE_DECK_GATE_TALLIES: Readonly<
  Record<"venus-92bar" | "venus-58bar", ReadonlyArray<GateImageTally>>
> = {
  "venus-92bar": [
    { image: "ground-s0", comparisons: 12, passes: 0, misses: 0, refusals: 12 },
    { image: "ground-s30", comparisons: 60, passes: 0, misses: 0, refusals: 60 },
    { image: "ground-s60", comparisons: 60, passes: 0, misses: 0, refusals: 60 },
    { image: "ground-s80", comparisons: 63, passes: 0, misses: 0, refusals: 63 },
    { image: "ground-s90", comparisons: 63, passes: 1, misses: 0, refusals: 62 },
    { image: "ground-s95", comparisons: 63, passes: 63, misses: 0, refusals: 0 },
    { image: "disc-p0", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "disc-p30", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "disc-p60", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "disc-p75", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "disc-p85", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "limb", comparisons: 15, passes: 6, misses: 0, refusals: 9 },
    { image: "aerial", comparisons: 9, passes: 0, misses: 0, refusals: 9 },
  ],
  "venus-58bar": [
    { image: "ground-s0", comparisons: 12, passes: 0, misses: 0, refusals: 12 },
    { image: "ground-s30", comparisons: 60, passes: 0, misses: 0, refusals: 60 },
    { image: "ground-s60", comparisons: 60, passes: 0, misses: 0, refusals: 60 },
    { image: "ground-s80", comparisons: 63, passes: 0, misses: 0, refusals: 63 },
    { image: "ground-s90", comparisons: 63, passes: 1, misses: 0, refusals: 62 },
    { image: "ground-s95", comparisons: 63, passes: 62, misses: 0, refusals: 1 },
    { image: "disc-p0", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "disc-p30", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "disc-p60", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "disc-p75", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "disc-p85", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
    { image: "limb", comparisons: 15, passes: 6, misses: 0, refusals: 9 },
    { image: "aerial", comparisons: 9, passes: 0, misses: 0, refusals: 9 },
  ],
};

/**
 * `titan-class`'s recorded verdict by image through its route (R08.T14.d.4): `vectorGate.json`'s
 * `titanClass.routed.images`, its whole column baked at m ≤ 2 with its correction as a haze route
 * bakes it, its green and blue at their own wavelengths and its diffuse streams on their curved
 * paths (`bake.ts`'s "Own wavelengths" and "Curved streams"), against the reference re-traced for
 * R08.T17's haze rising aloft, its vector I, held to the file by the test. 68 of its 360 values
 * pass, 117 miss and 175 are refused. On the images it judges 22 miss, all but three red: the sky
 * at s30 (v30, a90) and at s60 (17, 0.86–0.93 of the reference, the route dim under a low sun),
 * the disc at p30, p60 and p85 (1.10, 1.04 and 0.72), and the aerial view at 32 km; every one but
 * the disc at p60 unlisted, so the gate closes (R08.T17's rule E).
 */
export const HAZE_GATE_TALLIES: ReadonlyArray<GateImageTally> = [
  { image: "ground-s0", comparisons: 12, passes: 8, misses: 0, refusals: 4 },
  { image: "ground-s30", comparisons: 60, passes: 38, misses: 1, refusals: 21 },
  { image: "ground-s60", comparisons: 60, passes: 13, misses: 17, refusals: 30 },
  { image: "ground-s80", comparisons: 63, passes: 0, misses: 33, refusals: 30 },
  { image: "ground-s90", comparisons: 63, passes: 0, misses: 28, refusals: 35 },
  { image: "ground-s95", comparisons: 63, passes: 0, misses: 34, refusals: 29 },
  { image: "disc-p0", comparisons: 3, passes: 1, misses: 0, refusals: 2 },
  { image: "disc-p30", comparisons: 3, passes: 0, misses: 1, refusals: 2 },
  { image: "disc-p60", comparisons: 3, passes: 0, misses: 1, refusals: 2 },
  { image: "disc-p75", comparisons: 3, passes: 2, misses: 0, refusals: 1 },
  { image: "disc-p85", comparisons: 3, passes: 0, misses: 1, refusals: 2 },
  { image: "limb", comparisons: 15, passes: 5, misses: 0, refusals: 10 },
  { image: "aerial", comparisons: 9, passes: 1, misses: 1, refusals: 7 },
];

/**
 * One value a gate's recorded verdict misses (R08.T14.d.4): its image, the value as
 * `geometry/channel`, and its |twin − ref| ÷ (T + 4σ), in units of the gate's edge.
 */
export interface GateMiss {
  readonly image: string;
  readonly value: string;
  readonly ratio: number;
}

/**
 * `titan-class`'s misses through its route on the images its gate judges (R08.T14.d.4):
 * `vectorGate.json`'s `titanClass.geometries`, every channel that misses outside
 * {@link HAZE_UNJUDGED_IMAGES}, held to the file by the test.
 */
export const HAZE_GATE_MISSES: ReadonlyArray<GateMiss> = [
  { image: "ground-s30", value: "ground-s30-v30-a90/r", ratio: 1.194134 },
  { image: "ground-s60", value: "ground-s60-v30-a0/r", ratio: 1.703285 },
  { image: "ground-s60", value: "ground-s60-v30-a0/g", ratio: 1.034178 },
  { image: "ground-s60", value: "ground-s60-v30-a45/r", ratio: 1.110212 },
  { image: "ground-s60", value: "ground-s60-v60-a45/r", ratio: 1.566796 },
  { image: "ground-s60", value: "ground-s60-v60-a90/r", ratio: 1.396606 },
  { image: "ground-s60", value: "ground-s60-v60-a135/r", ratio: 1.041316 },
  { image: "ground-s60", value: "ground-s60-v75-a0/r", ratio: 1.992406 },
  { image: "ground-s60", value: "ground-s60-v75-a45/r", ratio: 1.695752 },
  { image: "ground-s60", value: "ground-s60-v75-a90/r", ratio: 1.361608 },
  { image: "ground-s60", value: "ground-s60-v75-a135/r", ratio: 1.105267 },
  { image: "ground-s60", value: "ground-s60-v85-a0/r", ratio: 1.798349 },
  { image: "ground-s60", value: "ground-s60-v85-a45/r", ratio: 1.621675 },
  { image: "ground-s60", value: "ground-s60-v85-a90/r", ratio: 1.512734 },
  { image: "ground-s60", value: "ground-s60-v85-a90/g", ratio: 1.047713 },
  { image: "ground-s60", value: "ground-s60-v85-a135/r", ratio: 1.198912 },
  { image: "ground-s60", value: "ground-s60-v85-a180/r", ratio: 1.169705 },
  { image: "ground-s60", value: "ground-s60-v85-a180/g", ratio: 1.094049 },
  { image: "disc-p30", value: "disc-p30/r", ratio: 1.319177 },
  { image: "disc-p60", value: "disc-p60/r", ratio: 1.036881 },
  { image: "disc-p85", value: "disc-p85/r", ratio: 3.880784 },
  { image: "aerial", value: "aerial-32km/r", ratio: 1.148176 },
];

/**
 * A miss a gate passes with, named and bounded (R08.T14.d.4): the value, as {@link GateMiss}'s;
 * its bound, its recorded |twin − ref| ÷ (T + 4σ) rounded outward at its third figure; and its
 * measured cause.
 */
export interface GateException {
  readonly value: string;
  readonly bound: number;
  readonly cause: string;
}

/**
 * The haze gate's named exceptions (R08.T14.d.4; main's ruling (1) of 2026-11-01): `titan-class`'s
 * misses on the images it judges that it would pass with, each held to its bound (one since R08.T17's
 * re-traced reference, whose other misses close the route), as Earth's route passed with its
 * named misses (research-leans-4 §3). Each reads no closer to the reference through the analytic
 * path, the one an unrouted world is drawn by, than through the route, judged per image in the
 * gate's edge units (main's reading of ruling (1), research-leans-28 §4: a pixel takes its three
 * channels from one path), as `vectorGate.json`'s `titanClass.worstByGeometry` records and the
 * test holds. A new miss, or one of these past its bound, closes the route
 * ({@link hazeGatePasses}).
 */
export const HAZE_GATE_EXCEPTIONS: ReadonlyArray<GateException> = [
  {
    value: "disc-p60/r",
    bound: 1.04,
    cause:
      "mode-free and red alone: the bins' red conversion, its methane's channel curve at the floor (T14.d.1's record, not measured again with R08.T17's haze); the image's worst channel through the analytic path, its red at 2.04 of the edge, is larger than through the route, 1.037; R08.T17's re-traced reference passes ground-s30-v30-a45/r and a180/r, whose exceptions went with it",
  },
];

/**
 * The haze gate's images it does not judge (R08.T14.d.4; main's ruling of 2026-11-01): those whose
 * sun stands at most {@link LOW_SUN_SINE} over the horizon, s80 and s90, where a body baked as a
 * haze reads `ATMOSPHERE: APPROXIMATE` with {@link LOW_SUN_REASON}, and s95 below it
 * ({@link TWILIGHT_REASON}), past the pseudo-spherical model's limit (`reference/cases.ts`'s
 * sphere set).
 */
export const HAZE_UNJUDGED_IMAGES: ReadonlyArray<string> = [
  "ground-s80",
  "ground-s90",
  "ground-s95",
];

/**
 * Whether a haze gate's recorded verdict passes (R08.T14.d.4; main's ruling of 2026-11-01): over the
 * images it judges (all but {@link HAZE_UNJUDGED_IMAGES}, which a haze labels), every image passing
 * somewhere, refused throughout or not (where {@link deckGatePasses} passes an image refused
 * throughout over), and every miss there one of `exceptions` within its bound. `misses` must account for every judged image's tally of misses,
 * so that a miss the list leaves out closes the route.
 *
 * @param misses - The verdict's misses ({@link HAZE_GATE_MISSES}); those on the images it does not
 *   judge are passed over.
 * @param exceptions - The named exceptions: {@link HAZE_GATE_EXCEPTIONS}, or a test's.
 */
export function hazeGatePasses(
  tallies: ReadonlyArray<GateImageTally>,
  misses: ReadonlyArray<GateMiss>,
  exceptions: ReadonlyArray<GateException> = HAZE_GATE_EXCEPTIONS,
): boolean {
  return passesWithExceptions(tallies, misses, exceptions, HAZE_UNJUDGED_IMAGES);
}

/**
 * Whether the chromophore gate's recorded verdict passes (R08.T16.a.2; main's ruling on
 * research-leans-43, 2026-11-05), as {@link hazeGatePasses} reads the haze's, but with an image
 * refused throughout passed over, as {@link deckGatePasses} passes it: every other image passing
 * somewhere, and every miss one of `exceptions` within its bound, `misses` accounting for every
 * image's tally of them. A verdict refused throughout fails.
 *
 * @param misses - The verdict's misses ({@link CHROMOPHORE_GATE_MISSES}).
 * @param exceptions - The named exceptions: {@link CHROMOPHORE_GATE_EXCEPTIONS}, or a test's.
 */
export function chromophoreGatePasses(
  tallies: ReadonlyArray<GateImageTally>,
  misses: ReadonlyArray<GateMiss>,
  exceptions: ReadonlyArray<GateException> = CHROMOPHORE_GATE_EXCEPTIONS,
): boolean {
  const refusedThroughout = tallies
    .filter((image) => image.refusals === image.comparisons)
    .map((image) => image.image);
  return passesWithExceptions(tallies, misses, exceptions, refusedThroughout);
}

/**
 * Whether a gate's recorded verdict passes with its named exceptions over the images it judges, all
 * but `unjudged` ({@link hazeGatePasses}' remarks).
 */
function passesWithExceptions(
  tallies: ReadonlyArray<GateImageTally>,
  misses: ReadonlyArray<GateMiss>,
  exceptions: ReadonlyArray<GateException>,
  unjudged: ReadonlyArray<string>,
): boolean {
  const judged = tallies.filter((image) => !unjudged.includes(image.image));
  const judgedMisses = misses.filter((miss) => !unjudged.includes(miss.image));
  return (
    judged.length > 0 &&
    judged.every(
      (image) =>
        image.passes > 0 &&
        judgedMisses.filter((miss) => miss.image === image.image).length === image.misses,
    ) &&
    judgedMisses.every((miss) =>
      exceptions.some(
        (exception) => exception.value === miss.value && miss.ratio <= exception.bound,
      ),
    )
  );
}

/**
 * The range a Titan-class haze routes over, `titan-class`'s gate passing (R08.T14.d.4; lean,
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
 * The range a decked thin air routes over to its bake, its gate passing (plan R08,
 * fix-r08-decked-air-opacity; lean, 2026-11-05): the channels of the served Earth-like
 * `42002cae00000004.0300` (generator version 28; `test/servedDeckedWorld.json`) as the client draws
 * it, over the check lane's ground of 0.1, about its drawn ground's, the range its gate covers, as
 * {@link HAZE_ROUTE_RANGE} is `titan-class`'s. Its water deck, τ(550) 1.72 under a cloud fraction of
 * 0.67, is drawn uniform at 1.15, under the deck split's τ 10 and its cover of 1 (`regime.ts`'s
 * `CLOUD_DECK_SPLIT_OPTICAL_DEPTH`), so it classifies `thickScattering`, and Ψ_ms drew its veil
 * 41–62% over the tracer's (check-atmosphere-opacity); through the bake it reads 0.97–1.01 of it at
 * disc p0–p30.
 *
 * @remarks
 * Each bound is the case's extreme, rounded outward at its third figure (the test holds them to the
 * case): τ to 1.805 (blue), ω̄ from 0.99999 (red), ḡ to 0.7931 (red, the deck's 45 µm drops), the
 * ground to 0.1, τ ḡ to 1.017 (red) and τ (1 − ω̄) to 1.420 × 10⁻⁵ (red). A Mars-class dust lies
 * outside it by its absorption (ω̄ 0.83–0.97), and a Titan- or Venus-class column by its depth. The
 * served world itself lies outside it by its ground as the atmosphere cache takes it, R07's p q of
 * 0.28–0.35 (`decision-r08-t10a-leans.md` §2), its clouds' brightness among it, which the drawn deck
 * counts again: routed over that ground, its veil on SwiftShader from 2,000 km rose from I/F 0.173
 * to 0.241 in red and its drawn ground's slope through the air fell from 0.56 to 0.20, a paler disc
 * than the analytic path's, and that ground is ungated.
 */
export const DECKED_AIR_ROUTE_RANGE: ThickRouteRange = {
  mostOpticalDepth: 1.81,
  leastSingleScatteringAlbedo: 0.999,
  mostAsymmetry: 0.794,
  mostGroundAlbedo: 0.1,
  mostForwardDepth: 1.02,
  mostAbsorbedDepth: 1.43e-5,
};

/**
 * The served Earth-like world's recorded verdict by image through its route (plan R08,
 * fix-r08-decked-air-opacity): `reference/deckedAirGate.json`'s `routed.images`, over the ground of
 * {@link DECKED_AIR_ROUTE_RANGE}, its whole column baked as a haze route bakes it
 * ({@link bakedAsHaze}), the scalar twin against its trace
 * (`reference/served-decked-orbit.reference.json`) over the disc and the limb from orbit, the
 * images traced at the gate's counts, held to the file by the test. No value misses, the worst
 * 0.51 of the gate's edge (the limb at 0.1 H in red); the disc at p0–p60 is refused throughout, its
 * trace's σ, about 1%, not falling as N^(−1/2) under the deck's forward peak, so
 * {@link deckGatePasses} passes those over, Titan's rule; there the route reads 0.95–1.06 of the
 * trace and the analytic path 1.22–1.56. The analytic path misses 6 values (the record's
 * `analytic`; disc-p85 2.2 times the trace in red). The views from within the air are not traced,
 * and read {@link DECKED_AIR_REASON}.
 */
export const DECKED_AIR_GATE_TALLIES: ReadonlyArray<GateImageTally> = [
  { image: "disc-p0", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
  { image: "disc-p30", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
  { image: "disc-p60", comparisons: 3, passes: 0, misses: 0, refusals: 3 },
  { image: "disc-p75", comparisons: 3, passes: 1, misses: 0, refusals: 2 },
  { image: "disc-p85", comparisons: 3, passes: 3, misses: 0, refusals: 0 },
  { image: "limb", comparisons: 9, passes: 6, misses: 0, refusals: 3 },
];

/**
 * Whether a decked thin air takes its bake (plan R08, fix-r08-decked-air-opacity; the lever, with
 * the way back beside it, as `hillaire.ts`'s `AtmosphereSettings`): `bake`, over
 * {@link DECKED_AIR_ROUTE_RANGE} where its gate passes, or `analytic`, Ψ_ms under
 * `ATMOSPHERE: APPROXIMATE`, the way back.
 */
export type DeckedAirRoute = "bake" | "analytic";

/** {@link DeckedAirRoute}: `bake`; `analytic` is the way back. */
export const DECKED_AIR_ROUTE: DeckedAirRoute = "bake";

/**
 * The ranges a `thickScattering` world routes over, given `tallies` and `misses`, the haze gate's
 * recorded verdict (R08.T14.d.4): {@link THICK_ROUTE_RANGE}, and `haze` beside it where the gate
 * passes ({@link hazeGatePasses}); and since fix-r08-decked-air-opacity `decked` after them where
 * `route` is `bake` and the decked air's gate, `deckedTallies`, passes ({@link deckGatePasses}).
 */
export function thickRouteRangesOf(
  tallies: ReadonlyArray<GateImageTally>,
  misses: ReadonlyArray<GateMiss>,
  haze: ThickRouteRange = HAZE_ROUTE_RANGE,
  route: DeckedAirRoute = "analytic",
  deckedTallies: ReadonlyArray<GateImageTally> = DECKED_AIR_GATE_TALLIES,
  decked: ThickRouteRange = DECKED_AIR_ROUTE_RANGE,
): ReadonlyArray<ThickRouteRange> {
  const opened = hazeGatePasses(tallies, misses) ? [THICK_ROUTE_RANGE, haze] : [THICK_ROUTE_RANGE];
  return route === "bake" && deckGatePasses(deckedTallies) ? [...opened, decked] : opened;
}

/**
 * The ranges a `thickScattering` world routes over (R08.T14.d.4): {@link THICK_ROUTE_RANGE} alone
 * since R08.T17, `titan-class`'s gate missing past its named exceptions ({@link HAZE_GATE_TALLIES},
 * {@link HAZE_GATE_EXCEPTIONS}), and {@link HAZE_ROUTE_RANGE} beside it were the gate to pass. A
 * Titan-class world is drawn by the analytic term and reads `ATMOSPHERE: APPROXIMATE`; baked as a
 * haze ({@link bakedAsHaze}) on a passing gate it read `ATMOSPHERE: COMPUTING`, then nothing, but
 * for its low sun ({@link LOW_SUN_REASON}).
 * And {@link DECKED_AIR_ROUTE_RANGE} under {@link DECKED_AIR_ROUTE}, since the decked air's gate
 * passes over the orbit's images ({@link DECKED_AIR_GATE_TALLIES}): baked as a haze, it reads
 * nothing from orbit and {@link DECKED_AIR_REASON} from within its air; its routed frame against
 * 16.67 ms is pending the owner's RTX run.
 */
export const THICK_ROUTE_RANGES: ReadonlyArray<ThickRouteRange> = thickRouteRangesOf(
  HAZE_GATE_TALLIES,
  HAZE_GATE_MISSES,
  HAZE_ROUTE_RANGE,
  DECKED_AIR_ROUTE,
);

/**
 * Whether a body routed `regime` takes its bake by `decked` alone (plan R08,
 * fix-r08-decked-air-opacity): routed by `ranges`, `decked` among them, and within neither
 * {@link THICK_ROUTE_RANGE} nor {@link HAZE_ROUTE_RANGE}, whose gates judge the views from within
 * the air too.
 *
 * @param ranges - The ranges its gates pass over: {@link THICK_ROUTE_RANGES}, or a test's.
 * @param decked - The decked air's range among them: {@link DECKED_AIR_ROUTE_RANGE}, or a test's.
 */
export function deckedAirRouted(
  regime: AtmosphereRegime,
  medium: AtmosphereMedium,
  ranges: ReadonlyArray<ThickRouteRange> = THICK_ROUTE_RANGES,
  decked: ThickRouteRange = DECKED_AIR_ROUTE_RANGE,
): boolean {
  return (
    thickBakeRouted(regime, medium, ranges) &&
    ranges.includes(decked) &&
    !withinRouteRange(medium, THICK_ROUTE_RANGE) &&
    !withinRouteRange(medium, HAZE_ROUTE_RANGE) &&
    withinRouteRange(medium, decked)
  );
}

/**
 * Whether a body routed `regime` is baked as a haze route's bake is (R08.T14.d.4): the channels its
 * lights allow at their own wavelengths and its diffuse streams' curved paths (`bake.ts`'s remarks,
 * "Own wavelengths" and "Curved streams"), never the horizon's first-order terms (main's ruling of
 * 2026-11-01: they pass s90 by a cancellation, research-leans-28 §1). A `thickScattering` world
 * that `ranges` route outside
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
 * it, its chromophore absorbing (the hand Jupiter's τ (1 − ω̄) 0.016–0.12, its blue τ 10.3 and red
 * ḡ 0.77 above its floor, under its NH₃ haze and chromophore of generator version 30's rules), and
 * so do the hand Saturn, the sim's own Saturn's gases and decks under the same haze (blue τ 12.3
 * above its floor, its chromophore's τ (1 − ω̄) 0.065–0.49), and the hand ice giant (blue τ 10.29,
 * red ḡ 0.7340; `routing.test.ts`). On the `closed` giant route each stays
 * `ATMOSPHERE: APPROXIMATE`, and on the `envelope` route each routes on
 * {@link COOL_GIANT_ROUTE_RANGE}.
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
 * The range a cool giant's envelope routes over on the `envelope` giant route (R08.T16.a.2;
 * research-leans-43's lean, kept by main, 2026-11-05), once the chromophore gate passes with its
 * named exceptions ({@link CHROMOPHORE_GATE_EXCEPTIONS}): the channels above their floors of the
 * served cool giants and the cases, a box over {@link ENVELOPE_ROUTE_RANGE}, so that the deck bake's
 * J takes Ψ_ms's place. On `jupiter-envelope`'s 26 medium the analytic path's disc ran green +14%
 * and blue +35% over the reference at p0. On its 30 medium it reads 0.61–0.89 of the reference and
 * the route 0.95–1.10, but for the disc's green at p85 (0.47 and 0.76, the reference's σ there 24%
 * of it).
 *
 * @remarks
 * Each bound is the extreme, rounded outward at its third figure (the test holds them), of
 * `giant-deck`'s and `jupiter-envelope`'s cases, the hand and the generated Jupiter, the hand
 * Saturn and ice giant, and the `Dev Fixture`'s 31 cool giants at generator version 28
 * (`fixtures/served-giants.json`, every giant with an `ok` envelope of 192 systems about the one
 * `41ffecb1ffc00013.0400` stands in): τ to 25.54 (a clear giant's blue, its floor the gas's
 * Rayleigh alone), ω̄ from 0.9602 and τ (1 − ω̄) to 0.4901 (the hand Saturn's blue under generator
 * version 30's chromophore, at the Great Red Spot's column), and ḡ to 0.7712 and τ ḡ to 7.542 (the
 * hand Jupiter's red under its NH₃ haze), over a black floor. Past `jupiter-envelope`'s own
 * channels (`vectorGate.json`'s: τ to 10.31, ω̄ from 0.9885, ḡ to 0.7712) and `giant-deck`'s (ḡ
 * to 0.7323, τ ḡ to 7.331) it is main's ruling, not a gate (lean, 2026-11-05): such a giant routed
 * is drawn without `THICK_REASON`, though both gates judge the disc and the limb alone.
 * `saturn-envelope`, retraced on generator version 30's media, covers τ (1 − ω̄) to 0.49: its
 * route misses 2 of its 24 values, the disc's blue at p0 and p60 (1.39 and 1.58 of the edge), where
 * the analytic path misses 10 and reads 3.2 and 3.7 of the edge on those images (not on file). A
 * served giant absorbing past it (ω̄ 0.81–0.92 and τ (1 − ω̄) 0.69–2.0, two giants at
 * 983 and 1,115 K) lies outside it, and a self-luminous one is refused by name
 * ({@link giantDeckRouteRange}).
 */
export const COOL_GIANT_ROUTE_RANGE: ThickRouteRange = {
  mostOpticalDepth: 25.6,
  leastSingleScatteringAlbedo: 0.96,
  mostAsymmetry: 0.772,
  mostGroundAlbedo: 0,
  mostForwardDepth: 7.55,
  mostAbsorbedDepth: 0.491,
};

/**
 * The range a deck over a surface routes over once the Venus-class cases' gates pass (R08.T15.b.2;
 * lean, 2026-11-02): the channels of both cases' whole media, the range their gates cover, as
 * {@link HAZE_ROUTE_RANGE} is `titan-class`'s.
 *
 * @remarks
 * Each bound is the cases' extreme, rounded outward at its third figure (the test holds them to the
 * cases): τ to 76.70 (`venus-92bar`'s blue, its Rayleigh the most of it), ω̄ from 0.9987
 * (`venus-58bar`'s blue), ḡ to 0.6660 (`venus-58bar`'s red), the ground to 0.15, τ ḡ to 26.58
 * (`venus-58bar`'s green) and τ (1 − ω̄) to 0.07805 (either case's blue, the same SO₂ and deck
 * absorber in both). Out of it
 * stand `giant-deck`'s NH₃ ice (ḡ 0.71–0.73 over a black ground), whose own gate below its split
 * misses 42 of its values at the horizon's sun, and `titan-class`'s haze (ḡ to 0.73).
 */
export const SURFACE_DECK_ROUTE_RANGE: ThickRouteRange = {
  mostOpticalDepth: 76.8,
  leastSingleScatteringAlbedo: 0.998,
  mostAsymmetry: 0.667,
  mostGroundAlbedo: 0.15,
  mostForwardDepth: 26.6,
  mostAbsorbedDepth: 0.0781,
};

/**
 * The deck routes the recorded verdicts open (R08.T16.a.2, R08.T15.b.2): each floor's range where its
 * gates pass ({@link deckGatePasses}), an envelope's {@link ENVELOPE_ROUTE_RANGE} on `envelope`, an
 * envelope gate's, and a surface's {@link SURFACE_DECK_ROUTE_RANGE} on every one of `surface`, the
 * gate above a deck and the Venus-class cases' below and above it; `null` for a floor whose gates do
 * not pass, and `null`, none, where neither does.
 */
export function deckRouteRangeOf(
  envelope: ReadonlyArray<GateImageTally>,
  surface: ReadonlyArray<ReadonlyArray<GateImageTally>>,
): DeckRouteRange | null {
  const ground =
    surface.length > 0 && surface.every(deckGatePasses) ? SURFACE_DECK_ROUTE_RANGE : null;
  const opened = deckGatePasses(envelope) ? ENVELOPE_ROUTE_RANGE : null;
  return ground === null && opened === null ? null : { ground, envelope: opened };
}

/**
 * The range the cloud-deck split's passing gates cover (R08.T15.b.2, R08.T16.a.2): a gas
 * envelope's, {@link ENVELOPE_ROUTE_RANGE}, since `giant-deck`'s envelope variant passes
 * ({@link ENVELOPE_GATE_TALLIES}); a deck over a surface's, {@link SURFACE_DECK_ROUTE_RANGE}, since
 * R08.T15's gates both pass, `giant-deck`'s above its split ({@link ABOVE_DECK_GATE_TALLIES}) and the
 * Venus-class cases' with their refused images passed over ({@link SURFACE_DECK_GATE_TALLIES}). A camera beneath a routed deck over a surface reads its low
 * sun as past the gates' range ({@link DECK_LOWEST_SUN_SINE}). No served giant tried lies within
 * the envelope's range, the hand and the generated Jupiter and Saturn by their chromophore, the ice
 * giant by its blue τ and red ḡ, so none routes on it: the `closed` giant route's, which the
 * `envelope` route widens to {@link COOL_GIANT_ROUTE_RANGE} ({@link giantDeckRouteRange}). A
 * routed deck's frame waits on the owner's RTX run against `decision-r08-t11-frame.md`'s 16.67 ms,
 * an item on RM4's close list, not a condition of the route (main's ruling of 2026-11-02).
 */
export const DECK_ROUTE_RANGE: DeckRouteRange | null = deckRouteRangeOf(ENVELOPE_GATE_TALLIES, [
  ABOVE_DECK_GATE_TALLIES,
  ...Object.values(SURFACE_DECK_GATE_TALLIES),
]);

/**
 * Whether a body routed `regime` takes its deck's bake (Design note 9's split; the atmosphere
 * cache's deck path, `thick/deck.ts`'s `bakeDeck`): only `cloudDeck`, at a split inside the medium,
 * and only where `range` opens its floor and the medium, its whole over a surface and its part above
 * the split for an envelope ({@link DeckRouteRange}), lies within that floor's range. `thin` and
 * `thickScattering` take none ({@link thickBakeRouted} routes the latter).
 *
 * @param envelope - Whether the body's cover is a gas envelope's, which splits at its floor.
 * @param range - The ranges its gates pass over: {@link giantDeckRouteRange}'s for the view's giant
 *   route, as the atmosphere cache passes them; {@link DECK_ROUTE_RANGE}, the `closed` route's, if
 *   absent; or a test's.
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
 * How a cool giant's envelope is drawn inside 10⁹ m (R08.T16.a.2; research-leans-43's lean, kept by
 * main, 2026-11-05), a field of each quality setting (`settings.ts`'s `AtmosphereViewSettings`):
 * `envelope`, through the envelope's deck route, the deck bake's J in Ψ_ms's place, wherever its
 * medium above the floor lies within {@link COOL_GIANT_ROUTE_RANGE}; `closed`, the way back, the
 * route open on `giant-deck`'s range alone ({@link DECK_ROUTE_RANGE}), every other envelope drawn by
 * the analytic term above its floor under `ATMOSPHERE: APPROXIMATE`.
 */
export type GiantRoute = "closed" | "envelope";

/** The giant route both quality settings take: `envelope` (lean, 2026-11-05). */
export const GIANT_ROUTE: GiantRoute = "envelope";

/**
 * The deck routes the `envelope` giant route opens (R08.T16.a.2): `range`'s, its envelope's widened
 * to {@link COOL_GIANT_ROUTE_RANGE} where `range` opens an envelope's, `giant-deck`'s gate passing,
 * and the chromophore gate's verdict, `tallies` and `misses`, passes with its named exceptions
 * ({@link chromophoreGatePasses}); `range` itself where either does not.
 */
export function coolGiantRouteRangeOf(
  tallies: ReadonlyArray<GateImageTally>,
  misses: ReadonlyArray<GateMiss>,
  range: DeckRouteRange | null = DECK_ROUTE_RANGE,
): DeckRouteRange | null {
  const opened = range !== null && range.envelope !== null;
  return opened && chromophoreGatePasses(tallies, misses)
    ? { ground: range.ground, envelope: COOL_GIANT_ROUTE_RANGE }
    : range;
}

/**
 * The deck routes the `envelope` giant route opens (R08.T16.a.2): {@link DECK_ROUTE_RANGE}'s, a gas
 * envelope's widened to {@link COOL_GIANT_ROUTE_RANGE}, since the chromophore gate passes with its
 * named exceptions ({@link CHROMOPHORE_GATE_TALLIES}, {@link CHROMOPHORE_GATE_EXCEPTIONS}).
 */
export const COOL_GIANT_DECK_ROUTE_RANGE: DeckRouteRange | null = coolGiantRouteRangeOf(
  CHROMOPHORE_GATE_TALLIES,
  CHROMOPHORE_GATE_MISSES,
);

/**
 * The deck routes a body routed `regime` takes on `giantRoute` (R08.T16.a.2), as {@link deckRouted}
 * reads them: {@link COOL_GIANT_DECK_ROUTE_RANGE} on `envelope`, but {@link DECK_ROUTE_RANGE} for a
 * self-luminous body, refused by name: its deck's bake carries no thermal light
 * (`AtmosphereCache.ts`'s `DECK_THERMAL_REASON`) and no gate covers a self-luminous envelope (the
 * `Dev Fixture`'s two, `fixtures/served-giants.json`); {@link DECK_ROUTE_RANGE} on `closed`.
 */
export function giantDeckRouteRange(
  regime: AtmosphereRegime,
  giantRoute: GiantRoute,
): DeckRouteRange | null {
  let range: DeckRouteRange | null;
  switch (giantRoute) {
    case "closed":
      range = DECK_ROUTE_RANGE;
      break;
    case "envelope":
      range = regime.selfLuminous === true ? DECK_ROUTE_RANGE : COOL_GIANT_DECK_ROUTE_RANGE;
      break;
  }
  return range;
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
 * The largest sine of the brightest light's elevation over the camera's local horizontal at which a
 * body baked as a haze ({@link bakedAsHaze}) reads {@link LOW_SUN_REASON}, the camera in its air
 * (R08.T14.d.4; main's ruling of 2026-11-01): `bake.ts`'s `HORIZON_SUN_COSINE`, μ₀ 0.35, an
 * elevation of 20.5°, held to it by the test, so that the cache needn't load the bake's module.
 * Below it a haze's pseudo-spherical sky is past the model's limit: the reference's own
 * pseudo-spherical arm reads s90 red at 0.69–0.84 of the spherical sky (research-titan-horizon),
 * and the haze's gate judges no image whose sun stands there.
 */
export const LOW_SUN_SINE = 0.35;

/**
 * Why a body baked as a haze carries `atmosphereApproximate` where its brightest light stands at
 * most {@link LOW_SUN_SINE} over the camera's local horizontal and at or above it, the camera in
 * its air (R08.T14.d.4; main's ruling of 2026-11-01), and a camera beneath a routed deck over a
 * surface where it stands under {@link DECK_LOWEST_SUN_SINE} (R08.T15.b.2), as player text in
 * {@link TWILIGHT_REASON}'s form; below the horizontal, {@link TWILIGHT_REASON} stands (lean,
 * 2026-11-01). Its words await the owner's UX sign-off, the deck's case among them.
 */
export const LOW_SUN_REASON =
  "multiple scattering: computed beyond its checked range, the brightest star low over the local horizontal";

/**
 * Why a body routed by {@link DECKED_AIR_ROUTE_RANGE} alone ({@link deckedAirRouted}) carries
 * `atmosphereApproximate` with the camera in its air (plan R08, fix-r08-decked-air-opacity): its
 * gate judges the disc and the limb from orbit alone, as player text in {@link TWILIGHT_REASON}'s
 * form. Its words await the owner's UX sign-off.
 */
export const DECKED_AIR_REASON =
  "multiple scattering: computed beyond its checked range, the sky seen from within the air";

/**
 * The sine of the lowest sun the deck gates pass beneath a deck over a surface (R08.T15.b.2; lean,
 * 2026-11-02): `giant-deck`'s s80, 10° over the horizon, sin 10° = cos 80°, whose 63 values below
 * its split pass where its s90's 42 miss (the plane-parallel table dark at a horizon's sun); the
 * Venus-class cases refuse every value of s80. A camera beneath a routed deck over a surface whose
 * brightest light stands under it and at or above the horizontal reads {@link LOW_SUN_REASON}, as a
 * haze's low sun does.
 */
export const DECK_LOWEST_SUN_SINE = 0.17364817766693033;

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
