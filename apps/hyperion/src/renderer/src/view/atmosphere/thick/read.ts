/**
 * The thick multiple-scattering table as the kernels read it (plan R08, R08.T14.c; Design note 9):
 * the bake's J_ms (`bake.ts`'s `MsSourceTable`, one a latitude band) packed for the GPU, and its
 * read, which the sky-view, aerial-perspective and march kernels make in place of σ_s·Ψ_ms·p_u
 * (`shaders/thickRead.wgsl`) and the CPU twin makes alike (`tablesCpu.ts`).
 *
 * @remarks
 * **What is stored.** The bake gives E = E⁰ + E¹ cos Δφ, the emission of orders two and up per unit
 * of true length, per steradian, per unit of the star's irradiance normal to its beam at the top
 * (m⁻¹ sr⁻¹). The table stores, per channel, at each of the bake's altitudes h, sun cosines μ₀ and
 * travel cosines μ_v:
 *
 * - J⁰ = E⁰ ÷ σ_s,k(h), and
 * - G = E¹ ÷ (σ_s,k(h) sin θ_v sin θ₀), sin θ = √(1 − μ²) at the cell's centre,
 *
 * σ_s,k(h) the kernels' own scattering at h, every term's forward delta out
 * (`tables.ts`'s `kernelMedium`), the factor each kernel multiplies its multiple scattering by. So
 * J is per steradian and per unit of scattering, as Hillaire's Ψ_ms is (sr⁻¹), and the read is
 * J = max(J⁰ + G·h_⊥, 0), h_⊥ = v·s − (v·n)(s·n) for the view's direction v, the source's s and the
 * zenith n: E¹ cos Δφ ÷ σ_s,k exactly at the cells' centres, since sin θ_v sin θ₀ cos Δφ is v's and
 * s's horizontal parts' dot product.
 * - Dividing by σ_s,k puts the values in the half float's range. E is about 10⁻⁷–10⁻⁶ m⁻¹ sr⁻¹ near
 *   an Earth-like ground and far less aloft, under `rgba16float`'s least normal (6.1 × 10⁻⁵), while
 *   J is of Ψ_ms's order, 10⁻³–10⁻¹ sr⁻¹ (plan R08's "Deferred findings", T14.b's _Half floats_).
 *   It is also what varies slowly with height: E follows the density's exponential between the
 *   table's rows, 1.3–24 km apart, and J does not.
 * - G in place of E¹ keeps the read continuous where the view or the sun is vertical. E¹ is an
 *   m = 1 Fourier coefficient, driven by the beam's Λ_l¹(μ₀) (the ground, Lambertian, acts at m = 0
 *   alone) and smooth at the poles, so it falls as sin θ_v sin θ₀, and E¹ ÷ (sin θ_v sin θ₀) is
 *   smooth. Read as E¹ cos Δφ
 *   with E¹ held at the last cell (μ_v = ±15/16) beyond it, the table would spin about the zenith,
 *   where Δφ has no value.
 * - Where E⁰ + E¹ cos Δφ dips below 0 (by up to 49% of E⁰ on the Titan-class fixture's far side of
 *   a forward-peaked point, T14.b's _m ≤ 1 dips below 0_), the read is held at 0 (T14.b's lean).
 *
 * **The texture** is `rgba32uint`, a 2D array of one layer a band and light slot, layer
 * b × slots + k (`multiScattering`'s order): 16 texels across, one a travel cosine, and
 * 33 × 32 + 1 rows, row a × 32 + s for altitude a and sun cosine s, then the info row. A texel holds
 * six halves, two to a word: J⁰'s red and green, J⁰'s blue and G's red, G's green and blue
 * (`pack2x16float`'s order, the first in the low half), and its fourth word is 0. A band and light
 * is 270,336 B, the bake's figure for `rgba16float` (33 × 32 × 16 texels × 2 modes × 8 B), and the
 * info row 256 B more. Layer 0's info row holds the altitudes as `f32` bits, words 0–32, and the
 * light slots a band, word 33.
 *
 * **The read** is linear in each of h, μ₀ and μ_v between the cells' centres about it, held at the
 * table's edges: μ₀ = (s + ½) ÷ 32 and μ_v = (v + ½) ÷ 8 − 1 lie at texel centres, and h between
 * the rows about it, found by bisection over the altitudes. Its band's s is read as the multiple
 * scattering's bands are (`oblate.wgsl`'s `bandTableAt`): linear in ln s between the bands about it,
 * the nearer band beyond them.
 *
 * **Below the horizon.** μ₀ ≤ 0 lies outside the table (T14.b's record, the beam's domain), so a
 * source there is read by the analytic term, and the two are blended over the table's first μ₀
 * cell, (0, 1/32]: the analytic term's share is 1 − clamp(32 μ₀, 0, 1)
 * (`decision-r08-t13c2-classes.md` §6). The forward lobes' higher orders (R08.T7, `forwardLobe.ts`)
 * take the same share (main, 2026-10-27): the bake's E holds the aureole scattered once more by the
 * smooth part, which the orders' tail also adds, so on the table's side they would count it twice.
 * The orders' in-lobe part, the aureole scattered again within the lobe (T14.b's IMS), is not in E,
 * so the table's side leaves it out: R08.T14.d judges it at Mars's aureole (plan R08's Risks,
 * "Preconditions of R08.T14.d's routing").
 */

import type { Rgb } from "../../photometry/toneCurve";
import { fromHalfBits, HALF_MAX, toHalfBits } from "../../sky/half";
import { type AtmosphereMedium, densityAt } from "../medium";
import type { MsSourceTable } from "./bake";

/** The table's altitudes: the bake's 33 (`bake.ts`'s `THICK_BAKE_GRID`, 64 layers at stride 2). */
export const THICK_ALTITUDES = 33;
/** The table's sun cosines: 32, at (s + ½) ÷ 32. */
export const THICK_SUN_COSINES = 32;
/** The table's travel cosines: 16, at (v + ½) ÷ 8 − 1, the downward half first. */
export const THICK_TRAVEL_COSINES = 16;
/** The table's azimuthal modes: m = 0 and 1 (`bake.ts`'s `THICK_BAKE_MODES`). */
export const THICK_MODES = 2;
/** The info row: after the altitudes' and sun cosines' rows. */
export const THICK_INFO_ROW = THICK_ALTITUDES * THICK_SUN_COSINES;
/** The texture's rows: the table's and the info row. */
export const THICK_ROWS = THICK_INFO_ROW + 1;
/** The word of the info row that holds the light slots a band. */
export const THICK_SLOTS_WORD = THICK_ALTITUDES;
/** The texture's format: four 32-bit words, six halves and a spare word. */
export const THICK_TABLE_FORMAT = "rgba32uint";
/** The bytes of one texel. */
const TEXEL_BYTES = 16;
/** The words of one texel. */
const TEXEL_WORDS = 4;

/**
 * A band and light slot's table as `rgba16float` would hold it, B: 270,336 (the bake's record; plan
 * R08, Design note 11's J_ms row).
 */
export const THICK_TABLE_BYTES_A_SLOT =
  THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * TEXEL_BYTES;

/** A layer's bytes as allocated, B: the table's and the info row's, 270,592. */
export const THICK_LAYER_BYTES = THICK_ROWS * THICK_TRAVEL_COSINES * TEXEL_BYTES;

/**
 * A body's thick table, packed: what the kernels' `thickTable` holds and the CPU twin reads
 * (the module's remarks). Structured-cloneable, for the smoke's twin workers.
 */
export interface PackedThickTable {
  /** Texels a row: {@link THICK_TRAVEL_COSINES}. */
  readonly widthTexels: number;
  /** Rows a layer: {@link THICK_ROWS}. */
  readonly heightTexels: number;
  /** Bands × slots. */
  readonly layers: number;
  /** The light slots a band. */
  readonly slots: number;
  /** The latitude bands, in the body's order. */
  readonly bands: number;
  /** The altitudes, h\* above the ground, m, ascending, as `f32` (the info row's). */
  readonly altitudesM: Float32Array;
  /** Four words a texel, row-major in each layer, layer after layer. */
  readonly texels: Uint32Array;
}

/** The bytes a packed table allocates, B: {@link THICK_LAYER_BYTES} a layer. */
export function thickTableBytes(table: Pick<PackedThickTable, "layers">): number {
  return table.layers * THICK_LAYER_BYTES;
}

/** σ_s,k per channel at `heightM`: the kernels' scattering, every term's own, by its density. */
function scatteringAt(medium: AtmosphereMedium, heightM: number): Rgb {
  let red = 0;
  let green = 0;
  let blue = 0;
  for (const term of medium.terms) {
    const rho = densityAt(term.density, heightM);
    red += term.scattering[0] * rho;
    green += term.scattering[1] * rho;
    blue += term.scattering[2] * rho;
  }
  return [red, green, blue];
}

/** A cosine grid's check: `n` values, each at (i + ½) ÷ `per` + `offset`, to 10⁻¹². */
function onGrid(values: Float64Array, n: number, per: number, offset: number): boolean {
  return (
    values.length === n &&
    values.every((value, i) => Math.abs(value - ((i + 0.5) / per + offset)) <= 1e-12)
  );
}

/** Each cosine's sine, √(1 − μ²). */
function sinesOf(cosines: Float64Array): Float64Array {
  return cosines.map((mu) => Math.sqrt(1 - mu * mu));
}

/**
 * A value's half-float bits, refused where no half holds it.
 *
 * @throws RangeError naming the value by `where` for one that is not finite or is past 65,504.
 */
function halfOf(value: number, where: () => string): number {
  if (!(Math.abs(value) <= HALF_MAX)) {
    throw new RangeError(`a thick table's ${where()} is ${value}, which no half float holds`);
  }
  return toHalfBits(value);
}

/**
 * A body's bake packed for the kernels (the module's remarks): J⁰ and G per band and light slot,
 * divided by the kernels' scattering at each altitude.
 *
 * @param bands - The bake's tables, one a latitude band in the body's order
 *   (`bake.ts`'s `thickBakeBands`), each with the same altitudes and light slots.
 * @param kernelMedium - The medium as the kernels read it (`tables.ts`'s `kernelMedium`, or the
 *   drawn medium with its terms' forward deltas taken out), whose scattering the kernels multiply
 *   the read by.
 * @throws RangeError for no band, a table off the bake's grid ({@link THICK_ALTITUDES} altitudes
 *   ascending from 0, {@link THICK_SUN_COSINES} μ₀ and {@link THICK_TRAVEL_COSINES} μ_v at their
 *   cells' centres, {@link THICK_MODES} modes), bands with different altitudes or light slots, no
 *   light slot, or a J⁰ or G that is not finite or past the half float's largest, 65,504.
 */
export function packThickTable(
  bands: ReadonlyArray<MsSourceTable>,
  kernelMedium: AtmosphereMedium,
): PackedThickTable {
  const first = bands[0];
  if (first === undefined) {
    throw new RangeError("a thick table has at least one band");
  }
  const slots = first.slots.length;
  if (slots === 0) {
    throw new RangeError("a thick table has at least one light slot");
  }
  const altitudes = first.altitudesM;
  if (
    altitudes.length !== THICK_ALTITUDES ||
    altitudes[0] !== 0 ||
    altitudes.some((h, a) => a > 0 && !(h > (altitudes[a - 1] ?? Number.POSITIVE_INFINITY)))
  ) {
    throw new RangeError(
      `a thick table's ${THICK_ALTITUDES} altitudes ascend from the ground, not ${Array.from(altitudes).join(", ")}`,
    );
  }
  for (const [b, band] of bands.entries()) {
    if (
      band.modes !== THICK_MODES ||
      !onGrid(band.sunCosines, THICK_SUN_COSINES, THICK_SUN_COSINES, 0) ||
      !onGrid(band.travelCosines, THICK_TRAVEL_COSINES, THICK_TRAVEL_COSINES / 2, -1)
    ) {
      throw new RangeError(`band ${b}'s table is off the thick bake's grid`);
    }
    if (band.slots.length !== slots || band.altitudesM.some((h, a) => h !== altitudes[a])) {
      throw new RangeError(`band ${b}'s table has other altitudes or light slots than band 0's`);
    }
  }
  const layers = bands.length * slots;
  const layerWords = THICK_ROWS * THICK_TRAVEL_COSINES * TEXEL_WORDS;
  const texels = new Uint32Array(layers * layerWords);
  const scattering = Array.from(altitudes, (h) => scatteringAt(kernelMedium, h));
  const sunSines = sinesOf(first.sunCosines);
  const travelSines = sinesOf(first.travelCosines);
  for (const [b, band] of bands.entries()) {
    for (const [k, slot] of band.slots.entries()) {
      const base = (b * slots + k) * layerWords;
      const values = slot.values;
      for (let a = 0; a < THICK_ALTITUDES; a += 1) {
        const sigma = scattering[a] ?? [Number.NaN, Number.NaN, Number.NaN];
        for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
          for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
            const at = ((a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * THICK_MODES * 3;
            const sines = (sunSines[s] ?? Number.NaN) * (travelSines[v] ?? Number.NaN);
            const halves = [0, 0, 0, 0, 0, 0];
            for (let c = 0; c < 3; c += 1) {
              const scatteringPerM = sigma[c] ?? Number.NaN;
              const perScattering = scatteringPerM > 0 ? 1 / scatteringPerM : 0;
              const e0 = values[at + c] ?? Number.NaN;
              const e1 = values[at + 3 + c] ?? Number.NaN;
              const where =
                (what: string): (() => string) =>
                () =>
                  `${what} at band ${b}, slot ${k}, altitude ${a}, μ₀ ${s}, μ_v ${v}, channel ${c}`;
              halves[c] = halfOf(e0 * perScattering, where("J⁰"));
              halves[3 + c] = halfOf((e1 * perScattering) / sines, where("G"));
            }
            const texel = base + ((a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * 4;
            texels[texel] = ((halves[0] ?? 0) | ((halves[1] ?? 0) << 16)) >>> 0;
            texels[texel + 1] = ((halves[2] ?? 0) | ((halves[3] ?? 0) << 16)) >>> 0;
            texels[texel + 2] = ((halves[4] ?? 0) | ((halves[5] ?? 0) << 16)) >>> 0;
          }
        }
      }
    }
  }
  const stored = Float32Array.from(altitudes);
  const words = new Uint32Array(stored.buffer.slice(0));
  const info = THICK_INFO_ROW * THICK_TRAVEL_COSINES * TEXEL_WORDS;
  texels.set(words, info);
  texels[info + THICK_SLOTS_WORD] = slots;
  return {
    widthTexels: THICK_TRAVEL_COSINES,
    heightTexels: THICK_ROWS,
    layers,
    slots,
    bands: bands.length,
    altitudesM: stored,
    texels,
  };
}

/** Where a height lies among the table's altitudes: the row below it and the share to the next. */
export interface ThickRow {
  readonly lower: number;
  readonly t: number;
}

/**
 * The rows about `heightM` (`thickRead.wgsl`'s `thickRowAt`): five halvings of 0 … 32, so that
 * `lower` ≤ 31, and t = (h − h_lower) ÷ (h_lower+1 − h_lower) held in [0, 1].
 */
export function thickRowAt(table: PackedThickTable, heightM: number): ThickRow {
  const altitudes = table.altitudesM;
  let lower = 0;
  let upper = THICK_ALTITUDES - 1;
  for (let i = 0; i < 5; i += 1) {
    const middle = (lower + upper) >>> 1;
    if ((altitudes[middle] ?? Number.NaN) <= heightM) {
      lower = middle;
    } else {
      upper = middle;
    }
  }
  const below = altitudes[lower] ?? Number.NaN;
  const above = altitudes[lower + 1] ?? Number.NaN;
  return { lower, t: Math.min(Math.max((heightM - below) / (above - below), 0), 1) };
}

/** The analytic term's share of a source's multiple scattering at sun cosine `muSun` (the module's remarks). */
export function thickAnalyticShare(muSun: number): number {
  return 1 - Math.min(Math.max(muSun * THICK_SUN_COSINES, 0), 1);
}

/** J⁰ and G at a point, per channel. */
export interface ThickSource {
  readonly level: [number, number, number];
  readonly slope: [number, number, number];
}

/** A zeroed {@link ThickSource}, for a reader's scratch. */
export function thickSourceScratch(): ThickSource {
  return { level: [0, 0, 0], slope: [0, 0, 0] };
}

/** Texel (a, s, v) of `layer`, its six halves added into `out` at weight `w`. */
function addTexel(
  table: PackedThickTable,
  layer: number,
  a: number,
  s: number,
  v: number,
  w: number,
  out: ThickSource,
): void {
  const at =
    ((layer * THICK_ROWS + a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * TEXEL_WORDS;
  const x = table.texels[at] ?? 0;
  const y = table.texels[at + 1] ?? 0;
  const z = table.texels[at + 2] ?? 0;
  out.level[0] += w * fromHalfBits(x & 0xffff);
  out.level[1] += w * fromHalfBits(x >>> 16);
  out.level[2] += w * fromHalfBits(y & 0xffff);
  out.slope[0] += w * fromHalfBits(y >>> 16);
  out.slope[1] += w * fromHalfBits(z & 0xffff);
  out.slope[2] += w * fromHalfBits(z >>> 16);
}

/**
 * One layer's J⁰ and G at `row`, sun cosine `muSun` and travel cosine `muTravel`, added into `out`
 * at weight `weight` (`thickRead.wgsl`'s `thickLayerAt`): linear in each between the cells' centres
 * about it, held at the edges.
 */
export function addThickLayer(
  table: PackedThickTable,
  layer: number,
  row: ThickRow,
  muSun: number,
  muTravel: number,
  weight: number,
  out: ThickSource,
): void {
  const x = Math.min(Math.max(muSun * THICK_SUN_COSINES - 0.5, 0), THICK_SUN_COSINES - 1);
  const s0 = Math.min(Math.floor(x), THICK_SUN_COSINES - 2);
  const fs = x - s0;
  const y = Math.min(
    Math.max((muTravel + 1) * (THICK_TRAVEL_COSINES / 2) - 0.5, 0),
    THICK_TRAVEL_COSINES - 1,
  );
  const v0 = Math.min(Math.floor(y), THICK_TRAVEL_COSINES - 2);
  const fv = y - v0;
  const a0 = row.lower;
  const ta = row.t;
  for (const [da, wa] of [
    [0, 1 - ta],
    [1, ta],
  ] as const) {
    for (const [ds, ws] of [
      [0, 1 - fs],
      [1, fs],
    ] as const) {
      for (const [dv, wv] of [
        [0, 1 - fv],
        [1, fv],
      ] as const) {
        addTexel(table, layer, a0 + da, s0 + ds, v0 + dv, weight * wa * ws * wv, out);
      }
    }
  }
}

/**
 * J⁰ and G at h\* `scaledHeightM` in light slot `slot`'s layers, read at the bands about s as
 * `thickRead.wgsl`'s `thickBandsAt` reads them: linear in ln s between the bands about s, the nearer
 * band beyond them, one band's alone.
 *
 * @param lnBands - ln s_b of the body's bands, ascending, one a band of the table.
 */
export function thickBandsInto(
  table: PackedThickTable,
  lnBands: ArrayLike<number>,
  scaledHeightM: number,
  s: number,
  muSun: number,
  muTravel: number,
  slot: number,
  out: ThickSource,
): void {
  out.level.fill(0);
  out.slope.fill(0);
  const slots = table.slots;
  const k = Math.min(slot, slots - 1);
  const row = thickRowAt(table, Math.max(scaledHeightM, 0));
  const last = table.bands - 1;
  const ls = Math.log(s);
  if (last === 0 || !(ls > (lnBands[0] ?? Number.NaN))) {
    addThickLayer(table, k, row, muSun, muTravel, 1, out);
    return;
  }
  if (!(ls < (lnBands[last] ?? Number.NaN))) {
    addThickLayer(table, last * slots + k, row, muSun, muTravel, 1, out);
    return;
  }
  let lower = 0;
  while (lower + 1 < last && (lnBands[lower + 1] ?? Number.NaN) <= ls) {
    lower += 1;
  }
  const l0 = lnBands[lower] ?? Number.NaN;
  const w = (ls - l0) / ((lnBands[lower + 1] ?? Number.NaN) - l0);
  addThickLayer(table, lower * slots + k, row, muSun, muTravel, 1 - w, out);
  addThickLayer(table, (lower + 1) * slots + k, row, muSun, muTravel, w, out);
}

/**
 * The table's J per channel for a source, sr⁻¹ per unit of its illuminance and of scattering, into
 * `out`: max(J⁰ + G·h_⊥, 0) (the module's remarks).
 *
 * @param horizontal - h_⊥ = v·s − (v·n)(s·n): the view's and the source's horizontal parts' dot product.
 */
export function thickJInto(
  source: ThickSource,
  horizontal: number,
  out: [number, number, number],
): void {
  for (let c = 0; c < 3; c += 1) {
    out[c] = Math.max((source.level[c] ?? 0) + (source.slope[c] ?? 0) * horizontal, 0);
  }
}
