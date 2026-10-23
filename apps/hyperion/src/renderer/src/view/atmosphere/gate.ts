/**
 * Design note 10's metric, the gate every atmosphere table is held to against the reference path
 * tracer (plan R08, R08.T12.b.1), with its statistics, its direct-beam checks and the diagnostics
 * it records beside it.
 *
 * @remarks
 * - **The metric** (the metric ruling, decision-backlog-1 item 2, 2026-10-09): for every geometry
 *   g of a case and channel c, |L_client − L_ref| ≤ T + 4σ_ref, with
 *   T = max(0.05 L_ref, F L_max(I(g), c)) and F = 3 × 10⁻⁴. I(g) is g's image, the geometries
 *   that share its observer and its suns, and L_max its brightest diffuse radiance in c, so that a
 *   twilight image is held to its own light and the floor takes over below 20F, 6 × 10⁻³ of the
 *   image's brightest. Only diffuse radiance is compared; the direct beam has its own two checks.
 *   The constants are named here, so that a later ruling changes one line, and a regime that cannot
 *   reach the gate stays `ATMOSPHERE: APPROXIMATE`: the gate is never loosened for it.
 * - **The statistics** (the statistics ruling, `decision-r08-t4b-t12c.md`, 2026-10-10, and its
 *   follow-up (h)): every comparison is a tolerance test, |ΔL| ≤ T + 4σ_ref, with no family-wise
 *   widening and no χ². The reference is refused, not held, where σ_ref > T ÷ 8, or where its σ
 *   does not fall as N^(−1/2): beyond 25% of its first quarter's σ_q √(N_q ÷ N), which the
 *   reference records (`decision-r08-t12b-scope.md`, ruling 6), so that the reference alone
 *   decides its refusals. A failure is a finding, and a reference is never re-traced to pass.
 * - **The aggregates**: |ΔF| ≤ 0.02 F_ref + 4σ_F with σ_F ≤ 0.0025 F_ref; a sanity range [a, b]
 *   as a − 4σ ≤ F ≤ b + 4σ with σ ≤ (b − a) ÷ 16.
 * - **The direct beam** (`decision-r08-spheroid.md`, 2026-10-10): the reference's
 *   `sunOpticalDepth` equals the client's own `f64` quadrature along the same chord to
 *   10⁻⁶ max(1, τ); and the client's beam, as a radiance E_c e^(−τ) ÷ π, the radiance of a white
 *   Lambertian surface facing the sun, is held under the same T with the image's L_max and no σ
 *   term, so that a beam under the image's floor passes.
 * - **Recorded, never gated**: Δu′v′; per image, the largest ΔE*ab through R07's `toneCurve` at
 *   the AVG meter's exposure, the image's mean diffuse luminance taken as the metered average;
 *   IPRT's relative RMS difference, (Σ(L_client − L_ref)²)^½ ÷ (Σ L_ref²)^½ (Emde et al. 2015,
 *   JQSRT 164, 8, eq. 6); and each image's L_max ÷ mean.
 *
 * Both pipelines average over the detector cone the reference states, uniformly in solid angle as
 * the tracer draws it ({@link coneAverage}). It runs no twin: applying it to the CPU twin is
 * R08.T13's.
 */

import { cross, normalise, type Vec3 } from "../../geometry/vec3";
import { gaussLegendre } from "../lighting/quadrature";
import { ev100FromAverageLuminance, exposureScale } from "../photometry/exposure";
import { type Rgb, toneCurve } from "../photometry/toneCurve";
import { REC709_TO_XYZ } from "./spectralColour";

/** The metric's relative tolerance: 5% of the reference's radiance. */
export const METRIC_RELATIVE_TOLERANCE = 0.05;

/** The metric's floor, F: 3 × 10⁻⁴ of the image's brightest diffuse radiance. */
export const METRIC_FLOOR = 3e-4;

/** The reference's standard errors the tolerance allows: 4. */
export const METRIC_SIGMA_ALLOWANCE = 4;

/** The reference is refused where its σ exceeds T over this: 8. */
export const METRIC_SIGMA_DIVISOR = 8;

/** The aggregates' relative tolerance: 2% of the reference's flux. */
export const AGGREGATE_RELATIVE_TOLERANCE = 0.02;

/** The largest σ an aggregate's reference may have, relative to its flux: 0.25%. */
export const AGGREGATE_SIGMA_CAP = 0.0025;

/** A sanity range [a, b] refuses a σ above (b − a) over this: 16. */
export const SANITY_SIGMA_DIVISOR = 16;

/**
 * How far a σ may stand from its first quarter's σ_q √(N_q ÷ N), relatively, before the reference
 * is refused as not falling as N^(−1/2): 25% (`benchmarks.rs`'s `SIGMA_SCALING_TOLERANCE`).
 */
export const SIGMA_SCALING_TOLERANCE = 0.25;

/** The reference beam's depth agrees with the client's quadrature to this times max(1, τ). */
export const BEAM_DEPTH_TOLERANCE = 1e-6;

/** A channel: red, green or blue. */
export type Channel = 0 | 1 | 2;

const CHANNELS: readonly Channel[] = [0, 1, 2];

/** One geometry of a reference, in its image, in the three channels. */
export interface ReferenceReading {
  readonly name: string;
  /** I(g), the image it is held in (`reference/geometries.ts`). */
  readonly image: string;
  /** L_ref: diffuse radiance, in the case's irradiance unit per steradian. */
  readonly radiance: Rgb;
  /** σ_ref over the whole run. */
  readonly standardError: Rgb;
  /** N. */
  readonly samples: number;
  /** N_q, the first quarter's samples. */
  readonly quarterSamples: number;
  /** σ_q over the first quarter. */
  readonly quarterStandardError: Rgb;
}

/** Each image's brightest diffuse radiance per channel, L_max(I, c). */
export function imageMaxima(readings: ReadonlyArray<ReferenceReading>): ReadonlyMap<string, Rgb> {
  const maxima = new Map<string, Rgb>();
  for (const r of readings) {
    const seen = maxima.get(r.image) ?? [0, 0, 0];
    maxima.set(r.image, [
      Math.max(seen[0], r.radiance[0]),
      Math.max(seen[1], r.radiance[1]),
      Math.max(seen[2], r.radiance[2]),
    ]);
  }
  return maxima;
}

/** T = max(0.05 L_ref, F L_max): the metric's tolerance at a radiance in an image. */
export function metricTolerance(referenceRadiance: number, imageMaximum: number): number {
  return Math.max(METRIC_RELATIVE_TOLERANCE * referenceRadiance, METRIC_FLOOR * imageMaximum);
}

/**
 * Whether a standard error falls as N^(−1/2) from its first quarter's: within
 * {@link SIGMA_SCALING_TOLERANCE} of σ_q √(N_q ÷ N); a value exact at both counts, σ 0 at each,
 * does.
 */
export function sigmaScales(
  standardError: number,
  quarterStandardError: number,
  samples: number,
  quarterSamples: number,
): boolean {
  if (standardError === 0 && quarterStandardError === 0) {
    return true;
  }
  const expected = quarterStandardError * Math.sqrt(quarterSamples / samples);
  return Math.abs(standardError - expected) <= SIGMA_SCALING_TOLERANCE * expected;
}

/** Why a reference is refused at a geometry and channel. */
export type RefusalReason = "noisy" | "unscaled";

/** A geometry and channel at which the reference is refused, not held. */
export interface Refusal {
  readonly name: string;
  readonly image: string;
  readonly channel: Channel;
  readonly reason: RefusalReason;
  readonly standardError: number;
  /** The metric's T there. */
  readonly tolerance: number;
  /** σ_q √(N_q ÷ N), the σ the first quarter predicts. */
  readonly expectedStandardError: number;
  /** The count at which σ would meet T ÷ 8 by N^(−1/2): N (σ ÷ (T ÷ 8))². */
  readonly neededSamples: number;
}

/**
 * The reference's refusals (the module's remarks): σ_ref > T ÷ 8 (`noisy`), or σ not within 25% of
 * its first quarter's prediction (`unscaled`), the first named where both hold.
 */
export function refusals(readings: ReadonlyArray<ReferenceReading>): Refusal[] {
  const maxima = imageMaxima(readings);
  const refused: Refusal[] = [];
  for (const r of readings) {
    const maximum = maxima.get(r.image) ?? [0, 0, 0];
    for (const c of CHANNELS) {
      const sigma = r.standardError[c];
      const tolerance = metricTolerance(r.radiance[c], maximum[c]);
      const cap = tolerance / METRIC_SIGMA_DIVISOR;
      const expected = r.quarterStandardError[c] * Math.sqrt(r.quarterSamples / r.samples);
      let reason: RefusalReason | undefined;
      if (sigma > cap) {
        reason = "noisy";
      } else if (!sigmaScales(sigma, r.quarterStandardError[c], r.samples, r.quarterSamples)) {
        reason = "unscaled";
      }
      if (reason !== undefined) {
        refused.push({
          name: r.name,
          image: r.image,
          channel: c,
          reason,
          standardError: sigma,
          tolerance,
          expectedStandardError: expected,
          neededSamples: Math.ceil(r.samples * (sigma / cap) ** 2),
        });
      }
    }
  }
  return refused;
}

/** One comparison of the gate: a geometry in one channel. */
export interface Comparison {
  readonly name: string;
  readonly image: string;
  readonly channel: Channel;
  readonly client: number;
  readonly reference: number;
  readonly standardError: number;
  /** T. */
  readonly tolerance: number;
  /** T + 4σ_ref. */
  readonly bound: number;
  /** |L_client − L_ref| ≤ T + 4σ_ref. */
  readonly passes: boolean;
  /** The reference is refused here ({@link refusals}). */
  readonly refused: boolean;
}

/** The gate over a case's geometries. */
export interface GateReport {
  readonly comparisons: ReadonlyArray<Comparison>;
  readonly refusals: ReadonlyArray<Refusal>;
  /**
   * The comparisons that fail where the reference is not refused: a refused geometry is reported
   * in {@link GateReport.refusals}, not held. From R08.T14.d on a refused geometry does not pass,
   * which that task takes from the refusals; this list does not change for it.
   */
  readonly failures: ReadonlyArray<Comparison>;
}

/**
 * The metric over a case's geometries (the module's remarks), the client's radiance of each by its
 * name.
 *
 * @throws Error for a geometry the client gives no radiance of.
 */
export function gate(
  readings: ReadonlyArray<ReferenceReading>,
  client: ReadonlyMap<string, Rgb>,
): GateReport {
  const maxima = imageMaxima(readings);
  const refused = refusals(readings);
  const refusedAt = new Set(refused.map((r) => `${r.name}\u0000${r.channel}`));
  const comparisons: Comparison[] = [];
  for (const r of readings) {
    const mine = client.get(r.name);
    if (mine === undefined) {
      throw new Error(`the client gives no radiance of ${r.name}`);
    }
    const maximum = maxima.get(r.image) ?? [0, 0, 0];
    for (const c of CHANNELS) {
      const tolerance = metricTolerance(r.radiance[c], maximum[c]);
      const bound = tolerance + METRIC_SIGMA_ALLOWANCE * r.standardError[c];
      comparisons.push({
        name: r.name,
        image: r.image,
        channel: c,
        client: mine[c],
        reference: r.radiance[c],
        standardError: r.standardError[c],
        tolerance,
        bound,
        passes: Math.abs(mine[c] - r.radiance[c]) <= bound,
        refused: refusedAt.has(`${r.name}\u0000${c}`),
      });
    }
  }
  return {
    comparisons,
    refusals: refused,
    failures: comparisons.filter((c) => !c.passes && !c.refused),
  };
}

/** The Gauss–Legendre nodes in cos θ a cone's average takes by default. */
export const CONE_POLAR_NODES = 8;

/** The azimuths about the axis a cone's average takes by default. */
export const CONE_AZIMUTH_NODES = 16;

/**
 * A radiance averaged over a detector cone of half-angle `halfAngleRad` about the unit vector
 * `axis`, uniformly in solid angle, as the tracer draws its detector's directions
 * (`atmosphere/geometry.rs`'s `in_cone`): Gauss–Legendre in cos θ over [cos α, 1] times even
 * azimuths about the axis, exact for a radiance polynomial in the direction to the rules' orders.
 * A cone of zero width is its axis.
 */
export function coneAverage(
  radianceAt: (direction: Vec3) => Rgb,
  axis: Vec3,
  halfAngleRad: number,
  polarNodes: number = CONE_POLAR_NODES,
  azimuthNodes: number = CONE_AZIMUTH_NODES,
): Rgb {
  if (halfAngleRad <= 0) {
    return radianceAt(axis);
  }
  const k = normalise(axis);
  const helper = Math.abs(k.x) < 0.9 ? { x: 1, y: 0, z: 0 } : { x: 0, y: 1, z: 0 };
  const u = normalise(cross(k, helper));
  const v = cross(k, u);
  const rule = gaussLegendre(polarNodes);
  const lowest = Math.cos(halfAngleRad);
  const sum: [number, number, number] = [0, 0, 0];
  let total = 0;
  for (const [i, x] of rule.x.entries()) {
    const cosTheta = lowest + (1 - lowest) * 0.5 * (x + 1);
    const sinTheta = Math.sqrt(Math.max(1 - cosTheta * cosTheta, 0));
    const weight = rule.w[i] ?? Number.NaN;
    for (let j = 0; j < azimuthNodes; j += 1) {
      const phi = (2 * Math.PI * (j + 0.5)) / azimuthNodes;
      const s = sinTheta * Math.cos(phi);
      const t = sinTheta * Math.sin(phi);
      const radiance = radianceAt({
        x: k.x * cosTheta + u.x * s + v.x * t,
        y: k.y * cosTheta + u.y * s + v.y * t,
        z: k.z * cosTheta + u.z * s + v.z * t,
      });
      sum[0] += weight * radiance[0];
      sum[1] += weight * radiance[1];
      sum[2] += weight * radiance[2];
      total += weight;
    }
  }
  return [sum[0] / total, sum[1] / total, sum[2] / total];
}

/** The direct beam as a radiance, E_c e^(−τ) ÷ π: a white Lambertian surface facing the sun. */
export function beamRadiance(irradiance: number, opticalDepth: number): number {
  return (irradiance * Math.exp(-opticalDepth)) / Math.PI;
}

/** Whether the reference's beam depth equals the client's quadrature to 10⁻⁶ max(1, τ). */
export function beamDepthAgrees(referenceDepth: number, clientDepth: number): boolean {
  return (
    Math.abs(referenceDepth - clientDepth) <= BEAM_DEPTH_TOLERANCE * Math.max(1, referenceDepth)
  );
}

/**
 * Whether the client's beam, as a radiance, meets the metric's T with the image's L_max and no σ
 * term, the reference's beam being a quadrature: a beam under the image's floor passes.
 */
export function beamPasses(
  clientBeamRadiance: number,
  referenceBeamRadiance: number,
  imageMaximum: number,
): boolean {
  return (
    Math.abs(clientBeamRadiance - referenceBeamRadiance) <=
    metricTolerance(referenceBeamRadiance, imageMaximum)
  );
}

/** One aggregate of a reference in the three channels: a flux, and its statistics. */
export interface FluxReading {
  readonly name: string;
  /** F_ref: the downwelling flux at the ground (direct and diffuse) or the plane albedo's flux. */
  readonly flux: Rgb;
  /** σ_F. */
  readonly standardError: Rgb;
  readonly samples: number;
  readonly quarterSamples: number;
  readonly quarterStandardError: Rgb;
}

/** Why an aggregate's reference is refused in a channel: σ_F > 0.0025 F, or not N^(−1/2). */
export function aggregateRefusal(
  reading: FluxReading,
  channel: Channel,
): RefusalReason | undefined {
  const sigma = reading.standardError[channel];
  if (sigma > AGGREGATE_SIGMA_CAP * reading.flux[channel]) {
    return "noisy";
  }
  return sigmaScales(
    sigma,
    reading.quarterStandardError[channel],
    reading.samples,
    reading.quarterSamples,
  )
    ? undefined
    : "unscaled";
}

/** Whether a client's aggregate meets |ΔF| ≤ 0.02 F_ref + 4σ_F in a channel. */
export function aggregatePasses(client: number, reading: FluxReading, channel: Channel): boolean {
  const reference = reading.flux[channel];
  return (
    Math.abs(client - reference) <=
    AGGREGATE_RELATIVE_TOLERANCE * reference +
      METRIC_SIGMA_ALLOWANCE * reading.standardError[channel]
  );
}

/** A sanity range's verdict on a reference's value. */
export interface SanityVerdict {
  /** σ > (b − a) ÷ 16: the range is refused, not held. */
  readonly refused: boolean;
  /** a − 4σ ≤ F ≤ b + 4σ. */
  readonly within: boolean;
}

/** A sanity range [a, b] held as a tolerance test (the module's remarks). */
export function sanityRange(
  value: number,
  standardError: number,
  low: number,
  high: number,
): SanityVerdict {
  const allowance = METRIC_SIGMA_ALLOWANCE * standardError;
  return {
    refused: standardError > (high - low) / SANITY_SIGMA_DIVISOR,
    within: value >= low - allowance && value <= high + allowance,
  };
}

// --- The recorded diagnostics ------------------------------------------------------------------

function xyzOf(rgb: Rgb): Rgb {
  const row = (r: Rgb): number => r[0] * rgb[0] + r[1] * rgb[1] + r[2] * rgb[2];
  return [row(REC709_TO_XYZ[0]), row(REC709_TO_XYZ[1]), row(REC709_TO_XYZ[2])];
}

/** A linear Rec. 709 colour's luminance, its Y. */
export function luminanceOf(rgb: Rgb): number {
  return xyzOf(rgb)[1];
}

/**
 * CIE 1976 u′v′ of a linear colour: (4X, 9Y) ÷ (X + 15Y + 3Z) (CIE 15:2004; its §8.1 from memory,
 * the section not re-read, the formula checked against CIE 15's as others print it).
 */
function uvPrime(rgb: Rgb): readonly [number, number] {
  const [x, y, z] = xyzOf(rgb);
  const d = x + 15 * y + 3 * z;
  return [(4 * x) / d, (9 * y) / d];
}

/** Δu′v′ between two linear Rec. 709 colours: recorded, never gated. */
export function uvPrimeDifference(a: Rgb, b: Rgb): number {
  const [ua, va] = uvPrime(a);
  const [ub, vb] = uvPrime(b);
  return Math.hypot(ua - ub, va - vb);
}

/**
 * CIE 1976 L*a*b* of a display-linear Rec. 709 colour, the display's white its white point (CIE
 * 15:2004; its §8.2.1 from memory, the section not re-read, the formula checked against CIE 15's
 * (24/116)³ threshold, 841/108 slope and 16/116 offset).
 */
function labOf(display: Rgb): Rgb {
  const white = xyzOf([1, 1, 1]);
  const [x, y, z] = xyzOf(display);
  const delta = 6 / 29;
  const f = (t: number): number =>
    t > delta ** 3 ? Math.cbrt(t) : t / (3 * delta * delta) + 4 / 29;
  const fx = f(x / white[0]);
  const fy = f(y / white[1]);
  const fz = f(z / white[2]);
  return [116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)];
}

/**
 * ΔE*ab between two radiances as R07's AgX shows them at the AVG meter's exposure for an image of
 * mean luminance `meanLuminance` (`exposure.ts`: the mean at 0.104 of the white point).
 */
export function deltaEab(a: Rgb, b: Rgb, meanLuminance: number): number {
  const scale = exposureScale(ev100FromAverageLuminance(meanLuminance));
  const shown = (rgb: Rgb): Rgb =>
    labOf(toneCurve([rgb[0] * scale, rgb[1] * scale, rgb[2] * scale]));
  const [la, aa, ba] = shown(a);
  const [lb, ab, bb] = shown(b);
  return Math.hypot(la - lb, aa - ab, ba - bb);
}

/** An image's recorded diagnostics. */
export interface ImageDiagnostics {
  /** The largest ΔE*ab over the image (the module's remarks). */
  readonly largestDeltaE: number;
  /** IPRT's relative RMS difference per channel. */
  readonly relativeRms: Rgb;
  /** L_max ÷ the image's mean diffuse radiance, per channel. */
  readonly maxOverMean: Rgb;
}

/**
 * Each image's recorded diagnostics (the module's remarks), the client's radiance of each geometry
 * by its name.
 *
 * @throws Error for a geometry the client gives no radiance of.
 */
export function imageDiagnostics(
  readings: ReadonlyArray<ReferenceReading>,
  client: ReadonlyMap<string, Rgb>,
): ReadonlyMap<string, ImageDiagnostics> {
  const images = new Map<string, ReferenceReading[]>();
  for (const r of readings) {
    images.set(r.image, [...(images.get(r.image) ?? []), r]);
  }
  const out = new Map<string, ImageDiagnostics>();
  for (const [image, members] of images) {
    const meanLuminance =
      members.reduce((sum, r) => sum + luminanceOf(r.radiance), 0) / members.length;
    const largestDeltaE = Math.max(
      ...members.map((r) => deltaEab(clientRadiance(client, r), r.radiance, meanLuminance)),
    );
    const relativeRms = perChannel((c) => {
      const difference = members.reduce(
        (s, r) => s + (clientRadiance(client, r)[c] - r.radiance[c]) ** 2,
        0,
      );
      const reference = members.reduce((s, r) => s + r.radiance[c] ** 2, 0);
      return Math.sqrt(difference / reference);
    });
    const maxOverMean = perChannel((c) => {
      const values = members.map((r) => r.radiance[c]);
      return Math.max(...values) / (values.reduce((s, v) => s + v, 0) / values.length);
    });
    out.set(image, { largestDeltaE, relativeRms, maxOverMean });
  }
  return out;
}

/** The client's radiance of a reading's geometry, by its name, or a thrown error naming it. */
function clientRadiance(client: ReadonlyMap<string, Rgb>, reading: ReferenceReading): Rgb {
  const mine = client.get(reading.name);
  if (mine === undefined) {
    throw new Error(`the client gives no radiance of ${reading.name}`);
  }
  return mine;
}

/** A triple of `f` at each channel. */
function perChannel(f: (c: Channel) => number): Rgb {
  return [f(0), f(1), f(2)];
}
