//! Poisson variates, by two exact methods.
//!
//! Poisson means run from well under 1 in a fine cell at the rim to a few hundred thousand in a
//! coarse cell at the galactic centre, so one method will not do (brainstorm, "Random streams, not
//! a random sequence"). Below [`POISSON_PTRS_MIN_MEAN`] the cumulative distribution is inverted by
//! sequential search on one uniform; from it up, Hörmann's transformed rejection with squeeze
//! (PTRS) takes two words per attempt. Both are exact: a rounded normal is not good enough, since
//! its error at a mean of 1.2 is over 5%.

use crate::math;
use crate::rng::Stream;

/// The mean from which [`Stream::poisson`] uses PTRS instead of inversion: 10, the lower limit of
/// Hörmann's (1993) method.
pub const POISSON_PTRS_MIN_MEAN: f64 = 10.0;

/// The largest mean [`Stream::poisson`] accepts: 2³¹. The generator needs a few hundred thousand.
pub const POISSON_MAX_MEAN: f64 = 2_147_483_648.0;

impl Stream {
    /// A Poisson variate of the given mean.
    ///
    /// Below [`POISSON_PTRS_MIN_MEAN`] it inverts the cumulative distribution by sequential
    /// search: one word, and none at a mean of 0, which returns 0. From it up to
    /// [`POISSON_MAX_MEAN`] it uses Hörmann's PTRS, two words per attempt, with 1.33 attempts on
    /// average at a mean of 10 falling to 1.13 at 1,000 and above (measured over 10⁵ draws).
    ///
    /// # Panics
    ///
    /// If `mean` is negative, NaN or above [`POISSON_MAX_MEAN`] (infinity included): a broken
    /// invariant in the caller, whose densities and volumes are finite and bounded.
    ///
    /// # Examples
    ///
    /// The number of candidates in a cell is a Poisson draw on the cell's stream, with the thinning
    /// bound times the cell's volume as its mean:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::rng::{ObjectKey, Stream, tags};
    ///
    /// let mut cell = Stream::open(Seed::new(3), tags::SELFTEST_STREAM, ObjectKey::cell(0));
    /// let candidates = cell.poisson(1.2);
    /// assert!(candidates < 20);
    /// assert_eq!(cell.position(), 1);
    /// ```
    pub fn poisson(&mut self, mean: f64) -> u64 {
        assert!(
            (0.0..=POISSON_MAX_MEAN).contains(&mean),
            "a Poisson mean must lie in [0, 2^31], got {mean}"
        );
        if mean < POISSON_PTRS_MIN_MEAN {
            poisson_inversion(self, mean)
        } else {
            poisson_ptrs(self, mean)
        }
    }
}

/// Inversion by sequential search, for a mean in `[0, 10)`. One word; none for a mean of 0.
fn poisson_inversion(stream: &mut Stream, mean: f64) -> u64 {
    // The mean is not negative here, so this is a mean of exactly 0.
    if mean <= 0.0 {
        return 0;
    }
    invert(mean, stream.uniform())
}

/// The smallest `k` whose cumulative probability exceeds `u`, by adding the probabilities
/// `p₀ = e^−mean`, `pₖ = pₖ₋₁ × (mean ÷ k)` in turn: sequential search, whose result the
/// comparisons below reproduce exactly.
///
/// Rounding can leave the running sum a few units in the last place short of 1 (at a mean of
/// 9.99 it settles at 1 − 3 × 2⁻⁵³), below the largest uniforms. The search therefore also stops
/// at the first term too small to move the sum, which returns that term's `k`: the far tail, and
/// below 60 at every mean under 10. That is what bounds the loop; without it the largest uniform
/// would search for ever. The mass it moves is at most those few units in the last place, the
/// resolution of a 53-bit uniform.
///
/// The first three terms do not depend on `u`, so they are summed before any comparison and the
/// result below 4 is counted without a branch on `u`: the same sums in the same order, so the same
/// answer as the plain loop, but the data-dependent exit no longer mispredicts on most draws. It
/// took the mean-1.2 draw from 41.8 to 34 ns (Criterion, `samplers/poisson_1.2`, on the
/// development machine, x86-64); the plan's target is 40 ns.
fn invert(mean: f64, u: f64) -> u64 {
    // Each bound is the running sum through term j, or infinity from the first term that fails
    // to move the sum, where the search stops; so the bounds never decrease, and the number of
    // them at or below `u` is the search's result whenever it is below 4.
    let s0 = math::exp(-mean);
    // mean ÷ 1 is `mean` exactly.
    let p1 = s0 * mean;
    let s1 = s0 + p1;
    let p2 = p1 * (mean / 2.0);
    let s2 = s1 + p2;
    let p3 = p2 * (mean / 3.0);
    let s3 = s2 + p3;
    let b1 = if s1 > s0 { s1 } else { f64::INFINITY };
    let b2 = if s2 > s1 { s2 } else { f64::INFINITY }.max(b1);
    let b3 = if s3 > s2 { s3 } else { f64::INFINITY }.max(b2);
    let below_four = u32::from(u >= s0) + u32::from(u >= b1) + u32::from(u >= b2);
    if u < b3 {
        return u64::from(below_four);
    }
    // No term so far failed to move the sum, and u ≥ s₃: carry on from term 3.
    let mut p = p3;
    let mut sum = s3;
    let mut k = 3_u32;
    while u >= sum {
        k += 1;
        p *= mean / f64::from(k);
        let next = sum + p;
        if next <= sum {
            break;
        }
        sum = next;
    }
    u64::from(k)
}

/// Hörmann's (1993) PTRS for a mean of at least 10. Two words per attempt.
///
/// W. Hörmann, _The transformed rejection method for generating Poisson random variables_,
/// Insurance: Mathematics and Economics 12 (1993) 39–45, algorithm PTRS, in the form of
/// `NumPy`'s `random_poisson_ptrs`: a hat of the form `(2a ÷ us + b) U + mean + 0.43` over the uniform
/// `U` in `(−½, ½)`, a squeeze that accepts many attempts without evaluating a logarithm, and an
/// exact test against the log-probability otherwise. `U` comes from [`Stream::uniform_open`], so
/// `us = ½ − |U|` is never 0, and `V` from [`Stream::uniform_open_low`], so `ln V` is finite.
#[expect(
    clippy::many_single_char_names,
    reason = "a, b, U, V and k are the names of Hörmann's paper, kept so the two can be compared"
)]
fn poisson_ptrs(stream: &mut Stream, mean: f64) -> u64 {
    let ln_mean = math::ln(mean);
    let b = 0.931 + 2.53 * mean.sqrt();
    let a = -0.059 + 0.02483 * b;
    let inv_alpha = 1.1239 + 1.1328 / (b - 3.4);
    let v_r = 0.9277 - 3.6224 / (b - 2.0);
    let ln_inv_alpha = math::ln(inv_alpha);
    loop {
        let u = stream.uniform_open() - 0.5;
        let v = stream.uniform_open_low();
        let us = 0.5 - u.abs();
        let k = ((2.0 * a / us + b) * u + mean + 0.43).floor();
        // `NumPy` tests the sign after the squeeze; for a mean of 10 or more the squeeze's region
        // (us ≥ 0.07) only reaches k ≥ 4, so testing it first changes no result and keeps every
        // conversion below on a non-negative value.
        if k < 0.0 {
            continue;
        }
        if us >= 0.07 && v <= v_r {
            return count(k);
        }
        if us < 0.013 && v > us {
            continue;
        }
        if math::ln(v) + ln_inv_alpha - math::ln(a / (us * us) + b)
            <= -mean + k * ln_mean - math::ln_gamma(k + 1.0)
        {
            return count(k);
        }
    }
}

/// An accepted PTRS count, a non-negative whole number far below 2⁵³, as an integer.
fn count(k: f64) -> u64 {
    debug_assert!(
        (0.0..9_007_199_254_740_992.0).contains(&k),
        "a PTRS count of {k} is out of range"
    );
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "k is a whole number in [0, 2^53), tested for sign by the caller"
    )]
    let k = k as u64;
    k
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_two_sample};

    use super::*;
    use crate::Seed;
    use crate::rng::sample::uniform::uniform_of;
    use crate::rng::{ObjectKey, tags};

    fn stream(n: u64) -> Stream {
        Stream::open(
            Seed::new(0x0901_5504),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy_item(n),
        )
    }

    #[test]
    fn a_mean_of_zero_returns_zero_without_drawing() {
        let mut s = stream(0);
        assert_eq!(s.poisson(0.0), 0);
        assert_eq!(s.position(), 0);
    }

    #[test]
    fn inversion_takes_one_word_and_follows_the_search() {
        let mut s = stream(1);
        let reference = s.clone();
        for n in 0..100 {
            let k = s.poisson(3.7);
            assert_eq!(k, invert(3.7, uniform_of(reference.word_at(n))));
        }
        assert_eq!(s.position(), 100);
    }

    /// The largest uniform ends the search in the far tail, below 60, at every inversion mean,
    /// including 9.99, where rounding leaves the running sum below it.
    #[test]
    fn the_largest_uniform_stops_in_the_tail() {
        let largest = uniform_of(u64::MAX);
        let mean = POISSON_PTRS_MIN_MEAN.next_down();
        for m in [0.01, 0.3, 1.2, 5.0, 9.99, mean] {
            let k = invert(m, largest);
            assert!(k < 60, "mean {m}: the largest uniform stops at {k}");
            assert!(k > 5, "mean {m}: the largest uniform stops at only {k}");
        }
        assert_eq!(invert(0.3, 0.0), 0);
        assert_eq!(invert(9.99, largest), 47, "the sum stalls at the 47th term");
    }

    /// The sequential search as the plan writes it, with the stop at a term too small to move the
    /// sum: the reference `invert` must reproduce exactly.
    fn sequential_search(mean: f64, u: f64) -> u64 {
        let mut p = math::exp(-mean);
        let mut sum = p;
        let mut k = 0_u32;
        while u >= sum {
            k += 1;
            p *= mean / f64::from(k);
            let next = sum + p;
            if next <= sum {
                break;
            }
            sum = next;
        }
        u64::from(k)
    }

    /// The branch-free prefix of `invert` gives the plain loop's answer on every uniform tried:
    /// LCG uniforms, the extremes, and the uniforms on either side of each of the first running
    /// sums, at means down to those where e^−mean rounds to 1 and the first terms fail to move
    /// the sum. The sweep from 10⁻⁹ to 10⁻⁷ crosses the means (about 1.3 to 10.5 × 10⁻⁹) where
    /// term 1 still moves the sum below the largest uniform and term 2 no longer does.
    #[test]
    fn invert_equals_the_sequential_search() {
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x9015);
        // (mean, LCG uniforms to try): the sweep needs only the uniforms at the running sums.
        let mut means: Vec<(f64, usize)> = [
            1e-300,
            1e-17,
            1e-12,
            1e-6,
            1e-3,
            0.01,
            0.3,
            1.0,
            1.2,
            2.5,
            3.0,
            5.0,
            7.3,
            9.0,
            9.99,
            POISSON_PTRS_MIN_MEAN.next_down(),
        ]
        .map(|mean| (mean, 20_000))
        .to_vec();
        let mut sweep = 1e-9;
        while sweep < 1e-7 {
            means.push((sweep, 0));
            sweep *= 1.02;
        }
        for (mean, random) in means {
            let mut uniforms = vec![0.0, uniform_of(u64::MAX), uniform_of(u64::MAX - (1 << 11))];
            let mut p = math::exp(-mean);
            let mut sum = p;
            for k in 1..=8 {
                // The grid points of the uniform next to this running sum.
                let step = sum * 9_007_199_254_740_992.0;
                for offset in [-1.0, 0.0, 1.0] {
                    let grid = (step.floor() + offset).clamp(0.0, 9_007_199_254_740_991.0);
                    uniforms.push(grid / 9_007_199_254_740_992.0);
                }
                p *= mean / f64::from(k);
                sum += p;
            }
            uniforms.extend((0..random).map(|_| uniform_of(lcg.next_u64())));
            for u in uniforms {
                assert_eq!(
                    invert(mean, u),
                    sequential_search(mean, u),
                    "mean {mean}, u {u}"
                );
            }
        }
    }

    #[test]
    fn ptrs_takes_two_words_per_attempt() {
        let mut s = stream(2);
        for _ in 0..1_000 {
            let before = s.position();
            let _ = s.poisson(40.0);
            let used = s.position() - before;
            assert!(used >= 2 && used.is_multiple_of(2), "{used} words");
        }
        // 1.21 attempts on average at a mean of 40, measured over 10⁵ draws.
        assert!(s.position() < 2 * 1_400, "{} words", s.position());
    }

    /// The two methods draw from one distribution where they meet.
    #[test]
    fn inversion_and_ptrs_agree_at_a_mean_of_ten() {
        let n = 100_000;
        let mut a = stream(3);
        let mut b = stream(4);
        let mut inverted: Vec<f64> = (0..n)
            .map(|_| count_to_f64(poisson_inversion(&mut a, 10.0)))
            .collect();
        let mut rejected: Vec<f64> = (0..n)
            .map(|_| count_to_f64(poisson_ptrs(&mut b, 10.0)))
            .collect();
        let ks = ks_two_sample(&mut inverted, &mut rejected);
        assert_p_value("inversion against PTRS at 10", ks.p_value, ALPHA);
    }

    fn count_to_f64(k: u64) -> f64 {
        f64::from(u32::try_from(k).unwrap())
    }

    #[test]
    #[should_panic(expected = "a Poisson mean must lie in [0, 2^31]")]
    fn a_negative_mean_panics() {
        let _ = stream(5).poisson(-0.5);
    }

    #[test]
    #[should_panic(expected = "a Poisson mean must lie in [0, 2^31]")]
    fn a_nan_mean_panics() {
        let _ = stream(5).poisson(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "a Poisson mean must lie in [0, 2^31]")]
    fn an_infinite_mean_panics() {
        let _ = stream(5).poisson(f64::INFINITY);
    }

    #[test]
    fn the_largest_supported_mean_draws_near_it() {
        let mut s = stream(6);
        let k = s.poisson(POISSON_MAX_MEAN);
        // 2³¹ ± 6 standard deviations of 46,341.
        assert!(k.abs_diff(1 << 31) < 280_000, "{k}");
    }
}
