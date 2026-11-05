//! The binary engine's white dwarfs at the Chandrasekhar mass, and the dwarf a merger's product
//! leaves where no star can hold it (plan 11, P11.T4's fix, on the record from 27; its union's
//! fixes held at [`DWARF_LIMIT_VERSION`]).
//!
//! As first built, two routes showed a white dwarf above the Chandrasekhar mass, which the engine
//! collapses everywhere else (`Engine::accretor_events`, ruling 129.4c):
//!
//! - `Engine::new_star`'s fallback turns a merger's product that no star of its kind can hold into
//!   a white dwarf of its core: helium below 0.5 M☉, carbon–oxygen otherwise, with no
//!   Chandrasekhar check and never oxygen–neon. Under every 27 hold it acts 87 times in 20,000
//!   layer-E pairs near the Sun (2026-11-02): thermally pulsing AGB products with carbon–oxygen
//!   cores of 1.44–1.9 M☉, which BSE's `gntage` sets up to collapse ("a type 6 with this sized core mass
//!   cannot exist as it should already have become a NS or BH as a type 5"), and
//!   core-helium-burning products with helium cores of 3–29 M☉, shown as carbon–oxygen dwarfs for
//!   the rest of the timeline;
//! - a carbon–oxygen dwarf fed helium past the edge-lit detonation's 0.15 M☉ matches that arm of
//!   `Engine::accretor_events` at every later step, so the Chandrasekhar arm below it never acts,
//!   and the dwarf grows on (to 1.87 M☉ beside a helium-dwarf donor in one pair of layer D; 163
//!   of 20,000 layer-D pairs show such a dwarf).
//!
//! So the fallback takes the engine's own rules ([`unplaced`]), and no white dwarf passes the
//! Chandrasekhar mass:
//!
//! - a product around a carbon–oxygen core (a thermally pulsing AGB star or a helium giant) leaves
//!   an oxygen–neon dwarf where its core at the base of the AGB, as its placement solves it back
//!   (`sse::pulsing_agb_helium_core`), is 1.6 M☉ or more (HPT section 6, the engine's
//!   `OXYGEN_NEON_MC_BAGB`), or where the dwarf it merged from was one; a carbon–oxygen dwarf
//!   otherwise, at any mass;
//! - a helium core at or above the Chandrasekhar mass is no dwarf: the product is the naked helium
//!   star of that core with its share burnt, as the engine strips a core-helium-burning star
//!   (`Track::remains_at`), and plan 06 ends it (lean, 2026-11-02: an accretion-induced collapse
//!   would leave a 1.26 M☉ neutron star of a 26 M☉ core, and pool it as a Type Ia candidate);
//! - a merger that leaves a white dwarf at or above the Chandrasekhar mass collapses at once as an
//!   accreting one does (`Engine::finish_merger`): a Type Ia candidate pooled unless it is
//!   oxygen–neon, then the accretion-induced collapse;
//! - a dwarf fed helium at or above the Chandrasekhar mass collapses, edge-lit or not
//!   ([`collapses`]);
//! - a dwarf a wind feeds in a detached phase stops at the Chandrasekhar mass and collapses there
//!   (`Engine::collapse_fed_dwarf`), where as first built it grew on (an oxygen–neon dwarf of
//!   1.37 M☉ to 1.44 M☉ beside an AGB star in one pair of layer D, 2026-11-02).

//!
//! From 29 ([`DwarfLimitRules::Whole`], the white-dwarf union's fixes; the held arm,
//! [`DwarfLimitRules::Held`], is version 27's code verbatim):
//!
//! - no white dwarf passes the Chandrasekhar mass but a pinned primary's whose pin is pending
//!   (design note 16), which is left to its pin (`Engine::pin_pending`): the merged dwarf's
//!   collapse in `Engine::finish_merger`, the edge-lit arm of `Engine::accretor_events` and the
//!   detached phase's stop wait for it, and its product takes the held arm, placed or not, the
//!   unplaced one's dwarf held by `Engine::held_product`, as at 27; the dwarfs' own merger
//!   collapses such a primary's merged dwarf at once at the Chandrasekhar mass, as before the fix;
//! - a thermally pulsing AGB product leaves an oxygen–neon dwarf where its core at the base of the
//!   AGB, as its placement solves it back, is 1.6 M☉ or more, a helium giant one where the product's
//!   mass, standing for the helium star's, is (HPT section 6.1), and either one where it merged from
//!   an oxygen–neon dwarf;
//! - a helium core of 0.5 M☉ or more, which is not degenerate, is the product's naked helium star
//!   (HPT section 6, before equation 76), where the held arm takes one only from the Chandrasekhar
//!   mass;
//! - only a carbon–oxygen or oxygen–neon dwarf a wind feeds stops at the Chandrasekhar mass in a
//!   detached phase: a helium dwarf ignites at 0.7 M☉ (BSE section 2.6.6), far below it;
//! - a product a star of its kind can hold, of a carbon–oxygen or oxygen–neon dwarf parent, keeps
//!   that dwarf's kind ([`DwarfLimitRules::placed`], `Engine::dwarf_cores`; oxygen–neon if either
//!   parent's core is), so that its strip, its own death and a later unplaced product take it,
//!   whatever its track's `Mc,BAGB` would type it: BSE carries no composition and re-types the
//!   core, but carbon burning is not undone, and a cold carbon–oxygen core a shell grows below
//!   about 2 × 10⁻⁶ M☉ yr⁻¹ ignites carbon at its centre at the Chandrasekhar mass, not off it
//!   (Nomoto and Iben 1985, ApJ 297, 531; Kawai, Saio and Nomoto 1987, ApJ 315, 229), as on the
//!   thermally pulsing AGB; so such a core whose track would collapse ends as a carbon–oxygen core
//!   at the Chandrasekhar mass does, in a disruption that leaves nothing
//!   (fix-p11-placed-product-kind, research-leans-35 §7; `Engine::core_limit_age`). An early AGB's
//!   core grows at about that rate, where off-centre ignition, an oxygen–neon core and a collapse
//!   are as likely (lean, 2026-11-03: the ruling's ending kept, for main);
//! - a thermally pulsing AGB product whose `Mc,BAGB`, as its placement solves it back, is 2.25 M☉
//!   or more (a carbon–oxygen core of 1.438 M☉ or more; the engine's `IRON_CORE_MC_BAGB`) and that
//!   merged from no dwarf has an iron core (HPT section 6), which BSE section 2.7.4 makes a neutron
//!   star or black hole (`gntage`: "a type 6 with this sized core mass cannot exist as it should
//!   already have become a NS or BH as a type 5"). The core is at its supernova already, since
//!   HPT's `Mc,SN` = 0.773 `Mc,BAGB` − 0.35 (equation 75) is the core itself at the `Mc,BAGB` it is
//!   solved back to, so it collapses at once with its hydrogen envelope on, by plan 06's iron-core
//!   collapse and its kick ([`Unplaced::IronCore`], `Engine::iron_core_collapse`), not by the
//!   accretion-induced collapse of a dwarf as the held arm has it; so does a helium-giant product
//!   of no dwarf with such a core, whose helium star's carbon–oxygen core at its `Mc,SN` HPT
//!   section 6 makes a collapse too (fix-p11-helium-product-core, research-leans-35 §8), with no
//!   hydrogen and its helium on;
//! - a star whose core is a dwarf's (`Engine::dwarf_cores`) stops where that core reaches its limit
//!   (`detached::Stop::CoreLimit`, `Engine::core_limit_age`, reading the track's carbon–oxygen core
//!   with `Track::carbon_oxygen_core_at`): an oxygen–neon core collapses by electron capture at
//!   `OXYGEN_NEON_CAPTURE_MASS` under `MandelMuller2020` and at the Chandrasekhar mass under
//!   `Hurley2000`, and a carbon–oxygen core disrupts at the Chandrasekhar mass, where an early AGB
//!   of `Mc,BAGB` above 2.316 M☉ would collapse it later.

use crate::GeneratorVersion;
use crate::stellar::Phase;
use crate::stellar::remnant::collapse::IRON_CORE_MC_BAGB;
use crate::stellar::remnant::structure::{CHANDRASEKHAR_MASS, OXYGEN_NEON_MC_BAGB};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse;
use crate::units::SolarMasses;

use super::core_endings::CoreEndingRules;
use super::star::Kind;

/// The generator version whose stellar rules take the white-dwarf union's fixes to the dwarf limit
/// (P11.T4): `Engine::new_star_around`, the dwarfs' merger, `Engine::finish_merger`, the edge-lit
/// arm of `Engine::accretor_events` and the detached phase's step limit take
/// [`DwarfLimitRules::Whole`].
///
/// Below it every pair takes the held arm, [`DwarfLimitRules::Held`], the code of version 27
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_29_hold()`. The bump to 29 deletes the hold, not the rule: the
/// [`DwarfLimitRules::Held`] arm and its branches, the stellar rules' `dwarf_limit` field and its
/// reads in the engine, the brief's read of [`DwarfLimitRules::fitted_tables_hold`] and the
/// tripwire (`the_dwarf_limit_is_held_only_until_the_29_bump`). It keeps, applied always, the
/// typing of [`DwarfLimitRules::unplaced`]'s new arm, the record of [`DwarfLimitRules::placed`],
/// the pin's bypasses, `detached::Stop::CoreLimit` and `Engine::iron_core_collapse`, and refits the
/// pair-light and reach tables, which the engine's runs fit.
pub(crate) const DWARF_LIMIT_VERSION: GeneratorVersion = GeneratorVersion::new(29);

/// How the binary engine's dwarf limit treats a pinned primary and types the dwarf a merger's
/// unplaced product leaves (the stellar rules' `dwarf_limit` field, the hold of
/// [`DWARF_LIMIT_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum DwarfLimitRules {
    /// Version 27's: a pinned primary's merged or fed dwarf collapses at the Chandrasekhar mass
    /// like any other, the unplaced product is oxygen–neon by its core at the base of the AGB alone,
    /// and a helium core is its naked helium star only from the Chandrasekhar mass.
    Held,
    /// Version 29's: a pinned primary is left to its pin, a helium giant product is typed by its
    /// mass, and a non-degenerate helium core is a helium star.
    Whole,
}

/// The helium core, 0.5 M☉, below which an unplaced product is a helium white dwarf and from which
/// a carbon–oxygen one; under [`DwarfLimitRules::Whole`], the core from which a helium core is no
/// white dwarf but its naked helium star. A degenerate helium core ignites at about 0.45–0.49 M☉
/// (`Mc,HeI`, HPT section 5.3 and equation 49, below `M_HeF`), so one of 0.5 M☉ or more is not
/// degenerate, and HPT section 6 leaves a naked helium star of a non-degenerate core (before
/// equation 76).
const DEGENERATE_HELIUM_CORE_LIMIT: SolarMasses = SolarMasses::new(0.5);

/// What a merger's product that no star of its kind can hold leaves
/// ([`DwarfLimitRules::unplaced`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Unplaced {
    /// A white dwarf of `phase` and `mass`.
    WhiteDwarf {
        /// Its phase.
        phase: Phase,
        /// Its mass.
        mass: SolarMasses,
    },
    /// The naked helium star of `mass`, its helium burnt to the product's share.
    HeliumStar {
        /// Its mass.
        mass: SolarMasses,
    },
    /// A star of `mass` whose carbon–oxygen core of `co_core`, inside a helium core of
    /// `helium_core`, is an iron core, which collapses at once with its envelope on.
    IronCore {
        /// Its carbon–oxygen core.
        co_core: SolarMasses,
        /// Its helium core: the carbon–oxygen core for a thermally pulsing AGB star, as plan 06's
        /// death at `Mc,SN` takes it, and the whole star for a helium giant.
        helium_core: SolarMasses,
        /// Its mass.
        mass: SolarMasses,
    },
    /// A helium star of `mass` whose carbon–oxygen core of `co_core` is an oxygen–neon one, which
    /// collapses at once by electron capture (from version 29, P11.T4's `core_endings`; plan 06's
    /// `helium_star_end`, HPT sections 6 and 6.1).
    ElectronCapture {
        /// Its carbon–oxygen core.
        co_core: SolarMasses,
        /// Its mass.
        mass: SolarMasses,
    },
    /// A helium star of `mass` whose carbon–oxygen core ignites carbon at the Chandrasekhar mass,
    /// which disrupts it and leaves nothing, with no Type Ia candidate (from version 29, P11.T4's
    /// `core_endings`; plan 06's `helium_star_end`).
    Disruption {
        /// Its mass.
        mass: SolarMasses,
    },
}

impl Unplaced {
    /// Counts the product on this thread ([`unplaced_counts`]), `pinned` where it is a pinned
    /// primary's, left to its pin.
    #[cfg(test)]
    pub(super) fn count(self, pinned: bool) {
        let k = match self {
            _ if pinned => 5,
            Self::WhiteDwarf {
                phase: Phase::HeliumWhiteDwarf,
                ..
            } => 0,
            Self::WhiteDwarf {
                phase: Phase::OxygenNeonWhiteDwarf,
                ..
            } => 2,
            Self::WhiteDwarf { .. } => 1,
            Self::HeliumStar { .. } => 3,
            Self::IronCore { .. } => 4,
            Self::Disruption { .. } => 6,
            Self::ElectronCapture { .. } => 7,
        };
        UNPLACED.with(|counts| {
            let mut c = counts.get();
            c[k] += 1;
            counts.set(c);
        });
    }
}

#[cfg(test)]
thread_local! {
    /// This thread's unplaced products since it started ([`unplaced_counts`]).
    static UNPLACED: core::cell::Cell<[usize; 8]> = const { core::cell::Cell::new([0; 8]) };
}

/// The merger products this thread's engines could not place since it started, by what they
/// left: helium, carbon–oxygen and oxygen–neon dwarfs, naked helium stars, iron cores, pinned
/// primaries' products left to their pin, disruptions and electron captures (a test build's
/// counter, for the dwarf limit's census).
#[cfg(test)]
#[must_use]
pub(crate) fn unplaced_counts() -> [usize; 8] {
    UNPLACED.with(core::cell::Cell::get)
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
/// tracks' own, a white dwarf of the dwarf's kind, an electron capture and a disruption at their
/// tracks' deaths (`supernova::SwollenDeath`), and an electron capture and a disruption at their
/// cores' limits (`Engine::reach_core_limit`), a test build's counter for the dwarf limit's census
/// ([`Ending`]'s order).
#[cfg(test)]
#[must_use]
pub(crate) fn dwarf_core_deaths() -> [usize; 6] {
    DWARF_CORE_DEATHS.with(core::cell::Cell::get)
}

/// What a merger's product's core was ([`DwarfLimitRules::unplaced`]): a carbon–oxygen or
/// oxygen–neon white dwarf, whose product's dwarf is of its kind whatever its mass, since gaining
/// and losing a hydrogen envelope does not change a core's composition, or anything else. The
/// engine keeps it too for the giant each dwarf swells into, whose every exit takes it
/// (`Engine::dwarf_cores`; P11.T4's swollen dwarf).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoreSource {
    /// A carbon–oxygen white dwarf.
    CarbonOxygenDwarf,
    /// An oxygen–neon white dwarf.
    OxygenNeonDwarf,
    /// Any other star's core.
    Other,
}

impl CoreSource {
    /// The source of a product whose core is both this one's and `other`'s: oxygen–neon if either
    /// is (carbon burning is not undone), carbon–oxygen if either is otherwise (P11.T4's dwarf
    /// limit; research-leans-35 §7).
    #[must_use]
    pub(super) const fn with(self, other: Self) -> Self {
        match (self, other) {
            (Self::OxygenNeonDwarf, _) | (_, Self::OxygenNeonDwarf) => Self::OxygenNeonDwarf,
            (Self::CarbonOxygenDwarf, _) | (_, Self::CarbonOxygenDwarf) => Self::CarbonOxygenDwarf,
            (Self::Other, Self::Other) => Self::Other,
        }
    }

    /// The source of a product whose core is a star of `kind`.
    #[must_use]
    pub(super) fn of(kind: Kind) -> Self {
        match kind {
            Kind::CarbonOxygenWhiteDwarf => Self::CarbonOxygenDwarf,
            Kind::OxygenNeonWhiteDwarf => Self::OxygenNeonDwarf,
            Kind::LowMassMainSequence
            | Kind::MainSequence
            | Kind::HertzsprungGap
            | Kind::GiantBranch
            | Kind::CoreHeliumBurning
            | Kind::EarlyAgb
            | Kind::PulsingAgb
            | Kind::HeliumMainSequence
            | Kind::HeliumGap
            | Kind::HeliumGiant
            | Kind::HeliumWhiteDwarf
            | Kind::NeutronStar
            | Kind::BlackHole
            | Kind::Massless => Self::Other,
        }
    }

    /// The phase of the dwarf the core is, if it is one's.
    #[must_use]
    pub(super) const fn dwarf(self) -> Option<Phase> {
        match self {
            Self::CarbonOxygenDwarf => Some(Phase::CarbonOxygenWhiteDwarf),
            Self::OxygenNeonDwarf => Some(Phase::OxygenNeonWhiteDwarf),
            Self::Other => None,
        }
    }

    /// The phase of the white dwarf a core of this source leaves where a track types it `phase`:
    /// the source's dwarf's kind in place of a carbon–oxygen or oxygen–neon one, and `phase`
    /// otherwise.
    #[must_use]
    pub(super) fn dwarf_phase(self, phase: Phase) -> Phase {
        let Some(kind) = self.dwarf() else {
            return phase;
        };
        match phase {
            Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => kind,
            Phase::Protostar
            | Phase::PreMainSequence
            | Phase::MainSequence
            | Phase::HertzsprungGap
            | Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::HeliumMainSequence
            | Phase::HeliumHertzsprungGap
            | Phase::HeliumGiantBranch
            | Phase::PostAgb
            | Phase::HeliumWhiteDwarf
            | Phase::NeutronStar
            | Phase::BlackHole
            | Phase::NoRemnant
            | Phase::Substellar => phase,
        }
    }
}

impl DwarfLimitRules {
    /// The dwarf whose kind a merger's product that a star of its kind can hold keeps, its core
    /// from `source` (`Engine::dwarf_cores`): `source` under [`DwarfLimitRules::Whole`], since a
    /// core keeps its composition whatever its placement's `Mc,BAGB` would type it, and none under
    /// [`DwarfLimitRules::Held`], whose product takes its track's kind as BSE's does (BSE carries
    /// no composition).
    #[must_use]
    pub(super) const fn placed(self, source: CoreSource) -> CoreSource {
        match self {
            Self::Held => CoreSource::Other,
            Self::Whole => source,
        }
    }

    /// What the product of `kind`, mass `m` and core `core` from `source` leaves where no star of
    /// its kind has such a core: see the module documentation; but under
    /// [`CoreEndingRules::Unified`] (`endings`, from version 29) a helium giant of no dwarf's ends
    /// by its mass `m`, which stands for its helium star's and so for `Mc,BAGB` (HPT section 6.1):
    /// an iron core from 2.25 M☉, an oxygen–neon core's electron capture from 1.6 M☉ (HPT section
    /// 6; research-leans-40), a disruption that leaves nothing from the Chandrasekhar mass, and a
    /// carbon–oxygen dwarf of min(core, m) below.
    #[must_use]
    pub(super) fn unplaced(
        self,
        kind: Kind,
        m: SolarMasses,
        core: SolarMasses,
        source: CoreSource,
        endings: CoreEndingRules,
    ) -> Unplaced {
        let mass = SolarMasses::new(core.value().max(1e-3).min(m.value()));
        // From 29 a helium giant of no dwarf ends by its mass, as plan 06's helium star does with
        // its core at the Chandrasekhar mass (P11.T4's `core_endings`).
        let by_mass = endings == CoreEndingRules::Unified
            && matches!(kind, Kind::HeliumGap | Kind::HeliumGiant)
            && source == CoreSource::Other;
        match self {
            Self::Whole if by_mass && m >= IRON_CORE_MC_BAGB => Unplaced::IronCore {
                co_core: mass,
                helium_core: m,
                mass: m,
            },
            Self::Whole if by_mass && m >= OXYGEN_NEON_MC_BAGB => Unplaced::ElectronCapture {
                co_core: mass,
                mass: m,
            },
            Self::Whole if by_mass && m >= CHANDRASEKHAR_MASS => Unplaced::Disruption { mass: m },
            Self::Whole if by_mass => Unplaced::WhiteDwarf {
                phase: Phase::CarbonOxygenWhiteDwarf,
                mass,
            },
            Self::Whole
                if carbon_oxygen_core(kind) && source == CoreSource::Other && iron_core(core) =>
            {
                Unplaced::IronCore {
                    co_core: mass,
                    helium_core: if kind == Kind::PulsingAgb { mass } else { m },
                    mass: m,
                }
            }
            Self::Held if carbon_oxygen_core(kind) => {
                let oxygen_neon = source == CoreSource::OxygenNeonDwarf
                    || sse::pulsing_agb_helium_core(core) >= OXYGEN_NEON_MC_BAGB;
                Unplaced::WhiteDwarf {
                    phase: if oxygen_neon {
                        Phase::OxygenNeonWhiteDwarf
                    } else {
                        Phase::CarbonOxygenWhiteDwarf
                    },
                    mass,
                }
            }
            Self::Whole if carbon_oxygen_core(kind) => {
                let oxygen_neon = if kind == Kind::PulsingAgb {
                    sse::pulsing_agb_helium_core(core) >= OXYGEN_NEON_MC_BAGB
                } else {
                    // HPT section 6.1: a helium star's mass takes `Mc,BAGB`'s place, and the
                    // product's mass stands for the helium star's.
                    m >= OXYGEN_NEON_MC_BAGB
                };
                let phase = if oxygen_neon {
                    Phase::OxygenNeonWhiteDwarf
                } else {
                    Phase::CarbonOxygenWhiteDwarf
                };
                Unplaced::WhiteDwarf {
                    phase: source.dwarf_phase(phase),
                    mass,
                }
            }
            Self::Held if mass >= CHANDRASEKHAR_MASS => Unplaced::HeliumStar { mass },
            Self::Whole if core >= DEGENERATE_HELIUM_CORE_LIMIT => Unplaced::HeliumStar { mass },
            Self::Held | Self::Whole => Unplaced::WhiteDwarf {
                phase: if core < DEGENERATE_HELIUM_CORE_LIMIT {
                    Phase::HeliumWhiteDwarf
                } else {
                    Phase::CarbonOxygenWhiteDwarf
                },
                mass,
            },
        }
    }

    /// Whether a pinned primary whose pin is pending (`Engine::pin_pending`) is left to its pin
    /// rather than collapsed at the Chandrasekhar mass or stopped there: under
    /// [`DwarfLimitRules::Whole`] only.
    #[must_use]
    pub(super) fn leaves_the_pin(self) -> bool {
        self == Self::Whole
    }

    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold for a pair under this arm: under the version's arm only, until the bump refits
    /// them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).dwarf_limit
    }
}

/// Whether a white dwarf of `mass` M☉ collapses now: at or above the Chandrasekhar mass.
#[must_use]
pub(super) fn collapses(mass: f64) -> bool {
    collapses_at(mass, CHANDRASEKHAR_MASS)
}

/// [`collapses`] for a dwarf whose collapse mass is `at` (`core_endings::CoreEndingRules::
/// collapse_mass`: the Chandrasekhar mass, and from version 29 an oxygen–neon dwarf's capture mass
/// under the default recipe).
#[must_use]
pub(super) fn collapses_at(mass: f64, at: SolarMasses) -> bool {
    mass >= at.value()
}

/// Whether a carbon–oxygen core of `core` is an iron core: one of 1.438 M☉ or more, from which
/// `sse::pulsing_agb_helium_core`, as BSE's `gntage` solves it back for a type 6 (equation 75's
/// `Mc,SN` = 0.773 `Mc,BAGB` − 0.35 inverted), reaches `IRON_CORE_MC_BAGB`, 2.25 M☉ (HPT section
/// 6; 1.438 = 0.44 × 2.25 + 0.448, HPT equation 69's `Mc,DU` there).
#[must_use]
fn iron_core(core: SolarMasses) -> bool {
    sse::pulsing_agb_helium_core(core) >= IRON_CORE_MC_BAGB
}

/// Whether a merger's product of `kind` is placed around a carbon–oxygen core (a thermally pulsing
/// AGB star or a helium giant) rather than a helium one.
#[must_use]
fn carbon_oxygen_core(kind: Kind) -> bool {
    match kind {
        Kind::PulsingAgb | Kind::HeliumGap | Kind::HeliumGiant => true,
        Kind::LowMassMainSequence
        | Kind::MainSequence
        | Kind::HertzsprungGap
        | Kind::GiantBranch
        | Kind::CoreHeliumBurning
        | Kind::EarlyAgb
        | Kind::HeliumMainSequence
        | Kind::HeliumWhiteDwarf
        | Kind::CarbonOxygenWhiteDwarf
        | Kind::OxygenNeonWhiteDwarf
        | Kind::NeutronStar
        | Kind::BlackHole
        | Kind::Massless => false,
    }
}
