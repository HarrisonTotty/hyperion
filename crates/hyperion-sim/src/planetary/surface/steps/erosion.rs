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
//!   surface, drawn with probability proportional to the drop per metre from the cell's own mark
//!   on `surface.coarse.erosion` (instance 0, keyed by its `surface_cell`; one word kept as an
//!   integer, against the integer thresholds of the weights' running sums), with Tzathas et al.'s
//!   slope correction on the face's two axes; the depth-first order with each cell's
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
//!   against each flooded cell's net loss (lakes on a world wet now, Q3); and the routing with
//!   water, the
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
//!   as a base and a part linear in the uplift's scale; and, at steady state, a threshold slope
//!   that holds each link's rise to min(u ℓ ÷ a, `s_c` ℓ), Tzathas et al.'s §6.2 term in its hard
//!   limit (decision-r09-t14-choices, "T14.c follow-up" item 2).
//! - `multigrid`: the fixed point of routing and solution with the moving average of weight ½,
//!   coarse to fine from level 4 on the quadtree's parents (solid-angle-weighted means), six
//!   iterations a level, the finest level's last iteration the pure solution; once a level, on its
//!   construction, the water bodies, the bounds and, on a world wet now, each cell's basin and
//!   base level, with the uplift u = `λ_b` max(0, z0 − `ℓ_b`) a field fixed in space, each cell's
//!   own basin's `λ_b` in the path integral, started from Q1's closed form at each level and
//!   refitted every iteration by an additive step, `λ_b` ← max(0, `λ_b` + ½ Σ Ω (z0 − s) ÷
//!   `S_b`), so that each basin keeps its construction's mean elevation (decision-r09-t14-choices
//!   Q1, as its "T14.c follow-up" item 1 and "T14.c follow-up 2" item 1 rule it), the receivers
//!   sticky from each level's third iteration on a world wet now; and each finer level's start,
//!   its construction plus the upsample of the coarser level's change through R09.T4's interpolant
//!   at child centres
//!   jittered on `surface.coarse.erosion` (instance 1, the receivers' being 0; "T14.b follow-up"
//!   item 3), with each level's residuals, largest changes and changed receivers, `λ_b`'s
//!   distribution and the implied mean uplift recorded.
//!
//! R09.T14.c builds the branches and the outputs, which [`run`] calls:
//!
//! - `branches`: never wet, nothing; dry now, the transient for the wet epoch's effective flow
//!   times the share of the epoch the surface saw, nothing where it saw none, with no uplift, the
//!   epoch's runoff on every cell and the paleo-inventory's lakes as bounds, no threshold slope,
//!   the young surface inside the waiting craters' rims eroded but shedding no runoff of its own
//!   (Q2, Q4), the
//!   volcanic provinces exposed and bedded by their own surface's age and renewed after the solve
//!   where younger than the epoch, and each eroded cell lowered by its valley's void, its
//!   bankfull floor and 10:1 walls, not its channel's whole incision
//!   (decision-r09-t14-mars-ceiling §2.1–2.2); wet now, the steady state under each basin's
//!   freeboard with the climate's runoff, no link rising more than `DEBRIS_FLOW_SLOPE` times its
//!   length, ice cells shedding it as it is, the sea and the construction's terminal lakes and dry
//!   playas by their water balance the bounds, its full lakes routed through and graded, and its
//!   heights then remapped onto the construction's (Q1, Q3, Q5; the "T14.c follow-up" items 1–2,
//!   follow-ups 2 and 3 and "T14.d lakes").
//! - `outputs`: the elevation, the water surfaces, the final elevation's routing, flow, geometric
//!   drainage and steepness index on the filled surface, the own and catchment runoffs, a world
//!   dry now's channel incision (F2), zero inside the waiting craters' rims (R09.T6.c), and the
//!   marker that the field is routed.
//!
//! R09.T14.d builds the rest, in the fixed order of decision-r09-t14-choices Q7 and its "T14.d
//! follow-up" ([`run`]), the climate's runoff read live and the construction's lakes held by their
//! water balance in the wet-now solve (Q3):
//!
//! 1. the solve's outputs, provisional, written into the cells (R09.T14.c's);
//! 2. `σ_h` re-matched once on the reconstructed field (`outputs::rematch`, R09.T12.e's
//!    `budget::match_sigma_h`, counting the waiting craters and folding their bytes, R4);
//! 3. every cell's water surface its ground and its liquid entry none (the follow-up's D);
//! 4. the sea placed again at the same ocean fraction (R09.T12.e's `sea::place`), the new ℓ;
//! 5. on a world wet now, the lakes on the final field outside the ocean by their water balance
//!    (Q3; Fill–Spill–Merge on fluxes), each flooded cell's water surface its lake's level and its
//!    liquid the main liquid; elsewhere every closed depression a dry playa;
//! 6. the climate's lapse term and mean re-applied at the final water surfaces and ℓ, with the
//!    ices and the liquids placed again (`climate::after_erosion`, Q6(b));
//! 7. the final outputs: the final field routed with its water bodies, the sea and the lakes,
//!    its flow, geometric drainage and steepness index on the filled surface, the own runoffs
//!    kept, the catchment runoff the own runoffs accumulated over that routing (Hergarten 2021,
//!    eq. 20), the field marked routed, and, where no crater waits for the step, the realised
//!    figures (`budget::figures`) of the field as it stands.
//!
//! After the late craters, `route_final` routes the final field again as it stands, the sea at
//! ℓ and the cells under liquid, keeping every own runoff (Q7's addition), so that no flow climbs a
//! late crater's rim. Labelled (Q7): a paleo-valley beheaded by a late crater takes the present
//! drainage downstream, which is smaller than the drainage that cut it.
//!
//! Every part is implemented from the papers alone: no code is copied or paraphrased from Tzathas
//! et al.'s repository (Inria research-only licence), Dendry (GPL-3.0) or Barnes's `RichDEM`.
//!
//! [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL

mod branches;
mod depressions;
mod drainage;
mod erodibility;
mod mesh;
mod multigrid;
mod outputs;
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
pub(crate) use branches::Branch;
use outputs::{Provisional, Record};

/// Runs the step on `working` as the climate step left it (see the module documentation): by the
/// world's branch, the multigrid's erosion and its provisional outputs, then R09.T14.d's re-match,
/// sea, lakes, climate re-application and final outputs, the cells' elevation, water surfaces,
/// liquids and ices, flow, drainage, steepness, runoffs and channel incision, the field marked
/// routed; nothing on a world never wet or where the surface is no older than its wet epoch's
/// end, where every cell stays terminal and the field unrouted.
///
/// # Panics
///
/// Never on a working state the steps before it left: every value is finite, and every sea cell's
/// ground at or below the sea's level.
pub fn run(pass: &Pass<'_>, working: &mut Working) {
    // The record is the tests' and the plan's, not the pass's.
    let _record = run_recorded(pass, working);
}

/// [`run`], returning what it found ([`Record`]), or `None` where it did nothing.
///
/// # Panics
///
/// As [`run`].
fn run_recorded(pass: &Pass<'_>, working: &mut Working) -> Option<Record> {
    let provisional = provisional(pass, working)?;
    Some(outputs::finish(pass, working, provisional))
}

/// The step's first part (R09.T14.c): by the world's branch, the multigrid's erosion and its
/// outputs written into `working` as they stand on the solve, before R09.T14.d's re-match
/// (decision-r09-t14-choices Q7's step 1); `None` where the step does nothing.
///
/// # Panics
///
/// As [`run`].
fn provisional(pass: &Pass<'_>, working: &mut Working) -> Option<Provisional> {
    match Branch::of(pass.inputs()) {
        Branch::NeverWet => None,
        Branch::DryNow => dry_now(pass, working),
        Branch::WetNow => Some(wet_now(pass, working)),
    }
}

/// Routes `working`'s final field again as it stands, after the late coarse craters
/// (decision-r09-t14-choices Q7's addition and its "T14.d follow-up" C): its flow, geometric
/// drainage, steepness index and catchment runoffs, the sea at ℓ and every cell under liquid a
/// root, every closed depression of the dry ground a dry playa, each cell's own runoff kept
/// (`outputs::route_as_it_stands`). It writes no elevation, water surface or own runoff, so a
/// body that shed no runoff (a pressure of zero has no wet epoch) gains none.
///
/// The relief step's `after_late_craters` calls it between the reflood and the book, only where
/// [`run`] routed the field (`Working::routed`). Labelled (decision-r09-t14-choices Q7): a
/// paleo-valley beheaded by a late crater takes the present drainage downstream, which is smaller
/// than the drainage that cut it.
///
/// # Panics
///
/// If `working` is not routed, or a value is not finite.
pub(crate) fn route_final(pass: &Pass<'_>, working: &mut Working) {
    assert!(working.routed, "the erosion step routed the field");
    outputs::route_as_it_stands(pass, working);
}

/// The dry-now branch's erosion and provisional outputs (decision-r09-t14-choices Q2 and Q4;
/// decision-r09-t14-mars-ceiling §2.1–2.2), nothing where the surface is no older than the epoch's
/// end.
fn dry_now(pass: &Pass<'_>, working: &mut Working) -> Option<Provisional> {
    let eroded = branches::dry_now(pass, working)?;
    let epoch = pass
        .inputs()
        .wet_epoch()
        .expect("a world dry now has a wet epoch");
    let runoff = eroded.output_runoff_m_per_year(epoch, &eroded.young);
    let incision = eroded.output_incision_m();
    let sea = branches::sea_cells(working);
    let sea_level_m = working.sea_level.value();
    // Routed as the final field will be, with no paleo-lake (Q7's step 5 on a world dry now).
    outputs::write_with(
        working,
        sea_level_m,
        &eroded.outcome,
        &outputs::Written {
            elevation_m: &eroded.elevation_m,
            sea: &sea,
            runoff_m_per_year: &runoff,
            channel_incision_m: Some(&incision),
        },
        outputs::ProvisionalWater::DryPlayas,
    );
    Some(Provisional {
        branch: Branch::DryNow,
        outcome: eroded.outcome,
    })
}

/// The wet-now branch's erosion and provisional outputs (decision-r09-t14-choices Q1, Q3 and Q5,
/// and the "T14.c follow-up 2" §2's remap), its source the climate's annual runoff, read for every
/// cell before any changes.
fn wet_now(pass: &Pass<'_>, working: &mut Working) -> Provisional {
    let fluid = erodibility::Fluid::of(pass.inputs()).expect("a world wet now has a fluid");
    let runoff = branches::wet_now_runoff_m_per_year(pass, working, fluid);
    let outcome = branches::erode_wet_now(pass, working, &runoff);
    let z0: Vec<f64> = working
        .cells
        .iter()
        .map(|cell| cell.elevation.value())
        .collect();
    let remapped = branches::remap_onto_construction(&outcome, &z0, &outcome.z_m);
    let sea = branches::sea_cells(working);
    let sea_level_m = working.sea_level.value();
    outputs::write(
        working,
        sea_level_m,
        &outcome,
        &outputs::Written {
            elevation_m: &remapped.elevation_m,
            sea: &sea,
            runoff_m_per_year: &runoff,
            channel_incision_m: None,
        },
    );
    Provisional {
        branch: Branch::WetNow,
        outcome,
    }
}
