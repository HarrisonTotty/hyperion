/**
 * The cloud volume's three-dimensional density (plan R11, R11.T7.c; Design note 10): the layer's
 * field (`cloudDensity.ts`) as its envelope, laid through the deck by a height profile of its cloud
 * kind and eroded by the shape noises (`cloudShapeNoise.ts`), so that the layer is its column
 * integral, the independent-pixel approximation of it; the CPU reference of `cloudShape.wgsl`.
 *
 * @remarks
 * - **A column.** At a place and footprint the field gives the covered share f, the mean τ over the
 *   footprint and the unresolved standard deviation s; with the coverage's threshold z_c and scale N
 *   it gives the covered part's mean excess ē = τ_in ÷ (τ̄ N), in standard deviations of z, the
 *   expectation of z − z_c over the covered part (`cloudDensity.ts`' remarks).
 * - **Its top.** The cloud stands from the deck's base to η_top of the deck's depth D (a share,
 *   {@link deckTopShare}). A cumuliform column's top rises with its τ as an adiabatic cloud's does:
 *   liquid water grows linearly above the base, so that extinction grows as height^(2/3) and τ as
 *   H^(5/3) (Brenguier et al. 2000, J. Atmos. Sci. 57, 803), H = D (ē ÷ ē_full)^(3/5); so a cumulus
 *   is a dome, flat at its base and highest where its τ is. A stratiform column fills most of the
 *   deck, its top a little lower where it is thin. Which kind: a liquid deck is cumuliform where
 *   its coverage is broken and stratiform where it closes ({@link CUMULUS_COVER} to
 *   {@link STRATUS_COVER}: a cumulus field below, a stratocumulus sheet above, blended between); an
 *   ice deck is a sheet (labelled heuristics, lean 2026-11-19).
 * - **Its profile.** Within the cloud, at u = η ÷ η_top of its height, the extinction goes as
 *   (1 + κ) u^κ, κ = 2/3 for a liquid deck (the adiabatic profile above) and 0 for an ice deck,
 *   which integrates to 1 over the cloud, so that the column's τ is the field's.
 * - **Its erosion** by the noises, after Nubis's (Schneider and Vos 2015): the envelope 1 − u, 1 at
 *   the base and 0 at the top, remapped by the base noise n_b, so that where n_b is low the cloud's
 *   upper part is eaten into billows, then by the detail noise n_d, which frays the edges left (its
 *   sense turned near the base, wisps below and billows above, as Nubis turns it), then sharpened:
 *   g = min(3 S, 1) ({@link erosion}). Each noise's share fades with the footprint, as the field's
 *   octaves fade (`cloudDensity.ts`), and with the time rate (`coverage.ts`' `timeRateFade`).
 * - **The column integral kept.** The erosion's mean over the profile and both noises, uniform
 *   once equalised and independent of the field, G(w_b, w_d) = ∫₀¹ (1 + κ) u^κ E[g] du
 *   ({@link cloudShapeNormalisation}), divides the eroded density, so that each column's expected τ
 *   is the field's exactly whatever the noises' shares; a column's own τ strays from it by the
 *   noises' vertical mean (`cloudShape.test.ts` states both).
 * - **Flattened for the hand-over.** A flattening φ in [0, 1] takes the density to the layer's
 *   uniform slab, η_top, the profile and the erosion each mixed to 1 by φ, which keeps the column
 *   integral; at φ = 1 the volume marches what the layer draws (`cloudVolumePass.ts`).
 * - So the density is σ = τ_col (1 + κ) u^κ g_φ ÷ (D η_top G_φ) per metre, τ_col the column's τ.
 */
import type { Vec3 } from "../../geometry/vec3";
import { log2, pow } from "../../lib/pinnedMath";
import {
  type CloudFieldParams,
  cloudFieldAt,
  CLOUD_OCTAVE_RATIO,
  OCTAVE_ROTATION,
  type CoverageTexels,
  fieldFromParts,
  normalTail,
  octaveNoisesAt,
  octavePowerOf,
  octaveWeightAt,
  sampleCoverage,
  scaleOfThreshold,
} from "./cloudDensity";
import type { CloudMapRead, CloudMapTexel } from "./cloudMap";
import { BASE_NOISE_SIZE, CURL_NOISE_SIZE, DETAIL_NOISE_SIZE } from "./cloudShapeNoise";
import type { CloudCondensate } from "./condensates";
import { timeRateFade } from "./coverage";

/** ē_full, the mean excess at which a cumuliform column reaches the deck's top (lean, 2026-11-19). */
export const EXCESS_FULL = 2;

/** The exponent of a cumuliform top's rise with its τ: 3/5, an adiabatic cloud's H ∝ τ^(3/5). */
export const TOP_EXPONENT = 0.6;

/** The share of the deck a stratiform column's top takes at least (lean, 2026-11-19). */
export const STRATIFORM_LEAST_TOP = 0.7;

/** The coverage at and below which a liquid deck is all cumuliform. */
export const CUMULUS_COVER = 0.5;

/** The coverage at and above which a liquid deck is all stratiform. */
export const STRATUS_COVER = 0.85;

/** The least top share a column takes, so that its density stays finite. */
export const LEAST_TOP_SHARE = 0.02;

/** A liquid deck's profile exponent κ: 2/3, adiabatic extinction's growth above the base. */
export const LIQUID_PROFILE_EXPONENT = 2 / 3;

/** The base noise's erosion strength: where n_b is 0, the upper 0.6 of the envelope is eaten. */
export const BASE_EROSION = 0.6;

/** The detail noise's erosion strength. */
export const DETAIL_EROSION = 0.4;

/** The eroded envelope's sharpening: full density from a third of the way in. */
export const EDGE_SHARPNESS = 3;

/** The share of a column's height above its base over which the detail turns from wisps to billows. */
export const WISP_SHARE = 0.2;

/** The steps of the erosion's weights the normalisation is tabled at: 0, 1/8, …, 1 on each. */
export const NORMALISATION_STEPS = 9;

/** A deck's cloud kind as its density reads it. */
export interface CloudShapeKind {
  /** κ of the profile (1 + κ) u^κ: {@link LIQUID_PROFILE_EXPONENT} for liquid, 0 for ice. */
  readonly profileExponent: number;
  /** Whether its columns may be cumuliform: a liquid deck's. */
  readonly cumuliform: boolean;
}

/** A condensate's cloud kind (the module's remarks): liquid decks cumuliform, ice decks sheets. */
export function cloudShapeKindOf(condensate: Pick<CloudCondensate, "phase">): CloudShapeKind {
  return condensate.phase === "liquid"
    ? { profileExponent: LIQUID_PROFILE_EXPONENT, cumuliform: true }
    : { profileExponent: 0, cumuliform: false };
}

function clamp01(x: number): number {
  return Math.min(Math.max(x, 0), 1);
}

/**
 * η_top, the share of the deck's depth a column's cloud reaches above the base (the module's
 * remarks), from its covered part's mean excess and the coverage there.
 */
export function deckTopShare(excess: number, coverage: number, cumuliform: boolean): number {
  const thickness = pow(clamp01(excess / EXCESS_FULL), TOP_EXPONENT);
  const sheet = STRATIFORM_LEAST_TOP + (1 - STRATIFORM_LEAST_TOP) * thickness;
  const cumulus = cumuliform
    ? clamp01((STRATUS_COVER - coverage) / (STRATUS_COVER - CUMULUS_COVER))
    : 0;
  return Math.max(sheet + cumulus * (thickness - sheet), LEAST_TOP_SHARE);
}

/** A cumuliform column's top share at an excess: an adiabatic dome's, (ē ÷ ē_full)^(3/5). */
export function domeTopShare(excess: number): number {
  return Math.max(pow(clamp01(excess / EXCESS_FULL), TOP_EXPONENT), LEAST_TOP_SHARE);
}

/** A stratiform column's top share at an excess: most of the deck, a little lower where thin. */
export function sheetTopShare(excess: number): number {
  return (
    STRATIFORM_LEAST_TOP +
    (1 - STRATIFORM_LEAST_TOP) * pow(clamp01(excess / EXCESS_FULL), TOP_EXPONENT)
  );
}

/**
 * w_cu, the cumuliform share of a column at its coverage C: a liquid deck's, 1 at and below
 * {@link CUMULUS_COVER}, 0 at and above {@link STRATUS_COVER}, linear between; an ice deck's 0.
 */
export function cumulusShare(cover: number, kind: CloudShapeKind): number {
  return kind.cumuliform ? clamp01((STRATUS_COVER - cover) / (STRATUS_COVER - CUMULUS_COVER)) : 0;
}

/** A cumulus cell's feature, in the base noise's tiles: its coarsest octave's cell, a quarter. */
export const CELL_FEATURE_TILES = 1 / 4;

/**
 * The footprint the cells' regional share and τ are read at, m (lean, 2026-11-19): 40 km, where the
 * field's share is fractional (its variance lies in its 1,000 km to 100 km octaves, so that at a
 * cell's footprint it is a solid mass or clear sky).
 */
export const CELL_REGION_M = 40_000;

/**
 * The share of the field's own structure at a cell's footprint the cells' cover follows over the
 * region's (lean, 2026-11-19): a half, the mesoscale's organisation of a cumulus field into its
 * masses and clearings, never solid where the region is broken.
 */
export const CELL_MESOSCALE = 0.5;

/**
 * The least in-cloud τ of a column's cells, over the condensate's τ̄ (lean, 2026-11-19): 0.6, about 5
 * for liquid water, a cumulus humilis's. The field ties its in-cloud τ to a mass's depth past its
 * threshold, so that where it is broken at a cell's region, about a mass's edge, its in-cloud τ is
 * 1–3, see-through and dark (the eye check of 2026-11-19); a cumulus's τ is its own, so the cells
 * keep at least this and cover less, their mean over the region still the field's.
 */
export const CELL_LEAST_TAU_SHARE = 0.6;

/** The second sampling's scale of the cells' noise over the first's: √3, incommensurate with 1. */
export const CELL_SECOND_SCALE = 1.7320508075688772;

/** The second sampling's offset, in its tiles. */
export const CELL_SECOND_OFFSET: Vec3 = { x: 0.371, y: 0.613, z: 0.129 };

/** The least share a column's cells are formed at, and the most. */
const CELL_LEAST_SHARE = 1e-6;

/**
 * The cells' uniform noise at a place on the layer's shell, m: the base noise read twice, at its
 * tile and turned by `OCTAVE_ROTATION` at √3 of it, so that its tile's repeat, 8 km on an Earth, is
 * not seen, their sum taken back to a uniform value by the triangular distribution's CDF, so that
 * its marginal stays uniform on [0, 1]. Kept uniform, where a Gaussianised noise's Φ⁻¹ would draw
 * its broad maxima into narrow spikes, and the cells standing on them into pillars (the eye check
 * of 2026-11-19).
 */
export function cellNoiseAt(texels: Uint8Array, frame: CloudShapeFrame, placeM: Vec3): number {
  const k = 1 / frame.baseTileM;
  const first = sampleTile(texels, BASE_NOISE_SIZE, {
    x: placeM.x * k + frame.baseDrift.x,
    y: placeM.y * k + frame.baseDrift.y,
    z: placeM.z * k + frame.baseDrift.z,
  });
  const [m0, m1, m2, m3, m4, m5, m6, m7, m8] = OCTAVE_ROTATION;
  const k2 = k / CELL_SECOND_SCALE;
  const second = sampleTile(texels, BASE_NOISE_SIZE, {
    x: (m0 * placeM.x + m1 * placeM.y + m2 * placeM.z) * k2 + CELL_SECOND_OFFSET.x,
    y: (m3 * placeM.x + m4 * placeM.y + m5 * placeM.z) * k2 + CELL_SECOND_OFFSET.y,
    z: (m6 * placeM.x + m7 * placeM.y + m8 * placeM.z) * k2 + CELL_SECOND_OFFSET.z,
  });
  return triangularCdf(first + second);
}

/** The CDF of the sum of two independent uniforms on [0, 1], at s in [0, 2]. */
export function triangularCdf(s: number): number {
  const t = Math.min(Math.max(s, 0), 2);
  return t <= 1 ? (t * t) / 2 : 1 - ((2 - t) * (2 - t)) / 2;
}

/** The excess of the region's tall cell over the cells' mean (lean, 2026-11-19). */
export const CELL_TALL_SHARE = 1.5;

/**
 * The cells' noise's excess past its threshold, Δu = u − (1 − f), at which a cell's rise saturates
 * (lean, 2026-11-19): 0.25. A cell's excess is at most its cover f, so that sparse cells are small,
 * low domes and a dense field's cells larger and tall, flat-topped ones, near a cumulus's aspect of
 * 1.
 */
export const CELL_SATURATION = 0.25;

/**
 * A cumulus cell's top share at its excess Δu (the module's remarks): the region's tall cell's, a
 * dome of the cells' mean excess times {@link CELL_TALL_SHARE}, times its rise ({@link cellRise}),
 * steep from the cell's edge to a rounded top, where an adiabatic dome of the cell's own τ would
 * follow the noise's peaks into cones.
 */
export function cellTopShare(meanExcess: number, du: number): number {
  return Math.max(domeTopShare(meanExcess * CELL_TALL_SHARE) * cellRise(du), LEAST_TOP_SHARE);
}

/** A cell's rise from its edge at its excess Δu, 1 − (1 − min(Δu ÷ saturation, 1))²: its top's, its τ's. */
export function cellRise(du: number): number {
  return 1 - (1 - clamp01(du / CELL_SATURATION)) ** 2;
}

/**
 * E[rise | u > 1 − f], the mean of {@link cellRise} over the cells at a share f, their excess Δu
 * uniform on [0, f]: f ÷ s − f² ÷ 3s² up to the saturation s, 1 − s ÷ 3f past it.
 */
export function cellRiseMean(f: number): number {
  const share = Math.min(Math.max(f, CELL_LEAST_SHARE), 1);
  const s = CELL_SATURATION;
  return share <= s ? share / s - (share * share) / (3 * s * s) : 1 - s / (3 * share);
}

/**
 * A column's cumulus cell at a place (the module's remarks): the cells cover f_c = f_R + m (f − f_R)
 * of the place's surroundings, f_R the region's share ({@link CELL_REGION_M}), f the field's at the
 * cell's footprint and m {@link CELL_MESOSCALE}, where the cells' uniform noise u exceeds 1 − f_c.
 * The cells' mean in-cloud τ is the region's, τ_R ÷ f_R, but at least
 * {@link CELL_LEAST_TAU_SHARE} of τ̄, their cover shrunk by as much, so that their mean τ over the
 * region is the field's. A cell's excess Δu = u − (1 − f_c) sets its top ({@link cellTopShare}) and
 * its in-cloud τ, the cells' mean times its rise ({@link cellRise}) over the rise's mean over the
 * cells ({@link cellRiseMean}), nearly uniform within it and rising steeply at its edge, as a
 * cumulus's; its excess is the cells' mean times the same share.
 *
 * @param u - The cells' noise at the place, uniform on [0, 1] ({@link cellNoiseAt}).
 * @param meanTauBar - τ̄, the condensate's mean in-cloud τ(550).
 */
export function cellOf(
  region: Pick<CloudColumn, "fraction" | "meanTau" | "excess">,
  local: Pick<CloudColumn, "fraction">,
  u: number,
  meanTauBar: number,
): { readonly inCloudTau: number; readonly excess: number; readonly topShare: number } {
  if (!(region.meanTau > 0)) {
    return { inCloudTau: 0, excess: 0, topShare: LEAST_TOP_SHARE };
  }
  const fR = Math.min(Math.max(region.fraction, CELL_LEAST_SHARE), 1);
  const fC = fR + CELL_MESOSCALE * (local.fraction - fR);
  const inCloud = region.meanTau / fR;
  const cellMean = Math.max(inCloud, CELL_LEAST_TAU_SHARE * meanTauBar);
  const f = Math.min(Math.max((fC * inCloud) / cellMean, CELL_LEAST_SHARE), 1);
  const du = Math.max(u - (1 - f), 0);
  const share = du > 0 ? cellRise(du) / cellRiseMean(f) : 0;
  const excess = (region.excess * cellMean) / inCloud;
  return {
    inCloudTau: cellMean * share,
    excess: excess * share,
    topShare: cellTopShare(excess, du),
  };
}

/**
 * The eroded envelope g in [0, 1] at a sample (the module's remarks).
 *
 * @param envelope - 1 − u: 1 at the column's base, 0 at its top.
 * @param base - n_b, the base noise there, in [0, 1].
 * @param detail - n_d, the detail noise there, in [0, 1], its sense already turned for the height.
 * @param baseShare - w_b, the base noise's share, in [0, 1].
 * @param detailShare - w_d, the detail noise's share, in [0, 1].
 */
export function erosion(
  envelope: number,
  base: number,
  detail: number,
  baseShare: number,
  detailShare: number,
): number {
  const lo = baseShare * BASE_EROSION * (1 - base);
  const shaped = clamp01((envelope - lo) / (1 - lo));
  const loDetail = detailShare * DETAIL_EROSION * detail;
  const frayed = clamp01((shaped - loDetail) / (1 - loDetail));
  return Math.min(frayed * EDGE_SHARPNESS, 1);
}

/** The detail noise's value as the erosion takes it at u: as stored near the base, turned above. */
export function turnedDetail(detail: number, u: number): number {
  const turn = clamp01(u / WISP_SHARE);
  return detail + turn * (1 - 2 * detail);
}

/** u^κ, held at u ≥ 10⁻⁶ so that 0^0 is 1. */
function powered(u: number, kappa: number): number {
  return kappa === 0 ? 1 : pow(Math.max(u, 1e-6), kappa);
}

/** The quadrature's points on u, and on each noise. */
const QUADRATURE_U = 64;
const QUADRATURE_NOISE = 32;

/**
 * G(w_b, w_d), the erosion's mean over the profile of exponent κ and both noises, uniform and
 * independent (the module's remarks): by the midpoint rule over 64 points of u and 32 of each
 * noise, whose error is under 10⁻³ of G (`cloudShape.test.ts`).
 */
export function cloudShapeNormalisation(
  baseShare: number,
  detailShare: number,
  kappa: number,
): number {
  let total = 0;
  for (let i = 0; i < QUADRATURE_U; i += 1) {
    const u = (i + 0.5) / QUADRATURE_U;
    let mean = 0;
    for (let a = 0; a < QUADRATURE_NOISE; a += 1) {
      const nb = (a + 0.5) / QUADRATURE_NOISE;
      for (let b = 0; b < QUADRATURE_NOISE; b += 1) {
        const nd = (b + 0.5) / QUADRATURE_NOISE;
        mean += erosion(1 - u, nb, nd, baseShare, detailShare);
      }
    }
    total += (1 + kappa) * powered(u, kappa) * (mean / (QUADRATURE_NOISE * QUADRATURE_NOISE));
  }
  return total / QUADRATURE_U;
}

/** Each κ's table, made once. */
const TABLES = new Map<number, Float32Array>();

/**
 * G tabled at w_b = i ÷ 8 and w_d = j ÷ 8, at 9 i + j ({@link NORMALISATION_STEPS}²), made once a κ;
 * {@link normalisationAt} reads it bilinear, as `cloudShape.wgsl` does.
 */
export function normalisationTable(kappa: number): Float32Array {
  let table = TABLES.get(kappa);
  if (table === undefined) {
    const n = NORMALISATION_STEPS;
    table = new Float32Array(n * n);
    for (let i = 0; i < n; i += 1) {
      for (let j = 0; j < n; j += 1) {
        table[i * n + j] = cloudShapeNormalisation(i / (n - 1), j / (n - 1), kappa);
      }
    }
    TABLES.set(kappa, table);
  }
  return table;
}

/** G at (w_b, w_d), bilinear in `table` ({@link normalisationTable}). */
export function normalisationAt(
  table: Float32Array,
  baseShare: number,
  detailShare: number,
): number {
  const n = NORMALISATION_STEPS;
  const x = clamp01(baseShare) * (n - 1);
  const y = clamp01(detailShare) * (n - 1);
  const i0 = Math.min(Math.floor(x), n - 2);
  const j0 = Math.min(Math.floor(y), n - 2);
  const fx = x - i0;
  const fy = y - j0;
  const at = (i: number, j: number): number => table[i * n + j] ?? 1;
  const top = at(i0, j0) + fy * (at(i0, j0 + 1) - at(i0, j0));
  const bottom = at(i0 + 1, j0) + fy * (at(i0 + 1, j0 + 1) - at(i0 + 1, j0));
  return top + fx * (bottom - top);
}

/** A column of the field as the density reads it: what the cloud map holds a texel. */
export interface CloudColumn {
  /** f, the covered share of the footprint. */
  readonly fraction: number;
  /** The footprint's mean τ(550). */
  readonly meanTau: number;
  /** η_top ({@link deckTopShare}). */
  readonly topShare: number;
  /** s, the field's unresolved standard deviation there. */
  readonly unresolvedSd: number;
  /** ē, the covered part's mean excess, in standard deviations of z. */
  readonly excess: number;
  /** C, the coverage's share there, Q(z_c). */
  readonly cover: number;
}

/** The least τ̄ N an excess is divided by. */
const LEAST_SCALE = 1e-9;

/**
 * The column at `point` over a footprint of `footprintM` (the module's remarks): the field's share,
 * mean τ and unresolved deviation, and its top share from its excess and the coverage there.
 *
 * @param point - On the layer, body-fixed, m.
 */
export function cloudColumnAt(
  coverage: CoverageTexels,
  noise: Uint8Array,
  params: CloudFieldParams,
  kind: CloudShapeKind,
  point: Vec3,
  footprintM: number,
  octaveCap?: number,
): CloudColumn {
  const sample = cloudFieldAt(coverage, noise, params, point, footprintM, octaveCap);
  const cell = sampleCoverage(coverage, point);
  const excess = sample.inCloudTau / Math.max(params.meanTau * cell.scale, LEAST_SCALE);
  const cover = normalTail(cell.threshold);
  return {
    fraction: sample.fraction,
    meanTau: sample.meanTau,
    topShare: deckTopShare(excess, cover, kind.cumuliform),
    unresolvedSd: sample.unresolvedSd,
    excess,
    cover,
  };
}

/**
 * A cloud map's texel at a place over a footprint (`cloudMap.ts`' remarks): the field's resolved
 * sum there, Σ w_k ρᵏ n_k = z_r √(Σρ²ᵏ), and the coverage's threshold and wind.
 *
 * @param point - On the layer, body-fixed, m.
 */
export function cloudMapTexelAt(
  coverage: CoverageTexels,
  noise: Uint8Array,
  params: CloudFieldParams,
  point: Vec3,
  footprintM: number,
): CloudMapTexel {
  const sample = cloudFieldAt(coverage, noise, params, point, footprintM);
  const cell = sampleCoverage(coverage, point);
  return {
    resolvedSum: sample.resolved * Math.sqrt(octavePowerOf(params)),
    threshold: cell.threshold,
    eastMps: cell.eastMps,
    northMps: cell.northMps,
  };
}

/** What a column read from the map needs of its body's field: its noise tile and parameters. */
export interface ColumnField {
  readonly noise: Uint8Array;
  readonly params: CloudFieldParams;
}

/**
 * A sample's column from a read of the cloud map (`cloudMap.ts`' remarks): the map's resolved sum
 * at its levels' weights, w̄_k = w_k(fine) + blend (w_k(coarse) − w_k(fine)), with, where `refine`
 * holds, each octave whose weight at `footprintM` differs from w̄_k added at the place,
 * (w_k(F) − w̄_k) ρᵏ n_k, so that the sum is the field's at F; then the field's share, mean τ and
 * unresolved deviation from it (`cloudDensity.ts`' `fieldFromParts`) and the top share from its
 * excess. Without `refine`, the map's own footprint's column: a march towards the source's.
 *
 * @param point - On the layer, body-fixed, m.
 */
export function columnFromMap(
  field: ColumnField,
  kind: CloudShapeKind,
  point: Vec3,
  footprintM: number,
  read: CloudMapRead,
  refine: boolean,
): CloudColumn {
  const { params } = field;
  const { texel } = read;
  const n = params.octaves;
  const mapWeights = new Float64Array(n);
  const weights = new Float64Array(n);
  let from = n;
  let to = 0;
  for (let k = 0; k < n; k += 1) {
    const fine = octaveWeightAt(params, k, read.fineM);
    const mapped = fine + read.blend * (octaveWeightAt(params, k, read.coarseM) - fine);
    mapWeights[k] = mapped;
    const own = refine ? octaveWeightAt(params, k, footprintM) : mapped;
    weights[k] = own;
    if (own !== mapped) {
      from = Math.min(from, k);
      to = k + 1;
    }
  }
  const noises =
    to > from
      ? octaveNoisesAt(field.noise, params, point, texel.eastMps, texel.northMps, from, to)
      : null;
  let sum = texel.resolvedSum;
  let variance = 0;
  let amplitude = 1;
  for (let k = 0; k < n; k += 1) {
    const w = weights[k] ?? 0;
    if (noises !== null && k >= from && k < to) {
      sum += (w - (mapWeights[k] ?? 0)) * amplitude * (noises[k] ?? 0);
    }
    variance += (w * amplitude) ** 2;
    amplitude *= CLOUD_OCTAVE_RATIO;
  }
  const scale = scaleOfThreshold(texel.threshold);
  const sample = fieldFromParts(params, texel.threshold, scale, sum, variance);
  const excess = sample.inCloudTau / Math.max(params.meanTau * scale, LEAST_SCALE);
  const cover = normalTail(texel.threshold);
  return {
    fraction: sample.fraction,
    meanTau: sample.meanTau,
    topShare: deckTopShare(excess, cover, kind.cumuliform),
    unresolvedSd: sample.unresolvedSd,
    excess,
    cover,
  };
}

/**
 * What a sample's density takes besides its column: the deck, the kind, the noises' shares and the
 * flattening.
 */
export interface DensityInputs {
  /** The deck's base above the datum, m. */
  readonly baseM: number;
  /** The deck's depth D, m, above 0. */
  readonly depthM: number;
  readonly kind: CloudShapeKind;
  /** κ's normalisation table ({@link normalisationTable}). */
  readonly table: Float32Array;
  /** φ, the flattening towards the layer's slab, in [0, 1]. */
  readonly flat: number;
}

/** A sample's noises and their shares. */
export interface DensityNoise {
  /** n_b there, in [0, 1]. */
  readonly base: number;
  /** n_d there as stored, in [0, 1]. */
  readonly detail: number;
  /** w_b, in [0, 1], before the flattening. */
  readonly baseShare: number;
  /** w_d, in [0, 1], before the flattening; 0 for a sample that reads no detail. */
  readonly detailShare: number;
}

/** A sample's place in its column. */
export interface ColumnPlace {
  /** u = η ÷ η_top, the share of the column's cloud below the sample; outside [0, 1) no cloud. */
  readonly u: number;
  /** The column's top share after the flattening. */
  readonly topShare: number;
}

/** A sample at `heightM` above the datum in a column of top share `topShare` (the module's remarks). */
export function columnPlace(heightM: number, topShare: number, inputs: DensityInputs): ColumnPlace {
  const top = topShare + inputs.flat * (1 - topShare);
  return { u: (heightM - inputs.baseM) / (inputs.depthM * top), topShare: top };
}

/**
 * σ(550), the extinction per metre at a sample (the module's remarks).
 *
 * @param columnTau - τ_col, the column's τ the density is laid through.
 */
export function densityPerM(
  columnTau: number,
  place: ColumnPlace,
  noise: DensityNoise,
  inputs: DensityInputs,
): number {
  const { u } = place;
  if (!(u >= 0 && u < 1) || !(columnTau > 0)) {
    return 0;
  }
  const { flat } = inputs;
  const kappa = inputs.kind.profileExponent;
  const profile = (1 + kappa) * powered(u, kappa);
  const shaped = profile + flat * (1 - profile);
  const wb = noise.baseShare * (1 - flat);
  const wd = noise.detailShare * (1 - flat);
  const g = erosion(1 - u, noise.base, turnedDetail(noise.detail, u), wb, wd);
  const eroded = g + flat * (1 - g);
  const mean = normalisationAt(inputs.table, wb, wd);
  const normalised = mean + flat * (1 - mean);
  return (columnTau * shaped * eroded) / (inputs.depthM * place.topShare * normalised);
}

/**
 * The share of the column's τ above a sample by its profile alone, 1 − P(u), P = u^(1 + κ) mixed
 * to u by the flattening: the plane-parallel sun's path and the sky's light read it.
 */
export function shareAbove(place: ColumnPlace, inputs: DensityInputs): number {
  const u = clamp01(place.u);
  const kappa = inputs.kind.profileExponent;
  const below = u * powered(u, kappa);
  return 1 - (below + inputs.flat * (u - below));
}

/** A noise's share at a footprint: 1 while its feature `featureM` spans 4 footprints, 0 below 2. */
export function noiseShare(featureM: number, footprintM: number): number {
  return clamp01(log2(featureM / (2 * Math.max(footprintM, 1e-3))));
}

/** The base noise's tile, m, for a deck of depth D: 4 D, held to 2–40 km (lean, 2026-11-19). */
export function baseTileM(depthM: number): number {
  return Math.min(Math.max(4 * depthM, 2_000), 40_000);
}

/** The detail noise's tile, m: D ÷ 2, held to 250 m–5 km (lean, 2026-11-19). */
export function detailTileM(depthM: number): number {
  return Math.min(Math.max(depthM / 2, 250), 5_000);
}

/** The curl noise's tile over the detail's. */
export const CURL_TILES = 4;

/** The curl's displacement of the detail's coordinates at its typical strength, in detail tiles. */
export const CURL_REACH = 0.3;

/** The base noise's feature for its fade, in tiles: its finest Perlin octave's cell, an eighth. */
export const BASE_FEATURE_TILES = 1 / 8;

/** The detail noise's feature for its fade, in tiles: its middle octave's cell, a quarter. */
export const DETAIL_FEATURE_TILES = 1 / 4;

/** A world's deck's noise frame this frame: its tiles, and each noise's drift with the wind. */
export interface CloudShapeFrame {
  readonly baseTileM: number;
  readonly detailTileM: number;
  /** The base noise's drift, in its tiles, each in [0, 1). */
  readonly baseDrift: Vec3;
  /** The detail noise's drift, in its tiles. */
  readonly detailDrift: Vec3;
  /** Each noise's share left by the time rate, in [0, 1]. */
  readonly baseRateShare: number;
  readonly detailRateShare: number;
}

/**
 * A deck's noise frame at a time: the noises drift with the wind at the camera's place, `windM` its
 * body-fixed velocity, m s⁻¹, so that their billows move with the field's structure there; each
 * drift is formed in `f64` and taken modulo its tile.
 *
 * @param timeS - The scene's time, s, which may be large; only its product with the wind is read.
 * @param rate - Simulation seconds per real second, which fades each noise as the field's octaves.
 */
export function cloudShapeFrame(
  depthM: number,
  windM: Vec3,
  timeS: number,
  rate: number,
): CloudShapeFrame {
  const base = baseTileM(depthM);
  const detail = detailTileM(depthM);
  const drift = (tile: number): Vec3 => {
    const fract = (v: number): number => {
      const x = (-v * timeS) / tile;
      return x - Math.floor(x);
    };
    return { x: fract(windM.x), y: fract(windM.y), z: fract(windM.z) };
  };
  const speed = Math.hypot(windM.x, windM.y, windM.z);
  return {
    baseTileM: base,
    detailTileM: detail,
    baseDrift: drift(base),
    detailDrift: drift(detail),
    baseRateShare: timeRateFade(rate, base * BASE_FEATURE_TILES, speed),
    detailRateShare: timeRateFade(rate, detail * DETAIL_FEATURE_TILES, speed),
  };
}

function wrap(i: number, n: number): number {
  return ((i % n) + n) % n;
}

/**
 * A 3D tile of `size`³ bytes at `u`, in tiles, trilinear between texel centres at (i + ½) ÷ size
 * with `repeat`, as a GPU's linear sampler filters an `r8unorm` texture: in [0, 1].
 */
export function sampleTile(texels: Uint8Array, size: number, u: Vec3): number {
  const tx = u.x * size - 0.5;
  const ty = u.y * size - 0.5;
  const tz = u.z * size - 0.5;
  const x0 = Math.floor(tx);
  const y0 = Math.floor(ty);
  const z0 = Math.floor(tz);
  const fx = tx - x0;
  const fy = ty - y0;
  const fz = tz - z0;
  const at = (dx: number, dy: number, dz: number): number =>
    (texels[(wrap(z0 + dz, size) * size + wrap(y0 + dy, size)) * size + wrap(x0 + dx, size)] ?? 0) /
    255;
  const x00 = at(0, 0, 0) + fx * (at(1, 0, 0) - at(0, 0, 0));
  const x10 = at(0, 1, 0) + fx * (at(1, 1, 0) - at(0, 1, 0));
  const x01 = at(0, 0, 1) + fx * (at(1, 0, 1) - at(0, 0, 1));
  const x11 = at(0, 1, 1) + fx * (at(1, 1, 1) - at(0, 1, 1));
  const y0v = x00 + fy * (x10 - x00);
  const y1v = x01 + fy * (x11 - x01);
  return y0v + fz * (y1v - y0v);
}

/** The curl's 2D tile at (x, y), in tiles, bilinear with `repeat`, in [0, 1]. */
export function sampleCurlPotential(texels: Uint8Array, x: number, y: number): number {
  const n = CURL_NOISE_SIZE;
  const tx = x * n - 0.5;
  const ty = y * n - 0.5;
  const x0 = Math.floor(tx);
  const y0 = Math.floor(ty);
  const fx = tx - x0;
  const fy = ty - y0;
  const at = (dx: number, dy: number): number =>
    (texels[wrap(y0 + dy, n) * n + wrap(x0 + dx, n)] ?? 0) / 255;
  const top = at(0, 0) + fx * (at(1, 0) - at(0, 0));
  const bottom = at(0, 1) + fx * (at(1, 1) - at(0, 1));
  return top + fy * (bottom - top);
}

/**
 * The scale from one texel's difference of the curl's potential to its displacement's strength:
 * a potential of 4 features a tile, uniform once equalised, changes by about 1/16 a texel at most.
 */
export const CURL_GAIN = 16;

/**
 * The curl's displacement at (x, y), in the curl's tiles, in detail tiles: (∂ψ ÷ ∂y, −∂ψ ÷ ∂x) by
 * forward differences of a texel, times {@link CURL_GAIN} and {@link CURL_REACH}, as `cloudShape.wgsl`
 * takes it.
 */
export function curlDisplacement(
  texels: Uint8Array,
  x: number,
  y: number,
): { readonly x: number; readonly y: number } {
  const h = 1 / CURL_NOISE_SIZE;
  const centre = sampleCurlPotential(texels, x, y);
  const alongX = sampleCurlPotential(texels, x + h, y) - centre;
  const alongY = sampleCurlPotential(texels, x, y + h) - centre;
  const k = CURL_GAIN * CURL_REACH;
  return { x: k * alongY, y: -k * alongX };
}

/** The base noise at a body-fixed place, m, in [0, 1]. */
export function baseNoiseAt(texels: Uint8Array, frame: CloudShapeFrame, placeM: Vec3): number {
  const k = 1 / frame.baseTileM;
  return sampleTile(texels, BASE_NOISE_SIZE, {
    x: placeM.x * k + frame.baseDrift.x,
    y: placeM.y * k + frame.baseDrift.y,
    z: placeM.z * k + frame.baseDrift.z,
  });
}

/**
 * The detail noise at a body-fixed place, m, its coordinates moved by the curl in the plane of
 * `east` and `north` (unit, body-fixed), the curl read at the place's (`planeM`) coordinates in that
 * plane, m; in [0, 1].
 */
export function detailNoiseAt(
  detail: Uint8Array,
  curl: Uint8Array,
  frame: CloudShapeFrame,
  placeM: Vec3,
  planeM: { readonly x: number; readonly y: number },
  east: Vec3,
  north: Vec3,
): number {
  const k = 1 / frame.detailTileM;
  const curlScale = k / CURL_TILES;
  const moved = curlDisplacement(curl, planeM.x * curlScale, planeM.y * curlScale);
  return sampleTile(detail, DETAIL_NOISE_SIZE, {
    x: placeM.x * k + frame.detailDrift.x + moved.x * east.x + moved.y * north.x,
    y: placeM.y * k + frame.detailDrift.y + moved.x * east.y + moved.y * north.y,
    z: placeM.z * k + frame.detailDrift.z + moved.x * east.z + moved.y * north.z,
  });
}
