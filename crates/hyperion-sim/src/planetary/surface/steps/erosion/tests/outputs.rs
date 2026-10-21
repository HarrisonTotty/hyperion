//! R09.T14.d's tests (the lane's design §5.3; decision-r09-t14-choices Q3, Q4, Q6(b), Q7 and its
//! "T14.d follow-up"): the step's re-match, sea, lakes, climate re-application and final outputs,
//! and the routing after the late craters, on the reference worlds through the rest of the pass.
//!
//! - The mean surface temperature is the input's again after the whole pass, to 10⁻⁹, on the
//!   arithmetic worlds (the small Earth; the reference Earth slow).
//! - The lakes are level, each body's cells one water surface (the small Earth; the reference
//!   Earth slow).
//! - A world wet now's realised `σ_h` after the whole pass is plan 14's to 10⁻⁴, the ruled
//!   fallback's tolerance (the small Earth; the reference Earth slow).
//! - The field's quantised catchment runoff times its drainage is the accumulation of the own
//!   runoffs times the cells' areas over the field's final routing, to the two codes' steps, and
//!   its steepness index over its drainage^θ is the slope to the receiver on the filled surface,
//!   to the steepness code's step (the small Earth and the reference Mars).
//! - On the reference Mars every eroded cell sheds the epoch's 0.1 m a⁻¹, and its catchment the
//!   same wherever all of it is older than the epoch's end; every younger cell, inside a waiting
//!   rim, renewed, or on a surface younger throughout, sheds none.
//! - Q4's (2a) and (2b) as the "T14.d follow-up" A restates them: each waiting crater's byte
//!   folded by the second match's rule, and the final elevation the eroded state plus each waiting
//!   crater's lift at its final byte, in draw order, to the bit.
//! - The routing after the late craters keeps every cell at or above its receiver and gives no
//!   runoff where none is shed; a world without the erosion step stays unrouted and books no
//!   channels.
//! - Slow, the reference Earth's whole pass: its mean, its lakes, a catchment runoff at least three
//!   times its own, T12.a's bands on the final field, the captured pits (decision-r09-t14-choices,
//!   "T14.c follow-up 2" and 3) as lakes or dry playas on the final field, its realised `σ_h` to
//!   10⁻⁴ and the pass's time.

use std::time::Instant;

use hyperion_surface::cube::Edge;
use hyperion_surface::field::{Crust, FlowDirection, LogSteepness, SynthesisCell};
use hyperion_surface::synth::craters::rims::{CoarseRims, kept_relief};
use hyperion_testkit::float::{assert_same_bits, bits};

use super::super::super::climate::seas::under_sea;
use super::super::branches::{Branch, WaterBalance, inside_waiting_rims};
use super::super::outputs::Record;
use super::super::{provisional, route_final};
use super::*;
use crate::planetary::surface::grid::parent_index;
use crate::planetary::surface::reference::{builder_of, small_earth};
use crate::planetary::surface::steps::relief::budget::{self, Rule};
use crate::units::Gigayears;

/// A world through the rest of the pass from its staging: the erosion step, as `run` runs it, with
/// its record, and the late coarse craters.
struct Through {
    staged: Staged,
    /// The state the erosion step left.
    eroded: Working,
    record: Record,
    /// The state after the late coarse craters (the classes step changes nothing).
    finished: Working,
    /// The wall times, s: the step's first part (the solve and its provisional outputs), its rest
    /// (R09.T14.d's re-match to the final outputs), and the late craters.
    seconds: (f64, f64, f64),
}

impl Through {
    /// `staged`'s world through the erosion step and the late craters.
    fn of(staged: Staged) -> Self {
        let pass = staged.pass();
        let mut eroded = staged.working.clone();
        let clock = Instant::now();
        let provisional = provisional(&pass, &mut eroded).expect("the world is eroded");
        let solve_s = clock.elapsed().as_secs_f64();
        let clock = Instant::now();
        let record = super::super::outputs::finish(&pass, &mut eroded, provisional);
        let rest_s = clock.elapsed().as_secs_f64();
        let mut finished = eroded.clone();
        let clock = Instant::now();
        craters::run_after_erosion(&pass, &mut finished);
        let late_s = clock.elapsed().as_secs_f64();
        Self {
            staged,
            eroded,
            record,
            finished,
            seconds: (solve_s, rest_s, late_s),
        }
    }

    fn pass(&self) -> Pass<'_> {
        self.staged.pass()
    }
}

/// The field cells' area-weighted mean surface temperature of `working` on `pass`'s grid, K: each
/// cell's climate cell's sea-level temperature less Γ times its water surface's height above the
/// sea level, summed in cell-index order (the climate step's mean, R10's classifier's view).
fn surface_mean_k(pass: &Pass<'_>, working: &Working) -> f64 {
    let (lapse, sea) = (working.lapse_rate_k_per_m, working.sea_level.value());
    let (mut sum, mut total) = (0.0, 0.0);
    for ((n, cell), &omega) in (0_u32..)
        .zip(&working.cells)
        .zip(pass.grid().solid_angles_sr())
    {
        let parent = slot(parent_index(n));
        let t = working.climate[parent].sea_level_temperature.value()
            - lapse * (cell.water_surface.value() - sea);
        sum += omega * t;
        total += omega;
    }
    sum / total
}

/// The mean surface temperature of `through`'s finished field is the input's to 10⁻⁹ relative,
/// printed under `what` beside the mean the climate step left.
fn assert_the_mean_holds(what: &str, through: &Through) {
    let pass = through.pass();
    let t_in = pass.inputs().mean_surface_temperature().value();
    let before = surface_mean_k(&pass, &through.staged.working);
    let mean = surface_mean_k(&pass, &through.finished);
    eprintln!(
        "{what}: the surface mean {mean:.9} K after the whole pass, against the input's {t_in} K \
         ({before:.9} K after the climate step)"
    );
    assert!(
        (mean - t_in).abs() <= 1e-9 * t_in,
        "{what}: the surface mean {mean} K is not the input's {t_in} K"
    );
}

/// The lakes of `through`'s final field are level and its liquids placed (decision-r09-t14-choices
/// Q3): each water body that floods a cell is a lake whose every cell lies under its level, one
/// water surface to the bit, naming a liquid; every connected run of flooded cells off the ocean
/// holds one water surface; and no cell off the ocean and the lakes is flooded. The counts of
/// lakes (terminal and full), dry playas and lake cells, printed under `what`.
fn assert_lakes_are_level(what: &str, through: &Through) -> (usize, usize, usize) {
    let pass = through.pass();
    let working = &through.eroded;
    let record = &through.record;
    let mut in_lake = vec![false; working.cells.len()];
    let (mut terminal, mut full, mut playas, mut cells) = (0, 0, 0, 0);
    for body in record.bodies.bodies() {
        if body.cells.is_empty() {
            playas += usize::from(!body.full);
            continue;
        }
        if body.full {
            full += 1;
        } else {
            terminal += 1;
        }
        for &c in &body.cells {
            let cell = &working.cells[slot(c)];
            assert_same_bits(cell.water_surface.value(), body.level_m);
            assert!(under_sea(cell), "{what}: lake cell {c} is under its level");
            assert!(
                cell.liquid_entry.is_some(),
                "{what}: lake cell {c} names a liquid"
            );
            in_lake[slot(c)] = true;
            cells += 1;
        }
    }
    let grid = pass.grid();
    let mut seen = vec![false; working.cells.len()];
    for start in 0..grid.len() {
        let s = slot(start);
        let cell = &working.cells[s];
        if !under_sea(cell) || record.ocean[s] || seen[s] {
            assert!(
                !under_sea(cell) || record.ocean[s] || in_lake[s],
                "{what}: cell {start} is flooded off the ocean and the lakes"
            );
            continue;
        }
        assert!(in_lake[s], "{what}: cell {start} is flooded off the lakes");
        let level = cell.water_surface.value();
        seen[s] = true;
        let mut stack = vec![start];
        while let Some(c) = stack.pop() {
            for m in grid.edge_neighbours(c) {
                let state = &working.cells[slot(m)];
                if under_sea(state) && !record.ocean[slot(m)] && !seen[slot(m)] {
                    seen[slot(m)] = true;
                    assert_same_bits(state.water_surface.value(), level);
                    stack.push(m);
                }
            }
        }
    }
    let omega = grid.solid_angles_sr();
    let (mut lake_sr, mut land_sr, mut total_sr) = (0.0, 0.0, 0.0);
    for (c, &w) in omega.iter().enumerate() {
        total_sr += w;
        if !record.ocean[c] {
            land_sr += w;
            if in_lake[c] {
                lake_sr += w;
            }
        }
    }
    let z0: Vec<f64> = through
        .staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let (_, _, pits) = not_above_receivers(&record.outcome, &z0);
    eprintln!(
        "{what}: {terminal} terminal and {full} full lakes over {cells} cells, {playas} dry \
         playas, {} held pits in the solve; the lakes cover {:.4} of the sphere, {:.4} of the land \
         off the ocean ({:.4} of the sphere), against Earth's about 0.037 (Verpoorter et al. \
         2014, GRL 41, 6396), recorded",
        pits.len(),
        lake_sr / total_sr,
        lake_sr / land_sr,
        land_sr / total_sr
    );
    print_land_hypsometry(what, through);
    (terminal + full, playas, cells)
}

/// Prints under `what` the land's area quantiles above the sea on `through`'s final field (off its
/// ocean) against the construction's (off the sea the climate step left): p1, p10, p50, p90, p99
/// and the greatest, metres (decision-r09-t14-choices, "T14.d lakes" item 3).
fn print_land_hypsometry(what: &str, through: &Through) {
    let pass = through.pass();
    let omega = pass.grid().solid_angles_sr();
    let row = |working: &Working, land: &dyn Fn(usize) -> bool| -> String {
        let sea = working.sea_level.value();
        let mut cells: Vec<(f64, f64)> = working
            .cells
            .iter()
            .enumerate()
            .filter(|&(c, _)| land(c))
            .map(|(c, cell)| (cell.elevation.value() - sea, omega[c]))
            .collect();
        cells.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let total: f64 = cells.iter().map(|x| x.1).sum();
        [0.01, 0.1, 0.5, 0.9, 0.99, 1.0]
            .iter()
            .map(|&q| {
                let mut held = 0.0;
                let at = cells
                    .iter()
                    .find(|x| {
                        held += x.1;
                        held >= q * total
                    })
                    .or(cells.last())
                    .map_or(0.0, |x| x.0);
                format!("{at:.0}")
            })
            .collect::<Vec<_>>()
            .join(" / ")
    };
    let construction = &through.staged.working;
    eprintln!(
        "{what}: the land above the sea by area, p1 / p10 / p50 / p90 / p99 / max: {} m on the \
         final field against {} m in the construction",
        row(&through.finished, &|c| !through.record.ocean[c]),
        row(construction, &|c| !under_sea(&construction.cells[c]))
    );
}

/// The cell that `cell`'s flow `flow` drains into on `graph`, or `None` where it is terminal.
fn receiver_of(graph: &CellGraph, cell: u32, flow: FlowDirection) -> Option<u32> {
    let edge = flow.edge()?;
    let k = Edge::ALL
        .iter()
        .position(|&e| e == edge)
        .expect("a flow's edge is one of the four");
    Some(graph.edge_neighbours(cell)[k])
}

/// `working`'s field's routing as its flows give it on `pass`'s grid: each cell's receiver, and
/// the cells in an order with every donor before its receiver (Kahn's, from the cells no other
/// drains into, in ascending order), which panics on a cycle.
fn field_routing(pass: &Pass<'_>, working: &Working) -> (Vec<Option<u32>>, Vec<u32>) {
    let grid = pass.grid();
    let receivers: Vec<Option<u32>> = (0_u32..)
        .zip(&working.cells)
        .map(|(c, cell)| receiver_of(grid, c, cell.flow))
        .collect();
    let mut donors = vec![0_u32; receivers.len()];
    for r in receivers.iter().flatten() {
        donors[slot(*r)] += 1;
    }
    let mut ready: Vec<u32> = (0_u32..)
        .zip(&donors)
        .filter(|&(_, &d)| d == 0)
        .map(|(c, _)| c)
        .collect();
    ready.reverse();
    let mut order = Vec::with_capacity(receivers.len());
    while let Some(c) = ready.pop() {
        order.push(c);
        if let Some(r) = receivers[slot(c)] {
            donors[slot(r)] -= 1;
            if donors[slot(r)] == 0 {
                ready.push(r);
            }
        }
    }
    assert_eq!(
        order.len(),
        receivers.len(),
        "the field's flows hold no cycle"
    );
    (receivers, order)
}

/// Each cell's sum of `weights` over its catchment on `working`'s field's routing
/// ([`field_routing`]), its own included.
fn accumulated(pass: &Pass<'_>, working: &Working, weights: &[f64]) -> Vec<f64> {
    let (receivers, order) = field_routing(pass, working);
    let mut sum = weights.to_vec();
    for &c in &order {
        if let Some(r) = receivers[slot(c)] {
            sum[slot(r)] += sum[slot(c)];
        }
    }
    sum
}

/// The relative half-steps of the drainage, runoff and steepness codes: 2^(1/2048) − 1,
/// 2^(1/24) − 1 and 2^(1/16) − 1 (`LogArea`'s 1,024 codes an octave, `LogRunoff`'s 12 and
/// `LogSteepness`'s 8).
fn half_steps() -> (f64, f64, f64) {
    (
        math::powf(2.0, 1.0 / 2048.0) - 1.0,
        math::powf(2.0, 1.0 / 24.0) - 1.0,
        math::powf(2.0, 1.0 / 16.0) - 1.0,
    )
}

/// On `through`'s final field, quantised (`budget::records`), printed under `what`:
///
/// - each cell's drainage code is the cells' areas accumulated over the field's flows, its own
///   included, to the code's half-step;
/// - its catchment runoff code times its drainage code is the own runoffs times the areas
///   accumulated over those flows (the equivalent area times 1 m a⁻¹, Hergarten 2021, eq. 20), to
///   the two codes' half-steps, or below the runoff code's first step where the code is none;
/// - its steepness code over its drainage code^θ is the slope to its receiver on the filled
///   surface, the drop of the water surfaces over the link, to the steepness code's half-step and
///   the drainage code's, wherever the drop is at least 0.1 m (the filled surface is the water
///   surface but by the shores, under half a millimetre) and the code is above none; and a
///   terminal cell's steepness is none.
fn assert_the_codes_hold(what: &str, through: &Through) {
    let pass = through.pass();
    let working = &through.finished;
    let grid = pass.grid();
    let r = pass.inputs().radius().value();
    let area: Vec<f64> = grid
        .solid_angles_sr()
        .iter()
        .map(|&w| w * (r * r))
        .collect();
    let discharge: Vec<f64> = working
        .cells
        .iter()
        .zip(&area)
        .map(|(cell, &a)| cell.runoff_m_per_year * a)
        .collect();
    let drainage = accumulated(&pass, working, &area);
    let flux = accumulated(&pass, working, &discharge);
    let (area_step, runoff_step, steep_step) = half_steps();
    let records: Vec<SynthesisCell> = budget::records(working).collect();
    let (mut worst_area, mut worst_flux, mut worst_slope) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut slopes = 0_usize;
    for (n, record) in (0_u32..).zip(&records) {
        let c = slot(n);
        let a = record.drainage.area().value();
        let error = (a / drainage[c] - 1.0).abs();
        worst_area = worst_area.max(error);
        assert!(
            error <= area_step + 1e-9,
            "{what}: cell {n}'s drainage {a} m² against {} m²",
            drainage[c]
        );
        let q = record.catchment_runoff.metres_per_year();
        if record.catchment_runoff.get() == 0 {
            assert!(
                flux[c] / drainage[c] < 1e-4 * (1.0 + runoff_step),
                "{what}: cell {n}'s catchment runoff is none, but {} m a⁻¹",
                flux[c] / drainage[c]
            );
        } else {
            let error = (q * a / flux[c] - 1.0).abs();
            worst_flux = worst_flux.max(error);
            assert!(
                error <= (1.0 + runoff_step) * (1.0 + area_step) - 1.0 + 1e-9,
                "{what}: cell {n}'s catchment runoff × drainage {} m³ a⁻¹ against {} m³ a⁻¹",
                q * a,
                flux[c]
            );
        }
        let Some(receiver) = receiver_of(grid, n, working.cells[c].flow) else {
            assert_eq!(
                record.steepness.get(),
                0,
                "{what}: terminal cell {n}'s steepness"
            );
            continue;
        };
        let drop = working.cells[c].water_surface.value()
            - working.cells[slot(receiver)].water_surface.value();
        if drop < 0.1 || record.steepness.get() == 0 {
            continue;
        }
        let slope = drop / (grid.arc(n, receiver).value() * r);
        let coded = record.steepness.index_m0_9() * math::powf(a, -LogSteepness::THETA);
        let error = (coded / slope - 1.0).abs();
        worst_slope = worst_slope.max(error);
        slopes += 1;
        assert!(
            error <= (1.0 + steep_step) * (1.0 + area_step) * 1.005 - 1.0,
            "{what}: cell {n}'s k_s A^-θ {coded} against its slope {slope}"
        );
    }
    eprintln!(
        "{what}: over {} cells, the drainage within {worst_area:.2e}, the catchment flux within \
         {worst_flux:.4} and, at {slopes} cells, the slope within {worst_slope:.4} of their codes",
        records.len()
    );
    assert!(slopes > 0, "{what}: some cell's slope is compared");
}

/// The bound on a world wet now's realised `σ_h` after the whole pass against plan 14's,
/// relative: 10⁻⁴, the pre-ruled fallback's (decision-r09-t14-choices, "T14.d follow-up" C). The
/// re-match books the channels' share from the provisional records, which the new sea, the lakes
/// and the final routing then re-route, so the whole pass misses T12.e's 10⁻⁶ (a finding, plan
/// 09's R09.T14 records).
const WET_NOW_SIGMA_H_TOLERANCE: f64 = 1e-4;

/// The mean surface temperature is the input's again after the whole pass, to 10⁻⁹, and the
/// lakes are level, on the small Earth (decision-r09-t14-choices Q6(b) and Q3; the reference
/// Earth's are the slow [`the_reference_earth_s_final_field_holds_r09_t14d`]'s). Its realised
/// `σ_h` is plan 14's to [`WET_NOW_SIGMA_H_TOLERANCE`], relative.
#[test]
fn the_small_earth_s_mean_and_lakes_hold_after_the_whole_pass() {
    let through = Through::of(Staged::of(small_earth()));
    assert_eq!(through.record.branch, Branch::WetNow);
    assert_the_mean_holds("small Earth", &through);
    let (lakes, playas, cells) = assert_lakes_are_level("small Earth", &through);
    eprintln!("small Earth: {lakes} lakes, {playas} dry playas, {cells} lake cells");
    let miss = print_sigma_h("small Earth", &through);
    assert!(
        miss.abs() <= WET_NOW_SIGMA_H_TOLERANCE,
        "the small Earth's realised σ_h misses plan 14's by {miss:.3e}, relative"
    );
}

/// Prints `through`'s realised `σ_h` after the whole pass against plan 14's, and its relative
/// miss, under `what`, with the channels' share `C_c` the re-match booked against the one the
/// state the step left and the finished state book; the relative miss, realised ÷ plan 14's − 1.
fn print_sigma_h(what: &str, through: &Through) -> f64 {
    let pass = through.pass();
    let target = pass.inputs().sigma_h().value();
    let realised = through.finished.realised_sigma_h.value();
    let book = |working: &Working| {
        budget::book(&pass, working, &budget::Body::of(&pass, &working.spectrum)).channels_m2
    };
    eprintln!(
        "{what}: C_c booked by the re-match {:.1} m², on the field the step left {:.1} m², after \
         the late craters {:.1} m²",
        through.record.matched.booked.channels_m2,
        book(&through.eroded),
        book(&through.finished)
    );
    eprintln!(
        "{what}: realised σ_h {realised:.6} m against plan 14's {target:.6} m, Δ {:.3e} relative; \
         the re-match's rule {:?}, s* {:?}, s_N {:.9}",
        realised / target - 1.0,
        through.record.matched.rule,
        through.record.matched.whole_scale,
        through.record.matched.non_crater_scale
    );
    realised / target - 1.0
}

/// The field's codes hold on the final field ([`assert_the_codes_hold`]): its catchment runoff
/// over its final routing (Hergarten 2021, eq. 20; decision-r09-t14-choices Q7, "over the field's
/// final routing") and its steepness index on the filled surface, on the small Earth and on the
/// reference Mars after its late craters.
#[test]
fn the_catchment_runoff_and_the_steepness_are_the_final_routing_s() {
    for (what, inputs) in [
        ("small Earth", small_earth()),
        ("reference Mars", reference::mars_like()),
    ] {
        assert_the_codes_hold(what, &Through::of(Staged::of(inputs)));
    }
}

/// On the reference Mars after the whole pass (decision-r09-t14-choices Q4): every cell older than
/// the epoch's end, outside a waiting crater's rim and off the renewed provinces, sheds the
/// epoch's 0.1 m a⁻¹, to the bit, and every younger cell none; and wherever a cell's whole
/// catchment on the final routing is older, its catchment runoff is 0.1 m a⁻¹ to 10⁻¹². On a
/// Mars whose surface is no older than its epoch's end (3.5 Ga), no cell sheds anything after the
/// whole pass, and none drains.
#[test]
fn the_reference_mars_s_runoffs_are_its_epoch_s() {
    let through = Through::of(Staged::of(reference::mars_like()));
    let pass = through.pass();
    let young = inside_waiting_rims(&pass, &through.staged.working);
    let working = &through.finished;
    let old: Vec<bool> = working
        .cells
        .iter()
        .zip(&young)
        .map(|(cell, &late)| !late && cell.crust != Crust::Province)
        .collect();
    for (c, (cell, &old)) in working.cells.iter().zip(&old).enumerate() {
        let expected = if old { 0.1 } else { 0.0 };
        assert!(
            cell.runoff_m_per_year.total_cmp(&expected).is_eq(),
            "cell {c}: its own runoff {} m a⁻¹, not {expected}",
            cell.runoff_m_per_year
        );
    }
    let (receivers, order) = field_routing(&pass, working);
    let mut whole = old.clone();
    for &c in &order {
        if let Some(r) = receivers[slot(c)] {
            let upstream = whole[slot(c)];
            whole[slot(r)] &= upstream;
        }
    }
    let mut held = 0_usize;
    for (c, cell) in working.cells.iter().enumerate() {
        if whole[c] {
            held += 1;
            assert!(
                (cell.catchment_runoff_m_per_year / 0.1 - 1.0).abs() <= 1e-12,
                "cell {c}: its catchment runoff {} m a⁻¹ over an old catchment",
                cell.catchment_runoff_m_per_year
            );
        }
    }
    let younger = old.iter().filter(|&&o| !o).count();
    eprintln!(
        "reference Mars: {younger} cells younger than the epoch's end; {held} of {} cells drain an \
         old catchment whole",
        old.len()
    );
    assert!(younger > 0 && held > 0);

    let young_world = builder_of(&reference::mars_like())
        .surface_age(Gigayears::new(3.5))
        .build()
        .unwrap();
    let staged = Staged::of(young_world);
    let finished = staged.finished();
    assert!(!finished.routed);
    for cell in &finished.cells {
        assert_same_bits(cell.runoff_m_per_year, 0.0);
        assert_same_bits(cell.catchment_runoff_m_per_year, 0.0);
        assert_eq!(cell.flow, FlowDirection::Terminal);
    }
}

/// Q4's (2a) and (2b) on the reference Mars, as decision-r09-t14-choices' "T14.d follow-up" A
/// restates them: (2a) each crater's byte after the step, the waiting ones among them, is the
/// erosion-off run's byte under the second match's rule, `byte_of`(s\*₂ × `kept_relief`(byte₁))
/// where that rule is [`Rule::Whole`] and unchanged otherwise; and (2b) the final elevation is,
/// bit for bit, the state the step left plus each waiting crater's lift at its final byte
/// (`craters::each_lift`), added in draw order, so that neither the step nor the routing after the
/// late craters touches them.
#[test]
fn the_waiting_craters_are_folded_by_the_second_match_and_placed_untouched() {
    let through = Through::of(Staged::of(reference::mars_like()));
    let pass = through.pass();
    let matched = &through.record.matched;
    let before = &through.staged.working.craters;
    let after = &through.eroded.craters;
    assert_eq!(before.len(), after.len());
    let waiting = before
        .iter()
        .filter(|c| craters::waits(&pass, c.age))
        .count();
    eprintln!(
        "reference Mars: {} craters, {waiting} waiting; the second match's rule {:?}, s* {:?}",
        before.len(),
        matched.rule,
        matched.whole_scale
    );
    assert!(waiting > 0);
    for (n, (one, two)) in before.iter().zip(after).enumerate() {
        let expected = match matched.rule {
            Rule::Whole => budget::byte_of(
                matched.whole_scale.expect("a whole scale was solved") * kept_relief(one),
            ),
            Rule::NonCrater | Rule::Unscaled => one.degradation,
        };
        assert_eq!(two.degradation, expected, "crater {n}'s byte");
    }
    let inputs = pass.inputs();
    let rims = CoarseRims::for_body(*inputs.craters(), inputs.figure(), pass.level());
    let mut z: Vec<f64> = through
        .eroded
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    for crater in through
        .finished
        .craters
        .iter()
        .filter(|c| craters::waits(&pass, c.age))
    {
        craters::each_lift(
            pass.grid(),
            &inputs.figure(),
            &rims,
            crater,
            |cell, lift| {
                z[slot(cell)] += lift;
            },
        );
    }
    let mut moved = 0_usize;
    for (c, (cell, &expected)) in through.finished.cells.iter().zip(&z).enumerate() {
        assert_same_bits(cell.elevation.value(), expected);
        moved += usize::from(
            expected
                .total_cmp(&through.eroded.cells[c].elevation.value())
                .is_ne(),
        );
    }
    eprintln!("reference Mars: {moved} cells lifted by the waiting craters");
    assert!(moved > 0);
    let _miss = print_sigma_h("reference Mars", &through);
}

/// The routing after the late craters (`route_final`; decision-r09-t14-choices Q7's addition): on
/// the reference Mars, every cell that drains lies at or above its receiver on the final field,
/// dry throughout; the late craters change some cell's flow; routing the field again as it
/// stands changes nothing; and with every own runoff zero, as on a body with no wet epoch (a
/// pressure of zero), it gives no catchment runoff.
#[test]
fn the_routing_after_the_late_craters_keeps_every_cell_above_its_receiver() {
    let through = Through::of(Staged::of(reference::mars_like()));
    let pass = through.pass();
    let working = &through.finished;
    assert!(working.routed);
    let (receivers, _) = field_routing(&pass, working);
    let mut draining = 0_usize;
    for (c, receiver) in receivers.iter().enumerate() {
        let cell = &working.cells[c];
        assert!(!under_sea(cell), "the reference Mars is dry");
        if let Some(r) = receiver {
            draining += 1;
            let below = working.cells[slot(*r)].elevation.value();
            assert!(
                cell.elevation.value() >= below,
                "cell {c} at {} m drains up to {r} at {below} m",
                cell.elevation.value()
            );
        }
    }
    let changed = working
        .cells
        .iter()
        .zip(&through.eroded.cells)
        .filter(|(a, b)| a.flow != b.flow)
        .count();
    eprintln!(
        "reference Mars: {draining} of {} cells drain after the late craters; {changed} flows \
         changed by them",
        receivers.len()
    );
    assert!(draining > 0 && changed > 0);
    let mut dry = working.clone();
    for cell in &mut dry.cells {
        cell.runoff_m_per_year = 0.0;
    }
    route_final(&pass, &mut dry);
    for cell in &dry.cells {
        assert_same_bits(cell.catchment_runoff_m_per_year, 0.0);
    }
    // The routing as it stands is the late craters' own: every field `route_final` writes, by
    // bits.
    let mut again = working.clone();
    route_final(&pass, &mut again);
    for (c, (a, b)) in again.cells.iter().zip(&working.cells).enumerate() {
        assert_eq!(a.flow, b.flow, "cell {c}'s flow");
        assert_eq!(
            bits(a.drainage.value()),
            bits(b.drainage.value()),
            "cell {c}'s drainage"
        );
        assert_eq!(
            bits(a.steepness_index_m0_9),
            bits(b.steepness_index_m0_9),
            "cell {c}'s steepness index"
        );
        assert_eq!(
            bits(a.catchment_runoff_m_per_year),
            bits(b.catchment_runoff_m_per_year),
            "cell {c}'s catchment runoff"
        );
    }
}

/// A world without the erosion step (as the craters' own tests' `cratered` stages it: the coarse
/// craters on the initial state, then those that waited) stays unrouted after the late craters,
/// every cell terminal with no runoff, and books no channels, `C_c` = 0 against the match's 0
/// (decision-r09-t14-choices, "T14.d follow-up" C); and the reference Moon and Ceres, bodies with
/// no atmosphere, carry no runoff after the whole pass.
#[test]
fn a_world_without_the_erosion_step_books_no_channels() {
    let inputs = reference::mars_like();
    let level = coarse_level(inputs.radius());
    let (grid, climate_grid) = (CellGraph::for_field(level), CellGraph::for_climate(level));
    let pass = Pass::new(reference::SEED, &inputs, &grid, &climate_grid);
    let mut working = Working::initial(&pass);
    craters::run(&pass, &mut working);
    craters::run_after_erosion(&pass, &mut working);
    assert!(!working.routed);
    for cell in &working.cells {
        assert_eq!(cell.flow, FlowDirection::Terminal);
        assert_same_bits(cell.runoff_m_per_year, 0.0);
    }
    let booked = budget::book(&pass, &working, &budget::Body::of(&pass, &working.spectrum));
    assert_same_bits(booked.channels_m2, 0.0);
    for (what, inputs) in [
        ("reference Moon", reference::moon_like()),
        ("reference Ceres", reference::ceres_like()),
    ] {
        assert_same_bits(inputs.surface_pressure().value(), 0.0);
        let staged = Staged::of(inputs);
        let finished = staged.finished();
        assert!(!finished.routed, "{what}");
        for cell in &finished.cells {
            assert_same_bits(cell.runoff_m_per_year, 0.0);
            assert_same_bits(cell.catchment_runoff_m_per_year, 0.0);
        }
    }
}

/// What each captured pit of `through`'s solve became on the final field (decision-r09-t14-choices,
/// "T14.c follow-up 2" and 3: a zero-uplift cell held at its construction below its receiver,
/// [`not_above_receivers`]'s pits, a closed depression that T14.d's balance lakes take): under a
/// lake, a dry playa's floor, draining into a lake, draining to a dry playa's floor, or draining to
/// the sea; the five counts, printed under `what` with the first eight pits.
fn captured_pits(what: &str, through: &Through) -> [usize; 5] {
    let pass = through.pass();
    let working = &through.finished;
    let record = &through.record;
    let z0: Vec<f64> = through
        .staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let (_, _, pits) = not_above_receivers(&record.outcome, &z0);
    let mut floors: Vec<u32> = record
        .bodies
        .bodies()
        .iter()
        .filter(|b| b.is_playa())
        .map(|b| b.pit)
        .collect();
    floors.sort_unstable();
    let lake = |c: u32| {
        let s = slot(c);
        under_sea(&working.cells[s]) && !record.ocean[s]
    };
    let (receivers, _) = field_routing(&pass, working);
    let mut counts = [0_usize; 5];
    for (k, &pit) in pits.iter().enumerate() {
        let mut root = pit;
        while let Some(r) = receivers[slot(root)] {
            root = r;
        }
        let fate = if lake(pit) {
            0
        } else if floors.binary_search(&pit).is_ok() {
            1
        } else if lake(root) {
            2
        } else if floors.binary_search(&root).is_ok() {
            3
        } else {
            4
        };
        counts[fate] += 1;
        if k < 8 {
            let cell = &working.cells[slot(pit)];
            eprintln!(
                "{what}: pit {pit} ends at {:.1} m (water {:.1} m), its root {root}: {}",
                cell.elevation.value(),
                cell.water_surface.value(),
                [
                    "under a lake",
                    "a dry playa's floor",
                    "draining into a lake",
                    "draining to a dry playa",
                    "draining to the sea"
                ][fate]
            );
        }
    }
    eprintln!(
        "{what}: of {} captured pits, {} under a lake, {} a dry playa's floor, {} draining into a \
         lake, {} draining to a dry playa, {} draining to the sea",
        pits.len(),
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        counts[4]
    );
    counts
}

/// The reference Earth's whole pass at level 8 (slow): the mean surface temperature is the
/// input's after it, to 10⁻⁹ (Q6(b)); its lakes are level (Q3); some cell's catchment runoff is
/// at least three times its own; T12.a's bands hold on the final field; its realised `σ_h` is plan
/// 14's to [`WET_NOW_SIGMA_H_TOLERANCE`], relative, and recorded; and the captured pits' fates on
/// the final field, the lakes' counts and the pass's time are recorded.
#[test]
#[ignore = "slow: the reference Earth's whole coarse pass at level 8, R09.T14.d"]
fn the_reference_earth_s_final_field_holds_r09_t14d() {
    let clock = Instant::now();
    let staged = Staged::of(reference::earth_like());
    let staging_s = clock.elapsed().as_secs_f64();
    let through = Through::of(staged);
    let pass = through.pass();
    let (solve_s, rest_s, late_s) = through.seconds;
    eprintln!(
        "reference Earth: the whole pass in {:.2} s: the steps to the climate's end {staging_s:.2} \
         s, the erosion step {:.2} s (the solve and its provisional outputs {solve_s:.2} s, the \
         re-match to the final outputs {rest_s:.2} s), the late craters {late_s:.2} s",
        staging_s + solve_s + rest_s + late_s,
        solve_s + rest_s
    );
    let clock = Instant::now();
    let runoff: Vec<f64> = through
        .staged
        .working
        .cells
        .iter()
        .map(|c| c.runoff_m_per_year)
        .collect();
    let _balance = WaterBalance::of(&pass, &through.staged.working, &runoff);
    eprintln!(
        "reference Earth: one water balance of its cells (the net loss at every cell) in {:.2} s",
        clock.elapsed().as_secs_f64()
    );
    assert_eq!(through.record.branch, Branch::WetNow);
    assert_the_mean_holds("reference Earth", &through);
    let _counts = assert_lakes_are_level("reference Earth", &through);
    let _fates = captured_pits("reference Earth", &through);
    let miss = print_sigma_h("reference Earth", &through);
    assert!(
        miss.abs() <= WET_NOW_SIGMA_H_TOLERANCE,
        "the reference Earth's realised σ_h misses plan 14's by {miss:.3e}, relative"
    );
    let (mut most, mut at) = (0.0_f64, 0_usize);
    let mut thrice = 0_usize;
    for (c, cell) in through.finished.cells.iter().enumerate() {
        if cell.runoff_m_per_year > 0.0 {
            let ratio = cell.catchment_runoff_m_per_year / cell.runoff_m_per_year;
            thrice += usize::from(ratio >= 3.0);
            if ratio > most {
                (most, at) = (ratio, c);
            }
        }
    }
    eprintln!(
        "reference Earth: {thrice} cells' catchment runoff at least three times their own, the \
         most {most:.1} times at cell {at}"
    );
    assert!(thrice > 0);
    assert_earth_bands("reference Earth, final field", &pass, &through.finished);
}
