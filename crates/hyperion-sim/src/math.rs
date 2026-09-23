//! Transcendental functions over `f64`, every one a thin wrapper of the exactly pinned `libm`.
//!
//! The rule: no code in `hyperion-sim` calls a transcendental method of `f64` or `f32`. It calls
//! the free function here instead. Clippy enforces it through the `disallowed-methods` list in
//! this crate's `clippy.toml`.
//!
//! The reason (brainstorm, "Floating point"): Rust's `+ - * /` and `sqrt` are IEEE-exact and
//! identical on every target, and the compiler never fuses a multiply and an add on its own. `sin`,
//! `cos`, `exp`, `ln`, `powf` and `powi` are not: they call the platform's maths library or have
//! unspecified precision, and differ between operating systems and CPU architectures. A saved
//! universe must survive being moved to another machine, so every such function goes through the
//! pure-Rust `libm`, which is reproducible across platforms though not across its own versions.
//! The version is therefore pinned with `=`, and golden tests pin function values.
//!
//! `f64::sqrt`, `abs`, `floor`, `ceil`, `round`, `trunc`, `min`, `max`, `copysign`, `rem_euclid`
//! and the four operators are exact and are used directly.
//!
//! A fused multiply-add goes through [`mul_add`] here, not `f64::mul_add`. Both are specified as
//! the correctly rounded IEEE 754 `fusedMultiplyAdd`, but where the CPU has no FMA instruction
//! (x86-64 at its baseline, WebAssembly) `f64::mul_add` calls the platform C library's `fma`,
//! which is exactly the dependency this module exists to avoid, and one tier-1 platform's `fma`
//! is known to round wrongly (mingw-w64, rust-lang/rust#140515). [`mul_add`] is `libm`'s pure-Rust
//! `fma`. A multiply and an add are fused only where [`mul_add`] is written: the compiler never
//! fuses them itself.
//!
//! Every function here is `#[inline]` and, for the wrappers, exactly `libm`'s result. The
//! hand-written functions are [`powi`] and [`normal_quantile`], which uses only the wrappers, the
//! four operators and `sqrt`.

/// The sine of `x` radians.
#[inline]
#[must_use]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// The cosine of `x` radians.
#[inline]
#[must_use]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// The sine and cosine of `x` radians, in that order.
#[inline]
#[must_use]
pub fn sin_cos(x: f64) -> (f64, f64) {
    libm::sincos(x)
}

/// The tangent of `x` radians.
#[inline]
#[must_use]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

/// The arcsine of `x`, in radians in `[−π/2, π/2]`; NaN outside `[−1, 1]`.
#[inline]
#[must_use]
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}

/// The arccosine of `x`, in radians in `[0, π]`; NaN outside `[−1, 1]`.
#[inline]
#[must_use]
pub fn acos(x: f64) -> f64 {
    libm::acos(x)
}

/// The arctangent of `x`, in radians in `(−π/2, π/2)`.
#[inline]
#[must_use]
pub fn atan(x: f64) -> f64 {
    libm::atan(x)
}

/// The angle of the point `(x, y)` from the +x axis, in radians in `(−π, π]`.
#[inline]
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// The hyperbolic sine of `x`.
#[inline]
#[must_use]
pub fn sinh(x: f64) -> f64 {
    libm::sinh(x)
}

/// The hyperbolic cosine of `x`.
#[inline]
#[must_use]
pub fn cosh(x: f64) -> f64 {
    libm::cosh(x)
}

/// The hyperbolic tangent of `x`.
#[inline]
#[must_use]
pub fn tanh(x: f64) -> f64 {
    libm::tanh(x)
}

/// The inverse hyperbolic sine of `x`.
#[inline]
#[must_use]
pub fn asinh(x: f64) -> f64 {
    libm::asinh(x)
}

/// The inverse hyperbolic cosine of `x`; NaN below 1.
#[inline]
#[must_use]
pub fn acosh(x: f64) -> f64 {
    libm::acosh(x)
}

/// The inverse hyperbolic tangent of `x`; NaN outside `[−1, 1]`.
#[inline]
#[must_use]
pub fn atanh(x: f64) -> f64 {
    libm::atanh(x)
}

/// e raised to `x`.
#[inline]
#[must_use]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// 2 raised to `x`.
#[inline]
#[must_use]
pub fn exp2(x: f64) -> f64 {
    libm::exp2(x)
}

/// 10 raised to `x`.
#[inline]
#[must_use]
pub fn exp10(x: f64) -> f64 {
    libm::exp10(x)
}

/// `exp(x) − 1`, accurate for small `x`.
#[inline]
#[must_use]
pub fn exp_m1(x: f64) -> f64 {
    libm::expm1(x)
}

/// The natural logarithm of `x`; NaN below 0, `−∞` at 0.
#[inline]
#[must_use]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// The base-2 logarithm of `x`.
#[inline]
#[must_use]
pub fn log2(x: f64) -> f64 {
    libm::log2(x)
}

/// The base-10 logarithm of `x`.
#[inline]
#[must_use]
pub fn log10(x: f64) -> f64 {
    libm::log10(x)
}

/// `ln(1 + x)`, accurate for small `x`.
#[inline]
#[must_use]
pub fn ln_1p(x: f64) -> f64 {
    libm::log1p(x)
}

/// `x` raised to the real power `y`.
#[inline]
#[must_use]
pub fn powf(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// The cube root of `x`.
#[inline]
#[must_use]
pub fn cbrt(x: f64) -> f64 {
    libm::cbrt(x)
}

/// `sqrt(x² + y²)` without intermediate overflow.
#[inline]
#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

/// The error function of `x`.
#[inline]
#[must_use]
pub fn erf(x: f64) -> f64 {
    libm::erf(x)
}

/// The complementary error function, `1 − erf(x)`, accurate in the tails.
#[inline]
#[must_use]
pub fn erfc(x: f64) -> f64 {
    libm::erfc(x)
}

/// The natural logarithm of the absolute value of the gamma function of `x`.
#[inline]
#[must_use]
pub fn ln_gamma(x: f64) -> f64 {
    libm::lgamma(x)
}

/// The gamma function of `x`.
#[inline]
#[must_use]
pub fn gamma(x: f64) -> f64 {
    libm::tgamma(x)
}

/// `x × a + b` with a single rounding: IEEE 754 `fusedMultiplyAdd`, correctly rounded.
///
/// Use it where the exact product must meet the addend before anything is rounded, for instance
/// to recover the rounding error of a product as `mul_add(x, a, −(x × a))`. It is `libm`'s
/// software `fma` (the `arch` feature is off), so it costs about ten times a plain multiply and add
/// (18 ns against 1.6 ns on the development machine, x86-64).
///
/// # Examples
///
/// ```
/// use hyperion_sim::math::mul_add;
///
/// let one_plus_eps = 1.0 + f64::EPSILON;
/// let one_minus_eps = 1.0 - f64::EPSILON;
/// // Exactly 1 − ε², where the unfused product rounds to 1 and loses it.
/// assert_eq!(mul_add(one_plus_eps, one_minus_eps, -1.0), -f64::EPSILON * f64::EPSILON);
/// assert_eq!(one_plus_eps * one_minus_eps - 1.0, 0.0);
/// ```
#[inline]
#[must_use]
pub fn mul_add(x: f64, a: f64, b: f64) -> f64 {
    libm::fma(x, a, b)
}

/// `x` raised to the integer power `n`, by exponentiation by squaring in a fixed order.
///
/// The bits of `|n|` are consumed from the lowest upwards, the accumulator is multiplied by the
/// running square at each set bit, and a negative `n` gives `1 ÷ powi(x, −n)`. Every operation is
/// an IEEE multiplication or division, so the result is the same on every target, which
/// `f64::powi` does not promise. It is not correctly rounded: it makes at most `2 log2(|n|)`
/// roundings, but each squaring doubles the relative error of what it squares, so the result is
/// good to about `|n|` units in the last place, as repeated multiplication is. For a negative `n`
/// whose positive power overflows it returns 0 even where the true value is representable.
///
/// # Examples
///
/// ```
/// use hyperion_sim::math::powi;
///
/// assert_eq!(powi(2.0, 10), 1024.0);
/// assert_eq!(powi(2.0, -2), 0.25);
/// assert_eq!(powi(0.0, 0), 1.0);
/// ```
#[inline]
#[must_use]
pub fn powi(x: f64, n: i32) -> f64 {
    let mut remaining = n.unsigned_abs();
    let mut square = x;
    let mut result = 1.0;
    while remaining != 0 {
        if remaining & 1 == 1 {
            result *= square;
        }
        remaining >>= 1;
        if remaining != 0 {
            square *= square;
        }
    }
    if n < 0 { 1.0 / result } else { result }
}

/// √(2π), the reciprocal of the standard normal density's peak.
const SQRT_TAU: f64 = 2.506_628_274_631_000_5;

/// Acklam's branch point: below it the tail approximation applies, above it the central one.
const ACKLAM_P_LOW: f64 = 0.024_25;

/// Acklam's central numerator, in `r = (p − ½)²`, highest power first.
const ACKLAM_A: [f64; 6] = [
    -3.969_683_028_665_376e1,
    2.209_460_984_245_205e2,
    -2.759_285_104_469_687e2,
    1.383_577_518_672_69e2,
    -3.066_479_806_614_716e1,
    2.506_628_277_459_239,
];

/// Acklam's central denominator, highest power first, without its constant term 1.
const ACKLAM_B: [f64; 5] = [
    -5.447_609_879_822_406e1,
    1.615_858_368_580_409e2,
    -1.556_989_798_598_866e2,
    6.680_131_188_771_972e1,
    -1.328_068_155_288_572e1,
];

/// Acklam's tail numerator, in `q = √(−2 ln p)`, highest power first.
const ACKLAM_C: [f64; 6] = [
    -7.784_894_002_430_293e-3,
    -3.223_964_580_411_365e-1,
    -2.400_758_277_161_838,
    -2.549_732_539_343_734,
    4.374_664_141_464_968,
    2.938_163_982_698_783,
];

/// Acklam's tail denominator, highest power first, without its constant term 1.
const ACKLAM_D: [f64; 4] = [
    7.784_695_709_041_462e-3,
    3.224_671_290_700_398e-1,
    2.445_134_137_142_996,
    3.754_408_661_907_416,
];

/// The polynomial with coefficients `c`, highest power first, at `x`, by Horner's rule.
#[inline]
#[must_use]
fn horner(c: &[f64], x: f64) -> f64 {
    c.iter().fold(0.0, |acc, &k| acc * x + k)
}

/// The quantile of the standard normal distribution: the `x` at which Φ(x) = `p`, for 0 < p < 1.
///
/// Φ⁻¹ turns a uniform deviate into a normal one by inversion, and a probability into the
/// number of standard deviations it lies from the mean. Valid for every `f64` strictly between 0
/// and 1, subnormals included; the result is finite, negative below ½, zero at ½, and increasing
/// to within its rounding error (where neighbouring doubles move the true value by less than an
/// ulp, two of them may map to the same result or, by an ulp or two, out of order).
///
/// The method is P. J. Acklam's, "An algorithm for computing the inverse normal cumulative
/// distribution function" (web note, updated 2003-05-06, never published elsewhere; archived at
/// <https://web.archive.org/web/20031209091847/http://home.online.no/~pjacklam/notes/invnorm/>):
/// a rational approximation in `p − ½` for 0.02425 ≤ p ≤ ½ and one in `√(−2 ln p)` below, whose
/// relative error is below 1.15 × 10⁻⁹ for x ≥ −38 (p ≥ 2.9 × 10⁻³¹⁶), then one step of Halley's
/// method on Φ(x) − p, which Acklam gives as the refinement to full precision. The step's cubic
/// convergence leaves only the rounding error of Φ, so the relative error is below 10⁻¹³ for every
/// normal `p` (a few units in the last place: 3.3 × 10⁻¹⁶ at worst in a sweep against 200-bit
/// values); the tests check it against M. J. Wichura's Algorithm AS 241 (PPND16; J. R. Stat. Soc.
/// C, Applied Statistics 37(3), 477–484, 1988, doi:10.2307/2347330) and by the round trip through
/// Φ. Below 2⁻¹⁰²² Φ is subnormal and the step resolves `p` only to its spacing; where Φ(x₀)
/// rounds to `p` the step vanishes and Acklam's unrefined value remains, 1.8 × 10⁻⁹ from the true
/// quantile at 2⁻¹⁰⁷⁴ and at most 3.9 × 10⁻⁹ in a sweep of subnormal `p` (at 6.5 × 10⁻³¹⁹, against
/// roots found to 45 digits). Three choices of evaluation keep the bound where Acklam's
/// reference form loses it:
///
/// - Above ½ the result is `−normal_quantile(1 − p)`, and `1 − p` is exact there, so the upper
///   tail is as accurate as the lower and `normal_quantile(1 − p) = −normal_quantile(p)` bit for
///   bit whenever `1 − (1 − p) = p`, except at p = ½, where the quantile is +0 and its negation
///   −0.
/// - For 0.25 ≤ p ≤ ½ the step evaluates Φ(x) − p as ½ erf(x ÷ √2) − (p − ½), both terms exact
///   or relatively accurate, so that the result keeps its relative precision as it goes to zero
///   at p = ½. Below 0.25 it uses ½ erfc(−x ÷ √2) − p.
/// - In the tail the density is scaled by p, as (Φ(x) − p) ÷ p × √(2π) exp(x²/2 + ln p), so the
///   step cannot overflow when p is subnormal.
///
/// Every operation is IEEE arithmetic, `sqrt` or a wrapper of the pinned `libm` in a fixed order,
/// so the result is the same on every target.
///
/// The resolution of `p` near 1 is 2⁻⁵³, which caps the upper tail at about 8.2. For a deep
/// upper quantile pass the complement: `−normal_quantile(q)` is the quantile at 1 − q.
///
/// # Panics
///
/// In debug builds, if `p` is not strictly between 0 and 1 (NaN included). Release builds return
/// −∞ for `p ≤ 0`, +∞ for `p ≥ 1` and a NaN `p` unchanged, the same bits on every target. To
/// invert a uniform draw, take it from [`Stream::uniform_open`](crate::rng::Stream::uniform_open),
/// whose range lies inside the domain; [`Stream::uniform`](crate::rng::Stream::uniform) can return
/// 0.
///
/// # Examples
///
/// ```
/// use hyperion_sim::math::normal_quantile;
///
/// // The half-width of the two-sided 95% interval, in standard deviations.
/// let z = normal_quantile(0.975);
/// assert!((z - 1.959_963_984_540_054).abs() < 1e-14);
/// // The quantile is antisymmetric about ½.
/// assert!((normal_quantile(0.025) + z).abs() < 1e-14);
/// // A deviate of mean 10 and standard deviation 2 by inversion of a uniform u, which in the
/// // generator comes from `Stream::uniform_open`.
/// let u = 0.3;
/// let deviate = 10.0 + 2.0 * normal_quantile(u);
/// assert!((deviate - 8.951_198_974_583_918).abs() < 1e-13);
/// ```
#[inline]
#[must_use]
pub fn normal_quantile(p: f64) -> f64 {
    debug_assert!(
        p > 0.0 && p < 1.0,
        "normal_quantile needs 0 < p < 1, got {p}"
    );
    normal_quantile_total(p)
}

/// [`normal_quantile`] without its debug assertion: −∞ for `p ≤ 0`, +∞ for `p ≥ 1`, and a NaN
/// returned unchanged.
///
/// Outside the domain the arithmetic of [`lower_normal_quantile`] would make the hardware's
/// default NaN, whose sign differs between architectures, so these values are fixed instead. A NaN
/// is passed through rather than replaced by [`f64::NAN`], whose bit pattern Rust does not
/// promise across targets.
#[inline]
#[must_use]
fn normal_quantile_total(p: f64) -> f64 {
    if p > 0.5 {
        if p >= 1.0 {
            return f64::INFINITY;
        }
        // Exact for ½ ≤ p ≤ 1 (Sterbenz), so the mirror loses nothing.
        -lower_normal_quantile(1.0 - p)
    } else if p > 0.0 {
        lower_normal_quantile(p)
    } else if p.is_nan() {
        p
    } else {
        f64::NEG_INFINITY
    }
}

/// [`normal_quantile`] for 0 < p ≤ ½.
#[inline]
#[must_use]
fn lower_normal_quantile(p: f64) -> f64 {
    use core::f64::consts::FRAC_1_SQRT_2;
    if p < ACKLAM_P_LOW {
        let ln_p = ln(p);
        let q = (-2.0 * ln_p).sqrt();
        let x0 = horner(&ACKLAM_C, q) / (horner(&ACKLAM_D, q) * q + 1.0);
        // Φ(x0) is within a relative 10⁻⁵ of p, so the difference is exact (Sterbenz).
        let excess = 0.5 * erfc(-x0 * FRAC_1_SQRT_2) - p;
        let step = excess / p * SQRT_TAU * exp(0.5 * x0 * x0 + ln_p);
        halley(x0, step)
    } else {
        let q = p - 0.5;
        let r = q * q;
        let x0 = horner(&ACKLAM_A, r) * q / (horner(&ACKLAM_B, r) * r + 1.0);
        // For p ≥ 0.25, q is exact (Sterbenz) and so is the difference, which stays relatively
        // accurate as x goes to zero; below 0.25, q is rounded and p is the exact target.
        let excess = if p >= 0.25 {
            0.5 * erf(x0 * FRAC_1_SQRT_2) - q
        } else {
            0.5 * erfc(-x0 * FRAC_1_SQRT_2) - p
        };
        let step = excess * SQRT_TAU * exp(0.5 * x0 * x0);
        halley(x0, step)
    }
}

/// One Halley step for Φ(x) = p from `x`, given the Newton step `u = (Φ(x) − p) ÷ φ(x)`.
///
/// With Φ″ = −x φ, Halley's `x − 2ff′ ÷ (2f′² − ff″)` becomes `x − u ÷ (1 + x u ÷ 2)`.
#[inline]
#[must_use]
fn halley(x: f64, u: f64) -> f64 {
    x - u / (1.0 + 0.5 * x * u)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::{assert_same_bits, bits, ulps_apart};

    use super::*;

    /// Reference bits of e, ln 2, sin 1 and cos 1, each the correctly rounded `f64`.
    #[test]
    fn pinned_libm_agrees_with_reference_constants_to_one_ulp() {
        for (name, actual, reference) in [
            ("exp(1)", exp(1.0), 0x4005_BF0A_8B14_5769_u64),
            ("ln(2)", ln(2.0), 0x3FE6_2E42_FEFA_39EF),
            ("sin(1)", sin(1.0), 0x3FEA_ED54_8F09_0CEE),
            ("cos(1)", cos(1.0), 0x3FE1_4A28_0FB5_068C),
        ] {
            let apart = ulps_apart(actual, f64::from_bits(reference));
            assert!(
                apart <= 1,
                "{name} is {apart} ulps from the reference (0x{:016x})",
                bits(actual)
            );
        }
    }

    #[test]
    fn sin_cos_pair_matches_the_separate_functions() {
        for x in [0.0, 0.5, 1.0, 3.0, 1e6] {
            let (s, c) = sin_cos(x);
            assert_same_bits(s, sin(x));
            assert_same_bits(c, cos(x));
        }
    }

    /// The rounding error of a product comes back exactly, and a zero keeps the exact sign.
    #[test]
    fn mul_add_rounds_once() {
        let x = 0.1;
        let product = x * 1e9;
        let error = mul_add(x, 1e9, -product);
        assert!(error != 0.0 && error.abs() <= 0.5 * (product.next_up() - product));
        assert_same_bits(mul_add(2.0, 3.0, 4.0), 10.0);
        assert_same_bits(mul_add(-0.0, 1.0, 0.0), 0.0);
        assert_same_bits(mul_add(f64::MIN_POSITIVE, -f64::MIN_POSITIVE, 0.0), -0.0);
    }

    /// Both are the correctly rounded `fusedMultiplyAdd`, so on a platform whose own `fma` is
    /// correct they agree bit for bit; this pins `libm`'s against the host's.
    #[test]
    fn mul_add_agrees_with_the_hosts_fused_multiply_add() {
        let mut g = hyperion_testkit::lcg::Lcg::new(0xf3a);
        let draw = |g: &mut hyperion_testkit::lcg::Lcg| {
            let scale = powi(2.0, i32::try_from(g.next_below(120)).unwrap() - 60);
            (2.0 * g.next_f64() - 1.0) * scale
        };
        for _ in 0..100_000 {
            let (x, a, b) = (draw(&mut g), draw(&mut g), draw(&mut g));
            #[expect(
                clippy::disallowed_methods,
                reason = "the host's fma is the reference this test compares against"
            )]
            let host = x.mul_add(a, b);
            assert_same_bits(mul_add(x, a, b), host);
        }
    }

    /// For bases whose small powers are exact the two orders of multiplication cannot differ.
    #[test]
    fn powi_equals_repeated_multiplication_bit_for_bit_when_exact() {
        for x in [2.0, 3.0, -1.5, 0.5, 10.0, 0.25, -7.0] {
            let mut product = 1.0;
            for n in 0..=8 {
                assert_same_bits(powi(x, n), product);
                product *= x;
            }
        }
    }

    /// The fixed order: the accumulator takes the running square at each set bit of `n`, lowest
    /// bit first, so the result is pinned bit for bit for any base.
    #[test]
    fn powi_multiplies_in_its_fixed_order() {
        for x in [1.1, 0.7, -2.3, 1e-3, 123.456, 3.0] {
            let x2 = x * x;
            let x4 = x2 * x2;
            let x8 = x4 * x4;
            let expected = [1.0, x, x2, x * x2, x4, x * x4, x2 * x4, (x * x2) * x4, x8];
            for (n, value) in (0..).zip(expected) {
                assert_same_bits(powi(x, n), value);
            }
        }
    }

    /// For a general base the order of multiplication matters: squaring and repeated
    /// multiplication round differently from x⁴ on, by at most a few ulps.
    #[test]
    fn powi_matches_repeated_multiplication_to_a_few_ulps() {
        for x in [1.1, 0.7, -2.3, 1e-3, 123.456] {
            let mut product = 1.0;
            for n in 0..=8 {
                assert!(ulps_apart(powi(x, n), product) <= 4, "powi({x}, {n})");
                product *= x;
            }
        }
    }

    #[test]
    fn negative_powers_are_reciprocals() {
        for x in [2.0, 1.1, -0.3, 10.0] {
            for n in 1..=12 {
                assert_same_bits(powi(x, -n), 1.0 / powi(x, n));
            }
        }
        assert_same_bits(powi(1.0, i32::MIN), 1.0);
        assert_same_bits(powi(2.0, i32::MIN), 0.0);
    }

    #[test]
    fn powi_at_the_edge_of_the_exponent_range() {
        assert!(powi(2.0, 1023).is_finite());
        assert_same_bits(powi(2.0, 1023), f64::MAX / (2.0 - f64::EPSILON));
        assert!(powi(2.0, 1024).is_infinite());
        assert_same_bits(powi(2.0, -1022), f64::MIN_POSITIVE);
        // 2⁻¹⁰²³ is subnormal and exact; 2⁻¹⁰²⁴ is too, but the positive power overflows first.
        assert_same_bits(powi(2.0, -1023), f64::MIN_POSITIVE / 2.0);
        assert_same_bits(powi(2.0, -1024), 0.0);
    }

    /// M. J. Wichura, "Algorithm AS 241: The percentage points of the normal distribution",
    /// Applied Statistics 37, 477–484 (1988), function PPND16: the published reference the
    /// quantile is checked against, relative error about 10⁻¹⁶ (7 × 10⁻¹⁶ at worst against 200-bit
    /// values in 3,000 points when this was transcribed). The coefficients are Wichura's, lowest
    /// power first.
    #[expect(
        clippy::excessive_precision,
        reason = "Wichura's published 20-digit coefficients, kept verbatim for checking"
    )]
    fn wichura_as_241(p: f64) -> f64 {
        const A: [f64; 8] = [
            3.387_132_872_796_366_608_0,
            1.331_416_678_917_843_774_5e2,
            1.971_590_950_306_551_442_7e3,
            1.373_169_376_550_946_112_5e4,
            4.592_195_393_154_987_145_7e4,
            6.726_577_092_700_870_085_3e4,
            3.343_057_558_358_812_810_5e4,
            2.509_080_928_730_122_672_7e3,
        ];
        const B: [f64; 8] = [
            1.0,
            4.231_333_070_160_091_125_2e1,
            6.871_870_074_920_579_083_0e2,
            5.394_196_021_424_751_107_7e3,
            2.121_379_430_158_659_586_7e4,
            3.930_789_580_009_271_061_0e4,
            2.872_908_573_572_194_267_4e4,
            5.226_495_278_852_854_561_0e3,
        ];
        const C: [f64; 8] = [
            1.423_437_110_749_683_577_34,
            4.630_337_846_156_545_295_90,
            5.769_497_221_460_691_405_50,
            3.647_848_324_763_204_605_04,
            1.270_458_252_452_368_382_58,
            2.417_807_251_774_506_117_70e-1,
            2.272_384_498_926_918_458_33e-2,
            7.745_450_142_783_414_076_40e-4,
        ];
        const D: [f64; 8] = [
            1.0,
            2.053_191_626_637_758_821_87,
            1.676_384_830_183_803_849_40,
            6.897_673_349_851_000_045_50e-1,
            1.481_039_764_274_800_745_90e-1,
            1.519_866_656_361_645_719_66e-2,
            5.475_938_084_995_344_946_00e-4,
            1.050_750_071_644_416_843_24e-9,
        ];
        const E: [f64; 8] = [
            6.657_904_643_501_103_777_20,
            5.463_784_911_164_114_369_90,
            1.784_826_539_917_291_335_80,
            2.965_605_718_285_048_912_30e-1,
            2.653_218_952_657_612_309_30e-2,
            1.242_660_947_388_078_438_60e-3,
            2.711_555_568_743_487_578_15e-5,
            2.010_334_399_292_288_132_65e-7,
        ];
        const F: [f64; 8] = [
            1.0,
            5.998_322_065_558_879_376_90e-1,
            1.369_298_809_227_358_053_10e-1,
            1.487_536_129_085_061_485_25e-2,
            7.868_691_311_456_132_591_00e-4,
            1.846_318_317_510_054_681_80e-5,
            1.421_511_758_316_445_888_70e-7,
            2.044_263_103_389_939_785_64e-15,
        ];
        let rational = |num: &[f64; 8], den: &[f64; 8], r: f64| {
            num.iter().rev().fold(0.0, |acc, &k| acc * r + k)
                / den.iter().rev().fold(0.0, |acc, &k| acc * r + k)
        };
        let q = p - 0.5;
        if q.abs() <= 0.425 {
            return q * rational(&A, &B, 0.180_625 - q * q);
        }
        let r = (-ln(if q < 0.0 { p } else { 1.0 - p })).sqrt();
        let magnitude = if r <= 5.0 {
            rational(&C, &D, r - 1.6)
        } else {
            rational(&E, &F, r - 5.0)
        };
        if q < 0.0 { -magnitude } else { magnitude }
    }

    /// 1,000 probabilities: 400 evenly spaced over (0, 1), 300 spaced evenly in log₂ through the
    /// lower tail from 2⁻² to 2⁻⁸⁰⁰, and 300 through the upper tail from 1 − 2⁻² to 1 − 2⁻⁵³,
    /// where the resolution of `f64` ends. Both of Acklam's branch points and the switch from erfc
    /// to erf at 0.25 lie inside the even run.
    fn quantile_points() -> impl Iterator<Item = f64> {
        let even = (0..400_u32).map(|k| (f64::from(k) + 0.5) / 400.0);
        let lower = (0..300_u32).map(|k| exp2(-2.0 - f64::from(k) * (798.0 / 299.0)));
        let upper = (0..300_u32).map(|k| 1.0 - exp2(-2.0 - f64::from(k) * (51.0 / 299.0)));
        even.chain(lower).chain(upper)
    }

    /// Φ(Φ⁻¹(p)) = p to 10⁻¹² relative. Above ½ the complement is checked as well, since 1 − p is
    /// exact there and Φ(x) near 1 would hide the upper tail's relative error.
    #[test]
    fn normal_quantile_round_trips_through_the_distribution_function() {
        use hyperion_testkit::stats::normal_cdf;
        for p in quantile_points() {
            let x = normal_quantile(p);
            let back = normal_cdf(x);
            assert!(
                (back - p).abs() <= 1e-12 * p,
                "Φ(Φ⁻¹({p:?})) = {back:?}, x = {x:?}"
            );
            if p >= 0.5 {
                let (tail, complement) = (normal_cdf(-x), 1.0 - p);
                assert!(
                    (tail - complement).abs() <= 1e-12 * complement,
                    "1 − Φ(Φ⁻¹({p:?})) = {tail:?}, not {complement:?}"
                );
            }
        }
    }

    /// Wichura's AS 241 and this function agree to the relative 10⁻¹³ the method promises, also
    /// just beside ½, where the quantile goes to zero.
    #[test]
    fn normal_quantile_agrees_with_wichura_as_241() {
        let near_half = [
            0.5 + exp2(-40.0),
            0.5 - exp2(-41.0),
            0.5 + exp2(-20.0),
            0.5 - exp2(-30.0),
        ];
        for p in quantile_points().chain(near_half) {
            let (x, reference) = (normal_quantile(p), wichura_as_241(p));
            assert!(
                (x - reference).abs() <= 1e-13 * reference.abs(),
                "Φ⁻¹({p:?}) = {x:?}, AS 241 gives {reference:?}"
            );
        }
    }

    /// Where 1 − p is exact the two quantiles are each other's negatives, and ½ maps to zero.
    #[test]
    fn normal_quantile_is_antisymmetric_about_one_half() {
        assert_same_bits(normal_quantile(0.5), 0.0);
        for p in quantile_points().filter(|&p| p >= 0.5) {
            let (upper, lower) = (normal_quantile(p), normal_quantile(1.0 - p));
            assert!(
                (upper + lower).abs() <= 1e-12 * upper.abs(),
                "Φ⁻¹({p:?}) = {upper:?} but Φ⁻¹(1 − p) = {lower:?}"
            );
            // Stronger than the plan asks: the upper half is the lower half's exact mirror.
            assert_same_bits(upper, -lower);
        }
    }

    /// Acklam's two approximations differ by up to 10⁻⁹ relative where they meet, and the Halley
    /// step must leave no step behind. Steps of 2⁻⁵⁰ in p move the quantile by at least 25 units
    /// in the last place at these points, clear of its rounding error (adjacent doubles move it by
    /// less than one, so it is only monotone to within that error).
    #[test]
    fn normal_quantile_increases_strictly_across_its_branch_points() {
        let step = exp2(-50.0);
        for branch in [ACKLAM_P_LOW, 0.25, 0.5, 1.0 - ACKLAM_P_LOW] {
            let values: Vec<f64> = (-64..=64)
                .map(|k| normal_quantile(branch + f64::from(k) * step))
                .collect();
            assert!(
                values.windows(2).all(|pair| pair[0] < pair[1]),
                "not strictly increasing around p = {branch:?}: {values:?}"
            );
        }
    }

    /// The tail step is scaled by p and cannot overflow: the smallest normal and the smallest
    /// subnormal p give finite quantiles, the first to full precision and the second to Acklam's
    /// own precision, since Φ is too coarse there for the step to act. References by Newton's
    /// method at 200 bits with mpmath 1.4.1, iterated until the step is below 2⁻¹⁹⁰ relative.
    #[test]
    fn normal_quantile_is_finite_down_to_the_smallest_subnormal() {
        for (p, reference, tolerance) in [
            (f64::MIN_POSITIVE, -37.519_379_347_144_5, 1e-13),
            (5e-324, -38.467_405_617_144_344, 1e-8),
        ] {
            let x = normal_quantile(p);
            assert!(
                (x - reference).abs() <= tolerance * reference.abs(),
                "Φ⁻¹({p:?}) = {x:?}, not {reference:?}"
            );
        }
    }

    /// What release builds return outside (0, 1), where the debug assertion does not run: fixed
    /// bits, not the hardware's default NaN.
    #[test]
    fn normal_quantile_is_total_outside_its_domain() {
        for (p, expected) in [
            (0.0, f64::NEG_INFINITY),
            (-0.0, f64::NEG_INFINITY),
            (-1.0, f64::NEG_INFINITY),
            (f64::NEG_INFINITY, f64::NEG_INFINITY),
            (1.0, f64::INFINITY),
            (2.0, f64::INFINITY),
            (f64::INFINITY, f64::INFINITY),
        ] {
            assert_same_bits(normal_quantile_total(p), expected);
        }
        // A NaN comes back as it went in; its sign is the one bit a test can see without its bits.
        for nan in [f64::NAN, -f64::NAN] {
            let result = normal_quantile_total(nan);
            assert!(result.is_nan());
            assert_eq!(result.is_sign_negative(), nan.is_sign_negative());
        }
        for p in quantile_points() {
            assert_same_bits(normal_quantile_total(p), normal_quantile(p));
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "normal_quantile needs 0 < p < 1, got 1")]
    fn normal_quantile_rejects_one_in_debug_builds() {
        let _ = normal_quantile(1.0);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "normal_quantile needs 0 < p < 1, got 0")]
    fn normal_quantile_rejects_zero_in_debug_builds() {
        let _ = normal_quantile(0.0);
    }
}
