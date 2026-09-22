//! Test support: telling a jump from a steep but continuous stretch.
//!
//! A sweep flags neighbouring samples that differ by more than a coarse bound; each flagged
//! interval is then bisected towards the half with the larger change. A continuous function's
//! change shrinks with the interval, while a jump keeps its size down to the resolution of `f64`.

/// Asserts that `f` has no jump on [`x0`, `x1`].
///
/// # Panics
///
/// If, after bisecting towards the larger change until the interval cannot shrink, the change
/// across it still exceeds `tolerance`.
#[track_caller]
pub(crate) fn assert_no_jump(what: &str, f: impl Fn(f64) -> f64, x0: f64, x1: f64, tolerance: f64) {
    let (mut a, mut b) = (x0, x1);
    let (mut fa, mut fb) = (f(a), f(b));
    for _ in 0..200 {
        let mid = f64::midpoint(a, b);
        if mid <= a || mid >= b {
            break;
        }
        let fm = f(mid);
        if (fm - fa).abs() >= (fb - fm).abs() {
            (b, fb) = (mid, fm);
        } else {
            (a, fa) = (mid, fm);
        }
    }
    assert!(
        (fb - fa).abs() <= tolerance,
        "{what} jumps by {} at {a} (between {x0} and {x1})",
        fb - fa
    );
}

/// Sweeps `f` over `xs` and asserts that it has no jump: neighbours that differ by more than
/// `coarse` are bisected with [`assert_no_jump`].
#[track_caller]
pub(crate) fn assert_continuous_over(
    what: &str,
    f: impl Fn(f64) -> f64,
    xs: &[f64],
    coarse: f64,
    tolerance: f64,
) {
    let values: Vec<f64> = xs.iter().map(|&x| f(x)).collect();
    for (i, pair) in values.windows(2).enumerate() {
        assert!(pair[0].is_finite(), "{what} is not finite at {}", xs[i]);
        if (pair[1] - pair[0]).abs() > coarse {
            assert_no_jump(what, &f, xs[i], xs[i + 1], tolerance);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "a step jumps")]
    fn a_step_is_a_jump() {
        assert_no_jump(
            "a step",
            |x| if x < 0.5 { 0.0 } else { 1.0 },
            0.0,
            1.0,
            1e-6,
        );
    }

    #[test]
    fn a_steep_ramp_is_not() {
        assert_no_jump("a ramp", |x| 1e6 * x, 0.0, 1.0, 1e-6);
        assert_continuous_over("a ramp", |x| 1e3 * x, &[0.0, 0.1, 0.2, 0.3], 1.0, 1e-6);
    }
}
