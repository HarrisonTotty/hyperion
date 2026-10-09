/**
 * The eclipse term of a lit point: the fraction of a limb-darkened star's light that bodies in
 * front of it leave visible (plan R07, Design note 6).
 *
 * @remarks
 * The star's disc is split into K annuli, each of uniform intensity carrying its exact share of the
 * power-2 law's flux, I(μ) ÷ I(1) = 1 − c (1 − μ^α) (Hestroffer 1997, A&A 327, 199, eq. 4; Maxted
 * 2018, A&A 616, A39, Sect. 1), with edges placed per law by equal concentric error ("equal dip",
 * decision-r07-dn6, 2026-10-03): the minimax flux-exact partition. An annulus's eclipsed area is
 * the difference of two exact circle–circle overlaps, so the term is continuous in every argument
 * and never bands. For the Sun (Maxted 2018, Table 2) the worst absolute error is 0.70% (B), 0.56%
 * (V) and 0.46% (R) at K = 4, and 1.23%, 0.97% and 0.81% at K = 3 (the low setting). Geometry is angular, in units of
 * the star's angular radius, as seen from the lit point, and the disc is taken as flat in angle (an
 * error of order ρ² ÷ 12, about 1% at ρ = 19.5°, estimated).
 */
import { cross, dot, norm, normalise, sub, type Vec3 } from "../../geometry/vec3";

/** Annuli of the high setting. */
export const DISC_ANNULI_HIGH = 4;

/**
 * Annuli of the low setting: 3, by the orchestrator's ruling under the owner's delegation
 * (2026-10-03), the Sun's worst error 0.8–1.2% against 1.7–2.7% at K = 2, for one more overlap
 * term per lit texel while an eclipse is on.
 */
export const DISC_ANNULI_LOW = 3;

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
  /**
   * The worst concentric error of every annulus, equal across them: the term's predicted worst
   * absolute error, which the plan's error grid meets to within 3 × 10⁻⁴.
   */
  readonly dip: number;
}

/** ∫₀^μ I(μ′) μ′ dμ′ for the power-2 law, per unit I(1). */
function power2Moment(c: number, alpha: number, mu: number): number {
  return ((1 - c) * mu * mu) / 2 + (c * mu ** (alpha + 2)) / (alpha + 2);
}

/** Bisection steps, for the level and for each edge. */
const EQUAL_DIP_STEPS = 60;

/**
 * The flux inside area a = r² of a unit power-2 disc, Φ(a) = 1 − M(√(1 − a)) ÷ M(1); concave in a,
 * its slope I(μ) ÷ 2M(1).
 */
function cumulativeFlux(c: number, alpha: number, a: number): number {
  const total = power2Moment(c, alpha, 1);
  return 1 - power2Moment(c, alpha, Math.sqrt(Math.max(0, 1 - a))) / total;
}

/**
 * The worst concentric error of a uniform annulus over areas [a₀, a₁] holding its exact flux: the
 * largest gap between Φ and its chord, at the area where the law's intensity equals the annulus's.
 */
function annulusDip(c: number, alpha: number, a0: number, a1: number): number {
  if (a1 <= a0) {
    return 0;
  }
  const f0 = cumulativeFlux(c, alpha, a0);
  const slope = (cumulativeFlux(c, alpha, a1) - f0) / (a1 - a0);
  const meanIntensity = 2 * power2Moment(c, alpha, 1) * slope;
  const muStar = c > 0 ? Math.max(0, (meanIntensity - (1 - c)) / c) ** (1 / alpha) : 1;
  const aStar = Math.min(Math.max(1 - muStar * muStar, a0), a1);
  return cumulativeFlux(c, alpha, aStar) - f0 - slope * (aStar - a0);
}

/** The area edges that march outward from the centre with every annulus but the last at `level`. */
function marchAtLevel(c: number, alpha: number, k: number, level: number): Float64Array {
  const areas = new Float64Array(k + 1);
  areas[k] = 1;
  for (let j = 1; j < k; j += 1) {
    const a0 = areas[j - 1] ?? 0;
    let low = a0;
    let high = 1;
    for (let step = 0; step < EQUAL_DIP_STEPS; step += 1) {
      const middle = (low + high) / 2;
      if (annulusDip(c, alpha, a0, middle) <= level) {
        low = middle;
      } else {
        high = middle;
      }
    }
    areas[j] = low;
  }
  return areas;
}

/**
 * K annuli of a power-2 disc with equal worst concentric error, each with its exact flux.
 *
 * @remarks
 * Bisects a level D: marching out from the centre, each edge is the furthest at which its annulus's
 * dip is at most D, and D rises while the last annulus's dip exceeds it (decision-r07-dn6). The
 * result matches a Nelder–Mead minimax to 0.001%. Computed per star and channel on the CPU, in
 * `f64`.
 *
 * A uniform disc (c = 0) has every partition exact, where the bisection would push every edge to
 * the limb and leave annuli of no area (whose eclipsed share is 0 ÷ 0, in the shader too): it takes
 * K annuli of equal area and flux instead.
 *
 * @param c - The power-2 law's c, 0 to 1.
 * @param alpha - The power-2 law's α, positive.
 * @param k - The number of annuli, at least 1.
 */
export function annulusEdges(c: number, alpha: number, k: number): AnnulusSet {
  if (c === 0) {
    const edges = new Float64Array(k + 1).map((_, j) => Math.sqrt(j / k));
    return { edges, flux: new Float64Array(k).fill(1 / k), dip: 0 };
  }
  let low = 0;
  let high = 0.5;
  for (let step = 0; step < EQUAL_DIP_STEPS; step += 1) {
    const level = (low + high) / 2;
    const areas = marchAtLevel(c, alpha, k, level);
    if (annulusDip(c, alpha, areas[k - 1] ?? 0, 1) > level) {
      low = level;
    } else {
      high = level;
    }
  }
  const areas = marchAtLevel(c, alpha, k, high);
  const edges = areas.map(Math.sqrt);
  const flux = new Float64Array(k);
  for (let j = 0; j < k; j += 1) {
    flux[j] = cumulativeFlux(c, alpha, areas[j + 1] ?? 1) - cumulativeFlux(c, alpha, areas[j] ?? 0);
  }
  return { edges, flux, dip: high };
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

/** One occluder's disc against a star's as a point sees them, in the star's angular radii. */
export interface Occultation {
  /** The occluder's angular radius over the star's. */
  readonly ratio: number;
  /** The angle between their centres over the star's angular radius. */
  readonly separation: number;
}

/**
 * How one occluder stands against a star from a point: `"inside"` where the point is within the
 * occluder, which hides the whole star; `null` where it hides none of the star, lying beyond it or
 * clear of its disc; else its {@link Occultation}.
 *
 * @param star - The star's centre and radius, m, in the frame of the point and the occluder.
 * @param fromM - The point, m.
 */
export function occultationFrom(
  star: { readonly centreM: Vec3; readonly radiusM: number },
  fromM: Vec3,
  occluder: Occluder,
): Occultation | "inside" | null {
  const toStar = sub(star.centreM, fromM);
  const starDistanceM = norm(toStar);
  const starRadiusRad = Math.asin(Math.min(star.radiusM / starDistanceM, 1));
  const toOccluder = sub(occluder.centreM, fromM);
  const distanceM = norm(toOccluder);
  if (distanceM <= occluder.radiusM) {
    return "inside";
  }
  if (distanceM >= starDistanceM) {
    return null;
  }
  const occluderRadiusRad = Math.asin(occluder.radiusM / distanceM);
  // atan2 of the cross and dot products keeps small separations exact, where acos loses √ε.
  const occluderDirection = normalise(toOccluder);
  const starDirection = normalise(toStar);
  const separationRad = Math.atan2(
    norm(cross(occluderDirection, starDirection)),
    dot(occluderDirection, starDirection),
  );
  if (separationRad >= occluderRadiusRad + starRadiusRad) {
    return null;
  }
  return { ratio: occluderRadiusRad / starRadiusRad, separation: separationRad / starRadiusRad };
}

/**
 * The fraction of one channel's flux from `disc` visible from `fromM` past `occluders`.
 *
 * @remarks
 * Each occluder nearer than the star and within the sum of the two angular radii of its direction
 * hides the overlap of its disc with the star's ({@link occultationFrom}); occluders are taken not
 * to overlap one another, which holds for the shadow cones `occludersFor` (`occluders.ts`) keeps,
 * and their hidden fractions add. A point inside an occluder sees nothing of the star.
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
  const annuli = annulusEdges(disc.limbC, disc.limbAlpha, k);
  let visible = 1;
  for (const occluder of occluders) {
    const occultation = occultationFrom(disc, fromM, occluder);
    if (occultation === "inside") {
      return 0;
    }
    if (occultation !== null) {
      visible -= 1 - annulusVisibleFraction(annuli, occultation.ratio, occultation.separation);
    }
  }
  return Math.max(0, visible);
}
