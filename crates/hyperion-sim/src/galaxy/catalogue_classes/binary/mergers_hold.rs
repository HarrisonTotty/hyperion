//! The hold on the merger classes' step (P15.T10.b's mergers, at [`MERGER_TABLES_VERSION`]; main's
//! rulings of 2026-11-05 and 2026-11-07, the lists of trains 29 and 30 being frozen).
//!
//! The merger census (`stellar::binary::testing`'s `brute_force_merger_members_in`), against which
//! the stellar and neutron-star merger classes are measured and which plan 15 fits their samplers
//! to, drew every record's first attempt under `Free` multiplicity on its primary's own draws, as
//! the class census did before 509ddc33. The generator shows a displaced record single and runs a
//! retained one's pair on its primary's kept attempt of plan 08's kick loop, so the census counted
//! mergers the grid never holds: of the first 40 non-field records of layers D and E with a merger
//! in a probe, 22 differed from the generator's first attempt, 13 of them displaced.
//!
//! Below 31 every galaxy's merger census draws as before ([`MergerTableRules::Scratch`]), so that
//! every figure its slow tests pin keeps its bits; from 31 it draws the generator's first attempt
//! ([`MergerTableRules::Fitted`]). The tests reach the new arm through
//! `GalaxyRules::every_31_hold()`. The bump to 31 deletes this hold: this module with its
//! tripwire, the galaxy rules' `merger_tables` field and `every_31_hold`, and the census's held
//! arm, and re-pins what moves.

use crate::GeneratorVersion;

/// The generator version from which the merger classes' step is in force (P15.T10.b's mergers):
/// the merger census draws the generator's first attempt.
pub(crate) const MERGER_TABLES_VERSION: GeneratorVersion = GeneratorVersion::new(31);

/// The merger classes' step a galaxy is under (the galaxy rules' `merger_tables` field, the hold of
/// [`MERGER_TABLES_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MergerTableRules {
    /// Version 29's and before: the merger census draws every record's first attempt under `Free`
    /// multiplicity on its primary's own draws.
    Scratch,
    /// Version 31's: the merger census draws each record's pairs as the generator does at its
    /// first attempt, under its own multiplicity and on its primary's kept kick.
    Fitted,
}

impl MergerTableRules {
    /// The arm at `version`: [`Fitted`](Self::Fitted) from [`MERGER_TABLES_VERSION`].
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        if version.get() >= MERGER_TABLES_VERSION.get() {
            Self::Fitted
        } else {
            Self::Scratch
        }
    }

    /// Whether the merger census draws each record's pairs as the generator does at its first
    /// attempt (`stellar::binary::testing`'s `mergers_of`, which only the tests build).
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub(crate) const fn census_draws_the_generator(self) -> bool {
        match self {
            Self::Scratch => false,
            Self::Fitted => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::rules::GalaxyRules;

    /// The hold is deleted by the bump to 31, with everything its module documentation lists.
    #[test]
    fn the_merger_step_is_held_only_until_the_31_bump() {
        assert!(
            GENERATOR_VERSION.get() < MERGER_TABLES_VERSION.get(),
            "the bump to 31 deletes the merger step's hold (module documentation)"
        );
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).merger_tables,
            MergerTableRules::Scratch
        );
        assert_eq!(
            GalaxyRules::every_31_hold().merger_tables,
            MergerTableRules::Fitted
        );
        assert!(!MergerTableRules::Scratch.census_draws_the_generator());
        assert!(MergerTableRules::Fitted.census_draws_the_generator());
    }
}
