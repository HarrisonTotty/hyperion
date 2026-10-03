/**
 * A law's disc-integrated phase function, its geometric albedo and phase integral, and the law that
 * reaches a body's p and q (plan R07, Design note 5).
 *
 * @remarks
 * Φ(α) = f(α) Φ_shape(α; L), f read from the law's table as the shader reads it, so that the point
 * regime integrates the law the disc draws. `lawFor` takes p and q per display channel, the
 * template's L, and solves each channel's exponent s in Φ_t^s by bisection: q falls monotonically
 * as s rises, since Φ_t ≤ 1 wherever the integrand's weight sin α is not negligible.
 */
import type { Rgb } from "../photometry/toneCurve";
import {
  PHASE_F_CLAMP,
  phaseFactorFromTable,
  phaseFactorTableOf,
  type PhaseTemplateId,
  type PhotometricLaw,
} from "./law";
import { shapeGeometricAlbedo, shapePhase } from "./shapes";
import { PHASE_TEMPLATES } from "./templates";

/**
 * The law's disc-integrated phase function per channel, Φ(0) = 1.
 *
 * @param phaseRad - Phase angle, rad, 0 to π.
 */
export function discIntegratedPhase(law: PhotometricLaw, phaseRad: number): Rgb {
  const [r, g, b] = phaseFactorFromTable(phaseFactorTableOf(law), phaseRad);
  const shape = shapePhase(law.lommelSeeligerShare, phaseRad);
  return [r * shape, g * shape, b * shape];
}

/** The law's geometric albedo per channel: p = A [L + ⅔(1 − L)], since f(0) = 1. */
export function geometricAlbedo(law: PhotometricLaw): Rgb {
  const k = shapeGeometricAlbedo(law.lommelSeeligerShare);
  return [law.a[0] * k, law.a[1] * k, law.a[2] * k];
}

/** The exponent's bracket for the bisection, s from 1/16 to 16. */
const EXPONENT_BRACKET: readonly [number, number] = [1 / 16, 16];

/** Bisection steps: the bracket's width over 2⁶⁰, far below any q's precision. */
const BISECTION_STEPS = 60;

/** Simpson intervals of `lawFor`'s phase integral over [0, π] (0.1°), even. */
const SOLVE_INTERVALS = 1800;

/**
 * The phase integral of a template's law as a function of the exponent s, over a grid sampled
 * once: q(s) = 2 ∫ min(Φ_t^s, 4 Φ_shape) sin α dα inside the range and f(α_end) Φ_shape beyond.
 */
function phaseIntegralOfExponent(template: PhaseTemplateId): (s: number) => number {
  const { phaseV, validToRad, lommelSeeligerShare } = PHASE_TEMPLATES[template];
  const h = Math.PI / SOLVE_INTERVALS;
  const weight = new Float64Array(SOLVE_INTERVALS + 1);
  const phaseTemplate = new Float64Array(SOLVE_INTERVALS + 1);
  const shape = new Float64Array(SOLVE_INTERVALS + 1);
  const inRange: boolean[] = [];
  for (let i = 0; i <= SOLVE_INTERVALS; i += 1) {
    const alpha = i * h;
    const simpson = i === 0 || i === SOLVE_INTERVALS ? 1 : i % 2 === 1 ? 4 : 2;
    weight[i] = ((2 * h) / 3) * simpson * Math.sin(alpha);
    phaseTemplate[i] = phaseV(alpha);
    shape[i] = shapePhase(lommelSeeligerShare, alpha);
    inRange.push(alpha <= validToRad);
  }
  const endTemplate = phaseV(validToRad);
  const endShape = shapePhase(lommelSeeligerShare, validToRad);
  return (s) => {
    const heldF =
      endShape > 0 ? Math.min(PHASE_F_CLAMP, endTemplate ** s / endShape) : PHASE_F_CLAMP;
    let q = 0;
    for (let i = 0; i <= SOLVE_INTERVALS; i += 1) {
      const shapeI = shape[i] ?? 0;
      const phase =
        inRange[i] === true
          ? Math.min((phaseTemplate[i] ?? 0) ** s, PHASE_F_CLAMP * shapeI)
          : heldF * shapeI;
      q += (weight[i] ?? 0) * phase;
    }
    return q;
  };
}

/** The exponent s at which `qOf(s)` equals `q`, by bisection; a bracket end where q is beyond it. */
function solveExponent(qOf: (s: number) => number, q: number): number {
  let [low, high] = EXPONENT_BRACKET;
  if (q >= qOf(low)) {
    return low;
  }
  if (q <= qOf(high)) {
    return high;
  }
  for (let step = 0; step < BISECTION_STEPS; step += 1) {
    const middle = Math.sqrt(low * high);
    if (qOf(middle) > q) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return Math.sqrt(low * high);
}

/**
 * The law of a body of geometric albedo p and phase integral q per display channel on a template.
 *
 * @remarks
 * L is the template's; A = p ÷ [L + ⅔(1 − L)]; each channel's s is solved so that the law's phase
 * integral is q. A q beyond what s from 1/16 to 16 reaches takes the bracket's end, the nearest law
 * the template allows. The phase integral is that of the exact, clamped and held f; the table's
 * interpolation moves it by about 10⁻⁴.
 *
 * @param p - Geometric albedo per display channel (r, g, b).
 * @param q - Phase integral per display channel (r, g, b).
 */
export function lawFor(p: Rgb, q: Rgb, template: PhaseTemplateId): PhotometricLaw {
  const { lommelSeeligerShare } = PHASE_TEMPLATES[template];
  const k = shapeGeometricAlbedo(lommelSeeligerShare);
  const qOf = phaseIntegralOfExponent(template);
  return {
    a: [p[0] / k, p[1] / k, p[2] / k],
    lommelSeeligerShare,
    template,
    phaseExponent: [solveExponent(qOf, q[0]), solveExponent(qOf, q[1]), solveExponent(qOf, q[2])],
  };
}
