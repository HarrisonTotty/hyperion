//! Uniform floats and unbiased bounded integers.
//!
//! A uniform float is built from the top bits of one word, so that it is exact in `f64` (Design
//! note 6 of the determinism plan):
//!
//! | Sampler                                     | From word `w`                     | Range     |
//! | ------------------------------------------- | --------------------------------- | --------- |
//! | [`uniform`](Stream::uniform)                 | `(w >> 11) × 2⁻⁵³`                | `[0, 1)`  |
//! | [`uniform_open_low`](Stream::uniform_open_low) | `((w >> 11) + 1) × 2⁻⁵³`      | `(0, 1]`  |
//! | [`uniform_open`](Stream::uniform_open)       | `((w >> 12) + ½) × 2⁻⁵²`          | `(0, 1)`  |

use std::num::NonZeroU64;

use crate::rng::Stream;

/// 2⁻⁵³, exactly.
pub(super) const TWO_POW_MINUS_53: f64 = 1.0 / 9_007_199_254_740_992.0;

/// 2⁻⁵², exactly.
const TWO_POW_MINUS_52: f64 = 1.0 / 4_503_599_627_370_496.0;

/// `x` as an `f64`, exactly: `x` is at most 2⁵³.
#[inline]
fn exact(x: u64) -> f64 {
    debug_assert!(x <= 1 << 53, "{x} is not exact in f64");
    #[expect(
        clippy::cast_precision_loss,
        reason = "every integer up to 2^53 is exact in f64, and callers pass at most 2^53"
    )]
    let value = x as f64;
    value
}

/// `uniform` of a word: `(w >> 11) × 2⁻⁵³`, in `[0, 1)`.
#[inline]
pub(in crate::rng) fn uniform_of(word: u64) -> f64 {
    exact(word >> 11) * TWO_POW_MINUS_53
}

/// `uniform_open_low` of a word: `((w >> 11) + 1) × 2⁻⁵³`, in `(0, 1]`.
#[inline]
pub(super) fn uniform_open_low_of(word: u64) -> f64 {
    exact((word >> 11) + 1) * TWO_POW_MINUS_53
}

/// `uniform_open` of a word: `((w >> 12) + ½) × 2⁻⁵²`, in `(0, 1)`.
#[inline]
pub(super) fn uniform_open_of(word: u64) -> f64 {
    (exact(word >> 12) + 0.5) * TWO_POW_MINUS_52
}

/// The low half of a 128-bit product.
#[inline]
fn low_half(product: u128) -> u64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the low 64 bits are what is wanted"
    )]
    let low = product as u64;
    low
}

/// The high half of a 128-bit product of two `u64`s.
#[inline]
fn high_half(product: u128) -> u64 {
    u64::try_from(product >> 64).expect("the high half of a 64 × 64-bit product fits in 64 bits")
}

impl Stream {
    /// A uniform float in `[0, 1)` from the top 53 bits of one word: `(w >> 11) × 2⁻⁵³`.
    ///
    /// Every value is a multiple of 2⁻⁵³ and exact. One word.
    #[inline]
    pub fn uniform(&mut self) -> f64 {
        uniform_of(self.next_u64())
    }

    /// A uniform float in `(0, 1]`: `((w >> 11) + 1) × 2⁻⁵³`, never 0, so its logarithm is
    /// finite. One word.
    #[inline]
    pub fn uniform_open_low(&mut self) -> f64 {
        uniform_open_low_of(self.next_u64())
    }

    /// A uniform float strictly inside `(0, 1)`: `((w >> 12) + ½) × 2⁻⁵²`, from the top 52 bits.
    ///
    /// The smallest value is 2⁻⁵³ and the largest 1 − 2⁻⁵³, both exact. One word.
    #[inline]
    pub fn uniform_open(&mut self) -> f64 {
        uniform_open_of(self.next_u64())
    }

    /// A uniform float between `lo` and `hi`: `lo + (hi − lo) × uniform`. One word.
    ///
    /// The result lies in `[lo, hi]`. It is below `hi` in exact arithmetic, but the final addition
    /// rounds, and where `(hi − lo) × 2⁻⁵³` is at most half the spacing of floats just under `hi`
    /// the largest uniforms can round up to `hi` itself: on `[1, 2)` the uniform 1 − 2⁻⁵³ gives
    /// exactly 2. A caller that needs `hi` excluded clamps, or works in offsets from `lo`.
    ///
    /// # Panics
    ///
    /// In debug builds, unless `lo < hi`.
    #[inline]
    pub fn uniform_in(&mut self, lo: f64, hi: f64) -> f64 {
        debug_assert!(lo < hi, "uniform_in needs lo < hi, got [{lo}, {hi}]");
        let u = self.uniform();
        lo + (hi - lo) * u
    }

    /// An unbiased integer in `[0, n)`, by Lemire's multiply-and-reject method.
    ///
    /// The high half of the 128-bit product `word × n` is the result, and a word whose low half
    /// falls below `2⁶⁴ mod n` is rejected and redrawn, which removes the bias exactly (Lemire
    /// 2019, _Fast random integer generation in an interval_, ACM TOMACS 29). One word per
    /// attempt; the chance of a rejection is below `n ÷ 2⁶⁴`, so under two words on average for
    /// every `n`.
    ///
    /// # Examples
    ///
    /// A die roll:
    ///
    /// ```
    /// use std::num::NonZeroU64;
    ///
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::rng::{ObjectKey, Stream, tags};
    ///
    /// let mut stream = Stream::open(Seed::new(1), tags::SELFTEST_STREAM, ObjectKey::galaxy());
    /// let six = NonZeroU64::new(6).expect("6 is not zero");
    /// let face = 1 + stream.below(six);
    /// assert!((1..=6).contains(&face));
    /// ```
    pub fn below(&mut self, n: NonZeroU64) -> u64 {
        let n = n.get();
        let mut product = u128::from(self.next_u64()) * u128::from(n);
        if low_half(product) < n {
            // 2⁶⁴ mod n, computed only on this rare path because the division is slow.
            let threshold = n.wrapping_neg() % n;
            while low_half(product) < threshold {
                product = u128::from(self.next_u64()) * u128::from(n);
            }
        }
        high_half(product)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::Seed;
    use crate::rng::{ObjectKey, tags};

    #[test]
    fn extreme_words_give_the_documented_ends() {
        assert_same_bits(uniform_of(0), 0.0);
        assert_same_bits(uniform_of(u64::MAX), 1.0 - TWO_POW_MINUS_53);
        assert_same_bits(uniform_open_low_of(0), TWO_POW_MINUS_53);
        assert_same_bits(uniform_open_low_of(u64::MAX), 1.0);
        assert_same_bits(uniform_open_of(0), TWO_POW_MINUS_53);
        assert_same_bits(uniform_open_of(u64::MAX), 1.0 - TWO_POW_MINUS_53);
    }

    #[test]
    fn the_powers_of_two_are_exact() {
        assert_same_bits(TWO_POW_MINUS_53, f64::EPSILON / 2.0);
        assert_same_bits(TWO_POW_MINUS_52, f64::EPSILON);
    }

    /// The low bits a uniform discards do not move it, and one step of the kept bits moves it by
    /// exactly one spacing.
    #[test]
    fn uniforms_use_the_top_bits() {
        let w = 0x9e37_79b9_7f4a_7c15;
        assert_same_bits(uniform_of(w), uniform_of(w | 0x7ff));
        assert_same_bits(uniform_of(w + (1 << 11)) - uniform_of(w), TWO_POW_MINUS_53);
        assert_same_bits(uniform_open_of(w), uniform_open_of(w | 0xfff));
        assert_same_bits(uniform_open_low_of(w) - uniform_of(w), TWO_POW_MINUS_53);
    }

    fn stream(n: u64) -> Stream {
        Stream::open(
            Seed::new(0x0a11_ce00),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy_item(n),
        )
    }

    #[test]
    fn uniform_in_follows_its_formula_and_can_reach_hi_by_rounding() {
        let mut s = stream(1);
        let reference = s.clone();
        let value = s.uniform_in(-3.0, 5.0);
        assert_same_bits(value, -3.0 + 8.0 * uniform_of(reference.word_at(0)));
        // The documented rounding: 1 + (1 − 2⁻⁵³) is a tie that rounds to 2.
        assert_same_bits(1.0 + 1.0 * uniform_of(u64::MAX), 2.0);
    }

    #[test]
    fn below_one_is_always_zero_and_takes_one_word() {
        let mut s = stream(2);
        for _ in 0..100 {
            assert_eq!(s.below(NonZeroU64::MIN), 0);
        }
        assert_eq!(s.position(), 100);
    }

    /// At n = 2⁶³ + 1 almost half of all words are rejected, so the loop is exercised, and every
    /// result stays in range.
    #[test]
    fn below_two_to_the_sixty_three_plus_one_rejects_and_stays_in_range() {
        let n = NonZeroU64::new((1 << 63) + 1).unwrap();
        let mut s = stream(3);
        let draws = 10_000;
        let mut high = 0_u64;
        for _ in 0..draws {
            let x = s.below(n);
            assert!(x < n.get());
            if x >= 1 << 62 {
                high += 1;
            }
        }
        let words = s.position();
        assert!(
            words > draws + draws / 3,
            "expected about half of the words rejected, drew {words} for {draws}"
        );
        assert!(
            words < 3 * draws,
            "far too many rejections: {words} words for {draws}"
        );
        // Half of [0, 2⁶³ + 1) lies at or above 2⁶²; 5 standard errors of 10⁴ halves are 250.
        let expected = draws / 2;
        assert!(
            high.abs_diff(expected) < 250,
            "{high} of {draws} at or above 2^62"
        );
    }

    /// Lemire's result is the high half of the product, and the rejection threshold is
    /// 2⁶⁴ mod n.
    #[test]
    fn below_is_the_high_half_of_the_product_when_nothing_is_rejected() {
        let n = NonZeroU64::new(6).unwrap();
        let mut s = stream(4);
        let reference = s.clone();
        let value = s.below(n);
        let word = reference.word_at(0);
        let product = u128::from(word) * 6;
        assert!(
            low_half(product) >= (u64::MAX - 5) % 6,
            "the first word was accepted"
        );
        assert_eq!(value, high_half(product));
        assert_eq!(s.position(), 1);
    }
}
