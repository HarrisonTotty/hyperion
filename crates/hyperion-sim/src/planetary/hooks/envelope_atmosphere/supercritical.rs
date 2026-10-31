//! A solid body's supercritical envelope (plan 14, P14.T53.f; decision-p14-t53b-part3 §7): the
//! visible atmosphere of a condensable's fluid above its critical point over its ground, a steam
//! envelope's and any other condensable's, on the body's published air, and the inflation of a
//! water envelope's transit radius (Turbet et al. 2019, A&A 628, A12; 2020, A&A 638, A41).
//!
//! It reuses P14.T24.d's and P14.T52.d's chain where an H₂/He envelope's pieces do not read its
//! hydrogen (the lane's leans, 2026-10-31):
//!
//! - **The profile** is Guillot's grey profile ([`EnvelopeProfile`]) at [`GreyProfile::ADOPTED`]'s
//!   n, γ and f on the air's own opacity: its thermal opacity at 1 bar κ₁ is the one at which its
//!   optical depth κ₁ p₁ (p ÷ p₁)ⁿ ÷ (n g) at the reference level equals the published air's grey
//!   depth above that level ([`SupercriticalAtmosphereInputs::reference`]), the thick-air closure's
//!   top 100 bar (`GreenhouseConstants::thick_air`, P14.T53.a) or the whole air where it is thinner.
//!   Below its join it is the dry adiabat of the published air's whole mixture (a steam envelope's
//!   water with what else it holds) on the registry's `c_p`(T), to 100 bar or one segment past the
//!   join, as every envelope's is. n = 2 is "pressure-broadened opacity" (Robinson and Catling 2012,
//!   ApJ 757, 104, Eq. 6), the H₂/He envelopes' own.
//! - **Its internal temperature** is the one at which the profile meets the published surface: its
//!   temperature at the surface pressure ([`EnvelopeProfile::temperature_at`], below its last node
//!   the last segment's power law, as the wire states it) is the published mean surface
//!   temperature, by [`INTERNAL_TEMPERATURE_BISECTIONS`] bisections in ln `T_int` between
//!   [`INTERNAL_TEMPERATURE_BRACKET`]'s shares of it. σ`T_int`⁴ is then the net flux the published
//!   surface implies through the deep fluid, the starlight its ground absorbs and the interior's heat
//!   together, so the profile and the published solve state one column. A body whose surface no
//!   such profile meets, or whose join lies at or below its surface, is refused
//!   ([`DeriveSupercriticalAtmosphereError`]).
//! - **The decks** are P14.T52.c's loop over that profile on the published air, in P14.T24.c's
//!   column ([`CLOUD_EDDY_DIFFUSION`]) from the surface, or [`DECK_COLUMN_BOTTOM`] where the surface
//!   lies deeper, up to [`DECK_COLUMN_TOP`]. The bulk condensable itself is labelled, never trapped
//!   (`NotFormed::BulkGas`), and a vapour past its critical point is labelled supercritical
//!   (P14.T52.f), so no deck is based at or above its condensable's critical point.
//! - **The composition** is the published air's mole fractions, at 1 bar and at 10 mbar alike, with
//!   no quench network (P14.T52.a speciates hydrogen-dominated gas alone), trimmed by the decks as an
//!   H₂/He envelope's is. No haze is modelled: the four rules are H₂/He envelopes'.
//!
//! # The transit radius ([`steam_transit_inflation`])
//!
//! Turbet et al. 2020 fit the thickness of a pure steam atmosphere above its ground, to the top of
//! its moist convective layer, the transit radius, from their 1-D radiative–convective model
//! (their Eqs. 2–3 and Table 2):
//!
//! ```text
//! z = (1 ÷ (ln(P_surf ÷ P_transit) H) − 1 ÷ R)⁻¹,   H = R_gas T_eff ÷ (M_H₂O g)
//! log₁₀ T_eff = β₁ + β₂ x + β₃ y + β₄ s + β₅ x y + β₆ y² + β₇ x³ + β₈ x² y + β₉ x y² + β₁₀ y⁴
//! ```
//!
//! with x = (log₁₀ `x_H₂O` − α₁) ÷ α₂, y = (log₁₀ g − α₃) ÷ α₄ and s = (log₁₀ `S_eff` − α₅) ÷ α₆,
//! `P_surf` the water's column weight, the whole fluid's, and `x_H₂O` the fit's water fraction from
//! it, `x` ÷ (1 − `x`) = 4π G `P_surf` ÷ g² (their Eqs. C.9–C.10, which the fit's runs read for
//! `P_surf` at a given g). It is published in the envelope section alone: the bulk section's radius
//! stays the bulk's, so that nothing that reads the radius moves. R and g are the bulk section's
//! (the lean): Turbet's core holds no water and the bulk's holds the body's, so with the bulk's g
//! 4π G `P_surf` ÷ g² is the water's share of the bulk itself, and `x` ÷ (1 + `x`) of it is taken.
//!
//! The fit holds "within a few percent for most of the parameter space (again, maximum error of ∼
//! 10%)" over its grid: irradiation from 1 to 30 S⊕, surface gravity from 0.2 to 6 g⊕, water
//! vapour pressure from 2.7 bar to 27 kbar, and a surface temperature between 300 and 4,300 K
//! (their §2.2). Outside the grid it is not modelled, never extrapolated. Its runs are about the
//! Sun ("the Sun (assumed to be the host star) is assumed to remain fixed, at a zenith angle of
//! 60°", Turbet et al. 2019, App. A), so about another host its `S_eff` is the flux alone and the
//! steam's spectral response is extrapolated, recorded.
//!
//! # Not on the record before 26
//!
//! The record's envelope section calls it from generator version 26 (`system::envelope`'s hold,
//! `SUPERCRITICAL_ENVELOPE_VERSION`); before it a solid body's under a supercritical envelope is
//! not modelled.
//!
//! # Determinism
//!
//! Every count is fixed (the bisections', and the profile's and the loop's own), every sum is in
//! the gas's given order and every transcendental is [`math`]'s.
//!
//! [`DECK_COLUMN_BOTTOM`]: super::DECK_COLUMN_BOTTOM
//! [`DECK_COLUMN_TOP`]: super::DECK_COLUMN_TOP

use core::cmp::Ordering;
use std::error::Error;
use std::fmt;

use super::{
    COMPOSITION_FLOOR, COMPOSITION_LEVEL, DECK_COLUMN_BOTTOM, DECK_COLUMN_TOP,
    DeriveEnvelopeAtmosphereError, EnvelopeAtmosphere, condense, molar_mass_g_per_mol, normalise,
};
use crate::math;
use crate::planetary::chem::quench::MixingColumn;
use crate::planetary::hooks::aerosols::CLOUD_EDDY_DIFFUSION;
use crate::planetary::hooks::structure::envelope::{
    BuildGreyProfileError, DeriveEnvelopeProfileError, EnvelopeProfile, EnvelopeProfileInputs,
    GreyProfile, REFERENCE_PRESSURE, irradiation_temperature,
};
use crate::planetary::hooks::surface::STANDARD_GRAVITY;
use crate::substance::{MOLAR_GAS_CONSTANT, SubstanceId};
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{Kelvin, Metres, MetresPerSecondSquared, Pascals, WattsPerSquareMetre};

#[cfg(test)]
pub(crate) mod tests;

/// The bisections in ln `T_int` that meet the published surface (the [module](self)'s
/// documentation): 48, to 8.3 × 2⁻⁴⁸ ≈ 3 × 10⁻¹⁴ in ln `T_int` across [`INTERNAL_TEMPERATURE_BRACKET`].
pub const INTERNAL_TEMPERATURE_BISECTIONS: u32 = 48;

/// The bracket of the internal temperature's bisection, as shares of the published surface
/// temperature: 10⁻³ to 4. At its cold end the profile's deep air is its irradiated isotherm, about
/// 1.12 `T_eq` at γ = 0.4, or a radiative column barely warmer; at its warm end a column of grey
/// depth above about 1 lies hotter than the surface at every depth.
pub const INTERNAL_TEMPERATURE_BRACKET: (f64, f64) = (1e-3, 4.0);

/// Turbet et al. 2020's α₁–α₆ (their Table 2): the centres and scales of log₁₀ `x_H₂O`, log₁₀ g (g
/// in m s⁻²) and log₁₀ `S_eff` (`S_eff` in Earth insolations) in their Eq. 3.
pub const STEAM_FIT_SCALES: [f64; 6] = [-3.550, 1.310, 1.099, 4.683e-1, 7.664e-1, 4.224e-1];

/// Turbet et al. 2020's β₁–β₁₀ (their Table 2), the coefficients of log₁₀ `T_eff` in their Eq. 3,
/// in its order: 1, x, y, s, x y, y², x³, x² y, x y² and y⁴.
pub const STEAM_FIT_COEFFICIENTS: [f64; 10] = [
    2.846, 1.555e-1, 8.777e-2, 6.045e-2, 1.143e-2, 1.736e-2, 1.859e-2, 4.314e-2, 3.393e-2,
    -1.034e-2,
];

/// The pressure at a steam atmosphere's transit radius, `P_transit`: 0.1 Pa, which Turbet et al.
/// 2020 find "varies little across the range of parameters we explored and is roughly equal to
/// 10⁻¹ Pa. We thus set it to this value" (their §2.2).
pub const STEAM_TRANSIT_PRESSURE: Pascals = Pascals::new(0.1);

/// Water's molar mass in Turbet et al. 2020's Eq. 2, kg mol⁻¹: 1.8 × 10⁻², the value their fitted
/// `T_eff` is fitted through.
pub const STEAM_FIT_MOLAR_MASS_KG_PER_MOL: f64 = 1.8e-2;

/// Turbet et al. 2020's Earth insolation, W m⁻²: 1,366, the unit of their `S_eff` ("the same
/// insolation as Earth of 1366 W m2", their §2.1).
pub const STEAM_FIT_EARTH_INSOLATION: WattsPerSquareMetre = WattsPerSquareMetre::new(1_366.0);

/// The irradiation of Turbet et al. 2020's grid, in Earth insolations
/// ([`STEAM_FIT_EARTH_INSOLATION`]): 1 to 30 (their §2.2).
pub const STEAM_FIT_INSOLATION_EARTHS: (f64, f64) = (1.0, 30.0);

/// The surface gravity of Turbet et al. 2020's grid, in Earth gravities against
/// [`STANDARD_GRAVITY`]: 0.2 to 6, their §2.2's stated validity (their runs, §2.1, spanned 2 to 50
/// m s⁻²).
pub const STEAM_FIT_GRAVITY_EARTH_G: (f64, f64) = (0.2, 6.0);

/// The water vapour pressure of Turbet et al. 2020's grid, Pa: 2.7 bar to 27 kbar (their §2.2).
pub const STEAM_FIT_PRESSURE: (Pascals, Pascals) = (Pascals::new(2.7e5), Pascals::new(2.7e9));

/// The surface temperature of Turbet et al. 2020's grid, K: 300 to 4,300 (their §2.2).
pub const STEAM_FIT_SURFACE_TEMPERATURE: (Kelvin, Kelvin) =
    (Kelvin::new(300.0), Kelvin::new(4_300.0));

/// What [`EnvelopeAtmosphere::supercritical`] reads of a body's published air, as plain values. The
/// derivation checks each, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupercriticalAtmosphereInputs<'a> {
    /// The published air's gases by mole fraction (its partial pressures', the airborne fluid's),
    /// each a registry gas row, summing to 1.
    pub gases: &'a [(SubstanceId, f64)],
    /// The published surface pressure, the top of the liquid-like fluid, above 1 bar.
    pub surface_pressure: Pascals,
    /// The published mean surface temperature, finite and positive.
    pub surface_temperature: Kelvin,
    /// The level at which the grey profile takes the air's own opacity, at or above the surface,
    /// and the published air's grey optical depth above it, finite and positive.
    pub reference: (Pascals, f64),
    /// The equilibrium temperature `T_eq`, finite and not negative: the bulk section's.
    pub equilibrium_temperature: Kelvin,
    /// The bulk section's surface gravity, finite and positive.
    pub gravity: MetresPerSecondSquared,
}

/// Why [`EnvelopeAtmosphere::supercritical`] gives no atmosphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveSupercriticalAtmosphereError {
    /// The surface pressure is not finite or not above 1 bar.
    SurfacePressure(Pascals),
    /// The surface temperature is not finite and positive.
    SurfaceTemperature(Kelvin),
    /// The reference level is not finite, positive and at or above the surface, or its depth is
    /// not finite and positive.
    Reference(Pascals, f64),
    /// The equilibrium temperature is not finite and not negative.
    EquilibriumTemperature(Kelvin),
    /// The gravity is not finite and positive.
    Gravity(MetresPerSecondSquared),
    /// The air's opacity gives no grey profile.
    Grey(BuildGreyProfileError),
    /// No internal temperature in the bracket meets the published surface.
    NoProfileMeetsTheSurface,
    /// The profile's join lies at or below the surface, so no adiabat lies above it.
    JoinBelowTheSurface {
        /// The join's pressure.
        join: Pascals,
    },
    /// The profile refused its inputs.
    Profile(DeriveEnvelopeProfileError),
    /// A step of the H₂/He envelope's chain it reuses refused: the gas's mixture, the
    /// condensation loop or a deck's mode.
    Atmosphere(DeriveEnvelopeAtmosphereError),
}

impl fmt::Display for DeriveSupercriticalAtmosphereError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SurfacePressure(p) => {
                write!(f, "surface pressure {} Pa is not above 1 bar", p.value())
            }
            Self::SurfaceTemperature(t) => write!(
                f,
                "surface temperature {} K is not finite and positive",
                t.value()
            ),
            Self::Reference(p, tau) => write!(
                f,
                "the reference level {} Pa with depth {tau} is not a level of the air",
                p.value()
            ),
            Self::EquilibriumTemperature(t) => write!(
                f,
                "equilibrium temperature {} K is not finite and non-negative",
                t.value()
            ),
            Self::Gravity(g) => write!(f, "gravity {} m/s2 is not finite and positive", g.value()),
            Self::Grey(error) => write!(f, "the grey profile: {error}"),
            Self::NoProfileMeetsTheSurface => {
                write!(f, "no internal temperature meets the published surface")
            }
            Self::JoinBelowTheSurface { join } => write!(
                f,
                "the profile joins its adiabat at {} Pa, at or below the surface",
                join.value()
            ),
            Self::Profile(error) => write!(f, "the profile: {error}"),
            Self::Atmosphere(error) => write!(f, "the atmosphere: {error}"),
        }
    }
}

impl Error for DeriveSupercriticalAtmosphereError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Grey(error) => Some(error),
            Self::Profile(error) => Some(error),
            Self::Atmosphere(error) => Some(error),
            Self::SurfacePressure(_)
            | Self::SurfaceTemperature(_)
            | Self::Reference(..)
            | Self::EquilibriumTemperature(_)
            | Self::Gravity(_)
            | Self::NoProfileMeetsTheSurface
            | Self::JoinBelowTheSurface { .. } => None,
        }
    }
}

impl From<DeriveEnvelopeProfileError> for DeriveSupercriticalAtmosphereError {
    fn from(error: DeriveEnvelopeProfileError) -> Self {
        Self::Profile(error)
    }
}

impl From<DeriveEnvelopeAtmosphereError> for DeriveSupercriticalAtmosphereError {
    fn from(error: DeriveEnvelopeAtmosphereError) -> Self {
        Self::Atmosphere(error)
    }
}

impl EnvelopeAtmosphere {
    /// The visible atmosphere of a solid body's supercritical envelope, of the published air
    /// `inputs` describes (P14.T53.f; the [module](self)'s documentation): its profile on its own
    /// opacity, meeting its surface, its decks, its composition and its absorbers, with no haze.
    ///
    /// # Errors
    ///
    /// - [`DeriveSupercriticalAtmosphereError::SurfacePressure`],
    ///   [`SurfaceTemperature`](DeriveSupercriticalAtmosphereError::SurfaceTemperature),
    ///   [`Reference`](DeriveSupercriticalAtmosphereError::Reference),
    ///   [`EquilibriumTemperature`](DeriveSupercriticalAtmosphereError::EquilibriumTemperature) and
    ///   [`Gravity`](DeriveSupercriticalAtmosphereError::Gravity) for an input out of its range;
    /// - [`Grey`](DeriveSupercriticalAtmosphereError::Grey) where the air's opacity is not a grey
    ///   profile's;
    /// - [`NoProfileMeetsTheSurface`](DeriveSupercriticalAtmosphereError::NoProfileMeetsTheSurface)
    ///   and [`JoinBelowTheSurface`](DeriveSupercriticalAtmosphereError::JoinBelowTheSurface) for a
    ///   published surface the profile cannot state;
    /// - the reused steps' refusals ([`Profile`](DeriveSupercriticalAtmosphereError::Profile) and
    ///   [`Atmosphere`](DeriveSupercriticalAtmosphereError::Atmosphere)), kept as errors rather than
    ///   panics.
    ///
    /// # Examples
    ///
    /// A steam envelope's air, nearly pure water at 2.5 kbar and 1,154 K above 300 K of
    /// equilibrium, whose top 100 bar have a grey depth of 40: its profile meets its surface, and
    /// its water is the bulk gas, labelled rather than trapped.
    ///
    /// ```
    /// use hyperion_sim::planetary::hooks::envelope_atmosphere::EnvelopeAtmosphere;
    /// use hyperion_sim::planetary::hooks::envelope_atmosphere::supercritical::SupercriticalAtmosphereInputs;
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let gases = [(by_key("H2O").ok_or("a row")?, 0.99), (by_key("CO2").ok_or("a row")?, 0.01)];
    /// let surface = Pascals::new(2.475e8);
    /// let steam = EnvelopeAtmosphere::supercritical(&SupercriticalAtmosphereInputs {
    ///     gases: &gases,
    ///     surface_pressure: surface,
    ///     surface_temperature: Kelvin::new(1_154.0),
    ///     reference: (Pascals::new(1.0e7), 40.0),
    ///     equilibrium_temperature: Kelvin::new(300.0),
    ///     gravity: MetresPerSecondSquared::new(9.82),
    /// })?;
    /// let at_surface = steam.profile().temperature_at(surface).value();
    /// assert!((at_surface - 1_154.0).abs() < 1e-6);
    /// assert!(steam.profile().join_pressure().value() < surface.value());
    /// # Ok(())
    /// # }
    /// ```
    pub fn supercritical(
        inputs: &SupercriticalAtmosphereInputs<'_>,
    ) -> Result<Self, DeriveSupercriticalAtmosphereError> {
        let equilibrium = check(inputs)?;
        let mut deep: Vec<(SubstanceId, f64)> = inputs
            .gases
            .iter()
            .copied()
            .filter(|&(id, x)| id.substance().gas().is_some() && x >= COMPOSITION_FLOOR)
            .collect();
        normalise(&mut deep)?;
        let grey = grey_of(inputs)?;
        let profile = profile_meeting_the_surface(inputs, &deep, grey, equilibrium)?;
        let join = profile.join_pressure();
        // By comparison, so that a join that is not a number is refused with a deep one.
        if join.value().partial_cmp(&inputs.surface_pressure.value()) != Some(Ordering::Less) {
            return Err(DeriveSupercriticalAtmosphereError::JoinBelowTheSurface { join });
        }
        let at = |p: Pascals| profile.temperature_at(p);
        let column = MixingColumn {
            gravity: inputs.gravity,
            molar_mass_g_per_mol: molar_mass_g_per_mol(&deep)?,
            eddy_diffusion: CLOUD_EDDY_DIFFUSION,
            // The laws aloft read a metallicity for the metals' co-reactants alone, which a
            // supercritical envelope's air holds none of: solar, stated.
            metallicity: 1.0,
            top: DECK_COLUMN_TOP,
            bottom: Pascals::new(
                inputs
                    .surface_pressure
                    .value()
                    .min(DECK_COLUMN_BOTTOM.value()),
            ),
        };
        let clouds = condense(&deep, at, &column)?;
        Ok(Self::assembled(
            profile,
            &deep,
            &deep,
            &clouds,
            inputs.gravity,
            |_, _, _, _| Ok((Vec::new(), None)),
        )?)
    }
}

/// The inputs' checks (their doc comments' ranges), giving the equilibrium temperature, a −0 K one
/// as +0.
fn check(
    inputs: &SupercriticalAtmosphereInputs<'_>,
) -> Result<Kelvin, DeriveSupercriticalAtmosphereError> {
    let positive = |x: f64| x.is_finite() && x > 0.0;
    let p_s = inputs.surface_pressure;
    if !(p_s.value().is_finite() && p_s.value() > COMPOSITION_LEVEL.value()) {
        return Err(DeriveSupercriticalAtmosphereError::SurfacePressure(p_s));
    }
    if !positive(inputs.surface_temperature.value()) {
        return Err(DeriveSupercriticalAtmosphereError::SurfaceTemperature(
            inputs.surface_temperature,
        ));
    }
    let (level, depth) = inputs.reference;
    if !(positive(level.value()) && level.value() <= p_s.value() && positive(depth)) {
        return Err(DeriveSupercriticalAtmosphereError::Reference(level, depth));
    }
    let t_eq = inputs.equilibrium_temperature.value();
    if !(t_eq.is_finite() && t_eq >= 0.0) {
        return Err(DeriveSupercriticalAtmosphereError::EquilibriumTemperature(
            inputs.equilibrium_temperature,
        ));
    }
    if !positive(inputs.gravity.value()) {
        return Err(DeriveSupercriticalAtmosphereError::Gravity(inputs.gravity));
    }
    // A signed zero's sign would reach the profile's T_irr; +0 is the one value.
    Ok(Kelvin::new(if t_eq > 0.0 { t_eq } else { 0.0 }))
}

/// [`GreyProfile::ADOPTED`]'s n, γ and f on the air's own opacity: κ₁ = n g τ ÷ (p₁ (p ÷ p₁)ⁿ) at
/// the reference level p and its depth τ, so that the profile's optical depth there is the air's.
fn grey_of(
    inputs: &SupercriticalAtmosphereInputs<'_>,
) -> Result<GreyProfile, DeriveSupercriticalAtmosphereError> {
    let adopted = GreyProfile::ADOPTED;
    let n = adopted.optical_depth_exponent();
    let (level, depth) = inputs.reference;
    let p_ref = REFERENCE_PRESSURE.value();
    let opacity =
        n * inputs.gravity.value() * depth / (p_ref * math::powf(level.value() / p_ref, n));
    GreyProfile::new(
        opacity,
        n,
        adopted.opacity_ratio(),
        adopted.redistribution(),
    )
    .map_err(DeriveSupercriticalAtmosphereError::Grey)
}

/// The profile on the mixture `gases` and the grey profile `grey` whose temperature at the surface
/// is the published surface's (the [module](self)'s documentation): its internal temperature by
/// [`INTERNAL_TEMPERATURE_BISECTIONS`] bisections in ln `T_int` across
/// [`INTERNAL_TEMPERATURE_BRACKET`], the bracket's midpoint at the last.
fn profile_meeting_the_surface(
    inputs: &SupercriticalAtmosphereInputs<'_>,
    gases: &[(SubstanceId, f64)],
    grey: GreyProfile,
    equilibrium: Kelvin,
) -> Result<EnvelopeProfile, DeriveSupercriticalAtmosphereError> {
    let irradiation = irradiation_temperature(equilibrium);
    let profile = |ln_internal: f64| {
        EnvelopeProfile::derive(&EnvelopeProfileInputs {
            internal_temperature: Kelvin::new(math::exp(ln_internal)),
            irradiation_temperature: irradiation,
            gravity: inputs.gravity,
            gases,
            grey,
        })
    };
    let target = math::ln(inputs.surface_temperature.value());
    let gap = |ln_internal: f64| -> Result<f64, DeriveEnvelopeProfileError> {
        let at_surface = profile(ln_internal)?.temperature_at(inputs.surface_pressure);
        Ok(math::ln(at_surface.value()) - target)
    };
    let (cold, warm) = INTERNAL_TEMPERATURE_BRACKET;
    let (mut below, mut above) = (target + math::ln(cold), target + math::ln(warm));
    // By comparison, so that a gap that is not a number has no bracket.
    if !(gap(below)? < 0.0 && gap(above)? > 0.0) {
        return Err(DeriveSupercriticalAtmosphereError::NoProfileMeetsTheSurface);
    }
    for _ in 0..INTERNAL_TEMPERATURE_BISECTIONS {
        let middle = f64::midpoint(below, above);
        if gap(middle)? > 0.0 {
            above = middle;
        } else {
            below = middle;
        }
    }
    Ok(profile(f64::midpoint(below, above))?)
}

/// What [`steam_transit_inflation`] reads of a body under a water envelope, as plain values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteamInflationInputs {
    /// The water's column weight, its whole fluid's (the air's water and its liquid-like reservoir
    /// beneath), M g ÷ 4πR²: Turbet et al.'s `P_surf`.
    pub water_column: Pascals,
    /// The bulk section's radius.
    pub radius: Metres,
    /// The bulk section's surface gravity.
    pub gravity: MetresPerSecondSquared,
    /// The flux the body receives from its hosts, averaged over its orbit.
    pub stellar_flux: WattsPerSquareMetre,
    /// The published mean surface temperature.
    pub surface_temperature: Kelvin,
}

/// The thickness of a water envelope above its ground to its transit radius, by Turbet et al.
/// 2020's fit (the [module](self)'s documentation, their Eqs. 2–3 and Table 2), at the inputs
/// `inputs` give.
///
/// `None` outside the fit's grid (irradiation from 1 to 30 S⊕, gravity from 0.2 to 6 g⊕, water
/// pressure from 2.7 bar to 27 kbar and surface temperature from 300 to 4,300 K, each inclusive), and
/// where the atmosphere is unbound, ln(`P_surf` ÷ `P_transit`) H at or beyond the radius (their
/// "gravitationally unbounded" small planets, §3.1), or a value is not finite; otherwise finite and
/// positive, above the radius.
///
/// # Examples
///
/// An Earth whose ocean is all steam just past the runaway limit stands some 500 km taller in
/// transit (Turbet et al. 2019, Fig. 2), and the fit is not stated for a body below its grid's
/// least irradiation:
///
/// ```
/// use hyperion_sim::planetary::hooks::envelope_atmosphere::supercritical::{
///     SteamInflationInputs, steam_transit_inflation,
/// };
/// use hyperion_sim::units::{Kelvin, Metres, MetresPerSecondSquared, Pascals, WattsPerSquareMetre};
///
/// let earth = SteamInflationInputs {
///     water_column: Pascals::new(2.7e7),
///     radius: Metres::new(6.371e6),
///     gravity: MetresPerSecondSquared::new(9.81),
///     stellar_flux: WattsPerSquareMetre::new(1.07 * 1_366.0),
///     surface_temperature: Kelvin::new(1_700.0),
/// };
/// let z = steam_transit_inflation(&earth).expect("inside the grid");
/// assert!((450e3..560e3).contains(&z.value()));
/// let dim = SteamInflationInputs { stellar_flux: WattsPerSquareMetre::new(0.5 * 1_366.0), ..earth };
/// assert_eq!(steam_transit_inflation(&dim), None);
/// ```
#[must_use]
pub fn steam_transit_inflation(inputs: &SteamInflationInputs) -> Option<Metres> {
    let within = |value: f64, (low, high): (f64, f64)| (low..=high).contains(&value);
    let g = inputs.gravity.value();
    let flux = inputs.stellar_flux.value() / STEAM_FIT_EARTH_INSOLATION.value();
    let p_surf = inputs.water_column.value();
    let t_s = inputs.surface_temperature.value();
    let radius = inputs.radius.value();
    let (p_low, p_high) = STEAM_FIT_PRESSURE;
    let (t_low, t_high) = STEAM_FIT_SURFACE_TEMPERATURE;
    if !(within(flux, STEAM_FIT_INSOLATION_EARTHS)
        && within(g / STANDARD_GRAVITY.value(), STEAM_FIT_GRAVITY_EARTH_G)
        && within(p_surf, (p_low.value(), p_high.value()))
        && within(t_s, (t_low.value(), t_high.value()))
        && radius.is_finite()
        && radius > 0.0)
    {
        return None;
    }
    // x ÷ (1 − x) = 4π G P_surf ÷ g² (Turbet et al.'s Eqs. C.9–C.10).
    let ratio = 4.0 * core::f64::consts::PI * GRAVITATIONAL_CONSTANT * p_surf / (g * g);
    let water_fraction = ratio / (1.0 + ratio);
    let effective = steam_effective_temperature(water_fraction, inputs.gravity, flux).value();
    let scale_height = MOLAR_GAS_CONSTANT * effective / (STEAM_FIT_MOLAR_MASS_KG_PER_MOL * g);
    let span = math::ln(p_surf / STEAM_TRANSIT_PRESSURE.value()) * scale_height;
    let thickness = 1.0 / (1.0 / span - 1.0 / radius);
    (thickness.is_finite() && thickness > 0.0).then(|| Metres::new(thickness))
}

/// Turbet et al. 2020's effective temperature of a steam atmosphere (their Eq. 3 and Table 2), for
/// the fit's water fraction `water_fraction`, dimensionless in (0, 1), the surface gravity
/// `gravity` and the irradiation `insolation_earths` in Earth insolations
/// ([`STEAM_FIT_EARTH_INSOLATION`]).
#[must_use]
pub fn steam_effective_temperature(
    water_fraction: f64,
    gravity: MetresPerSecondSquared,
    insolation_earths: f64,
) -> Kelvin {
    let [a1, a2, a3, a4, a5, a6] = STEAM_FIT_SCALES;
    let [b1, b2, b3, b4, b5, b6, b7, b8, b9, b10] = STEAM_FIT_COEFFICIENTS;
    let x = (math::log10(water_fraction) - a1) / a2;
    let y = (math::log10(gravity.value()) - a3) / a4;
    let s = (math::log10(insolation_earths) - a5) / a6;
    let log_t = b1
        + b2 * x
        + b3 * y
        + b4 * s
        + b5 * x * y
        + b6 * math::powi(y, 2)
        + b7 * math::powi(x, 3)
        + b8 * math::powi(x, 2) * y
        + b9 * x * math::powi(y, 2)
        + b10 * math::powi(y, 4);
    Kelvin::new(math::exp10(log_t))
}
