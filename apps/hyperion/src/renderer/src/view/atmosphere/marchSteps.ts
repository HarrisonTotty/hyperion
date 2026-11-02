/**
 * The per-frame marches' steps (plan R05, R05.T12.e, with addendum A of
 * `decision-r05-high-atmosphere.md`): the TypeScript twin of `marchSplit` and `marchStep` in
 * `shaders/common.wgsl`, which the sky-view, ray-march and multiple-scattering kernels take, and the quadrature gate's
 * tolerances.
 *
 * @remarks
 * Each ray's segment [t_start, t_end] is split at its lowest point t* = clamp(−o·d, t_start, t_end),
 * the point nearest the centre of the sphere the ray is marched about: the camera, the ground or a
 * limb's tangent point. Even steps under-sample the air there, whose aerosol has a 1.2 km scale
 * height: a 100 km vertical ray at 30–32 even steps undercounts its aerosol column by 24–26%, the
 * midpoint rule's 1 − (x/2) ÷ sinh(x/2) at x = Δh ÷ H (plan R05's Risks, "The per-frame marches
 * under-sample the dense air"). So each side is placed quadratically toward t*, at t* ∓ L·(k ÷ n)².
 * The camera side of a two-sided segment that starts at a camera inside the atmosphere is placed
 * toward the camera instead: where the camera's air is dense, on a ray that falls to a lowest point
 * below the camera and climbs out, the view's own attenuation puts that side's light at the camera
 * end. From 60–100 km it is not, and the high setting's 75 sky-view steps carry those rays
 * (addendum B). A two-sided segment shares its steps in proportion to the square root of each
 * side's length, which gives each side the same smallest step, L ÷ n², at the end it is placed
 * toward. Each step is sampled at its midpoint.
 *
 * Since plan R08's R08.T6.i each side is cut into pieces where the ray crosses a term's layer edge
 * ({@link marchPieces}, `common.wgsl`'s `MarchWalk`), each piece placed by the same rule toward its
 * end nearer t*, at the same counts; a segment that crosses no edge keeps its steps to the bit.
 *
 * Since plan R08's R08.T8 the ray march's two-sided sky segments, inside the air or outside it,
 * take the sun-aware warp instead ({@link marchEvenStep}, {@link marchEvenSteps};
 * `decision-r08-t8-leans.md` §4 and §5.4): even where the Sun is ahead or across the ray, R05's
 * one-sided placement from the segment's start where it is behind. A camera in the march band's
 * thin air, and the night-side limb from outside, put a sky segment's light away from t*.
 *
 * sebh's reference code (UnrealEngineSkyAtmosphere, `RenderSkyRayMarching.hlsl`,
 * `IntegrateScatteredLuminance`) bounds its steps quadratically from the ray's start, at
 * t_max·(s ÷ N)² with N from 4 to 14, and samples each at 0.3 of its length; Bevy 0.19.1
 * (`functions.wgsl`, `raymarch_atmosphere`) spaces them evenly. The `f64` twin of the two kernels'
 * quadrature in `marchSteps.test.ts` is R05.T12.e's gate on the rule and the step counts.
 */

import { add, dot, normalise, scale, type Vec3 } from "../../geometry/vec3";

/**
 * R05.T12.e's gate on a march's radiance: e = max over channels of |ΔL| ÷ max(L, 10⁻³ L_max) at
 * most 2% against a converged march, L_max the kernel's brightest reference pixel per channel
 * (`decision-r05-high-atmosphere.md`). It holds the high setting's counts.
 */
export const MARCH_TOLERANCE = 0.02;

/** The gate's tolerance for a twilight ray ({@link grazingTwilight}) at the high setting's counts. */
export const TWILIGHT_TOLERANCE = 0.05;

/**
 * The worst e of the twin at the low setting's 16 and 16 steps over the media it holds, R05's Earth
 * and `earth.ts`'s `EARTH_REFERENCE`, each per kernel and class, rounded up to 0.1%: the bound the
 * GPU agreement check holds low's outputs to, plus 1% for f32. It is a measurement, not an accuracy
 * gate (`decision-r08-t6d-findings.md` §10): low's own gate is only that its worst e over the gate's
 * rays is no worse than even steps' at the same counts (addendum A, 4(a)). `marchSteps.test.ts` and
 * `tablesCpu.test.ts` hold the twins within these. The sky view's are split by the camera's height,
 * so that the limb from 60–100 km (addendum B), which 16 steps cannot resolve, does not loosen the
 * bound from below 50 km.
 *
 * @remarks
 * The sky view's twilight bound is 0.176 since R08.T6.h, set by the measured Earth's band ray from
 * 50,000 m at view zenith 97.008°, sun 90°/180° (17.59%, against 13.13% without its ozone and
 * 14.46% on R05's Earth); it was 0.175, from R05's Earth.
 *
 * The ray march's are 0.036 and 0.240 since R08.T8 (2026-10-21, `decision-r08-t8-leans.md` §4.2
 * and §5.5); they were 0.049 and 0.182 before it. Under §4's even steps for the march's two-sided
 * sky segments they were 0.036 and 0.245: the gate gained the night-side and terminator limb
 * (`test/atmosphereGate.ts`'s `night limb`), which R05.T12.e's placement had put at 39.0% on R05's
 * Earth, and its twilight bound was the measured Earth's night limb tangent at 0.1 H, sun
 * 100°/180° (24.47%). Under the sun-aware warp (§5.4), with the gate's last light (`last light`,
 * the ray march's, 4.50% on R05's Earth and 3.71% on the measured one), that night limb's sun is
 * behind the camera, its rays at a = 1, and the measured Earth's night limb falls to 22.35%, so the
 * twilight bound is R05's Earth's limb at 0.1 H, sun 90°/0° (23.94%, its sun ahead and its steps
 * even; 5.87% under R05.T12.e's placement), 0.245 → 0.240. The ordinary bound is the measured
 * Earth's disc at ground zenith 80°, sun 95°/180° (3.59%), since the limb's ordinary rays, which set
 * the old one (R05's Earth at 0.3 H, sun 30°/180°, 4.80%), fall to 1.81% on R05's Earth and 2.75%
 * on the measured one (`tablesCpu.test.ts` records each by family). A measurement under addendum A,
 * 4(b), not a wider gate. The sky view's are unchanged by R08.T8: its twin stands at 8.20% / 17.59%
 * from cameras up to 50 km and 8.25% / 45.09% from 60–100 km over the two media, within them.
 */
export const LOW_TWIN_WORST = {
  /** The sky view from cameras up to 50 km. */
  skyView: { ordinary: 0.091, twilight: 0.176 },
  /** The sky view from cameras at 60–100 km. */
  skyViewHigh: { ordinary: 0.116, twilight: 0.495 },
  rayMarch: { ordinary: 0.036, twilight: 0.24 },
} as const;

/** The sun's least angle from the zenith, and the ray's greatest from the horizon, of a twilight ray. */
const TWILIGHT_SUN_ZENITH_RAD = (80 * Math.PI) / 180;
const TWILIGHT_RAY_ELEVATION_RAD = (10 * Math.PI) / 180;

/** The margin each twilight limit must be passed by, rad, so that rays set at 80° or 10° are not. */
const TWILIGHT_MARGIN_RAD = 1e-12;

/**
 * Whether a march's ray is a twilight ray, held to {@link TWILIGHT_TOLERANCE}: at the segment's
 * lowest point t* (the camera, the ground or the tangent point), the sun more than 80° from the
 * zenith and the ray within 10° of the horizon, each by more than 10⁻¹² rad. Every limb ray meets
 * its tangent point level, a sky ray above the horizon beyond 80° of view zenith meets the camera
 * within 10° of its horizon, and a disc ray meets the ground within 10° of its horizon beyond 80° of
 * its zenith.
 *
 * @param originM - The ray's origin from the centre of the sphere it is marched about, m.
 * @param direction - The ray's unit direction.
 * @param sun - The unit direction to the sun.
 */
export function grazingTwilight(
  originM: Vec3,
  direction: Vec3,
  sun: Vec3,
  tStartM: number,
  tEndM: number,
): boolean {
  const lowestM = Math.min(Math.max(-dot(originM, direction), tStartM), tEndM);
  const up = normalise(add(originM, scale(direction, lowestM)));
  return (
    dot(up, sun) < Math.cos(TWILIGHT_SUN_ZENITH_RAD + TWILIGHT_MARGIN_RAD) &&
    Math.abs(dot(up, direction)) < Math.sin(TWILIGHT_RAY_ELEVATION_RAD - TWILIGHT_MARGIN_RAD)
  );
}

/** A ray's segment split at its lowest point, with the steps each side takes. */
export interface MarchSplit {
  readonly startM: number;
  /** The lowest point's distance along the ray, m: t* = clamp(−o·d, t_start, t_end). */
  readonly lowestM: number;
  /** The segment's length before the lowest point, m. */
  readonly beforeM: number;
  /** The segment's length after the lowest point, m. */
  readonly afterM: number;
  readonly stepsBefore: number;
  readonly stepsAfter: number;
  /** Whether the side before t* is placed toward the segment's start, the camera. */
  readonly beforeTowardStart: boolean;
}

/** One step of a march: its midpoint's distance along the ray and its length, m. */
export interface MarchStep {
  readonly tM: number;
  readonly dtM: number;
}

/**
 * Splits the segment [`tStartM`, `tEndM`] at `nearestM`, the distance −o·d along the ray to its
 * closest approach to the centre, clamped into the segment. A two-sided segment shares `samples`
 * steps in proportion to the square root of each side's length, at least one a side; a one-sided
 * segment gives them all to its side.
 *
 * @param samples - The march's steps, an integer of at least 2 (the kernels take at least 2).
 * @param fromCamera - Whether the segment starts at a camera inside the atmosphere, whose side of a
 * two-sided segment is then placed toward the camera.
 * @throws RangeError if `samples` is not an integer of at least 2.
 */
export function marchSplit(
  tStartM: number,
  tEndM: number,
  nearestM: number,
  samples: number,
  fromCamera: boolean,
): MarchSplit {
  if (!Number.isInteger(samples) || samples < 2) {
    throw new RangeError(`a march takes at least 2 steps, not ${samples}`);
  }
  const lowestM = Math.min(Math.max(nearestM, tStartM), tEndM);
  const beforeM = lowestM - tStartM;
  const afterM = tEndM - lowestM;
  const rootBefore = Math.sqrt(beforeM);
  const total = rootBefore + Math.sqrt(afterM);
  // floor(x + 0.5), as the WGSL rounds, rather than WGSL's round(), which rounds half to even.
  const share = total > 0 ? Math.floor((samples * rootBefore) / total + 0.5) : 0;
  const least = beforeM > 0 ? 1 : 0;
  const most = samples - (afterM > 0 ? 1 : 0);
  const stepsBefore = Math.min(Math.max(share, least), most);
  return {
    startM: tStartM,
    lowestM,
    beforeM,
    afterM,
    stepsBefore,
    stepsAfter: samples - stepsBefore,
    beforeTowardStart: fromCamera && beforeM > 0 && afterM > 0,
  };
}

/**
 * Step `i` of a split segment, counted from t_start. A side of length L and n steps placed toward
 * an end a has its boundaries at a ∓ L (k ÷ n)², k = 0…n, so its step k from a has its midpoint at
 * a ∓ L (k² + k + ½) ÷ n² and the length L (2k + 1) ÷ n².
 *
 * @param i - In [0, `stepsBefore` + `stepsAfter`).
 */
export function marchStep(split: MarchSplit, i: number): MarchStep {
  if (i < split.stepsBefore) {
    const n = split.stepsBefore;
    const unitM = split.beforeM / (n * n);
    if (split.beforeTowardStart) {
      return { tM: split.startM + unitM * (i * i + i + 0.5), dtM: unitM * (2 * i + 1) };
    }
    const k = n - 1 - i;
    return { tM: split.lowestM - unitM * (k * k + k + 0.5), dtM: unitM * (2 * k + 1) };
  }
  const n = split.stepsAfter;
  const k = i - split.stepsBefore;
  const unitM = split.afterM / (n * n);
  return { tM: split.lowestM + unitM * (k * k + k + 0.5), dtM: unitM * (2 * k + 1) };
}

// --- The ray march's count by a segment's optical depth (plan R08, R08.T8.e) ---------------------

/**
 * The band over the datum within which high's march counts a drawn surface as the bare ground, m:
 * 25 km (plan R08, R08.T11.b; `decision-r08-t11-frame.md` §2, lever A;
 * {@link MarchDepthSteps.drawnBandM}).
 *
 * @remarks
 * The least band that holds R07's mesh and terrain's heights (`decision-r08-t11-frame.md` §8,
 * which confirmed it). R07's mesh draws the face on its chords, which sag inside the spheroid, so
 * its depth lies at or beyond the hit (its limb draw writes none), where the march ends at the hit
 * as on the bare ground: its segment is the bare ground's, and its count the one the gate accepted
 * for the same rays before R08.T10.b drew the face with depth. Terrain stands above the datum,
 * before the hit: plan 14 caps a volcanic edifice's rise over its base at 21.9 km at Mars's
 * gravity, Olympus Mons's (`TALLEST_EDIFICE_RELIEF`, after Plescia 2004), so 25 km holds an
 * edifice's rise at Mars's gravity and above, and its summit where its base stands within about
 * 3 km of the datum (an Earth's 8.8 km, Everest's 8,848.86 m by the 2020 Nepal–China survey, and
 * Venus's 11 km, Maxwell Montes over its mean radius by Magellan's altimetry, Ford and Pettengill
 * 1992, J. Geophys. Res. 97, 13103). A summit over 25 km, on a world lighter
 * than Mars, whose cap grows as 1 ÷ g, or on high ground, keeps the setting's count, and so its
 * reference path. V4's share of the hit distance, 1%, holds neither: 4 km from 400 km at the
 * nadir, under an Earth's summit, and 36–77 km before the face from 10⁷ m.
 */
export const DRAWN_SURFACE_BAND_M = 25_000;

/**
 * The ray march's count scaled by a segment's optical depth (plan R08, R08.T8.e; lever 3 of
 * `decision-r08-t8-leans.md` §8, taken by §9, Ruling 2, item 3; the thin sky's floor by §10,
 * Ruling 4, item 2): at most the setting's count, down to the floors R05.T12.e's gate sets, at no
 * wider tolerance, the shader and the twin alike ({@link marchSamplesOf}).
 *
 * @remarks
 * Measured against the gate at one source and at six on R05's Earth, `earth.ts`'s `EARTH_REFERENCE`
 * and the slab Earth (2026-10-22). The bare ground's error does not follow its optical depth: its
 * worst rays are the low suns, 80° from the zenith ahead of the ray and 95° behind it, at depths of
 * 0.45–1.3, where the ground's least passing count is 18 on the two media without a march edge and
 * 28 on the slab Earth, whose ozone slab's two edges cut each ray into three pieces. Grazing bare
 * ground under a twilight sun behind the camera (the gate's `past` family from 50 km, depth 6–10)
 * needs 22–28 even on R05's Earth, so the ground's count climbs back to the setting's over
 * {@link MarchDepthSteps.groundRampDepths}. Since R08.T11.b a drawn surface on the datum takes the
 * bare ground's count ({@link MarchDepthSteps.drawnBandM}, {@link drawnSurfaceKind}); anything else
 * drawn keeps the setting's.
 *
 * A thin sky segment's error follows its depth in the gate (every one under 0.05 in green held at
 * 8–12 steps), but the gate does not bound the floor (2026-10-22–23): 12 steps put the far limb of
 * a Saturn-class body at 10⁹ m over its 5% flicker bound (5.42%) and its quadrature on the limb set
 * to 14.6%, the camera's last light from 50–100 km 4–10 codes off, and a seam of 3.5 codes at the
 * horizontal from 40 km, where a thin ray up meets a thick one down. Searched at 16, 20, 24, 28 and
 * 32 steps against the gate, the far limb's flicker and fidelity, the band's seam, the top's step
 * and the last light, which binds: on high its worst window stands at 3.76, 2.70 and 2.27 codes at
 * 16, 20 and 24 (1.68 at 36, from 95 km; §5.4's warp held it within 1.7), and at 28 at 1.68 as at
 * 36, the rows it moves at most 1.26 (80 km, from 1.21). So the thin sky takes 28 steps
 * ({@link MarchDepthSteps.thinSky}). On T8.c's timing frames it takes the march's mean steps a
 * marched pixel from 0.622 to 0.604 of 36 from 400 km on the measured Earth, and 4–7% more off the
 * far cameras' footprint nodes over the thin limb.
 */
export interface MarchDepthSteps {
  /** The bare ground's count on a medium with no march edge, before its ramp. */
  readonly groundFloor: number;
  /** What each of the medium's march edges ({@link marchEdgesM}) adds to the ground's floor. */
  readonly groundFloorPerEdge: number;
  /**
   * The green optical depths [d₀, d₁] of the bare ground's segment over which its count climbs from
   * its floor to the setting's, in even steps: the floor at d₀ and below, the setting's at d₁ and
   * above.
   */
  readonly groundRampDepths: readonly [number, number];
  /** A thin sky segment's count, or `null` where the sky keeps the setting's count. */
  readonly thinSky: ThinSkyRule | null;
  /**
   * The band over the datum within which a drawn surface takes the bare ground's count, m (plan
   * R08, R08.T11.b): a surface on a ray that meets the datum, at most this high above it before the
   * datum's hit, or anywhere at or beyond the hit ({@link drawnSurfaceKind}); high's is
   * {@link DRAWN_SURFACE_BAND_M}.
   */
  readonly drawnBandM: number;
}

/** A thin sky segment's count under {@link MarchDepthSteps} (plan R08, R08.T8.e). */
export interface ThinSkyRule {
  /** The count of a sky segment under `depthG`, at most the setting's. */
  readonly samples: number;
  /**
   * The green optical depth, absorbers included, under which a sky segment is thin: summed from its
   * lowest point along each side it has.
   */
  readonly depthG: number;
}

/**
 * What a ray-march segment ends on, as `rayMarch.wgsl`'s `marchRay` tells it (plan R08, R08.T8.e):
 * the bare ground with nothing drawn (`ground`), the sky with nothing drawn and the ground not
 * reached (`sky`), or a drawn surface (`surface`). Since R08.T11.b a drawn surface on the datum
 * counts as the bare ground ({@link drawnSurfaceKind}).
 */
export type MarchSegmentKind = "ground" | "sky" | "surface";

/**
 * What a ray-march segment that ends on a drawn surface counts as (plan R08, R08.T11.b, lever A of
 * `decision-r08-t11-frame.md` §2): the bare ground (`ground`), its count by its optical depth from
 * the datum's hit back along the ray, where the ray meets the datum and the surface stands within
 * the rule's {@link MarchDepthSteps.drawnBandM} above it, at or beyond the hit included; a drawn
 * surface (`surface`), the setting's count, otherwise or with no rule. Only the count changes: the
 * ray's end, its transmittance and the surface's light are the drawn surface's, and no bare ground
 * is added. As `rayMarch.wgsl`'s `marchRay` tells it.
 *
 * @param heightM - The drawn point's height above the datum, m: 0 where it stands at or beyond the
 *   datum's hit, where the march ends at the hit; `null` where the ray meets no datum.
 */
export function drawnSurfaceKind(
  heightM: number | null,
  rule: MarchDepthSteps | null,
): MarchSegmentKind {
  return rule !== null && heightM !== null && heightM <= rule.drawnBandM ? "ground" : "surface";
}

/**
 * The ray march's count for one segment (plan R08, R08.T8.e; `decision-r08-t8-leans.md` §8, lever 3):
 * the setting's `samples` with no rule, on a drawn surface, or where the depth is not finite; on a
 * sky segment the rule's {@link ThinSkyRule}'s count, at most `samples`, under its depth, and
 * `samples` otherwise or with none; on the bare ground the floor F₀ = min(`samples`, `groundFloor` +
 * `groundFloorPerEdge` × `edges`) up to d₀, the setting's from d₁, and between them F₀ +
 * 2⌈(`samples` − F₀)(d − d₀) ÷ (2 (d₁ − d₀))⌉, held to [F₀, `samples`]. The kernel forms the same in
 * `f32` (`rayMarch.wgsl`).
 *
 * @param depthG - The segment's green optical depth, absorbers included: the bare ground's from its
 *   ground point along −d (a drawn surface's on the datum from the datum's hit, R08.T11.b), a sky
 *   segment's from its lowest point along each side (`tablesCpu.ts`'s `rayMarchTexelInto`).
 * @param samples - The setting's count (`TableSizes`' `rayMarchSamples`).
 * @param edges - The medium's march edges, as many as the kernel's `layerEdges.count.x`.
 * @param rule - The setting's {@link MarchDepthSteps}, or `null` for the setting's count everywhere.
 */
export function marchSamplesOf(
  kind: MarchSegmentKind,
  depthG: number,
  samples: number,
  edges: number,
  rule: MarchDepthSteps | null,
): number {
  if (rule === null || kind === "surface" || !Number.isFinite(depthG)) {
    return samples;
  }
  if (kind === "sky") {
    const { thinSky } = rule;
    return thinSky !== null && depthG < thinSky.depthG
      ? Math.min(samples, thinSky.samples)
      : samples;
  }
  const floor = Math.min(samples, rule.groundFloor + rule.groundFloorPerEdge * edges);
  const [d0, d1] = rule.groundRampDepths;
  if (depthG <= d0) {
    return floor;
  }
  if (depthG >= d1) {
    return samples;
  }
  const climb = 2 * Math.ceil(((samples - floor) * (depthG - d0)) / (2 * (d1 - d0)));
  return Math.min(Math.max(floor + climb, floor), samples);
}

// --- Layer edges (plan R08, R08.T6.i) ------------------------------------------------------------

/**
 * The most edges the per-frame marches cut at, two a term of the kernels' `MAX_TERMS` (8):
 * `common.wgsl`'s `MARCH_EDGES_MAX`.
 */
export const MARCH_EDGES_MAX = 16;

/**
 * A term's layer edges as the per-frame marches cut at them (plan R08, R08.T6.i;
 * `decision-r08-t6d-findings.md` §1.3): each ramp of its density, a pair of `medium.ts`'s
 * `layerEdgesM` (its lower and upper edge, as `tables.ts`'s `packLayerEdges` packs them), is one
 * edge at its middle height, so at most two a term. A middle at or below the ground or at or above
 * `topHeightM` is crossed by no ray inside the atmosphere and is left out, as is a pair of
 * `packLayerEdges`' unused −1s.
 *
 * @remarks
 * A ramp of 10–300 m (78 m at the ozone slab's base, `earth.ts`'s `EARTH_OZONE_SLAB`, resampled over
 * one to three of the kernels' 1,024 levels) inside a step of tens of kilometres errs by about its
 * half-width's column. Four edges a term would give each ramp a piece of its own, up to nine pieces
 * on a low ray's eight-step side.
 *
 * A projected density (`medium.ts`'s `TabulatedDensity.projected`; plan R08, R08.T16.a.2) is cut at
 * both ends of each pair that holds more than {@link MARCH_SHEET_NEGLIGIBLE} of its largest density
 * instead, and at none of a pair that holds less: a sheet its levels would alias spreads over the
 * levels about it, which its widened pair brackets, so it takes a piece of its own. Cut at the
 * pairs' middles the hand Jupiter's decks drawn at their own columns read 9.3% (0°) and 15.7% (60°)
 * from their march at 16 times high's samples, and at their outer ends 34% and 17%; their sedimented
 * sheets' tops hold some 10⁻²⁷⁰ of their bases', and take no cut (2026-11-02).
 *
 * @param edgesM - A term's layer edges, m, in `layerEdgesM`'s pairs.
 * @param projected - A projected term's density at a height and its largest, or `null` to cut at
 *   each pair's middle.
 */
export function marchEdgesM(
  edgesM: ReadonlyArray<number>,
  topHeightM: number,
  projected: ProjectedDensity | null = null,
): number[] {
  const middles: number[] = [];
  const inside = (edge: number): void => {
    if (edge > 0 && edge < topHeightM) {
      middles.push(edge);
    }
  };
  for (let i = 0; i + 1 < edgesM.length; i += 2) {
    const lower = edgesM[i] ?? Number.NaN;
    const upper = edgesM[i + 1] ?? Number.NaN;
    if (projected === null) {
      inside(0.5 * (lower + upper));
      continue;
    }
    const held = Math.max(
      projected.densityAt(lower),
      projected.densityAt(0.5 * (lower + upper)),
      projected.densityAt(upper),
    );
    if (held > MARCH_SHEET_NEGLIGIBLE * projected.largest) {
      inside(lower);
      inside(upper);
    }
  }
  return middles;
}

/** A projected term's density as {@link marchEdgesM} reads it: at a height, and its largest. */
export interface ProjectedDensity {
  readonly densityAt: (heightM: number) => number;
  readonly largest: number;
}

/**
 * The share of a projected term's largest density under which {@link marchEdgesM} takes a ramp to
 * hold nothing and cuts none there: 10⁻⁹, as far below the light it could return as below the
 * marches' `f32`.
 */
export const MARCH_SHEET_NEGLIGIBLE = 1e-9;

/**
 * The least rise of h\* along a ray, |s (d · n)|, at which the ray march takes a layer edge's crossing
 * onto the edge's own level h\* = e (plan R08, R08.T16.a.2; `rayMarch.wgsl`'s
 * `MARCH_EDGE_LEAST_RISE`): 0.05, a ray within about 3° of the level there keeping its shell's root,
 * whose Newton step would be long and whose two crossings lie close about a tangent point.
 *
 * @remarks
 * An oblate body's edge e is the shell (a + e ÷ s_e, c + e ÷ s_p), which stands at h\* = e at the
 * equator and the poles alone: between them s(φ) is not the shell's, and on the hand Jupiter's
 * figure (Δs 16.7% pole to equator) its 178 km NH₃ edge stands about 1 km off its level at 45°,
 * as thick as the deck's sheet above it. The ray march's crossings take one Newton step of h\* along
 * the ray onto the level, which leaves their error at the metre: the hand Jupiter's disc at
 * mid-latitudes read 0.75–0.85 of its converged march on the shells' crossings, within 2% on the
 * level's (2026-11-02).
 */
export const MARCH_EDGE_LEAST_RISE = 0.05;

/**
 * A split segment cut into pieces where it crosses the marches' layer edges (plan R08, R08.T6.i):
 * `common.wgsl`'s `MarchWalk` once made ready.
 */
export interface MarchPieces {
  readonly split: MarchSplit;
  /** Each crossing's signed distance from t*, m, ascending, each once: those before t* first. */
  readonly fromLowestM: ReadonlyArray<number>;
  /** How many of them lie before t*. */
  readonly crossingsBefore: number;
  /**
   * The steps of each piece of the side before t*, in order along the ray, or `null` where the side
   * keeps R05.T12.e's placement: it crosses no edge, or has more pieces than steps.
   */
  readonly before: ReadonlyArray<number> | null;
  /** The same for the side after t*. */
  readonly after: ReadonlyArray<number> | null;
  /** How many of its sides cross an edge but have more pieces than steps, 0 to 2. */
  readonly crowded: number;
  /**
   * Whether each piece of the side before t* is placed toward its end nearer the segment's start,
   * in order along the ray (plan R08, R08.T16.a.2; {@link depthSharedPieces}), or `null` where the
   * side places them as R08.T6.i does: toward t*, or toward the camera on a camera side.
   */
  readonly beforeTowardStart: ReadonlyArray<boolean> | null;
  /** The same for the side after t*, `null` where each is placed toward t*. */
  readonly afterTowardStart: ReadonlyArray<boolean> | null;
}

/**
 * One side's steps by piece (`common.wgsl`'s `marchSideShares`): `steps` among the pieces from
 * `startM` to `endM` from t* that the crossings `first` to `first + count` cut, one a piece and the
 * rest in proportion to the square root of each piece's length, each piece's count rounding its
 * end's share of the rest (or taking its other side, {@link sharesOf}), the last the rest; `null`
 * for a side that crosses no edge or has more pieces than steps.
 */
function sideShares(
  fromLowestM: ReadonlyArray<number>,
  first: number,
  count: number,
  startM: number,
  endM: number,
  steps: number,
  otherWithinSteps: number,
): number[] | null {
  const pieces = count + 1;
  if (count === 0 || pieces > steps) {
    return null;
  }
  const roots: number[] = [];
  let total = 0;
  let from = startM;
  for (let p = 0; p < pieces; p += 1) {
    const to = p < count ? (fromLowestM[first + p] ?? Number.NaN) : endM;
    const root = Math.sqrt(to - from);
    roots.push(root);
    total += root;
    from = to;
  }
  return sharesOf(roots, total, steps, otherWithinSteps);
}

/**
 * `steps` among pieces by their weights (`rayMarch.wgsl`'s `marchSharesOf`): one a piece and the rest
 * in proportion to each piece's weight, each piece's count rounding its end's share of the rest, the
 * last the rest. `total` is the weights' sum, above 0.
 *
 * @param otherWithinSteps - How near its rounding an end's share may stand, in steps, to be taken
 *   to the other side of it, each piece still at least one: 0 for the kernels' rounding; the
 *   smoke's envelope's share the kernel's `f32` may take ({@link MARCH_SHARE_ROUNDING_STEPS}).
 */
function sharesOf(
  weights: ReadonlyArray<number>,
  total: number,
  steps: number,
  otherWithinSteps = 0,
): number[] {
  const pieces = weights.length;
  const rest = steps - pieces;
  let spent = 0;
  let cumulative = 0;
  return weights.map((weight, p) => {
    cumulative += weight;
    let share = rest;
    if (p + 1 < pieces) {
      // floor(x + 0.5), as the WGSL rounds.
      const at = (rest * cumulative) / total + 0.5;
      share = Math.floor(at);
      const off = at - Math.round(at);
      if (Math.abs(off) < otherWithinSteps) {
        share = Math.min(Math.max(off < 0 ? share + 1 : share - 1, spent), rest);
      }
    }
    const taken = 1 + share - spent;
    spent = share;
    return taken;
  });
}

/**
 * Cuts each side of a split segment into pieces at the crossings inside it (plan R08, R08.T6.i;
 * `decision-r08-t6d-findings.md` §1): the TypeScript twin of `common.wgsl`'s `marchWalkCross` and
 * `marchWalkReady`.
 *
 * @remarks
 * R05.T12.e's placed steps take a layer's edge to first order, as T6.d's slab Earth showed (limb
 * 21.3%, high limb 14.2%): a step whose midpoint falls just below a dense base misses up to half a
 * step of it, and one just above counts its density over the whole step. A side's steps are shared
 * among its pieces in proportion to the square root of each piece's length, which gives each piece
 * the same smallest step at the end it is placed toward, as {@link marchSplit} shares a segment's
 * between its sides; at least one a piece, each piece's count rounding its end's share of the rest
 * so that they sum to the side's, as `tablesCpu.ts`'s `transmittancePieces` rounds its pieces'. A
 * side that crosses no edge, or has more pieces than steps, keeps R05.T12.e's placement. The twin
 * forms the shares in `f64` where the kernel forms them in `f32`, so at a rounding boundary a piece
 * may differ by a step.
 *
 * @param crossingsFromLowestM - Where the ray crosses the edges, as signed distances from t*, m, in
 *   any order; those not strictly inside either side, and repeats, are left out.
 * @param otherWithinSteps - The shares taken to the other side of their rounding
 *   ({@link MARCH_SHARE_ROUNDING_STEPS}): 0, none, the kernels' own.
 */
export function marchPieces(
  split: MarchSplit,
  crossingsFromLowestM: Iterable<number>,
  otherWithinSteps = 0,
): MarchPieces {
  const inside = [...crossingsFromLowestM]
    .filter((t) => (t > -split.beforeM && t < 0) || (t > 0 && t < split.afterM))
    .toSorted((a, b) => a - b);
  const fromLowestM: number[] = [];
  for (const t of inside) {
    const previous = fromLowestM.at(-1);
    if (previous === undefined || t > previous) {
      fromLowestM.push(t);
    }
  }
  const crossingsBefore = fromLowestM.filter((t) => t < 0).length;
  const crossingsAfter = fromLowestM.length - crossingsBefore;
  const before = sideShares(
    fromLowestM,
    0,
    crossingsBefore,
    -split.beforeM,
    0,
    split.stepsBefore,
    otherWithinSteps,
  );
  const after = sideShares(
    fromLowestM,
    crossingsBefore,
    crossingsAfter,
    0,
    split.afterM,
    split.stepsAfter,
    otherWithinSteps,
  );
  const crowded =
    (crossingsBefore > 0 && before === null ? 1 : 0) +
    (crossingsAfter > 0 && after === null ? 1 : 0);
  return {
    split,
    fromLowestM,
    crossingsBefore,
    before,
    after,
    crowded,
    beforeTowardStart: null,
    afterTowardStart: null,
  };
}

/**
 * The most green optical depth a piece's end is read at by {@link depthSharedPieces}, from it to the
 * top: 64, under the 87.3 at which `f32`'s e^−τ leaves its normal range (2⁻¹²⁶), so that the
 * kernel's −ln of the lookup's transmittance stays finite; light from beyond it is e^−64 of the
 * source's. `rayMarch.wgsl`'s `MARCH_PIECE_DEPTH_CAP`.
 */
export const MARCH_PIECE_DEPTH_CAP = 64;

/**
 * The green optical depth, absorbers included, from the point `fromLowestM` from t* to the top, read
 * back along the ray (`back`, on the side before t*) or on along it (on the side after it), as the
 * ray march's count reads a depth (plan R08, R08.T16.a.2; `rayMarch.wgsl`'s `rayMarchDepthAlong`).
 */
export type PieceDepthAt = (fromLowestM: number, back: boolean) => number;

/**
 * The march's first source's light at the point `fromLowestM` from t*, in [0, 1]: its green
 * transmittance there, absorbers included, 0 where the ground shadows it, 1 where no source is drawn
 * (plan R08, R08.T16.a.2; `rayMarch.wgsl`'s `rayMarchPieceAt`).
 */
export type PieceLightAt = (fromLowestM: number) => number;

/**
 * The medium's green extinction at the point `fromLowestM` from t*, m⁻¹, at its h* as the march's
 * steps read the medium (plan R08, R08.T16.a.2; `rayMarch.wgsl`'s `rayMarchPieceAt`).
 */
export type PieceExtinctionAt = (fromLowestM: number) => number;

/**
 * How much brighter a piece's other end must be than the end its placement is fine at before
 * {@link depthSharedPieces} turns it there: 100. A thick deck under a low sun is brighter at its top
 * than at its floor by some e^10 (`giant-deck`'s envelope at μ₀ cos 85°), and R05.T12.e's
 * placement toward t* holds the troposphere's few-fold step between its ground and its top under
 * the sun at the horizon, which R05.T12.e's gate measures (2.3–3.2% when turned).
 */
export const MARCH_PIECE_TURN = 100;

/**
 * What a shadowed piece's light counts for in {@link depthSharedPieces}' weights, against 1 for a
 * piece the first source lights unattenuated: 0.01, so that a ray wholly in the night keeps its
 * shares by depth, which its lit neighbours' approach as the light fails, with no step between.
 */
export const MARCH_PIECE_SHADE = 0.01;

/**
 * How near its rounding a piece's share may stand, in steps, for the kernel's `f32` to take the
 * other side of it: 2⁻⁸ (plan R08, R08.T16.a.2; the smoke's envelope of a march's piece shares).
 *
 * @remarks
 * The ray march shares a cut side's steps by its pieces' light ({@link depthSharedPieces}), formed
 * from the lookups at the pieces' ends, which the kernel forms in `f32` and the twin in `f64`. On
 * the hand Jupiter's deck route, whose four sheets cut each side into ten pieces of a rest of 26
 * steps, the twin's shares stood 1.7 × 10⁻³ and 7 × 10⁻⁵ of a step from their rounding at the one
 * texel in each of two frames whose kernel took a step from one piece to the next (2026-11-03):
 * 2⁻⁸ holds twice the larger, and a kernel's share further off still misses. The kernels round
 * every share; only the smoke's twin envelope takes the other side, as it holds the split
 * (`marchSplit`) the kernel may take at its rounding.
 */
export const MARCH_SHARE_ROUNDING_STEPS = 2 ** -8;

/**
 * `pieces` with each side that is cut into pieces sharing its steps among them by the light each can
 * return along the view, and each placed toward its brighter end (plan R08, R08.T16.a.2;
 * `rayMarch.wgsl`'s `rayMarchDepthShares`).
 *
 * - **Shares**: one a piece, the rest in proportion to the weight ½ D_p ÷ ΣD + ½ √L_p ÷ Σ√L, each
 *   piece's count rounding its end's share of the rest, the last the rest, so that the side keeps
 *   its count. D_p = e^−c_p (1 − e^−τ_p) (ε + l̄_p) is the piece's lit drop in the side's view
 *   transmittance from the side's start (the segment's start before t*, t* after it): τ_p its green
 *   optical depth along the ray, the difference of `depthAt` at its ends, each held to
 *   {@link MARCH_PIECE_DEPTH_CAP} (a value that is not a number reads the cap, as the kernel's
 *   `select` reads it) and the difference to at least 0; c_p the side's τ before it; l̄_p the mean of
 *   `lightAt` at its ends, ε {@link MARCH_PIECE_SHADE}. The other half is R08.T6.i's share by the
 *   square root of each piece's length L_p.
 * - **Placement**: each piece's steps placed quadratically as R05.T12.e places them, toward t* (or
 *   toward the camera on a camera side), unless its other end's b = σ e^−c (ε + l) is more than
 *   {@link MARCH_PIECE_TURN} times this end's, when they are placed toward the other: σ
 *   `extinctionPerMAt` there, c the side's τ to it and l its light.
 *
 * A side not cut, or whose drops sum to 0, keeps R08.T6.i's shares and placement.
 *
 * @remarks
 * R08.T6.i's shares by length, placed toward t*, gave the thin decks stacked above the hand
 * Jupiter's envelope floor a few steps of a long ray, and its routed march at the high setting's 36
 * read 12% (0°) and 24% (90°) dark in red against its march at 32 times them. The decks are sheets:
 * its NH₃ deck's τ(550) of 1.2 lies within 2 km of its 178 km base, at the lower end of an 18 km
 * piece that took 3 steps. Measured on the twin (2026-11-02):
 *
 * - by τ_p alone, the brief's lean, the hand Jupiter's disc read 0.92 and 0.85 in red, the nadir's
 *   deep gas under the decks (τ 4 of its 5.6) took 19 of 36 steps, and `giant-deck`'s envelope disc
 *   read 28% off at its edge;
 * - by the unlit drop, half by length, the Jupiter read within 2.1% in every channel, but the slab
 *   Earth's twilight disc (ground zenith 80°, the sun 5° under its horizon) missed R05.T12.e's 2%,
 *   its steps in the troposphere's shadow, and a thick deck under a low sun (`giant-deck`'s envelope
 *   at μ₀ cos 85°) took its fine steps at its floor, away from its lit top: lit, placed toward t*,
 *   4.9% off;
 * - lit, and turned toward each piece's brighter end, the slab Earth's grazing disc under a sun at
 *   the horizon missed it (2.3–3.2%), its troposphere's fine steps turned from the ground;
 * - lit, turned only toward an end 100 times the brighter, the measures of `marchSheets.test.ts`,
 *   `thick/discBakeJupiter.test.ts`, `reference/envelopeMarch.test.ts` and `test/marchGate.ts`.
 *
 * @param otherWithinSteps - The shares taken to the other side of their rounding
 *   ({@link MARCH_SHARE_ROUNDING_STEPS}): 0, none, the kernels' own.
 */
export function depthSharedPieces(
  pieces: MarchPieces,
  depthAt: PieceDepthAt,
  lightAt: PieceLightAt,
  extinctionPerMAt: PieceExtinctionAt,
  otherWithinSteps = 0,
): MarchPieces {
  const { split, fromLowestM, crossingsBefore } = pieces;
  const capped = (t: number, back: boolean): number => {
    const depth = depthAt(t, back);
    return depth < MARCH_PIECE_DEPTH_CAP ? depth : MARCH_PIECE_DEPTH_CAP;
  };
  const reshared = (
    shares: ReadonlyArray<number> | null,
    bounds: ReadonlyArray<number>,
    back: boolean,
    placedTowardStart: boolean,
  ): { readonly shares: ReadonlyArray<number>; readonly towardStart: boolean[] } | null => {
    if (shares === null) {
      return null;
    }
    const drops: number[] = [];
    const roots: number[] = [];
    const brightness: number[] = [];
    let dropTotal = 0;
    let rootTotal = 0;
    let before = 0;
    let near = capped(bounds[0] ?? Number.NaN, back);
    let nearLight = lightAt(bounds[0] ?? Number.NaN);
    brightness.push(extinctionPerMAt(bounds[0] ?? Number.NaN) * (MARCH_PIECE_SHADE + nearLight));
    for (let p = 0; p < shares.length; p += 1) {
      const far = capped(bounds[p + 1] ?? Number.NaN, back);
      const farLight = lightAt(bounds[p + 1] ?? Number.NaN);
      const depth = Math.max(back ? far - near : near - far, 0);
      const light = MARCH_PIECE_SHADE + 0.5 * (nearLight + farLight);
      const drop = Math.exp(-before) * (1 - Math.exp(-depth)) * light;
      const root = Math.sqrt((bounds[p + 1] ?? Number.NaN) - (bounds[p] ?? Number.NaN));
      drops.push(drop);
      roots.push(root);
      dropTotal += drop;
      rootTotal += root;
      before += depth;
      brightness.push(
        extinctionPerMAt(bounds[p + 1] ?? Number.NaN) *
          Math.exp(-before) *
          (MARCH_PIECE_SHADE + farLight),
      );
      near = far;
      nearLight = farLight;
    }
    if (!(dropTotal > 0)) {
      return null;
    }
    let total = 0;
    const weights = drops.map((drop, p) => {
      const weight = (0.5 * drop) / dropTotal + (0.5 * (roots[p] ?? Number.NaN)) / rootTotal;
      total += weight;
      return weight;
    });
    const steps = shares.reduce((sum, n) => sum + n, 0);
    return {
      shares: sharesOf(weights, total, steps, otherWithinSteps),
      towardStart: shares.map((_, p) => {
        const start = brightness[p] ?? Number.NaN;
        const end = brightness[p + 1] ?? Number.NaN;
        return placedTowardStart
          ? !(end > MARCH_PIECE_TURN * start)
          : start > MARCH_PIECE_TURN * end;
      }),
    };
  };
  const before = reshared(
    pieces.before,
    [-split.beforeM, ...fromLowestM.slice(0, crossingsBefore), 0],
    true,
    split.beforeTowardStart,
  );
  const after = reshared(
    pieces.after,
    [0, ...fromLowestM.slice(crossingsBefore), split.afterM],
    false,
    true,
  );
  return {
    ...pieces,
    before: before?.shares ?? pieces.before,
    after: after?.shares ?? pieces.after,
    beforeTowardStart: before?.towardStart ?? null,
    afterTowardStart: after?.towardStart ?? null,
  };
}

/**
 * The steps of a segment cut into pieces, counted from t_start (`common.wgsl`'s `marchWalkStep`):
 * on a side not cut, {@link marchStep}'s; on one that is, step k of each piece of length L and n
 * steps placed toward its end a, at a ∓ L (k² + k + ½) ÷ n², L (2k + 1) ÷ n² long. Each piece is
 * placed toward its end nearer t*, or, on a camera side placed toward the camera, toward its end
 * nearer the camera.
 */
export function marchPiecesSteps(pieces: MarchPieces): MarchStep[] {
  const { split, fromLowestM, crossingsBefore } = pieces;
  const steps: MarchStep[] = [];
  const cut = (
    shares: ReadonlyArray<number>,
    first: number,
    startM: number,
    endM: number,
    towardStart: boolean,
    placed: ReadonlyArray<boolean> | null,
  ): void => {
    const count = shares.length - 1;
    for (const [p, n] of shares.entries()) {
      const lowM = p === 0 ? startM : (fromLowestM[first + p - 1] ?? Number.NaN);
      const highM = p < count ? (fromLowestM[first + p] ?? Number.NaN) : endM;
      const unitM = (highM - lowM) / (n * n);
      const towardLow = placed?.[p] ?? towardStart;
      for (let j = 0; j < n; j += 1) {
        const k = towardLow ? j : n - 1 - j;
        const fromLowest = towardLow
          ? lowM + unitM * (k * k + k + 0.5)
          : highM - unitM * (k * k + k + 0.5);
        steps.push({ tM: split.lowestM + fromLowest, dtM: unitM * (2 * k + 1) });
      }
    }
  };
  if (pieces.before === null) {
    for (let i = 0; i < split.stepsBefore; i += 1) {
      steps.push(marchStep(split, i));
    }
  } else {
    cut(pieces.before, 0, -split.beforeM, 0, split.beforeTowardStart, pieces.beforeTowardStart);
  }
  if (pieces.after === null) {
    for (let i = split.stepsBefore; i < split.stepsBefore + split.stepsAfter; i += 1) {
      steps.push(marchStep(split, i));
    }
  } else {
    cut(pieces.after, crossingsBefore, 0, split.afterM, true, pieces.afterTowardStart);
  }
  return steps;
}

// --- The sun-aware warp of the ray march's two-sided sky segments (plan R08, R08.T8) -------------

/**
 * The span of the sun-aware warp in the cosine between a sky segment's ray and its march's first
 * source (plan R08, R08.T8): the warp a = clamp(−(d · s₀) ÷ 0.2, 0, 1) rises from 0 with the Sun 90°
 * from the ray to 1 with it about 101.5° from the ray. The ruling's probe's constant
 * (`decision-r08-t8-leans.md` §5.4), tuned only through the gate and its `last light` family;
 * `common.wgsl`'s `MARCH_SKY_WARP_COSINE`.
 */
export const SKY_WARP_COSINE = 0.2;

/**
 * The sun-aware warp a of a two-sided sky segment of the ray march (plan R08, R08.T8;
 * `common.wgsl`'s `marchSkyWarp`): clamp(−(d · s₀) ÷ {@link SKY_WARP_COSINE}, 0, 1), 0 with the
 * Sun ahead of the ray or across it, 1 with it behind, more than about 101.5° from the ray.
 *
 * @param direction - The ray's unit direction.
 * @param sun - The unit direction to the march's first source, or `null` where none is drawn,
 *   whose warp is 0.
 */
export function skyWarpOf(direction: Vec3, sun: Vec3 | null): number {
  if (sun === null) {
    return 0;
  }
  return Math.min(Math.max(-dot(direction, sun) / SKY_WARP_COSINE, 0), 1);
}

/**
 * Step k of n over a length warped by a: [t(k ÷ n), t((k + 1) ÷ n)] for t(u) = L ((1 − a) u + a u²),
 * as its midpoint's distance from the start, (L ÷ n)((1 − a)(k + ½) + a (k² + k + ½) ÷ n), and its
 * length, (L ÷ n)((1 − a) + a (2k + 1) ÷ n), in `common.wgsl`'s `marchWarpedStep`'s order. At a = 0
 * they are (k + ½) L ÷ n and L ÷ n to the bit; at a = 1 `marchStep`'s quadratic placement toward
 * the start.
 *
 * @param unitM - L ÷ n, m.
 */
function warpedStep(
  unitM: number,
  n: number,
  warp: number,
  k: number,
): { readonly fromStartM: number; readonly dtM: number } {
  const mid = (1 - warp) * (k + 0.5) + (warp * (k * k + k + 0.5)) / n;
  const width = 1 - warp + (warp * (2 * k + 1)) / n;
  return { fromStartM: unitM * mid, dtM: unitM * width };
}

/**
 * Where a point `fromLowestM` from t* stands in u on a split segment warped by a (`common.wgsl`'s
 * `marchWarpU`): the root of a u² + (1 − a) u = x, x = (t − t_start) ÷ L, as
 * 2x ÷ ((1 − a) + √((1 − a)² + 4 a x)), which does not cancel and is x at a = 0.
 */
function warpedU(split: MarchSplit, warp: number, fromLowestM: number): number {
  const x = (fromLowestM + split.beforeM) / (split.beforeM + split.afterM);
  const b = 1 - warp;
  return (2 * x) / (b + Math.sqrt(b * b + 4 * warp * x));
}

/**
 * Step `i` of a two-sided sky segment of the ray march, from inside the air or outside it, under the
 * sun-aware warp (plan R08, R08.T8; `common.wgsl`'s `marchEvenStep`): its n = `stepsBefore` +
 * `stepsAfter` steps evenly spaced in u ∈ [0, 1], t(u) = t_start + L ((1 − a) u + a u²), L the
 * segment's length, step i's midpoint and length those of [t(i ÷ n), t((i + 1) ÷ n)], its offset
 * from t* formed from the start's.
 *
 * @remarks
 * Every two-sided sky segment the ray march marches (nothing drawn along it) takes the sun-aware
 * warp: even where the Sun is ahead or across the ray (a = 0, `decision-r08-t8-leans.md` §4), R05's
 * one-sided placement from the segment's start where it is behind (a = 1, §5.4). Where the light
 * lies decides. With the Sun ahead it lies beyond the lowest point: on the night-side and
 * terminator limb from outside (the Sun 90–105° from the zenith at the tangent point), where the
 * placement about the lowest point missed R05.T12.e's gate (12.19% from 400 km) and even steps hold
 * it (4.85% at high's 36). With the Sun behind, at the camera's last light, it lies on the camera's
 * end of a night-side chord, its first 30–65 km of 950–1,200, where even steps put one midpoint or
 * none and drew the sky under the horizontal 45–124 codes off (63–97%) under a seam of 2–36 codes;
 * the warp holds it within 1.7 codes on high and 10.8 on low (§5.1, §5.4). The warp is continuous in
 * the ray's direction and is the same on both sides of the top, so neither the sky nor the top
 * (0.057%) steps. A one-sided segment, and a drawn surface's, keep R05.T12.e's placement.
 *
 * @param i - In [0, `stepsBefore` + `stepsAfter`).
 * @param warp - The segment's warp a ({@link skyWarpOf}), in [0, 1].
 */
export function marchEvenStep(split: MarchSplit, i: number, warp: number): MarchStep {
  const samples = split.stepsBefore + split.stepsAfter;
  const unitM = (split.beforeM + split.afterM) / samples;
  const { fromStartM, dtM } = warpedStep(unitM, samples, warp, i);
  const fromLowest = fromStartM - split.beforeM;
  return { tM: split.lowestM + fromLowest, dtM };
}

/**
 * The steps of a two-sided sky segment of the ray march under the sun-aware warp, cut into pieces
 * where it crosses the marches' layer edges (plan R08, R08.T8 on R08.T6.i's cuts; `common.wgsl`'s
 * `marchWalkEvenReady`, `marchWalkEvenEnter` and `marchWalkEvenStep`): each crossing's u found by
 * inverting t(u), the segment's steps shared among its pieces in proportion to each piece's length
 * in u, at least one a piece, each piece's count rounding its end's share of the rest, the last the
 * rest, and each piece's steps evenly spaced in u over it, each at its midpoint. On a piece from u₀
 * to u₁ the warp is t(u)'s own, the same quadratic in the piece's own u with a_p = a (u₁ − u₀) ÷
 * ((1 − a) + a (u₀ + u₁)), over the piece's length in t between its crossings. With no crossing, or
 * more pieces than steps, {@link marchEvenStep}'s steps over the whole segment. As in
 * {@link marchPieces}, the twin forms the shares in `f64` where the kernel forms them in `f32`.
 *
 * @param crossingsFromLowestM - Where the ray crosses the edges, as signed distances from t*, m, in
 *   any order; those not strictly inside the segment, at t* itself, and repeats are left out, as
 *   {@link marchPieces} leaves them.
 * @param warp - The segment's warp a ({@link skyWarpOf}), in [0, 1].
 */
export function marchEvenSteps(
  split: MarchSplit,
  crossingsFromLowestM: Iterable<number>,
  warp: number,
): MarchStep[] {
  const samples = split.stepsBefore + split.stepsAfter;
  const cuts = marchPieces(split, crossingsFromLowestM).fromLowestM;
  const pieces = cuts.length + 1;
  if (cuts.length === 0 || pieces > samples) {
    return Array.from({ length: samples }, (_, i) => marchEvenStep(split, i, warp));
  }
  const bounds = [-split.beforeM, ...cuts, split.afterM];
  const boundsU = [0, ...cuts.map((t) => warpedU(split, warp, t)), 1];
  const lengthsU = Array.from(
    { length: pieces },
    (_, p) => (boundsU[p + 1] ?? Number.NaN) - (boundsU[p] ?? Number.NaN),
  );
  const total = lengthsU.reduce((sum, length) => sum + length, 0);
  const rest = samples - pieces;
  let spent = 0;
  let cumulative = 0;
  const steps: MarchStep[] = [];
  for (const [p, lengthU] of lengthsU.entries()) {
    cumulative += lengthU;
    // floor(x + 0.5), as the WGSL rounds.
    const share = p + 1 < pieces ? Math.floor((rest * cumulative) / total + 0.5) : rest;
    const n = 1 + share - spent;
    spent = share;
    const lowM = bounds[p] ?? Number.NaN;
    const highM = bounds[p + 1] ?? Number.NaN;
    const lowU = boundsU[p] ?? Number.NaN;
    const highU = boundsU[p + 1] ?? Number.NaN;
    const pieceWarp = (warp * (highU - lowU)) / (1 - warp + warp * (lowU + highU));
    const unitM = (highM - lowM) / n;
    for (let k = 0; k < n; k += 1) {
      const { fromStartM, dtM } = warpedStep(unitM, n, pieceWarp, k);
      steps.push({ tM: split.lowestM + (lowM + fromStartM), dtM });
    }
  }
  return steps;
}
