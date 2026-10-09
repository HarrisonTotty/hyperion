/**
 * The per-frame marches' steps (plan R05, R05.T12.e, with addendum A of
 * `decision-r05-high-atmosphere.md`): the TypeScript twin of `marchSplit` and `marchStep` in
 * `shaders/source.wgsl`, which the sky-view and ray-march kernels take, and the quadrature gate's
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
 * The worst e of the twin at the low setting's 16 and 16 steps, per kernel and class, rounded up to
 * 0.1%: the bound the GPU agreement check holds low's outputs to, plus 1% for f32. Low's own gate is
 * only that its worst e over the gate's rays is no worse than even steps' at the same counts
 * (addendum A, 4(a)); `marchSteps.test.ts` holds the twin within these. The sky view's are split by
 * the camera's height, so that the limb from 60–100 km (addendum B), which 16 steps cannot resolve,
 * does not loosen the bound from below 50 km.
 */
export const LOW_TWIN_WORST = {
  /** The sky view from cameras up to 50 km. */
  skyView: { ordinary: 0.091, twilight: 0.175 },
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
