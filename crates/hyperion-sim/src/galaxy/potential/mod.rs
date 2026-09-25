//! The galaxy's mass model and its potential tables (plan 02, P02.T6).
//!
//! The brainstorm builds the potential from sums of Gaussians whose dimensionless coefficients
//! are fitted offline, so that each Gaussian's potential and forces are one-dimensional
//! quadratures ("Galaxy parameters"). [`MassModel`] assembles the galaxy from such expansions
//! ([`mge`]) and from spherical components in closed form ([`nfw`], [`spherical`]), and
//! evaluates the potential, the circular speed and the vertical force directly.
//! [`PotentialTables`] reduces it once per galaxy to tables in the plane, and on request on an
//! (R, z) grid, from which the circular speed, Ω, κ, the escape speed, the tidal radius and the
//! bar's pattern speed are read. [`sigma`] estimates the bulge's velocity dispersion, which sets
//! the central black hole's mass through the M–σ relation.
//!
//! Working units (plan 02, Design note 1): light-years, solar masses, (km/s)² for potentials,
//! (km/s)² per light-year for forces.

pub mod mge;
mod model;
pub mod nfw;
pub mod sigma;
pub mod spherical;
mod tables;

use std::error::Error;
use std::fmt;

pub use model::MassModel;
pub use tables::PotentialTables;

/// A mass component could not be built: `quantity` is `value`, which is not finite or lies
/// outside its range (a mass below zero, a length, width, ratio or slope margin not above zero).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildComponentError {
    /// What was wrong, such as `mass` or `width`.
    pub quantity: &'static str,
    /// The value given.
    pub value: f64,
}

impl BuildComponentError {
    /// `Ok` if `value` is finite.
    pub(crate) fn check_finite(quantity: &'static str, value: f64) -> Result<(), Self> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(Self { quantity, value })
        }
    }

    /// `Ok` if `value` is finite and not negative.
    pub(crate) fn check_non_negative(quantity: &'static str, value: f64) -> Result<(), Self> {
        if value.is_finite() && value >= 0.0 {
            Ok(())
        } else {
            Err(Self { quantity, value })
        }
    }

    /// `Ok` if `value` is finite and above zero.
    pub(crate) fn check_positive(quantity: &'static str, value: f64) -> Result<(), Self> {
        if value.is_finite() && value > 0.0 {
            Ok(())
        } else {
            Err(Self { quantity, value })
        }
    }
}

impl fmt::Display for BuildComponentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a mass component's {} is {}, outside its range",
            self.quantity, self.value
        )
    }
}

impl Error for BuildComponentError {}
