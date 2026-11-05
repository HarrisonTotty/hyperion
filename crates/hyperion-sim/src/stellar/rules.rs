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
//! - [`dwarf_limit`](StellarRules::dwarf_limit), at [`DWARF_LIMIT_VERSION`] (plan 11, P11.T4's
//!   white-dwarf union): how the dwarf limit treats a pinned primary and types the dwarf a merger's
//!   unplaced product leaves, read by the merger's fallback (`Engine::new_star_around`), the
//!   dwarfs' merger, `Engine::finish_merger`, the accretor's events and the detached phase's step
//!   limit through the engine's input's rules. The fitted tables keep the version's rules until
//!   the bump refits them (`stellar/rules/dwarf_limit_hold.rs` lists the movers).
//! - [`wind_reach`](StellarRules::wind_reach), whether the census's pair bound reads the wind's
//!   reach for its `Detached`, from 29 ([`WIND_REACH_VERSION`]; P11.T12's fix). Its one reader is
//!   `light::detached`, which `pair_light_bound_under` reaches with the galaxy's rules from the
//!   census (`StarBounds`) and the brief's gate.
//!
//! The 27 folds put P11's 27 holds on the record; the holds above but `kick_rank` are at 29.
//!
//! [`WIND_REACH_VERSION`]: crate::stellar::binary::wind_reach::WIND_REACH_VERSION
//!
//! [`GalaxyRules`]: crate::galaxy::rules::GalaxyRules
//! [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
//! [`StandardKickLaw::under`]: crate::stellar::remnant::StandardKickLaw::under

use crate::GeneratorVersion;
use crate::stellar::binary::dwarf_limit::{DWARF_LIMIT_VERSION, DwarfLimitRules};
use crate::stellar::binary::wind_reach::WindReachRules;
use crate::stellar::remnant::KickRankRules;

/// The stellar stage's holds, carried as an argument from the `Galaxy`'s rules
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct StellarRules {
    /// The rank table the ordinary kick reads ([`KickRankTable::of`]).
    ///
    /// [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
    pub(crate) kick_rank: KickRankRules,
    /// How the binary engine's dwarf limit treats a pinned primary and types the dwarf a merger's
    /// unplaced product leaves, the hold of [`DWARF_LIMIT_VERSION`].
    pub(crate) dwarf_limit: DwarfLimitRules,
    /// Whether the census's pair bound reads the wind's reach for its `Detached`
    /// ([`WindReachRules`]), from 29.
    pub(crate) wind_reach: WindReachRules,
}

impl StellarRules {
    /// Each field's arm at `version`: the new arm from its `*_VERSION`, the held arm below it.
    ///
    /// Every version takes the committed kick-rank table; the wind's reach is held below 29.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        Self {
            kick_rank: KickRankRules::Committed,
            dwarf_limit: if version.get() >= DWARF_LIMIT_VERSION.get() {
                DwarfLimitRules::Whole
            } else {
                DwarfLimitRules::Held
            },
            wind_reach: WindReachRules::of_version(version),
        }
    }

    /// Every stellar hold at 29 in force, for a test that asserts a 29 figure without a galaxy:
    /// P11.T4's `dwarf_limit` and P11.T12's `wind_reach`. The
    /// stellar part of `GalaxyRules::every_29_hold()`, so a later hold at 29 joins both with nothing
    /// to edit; the 29 bump deletes it with it.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_29_hold() -> Self {
        Self::of_version(GeneratorVersion::new(29))
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
mod wind_reach_hold;
#[cfg(test)]
mod xrb_class_hold;
