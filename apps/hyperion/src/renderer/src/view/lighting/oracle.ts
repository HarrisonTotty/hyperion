/**
 * The `f64` oracles that the lighting and shading closed forms are tested against (plan R07,
 * Design notes 5 and 6): slow, dense numerical integrals, never called by the renderer.
 */
import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  annulusEdges,
  annulusVisibleFraction,
  circleOverlapArea,
  type LimbDarkenedDisc,
  type Occluder,
  occultationFrom,
} from "./annuli";
import { gaussLegendre } from "./quadrature";

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
 *
 * @remarks
 * It shares `circleOverlapArea` with the term it checks; that function is tested on its own
 * against closed forms (the lens, containment, continuity), and the concentric case, which needs
 * no overlap, is checked against the law's closed form.
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

/** Midpoints of the eclipse integral oracle's quadrature in t = μ^α. */
const ECLIPSE_INTEGRAL_STEPS = 8000;

/**
 * The exact fraction of a power-2 disc's flux left visible by a disc of radius `ratio` stellar
 * radii at `separation` stellar radii from its centre (decision-r07-dn6).
 *
 * @remarks
 * With A(r) the overlap of the occulter with the star's disc cut at radius r, the hidden flux is
 * (1 − c) A(1) + c ∫₀¹ A(√(1 − t^(2 ÷ α))) dt (t = μ^α, the region where μ^α exceeds t), over the
 * disc's total π [(1 − c) + 2c ÷ (α + 2)]; the integral by the midpoint rule over 8,000 steps.
 * It agrees with {@link denseAnnulusVisibleFraction} to about 10⁻⁵.
 */
export function eclipseIntegralVisibleFraction(
  c: number,
  alpha: number,
  ratio: number,
  separation: number,
): number {
  let integral = 0;
  for (let i = 0; i < ECLIPSE_INTEGRAL_STEPS; i += 1) {
    const t = (i + 0.5) / ECLIPSE_INTEGRAL_STEPS;
    integral += circleOverlapArea(Math.sqrt(Math.max(0, 1 - t ** (2 / alpha))), ratio, separation);
  }
  integral /= ECLIPSE_INTEGRAL_STEPS;
  const hidden = (1 - c) * circleOverlapArea(1, ratio, separation) + c * integral;
  const total = Math.PI * (1 - c + (2 * c) / (alpha + 2));
  return Math.max(0, 1 - hidden / total);
}

/** A power-2 law I(μ) ÷ I(1) = 1 − c (1 − μ^α) as a {@link LimbProfile}. */
export function power2Profile(c: number, alpha: number): LimbProfile {
  return (mu) => 1 - c * (1 - mu ** alpha);
}

/** One far point's view of a body: its unit direction from the body, the law's L and the whole. */
export interface FarView {
  readonly towards: Vec3;
  readonly share: number;
  /** ∫ (I/F) μ dS ÷ A f(α) over the lit, visible body: π R² [L + ⅔ (1 − L)] Φ_shape(α), m². */
  readonly totalM2: number;
}

/**
 * The fraction of a body's reflected light towards far points that one occluder's shadow leaves,
 * by brute force over the surface (plan R07, T10.b's oracle): V̄ = 1 − ∫ (1 − V) (I/F) μ dS ÷
 * ∫ (I/F) μ dS, one channel's eclipse term V taken at each surface point itself, never along a
 * shadow axis.
 *
 * @remarks
 * The midpoint rule over `side` × `side` points in (θ, φ), θ the angle of the normal from the
 * star's direction (μ₀ = cos θ, the lit hemisphere) and φ the azimuth about it,
 * dS = R² sin θ dθ dφ, over the region whose projection along the star's direction is the
 * occluder's penumbra there (its exact tangent cone at the plane through the centre, widened by
 * the axis's drift across the lit depth). At each point V is the eclipse term (`occultationFrom` and the K annuli,
 * `eclipseVisible`'s arithmetic) and (I/F) μ ∝ [L · 2μ₀ ÷ (μ₀ + μ) + (1 − L) μ₀] μ for μ > 0,
 * continuous over the surface. Each view's whole is its `totalM2`.
 *
 * @param body - The body's centre and radius, m, a sphere.
 * @param views - The far points, each with its law's L; V is shared by all of them.
 * @param side - Points along each coordinate.
 */
export function discEclipseBruteForce(
  star: LimbDarkenedDisc,
  body: { readonly centreM: Vec3; readonly radiusM: number },
  occluder: Occluder,
  views: ReadonlyArray<FarView>,
  k: number,
  side = 1000,
): number[] {
  const radius = body.radiusM;
  const starM = sub(star.centreM, body.centreM);
  const d = norm(starM);
  const s = scale(starM, 1 / d);
  const seed = Math.abs(s.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const e1 = normalise(cross(seed, s));
  const e2 = cross(s, e1);
  const occluderM = sub(occluder.centreM, body.centreM);
  const relative = { centreM: occluderM, radiusM: occluder.radiusM };
  const starSphere = { centreM: starM, radiusM: star.radiusM };
  const annuli = annulusEdges(star.limbC, star.limbAlpha, k);
  // The penumbra's disc about the axis's crossing of the plane through the centre.
  const axis = sub(occluderM, starM);
  const axisLength = norm(axis);
  const t = -d / dot(axis, s);
  const crossing = add(starM, scale(axis, t));
  const behind = (t - 1) * axisLength;
  const sinGamma = (star.radiusM + occluder.radiusM) / axisLength;
  const penumbra = (occluder.radiusM + behind * sinGamma) / Math.sqrt(1 - sinGamma * sinGamma);
  const unitAxis = scale(axis, 1 / axisLength);
  const reach = penumbra + (norm(cross(unitAxis, s)) / Math.abs(dot(unitAxis, s))) * radius;
  const b = Math.hypot(dot(crossing, e1), dot(crossing, e2));
  const rhoLow = Math.max(0, b - reach);
  const rhoHigh = Math.min(radius, b + reach);
  if (!(rhoLow < rhoHigh)) {
    return views.map(() => 1);
  }
  const theta0 = Math.asin(rhoLow / radius);
  const theta1 = Math.asin(rhoHigh / radius);
  const towardsAxis = Math.atan2(dot(crossing, e2), dot(crossing, e1));
  const halfWidth = b > reach ? Math.asin(reach / b) : Math.PI;
  const phi0 = towardsAxis - halfWidth;
  const dTheta = (theta1 - theta0) / side;
  const dPhi = (2 * halfWidth) / side;
  const deficits = views.map(() => 0);
  for (let i = 0; i < side; i += 1) {
    const theta = theta0 + (i + 0.5) * dTheta;
    const mu0 = Math.cos(theta);
    const sinTheta = Math.sin(theta);
    const area = radius * radius * sinTheta * dTheta * dPhi;
    for (let j = 0; j < side; j += 1) {
      const phi = phi0 + (j + 0.5) * dPhi;
      const u = sinTheta * Math.cos(phi);
      const v = sinTheta * Math.sin(phi);
      const normal = vec3(
        u * e1.x + v * e2.x + mu0 * s.x,
        u * e1.y + v * e2.y + mu0 * s.y,
        u * e1.z + v * e2.z + mu0 * s.z,
      );
      const occultation = occultationFrom(starSphere, scale(normal, radius), relative);
      if (occultation === null) {
        continue;
      }
      const hidden =
        occultation === "inside"
          ? 1
          : 1 - annulusVisibleFraction(annuli, occultation.ratio, occultation.separation);
      for (let n = 0; n < views.length; n += 1) {
        const view = views[n];
        const mu = view === undefined ? 0 : dot(normal, view.towards);
        if (view === undefined || mu <= 0) {
          continue;
        }
        const reflected = ((view.share * 2 * mu0) / (mu0 + mu) + (1 - view.share) * mu0) * mu;
        deficits[n] = (deficits[n] ?? 0) + hidden * reflected * area;
      }
    }
  }
  return views.map((view, n) => 1 - (deficits[n] ?? 0) / view.totalM2);
}
