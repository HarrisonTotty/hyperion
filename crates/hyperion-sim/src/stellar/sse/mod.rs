//! The backbone: the analytic stellar evolution formulae of Hurley, Pols and Tout (2000, MNRAS 315,
//! 543; "HPT"), for every phase from the zero-age main sequence to the remnant, at any mass from 0.1
//! to 100 M☉ and metal fraction from 0.0001 to 0.03.
//!
//! The formulae are transcribed from the paper; the published SSE code is used only to produce
//! reference output for the comparison tests and to settle where the printed form and the code
//! disagree, each case named in the doc comment that settles it, and none of its code is copied.
//! Mass loss is integrated on a fixed grid of knots per phase that never reads the age asked for
//! (plan 06, design notes 1 and 2), so a star's state is a continuous function of age and the same
//! whatever was asked before.
//!
//! Tests compare with reference values from the published SSE package: `sse.tar.gz` from Jarrod
//! Hurley's page (<http://astronomy.swin.edu.au/~jhurley/sse.tar.gz>, the archive dated
//! 2006-11-24, retrieved on 2026-09-21 through the Internet Archive's capture of 2025-04-20, sha256
//! `61c02d333943226c3a78b9bb832208ceaa853b16faa7f466d85c21b3b73ebcfe`), built with
//! `x86_64-w64-mingw32-gfortran` 16.2.0 and run under Wine 11.17 on 2026-09-21; each test states
//! the options of its run. Only the numbers are committed; no code of the package is.

mod calibration;
mod coeffs;
mod coeffs_data;
#[cfg(test)]
mod continuity;
mod envelope;
mod gb;
mod hg;
mod ms;
#[cfg(test)]
mod reference;
mod wind;
pub mod zams;

// Core helium burning, the asymptotic giant branch and naked helium stars (P06.T7–T9).
mod agb;
mod cheb;
mod helium;

// The track integrator (P06.T10.c–e).
mod evolve;
mod track;
pub use evolve::{evolve, lifetime, main_sequence_state, turn_off_mass};
pub(crate) use track::fate_of;
// Plan 11's hooks into the track (P11.T4, ruling 34.1): see `track/binary.rs`.
pub use track::{Bridges, MAX_INITIAL_MASS, MIN_INITIAL_MASS, Track, TrackOptions};
pub(crate) use track::{
    CORE_GYRATION, ConvectiveEnvelope, ENVELOPE_GYRATION, NewStar, Remains, Structure,
    giant_radius_exponent, lightest_helium_star, main_sequence_lifetime, main_sequence_radius,
    main_sequence_structure, new_star_mass,
};

// The core mass at the base of the AGB and the interpulse period, for P06.T18.b and T28.f.
#[cfg_attr(
    not(test),
    expect(unused_imports, reason = "P06.T18.b and T28.f are the first callers")
)]
pub(crate) use agb::interpulse_period;
pub(crate) use gb::mc_bagb as m_c_bagb;

pub use coeffs::ZCoeffs;
pub use wind::WindRecipe;

// Defined with the remnants; re-exported beside `WindRecipe`, the other choice a track is built
// with, where plan 06's Provides places both.
pub use crate::stellar::remnant::RemnantRecipe;

use crate::units::{SolarLuminosities, SolarMasses, SolarRadii};

/// Luminosity, radius and core mass at one age, as one phase's formulae give them for a star of
/// fixed (effective initial) mass; the track integrator (P06.T10) turns these into a
/// [`StarState`](crate::stellar::StarState).
///
/// An open record: it carries no invariant of its own, since [`StarState::new`] checks the
/// values when the integrator builds a state from them.
///
/// [`StarState::new`]: crate::stellar::StarState::new
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PhasePoint {
    /// Bolometric luminosity, L☉.
    pub(crate) luminosity: SolarLuminosities,
    /// Radius, R☉.
    pub(crate) radius: SolarRadii,
    /// Core mass, M☉; zero on the main sequence, where HPT define no core.
    pub(crate) core_mass: SolarMasses,
}
