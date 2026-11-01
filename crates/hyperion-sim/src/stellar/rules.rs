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
//! - [`ia_explosion`](StellarRules::ia_explosion), at [`IA_EXPLOSION_VERSION`] (plan 11, P11.T6):
//!   whether the binary engine's pooled Type Ia candidates take their explosion mark. From it the
//!   grid redraws a binary whose candidate the mark explodes at or before +H, in one test,
//!   [`SystemStars::exploded_pair`], which the system stage's redraw and P09.T35's hook read
//!   (`stellar/rules/ia_hold.rs`).
//! - [`ns_accretion`](StellarRules::ns_accretion), at [`NS_ACCRETION_VERSION`] (plan 11, P11.T4's
//!   fix): what a neutron star keeps of a companion's stable Roche-lobe overflow, all of it up to
//!   the Eddington rate below it, a share from it, read at one choke point, `Engine::accretion`,
//!   through the engine's input's rules. The pair-light and reach tables keep the version's rules
//!   until the bump refits them, as for the merger mass (`stellar/rules/ns_accretion_hold.rs`
//!   lists the movers).
//! - [`binary_kicks`](StellarRules::binary_kicks), at [`BINARY_KICKS_VERSION`] (plan 11, P11.T10):
//!   whether a system that a supernova has unbound goes on showing the star it released, both
//!   stars of the drawn hierarchy below it, from it only the stars it still holds, chosen at one
//!   choke point, [`SystemStars::shown_at`], which the system's summary, state and brief and the
//!   range brief's paired route read, and under which the range brief's gate checks every pair
//!   (`stellar/rules/binary_kicks_hold.rs` lists the movers).
//! - [`heavy_donor`](StellarRules::heavy_donor), at [`HEAVY_DONOR_VERSION`] (plan 11, P11.T4's
//!   fix): stable transfer from a main-sequence donor heavier than its compact accretor at its
//!   thermal rate, and the transfer's angular momentum kept, read by the transfer's step, its
//!   trials and its corotation through the engine's input's rules. The fitted tables keep the
//!   version's rules until the bump refits them (`stellar/rules/heavy_donor_hold.rs` lists the
//!   movers).
//! - [`xrb_class`](StellarRules::xrb_class), at [`XRB_CLASS_VERSION`] (plan 11, P11.T5's and
//!   P11.T8.b's fix): what `classify` counts as an X-ray binary, a floor on the transfer rate onto
//!   a compact star, Be donors spun up by stable transfer, a wind-fed black hole's disc and Be/X
//!   of neutron stars alone, each
//!   read through [`XrbClassRules::carves`] and each timeline's rules
//!   (`stellar/rules/xrb_class_hold.rs` lists the movers).
//! - [`awd_donors`](StellarRules::awd_donors), at [`AWD_DONORS_VERSION`] (plan 11, P11.T5's fix):
//!   which donors give an accreting white dwarf its novae, every donor below it and a hydrogen
//!   donor alone from it, read at one choke point, `classify::nova_recurrence_under`, through each
//!   timeline's rules, by `carved_class` and the census's marks. The catalogue's own timelines,
//!   built from marks, take the version's rules (`stellar/rules/awd_donors_hold.rs` lists the
//!   movers).
//! - [`collapses`](StellarRules::collapses), at [`COLLAPSES_VERSION`] (plan 11, P11.T10's fix):
//!   whether a core collapse acts on the orbits the engine does not otherwise run, a pair the
//!   pre-test passes over and the outer orbits above a run pair, both drawn below it, and from it
//!   a pair a star of which collapses by its age run through the engine and each run pair's
//!   supernova applied to the orbits above it, read at two choke points, `evolve`'s pre-test through the engine's input's
//!   rules and `SystemStars`' outer releases through the primary's, which
//!   [`SystemStars::shown_at`] reads (`stellar/rules/collapses_hold.rs` lists the movers).
//!
//! [`GalaxyRules`]: crate::galaxy::rules::GalaxyRules
//! [`KickRankTable::of`]: crate::stellar::remnant::KickRankTable::of
//! [`StandardKickLaw::under`]: crate::stellar::remnant::StandardKickLaw::under
//! [`SystemStars::exploded_pair`]: crate::stellar::system::SystemStars::exploded_pair
//! [`BINARY_KICKS_VERSION`]: crate::stellar::system::BINARY_KICKS_VERSION
//! [`SystemStars::shown_at`]: crate::stellar::system::SystemStars::shown_at

use crate::GeneratorVersion;
use crate::stellar::binary::awd_donors::{AWD_DONORS_VERSION, AwdDonorRules};
use crate::stellar::binary::collapses::{COLLAPSES_VERSION, CollapseRules};
use crate::stellar::binary::heavy_donor::{HEAVY_DONOR_VERSION, HeavyDonorRules};
use crate::stellar::binary::ia::{IA_EXPLOSION_VERSION, IaExplosionRules};
use crate::stellar::binary::merger_mass::{MERGER_MASS_VERSION, MergerMassRules};
use crate::stellar::binary::ns_accretion::{NS_ACCRETION_VERSION, NsAccretionRules};
use crate::stellar::binary::xrb_class::{XRB_CLASS_VERSION, XrbClassRules};
use crate::stellar::remnant::KickRankRules;
use crate::stellar::system::{BINARY_KICKS_VERSION, BinaryKicksRules};

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
    /// Whether the system stage reads the Type Ia explosion mark and redraws a binary it explodes
    /// by +H, the hold of [`IA_EXPLOSION_VERSION`].
    pub(crate) ia_explosion: IaExplosionRules,
    /// What a neutron star keeps of a companion's stable Roche-lobe overflow in the binary engine,
    /// the hold of [`NS_ACCRETION_VERSION`].
    pub(crate) ns_accretion: NsAccretionRules,
    /// Whether a system that a supernova has unbound goes on showing the star it released, the
    /// hold of [`BINARY_KICKS_VERSION`].
    pub(crate) binary_kicks: BinaryKicksRules,
    /// How the binary engine's stable transfer treats a main-sequence donor heavier than its
    /// compact accretor and books the transfer's angular momentum, the hold of
    /// [`HEAVY_DONOR_VERSION`].
    pub(crate) heavy_donor: HeavyDonorRules,
    /// Which donors give an accreting white dwarf its novae, the hold of [`AWD_DONORS_VERSION`].
    pub(crate) awd_donors: AwdDonorRules,
    /// What `classify` counts as an X-ray binary, the hold of [`XRB_CLASS_VERSION`].
    pub(crate) xrb_class: XrbClassRules,
    /// Whether a core collapse acts on the orbits the binary engine does not otherwise run, the
    /// hold of [`COLLAPSES_VERSION`].
    pub(crate) collapses: CollapseRules,
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
            ia_explosion: if version.get() >= IA_EXPLOSION_VERSION.get() {
                IaExplosionRules::Marked
            } else {
                IaExplosionRules::Pooled
            },
            ns_accretion: if version.get() >= NS_ACCRETION_VERSION.get() {
                NsAccretionRules::Efficiency
            } else {
                NsAccretionRules::Eddington
            },
            binary_kicks: if version.get() >= BINARY_KICKS_VERSION.get() {
                BinaryKicksRules::Alone
            } else {
                BinaryKicksRules::Paired
            },
            heavy_donor: if version.get() >= HEAVY_DONOR_VERSION.get() {
                HeavyDonorRules::Thermal
            } else {
                HeavyDonorRules::Nuclear
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
            collapses: if version.get() >= COLLAPSES_VERSION.get() {
                CollapseRules::Applied
            } else {
                CollapseRules::Gated
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
    /// those that turn on at 27 (P11.T6's `ia_explosion`, P11.T4's `ns_accretion` and
    /// `heavy_donor`, P11.T4.d's `merger_mass`, P11.T5's `awd_donors` and `xrb_class`, and P11.T10's
    /// `binary_kicks` and `collapses`).
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
mod events_hold;
#[cfg(test)]
mod heavy_donor_hold;
#[cfg(test)]
mod ia_hold;
#[cfg(test)]
mod merger_mass_hold;
#[cfg(test)]
mod ns_accretion_hold;
#[cfg(test)]
mod stripping_hold;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod xrb_class_hold;
