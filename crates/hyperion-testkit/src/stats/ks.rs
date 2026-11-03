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

/// Stephens's finite-sample correction (Stephens 1970, J. R. Stat. Soc. B 32, 115): λ =
/// (√n + 0.12 + 0.11 ÷ √n) D.
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

/// Tests whether two weighted samples come from one distribution.
///
/// Each sample is `(value, weight)` pairs, which are sorted in place by value. D is the largest
/// distance between the two weighted empirical distributions. Each sample's size is Kish's (1965,
/// _Survey Sampling_) effective size, (Σw)² ÷ Σw², and the effective sample size is
/// `n₁ n₂ ÷ (n₁ + n₂)` of the two, so that with unit weights the test is [`ks_two_sample`]'s.
///
/// # Panics
///
/// If either sample is empty, holds a NaN value, or holds a weight that is not positive and
/// finite.
///
/// # Examples
///
/// A sample of 0.25 and 0.75, the first counted three times, is the same distribution as
/// 0.25, 0.25, 0.25, 0.75:
///
/// ```
/// use hyperion_testkit::stats::ks_two_sample_weighted;
///
/// let mut a = [(0.25, 3.0), (0.75, 1.0)];
/// let mut b = [(0.25, 1.0), (0.75, 1.0), (0.25, 1.0), (0.25, 1.0)];
/// assert!(ks_two_sample_weighted(&mut a, &mut b).statistic.abs() < 1e-15);
/// ```
#[must_use]
pub fn ks_two_sample_weighted(a: &mut [(f64, f64)], b: &mut [(f64, f64)]) -> KolmogorovSmirnov {
    assert!(
        !a.is_empty() && !b.is_empty(),
        "a two-sample KS test needs two samples"
    );
    for sample in [&*a, &*b] {
        if let Some(i) = sample.iter().position(|(x, _)| x.is_nan()) {
            panic!("sample {i} is NaN: a KS test needs ordered values");
        }
        if let Some(i) = sample
            .iter()
            .position(|(_, w)| !(w.is_finite() && *w > 0.0))
        {
            panic!("weight {i} is not positive and finite: {}", sample[i].1);
        }
    }
    a.sort_by(|x, y| x.0.total_cmp(&y.0));
    b.sort_by(|x, y| x.0.total_cmp(&y.0));
    let total = |s: &[(f64, f64)]| s.iter().fold(0.0, |sum, (_, w)| sum + w);
    let (wa, wb) = (total(a), total(b));
    let (mut seen_a, mut seen_b) = (0_usize, 0_usize);
    let (mut below_a, mut below_b) = (0.0, 0.0);
    let mut distance: f64 = 0.0;
    while seen_a < a.len() && seen_b < b.len() {
        let step = a[seen_a].0.min(b[seen_b].0);
        while seen_a < a.len() && a[seen_a].0 <= step {
            below_a += a[seen_a].1;
            seen_a += 1;
        }
        while seen_b < b.len() && b[seen_b].0 <= step {
            below_b += b[seen_b].1;
            seen_b += 1;
        }
        distance = distance.max((below_a / wa - below_b / wb).abs());
    }
    let kish = |s: &[(f64, f64)], w: f64| w * w / s.iter().fold(0.0, |sum, (_, x)| sum + x * x);
    let (na, nb) = (kish(a, wa), kish(b, wb));
    let effective_n = na * nb / (na + nb);
    KolmogorovSmirnov {
        statistic: distance,
        effective_n,
        p_value: kolmogorov_survival(stephens_lambda(effective_n, distance)),
    }
}

/// The distance at which a Kolmogorov–Smirnov test of effective sample size `effective_n` has
/// p-value `level`.
///
/// It inverts the tests' p-value: Stephens's λ at which [`kolmogorov_survival`] is `level`, over
/// his factor (Stephens 1970, J. R. Stat. Soc. B 32, 115). At 1% λ is 1.628, so for a large sample
/// the distance is about 1.628 ÷ √n.
///
/// # Panics
///
/// If `effective_n` is not positive and finite, or `level` is not inside (0, 1).
#[must_use]
pub fn ks_critical_distance(effective_n: f64, level: f64) -> f64 {
    assert!(
        effective_n.is_finite() && effective_n > 0.0,
        "an effective sample size is positive: {effective_n}"
    );
    assert!(
        level > 0.0 && level < 1.0,
        "a level is inside (0, 1): {level}"
    );
    // Q_KS falls from 1 at λ = 0 to under 10⁻¹⁰⁰ by λ = 11, so the root is bracketed; 100
    // halvings take the bracket below a double's spacing.
    let (mut lo, mut hi) = (0.0_f64, 11.0_f64);
    for _ in 0..100 {
        let mid = f64::midpoint(lo, hi);
        if kolmogorov_survival(mid) > level {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let root = effective_n.sqrt();
    f64::midpoint(lo, hi) / (root + 0.12 + 0.11 / root)
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
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

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

    /// With unit weights the weighted test is the unweighted one, bit for bit; a sample whose
    /// weights tilt it towards its low values is told from its unweighted self, and passes
    /// against a sample drawn from the tilted distribution; Kish's size of a weighted sample is
    /// under its count.
    #[test]
    fn the_weighted_test_is_the_unweighted_one_at_unit_weights() {
        let (a, b) = (uniform_sample(1, 20_000), uniform_sample(2, 30_000));
        let unit = |s: &[f64]| s.iter().map(|&x| (x, 1.0)).collect::<Vec<_>>();
        let plain = ks_two_sample(&mut a.clone(), &mut b.clone());
        let weighted = ks_two_sample_weighted(&mut unit(&a), &mut unit(&b));
        assert_eq!(plain.statistic.to_bits(), weighted.statistic.to_bits());
        assert!((weighted.effective_n - plain.effective_n).abs() < 1e-6);
        assert!((weighted.p_value - plain.p_value).abs() < 1e-12);
        // Weights of 2 (1 − x) make a uniform sample's density 2 (1 − x), whose quantile at u is
        // 1 − √(1 − u).
        let mut tilted: Vec<(f64, f64)> = a.iter().map(|&x| (x, 2.0 * (1.0 - x))).collect();
        assert!(ks_two_sample_weighted(&mut tilted.clone(), &mut unit(&b)).p_value < ALPHA);
        let mut drawn: Vec<(f64, f64)> = uniform_sample(3, 30_000)
            .into_iter()
            .map(|u| (1.0 - (1.0 - u).sqrt(), 1.0))
            .collect();
        let ks = ks_two_sample_weighted(&mut tilted, &mut drawn);
        assert_p_value("a tilted sample against its draws", ks.p_value, ALPHA);
        assert!(ks.effective_n < 20_000.0 * 30_000.0 / 50_000.0);
    }

    /// The critical distance is the test's p-value inverted: at it the p-value is the level, and
    /// at 1% for a large sample it is 1.628 ÷ √n.
    #[test]
    fn the_critical_distance_inverts_the_p_value() {
        for n in [50.0, 1_000.0, 120_000.0] {
            let d = ks_critical_distance(n, 0.01);
            let p = kolmogorov_survival(stephens_lambda(n, d));
            assert!((p - 0.01).abs() < 1e-9, "{n}: {p}");
        }
        let d = ks_critical_distance(1.0e8, 0.01);
        assert!((d * 1.0e4 - 1.627_6).abs() < 1e-3, "{d}");
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
