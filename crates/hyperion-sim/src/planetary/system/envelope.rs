//! The envelope section on the record (plan 14, P14.T54.b.i, the 23 → 24 batch's wiring): a
//! sub-Neptune's or a giant's visible atmosphere, P14.T24.d's profile and internal heat with
//! P14.T52.d's chemistry, decks, hazes and absorbers on P14.T50.b's element abundances, held off
//! the record until the bump by the air's sections' hold ([`AirRules`], `AIR_SECTIONS_VERSION`;
//! decision-p14-t54b-open §1.2 item 6): no hold of its own.
//!
//! The record's primary (`PlanetarySystem::envelope_section`) gives what it publishes
//! ([`EnvelopeOf`]) and the query's network, each through a closure, so that under the hold
//! nothing of it is read or computed ([`record_envelope`]). Every reach of the record into
//! `hooks::envelope_atmosphere` is in this file, behind the hold (the envelope scan's `HELD` list,
//! decision-p14-t54b-open §3.3).
//!
//! The chain ([`envelope_of`]): the host's \[α/Fe\] and the body's formation zone, which no
//! default replaces; P14.T24.d's internal temperature, the record's heat blended with its cooling
//! law; the envelope's effective temperature (`T_eq`⁴ + `T_int`⁴)^¼ with that `T_int`
//! (decision-p14-t24d-profile §3.5), which P14.T50.b's helium rain reads; P14.T50.b's abundances;
//! and P14.T52.d's atmosphere on [`GreyProfile::ADOPTED`]. Any refusal leaves the section not
//! modelled, and the record keeps no reason ([`DeriveEnvelopeError`] is the census's, and the
//! testing helper's).

use std::error::Error;
use std::fmt;

use super::air::AirRules;
use crate::math;
use crate::planetary::chem::HostAbundances;
use crate::planetary::chem::envelope_abundances::{
    DeriveEnvelopeAbundancesError, EnvelopeAbundances, EnvelopeInputs,
};
use crate::planetary::chem::equilibrium::{BuildNetworkError, Network};
use crate::planetary::derive::{BuildFormationZoneError, FormationZone, MassFractions};
use crate::planetary::hooks::{
    DeriveEnvelopeAtmosphereError, DeriveInternalTemperatureError, EnvelopeAtmosphere,
    EnvelopeAtmosphereInputs, GreyProfile, envelope_internal_temperature,
};
use crate::planetary::record::{Envelope, Section};
use crate::units::{Dex, EarthMasses, Kelvin, Metres, MetresPerSecondSquared, Watts, Years};

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
    /// its escape where the escape's batch carries it (P14.T53.a, P14.T54.b.h).
    pub(crate) primordial_helium_per_hydrogen: f64,
    /// Whether a star's ultraviolet lights it, which drives its photochemistry.
    pub(crate) starlit: bool,
}

/// Why a record's envelope is not modelled under the air's batch, or the testing helper's
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
            Self::NoAlphaFe | Self::NoDisc => None,
        }
    }
}

/// The record's envelope section of the body `of` describes, its network `network`, under the
/// rules `rules` (P14.T54.b.i).
///
/// Under [`AirRules::Held`] it is not modelled, and neither closure runs: nothing is read or
/// computed. Under [`AirRules::Batch`] it is [`envelope_of`]'s envelope, and not modelled for
/// every refusal ([`DeriveEnvelopeError`]), whose reason the record does not keep.
#[must_use]
pub(super) fn record_envelope<'n>(
    rules: AirRules,
    of: impl FnOnce() -> EnvelopeOf,
    network: impl FnOnce() -> Result<&'n Network, BuildNetworkError>,
) -> Section<Envelope> {
    match rules {
        AirRules::Held => Section::NotModelled,
        AirRules::Batch => {
            let of = of();
            network()
                .map_err(DeriveEnvelopeError::Network)
                .and_then(|network| envelope_of(&of, network))
                .map_or(Section::NotModelled, Section::Ok)
        }
    }
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
    let abundances = EnvelopeAbundances::derive(&EnvelopeInputs {
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
    })
    .map_err(DeriveEnvelopeError::Abundances)?;
    let atmosphere = EnvelopeAtmosphere::derive(&EnvelopeAtmosphereInputs {
        abundances: &abundances,
        network,
        internal_temperature,
        equilibrium_temperature: of.equilibrium_temperature,
        gravity: of.gravity,
        fe_h: of.fe_h,
        grey: GreyProfile::ADOPTED,
        starlit: of.starlit,
    })
    .map_err(DeriveEnvelopeError::Atmosphere)?;
    Ok(Envelope::hydrogen_helium(abundances, atmosphere))
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
mod tests;
