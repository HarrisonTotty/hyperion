//! The surface's modifiers on the record (plan 14, P14.T54.b.c, the 23 → 24 batch's wiring):
//! P14.T51.d's space-weathering rate and maturity, organic cover, resurfacing rate and an Io's
//! volcanic deposits, with P14.T51.j's oxidised surface, held off the record until the bump behind
//! the air's sections' hold ([`AirRules`], `AIR_SECTIONS_VERSION`; decision-p14-t54b-open §1.2
//! item 6), since they too are a member P14.T54.b first puts on the record and move no published
//! value.
//!
//! The record's surface section (`published_surface`) builds them here ([`record_modifiers`]) from
//! what it publishes and what its frame reads of the system: the host star, the zone's brightest at
//! the time ([`ModifierStar`]); the orbit about it, a moon's planet's; what the body orbits
//! ([`RecordPrimary`]), a moon's planet with that planet's magnetosphere from its own record
//! ([`planet_field`]); the bulk section's gravity; the conditions' surface pressure and mean
//! surface temperature; the figures' field, surface age and heat flow, with the resurfacing time
//! their surface age read; the mantle's sulphur of the body's element inventory; the published
//! condensates; the crust's lid; the air's partial pressures; and the water's history, from the
//! conditions' liquid-water flag and the figures' wet epoch. Every reach of the record into
//! `hooks::modifiers` is in this file, behind the hold, so that the held arm computes nothing (the
//! source scan's `HELD` list, `modifiers_nothing_on_the_record_reads_the_modifiers_yet`,
//! decision-p14-t54b-open §3.3).

use crate::orbit::KeplerElements;
use crate::planetary::chem::partition::Condensates;
use crate::planetary::context::XuvHistory;
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::atmosphere::PartialPressures;
use crate::planetary::hooks::crust::CrustComposition;
use crate::planetary::hooks::figures::surface_age::resurfacing_time;
use crate::planetary::hooks::figures::{GlobalFigures, MagneticField};
use crate::planetary::hooks::modifiers::{
    HeliocentricOrbit, ModifierHost, ModifierInputs, PlanetMagnetosphere, Primary,
    SurfaceModifiers, WaterHistory,
};
use crate::planetary::hooks::surface::SurfaceConditions;
use crate::planetary::record::{BodyRecord, Modelled, Section};
use crate::units::{Kilograms, Metres, MetresPerSecondSquared, SolarRadii, Years};

use super::air::AirRules;

/// The star a body's modifiers read (P14.T51.d's `ModifierHost`): the most luminous of the stars
/// its primary's zone orbits at the time, the first of equals in the zone's order, the star
/// P14.T51.f's cold traps and P14.T24.c's photochemistry read too (`Epoch::brightest`), with its
/// radius then and its X-ray and ultraviolet history. A pair's zone takes its brightest star alone,
/// labelled: the weathering's wind and light are one star's (for the composition audit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ModifierStar {
    /// Its radius at the time.
    pub(super) radius: SolarRadii,
    /// Its X-ray and ultraviolet history (P14.T1.a), read at the system's age then.
    pub(super) xuv: XuvHistory,
}

/// A planet as its moons' modifiers read its magnetosphere, from its record: its radius and its
/// field's class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PlanetField {
    /// Its bulk section's radius.
    pub(super) radius: Metres,
    /// Its field: its surface figures' for a solid class, [`MagneticField::Strong`] for a gas
    /// envelope's.
    pub(super) field: MagneticField,
}

/// What a body's modifiers read of what it orbits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum RecordPrimary {
    /// A planet or a belt's member, about its host stars.
    Star,
    /// A moon, about its planet.
    Planet {
        /// The planet's magnetosphere from its record ([`planet_field`]); `None` where the record
        /// does not give it, which leaves the moon's modifiers not modelled.
        field: Option<PlanetField>,
        /// The moon's semi-major axis about the planet at the time.
        semi_major_axis: Metres,
    },
}

/// What a body's record's modifiers read beyond its published surface: its host star, what it
/// orbits and its mantle's sulphur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ModifierCase {
    /// The host star at the time ([`ModifierStar`]); `None` before any star of its zone forms.
    pub(super) star: Option<ModifierStar>,
    /// What it orbits.
    pub(super) primary: RecordPrimary,
    /// Its mantle's sulphur, P14.T50.a's element inventory's, which an Io's deposits read.
    pub(super) mantle_sulphur: Kilograms,
}

/// What a body's record's modifiers read of its published surface and its orbit then.
#[derive(Debug, Clone, Copy)]
pub(super) struct ModifierSurface<'a> {
    /// The orbit about its host stars at the time, a moon's planet's, with the zone's G M.
    pub(super) seasonal: &'a KeplerElements,
    /// The system's age at the time, at which the star's X-ray and ultraviolet light is read.
    pub(super) age: Years,
    /// The bulk section's surface gravity.
    pub(super) gravity: MetresPerSecondSquared,
    /// The published conditions (P14.T24.a).
    pub(super) conditions: &'a SurfaceConditions,
    /// The published figures, after their second stage (P14.T24.b).
    pub(super) figures: &'a GlobalFigures,
    /// The published condensates (P14.T51.a).
    pub(super) condensates: &'a Condensates,
    /// The record's crust (P14.T51.c); `None` for a body whose passes took no lid.
    pub(super) crust: Option<&'a CrustComposition>,
    /// The air's partial pressures at the surface (P14.T13).
    pub(super) gases: &'a PartialPressures,
}

/// The planet's magnetosphere its moons' records read, from its record `record` under the rules
/// `rules`: its bulk section's radius, with its surface figures' field for a solid class, and
/// [`MagneticField::Strong`] for a gas envelope's (`interior::magnetic_field`'s first clause, which
/// reads the class alone: a giant's metallic hydrogen or an ice giant's ionic water convecting).
///
/// `None` under [`AirRules::Held`], which computes nothing, and where the record gives neither: a
/// planet with no bulk section, or of a solid class whose surface section is not modelled or not
/// applicable.
#[must_use]
pub(super) fn planet_field(rules: AirRules, record: &BodyRecord) -> Option<PlanetField> {
    match rules {
        AirRules::Held => None,
        AirRules::Batch => {
            let Section::Ok(bulk) = record.bulk() else {
                return None;
            };
            let field = match bulk.class() {
                PlanetClass::SubNeptune | PlanetClass::IceGiant | PlanetClass::GasGiant => {
                    MagneticField::Strong
                }
                PlanetClass::Rocky | PlanetClass::Icy => match record.surface() {
                    Section::Ok(surface) => surface.figures().magnetic_field(),
                    Section::NotResolved | Section::NotModelled | Section::NotApplicable => {
                        return None;
                    }
                },
            };
            Some(PlanetField {
                radius: Metres::from(bulk.radius()),
                field,
            })
        }
    }
}

/// The record's modifiers of the body `case` describes, whose published surface is `surface`,
/// under the rules `rules` (P14.T51.d with T51.j, wired by P14.T54.b.c; plan 14's "Deviations in
/// P14.T51.d, as built" and "Deviations in P14.T51.j, as built").
///
/// Under [`AirRules::Held`] they are not modelled, and nothing of them is computed. Under
/// [`AirRules::Batch`] they are `SurfaceModifiers::derive` over [`modifier_inputs`], and not
/// modelled where those are none (no crust on the record, no host star at the time, a moon whose
/// planet's field its record does not give) or the derivation refuses them: an unlit body's mean
/// surface temperature of 0 K (`ModifierInput::SurfaceTemperature`), which the record states as its
/// equilibrium temperature, the one refusal the generated sample gives (this module's tests count
/// and print every refusal). A refusal's reason is not carried: the record's member has none.
#[must_use]
pub(super) fn record_modifiers(
    rules: AirRules,
    case: &ModifierCase,
    surface: &ModifierSurface<'_>,
) -> Modelled<SurfaceModifiers> {
    match rules {
        AirRules::Held => Modelled::NotModelled,
        // A refusal carries nothing a reader could act on, and the record's member holds no
        // reason: the surface's modifiers are then not modelled.
        AirRules::Batch => modifier_inputs(case, surface)
            .and_then(|inputs| SurfaceModifiers::derive(&inputs).ok())
            .map_or(Modelled::NotModelled, Modelled::Ok),
    }
}

/// The modifiers' inputs of the body `case` describes, whose published surface is `surface`, each
/// the record's own (plan 14's T54.b list), or `None` where the record gives no crust, no host
/// star or no planet's field:
///
/// - **the host:** the star's radius and its X-ray and ultraviolet luminosity at the system's age
///   ([`XuvHistory::luminosity`], as the derivation reads its flux), and the zone's G M from the
///   orbit;
/// - **the orbit:** its semi-major axis and eccentricity, a moon's planet's;
/// - **the primary:** the star, or a moon's planet's magnetosphere with the moon's semi-major axis
///   about it;
/// - **the surface:** the bulk section's gravity, the conditions' surface pressure and mean surface
///   temperature, the figures' field, surface age and heat flow, and the resurfacing time their
///   surface age read (`surface_age::resurfacing_time` of the impactors' crust at that heat flow,
///   tides included, so the record's tide rule);
/// - **the rest:** the mantle's sulphur, the published condensates, the crust's lid
///   ([`CrustComposition::lid_lithology`]), the air's partial pressures, and the water: wet now
///   where the conditions' liquid-water flag holds (P14.T24.a), else dry since the figures' wet
///   epoch's end (P14.T48.b's `WetEpoch::end`), else never wet.
#[must_use]
fn modifier_inputs<'a>(
    case: &ModifierCase,
    surface: &ModifierSurface<'a>,
) -> Option<ModifierInputs<'a>> {
    let star = case.star?;
    let lid = surface.crust?.lid_lithology();
    let primary = match case.primary {
        RecordPrimary::Star => Primary::Star,
        RecordPrimary::Planet {
            field,
            semi_major_axis,
        } => {
            let planet = field?;
            Primary::Planet(PlanetMagnetosphere {
                radius: planet.radius,
                field: planet.field,
                semi_major_axis,
            })
        }
    };
    let (conditions, figures, seasonal) = (surface.conditions, surface.figures, surface.seasonal);
    let water = if conditions.liquid_water() {
        WaterHistory::WetNow
    } else {
        figures.wet_epoch().map_or(WaterHistory::NeverWet, |epoch| {
            WaterHistory::DrySince(epoch.end())
        })
    };
    Some(ModifierInputs {
        host: ModifierHost {
            radius: Metres::from(star.radius),
            xuv_luminosity: star.xuv.luminosity(surface.age),
            gravitational_parameter: seasonal.gravitational_parameter(),
        },
        orbit: HeliocentricOrbit {
            semi_major_axis: seasonal.semi_major_axis(),
            eccentricity: seasonal.eccentricity().value(),
        },
        primary,
        gravity: surface.gravity,
        surface_pressure: conditions.surface_pressure(),
        field: figures.magnetic_field(),
        surface_age: figures.surface_age(),
        resurfacing_time: resurfacing_time(figures.impactors().crust(), figures.heat_flow()),
        heat_flow: figures.heat_flow(),
        surface_temperature: conditions.mean_surface_temperature(),
        mantle_sulphur: case.mantle_sulphur,
        condensates: surface.condensates,
        lid,
        gases: *surface.gases,
        water,
    })
}

#[cfg(test)]
mod tests;
