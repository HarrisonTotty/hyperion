//! Power laws with both limits explicit, by inverse transform.
//!
//! A density ∝ x^−α on `[lo, hi]`. With `g = 1 − α` and `ℓ = ln(hi ÷ lo)` its cumulative
//! distribution is `F(x) = expm1(g ln(x ÷ lo)) ÷ expm1(g ℓ)`, and a uniform `u` maps to
//! `x = lo × exp(ln_1p(u × expm1(g ℓ)) ÷ g)`. Written through `exp_m1` and `ln_1p` it stays
//! accurate as α approaches 1, where the textbook `x^g` form cancels; within
//! [`EXPONENT_ONE_TOLERANCE`] of 1 the exact limit, `x = lo × exp(u ℓ)`, takes over. The rogue
//! planets' mass function falls "nearly as 1 ÷ mass", so that neighbourhood is not a corner case
//! (brainstorm, "Random streams, not a random sequence": "the power law needs the exponent 1
//! special case and an explicit upper mass limit").

use std::error::Error;
use std::fmt;

use crate::math;
use crate::rng::Stream;

/// Within this distance of 1 the exponent is treated as exactly 1: `|1 − α| < 10⁻⁸`.
///
/// At the switch the two forms differ by about `|1 − α| ℓ² ÷ 8` in `ln x`, below 10⁻⁷ for any
/// range under 20 decades.
pub(super) const EXPONENT_ONE_TOLERANCE: f64 = 1e-8;

/// `∫ₐᵇ x^−α dx` for `0 < a ≤ b`, through `exp_m1`, with the limit form `a^g ln(b ÷ a)` within
/// [`EXPONENT_ONE_TOLERANCE`] of α = 1.
pub(super) fn power_integral(alpha: f64, a: f64, b: f64) -> f64 {
    let g = 1.0 - alpha;
    let log_ratio = math::ln(b / a);
    if g.abs() >= EXPONENT_ONE_TOLERANCE {
        math::powf(a, g) * math::exp_m1(g * log_ratio) / g
    } else {
        math::powf(a, g) * log_ratio
    }
}

/// A [`PowerLaw`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildPowerLawError {
    /// The exponent or a limit is NaN or infinite.
    NonFinite,
    /// The lower limit is zero or negative.
    LowerLimitNotPositive,
    /// The upper limit is not above the lower, or so close that `ln(hi ÷ lo)` rounds to 0.
    EmptyRange,
    /// The density's integral over the range, or the sampler's `expm1(g ℓ)`, overflows `f64`.
    NotNormalisable,
}

impl fmt::Display for BuildPowerLawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "a power law's exponent and limits must be finite",
            Self::LowerLimitNotPositive => "a power law's lower limit must be positive",
            Self::EmptyRange => "a power law's upper limit must lie above its lower limit",
            Self::NotNormalisable => "a power law's integral over its range overflows",
        })
    }
}

impl Error for BuildPowerLawError {}

/// A power-law density ∝ x^−α on `[lo, hi]`, both limits explicit and required.
///
/// The upper limit is not optional (brainstorm: "an explicit upper mass limit"): a mass function
/// without one has no maximum mass, and for α ≤ 1 no normalisation. Built once, drawn from with
/// [`Stream::power_law`], one word per draw.
///
/// # Examples
///
/// The high-mass end of Kroupa's (2001) initial mass function, α = 2.3 from 8 to 150 M☉:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::rng::{ObjectKey, PowerLaw, Stream, tags};
///
/// let massive = PowerLaw::new(2.3, 8.0, 150.0)?;
/// let mut stream = Stream::open(Seed::new(5), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let mass = stream.power_law(&massive);
/// assert!((8.0..=150.0).contains(&mass));
/// // Half of such stars lie below the median.
/// let median = massive.quantile(0.5);
/// assert!((massive.cdf(median) - 0.5).abs() < 1e-12);
/// # Ok::<(), hyperion_sim::rng::BuildPowerLawError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerLaw {
    exponent: f64,
    lo: f64,
    hi: f64,
    /// `ln(hi ÷ lo)`, positive.
    log_ratio: f64,
    /// `expm1(g ℓ)` with `g = 1 − α`: the sampler's span. Unused within the tolerance of α = 1.
    span: f64,
    /// `∫ x^−α dx` over the range, positive and finite.
    integral: f64,
}

impl PowerLaw {
    /// A density ∝ x^−`exponent` on `[lo, hi]`.
    ///
    /// # Errors
    ///
    /// - [`BuildPowerLawError::NonFinite`] if any argument is NaN or infinite.
    /// - [`BuildPowerLawError::LowerLimitNotPositive`] if `lo ≤ 0`.
    /// - [`BuildPowerLawError::EmptyRange`] if `hi ≤ lo`, or `ln(hi ÷ lo)` rounds to 0.
    /// - [`BuildPowerLawError::NotNormalisable`] if the integral over the range overflows.
    pub fn new(exponent: f64, lo: f64, hi: f64) -> Result<Self, BuildPowerLawError> {
        if !(exponent.is_finite() && lo.is_finite() && hi.is_finite()) {
            return Err(BuildPowerLawError::NonFinite);
        }
        if lo <= 0.0 {
            return Err(BuildPowerLawError::LowerLimitNotPositive);
        }
        let log_ratio = math::ln(hi / lo);
        if hi <= lo || log_ratio <= 0.0 {
            return Err(BuildPowerLawError::EmptyRange);
        }
        let span = math::exp_m1((1.0 - exponent) * log_ratio);
        let integral = power_integral(exponent, lo, hi);
        if !(span.is_finite() && integral.is_finite() && integral > 0.0) {
            return Err(BuildPowerLawError::NotNormalisable);
        }
        Ok(Self {
            exponent,
            lo,
            hi,
            log_ratio,
            span,
            integral,
        })
    }

    /// The exponent α of the density x^−α.
    #[must_use]
    pub fn exponent(&self) -> f64 {
        self.exponent
    }

    /// The lower limit.
    #[must_use]
    pub fn lo(&self) -> f64 {
        self.lo
    }

    /// The upper limit.
    #[must_use]
    pub fn hi(&self) -> f64 {
        self.hi
    }

    /// `∫ x^−α dx` over `[lo, hi]`: the density's normalisation.
    #[must_use]
    pub fn integral(&self) -> f64 {
        self.integral
    }

    /// The normalised density at `x`: `x^−α ÷ integral` on `[lo, hi]`, 0 outside.
    #[must_use]
    pub fn pdf(&self, x: f64) -> f64 {
        if (self.lo..=self.hi).contains(&x) {
            math::powf(x, -self.exponent) / self.integral
        } else {
            0.0
        }
    }

    /// The cumulative distribution at `x`: 0 below `lo`, 1 above `hi`.
    #[must_use]
    pub fn cdf(&self, x: f64) -> f64 {
        if x <= self.lo {
            return 0.0;
        }
        if x >= self.hi {
            return 1.0;
        }
        let g = 1.0 - self.exponent;
        let log_x = math::ln(x / self.lo);
        let f = if g.abs() >= EXPONENT_ONE_TOLERANCE {
            math::exp_m1(g * log_x) / self.span
        } else {
            log_x / self.log_ratio
        };
        f.clamp(0.0, 1.0)
    }

    /// The value whose cumulative probability is `u`, for `u` in `[0, 1]`: the inverse transform
    /// [`Stream::power_law`] applies to a uniform.
    ///
    /// `lo × exp(ln_1p(u × expm1(g ℓ)) ÷ g)`, or `lo × exp(u ℓ)` within 10⁻⁸ of α = 1, clamped to
    /// `[lo, hi]` against rounding in the last place.
    #[must_use]
    pub fn quantile(&self, u: f64) -> f64 {
        let g = 1.0 - self.exponent;
        let x = if g.abs() >= EXPONENT_ONE_TOLERANCE {
            self.lo * math::exp(math::ln_1p(u * self.span) / g)
        } else {
            self.lo * math::exp(u * self.log_ratio)
        };
        x.clamp(self.lo, self.hi)
    }
}

impl Stream {
    /// A draw from a power law: [`PowerLaw::quantile`] of [`uniform`](Self::uniform). One word.
    ///
    /// The smallest uniform gives `lo`, and the largest a value just under `hi`.
    #[inline]
    pub fn power_law(&mut self, law: &PowerLaw) -> f64 {
        law.quantile(self.uniform())
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::Seed;
    use crate::rng::sample::uniform::uniform_of;
    use crate::rng::{ObjectKey, tags};

    #[test]
    fn invalid_arguments_are_rejected_with_their_variant() {
        use BuildPowerLawError as E;
        assert_eq!(PowerLaw::new(f64::NAN, 1.0, 2.0), Err(E::NonFinite));
        assert_eq!(PowerLaw::new(2.0, 1.0, f64::INFINITY), Err(E::NonFinite));
        assert_eq!(PowerLaw::new(2.0, 0.0, 2.0), Err(E::LowerLimitNotPositive));
        assert_eq!(PowerLaw::new(2.0, -1.0, 2.0), Err(E::LowerLimitNotPositive));
        assert_eq!(PowerLaw::new(2.0, 2.0, 2.0), Err(E::EmptyRange));
        assert_eq!(PowerLaw::new(2.0, 3.0, 2.0), Err(E::EmptyRange));
        assert_eq!(PowerLaw::new(-2.0, 1.0, 1e300), Err(E::NotNormalisable));
        assert_eq!(PowerLaw::new(3.0, 1e-300, 1.0), Err(E::NotNormalisable));
    }

    /// The smallest uniform gives `lo` exactly, the largest a value within a few ulps under `hi`.
    #[test]
    fn extreme_uniforms_give_lo_and_just_under_hi() {
        for (alpha, lo, hi) in [
            (2.3, 8.0, 150.0),
            (1.3, 0.08, 0.5),
            (1.0, 1.0, 1000.0),
            (1.0 + 1e-9, 1.0, 1000.0),
            (-0.5, 1.0, 4.0),
            (0.0, 2.0, 5.0),
        ] {
            let law = PowerLaw::new(alpha, lo, hi).unwrap();
            assert_same_bits(law.quantile(uniform_of(0)), lo);
            let top = law.quantile(uniform_of(u64::MAX));
            assert!(top <= hi, "α = {alpha}: {top} above {hi}");
            assert!(
                (hi - top) / hi < 1e-13,
                "α = {alpha}: the largest uniform gives {top}, not just under {hi}"
            );
        }
    }

    /// Across α = 1 the sampler's two forms meet: the same uniform gives values that differ in
    /// `ln x` by at most `|1 − α| ℓ² ÷ 8`, the true change of the distribution, and within 10⁻⁶
    /// relative on a Kroupa segment for |1 − α| up to 10⁻⁶.
    #[test]
    fn the_exponent_one_forms_are_continuous() {
        let (lo, hi) = (0.08, 0.5);
        let log_ratio = math::ln(hi / lo);
        let exact = PowerLaw::new(1.0, lo, hi).unwrap();
        let mut lcg = Lcg::new(0x0a1f);
        let offsets = [
            1e-9, -1e-9, 0.99e-8, -0.99e-8, 1.01e-8, -1.01e-8, 1e-6, -1e-6,
        ];
        for _ in 0..10_000 {
            let u = uniform_of(lcg.next_u64());
            let reference = exact.quantile(u);
            for offset in offsets {
                let x = PowerLaw::new(1.0 + offset, lo, hi).unwrap().quantile(u);
                let bound = offset.abs() * log_ratio * log_ratio / 8.0 + 1e-14;
                let moved = math::ln(x / reference).abs();
                assert!(
                    moved <= bound,
                    "α = 1 + {offset}, u = {u}: {moved} > {bound}"
                );
                assert!((x - reference).abs() / reference < 1e-6);
            }
        }
    }

    #[test]
    fn cdf_inverts_quantile_and_pdf_integrates_to_it() {
        for (alpha, lo, hi) in [
            (2.3, 8.0, 150.0),
            (1.3, 0.08, 0.5),
            (1.0, 1.0, 1000.0),
            (-0.5, 1.0, 4.0),
            (0.0, 2.0, 5.0),
        ] {
            let law = PowerLaw::new(alpha, lo, hi).unwrap();
            for u in [0.0, 0.1, 0.25, 0.5, 0.9, 0.999] {
                let x = law.quantile(u);
                assert!((law.cdf(x) - u).abs() < 1e-12, "α = {alpha}, u = {u}");
            }
            // A midpoint rule over 10⁵ steps against the cdf.
            let steps = 100_000_u32;
            let width = (hi - lo) / f64::from(steps);
            let area: f64 = (0..steps)
                .map(|i| law.pdf(lo + (f64::from(i) + 0.5) * width) * width)
                .sum();
            assert!((area - 1.0).abs() < 1e-6, "α = {alpha}: {area}");
            assert_same_bits(law.cdf(lo), 0.0);
            assert_same_bits(law.cdf(hi), 1.0);
            assert_same_bits(law.pdf(lo / 2.0), 0.0);
        }
    }

    #[test]
    fn integral_has_its_closed_forms() {
        let law = PowerLaw::new(2.0, 1.0, 4.0).unwrap();
        assert!((law.integral() - 0.75).abs() < 1e-15);
        let law = PowerLaw::new(1.0, 1.0, 4.0).unwrap();
        assert!((law.integral() - math::ln(4.0)).abs() < 1e-15);
        let law = PowerLaw::new(0.0, 2.0, 5.0).unwrap();
        assert!((law.integral() - 3.0).abs() < 1e-14);
    }

    /// The exponent's sign convention, density ∝ x^−α, pinned through `cdf` and `quantile` by hand:
    /// x⁻² on [1, 4] has F(x) = (1 − 1 ÷ x) ÷ ¾, x⁻¹ has F = ln x ÷ ln 4, x⁰ on [2, 5] is linear
    /// and x¹ on [1, 3] has F(x) = (x² − 1) ÷ 8.
    #[test]
    fn cdf_and_quantile_have_their_closed_forms() {
        for (alpha, lo, hi, x, f) in [
            (2.0, 1.0, 4.0, 2.0, 2.0 / 3.0),
            (1.0, 1.0, 4.0, 2.0, 0.5),
            (0.0, 2.0, 5.0, 3.0, 1.0 / 3.0),
            (-1.0, 1.0, 3.0, 2.0, 3.0 / 8.0),
        ] {
            let law = PowerLaw::new(alpha, lo, hi).unwrap();
            assert!((law.cdf(x) - f).abs() < 1e-15, "α = {alpha}: F({x})");
            assert!((law.quantile(f) - x).abs() < 1e-14, "α = {alpha}: quantile");
        }
    }

    #[test]
    fn a_draw_takes_one_word() {
        let law = PowerLaw::new(2.3, 8.0, 150.0).unwrap();
        let mut s = Stream::open(
            Seed::new(0x9a3e),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        );
        let reference = s.clone();
        let x = s.power_law(&law);
        assert_same_bits(x, law.quantile(uniform_of(reference.word_at(0))));
        assert_eq!(s.position(), 1);
    }
}
