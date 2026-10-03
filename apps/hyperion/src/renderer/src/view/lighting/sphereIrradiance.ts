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
 * one, an error of order ρ² sin η). η = 0 is Howell's form exactly. A horizon below the tangent
 * plane is taken as 0, since the element's own plane still hides what is behind it.
 *
 * @param h - H = d ÷ R★, the distance to the star's centre in stellar radii; above 1.
 * @param phiRad - φ, the angle between the element's normal and the star's centre, 0 to π.
 * @param horizonRad - The local horizon's elevation towards the star, rad; 0 on the smooth figure.
 */
export function sphereIrradianceFactor(h: number, phiRad: number, horizonRad = 0): number {
  const eta = Math.max(horizonRad, 0);
  if (eta === 0) {
    return h * h * howellViewFactor(h, phiRad);
  }
  const tilted = phiRad + eta;
  const direct = Math.cos(eta) * h * h * howellViewFactor(h, tilted);
  const tangential = Math.sin(eta) * Math.sin(tilted) * visibleFraction(h, tilted);
  return Math.max(0, direct + tangential);
}

/**
 * Howell's view factor F from a planar element to a sphere at H = d ÷ R, the element's normal at φ
 * to the sphere's centre.
 *
 * @param h - H, above 1.
 * @param phiRad - φ, rad; beyond π it is folded back, since the factor is even in φ.
 */
export function howellViewFactor(h: number, phiRad: number): number {
  const phi = Math.min(Math.abs(phiRad), Math.PI);
  const h2 = h * h;
  const edge = Math.acos(1 / h);
  if (phi <= edge) {
    return Math.cos(phi) / h2;
  }
  if (phi >= Math.PI - edge) {
    return 0;
  }
  const x = Math.sqrt(h2 - 1);
  const y = -x / Math.tan(phi);
  const root = Math.sqrt(Math.max(0, 1 - y * y));
  return (
    (Math.cos(phi) * Math.acos(y) - x * Math.sin(phi) * root) / (Math.PI * h2) +
    Math.atan((Math.sin(phi) * root) / x) / Math.PI
  );
}

/**
 * The fraction of a star's disc above a plane at φ from its centre, as a flat disc of angular
 * radius ρ = asin(1/H) cut by a line at the centre's elevation π/2 − φ.
 */
function visibleFraction(h: number, phiRad: number): number {
  const rho = Math.asin(1 / h);
  const x = Math.min(Math.max((Math.PI / 2 - phiRad) / rho, -1), 1);
  return (Math.acos(-x) + x * Math.sqrt(1 - x * x)) / Math.PI;
}
