/**
 * Normal gravity on a body's level spheroid and the slicing of its atmosphere's tables (plan R08,
 * R08.T3.d, Design note 17): what makes one medium serve every latitude of an oblate body.
 *
 * @remarks
 * A hydrostatic atmosphere under one T(p) stands at heights proportional to 1 ÷ g(φ), so the medium
 * is built once at g_ref = √(g_e g_p) ({@link referenceGravity}) and read at the gravity-scaled
 * height h\* = s h, s = g(φ) ÷ g_ref, optical depth from a table being divided by s: the U.S.
 * Standard Atmosphere 1976's geopotential height. The transmittance table is a family of slices
 * over κ = s R_α ÷ R_ref, R_α the radius of curvature along the looked-up ray's azimuth
 * ({@link directionalCurvatureRadiusM}) and R_ref the per-planet tables' ground, R05's
 * `tableRadiusM`; the multiple-scattering and baked tables are a family of bands over s
 * ({@link oblateSlicing}). Earth-class bodies, and every body without a rotation section, take one
 * slice and one band.
 *
 * g(φ) is the normal gravity of the level ellipsoid of revolution (a, a, c) that carries the
 * body's GM and turns at ω, by Somigliana's closed form, which holds for any flattening (Heiskanen
 * and Moritz 1967, _Physical Geodesy_, §2-7 and 2-8; NIMA TR8350.2, Third Edition, 4 July 1997,
 * Amendment 1, 3 January 2000, eq. 4-1). Three effects are not modelled (Design note 17): zonal
 * winds' 2ωu + u²/a, the real 1-bar surface's departure from the best-fit spheroid, and T(p)
 * varying with latitude.
 *
 * Every figure is taken as a level surface of its rotation alone, which the wire's `FigureLawDto`
 * says is not always so:
 *
 * - `rotational_and_tidal`: the flattening f carries the primary's tide while m is the spin's
 *   alone, so Clairaut's f + f\* = 5m ÷ 2 (Heiskanen and Moritz 1967, eq. 2-99) understates the
 *   gravity's rise to the pole by about 15m ÷ 4, and reverses it (g_p < g_e) where f > 5m ÷ 2,
 *   about C ÷ Ma² > 0.34 under Darwin–Radau. It is under 0.3% in s while m < 2 × 10⁻³, and about
 *   ±2.7% for a synchronous inflated hot Saturn;
 * - `sphere`: a spinning sphere is rigid, not level, and its gravity rises to the pole by about m,
 *   not 5m ÷ 2;
 * - `capped`: a flattening held at its cap is not the level one for its spin.
 */

import { GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 } from "../../lib/system/constants";
import type { SpheroidFigure } from "./hillaire";

/**
 * A body's level spheroid: the ellipsoid of revolution (a, a, c) about its spin axis that is a
 * surface of constant gravity potential for its mass and spin (Heiskanen and Moritz 1967, §2-7).
 *
 * @remarks
 * a and c are R07's `BodyFigure` (`view/terrain/planet.ts`, from the wire's `BodyFigureDto`); GM is
 * {@link GRAVITATIONAL_CONSTANT_M3_PER_KG_S2} times the record's `mass_kg` ({@link bodyGravity});
 * ω is the spin rate at the scene time of the record's rotation law (`lib/system/rotation.ts`'s
 * `spinRateAt`), or R05 Design note 14's test-planet rate for R05's Earth
 * (`view/spike/rotation.ts`).
 */
export interface LevelSpheroid {
  /** a, m. */
  readonly equatorialRadiusM: number;
  /** c, m, in (0, a]. */
  readonly polarRadiusM: number;
  /** GM, m³ s⁻². */
  readonly gmM3S2: number;
  /** ω, rad s⁻¹; only its square enters. */
  readonly angularVelocityRadS: number;
}

/**
 * The step of the transmittance slices in ln κ (Design note 17): an interpolation error under 0.1%
 * in grazing optical depth.
 *
 * @remarks
 * Computed, not measured; R08.T12.d's spheroid mode and T12.b's Saturn-class case measure it. A
 * world whose per-planet tables would pass the 2 MB row widens it to 0.3 (Design note 11).
 */
export const KAPPA_STEP = 0.15;

/**
 * The step of the latitude bands in ln s (Design note 17), and the span of ln s one band serves.
 *
 * @remarks
 * Computed, not measured, as {@link KAPPA_STEP} is. Widened to 0.15 over the 2 MB row, after
 * {@link KAPPA_STEP} (Design note 11), which raises the one-band span with it.
 *
 * Bands between two neighbours are interpolated, with an error of second order in the step; one
 * band at s = 1 is wrong in the column at the extremes by up to ±½ ln(s_max ÷ s_min), ±2.5% on the
 * ice giants and ±5% at the span's limit, which no gate yet measures. Design note 17 gives the ice
 * giants one band; whether the one-band span should instead be {@link ONE_SLICE_BELOW}'s, giving
 * them two, is raised for a ruling (R08's Risks).
 */
export const BAND_STEP = 0.1;

/**
 * The span of ln κ, ln(κ_max ÷ κ_min), under which one slice and one band serve (Design note 17).
 *
 * @remarks
 * Grazing optical depth goes as √R (Chapman 1931, Proc. Phys. Soc. 43, 483), so one slice is wrong
 * in it by about a quarter of the span, 0.5% here, a little more since κ = 1 sits about f ÷ 6 below
 * the span's centre in ln κ: within the 1% of Design note 10's 5% that geometry may take.
 */
export const ONE_SLICE_BELOW = 0.02;

/** The slices and bands of a body's tables (Design note 17). */
export interface OblateSlicing {
  /** κ_k of the transmittance slices, ascending, Δln κ ≤ {@link KAPPA_STEP}; `[1]` for one. */
  readonly kappa: Float64Array;
  /** s_b of the latitude bands, ascending, Δln s ≤ {@link BAND_STEP}; `[1]` for one. */
  readonly bandGravityRatio: Float64Array;
}

/** e′ below which q₀ and q₀′ are summed as series, where their closed forms cancel. */
const SERIES_BELOW_E_PRIME = 0.25;

/** Terms of the q-ratio series: e′² < 1/16, so 16 terms leave under 10⁻¹⁹. */
const SERIES_TERMS = 16;

/**
 * Latitudes from the equator to the pole over which {@link oblateSlicing} finds the ranges of κ and
 * s, both ends included.
 */
const RANGE_SCAN_STEPS = 1_024;

/** The figure's radii a and c, m, checked. */
function radiiOf(figure: SpheroidFigure): { readonly a: number; readonly c: number } {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  if (!(Number.isFinite(a) && Number.isFinite(c) && c > 0 && c <= a)) {
    throw new RangeError(`a level spheroid needs 0 < c ≤ a, got a = ${a} m, c = ${c} m`);
  }
  return { a, c };
}

/**
 * e′ q₀′ ÷ q₀, the one function of the second eccentricity e′ = √(a² − c²) ÷ c (Heiskanen and
 * Moritz 1967, eq. 2-71) that Somigliana's normal gravity at the equator and the pole needs.
 *
 * @remarks
 * q₀ = ½[(1 + 3 ÷ e′²) arctan e′ − 3 ÷ e′] (eq. 2-58) and q₀′ = 3(1 + 1 ÷ e′²)(1 − arctan e′ ÷ e′)
 * − 1 (eq. 2-67; NIMA TR8350.2 eqs. 4-12 and 4-13). Both
 * vanish as e′ → 0, q₀ as e′³ and q₀′ as e′², so their closed forms cancel catastrophically for a
 * near-sphere. Below {@link SERIES_BELOW_E_PRIME} they are summed from arctan's Maclaurin series,
 * term by term (derived here): q₀ = Σ (−1)^(k+1) 2k e′^(2k+1) ÷ ((2k + 1)(2k + 3)) and
 * q₀′ = Σ (−1)^(k+1) 6 e′^(2k) ÷ ((2k + 1)(2k + 3)), k ≥ 1, whose leading terms are the familiar
 * 2e′³ ÷ 15 and 2e′² ÷ 5. Their ratio tends to 3 at the sphere.
 */
function qRatio(ePrime: number): number {
  if (ePrime >= SERIES_BELOW_E_PRIME) {
    const atan = Math.atan(ePrime);
    const e2 = ePrime * ePrime;
    const q0 = 0.5 * ((1 + 3 / e2) * atan - 3 / ePrime);
    const q0Prime = 3 * (1 + 1 / e2) * (1 - atan / ePrime) - 1;
    return (ePrime * q0Prime) / q0;
  }
  // Both series divided by e′³, in y = e′², summed from the smallest term (Horner's rule).
  const y = ePrime * ePrime;
  let numerator = 0;
  let denominator = 0;
  for (let k = SERIES_TERMS; k >= 1; k -= 1) {
    const sign = k % 2 === 1 ? 1 : -1;
    const d = (2 * k + 1) * (2 * k + 3);
    numerator = numerator * y + (sign * 6) / d;
    denominator = denominator * y + (sign * 2 * k) / d;
  }
  return numerator / denominator;
}

/** Normal gravity at the equator and the pole of a level spheroid, m s⁻², with its radii. */
interface PoleAndEquator {
  readonly a: number;
  readonly c: number;
  /** γ_e = γ_a. */
  readonly equatorialMS2: number;
  /** γ_p = γ_b. */
  readonly polarMS2: number;
}

/**
 * γ_e and γ_p of a level spheroid (Heiskanen and Moritz 1967, §2-8, whose b is c here):
 * γ_a = GM ÷ (ac) × (1 − m − m e′q₀′ ÷ 6q₀) (eq. 2-73) and γ_b = GM ÷ a² × (1 + m e′q₀′ ÷ 3q₀)
 * (eq. 2-74), with m = ω²a²c ÷ GM (eq. 2-70).
 */
function poleAndEquator(body: LevelSpheroid): PoleAndEquator {
  const { a, c } = radiiOf(body);
  const gm = body.gmM3S2;
  const omega = body.angularVelocityRadS;
  if (!(Number.isFinite(gm) && gm > 0 && Number.isFinite(omega))) {
    throw new RangeError(`a level spheroid needs GM > 0 and a finite ω, got ${gm} and ${omega}`);
  }
  const ePrime = Math.sqrt((a - c) * (a + c)) / c;
  const m = (omega * omega * a * a * c) / gm;
  const ratio = qRatio(ePrime);
  const equatorialMS2 = (gm / (a * c)) * (1 - m - (m * ratio) / 6);
  const polarMS2 = (gm / (a * a)) * (1 + (m * ratio) / 3);
  if (!(equatorialMS2 > 0)) {
    throw new RangeError(`ω = ${omega} rad/s spins the equator past breakup: γ_e ≤ 0`);
  }
  return { a, c, equatorialMS2, polarMS2 };
}

/** Somigliana's closed form at geodetic latitude φ from γ_e and γ_p. */
function somigliana(g: PoleAndEquator, latitudeRad: number): number {
  const cos = Math.cos(latitudeRad);
  const sin = Math.sin(latitudeRad);
  const acos2 = g.a * cos * cos;
  const csin2 = g.c * sin * sin;
  return (acos2 * g.equatorialMS2 + csin2 * g.polarMS2) / Math.sqrt(g.a * acos2 + g.c * csin2);
}

/**
 * Somigliana's closed-form normal gravity on a level spheroid at geodetic latitude φ, m s⁻² (Design
 * note 17).
 *
 * @remarks
 * γ(φ) = (a γ_e cos²φ + c γ_p sin²φ) ÷ √(a² cos²φ + c² sin²φ) (Heiskanen and Moritz 1967, eq.
 * 2-78; NIMA TR8350.2 eq. 4-1 in the form γ_e (1 + k sin²φ) ÷ √(1 − e² sin²φ)), exact for any
 * flattening. On WGS 84 it gives TR8350.2's γ_e = 9.7803253359 and γ_p = 9.8321849378 m s⁻²
 * (Table 3.4).
 *
 * @throws RangeError for radii not 0 < c ≤ a, a GM that is not positive, an ω that is not finite,
 *   or a spin so fast that the equator's gravity is not positive.
 */
export function normalGravity(body: LevelSpheroid, geodeticLatitudeRad: number): number {
  return somigliana(poleAndEquator(body), geodeticLatitudeRad);
}

/**
 * g_ref = √(g_e g_p), m s⁻²: the gravity the column is built at (R08.T3.a), so that s = g(φ) ÷
 * g_ref spans the same factor either side of 1 from the equator to the pole (Design note 17).
 *
 * @throws RangeError as {@link normalGravity}.
 */
export function referenceGravity(body: LevelSpheroid): number {
  const g = poleAndEquator(body);
  return Math.sqrt(g.equatorialMS2 * g.polarMS2);
}

/** The principal radii of curvature at geodetic latitude φ, m: meridional M and prime-vertical N. */
function principalRadii(
  a: number,
  c: number,
  latitudeRad: number,
): { readonly m: number; readonly n: number } {
  const e2 = ((a - c) * (a + c)) / (a * a);
  const sin = Math.sin(latitudeRad);
  const w2 = 1 - e2 * sin * sin;
  const w = Math.sqrt(w2);
  return { m: (a * (1 - e2)) / (w2 * w), n: a / w };
}

/**
 * The radius of curvature of the datum along a ray of azimuth α from north, m: the radius of the
 * osculating sphere a ray in that vertical plane is marched in (Design note 17).
 *
 * @remarks
 * By Euler's theorem 1 ÷ R_α = cos²α ÷ M + sin²α ÷ N, with the meridional M = a(1 − e²) ÷
 * (1 − e² sin²φ)^(3/2) (NIMA TR8350.2 §7.4's R_M) and the prime-vertical N = a ÷ √(1 − e² sin²φ)
 * (TR8350.2 eq. 4-15), both Heiskanen and Moritz 1967's eq. 2-81, as R05's `geodeticOf` takes
 * them: the local centre of curvature in the ray's plane, as Syndergaard 1998 (J. Atmos. Sol.-Terr.
 * Phys. 60, 171–180, doi:10.1016/S1364-6826(97)00056-4) corrects limb sounding for oblateness. It
 * is M along the meridian, N along the prime vertical, and a² ÷ c in every direction at the pole
 * (eq. 2-82). It depends on the figure alone, so a {@link LevelSpheroid} or R07's `BodyFigure`
 * serves.
 *
 * @throws RangeError for radii not 0 < c ≤ a.
 */
export function directionalCurvatureRadiusM(
  figure: SpheroidFigure,
  geodeticLatitudeRad: number,
  azimuthRad: number,
): number {
  const { a, c } = radiiOf(figure);
  const { m, n } = principalRadii(a, c, geodeticLatitudeRad);
  const cos = Math.cos(azimuthRad);
  const sin = Math.sin(azimuthRad);
  return (m * n) / (n * cos * cos + m * sin * sin);
}

/** One slice and one band, each the reference itself. */
function oneSliceAndBand(): OblateSlicing {
  return { kappa: Float64Array.of(1), bandGravityRatio: Float64Array.of(1) };
}

/** `count` values from `low` to `high`, both exact, spaced evenly in their logarithm. */
function logSpaced(low: number, high: number, count: number): Float64Array {
  const values = new Float64Array(count);
  const span = Math.log(high / low);
  for (let i = 0; i < count; i += 1) {
    values[i] = i === count - 1 ? high : low * Math.exp((span * i) / (count - 1));
  }
  return values;
}

/**
 * The κ slices and latitude bands a body's tables are built in (Design note 17).
 *
 * @remarks
 * κ = s R_α ÷ R_ref over every latitude and azimuth, and s = g(φ) ÷ g_ref over every latitude, are
 * ranged by a scan of {@link RANGE_SCAN_STEPS} latitudes from the equator to the pole, R_α taking
 * its extremes M and N along the meridian and the prime vertical. On a body whose gravity rises
 * towards the pole the extremes are the equator's s_e M_e and the pole's s_p a² ÷ c.
 *
 * - While ln(κ_max ÷ κ_min) ≤ {@link ONE_SLICE_BELOW}, one slice at κ = 1 and one band at s = 1,
 *   R05's per-planet tables as they are, serve.
 * - Otherwise the slices number 1 + ⌈ln(κ_max ÷ κ_min) ÷ {@link KAPPA_STEP}⌉, from κ_min to κ_max
 *   evenly in ln κ.
 * - The bands are one at s = 1 while ln(s_max ÷ s_min) ≤ {@link BAND_STEP}, and otherwise
 *   1 + ⌈ln(s_max ÷ s_min) ÷ {@link BAND_STEP}⌉ from s_min to s_max evenly in ln s.
 *
 * That is 1 and 1 for Earth, 2 and 1 for the ice giants, 4 and 3 for Jupiter and 5 and 4 for
 * Saturn: the counts of Design note 17. The one-band rule is this task's reading of the note,
 * which gives the step and the ice giants' single band but not the threshold between them; its
 * error is {@link BAND_STEP}'s to state.
 *
 * @param referenceRadiusM - R_ref, m: the per-planet tables' ground, R05's `tableRadiusM(figure)`.
 * @throws RangeError as {@link normalGravity}, or for an R_ref that is not finite and positive.
 */
export function oblateSlicing(body: LevelSpheroid, referenceRadiusM: number): OblateSlicing {
  if (!(Number.isFinite(referenceRadiusM) && referenceRadiusM > 0)) {
    throw new RangeError(`R_ref must be finite and positive, got ${referenceRadiusM} m`);
  }
  const g = poleAndEquator(body);
  const gRef = Math.sqrt(g.equatorialMS2 * g.polarMS2);
  let kappaMin = Number.POSITIVE_INFINITY;
  let kappaMax = 0;
  let sMin = Number.POSITIVE_INFINITY;
  let sMax = 0;
  for (let i = 0; i <= RANGE_SCAN_STEPS; i += 1) {
    const latitudeRad = (i / RANGE_SCAN_STEPS) * (Math.PI / 2);
    const s = somigliana(g, latitudeRad) / gRef;
    const { m, n } = principalRadii(g.a, g.c, latitudeRad);
    kappaMin = Math.min(kappaMin, (s * Math.min(m, n)) / referenceRadiusM);
    kappaMax = Math.max(kappaMax, (s * Math.max(m, n)) / referenceRadiusM);
    sMin = Math.min(sMin, s);
    sMax = Math.max(sMax, s);
  }
  const kappaSpan = Math.log(kappaMax / kappaMin);
  if (kappaSpan <= ONE_SLICE_BELOW) {
    return oneSliceAndBand();
  }
  const sSpan = Math.log(sMax / sMin);
  return {
    kappa: logSpaced(kappaMin, kappaMax, 1 + Math.ceil(kappaSpan / KAPPA_STEP)),
    bandGravityRatio:
      sSpan <= BAND_STEP
        ? Float64Array.of(1)
        : logSpaced(sMin, sMax, 1 + Math.ceil(sSpan / BAND_STEP)),
  };
}

/** What a body's record gives its gravity (Design note 17; R08.T10.a assembles it). */
export interface BodyGravityInput {
  /** The datum spheroid: R07's `BodyFigure`. */
  readonly figure: SpheroidFigure;
  /** The record's `mass_kg`, kg. */
  readonly massKg: number;
  /**
   * ω at the scene time, rad s⁻¹ (`spinRateAt` of the record's rotation law), or `null` where the
   * record has no rotation section.
   */
  readonly angularVelocityRadS: number | null;
  /** The bulk section's surface gravity, m s⁻²: g everywhere when ω is `null`. */
  readonly bulkGravityMS2: number;
}

/** A body's gravity as its medium is built and read at (Design note 17). */
export interface BodyGravity {
  /**
   * The level spheroid, or `null` for a body with no rotation section, whose gravity is taken as
   * its bulk gravity at every latitude.
   */
  readonly spheroid: LevelSpheroid | null;
  /** g_ref, m s⁻²: {@link referenceGravity}, or the bulk gravity with no spheroid. */
  readonly referenceGravityMS2: number;
  readonly slicing: OblateSlicing;
}

/**
 * A body's gravity from its record: its level spheroid with GM = G × `mass_kg`, g_ref and the
 * slicing, or, with no rotation section, the bulk gravity everywhere and one slice and one band.
 *
 * @remarks
 * The wire's figure is `not_modelled` wherever its rotation is (`BodySummaryDto.figure`), so a
 * generated body with a figure has a rotation, and the fallback serves fixtures and hand-built
 * bodies. It takes no account of the figure's flattening.
 *
 * @param referenceRadiusM - R_ref, m: R05's `tableRadiusM(figure)`.
 * @throws RangeError where ω is given, for a mass that is not finite and positive and as
 *   {@link oblateSlicing}; where ω is `null`, for a bulk gravity that is not finite and positive.
 */
export function bodyGravity(input: BodyGravityInput, referenceRadiusM: number): BodyGravity {
  const omega = input.angularVelocityRadS;
  if (omega === null) {
    const g = input.bulkGravityMS2;
    if (!(Number.isFinite(g) && g > 0)) {
      throw new RangeError(`a bulk gravity must be finite and positive, got ${g} m/s²`);
    }
    return { spheroid: null, referenceGravityMS2: g, slicing: oneSliceAndBand() };
  }
  if (!(Number.isFinite(input.massKg) && input.massKg > 0)) {
    throw new RangeError(`a body's mass must be finite and positive, got ${input.massKg} kg`);
  }
  const spheroid: LevelSpheroid = {
    equatorialRadiusM: input.figure.equatorialRadiusM,
    polarRadiusM: input.figure.polarRadiusM,
    gmM3S2: GRAVITATIONAL_CONSTANT_M3_PER_KG_S2 * input.massKg,
    angularVelocityRadS: omega,
  };
  return {
    spheroid,
    referenceGravityMS2: referenceGravity(spheroid),
    slicing: oblateSlicing(spheroid, referenceRadiusM),
  };
}

/**
 * s = g(φ) ÷ g_ref at geodetic latitude φ: the factor heights are scaled by and table optical
 * depths divided by (Design note 17); 1 everywhere for a body with no spheroid.
 *
 * @throws RangeError as {@link normalGravity}.
 */
export function gravityRatio(gravity: BodyGravity, geodeticLatitudeRad: number): number {
  return gravity.spheroid === null
    ? 1
    : normalGravity(gravity.spheroid, geodeticLatitudeRad) / gravity.referenceGravityMS2;
}
