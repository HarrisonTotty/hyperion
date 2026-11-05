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
//!   (design note 16), which is left to its pin (`Engine::pin_pending`): the dwarfs' merger, the
//!   merged dwarf's collapse in `Engine::finish_merger`, the edge-lit arm of
//!   `Engine::accretor_events` and the detached phase's stop wait for it, and the unplaced product's
//!   dwarf is typed by the held arm and held by `Engine::held_product`, as at 27;
//! - a thermally pulsing AGB product leaves an oxygen–neon dwarf where its core at the base of the
//!   AGB, as its placement solves it back, is 1.6 M☉ or more, a helium giant one where the product's
//!   mass, standing for the helium star's, is (HPT section 6.1), and either one where it merged from
//!   an oxygen–neon dwarf;
//! - a helium core of 0.5 M☉ or more, which is not degenerate, is the product's naked helium star
//!   (HPT section 6, before equation 76), where the held arm takes one only from the Chandrasekhar
//!   mass;
//! - only a carbon–oxygen or oxygen–neon dwarf a wind feeds stops at the Chandrasekhar mass in a
//!   detached phase: a helium dwarf ignites at 0.7 M☉ (BSE section 2.6.6), far below it;
//! - a thermally pulsing AGB product whose `Mc,BAGB`, as its placement solves it back, is 2.25 M☉
//!   or more (a carbon–oxygen core of 1.438 M☉ or more; the engine's `IRON_CORE_MC_BAGB`) and that
//!   merged from no dwarf has an iron core (HPT section 6), which BSE section 2.7.4 makes a neutron
//!   star or black hole (`gntage`: "a type 6 with this sized core mass cannot exist as it should
//!   already have become a NS or BH as a type 5"). The core is at its supernova already, since
//!   HPT's `Mc,SN` = 0.773 `Mc,BAGB` − 0.35 (equation 75) is the core itself at the `Mc,BAGB` it is
//!   solved back to, so it collapses at once with its hydrogen envelope on, by plan 06's iron-core
//!   collapse and its kick ([`Unplaced::IronCore`], `Engine::iron_core_collapse`), not by the
//!   accretion-induced collapse of a dwarf as the held arm has it.

use crate::GeneratorVersion;
use crate::stellar::Phase;
use crate::stellar::remnant::collapse::IRON_CORE_MC_BAGB;
use crate::stellar::remnant::structure::{CHANDRASEKHAR_MASS, OXYGEN_NEON_MC_BAGB};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse;
use crate::units::SolarMasses;

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
/// typing of [`DwarfLimitRules::unplaced`]'s new arm, the pin's bypasses and
/// `Engine::iron_core_collapse`, and refits the
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
    /// A star of `mass` whose carbon–oxygen core of `co_core` is an iron core, which collapses at
    /// once with its hydrogen envelope on.
    IronCore {
        /// Its carbon–oxygen core.
        co_core: SolarMasses,
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
    static UNPLACED: core::cell::Cell<[usize; 6]> = const { core::cell::Cell::new([0; 6]) };
}

/// The merger products this thread's engines could not place since it started, by what they
/// left: helium, carbon–oxygen and oxygen–neon dwarfs, naked helium stars, iron cores, and pinned
/// primaries' products left to their pin (a test build's counter, for the dwarf limit's census).
#[cfg(test)]
#[must_use]
pub(crate) fn unplaced_counts() -> [usize; 6] {
    UNPLACED.with(core::cell::Cell::get)
}

/// What a merger's product's core was ([`DwarfLimitRules::unplaced`]): an oxygen–neon white dwarf,
/// whose product's dwarf is oxygen–neon whatever its mass, or anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoreSource {
    /// An oxygen–neon white dwarf.
    OxygenNeonDwarf,
    /// Any other star's core.
    Other,
}

impl CoreSource {
    /// The source of a product whose core is a star of `kind`.
    #[must_use]
    pub(super) fn of(kind: Kind) -> Self {
        if kind == Kind::OxygenNeonWhiteDwarf {
            Self::OxygenNeonDwarf
        } else {
            Self::Other
        }
    }
}

impl DwarfLimitRules {
    /// What the product of `kind`, mass `m` and core `core` from `source` leaves where no star of
    /// its kind has such a core: see the module documentation.
    #[must_use]
    pub(super) fn unplaced(
        self,
        kind: Kind,
        m: SolarMasses,
        core: SolarMasses,
        source: CoreSource,
    ) -> Unplaced {
        let mass = SolarMasses::new(core.value().max(1e-3).min(m.value()));
        let oxygen_neon_source = source == CoreSource::OxygenNeonDwarf;
        match self {
            Self::Whole
                if kind == Kind::PulsingAgb
                    && source == CoreSource::Other
                    && sse::pulsing_agb_helium_core(core) >= IRON_CORE_MC_BAGB =>
            {
                Unplaced::IronCore {
                    co_core: mass,
                    mass: m,
                }
            }
            Self::Held if carbon_oxygen_core(kind) => {
                let oxygen_neon =
                    oxygen_neon_source || sse::pulsing_agb_helium_core(core) >= OXYGEN_NEON_MC_BAGB;
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
                let oxygen_neon = oxygen_neon_source
                    || if kind == Kind::PulsingAgb {
                        sse::pulsing_agb_helium_core(core) >= OXYGEN_NEON_MC_BAGB
                    } else {
                        // HPT section 6.1: a helium star's mass takes `Mc,BAGB`'s place, and the
                        // product's mass stands for the helium star's.
                        m >= OXYGEN_NEON_MC_BAGB
                    };
                Unplaced::WhiteDwarf {
                    phase: if oxygen_neon {
                        Phase::OxygenNeonWhiteDwarf
                    } else {
                        Phase::CarbonOxygenWhiteDwarf
                    },
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
    mass >= CHANDRASEKHAR_MASS.value()
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
