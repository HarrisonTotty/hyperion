//! Test support: telling a jump from a steep but continuous stretch.
//!
//! A sweep flags every interval a jump could hide in: one whose change exceeds a coarse bound, or
//! departs from what its neighbours' changes predict by more than a thousandth of it, which is how
//! a small jump inside a steep stretch shows. Each flagged interval is cut into 32 pieces, so that
//! curvature cannot outweigh a jump, and every piece whose change departs from its neighbours' by
//! more than the tolerance is bisected towards the part whose change departs most from the others,
//! whatever the jump's sign against the slope. A continuous function's change shrinks with the
//! interval, even at a cusp such as (M − M₀)^0.4, while a jump keeps its size down to the
//! resolution of `f64`.

/// The share of the coarse bound by which an interval's change may depart from its neighbours'
/// before it is examined.
const SENSITIVITY: f64 = 1e-3;

/// The pieces a flagged interval is cut into before any bisection: curvature's share of a
/// quarter's change falls with the square of the width, so that a jump, which does not, stands out.
const PIECES: u32 = 32;

/// Halvings over which a continuous function's change must at least halve: 2²⁰ in width, over
/// which a cusp as weak as x^0.1 shrinks to a quarter.
const SHRINK_LEVELS: usize = 20;

/// Each change of `changes` less the change the cubic through its two neighbours on each side
/// predicts, (−Δᵢ₋₂ + 4Δᵢ₋₁ + 4Δᵢ₊₁ − Δᵢ₊₂) ÷ 6, with fewer neighbours nearer the ends; a jump in
/// one change shows here at its full size, curvature hardly at all.
fn residuals(changes: &[f64]) -> Vec<f64> {
    let n = changes.len();
    let at = |i: usize, offset: isize| {
        i.checked_add_signed(offset)
            .filter(|&j| j < n)
            .map(|j| changes[j])
    };
    (0..n)
        .map(|i| {
            let predicted = match (at(i, -2), at(i, -1), at(i, 1), at(i, 2)) {
                (Some(l2), Some(l1), Some(r1), Some(r2)) => (-l2 + 4.0 * l1 + 4.0 * r1 - r2) / 6.0,
                (_, Some(l1), Some(r1), _) => f64::midpoint(l1, r1),
                (Some(l2), Some(l1), None, _) => 2.0 * l1 - l2,
                (_, None, Some(r1), Some(r2)) => 2.0 * r1 - r2,
                (_, Some(l1), None, _) => l1,
                (_, None, Some(r1), None) => r1,
                (_, None, None, _) => 0.0,
            };
            changes[i] - predicted
        })
        .collect()
}

/// Bisects [`a`, `b`] towards the quarter whose change departs most from the median of the four,
/// which is where a jump is, whichever way it points; when the quarters can no longer be told
/// apart it keeps the half with the larger change. Returns the last interval if its change ends
/// above `tolerance` without having halved over the last 2²⁰ of width: a jump there.
fn bisect_for_jump(f: impl Fn(f64) -> f64, a: f64, b: f64, tolerance: f64) -> Option<(f64, f64)> {
    let (mut a, mut b) = (a, b);
    let (mut fa, mut fb) = (f(a), f(b));
    let mut changes = vec![(fb - fa).abs()];
    // The kept half's quarter point is the next midpoint, so each step costs two evaluations.
    let mut known_mid: Option<f64> = None;
    for _ in 0..200 {
        let mid = f64::midpoint(a, b);
        if mid <= a || mid >= b {
            break;
        }
        let fm = known_mid.unwrap_or_else(|| f(mid));
        let (q1, q3) = (f64::midpoint(a, mid), f64::midpoint(mid, b));
        let (left, next) = if a < q1 && q1 < mid && mid < q3 && q3 < b {
            let (f1, f3) = (f(q1), f(q3));
            let quarters = [f1 - fa, fm - f1, f3 - fm, fb - f3];
            let mut sorted = quarters;
            sorted.sort_by(f64::total_cmp);
            let median = f64::midpoint(sorted[1], sorted[2]);
            let outlier = (0..4)
                .max_by(|&i, &j| {
                    (quarters[i] - median)
                        .abs()
                        .total_cmp(&(quarters[j] - median).abs())
                })
                .expect("four quarters");
            if outlier < 2 {
                (true, Some(f1))
            } else {
                (false, Some(f3))
            }
        } else {
            ((fm - fa).abs() >= (fb - fm).abs(), None)
        };
        if left {
            (b, fb) = (mid, fm);
        } else {
            (a, fa) = (mid, fm);
        }
        known_mid = next;
        changes.push((fb - fa).abs());
    }
    let last = changes[changes.len() - 1];
    let earlier = changes[changes.len().saturating_sub(SHRINK_LEVELS + 1)];
    let continuous = last <= tolerance || last < 0.5 * earlier;
    (!continuous).then_some((a, b))
}

/// Where `f` jumps by more than `tolerance` on [`x0`, `x1`], as the pair of neighbouring doubles
/// the jump lies between, or `None`.
///
/// The interval is cut into 32 pieces, and each piece whose change departs by more than
/// `tolerance` from the cubic through its neighbours' changes is bisected down to the resolution
/// of `f64`. A change that ends above `tolerance` and has not halved over the last 2²⁰ of width is
/// a jump; one that shrinks is a continuous stretch, however steep, a kink or a cusp.
pub(crate) fn find_jump(
    f: impl Fn(f64) -> f64,
    x0: f64,
    x1: f64,
    tolerance: f64,
) -> Option<(f64, f64)> {
    let xs: Vec<f64> = (0..=PIECES)
        .map(|i| {
            if i == PIECES {
                x1
            } else {
                x0 + (x1 - x0) * f64::from(i) / f64::from(PIECES)
            }
        })
        .collect();
    let values: Vec<f64> = xs.iter().map(|&x| f(x)).collect();
    if values.iter().any(|v| !v.is_finite()) {
        return Some((x0, x1));
    }
    let changes: Vec<f64> = values.windows(2).map(|p| p[1] - p[0]).collect();
    let departures: Vec<f64> = residuals(&changes).into_iter().map(f64::abs).collect();
    // A jump also shifts its neighbours' predictions, by up to twice its size at the ends, so
    // every piece past the tolerance is bisected, not only the largest.
    (0..departures.len())
        .filter(|&i| departures[i] > tolerance)
        .find_map(|i| bisect_for_jump(&f, xs[i], xs[i + 1], tolerance))
}

/// Asserts that `f` has no jump larger than `tolerance` on [`x0`, `x1`] (see [`find_jump`]).
///
/// # Panics
///
/// If [`find_jump`] finds one.
#[track_caller]
pub(crate) fn assert_no_jump(what: &str, f: impl Fn(f64) -> f64, x0: f64, x1: f64, tolerance: f64) {
    if let Some((a, b)) = find_jump(&f, x0, x1, tolerance) {
        panic!(
            "{what} jumps by {} at {a} (between {x0} and {x1})",
            f(b) - f(a)
        );
    }
}

/// The intervals of a sweep's `values` that a jump could hide in: those whose change exceeds
/// `coarse`, or departs by more than `coarse` ÷ 1,000 from the change its neighbours predict (the
/// cubic of [`residuals`], which curvature alone does not trip), so that on a smooth sweep only
/// the steep intervals, kinks and cusps are examined.
pub(crate) fn suspect_intervals(values: &[f64], coarse: f64) -> Vec<usize> {
    let changes: Vec<f64> = values.windows(2).map(|p| p[1] - p[0]).collect();
    let departures = residuals(&changes);
    (0..changes.len())
        .filter(|&i| {
            changes.len() == 1
                || changes[i].abs() > coarse
                || departures[i].abs() > SENSITIVITY * coarse
        })
        .collect()
}

/// Sweeps `f` over `xs` and asserts that it has no jump: every interval of
/// [`suspect_intervals`] is examined with [`assert_no_jump`].
#[track_caller]
pub(crate) fn assert_continuous_over(
    what: &str,
    f: impl Fn(f64) -> f64,
    xs: &[f64],
    coarse: f64,
    tolerance: f64,
) {
    let values: Vec<f64> = xs.iter().map(|&x| f(x)).collect();
    for (i, v) in values.iter().enumerate() {
        assert!(v.is_finite(), "{what} is not finite at {}", xs[i]);
    }
    for i in suspect_intervals(&values, coarse) {
        assert_no_jump(what, &f, xs[i], xs[i + 1], tolerance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math;

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

    /// A cusp with an infinite slope, (x − x₀)^0.4, its square root and a weaker x^0.1, and a
    /// kink, are continuous, though the cusps change over the last ulp by far more than the
    /// tolerance.
    #[test]
    fn cusps_and_kinks_are_not_jumps() {
        for (name, exponent) in [("x^0.4", 0.4), ("x^0.5", 0.5), ("x^0.1", 0.1)] {
            let cusp = |x: f64| {
                if x <= 0.3 {
                    0.0
                } else {
                    math::powf(x - 0.3, exponent)
                }
            };
            assert_eq!(find_jump(cusp, 0.0, 1.0, 1e-12), None, "{name}");
        }
        let kink = |x: f64| (x - 0.51).abs();
        assert_eq!(find_jump(kink, 0.0, 1.0, 1e-12), None);
        let root = |x: f64| (x - 0.3).abs().sqrt();
        assert_eq!(find_jump(root, 0.0, 1.0, 1e-12), None);
    }

    /// A jump of 10⁻⁴ is found wherever it falls in an interval that bends far more than that, for
    /// either sign against the slope: 0.2 τ²⁰ near τ = 1, where one sweep interval of 0.025 changes
    /// by 0.1 and its curvature per quarter is 3 × 10⁻³, and ±3x².
    #[test]
    fn a_small_jump_is_found_anywhere_in_a_curved_interval() {
        type Curve = fn(f64) -> f64;
        let curves: [(&str, Curve, f64, f64); 3] = [
            ("0.2 τ^20", |x| 0.2 * math::powi(x, 20), 0.975, 1.0),
            ("3x²", |x| 3.0 * x * x, 1.0, 1.02),
            ("−3x²", |x| -3.0 * x * x, 1.0, 1.02),
        ];
        for (name, curve, a, b) in curves {
            assert_eq!(find_jump(curve, a, b, 1e-9), None, "{name} alone");
            for k in 0..64 {
                let at = a + (b - a) * (f64::from(k) + 0.37) / 64.0;
                for jump in [1e-4, -1e-4] {
                    let f = |x: f64| curve(x) + if x > at { jump } else { 0.0 };
                    let found = find_jump(f, a, b, 1e-6);
                    assert!(
                        found.is_some_and(|(lo, hi)| lo <= at && at <= hi),
                        "{name}: a jump of {jump} at {at} is not found ({found:?})"
                    );
                }
            }
        }
    }

    /// In a sweep, a jump of 10⁻⁴ inside a steep, curved stretch, whose every interval changes by
    /// far more, is found whether it points with the slope or against it.
    #[test]
    #[should_panic(expected = "a small jump against the slope jumps")]
    fn a_small_jump_against_a_steep_slope_is_found() {
        let xs: Vec<f64> = (0..=600).map(|i| f64::from(i) / 200.0).collect();
        let f = |x: f64| -3.0 * x * x + if x > 1.2345 { 1e-4 } else { 0.0 };
        assert_continuous_over("a small jump against the slope", f, &xs, 0.05, 1e-6);
    }

    #[test]
    #[should_panic(expected = "a small jump with the slope jumps")]
    fn a_small_jump_with_a_steep_slope_is_found() {
        let xs: Vec<f64> = (0..=600).map(|i| f64::from(i) / 200.0).collect();
        let f = |x: f64| 3.0 * x * x + if x > 2.3456 { 1e-4 } else { 0.0 };
        assert_continuous_over("a small jump with the slope", f, &xs, 0.05, 1e-6);
    }

    #[test]
    fn a_smooth_curve_and_a_kink_pass_the_sweep() {
        let xs: Vec<f64> = (0..=600).map(|i| f64::from(i) / 200.0).collect();
        assert_continuous_over("a parabola", |x| 3.0 * x * x, &xs, 0.05, 1e-9);
        assert_continuous_over("a kink", |x: f64| (x - 1.5).abs(), &xs, 0.05, 1e-9);
    }
}
