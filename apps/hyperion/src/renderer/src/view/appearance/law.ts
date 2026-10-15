/**
 * The photometric law a body is shaded with, and its phase factor f tabulated for the GPU (plan
 * R07, Design note 5; plan R10, R10.T10.b and Design note 8).
 *
 * @remarks
 * I/F = A · f(α) · [L(α) · 2μ₀ ÷ (μ₀ + μ) + (1 − L(α)) μ₀] per display channel, f(0) = 1. L(α) is
 * the Lommel–Seeliger share at phase α: L(0), the law's {@link PhotometricLaw.lommelSeeligerShare},
 * at every phase for a law without a curve, else L(0) times its curve's profile
 * ({@link limbShareAt}; decision-r07-t8b). The phase factor reproduces a measured V curve Φ_t
 * raised to a per-channel exponent s: f(α) = Φ_t(α)^s ÷ Φ_shape(α; L), Φ_shape taken with L(α), so
 * that the disc-integrated phase function is Φ_t^s wherever f is not clamped, whatever L(α) does,
 * and the curve moves only the limb profile. It is clamped at {@link PHASE_F_CLAMP} (a Lambert
 * crescent vanishes faster than a cloudy one: Venus's f would reach 61 at 170°) and held at its
 * value at the template's last valid phase beyond it. The table samples f every 0.5° from 0° to
 * 180° in `f32`, with L(α) beside it (the row's alpha), and both the TypeScript reference and the
 * shader interpolate them linearly, so the two read one f and one L.
 */
import type { Rgb } from "../photometry/toneCurve";
import { shapePhase } from "./shapes";
import { PHASE_TEMPLATES } from "./templates";

/**
 * Which measured V phase curve a law's f is fitted to.
 *
 * @remarks
 * The Solar System analogues of Design note 5's rule (pressure and cloud fraction); the last three
 * are provisional stand-ins (`templates.ts`) and are labelled; `lambert` is the Lambert sphere of
 * the provisional photometry a body without a photometric section takes (R07.T2.a).
 */
export type PhaseTemplateId =
  | "moon"
  | "mercury"
  | "mars"
  | "venus"
  | "earth"
  | "jupiter"
  | "saturn"
  | "uranus"
  | "neptune"
  | "airless-ice"
  | "snowball"
  | "magma"
  | "lambert";

/**
 * A profile of the Lommel–Seeliger share over phase, L(α) ÷ L(0): McEwen 1996's lunar polynomial
 * ({@link mcewenLunarProfile}).
 */
export type LommelSeeligerCurve = "mcewen-1996";

/** A body's lunar-Lambert law with a fitted phase factor (Design note 5). */
export interface PhotometricLaw {
  /** The albedo scale A per display channel (r, g, b); p = A [L(0) + ⅔(1 − L(0))]. */
  readonly a: Rgb;
  /**
   * L(0), the Lommel–Seeliger share at opposition, 0 (Lambert) to 1; the share at every phase where
   * the law has no {@link PhotometricLaw.lommelSeeligerCurve}.
   */
  readonly lommelSeeligerShare: number;
  /**
   * The profile L(α) ÷ L(0) follows, its template's (`templates.ts`); absent, L is constant at
   * {@link PhotometricLaw.lommelSeeligerShare} (decision-r07-t8b).
   */
  readonly lommelSeeligerCurve?: LommelSeeligerCurve;
  /** Whose V curve f is fitted to. */
  readonly template: PhaseTemplateId;
  /** s per display channel (r, g, b) in Φ_t(α)^s; 1 reproduces the template's V curve. */
  readonly phaseExponent: Rgb;
}

/** The ceiling on the phase factor f (Design note 5). */
export const PHASE_F_CLAMP = 4;

/** The table's step, rad: 0.5°. */
export const PHASE_TABLE_STEP_RAD = Math.PI / 360;

/** The table's samples, 0° to 180° inclusive. */
export const PHASE_TABLE_SAMPLES = 361;

/**
 * McEwen's lunar L(α) polynomial, L(α) = 1 + A α + B α² + C α³ with α in degrees: A, B and C
 * (McEwen 1996, "A precise lunar photometric function", LPSC XXVII, 841, Table 1 on p. 842, the
 * same at 0.56 and 0.76 µm, fitted to Galileo SSI images at 19.5°–101° with L(0) = 1 imposed; the
 * coefficients of USGS ISIS's `LunarLambertMcEwen.cpp`; decision-r07-t8b). The surface crate's
 * `MCEWEN_LUNAR_L` (`material/properties.rs`) holds the same three.
 *
 * McEwen 1996 prints the law without the factor 2 on its Lommel–Seeliger term (p. 841); the
 * coefficients are applied here in the 2L form, 2 L μ₀ ÷ (μ₀ + μ), of McEwen 1991 and of McEwen's
 * own ISIS `LunarLambertMcEwen.cpp` (1995), as R10's Design note 8 and the surface crate have it.
 */
export const MCEWEN_LUNAR_L: readonly [number, number, number] = [-0.019, 2.42e-4, -1.46e-6];

const DEG_PER_RAD = 180 / Math.PI;

/**
 * McEwen's lunar profile L(α) ÷ L(0) at phase α: 1 at opposition, falling monotonically to 0 at
 * 103.95° (the polynomial's one real root, computed; decision-r07-t8b's 103.9° rounds it),
 * extrapolated past McEwen's last image at 101° and held at 0 beyond its root, where the polynomial
 * is negative. NaN for a NaN phase, as the surface crate's `mcewen_lunar_profile`.
 *
 * @param alphaRad - Phase angle, rad; its magnitude is taken.
 */
export function mcewenLunarProfile(alphaRad: number): number {
  const a = Math.abs(alphaRad) * DEG_PER_RAD;
  const [c1, c2, c3] = MCEWEN_LUNAR_L;
  const l = 1 + a * (c1 + a * (c2 + a * c3));
  // A comparison, not `Math.max`, so that a NaN phase stays NaN.
  return l < 0 ? 0 : l;
}

/** Each curve's profile L(α) ÷ L(0), by its name. */
const PROFILES: Readonly<Record<LommelSeeligerCurve, (alphaRad: number) => number>> = {
  "mcewen-1996": mcewenLunarProfile,
};

/**
 * L(α), a law's Lommel–Seeliger share at phase α: L(0) times its curve's profile, or L(0) at every
 * phase for a law without a curve.
 *
 * @param alphaRad - Phase angle, rad, 0 to π.
 */
export function limbShareAt(
  law: Pick<PhotometricLaw, "lommelSeeligerShare" | "lommelSeeligerCurve">,
  alphaRad: number,
): number {
  const curve = law.lommelSeeligerCurve;
  return curve === undefined
    ? law.lommelSeeligerShare
    : law.lommelSeeligerShare * PROFILES[curve](alphaRad);
}

/**
 * The phase factor f per channel at phase α, exactly: clamped at {@link PHASE_F_CLAMP} and held
 * past the template's range.
 *
 * @param alphaRad - Phase angle, rad, 0 to π.
 */
export function phaseFactor(law: PhotometricLaw, alphaRad: number): Rgb {
  const template = PHASE_TEMPLATES[law.template];
  const alpha = Math.min(Math.max(alphaRad, 0), template.validToRad);
  const phaseV = template.phaseV(alpha);
  const shape = shapePhase(law.lommelSeeligerShare, alpha, limbShareAt(law, alpha));
  // At conjunction the shape's phase function is zero (or rounds below it), where f is the clamp.
  const channel = (s: number): number =>
    shape > 0 ? Math.min(PHASE_F_CLAMP, phaseV ** s / shape) : PHASE_F_CLAMP;
  return [
    channel(law.phaseExponent[0]),
    channel(law.phaseExponent[1]),
    channel(law.phaseExponent[2]),
  ];
}

/**
 * f and L(α) sampled every {@link PHASE_TABLE_STEP_RAD}, the layout of one row of the shader's
 * `rgba32float` table: f in r, g and b, L(α) in a.
 */
export interface PhaseFactorTable {
  /** {@link PHASE_TABLE_SAMPLES} texels, r, g, b interleaved, in `f32`. */
  readonly rgb: Float32Array;
  /**
   * {@link PHASE_TABLE_SAMPLES} values of L(α) ({@link limbShareAt}) at the samples' own phases, in
   * `f32`: L(0) at every sample for a law without a curve.
   */
  readonly alpha: Float32Array;
}

/** Tabulates a law's phase factor and L(α) for the GPU and for {@link phaseFactorFromTable}. */
export function phaseFactorTable(law: PhotometricLaw): PhaseFactorTable {
  const rgb = new Float32Array(PHASE_TABLE_SAMPLES * 3);
  const alpha = new Float32Array(PHASE_TABLE_SAMPLES);
  for (let i = 0; i < PHASE_TABLE_SAMPLES; i += 1) {
    const phase = i * PHASE_TABLE_STEP_RAD;
    const [r, g, b] = phaseFactor(law, phase);
    rgb[3 * i] = r;
    rgb[3 * i + 1] = g;
    rgb[3 * i + 2] = b;
    alpha[i] = limbShareAt(law, phase);
  }
  return { rgb, alpha };
}

/** Each law's table, made once; laws are immutable values, so a law's table never changes. */
const TABLES = new WeakMap<PhotometricLaw, PhaseFactorTable>();

/**
 * The law's table, tabulated on first use and kept while the law is.
 *
 * @remarks
 * The reference BRDF reads f per pixel, so it must not re-tabulate per call.
 */
export function phaseFactorTableOf(law: PhotometricLaw): PhaseFactorTable {
  const cached = TABLES.get(law);
  if (cached !== undefined) {
    return cached;
  }
  const table = phaseFactorTable(law);
  TABLES.set(law, table);
  return table;
}

/**
 * f read from the table by linear interpolation, as the shader reads it.
 *
 * @param alphaRad - Phase angle, rad; clamped to [0, π].
 */
export function phaseFactorFromTable(table: PhaseFactorTable, alphaRad: number): Rgb {
  const x = Math.min(Math.max(alphaRad, 0), Math.PI) / PHASE_TABLE_STEP_RAD;
  const i = Math.min(Math.floor(x), PHASE_TABLE_SAMPLES - 2);
  const t = x - i;
  const texel = (k: number, c: number): number => table.rgb[3 * k + c] ?? 0;
  const channel = (c: number): number => texel(i, c) + t * (texel(i + 1, c) - texel(i, c));
  return [channel(0), channel(1), channel(2)];
}

/**
 * L(α) read from the table by linear interpolation, as the shader reads the row's alpha
 * (`litBody.wgsl`'s `phase_row`).
 *
 * @param alphaRad - Phase angle, rad; clamped to [0, π].
 */
export function limbShareFromTable(table: PhaseFactorTable, alphaRad: number): number {
  const x = Math.min(Math.max(alphaRad, 0), Math.PI) / PHASE_TABLE_STEP_RAD;
  const i = Math.min(Math.floor(x), PHASE_TABLE_SAMPLES - 2);
  const t = x - i;
  const below = table.alpha[i] ?? 0;
  return below + t * ((table.alpha[i + 1] ?? 0) - below);
}
