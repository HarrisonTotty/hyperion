//! Tests of a single Poisson count.

use super::special::{check_mean, count_as_f64};
use super::{poisson_cdf, regularised_gamma_p};

/// The two-sided p-value of observing `observed` from a Poisson variable of the given mean:
/// `min(1, 2 min(P(X ≤ n), P(X ≥ n)))`.
///
/// The upper tail is `P(X ≥ n) = P(n, mean)`, the regularised lower incomplete gamma function,
/// which keeps its precision far out in the tail where `1 − P(X ≤ n − 1)` would round to 0.
///
/// # Panics
///
/// If `mean` is negative or not finite.
#[must_use]
pub fn poisson_two_sided_p(observed: u64, mean: f64) -> f64 {
    check_mean(mean);
    let lower = poisson_cdf(observed, mean);
    let upper = if observed == 0 {
        1.0
    } else {
        regularised_gamma_p(count_as_f64(observed), mean)
    };
    (2.0 * lower.min(upper)).min(1.0)
}

/// The narrowest `[lo, hi]` with `P(X < lo) ≤ α ÷ 2` and `P(X > hi) ≤ α ÷ 2`.
///
/// `lo` is the smallest count whose cumulative probability exceeds α ÷ 2, and `hi` the smallest
/// whose cumulative probability is at least 1 − α ÷ 2. Both are found by bisection over the
/// cumulative distribution, which is monotone in the count.
///
/// # Panics
///
/// If `mean` is negative or not finite, or if `alpha` is not in `(0, 1)`.
///
/// # Examples
///
/// ```
/// use hyperion_testkit::stats::poisson_interval;
///
/// assert_eq!(poisson_interval(100.0, 0.05), (81, 120));
/// ```
#[must_use]
pub fn poisson_interval(mean: f64, alpha: f64) -> (u64, u64) {
    check_mean(mean);
    assert!(
        alpha > 0.0 && alpha < 1.0,
        "alpha = {alpha} must lie in (0, 1)"
    );
    let half = alpha / 2.0;
    let lo = smallest_count(|k| poisson_cdf(k, mean) > half);
    let hi = smallest_count(|k| poisson_cdf(k, mean) >= 1.0 - half);
    (lo, hi)
}

/// The smallest `k` for which a monotone predicate over counts holds.
fn smallest_count(holds: impl Fn(u64) -> bool) -> u64 {
    let mut hi = 1_u64;
    while !holds(hi) {
        hi = hi
            .checked_mul(2)
            .expect("a Poisson count bracket fits in 64 bits");
    }
    let mut lo = 0_u64;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if holds(mid) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

/// Asserts that an observed count lies in [`poisson_interval`]`(mean, alpha)`.
///
/// # Panics
///
/// If the count lies outside the interval, naming the test, the count, the mean and the interval.
#[track_caller]
pub fn assert_poisson_count(name: &str, observed: u64, mean: f64, alpha: f64) {
    let (lo, hi) = poisson_interval(mean, alpha);
    assert!(
        (lo..=hi).contains(&observed),
        "{name}: observed {observed} outside the Poisson interval [{lo}, {hi}] of mean {mean} at alpha = {alpha:e} \
         (two-sided p = {:e})",
        poisson_two_sided_p(observed, mean)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::ALPHA;

    /// From the exact cumulative distribution at a mean of 100, summed in 50-digit decimal
    /// arithmetic: P(X ≤ 80) = 0.022 649, P(X ≤ 81) = 0.029 066, P(X > 119) = 0.028 230,
    /// P(X > 120) = 0.022 669. R's `qpois(c(0.025, 0.975), 100)` gives the same 81 and 120.
    #[test]
    fn interval_at_mean_one_hundred_matches_the_exact_table() {
        assert_eq!(poisson_interval(100.0, 0.05), (81, 120));
    }

    #[test]
    fn interval_at_small_means_starts_at_zero() {
        assert_eq!(poisson_interval(0.01, ALPHA).0, 0);
        assert_eq!(poisson_interval(0.0, ALPHA), (0, 0));
    }

    #[test]
    fn two_sided_p_is_symmetric_in_the_tails_and_capped_at_one() {
        assert!((poisson_two_sided_p(100, 100.0) - 1.0).abs() < 1e-12);
        assert!(poisson_two_sided_p(0, 100.0) < 1e-40);
        assert!((poisson_two_sided_p(0, 0.0) - 1.0).abs() < 1e-15);
        // P(X ≥ 200) at a mean of 100 is 9.343 150 073 × 10⁻¹⁹ (mpmath `gammainc`, 40 digits),
        // far below what 1 − P(X ≤ 199) resolves.
        let far = poisson_two_sided_p(200, 100.0) / 2.0;
        assert!((far / 9.343_150_073e-19 - 1.0).abs() < 1e-9, "{far:e}");
    }

    #[test]
    fn count_inside_the_interval_passes() {
        assert_poisson_count("inside", 81, 100.0, 0.05);
        assert_poisson_count("inside", 120, 100.0, 0.05);
        assert_poisson_count("large mean", 300_000, 300_000.0, ALPHA);
    }

    #[test]
    #[should_panic(
        expected = "outside: observed 121 outside the Poisson interval [81, 120] of mean 100"
    )]
    fn count_outside_the_interval_fails_naming_both() {
        assert_poisson_count("outside", 121, 100.0, 0.05);
    }
}
