/**
 * A host star's disc as light: its angular radius, its limb-darkened luminance across it, its
 * illuminance, and the light a half-float target cannot hold (plan R06, Design note 16, T13.e).
 *
 * @remarks
 * The power-2 law I(μ) = I(1) (1 − c (1 − μ^α)) (Maxted 2018), per channel, with μ the cosine of the
 * angle from the disc's normal at the point seen: for a point at angle θ from the disc's centre on
 * a disc of angular radius ρ, μ = √(1 − (sin θ ÷ sin ρ)²). Flux conservation gives the disc's
 * illuminance as E = π L̄ sin² ρ exactly for a sphere, L̄ the disc's mean luminance (the wire's
 * `mean_luminance_cd_m2`), and I(1) is the wire's `central_luminance_cd_m2`. Every array is in
 * the wire's order B, V, R, for the display's blue, green and red; the functions here return
 * linear Rec. 709 red, green and blue.
 */

import type { HostDiscDto } from "@hyperion/protocol";

import type { Rgb } from "../photometry/toneCurve";
import { HALF_MAX } from "./half";

/** A disc's angular radius, rad: asin(R ÷ d). */
export function angularRadiusRad(radiusM: number, distanceM: number): number {
  return Math.asin(Math.min(1, radiusM / distanceM));
}

/** A wire array in B, V, R order as red, green, blue. */
export function rgbOfBvr(bvr: readonly [number, number, number]): Rgb {
  return [bvr[2], bvr[1], bvr[0]];
}

/**
 * The disc's luminance per channel at μ, cd/m²: the power-2 law on the centre's luminance.
 *
 * @param mu - The cosine at the point seen, in [0, 1].
 */
export function discLuminanceRgb(host: HostDiscDto, mu: number): Rgb {
  const central = rgbOfBvr(host.central_luminance_cd_m2);
  const limb = [host.limb[2], host.limb[1], host.limb[0]] as const;
  const law = (channel: 0 | 1 | 2): number =>
    central[channel] * (1 - limb[channel].c * (1 - mu ** limb[channel].alpha));
  return [law(0), law(1), law(2)];
}

/** The disc's illuminance per channel, lx: π L̄ sin² ρ. */
export function discIlluminanceRgbLx(host: HostDiscDto, radiusRad: number): Rgb {
  const mean = rgbOfBvr(host.mean_luminance_cd_m2);
  const area = Math.PI * Math.sin(radiusRad) ** 2;
  return [mean[0] * area, mean[1] * area, mean[2] * area];
}

/** Rings the disc's excess is integrated over, centre to limb. */
const EXCESS_RINGS = 256;

/**
 * The luminance a target holding at most 65,504 after pre-exposure cannot store, per channel,
 * cd/m², averaged over the disc: R07's `GlareSource.excessLuminance`.
 *
 * @param exposureScale - The frame's pre-exposure scale (R02's `exposureScale`).
 * @remarks
 * ∫ max(0, I(μ) − 65,504 ÷ scale) over the disc's projected area, by the midpoint rule over equal
 * rings in r, the projected radius, with μ = √(1 − r²); 256 rings hold it to well under 1%.
 */
export function discExcessLuminanceRgb(host: HostDiscDto, exposureScale: number): Rgb {
  const held = HALF_MAX / exposureScale;
  const excess: [number, number, number] = [0, 0, 0];
  for (let ring = 0; ring < EXCESS_RINGS; ring += 1) {
    const r = (ring + 0.5) / EXCESS_RINGS;
    const weight = (2 * r) / EXCESS_RINGS;
    const [lr, lg, lb] = discLuminanceRgb(host, Math.sqrt(1 - r * r));
    excess[0] += Math.max(0, lr - held) * weight;
    excess[1] += Math.max(0, lg - held) * weight;
    excess[2] += Math.max(0, lb - held) * weight;
  }
  return excess;
}
