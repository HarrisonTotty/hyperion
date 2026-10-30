/**
 * The disc's reflectance table (plan R08, R08.T16.b; Design notes 13 and 17): its grid and its read,
 * shared by the optics worker's bake (`thick/discBake.ts`), the render thread's upload and the CPU
 * twin of R07's disc (`view/bodies/discShading.ts`), whose kernels read it as `discAir.wgsl` does.
 *
 * @remarks
 * **What it holds.** For each latitude band of a body's tables and each light slot, the air's own
 * light from outside the air at a point of the datum: the radiance the full passes' march gathers
 * along a view ray from the top down to the ground, per unit of the star's irradiance normal to its
 * beam at the top, held as π L ÷ E (the air's I/F, dimensionless) in rgb, and the march's mean
 * transmittance over the channels along the same ray in alpha, which the composite dims a drawn
 * surface by (`composite.wgsl`'s `colour * marched.a`). The ground's own light is not in it: the
 * disc shades its ground as before, its surface lighting through the air (R08.T9.b), and takes it
 * times alpha, then adds each light's air (`discAir.wgsl`), as the composite lays the march over the
 * mesh inside the boundary. So the disc's class map, normal map and eclipses stay its own, and only
 * the air beyond the datum's limb is not drawn.
 *
 * **The grid**, by the point's view cosine μ_v (towards the camera), the star's cosine μ₀ and the
 * azimuth Δφ between their horizontal parts (0 with the camera on the star's side), each node a
 * texel, read linearly between them in each mapped coordinate:
 * - {@link DISC_VIEW_COSINES} view cosines, μ_v = (v ÷ 15)², dense at the limb, where the air's
 *   light rises steeply;
 * - {@link DISC_SUN_COSINES} sun cosines: {@link DISC_SUN_ROWS_BELOW} below the horizontal from the
 *   table's floor μ_f (the deepest sun under which the top of any band's air is lit,
 *   −√(1 − (R_b ÷ (R_b + H))²)), μ₀ = μ_f (1 − s ÷ 8)², and {@link DISC_SUN_ROWS_ABOVE} at
 *   μ₀ = (k ÷ 23)², both dense at the terminator, where the twilight's light falls steeply;
 * - {@link DISC_AZIMUTHS} azimuths, Δφ = π k ÷ 12, the table symmetric in Δφ.
 *
 * **The texture** is `rgba16float`, a 2D array of one layer a band and light slot, layer
 * b × slots + k as the multiple scattering's (`oblate.wgsl`'s `bandTableAt`): x = Δφ's node × 16 +
 * μ_v's, y the sun's row. Its values are stored rounded to the half floats it holds
 * ({@link discTableValuesOf}), so that the twin reads what the kernels read.
 */

import type { Rgb } from "../photometry/toneCurve";
import { fromHalfBits, toHalfBits } from "../sky/half";
import { atan2, log } from "../../lib/pinnedMath";

/** The view cosines: μ_v = (v ÷ 15)², v = 0 … 15. */
export const DISC_VIEW_COSINES = 16;
/** The sun's rows below the horizontal: μ₀ = μ_f (1 − s ÷ 8)², s = 0 … 7, from the floor. */
export const DISC_SUN_ROWS_BELOW = 8;
/** The sun's rows from the horizontal up: μ₀ = (k ÷ 23)², k = 0 … 23. */
export const DISC_SUN_ROWS_ABOVE = 24;
/** The sun's rows in all. */
export const DISC_SUN_COSINES = DISC_SUN_ROWS_BELOW + DISC_SUN_ROWS_ABOVE;
/** The azimuths: Δφ = π k ÷ 12, k = 0 … 12. */
export const DISC_AZIMUTHS = 13;
/** Texels a row: one a view cosine and azimuth. */
export const DISC_TABLE_WIDTH = DISC_VIEW_COSINES * DISC_AZIMUTHS;
/** Rows a layer: one a sun cosine. */
export const DISC_TABLE_HEIGHT = DISC_SUN_COSINES;
/** `f32` values a layer: four a texel. */
export const DISC_LAYER_VALUES = DISC_TABLE_WIDTH * DISC_TABLE_HEIGHT * 4;
/** The texture's format. */
export const DISC_TABLE_FORMAT = "rgba16float";
/** Bytes a layer as the texture holds it: 8 a texel. */
export const DISC_LAYER_BYTES = DISC_TABLE_WIDTH * DISC_TABLE_HEIGHT * 8;

/** A body's disc reflectance table (the module's remarks), one layer a band and light slot. */
export interface DiscReflectanceTable {
  /** The latitude bands, in the body's tables' order. */
  readonly bands: number;
  /** The light slots a band. */
  readonly slots: number;
  /** The sun's floor μ_f, below 0: the lowest row. */
  readonly sunFloor: number;
  /**
   * Four values a texel, row-major in each layer, layer after layer: the air's π L ÷ E in r, g and
   * b, and the march's mean transmittance in a, each a half float's value; then, with
   * {@link DiscReflectanceTable.thermalScale}, a thermal layer a band.
   */
  readonly values: Float32Array;
  /**
   * The scale of the air's own light (R08.T18.b), cd m⁻² times π: a self-luminous medium's table
   * holds, past its bands' light slots, one layer a band of π L_th ÷ this in r, g and b, L_th the
   * air's thermal emission and its scattering seen at each view cosine, the same at every sun row
   * and azimuth; absent for a medium that is not self-luminous.
   */
  readonly thermalScale?: number;
}

/** A table's layers: a band's light slots, and its thermal layer where it holds one (R08.T18.b). */
export function discTableLayers(
  table: Pick<DiscReflectanceTable, "bands" | "slots" | "thermalScale">,
): number {
  return table.bands * (table.slots + (table.thermalScale === undefined ? 0 : 1));
}

/** The bytes a table's texture allocates, B: {@link DISC_LAYER_BYTES} a layer. */
export function discTableBytes(
  table: Pick<DiscReflectanceTable, "bands" | "slots" | "thermalScale">,
): number {
  return discTableLayers(table) * DISC_LAYER_BYTES;
}

/** View cosine node v's μ_v. */
export function discViewCosineAt(v: number): number {
  return (v / (DISC_VIEW_COSINES - 1)) ** 2;
}

/** Sun row s's μ₀ under the floor `sunFloor`. */
export function discSunCosineAt(s: number, sunFloor: number): number {
  if (s < DISC_SUN_ROWS_BELOW) {
    return sunFloor * (1 - s / DISC_SUN_ROWS_BELOW) ** 2;
  }
  return ((s - DISC_SUN_ROWS_BELOW) / (DISC_SUN_ROWS_ABOVE - 1)) ** 2;
}

/** Azimuth node k's Δφ, rad. */
export function discAzimuthAt(k: number): number {
  return (Math.PI * k) / (DISC_AZIMUTHS - 1);
}

/** A view cosine's place among the nodes, in [0, 15]. */
export function discViewPlace(muView: number): number {
  return (DISC_VIEW_COSINES - 1) * Math.sqrt(Math.min(Math.max(muView, 0), 1));
}

/** A sun cosine's place among the rows under the floor `sunFloor`, in [0, 31]. */
export function discSunPlace(muSun: number, sunFloor: number): number {
  if (muSun < 0) {
    return DISC_SUN_ROWS_BELOW * Math.max(1 - Math.sqrt(Math.min(muSun / sunFloor, 1)), 0);
  }
  return DISC_SUN_ROWS_BELOW + (DISC_SUN_ROWS_ABOVE - 1) * Math.sqrt(Math.min(muSun, 1));
}

/** An azimuth's place among the nodes, in [0, 12]. */
export function discAzimuthPlace(azimuthRad: number): number {
  return ((DISC_AZIMUTHS - 1) * Math.min(Math.max(azimuthRad, 0), Math.PI)) / Math.PI;
}

/** Where a texel's four values start in {@link DiscReflectanceTable.values}. */
export function discTexelIndex(layer: number, s: number, v: number, k: number): number {
  return ((layer * DISC_TABLE_HEIGHT + s) * DISC_TABLE_WIDTH + k * DISC_VIEW_COSINES + v) * 4;
}

/** `values` with each rounded to the half float the texture holds of it. */
export function discTableValuesOf(values: Float32Array): Float32Array {
  return Float32Array.from(values, (value) => fromHalfBits(toHalfBits(value)));
}

/** The table's texels as the texture's upload takes them: each value's half bits, in order. */
export function discTableHalves(table: DiscReflectanceTable): Uint16Array {
  return Uint16Array.from(table.values, (value) => toHalfBits(value));
}

/** The node below a place on an axis of `n` nodes and the share to the next, held at the ends. */
function cellOf(place: number, n: number): readonly [number, number] {
  const lower = Math.min(Math.max(Math.floor(place), 0), n - 2);
  return [lower, Math.min(Math.max(place - lower, 0), 1)];
}

/**
 * One layer's texel, linear in each of the three mapped coordinates between the nodes about it,
 * added into `out` at `weight` (`discAir.wgsl`'s `disc_air_layer`).
 */
function addLayer(
  table: DiscReflectanceTable,
  layer: number,
  place: readonly [number, number, number],
  weight: number,
  out: [number, number, number, number],
): void {
  const [s0, fs] = cellOf(place[0], DISC_SUN_COSINES);
  const [v0, fv] = cellOf(place[1], DISC_VIEW_COSINES);
  const [k0, fk] = cellOf(place[2], DISC_AZIMUTHS);
  for (const [ds, ws] of [
    [0, 1 - fs],
    [1, fs],
  ] as const) {
    for (const [dk, wk] of [
      [0, 1 - fk],
      [1, fk],
    ] as const) {
      for (const [dv, wv] of [
        [0, 1 - fv],
        [1, fv],
      ] as const) {
        const w = weight * ws * wk * wv;
        const at = discTexelIndex(layer, s0 + ds, v0 + dv, k0 + dk);
        for (let c = 0; c < 4; c += 1) {
          out[c] = (out[c] ?? 0) + w * (table.values[at + c] ?? Number.NaN);
        }
      }
    }
  }
}

/**
 * The table at a point, light slot `slot`'s (held to the table's), its bands read as the multiple
 * scattering's are (`oblate.wgsl`'s `bandTableAt`): linear in ln s between the two bands about the
 * point's s, the nearer band beyond them, one band's alone (`discAir.wgsl`'s `disc_air_at`). The
 * air's π L ÷ E per channel and the march's mean transmittance.
 *
 * @param lnBands - ln s_b of the body's bands, ascending, one a band of the table.
 * @param s - s = g(φ) ÷ g_ref at the point's geodetic latitude.
 * @param azimuthRad - Δφ, in [0, π].
 */
export function discTableAt(
  table: DiscReflectanceTable,
  lnBands: ArrayLike<number>,
  s: number,
  slot: number,
  muView: number,
  muSun: number,
  azimuthRad: number,
): readonly [number, number, number, number] {
  const out: [number, number, number, number] = [0, 0, 0, 0];
  const k = Math.min(slot, table.slots - 1);
  const place = [
    discSunPlace(muSun, table.sunFloor),
    discViewPlace(muView),
    discAzimuthPlace(azimuthRad),
  ] as const;
  const last = table.bands - 1;
  const ls = log(s);
  if (last === 0 || !(ls > (lnBands[0] ?? Number.NaN))) {
    addLayer(table, k, place, 1, out);
    return out;
  }
  if (!(ls < (lnBands[last] ?? Number.NaN))) {
    addLayer(table, last * table.slots + k, place, 1, out);
    return out;
  }
  let lower = 0;
  while (lower + 1 < last && (lnBands[lower + 1] ?? Number.NaN) <= ls) {
    lower += 1;
  }
  const l0 = lnBands[lower] ?? Number.NaN;
  const w = (ls - l0) / ((lnBands[lower + 1] ?? Number.NaN) - l0);
  addLayer(table, lower * table.slots + k, place, 1 - w, out);
  addLayer(table, (lower + 1) * table.slots + k, place, w, out);
  return out;
}

/**
 * The air's own light at s and the view cosine `muView` (R08.T18.b), cd m⁻² times π, rgb: its
 * thermal layers at the top sun row and azimuth 0, read as {@link discTableAt} reads the bands, times
 * the table's scale; 0 for a table without them (`discAir.wgsl`'s `disc_air_thermal_at`).
 */
export function discThermalAt(
  table: DiscReflectanceTable,
  lnBands: ArrayLike<number>,
  s: number,
  muView: number,
): readonly [number, number, number] {
  const scale = table.thermalScale;
  if (scale === undefined) {
    return [0, 0, 0];
  }
  const out: [number, number, number, number] = [0, 0, 0, 0];
  const base = table.bands * table.slots;
  const place = [DISC_SUN_COSINES - 1, discViewPlace(muView), 0] as const;
  const last = table.bands - 1;
  const ls = log(s);
  if (last === 0 || !(ls > (lnBands[0] ?? Number.NaN))) {
    addLayer(table, base, place, 1, out);
  } else if (!(ls < (lnBands[last] ?? Number.NaN))) {
    addLayer(table, base + last, place, 1, out);
  } else {
    let lower = 0;
    while (lower + 1 < last && (lnBands[lower + 1] ?? Number.NaN) <= ls) {
      lower += 1;
    }
    const l0 = lnBands[lower] ?? Number.NaN;
    const w = (ls - l0) / ((lnBands[lower + 1] ?? Number.NaN) - l0);
    addLayer(table, base + lower, place, 1 - w, out);
    addLayer(table, base + lower + 1, place, w, out);
  }
  return [out[0] * scale, out[1] * scale, out[2] * scale];
}

/**
 * Δφ between the horizontal parts of `towardsCamera` and `towardsLight` about the unit `normal`,
 * rad in [0, π]; 0 where either is vertical, its azimuth having no value (`discAir.wgsl`'s
 * `disc_air_azimuth`).
 */
export function discAzimuthOf(
  normal: { readonly x: number; readonly y: number; readonly z: number },
  towardsCamera: { readonly x: number; readonly y: number; readonly z: number },
  towardsLight: { readonly x: number; readonly y: number; readonly z: number },
): number {
  const muView =
    normal.x * towardsCamera.x + normal.y * towardsCamera.y + normal.z * towardsCamera.z;
  const muSun = normal.x * towardsLight.x + normal.y * towardsLight.y + normal.z * towardsLight.z;
  const hv = [
    towardsCamera.x - muView * normal.x,
    towardsCamera.y - muView * normal.y,
    towardsCamera.z - muView * normal.z,
  ] as const;
  const hs = [
    towardsLight.x - muSun * normal.x,
    towardsLight.y - muSun * normal.y,
    towardsLight.z - muSun * normal.z,
  ] as const;
  const along = hv[0] * hs[0] + hv[1] * hs[1] + hv[2] * hs[2];
  const across = Math.hypot(
    hv[1] * hs[2] - hv[2] * hs[1],
    hv[2] * hs[0] - hv[0] * hs[2],
    hv[0] * hs[1] - hv[1] * hs[0],
  );
  if (!(across > 0) && !(Math.abs(along) > 0)) {
    return 0;
  }
  return atan2(across, along);
}

/** The table's π L ÷ E per channel at a point, without its alpha. */
export function discTableAir(texel: readonly [number, number, number, number]): Rgb {
  return [texel[0], texel[1], texel[2]];
}
