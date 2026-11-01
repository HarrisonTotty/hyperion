//! The envelope section on the record (plan 14, P14.T54.b.i, the 23 → 24 batch's wiring): a
//! sub-Neptune's or a giant's visible atmosphere, P14.T24.d's profile and internal heat with
//! P14.T52.d's chemistry, decks, hazes and absorbers on P14.T50.b's element abundances, on the
//! record from generator version 24 (the 23 → 24 bump, P14.T54.b.l; decision-p14-t54b-open §1.2
//! item 6), with the air's sections.
//!
//! The record's primary (`PlanetarySystem::envelope_section`) gives what it publishes
//! ([`EnvelopeOf`]) and the query's network ([`record_envelope`]). Every reach of the record into
//! `hooks::envelope_atmosphere` is in this file.
//!
//! From generator version 26 a solid body's under a supercritical envelope is its supercritical
//! envelope's model ([`record_supercritical_envelope`], P14.T53.f): its published air's visible
//! atmosphere and a water envelope's transit radius
//! ([`supercritical`](crate::planetary::hooks::envelope_atmosphere::supercritical)), behind its hold
//! ([`SUPERCRITICAL_ENVELOPE_VERSION`]).
//!
//! The chain ([`envelope_of`]): the host's \[α/Fe\] and the body's formation zone, which no
//! default replaces; P14.T24.d's internal temperature, the record's heat blended with its cooling
//! law; the envelope's effective temperature (`T_eq`⁴ + `T_int`⁴)^¼ with that `T_int`
//! (decision-p14-t24d-profile §3.5), which P14.T50.b's helium rain reads; P14.T50.b's abundances,
//! from generator version 26 at the body's formation draws (P14.T50.d, behind
//! `FORMATION_DRAWS_VERSION`); and P14.T52.d's atmosphere on the grey profile in force
//! (`GreyProfile::ADOPTED` to 26, κ₁ refitted from 27). Any refusal leaves the section not
//! modelled, and the record keeps no reason ([`DeriveEnvelopeError`] is the census's, and the
//! testing helper's).

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::chem::HostAbundances;
use crate::planetary::chem::envelope_abundances::{
    DeriveEnvelopeAbundancesError, EnvelopeAbundances, EnvelopeDraws, EnvelopeInputs,
    FormationDrawRules, Migration,
};
use crate::planetary::chem::equilibrium::{BuildNetworkError, Network};
use crate::planetary::derive::atmosphere::{
    Atmosphere, Gas, GreenhouseConstants, THICK_AIR_ANCHOR_PRESSURE,
};
use crate::planetary::derive::{BuildFormationZoneError, FormationZone, MassFractions};
use crate::planetary::hooks::envelope_atmosphere::supercritical::{
    DeriveSupercriticalAtmosphereError, SteamInflationInputs, SupercriticalAtmosphereInputs,
    steam_transit_inflation,
};
use crate::planetary::hooks::surface::atmosphere_mole_fractions;
use crate::planetary::hooks::{
    DeriveEnvelopeAtmosphereError, DeriveInternalTemperatureError, EnvelopeAtmosphere,
    EnvelopeAtmosphereInputs, GreyProfile, envelope_internal_temperature,
};
use crate::planetary::record::{Envelope, Modelled, Section};
use crate::units::{
    Dex, EarthMasses, Kelvin, Metres, MetresPerSecondSquared, Watts, WattsPerSquareMetre, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which a solid body's supercritical envelope is on the record, its
/// envelope section [`Section::Ok`] where its model gives one (P14.T53.f): 26, the planetary bump
/// after 25's, since P14.T53.f was placed with 25's train but not built (decision-p14-t53b-part3
/// §7).
///
/// It is P14.T53.f's one hold. Below it [`SupercriticalEnvelopeRules::in_force`] is
/// [`SupercriticalEnvelopeRules::Held`]: such a body's envelope section is not modelled, as every
/// figure on the record is version 25's bit for bit. 26's bump deletes this hold,
/// [`SupercriticalEnvelopeRules`] with its `Held` arm, the tests' switch and its tripwire
/// (`the_supercritical_envelope_is_held_only_until_the_26_bump`).
pub(crate) const SUPERCRITICAL_ENVELOPE_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// Whether a solid body's supercritical envelope is modelled on the record (its hold,
/// [`SUPERCRITICAL_ENVELOPE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SupercriticalEnvelopeRules {
    /// Version 25's and before: a solid body's envelope section under a supercritical envelope is
    /// not modelled.
    Held,
    /// Version 26's: it is its supercritical envelope's model ([`record_supercritical_envelope`]).
    Modelled,
}

impl SupercriticalEnvelopeRules {
    /// The rules this generator version gives: the model from [`SUPERCRITICAL_ENVELOPE_VERSION`],
    /// none below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_supercritical_envelope`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= SUPERCRITICAL_ENVELOPE_VERSION {
            Self::Modelled
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The supercritical envelope's rules a test sets on its thread, `None` for the version's.
    static RULES: core::cell::Cell<Option<SupercriticalEnvelopeRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the supercritical envelope's rules `rules` on this thread, for the tests to
/// reach P14.T53.f ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_supercritical_envelope<R>(
    rules: SupercriticalEnvelopeRules,
    run: impl FnOnce() -> R,
) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<SupercriticalEnvelopeRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// Whether a solid body's supercritical envelope is modelled on the record
/// ([`SupercriticalEnvelopeRules::Modelled`]); never under its hold.
#[must_use]
pub(crate) fn supercritical_envelope_in_force() -> bool {
    SupercriticalEnvelopeRules::in_force() == SupercriticalEnvelopeRules::Modelled
}

/// What a record's envelope reads of its body, its host and its time, as plain values, each the
/// record's own (`PlanetarySystem::envelope_inputs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EnvelopeOf {
    /// Its mass, its derivation's.
    pub(crate) mass: EarthMasses,
    /// Its published bulk's mass fractions, whose envelope share bounds the envelope's metals.
    pub(crate) fractions: MassFractions,
    /// Its published bulk's radius.
    pub(crate) radius: Metres,
    /// Its published bulk's surface gravity, which the profile's optical depth and the helium
    /// rain's adiabat read.
    pub(crate) gravity: MetresPerSecondSquared,
    /// Its published bulk's equilibrium temperature, from its hosts' light alone with its Bond
    /// albedo (ruling 112.7), from which `T_irr`⁴ = 4 `T_eq`⁴.
    pub(crate) equilibrium_temperature: Kelvin,
    /// Its internal luminosity, the record's (already times `giant_share`; ruling 112.7).
    pub(crate) internal_luminosity: Watts,
    /// Its age at the time.
    pub(crate) age: Years,
    /// Where it formed in its host's disc: [`DeriveEnvelopeError::NoDisc`] for a host with no disc,
    /// and [`DeriveEnvelopeError::FormationZone`] for a zone refused, a formation distance or snow
    /// line that is not positive and finite.
    pub(crate) formation: Result<FormationZone, DeriveEnvelopeError>,
    /// Its host's \[Fe/H\], dex.
    pub(crate) fe_h: Dex,
    /// Its host's \[α/Fe\], dex, `None` for a host with no abundances of its own (ruling 33),
    /// which is not modelled, never solar.
    pub(crate) alpha_fe: Option<Dex>,
    /// Its host's C/O and Mg/Si (P14.T50.a).
    pub(crate) host: HostAbundances,
    /// Its envelope's He/H by number before any helium rain: the host star's own, fractionated by
    /// its escape where its derivation carries it (P14.T53.a, P14.T54.b.h).
    pub(crate) primordial_helium_per_hydrogen: f64,
    /// Whether a star's ultraviolet lights it, which drives its photochemistry.
    pub(crate) starlit: bool,
    /// Its formation draws, read from generator version 26 (P14.T50.d,
    /// [`FormationDrawRules`]).
    pub(crate) draws: EnvelopeDraws,
    /// Whether it migrated inward from where it formed, which its solids' rank reads.
    pub(crate) migration: Migration,
}

/// Why a record's envelope is not modelled, or the testing helper's
/// envelope by hand gives none: the census's reasons, of which the record keeps none
/// (P14.T54.b.b's rule: `Section::NotModelled` carries no reason).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveEnvelopeError {
    /// The host has no \[α/Fe\] of its own, which is never defaulted.
    NoAlphaFe,
    /// The host has no disc to place the body's formation in.
    NoDisc,
    /// The body's formation zone is not one: a formation distance or snow line that is not
    /// positive and finite, which a record's zone, a placed body's distance against its disc's
    /// snow line, never gives.
    FormationZone(BuildFormationZoneError),
    /// P14.T24.d's internal temperature refused its inputs.
    InternalTemperature(DeriveInternalTemperatureError),
    /// P14.T50.b refused its inputs.
    Abundances(DeriveEnvelopeAbundancesError),
    /// P14.T52.a's network could not be built, which the registry as built never gives.
    Network(BuildNetworkError),
    /// P14.T52.d refused the envelope: it is not hydrogen-dominated (decision-p14-t24d-profile
    /// §4), its 1-bar or 10-mbar level is hotter than the network's records (T52.d left that case
    /// to P14.T54.b; main's ruling, 2026-10-27: not modelled), or a step refused its inputs.
    Atmosphere(DeriveEnvelopeAtmosphereError),
    /// A solid body's air has no supercritical reservoir that hides its surface under the bulk's
    /// gravity, or no surface pressure: a steam envelope's label alone, which the bulk of another
    /// gravity than its derivation's can give (P14.T53.b part 3), is not modelled (P14.T53.f).
    NoSupercriticalReservoir,
    /// P14.T53.f's model refused the supercritical envelope's air.
    Supercritical(DeriveSupercriticalAtmosphereError),
}

impl fmt::Display for DeriveEnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAlphaFe => write!(f, "the host has no [α/Fe] of its own"),
            Self::NoDisc => write!(f, "the host has no disc to place the body's formation in"),
            Self::FormationZone(error) => write!(f, "the formation zone: {error}"),
            Self::InternalTemperature(error) => write!(f, "the internal temperature: {error}"),
            Self::Abundances(error) => write!(f, "the abundances: {error}"),
            Self::Network(error) => write!(f, "the network: {error}"),
            Self::Atmosphere(error) => write!(f, "the atmosphere: {error}"),
            Self::NoSupercriticalReservoir => {
                write!(f, "no supercritical reservoir hides the surface")
            }
            Self::Supercritical(error) => write!(f, "the supercritical envelope: {error}"),
        }
    }
}

impl Error for DeriveEnvelopeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FormationZone(error) => Some(error),
            Self::InternalTemperature(error) => Some(error),
            Self::Abundances(error) => Some(error),
            Self::Network(error) => Some(error),
            Self::Atmosphere(error) => Some(error),
            Self::Supercritical(error) => Some(error),
            Self::NoAlphaFe | Self::NoDisc | Self::NoSupercriticalReservoir => None,
        }
    }
}

/// The record's envelope section of the body `of` describes, on the query's network `network`
/// (P14.T54.b.i): [`envelope_of`]'s envelope, and not modelled for every refusal
/// ([`DeriveEnvelopeError`]), whose reason the record does not keep.
#[must_use]
pub(super) fn record_envelope(
    of: &EnvelopeOf,
    network: Result<&Network, BuildNetworkError>,
) -> Section<Envelope> {
    network
        .map_err(DeriveEnvelopeError::Network)
        .and_then(|network| envelope_of(of, network))
        .map_or(Section::NotModelled, Section::Ok)
}

/// The envelope of the body `of` describes, on P14.T52.a's network `network` (the [module](self)'s
/// chain).
///
/// # Errors
///
/// [`DeriveEnvelopeError::NoAlphaFe`] or [`DeriveEnvelopeError::NoDisc`] for a host with no
/// \[α/Fe\] or no disc, before anything is computed; then each step's refusal,
/// [`DeriveEnvelopeError::InternalTemperature`] for P14.T24.d's internal temperature's,
/// [`DeriveEnvelopeError::Abundances`] for P14.T50.b's and [`DeriveEnvelopeError::Atmosphere`] for
/// P14.T52.d's.
pub(crate) fn envelope_of(
    of: &EnvelopeOf,
    network: &Network,
) -> Result<Envelope, DeriveEnvelopeError> {
    let alpha_fe = of.alpha_fe.ok_or(DeriveEnvelopeError::NoAlphaFe)?;
    let formation = of.formation?;
    let internal_temperature =
        envelope_internal_temperature(of.internal_luminosity, of.radius, of.mass, of.age)
            .map_err(DeriveEnvelopeError::InternalTemperature)?;
    let inputs = EnvelopeInputs {
        mass: of.mass,
        fractions: of.fractions,
        formation_distance: formation.distance(),
        snow_line: formation.snow_line(),
        fe_h: of.fe_h,
        alpha_fe,
        host: of.host,
        effective_temperature: effective_temperature(
            of.equilibrium_temperature,
            internal_temperature,
        ),
        age: of.age,
        surface_gravity: of.gravity,
        primordial_helium_per_hydrogen: of.primordial_helium_per_hydrogen,
    };
    let abundances = match FormationDrawRules::in_force() {
        FormationDrawRules::Trend => EnvelopeAbundances::derive(&inputs),
        FormationDrawRules::Drawn => {
            EnvelopeAbundances::derive_drawn(&inputs, &of.draws, of.migration)
        }
    }
    .map_err(DeriveEnvelopeError::Abundances)?;
    let atmosphere = EnvelopeAtmosphere::derive(&EnvelopeAtmosphereInputs {
        abundances: &abundances,
        network,
        internal_temperature,
        equilibrium_temperature: of.equilibrium_temperature,
        gravity: of.gravity,
        fe_h: of.fe_h,
        grey: GreyProfile::in_force(),
        starlit: of.starlit,
    })
    .map_err(DeriveEnvelopeError::Atmosphere)?;
    Ok(Envelope::hydrogen_helium(abundances, atmosphere))
}

/// What a solid body's supercritical envelope reads of its body (P14.T53.f), each the record's own
/// ([`supercritical_section`](super::supercritical_section)): its derivation's published air and flux, and its published
/// bulk's radius, gravity and equilibrium temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SupercriticalOf<'a> {
    /// Its published air, its derivation's.
    pub(crate) air: &'a Atmosphere,
    /// Its published bulk's radius.
    pub(crate) radius: Metres,
    /// Its published bulk's surface gravity, under which its reservoir hides its surface.
    pub(crate) gravity: MetresPerSecondSquared,
    /// Its published bulk's equilibrium temperature.
    pub(crate) equilibrium_temperature: Kelvin,
    /// The flux it receives from its hosts at the time, averaged over its orbit, its derivation's.
    pub(crate) stellar_flux: WattsPerSquareMetre,
}

/// The record's envelope section of a solid body's supercritical envelope `of` describes
/// (P14.T53.f): not modelled under its hold ([`supercritical_envelope_in_force`]) and for every
/// refusal of [`supercritical_envelope_of`], whose reason the record does not keep.
#[must_use]
pub(super) fn record_supercritical_envelope(of: &SupercriticalOf<'_>) -> Section<Envelope> {
    if !supercritical_envelope_in_force() {
        return Section::NotModelled;
    }
    supercritical_envelope_of(of).map_or(Section::NotModelled, Section::Ok)
}

/// The supercritical envelope of the solid body `of` describes (P14.T53.f; the
/// [`supercritical`](crate::planetary::hooks::envelope_atmosphere::supercritical) module's
/// leans): its condensable, the reservoir's that hides its surface under the bulk's gravity
/// ([`Atmosphere::supercritical_reservoir`]); its visible atmosphere on its published air
/// ([`EnvelopeAtmosphere::supercritical`]), its grey profile's opacity the air's own above the
/// thick-air closure's anchor, 100 bar ([`THICK_AIR_ANCHOR_PRESSURE`], the closure's top depth at
/// the bulk's equilibrium temperature), or above its surface where it is thinner; and for a water
/// envelope the transit radius's inflation over its whole fluid, the air's water and the reservoir's
/// beneath ([`steam_transit_inflation`]), not modelled outside the fit's grid and for any other
/// condensable.
///
/// # Errors
///
/// [`DeriveEnvelopeError::NoSupercriticalReservoir`] where no reservoir hides the surface under the
/// bulk's gravity or the air has no surface pressure; [`DeriveEnvelopeError::Supercritical`] for
/// the model's refusal.
pub(crate) fn supercritical_envelope_of(
    of: &SupercriticalOf<'_>,
) -> Result<Envelope, DeriveEnvelopeError> {
    let air = of.air;
    let reservoir = air
        .supercritical_reservoir(of.gravity)
        .ok_or(DeriveEnvelopeError::NoSupercriticalReservoir)?;
    let surface_pressure = air
        .surface_pressure()
        .ok_or(DeriveEnvelopeError::NoSupercriticalReservoir)?;
    let partials = air.partial_pressures();
    let reference = GreenhouseConstants::in_force()
        .thick_air(partials, of.equilibrium_temperature, of.gravity)
        .map_or((surface_pressure, air.optical_depth()), |(top, _, _)| {
            (THICK_AIR_ANCHOR_PRESSURE, top)
        });
    let gases = atmosphere_mole_fractions(partials);
    let atmosphere = EnvelopeAtmosphere::supercritical(&SupercriticalAtmosphereInputs {
        gases: &gases,
        surface_pressure,
        surface_temperature: air.surface_temperature(),
        reference,
        equilibrium_temperature: of.equilibrium_temperature,
        gravity: of.gravity,
    })
    .map_err(DeriveEnvelopeError::Supercritical)?;
    let gas = reservoir.gas();
    let inflation = if gas == Gas::Water {
        steam_transit_inflation(&SteamInflationInputs {
            water_column: partials.of(Gas::Water) + reservoir.beneath_the_air(),
            radius: of.radius,
            gravity: of.gravity,
            stellar_flux: of.stellar_flux,
            surface_temperature: air.surface_temperature(),
        })
        .map_or(Modelled::NotModelled, Modelled::Ok)
    } else {
        Modelled::NotModelled
    };
    Ok(Envelope::of_condensable(
        gas.substance(),
        atmosphere,
        inflation,
    ))
}

/// The envelope's effective temperature, (`T_eq`⁴ + `T_int`⁴)^¼ of its equilibrium temperature
/// `equilibrium` and its internal temperature `internal` (decision-p14-t24d-profile §3.5): the one
/// temperature its profile radiates, which P14.T50.b's helium rain reads, not the bulk section's
/// `T_eff`, which below 0.414 Jupiter masses has no internal heat of its own.
#[must_use]
fn effective_temperature(equilibrium: Kelvin, internal: Kelvin) -> Kelvin {
    Kelvin::new(math::powf(
        math::powi(equilibrium.value(), 4) + math::powi(internal.value(), 4),
        0.25,
    ))
}

#[cfg(test)]
pub(crate) mod tests;
