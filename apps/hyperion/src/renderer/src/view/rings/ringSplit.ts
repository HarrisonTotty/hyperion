/**
 * A ring close to (plan R11, R11.T9.d; Design note 16): where its slab hands its largest particles
 * over to instances, the share of its optical depth they take from it, and how many of them a patch
 * of the ring plane holds.
 *
 * @remarks
 * The criterion is an angle: the ring is a slab while its largest particle subtends less than a
 * pixel. Nearer, a particle is instanced once its diameter 2s exceeds the pixel's footprint p = d θ,
 * so the threshold radius is s_pix = p ÷ 2, and for the s⁻³ distribution of radii, whose optical
 * depth per ln s is flat, the instanced share of the optical depth is ln(s_max ÷ s_pix) ÷ ln(s_max ÷
 * s_min), clamped to [0, 1]: over plan 14's radii of 1 cm–5 m, 0 at the hand-over (p = 10 m, 18.3 km
 * at 1080p across 60°), 0.371 at p = 1 m and 1 once p is below 2 cm. That share is removed from the
 * slab (`ringSlab.wgsl`), so that the light let through slab and instances together is the unsplit
 * slab's. Pure, `f64`, no state.
 */
import type { RingShape } from "./ringProfile";

/**
 * A massive ring's particle radii, m: 1 cm to 5 m, n(s) ∝ s⁻³ between them, plan 14's
 * `RING_PARTICLE_RADII` (`crates/hyperion-sim/src/planetary/rings.rs`; Zebker, Marouf and Tyler
 * 1985, Icarus 64, 531, power laws of index 2.7–3.0 between about 1 cm and 5 m).
 */
export const RING_PARTICLE_RADII_M = [0.01, 5] as const;

/** A view's mean pixel angle, rad: its horizontal field over its width in pixels. */
export function pixelAngleRad(fovXRad: number, widthPx: number): number {
  return fovXRad / widthPx;
}

/**
 * The distance from a ring's particles at which its largest subtends a pixel of `pixelRad`, m: 2
 * s_max ÷ θ, 1,833 × its diameter at 1080p across 60°; 0 for a dusty ring, whose grains a slab
 * draws at any distance.
 */
export function handOverDistance(ring: Pick<RingShape, "ringKind">, pixelRad: number): number {
  let distanceM: number;
  switch (ring.ringKind) {
    case "massive":
      distanceM = (2 * RING_PARTICLE_RADII_M[1]) / pixelRad;
      break;
    case "dusty":
      distanceM = 0;
      break;
  }
  return distanceM;
}

/**
 * The share of a ring's optical depth instanced where the pixel's footprint is
 * `pixelFootprintM`: ln(s_max ÷ s_pix) ÷ ln(s_max ÷ s_min) with s_pix half the footprint, clamped to
 * [0, 1]: 1 for a footprint of 0, where every particle fills a pixel.
 *
 * @param radiusMinM - s_min, the least particle's radius, m, positive.
 * @param radiusMaxM - s_max, the largest's, m, above s_min.
 */
export function instancedShare(
  pixelFootprintM: number,
  radiusMinM: number,
  radiusMaxM: number,
): number {
  const threshold = pixelFootprintM / 2;
  if (!(threshold > 0)) {
    return 1;
  }
  const share = Math.log(radiusMaxM / threshold) / Math.log(radiusMaxM / radiusMinM);
  return Math.min(Math.max(share, 0), 1);
}

/** The least instanced radius where the footprint is `pixelFootprintM`: s_pix within [s_min, s_max]. */
export function instancedRadiusMinM(
  pixelFootprintM: number,
  radiusMinM: number,
  radiusMaxM: number,
): number {
  return Math.min(Math.max(pixelFootprintM / 2, radiusMinM), radiusMaxM);
}

/**
 * The instanced particles a square metre of the ring plane holds where its normal optical depth is
 * `tau` and the footprint `pixelFootprintM`: for n(s) = C s⁻³ between s_min and s_max, τ = π C ln(s_max
 * ÷ s_min), and those from s_lo = s_pix up number C ÷ 2 × (s_lo⁻² − s_max⁻²).
 */
export function instancedPerSquareMetre(
  tau: number,
  pixelFootprintM: number,
  radiusMinM: number,
  radiusMaxM: number,
): number {
  const c = Math.max(tau, 0) / (Math.PI * Math.log(radiusMaxM / radiusMinM));
  const low = instancedRadiusMinM(pixelFootprintM, radiusMinM, radiusMaxM);
  return (c / 2) * (low ** -2 - radiusMaxM ** -2);
}

/**
 * The radius, m, at quantile `u` ∈ [0, 1) of the s⁻³ distribution between `radiusLowM` and
 * `radiusMaxM`: its inverse CDF, (s_lo⁻² − u (s_lo⁻² − s_max⁻²))^(−1/2).
 */
export function instancedRadiusAt(u: number, radiusLowM: number, radiusMaxM: number): number {
  const low = radiusLowM ** -2;
  return (low - u * (low - radiusMaxM ** -2)) ** -0.5;
}
