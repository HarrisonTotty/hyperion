/**
 * The `f64` oracles that the lighting and shading closed forms are tested against (plan R07,
 * Design notes 5 and 6): slow, dense numerical integrals, never called by the renderer.
 */
import { circleOverlapArea } from "./annuli";

/** A reflectance I/F as a function of μ₀, μ and the phase angle, rad. */
export type Reflectance = (mu0: number, mu: number, phaseRad: number) => number;

/**
 * The disc integral of a reflectance at phase α: (1 ÷ π) ∫ (I/F) μ dS over the lit, visible
 * hemisphere of a unit sphere, which is p Φ(α) for the law, p at α = 0.
 *
 * @remarks
 * In photometric coordinates, luminance longitude Λ from the sub-observer point towards the Sun
 * and latitude B: μ = cos B cos Λ, μ₀ = cos B cos(α − Λ), dS = cos B dB dΛ, over Λ from α − π/2
 * to π/2 and B from −π/2 to π/2. Gauss–Legendre in both, `nodes` per axis; the integrand is smooth
 * inside the domain, so a few hundred nodes reach 10⁻⁸.
 *
 * @param phaseRad - Phase angle, rad, 0 to π.
 * @param nodes - Gauss–Legendre nodes per axis.
 */
export function discIntegral(reflectance: Reflectance, phaseRad: number, nodes = 200): number {
  const { x, w } = gaussLegendre(nodes);
  const lowLon = phaseRad - Math.PI / 2;
  const highLon = Math.PI / 2;
  const halfLon = (highLon - lowLon) / 2;
  const midLon = (highLon + lowLon) / 2;
  let sum = 0;
  for (let i = 0; i < nodes; i += 1) {
    const lon = midLon + halfLon * (x[i] ?? 0);
    let inner = 0;
    for (let j = 0; j < nodes; j += 1) {
      const lat = (Math.PI / 2) * (x[j] ?? 0);
      const cosLat = Math.cos(lat);
      const mu = cosLat * Math.cos(lon);
      const mu0 = cosLat * Math.cos(phaseRad - lon);
      if (mu > 0 && mu0 > 0) {
        inner += (w[j] ?? 0) * reflectance(mu0, mu, phaseRad) * mu * cosLat;
      }
    }
    sum += (w[i] ?? 0) * inner * (Math.PI / 2);
  }
  return (sum * halfLon) / Math.PI;
}

/** A Gauss–Legendre rule on [−1, 1]. */
export interface GaussLegendreRule {
  /** The nodes. */
  readonly x: Float64Array;
  /** The weights, summing to 2. */
  readonly w: Float64Array;
}

/** Gauss–Legendre nodes and weights on [−1, 1], by Newton's method on Pₙ. */
export function gaussLegendre(n: number): GaussLegendreRule {
  const x = new Float64Array(n);
  const w = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    let z = Math.cos((Math.PI * (i + 0.75)) / (n + 0.5));
    let derivative = 0;
    for (let iteration = 0; iteration < 100; iteration += 1) {
      let p0 = 1;
      let p1 = 0;
      for (let k = 1; k <= n; k += 1) {
        const p2 = p1;
        p1 = p0;
        p0 = ((2 * k - 1) * z * p1 - (k - 1) * p2) / k;
      }
      derivative = (n * (z * p0 - p1)) / (z * z - 1);
      const step = p0 / derivative;
      z -= step;
      if (Math.abs(step) < 1e-15) {
        break;
      }
    }
    x[i] = z;
    w[i] = 2 / ((1 - z * z) * derivative * derivative);
  }
  return { x, w };
}

/** A star's intensity across its disc, I(μ) ÷ I(1), μ the cosine at the star's surface. */
export type LimbProfile = (mu: number) => number;

/** A uniform disc. */
export const UNIFORM_DISC: LimbProfile = () => 1;

/**
 * The irradiance on a surface element from a limb-darkened sphere by brute force, over that of the
 * same sphere face-on (plan R07, Design note 6).
 *
 * @remarks
 * Sums I(μ★) (n · ω) dΩ over the directions ω to the star's disc, in polar coordinates about its
 * centre: θ from 0 to ρ = asin(1/H) by Gauss–Legendre, the azimuth by the midpoint rule; along a
 * direction θ off the centre the ray meets the star at μ★ = √(1 − (sin θ ÷ sin ρ)²). A direction
 * counts where it is above the element's tangent plane and above a local horizon of elevation
 * `horizonRad`, uniform in azimuth.
 *
 * @param h - H = d ÷ R★, above 1.
 * @param phiRad - The angle between the element's normal and the star's centre, rad.
 * @param horizonRad - The local horizon's elevation, rad.
 * @param radialNodes - Gauss–Legendre nodes in θ.
 * @param azimuthSteps - Midpoint steps in azimuth.
 */
export function sphereIrradianceBruteForce(
  h: number,
  phiRad: number,
  horizonRad: number,
  profile: LimbProfile,
  radialNodes = 200,
  azimuthSteps = 2000,
): number {
  const rho = Math.asin(1 / h);
  const sinRho = Math.sin(rho);
  const { x, w } = gaussLegendre(radialNodes);
  const sinHorizon = Math.sin(Math.max(horizonRad, 0));
  // The centre at (sin φ, 0, cos φ) about the normal z; e₁ and e₂ span the plane across it.
  const centre = [Math.sin(phiRad), 0, Math.cos(phiRad)] as const;
  const across1 = [Math.cos(phiRad), 0, -Math.sin(phiRad)] as const;
  const dPsi = (2 * Math.PI) / azimuthSteps;
  let lit = 0;
  let faceOn = 0;
  for (let i = 0; i < radialNodes; i += 1) {
    const theta = (rho / 2) * (1 + (x[i] ?? 0));
    const ratio = Math.min(Math.sin(theta) / sinRho, 1);
    const intensity = profile(Math.sqrt(1 - ratio * ratio));
    const weight = (w[i] ?? 0) * (rho / 2) * Math.sin(theta) * intensity;
    faceOn += weight * 2 * Math.PI * Math.cos(theta);
    let ring = 0;
    for (let k = 0; k < azimuthSteps; k += 1) {
      const psi = (k + 0.5) * dPsi;
      const c = Math.cos(theta);
      const s = Math.sin(theta);
      // ω = cos θ ĉ + sin θ (cos ψ e₁ + sin ψ e₂), e₂ = ŷ; its z component is n · ω.
      const nz = c * centre[2] + s * Math.cos(psi) * across1[2];
      if (nz > sinHorizon) {
        ring += nz;
      }
    }
    lit += weight * ring * dPsi;
  }
  return lit / faceOn;
}

/** Annuli of the dense eclipse oracle. */
const DENSE_ANNULI = 400;

/**
 * The fraction of a power-2 disc's flux (I(μ)/I(1) = 1 − c (1 − μ^α)) left visible by a disc of
 * radius `ratio` stellar radii at `separation` stellar radii from its centre, by 400 annuli
 * uniform in radius, each carrying its exact flux and eclipsed by exact circle overlaps (Design
 * note 6's reference for the K-annulus term).
 */
export function denseAnnulusVisibleFraction(
  c: number,
  alpha: number,
  ratio: number,
  separation: number,
): number {
  const moment = (mu: number): number =>
    ((1 - c) * mu * mu) / 2 + (c * mu ** (alpha + 2)) / (alpha + 2);
  const total = moment(1);
  let hidden = 0;
  let innerOverlap = 0;
  let innerRadius = 0;
  for (let j = 1; j <= DENSE_ANNULI; j += 1) {
    const outerRadius = j / DENSE_ANNULI;
    const flux =
      (moment(Math.sqrt(1 - innerRadius * innerRadius)) -
        moment(Math.sqrt(Math.max(0, 1 - outerRadius * outerRadius)))) /
      total;
    const outerOverlap = circleOverlapArea(outerRadius, ratio, separation);
    const area = Math.PI * (outerRadius * outerRadius - innerRadius * innerRadius);
    hidden += (flux * (outerOverlap - innerOverlap)) / area;
    innerOverlap = outerOverlap;
    innerRadius = outerRadius;
  }
  return Math.max(0, 1 - hidden);
}
