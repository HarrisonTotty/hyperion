/**
 * The polarisation correction (plan R08, R08.T14.e; Design note 10, "Polarisation", ruled
 * 2026-10-09 in decision-r08-vector item 2): the vector mode of the discrete-ordinates solver
 * (`discreteOrdinates.ts`) for the Stokes vector (I, Q, U, V), and the bake of the correction
 * ΔJ_pol(h, μ₀, μ_v, m) = J_vector − J_scalar (the I component) that R08.T14.f adds to every
 * kernel's multiple-scattering term.
 *
 * @remarks
 * **The frame** is the reference tracer's (`hyperion-fit`'s `atmosphere::reference`, R08.T12.a):
 * for a direction of travel k of polar angle θ from the upward vertical (μ = cos θ,
 * positive upward) and azimuth φ, counted from north toward east in a right-handed east, north,
 * up frame (clockwise seen from above), e₁ = ∂k ÷ ∂θ lies in k's vertical (meridian) plane and
 * e₂ = k × e₁ is horizontal. Q = I₁ − I₂ and U = I(+45°) − I(−45°), the +45° direction from e₁
 * toward e₂. Δφ is the azimuth of the stream's direction of travel less the beam's, as the scalar
 * solver's ({@link PolarisedBeamSolution}), counted in that sense: I and Q go as cos mΔφ and U and
 * V as sin mΔφ, so that the sense sets the sign of U and V (counted anticlockwise, both would
 * flip). For an observer, Δφ is the azimuth looked toward less the sun's, both counted north to
 * east. Its Stokes parameters at a relative azimuth are Hovenier and van der Mee 1983's (their
 * eqs. 87–95), though its construction is their frame's mirror image: they count azimuth
 * anticlockwise seen from above (p. 6) and take r × ℓ = k (p. 7).
 *
 * **The scattering matrix** of a term in its scattering plane (e∥, e⊥, k right-handed) is
 * F = [[a₁, b₁, 0, 0], [b₁, a₂, 0, 0], [0, 0, a₃, b₂], [0, 0, −b₂, a₄]], normalised as a₁ is
 * (`medium.ts`'s `PhaseMatrixTable`; b₁ = ½(|S₂|² − |S₁|²), negative for Rayleigh at 90°).
 * Expanded in the generalised spherical functions (W. A. de Rooij and C. C. A. H. van der Stap,
 * A&A 131 (1984) 237; J. W. Hovenier and C. V. M. van der Mee, A&A 128 (1983) 1), here written
 * as Wigner's d^l_mn(Θ) ({@link wignerD}): a₁ and a₄ in d^l_00 = P_l, a₂ + a₃ in d^l_22, a₂ − a₃
 * in d^l_2,−2, and b₁ and b₂ in d^l_02 ({@link MatrixMoments}, each coefficient over 2l + 1, the
 * scalar solver's χ_l for a₁). Rayleigh's are closed ({@link rayleighMatrixMoments}); a tabulated
 * term's are its table's quadrature ({@link tableMatrixMoments}); a term with no matrix is a total
 * depolariser, a₁ alone. A phase known by its moments alone (`spectralColumn.ts`'s `moments`: a
 * kept scene's Cornette–Shanks aerosol, and a channel source's blend between two channels) is one
 * too, a stated approximation (R08.T14.c): the blend carries no matrix.
 *
 * **The azimuthal modes** (derived here through the circular-polarisation representation and the
 * addition theorem of the d-functions, the route Hovenier and van der Mee 1983 take, and held by
 * `polarisation.test.ts` to the matrix rotated geometrically into the meridian frames). For mode m,
 * the stream (I^m, Q^m, U^m, V^m), I and Q the cos mΔφ coefficients and U and V the sin mΔφ ones,
 * scatters by Π^m(μ, μ′) = Σ_l (2l + 1) P^m_l(μ) B_l P^m_l(μ′), with
 * P^m_l(μ) = [[d_m0, 0, 0, 0], [0, R, −T, 0], [0, −T, R, 0], [0, 0, 0, d_m0]],
 * R = ½(d^l_m2 + d^l_m,−2) and T = ½(d^l_m,−2 − d^l_m2) at θ = acos μ (T's sign the azimuth's
 * sense: counted anticlockwise, T would be −T), and
 * B_l = [[a₁, b₁, 0, 0], [b₁, a₂, 0, 0], [0, 0, a₃, b₂], [0, 0, −b₂, a₄]] of the moments at l: the
 * form of C. E. Siewert, JQSRT 64 (2000) 227, eqs. 2–6 and 8–9, and of Hovenier and van der Mee
 * 1983's W^j (their eqs. 107, 115 and 187–189), both read for the check after the derivation (plan
 * R08's record of R08.T14.e). Hovenier and van der Mee's B_l (their eqs. 178–179, after Siewert
 * 1981) carries −ε_l at (3, 4), which their eq. 204 makes (2l + 1) b₂ here, the same entry. Each
 * mode obeys
 * μ dI^m ÷ dτ = I^m − (ω ÷ 2) ∫ Π^m(μ, μ′) I^m(μ′) dμ′ − (2 − δ_m0)(ω ÷ 4π) Π^m(μ, −μ₀) s₀ T(τ),
 * s₀ = (1, 0, 0, 0) the unpolarised beam's Stokes vector: the scalar solver's equation with its
 * kernel a matrix. At m = 0 U and V vanish and (I, Q) are
 * solved; past it (I, Q, U), and V too where some term has b₂ ({@link PolarisedProblem.circular}),
 * since with b₂ = 0 V decouples and an unpolarised beam leaves it 0.
 *
 * **The eigenproblem.** With J⁻ = ΔI⁻, Δ = diag(1, 1, −1, −1), the two hemispheres obey the scalar
 * solver's [[−α, −β], [β, α]] system, α ± β = M⁻¹(S±W − 1), S± = (ω ÷ 2) Σ_l (2l + 1) P_l B_l
 * (1 ± (−1)^{l+m} Δ) P_l on the nodes, and (α − β)(α + β) X = k² X as there. With b₂ = 0 the S±
 * are symmetric and the scalar solver's symmetric-definite reduction (the Cholesky factor of −P and
 * cyclic Jacobi) carries over, its k² real. With b₂ ≠ 0 they are not, and the eigenvalues of a
 * product of two matrices self-adjoint only in an indefinite metric can be complex: those layers
 * are solved by the real nonsymmetric QR iteration (`linearAlgebra.ts`'s `realEigen`) on
 * (−P)(−Q), a complex pair k, k̄ giving the two real solutions Re and Im of G e^{−kt}, and the
 * pairs met are counted ({@link ModeEigenRecord}). A −P that is not definite on the symmetric path
 * is refused, as the scalar solver refuses it: the moments' operator is not a contraction.
 * m = 0 carries no V and is always symmetric; there ω = 1 is the conservative solution, the null
 * pair replaced by the unpolarised I = 1 and I = t ± u, as the scalar solver's.
 *
 * Everything else is the scalar solver's: the double-Gauss quadrature, the pseudo-spherical beam's
 * seam, the shifted and averaged beam at an eigenvalue's reciprocal, the Stamnes–Conklin scaling
 * and the banded boundary-value problem (`factorLayerStack`), and the formal solution at the user
 * cosines. The Lambertian ground and the decks depolarise: they reflect I alone.
 *
 * **Delta-M on the matrix** (derived here: none of the papers read truncates the full matrix, and
 * Hovenier and van der Mee 1983, p. 11, call it an open problem). a₁ is truncated as the thick
 * bake's (`bake.ts`'s `deltaMLayer`, W. J. Wiscombe, J. Atmos. Sci. 34 (1977) 1408): f =
 * max(a₁_NSTR, f_c, 0), τ′ = (1 − ωf)τ, ω′ = (1 − f)ω ÷ (1 − ωf), a₁_l′ = (a₁_l − f) ÷ (1 − f). One
 * τ′ for all four components takes the removed peak as the identity, since ωf M I joins the
 * extinction as (1 − ωf) I only where M = 1. So every element keeps its moments below NSTR, as the
 * DISORT report's eq. 54b keeps a₁'s (K. Stamnes, S.-C. Tsay, W. Wiscombe and I. Laszlo,
 * "DISORT, a General-Purpose Fortran Program for Discrete-Ordinate-Method Radiative Transfer in
 * Scattering and Emitting Layered Media: Documentation of Methodology", v1.1, 2000), applied to
 * each: a₂, a₃ and a₄ lose f, x′ = (x − f) ÷ (1 − f), and b₁ and b₂, which the identity has none
 * of, become b ÷ (1 − f); l < NSTR. For a term with a matrix, the identity is its own forward
 * matrix (as `aerosol.ts` lays a phase file's peak, a₂ = a₃ = a₄ = a₁ and no b). A depolariser's
 * own peak, a₁ alone, would also take Q, U and V out of the forward beam, and its negative
 * a₂′ … a₄′ (−f ÷ (1 − f) for a depolariser alone) are that, kept to NSTR. Taking its peak as the
 * identity instead, and its a₂ … a₄ as 0, is 3 to 18 times further from 64- and 128-stream solves
 * of air with a depolarising aerosol (plan R08's record of R08.T14.e). With no b, Q is never made,
 * so the depolarisers' I is the scalar I whatever a₂ … a₄ are.
 *
 * **The bake** ({@link bakePolarisationCorrection}). Per band, light and render channel, on the
 * thick bake's grid and layers (`bake.ts`'s `bakeLayering`), and for each term of the gases'
 * exponential sums in the bake bin that holds the channel's wavelength (`bakeLightBins`, the thick
 * bake's own fits): the delta-M layers of the channel's optics, the pseudo-spherical beam through
 * their depths (`sphericalBeam.ts`), and an 8-stream vector and scalar solve on them, m = 0 … 2. At
 * each table point, with the point's own mixed optics (σ_s, f, f_c and the truncated moments), each
 * solve's source of orders two and up per unit of the whole σ_s (forward deltas included) is
 * J = (f − f_c) I_d(Ω) + (1 − f)(Σ_l (2l + 1)
 * P_l(μ_v) (a₁′ H_I + b₁′ H_Q) ÷ 2 + the aureole's term), the thick bake's E ÷ σ_s, H the diffuse
 * field's half-range moments: the vector's I row reads its Q through b₁. ΔJ = J_vector − J_scalar,
 * the terms' weights summed. The single scattering of the unpolarised beam, which the kernels add
 * exactly, and the aureole's, the I row of a₁′ alone, are the same in both and cancel.
 */

import { gaussLegendre as gaussLegendreSymmetric } from "../../lighting/quadrature";
import { BAKE_WAVELENGTHS_NM, CHANNEL_WAVELENGTHS_NM, densityAt } from "../medium";
import {
  type BakeLayering,
  bakeLayering,
  type BakeLightBin,
  bakeLightBins,
  binGroundAlbedo,
  type BinTerm,
  checkBakeInputs,
  deltaMLayer,
  THICK_BAKE_GRID,
  THICK_BAKE_REFERENCE_BIN,
  type ThickBakeBand,
  thickBakeBands,
  type ThickBakeBody,
  type ThickBakeGrid,
  type ThickBakeLight,
} from "./bake";
import { binOfRow } from "./binConversion";
import {
  type BeamPath,
  DISCRETE_ORDINATE_CONSERVATIVE_CO_ALBEDO,
  DISCRETE_ORDINATE_STREAMS,
  type DiscreteOrdinatesSolution,
  doubleGaussQuadrature,
  type DoubleGaussQuadrature,
  factorLayerStack,
  type InterfaceFluxes,
  type LayerFaces,
  layerStackRightHandSide,
  normalizedAssociatedLegendre,
  type OrdinateLayer,
  planeParallelBeam,
  RESONANCE_HALF_WIDTH,
  solveDiscreteOrdinates,
  solveLayerStack,
  type StreamDirection,
} from "./discreteOrdinates";
import { EXP_SUM_OPTIONS, type ExpSumOptions } from "./expSum";
import { type SphericalBeam, sphericalBeamPath } from "./sphericalBeam";
import {
  forwardDeltaShare,
  phaseMoments,
  type SpectralColumn,
  spectralOptics,
  type SpectralPhase,
  type SpectralSource,
  type SpectralTerm,
  type SpectralTermOptics,
} from "./spectralColumn";
import {
  choleskyLower,
  factorDense,
  realEigen,
  solveDense,
  solveLower,
  solveLowerTransposed,
  symmetricEigen,
} from "./linearAlgebra";

// ---------------------------------------------------------------------------------------------
// The generalised spherical functions.

/**
 * Wigner's d^l_mn(θ), l = 0 … `maxDegree`, 0 below l = max(|m|, |n|), at μ = cos θ: the
 * generalised spherical functions the scattering matrix is expanded in (the module's remarks).
 *
 * @remarks
 * By the three-term recurrence in l (as M. I. Mishchenko, L. D. Travis and A. A. Lacis,
 * _Scattering, Absorption, and Emission of Light by Small Particles_, 2002, Appendix B, give it;
 * cited, not re-read: the forms were derived here and are held by the tests to closed forms):
 * l √((l + 1)² − m²) √((l + 1)² − n²) d^{l+1} = (2l + 1)(l(l + 1)μ − mn) d^l −
 * (l + 1) √(l² − m²) √(l² − n²) d^{l−1}, from
 * d^{l₀} = ξ √((2l₀)! ÷ (|m − n|! |m + n|!)) ((1 − μ) ÷ 2)^{|m−n|/2} ((1 + μ) ÷ 2)^{|m+n|/2},
 * l₀ = max(|m|, |n|), ξ = 1 for n ≥ m and (−1)^{m−n} otherwise: the convention in which
 * d^1_10 = −sin θ ÷ √2, d^l_m0 = (−1)^m Λ_l^m (`discreteOrdinates.ts`'s normalised Legendre
 * functions), d^l_mn = (−1)^{m−n} d^l_nm and d^l_mn(π − θ) = (−1)^{l+m} d^l_m,−n(θ).
 *
 * @throws RangeError for m or n not whole, a degree not a whole number ≥ 0, or a cosine outside
 *   [−1, 1].
 */
export function wignerD(m: number, n: number, maxDegree: number, mu: number): Float64Array {
  if (!Number.isInteger(m) || !Number.isInteger(n)) {
    throw new RangeError(`d^l_mn needs whole m and n, not ${m} and ${n}`);
  }
  if (!Number.isInteger(maxDegree) || maxDegree < 0) {
    throw new RangeError(`d^l_mn needs a whole degree ≥ 0, not ${maxDegree}`);
  }
  if (!(mu >= -1 && mu <= 1)) {
    throw new RangeError(`d^l_mn needs a cosine in [−1, 1], not ${mu}`);
  }
  const out = new Float64Array(maxDegree + 1);
  wignerInto(m, n, maxDegree, mu, out, 0, 1);
  return out;
}

/** d^l_mn(μ), l = 0 … `maxDegree`, into `out[offset + l * stride]` (zeros below l₀ left as they are). */
function wignerInto(
  m: number,
  n: number,
  maxDegree: number,
  mu: number,
  out: Float64Array,
  offset: number,
  stride: number,
): void {
  const lowest = Math.max(Math.abs(m), Math.abs(n));
  if (lowest > maxDegree) {
    return;
  }
  const a = Math.abs(m - n);
  const b = Math.abs(m + n);
  let binomial = 1;
  for (let i = 1; i <= a; i += 1) {
    binomial = (binomial * (b + i)) / i;
  }
  const below = Math.sqrt(Math.max((1 - mu) / 2, 0));
  const above = Math.sqrt(Math.max((1 + mu) / 2, 0));
  let value = Math.sqrt(binomial) * below ** a * above ** b;
  if (n < m && (m - n) % 2 !== 0) {
    value = -value;
  }
  out[offset + lowest * stride] = value;
  let previous = 0;
  let current = value;
  for (let l = lowest; l < maxDegree; l += 1) {
    const next =
      l === 0
        ? mu * current
        : ((2 * l + 1) * (l * (l + 1) * mu - m * n) * current -
            (l + 1) * Math.sqrt((l * l - m * m) * (l * l - n * n)) * previous) /
          (l * Math.sqrt(((l + 1) ** 2 - m * m) * ((l + 1) ** 2 - n * n)));
    out[offset + (l + 1) * stride] = next;
    previous = current;
    current = next;
  }
}

/** The functions P^m_l(μ) is built of at a set of cosines, each (maxDegree + 1) × count. */
interface GsfTable {
  /** d^l_m0, entry (l, i) at `l * count + i`. */
  readonly zero: Float64Array;
  /** R = ½(d^l_m2 + d^l_m,−2). */
  readonly even: Float64Array;
  /** T = ½(d^l_m,−2 − d^l_m2), in the tracer's sense of azimuth (the module's remarks). */
  readonly odd: Float64Array;
  readonly count: number;
}

function gsfTable(m: number, maxDegree: number, cosines: ArrayLike<number>): GsfTable {
  const count = cosines.length;
  const size = (maxDegree + 1) * count;
  const zero = new Float64Array(size);
  const plus = new Float64Array(size);
  const minus = new Float64Array(size);
  for (let i = 0; i < count; i += 1) {
    const mu = cosines[i] ?? Number.NaN;
    wignerInto(m, 0, maxDegree, mu, zero, i, count);
    wignerInto(m, 2, maxDegree, mu, plus, i, count);
    wignerInto(m, -2, maxDegree, mu, minus, i, count);
  }
  const even = plus.map((p, k) => 0.5 * (p + at(minus, k)));
  // T in the tracer's sense of azimuth, north toward east (the module's remarks).
  const odd = plus.map((p, k) => 0.5 * (at(minus, k) - p));
  return { zero, even, odd, count };
}

// ---------------------------------------------------------------------------------------------
// The scattering matrix's moments.

/**
 * A scattering matrix's moments, each element against its generalised spherical function and
 * normalised as the scalar solver's χ_l (the module's remarks): with μ = cos Θ,
 * a₁_l = ½ ∫ a₁ P_l dμ (χ_l, a₁_0 = 1), a₂_l ± a₃_l = ½ ∫ (a₂ ± a₃) d^l_2,±2 dμ,
 * a₄_l = ½ ∫ a₄ P_l dμ and b_l = ½ ∫ b d^l_02 dμ for b₁ and b₂. a₁ … a₄ are de Rooij and van der
 * Stap's α₁ … α₄ over 2l + 1, but b₁_l = −β₁ ÷ (2l + 1) and b₂_l = −β₂ ÷ (2l + 1): their
 * generalised spherical function P^l_02 is −d^l_02 (Mishchenko, Travis and Lacis 2002, eq. B.30;
 * Hovenier and van der Mee 1983, eq. A20). Published coefficients are loaded with that sign.
 *
 * @remarks
 * a₂, a₃, b₁ and b₂ begin at l = 2 (the d-functions they take vanish below it); their entries
 * below it are read as 0. A total depolariser's are a₁ alone, the rest 0.
 */
export interface MatrixMoments {
  readonly a1: ArrayLike<number>;
  readonly a2: ArrayLike<number>;
  readonly a3: ArrayLike<number>;
  readonly a4: ArrayLike<number>;
  readonly b1: ArrayLike<number>;
  readonly b2: ArrayLike<number>;
}

/** The matrix's six elements, in {@link MatrixMoments}' order. */
const ELEMENTS = ["a1", "a2", "a3", "a4", "b1", "b2"] as const;

/** {@link MatrixMoments} held as typed arrays. */
interface MomentArrays extends MatrixMoments {
  readonly a1: Float64Array;
  readonly a2: Float64Array;
  readonly a3: Float64Array;
  readonly a4: Float64Array;
  readonly b1: Float64Array;
  readonly b2: Float64Array;
}

/** An empty array, read as NaN past its end. */
const NO_VALUES = new Float64Array(0);

/** A {@link MatrixMoments} of zeros, l = 0 … `order`. */
function zeroMoments(order: number): MomentArrays {
  return {
    a1: new Float64Array(order + 1),
    a2: new Float64Array(order + 1),
    a3: new Float64Array(order + 1),
    a4: new Float64Array(order + 1),
    b1: new Float64Array(order + 1),
    b2: new Float64Array(order + 1),
  };
}

/**
 * Rayleigh's matrix moments with the depolarisation ratio ρ, in closed form: Hansen and Travis's
 * matrix (J. E. Hansen and L. D. Travis, Space Sci. Rev. 16 (1974) 527, eqs. 2.15–2.16, their δ
 * this ρ), Δ times the dipole's plus (1 − Δ) times diag(1, 0, 0, 0), its a₄ further times Δ′, with
 * Δ = (1 − ρ) ÷ (1 + ρ ÷ 2) and Δ′ = (1 − 2ρ) ÷ (1 − ρ).
 *
 * @remarks
 * Derived here: a₁ = 1 − Δ + ¾Δ(1 + μ²) = 1 + (Δ ÷ 2) P₂, so a₁_2 = Δ ÷ 10, `spectralColumn.ts`'s
 * `phaseMoments` χ₂ = (1 − ρ) ÷ (5(2 + ρ)); a₂ ± a₃ = ¾Δ(1 ± μ)² = 3Δ d²_2,±2, so a₂_2 = 3Δ ÷ 5 and
 * a₃ = 0; a₄ = (3 ÷ 2)ΔΔ′ μ, so a₄_1 = ΔΔ′ ÷ 2; and b₁ = −¾Δ(1 − μ²) = −Δ √6 ÷ 2 d²_02, so
 * b₁_2 = −Δ √6 ÷ 10. b₂ = 0. The tests hold each to a Gauss–Legendre quadrature of the matrix. The
 * a₁ moments are `phaseMoments`' own, so that the scalar and the vector solve read the same.
 *
 * @param order - The highest l, an even whole number ≥ 2 (`phaseMoments`' rule).
 * @param depolarisation - ρ, in [0, ½).
 * @param a1 - χ₀ … χ_order as `phaseMoments` gives them for this phase.
 */
export function rayleighMatrixMoments(
  order: number,
  depolarisation: number,
  a1: ArrayLike<number>,
): MatrixMoments {
  const rho = depolarisation;
  const big = (1 - rho) / (1 + rho / 2);
  const circular = (1 - 2 * rho) / (1 - rho);
  const moments = zeroMoments(order);
  for (let l = 0; l <= order; l += 1) {
    moments.a1[l] = a1[l] ?? 0;
  }
  moments.a2[2] = (3 * big) / 5;
  moments.a4[1] = (big * circular) / 2;
  moments.b1[2] = (-big * Math.sqrt(6)) / 10;
  return moments;
}

/**
 * The Gauss–Legendre rule each interval of a table's u is integrated with, (x, w) on [−1, 1]:
 * `forwardPeak.ts`'s, so that a table's moments here are its `phaseTableMoment`'s.
 */
const TABLE_RULE: ReadonlyArray<readonly [number, number]> = (() => {
  const rule = gaussLegendreSymmetric(8);
  return Array.from(rule.x, (x, i) => [x, rule.w[i] ?? Number.NaN] as const);
})();

/**
 * A phase table's matrix moments (a₂ … b₂) to `order`, from its `PhaseMatrixTable` on its own u,
 * by the quadrature `forwardPeak.ts`'s `phaseTableMoment` takes (each element linear in u between
 * entries, eight Gauss–Legendre points an interval, ∫ e w dΩ for an element e normalised as the
 * table's a₁, per steradian), so that its a₁ would be `phaseMoments`'.
 *
 * @param u - The table's u = √(θ ÷ π), ascending from 0 to 1.
 * @param elements - a₂, a₃, a₄, b₁ and b₂ at each u, each normalised as a₁ is (per steradian).
 * @param a1 - χ₀ … χ_order of the same table (`phaseMoments`).
 */
export function tableMatrixMoments(
  order: number,
  u: Float64Array,
  elements: {
    readonly a2: Float64Array;
    readonly a3: Float64Array;
    readonly a4: Float64Array;
    readonly b1: Float64Array;
    readonly b2: Float64Array;
  },
  a1: ArrayLike<number>,
): MatrixMoments {
  const moments = zeroMoments(order);
  for (let l = 0; l <= order; l += 1) {
    moments.a1[l] = a1[l] ?? 0;
  }
  const zero = new Float64Array(order + 1);
  const plus = new Float64Array(order + 1);
  const minus = new Float64Array(order + 1);
  const across = new Float64Array(order + 1);
  const { a2, a3, a4, b1, b2 } = elements;
  const sumPlus = new Float64Array(order + 1);
  const sumMinus = new Float64Array(order + 1);
  const sumA4 = new Float64Array(order + 1);
  const sumB1 = new Float64Array(order + 1);
  const sumB2 = new Float64Array(order + 1);
  for (let i = 0; i + 1 < u.length; i += 1) {
    const u0 = at(u, i);
    const u1 = at(u, i + 1);
    const half = 0.5 * (u1 - u0);
    const lerp = (e: Float64Array, t: number): number => at(e, i) + t * (at(e, i + 1) - at(e, i));
    for (const [x, w] of TABLE_RULE) {
      const t = 0.5 * (1 + x);
      const uu = u0 + t * (u1 - u0);
      const angle = Math.PI * uu * uu;
      const mu = Math.cos(angle);
      const weight = half * w * Math.sin(angle) * 2 * Math.PI * uu;
      wignerInto(0, 0, order, mu, zero, 0, 1);
      wignerInto(2, 2, order, mu, plus, 0, 1);
      wignerInto(2, -2, order, mu, minus, 0, 1);
      wignerInto(0, 2, order, mu, across, 0, 1);
      const v2 = lerp(a2, t);
      const v3 = lerp(a3, t);
      const vPlus = weight * (v2 + v3);
      const vMinus = weight * (v2 - v3);
      const v4 = weight * lerp(a4, t);
      const vb1 = weight * lerp(b1, t);
      const vb2 = weight * lerp(b2, t);
      for (let l = 0; l <= order; l += 1) {
        sumPlus[l] = at(sumPlus, l) + vPlus * at(plus, l);
        sumMinus[l] = at(sumMinus, l) + vMinus * at(minus, l);
        sumA4[l] = at(sumA4, l) + v4 * at(zero, l);
        sumB1[l] = at(sumB1, l) + vb1 * at(across, l);
        sumB2[l] = at(sumB2, l) + vb2 * at(across, l);
      }
    }
  }
  for (let l = 0; l <= order; l += 1) {
    const p = 2 * Math.PI * at(sumPlus, l);
    const q = 2 * Math.PI * at(sumMinus, l);
    moments.a2[l] = l < 2 ? 0 : 0.5 * (p + q);
    moments.a3[l] = l < 2 ? 0 : 0.5 * (p - q);
    moments.a4[l] = 2 * Math.PI * at(sumA4, l);
    moments.b1[l] = l < 2 ? 0 : 2 * Math.PI * at(sumB1, l);
    moments.b2[l] = l < 2 ? 0 : 2 * Math.PI * at(sumB2, l);
  }
  return moments;
}

/**
 * The phase matrix Z(μ, μ′, Δφ) in the meridian frames (the module's remarks), 4 × 4 row-major,
 * summed from its azimuthal modes, l = 0 … `maxDegree`: Z = Π⁰ + 2 Σ_{m≥1} (E^m cos mΔφ + O^m
 * sin mΔφ), E^m the 2 × 2 diagonal blocks of Π^m and O^m = −D × its off-diagonal blocks,
 * D = diag(1, 1, −1, −1). It normalises as a₁: ∫ Z₁₁ dΩ ÷ 4π = 1.
 *
 * @remarks
 * For the tests, which hold it to the scattering matrix rotated geometrically from each direction's
 * meridian plane into the scattering plane and back.
 *
 * @param mu - The scattered direction's μ, in [−1, 1].
 * @param muPrime - The incident direction's μ′.
 * @param deltaPhiRad - Δφ = φ − φ′, rad.
 */
export function meridianPhaseMatrix(
  moments: MatrixMoments,
  maxDegree: number,
  mu: number,
  muPrime: number,
  deltaPhiRad: number,
): Float64Array {
  const z = new Float64Array(16);
  const layer = prepareVectorLayer(
    { opticalDepth: 0, singleScatteringAlbedo: 1, moments },
    0,
    maxDegree + 1,
  );
  const unit = new Float64Array(4);
  const right = new Float64Array(4);
  const middle = new Float64Array(4);
  const left = new Float64Array(4);
  for (let m = 0; m <= maxDegree; m += 1) {
    const out = gsfTable(m, maxDegree, [mu]);
    const inc = gsfTable(m, maxDegree, [muPrime]);
    const pi = new Float64Array(16);
    for (let l = m; l <= maxDegree; l += 1) {
      for (let d = 0; d < 4; d += 1) {
        unit.fill(0);
        unit[d] = 1;
        applyP(at(inc.zero, l), at(inc.even, l), at(inc.odd, l), unit, 0, right, 0, 4);
        applyB(layer, l, 2 * l + 1, right, 0, middle, 0, 4);
        applyP(at(out.zero, l), at(out.even, l), at(out.odd, l), middle, 0, left, 0, 4);
        for (let c = 0; c < 4; c += 1) {
          pi[c * 4 + d] = at(pi, c * 4 + d) + at(left, c);
        }
      }
    }
    const factor = m === 0 ? 1 : 2;
    const cos = Math.cos(m * deltaPhiRad);
    const sin = Math.sin(m * deltaPhiRad);
    for (let c = 0; c < 4; c += 1) {
      for (let d = 0; d < 4; d += 1) {
        const value = at(pi, c * 4 + d);
        const diagonalBlock = c < 2 === d < 2;
        z[c * 4 + d] =
          at(z, c * 4 + d) + factor * (diagonalBlock ? value * cos : -deltaSign(c) * value * sin);
      }
    }
  }
  return z;
}

/** A total depolariser's moments: a₁ alone (the module's remarks). */
export function depolarisingMoments(order: number, a1: ArrayLike<number>): MatrixMoments {
  const moments = zeroMoments(order);
  for (let l = 0; l <= order; l += 1) {
    moments.a1[l] = a1[l] ?? 0;
  }
  return moments;
}

// ---------------------------------------------------------------------------------------------
// The problem and its solution.

/** One homogeneous layer of a vector solve, each quantity dimensionless. */
export interface PolarisedLayer {
  /** τ, its vertical optical depth: finite and at least 0. */
  readonly opticalDepth: number;
  /** ω, in [0, 1]. */
  readonly singleScatteringAlbedo: number;
  /**
   * Its matrix's moments ({@link MatrixMoments}), a₁_0 = 1: l = 0 … NSTR − 1 enter the solve, later
   * ones are ignored (their truncation is the caller's) and missing ones are 0.
   */
  readonly moments: MatrixMoments;
}

/** A vector solve: the layers, the ground, the beams, the user cosines and the modes. */
export interface PolarisedProblem {
  /** The layers from the top down; at least one. */
  readonly layers: ReadonlyArray<PolarisedLayer>;
  /** A, the ground's Lambertian albedo, in [0, 1]: it reflects I alone, depolarised. */
  readonly groundAlbedo: number;
  /** μ₀ of each beam, in (0, 1]: an unpolarised beam of flux 1 normal to itself. */
  readonly beamCosines: ArrayLike<number>;
  /** The user cosines, in (0, 1]: each is solved upward and downward. */
  readonly userCosines: ArrayLike<number>;
  /** M: the modes 0 … M, M ≤ NSTR − 1. */
  readonly maxMode: number;
  /** NSTR, even and at least 2; `discreteOrdinates.ts`'s default when absent. */
  readonly streams?: number;
  /** One per beam: their paths, plane-parallel when absent (`discreteOrdinates.ts`'s seam). */
  readonly beamPaths?: ReadonlyArray<BeamPath>;
  /**
   * Whether V is carried past m = 0; by default where some layer's b₂ is not 0 (the module's
   * remarks: with b₂ = 0 everywhere V stays 0).
   */
  readonly circular?: boolean;
}

/** One mode's Stokes intensities for one beam, at every interface. */
export interface StokesModeIntensities {
  /** m. */
  readonly mode: number;
  /** C, the components carried: 2 (I, Q) at m = 0, 3 (I, Q, U) or 4 (and V) past it. */
  readonly components: number;
  /** (L + 1) × N × C, entry ((k N + i) C + c): interface k, node i, component c, upward. */
  readonly quadratureUp: Float64Array;
  /** The same, downward. */
  readonly quadratureDown: Float64Array;
  /** (L + 1) × U × C, entry ((k U + u) C + c): user cosine u, upward. */
  readonly userUp: Float64Array;
  /** The same, downward. */
  readonly userDown: Float64Array;
}

/**
 * One beam's vector solution: its modes and its I fluxes (m = 0), the beam's flux normal to itself
 * 1.
 *
 * @remarks
 * I = Σ_m I^m cos mΔφ and Q likewise, U = Σ_m U^m sin mΔφ and V likewise ({@link stokesAtAzimuth}),
 * Δφ the azimuth of the stream's direction of travel less the beam's, counted from north toward
 * east (clockwise seen from above), in the frame of the module's remarks.
 */
export interface PolarisedBeamSolution {
  readonly beamCosine: number;
  readonly modes: ReadonlyArray<StokesModeIntensities>;
  /** The I component's fluxes, `discreteOrdinates.ts`'s. */
  readonly fluxes: InterfaceFluxes;
}

/** What one mode's eigenproblems met, over the layers. */
export interface ModeEigenRecord {
  readonly mode: number;
  readonly components: number;
  /** The layers solved by the nonsymmetric QR iteration. */
  readonly nonsymmetricLayers: number;
  /** The complex pairs k, k̄ met, over the layers. */
  readonly complexPairs: number;
  /** The layers, from the top, that met one. */
  readonly complexLayers: ReadonlyArray<number>;
  /**
   * The smallest and largest |Im k²| ÷ |k²| of the pairs, [∞, 0] with none: a pair of an eigenvalue
   * repeated to rounding carries a share near ε, a genuine one a share of its own.
   */
  readonly imaginaryShares: readonly [number, number];
}

/** A vector solve's results. */
export interface PolarisedSolution {
  readonly quadrature: DoubleGaussQuadrature;
  /** τ_k, k = 0 … L. */
  readonly interfaceDepths: Float64Array;
  readonly userCosines: Float64Array;
  /** Whether V was carried. */
  readonly circular: boolean;
  readonly beams: ReadonlyArray<PolarisedBeamSolution>;
  /** One a mode, in order. */
  readonly eigen: ReadonlyArray<ModeEigenRecord>;
}

/** The components carried at mode m. */
function componentsOf(m: number, circular: boolean): number {
  if (m === 0) {
    return 2;
  }
  return circular ? 4 : 3;
}

// ---------------------------------------------------------------------------------------------
// The layer's homogeneous solutions.

/** A layer as solved: ω after the conservative rule and its moments to NSTR − 1. */
interface PreparedVectorLayer {
  readonly opticalDepth: number;
  readonly omega: number;
  readonly conservative: boolean;
  readonly a1: Float64Array;
  readonly a2: Float64Array;
  readonly a3: Float64Array;
  readonly a4: Float64Array;
  readonly b1: Float64Array;
  readonly b2: Float64Array;
  /** Whether some b₂ is not 0. */
  readonly circular: boolean;
}

/**
 * One layer's solutions at one mode. Solution s's decaying form is part(G e^{−κt}) at the
 * intensities (G⁺, Δ G_J⁻) and its growing form part(G̃ e^{−κ(τ − t)}) at (G_J⁻, Δ G⁺), part Re
 * or Im by its kind, κ and G its pair's (the module's remarks).
 */
interface VectorLayerMode {
  readonly layer: PreparedVectorLayer;
  readonly mode: number;
  readonly components: number;
  /** n = N C. */
  readonly size: number;
  readonly rateRe: Float64Array;
  readonly rateIm: Float64Array;
  /** 0 a real solution, 1 a pair's real part, 2 its imaginary part. */
  readonly kind: Uint8Array;
  /** G⁺ and G_J⁻, n × n solution-major: entry (s, a) at `s * n + a`. */
  readonly upRe: Float64Array;
  readonly upIm: Float64Array;
  readonly downRe: Float64Array;
  readonly downIm: Float64Array;
  readonly conservative: boolean;
  /** u of the conservative linear solution. */
  readonly linear: Float64Array;
  /** d_a = 1 ÷ √(w_a μ_a). */
  readonly scale: Float64Array;
  /** −Q, n × n, for the particular solution. */
  readonly negQ: Float64Array;
  /** The eigenproblem's path and what its particular solution needs of it. */
  readonly path: LayerModePath;
  /** The complex pairs met. */
  readonly complexPairs: number;
  /** The smallest and largest |Im k²| ÷ |k²| of the pairs met, [∞, 0] with none. */
  readonly imaginaryShares: readonly [number, number];
  /** The source at the user cosines as a matrix ({@link userSourceMatrix}). */
  readonly userMatrix: Float64Array;
}

/**
 * How a layer-mode's eigenproblem was solved (the module's remarks), and what the beam's particular
 * solution (`vectorParticular`) takes of it.
 *
 * - `symmetric`: the scalar solver's Cholesky-and-Jacobi reduction, with k² and the maps, each
 *   n × n: Yᵀ Lᵀ and Yᵀ L⁻¹ (row k), D L Y and D L⁻ᵀ Y (column k), and D L⁻ᵀ L⁻¹.
 * - `nonsymmetric`: the real QR iteration on (−P)(−Q), with that product and −P.
 */
type LayerModePath =
  | {
      readonly kind: "symmetric";
      readonly decayRatesSquared: Float64Array;
      readonly qMap: Float64Array;
      readonly pMap: Float64Array;
      readonly sumMap: Float64Array;
      readonly diffMap: Float64Array;
      readonly inverseMap: Float64Array;
    }
  | { readonly kind: "nonsymmetric"; readonly product: Float64Array; readonly negP: Float64Array };

/** Row a's node and component: a = i C + c. */
function nodeOf(a: number, components: number): number {
  return Math.floor(a / components);
}

/** Applies P^m_l(μ) (the module's remarks) of functions (p, r, t) to `v` into `out`, C components. */
function applyP(
  p: number,
  r: number,
  t: number,
  v: ArrayLike<number>,
  vOffset: number,
  out: Float64Array,
  outOffset: number,
  components: number,
): void {
  const vi = v[vOffset] ?? Number.NaN;
  const vq = v[vOffset + 1] ?? Number.NaN;
  out[outOffset] = p * vi;
  if (components === 2) {
    out[outOffset + 1] = r * vq;
    return;
  }
  const vu = v[vOffset + 2] ?? Number.NaN;
  out[outOffset + 1] = r * vq - t * vu;
  out[outOffset + 2] = -t * vq + r * vu;
  if (components === 4) {
    out[outOffset + 3] = p * (v[vOffset + 3] ?? Number.NaN);
  }
}

/** Applies B_l of a layer's moments at l, times `factor`, to `v` into `out`, C components. */
function applyB(
  layer: PreparedVectorLayer,
  l: number,
  factor: number,
  v: ArrayLike<number>,
  vOffset: number,
  out: Float64Array,
  outOffset: number,
  components: number,
): void {
  const vi = v[vOffset] ?? Number.NaN;
  const vq = v[vOffset + 1] ?? Number.NaN;
  const b1 = at(layer.b1, l);
  out[outOffset] = factor * (at(layer.a1, l) * vi + b1 * vq);
  out[outOffset + 1] = factor * (b1 * vi + at(layer.a2, l) * vq);
  if (components === 2) {
    return;
  }
  const vu = v[vOffset + 2] ?? Number.NaN;
  if (components === 3) {
    out[outOffset + 2] = factor * at(layer.a3, l) * vu;
    return;
  }
  const vv = v[vOffset + 3] ?? Number.NaN;
  const b2 = at(layer.b2, l);
  out[outOffset + 2] = factor * (at(layer.a3, l) * vu + b2 * vv);
  out[outOffset + 3] = factor * (-b2 * vu + at(layer.a4, l) * vv);
}

/**
 * S⁺ and S⁻ (the module's remarks), n × n row-major, n = N C: per l, ω (2l + 1) P_l(μ_i) B_l E
 * P_l(μ_j) with E = diag(1, 1, 0, 0) for the parity l + m takes (even for S⁺) and diag(0, 0, 1, 1)
 * for the other.
 */
function parityMatrices(
  layer: PreparedVectorLayer,
  m: number,
  components: number,
  nodes: GsfTable,
  lmax: number,
): { readonly plus: Float64Array; readonly minus: Float64Array } {
  const count = nodes.count;
  const n = count * components;
  const plus = new Float64Array(n * n);
  const minus = new Float64Array(n * n);
  const column = new Float64Array(components);
  const projected = new Float64Array(components);
  const scattered = new Float64Array(components);
  const block = new Float64Array(components);
  const unit = new Float64Array(components);
  for (let l = m; l <= lmax; l += 1) {
    const coefficient = layer.omega * (2 * l + 1);
    if (
      at(layer.a1, l) === 0 &&
      at(layer.a2, l) === 0 &&
      at(layer.a3, l) === 0 &&
      at(layer.a4, l) === 0 &&
      at(layer.b1, l) === 0 &&
      at(layer.b2, l) === 0
    ) {
      continue;
    }
    const evenParity = (l + m) % 2 === 0;
    for (let j = 0; j < count; j += 1) {
      const pj = at(nodes.zero, l * count + j);
      const rj = at(nodes.even, l * count + j);
      const tj = at(nodes.odd, l * count + j);
      for (let d = 0; d < components; d += 1) {
        // Column d of P_l(μ_j), split by E into S⁺'s rows and S⁻'s.
        unit.fill(0);
        unit[d] = 1;
        applyP(pj, rj, tj, unit, 0, column, 0, components);
        for (const target of [plus, minus]) {
          const keepsLinear = (target === plus) === evenParity;
          for (let c = 0; c < components; c += 1) {
            projected[c] = c < 2 === keepsLinear ? at(column, c) : 0;
          }
          applyB(layer, l, coefficient, projected, 0, scattered, 0, components);
          for (let i = 0; i < count; i += 1) {
            applyP(
              at(nodes.zero, l * count + i),
              at(nodes.even, l * count + i),
              at(nodes.odd, l * count + i),
              scattered,
              0,
              block,
              0,
              components,
            );
            for (let c = 0; c < components; c += 1) {
              const k = (i * components + c) * n + j * components + d;
              target[k] = at(target, k) + at(block, c);
            }
          }
        }
      }
    }
  }
  return { plus, minus };
}

/** −P or −Q from S∓: M^{−½}(1 − W^{½} S W^{½})M^{−½}, the node's μ and w on each of its rows. */
function negativeOf(
  s: Float64Array,
  quadrature: DoubleGaussQuadrature,
  components: number,
): Float64Array {
  const n = quadrature.nodes.length * components;
  const out = new Float64Array(n * n);
  for (let a = 0; a < n; a += 1) {
    const i = nodeOf(a, components);
    const wi = at(quadrature.weights, i);
    const mi = at(quadrature.nodes, i);
    for (let b = 0; b < n; b += 1) {
      const j = nodeOf(b, components);
      const root = Math.sqrt(wi * at(quadrature.weights, j));
      const across = 1 / Math.sqrt(mi * at(quadrature.nodes, j));
      out[a * n + b] = ((a === b ? 1 : 0) - root * at(s, a * n + b)) * across;
    }
  }
  return out;
}

/** A refusal for moments whose discrete scattering operator is not a contraction. */
function notContraction(m: number, detail: string, cause?: unknown): RangeError {
  return new RangeError(
    `layer moments give mode ${m} ${detail} (truncate a strongly forward-peaked phase function's ` +
      "moments, by delta-M, first)",
    cause === undefined ? undefined : { cause },
  );
}

function vectorLayerMode(
  layer: PreparedVectorLayer,
  m: number,
  components: number,
  quadrature: DoubleGaussQuadrature,
  functions: ModeFunctions,
): VectorLayerMode {
  const lmax = quadrature.streams - 1;
  const count = quadrature.nodes.length;
  const n = count * components;
  const { plus, minus } = parityMatrices(layer, m, components, functions.nodes, lmax);
  const negP = negativeOf(minus, quadrature, components);
  const negQ = negativeOf(plus, quadrature, components);
  const scale = Float64Array.from({ length: n }, (_, a) => {
    const i = nodeOf(a, components);
    return 1 / Math.sqrt(at(quadrature.weights, i) * at(quadrature.nodes, i));
  });
  const conservative = m === 0 && layer.conservative;
  const rateRe = new Float64Array(n);
  const rateIm = new Float64Array(n);
  const kind = new Uint8Array(n);
  const upRe = new Float64Array(n * n);
  const upIm = new Float64Array(n * n);
  const downRe = new Float64Array(n * n);
  const downIm = new Float64Array(n * n);
  const linear = new Float64Array(n);
  let path: LayerModePath;
  let complexPairs = 0;
  let smallestShare = Number.POSITIVE_INFINITY;
  let largestShare = 0;
  if (m === 0 || !(components === 4 && layer.circular)) {
    let cholesky: Float64Array;
    try {
      cholesky = choleskyLower(negP, n);
    } catch (error) {
      // As the scalar solver refuses the same moments: the operator is not a contraction.
      throw notContraction(m, "a scattering operator that is not a contraction", error);
    }
    // The scalar solver's symmetric-definite reduction: H = Lᵀ(−Q)L, by Jacobi.
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
    const k2 = eigen.values;
    const largest = at(k2, n - 1);
    for (let k = 0; k < n; k += 1) {
      const value = at(k2, k);
      const isNull = conservative && k === 0;
      if (isNull ? !(Math.abs(value) <= 1e-8 * Math.max(largest, 1)) : !(value > 0)) {
        throw notContraction(
          m,
          `an eigenvalue k² = ${value}, ${isNull ? "not the conservative 0" : "not positive"}`,
        );
      }
    }
    const ly = new Float64Array(n);
    const lty = new Float64Array(n);
    const sumMap = new Float64Array(n * n);
    const diffMap = new Float64Array(n * n);
    for (let k = 0; k < n; k += 1) {
      const rate = conservative && k === 0 ? 0 : Math.sqrt(at(k2, k));
      rateRe[k] = rate;
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
        upRe[k * n + i] = half * (at(ly, i) - rate * at(lty, i));
        downRe[k * n + i] = half * (at(ly, i) + rate * at(lty, i));
        sumMap[i * n + k] = at(scale, i) * at(ly, i);
        diffMap[i * n + k] = at(scale, i) * at(lty, i);
      }
    }
    // The particular solution's maps (`vectorParticular`): Yᵀ Lᵀ, Yᵀ L⁻¹ and D L⁻ᵀ L⁻¹.
    const lInverse = new Float64Array(n * n);
    const unitColumn = new Float64Array(n);
    for (let j = 0; j < n; j += 1) {
      unitColumn.fill(0);
      unitColumn[j] = 1;
      solveLower(cholesky, n, unitColumn);
      for (let i = 0; i < n; i += 1) {
        lInverse[i * n + j] = at(unitColumn, i);
      }
    }
    const qMap = new Float64Array(n * n);
    const pMap = new Float64Array(n * n);
    const inverseMap = new Float64Array(n * n);
    for (let k = 0; k < n; k += 1) {
      for (let i = 0; i < n; i += 1) {
        let lt = 0;
        let li = 0;
        let pi = 0;
        for (let r = 0; r < n; r += 1) {
          const y = at(eigen.vectors, r * n + k);
          lt += y * at(cholesky, i * n + r);
          li += y * at(lInverse, r * n + i);
          pi += at(lInverse, r * n + k) * at(lInverse, r * n + i);
        }
        qMap[k * n + i] = lt;
        pMap[k * n + i] = li;
        inverseMap[k * n + i] = at(scale, k) * pi;
      }
    }
    path = { kind: "symmetric", decayRatesSquared: k2, qMap, pMap, sumMap, diffMap, inverseMap };
    if (conservative) {
      // u = −(α − β)⁻¹𝟙, 𝟙 the unpolarised field (1, 0) at every node.
      for (let a = 0; a < n; a += 1) {
        linear[a] = a % components === 0 ? 1 / at(scale, a) : 0;
      }
      solveLower(cholesky, n, linear);
      solveLowerTransposed(cholesky, n, linear);
      for (let a = 0; a < n; a += 1) {
        linear[a] = at(linear, a) * at(scale, a);
      }
    }
  } else {
    // The nonsymmetric path on (−P)(−Q), whose eigenvectors z give X = D z and, with w = (−P)⁻¹z,
    // G⁺ = (D ÷ 2)(z − k w) and G_J⁻ = (D ÷ 2)(z + k w).
    const product = multiply(negP, negQ, n);
    const eigen = realEigen(product, n);
    const negPLu = factorDense(negP, n);
    const wRe = new Float64Array(n);
    const wIm = new Float64Array(n);
    for (let k = 0; k < n; k += 1) {
      const lr = at(eigen.valuesRe, k);
      const li = at(eigen.valuesIm, k);
      if (li < 0) {
        continue;
      }
      if (li === 0 && !(lr > 0)) {
        throw notContraction(m, `an eigenvalue k² = ${lr}, not positive`);
      }
      // κ = √(k²), the principal root, Re κ > 0.
      const modulus = Math.hypot(lr, li);
      const kr = Math.sqrt((modulus + lr) / 2);
      const ki = li === 0 ? 0 : li / (2 * kr);
      if (!(kr > 0)) {
        throw notContraction(m, `an eigenvalue k² = ${lr} + ${li}i with no decay`);
      }
      for (let a = 0; a < n; a += 1) {
        wRe[a] = at(eigen.vectors, a * n + k);
        wIm[a] = li > 0 ? at(eigen.vectors, a * n + k + 1) : 0;
      }
      const zRe = Float64Array.from(wRe);
      const zIm = Float64Array.from(wIm);
      solveDense(negPLu, wRe);
      if (li > 0) {
        solveDense(negPLu, wIm);
      }
      const members = li > 0 ? [k, k + 1] : [k];
      for (const [member, s] of members.entries()) {
        rateRe[s] = kr;
        rateIm[s] = ki;
        kind[s] = li > 0 ? member + 1 : 0;
        for (let a = 0; a < n; a += 1) {
          const half = at(scale, a) / 2;
          // k w, complex.
          const kwRe = kr * at(wRe, a) - ki * at(wIm, a);
          const kwIm = kr * at(wIm, a) + ki * at(wRe, a);
          upRe[s * n + a] = half * (at(zRe, a) - kwRe);
          upIm[s * n + a] = half * (at(zIm, a) - kwIm);
          downRe[s * n + a] = half * (at(zRe, a) + kwRe);
          downIm[s * n + a] = half * (at(zIm, a) + kwIm);
        }
      }
      if (li > 0) {
        complexPairs += 1;
        smallestShare = Math.min(smallestShare, li / modulus);
        largestShare = Math.max(largestShare, li / modulus);
      }
    }
    path = { kind: "nonsymmetric", product, negP };
  }
  return {
    layer,
    mode: m,
    components,
    size: n,
    rateRe,
    rateIm,
    kind,
    upRe,
    upIm,
    downRe,
    downIm,
    conservative,
    linear,
    scale,
    negQ,
    path,
    complexPairs,
    imaginaryShares: [smallestShare, largestShare],
    userMatrix: userSourceMatrix(layer, m, components, quadrature, functions),
  };
}

/**
 * The source at the user cosines as a matrix ({@link userSourceVector}'s map): row
 * `(d U + u) C + c`, d = 0 upward (+μ_u) and 1 downward (−μ_u), column a of I⁺ at the nodes
 * (a = j C + e) and n + a of I⁻; per l, (ω ÷ 2)(2l + 1) P_l(±μ_u) B_l times the half-range moments'
 * map w_j (P_l(μ_j), P_l(−μ_j)).
 */
function userSourceMatrix(
  layer: PreparedVectorLayer,
  m: number,
  components: number,
  quadrature: DoubleGaussQuadrature,
  functions: ModeFunctions,
): Float64Array {
  const count = quadrature.nodes.length;
  const n = count * components;
  const { nodes, users } = functions;
  const userCount = users.count;
  const width = userCount * components;
  const columns = 2 * n;
  const out = new Float64Array(2 * width * columns);
  const lmax = quadrature.streams - 1;
  const moments = new Float64Array(components * columns);
  const unit = new Float64Array(components);
  const column = new Float64Array(components);
  const scattered = new Float64Array(components);
  const across = new Float64Array(2 * components * components);
  for (let l = m; l <= lmax; l += 1) {
    if (ELEMENTS.every((element) => at(layer[element], l) === 0)) {
      continue;
    }
    const sign = (l + m) % 2 === 0 ? 1 : -1;
    moments.fill(0);
    for (let j = 0; j < count; j += 1) {
      const w = at(quadrature.weights, j);
      const p = at(nodes.zero, l * count + j);
      const r = at(nodes.even, l * count + j);
      const t = at(nodes.odd, l * count + j);
      for (let d = 0; d < components; d += 1) {
        unit.fill(0);
        unit[d] = 1;
        applyP(p, r, t, unit, 0, column, 0, components);
        for (let e = 0; e < components; e += 1) {
          // P_l(μ_j)'s column d, and P_l(−μ_j)'s: s Δ P_l(μ_j) Δ.
          moments[e * columns + j * components + d] = w * at(column, e);
          moments[e * columns + n + j * components + d] =
            w * sign * deltaSign(e) * deltaSign(d) * at(column, e);
        }
      }
    }
    const coefficient = (layer.omega / 2) * (2 * l + 1);
    for (let u = 0; u < userCount; u += 1) {
      const p = at(users.zero, l * userCount + u);
      const r = at(users.even, l * userCount + u);
      const t = at(users.odd, l * userCount + u);
      for (let e = 0; e < components; e += 1) {
        unit.fill(0);
        unit[e] = 1;
        applyB(layer, l, coefficient, unit, 0, scattered, 0, components);
        applyP(p, r, t, scattered, 0, column, 0, components);
        for (let c = 0; c < components; c += 1) {
          across[c * components + e] = at(column, c);
          scattered[c] = deltaSign(c) * at(scattered, c);
        }
        applyP(p, r, t, scattered, 0, column, 0, components);
        for (let c = 0; c < components; c += 1) {
          across[(components + c) * components + e] = sign * deltaSign(c) * at(column, c);
        }
      }
      for (let direction = 0; direction < 2; direction += 1) {
        for (let c = 0; c < components; c += 1) {
          const row = (direction * width + u * components + c) * columns;
          for (let e = 0; e < components; e += 1) {
            const g = at(across, (direction * components + c) * components + e);
            if (g === 0) {
              continue;
            }
            for (let k = 0; k < columns; k += 1) {
              out[row + k] = at(out, row + k) + g * at(moments, e * columns + k);
            }
          }
        }
      }
    }
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// Complex helpers for the pairs.

/** e^{−zΔ} for z = (re, im), as (re, im). */
function expNeg(re: number, im: number, depth: number): readonly [number, number] {
  const magnitude = Math.exp(-re * depth);
  return [magnitude * Math.cos(im * depth), -magnitude * Math.sin(im * depth)];
}

/** φ(z, Δ) = (1 − e^{−zΔ}) ÷ z, Re z ≥ 0, Δ at z = 0 (`discreteOrdinates.ts`'s, complex). */
function phiComplex(re: number, im: number, depth: number): readonly [number, number] {
  const xr = re * depth;
  const xi = im * depth;
  if (Math.hypot(xr, xi) < 1e-4) {
    // 1 − x ÷ 2 + x² ÷ 6 − x³ ÷ 24, times Δ.
    const x2r = xr * xr - xi * xi;
    const x2i = 2 * xr * xi;
    const x3r = x2r * xr - x2i * xi;
    const x3i = x2r * xi + x2i * xr;
    return [depth * (1 - xr / 2 + x2r / 6 - x3r / 24), depth * (-xi / 2 + x2i / 6 - x3i / 24)];
  }
  // 1 − e^{−x} = −expm1(−x_r) cos x_i + 2 sin²(x_i ÷ 2) + i e^{−x_r} sin x_i.
  const decay = Math.exp(-xr);
  const sinHalf = Math.sin(xi / 2);
  const nr = -Math.expm1(-xr) * Math.cos(xi) + 2 * sinHalf * sinHalf;
  const ni = decay * Math.sin(xi);
  // ÷ z.
  const d = re * re + im * im;
  return [(nr * re + ni * im) / d, (ni * re - nr * im) / d];
}

/** ψ(a, b, Δ) = (e^{−aΔ} − e^{−bΔ}) ÷ (b − a), a complex and b real, both of real part ≥ 0. */
function psiComplex(re: number, im: number, b: number, depth: number): readonly [number, number] {
  if (re <= b) {
    const [er, ei] = expNeg(re, im, depth);
    const [fr, fi] = phiComplex(b - re, -im, depth);
    return [er * fr - ei * fi, er * fi + ei * fr];
  }
  const eb = Math.exp(-b * depth);
  const [fr, fi] = phiComplex(re - b, im, depth);
  return [eb * fr, eb * fi];
}

/** φ(z, Δ), real (`discreteOrdinates.ts`'s). */
function phi(z: number, depth: number): number {
  const x = z * depth;
  return x === 0 ? depth : -Math.expm1(-x) / z;
}

/** ψ(a, b, Δ), real (`discreteOrdinates.ts`'s). */
function psi(a: number, b: number, depth: number): number {
  return Math.exp(-Math.min(a, b) * depth) * phi(Math.abs(a - b), depth);
}

// ---------------------------------------------------------------------------------------------
// The layer stack's faces and the user angles.

/** Δ = diag(1, 1, −1, −1)'s sign on component c. */
function deltaSign(c: number): number {
  return c < 2 ? 1 : -1;
}

/** The real or imaginary part of x + i y by a solution's kind. */
function partOf(kind: number, re: number, im: number): number {
  return kind === 2 ? im : re;
}

/** A vector layer's solutions at its faces (the module's remarks, the scalar solver's scaling). */
function vectorFaces(lm: VectorLayerMode): LayerFaces {
  const n = lm.size;
  const span = 2 * n;
  const depth = lm.layer.opticalDepth;
  const top = new Float64Array(span * span);
  const bottom = new Float64Array(span * span);
  const { components } = lm;
  for (let s = 0; s < n; s += 1) {
    const decaying = s;
    const growing = n + s;
    if (lm.conservative && s === 0) {
      for (let a = 0; a < n; a += 1) {
        const one = a % components === 0 ? 1 : 0;
        const u = at(lm.linear, a);
        for (const face of [top, bottom]) {
          face[a * span + decaying] = one;
          face[(n + a) * span + decaying] = one;
        }
        top[a * span + growing] = u;
        top[(n + a) * span + growing] = -u;
        bottom[a * span + growing] = depth * one + u;
        bottom[(n + a) * span + growing] = depth * one - u;
      }
      continue;
    }
    const kind = lm.kind[s] ?? 0;
    const [er, ei] = expNeg(at(lm.rateRe, s), at(lm.rateIm, s), depth);
    for (let a = 0; a < n; a += 1) {
      const sign = deltaSign(a % components);
      const ur = at(lm.upRe, s * n + a);
      const ui = at(lm.upIm, s * n + a);
      const dr = at(lm.downRe, s * n + a);
      const di = at(lm.downIm, s * n + a);
      // Decaying: (G⁺, Δ G_J⁻); growing: (G_J⁻, Δ G⁺).
      top[a * span + decaying] = partOf(kind, ur, ui);
      top[(n + a) * span + decaying] = sign * partOf(kind, dr, di);
      bottom[a * span + decaying] = partOf(kind, ur * er - ui * ei, ur * ei + ui * er);
      bottom[(n + a) * span + decaying] = sign * partOf(kind, dr * er - di * ei, dr * ei + di * er);
      top[a * span + growing] = partOf(kind, dr * er - di * ei, dr * ei + di * er);
      top[(n + a) * span + growing] = sign * partOf(kind, ur * er - ui * ei, ur * ei + ui * er);
      bottom[a * span + growing] = partOf(kind, dr, di);
      bottom[(n + a) * span + growing] = sign * partOf(kind, ur, ui);
    }
  }
  return { top, bottom };
}

/** The functions the source takes at the user cosines and the nodes, at one mode. */
interface ModeFunctions {
  readonly nodes: GsfTable;
  readonly users: GsfTable;
}

/**
 * The source (ω ÷ 2) Σ_l (2l + 1) P_l(±μ_u) B_l H_l of quadrature intensities (I⁺, I⁻), H_l =
 * Σ_j w_j (P_l(μ_j) I⁺_j + P_l(−μ_j) I⁻_j), at each user cosine upward and downward, U × C each:
 * the layer-mode's {@link userSourceMatrix} applied to them.
 */
function userSourceVector(
  lm: VectorLayerMode,
  intensityUp: Float64Array,
  intensityDown: Float64Array,
  outUp: Float64Array,
  outDown: Float64Array,
): void {
  const n = lm.size;
  const columns = 2 * n;
  const width = outUp.length;
  const matrix = lm.userMatrix;
  for (let direction = 0; direction < 2; direction += 1) {
    const out = direction === 0 ? outUp : outDown;
    for (let x = 0; x < width; x += 1) {
      const row = (direction * width + x) * columns;
      let sum = 0;
      for (let a = 0; a < n; a += 1) {
        sum +=
          at(matrix, row + a) * at(intensityUp, a) + at(matrix, row + n + a) * at(intensityDown, a);
      }
      out[x] = sum;
    }
  }
}

/** A layer's solutions integrated along each user stream, 2n × (U C) each, and its attenuation. */
interface VectorUserTransfer {
  readonly up: Float64Array;
  readonly down: Float64Array;
  readonly attenuation: Float64Array;
}

function vectorUserTransfer(lm: VectorLayerMode, userCosines: Float64Array): VectorUserTransfer {
  const n = lm.size;
  const { components } = lm;
  const users = userCosines.length;
  const width = users * components;
  const depth = lm.layer.opticalDepth;
  const up = new Float64Array(2 * n * width);
  const down = new Float64Array(2 * n * width);
  const attenuation = Float64Array.from(userCosines, (mu) => Math.exp(-depth / mu));
  const sourceUpRe = new Float64Array(width);
  const sourceDownRe = new Float64Array(width);
  const sourceUpIm = new Float64Array(width);
  const sourceDownIm = new Float64Array(width);
  const vectorUp = new Float64Array(n);
  const vectorDown = new Float64Array(n);
  for (let s = 0; s < n; s += 1) {
    const decaying = s;
    const growing = n + s;
    if (lm.conservative && s === 0) {
      for (let a = 0; a < n; a += 1) {
        const one = a % components === 0 ? 1 : 0;
        vectorUp[a] = one;
        vectorDown[a] = one;
      }
      userSourceVector(lm, vectorUp, vectorDown, sourceUpRe, sourceDownRe);
      const constantUp = Float64Array.from(sourceUpRe);
      const constantDown = Float64Array.from(sourceDownRe);
      for (let a = 0; a < n; a += 1) {
        vectorUp[a] = at(lm.linear, a);
        vectorDown[a] = -at(lm.linear, a);
      }
      userSourceVector(lm, vectorUp, vectorDown, sourceUpRe, sourceDownRe);
      for (let u = 0; u < users; u += 1) {
        const mu = at(userCosines, u);
        const passed = -Math.expm1(-depth / mu);
        const e = at(attenuation, u);
        for (let c = 0; c < components; c += 1) {
          const x = u * components + c;
          up[decaying * width + x] = at(constantUp, x) * passed;
          down[decaying * width + x] = at(constantDown, x) * passed;
          up[growing * width + x] =
            at(constantUp, x) * (mu * passed - depth * e) + at(sourceUpRe, x) * passed;
          down[growing * width + x] =
            at(constantDown, x) * (depth - mu * passed) + at(sourceDownRe, x) * passed;
        }
      }
      continue;
    }
    const kind = lm.kind[s] ?? 0;
    const kr = at(lm.rateRe, s);
    const ki = at(lm.rateIm, s);
    // The decaying solution's intensities, (G⁺, Δ G_J⁻), real and imaginary parts.
    for (let a = 0; a < n; a += 1) {
      vectorUp[a] = at(lm.upRe, s * n + a);
      vectorDown[a] = deltaSign(a % components) * at(lm.downRe, s * n + a);
    }
    userSourceVector(lm, vectorUp, vectorDown, sourceUpRe, sourceDownRe);
    if (ki !== 0) {
      for (let a = 0; a < n; a += 1) {
        vectorUp[a] = at(lm.upIm, s * n + a);
        vectorDown[a] = deltaSign(a % components) * at(lm.downIm, s * n + a);
      }
      userSourceVector(lm, vectorUp, vectorDown, sourceUpIm, sourceDownIm);
    }
    for (let u = 0; u < users; u += 1) {
      const mu = at(userCosines, u);
      const inverse = 1 / mu;
      let alongRe: number;
      let alongIm: number;
      let againstRe: number;
      let againstIm: number;
      if (ki === 0) {
        alongRe = inverse * phi(kr + inverse, depth);
        againstRe = inverse * psi(kr, inverse, depth);
        alongIm = 0;
        againstIm = 0;
      } else {
        const along = phiComplex(kr + inverse, ki, depth);
        const against = psiComplex(kr, ki, inverse, depth);
        alongRe = inverse * along[0];
        alongIm = inverse * along[1];
        againstRe = inverse * against[0];
        againstIm = inverse * against[1];
      }
      for (let c = 0; c < components; c += 1) {
        const x = u * components + c;
        const sign = deltaSign(c);
        const jUr = at(sourceUpRe, x);
        const jUi = ki === 0 ? 0 : at(sourceUpIm, x);
        const jDr = at(sourceDownRe, x);
        const jDi = ki === 0 ? 0 : at(sourceDownIm, x);
        // The growing solution's source is Δ times the decaying one's, the hemispheres swapped.
        up[decaying * width + x] = partOf(
          kind,
          jUr * alongRe - jUi * alongIm,
          jUr * alongIm + jUi * alongRe,
        );
        down[decaying * width + x] = partOf(
          kind,
          jDr * againstRe - jDi * againstIm,
          jDr * againstIm + jDi * againstRe,
        );
        up[growing * width + x] =
          sign * partOf(kind, jDr * againstRe - jDi * againstIm, jDr * againstIm + jDi * againstRe);
        down[growing * width + x] =
          sign * partOf(kind, jUr * alongRe - jUi * alongIm, jUr * alongIm + jUi * alongRe);
      }
    }
  }
  return { up, down, attenuation };
}

// ---------------------------------------------------------------------------------------------
// The beam's particular solution.

/** The beam's particular solution per unit transmission at the layer's top: Z⁺ and Z⁻, n each. */
interface VectorParticular {
  readonly up: Float64Array;
  readonly down: Float64Array;
}

/**
 * The beam source's coefficients X⁺ and X⁻ at the nodes per unit transmission:
 * (2 − δ_m0)(ω ÷ 4π) Σ_l (2l + 1) P_l(±μ_i) B_l P_l(−μ₀) e₁.
 */
function beamSource(
  lm: VectorLayerMode,
  quadrature: DoubleGaussQuadrature,
  nodes: GsfTable,
  beamZero: Float64Array,
): { readonly up: Float64Array; readonly down: Float64Array } {
  const { components } = lm;
  const count = quadrature.nodes.length;
  const n = lm.size;
  const lmax = quadrature.streams - 1;
  const m = lm.mode;
  const strength = (lm.layer.omega * (m === 0 ? 1 : 2)) / (4 * Math.PI);
  const up = new Float64Array(n);
  const down = new Float64Array(n);
  for (let l = m; l <= lmax; l += 1) {
    const sign = (l + m) % 2 === 0 ? 1 : -1;
    // B_l P_l(−μ₀) e₁ = d^l_m0(−μ₀) (a₁, b₁, 0, 0), d^l_m0(−μ₀) = (−1)^{l+m} d^l_m0(μ₀).
    const d0 = sign * at(beamZero, l) * strength * (2 * l + 1);
    const vi = d0 * at(lm.layer.a1, l);
    const vq = d0 * at(lm.layer.b1, l);
    if (vi === 0 && vq === 0) {
      continue;
    }
    for (let i = 0; i < count; i += 1) {
      const p = at(nodes.zero, l * count + i);
      const r = at(nodes.even, l * count + i);
      const t = at(nodes.odd, l * count + i);
      const base = i * components;
      // P_l(μ_i) v and P_l(−μ_i) v = s (P v_I, R v_Q, T v_Q) for v = (v_I, v_Q, 0, 0).
      up[base] = at(up, base) + p * vi;
      up[base + 1] = at(up, base + 1) + r * vq;
      down[base] = at(down, base) + sign * p * vi;
      down[base + 1] = at(down, base + 1) + sign * r * vq;
      if (components > 2) {
        up[base + 2] = at(up, base + 2) - t * vq;
        down[base + 2] = at(down, base + 2) + sign * t * vq;
      }
    }
  }
  return { up, down };
}

/**
 * Z^± for the beam's source (the module's remarks): ((α − β)(α + β) − λ²) Zs =
 * −(α − β) M⁻¹ Xs − λ M⁻¹ Xd with Xs = X⁺ + Δ X⁻ and Xd = X⁺ − Δ X⁻, by the scalar solver's
 * eigenbasis on the symmetric path and by elimination on the nonsymmetric one.
 */
function vectorParticular(
  lm: VectorLayerMode,
  quadrature: DoubleGaussQuadrature,
  source: { readonly up: Float64Array; readonly down: Float64Array },
  slant: number,
): VectorParticular {
  const n = lm.size;
  const { components } = lm;
  const q = new Float64Array(n);
  const p = new Float64Array(n);
  for (let a = 0; a < n; a += 1) {
    const i = nodeOf(a, components);
    const factor = Math.sqrt(at(quadrature.weights, i) / at(quadrature.nodes, i));
    const mirrored = deltaSign(a % components) * at(source.down, a);
    q[a] = factor * (at(source.up, a) + mirrored);
    p[a] = factor * (at(source.up, a) - mirrored);
  }
  const { scale } = lm;
  const sum = new Float64Array(n);
  const diff = new Float64Array(n);
  const { path } = lm;
  switch (path.kind) {
    case "symmetric": {
      // The scalar solver's eigenbasis form through the layer-mode's maps: c = (Yᵀ Lᵀ q − λ Yᵀ L⁻¹
      // p) ÷ (k² − λ²), Zs = D L Y c and Zd = −λ D L⁻ᵀ Y c + D L⁻ᵀ L⁻¹ p.
      const { decayRatesSquared, qMap, pMap, sumMap, diffMap, inverseMap } = path;
      const lambda2 = slant * slant;
      const coefficients = new Float64Array(n);
      for (let k = 0; k < n; k += 1) {
        let s = 0;
        for (let i = 0; i < n; i += 1) {
          s += at(qMap, k * n + i) * at(q, i) - slant * at(pMap, k * n + i) * at(p, i);
        }
        coefficients[k] = s / (at(decayRatesSquared, k) - lambda2);
      }
      for (let i = 0; i < n; i += 1) {
        let s = 0;
        let d = 0;
        for (let k = 0; k < n; k += 1) {
          const c = at(coefficients, k);
          s += at(sumMap, i * n + k) * c;
          d += at(inverseMap, i * n + k) * at(p, k) - slant * at(diffMap, i * n + k) * c;
        }
        sum[i] = s;
        diff[i] = d;
      }
      break;
    }
    case "nonsymmetric": {
      const { product, negP } = path;
      // ((−P)(−Q) − λ²) z = (−P) q − λ p; Zs = D z; Zd = D (q − (−Q) z) ÷ λ.
      const shifted = Float64Array.from(product);
      for (let i = 0; i < n; i += 1) {
        shifted[i * n + i] = at(shifted, i * n + i) - slant * slant;
      }
      const z = new Float64Array(n);
      for (let i = 0; i < n; i += 1) {
        let s = 0;
        for (let j = 0; j < n; j += 1) {
          s += at(negP, i * n + j) * at(q, j);
        }
        z[i] = s - slant * at(p, i);
      }
      solveDense(factorDense(shifted, n), z);
      for (let i = 0; i < n; i += 1) {
        let s = 0;
        for (let j = 0; j < n; j += 1) {
          s += at(lm.negQ, i * n + j) * at(z, j);
        }
        sum[i] = at(scale, i) * at(z, i);
        diff[i] = (at(scale, i) * (at(q, i) - s)) / slant;
      }
      break;
    }
  }
  const up = new Float64Array(n);
  const down = new Float64Array(n);
  for (let a = 0; a < n; a += 1) {
    up[a] = (at(sum, a) + at(diff, a)) / 2;
    down[a] = (deltaSign(a % components) * (at(sum, a) - at(diff, a))) / 2;
  }
  return { up, down };
}

/** Whether layer `lm`'s slant factor lies within the resonance window of one of its rates. */
function isResonant(lm: VectorLayerMode, slant: number): boolean {
  for (let s = lm.conservative ? 1 : 0; s < lm.size; s += 1) {
    if (Math.hypot(at(lm.rateRe, s) - slant, at(lm.rateIm, s)) <= RESONANCE_HALF_WIDTH * slant) {
      return true;
    }
  }
  return false;
}

// ---------------------------------------------------------------------------------------------
// The solve.

/** What one mode's solve shares across its beams. */
interface VectorModeContext {
  readonly m: number;
  readonly components: number;
  readonly quadrature: DoubleGaussQuadrature;
  readonly layerModes: ReadonlyArray<VectorLayerMode>;
  readonly faces: ReadonlyArray<LayerFaces>;
  readonly transfers: ReadonlyArray<VectorUserTransfer>;
  readonly stack: ReturnType<typeof factorLayerStack>;
  readonly functions: ModeFunctions;
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
 * Solves the vector discrete-ordinates problem: per mode, every layer's homogeneous solutions, one
 * factorisation of the boundary-value problem, and each beam's particular solutions and Stokes
 * intensities at the nodes and user cosines (the module's remarks).
 *
 * @example
 * ```ts
 * const rho = 0.03;
 * const a1 = phaseMoments({ kind: "rayleigh", depolarisation: rho }, 16);
 * const solution = solvePolarised({
 *   layers: [{ opticalDepth: 0.5, singleScatteringAlbedo: 1, moments: rayleighMatrixMoments(16,
 *   rho, a1) }], groundAlbedo: 0,
 *   beamCosines: [0.2],
 *   userCosines: [0.1, 0.5, 1],
 *   maxMode: 2,
 * });
 * const [beam] = solution.beams;
 * if (beam === undefined) {
 *   throw new Error("one beam was asked");
 * }
 * const [i, q, u] = stokesAtAzimuth(beam, 0, 1, "up", Math.PI / 2);
 * ```
 *
 * @throws RangeError as `discreteOrdinates.ts`'s `solveDiscreteOrdinates` for the same inputs, for
 *   a moment array that is not finite, or for a matrix whose discrete scattering operator is not
 *   a contraction (an eigenvalue k² that is not positive, or no decay).
 */
export function solvePolarised(problem: PolarisedProblem): PolarisedSolution {
  const streams = problem.streams ?? DISCRETE_ORDINATE_STREAMS;
  requireStreams(streams);
  requireMode(problem.maxMode, streams);
  if (problem.layers.length < 1) {
    throw new RangeError("a solve needs at least one layer");
  }
  requireIn("groundAlbedo", problem.groundAlbedo, (v) => v >= 0 && v <= 1, "in [0, 1]");
  const layers = problem.layers.map((layer, p) => prepareVectorLayer(layer, p, streams));
  const circular = problem.circular ?? layers.some((layer) => layer.circular);
  const userCosines = Float64Array.from(problem.userCosines);
  userCosines.forEach((mu) => {
    requireIn("userCosine", mu, (v) => v > 0 && v <= 1, "in (0, 1]");
  });
  const beams = prepareBeams(problem, layers);
  const quadrature = doubleGaussQuadrature(streams);
  const count = quadrature.nodes.length;
  const layerCount = layers.length;
  const interfaceDepths = new Float64Array(layerCount + 1);
  layers.forEach((layer, p) => {
    interfaceDepths[p + 1] = at(interfaceDepths, p) + layer.opticalDepth;
  });
  const perBeam: StokesModeIntensities[][] = beams.map(() => []);
  const eigen: ModeEigenRecord[] = [];
  for (let m = 0; m <= problem.maxMode; m += 1) {
    const components = componentsOf(m, circular);
    const context = vectorModeContext(
      m,
      components,
      quadrature,
      layers,
      userCosines,
      problem.groundAlbedo,
    );
    const complexLayers = context.layerModes.flatMap((lm, p) => (lm.complexPairs > 0 ? [p] : []));
    eigen.push({
      mode: m,
      components,
      nonsymmetricLayers: context.layerModes.filter((lm) => lm.path.kind === "nonsymmetric").length,
      complexPairs: context.layerModes.reduce((sum, lm) => sum + lm.complexPairs, 0),
      complexLayers,
      imaginaryShares: [
        Math.min(...context.layerModes.map((lm) => lm.imaginaryShares[0])),
        Math.max(0, ...context.layerModes.map((lm) => lm.imaginaryShares[1])),
      ],
    });
    beams.forEach((beam, b) => {
      perBeam[b]?.push(solveVectorBeamMode(context, beam));
    });
  }
  const solved = beams.map((beam, b): PolarisedBeamSolution => {
    const modes = perBeam[b] ?? [];
    const zeroth = modes[0];
    if (zeroth === undefined) {
      throw new Error("mode 0 is always solved");
    }
    return { beamCosine: beam.cosine, modes, fluxes: fluxesOf(zeroth, beam, quadrature, count) };
  });
  return { quadrature, interfaceDepths, userCosines, circular, beams: solved, eigen };
}

/**
 * The Stokes vector (I, Q, U, V) at a user cosine at azimuth Δφ from a beam's modes: I and Q as
 * Σ_m X^m cos mΔφ, U and V as Σ_m X^m sin mΔφ ({@link PolarisedBeamSolution}).
 *
 * @param interfaceIndex - k, 0 at the top.
 * @param userIndex - u.
 * @param relativeAzimuthRad - Δφ, rad.
 * @throws RangeError for an interface or user index out of range.
 */
export function stokesAtAzimuth(
  beam: PolarisedBeamSolution,
  interfaceIndex: number,
  userIndex: number,
  direction: StreamDirection,
  relativeAzimuthRad: number,
): readonly [number, number, number, number] {
  const stokes = [0, 0, 0, 0];
  const interfaces = beam.fluxes.direct.length;
  for (const field of beam.modes) {
    let values: Float64Array;
    switch (direction) {
      case "up":
        values = field.userUp;
        break;
      case "down":
        values = field.userDown;
        break;
    }
    const users = values.length / (interfaces * field.components);
    if (
      !Number.isInteger(interfaceIndex) ||
      !Number.isInteger(userIndex) ||
      interfaceIndex < 0 ||
      interfaceIndex >= interfaces ||
      userIndex < 0 ||
      userIndex >= users
    ) {
      throw new RangeError(`no intensity at interface ${interfaceIndex}, user cosine ${userIndex}`);
    }
    const base = (interfaceIndex * users + userIndex) * field.components;
    const cos = Math.cos(field.mode * relativeAzimuthRad);
    const sin = Math.sin(field.mode * relativeAzimuthRad);
    for (let c = 0; c < field.components; c += 1) {
      stokes[c] = (stokes[c] ?? 0) + at(values, base + c) * (c < 2 ? cos : sin);
    }
  }
  return [stokes[0] ?? 0, stokes[1] ?? 0, stokes[2] ?? 0, stokes[3] ?? 0];
}

function vectorModeContext(
  m: number,
  components: number,
  quadrature: DoubleGaussQuadrature,
  layers: ReadonlyArray<PreparedVectorLayer>,
  userCosines: Float64Array,
  groundAlbedo: number,
): VectorModeContext {
  const lmax = quadrature.streams - 1;
  const count = quadrature.nodes.length;
  const n = count * components;
  const functions: ModeFunctions = {
    nodes: gsfTable(m, lmax, quadrature.nodes),
    users: gsfTable(m, lmax, userCosines),
  };
  const layerModes = layers.map((layer) =>
    vectorLayerMode(layer, m, components, quadrature, functions),
  );
  const faces = layerModes.map(vectorFaces);
  const transfers = layerModes.map((lm) => vectorUserTransfer(lm, userCosines));
  let reflection: Float64Array | undefined;
  if (m === 0 && groundAlbedo > 0) {
    // Lambertian and depolarising: I⁺ = 2A Σ_j w_j μ_j I⁻_j on the I rows alone.
    const r = new Float64Array(n * n);
    for (let i = 0; i < count; i += 1) {
      for (let j = 0; j < count; j += 1) {
        r[i * components * n + j * components] =
          2 * groundAlbedo * at(quadrature.weights, j) * at(quadrature.nodes, j);
      }
    }
    reflection = r;
  }
  const stack = factorLayerStack(faces, n, reflection);
  return {
    m,
    components,
    quadrature,
    layerModes,
    faces,
    transfers,
    stack,
    functions,
    userCosines,
    groundAlbedo,
  };
}

/** One beam's mode, solved once or, where a layer's λ is resonant, shifted both ways and averaged. */
function solveVectorBeamMode(
  context: VectorModeContext,
  beam: PreparedBeam,
): StokesModeIntensities {
  const resonant = context.layerModes.map(
    (lm, p) =>
      at(beam.topTransmission, p) > 0 &&
      lm.layer.omega > 0 &&
      isResonant(lm, at(beam.slantFactor, p)),
  );
  if (!resonant.includes(true)) {
    return solveVectorBeamModeAt(context, beam, beam.slantFactor);
  }
  const shifted = (sign: number): Float64Array =>
    beam.slantFactor.map((lambda, p) =>
      resonant[p] === true ? lambda * (1 + sign * 2 * RESONANCE_HALF_WIDTH) : lambda,
    );
  const below = solveVectorBeamModeAt(context, beam, shifted(-1));
  const above = solveVectorBeamModeAt(context, beam, shifted(1));
  const mean = (x: Float64Array, y: Float64Array): Float64Array =>
    x.map((value, i) => (value + at(y, i)) / 2);
  return {
    mode: below.mode,
    components: below.components,
    quadratureUp: mean(below.quadratureUp, above.quadratureUp),
    quadratureDown: mean(below.quadratureDown, above.quadratureDown),
    userUp: mean(below.userUp, above.userUp),
    userDown: mean(below.userDown, above.userDown),
  };
}

function solveVectorBeamModeAt(
  context: VectorModeContext,
  beam: PreparedBeam,
  slantFactor: Float64Array,
): StokesModeIntensities {
  const { m, components, quadrature, layerModes, faces, transfers, stack, functions } = context;
  const { userCosines, groundAlbedo } = context;
  const count = quadrature.nodes.length;
  const n = count * components;
  const span = 2 * n;
  const users = userCosines.length;
  const width = users * components;
  const layerCount = layerModes.length;
  const lmax = quadrature.streams - 1;
  const beamZero = new Float64Array(lmax + 1);
  wignerInto(m, 0, lmax, beam.cosine, beamZero, 0, 1);

  const particularTop: Float64Array[] = [];
  const particularBottom: Float64Array[] = [];
  const particularUserUp: Float64Array[] = [];
  const particularUserDown: Float64Array[] = [];
  const sourceUp = new Float64Array(width);
  const sourceDown = new Float64Array(width);
  const userBeam = gsfBeamAtUsers(functions.users, beamZero, m, lmax);
  layerModes.forEach((lm, p) => {
    const top = new Float64Array(span);
    const bottom = new Float64Array(span);
    const userUp = new Float64Array(width);
    const userDown = new Float64Array(width);
    const transmission = at(beam.topTransmission, p);
    const lambda = at(slantFactor, p);
    const { omega, opticalDepth: depth } = lm.layer;
    if (transmission > 0 && omega > 0) {
      const source = beamSource(lm, quadrature, functions.nodes, beamZero);
      const z = vectorParticular(lm, quadrature, source, lambda);
      const through = Math.exp(-lambda * depth);
      for (let a = 0; a < n; a += 1) {
        top[a] = transmission * at(z.up, a);
        top[n + a] = transmission * at(z.down, a);
        bottom[a] = transmission * through * at(z.up, a);
        bottom[n + a] = transmission * through * at(z.down, a);
      }
      // The source at the user cosines: the scattered particular solution and the beam itself.
      userSourceVector(lm, z.up, z.down, sourceUp, sourceDown);
      addBeamAtUsers(lm, userBeam, users, sourceUp, sourceDown);
      for (let u = 0; u < users; u += 1) {
        const inverse = 1 / at(userCosines, u);
        const along = transmission * inverse * phi(lambda + inverse, depth);
        const against = transmission * inverse * psi(lambda, inverse, depth);
        for (let c = 0; c < components; c += 1) {
          const x = u * components + c;
          userUp[x] = at(sourceUp, x) * along;
          userDown[x] = at(sourceDown, x) * against;
        }
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
  let groundSource: Float64Array | undefined;
  if (groundBeam > 0) {
    groundSource = new Float64Array(n);
    for (let i = 0; i < count; i += 1) {
      groundSource[i * components] = groundBeam;
    }
  }
  const coefficients = solveLayerStack(
    stack,
    layerStackRightHandSide(stack, particularTop, particularBottom, groundSource),
  );

  const quadratureUp = new Float64Array((layerCount + 1) * n);
  const quadratureDown = new Float64Array((layerCount + 1) * n);
  for (let k = 0; k <= layerCount; k += 1) {
    const p = Math.min(k, layerCount - 1);
    const face = faces[p];
    const part = (k < layerCount ? particularTop : particularBottom)[p];
    if (face === undefined || part === undefined) {
      throw new Error(`layer ${p} has no faces`);
    }
    const values = k < layerCount ? face.top : face.bottom;
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

  const userUp = new Float64Array((layerCount + 1) * width);
  const userDown = new Float64Array((layerCount + 1) * width);
  for (let p = 0; p < layerCount; p += 1) {
    const transfer = transfers[p];
    const part = particularUserDown[p];
    if (transfer === undefined || part === undefined) {
      throw new Error(`layer ${p} has no user-angle transfer`);
    }
    for (let u = 0; u < users; u += 1) {
      const e = at(transfer.attenuation, u);
      for (let c = 0; c < components; c += 1) {
        const x = u * components + c;
        let value = at(userDown, p * width + x) * e + at(part, x);
        for (let s = 0; s < span; s += 1) {
          value += at(coefficients, span * p + s) * at(transfer.down, s * width + x);
        }
        userDown[(p + 1) * width + x] = value;
      }
    }
  }
  if (m === 0 && groundAlbedo > 0) {
    let flux = 0;
    for (let j = 0; j < count; j += 1) {
      flux +=
        at(quadrature.weights, j) *
        at(quadrature.nodes, j) *
        at(quadratureDown, layerCount * n + j * components);
    }
    for (let u = 0; u < users; u += 1) {
      userUp[layerCount * width + u * components] = 2 * groundAlbedo * flux + groundBeam;
    }
  }
  for (let p = layerCount - 1; p >= 0; p -= 1) {
    const transfer = transfers[p];
    const part = particularUserUp[p];
    if (transfer === undefined || part === undefined) {
      throw new Error(`layer ${p} has no user-angle transfer`);
    }
    for (let u = 0; u < users; u += 1) {
      const e = at(transfer.attenuation, u);
      for (let c = 0; c < components; c += 1) {
        const x = u * components + c;
        let value = at(userUp, (p + 1) * width + x) * e + at(part, x);
        for (let s = 0; s < span; s += 1) {
          value += at(coefficients, span * p + s) * at(transfer.up, s * width + x);
        }
        userUp[p * width + x] = value;
      }
    }
  }
  return { mode: m, components, quadratureUp, quadratureDown, userUp, userDown };
}

/** P_l(μ_u)'s functions times d^l_m0(−μ₀), per l and user cosine: what the beam's term at the users takes. */
interface UserBeam {
  readonly users: GsfTable;
  /** d^l_m0(−μ₀), per l. */
  readonly beamZero: Float64Array;
}

function gsfBeamAtUsers(
  users: GsfTable,
  beamZero: Float64Array,
  m: number,
  lmax: number,
): UserBeam {
  const mirrored = Float64Array.from({ length: lmax + 1 }, (_, l) =>
    (l + m) % 2 === 0 ? at(beamZero, l) : -at(beamZero, l),
  );
  return { users, beamZero: mirrored };
}

/**
 * Adds the beam's own scattering at the user cosines, per unit transmission:
 * (2 − δ_m0)(ω ÷ 4π) Σ_l (2l + 1) P_l(±μ_u) d^l_m0(−μ₀)(a₁, b₁, 0, 0).
 */
function addBeamAtUsers(
  lm: VectorLayerMode,
  userBeam: UserBeam,
  users: number,
  sourceUp: Float64Array,
  sourceDown: Float64Array,
): void {
  const { components, mode: m } = lm;
  const strength = (lm.layer.omega * (m === 0 ? 1 : 2)) / (4 * Math.PI);
  const lmax = userBeam.beamZero.length - 1;
  const table = userBeam.users;
  for (let l = m; l <= lmax; l += 1) {
    const sign = (l + m) % 2 === 0 ? 1 : -1;
    const d0 = at(userBeam.beamZero, l) * strength * (2 * l + 1);
    const vi = d0 * at(lm.layer.a1, l);
    const vq = d0 * at(lm.layer.b1, l);
    if (vi === 0 && vq === 0) {
      continue;
    }
    for (let u = 0; u < users; u += 1) {
      const p = at(table.zero, l * table.count + u);
      const r = at(table.even, l * table.count + u);
      const t = at(table.odd, l * table.count + u);
      const base = u * components;
      sourceUp[base] = at(sourceUp, base) + p * vi;
      sourceUp[base + 1] = at(sourceUp, base + 1) + r * vq;
      sourceDown[base] = at(sourceDown, base) + sign * p * vi;
      sourceDown[base + 1] = at(sourceDown, base + 1) + sign * r * vq;
      if (components > 2) {
        sourceUp[base + 2] = at(sourceUp, base + 2) - t * vq;
        sourceDown[base + 2] = at(sourceDown, base + 2) + sign * t * vq;
      }
    }
  }
}

function fluxesOf(
  zeroth: StokesModeIntensities,
  beam: PreparedBeam,
  quadrature: DoubleGaussQuadrature,
  count: number,
): InterfaceFluxes {
  const layerCount = beam.topTransmission.length;
  const { components } = zeroth;
  const direct = new Float64Array(layerCount + 1);
  const diffuseDown = new Float64Array(layerCount + 1);
  const diffuseUp = new Float64Array(layerCount + 1);
  const meanIntensity = new Float64Array(layerCount + 1);
  for (let k = 0; k <= layerCount; k += 1) {
    const transmission = k < layerCount ? at(beam.topTransmission, k) : beam.groundTransmission;
    let down = 0;
    let up = 0;
    let mean = 0;
    for (let i = 0; i < count; i += 1) {
      const w = at(quadrature.weights, i);
      const mu = at(quadrature.nodes, i);
      const iUp = at(zeroth.quadratureUp, (k * count + i) * components);
      const iDown = at(zeroth.quadratureDown, (k * count + i) * components);
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
// The correction's bake.

/**
 * The streams the correction is solved at, NSTR 8, 4 a hemisphere: the vector ruling's 8-stream
 * difference (decision-r08-vector item 2, measured at 4 Gauss nodes a hemisphere;
 * decision-r08-t14a-scope ruling 3).
 */
export const POLARISATION_STREAMS = 8;

/** The modes the correction holds: m = 0, 1 and 2 (the ruling's m ≤ 2 leaves under 0.004%). */
export const POLARISATION_MODES = 3;

/**
 * The highest moment a preparation keeps by default, l = 16: delta-M at 16 streams reads a₁_16,
 * so that the tests' 16-stream solves read the same preparation.
 */
export const POLARISATION_MOMENT_ORDER = 16;

/**
 * Which matrices a preparation gives its terms: `drawn`, each term's own (Rayleigh's and Thomson's,
 * a tabulated term's where its table has one, else a total depolariser); `depolarising`, every term
 * a total depolariser, Rayleigh included, the tests' check that the correction then vanishes.
 */
export type PolarisationMatrices = "drawn" | "depolarising";

/** A correction's preparation options. */
export interface PolarisationOptions {
  /** The bake's grid; `bake.ts`'s `THICK_BAKE_GRID` by default. */
  readonly grid?: ThickBakeGrid;
  /** The exponential sums' options; `expSum.ts`'s by default. */
  readonly expSum?: ExpSumOptions;
  /**
   * The wavelengths solved at, nm, each reading its gases through the bake bin that holds it:
   * `medium.ts`'s `CHANNEL_WAVELENGTHS_NM` by default, the only set {@link
   * bakePolarisationCorrection} stores.
   */
  readonly wavelengthsNm?: ReadonlyArray<number>;
  /** The highest moment kept, even and ≥ 2; {@link POLARISATION_MOMENT_ORDER} by default. */
  readonly momentOrder?: number;
  /** {@link PolarisationMatrices}; `drawn` by default. */
  readonly matrices?: PolarisationMatrices;
}

/** A term's optics at one wavelength, at relative density 1, with its matrix's moments. */
export interface PolarisedTermOptics {
  readonly scatteringPerM: number;
  readonly absorptionPerM: number;
  /** Its phase function at the wavelength (`spectralColumn.ts`'s), for its exact single scattering. */
  readonly phase: SpectralPhase;
  /** l = 0 … the preparation's moment order. */
  readonly moments: MatrixMoments;
  /** f_c, the forward delta's share of its scattering. */
  readonly forwardDelta: number;
  /** Whether it carries a matrix: not a total depolariser. */
  readonly polarising: boolean;
}

/** One wavelength a correction is solved at. */
export interface PolarisationPoint {
  readonly wavelengthNm: number;
  /** The bake bin that holds it (`binConversion.ts`'s `binOfRow`), whose gases' sums it reads. */
  readonly bin: number;
  /** The ground's Lambertian albedo there (`bake.ts`'s `binGroundAlbedo`). */
  readonly groundAlbedo: number;
  /** Per column term, in its order. */
  readonly terms: ReadonlyArray<PolarisedTermOptics>;
}

/** Everything of a correction that does not depend on the band. */
export interface PreparedPolarisation {
  readonly column: SpectralColumn;
  readonly grid: ThickBakeGrid;
  /** The thick bake's layers on the same grid (`bake.ts`'s `bakeLayering`). */
  readonly layering: BakeLayering;
  readonly momentOrder: number;
  readonly matrices: PolarisationMatrices;
  readonly points: ReadonlyArray<PolarisationPoint>;
  /** Per light, per point: its gases' terms in the point's bin (`bake.ts`'s `bakeLightBins`). */
  readonly lights: ReadonlyArray<ReadonlyArray<BakeLightBin>>;
}

/** A term's optics with its matrix's moments (the module's remarks). */
function polarisedTermOptics(
  optics: SpectralTermOptics,
  order: number,
  matrices: PolarisationMatrices,
): PolarisedTermOptics {
  const { phase } = optics;
  const a1 = phaseMoments(phase, order);
  let moments: MatrixMoments;
  let polarising: boolean;
  switch (phase.kind) {
    case "rayleigh":
      polarising = matrices === "drawn";
      moments = polarising
        ? rayleighMatrixMoments(order, phase.depolarisation, a1)
        : depolarisingMoments(order, a1);
      break;
    case "tabulated": {
      const matrix = phase.table.matrix;
      polarising = matrices === "drawn" && matrix !== undefined;
      moments =
        matrix === undefined || !polarising
          ? depolarisingMoments(order, a1)
          : tableMatrixMoments(
              order,
              phase.table.u,
              {
                a2: matrix.a2[0],
                a3: matrix.a3[0],
                a4: matrix.a4[0],
                b1: matrix.b1[0],
                b2: matrix.b2[0],
              },
              a1,
            );
      break;
    }
    case "none":
      polarising = false;
      moments = depolarisingMoments(order, a1);
      break;
    case "moments":
      // A stated approximation (R08.T14.c): a channel source's Cornette–Shanks phase, and its
      // blend between two channels, carry no matrix, so the term is a total depolariser.
      polarising = false;
      moments = depolarisingMoments(order, a1);
      break;
  }
  return {
    scatteringPerM: optics.scatteringPerM,
    absorptionPerM: optics.absorptionPerM,
    phase,
    moments,
    forwardDelta: forwardDeltaShare(phase),
    polarising,
  };
}

/**
 * Everything of a correction that does not depend on the band (the module's remarks): the column's
 * optics with each term's matrix moments at each wavelength (an aerosol's Mie, MMF or phase-file
 * optics once a wavelength, and at 570 nm for the layers), the thick bake's layers on the same
 * grid, and each light's exponential sums in the bins that hold the wavelengths.
 *
 * @remarks
 * Its layers are the thick bake's to the bit (`bake.ts`'s `bakeLayering` on the reference bin's
 * optics), so that a thick world's two tables share their altitudes. It computes the aerosols'
 * optics at the reference bin and the channels only, not in the 15 bins the thick bake's
 * `prepareBake` takes, and fits the sums only in the channels' three bins.
 *
 * @throws RangeError as `bake.ts`'s `prepareBake` refuses its grid or lights, for a moment order
 *   that is not even and at least 2, or as the column's optics refuse.
 */
export function preparePolarisation(
  column: SpectralColumn,
  lights: ReadonlyArray<ThickBakeLight>,
  options: PolarisationOptions = {},
): PreparedPolarisation {
  const grid = options.grid ?? THICK_BAKE_GRID;
  checkBakeInputs(column, lights, grid);
  const order = options.momentOrder ?? POLARISATION_MOMENT_ORDER;
  if (!(Number.isInteger(order) && order >= 2 && order % 2 === 0)) {
    throw new RangeError(`a correction keeps an even moment order ≥ 2, not ${order}`);
  }
  const matrices = options.matrices ?? "drawn";
  const wavelengths = options.wavelengthsNm ?? CHANNEL_WAVELENGTHS_NM;
  const reference = BAKE_WAVELENGTHS_NM[THICK_BAKE_REFERENCE_BIN] ?? Number.NaN;
  const [referenceOptics, ...pointOptics] = spectralOptics(column, [reference, ...wavelengths]);
  const layering = bakeLayering(column, referenceOptics ?? [], grid);
  const points = wavelengths.map((nm, i): PolarisationPoint => ({
    wavelengthNm: nm,
    bin: binOfRow(nm),
    groundAlbedo: binGroundAlbedo(column.groundAlbedo, nm),
    terms: (pointOptics[i] ?? []).map((t) => polarisedTermOptics(t, order, matrices)),
  }));
  const bins = [...new Set(points.map((point) => point.bin))];
  const perLight = bakeLightBins(column, layering, lights, bins, options.expSum ?? EXP_SUM_OPTIONS);
  return {
    column,
    grid,
    layering,
    momentOrder: order,
    matrices,
    points,
    lights: perLight.map((lightBins) =>
      points.map((point) => {
        const bin = lightBins[bins.indexOf(point.bin)];
        if (bin === undefined) {
          throw new Error(`no bin ${point.bin} prepared`);
        }
        return bin;
      }),
    ),
  };
}

/**
 * Whether a channel source scatters by a matrix whose b₁ is not 0, read by its phase: Rayleigh's
 * does wherever it scatters, as a molecular term's; a table's where it scatters in green and its
 * matrix's b₁ there is not 0, as an aerosol's is read; Cornette–Shanks's and none's never (the
 * correction takes them as depolarisers).
 */
function channelSourcePolarises(source: Extract<SpectralSource, { kind: "channels" }>): boolean {
  const { phase } = source;
  let polarises: boolean;
  switch (phase.kind) {
    case "rayleigh":
      polarises = source.scattering.some((s) => s > 0);
      break;
    case "tabulated":
      polarises =
        source.scattering[1] > 0 && phase.table.matrix?.b1[1].some((value) => value !== 0) === true;
      break;
    case "cornette-shanks":
    case "none":
      polarises = false;
      break;
  }
  return polarises;
}

/**
 * Whether a column needs the correction: whether some term scatters by a matrix whose b₁ is not 0
 * (Design note 10: "every world with a term whose b₁ ≠ 0, thin or thick"). A molecular or Thomson
 * term does, by Rayleigh's matrix (b₁ = −¾Δ sin²Θ, Δ > 0 for every ρ < 1), so every world with air;
 * an aerosol does where its optics at the green channel carry a matrix with a b₁ not 0; a channel
 * source by its phase, Rayleigh's as a molecular term's, a table's by its green channel's b₁ as an
 * aerosol's, and Cornette–Shanks's and none's never.
 *
 * @remarks
 * Calling it is R08.T14.f's. Only a column with no molecular or Thomson term takes optics here: its
 * aerosols' Mie, MMF or phase-file optics at one wavelength, which can take a second or so.
 *
 * @throws RangeError as the column's optics refuse (`spectralColumn.ts`'s `spectralOptics`), for a
 *   column with no molecular or Thomson term.
 */
export function needsPolarisationCorrection(column: SpectralColumn): boolean {
  const aerosols: SpectralTerm[] = [];
  for (const term of column.terms) {
    const { source } = term;
    switch (source.kind) {
      case "molecular":
        if (source.numberDensityPerM3 > 0) {
          return true;
        }
        break;
      case "thomson":
        if (source.electronDensityPerM3 > 0) {
          return true;
        }
        break;
      case "aerosol":
        if (source.extinction550PerM > 0) {
          aerosols.push(term);
        }
        break;
      case "absorber":
        break;
      case "channels":
        if (channelSourcePolarises(source)) {
          return true;
        }
        break;
    }
  }
  if (aerosols.length === 0) {
    return false;
  }
  const [optics] = spectralOptics({ ...column, terms: aerosols }, [CHANNEL_WAVELENGTHS_NM[1]]);
  return (optics ?? []).some(
    (t) =>
      t.scatteringPerM > 0 &&
      t.phase.kind === "tabulated" &&
      t.phase.table.matrix?.b1[0].some((value) => value !== 0) === true,
  );
}

/** A correction's solve options: the streams and the highest mode. */
export interface PolarisationSolveOptions {
  /** NSTR; {@link POLARISATION_STREAMS} by default. At most the preparation's moment order. */
  readonly streams?: number;
  /** M, the modes 0 … M; {@link POLARISATION_MODES} − 1 by default. */
  readonly maxMode?: number;
}

/**
 * One light's sources at one wavelength of a band, summed over its gases' terms by their weights,
 * each `[altitude][μ₀][μ_v][m]` row-major on the bake's grid ({@link PolarisationTable}'s layout
 * without the channel).
 */
export interface PolarisationSources {
  /** ΔJ = J_vector − J_scalar, the I component: `vector − scalar`, sr⁻¹ (the module's remarks). */
  readonly correction: Float64Array;
  /**
   * The scalar solve's J of orders two and up per unit of σ_s at h\*: `bake.ts`'s E ÷ σ_s at these
   * streams, the aureole's term included, sr⁻¹ per unit of the star's irradiance normal to its
   * beam.
   */
  readonly scalar: Float64Array;
  /** The vector solve's I of the same. */
  readonly vector: Float64Array;
  /** σ_s at each altitude, m⁻¹ of true length (the reference medium's at h\*). */
  readonly scatteringPerM: Float64Array;
  /** The kernels' beam, extinction less the forward delta, at each altitude and μ₀, `[a][s]`. */
  readonly kernelTransmission: Float64Array;
  /** The scalar I^m upward at the top, `[s][u][m]` at the grid's upward cosines. */
  readonly topScalar: Float64Array;
  /** The vector solve's I^m upward at the top. */
  readonly topVector: Float64Array;
  /** The scalar I^m downward at the ground, `[s][u][m]`. */
  readonly groundScalar: Float64Array;
  /** The vector solve's I^m downward at the ground. */
  readonly groundVector: Float64Array;
  /** The vector and scalar pairs of solves. */
  readonly solves: number;
  /** The complex pairs the vector eigenproblems met, over the solves, modes and layers. */
  readonly complexPairs: number;
  /** Their smallest and largest |Im k²| ÷ |k²| ({@link ModeEigenRecord}), [∞, 0] with none. */
  readonly imaginaryShares: readonly [number, number];
  /** The layers solved by the nonsymmetric QR, over the solves and modes. */
  readonly nonsymmetricLayers: number;
}

/** A layer's or a point's mixed optics before delta-M. */
interface MixedOptics {
  /**
   * Σ σ_s weights, in the weights' unit: a layer's scattering optical depth where the weights are
   * σ_s times each term's column, a point's σ_s in m⁻¹ where they are σ_s times its density.
   */
  readonly scatteringWeight: number;
  readonly forwardDelta: number;
  /** Per element, l = 0 … NSTR, σ_s-weighted. */
  readonly moments: {
    readonly a1: Float64Array;
    readonly a2: Float64Array;
    readonly a3: Float64Array;
    readonly a4: Float64Array;
    readonly b1: Float64Array;
    readonly b2: Float64Array;
  };
}

/**
 * Mixes the terms' optics by their scattering `weights` (σ_s τ or σ_s ρ): each element's moments
 * σ_s-weighted to l = NSTR, as the thick bake mixes χ_l.
 */
function mixOptics(
  terms: ReadonlyArray<PolarisedTermOptics>,
  weights: Float64Array,
  streams: number,
): MixedOptics {
  const moments = zeroMoments(streams);
  let scattering = 0;
  let delta = 0;
  for (const [t, o] of terms.entries()) {
    const s = at(weights, t);
    if (!(s > 0)) {
      continue;
    }
    scattering += s;
    delta += s * o.forwardDelta;
    for (const element of ELEMENTS) {
      const target = moments[element];
      const source = o.moments[element];
      for (let l = 0; l <= streams; l += 1) {
        target[l] = at(target, l) + s * (source[l] ?? 0);
      }
    }
  }
  for (const element of ELEMENTS) {
    const target = moments[element];
    for (let l = 0; l <= streams; l += 1) {
      target[l] = scattering > 0 ? at(target, l) / scattering : element === "a1" && l === 0 ? 1 : 0;
    }
  }
  return {
    scatteringWeight: scattering,
    forwardDelta: scattering > 0 ? delta / scattering : 0,
    moments,
  };
}

/**
 * The delta-M truncation of a mixed matrix's moments, the bake's (the module's remarks): the
 * removed peak is the identity, so a₂, a₃ and a₄ lose f, x′ = (x − f) ÷ (1 − f), and b₁ and b₂,
 * which the identity has none of, are b ÷ (1 − f); l < NSTR. Every element keeps its moments
 * there, (1 − f) x′ + f = x and (1 − f) b′ = b, whether its terms carry a matrix or not.
 *
 * @param moments - The mixed moments, l = 0 … NSTR − 1 at least, a₁_0 = 1.
 * @param f - The share removed, max(a₁_NSTR, f_c, 0) as the thick bake's `deltaMLayer`, in [0, 1).
 * @param a1 - a₁′, l = 0 … NSTR − 1, as `deltaMLayer` gives it: (a₁_l − f) ÷ (1 − f).
 * @param streams - NSTR, even and at least 2.
 */
export function truncateMatrixMoments(
  moments: MatrixMoments,
  f: number,
  a1: ArrayLike<number>,
  streams: number,
): MatrixMoments {
  return truncatedMoments(moments, f, a1, streams);
}

/** {@link truncateMatrixMoments}, in the arrays the sources read. */
function truncatedMoments(
  moments: MatrixMoments,
  f: number,
  a1: ArrayLike<number>,
  streams: number,
): MomentArrays {
  const out = zeroMoments(streams - 1);
  const kept = 1 - f;
  for (let l = 0; l < streams; l += 1) {
    out.a1[l] = a1[l] ?? Number.NaN;
    for (const element of ["a2", "a3", "a4"] as const) {
      const value = moments[element][l] ?? 0;
      out[element][l] = element === "a4" || l >= 2 ? (value - f) / kept : 0;
    }
    out.b1[l] = l < 2 ? 0 : (moments.b1[l] ?? 0) / kept;
    out.b2[l] = l < 2 ? 0 : (moments.b2[l] ?? 0) / kept;
  }
  return out;
}

/** One term's pair of solves at one point of a band. */
interface PointSolve {
  readonly scalar: DiscreteOrdinatesSolution;
  readonly vector: PolarisedSolution;
  readonly scaledBeams: ReadonlyArray<SphericalBeam>;
  readonly kernelBeams: ReadonlyArray<SphericalBeam>;
}

function solvePoint(
  prepared: PreparedPolarisation,
  geometry: ThickBakeBand,
  point: PolarisationPoint,
  term: BinTerm,
  streams: number,
  maxMode: number,
): PointSolve {
  const { grid, layering } = prepared;
  const layers = grid.layers;
  const factor = geometry.coefficientFactor;
  const scalarLayers: OrdinateLayer[] = [];
  const vectorLayers: PolarisedLayer[] = [];
  const scaledDepths = new Float64Array(layers);
  const kernelDepths = new Float64Array(layers);
  const weights = new Float64Array(point.terms.length);
  for (let p = 0; p < layers; p += 1) {
    let tau = 0;
    for (const [t, o] of point.terms.entries()) {
      const c = (layering.layerColumnsM[t]?.[p] ?? Number.NaN) * factor;
      tau += (o.scatteringPerM + o.absorptionPerM) * c;
      weights[t] = o.scatteringPerM * c;
    }
    const mixed = mixOptics(point.terms, weights, streams);
    tau += factor * at(term.absorptionDepth, p);
    const { layer, truncated } = deltaMLayer(
      tau,
      mixed.scatteringWeight,
      mixed.moments.a1,
      mixed.forwardDelta,
      streams,
    );
    scalarLayers.push(layer);
    vectorLayers.push({
      opticalDepth: layer.opticalDepth,
      singleScatteringAlbedo: layer.singleScatteringAlbedo,
      moments: truncatedMoments(mixed.moments, truncated, layer.moments, streams),
    });
    scaledDepths[p] = layer.opticalDepth;
    kernelDepths[p] = Math.max(tau - mixed.scatteringWeight * mixed.forwardDelta, 0);
  }
  const radius = geometry.bottomRadiusM;
  const heights = layering.interfaceHeightsM;
  const scaledBeams = Array.from(grid.sunCosines, (mu0) =>
    sphericalBeamPath(radius, heights, scaledDepths, mu0),
  );
  const kernelBeams = Array.from(grid.sunCosines, (mu0) =>
    sphericalBeamPath(radius, heights, kernelDepths, mu0),
  );
  const common = {
    groundAlbedo: point.groundAlbedo,
    beamCosines: grid.sunCosines,
    userCosines: grid.upwardCosines,
    maxMode,
    streams,
    beamPaths: scaledBeams,
  };
  return {
    scalar: solveDiscreteOrdinates({ ...common, layers: scalarLayers }),
    vector: solvePolarised({ ...common, layers: vectorLayers }),
    scaledBeams,
    kernelBeams,
  };
}

/** Λ_l^m at each cosine, `[l × count + i]`. */
function legendreRows(m: number, lmax: number, cosines: ArrayLike<number>): Float64Array {
  const count = cosines.length;
  const out = new Float64Array((lmax + 1) * count);
  for (let i = 0; i < count; i += 1) {
    const values = normalizedAssociatedLegendre(m, lmax, cosines[i] ?? Number.NaN);
    for (let l = 0; l <= lmax; l += 1) {
      out[l * count + i] = at(values, l);
    }
  }
  return out;
}

/** The functions a mode's sources take: Λ and the d-functions at the nodes, views and suns. */
interface SourceFunctions {
  readonly nodeLegendre: Float64Array;
  readonly viewLegendre: Float64Array;
  readonly sunLegendre: Float64Array;
  readonly nodes: GsfTable;
  readonly views: GsfTable;
}

/**
 * Adds one term's sources, weighted, at every table point (the module's remarks): per mode, at each
 * altitude's interface, J = (f − f_c) I_d(Ω) + (1 − f)(the smooth part's scattering of the diffuse
 * field + the aureole's), per unit σ_s, for the scalar field and the vector field's I.
 */
function addPointSources(
  prepared: PreparedPolarisation,
  point: PolarisationPoint,
  solve: PointSolve,
  weight: number,
  streams: number,
  maxMode: number,
  functions: ReadonlyArray<SourceFunctions>,
  out: {
    readonly scalar: Float64Array;
    readonly vector: Float64Array;
    readonly kernelTransmission: Float64Array;
    readonly topScalar: Float64Array;
    readonly topVector: Float64Array;
    readonly groundScalar: Float64Array;
    readonly groundVector: Float64Array;
  },
): void {
  const { grid, layering, column } = prepared;
  const layers = grid.layers;
  const altitudes = layering.altitudesM.length;
  const suns = grid.sunCosines.length;
  const views = grid.upwardCosines.length;
  const v2 = 2 * views;
  const modes = maxMode + 1;
  const { nodes, weights: w } = solve.scalar.quadrature;
  const count = nodes.length;
  const lmax = streams - 1;
  const densities = new Float64Array(point.terms.length);
  for (let a = 0; a < altitudes; a += 1) {
    const h = at(layering.altitudesM, a);
    for (const [t, term] of column.terms.entries()) {
      densities[t] = (point.terms[t]?.scatteringPerM ?? 0) * densityAt(term.density, h);
    }
    const mixed = mixOptics(point.terms, densities, streams);
    const k = layers - a * grid.altitudeStride;
    for (let s = 0; s < suns; s += 1) {
      out.kernelTransmission[a * suns + s] =
        at(out.kernelTransmission, a * suns + s) +
        weight * at(solve.kernelBeams[s]?.interfaceTransmission ?? NO_VALUES, k);
    }
    if (!(mixed.scatteringWeight > 0)) {
      continue;
    }
    const f = Math.max(at(mixed.moments.a1, streams), mixed.forwardDelta, 0);
    const lobe = f - mixed.forwardDelta;
    const kept = 1 - f;
    const a1 = Float64Array.from({ length: streams }, (_, l) =>
      l === 0 ? 1 : (at(mixed.moments.a1, l) - f) / kept,
    );
    const truncated = truncatedMoments(mixed.moments, f, a1, streams);
    for (let s = 0; s < suns; s += 1) {
      const scalarBeam = solve.scalar.beams[s];
      const vectorBeam = solve.vector.beams[s];
      if (scalarBeam === undefined || vectorBeam === undefined) {
        throw new Error(`no beam ${s}`);
      }
      const aureole = Math.max(
        at(solve.scaledBeams[s]?.interfaceTransmission ?? NO_VALUES, k) -
          at(solve.kernelBeams[s]?.interfaceTransmission ?? NO_VALUES, k),
        0,
      );
      for (let m = 0; m < modes; m += 1) {
        const fn = functions[m];
        const scalarMode = scalarBeam.modes[m];
        const vectorMode = vectorBeam.modes[m];
        if (fn === undefined || scalarMode === undefined || vectorMode === undefined) {
          throw new Error(`no mode ${m}`);
        }
        const c = vectorMode.components;
        // The half-range moments of the diffuse field: scalar H_l, and the vector's I and Q rows.
        const hScalar = new Float64Array(streams);
        const hI = new Float64Array(streams);
        const hQ = new Float64Array(streams);
        for (let l = m; l <= lmax; l += 1) {
          const sign = (l + m) % 2 === 0 ? 1 : -1;
          let sumScalar = 0;
          let sumI = 0;
          let sumQ = 0;
          for (let j = 0; j < count; j += 1) {
            const wj = at(w, j);
            sumScalar +=
              wj *
              at(fn.nodeLegendre, l * count + j) *
              (at(scalarMode.quadratureUp, k * count + j) +
                sign * at(scalarMode.quadratureDown, k * count + j));
            const base = (k * count + j) * c;
            const p = at(fn.nodes.zero, l * count + j);
            const r = at(fn.nodes.even, l * count + j);
            const t = at(fn.nodes.odd, l * count + j);
            const up = vectorMode.quadratureUp;
            const down = vectorMode.quadratureDown;
            sumI += wj * p * (at(up, base) + sign * at(down, base));
            let q = r * (at(up, base + 1) + sign * at(down, base + 1));
            if (c > 2) {
              q += t * (sign * at(down, base + 2) - at(up, base + 2));
            }
            sumQ += wj * q;
          }
          hScalar[l] = sumScalar;
          hI[l] = sumI;
          hQ[l] = sumQ;
        }
        const azimuthal = (m === 0 ? 1 : 2) / (4 * Math.PI);
        for (let u = 0; u < views; u += 1) {
          let even = 0;
          let odd = 0;
          let vectorEven = 0;
          let vectorOdd = 0;
          let beamEven = 0;
          let beamOdd = 0;
          for (let l = m; l <= lmax; l += 1) {
            const lambdaV = at(fn.viewLegendre, l * views + u);
            const scale = (2 * l + 1) * at(truncated.a1, l);
            const diffuse = scale * lambdaV * at(hScalar, l);
            const scatteredBeam = scale * lambdaV * at(fn.sunLegendre, l * suns + s);
            const vectorTerm =
              (2 * l + 1) *
              at(fn.views.zero, l * views + u) *
              (at(truncated.a1, l) * at(hI, l) + at(truncated.b1, l) * at(hQ, l));
            if ((l + m) % 2 === 0) {
              even += diffuse;
              beamEven += scatteredBeam;
              vectorEven += vectorTerm;
            } else {
              odd += diffuse;
              beamOdd += scatteredBeam;
              vectorOdd += vectorTerm;
            }
          }
          const aureoleUp = azimuthal * (beamEven - beamOdd) * aureole;
          const aureoleDown = azimuthal * (beamEven + beamOdd) * aureole;
          const scalarUp = at(scalarMode.userUp, k * views + u);
          const scalarDown = at(scalarMode.userDown, k * views + u);
          const vectorUp = at(vectorMode.userUp, (k * views + u) * c);
          const vectorDown = at(vectorMode.userDown, (k * views + u) * c);
          const cell = (a * suns + s) * v2;
          const upIndex = (cell + views + u) * modes + m;
          const downIndex = (cell + views - 1 - u) * modes + m;
          const add = (target: Float64Array, index: number, value: number): void => {
            target[index] = at(target, index) + weight * value;
          };
          add(out.scalar, upIndex, lobe * scalarUp + kept * (0.5 * (even + odd) + aureoleUp));
          add(out.scalar, downIndex, lobe * scalarDown + kept * (0.5 * (even - odd) + aureoleDown));
          add(
            out.vector,
            upIndex,
            lobe * vectorUp + kept * (0.5 * (vectorEven + vectorOdd) + aureoleUp),
          );
          add(
            out.vector,
            downIndex,
            lobe * vectorDown + kept * (0.5 * (vectorEven - vectorOdd) + aureoleDown),
          );
        }
      }
    }
  }
  // The boundary radiances, I^m at the top upward and at the ground downward.
  for (let s = 0; s < suns; s += 1) {
    for (let m = 0; m < modes; m += 1) {
      const scalarMode = solve.scalar.beams[s]?.modes[m];
      const vectorMode = solve.vector.beams[s]?.modes[m];
      if (scalarMode === undefined || vectorMode === undefined) {
        throw new Error(`no mode ${m}`);
      }
      const c = vectorMode.components;
      for (let u = 0; u < views; u += 1) {
        const index = (s * views + u) * modes + m;
        out.topScalar[index] = at(out.topScalar, index) + weight * at(scalarMode.userUp, u);
        out.topVector[index] = at(out.topVector, index) + weight * at(vectorMode.userUp, u * c);
        out.groundScalar[index] =
          at(out.groundScalar, index) + weight * at(scalarMode.userDown, layers * views + u);
        out.groundVector[index] =
          at(out.groundVector, index) + weight * at(vectorMode.userDown, (layers * views + u) * c);
      }
    }
  }
}

/**
 * Each light's sources at one wavelength of a band (the module's remarks): for every term of its
 * gases in the wavelength's bin, the vector and the scalar solve on the same delta-M layers and
 * pseudo-spherical beams, their sources at every table point summed by the terms' weights.
 *
 * @remarks
 * A light's terms that another light shares (a column with no gases) are solved once. The table's
 * points where the medium scatters nothing hold 0.
 *
 * @param band - The band's index in `bake.ts`'s `thickBakeBands` order.
 * @param point - The wavelength's index in the preparation's points.
 * @throws RangeError for a band the body does not have, a point the preparation does not have, more
 *   streams than its moment order or a mode past NSTR − 1, or as the solvers refuse a layer.
 */
export function polarisationSources(
  prepared: PreparedPolarisation,
  body: ThickBakeBody,
  band: number,
  point: number,
  options: PolarisationSolveOptions = {},
): ReadonlyArray<PolarisationSources> {
  const bands = thickBakeBands(body);
  const geometry = bands[band];
  if (geometry === undefined || !Number.isInteger(band)) {
    throw new RangeError(`the body has ${bands.length} latitude bands, not a band ${band}`);
  }
  const at0 = prepared.points[point];
  if (at0 === undefined || !Number.isInteger(point)) {
    throw new RangeError(`the correction has ${prepared.points.length} wavelengths, not ${point}`);
  }
  const streams = options.streams ?? POLARISATION_STREAMS;
  const maxMode = options.maxMode ?? POLARISATION_MODES - 1;
  requireStreams(streams);
  requireMode(maxMode, streams);
  if (streams > prepared.momentOrder) {
    throw new RangeError(
      `${streams} streams read moments to ${streams}, past the preparation's ${prepared.momentOrder}`,
    );
  }
  const { grid } = prepared;
  const altitudes = prepared.layering.altitudesM.length;
  const suns = grid.sunCosines.length;
  const views = grid.upwardCosines.length;
  const modes = maxMode + 1;
  const quadrature = doubleGaussQuadrature(streams);
  const lmax = streams - 1;
  const functions = Array.from({ length: modes }, (_, m): SourceFunctions => ({
    nodeLegendre: legendreRows(m, lmax, quadrature.nodes),
    viewLegendre: legendreRows(m, lmax, grid.upwardCosines),
    sunLegendre: legendreRows(m, lmax, grid.sunCosines),
    nodes: gsfTable(m, lmax, quadrature.nodes),
    views: gsfTable(m, lmax, grid.upwardCosines),
  }));
  const scatteringPerM = Float64Array.from(prepared.layering.altitudesM, (h) => {
    let sum = 0;
    for (const [t, term] of prepared.column.terms.entries()) {
      sum += (at0.terms[t]?.scatteringPerM ?? 0) * densityAt(term.density, h);
    }
    return sum;
  });
  const solved = new Map<BinTerm, PointSolve>();
  return prepared.lights.map((lightBins): PolarisationSources => {
    const bin = lightBins[point];
    if (bin === undefined) {
      throw new Error(`no bin for wavelength ${point}`);
    }
    const size = altitudes * suns * 2 * views * modes;
    const edge = suns * views * modes;
    const out = {
      scalar: new Float64Array(size),
      vector: new Float64Array(size),
      kernelTransmission: new Float64Array(altitudes * suns),
      topScalar: new Float64Array(edge),
      topVector: new Float64Array(edge),
      groundScalar: new Float64Array(edge),
      groundVector: new Float64Array(edge),
    };
    let complexPairs = 0;
    let nonsymmetricLayers = 0;
    let smallestShare = Number.POSITIVE_INFINITY;
    let largestShare = 0;
    for (const term of bin.terms) {
      let solve = solved.get(term);
      if (solve === undefined) {
        solve = solvePoint(prepared, geometry, at0, term, streams, maxMode);
        solved.set(term, solve);
      }
      for (const record of solve.vector.eigen) {
        complexPairs += record.complexPairs;
        nonsymmetricLayers += record.nonsymmetricLayers;
        smallestShare = Math.min(smallestShare, record.imaginaryShares[0]);
        largestShare = Math.max(largestShare, record.imaginaryShares[1]);
      }
      addPointSources(prepared, at0, solve, term.weight, streams, maxMode, functions, out);
    }
    return {
      ...out,
      correction: out.vector.map((value, i) => value - at(out.scalar, i)),
      scatteringPerM,
      solves: bin.terms.length,
      complexPairs,
      imaginaryShares: [smallestShare, largestShare],
      nonsymmetricLayers,
    };
  });
}

/** One light slot's correction. */
export interface PolarisationSlot {
  /**
   * ΔJ_pol per channel, sr⁻¹ per unit of the star's irradiance normal to its beam at the top, per
   * unit of the column's whole σ_s at h\*, its forward deltas included ({@link PolarisationTable}),
   * A × S × V × M × 3 row-major, entry
   * `(((a S + s) V + v) M + m) 3 + c`, as `bake.ts`'s `MsSourceSlot` with M = 3.
   */
  readonly values: Float32Array;
  /** The vector and scalar pairs of solves, over the channels. */
  readonly solves: number;
  /** The complex pairs the vector eigenproblems met, over the solves, modes and layers. */
  readonly complexPairs: number;
  /** Their smallest and largest |Im k²| ÷ |k²| ({@link ModeEigenRecord}), [∞, 0] with none. */
  readonly imaginaryShares: readonly [number, number];
  /** The layers solved by the nonsymmetric QR iteration, over the solves and modes. */
  readonly nonsymmetricLayers: number;
}

/**
 * ΔJ_pol(h, μ₀, μ_v, m) of one latitude band (Provides' `PolarisationTable`), one table a light
 * slot, on the thick bake's grid: what R08.T14.f adds, as σ_s Σ_m ΔJ_m cos mΔφ per source, to the
 * kernels' multiple-scattering term in both regimes.
 *
 * @remarks
 * **The unit and frame.** ΔJ is per unit of the column's whole σ_s at h\*, every scattering term's
 * forward delta included (`PolarisationSources.scatteringPerM`, the reference medium's per unit of
 * true length), per steradian, per unit of the star's irradiance normal to its beam at the top:
 * σ_s ΔJ = E_vector − E_scalar, `bake.ts`'s E (m⁻¹ sr⁻¹), in the kernels' frame of that E (they
 * march with σ_ext − f_c σ_s and add their own exact single scattering, which is the same for both
 * and cancels). A kernel whose σ_s leaves the forward delta out, σ_s,k = Σ (1 − f_c) σ_s
 * (`tables.ts`'s `withoutForwardDelta`), multiplies ΔJ by σ_s,k ÷ (1 − f̄_c), f̄_c the σ_s-weighted
 * delta: the whole σ_s again. Δφ is the azimuth looked toward less the sun's, cos mΔφ even, so its
 * sense does not matter here. Stored per channel, from a solve at the channel's own wavelength, and
 * multiplied by the star's colour as every table is.
 *
 * **The beam.** μ₀ ∈ (0, 1], where the pseudo-spherical beam is defined at every height: no entry
 * is set to 0 for want of a beam, and μ₀ ≤ 0 lies outside the table (R08.T14.c and T14.d judge it).
 *
 * **Size.** 33 × 32 × 16 × 3 × 3 values a band and slot, 608,256 B as `f32` (R08.T14.f's packing,
 * as `rgba16float` RGB, is 304,128 B before any padding to four channels).
 */
export interface PolarisationTable {
  readonly band: ThickBakeBand;
  /** h\* above the ground, m, ascending: the thick bake's altitudes. */
  readonly altitudesM: Float64Array;
  /** μ₀, ascending in (0, 1]. */
  readonly sunCosines: Float64Array;
  /** The travel cosines, positive upward, ascending: the downward half, then the upward. */
  readonly travelCosines: Float64Array;
  /** M: m = 0 … M − 1, {@link POLARISATION_MODES}. */
  readonly modes: number;
  /** One a light of the preparation, in its order. */
  readonly slots: ReadonlyArray<PolarisationSlot>;
}

/**
 * The polarisation correction of one latitude band (Provides' `bakePolarisationCorrection`; the
 * module's remarks): for every light, every render channel and every term of the channel's bin's
 * gases, an 8-stream vector and scalar solve, m = 0 … 2, on the thick bake's layers and
 * pseudo-spherical beam, ΔJ_pol their difference at the table's points.
 *
 * @param prepared - {@link preparePolarisation} at the render channels.
 * @param band - The band's index in `bake.ts`'s `thickBakeBands` order.
 * @throws RangeError for a preparation not at the render channels, a band the body does not have,
 *   or as the solvers refuse a layer.
 */
export function bakePolarisationCorrection(
  prepared: PreparedPolarisation,
  body: ThickBakeBody,
  band: number,
): PolarisationTable {
  const channels = CHANNEL_WAVELENGTHS_NM;
  if (
    prepared.points.length !== channels.length ||
    prepared.points.some((p, c) => p.wavelengthNm !== channels[c])
  ) {
    throw new RangeError("the correction's table is stored at the render channels' wavelengths");
  }
  const perChannel = channels.map((_, c) => polarisationSources(prepared, body, band, c));
  const { grid } = prepared;
  const geometry = thickBakeBands(body)[band];
  if (geometry === undefined) {
    throw new RangeError(`the body has no band ${band}`);
  }
  const slots = prepared.lights.map((_, slot): PolarisationSlot => {
    const sources = perChannel.map((each) => {
      const source = each[slot];
      if (source === undefined) {
        throw new Error(`no light ${slot}`);
      }
      return source;
    });
    const points = sources[0]?.correction.length ?? 0;
    const values = new Float32Array(points * 3);
    for (const [c, source] of sources.entries()) {
      for (let i = 0; i < points; i += 1) {
        values[i * 3 + c] = at(source.correction, i);
      }
    }
    return {
      values,
      solves: sources.reduce((sum, source) => sum + source.solves, 0),
      complexPairs: sources.reduce((sum, source) => sum + source.complexPairs, 0),
      imaginaryShares: [
        Math.min(...sources.map((source) => source.imaginaryShares[0])),
        Math.max(...sources.map((source) => source.imaginaryShares[1])),
      ],
      nonsymmetricLayers: sources.reduce((sum, source) => sum + source.nonsymmetricLayers, 0),
    };
  });
  const upward = grid.upwardCosines;
  const views = 2 * upward.length;
  return {
    band: geometry,
    altitudesM: prepared.layering.altitudesM.slice(),
    sunCosines: grid.sunCosines.slice(),
    travelCosines: Float64Array.from({ length: views }, (_, v) =>
      v < upward.length ? -at(upward, upward.length - 1 - v) : at(upward, v - upward.length),
    ),
    modes: POLARISATION_MODES,
    slots,
  };
}

// ---------------------------------------------------------------------------------------------
// Validation.

function prepareVectorLayer(
  layer: PolarisedLayer,
  p: number,
  streams: number,
): PreparedVectorLayer {
  const name = `layer ${p}`;
  requireIn(`${name}'s opticalDepth`, layer.opticalDepth, (v) => v >= 0, "finite and at least 0");
  requireIn(
    `${name}'s singleScatteringAlbedo`,
    layer.singleScatteringAlbedo,
    (v) => v >= 0 && v <= 1,
    "in [0, 1]",
  );
  const copy = (element: (typeof ELEMENTS)[number]): Float64Array => {
    const source = layer.moments[element];
    const out = new Float64Array(streams);
    for (let l = 0; l < source.length; l += 1) {
      const value = source[l] ?? Number.NaN;
      requireIn(`${name}'s ${element} moment ${l}`, value, () => true, "finite");
      if (l < streams) {
        out[l] =
          (element === "a2" || element === "a3" || element === "b1" || element === "b2") && l < 2
            ? 0
            : value;
      }
    }
    return out;
  };
  const a1 = copy("a1");
  const zeroth = layer.moments.a1[0];
  if (zeroth === undefined || !(Math.abs(zeroth - 1) <= 1e-12)) {
    throw new RangeError(`${name}'s a₁ moment 0 must be 1, not ${zeroth}`);
  }
  a1[0] = 1;
  const b2 = copy("b2");
  const conservative = 1 - layer.singleScatteringAlbedo <= DISCRETE_ORDINATE_CONSERVATIVE_CO_ALBEDO;
  return {
    opticalDepth: layer.opticalDepth,
    omega: conservative ? 1 : layer.singleScatteringAlbedo,
    conservative,
    a1,
    a2: copy("a2"),
    a3: copy("a3"),
    a4: copy("a4"),
    b1: copy("b1"),
    b2,
    circular: layer.singleScatteringAlbedo > 0 && b2.some((v) => v !== 0),
  };
}

function prepareBeams(
  problem: PolarisedProblem,
  layers: ReadonlyArray<PreparedVectorLayer>,
): PreparedBeam[] {
  const cosines = Array.from(problem.beamCosines);
  const paths = problem.beamPaths;
  if (paths !== undefined && paths.length !== cosines.length) {
    throw new RangeError(`${paths.length} beam paths for ${cosines.length} beams`);
  }
  const plain = layers.map((layer) => ({
    opticalDepth: layer.opticalDepth,
    singleScatteringAlbedo: 0,
    moments: [1],
  }));
  return cosines.map((cosine, b) => {
    requireIn("beamCosine", cosine, (v) => v > 0 && v <= 1, "in (0, 1]");
    const path = paths?.[b] ?? planeParallelBeam(plain, cosine);
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
    for (let k = 0; k < n; k += 1) {
      const aik = at(a, i * n + k);
      if (aik === 0) {
        continue;
      }
      for (let j = 0; j < n; j += 1) {
        out[i * n + j] = at(out, i * n + j) + aik * at(b, k * n + j);
      }
    }
  }
  return out;
}

/** An entry that the loops' bounds keep in range; NaN, never `undefined`, should they not. */
function at(a: Float64Array, i: number): number {
  return a[i] ?? Number.NaN;
}
