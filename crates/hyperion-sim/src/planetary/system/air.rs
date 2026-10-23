//! The air's sections on the record (plan 14, P14.T54.b, the 23 → 24 batch's wiring): a surface's
//! vertical structure (P14.T24.e, P14.T54.b.a), with P14.T24.c's aerosols and T24.g's decks to
//! join it (P14.T54.b.b), each held off the record until the bump ([`AIR_SECTIONS_VERSION`]).
//!
//! The record's surface section (`published_surface`) builds its air's members here from what it
//! publishes: the vertical structure from its surface conditions with their climate regime, its
//! bulk section's equilibrium temperature and its figures' heat flow ([`record_structure`]).

use crate::planetary::hooks::structure::{
    DeriveStructureError, RadiativeConvective, StructureInputs, VerticalStructure,
};
use crate::planetary::hooks::surface::SurfaceConditions;
use crate::planetary::record::Modelled;
use crate::units::{Kelvin, WattsPerSquareMetre};
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
    /// Versions 23's and before: the vertical structure and the aerosols are not modelled.
    Held,
    /// The 23 → 24 batch's: the surface carries its air's vertical structure, computed as none for
    /// a surface with no air ([`record_structure`]).
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

#[cfg(test)]
mod tests;
