//! The binary engine's white dwarfs at the Chandrasekhar mass, and the dwarf a merger's product
//! leaves where no star can hold it (plan 11, P11.T4's fix).
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

use crate::stellar::Phase;
use crate::stellar::remnant::structure::{CHANDRASEKHAR_MASS, OXYGEN_NEON_MC_BAGB};
use crate::stellar::sse;

use super::star::Kind;

/// What a merger's product that no star of its kind can hold leaves ([`unplaced`]).
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

/// What the product of `kind`, `m` M☉ and core `core` M☉ leaves where no star of its kind has such
/// a core, with `dwarf` the kind of the white dwarf whose core it is, if it merged from one: see the
/// module documentation.
#[must_use]
pub(super) fn unplaced(kind: Kind, m: f64, core: f64, dwarf: Option<Kind>) -> Unplaced {
    let mass = core.max(1e-3).min(m);
    if carbon_oxygen_core(kind) {
        let oxygen_neon = dwarf == Some(Kind::OxygenNeonWhiteDwarf)
            || sse::pulsing_agb_helium_core(core) >= OXYGEN_NEON_MC_BAGB.value();
        return Unplaced::WhiteDwarf {
            phase: if oxygen_neon {
                Phase::OxygenNeonWhiteDwarf
            } else {
                Phase::CarbonOxygenWhiteDwarf
            },
            mass,
        };
    }
    if mass >= CHANDRASEKHAR_MASS.value() {
        return Unplaced::HeliumStar { mass };
    }
    Unplaced::WhiteDwarf {
        phase: if core < 0.5 {
            Phase::HeliumWhiteDwarf
        } else {
            Phase::CarbonOxygenWhiteDwarf
        },
        mass,
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
