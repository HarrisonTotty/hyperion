/**
 * The cloud volume's light (plan R11, R11.T7.c; Design note 10): the high setting's march through a
 * deck, its phase, its multiple-scattering octaves and their correction to δ-Eddington, the CPU
 * reference of `shaders/cloudVolume.wgsl`.
 *
 * @remarks
 * - **The density** is the layer's (`cloudDensity.ts`): a column's τ is the field's mean τ over the
 *   column's footprint, spread uniformly through the deck's depth, so that the two-dimensional layer
 *   is the independent-pixel approximation of the volume (Design note 10).
 * - **The phase** is two Henyey–Greenstein lobes whose weighted asymmetry is the condensate's g
 *   ({@link twoLobeOf}): a forward lobe half-way from g to 1 and a backward one at
 *   {@link BACKWARD_LOBE_ASYMMETRY} (lean, 2026-11-13).
 * - **Multiple scattering** is Wrenninge et al. 2013's octaves (as Hillaire 2016 takes them):
 *   octave i is a medium of extinction a^i σ on both the sun's path and the view's, of scattering
 *   b^i σ with b ≤ a, so that no octave adds light, and of phase asymmetry c^i
 *   ({@link CLOUD_OCTAVES}).
 * - **The correction** ({@link octaveCorrectionOf}): the octaves past the first, the multiple
 *   scattering, take a factor K(τ, μ₀) = (R_δE − R₀) ÷ (R_octaves − R₀), R₀ the first octave's single
 *   scattering, so that each column's flux reflectance is δ-Eddington's within the table's
 *   interpolation while its single scattering stays its own (lean, 2026-11-13): the octaves alone
 *   miss δ-Eddington by up to 59% ({@link CLOUD_OCTAVES}' remarks), a step a switch of setting
 *   would show. δ-Eddington is itself about 10% high at τ 1, g 0.85, μ₀ 1 against a converged Monte
 *   Carlo (10.0 ± 0.2% and 10.2 ± 0.15% in two 10⁷-photon runs; T7.a.1's records), within 2.6% at
 *   τ ≥ 10.
 * - **The night side**: a sample whose sun stands below its horizon, μ < −√(2h ÷ a) at height h,
 *   takes no sun; above that the sun's cosine is held at {@link CLOUD_VOLUME_LEAST_MU} at least.
 * - **The march** ({@link marchColumn}) integrates each step's in-scattering by Hillaire 2016's
 *   energy-conserving form, S σ_s (1 − e^(−σ_e Δ)) ÷ σ_e, the source held across the step.
 * - Pure; `f64`; no platform API.
 */
import type { Vec3 } from "../../geometry/vec3";
import { CLOUD_OCTAVES_MAX, cloudFieldAt } from "./cloudDensity";
import { deltaEddington } from "./cloudOptics";
import type { CloudLightInput } from "./cloudShadow";
import type { CloudOptics } from "./condensates";

/** The backward lobe's asymmetry (lean, 2026-11-13). */
export const BACKWARD_LOBE_ASYMMETRY = -0.3;

/** How far the forward lobe's asymmetry stands from g towards 1 (lean, 2026-11-13). */
export const FORWARD_LOBE_SHARE = 0.5;

/** A two-lobe Henyey–Greenstein phase function. */
export interface TwoLobePhase {
  /** g of the forward lobe, in (0, 1). */
  readonly forwardAsymmetry: number;
  /** g of the backward lobe, in (−1, 0]. */
  readonly backwardAsymmetry: number;
  /** The forward lobe's weight, in [0, 1]; the backward lobe's is the rest. */
  readonly forwardWeight: number;
}

/**
 * The two lobes of a condensate of asymmetry `g` in [0, 1): the forward lobe at
 * g + {@link FORWARD_LOBE_SHARE} (1 − g), the backward at {@link BACKWARD_LOBE_ASYMMETRY}, weighted
 * so that their weighted asymmetry is g.
 */
export function twoLobeOf(g: number): TwoLobePhase {
  const forwardAsymmetry = g + FORWARD_LOBE_SHARE * (1 - g);
  const backwardAsymmetry = BACKWARD_LOBE_ASYMMETRY;
  return {
    forwardAsymmetry,
    backwardAsymmetry,
    forwardWeight: (g - backwardAsymmetry) / (forwardAsymmetry - backwardAsymmetry),
  };
}

/** The weighted asymmetry of two lobes, w g_f + (1 − w) g_b. */
export function twoLobeAsymmetry(lobes: TwoLobePhase): number {
  return (
    lobes.forwardWeight * lobes.forwardAsymmetry +
    (1 - lobes.forwardWeight) * lobes.backwardAsymmetry
  );
}

/**
 * Henyey and Greenstein's (1941) phase function, sr⁻¹, normalised to 1 over the sphere:
 * (1 − g²) ÷ (4π (1 + g² − 2g cos Θ)^(3/2)).
 *
 * @param cosTheta - The cosine of the scattering angle, 1 straight on.
 */
export function henyeyGreenstein(cosTheta: number, g: number): number {
  return (1 - g * g) / (4 * Math.PI * (1 + g * g - 2 * g * cosTheta) ** 1.5);
}

/** The two lobes' phase, sr⁻¹, each lobe's asymmetry scaled by `scale` (an octave's c^i). */
export function twoLobePhase(cosTheta: number, lobes: TwoLobePhase, scale = 1): number {
  const w = lobes.forwardWeight;
  return (
    w * henyeyGreenstein(cosTheta, scale * lobes.forwardAsymmetry) +
    (1 - w) * henyeyGreenstein(cosTheta, scale * lobes.backwardAsymmetry)
  );
}

/** Wrenninge et al. 2013's octaves: their count and each one's scale on the last's. */
export interface CloudOctaves {
  readonly count: number;
  /** a, on the extinction, the sun's path's and the view's. */
  readonly extinction: number;
  /** b, on the scattering, at most a. */
  readonly scattering: number;
  /** c, on the phase's asymmetry. */
  readonly phase: number;
}

/**
 * The octaves, fitted by differential evolution to the minimax of R ÷ R_δE − 1 over τ 1, 1.5, 2,
 * 3, 5, 7, 10, 15, 20, 30 and 50 and μ₀ 0.5 to 1 by eighths, at liquid water's g 0.8654 and
 * 1 − ϖ₀ 5.4 × 10⁻⁷, with the two lobes of {@link twoLobeOf}, under b ≤ a (2026-11-14).
 *
 * @remarks
 * The fit puts b at a and c at 0: each octave past the first a conservative, isotropic medium of
 * a^i the extinction. Alone they miss δ-Eddington's flux reflectance by up to 59% (+59% at τ 1,
 * μ₀ 1; −59% at τ 50, μ₀ 1): octaves that add no light cannot reach a thick deck's albedo, and
 * fits that let b exceed a (a 22–23% miss) add light, unboundedly along a grazing view.
 * {@link octaveCorrectionOf} takes up the rest.
 */
export const CLOUD_OCTAVES: CloudOctaves = {
  count: 4,
  extinction: 0.1102,
  scattering: 0.1102,
  phase: 0,
};

/** The least zenith cosine the sun is taken at inside the deck (`cloudVolume.wgsl`'s). */
export const CLOUD_VOLUME_LEAST_MU = 0.02;

/** The least view cosine a column's march is taken at, against 0 ÷ 0 at a grazing view. */
export const LEAST_VIEW_MU = 1e-6;

/** Below this σ_e Δ, (1 − e^(−x)) ÷ x is taken by its series (`cloudVolume.wgsl`'s). */
export const ESCAPE_SERIES = 0.03;

/** (1 − e^(−x)) ÷ x for x ≥ 0, by its series 1 − x ÷ 2 + x² ÷ 6 − x³ ÷ 24 below {@link ESCAPE_SERIES}. */
export function escape(x: number): number {
  return x > ESCAPE_SERIES ? -Math.expm1(-x) / x : 1 - x * (0.5 - x * (1 / 6 - x / 24));
}

/** One column's march: a homogeneous slab seen from above. */
export interface ColumnMarchInput {
  /** The column's vertical τ. */
  readonly tau: number;
  /** The sun's zenith cosine, in (0, 1]. */
  readonly mu0: number;
  /** The view's zenith cosine, upward, in [0, 1]. */
  readonly muView: number;
  /** The cosine of the scattering angle from the sun's beam to the view. */
  readonly cosTheta: number;
  readonly optics: CloudOptics;
  /** The steps through the column. */
  readonly steps: number;
}

/** A column's light towards a view: its single scattering and its octaves' multiple scattering. */
export interface ColumnLight {
  /** The first octave's, per unit of the sun's irradiance face-on, sr⁻¹. */
  readonly single: number;
  /** The rest's, uncorrected, sr⁻¹. */
  readonly multiple: number;
}

/**
 * The radiance a column reflects towards the view by its octaves (the module's remarks), uncorrected:
 * the march from the top down through `steps` equal steps of vertical τ, each step's source the
 * sun's light at its middle.
 */
export function marchColumn(
  input: ColumnMarchInput,
  octaves: CloudOctaves = CLOUD_OCTAVES,
): ColumnLight {
  const { tau, mu0, cosTheta, optics, steps } = input;
  const muView = Math.max(input.muView, LEAST_VIEW_MU);
  const lobes = twoLobeOf(optics.asymmetry);
  const albedo = 1 - optics.coAlbedo;
  const dt = tau / steps;
  let single = 0;
  let multiple = 0;
  let a = 1;
  let b = 1;
  let c = 1;
  for (let i = 0; i < octaves.count; i += 1) {
    const phase = twoLobePhase(cosTheta, lobes, c);
    const viewStep = (a * dt) / muView;
    let radiance = 0;
    let transmittance = 1;
    for (let k = 0; k < steps; k += 1) {
      const sun = Math.exp((-a * (k + 0.5) * dt) / mu0);
      radiance += transmittance * b * albedo * phase * sun * (dt / muView) * escape(viewStep);
      transmittance *= Math.exp(-viewStep);
    }
    if (i === 0) {
      single = radiance;
    } else {
      multiple += radiance;
    }
    a *= octaves.extinction;
    b *= octaves.scattering;
    c *= octaves.phase;
  }
  return { single, multiple };
}

/** Gauss–Legendre's 16 nodes and weights on (0, 1), for the view's zenith cosine. */
const VIEW_NODES = (() => {
  const n = 16;
  const nodes: Array<{ readonly mu: number; readonly weight: number }> = [];
  for (let i = 1; i <= n; i += 1) {
    let x = Math.cos((Math.PI * (i - 0.25)) / (n + 0.5));
    let derivative = 0;
    for (let iteration = 0; iteration < 100; iteration += 1) {
      let p0 = 1;
      let p1 = x;
      for (let k = 2; k <= n; k += 1) {
        const p2 = ((2 * k - 1) * x * p1 - (k - 1) * p0) / k;
        p0 = p1;
        p1 = p2;
      }
      derivative = (n * (x * p1 - p0)) / (x * x - 1);
      const step = p1 / derivative;
      x -= step;
      if (Math.abs(step) < 1e-15) {
        break;
      }
    }
    nodes.push({ mu: 0.5 * (x + 1), weight: 1 / ((1 - x * x) * derivative * derivative) });
  }
  return nodes;
})();

/** The view's azimuths the flux is summed over. */
const VIEW_AZIMUTHS = 32;

/**
 * A column's flux reflectance from its radiance towards each upward direction, ∫ L μ dΩ ÷ μ₀, by 16
 * Gauss–Legendre nodes in μ and {@link VIEW_AZIMUTHS} azimuths: within 10⁻⁹ of a finer sum at
 * μ₀ ≥ 0.5, and 0.9% at 0.05.
 *
 * @param radiance - The radiance towards a view of zenith cosine μ at scattering cosine cos Θ, per
 *   unit of the sun's irradiance face-on, sr⁻¹.
 */
export function fluxReflectance(
  mu0: number,
  radiance: (muView: number, cosTheta: number) => number,
): number {
  const sun = Math.sqrt(Math.max(1 - mu0 * mu0, 0));
  let total = 0;
  for (const { mu, weight } of VIEW_NODES) {
    const across = Math.sqrt(Math.max(1 - mu * mu, 0));
    for (let j = 0; j < VIEW_AZIMUTHS; j += 1) {
      const phi = ((j + 0.5) * 2 * Math.PI) / VIEW_AZIMUTHS;
      const cosTheta = -mu0 * mu - sun * across * Math.cos(phi);
      total += radiance(mu, cosTheta) * mu * weight * ((2 * Math.PI) / VIEW_AZIMUTHS);
    }
  }
  return total / mu0;
}

/** A column's flux reflectance by its octaves: the single scattering's, and the rest's. */
export interface OctaveFlux {
  readonly single: number;
  readonly multiple: number;
}

/**
 * The octaves' flux reflectance of a homogeneous slab in closed form, the limit of
 * {@link marchColumn} as its steps grow: octave i's radiance b^i ϖ p_i μ₀ ÷ (a^i (μ + μ₀)) times
 * 1 − e^(−a^i τ (1 ÷ μ₀ + 1 ÷ μ)).
 */
export function octaveReflectance(
  tau: number,
  mu0: number,
  optics: CloudOptics,
  octaves: CloudOctaves = CLOUD_OCTAVES,
): OctaveFlux {
  const lobes = twoLobeOf(optics.asymmetry);
  const albedo = 1 - optics.coAlbedo;
  const part = (from: number, to: number): number =>
    fluxReflectance(mu0, (mu, cosTheta) => {
      let radiance = 0;
      let a = 1;
      let b = 1;
      let c = 1;
      for (let i = 0; i < to; i += 1) {
        if (i >= from) {
          radiance +=
            ((b * albedo * twoLobePhase(cosTheta, lobes, c) * mu0) / (a * (mu + mu0))) *
            -Math.expm1(-a * tau * (1 / mu0 + 1 / mu));
        }
        a *= octaves.extinction;
        b *= octaves.scattering;
        c *= octaves.phase;
      }
      return radiance;
    });
  return { single: part(0, 1), multiple: part(1, octaves.count) };
}

/** The correction table's τ samples, log-spaced over {@link CORRECTION_TAU_RANGE}. */
export const CORRECTION_TAUS = 32;
/** Its least and largest τ. */
export const CORRECTION_TAU_RANGE = [0.05, 10_000] as const;
/** Its μ₀ samples, evenly spaced from {@link CORRECTION_LEAST_MU} to 1. */
export const CORRECTION_MUS = 16;
/** Its least μ₀, the deck's least sun cosine. */
export const CORRECTION_LEAST_MU = CLOUD_VOLUME_LEAST_MU;
/** The largest correction taken, against a vanishing multiple scattering. */
export const CORRECTION_MAX = 50;

/** A condensate's correction of its octaves' multiple scattering to δ-Eddington. */
export interface OctaveCorrection {
  /** K at τ sample i and μ₀ sample j, at j × {@link CORRECTION_TAUS} + i. */
  readonly factors: Float32Array;
}

/** τ sample i of the correction table. */
export function correctionTau(i: number): number {
  const [least, most] = CORRECTION_TAU_RANGE;
  return least * (most / least) ** (i / (CORRECTION_TAUS - 1));
}

/** μ₀ sample j of the correction table. */
export function correctionMu(j: number): number {
  return CORRECTION_LEAST_MU + ((1 - CORRECTION_LEAST_MU) * j) / (CORRECTION_MUS - 1);
}

/**
 * A condensate's correction table (the module's remarks), K = (R_δE − R₀) ÷ R_multiple held to
 * [0, {@link CORRECTION_MAX}], `r32float` texels as the volume's texture holds them, made in some tens
 * of milliseconds on the CPU.
 */
export function octaveCorrectionOf(
  optics: CloudOptics,
  octaves: CloudOctaves = CLOUD_OCTAVES,
): OctaveCorrection {
  const factors = new Float32Array(CORRECTION_TAUS * CORRECTION_MUS);
  for (let j = 0; j < CORRECTION_MUS; j += 1) {
    const mu0 = correctionMu(j);
    for (let i = 0; i < CORRECTION_TAUS; i += 1) {
      const tau = correctionTau(i);
      const target = deltaEddington({
        tau,
        asymmetry: optics.asymmetry,
        singleScatterAlbedo: 1 - optics.coAlbedo,
        mu0,
      }).reflectance;
      const flux = octaveReflectance(tau, mu0, optics, octaves);
      const k = flux.multiple > 0 ? (target - flux.single) / flux.multiple : 0;
      factors[j * CORRECTION_TAUS + i] = Math.min(Math.max(k, 0), CORRECTION_MAX);
    }
  }
  return { factors };
}

/**
 * K at a column's τ and the sun's μ₀: bilinear in ln τ and μ₀ between the table's samples, held at
 * its edges (`cloudVolume.wgsl`'s `cloud_octave_correction_at`).
 */
export function octaveCorrectionAt(table: OctaveCorrection, tau: number, mu0: number): number {
  const [least, most] = CORRECTION_TAU_RANGE;
  const u = Math.log(Math.min(Math.max(tau, least), most) / least) / Math.log(most / least);
  const v =
    (Math.min(Math.max(mu0, CORRECTION_LEAST_MU), 1) - CORRECTION_LEAST_MU) /
    (1 - CORRECTION_LEAST_MU);
  const x = u * (CORRECTION_TAUS - 1);
  const y = v * (CORRECTION_MUS - 1);
  const i0 = Math.min(Math.floor(x), CORRECTION_TAUS - 2);
  const j0 = Math.min(Math.floor(y), CORRECTION_MUS - 2);
  const fx = x - i0;
  const fy = y - j0;
  const at = (i: number, j: number): number => table.factors[j * CORRECTION_TAUS + i] ?? 1;
  const top = at(i0, j0) + fx * (at(i0 + 1, j0) - at(i0, j0));
  const bottom = at(i0, j0 + 1) + fx * (at(i0 + 1, j0 + 1) - at(i0, j0 + 1));
  return top + fy * (bottom - top);
}

/** A column's corrected radiance: its single scattering and K times the rest. */
export function correctedColumn(light: ColumnLight, k: number): number {
  return light.single + k * light.multiple;
}

/** A deck's base and top above the datum, m. */
export interface CloudDeckSpan {
  readonly baseM: number;
  readonly topM: number;
}

/** A ray's distances to the spheroid |S x| = r, S = diag(1, 1, k), near then far; `null` if it misses. */
function spheroidHits(o: Vec3, d: Vec3, k: number, r: number): readonly [number, number] | null {
  const a = d.x * d.x + d.y * d.y + d.z * k * (d.z * k);
  const b = o.x * d.x + o.y * d.y + o.z * k * (d.z * k);
  const c = o.x * o.x + o.y * o.y + o.z * k * (o.z * k) - r * r;
  const disc = b * b - a * c;
  if (disc < 0) {
    return null;
  }
  const root = Math.sqrt(disc);
  return [(-b - root) / a, (-b + root) / a];
}

/** A view ray through a deck. */
export interface DeckRay {
  /** Where it starts, body-fixed, m. */
  readonly originM: Vec3;
  /** Its unit direction, body-fixed. */
  readonly direction: Vec3;
  /** The unit direction to the source that lights it, body-fixed. */
  readonly sun: Vec3;
  /** The equal steps of its first stretch through the deck. */
  readonly steps: number;
  /** Its footprint's growth with distance, m per m; the footprint is at least 1 m. */
  readonly footprintPerM: number;
}

/** One view ray's march through a deck. */
export interface DeckMarch {
  /** The radiance towards the camera per unit of the source's irradiance face-on, sr⁻¹. */
  readonly radiance: number;
  /** The view's transmittance through the deck. */
  readonly transmittance: number;
}

/**
 * A view ray's march through a body's deck (the module's remarks), the reference of
 * `cloud_volume_march`: each sample's column read on the layer's shell above it over the ray's
 * footprint there, its multiple scattering corrected by `correction`.
 */
export function marchDeckRay(
  clouds: CloudLightInput,
  deck: CloudDeckSpan,
  correction: OctaveCorrection,
  ray: DeckRay,
  octaves: CloudOctaves = CLOUD_OCTAVES,
): DeckMarch {
  const { originM: origin, direction, sun, steps, footprintPerM } = ray;
  const { shell, optics, datumRadiusM: datum } = clouds;
  const k = shell.equatorialRadiusM / shell.polarRadiusM;
  const outer = spheroidHits(origin, direction, k, datum + deck.topM);
  if (outer === null || outer[1] <= 0) {
    return { radiance: 0, transmittance: 1 };
  }
  const inner = spheroidHits(origin, direction, k, datum + deck.baseM) ?? [-1, -1];
  let t0 = Math.max(outer[0], 0);
  let t1 = outer[1];
  if (inner[0] > t0) {
    t1 = inner[0];
  } else if (inner[0] < 0 && inner[1] > t0) {
    t0 = inner[1];
  }
  const depth = Math.max(deck.topM - deck.baseM, 1);
  const dt = (t1 - t0) / steps;
  const cosTheta = sun.x * direction.x + sun.y * direction.y + sun.z * direction.z;
  const lobes = twoLobeOf(optics.asymmetry);
  const albedo = 1 - optics.coAlbedo;
  const view = Array.from({ length: octaves.count }, () => 1);
  let radiance = 0;
  for (let s = 0; s < steps; s += 1) {
    const t = t0 + (s + 0.5) * dt;
    const x = {
      x: origin.x + t * direction.x,
      y: origin.y + t * direction.y,
      z: origin.z + t * direction.z,
    };
    const scaled = Math.hypot(x.x, x.y, x.z * k);
    const height = scaled - datum;
    const onShell = {
      x: (x.x * shell.equatorialRadiusM) / scaled,
      y: (x.y * shell.equatorialRadiusM) / scaled,
      z: (x.z * shell.equatorialRadiusM) / scaled,
    };
    const column = cloudFieldAt(
      clouds.coverage,
      clouds.noise,
      clouds.params,
      onShell,
      Math.max(footprintPerM * t, 1),
      CLOUD_OCTAVES_MAX,
    ).meanTau;
    const sigma = column / depth;
    const n = { x: x.x, y: x.y, z: x.z * k * k };
    const rawMu = (n.x * sun.x + n.y * sun.y + n.z * sun.z) / Math.hypot(n.x, n.y, n.z);
    const lit = rawMu >= -Math.sqrt((2 * Math.max(height, 0)) / datum);
    const muSun = Math.max(rawMu, CLOUD_VOLUME_LEAST_MU);
    const above = column * Math.min(Math.max((deck.topM - height) / depth, 0), 1);
    const multiple = octaveCorrectionAt(correction, column, muSun);
    let a = 1;
    let b = 1;
    let c = 1;
    for (let i = 0; i < octaves.count; i += 1) {
      const viewStep = a * sigma * dt;
      const weight = i === 0 ? 1 : multiple;
      const source = lit
        ? weight * b * albedo * twoLobePhase(cosTheta, lobes, c) * Math.exp((-a * above) / muSun)
        : 0;
      radiance += (view[i] ?? 0) * source * sigma * dt * escape(viewStep);
      view[i] = (view[i] ?? 0) * Math.exp(-viewStep);
      a *= octaves.extinction;
      b *= octaves.scattering;
      c *= octaves.phase;
    }
  }
  return { radiance, transmittance: view[0] ?? 1 };
}
