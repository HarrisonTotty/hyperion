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
//!
//! No stellar hold is held now: the 27 folds put P11's 27 holds on the record.
//!
//! [`GalaxyRules`]: crate::galaxy::rules::GalaxyRules
//! [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
//! [`StandardKickLaw::under`]: crate::stellar::remnant::StandardKickLaw::under

use crate::GeneratorVersion;
use crate::stellar::remnant::KickRankRules;

/// The stellar stage's holds, carried as an argument from the `Galaxy`'s rules
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct StellarRules {
    /// The rank table the ordinary kick reads ([`KickRankTable::of`]).
    ///
    /// [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
    pub(crate) kick_rank: KickRankRules,
}

impl StellarRules {
    /// Each field's arm at `_version`: the new arm from its `*_VERSION`, the held arm below it.
    ///
    /// Every version takes the committed kick-rank table, and no field is held now (the 27 folds).
    #[must_use]
    pub(crate) const fn of_version(_version: GeneratorVersion) -> Self {
        Self {
            kick_rank: KickRankRules::Committed,
        }
    }

    /// The version's stellar rules with those retargeted from 26 to 27 in force too: none is left,
    /// P11.T4.d's `merger_mass` folded (the 27 folds), so they are the version's; the stellar part
    /// of [`GalaxyRules::every_26_hold_with_its_retargets`].
    ///
    /// [`GalaxyRules::every_26_hold_with_its_retargets`]:
    ///     crate::galaxy::rules::GalaxyRules::every_26_hold_with_its_retargets
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_26_hold_with_its_retargets() -> Self {
        Self::of_version(crate::GENERATOR_VERSION)
    }

    /// Every stellar hold at 27 in force, for a test that asserts a 27 figure without a galaxy:
    /// none is left, the 27 folds having put each on the record, so these are the version's rules
    /// (the lane deletes this helper after the merge).
    ///
    /// It is the stellar part of `GalaxyRules::every_27_hold()`, so a later hold at 27 joins both
    /// with nothing to edit.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_27_hold() -> Self {
        Self::of_version(GeneratorVersion::new(27))
    }

    /// The test-only second table in place of the committed one
    /// ([`KickRankRules::Probe`]), which no version takes, under the version's other rules: the
    /// tests' door to every reader's choke point.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn probe() -> Self {
        Self {
            kick_rank: KickRankRules::Probe,
        }
    }
}

#[cfg(test)]
mod awd_donors_hold;
#[cfg(test)]
mod binary_kicks_hold;
#[cfg(test)]
mod classes_hold;
#[cfg(test)]
mod collapses_hold;
#[cfg(test)]
mod dwarf_limit_hold;
#[cfg(test)]
mod events_hold;
#[cfg(test)]
mod heavy_donor_hold;
#[cfg(test)]
mod merger_mass_hold;
#[cfg(test)]
mod ns_accretion_hold;
#[cfg(test)]
mod pinned_white_dwarfs_hold;
#[cfg(test)]
mod stripping_hold;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod wind_accretion_hold;
#[cfg(test)]
mod xrb_class_hold;
