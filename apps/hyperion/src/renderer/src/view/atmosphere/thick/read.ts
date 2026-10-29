/**
 * The thick multiple-scattering table as the kernels read it (plan R08, R08.T14.c and R08.T14.f;
 * Design notes 9 and 10): the bake's J_ms (`bake.ts`'s `MsSourceTable`, one a latitude band) and the
 * polarisation correction ΔJ_pol (`polarisation.ts`'s `PolarisationTable`) packed for the GPU, and
 * their read, which the sky-view, aerial-perspective and march kernels make in place of, or beside,
 * σ_s·Ψ_ms·p_u (`shaders/thickRead.wgsl`) and the CPU twin makes alike (`tablesCpu.ts`).
 *
 * @remarks
 * **What is stored.** The bake gives J = Σ_m Jᵐ cos mΔφ, m = 0 … 2, the source of orders two and
 * up per steradian, per unit of the kernels' scattering σ_s,k and of the star's irradiance normal to
 * its beam at the top (sr⁻¹; `bake.ts`'s remarks, "What the table holds is J", R08.T14.d.1); the
 * correction gives ΔJ = Σ_m ΔJ_m cos mΔφ, m = 0 … 2, per unit of the column's whole σ_s (every
 * forward delta in), so that σ_s ΔJ is the emission's vector less its scalar (R08.T14.e's record,
 * _ΔJ's unit and frame_). A table is of one of two kinds ({@link ThickTableKind}):
 *
 * - **`thick`**, a body whose bake landed: J + ΔJ σ_s ÷ σ_s,k, the two summed on T14.b's grid, which
 *   they share to the bit, so that the kernels make one read where a thin world makes Ψ_ms's and the
 *   correction's (`decision-r08-t13c2-classes.md` §6's lever, R08.T14.f). m = 2 is the bake's J²
 *   and the correction's ΔJ_2 together; a body with no correction, or none yet, holds the bake's
 *   alone.
 * - **`correction`**, a thin world: ΔJ σ_s ÷ σ_s,k alone, added to Hillaire's term, which stays
 *   whole.
 *
 * The table stores, per channel, at each of the bake's altitudes h, sun cosines μ₀ and travel
 * cosines μ_v, with Jᵐ the m-th coefficient of what the kind sums, per unit of σ_s,k:
 *
 * - J⁰,
 * - G = J¹ ÷ (sin θ_v sin θ₀), and
 * - G₂ = J² ÷ (sin² θ_v sin² θ₀), sin θ = √(1 − μ²) at the cell's centre,
 *
 * σ_s,k(h) the kernels' own scattering at h, every term's forward delta out
 * (`tables.ts`'s `kernelMedium`), the factor each kernel multiplies its multiple scattering by, and
 * σ_s(h) of the correction the drawn medium's, the deltas in: σ_s,k ÷ (1 − f̄_c), as the correction's
 * record has a kernel take it. So J is per steradian and per unit of scattering, as Hillaire's Ψ_ms
 * is (sr⁻¹), and the read is J = J⁰ + G·h_⊥ + G₂·(2h_⊥² − (1 − μ_v²)(1 − μ₀²)), h_⊥ = v·s − (v·n)(s·n)
 * for the view's direction v, the source's s and the zenith n: Σ_m Jᵐ cos mΔφ exactly at the cells'
 * centres, since sin θ_v sin θ₀ cos Δφ is v's and s's horizontal parts' dot product and
 * sin² θ_v sin² θ₀ cos 2Δφ = 2h_⊥² − sin² θ_v sin² θ₀.
 * - Per unit of scattering, the values lie in the half float's range. The emission E = σ_s,k J is
 *   up to about 10⁻⁵ m⁻¹ sr⁻¹ near an Earth-like ground (7.5 × 10⁻⁶ on `earth`, plan R08's record
 *   of T14.c) and far less aloft, under `rgba16float`'s least normal (6.1 × 10⁻⁵), while J is of
 *   Ψ_ms's order, 10⁻³–10⁻¹ sr⁻¹ (plan R08's "Deferred findings", T14.b's _Half floats_). It is
 *   also what varies slowly with height: E follows the density's exponential between the table's
 *   rows, 1.3–24 km apart, and J does not.
 * - G and G₂ in place of J¹ and J² keep the read continuous where the view or the sun is vertical.
 *   Jᵐ is an m-th Fourier coefficient, driven by the beam's Λ_lᵐ(μ₀) (the ground, Lambertian, acts
 *   at m = 0 alone) and smooth at the poles, so it falls as sinᵐ θ_v sinᵐ θ₀, and their quotient is
 *   smooth: Pₗᵐ(μ) ∝ (1 − μ²)^(m/2) (S. Chandrasekhar, _Radiative Transfer_, Oxford 1950, §48), and
 *   the I row of a mode with an unpolarised beam carries the generalised spherical function
 *   P^l_m0 alike (W. A. de Rooij and C. C. A. H. van der Stap, A&A 131 (1984) 237), so the
 *   correction's modes fall so too. Read as Jᵐ cos mΔφ with Jᵐ held at the last cell
 *   (μ_v = ±15/16) beyond it, the table would spin about the zenith, where Δφ has no value.
 * - On a `thick` table, where J dips below 0 (by up to 49% of J⁰ on the Titan-class fixture's far
 *   side of a forward-peaked point, T14.b's _m ≤ 1 dips below 0_), the read is held at 0 (T14.b's
 *   lean). A `correction` is a difference of either sign, and the sum it makes with Ψ_ms is held
 *   at 0 instead.
 *
 * **The texture** is `rgba32uint`, a 2D array of one layer a band and light slot, layer
 * b × slots + k (`multiScattering`'s order): 16 texels across, one a travel cosine, and
 * 33 × 32 + 1 rows, row a × 32 + s for altitude a and sun cosine s, then the info row. A texel holds
 * six halves in its first three words, two to a word: J⁰'s red and green, J⁰'s blue and G's red,
 * G's green and blue (`pack2x16float`'s order, the first in the low half); and G₂ in its fourth, three
 * 9-bit two's-complement mantissas (red in bits 0–8, green 9–17, blue 18–26) and their shared
 * exponent e in bits 27–31, each channel m·2^(e − 24) ({@link THICK_CURVE_EXPONENT_BIAS}): 2⁻²⁴ to
 * 32,640, each to 1 in 255 of the texel's largest, or to 2⁻²⁵ where that is the larger (its least
 * exponent). G₂ carries the bake's m = 2 with the correction's: |J²| reaches 0.44 of J⁰ on `earth`
 * and 1.37 on `mars` (`reference/vectorGate.json`'s `bakeCurveShareOfJ0`). The shared exponent's
 * rounding moves each channel's J by at most 1 in 255.5 of the texel's largest |G₂| sin² θ_v sin² θ₀,
 * so a weaker channel takes the strongest channel's |J²| ÷ 255.5 against its own J⁰: at most 0.15% of
 * a channel's J⁰ on `earth` and 0.51% on `mars`, with the correction summed in or not
 * (`bakeCurveRoundingOfJ0` and `routedCurveRoundingOfJ0`, against `f32`), a tenth of Design note
 * 10's 5% or less, and the texel stays one load. A band and light is 270,336 B, the bake's figure
 * for `rgba16float` (33 × 32 × 16 texels × 16 B), and the info row 256 B more, so that the lever
 * costs no byte over R08.T14.c's table. Layer 0's info row holds the altitudes as `f32` bits,
 * words 0–32, the light slots a band, word 33, and the kind, word 34: 0 for `thick`, 1 for
 * `correction`.
 *
 * **The read** is linear in each of h, μ₀ and μ_v between the cells' centres about it, held at the
 * table's edges: μ₀ = (s + ½) ÷ 32 and μ_v = (v + ½) ÷ 8 − 1 lie at texel centres, and h between
 * the rows about it, found by bisection over the altitudes. Its band's s is read as the multiple
 * scattering's bands are (`oblate.wgsl`'s `bandTableAt`): linear in ln s between the bands about it,
 * the nearer band beyond them.
 *
 * **Below the horizon.** μ₀ ≤ 0 lies outside the table (T14.b's record, the beam's domain; the
 * correction's too, R08.T14.e), so a source there is read by the analytic term alone, and the two
 * are blended over the table's first μ₀ cell, (0, 1/32]: the table's share is clamp(32 μ₀, 0, 1)
 * (`decision-r08-t13c2-classes.md` §6). On a `thick` table the analytic term takes the rest,
 * 1 − clamp(32 μ₀, 0, 1), and the forward lobes' higher orders (R08.T7, `forwardLobe.ts`) the same
 * share (main, 2026-10-27): the bake's J holds the aureole scattered once more by the smooth part,
 * which the orders' tail also adds, so on the table's side they would count it twice. The orders'
 * in-lobe part, the aureole scattered again within the lobe (T14.b's IMS), is not in J, so the
 * table's side leaves it out: R08.T14.d judges it at Mars's aureole (plan R08's Risks,
 * "Preconditions of R08.T14.d's routing"). On a `correction` table the analytic term and the lobes
 * stay whole at every μ₀, and the correction fades in across the same cell, where its beam is
 * defined, so that a sunset's sky is continuous ({@link thickAnalyticWeight}).
 */

import type { Rgb } from "../../photometry/toneCurve";
import { fromHalfBits, HALF_MAX, toHalfBits } from "../../sky/half";
import { type AtmosphereMedium, densityAt } from "../medium";
import type { MsSourceTable } from "./bake";
import type { PolarisationTable } from "./polarisation";

/** The table's altitudes: the bake's 33 (`bake.ts`'s `THICK_BAKE_GRID`, 64 layers at stride 2). */
export const THICK_ALTITUDES = 33;
/** The table's sun cosines: 32, at (s + ½) ÷ 32. */
export const THICK_SUN_COSINES = 32;
/** The table's travel cosines: 16, at (v + ½) ÷ 8 − 1, the downward half first. */
export const THICK_TRAVEL_COSINES = 16;
/** The bake's azimuthal modes the table takes: m = 0, 1 and 2 (`bake.ts`'s `THICK_BAKE_MODES`). */
export const THICK_SOURCE_MODES = 3;
/**
 * The polarisation correction's azimuthal modes: m = 0, 1 and 2 (`polarisation.ts`'s
 * `POLARISATION_MODES`, held equal by `read.test.ts`; not imported, so that the render thread loads
 * no solver).
 */
export const THICK_CORRECTION_MODES = 3;
/**
 * The table's azimuthal modes: m = 0, 1 and 2, the bake's and the correction's (R08.T14.f and
 * T14.d.1).
 */
export const THICK_MODES = THICK_CORRECTION_MODES;
/** The info row: after the altitudes' and sun cosines' rows. */
export const THICK_INFO_ROW = THICK_ALTITUDES * THICK_SUN_COSINES;
/** The texture's rows: the table's and the info row. */
export const THICK_ROWS = THICK_INFO_ROW + 1;
/** The word of the info row that holds the light slots a band. */
export const THICK_SLOTS_WORD = THICK_ALTITUDES;
/** The word of the info row that holds the table's kind: 0 `thick`, 1 `correction` (R08.T14.f). */
export const THICK_KIND_WORD = THICK_ALTITUDES + 1;
/** The texture's format: four 32-bit words, six halves and G₂'s shared-exponent word. */
export const THICK_TABLE_FORMAT = "rgba32uint";
/**
 * G₂'s exponent bias (the module's remarks): a channel is m·2^(e − 24), e in 0 … 31 and m in
 * −255 … 255, so that the word holds 2⁻²⁴ (a half's least subnormal) to 255·2⁷ = 32,640.
 */
export const THICK_CURVE_EXPONENT_BIAS = 24;
/** The bytes of one texel. */
const TEXEL_BYTES = 16;
/** The words of one texel. */
const TEXEL_WORDS = 4;
/** G₂'s mantissas' bits, the sign's among them, and their largest magnitude. */
const CURVE_BITS = 9;
const CURVE_MANTISSA_MOST = 255;
/** G₂'s largest exponent field. */
const CURVE_EXPONENT_MOST = 31;

/**
 * A band and light slot's table as `rgba16float` would hold it, B: 270,336 (the bake's record; plan
 * R08, Design note 11's J_ms row).
 */
export const THICK_TABLE_BYTES_A_SLOT =
  THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * TEXEL_BYTES;

/** A layer's bytes as allocated, B: the table's and the info row's, 270,592. */
export const THICK_LAYER_BYTES = THICK_ROWS * THICK_TRAVEL_COSINES * TEXEL_BYTES;

/**
 * What a table holds (the module's remarks), sr⁻¹ per unit of the kernels' scattering and of the
 * star's irradiance: `thick`, a bake's J with the correction summed into it, read in place of Ψ_ms
 * from the first sun cosine cell up; or `correction`, the correction alone, read beside Ψ_ms on a
 * thin world.
 */
export type ThickTableKind = "thick" | "correction";

/**
 * A body's thick table, packed: what the kernels' `thickTable` holds and the CPU twin reads
 * (the module's remarks). Structured-cloneable, for the smoke's twin workers.
 */
export interface PackedThickTable {
  readonly kind: ThickTableKind;
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

/**
 * The polarisation correction a table carries (R08.T14.f): one table a latitude band, in the body's
 * order, and the medium whose whole σ_s it is per unit of.
 */
export interface ThickCorrection {
  /** `polarisation.ts`'s `bakePolarisationCorrection`, one a band, each with a slot a light. */
  readonly bands: ReadonlyArray<PolarisationTable>;
  /**
   * The drawn medium, every term's forward delta in, of which the packing's kernel medium is
   * `tables.ts`'s `kernelMedium`: term for term, in the same order.
   */
  readonly medium: AtmosphereMedium;
}

/** The bytes a packed table allocates, B: {@link THICK_LAYER_BYTES} a layer. */
export function thickTableBytes(table: Pick<PackedThickTable, "layers">): number {
  return table.layers * THICK_LAYER_BYTES;
}

/**
 * σ_s per channel at `heightM`, each term's density the kernels' (`densities`' terms') and its
 * scattering `scatterings`' term's: the kernels' own σ_s,k with `densities` alone, the drawn
 * medium's whole σ_s with `scatterings` the drawn medium.
 */
function scatteringAt(
  densities: AtmosphereMedium,
  scatterings: AtmosphereMedium,
  heightM: number,
): Rgb {
  let red = 0;
  let green = 0;
  let blue = 0;
  for (const [i, term] of densities.terms.entries()) {
    const rho = densityAt(term.density, heightM);
    const scattering = scatterings.terms[i]?.scattering ?? [Number.NaN, Number.NaN, Number.NaN];
    red += scattering[0] * rho;
    green += scattering[1] * rho;
    blue += scattering[2] * rho;
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
 * G₂'s word for its three channels (the module's remarks): the least exponent whose mantissas hold
 * the largest, each rounded to the nearest, halves away from 0; 0 for three zeros, and a channel
 * under half of 2⁻²⁴ is 0.
 *
 * @throws RangeError naming the value by `where` for one that is not finite or is past 32,640.
 */
export function packThickCurve(values: Rgb, where: () => string): number {
  let largest = 0;
  for (const value of values) {
    if (!Number.isFinite(value)) {
      throw new RangeError(`a thick table's ${where()} is ${value}, which is not finite`);
    }
    largest = Math.max(largest, Math.abs(value));
  }
  if (largest > CURVE_MANTISSA_MOST * 2 ** (CURVE_EXPONENT_MOST - THICK_CURVE_EXPONENT_BIAS)) {
    throw new RangeError(`a thick table's ${where()} is ${largest}, past its word's 32,640`);
  }
  // The least exponent whose largest mantissa rounds to at most 255, by exact comparisons with
  // powers of two.
  let exponent = 0;
  while (
    exponent < CURVE_EXPONENT_MOST &&
    !(largest < (CURVE_MANTISSA_MOST + 0.5) * 2 ** (exponent - THICK_CURVE_EXPONENT_BIAS))
  ) {
    exponent += 1;
  }
  const scale = 2 ** (exponent - THICK_CURVE_EXPONENT_BIAS);
  const field = (value: number): number => {
    const m = Math.sign(value) * Math.round(Math.abs(value) / scale);
    return (m + (1 << CURVE_BITS)) & ((1 << CURVE_BITS) - 1);
  };
  const [red, green, blue] = values.map(field);
  return (
    ((red ?? 0) |
      ((green ?? 0) << CURVE_BITS) |
      ((blue ?? 0) << (2 * CURVE_BITS)) |
      (exponent << (3 * CURVE_BITS))) >>>
    0
  );
}

/** G₂'s channel `c` of its word (`thickRead.wgsl`'s `thickCurveOf`): exact, m·2^(e − 24). */
export function thickCurveChannel(word: number, c: number): number {
  const field = (word >>> (c * CURVE_BITS)) & ((1 << CURVE_BITS) - 1);
  const mantissa = field >= 1 << (CURVE_BITS - 1) ? field - (1 << CURVE_BITS) : field;
  return mantissa * 2 ** ((word >>> (3 * CURVE_BITS)) - THICK_CURVE_EXPONENT_BIAS);
}

/** A table's grid, as either bake gives it. */
interface GridOf {
  readonly altitudesM: Float64Array;
  readonly sunCosines: Float64Array;
  readonly travelCosines: Float64Array;
  readonly modes: number;
  readonly slots: { readonly length: number };
  readonly band: { readonly gravityRatio: number };
}

/**
 * Checks one bake's bands against the table's grid and against the first band's (or `first`'s)
 * altitudes, light slots and band count.
 *
 * @throws RangeError as {@link packThickTable} refuses them.
 */
function checkBands(
  what: string,
  bands: ReadonlyArray<GridOf>,
  modes: number,
  first: GridOf,
  bandCount: number,
): void {
  if (bands.length !== bandCount) {
    throw new RangeError(`a thick table's ${what} has ${bands.length} bands, not ${bandCount}`);
  }
  const altitudes = first.altitudesM;
  for (const [b, band] of bands.entries()) {
    if (
      band.modes !== modes ||
      !onGrid(band.sunCosines, THICK_SUN_COSINES, THICK_SUN_COSINES, 0) ||
      !onGrid(band.travelCosines, THICK_TRAVEL_COSINES, THICK_TRAVEL_COSINES / 2, -1)
    ) {
      throw new RangeError(`${what} band ${b}'s table is off the thick bake's grid`);
    }
    if (
      band.slots.length !== first.slots.length ||
      band.altitudesM.some((h, a) => h !== altitudes[a])
    ) {
      throw new RangeError(
        `${what} band ${b}'s table has other altitudes or light slots than band 0's`,
      );
    }
  }
}

/**
 * A body's bake, its polarisation correction, or both summed, packed for the kernels (the module's
 * remarks): J⁰, G and G₂ per band and light slot, per unit of the kernels' scattering at each
 * altitude.
 *
 * @param bands - The bake's tables, J in sr⁻¹ per unit of the kernels' scattering, taken as they
 *   are, one a latitude band in the body's order (`bake.ts`'s `thickBakeBands`), each with the same
 *   altitudes and light slots; `null` for a `correction` table, a thin world's.
 * @param kernelMedium - The medium as the kernels read it (`tables.ts`'s `kernelMedium`, or the
 *   drawn medium with its terms' forward deltas taken out), whose scattering the kernels multiply
 *   the read by, and the correction is taken per unit of.
 * @param correction - The polarisation correction, summed into the bake's J where both are given,
 *   or alone; `null` for none, a `thick` table of the bake alone.
 * @throws RangeError for no bake and no correction, no band, a table off the bake's grid
 *   ({@link THICK_ALTITUDES} altitudes ascending from 0, {@link THICK_SUN_COSINES} μ₀ and
 *   {@link THICK_TRAVEL_COSINES} μ_v at their cells' centres, {@link THICK_SOURCE_MODES} modes a
 *   bake and {@link THICK_CORRECTION_MODES} a correction), bands with different altitudes or light
 *   slots, a correction of other bands, s, altitudes or light slots than the bake's, a correction
 *   whose medium has another count of terms than the kernel medium (the terms' order is the
 *   caller's to keep), no light slot, or a J⁰ or G that is not finite or past the half float's
 *   largest, 65,504, or a G₂ not finite or past 32,640.
 */
export function packThickTable(
  bands: ReadonlyArray<MsSourceTable> | null,
  kernelMedium: AtmosphereMedium,
  correction: ThickCorrection | null = null,
): PackedThickTable {
  const kind: ThickTableKind = bands === null ? "correction" : "thick";
  const first: GridOf | undefined = bands === null ? correction?.bands[0] : bands[0];
  if (first === undefined) {
    throw new RangeError(
      bands === null && correction === null
        ? "a thick table holds a bake, a correction or both"
        : "a thick table has at least one band",
    );
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
  const bandCount = bands?.length ?? correction?.bands.length ?? 0;
  if (bands !== null) {
    checkBands("the bake's", bands, THICK_SOURCE_MODES, first, bandCount);
  }
  if (correction !== null) {
    checkBands("the correction's", correction.bands, THICK_CORRECTION_MODES, first, bandCount);
    if (
      correction.medium.terms.length !== kernelMedium.terms.length ||
      correction.bands.some(
        (band, b) =>
          bands !== null &&
          Math.abs(band.band.gravityRatio - (bands[b]?.band.gravityRatio ?? Number.NaN)) >
            1e-12 * band.band.gravityRatio,
      )
    ) {
      throw new RangeError(
        "a thick table's correction is of another medium or other bands than its bake",
      );
    }
  }
  const layers = bandCount * slots;
  const layerWords = THICK_ROWS * THICK_TRAVEL_COSINES * TEXEL_WORDS;
  const texels = new Uint32Array(layers * layerWords);
  const scattering = Array.from(altitudes, (h) => scatteringAt(kernelMedium, kernelMedium, h));
  const whole =
    correction === null
      ? scattering
      : Array.from(altitudes, (h) => scatteringAt(kernelMedium, correction.medium, h));
  const sunSines = sinesOf(first.sunCosines);
  const travelSines = sinesOf(first.travelCosines);
  const sourceStride = THICK_SOURCE_MODES * 3;
  const correctionStride = THICK_CORRECTION_MODES * 3;
  const curve: [number, number, number] = [0, 0, 0];
  for (let b = 0; b < bandCount; b += 1) {
    for (let k = 0; k < slots; k += 1) {
      const base = (b * slots + k) * layerWords;
      const source = bands?.[b]?.slots[k]?.values;
      const delta = correction?.bands[b]?.slots[k]?.values;
      for (let a = 0; a < THICK_ALTITUDES; a += 1) {
        const sigma = scattering[a] ?? [Number.NaN, Number.NaN, Number.NaN];
        const sigmaWhole = whole[a] ?? [Number.NaN, Number.NaN, Number.NaN];
        for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
          for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
            const cell = (a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v;
            const sines = (sunSines[s] ?? Number.NaN) * (travelSines[v] ?? Number.NaN);
            const halves = [0, 0, 0, 0, 0, 0];
            const where =
              (what: string, c: number | null): (() => string) =>
              () =>
                `${what} at band ${b}, slot ${k}, altitude ${a}, μ₀ ${s}, μ_v ${v}${c === null ? "" : `, channel ${c}`}`;
            for (let c = 0; c < 3; c += 1) {
              const scatteringPerM = sigma[c] ?? Number.NaN;
              // The correction's ΔJ is per unit of the whole σ_s, the table per unit of σ_s,k.
              const wholeShare =
                scatteringPerM > 0 ? (sigmaWhole[c] ?? Number.NaN) / scatteringPerM : 0;
              const from = cell * sourceStride + c;
              const at = cell * correctionStride + c;
              const j0 =
                (source === undefined ? 0 : (source[from] ?? Number.NaN)) +
                (delta === undefined ? 0 : wholeShare * (delta[at] ?? Number.NaN));
              const j1 =
                (source === undefined ? 0 : (source[from + 3] ?? Number.NaN)) +
                (delta === undefined ? 0 : wholeShare * (delta[at + 3] ?? Number.NaN));
              const j2 =
                (source === undefined ? 0 : (source[from + 6] ?? Number.NaN)) +
                (delta === undefined ? 0 : wholeShare * (delta[at + 6] ?? Number.NaN));
              halves[c] = halfOf(j0, where("J⁰", c));
              halves[3 + c] = halfOf(j1 / sines, where("G", c));
              curve[c] = j2 / (sines * sines);
            }
            const texel = base + cell * TEXEL_WORDS;
            texels[texel] = ((halves[0] ?? 0) | ((halves[1] ?? 0) << 16)) >>> 0;
            texels[texel + 1] = ((halves[2] ?? 0) | ((halves[3] ?? 0) << 16)) >>> 0;
            texels[texel + 2] = ((halves[4] ?? 0) | ((halves[5] ?? 0) << 16)) >>> 0;
            texels[texel + 3] = packThickCurve(curve, where("G₂", null));
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
  texels[info + THICK_KIND_WORD] = kind === "correction" ? 1 : 0;
  return {
    kind,
    widthTexels: THICK_TRAVEL_COSINES,
    heightTexels: THICK_ROWS,
    layers,
    slots,
    bands: bandCount,
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

/** The analytic term's share of a source's multiple scattering at sun cosine `muSun` on a `thick` table (the module's remarks). */
export function thickAnalyticShare(muSun: number): number {
  return 1 - Math.min(Math.max(muSun * THICK_SUN_COSINES, 0), 1);
}

/**
 * The analytic term's weight in a source's multiple scattering at sun cosine `muSun`, and the
 * forward lobes' higher orders' (`thickRead.wgsl`'s `thickAnalyticWeight`): its share on a `thick`
 * table, whole beside a `correction` (the module's remarks).
 */
export function thickAnalyticWeight(kind: ThickTableKind, muSun: number): number {
  let weight: number;
  switch (kind) {
    case "thick":
      weight = thickAnalyticShare(muSun);
      break;
    case "correction":
      weight = 1;
      break;
  }
  return weight;
}

/** J⁰, G and G₂ at a point, per channel. */
export interface ThickSource {
  readonly level: [number, number, number];
  readonly slope: [number, number, number];
  readonly curve: [number, number, number];
}

/** A zeroed {@link ThickSource}, for a reader's scratch. */
export function thickSourceScratch(): ThickSource {
  return { level: [0, 0, 0], slope: [0, 0, 0], curve: [0, 0, 0] };
}

/** Texel (a, s, v) of `layer`, its six halves and G₂ added into `out` at weight `w`. */
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
  const curve = table.texels[at + 3] ?? 0;
  out.level[0] += w * fromHalfBits(x & 0xffff);
  out.level[1] += w * fromHalfBits(x >>> 16);
  out.level[2] += w * fromHalfBits(y & 0xffff);
  out.slope[0] += w * fromHalfBits(y >>> 16);
  out.slope[1] += w * fromHalfBits(z & 0xffff);
  out.slope[2] += w * fromHalfBits(z >>> 16);
  if (curve !== 0) {
    out.curve[0] += w * thickCurveChannel(curve, 0);
    out.curve[1] += w * thickCurveChannel(curve, 1);
    out.curve[2] += w * thickCurveChannel(curve, 2);
  }
}

/** A {@link ThickSource}'s every channel set to 0. */
function clear(out: ThickSource): void {
  out.level.fill(0);
  out.slope.fill(0);
  out.curve.fill(0);
}

/**
 * One layer's J⁰, G and G₂ at `row`, sun cosine `muSun` and travel cosine `muTravel`, added into
 * `out` at weight `weight` (`thickRead.wgsl`'s `thickLayerAt`): linear in each between the cells'
 * centres about it, held at the edges.
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
 * J⁰, G and G₂ at h\* `scaledHeightM` in light slot `slot`'s layers, read at the bands about s as
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
  clear(out);
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
 * `out`: J⁰ + G·h_⊥ + G₂·(2h_⊥² − (1 − μ_v²)(1 − μ₀²)) (the module's remarks), not held at 0, which
 * {@link thickMultipleInto} does by the table's kind.
 *
 * @param muSun - The source's zenith cosine μ₀ at the sample.
 * @param muTravel - The view's travel cosine μ_v, −(v·n).
 * @param horizontal - h_⊥ = v·s − (v·n)(s·n): the view's and the source's horizontal parts' dot product.
 */
export function thickJInto(
  source: ThickSource,
  muSun: number,
  muTravel: number,
  horizontal: number,
  out: [number, number, number],
): void {
  const crossed = (1 - muTravel * muTravel) * (1 - muSun * muSun);
  const harmonic = 2 * horizontal * horizontal - crossed;
  for (let c = 0; c < 3; c += 1) {
    out[c] =
      (source.level[c] ?? 0) +
      (source.slope[c] ?? 0) * horizontal +
      (source.curve[c] ?? 0) * harmonic;
  }
}

/**
 * A source's multiple scattering as `thickRead.wgsl`'s `thickMultipleAt` forms it, into `multiple`,
 * which holds Hillaire's term (Ψ_ms, sr⁻¹ per unit of scattering) at full weight, from the table's J
 * at the source (`thickJInto`'s): on a `thick` table Ψ_ms at the analytic term's share and J, held at
 * 0, at the rest; beside a `correction`, Ψ_ms whole and J at the table's share, the sum held at 0
 * (the module's remarks).
 */
export function thickMultipleInto(
  kind: ThickTableKind,
  muSun: number,
  j: Readonly<[number, number, number]>,
  multiple: [number, number, number],
): void {
  const analytic = thickAnalyticShare(muSun);
  for (let c = 0; c < 3; c += 1) {
    const read = j[c] ?? 0;
    switch (kind) {
      case "thick":
        multiple[c] = analytic * (multiple[c] ?? 0) + (1 - analytic) * Math.max(read, 0);
        break;
      case "correction":
        multiple[c] = Math.max((multiple[c] ?? 0) + (1 - analytic) * read, 0);
        break;
    }
  }
}
