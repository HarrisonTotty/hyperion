//! Step 5, drainage and erosion (R09.T14, Design note 9).
//!
//! Where the world has or had liquid at its surface, the step routes the water over the coarse
//! cells and solves the stream-power law ∂z/∂t = u − k A^m ‖∇z‖ⁿ by Tzathas et al. 2024's
//! analytical method (CGF 43(2), doi:10.1111/cgf.15033), which requires n = 1: to steady state on
//! a world wet now, for the wet epoch's effective flow on one dry now, and not at all on a world
//! never wet. Each river tree is marched depth first from its outlet; the fixed point iterates
//! routing and solution with a moving average of weight ½; multigrid runs on the quadtree's own
//! parents down to level 4, six iterations a level; and `σ_h` is then re-matched on the
//! reconstructed field.
//!
//! # As built
//!
//! R09.T14.a builds the drainage, which the solver re-runs on its field at every iteration:
//!
//! - `mesh`: one level's cells as the drainage reads them, each cell's area (its solid angle
//!   times R²), its four edge neighbours in [`Edge::ALL`]'s order and each link's great-circle
//!   length.
//! - `drainage`: Barnes, Lehman and Mulla 2014's improved Priority-Flood from the sea and the
//!   terminal lakes, a flat's cells draining to the cell that closed them, so with no ε-raising;
//!   each cell's receiver, one of its four edge neighbours strictly below it on the filled
//!   surface, drawn with probability proportional to the drop per metre from the cell's own
//!   uniform on `surface.coarse.erosion` (instance 0, keyed by its `surface_cell`), with Tzathas
//!   et al.'s slope correction on the face's two axes; the depth-first order with each cell's
//!   depth, for the solver's path array; the donors; each cell's basin, the root it drains to,
//!   with the basin's base level; and the accumulation in reverse preorder, which gives the
//!   geometric drainage area by solid angle (Hack 1957's) and the solver's equivalent area
//!   (Hergarten 2021, eqs. 16 and 20).
//! - `depressions`: Barnes, Callaghan and Wickert 2020's depression hierarchy, built as the
//!   terrain's merge tree; their 2021 Fill–Spill–Merge, which spreads water through it, filling
//!   the lowest depressions first and spilling into their neighbours, merged above their shared
//!   sill when both are full, and on to the sea, a high basin's spill entering the basin below it;
//!   the water bodies, full (through-flow) at their spill or terminal below it, a lake or a dry
//!   playa, with their levels and the cells under them by the field's millimetre codes, the
//!   water a volume (a paleo-inventory, decision-r09-t14-choices Q2) or a balance of inflow
//!   against net evaporation (lakes on a world wet now, Q3); and the routing with water, the
//!   drainage on the current ground from the sea and the terminal bodies' pits, the bodies found
//!   once on a level's construction.
//!
//! R09.T14.b–d build the rest: the erodibility K from the bed and its fluid, Tzathas et al.'s
//! recursion with the fixed point and the multigrid with the jittered upsample (R09.T14.b); the
//! wet-now, dry-now and never-wet branches (R09.T14.c); and the re-match of `σ_h` with the
//! outputs, the flow directions, the geometric drainage area, the steepness index, the two
//! runoffs and the water surfaces (R09.T14.d). Until R09.T14.c, [`run`] is a no-op.
//!
//! Every part is implemented from the papers alone: no code is copied or paraphrased from Tzathas
//! et al.'s repository (Inria research-only licence), Dendry (GPL-3.0) or Barnes's `RichDEM`.
//!
//! [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.b's solver and R09.T14.c's branches call it"
    )
)]
mod depressions;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.b's solver and R09.T14.c's branches call it"
    )
)]
mod drainage;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.b's solver and R09.T14.c's branches call it"
    )
)]
mod mesh;

#[cfg(test)]
mod tests;

use super::{Pass, Working};

/// Runs the step on `working`, which it leaves unchanged until R09.T14.c's branches call the
/// drainage and R09.T14.b's solver: it will write the cells' elevation, flow, drainage, steepness,
/// runoffs and water surfaces.
pub fn run(_pass: &Pass<'_>, _working: &mut Working) {}
