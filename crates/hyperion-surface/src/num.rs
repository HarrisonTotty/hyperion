//! Comparisons that are exact at a signed zero, and the finiteness check every height passes.
//!
//! Rust's `f64::min` and `f64::max` may return either input when the two compare equal, such as
//! `+0.0` and `-0.0`, and constant folding orders −0 below +0 while the x86-64 instruction returns
//! the second operand (`f64::max`'s documentation, rustc 1.98.1), so a height compared through
//! them could differ in its sign bit between the server and the client. These fix the sign: −0 is
//! below +0, as IEEE 754-2019's `minimum` and `maximum` order them, until `f64::minimum` and
//! `f64::maximum` are stable (rust-lang/rust issue 91079). A NaN's sign is not portable either,
//! so they refuse one, and every height is asserted finite with [`assert_finite`] before it is
//! compared or emitted (plan R05, Design note 13).

/// The smaller of `a` and `b`, with −0 below +0.
///
/// # Panics
///
/// If either is a NaN.
#[must_use]
#[inline]
pub fn min(a: f64, b: f64) -> f64 {
    assert!(!a.is_nan() && !b.is_nan(), "min of a NaN: {a}, {b}");
    if a < b {
        a
    } else if b < a {
        b
    } else if a.is_sign_negative() {
        // Equal, so both are the same value or a pair of zeros; the negative one is the smaller.
        a
    } else {
        b
    }
}

/// The larger of `a` and `b`, with +0 above −0.
///
/// # Panics
///
/// If either is a NaN.
#[must_use]
#[inline]
pub fn max(a: f64, b: f64) -> f64 {
    assert!(!a.is_nan() && !b.is_nan(), "max of a NaN: {a}, {b}");
    // Equal and a negative zero first: the other is the larger or the same.
    if a > b || (a >= b && !a.is_sign_negative()) {
        a
    } else {
        b
    }
}

/// `x`, once it is known to be finite.
///
/// # Panics
///
/// If `x` is a NaN or infinite: a height that is not finite is a bug in the height function, and
/// its bits would differ between targets.
#[must_use]
#[inline]
#[track_caller]
pub fn assert_finite(x: f64) -> f64 {
    assert!(x.is_finite(), "a height must be finite, got {x}");
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::float::assert_same_bits;
    use std::hint::black_box;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn min_puts_negative_zero_below_positive_zero_both_ways() {
        assert_same_bits(min(0.0, -0.0), -0.0);
        assert_same_bits(min(-0.0, 0.0), -0.0);
        assert_same_bits(min(black_box(0.0), black_box(-0.0)), -0.0);
        assert_same_bits(min(black_box(-0.0), black_box(0.0)), -0.0);
    }

    #[test]
    fn max_puts_positive_zero_above_negative_zero_both_ways() {
        assert_same_bits(max(0.0, -0.0), 0.0);
        assert_same_bits(max(-0.0, 0.0), 0.0);
        assert_same_bits(max(black_box(0.0), black_box(-0.0)), 0.0);
        assert_same_bits(max(black_box(-0.0), black_box(0.0)), 0.0);
    }

    #[test]
    fn min_and_max_order_ordinary_values() {
        assert_same_bits(min(1.5, -2.0), -2.0);
        assert_same_bits(max(1.5, -2.0), 1.5);
        assert_same_bits(min(3.0, 3.0), 3.0);
        assert_same_bits(max(f64::NEG_INFINITY, -1.0), -1.0);
        assert_same_bits(min(black_box(-0.0), black_box(-0.0)), -0.0);
    }

    #[test]
    fn assert_finite_passes_finite_values_through() {
        assert_same_bits(assert_finite(-0.0), -0.0);
        assert_same_bits(assert_finite(1e300), 1e300);
    }
}
