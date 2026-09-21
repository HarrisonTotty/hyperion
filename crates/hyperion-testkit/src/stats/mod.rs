//! Hand-written statistical tests, which is how "realistic" becomes a test and not an opinion.
//!
//! Every test that uses these helpers runs on fixed seeds, so it is deterministic: it cannot flake,
//! it can only start failing when generated output changes. The shared significance level is
//! [`ALPHA`]. When a deliberate generator-version bump trips a test, run it under three other
//! seeds. Two failures out of three is a real defect. Otherwise change the seed in the same commit
//! and say so.
//!
//! The special functions go through the pinned `libm`, so a p-value is the same on every platform.

mod chi_square;
mod ks;
mod poisson;
mod special;

pub use chi_square::{ChiSquare, chi_square_gof};
pub use ks::{KolmogorovSmirnov, kolmogorov_survival, ks_one_sample, ks_two_sample};
pub use poisson::{assert_poisson_count, poisson_interval, poisson_two_sided_p};
pub use special::{normal_cdf, poisson_cdf, poisson_pmf, regularised_gamma_p, regularised_gamma_q};

/// The significance level every statistical test uses: a true hypothesis fails once in a thousand.
pub const ALPHA: f64 = 1e-3;

/// Asserts that a test's p-value is not below `alpha`.
///
/// # Panics
///
/// If `p < alpha`, or if `p` is not a probability, naming the test.
#[track_caller]
pub fn assert_p_value(name: &str, p: f64, alpha: f64) {
    assert!(
        (0.0..=1.0).contains(&p),
        "{name}: p-value {p} is not a probability"
    );
    assert!(
        p >= alpha,
        "{name}: p-value {p:e} is below alpha = {alpha:e}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_value_at_or_above_alpha_passes() {
        assert_p_value("edge", ALPHA, ALPHA);
        assert_p_value("comfortable", 0.4, ALPHA);
    }

    #[test]
    #[should_panic(expected = "loaded: p-value 1e-4 is below alpha")]
    fn p_value_below_alpha_fails_with_the_name() {
        assert_p_value("loaded", 1e-4, ALPHA);
    }

    #[test]
    #[should_panic(expected = "is not a probability")]
    fn nan_p_value_fails() {
        assert_p_value("broken", f64::NAN, ALPHA);
    }
}
