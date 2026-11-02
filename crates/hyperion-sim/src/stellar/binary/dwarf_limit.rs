//! The hold on the binary engine's white dwarfs at the Chandrasekhar mass, and on the dwarf a
//! merger's product leaves where no star can hold it (plan 11, P11.T4's fix, at
//! [`DWARF_LIMIT_VERSION`]).
//!
//! As built, two routes show a white dwarf above the Chandrasekhar mass, which the engine collapses
//! everywhere else (`Engine::accretor_events`, ruling 129.4c):
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
//! From 27 ([`DwarfLimitRules::Chandrasekhar`]) the fallback takes the engine's own rules
//! ([`DwarfLimitRules::unplaced`]), and no white dwarf passes the Chandrasekhar mass:
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
//!   ([`DwarfLimitRules::collapses`]);
//! - a dwarf a wind feeds in a detached phase stops at the Chandrasekhar mass and collapses there
//!   ([`DwarfLimitRules::limit`]), where the held arm lets it grow on (an oxygen–neon dwarf of
//!   1.37 M☉ to 1.44 M☉ beside an AGB star in one pair of layer D, 2026-11-02).

use crate::GeneratorVersion;
use crate::stellar::Phase;
use crate::stellar::remnant::structure::{CHANDRASEKHAR_MASS, OXYGEN_NEON_MC_BAGB};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse;

use super::star::Kind;

/// The generator version whose stellar rules hold every white dwarf of the binary engine below the
/// Chandrasekhar mass and type the dwarf a merger's unplaced product leaves (P11.T4's fix):
/// `Engine::new_star`, `Engine::finish_merger`, `Engine::mix_with`'s merger of two carbon–oxygen
/// or oxygen–neon dwarfs, `Engine::accretor_events` and the detached phase's step limit take
/// [`DwarfLimitRules::Chandrasekhar`].
///
/// Below it every pair takes the held arm, [`DwarfLimitRules::Unchecked`], the code of version 26
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_27_hold()`. The bump to 27 deletes this hold: this module with
/// [`DwarfLimitRules`], the stellar rules' `dwarf_limit` field, its reads in the engine with
/// `detached::Stop::Chandrasekhar` and `Engine::collapse_fed_dwarf`, the
/// held branch of the dwarfs' merger, the brief's read of
/// [`DwarfLimitRules::fitted_tables_hold`], and the tripwire
/// (`the_dwarf_limit_is_held_only_until_the_27_bump`); the bump refits the pair-light and reach
/// tables, which the engine's runs fit.
pub(crate) const DWARF_LIMIT_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether the binary engine holds its white dwarfs below the Chandrasekhar mass, and how it types
/// the dwarf a merger's unplaced product leaves (the stellar rules' `dwarf_limit` field, the hold
/// of [`DWARF_LIMIT_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum DwarfLimitRules {
    /// Version 26's and before: the unplaced product is the white dwarf of its core, helium below
    /// 0.5 M☉ and carbon–oxygen otherwise, at any mass; a dwarf fed helium past the edge-lit mark
    /// never collapses.
    Unchecked,
    /// Version 27's: the unplaced product is typed by its core, a helium core at or above the
    /// Chandrasekhar mass is a helium star, and a dwarf at or above it collapses.
    Chandrasekhar,
}

/// What a merger's product that no star of its kind can hold leaves
/// ([`DwarfLimitRules::unplaced`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Unplaced {
    /// A white dwarf of `phase` and `mass` M☉.
    WhiteDwarf {
        /// Its phase.
        phase: Phase,
        /// Its mass, M☉.
        mass: f64,
    },
    /// The naked helium star of `mass` M☉, its helium burnt to the product's share.
    HeliumStar {
        /// Its mass, M☉.
        mass: f64,
    },
}

impl DwarfLimitRules {
    /// What the product of `kind`, `m` M☉ and core `core` M☉ leaves where no star of its kind has
    /// such a core, with `dwarf` the kind of the white dwarf whose core it is, if it merged from
    /// one: under [`DwarfLimitRules::Unchecked`], the white dwarf of min(core, m), helium below a
    /// 0.5 M☉ core and carbon–oxygen otherwise; under [`DwarfLimitRules::Chandrasekhar`], see the
    /// module documentation.
    #[must_use]
    pub(super) fn unplaced(self, kind: Kind, m: f64, core: f64, dwarf: Option<Kind>) -> Unplaced {
        let mass = core.max(1e-3).min(m);
        match self {
            Self::Chandrasekhar if carbon_oxygen_core(kind) => {
                let oxygen_neon = dwarf == Some(Kind::OxygenNeonWhiteDwarf)
                    || sse::pulsing_agb_helium_core(core) >= OXYGEN_NEON_MC_BAGB.value();
                Unplaced::WhiteDwarf {
                    phase: if oxygen_neon {
                        Phase::OxygenNeonWhiteDwarf
                    } else {
                        Phase::CarbonOxygenWhiteDwarf
                    },
                    mass,
                }
            }
            Self::Chandrasekhar if mass >= CHANDRASEKHAR_MASS.value() => {
                Unplaced::HeliumStar { mass }
            }
            Self::Unchecked | Self::Chandrasekhar => Unplaced::WhiteDwarf {
                phase: if core < 0.5 {
                    Phase::HeliumWhiteDwarf
                } else {
                    Phase::CarbonOxygenWhiteDwarf
                },
                mass,
            },
        }
    }

    /// Whether a white dwarf of `mass` M☉ collapses now: at or above the Chandrasekhar mass under
    /// [`DwarfLimitRules::Chandrasekhar`], never by this rule under [`DwarfLimitRules::Unchecked`].
    #[must_use]
    pub(super) fn collapses(self, mass: f64) -> bool {
        match self {
            Self::Unchecked => false,
            Self::Chandrasekhar => mass >= CHANDRASEKHAR_MASS.value(),
        }
    }

    /// The mass, M☉, at which a white dwarf a wind feeds in a detached phase stops and collapses
    /// (`Engine::collapse_fed_dwarf`): the Chandrasekhar mass under
    /// [`DwarfLimitRules::Chandrasekhar`], none under [`DwarfLimitRules::Unchecked`].
    #[must_use]
    pub(super) fn limit(self) -> Option<f64> {
        match self {
            Self::Unchecked => None,
            Self::Chandrasekhar => Some(CHANDRASEKHAR_MASS.value()),
        }
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
