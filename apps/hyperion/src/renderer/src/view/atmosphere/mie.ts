/**
 * Mie scattering by a homogeneous sphere (plan R08, R08.T5.a; Design note 6): the efficiencies,
 * the asymmetry parameter and the scattering amplitudes of one sphere, from which R08.T5.b builds
 * the optics of the liquid aerosols over their size distributions.
 *
 * @remarks
 * The algorithm is Wiscombe 1980 (W. J. Wiscombe, "Improved Mie scattering algorithms", Appl. Opt.
 * 19, 1505–1509), as his report derives it and his MIEV0 builds it (Wiscombe, "Mie scattering
 * calculations: advances in technique and fast, vector-speed computer codes", NCAR/TN-140+STR,
 * 1979, edited 1996; the equation numbers below are the report's). It runs in double precision,
 * without MIEV0's small-particle formulas or its up-recurrence of Dₙ:
 *
 * - the series is cut at N_stop terms ({@link mieTermCount});
 * - the Riccati–Bessel functions ψₙ(x) and χₙ(x) go by upward recurrence, (17) from (19), which the
 *   report finds accurate to N_stop (its Table 1), since ψₙ only begins to lose digits past n = x;
 * - the logarithmic derivative Dₙ(mx) = ψₙ′(mx) ÷ ψₙ(mx) goes by downward recurrence (23) from its
 *   value at N_stop, which Lentz 1976's continued fraction gives to rounding
 *   ({@link logarithmicDerivatives}). Downward recurrence is stable for every index (G. W.
 *   Kattawar and G. N. Plass, "Electromagnetic scattering from absorbing spheres", Appl. Opt. 6,
 *   1377–1382, 1967); MIEV0 recurs upward where its criterion (47, 48) allows only for speed;
 * - aₙ and bₙ are (16), the efficiencies (6) and (7), the asymmetry (8), and the amplitudes S₁ and
 *   S₂ (9), with the angular functions πₙ and τₙ by upward recurrence (37, 38).
 *
 * Sign convention: a {@link ComplexIndex} is m = n + ik with k ≥ 0 absorbing, as tables of optical
 * constants give it. The amplitudes are in Wiscombe's and van de Hulst's convention (time factor
 * e^(+iωt), the index written n − ik, the report's (3)); Bohren and Huffman's (e^(−iωt), their
 * §4.3) are their complex conjugates. The efficiencies, the asymmetry and |S₁|², |S₂|² do not
 * depend on it.
 */

/**
 * A complex refractive index relative to the medium around the sphere, m = n + ik.
 *
 * @remarks
 * The index of the sphere's material divided by the medium's real index, as tables of optical
 * constants give n and k (k ≥ 0 absorbing; Bohren and Huffman's sign).
 */
export interface ComplexIndex {
  /** The real part, the phase index, finite and > 0. */
  readonly n: number;
  /** The absorption index, the imaginary part, finite and ≥ 0. */
  readonly k: number;
}

/**
 * The optics of one sphere at one size parameter: what {@link mieSphere} returns.
 *
 * @remarks
 * The absorption efficiency is `qExt − qSca`. With the amplitudes, the differential scattering
 * cross-section for unpolarised light is (|S₁|² + |S₂|²) ÷ (2k²), k the wavenumber in the medium,
 * so the phase function normalised over the sphere, ∫ p dΩ = 4π, is
 * 2 (|S₁|² + |S₂|²) ÷ (x² Q_sca), and ∫₋₁¹ (|S₁|² + |S₂|²) dμ = x² Q_sca.
 */
export interface MieResult {
  /** The extinction efficiency Q_ext, the extinction cross-section over πr². */
  readonly qExt: number;
  /** The scattering efficiency Q_sca. */
  readonly qSca: number;
  /** The asymmetry parameter g, the mean cosine of the scattering angle. */
  readonly asymmetry: number;
  /**
   * The amplitude S₁ (perpendicular to the scattering plane) at each requested cosine, as
   * interleaved real and imaginary parts: S₁(μᵢ) is `s1[2i] + i s1[2i + 1]`.
   */
  readonly s1: Float64Array;
  /** The amplitude S₂ (parallel to the scattering plane), interleaved as {@link MieResult.s1} is. */
  readonly s2: Float64Array;
}

/**
 * The largest size parameter {@link mieSphere} accepts: the top of the range over which Wiscombe
 * fitted his N_stop (NCAR/TN-140+STR, (50), 0.02 ≤ x ≤ 20,000; {@link mieTermCount}).
 */
export const MIE_MAX_SIZE_PARAMETER = 20_000;

/**
 * The smallest size parameter {@link mieSphere} accepts, far below any aerosol: x = 10⁻⁶ is a radius
 * of 6 × 10⁻⁵ nm at 380 nm.
 *
 * @remarks
 * It keeps the arithmetic far from underflow, which tens of orders of magnitude lower empties
 * Q_sca (|a₁|² ∝ x⁶), leaves g as 0 ÷ 0 and overflows the continued fraction's 1 ÷ |z|².
 */
export const MIE_MIN_SIZE_PARAMETER = 1e-6;

/**
 * The number of terms the Mie series is summed to, N_stop = ⌊x + 4.05 x^(1/3) + 2⌋.
 *
 * @remarks
 * Wiscombe 1980's criterion, (50) of his report: x + 4 x^(1/3) + 1 to x = 8, x + 4.05 x^(1/3) + 2
 * to x = 4,200, x + 4 x^(1/3) + 2 to 20,000. He fitted it to the first n at which
 * |aₙ|² + |bₙ|² < 5 × 10⁻¹⁴ (Dave's criterion, his (49), tightened in his §6), over
 * 0.1 ≤ x ≤ 20,000, 1.05 ≤ Re m ≤ 2.50 and 0 ≤ Im m ≤ 1; his own test cases go beyond that range
 * of index (0.75, 10 − 10i). Its middle branch is used throughout, which is never fewer terms than
 * the other two, and so never less accurate. At least two terms are summed, so a small sphere keeps
 * its first quadrupole and magnetic-dipole terms.
 *
 * @param sizeParameter - x = 2πr ÷ λ, with λ the wavelength in the medium; > 0.
 */
export function mieTermCount(sizeParameter: number): number {
  return Math.floor(sizeParameter + 4.05 * Math.cbrt(sizeParameter) + 2);
}

/** The logarithmic derivatives Dₙ(z) of ψₙ(z), n = 0 … nMax, as {@link logarithmicDerivatives} returns them. */
export interface LogarithmicDerivatives {
  /** Re Dₙ(z) at index n. */
  readonly re: Float64Array;
  /** Im Dₙ(z) at index n. */
  readonly im: Float64Array;
}

/**
 * The continued fraction's convergence test, |Δ − 1| for the modified Lentz method's last factor.
 *
 * @remarks
 * A few units of double rounding: Wiscombe's single-precision ε₂ = 10⁻⁸ (his (31)) gave 5 to 6
 * digits; this gives D_N to about 15.
 */
const LENTZ_TOLERANCE = 1e-15;

/** The continued fraction's guard against a zero convergent (Thompson and Barnett 1986). */
const LENTZ_TINY = 1e-300;

/**
 * Dₙ(z) = ψₙ′(z) ÷ ψₙ(z), the logarithmic derivative of the Riccati–Bessel function ψₙ(z) = z jₙ(z),
 * for n = 0 … `nMax`, by downward recurrence from D at `nMax`.
 *
 * @remarks
 * D_nMax is the continued fraction (24, 25) of Lentz 1976 (W. J. Lentz, "Generating Bessel functions
 * in Mie scattering calculations using continued fractions", Appl. Opt. 15, 668–671),
 * D_N = a₁ + 1 ÷ (a₂ + 1 ÷ (a₃ + …)) with a₁ = (N + 1) ÷ z and a_k = (−1)^(k+1) (2N + 2k − 1) ÷ z.
 * It is evaluated as Lentz's running product of ratios of convergents, in the modified form of
 * Thompson and Barnett 1986 (J. Comput. Phys. 64, 490–509), which replaces Lentz's two-term stride
 * past an ill-conditioned ratio (Wiscombe's (32–35)) by a tiny-number guard. The recurrence (23),
 * D_{n−1} = n ÷ z − 1 ÷ (Dₙ + n ÷ z), then runs down to D₀ = cot z.
 *
 * Why not Bohren and Huffman's start: their BHMIE (Absorption and Scattering of Light by Small
 * Particles, 1983, Appendix A, p. 478) sets D = 0 at n = max(x + 4 x^(1/3) + 2, |z|) + 15 and
 * recurs down. A starting error shrinks, by (ψ at the start ÷ ψ at n)², only above n = |z|, where
 * ψₙ(z) falls off across the Airy transition, and is carried undamped below it. The transition
 * widens as |z|^(1/3), so a fixed 15 terms covers less of it as |z| grows: at |z| = 1,500 they
 * reach only about 1.7 on the Airy argument 2^(1/3) (ν − |z|) ÷ ν^(1/3), ν = n + ½ (the
 * transition-region form, Abramowitz and Stegun 9.3.23), and D₀ comes out 27% wrong and D₁ 38%
 * (`mie.test.ts` measures D₀). The figure swings with z, but over |z| = 1,450–1,550 neither D₀
 * nor D₁ is ever better than 5%, measured in steps of 0.05.
 * The continued fraction is exact at any |z|.
 *
 * @param zRe - Re z, with z = mx in {@link mieSphere}.
 * @param zIm - Im z, ≤ 0 for an absorbing sphere in {@link mieSphere}'s convention, though the
 *   recurrence holds for either sign.
 * @param nMax - The highest index, an integer ≥ 1.
 * @throws RangeError if z is zero or not finite, or `nMax` is not an integer ≥ 1.
 * @throws Error if the continued fraction has not converged in its iteration cap, a broken
 *   invariant: it converges for every finite z ≠ 0 whose 1 ÷ |z|² is representable.
 */
export function logarithmicDerivatives(
  zRe: number,
  zIm: number,
  nMax: number,
): LogarithmicDerivatives {
  if (!(Number.isFinite(zRe) && Number.isFinite(zIm) && (zRe !== 0 || zIm !== 0))) {
    throw new RangeError(`argument ${zRe} + ${zIm}i needs to be finite and non-zero`);
  }
  if (!(Number.isInteger(nMax) && nMax >= 1)) {
    throw new RangeError(`highest index ${nMax} needs to be an integer ≥ 1`);
  }
  const re = new Float64Array(nMax + 1);
  const im = new Float64Array(nMax + 1);
  // w = 1 ÷ z.
  const zNorm = zRe * zRe + zIm * zIm;
  const wRe = zRe / zNorm;
  const wIm = -zIm / zNorm;
  const start = lentzStart(wRe, wIm, Math.hypot(zRe, zIm), nMax);
  let dRe = start.re;
  let dIm = start.im;
  re[nMax] = dRe;
  im[nMax] = dIm;
  for (let n = nMax; n >= 1; n -= 1) {
    // D_{n−1} = n w − 1 ÷ (Dₙ + n w).
    const nwRe = n * wRe;
    const nwIm = n * wIm;
    const sRe = dRe + nwRe;
    const sIm = dIm + nwIm;
    const sNorm = sRe * sRe + sIm * sIm;
    dRe = nwRe - sRe / sNorm;
    dIm = nwIm + sIm / sNorm;
    re[n - 1] = dRe;
    im[n - 1] = dIm;
  }
  return { re, im };
}

/** A complex number, as the continued fraction and each Mie coefficient return it. */
interface Complex {
  readonly re: number;
  readonly im: number;
}

/**
 * D_N(z) from Lentz's continued fraction by the modified Lentz method, given w = 1 ÷ z.
 *
 * @remarks
 * Wiscombe's Table 2 finds about |m − 1| x iterations at small Im m once x > 100, and far fewer as
 * Im m grows; the cap allows four times the larger of |z| and N, and 1,000 more.
 */
function lentzStart(wRe: number, wIm: number, zAbs: number, order: number): Complex {
  // f₀ = a₁ = (N + 1) w.
  let fRe = (order + 1) * wRe;
  let fIm = (order + 1) * wIm;
  if (fRe === 0 && fIm === 0) {
    fRe = LENTZ_TINY;
  }
  let cRe = fRe;
  let cIm = fIm;
  let dRe = 0;
  let dIm = 0;
  const cap = 4 * Math.ceil(Math.max(zAbs, order)) + 1_000;
  for (let k = 2; k <= cap; k += 1) {
    // a_k = (−1)^(k+1) (2N + 2k − 1) w.
    const scale = (k % 2 === 0 ? -1 : 1) * (2 * order + 2 * k - 1);
    const aRe = scale * wRe;
    const aIm = scale * wIm;
    // D ← 1 ÷ (a_k + D).
    let pRe = aRe + dRe;
    let pIm = aIm + dIm;
    if (pRe === 0 && pIm === 0) {
      pRe = LENTZ_TINY;
    }
    const pNorm = pRe * pRe + pIm * pIm;
    dRe = pRe / pNorm;
    dIm = -pIm / pNorm;
    // C ← a_k + 1 ÷ C.
    const cNorm = cRe * cRe + cIm * cIm;
    cRe = aRe + cRe / cNorm;
    cIm = aIm - cIm / cNorm;
    if (cRe === 0 && cIm === 0) {
      cRe = LENTZ_TINY;
    }
    // Δ = C D; f ← f Δ.
    const deltaRe = cRe * dRe - cIm * dIm;
    const deltaIm = cRe * dIm + cIm * dRe;
    const nextRe = fRe * deltaRe - fIm * deltaIm;
    fIm = fRe * deltaIm + fIm * deltaRe;
    fRe = nextRe;
    if (Math.abs(deltaRe - 1) + Math.abs(deltaIm) < LENTZ_TOLERANCE) {
      return { re: fRe, im: fIm };
    }
  }
  throw new Error(
    `Lentz's continued fraction for D_${order} did not converge in ${cap} iterations`,
  );
}

/**
 * Below this size parameter ψ₁(x) is taken from its Taylor series: sin x ÷ x − cos x loses
 * 2 log₁₀(1 ÷ x) digits to cancellation, measured at 2 × 10⁻¹⁴ of its value at x = 0.1 and
 * 3 × 10⁻⁶ at 10⁻⁵.
 */
const PSI1_SERIES_BELOW = 0.1;

/**
 * ψ₁(x) = sin x ÷ x − cos x, from its Taylor series below {@link PSI1_SERIES_BELOW}:
 * x²/3 − x⁴/30 + x⁶/840 − x⁸/45360 + x¹⁰/3991680, whose next term is 6 × 10⁻¹⁹ of the sum at 0.1.
 */
function psi1(x: number): number {
  if (x >= PSI1_SERIES_BELOW) {
    return Math.sin(x) / x - Math.cos(x);
  }
  const x2 = x * x;
  return x2 * (1 / 3 - x2 * (1 / 30 - x2 * (1 / 840 - x2 * (1 / 45_360 - x2 / 3_991_680))));
}

/**
 * The Mie solution for a homogeneous sphere: Q_ext, Q_sca, g and the amplitudes S₁, S₂ at the
 * scattering-angle cosines `mu`.
 *
 * @remarks
 * Wiscombe's algorithm, as the module's remarks describe. The cost is N_stop × (`mu.length` + 1)
 * terms, plus the continued fraction's iterations; R08.T5.a records the measured times.
 *
 * @param sizeParameter - x = 2πr ÷ λ, with λ the wavelength in the medium; in
 *   [{@link MIE_MIN_SIZE_PARAMETER}, {@link MIE_MAX_SIZE_PARAMETER}].
 * @param index - The sphere's index relative to the medium.
 * @param mu - The cosines of the scattering angles, each in [−1, 1]; may be empty.
 * @throws RangeError if the size parameter, the index or a cosine is outside its range.
 */
export function mieSphere(sizeParameter: number, index: ComplexIndex, mu: Float64Array): MieResult {
  const coefficients = mieCoefficients(sizeParameter, index);
  const { aRe, aIm, bRe, bIm } = coefficients;
  const x = sizeParameter;
  let extinctionSum = 0;
  let scatteringSum = 0;
  let asymmetrySum = 0;
  let aPrevRe = 0;
  let aPrevIm = 0;
  let bPrevRe = 0;
  let bPrevIm = 0;
  for (let n = 1; n <= aRe.length; n += 1) {
    const a1 = aRe[n - 1] ?? 0;
    const a2 = aIm[n - 1] ?? 0;
    const b1 = bRe[n - 1] ?? 0;
    const b2 = bIm[n - 1] ?? 0;
    const twoNPlus1 = 2 * n + 1;
    extinctionSum += twoNPlus1 * (a1 + b1);
    scatteringSum += twoNPlus1 * (a1 * a1 + a2 * a2 + b1 * b1 + b2 * b2);
    // (8): (n − 1)(n + 1) ÷ n Re(aₙ₋₁ aₙ* + bₙ₋₁ bₙ*) + (2n + 1) ÷ (n (n + 1)) Re(aₙ bₙ*).
    const weight = twoNPlus1 / (n * (n + 1));
    asymmetrySum +=
      (n - 1 / n) * (aPrevRe * a1 + aPrevIm * a2 + bPrevRe * b1 + bPrevIm * b2) +
      weight * (a1 * b1 + a2 * b2);
    aPrevRe = a1;
    aPrevIm = a2;
    bPrevRe = b1;
    bPrevIm = b2;
  }
  const { s1, s2 } = mieAmplitudes(coefficients, mu);
  const xSquared = x * x;
  const qSca = (2 / xSquared) * scatteringSum;
  return {
    qExt: (2 / xSquared) * extinctionSum,
    qSca,
    asymmetry: ((4 / xSquared) * asymmetrySum) / qSca,
    s1,
    s2,
  };
}

/**
 * A sphere's Mie coefficients aₙ and bₙ, n = 1 … N, in {@link mieSphere}'s convention (e^(+iωt);
 * Bohren and Huffman's are their complex conjugates): entry n − 1 holds term n.
 */
export interface MieCoefficients {
  readonly aRe: Float64Array;
  readonly aIm: Float64Array;
  readonly bRe: Float64Array;
  readonly bIm: Float64Array;
}

/**
 * A sphere's Mie coefficients aₙ and bₙ to {@link mieTermCount} terms, as {@link mieSphere} sums
 * them (Wiscombe's (16), with the same recurrences): R08.T5.c's aggregates replace them by their
 * mean-field coefficients and sum the amplitudes with {@link mieAmplitudes}.
 *
 * @param sizeParameter - x = 2πr ÷ λ; in [{@link MIE_MIN_SIZE_PARAMETER}, {@link MIE_MAX_SIZE_PARAMETER}].
 * @throws RangeError if the size parameter or the index is outside its range.
 */
export function mieCoefficients(sizeParameter: number, index: ComplexIndex): MieCoefficients {
  const x = sizeParameter;
  if (!(x >= MIE_MIN_SIZE_PARAMETER && x <= MIE_MAX_SIZE_PARAMETER)) {
    throw new RangeError(
      `size parameter ${x} is outside [${MIE_MIN_SIZE_PARAMETER}, ${MIE_MAX_SIZE_PARAMETER}]`,
    );
  }
  if (!(index.n > 0 && Number.isFinite(index.n) && index.k >= 0 && Number.isFinite(index.k))) {
    throw new RangeError(
      `refractive index ${index.n} + ${index.k}i needs a finite n > 0 and a finite k ≥ 0`,
    );
  }
  const terms = mieTermCount(x);
  const mRe = index.n;
  const mIm = -index.k;
  const mNorm = mRe * mRe + mIm * mIm;
  const mInvRe = mRe / mNorm;
  const mInvIm = -mIm / mNorm;
  const logD = logarithmicDerivatives(mRe * x, mIm * x, terms);
  const out = {
    aRe: new Float64Array(terms),
    aIm: new Float64Array(terms),
    bRe: new Float64Array(terms),
    bIm: new Float64Array(terms),
  };
  const xInv = 1 / x;
  let psiPrevious = Math.sin(x);
  let chiPrevious = Math.cos(x);
  let psi = psi1(x);
  let chi = chiPrevious * xInv + psiPrevious;
  for (let n = 1; n <= terms; n += 1) {
    const dRe = logD.re[n] ?? 0;
    const dIm = logD.im[n] ?? 0;
    const nOverX = n * xInv;
    const a = coefficient(
      dRe * mInvRe - dIm * mInvIm + nOverX,
      dRe * mInvIm + dIm * mInvRe,
      psi,
      chi,
      psiPrevious,
      chiPrevious,
    );
    const b = coefficient(
      dRe * mRe - dIm * mIm + nOverX,
      dRe * mIm + dIm * mRe,
      psi,
      chi,
      psiPrevious,
      chiPrevious,
    );
    out.aRe[n - 1] = a.re;
    out.aIm[n - 1] = a.im;
    out.bRe[n - 1] = b.re;
    out.bIm[n - 1] = b.im;
    const psiNext = (2 * n + 1) * xInv * psi - psiPrevious;
    const chiNext = (2 * n + 1) * xInv * chi - chiPrevious;
    psiPrevious = psi;
    chiPrevious = chi;
    psi = psiNext;
    chi = chiNext;
  }
  return out;
}

/**
 * The amplitudes S₁ and S₂ (9) that a set of coefficients aₙ, bₙ gives at the cosines `mu`, in
 * {@link mieSphere}'s convention and interleaving: a sphere's from {@link mieCoefficients}, or a
 * monomer's mean-field ones (R08.T5.c).
 *
 * @param mu - The scattering-angle cosines, each in [−1, 1].
 * @throws RangeError if a cosine is outside [−1, 1] or the coefficient arrays differ in length.
 */
export function mieAmplitudes(
  coefficients: MieCoefficients,
  mu: Float64Array,
): { readonly s1: Float64Array; readonly s2: Float64Array } {
  const { aRe, aIm, bRe, bIm } = coefficients;
  const terms = aRe.length;
  if (aIm.length !== terms || bRe.length !== terms || bIm.length !== terms) {
    throw new RangeError("the coefficient arrays must have one entry per term");
  }
  for (const cosine of mu) {
    if (!(cosine >= -1 && cosine <= 1)) {
      throw new RangeError(`scattering-angle cosine ${cosine} is outside [−1, 1]`);
    }
  }
  const angles = mu.length;
  const s1 = new Float64Array(2 * angles);
  const s2 = new Float64Array(2 * angles);
  const piPrevious = new Float64Array(angles);
  const piCurrent = new Float64Array(angles).fill(1);
  for (let n = 1; n <= terms; n += 1) {
    const weight = (2 * n + 1) / (n * (n + 1));
    const caRe = weight * (aRe[n - 1] ?? 0);
    const caIm = weight * (aIm[n - 1] ?? 0);
    const cbRe = weight * (bRe[n - 1] ?? 0);
    const cbIm = weight * (bIm[n - 1] ?? 0);
    const nPlus1OverN = 1 + 1 / n;
    for (let j = 0; j < angles; j += 1) {
      const cosine = mu[j] ?? 0;
      const piN = piCurrent[j] ?? 0;
      const piN1 = piPrevious[j] ?? 0;
      const t = cosine * piN - piN1;
      const tau = n * t - piN1;
      s1[2 * j] = (s1[2 * j] ?? 0) + caRe * piN + cbRe * tau;
      s1[2 * j + 1] = (s1[2 * j + 1] ?? 0) + caIm * piN + cbIm * tau;
      s2[2 * j] = (s2[2 * j] ?? 0) + caRe * tau + cbRe * piN;
      s2[2 * j + 1] = (s2[2 * j + 1] ?? 0) + caIm * tau + cbIm * piN;
      piPrevious[j] = piN;
      piCurrent[j] = cosine * piN + nPlus1OverN * t;
    }
  }
  return { s1, s2 };
}

/**
 * One Mie coefficient, (T ψₙ − ψₙ₋₁) ÷ (T ζₙ − ζₙ₋₁) with ζ = ψ + iχ and T complex: (16) of
 * Wiscombe's report, for aₙ with T = Dₙ ÷ m + n ÷ x and bₙ with T = m Dₙ + n ÷ x.
 */
function coefficient(
  tRe: number,
  tIm: number,
  psi: number,
  chi: number,
  psiPrevious: number,
  chiPrevious: number,
): Complex {
  const numRe = tRe * psi - psiPrevious;
  const numIm = tIm * psi;
  // T (ψ + iχ) − (ψ₋₁ + iχ₋₁).
  const denRe = tRe * psi - tIm * chi - psiPrevious;
  const denIm = tRe * chi + tIm * psi - chiPrevious;
  const denNorm = denRe * denRe + denIm * denIm;
  return {
    re: (numRe * denRe + numIm * denIm) / denNorm,
    im: (numIm * denRe - numRe * denIm) / denNorm,
  };
}
