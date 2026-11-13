//! An airless planet over a hot ice bulk takes P14.T56.g's rock lag (research-census-25 §3), from
//! generator version 26.
//!
//! decision-p14-t56g §4 gave planets no lag, since under an air a planet's water is a sea, a cover
//! or vapour, and left "an `Airless` `Icy` planet at T0 above 273.15 K" to main as the one case
//! that would change the design. P14.T33.c's census found three in 10⁶ bodies (0.04–0.12 M⊕ at
//! 1.2–1.6 S⊕, kept at 205–215 K by their bright ice). Ice in vacuum retreats there (Hertz–Knudsen
//! at the vapour pressure of Murphy and Koop 2005, QJRMS 131, 1539): free sublimation of 15–60 m a
//! year at 205–215 K, held near a metre a year by the light it absorbs, so over Gyr its surface is
//! the lag, §4's own condition of vacuum or an air far below saturation. From 26 a planet with no
//! given bulk whose bare derivation publishes `Airless` or `Tenuous` over an ice bulk at a T0 above
//! the melting point is derived once more under [`SolvedLag::AboveMeltingOverIce`], by one test
//! ([`planet_takes_the_lag`]) at each site that derives it, its record's and its rings' parent's,
//! and every other planet keeps its bits. The lag takes only what its ice crust held, so the
//! lagged derivation cannot gain an air: there is no feedback.
//!
//! [`SolvedLag::AboveMeltingOverIce`]: crate::planetary::derive::SolvedLag::AboveMeltingOverIce

use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::planetary::hooks::figures::cover::UNDER_ICE_MELTING_POINT;

/// Whether the planet derived bare as `derived` is derived once more under
/// [`SolvedLag::AboveMeltingOverIce`](crate::planetary::derive::SolvedLag::AboveMeltingOverIce):
/// where it publishes `Airless` or `Tenuous`, its bulk is ice and its zero-albedo temperature at
/// its flux exceeds [`UNDER_ICE_MELTING_POINT`], the lag's own test to the bit (from 26).
#[must_use]
pub(crate) fn planet_takes_the_lag(derived: &DerivedBody) -> bool {
    matches!(
        derived.atmosphere().published_state(),
        SurfaceState::Airless | SurfaceState::Tenuous
    ) && SurfaceMaterial::of(&derived.fractions()) == SurfaceMaterial::Ice
        && equilibrium_temperature(derived.flux(), BondAlbedo::from_fraction(0.0))
            > UNDER_ICE_MELTING_POINT
}
