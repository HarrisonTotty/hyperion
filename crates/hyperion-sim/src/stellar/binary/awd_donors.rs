//! The hold on the novae of a white dwarf fed helium (plan 11, P11.T5's fix, at
//! [`AWD_DONORS_VERSION`]).
//!
//! As built, `nova_recurrence_period` gives every accreting white dwarf hydrogen's ignition mass,
//! the fit to Yaron et al.'s (2005, ApJ 623, 398) hydrogen novae, whatever its donor gives it. A
//! white dwarf fed by a helium star or a helium white dwarf (BSE's donor types 7–10, whose surface
//! is helium: the AM Canum Venaticorum stars and the helium-star channel) accretes no hydrogen and
//! has no hydrogen nova. Helium ignites only under a far heavier envelope, and a helium nova is
//! rare: V445 Puppis is the one known (Ashok and Banerjee 2003, A&A 409, 1007; Kato et al. 2008,
//! ApJ 684, 1366). research-p15-t10b-rates found 77 such hosts among the engine's 1,814 fast ones
//! in the fit's slab census, 12 of them helium stars at 2–5 × 10⁻⁸ M☉ yr⁻¹ onto 1.0–1.5 M☉ dwarfs
//! that recur every 6–600 years and make 55% of its novae.
//!
//! From 27 a white dwarf fed helium (by a donor of [`Phase::HeliumMainSequence`],
//! [`Phase::HeliumHertzsprungGap`], [`Phase::HeliumGiantBranch`] or [`Phase::HeliumWhiteDwarf`], as
//! the census's `AccretingMarks::donor_kind` reads it) has no nova: its recurrence is infinite, so
//! it is a slow host. No helium-flash law is taken, since the class's novae are hydrogen's (the
//! brief's lean of 2026-11-01, recorded in plan 11): an approximation, which leaves out a helium
//! nova like V445 Puppis's, far under 1% of the Galaxy's novae.
//!
//! The rules' one `match` is [`AwdDonorRules::has_novae`]. The engine reads it through each
//! timeline's rules (`classify::nova_recurrence_under`, read by `carved_class` and the census's
//! marks), and the catalogue's fast scan through the galaxy's (`AwdProcess::has_no_nova`), which
//! refuses a candidate fed helium, so that the scan keeps exactly the class's entries whichever
//! rules the class's test reads: its timelines, built from marks, take the version's.

use crate::GeneratorVersion;
use crate::stellar::Phase;

/// The generator version whose stellar rules give a white dwarf fed helium no nova (P11.T5's
/// fix): [`AwdDonorRules::HydrogenDonors`].
///
/// Below it every pair takes the held arm, [`AwdDonorRules::EveryDonor`], the code of version 24
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_27_hold()`. The bump to 27 deletes this hold: this module with
/// [`AwdDonorRules`] and its `has_novae`, the stellar rules' `awd_donors` field, its two readers'
/// calls (`nova_recurrence_under`, `AwdProcess::has_no_nova`), and the tripwire
/// (`the_awd_donors_are_held_only_until_the_27_bump`).
pub(crate) const AWD_DONORS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Which donors give an accreting white dwarf its novae (the stellar rules' `awd_donors` field,
/// the hold of [`AWD_DONORS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum AwdDonorRules {
    /// Version 26's and before: every donor's, at hydrogen's ignition mass.
    EveryDonor,
    /// Version 27's: a hydrogen donor's alone; a white dwarf fed helium has none.
    HydrogenDonors,
}

impl AwdDonorRules {
    /// Whether a white dwarf has novae under these rules, `fed_helium` saying whether its donor
    /// feeds it helium, asked only under [`HydrogenDonors`](Self::HydrogenDonors): the hold's one
    /// `match`, which the engine's recurrence (`classify::nova_recurrence_under`) and the
    /// catalogue's fast scan (`AwdProcess::has_no_nova`) both go through.
    #[must_use]
    pub(crate) fn has_novae(self, fed_helium: impl FnOnce() -> bool) -> bool {
        match self {
            Self::EveryDonor => true,
            Self::HydrogenDonors => !fed_helium(),
        }
    }
}

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
