//! Step 6, the classes and the crater state (R09.T15, Design note 11).
//!
//! Köppen–Geiger classes for seasonal water-cycle regimes, by rates over each cell's own summer,
//! surface-state classes naming their palette entry for the other regimes and one-month worlds,
//! and each cell's crater state.
//!
//! As built by R09.T10 it is a no-op, which R09.T15 replaces.

use super::{Pass, Working};

/// Runs the step on `working`, which it leaves unchanged until R09.T15 builds it: it will write
/// the cells' class and crater state.
pub fn run(_pass: &Pass<'_>, _working: &mut Working) {}
