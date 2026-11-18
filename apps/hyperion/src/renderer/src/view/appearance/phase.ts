/**
 * A law's disc-integrated phase function, its geometric albedo and phase integral, and the law that
 * reaches a body's p and q (plan R07, Design note 5).
 *
 * @remarks
 * Φ(α) = f(α) Φ_shape(α; L), f read from the law's table as the shader reads it and Φ_shape taken
 * with the law's L(α) (`law.ts`'s `limbShareAt`), so that the point regime integrates the law the
 * disc draws. `lawFor` takes p and q per display channel, the template's L(0) and its L(α) curve,
 * and solves each channel's exponent s in Φ_t^s by bisection: q falls monotonically as s rises,
 * since Φ_t ≤ 1 wherever the integrand's weight sin α is not negligible.
 */
import { pow, powFromLog2Parts, powLog2Parts, sin } from "../../lib/pinnedMath";
import type { Rgb } from "../photometry/toneCurve";
import {
  limbShareAt,
  PHASE_F_CLAMP,
  phaseFactor,
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
  const shape = shapePhase(law.lommelSeeligerShare, phaseRad, limbShareAt(law, phaseRad));
  return [r * shape, g * shape, b * shape];
}

/** Simpson intervals of {@link lawPhaseIntegral} over [0, π], as `shapes.ts`' `phaseIntegral`. */
const LAW_INTEGRAL_INTERVALS = 7200;

/**
 * The law's phase integral per channel, q = 2 ∫₀^π f(α) Φ_shape(α; L) sin α dα, with the exact f
 * (clamped, and held past the template's range) and the law's L(α), as plan 14 defines a body's q
 * (P14.T47).
 *
 * @remarks
 * Simpson's rule over 7,200 intervals, as `shapes.ts`' `phaseIntegral`, with each node's f found
 * once for the three channels. The table the shader reads interpolates f, which moves q by about
 * 10⁻⁴ ({@link discIntegratedPhase}).
 */
export function lawPhaseIntegral(law: PhotometricLaw): Rgb {
  const n = LAW_INTEGRAL_INTERVALS;
  const h = Math.PI / n;
  const sums: [number, number, number] = [0, 0, 0];
  for (let i = 0; i <= n; i += 1) {
    const alpha = i * h;
    const weight =
      (i === 0 || i === n ? 1 : i % 2 === 1 ? 4 : 2) *
      shapePhase(law.lommelSeeligerShare, alpha, limbShareAt(law, alpha)) *
      sin(alpha);
    const f = phaseFactor(law, alpha);
    sums[0] += weight * f[0];
    sums[1] += weight * f[1];
    sums[2] += weight * f[2];
  }
  const scale = (2 * h) / 3;
  return [sums[0] * scale, sums[1] * scale, sums[2] * scale];
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
 * A template's grid for {@link phaseIntegralAt}, sampled once and kept ({@link exponentGridOf}): q(s)
 * = 2 ∫ min(Φ_t^s, 4 Φ_shape) sin α dα inside the range and f(α_end) Φ_shape beyond, Φ_shape with
 * the template's L(α).
 */
interface ExponentGrid {
  /** The nodes within the template's range, 0 to `inRange` − 1: α rises with the node. */
  readonly inRange: number;
  /** Each node's Simpson weight times 2h ÷ 3 times sin α. */
  readonly weight: Float64Array;
  /** Φ_t at each node, the base its exponent raises. */
  readonly phaseTemplate: Float64Array;
  /** `pinnedMath.ts`' {@link powLog2Parts} of each node's Φ_t, two a node. */
  readonly log2Parts: Float64Array;
  /** Φ_shape at each node. */
  readonly shape: Float64Array;
  /** Φ_t and Φ_shape at the range's end, which f is held at beyond it. */
  readonly endTemplate: number;
  readonly endShape: number;
  /** q at the bracket's ends, which every solve reads first. */
  readonly bracketQ: readonly [number, number];
}

/** Each template's grid, made on its first law. */
const GRIDS = new Map<PhaseTemplateId, ExponentGrid>();

/** The template's {@link ExponentGrid}, sampled on first use and kept. */
function exponentGridOf(template: PhaseTemplateId): ExponentGrid {
  const held = GRIDS.get(template);
  if (held !== undefined) {
    return held;
  }
  const curve = PHASE_TEMPLATES[template];
  const { phaseV, validToRad, lommelSeeligerShare } = curve;
  const h = Math.PI / SOLVE_INTERVALS;
  const weight = new Float64Array(SOLVE_INTERVALS + 1);
  const phaseTemplate = new Float64Array(SOLVE_INTERVALS + 1);
  const log2Parts = new Float64Array(2 * (SOLVE_INTERVALS + 1));
  const shape = new Float64Array(SOLVE_INTERVALS + 1);
  let inRange = 0;
  for (let i = 0; i <= SOLVE_INTERVALS; i += 1) {
    const alpha = i * h;
    const simpson = i === 0 || i === SOLVE_INTERVALS ? 1 : i % 2 === 1 ? 4 : 2;
    weight[i] = ((2 * h) / 3) * simpson * sin(alpha);
    phaseTemplate[i] = phaseV(alpha);
    powLog2Parts(phaseTemplate[i] ?? 0, log2Parts, 2 * i);
    shape[i] = shapePhase(lommelSeeligerShare, alpha, limbShareAt(curve, alpha));
    if (alpha <= validToRad) {
      inRange = i + 1;
    }
  }
  const endTemplate = phaseV(validToRad);
  const endShape = shapePhase(lommelSeeligerShare, validToRad, limbShareAt(curve, validToRad));
  const sampled = { inRange, weight, phaseTemplate, log2Parts, shape, endTemplate, endShape };
  const grid: ExponentGrid = {
    ...sampled,
    bracketQ: [
      phaseIntegralAt(sampled, EXPONENT_BRACKET[0]),
      phaseIntegralAt(sampled, EXPONENT_BRACKET[1]),
    ],
  };
  GRIDS.set(template, grid);
  return grid;
}

/**
 * The phase integral of the grid's template at the exponent s.
 *
 * @remarks
 * Each node's Φ_t^s is pinned `pow`'s, from the node's log₂ parts ({@link powFromLog2Parts}), and
 * the nodes are summed in order, so q is bit for bit what pow at every node gives.
 */
function phaseIntegralAt(grid: Omit<ExponentGrid, "bracketQ">, s: number): number {
  const { inRange, weight, phaseTemplate, log2Parts, shape, endTemplate, endShape } = grid;
  const heldF =
    endShape > 0 ? Math.min(PHASE_F_CLAMP, pow(endTemplate, s) / endShape) : PHASE_F_CLAMP;
  let q = 0;
  for (let i = 0; i < inRange; i += 1) {
    const power = powFromLog2Parts(
      phaseTemplate[i] ?? 0,
      log2Parts[2 * i] ?? Number.NaN,
      log2Parts[2 * i + 1] ?? Number.NaN,
      s,
    );
    q += (weight[i] ?? 0) * Math.min(power, PHASE_F_CLAMP * (shape[i] ?? 0));
  }
  for (let i = inRange; i <= SOLVE_INTERVALS; i += 1) {
    q += (weight[i] ?? 0) * (heldF * (shape[i] ?? 0));
  }
  return q;
}

/**
 * The exponent s at which the grid's q(s) equals `q`, by bisection; a bracket end where q is beyond
 * it.
 *
 * @remarks
 * Once the bracket closes to two neighbouring doubles, the middle is one of its ends, whose q is
 * already known, and is not integrated again.
 */
function solveExponent(grid: ExponentGrid, q: number): number {
  let [low, high] = EXPONENT_BRACKET;
  let [qLow, qHigh] = grid.bracketQ;
  if (q >= qLow) {
    return low;
  }
  if (q <= qHigh) {
    return high;
  }
  for (let step = 0; step < BISECTION_STEPS; step += 1) {
    const middle = Math.sqrt(low * high);
    const qMiddle = middle === low ? qLow : middle === high ? qHigh : phaseIntegralAt(grid, middle);
    if (qMiddle > q) {
      low = middle;
      qLow = qMiddle;
    } else {
      high = middle;
      qHigh = qMiddle;
    }
  }
  return Math.sqrt(low * high);
}

/**
 * How many solved exponents `lawFor` keeps a template: far more than the phase integrals a scene's
 * laws ask of one.
 */
export const SOLVED_EXPONENTS_KEPT = 256;

/** Each template's solved exponents, by q, the least recently used first. */
const SOLVED = new Map<PhaseTemplateId, Map<number, number>>();

/**
 * The exponent at which the template's law reaches q ({@link solveExponent}), solved once for equal
 * q and kept among the {@link SOLVED_EXPONENTS_KEPT} used most recently.
 *
 * @remarks
 * A solve reads nothing but its template's grid and q, so a kept exponent is the one solving again
 * gives. The map's keys are equal for 0 and −0 and for every NaN, each of which compares alike.
 */
function exponentFor(template: PhaseTemplateId, q: number): number {
  let solved = SOLVED.get(template);
  if (solved === undefined) {
    solved = new Map();
    SOLVED.set(template, solved);
  }
  const held = solved.get(q);
  if (held !== undefined) {
    solved.delete(q);
    solved.set(q, held);
    return held;
  }
  const s = solveExponent(exponentGridOf(template), q);
  solved.set(q, s);
  if (solved.size > SOLVED_EXPONENTS_KEPT) {
    const oldest = solved.keys().next();
    if (oldest.done !== true) {
      solved.delete(oldest.value);
    }
  }
  return s;
}

/**
 * The law of a body of geometric albedo p and phase integral q per display channel on a template.
 *
 * @remarks
 * L(0) and the L(α) curve are the template's; A = p ÷ [L(0) + ⅔(1 − L(0))]; each channel's s is
 * solved so that the law's phase integral, under its L(α), is q. A q beyond what s from 1/16 to 16
 * reaches takes the bracket's end, the nearest law the template allows. The phase integral is that
 * of the exact, clamped and held f; the table's interpolation moves it by about 10⁻⁴. Each
 * template's grid is sampled on its first law and kept, and each channel's exponent is solved once
 * for equal q and kept among the {@link SOLVED_EXPONENTS_KEPT} last used: a solve reads nothing
 * else, so a law is bit for bit the same whatever came before it.
 *
 * @param p - Geometric albedo per display channel (r, g, b).
 * @param q - Phase integral per display channel (r, g, b).
 */
export function lawFor(p: Rgb, q: Rgb, template: PhaseTemplateId): PhotometricLaw {
  const { lommelSeeligerShare } = PHASE_TEMPLATES[template];
  const k = shapeGeometricAlbedo(lommelSeeligerShare);
  return withTemplateCurve({
    a: [p[0] / k, p[1] / k, p[2] / k],
    lommelSeeligerShare,
    template,
    phaseExponent: [
      exponentFor(template, q[0]),
      exponentFor(template, q[1]),
      exponentFor(template, q[2]),
    ],
  });
}

/**
 * `law` with its template's L(α) curve (`templates.ts`), as every law made for a template carries
 * it; `law` itself where the template has none.
 */
export function withTemplateCurve(law: PhotometricLaw): PhotometricLaw {
  const curve = PHASE_TEMPLATES[law.template].lommelSeeligerCurve;
  return curve === undefined ? law : { ...law, lommelSeeligerCurve: curve };
}
