//! Single-star events in time: flares, pulsar glitches, magnetar bursts and giant flares, FU
//! Orionis outbursts, the giant eruptions of luminous blue variables and thermal pulses, each built
//! on the generic, random-access constructions of `hyperion_sim::events` and keyed on the star's own
//! streams (plan 06, phase F).
//!
//! An event may change what a console reads, never what the star's track is (plan 06, design
//! note 14).
//!
//! - [`flares`]: the flares of stars with convective envelopes, Poisson bins of a day (P06.T28.a).
//! - [`fu_orionis`]: the FU Orionis outbursts of young stars, Poisson bins of 500 years
//!   (P06.T28.d).
//! - [`glitches`]: pulsar glitches, Crab-like on Poisson bins of 30 days and Vela-like on a
//!   monotone phase (P06.T28.b).
//! - [`lbv`]: the giant eruptions of luminous blue variables, Poisson bins of 100 years
//!   (P06.T28.e).
//! - [`thermal_pulse`]: the thermal pulses of stars on the thermally pulsing AGB, on a monotone
//!   phase of their track's own pulse count (P06.T28.f).

pub mod flares;
pub mod fu_orionis;
pub mod glitches;
pub mod lbv;
pub mod thermal_pulse;
