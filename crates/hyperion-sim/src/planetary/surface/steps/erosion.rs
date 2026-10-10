//! Step 5, drainage and erosion (R09.T14, Design note 9).
//!
//! Where the world has or had liquid at its surface: random receivers on `surface.coarse.erosion`,
//! priority flood and Fill–Spill–Merge, and the stream-power law by Tzathas et al.'s analytical
//! method with multigrid, to steady state on a world wet now or for the wet epoch's effective flow
//! on one dry now; then `σ_h` re-matched on the reconstructed field.
//!
//! As built by R09.T10 it is a no-op, which R09.T14 replaces.

use super::{Pass, Working};

/// Runs the step on `working`, which it leaves unchanged until R09.T14 builds it: it will write
/// the cells' elevation, flow, drainage, steepness and water surface.
pub fn run(_pass: &Pass<'_>, _working: &mut Working) {}
