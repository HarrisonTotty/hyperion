//! Step 4, the climate (R09.T13, Design note 8).
//!
//! The model the climate regime names: the seasonal moist energy-balance model, in tidally locked
//! coordinates for a locked world, radiative equilibrium with thermal inertia, or the isothermal
//! surface; normalised to plan 14's mean and signed contrasts, with ice and liquids placed by
//! substance to plan 14's areas, and the labelled precipitation heuristic with each month's wind.
//!
//! As built by R09.T10 it is a no-op, which R09.T13 replaces.

use super::{Pass, Working};

/// Runs the step on `working`, which it leaves unchanged until R09.T13 builds it: it will write
/// the climate cells, the lapse rate and the cells' ice.
pub fn run(_pass: &Pass<'_>, _working: &mut Working) {}
