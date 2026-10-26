//! The air's sections on the record (plan 14, P14.T54.b, the 23 → 24 batch's wiring): a surface's
//! vertical structure (P14.T24.e, P14.T54.b.a) and its aerosol and absorber inventory (P14.T24.c
//! with T24.g's sulphur, P14.T54.b.b), each held off the record until the bump
//! ([`AIR_SECTIONS_VERSION`]).
//!
//! The record's surface section (`published_surface`) builds its air's members here from what it
//! publishes: the vertical structure from its surface conditions with their climate regime, its
//! bulk section's equilibrium temperature and its figures' heat flow ([`record_structure`]); the
//! inventory from that structure, the state, the conditions, the figures, the bulk section's
//! gravity and the photochemistry's star ([`record_aerosols`]). Every reach of the record into
//! `hooks::aerosols` (and through it `hooks::sulphur` and `chem::condense`) is in this file, behind
//! the hold, so that the held arm calls none of them (the source scans' `HELD` lists,
//! decision-p14-t54b-open §3.3).

use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::hooks::aerosols::{AerosolInputs, AerosolInventory};
use crate::planetary::hooks::figures::GlobalFigures;
use crate::planetary::hooks::structure::{
    DeriveStructureError, RadiativeConvective, StructureInputs, VerticalStructure,
};
use crate::planetary::hooks::sulphur::RunawayDeck;
use crate::planetary::hooks::surface::SurfaceConditions;
use crate::planetary::record::Modelled;
use crate::units::{Kelvin, MetresPerSecondSquared, WattsPerSquareMetre};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which the air's sections are on the record: 24, the 23 → 24 batch's
/// bump, P14.T54.b ("the air and the rest of the minimum", decision-bump-plan-after-22 §1.3,
/// updated 2026-10-15).
///
/// It holds the members P14.T54.b first puts on the record: a surface's vertical structure
/// (P14.T24.e, P14.T54.b.a), and its aerosol and absorber inventory (P14.T24.c with T24.g), the
/// modifiers and the envelope as P14.T54.b's later subtasks wire them (decision-p14-t54b-open
/// §1.2 item 6). Below it [`AirRules::in_force`] is [`AirRules::Held`], so that each is not
/// modelled, as at 23, and every golden keeps its bits. The bump, P14.T54.b.l, deletes this hold,
/// [`AirRules`] with its `Held` arm, `RecordRules::air`, the four source scans with their `HELD`
/// lists, and its tripwire (`the_air_sections_are_held_only_until_the_24_bump`).
pub(crate) const AIR_SECTIONS_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether the record's surface carries its air's sections (the hold, [`AIR_SECTIONS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum AirRules {
    /// Versions 23's and before: the vertical structure and the aerosols are not modelled, and
    /// nothing of either is computed.
    Held,
    /// The 23 → 24 batch's: the surface carries its air's vertical structure, computed as none for
    /// a surface with no air ([`record_structure`]), and its aerosol and absorber inventory, the
    /// empty inventory for a surface with no air or a magma ocean ([`record_aerosols`]).
    Batch,
}

impl AirRules {
    /// The rules this generator version gives: the batch's from [`AIR_SECTIONS_VERSION`], 23's
    /// below it.
    #[must_use]
    pub(crate) fn in_force() -> Self {
        if GENERATOR_VERSION >= AIR_SECTIONS_VERSION {
            Self::Batch
        } else {
            Self::Held
        }
    }
}

/// The record's vertical structure of a surface whose conditions are `conditions`, under the rules
/// `rules`, at the bulk section's equilibrium temperature `equilibrium` and the figures' heat flow
/// `heat_flow` (P14.T24.e, wired by P14.T54.b; plan 14's "Deviations in P14.T24.e, as built").
///
/// Under [`AirRules::Held`] it is not modelled. Under [`AirRules::Batch`] it is
/// `RadiativeConvective::derive` over [`StructureInputs::of_surface`]: the conditions' mean
/// surface temperature, pressure and gases with their climate regime's condensable, and the skin of
/// `equilibrium` and the internal temperature of `heat_flow`.
///
/// - The equilibrium temperature is the bulk section's, from the hosts' light alone (ruling
///   112.7), the record's own; its sky's is the same value.
/// - The heat flow is the figures section's, the record's own K(P*), as plan 14's T54.b and T24.e
///   name it, so that a reader rebuilds the skin from what the record publishes. Rule (f)'s
///   basis, what the conditions' night floor read, is its published pass's context's: the same
///   within 10⁻⁹ for a converged body, and a bracketed body's by its context residual
///   (`SurfaceBasis`), which reaches the skin only through `T_int` = (F ÷ σ)^¼.
///
/// A surface with no air, a surface pressure of zero and no gases (an airless body's, or a melt's
/// whose volatiles are gone), is computed as none, `Ok(None)`, which the wire sends `null`. Any
/// other refusal (a mixture that is not one, a gas row with no heat capacity, a temperature that is
/// not finite and positive, an adiabat deeper than `MAX_SEGMENTS` segments) is not modelled: the
/// record's member carries no reason, and the generated sample gives none (this module's tests).
/// Conditions without their climate regime, which no record publishes, are not modelled either.
#[must_use]
pub(crate) fn record_structure(
    rules: AirRules,
    conditions: &SurfaceConditions,
    equilibrium: Kelvin,
    heat_flow: WattsPerSquareMetre,
) -> Modelled<Option<VerticalStructure>> {
    match rules {
        AirRules::Held => Modelled::NotModelled,
        AirRules::Batch => {
            let Some(inputs) = StructureInputs::of_surface(conditions, equilibrium, heat_flow)
            else {
                return Modelled::NotModelled;
            };
            match RadiativeConvective::derive(&inputs) {
                Ok(profile) => Modelled::Ok(Some(VerticalStructure::RadiativeConvective(profile))),
                Err(DeriveStructureError::NoAir(_)) => Modelled::Ok(None),
                Err(
                    DeriveStructureError::Mixture
                    | DeriveStructureError::NoHeatCapacity(_)
                    | DeriveStructureError::SurfaceTemperature(_)
                    | DeriveStructureError::SkinTemperature(_)
                    | DeriveStructureError::TooDeep,
                ) => Modelled::NotModelled,
            }
        }
    }
}

/// What the record's aerosol and absorber inventory reads of a published surface beside its
/// vertical structure (P14.T24.c with T24.g, wired by P14.T54.b.b; plan 14's T54.b list): its
/// state, its conditions, its figures, its bulk section's gravity and the photochemistry's star.
///
/// P14.T24.g's sulphur takes its case from the state, as T24.g takes it, until P14.T54.b.g gives
/// the inventory `case_of(state, deck)`, which joins these members as one more.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AirSurface<'a> {
    /// The surface's state (P14.T13.c): an airless body's and a magma ocean's inventory is empty.
    pub(crate) state: SurfaceState,
    /// Its conditions (P14.T24.a): the surface pressure, the gases and their mean molar mass, and
    /// the liquid-water flag P14.T24.g's SO₂ reads.
    pub(crate) conditions: &'a SurfaceConditions,
    /// Its published figures (P14.T24.b, second stage): the cloud fraction the decks cover
    /// (P14.T51.b's, the snapshot path's), the share of the ground its liquids cover, which the
    /// dust reads, and the outgassing O P14.T24.g's SO₂ reads.
    pub(crate) figures: &'a GlobalFigures,
    /// The bulk section's surface gravity.
    pub(crate) gravity: MetresPerSecondSquared,
    /// The effective temperature of the star whose ultraviolet drives the air's photochemistry,
    /// the brightest of its primary's hosts; `None` for a brown dwarf or no star, which makes no
    /// ozone, haze or acid.
    pub(crate) star_temperature: Option<Kelvin>,
}

/// The record's aerosol and absorber inventory of the surface `surface` whose air's vertical
/// structure is `structure` ([`record_structure`]'s), under the rules `rules` (P14.T24.c with
/// T24.g's sulphur, wired by P14.T54.b.b; plan 14's "Deviations in P14.T24.c, as built").
///
/// Under [`AirRules::Held`] it is not modelled, and nothing of it is computed. Under
/// [`AirRules::Batch`] it is `AerosolInventory::derive` over [`aerosol_inputs`]:
///
/// - a surface computed to hold no air (its structure `Ok(None)`: an airless body, a melt whose
///   volatiles are gone) has the empty inventory, as does a magma ocean (the inventory's own
///   rule), never none, as the condensates are an empty list;
/// - a structure not modelled leaves the inventory not modelled;
/// - an air the inventory refuses is not modelled: its error carries nothing a reader could act
///   on, and the generated sample gives none (this module's tests, which re-derive and print any).
#[must_use]
pub(crate) fn record_aerosols(
    rules: AirRules,
    surface: &AirSurface<'_>,
    structure: &Modelled<Option<VerticalStructure>>,
) -> Modelled<AerosolInventory> {
    match rules {
        AirRules::Held => Modelled::NotModelled,
        AirRules::Batch => {
            let profile = match structure {
                // A gas envelope's profile (P14.T24.d) is the envelope section's, never a
                // surface's: `record_structure` builds none, so no surface holds one.
                Modelled::NotModelled | Modelled::Ok(Some(VerticalStructure::Envelope(_))) => {
                    return Modelled::NotModelled;
                }
                Modelled::Ok(None) => return Modelled::Ok(AerosolInventory::empty()),
                Modelled::Ok(Some(VerticalStructure::RadiativeConvective(profile))) => profile,
            };
            let Some(inputs) = aerosol_inputs(surface, profile) else {
                return Modelled::Ok(AerosolInventory::empty());
            };
            AerosolInventory::derive(&inputs).map_or(Modelled::NotModelled, Modelled::Ok)
        }
    }
}

/// The inventory's inputs of the surface `surface` whose air's structure is `structure`, each the
/// published surface's own (plan 14's T54.b list): the state, the conditions' surface pressure,
/// gases (P14.T24.a's published fractions at the ground), mean molar mass and liquid-water flag,
/// the bulk section's gravity, the structure, the figures' cloud fraction, the share of the ground
/// their surface liquids cover (their summed cover held at 1, the ocean fraction) and their
/// outgassing, and the photochemistry's star. `None` for gases with no molar mass, a surface with
/// no gas, whose inventory is empty.
#[must_use]
fn aerosol_inputs<'a>(
    surface: &AirSurface<'a>,
    structure: &'a RadiativeConvective,
) -> Option<AerosolInputs<'a>> {
    let conditions = surface.conditions;
    let figures = surface.figures;
    Some(AerosolInputs {
        state: surface.state,
        surface_pressure: conditions.surface_pressure(),
        gases: conditions.gases(),
        mean_molar_mass_g_per_mol: conditions.mean_molar_mass_g_per_mol()?,
        gravity: surface.gravity,
        structure,
        cloud_fraction: figures.cloud_fraction(),
        star_temperature: surface.star_temperature,
        surface_liquid_cover: figures.ocean_fraction(),
        outgassing: figures.outgassing(),
        liquid_water: conditions.liquid_water(),
        // T24.g's case as the state gives it, until P14.T54.b.g reads P14.T53.b part 1 (b)'s
        // deck.
        runaway_deck: RunawayDeck::Kept,
    })
}

#[cfg(test)]
mod tests;
