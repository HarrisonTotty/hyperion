//! The erosion step's outputs (R09.T14.c and R09.T14.d; the lane's design §4.3 and §5.2): what the
//! step writes into the working state from the multigrid's outcome, provisionally on the solve,
//! then in the fixed order of decision-r09-t14-choices Q7 and its "T14.d follow-up" on the final
//! field, and again after the late craters.
//!
//! # The provisional outputs ([`write`], Q7's step 1)
//!
//! - **The elevation**: each eroded cell's, the branch's (the solve's, on a world dry now through
//!   `branches::dry_now_elevation_m`); each bound keeps its ground to the bit.
//! - **The water surface**: the elevation on every cell off the sea, which is dry (a world dry now
//!   writes none: its paleo-lakes are gone); the sea's cells, bounds, keep theirs.
//! - **The routing**: the final elevation routed afresh with the field's water bodies
//!   ([`ProvisionalWater`]: on a world wet now the solve's lakes on its construction, on one dry
//!   now every closed depression a dry playa, its paleo-lakes gone, as the final field takes it,
//!   so that the channels' share the re-match books from these records is routed as the final
//!   field's) and the solver's receiver marks ([`route_with_water`]), so that no cell drains
//!   uphill on the field the step leaves (T14.a's property), the fixed point's last routing being
//!   the previous iterate's (decision-r09-t14-choices, "T14.b follow-up" item 1): each cell's
//!   flow, its receiver's edge or terminal at a root; its geometric drainage area, its own
//!   included (Hack 1957's, by solid angle); and its steepness index `k_s` = S A^θ, S = max(0,
//!   `W_c` − `W_r`) ÷ ℓ on the filled surface W and θ = [`LogSteepness::THETA`], zero at a root.
//! - **The runoffs**: each cell's own, the branch's output rule; and its catchment's, the own
//!   runoffs times the cells' areas accumulated over that routing, over its drainage area
//!   (Hergarten 2021, eq. 20), zero where that is zero.
//! - **The channel incision** ([`CellState::channel_incision`]): on a world dry now, z0 less the
//!   solve, before any closure (decision-r09-t14-choices F2), zero inside a waiting crater's rim
//!   (R09.T6.c); zero elsewhere.
//! - **The marker** [`Working::routed`], which the re-routing after the late craters reads.
//!
//! # The final field ([`finish`], Q7's steps 2–7)
//!
//! 2. **The re-match** of `σ_h` on the reconstructed field, once ([`rematch`], R09.T12.e's
//!    `budget::match_sigma_h`), its channels' share booked from the provisional records: it counts
//!    the craters that wait for the step and folds their bytes, never stretching them (R4).
//! 3. **Every cell dry**: its water surface its ground and its liquid entry none (the follow-up's
//!    D), since the match dries the water surfaces and leaves the entries.
//! 4. **The sea** at the same ocean fraction (`sea::place`), the new ℓ.
//! 5. **The lakes**, on a world wet now, on the final field outside the ocean ([`final_bodies`]):
//!    Fill–Spill–Merge on fluxes ([`Measure::Balance`] of `branches::WaterBalance`), each cell's
//!    inflow its own runoff times its area against its net loss were the lake to flood it, the
//!    open liquid's E − P at its ground plus its own runoff (Q3; Matsubara et al. 2011's X ratio);
//!    each flooded cell's water surface its body's level and its liquid entry the main
//!    liquid's. A lake that floods no cell is a dry playa. On a world dry now no liquid stands on
//!    the land now, and every closed depression is a dry playa ([`dry_playas`]).
//! 6. **The climate** re-applied at the final water surfaces and ℓ, its ices and liquids placed
//!    again (`climate::after_erosion::reapply_after_erosion`, Q6(b)).
//! 7. **The final outputs**: the final field routed with its water bodies ([`route_with_water`]:
//!    the sea's cells at ℓ and each terminal body's pit at its level the seeds, a through-flow
//!    lake filled to its spill, draining through its outlet), its flow, geometric drainage and
//!    `k_s` on the filled surface, the own runoff kept, and the catchment runoff the own runoffs
//!    accumulated over that routing (Hergarten 2021, eq. 20), the field marked routed; and, where
//!    no crater waits for the step, the realised figures of the field as it stands
//!    (`budget::figures`, from a fresh book of its records), which the late craters' figures
//!    otherwise replace.
//!
//! # After the late craters ([`route_as_it_stands`])
//!
//! The field routed again as it stands, its own runoffs kept: every cell under liquid a root at
//! its water surface (the sea at ℓ; a lake's cell, which no reference world meets there, since
//! waiting craters need an epoch that ended and lakes a world wet now, as a terminal one), and
//! every closed depression of the dry ground a dry playa, its floor a root, as on a world dry now
//! at step 7, so that where no late crater intervenes the routing is step 7's.
//!
//! [`CellState::channel_incision`]: super::super::CellState::channel_incision
//! [`Working::routed`]: super::super::Working::routed

use hyperion_surface::field::LogSteepness;
use hyperion_surface::num;

use super::super::climate::after_erosion::reapply_after_erosion;
use super::super::climate::seas::under_sea;
use super::super::craters::{Phase, waits};
use super::super::plates::{cell_of, slot};
use super::super::relief::budget::{self, Figures, Match};
use super::super::relief::sea::{self, Sea};
use super::super::{Pass, Working};
use super::branches::{Branch, WaterBalance, sea_cells};
use super::depressions::{Measure, WaterBodies, route_with_water, water_bodies};
use super::drainage::{self, Routing, Seed, accumulate, receiver_marks};
use super::mesh::Mesh;
use super::multigrid::Outcome;
use crate::math;
use crate::units::{Metres, SquareMetres};

/// What a branch writes beside the outcome's routing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Written<'a> {
    /// Each cell's elevation, metres, one a cell: the branch's for an eroded cell; a bound's is
    /// not read.
    pub(crate) elevation_m: &'a [f64],
    /// The sea's cells, whose water surfaces stay.
    pub(crate) sea: &'a [bool],
    /// Each cell's own runoff in the output, m a⁻¹.
    pub(crate) runoff_m_per_year: &'a [f64],
    /// Each cell's channel incision, metres, on a world dry now; `None` elsewhere.
    pub(crate) channel_incision_m: Option<&'a [f64]>,
}

/// What the step's first part leaves for the rest: the world's branch and the multigrid's outcome,
/// whose mesh and receiver marks the final routing reads.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Provisional {
    /// The world's branch, wet now or dry now.
    pub(crate) branch: Branch,
    /// The multigrid's solve on the construction.
    pub(crate) outcome: Outcome,
}

/// What the step found on a world it routed, which its tests and the plan's records read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Record {
    /// The world's branch, wet now or dry now.
    pub(crate) branch: Branch,
    /// The multigrid's solve on the construction.
    pub(crate) outcome: Outcome,
    /// The re-match of `σ_h` (Q7's step 2).
    pub(crate) matched: Match,
    /// The sea on the final field (step 4).
    pub(crate) sea: Sea,
    /// The ocean's cells on the final field, the sea's flood (step 4), the lakes apart.
    pub(crate) ocean: Vec<bool>,
    /// The final field's water bodies (step 5): its lakes, full or terminal, and its dry playas.
    pub(crate) bodies: WaterBodies,
    /// The realised figures of the field the step leaves (step 7), where no crater waits for it.
    pub(crate) figures: Option<Figures>,
}

/// The water bodies the provisional outputs are routed with ([`write_with`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProvisionalWater {
    /// The solve's own, on the construction (the outcome's): on a world wet now, its lakes by
    /// their water balance, close to those of the final field.
    Solve,
    /// None now: every closed depression of the written ground a dry playa ([`dry_playas`]), as
    /// the final field of a world dry now takes it (Q7's step 5), its paleo-lakes gone, so that
    /// the records the re-match books are routed as the final field's will be.
    DryPlayas,
}

/// Writes the step's provisional outputs into `working` from `outcome` and `written` (the module
/// documentation, "The provisional outputs"), routed with the solve's water bodies, and marks the
/// field routed: [`write_with`] of [`ProvisionalWater::Solve`].
///
/// # Panics
///
/// As [`write_with`].
pub(crate) fn write(
    working: &mut Working,
    sea_level_m: f64,
    outcome: &Outcome,
    written: &Written<'_>,
) {
    write_with(
        working,
        sea_level_m,
        outcome,
        written,
        ProvisionalWater::Solve,
    );
}

/// Writes the step's provisional outputs into `working` from `outcome` and `written` (the module
/// documentation, "The provisional outputs"), routed with `water`'s bodies, and marks the field
/// routed.
///
/// # Panics
///
/// If an input is not one a cell of `working`, or a value is not finite.
pub(crate) fn write_with(
    working: &mut Working,
    sea_level_m: f64,
    outcome: &Outcome,
    written: &Written<'_>,
    water: ProvisionalWater,
) {
    let n = working.cells.len();
    for (what, len) in [
        ("elevation", written.elevation_m.len()),
        ("sea flag", written.sea.len()),
        ("runoff", written.runoff_m_per_year.len()),
        ("bound", outcome.bounds.len()),
    ] {
        assert_eq!(len, n, "one {what} a cell");
    }
    for (c, state) in working.cells.iter_mut().enumerate() {
        if outcome.bounds[c].is_none() {
            state.elevation = Metres::new(num::assert_finite(written.elevation_m[c]));
        }
        if !written.sea[c] {
            state.water_surface = state.elevation;
        }
        state.runoff_m_per_year = num::assert_finite(written.runoff_m_per_year[c]);
        state.channel_incision = Metres::new(
            written
                .channel_incision_m
                .map_or(0.0, |incision| num::assert_finite(incision[c])),
        );
    }
    let z = elevations(working);
    let playas;
    let bodies = match water {
        ProvisionalWater::Solve => &outcome.bodies,
        ProvisionalWater::DryPlayas => {
            playas = dry_playas(&outcome.mesh, &z, written.sea);
            &playas
        }
    };
    let routing = route_with_water(
        &outcome.mesh,
        &z,
        written.sea,
        sea_level_m,
        bodies,
        &outcome.marks,
    );
    write_routing(working, &outcome.mesh, &routing);
    working.routed = true;
}

/// The step's rest on the provisional outputs `provisional` has written into `working`: Q7's steps
/// 2 to 7 (the module documentation, "The final field"), and what they found.
///
/// # Panics
///
/// If a value is not finite, or as the steps it calls.
pub(crate) fn finish(pass: &Pass<'_>, working: &mut Working, provisional: Provisional) -> Record {
    let Provisional { branch, outcome } = provisional;
    let matched = rematch(pass, working);
    dry_every_cell(working);
    let sea = sea::place(pass, working);
    let ocean = sea_cells(working);
    let bodies = final_bodies(pass, working, branch, &outcome.mesh, &ocean);
    reapply_after_erosion(pass, working);
    let z = elevations(working);
    let routing = route_with_water(
        &outcome.mesh,
        &z,
        &ocean,
        working.sea_level.value(),
        &bodies,
        &outcome.marks,
    );
    write_routing(working, &outcome.mesh, &routing);
    working.routed = true;
    let late = working.craters.iter().any(|crater| waits(pass, crater.age));
    let figures = (!late).then(|| realised_figures(pass, working));
    Record {
        branch,
        outcome,
        matched,
        sea,
        ocean,
        bodies,
        figures,
    }
}

/// Q7's step 2: `σ_h` re-matched on `working`'s reconstructed field after the solve, R09.T12.e's
/// match once (`budget::match_sigma_h`), the channels' share booked from the records as they
/// stand, the provisional ones (decision-r09-t14-choices Q7 and its "T14.d follow-up" C: the
/// predicted-record iteration replaces this one pass, in this one function).
///
/// # Panics
///
/// As `budget::match_sigma_h`.
pub(crate) fn rematch(pass: &Pass<'_>, working: &mut Working) -> Match {
    budget::match_sigma_h(pass, working)
}

/// Q7's step 3: every cell of `working` dry, its water surface its ground and its liquid entry
/// none (decision-r09-t14-choices, "T14.d follow-up" D), before the sea is placed again.
fn dry_every_cell(working: &mut Working) {
    for cell in &mut working.cells {
        cell.water_surface = cell.elevation;
        cell.liquid_entry = None;
    }
}

/// Q7's step 5 on `working`'s final field, `ocean` its sea's cells on `mesh`: on a world wet now
/// with a main liquid, the lakes by their water balance, written into the cells
/// ([`write_lakes`]); otherwise every closed depression a dry playa ([`dry_playas`]). The water
/// bodies the final routing takes.
fn final_bodies(
    pass: &Pass<'_>,
    working: &mut Working,
    branch: Branch,
    mesh: &Mesh,
    ocean: &[bool],
) -> WaterBodies {
    let z = elevations(working);
    match branch {
        Branch::WetNow => {
            let runoff: Vec<f64> = working.cells.iter().map(|c| c.runoff_m_per_year).collect();
            match WaterBalance::of(pass, working, &runoff) {
                Some(balance) => {
                    let bodies = water_bodies(mesh, &z, ocean, balance.measure());
                    write_lakes(pass, working, &bodies);
                    bodies
                }
                None => dry_playas(mesh, &z, ocean),
            }
        }
        Branch::DryNow | Branch::NeverWet => dry_playas(mesh, &z, ocean),
    }
}

/// Writes `bodies`' lakes into `working` (decision-r09-t14-choices Q3, Q7's step 5): each flooded
/// cell's water surface its body's level and its liquid entry the main liquid's, which the
/// climate's re-application then shares by substance among the flooded cells.
///
/// # Panics
///
/// If a body floods a cell on a world with no main liquid, which has no balance.
fn write_lakes(pass: &Pass<'_>, working: &mut Working, bodies: &WaterBodies) {
    let main = pass.inputs().main_liquid();
    for body in bodies.bodies() {
        for &c in &body.cells {
            let cell = &mut working.cells[slot(c)];
            cell.water_surface = Metres::new(num::assert_finite(body.level_m));
            cell.liquid_entry = Some(main.expect("a world with a water balance has a main liquid"));
        }
    }
}

/// The water bodies of no water on `mesh`'s ground `z`, `wet` its cells under liquid: every
/// closed depression of the dry ground a dry, terminal playa, its pit's ground its level, as
/// Fill–Spill–Merge leaves it with nothing to spread (decision-r09-t14-choices Q3's "a lake that
/// takes no cell is a dry playa").
#[must_use]
fn dry_playas(mesh: &Mesh, z: &[f64], wet: &[bool]) -> WaterBodies {
    let none = vec![0.0; z.len()];
    water_bodies(mesh, z, wet, Measure::Volume { water_m3: &none })
}

/// The routing outputs of `working` on `routing` of `mesh` (the module documentation, "The
/// provisional outputs"): each cell's flow, geometric drainage, steepness index on the filled
/// surface and catchment runoff, the own runoffs as `working` holds them accumulated over it.
///
/// # Panics
///
/// If a value is not finite.
fn write_routing(working: &mut Working, mesh: &Mesh, routing: &Routing) {
    let drainage = accumulate(routing, mesh.areas_m2());
    let discharge: Vec<f64> = working
        .cells
        .iter()
        .zip(mesh.areas_m2())
        .map(|(cell, &a)| cell.runoff_m_per_year * a)
        .collect();
    let catchment = accumulate(routing, &discharge);
    let filled = routing.filled_m();
    for (c, state) in working.cells.iter_mut().enumerate() {
        let cell = cell_of(c);
        let area = drainage[c];
        state.flow = routing.flow(cell);
        state.drainage = SquareMetres::new(num::assert_finite(area));
        state.steepness_index_m0_9 = routing.receiver(cell).map_or(0.0, |link| {
            let slope = num::max(0.0, filled[c] - filled[slot(link.cell)]) / link.length_m;
            num::assert_finite(slope * math::powf(area, LogSteepness::THETA))
        });
        state.catchment_runoff_m_per_year = if area > 0.0 {
            num::assert_finite(catchment[c] / area)
        } else {
            0.0
        };
    }
}

/// Routes `working`'s field of `pass` again as it stands and writes its routing outputs (the
/// module documentation, "After the late craters"): every cell under liquid a seed at its water
/// surface, every closed depression of the dry ground a dry playa whose pit is a seed at its floor
/// ([`dry_playas`]), each cell's receiver drawn with the step's own marks, and its own runoff
/// kept.
///
/// # Panics
///
/// If a value is not finite.
pub(crate) fn route_as_it_stands(pass: &Pass<'_>, working: &mut Working) {
    let mesh = Mesh::of(pass.grid(), pass.inputs().radius());
    let marks = receiver_marks(pass.seed(), pass.grid());
    let z = elevations(working);
    let wet: Vec<bool> = working.cells.iter().map(under_sea).collect();
    let playas = dry_playas(&mesh, &z, &wet);
    let mut seeds: Vec<Seed> = (0_u32..)
        .zip(&working.cells)
        .filter(|(_, state)| under_sea(state))
        .map(|(cell, state)| Seed {
            cell,
            level_m: state.water_surface.value(),
        })
        .collect();
    seeds.extend(playas.bodies().iter().filter_map(|body| {
        body.base_level_m().map(|level_m| Seed {
            cell: body.pit,
            level_m,
        })
    }));
    seeds.sort_unstable_by_key(|seed| seed.cell);
    if seeds.is_empty() {
        // A field with no liquid and no closed depression has none: its lowest cell drains it,
        // as `route_with_water` takes it.
        let lowest = (0..mesh.len())
            .min_by(|&a, &b| z[slot(a)].total_cmp(&z[slot(b)]).then(a.cmp(&b)))
            .expect("a grid has cells");
        seeds.push(Seed {
            cell: lowest,
            level_m: z[slot(lowest)],
        });
    }
    let routing = drainage::route(&mesh, &z, &seeds, &marks);
    write_routing(working, &mesh, &routing);
}

/// The realised figures of `working`'s field of `pass` as it stands at the step's end, with the
/// craters that do not wait smoothed in, from a fresh book of its records (`budget::figures`).
fn realised_figures(pass: &Pass<'_>, working: &mut Working) -> Figures {
    let booked = budget::book(pass, working, &budget::Body::of(pass, &working.spectrum));
    budget::figures(pass, working, Phase::BeforeErosion, &booked)
}

/// Each cell's elevation in `working`, metres, each asserted finite.
#[must_use]
fn elevations(working: &Working) -> Vec<f64> {
    working
        .cells
        .iter()
        .map(|cell| num::assert_finite(cell.elevation.value()))
        .collect()
}
