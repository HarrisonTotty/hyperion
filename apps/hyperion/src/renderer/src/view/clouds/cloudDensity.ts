/**
 * The clouds' one optical-depth field (plan R11, R11.T7.b; Design notes 8 and 10): at a place on
 * the layer and a footprint, the share of it the clouds cover and their in-cloud τ, which the
 * two-dimensional layer shades, the terrain's cloud shadow reads and the volume (T7.c) spreads
 * through the layer's depth; the CPU reference of `shaders/cloudField.wgsl`.
 *
 * @remarks
 * - **The structure.** A standard normal field z, the sum of up to {@link CLOUD_OCTAVES_MAX}
 *   octaves of the noise tile (`cloudNoise.ts`), octave k of lattice cell ℓ₀ ÷ 2ᵏ and amplitude ρᵏ
 *   ({@link CLOUD_OCTAVE_RATIO}), each octave's coordinates turned by a fixed rotation from the
 *   last's ({@link OCTAVE_ROTATION}) and the body's own rotation and offsets keyed by its seed, so
 *   that no two worlds share a pattern; normalised by √(Σρ²ᵏ), so that z has unit variance.
 * - **The coverage** C at a place (`coverage.ts`) is met by a threshold: the clouds stand where
 *   z > z_c = Φ⁻¹(1 − C), which covers C of the area exactly for a standard normal z.
 * - **The in-cloud τ** grows with the excess z − z_c, linearly, so that a cloud thins to nothing at
 *   its edge and thickens inwards, scaled so that its mean over the cloudy area is the condensate's
 *   τ̄: τ = τ̄ N (z − z_c), N = C ÷ (φ(z_c) − z_c C), φ the normal density (a labelled
 *   heuristic, lean 2026-11-02, in place of Design note 10's log-normal: its spread within a cloud
 *   is a truncated normal's, whose inhomogeneity χ = exp⟨ln τ⟩ ÷ ⟨τ⟩ is 0.66 at C = 0.5 and 0.70 at
 *   0.67, from 0.60 to 0.82 over C 0.05–0.95, against Oreopoulos and Cahalan 2005's 0.75, which it
 *   meets near C = 0.82).
 * - **The footprint** decides what is resolved: octave k is drawn with weight
 *   w_k = clamp(log₂(ℓ_k ÷ 2F), 0, 1), fading out across one octave of footprint F (Design note 3's
 *   fade), times the time-rate fade (`coverage.ts`' `timeRateFade`); its variance, unresolved,
 *   joins s², the variance left unresolved. With the resolved z_r and s the footprint's share and
 *   mean τ are the expectations over the unresolved part, z ~ N(z_r, s²): the share
 *   f = Q(d) and the mean τ τ̄ N s (φ(d) − d Q(d)), d = (z_c − z_r) ÷ s, Q the normal's upper tail.
 *   So the field is its own filter: a footprint that resolves nothing has f = C and in-cloud τ τ̄,
 *   the calibrated mean, and the time-rate fade reaches it the same way.
 * - **Motion** is the flow map's (`coverage.ts`): each phase samples the structure upwind by its
 *   advection time, under the wind the coverage texture holds, the two phases' samples blended as
 *   (w₀ n₀ + w₁ n₁) ÷ √(w₀² + w₁²), which keeps unit variance (Neyret 2003's contrast restoration).
 * - **Two clients** compute the same field bit for bit here, a function of the seed, the coverage,
 *   the scene's time and the footprint; the GPU's `f32` draws it to the eye.
 */
import type { UniverseTime } from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import { asin, atan2, cos, exp, log2, sin } from "../../lib/pinnedMath";
import { CLOUD_NOISE_STD, inverseNormal, latticeHash, sampleCloudNoise } from "./cloudNoise";
import type { CloudCondensate } from "./condensates";
import {
  cellLatitudeRad,
  type CoverageGrid,
  FLOW_MAP_PERIOD_S,
  flowMapPhases,
  timeRateFade,
} from "./coverage";

/** The most octaves any form of the clouds draws. */
export const CLOUD_OCTAVES_MAX = 12;

/**
 * ρ, each octave's amplitude over the one before: 0.7, between fbm's customary ½ and the 2^(−1/3)
 * of a −5/3 spectrum, which cloud fields' optical depth roughly follows (lean, 2026-11-02).
 */
export const CLOUD_OCTAVE_RATIO = 0.7;

/**
 * ℓ₀'s largest, m: 1,000 km, the synoptic scale of a frontal system, the coarsest structure the
 * octaves add to the coverage's (lean, 2026-11-02).
 */
export const CLOUD_BASE_CELL_MAX_M = 1_000_000;

/** The octave lattice's cell at octave 0 on a body of radius `radiusM`: at most π R ÷ 8. */
export function cloudBaseCellM(radiusM: number): number {
  return Math.min(CLOUD_BASE_CELL_MAX_M, (Math.PI * radiusM) / 8);
}

/** A 3 × 3 matrix, row-major. */
export type Matrix3 = readonly [
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
];

/**
 * The rotation each octave's coordinates take from the last's, row-major (Quilez's fbm matrix,
 * orthonormal), so that the octaves' lattices never align.
 */
export const OCTAVE_ROTATION: Matrix3 = [0, 0.8, 0.6, -0.8, 0.36, -0.48, -0.6, -0.48, 0.64];

/**
 * The least unresolved standard deviation the field takes, so that a fully resolved footprint's
 * threshold is a steep ramp rather than a step: 10⁻³ of z's.
 */
export const LEAST_UNRESOLVED_SD = 1e-3;

/** The least share a footprint's in-cloud τ is divided by. */
const LEAST_SHARE = 1e-6;

/**
 * The upper tail of the standard normal, Q(x) = erfc(x ÷ √2) ÷ 2, by Numerical Recipes' erfc
 * (Press et al., 2nd ed., §6.2's `erfcc`, fractional error under 1.2 × 10⁻⁷ everywhere), the form
 * the shaders take.
 */
export function normalTail(x: number): number {
  const z = Math.abs(x) / Math.SQRT2;
  const t = 1 / (1 + 0.5 * z);
  const poly =
    -z * z -
    1.265_512_23 +
    t *
      (1.000_023_68 +
        t *
          (0.374_091_96 +
            t *
              (0.096_784_18 +
                t *
                  (-0.186_288_06 +
                    t *
                      (0.278_868_07 +
                        t *
                          (-1.135_203_98 +
                            t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77))))))));
  const erfc = t * exp(poly);
  return x >= 0 ? 0.5 * erfc : 1 - 0.5 * erfc;
}

/** The standard normal density, φ(x). */
export function normalDensity(x: number): number {
  return exp(-0.5 * x * x) / Math.sqrt(2 * Math.PI);
}

/** The threshold z_c's magnitude at which C of 0 or 1 is held: Q(6) is 10⁻⁹. */
export const THRESHOLD_LIMIT = 6;

/**
 * The threshold and the τ scale of a coverage C: z_c = Φ⁻¹(1 − C), held to ±{@link THRESHOLD_LIMIT},
 * and N = C ÷ (φ(z_c) − z_c C), with C taken as Q(z_c) so that the two agree at the limits.
 */
export function thresholdOf(coverage: number): {
  readonly threshold: number;
  readonly scale: number;
} {
  const c = Math.min(Math.max(coverage, 0), 1);
  const raw = c <= 0 ? THRESHOLD_LIMIT : c >= 1 ? -THRESHOLD_LIMIT : inverseNormal(1 - c);
  const threshold = Math.min(Math.max(raw, -THRESHOLD_LIMIT), THRESHOLD_LIMIT);
  const share = normalTail(threshold);
  return { threshold, scale: share / (normalDensity(threshold) - threshold * share) };
}

/**
 * The coverage texture's texels: each cell's z_c, N, and the wind there, eastward and northward,
 * m s⁻¹, four `f32` a cell in the coverage grid's order (`coverage.ts`' `CoverageGrid`), the
 * `rgba16float` texture's values before narrowing.
 */
export interface CoverageTexels {
  readonly widthCells: number;
  readonly heightCells: number;
  readonly texels: Float32Array;
}

/**
 * A coverage grid as its texture's texels, with the wind `windAt` gives at each cell's centre.
 *
 * @param narrow - The narrowing the texture applies (`toHalf` and back for `rgba16float`), so that
 *   the CPU reference reads the texture's own values; the identity by default.
 */
export function coverageTexels(
  grid: CoverageGrid,
  windAt: (latitudeRad: number) => { readonly eastMps: number; readonly northMps: number },
  narrow: (value: number) => number = (value) => value,
): CoverageTexels {
  const { widthCells: w, heightCells: h, fractions } = grid;
  const texels = new Float32Array(4 * w * h);
  for (let j = 0; j < h; j += 1) {
    const wind = windAt(cellLatitudeRad(j, h));
    for (let i = 0; i < w; i += 1) {
      const { threshold, scale } = thresholdOf(fractions[j * w + i] ?? 0);
      texels.set(
        [narrow(threshold), narrow(scale), narrow(wind.eastMps), narrow(wind.northMps)],
        4 * (j * w + i),
      );
    }
  }
  return { widthCells: w, heightCells: h, texels };
}

/** A cell of the coverage texture at a place, filtered bilinearly as the GPU filters it. */
export interface CoverageSample {
  readonly threshold: number;
  readonly scale: number;
  readonly eastMps: number;
  readonly northMps: number;
}

/**
 * The coverage texture at a place, bilinear between cell centres, `repeat` around a parallel and
 * held at the first and last rows' centres towards the poles, as `cloudField.wgsl` reads it.
 *
 * @param point - Body-fixed; only its direction is read.
 */
export function sampleCoverage(coverage: CoverageTexels, point: Vec3): CoverageSample {
  const { widthCells: w, heightCells: h, texels } = coverage;
  const r = Math.hypot(point.x, point.y, point.z);
  const latitude = asin(Math.min(Math.max(point.z / r, -1), 1));
  const longitude = atan2(point.y, point.x);
  const u = (longitude + Math.PI) / (2 * Math.PI);
  const v = Math.min(Math.max((Math.PI / 2 - latitude) / Math.PI, 0.5 / h), 1 - 0.5 / h);
  const tx = u * w - 0.5;
  const ty = v * h - 0.5;
  const x0 = Math.floor(tx);
  const y0 = Math.floor(ty);
  const fx = tx - x0;
  const fy = ty - y0;
  const at = (dx: number, dy: number, c: number): number => {
    const i = (((x0 + dx) % w) + w) % w;
    const j = Math.min(Math.max(y0 + dy, 0), h - 1);
    return texels[4 * (j * w + i) + c] ?? 0;
  };
  const channel = (c: number): number => {
    const top = at(0, 0, c) + fx * (at(1, 0, c) - at(0, 0, c));
    const bottom = at(0, 1, c) + fx * (at(1, 1, c) - at(0, 1, c));
    return top + fy * (bottom - top);
  };
  return { threshold: channel(0), scale: channel(1), eastMps: channel(2), northMps: channel(3) };
}

/** What the field of one body's clouds takes this frame, besides its coverage and the noise. */
export interface CloudFieldParams {
  /** τ̄, the condensate's mean in-cloud τ(550). */
  readonly meanTau: number;
  /** The octaves drawn at most, 1 to {@link CLOUD_OCTAVES_MAX}. */
  readonly octaves: number;
  /** ℓ₀, octave 0's lattice cell, m ({@link cloudBaseCellM}). */
  readonly baseCellM: number;
  /** The body's rotation of the octaves' coordinates, row-major. */
  readonly rotation: Matrix3;
  /** Each phase's advection time, s (`coverage.ts`' `FlowPhase.advectionS`). */
  readonly advectionS: readonly [number, number];
  /** Each phase's weight. */
  readonly phaseWeights: readonly [number, number];
  /**
   * Each octave's and phase's offset of its coordinates, in tiles: three numbers for octave k and
   * phase i at 3(2k + i), from the body's seed and the phase's cycle.
   */
  readonly offsets: Float64Array;
  /** Each octave's share left by the time-rate fade, in [0, 1]. */
  readonly octaveFades: Float64Array;
}

/** The field's answer at a footprint. */
export interface CloudSample {
  /** f, the share of the footprint the clouds cover, in [0, 1]. */
  readonly fraction: number;
  /** The footprint's mean τ(550), over its whole area. */
  readonly meanTau: number;
  /** The in-cloud τ(550), its mean over the covered share: the mean ÷ f. */
  readonly inCloudTau: number;
  /** z_r, the resolved structure, and s, the unresolved standard deviation. */
  readonly resolved: number;
  readonly unresolvedSd: number;
}

/** A point turned by a matrix. */
function turned(m: Matrix3, p: Vec3): Vec3 {
  const [m0, m1, m2, m3, m4, m5, m6, m7, m8] = m;
  return {
    x: m0 * p.x + m1 * p.y + m2 * p.z,
    y: m3 * p.x + m4 * p.y + m5 * p.z,
    z: m6 * p.x + m7 * p.y + m8 * p.z,
  };
}

/**
 * Where the structure at `point` came from `advectionS` earlier under the wind there: turned about
 * the pole by −u t ÷ ρ (`coverage.ts`' `zonalAdvection`), then about the local east by v t ÷ r, so
 * that the structure moves north at v; a point on the axis takes no zonal turn.
 *
 * @param point - Body-fixed, m.
 * @param eastMps - The wind's eastward part, m s⁻¹.
 * @param northMps - Its northward part, m s⁻¹.
 * @param advectionS - How long ago, s.
 */
export function advected(point: Vec3, eastMps: number, northMps: number, advectionS: number): Vec3 {
  const rho = Math.hypot(point.x, point.y);
  let p = point;
  if (rho > 0) {
    const a = (-eastMps * advectionS) / rho;
    const c = cos(a);
    const s = sin(a);
    p = { x: c * p.x - s * p.y, y: s * p.x + c * p.y, z: p.z };
  }
  const r = Math.hypot(p.x, p.y, p.z);
  if (northMps === 0 || !(rho > 0) || !(r > 0)) {
    return p;
  }
  // Upwind of a northward wind is south: the colatitude grows by v t ÷ r, so ρ′ = ρ cos β + z sin β
  // and z′ = z cos β − ρ sin β.
  const b = (northMps * advectionS) / r;
  const c = cos(b);
  const s = sin(b);
  const scale = (c * rho + s * p.z) / rho;
  return { x: p.x * scale, y: p.y * scale, z: c * p.z - s * rho };
}

/**
 * The field at `point` over a footprint of `footprintM` (the module's remarks).
 *
 * @param point - On the layer, body-fixed, m.
 * @param footprintM - The footprint's size, m, above 0.
 * @param octaveCap - The octaves drawn at most, the rest left unresolved: a cheaper reader's choice
 *   that keeps the coverage (`cloud_field_at`'s `octave_cap`).
 */
export function cloudFieldAt(
  coverage: CoverageTexels,
  noise: Uint8Array,
  params: CloudFieldParams,
  point: Vec3,
  footprintM: number,
  octaveCap: number = CLOUD_OCTAVES_MAX,
): CloudSample {
  const cell = sampleCoverage(coverage, point);
  const [w0, w1] = params.phaseWeights;
  const norm = 1 / Math.hypot(w0, w1);
  const starts = params.advectionS.map((s) =>
    turned(params.rotation, advected(point, cell.eastMps, cell.northMps, s)),
  );
  const tile = 1 / (8 * params.baseCellM);
  let c0 = scaled(starts[0] ?? point, tile);
  let c1 = scaled(starts[1] ?? point, tile);
  let total = 0;
  let power = 1;
  for (let k = 0; k < params.octaves; k += 1) {
    total += power;
    power *= CLOUD_OCTAVE_RATIO * CLOUD_OCTAVE_RATIO;
  }
  let resolved = 0;
  let resolvedVariance = 0;
  let amplitude = 1;
  let cellM = params.baseCellM;
  const drawn = Math.min(params.octaves, octaveCap, CLOUD_OCTAVES_MAX);
  for (let k = 0; k < drawn; k += 1) {
    const reach = Math.min(Math.max(log2(cellM / (2 * footprintM)), 0), 1);
    const weight = reach * (params.octaveFades[k] ?? 0);
    if (!(weight > 0)) {
      break;
    }
    const n0 = sampleCloudNoise(noise, offsetBy(c0, params.offsets, 6 * k)) / CLOUD_NOISE_STD;
    const n1 = sampleCloudNoise(noise, offsetBy(c1, params.offsets, 6 * k + 3)) / CLOUD_NOISE_STD;
    const n = (w0 * n0 + w1 * n1) * norm;
    resolved += weight * amplitude * n;
    resolvedVariance += (weight * amplitude) ** 2;
    c0 = scaled(turned(OCTAVE_ROTATION, c0), 2);
    c1 = scaled(turned(OCTAVE_ROTATION, c1), 2);
    amplitude *= CLOUD_OCTAVE_RATIO;
    cellM *= 0.5;
  }
  const zr = resolved / Math.sqrt(total);
  const s = Math.max(Math.sqrt(Math.max(1 - resolvedVariance / total, 0)), LEAST_UNRESOLVED_SD);
  const d = (cell.threshold - zr) / s;
  const q = normalTail(d);
  const meanTau = Math.max(params.meanTau * cell.scale * s * (normalDensity(d) - d * q), 0);
  return {
    fraction: q,
    meanTau,
    inCloudTau: meanTau / Math.max(q, LEAST_SHARE),
    resolved: zr,
    unresolvedSd: s,
  };
}

function scaled(p: Vec3, by: number): Vec3 {
  return { x: p.x * by, y: p.y * by, z: p.z * by };
}

function offsetBy(p: Vec3, offsets: Float64Array, at: number): Vec3 {
  return {
    x: p.x + (offsets[at] ?? 0),
    y: p.y + (offsets[at + 1] ?? 0),
    z: p.z + (offsets[at + 2] ?? 0),
  };
}

/** A body's clouds' seed from its id: FNV-1a over its characters, a `u32`. */
export function cloudSeedOf(body: string): number {
  let h = 0x811c_9dc5;
  for (let i = 0; i < body.length; i += 1) {
    h = Math.imul(h ^ body.charCodeAt(i), 0x0100_0193) >>> 0;
  }
  return h;
}

/** A hash's value as a fraction in [0, 1). */
function unit(hash: number): number {
  return hash / 4_294_967_296;
}

/**
 * A body's rotation of its octaves' coordinates from its seed: the unit quaternion of three uniform
 * hashes (Shoemake 1992's uniform rotation), row-major.
 */
export function cloudRotationOf(seed: number): Matrix3 {
  const u1 = unit(latticeHash(1, 0, 0, seed));
  const u2 = unit(latticeHash(2, 0, 0, seed));
  const u3 = unit(latticeHash(3, 0, 0, seed));
  const a = Math.sqrt(1 - u1);
  const b = Math.sqrt(u1);
  const qx = a * sin(2 * Math.PI * u2);
  const qy = a * cos(2 * Math.PI * u2);
  const qz = b * sin(2 * Math.PI * u3);
  const qw = b * cos(2 * Math.PI * u3);
  return [
    1 - 2 * (qy * qy + qz * qz),
    2 * (qx * qy - qz * qw),
    2 * (qx * qz + qy * qw),
    2 * (qx * qy + qz * qw),
    1 - 2 * (qx * qx + qz * qz),
    2 * (qy * qz - qx * qw),
    2 * (qx * qz - qy * qw),
    2 * (qy * qz + qx * qw),
    1 - 2 * (qx * qx + qy * qy),
  ];
}

/** What makes a body's {@link CloudFieldParams} at a time. */
export interface CloudFieldInput {
  readonly seed: number;
  readonly condensate: Pick<CloudCondensate, "inCloudOpticalDepth550">;
  /** The body's equatorial radius, m. */
  readonly radiusM: number;
  readonly time: Pick<UniverseTime, "seconds" | "nanos">;
  /** Simulation seconds per real second. */
  readonly rate: number;
  /** The wind that sets the time-rate fade, m s⁻¹: the largest the coverage texture holds. */
  readonly windMps: number;
  readonly octaves: number;
}

/**
 * A body's field parameters at a time: its seed's rotation, each octave's and phase's offsets from
 * its seed and the phase's cycle, the flow map's phases (`coverage.ts`' `flowMapPhases` over
 * {@link FLOW_MAP_PERIOD_S}) and each octave's time-rate fade at its own cell.
 *
 * @throws RangeError for an octave count outside 1 to {@link CLOUD_OCTAVES_MAX}.
 */
export function cloudFieldParams(input: CloudFieldInput): CloudFieldParams {
  const { seed, octaves } = input;
  if (!(Number.isInteger(octaves) && octaves >= 1 && octaves <= CLOUD_OCTAVES_MAX)) {
    throw new RangeError(`the clouds' octaves ${octaves} must be a whole number from 1 to 12`);
  }
  const phases = flowMapPhases(input.time, FLOW_MAP_PERIOD_S);
  const baseCellM = cloudBaseCellM(input.radiusM);
  const offsets = new Float64Array(6 * octaves);
  const octaveFades = new Float64Array(octaves);
  let cellM = baseCellM;
  for (let k = 0; k < octaves; k += 1) {
    for (const [i, phase] of phases.entries()) {
      // The cycle keys each restart's structure afresh; the octave and phase keep them apart.
      const cycle = phase.cycle | 0;
      for (let axis = 0; axis < 3; axis += 1) {
        offsets[3 * (2 * k + i) + axis] = unit(latticeHash(cycle, 2 * k + i, axis, seed));
      }
    }
    octaveFades[k] = timeRateFade(input.rate, cellM, input.windMps);
    cellM *= 0.5;
  }
  return {
    meanTau: input.condensate.inCloudOpticalDepth550,
    octaves,
    baseCellM,
    rotation: cloudRotationOf(seed),
    advectionS: [phases[0].advectionS, phases[1].advectionS],
    phaseWeights: [phases[0].weight, phases[1].weight],
    offsets,
    octaveFades,
  };
}
