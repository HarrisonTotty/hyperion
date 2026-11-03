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
//! - [`merger_mass`](StellarRules::merger_mass), at [`MERGER_MASS_VERSION`] (plan 11, P11.T4.d's
//!   fix): what the binary engine's dynamical main-sequence merger keeps, the accretor's mass and
//!   what its thermal timescale allows below it, both stars' masses from it, read at one choke
//!   point, `Engine::merge_dynamically`, through the engine's input's rules. The pair-light and
//!   reach tables fitted from the engine's runs keep the version's rules until the bump refits
//!   them, and the range brief's gate refuses them under another arm
//!   (`stellar/rules/merger_mass_hold.rs` lists the movers).
//! - [`xrb_class`](StellarRules::xrb_class), at [`XRB_CLASS_VERSION`] (plan 11, P11.T5's and
//!   P11.T8.b's fix): what `classify` counts as an X-ray binary, a floor on the transfer rate onto
//!   a compact star, Be donors spun up by stable transfer, a wind-fed black hole's disc, Be/X of
//!   neutron stars alone and symbiotic X-ray binaries of neutron stars beside cool giants, each
//!   read through [`XrbClassRules::carves`] and each timeline's rules
//!   (`stellar/rules/xrb_class_hold.rs` lists the movers).
//! - [`awd_donors`](StellarRules::awd_donors), at [`AWD_DONORS_VERSION`] (plan 11, P11.T5's fix):
//!   which donors give an accreting white dwarf its novae, every donor below it and a hydrogen
//!   donor alone from it, read at one choke point, `classify::nova_recurrence_under`, through each
//!   timeline's rules, by `carved_class` and the census's marks. The catalogue's own timelines,
//!   built from marks, take the version's rules (`stellar/rules/awd_donors_hold.rs` lists the
//!   movers).
//! - [`pinned_white_dwarfs`](StellarRules::pinned_white_dwarfs), at
//!   [`PINNED_WHITE_DWARFS_VERSION`] (plan 11, P11.T4's fix): whether a pinned primary, its pin
//!   pending, that its binary would make a white dwarf is born one below it or held at its core's
//!   last living state until its pin from it, read through the engine's input's rules at one
//!   `match`, [`PinnedWhiteDwarfRules::holds`], by the strip (`Engine::stripped_member_at`) and a
//!   merger's product that no star can hold (`Engine::held_product`)
//!   (`stellar/rules/pinned_white_dwarfs_hold.rs` lists the movers).
//! - [`wind_accretion`](StellarRules::wind_accretion), at [`WIND_ACCRETION_VERSION`] (plan 11,
//!   P11.T12's fix): whether a pair the pre-test's lobe and decay tests pass over takes its winds,
//!   two single stars on their orbit below it, from it a main-sequence star fed its Bondi–Hoyle
//!   share of its partner's wind, or the larger share of an AGB partner's wind Roche-lobe
//!   overflow, read at one choke point, `wind_accretion::onset`, through the
//!   pair's input's rules, by `can_interact` and `evolve`'s pass-over. The fitted tables keep the
//!   version's rules until the bump refits them, and the range brief's gate refuses them under
//!   another arm (`stellar/rules/wind_accretion_hold.rs` lists the movers).
//! - [`dwarf_limit`](StellarRules::dwarf_limit), at [`DWARF_LIMIT_VERSION`] (plan 11, P11.T4's
//!   fix): whether the binary engine holds its white dwarfs below the Chandrasekhar mass and types
//!   the dwarf a merger's unplaced product leaves by its core, read by `Engine::new_star`'s
//!   fallback, `Engine::finish_merger` and the accretor's events through the engine's input's
//!   rules. The fitted tables keep the version's rules until the bump refits them
//!   (`stellar/rules/dwarf_limit_hold.rs` lists the movers).
//!
//! [`GalaxyRules`]: crate::galaxy::rules::GalaxyRules
//! [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
//! [`StandardKickLaw::under`]: crate::stellar::remnant::StandardKickLaw::under
//! [`SystemStars::shown_at`]: crate::stellar::system::SystemStars::shown_at
//! [`WIND_ACCRETION_VERSION`]: crate::stellar::binary::wind_accretion::WIND_ACCRETION_VERSION

use crate::GeneratorVersion;
use crate::stellar::binary::awd_donors::{AWD_DONORS_VERSION, AwdDonorRules};
use crate::stellar::binary::dwarf_limit::{DWARF_LIMIT_VERSION, DwarfLimitRules};
use crate::stellar::binary::merger_mass::{MERGER_MASS_VERSION, MergerMassRules};
use crate::stellar::binary::pinned_white_dwarfs::{
    PINNED_WHITE_DWARFS_VERSION, PinnedWhiteDwarfRules,
};
use crate::stellar::binary::wind_accretion::{WIND_ACCRETION_VERSION, WindAccretionRules};
use crate::stellar::binary::xrb_class::{XRB_CLASS_VERSION, XrbClassRules};
use crate::stellar::remnant::KickRankRules;

/// The stellar stage's holds, carried as an argument from the `Galaxy`'s rules
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct StellarRules {
    /// The rank table the ordinary kick reads ([`KickRankTable::of`]).
    ///
    /// [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
    pub(crate) kick_rank: KickRankRules,
    /// What a dynamically transferring main-sequence pair's merger keeps in the binary engine,
    /// the hold of [`MERGER_MASS_VERSION`].
    pub(crate) merger_mass: MergerMassRules,
    /// Which donors give an accreting white dwarf its novae, the hold of [`AWD_DONORS_VERSION`].
    pub(crate) awd_donors: AwdDonorRules,
    /// What `classify` counts as an X-ray binary, the hold of [`XRB_CLASS_VERSION`].
    pub(crate) xrb_class: XrbClassRules,
    /// Whether a pinned primary, its pin pending, that its binary would make a white dwarf is
    /// held until its pin, the hold of [`PINNED_WHITE_DWARFS_VERSION`].
    pub(crate) pinned_white_dwarfs: PinnedWhiteDwarfRules,
    /// Whether a pair the pre-test's lobe and decay tests pass over takes its winds, the hold of
    /// [`WIND_ACCRETION_VERSION`].
    pub(crate) wind_accretion: WindAccretionRules,
    /// Whether the binary engine holds its white dwarfs below the Chandrasekhar mass and types the
    /// dwarf a merger's unplaced product leaves by its core, the hold of [`DWARF_LIMIT_VERSION`].
    pub(crate) dwarf_limit: DwarfLimitRules,
}

impl StellarRules {
    /// Each field's arm at `version`: the new arm from its `*_VERSION`, the held arm below it.
    ///
    /// Every version takes the committed kick-rank table.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        Self {
            kick_rank: KickRankRules::Committed,
            merger_mass: if version.get() >= MERGER_MASS_VERSION.get() {
                MergerMassRules::Pair
            } else {
                MergerMassRules::ThermalLimit
            },
            awd_donors: if version.get() >= AWD_DONORS_VERSION.get() {
                AwdDonorRules::HydrogenDonors
            } else {
                AwdDonorRules::EveryDonor
            },
            xrb_class: if version.get() >= XRB_CLASS_VERSION.get() {
                XrbClassRules::Carved
            } else {
                XrbClassRules::Held
            },
            pinned_white_dwarfs: if version.get() >= PINNED_WHITE_DWARFS_VERSION.get() {
                PinnedWhiteDwarfRules::Frozen
            } else {
                PinnedWhiteDwarfRules::Born
            },
            wind_accretion: if version.get() >= WIND_ACCRETION_VERSION.get() {
                WindAccretionRules::WindRocheLobe
            } else {
                WindAccretionRules::OwnTracks
            },
            dwarf_limit: if version.get() >= DWARF_LIMIT_VERSION.get() {
                DwarfLimitRules::Chandrasekhar
            } else {
                DwarfLimitRules::Unchecked
            },
        }
    }

    /// The version's stellar rules with those retargeted from 26 to 27 in force too (P11.T4.d's
    /// `merger_mass`): the stellar part of [`GalaxyRules::every_26_hold_with_its_retargets`].
    ///
    /// [`GalaxyRules::every_26_hold_with_its_retargets`]:
    ///     crate::galaxy::rules::GalaxyRules::every_26_hold_with_its_retargets
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_26_hold_with_its_retargets() -> Self {
        Self {
            merger_mass: MergerMassRules::Pair,
            ..Self::of_version(crate::GENERATOR_VERSION)
        }
    }

    /// Every stellar hold at 27 in force, for a test that asserts a 27 figure without a galaxy:
    /// those that turn on at 27 (P11.T4's `pinned_white_dwarfs` and `dwarf_limit`, P11.T4.d's
    /// `merger_mass`, P11.T5's `awd_donors` and `xrb_class`, and P11.T12's `wind_accretion`).
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
mod xrb_class_hold;
