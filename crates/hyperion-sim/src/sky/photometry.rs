//! A star's absolute V magnitude and colour as the sky reads them (rendering plan R06, Design
//! note 7).
//!
//! The V magnitude, [`absolute_v_of_state`] and [`is_dark_in_v`], carries the interims of galaxy
//! plan 06's asks A3 and A4 (protostars and white dwarfs dark in V). It lives in
//! [`stellar::photometry`](crate::stellar::photometry), beside the photometry it stands in for,
//! whose module documentation gives both interims and their sources, and is re-exported here, so
//! every sky path to it holds. When plan 06 answers A3 and A4, those two functions switch to its
//! photometry and nothing here changes. Two of the census's bounds rest on A4's interim, and must
//! change with it: the phase envelope's bins beyond a star's lifetime hold only remnants, which it
//! keeps dark ([`PhaseEnvelope`](super::phase::PhaseEnvelope), R06.T8.m), and a pair that plan 11
//! answers `Remnants` gives the census nothing ([`StarBounds`](super::census::StarBounds),
//! R06.T8.g). Both then take white-dwarf rows.

use super::colour::{StarColour, star_colour, surface_gravity};
use super::disc::grid_of;
use crate::stellar::StarState;

pub use crate::stellar::photometry::{absolute_v_of_state, is_dark_in_v};

/// The colour of `state` from the spectral table ([`star_colour`]): its effective temperature, its
/// gravity from its own mass and radius ([`surface_gravity`], Design note 6) and the grid of its
/// phase, as the host discs read them.
#[must_use]
pub fn colour_of_state(state: &StarState) -> StarColour {
    star_colour(
        state.effective_temperature(),
        surface_gravity(state.mass(), state.radius()),
        grid_of(state.phase()),
    )
}
