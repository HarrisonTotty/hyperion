//! A small fixed generator for test inputs.
//!
//! Tests of the sim's own random streams need inputs that do not come from those streams. This is
//! Knuth's 64-bit linear congruential generator (MMIX: multiplier 6364136223846793005, increment
//! 1442695040888963407) with the `MurmurHash3` 64-bit finaliser applied to each state, so that the
//! low bits are as usable as the high ones. It is not part of the generator version: it may change
//! whenever no golden file depends on it.

const MULTIPLIER: u64 = 6_364_136_223_846_793_005;
const INCREMENT: u64 = 1_442_695_040_888_963_407;

/// A deterministic source of test words.
///
/// # Examples
///
/// ```
/// use hyperion_testkit::lcg::Lcg;
///
/// let mut a = Lcg::new(7);
/// let mut b = Lcg::new(7);
/// assert_eq!(a.next_u64(), b.next_u64());
/// assert!((0.0..1.0).contains(&a.next_f64()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Lcg {
    state: u64,
}

impl Lcg {
    /// Creates a generator from a seed. Every seed is valid.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64-bit word.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        let mut z = self.state;
        z = (z ^ (z >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        z = (z ^ (z >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        z ^ (z >> 33)
    }

    /// The next float, uniform in `[0, 1)`, from the top 53 bits of a word.
    pub fn next_f64(&mut self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit integer is exact in f64"
        )]
        let numerator = (self.next_u64() >> 11) as f64;
        numerator * TWO_POW_MINUS_53
    }

    /// The next integer in `[0, n)`, by reduction of the high half of a 128-bit product.
    ///
    /// The bias is below `n ÷ 2⁶⁴`, which is nothing for test inputs.
    ///
    /// # Panics
    ///
    /// If `n` is 0.
    pub fn next_below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "next_below needs a non-empty range");
        let product = u128::from(self.next_u64()) * u128::from(n);
        u64::try_from(product >> 64)
            .expect("the high half of a 64 × 64-bit product fits in 64 bits")
    }
}

/// 2⁻⁵³, the spacing of [`Lcg::next_f64`].
const TWO_POW_MINUS_53: f64 = 1.0 / 9_007_199_254_740_992.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_words_and_different_seeds_differ() {
        let words = |seed| {
            let mut g = Lcg::new(seed);
            (0..8).map(|_| g.next_u64()).collect::<Vec<_>>()
        };
        assert_eq!(words(1), words(1));
        assert_ne!(words(1), words(2));
    }

    #[test]
    fn floats_and_bounded_integers_stay_in_range() {
        let mut g = Lcg::new(99);
        for _ in 0..10_000 {
            let f = g.next_f64();
            assert!((0.0..1.0).contains(&f), "{f}");
            assert!(g.next_below(6) < 6);
        }
    }
}
