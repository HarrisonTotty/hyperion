//! A gas envelope's visible atmosphere (plan 14, P14.T24.d with P14.T52.d): for every body in
//! [`SurfaceState::GasEnvelope`](crate::planetary::derive::atmosphere::SurfaceState::GasEnvelope),
//! the giants and every sub-Neptune, its internal temperature and its profile
//! ([`EnvelopeProfile`](crate::planetary::hooks::structure::envelope::EnvelopeProfile), Guillot's
//! grey profile joined to its adiabat, P14.T24.d).
//!
//! # The internal temperature ([`envelope_internal_temperature`])
//!
//! The profile needs an internal heat: with `T_int` = 0 its gradient falls to 0 at depth and no
//! envelope joins its adiabat, an isothermal column to 100 bar (decision-p14-t24d-profile §3.1).
//! The record gives a giant's internal luminosity from 0.3 Jupiter masses, faded in to full at
//! 0.414 by [`giant_share`] (ruling 112.7; P14.T11.d), and none below, so the generated Saturn,
//! every ice giant and every sub-Neptune would have none. This module states one, a cooling law
//! blended with the record's by [`giant_share`], the radius blend's weight
//! (decision-p14-t24d-profile §3.2–§3.3):
//!
//! ```text
//! T_int⁴ = L_int ÷ (4π R² σ) + (1 − s) T_law⁴
//! T_law  = 84.13 K × (M ÷ 95.161 M⊕)^0.2744 × (max(t, 1 Myr) ÷ 4.57 Gyr)^−0.32
//! ```
//!
//! with `L_int` the record's internal luminosity (already times s) and s = [`giant_share`] of M: the
//! law alone below 0.3 Jupiter masses, the record's alone from 0.414, continuous in mass and time.
//! A blend rather than the larger of the two, so that the law never overrides the record's giant
//! cooling where that applies: with modern anchors it would put Jupiter at 117.1 K against the
//! record's 104.8 K.
//!
//! - **The anchor** ([`COOLING_LAW_TEMPERATURE`] at [`COOLING_LAW_MASS`]) is Saturn's measured
//!   internal heat, 2.84 ± 0.20 W m⁻² (Wang et al. 2024, Nat. Commun. 15, 5045, Cassini), (F ÷
//!   σ)^¼ = 84.13 K, at the Solar System table's Saturn, 568.32 × 10²⁴ kg.
//! - **The mass index** ([`COOLING_LAW_MASS_INDEX`]) is ln(84.13 ÷ 52.57) ÷ ln(95.161 ÷ 17.148),
//!   Neptune's 0.433 ± 0.046 W m⁻² (Pearl and Conrath 1991, JGR 96, 18921) at the table's mass;
//!   0.25–0.30 across both 1σ ranges.
//! - **The age index** ([`COOLING_LAW_AGE_INDEX`]) is Burrows et al. 2001's `T_eff` ∼ 1550 K (10⁹ yr
//!   ÷ t)^0.32 for older substellar objects (Rev. Mod. Phys. 73, 719, Eq. 2), its L ∝ t^−1.3 at
//!   nearly fixed radius; its mass index, the brown dwarfs', is not taken.
//! - **The reference age** is 4.57 Gyr ([`COOLING_LAW_AGE`]), the table's, and the floor
//!   [`MIN_AGE`], 1 Myr, plan 13's: a younger envelope has a 1 Myr one's heat.
//!
//! Checks, recorded: the law gives Jupiter 117.1 K against Li et al. 2018's 107.2 K (the record's
//! 104.8 K is the one used), and Uranus 50.2 K against Wang et al. 2025's 34.2 K (0.078 W m⁻²) or
//! Pearl et al. 1990's 29.3 K, the known outlier. It is fitted on 17–95 M⊕ and extrapolated to the
//! smallest sub-Neptunes, whose cores' heat is the physical floor.
//!
//! The law lives here, not in the record's derivation: the record's internal luminosity, the bulk
//! section's effective temperature and every reader of them (the rings, the moons' heating, the
//! photometry) are unchanged, so below 0.414 Jupiter masses the envelope's `T_int` is not the bulk
//! section's (decision-p14-t24d-profile §3.4). P14.T54.b passes P14.T50.b the envelope's `T_eff`,
//! (`T_eq`⁴ + `T_int`⁴)^¼, so that the profile and the helium read one temperature.
//!
//! # Not on the record yet
//!
//! This module is pure: nothing on the record calls it at generator version 23
//! (decision-bump-plan-after-22 §1.3). P14.T54.b fills the record's envelope section from it.

use crate::math;
use crate::planetary::derive::radius::giant_share;
use crate::stellar::substellar::MIN_AGE;
use crate::units::consts::STEFAN_BOLTZMANN;
use crate::units::{EarthMasses, Kelvin, Metres, Watts, Years};

#[cfg(test)]
mod tests;

/// The cooling law's temperature at its anchor, K: Saturn's internal heat, 2.84 ± 0.20 W m⁻² (Wang
/// et al. 2024, Nat. Commun. 15, 5045), (F ÷ σ)^¼ = 84.13 K (82.6–85.6 K at 1σ).
pub const COOLING_LAW_TEMPERATURE: Kelvin = Kelvin::new(84.13);

/// The cooling law's anchor mass: the Solar System table's Saturn, 568.32 × 10²⁴ kg, 95.161 M⊕.
pub const COOLING_LAW_MASS: EarthMasses = EarthMasses::new(95.161);

/// The cooling law's mass index b: 0.2744, ln(84.13 ÷ 52.57) ÷ ln(95.161 ÷ 17.148), with Neptune's
/// internal heat, 0.433 ± 0.046 W m⁻² (Pearl and Conrath 1991, JGR 96, 18921), 52.57 K at the
/// table's 17.148 M⊕; 0.25–0.30 across both anchors' 1σ ranges.
pub const COOLING_LAW_MASS_INDEX: f64 = 0.2744;

/// The cooling law's age index c: 0.32, Burrows et al. 2001's `T_eff` ∝ t^−0.32 for older
/// substellar objects (Rev. Mod. Phys. 73, 719, Eq. 2, after Burrows and Liebert 1993 and Marley
/// et al. 1996).
pub const COOLING_LAW_AGE_INDEX: f64 = 0.32;

/// The cooling law's reference age: 4.57 Gyr, the Solar System table's.
pub const COOLING_LAW_AGE: Years = Years::new(4.57e9);

/// The cooling law's internal temperature of an envelope of mass `mass` at age `age` (the
/// [module](self)'s documentation): 84.13 K (M ÷ 95.161 M⊕)^0.2744 (max(t, 1 Myr) ÷ 4.57 Gyr)^−0.32,
/// positive and finite for every positive mass and every age; NaN for a mass that is not finite
/// and positive.
///
/// # Examples
///
/// Saturn's internal heat at the table's mass and age is the anchor's:
///
/// ```
/// use hyperion_sim::planetary::hooks::envelope_atmosphere::{
///     COOLING_LAW_AGE, COOLING_LAW_MASS, cooling_law_temperature,
/// };
///
/// let saturn = cooling_law_temperature(COOLING_LAW_MASS, COOLING_LAW_AGE);
/// assert!((saturn.value() - 84.13).abs() < 1e-9);
/// ```
#[must_use]
pub fn cooling_law_temperature(mass: EarthMasses, age: Years) -> Kelvin {
    let m = mass.value();
    if !(m.is_finite() && m > 0.0) {
        return Kelvin::new(f64::NAN);
    }
    let t = age.value().max(MIN_AGE.value());
    Kelvin::new(
        COOLING_LAW_TEMPERATURE.value()
            * math::powf(m / COOLING_LAW_MASS.value(), COOLING_LAW_MASS_INDEX)
            * math::powf(t / COOLING_LAW_AGE.value(), -COOLING_LAW_AGE_INDEX),
    )
}

/// The internal temperature of a gas envelope of mass `mass` and radius `radius` (positive) at age
/// `age`,
/// whose record radiates `internal_luminosity` of its own (the record's, already times
/// [`giant_share`]): (`L_int` ÷ (4π R² σ) + (1 − s) `T_law`⁴)^¼ with s = [`giant_share`] of M and
/// `T_law` [`cooling_law_temperature`] (the [module](self)'s documentation). Positive for every
/// envelope; the record's alone from 0.414 Jupiter masses and the law's alone below 0.3.
///
/// # Examples
///
/// The generated Saturn, at 0.2994 Jupiter masses, has no internal luminosity in the record, and
/// takes the law's heat:
///
/// ```
/// use hyperion_sim::planetary::hooks::envelope_atmosphere::{
///     COOLING_LAW_AGE, COOLING_LAW_MASS, envelope_internal_temperature,
/// };
/// use hyperion_sim::units::{Metres, Watts};
///
/// let saturn = envelope_internal_temperature(
///     Watts::new(0.0),
///     Metres::new(5.8232e7),
///     COOLING_LAW_MASS,
///     COOLING_LAW_AGE,
/// );
/// assert!((saturn.value() - 84.13).abs() < 1e-9);
/// ```
#[must_use]
pub fn envelope_internal_temperature(
    internal_luminosity: Watts,
    radius: Metres,
    mass: EarthMasses,
    age: Years,
) -> Kelvin {
    let r = radius.value();
    let record = internal_luminosity.value().max(0.0)
        / (4.0 * core::f64::consts::PI * r * r * STEFAN_BOLTZMANN);
    let law = math::powi(cooling_law_temperature(mass, age).value(), 4);
    Kelvin::new(math::powf(record + (1.0 - giant_share(mass)) * law, 0.25))
}
