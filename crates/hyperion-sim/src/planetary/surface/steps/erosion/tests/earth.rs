//! The reference Earth's erosion, wet now (R09.T14.c; the lane's design §4.5 test 6,
//! decision-r09-t14-choices Q1, the "T14.b follow-up" item 5, the "T14.c follow-up" items 1–3 and
//! the "T14.c follow-up 2" and 3): the plan's restated fixed-point test on the reference Earth at
//! level 8, with the uplift as a field held by the additive refit, the threshold slope, sticky
//! receivers, the remap onto the construction's absolute hypsometry, the assertions on the final
//! field, its records and the ruling's triggers, slow.

use std::collections::BTreeMap;
use std::time::Instant;

use hyperion_surface::num;

use super::super::super::CellState;
use super::super::branches::{
    Remapped, erode_wet_now_with, remap_onto_construction, sea_cells, wet_now_runoff_m_per_year,
    wet_now_threshold,
};
use super::super::erodibility::Fluid;
use super::super::multigrid::{Options, Outcome, Receivers};
use super::super::outputs::{self, Written};
use super::super::solver::Threshold;
use super::*;

/// The ruling's trigger on the final RMS residual of the solve, m: 125, 5% of the reference
/// Earth's `σ_h` of 2.51 km (decision-r09-t14-choices, "T14.b follow-up" item 5).
const RESIDUAL_TRIGGER_M: f64 = 125.0;

/// The steepest gradient between two land neighbours of the final field above which the edge is
/// counted: 0.1, the threshold slope, which bounds the links but not the edges across a divide
/// (decision-r09-t14-choices, "T14.c follow-up 3" item T; it replaces the largest change's 1 km
/// as the trigger, the largest change still recorded).
const EDGE_GRADIENT_COUNTED: f64 = DEBRIS_FLOW_SLOPE;

/// The steepest gradient between two land neighbours of the final field above which it is a
/// finding to main: 0.2, twice the threshold, about 7 km across one cell (the follow-up's item T).
const EDGE_GRADIENT_FINDING: f64 = 0.2;

/// The land's mode above the sea, m: T12.a's +125 m, Earth2014's, a finding outside 300 m of it
/// (decision-r09-t12-coarse item 1; decision-r09-t14-choices Q1's records).
const LAND_MODE_M: (f64, f64) = (125.0, 300.0);

/// The modal 250 m bin's centre, metres, of `heights` (each above the sea) weighted by `weights`,
/// among the bins at −2 km and above (the land's, as T12.a's test takes them).
fn land_mode(heights: &[f64], weights: &[f64]) -> f64 {
    let mut bins: BTreeMap<i64, f64> = BTreeMap::new();
    for (&z, &w) in heights.iter().zip(weights) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a coarse elevation's 250 m bin is within an i64"
        )]
        let k = (z / 250.0).floor() as i64;
        *bins.entry(k).or_insert(0.0) += w;
    }
    let (k, _) =
        bins.iter()
            .filter(|(k, _)| **k >= -8)
            .fold((0, f64::NEG_INFINITY), |best, (&k, &w)| {
                if w > best.1 { (k, w) } else { best }
            });
    #[expect(clippy::cast_precision_loss, reason = "a bin index is small")]
    let centre = (k as f64 + 0.5) * 250.0;
    centre
}

/// The largest change of `outcome`'s final remapped field `remapped` against the remap of the
/// iterate its last solve started from (the solve less its last change), over the cells remapped,
/// metres (decision-r09-t14-choices, "T14.c follow-up 2" §3's trigger).
fn remapped_largest_change_m(outcome: &Outcome, z0: &[f64], remapped: &Remapped) -> f64 {
    let previous: Vec<f64> = outcome
        .z_m
        .iter()
        .zip(&outcome.last_change_m)
        .map(|(&s, &d)| s - d)
        .collect();
    let before = remap_onto_construction(outcome, z0, &previous);
    remapped
        .moved
        .iter()
        .enumerate()
        .filter(|&(_, &moved)| moved)
        .fold(0.0, |hi, (c, _)| {
            num::max(hi, (remapped.elevation_m[c] - before.elevation_m[c]).abs())
        })
}

/// The steepest gradient between two edge neighbours that are both land (off `sea`) on `working`'s
/// field of `pass`, each pair once: the gradient, the pair, and the count of pairs steeper than
/// [`EDGE_GRADIENT_COUNTED`] (decision-r09-t14-choices, "T14.c follow-up 3" item T).
fn steepest_edge(pass: &Pass<'_>, working: &Working, sea: &[bool]) -> (f64, (u32, u32), usize) {
    let grid = pass.grid();
    let r = pass.inputs().radius().value();
    let (mut steepest, mut pair, mut counted) = (0.0_f64, (0, 0), 0_usize);
    for c in 0..grid.len() {
        if sea[slot(c)] {
            continue;
        }
        for m in grid.edge_neighbours(c) {
            if m <= c || sea[slot(m)] {
                continue;
            }
            let rise = (working.cells[slot(c)].elevation.value()
                - working.cells[slot(m)].elevation.value())
            .abs();
            let gradient = rise / (grid.arc(c, m).value() * r);
            counted += usize::from(gradient > EDGE_GRADIENT_COUNTED);
            if gradient > steepest {
                (steepest, pair) = (gradient, (c, m));
            }
        }
    }
    (steepest, pair, counted)
}

/// The ruling's triggers (decision-r09-t14-choices, "T14.c follow-up 2" §3 as its "T14.c
/// follow-up 3" item T restates them), findings and not blockers: the solve's final RMS residual
/// above 125 m, the final field's steepest edge between land neighbours above 0.2
/// ([`steepest_edge`]; the largest change is recorded, no longer a trigger) and a level failing
/// the within-level fall; each tripped, as text.
fn triggers(outcome: &Outcome, steepest_edge: f64) -> Vec<String> {
    let residuals = &outcome.diagnostics.residuals;
    let finest = residuals.last().expect("a level");
    let mut findings = Vec::new();
    if finest.last_m() > RESIDUAL_TRIGGER_M {
        findings.push(format!(
            "final residual {:.1} m above 125 m",
            finest.last_m()
        ));
    }
    if steepest_edge > EDGE_GRADIENT_FINDING {
        findings.push(format!(
            "the final field's steepest edge between land neighbours {steepest_edge:.4} above 0.2"
        ));
    }
    for r in residuals {
        if r.last_m() >= WITHIN_LEVEL_FALL * r.first_m() {
            findings.push(format!(
                "level {} falls from {:.1} m only to {:.1} m",
                r.level,
                r.first_m(),
                r.last_m()
            ));
        }
    }
    findings
}

/// Prints `outcome`'s records under `what` (the reference Earth's steady state, `staged` its
/// construction, `working` the state the step left with the remap `remapped`): each level's, the
/// triggers, the final field's steepest edge between land neighbours and the edges above 0.1, the
/// raw solve's largest change beside the remapped field's, the implied denudation, the land's mode
/// and heights against the construction's, the remap's ([`remap_records`]) and [`print_raised`]'s.
fn record(what: &str, staged: &Staged, working: &Working, outcome: &Outcome, remapped: &Remapped) {
    report(what, outcome);
    let z0: Vec<f64> = staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let remapped_change_m = remapped_largest_change_m(outcome, &z0, remapped);
    let (edge, (a, b), steep_edges) =
        steepest_edge(&staged.pass(), working, &sea_cells(&staged.working));
    eprintln!(
        "{what}: {}the final field's steepest edge between land neighbours {edge:.4}, cells {a} \
         and {b}; {steep_edges} edges steeper than 0.1",
        if edge > EDGE_GRADIENT_FINDING {
            "FINDING, above 0.2: "
        } else {
            ""
        }
    );
    let findings = triggers(outcome, edge);
    eprintln!(
        "{what}: triggers: {}",
        if findings.is_empty() {
            "none trip".to_owned()
        } else {
            format!("FINDING: {}", findings.join("; "))
        }
    );
    eprintln!(
        "{what}: the largest change {remapped_change_m:.1} m on the remapped field against the \
         previous iterate's remap, {:.1} m on the raw solve",
        outcome
            .diagnostics
            .residuals
            .last()
            .expect("a level")
            .largest_change_m()
    );
    eprintln!(
        "{what}: implied mean denudation over the land {:.4} mm a⁻¹ against Earth's 0.05–0.1",
        1_000.0 * outcome.diagnostics.mean_uplift_m_per_year
    );
    let pass = staged.pass();
    let omega = pass.grid().solid_angles_sr();
    let sea = sea_cells(&staged.working);
    let sea_level_m = staged.working.sea_level.value();
    let land = |cells: &[CellState]| -> Vec<f64> {
        cells
            .iter()
            .zip(&sea)
            .filter(|&(_, &wet)| !wet)
            .map(|(cell, _)| cell.elevation.value() - sea_level_m)
            .collect()
    };
    let weights: Vec<f64> = omega
        .iter()
        .zip(&sea)
        .filter(|&(_, &wet)| !wet)
        .map(|(&w, _)| w)
        .collect();
    let (after, before) = (land(&working.cells), land(&staged.working.cells));
    let mode = land_mode(&after, &weights);
    eprintln!(
        "{what}: the land's mode {mode:.0} m after the step ({:.0} m before) against +125 m within \
         300 m: {}",
        land_mode(&before, &weights),
        if (mode - LAND_MODE_M.0).abs() <= LAND_MODE_M.1 {
            "inside"
        } else {
            "FINDING, outside"
        }
    );
    print_land_quantiles(&after, &format!("{what}, after the remap"));
    print_land_quantiles(&before, &format!("{what}, the construction"));
    remap_records(what, outcome, (&z0, sea_level_m), remapped);
    print_raised(what, staged, working, outcome);
}

/// Prints the remap's records under `what` (decision-r09-t14-choices, "T14.c follow-up 2" and 3),
/// `sea_level_m` the sea's level: the remapped cells' area quantiles of height above the sea
/// against the construction's over the same cells, the construction basins' mean drift after the
/// remap, the held cells, and the pass's lifts and lowerings.
fn remap_records(
    what: &str,
    outcome: &Outcome,
    (z0, sea_level_m): (&[f64], f64),
    remapped: &Remapped,
) {
    let cells: Vec<usize> = (0..z0.len()).filter(|&c| remapped.moved[c]).collect();
    let weights: Vec<f64> = cells.iter().map(|&c| outcome.mesh.areas_m2()[c]).collect();
    let after: Vec<f64> = cells
        .iter()
        .map(|&c| remapped.elevation_m[c] - sea_level_m)
        .collect();
    let before: Vec<f64> = cells.iter().map(|&c| z0[c] - sea_level_m).collect();
    let row = |values: &[f64]| -> String {
        [0.1, 0.5, 0.9, 0.99, 1.0]
            .iter()
            .map(|&q| format!("{:.0}", area_quantile(values, &weights, q)))
            .collect::<Vec<_>>()
            .join(" / ")
    };
    eprintln!(
        "{what}: the remap over {} cells, height above the sea by area p10 / p50 / p90 / p99 / \
         max: {} m after against {} m in the construction",
        cells.len(),
        row(&after),
        row(&before)
    );
    let n = z0.len();
    let (mut moved, mut area) = (vec![0.0; n], vec![0.0; n]);
    for (&c, &w) in cells.iter().zip(&weights) {
        let b = slot(outcome.basins[c]);
        moved[b] += w * (remapped.elevation_m[c] - z0[c]);
        area[b] += w;
    }
    let drifts: Vec<f64> = (0..n)
        .filter(|&b| area[b] > 0.0)
        .map(|b| (moved[b] / area[b]).abs())
        .collect();
    if let Some([_, _, _, median, _, p90, max]) = quantiles(drifts) {
        eprintln!(
            "{what}: the construction basins' mean drift after the remap: median {median:.1} m, \
             p90 {p90:.1} m, max {max:.1} m"
        );
    }
    eprintln!(
        "{what}: {} cells held apart; the pass lifted {} cells to their receiver's surface (the \
         largest {:.1} m) and lowered {} to the threshold (the largest {:.1} m)",
        remapped.held.iter().filter(|&&h| h).count(),
        remapped.lifted,
        remapped.largest_lift_m,
        remapped.lowered,
        remapped.largest_lowering_m
    );
}

/// Prints under `what` the cells `outcome` raises most above their construction (`staged`), with
/// their basins, receivers, runoff and areas, and the share of the cells shedding under 1 cm a⁻¹
/// with the most raised of them and of the rest; and the share of the cells draining at the end to
/// another root than their construction basin's (`working` the state the step left).
fn print_raised(what: &str, staged: &Staged, working: &Working, outcome: &Outcome) {
    let free: Vec<usize> = (0..outcome.z_m.len())
        .filter(|&c| outcome.bounds[c].is_none())
        .collect();
    let dry: Vec<&usize> = free
        .iter()
        .filter(|&&c| working.cells[c].runoff_m_per_year < 0.01)
        .collect();
    let rise = |cells: &mut dyn Iterator<Item = usize>| {
        cells.fold(f64::NEG_INFINITY, |hi, c| {
            num::max(
                hi,
                outcome.z_m[c] - staged.working.cells[c].elevation.value(),
            )
        })
    };
    eprintln!(
        "{what}: {} of {} cells not bound shed under 1 cm a⁻¹ of their own; the most raised of \
         them {:.0} m, of the rest {:.0} m",
        dry.len(),
        free.len(),
        rise(&mut dry.iter().map(|&&c| c)),
        rise(
            &mut free
                .iter()
                .copied()
                .filter(|&c| working.cells[c].runoff_m_per_year >= 0.01)
        )
    );
    let moved = free
        .iter()
        .filter(|&&c| outcome.routing.root_of(cell_of(c)) != outcome.basins[c])
        .count();
    eprintln!(
        "{what}: {moved} of {} cells not bound drain at the end to another root than their \
         construction basin's",
        free.len()
    );
    let z0: Vec<f64> = staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let mut raised = free.clone();
    raised.sort_unstable_by(|&a, &b| {
        (outcome.z_m[b] - z0[b])
            .total_cmp(&(outcome.z_m[a] - z0[a]))
            .then(a.cmp(&b))
    });
    for &c in raised.iter().take(5) {
        let cell = cell_of(c);
        let receiver = outcome.routing.receiver(cell).map(|link| slot(link.cell));
        eprintln!(
            "{what}: cell {c} raised {:.0} m to {:.0} m (z0 {:.0}); construction basin {}, final \
             root {}, depth {}; receiver {receiver:?} at {:.0} m, its basin {:?}; own runoff \
             {:.4} m a⁻¹, A_eq {:.3e} m², drainage {:.3e} m², link {:.0} m",
            outcome.z_m[c] - z0[c],
            outcome.z_m[c],
            z0[c],
            outcome.basins[c],
            outcome.routing.root_of(cell),
            outcome.routing.depth(cell),
            receiver.map_or(f64::NAN, |r| outcome.z_m[r]),
            receiver.map(|r| outcome.basins[r]),
            working.cells[c].runoff_m_per_year,
            outcome.equivalent_area_m2[c],
            outcome.drainage_m2[c],
            outcome
                .routing
                .receiver(cell)
                .map_or(0.0, |link| link.length_m)
        );
    }
}

/// The reference Earth's steady state at level 8 (the plan's R09.T14.b test as
/// decision-r09-t14-choices' "T14.b follow-up" item 5 restates it, with the "T14.c follow-up" items
/// 2–3 and the "T14.c follow-up 2"), wet now, under the step's own options (sticky receivers):
///
/// - (iii) the solve is that of a non-negative uplift field on its last routing, every cell not
///   bound at or above its receiver on it, strictly where its uplift is positive, the pits held at
///   their construction counted and recorded apart ([`assert_above_receivers`]; the "T14.c
///   follow-up 3" item 1), and no link rises more than `DEBRIS_FLOW_SLOPE` times its length, but
///   a link into a held pit, measured against the pit's construction below the path's value there,
///   counted apart with the pits (the remap's pass holds it on the final field);
/// - the remap of absolute heights ([`remap_onto_construction`]) holds the pits apart and keeps
///   every remapped cell at or above its receiver with no link steeper than the threshold
///   ([`assert_remap_keeps_the_drainage`]), and (ii) the remapped set's highest cell is the
///   construction's, to the pass's lowering, with no land cell above the construction's highest
///   ([`assert_peak_is_the_construction_s`]; the "T14.c follow-up 3" item 3);
/// - T12.a's bands hold on the final field, after the whole pass ([`assert_earth_bands`]; the
///   "T14.c follow-up 3" item 3);
/// - (i) the final field's realised relief, after the whole pass, lies within R09.T12.e's 6–16
///   `σ_h` ([`assert_final_relief`]);
/// - the fixed point's residual falls within each multigrid level, its last iteration's below
///   half its first.
///
/// Recorded ([`record`]): each level's first and last residual, largest change, changed receivers
/// and basin misfits, against the ruling's triggers ([`triggers`]); the final field's steepest edge
/// between land neighbours and the edges steeper than 0.1 ([`steepest_edge`]; above 0.2 a
/// finding); the raw solve's largest change beside the remapped field's, recorded, no longer a
/// trigger; `λ_b`'s distribution, the basins of `S_b` zero and the cells held at the threshold;
/// the implied mean denudation over the land against Earth's 0.05–0.1 mm a⁻¹ (Milliman and Meade
/// 1983, from memory), a deferred finding on K⊕'s absolute value at the coarse cell, not tuned
/// (the "T14.c follow-up" item 2: K cancels per basin, so no elevation moves with it); the land's
/// hypsometry above the sea after the remap against the construction's, the basins' mean drift,
/// the held cells and the pass's lifts and lowerings ([`remap_records`]); and the land's mode,
/// +125 m within 300 m. Each is measured with sticky and with fresh receivers, the step's and
/// T14.b's, with one row of a table each ([`summarise`]). The triggers and the records are printed
/// before any assertion.
#[test]
#[ignore = "slow: the reference Earth's steady state at level 8, R09.T14.c"]
fn the_reference_earth_s_steady_state_falls_within_each_level() {
    let clock = Instant::now();
    let staged = Staged::of(reference::earth_like());
    let staging_s = clock.elapsed().as_secs_f64();
    let pass = staged.pass();
    let clock = Instant::now();
    let fluid = Fluid::of(pass.inputs()).expect("a world wet now has a fluid");
    let runoff = wet_now_runoff_m_per_year(&pass, &staged.working, fluid);
    let runoff_s = clock.elapsed().as_secs_f64();
    let sea = sea_cells(&staged.working);
    let sea_level_m = staged.working.sea_level.value();
    let z0: Vec<f64> = staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let solve = |options: Options| {
        let clock = Instant::now();
        let outcome = erode_wet_now_with(&pass, &staged.working, &runoff, options);
        let remapped = remap_onto_construction(&outcome, &z0, &outcome.z_m);
        let solve_s = clock.elapsed().as_secs_f64();
        let mut working = staged.working.clone();
        outputs::write(
            &mut working,
            sea_level_m,
            &outcome,
            &Written {
                elevation_m: &remapped.elevation_m,
                sea: &sea,
                runoff_m_per_year: &runoff,
                channel_incision_m: None,
            },
        );
        (outcome, remapped, working, solve_s)
    };
    eprintln!(
        "reference Earth: level {}, {} cells; staged in {staging_s:.2} s; the runoff read in \
         {runoff_s:.2} s",
        pass.grid().level(),
        staged.working.cells.len()
    );
    let omega = pass.grid().solid_angles_sr();
    let mut step = None;
    for receivers in [Receivers::Sticky, Receivers::Fresh] {
        let options = Options {
            receivers,
            ..Options::WET_NOW
        };
        let (outcome, remapped, working, solve_s) = solve(options);
        let what = format!("reference Earth, {receivers:?}");
        eprintln!("{what}: the multigrid and the remap in {solve_s:.2} s");
        record(&what, &staged, &working, &outcome, &remapped);
        print_tallest(&what, &staged, &working, &outcome, &remapped);
        let mut finished = working.clone();
        craters::run_after_erosion(&pass, &mut finished);
        summarise(
            &what,
            pass.inputs(),
            (&outcome, &remapped),
            (&staged.working, &working, &finished),
            (&sea, omega, sea_level_m),
            solve_s,
        );
        if options == Options::WET_NOW {
            step = Some((outcome, remapped, finished));
        }
    }
    let (outcome, remapped, finished) =
        step.expect("the step's own options are among those measured");
    assert_final_field(
        &pass,
        (&z0, &sea, sea_level_m),
        (&outcome, &remapped),
        &finished,
    );
}

/// The slow test's assertions on the reference Earth's `outcome` and its remap `remapped` under
/// the step's own options on `pass`, `z0` the construction with its sea `sea` at `sea_level_m`,
/// and `finished` the state after the whole pass: (iii) and the threshold on the solve, the
/// remap's drainage, (ii), T12.a's bands, (i) and the fall within each level, in that order.
fn assert_final_field(
    pass: &Pass<'_>,
    (z0, sea, sea_level_m): (&[f64], &[bool], f64),
    (outcome, remapped): (&Outcome, &Remapped),
    finished: &Working,
) {
    assert_above_receivers("reference Earth", outcome, z0);
    let Threshold::Slope(slope) = wet_now_threshold() else {
        panic!("a world wet now has a threshold slope");
    };
    // A link into a held pit is measured against the pit's construction, below the path's own
    // value there, which the solver's threshold caps: such links are counted apart, as the pits
    // are, and the remap's pass holds them on the final field (`assert_remap_keeps_the_drainage`).
    let (_, _, pits) = not_above_receivers(outcome, z0);
    let (steeper, steepest) = steeper_than(outcome, &outcome.z_m, slope);
    let (into_pits, steeper): (Vec<u32>, Vec<u32>) = steeper.into_iter().partition(|&c| {
        outcome
            .routing
            .receiver(c)
            .is_some_and(|link| pits.binary_search(&link.cell).is_ok())
    });
    eprintln!(
        "reference Earth: {} links of the solve steeper than {slope} drain into held pits, the \
         first {:?}; {} others (the steepest of all {steepest:.4})",
        into_pits.len(),
        &into_pits[..into_pits.len().min(8)],
        steeper.len()
    );
    assert!(
        steeper.is_empty(),
        "reference Earth: {} links of the solve steeper than {slope} (the steepest {steepest}), \
         the first {:?}",
        steeper.len(),
        &steeper[..steeper.len().min(8)]
    );
    assert_remap_keeps_the_drainage("reference Earth", outcome, z0, remapped);
    assert_peak_is_the_construction_s("reference Earth", z0, (sea, sea_level_m), remapped);
    assert_earth_bands("reference Earth, the final field", pass, finished);
    assert_final_relief("reference Earth", pass.inputs(), finished);
    for r in &outcome.diagnostics.residuals {
        assert!(
            r.last_m() < WITHIN_LEVEL_FALL * r.first_m(),
            "level {} falls from {} m to {} m",
            r.level,
            r.first_m(),
            r.last_m()
        );
    }
}

/// Prints under `what` the five land cells highest above the sea after the remap `remapped` of
/// `outcome` (`working` the state the step left, `staged` its construction), each with its solve,
/// construction, receiver, construction basin and final root, own runoff, `A_eq`, uplift and link.
fn print_tallest(
    what: &str,
    staged: &Staged,
    working: &Working,
    outcome: &Outcome,
    remapped: &Remapped,
) {
    let sea_level_m = staged.working.sea_level.value();
    let sea = sea_cells(&staged.working);
    let z = &remapped.elevation_m;
    let mut land: Vec<usize> = (0..z.len()).filter(|&c| !sea[c]).collect();
    land.sort_unstable_by(|&a, &b| z[b].total_cmp(&z[a]).then(a.cmp(&b)));
    for &c in land.iter().take(5) {
        let cell = cell_of(c);
        let link = outcome.routing.receiver(cell);
        let receiver = link.map(|link| slot(link.cell));
        eprintln!(
            "{what}: tallest cell {c} at {:.0} m above the sea after the remap ({:.0} m solved, \
             {:.0} m constructed); construction basin {}, final root {}; receiver {receiver:?} \
             at {:.0} m; own runoff {:.4} m a⁻¹, A_eq {:.3e} m², uplift {:.3e} m a⁻¹, link {:.0} m",
            z[c] - sea_level_m,
            outcome.z_m[c] - sea_level_m,
            staged.working.cells[c].elevation.value() - sea_level_m,
            outcome.basins[c],
            outcome.routing.root_of(cell),
            receiver.map_or(f64::NAN, |r| z[r] - sea_level_m),
            working.cells[c].runoff_m_per_year,
            outcome.equivalent_area_m2[c],
            outcome.uplift_m_per_year[c],
            link.map_or(0.0, |link| link.length_m)
        );
    }
}

/// The area quantile `q` of `values` weighted by `weights`: the least value whose cells, with all
/// below it, hold at least `q` of the weight.
fn area_quantile(values: &[f64], weights: &[f64], q: f64) -> f64 {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_unstable_by(|&a, &b| values[a].total_cmp(&values[b]).then(a.cmp(&b)));
    let total: f64 = weights.iter().sum();
    let mut held = 0.0;
    for &c in &order {
        held += weights[c];
        if held >= q * total {
            return values[c];
        }
    }
    order.last().map_or(0.0, |&c| values[c])
}

/// Prints one row of the table under `what` (`inputs` the world's, `construction` the state
/// entering the step, `working` the state it left and `finished` the state after the whole pass):
/// the finest two levels' last residual, raw largest change and fall; the cells breaking (iii) on
/// the solve; the solve's steepest link, the cells held at the threshold and the basins of `S_b`
/// zero; the re-capped links; the land's area p99, p999 and greatest height above the sea after
/// the remap, against the construction's and the solve's greatest, and its mode; the final
/// field's relief over `σ_h`; `λ_b`'s median and greatest; the implied denudation and the wall
/// time.
fn summarise(
    what: &str,
    inputs: &CoarseInputs,
    (outcome, remapped): (&Outcome, &Remapped),
    (construction, working, finished): (&Working, &Working, &Working),
    (sea, omega, sea_level_m): (&[bool], &[f64], f64),
    solve_s: f64,
) {
    let levels: Vec<String> = outcome
        .diagnostics
        .residuals
        .iter()
        .filter(|r| r.level >= 7)
        .map(|r| {
            format!(
                "L{} {:.1} m (fall {:.3}), largest {:.0} m",
                r.level,
                r.last_m(),
                r.last_m() / r.first_m(),
                r.largest_change_m()
            )
        })
        .collect();
    let (_, steepest) = steeper_than(outcome, &outcome.z_m, f64::INFINITY);
    let (mut heights, mut weights) = (Vec::new(), Vec::new());
    for ((cell, &wet), &w) in working.cells.iter().zip(sea).zip(omega) {
        if !wet {
            heights.push(cell.elevation.value() - sea_level_m);
            weights.push(w);
        }
    }
    let z0: Vec<f64> = construction
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let (below, level, pits) = not_above_receivers(outcome, &z0);
    let (peak, peak0, _) = land_peaks(&remapped.elevation_m, &z0, sea, sea_level_m);
    let (solved, _, _) = land_peaks(&outcome.z_m, &z0, sea, sea_level_m);
    let (relief, relief_ratio) = relief_over_sigma_h(inputs, finished);
    let lambda = outcome.diagnostics.lambda.expect("a fitted basin");
    eprintln!(
        "TABLE {what}: {}; (iii) {} below, {} level under uplift; steepest link {steepest:.4}, \
         {} pits, {} cells held at the threshold, {} basins of S_b zero; {} cells lifted, {} \
         lowered; land p99 {:.0} m, p999 {:.0} m, max {peak:.0} m against the construction's \
         {peak0:.0} m and the solve's {solved:.0} m, mode {:.0} m; (i) relief {relief:.0} m, \
         {relief_ratio:.2} σ_h; λ_b median {:.3e}, max {:.3e} a⁻¹; denudation {:.4} mm a⁻¹; \
         {solve_s:.2} s",
        levels.join("; "),
        below.len(),
        level.len(),
        pits.len(),
        outcome.diagnostics.held_at_threshold,
        outcome.diagnostics.insensitive_basins,
        remapped.lifted,
        remapped.lowered,
        area_quantile(&heights, &weights, 0.99),
        area_quantile(&heights, &weights, 0.999),
        land_mode(&heights, &weights),
        lambda.percentiles[2],
        lambda.max,
        1_000.0 * outcome.diagnostics.mean_uplift_m_per_year
    );
}

/// Prints the land's heights above the sea under `what`: their unweighted quantiles.
fn print_land_quantiles(heights: &[f64], what: &str) {
    if let Some([min, p10, p25, p50, p75, p90, max]) = quantiles(heights.to_vec()) {
        eprintln!(
            "the land's heights {what}: min {min:.0} m, p10 {p10:.0}, p25 {p25:.0}, median {p50:.0}, \
             p75 {p75:.0}, p90 {p90:.0}, max {max:.0} m"
        );
    }
}
