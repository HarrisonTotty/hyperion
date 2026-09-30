//! The observed spread of rocky planets' core mass fractions, through which a rocky body's drawn
//! rank sets its composition (ruling 53 of 2026-09-22, amending ruling 47.1; plan 14, design note
//! 8).
//!
//! Chen and Kipping's scatter at low mass, 0.040 dex, mixes measurement error and sub-Neptunes into
//! a spread of radii wider than the whole range from pure iron to pure rock, so reading a rocky
//! body's composition off its Chen and Kipping radius made small inner planets iron-rich (a median
//! core mass fraction of 0.42 below 1.5 M⊕). For a rocky outcome the derivation instead takes the
//! composition from the observed distribution, and the radius from Zeng et al.'s curves at it.
//!
//! # Plotnykov and Valencia (2020)
//!
//! Plotnykov and Valencia (2020, MNRAS 499, 932; arXiv:2010.06480, abstract and §3.2, Figure 7)
//! infer the core mass fractions of the 33 rocky exoplanets whose masses and radii are known to
//! better than 25%, and give the population's as 0.24 +0.33 −0.18, broader than the 0.32 +0.14
//! −0.12 their host stars' abundances predict. That is read here as the median and the 16th and
//! 84th percentiles, 0.06, 0.24 and 0.57. The paper gives no functional form (its distribution is
//! a kernel density estimate), so the one used is a two-piece logit-normal through those three
//! points: logit(cmf) is normal about logit(0.24), with a scale below the median that puts the
//! 16th percentile at 0.06 and one above it that puts the 84th at 0.57. It lies inside (0, 1), as
//! a core mass fraction must, is continuous and increasing in the rank, and matches the quoted
//! percentiles exactly; Earth's 0.325 is at its 62nd percentile and Mercury's 0.70 at its 92nd.
//! Its share over half iron is 21%.
//!
//! # Beyond the snow line (ruling 58)
//!
//! The measured fractions are those of close-in rocky planets, and correct the iron excess of
//! Chen and Kipping's radii, not a body's water. A body formed beyond the snow line is ice-rich
//! (design note 8), so there only the part of its rank below the Earth-like curve is a rocky
//! outcome: its composition is the same distribution held to Earth's core mass fraction and above
//! ([`core_mass_fraction_at_least`]), and every rank above that curve keeps Chen and Kipping's
//! radius, and with it the water the solve reads there.

use crate::math;

/// The median core mass fraction of rocky exoplanets, 0.24 (Plotnykov and Valencia 2020,
/// abstract).
pub const MEDIAN_CORE_MASS_FRACTION: f64 = 0.24;

/// The 16th percentile of rocky exoplanets' core mass fractions, 0.24 − 0.18 = 0.06 (Plotnykov and
/// Valencia 2020, abstract).
pub const LOW_CORE_MASS_FRACTION: f64 = 0.06;

/// The 84th percentile of rocky exoplanets' core mass fractions, 0.24 + 0.33 = 0.57 (Plotnykov and
/// Valencia 2020, abstract).
pub const HIGH_CORE_MASS_FRACTION: f64 = 0.57;

/// logit(x) = ln(x ÷ (1 − x)), for 0 < x < 1.
#[must_use]
fn logit(x: f64) -> f64 {
    math::ln(x / (1.0 - x))
}

/// The logistic function, the inverse of [`logit`]: 1 ÷ (1 + e^−y), 0 for y = −∞ and 1 for +∞.
#[must_use]
fn logistic(y: f64) -> f64 {
    1.0 / (1.0 + math::exp(-y))
}

/// The distribution's scales in logit(cmf) below and above the median: the distances from the
/// median's logit to the 16th and 84th percentiles', which lie one standard deviation away.
#[must_use]
fn scales() -> (f64, f64) {
    let median = logit(MEDIAN_CORE_MASS_FRACTION);
    (
        median - logit(LOW_CORE_MASS_FRACTION),
        logit(HIGH_CORE_MASS_FRACTION) - median,
    )
}

/// The core mass fraction at rank `rank` of the observed distribution of rocky planets'
/// (Plotnykov and Valencia 2020; see the [module](self) documentation).
///
/// `rank` is clamped to 0–1, where the fraction is 0 (pure rock) and 1 (pure iron); inside, it is
/// logistic(logit(0.24) + s Φ⁻¹(`rank`)), with s the lower scale below the median and the upper
/// scale above it.
///
/// # Examples
///
/// The median, and the percentiles the paper quotes:
///
/// ```
/// use hyperion_sim::planetary::derive::rocky::core_mass_fraction_at;
///
/// assert!((core_mass_fraction_at(0.5) - 0.24).abs() < 1e-12);
/// assert!((core_mass_fraction_at(0.158_655_253_931_457) - 0.06).abs() < 1e-9);
/// assert!((core_mass_fraction_at(0.841_344_746_068_543) - 0.57).abs() < 1e-9);
/// ```
#[must_use]
pub fn core_mass_fraction_at(rank: f64) -> f64 {
    if rank.is_nan() || rank <= 0.0 {
        return 0.0;
    }
    if rank >= 1.0 {
        return 1.0;
    }
    let (below, above) = scales();
    let z = math::normal_quantile(rank);
    let scale = if z < 0.0 { below } else { above };
    logistic(logit(MEDIAN_CORE_MASS_FRACTION) + scale * z)
}

/// The rank at which [`core_mass_fraction_at`] gives `core_mass_fraction`: the distribution's
/// cumulative probability, 0 at pure rock and 1 at pure iron.
#[must_use]
pub fn core_mass_fraction_rank(core_mass_fraction: f64) -> f64 {
    if core_mass_fraction.is_nan() || core_mass_fraction <= 0.0 {
        return 0.0;
    }
    if core_mass_fraction >= 1.0 {
        return 1.0;
    }
    let (below, above) = scales();
    let offset = logit(core_mass_fraction) - logit(MEDIAN_CORE_MASS_FRACTION);
    let scale = if offset < 0.0 { below } else { above };
    0.5 * math::erfc(-offset / scale * core::f64::consts::FRAC_1_SQRT_2)
}

/// The core mass fraction at rank `rank` of the observed distribution held to `floor` and above:
/// the distribution conditioned on a core mass fraction of at least `floor` (ruling 58).
///
/// With F the distribution's cumulative probability ([`core_mass_fraction_rank`]), it is
/// [`core_mass_fraction_at`] of F(`floor`) + `rank` (1 − F(`floor`)): `floor` at rank 0, pure iron
/// at rank 1, and increasing between. A floor of 0 is the whole distribution, bit for bit, which
/// is a rocky outcome's inside the snow line; beyond it the floor is Earth's 0.325, the Earth-like
/// curve that tops the rocky outcomes there.
///
/// # Examples
///
/// The whole distribution with no floor, and Earth's composition at the bottom of the part held
/// to Earth's and above:
///
/// ```
/// use hyperion_sim::planetary::derive::rocky::{core_mass_fraction_at, core_mass_fraction_at_least};
///
/// assert!((core_mass_fraction_at_least(0.3, 0.0) - core_mass_fraction_at(0.3)).abs() < 1e-15);
/// assert!((core_mass_fraction_at_least(0.0, 0.325) - 0.325).abs() < 1e-12);
/// assert!(core_mass_fraction_at_least(0.5, 0.325) > core_mass_fraction_at(0.5));
/// ```
#[must_use]
pub fn core_mass_fraction_at_least(rank: f64, floor: f64) -> f64 {
    let below = core_mass_fraction_rank(floor);
    core_mass_fraction_at(below + rank * (1.0 - below))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    #[test]
    fn the_distribution_passes_through_the_quoted_percentiles() {
        let one_sigma = 0.5 * math::erfc(core::f64::consts::FRAC_1_SQRT_2);
        for (rank, cmf) in [
            (one_sigma, LOW_CORE_MASS_FRACTION),
            (0.5, MEDIAN_CORE_MASS_FRACTION),
            (1.0 - one_sigma, HIGH_CORE_MASS_FRACTION),
        ] {
            let got = core_mass_fraction_at(rank);
            assert!((got - cmf).abs() < 1e-12, "{rank}: {got}");
        }
    }

    #[test]
    fn the_rank_and_the_fraction_invert_each_other_and_rise_together() {
        let mut previous = 0.0;
        for i in 1..1_000_u32 {
            let rank = f64::from(i) / 1_000.0;
            let cmf = core_mass_fraction_at(rank);
            assert!(cmf > previous && cmf < 1.0, "{rank}: {cmf}");
            assert!(
                (core_mass_fraction_rank(cmf) - rank).abs() < 1e-12,
                "{rank}"
            );
            previous = cmf;
        }
        assert!(core_mass_fraction_at(0.0).abs() < f64::EPSILON);
        assert!((core_mass_fraction_at(1.0) - 1.0).abs() < f64::EPSILON);
        assert!(core_mass_fraction_at(1e-300) < 1e-12);
        assert!(core_mass_fraction_at(1.0 - 1e-16) > 0.999);
    }

    #[test]
    fn the_distribution_held_to_a_floor_rises_from_the_floor_to_iron() {
        let floor = crate::planetary::derive::radius::EARTH_CORE_MASS_FRACTION;
        let mut previous = 0.0;
        for i in 0..=1_000_u32 {
            let rank = f64::from(i) / 1_000.0;
            let whole = core_mass_fraction_at(rank);
            // No floor is the whole distribution, bit for bit.
            assert_same_bits(core_mass_fraction_at_least(rank, 0.0), whole);
            let held = core_mass_fraction_at_least(rank, floor);
            assert!(held >= floor - 1e-12 && held >= previous, "{rank}: {held}");
            // The conditional distribution: its rank is the whole one's, rescaled.
            if i > 0 && i < 1_000 {
                let below = core_mass_fraction_rank(floor);
                let conditional = (core_mass_fraction_rank(held) - below) / (1.0 - below);
                assert!((conditional - rank).abs() < 1e-9, "{rank}: {conditional}");
            }
            previous = held;
        }
        assert!((core_mass_fraction_at_least(0.0, floor) - floor).abs() < 1e-12);
        assert!((core_mass_fraction_at_least(1.0, floor) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn earth_and_mercury_sit_where_the_documentation_says() {
        let earth = core_mass_fraction_rank(0.325);
        let mercury = core_mass_fraction_rank(0.70);
        assert!((earth - 0.616).abs() < 0.001, "{earth}");
        assert!((mercury - 0.918).abs() < 0.001, "{mercury}");
        let over_half = 1.0 - core_mass_fraction_rank(0.5);
        assert!((over_half - 0.21).abs() < 0.01, "{over_half}");
    }
}
