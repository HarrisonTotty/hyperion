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
//!   deposition and the linearity in the uplift's scale, on hand-built networks (`solver`); and
//!   the residual falling over the multigrid's levels on a synthetic world, the final elevation
//!   above its receivers on its last routing, the jitter's draws and the upsample of a constant
//!   field, with the measurement of the three starts of a finer level (`multigrid`).
//! - R09.T14.c's of the branches and the Mars check: the branch and the share of the epoch, a
//!   surface younger than its epoch's end and a world never wet untouched, the young surface
//!   eroded without runoff of its own, the paleo-water stored, the outputs (`branches`); F1's
//!   trunk incision against Black et al.'s, the sub-cell volume pinned to the synthesis's closed
//!   form, the ceiling's measurement and the slow records (`mars`); and the reference Earth's slow
//!   record of the steady state (`earth`).
//!
//! Each world the branches' tests stage ([`Staged`]) runs the pass's steps before the erosion in
//! the pass's order, under the reference seed.

use hyperion_surface::field::{CoarseLevel, coarse_level};
use hyperion_testkit::lcg::Lcg;

use super::super::{Pass, Working, climate, craters, plates, relief};
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
}

/// Prints `outcome`'s records under `what`: each level's first and last residual, its residuals by
/// iteration, its largest change and the receivers its last two routings changed; `λ_b`'s
/// distribution; and the implied uplift.
fn report(what: &str, outcome: &Outcome) {
    let d = &outcome.diagnostics;
    for r in &d.residuals {
        let each: Vec<String> = r.by_iteration_m.iter().map(|x| format!("{x:.1}")).collect();
        eprintln!(
            "{what}: level {} residual {:.3} → {:.3} m (by iteration: {} m), largest change {:.1} m, \
             {} receivers changed",
            r.level,
            r.first_m(),
            r.last_m(),
            each.join(", "),
            r.largest_change_m(),
            r.receivers_changed
        );
    }
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

/// The cells of `outcome` not bound that do not lie strictly above their receiver on its last
/// routing, ascending: those whose receiver lies in their own basin of the construction routing
/// (the uplift's partition, decision-r09-t14-choices Q1), then those whose receiver lies across
/// its edge. With no uplift every cell is taken as one basin's.
fn not_above_receivers(outcome: &Outcome) -> (Vec<u32>, Vec<u32>) {
    let basin = |c: u32| outcome.basins.get(slot(c)).copied().unwrap_or(0);
    (0..outcome.routing.len())
        .filter_map(|c| {
            let link = outcome.routing.receiver(c)?;
            let below = outcome.bounds[slot(c)].is_none()
                && outcome.z_m[slot(c)] <= outcome.z_m[slot(link.cell)];
            below.then_some((c, basin(c) == basin(link.cell)))
        })
        .fold(
            (Vec::new(), Vec::new()),
            |(mut within, mut across), (c, same)| {
                if same {
                    within.push(c);
                } else {
                    across.push(c);
                }
                (within, across)
            },
        )
}

/// The final elevation of `outcome` is the solve on its last routing (decision-r09-t14-choices,
/// "T14.b follow-up" item 5): every cell not bound lies strictly above its receiver on that
/// routing where the two lie in one basin of the construction, whose `λ_b` they share. Where a
/// cell's receiver lies across the edge of its construction basin, the two take different `λ_b`
/// and the cell may lie below it: those cells are printed under `what`, a record, since the
/// ruling's "s rises monotonically up every path" holds only within one `λ_b`.
///
/// # Panics
///
/// If a cell not bound lies at or below its receiver in its own construction basin.
fn assert_above_receivers(what: &str, outcome: &Outcome) {
    let (within, across) = not_above_receivers(outcome);
    eprintln!(
        "{what}: {} cells not bound lie at or below their receiver across a construction basin's \
         edge (of {} cells), the first {:?}",
        across.len(),
        outcome.routing.len(),
        &across[..across.len().min(8)]
    );
    assert!(
        within.is_empty(),
        "{what}: {} cells not bound lie at or below their receiver in their own construction \
         basin, the first {:?}",
        within.len(),
        &within[..within.len().min(8)]
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
