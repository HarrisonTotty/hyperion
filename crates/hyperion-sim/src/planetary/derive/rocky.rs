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
//!
//! # About the host's iron
//!
//! A host's rock sets the iron its rocky planets can hold: Plotnykov and Valencia (2020) predict
//! their planets' core mass fractions from their host stars' abundances as 0.32 +0.14 −0.12,
//! inside the planets' own wider 0.24 +0.33 −0.18 (abstract, read), and Adibekyan et al. (2021,
//! Science 374, 330; not read here) is cited for the planets' iron following their hosts'. The
//! population's spread is kept and shifted in logit by its host's iron ([`CoreShareShift`], lean, 2026-11-01): the host's Fe/(Mg + Si), so its
//! \[α/Fe\] and Mg/Si, and not its \[Fe/H\], sets a rocky body's iron per kilogram of rock. Ruling
//! 58's floor beyond the snow line takes the same shift, and a host at the Sun's ratios moves no
//! share, to the bit.

use crate::math;
use crate::planetary::chem::solar_magnesium_to_silicon;
use crate::planetary::derive::radius::EARTH_CORE_MASS_FRACTION;
use crate::planetary::hooks::crust::bulk_iron_mass_fraction;
use crate::units::Dex;

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

/// How far a host's iron moves its rocky bodies' core shares, Δ in logit(cmf): logit(0.325 ρ) −
/// logit(0.325), with ρ the host's solids' iron mass fraction over the Sun's (lean, 2026-11-01).
///
/// ρ is [`bulk_iron_mass_fraction`] at the host's Mg/Si and \[α/Fe\], read as a body's crust reads
/// them, over its value at the Sun's Mg/Si and \[α/Fe\] 0, so that an Earth's core share about the
/// Sun, [`EARTH_CORE_MASS_FRACTION`], becomes 0.325 ρ about the host. The population's draw takes
/// Δ in its logit ([`core_mass_fraction_at`]), and so does ruling 58's floor beyond the snow line
/// ([`about_host`](Self::about_host) of 0.325); P14.T51.k's factor reads a body's share about the
/// Sun's ratios ([`solar_equivalent`](Self::solar_equivalent)), which its two anchors, an Earth
/// and a Mars, are. An α-rich host's ρ is below 1: 0.82 at \[α/Fe\] +0.12 and 0.60 at +0.3, at the
/// Sun's Mg/Si, from P14.T51.k's bulk iron. A host's at the Sun's ratios is
/// [`NONE`](Self::NONE): a shift of 0 moves no share, to the bit.
///
/// # Examples
///
/// The Sun's ratios move no share:
///
/// ```
/// use hyperion_sim::planetary::chem::solar_magnesium_to_silicon;
/// use hyperion_sim::planetary::derive::rocky::CoreShareShift;
/// use hyperion_sim::units::Dex;
///
/// let sun = CoreShareShift::of_host(solar_magnesium_to_silicon(), Dex::new(0.0));
/// assert_eq!(sun, CoreShareShift::NONE);
/// assert_eq!(sun.about_host(0.325).to_bits(), 0.325_f64.to_bits());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct CoreShareShift(f64);

impl CoreShareShift {
    /// No shift: the population's distribution, a host's at the Sun's ratios.
    pub const NONE: Self = Self(0.0);

    /// The shift of a host of Mg/Si `host_magnesium_to_silicon` (by number) and \[α/Fe\]
    /// `alpha_fe`.
    ///
    /// 0.325 ρ is held inside (0, 1), which only a host of over 3.08 times the Sun's iron
    /// fraction would leave, so the shift is finite for every finite host.
    #[must_use]
    pub fn of_host(host_magnesium_to_silicon: f64, alpha_fe: Dex) -> Self {
        let sun = bulk_iron_mass_fraction(solar_magnesium_to_silicon(), Dex::ZERO);
        let rho = bulk_iron_mass_fraction(host_magnesium_to_silicon, alpha_fe) / sun;
        let earth = (EARTH_CORE_MASS_FRACTION * rho).clamp(f64::EPSILON, 1.0 - f64::EPSILON);
        Self(logit(earth) - logit(EARTH_CORE_MASS_FRACTION))
    }

    /// Δ, in logit(cmf): negative about a host poorer in iron than the Sun.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }

    /// The core share, in [0, 1], about the host of a body whose share about the Sun's ratios is
    /// `solar_equivalent`: logistic(logit(`solar_equivalent`) + Δ), and `solar_equivalent` itself
    /// for no shift.
    #[must_use]
    pub fn about_host(self, solar_equivalent: f64) -> f64 {
        if self.0 == 0.0 {
            return solar_equivalent;
        }
        logistic(logit(solar_equivalent) + self.0)
    }

    /// The core share, in [0, 1], about the Sun's ratios of a body whose share about the host is
    /// `core_share`: logistic(logit(`core_share`) − Δ), the inverse of
    /// [`about_host`](Self::about_host), and `core_share` itself for no shift.
    #[must_use]
    pub fn solar_equivalent(self, core_share: f64) -> f64 {
        if self.0 == 0.0 {
            return core_share;
        }
        logistic(logit(core_share) - self.0)
    }
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
/// (Plotnykov and Valencia 2020; see the [module](self) documentation), about a host of shift
/// `shift` ([`CoreShareShift`]).
///
/// `rank` is clamped to 0–1, where the fraction is 0 (pure rock) and 1 (pure iron); inside, it is
/// logistic(logit(0.24) + Δ + s Φ⁻¹(`rank`)), with Δ the host's shift and s the lower scale below
/// the median and the upper scale above it. With no shift it is the population's, to the bit.
///
/// # Examples
///
/// The median, and the percentiles the paper quotes:
///
/// ```
/// use hyperion_sim::planetary::derive::rocky::{CoreShareShift, core_mass_fraction_at};
///
/// let at = |rank| core_mass_fraction_at(rank, CoreShareShift::NONE);
/// assert!((at(0.5) - 0.24).abs() < 1e-12);
/// assert!((at(0.158_655_253_931_457) - 0.06).abs() < 1e-9);
/// assert!((at(0.841_344_746_068_543) - 0.57).abs() < 1e-9);
/// ```
#[must_use]
pub fn core_mass_fraction_at(rank: f64, shift: CoreShareShift) -> f64 {
    if rank.is_nan() || rank <= 0.0 {
        return 0.0;
    }
    if rank >= 1.0 {
        return 1.0;
    }
    let (below, above) = scales();
    let z = math::normal_quantile(rank);
    let scale = if z < 0.0 { below } else { above };
    logistic(logit(MEDIAN_CORE_MASS_FRACTION) + shift.value() + scale * z)
}

/// The rank at which [`core_mass_fraction_at`] gives `core_mass_fraction` about a host of shift
/// `shift`: the distribution's cumulative probability, 0 at pure rock and 1 at pure iron.
#[must_use]
pub fn core_mass_fraction_rank(core_mass_fraction: f64, shift: CoreShareShift) -> f64 {
    if core_mass_fraction.is_nan() || core_mass_fraction <= 0.0 {
        return 0.0;
    }
    if core_mass_fraction >= 1.0 {
        return 1.0;
    }
    let (below, above) = scales();
    let offset = logit(core_mass_fraction) - (logit(MEDIAN_CORE_MASS_FRACTION) + shift.value());
    let scale = if offset < 0.0 { below } else { above };
    0.5 * math::erfc(-offset / scale * core::f64::consts::FRAC_1_SQRT_2)
}

/// The core mass fraction at rank `rank` of the observed distribution about a host of shift
/// `shift`, held to `floor` and above: the distribution conditioned on a core mass fraction of at
/// least `floor` (ruling 58).
///
/// With F the distribution's cumulative probability ([`core_mass_fraction_rank`]), it is
/// [`core_mass_fraction_at`] of F(`floor`) + `rank` (1 − F(`floor`)): `floor` at rank 0, pure iron
/// at rank 1, and increasing between. A floor of 0 is the whole distribution, bit for bit, which
/// is a rocky outcome's inside the snow line; beyond it the floor is Earth's 0.325 about the host
/// ([`CoreShareShift::about_host`]), the Earth-like curve that tops the rocky outcomes there.
///
/// # Examples
///
/// The whole distribution with no floor, and Earth's composition at the bottom of the part held
/// to Earth's and above:
///
/// ```
/// use hyperion_sim::planetary::derive::rocky::{
///     CoreShareShift, core_mass_fraction_at, core_mass_fraction_at_least,
/// };
///
/// let none = CoreShareShift::NONE;
/// let held = |rank, floor| core_mass_fraction_at_least(rank, floor, none);
/// assert!((held(0.3, 0.0) - core_mass_fraction_at(0.3, none)).abs() < 1e-15);
/// assert!((held(0.0, 0.325) - 0.325).abs() < 1e-12);
/// assert!(held(0.5, 0.325) > core_mass_fraction_at(0.5, none));
/// ```
#[must_use]
pub fn core_mass_fraction_at_least(rank: f64, floor: f64, shift: CoreShareShift) -> f64 {
    let below = core_mass_fraction_rank(floor, shift);
    core_mass_fraction_at(below + rank * (1.0 - below), shift)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// No host's shift: the population's distribution, as built.
    const NONE: CoreShareShift = CoreShareShift::NONE;

    #[test]
    fn the_distribution_passes_through_the_quoted_percentiles() {
        let one_sigma = 0.5 * math::erfc(core::f64::consts::FRAC_1_SQRT_2);
        for (rank, cmf) in [
            (one_sigma, LOW_CORE_MASS_FRACTION),
            (0.5, MEDIAN_CORE_MASS_FRACTION),
            (1.0 - one_sigma, HIGH_CORE_MASS_FRACTION),
        ] {
            let got = core_mass_fraction_at(rank, NONE);
            assert!((got - cmf).abs() < 1e-12, "{rank}: {got}");
        }
    }

    #[test]
    fn the_rank_and_the_fraction_invert_each_other_and_rise_together() {
        let mut previous = 0.0;
        for i in 1..1_000_u32 {
            let rank = f64::from(i) / 1_000.0;
            let cmf = core_mass_fraction_at(rank, NONE);
            assert!(cmf > previous && cmf < 1.0, "{rank}: {cmf}");
            assert!(
                (core_mass_fraction_rank(cmf, NONE) - rank).abs() < 1e-12,
                "{rank}"
            );
            previous = cmf;
        }
        assert!(core_mass_fraction_at(0.0, NONE).abs() < f64::EPSILON);
        assert!((core_mass_fraction_at(1.0, NONE) - 1.0).abs() < f64::EPSILON);
        assert!(core_mass_fraction_at(1e-300, NONE) < 1e-12);
        assert!(core_mass_fraction_at(1.0 - 1e-16, NONE) > 0.999);
    }

    #[test]
    fn the_distribution_held_to_a_floor_rises_from_the_floor_to_iron() {
        let floor = crate::planetary::derive::radius::EARTH_CORE_MASS_FRACTION;
        let mut previous = 0.0;
        for i in 0..=1_000_u32 {
            let rank = f64::from(i) / 1_000.0;
            let whole = core_mass_fraction_at(rank, NONE);
            // No floor is the whole distribution, bit for bit.
            assert_same_bits(core_mass_fraction_at_least(rank, 0.0, NONE), whole);
            let held = core_mass_fraction_at_least(rank, floor, NONE);
            assert!(held >= floor - 1e-12 && held >= previous, "{rank}: {held}");
            // The conditional distribution: its rank is the whole one's, rescaled.
            if i > 0 && i < 1_000 {
                let below = core_mass_fraction_rank(floor, NONE);
                let conditional = (core_mass_fraction_rank(held, NONE) - below) / (1.0 - below);
                assert!((conditional - rank).abs() < 1e-9, "{rank}: {conditional}");
            }
            previous = held;
        }
        assert!((core_mass_fraction_at_least(0.0, floor, NONE) - floor).abs() < 1e-12);
        assert!((core_mass_fraction_at_least(1.0, floor, NONE) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn earth_and_mercury_sit_where_the_documentation_says() {
        let earth = core_mass_fraction_rank(0.325, NONE);
        let mercury = core_mass_fraction_rank(0.70, NONE);
        assert!((earth - 0.616).abs() < 0.001, "{earth}");
        assert!((mercury - 0.918).abs() < 0.001, "{mercury}");
        let over_half = 1.0 - core_mass_fraction_rank(0.5, NONE);
        assert!((over_half - 0.21).abs() < 0.01, "{over_half}");
    }

    /// Under every 27 hold a host at the Sun's ratios moves no share, to the bit: its shift is 0,
    /// and the draw, its rank, its floor and their inverses are the population's.
    #[test]
    fn under_27_the_sun_s_ratios_move_no_share() {
        use crate::planetary::testing::with_every_27_hold;
        with_every_27_hold(|| {
            let sun = CoreShareShift::of_host(solar_magnesium_to_silicon(), Dex::ZERO);
            assert_same_bits(sun.value(), 0.0);
            for i in 0..=1_000_u32 {
                let rank = f64::from(i) / 1_000.0;
                assert_same_bits(
                    core_mass_fraction_at(rank, sun),
                    core_mass_fraction_at(rank, NONE),
                );
                assert_same_bits(
                    core_mass_fraction_rank(rank, sun),
                    core_mass_fraction_rank(rank, NONE),
                );
                assert_same_bits(
                    core_mass_fraction_at_least(rank, EARTH_CORE_MASS_FRACTION, sun),
                    core_mass_fraction_at_least(rank, EARTH_CORE_MASS_FRACTION, NONE),
                );
                assert_same_bits(sun.about_host(rank), rank);
                assert_same_bits(sun.solar_equivalent(rank), rank);
            }
        });
    }

    /// Under every 27 hold an α-rich host's iron moves an Earth's core share from 0.325 to 0.325 ρ:
    /// ρ 0.822 at \[α/Fe\] +0.12 and 0.596 at +0.3 at the Sun's Mg/Si (the research's
    /// reproduction of version 27's bulk iron); the share about the Sun's ratios inverts it. At a
    /// fixed Fe/(Mg + Si) a Mg-rich host's rock is lighter per atom (`MgO` for `SiO₂`), so its
    /// solids hold a little more iron: its shift is small and positive.
    #[test]
    fn under_27_an_alpha_rich_host_s_earth_holds_less_iron() {
        use crate::planetary::testing::with_every_27_hold;
        with_every_27_hold(|| {
            let solar = solar_magnesium_to_silicon();
            for (alpha, rho) in [(0.12, 0.822), (0.2, 0.715), (0.3, 0.596)] {
                let shift = CoreShareShift::of_host(solar, Dex::new(alpha));
                assert!(shift.value() < 0.0, "{alpha}");
                let earth = shift.about_host(EARTH_CORE_MASS_FRACTION);
                assert!(
                    (earth / EARTH_CORE_MASS_FRACTION - rho).abs() < 5e-4,
                    "{alpha}: {earth}"
                );
                let back = shift.solar_equivalent(earth);
                assert!((back - EARTH_CORE_MASS_FRACTION).abs() < 1e-14, "{back}");
                // The floor's rank is the population's: the shift translates the distribution.
                let floor_rank = core_mass_fraction_rank(earth, shift);
                let population = core_mass_fraction_rank(EARTH_CORE_MASS_FRACTION, NONE);
                assert!((floor_rank - population).abs() < 1e-12, "{floor_rank}");
            }
            let (silicon_rich, magnesium_rich) = (
                CoreShareShift::of_host(0.7, Dex::ZERO),
                CoreShareShift::of_host(1.5, Dex::ZERO),
            );
            assert!(
                (-0.05..0.0).contains(&silicon_rich.value())
                    && (0.0..0.05).contains(&magnesium_rich.value()),
                "{silicon_rich:?} {magnesium_rich:?}"
            );
        });
    }

    /// Under every 27 hold the draw is finite, inside (0, 1) and rises with the rank, and its
    /// floor beyond the snow line lies inside (0, 1), over host Mg/Si of 0.7–1.5 and \[α/Fe\] of
    /// −0.05 to +0.4.
    #[test]
    fn under_27_the_draw_rises_with_its_rank_about_every_host() {
        use crate::planetary::testing::with_every_27_hold;
        with_every_27_hold(|| {
            for ratio in [0.7, 0.9, 1.1, 1.3, 1.5] {
                for alpha in [-0.05, 0.0, 0.1, 0.2, 0.3, 0.4] {
                    let shift = CoreShareShift::of_host(ratio, Dex::new(alpha));
                    assert!(shift.value().is_finite(), "{ratio} {alpha}");
                    let floor = shift.about_host(EARTH_CORE_MASS_FRACTION);
                    assert!(floor > 0.0 && floor < 1.0, "{ratio} {alpha}: {floor}");
                    let mut previous = 0.0;
                    for i in 1..2_000_u32 {
                        let rank = f64::from(i) / 2_000.0;
                        for cmf in [
                            core_mass_fraction_at(rank, shift),
                            core_mass_fraction_at_least(rank, floor, shift),
                        ] {
                            assert!(cmf.is_finite() && cmf > 0.0 && cmf < 1.0, "{rank}: {cmf}");
                        }
                        let cmf = core_mass_fraction_at(rank, shift);
                        assert!(cmf > previous, "{ratio} {alpha} {rank}: {cmf}");
                        previous = cmf;
                    }
                }
            }
        });
    }
}
