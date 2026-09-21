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
//! Every function here is `#[inline]` and, for the wrappers, exactly `libm`'s result. The one
//! hand-written function is [`powi`].

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
}
