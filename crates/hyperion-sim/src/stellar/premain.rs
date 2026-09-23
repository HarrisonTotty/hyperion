//! Before the main sequence: the protostar phase (Class 0 and I, with a closed form for its growth
//! in mass) and the contraction tracks ahead of the zero-age main sequence, which make the T Tauri
//! and Herbig Ae/Be stars (plan 06, P06.T15; the brainstorm's "Covering every class of star",
//! row "Before the main sequence").
//!
//! Only the disc-lifetime law of P06.T15.c is built so far, because plan 14's disc is its first
//! caller (ruling 33 of 2026-09-22, "One disc lifetime per circumstellar disc"): a star's T Tauri
//! class (P06.T24) and its planets' formation (P14.T3) read the same disc, so there is one
//! lifetime, [`disc_lifetime`], of the star's own [`StarDraws::disc_lifetime`] rank. It draws
//! nothing. The protostar and contraction segments (P06.T15.a and b) are not built.
//!
//! [`StarDraws::disc_lifetime`]: crate::stellar::draws::StarDraws::disc_lifetime

use crate::math;
use crate::stellar::draws::UnitUniform;
use crate::units::{Megayears, SolarMasses};

/// The mean lifetime of a solar-mass star's protostellar disc: 2.5 Myr.
///
/// Mamajek (2009, AIP Conf. Proc. 1158, 3), Fig. 1 and eq. 1: the fraction of young stars in 22
/// clusters and groups with optically thick primordial discs, or with spectroscopic signs of
/// accretion, falls with age as exp(−t ÷ τ) with a best-fit e-folding time τ = 2.5 Myr (half-life
/// 1.7 Myr), to about 10% (dropping any one cluster moves τ by less than that). A disc fraction
/// that decays exponentially is the survival function of an exponential distribution of
/// lifetimes, which is why the law is exponential and why its mean is τ (ruling 33).
pub const DISC_LIFETIME_MEAN_SOLAR: Megayears = Megayears::new(2.5);

/// The shortest disc lifetime: 0.3 Myr.
///
/// The age of NGC 2024, the youngest cluster of Mamajek's (2009) Fig. 1, where nearly every star
/// still has its disc. Plan 06's interim bound (P06.T15.c), kept by ruling 33.
pub const DISC_LIFETIME_MIN: Megayears = Megayears::new(0.3);

/// The longest disc lifetime: 15 Myr.
///
/// Within the ages of the oldest known accretors Mamajek (2009) lists, of about 7–17 Myr and
/// 8–25 Myr. Plan 06's interim bound (P06.T15.c), kept by ruling 33.
pub const DISC_LIFETIME_MAX: Megayears = Megayears::new(15.0);

/// How slowly the mean disc lifetime falls with mass up to a solar mass: τ ∝ m^−0.1 (ruling 38).
///
/// Luhman et al. (2005, ApJ 631, L69, abstract) find disc fractions of 42 ± 13% and 50 ± 17% for
/// the brown dwarfs of IC 348 and Chamaeleon I (below about 0.08 M☉) against 33 ± 4% and 45 ± 7%
/// for their M0–M6 stars (0.1–0.7 M☉). At one age an exponential's τ goes as −1 ÷ ln f, so the
/// brown dwarfs' discs live 1.28 and 1.15 times as long across a factor of about six in mass, an
/// exponent of 0.14 and 0.08. With it a 0.05 M☉ brown dwarf's mean is 3.4 Myr, against the
/// 2.4–5.9 Myr (typically 3) Mamajek (2009) derives for four clusters' brown dwarfs.
pub const DISC_LIFETIME_LOW_MASS_EXPONENT: f64 = 0.1;

/// How fast the mean disc lifetime falls with mass above a solar mass: τ ∝ m^−1.06 (ruling 38).
///
/// Ribas, Bouy and Merín (2015, A&A 576, A52, Table 3) measure protoplanetary disc fractions of
/// 63 ± 2% and 38 ± 6% for stars below and above 2 M☉ at 1–3 Myr, and 17 ± 2% and 2% at 3–11 Myr:
/// at one age the lifetimes on the two sides of 2 M☉ differ by ln 0.38 ÷ ln 0.63 = 2.09 and
/// ln 0.02 ÷ ln 0.17 = 2.2. Mamajek (2009) gives τ ≈ 1.2 Myr for stars above 1.3 M☉. The exponent
/// halves the lifetime between 1 and 2 M☉, 2.5 Myr × 2^−1.06 = 1.20 Myr, so the law meets both.
/// Above about 7 M☉ the mean is below the 0.3 Myr floor.
pub const DISC_LIFETIME_HIGH_MASS_EXPONENT: f64 = 1.06;

/// The mean lifetime of the protostellar disc of a star of initial mass `mass`, before
/// [`disc_lifetime`] holds a lifetime to 0.3–15 Myr: 2.5 Myr × (m ÷ M☉)^−0.1 up to a solar mass
/// and 2.5 Myr × (m ÷ M☉)^−1.06 above it (ruling 38).
///
/// Nearly flat from brown dwarfs to the Sun, as Luhman et al. (2005) find
/// ([`DISC_LIFETIME_LOW_MASS_EXPONENT`]), and halving by 2 M☉, as Ribas et al. (2015) and Mamajek
/// (2009) find ([`DISC_LIFETIME_HIGH_MASS_EXPONENT`]). The normalisation is Mamajek's 2.5 Myr at a
/// solar mass ([`DISC_LIFETIME_MEAN_SOLAR`]); Ribas et al.'s own fit to all their stars, 2.7 ±
/// 0.7 Myr for inner-disc excesses (their Table A.2), agrees within its error. The two pieces meet
/// at 1 M☉, where both are 2.5 Myr exactly, and the mean never rises with mass.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
#[must_use]
pub fn disc_lifetime_mean(mass: SolarMasses) -> Megayears {
    debug_assert!(
        mass.value().is_finite() && mass.value() > 0.0,
        "a star's mass is positive and finite, got {}",
        mass.value()
    );
    let m = mass.value();
    let exponent = if m <= 1.0 {
        DISC_LIFETIME_LOW_MASS_EXPONENT
    } else {
        DISC_LIFETIME_HIGH_MASS_EXPONENT
    };
    DISC_LIFETIME_MEAN_SOLAR * math::powf(m, -exponent)
}

/// The lifetime of the protostellar disc of a star of initial mass `mass` whose disc-lifetime rank
/// is `rank`: exponential with mean [`disc_lifetime_mean`], held to [`DISC_LIFETIME_MIN`]–
/// [`DISC_LIFETIME_MAX`] (plan 06, P06.T15.c; ruling 33).
///
/// The lifetime is the exponential quantile of the rank, −τ ln(1 − u), so it rises with the rank,
/// and the median rank gives τ ln 2 (1.73 Myr at 1 M☉, Mamajek's half-life). The rank is the
/// star's own [`StarDraws::disc_lifetime`] for a circumstellar disc; a circumbinary disc's comes
/// from plan 14's `planet.disc` stream, at the pair's total mass. The function draws nothing.
///
/// [`StarDraws::disc_lifetime`]: crate::stellar::draws::StarDraws::disc_lifetime
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
///
/// # Examples
///
/// The median star's disc lasts τ ln 2; an M dwarf's lasts a little longer, and an A star's half
/// as long:
///
/// ```
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::stellar::premain::disc_lifetime;
/// use hyperion_sim::units::SolarMasses;
///
/// let median = |m: f64| disc_lifetime(SolarMasses::new(m), UnitUniform::HALF).value();
/// assert!((median(1.0) - 2.5 * core::f64::consts::LN_2).abs() < 1e-12);
/// assert!(median(0.3) > median(1.0) && median(0.3) < 1.2 * median(1.0));
/// assert!((median(2.0) / median(1.0) - 0.48).abs() < 0.01);
/// ```
#[must_use]
pub fn disc_lifetime(mass: SolarMasses, rank: UnitUniform) -> Megayears {
    let mean = disc_lifetime_mean(mass).value();
    // −ln(1 − u) through ln_1p keeps the short lifetimes of small ranks accurate.
    let lifetime = -mean * math::ln_1p(-rank.value());
    Megayears::new(lifetime.clamp(DISC_LIFETIME_MIN.value(), DISC_LIFETIME_MAX.value()))
}

#[cfg(test)]
mod tests {
    use core::f64::consts::LN_2;

    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn rank(u: f64) -> UnitUniform {
        UnitUniform::new(u).expect("a rank in (0, 1)")
    }

    fn mean(m: f64) -> f64 {
        disc_lifetime_mean(SolarMasses::new(m)).value()
    }

    #[test]
    fn the_mean_is_mamajek_s_two_and_a_half_megayears_at_a_solar_mass() {
        assert_same_bits(mean(1.0), 2.5);
        // Continuous where the two exponents meet.
        assert!((mean(1.0 - 1e-12) - 2.5).abs() < 1e-11);
        assert!((mean(1.0 + 1e-12) - 2.5).abs() < 1e-11);
    }

    #[test]
    fn the_mean_follows_the_measured_mass_dependence() {
        // Mamajek's (2009) 1.2 Myr above 1.3 M☉, reached at 2 M☉.
        assert!((mean(2.0) - 1.2).abs() < 0.005, "{}", mean(2.0));
        // Ribas et al.'s (2015) factor of 2.09–2.2 across 2 M☉.
        let ratio = mean(1.0) / mean(2.0);
        assert!((2.05..2.25).contains(&ratio), "{ratio}");
        // Luhman et al.'s (2005) brown dwarfs outlast their M stars by 1.15–1.28 across a factor
        // of six in mass.
        let dwarfs = mean(0.05) / mean(0.3);
        assert!((1.15..1.28).contains(&dwarfs), "{dwarfs}");
        // Mamajek's brown dwarfs, about 3 Myr (2.4–5.9 over four clusters).
        assert!((2.4..5.9).contains(&mean(0.05)), "{}", mean(0.05));
        // The mean never rises with mass.
        let mut previous = f64::INFINITY;
        for k in 1..2_000_u32 {
            let m = 0.01 * f64::from(k);
            assert!(mean(m) <= previous, "{m} M☉");
            previous = mean(m);
        }
    }

    #[test]
    fn the_median_rank_gives_the_half_life() {
        for m in [0.3, 1.0, 2.0] {
            let mean = disc_lifetime_mean(SolarMasses::new(m)).value();
            let median = disc_lifetime(SolarMasses::new(m), UnitUniform::HALF).value();
            assert!(
                (median - mean * LN_2).abs() < 1e-12 * mean,
                "{m} M☉: {median} against {}",
                mean * LN_2
            );
        }
        // Mamajek's (2009) half-life of 1.7 Myr, at a solar mass.
        let sun = disc_lifetime(SolarMasses::new(1.0), UnitUniform::HALF).value();
        assert!((sun - 1.733).abs() < 1e-3, "{sun}");
    }

    #[test]
    fn a_rank_maps_to_its_exponential_quantile() {
        for u in [0.2, 0.5, 0.9, 0.99] {
            let t = disc_lifetime(SolarMasses::new(1.0), rank(u)).value();
            // The survival function of the lifetime at t is the rank's complement.
            let survival = math::exp(-t / 2.5);
            assert!((survival - (1.0 - u)).abs() < 1e-12, "rank {u}: {t} Myr");
        }
    }

    #[test]
    fn lifetimes_are_held_to_the_clamps() {
        let sun = SolarMasses::new(1.0);
        assert_same_bits(disc_lifetime(sun, rank(1e-12)).value(), 0.3);
        assert_same_bits(disc_lifetime(sun, rank(1.0 - 1e-12)).value(), 15.0);
        // The rank at which a solar-mass disc reaches each clamp: 1 − e^(−0.12) and 1 − e^(−6).
        let low = 1.0 - math::exp(-0.3 / 2.5);
        assert!(disc_lifetime(sun, rank(low - 1e-9)).value() <= 0.3);
        assert!(disc_lifetime(sun, rank(low + 1e-9)).value() > 0.3);
        let high = 1.0 - math::exp(-15.0 / 2.5);
        assert!(disc_lifetime(sun, rank(high - 1e-9)).value() < 15.0);
        assert!(disc_lifetime(sun, rank(high + 1e-9)).value() >= 15.0);
        // A massive star's mean is below the floor at every rank below the floor's quantile.
        let massive = disc_lifetime(SolarMasses::new(100.0), UnitUniform::HALF);
        assert_same_bits(massive.value(), DISC_LIFETIME_MIN.value());
        for m in [0.08, 0.1, 0.5, 1.0, 3.0, 20.0, 150.0] {
            for u in [1e-15, 0.001, 0.3, 0.7, 0.999, 1.0 - 1e-15] {
                let t = disc_lifetime(SolarMasses::new(m), rank(u)).value();
                assert!((0.3..=15.0).contains(&t), "{m} M☉, rank {u}: {t} Myr");
            }
        }
    }

    #[test]
    fn the_lifetime_never_falls_as_the_rank_rises() {
        for m in [0.1, 1.0, 8.0] {
            let mut previous = 0.0;
            for k in 1..10_000_u32 {
                let u = f64::from(k) / 10_000.0;
                let t = disc_lifetime(SolarMasses::new(m), rank(u)).value();
                assert!(t >= previous, "{m} M☉: {t} at rank {u} after {previous}");
                previous = t;
            }
        }
    }

    #[test]
    fn a_heavier_star_never_keeps_its_disc_longer_at_the_same_rank() {
        for u in [0.05, 0.5, 0.95] {
            let mut previous = f64::INFINITY;
            for m in [0.08, 0.2, 0.5, 1.0, 2.0, 5.0, 20.0] {
                let t = disc_lifetime(SolarMasses::new(m), rank(u)).value();
                assert!(t <= previous, "rank {u}: {t} Myr at {m} M☉");
                previous = t;
            }
        }
    }
}
