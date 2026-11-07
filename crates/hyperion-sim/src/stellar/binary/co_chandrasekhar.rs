//! How a carbon–oxygen white dwarf or core at the Chandrasekhar mass ends (plan 11, P11.T4's fix;
//! research-leans-35 §8).
//!
//! Such a dwarf or core reaches the Chandrasekhar mass by one of three kinds of path
//! ([`ChandrasekharPath`]):
//!
//! - a white dwarf a companion feeds to its collapse mass, by stable transfer
//!   (`Engine::accretor_events`) or by its wind in a detached phase (`Engine::collapse_fed_dwarf`),
//!   and a merger's white-dwarf product at it (`Engine::finish_merger`; a pinned primary's merged
//!   dwarf in the dwarfs' merger);
//! - a giant whose core is a carbon–oxygen dwarf's (`Engine::dwarf_cores`), where its track dies
//!   (`Engine::die`); such a kept core has no limit of its own (P11.T4's kept-core endings), so none
//!   reaches the Chandrasekhar mass before its track dies;
//! - a merger's helium-giant product that no star holds, which carbon ignites in where its core
//!   reaches the Chandrasekhar mass (`dwarf_limit::Unplaced::Disruption`).
//!
//! All three are a carbon–oxygen core reaching the Chandrasekhar mass: the single-degenerate
//! channel for a fed dwarf, the double-degenerate one for two dwarfs' merger product (Claeys et al.
//! 2014, A&A 563, A83, whose abstract defines the two), and a dwarf's core inside a merger product
//! otherwise. Hurley, Tout and Pols 2002 (MNRAS 329, 897) destroy such a dwarf with no remnant:
//! §2.6.5 for two carbon–oxygen dwarfs' coalescence over the Chandrasekhar mass ("it explodes as a
//! possible type Ia SN … leaving no remnant (k2 = 15)"), §2.6.6 for a fed one ("we annihilate it in
//! a type Ia SN, unless it is an `ONeWD` (k2 = 12) which undergoes AIC leaving a NS remnant");
//! Hurley's BSE codes it so (`evolv2.f`: "If the Chandrasekhar limit is exceeded for a white dwarf
//! then destroy the white dwarf in a supernova. If the WD is `ONe` then a neutron star will survive
//! the supernova", its type set to 15, a massless remnant, for a helium or carbon–oxygen dwarf).
//!
//! Every one of them disrupts and leaves nothing, its companion going on alone as after a thermally
//! pulsing AGB star's carbon ignition, and pools one Type Ia candidate on
//! [`IaPoolChannel::Accretion`] (lean, 2026-11-05: the brief's; a pair that pooled one before, a
//! white dwarfs' merger's or an edge-lit detonation's, keeps it, P11.T6's pool holding a pair's
//! first event, so that its mark alone decides the pair). The candidate's masses are the dwarf's or
//! the core's at ignition and its companion's then, none after a merger, so that it can detonate
//! (`ia::is_detonatable`). A helium dwarf at its collapse mass disrupts as a carbon–oxygen one does, as BSE's does
//! (none reaches it in the engine: one fed helium ignites at 0.7 M☉, HTP §2.6.6, and one fed
//! hydrogen swells into a giant or keeps it).
//! An oxygen–neon dwarf keeps its accretion-induced collapse, with no candidate. A track's own
//! carbon ignition in a star whose core is no dwarf's, a thermally pulsing AGB star's or a helium
//! star's, pools none: plan 09 rules that a single star's thermonuclear disruption is in no Type Ia
//! channel (`SupernovaKind::of_death`), and the engine's pairs would otherwise pool what the same
//! star alone does not (lean, 2026-11-05).

use crate::stellar::Phase;
use crate::stellar::remnant::{CompactRemnant, DeathKind, RemnantKind};
use crate::units::{SolarMasses, Years};

use super::evolve::Engine;
use super::star::Member;
use super::timeline::{IaPoolChannel, PooledIaEvent};

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
    /// A pinned primary's merged dwarf in the dwarfs' merger, which ends at once (`dwarf_limit`'s
    /// pin).
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
}

impl Engine {
    /// Member `i`, a carbon–oxygen or helium white dwarf at its collapse mass, or a star whose
    /// carbon–oxygen core does not survive the Chandrasekhar mass, reached by `path`, ends now (the
    /// module documentation): a Type Ia candidate of `masses` pooled
    /// ([`Engine::chandrasekhar_candidate`]), then the member, of `m_msun` M☉ with any envelope it
    /// keeps, disrupts and leaves nothing, the pair unbound and its companion alone. The caller has
    /// closed the segment.
    pub(super) fn end_at_the_chandrasekhar_mass(
        &mut self,
        i: usize,
        m_msun: f64,
        path: ChandrasekharPath,
        masses: [SolarMasses; 2],
    ) {
        self.chandrasekhar_candidate(path, masses);
        self.set_member(i, Member::Gone);
        let nothing = CompactRemnant::new(RemnantKind::None, SolarMasses::ZERO);
        self.explode(i, m_msun, nothing, None);
    }

    /// The masses of the Type Ia candidate of member `i`, of `before_msun` M☉, whose carbon–oxygen
    /// core of `core` ignites carbon: the core, held to the member's mass, and its companion's now.
    #[must_use]
    pub(super) fn ignition_masses(
        &self,
        i: usize,
        core: SolarMasses,
        before_msun: f64,
    ) -> [SolarMasses; 2] {
        [
            SolarMasses::new(core.value().min(before_msun)),
            SolarMasses::new(self.current(1 - i).0),
        ]
    }

    /// Member `i`, of `before_msun` M☉, a giant whose core of `core` is a carbon–oxygen dwarf's
    /// (`Engine::dwarf_cores`), dies by its own track's `death`: where that is the track's carbon
    /// ignition (a thermally pulsing AGB star's or a helium star's,
    /// [`DeathKind::ThermonuclearDisruption`]), its candidate is pooled
    /// ([`ChandrasekharPath::KeptCoreIgnition`]). The track's own death leaves nothing.
    pub(super) fn own_carbon_ignition(
        &mut self,
        i: usize,
        death: DeathKind,
        core: SolarMasses,
        before_msun: f64,
    ) {
        if death == DeathKind::ThermonuclearDisruption {
            let masses = self.ignition_masses(i, core, before_msun);
            self.chandrasekhar_candidate(ChandrasekharPath::KeptCoreIgnition, masses);
        }
    }

    /// Member `i`, a white dwarf of `phase` and `m_msun` M☉ at its collapse mass, reached by `path`,
    /// ends now: an oxygen–neon one by accretion-induced collapse with no candidate, and any other as
    /// [`Engine::end_at_the_chandrasekhar_mass`] ends it, with a candidate of `masses`.
    pub(super) fn end_dwarf_at_its_collapse(
        &mut self,
        i: usize,
        (phase, m_msun): (Phase, f64),
        path: ChandrasekharPath,
        masses: [SolarMasses; 2],
    ) {
        if phase == Phase::OxygenNeonWhiteDwarf {
            self.accretion_induced_collapse(i, m_msun);
        } else {
            self.end_at_the_chandrasekhar_mass(i, m_msun, path, masses);
        }
    }

    /// Pools the Type Ia candidate of a dwarf or core at the Chandrasekhar mass reached by `path`,
    /// of `masses`, on [`IaPoolChannel::Accretion`]: the pair's first candidate unless it has one
    /// (P11.T6's pool holds a pair's first event). The caller ends the member.
    #[cfg_attr(
        not(test),
        expect(
            unused_variables,
            reason = "the path is read by the test build's census counter alone"
        )
    )]
    pub(super) fn chandrasekhar_candidate(
        &mut self,
        path: ChandrasekharPath,
        masses: [SolarMasses; 2],
    ) {
        let event = PooledIaEvent::new(Years::new(self.age), IaPoolChannel::Accretion, masses);
        #[cfg(test)]
        count_chandrasekhar_ending(path, event, self.pooled);
        self.pool(event);
    }
}

/// Counts an ending by `path` on this thread ([`chandrasekhar_endings`]): its `event`, and the
/// pair's candidate `before` it, if it has one.
#[cfg(test)]
fn count_chandrasekhar_ending(
    path: ChandrasekharPath,
    event: PooledIaEvent,
    before: Option<PooledIaEvent>,
) {
    use super::ia::is_detonatable;

    let k = ChandrasekharPath::ALL
        .iter()
        .position(|&p| p == path)
        .expect("every path is in ALL");
    CHANDRASEKHAR_ENDINGS.with(|counts| {
        let mut c = counts.get();
        c[k][0] += 1;
        match before {
            None => {
                c[k][1] += 1;
                c[k][2] += usize::from(is_detonatable(&event));
            }
            Some(before) => {
                c[k][3] += usize::from(is_detonatable(&event) && !is_detonatable(&before));
            }
        }
        counts.set(c);
    });
}

#[cfg(test)]
thread_local! {
    /// This thread's endings at the Chandrasekhar mass since it started ([`chandrasekhar_endings`]).
    static CHANDRASEKHAR_ENDINGS: core::cell::Cell<[[usize; 4]; 7]> =
        const { core::cell::Cell::new([[0; 4]; 7]) };
}

/// The endings of carbon–oxygen and helium white dwarfs and cores at the Chandrasekhar mass this
/// thread's engines have made since it started, by path ([`ChandrasekharPath::ALL`]'s order): each
/// path's endings, every one a disruption, the Type Ia candidates it pooled first for their pairs,
/// with those that can detonate (`ia::is_detonatable`), and the candidates that can detonate but
/// sit behind an earlier one of their pair's that cannot, which the pool keeps: a test build's
/// counter for the census.
#[cfg(test)]
#[must_use]
pub(crate) fn chandrasekhar_endings() -> [[usize; 4]; 7] {
    CHANDRASEKHAR_ENDINGS.with(core::cell::Cell::get)
}
