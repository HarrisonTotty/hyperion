/**
 * The eclipse term of a lit point: the fraction of a limb-darkened star's light that bodies in
 * front of it leave visible (plan R07, Design note 6).
 *
 * @remarks
 * The star's disc is split into K annuli with edges uniform in μ (μ_j = 1 − j ÷ K, radius
 * √(1 − μ_j²) in stellar radii), each of uniform intensity carrying its exact share of the power-2
 * law's flux, I(μ) ÷ I(1) = 1 − c (1 − μ^α) (Maxted 2018, A&A 616, A39). An annulus's eclipsed
 * area is the difference of two exact circle–circle overlaps, so the term is continuous in every
 * argument and never bands. Design note 6 states 0.62% worst absolute error at K = 4 and 1.5% at
 * K = 2; against a 400-annulus oracle the worst case, a concentric occulter, measures 0.73% and
 * 2.6% for a Sun-like power-2 law (c 0.71, α 0.6), pending decision-r07-dn6.
 * Geometry is angular, in units of the star's angular radius, as seen from the lit point.
 */
import { cross, dot, norm, normalise, sub, type Vec3 } from "../../geometry/vec3";

/** Annuli of the high setting. */
export const DISC_ANNULI_HIGH = 4;

/** Annuli of the low setting. */
export const DISC_ANNULI_LOW = 2;

/** A star's disc as the eclipse term needs it, one display channel's limb darkening. */
export interface LimbDarkenedDisc {
  /** The star's centre, m, in the frame of the lit point and the occluders. */
  readonly centreM: Vec3;
  readonly radiusM: number;
  /** The power-2 law's c for the channel. */
  readonly limbC: number;
  /** The power-2 law's α for the channel. */
  readonly limbAlpha: number;
}

/** A body that may stand between a lit point and a star. */
export interface Occluder {
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/** K annuli of a power-2 disc: their edges and the flux each carries. */
export interface AnnulusSet {
  /** K + 1 radii in stellar radii, from 0 at the centre to 1 at the limb. */
  readonly edges: Float64Array;
  /** K fractions of the disc's flux, summing to 1, centre first. */
  readonly flux: Float64Array;
}

/** ∫₀^μ I(μ′) μ′ dμ′ for the power-2 law, per unit I(1). */
function power2Moment(c: number, alpha: number, mu: number): number {
  return ((1 - c) * mu * mu) / 2 + (c * mu ** (alpha + 2)) / (alpha + 2);
}

/**
 * The edges of K annuli uniform in μ and the share of the power-2 disc's flux in each.
 *
 * @param c - The power-2 law's c, 0 to 1.
 * @param alpha - The power-2 law's α, positive.
 * @param k - The number of annuli, at least 1.
 */
export function annulusEdges(c: number, alpha: number, k: number): AnnulusSet {
  const edges = new Float64Array(k + 1);
  const flux = new Float64Array(k);
  const total = power2Moment(c, alpha, 1);
  for (let j = 0; j <= k; j += 1) {
    const mu = 1 - j / k;
    edges[j] = Math.sqrt(1 - mu * mu);
    if (j > 0) {
      flux[j - 1] = (power2Moment(c, alpha, 1 - (j - 1) / k) - power2Moment(c, alpha, mu)) / total;
    }
  }
  return { edges, flux };
}

/** The area of a circle of radius `radius` beyond a chord `d` from its centre. */
function circularSegment(radius: number, d: number): number {
  const cosine = Math.min(Math.max(d / radius, -1), 1);
  return radius * radius * Math.acos(cosine) - d * Math.sqrt(Math.max(radius * radius - d * d, 0));
}

/**
 * The exact area of the overlap of two circles of radii r and k whose centres are z apart.
 *
 * @remarks
 * The lens formula, r² acos(d₁ ÷ r) − d₁ √(r² − d₁²) and its twin for k, with the chord's distances
 * d₁ = (z² + r² − k²) ÷ 2z and d₂ = z − d₁; π min(r, k)² when one lies inside the other, 0 when
 * they are apart.
 */
export function circleOverlapArea(r: number, k: number, z: number): number {
  if (r <= 0 || k <= 0 || z >= r + k) {
    return 0;
  }
  if (z <= Math.abs(r - k)) {
    const small = Math.min(r, k);
    return Math.PI * small * small;
  }
  const d1 = (z * z + r * r - k * k) / (2 * z);
  const d2 = z - d1;
  return circularSegment(r, d1) + circularSegment(k, d2);
}

/**
 * The fraction of a unit power-2 disc's flux left visible by a disc of radius `ratio` (in stellar
 * radii) whose centre is `separation` from the star's.
 */
export function annulusVisibleFraction(
  annuli: AnnulusSet,
  ratio: number,
  separation: number,
): number {
  let hidden = 0;
  let inner = 0;
  for (let j = 0; j < annuli.flux.length; j += 1) {
    const outerEdge = annuli.edges[j + 1] ?? 1;
    const innerEdge = annuli.edges[j] ?? 0;
    const outer = circleOverlapArea(outerEdge, ratio, separation);
    const area = Math.PI * (outerEdge * outerEdge - innerEdge * innerEdge);
    hidden += ((annuli.flux[j] ?? 0) * (outer - inner)) / area;
    inner = outer;
  }
  return Math.max(0, 1 - hidden);
}

/**
 * The fraction of one channel's flux from `disc` visible from `fromM` past `occluders`.
 *
 * @remarks
 * Each occluder nearer than the star and within the sum of the two angular radii of its direction
 * hides the overlap of its disc with the star's; occluders are taken not to overlap one another,
 * which holds for the shadow cones `occludersFor` (`occluders.ts`) keeps, and their hidden fractions add. A
 * point inside an occluder sees nothing of the star.
 *
 * @param fromM - The lit point, m, in the frame of the disc and the occluders.
 * @param k - The number of annuli, {@link DISC_ANNULI_HIGH} or {@link DISC_ANNULI_LOW}.
 */
export function eclipseVisible(
  disc: LimbDarkenedDisc,
  fromM: Vec3,
  occluders: ReadonlyArray<Occluder>,
  k: number,
): number {
  const toStar = sub(disc.centreM, fromM);
  const starDistanceM = norm(toStar);
  const starRadiusRad = Math.asin(Math.min(disc.radiusM / starDistanceM, 1));
  const starDirection = normalise(toStar);
  const annuli = annulusEdges(disc.limbC, disc.limbAlpha, k);
  let visible = 1;
  for (const occluder of occluders) {
    const toOccluder = sub(occluder.centreM, fromM);
    const distanceM = norm(toOccluder);
    if (distanceM <= occluder.radiusM) {
      return 0;
    }
    if (distanceM >= starDistanceM) {
      continue;
    }
    const occluderRadiusRad = Math.asin(occluder.radiusM / distanceM);
    // atan2 of the cross and dot products keeps small separations exact, where acos loses √ε.
    const occluderDirection = normalise(toOccluder);
    const separationRad = Math.atan2(
      norm(cross(occluderDirection, starDirection)),
      dot(occluderDirection, starDirection),
    );
    if (separationRad >= occluderRadiusRad + starRadiusRad) {
      continue;
    }
    visible -=
      1 -
      annulusVisibleFraction(
        annuli,
        occluderRadiusRad / starRadiusRad,
        separationRad / starRadiusRad,
      );
  }
  return Math.max(0, visible);
}
