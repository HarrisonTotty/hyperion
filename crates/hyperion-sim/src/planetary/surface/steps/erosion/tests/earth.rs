//! The reference Earth's erosion, wet now (R09.T14.c; the lane's design §4.5 test 6,
//! decision-r09-t14-choices Q1 and the "T14.b follow-up" item 5): the plan's restated fixed-point
//! test on the reference Earth at level 8, with its records and the ruling's triggers, slow.

use std::collections::BTreeMap;
use std::time::Instant;

use hyperion_surface::num;

use super::super::super::CellState;
use super::super::branches::{erode_wet_now_with, sea_cells, wet_now_runoff_m_per_year};
use super::super::erodibility::Fluid;
use super::super::multigrid::{Options, Outcome, Receivers, UpliftFit};
use super::super::outputs::{self, Written};
use super::*;

/// The ruling's trigger on the final RMS residual, m: 125, 5% of the reference Earth's `σ_h` of
/// 2.51 km (decision-r09-t14-choices, "T14.b follow-up" item 5).
const RESIDUAL_TRIGGER_M: f64 = 125.0;

/// The ruling's trigger on the largest change, m: 1 km, a cliff of 2.6% across a cell.
const LARGEST_CHANGE_TRIGGER_M: f64 = 1_000.0;

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

/// The ruling's triggers on `outcome`'s records (decision-r09-t14-choices, "T14.b follow-up"
/// item 5): a final residual above 125 m, a largest change above 1 km, or the fall within a
/// level failing at level 7 or 8; each tripped, as text.
fn triggers(outcome: &Outcome) -> Vec<String> {
    let residuals = &outcome.diagnostics.residuals;
    let finest = residuals.last().expect("a level");
    let mut findings = Vec::new();
    if finest.last_m() > RESIDUAL_TRIGGER_M {
        findings.push(format!(
            "final residual {:.1} m above 125 m",
            finest.last_m()
        ));
    }
    if finest.largest_change_m() > LARGEST_CHANGE_TRIGGER_M {
        findings.push(format!(
            "largest change {:.1} m above 1 km",
            finest.largest_change_m()
        ));
    }
    for r in residuals {
        if r.level >= 7 && r.last_m() >= 0.25 * r.first_m() {
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
/// construction, `working` the state the step left): each level's, the triggers, the implied
/// denudation, the land's mode and heights against the construction's, and [`print_raised`]'s.
fn record(what: &str, staged: &Staged, working: &Working, outcome: &Outcome) {
    report(what, outcome);
    let findings = triggers(outcome);
    eprintln!(
        "{what}: triggers: {}",
        if findings.is_empty() {
            "none trip".to_owned()
        } else {
            format!("FINDING: {}", findings.join("; "))
        }
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
    print_land_quantiles(&after, &format!("{what}, after"));
    print_land_quantiles(&before, &format!("{what}, before"));
    print_raised(what, staged, working, outcome);
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
/// decision-r09-t14-choices' "T14.b follow-up" item 5 restates it), wet now: the fixed point's
/// residual falls within each multigrid level, its last iteration's below a quarter of its first,
/// and the final elevation is the solve on its last routing, every cell not bound above its
/// receiver on it within one construction basin ([`assert_above_receivers`]). Recorded
/// ([`record`]): each level's first and last residual, largest change and changed receivers,
/// against the ruling's triggers; `λ_b`'s distribution; the implied mean denudation over the land
/// against Earth's 0.05–0.1 mm a⁻¹ (Milliman and Meade 1983, from memory); and the land's
/// hypsometry after the step against T12.a's mode, +125 m within 300 m. Each is measured for both
/// uplift fits ([`UpliftFit`]), each with fresh and with sticky receivers ([`Receivers::Sticky`],
/// the ruling's lever where a trigger trips), with one row of a table each ([`summarise`]); the
/// assertions hold the step's own options. The triggers and the records are printed before any
/// assertion.
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
    let solve = |options: Options| {
        let clock = Instant::now();
        let outcome = erode_wet_now_with(&pass, &staged.working, &runoff, options);
        let solve_s = clock.elapsed().as_secs_f64();
        let clock = Instant::now();
        let mut working = staged.working.clone();
        outputs::write(
            &mut working,
            sea_level_m,
            &outcome,
            &Written {
                elevation_m: &outcome.z_m,
                sea: &sea,
                runoff_m_per_year: &runoff,
                channel_incision_m: None,
            },
        );
        (outcome, working, solve_s, clock.elapsed().as_secs_f64())
    };
    eprintln!(
        "reference Earth: level {}, {} cells; staged in {staging_s:.2} s; the runoff read in \
         {runoff_s:.2} s",
        pass.grid().level(),
        staged.working.cells.len()
    );
    let omega = pass.grid().solid_angles_sr();
    let mut step = None;
    for (uplift_fit, receivers) in [
        (UpliftFit::Diagonal, Receivers::Fresh),
        (UpliftFit::Diagonal, Receivers::Sticky),
        (UpliftFit::SpatialField, Receivers::Fresh),
        (UpliftFit::SpatialField, Receivers::Sticky),
    ] {
        let options = Options {
            receivers,
            uplift_fit,
            ..Options::STEP
        };
        let (outcome, working, solve_s, write_s) = solve(options);
        let what = format!("reference Earth, {uplift_fit:?}, {receivers:?}");
        eprintln!("{what}: the multigrid in {solve_s:.2} s, the outputs in {write_s:.2} s");
        record(&what, &staged, &working, &outcome);
        summarise(
            &what,
            &outcome,
            (&staged.working, &working),
            &sea,
            omega,
            sea_level_m,
            solve_s,
        );
        if options == Options::STEP {
            step = Some(outcome);
        }
    }
    let outcome = step.expect("the step's own options are among those measured");
    for r in &outcome.diagnostics.residuals {
        assert!(
            r.last_m() < 0.25 * r.first_m(),
            "level {} falls from {} m to {} m",
            r.level,
            r.first_m(),
            r.last_m()
        );
    }
    assert_above_receivers("reference Earth", &outcome);
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

/// Prints one row of the uplift fits' table under `what` (`construction` the state entering the
/// step and `working` the state it left): the finest two levels' last residual, largest change and
/// fall, the cells not above their receiver (within and across a construction basin, the first
/// within printed), the land's area p99, p999 and greatest height above the sea, its mode, `λ_b`'s median
/// and greatest, the implied denudation and the multigrid's wall time.
fn summarise(
    what: &str,
    outcome: &Outcome,
    (construction, working): (&Working, &Working),
    sea: &[bool],
    omega: &[f64],
    sea_level_m: f64,
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
    let (within, across) = not_above_receivers(outcome);
    for &c in within.iter().take(3) {
        let link = outcome
            .routing
            .receiver(c)
            .expect("a cell not above its receiver drains");
        let (a, b) = (slot(c), slot(link.cell));
        eprintln!(
            "{what}: cell {c} at {:.3} m, its receiver {} at {:.3} m, in one construction basin \
             {}; its construction {:.3} m, its receiver's {:.3} m",
            outcome.z_m[a],
            link.cell,
            outcome.z_m[b],
            outcome.basins[a],
            construction.cells[a].elevation.value(),
            construction.cells[b].elevation.value()
        );
    }
    let (mut heights, mut weights) = (Vec::new(), Vec::new());
    for ((cell, &wet), &w) in working.cells.iter().zip(sea).zip(omega) {
        if !wet {
            heights.push(cell.elevation.value() - sea_level_m);
            weights.push(w);
        }
    }
    let lambda = outcome.diagnostics.lambda.expect("a fitted basin");
    eprintln!(
        "TABLE {what}: {}; not above {} within, {} across; land p99 {:.0} m, p999 {:.0} m, max \
         {:.0} m, mode {:.0} m; λ_b median {:.3e}, max {:.3e} a⁻¹; denudation {:.4} mm a⁻¹; \
         {solve_s:.2} s",
        levels.join("; "),
        within.len(),
        across.len(),
        area_quantile(&heights, &weights, 0.99),
        area_quantile(&heights, &weights, 0.999),
        heights.iter().copied().fold(f64::NEG_INFINITY, num::max),
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
