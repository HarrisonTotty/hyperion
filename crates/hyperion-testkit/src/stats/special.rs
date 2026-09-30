//! Special functions: the regularised incomplete gamma function and what is built on it.

/// Iteration cap of the series and the continued fraction. Convergence takes a few times `√a`
/// steps, so this covers shape parameters far beyond the few hundred thousand the tests need.
const MAX_ITERATIONS: u32 = 10_000_000;

/// Relative accuracy at which the series and the continued fraction stop.
const EPSILON: f64 = 1e-16;

/// The smallest magnitude the Lentz recurrence lets a denominator take.
const TINY: f64 = 1e-300;

/// The regularised lower incomplete gamma function, P(a, x) = γ(a, x) ÷ Γ(a).
///
/// # Panics
///
/// If `a` is not positive and finite, or if `x` is negative or NaN.
#[must_use]
pub fn regularised_gamma_p(a: f64, x: f64) -> f64 {
    check_arguments(a, x);
    if x == 0.0 {
        0.0
    } else if x.is_infinite() {
        1.0
    } else if x < a + 1.0 {
        series_p(a, x)
    } else {
        1.0 - continued_fraction_q(a, x)
    }
}

/// The regularised upper incomplete gamma function, Q(a, x) = Γ(a, x) ÷ Γ(a) = 1 − P(a, x).
///
/// Computed by the series for `x < a + 1` and by the modified Lentz continued fraction otherwise
/// (Press et al., *Numerical Recipes*, 3rd ed., §6.2), with ln Γ from the pinned `libm`. The
/// chi-square survival function is `Q(dof ÷ 2, χ² ÷ 2)`, and the Poisson cumulative distribution
/// is `Q(k + 1, mean)`.
///
/// # Panics
///
/// If `a` is not positive and finite, or if `x` is negative or NaN.
#[must_use]
pub fn regularised_gamma_q(a: f64, x: f64) -> f64 {
    check_arguments(a, x);
    if x == 0.0 {
        1.0
    } else if x.is_infinite() {
        0.0
    } else if x < a + 1.0 {
        1.0 - series_p(a, x)
    } else {
        continued_fraction_q(a, x)
    }
}

fn check_arguments(a: f64, x: f64) {
    assert!(
        a > 0.0 && a.is_finite(),
        "incomplete gamma: a = {a} must be positive and finite"
    );
    assert!(x >= 0.0, "incomplete gamma: x = {x} must be non-negative");
}

/// `x^a e^(−x) ÷ Γ(a)`, the factor both expansions share.
fn prefactor(a: f64, x: f64) -> f64 {
    libm::exp(a * libm::log(x) - x - libm::lgamma(a))
}

/// P(a, x) by its power series, for `x < a + 1`.
fn series_p(a: f64, x: f64) -> f64 {
    let mut denominator = a;
    let mut term = 1.0 / a;
    let mut sum = term;
    for _ in 0..MAX_ITERATIONS {
        denominator += 1.0;
        term *= x / denominator;
        sum += term;
        if term.abs() < sum.abs() * EPSILON {
            return (sum * prefactor(a, x)).clamp(0.0, 1.0);
        }
    }
    panic!("incomplete gamma series did not converge for a = {a}, x = {x}");
}

/// Q(a, x) by the modified Lentz evaluation of its continued fraction, for `x ≥ a + 1`.
fn continued_fraction_q(a: f64, x: f64) -> f64 {
    let mut term_b = x + 1.0 - a;
    let mut lentz_c = 1.0 / TINY;
    let mut lentz_d = 1.0 / term_b;
    let mut fraction = lentz_d;
    let mut step = 0.0;
    for _ in 0..MAX_ITERATIONS {
        step += 1.0;
        let term_a = -step * (step - a);
        term_b += 2.0;
        lentz_d = term_a * lentz_d + term_b;
        if lentz_d.abs() < TINY {
            lentz_d = TINY;
        }
        lentz_c = term_b + term_a / lentz_c;
        if lentz_c.abs() < TINY {
            lentz_c = TINY;
        }
        lentz_d = 1.0 / lentz_d;
        let delta = lentz_d * lentz_c;
        fraction *= delta;
        if (delta - 1.0).abs() < EPSILON {
            return (fraction * prefactor(a, x)).clamp(0.0, 1.0);
        }
    }
    panic!("incomplete gamma continued fraction did not converge for a = {a}, x = {x}");
}

/// The cumulative distribution of the standard normal, Φ(z) = ½ erfc(−z ÷ √2).
#[must_use]
pub fn normal_cdf(z: f64) -> f64 {
    0.5 * libm::erfc(-z * std::f64::consts::FRAC_1_SQRT_2)
}

/// The probability that a Poisson variable of the given mean equals `k`.
///
/// Computed as `exp(k ln μ − μ − ln Γ(k + 1))`.
///
/// # Panics
///
/// If `mean` is negative or not finite.
#[must_use]
pub fn poisson_pmf(k: u64, mean: f64) -> f64 {
    check_mean(mean);
    if mean == 0.0 {
        return if k == 0 { 1.0 } else { 0.0 };
    }
    let k = count_as_f64(k);
    libm::exp(k * libm::log(mean) - mean - libm::lgamma(k + 1.0))
}

/// The probability that a Poisson variable of the given mean is at most `k`: `Q(k + 1, mean)`.
///
/// # Panics
///
/// If `mean` is negative or not finite.
#[must_use]
pub fn poisson_cdf(k: u64, mean: f64) -> f64 {
    check_mean(mean);
    regularised_gamma_q(count_as_f64(k) + 1.0, mean)
}

pub(super) fn check_mean(mean: f64) {
    assert!(
        mean >= 0.0 && mean.is_finite(),
        "Poisson mean {mean} must be finite and non-negative"
    );
}

/// A count as a float. Counts in tests are far below 2⁵³, where the conversion is exact.
pub(super) fn count_as_f64(count: u64) -> f64 {
    debug_assert!(count < (1 << 53), "count {count} is not exact in f64");
    #[expect(
        clippy::cast_precision_loss,
        reason = "test counts are below 2^53, asserted above"
    )]
    let value = count as f64;
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} is not within {tolerance} of {expected}"
        );
    }

    /// Reference values, each derived independently of this code.
    ///
    /// - Q(½, ½) = erfc(√½) = 0.317 310 507 863, the two-sided tail of a normal beyond 1σ
    ///   (Abramowitz & Stegun table 26.1: 1 − A(1) with A(1) = 0.682 689 492 137).
    /// - Q(5, 10) = P(Poisson(10) ≤ 4) = e⁻¹⁰ × 644⅓ = 0.029 252 688 077, summed exactly in
    ///   50-digit decimal arithmetic (A&S table 26.7 gives 0.029 25).
    /// - Q(50, 40) = P(Poisson(40) ≤ 49) = 0.929 664 933 341, by the same exact sum.
    #[test]
    fn gamma_q_matches_published_values() {
        assert_close(regularised_gamma_q(0.5, 0.5), 0.317_310_507_863, 1e-11);
        assert_close(regularised_gamma_q(5.0, 10.0), 0.029_252_688_077, 1e-11);
        assert_close(regularised_gamma_q(50.0, 40.0), 0.929_664_933_341, 1e-11);
    }

    #[test]
    fn gamma_p_and_q_are_complements_on_both_branches() {
        for (a, x) in [
            (0.5, 0.1),
            (0.5, 3.0),
            (7.0, 2.0),
            (7.0, 20.0),
            (300_000.0, 300_500.0),
        ] {
            assert_close(
                regularised_gamma_p(a, x) + regularised_gamma_q(a, x),
                1.0,
                1e-14,
            );
        }
        assert_close(regularised_gamma_q(3.0, 0.0), 1.0, 0.0);
        assert_close(regularised_gamma_p(3.0, f64::INFINITY), 1.0, 0.0);
    }

    /// The 5% critical value of chi-square at 5 degrees of freedom is 11.070 (A&S table 26.8),
    /// and Q(2.5, 5.535) = 0.050 009 6 to the digits the table cannot show (mpmath `gammainc`).
    #[test]
    fn chi_square_critical_value_gives_five_per_cent() {
        assert_close(regularised_gamma_q(2.5, 11.07 / 2.0), 0.0500, 5e-5);
        assert_close(
            regularised_gamma_q(2.5, 11.07 / 2.0),
            0.050_009_618_6,
            1e-10,
        );
    }

    #[test]
    fn normal_cdf_matches_the_table() {
        // A&S table 26.1.
        assert_close(normal_cdf(0.0), 0.5, 1e-15);
        assert_close(normal_cdf(1.0), 0.841_344_746_068_543, 1e-14);
        assert_close(normal_cdf(-1.96), 0.024_997_895_148_220, 1e-14);
        assert_close(normal_cdf(-8.0), 6.220_960_574_271_78e-16, 1e-28);
    }

    #[test]
    fn poisson_cdf_sums_the_pmf() {
        for mean in [0.01, 0.3, 1.2, 9.99, 40.0, 1_000.0] {
            let mut sum = 0.0;
            for k in 0..=2_000 {
                sum += poisson_pmf(k, mean);
                assert_close(poisson_cdf(k, mean), sum, 1e-12);
            }
        }
    }

    #[test]
    fn poisson_of_mean_zero_is_a_point_mass() {
        assert_close(poisson_pmf(0, 0.0), 1.0, 0.0);
        assert_close(poisson_pmf(3, 0.0), 0.0, 0.0);
        assert_close(poisson_cdf(0, 0.0), 1.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "must be positive and finite")]
    fn non_positive_shape_panics() {
        let _ = regularised_gamma_q(0.0, 1.0);
    }
}
