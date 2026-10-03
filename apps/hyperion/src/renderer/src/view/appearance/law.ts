/**
 * The photometric law a body is shaded with, and its phase factor f tabulated for the GPU (plan
 * R07, Design note 5).
 *
 * @remarks
 * I/F = A · f(α) · [L · 2μ₀ ÷ (μ₀ + μ) + (1 − L) μ₀] per display channel, f(0) = 1. The phase
 * factor reproduces a measured V curve Φ_t raised to a per-channel exponent s:
 * f(α) = Φ_t(α)^s ÷ Φ_shape(α; L), so that the disc-integrated phase function is Φ_t^s wherever f
 * is not clamped. It is clamped at {@link PHASE_F_CLAMP} (a Lambert crescent vanishes faster than a
 * cloudy one: Venus's f would reach 61 at 170°) and held at its value at the template's last valid
 * phase beyond it. The table samples f every 0.5° from 0° to 180° in `f32`, and both the
 * TypeScript reference and the shader interpolate it linearly, so the two read one f.
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

/** A body's lunar-Lambert law with a fitted phase factor (Design note 5). */
export interface PhotometricLaw {
  /** The albedo scale A per display channel (r, g, b); p = A [L + ⅔(1 − L)]. */
  readonly a: Rgb;
  /** L, the Lommel–Seeliger share, 0 (Lambert) to 1. */
  readonly lommelSeeligerShare: number;
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
 * The phase factor f per channel at phase α, exactly: clamped at {@link PHASE_F_CLAMP} and held
 * past the template's range.
 *
 * @param alphaRad - Phase angle, rad, 0 to π.
 */
export function phaseFactor(law: PhotometricLaw, alphaRad: number): Rgb {
  const template = PHASE_TEMPLATES[law.template];
  const alpha = Math.min(Math.max(alphaRad, 0), template.validToRad);
  const phaseV = template.phaseV(alpha);
  const shape = shapePhase(law.lommelSeeligerShare, alpha);
  // At conjunction the shape's phase function is zero (or rounds below it), where f is the clamp.
  const channel = (s: number): number =>
    shape > 0 ? Math.min(PHASE_F_CLAMP, phaseV ** s / shape) : PHASE_F_CLAMP;
  return [
    channel(law.phaseExponent[0]),
    channel(law.phaseExponent[1]),
    channel(law.phaseExponent[2]),
  ];
}

/** f sampled every {@link PHASE_TABLE_STEP_RAD}, the layout of the shader's RGB 1D texture. */
export interface PhaseFactorTable {
  /** {@link PHASE_TABLE_SAMPLES} texels, r, g, b interleaved, in `f32`. */
  readonly rgb: Float32Array;
}

/** Tabulates a law's phase factor for the GPU and for {@link phaseFactorFromTable}. */
export function phaseFactorTable(law: PhotometricLaw): PhaseFactorTable {
  const rgb = new Float32Array(PHASE_TABLE_SAMPLES * 3);
  for (let i = 0; i < PHASE_TABLE_SAMPLES; i += 1) {
    const [r, g, b] = phaseFactor(law, i * PHASE_TABLE_STEP_RAD);
    rgb[3 * i] = r;
    rgb[3 * i + 1] = g;
    rgb[3 * i + 2] = b;
  }
  return { rgb };
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
