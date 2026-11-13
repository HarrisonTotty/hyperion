/**
 * The near sea's Gerstner waves under one slope budget (plan R11, R11.T8.a; Design notes 11 and
 * 12): the components drawn from a sea's spectrum, and the slope variance they resolve, which the
 * BRDF's unresolved share makes up to Cox and Munk's total.
 *
 * @remarks
 * Pure: no state, no clock, `f64` throughout, the CPU reference for `oceanNear.wgsl`.
 *
 * - **The components** ({@link gerstnerSet}) are sampled geometrically in wavelength from twice
 *   the peak's down to the grid's cut, each at the centre of its share of the band in ln λ, its
 *   amplitude √(2 S(ω) Δω) over its share, its direction within ±{@link DIRECTION_SPREAD_RAD} of
 *   the wind by a fixed low-discrepancy sequence, and its phase by another, so that every console
 *   draws the same set for a sea. Its frequency is the dispersion's, ω² = gk + (γ ÷ ρ) k³, so that
 *   capillary components and other liquids are right.
 * - **The budget.** A component's resolved slope variance is 1 − √(1 − k²A²) along its direction
 *   (Bruneton, Neyret and Holzschuch 2010, eq. 4), shared between the wind's two axes as cos² and
 *   sin² of its angle from the wind. Where the sum on an axis would leave the BRDF less than the
 *   floor of 10⁻⁴ of Cox and Munk's total there, every amplitude is scaled down by one factor until
 *   it does not ({@link GerstnerSet.amplitudeScale}), so that resolved and unresolved always sum to
 *   the total ({@link unresolvedSlopeVariance}).
 * - **The limits.** Each kA is held to the Stokes wave's limiting steepness, 0.44 (Michell 1893's
 *   H ÷ L = 0.142, times π), and Σ Qᵢ kᵢ Aᵢ to 1, Finch's condition for no loops, by one Q for
 *   every component: 1, the trochoid's, unless the sum would pass 1.
 * - **The mean surface.** Gerstner's surface has zero mean height over its Lagrangian grid, the
 *   grid the GPU displaces, and an Eulerian mean that differs at second order, −Σ Qᵢ kᵢ Aᵢ² ÷ 2:
 *   the authoritative surface is the mean of the Lagrangian grid, the sea level itself (Design
 *   note 11).
 */
import type { SeaState } from "./seaState";
import { angularFrequencyOf, SLOPE_VARIANCE_FLOOR, spectralDensity } from "./seaState";

/** The largest kA a component takes: the Stokes wave's limiting steepness, 0.44 (Design note 12). */
export const STOKES_LIMIT = 0.44;

/** Finch's bound on Σ Qᵢ kᵢ Aᵢ, beyond which the summed surface loops: 1. */
export const FINCH_LIMIT = 1;

/**
 * The half-angle about the wind within which the components point, rad: 40°, inside Design note
 * 12's ±30–45°.
 */
export const DIRECTION_SPREAD_RAD = (40 * Math.PI) / 180;

/** The longest component's wavelength, in units of the peak's: twice it (Design note 12). */
export const LONGEST_OVER_PEAK = 2;

/** One Gerstner component, in the wind's frame: x along the wind, y across it. */
export interface GerstnerComponent {
  readonly wavenumberRadPerM: number;
  readonly amplitudeM: number;
  /** ω, rad/s, the dispersion's at k. */
  readonly angularFrequencyRadS: number;
  /** The direction it travels, rad from the wind towards +y. */
  readonly directionRad: number;
  /** Q, its share of the trochoid's horizontal motion, 0 to 1. */
  readonly steepness: number;
  readonly phaseRad: number;
}

/** A sea's Gerstner set. */
export interface GerstnerSet {
  readonly components: ReadonlyArray<GerstnerComponent>;
  /** The factor the slope budget scaled every amplitude by, 1 where it did not bind. */
  readonly amplitudeScale: number;
  /** The total slope variances the set was budgeted against, the sea state's. */
  readonly totalSlopeVariance: { readonly upwind: number; readonly crosswind: number };
}

/** The fractional part of a non-negative number. */
function fraction(x: number): number {
  return x - Math.floor(x);
}

/** The golden ratio's fractional part, the additive recurrence's step for directions. */
const DIRECTION_STEP = (Math.sqrt(5) - 1) / 2;

/** √2's fractional part, the phases' step, independent of the directions'. */
const PHASE_STEP = Math.SQRT2 - 1;

/** The slope variance a component of amplitude A and wavenumber k resolves along its direction. */
function componentSlopeVariance(k: number, amplitudeM: number): number {
  const ka = k * amplitudeM;
  return 1 - Math.sqrt(1 - ka * ka);
}

/** The resolved slope variance of components at amplitude scale `s`, per axis. */
function resolvedAt(
  components: ReadonlyArray<GerstnerComponent>,
  s: number,
): { upwind: number; crosswind: number } {
  let upwind = 0;
  let crosswind = 0;
  for (const c of components) {
    const v = componentSlopeVariance(c.wavenumberRadPerM, s * c.amplitudeM);
    const cos = Math.cos(c.directionRad);
    const sin = Math.sin(c.directionRad);
    upwind += v * cos * cos;
    crosswind += v * sin * sin;
  }
  return { upwind, crosswind };
}

/**
 * The Gerstner set of `count` components for a sea, down to the grid's shortest resolved
 * wavelength `cutWavelengthM`; empty where the sea is glassy or the cut is longer than the longest
 * component's.
 *
 * @param count - The components to draw: the setting's `oceanComponents`, 32 on high and 8 on low.
 * @param cutWavelengthM - The projected grid's cut, the shortest wavelength it resolves, m.
 */
export function gerstnerSet(state: SeaState, count: number, cutWavelengthM: number): GerstnerSet {
  const total = state.slopeVariance;
  const longest = (LONGEST_OVER_PEAK * 2 * Math.PI) / state.peakWavenumberRadPerM;
  if (state.glassy || !(count > 0) || !(cutWavelengthM < longest) || !(longest > 0)) {
    return { components: [], amplitudeScale: 1, totalSlopeVariance: total };
  }
  const g = state.gravityMps2;
  const capillary = state.capillaryM3PerS2;
  const lnLongest = Math.log(longest);
  const lnSpan = Math.log(cutWavelengthM) - lnLongest;
  const omegaAt = (lnLambda: number): number =>
    angularFrequencyOf((2 * Math.PI) / Math.exp(lnLambda), g, capillary);
  const raw: GerstnerComponent[] = [];
  for (let i = 0; i < count; i++) {
    const lnLambda = lnLongest + ((i + 0.5) / count) * lnSpan;
    const k = (2 * Math.PI) / Math.exp(lnLambda);
    const omega = angularFrequencyOf(k, g, capillary);
    const width =
      omegaAt(lnLongest + ((i + 1) / count) * lnSpan) - omegaAt(lnLongest + (i / count) * lnSpan);
    const amplitude = Math.min(
      Math.sqrt(2 * spectralDensity(state, omega) * width),
      STOKES_LIMIT / k,
    );
    raw.push({
      wavenumberRadPerM: k,
      amplitudeM: amplitude,
      angularFrequencyRadS: omega,
      directionRad: DIRECTION_SPREAD_RAD * (2 * fraction((i + 1) * DIRECTION_STEP) - 1),
      steepness: 1,
      phaseRad: 2 * Math.PI * fraction((i + 1) * PHASE_STEP),
    });
  }
  const budget = {
    upwind: total.upwind - SLOPE_VARIANCE_FLOOR,
    crosswind: total.crosswind - SLOPE_VARIANCE_FLOOR,
  };
  const fits = (s: number): boolean => {
    const r = resolvedAt(raw, s);
    return r.upwind <= budget.upwind && r.crosswind <= budget.crosswind;
  };
  let scale = 1;
  if (!fits(1)) {
    // The resolved variance rises with the scale, so the largest scale that fits is found by
    // bisection, kept on the side that fits.
    let low = 0;
    let high = 1;
    for (let i = 0; i < 64; i++) {
      const mid = 0.5 * (low + high);
      if (fits(mid)) {
        low = mid;
      } else {
        high = mid;
      }
    }
    scale = low;
  }
  const sum = raw.reduce((acc, c) => acc + c.wavenumberRadPerM * c.amplitudeM * scale, 0);
  const build = (q: number): GerstnerSet => ({
    components: raw.map((c): GerstnerComponent => ({
      wavenumberRadPerM: c.wavenumberRadPerM,
      amplitudeM: c.amplitudeM * scale,
      angularFrequencyRadS: c.angularFrequencyRadS,
      directionRad: c.directionRad,
      steepness: q,
      phaseRad: c.phaseRad,
    })),
    amplitudeScale: scale,
    totalSlopeVariance: total,
  });
  let q = sum > FINCH_LIMIT ? FINCH_LIMIT / sum : 1;
  let set = build(q);
  // A quotient may round a few units above the limit; steps down keep the sum within it.
  while (summedSteepness(set) > FINCH_LIMIT) {
    q *= 1 - 4 * Number.EPSILON;
    set = build(q);
  }
  return set;
}

/** Σ Qᵢ kᵢ Aᵢ, Finch's sum, at most 1 for every set {@link gerstnerSet} makes. */
export function summedSteepness(set: GerstnerSet): number {
  return set.components.reduce(
    (acc, c) => acc + c.steepness * c.wavenumberRadPerM * c.amplitudeM,
    0,
  );
}

/** The slope variance the set resolves along the wind and across it. */
export function resolvedSlopeVariance(set: GerstnerSet): {
  readonly upwind: number;
  readonly crosswind: number;
} {
  return resolvedAt(set.components, 1);
}

/**
 * The slope variance the BRDF keeps along the wind and across it: the total less what the set
 * resolves, never below the floor of 10⁻⁴.
 */
export function unresolvedSlopeVariance(set: GerstnerSet): {
  readonly upwind: number;
  readonly crosswind: number;
} {
  const resolved = resolvedSlopeVariance(set);
  return {
    upwind: Math.max(set.totalSlopeVariance.upwind - resolved.upwind, SLOPE_VARIANCE_FLOOR),
    crosswind: Math.max(
      set.totalSlopeVariance.crosswind - resolved.crosswind,
      SLOPE_VARIANCE_FLOOR,
    ),
  };
}

/**
 * The displaced place of the Lagrangian grid's point (x₀, y₀) at time t, m, in the wind's frame:
 * x₀ − Σ Qᵢ Aᵢ dᵢ sin θᵢ across, Σ Aᵢ cos θᵢ up, θᵢ = kᵢ dᵢ · (x₀, y₀) − ωᵢ t + φᵢ.
 */
export function gerstnerDisplacement(
  set: GerstnerSet,
  x0M: number,
  y0M: number,
  timeS: number,
): readonly [number, number, number] {
  let x = x0M;
  let y = y0M;
  let z = 0;
  for (const c of set.components) {
    const dx = Math.cos(c.directionRad);
    const dy = Math.sin(c.directionRad);
    const theta =
      c.wavenumberRadPerM * (dx * x0M + dy * y0M) - c.angularFrequencyRadS * timeS + c.phaseRad;
    const s = Math.sin(theta);
    x -= c.steepness * c.amplitudeM * dx * s;
    y -= c.steepness * c.amplitudeM * dy * s;
    z += c.amplitudeM * Math.cos(theta);
  }
  return [x, y, z];
}

/**
 * The Jacobian determinant of the grid's horizontal displacement at (x₀, y₀, t): the area a unit of
 * the Lagrangian grid covers, which weighs a Lagrangian average into an Eulerian one.
 */
export function gerstnerJacobian(
  set: GerstnerSet,
  x0M: number,
  y0M: number,
  timeS: number,
): number {
  let xx = 1;
  let yy = 1;
  let xy = 0;
  for (const c of set.components) {
    const dx = Math.cos(c.directionRad);
    const dy = Math.sin(c.directionRad);
    const theta =
      c.wavenumberRadPerM * (dx * x0M + dy * y0M) - c.angularFrequencyRadS * timeS + c.phaseRad;
    const f = c.steepness * c.amplitudeM * c.wavenumberRadPerM * Math.cos(theta);
    xx -= f * dx * dx;
    yy -= f * dy * dy;
    xy -= f * dx * dy;
  }
  return xx * yy - xy * xy;
}

/** The Eulerian mean height of the set to second order, −Σ Qᵢ kᵢ Aᵢ² ÷ 2, m. */
export function eulerianMeanHeightM(set: GerstnerSet): number {
  return set.components.reduce(
    (acc, c) => acc - (c.steepness * c.wavenumberRadPerM * c.amplitudeM * c.amplitudeM) / 2,
    0,
  );
}
