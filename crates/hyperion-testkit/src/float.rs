//! Bit-exact comparison of floats, for tests.
//!
//! `hyperion-sim` forbids `f64::to_bits` in its own code so that float bits can never be hashed
//! into a stream key. Its tests still need to say "these two values are the same bits", and they
//! say it through this module.

/// The IEEE 754 bit pattern of `value`.
///
/// # Panics
///
/// If `value` is a NaN, whose bits are unspecified.
#[must_use]
pub fn bits(value: f64) -> u64 {
    assert!(!value.is_nan(), "the bits of a NaN are unspecified");
    value.to_bits()
}

/// The IEEE 754 bit pattern of the single-precision `value`, for the goldens of `f32` arrays a
/// GPU reads (plan R05, T4.b).
///
/// # Panics
///
/// If `value` is a NaN, whose bits are unspecified.
#[must_use]
pub fn bits_f32(value: f32) -> u32 {
    assert!(!value.is_nan(), "the bits of a NaN are unspecified");
    value.to_bits()
}

/// How many representable values lie between `a` and `b`: 0 when they are the same bits.
///
/// Both values must be finite and of the same sign (or zero), which is all a last-place comparison
/// of two results needs.
///
/// # Panics
///
/// If either value is not finite, or if the two have opposite signs.
#[must_use]
pub fn ulps_apart(a: f64, b: f64) -> u64 {
    assert!(
        a.is_finite() && b.is_finite(),
        "ulps_apart takes finite values, got {a:?} and {b:?}"
    );
    let (a, b) = (a + 0.0, b + 0.0);
    assert!(
        a.is_sign_negative() == b.is_sign_negative() || a == 0.0 || b == 0.0,
        "ulps_apart takes values of one sign, got {a:?} and {b:?}"
    );
    (a.abs().to_bits()).abs_diff(b.abs().to_bits())
}

/// Asserts that `actual` and `expected` are the same bits, printing both as bits and decimals.
///
/// # Panics
///
/// If the two differ in any bit, or if either is a NaN.
#[track_caller]
pub fn assert_same_bits(actual: f64, expected: f64) {
    let (a, e) = (bits(actual), bits(expected));
    assert!(
        a == e,
        "float bits differ\n  actual:   0x{a:016x}  ({actual:?})\n  expected: 0x{e:016x}  ({expected:?})"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn same_bits_pass_and_neighbours_are_one_ulp_apart() {
        assert_same_bits(0.1 + 0.2, 0.300_000_000_000_000_04);
        assert_eq!(ulps_apart(1.0, 1.0_f64.next_up()), 1);
        assert_eq!(ulps_apart(-1.0, -1.0_f64.next_up()), 1);
        assert_eq!(ulps_apart(2.5, 2.5), 0);
    }
}
