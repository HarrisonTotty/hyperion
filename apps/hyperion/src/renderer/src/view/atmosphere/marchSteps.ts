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
 */
export const LOW_TWIN_WORST = {
  /** The sky view from cameras up to 50 km. */
  skyView: { ordinary: 0.091, twilight: 0.176 },
  /** The sky view from cameras at 60–100 km. */
  skyViewHigh: { ordinary: 0.116, twilight: 0.495 },
  rayMarch: { ordinary: 0.049, twilight: 0.182 },
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
 * @param edgesM - A term's layer edges, m, in `layerEdgesM`'s pairs.
 */
export function marchEdgesM(edgesM: ReadonlyArray<number>, topHeightM: number): number[] {
  const middles: number[] = [];
  for (let i = 0; i + 1 < edgesM.length; i += 2) {
    const middle = 0.5 * ((edgesM[i] ?? Number.NaN) + (edgesM[i + 1] ?? Number.NaN));
    if (middle > 0 && middle < topHeightM) {
      middles.push(middle);
    }
  }
  return middles;
}

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
}

/**
 * One side's steps by piece (`common.wgsl`'s `marchSideShares`): `steps` among the pieces from
 * `startM` to `endM` from t* that the crossings `first` to `first + count` cut, one a piece and the
 * rest in proportion to the square root of each piece's length, each piece's count rounding its
 * end's share of the rest, the last the rest; `null` for a side that crosses no edge or has more
 * pieces than steps.
 */
function sideShares(
  fromLowestM: ReadonlyArray<number>,
  first: number,
  count: number,
  startM: number,
  endM: number,
  steps: number,
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
  const rest = steps - pieces;
  let spent = 0;
  let cumulative = 0;
  return roots.map((root, p) => {
    cumulative += root;
    // floor(x + 0.5), as the WGSL rounds.
    const share = p + 1 < pieces ? Math.floor((rest * cumulative) / total + 0.5) : rest;
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
 */
export function marchPieces(
  split: MarchSplit,
  crossingsFromLowestM: Iterable<number>,
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
  const before = sideShares(fromLowestM, 0, crossingsBefore, -split.beforeM, 0, split.stepsBefore);
  const after = sideShares(
    fromLowestM,
    crossingsBefore,
    crossingsAfter,
    0,
    split.afterM,
    split.stepsAfter,
  );
  const crowded =
    (crossingsBefore > 0 && before === null ? 1 : 0) +
    (crossingsAfter > 0 && after === null ? 1 : 0);
  return { split, fromLowestM, crossingsBefore, before, after, crowded };
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
    towardLow: boolean,
  ): void => {
    const count = shares.length - 1;
    for (const [p, n] of shares.entries()) {
      const lowM = p === 0 ? startM : (fromLowestM[first + p - 1] ?? Number.NaN);
      const highM = p < count ? (fromLowestM[first + p] ?? Number.NaN) : endM;
      const unitM = (highM - lowM) / (n * n);
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
    cut(pieces.before, 0, -split.beforeM, 0, split.beforeTowardStart);
  }
  if (pieces.after === null) {
    for (let i = split.stepsBefore; i < split.stepsBefore + split.stepsAfter; i += 1) {
      steps.push(marchStep(split, i));
    }
  } else {
    cut(pieces.after, crossingsBefore, 0, split.afterM, true);
  }
  return steps;
}
