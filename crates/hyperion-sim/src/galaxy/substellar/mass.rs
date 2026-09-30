//! The two substellar mass functions and their draws (plan 13, P13.T1).
//!
//! A brown dwarf's mass comes from the substellar branch of the universe's own mass function
//! ([`MassFunction::substellar_quantile_in`]) between 13 `M_Jup` and 0.08 M☉, and a rogue planet's from
//! Sumi et al.'s (2023) power law between ⅓ M⊕ and 13 `M_Jup`. Each is one uniform on the object's
//! `substellar.mass` stream (Design note 11), which the caller opens.

use super::params::{BROWN_DWARF_MIN_MSUN, SubstellarParams, rogue_planets_above_per_star};
use crate::galaxy::imf::{MASS_LIMIT_LO, MassFunction};
use crate::rng::Stream;
use crate::units::{EarthMasses, SolarMasses};

/// A brown dwarf's mass: one uniform through the substellar branch of `f`, restricted to
/// 13 `M_Jup`–0.08 M☉ (plan 13, Design note 6).
///
/// Only the branch's shape is used: Kroupa's (2001) dN ÷ dm ∝ m^−0.3, or the continuation of
/// Chabrier's (2003) log-normal. The count is the abundance parameter's.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::imf::Kroupa;
/// use hyperion_sim::galaxy::substellar::draw_brown_dwarf_mass;
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
///
/// let mut stream = Stream::open(Seed::new(5), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let mass = draw_brown_dwarf_mass(&Kroupa, &mut stream);
/// assert!((0.0124..=0.08).contains(&mass.value()));
/// assert_eq!(stream.position(), 1);
/// ```
#[must_use]
pub fn draw_brown_dwarf_mass(f: &dyn MassFunction, s: &mut Stream) -> SolarMasses {
    SolarMasses::new(f.substellar_quantile_in(BROWN_DWARF_MIN_MSUN, MASS_LIMIT_LO, s.uniform()))
}

/// A rogue planet's mass: one uniform through dN ÷ dM ∝ M^−1.96 on ⅓ M⊕–13 `M_Jup` (plan 13,
/// Design note 7).
///
/// Placement draws through the galaxy's own copy of [`SubstellarParams::rogue_mass_law`], built
/// once with the galaxy; this builds the same law for the call, so both give the same bits.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::substellar::{SubstellarParams, draw_rogue_planet_mass};
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
/// use hyperion_sim::units::EarthMasses;
///
/// let p = SubstellarParams::generator_default();
/// let mut stream = Stream::open(Seed::new(5), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let mass = EarthMasses::from(draw_rogue_planet_mass(&p, &mut stream));
/// // Most rogue planets are of the order of an Earth mass or less.
/// assert!((0.33..4_132.0).contains(&mass.value()));
/// ```
#[must_use]
pub fn draw_rogue_planet_mass(p: &SubstellarParams, s: &mut Stream) -> SolarMasses {
    SolarMasses::new(s.power_law(&p.rogue_mass_law()))
}

/// Rogue planets per star above `m`, before the per-galaxy cap: the closed-form integral of
/// Sumi et al.'s (2023) mass function from `m` to 13 `M_Jup`, `m` clamped to the band.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::substellar::{SubstellarParams, rogue_planets_above};
/// use hyperion_sim::units::{EarthMasses, JupiterMasses};
///
/// let p = SubstellarParams::generator_default();
/// // Jupiters stay under one for every four stars (Mróz et al. 2017).
/// let jupiters = rogue_planets_above(&p, EarthMasses::from(JupiterMasses::new(0.3)));
/// assert!(jupiters < 0.25);
/// ```
#[must_use]
pub fn rogue_planets_above(p: &SubstellarParams, m: EarthMasses) -> f64 {
    rogue_planets_above_per_star(p, m)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::Seed;
    use crate::galaxy::imf::{Chabrier, Kroupa};
    use crate::galaxy::substellar::params::{
        ROGUE_PLANET_MAX_MSUN, ROGUE_PLANET_MIN, ROGUE_PLANET_MIN_MSUN,
    };
    use crate::rng::{ObjectKey, tags};
    use crate::units::JupiterMasses;

    const DRAWS: u32 = 100_000;

    fn stream(item: u64) -> Stream {
        Stream::open(
            Seed::new(0x1300_0001),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy_item(item),
        )
    }

    /// 10⁵ brown-dwarf masses per mass function pass the Kolmogorov–Smirnov test against the
    /// branch's own distribution, and every one lies in the band.
    #[test]
    fn brown_dwarf_masses_follow_the_substellar_branch() {
        let functions: [(&str, &dyn MassFunction); 2] =
            [("kroupa", &Kroupa), ("chabrier", &Chabrier::provisional())];
        for (item, (name, f)) in (0_u64..).zip(functions) {
            let mut s = stream(item);
            let mut masses: Vec<f64> = (0..DRAWS)
                .map(|_| draw_brown_dwarf_mass(f, &mut s).value())
                .collect();
            assert_eq!(s.position(), u64::from(DRAWS), "one word per draw");
            assert!(
                masses
                    .iter()
                    .all(|m| (BROWN_DWARF_MIN_MSUN..=MASS_LIMIT_LO).contains(m)),
                "{name}"
            );
            let total = f.substellar_integral(BROWN_DWARF_MIN_MSUN, MASS_LIMIT_LO);
            let ks = ks_one_sample(&mut masses, |m| {
                (f.substellar_integral(BROWN_DWARF_MIN_MSUN, m) / total).clamp(0.0, 1.0)
            });
            assert_p_value(&format!("{name} brown dwarfs"), ks.p_value, ALPHA);
        }
    }

    /// 10⁵ rogue-planet masses pass the Kolmogorov–Smirnov test against the closed-form count
    /// above, the share below Neptune's 17 M⊕ is over 95%, and every mass lies in the band.
    #[test]
    fn rogue_planet_masses_follow_sumi_s_law() {
        let p = SubstellarParams::generator_default();
        let mut s = stream(7);
        let mut masses: Vec<f64> = (0..DRAWS)
            .map(|_| draw_rogue_planet_mass(&p, &mut s).value())
            .collect();
        assert_eq!(s.position(), u64::from(DRAWS), "one word per draw");
        assert!(
            masses
                .iter()
                .all(|m| (ROGUE_PLANET_MIN_MSUN..=ROGUE_PLANET_MAX_MSUN).contains(m))
        );
        let all = p.rogue_planets_per_star();
        let cdf = |m: f64| {
            let above = rogue_planets_above(&p, EarthMasses::from(SolarMasses::new(m)));
            (1.0 - above / all).clamp(0.0, 1.0)
        };
        let ks = ks_one_sample(&mut masses, cdf);
        assert_p_value("rogue planets", ks.p_value, ALPHA);
        let below_neptune = 1.0 - rogue_planets_above(&p, EarthMasses::new(17.0)) / all;
        assert!(below_neptune > 0.95, "{below_neptune}");
        assert!(below_neptune < 0.99, "{below_neptune}");
    }

    /// Mróz et al. (2017, Nature 548, 183): fewer than 0.25 Jupiter-mass rogue planets per star
    /// (95% upper limit); the law gives about 0.09 above 0.3 `M_Jup` (Design note 7).
    #[test]
    fn jupiters_stay_under_one_for_every_four_stars() {
        let p = SubstellarParams::generator_default();
        let above = rogue_planets_above(&p, EarthMasses::from(JupiterMasses::new(0.3)));
        assert!((0.07..0.25).contains(&above), "{above}");
        assert!(rogue_planets_above(&p, ROGUE_PLANET_MIN) > 20.99);
    }
}
