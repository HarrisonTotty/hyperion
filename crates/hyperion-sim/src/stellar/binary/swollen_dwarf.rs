//! The hold on the giant a white dwarf fed hydrogen swells into (plan 11, P11.T4's fixes, at
//! [`SWOLLEN_DWARF_VERSION`]): a helium dwarf's core stays degenerate, and a carbon–oxygen or
//! oxygen–neon dwarf's giant keeps the dwarf's kind.
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
//!
//! As built too, a carbon–oxygen or oxygen–neon dwarf swells onto a thermally pulsing AGB track
//! placed by its core alone (`Engine::swelled_star`), whose own core at the base of the AGB
//! (`sse::pulsing_agb_helium_core`) types what its strip and its death leave (HPT section 6), so
//! that a dwarf can come back the other kind: pinned pair 0080's oxygen–neon dwarf of 1.070 M☉
//! comes back carbon–oxygen, as does 1 of the dwarf-limit census's swollen dwarfs of layer D the
//! other way (2026-11-03). Gaining and losing a hydrogen envelope does not change a core's
//! composition, so from 29 the swell records the dwarf's kind for its member
//! (`Engine::dwarf_cores`, fix-p11-swollen-dwarf-kind), and every exit of the giant takes it:
//! its strip leaves a dwarf of that kind, a merger product no star holds made from it is one
//! (`dwarf_limit::CoreSource`), and its own track's death ends as the dwarf's core would
//! ([`SwollenDeath`]). A merger product placed on a track of its own keeps it too, from the dwarf
//! limit's arm (`dwarf_limit::DwarfLimitRules::placed`, fix-p11-placed-product-kind).

use crate::GeneratorVersion;
use crate::stellar::Phase;
use crate::stellar::remnant::DeathKind;
use crate::stellar::rules::StellarRules;

use crate::units::SolarMasses;

use super::core_endings::CoreEndingRules;
use super::dwarf_limit::CoreSource;

/// The generator version whose stellar rules swell a helium white dwarf fed hydrogen into a giant
/// whose core stays degenerate, and keep a carbon–oxygen or oxygen–neon dwarf's kind for its giant
/// (P11.T4's fixes): `Engine::swell`'s two reads take [`SwollenDwarfRules::Degenerate`].
///
/// Below it every pair takes the held arm, [`SwollenDwarfRules::AtBase`], the code of version 27
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_29_hold()`. The bump to 29 deletes the hold, not the rule: this module with
/// [`SwollenDwarfRules`] (but [`SwollenDeath`], which moves to the engine), the stellar rules'
/// `swollen_dwarf` field, the swell's two reads of it, the brief's read of
/// [`SwollenDwarfRules::fitted_tables_hold`], and the tripwire
/// (`the_swollen_dwarf_is_held_only_until_the_29_bump`). It keeps `Engine::degenerate_giant`,
/// `sse::degenerate_giant_mass`, `Engine::feeds_a_swollen_dwarf`, the record of the dwarf's kind
/// at every swell (`Engine::dwarf_cores`) and [`SwollenDeath`] applied always, and refits the
/// pair-light and reach tables, which the engine's runs fit.
pub(crate) const SWOLLEN_DWARF_VERSION: GeneratorVersion = GeneratorVersion::new(29);

/// Where a helium white dwarf fed hydrogen is placed as a giant, and whether a carbon–oxygen or
/// oxygen–neon dwarf's giant keeps the dwarf's kind (the stellar rules' `swollen_dwarf` field, the
/// hold of [`SWOLLEN_DWARF_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SwollenDwarfRules {
    /// Version 28's and before: BSE's placement, a core above `Mc,BGB`(`M_HeF`) at the base of a
    /// heavier star's giant branch; a carbon–oxygen or oxygen–neon dwarf's giant ends as its
    /// track's kind.
    AtBase,
    /// Version 29's: on the first giant branch of a star below `M_HeF`, where its core is the
    /// dwarf's; a carbon–oxygen or oxygen–neon dwarf's giant keeps the dwarf's kind.
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

/// How the giant a dwarf swelled into ends where its own track dies: as the dwarf's core would.
///
/// From version 29 (`Engine::die`): the swell places the giant on a track by its core alone, and
/// the track types its death by its own core at the base of the AGB (HPT section 6), which need not
/// be the dwarf's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SwollenDeath {
    /// The track's own death, which the dwarf's kind agrees with, or no dwarf's.
    Own,
    /// The white dwarf of `phase`, the dwarf's kind, where the track leaves the other kind.
    WhiteDwarf(Phase),
    /// An oxygen–neon core that dies with its envelope on where the track's would not collapse by
    /// electron capture: it does, an electron-capture supernova (Nomoto 1984, ApJ 277, 791) with
    /// no Type Ia candidate (`Engine::electron_capture_collapse`; lean, 2026-11-03, where the
    /// ruling has the accretion-induced collapse of a bare dwarf, whose remnant and kick are the
    /// same).
    ElectronCapture,
    /// A carbon–oxygen core that dies with its envelope on where the track's would not ignite
    /// carbon: carbon ignites in the degenerate core at `Mc,SN` and nothing is left (HPT section 6,
    /// [`DeathKind::ThermonuclearDisruption`]).
    Disruption,
}

impl SwollenDeath {
    /// How a giant whose core came from `source` ends where its own track's death is `death`, with
    /// its carbon–oxygen core at `core`, leaving a remnant of `phase`, under `endings`.
    ///
    /// From version 29 ([`CoreEndingRules::Unified`]) a track that collapses an iron core collapses
    /// the kept core too, a modelling choice after BSE, whose merger product takes the type of the
    /// giant it makes (BSE table 2; research-leans-38): the kept kind sets only what is left where
    /// the track leaves a white dwarf or collapses an oxygen–neon core by electron capture, there a
    /// carbon–oxygen core below the Chandrasekhar mass
    /// ([`CoreEndingRules::below_the_chandrasekhar_mass`]) being left as its dwarf.
    #[must_use]
    pub(super) fn of(
        source: CoreSource,
        death: DeathKind,
        core: SolarMasses,
        phase: Phase,
        endings: CoreEndingRules,
    ) -> Self {
        let Some(kind) = source.dwarf() else {
            return Self::Own;
        };
        match death {
            DeathKind::ElectronCapture
                if endings == CoreEndingRules::Unified
                    && kind == Phase::CarbonOxygenWhiteDwarf
                    && endings.below_the_chandrasekhar_mass(core) =>
            {
                Self::WhiteDwarf(kind)
            }
            DeathKind::CoreCollapse { .. }
            | DeathKind::DirectCollapse
            | DeathKind::PairInstability
                if endings == CoreEndingRules::Unified =>
            {
                Self::Own
            }
            DeathKind::EnvelopeLoss if source.dwarf_phase(phase) == phase => Self::Own,
            DeathKind::EnvelopeLoss => Self::WhiteDwarf(kind),
            DeathKind::ElectronCapture if kind == Phase::OxygenNeonWhiteDwarf => Self::Own,
            DeathKind::ThermonuclearDisruption if kind == Phase::CarbonOxygenWhiteDwarf => {
                Self::Own
            }
            DeathKind::ElectronCapture
            | DeathKind::ThermonuclearDisruption
            | DeathKind::CoreCollapse { .. }
            | DeathKind::DirectCollapse
            | DeathKind::PairInstability => {
                if kind == Phase::OxygenNeonWhiteDwarf {
                    Self::ElectronCapture
                } else {
                    Self::Disruption
                }
            }
        }
    }
}

impl SwollenDeath {
    /// Counts the death of a giant whose core is a dwarf's on this thread ([`dwarf_core_deaths`]).
    #[cfg(test)]
    pub(super) fn count(self) {
        count_dwarf_core_death(match self {
            Self::Own => Ending::Own,
            Self::WhiteDwarf(_) => Ending::WhiteDwarf,
            Self::ElectronCapture => Ending::ElectronCapture,
            Self::Disruption => Ending::Disruption,
        });
    }
}

/// How a giant whose core is a dwarf's ended ([`dwarf_core_deaths`]'s columns, in order).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ending {
    /// Its track's own death.
    Own,
    /// A white dwarf of the dwarf's kind at its track's death.
    WhiteDwarf,
    /// An electron capture at its track's death.
    ElectronCapture,
    /// A disruption at its track's death.
    Disruption,
    /// An electron capture at its core's limit (`Engine::reach_core_limit`).
    CaptureAtLimit,
    /// A disruption at its core's limit (`Engine::reach_core_limit`).
    DisruptionAtLimit,
}

#[cfg(test)]
thread_local! {
    /// This thread's deaths of giants whose cores are dwarfs' since it started
    /// ([`dwarf_core_deaths`]).
    static DWARF_CORE_DEATHS: core::cell::Cell<[usize; 6]> =
        const { core::cell::Cell::new([0; 6]) };
}

/// Counts a death of `ending` ([`dwarf_core_deaths`]) on this thread.
#[cfg(test)]
pub(super) fn count_dwarf_core_death(ending: Ending) {
    DWARF_CORE_DEATHS.with(|counts| {
        let mut c = counts.get();
        c[ending as usize] += 1;
        counts.set(c);
    });
}

/// The deaths of giants whose cores are carbon–oxygen or oxygen–neon dwarfs'
/// (`Engine::dwarf_cores`) this thread's engines have acted on since it started, by ending: their
/// tracks' own, a white
/// dwarf of the dwarf's kind, an electron capture and a disruption at their tracks' deaths
/// ([`SwollenDeath`]), and an electron capture and a disruption at their cores' limits
/// (`Engine::reach_core_limit`), a test build's counter for the dwarf limit's census
/// ([`Ending`]'s order).
#[cfg(test)]
#[must_use]
pub(crate) fn dwarf_core_deaths() -> [usize; 6] {
    DWARF_CORE_DEATHS.with(core::cell::Cell::get)
}
