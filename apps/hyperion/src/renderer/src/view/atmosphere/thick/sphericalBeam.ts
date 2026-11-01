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
import { exp, expm1 } from "../../../lib/pinnedMath";

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
 * The ray's chord through layer `q` from height `fromM` for μ₀ ≤ 0, lit (the module's remarks,
 * "Below the horizontal"): through a shell above it once, through one between it and its tangent
 * point twice, and through a shell that holds it (the shadow height, `fromM` between interfaces)
 * split at it; 0 where it crosses none of the shell.
 */
function chordBelowHorizon(
  bottomRadiusM: number,
  top: number,
  bottom: number,
  fromM: number,
  mu0: number,
): number {
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
  return chord;
}

/**
 * τ_s from height `fromM` for μ₀ ≤ 0, lit (the module's remarks, "Below the horizontal"): each
 * shell's {@link chordBelowHorizon}, for {@link sphericalSlantDepths}, which takes no chords.
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
    const chord = chordBelowHorizon(bottomRadiusM, top, bottom, fromM, mu0);
    if (chord === 0) {
      continue;
    }
    sum += (tau * chord) / (top - bottom);
  }
  return sum;
}

/** τ_s along the chords `chords[at …]`, one a layer: Σ τ_q chord_q ÷ Δh_q over the shells it crosses. */
function slantAlong(
  height: (k: number) => number,
  depthOf: (p: number) => number,
  layers: number,
  chords: Float64Array,
  at: number,
): number {
  let sum = 0;
  for (let q = 0; q < layers; q += 1) {
    const tau = depthOf(q);
    if (tau === 0) {
      continue;
    }
    const chord = chords[at + q] ?? Number.NaN;
    if (chord === 0) {
      continue;
    }
    sum += (tau * chord) / (height(q) - height(q + 1));
  }
  return sum;
}

/** (1 − e^(−x)) ÷ x: the beam e^(−λt) deposits over a layer of τ, per unit of its top's T and τ, x = λτ. */
function deposit(x: number): number {
  return -expm1(-x) / x;
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
 * A beam's chords through a column's shells, which its geometry alone sets
 * ({@link sphericalBeamChords}): for a caller that traces one beam through one column at many
 * optical depths, as a band's every bin and term (plan R08, R08.T16.b.2's lever), so that each
 * trace takes them in place of finding them anew, to the bit.
 */
export interface BeamChords {
  /** The column they were found for: R, m, its interfaces' heights, m, and μ₀. */
  readonly bottomRadiusM: number;
  readonly interfaceHeightsM: Float64Array;
  readonly beamCosine: number;
  /**
   * The ray's chord, m, from interface k through layer q, entry k × L + q: for μ₀ > 0 through the
   * shells above k (q < k), below the horizontal through every shell from a lit interface; 0
   * elsewhere.
   */
  readonly fromInterfacesM: Float64Array;
  /** Below the horizontal, the chords from the shadow height through each layer; else empty. */
  readonly fromShadowM: Float64Array;
  /** The shadow height, m, h_s, below the horizontal (the module's remarks); else NaN. */
  readonly shadowM: number;
  /** Each layer's geometric slant, which a layer of τ 0 takes (the module's remarks); 1 in the shadow. */
  readonly emptySlant: Float64Array;
}

/**
 * The chords of the beam at μ₀ through a column ({@link BeamChords}).
 *
 * @param bottomRadiusM - R, the ground sphere's radius, m.
 * @param interfaceHeightsM - The interfaces' heights above the ground, m, from the top down,
 *   strictly descending: L + 1 of them for L ≥ 1 layers.
 * @param beamCosine - μ₀, in [−1, 1].
 * @throws RangeError as {@link sphericalBeamPath} does for the radius, the heights or μ₀.
 */
export function sphericalBeamChords(
  bottomRadiusM: number,
  interfaceHeightsM: Float64Array,
  beamCosine: number,
): BeamChords {
  checkColumn(bottomRadiusM, interfaceHeightsM, beamCosine);
  return chordsOf(bottomRadiusM, interfaceHeightsM, beamCosine);
}

/** Throws unless the radius, μ₀ and the heights are a column's ({@link sphericalBeamPath}). */
function checkColumn(
  bottomRadiusM: number,
  interfaceHeightsM: Float64Array,
  beamCosine: number,
): void {
  if (!(Number.isFinite(bottomRadiusM) && bottomRadiusM > 0)) {
    throw new RangeError(`a column's sphere has a radius above 0, not ${bottomRadiusM} m`);
  }
  if (!(beamCosine >= -1 && beamCosine <= 1)) {
    throw new RangeError(`μ₀ lies in [−1, 1], not ${beamCosine}`);
  }
  const layers = interfaceHeightsM.length - 1;
  if (layers < 1) {
    throw new RangeError(`a column has at least 2 interfaces, not ${layers + 1}`);
  }
  checkHeights(interfaceHeightsM);
}

/** Throws unless the heights descend strictly to 0 or above. */
function checkHeights(interfaceHeightsM: Float64Array): void {
  const layers = interfaceHeightsM.length - 1;
  for (let k = 0; k <= layers; k += 1) {
    const h = interfaceHeightsM[k] ?? Number.NaN;
    const below = k === layers ? -Infinity : (interfaceHeightsM[k + 1] ?? Number.NaN);
    if (!(Number.isFinite(h) && h > below && (k < layers || h >= 0))) {
      throw new RangeError(`a column's interfaces descend strictly to 0 or above: ${h} m at ${k}`);
    }
  }
}

/** {@link sphericalBeamChords}' chords, the column checked. */
function chordsOf(
  bottomRadiusM: number,
  interfaceHeightsM: Float64Array,
  beamCosine: number,
): BeamChords {
  const layers = interfaceHeightsM.length - 1;
  const height = (k: number): number => interfaceHeightsM[k] ?? Number.NaN;
  const fromInterfacesM = new Float64Array((layers + 1) * layers);
  const emptySlant = new Float64Array(layers).fill(1);
  if (beamCosine > 0) {
    for (let k = 1; k <= layers; k += 1) {
      const from = height(k);
      for (let q = 0; q < k; q += 1) {
        fromInterfacesM[k * layers + q] =
          distanceTo(bottomRadiusM, from, height(q), beamCosine) -
          distanceTo(bottomRadiusM, from, height(q + 1), beamCosine);
      }
    }
    for (let p = 0; p < layers; p += 1) {
      const top = height(p);
      const bottom = height(p + 1);
      emptySlant[p] = distanceTo(bottomRadiusM, bottom, top, beamCosine) / (top - bottom);
    }
    return {
      bottomRadiusM,
      interfaceHeightsM,
      beamCosine,
      fromInterfacesM,
      fromShadowM: new Float64Array(0),
      shadowM: Number.NaN,
      emptySlant,
    };
  }
  const lit = litBelowHorizon(bottomRadiusM, beamCosine);
  for (let k = 0; k <= layers; k += 1) {
    const from = height(k);
    if (!lit(from)) {
      continue;
    }
    for (let q = 0; q < layers; q += 1) {
      fromInterfacesM[k * layers + q] = chordBelowHorizon(
        bottomRadiusM,
        height(q),
        height(q + 1),
        from,
        beamCosine,
      );
    }
  }
  // The shadow's edge: (R + h_s)² (1 − μ₀²) = R², h_s = R μ₀² ÷ (s (1 + s)), s = √(1 − μ₀²).
  const s = Math.sqrt((1 - beamCosine) * (1 + beamCosine));
  const shadowM = (bottomRadiusM * beamCosine * beamCosine) / (s * (1 + s));
  let fromShadowM = new Float64Array(0);
  for (let p = 0; p < layers && lit(height(p)); p += 1) {
    const top = height(p);
    const bottom = height(p + 1);
    if (lit(bottom)) {
      emptySlant[p] = crossing(bottomRadiusM, bottom, bottom, top, beamCosine) / (top - bottom);
      continue;
    }
    // The layer the shadow's edge cuts, the last the beam reaches.
    emptySlant[p] = crossing(bottomRadiusM, shadowM, shadowM, top, beamCosine) / (top - shadowM);
    fromShadowM = Float64Array.from({ length: layers }, (_, q) =>
      chordBelowHorizon(bottomRadiusM, height(q), height(q + 1), shadowM, beamCosine),
    );
  }
  return {
    bottomRadiusM,
    interfaceHeightsM,
    beamCosine,
    fromInterfacesM,
    fromShadowM,
    shadowM,
    emptySlant,
  };
}

/** Whether height h is outside the planet's shadow for μ₀ ≤ 0: (R + h)² (1 − μ₀²) > R². */
function litBelowHorizon(bottomRadiusM: number, beamCosine: number): (h: number) => boolean {
  const sines = (1 - beamCosine) * (1 + beamCosine);
  const radius2 = bottomRadiusM * bottomRadiusM;
  return (h) => (bottomRadiusM + h) * (bottomRadiusM + h) * sines > radius2;
}

/**
 * The pseudo-spherical beam through a column of homogeneous shells (the module's remarks).
 *
 * @param bottomRadiusM - R, the ground sphere's radius, m.
 * @param interfaceHeightsM - The interfaces' heights above the ground, m, from the top down,
 *   strictly descending: L + 1 of them for L layers.
 * @param layerOpticalDepths - τ_p of each layer from the top down, finite and at least 0.
 * @param beamCosine - μ₀, in [−1, 1]: at or below 0, a sun below the point's horizontal.
 * @param chords - The beam's chords through the column ({@link sphericalBeamChords}), kept by a
 *   caller that traces it at many optical depths; found anew by default.
 * @throws RangeError for a radius not finite and above 0, heights not strictly descending to at
 *   least 0, optical depths of another count or not finite and at least 0, or μ₀ outside [−1, 1],
 *   or for chords of another column or μ₀.
 */
export function sphericalBeamPath(
  bottomRadiusM: number,
  interfaceHeightsM: Float64Array,
  layerOpticalDepths: ArrayLike<number>,
  beamCosine: number,
  chords?: BeamChords,
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
  if (chords === undefined) {
    checkHeights(interfaceHeightsM);
  } else if (
    chords.bottomRadiusM !== bottomRadiusM ||
    chords.interfaceHeightsM !== interfaceHeightsM ||
    chords.beamCosine !== beamCosine
  ) {
    throw new RangeError(`the chords are of another column or μ₀ than ${beamCosine}'s`);
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
  const geometry = chords ?? chordsOf(bottomRadiusM, interfaceHeightsM, beamCosine);
  if (!(beamCosine > 0)) {
    return beamBelowHorizon(geometry, height, depthOf, layers);
  }
  const { fromInterfacesM, emptySlant } = geometry;
  const slant = new Float64Array(layers + 1);
  for (let k = 1; k <= layers; k += 1) {
    let sum = 0;
    for (let q = 0; q < k; q += 1) {
      const tau = depthOf(q);
      if (tau === 0) {
        continue;
      }
      const chord = fromInterfacesM[k * layers + q] ?? Number.NaN;
      sum += (tau * chord) / (height(q) - height(q + 1));
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
    topTransmission[p] = exp(-accumulated);
    const tau = depthOf(p);
    let lambda: number;
    if (tau === 0) {
      lambda = emptySlant[p] ?? Number.NaN;
    } else {
      lambda = ((slant[p + 1] ?? Number.NaN) - accumulated) / tau;
      if (!(lambda >= SLANT_FACTOR_FLOOR)) {
        lambda = SLANT_FACTOR_FLOOR;
        flooredLayers += 1;
      }
    }
    slantFactor[p] = lambda;
    accumulated += lambda * tau;
    interfaceTransmission[p + 1] = exp(-accumulated);
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

/**
 * τ_s at each of `interfaces` (indices into `interfaceHeightsM`, from the top), each the value
 * {@link sphericalBeamPath}'s `slantOpticalDepth` holds there, to the bit, without the beam's
 * transmissions and slant factors: +∞ in the planet's shadow. For `bake.ts`'s curved streams
 * (R08.T14.d.4), which read the slant at the table's interfaces alone.
 *
 * @param interfaceHeightsM - As {@link sphericalBeamPath}'s, strictly descending (unchecked).
 * @param layerOpticalDepths - As {@link sphericalBeamPath}'s, finite and at least 0 (unchecked).
 * @throws RangeError for a radius not finite and above 0, or μ₀ outside [−1, 1].
 */
export function sphericalSlantDepths(
  bottomRadiusM: number,
  interfaceHeightsM: Float64Array,
  layerOpticalDepths: ArrayLike<number>,
  beamCosine: number,
  interfaces: ArrayLike<number>,
): Float64Array {
  if (!(Number.isFinite(bottomRadiusM) && bottomRadiusM > 0)) {
    throw new RangeError(`a column's sphere has a radius above 0, not ${bottomRadiusM} m`);
  }
  if (!(beamCosine >= -1 && beamCosine <= 1)) {
    throw new RangeError(`μ₀ lies in [−1, 1], not ${beamCosine}`);
  }
  const layers = interfaceHeightsM.length - 1;
  const height = (k: number): number => interfaceHeightsM[k] ?? Number.NaN;
  const depthOf = (p: number): number => layerOpticalDepths[p] ?? Number.NaN;
  const out = new Float64Array(interfaces.length);
  if (!(beamCosine > 0)) {
    const sines = (1 - beamCosine) * (1 + beamCosine);
    const radius2 = bottomRadiusM * bottomRadiusM;
    for (let i = 0; i < interfaces.length; i += 1) {
      const h = height(interfaces[i] ?? Number.NaN);
      out[i] =
        (bottomRadiusM + h) * (bottomRadiusM + h) * sines > radius2
          ? slantBelowHorizon(bottomRadiusM, height, depthOf, layers, h, beamCosine)
          : Number.POSITIVE_INFINITY;
    }
    return out;
  }
  for (let i = 0; i < interfaces.length; i += 1) {
    const k = interfaces[i] ?? Number.NaN;
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
    out[i] = sum;
  }
  return out;
}

/** {@link sphericalBeamPath} for μ₀ ≤ 0 (the module's remarks, "Below the horizontal"). */
function beamBelowHorizon(
  chords: BeamChords,
  height: (k: number) => number,
  depthOf: (p: number) => number,
  layers: number,
): SphericalBeam {
  const { bottomRadiusM, beamCosine, fromInterfacesM, emptySlant, shadowM } = chords;
  const lit = litBelowHorizon(bottomRadiusM, beamCosine);
  const slant = new Float64Array(layers + 1);
  for (let k = 0; k <= layers; k += 1) {
    slant[k] = lit(height(k))
      ? slantAlong(height, depthOf, layers, fromInterfacesM, k * layers)
      : Number.POSITIVE_INFINITY;
  }
  const topTransmission = new Float64Array(layers);
  const slantFactor = new Float64Array(layers).fill(1);
  const interfaceTransmission = new Float64Array(layers + 1);
  let accumulated = slant[0] ?? Number.NaN;
  interfaceTransmission[0] = exp(-accumulated);
  let flooredLayers = 0;
  for (let p = 0; p < layers && lit(height(p)); p += 1) {
    topTransmission[p] = exp(-accumulated);
    const tau = depthOf(p);
    const top = height(p);
    const bottom = height(p + 1);
    if (lit(bottom)) {
      let lambda: number;
      if (tau === 0) {
        lambda = emptySlant[p] ?? Number.NaN;
      } else {
        lambda = ((slant[p + 1] ?? Number.NaN) - accumulated) / tau;
        if (!(lambda >= SLANT_FACTOR_FLOOR)) {
          lambda = SLANT_FACTOR_FLOOR;
          flooredLayers += 1;
        }
      }
      slantFactor[p] = lambda;
      accumulated += lambda * tau;
      interfaceTransmission[p + 1] = exp(-accumulated);
      continue;
    }
    // The shadow's edge cuts the layer at h_s (the chords').
    const litShare = (top - shadowM) / (top - bottom);
    if (!(litShare > 0)) {
      // The edge rounds onto the layer's top: the layer is dark.
      topTransmission[p] = 0;
    } else if (tau === 0) {
      slantFactor[p] = emptySlant[p] ?? Number.NaN;
    } else {
      const litDepth = tau * litShare;
      const slantFromShadow = slantAlong(height, depthOf, layers, chords.fromShadowM, 0);
      const litLambda = Math.max((slantFromShadow - accumulated) / litDepth, SLANT_FACTOR_FLOOR);
      const share = -expm1(-litLambda * litDepth) / (litLambda * tau);
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
        exp(-(slant[k] ?? Number.NaN)) - (interfaceTransmission[k] ?? Number.NaN),
      );
    }
  }
  return largestDeficit;
}
