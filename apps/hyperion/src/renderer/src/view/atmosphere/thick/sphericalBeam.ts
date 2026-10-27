/**
 * The pseudo-spherical beam (plan R08, R08.T14.b; Design note 9): the direct beam's transmission to
 * each interface of a column of homogeneous spherical shells, and each layer's slant factor, for
 * the solver's `BeamPath` seam (`discreteOrdinates.ts`), after A. Dahlback and K. Stamnes, "A new
 * spherical model for computing the radiation field available for photolysis and heating at
 * twilight", Planet. Space Sci. 39(5) (1991) 671–683, doi:10.1016/0032-0633(91)90061-E (cited by
 * its DOI record and the solver's seam, not re-read).
 *
 * @remarks
 * The column stands on a sphere of radius R (a latitude band's ground, `bake.ts`), each layer a
 * shell between two interfaces' radii R + h. The point at interface k, radius r_k, sees the sun at
 * the zenith angle θ₀ of its own vertical, the column's one μ₀ = cos θ₀ in (0, 1]: the
 * pseudo-spherical approximation scatters with that μ₀ in every layer, and takes the beam alone in
 * spherical geometry.
 *
 * - **The slant depth.** The ray from r_k towards the sun meets radius r ≥ r_k after the distance
 *   s(r) = (r² − r_k²) ÷ (√(r² − r_k²(1 − μ₀²)) + r_k μ₀), the stable form of
 *   √(r² − r_k² sin²θ₀) − r_k μ₀, with r² − r_k² = (h − h_k)(2R + h + h_k). Through shell q above
 *   it, of vertical optical depth τ_q over its thickness Δh_q, it crosses τ_q (s(r_top) −
 *   s(r_bottom)) ÷ Δh_q, so τ_s(k) is that sum over the shells above k. For μ₀ > 0 the ray climbs
 *   from r_k, so it never meets the ground: the beam is defined at every interface of (0, 1].
 * - **The slant factor.** λ_p = (τ_s(p + 1) − τ_s(p)) ÷ τ_p, the average secant, so that the beam
 *   inside layer p is T_p e^(−λ_p t) and T_{p+1} = T_p e^(−λ_p τ_p) holds at each interface.
 *   A deeper point's ray crosses each shell above more steeply, so with the sun low a thin layer
 *   under a thick one can see τ_s fall: the beam then grows downward, which e^(−λt), λ > 0, cannot
 *   carry. There λ_p is floored at {@link SLANT_FACTOR_FLOOR} and the deficit is made up below,
 *   each λ taken from the accumulated slant depth (Σ λ_q τ_q above), so that T matches e^(−τ_s)
 *   at every interface past the next layer that can take it. A layer of τ 0 takes its geometric
 *   slant, its chord over its thickness for the ray from its base.
 */

import type { BeamPath } from "./discreteOrdinates";

/** The least slant factor a layer takes where the spherical slant depth falls downward (the module's remarks). */
export const SLANT_FACTOR_FLOOR = 1e-6;

/** A beam through a column of shells: the solver's seam, and the transmission at each interface. */
export interface SphericalBeam extends BeamPath {
  /** μ₀. */
  readonly beamCosine: number;
  /** T at each interface from the top down, L + 1, the ground last: e^(−Σ λ_q τ_q) above it. */
  readonly interfaceTransmission: Float64Array;
  /** τ_s at each interface, the slant optical depth in spherical geometry, L + 1. */
  readonly slantOpticalDepth: Float64Array;
  /** How many layers took {@link SLANT_FACTOR_FLOOR} (the module's remarks). */
  readonly flooredLayers: number;
  /**
   * The largest e^(−τ_s) − T over the interfaces: the beam the floor withholds, of the incident 1
   * normal to it; 0 where no layer took the floor.
   */
  readonly largestDeficit: number;
}

/** s(r) for the ray from height `fromM` at cosine μ₀: its distance to height `toM` ≥ `fromM`. */
function distanceTo(bottomRadiusM: number, fromM: number, toM: number, mu0: number): number {
  const rk = bottomRadiusM + fromM;
  const radial = (toM - fromM) * (2 * bottomRadiusM + toM + fromM);
  return radial / (Math.sqrt(radial + rk * rk * mu0 * mu0) + rk * mu0);
}

/**
 * The pseudo-spherical beam through a column of homogeneous shells (the module's remarks).
 *
 * @param bottomRadiusM - R, the ground sphere's radius, m.
 * @param interfaceHeightsM - The interfaces' heights above the ground, m, from the top down,
 *   strictly descending: L + 1 of them for L layers.
 * @param layerOpticalDepths - τ_p of each layer from the top down, finite and at least 0.
 * @param beamCosine - μ₀, in (0, 1].
 * @throws RangeError for a radius not finite and above 0, heights not strictly descending to at
 *   least 0, optical depths of another count or not finite and at least 0, or μ₀ outside (0, 1].
 */
export function sphericalBeamPath(
  bottomRadiusM: number,
  interfaceHeightsM: Float64Array,
  layerOpticalDepths: ArrayLike<number>,
  beamCosine: number,
): SphericalBeam {
  const layers = interfaceHeightsM.length - 1;
  if (!(Number.isFinite(bottomRadiusM) && bottomRadiusM > 0)) {
    throw new RangeError(`a column's sphere has a radius above 0, not ${bottomRadiusM} m`);
  }
  if (!(beamCosine > 0 && beamCosine <= 1)) {
    throw new RangeError(`μ₀ lies in (0, 1], not ${beamCosine}`);
  }
  if (layers < 1 || layerOpticalDepths.length !== layers) {
    throw new RangeError(
      `${layers + 1} interfaces take ${layers} optical depths, not ${layerOpticalDepths.length}`,
    );
  }
  for (let k = 0; k <= layers; k += 1) {
    const h = interfaceHeightsM[k] ?? Number.NaN;
    const below = k === layers ? -Infinity : (interfaceHeightsM[k + 1] ?? Number.NaN);
    if (!(Number.isFinite(h) && h > below && (k < layers || h >= 0))) {
      throw new RangeError(`a column's interfaces descend strictly to 0 or above: ${h} m at ${k}`);
    }
  }
  const depthOf = (p: number): number => {
    const tau = layerOpticalDepths[p] ?? Number.NaN;
    if (!(Number.isFinite(tau) && tau >= 0)) {
      throw new RangeError(`layer ${p}'s optical depth is finite and at least 0, not ${tau}`);
    }
    return tau;
  };
  const height = (k: number): number => interfaceHeightsM[k] ?? Number.NaN;
  const slant = new Float64Array(layers + 1);
  for (let k = 1; k <= layers; k += 1) {
    const from = height(k);
    let sum = 0;
    for (let q = 0; q < k; q += 1) {
      const tau = depthOf(q);
      if (tau === 0) {
        continue;
      }
      const top = height(q);
      const bottom = height(q + 1);
      const chord =
        distanceTo(bottomRadiusM, from, top, beamCosine) -
        distanceTo(bottomRadiusM, from, bottom, beamCosine);
      sum += (tau * chord) / (top - bottom);
    }
    slant[k] = sum;
  }
  const topTransmission = new Float64Array(layers);
  const slantFactor = new Float64Array(layers);
  const interfaceTransmission = new Float64Array(layers + 1);
  interfaceTransmission[0] = 1;
  let accumulated = 0;
  let flooredLayers = 0;
  for (let p = 0; p < layers; p += 1) {
    topTransmission[p] = Math.exp(-accumulated);
    const tau = depthOf(p);
    let lambda: number;
    if (tau === 0) {
      const top = height(p);
      const bottom = height(p + 1);
      lambda = distanceTo(bottomRadiusM, bottom, top, beamCosine) / (top - bottom);
    } else {
      lambda = ((slant[p + 1] ?? Number.NaN) - accumulated) / tau;
      if (!(lambda >= SLANT_FACTOR_FLOOR)) {
        lambda = SLANT_FACTOR_FLOOR;
        flooredLayers += 1;
      }
    }
    slantFactor[p] = lambda;
    accumulated += lambda * tau;
    interfaceTransmission[p + 1] = Math.exp(-accumulated);
  }
  let largestDeficit = 0;
  if (flooredLayers > 0) {
    for (let k = 1; k <= layers; k += 1) {
      largestDeficit = Math.max(
        largestDeficit,
        Math.exp(-(slant[k] ?? Number.NaN)) - (interfaceTransmission[k] ?? Number.NaN),
      );
    }
  }
  return {
    beamCosine,
    topTransmission,
    slantFactor,
    interfaceTransmission,
    slantOpticalDepth: slant,
    flooredLayers,
    largestDeficit,
  };
}
