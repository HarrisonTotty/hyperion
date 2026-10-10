//! Step 3, the coarse craters (R09.T12.d, Design notes 5 and 10).
//!
//! Craters of the boundary diameter and wider, drawn on `surface.coarse.crater` from the crater
//! contract, min(production, saturation) per octave, smoothed into the elevation and listed with
//! their reach; on a world whose wet epoch ended before its surface age ran out, those younger
//! than the epoch's end wait until after erosion (Design note 5).
//!
//! As built by R09.T10 it is a no-op, which R09.T12.d replaces.

use super::{Pass, Working};

/// Runs the step on `working`, which it leaves unchanged until R09.T12.d builds it: it will write
/// the craters and the elevation they shape.
pub fn run(_pass: &Pass<'_>, _working: &mut Working) {}
