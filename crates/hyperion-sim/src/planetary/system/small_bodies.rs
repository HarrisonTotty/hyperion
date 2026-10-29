//! A small body's taxonomic class on the record (plan 14, P14.T51.e's wiring;
//! decision-p14-t51e-contract §1.4), on the record from generator version 24 (the 23 → 24 bump,
//! P14.T54.b.l, a P14.T54.b.j rider).
//!
//! The record's builders (`primary_record` for a planet or a belt's member, `satellite_record` for
//! a moon) fill it here from the body's **built** surface, after the surface is built and outside
//! `published_surface`, `SurfaceCase` and `with_held_members` (decision-p14-t51e-contract §2.2):
//!
//! - **the kind** ([`TaxonomyBodyKind`]): a planet; a member of a [`BeltKind::Kuiper`] belt, or a
//!   moon of one, a Kuiper-belt body; another belt's member; another moon;
//! - **the lid**: the crust's `CrustComposition::lid_lithology`, a registry row, as its
//!   [`CrustLithology`] by its key;
//! - **the veneer** ([`Veneer`]): `s_t`, the modifiers' organic ground times their organic cover,
//!   and `s_i`, the published condensates' solid, non-seasonal areas summed in their order, held to
//!   1, less `s_t`, held to +0, an upper bound, labelled;
//! - **the clocks** ([`WeatheringClocks`]): the figures' surface age `t_x`, the modifiers' wind term
//!   at an unshielded share of 1 (`w₀`), the reference wind term `w_ref` ([`reference_wind_term`]),
//!   the modifiers' maturity m, and the air: above T51.d's ion floor where the conditions' surface
//!   pressure over the bulk section's gravity exceeds `ION_STOPPING_COLUMN`, as the modifiers' own
//!   exposure reads it.
//!
//! A refusal of the pure rule's (`ClassifyTaxonomyError::NotGiven`: no crust where the rule reads a
//! lid, no modifiers where it reads the veneer or the clocks) is not modelled, never none
//! (decision-p14-t51e-contract §1.5).
//!
//! # The reference wind term
//!
//! `w_ref` is the same unshielded wind term on a stated reference body: a circular orbit at
//! `Q_TYPE_ANCHOR_DISTANCE`, 2.5 au, about T51.d's reference Sun (`ModifierHost::sun()`), with
//! no field and no air (main's ruling, 2026-10-26, on decision-p14-t51e-qs §1.2). It is derived by
//! `SurfaceModifiers::derive`, the rule every record's term comes from, once a query, and only by
//! a query that builds records (`snapshot_at`, `body_at` and `hooks_at`, each opting its epoch in
//! with `Epoch::with_small_bodies`, [`SmallBodyTypes::new`]); never once a body, never by a query
//! that builds none, and never cached: 0.15998 by the ruling's table.
//!
//! Every reach of the record into the pure module and into the modifiers' getters for it is in
//! this file.

use crate::planetary::chem::partition::{CondensatePhase, Condensates};
use crate::planetary::derive::atmosphere::PartialPressures;
use crate::planetary::hooks::crust::CrustLithology;
use crate::planetary::hooks::figures::MagneticField;
use crate::planetary::hooks::modifiers::{
    HeliocentricOrbit, ION_STOPPING_COLUMN, ModifierHost, ModifierInputs, Primary,
    SurfaceModifiers, WaterHistory,
};
use crate::planetary::hooks::small_bodies::{
    AirShield, BuildTaxonomyInputsError, ClassifyTaxonomyError, Q_TYPE_ANCHOR_DISTANCE,
    TaxonomicClass, TaxonomyBodyKind, TaxonomyInputs, Veneer, WeatheringClocks, WindTerms,
};
use crate::planetary::placement::OrbitHost;
use crate::planetary::record::{BeltKind, BulkProperties, Modelled, Section, Surface};
use crate::units::{
    Kelvin, Kilograms, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, Pascals,
    WattsPerSquareMetre, Years,
};

use super::{Body, Part, PlanetarySystem};

/// What one query's records' classes share, its epoch's: the reference wind term, derived once
/// for the query. An epoch holds none until a query that builds records opts it in
/// (`Epoch::with_small_bodies`), so that a record built on an epoch that did not opt in is not
/// modelled, never wrong.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SmallBodyTypes {
    /// The reference wind term `w_ref`, dimensionless, finite and positive
    /// ([`reference_wind_term`]).
    reference_wind_term: f64,
}

impl SmallBodyTypes {
    /// What a query that builds records shares: the reference wind term, derived here, once for
    /// the query.
    #[must_use]
    pub(super) fn new() -> Self {
        Self {
            reference_wind_term: reference_wind_term(),
        }
    }
}

/// The reference wind term `w_ref`, dimensionless: T51.d's wind term at an unshielded share of 1
/// ([`SurfaceModifiers::unshielded_wind_term`]) of the reference body ([`reference_inputs`]), a
/// circular orbit at `Q_TYPE_ANCHOR_DISTANCE`, 2.5 au, about `ModifierHost::sun()`: 0.15998, the
/// Moon's eccentricity cancelling (decision-p14-t51e-qs §1.2 and its table).
///
/// # Panics
///
/// Never: the reference body's stated inputs are each in their range, as this module's tests hold.
#[must_use]
fn reference_wind_term() -> f64 {
    let none = Condensates::default();
    SurfaceModifiers::derive(&reference_inputs(&none))
        .expect("the reference body's stated inputs are each in their range")
        .unshielded_wind_term()
}

/// The modifiers' inputs of the reference body whose wind term is `w_ref`, with the condensates
/// `none` (empty): `ModifierHost::sun()`, a circular orbit at `Q_TYPE_ANCHOR_DISTANCE`, about the
/// star, with no field and no air (main's ruling, 2026-10-26).
///
/// The rest are stated valid values the wind term does not read, each unread by it: a gravity of
/// 1 m s⁻², a surface age of 0, no resurfacing, no heat flow, a surface at 100 K, no mantle
/// sulphur, an ordinary chondrite's lid, no gases and no water.
#[must_use]
fn reference_inputs(none: &Condensates) -> ModifierInputs<'_> {
    ModifierInputs {
        host: ModifierHost::sun(),
        orbit: HeliocentricOrbit {
            semi_major_axis: Metres::from(Q_TYPE_ANCHOR_DISTANCE),
            eccentricity: 0.0,
        },
        primary: Primary::Star,
        gravity: MetresPerSecondSquared::new(1.0),
        surface_pressure: Pascals::ZERO,
        field: MagneticField::None,
        surface_age: Years::ZERO,
        resurfacing_time: Years::new(f64::INFINITY),
        heat_flow: WattsPerSquareMetre::ZERO,
        surface_temperature: Kelvin::new(100.0),
        mantle_sulphur: Kilograms::ZERO,
        condensates: none,
        lid: CrustLithology::ChondriteL.substance(),
        gases: PartialPressures::default(),
        water: WaterHistory::NeverWet,
    }
}

/// The record's surface section `surface` of the body `body` of `system`, as its builder built it,
/// with its taxonomic class ([`record_taxonomic_class`], through
/// [`Surface::with_taxonomic_class`]), whose bulk section is `bulk` (`None` where it is not
/// modelled), under what its query shares `types` (`None` where it did not opt in): the record
/// builders' one call, after the surface is built (decision-p14-t51e-contract §1.4). A section that
/// is not `Ok` is unchanged.
#[must_use]
pub(super) fn with_class(
    types: Option<SmallBodyTypes>,
    (system, body): (&PlanetarySystem, &Body),
    bulk: Option<&BulkProperties>,
    surface: Section<Surface>,
) -> Section<Surface> {
    surface.map(|surface| {
        let gravity = bulk.map(BulkProperties::surface_gravity);
        let class = record_taxonomic_class(types, (system, body), gravity, &surface);
        surface.with_taxonomic_class(class)
    })
}

/// The taxonomic class of the record's body `body` of `system`, whose built surface is `surface`
/// and whose bulk section's surface gravity is `gravity` (`None` where its bulk section is not
/// modelled), under what its query shares `types` (the [module](self) documentation):
/// `TaxonomicClass::of` over the surface's members, not modelled where the body's rule reads a
/// member the record does not give, never none, and not modelled where its query shares nothing
/// (`types` `None`), which computes nothing of it.
#[must_use]
fn record_taxonomic_class(
    types: Option<SmallBodyTypes>,
    (system, body): (&PlanetarySystem, &Body),
    gravity: Option<MetresPerSecondSquared>,
    surface: &Surface,
) -> Modelled<Option<TaxonomicClass>> {
    let Some(SmallBodyTypes {
        reference_wind_term,
    }) = types
    else {
        return Modelled::NotModelled;
    };
    taxonomy_kind(system, body).map_or(Modelled::NotModelled, |kind| {
        surface_class(kind, reference_wind_term, gravity, surface)
    })
}

/// The kind of `body` of `system` as its class reads it: a planet; a member of a
/// [`BeltKind::Kuiper`] belt, or a moon of one, a Kuiper-belt body; another member; another moon.
/// `None` for a ring, which has no surface.
///
/// # Panics
///
/// Never: a member's belt and a moon's parent are of its system.
#[must_use]
fn taxonomy_kind(system: &PlanetarySystem, body: &Body) -> Option<TaxonomyBodyKind> {
    let kuiper = |belt| match system
        .belt(belt)
        .expect("a member's belt is a belt of its system")
        .kind()
    {
        BeltKind::Kuiper => true,
        BeltKind::Asteroid => false,
    };
    Some(match &body.part {
        Part::Planet(_) => TaxonomyBodyKind::Planet,
        Part::Member(member) if kuiper(member.belt) => TaxonomyBodyKind::KuiperBody,
        Part::Member(_) => TaxonomyBodyKind::AsteroidBeltMember,
        Part::Moon(_) => {
            let parent = match body.host {
                OrbitHost::Body(parent) => system.body(parent),
                OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => None,
            }
            .expect("a moon's parent is a body of its system");
            match &parent.part {
                Part::Member(member) if kuiper(member.belt) => TaxonomyBodyKind::KuiperBody,
                Part::Planet(_) | Part::Member(_) | Part::Moon(_) | Part::Ring(_) => {
                    TaxonomyBodyKind::OtherMoon
                }
            }
        }
        Part::Ring(_) => return None,
    })
}

/// The class of a body of kind `kind` whose built surface is `surface` and whose gravity is
/// `gravity`, against the reference wind term `reference_wind_term`: not modelled where its rule
/// reads an input the surface does not give.
#[must_use]
fn surface_class(
    kind: TaxonomyBodyKind,
    reference_wind_term: f64,
    gravity: Option<MetresPerSecondSquared>,
    surface: &Surface,
) -> Modelled<Option<TaxonomicClass>> {
    let lid = match surface.crust() {
        Modelled::Ok(crust) => {
            CrustLithology::of_key(crust.lid_lithology().substance().key().as_str())
        }
        Modelled::NotModelled => None,
    };
    let modifiers = match surface.modifiers() {
        Modelled::Ok(modifiers) => Some(modifiers),
        Modelled::NotModelled => None,
    };
    // An input out of its range is one validated members never give; it is taken as not given,
    // so that a rule that reads it is not modelled, and one that does not is unaffected.
    let veneer = modifiers.and_then(|modifiers| veneer_of(modifiers, surface.condensates()).ok());
    let clocks = modifiers.zip(gravity).and_then(|(modifiers, gravity)| {
        let wind = WindTerms::new(modifiers.unshielded_wind_term(), reference_wind_term).ok()?;
        let air = air_shield(surface.conditions().surface_pressure(), gravity);
        WeatheringClocks::new(
            surface.figures().surface_age(),
            wind,
            modifiers.maturity(),
            air,
        )
        .ok()
    });
    match TaxonomicClass::of(&TaxonomyInputs::new(kind, lid, veneer, clocks)) {
        Ok(class) => Modelled::Ok(class),
        Err(ClassifyTaxonomyError::NotGiven(_)) => Modelled::NotModelled,
    }
}

/// The veneer of a surface whose modifiers are `modifiers` and whose published condensates are
/// `condensates`: `s_t`, the organic ground times the organic cover, and `s_i`, the solid,
/// non-seasonal entries' areas summed in their order, held to 1, less `s_t`, held to +0, an upper
/// bound, labelled, since the entries' areas may overlap (decision-p14-t51e-contract §1.2).
///
/// # Errors
///
/// As [`Veneer::new`]: a share that is not a number is kept, not held, so that it is refused
/// there, and a rule that reads the veneer is not modelled.
fn veneer_of(
    modifiers: &SurfaceModifiers,
    condensates: &Condensates,
) -> Result<Veneer, BuildTaxonomyInputsError> {
    let tholin = modifiers.organic_ground() * modifiers.organic_cover();
    let ice = condensates
        .entries()
        .iter()
        .filter(|entry| entry.phase() == CondensatePhase::Solid && !entry.is_seasonal())
        .fold(0.0, |sum, entry| sum + entry.area_fraction());
    let ice = if ice > 1.0 { 1.0 } else { ice };
    let uncovered = ice - tholin;
    // Held to +0 where it is at most 0, a negative zero included; a NaN reaches the veneer's check.
    let uncovered = if uncovered <= 0.0 { 0.0 } else { uncovered };
    Veneer::new(tholin, uncovered)
}

/// Whether the air of a surface at the pressure `pressure` under the gravity `gravity` stops the
/// wind above the ground: above T51.d's ion floor where its column p ÷ g exceeds
/// `ION_STOPPING_COLUMN`, the comparison the modifiers' own exposure makes, so that a column that
/// is not a number is above it too.
#[must_use]
fn air_shield(pressure: Pascals, gravity: MetresPerSecondSquared) -> AirShield {
    let column = KilogramsPerSquareMetre::new(pressure.value() / gravity.value());
    if column <= ION_STOPPING_COLUMN {
        AirShield::Open
    } else {
        AirShield::AboveIonFloor
    }
}

#[cfg(test)]
mod tests;
