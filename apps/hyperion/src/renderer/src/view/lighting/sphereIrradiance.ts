/**
 * The horizon term of a lit point: the irradiance from a star's whole disc on a surface element
 * whose tangent plane may cut it (plan R07, Design note 6).
 *
 * @remarks
 * Howell's catalogue of radiation view factors, the differential planar element tilted at φ to the
 * direction of a sphere's centre (<https://www.thermalradiation.net/tablecon.html>, configuration
 * B-43; Cunningham 1961, Hauptmann 1968), with X = √(H² − 1) and Y = −X cot φ for H = d ÷ R★:
 *
 * - φ ≤ arccos(1/H), the whole disc above the plane: F = cos φ ÷ H²;
 * - φ ≥ π − arccos(1/H), none of it: F = 0;
 * - between, F = [cos φ arccos Y − X sin φ √(1 − Y²)] ÷ (π H²) + arctan[sin φ √(1 − Y²) ÷ X] ÷ π.
 *
 * Exact for a uniform disc. For a limb-darkened star 19.5° in radius it errs by 0.47% of the
 * face-on value at φ = 90° with the brainstorm's solar polynomial, and by 0.61%, 0.47% and 0.39% in
 * B, V and R with the Sun's power-2 laws (Maxted 2018, Table 2); the error falls about as 1 ÷ H
 * (decision-r07-dn6). The factor returned is H² F, the irradiance over that of the disc face-on
 * (E = π L̄ sin²ρ), so that it is cos φ wherever the whole disc is up. It softens the terminator,
 * → 2 ÷ (3π H) of the face-on value at φ = 90° for H ≫ 1 (3.5% above that at H = 3), and lights
 * a close-in planet beyond its hemisphere, to 109.5° at H = 3.
 *
 * The partial branch is evaluated in a form in which nothing cancels that matters of face-on
 * (R07.T6.d), the arithmetic of `shaders/litBody.wgsl`'s `howell_irradiance`, which is the reason
 * for it: as written, its terms (H² ÷ π) arctan z and X sin φ √(1 − Y²) ÷ π,
 * z = sin φ √(1 − Y²) ÷ X, are each about H ÷ π of face-on and cancel to about 2 ÷ (3H²) of
 * themselves at 90°, so that a relative error ε in an `f32` arctan arrives as (H ÷ π) ε of face-on,
 * 68 ε at 1 au (decision-r10-t8b §2). Since X sin φ √(1 − Y²) = X² z = (H² − 1) z, their sum is
 * z + H² (arctan z − z), an identity; with q = 1 ÷ H = sin ρ, the band's coordinate c = H cos φ
 * (|c| < 1) and u = √(1 − c²) ÷ cos ρ, so that z = q u, it is
 *
 * H² F = (q ÷ π) [c arccos Y + u (1 + u² B(q u))], B(z) = (arctan z − z) ÷ z³ ({@link atanTail}),
 *
 * with arccos Y = atan2(√(1 − c²), −c cos ρ). No H² is formed, so it is finite at every H,
 * H = ∞ (ρ = 0, a point star) included, where it is the hard step.
 */

/**
 * The irradiance from a uniform sphere on an element, over that of the sphere face-on.
 *
 * @remarks
 * A local horizon of elevation η in the star's azimuth (R10's horizon map; 0 on the smooth figure)
 * is a plane tilted by η towards the star: the disc is cut by it, at φ + η in Howell's form, and
 * the light that passes is weighed by the element's own normal. With n = cos η n_h + sin η t, the
 * horizon plane's normal n_h and its tangent t towards the star, the factor is
 * cos η · H² F(φ + η) + sin η · sin(φ + η) · v, where v is the visible fraction of the disc as a
 * small disc cut by a line (exact for the first term; the second treats the disc's directions as
 * one, an error of order ρ² sin η). η = 0 is Howell's form exactly, clamped at 0. A horizon below
 * the tangent plane is taken as 0, since the element's own plane still hides what is behind it.
 *
 * @param h - H = d ÷ R★, the distance to the star's centre in stellar radii; above 1, and infinite
 *   for a point star.
 * @param phiRad - φ, the angle between the element's normal and the star's centre, 0 to π.
 * @param horizonRad - The local horizon's elevation towards the star, rad; 0 on the smooth figure.
 */
export function sphereIrradianceFactor(h: number, phiRad: number, horizonRad = 0): number {
  const eta = Math.max(horizonRad, 0);
  if (eta === 0) {
    return Math.max(0, howellIrradiance(h, phiRad));
  }
  // Past π the disc is wholly cut off, by either term, whatever η is (as the WGSL clamps it).
  const tilted = Math.min(Math.max(phiRad + eta, 0), Math.PI);
  const direct = Math.cos(eta) * howellIrradiance(h, tilted);
  const tangential = Math.sin(eta) * Math.sin(tilted) * visibleFraction(h, tilted);
  return Math.max(0, direct + tangential);
}

/**
 * Howell's view factor F from a planar element to a sphere at H = d ÷ R, the element's normal at φ
 * to the sphere's centre.
 *
 * @param h - H, above 1; 0 is returned for H infinite.
 * @param phiRad - φ, rad; beyond π it is folded back, since the factor is even in φ.
 */
export function howellViewFactor(h: number, phiRad: number): number {
  return howellIrradiance(h, phiRad) / (h * h);
}

/**
 * (arctan z − z) ÷ z³ for z ≥ 0, −1/3 at 0: the arctangent's series beyond its first term, over z³,
 * as `shaders/litBody.wgsl`'s `atan_tail` takes it.
 *
 * @remarks
 * Three halvings, arctan z − z = 2 (arctan t − t) − z³ ÷ (1 + √(1 + z²))² with
 * t = z ÷ (1 + √(1 + z²)), take any z to t ≤ tan(π ÷ 16) = 0.199 (arctan t is arctan z ÷ 8 < π ÷ 16),
 * where the series −1/3 + t²/5 − t⁴/7 + t⁶/9 − t⁸/11, alternating with falling terms, truncates at
 * under t¹⁰/13 = 7.5 × 10⁻⁹, 2.3 × 10⁻⁸ of itself. Per halving B(z) = (2 B(t) − p) ÷ p³,
 * p = 1 + √(1 + z²), a sum of two negative terms, which carries a relative error in B(t) into B(z)
 * at under a quarter of itself, so B(z) is truncated by under 4 × 10⁻¹⁰ of itself.
 *
 * @param z - The argument, 0 or above.
 */
export function atanTail(z: number): number {
  const p0 = 1 + Math.sqrt(1 + z * z);
  const t1 = z / p0;
  const p1 = 1 + Math.sqrt(1 + t1 * t1);
  const t2 = t1 / p1;
  const p2 = 1 + Math.sqrt(1 + t2 * t2);
  const t3 = t2 / p2;
  const t = t3 * t3;
  let b = -1 / 3 + t * (1 / 5 + t * (-1 / 7 + t * (1 / 9 + t * (-1 / 11))));
  b = (2 * b - p2) / (p2 * p2 * p2);
  b = (2 * b - p1) / (p1 * p1 * p1);
  return (2 * b - p0) / (p0 * p0 * p0);
}

/**
 * H² F: Howell's view factor over that of the sphere face-on, in the form of the module's remarks.
 *
 * @param h - H, above 1, or infinite.
 * @param phiRad - φ, rad; folded into [0, π].
 */
function howellIrradiance(h: number, phiRad: number): number {
  const phi = Math.min(Math.abs(phiRad), Math.PI);
  const sinRho = 1 / h;
  const cosPhi = Math.cos(phi);
  if (cosPhi >= sinRho) {
    return cosPhi;
  }
  if (cosPhi <= -sinRho) {
    return 0;
  }
  const c = Math.min(Math.max(cosPhi * h, -1), 1);
  const cosRho = Math.sqrt((1 - sinRho) * (1 + sinRho));
  const chord = Math.sqrt((1 - c) * (1 + c));
  const u = chord / cosRho;
  const arc = Math.atan2(chord, -c * cosRho);
  return (sinRho / Math.PI) * (c * arc + u * (1 + u * u * atanTail(sinRho * u)));
}

/**
 * The fraction of a star's disc above a plane at φ from its centre, as a flat disc of angular
 * radius ρ = asin(1/H) cut by a line at the centre's elevation π/2 − φ; for ρ = 0 the step, ½ on
 * the plane.
 */
function visibleFraction(h: number, phiRad: number): number {
  const rho = Math.asin(1 / h);
  const offset = Math.PI / 2 - phiRad;
  if (Math.abs(offset) >= rho) {
    return offset > 0 ? 1 : offset < 0 ? 0 : 0.5;
  }
  const x = Math.min(Math.max(offset / rho, -1), 1);
  return (Math.acos(-x) + x * Math.sqrt(1 - x * x)) / Math.PI;
}
