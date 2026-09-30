//! Pearson's chi-square goodness-of-fit test.

use super::regularised_gamma_q;
use super::special::count_as_f64;

/// Expected count below which a bin is merged with its neighbour.
const MIN_EXPECTED: f64 = 5.0;

/// Relative slack allowed between the observed total and the expected total.
const TOTAL_TOLERANCE: f64 = 1e-6;

/// The result of [`chi_square_gof`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChiSquare {
    /// The statistic, Σ (observed − expected)² ÷ expected, over the merged bins.
    pub statistic: f64,
    /// Degrees of freedom: merged bins − 1.
    pub dof: u32,
    /// The survival probability `Q(dof ÷ 2, statistic ÷ 2)`.
    pub p_value: f64,
    /// How many bins remained after merging.
    pub bins: usize,
}

/// Tests observed counts against expected counts.
///
/// `expected` are counts, not probabilities, and must sum to the observed total to within one part
/// in a million, so a caller with a distribution that runs past the last bin puts the tail's mass
/// into that bin. Bins are first merged so that every expected count is at least 5: from the left
/// end and from the right end inwards, and then any thin bin that remains inside is merged into its
/// right-hand neighbour. The degrees of freedom are the merged bin count less one.
///
/// # Panics
///
/// If the slices differ in length, hold fewer than two bins, contain a negative or non-finite
/// expected count, or if the totals disagree; and if fewer than two bins remain after merging.
///
/// # Examples
///
/// ```
/// use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};
///
/// let observed = [1_650_u64, 1_680, 1_700, 1_640, 1_670, 1_660];
/// let expected = [10_000.0 / 6.0; 6];
/// let fit = chi_square_gof(&observed, &expected);
/// assert_eq!(fit.dof, 5);
/// assert_p_value("fair die", fit.p_value, ALPHA);
/// ```
#[must_use]
pub fn chi_square_gof(observed: &[u64], expected: &[f64]) -> ChiSquare {
    assert_eq!(
        observed.len(),
        expected.len(),
        "observed and expected bin counts differ"
    );
    assert!(
        observed.len() >= 2,
        "a chi-square test needs at least two bins"
    );
    assert!(
        expected.iter().all(|e| e.is_finite() && *e >= 0.0),
        "expected counts must be finite and non-negative: {expected:?}"
    );
    let observed_total = count_as_f64(observed.iter().sum());
    let expected_total: f64 = expected.iter().sum();
    assert!(
        (observed_total - expected_total).abs() <= TOTAL_TOLERANCE * observed_total.max(1.0),
        "expected counts sum to {expected_total} but {observed_total} values were observed"
    );

    let mut bins: Vec<(f64, f64)> = observed
        .iter()
        .zip(expected)
        .map(|(o, e)| (count_as_f64(*o), *e))
        .collect();
    merge_thin_bins(&mut bins);
    assert!(
        bins.len() >= 2,
        "fewer than two bins have an expected count of {MIN_EXPECTED}"
    );

    let statistic: f64 = bins.iter().map(|(o, e)| (o - e) * (o - e) / e).sum();
    let dof = u32::try_from(bins.len() - 1).expect("bin counts are small");
    let p_value = regularised_gamma_q(f64::from(dof) / 2.0, statistic / 2.0);
    ChiSquare {
        statistic,
        dof,
        p_value,
        bins: bins.len(),
    }
}

fn merge_thin_bins(bins: &mut Vec<(f64, f64)>) {
    let merge = |bins: &mut Vec<(f64, f64)>, from: usize, into: usize| {
        let (o, e) = bins[from];
        bins[into].0 += o;
        bins[into].1 += e;
        bins.remove(from);
    };
    while bins.len() > 1 && bins[0].1 < MIN_EXPECTED {
        merge(bins, 0, 1);
    }
    while bins.len() > 1 && bins[bins.len() - 1].1 < MIN_EXPECTED {
        let last = bins.len() - 1;
        merge(bins, last, last - 1);
    }
    let mut i = 0;
    while i + 1 < bins.len() {
        if bins[i].1 < MIN_EXPECTED {
            merge(bins, i, i + 1);
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcg::Lcg;
    use crate::stats::{ALPHA, assert_p_value};

    fn die_counts(n: u64, roll: impl Fn(&mut Lcg) -> u64) -> [u64; 6] {
        let mut g = Lcg::new(0xd1ce);
        let mut counts = [0_u64; 6];
        for _ in 0..n {
            counts[usize::try_from(roll(&mut g)).unwrap()] += 1;
        }
        counts
    }

    fn fair(g: &mut Lcg) -> u64 {
        g.next_below(6)
    }

    /// Face 5 with probability 5 ÷ 25 = 0.2, the others 4 ÷ 25 = 0.16 each.
    fn loaded(g: &mut Lcg) -> u64 {
        let roll = g.next_below(25);
        if roll < 5 { 5 } else { (roll - 5) / 4 }
    }

    #[test]
    fn fair_die_passes_at_ten_to_the_five() {
        let n = 100_000;
        let observed = die_counts(n, fair);
        let expected = [count_as_f64(n) / 6.0; 6];
        let fit = chi_square_gof(&observed, &expected);
        assert_eq!(fit.dof, 5);
        assert_p_value("fair die", fit.p_value, ALPHA);
    }

    #[test]
    fn loaded_die_fails_at_ten_to_the_five() {
        let n = 100_000;
        let observed = die_counts(n, loaded);
        let expected = [count_as_f64(n) / 6.0; 6];
        let fit = chi_square_gof(&observed, &expected);
        assert!(fit.p_value < ALPHA, "a face at 0.2 went unnoticed: {fit:?}");
    }

    #[test]
    #[ignore = "slow: chi-square at N = 10^7"]
    fn fair_die_passes_at_ten_to_the_seven() {
        let n = 10_000_000;
        let observed = die_counts(n, fair);
        let expected = [count_as_f64(n) / 6.0; 6];
        assert_p_value(
            "fair die",
            chi_square_gof(&observed, &expected).p_value,
            ALPHA,
        );
    }

    #[test]
    fn critical_value_reproduces_the_table() {
        // Six equal bins of 100 with a statistic of exactly 11.07 at 5 degrees of freedom.
        let expected = [100.0; 6];
        let fit = chi_square_gof(&[100, 100, 100, 100, 100, 100], &expected);
        assert!(fit.statistic.abs() < 1e-12);
        assert!((fit.p_value - 1.0).abs() < 1e-12);
        assert_eq!(fit.dof, 5);
        let p = regularised_gamma_q(2.5, 11.07 / 2.0);
        assert!((p - 0.0500).abs() < 5e-5, "{p}");
    }

    #[test]
    fn thin_bins_are_merged_from_both_ends_and_inside() {
        let mut bins = vec![
            (1.0, 1.0),
            (2.0, 2.0),
            (30.0, 30.0),
            (3.0, 3.0),
            (20.0, 20.0),
            (1.0, 2.0),
            (0.0, 1.0),
        ];
        merge_thin_bins(&mut bins);
        assert_eq!(bins, vec![(33.0, 33.0), (24.0, 26.0)]);
    }

    #[test]
    fn merging_keeps_the_totals_and_the_degrees_of_freedom_follow() {
        let observed = [3_u64, 3, 60, 40, 2];
        let expected = [3.0, 3.0, 60.0, 40.0, 2.0];
        let fit = chi_square_gof(&observed, &expected);
        assert_eq!(fit.bins, 3);
        assert_eq!(fit.dof, 2);
    }

    #[test]
    #[should_panic(expected = "expected counts sum to")]
    fn mismatched_totals_panic() {
        let _ = chi_square_gof(&[50, 50], &[60.0, 60.0]);
    }
}
