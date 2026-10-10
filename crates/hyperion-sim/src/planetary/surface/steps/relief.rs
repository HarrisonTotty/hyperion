//! Step 2, the coarse elevation (R09.T12.a–c and e, Design note 7).
//!
//! The two crust populations about sea level with GDH1's age–depth law at the ridges, the
//! convergent landforms, a stagnant lid's volcanic provinces with their flexure, and the affine
//! scaling that makes the reconstructed field's `σ_h` plan 14's, with sea level at the
//! area-weighted quantile that leaves plan 14's ocean fraction below it.
//!
//! As built by R09.T10 it is a no-op, which R09.T12 replaces.

use super::{Pass, Working};

/// Runs the step on `working`, which it leaves unchanged until R09.T12 builds it: it will write
/// the cells' elevation and water surface, the sea level and the realised figures.
pub fn run(_pass: &Pass<'_>, _working: &mut Working) {}
