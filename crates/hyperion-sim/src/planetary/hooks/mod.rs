//! Hooks for the layers above (plan 14, phase E; the brainstorm's "Hooks for the layers above"):
//! what the surface, life and civilisation generators read of a body, and no more.
//!
//! Built so far (P14.T23, T24.a, T24.b's interior, T48.f):
//!
//! - [`seed`]: the body's [`SurfaceSeed`], one block output of the universe seed on
//!   `body.surface` keyed by the body's ID, which depends on nothing else, so that no change to the
//!   derivation moves a map.
//! - [`BulkComposition`]: the body's mass fractions (P14.T11), the volatile inventory of its
//!   atmosphere (P14.T13) and its host's \[Fe/H\] and \[α/Fe\], which the resource model and the
//!   later generators read.
//! - [`surface`]: the body's [`SurfaceConditions`] (P14.T24.a): its mean temperature and signed
//!   contrasts, its surface pressure and gases as mole fractions, the light and radiation it
//!   receives and whether water can be liquid on it; held until the 21 → 22 batch's bump.
//! - [`figures`]: the body's [`GlobalFigures`] (P14.T24.b), so far its heat flow, tectonic
//!   regime, magnetic field, present outgassing and subsurface ocean, its surface age and crater
//!   contract (P14.T48.c) with its [`Impactors`] (P14.T48.f), and its volatile history
//!   (P14.T48.b), its wet epoch and whether it came from a colder state, read from the
//!   conditions' liquid-water flag; held with the surface conditions.
//!
//! [`BodyHooks`] holds them, from
//! [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at). The habitability
//! assessment (T25) and the resource abundances (T26) join it with their tasks.
//!
//! # The surface seed stays on the server
//!
//! The seed reveals a body's whole surface at once, and a client holds only what its ship has
//! seen (the rendering brainstorm's "Knowledge, and the surface seed"; the rendering plans' R04,
//! "a detail seed on the wire, the surface seed off it"). So [`BodyHooks`] is not part of a
//! [`BodyRecord`](crate::planetary::record::BodyRecord), whose hooks section stays
//! [`Section::NotModelled`] until the wire's hooks carry a detail seed in its place, and no wire
//! type is built from it.

pub mod figures;
pub mod seed;
pub mod surface;

pub use figures::{
    GlobalFigures, Impactors, MagneticField, OceanFloor, Subsurface, SubsurfaceOcean,
    TectonicRegime, impactors,
};
pub use seed::{SurfaceSeed, surface_seed};
pub use surface::{RadiationClass, SurfaceConditions};

use crate::planetary::derive::MassFractions;
use crate::planetary::derive::atmosphere::VolatileInventory;
use crate::planetary::record::Section;
use crate::units::Dex;

/// A body's bulk composition as the layers above read it (P14.T23): its mass fractions of iron,
/// rock, water and envelope, its volatile inventory, and its host's abundances.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BulkComposition {
    fractions: MassFractions,
    inventory: VolatileInventory,
    fe_h: Dex,
    alpha_fe: Option<Dex>,
}

impl BulkComposition {
    /// The composition of a body of mass fractions `fractions` and volatile inventory
    /// `inventory`, about a host of \[Fe/H\] `fe_h` and \[α/Fe\] `alpha_fe` (`None` for a host
    /// with no stellar abundances of its own, as [`SystemContext::alpha_fe`](crate::planetary::SystemContext::alpha_fe) keeps it).
    #[must_use]
    pub const fn new(
        fractions: MassFractions,
        inventory: VolatileInventory,
        fe_h: Dex,
        alpha_fe: Option<Dex>,
    ) -> Self {
        Self {
            fractions,
            inventory,
            fe_h,
            alpha_fe,
        }
    }

    /// The mass fractions of iron, rock, water and envelope at the time (P14.T11), as the body's
    /// record gives them.
    #[must_use]
    pub const fn fractions(&self) -> MassFractions {
        self.fractions
    }

    /// The volatile inventory at the time (P14.T13.a): water, carbon dioxide, nitrogen and argon,
    /// before escape and the climate divide them.
    #[must_use]
    pub const fn inventory(&self) -> &VolatileInventory {
        &self.inventory
    }

    /// The host's \[Fe/H\], dex: the system's, as drawn (plan 06).
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.fe_h
    }

    /// The host's \[α/Fe\], dex ([`context::alpha_fe`](fn@crate::planetary::context::alpha_fe));
    /// `None`, "not modelled", for a host with no stellar abundances of its own.
    #[must_use]
    pub const fn alpha_fe(&self) -> Option<Dex> {
        self.alpha_fe
    }
}

/// What the layers above read of a body at a time (P14.T23, T24.a, T24.b): its surface seed, its
/// bulk composition, its surface conditions and its global figures. Server-only (see the
/// [module](self) documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyHooks {
    surface_seed: SurfaceSeed,
    bulk: Section<BulkComposition>,
    surface: Section<SurfaceConditions>,
    figures: Section<GlobalFigures>,
}

impl BodyHooks {
    /// The hooks of a body of surface seed `surface_seed`, bulk composition `bulk`, surface
    /// conditions `surface` and global figures `figures`.
    #[must_use]
    pub const fn new(
        surface_seed: SurfaceSeed,
        bulk: Section<BulkComposition>,
        surface: Section<SurfaceConditions>,
        figures: Section<GlobalFigures>,
    ) -> Self {
        Self {
            surface_seed,
            bulk,
            surface,
            figures,
        }
    }

    /// The surface seed, which a body keeps whatever happens to it.
    #[must_use]
    pub const fn surface_seed(&self) -> SurfaceSeed {
        self.surface_seed
    }

    /// The bulk composition: [`Section::Ok`] for a body present at the time,
    /// [`Section::NotApplicable`] for one not yet formed, destroyed or unbound, and
    /// [`Section::NotModelled`] for a moon whose derivation this version does not compute.
    #[must_use]
    pub const fn bulk(&self) -> &Section<BulkComposition> {
        &self.bulk
    }

    /// The surface conditions (P14.T24.a): [`Section::Ok`] for a present body with a surface,
    /// [`Section::NotApplicable`] for one in the gas-envelope state or not present, and
    /// [`Section::NotModelled`] for a body whose spin or derivation this version does not compute.
    /// Every body's is [`Section::NotModelled`] until the 21 → 22 batch's bump
    /// ([`surface`](mod@surface)'s documentation, "Held until the batch's bump").
    #[must_use]
    pub const fn surface(&self) -> &Section<SurfaceConditions> {
        &self.surface
    }

    /// The global figures (P14.T24.b): in the same state as [`surface`](Self::surface), since the
    /// conditions read their heat flow and the figures of later tasks read the conditions. Every
    /// body's is [`Section::NotModelled`] until the 21 → 22 batch's bump, as its conditions are.
    #[must_use]
    pub const fn figures(&self) -> &Section<GlobalFigures> {
        &self.figures
    }
}
