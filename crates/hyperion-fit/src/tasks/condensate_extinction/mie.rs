//! Mie scattering by a homogeneous sphere (galaxy plan 14, P14.T49.e): the efficiencies, the
//! asymmetry parameter and the scattering amplitudes of one sphere, from which
//! [`condensate_extinction`](super) averages each aerosol material's extinction over its size
//! distributions.
//!
//! It is the client's Mie (`apps/hyperion/src/renderer/src/view/atmosphere/mie.ts`, rendering plan
//! R08.T5.a) in Rust, on the same structure, so that the record's τ(550) and the client's sky take
//! one sphere's optics from one algorithm. The algorithm is Wiscombe 1980 (W. J. Wiscombe,
//! "Improved Mie scattering algorithms", Appl. Opt. 19, 1505–1509), as his report derives it and
//! his MIEV0 builds it (Wiscombe, "Mie scattering calculations: advances in technique and fast,
//! vector-speed computer codes", NCAR/TN-140+STR, 1979, edited 1996; the equation numbers below
//! are the report's). It runs in double precision, without MIEV0's small-particle formulas or its
//! up-recurrence of Dₙ:
//!
//! - the series is cut at `N_stop` terms ([`term_count`]);
//! - the Riccati–Bessel functions ψₙ(x) and χₙ(x) go by upward recurrence, (17) from (19), which
//!   the report finds accurate to `N_stop` (its Table 1), since ψₙ only begins to lose digits past
//!   n = x;
//! - the logarithmic derivative Dₙ(mx) = ψₙ′(mx) ÷ ψₙ(mx) goes by downward recurrence (23) from
//!   its value at `N_stop`, which Lentz 1976's continued fraction gives to rounding. Downward
//!   recurrence is stable for every index (G. W. Kattawar and G. N. Plass, "Electromagnetic
//!   scattering from absorbing spheres", Appl. Opt. 6, 1377–1382, 1967);
//! - aₙ and bₙ are (16), the efficiencies (6) and (7), the asymmetry (8), and the amplitudes S₁
//!   and S₂ (9), with the angular functions πₙ and τₙ by upward recurrence (37, 38).
//!
//! Every transcendental goes through [`hyperion_sim::math`], so a table made from it comes out the
//! same on every platform.
//!
//! Sign convention: a [`ComplexIndex`] is m = n + ik with k ≥ 0 absorbing, as tables of optical
//! constants give it. The amplitudes are in Wiscombe's and van de Hulst's convention (time factor
//! e^(+iωt), the index written n − ik, the report's (3)); Bohren and Huffman's (e^(−iωt), their
//! §4.3) are their complex conjugates. The efficiencies, the asymmetry and |S₁|², |S₂|² do not
//! depend on it.

use std::ops::{Add, Div, Mul, Sub};

use hyperion_sim::math;

/// The largest size parameter the sphere accepts: the top of the range Wiscombe states for his
/// `N_stop` criterion, 0.02 ≤ x ≤ 20,000 (Appl. Opt. 19, 1505–1509, 1980, p. 1508; NCAR/TN-140+STR,
/// (50); [`term_count`]), the client's `MIE_MAX_SIZE_PARAMETER`.
pub const MAX_SIZE_PARAMETER: f64 = 20_000.0;

/// The smallest size parameter the sphere accepts, far below any aerosol, the client's
/// `MIE_MIN_SIZE_PARAMETER`: x = 10⁻⁶ is a radius of 9 × 10⁻⁵ nm at 550 nm. Below x = 0.02, the
/// bottom of the criterion's range, the term count is extrapolated ([`term_count`]), and the
/// Rayleigh test holds the series there.
///
/// It keeps the arithmetic far from underflow, which tens of orders of magnitude lower empties
/// `Q_sca` (|a₁|² ∝ x⁶), leaves g as 0 ÷ 0 and overflows the continued fraction's 1 ÷ |z|².
pub const MIN_SIZE_PARAMETER: f64 = 1e-6;

/// The continued fraction's convergence test, |Δ − 1| for the modified Lentz method's last factor:
/// a few units of double rounding. Wiscombe's single-precision ε₂ = 10⁻⁸ (his (31)) gave 5 to 6
/// digits; this gives `D_N` to about 15.
const LENTZ_TOLERANCE: f64 = 1e-15;

/// The continued fraction's guard against a zero convergent (Thompson and Barnett 1986).
const LENTZ_TINY: f64 = 1e-300;

/// Below this size parameter ψ₁(x) is taken from its Taylor series: sin x ÷ x − cos x loses
/// 2 log₁₀(1 ÷ x) digits to cancellation, measured at 2 × 10⁻¹⁴ of its value at x = 0.1 and
/// 3 × 10⁻⁶ at 10⁻⁵ (the client's `PSI1_SERIES_BELOW`).
const PSI1_SERIES_BELOW: f64 = 0.1;

/// A complex refractive index relative to the medium around the sphere, m = n + ik: the index of
/// the sphere's material divided by the medium's real index, as tables of optical constants give
/// n and k (k ≥ 0 absorbing; Bohren and Huffman's sign).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ComplexIndex {
    n: f64,
    k: f64,
}

/// The largest n and k an index may have: ten times Wiscombe's largest test index (10 + 10i) and
/// thirty times the largest of R08's files (chromium's n 3.18, k 3.33 at 550 nm), so that |m| x
/// stays below 3 × 10⁶ and the continued fraction's cap far below `usize::MAX`.
pub const MAX_INDEX_PART: f64 = 100.0;

impl ComplexIndex {
    /// The index n + ik.
    ///
    /// # Errors
    ///
    /// [`ComputeMieError::Index`] unless 0 < n ≤ [`MAX_INDEX_PART`] and 0 ≤ k ≤
    /// [`MAX_INDEX_PART`].
    pub fn new(n: f64, k: f64) -> Result<Self, ComputeMieError> {
        if n > 0.0 && n <= MAX_INDEX_PART && (0.0..=MAX_INDEX_PART).contains(&k) {
            Ok(Self { n, k })
        } else {
            Err(ComputeMieError::Index { n, k })
        }
    }

    /// The real part, the phase index: finite and positive.
    #[must_use]
    pub const fn n(self) -> f64 {
        self.n
    }

    /// The absorption index, the imaginary part: finite and at least zero.
    #[must_use]
    pub const fn k(self) -> f64 {
        self.k
    }
}

/// A sphere's extinction and scattering efficiencies and its asymmetry parameter at one size
/// parameter.
///
/// The absorption efficiency is `q_ext − q_sca`, and the cross-sections are the efficiencies times
/// the sphere's geometric cross-section πr².
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Efficiencies {
    q_ext: f64,
    q_sca: f64,
    asymmetry: f64,
}

impl Efficiencies {
    /// The extinction efficiency `Q_ext`, the extinction cross-section over πr².
    #[must_use]
    pub const fn q_ext(self) -> f64 {
        self.q_ext
    }

    /// The scattering efficiency `Q_sca`.
    #[must_use]
    pub const fn q_sca(self) -> f64 {
        self.q_sca
    }

    /// The asymmetry parameter g, the mean cosine of the scattering angle.
    #[must_use]
    pub const fn asymmetry(self) -> f64 {
        self.asymmetry
    }
}

/// A sphere's scattering amplitudes at a set of scattering-angle cosines, each as its real and
/// imaginary parts, in the [module's](self) convention.
///
/// With them, the differential scattering cross-section for unpolarised light is
/// (|S₁|² + |S₂|²) ÷ (2k²), k the wavenumber in the medium.
#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub struct Amplitudes {
    s1: Vec<(f64, f64)>,
    s2: Vec<(f64, f64)>,
}

impl Amplitudes {
    /// S₁, perpendicular to the scattering plane, at each cosine asked, as (real, imaginary).
    #[must_use]
    pub fn s1(&self) -> &[(f64, f64)] {
        &self.s1
    }

    /// S₂, parallel to the scattering plane, at each cosine asked, as (real, imaginary).
    #[must_use]
    pub fn s2(&self) -> &[(f64, f64)] {
        &self.s2
    }
}

/// A sphere's optics could not be computed.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum ComputeMieError {
    /// The size parameter is outside [[`MIN_SIZE_PARAMETER`], [`MAX_SIZE_PARAMETER`]].
    #[error("size parameter {0} is outside [1e-6, 20000]")]
    SizeParameter(f64),
    /// The index is not an n in (0, 100] with a k in [0, 100] ([`MAX_INDEX_PART`]).
    #[error("refractive index {n} + {k}i needs an n in (0, 100] and a k in [0, 100]")]
    Index {
        /// The real part asked.
        n: f64,
        /// The imaginary part asked.
        k: f64,
    },
    /// A scattering-angle cosine is outside [−1, 1].
    #[error("scattering-angle cosine {0} is outside [−1, 1]")]
    Cosine(f64),
    /// Lentz's continued fraction did not converge within its cap, a broken invariant: it
    /// converges for every finite z ≠ 0 whose 1 ÷ |z|² is representable.
    #[error(
        "the continued fraction for D_{order} (Lentz 1976) did not converge in {cap} iterations"
    )]
    Unconverged {
        /// The order whose logarithmic derivative was sought.
        order: usize,
        /// The iteration cap.
        cap: usize,
    },
}

/// A complex number, as the recurrences and the coefficients carry it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    #[must_use]
    const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// |z|².
    #[must_use]
    fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// 1 ÷ z.
    #[must_use]
    fn recip(self) -> Self {
        let norm = self.norm_sqr();
        Self::new(self.re / norm, -self.im / norm)
    }

    /// z times the real `s`.
    #[must_use]
    fn scale(self, s: f64) -> Self {
        Self::new(self.re * s, self.im * s)
    }

    /// Whether both parts are zero (or subnormal, whose reciprocal would overflow): the modified
    /// Lentz method's test for a zero convergent.
    #[must_use]
    fn is_negligible(self) -> bool {
        self.re.abs() < f64::MIN_POSITIVE && self.im.abs() < f64::MIN_POSITIVE
    }
}

impl Add for Complex {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.re + other.re, self.im + other.im)
    }
}

impl Sub for Complex {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.re - other.re, self.im - other.im)
    }
}

impl Mul for Complex {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }
}

impl Div for Complex {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        let norm = other.norm_sqr();
        Self::new(
            (self.re * other.re + self.im * other.im) / norm,
            (self.im * other.re - self.re * other.im) / norm,
        )
    }
}

/// The number of terms the Mie series is summed to, `N_stop` = ⌊x + 4.05 x^(1/3) + 2⌋.
///
/// Wiscombe 1980's criterion, (50) of his report: x + 4 x^(1/3) + 1 to x = 8, x + 4.05 x^(1/3) + 2
/// to x = 4,200, x + 4 x^(1/3) + 2 to 20,000, stated for 0.02 ≤ x ≤ 20,000 (Appl. Opt. 19, p.
/// 1508). Two ranges are his, and they differ:
///
/// - **the criterion's**, 0.02 ≤ x ≤ 20,000, which the sphere's bounds follow at the top; below x =
///   0.02 the count is extrapolated to [`MIN_SIZE_PARAMETER`], and the Rayleigh test
///   (`mie_meets_the_rayleigh_limit`, x = 10⁻³ and 10⁻⁵) holds the series there;
/// - **the data's** he fitted it to, the first n at which |aₙ|² + |bₙ|² < 5 × 10⁻¹⁴ (Dave's
///   criterion, his (49), tightened in his §6), over 0.1 ≤ x ≤ 20,000, 1.05 ≤ Re m ≤ 2.50 and 0 ≤
///   Im m ≤ 1. Chromium's index at 550 nm, 3.18 + 3.33i, lies outside it, as do his own test
///   cases at 0.75 and 10 − 10i, of which MIEV0's cases 17–19 at 10 − 10i pass here to 10⁻⁶.
///
/// Its middle branch is used throughout, as the client's `mieTermCount` does, which is never fewer
/// terms than the other two and so never less accurate. At least two terms are summed, so a small
/// sphere keeps its first quadrupole and magnetic-dipole terms.
///
/// `size_parameter` is x = 2πr ÷ λ, with λ the wavelength in the medium, taken into
/// [[`MIN_SIZE_PARAMETER`], [`MAX_SIZE_PARAMETER`]] (NaN as the smallest), where the count is 2 to
/// 20,111.
#[must_use]
pub fn term_count(size_parameter: f64) -> usize {
    let x = if size_parameter.is_nan() {
        MIN_SIZE_PARAMETER
    } else {
        size_parameter.clamp(MIN_SIZE_PARAMETER, MAX_SIZE_PARAMETER)
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number from 2 to 20,111 for x clamped to [1e-6, 20000], exact in a usize"
    )]
    let count = (x + 4.05 * math::cbrt(x) + 2.0).floor() as usize;
    count
}

/// Dₙ(z) = ψₙ′(z) ÷ ψₙ(z), the logarithmic derivative of the Riccati–Bessel function ψₙ(z) =
/// z jₙ(z), for n = 0 … `n_max`, by downward recurrence from D at `n_max`.
///
/// `D_nMax` is the continued fraction (24, 25) of Lentz 1976 (W. J. Lentz, "Generating Bessel
/// functions in Mie scattering calculations using continued fractions", Appl. Opt. 15, 668–671),
/// `D_N` = a₁ + 1 ÷ (a₂ + 1 ÷ (a₃ + …)) with a₁ = (N + 1) ÷ z and `a_k` = (−1)^(k+1) (2N + 2k − 1)
/// ÷ z, evaluated as Lentz's running product of ratios of convergents in the modified form of
/// Thompson and Barnett 1986 (J. Comput. Phys. 64, 490–509). The recurrence (23), `D_{n−1}` = n ÷
/// z − 1 ÷ (Dₙ + n ÷ z), then runs down to D₀ = cot z. The client's `logarithmicDerivatives`
/// gives its reasons for this start over Bohren and Huffman's.
fn logarithmic_derivatives(z: Complex, n_max: usize) -> Result<Vec<Complex>, ComputeMieError> {
    let w = z.recip();
    let mut d = vec![Complex::default(); n_max + 1];
    let mut current = lentz_start(w, math::hypot(z.re, z.im), n_max)?;
    d[n_max] = current;
    for n in (1..=n_max).rev() {
        // D_{n−1} = n w − 1 ÷ (Dₙ + n w).
        #[expect(
            clippy::cast_precision_loss,
            reason = "an order below 2^53, exact in an f64"
        )]
        let nw = w.scale(n as f64);
        current = nw - (current + nw).recip();
        d[n - 1] = current;
    }
    Ok(d)
}

/// `D_N`(z) from Lentz's continued fraction by the modified Lentz method, given w = 1 ÷ z.
///
/// Wiscombe's Table 2 finds about |m − 1| x iterations at small Im m once x > 100, and far fewer
/// as Im m grows; the cap allows four times the larger of |z| and N, and 1,000 more, as the
/// client's does.
#[expect(
    clippy::cast_precision_loss,
    reason = "orders and iteration counts below 2^53, exact in an f64"
)]
fn lentz_start(inverse: Complex, z_abs: f64, order: usize) -> Result<Complex, ComputeMieError> {
    let order_f = order as f64;
    // f₀ = a₁ = (N + 1) w.
    let mut fraction = inverse.scale(order_f + 1.0);
    if fraction.is_negligible() {
        fraction = Complex::new(LENTZ_TINY, 0.0);
    }
    let mut c_ratio = fraction;
    let mut d_ratio = Complex::default();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "|z| is at most 20,000 × 100√2 for an index ComplexIndex admits, and N at most \
                  20,111, far below usize::MAX; the arithmetic saturates all the same"
    )]
    let cap = (z_abs.max(order_f).ceil() as usize)
        .saturating_mul(4)
        .saturating_add(1_000);
    for k in 2..=cap {
        // a_k = (−1)^(k+1) (2N + 2k − 1) w.
        let sign = if k % 2 == 0 { -1.0 } else { 1.0 };
        let term = inverse.scale(sign * (2.0 * order_f + 2.0 * k as f64 - 1.0));
        // D ← 1 ÷ (a_k + D).
        let mut shifted = term + d_ratio;
        if shifted.is_negligible() {
            shifted = Complex::new(LENTZ_TINY, 0.0);
        }
        d_ratio = shifted.recip();
        // C ← a_k + 1 ÷ C.
        c_ratio = term + c_ratio.recip();
        if c_ratio.is_negligible() {
            c_ratio = Complex::new(LENTZ_TINY, 0.0);
        }
        // Δ = C D; f ← f Δ.
        let delta = c_ratio * d_ratio;
        fraction = fraction * delta;
        if (delta.re - 1.0).abs() + delta.im.abs() < LENTZ_TOLERANCE {
            return Ok(fraction);
        }
    }
    Err(ComputeMieError::Unconverged { order, cap })
}

/// ψ₁(x) = sin x ÷ x − cos x, from its Taylor series below [`PSI1_SERIES_BELOW`]: x²/3 − x⁴/30 +
/// x⁶/840 − x⁸/45360 + x¹⁰/3991680, whose next term is 6 × 10⁻¹⁹ of the sum at 0.1.
#[must_use]
fn psi1(x: f64) -> f64 {
    if x >= PSI1_SERIES_BELOW {
        return math::sin(x) / x - math::cos(x);
    }
    let x2 = x * x;
    x2 * (1.0 / 3.0
        - x2 * (1.0 / 30.0 - x2 * (1.0 / 840.0 - x2 * (1.0 / 45_360.0 - x2 / 3_991_680.0))))
}

/// One Mie coefficient, (T ψₙ − ψₙ₋₁) ÷ (T ζₙ − ζₙ₋₁) with ζ = ψ + iχ and T complex: (16) of
/// Wiscombe's report, for aₙ with T = Dₙ ÷ m + n ÷ x and bₙ with T = m Dₙ + n ÷ x.
#[must_use]
fn coefficient(t: Complex, psi: f64, chi: f64, psi_previous: f64, chi_previous: f64) -> Complex {
    let numerator = Complex::new(t.re * psi - psi_previous, t.im * psi);
    // T (ψ + iχ) − (ψ₋₁ + iχ₋₁).
    let denominator = Complex::new(
        t.re * psi - t.im * chi - psi_previous,
        t.re * chi + t.im * psi - chi_previous,
    );
    numerator / denominator
}

/// Calls `each` with n, aₙ and bₙ for n = 1 … [`term_count`] of a sphere of size parameter `x`
/// and index `index`, in the [module's](self) convention.
fn for_each_coefficient(
    x: f64,
    index: ComplexIndex,
    mut each: impl FnMut(usize, Complex, Complex),
) -> Result<(), ComputeMieError> {
    if !(MIN_SIZE_PARAMETER..=MAX_SIZE_PARAMETER).contains(&x) {
        return Err(ComputeMieError::SizeParameter(x));
    }
    let terms = term_count(x);
    // The index as n − ik, Wiscombe's convention.
    let m = Complex::new(index.n, -index.k);
    let m_inv = m.recip();
    let log_d = logarithmic_derivatives(m.scale(x), terms)?;
    let x_inv = 1.0 / x;
    let mut psi_previous = math::sin(x);
    let mut chi_previous = math::cos(x);
    let mut psi = psi1(x);
    let mut chi = chi_previous * x_inv + psi_previous;
    for (n, &d) in log_d.iter().enumerate().skip(1) {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a term number at most 20,111, exact in an f64"
        )]
        let n_f = n as f64;
        let n_over_x = Complex::new(n_f * x_inv, 0.0);
        let a = coefficient(d * m_inv + n_over_x, psi, chi, psi_previous, chi_previous);
        let b = coefficient(m * d + n_over_x, psi, chi, psi_previous, chi_previous);
        each(n, a, b);
        let psi_next = (2.0 * n_f + 1.0) * x_inv * psi - psi_previous;
        let chi_next = (2.0 * n_f + 1.0) * x_inv * chi - chi_previous;
        psi_previous = psi;
        chi_previous = chi;
        psi = psi_next;
        chi = chi_next;
    }
    Ok(())
}

/// The efficiencies and the asymmetry of a sphere of size parameter x = 2πr ÷ λ (λ in the medium)
/// and index `index`: (6), (7) and (8) of Wiscombe's report, summed to [`term_count`] terms.
///
/// The cost is `N_stop` terms plus the continued fraction's iterations, about x each for a weakly
/// absorbing sphere.
///
/// # Errors
///
/// [`ComputeMieError::SizeParameter`] if `size_parameter` is outside [[`MIN_SIZE_PARAMETER`],
/// [`MAX_SIZE_PARAMETER`]] (or NaN), and [`ComputeMieError::Unconverged`] if the continued
/// fraction failed, a broken invariant.
///
/// # Examples
///
/// Bohren and Huffman's sample sphere, m = 1.55 at x = 5.213, extinguishes about three times its
/// geometric cross-section:
///
/// ```
/// use hyperion_fit::tasks::condensate_extinction::mie::{ComplexIndex, efficiencies};
///
/// let sphere = efficiencies(2.0 * std::f64::consts::PI * 0.525 / 0.6328, ComplexIndex::new(1.55, 0.0)?)?;
/// assert!((sphere.q_ext() - 3.105_43).abs() < 5e-6);
/// # Ok::<(), hyperion_fit::tasks::condensate_extinction::mie::ComputeMieError>(())
/// ```
pub fn efficiencies(
    size_parameter: f64,
    index: ComplexIndex,
) -> Result<Efficiencies, ComputeMieError> {
    let x = size_parameter;
    let mut extinction = 0.0;
    let mut scattering = 0.0;
    let mut asymmetry = 0.0;
    let mut previous = (Complex::default(), Complex::default());
    for_each_coefficient(x, index, |n, a, b| {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a term number at most 20,111, exact in an f64"
        )]
        let n = n as f64;
        let two_n_plus_1 = 2.0 * n + 1.0;
        extinction += two_n_plus_1 * (a.re + b.re);
        scattering += two_n_plus_1 * (a.norm_sqr() + b.norm_sqr());
        // (8): (n − 1)(n + 1) ÷ n Re(aₙ₋₁ aₙ* + bₙ₋₁ bₙ*) + (2n + 1) ÷ (n (n + 1)) Re(aₙ bₙ*).
        let (a_previous, b_previous) = previous;
        asymmetry += (n - 1.0 / n)
            * (a_previous.re * a.re
                + a_previous.im * a.im
                + b_previous.re * b.re
                + b_previous.im * b.im)
            + two_n_plus_1 / (n * (n + 1.0)) * (a.re * b.re + a.im * b.im);
        previous = (a, b);
    })?;
    let x_squared = x * x;
    let q_sca = 2.0 / x_squared * scattering;
    Ok(Efficiencies {
        q_ext: 2.0 / x_squared * extinction,
        q_sca,
        asymmetry: 4.0 / x_squared * asymmetry / q_sca,
    })
}

/// The amplitudes S₁ and S₂ of a sphere of size parameter x and index `index` at the scattering-
/// angle cosines `mu`: (9) of Wiscombe's report, with the angular functions πₙ and τₙ by upward
/// recurrence (37, 38).
///
/// # Errors
///
/// As [`efficiencies`], and [`ComputeMieError::Cosine`] if a cosine is outside [−1, 1].
pub fn amplitudes(
    size_parameter: f64,
    index: ComplexIndex,
    mu: &[f64],
) -> Result<Amplitudes, ComputeMieError> {
    if let Some(&cosine) = mu.iter().find(|c| !(-1.0..=1.0).contains(*c)) {
        return Err(ComputeMieError::Cosine(cosine));
    }
    let angles = mu.len();
    let mut s1 = vec![Complex::default(); angles];
    let mut s2 = vec![Complex::default(); angles];
    let mut pi_previous = vec![0.0; angles];
    let mut pi_current = vec![1.0; angles];
    for_each_coefficient(size_parameter, index, |n, a, b| {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a term number at most 20,111, exact in an f64"
        )]
        let n = n as f64;
        let weight = (2.0 * n + 1.0) / (n * (n + 1.0));
        let ca = a.scale(weight);
        let cb = b.scale(weight);
        let n_plus_1_over_n = 1.0 + 1.0 / n;
        for (j, &cosine) in mu.iter().enumerate() {
            let pi_n = pi_current[j];
            let pi_n1 = pi_previous[j];
            let t = cosine * pi_n - pi_n1;
            let tau = n * t - pi_n1;
            s1[j] = s1[j] + ca.scale(pi_n) + cb.scale(tau);
            s2[j] = s2[j] + ca.scale(tau) + cb.scale(pi_n);
            pi_previous[j] = pi_n;
            pi_current[j] = cosine * pi_n + n_plus_1_over_n * t;
        }
    })?;
    let pairs = |s: Vec<Complex>| s.into_iter().map(|z| (z.re, z.im)).collect();
    Ok(Amplitudes {
        s1: pairs(s1),
        s2: pairs(s2),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of Wiscombe's MIEV0 test cases with his stored answers.
    struct WiscombeCase {
        /// MIEV0's case number.
        id: u32,
        x: f64,
        n: f64,
        k: f64,
        q_ext: f64,
        q_sca: f64,
        /// g `Q_sca`, MIEV0's `GQSC`.
        g_q_sca: f64,
        /// S₁ at 0°, 30°, … 180°, in Wiscombe's sign convention, as [`amplitudes`] returns it.
        s1: [(f64, f64); 7],
        /// S₂ at the same angles.
        s2: [(f64, f64); 7],
        /// The amplitudes, named `S1 90` or `S2 180`, where Wiscombe's stored answer is good to
        /// only a few 10⁻⁶ ([`REFERENCE_LIMITED_TOLERANCE`]).
        limited: &'static [&'static str],
    }

    /// MIEV0's refractive-index test cases 5 to 19 and the answers its test driver checks against,
    /// as the client's `mie.test.ts` holds them: `BLOCK DATA CHEKMI` of `MVTstNew.f` in Wiscombe's
    /// MIEV distribution (the 1996 revision of NCAR/TN-140+STR), "the answer obtained on a Cray
    /// computer using 14-digit precision", stored to 7 significant figures. The `limited` entries
    /// and their reasons are the client's: cases 5 and 12 come from MIEV0's small-particle
    /// formulas, S₂(90°) a remainder 10⁻⁴ of the forward amplitude; case 6 sums 2 terms where
    /// `N_stop` keeps a third, which moves S₂ at 60° and 120° by 7.6 × 10⁻⁷ of the 1.1 × 10⁻⁶
    /// found; case 11 sums 10,088 terms where `N_stop` keeps a 10,089th, a resonance that moves
    /// S(180°) by 4.2 × 10⁻⁶.
    const WISCOMBE_CASES: [WiscombeCase; 15] = [
        WiscombeCase {
            id: 5,
            x: 0.099,
            n: 0.75,
            k: 0.0,
            q_ext: 7.417_859e-6,
            q_sca: 7.417_859e-6,
            g_q_sca: 1.074_279e-8,
            s1: [
                (1.817_558e-8, -1.654_225e-4),
                (1.817_558e-8, -1.653_815e-4),
                (1.817_558e-8, -1.652_694e-4),
                (1.817_558e-8, -1.651_163e-4),
                (1.817_558e-8, -1.649_631e-4),
                (1.817_558e-8, -1.648_51e-4),
                (1.817_558e-8, -1.648_1e-4),
            ],
            s2: [
                (1.817_558e-8, -1.654_225e-4),
                (1.574_051e-8, -1.432_172e-4),
                (9.087_788e-9, -8.261_265e-5),
                (9.797_186e-23, 2.938_374e-8),
                (-9.087_788e-9, 8.250_36e-5),
                (-1.574_051e-8, 1.427_725e-4),
                (-1.817_558e-8, 1.648_1e-4),
            ],
            limited: &["S2 90"],
        },
        WiscombeCase {
            id: 6,
            x: 0.101,
            n: 0.75,
            k: 0.0,
            q_ext: 8.033_542e-6,
            q_sca: 8.033_542e-6,
            g_q_sca: 1.211e-8,
            s1: [
                (2.048_754e-8, -1.756_419e-4),
                (2.048_754e-8, -1.755_965e-4),
                (2.048_753e-8, -1.754_726e-4),
                (2.048_751e-8, -1.753_033e-4),
                (2.048_75e-8, -1.751_341e-4),
                (2.048_749e-8, -1.750_103e-4),
                (2.048_749e-8, -1.749_65e-4),
            ],
            s2: [
                (2.048_754e-8, -1.756_419e-4),
                (1.774_273e-8, -1.520_629e-4),
                (1.024_377e-8, -8.771_198e-5),
                (1.845_057e-15, 3.247_27e-8),
                (-1.024_375e-8, 8.759_147e-5),
                (-1.774_269e-8, 1.515_715e-4),
                (-2.048_749e-8, 1.749_65e-4),
            ],
            limited: &["S2 60", "S2 120"],
        },
        WiscombeCase {
            id: 7,
            x: 10.0,
            n: 0.75,
            k: 0.0,
            q_ext: 2.232_265,
            q_sca: 2.232_265,
            g_q_sca: 2.001_164,
            s1: [
                (5.580_662e1, -9.758_097),
                (-7.672_879, 1.087_317e1),
                (3.587_894, -1.756_177),
                (-1.785_905, -5.232_828e-2),
                (1.537_971, -8.329_374e-2),
                (-4.140_427e-1, 1.876_851e-1),
                (-1.078_568, -3.608_807e-2),
            ],
            s2: [
                (5.580_662e1, -9.758_097),
                (-1.092_923e1, 9.629_667),
                (3.427_411, 8.082_691e-2),
                (-5.148_748e-1, -7.027_288e-1),
                (-6.908_338e-1, 2.152_693e-1),
                (5.247_557e-1, -1.923_391e-1),
                (1.078_568, 3.608_807e-2),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 8,
            x: 1_000.0,
            n: 0.75,
            k: 0.0,
            q_ext: 1.997_908,
            q_sca: 1.997_908,
            g_q_sca: 1.688_121,
            s1: [
                (4.994_77e5, -1.336_502e4),
                (-3.999_296e2, -3.316_361e2),
                (-5.209_852e2, -5.776_614e2),
                (-1.600_887e2, 1.348_013e2),
                (8.431_72e1, -1.209_493e2),
                (-7.556_092e1, -8.134_81e1),
                (1.705_778e1, 4.842_51e2),
            ],
            s2: [
                (4.994_77e5, -1.336_502e4),
                (-3.946_018e2, -1.147_791e2),
                (-1.970_767e2, -6.937_47e2),
                (-4.152_365e1, 1.143e2),
                (-4.261_732e1, 5.535_055e1),
                (4.218_303e1, 9.100_831e1),
                (-1.705_778e1, -4.842_51e2),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 9,
            x: 1.0,
            n: 1.33,
            k: 1.0e-5,
            q_ext: 9.395_198e-2,
            q_sca: 9.392_33e-2,
            g_q_sca: 1.733_048e-2,
            s1: [
                (2.348_8e-2, 2.281_705e-1),
                (2.341_722e-2, 2.217_102e-1),
                (2.322_408e-2, 2.046_815e-1),
                (2.296_081e-2, 1.828_349e-1),
                (2.269_82e-2, 1.625_401e-1),
                (2.250_635e-2, 1.486_17e-1),
                (2.243_622e-2, 1.437_106e-1),
            ],
            s2: [
                (2.348_8e-2, 2.281_705e-1),
                (2.034_83e-2, 1.938_171e-1),
                (1.181_704e-2, 1.075_976e-1),
                (2.729_533e-4, 6.702_879e-3),
                (-1.114_466e-2, -7.646_326e-2),
                (-1.942_3e-2, -1.271_557e-1),
                (-2.243_622e-2, -1.437_106e-1),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 10,
            x: 100.0,
            n: 1.33,
            k: 1.0e-5,
            q_ext: 2.101_321,
            q_sca: 2.096_594,
            g_q_sca: 1.821_854,
            s1: [
                (5.253_302e3, -1.243_188e2),
                (-5.534_573e1, -2.971_881e1),
                (1.710_488e1, -1.520_096e1),
                (-3.655_758, 8.769_86),
                (2.414_318, 5.380_874e-1),
                (-1.222_996, 3.283_917e1),
                (-5.659_205e1, 4.650_974e1),
            ],
            s2: [
                (5.253_302e3, -1.243_188e2),
                (-8.467_204e1, -1.999_47e1),
                (3.310_764e1, -2.709_787),
                (-6.550_512, -4.675_37),
                (6.039_011, -1.169_971e1),
                (-9.653_812, 1.474_455e1),
                (5.659_205e1, -4.650_974e1),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 11,
            x: 10_000.0,
            n: 1.33,
            k: 1.0e-5,
            q_ext: 2.004_089,
            q_sca: 1.723_857,
            g_q_sca: 1.564_987,
            s1: [
                (5.010_222e7, -1.535_815e5),
                (3.786_814e3, -7.654_293e3),
                (-2.731_172e3, 1.326_633e3),
                (-1.061_003e3, -1.930_155e2),
                (-1.058_14e3, 2.298_414e1),
                (-2.748_855e3, 2.298_181e3),
                (-1.821_193e2, -9.519_122e2),
            ],
            s2: [
                (5.010_222e7, -1.535_815e5),
                (5.074_755e3, -7.515_986e3),
                (-3.076_558e3, -1.775_975e2),
                (2.430_92e2, 8.409_836e1),
                (5.906_487e1, -5.370_283e2),
                (-8.036_201e1, -4.939_186),
                (1.821_194e2, 9.519_123e2),
            ],
            limited: &["S1 180", "S2 180"],
        },
        WiscombeCase {
            id: 12,
            x: 0.055,
            n: 1.5,
            k: 1.0,
            q_ext: 1.014_91e-1,
            q_sca: 1.131_687e-5,
            g_q_sca: 5.558_541e-9,
            s1: [
                (7.675_259e-5, 8.343_879e-5),
                (7.674_331e-5, 8.343_495e-5),
                (7.671_794e-5, 8.342_445e-5),
                (7.668_328e-5, 8.341_012e-5),
                (7.664_863e-5, 8.339_578e-5),
                (7.662_326e-5, 8.338_529e-5),
                (7.661_398e-5, 8.338_145e-5),
            ],
            s2: [
                (7.675_259e-5, 8.343_879e-5),
                (6.646_948e-5, 7.225_169e-5),
                (3.838_246e-5, 4.169_695e-5),
                (3.132_066e-8, -2.037_399e-8),
                (-3.830_082e-5, -4.171_317e-5),
                (-6.634_986e-5, -7.221_887e-5),
                (-7.661_398e-5, -8.338_145e-5),
            ],
            limited: &["S2 90"],
        },
        WiscombeCase {
            id: 13,
            x: 0.056,
            n: 1.5,
            k: 1.0,
            q_ext: 1.033_467e-1,
            q_sca: 1.216_311e-5,
            g_q_sca: 6.193_255e-9,
            s1: [
                (8.102_381e-5, 8.807_251e-5),
                (8.101_364e-5, 8.806_83e-5),
                (8.098_587e-5, 8.805_682e-5),
                (8.094_795e-5, 8.804_113e-5),
                (8.091_003e-5, 8.802_545e-5),
                (8.088_228e-5, 8.801_396e-5),
                (8.087_213e-5, 8.800_976e-5),
            ],
            s2: [
                (8.102_381e-5, 8.807_251e-5),
                (7.016_844e-5, 7.626_381e-5),
                (4.051_865e-5, 4.401_169e-5),
                (3.427_277e-8, -2.229_631e-8),
                (-4.042_932e-5, -4.402_945e-5),
                (-7.003_755e-5, -7.622_79e-5),
                (-8.087_213e-5, -8.800_976e-5),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 14,
            x: 1.0,
            n: 1.5,
            k: 1.0,
            q_ext: 2.336_321,
            q_sca: 6.634_538e-1,
            g_q_sca: 1.274_736e-1,
            s1: [
                (5.840_802e-1, 1.905_153e-1),
                (5.657_02e-1, 1.871_997e-1),
                (5.175_251e-1, 1.784_426e-1),
                (4.563_396e-1, 1.671_665e-1),
                (4.002_117e-1, 1.566_427e-1),
                (3.621_572e-1, 1.493_91e-1),
                (3.488_438e-1, 1.468_286e-1),
            ],
            s2: [
                (5.840_802e-1, 1.905_153e-1),
                (5.001_61e-1, 1.456_112e-1),
                (2.879_639e-1, 4.105_398e-2),
                (3.622_847e-2, -6.182_646e-2),
                (-1.748_75e-1, -1.229_586e-1),
                (-3.056_823e-1, -1.438_46e-1),
                (-3.488_438e-1, -1.468_286e-1),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 15,
            x: 100.0,
            n: 1.5,
            k: 1.0,
            q_ext: 2.097_502,
            q_sca: 1.283_697,
            g_q_sca: 1.091_466,
            s1: [
                (5.243_754e3, -2.934_167e2),
                (4.049_055e1, -1.898_456e1),
                (-2.646_835e1, -1.929_564e1),
                (1.268_89e1, 2.397_474e1),
                (5.149_886, 2.290_736e1),
                (-1.605_395e1, 1.418_642e1),
                (-2.029_36e1, 4.384_435),
            ],
            s2: [
                (5.243_754e3, -2.934_167e2),
                (2.019_198e1, 3.110_731),
                (9.152_743, -7.470_202),
                (-1.232_914e1, -7.823_167),
                (-7.173_357, -1.655_464e1),
                (1.448_052e1, -1.393_594e1),
                (2.029_36e1, -4.384_435),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 16,
            x: 10_000.0,
            n: 1.5,
            k: 1.0,
            q_ext: 2.004_368,
            q_sca: 1.236_574,
            g_q_sca: 1.046_525,
            s1: [
                (5.010_919e7, -1.753_404e5),
                (-3.690_394e3, -1.573_897e3),
                (2.391_551e2, 3.247_786e3),
                (-2.607_463e3, 7.414_859e2),
                (-6.183_154e2, 2.264_97e3),
                (-3.368_019e2, 2.115_75e3),
                (-2.184_719e2, -2.064_61e3),
            ],
            s2: [
                (5.010_919e7, -1.753_404e5),
                (-9.333_175e2, -1.839_736e3),
                (-1.202_951e3, -1.899_647e2),
                (1.013_073e3, -1.064_666e3),
                (1.334_826e2, -1.800_859e3),
                (2.293_862e2, -1.996_754e3),
                (2.184_719e2, 2.064_61e3),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 17,
            x: 1.0,
            n: 10.0,
            k: 10.0,
            q_ext: 2.532_993,
            q_sca: 2.049_405,
            g_q_sca: -2.267_961e-1,
            s1: [
                (6.332_483e-1, 4.179_305e-1),
                (6.162_264e-1, 4.597_163e-1),
                (5.736_317e-1, 5.602_514e-1),
                (5.238_628e-1, 6.675_352e-1),
                (4.825_816e-1, 7.434_033e-1),
                (4.570_214e-1, 7.809_867e-1),
                (4.485_464e-1, 7.912_365e-1),
            ],
            s2: [
                (6.332_483e-1, 4.179_305e-1),
                (5.573_186e-1, 2.954_338e-1),
                (3.525_107e-1, -5.921_611e-3),
                (7.881_172e-2, -3.435_544e-1),
                (-1.881_212e-1, -6.028_739e-1),
                (-3.793_898e-1, -7.473_279e-1),
                (-4.485_464e-1, -7.912_365e-1),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 18,
            x: 100.0,
            n: 10.0,
            k: 10.0,
            q_ext: 2.071_124,
            q_sca: 1.836_785,
            g_q_sca: 1.021_648,
            s1: [
                (5.177_811e3, -2.633_811e1),
                (5.227_436e1, -1.270_012e1),
                (-2.705_712e1, -3.951_751e1),
                (1.008_86, 4.663_027e1),
                (-1.505_64e1, 4.333_057e1),
                (-4.510_77e1, 5.199_554),
                (-4.145_383e1, -1.821_808e1),
            ],
            s2: [
                (5.177_811e3, -2.633_811e1),
                (-2.380_252e1, -3.872_567e-1),
                (2.585_821e1, 3.323_624e1),
                (-3.479_935, -4.364_245e1),
                (1.360_634e1, -4.238_302e1),
                (4.474_564e1, -5.452_513),
                (4.145_383e1, 1.821_808e1),
            ],
            limited: &[],
        },
        WiscombeCase {
            id: 19,
            x: 10_000.0,
            n: 10.0,
            k: 10.0,
            q_ext: 2.005_914,
            q_sca: 1.795_393,
            g_q_sca: 9.842_238e-1,
            s1: [
                (5.014_786e7, -1.206_004e5),
                (-4.080_09e3, -2.664_399e3),
                (-1.224_04e3, 4.596_569e3),
                (-4.579_49e3, -8.590_486e2),
                (-3.356_286e3, 3.125_121e3),
                (-3.149_584e3, 3.270_358e3),
                (2.252_48e3, -3.924_468e3),
            ],
            s2: [
                (5.014_786e7, -1.206_004e5),
                (3.351_286e3, 7.291_906e2),
                (4.497_446e2, -4.072_999e3),
                (4.313_394e3, 4.969_719e2),
                (3.171_91e3, -3.129_068e3),
                (3.105_243e3, -3.269_355e3),
                (-2.252_48e3, 3.924_468e3),
            ],
            limited: &[],
        },
    ];

    /// Wiscombe's seven test angles, 0° to 180° in steps of 30° (`MVTstNew.f`).
    const WISCOMBE_ANGLES_DEG: [f64; 7] = [0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0];

    /// The tolerance on `Q_ext`, `Q_sca` and g against Wiscombe's answers: 10⁻⁶ (the plan's, the
    /// client's), absolute at values of order one and relative below, where an absolute 10⁻⁶ would
    /// test nothing.
    const WISCOMBE_TOLERANCE: f64 = 1e-6;

    /// The tolerance on the amplitudes Wiscombe's answers hold to only a few 10⁻⁶, each case's
    /// `limited`.
    const REFERENCE_LIMITED_TOLERANCE: f64 = 5e-6;

    fn index(n: f64, k: f64) -> ComplexIndex {
        ComplexIndex::new(n, k).unwrap()
    }

    /// |a − b| ÷ |b| for complex a and b.
    fn relative_error(a: (f64, f64), b: (f64, f64)) -> f64 {
        math::hypot(a.0 - b.0, a.1 - b.1) / math::hypot(b.0, b.1)
    }

    #[test]
    fn mie_gives_wiscombe_s_efficiencies_and_asymmetry_to_a_millionth() {
        for case in &WISCOMBE_CASES {
            let sphere = efficiencies(case.x, index(case.n, case.k)).unwrap();
            let g = case.g_q_sca / case.q_sca;
            let id = case.id;
            assert!(
                (sphere.q_ext() - case.q_ext).abs() <= WISCOMBE_TOLERANCE * case.q_ext.min(1.0),
                "case {id}: Q_ext {} against {}",
                sphere.q_ext(),
                case.q_ext
            );
            assert!(
                (sphere.q_sca() - case.q_sca).abs() <= WISCOMBE_TOLERANCE * case.q_sca.min(1.0),
                "case {id}: Q_sca {} against {}",
                sphere.q_sca(),
                case.q_sca
            );
            // g, a mean cosine, absolute: a small sphere's g is a higher-order remainder, which
            // Wiscombe's small-particle answers (cases 5, 12) hold to 2 × 10⁻⁹ absolute.
            assert!(
                (sphere.asymmetry() - g).abs() <= WISCOMBE_TOLERANCE,
                "case {id}: g {} against {g}",
                sphere.asymmetry()
            );
        }
    }

    #[test]
    fn mie_gives_wiscombe_s_amplitudes_to_a_millionth_of_each() {
        let mu = WISCOMBE_ANGLES_DEG.map(|deg| math::cos(deg.to_radians()));
        for case in &WISCOMBE_CASES {
            let sphere = amplitudes(case.x, index(case.n, case.k), &mu).unwrap();
            for (j, deg) in WISCOMBE_ANGLES_DEG.iter().enumerate() {
                for (name, computed, reference) in [
                    ("S1", sphere.s1()[j], case.s1[j]),
                    ("S2", sphere.s2()[j], case.s2[j]),
                ] {
                    let label = format!("{name} {deg:.0}");
                    let tolerance = if case.limited.contains(&label.as_str()) {
                        REFERENCE_LIMITED_TOLERANCE
                    } else {
                        WISCOMBE_TOLERANCE
                    };
                    let error = relative_error(computed, reference);
                    assert!(
                        error < tolerance,
                        "case {}: {label}: {computed:?} against {reference:?} ({error:.2e})",
                        case.id
                    );
                }
            }
        }
    }

    /// Bohren and Huffman 1983, Appendix A, p. 482: `QSCA` = `QEXT` = 3.10543 and `QBACK` = 2.92534
    /// at m = 1.55, x = 2π × 0.525 ÷ 0.6328; g 0.63314 is miepython's for the same case
    /// (`test_03_bh_dielectric`), as the client's test takes it. The tolerance is half a unit of
    /// the last printed digit.
    #[test]
    fn mie_gives_bohren_and_huffman_s_sample_to_its_printed_digits() {
        let x = 2.0 * std::f64::consts::PI * 0.525 / 0.6328;
        let m = index(1.55, 0.0);
        let sphere = efficiencies(x, m).unwrap();
        let back = amplitudes(x, m, &[-1.0]).unwrap().s1()[0];
        let q_back = 4.0 * (back.0 * back.0 + back.1 * back.1) / (x * x);
        let half_unit = 5e-6;
        assert!((sphere.q_ext() - 3.105_43).abs() <= half_unit, "{sphere:?}");
        assert!((sphere.q_sca() - 3.105_43).abs() <= half_unit, "{sphere:?}");
        assert!((q_back - 2.925_34).abs() <= half_unit, "{q_back}");
        assert!(
            (sphere.asymmetry() - 0.633_14).abs() <= half_unit,
            "{sphere:?}"
        );
    }

    /// K = (m² − 1) ÷ (m² + 2) for m = n + ik (Bohren and Huffman's sign), as (re, im).
    fn polarisability(n: f64, k: f64) -> (f64, f64) {
        let m2 = Complex::new(n * n - k * k, 2.0 * n * k);
        let ratio = (m2 - Complex::new(1.0, 0.0)) / (m2 + Complex::new(2.0, 0.0));
        (ratio.re, ratio.im)
    }

    /// Bohren and Huffman 1983, §5.1, (5.4) and (5.8): as x → 0, `Q_sca` → (8/3) x⁴ |K|² and
    /// `Q_abs` → 4x Im K, with corrections of order x², which bound each comparison.
    #[test]
    fn mie_meets_the_rayleigh_limit() {
        for (n, k) in [(1.33, 0.0), (1.44, 0.0), (1.5, 0.1), (1.57, 0.53)] {
            for x in [1e-3, 1e-5] {
                let sphere = efficiencies(x, index(n, k)).unwrap();
                let (k_re, k_im) = polarisability(n, k);
                let rayleigh = 8.0 / 3.0 * math::powi(x, 4) * (k_re * k_re + k_im * k_im);
                assert!(
                    (sphere.q_sca() / rayleigh - 1.0).abs() < x * x,
                    "m = {n} + {k}i, x = {x}: Q_sca {} against {rayleigh}",
                    sphere.q_sca()
                );
                if k > 0.0 {
                    let absorption = sphere.q_ext() - sphere.q_sca();
                    assert!(
                        (absorption / (4.0 * x * k_im) - 1.0).abs() < x * x,
                        "m = {n} + {k}i, x = {x}: Q_abs {absorption}"
                    );
                }
                assert!(sphere.asymmetry().abs() < x * x, "{sphere:?}");
            }
        }
    }

    /// The geometric limit, `Q_ext` → 2 (the extinction paradox: a large sphere removes twice its
    /// geometric cross-section, half of it by diffraction; Bohren and Huffman 1983, §4.4). Its
    /// edge term falls as x^(−2/3) (H. M. Nussenzveig and W. J. Wiscombe, "Efficiency factors in
    /// Mie scattering", Phys. Rev. Lett. 45, 1490–1494, 1980), under a ripple that a lossless
    /// sphere keeps at every size. So the mean over 40 sizes 2.5 apart, which spans the ripple, is
    /// held within 3 x^(−2/3) of 2 (the means measured 1.99–2.4 x^(−2/3) above it at x = 10⁴ over
    /// these indices), and each sphere at x = 10⁴ within 0.02 (the ripple measured ±0.013 at most,
    /// for a lossless n = 2.44).
    #[test]
    fn mie_meets_the_geometric_limit() {
        for (n, k) in [
            (0.75, 0.0),
            (1.33, 0.0),
            (1.44, 1e-8),
            (1.5, 0.01),
            (1.5, 1.0),
            (2.44, 0.0),
            (3.18, 3.33),
            (10.0, 10.0),
        ] {
            for x0 in [100.0, 1_000.0, 10_000.0] {
                let values: Vec<f64> = (0..40)
                    .map(|i| {
                        let x = x0 + 2.5 * f64::from(i);
                        efficiencies(x, index(n, k)).unwrap().q_ext()
                    })
                    .collect();
                let mean = values.iter().sum::<f64>() / 40.0;
                let bound = 3.0 * math::powf(x0, -2.0 / 3.0);
                assert!(
                    (mean - 2.0).abs() < bound,
                    "m = {n} + {k}i, x = {x0}: mean Q_ext {mean} against 2 ± {bound}"
                );
                if x0 >= 10_000.0 {
                    for q in values {
                        assert!((q - 2.0).abs() < 0.02, "m = {n} + {k}i, x = {x0}: {q}");
                    }
                }
            }
        }
    }

    /// A lossless sphere absorbs nothing: `Q_ext` = `Q_sca` to 10⁻¹² at every size and index.
    #[test]
    fn mie_absorbs_nothing_when_k_is_zero() {
        for x in [0.1, 1.0, 10.0, 100.0, 1_000.0, 10_000.0] {
            for n in [0.75, 1.0001, 1.33, 1.55, 2.5, 10.0] {
                let sphere = efficiencies(x, index(n, 0.0)).unwrap();
                assert!(
                    (sphere.q_ext() - sphere.q_sca()).abs() < 1e-12,
                    "x = {x}, n = {n}: {sphere:?}"
                );
            }
        }
    }

    /// `N_stop` is Wiscombe's middle branch: 7 at x = 1, 20,111 at the top, and 2 at the bottom.
    #[test]
    fn mie_sums_wiscombe_s_term_count() {
        assert_eq!(term_count(1.0), 7);
        assert_eq!(term_count(100.0), 120);
        assert_eq!(term_count(MAX_SIZE_PARAMETER), 20_111);
        assert_eq!(term_count(MIN_SIZE_PARAMETER), 2);
        assert_eq!(term_count(f64::NAN), 2);
    }

    /// A size parameter outside the range, an index outside (0, 100] + [0, 100]i, and a cosine
    /// outside [−1, 1] are refused.
    #[test]
    fn mie_refuses_what_is_outside_its_range() {
        let water = index(1.33, 0.0);
        for x in [0.0, 5e-7, 20_000.5, f64::NAN, f64::INFINITY] {
            assert!(
                matches!(
                    efficiencies(x, water),
                    Err(ComputeMieError::SizeParameter(_))
                ),
                "{x}"
            );
        }
        for (n, k) in [
            (0.0, 0.0),
            (-1.0, 0.0),
            (1.33, -1e-9),
            (f64::NAN, 0.0),
            (1.33, f64::INFINITY),
            (100.5, 0.0),
            (1.5, 1e3),
        ] {
            assert!(
                matches!(ComplexIndex::new(n, k), Err(ComputeMieError::Index { .. })),
                "{n} + {k}i"
            );
        }
        assert!(matches!(
            amplitudes(1.0, water, &[0.5, 1.000_001]),
            Err(ComputeMieError::Cosine(_))
        ));
    }

    /// The five cases the client's `mieSphere` writes into `mie_cases.json`
    /// (`apps/hyperion/src/renderer/src/view/atmosphere/mieSharedCases.test.ts`): a Rayleigh
    /// sphere, the 75 wt% acid's first resonances, a dust, a soot and a large lossless sphere of
    /// high index. This Mie gives their efficiencies, asymmetry and amplitudes at 0°, 90° and 180°
    /// to 10⁻⁶ of each (absolute for g, as against Wiscombe's answers).
    #[test]
    fn mie_shares_five_cases_with_the_client() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("mie_cases.json")).unwrap();
        let angles: Vec<f64> = fixture["anglesDeg"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_f64().unwrap())
            .collect();
        let mu: Vec<f64> = angles
            .iter()
            .map(|deg| math::cos(deg.to_radians()))
            .collect();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 5);
        let number = |value: &serde_json::Value, field: &str| value[field].as_f64().unwrap();
        let mut worst = 0.0_f64;
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let x = number(case, "sizeParameter");
            let m = index(number(case, "n"), number(case, "k"));
            let sphere = efficiencies(x, m).unwrap();
            for (field, value) in [("qExt", sphere.q_ext()), ("qSca", sphere.q_sca())] {
                let client = number(case, field);
                let miss = (value / client - 1.0).abs();
                worst = worst.max(miss);
                assert!(miss < 1e-6, "{name} {field}: {value} against {client}");
            }
            let g = number(case, "asymmetry");
            assert!(
                (sphere.asymmetry() - g).abs() < 1e-6,
                "{name} g: {sphere:?} against {g}"
            );
            let waves = amplitudes(x, m, &mu).unwrap();
            for (field, computed) in [("s1", waves.s1()), ("s2", waves.s2())] {
                let client = case[field].as_array().unwrap();
                for (j, pair) in client.iter().enumerate() {
                    let reference = (pair[0].as_f64().unwrap(), pair[1].as_f64().unwrap());
                    let miss = relative_error(computed[j], reference);
                    worst = worst.max(miss);
                    assert!(miss < 1e-6, "{name} {field} at {}°: {miss:e}", angles[j]);
                }
            }
        }
        assert!(worst < 1e-6, "{worst:e}");
    }
}
