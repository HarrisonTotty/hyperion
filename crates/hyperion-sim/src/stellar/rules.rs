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
//!   rotation in its summary and its range brief, the binary engine's Be stars, the sky census's
//!   stars and the host stars' discs. The sky's fitted tables, the server's converters and
//!   `hyperion-fit` take the version's rules, as do the sky's luminosity tables
//!   (`sky::luminosity`): the server builds them once per seed and keeps them under a key of the
//!   seed alone, so they follow the version, not a test's arm. The 25 → 26 bump deletes the field,
//!   its held arm and its tripwire.
//! - [`helium`](StellarRules::helium), the helium-excess table the tracks read, at
//!   [`HELIUM_VERSION`] (plan 15, P15.T7.b's swap): P06.T17's identity below it, the fitted
//!   `tables::helium_26` from it. A track's rules ride its [`TrackOptions`], whose builders read
//!   the table at one choke point, [`HeliumTable::of`]; `StarModel::new_under`, the engine's
//!   context and the brief put the galaxy's rules on the options of every track they build.
//! - [`hd_limit`](StellarRules::hd_limit), the Humphreys–Davidson limit in the modern winds, at
//!   [`HD_LIMIT_VERSION`] (plan 06, P06.T42.b's swap, rendering plan R06's ask A2): Belczynski et
//!   al.'s constant below it, the limit enforced from it, read at one choke point, the wind's
//!   rate, through every track's options as the helium table is; and from it a luminous blue
//!   variable's eruptions are thinned to the instants at which it is one. The fitted tables built
//!   from the tracks keep the version's rules until the bump refits them, and with them the
//!   galaxy's mean masses and so its parameters (`stellar/rules/hd_limit_hold.rs` lists them).
//! - [`v_magnitude`](StellarRules::v_magnitude), the V magnitudes of white dwarfs and evolved
//!   stars, at [`V_MAGNITUDE_VERSION`] (plan 06, P06.T43.b's swap, rendering plan R06's ask A4): A4's
//!   interim below it, white dwarfs dark and every living star on the dwarf table's correction;
//!   from it the Montreal grid's V for white dwarfs and the giants' correction for every living
//!   state past the hydrogen main sequence, chosen at one choke point,
//!   [`absolute_magnitude_v_under`]. Its readers on the record are those of the dwarf sequence's
//!   V: a star's summary and its range brief, the sky census's stars and the host stars' discs. The
//!   census restates the bounds that rest on dark white dwarfs under the new arm (its
//!   `StarBounds`). The sky's fitted tables, its luminosity tables, the pair-light tables, the
//!   server's converters and `hyperion-fit` take the version's rules, as for the dwarf sequence
//!   (`stellar/rules/v_magnitude_hold.rs` lists them).
//!
//! [`GalaxyRules`]: crate::galaxy::rules::GalaxyRules
//! [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
//! [`StandardKickLaw::under`]: crate::stellar::remnant::StandardKickLaw::under
//! [`DwarfSequence::of`]: crate::stellar::classify::pm13::DwarfSequence::of
//! [`TrackOptions`]: crate::stellar::sse::TrackOptions
//! [`HeliumTable::of`]: crate::stellar::sse::track::HeliumTable::of
//! [`V_MAGNITUDE_VERSION`]: crate::stellar::photometry::V_MAGNITUDE_VERSION
//! [`absolute_magnitude_v_under`]: crate::stellar::photometry::absolute_magnitude_v_under

use crate::GeneratorVersion;
use crate::stellar::classify::pm13::{DWARF_SEQUENCE_VERSION, DwarfSequenceRules};
use crate::stellar::photometry::{V_MAGNITUDE_VERSION, VMagnitudeRules};
use crate::stellar::remnant::KickRankRules;
use crate::stellar::sse::wind::{HD_LIMIT_VERSION, HdLimitRules};
use crate::stellar::sse::{HELIUM_VERSION, HeliumRules};

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
    /// The helium-excess table the tracks read ([`HeliumTable::of`]), the hold of
    /// [`HELIUM_VERSION`].
    ///
    /// [`HeliumTable::of`]: crate::stellar::sse::track::HeliumTable::of
    pub(crate) helium: HeliumRules,
    /// How the modern winds treat an evolved star past the Humphreys–Davidson limit (the wind's
    /// rate, `wind::rate`), and whether a luminous blue variable's eruptions are thinned to it,
    /// the hold of [`HD_LIMIT_VERSION`].
    pub(crate) hd_limit: HdLimitRules,
    /// Which V magnitudes white dwarfs and evolved stars read (`photometry`'s
    /// `absolute_magnitude_v_under`), the hold of [`V_MAGNITUDE_VERSION`].
    pub(crate) v_magnitude: VMagnitudeRules,
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
            helium: if version.get() >= HELIUM_VERSION.get() {
                HeliumRules::Fitted
            } else {
                HeliumRules::Identity
            },
            hd_limit: if version.get() >= HD_LIMIT_VERSION.get() {
                HdLimitRules::Enforced
            } else {
                HdLimitRules::Belczynski
            },
            v_magnitude: if version.get() >= V_MAGNITUDE_VERSION.get() {
                VMagnitudeRules::A4
            } else {
                VMagnitudeRules::Interim
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
mod hd_limit_hold;
#[cfg(test)]
mod helium_hold;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod v_magnitude_hold;
