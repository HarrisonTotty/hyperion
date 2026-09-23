//! Derivation: everything about a body that is computed from its mass, orbit and host rather than
//! drawn (plan 14, phase C; the brainstorm's "Everything else is computed, not rolled").
//!
//! So far only [`limits`] is built: Roche limits, Hill radii, the stability limit of satellites
//! and the heaviest moon a close-in planet can keep (P14.T15). Radius and composition (T11),
//! irradiation and the habitable zone (T12), atmospheres (T13) and rotation and tides (T14) follow.

pub mod limits;

pub use limits::{
    OrbitSense, hill_radius, maximum_surviving_moon_mass, roche_limit_fluid, roche_limit_rigid,
    satellite_stability_limit,
};
