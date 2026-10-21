/**
 * The plane-parallel discrete-ordinates solver (plan R08, R08.T14.a; Design notes 9 and 10): the
 * scalar radiative-transfer equation for a collimated beam on a stack of homogeneous, absorbing and
 * anisotropically scattering layers over a Lambertian ground, solved in `f64` one azimuthal mode at
 * a time, every beam cosine on one factorisation of the boundary-value problem.
 *
 * @remarks
 * Ported from the papers, never from GPL cdisort: K. Stamnes, S.-C. Tsay, W. Wiscombe and K.
 * Jayaweera, "Numerically stable algorithm for discrete-ordinate-method radiative transfer in
 * multiple scattering and emitting layered media", Appl. Opt. 27(12) (1988) 2502–2509,
 * doi:10.1364/AO.27.002502 (the method); K. Stamnes and R. A. Swanson, "A new look at the discrete
 * ordinate method for radiative transfer calculations in anisotropically scattering atmospheres",
 * J. Atmos. Sci. 38(2) (1981) 387–399 (the N × N reduction, as Stamnes et al. 1988 credit it); and
 * K. Stamnes and P. Conklin, "A new multi-layer discrete ordinate approach to radiative transfer in
 * vertically inhomogeneous atmospheres", JQSRT 31(3) (1984) 273–282,
 * doi:10.1016/0022-4073(84)90031-1 (the scaling that keeps every exponential at or below 1).
 *
 * **The equation** (Stamnes et al. 1988, eqs. 4–6, with the normalised Λ_l^m in place of their
 * P_l^m and (l − m)! ÷ (l + m)!). τ is the vertical optical depth from the top, μ the cosine of a
 * stream's direction of travel, positive upward. With I = Σ_m I^m cos(mΔφ) (see
 * {@link BeamSolution} for Δφ), each mode m obeys
 * μ dI^m ÷ dτ = I^m − (ω ÷ 2) ∫₋₁¹ D^m(μ, μ′) I^m(τ, μ′) dμ′ − Q^m(τ, μ), where
 * D^m(μ, μ′) = Σ_{l=m}^{NSTR−1} (2l + 1) χ_l Λ_l^m(μ) Λ_l^m(μ′),
 * Q^m(τ, μ) = (ω ÷ 4π)(2 − δ_m0) Σ_l (2l + 1) χ_l Λ_l^m(μ) Λ_l^m(−μ₀) T(τ),
 * χ_l = ∫ p P_l dΩ the phase function's moments (p per steradian, χ₀ = 1, `aerosol.ts`'s
 * convention), Λ_l^m the normalised associated Legendre functions
 * ({@link normalizedAssociatedLegendre}), and T(τ) the beam's transmission, its flux on a plane
 * normal to it being 1. The integral is taken on the double-Gauss quadrature
 * ({@link doubleGaussQuadrature}).
 *
 * **Per layer and mode** (Stamnes et al. 1988, eqs. 7–8e): with M = diag(μ_i), W = diag(w_i) and
 * S± = ω Σ_{l+m even/odd} (2l + 1) χ_l Λ_l Λ_lᵀ on the nodes, the homogeneous solutions
 * I^±(τ) = G^± e^{−kτ} come from the reduced eigenproblem (α − β)(α + β) X = k² X,
 * α ± β = M⁻¹(S±W − 1), X = G⁺ + G⁻ and G⁺ − G⁻ = k (α − β)⁻¹ X. Writing
 * −P = M^{−½}(1 − W^{½}S⁻W^{½})M^{−½} = L Lᵀ (Cholesky) and −Q likewise from S⁺, the product
 * (α − β)(α + β) is similar to (−P)(−Q), and so to the symmetric Lᵀ(−Q)L, X ∝ D L y with
 * D = W^{−½}M^{−½}: the symmetric-definite problem B A z = k² z with B = L Lᵀ reduced to
 * C = Lᵀ A L, z = L y (Golub and Van Loan, _Matrix Computations_, 4th ed., §8.7; E. Anderson et
 * al., _LAPACK Users' Guide_, 3rd ed., 1999, Table 2.13, type 3), solved by cyclic Jacobi
 * (`linearAlgebra.ts`). −P is the one factorised, since at ω = 1 −Q holds the conservative null.
 * The eigenvalues k² are real, and positive while the truncated expansion's discrete scattering
 * operator is a contraction. The raw moments of a strongly forward-peaked phase function need not
 * give one, since the double-Gauss rule integrates Λ_l Λ_l′ exactly only to l + l′ = 2N − 1:
 * Henyey–Greenstein's χ_l = g^l fail from g about 0.944 at 16 streams and ω 1 (0.952 at ω 0.9,
 * 0.99 at 32 streams), and are refused, while their delta-M truncations (f = χ_NSTR) passed for g
 * up to 0.999 (R08.T14.a's science check, 2026-10-21). G⁺ − G⁻ is taken as k (α − β)⁻¹ X, not as
 * the equal (α + β) X ÷ k, so that it stays finite as k → 0. The beam's particular
 * solution Z^± T e^{−λt} (λ the slant factor, 1 ÷ μ₀ plane-parallel) solves
 * ((α − β)(α + β) − λ²) Zs = −(α − β)M⁻¹Xs − λM⁻¹Xd on the same eigenvectors, so a beam costs
 * no factorisation.
 *
 * **Conservative scattering.** At m = 0 and ω = 1 one eigenvalue is 0 and its pair e^{∓kt}
 * degenerates. That pair is replaced by the exact solutions I = 1 and I(t, ±μ_i) = t ± u_i,
 * u = −(α − β)⁻¹1 (for isotropic scattering u_i = μ_i, the diffusion solution I = τ + μ): no
 * dither of ω. A co-albedo at or below {@link DISCRETE_ORDINATE_CONSERVATIVE_CO_ALBEDO} is solved
 * as conservative.
 *
 * **A beam at an eigenvalue.** Where λ = k_n the particular solution is singular, though the
 * solution is not (it gains a t e^{−kt} term). For a layer whose λ lies within
 * {@link RESONANCE_HALF_WIDTH} of a k_n, relatively, the mode is solved with that layer's λ moved
 * by twice that up and down, and the two solutions averaged: the solution is analytic in λ, so
 * the average's error is about (2h)² ÷ 2 of its second derivative, and each side's rounding about
 * ε ÷ h, h the half-width and ε the `f64` epsilon.
 *
 * **The boundary-value problem** (Stamnes and Conklin 1984): in each layer the decaying solutions
 * are scaled to its top and the growing ones to its bottom, so that every exponential is e^{−x},
 * x ≥ 0, at most 1. No diffuse light enters the top; the intensities are continuous
 * at each interface; the ground reflects Lambertian, I^0(τ_L, μ_i) = 2A Σ_j w_j μ_j I^0(τ_L, −μ_j)
 * + (A ÷ π) μ₀ T(τ_L), and modes past 0 not at all. The 2N L equations are banded, 3N − 1 either
 * side of the diagonal, and are factorised once a mode by banded elimination with partial pivoting
 * (`linearAlgebra.ts`), then solved for each beam ({@link factorLayerStack}).
 *
 * **User angles** (Stamnes et al. 1988): the intensity at a cosine off the quadrature is the
 * formal solution, the source function integrated analytically through each layer from the
 * boundary on the side the stream comes from. Each term of the source is an exponential, and its
 * integral a divided difference of two exponentials, taken stably where the two rates meet (a
 * user cosine at μ₀ or at a 1 ÷ k_n), so that no user angle is singular.
 *
 * Out of scope here (plan R08): delta-M and the Nakajima–Tanaka corrections and the optics worker
 * (R08.T14.b), thermal emission (R08.T18), a ground other than Lambertian (R08.T15) and the
 * vector mode (R08.T14.e). Pure: no state, no clock, no platform API.
 */

import { gaussLegendre } from "../irradianceQuadrature";
import {
  type BandLu,
  bandIndex,
  bandMatrix,
  choleskyLower,
  factorBand,
  solveBand,
  solveLower,
  solveLowerTransposed,
  symmetricEigen,
} from "./linearAlgebra";

/**
 * The streams the solver takes by default: 16 in all, 8 a hemisphere, Stamnes et al. 1988's NSTR
 * (plan R08, Design note 9; ruled 2026-10-21, decision-r08-t14a-scope item 3).
 *
 * @remarks
 * It counts both hemispheres, the count the same ruling reads in R08.T14.e's "8 streams".
 * `aerosol.ts`'s `DELTA_M_STREAMS` = 16 counts a hemisphere instead; R08.T14.b reconciles the
 * two.
 */
export const DISCRETE_ORDINATE_STREAMS = 16;

/**
 * The co-albedo 1 − ω at and below which a layer is solved as conservative, ω = 1.
 *
 * @remarks
 * Near ω = 1 the smallest k² of the m = 0 eigenproblem goes as diffusion's 3(1 − ω)(1 − ωχ₁),
 * and the general solution holds it while Jacobi's rounding stays below it. Measured with this
 * bound set to 0 on Henyey–Greenstein g 0.85 (2026-10-21): k₀² is within 0.2% of 0.45(1 − ω) down
 * to a co-albedo of 10⁻¹² at 16, 32 and 64 streams, about 1% off at 10⁻¹³ and up to about 15% at
 * 10⁻¹⁴ (figures that move with the order of summation), and at 10⁻¹⁵ it reaches rounding (at 64
 * streams, k₀² < 0). Taken as conservative, a layer of
 * co-albedo c drops an absorption of about c times the scatterings a photon makes there: a slab of
 * τ 1,000 lit at μ₀ 0.5 absorbs about 870c of the beam's flux normal to it, so the rule drops at
 * most 9 × 10⁻¹⁰ there (`cloudOptics.ts`'s `CONSERVATIVE_CO_ALBEDO` is the same figure for the
 * two-stream).
 */
export const DISCRETE_ORDINATE_CONSERVATIVE_CO_ALBEDO = 1e-12;

/**
 * Half the width, relative to λ, of the window about an eigenvalue k_n inside which a layer's
 * beam is moved off it (the module's remarks, "A beam at an eigenvalue").
 *
 * @remarks
 * The average's error goes as about 4h² from its interpolation and 4 × 10⁻¹⁵ ÷ h from rounding:
 * measured on a conservative slab (τ 3, Henyey–Greenstein g 0.85, μ₀ at each 1 ÷ k_n ≤ 1), the
 * energy balance misses by 4 × 10⁻⁸ at h = 10⁻⁴, 6 × 10⁻¹⁰ at 10⁻⁵ and 1.2 × 10⁻⁹ at 3 × 10⁻⁶
 * (against 4 × 10⁻¹⁴ off the window; 2026-10-21), so 10⁻⁵ is near the balance.
 */
export const RESONANCE_HALF_WIDTH = 1e-5;

/** How far χ₀ may sit from 1, the rounding a phase table's normalisation leaves. */
const ZEROTH_MOMENT_TOLERANCE = 1e-12;

/**
 * How near 0, relative to the largest k² (or to 1), a conservative layer's smallest k² must be:
 * a check that the m = 0 null vector is there, not a tuning. Measured, |k₀²| is at most
 * 7 × 10⁻¹⁶ at 16 to 64 streams, against a largest k² of about 1 ÷ μ₁² (2,500 to 5 × 10⁵, μ₁ the
 * smallest node; 2026-10-21), so the bound sits ten orders above it and as far below the next k².
 */
const CONSERVATIVE_NULL_TOLERANCE = 1e-8;

// ---------------------------------------------------------------------------------------------
// The quadrature and the Legendre functions.

/** The double-Gauss quadrature: an N-point Gauss–Legendre rule on each hemisphere. */
export interface DoubleGaussQuadrature {
  /** NSTR, the streams in all: 2N. */
  readonly streams: number;
  /** μ_i, i < N, ascending in (0, 1): the cosines of each hemisphere's streams. */
  readonly nodes: Float64Array;
  /** w_i, summing to 1 over a hemisphere: ∫₀¹ f dμ ≈ Σ w_i f(μ_i), exact to degree 2N − 1. */
  readonly weights: Float64Array;
}

/**
 * The double-Gauss quadrature of NSTR streams: `irradianceQuadrature.ts`'s
 * {@link gaussLegendre} rule of N = NSTR ÷ 2 nodes on [0, 1], for each hemisphere, as Stamnes et
 * al. 1988 take it.
 *
 * @remarks
 * Exact on each hemisphere to degree 2N − 1, so that the half-range moments of every Legendre
 * polynomial the solve carries (degree ≤ NSTR − 1) are exact, and with them the m = 0 energy
 * balance at ω = 1.
 *
 * @throws RangeError for a stream count that is not an even integer of at least 2.
 */
export function doubleGaussQuadrature(
  streams: number = DISCRETE_ORDINATE_STREAMS,
): DoubleGaussQuadrature {
  requireStreams(streams);
  const rule = gaussLegendre(streams / 2);
  return { streams, nodes: rule.nodes, weights: rule.weights };
}

/**
 * The normalised associated Legendre functions Λ_l^m(μ) = √((l − m)! ÷ (l + m)!) P_l^m(μ) for
 * l = 0 … `maxDegree`, 0 below l = m, without the Condon–Shortley phase.
 *
 * @remarks
 * By the stable recurrences in l at fixed m: Λ_m^m = Π_{i≤m} √((2i − 1) ÷ 2i) (1 − μ²)^{m/2},
 * Λ_{m+1}^m = √(2m + 1) μ Λ_m^m and
 * √((l − m)(l + m)) Λ_l^m = (2l − 1) μ Λ_{l−1}^m − √((l − 1 − m)(l − 1 + m)) Λ_{l−2}^m, the
 * normalised form of (l − m) P_l^m = (2l − 1) μ P_{l−1}^m − (l + m − 1) P_{l−2}^m (M. Abramowitz and
 * I. A. Stegun, _Handbook of Mathematical Functions_, 1964, §8.5; W. H. Press et al., _Numerical
 * Recipes_, 3rd ed., §6.7; both cited, not re-read: the normalised forms were derived here, and the
 * tests hold Λ_l^0 to P_l and the set to the addition theorem).
 * Λ_l^0 = P_l. The phase is immaterial here, since the solve takes them in products at one m;
 * Λ_l^m(−μ) = (−1)^{l+m} Λ_l^m(μ) holds either way. With them the addition theorem reads
 * P_l(cos Θ) = Σ_m (2 − δ_m0) Λ_l^m(μ) Λ_l^m(μ′) cos(mΔφ).
 *
 * @param order - m, a whole number ≥ 0.
 * @param maxDegree - The largest l, a whole number ≥ 0.
 * @param mu - The cosine, in [−1, 1].
 * @returns Λ_l^m(μ) at index l.
 * @throws RangeError for an order or degree that is not a whole number ≥ 0, or a cosine outside
 *   [−1, 1].
 */
export function normalizedAssociatedLegendre(
  order: number,
  maxDegree: number,
  mu: number,
): Float64Array {
  if (!Number.isInteger(order) || order < 0 || !Number.isInteger(maxDegree) || maxDegree < 0) {
    throw new RangeError(`Λ_l^m needs whole m and l ≥ 0, not m ${order} and l ${maxDegree}`);
  }
  requireIn("mu", mu, (v) => v >= -1 && v <= 1, "in [−1, 1]");
  const out = new Float64Array(maxDegree + 1);
  legendreInto(order, maxDegree, mu, out, 0, 1);
  return out;
}

/** Λ_l^m(μ), l = 0 … `maxDegree`, into `out[offset + l * stride]`. */
function legendreInto(
  m: number,
  maxDegree: number,
  mu: number,
  out: Float64Array,
  offset: number,
  stride: number,
): void {
  if (m > maxDegree) {
    return;
  }
  const across = Math.sqrt(Math.max((1 - mu) * (1 + mu), 0));
  let diagonal = 1;
  for (let i = 1; i <= m; i += 1) {
    diagonal *= Math.sqrt((2 * i - 1) / (2 * i)) * across;
  }
  out[offset + m * stride] = diagonal;
  if (m + 1 > maxDegree) {
    return;
  }
  let previous = diagonal;
  let current = Math.sqrt(2 * m + 1) * mu * diagonal;
  out[offset + (m + 1) * stride] = current;
  for (let l = m + 2; l <= maxDegree; l += 1) {
    const next =
      ((2 * l - 1) * mu * current - Math.sqrt((l - 1 - m) * (l - 1 + m)) * previous) /
      Math.sqrt((l - m) * (l + m));
    out[offset + l * stride] = next;
    previous = current;
    current = next;
  }
}

/** Λ_l^m at each cosine, (maxDegree + 1) × count, entry (l, i) at `l * count + i`. */
function legendreTable(m: number, maxDegree: number, cosines: Float64Array): Float64Array {
  const count = cosines.length;
  const table = new Float64Array((maxDegree + 1) * count);
  for (let i = 0; i < count; i += 1) {
    legendreInto(m, maxDegree, at(cosines, i), table, i, count);
  }
  return table;
}

// ---------------------------------------------------------------------------------------------
// The problem.

/** One homogeneous layer, each quantity dimensionless. */
export interface OrdinateLayer {
  /** τ, its vertical optical depth: finite and at least 0. */
  readonly opticalDepth: number;
  /** ω, its single-scattering albedo, in [0, 1]. */
  readonly singleScatteringAlbedo: number;
  /**
   * χ_l = ∫ p P_l(cos Θ) dΩ, l = 0, 1, …, p the phase function per steradian (`aerosol.ts`'s
   * `deltaMTruncation` convention; χ_l = g^l for Henyey–Greenstein, χ₂ = 0.1 for Rayleigh), finite,
   * with χ₀ = 1. χ₀ … χ_{NSTR−1} enter the solve: moments past them are ignored (truncating them,
   * by delta-M or otherwise, is the caller's), and missing ones are 0.
   */
  readonly moments: ArrayLike<number>;
}

/**
 * The seam for a pseudo-spherical beam (R08.T14.b, after Dahlback and Stamnes 1991): how the
 * beam reaches and crosses each layer, for one μ₀.
 *
 * @remarks
 * Inside layer p the beam's transmission is T_p e^{−λ_p t}, t the vertical optical depth below the
 * layer's top. Plane-parallel, T_p = e^{−τ_p ÷ μ₀} and λ_p = 1 ÷ μ₀ ({@link planeParallelBeam});
 * a pseudo-spherical caller supplies T_p from the slant path to the layer's top and λ_p as the
 * layer's slant optical depth over its vertical one, so that T_{p+1} = T_p e^{−λ_p τ_layer}: the
 * solver does not check it, and reports the direct flux at interface p as μ₀ T_p. The scattering
 * geometry is the beam's one μ₀ in every layer, the pseudo-spherical approximation's. The beam's
 * transmission at the ground is T_L e^{−λ_L τ_L} of the last layer.
 */
export interface BeamPath {
  /** T_p, per layer from the top: the beam's transmission at the layer's top, in [0, 1]. */
  readonly topTransmission: ArrayLike<number>;
  /** λ_p, per layer: the beam's slant optical depth per unit vertical optical depth, > 0. */
  readonly slantFactor: ArrayLike<number>;
}

/** A solve: the layers, the ground, the beams, the user cosines and the modes. */
export interface DiscreteOrdinatesProblem {
  /** The layers from the top down; at least one. */
  readonly layers: ReadonlyArray<OrdinateLayer>;
  /** A, the ground's Lambertian albedo, in [0, 1]. */
  readonly groundAlbedo: number;
  /** μ₀ of each beam, in (0, 1]; all are solved on each mode's one factorisation. */
  readonly beamCosines: ArrayLike<number>;
  /** The user cosines, in (0, 1]: each is solved upward (+μ) and downward (−μ). */
  readonly userCosines: ArrayLike<number>;
  /** M: the azimuthal modes 0 … M are solved, M ≤ NSTR − 1. */
  readonly maxMode: number;
  /** NSTR, even and at least 2; {@link DISCRETE_ORDINATE_STREAMS} when absent. */
  readonly streams?: number;
  /** One per beam, in `beamCosines`' order: their paths, plane-parallel when absent. */
  readonly beamPaths?: ReadonlyArray<BeamPath>;
}

/** One mode's intensities I^m for one beam, at every interface. */
export interface ModeIntensities {
  /** m. */
  readonly mode: number;
  /** I^m(τ_k, +μ_i), (L + 1) × N, entry (k, i) at `k * N + i`: interface k, node i, upward. */
  readonly quadratureUp: Float64Array;
  /** I^m(τ_k, −μ_i): downward. */
  readonly quadratureDown: Float64Array;
  /** I^m(τ_k, +μ_u), (L + 1) × U, entry (k, u) at `k * U + u`: user cosine u, upward. */
  readonly userUp: Float64Array;
  /** I^m(τ_k, −μ_u): downward. */
  readonly userDown: Float64Array;
}

/** The m = 0 fluxes and mean intensity for one beam, at every interface k = 0 … L. */
export interface InterfaceFluxes {
  /** The direct beam's flux on a horizontal plane, μ₀ T(τ_k). */
  readonly direct: Float64Array;
  /** The diffuse downward flux, 2π Σ_i w_i μ_i I^0(τ_k, −μ_i). */
  readonly diffuseDown: Float64Array;
  /** The diffuse upward flux, 2π Σ_i w_i μ_i I^0(τ_k, +μ_i). */
  readonly diffuseUp: Float64Array;
  /**
   * The mean intensity, (1 ÷ 4π) ∫ I dΩ, the direct beam's T(τ_k) ÷ 4π included:
   * ½ Σ_i w_i (I^0(τ_k, μ_i) + I^0(τ_k, −μ_i)) + T(τ_k) ÷ 4π.
   */
  readonly meanIntensity: Float64Array;
}

/**
 * One beam's solution: its intensity's azimuthal modes and its fluxes, the beam's flux on a plane
 * normal to it being 1 (so μ₀ on the horizontal).
 *
 * @remarks
 * I(τ, μ, Δφ) = Σ_m I^m(τ, μ) cos(mΔφ) ({@link intensityAtAzimuth}), where Δφ is the azimuth of
 * the stream's direction of travel less the azimuth of the beam's: at Δφ = 0 a downward stream
 * (μ = −μ₀) is the forward-scattered one, Θ = 0, and at Δφ = π an upward one (μ = μ₀) returns
 * along the beam, Θ = π. For an observer this is the azimuth they look toward less the sun's, so
 * looking toward the sun's azimuth is Δφ = 0. cos is even, so the sense in which Δφ is counted does
 * not matter.
 */
export interface BeamSolution {
  /** μ₀. */
  readonly beamCosine: number;
  /** I^m for m = 0 … M, in order. */
  readonly modes: ReadonlyArray<ModeIntensities>;
  readonly fluxes: InterfaceFluxes;
}

/** A solve's results: the quadrature, the interfaces, and each beam's solution. */
export interface DiscreteOrdinatesSolution {
  readonly quadrature: DoubleGaussQuadrature;
  /** τ_k, k = 0 … L: 0 at the top, then each layer's base. */
  readonly interfaceDepths: Float64Array;
  /** The user cosines μ_u, as given. */
  readonly userCosines: Float64Array;
  /** One per beam cosine, in order. */
  readonly beams: ReadonlyArray<BeamSolution>;
}

// ---------------------------------------------------------------------------------------------
// The layer's homogeneous solution.

/** One layer's homogeneous solutions at one mode (the module's remarks, "Per layer and mode"). */
export interface HomogeneousSolution {
  /** m. */
  readonly mode: number;
  /** k_n², n < N, ascending: the reduced eigenproblem's eigenvalues. */
  readonly decayRatesSquared: Float64Array;
  /** k_n = √(k_n²); 0 for the conservative pair. */
  readonly decayRates: Float64Array;
  /**
   * Whether pair 0 is the conservative pair (m = 0, ω solved as 1): I = 1 and
   * I(t, ±μ_i) = t ± u_i in place of e^{∓k₀t}.
   */
  readonly conservative: boolean;
  /** G⁺_{n,i}, N × N, entry (n, i) at `n * N + i`: e^{−k_n t}'s upward intensities. */
  readonly up: Float64Array;
  /** G⁻_{n,i}: its downward intensities. The growing e^{−k_n(τ_layer − t)} swaps the two. */
  readonly down: Float64Array;
  /** u_i of the conservative linear solution; zeros when not conservative. */
  readonly linear: Float64Array;
}

/** A layer as solved: ω after the conservative rule, and χ_l to NSTR − 1. */
interface PreparedLayer {
  readonly opticalDepth: number;
  readonly omega: number;
  readonly conservative: boolean;
  readonly moments: Float64Array;
}

/** The homogeneous solution with what the particular solution needs of the eigenproblem. */
interface LayerMode extends HomogeneousSolution {
  readonly layer: PreparedLayer;
  /** L, the lower Cholesky factor of −P. */
  readonly cholesky: Float64Array;
  /** Y, the orthonormal eigenvectors of Lᵀ(−Q)L, by column. */
  readonly eigenvectors: Float64Array;
  /** d_i = 1 ÷ √(w_i μ_i). */
  readonly scale: Float64Array;
}

/**
 * One layer's homogeneous solutions at mode m: the reduced eigenproblem's decay rates and
 * eigenvectors, and the conservative pair at m = 0 and ω = 1 (the module's remarks).
 *
 * @throws RangeError for a bad layer, mode or quadrature, or for moments whose discrete
 *   scattering operator is not a contraction (an eigenvalue k² ≤ 0 off the conservative pair, or
 *   a −P that is not positive definite): the raw moments of a strongly forward-peaked phase
 *   function can be (the module's remarks), and their delta-M truncation is the cure.
 */
export function homogeneousSolution(
  layer: OrdinateLayer,
  mode: number,
  quadrature: DoubleGaussQuadrature,
): HomogeneousSolution {
  requireStreams(quadrature.streams);
  requireMode(mode, quadrature.streams);
  const prepared = prepareLayer(layer, 0, quadrature.streams);
  const nodeLegendre = legendreTable(mode, quadrature.streams - 1, quadrature.nodes);
  return layerMode(prepared, mode, quadrature, nodeLegendre);
}

function layerMode(
  layer: PreparedLayer,
  m: number,
  quadrature: DoubleGaussQuadrature,
  nodeLegendre: Float64Array,
): LayerMode {
  const { nodes, weights, streams } = quadrature;
  const n = nodes.length;
  const lmax = streams - 1;
  const { omega, moments } = layer;
  const sPlus = new Float64Array(n * n);
  const sMinus = new Float64Array(n * n);
  for (let l = m; l <= lmax; l += 1) {
    const coefficient = omega * (2 * l + 1) * at(moments, l);
    if (coefficient === 0) {
      continue;
    }
    const target = (l + m) % 2 === 0 ? sPlus : sMinus;
    for (let i = 0; i < n; i += 1) {
      const li = coefficient * at(nodeLegendre, l * n + i);
      for (let j = 0; j < n; j += 1) {
        const k = i * n + j;
        target[k] = at(target, k) + li * at(nodeLegendre, l * n + j);
      }
    }
  }
  // −P and −Q: M^{−½}(1 − W^{½} S∓ W^{½})M^{−½}.
  const negP = new Float64Array(n * n);
  const negQ = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j < n; j += 1) {
      const root = Math.sqrt(at(weights, i) * at(weights, j));
      const across = 1 / Math.sqrt(at(nodes, i) * at(nodes, j));
      const identity = i === j ? 1 : 0;
      negP[i * n + j] = (identity - root * at(sMinus, i * n + j)) * across;
      negQ[i * n + j] = (identity - root * at(sPlus, i * n + j)) * across;
    }
  }
  let cholesky: Float64Array;
  try {
    cholesky = choleskyLower(negP, n);
  } catch (error) {
    throw new RangeError(
      `layer moments give mode ${m} a scattering operator that is not a contraction ` +
        "(truncate a strongly forward-peaked phase function's moments, by delta-M, first)",
      { cause: error },
    );
  }
  // H = Lᵀ (−Q) L.
  const negQL = multiply(negQ, cholesky, n);
  const h = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j <= i; j += 1) {
      let sum = 0;
      for (let r = 0; r < n; r += 1) {
        sum += at(cholesky, r * n + i) * at(negQL, r * n + j);
      }
      h[i * n + j] = sum;
      h[j * n + i] = sum;
    }
  }
  const eigen = symmetricEigen(h, n);
  const conservative = m === 0 && layer.conservative;
  const decayRatesSquared = eigen.values;
  const largest = at(decayRatesSquared, n - 1);
  for (let k = 0; k < n; k += 1) {
    const k2 = at(decayRatesSquared, k);
    const isNull = conservative && k === 0;
    if (
      isNull ? !(Math.abs(k2) <= CONSERVATIVE_NULL_TOLERANCE * Math.max(largest, 1)) : !(k2 > 0)
    ) {
      throw new RangeError(
        `layer moments give mode ${m} an eigenvalue k² = ${k2}, ` +
          `${isNull ? "not the conservative 0" : "not positive"} (truncate a strongly ` +
          "forward-peaked phase function's moments, by delta-M, first)",
      );
    }
  }
  const scale = Float64Array.from(nodes, (mu, i) => 1 / Math.sqrt(at(weights, i) * mu));
  const decayRates = new Float64Array(n);
  const up = new Float64Array(n * n);
  const down = new Float64Array(n * n);
  const ly = new Float64Array(n);
  const lty = new Float64Array(n);
  for (let k = 0; k < n; k += 1) {
    const rate = conservative && k === 0 ? 0 : Math.sqrt(at(decayRatesSquared, k));
    decayRates[k] = rate;
    for (let i = 0; i < n; i += 1) {
      let sum = 0;
      for (let r = 0; r <= i; r += 1) {
        sum += at(cholesky, i * n + r) * at(eigen.vectors, r * n + k);
      }
      ly[i] = sum;
      lty[i] = at(eigen.vectors, i * n + k);
    }
    solveLowerTransposed(cholesky, n, lty);
    for (let i = 0; i < n; i += 1) {
      const half = at(scale, i) / 2;
      up[k * n + i] = half * (at(ly, i) - rate * at(lty, i));
      down[k * n + i] = half * (at(ly, i) + rate * at(lty, i));
    }
  }
  // u = −(α − β)⁻¹1 = D L⁻ᵀ L⁻¹ D⁻¹ 1.
  const linear = new Float64Array(n);
  if (conservative) {
    for (let i = 0; i < n; i += 1) {
      linear[i] = 1 / at(scale, i);
    }
    solveLower(cholesky, n, linear);
    solveLowerTransposed(cholesky, n, linear);
    for (let i = 0; i < n; i += 1) {
      linear[i] = at(linear, i) * at(scale, i);
    }
  }
  return {
    mode: m,
    decayRatesSquared,
    decayRates,
    conservative,
    up,
    down,
    linear,
    layer,
    cholesky,
    eigenvectors: eigen.vectors,
    scale,
  };
}

/** The beam's particular solution per unit transmission at the layer's top: Z⁺ and Z⁻. */
interface Particular {
  readonly up: Float64Array;
  readonly down: Float64Array;
}

/**
 * Z^± for the source (ω ÷ 4π)(2 − δ_m0) Σ (2l + 1) χ_l Λ_l(μ) Λ_l(−μ₀) e^{−λt} (the module's
 * remarks): Zs = D L Y (K² − λ²)⁻¹ Yᵀ (Lᵀq − λL⁻¹p) and Zd = −D L⁻ᵀ(λ Y c − L⁻¹p), with
 * q = W^{½}M^{−½}Xs, p = W^{½}M^{−½}Xd and c the bracketed coefficients.
 */
function particular(
  lm: LayerMode,
  quadrature: DoubleGaussQuadrature,
  nodeLegendre: Float64Array,
  beamLegendre: Float64Array,
  slant: number,
): Particular {
  const { nodes, weights, streams } = quadrature;
  const n = nodes.length;
  const lmax = streams - 1;
  const m = lm.mode;
  const { omega, moments } = lm.layer;
  const strength = (omega * (m === 0 ? 1 : 2)) / (4 * Math.PI);
  // q and p from X0± at the nodes: X0⁺_i = c Σ (−1)^{l+m} (…) and X0⁻_i = c Σ (…).
  const q = new Float64Array(n);
  const p = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    let even = 0;
    let odd = 0;
    for (let l = m; l <= lmax; l += 1) {
      const term = (2 * l + 1) * at(moments, l) * at(nodeLegendre, l * n + i) * at(beamLegendre, l);
      if ((l + m) % 2 === 0) {
        even += term;
      } else {
        odd += term;
      }
    }
    const factor = Math.sqrt(at(weights, i) / at(nodes, i));
    q[i] = factor * 2 * strength * even;
    p[i] = -factor * 2 * strength * odd;
  }
  const { cholesky, eigenvectors: y, decayRatesSquared, scale } = lm;
  const lInvP = Float64Array.from(p);
  solveLower(cholesky, n, lInvP);
  const t = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    let sum = 0;
    for (let r = i; r < n; r += 1) {
      sum += at(cholesky, r * n + i) * at(q, r);
    }
    t[i] = sum - slant * at(lInvP, i);
  }
  const lambda2 = slant * slant;
  const coefficients = new Float64Array(n);
  for (let k = 0; k < n; k += 1) {
    let sum = 0;
    for (let i = 0; i < n; i += 1) {
      sum += at(y, i * n + k) * at(t, i);
    }
    coefficients[k] = sum / (at(decayRatesSquared, k) - lambda2);
  }
  const yc = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    let sum = 0;
    for (let k = 0; k < n; k += 1) {
      sum += at(y, i * n + k) * at(coefficients, k);
    }
    yc[i] = sum;
  }
  const difference = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    difference[i] = slant * at(yc, i) - at(lInvP, i);
  }
  solveLowerTransposed(cholesky, n, difference);
  const up = new Float64Array(n);
  const down = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    let lyc = 0;
    for (let r = 0; r <= i; r += 1) {
      lyc += at(cholesky, i * n + r) * at(yc, r);
    }
    const sum = at(scale, i) * lyc;
    const diff = -at(scale, i) * at(difference, i);
    up[i] = (sum + diff) / 2;
    down[i] = (sum - diff) / 2;
  }
  return { up, down };
}

/** Whether layer `lm`'s slant factor lies within the resonance window of one of its k_n. */
function isResonant(lm: LayerMode, slant: number): boolean {
  const { decayRates } = lm;
  for (let k = lm.conservative ? 1 : 0; k < decayRates.length; k += 1) {
    if (Math.abs(at(decayRates, k) - slant) <= RESONANCE_HALF_WIDTH * slant) {
      return true;
    }
  }
  return false;
}

// ---------------------------------------------------------------------------------------------
// The layer stack and its boundary-value problem.

/**
 * One layer's 2n homogeneous solutions at its two faces, for {@link factorLayerStack}: n the
 * streams a hemisphere (times the Stokes components, for a vector mode).
 */
export interface LayerFaces {
  /**
   * Their intensities at the layer's top, 2n × 2n row-major: row r < n the upward stream r, row
   * n + r the downward stream r, column s the solution s.
   */
  readonly top: Float64Array;
  /** The same at the layer's bottom. */
  readonly bottom: Float64Array;
}

/** The boundary-value problem of a stack of layers, factorised: {@link factorLayerStack}. */
export interface LayerStack {
  /** n, the streams a hemisphere. */
  readonly hemisphereStreams: number;
  /** L, the layers. */
  readonly layerCount: number;
  /** R, n × n, the ground's reflection at the nodes, or absent for a black ground. */
  readonly reflection: Float64Array | undefined;
  readonly lu: BandLu;
}

/**
 * Assembles and factorises the boundary-value problem of a stack of layers (Stamnes and Conklin
 * 1984; Stamnes et al. 1988): no diffuse light down at the top, every stream continuous at each
 * interface, and I⁺ = R I⁻ + S at the ground, for the coefficients of each layer's 2n solutions,
 * layer by layer.
 *
 * @remarks
 * Rows: the top's n, then 2n at each interface (upward streams, then downward), then the ground's
 * n. The matrix is banded, 3n − 1 either side of its diagonal, and is factorised by banded
 * elimination with partial pivoting. Its right-hand side, {@link layerStackRightHandSide}, carries
 * the particular solutions and the ground's source, so that one factorisation serves every beam.
 *
 * @param faces - Per layer from the top down, its solutions at its faces.
 * @param hemisphereStreams - n.
 * @param reflection - R, n × n row-major: I⁺_i(τ_L) = Σ_j R_ij I⁻_j(τ_L) + S_i; absent for a black
 *   ground. A Lambertian ground of albedo A is R_ij = 2A w_j μ_j at m = 0.
 * @throws RangeError for no layers, faces of the wrong size, or a reflection of the wrong size.
 * @throws Error if the problem is singular.
 */
export function factorLayerStack(
  faces: ReadonlyArray<LayerFaces>,
  hemisphereStreams: number,
  reflection: Float64Array | undefined,
): LayerStack {
  const n = hemisphereStreams;
  const layerCount = faces.length;
  if (!Number.isInteger(n) || n < 1 || layerCount < 1) {
    throw new RangeError(`a stack needs n ≥ 1 and a layer, not n ${n} and ${layerCount} layers`);
  }
  const span = 2 * n;
  for (const face of faces) {
    if (face.top.length !== span * span || face.bottom.length !== span * span) {
      throw new RangeError(`each layer's faces are ${span} × ${span}`);
    }
  }
  if (reflection !== undefined && reflection.length !== n * n) {
    throw new RangeError(
      `the ground's reflection is ${n} × ${n}, not ${reflection.length} entries`,
    );
  }
  const band = 3 * n - 1;
  const matrix = bandMatrix(span * layerCount, band, band);
  const a = matrix.entries;
  const set = (row: number, column: number, value: number): void => {
    a[bandIndex(matrix, row, column)] = value;
  };
  const first = faceAt(faces, 0);
  for (let i = 0; i < n; i += 1) {
    for (let s = 0; s < span; s += 1) {
      set(i, s, at(first.top, (n + i) * span + s));
    }
  }
  for (let p = 0; p + 1 < layerCount; p += 1) {
    const upper = faceAt(faces, p);
    const lower = faceAt(faces, p + 1);
    const base = n + span * p;
    for (let r = 0; r < span; r += 1) {
      for (let s = 0; s < span; s += 1) {
        set(base + r, span * p + s, at(upper.bottom, r * span + s));
        set(base + r, span * (p + 1) + s, -at(lower.top, r * span + s));
      }
    }
  }
  const last = faceAt(faces, layerCount - 1);
  const base = n + span * (layerCount - 1);
  for (let i = 0; i < n; i += 1) {
    for (let s = 0; s < span; s += 1) {
      let value = at(last.bottom, i * span + s);
      if (reflection !== undefined) {
        for (let j = 0; j < n; j += 1) {
          value -= at(reflection, i * n + j) * at(last.bottom, (n + j) * span + s);
        }
      }
      set(base + i, span * (layerCount - 1) + s, value);
    }
  }
  return { hemisphereStreams: n, layerCount, reflection, lu: factorBand(matrix) };
}

/**
 * The right-hand side of a {@link LayerStack}'s problem for one source: each layer's particular
 * solution at its faces and the ground's source S.
 *
 * @param particularTop - Per layer, the particular solution at its top: 2n, the upward streams then
 *   the downward.
 * @param particularBottom - The same at its bottom.
 * @param groundSource - S, n: the ground's own upward intensities (its reflection of the direct
 *   beam), or absent for none.
 * @throws RangeError for arrays of the wrong count or size.
 */
export function layerStackRightHandSide(
  stack: LayerStack,
  particularTop: ReadonlyArray<Float64Array>,
  particularBottom: ReadonlyArray<Float64Array>,
  groundSource: Float64Array | undefined,
): Float64Array {
  const { hemisphereStreams: n, layerCount, reflection } = stack;
  const span = 2 * n;
  if (particularTop.length !== layerCount || particularBottom.length !== layerCount) {
    throw new RangeError(`a particular solution a layer, ${layerCount} of them`);
  }
  if (groundSource !== undefined && groundSource.length !== n) {
    throw new RangeError(`the ground's source has ${n} entries, not ${groundSource.length}`);
  }
  const vectorAt = (list: ReadonlyArray<Float64Array>, p: number): Float64Array => {
    const v = list[p];
    if (v?.length !== span) {
      throw new RangeError(`layer ${p}'s particular solution needs ${span} entries`);
    }
    return v;
  };
  const rhs = new Float64Array(span * layerCount);
  const top = vectorAt(particularTop, 0);
  for (let i = 0; i < n; i += 1) {
    rhs[i] = -at(top, n + i);
  }
  for (let p = 0; p + 1 < layerCount; p += 1) {
    const bottom = vectorAt(particularBottom, p);
    const next = vectorAt(particularTop, p + 1);
    for (let r = 0; r < span; r += 1) {
      rhs[n + span * p + r] = at(next, r) - at(bottom, r);
    }
  }
  const bottom = vectorAt(particularBottom, layerCount - 1);
  const base = n + span * (layerCount - 1);
  for (let i = 0; i < n; i += 1) {
    let value = (groundSource === undefined ? 0 : at(groundSource, i)) - at(bottom, i);
    if (reflection !== undefined) {
      for (let j = 0; j < n; j += 1) {
        value += at(reflection, i * n + j) * at(bottom, n + j);
      }
    }
    rhs[base + i] = value;
  }
  return rhs;
}

/**
 * Solves a {@link LayerStack}'s problem for one right-hand side, in place: on return it holds the
 * solutions' coefficients, layer p's solution s at `2n p + s`.
 */
export function solveLayerStack(stack: LayerStack, rightHandSide: Float64Array): Float64Array {
  solveBand(stack.lu, rightHandSide);
  return rightHandSide;
}

function faceAt(faces: ReadonlyArray<LayerFaces>, p: number): LayerFaces {
  const face = faces[p];
  if (face === undefined) {
    throw new RangeError(`no faces for layer ${p}`);
  }
  return face;
}

/** A scalar layer's solutions at its faces (the module's remarks, "The boundary-value problem"). */
function scalarFaces(lm: LayerMode): LayerFaces {
  const n = lm.decayRates.length;
  const span = 2 * n;
  const depth = lm.layer.opticalDepth;
  const top = new Float64Array(span * span);
  const bottom = new Float64Array(span * span);
  for (let k = 0; k < n; k += 1) {
    const decaying = k;
    const growing = n + k;
    if (lm.conservative && k === 0) {
      for (let i = 0; i < n; i += 1) {
        const u = at(lm.linear, i);
        for (const face of [top, bottom]) {
          face[i * span + decaying] = 1;
          face[(n + i) * span + decaying] = 1;
        }
        top[i * span + growing] = u;
        top[(n + i) * span + growing] = -u;
        bottom[i * span + growing] = depth + u;
        bottom[(n + i) * span + growing] = depth - u;
      }
      continue;
    }
    const attenuation = Math.exp(-at(lm.decayRates, k) * depth);
    for (let i = 0; i < n; i += 1) {
      const gUp = at(lm.up, k * n + i);
      const gDown = at(lm.down, k * n + i);
      top[i * span + decaying] = gUp;
      top[(n + i) * span + decaying] = gDown;
      bottom[i * span + decaying] = gUp * attenuation;
      bottom[(n + i) * span + decaying] = gDown * attenuation;
      top[i * span + growing] = gDown * attenuation;
      top[(n + i) * span + growing] = gUp * attenuation;
      bottom[i * span + growing] = gDown;
      bottom[(n + i) * span + growing] = gUp;
    }
  }
  return { top, bottom };
}

// ---------------------------------------------------------------------------------------------
// User angles.

/** φ(z, Δ) = ∫₀^Δ e^{−zt} dt = (1 − e^{−zΔ}) ÷ z for z ≥ 0, Δ at z = 0. */
function phi(z: number, depth: number): number {
  const x = z * depth;
  return x === 0 ? depth : -Math.expm1(-x) / z;
}

/**
 * ψ(a, b, Δ) = ∫₀^Δ e^{−at} e^{−b(Δ−t)} dt = (e^{−aΔ} − e^{−bΔ}) ÷ (b − a), Δe^{−aΔ} at a = b,
 * for a, b ≥ 0: the divided difference that meets no singularity where the rates meet.
 */
function psi(a: number, b: number, depth: number): number {
  return Math.exp(-Math.min(a, b) * depth) * phi(Math.abs(a - b), depth);
}

/**
 * The source function (ω ÷ 2) Σ_l (2l + 1) χ_l Λ_l(±μ_u) Σ_j w_j Λ_l(μ_j)(I⁺_j + (−1)^{l+m} I⁻_j)
 * of quadrature intensities (I⁺, I⁻) at each user cosine, upward (+μ_u) and downward (−μ_u).
 */
function userSource(
  lm: LayerMode,
  quadrature: DoubleGaussQuadrature,
  nodeLegendre: Float64Array,
  userLegendre: Float64Array,
  intensityUp: Float64Array,
  intensityDown: Float64Array,
  outUp: Float64Array,
  outDown: Float64Array,
): void {
  const { weights, streams } = quadrature;
  const n = weights.length;
  const users = outUp.length;
  const lmax = streams - 1;
  const m = lm.mode;
  const { omega, moments } = lm.layer;
  outUp.fill(0);
  outDown.fill(0);
  for (let l = m; l <= lmax; l += 1) {
    const sign = (l + m) % 2 === 0 ? 1 : -1;
    const coefficient = (omega / 2) * (2 * l + 1) * at(moments, l);
    if (coefficient === 0) {
      continue;
    }
    let half = 0;
    for (let j = 0; j < n; j += 1) {
      half +=
        at(weights, j) *
        at(nodeLegendre, l * n + j) *
        (at(intensityUp, j) + sign * at(intensityDown, j));
    }
    const c = coefficient * half;
    for (let u = 0; u < users; u += 1) {
      const term = c * at(userLegendre, l * users + u);
      outUp[u] = at(outUp, u) + term;
      outDown[u] = at(outDown, u) + sign * term;
    }
  }
}

/**
 * A layer's homogeneous solutions integrated along each user stream: solution s's contribution,
 * per unit coefficient, to the upward intensity at the layer's top from its bottom and to the
 * downward at its bottom from its top, 2N × U each, and the layer's attenuation e^{−Δ ÷ μ_u}.
 */
interface UserTransfer {
  readonly up: Float64Array;
  readonly down: Float64Array;
  readonly attenuation: Float64Array;
}

function userTransfer(
  lm: LayerMode,
  quadrature: DoubleGaussQuadrature,
  nodeLegendre: Float64Array,
  userLegendre: Float64Array,
  userCosines: Float64Array,
): UserTransfer {
  const n = lm.decayRates.length;
  const users = userCosines.length;
  const depth = lm.layer.opticalDepth;
  const up = new Float64Array(2 * n * users);
  const down = new Float64Array(2 * n * users);
  const attenuation = Float64Array.from(userCosines, (mu) => Math.exp(-depth / mu));
  const sourceUp = new Float64Array(users);
  const sourceDown = new Float64Array(users);
  const vectorUp = new Float64Array(n);
  const vectorDown = new Float64Array(n);
  for (let k = 0; k < n; k += 1) {
    const decaying = k;
    const growing = n + k;
    if (lm.conservative && k === 0) {
      // The constant solution's source, ≈ 1, and the linear one's: t × it plus that of (u, −u).
      vectorUp.fill(1);
      vectorDown.fill(1);
      userSource(
        lm,
        quadrature,
        nodeLegendre,
        userLegendre,
        vectorUp,
        vectorDown,
        sourceUp,
        sourceDown,
      );
      const constantUp = Float64Array.from(sourceUp);
      const constantDown = Float64Array.from(sourceDown);
      for (let i = 0; i < n; i += 1) {
        vectorUp[i] = at(lm.linear, i);
        vectorDown[i] = -at(lm.linear, i);
      }
      userSource(
        lm,
        quadrature,
        nodeLegendre,
        userLegendre,
        vectorUp,
        vectorDown,
        sourceUp,
        sourceDown,
      );
      for (let u = 0; u < users; u += 1) {
        const mu = at(userCosines, u);
        const passed = -Math.expm1(-depth / mu);
        const e = at(attenuation, u);
        up[decaying * users + u] = at(constantUp, u) * passed;
        down[decaying * users + u] = at(constantDown, u) * passed;
        up[growing * users + u] =
          at(constantUp, u) * (mu * passed - depth * e) + at(sourceUp, u) * passed;
        down[growing * users + u] =
          at(constantDown, u) * (depth - mu * passed) + at(sourceDown, u) * passed;
      }
      continue;
    }
    for (let i = 0; i < n; i += 1) {
      vectorUp[i] = at(lm.up, k * n + i);
      vectorDown[i] = at(lm.down, k * n + i);
    }
    userSource(
      lm,
      quadrature,
      nodeLegendre,
      userLegendre,
      vectorUp,
      vectorDown,
      sourceUp,
      sourceDown,
    );
    const rate = at(lm.decayRates, k);
    for (let u = 0; u < users; u += 1) {
      const mu = at(userCosines, u);
      const inverse = 1 / mu;
      const along = inverse * phi(rate + inverse, depth);
      const against = inverse * psi(rate, inverse, depth);
      // The decaying solution's source is J(±μ); the growing one's, with G⁺ and G⁻ swapped, J(∓μ).
      up[decaying * users + u] = at(sourceUp, u) * along;
      down[decaying * users + u] = at(sourceDown, u) * against;
      up[growing * users + u] = at(sourceDown, u) * against;
      down[growing * users + u] = at(sourceUp, u) * along;
    }
  }
  return { up, down, attenuation };
}

// ---------------------------------------------------------------------------------------------
// The solve.

/**
 * The beam paths of a plane-parallel beam at μ₀ through `layers`: T_p = e^{−τ_p ÷ μ₀} at each
 * layer's top, τ_p the optical depth above it, and λ_p = 1 ÷ μ₀.
 *
 * @throws RangeError for a cosine outside (0, 1] or a layer whose optical depth is negative or not
 *   finite.
 */
export function planeParallelBeam(
  layers: ReadonlyArray<OrdinateLayer>,
  beamCosine: number,
): BeamPath {
  requireIn("beamCosine", beamCosine, (v) => v > 0 && v <= 1, "in (0, 1]");
  const topTransmission = new Float64Array(layers.length);
  let above = 0;
  layers.forEach((layer, p) => {
    requireIn("opticalDepth", layer.opticalDepth, (v) => v >= 0, "finite and at least 0");
    topTransmission[p] = Math.exp(-above / beamCosine);
    above += layer.opticalDepth;
  });
  return { topTransmission, slantFactor: new Float64Array(layers.length).fill(1 / beamCosine) };
}

/** What one mode's solve shares across its beams. */
interface ModeContext {
  readonly m: number;
  readonly quadrature: DoubleGaussQuadrature;
  readonly layerModes: ReadonlyArray<LayerMode>;
  readonly faces: ReadonlyArray<LayerFaces>;
  readonly transfers: ReadonlyArray<UserTransfer>;
  readonly stack: LayerStack;
  readonly nodeLegendre: Float64Array;
  readonly userLegendre: Float64Array;
  readonly userCosines: Float64Array;
  readonly groundAlbedo: number;
}

/** A beam as validated: μ₀, its path, and its transmission at the ground. */
interface PreparedBeam {
  readonly cosine: number;
  readonly topTransmission: Float64Array;
  readonly slantFactor: Float64Array;
  readonly groundTransmission: number;
}

/**
 * Solves the discrete-ordinates problem: per mode, every layer's homogeneous solutions, one
 * factorisation of the boundary-value problem, and then each beam's particular solutions, the
 * intensities at the quadrature and user cosines at every interface, and the m = 0 fluxes (the
 * module's remarks).
 *
 * @example
 * ```ts
 * const solution = solveDiscreteOrdinates({
 *   layers: [{ opticalDepth: 1, singleScatteringAlbedo: 0.9, moments: [1, 0, 0.1] }],
 *   groundAlbedo: 0,
 *   beamCosines: [0.5],
 *   userCosines: [0.5, 1],
 *   maxMode: 2,
 * });
 * const reflected = solution.beams[0]?.fluxes.diffuseUp[0]; // of the incident 0.5
 * ```
 *
 * @throws RangeError for a stream count that is not an even integer of at least 2; no layers; a
 *   τ that is negative or not finite; an ω outside [0, 1]; a moment that is not finite, or χ₀ more
 *   than 10⁻¹² from 1; a beam or user cosine outside (0, 1]; a ground albedo outside [0, 1]; a
 *   mode that is not a whole number in [0, NSTR − 1]; beam paths that do not match the beams and
 *   layers, or hold a transmission outside [0, 1] or a slant factor not positive and finite; or
 *   moments whose discrete scattering operator is not a contraction ({@link homogeneousSolution}).
 */
export function solveDiscreteOrdinates(
  problem: DiscreteOrdinatesProblem,
): DiscreteOrdinatesSolution {
  const streams = problem.streams ?? DISCRETE_ORDINATE_STREAMS;
  requireStreams(streams);
  requireMode(problem.maxMode, streams);
  if (problem.layers.length < 1) {
    throw new RangeError("a solve needs at least one layer");
  }
  requireIn("groundAlbedo", problem.groundAlbedo, (v) => v >= 0 && v <= 1, "in [0, 1]");
  const layers = problem.layers.map((layer, p) => prepareLayer(layer, p, streams));
  const userCosines = Float64Array.from(problem.userCosines);
  userCosines.forEach((mu) => {
    requireIn("userCosine", mu, (v) => v > 0 && v <= 1, "in (0, 1]");
  });
  const beams = prepareBeams(problem, layers);
  const quadrature = doubleGaussQuadrature(streams);
  const n = quadrature.nodes.length;
  const layerCount = layers.length;
  const interfaceDepths = new Float64Array(layerCount + 1);
  layers.forEach((layer, p) => {
    interfaceDepths[p + 1] = at(interfaceDepths, p) + layer.opticalDepth;
  });

  const perBeam: ModeIntensities[][] = beams.map(() => []);
  for (let m = 0; m <= problem.maxMode; m += 1) {
    const context = modeContext(m, quadrature, layers, userCosines, problem.groundAlbedo);
    beams.forEach((beam, b) => {
      perBeam[b]?.push(solveBeamMode(context, beam));
    });
  }

  const solved = beams.map((beam, b): BeamSolution => {
    const modes = perBeam[b] ?? [];
    const zeroth = modes[0];
    if (zeroth === undefined) {
      throw new Error("mode 0 is always solved");
    }
    return {
      beamCosine: beam.cosine,
      modes,
      fluxes: fluxesOf(zeroth, beam, quadrature, layerCount, n),
    };
  });
  return { quadrature, interfaceDepths, userCosines, beams: solved };
}

/** A stream's direction of travel: up (+μ) or down (−μ). */
export type StreamDirection = "up" | "down";

/** A mode's intensities at the user cosines in one direction. */
function userField(field: ModeIntensities, direction: StreamDirection): Float64Array {
  let values: Float64Array;
  switch (direction) {
    case "up":
      values = field.userUp;
      break;
    case "down":
      values = field.userDown;
      break;
  }
  return values;
}

/**
 * The intensity I(τ_k, ±μ_u, Δφ) = Σ_m I^m cos(mΔφ) at a user cosine, from a beam's modes
 * ({@link BeamSolution} for Δφ).
 *
 * @param interfaceIndex - k, 0 at the top.
 * @param userIndex - u, the user cosine's index.
 * @param direction - The stream's direction of travel.
 * @param relativeAzimuthRad - Δφ, rad.
 * @throws RangeError for an interface or user index out of range.
 */
export function intensityAtAzimuth(
  beam: BeamSolution,
  interfaceIndex: number,
  userIndex: number,
  direction: StreamDirection,
  relativeAzimuthRad: number,
): number {
  if (!Number.isInteger(interfaceIndex) || !Number.isInteger(userIndex)) {
    throw new RangeError(`no intensity at interface ${interfaceIndex}, user cosine ${userIndex}`);
  }
  let sum = 0;
  for (const field of beam.modes) {
    const values = userField(field, direction);
    const users = values.length / Math.max(1, beam.fluxes.direct.length);
    const value = values[interfaceIndex * users + userIndex];
    if (value === undefined || userIndex < 0 || userIndex >= users) {
      throw new RangeError(`no intensity at interface ${interfaceIndex}, user cosine ${userIndex}`);
    }
    sum += value * Math.cos(field.mode * relativeAzimuthRad);
  }
  return sum;
}

function modeContext(
  m: number,
  quadrature: DoubleGaussQuadrature,
  layers: ReadonlyArray<PreparedLayer>,
  userCosines: Float64Array,
  groundAlbedo: number,
): ModeContext {
  const lmax = quadrature.streams - 1;
  const n = quadrature.nodes.length;
  const nodeLegendre = legendreTable(m, lmax, quadrature.nodes);
  const userLegendre = legendreTable(m, lmax, userCosines);
  const layerModes = layers.map((layer) => layerMode(layer, m, quadrature, nodeLegendre));
  const faces = layerModes.map(scalarFaces);
  const transfers = layerModes.map((lm) =>
    userTransfer(lm, quadrature, nodeLegendre, userLegendre, userCosines),
  );
  let reflection: Float64Array | undefined;
  if (m === 0 && groundAlbedo > 0) {
    const r = new Float64Array(n * n);
    for (let i = 0; i < n; i += 1) {
      for (let j = 0; j < n; j += 1) {
        r[i * n + j] = 2 * groundAlbedo * at(quadrature.weights, j) * at(quadrature.nodes, j);
      }
    }
    reflection = r;
  }
  const stack = factorLayerStack(faces, n, reflection);
  return {
    m,
    quadrature,
    layerModes,
    faces,
    transfers,
    stack,
    nodeLegendre,
    userLegendre,
    userCosines,
    groundAlbedo,
  };
}

/**
 * One beam's mode: solved once, or, where a layer's λ is resonant, at λ(1 ∓ 2h) in those layers
 * and averaged (the module's remarks, "A beam at an eigenvalue").
 */
function solveBeamMode(context: ModeContext, beam: PreparedBeam): ModeIntensities {
  const resonant = context.layerModes.map(
    (lm, p) =>
      at(beam.topTransmission, p) > 0 &&
      lm.layer.omega > 0 &&
      isResonant(lm, at(beam.slantFactor, p)),
  );
  if (!resonant.includes(true)) {
    return solveBeamModeAt(context, beam, beam.slantFactor);
  }
  const shifted = (sign: number): Float64Array =>
    beam.slantFactor.map((lambda, p) =>
      resonant[p] === true ? lambda * (1 + sign * 2 * RESONANCE_HALF_WIDTH) : lambda,
    );
  const below = solveBeamModeAt(context, beam, shifted(-1));
  const above = solveBeamModeAt(context, beam, shifted(1));
  const mean = (x: Float64Array, y: Float64Array): Float64Array =>
    x.map((value, i) => (value + at(y, i)) / 2);
  return {
    mode: below.mode,
    quadratureUp: mean(below.quadratureUp, above.quadratureUp),
    quadratureDown: mean(below.quadratureDown, above.quadratureDown),
    userUp: mean(below.userUp, above.userUp),
    userDown: mean(below.userDown, above.userDown),
  };
}

function solveBeamModeAt(
  context: ModeContext,
  beam: PreparedBeam,
  slantFactor: Float64Array,
): ModeIntensities {
  const { m, quadrature, layerModes, faces, transfers, stack, nodeLegendre, userLegendre } =
    context;
  const { userCosines, groundAlbedo } = context;
  const n = quadrature.nodes.length;
  const span = 2 * n;
  const users = userCosines.length;
  const layerCount = layerModes.length;
  const beamLegendre = new Float64Array(quadrature.streams);
  legendreInto(m, quadrature.streams - 1, beam.cosine, beamLegendre, 0, 1);

  const particularTop: Float64Array[] = [];
  const particularBottom: Float64Array[] = [];
  const particularUserUp: Float64Array[] = [];
  const particularUserDown: Float64Array[] = [];
  const sourceUp = new Float64Array(users);
  const sourceDown = new Float64Array(users);
  layerModes.forEach((lm, p) => {
    const top = new Float64Array(span);
    const bottom = new Float64Array(span);
    const userUp = new Float64Array(users);
    const userDown = new Float64Array(users);
    const transmission = at(beam.topTransmission, p);
    const lambda = at(slantFactor, p);
    const { omega, opticalDepth: depth, moments } = lm.layer;
    if (transmission > 0 && omega > 0) {
      const z = particular(lm, quadrature, nodeLegendre, beamLegendre, lambda);
      const through = Math.exp(-lambda * depth);
      for (let i = 0; i < n; i += 1) {
        top[i] = transmission * at(z.up, i);
        top[n + i] = transmission * at(z.down, i);
        bottom[i] = transmission * through * at(z.up, i);
        bottom[n + i] = transmission * through * at(z.down, i);
      }
      // The source at the user cosines: the scattered particular solution and the beam itself,
      // X0(+μ_u) = c Σ (2l + 1) χ_l (−1)^{l+m} Λ_l(μ_u) Λ_l(μ₀) and X0(−μ_u) without the sign.
      userSource(lm, quadrature, nodeLegendre, userLegendre, z.up, z.down, sourceUp, sourceDown);
      const strength = (omega * (m === 0 ? 1 : 2)) / (4 * Math.PI);
      for (let l = m; l < quadrature.streams; l += 1) {
        const sign = (l + m) % 2 === 0 ? 1 : -1;
        const c = strength * (2 * l + 1) * at(moments, l) * at(beamLegendre, l);
        if (c === 0) {
          continue;
        }
        for (let u = 0; u < users; u += 1) {
          const term = c * at(userLegendre, l * users + u);
          sourceUp[u] = at(sourceUp, u) + sign * term;
          sourceDown[u] = at(sourceDown, u) + term;
        }
      }
      for (let u = 0; u < users; u += 1) {
        const inverse = 1 / at(userCosines, u);
        userUp[u] = transmission * at(sourceUp, u) * inverse * phi(lambda + inverse, depth);
        userDown[u] = transmission * at(sourceDown, u) * inverse * psi(lambda, inverse, depth);
      }
    }
    particularTop.push(top);
    particularBottom.push(bottom);
    particularUserUp.push(userUp);
    particularUserDown.push(userDown);
  });

  const groundBeam =
    m === 0 && groundAlbedo > 0
      ? (groundAlbedo / Math.PI) * beam.cosine * beam.groundTransmission
      : 0;
  const groundSource = groundBeam > 0 ? new Float64Array(n).fill(groundBeam) : undefined;
  const coefficients = solveLayerStack(
    stack,
    layerStackRightHandSide(stack, particularTop, particularBottom, groundSource),
  );

  const quadratureUp = new Float64Array((layerCount + 1) * n);
  const quadratureDown = new Float64Array((layerCount + 1) * n);
  for (let k = 0; k <= layerCount; k += 1) {
    const p = Math.min(k, layerCount - 1);
    const face = faceAt(faces, p);
    const values = k < layerCount ? face.top : face.bottom;
    const part = (k < layerCount ? particularTop : particularBottom)[p] ?? new Float64Array(span);
    for (let r = 0; r < span; r += 1) {
      let sum = at(part, r);
      for (let s = 0; s < span; s += 1) {
        sum += at(values, r * span + s) * at(coefficients, span * p + s);
      }
      if (r < n) {
        quadratureUp[k * n + r] = sum;
      } else {
        quadratureDown[k * n + r - n] = sum;
      }
    }
  }

  const userUp = new Float64Array((layerCount + 1) * users);
  const userDown = new Float64Array((layerCount + 1) * users);
  for (let p = 0; p < layerCount; p += 1) {
    const transfer = transfers[p];
    const part = particularUserDown[p];
    if (transfer === undefined || part === undefined) {
      throw new Error(`layer ${p} has no user-angle transfer`);
    }
    for (let u = 0; u < users; u += 1) {
      let value = at(userDown, p * users + u) * at(transfer.attenuation, u) + at(part, u);
      for (let s = 0; s < span; s += 1) {
        value += at(coefficients, span * p + s) * at(transfer.down, s * users + u);
      }
      userDown[(p + 1) * users + u] = value;
    }
  }
  if (m === 0 && groundAlbedo > 0) {
    let flux = 0;
    for (let j = 0; j < n; j += 1) {
      flux +=
        at(quadrature.weights, j) *
        at(quadrature.nodes, j) *
        at(quadratureDown, layerCount * n + j);
    }
    userUp.fill(2 * groundAlbedo * flux + groundBeam, layerCount * users);
  }
  for (let p = layerCount - 1; p >= 0; p -= 1) {
    const transfer = transfers[p];
    const part = particularUserUp[p];
    if (transfer === undefined || part === undefined) {
      throw new Error(`layer ${p} has no user-angle transfer`);
    }
    for (let u = 0; u < users; u += 1) {
      let value = at(userUp, (p + 1) * users + u) * at(transfer.attenuation, u) + at(part, u);
      for (let s = 0; s < span; s += 1) {
        value += at(coefficients, span * p + s) * at(transfer.up, s * users + u);
      }
      userUp[p * users + u] = value;
    }
  }
  return { mode: m, quadratureUp, quadratureDown, userUp, userDown };
}

function fluxesOf(
  zeroth: ModeIntensities,
  beam: PreparedBeam,
  quadrature: DoubleGaussQuadrature,
  layerCount: number,
  n: number,
): InterfaceFluxes {
  const direct = new Float64Array(layerCount + 1);
  const diffuseDown = new Float64Array(layerCount + 1);
  const diffuseUp = new Float64Array(layerCount + 1);
  const meanIntensity = new Float64Array(layerCount + 1);
  for (let k = 0; k <= layerCount; k += 1) {
    const transmission = k < layerCount ? at(beam.topTransmission, k) : beam.groundTransmission;
    let down = 0;
    let up = 0;
    let mean = 0;
    for (let i = 0; i < n; i += 1) {
      const w = at(quadrature.weights, i);
      const mu = at(quadrature.nodes, i);
      const iUp = at(zeroth.quadratureUp, k * n + i);
      const iDown = at(zeroth.quadratureDown, k * n + i);
      down += w * mu * iDown;
      up += w * mu * iUp;
      mean += w * (iUp + iDown);
    }
    direct[k] = beam.cosine * transmission;
    diffuseDown[k] = 2 * Math.PI * down;
    diffuseUp[k] = 2 * Math.PI * up;
    meanIntensity[k] = mean / 2 + transmission / (4 * Math.PI);
  }
  return { direct, diffuseDown, diffuseUp, meanIntensity };
}

// ---------------------------------------------------------------------------------------------
// Validation.

function prepareLayer(layer: OrdinateLayer, p: number, streams: number): PreparedLayer {
  const name = `layer ${p}`;
  requireIn(`${name}'s opticalDepth`, layer.opticalDepth, (v) => v >= 0, "finite and at least 0");
  requireIn(
    `${name}'s singleScatteringAlbedo`,
    layer.singleScatteringAlbedo,
    (v) => v >= 0 && v <= 1,
    "in [0, 1]",
  );
  const moments = new Float64Array(streams);
  for (let l = 0; l < layer.moments.length; l += 1) {
    const chi = layer.moments[l] ?? Number.NaN;
    requireIn(`${name}'s moment ${l}`, chi, () => true, "finite");
    if (l < streams) {
      moments[l] = chi;
    }
  }
  const zeroth = layer.moments[0];
  if (zeroth === undefined || !(Math.abs(zeroth - 1) <= ZEROTH_MOMENT_TOLERANCE)) {
    throw new RangeError(`${name}'s moment 0 must be 1, not ${zeroth}`);
  }
  moments[0] = 1;
  const conservative = 1 - layer.singleScatteringAlbedo <= DISCRETE_ORDINATE_CONSERVATIVE_CO_ALBEDO;
  return {
    opticalDepth: layer.opticalDepth,
    omega: conservative ? 1 : layer.singleScatteringAlbedo,
    conservative,
    moments,
  };
}

function prepareBeams(
  problem: DiscreteOrdinatesProblem,
  layers: ReadonlyArray<PreparedLayer>,
): PreparedBeam[] {
  const cosines = Array.from(problem.beamCosines);
  const paths = problem.beamPaths;
  if (paths !== undefined && paths.length !== cosines.length) {
    throw new RangeError(`${paths.length} beam paths for ${cosines.length} beams`);
  }
  return cosines.map((cosine, b) => {
    requireIn("beamCosine", cosine, (v) => v > 0 && v <= 1, "in (0, 1]");
    const path = paths?.[b] ?? planeParallelBeam(problem.layers, cosine);
    const topTransmission = Float64Array.from(path.topTransmission);
    const slantFactor = Float64Array.from(path.slantFactor);
    if (topTransmission.length !== layers.length || slantFactor.length !== layers.length) {
      throw new RangeError(`beam ${b}'s path needs one entry a layer, ${layers.length} of them`);
    }
    topTransmission.forEach((t) => {
      requireIn(`beam ${b}'s topTransmission`, t, (v) => v >= 0 && v <= 1, "in [0, 1]");
    });
    slantFactor.forEach((lambda) => {
      requireIn(`beam ${b}'s slantFactor`, lambda, (v) => v > 0, "positive and finite");
    });
    const last = layers.length - 1;
    const groundTransmission =
      at(topTransmission, last) *
      Math.exp(-at(slantFactor, last) * (layers[last]?.opticalDepth ?? Number.NaN));
    return { cosine, topTransmission, slantFactor, groundTransmission };
  });
}

function requireStreams(streams: number): void {
  if (!Number.isInteger(streams) || streams < 2 || streams % 2 !== 0) {
    throw new RangeError(`the stream count must be an even integer of at least 2, not ${streams}`);
  }
}

function requireMode(mode: number, streams: number): void {
  if (!Number.isInteger(mode) || mode < 0 || mode > streams - 1) {
    throw new RangeError(`the mode must be a whole number in [0, ${streams - 1}], not ${mode}`);
  }
}

function requireIn(
  name: string,
  value: number,
  valid: (v: number) => boolean,
  domain: string,
): void {
  if (!Number.isFinite(value) || !valid(value)) {
    throw new RangeError(`${name} must be ${domain}, not ${value}`);
  }
}

function multiply(a: Float64Array, b: Float64Array, n: number): Float64Array {
  const out = new Float64Array(n * n);
  for (let i = 0; i < n; i += 1) {
    for (let j = 0; j < n; j += 1) {
      let sum = 0;
      for (let k = 0; k < n; k += 1) {
        sum += at(a, i * n + k) * at(b, k * n + j);
      }
      out[i * n + j] = sum;
    }
  }
  return out;
}

/** An entry that the loops' bounds keep in range; NaN, never `undefined`, should they not. */
function at(a: Float64Array, i: number): number {
  return a[i] ?? Number.NaN;
}
