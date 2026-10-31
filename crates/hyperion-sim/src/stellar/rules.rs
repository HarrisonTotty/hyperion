//! The stellar stage's holds, carried as an argument from the galaxy's rules down to the stellar
//! stage's values (decision-galaxy-25-train §1's hold pattern, carried to the stellar stage).
//!
//! A change that moves stellar output, built before the generator version it belongs to, lands
//! behind a hold as a galaxy-side one does ([`GalaxyRules`]): a `*_VERSION` constant beside the code
//! it holds, and a field of [`StellarRules`] whose new arm that version turns on. The galaxy carries
//! them as its rules' [`stellar`](GalaxyRules::stellar) field. Below the entry points the stellar
//! stage takes no galaxy, so the rules go down as an argument, never as a switch on the thread:
//!
//! - [`SystemStars`](crate::stellar::system::SystemStars)' builders read `galaxy.rules().stellar`
//!   and build each star through `StarModel::new_under`, which keeps them for its remnant stage, and
//!   each pair through `BinaryInput::with_rules`, whose engine reads them from its input;
//! - the galaxy-side readers of the stellar law read them from the galaxy: the class table's kick
//!   shares, the feature and centre members' conditional draws, and the range rows' brief;
//! - each held value is chosen in one `match`, at one choke point, which every reader goes
//!   through (for the kick law, [`KickRankTable::of`] and [`StandardKickLaw::under`]).
//!
//! Whatever builds no galaxy takes the version's rules: `StarModel::new`, `BinaryInput::new`,
//! `hyperion-fit`'s tasks, the process-wide tables (the pair light's and the fates') and the
//! server's converters. So no committed table, fingerprint or static depends on a test's arm. A
//! static or memo that a held field reaches must be keyed by its arm, or be reset by
//! `Galaxy::with_rules` and built from the galaxy's rules. `with_rules` already empties the
//! centre's model when the stellar rules change, though no field reaches it yet.
//!
//! These rules outlive each bump: a bump deletes a stellar hold's own field, never
//! [`StellarRules`] or `GalaxyRules::stellar`.
//!
//! The fields:
//!
//! - [`kick_rank`](StellarRules::kick_rank), the ordinary kick's rank table. No version holds a
//!   second table yet, so every version takes the committed one. The tests reach a test-only
//!   second table through it, to show that every reader goes through the choke point.
//! - [`dwarf_sequence`](StellarRules::dwarf_sequence), at [`DWARF_SEQUENCE_VERSION`] (plan 06,
//!   P06.T23.a's licence fix): the dwarf sequence that classification, photometry and rotation
//!   read, the source's rows to 25 and the fitted grids from 26, through
//!   [`DwarfSequence::of`]. Its readers on the record are a star's classification, photometry and
//!   rotation in its summary and its range brief, the binary engine's Be stars and the sky
//!   census's stars; the sky's fitted tables, the server's converters and `hyperion-fit` take the
//!   version's rules. The 25 → 26 bump deletes the field, its held arm and its tripwire.
//!
//! [`GalaxyRules`]: crate::galaxy::rules::GalaxyRules
//! [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
//! [`StandardKickLaw::under`]: crate::stellar::remnant::StandardKickLaw::under
//! [`DwarfSequence::of`]: crate::stellar::classify::pm13::DwarfSequence::of

use crate::GeneratorVersion;
use crate::stellar::classify::pm13::{DWARF_SEQUENCE_VERSION, DwarfSequenceRules};
use crate::stellar::remnant::KickRankRules;

/// The stellar stage's holds, carried as an argument from the `Galaxy`'s rules
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct StellarRules {
    /// The rank table the ordinary kick reads ([`KickRankTable::of`]).
    ///
    /// [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
    pub(crate) kick_rank: KickRankRules,
    /// The dwarf sequence classification, photometry and rotation read
    /// ([`DwarfSequence::of`](crate::stellar::classify::pm13::DwarfSequence::of)).
    pub(crate) dwarf_sequence: DwarfSequenceRules,
}

impl StellarRules {
    /// Each field's arm at `version`: the new arm from its `*_VERSION`, the held arm below it.
    ///
    /// Every version takes the committed kick-rank table.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        Self {
            kick_rank: KickRankRules::Committed,
            dwarf_sequence: if version.get() >= DWARF_SEQUENCE_VERSION.get() {
                DwarfSequenceRules::Reduced
            } else {
                DwarfSequenceRules::Rows
            },
        }
    }

    /// Every stellar hold at 25 in force, for a test that asserts a 25 figure without a galaxy.
    ///
    /// It is the stellar part of `GalaxyRules::every_25_hold()`, so a later hold at 25 joins both
    /// with nothing to edit.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_25_hold() -> Self {
        Self::of_version(GeneratorVersion::new(25))
    }

    /// Every stellar hold at 26 in force, for a test that asserts a 26 figure without a galaxy.
    ///
    /// It is the stellar part of `GalaxyRules::every_26_hold()`, so a later hold at 26 joins both
    /// with nothing to edit.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_26_hold() -> Self {
        Self::of_version(GeneratorVersion::new(26))
    }

    /// The test-only second table in place of the committed one
    /// ([`KickRankRules::Probe`]), which no version takes, under the version's other rules: the
    /// tests' door to every reader's choke point.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn probe() -> Self {
        Self {
            kick_rank: KickRankRules::Probe,
            ..Self::of_version(crate::GENERATOR_VERSION)
        }
    }
}

#[cfg(test)]
mod tests;
