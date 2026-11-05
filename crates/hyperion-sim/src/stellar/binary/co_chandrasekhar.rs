//! The hold on how a carbon–oxygen white dwarf or core at the Chandrasekhar mass ends (plan 11,
//! P11.T4's fix, at [`CO_CHANDRASEKHAR_VERSION`]; research-leans-35 §8).
//!
//! Under the version's rules such a dwarf or core ends three ways, each one way by its path
//! ([`ChandrasekharPath`]):
//!
//! - a white dwarf a companion feeds to its collapse mass, by stable transfer
//!   (`Engine::accretor_events`) or by its wind in a detached phase (`Engine::collapse_fed_dwarf`),
//!   and a merger's white-dwarf product at it (`Engine::finish_merger`; a pinned primary's merged
//!   dwarf in the dwarfs' merger) pool a Type Ia candidate, then collapse to the 1.26 M☉ neutron
//!   star of an accretion-induced collapse;
//! - a giant whose core is a carbon–oxygen dwarf's (`Engine::dwarf_cores`) disrupts and leaves
//!   nothing where its track dies so (`Engine::die`), with no candidate; since the 29 folds such a
//!   kept core has no limit of its own (P11.T4's kept-core endings), so none reaches the
//!   Chandrasekhar mass before its track dies;
//! - a merger's helium-giant product that no star holds, which carbon ignites in where its core
//!   reaches the Chandrasekhar mass (`dwarf_limit::Unplaced::Disruption`), disrupts with no
//!   candidate.
//!
//! All three are a carbon–oxygen core reaching the Chandrasekhar mass, the single-degenerate
//! channel. In BSE such a dwarf, and a helium one, is destroyed with no remnant, and only an
//! oxygen–neon one leaves a neutron star (Hurley, Tout and Pols 2002, MNRAS 329, 897, section 2.6;
//! BSE's `evolv2.f`: "If the Chandrasekhar limit is exceeded for a white dwarf then destroy the
//! white dwarf in a supernova. If the WD is `ONe` then a neutron star will survive the supernova",
//! its type set to 15, a massless remnant, for a helium or carbon–oxygen dwarf). The source was
//! read in BSE's code as COSMIC carries it, not in the paper, which the pod could not reach.
//!
//! From 30 ([`CoChandrasekharRules::Disrupted`]) every one of them disrupts and leaves nothing, its
//! companion going on alone as after a thermally pulsing AGB star's carbon ignition, and pools one
//! Type Ia candidate on [`IaPoolChannel::Accretion`] (lean, 2026-11-05: the brief's; a pair that
//! pooled one before, a white dwarfs' merger's, keeps it, the pool holding a pair's first). The
//! candidate's masses are the dwarf's or the core's at ignition and its companion's then, none after
//! a merger, so that it can detonate (`ia::is_detonatable`); the paths that pooled under the held
//! arm keep their masses but the merger's dwarf product, which took the two stars' before the
//! merger. A helium dwarf at its collapse mass disrupts as a carbon–oxygen one does, as BSE's does
//! (none reaches it: one fed helium ignites at 0.7 M☉, and one fed hydrogen swells into a giant).
//! An oxygen–neon dwarf keeps its accretion-induced collapse, with no candidate. A track's own
//! carbon ignition in a star whose core is no dwarf's, a thermally pulsing AGB star's or a helium
//! star's, pools none: plan 09 rules that a single star's thermonuclear disruption is in no Type Ia
//! channel (`SupernovaKind::of_death`), and the engine's pairs would otherwise pool what the same
//! star alone does not (lean, 2026-11-05).

use crate::GeneratorVersion;
use crate::stellar::remnant::{CompactRemnant, DeathKind, RemnantKind};
use crate::stellar::rules::StellarRules;
use crate::units::{SolarMasses, Years};

use super::evolve::Engine;
use super::star::Member;
use super::timeline::{IaPoolChannel, PooledIaEvent};

/// The generator version whose stellar rules end every carbon–oxygen white dwarf or core at the
/// Chandrasekhar mass in a disruption that leaves nothing and pools one Type Ia candidate (P11.T4's
/// fix): `Engine::accretor_events`, `Engine::collapse_fed_dwarf`, `Engine::finish_merger`, the
/// dwarfs' merger and `Engine::die` take [`CoChandrasekharRules::Disrupted`] through
/// [`Engine::end_at_the_chandrasekhar_mass`].
///
/// Below it every pair takes the held arm, [`CoChandrasekharRules::Held`], the code of version 29
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_30_hold()`. The bump to 30 deletes the hold, not the rule: this module's
/// [`CoChandrasekharRules`] with its `Held` arm and [`ChandrasekharPath::held_ending`], the stellar
/// rules' `co_chandrasekhar` field, the brief's read of
/// [`CoChandrasekharRules::fitted_tables_hold`] and the tripwire
/// (`the_co_chandrasekhar_ending_is_held_only_until_the_30_bump`); it keeps
/// [`Engine::end_at_the_chandrasekhar_mass`] with its candidate, and refits the pair-light and
/// reach tables, which the engine's runs fit.
pub(crate) const CO_CHANDRASEKHAR_VERSION: GeneratorVersion = GeneratorVersion::new(30);

/// How a carbon–oxygen white dwarf or core at the Chandrasekhar mass ends (the stellar rules'
/// `co_chandrasekhar` field, the hold of [`CO_CHANDRASEKHAR_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CoChandrasekharRules {
    /// Version 29's and before: each path's own ending ([`ChandrasekharPath::held_ending`]).
    Held,
    /// Version 30's: a disruption that leaves nothing, with one Type Ia candidate.
    Disrupted,
}

impl CoChandrasekharRules {
    /// The arm at `version`: [`CoChandrasekharRules::Disrupted`] from
    /// [`CO_CHANDRASEKHAR_VERSION`], [`CoChandrasekharRules::Held`] below it.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        if version.get() >= CO_CHANDRASEKHAR_VERSION.get() {
            Self::Disrupted
        } else {
            Self::Held
        }
    }

    /// How a carbon–oxygen or helium dwarf or core at its collapse mass reached by `path` ends.
    #[must_use]
    pub(crate) const fn ending(self, path: ChandrasekharPath) -> ChandrasekharEnding {
        match self {
            Self::Held => path.held_ending(),
            Self::Disrupted => ChandrasekharEnding::Disruption,
        }
    }

    /// Whether the ending pools a Type Ia candidate: under the held arm only where it collapses,
    /// and always from 30.
    #[must_use]
    pub(crate) const fn pools(self, path: ChandrasekharPath) -> bool {
        match self {
            Self::Held => matches!(path.held_ending(), ChandrasekharEnding::Collapse),
            Self::Disrupted => true,
        }
    }

    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold for a pair under this arm: under the version's arm only, until the bump refits
    /// them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).co_chandrasekhar
    }
}

/// How a white dwarf or core at its collapse mass reached the Chandrasekhar mass (the module
/// documentation): the census's columns, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ChandrasekharPath {
    /// A white dwarf a companion's stable transfer feeds (`Engine::accretor_events`).
    Transfer,
    /// A white dwarf a companion's wind feeds in a detached phase (`Engine::collapse_fed_dwarf`).
    Wind,
    /// A merger's white-dwarf product (`Engine::finish_merger`).
    MergedDwarf,
    /// A pinned primary's merged dwarf in the dwarfs' merger, which collapses at once
    /// (`dwarf_limit`'s pin).
    PinnedMerger,
    /// A merger's helium-giant product that no star holds, its core at its carbon ignition
    /// (`dwarf_limit::Unplaced::Disruption`).
    UnplacedProduct,
    /// A giant whose core is a carbon–oxygen dwarf's, at its track's death where the track would
    /// not ignite carbon, which disrupts it (`supernova::SwollenDeath::Disruption`).
    KeptCoreAtDeath,
    /// The same where its own track ignites carbon at its death, a thermally pulsing AGB star's or
    /// a helium star's (`supernova::SwollenDeath::Own`, `DeathKind::ThermonuclearDisruption`).
    KeptCoreIgnition,
}

impl ChandrasekharPath {
    /// Every path, in the census's order.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 7] = [
        Self::Transfer,
        Self::Wind,
        Self::MergedDwarf,
        Self::PinnedMerger,
        Self::UnplacedProduct,
        Self::KeptCoreAtDeath,
        Self::KeptCoreIgnition,
    ];

    /// How the path ends under the held arm: a collapse after a pooled candidate for a white
    /// dwarf, a disruption with none for a giant's or a product's core.
    #[must_use]
    pub(crate) const fn held_ending(self) -> ChandrasekharEnding {
        match self {
            Self::Transfer | Self::Wind | Self::MergedDwarf | Self::PinnedMerger => {
                ChandrasekharEnding::Collapse
            }
            Self::UnplacedProduct | Self::KeptCoreAtDeath | Self::KeptCoreIgnition => {
                ChandrasekharEnding::Disruption
            }
        }
    }
}

/// How a carbon–oxygen or helium white dwarf or core at its collapse mass ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ChandrasekharEnding {
    /// An accretion-induced collapse into a 1.26 M☉ neutron star (`Engine::accretion_induced_collapse`).
    Collapse,
    /// A thermonuclear disruption that leaves nothing.
    Disruption,
}

impl Engine {
    /// Member `i`, a carbon–oxygen or helium white dwarf at its collapse mass, or a star whose
    /// carbon–oxygen core does not survive the Chandrasekhar mass, reached by `path`, ends now
    /// under the stellar rules' `co_chandrasekhar` arm (the module documentation): a Type Ia
    /// candidate of `masses` pooled where the arm pools one
    /// ([`Engine::chandrasekhar_candidate`]), then the member, of `m` M☉ with any envelope it keeps,
    /// collapses by accretion-induced collapse or disrupts and leaves nothing, the pair unbound and
    /// its companion alone. The caller has closed the segment.
    pub(super) fn end_at_the_chandrasekhar_mass(
        &mut self,
        i: usize,
        m: f64,
        path: ChandrasekharPath,
        masses: [SolarMasses; 2],
    ) {
        self.chandrasekhar_candidate(path, masses);
        match self.ctx.rules().co_chandrasekhar.ending(path) {
            ChandrasekharEnding::Collapse => self.accretion_induced_collapse(i, m),
            ChandrasekharEnding::Disruption => {
                self.set_member(i, Member::Gone);
                let nothing = CompactRemnant::new(RemnantKind::None, SolarMasses::ZERO);
                self.explode(i, m, nothing, None);
            }
        }
    }

    /// The masses of the Type Ia candidate of member `i`, of `before` M☉, whose carbon–oxygen core
    /// of `core` ignites carbon: the core, held to the member's mass, and its companion's now.
    #[must_use]
    pub(super) fn ignition_masses(
        &self,
        i: usize,
        core: SolarMasses,
        before: f64,
    ) -> [SolarMasses; 2] {
        [
            SolarMasses::new(core.value().min(before)),
            SolarMasses::new(self.current(1 - i).0),
        ]
    }

    /// Member `i`, of `before` M☉, a giant whose core of `core` is a carbon–oxygen dwarf's
    /// (`Engine::dwarf_cores`), dies by its own track's `death`: where that is the track's carbon
    /// ignition (a thermally pulsing AGB star's or a helium star's,
    /// [`DeathKind::ThermonuclearDisruption`]), its candidate is pooled as the stellar rules'
    /// `co_chandrasekhar` arm pools one ([`ChandrasekharPath::KeptCoreIgnition`]). The track's own
    /// death leaves nothing.
    pub(super) fn own_carbon_ignition(
        &mut self,
        i: usize,
        death: DeathKind,
        core: SolarMasses,
        before: f64,
    ) {
        if death == DeathKind::ThermonuclearDisruption {
            let masses = self.ignition_masses(i, core, before);
            self.chandrasekhar_candidate(ChandrasekharPath::KeptCoreIgnition, masses);
        }
    }

    /// Member `i`, a white dwarf of `m` M☉ at its collapse mass, ends now: by accretion-induced
    /// collapse with no candidate where it is oxygen–neon (`path` `None`), and otherwise as
    /// [`Engine::end_at_the_chandrasekhar_mass`] ends it by `path`, with a candidate of `masses`.
    pub(super) fn end_dwarf_at_its_collapse(
        &mut self,
        i: usize,
        m: f64,
        path: Option<ChandrasekharPath>,
        masses: [SolarMasses; 2],
    ) {
        match path {
            None => self.accretion_induced_collapse(i, m),
            Some(path) => self.end_at_the_chandrasekhar_mass(i, m, path, masses),
        }
    }

    /// Pools the Type Ia candidate of a dwarf or core at the Chandrasekhar mass reached by `path`,
    /// of `masses`, where the stellar rules' `co_chandrasekhar` arm pools one
    /// ([`CoChandrasekharRules::pools`]), on [`IaPoolChannel::Accretion`]: the pair's first
    /// candidate unless it has one. The caller ends the member.
    pub(super) fn chandrasekhar_candidate(
        &mut self,
        path: ChandrasekharPath,
        masses: [SolarMasses; 2],
    ) {
        let rules = self.ctx.rules().co_chandrasekhar;
        let event = rules
            .pools(path)
            .then(|| PooledIaEvent::new(Years::new(self.age), IaPoolChannel::Accretion, masses));
        #[cfg(test)]
        count_chandrasekhar_ending(
            path,
            rules.ending(path),
            event.filter(|_| self.pooled.is_none()),
        );
        if let Some(event) = event {
            self.pool(event);
        }
    }
}

/// Counts an ending by `path` on this thread ([`chandrasekhar_endings`]), with the candidate it
/// pooled first for its pair, if any.
#[cfg(test)]
fn count_chandrasekhar_ending(
    path: ChandrasekharPath,
    ending: ChandrasekharEnding,
    first: Option<PooledIaEvent>,
) {
    let k = ChandrasekharPath::ALL
        .iter()
        .position(|&p| p == path)
        .expect("every path is in ALL");
    CHANDRASEKHAR_ENDINGS.with(|counts| {
        let mut c = counts.get();
        c[k][0] += 1;
        c[k][match ending {
            ChandrasekharEnding::Collapse => 1,
            ChandrasekharEnding::Disruption => 2,
        }] += 1;
        if let Some(event) = first {
            c[k][3] += 1;
            c[k][4] += usize::from(super::ia::is_detonatable(&event));
        }
        counts.set(c);
    });
}

#[cfg(test)]
thread_local! {
    /// This thread's endings at the Chandrasekhar mass since it started ([`chandrasekhar_endings`]).
    static CHANDRASEKHAR_ENDINGS: core::cell::Cell<[[usize; 5]; 7]> =
        const { core::cell::Cell::new([[0; 5]; 7]) };
}

/// The endings of carbon–oxygen and helium white dwarfs and cores at the Chandrasekhar mass this
/// thread's engines have made since it started, by path ([`ChandrasekharPath::ALL`]'s order): each
/// path's endings, its accretion-induced collapses and its disruptions, and the Type Ia candidates
/// it pooled first for their pairs, with those that can detonate (`ia::is_detonatable`): a test
/// build's counter for the census.
#[cfg(test)]
#[must_use]
pub(crate) fn chandrasekhar_endings() -> [[usize; 5]; 7] {
    CHANDRASEKHAR_ENDINGS.with(core::cell::Cell::get)
}
