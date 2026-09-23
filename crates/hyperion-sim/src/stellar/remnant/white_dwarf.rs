//! White dwarfs (plan 06, P06.T20): how they cool.
//!
//! Until P06.T20.a builds the cooling of Hurley and Shara (2003) and the spectral types, this
//! holds the cooling law of Hurley, Pols and Tout (2000, MNRAS 315, 543, "HPT", section 6.2.1,
//! equation 90), which the track's remnant stage and HPT section 6.3's small-envelope
//! perturbation need (ruling 33 of 2026-09-22). It serves both recipes until T20.a replaces it
//! under the modern one, with a generator-version bump; `RemnantRecipe::Hurley2000` keeps it for
//! good, since P06.T12.b compares with the published SSE code.

use crate::math;
use crate::stellar::Phase;
use crate::units::{MetalFraction, SolarLuminosities, SolarMasses, Years};

/// What a white dwarf is made of, which sets the effective baryon number A of HPT's cooling law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WhiteDwarfCore {
    /// Helium: A = 4.
    Helium,
    /// Carbon and oxygen: A = 15.
    CarbonOxygen,
    /// Oxygen and neon: A = 17.
    OxygenNeon,
}

impl WhiteDwarfCore {
    /// The white dwarf of `phase`, or `None` if it is not a white dwarf's.
    #[must_use]
    pub(crate) const fn of(phase: Phase) -> Option<Self> {
        match phase {
            Phase::HeliumWhiteDwarf => Some(Self::Helium),
            Phase::CarbonOxygenWhiteDwarf => Some(Self::CarbonOxygen),
            Phase::OxygenNeonWhiteDwarf => Some(Self::OxygenNeon),
            Phase::Protostar
            | Phase::PreMainSequence
            | Phase::MainSequence
            | Phase::HertzsprungGap
            | Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::HeliumMainSequence
            | Phase::HeliumHertzsprungGap
            | Phase::HeliumGiantBranch
            | Phase::PostAgb
            | Phase::NeutronStar
            | Phase::BlackHole
            | Phase::NoRemnant
            | Phase::Substellar => None,
        }
    }

    /// HPT's effective baryon number A for the composition (section 6.2.1): 4, 15 or 17.
    #[must_use]
    const fn baryon_number(self) -> f64 {
        match self {
            Self::Helium => 4.0,
            Self::CarbonOxygen => 15.0,
            Self::OxygenNeon => 17.0,
        }
    }
}

/// The luminosity of a white dwarf of `mass` and composition `core`, `cooling_age` after it
/// formed, from a star of metal fraction `z`, by HPT's form of Mestel cooling (equation 90):
/// L = 635 M Z^0.4 ÷ (A (t + 0.1))^1.4 L☉, with t in Myr.
///
/// HPT add the 0.1 Myr so that the law starts at a finite luminosity, as if at a cooling age of
/// 10⁵ years, and note that it is adequate for old white dwarfs; the small-envelope perturbation of
/// their section 6.3 evaluates it at t = 0 with A = 4 or 15 for the white dwarf a giant's core
/// would become. The published SSE code carries 16 for A of both carbon–oxygen and oxygen–neon
/// dwarfs where the paper prints 15 and 17, and its distributed input file selects the cooling of
/// Hurley and Shara (2003) instead; P06.T12.b does not compare white-dwarf luminosities.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive or `cooling_age` is negative.
#[must_use]
pub(crate) fn hpt_luminosity(
    core: WhiteDwarfCore,
    mass: SolarMasses,
    cooling_age: Years,
    z: MetalFraction,
) -> SolarLuminosities {
    debug_assert!(
        mass.value() > 0.0 && cooling_age.value() >= 0.0,
        "a white dwarf of {mass:?} at a cooling age of {cooling_age:?}"
    );
    let t_myr = cooling_age.value() * 1e-6;
    SolarLuminosities::new(
        635.0 * mass.value() * math::powf(z.value(), 0.4)
            / math::powf(core.baryon_number() * (t_myr + 0.1), 1.4),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 0.6 M☉ carbon–oxygen dwarf at solar Z: 635 × 0.6 × 0.02^0.4 ÷ (15 × 0.1)^1.4 = 45.2 L☉
    /// at formation and 3.2 × 10⁻⁴ L☉ after 10 Gyr (Python's double precision), fainter with age
    /// and for a heavier nucleus.
    #[test]
    fn a_carbon_oxygen_dwarf_follows_equation_90() {
        let z = MetalFraction::new(0.02);
        let m = SolarMasses::new(0.6);
        let at = |core, years| hpt_luminosity(core, m, Years::new(years), z).value();
        let young = at(WhiteDwarfCore::CarbonOxygen, 0.0);
        assert!(
            (young / 45.165_748_435_515_77 - 1.0).abs() < 1e-12,
            "{young}"
        );
        let old = at(WhiteDwarfCore::CarbonOxygen, 1e10);
        let expected = 635.0 * 0.6 * math::powf(0.02, 0.4) / math::powf(15.0 * 10_000.1, 1.4);
        assert!((old / expected - 1.0).abs() < 1e-14, "{old}");
        assert!(old < young);
        assert!(at(WhiteDwarfCore::Helium, 1e8) > at(WhiteDwarfCore::CarbonOxygen, 1e8));
        assert!(at(WhiteDwarfCore::CarbonOxygen, 1e8) > at(WhiteDwarfCore::OxygenNeon, 1e8));
    }

    #[test]
    fn only_white_dwarf_phases_have_a_white_dwarf_core() {
        let dwarfs = Phase::ALL
            .into_iter()
            .filter(|p| WhiteDwarfCore::of(*p).is_some())
            .count();
        assert_eq!(dwarfs, 3);
    }
}
