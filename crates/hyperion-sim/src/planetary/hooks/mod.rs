//! Hooks for the layers above (plan 14, phase E; the brainstorm's "Hooks for the layers above"):
//! what the surface, life and civilisation generators read of a body, and no more.
//!
//! Built so far (P14.T23, T24.a, T24.b, T24.d, T24.e, T25, T26, T48.a–d, T48.f, T52.d, T53.d; the
//! rendering plans' R09.T1.b):
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
//!   P14.T54.b's wiring puts the surface's in the record's surface section and the envelope's in
//!   its envelope section (P14.T54.b.i), each from the 23 → 24 bump.
//! - [`envelope_atmosphere`]: a gas envelope's visible atmosphere (P14.T24.d with P14.T52.d): its
//!   internal temperature, the record's giant cooling blended with a stated cooling law
//!   ([`envelope_internal_temperature`]), and its [`EnvelopeAtmosphere`], Guillot's profile on the
//!   envelope's own gas, its quenched and cold-trapped composition at 1 bar, its decks, its
//!   photochemical hazes and its absorbers at 10 mbar. It is pure: the record calls it for its
//!   envelope section (`system::envelope`, P14.T54.b.i) from the 23 → 24 bump.
//! - [`modifiers`]: a solid surface's [`SurfaceModifiers`] (P14.T51.d, T51.j), its
//!   space-weathering rate and maturity, organic cover, resurfacing rate, an Io's volcanic
//!   deposits and an oxidised surface's `ferric`. It is pure: P14.T54.b's wiring puts it in the
//!   record's surface section from the 23 → 24 bump.
//! - [`aerosols`]: a surface's [`AerosolInventory`] (P14.T24.c), its condensate decks from
//!   P14.T52.c's loop, its hazes and dust as modes of particles and its absorbers. It is pure:
//!   P14.T54.b's wiring puts it in the record's surface section from the 23 → 24 bump.
//! - [`sulphur`]: a body's sulphur (P14.T24.g), the acid deck of a runaway greenhouse, the
//!   sulphate layer of a temperate or snowball world and SO₂ as an absorber, which
//!   [`aerosols`] calls. It is pure; the record reads it through the aerosols from P14.T54.b.
//! - [`cold_traps`]: an airless body's cold traps (P14.T51.f): the exposed fraction of their ice,
//!   `f_x`, drawn once a body on `body.cold_traps`, the zonal annual maximum and the cold-trap
//!   fraction of each condensable, which the record's published condensates take from generator
//!   version 24 (P14.T54.b's bump).
//! - `ocean`: a body's subsurface ocean beyond its flag (P14.T51.g): the floor of a regular moon's
//!   ocean at the liquidus of its base (P14.T51.g.a), and every solid body's ice shell and ocean by
//!   a conductive solve with its chlorine and ammonia, which a moon's tidal ocean branch reads too
//!   (P14.T51.g.b). Its figures' [`SubsurfaceOcean`] takes them from generator version 24
//!   (P14.T54.b's bump).
//! - [`small_bodies`]: a small body's [`TaxonomicClass`] (P14.T51.e), a label and not a substance,
//!   from its kind, its lid's lithology, its veneer and its weathering clocks. It is pure; its
//!   wiring puts it on the record from generator version 24.
//! - [`habitability`]: a body's [`HabitabilityAssessment`] at a time (P14.T25), a class of whether
//!   liquid water stands on or under its surface and its limits from a closed list, read from its
//!   record, its seasonal orbit's place in its zone and its zone's stars. It says nothing about
//!   life, and it is not on the record.
//! - [`biosphere`]: a biosphere's net surface fluxes of O₂, CH₄ and N₂O ([`Biosphere`], P14.T53.d),
//!   the chemistry that takes them as sources against their stated sinks
//!   ([`BioticAir`](biosphere::BioticAir)), and the palette's cover with its `biological` flag
//!   ([`Cover`](biosphere::Cover)). It is pure, and no rule makes life: the record's surface
//!   section holds the biosphere as a member not modelled until a life plan exists, so it needs no
//!   field here.
//! - [`resources`]: the body's [`ResourceAbundances`] (P14.T26), a mass fraction of its
//!   accessible outer layers for each [`Resource`] of the plan's closed list, from its bulk
//!   composition, its crust's lid or its envelope's gas, the side of the snow line it formed on,
//!   its host's \[Fe/H\] and \[α/Fe\] and its age, times one log-normal multiple per resource
//!   drawn on `body.resources`. Server-only, with the hooks, and on no record.
//!
//! The conditions and figures are a body's record's surface section
//! ([`record::Surface`](crate::planetary::record::Surface), P14.T48.e), from the 21 → 22 batch's
//! bump. [`BodyHooks`] holds them with the seed and the composition, from
//! [`PlanetarySystem::hooks_at`](crate::planetary::PlanetarySystem::hooks_at), and with the
//! habitability assessment (T25) and the resource abundances (T26).
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
pub mod biosphere;
pub mod cold_traps;
pub mod crust;
pub mod envelope_atmosphere;
pub mod figures;
pub mod habitability;
pub mod modifiers;
pub(crate) mod ocean;
pub mod resources;
pub mod seed;
pub mod small_bodies;
pub mod structure;
pub mod sulphur;
pub mod surface;

pub use aerosols::AerosolInventory;
pub use biosphere::Biosphere;
pub use crust::{CrustComposition, CrustCompositionInputs, CrustLithology, DeriveCrustError};
pub use envelope_atmosphere::{
    DeckKind, DeriveEnvelopeAtmosphereError, DeriveInternalTemperatureError, EnvelopeAtmosphere,
    EnvelopeAtmosphereInputs, envelope_internal_temperature,
};
pub use figures::{
    GlobalFigures, Impactors, MagneticField, OceanFloor, Subsurface, SubsurfaceOcean,
    TectonicRegime, impactors,
};
pub use habitability::{
    HabitabilityAssessment, HabitabilityClass, HabitabilityLimit, HabitabilityLimits,
};
pub use modifiers::{DeriveModifiersError, ModifierInputs, SurfaceModifiers};
pub use resources::{Resource, ResourceAbundances};
pub use seed::{DetailSeed, SurfaceSeed, detail_seed, surface_seed};
pub use small_bodies::TaxonomicClass;
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

/// What the layers above read of a body at a time (P14.T23, T24.a, T24.b, T25, T26): its surface
/// seed, its bulk composition, its surface conditions, its global figures, its habitability
/// assessment and its resource abundances. Server-only (see the [module](self) documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyHooks {
    surface_seed: SurfaceSeed,
    bulk: Section<BulkComposition>,
    surface: Section<SurfaceConditions>,
    figures: Section<GlobalFigures>,
    habitability: Section<HabitabilityAssessment>,
    resources: Section<ResourceAbundances>,
}

impl BodyHooks {
    /// The hooks of a body of surface seed `surface_seed`, bulk composition `bulk`, surface
    /// conditions `surface` and global figures `figures`, its habitability and its resource
    /// abundances not modelled until [`with_habitability`](Self::with_habitability) and
    /// [`with_resources`](Self::with_resources).
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
            habitability: Section::NotModelled,
            resources: Section::NotModelled,
        }
    }

    /// The same hooks with the habitability assessment `habitability` (P14.T25).
    #[must_use]
    pub fn with_habitability(self, habitability: Section<HabitabilityAssessment>) -> Self {
        Self {
            habitability,
            ..self
        }
    }

    /// The same hooks with the resource abundances `resources` (P14.T26).
    #[must_use]
    pub fn with_resources(self, resources: Section<ResourceAbundances>) -> Self {
        Self { resources, ..self }
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

    /// The habitability assessment at the time (P14.T25): [`Section::Ok`] for a present body
    /// whose surface section is built, or that has none beneath an envelope, which is
    /// [`Hostile`](HabitabilityClass::Hostile); [`Section::NotApplicable`] for one not present;
    /// and the surface's state where that is not modelled or not resolved.
    #[must_use]
    pub const fn habitability(&self) -> &Section<HabitabilityAssessment> {
        &self.habitability
    }

    /// The resource abundances (P14.T26, [`resources`]): [`Section::Ok`] for a present body whose
    /// bulk composition is known and whose accessible outer layers are, its crust's lid or its
    /// envelope's gas; otherwise the state of the section that withholds them, [`Section::NotApplicable`]
    /// for a body not present and [`Section::NotModelled`] where this version computes no bulk,
    /// crust or envelope.
    #[must_use]
    pub const fn resources(&self) -> &Section<ResourceAbundances> {
        &self.resources
    }
}
