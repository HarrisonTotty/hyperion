//! Integer-threshold decisions: one uniform draw, kept as an integer, against 53-bit thresholds.
//!
//! Random decisions compare a hashed integer against an integer threshold (brainstorm, "Floating
//! point"). A [`Mark`] is the top 53 bits of one word, `w >> 11`: the uniform `(w >> 11) × 2⁻⁵³`
//! of [`Stream::uniform`] kept as the integer it is. A [`Threshold`] is `ceil(p × 2⁵³)` for a
//! probability `p`, and a decision is `mark < threshold` (Design note 7 of the determinism plan).
//! Multiplying by 2⁵³ is exact, so a threshold is a pure function of `p`'s bits; and because a
//! mark is an integer, `mark < ceil(p × 2⁵³)` holds exactly when `mark × 2⁻⁵³ < p`, so a decision
//! is exactly `uniform() < p` on the same word. `p = 1` gives 2⁵³, above every mark, so it always
//! accepts, which no threshold on a whole 64-bit word could say; `p = 0` gives 0 and never accepts.
//!
//! [`Thresholds`] turn this into a pick among classes: the running sums of the class weights in
//! index order, each over a bound. One mark then rejects a candidate or picks its class by the odds
//! at its position (brainstorm, "Large features"), which is how thinning accepts a system and
//! picks its component. [`Mark::pick_weighted`] gives the same answer without allocating, for the
//! thinning hot path.
//!
//! A mark is fixed by its word while a threshold may move: a property that changes with phase is
//! "one fixed uniform compared against a threshold that moves" (brainstorm, "Random streams, not a
//! random sequence"), and a mark can be compared against any number of thresholds.
//!
//! This is a convention, not a second line of defence. A threshold is itself computed from floats
//! (a density, a bound, their ratio), so reproducibility still rests on the pinned `libm` behind
//! [`math`](crate::math). What the convention removes is the edge case of a uniform draw equal to
//! exactly 1.

use super::Stream;

/// 2⁵³: the number of marks, and the threshold that accepts every one of them.
const MARKS: u64 = 1 << 53;

/// 2⁵³ as an `f64`, exactly.
const TWO_POW_53: f64 = 9_007_199_254_740_992.0;

/// A fixed uniform draw kept as an integer: the top 53 bits of one word, in `[0, 2⁵³)`.
///
/// It stands for the uniform `mark × 2⁻⁵³` in `[0, 1)`, the value [`Stream::uniform`] makes of the
/// same word, and it is compared against [`Threshold`]s instead of floats.
///
/// # Examples
///
/// One mark against a threshold that moves: a variable star is bright while its phase lies below a
/// duty cycle that grows with age, and the same mark answers at every age.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::rng::{ObjectKey, Stream, Threshold, tags};
///
/// let mut stream = Stream::open(Seed::new(5), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let mark = stream.mark();
/// let bright_at = |duty: f64| mark.is_below(Threshold::from_probability(duty));
/// // A larger duty cycle never turns a bright star dark.
/// assert!(!bright_at(0.2) || bright_at(0.6));
/// assert!(bright_at(1.0));
/// assert!(!bright_at(0.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mark(u64);

impl Mark {
    /// The mark of a word: its top 53 bits, `word >> 11`.
    ///
    /// [`Stream::mark`] draws a word and calls this; a word read by number with
    /// [`Stream::word_at`] gives the same mark.
    #[must_use]
    pub const fn from_word(word: u64) -> Self {
        Self(word >> 11)
    }

    /// The mark's value, in `[0, 2⁵³)`.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Whether the mark lies below `threshold`: the decision `uniform < p` for the threshold's
    /// probability `p`.
    #[must_use]
    pub const fn is_below(self, threshold: Threshold) -> bool {
        self.0 < threshold.0
    }

    /// The first class whose threshold lies above the mark, or `None`, which is rejection.
    #[must_use]
    pub fn pick(self, thresholds: &Thresholds) -> Option<usize> {
        thresholds.0.iter().position(|&t| self.is_below(t))
    }

    /// The class of `weights` the mark picks against `bound`, or `None`, which is rejection: the
    /// same answer as [`Thresholds::from_weights`] followed by [`pick`](Self::pick), without
    /// allocating and stopping at the first hit.
    ///
    /// The weights are summed in index order, class `i`'s threshold is that of the probability
    /// `sumᵢ ÷ bound`, and the first class whose threshold lies above the mark is picked. The mark
    /// is rejected with probability `1 − Σ weights ÷ bound`. This is thinning: the weights are the
    /// densities of the components at a candidate's position and the bound is the cell's thinning
    /// bound, so one mark rejects the candidate or picks its component by the odds.
    ///
    /// # Panics
    ///
    /// In debug builds, if `bound` is not positive, if a weight is negative or NaN, or if the
    /// weights' total exceeds `bound`: a density above its thinning bound, which the brainstorm's
    /// bound checks hunt for. The total is checked even when the pick stops early. In release
    /// builds each threshold saturates instead, as [`Threshold::from_ratio`]'s does.
    ///
    /// # Examples
    ///
    /// A candidate under a thinning bound of 2.5 systems per cubic light-year, where three
    /// components have densities 0.25, 1.5 and 0.5: it is rejected with probability 0.1 and
    /// otherwise takes a component with odds 1 : 6 : 2.
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::rng::{ObjectKey, Stream, Thresholds, tags};
    ///
    /// let densities = [0.25, 1.5, 0.5];
    /// let mut stream = Stream::open(Seed::new(8), tags::SELFTEST_STREAM, ObjectKey::galaxy());
    /// let mark = stream.mark();
    /// let component = mark.pick_weighted(&densities, 2.5);
    /// assert_eq!(component, mark.pick(&Thresholds::from_weights(&densities, 2.5)));
    /// ```
    #[must_use]
    pub fn pick_weighted(self, weights: &[f64], bound: f64) -> Option<usize> {
        debug_assert!(
            bound > 0.0,
            "a thinning bound must be positive, got {bound}"
        );
        let mut sum = 0.0;
        for (i, &weight) in weights.iter().enumerate() {
            debug_assert!(weight >= 0.0, "weight {i} is {weight}, not a density");
            sum += weight;
            if self.is_below(Threshold::saturating(sum / bound)) {
                let rest = &weights[i + 1..];
                debug_assert!(
                    rest.iter().all(|&weight| weight >= 0.0),
                    "a weight after {i} is negative or NaN, not a density"
                );
                debug_assert!(
                    running_total(sum, rest) <= bound,
                    "the weights' total {} exceeds its thinning bound {bound}",
                    running_total(sum, rest)
                );
                return Some(i);
            }
        }
        debug_assert!(
            sum <= bound,
            "the weights' total {sum} exceeds its thinning bound {bound}"
        );
        None
    }
}

/// `sum` plus `rest`, added in index order: the running total the pick would have reached.
fn running_total(sum: f64, rest: &[f64]) -> f64 {
    rest.iter().fold(sum, |total, &weight| total + weight)
}

/// A 53-bit decision threshold: `ceil(p × 2⁵³)` for a probability `p`, in `[0, 2⁵³]`.
///
/// A [`Mark`] below it accepts. [`ALWAYS`](Self::ALWAYS) (2⁵³) accepts every mark and
/// [`NEVER`](Self::NEVER) (0) none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Threshold(u64);

impl Threshold {
    /// The threshold of probability 1, 2⁵³: every mark lies below it.
    pub const ALWAYS: Self = Self(MARKS);

    /// The threshold of probability 0, 0: no mark lies below it.
    pub const NEVER: Self = Self(0);

    /// The threshold of probability `p`: `ceil(p × 2⁵³)`.
    ///
    /// A mark lies below it exactly when the uniform of the mark's word is below `p`.
    ///
    /// # Panics
    ///
    /// In debug builds, unless `0 ≤ p ≤ 1`. In release builds `p` is clamped instead: above 1
    /// gives [`ALWAYS`](Self::ALWAYS), below 0 or NaN gives [`NEVER`](Self::NEVER).
    ///
    /// # Examples
    ///
    /// Two stars in five have a companion:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::rng::{ObjectKey, Stream, Threshold, tags};
    ///
    /// let binary = Threshold::from_probability(0.4);
    /// let mut stream = Stream::open(Seed::new(2), tags::SELFTEST_STREAM, ObjectKey::galaxy());
    /// let binaries = (0..1_000).filter(|_| stream.decide(binary)).count();
    /// assert!((330..470).contains(&binaries));
    /// ```
    #[must_use]
    pub fn from_probability(p: f64) -> Self {
        debug_assert!(
            (0.0..=1.0).contains(&p),
            "a probability must lie in [0, 1], got {p}"
        );
        Self::saturating(p)
    }

    /// The threshold of `value ÷ bound`: the thinning form, [`from_probability`] of the ratio.
    ///
    /// This is where the brainstorm's "debug assertion that the density never exceeds the thinning
    /// bound" sits, so that every thinning decision passes through it.
    ///
    /// # Panics
    ///
    /// In debug builds, unless `bound > 0` and `0 ≤ value ≤ bound`. In release builds a value
    /// above its bound saturates to [`ALWAYS`](Self::ALWAYS).
    ///
    /// [`from_probability`]: Self::from_probability
    #[must_use]
    pub fn from_ratio(value: f64, bound: f64) -> Self {
        debug_assert!(
            bound > 0.0,
            "a thinning bound must be positive, got {bound}"
        );
        debug_assert!(value >= 0.0, "{value} is negative, not a density");
        debug_assert!(value <= bound, "{value} exceeds its thinning bound {bound}");
        Self::saturating(value / bound)
    }

    /// The threshold's value, in `[0, 2⁵³]`.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// `ceil(p × 2⁵³)` with `p` clamped to `[0, 1]`; a NaN gives 0.
    #[inline]
    fn saturating(p: f64) -> Self {
        // Multiplying by a power of two is exact, and so is `ceil`, so the threshold is a pure
        // function of `p`'s bits.
        let scaled = (p.clamp(0.0, 1.0) * TWO_POW_53).ceil();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a value clamped to [0, 1] scales to a whole number in [0, 2^53], exact in \
                      u64, and `as` takes a NaN to 0"
        )]
        let threshold = scaled as u64;
        Self(threshold)
    }
}

/// The thresholds of a pick among classes: the running sums of their weights over a bound.
///
/// Class `i`'s threshold is that of `(w₀ + … + wᵢ) ÷ bound`, the sums taken in index order, so the
/// order of the classes is part of the output. A mark picks the first class whose threshold lies
/// above it, and is rejected if none does.
///
/// # Examples
///
/// A mass function's segments, picked by their shares of its integral: the last threshold is 2⁵³,
/// so a pick never fails.
///
/// ```
/// use hyperion_sim::rng::{Mark, Threshold, Thresholds};
///
/// let integrals = [0.62, 0.3, 0.08];
/// let segments = Thresholds::from_weights(&integrals, 0.62 + 0.3 + 0.08);
/// assert_eq!(segments.as_slice().last(), Some(&Threshold::ALWAYS));
/// assert_eq!(Mark::from_word(0).pick(&segments), Some(0));
/// assert_eq!(Mark::from_word(u64::MAX).pick(&segments), Some(2));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Thresholds(Vec<Threshold>);

impl Thresholds {
    /// The thresholds of classes with these weights under `bound`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `bound` is not positive, if a weight is negative or NaN, or if the
    /// weights' total exceeds `bound`. In release builds each threshold saturates instead, as
    /// [`Threshold::from_ratio`]'s does.
    #[must_use]
    pub fn from_weights(weights: &[f64], bound: f64) -> Self {
        debug_assert!(
            bound > 0.0,
            "a thinning bound must be positive, got {bound}"
        );
        let mut sum = 0.0;
        let thresholds = weights
            .iter()
            .enumerate()
            .map(|(i, &weight)| {
                debug_assert!(weight >= 0.0, "weight {i} is {weight}, not a density");
                sum += weight;
                Threshold::saturating(sum / bound)
            })
            .collect();
        debug_assert!(
            sum <= bound,
            "the weights' total {sum} exceeds its thinning bound {bound}"
        );
        Self(thresholds)
    }

    /// The thresholds, one per class, in class order.
    #[must_use]
    pub fn as_slice(&self) -> &[Threshold] {
        &self.0
    }

    /// The number of classes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no classes, so that every mark is rejected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Stream {
    /// A [`Mark`]: the top 53 bits of the next word. One word.
    #[inline]
    pub fn mark(&mut self) -> Mark {
        Mark::from_word(self.next_u64())
    }

    /// A decision: whether the next word's mark lies below `threshold`, which is `uniform() < p`
    /// for the threshold's probability `p`. One word.
    #[inline]
    pub fn decide(&mut self, threshold: Threshold) -> bool {
        self.mark().is_below(threshold)
    }

    /// The class the next word's mark picks, or `None` for rejection. One word.
    #[inline]
    pub fn pick(&mut self, thresholds: &Thresholds) -> Option<usize> {
        self.mark().pick(thresholds)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};

    use super::*;
    use crate::Seed;
    use crate::rng::sample::uniform_of;
    use crate::rng::{ObjectKey, tags};

    /// 2⁻⁵³, exactly: the smallest non-zero uniform, and one step between marks.
    const TWO_POW_MINUS_53: f64 = 1.0 / TWO_POW_53;

    /// Words whose marks are 0, 1, 2⁵³ − 2 and 2⁵³ − 1.
    const EXTREME_WORDS: [u64; 4] = [0, 1 << 11, u64::MAX - (1 << 11), u64::MAX];

    /// Probabilities where a decision is most likely to differ from the float comparison: the
    /// smallest uniforms, just under 1, a half, and values far below 2⁻⁵³ that still accept mark 0.
    const EDGE_PROBABILITIES: [f64; 8] = [
        TWO_POW_MINUS_53,
        2.0 * TWO_POW_MINUS_53,
        1.5 * TWO_POW_MINUS_53,
        1.0 - TWO_POW_MINUS_53,
        1.0 - 2.0 * TWO_POW_MINUS_53,
        0.5,
        1e-300,
        5e-324,
    ];

    fn stream(item: u64) -> Stream {
        Stream::open(
            Seed::new(0xdec1_de00),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy_item(item),
        )
    }

    /// A count as an `f64`; the counts here are below 2⁵³.
    fn real(n: u64) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "the counts in these tests are below 2^53"
        )]
        let x = n as f64;
        x
    }

    #[test]
    fn probability_one_always_accepts_and_zero_never_does() {
        assert_eq!(Threshold::from_probability(1.0), Threshold::ALWAYS);
        assert_eq!(Threshold::from_probability(0.0), Threshold::NEVER);
        assert_eq!(Threshold::from_ratio(7.5, 7.5), Threshold::ALWAYS);
        assert_eq!(Threshold::from_ratio(0.0, 7.5), Threshold::NEVER);
        assert_eq!(Threshold::ALWAYS.get(), 1 << 53);
        for word in EXTREME_WORDS {
            let mark = Mark::from_word(word);
            assert!(mark.is_below(Threshold::ALWAYS), "mark {}", mark.get());
            assert!(!mark.is_below(Threshold::NEVER), "mark {}", mark.get());
        }
        assert_eq!(Mark::from_word(u64::MAX).get(), (1 << 53) - 1);
    }

    #[track_caller]
    fn assert_decision_matches(word: u64, p: f64) {
        assert_eq!(
            Mark::from_word(word).is_below(Threshold::from_probability(p)),
            uniform_of(word) < p,
            "word {word:#018x}, p = {p:e}"
        );
    }

    /// Design note 7: the integer decision is exactly `uniform < p` on the same word.
    #[test]
    fn a_decision_equals_the_uniform_comparison_on_the_same_word() {
        let mut lcg = Lcg::new(0xdec1_de01);
        for n in 0..100_000_usize {
            let word = lcg.next_u64();
            let p = match n % 4 {
                // A multiple of 2⁻⁵³, where a mark can equal the probability.
                0 => lcg.next_f64(),
                // Finer than 2⁻⁵³.
                1 => lcg.next_f64() * lcg.next_f64(),
                2 => EDGE_PROBABILITIES[n / 4 % EDGE_PROBABILITIES.len()],
                _ => 1.0 - lcg.next_f64() * 1e-12,
            };
            assert_decision_matches(word, p);
        }
        // At the threshold itself, where the two could first disagree, and at the extreme marks.
        let mut probabilities = EDGE_PROBABILITIES.to_vec();
        probabilities.extend((0..1_000).map(|_| lcg.next_f64() * lcg.next_f64()));
        probabilities.extend([0.0, 1.0]);
        for p in probabilities {
            let t = Threshold::from_probability(p).get();
            for mark in [t.saturating_sub(1), t, t + 1] {
                if mark < MARKS {
                    assert_decision_matches(mark << 11, p);
                    assert_decision_matches((mark << 11) | 0x7ff, p);
                }
            }
            for word in EXTREME_WORDS {
                assert_decision_matches(word, p);
            }
        }
        assert_eq!(Threshold::from_probability(TWO_POW_MINUS_53).get(), 1);
        assert_eq!(
            Threshold::from_probability(1.0 - TWO_POW_MINUS_53).get(),
            (1 << 53) - 1
        );
        assert_eq!(Threshold::from_probability(5e-324).get(), 1);
    }

    /// Four classes with shares 0.1, 0.25, 0.05 and 0.3 of a bound of 2.5, and rejection as a fifth
    /// class with the remaining 0.3.
    #[test]
    fn pick_frequencies_follow_the_weights_with_rejection_as_a_class() {
        let weights = [0.25, 0.625, 0.125, 0.75];
        let shares = [0.1, 0.25, 0.05, 0.3, 0.3];
        let thresholds = Thresholds::from_weights(&weights, 2.5);
        let n = 100_000;
        let mut s = stream(1);
        let mut counts = [0_u64; 5];
        for _ in 0..n {
            counts[s.pick(&thresholds).unwrap_or(4)] += 1;
        }
        assert_eq!(s.position(), n, "one word per pick");
        let expected = shares.map(|share| share * real(n));
        let fit = chi_square_gof(&counts, &expected);
        assert_eq!(fit.dof, 4);
        assert_p_value("pick among four classes and rejection", fit.p_value, ALPHA);
    }

    /// Weights with zeros, empty lists, and bounds at and above the total.
    fn random_case(lcg: &mut Lcg) -> (Vec<f64>, f64) {
        let len = usize::try_from(lcg.next_below(8)).unwrap();
        let weights: Vec<f64> = (0..len)
            .map(|_| {
                if lcg.next_below(4) == 0 {
                    0.0
                } else {
                    lcg.next_f64() * 3.0
                }
            })
            .collect();
        let total = running_total(0.0, &weights);
        let bound = if total <= 0.0 {
            1.0
        } else if lcg.next_below(3) == 0 {
            total
        } else {
            total * (1.0 + lcg.next_f64())
        };
        (weights, bound)
    }

    #[test]
    fn pick_weighted_equals_from_weights_then_pick() {
        let mut lcg = Lcg::new(0xdec1_de02);
        for _ in 0..10_000 {
            let (weights, bound) = random_case(&mut lcg);
            let thresholds = Thresholds::from_weights(&weights, bound);
            assert_eq!(thresholds.len(), weights.len());
            let mut words: Vec<u64> = EXTREME_WORDS.to_vec();
            words.extend((0..4).map(|_| lcg.next_u64()));
            for t in thresholds.as_slice() {
                let t = t.get();
                words.extend(
                    [t.saturating_sub(1), t]
                        .into_iter()
                        .filter(|&m| m < MARKS)
                        .map(|m| m << 11),
                );
            }
            for word in words {
                let mark = Mark::from_word(word);
                assert_eq!(
                    mark.pick_weighted(&weights, bound),
                    mark.pick(&thresholds),
                    "weights {weights:?}, bound {bound}, mark {}",
                    mark.get()
                );
            }
        }
    }

    /// Weights 1, 2, 0 and 1 over 8: the running sums 1, 3, 3 and 4 give thresholds of 2⁵⁰,
    /// 3 × 2⁵⁰, 3 × 2⁵⁰ and 2⁵², so the zero-weight class is never picked.
    #[test]
    fn thresholds_are_the_running_sums_in_index_order() {
        let thresholds = Thresholds::from_weights(&[1.0, 2.0, 0.0, 1.0], 8.0);
        let values: Vec<u64> = thresholds.as_slice().iter().map(|t| t.get()).collect();
        assert_eq!(values, [1 << 50, 3 << 50, 3 << 50, 1 << 52]);
        let pick = |mark: u64| Mark::from_word(mark << 11).pick(&thresholds);
        assert_eq!(pick(0), Some(0));
        assert_eq!(pick((1 << 50) - 1), Some(0));
        assert_eq!(pick(1 << 50), Some(1));
        assert_eq!(pick((3 << 50) - 1), Some(1));
        assert_eq!(pick(3 << 50), Some(3));
        assert_eq!(pick((1 << 52) - 1), Some(3));
        assert_eq!(pick(1 << 52), None);
        let reversed = Thresholds::from_weights(&[1.0, 0.0, 2.0, 1.0], 8.0);
        assert_ne!(
            reversed, thresholds,
            "the order of the classes is part of the output"
        );
        let empty = Thresholds::from_weights(&[], 1.0);
        assert!(empty.is_empty());
        assert_eq!(Mark::from_word(0).pick(&empty), None);
        assert_eq!(Mark::from_word(0).pick_weighted(&[], 1.0), None);
    }

    #[test]
    fn a_ratio_is_the_probability_of_its_quotient() {
        for (value, bound) in [(3.0, 7.0), (0.1, 0.3), (1e-9, 2.5e4), (2.5, 2.5)] {
            assert_eq!(
                Threshold::from_ratio(value, bound),
                Threshold::from_probability(value / bound)
            );
        }
    }

    /// The release behaviour: what the debug assertions reject saturates.
    #[test]
    fn out_of_range_probabilities_saturate() {
        assert_eq!(Threshold::saturating(1.5), Threshold::ALWAYS);
        assert_eq!(Threshold::saturating(f64::INFINITY), Threshold::ALWAYS);
        assert_eq!(Threshold::saturating(-0.5), Threshold::NEVER);
        assert_eq!(Threshold::saturating(f64::NAN), Threshold::NEVER);
    }

    #[test]
    fn stream_decisions_take_one_word_each() {
        let mut s = stream(2);
        let reference = s.clone();
        let third = Threshold::from_probability(1.0 / 3.0);
        let thresholds = Thresholds::from_weights(&[0.2, 0.3], 1.0);
        for n in 0..30 {
            let word = reference.word_at(n);
            match n % 3 {
                0 => assert_eq!(s.mark(), Mark::from_word(word)),
                1 => assert_eq!(s.decide(third), Mark::from_word(word).is_below(third)),
                _ => assert_eq!(s.pick(&thresholds), Mark::from_word(word).pick(&thresholds)),
            }
            assert_eq!(s.position(), n + 1);
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "exceeds its thinning bound")]
    fn a_ratio_above_its_bound_panics_in_debug() {
        let _ = Threshold::from_ratio(1.5, 1.0);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "exceeds its thinning bound")]
    fn weights_above_their_bound_panic_in_debug() {
        let _ = Thresholds::from_weights(&[0.5, 0.7], 1.0);
    }

    /// Mark 0 picks class 0 at once; the total is still checked.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "exceeds its thinning bound")]
    fn pick_weighted_checks_the_total_after_an_early_pick() {
        let _ = Mark::from_word(0).pick_weighted(&[0.5, 0.7], 1.0);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "must lie in [0, 1]")]
    fn a_probability_above_one_panics_in_debug() {
        let _ = Threshold::from_probability(1.0 + f64::EPSILON);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "not a density")]
    fn a_negative_weight_panics_in_debug() {
        let _ = Mark::from_word(u64::MAX).pick_weighted(&[0.5, -0.1], 1.0);
    }
}
