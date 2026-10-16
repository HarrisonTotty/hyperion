/**
 * The terrain's shadow on itself as a star's horizon term (plan R10, R10.T8.b; Design note 10): the
 * share of a star's disc above a local horizon, and R07's `sphere_irradiance` above the horizon map's
 * horizon, the TypeScript twin of `shadows/horizon.wgsl`'s `terrain_horizon` and its call in
 * `terrainLit.wgsl`.
 *
 * @remarks
 * The lit fraction is the share of a uniform disc of angular radius ρ above a horizon at e_h with
 * the disc's centre at e★: 1 − (arccos x − x √(1 − x²)) ÷ π, x = (e★ − e_h) ÷ ρ clamped to
 * [−1, 1], without limb darkening, which moves no half-light point. R07's horizon term takes a
 * local horizon as an elevation above the element's own tangent plane towards the star (its
 * oracle's cone about the normal, `lighting/oracle.ts`), and its visible fraction of the disc is
 * this formula with x measured from that plane; the map's horizon is against the datum's
 * horizontal, so it is carried into the element's frame first ({@link elementHorizonRad}), and the
 * disc is then cut exactly where its centre stands at e_h above the datum, or at the element's own
 * plane where that hides more. R07's function is used unchanged (`sphereIrradianceFactor`).
 */

import { sphereIrradianceFactor } from "../../lighting/sphereIrradiance";

/**
 * The most stellar radii a star's distance is taken as in the terrain's horizon term: 10¹⁸, ρ of
 * 10⁻¹⁸ rad, a hard step to every precision the term carries, while the H² that R07's
 * `sphereIrradianceFactor` forms stays finite in `f32` (10³⁶ against 3.4 × 10³⁸). A point star,
 * H infinite, would make R07's term ∞ × 0, NaN; the guard is the shadow code's, R07's function
 * unchanged (`shadows/horizon.wgsl`'s `terrain_star_distance_radii`).
 */
export const MAX_STAR_DISTANCE_RADII = 1e18;

/**
 * The share of a star's disc above a horizon (Design note 10): 0 with the disc wholly below it, 1
 * wholly above, ½ with its centre on it; for a point star, ρ = 0, the hard step, ½ on the horizon.
 *
 * @param starElevationRad - e★, the elevation of the disc's centre, rad.
 * @param horizonRad - e_h, the horizon's elevation towards it, rad.
 * @param starRadiusRad - ρ, the disc's angular radius, rad; 0 or positive.
 */
export function litFraction(
  starElevationRad: number,
  horizonRad: number,
  starRadiusRad: number,
): number {
  if (!(starRadiusRad > 0)) {
    return starElevationRad > horizonRad ? 1 : starElevationRad < horizonRad ? 0 : 0.5;
  }
  const x = Math.min(Math.max((starElevationRad - horizonRad) / starRadiusRad, -1), 1);
  return 1 - (Math.acos(x) - x * Math.sqrt(1 - x * x)) / Math.PI;
}

/**
 * The map's horizon carried into the element's frame towards the star, as R07's horizon term takes
 * its `horizon`: e_h − (e★ − (π ÷ 2 − φ)), the star's elevation above the datum less its elevation
 * above the element's plane taken off; e_h itself where the element's normal is the datum's.
 *
 * @param horizonRad - e_h, against the datum's horizontal, rad.
 * @param starElevationRad - e★, the star's elevation above the datum's horizontal, rad.
 * @param phiRad - φ, the star's angle from the element's normal, rad.
 */
export function elementHorizonRad(
  horizonRad: number,
  starElevationRad: number,
  phiRad: number,
): number {
  return horizonRad - (starElevationRad - (Math.PI / 2 - phiRad));
}

/**
 * A star's horizon term on a terrain element above the map's horizon: R07's
 * `sphereIrradianceFactor(h, φ, horizon)` at {@link elementHorizonRad}, H taken at most
 * {@link MAX_STAR_DISTANCE_RADII}, the irradiance over that of the star face-on.
 *
 * @param h - H, the star's distance over its radius; above 1, infinite for a point star.
 * @param phiRad - φ, the star's angle from the element's normal, rad.
 * @param starElevationRad - e★, the star's elevation above the datum's horizontal, rad.
 * @param horizonRad - e_h, the map's horizon towards the star, against the datum's horizontal, rad.
 */
export function terrainHorizonFactor(
  h: number,
  phiRad: number,
  starElevationRad: number,
  horizonRad: number,
): number {
  return sphereIrradianceFactor(
    Math.min(h, MAX_STAR_DISTANCE_RADII),
    phiRad,
    elementHorizonRad(horizonRad, starElevationRad, phiRad),
  );
}
