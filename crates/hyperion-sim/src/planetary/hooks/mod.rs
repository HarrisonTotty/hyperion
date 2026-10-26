//! Hooks for the layers above (plan 14, phase E; the brainstorm's "Hooks for the layers above"):
//! what the surface, life and civilisation generators read of a body, and no more.
//!
//! Built so far (P14.T23, T24.a, T24.b, T24.d, T24.e, T48.a–d, T48.f, T52.d; the rendering plans'
//! R09.T1.b):
//!
//! - [`seed`]: the body's [`SurfaceSeed`], one block output of the universe seed on
//!   `body.surface` keyed by the body's ID, which depends on nothing else, so that no change to the
//!   derivation moves a map; and its [`DetailSeed`] (R09.T1.b), likewise on `body.surface.detail`,
//!   which the client is given in the surface seed's place.
//! - [`BulkComposition`]: the body's mass fractions (P14.T11), its element inventory (P14.T50.a),
//!   whose carriers are the volatile inventory of its atmosphere (P14.T13), and its host's
//!   \[Fe/H\] and \[α/Fe\], which the resource model and the later generators read.
//! - [`surface`]: the body's [`SurfaceConditions`] (P14.T24.a): its mean temperature and signed
//!   contrasts, its surface pressure and gases as mole fractions, the light and radiation it
//!   receives, whether water can be liquid on it, and its [`ClimateRegime`] (P14.T48.d).
//! - [`figures`]: the body's [`GlobalFigures`] (P14.T24.b), so far its heat flow, tectonic
//!   regime, magnetic field, present outgassing and subsurface ocean, its surface age and crater
//!   contract (P14.T48.c) with its [`Impactors`] (P14.T48.f), its volcanism level, continents and
//!   relief (P14.T48.a), its surface water and the ocean weight θ the conditions read, and,
//!   read from the conditions, its volatile history (P14.T48.b), ocean, ice and cloud.
//! - [`structure`]: the [`VerticalStructure`] of a surface's air (P14.T24.e), a scaled adiabat
//!   over P14.T49.b's `c_p`(T) to an isothermal skin, with [`temperature_at`](structure::temperature_at),
//!   which the flight model's drag and rendering plan R08 read, and of a gas envelope
//!   ([`EnvelopeProfile`], P14.T24.d), Guillot's grey profile joined to its dry adiabat.
//!   P14.T54.b's wiring puts the surface's in the record's surface section, held off the record
//!   until the 23 → 24 bump; the envelope's stays pure until P14.T54.b puts it in the record's
//!   envelope section.
//! - [`envelope_atmosphere`]: a gas envelope's visible atmosphere (P14.T24.d with P14.T52.d): its
//!   internal temperature, the record's giant cooling blended with a stated cooling law
//!   ([`envelope_internal_temperature`]), and its [`EnvelopeAtmosphere`], Guillot's profile on the
//!   envelope's own gas, its quenched and cold-trapped composition at 1 bar, its decks, its
//!   photochemical hazes and its absorbers at 10 mbar. It is pure until P14.T54.b puts it in the
//!   record's envelope section.
//! - [`modifiers`]: a solid surface's [`SurfaceModifiers`] (P14.T51.d, T51.j), its
//!   space-weathering rate and maturity, organic cover, resurfacing rate, an Io's volcanic
//!   deposits and an oxidised surface's `ferric`. It is pure: P14.T54.b's wiring puts it in the
//!   record's surface section, held off the record until the 23 → 24 bump.
//! - [`aerosols`]: a surface's [`AerosolInventory`] (P14.T24.c), its condensate decks from
//!   P14.T52.c's loop, its hazes and dust as modes of particles and its absorbers. It is pure:
//!   P14.T54.b's wiring puts it in the record's surface section, held off the record until the
//!   23 → 24 bump.
//! - [`sulphur`]: a body's sulphur (P14.T24.g), the acid deck of a runaway greenhouse, the
//!   sulphate layer of a temperate or snowball world and SO₂ as an absorber, which
//!   [`aerosols`] calls. It is pure until P14.T54.b.
//! - [`cold_traps`]: an airless body's cold traps (P14.T51.f): the exposed fraction of their ice,
//!   `f_x`, drawn once a body on `body.cold_traps`, the zonal annual maximum and the cold-trap
//!   fraction of each condensable, which the record's published condensates take behind a hold at
//!   generator version 24, until P14.T54.b's bump.
//! - `ocean`: a body's subsurface ocean beyond its flag (P14.T51.g), so far the floor of a regular
//!   moon's ocean at the liquidus of its base (P14.T51.g.a), which its figures'
//!   [`SubsurfaceOcean`] takes behind a hold at generator version 24, until P14.T54.b's bump.
//! - [`small_bodies`]: a small body's [`TaxonomicClass`](small_bodies::TaxonomicClass)
//!   (P14.T51.e), a label and not a substance, from its kind, its lid's lithology, its veneer and
//!   its weathering clocks. It is pure; its wiring puts it on the record behind a hold at
//!   generator version 24.
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

pub mod aerosols;
pub mod cold_traps;
pub mod crust;
pub mod envelope_atmosphere;
pub mod figures;
pub mod modifiers;
pub(crate) mod ocean;
pub mod seed;
pub mod small_bodies;
pub mod structure;
pub mod sulphur;
pub mod surface;

pub use aerosols::AerosolInventory;
pub use crust::{CrustComposition, CrustCompositionInputs, CrustLithology, DeriveCrustError};
pub use envelope_atmosphere::{
    DeckKind, DeriveEnvelopeAtmosphereError, DeriveInternalTemperatureError, EnvelopeAtmosphere,
    EnvelopeAtmosphereInputs, envelope_internal_temperature,
};
pub use figures::{
    GlobalFigures, Impactors, MagneticField, OceanFloor, Subsurface, SubsurfaceOcean,
    TectonicRegime, impactors,
};
pub use modifiers::{DeriveModifiersError, ModifierInputs, SurfaceModifiers};
pub use seed::{DetailSeed, SurfaceSeed, detail_seed, surface_seed};
pub use structure::envelope::{EnvelopeProfile, GreyProfile};
pub use structure::{ProfileNode, RadiativeConvective, VerticalStructure};
pub use surface::{ClimateRegime, RadiationClass, SurfaceConditions};

use crate::planetary::chem::inventory::ElementInventory;
use crate::planetary::derive::MassFractions;
use crate::planetary::derive::atmosphere::VolatileInventory;
use crate::planetary::record::Section;
use crate::units::Dex;

/// A body's bulk composition as the layers above read it (P14.T23): its mass fractions of iron,
/// rock, water and envelope, its element inventory (P14.T50.a), and its host's abundances.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BulkComposition {
    fractions: MassFractions,
    elements: ElementInventory,
    fe_h: Dex,
    alpha_fe: Option<Dex>,
}

impl BulkComposition {
    /// The composition of a body of mass fractions `fractions` and element inventory
    /// `elements`, about a host of \[Fe/H\] `fe_h` and \[α/Fe\] `alpha_fe` (`None` for a host
    /// with no stellar abundances of its own, as [`SystemContext::alpha_fe`](crate::planetary::SystemContext::alpha_fe) keeps it).
    #[must_use]
    pub const fn new(
        fractions: MassFractions,
        elements: ElementInventory,
        fe_h: Dex,
        alpha_fe: Option<Dex>,
    ) -> Self {
        Self {
            fractions,
            elements,
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
    /// before escape and the climate divide them. It is the stored carriers of the
    /// [`element_inventory`](Self::element_inventory), its view ([`ElementInventory::volatiles`]).
    #[must_use]
    pub const fn inventory(&self) -> &VolatileInventory {
        self.elements.volatiles()
    }

    /// The element inventory at the time (P14.T50.a): its eleven elements by reservoir, its stored
    /// carriers and its mantle's ΔIW, the body's derivation's
    /// ([`DerivedBody::element_inventory`](crate::planetary::derive::DerivedBody::element_inventory)).
    #[must_use]
    pub const fn element_inventory(&self) -> &ElementInventory {
        &self.elements
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
