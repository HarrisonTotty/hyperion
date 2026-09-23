//! What a star leaves when it dies and the kick it gets: white dwarfs, neutron stars and black
//! holes, their structure and masses, and the natal kick law behind one interface (plan 06,
//! phase D).
//!
//! Neutron star and black hole masses follow Mandel and Müller (2020, MNRAS 499, 3214) by default;
//! a white dwarf's mass is the core mass its track ends with (plan 06, design note 9). The original
//! prescription of Hurley, Pols and Tout (2000) is kept for validation against the published SSE
//! output. `structure` holds the radii of the three kinds and the original prescription's
//! remnant masses (P06.T11).

pub(crate) mod structure;

// The death types of P06.T10.e, which the track integrator builds and T18 and T19 read.
mod death;
pub use death::{Death, DeathKind, ProgenitorAtDeath, Stripping, SupernovaType};

// HPT's cooling of white dwarfs and neutron stars, which the track's remnant stage needs before
// P06.T20 and T21 (ruling 33).
pub(crate) mod neutron_star;
pub(crate) mod white_dwarf;

use crate::units::SolarMasses;

/// Which prescription sets a remnant's mass and structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum RemnantRecipe {
    /// Hurley, Pols and Tout (2000, MNRAS 315, 543, section 6.2) as published and as the SSE code
    /// implements them: neutron stars of 1.4 × 10⁻⁵ R☉ (their "10 km") and 1.17 + 0.09 Mc,SN M☉,
    /// black holes where that mass passes 1.8 M☉, and black-hole radii of 4.24 × 10⁻⁶ R☉ per
    /// M☉. Kept so that the backbone can be validated against SSE's output (P06.T12.b).
    Hurley2000,
    /// The generator's default: neutron-star and black-hole masses after Mandel and Müller (2020,
    /// P06.T18), a neutron-star radius from current measurements, and the Schwarzschild radius
    /// from the nominal solar constants.
    #[default]
    MandelMuller2020,
}

/// What kind of object a dead star leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RemnantKind {
    /// A white dwarf of any composition.
    WhiteDwarf,
    /// A neutron star.
    NeutronStar,
    /// A black hole.
    BlackHole,
    /// Nothing: the star was destroyed.
    None,
}

/// A dead star's remnant: its kind and gravitational mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactRemnant {
    kind: RemnantKind,
    mass: SolarMasses,
}

impl CompactRemnant {
    /// A remnant of `kind` and gravitational `mass`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `mass` is not finite and non-negative, or is not zero for
    /// [`RemnantKind::None`].
    #[must_use]
    pub(crate) fn new(kind: RemnantKind, mass: SolarMasses) -> Self {
        debug_assert!(
            mass.value().is_finite() && mass.value() >= 0.0,
            "a remnant's mass is finite and non-negative: {mass:?}"
        );
        debug_assert!(
            kind != RemnantKind::None || mass.value() <= 0.0,
            "nothing is left where there is no remnant: {mass:?}"
        );
        Self { kind, mass }
    }

    /// The kind of remnant.
    #[must_use]
    pub const fn kind(&self) -> RemnantKind {
        self.kind
    }

    /// The remnant's gravitational mass, M☉; zero where there is none.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }
}

// Mandel and Müller's (2020) neutron stars and black holes, electron capture and pair instability
// (P06.T18.a–c), which the track's death (P06.T18.d) calls.
pub mod collapse;
