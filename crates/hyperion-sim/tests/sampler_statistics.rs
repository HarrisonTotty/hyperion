//! Statistical tests of the samplers, each at N = 10⁵ under `just test` and at N = 10⁷ under
//! `just test-slow`.
//!
//! Every test draws from `selftest.stream` under a fixed seed, so it is deterministic: it cannot
//! flake, it can only start failing when generated output changes (determinism plan, Design note
//! 28). The significance level is `stats::ALPHA`. The fast and slow runs of a test use different
//! streams, so the slow run is not a superset of the fast one.

use std::num::NonZeroU64;

use hyperion_sim::Seed;
use hyperion_sim::math;
use hyperion_sim::rng::{ObjectKey, PiecewiseLinear, PiecewisePowerLaw, PowerLaw, Stream, tags};
use hyperion_testkit::stats::{
    ALPHA, ChiSquare, assert_p_value, assert_poisson_count, chi_square_gof, ks_one_sample,
    normal_cdf, poisson_cdf, poisson_pmf, regularised_gamma_p,
};

/// The sample size of the fast tests.
const FAST: u64 = 100_000;

/// The sample size of the slow tests.
const SLOW: u64 = 10_000_000;

/// The stream of test number `test` at sample size `n`.
fn stream(test: u64, n: u64) -> Stream {
    Stream::open(
        Seed::new(0x5a3f_1e00_0000_0000 | test),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy_item(n),
    )
}

/// `n` draws of `draw`.
fn sample(n: u64, mut draw: impl FnMut() -> f64) -> Vec<f64> {
    (0..n).map(|_| draw()).collect()
}

/// A count as an `f64`; counts in these tests are far below 2⁵³.
fn real(n: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the counts in these tests are below 2^53"
    )]
    let x = n as f64;
    x
}

/// `floor(x)` as an index, for `x` in `[0, 2⁵³)`.
fn floor_index(x: f64) -> usize {
    assert!((0.0..9e15).contains(&x), "{x} is not an index");
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "x is non-negative and far below 2^53, and flooring is the point"
    )]
    let i = x.floor() as usize;
    i
}

/// Asserts that a sample passes KS against `cdf`.
#[track_caller]
fn assert_ks(name: &str, mut samples: Vec<f64>, cdf: impl Fn(f64) -> f64) {
    let ks = ks_one_sample(&mut samples, cdf);
    assert_p_value(name, ks.p_value, ALPHA);
}

/// Declares a check as a fast test at N = 10⁵ and a slow one at N = 10⁷.
macro_rules! fast_and_slow {
    ($($check:ident: $fast:ident, $slow:ident, $reason:literal;)*) => {
        $(
            #[test]
            fn $fast() {
                $check(FAST);
            }

            #[test]
            #[ignore = $reason]
            fn $slow() {
                $check(SLOW);
            }
        )*
    };
}

// --- Uniforms and bounded integers (P01.T8.a) ---

fn uniform_fills_256_bins_evenly(n: u64) {
    let mut s = stream(1, n);
    let mut counts = [0_u64; 256];
    for _ in 0..n {
        counts[floor_index(s.uniform() * 256.0)] += 1;
    }
    let expected = [real(n) / 256.0; 256];
    let fit = chi_square_gof(&counts, &expected);
    assert_eq!(fit.dof, 255);
    assert_p_value("uniform over 256 bins", fit.p_value, ALPHA);
}

fn uniforms_pass_ks(n: u64) {
    let unit = |x: f64| x.clamp(0.0, 1.0);
    let mut s = stream(2, n);
    assert_ks("uniform", sample(n, || s.uniform()), unit);
    assert_ks("uniform_open_low", sample(n, || s.uniform_open_low()), unit);
    assert_ks("uniform_open", sample(n, || s.uniform_open()), unit);
    let values = sample(n, || s.uniform_in(-3.0, 5.0));
    assert!(values.iter().all(|x| (-3.0..=5.0).contains(x)));
    assert_ks("uniform_in(-3, 5)", values, |x| {
        ((x + 3.0) / 8.0).clamp(0.0, 1.0)
    });
}

fn below_six_is_a_fair_die(n: u64) {
    let six = NonZeroU64::new(6).unwrap();
    let mut s = stream(3, n);
    let mut counts = [0_u64; 6];
    for _ in 0..n {
        counts[usize::try_from(s.below(six)).unwrap()] += 1;
    }
    let fit = chi_square_gof(&counts, &[real(n) / 6.0; 6]);
    assert_p_value("below(6)", fit.p_value, ALPHA);
}

fast_and_slow! {
    uniform_fills_256_bins_evenly:
        uniform_chi_square_fast, uniform_chi_square_slow, "slow: uniform chi-square at N = 10^7";
    uniforms_pass_ks:
        uniform_ks_fast, uniform_ks_slow, "slow: uniform KS at N = 10^7";
    below_six_is_a_fair_die:
        below_six_fast, below_six_slow, "slow: below(6) chi-square at N = 10^7";
}

// --- Normal and log-normal (P01.T8.b) ---

fn normals_pass_ks(n: u64) {
    let mut s = stream(4, n);
    assert_ks(
        "standard_normal",
        sample(n, || s.standard_normal()),
        normal_cdf,
    );
    assert_ks("normal(2.5, 0.3)", sample(n, || s.normal(2.5, 0.3)), |x| {
        normal_cdf((x - 2.5) / 0.3)
    });
}

/// Both halves of a pair are standard normal, and they are uncorrelated: the sample correlation
/// lies within 4 ÷ √N of 0.
fn pair_halves_are_normal_and_uncorrelated(n: u64) {
    let mut s = stream(5, n);
    let pairs: Vec<(f64, f64)> = (0..n).map(|_| s.standard_normal_pair()).collect();
    let mean = |values: &mut dyn Iterator<Item = f64>| values.sum::<f64>() / real(n);
    let (mx, my) = (
        mean(&mut pairs.iter().map(|p| p.0)),
        mean(&mut pairs.iter().map(|p| p.1)),
    );
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for &(x, y) in &pairs {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
    }
    let correlation = sxy / (sxx * syy).sqrt();
    let bound = 4.0 / real(n).sqrt();
    assert!(
        correlation.abs() < bound,
        "pair correlation {correlation} beyond {bound}"
    );
    assert_ks("pair.0", pairs.iter().map(|p| p.0).collect(), normal_cdf);
    assert_ks("pair.1", pairs.iter().map(|p| p.1).collect(), normal_cdf);
}

/// Half of a log-normal lies below its median, and its logarithm is normal.
fn log_normals_have_their_median_and_normal_logarithms(n: u64) {
    let mut s = stream(6, n);
    let (mu, sigma) = (0.3, 0.5);
    let values = sample(n, || s.log_normal(mu, sigma));
    let median = math::exp(mu);
    let below = values.iter().filter(|&&x| x < median).count();
    assert_poisson_count(
        "log-normal draws below the median",
        u64::try_from(below).unwrap(),
        real(n) / 2.0,
        ALPHA,
    );
    let logs = values.iter().map(|&x| math::ln(x)).collect();
    assert_ks("ln of log_normal(0.3, 0.5)", logs, |y| {
        normal_cdf((y - mu) / sigma)
    });

    let (median, sigma_dex) = (8.0, 0.11);
    let values = sample(n, || s.log_normal_dex(median, sigma_dex));
    let centre = math::log10(median);
    let logs = values.iter().map(|&x| math::log10(x)).collect();
    assert_ks("log10 of log_normal_dex(8, 0.11)", logs, |y| {
        normal_cdf((y - centre) / sigma_dex)
    });
}

fast_and_slow! {
    normals_pass_ks:
        normal_ks_fast, normal_ks_slow, "slow: normal KS at N = 10^7";
    pair_halves_are_normal_and_uncorrelated:
        normal_pair_fast, normal_pair_slow, "slow: normal pairs at N = 10^7";
    log_normals_have_their_median_and_normal_logarithms:
        log_normal_fast, log_normal_slow, "slow: log-normal at N = 10^7";
}

// --- Poisson (P01.T8.c, P01.T8.d) ---

/// Chi-square of Poisson counts against the distribution: one bin per count from the smallest
/// observed to the largest, the tails folded into the end bins, thin bins merged by the helper.
fn poisson_fit(counts: &[u64], mean: f64) -> ChiSquare {
    let lo = *counts.iter().min().unwrap();
    let hi = *counts.iter().max().unwrap();
    assert!(hi > lo, "a Poisson sample of mean {mean} took one value");
    let width = usize::try_from(hi - lo + 1).unwrap();
    let mut observed = vec![0_u64; width];
    for &k in counts {
        observed[usize::try_from(k - lo).unwrap()] += 1;
    }
    let n = real(u64::try_from(counts.len()).unwrap());
    let expected: Vec<f64> = (lo..=hi)
        .map(|k| {
            let p = if k == lo {
                poisson_cdf(lo, mean)
            } else if k == hi {
                regularised_gamma_p(real(hi), mean)
            } else {
                poisson_pmf(k, mean)
            };
            n * p
        })
        .collect();
    chi_square_gof(&observed, &expected)
}

/// The sample mean and variance lie within 5 standard errors of the mean: `√(μ ÷ N)` for the
/// mean, `√((μ + 2μ²) ÷ N)` for the variance (the fourth central moment is μ + 3μ²).
fn assert_moments(name: &str, counts: &[u64], mean: f64) {
    let n = real(u64::try_from(counts.len()).unwrap());
    let sample_mean = counts.iter().map(|&k| real(k)).sum::<f64>() / n;
    let variance = counts
        .iter()
        .map(|&k| (real(k) - sample_mean) * (real(k) - sample_mean))
        .sum::<f64>()
        / (n - 1.0);
    let mean_error = (mean / n).sqrt();
    let variance_error = ((mean + 2.0 * mean * mean) / n).sqrt();
    assert!(
        (sample_mean - mean).abs() < 5.0 * mean_error,
        "{name}: sample mean {sample_mean}"
    );
    assert!(
        (variance - mean).abs() < 5.0 * variance_error,
        "{name}: sample variance {variance}"
    );
}

fn check_poisson_means(test: u64, n: u64, means: &[f64]) {
    for (i, &mean) in (0..).zip(means) {
        let mut s = stream(test, n + i);
        let counts: Vec<u64> = (0..n).map(|_| s.poisson(mean)).collect();
        let name = format!("poisson({mean})");
        assert_p_value(&name, poisson_fit(&counts, mean).p_value, ALPHA);
        assert_moments(&name, &counts, mean);
    }
}

fn inversion_matches_the_distribution(n: u64) {
    check_poisson_means(7, n, &[0.01, 0.3, 1.2, 5.0, 9.99]);
}

/// At a mean of 0.01 the zero class carries nearly everything, so the number of non-zero draws is
/// checked on its own.
fn inversion_at_a_hundredth_has_the_right_non_zero_count(n: u64) {
    let mut s = stream(8, n);
    let non_zero = (0..n).filter(|_| s.poisson(0.01) > 0).count();
    assert_poisson_count(
        "non-zero draws at a mean of 0.01",
        u64::try_from(non_zero).unwrap(),
        real(n) * -math::exp_m1(-0.01),
        ALPHA,
    );
}

fn ptrs_matches_the_distribution(n: u64) {
    check_poisson_means(9, n, &[10.0, 10.01, 50.0, 1_000.0, 3e5]);
}

fast_and_slow! {
    inversion_matches_the_distribution:
        poisson_inversion_fast, poisson_inversion_slow, "slow: Poisson by inversion at N = 10^7";
    inversion_at_a_hundredth_has_the_right_non_zero_count:
        poisson_small_mean_fast, poisson_small_mean_slow, "slow: Poisson at 0.01 at N = 10^7";
    ptrs_matches_the_distribution:
        poisson_ptrs_fast, poisson_ptrs_slow, "slow: Poisson by PTRS at N = 10^7";
}

/// The power check the brainstorm implies: a normal of mean and variance 1.2, rounded to the
/// nearest count, fails the chi-square that the Poisson sampler passes, so the test would catch
/// the shortcut.
#[test]
fn a_rounded_normal_fails_the_poisson_test() {
    let mean = 1.2;
    let mut s = stream(10, FAST);
    let counts: Vec<u64> = (0..FAST)
        .map(|_| {
            let x = s.normal(mean, mean.sqrt()).round().max(0.0);
            u64::try_from(floor_index(x)).unwrap()
        })
        .collect();
    let fit = poisson_fit(&counts, mean);
    assert!(
        fit.p_value < ALPHA,
        "a rounded normal passed as Poisson: {fit:?}"
    );
}

// --- Power law (P01.T8.e) ---

/// The cumulative distribution of a density ∝ x^−α on `[lo, hi]`, written out here rather than
/// taken from `PowerLaw::cdf`, so that a sampler and a `cdf` sharing a wrong exponent convention
/// cannot pass together: `(x^(1−α) − lo^(1−α)) ÷ (hi^(1−α) − lo^(1−α))`, and
/// `ln(x ÷ lo) ÷ ln(hi ÷ lo)` at α = 1. Near α = 1 the difference form loses about seven digits
/// to cancellation, which leaves it far more accurate than the test needs.
fn analytic_power_law_cdf(alpha: f64, lo: f64, hi: f64, x: f64) -> f64 {
    let x = x.clamp(lo, hi);
    let rise = 1.0 - alpha;
    if rise.abs() < 1e-12 {
        math::ln(x / lo) / math::ln(hi / lo)
    } else {
        let start = math::powf(lo, rise);
        (math::powf(x, rise) - start) / (math::powf(hi, rise) - start)
    }
}

fn power_laws_pass_ks(n: u64) {
    let laws = [
        (2.3, 8.0, 150.0),
        (1.3, 0.08, 0.5),
        (1.0, 1.0, 1_000.0),
        (1.0 + 1e-9, 0.08, 0.5),
        (1.0 - 1e-9, 0.08, 0.5),
        (1.0 + 1e-6, 0.08, 0.5),
        (1.0 - 1e-6, 0.08, 0.5),
        (-0.5, 1.0, 4.0),
        (0.0, 2.0, 5.0),
    ];
    for (i, (alpha, lo, hi)) in (0..).zip(laws) {
        let law = PowerLaw::new(alpha, lo, hi).unwrap();
        let mut s = stream(11, n + i);
        let values = sample(n, || s.power_law(&law));
        assert!(values.iter().all(|x| (lo..=hi).contains(x)));
        assert_ks(&format!("power law {alpha} on [{lo}, {hi}]"), values, |x| {
            analytic_power_law_cdf(alpha, lo, hi, x)
        });
    }
}

fast_and_slow! {
    power_laws_pass_ks:
        power_law_fast, power_law_slow, "slow: power-law KS at N = 10^7";
}

// --- Piecewise samplers (P01.T8.f) ---

/// Kroupa's (2001) initial mass function, MNRAS 322, 231, eq. 2: α = 0.3 on 0.01–0.08 M☉, 1.3 on
/// 0.08–0.5 M☉ and 2.3 above, here to 150 M☉.
fn kroupa() -> PiecewisePowerLaw {
    PiecewisePowerLaw::continuous(&[0.01, 0.08, 0.5, 150.0], &[0.3, 1.3, 2.3]).unwrap()
}

fn kroupa_passes_ks(n: u64) {
    let law = kroupa();
    let mut s = stream(12, n);
    let values = sample(n, || law.sample(&mut s));
    assert!(values.iter().all(|x| (0.01..=150.0).contains(x)));
    assert_ks("Kroupa", values, |x| law.cdf(x));
}

/// A band of the law samples only inside the band, with the law's own shape there.
fn truncation_samples_inside_the_band(n: u64) {
    let law = kroupa();
    let (lo, hi) = (0.3, 2.5);
    let band = law.truncated(lo, hi).unwrap();
    let mut s = stream(13, n);
    let values = sample(n, || band.sample(&mut s));
    assert!(values.iter().all(|x| (lo..=hi).contains(x)));
    let (f_lo, f_hi) = (law.cdf(lo), law.cdf(hi));
    assert_ks("Kroupa truncated to [0.3, 2.5]", values, |x| {
        ((law.cdf(x) - f_lo) / (f_hi - f_lo)).clamp(0.0, 1.0)
    });
}

/// A law with a jump at its break puts each segment's share of the integral in it: 1 ÷ 7 on
/// `[1, 2)` at density 1 and 6 ÷ 7 on `[2, 4]` at density 3.
fn a_discontinuous_law_splits_by_segment_integral(n: u64) {
    let law =
        PiecewisePowerLaw::with_coefficients(&[1.0, 2.0, 4.0], &[0.0, 0.0], &[1.0, 3.0]).unwrap();
    let mut s = stream(14, n);
    let values = sample(n, || law.sample(&mut s));
    let first = values.iter().filter(|&&x| x < 2.0).count();
    assert_poisson_count(
        "draws in the first segment",
        u64::try_from(first).unwrap(),
        real(n) / 7.0,
        ALPHA,
    );
    assert_ks("two-segment step", values, |x| law.cdf(x));
}

fn piecewise_linear_passes_ks(n: u64) {
    let triangle = PiecewiseLinear::new(&[0.0, 1.0, 3.0], &[0.0, 2.0, 0.0]).unwrap();
    let mut s = stream(15, n);
    let values = sample(n, || triangle.sample(&mut s));
    assert_ks("triangle", values, |x| triangle.cdf(x));

    let gapped =
        PiecewiseLinear::new(&[-2.0, 0.0, 1.0, 2.0, 5.0], &[0.5, 1.5, 0.0, 0.0, 2.0]).unwrap();
    let values = sample(n, || gapped.sample(&mut s));
    assert!(values.iter().all(|&x| !(x > 1.0 && x < 2.0)));
    assert_ks("profile with a zero segment", values, |x| gapped.cdf(x));
}

fast_and_slow! {
    kroupa_passes_ks:
        kroupa_fast, kroupa_slow, "slow: Kroupa KS at N = 10^7";
    truncation_samples_inside_the_band:
        kroupa_band_fast, kroupa_band_slow, "slow: truncated Kroupa KS at N = 10^7";
    a_discontinuous_law_splits_by_segment_integral:
        step_fast, step_slow, "slow: discontinuous power law at N = 10^7";
    piecewise_linear_passes_ks:
        piecewise_linear_fast, piecewise_linear_slow, "slow: piecewise linear KS at N = 10^7";
}
