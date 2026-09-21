//! Special functions the galaxy model needs and `libm` lacks.

use crate::math;

/// Where [`bessel_i0e`] switches from its power series to its asymptotic series.
///
/// At k = 15 the power series takes 31 terms and the asymptotic series is already good to
/// 10⁻¹⁴, below the plan's 10⁻¹² for both sides of the switch.
const I0E_SERIES_LIMIT: f64 = 15.0;

/// A term smaller than this fraction of the running sum ends a series: 2⁻⁶⁰, below the last bit.
const SERIES_CUTOFF: f64 = 1.0 / 1_152_921_504_606_846_976.0;

/// The exponentially scaled modified Bessel function of the first kind and order zero,
/// `e^(−k) I₀(k)`, for `k ≥ 0`.
///
/// The sharp spiral arm divides by it (plan 02, Design note 10): `exp(k (cos φ − 1)) ÷ I₀ₑ(k)`
/// averages exactly 1 around a circle, because `(1 ÷ 2π) ∫ exp(k cos φ) dφ = I₀(k)`. The scaled
/// form stays finite for the large `k` of a narrow arm far out, where `I₀` itself overflows.
///
/// Below k = 15 it sums the power series `I₀(k) = Σ (k² ÷ 4)ʲ ÷ (j!)²` until a term falls below
/// 2⁻⁶⁰ of the sum, then multiplies by `e^(−k)`; every term is positive, so nothing cancels. From
/// k = 15 up it sums the asymptotic series `e^(−k) I₀(k) ~ (2πk)^(−½) Σ aⱼ` with
/// `aⱼ = aⱼ₋₁ (2j − 1)² ÷ (8 j k)` until a term falls below 2⁻⁶⁰ of the sum or would grow
/// (Abramowitz and Stegun 1964, §9.6.10 and §9.7.1). Both are good to a few units in the last
/// place against `mpmath`, and they meet at k = 15 to about 10⁻¹⁴.
///
/// # Panics
///
/// In debug builds, if `k` is negative or NaN.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::special::bessel_i0e;
///
/// assert_eq!(bessel_i0e(0.0), 1.0);
/// // Far out it tends to 1 ÷ √(2πk).
/// let k = 1e6;
/// assert!((bessel_i0e(k) * (2.0 * std::f64::consts::PI * k).sqrt() - 1.0).abs() < 1e-6);
/// ```
#[must_use]
pub fn bessel_i0e(k: f64) -> f64 {
    debug_assert!(k >= 0.0, "bessel_i0e needs k ≥ 0, got {k}");
    if k < I0E_SERIES_LIMIT {
        let quarter_k_squared = 0.25 * k * k;
        let mut term = 1.0;
        let mut sum = 1.0;
        let mut j = 0.0;
        loop {
            j += 1.0;
            term *= quarter_k_squared / (j * j);
            sum += term;
            if term <= sum * SERIES_CUTOFF {
                break;
            }
        }
        sum * math::exp(-k)
    } else {
        let eight_k = 8.0 * k;
        let mut term = 1.0;
        let mut sum = 1.0;
        let mut j = 0.0;
        loop {
            j += 1.0;
            let odd = 2.0 * j - 1.0;
            let next = term * (odd * odd) / (j * eight_k);
            if next >= term {
                break;
            }
            term = next;
            sum += term;
            if term <= sum * SERIES_CUTOFF {
                break;
            }
        }
        sum / (2.0 * std::f64::consts::PI * k).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `e^(−k) I₀(k)` at 40 significant digits from `mpmath` (`besseli(0, k) * exp(-k)`),
    /// rounded to the nearest `f64`.
    const REFERENCE: [(f64, f64); 12] = [
        (0.0, 1.0),
        (1e-3, 0.999_000_749_583_515_6),
        (0.25, 0.791_017_162_139_719_4),
        (1.0, 0.465_759_607_593_640_43),
        (2.5, 0.270_046_441_612_202_76),
        (5.0, 0.183_540_812_609_328_36),
        (10.0, 0.127_833_337_163_428_6),
        (14.5, 0.105_708_407_377_795),
        (15.5, 0.102_180_506_523_292_1),
        (25.0, 0.080_196_773_547_436_7),
        (100.0, 0.039_944_379_299_096_68),
        (1000.0, 0.012_617_240_455_891_257),
    ];

    #[test]
    fn matches_reference_values_to_one_part_in_a_trillion() {
        for (k, expected) in REFERENCE {
            let actual = bessel_i0e(k);
            let error = ((actual - expected) / expected).abs();
            assert!(error < 1e-12, "i0e({k}) = {actual}, expected {expected}");
        }
    }

    /// The two series meet at the switch: just below 15 the power series answers, from 15 the
    /// asymptotic one, and they agree to far better than the function's slope over one step.
    #[test]
    fn is_continuous_across_the_switch() {
        let below = bessel_i0e(I0E_SERIES_LIMIT.next_down());
        let at = bessel_i0e(I0E_SERIES_LIMIT);
        assert!(((below - at) / at).abs() < 1e-12, "{below} against {at}");
        for k in [14.0, 14.9, 15.1, 16.0] {
            let slope = (bessel_i0e(k + 1e-6) - bessel_i0e(k - 1e-6)) / 2e-6;
            assert!(slope < 0.0, "i0e falls at {k}");
        }
    }

    #[test]
    fn falls_monotonically_from_one() {
        let mut previous = bessel_i0e(0.0);
        for i in 1..=400 {
            let k = f64::from(i) * 0.1;
            let value = bessel_i0e(k);
            assert!(value < previous, "i0e({k}) = {value} ≥ {previous}");
            previous = value;
        }
    }
}
