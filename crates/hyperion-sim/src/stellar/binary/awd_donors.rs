//! The novae of a white dwarf fed helium (plan 11, P11.T5's fix).
//!
//! As first built, `nova_recurrence_period` gave every accreting white dwarf hydrogen's ignition
//! mass, the fit to Yaron et al.'s (2005, ApJ 623, 398) hydrogen novae, whatever its donor gave
//! it. A white dwarf fed by a helium star or a helium white dwarf (BSE's donor types 7–10, whose
//! surface is helium: the AM Canum Venaticorum stars and the helium-star channel) accretes no
//! hydrogen and has no hydrogen nova. Helium ignites only under a far heavier envelope, and a helium nova is
//! rare: V445 Puppis is the one known (Ashok and Banerjee 2003, A&A 409, 1007; Kato et al. 2008,
//! ApJ 684, 1366). research-p15-t10b-rates found 77 such hosts among the engine's 1,814 fast ones
//! in the fit's slab census, 12 of them helium stars at 2–5 × 10⁻⁸ M☉ yr⁻¹ onto 1.0–1.5 M☉ dwarfs
//! that recur every 6–600 years and make 55% of its novae.
//!
//! So a white dwarf fed helium (by a donor of [`Phase::HeliumMainSequence`],
//! [`Phase::HeliumHertzsprungGap`], [`Phase::HeliumGiantBranch`] or [`Phase::HeliumWhiteDwarf`], as
//! the census's `AccretingMarks::donor_kind` reads it) has no nova: its recurrence is infinite, so
//! it is a slow host. No helium-flash law is taken, since the class's novae are hydrogen's (the
//! brief's lean of 2026-11-01, recorded in plan 11): an approximation, which leaves out a helium
//! nova like V445 Puppis's, far under 1% of the Galaxy's novae.
//!
//! The engine reads it at one choke point (`classify::nova_recurrence_under`, read by
//! `carved_class` and the census's marks), and the catalogue's fast scan refuses a candidate fed
//! helium (`AwdProcess::has_no_nova`), so that the scan keeps exactly the class's entries.

use crate::stellar::Phase;

/// Whether a donor of `phase` feeds its white dwarf helium: a helium star's or a helium white
/// dwarf's surface (BSE's types 7–10, the `Surface::Helium` of the engine's transfer). A carbon–
/// oxygen or oxygen–neon dwarf's matter makes no accreting white dwarf (`transfer_class`), so no
/// accreting white dwarf is fed by one.
#[must_use]
pub(crate) const fn feeds_helium(phase: Phase) -> bool {
    match phase {
        Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::HeliumWhiteDwarf => true,
        Phase::Protostar
        | Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::PostAgb
        | Phase::CarbonOxygenWhiteDwarf
        | Phase::OxygenNeonWhiteDwarf
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant
        | Phase::Substellar => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stellar::binary::star::{Kind, Surface};
    use crate::stellar::binary::testing::AccretingMarks;
    use crate::tables::binary::AwdDonorKind;
    use crate::units::{SolarMasses, SolarMassesPerYear, Years};

    /// The phases that feed helium are the engine's helium surface, at any mass, and the census's
    /// helium kind.
    #[test]
    fn the_helium_phases_are_the_engine_s_and_the_census_s() {
        for phase in Phase::ALL {
            for mass in [0.3, 0.9, 3.0] {
                assert_eq!(
                    feeds_helium(phase),
                    Kind::of(phase, mass).surface() == Surface::Helium,
                    "{phase:?} of {mass} M_sun"
                );
            }
            let marks = AccretingMarks {
                age: Years::new(1.0e9),
                white_dwarf_mass: SolarMasses::new(0.8),
                donor_mass: SolarMasses::new(0.2),
                donor_phase: phase,
                period: None,
                transfer_rate: SolarMassesPerYear::new(1.0e-10),
                recurrence: Years::new(1.0e5),
                magnetic: false,
                white_dwarf_initial_mass: SolarMasses::new(2.0),
                donor_initial_mass: SolarMasses::new(1.0),
            };
            assert_eq!(
                feeds_helium(phase),
                marks.donor_kind() == AwdDonorKind::Helium,
                "{phase:?}"
            );
        }
    }
}
