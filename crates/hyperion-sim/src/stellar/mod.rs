//! Stars: evolution, remnants and classes (plan 06).
//!
//! A star's state at any age is a pure function of its initial mass, its [`Composition`], its
//! fixed per-star draws and the age: `state = evolve(m, Z, age)`, continuous in age, with no table
//! binned by age. The backbone is the analytic evolution of Hurley, Pols and Tout (2000, MNRAS 315,
//! 543) in [`sse`]; around it sit the stages it does not cover (below 0.1 M☉ in [`substellar`],
//! before the main sequence in [`premain`]), what a star leaves when it dies and the kick it gets
//! ([`remnant`]), its spectral and luminosity class ([`classify`]), magnitudes ([`photometry`]),
//! pulsation ([`variability`]), rotation and magnetism ([`rotation`]), planetary nebulae
//! ([`nebula`]) and single-star events in time ([`events`]). [`system`] assembles a system's stars
//! from its record, [`fates`] hands lifetimes and remnant masses to the galaxy's mean-mass
//! quadrature, and [`draws`] holds every random draw a star makes, each on a stream of its own.
//!
//! Everything here works in solar units and Julian years through the
//! [`units`](crate::units) newtypes. Nothing in this module reads plan 03's placement except
//! [`system`].

pub mod classify;
pub mod composition;
pub mod draws;
pub mod events;
pub mod fates;
pub mod nebula;
pub mod photometry;
pub mod premain;
pub mod remnant;
pub mod rotation;
pub mod sse;
pub mod state;
pub mod substellar;
pub mod system;
#[cfg(test)]
pub(crate) mod testing;
pub mod variability;

pub use composition::Composition;
pub use state::{ObjectKind, Phase, StarState, StarStateParts};
