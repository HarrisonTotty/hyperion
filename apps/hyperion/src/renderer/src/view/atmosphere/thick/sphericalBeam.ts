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
 * the zenith angle θ₀ of its own vertical, the column's one μ₀ = cos θ₀ in [−1, 1]: the
 * pseudo-spherical approximation scatters with that μ₀ in every layer, its sign kept, and takes the
 * beam alone in spherical geometry.
 *
 * - **The slant depth.** The ray from r_k towards the sun meets radius r ≥ r_k after the distance
 *   s(r) = (r² − r_k²) ÷ (√(r² − r_k²(1 − μ₀²)) + r_k μ₀), the stable form of
 *   √(r² − r_k² sin²θ₀) − r_k μ₀, with r² − r_k² = (h − h_k)(2R + h + h_k). Through shell q above
 *   it, of vertical optical depth τ_q over its thickness Δh_q, it crosses τ_q (s(r_top) −
 *   s(r_bottom)) ÷ Δh_q, so τ_s(k) is that sum over the shells above k. For μ₀ > 0 the ray climbs
 *   from r_k, so it never meets the ground: the beam is defined at every interface of (0, 1].
 * - **Below the horizontal** (μ₀ ≤ 0, twilight; R08.T14.d.2). The ray first falls to its tangent
 *   radius r_t = r_k √(1 − μ₀²), then climbs: it crosses each shell above r_k once and each between
 *   r_t and r_k twice, so the top interface's T is below 1. Each crossing is the difference of
 *   D(r) = √(r² − r_t²) at the shell's faces, D² = (h − h_k)(2R + h + h_k) + r_k² μ₀², taken in the
 *   stable form (r_a² − r_b²) ÷ (D_a + D_b). Where r_t ≤ R the ray meets the ground: the interface
 *   lies in the planet's shadow, r_k² (1 − μ₀²) ≤ R² (`tablesCpu.ts`'s `groundShadows`, the
 *   kernels' test), with T = 0 and τ_s = +∞, and so does every interface below it. A layer wholly
 *   in the shadow takes no beam (T 0 at its top, λ 1); the layer the shadow's edge cuts, lit at its
 *   top and dark at its base, takes the λ whose beam e^(−λt) deposits over the whole layer the
 *   same ∫T dt as the true beam over its lit part, down to the shadow height h_s, where
 *   (R + h_s)² (1 − μ₀²) = R², at the lit part's own average secant: unique, and continuous in μ₀
 *   as the edge crosses an interface, where a dark layer would step. (cdisort's `c_chapman`
 *   carries the same geometry, read for its behaviour, not ported; for its edge layer it takes
 *   Kylling and Stamnes 1992's form, J. Comput. Phys. 102, 265, as its comments describe it.)
 * - **The slant factor.** λ_p = (τ_s(p + 1) − τ_s(p)) ÷ τ_p, the average secant, so that the beam
 *   inside layer p is T_p e^(−λ_p t) and T_{p+1} = T_p e^(−λ_p τ_p) holds at each interface outside
 *   the shadow. A deeper point's ray crosses each shell above more steeply, so with the sun low a
 *   thin layer under a thick one can see τ_s fall: the beam then grows downward, which e^(−λt),
 *   λ > 0, cannot carry. There λ_p is floored at {@link SLANT_FACTOR_FLOOR} and the deficit is made
 *   up below, each λ taken from the accumulated slant depth (Σ λ_q τ_q above), so that T matches
 *   e^(−τ_s) at every interface past the next layer that can take it. A layer of τ 0 takes its
 *   geometric slant, its chord over its thickness for the ray from its base (below the horizontal,
 *   from where that ray, past its dip, climbs back through the base; for the layer the shadow's
 *   edge cuts, from the shadow height). Its λ moves no T and no source.
 */

import type { BeamPath } from "./discreteOrdinates";

/** The least slant factor a layer takes where the spherical slant depth falls downward (the module's remarks). */
export const SLANT_FACTOR_FLOOR = 1e-6;

/** A beam through a column of shells: the solver's seam, and the transmission at each interface. */
export interface SphericalBeam extends BeamPath {
  /** μ₀. */
  readonly beamCosine: number;
  /**
   * T at each interface from the top down, L + 1, the ground last: e^(−Σ λ_q τ_q) above it, and 0
   * in the planet's shadow (the module's remarks).
   */
  readonly interfaceTransmission: Float64Array;
  /**
   * τ_s at each interface, the slant optical depth in spherical geometry, L + 1; +∞ in the planet's
   * shadow.
   */
  readonly slantOpticalDepth: Float64Array;
  /** How many layers outside the shadow took {@link SLANT_FACTOR_FLOOR} (the module's remarks). */
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
 * D² = r² − r_t² at height `toM` for the ray from height `fromM` at cosine μ₀ (the module's
 * remarks, "Below the horizontal"): the square of its distance from its tangent point, held at 0
 * below it.
 */
function tangentSquare(bottomRadiusM: number, fromM: number, toM: number, mu0: number): number {
  const rk = bottomRadiusM + fromM;
  return Math.max((toM - fromM) * (2 * bottomRadiusM + toM + fromM) + rk * rk * mu0 * mu0, 0);
}

/** The ray's length between heights `lowM` < `highM`, both at or above its tangent point, once. */
function crossing(
  bottomRadiusM: number,
  fromM: number,
  lowM: number,
  highM: number,
  mu0: number,
): number {
  const sum =
    Math.sqrt(tangentSquare(bottomRadiusM, fromM, highM, mu0)) +
    Math.sqrt(tangentSquare(bottomRadiusM, fromM, lowM, mu0));
  return sum > 0 ? ((highM - lowM) * (2 * bottomRadiusM + highM + lowM)) / sum : 0;
}

/**
 * τ_s from height `fromM` for μ₀ ≤ 0, lit (the module's remarks, "Below the horizontal"): the shells
 * above it crossed once, those between it and its tangent point twice, and a shell that holds it
 * (the shadow height, `fromM` between interfaces) split at it.
 */
function slantBelowHorizon(
  bottomRadiusM: number,
  heightOf: (k: number) => number,
  depthOf: (p: number) => number,
  layers: number,
  fromM: number,
  mu0: number,
): number {
  let sum = 0;
  for (let q = 0; q < layers; q += 1) {
    const tau = depthOf(q);
    if (tau === 0) {
      continue;
    }
    const top = heightOf(q);
    const bottom = heightOf(q + 1);
    let chord = 0;
    if (top > fromM) {
      chord += crossing(bottomRadiusM, fromM, Math.max(bottom, fromM), top, mu0);
    }
    const high = Math.min(top, fromM);
    if (high > bottom && tangentSquare(bottomRadiusM, fromM, high, mu0) > 0) {
      chord +=
        tangentSquare(bottomRadiusM, fromM, bottom, mu0) > 0
          ? 2 * crossing(bottomRadiusM, fromM, bottom, high, mu0)
          : 2 * Math.sqrt(tangentSquare(bottomRadiusM, fromM, high, mu0));
    }
    if (chord === 0) {
      continue;
    }
    sum += (tau * chord) / (top - bottom);
  }
  return sum;
}

/** (1 − e^(−x)) ÷ x: the beam e^(−λt) deposits over a layer of τ, per unit of its top's T and τ, x = λτ. */
function deposit(x: number): number {
  return -Math.expm1(-x) / x;
}

/**
 * x = λτ of the layer the shadow's edge cuts (the module's remarks): the root of
 * (1 − e^(−x)) ÷ x = `share`, the lit part's ∫T dt over the layer's τ, at or above `least`, the lit
 * part's own λ τ, where the left side is at least `share`. That side is convex and falls from 1, so
 * the root is bracketed by `least` and 1 ÷ `share`; 64 halvings of the bracket's logarithm.
 */
function edgeSlant(share: number, least: number): number {
  let low = least;
  let high = Math.max(1 / share, least);
  for (let i = 0; i < 64; i += 1) {
    const middle = Math.sqrt(low * high);
    if (deposit(middle) > share) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return Math.sqrt(low * high);
}

/**
 * The pseudo-spherical beam through a column of homogeneous shells (the module's remarks).
 *
 * @param bottomRadiusM - R, the ground sphere's radius, m.
 * @param interfaceHeightsM - The interfaces' heights above the ground, m, from the top down,
 *   strictly descending: L + 1 of them for L layers.
 * @param layerOpticalDepths - τ_p of each layer from the top down, finite and at least 0.
 * @param beamCosine - μ₀, in [−1, 1]: at or below 0, a sun below the point's horizontal.
 * @throws RangeError for a radius not finite and above 0, heights not strictly descending to at
 *   least 0, optical depths of another count or not finite and at least 0, or μ₀ outside [−1, 1].
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
  if (!(beamCosine >= -1 && beamCosine <= 1)) {
    throw new RangeError(`μ₀ lies in [−1, 1], not ${beamCosine}`);
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
  // Every depth, though a column wholly in the shadow reads none.
  for (let p = 0; p < layers; p += 1) {
    depthOf(p);
  }
  if (!(beamCosine > 0)) {
    return beamBelowHorizon(bottomRadiusM, height, depthOf, layers, beamCosine);
  }
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
  return {
    beamCosine,
    topTransmission,
    slantFactor,
    interfaceTransmission,
    slantOpticalDepth: slant,
    flooredLayers,
    largestDeficit: deficitOf(slant, interfaceTransmission, flooredLayers),
  };
}

/** {@link sphericalBeamPath} for μ₀ ≤ 0 (the module's remarks, "Below the horizontal"). */
function beamBelowHorizon(
  bottomRadiusM: number,
  height: (k: number) => number,
  depthOf: (p: number) => number,
  layers: number,
  beamCosine: number,
): SphericalBeam {
  const sines = (1 - beamCosine) * (1 + beamCosine);
  const radius2 = bottomRadiusM * bottomRadiusM;
  const lit = (h: number): boolean => (bottomRadiusM + h) * (bottomRadiusM + h) * sines > radius2;
  const slantFrom = (h: number): number =>
    slantBelowHorizon(bottomRadiusM, height, depthOf, layers, h, beamCosine);
  const slant = new Float64Array(layers + 1);
  for (let k = 0; k <= layers; k += 1) {
    slant[k] = lit(height(k)) ? slantFrom(height(k)) : Number.POSITIVE_INFINITY;
  }
  const topTransmission = new Float64Array(layers);
  const slantFactor = new Float64Array(layers).fill(1);
  const interfaceTransmission = new Float64Array(layers + 1);
  let accumulated = slant[0] ?? Number.NaN;
  interfaceTransmission[0] = Math.exp(-accumulated);
  let flooredLayers = 0;
  for (let p = 0; p < layers && lit(height(p)); p += 1) {
    topTransmission[p] = Math.exp(-accumulated);
    const tau = depthOf(p);
    const top = height(p);
    const bottom = height(p + 1);
    if (lit(bottom)) {
      let lambda: number;
      if (tau === 0) {
        lambda = crossing(bottomRadiusM, bottom, bottom, top, beamCosine) / (top - bottom);
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
      continue;
    }
    // The shadow's edge: (R + h_s)² (1 − μ₀²) = R², h_s = R μ₀² ÷ (s (1 + s)), s = √(1 − μ₀²).
    const s = Math.sqrt(sines);
    const shadowM = (bottomRadiusM * beamCosine * beamCosine) / (s * (1 + s));
    const litShare = (top - shadowM) / (top - bottom);
    if (!(litShare > 0)) {
      // The edge rounds onto the layer's top: the layer is dark.
      topTransmission[p] = 0;
    } else if (tau === 0) {
      slantFactor[p] = crossing(bottomRadiusM, shadowM, shadowM, top, beamCosine) / (top - shadowM);
    } else {
      const litDepth = tau * litShare;
      const litLambda = Math.max((slantFrom(shadowM) - accumulated) / litDepth, SLANT_FACTOR_FLOOR);
      const share = -Math.expm1(-litLambda * litDepth) / (litLambda * tau);
      slantFactor[p] = edgeSlant(share, litLambda * tau) / tau;
    }
  }
  return {
    beamCosine,
    topTransmission,
    slantFactor,
    interfaceTransmission,
    slantOpticalDepth: slant,
    flooredLayers,
    largestDeficit: deficitOf(slant, interfaceTransmission, flooredLayers),
  };
}

/** The largest e^(−τ_s) − T over the interfaces, 0 where no layer took the floor. */
function deficitOf(
  slant: Float64Array,
  interfaceTransmission: Float64Array,
  flooredLayers: number,
): number {
  let largestDeficit = 0;
  if (flooredLayers > 0) {
    for (let k = 1; k < slant.length; k += 1) {
      largestDeficit = Math.max(
        largestDeficit,
        Math.exp(-(slant[k] ?? Number.NaN)) - (interfaceTransmission[k] ?? Number.NaN),
      );
    }
  }
  return largestDeficit;
}
