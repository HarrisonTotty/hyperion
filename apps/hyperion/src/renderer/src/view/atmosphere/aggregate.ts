/**
 * Fractal aggregates of spherical monomers, soot and tholin hazes among them (plan R08, R08.T5.c;
 * Design note 6): Tazaki and Tanaka 2018's modified mean-field theory (MMF), with optool as its
 * reference.
 *
 * @remarks
 * R. Tazaki and H. Tanaka, "Light scattering by fractal dust aggregates. II. Opacity and asymmetry
 * parameter", Astrophys. J. 860, 79 (2018), DOI 10.3847/1538-4357/aac32d. An aggregate of N
 * monomers of radius a₀ has the radius of gyration R_g by N = k_f (R_g ÷ a₀)^D_f, and the
 * characteristic radius R_c = √(5 ÷ 3) R_g. Its monomers are distributed by the two-point
 * correlation function with the Gaussian cut-off, g(u) ∝ u^(D_f − 3) exp(−(D_f ÷ 4)(u ÷ R_g)²)
 * (their (18)–(19), Tazaki et al. 2016's form), so its static structure factor is Kummer's
 * function, S(q) = M(D_f ÷ 2, 3 ÷ 2, −q² R_g² ÷ D_f), q = 2k sin(θ ÷ 2).
 *
 * - **The mean field.** Each monomer's Mie coefficients aₙ, bₙ are replaced by the mean-field
 *   coefficients dₙ that solve (I + (N − 1) a·T) d = a (Botet, Rannou and Cabane 1997's mean-field
 *   equations, Appl. Opt. 36, 8791), with the translation coefficients of Tazaki and Tanaka's
 *   (14)–(15) built from the structure integrals S_p(kR_g) (their (31)) and the Legendre integrals
 *   a(ν, n, p), b(ν, n, p) (their (29)–(30)).
 * - **The cross-sections.** C_ext is the mean field's. C_abs is the larger of the mean field's and
 *   the Rayleigh–Gans–Debye absorption made opaque by geometric optics, G (1 − e^(−τ)) with
 *   τ = N C_abs,monomer ÷ G and G the aggregate's geometric cross-section (Tazaki 2021, below);
 *   C_sca = C_ext − C_abs (their §3.3).
 * - **The phase matrix** is the mean-field monomer's times N (1 + (N − 1) S(q)), so a₂ = a₁ and
 *   a₄ = a₃, the matrix of spherical monomers.
 * - **The geometric cross-section** is R. Tazaki, "Analytic expressions for geometric cross-sections
 *   of fractal dust aggregates", Mon. Not. R. Astron. Soc. 504, 2811–2821 (2021):
 *   G = N π a₀² ÷ (1 + (N − 1) σ̃), with σ̃ = x_min ÷ (16 Γ(D_f ÷ 2)) ∫ from x_min to ∞ of
 *   x^((D_f − 2) ÷ 2 − 1) e^(−x) dx, x_min = D_f (k_f ÷ N)^(2 ÷ D_f), for the Gaussian cut-off and
 *   with the numerical factor A = 1.
 *
 * These are optool's choices for its `-mmf` option (C. Dominik, M. Min and R. Tazaki, "OpTool:
 * Command-line driven tool for creating complex dust opacities", ascl:2104.010, version 1.9.14,
 * MIT licence; its `optool_fractal.f90` is Tazaki's OPACFRACTAL v3.0): the Gaussian cut-off
 * (`iqcor = 1`), Tazaki 2021's geometric cross-section (`iqgeo = 3`) and k_f = (5 ÷ 3)^(D_f ÷ 2),
 * which makes R_c = a₀ N^(1 ÷ D_f). The numerical methods here are this module's own: Gauss–Legendre
 * panels in ln u for the structure integrals, Miller's algorithm for the spherical Bessel functions,
 * Kummer's transformation and its asymptotic series for S(q), and Gaussian elimination. The tests
 * hold the result to optool's committed output (`fixtures/optool-titan.json`).
 *
 * **The phase-shift gate** (ruled 2026-10-10, science-r08-mmf.md). The paper validates the MMF
 * against T-matrix results at D_f 1.9 and 3.0, with opacities within about 20–25%, and its phase
 * function while Δφ = max(2x₀|m − 1|, 2kR_c|m_MG − 1|) < 1, their (9) (the larger of the monomer's
 * and the aggregate's phase shift, m_MG the Maxwell–Garnett index at the filling factor
 * N (a₀ ÷ R_c)³; their §2.2, applied to the phase function in §4.2), as optool computes it. Past
 * it the mode keeps its cross-sections ({@link mmfPhaseBasis}): while the aggregate's own term is
 * below 1 it keeps the MMF table and matrix, optool's `-mmfss` result, as an `analogue`; where
 * that term reaches 1 it takes a Henyey–Greenstein phase from its MMF asymmetry, a `fallback`
 * with no matrix (Design note 6). The mean field degrades gradually past x₀ ≈ 0.5, where its pair
 * correlation has no contact peak (Rannou, Botet and Tazaki 2024, Icarus 424, 116247; R08's Risks).
 */

import { gaussLegendre } from "../lighting/quadrature";
import { type MaterialForm, indexAt, resolveMaterial } from "./materials/materials";
import { type ComplexIndex, mieAmplitudes, mieCoefficients, type MieCoefficients } from "./mie";
import { type ModeOptics, PHASE_TABLE_MU, type ScatteringMatrix } from "./sizeDistribution";

/**
 * A mode of identical fractal aggregates (Design note 6; P14.T24.c's aggregate mode): its monomers'
 * material, radius and count, and the aggregate's fractal dimension.
 */
export interface AggregateMode {
  /** The monomers' material key (P14.T49.b), never narrowed: an unknown key takes the generic stand-in. */
  readonly material: string;
  readonly form?: MaterialForm;
  /** a₀, µm, > 0. */
  readonly monomerRadiusUm: number;
  /** N ≥ 1. */
  readonly monomerCount: number;
  /** D_f, in (1, {@link MMF_FRACTAL_DIMENSION_MAX}]; below 1.9 an extrapolation (its remarks). */
  readonly fractalDimension: number;
  /**
   * k_f in N = k_f (R_g ÷ a₀)^D_f, > 0; by default (5 ÷ 3)^(D_f ÷ 2), optool's and Tazaki's, for
   * which R_c = a₀ N^(1 ÷ D_f).
   */
  readonly fractalPrefactor?: number;
}

/**
 * The largest fractal dimension a mode may have: the inventory's cap (P14.T24.c), inside the range
 * Tazaki and Tanaka 2018 validated against T-matrix (D_f 1.9 to 3.0).
 *
 * @remarks
 * Below D_f 1.9 (soot near 1.8, say) the MMF is extrapolated: no T-matrix comparison exists there,
 * and the mode keeps (9)'s basis unlabelled for it, pending the owner (R08's Risks).
 */
export const MMF_FRACTAL_DIMENSION_MAX = 2.5;

/**
 * The phase shift at and above which an aggregate's MMF phase function is no longer a validated
 * `model` (Tazaki and Tanaka 2018's (9), §2.2 and §4.2; Design note 6): past it by (9) the table
 * is kept as an `analogue`, and past it by the aggregate's own term the phase is a
 * Henyey–Greenstein `fallback` ({@link mmfPhaseBasis}).
 */
export const MMF_PHASE_SHIFT_LIMIT = 1;

/**
 * What an aggregate carries wherever its MMF table is drawn (a `model`, or an `analogue` past (9)):
 * its MMF matrix (`kept`), or none, a total depolariser.
 * The plan keeps the matrix only if it agrees with optool's elements to their stated tolerance
 * (Design note 10), which `aggregate.test.ts` holds against `fixtures/optool-titan.json`.
 */
export const MMF_MATRIX: "kept" | "depolariser" = "kept";

/** The matrix an aggregate whose MMF table is drawn carries, by {@link MMF_MATRIX}. */
function keptMatrix(matrix: ScatteringMatrix): ScatteringMatrix | undefined {
  let kept: ScatteringMatrix | undefined;
  switch (MMF_MATRIX) {
    case "kept":
      kept = matrix;
      break;
    case "depolariser":
      kept = undefined;
      break;
  }
  return kept;
}

/**
 * The largest monomer size parameter accepted, x₀ = 2πa₀ ÷ λ: 10, so that N_stop ≤ 22 and the
 * structure integrals' orders stay where the spherical Bessel functions are representable. Haze
 * monomers are far smaller (Titan's 0.05 µm is x₀ 0.83 at 380 nm).
 */
export const MMF_MONOMER_SIZE_PARAMETER_MAX = 10;

/** One aggregate's MMF optics at one wavelength, before the gate. */
export interface MmfResult {
  /** C_ext, µm². */
  readonly extinctionCrossSectionUm2: number;
  /** C_sca, µm². */
  readonly scatteringCrossSectionUm2: number;
  /** The mean cosine of the mean-field phase function. */
  readonly asymmetry: number;
  /** G, Tazaki 2021's geometric cross-section, µm². */
  readonly geometricCrossSectionUm2: number;
  /** The aggregate's term of Tazaki and Tanaka 2018's (9), 2kR_c|m_MG − 1|. */
  readonly phaseShift: number;
  /** The monomer's term of (9), 2x₀|m − 1|. */
  readonly monomerPhaseShift: number;
  /** The mean-field matrix at the cosines asked, normalised as {@link ScatteringMatrix} is. */
  readonly matrix: ScatteringMatrix;
}

/** An aggregate mode's optics: {@link ModeOptics} with the gate's phase shifts. */
export interface AggregateOptics extends ModeOptics {
  readonly phaseShift: number;
  readonly monomerPhaseShift: number;
}

interface Aggregate {
  readonly a0: number;
  readonly count: number;
  readonly df: number;
  readonly kf: number;
}

function checkAggregate(mode: Omit<AggregateMode, "material" | "form">): Aggregate {
  const { monomerRadiusUm: a0, monomerCount: count, fractalDimension: df } = mode;
  if (!(a0 > 0 && Number.isFinite(a0))) {
    throw new RangeError(`monomer radius ${a0} µm must be finite and > 0`);
  }
  if (!(count >= 1 && Number.isFinite(count))) {
    throw new RangeError(`monomer count ${count} must be finite and ≥ 1`);
  }
  if (!(df > 1 && df <= MMF_FRACTAL_DIMENSION_MAX)) {
    throw new RangeError(
      `fractal dimension ${df} must lie in (1, ${MMF_FRACTAL_DIMENSION_MAX}], the MMF's range here`,
    );
  }
  const kf = mode.fractalPrefactor ?? (5 / 3) ** (df / 2);
  if (!(kf > 0 && Number.isFinite(kf))) {
    throw new RangeError(`fractal prefactor ${kf} must be finite and > 0`);
  }
  return { a0, count, df, kf };
}

// ---------------------------------------------------------------------------------------------
// Special functions.

/** Lanczos's coefficients for g = 7, n = 9 (P. Godfrey's set), good to about 10⁻¹⁵. */
const LANCZOS = [
  0.999_999_999_999_809_9, 676.520_368_121_885_1, -1_259.139_216_722_402_8, 771.323_428_777_653_1,
  -176.615_029_162_140_59, 12.507_343_278_686_905, -0.138_571_095_265_720_12,
  9.984_369_578_019_571_6e-6, 1.505_632_735_149_311_6e-7,
];

/**
 * Γ(x) for x > 0, by Lanczos's approximation (C. Lanczos, "A precision approximation of the gamma
 * function", SIAM J. Numer. Anal. B 1 (1964) 86–96) with P. Godfrey's coefficients, and the
 * reflection formula below ½.
 */
export function gammaFunction(x: number): number {
  if (x < 0.5) {
    return Math.PI / (Math.sin(Math.PI * x) * gammaFunction(1 - x));
  }
  const z = x - 1;
  let sum = LANCZOS[0] ?? 0;
  for (let i = 1; i < LANCZOS.length; i += 1) {
    sum += (LANCZOS[i] ?? 0) / (z + i);
  }
  const t = z + 7.5;
  return Math.sqrt(2 * Math.PI) * t ** (z + 0.5) * Math.exp(-t) * sum;
}

/** Below this z the transformed series sums M(a, b, −z); above it, the asymptotic series. */
const KUMMER_ASYMPTOTIC_FROM = 60;

/**
 * Kummer's function M(a, b, −z) for z ≥ 0 and 0 < a < b: by Kummer's transformation
 * e^(−z) M(b − a, b, z), whose series has positive terms, and for large z by the asymptotic series
 * Γ(b) ÷ Γ(b − a) z^(−a) Σₛ (a)ₛ (1 + a − b)ₛ ÷ s! z^(−s) (DLMF 13.2.39 and 13.7.2), cut at its
 * smallest term, whose size at z = 60 is far below double rounding.
 */
export function kummerNegative(a: number, b: number, z: number): number {
  if (z <= KUMMER_ASYMPTOTIC_FROM) {
    const c = b - a;
    let term = 1;
    let sum = 1;
    for (let k = 0; k < 10_000; k += 1) {
      term *= ((c + k) * z) / ((b + k) * (k + 1));
      sum += term;
      if (term < 1e-17 * sum) {
        break;
      }
    }
    return Math.exp(-z) * sum;
  }
  let term = 1;
  let sum = 1;
  for (let s = 0; s < 200; s += 1) {
    const next = (term * (a + s) * (1 + a - b + s)) / ((s + 1) * z);
    if (Math.abs(next) >= Math.abs(term)) {
      break;
    }
    term = next;
    sum += term;
    if (Math.abs(term) < 1e-17 * Math.abs(sum)) {
      break;
    }
  }
  return ((gammaFunction(b) / gammaFunction(b - a)) * sum) / z ** a;
}

/** The spherical Bessel functions jₙ(u) and yₙ(u), n = 0 … order, into the arrays given. */
function sphericalBessel(order: number, u: number, j: Float64Array, y: Float64Array): void {
  const sin = Math.sin(u);
  const cos = Math.cos(u);
  y[0] = -cos / u;
  if (order >= 1) {
    y[1] = -cos / (u * u) - sin / u;
  }
  for (let n = 1; n < order; n += 1) {
    y[n + 1] = ((2 * n + 1) / u) * (y[n] ?? 0) - (y[n - 1] ?? 0);
  }
  const j0 = sin / u;
  if (u > order) {
    // Upward recurrence is stable for jₙ while n < u.
    j[0] = j0;
    if (order >= 1) {
      j[1] = sin / (u * u) - cos / u;
    }
    for (let n = 1; n < order; n += 1) {
      j[n + 1] = ((2 * n + 1) / u) * (j[n] ?? 0) - (j[n - 1] ?? 0);
    }
    return;
  }
  // Miller's algorithm: downward from far above the order, where jₙ₊₁ ÷ jₙ ≈ u ÷ (2n + 3) < ½,
  // so 60 extra orders leave a relative error under 2⁻⁶⁰; then normalised by j₀ or j₁, whichever
  // is the larger, so that neither's zero spoils it.
  const start = order + 60;
  let above = 0;
  let here = 1e-280;
  for (let n = start; n >= 1; n -= 1) {
    const below = ((2 * n + 1) / u) * here - above;
    above = here;
    here = below;
    if (n - 1 <= order) {
      j[n - 1] = below;
    }
    if (Math.abs(here) > 1e250) {
      above *= 1e-250;
      here *= 1e-250;
      for (let m = n - 1; m <= order; m += 1) {
        j[m] = (j[m] ?? 0) * 1e-250;
      }
    }
  }
  const j1 = u < 0.1 ? u / 3 - (u * u * u) / 30 + u ** 5 / 840 : sin / (u * u) - cos / u;
  const f0 = j[0] ?? 0;
  const f1 = j[1] ?? above;
  const scale = Math.abs(j0) >= Math.abs(j1) || order < 1 ? j0 / f0 : j1 / f1;
  for (let n = 0; n <= order; n += 1) {
    j[n] = (j[n] ?? 0) * scale;
  }
}

// ---------------------------------------------------------------------------------------------
// The structure integrals S_p(kR_g), Tazaki and Tanaka's (31) with the Gaussian cut-off.

/** The structure integrals' panels' widest step in ln u. */
const SP_PANEL_LN_U = 0.25;
/** … and in u, against the integrand's oscillation of period π. */
const SP_PANEL_U = 1;
/** The Gauss–Legendre order of each panel. */
const SP_PANEL_ORDER = 8;
const SP_RULE = gaussLegendre(SP_PANEL_ORDER);
/** The lower end of the numerical integral, as a fraction of x_g; below it jₚ yₚ is its leading term. */
const SP_TAIL_FRACTION = 1e-4;
/** The cut-off's exponent at the upper end, e^(−25), as optool's η₁. */
const SP_CUTOFF_EXPONENT = 25;

/**
 * S_p for p = 0 … pMax: (1 ÷ (2x_g^D_f)) ∫₀^∞ u^(D_f − 1) jₚ(u) hₚ⁽¹⁾(u) f_c(u ÷ x_g) du, with
 * f_c(t) = 2c^(D_f ÷ 2) ÷ Γ(D_f ÷ 2) · e^(−ct²), c = D_f ÷ 4, normalised so that
 * (1 ÷ x_g^D_f) ∫ u^(D_f − 1) f_c du = 1. Returned as (re, im) pairs.
 */
function structureIntegrals(df: number, xg: number, pMax: number): Float64Array {
  const c = df / 4;
  const fcNorm = (2 * c ** (df / 2)) / gammaFunction(df / 2);
  const uLow = SP_TAIL_FRACTION * xg;
  const uHigh = 2 * xg * Math.sqrt(SP_CUTOFF_EXPONENT / df);
  const sums = new Float64Array(2 * (pMax + 1));
  const j = new Float64Array(pMax + 1);
  const y = new Float64Array(pMax + 1);
  let lnU = Math.log(uLow);
  const lnHigh = Math.log(uHigh);
  while (lnU < lnHigh) {
    const u0 = Math.exp(lnU);
    const next = Math.min(lnHigh, lnU + Math.min(SP_PANEL_LN_U, Math.log1p(SP_PANEL_U / u0)));
    const half = 0.5 * (next - lnU);
    for (let i = 0; i < SP_PANEL_ORDER; i += 1) {
      const lu = lnU + half * (1 + (SP_RULE.x[i] ?? 0));
      const u = Math.exp(lu);
      const t = u / xg;
      // du = u d(ln u), so the integrand gains one power of u.
      const weight = half * (SP_RULE.w[i] ?? 0) * u ** df * fcNorm * Math.exp(-c * t * t);
      sphericalBessel(pMax, u, j, y);
      for (let p = 0; p <= pMax; p += 1) {
        const jp = j[p] ?? 0;
        sums[2 * p] = (sums[2 * p] ?? 0) + weight * jp * jp;
        sums[2 * p + 1] = (sums[2 * p + 1] ?? 0) + weight * jp * (y[p] ?? 0);
      }
    }
    lnU = next;
  }
  // Below u_low: jₚ yₚ → −1 ÷ ((2p + 1) u) and jₚ² → u^(2p) ÷ ((2p + 1)!!)², with f_c at its peak.
  let doubleFactorial = 1;
  for (let p = 0; p <= pMax; p += 1) {
    doubleFactorial *= 2 * p + 1;
    sums[2 * p] =
      (sums[2 * p] ?? 0) +
      (fcNorm * uLow ** (2 * p + df)) / ((2 * p + df) * doubleFactorial * doubleFactorial);
    sums[2 * p + 1] =
      (sums[2 * p + 1] ?? 0) - (fcNorm * uLow ** (df - 1)) / ((2 * p + 1) * (df - 1));
  }
  const scale = 1 / (2 * xg ** df);
  return sums.map((v) => v * scale);
}

/**
 * Tazaki 2021's σ̃ for the Gaussian cut-off: x_min ÷ (16 Γ(D_f ÷ 2)) ∫ from x_min of
 * x^(a − 1) e^(−x) dx, a = (D_f − 2) ÷ 2, x_min = D_f (k_f ÷ N)^(2 ÷ D_f); the integral by
 * Gauss–Legendre panels in ln x to x = 60, past which it is under e^(−60).
 */
function meanOverlapEfficiency(aggregate: Aggregate): number {
  const { df, kf, count } = aggregate;
  const a = 0.5 * (df - 2);
  const xMin = df * (kf / count) ** (2 / df);
  const factor = xMin / (16 * gammaFunction(0.5 * df));
  const rule = gaussLegendre(16);
  let integral = 0;
  let ln = Math.log(xMin);
  const top = Math.log(Math.max(60, 2 * xMin));
  while (ln < top) {
    const next = Math.min(top, ln + 0.25);
    const half = 0.5 * (next - ln);
    for (let i = 0; i < rule.x.length; i += 1) {
      const x = Math.exp(ln + half * (1 + (rule.x[i] ?? 0)));
      integral += half * (rule.w[i] ?? 0) * x ** a * Math.exp(-x);
    }
    ln = next;
  }
  return factor * integral;
}

// ---------------------------------------------------------------------------------------------
// Complex helpers, on (re, im) pairs.

function cMul(aRe: number, aIm: number, bRe: number, bIm: number): [number, number] {
  return [aRe * bRe - aIm * bIm, aRe * bIm + aIm * bRe];
}

/**
 * Solves the complex system M z = v in place by Gaussian elimination with partial pivoting; M is
 * n × n row-major as (re, im) pairs, v has n pairs, and the solution replaces v.
 */
function solveComplex(n: number, matrix: Float64Array, vector: Float64Array): void {
  const at = (r: number, c: number): number => 2 * (r * n + c);
  for (let col = 0; col < n; col += 1) {
    let pivot = col;
    let best = 0;
    for (let r = col; r < n; r += 1) {
      const size = Math.hypot(matrix[at(r, col)] ?? 0, matrix[at(r, col) + 1] ?? 0);
      if (size > best) {
        best = size;
        pivot = r;
      }
    }
    if (!(best > 0)) {
      throw new Error("the mean-field system is singular, a broken invariant of the MMF");
    }
    if (pivot !== col) {
      for (let c = 0; c < n; c += 1) {
        for (let part = 0; part < 2; part += 1) {
          const t = matrix[at(col, c) + part] ?? 0;
          matrix[at(col, c) + part] = matrix[at(pivot, c) + part] ?? 0;
          matrix[at(pivot, c) + part] = t;
        }
      }
      for (let part = 0; part < 2; part += 1) {
        const t = vector[2 * col + part] ?? 0;
        vector[2 * col + part] = vector[2 * pivot + part] ?? 0;
        vector[2 * pivot + part] = t;
      }
    }
    const pRe = matrix[at(col, col)] ?? 0;
    const pIm = matrix[at(col, col) + 1] ?? 0;
    const pNorm = pRe * pRe + pIm * pIm;
    for (let r = col + 1; r < n; r += 1) {
      const eRe = matrix[at(r, col)] ?? 0;
      const eIm = matrix[at(r, col) + 1] ?? 0;
      // f = e ÷ p.
      const fRe = (eRe * pRe + eIm * pIm) / pNorm;
      const fIm = (eIm * pRe - eRe * pIm) / pNorm;
      for (let c = col; c < n; c += 1) {
        const [mRe, mIm] = cMul(fRe, fIm, matrix[at(col, c)] ?? 0, matrix[at(col, c) + 1] ?? 0);
        matrix[at(r, c)] = (matrix[at(r, c)] ?? 0) - mRe;
        matrix[at(r, c) + 1] = (matrix[at(r, c) + 1] ?? 0) - mIm;
      }
      const [vRe, vIm] = cMul(fRe, fIm, vector[2 * col] ?? 0, vector[2 * col + 1] ?? 0);
      vector[2 * r] = (vector[2 * r] ?? 0) - vRe;
      vector[2 * r + 1] = (vector[2 * r + 1] ?? 0) - vIm;
    }
  }
  for (let r = n - 1; r >= 0; r -= 1) {
    let sRe = vector[2 * r] ?? 0;
    let sIm = vector[2 * r + 1] ?? 0;
    for (let c = r + 1; c < n; c += 1) {
      const [mRe, mIm] = cMul(
        matrix[at(r, c)] ?? 0,
        matrix[at(r, c) + 1] ?? 0,
        vector[2 * c] ?? 0,
        vector[2 * c + 1] ?? 0,
      );
      sRe -= mRe;
      sIm -= mIm;
    }
    const pRe = matrix[at(r, r)] ?? 0;
    const pIm = matrix[at(r, r) + 1] ?? 0;
    const pNorm = pRe * pRe + pIm * pIm;
    vector[2 * r] = (sRe * pRe + sIm * pIm) / pNorm;
    vector[2 * r + 1] = (sIm * pRe - sRe * pIm) / pNorm;
  }
}

// ---------------------------------------------------------------------------------------------
// The mean field.

/**
 * The Legendre integrals a(ν, n, p) and b(ν, n, p) of Tazaki and Tanaka's (29)–(30),
 * (2p + 1) ÷ 2 ∫₋₁¹ P_ν¹ P_n¹ Pₚ dx and the same with Pₚ′, for ν, n = 1 … terms and
 * p = 0 … 2 terms, by a Gauss–Legendre rule exact for their degree. Indexed [(ν − 1) terms + n − 1]
 * then p.
 */
function legendreIntegrals(terms: number): {
  readonly a: Float64Array;
  readonly b: Float64Array;
} {
  const pMax = 2 * terms;
  const rule = gaussLegendre(2 * terms + 8);
  const nodes = rule.x.length;
  // P_n¹ P_ν¹ = (1 − x²) P_n′ P_ν′, whichever sign convention P_n¹ takes.
  const pn = new Float64Array((pMax + 1) * nodes);
  const dpn = new Float64Array((pMax + 1) * nodes);
  for (let i = 0; i < nodes; i += 1) {
    const x = rule.x[i] ?? 0;
    pn[i] = 1;
    dpn[i] = 0;
    pn[nodes + i] = x;
    dpn[nodes + i] = 1;
    for (let n = 1; n < pMax; n += 1) {
      const p0 = pn[n * nodes + i] ?? 0;
      pn[(n + 1) * nodes + i] =
        ((2 * n + 1) * x * p0 - n * (pn[(n - 1) * nodes + i] ?? 0)) / (n + 1);
      dpn[(n + 1) * nodes + i] = (dpn[(n - 1) * nodes + i] ?? 0) + (2 * n + 1) * p0;
    }
  }
  const size = terms * terms * (pMax + 1);
  const a = new Float64Array(size);
  const b = new Float64Array(size);
  for (let nu = 1; nu <= terms; nu += 1) {
    for (let n = 1; n <= terms; n += 1) {
      const base = ((nu - 1) * terms + (n - 1)) * (pMax + 1);
      for (let p = Math.abs(n - nu); p <= n + nu; p += 1) {
        let sa = 0;
        let sb = 0;
        for (let i = 0; i < nodes; i += 1) {
          const x = rule.x[i] ?? 0;
          const assoc =
            (1 - x * x) * (dpn[nu * nodes + i] ?? 0) * (dpn[n * nodes + i] ?? 0) * (rule.w[i] ?? 0);
          sa += assoc * (pn[p * nodes + i] ?? 0);
          sb += assoc * (dpn[p * nodes + i] ?? 0);
        }
        a[base + p] = 0.5 * (2 * p + 1) * sa;
        b[base + p] = 0.5 * (2 * p + 1) * sb;
      }
    }
  }
  return { a, b };
}

/**
 * The mean-field coefficients d¹ₙ, d²ₙ from the monomer's Mie coefficients, in Bohren and Huffman's
 * convention, in which Tazaki and Tanaka write the translation coefficients with h⁽¹⁾.
 */
function meanFieldCoefficients(
  monomer: MieCoefficients,
  aggregate: Aggregate,
  xg: number,
): MieCoefficients {
  const terms = monomer.aRe.length;
  const pMax = 2 * terms;
  const sp = structureIntegrals(aggregate.df, xg, pMax);
  const { a: legA, b: legB } = legendreIntegrals(terms);
  // T₁(ν, n) and T₂(ν, n), Tazaki and Tanaka's Ā and B̄ ((14)–(15)), as (re, im).
  const t1 = new Float64Array(2 * terms * terms);
  const t2 = new Float64Array(2 * terms * terms);
  for (let nu = 1; nu <= terms; nu += 1) {
    for (let n = 1; n <= terms; n += 1) {
      const base = ((nu - 1) * terms + (n - 1)) * (pMax + 1);
      let aRe = 0;
      let aIm = 0;
      let bRe = 0;
      let bIm = 0;
      for (let p = Math.abs(n - nu); p <= n + nu; p += 1) {
        const factor = (n * (n + 1) + nu * (nu + 1) - p * (p + 1)) * (legA[base + p] ?? 0);
        const spRe = sp[2 * p] ?? 0;
        const spIm = sp[2 * p + 1] ?? 0;
        aRe += factor * spRe;
        aIm += factor * spIm;
        bRe += (legB[base + p] ?? 0) * spRe;
        bIm += (legB[base + p] ?? 0) * spIm;
      }
      const scale = (2 * nu + 1) / (n * (n + 1) * nu * (nu + 1));
      const k = 2 * ((nu - 1) * terms + (n - 1));
      t1[k] = scale * aRe;
      t1[k + 1] = scale * aIm;
      t2[k] = 2 * scale * bRe;
      t2[k + 1] = 2 * scale * bIm;
    }
  }
  const size = 2 * terms;
  const system = new Float64Array(2 * size * size);
  const vector = new Float64Array(2 * size);
  const others = aggregate.count - 1;
  // Bohren and Huffman's aₙ, bₙ are the conjugates of mieCoefficients'.
  for (let n = 1; n <= terms; n += 1) {
    const anRe = monomer.aRe[n - 1] ?? 0;
    const anIm = -(monomer.aIm[n - 1] ?? 0);
    const bnRe = monomer.bRe[n - 1] ?? 0;
    const bnIm = -(monomer.bIm[n - 1] ?? 0);
    const rowA = 2 * (n - 1);
    const rowB = rowA + 1;
    for (let nu = 1; nu <= terms; nu += 1) {
      const k = 2 * ((nu - 1) * terms + (n - 1));
      const colA = 2 * (nu - 1);
      const colB = colA + 1;
      const put = (row: number, col: number, [re, im]: [number, number]): void => {
        system[2 * (row * size + col)] = others * re;
        system[2 * (row * size + col) + 1] = others * im;
      };
      put(rowA, colA, cMul(anRe, anIm, t1[k] ?? 0, t1[k + 1] ?? 0));
      put(rowA, colB, cMul(anRe, anIm, t2[k] ?? 0, t2[k + 1] ?? 0));
      put(rowB, colA, cMul(bnRe, bnIm, t2[k] ?? 0, t2[k + 1] ?? 0));
      put(rowB, colB, cMul(bnRe, bnIm, t1[k] ?? 0, t1[k + 1] ?? 0));
    }
    vector[2 * rowA] = anRe;
    vector[2 * rowA + 1] = anIm;
    vector[2 * rowB] = bnRe;
    vector[2 * rowB + 1] = bnIm;
  }
  for (let i = 0; i < size; i += 1) {
    system[2 * (i * size + i)] = (system[2 * (i * size + i)] ?? 0) + 1;
  }
  solveComplex(size, system, vector);
  const out = {
    aRe: new Float64Array(terms),
    aIm: new Float64Array(terms),
    bRe: new Float64Array(terms),
    bIm: new Float64Array(terms),
  };
  // Back to mieCoefficients' convention (conjugates) for mieAmplitudes.
  for (let n = 0; n < terms; n += 1) {
    out.aRe[n] = vector[4 * n] ?? 0;
    out.aIm[n] = -(vector[4 * n + 1] ?? 0);
    out.bRe[n] = vector[4 * n + 2] ?? 0;
    out.bIm[n] = -(vector[4 * n + 3] ?? 0);
  }
  return out;
}

/** The scattering-angle panels for the aggregate's angular integrals: 2° wide, order 8. */
const ANGLE_PANELS = 90;
const ANGLE_RULE = gaussLegendre(8);

interface AngularSums {
  /** ∫ S₁₁ sin θ dθ of the aggregate. */
  readonly s11: number;
  /** ∫ S₁₁ cos θ sin θ dθ. */
  readonly s11Cos: number;
}

/** The aggregate's S₁₁ and the matrix sums at the cosines asked, from the mean-field amplitudes. */
function aggregateMatrix(
  field: MieCoefficients,
  aggregate: Aggregate,
  wavenumberPerUm: number,
  rgUm: number,
  mu: Float64Array,
): {
  readonly a1: Float64Array;
  readonly a3: Float64Array;
  readonly b1: Float64Array;
  readonly b2: Float64Array;
} {
  const { s1, s2 } = mieAmplitudes(field, mu);
  const n = mu.length;
  const out = {
    a1: new Float64Array(n),
    a3: new Float64Array(n),
    b1: new Float64Array(n),
    b2: new Float64Array(n),
  };
  const { count, df } = aggregate;
  for (let i = 0; i < n; i += 1) {
    const cosine = mu[i] ?? 0;
    // q² = 4k² sin²(θ ÷ 2) = 2k²(1 − cos θ).
    const q2 = 2 * wavenumberPerUm * wavenumberPerUm * (1 - cosine);
    const structure = kummerNegative(df / 2, 1.5, (q2 * rgUm * rgUm) / df);
    const factor = count * (1 + (count - 1) * structure);
    const s1Re = s1[2 * i] ?? 0;
    const s1Im = s1[2 * i + 1] ?? 0;
    const s2Re = s2[2 * i] ?? 0;
    const s2Im = s2[2 * i + 1] ?? 0;
    const s1Sq = s1Re * s1Re + s1Im * s1Im;
    const s2Sq = s2Re * s2Re + s2Im * s2Im;
    out.a1[i] = factor * 0.5 * (s1Sq + s2Sq);
    out.b1[i] = factor * 0.5 * (s2Sq - s1Sq);
    out.a3[i] = factor * (s1Re * s2Re + s1Im * s2Im);
    // T5.b's b₂ = −Im(S₁S₂*) in mieSphere's convention.
    out.b2[i] = factor * (s1Re * s2Im - s1Im * s2Re);
  }
  return out;
}

/** ∫ S₁₁ sin θ dθ and ∫ S₁₁ cos θ sin θ dθ over 0–π, by Gauss–Legendre panels in θ. */
function angularSums(
  field: MieCoefficients,
  aggregate: Aggregate,
  wavenumberPerUm: number,
  rgUm: number,
): AngularSums {
  const order = ANGLE_RULE.x.length;
  const theta = new Float64Array(ANGLE_PANELS * order);
  const weight = new Float64Array(ANGLE_PANELS * order);
  const half = Math.PI / ANGLE_PANELS / 2;
  for (let p = 0; p < ANGLE_PANELS; p += 1) {
    for (let i = 0; i < order; i += 1) {
      const t = (2 * p + 1 + (ANGLE_RULE.x[i] ?? 0)) * half;
      theta[p * order + i] = t;
      weight[p * order + i] = half * (ANGLE_RULE.w[i] ?? 0) * Math.sin(t);
    }
  }
  const mu = theta.map(Math.cos);
  const { a1 } = aggregateMatrix(field, aggregate, wavenumberPerUm, rgUm, mu);
  let s11 = 0;
  let s11Cos = 0;
  for (let i = 0; i < mu.length; i += 1) {
    const w = (weight[i] ?? 0) * (a1[i] ?? 0);
    s11 += w;
    s11Cos += w * (mu[i] ?? 0);
  }
  return { s11, s11Cos };
}

/**
 * One aggregate's MMF optics at one wavelength, before the phase-shift gate.
 *
 * @param index - The monomers' refractive index.
 * @param wavelengthNm - The vacuum wavelength, nm, finite and > 0.
 * @param mu - The cosines to give the matrix at; the phase tables' by default.
 * @throws RangeError for an aggregate outside {@link AggregateMode}'s ranges, a monomer past
 *   {@link MMF_MONOMER_SIZE_PARAMETER_MAX}, or a bad wavelength or index.
 */
export function mmfAggregate(
  shape: Omit<AggregateMode, "material" | "form">,
  index: ComplexIndex,
  wavelengthNm: number,
  mu: Float64Array = PHASE_TABLE_MU,
): MmfResult {
  const aggregate = checkAggregate(shape);
  if (!(wavelengthNm > 0 && Number.isFinite(wavelengthNm))) {
    throw new RangeError(`wavelength ${wavelengthNm} nm must be finite and > 0`);
  }
  const k = (2 * Math.PI * 1000) / wavelengthNm;
  const { a0, count, df, kf } = aggregate;
  const x0 = k * a0;
  if (x0 > MMF_MONOMER_SIZE_PARAMETER_MAX) {
    throw new RangeError(
      `monomer size parameter ${x0} exceeds the MMF's ${MMF_MONOMER_SIZE_PARAMETER_MAX} here`,
    );
  }
  const rg = a0 * (count / kf) ** (1 / df);
  const rc = Math.sqrt(5 / 3) * rg;
  const xg = k * rg;
  const monomer = mieCoefficients(x0, index);
  const field = count > 1 ? meanFieldCoefficients(monomer, aggregate, xg) : monomer;

  const terms = monomer.aRe.length;
  let extSum = 0;
  let absSum = 0;
  for (let n = 0; n < terms; n += 1) {
    const w = 2 * n + 3;
    extSum += w * ((field.aRe[n] ?? 0) + (field.bRe[n] ?? 0));
    const aRe = monomer.aRe[n] ?? 0;
    const aIm = monomer.aIm[n] ?? 0;
    const bRe = monomer.bRe[n] ?? 0;
    const bIm = monomer.bIm[n] ?? 0;
    absSum += w * (aRe - aRe * aRe - aIm * aIm + bRe - bRe * bRe - bIm * bIm);
  }
  const perK2 = (2 * Math.PI) / (k * k);
  const cExt = count * perK2 * extSum;
  const cAbsRgd = count * perK2 * absSum;
  const sums = angularSums(field, aggregate, k, rg);
  const cScaMeanField = (2 * Math.PI * sums.s11) / (k * k);
  const geometric =
    (count * Math.PI * a0 * a0) / (1 + (count - 1) * meanOverlapEfficiency(aggregate));
  const tau = cAbsRgd / geometric;
  const cAbs = Math.max(geometric * -Math.expm1(-tau), cExt - cScaMeanField);
  const cSca = cExt - cAbs;

  const sums4pi = (4 * Math.PI) / (k * k * cScaMeanField);
  const raw = aggregateMatrix(field, aggregate, k, rg, mu);
  const a1 = raw.a1.map((v) => v * sums4pi);
  const a3 = raw.a3.map((v) => v * sums4pi);
  const matrix: ScatteringMatrix = {
    mu: mu.slice(),
    forwardPeak: 0,
    a1,
    a2: a1.slice(),
    a3,
    a4: a3.slice(),
    b1: raw.b1.map((v) => v * sums4pi),
    b2: raw.b2.map((v) => v * sums4pi),
  };

  const filling = count * (a0 / rc) ** 3;
  const phaseShift = 2 * k * rc * maxwellGarnettContrast(index, filling);
  const monomerPhaseShift = 2 * x0 * Math.hypot(index.n - 1, index.k);
  return {
    extinctionCrossSectionUm2: cExt,
    scatteringCrossSectionUm2: cSca,
    asymmetry: sums.s11Cos / sums.s11,
    geometricCrossSectionUm2: geometric,
    phaseShift,
    monomerPhaseShift,
    matrix,
  };
}

/**
 * |m_MG − 1| for the Maxwell–Garnett index of monomers of index m in vacuum at filling factor f:
 * ε_MG = (ε + 2 + 2f(ε − 1)) ÷ (ε + 2 − f(ε − 1)), ε = m².
 */
function maxwellGarnettContrast(index: ComplexIndex, filling: number): number {
  const eRe = index.n * index.n - index.k * index.k;
  const eIm = 2 * index.n * index.k;
  const numRe = eRe + 2 + 2 * filling * (eRe - 1);
  const numIm = eIm + 2 * filling * eIm;
  const denRe = eRe + 2 - filling * (eRe - 1);
  const denIm = eIm - filling * eIm;
  const denNorm = denRe * denRe + denIm * denIm;
  const mgRe = (numRe * denRe + numIm * denIm) / denNorm;
  const mgIm = (numIm * denRe - numRe * denIm) / denNorm;
  // The principal square root of ε_MG.
  const modulus = Math.hypot(mgRe, mgIm);
  const sRe = Math.sqrt(0.5 * (modulus + mgRe));
  const sIm = Math.sign(mgIm) * Math.sqrt(0.5 * (modulus - mgRe));
  return Math.hypot(sRe - 1, sIm);
}

/** Tazaki and Tanaka 2018's (9): the larger of the monomer's and the aggregate's phase shift. */
export function mmfPhaseShift(result: Pick<MmfResult, "phaseShift" | "monomerPhaseShift">): number {
  return Math.max(result.phaseShift, result.monomerPhaseShift);
}

/**
 * The basis an aggregate's phase rests on (science-r08-mmf.md §1.3): `model` while (9) is below
 * {@link MMF_PHASE_SHIFT_LIMIT}; past it, `analogue` (the MMF table kept) while the aggregate's
 * own term is below the limit, and `fallback` (Henyey–Greenstein) once it reaches it.
 */
export function mmfPhaseBasis(
  result: Pick<MmfResult, "phaseShift" | "monomerPhaseShift">,
): "model" | "analogue" | "fallback" {
  if (mmfPhaseShift(result) < MMF_PHASE_SHIFT_LIMIT) {
    return "model";
  }
  return result.phaseShift < MMF_PHASE_SHIFT_LIMIT ? "analogue" : "fallback";
}

/**
 * An aggregate mode's optics at one wavelength (Design note 6): the MMF's cross-sections, with its
 * table and matrix as a `model` inside (9)'s gate and as an `analogue` past it while the
 * aggregate's own term is below 1; otherwise a labelled Henyey–Greenstein `fallback` from the
 * MMF asymmetry, with no matrix ({@link mmfPhaseBasis}).
 *
 * @param wavelengthNm - The vacuum wavelength, nm, in 380–780.
 * @param mu - The cosines to give the matrix at; the phase tables' by default.
 * @throws RangeError as {@link mmfAggregate}, or for a wavelength outside the files' range.
 */
export function aggregateOptics(
  mode: AggregateMode,
  wavelengthNm: number,
  mu: Float64Array = PHASE_TABLE_MU,
): AggregateOptics {
  const material = resolveMaterial(mode.material, mode.form);
  const index = indexAt(material, wavelengthNm);
  const result = mmfAggregate(mode, index, wavelengthNm, mu);
  const basis = mmfPhaseBasis(result);
  const geometricM2 = result.geometricCrossSectionUm2 * 1e-12;
  const extM2 = result.extinctionCrossSectionUm2 * 1e-12;
  const scaM2 = result.scatteringCrossSectionUm2 * 1e-12;
  const shifts = `Δφ ${mmfPhaseShift(result).toFixed(2)} by Tazaki and Tanaka's (9) (aggregate term ${result.phaseShift.toFixed(2)})`;
  let note: string | undefined;
  switch (basis) {
    case "model":
      note = undefined;
      break;
    case "analogue":
      note = `an aggregate of ${mode.material}, ${shifts}, past the gate: the MMF table, an analogue`;
      break;
    case "fallback":
      note = `an aggregate of ${mode.material}, ${shifts}, its own term past the gate: a Henyey–Greenstein phase from its MMF asymmetry, a fallback`;
      break;
  }
  const hasTable = basis !== "fallback";
  return {
    wavelengthNm,
    qExt: extM2 / geometricM2,
    qSca: scaM2 / geometricM2,
    singleScatteringAlbedo: scaM2 / extM2,
    asymmetry: result.asymmetry,
    geometricCrossSectionM2: geometricM2,
    extinctionCrossSectionM2: extM2,
    scatteringCrossSectionM2: scaM2,
    volumeM3: mode.monomerCount * (4 / 3) * Math.PI * mode.monomerRadiusUm ** 3 * 1e-18,
    matrix: hasTable ? keptMatrix(result.matrix) : undefined,
    shape: "aggregate",
    provenance: material.provenance,
    phaseBasis: basis,
    phaseModel: hasTable ? "mmf" : "henyeyGreenstein",
    phaseNote: note,
    phaseShift: result.phaseShift,
    monomerPhaseShift: result.monomerPhaseShift,
  };
}
