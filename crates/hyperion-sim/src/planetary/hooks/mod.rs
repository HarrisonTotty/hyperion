//! Hooks for the layers above (plan 14, phase E; the brainstorm's "Hooks for the layers above"):
//! what the surface, life and civilisation generators read of a body, and no more.
//!
//! Built so far (P14.T23, T24.a, T24.b, T48.a–d, T48.f; the rendering plans' R09.T1.b):
//!
//! - [`seed`]: the body's [`SurfaceSeed`], one block output of the universe seed on
//!   `body.surface` keyed by the body's ID, which depends on nothing else, so that no change to the
//!   derivation moves a map; and its [`DetailSeed`] (R09.T1.b), likewise on `body.surface.detail`,
//!   which the client is given in the surface seed's place.
//! - [`BulkComposition`]: the body's mass fractions (P14.T11), the volatile inventory of its
//!   atmosphere (P14.T13) and its host's \[Fe/H\] and \[α/Fe\], which the resource model and the
//!   later generators read.
//! - [`surface`]: the body's [`SurfaceConditions`] (P14.T24.a): its mean temperature and signed
//!   contrasts, its surface pressure and gases as mole fractions, the light and radiation it
//!   receives, whether water can be liquid on it, and its [`ClimateRegime`] (P14.T48.d).
//! - [`figures`]: the body's [`GlobalFigures`] (P14.T24.b), so far its heat flow, tectonic
//!   regime, magnetic field, present outgassing and subsurface ocean, its surface age and crater
//!   contract (P14.T48.c) with its [`Impactors`] (P14.T48.f), its volcanism level, continents and
//!   relief (P14.T48.a), its surface water and the ocean weight θ the conditions read, and,
//!   read from the conditions, its volatile history (P14.T48.b), ocean, ice and cloud.
//!
//! The conditions and figures are a body's record's surface section
//! ([`record::Surface`](crate::planetary::record::Surface), P14.T48.e), from the 21 → 22 batch's
//! bump. [`BodyHooks`] holds them with the seed and the composition, from
//! [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at). The habitability
//! assessment (T25) and the resource abundances (T26) join it with their tasks.
//!
//! # The surface seed stays on the server
//!
//! The seed reveals a body's whole surface at once, and a client holds only what its ship has
//! seen (the rendering brainstorm's "Knowledge, and the surface seed"; the rendering plans' R04,
//! "a detail seed on the wire, the surface seed off it"). So [`BodyHooks`] is not part of a
//! [`BodyRecord`](crate::planetary::record::BodyRecord), and no wire type is built from it. The
//! record's hooks section ([`record::Hooks`](crate::planetary::record::Hooks)) carries the body's
//! [`DetailSeed`] in the surface seed's place (R09.T1.b), and nothing server-only.

pub mod figures;
pub mod seed;
pub mod surface;

pub use figures::{
    GlobalFigures, Impactors, MagneticField, OceanFloor, Subsurface, SubsurfaceOcean,
    TectonicRegime, impactors,
};
pub use seed::{DetailSeed, SurfaceSeed, detail_seed, surface_seed};
pub use surface::{ClimateRegime, RadiationClass, SurfaceConditions};

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

    /// The surface conditions (P14.T24.a), the body's record's surface section's, in its state
    /// (P14.T48.e): [`Section::Ok`] for a present body with a solid surface,
    /// [`Section::NotApplicable`] for one in the gas-envelope state or not present, and
    /// [`Section::NotModelled`] for a body whose spin or derivation this version does not compute.
    #[must_use]
    pub const fn surface(&self) -> &Section<SurfaceConditions> {
        &self.surface
    }

    /// The global figures (P14.T24.b): in the same state as [`surface`](Self::surface), since the
    /// conditions read their heat flow and the figures of later tasks read the conditions.
    #[must_use]
    pub const fn figures(&self) -> &Section<GlobalFigures> {
        &self.figures
    }
}
