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
//! R09.T14.b builds the solver, which R09.T14.c's branches call:
//!
//! - `erodibility`: K = K⊕ (g ÷ g⊕)(`ρ_f` ÷ `ρ_ref`) B in m^0.1 a⁻¹ (Design note 9, R09.T0.b), K⊕
//!   = 6.3 × 10⁻⁶, Tzathas et al.'s calibration converted to m = 0.45; the fluid the main
//!   liquid's palette entry on a world wet now and the wet epoch's liquid's registry row on one
//!   dry now (decision-r09-t14-choices Q6a); B a (substrate, fluid) table with its sources, rock
//!   under water 1, a crater-saturated regolith under water 10 and water ice under methane,
//!   ethane or Titan's sea 8, any other pair 1 (not modelled); and the per-world criterion of a
//!   crater-saturated surface, the craters older than the epoch's end at or above the basins'
//!   saturation density.
//! - `solver`: Tzathas et al.'s recursion with n = 1 over a network's depth-first order with a
//!   path array, at steady state (their eq. 9) or after a time (eq. 8, D found by a binary search
//!   of the path's τ and interpolated inside its link), m = 0.45 on the equivalent area with the
//!   slope correction; roots, bounds and cells of no equivalent area kept at their ground and
//!   reset the path to their base; no deposition on a cell with no uplift; the elevation returned
//!   as a base and a part linear in the uplift's scale.
//! - `multigrid`: the fixed point of routing and solution with the moving average of weight ½,
//!   coarse to fine from level 4 on the quadtree's parents (solid-angle-weighted means), six
//!   iterations a level, the finest level's last iteration the pure solution; once a level, on its
//!   construction, the water bodies, the bounds and, on a world wet now, each cell's basin and
//!   base level, with the uplift u = `λ_b` max(0, z0 − `ℓ_b`) refitted every iteration so that
//!   each basin keeps its construction's mean height above its base level
//!   (decision-r09-t14-choices Q1); and the upsample through R09.T4's interpolant at child centres
//!   jittered on `surface.coarse.erosion` (instance 1, the receivers' being 0), with each level's
//!   last residual, `λ_b`'s distribution and the implied mean uplift recorded.
//!
//! R09.T14.c–d build the rest: the wet-now, dry-now and never-wet branches (R09.T14.c); and the
//! re-match of `σ_h` with the outputs, the flow directions, the geometric drainage area, the
//! steepness index, the two runoffs and the water surfaces (R09.T14.d). Until R09.T14.c, [`run`]
//! is a no-op.
//!
//! Every part is implemented from the papers alone: no code is copied or paraphrased from Tzathas
//! et al.'s repository (Inria research-only licence), Dendry (GPL-3.0) or Barnes's `RichDEM`.
//!
//! [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.b's multigrid calls it, which R09.T14.c's branches call"
    )
)]
mod depressions;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.b's multigrid calls it, which R09.T14.c's branches call"
    )
)]
mod drainage;
#[cfg_attr(not(test), expect(dead_code, reason = "R09.T14.c's branches call it"))]
mod erodibility;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "R09.T14.b's multigrid calls it, which R09.T14.c's branches call"
    )
)]
mod mesh;
#[cfg_attr(not(test), expect(dead_code, reason = "R09.T14.c's branches call it"))]
mod multigrid;
#[cfg_attr(not(test), expect(dead_code, reason = "R09.T14.c's branches call it"))]
mod solver;

#[cfg(test)]
mod tests;

// The draws' instances on `surface.coarse.erosion` are distinct, so that the receivers and the
// upsample's jitter never share a stream (as the relief step's are).
const _: () = assert!(
    drainage::RECEIVER_INSTANCE != multigrid::JITTER_INSTANCE,
    "the erosion step's receivers and jitter share an instance"
);

use super::{Pass, Working};

/// Runs the step on `working`, which it leaves unchanged until R09.T14.c's branches call the
/// drainage and R09.T14.b's solver: it will write the cells' elevation, flow, drainage, steepness,
/// runoffs and water surfaces.
pub fn run(_pass: &Pass<'_>, _working: &mut Working) {}
