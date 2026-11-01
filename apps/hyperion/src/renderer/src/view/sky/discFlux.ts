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

import { cross, dot, normalise, type Vec3 } from "../../geometry/vec3";
import { gaussLegendre } from "../lighting/quadrature";
import type { Rgb } from "../photometry/toneCurve";
import { DISC_AIR_RULE, type DiscAir, type LineAir } from "../post/glareAir";

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
 * The disc's luminance above `levelCdM2`, per channel, cd/m², averaged over the disc: R07's
 * `GlareSource.excessLuminance` above the bloom threshold (fix-view-light).
 *
 * @remarks
 * ∫ max(0, I(μ) − level) over the disc's projected area, by the midpoint rule over equal rings in
 * r, the projected radius, with μ = √(1 − r²); 256 rings hold it to well under 1%.
 */
export function discLuminanceAboveRgb(host: HostDiscDto, levelCdM2: number): Rgb {
  const excess: [number, number, number] = [0, 0, 0];
  for (let ring = 0; ring < EXCESS_RINGS; ring += 1) {
    const r = (ring + 0.5) / EXCESS_RINGS;
    const weight = (2 * r) / EXCESS_RINGS;
    const [lr, lg, lb] = discLuminanceRgb(host, Math.sqrt(1 - r * r));
    excess[0] += Math.max(0, lr - levelCdM2) * weight;
    excess[1] += Math.max(0, lg - levelCdM2) * weight;
    excess[2] += Math.max(0, lb - levelCdM2) * weight;
  }
  return excess;
}

/**
 * {@link discLuminanceAboveRgb} of the disc as the air between dims it, `transmittance` per channel:
 * T ∫ max(0, I(μ) − level ÷ T), the light of the dimmed disc above `levelCdM2` (fix-view-light);
 * none in a channel the air takes whole.
 */
export function discLuminanceAboveThroughRgb(
  host: HostDiscDto,
  levelCdM2: number,
  transmittance: Rgb,
): Rgb {
  const [tr, tg, tb] = transmittance;
  if (tr === tg && tg === tb) {
    const [er, eg, eb] = tr > 0 ? discLuminanceAboveRgb(host, levelCdM2 / tr) : [0, 0, 0];
    return [tr * er, tr * eg, tr * eb];
  }
  const channel = (t: number, c: 0 | 1 | 2): number =>
    t > 0 ? t * discLuminanceAboveRgb(host, levelCdM2 / t)[c] : 0;
  return [channel(tr, 0), channel(tg, 1), channel(tb, 2)];
}

/** The Gauss–Legendre rule across each chord of a disc whose air changes across it: 8 nodes. */
const CHORD_RULE = gaussLegendre(8);

/** A host disc's light as the air between dims it, beside the share the frame's bodies leave. */
export interface DiscThroughAir {
  /**
   * The dimmed disc's light over the light of its part whose lines clear the ground, per channel:
   * the factor its mean luminance takes beside the share of the disc the frame's bodies leave the
   * camera (`eclipseVisible`), which counts the ground already.
   */
  readonly light: Rgb;
  /**
   * The dimmed disc's luminance above the level, averaged over the disc, cd/m², over the same part's
   * share of its light: R07's `GlareSource.excessLuminance` beside that share.
   */
  readonly excess: Rgb;
}

/**
 * The air's transmittance `air` gives at an offset along its vertical, in units of the disc's
 * angular radius, and whether that line clears the ground: linear between its nodes, the end
 * nodes' beyond them.
 */
function airAt(air: Extract<DiscAir, { kind: "across" }>, offset: number): LineAir {
  const { offsets, lines } = air;
  let below = -1;
  let above = -1;
  offsets.forEach((node, k) => {
    if (node <= offset && (below < 0 || node > (offsets[below] ?? -Infinity))) {
      below = k;
    }
    if (node >= offset && (above < 0 || node < (offsets[above] ?? Infinity))) {
      above = k;
    }
  });
  const low = lines[below];
  const high = lines[above];
  if (low === undefined || high === undefined || below === above) {
    return low ?? high ?? { transmittance: [1, 1, 1], clear: true };
  }
  const from = offsets[below] ?? 0;
  const to = offsets[above] ?? 1;
  const f = (offset - from) / (to - from);
  const mix = (c: 0 | 1 | 2): number => (1 - f) * low.transmittance[c] + f * high.transmittance[c];
  return { transmittance: [mix(0), mix(1), mix(2)], clear: f < 0.5 ? low.clear : high.clear };
}

/**
 * The disc's light through `airs`, each body's air between the camera and the star, whose centre
 * lies along `axis` (fix-view-light, the setting Sun's halo; science check, 2026-11-02).
 *
 * @remarks
 * Air the same across the disc multiplies its light, and its excess is
 * {@link discLuminanceAboveThroughRgb}'s. Air that changes across it is integrated over the part of
 * the disc whose lines clear the ground: by the first such air's nodes along its vertical, at
 * which its lines lie, and `CHORD_RULE`'s along each chord across it, each point's light the
 * power-2 law's and its transmittance every body's product, each other changing air's read along
 * its own vertical, a point whose line meets its ground left out. The light is over that part's,
 * which the frame's bodies' share of the disc already counts; the excess is averaged over the
 * whole disc, whose light and area `DISC_AIR_RULE` and `CHORD_RULE` give without the air, and is
 * over the same part's share of its light. The ground the air's lines meet is the datum, and the
 * share's the body's sphere: near a horizon the two differ by the figure's flattening.
 */
export function discLightThroughAir(
  host: HostDiscDto,
  airs: ReadonlyArray<DiscAir>,
  axis: Vec3,
  levelCdM2: number,
): DiscThroughAir {
  const uniform: [number, number, number] = [1, 1, 1];
  const across: Array<Extract<DiscAir, { kind: "across" }>> = [];
  for (const air of airs) {
    switch (air.kind) {
      case "uniform":
        uniform[0] *= air.transmittance[0];
        uniform[1] *= air.transmittance[1];
        uniform[2] *= air.transmittance[2];
        break;
      case "across":
        across.push(air);
        break;
    }
  }
  const first = across[0];
  if (first === undefined) {
    return { light: uniform, excess: discLuminanceAboveThroughRgb(host, levelCdM2, uniform) };
  }
  // The disc's axes in the sky: `first`'s vertical, and the horizontal across it.
  const horizontal = normalise(cross(axis, first.vertical));
  const reads = across.map((air) => ({
    air,
    alongX: dot(air.vertical, horizontal),
    alongY: dot(air.vertical, first.vertical),
  }));
  const clear: [number, number, number] = [0, 0, 0];
  const dimmed: [number, number, number] = [0, 0, 0];
  const excess: [number, number, number] = [0, 0, 0];
  first.offsets.forEach((y, j) => {
    const half = Math.sqrt(Math.max(0, 1 - y * y));
    CHORD_RULE.x.forEach((xi, i) => {
      const x = half * xi;
      const weight = (first.weights[j] ?? 0) * (CHORD_RULE.w[i] ?? 0) * half;
      const luminance = discLuminanceRgb(host, Math.sqrt(Math.max(0, 1 - x * x - y * y)));
      const transmittance: [number, number, number] = [uniform[0], uniform[1], uniform[2]];
      let clears = true;
      for (const { air, alongX, alongY } of reads) {
        const line = airAt(air, x * alongX + y * alongY);
        transmittance[0] *= line.transmittance[0];
        transmittance[1] *= line.transmittance[1];
        transmittance[2] *= line.transmittance[2];
        clears &&= line.clear;
      }
      if (!clears) {
        return;
      }
      for (const c of [0, 1, 2] as const) {
        clear[c] += weight * luminance[c];
        dimmed[c] += weight * luminance[c] * transmittance[c];
        excess[c] += weight * Math.max(0, luminance[c] * transmittance[c] - levelCdM2);
      }
    });
  });
  const { light: whole, area } = wholeDisc(host);
  const light = (c: 0 | 1 | 2): number => (clear[c] > 0 ? dimmed[c] / clear[c] : 0);
  const above = (c: 0 | 1 | 2): number =>
    clear[c] > 0 && area > 0 ? (excess[c] / area) * (whole[c] / clear[c]) : 0;
  return { light: [light(0), light(1), light(2)], excess: [above(0), above(1), above(2)] };
}

/** The whole disc's light and area by `DISC_AIR_RULE` and `CHORD_RULE`, in the disc's units. */
function wholeDisc(host: HostDiscDto): { readonly light: Rgb; readonly area: number } {
  const light: [number, number, number] = [0, 0, 0];
  let area = 0;
  DISC_AIR_RULE.x.forEach((y, j) => {
    const half = Math.sqrt(Math.max(0, 1 - y * y));
    CHORD_RULE.x.forEach((xi, i) => {
      const x = half * xi;
      const weight = (DISC_AIR_RULE.w[j] ?? 0) * (CHORD_RULE.w[i] ?? 0) * half;
      const luminance = discLuminanceRgb(host, Math.sqrt(Math.max(0, 1 - x * x - y * y)));
      area += weight;
      light[0] += weight * luminance[0];
      light[1] += weight * luminance[1];
      light[2] += weight * luminance[2];
    });
  });
  return { light, area };
}
