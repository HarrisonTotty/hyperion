//! Step 1, the plates (R09.T11, Design note 6).
//!
//! Where the tectonic regime is a mobile lid, the plates are drawn on `surface.coarse.plates` keyed
//! by `surface_item(k)`, rasterised onto the cells by the largest dot product with their seeds
//! under the low-frequency warp of `surface.coarse.warp`, ties to the lower seed index, and a
//! distance transform over the cell graph gives each cell its signed distance to the nearest
//! boundary, that boundary's kind and its obliquity. A stagnant lid gets no boundaries.
//!
//! As built by R09.T10 it is a no-op, which R09.T11 replaces.

use super::{Pass, Working};

/// Runs the step on `working`, which it leaves unchanged until R09.T11 builds it: it will write
/// the cells' plate, crust and boundary.
pub fn run(_pass: &Pass<'_>, _working: &mut Working) {}
