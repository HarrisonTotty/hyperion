/**
 * The cloud deck as the kernels read it (plan R08, R08.T15.a.2; Design notes 9 and 17): the upper
 * medium as the kernels' medium, its ground the split, and the deck's own tables (`deck.ts`'s
 * `DeckTables`) packed for the GPU, with their read, which the ray march makes at a ray's hit on the
 * deck in place of the bare ground's Lambertian term (`shaders/deckRead.wgsl`) and the CPU twin
 * makes alike (`tablesCpu.ts`).
 *
 * @remarks
 * **The kernels' medium and ground.** Over a deck the kernels draw the upper medium alone, its
 * ground the split h\* = `deckAltitudeM` ({@link upperMediumOf}: each term's density from the split
 * up, the top lowered by it), and the bake's J above the deck (`DeckTables.above`) and the
 * correction over it with their altitudes from the split ({@link deckAboveTables},
 * {@link deckCorrectionAbove}), so that T14.c's table and its read serve unchanged. Their datum is
 * the deck's level surface h\* = `deckAltitudeM`, at geometric height h\* ÷ s(φ) above the body's
 * datum (Design note 17): {@link deckDatumOf} takes it as the spheroid through it at the equator and
 * the poles, (a + h\* ÷ s_e, c + h\* ÷ s_p), the form the kernels' top and layer-edge shells take
 * (R08.T6.f), so that every kernel's ground test, its slicing and its heights take that surface
 * with no line of theirs changed. Between the equator and the poles the spheroid departs from the
 * level surface by {@link deckDatumDepartureM}: on a Saturn-class figure by at most 1.7% of h\* ÷ s,
 * at 46.5° and whatever the split, 0.012 of the local scale height for a split at 0.5 bar: the
 * column from the deck's top up is whole, but the top lies 0.012 H ÷ s off its level surface there,
 * so that the medium at a given height above it reads some 1.2% dense (`deckRead.test.ts`), a
 * stated approximation over Design note 17's 1% of geometry (pending the owner's ruling).
 *
 * **What the table holds** ({@link packDeckTable}), per band and light slot, at the deck's sun
 * cosines (the thick table's 46 rows) and its eight upward view cosines, per channel, each mode of
 * the deck's three series stored over sinᵐ θ_v sinᵐ θ₀ at the cell's centre, as `read.ts` stores G
 * and G₂, so that the read stays continuous where the view or the sun is vertical:
 *
 * - ρᵐ − ρ_ssᵐ, the multiple-scattering part of the deck's reflectance, m = 0 … 7, for a beam the
 *   bake did not carry;
 * - βρᵐ, the same weighted term by term by the bake's own beam at the deck, for the beam it did;
 * - Dᵐ, the deck's sky-lit radiance, at its modes (the bake's 3, or 8 under the test's option), 0
 *   beyond them;
 * - G and G_β, the exact single scattering's geometry factor and its beam's;
 * - and per channel the deck's phase p(Θ), sr⁻¹, on the kernels' phase tables' u = √(θ ÷ π) grid.
 *
 * **The read** ({@link deckGroundInto}), per source of the view at a ray's hit on the deck, per unit
 * of the source's weight: (max(μ₀, 0) ÷ π)(Σ_m βρᵐ cᵐ + G_β p(Θ)) + w(μ₀) Σ_m Dᵐ cᵐ, held at 0, with
 * cᵐ = sinᵐ θ_v sinᵐ θ₀ cos mΔφ formed from the horizontal parts of the view and the source without
 * Δφ (c⁰ = 1, c¹ = h_⊥, cᵐ⁺¹ = 2 h_⊥ cᵐ − (1 − μ_v²)(1 − μ₀²) cᵐ⁻¹, Chebyshev's recurrence; h_⊥ as
 * `read.ts`'s), cos Θ = s·v between the source's and the view's directions, w(μ₀) = clamp(32 μ₀ + 7,
 * 0, 1) the thick table's share, so that the sky-lit radiance fades with J at the rows' edge, and
 * the beam's term at μ₀ > 0 alone, whose rows hold 0 below the horizontal. A source the bake did not
 * carry (a light slot past the table's) reads ρᵐ − ρ_ssᵐ and G times the kernels' own transmittance
 * to the deck in place of βρᵐ and G_β, and the nearest slot's sky-lit radiance, as J's read takes the
 * nearest slot. The beam's term takes no other visibility of the source: the bake's beam is already
 * in βρ and G_β, and an eclipse or a ring's shadow is in the source's weight.
 *
 * The read is linear between the cells' centres in μ₀ (the thick table's rows, `read.ts`'s
 * `thickSunPlace`, the beam's terms held at the first row above the horizontal below it), in μ_v
 * (μ_v = (v + ½) ÷ 8, held at the edges), in u for the phase, and in ln s between the bands about
 * the hit's s, as `thickRead.wgsl`'s `thickBandsAt` reads them.
 *
 * **The texture** is `rgba32uint`, a 2D array of one layer a band and light slot (layer
 * b × slots + k), each texel's first three words three `f32` values' bits (red, green, blue), the
 * fourth 0: {@link DECK_TABLE_WIDTH} texels across, eight cells of {@link DECK_TEXELS_A_CELL} a row,
 * cell v of row s at texels v × 26 … v × 26 + 25 (the eight ρᵐ − ρ_ssᵐ, the eight βρᵐ, the eight Dᵐ,
 * G and G_β), and the phase's 256 entries after the 46 rows, entry i at texel (i mod 208,
 * 46 + ⌊i ÷ 208⌋): 208 × 48 texels, 159,744 B a band and light. The values are `f32`, as the
 * texture's words hold them: the deck's table is read once a ray, not a step, so the half floats of
 * the thick table, and their least normal, do not arise.
 *
 * **Below the deck** (R08.T15.b.2; {@link packBelowDeckTable}). Under a deck over a surface the
 * kernels read `deck.ts`'s `BelowDeckTable`, the whole column's plane-parallel field at the deck's
 * 17 altitudes from the ground to the split, in place of every table of the upper medium:
 *
 * - **The sky** ({@link belowDeckRadianceInto}): the diffuse I = Σ_m Iᵐ cos mΔφ, m = 0 … 2, of the
 *   light's travel cosine −(v·n) and Δφ between the light's direction of travel and the sun's beam,
 *   read as the deck's own series are: each mode stored over sinᵐ θ_v sinᵐ θ₀ at the cell's
 *   centre and summed with the Chebyshev factors cᵐ of the horizontal parts, c¹ = h_⊥, so that
 *   m = 1 is I¹ ÷ (sin θ_v sin θ₀) times h_⊥ = sin θ_v sin θ₀ cos Δφ, continuous where the view or
 *   the sun is vertical. The field is delta-M's diffuse field: its aureole inside the truncated
 *   lobe and its exact single scattering (Nakajima and Tanaka's TMS) are left out, a stated
 *   approximation (the aureole's flux is 0.06% under a τ 12 deck and under 10⁻⁹ at a τ ≥ 30
 *   ground, largest in the deck's upper rows; T15.b.1's science check). No source below the
 *   horizontal lights it: under the first sun row (μ₀ 1/64) the field is scaled by μ₀ ÷ (1/64), as
 *   the flux it carries falls, to 0 at the horizontal (a stated approximation: a deck's twilight
 *   is dark beneath).
 * - **The path to a target** ({@link belowDeckPathDepthInto}): I(h_obs, Ω) − T I(h_target, Ω), both
 *   read in the observer's frame (plane-parallel), T = e^(−Δτ) with Δτ the path's optical depth in
 *   the kernels' extinction (every forward delta out, the absorbers' curves of growth left out), d s
 *   times the mean between the two heights of the extinction the second info row holds, read
 *   geometrically in h\* between the altitudes; the light held at 0. It is the field's own transmission where delta-M's truncation is 0 (Rayleigh's and
 *   every gas's, so everywhere beneath a cloud's base) and the kernels' beam's elsewhere, a stated
 *   approximation inside the deck.
 * - **The ground's light** ({@link belowDeckLightInto}): the sky's irradiance is `irradiance`, the
 *   downward flux the kernels' beam leaves out, and the sun's transmittance `direct`, the kernels'
 *   beam's, so that μ₀ T plus the sky is the whole column's downward flux; a sight to space takes
 *   e^(−τ ÷ μ) of the vertical depth, its green channel's, the absorbers' curves left out.
 *
 * The texture is `rgba32float`, a 2D array of one layer a band and light slot,
 * {@link BELOW_DECK_TABLE_WIDTH} texels across: row a × 32 + s holds, for altitude a and
 * sun cosine (s + ½) ÷ 32, at texel v × 3 + m mode m's Iᵐ ÷ (sinᵐ θ_v sinᵐ θ₀) at travel cosine
 * (v + ½) ÷ 8 − 1, at texel 48 the irradiance and at 49 the beam, red, green and blue in the first
 * three values; the info rows: 544, at texel a the kernels' vertical optical depth from the top per
 * channel and the altitude's h\* in the fourth value; 545, at texel a their extinction per unit of
 * h\*, each of the whole medium's terms in the band's frame (a channel's converted beam, the bins'
 * with factors of either sign, is noise where it is all but out, so the depth is not read from it).
 * 50 × 546 texels, 436,800 B a band and light. As the deck's own table it is read once a ray or a
 * froxel, not a step, so it takes `f32` and no half float.
 */

import type { Rgb } from "../../photometry/toneCurve";
import {
  type AtmosphereMedium,
  columnLengthM,
  densityAt,
  extinction,
  type MediumTerm,
  type TabulatedDensity,
} from "../medium";
import { type BodyGravity, gravityRatio, type SpheroidFigure } from "../oblate";
import type { MsSourceTable } from "./bake";
import type { DeckTables } from "./deck";
import type { PolarisationTable } from "./polarisation";
import {
  THICK_SOURCE_MODES,
  THICK_SUN_COSINES,
  THICK_SUN_COSINES_BELOW,
  THICK_SUN_ROWS_A_UNIT_ABOVE,
  THICK_TRAVEL_COSINES,
  thickSunCosineAt,
  thickSunPlace,
} from "./read";
import { acos, cos, exp, log, pow, sin } from "../../../lib/pinnedMath";

/** The deck table's modes: `deck.ts`'s `DECK_MODES`, 8, held equal by `deckRead.test.ts`. */
export const DECK_TABLE_MODES = 8;
/** The deck table's upward view cosines: 8, at (v + ½) ÷ 8, the bake's. */
export const DECK_TABLE_VIEW_COSINES = 8;
/** The deck's phase's entries: `deck.ts`'s `DECK_PHASE_U`, 256 even in u = √(θ ÷ π). */
export const DECK_TABLE_PHASE_ENTRIES = 256;
/** A cell's texels: the eight ρᵐ − ρ_ssᵐ, the eight βρᵐ, the eight Dᵐ, G and G_β. */
export const DECK_TEXELS_A_CELL = 3 * DECK_TABLE_MODES + 2;
// The texel of a cell where each series begins (the module's remarks), as `deckRead.wgsl` holds them.
const DECK_CELL_REFLECTANCE = 0;
const DECK_CELL_BEAM = DECK_TABLE_MODES;
const DECK_CELL_DIFFUSE = 2 * DECK_TABLE_MODES;
const DECK_CELL_SINGLE = 3 * DECK_TABLE_MODES;
const DECK_CELL_SINGLE_BEAM = 3 * DECK_TABLE_MODES + 1;
/** The texels a row: 208, eight cells of 26. */
export const DECK_TABLE_WIDTH = DECK_TABLE_VIEW_COSINES * DECK_TEXELS_A_CELL;
/** The phase's first row, after the 46 sun cosine rows. */
export const DECK_PHASE_ROW = THICK_SUN_COSINES;
/** The rows a layer: the 46 sun cosines' and the phase's two. */
export const DECK_TABLE_ROWS =
  DECK_PHASE_ROW + Math.ceil(DECK_TABLE_PHASE_ENTRIES / DECK_TABLE_WIDTH);
/** The texture's format: four 32-bit words a texel, three `f32` values' bits and a 0. */
export const DECK_TABLE_FORMAT = "rgba32uint";
/** A layer's bytes as allocated, B: 208 × 48 texels × 16 B, 159,744. */
export const DECK_LAYER_BYTES = DECK_TABLE_WIDTH * DECK_TABLE_ROWS * 16;

/** The thick table's share at μ₀ (`read.ts`'s `thickAnalyticShare`'s complement): clamp(32 μ₀ + 7, 0, 1). */
const SHARE_CELLS_A_UNIT = 32;
const SHARE_EDGE_CELLS = 7;

/**
 * A body's deck tables, packed: what the kernels' `deckTable` holds and the CPU twin reads (the
 * module's remarks). Structured-cloneable.
 */
export interface PackedDeckTable {
  /** Texels a row: {@link DECK_TABLE_WIDTH}. */
  readonly widthTexels: number;
  /** Rows a layer: {@link DECK_TABLE_ROWS}. */
  readonly heightTexels: number;
  /** Bands × slots. */
  readonly layers: number;
  /** The light slots a band. */
  readonly slots: number;
  /** The latitude bands, in the body's order. */
  readonly bands: number;
  /** The deck's sky-lit radiance's modes, those past them held 0. */
  readonly diffuseModes: number;
  /** Four words a texel, row-major in each layer, layer after layer. */
  readonly texels: Uint32Array;
}

/** The bytes a packed deck table allocates, B: {@link DECK_LAYER_BYTES} a layer. */
export function deckTableBytes(table: Pick<PackedDeckTable, "layers">): number {
  return table.layers * DECK_LAYER_BYTES;
}

/** A cosine grid's check: `n` values, each `cosineAt(i)`, to 10⁻¹². */
function onCosines(values: Float64Array, n: number, cosineAt: (i: number) => number): boolean {
  return values.length === n && values.every((value, i) => Math.abs(value - cosineAt(i)) <= 1e-12);
}

/** The view cosine of column `v`: (v + ½) ÷ 8. */
function viewCosineAt(v: number): number {
  return (v + 0.5) / DECK_TABLE_VIEW_COSINES;
}

/**
 * A body's deck tables packed for the kernels (the module's remarks): each band and light slot's
 * three series over sinᵐ θ_v sinᵐ θ₀, G, G_β and the phase.
 *
 * @param bands - The deck's tables (`deck.ts`'s `bakeDeck`), one a latitude band in the body's
 *   order, each with the same light slots.
 * @throws RangeError for no band or no light slot, bands with other light slots or modes, a table
 *   off the grid ({@link THICK_SUN_COSINES} μ₀ at `read.ts`'s `thickSunCosineAt`, eight view
 *   cosines at (v + ½) ÷ 8, {@link DECK_TABLE_MODES} modes, at most as many of the sky-lit radiance,
 *   {@link DECK_TABLE_PHASE_ENTRIES} phase entries), or a value that is not finite.
 */
export function packDeckTable(bands: ReadonlyArray<DeckTables>): PackedDeckTable {
  const [first] = bands;
  if (first === undefined) {
    throw new RangeError("a deck table has at least one band");
  }
  const slots = first.slots.length;
  if (slots === 0) {
    throw new RangeError("a deck table has at least one light slot");
  }
  const diffuseModes = first.diffuseModes;
  for (const [b, band] of bands.entries()) {
    if (
      band.slots.length !== slots ||
      band.modes !== DECK_TABLE_MODES ||
      band.diffuseModes !== diffuseModes ||
      diffuseModes > DECK_TABLE_MODES ||
      !onCosines(band.sunCosines, THICK_SUN_COSINES, thickSunCosineAt) ||
      !onCosines(band.viewCosines, DECK_TABLE_VIEW_COSINES, viewCosineAt) ||
      band.slots.some((slot) => slot.phase.length !== DECK_TABLE_PHASE_ENTRIES * 3)
    ) {
      throw new RangeError(`the deck's band ${b} is off the deck table's grid, modes or slots`);
    }
  }
  const layerWords = DECK_TABLE_WIDTH * DECK_TABLE_ROWS * 4;
  const layers = bands.length * slots;
  const texels = new Uint32Array(layers * layerWords);
  const values = new Float32Array(texels.buffer);
  const views = DECK_TABLE_VIEW_COSINES;
  const modes = DECK_TABLE_MODES;
  const put = (at: number, rgb: Rgb, where: () => string): void => {
    for (let c = 0; c < 3; c += 1) {
      const value = rgb[c] ?? Number.NaN;
      if (!Number.isFinite(value) || !Number.isFinite(Math.fround(value))) {
        throw new RangeError(`the deck table's ${where()} is ${value} in channel ${c}`);
      }
      values[at * 4 + c] = value;
    }
  };
  for (const [b, band] of bands.entries()) {
    for (const [k, slot] of band.slots.entries()) {
      const base = (b * slots + k) * DECK_TABLE_WIDTH * DECK_TABLE_ROWS;
      for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
        const mu0 = band.sunCosines[s] ?? Number.NaN;
        const sunSine = Math.sqrt(1 - mu0 * mu0);
        for (let v = 0; v < views; v += 1) {
          const muV = band.viewCosines[v] ?? Number.NaN;
          const sines = sunSine * Math.sqrt(1 - muV * muV);
          const cell = base + s * DECK_TABLE_WIDTH + v * DECK_TEXELS_A_CELL;
          const point = s * views + v;
          const where =
            (what: string, m: number | null): (() => string) =>
            () =>
              `${what}${m === null ? "" : ` mode ${m}`} at band ${b}, slot ${k}, μ₀ ${s}, μ_v ${v}`;
          let power = 1;
          for (let m = 0; m < modes; m += 1) {
            const series = (table: Float32Array, stride: number): Rgb => {
              const at = (point * stride + m) * 3;
              return [
                (table[at] ?? Number.NaN) / power,
                (table[at + 1] ?? Number.NaN) / power,
                (table[at + 2] ?? Number.NaN) / power,
              ];
            };
            put(cell + DECK_CELL_REFLECTANCE + m, series(slot.reflectance, modes), where("ρ", m));
            put(cell + DECK_CELL_BEAM + m, series(slot.beamReflection, modes), where("βρ", m));
            if (m < diffuseModes) {
              put(cell + DECK_CELL_DIFFUSE + m, series(slot.diffuse, diffuseModes), where("D", m));
            }
            power *= sines;
          }
          const at = point * 3;
          const single = (table: Float32Array): Rgb => [
            table[at] ?? Number.NaN,
            table[at + 1] ?? Number.NaN,
            table[at + 2] ?? Number.NaN,
          ];
          put(cell + DECK_CELL_SINGLE, single(slot.singleGeometry), where("G", null));
          put(cell + DECK_CELL_SINGLE_BEAM, single(slot.singleBeamGeometry), where("G_β", null));
        }
      }
      for (let i = 0; i < DECK_TABLE_PHASE_ENTRIES; i += 1) {
        const texel =
          base +
          (DECK_PHASE_ROW + Math.floor(i / DECK_TABLE_WIDTH)) * DECK_TABLE_WIDTH +
          (i % DECK_TABLE_WIDTH);
        put(
          texel,
          [
            slot.phase[i * 3] ?? Number.NaN,
            slot.phase[i * 3 + 1] ?? Number.NaN,
            slot.phase[i * 3 + 2] ?? Number.NaN,
          ],
          () => `phase entry ${i} at band ${b}, slot ${k}`,
        );
      }
    }
  }
  return {
    widthTexels: DECK_TABLE_WIDTH,
    heightTexels: DECK_TABLE_ROWS,
    layers,
    slots,
    bands: bands.length,
    diffuseModes,
    texels,
  };
}

// ---------------------------------------------------------------------------------------------
// The read.

/** Each packed table's words read as `f32`, made once a table. */
const VALUES = new WeakMap<Uint32Array, Float32Array>();

/** A texel's three values, read from its words' bits. */
function texelAt(table: PackedDeckTable, layer: number, x: number, y: number, out: number[]): void {
  const at = ((layer * DECK_TABLE_ROWS + y) * DECK_TABLE_WIDTH + x) * 4;
  let values = VALUES.get(table.texels);
  if (values === undefined) {
    values = new Float32Array(table.texels.buffer, table.texels.byteOffset, table.texels.length);
    VALUES.set(table.texels, values);
  }
  out[0] = values[at] ?? 0;
  out[1] = values[at + 1] ?? 0;
  out[2] = values[at + 2] ?? 0;
}

/** The deck's read's scratch, kept by a reader between reads. */
export interface DeckReadScratch {
  readonly texel: number[];
  /** The Chebyshev factors cᵐ, m = 0 … 7. */
  readonly harmonics: Float64Array;
  readonly beam: [number, number, number];
  readonly sky: [number, number, number];
}

/** A zeroed {@link DeckReadScratch}. */
export function deckReadScratch(): DeckReadScratch {
  return {
    texel: [0, 0, 0],
    harmonics: new Float64Array(DECK_TABLE_MODES),
    beam: [0, 0, 0],
    sky: [0, 0, 0],
  };
}

/**
 * Where `muTravel` lies among the view cosines: the column below it and its share to the next, held
 * at the edges (y = 8 μ_v − ½ in [0, 7]).
 */
function viewPlace(muTravel: number): { readonly v0: number; readonly fv: number } {
  const y = Math.min(
    Math.max(muTravel * DECK_TABLE_VIEW_COSINES - 0.5, 0),
    DECK_TABLE_VIEW_COSINES - 1,
  );
  const v0 = Math.min(Math.floor(y), DECK_TABLE_VIEW_COSINES - 2);
  return { v0, fv: y - v0 };
}

/** A row place's row below it, at most 44, and its share to the next. */
function rowOf(x: number): { readonly s0: number; readonly fs: number } {
  const s0 = Math.min(Math.floor(x), THICK_SUN_COSINES - 2);
  return { s0, fs: x - s0 };
}

/**
 * One layer's beam and sky-lit series at a point into `scratch.beam` and `scratch.sky`, added at
 * `weight` (`deckRead.wgsl`'s `deckLayerAt`): the beam's Σ_m Xᵐ cᵐ + G p(Θ) of the beam's set
 * (βρ and G_β where `carried`, else ρ − ρ_ss and G), its rows held at the first above the horizontal,
 * and the sky-lit Σ_m Dᵐ cᵐ, each bilinear over the cells' centres about the point.
 */
function addDeckLayer(
  table: PackedDeckTable,
  layer: number,
  muSun: number,
  muTravel: number,
  phase: number,
  carried: boolean,
  weight: number,
  scratch: DeckReadScratch,
): void {
  const { texel, harmonics, beam, sky } = scratch;
  const place = thickSunPlace(muSun);
  const lit = rowOf(Math.max(place, THICK_SUN_COSINES_BELOW));
  const all = rowOf(place);
  const { v0, fv } = viewPlace(muTravel);
  const series = carried ? DECK_CELL_BEAM : DECK_CELL_REFLECTANCE;
  const single = carried ? DECK_CELL_SINGLE_BEAM : DECK_CELL_SINGLE;
  // The phase's two entries about u, each channel's.
  const u = phase * (DECK_TABLE_PHASE_ENTRIES - 1);
  const i0 = Math.min(Math.floor(u), DECK_TABLE_PHASE_ENTRIES - 2);
  const fu = u - i0;
  const p: [number, number, number] = [0, 0, 0];
  for (const [i, w] of [
    [i0, 1 - fu],
    [i0 + 1, fu],
  ] as const) {
    texelAt(
      table,
      layer,
      i % DECK_TABLE_WIDTH,
      DECK_PHASE_ROW + Math.floor(i / DECK_TABLE_WIDTH),
      texel,
    );
    for (let c = 0; c < 3; c += 1) {
      p[c] = (p[c] ?? 0) + w * (texel[c] ?? 0);
    }
  }
  for (const ds of [0, 1] as const) {
    for (const [dv, wv] of [
      [0, 1 - fv],
      [1, fv],
    ] as const) {
      const x0 = (v0 + dv) * DECK_TEXELS_A_CELL;
      const beamWeight = weight * (ds === 0 ? 1 - lit.fs : lit.fs) * wv;
      const skyWeight = weight * (ds === 0 ? 1 - all.fs : all.fs) * wv;
      const beamRow = lit.s0 + ds;
      const skyRow = all.s0 + ds;
      for (let m = 0; m < DECK_TABLE_MODES; m += 1) {
        const h = harmonics[m] ?? 0;
        texelAt(table, layer, x0 + series + m, beamRow, texel);
        for (let c = 0; c < 3; c += 1) {
          beam[c] = (beam[c] ?? 0) + beamWeight * h * (texel[c] ?? 0);
        }
        texelAt(table, layer, x0 + DECK_CELL_DIFFUSE + m, skyRow, texel);
        for (let c = 0; c < 3; c += 1) {
          sky[c] = (sky[c] ?? 0) + skyWeight * h * (texel[c] ?? 0);
        }
      }
      texelAt(table, layer, x0 + single, beamRow, texel);
      for (let c = 0; c < 3; c += 1) {
        beam[c] = (beam[c] ?? 0) + beamWeight * (texel[c] ?? 0) * (p[c] ?? 0);
      }
    }
  }
}

/**
 * The deck's light at a ray's hit on it for one source, per unit of the source's weight, into `out`
 * (the module's remarks; `deckRead.wgsl`'s `deckGroundAt`), sr⁻¹ per unit of the source's
 * irradiance: the beam's term and the sky-lit radiance, held at 0.
 *
 * @param lnBands - ln s_b of the body's bands, ascending, one a band of the table.
 * @param s - The hit's gravity ratio.
 * @param muSun - The source's cosine against the deck's normal at the hit.
 * @param muTravel - The reflected light's travel cosine, −(v·n), v the view's direction.
 * @param horizontal - h_⊥ = v·s − (v·n)(s·n), the view's and the source's horizontal parts' dot product.
 * @param cosTheta - s·v, the cosine of the scattering angle.
 * @param toSun - The kernels' own transmittance to the hit from the source, which a source the bake
 *   did not carry takes in place of its beam's.
 */
export function deckGroundInto(
  table: PackedDeckTable,
  lnBands: ArrayLike<number>,
  s: number,
  muSun: number,
  muTravel: number,
  horizontal: number,
  cosTheta: number,
  slot: number,
  toSun: Readonly<Rgb>,
  scratch: DeckReadScratch,
  out: [number, number, number],
): void {
  const { harmonics, beam, sky } = scratch;
  const crossed = (1 - muTravel * muTravel) * (1 - muSun * muSun);
  harmonics[0] = 1;
  harmonics[1] = horizontal;
  for (let m = 2; m < DECK_TABLE_MODES; m += 1) {
    harmonics[m] = 2 * horizontal * (harmonics[m - 1] ?? 0) - crossed * (harmonics[m - 2] ?? 0);
  }
  beam.fill(0);
  sky.fill(0);
  const carried = slot < table.slots;
  const k = Math.min(slot, table.slots - 1);
  const phase = Math.sqrt(acos(Math.min(Math.max(cosTheta, -1), 1)) / Math.PI);
  const last = table.bands - 1;
  const ls = log(s);
  if (last === 0 || !(ls > (lnBands[0] ?? Number.NaN))) {
    addDeckLayer(table, k, muSun, muTravel, phase, carried, 1, scratch);
  } else if (!(ls < (lnBands[last] ?? Number.NaN))) {
    addDeckLayer(table, last * table.slots + k, muSun, muTravel, phase, carried, 1, scratch);
  } else {
    let lower = 0;
    while (lower + 1 < last && (lnBands[lower + 1] ?? Number.NaN) <= ls) {
      lower += 1;
    }
    const l0 = lnBands[lower] ?? Number.NaN;
    const w = (ls - l0) / ((lnBands[lower + 1] ?? Number.NaN) - l0);
    addDeckLayer(table, lower * table.slots + k, muSun, muTravel, phase, carried, 1 - w, scratch);
    addDeckLayer(table, (lower + 1) * table.slots + k, muSun, muTravel, phase, carried, w, scratch);
  }
  const direct = Math.max(muSun, 0) / Math.PI;
  const share = Math.min(Math.max(muSun * SHARE_CELLS_A_UNIT + SHARE_EDGE_CELLS, 0), 1);
  for (let c = 0; c < 3; c += 1) {
    const transmitted = carried ? 1 : (toSun[c] ?? 0);
    out[c] = Math.max(direct * transmitted * (beam[c] ?? 0) + share * (sky[c] ?? 0), 0);
  }
}

// ---------------------------------------------------------------------------------------------
// Below the deck (R08.T15.b.2).

/**
 * The table below the deck's altitudes: `deck.ts`'s 32 deck layers at the bake's stride 2, 17 from
 * the ground to the split (held equal by `deckRead.test.ts`; not imported, so that the render
 * thread loads no solver).
 */
export const BELOW_DECK_ALTITUDES = 17;
/** Its sun cosines: the thick table's 32 daylit rows, at (s + ½) ÷ 32. */
export const BELOW_DECK_SUN_COSINES = THICK_SUN_ROWS_A_UNIT_ABOVE;
/** Its travel cosines: 16, at (v + ½) ÷ 8 − 1, the downward half first. */
export const BELOW_DECK_TRAVEL_COSINES = THICK_TRAVEL_COSINES;
/** Its azimuthal modes: m = 0, 1 and 2, the bake's (`bake.ts`'s `THICK_BAKE_MODES`). */
export const BELOW_DECK_MODES = THICK_SOURCE_MODES;
/** The texel of a row that holds the downward irradiance the kernels' beam leaves out. */
export const BELOW_DECK_IRRADIANCE_TEXEL = BELOW_DECK_TRAVEL_COSINES * BELOW_DECK_MODES;
/** The texel of a row that holds the kernels' beam's transmission from the top. */
export const BELOW_DECK_DIRECT_TEXEL = BELOW_DECK_IRRADIANCE_TEXEL + 1;
/** The texels a row: 50, the 48 of the radiance's modes, then the irradiance and the beam. */
export const BELOW_DECK_TABLE_WIDTH = BELOW_DECK_DIRECT_TEXEL + 1;
/**
 * The first info row, after the 17 × 32 rows of the altitudes and sun cosines: at texel a the
 * kernels' vertical optical depth from the top per channel and the altitude's h\*.
 */
export const BELOW_DECK_INFO_ROW = BELOW_DECK_ALTITUDES * BELOW_DECK_SUN_COSINES;
/** The second: at texel a the kernels' extinction σ\* per unit of h\* per channel. */
export const BELOW_DECK_EXTINCTION_ROW = BELOW_DECK_INFO_ROW + 1;
/** The rows a layer: 546. */
export const BELOW_DECK_TABLE_ROWS = BELOW_DECK_EXTINCTION_ROW + 1;
/**
 * The texture's format: four `f32` values a texel, `rgba32float`, read by `textureLoad` (so that a
 * lit pass binds one-texel `rgba32float` stub where a body has no deck, as it binds its other
 * tables; `surfaceLighting.ts`'s `createAirlessTables`).
 */
export const BELOW_DECK_TABLE_FORMAT = "rgba32float";
/** A layer's bytes as allocated, B: 50 × 546 texels × 16 B, 436,800. */
export const BELOW_DECK_LAYER_BYTES = BELOW_DECK_TABLE_WIDTH * BELOW_DECK_TABLE_ROWS * 16;

/**
 * A body's tables below its deck, packed (the module's remarks, "Below the deck"): what the
 * kernels' `belowDeckTable` holds and the CPU twin reads. Structured-cloneable.
 */
export interface PackedBelowDeckTable {
  /** Texels a row: {@link BELOW_DECK_TABLE_WIDTH}. */
  readonly widthTexels: number;
  /** Rows a layer: {@link BELOW_DECK_TABLE_ROWS}. */
  readonly heightTexels: number;
  /** Bands × slots. */
  readonly layers: number;
  /** The light slots a band. */
  readonly slots: number;
  /** The latitude bands, in the body's order. */
  readonly bands: number;
  /** Four words a texel, each an `f32` value's bits, row-major in each layer, layer after layer. */
  readonly texels: Uint32Array;
}

/** The bytes a packed table below a deck allocates, B: {@link BELOW_DECK_LAYER_BYTES} a layer. */
export function belowDeckTableBytes(table: Pick<PackedBelowDeckTable, "layers">): number {
  return table.layers * BELOW_DECK_LAYER_BYTES;
}

/** The sun cosine of row `s` below the deck: (s + ½) ÷ 32. */
function belowSunCosineAt(s: number): number {
  return (s + 0.5) / BELOW_DECK_SUN_COSINES;
}

/** The travel cosine of column `v` below the deck: (v + ½) ÷ 8 − 1. */
function belowTravelCosineAt(v: number): number {
  return (v + 0.5) / (BELOW_DECK_TRAVEL_COSINES / 2) - 1;
}

/**
 * A body's tables below its deck packed for the kernels (the module's remarks, "Below the deck"):
 * each band and light slot's radiance, its modes over sinᵐ θ_v sinᵐ θ₀, its irradiance and beam,
 * and the info rows of each altitude's h\*, the kernels' vertical optical depth from the top and
 * their extinction, of `column`'s terms in the band's frame (each coefficient times the band's
 * factor, 1 ÷ s_b).
 *
 * @param bands - The deck's tables (`deck.ts`'s `bakeDeck`), one a latitude band in the body's
 *   order, each over a surface, with the same light slots.
 * @param column - The whole medium the deck was baked from, as the kernels read it (`tables.ts`'s
 *   `kernelMedium`): the depth and the extinction its terms give, the absorbers' curves of growth
 *   left out.
 * @throws RangeError for no band or no light slot, a band with no table below (an envelope's),
 *   bands with other light slots, a table off the grid ({@link BELOW_DECK_ALTITUDES} altitudes
 *   ascending from 0, {@link BELOW_DECK_SUN_COSINES} sun cosines at (s + ½) ÷ 32,
 *   {@link BELOW_DECK_TRAVEL_COSINES} travel cosines at (v + ½) ÷ 8 − 1, {@link BELOW_DECK_MODES}
 *   modes), or a value that is not finite.
 */
export function packBelowDeckTable(
  bands: ReadonlyArray<DeckTables>,
  column: AtmosphereMedium,
): PackedBelowDeckTable {
  const [first] = bands;
  const slots = first?.below?.slots.length ?? 0;
  if (first === undefined || slots === 0) {
    throw new RangeError("a table below a deck has at least one band and one light slot");
  }
  const tables = bands.map((band, b) => {
    const below = band.below;
    if (
      below === null ||
      below.slots.length !== slots ||
      below.modes !== BELOW_DECK_MODES ||
      below.altitudesM.length !== BELOW_DECK_ALTITUDES ||
      below.altitudesM[0] !== 0 ||
      !below.altitudesM.every((h, a) => a === 0 || h > (below.altitudesM[a - 1] ?? Number.NaN)) ||
      !onCosines(below.sunCosines, BELOW_DECK_SUN_COSINES, belowSunCosineAt) ||
      !onCosines(below.travelCosines, BELOW_DECK_TRAVEL_COSINES, belowTravelCosineAt)
    ) {
      throw new RangeError(`the deck's band ${b} has no table below it on the packing's grid`);
    }
    return below;
  });
  const width = BELOW_DECK_TABLE_WIDTH;
  const layerTexels = width * BELOW_DECK_TABLE_ROWS;
  const layers = bands.length * slots;
  const texels = new Uint32Array(layers * layerTexels * 4);
  const values = new Float32Array(texels.buffer);
  const suns = BELOW_DECK_SUN_COSINES;
  const views = BELOW_DECK_TRAVEL_COSINES;
  const modes = BELOW_DECK_MODES;
  const put = (at: number, word: number, value: number, where: () => string): void => {
    if (!Number.isFinite(value) || !Number.isFinite(Math.fround(value))) {
      throw new RangeError(`the table below the deck's ${where()} is ${value}`);
    }
    values[at * 4 + word] = value;
  };
  for (const [b, below] of tables.entries()) {
    for (const [k, slot] of below.slots.entries()) {
      const base = (b * slots + k) * layerTexels;
      for (let a = 0; a < BELOW_DECK_ALTITUDES; a += 1) {
        for (let s = 0; s < suns; s += 1) {
          const mu0 = below.sunCosines[s] ?? Number.NaN;
          const sunSine = Math.sqrt(1 - mu0 * mu0);
          const row = base + (a * suns + s) * width;
          const where =
            (what: string): (() => string) =>
            () =>
              `${what} at band ${b}, slot ${k}, altitude ${a}, μ₀ ${s}`;
          for (let v = 0; v < views; v += 1) {
            const muV = below.travelCosines[v] ?? Number.NaN;
            const sines = sunSine * Math.sqrt(1 - muV * muV);
            let power = 1;
            for (let m = 0; m < modes; m += 1) {
              for (let c = 0; c < 3; c += 1) {
                const at = (((a * suns + s) * views + v) * modes + m) * 3 + c;
                put(
                  row + v * modes + m,
                  c,
                  (slot.radiance[at] ?? Number.NaN) / power,
                  where(`radiance mode ${m}, travel cosine ${v}, channel ${c}`),
                );
              }
              power *= sines;
            }
          }
          for (let c = 0; c < 3; c += 1) {
            const at = (a * suns + s) * 3 + c;
            put(
              row + BELOW_DECK_IRRADIANCE_TEXEL,
              c,
              slot.irradiance[at] ?? Number.NaN,
              where("irradiance"),
            );
            put(row + BELOW_DECK_DIRECT_TEXEL, c, slot.direct[at] ?? Number.NaN, where("beam"));
          }
        }
        // The info rows: the kernels' vertical optical depth from the top and their extinction,
        // per channel, in the band's frame, and the altitude's h*.
        const heightM = below.altitudesM[a] ?? Number.NaN;
        const factor = bands[b]?.band.coefficientFactor ?? Number.NaN;
        const depth = [0, 0, 0];
        const sigma = [0, 0, 0];
        for (const term of column.terms) {
          const ext = extinction(term);
          const above =
            columnLengthM(term.density, column.topHeightM) - columnLengthM(term.density, heightM);
          const rho = densityAt(term.density, heightM);
          for (let c = 0; c < 3; c += 1) {
            depth[c] = (depth[c] ?? 0) + factor * (ext[c] ?? Number.NaN) * above;
            sigma[c] = (sigma[c] ?? 0) + factor * (ext[c] ?? Number.NaN) * rho;
          }
        }
        const info = base + BELOW_DECK_INFO_ROW * width + a;
        const sigmaTexel = base + BELOW_DECK_EXTINCTION_ROW * width + a;
        for (let c = 0; c < 3; c += 1) {
          put(info, c, depth[c] ?? Number.NaN, () => `depth at altitude ${a}`);
          put(sigmaTexel, c, sigma[c] ?? Number.NaN, () => `extinction at altitude ${a}`);
        }
        put(info, 3, heightM, () => `altitude ${a}`);
      }
    }
  }
  return {
    widthTexels: width,
    heightTexels: BELOW_DECK_TABLE_ROWS,
    layers,
    slots,
    bands: bands.length,
    texels,
  };
}

/** A texel of a table below the deck: its four values, read from its words' bits. */
function belowTexelInto(
  table: PackedBelowDeckTable,
  layer: number,
  x: number,
  y: number,
  out: number[],
): void {
  const at = ((layer * BELOW_DECK_TABLE_ROWS + y) * BELOW_DECK_TABLE_WIDTH + x) * 4;
  let values = VALUES.get(table.texels);
  if (values === undefined) {
    values = new Float32Array(table.texels.buffer, table.texels.byteOffset, table.texels.length);
    VALUES.set(table.texels, values);
  }
  out[0] = values[at] ?? 0;
  out[1] = values[at + 1] ?? 0;
  out[2] = values[at + 2] ?? 0;
  out[3] = values[at + 3] ?? 0;
}

/** The reads' scratch below the deck, kept by a reader between reads. */
export interface BelowDeckReadScratch {
  readonly texel: number[];
  /** The Chebyshev factors cᵐ, m = 0 … 2. */
  readonly harmonics: Float64Array;
  readonly sum: [number, number, number];
  readonly other: [number, number, number];
}

/** A zeroed {@link BelowDeckReadScratch}. */
export function belowDeckReadScratch(): BelowDeckReadScratch {
  return {
    texel: [0, 0, 0, 0],
    harmonics: new Float64Array(BELOW_DECK_MODES),
    sum: [0, 0, 0],
    other: [0, 0, 0],
  };
}

/** A place among a table's rows: the row below, its share to the next, and a scale on the value. */
interface BelowPlace {
  readonly i0: number;
  readonly f: number;
}

/** Each packed table's altitudes by layer (the info row's fourth words), read once a table. */
const ALTITUDES = new WeakMap<Uint32Array, Float64Array[]>();

/** A layer's altitudes, h\* above the ground, ascending from 0 to the split. */
function belowAltitudesOf(table: PackedBelowDeckTable, layer: number): Float64Array {
  let byLayer = ALTITUDES.get(table.texels);
  if (byLayer === undefined) {
    const texel = [0, 0, 0, 0];
    byLayer = Array.from({ length: table.layers }, (_, l) =>
      Float64Array.from({ length: BELOW_DECK_ALTITUDES }, (__, a) => {
        belowTexelInto(table, l, a, BELOW_DECK_INFO_ROW, texel);
        return texel[3] ?? Number.NaN;
      }),
    );
    ALTITUDES.set(table.texels, byLayer);
  }
  const altitudes = byLayer[layer];
  if (altitudes === undefined) {
    throw new RangeError(
      `the table below the deck has ${table.layers} layers, not a layer ${layer}`,
    );
  }
  return altitudes;
}

/**
 * Where `heightStarM` lies among a layer's altitudes: the row below it and its share to the next,
 * held at the ground and at the split.
 */
function belowAltitudePlace(
  table: PackedBelowDeckTable,
  layer: number,
  heightStarM: number,
): BelowPlace {
  const altitudes = belowAltitudesOf(table, layer);
  let lower = 0;
  while (lower + 2 < BELOW_DECK_ALTITUDES && (altitudes[lower + 1] ?? Number.NaN) <= heightStarM) {
    lower += 1;
  }
  const h0 = altitudes[lower] ?? Number.NaN;
  const h1 = altitudes[lower + 1] ?? Number.NaN;
  return { i0: lower, f: Math.min(Math.max((heightStarM - h0) / (h1 - h0), 0), 1) };
}

/**
 * Where `muSun` lies among the sun cosines: the row below it, its share to the next, held at the
 * last row; below the first row, (1/64), the first row, which the reads scale by μ₀ ÷ (1/64).
 */
function belowSunPlace(muSun: number): BelowPlace {
  const x = Math.min(Math.max(muSun * BELOW_DECK_SUN_COSINES - 0.5, 0), BELOW_DECK_SUN_COSINES - 1);
  const s0 = Math.min(Math.floor(x), BELOW_DECK_SUN_COSINES - 2);
  return { i0: s0, f: x - s0 };
}

/**
 * The diffuse field's scale at a low sun: μ₀ ÷ (1/64) under the first row, which the field and its
 * flux follow to 0 at the horizontal; 0 below it, where the table holds no light (a stated
 * approximation: a deck's twilight is dark beneath).
 */
function belowLowSunScale(muSun: number): number {
  return Math.min(Math.max(muSun * 2 * BELOW_DECK_SUN_COSINES, 0), 1);
}

/** Where `muTravel` lies among the travel cosines: y = 8 (μ + 1) − ½ in [0, 15]. */
function belowTravelPlace(muTravel: number): BelowPlace {
  const half = BELOW_DECK_TRAVEL_COSINES / 2;
  const y = Math.min(Math.max((muTravel + 1) * half - 0.5, 0), BELOW_DECK_TRAVEL_COSINES - 1);
  const v0 = Math.min(Math.floor(y), BELOW_DECK_TRAVEL_COSINES - 2);
  return { i0: v0, f: y - v0 };
}

/**
 * Each band layer about the gravity ratio `s` with its weight, linear in ln s between the bands
 * about it and the nearer band beyond them (`deckGroundInto`'s), for the light slot `slot`, the
 * nearest slot held where the table has fewer.
 */
function belowLayersAt(
  table: PackedBelowDeckTable,
  lnBands: ArrayLike<number>,
  s: number,
  slot: number,
): ReadonlyArray<readonly [number, number]> {
  const k = Math.min(Math.max(slot, 0), table.slots - 1);
  const last = table.bands - 1;
  const ls = log(s);
  if (last === 0 || !(ls > (lnBands[0] ?? Number.NaN))) {
    return [[k, 1]];
  }
  if (!(ls < (lnBands[last] ?? Number.NaN))) {
    return [[last * table.slots + k, 1]];
  }
  let lower = 0;
  while (lower + 1 < last && (lnBands[lower + 1] ?? Number.NaN) <= ls) {
    lower += 1;
  }
  const l0 = lnBands[lower] ?? Number.NaN;
  const w = (ls - l0) / ((lnBands[lower + 1] ?? Number.NaN) - l0);
  return [
    [lower * table.slots + k, 1 - w],
    [(lower + 1) * table.slots + k, w],
  ];
}

/**
 * The diffuse radiance beneath a deck for one source, per unit of the source's weight, into `out`
 * (the module's remarks, "Below the deck"; `deckRead.wgsl`'s `belowDeckRadianceAt`), sr⁻¹ per unit
 * of the source's irradiance at the top: Σ_m Iᵐ cᵐ, cᵐ the Chebyshev factors of the view's and
 * the source's horizontal parts (`deckGroundInto`'s), linear between the cells' centres in h\*, μ₀
 * and μ, scaled by μ₀ ÷ (1/64) under the first sun row, held at 0.
 *
 * @param heightStarM - The point's h\* above the ground, held to the table's altitudes.
 * @param muSun - The source's cosine against the datum's normal there.
 * @param muTravel - The light's travel cosine, −(v·n), v the view's direction.
 * @param horizontal - h_⊥ = v·s − (v·n)(s·n), the view's and the source's horizontal parts' dot
 *   product.
 */
export function belowDeckRadianceInto(
  table: PackedBelowDeckTable,
  lnBands: ArrayLike<number>,
  s: number,
  heightStarM: number,
  muSun: number,
  muTravel: number,
  horizontal: number,
  slot: number,
  scratch: BelowDeckReadScratch,
  out: [number, number, number],
): void {
  const { texel, harmonics, sum } = scratch;
  const scale = belowLowSunScale(muSun);
  out.fill(0);
  if (!(scale > 0)) {
    return;
  }
  const crossed = (1 - muTravel * muTravel) * (1 - muSun * muSun);
  harmonics[0] = 1;
  harmonics[1] = horizontal;
  for (let m = 2; m < BELOW_DECK_MODES; m += 1) {
    harmonics[m] = 2 * horizontal * (harmonics[m - 1] ?? 0) - crossed * (harmonics[m - 2] ?? 0);
  }
  const sun = belowSunPlace(muSun);
  const travel = belowTravelPlace(muTravel);
  sum.fill(0);
  for (const [layer, w] of belowLayersAt(table, lnBands, s, slot)) {
    const altitude = belowAltitudePlace(table, layer, heightStarM);
    for (const [da, wa] of [
      [0, 1 - altitude.f],
      [1, altitude.f],
    ] as const) {
      for (const [ds, ws] of [
        [0, 1 - sun.f],
        [1, sun.f],
      ] as const) {
        const row = (altitude.i0 + da) * BELOW_DECK_SUN_COSINES + sun.i0 + ds;
        for (const [dv, wv] of [
          [0, 1 - travel.f],
          [1, travel.f],
        ] as const) {
          const weight = w * wa * ws * wv;
          for (let m = 0; m < BELOW_DECK_MODES; m += 1) {
            belowTexelInto(table, layer, (travel.i0 + dv) * BELOW_DECK_MODES + m, row, texel);
            const h = weight * (harmonics[m] ?? 0);
            for (let c = 0; c < 3; c += 1) {
              sum[c] = (sum[c] ?? 0) + h * (texel[c] ?? 0);
            }
          }
        }
      }
    }
  }
  for (let c = 0; c < 3; c += 1) {
    out[c] = Math.max(scale * (sum[c] ?? 0), 0);
  }
}

/**
 * One step of the beam's read between two values: geometric at share `f` where both hold light,
 * else linear at share `linear`.
 */
function beamStep(t0: number, t1: number, f: number, linear: number): number {
  return t0 > 0 && t1 > 0 ? exp((1 - f) * log(t0) + f * log(t1)) : (1 - linear) * t0 + linear * t1;
}

/**
 * The light on a horizontal surface beneath a deck for one source, per unit of the source's
 * irradiance normal to its beam at the top (the module's remarks, "Below the deck";
 * `deckRead.wgsl`'s `belowDeckSkyAt` and `belowDeckSunAt`): into `sky`, the downward irradiance
 * the kernels' beam leaves out (the diffuse field's and delta-M's aureole), linear in h\* and μ₀ and
 * scaled under the first sun row as the radiance is; into `sun`, the kernels' beam's transmission
 * T_k from the top, geometric in 1 ÷ μ₀ between the rows about the point and then in h\* where both
 * values of a step hold light (exact for a plane-parallel beam through a column of even
 * extinction), else linear, T^(μ_r ÷ μ₀) of a row μ_r beyond the rows or above a row the `f32` holds
 * as 0, 0 at and below the horizontal. A channel's beam is the bins' converted, whose factors may be
 * negative, so it may be below 0 where the bins' beams are all but out; it is read as stored. μ₀ T_k plus the
 * sky is the whole column's downward flux at the table's altitudes.
 */
export function belowDeckLightInto(
  table: PackedBelowDeckTable,
  lnBands: ArrayLike<number>,
  s: number,
  heightStarM: number,
  muSun: number,
  slot: number,
  scratch: BelowDeckReadScratch,
  sky: [number, number, number],
  sun: [number, number, number],
): void {
  const { texel, sum } = scratch;
  sky.fill(0);
  sun.fill(0);
  if (!(muSun > 0)) {
    return;
  }
  const scale = belowLowSunScale(muSun);
  const place = belowSunPlace(muSun);
  const first = belowSunCosineAt(0);
  const last = belowSunCosineAt(BELOW_DECK_SUN_COSINES - 1);
  const beyond = muSun < first || muSun > last;
  for (const [layer, w] of belowLayersAt(table, lnBands, s, slot)) {
    const altitude = belowAltitudePlace(table, layer, heightStarM);
    // The four corners' beams, [altitude][sun], per channel.
    const beams: number[][] = [];
    sum.fill(0);
    for (const [da, wa] of [
      [0, 1 - altitude.f],
      [1, altitude.f],
    ] as const) {
      for (const [ds, ws] of [
        [0, 1 - place.f],
        [1, place.f],
      ] as const) {
        const row = (altitude.i0 + da) * BELOW_DECK_SUN_COSINES + place.i0 + ds;
        belowTexelInto(table, layer, BELOW_DECK_IRRADIANCE_TEXEL, row, texel);
        for (let c = 0; c < 3; c += 1) {
          sum[c] = (sum[c] ?? 0) + wa * ws * (texel[c] ?? 0);
        }
        belowTexelInto(table, layer, BELOW_DECK_DIRECT_TEXEL, row, texel);
        beams.push([texel[0] ?? 0, texel[1] ?? 0, texel[2] ?? 0]);
      }
    }
    for (let c = 0; c < 3; c += 1) {
      sky[c] = (sky[c] ?? 0) + w * scale * (sum[c] ?? 0);
    }
    // The beam: in 1 ÷ μ₀ between the rows (the edge row's power beyond them), then in h* between
    // the altitudes, each step geometric where both its values hold light, else linear.
    const mu0 = belowSunCosineAt(place.i0);
    const mu1 = belowSunCosineAt(place.i0 + 1);
    const edge = muSun < first ? 0 : 1;
    const power = (muSun < first ? first : last) / muSun;
    const fs = (1 / muSun - 1 / mu0) / (1 / mu1 - 1 / mu0);
    for (let c = 0; c < 3; c += 1) {
      const atRow = (da: 0 | 1): number => {
        const lower = beams[da * 2]?.[c] ?? 0;
        const upper = beams[da * 2 + 1]?.[c] ?? 0;
        if (beyond) {
          const held = edge === 0 ? lower : upper;
          return held > 0 ? pow(held, power) : held;
        }
        // A lower row that the f32 holds as 0 under a lit one: the lit row's plane-parallel power.
        // (A channel's beam is the bins' converted, whose factors may be negative, so a row may
        // hold one below 0: that one is read linearly, as is.)
        if (lower === 0 && upper > 0) {
          return pow(upper, mu1 / muSun);
        }
        return beamStep(lower, upper, fs, place.f);
      };
      sun[c] = (sun[c] ?? 0) + w * beamStep(atRow(0), atRow(1), altitude.f, altitude.f);
    }
  }
}

/**
 * The kernels' vertical optical depth from the top to h\* beneath a deck, per channel, into `out`
 * (the info row's, linear in h\* between the altitudes and in ln s between the bands).
 */
export function belowDeckOpticalDepthInto(
  table: PackedBelowDeckTable,
  lnBands: ArrayLike<number>,
  s: number,
  heightStarM: number,
  scratch: BelowDeckReadScratch,
  out: [number, number, number],
): void {
  const { texel } = scratch;
  out.fill(0);
  for (const [layer, w] of belowLayersAt(table, lnBands, s, 0)) {
    const altitude = belowAltitudePlace(table, layer, heightStarM);
    for (const [da, wa] of [
      [0, 1 - altitude.f],
      [1, altitude.f],
    ] as const) {
      belowTexelInto(table, layer, altitude.i0 + da, BELOW_DECK_INFO_ROW, texel);
      for (let c = 0; c < 3; c += 1) {
        out[c] = (out[c] ?? 0) + w * wa * (texel[c] ?? 0);
      }
    }
  }
}

/**
 * ∫ σ dh over [x0, x1] inside an altitude interval [h0, h1] whose extinction runs from σ0 to σ1,
 * geometric in h (exact for an exponential profile) where both are above 0, else linear.
 */
function intervalDepth(
  h0: number,
  h1: number,
  s0: number,
  s1: number,
  x0: number,
  x1: number,
): number {
  const span = h1 - h0;
  const f0 = (x0 - h0) / span;
  const f1 = (x1 - h0) / span;
  if (s0 > 0 && s1 > 0) {
    const lr = log(s1 / s0);
    if (Math.abs(lr) > 1e-6) {
      return (s0 * span * (exp(lr * f1) - exp(lr * f0))) / lr;
    }
  }
  return span * (s0 * (f1 - f0) + 0.5 * (s1 - s0) * (f1 * f1 - f0 * f0));
}

/** The extinction at a share `f` of an interval from σ0 to σ1: geometric where both are above 0. */
function extinctionAt(s0: number, s1: number, f: number): number {
  return s0 > 0 && s1 > 0 ? s0 * exp(f * log(s1 / s0)) : (1 - f) * s0 + f * s1;
}

/**
 * The optical depth of a straight path beneath a deck, per channel, into `out` (the module's
 * remarks, "Below the deck"): d s σ̄\*, σ̄\* the mean between h\*₁ and h\*₂ of the kernels'
 * extinction σ\* per unit of h\* (the second info row), read geometrically in h\* between the
 * altitudes about each point (exact for an exponential profile), the plane-parallel path's own,
 * and σ\*(h\*₁) where the two lie within a millimetre.
 *
 * @param distanceM - The path's length, m.
 */
export function belowDeckPathDepthInto(
  table: PackedBelowDeckTable,
  lnBands: ArrayLike<number>,
  s: number,
  fromHeightStarM: number,
  toHeightStarM: number,
  distanceM: number,
  scratch: BelowDeckReadScratch,
  out: [number, number, number],
): void {
  const { texel, sum, other } = scratch;
  const lowM = Math.min(fromHeightStarM, toHeightStarM);
  const highM = Math.max(fromHeightStarM, toHeightStarM);
  const level = !(highM - lowM > 1e-3);
  out.fill(0);
  for (const [layer, w] of belowLayersAt(table, lnBands, s, 0)) {
    const altitudes = belowAltitudesOf(table, layer);
    sum.fill(0);
    if (level) {
      const place = belowAltitudePlace(table, layer, fromHeightStarM);
      belowTexelInto(table, layer, place.i0, BELOW_DECK_EXTINCTION_ROW, texel);
      other[0] = texel[0] ?? 0;
      other[1] = texel[1] ?? 0;
      other[2] = texel[2] ?? 0;
      belowTexelInto(table, layer, place.i0 + 1, BELOW_DECK_EXTINCTION_ROW, texel);
      for (let c = 0; c < 3; c += 1) {
        sum[c] = extinctionAt(other[c] ?? 0, texel[c] ?? 0, place.f);
      }
    } else {
      for (let a = 0; a + 1 < BELOW_DECK_ALTITUDES; a += 1) {
        const h0 = altitudes[a] ?? Number.NaN;
        const h1 = altitudes[a + 1] ?? Number.NaN;
        const x0 = Math.max(lowM, h0);
        const x1 = Math.min(highM, h1);
        if (!(x1 > x0)) {
          continue;
        }
        belowTexelInto(table, layer, a, BELOW_DECK_EXTINCTION_ROW, texel);
        other[0] = texel[0] ?? 0;
        other[1] = texel[1] ?? 0;
        other[2] = texel[2] ?? 0;
        belowTexelInto(table, layer, a + 1, BELOW_DECK_EXTINCTION_ROW, texel);
        for (let c = 0; c < 3; c += 1) {
          sum[c] = (sum[c] ?? 0) + intervalDepth(h0, h1, other[c] ?? 0, texel[c] ?? 0, x0, x1);
        }
      }
      for (let c = 0; c < 3; c += 1) {
        sum[c] = (sum[c] ?? 0) / (highM - lowM);
      }
    }
    for (let c = 0; c < 3; c += 1) {
      out[c] = (out[c] ?? 0) + w * distanceM * s * (sum[c] ?? 0);
    }
  }
}

/** The split's h\* of a table below a deck: its last altitude, layer 0's. */
export function belowDeckSplitM(table: PackedBelowDeckTable): number {
  const texel = [0, 0, 0, 0];
  belowTexelInto(table, 0, BELOW_DECK_ALTITUDES - 1, BELOW_DECK_INFO_ROW, texel);
  return texel[3] ?? Number.NaN;
}

// ---------------------------------------------------------------------------------------------
// The kernels' medium and datum.

/** A tabulated profile from `splitM` up, its heights less `splitM`: exact for its linear read. */
function tabulatedAbove(profile: TabulatedDensity, splitM: number): TabulatedDensity {
  const heights: number[] = [0];
  const relative: number[] = [densityAt(profile, splitM)];
  for (const [i, h] of profile.altitudesM.entries()) {
    if (h > splitM) {
      heights.push(h - splitM);
      relative.push(profile.relative[i] ?? Number.NaN);
    }
  }
  if (heights.length === 1) {
    heights.push(1);
    relative.push(relative[0] ?? Number.NaN);
  }
  const edgesM = profile.edgesM?.map((e) => e - splitM).filter((e) => e > 0);
  return {
    kind: "tabulated",
    altitudesM: Float64Array.from(heights),
    relative: Float64Array.from(relative),
    ...(edgesM === undefined ? {} : { edgesM }),
  };
}

/** A term from `splitM` up (the module's remarks): its density from there, its ground there. */
function termAbove(term: MediumTerm, splitM: number): MediumTerm {
  const density = term.density;
  let above: MediumTerm;
  switch (density.kind) {
    case "exponential": {
      // e^(−(h + h*) ÷ H): the profile's own, its coefficients and amount at the split.
      const at = exp(-splitM / density.scaleHeightM);
      const scaled = (rgb: Rgb): Rgb => [rgb[0] * at, rgb[1] * at, rgb[2] * at];
      const absorber = term.absorber;
      let moved: MediumTerm["absorber"];
      if (absorber === undefined) {
        moved = undefined;
      } else {
        switch (absorber.kind) {
          case "column":
            moved = { ...absorber, numberDensityPerM3: absorber.numberDensityPerM3 * at };
            break;
          case "pair":
            moved = { ...absorber, pairDensityPerM6: absorber.pairDensityPerM6 * at };
            break;
        }
      }
      above = {
        ...term,
        scattering: scaled(term.scattering),
        absorption: scaled(term.absorption),
        ...(moved === undefined ? {} : { absorber: moved }),
      };
      break;
    }
    case "tent":
      above = {
        ...term,
        density: {
          kind: "tent",
          bottomM: density.bottomM - splitM,
          peakM: density.peakM - splitM,
          topM: density.topM - splitM,
        },
      };
      break;
    case "tabulated":
      above = { ...term, density: tabulatedAbove(density, splitM) };
      break;
  }
  return above;
}

/**
 * The upper medium of a deck split at `splitM` as the kernels' medium (the module's remarks): each
 * term's density from the split up, h\* measured from the split, the top lowered by it, and the
 * pressure profile with them; the ground's albedo the medium's, which the march's deck read
 * replaces at the ground.
 *
 * @throws RangeError for a split that is not inside the medium, (0, top).
 */
export function upperMediumOf(medium: AtmosphereMedium, splitM: number): AtmosphereMedium {
  if (!(splitM > 0 && splitM < medium.topHeightM)) {
    throw new RangeError(
      `a deck's split lies inside its medium, (0, ${medium.topHeightM}) m, not at ${splitM} m`,
    );
  }
  const pressureBar = medium.pressureBar;
  return {
    ...medium,
    name: `${medium.name} above its deck`,
    topHeightM: medium.topHeightM - splitM,
    terms: medium.terms.map((term) => termAbove(term, splitM)),
    ...(pressureBar === undefined ? {} : { pressureBar: tabulatedAbove(pressureBar, splitM) }),
  };
}

/** A table's altitudes from the split: each less `splitM`, the split's own exactly 0. */
function fromSplit(altitudesM: Float64Array, splitM: number): Float64Array {
  return altitudesM.map((h) => h - splitM);
}

/**
 * The bake's J above the deck, one table a band, its altitudes from the split, as the upper medium's
 * kernels read it (`read.ts`'s `packThickTable` against {@link upperMediumOf}'s kernel medium).
 */
export function deckAboveTables(bands: ReadonlyArray<DeckTables>): MsSourceTable[] {
  return bands.map((band) => ({
    ...band.above,
    altitudesM: fromSplit(band.above.altitudesM, band.deckAltitudeM),
  }));
}

/**
 * The polarisation correction over a deck (`deck.ts`'s `bakeDeckPolarisation`), one table a band,
 * its altitudes from the split at `splitM`, to sum into {@link deckAboveTables}' J.
 */
export function deckCorrectionAbove(
  bands: ReadonlyArray<PolarisationTable>,
  splitM: number,
): PolarisationTable[] {
  return bands.map((band) => ({ ...band, altitudesM: fromSplit(band.altitudesM, splitM) }));
}

/**
 * The kernels' datum over a deck split at `splitM` (the module's remarks): the spheroid
 * (a + h\* ÷ s_e, c + h\* ÷ s_p) through the deck's level surface at the equator and the poles, s
 * the body's gravity ratio there (1 for a body with no spheroid).
 *
 * @throws RangeError for a split that is not finite and above 0.
 */
export function deckDatumOf(
  figure: SpheroidFigure,
  gravity: BodyGravity,
  splitM: number,
): SpheroidFigure {
  if (!(Number.isFinite(splitM) && splitM > 0)) {
    throw new RangeError(`a deck's split lies above the datum, not at ${splitM} m`);
  }
  return {
    equatorialRadiusM: figure.equatorialRadiusM + splitM / gravityRatio(gravity, 0),
    polarRadiusM: figure.polarRadiusM + splitM / gravityRatio(gravity, Math.PI / 2),
  };
}

/**
 * How far {@link deckDatumOf}'s spheroid lies from the deck's level surface at geodetic latitude φ,
 * m: its height above the body's datum along the datum's normal there, less h\* ÷ s(φ), the level
 * surface's (Design note 17). 0 at the equator and the poles.
 */
export function deckDatumDepartureM(
  figure: SpheroidFigure,
  gravity: BodyGravity,
  splitM: number,
  geodeticLatitudeRad: number,
): number {
  const raised = deckDatumOf(figure, gravity, splitM);
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const sinLatitude = sin(geodeticLatitudeRad);
  const cosLatitude = cos(geodeticLatitudeRad);
  // The datum's point at φ and its normal; then t along the normal to the raised spheroid, the
  // root of (ρ + t cos φ)² ÷ A² + (z + t sin φ)² ÷ C² = 1.
  const n =
    (a * a) / Math.sqrt(a * a * cosLatitude * cosLatitude + c * c * sinLatitude * sinLatitude);
  const rho = n * cosLatitude;
  const z = ((n * c * c) / (a * a)) * sinLatitude;
  const big = raised.equatorialRadiusM;
  const small = raised.polarRadiusM;
  const qa =
    (cosLatitude * cosLatitude) / (big * big) + (sinLatitude * sinLatitude) / (small * small);
  const qb = (rho * cosLatitude) / (big * big) + (z * sinLatitude) / (small * small);
  const qc = (rho * rho) / (big * big) + (z * z) / (small * small) - 1;
  const t = -qc / (qb + Math.sqrt(qb * qb - qa * qc));
  return t - splitM / gravityRatio(gravity, geodeticLatitudeRad);
}
