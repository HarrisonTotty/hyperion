//! Hooks for the layers above (plan 14, phase E; the brainstorm's "Hooks for the layers above"):
//! what the surface, life and civilisation generators read of a body, and no more.
//!
//! Built so far (P14.T23):
//!
//! - [`seed`]: the body's [`SurfaceSeed`], one block output of the universe seed on
//!   `body.surface` keyed by the body's ID, which depends on nothing else, so that no change to the
//!   derivation moves a map.
//! - [`BulkComposition`]: the body's mass fractions (P14.T11), the volatile inventory of its
//!   atmosphere (P14.T13) and its host's \[Fe/H\] and \[α/Fe\], which the resource model and the
//!   later generators read.
//!
//! [`BodyHooks`] holds them, from
//! [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at). The surface
//! conditions and global figures (P14.T24), the habitability assessment (T25) and the resource
//! abundances (T26) join it with their tasks.
//!
//! # The surface seed stays on the server
//!
//! The seed reveals a body's whole surface at once, and a client holds only what its ship has
//! seen (the rendering brainstorm's "Knowledge, and the surface seed"; the rendering plans' R04,
//! "a detail seed on the wire, the surface seed off it"). So [`BodyHooks`] is not part of a
//! [`BodyRecord`](crate::planetary::record::BodyRecord), whose hooks section stays
//! [`Section::NotModelled`] until the wire's hooks carry a detail seed in its place, and no wire
//! type is built from it.

pub mod seed;

pub use seed::{SurfaceSeed, surface_seed};

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

/// What the layers above read of a body at a time (P14.T23): its surface seed and its bulk
/// composition. Server-only (see the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyHooks {
    surface_seed: SurfaceSeed,
    bulk: Section<BulkComposition>,
}

impl BodyHooks {
    /// The hooks of a body of surface seed `surface_seed` and bulk composition `bulk`.
    #[must_use]
    pub const fn new(surface_seed: SurfaceSeed, bulk: Section<BulkComposition>) -> Self {
        Self { surface_seed, bulk }
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
}
