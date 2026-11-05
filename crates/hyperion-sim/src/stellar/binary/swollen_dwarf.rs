//! The hold on the giant a helium white dwarf fed hydrogen swells into (plan 11, P11.T4's fix, at
//! [`SWOLLEN_DWARF_VERSION`]).
//!
//! As built, `Engine::swell` places the giant as BSE's `gntage` does (`sse::new_star_mass`): a core
//! above `Mc,BGB`(`M_HeF`) at the base of the giant branch of a star heavier than `M_HeF`, whose core
//! is not degenerate. The next strip (`Track::remains_at`) then leaves a helium-burning star where
//! the dwarf was degenerate and below its flash. Since a main-sequence donor heavier than its
//! compact accretor transfers at its thermal rate (version 27's heavy donor), a helium dwarf beside
//! such a donor swells at once, and in the old populations' timelines it becomes a helium
//! main-sequence star, a hot subdwarf, in 55 of 3 × 10⁴ layer-C records of the thick disc, 26 of
//! the bulge and 126 of the halo (2026-11-02, under every 27 hold).
//!
//! From 29 ([`SwollenDwarfRules::Degenerate`]) such a dwarf swells onto a track whose core stays
//! degenerate (`sse::degenerate_giant_mass`): a star below `M_HeF`, placed on its first giant branch
//! where its core is the dwarf's (`Engine::degenerate_giant`), so that a strip leaves a helium
//! white dwarf again and helium ignites only when the core reaches the track's own flash. A core at
//! or below `Mc,BGB`(`M_HeF`), which the held placement already puts on a degenerate branch, or not
//! below that track's flash keeps the held placement. Such a dwarf does not swell while its donor
//! is held to the heavy donor's thermal rate (`Engine::feeds_a_swollen_dwarf`, lean, 2026-11-02).

use crate::GeneratorVersion;
use crate::stellar::rules::StellarRules;

/// The generator version whose stellar rules swell a helium white dwarf fed hydrogen into a giant
/// whose core stays degenerate (P11.T4's fix): `Engine::swell` takes
/// [`SwollenDwarfRules::Degenerate`].
///
/// Below it every pair takes the held arm, [`SwollenDwarfRules::AtBase`], the code of version 27
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_29_hold()`. The bump to 29 deletes the hold, not the rule: this module with
/// [`SwollenDwarfRules`], the stellar rules' `swollen_dwarf` field, the swell's read of it, the
/// brief's read of [`SwollenDwarfRules::fitted_tables_hold`], and the tripwire
/// (`the_swollen_dwarf_is_held_only_until_the_29_bump`). It keeps `Engine::degenerate_giant`,
/// `sse::degenerate_giant_mass` and `Engine::feeds_a_swollen_dwarf` applied always, and refits the
/// pair-light and reach tables, which the engine's runs fit.
pub(crate) const SWOLLEN_DWARF_VERSION: GeneratorVersion = GeneratorVersion::new(29);

/// Where a helium white dwarf fed hydrogen is placed as a giant (the stellar rules'
/// `swollen_dwarf` field, the hold of [`SWOLLEN_DWARF_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SwollenDwarfRules {
    /// Version 29's and before: BSE's placement, a core above `Mc,BGB`(`M_HeF`) at the base of a
    /// heavier star's giant branch.
    AtBase,
    /// Version 29's: on the first giant branch of a star below `M_HeF`, where its core is the
    /// dwarf's.
    Degenerate,
}

impl SwollenDwarfRules {
    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold for a pair under this arm: under the version's arm only, until the bump refits
    /// them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).swollen_dwarf
    }
}
