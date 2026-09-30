//! Kolmogorov–Smirnov tests.

use super::special::count_as_f64;

/// The result of a Kolmogorov–Smirnov test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KolmogorovSmirnov {
    /// The largest distance between the two cumulative distributions.
    pub statistic: f64,
    /// The effective sample size: `n` for one sample, `n₁ n₂ ÷ (n₁ + n₂)` for two.
    pub effective_n: f64,
    /// The probability of a distance at least this large under the null hypothesis.
    pub p_value: f64,
}

/// The Kolmogorov survival function, `Q_KS(λ) = 2 Σ_{j≥1} (−1)^(j−1) exp(−2 j² λ²)`.
///
/// Summed until a term falls below 10⁻¹². `Q_KS(0) = 1`, and the result is clamped to `[0, 1]`.
///
/// # Panics
///
/// If `lambda` is negative or NaN.
#[must_use]
pub fn kolmogorov_survival(lambda: f64) -> f64 {
    assert!(lambda >= 0.0, "λ = {lambda} must be non-negative");
    if lambda == 0.0 {
        return 1.0;
    }
    let mut sum = 0.0;
    let mut sign = 1.0;
    let mut j = 1.0;
    // Terms fall below 10⁻¹² once 2 j² λ² > 27.6; for λ = 0.01 that is j ≈ 372.
    loop {
        let term = libm::exp(-2.0 * j * j * lambda * lambda);
        sum += sign * term;
        if term < 1e-12 {
            break;
        }
        sign = -sign;
        j += 1.0;
    }
    (2.0 * sum).clamp(0.0, 1.0)
}

/// Stephens's finite-sample correction: λ = (√n + 0.12 + 0.11 ÷ √n) D.
fn stephens_lambda(effective_n: f64, d: f64) -> f64 {
    let root = effective_n.sqrt();
    (root + 0.12 + 0.11 / root) * d
}

/// Tests a sample against a cumulative distribution function.
///
/// `samples` is sorted in place with `total_cmp`. D is the largest distance between the empirical
/// distribution and `cdf`, and the p-value comes from [`kolmogorov_survival`] at Stephens's λ.
///
/// # Panics
///
/// If `samples` is empty or holds a NaN, or if `cdf` returns a value outside `[0, 1]` or a NaN.
///
/// # Examples
///
/// ```
/// use hyperion_testkit::lcg::Lcg;
/// use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};
///
/// let mut g = Lcg::new(3);
/// let mut sample: Vec<f64> = (0..10_000).map(|_| g.next_f64()).collect();
/// let ks = ks_one_sample(&mut sample, |x| x.clamp(0.0, 1.0));
/// assert_p_value("uniform", ks.p_value, ALPHA);
/// ```
#[must_use]
pub fn ks_one_sample(samples: &mut [f64], cdf: impl Fn(f64) -> f64) -> KolmogorovSmirnov {
    assert!(!samples.is_empty(), "a KS test needs a sample");
    assert_no_nan(samples);
    samples.sort_by(f64::total_cmp);
    let n = count_as_f64(u64::try_from(samples.len()).expect("a slice length fits in 64 bits"));
    let mut d: f64 = 0.0;
    for (i, x) in samples.iter().enumerate() {
        let rank = count_as_f64(u64::try_from(i).expect("an index fits in 64 bits"));
        let f = cdf(*x);
        assert!(
            (0.0..=1.0).contains(&f),
            "cdf({x}) = {f} is not a probability"
        );
        d = d
            .max(((rank + 1.0) / n - f).abs())
            .max((f - rank / n).abs());
    }
    KolmogorovSmirnov {
        statistic: d,
        effective_n: n,
        p_value: kolmogorov_survival(stephens_lambda(n, d)),
    }
}

/// Tests whether two samples come from one distribution.
///
/// Both slices are sorted in place. The effective sample size is `n₁ n₂ ÷ (n₁ + n₂)`.
///
/// # Panics
///
/// If either sample is empty or holds a NaN.
#[must_use]
pub fn ks_two_sample(a: &mut [f64], b: &mut [f64]) -> KolmogorovSmirnov {
    assert!(
        !a.is_empty() && !b.is_empty(),
        "a two-sample KS test needs two samples"
    );
    // A NaN in both samples would stop the merge below from advancing.
    assert_no_nan(a);
    assert_no_nan(b);
    a.sort_by(f64::total_cmp);
    b.sort_by(f64::total_cmp);
    let na = count_as_f64(u64::try_from(a.len()).expect("a slice length fits in 64 bits"));
    let nb = count_as_f64(u64::try_from(b.len()).expect("a slice length fits in 64 bits"));
    let (mut seen_a, mut seen_b) = (0_usize, 0_usize);
    let mut distance: f64 = 0.0;
    while seen_a < a.len() && seen_b < b.len() {
        let step = a[seen_a].min(b[seen_b]);
        while seen_a < a.len() && a[seen_a] <= step {
            seen_a += 1;
        }
        while seen_b < b.len() && b[seen_b] <= step {
            seen_b += 1;
        }
        let fraction_a =
            count_as_f64(u64::try_from(seen_a).expect("an index fits in 64 bits")) / na;
        let fraction_b =
            count_as_f64(u64::try_from(seen_b).expect("an index fits in 64 bits")) / nb;
        distance = distance.max((fraction_a - fraction_b).abs());
    }
    let effective_n = na * nb / (na + nb);
    KolmogorovSmirnov {
        statistic: distance,
        effective_n,
        p_value: kolmogorov_survival(stephens_lambda(effective_n, distance)),
    }
}

/// Panics, naming the position, if a sample holds a NaN, which has no place in an ordering.
fn assert_no_nan(samples: &[f64]) {
    if let Some(i) = samples.iter().position(|x| x.is_nan()) {
        panic!("sample {i} is NaN: a KS test needs ordered values");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcg::Lcg;
    use crate::stats::{ALPHA, assert_p_value};

    /// `Q_KS(1.36) = 0.049 486`: 1.36 is the 5% critical value of the asymptotic distribution
    /// (Numerical Recipes §14.3; Massey 1951 table for large n gives 1.36 ÷ √n).
    #[test]
    fn critical_lambda_gives_five_per_cent() {
        assert!((kolmogorov_survival(1.36) - 0.049_485_877).abs() < 1e-8);
        assert!((kolmogorov_survival(0.0) - 1.0).abs() < 1e-15);
        assert!((kolmogorov_survival(0.05) - 1.0).abs() < 1e-9);
    }

    fn uniform_sample(seed: u64, n: usize) -> Vec<f64> {
        let mut g = Lcg::new(seed);
        (0..n).map(|_| g.next_f64()).collect()
    }

    #[test]
    fn uniform_sample_passes_against_the_uniform_cdf() {
        let mut sample = uniform_sample(0x5a3e, 50_000);
        let ks = ks_one_sample(&mut sample, |x| x.clamp(0.0, 1.0));
        assert_p_value("uniform", ks.p_value, ALPHA);
    }

    #[test]
    fn squared_sample_fails_against_the_uniform_cdf() {
        let mut sample: Vec<f64> = uniform_sample(0x5a3e, 50_000)
            .into_iter()
            .map(|x| x * x)
            .collect();
        let ks = ks_one_sample(&mut sample, |x| x.clamp(0.0, 1.0));
        assert!(
            ks.p_value < ALPHA,
            "a squared uniform went unnoticed: {ks:?}"
        );
        let ks = ks_one_sample(&mut sample, |x| x.clamp(0.0, 1.0).sqrt());
        assert_p_value("squared uniform against its own cdf", ks.p_value, ALPHA);
    }

    #[test]
    fn two_samples_of_one_distribution_pass_and_of_two_fail() {
        let mut a = uniform_sample(1, 20_000);
        let mut b = uniform_sample(2, 30_000);
        let ks = ks_two_sample(&mut a, &mut b);
        assert!((ks.effective_n - 12_000.0).abs() < 1e-9);
        assert_p_value("two uniform samples", ks.p_value, ALPHA);
        let mut c: Vec<f64> = uniform_sample(3, 30_000)
            .into_iter()
            .map(|x| x * x)
            .collect();
        assert!(ks_two_sample(&mut a, &mut c).p_value < ALPHA);
    }

    #[test]
    #[should_panic(expected = "sample 1 is NaN")]
    fn a_nan_in_both_samples_panics_instead_of_hanging() {
        let _ = ks_two_sample(&mut [0.5, f64::NAN], &mut [0.25, f64::NAN]);
    }

    #[test]
    #[should_panic(expected = "sample 0 is NaN")]
    fn a_nan_in_one_sample_panics() {
        let _ = ks_one_sample(&mut [f64::NAN, 0.5], |x| x.clamp(0.0, 1.0));
    }

    #[test]
    fn identical_samples_have_zero_distance() {
        let mut a = vec![0.1, 0.5, 0.9];
        let mut b = a.clone();
        let ks = ks_two_sample(&mut a, &mut b);
        assert!(ks.statistic.abs() < 1e-15);
        assert!((ks.p_value - 1.0).abs() < 1e-15);
    }
}
