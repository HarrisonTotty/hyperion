//! Normal and log-normal variates, by the Box–Muller transform.
//!
//! Box–Muller rather than a rejection method (ziggurat, polar), so that a normal always consumes
//! exactly two words and a stream's later draws never depend on how many attempts an earlier one
//! took. The logarithm takes a uniform in `(0, 1]`, which is never 0, so the transform never
//! evaluates `ln 0` (brainstorm, "Random streams, not a random sequence"). The smallest such
//! uniform, 2⁻⁵³, cuts the tail at `√(106 ln 2)` = 8.57 standard deviations, beyond which a normal
//! falls with probability about 10⁻¹⁷.

use std::f64::consts::{LN_10, TAU};

use super::uniform::{uniform_of, uniform_open_low_of};
use crate::math;
use crate::rng::Stream;

/// Box–Muller on two words: `u₁ = uniform_open_low(first)`, `u₂ = uniform(second)`,
/// `r = √(−2 ln u₁)`, `θ = 2π u₂`, and the pair `(r cos θ, r sin θ)`.
#[inline]
pub(super) fn box_muller(first: u64, second: u64) -> (f64, f64) {
    let u1 = uniform_open_low_of(first);
    let u2 = uniform_of(second);
    let r = (-2.0 * math::ln(u1)).sqrt();
    let theta = TAU * u2;
    let (sin, cos) = math::sin_cos(theta);
    (r * cos, r * sin)
}

/// Debug-asserts that a standard deviation is finite and not negative.
#[inline]
fn check_sigma(sigma: f64) {
    debug_assert!(
        sigma.is_finite() && sigma >= 0.0,
        "a standard deviation must be finite and non-negative, got {sigma}"
    );
}

impl Stream {
    /// Two independent standard normal variates, by the Box–Muller transform. Two words.
    ///
    /// The first word gives the radius through `√(−2 ln u)` with `u` in `(0, 1]`, so the largest
    /// magnitude is 8.57; the second gives the angle.
    #[inline]
    pub fn standard_normal_pair(&mut self) -> (f64, f64) {
        let first = self.next_u64();
        let second = self.next_u64();
        box_muller(first, second)
    }

    /// One standard normal variate: the first of [`standard_normal_pair`](Self::standard_normal_pair),
    /// the second discarded. Two words, always.
    #[inline]
    pub fn standard_normal(&mut self) -> f64 {
        self.standard_normal_pair().0
    }

    /// A normal variate, `mean + sigma × z` for a standard normal `z`. Two words.
    ///
    /// A `sigma` of 0 returns `mean`, and still consumes the two words.
    ///
    /// # Panics
    ///
    /// In debug builds, if `sigma` is negative or not finite.
    #[inline]
    pub fn normal(&mut self, mean: f64, sigma: f64) -> f64 {
        check_sigma(sigma);
        mean + sigma * self.standard_normal()
    }

    /// A log-normal variate, `exp(mu_ln + sigma_ln × z)`: its natural logarithm is normal with
    /// mean `mu_ln` and standard deviation `sigma_ln`, and its median is `exp(mu_ln)`. Two words.
    ///
    /// # Panics
    ///
    /// In debug builds, if `sigma_ln` is negative or not finite.
    #[inline]
    pub fn log_normal(&mut self, mu_ln: f64, sigma_ln: f64) -> f64 {
        check_sigma(sigma_ln);
        math::exp(mu_ln + sigma_ln * self.standard_normal())
    }

    /// A log-normal variate given its median and its scatter in dex (decades):
    /// `median × exp(ln 10 × sigma_dex × z)`. Two words.
    ///
    /// This is the form astrophysical relations quote, "0.11 dex of scatter" about a median.
    ///
    /// # Panics
    ///
    /// In debug builds, if `sigma_dex` is negative or not finite.
    ///
    /// # Examples
    ///
    /// A dark halo's concentration scatters by 0.11 dex about the median of Dutton and Macciò's
    /// (2014) concentration–mass relation (brainstorm, "Galaxy parameters"):
    ///
    /// ```
    /// use hyperion_sim::rng::{ObjectKey, Stream, tags};
    /// use hyperion_sim::{Seed, math};
    ///
    /// let median = 8.0; // from the relation, for the halo's mass
    /// let mut stream = Stream::open(Seed::new(7), tags::SELFTEST_STREAM, ObjectKey::galaxy());
    /// let concentration = stream.log_normal_dex(median, 0.11);
    /// // Within 8.57 standard deviations, the transform's cut-off, of the median.
    /// let widest = math::exp10(0.11 * 8.58);
    /// assert!(concentration > median / widest && concentration < median * widest);
    /// ```
    #[inline]
    pub fn log_normal_dex(&mut self, median: f64, sigma_dex: f64) -> f64 {
        check_sigma(sigma_dex);
        median * math::exp(LN_10 * sigma_dex * self.standard_normal())
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::Seed;
    use crate::rng::{ObjectKey, tags};

    /// The smallest `u₁`, 2⁻⁵³, gives the largest radius, `√(106 ln 2)` = 8.5717, and no `ln 0`.
    #[test]
    fn the_smallest_uniform_gives_a_finite_tail_of_8_57() {
        let (z, zero) = box_muller(0, 0);
        assert!(z.is_finite());
        assert!((z - 8.571_674).abs() < 1e-6, "{z}");
        assert!(zero.abs() < 1e-300);
        let (x, y) = box_muller(0, 0x4000_0000_0000_0000);
        assert!((x * x + y * y).sqrt() <= 8.571_675);
    }

    /// `u₁ = 1` gives a radius of 0, and every other word a finite pair.
    #[test]
    fn the_largest_uniform_gives_zero() {
        let (x, y) = box_muller(u64::MAX, 12_345);
        assert!(x.abs() < 1e-300 && y.abs() < 1e-300);
    }

    fn stream() -> Stream {
        Stream::open(
            Seed::new(0x0b0e_d0e5),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        )
    }

    #[test]
    fn every_normal_takes_two_words_and_follows_its_formula() {
        let mut s = stream();
        let reference = s.clone();
        let (a, b) = s.standard_normal_pair();
        let (ra, rb) = box_muller(reference.word_at(0), reference.word_at(1));
        assert_same_bits(a, ra);
        assert_same_bits(b, rb);
        assert_eq!(s.position(), 2);

        let z = box_muller(reference.word_at(2), reference.word_at(3)).0;
        assert_same_bits(s.standard_normal(), z);
        assert_eq!(s.position(), 4);

        let z = box_muller(reference.word_at(4), reference.word_at(5)).0;
        assert_same_bits(s.normal(3.0, 0.5), 3.0 + 0.5 * z);
        let z = box_muller(reference.word_at(6), reference.word_at(7)).0;
        assert_same_bits(s.log_normal(0.2, 0.7), math::exp(0.2 + 0.7 * z));
        let z = box_muller(reference.word_at(8), reference.word_at(9)).0;
        assert_same_bits(
            s.log_normal_dex(8.0, 0.11),
            8.0 * math::exp(LN_10 * 0.11 * z),
        );
        assert_eq!(s.position(), 10);
    }

    #[test]
    fn zero_sigma_returns_the_mean_and_still_draws() {
        let mut s = stream();
        assert_same_bits(s.normal(1.25, 0.0), 1.25);
        assert_same_bits(s.log_normal_dex(4.0, 0.0), 4.0);
        assert_eq!(s.position(), 4);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "standard deviation must be finite and non-negative")]
    fn a_negative_sigma_panics_in_debug() {
        let _ = stream().normal(0.0, -1.0);
    }
}
