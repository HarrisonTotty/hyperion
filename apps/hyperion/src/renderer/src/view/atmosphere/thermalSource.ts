/**
 * The medium's own light (plan R08, R08.T18.b; decision-composition §1.9): each term's thermal
 * source (1 − ω) B_λ(T(h)) beside its scattering, as the per-frame kernels' thermal forms march it
 * (`shaders/thermalSource.wgsl`, `hillaire.ts`' `withThermalSource`) and their CPU twin
 * (`tablesCpu.ts`) marches it alike; the thick bake's scattered thermal light J_th that they read
 * beside it; and the visibility cut that sets `AtmosphereRegime`'s `selfLuminous`.
 *
 * @remarks
 * - **The temperature** ({@link MediumTemperature}) is the column's at its levels' gravity-scaled
 *   heights h\* (`column.ts`), where the kernels read the medium: today's assembly holds it
 *   isothermal at the record's T_s, the seam until P14.T24.e's nodes.
 * - **The emission a step makes** is Kirchhoff's law in each render channel (lean, 2026-10-30): a
 *   step absorbs part of its optical depth, its terms' absorption (σ_ext − σ_s of the kernels' medium,
 *   the forward delta out of both) and its absorbers' curves' depth, and emits B̃_c(T(h\*)) times that
 *   depth through the step factor it scatters through, so that an opaque isothermal path emits
 *   B̃_c exactly, the limit an isothermal column's emergent radiance tends to as τ grows.
 *   B̃_c(T) = Σ_b V_cb B_λ(λ_b, T), Planck's law at the 15 bake bins reduced through T4.a's colour
 *   pipeline (`thermal.ts`' `thermalLuminance` of a black body), cd m⁻²: spectral in B, while the
 *   absorption that weights it is the channel's own, as the march's transmittance is. The kernels
 *   read B̃ at {@link THERMAL_SOURCE_HEIGHTS} heights evenly from 0 to the medium's top, linear
 *   between them, held at its ends, times the frame's pre-exposure scale (`view.marchSky.w`), for no
 *   star lights it.
 * - **The scattered thermal light** ({@link ThermalScattering}): the thick bake's thermal field's
 *   source of every order, J_th per unit of the kernels' scattering σ_s,k (`bake.ts`, spectral in
 *   the bins and reduced by V_cb), its ground's emission at the column's bottom among it, per band at
 *   the bake's altitudes and 16 travel cosines, read as σ_s,k J_th, the bands in ln s as the thick
 *   table's are. A thin body has none (lean, 2026-10-30): its air's own glow scattered again is of
 *   order τ_s τ_a B, but a glowing ground's scattered once is of order τ_s ε B_g, a sky over a lava
 * world's night side that stays dark (a finding).
 * - **The cut** ({@link selfLuminousOf}): the column's black body at its hottest, B̃_c(T_max), at
 *   emissivity 1 wherever the column absorbs at all, against 10⁻⁶ of the light its ground reflects of
 *   the record's stellar flux, E_c A_c ÷ π held to 10⁻⁵ cd m⁻² (`thermal.ts`' {@link THERMAL_CUT}
 *   and {@link REFLECTED_FLOOR_CD_M2}), the flux in lux by the Sun's luminous efficacy at the top of
 *   Earth's air, {@link STELLAR_EFFICACY_LM_PER_W}. The emitted side bounds the column's emission;
 *   the reflected side is an estimate, which overstates an M dwarf's lux by up to 10 (a black body's
 *   efficacy is 9 lm W⁻¹ at 2,600 K) and leaves out the air's and clouds' own reflection, so that the
 *   cut sits within a decade or so of 10⁻⁶ either way, far under what a frame shows.
 */

import type { Rgb } from "../photometry/toneCurve";
import { type AtmosphereMedium, columnLengthM } from "./medium";
import { E490_ILLUMINANCE_LX } from "./solar";
import { REFLECTED_FLOOR_CD_M2, THERMAL_CUT, thermalLuminance } from "./thermal";
import { log } from "../../lib/pinnedMath";

/** A mutable channel triple, an out parameter's. */
export type ThermalRgb = [number, number, number];

/** A column's temperature by height, plain data a structured clone carries. */
export interface MediumTemperature {
  /** The levels' gravity-scaled heights h\*, m, ascending from 0. */
  readonly heightsM: Float64Array;
  /** T at each level, K, positive. */
  readonly temperaturesK: Float64Array;
}

/** A column's temperature at `heightM`, K: linear between its levels, held at its ends. */
export function mediumTemperatureK(temperature: MediumTemperature, heightM: number): number {
  const { heightsM: h, temperaturesK: t } = temperature;
  const last = h.length - 1;
  if (!(heightM > (h[0] ?? Number.NaN))) {
    return t[0] ?? Number.NaN;
  }
  if (!(heightM < (h[last] ?? Number.NaN))) {
    return t[last] ?? Number.NaN;
  }
  let lo = 0;
  let hi = last;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if ((h[mid] ?? Number.NaN) <= heightM) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  const h0 = h[lo] ?? Number.NaN;
  const f = (heightM - h0) / ((h[hi] ?? Number.NaN) - h0);
  return (t[lo] ?? Number.NaN) * (1 - f) + (t[hi] ?? Number.NaN) * f;
}

/** A column's hottest level, K. */
export function mediumPeakK(temperature: MediumTemperature): number {
  return Math.max(...temperature.temperaturesK);
}

/** The heights the kernels read B̃ at, evenly from 0 to the medium's top. */
export const THERMAL_SOURCE_HEIGHTS = 64;

/** The travel cosines the scattered thermal light holds, the thick table's (v + ½) ÷ 8 − 1. */
export const THERMAL_TRAVEL_COSINES = 16;

/**
 * One band's scattered thermal light (the module's remarks): J_th per unit of the kernels'
 * scattering, cd m⁻² (a luminance per unit of σ_s,k per metre), at each altitude and travel cosine.
 */
export interface ThermalScattering {
  /** The bake's altitudes h\*, m, ascending. */
  readonly altitudesM: Float64Array;
  /** Altitude a's travel cosine v at `(a × 16 + v) × 3`, rgb. */
  readonly values: Float32Array;
}

/** A body's thermal source as the kernels read it. */
export interface ThermalSource {
  /** The medium's top, m: the B̃ table spans [0, top]. */
  readonly topHeightM: number;
  /** B̃_c at height i × top ÷ 63, cd m⁻², at `i × 3`. */
  readonly planck: Float32Array;
  /** Each band's scattered thermal light, the tables' plan's bands in order; `null` for none. */
  readonly scattering: ReadonlyArray<ThermalScattering> | null;
}

/**
 * A thick bake's scattered thermal light, one a band in its order, where every band holds it
 * (`bake.ts`' `MsSourceTable.thermal`); `null` where any does not, or for no bake.
 */
export function bandsScattering(
  bands: ReadonlyArray<{ readonly thermal?: ThermalScattering }> | null,
): ReadonlyArray<ThermalScattering> | null {
  if (bands === null) {
    return null;
  }
  const held: ThermalScattering[] = [];
  for (const band of bands) {
    if (band.thermal === undefined) {
      return null;
    }
    held.push(band.thermal);
  }
  return held;
}

/** A black body's luminance per render channel at `temperatureK`, cd m⁻²: B̃_c(T). */
export function blackBodyLuminance(temperatureK: number): Rgb {
  return thermalLuminance([0, 0, 0], 1, temperatureK);
}

/**
 * The thermal source of a column of `temperature` under a medium whose top is `topHeightM`, its
 * scattered light `scattering` (the thick bake's, or none).
 *
 * @throws RangeError for a top that is not positive and finite.
 */
export function thermalSourceOf(
  temperature: MediumTemperature,
  topHeightM: number,
  scattering: ReadonlyArray<ThermalScattering> | null = null,
): ThermalSource {
  if (!(topHeightM > 0) || !Number.isFinite(topHeightM)) {
    throw new RangeError(`a thermal source needs a positive top, not ${topHeightM} m`);
  }
  const planck = new Float32Array(THERMAL_SOURCE_HEIGHTS * 3);
  for (let i = 0; i < THERMAL_SOURCE_HEIGHTS; i += 1) {
    const heightM = (topHeightM * i) / (THERMAL_SOURCE_HEIGHTS - 1);
    planck.set(blackBodyLuminance(mediumTemperatureK(temperature, heightM)), i * 3);
  }
  return { topHeightM, planck, scattering };
}

/** A source's B̃ at h\* = `heightM`, cd m⁻², into `out`: `thermalSource.wgsl`'s thermalPlanckAt. */
export function thermalPlanckInto(source: ThermalSource, heightM: number, out: ThermalRgb): void {
  const last = THERMAL_SOURCE_HEIGHTS - 1;
  const place = Math.min(Math.max(heightM / source.topHeightM, 0), 1) * last;
  const lower = Math.min(Math.floor(place), last - 1);
  const f = place - lower;
  for (const c of [0, 1, 2] as const) {
    const a = source.planck[lower * 3 + c] ?? Number.NaN;
    const b = source.planck[(lower + 1) * 3 + c] ?? Number.NaN;
    out[c] = a + (b - a) * f;
  }
}

/** One band's J_th at `heightM` and the travel cosine `muTravel`, bilinear, held at the edges. */
function scatteringLayerInto(
  layer: ThermalScattering,
  heightM: number,
  muTravel: number,
  weight: number,
  out: ThermalRgb,
): void {
  const h = layer.altitudesM;
  const last = h.length - 1;
  let lo = 0;
  let f = 0;
  if (!(heightM > (h[0] ?? Number.NaN))) {
    lo = 0;
  } else if (!(heightM < (h[last] ?? Number.NaN))) {
    lo = last - 1;
    f = 1;
  } else {
    let hi = last;
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1;
      if ((h[mid] ?? Number.NaN) <= heightM) {
        lo = mid;
      } else {
        hi = mid;
      }
    }
    const h0 = h[lo] ?? Number.NaN;
    f = (heightM - h0) / ((h[lo + 1] ?? Number.NaN) - h0);
  }
  const place = Math.min(
    Math.max((muTravel + 1) * (THERMAL_TRAVEL_COSINES / 2) - 0.5, 0),
    THERMAL_TRAVEL_COSINES - 1,
  );
  const v0 = Math.min(Math.floor(place), THERMAL_TRAVEL_COSINES - 2);
  const g = place - v0;
  const at = (a: number, v: number, c: number): number =>
    layer.values[(a * THERMAL_TRAVEL_COSINES + v) * 3 + c] ?? Number.NaN;
  for (const c of [0, 1, 2] as const) {
    const low = at(lo, v0, c) * (1 - g) + at(lo, v0 + 1, c) * g;
    const high = at(lo + 1, v0, c) * (1 - g) + at(lo + 1, v0 + 1, c) * g;
    out[c] += weight * (low * (1 - f) + high * f);
  }
}

/**
 * A source's J_th at s and h\* = `heightM` for the travel cosine `muTravel`, cd m⁻² per unit of
 * σ_s,k, into `out`: 0 without its scattered light, its bands linear in ln s between the two about
 * s and the nearer beyond them (`thermalSource.wgsl`'s thermalScatteringAt, as `thick/read.ts`'s
 * thickBandsInto).
 *
 * @param lnBands - ln s_b of the tables' plan's bands, ascending.
 */
export function thermalScatteringInto(
  source: ThermalSource,
  lnBands: ArrayLike<number>,
  s: number,
  heightM: number,
  muTravel: number,
  out: ThermalRgb,
): void {
  out[0] = 0;
  out[1] = 0;
  out[2] = 0;
  const bands = source.scattering;
  if (bands === null || bands.length === 0) {
    return;
  }
  const last = bands.length - 1;
  const layerAt = (b: number): ThermalScattering => {
    const layer = bands[b];
    if (layer === undefined) {
      throw new Error(`no scattered thermal light for band ${b}`);
    }
    return layer;
  };
  if (last === 0) {
    scatteringLayerInto(layerAt(0), heightM, muTravel, 1, out);
    return;
  }
  const ls = log(s);
  if (!(ls > (lnBands[0] ?? Number.NaN))) {
    scatteringLayerInto(layerAt(0), heightM, muTravel, 1, out);
    return;
  }
  if (!(ls < (lnBands[last] ?? Number.NaN))) {
    scatteringLayerInto(layerAt(last), heightM, muTravel, 1, out);
    return;
  }
  let lower = 0;
  while (lower + 1 < last && (lnBands[lower + 1] ?? Number.NaN) <= ls) {
    lower += 1;
  }
  const l0 = lnBands[lower] ?? Number.NaN;
  const w = (ls - l0) / ((lnBands[lower + 1] ?? Number.NaN) - l0);
  scatteringLayerInto(layerAt(lower), heightM, muTravel, 1 - w, out);
  scatteringLayerInto(layerAt(lower + 1), heightM, muTravel, w, out);
}

/**
 * The buffer `thermalSource.wgsl` reads, `vec4f` by `vec4f`: the header (the top, m; the heights
 * B̃ is held at; the bands of scattered light, 0 for none; the altitudes a band); B̃ at each
 * height, rgb; then each band's altitudes in x, followed by its J_th, rgb, altitude by altitude and
 * travel cosine by travel cosine.
 *
 * @throws RangeError for bands of scattered light whose altitudes differ in count.
 */
export function packThermalSource(source: ThermalSource): Float32Array {
  const bands = source.scattering ?? [];
  const altitudes = bands[0]?.altitudesM.length ?? 0;
  if (bands.some((band) => band.altitudesM.length !== altitudes)) {
    throw new RangeError("every band of scattered thermal light needs the same altitudes");
  }
  const perBand = altitudes * (1 + THERMAL_TRAVEL_COSINES);
  const out = new Float32Array((1 + THERMAL_SOURCE_HEIGHTS + bands.length * perBand) * 4);
  out.set([source.topHeightM, THERMAL_SOURCE_HEIGHTS, bands.length, altitudes], 0);
  for (let i = 0; i < THERMAL_SOURCE_HEIGHTS; i += 1) {
    out.set(source.planck.subarray(i * 3, i * 3 + 3), (1 + i) * 4);
  }
  for (const [b, band] of bands.entries()) {
    const base = 1 + THERMAL_SOURCE_HEIGHTS + b * perBand;
    for (let a = 0; a < altitudes; a += 1) {
      out[(base + a) * 4] = band.altitudesM[a] ?? Number.NaN;
    }
    for (let i = 0; i < altitudes * THERMAL_TRAVEL_COSINES; i += 1) {
      out.set(band.values.subarray(i * 3, i * 3 + 3), (base + altitudes + i) * 4);
    }
  }
  return out;
}

/** ASTM E-490-00a's solar constant, the integral of its spectrum, W m⁻². */
const E490_SOLAR_CONSTANT_W_M2 = 1_366.1;

/**
 * The Sun's luminous efficacy at the top of Earth's air, lm W⁻¹: E-490's photopic illuminance
 * (`solar.ts`' {@link E490_ILLUMINANCE_LX}) over its solar constant, 97.6: the cut's conversion of
 * a record's stellar flux to lux, whatever the star (the module's remarks, "The cut").
 */
export const STELLAR_EFFICACY_LM_PER_W = E490_ILLUMINANCE_LX / E490_SOLAR_CONSTANT_W_M2;

/**
 * Whether a body's medium is self-luminous (the module's remarks, "The cut"): its column's black
 * body at its hottest, where its column absorbs in a channel, above {@link THERMAL_CUT} of its
 * ground's reflected light there.
 *
 * @param stellarFluxWM2 - The record's stellar flux at the body, W m⁻².
 * @param albedo - The albedo the reflected light is judged by: the medium's ground's by default; a
 *   gas envelope's own, whose medium has no ground (R08.T16.a).
 */
export function selfLuminousOf(
  medium: AtmosphereMedium,
  temperature: MediumTemperature,
  stellarFluxWM2: number,
  albedo: Rgb = medium.groundAlbedo,
): boolean {
  const peak = blackBodyLuminance(mediumPeakK(temperature));
  // An absorber's depth is each light's curves': it absorbs in every channel.
  const absorbing = medium.terms.some((term) => term.absorber !== undefined);
  const lux = Math.max(stellarFluxWM2, 0) * STELLAR_EFFICACY_LM_PER_W;
  return ([0, 1, 2] as const).some((c) => {
    // A slant path's emissivity is past the vertical's, so any absorption at all bounds it at 1.
    const absorbs =
      absorbing ||
      medium.terms.some(
        (term) => term.absorption[c] > 0 && columnLengthM(term.density, medium.topHeightM) > 0,
      );
    const reflected = Math.max((albedo[c] * lux) / Math.PI, REFLECTED_FLOOR_CD_M2);
    return absorbs && peak[c] > THERMAL_CUT * reflected;
  });
}
