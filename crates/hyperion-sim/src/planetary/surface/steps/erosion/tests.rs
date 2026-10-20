//! The erosion step's tests (R09.T14, Design note 9).
//!
//! - R09.T14.a's of the drainage: the plan's four (drainage area at every outlet, no cell draining
//!   uphill after filling, the same seed's receivers, a finite inventory filling the lowest basins
//!   first and spilling) and the lane's checks (flats without ε, the flood against a minimax
//!   oracle, Priority-Flood against the merge tree when every depression is full, and the
//!   receivers' draw in proportion to the drop per metre). Each runs on hand-built ground over a
//!   real cube-sphere level, 3 to 6, on a body of radius 1,000 km.
//! - R09.T14.b's of the solver: the three calibrated erodibilities and the bed table
//!   (`erodibility`); the ridge-to-sea steady state against its closed form, the lowering linear in
//!   t and in runoff^0.45, the Mars calibration's K t, the bounds and the cells of no area, no
//!   deposition, the linearity in the uplift's scale and the threshold slope, on hand-built
//!   networks (`solver`); and
//!   the residual falling over the multigrid's levels on a synthetic world, the final elevation
//!   above its receivers on its last routing, the jitter's draws and the upsample of a constant
//!   field, with the measurement of the three starts of a finer level (`multigrid`).
//! - R09.T14.c's of the branches and the Mars check: the branch and the share of the epoch, a
//!   surface younger than its epoch's end and a world never wet untouched, the young surface
//!   eroded without runoff of its own, the paleo-water stored, the outputs (`branches`); F1's
//!   trunk incision against Black et al.'s, the sub-cell volume pinned to the synthesis's closed
//!   form, the ceiling's measurement and the slow records (`mars`); the reference Earth's slow
//!   record of the steady state (`earth`); and decision-r09-t14-choices' "T14.c follow-up" item 3
//!   on the final field, its realised relief on every reference world, the highest land on every
//!   world wet now and each cell at or above its receiver.
//!
//! Each world the branches' tests stage ([`Staged`]) runs the pass's steps before the erosion in
//! the pass's order, under the reference seed.

use core::cmp::Ordering;

use hyperion_surface::field::{CoarseLevel, coarse_level};
use hyperion_surface::synth::channels::DEBRIS_FLOW_SLOPE;
use hyperion_testkit::lcg::Lcg;

use super::super::{Pass, Working, climate, craters, plates, relief};
use super::branches::Remapped;
use super::mesh::Mesh;
use super::multigrid::Outcome;
use crate::math;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::planetary::surface::reference;
use crate::planetary::surface::steps::plates::{cell_of, slot};
use crate::units::Metres;

mod branches;
mod depressions;
mod drainage;
mod earth;
mod erodibility;
mod mars;
mod multigrid;
mod solver;

/// A world's working state staged to the end of the climate step, under the reference seed: the
/// pass's steps before the erosion, in the pass's order (`coarse_pass`'s).
struct Staged {
    inputs: CoarseInputs,
    grid: CellGraph,
    climate_grid: CellGraph,
    working: Working,
}

impl Staged {
    /// `inputs`' world at its own level, through plates, relief, craters, the relief's second
    /// entry and climate.
    fn of(inputs: CoarseInputs) -> Self {
        let level = coarse_level(inputs.radius());
        Self::at(inputs, level)
    }

    /// [`Staged::of`] at the coarse level `level` rather than the body's own.
    fn at(inputs: CoarseInputs, level: CoarseLevel) -> Self {
        let grid = CellGraph::for_field(level);
        let climate_grid = CellGraph::for_climate(level);
        let working = {
            let pass = Pass::new(reference::SEED, &inputs, &grid, &climate_grid);
            let mut working = Working::initial(&pass);
            plates::run(&pass, &mut working);
            relief::run(&pass, &mut working);
            craters::run(&pass, &mut working);
            relief::run_after_craters(&pass, &mut working);
            climate::run(&pass, &mut working);
            working
        };
        Self {
            inputs,
            grid,
            climate_grid,
            working,
        }
    }

    /// The pass over the staged world.
    fn pass(&self) -> Pass<'_> {
        Pass::new(
            reference::SEED,
            &self.inputs,
            &self.grid,
            &self.climate_grid,
        )
    }

    /// The working state after the whole pass from the staged one: the erosion step and the late
    /// coarse craters (`coarse_pass`'s rest; the classes step changes nothing).
    fn finished(&self) -> Working {
        let pass = self.pass();
        let mut working = self.working.clone();
        super::run(&pass, &mut working);
        craters::run_after_erosion(&pass, &mut working);
        working
    }
}

/// Prints `outcome`'s records under `what`: each level's first and last residual, its residuals by
/// iteration, its largest change, the receivers its last two routings changed and its basin
/// misfits by iteration; `λ_b`'s distribution, the basins with no response and the cells held at
/// the threshold; and the implied uplift.
fn report(what: &str, outcome: &Outcome) {
    let d = &outcome.diagnostics;
    for r in &d.residuals {
        let each: Vec<String> = r.by_iteration_m.iter().map(|x| format!("{x:.1}")).collect();
        let misfit: Vec<String> = r
            .basin_misfit_by_iteration_m
            .iter()
            .map(|x| format!("{x:.2}"))
            .collect();
        eprintln!(
            "{what}: level {} residual {:.3} → {:.3} m (by iteration: {} m), largest change {:.1} m, \
             {} receivers changed; basin misfit by iteration [{}] m",
            r.level,
            r.first_m(),
            r.last_m(),
            each.join(", "),
            r.largest_change_m(),
            r.receivers_changed,
            misfit.join(", ")
        );
    }
    eprintln!(
        "{what}: {} fitted basins of sensitivity S_b zero at the finest level's last refit; {} \
         cells held at the threshold slope",
        d.insensitive_basins, d.held_at_threshold
    );
    if let Some(l) = d.lambda {
        eprintln!(
            "{what}: λ_b over {} basins, a⁻¹: min {:.3e}, p10 {:.3e}, p25 {:.3e}, p50 {:.3e}, \
             p75 {:.3e}, p90 {:.3e}, max {:.3e}",
            l.basins,
            l.min,
            l.percentiles[0],
            l.percentiles[1],
            l.percentiles[2],
            l.percentiles[3],
            l.percentiles[4],
            l.max
        );
    }
    eprintln!(
        "{what}: implied mean uplift over the land {:.4} mm a⁻¹",
        1_000.0 * d.mean_uplift_m_per_year
    );
}

/// The cells of `outcome` that break decision-r09-t14-choices' "T14.c follow-up" item 3 (iii) on
/// its last routing, ascending, over the cells not bound and of equivalent area above zero (Q1's
/// bounds apart): those below their receiver, those level with it where their uplift is positive,
/// and, apart, the pits, the cells without uplift held at their construction `z0` to the bit below
/// their receiver.
///
/// A pit is the solver's no-deposition rule (R09.T14.b: a cell with no uplift never ends above its
/// construction) where a construction basin's `λ_b` has reached zero and its cells drain through a
/// path another basin's uplift raised: a closed depression of the final field, which R09.T14.d's
/// lakes on the final field take. Raising it level with its receiver would deposit up to 1.3 km on
/// the reference Earth with no uplift.
fn not_above_receivers(outcome: &Outcome, z0: &[f64]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let (mut below, mut level, mut pits) = (Vec::new(), Vec::new(), Vec::new());
    for c in 0..outcome.routing.len() {
        let (Some(link), None) = (outcome.routing.receiver(c), outcome.bounds[slot(c)]) else {
            continue;
        };
        if outcome.equivalent_area_m2[slot(c)] <= 0.0 {
            continue;
        }
        let (z, uplift) = (outcome.z_m[slot(c)], outcome.uplift_m_per_year[slot(c)]);
        match z.total_cmp(&outcome.z_m[slot(link.cell)]) {
            Ordering::Less if uplift <= 0.0 && z.total_cmp(&z0[slot(c)]).is_eq() => pits.push(c),
            Ordering::Less => below.push(c),
            Ordering::Equal if uplift > 0.0 => level.push(c),
            Ordering::Equal | Ordering::Greater => {}
        }
    }
    (below, level, pits)
}

/// The final elevation of `outcome` is the solve of a non-negative uplift field on its last
/// routing (decision-r09-t14-choices, "T14.c follow-up" item 3 (iii), which replaces the "T14.b
/// follow-up" item 5's form within a construction basin): every cell not bound and of equivalent
/// area above zero lies at or above its receiver on that routing, strictly where its uplift is
/// positive, the pits held at their construction `z0` apart ([`not_above_receivers`]). The counts
/// are printed under `what`.
///
/// # Panics
///
/// If a cell lies below its receiver other than as a pit, or level with it where its uplift is
/// positive.
fn assert_above_receivers(what: &str, outcome: &Outcome, z0: &[f64]) {
    let (below, level, pits) = not_above_receivers(outcome, z0);
    eprintln!(
        "{what}: (iii) {} cells not bound below their receiver and {} level with it under \
         positive uplift (of {} cells); {} pits held at their construction without uplift",
        below.len(),
        level.len(),
        outcome.routing.len(),
        pits.len()
    );
    for &c in below.iter().chain(&level).chain(&pits).take(4) {
        let link = outcome.routing.receiver(c).expect("a cell counted drains");
        eprintln!(
            "{what}: (iii) cell {c} at {:.3} m, uplift {:.3e} m a⁻¹, A_eq {:.3e} m²; its receiver \
             {} at {:.3} m",
            outcome.z_m[slot(c)],
            outcome.uplift_m_per_year[slot(c)],
            outcome.equivalent_area_m2[slot(c)],
            link.cell,
            outcome.z_m[slot(link.cell)]
        );
    }
    assert!(
        below.is_empty() && level.is_empty(),
        "{what}: {} cells lie below their receiver, the first {:?}, and {} level with it under \
         positive uplift, the first {:?}",
        below.len(),
        &below[..below.len().min(8)],
        level.len(),
        &level[..level.len().min(8)]
    );
}

/// The links of `outcome`'s last routing that rise more than `slope` times their length on the
/// field `z`, beyond 1 µm of rounding, ascending by cell, and the steepest link's slope, over the
/// cells not bound and of equivalent area above zero, each rise from its receiver's base, the
/// water's surface over a bound (decision-r09-t14-choices, "T14.c follow-up" item 2).
fn steeper_than(outcome: &Outcome, z: &[f64], slope: f64) -> (Vec<u32>, f64) {
    let (mut steeper, mut steepest) = (Vec::new(), 0.0_f64);
    for c in 0..outcome.routing.len() {
        let (Some(link), None) = (outcome.routing.receiver(c), outcome.bounds[slot(c)]) else {
            continue;
        };
        if outcome.equivalent_area_m2[slot(c)] <= 0.0 {
            continue;
        }
        let r = slot(link.cell);
        let rise = z[slot(c)] - outcome.bounds[r].unwrap_or(z[r]);
        steepest = steepest.max(rise / link.length_m);
        if rise > slope * link.length_m + 1.0e-6 {
            steeper.push(c);
        }
    }
    (steeper, steepest)
}

/// The remapped field `remapped` of `outcome` keeps (iii) and the threshold
/// (decision-r09-t14-choices, "T14.c follow-up 2" §2 and §4): every cell remapped lies at or above
/// its receiver on the last routing (the water's surface over a bound), the solve's pits apart
/// ([`not_above_receivers`], `z0` the construction), and no link rises more than
/// [`DEBRIS_FLOW_SLOPE`] times its length; the counts and the re-capped links are printed under
/// `what`.
///
/// # Panics
///
/// If a remapped cell other than a pit lies below its receiver, or a link is steeper.
fn assert_remap_keeps_the_drainage(what: &str, outcome: &Outcome, z0: &[f64], remapped: &Remapped) {
    let z = &remapped.elevation_m;
    let (_, _, pits) = not_above_receivers(outcome, z0);
    let below: Vec<u32> = (0..outcome.routing.len())
        .filter(|&c| {
            remapped.moved[slot(c)]
                && pits.binary_search(&c).is_err()
                && outcome.routing.receiver(c).is_some_and(|link| {
                    let r = slot(link.cell);
                    z[slot(c)] < outcome.bounds[r].unwrap_or(z[r])
                })
        })
        .collect();
    let (steeper, steepest) = steeper_than(outcome, z, DEBRIS_FLOW_SLOPE);
    eprintln!(
        "{what}: the remap: {} cells below their receiver, the steepest link {steepest:.4}, {} \
         links re-capped, the largest lowering {:.1} m",
        below.len(),
        remapped.recapped,
        remapped.largest_recap_m
    );
    assert!(
        below.is_empty() && steeper.is_empty(),
        "{what}: after the remap {} cells lie below their receiver, the first {:?}, and {} links \
         are steeper than the threshold, the first {:?}",
        below.len(),
        &below[..below.len().min(8)],
        steeper.len(),
        &steeper[..steeper.len().min(8)]
    );
}

/// The highest remapped cell of `remapped` above its base on `outcome`'s last routing and the
/// highest construction cell `z0` above its construction basin's base over the same cells, metres
/// (decision-r09-t14-choices, "T14.c follow-up 2" §4's (ii)).
fn peaks_above_base(outcome: &Outcome, z0: &[f64], remapped: &Remapped) -> (f64, f64) {
    let (mut after, mut before) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (c, &moved) in remapped.moved.iter().enumerate() {
        if moved {
            let base = outcome.routing.base_level_m(cell_of(c));
            after = after.max(remapped.elevation_m[c] - base);
            before = before.max(z0[c] - outcome.construction_base_m[c]);
        }
    }
    (after, before)
}

/// (ii) after the remap (decision-r09-t14-choices, "T14.c follow-up 2" §4): the highest remapped
/// cell above its base equals the construction's highest above its base, to the re-cap's largest
/// lowering and 1 µm of rounding; printed under `what`.
///
/// # Panics
///
/// If it does not.
fn assert_peak_is_the_construction_s(
    what: &str,
    outcome: &Outcome,
    z0: &[f64],
    remapped: &Remapped,
) {
    let (after, before) = peaks_above_base(outcome, z0, remapped);
    eprintln!(
        "{what}: (ii) the highest cell above its base {after:.3} m after the remap against the \
         construction's {before:.3} m"
    );
    assert!(
        after <= before + 1.0e-6 && after >= before - remapped.largest_recap_m - 1.0e-6,
        "{what}: the highest cell above its base is {after} m against the construction's {before} m"
    );
}

/// The highest cell off `sea` above `sea_level_m` in `z` and in `z0`, metres, and their ratio.
fn land_peaks(z: &[f64], z0: &[f64], sea: &[bool], sea_level_m: f64) -> (f64, f64, f64) {
    let peak = |values: &[f64]| {
        values
            .iter()
            .zip(sea)
            .filter(|&(_, &wet)| !wet)
            .fold(f64::NEG_INFINITY, |hi, (&h, _)| hi.max(h - sea_level_m))
    };
    let (after, before) = (peak(z), peak(z0));
    (after, before, after / before)
}

/// The realised relief of `working`, its cells' range, over `inputs`' `σ_h` (R09.T12.e's test 6,
/// `relief::tests::sigma_h`'s `assert_relief`), with the relief in metres.
fn relief_over_sigma_h(inputs: &CoarseInputs, working: &Working) -> (f64, f64) {
    let (lowest, highest) =
        working
            .cells
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), cell| {
                let z = cell.elevation.value();
                (lo.min(z), hi.max(z))
            });
    let relief = highest - lowest;
    (relief, relief / inputs.sigma_h().value())
}

/// R09.T12.e's realised relief, 6 to 16 `σ_h` (Design note 3's observed 7–15, widened by one `σ_h`
/// each way), re-asserted on the final field (decision-r09-t14-choices, "T14.c follow-up" item 3
/// (i)): `working` after the whole pass, printed under `what`.
///
/// # Panics
///
/// If the relief lies outside 6–16 `σ_h`.
fn assert_final_relief(what: &str, inputs: &CoarseInputs, working: &Working) {
    let (relief, ratio) = relief_over_sigma_h(inputs, working);
    eprintln!(
        "{what}: (i) the final field's realised relief {relief:.0} m, {ratio:.2} σ_h (σ_h {:.0} m)",
        inputs.sigma_h().value()
    );
    assert!(
        (6.0..=16.0).contains(&ratio),
        "{what}: the final field's realised relief is {ratio} σ_h"
    );
}

/// The distribution of `values` by nearest rank, as the multigrid's `λ_b` spread takes it: the
/// least, the 10th, 25th, 50th, 75th and 90th percentiles and the greatest, each the value of
/// rank round(p × (count − 1) ÷ 100) in ascending order; `None` if `values` is empty.
fn quantiles(mut values: Vec<f64>) -> Option<[f64; 7]> {
    let last = values.len().checked_sub(1)?;
    values.sort_unstable_by(f64::total_cmp);
    let rank = |percent: usize| values[(percent * last + 50) / 100];
    Some([
        values[0],
        rank(10),
        rank(25),
        rank(50),
        rank(75),
        rank(90),
        values[last],
    ])
}

/// The share of its first residual below which each multigrid level's last falls, fast and slow:
/// ½, the field formulation's slower fall (decision-r09-t14-choices, "T14.c follow-up 2" §3,
/// which restates the "T14.b follow-up" item 5's quarter).
const WITHIN_LEVEL_FALL: f64 = 0.5;

/// The radius of the test bodies, 1,000 km.
const RADIUS: Metres = Metres::new(1.0e6);

/// The sea level of the test fields with a sea, metres.
const SEA_LEVEL_M: f64 = 0.0;

/// Level `level`'s cells and their mesh on the test body.
fn world(level: u8) -> (CellGraph, Mesh) {
    let graph = CellGraph::new(level).unwrap();
    let mesh = Mesh::of(&graph, RADIUS);
    assert_eq!(mesh.level(), level);
    assert_eq!(mesh.len(), graph.len());
    (graph, mesh)
}

/// `v` scaled to unit length.
fn unit(v: [f64; 3]) -> [f64; 3] {
    let norm = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|x| x / norm)
}

/// The dot product of `a` and `b`.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cells of `graph` whose centres lie within `radius_rad` of the unit vector `centre`.
fn cap(graph: &CellGraph, centre: [f64; 3], radius_rad: f64) -> Vec<bool> {
    let threshold = math::cos(radius_rad);
    graph
        .centres()
        .iter()
        .map(|&d| dot(d, centre) > threshold)
        .collect()
}

/// A rough field with a sea: ground rising northwards by 2.5 km per unit of the centre's z, from
/// −1,875 m at the south pole to 3,125 m at the north, with ±300 m of independent noise a cell
/// (the testkit's LCG under `seed`), so that the land is pitted with depressions; the sea is every
/// cell below [`SEA_LEVEL_M`].
fn rough_terrain(graph: &CellGraph, seed: u64) -> (Vec<f64>, Vec<bool>) {
    let mut lcg = Lcg::new(seed);
    let z: Vec<f64> = graph
        .centres()
        .iter()
        .map(|d| 2_500.0 * (d[2] + 0.25) + 600.0 * (lcg.next_f64() - 0.5))
        .collect();
    let sea = z.iter().map(|&h| h < SEA_LEVEL_M).collect();
    (z, sea)
}

/// Rain of depth `depth_m` on every cell: its volume, m³, a cell.
fn rain(mesh: &Mesh, depth_m: f64) -> Vec<f64> {
    mesh.areas_m2().iter().map(|&a| a * depth_m).collect()
}

/// The cells `inside` marks, ascending.
fn cells_of(inside: &[bool]) -> Vec<u32> {
    inside
        .iter()
        .enumerate()
        .filter(|&(_, &x)| x)
        .map(|(c, _)| cell_of(c))
        .collect()
}

/// The cells outside `inside` that are edge neighbours of a cell inside it, ascending.
fn rim(graph: &CellGraph, inside: &[bool]) -> Vec<u32> {
    (0..graph.len())
        .filter(|&c| !inside[slot(c)] && graph.edge_neighbours(c).iter().any(|&m| inside[slot(m)]))
        .collect()
}

/// Whether the cells `cells` are connected through edge neighbours among themselves.
fn connected(graph: &CellGraph, cells: &[u32]) -> bool {
    let mut inside = vec![false; slot(graph.len())];
    for &c in cells {
        inside[slot(c)] = true;
    }
    let Some(&first) = cells.first() else {
        return true;
    };
    let mut seen = vec![false; inside.len()];
    seen[slot(first)] = true;
    let mut queue = vec![first];
    let mut count = 0;
    while let Some(c) = queue.pop() {
        count += 1;
        for m in graph.edge_neighbours(c) {
            if inside[slot(m)] && !seen[slot(m)] {
                seen[slot(m)] = true;
                queue.push(m);
            }
        }
    }
    count == cells.len()
}

/// The sum of `values` over `cells`, in their order.
fn sum_over(values: &[f64], cells: &[u32]) -> f64 {
    cells.iter().map(|&c| values[slot(c)]).sum()
}
