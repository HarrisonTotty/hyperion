//! The fixed point of routing and solution, accelerated by multigrid (R09.T14.b; Tzathas et al.
//! 2024, CGF 43(2), doi:10.1111/cgf.15033, §5.1–5.2; Design note 9; decision-r09-t14-choices Q1).
//!
//! The analytical solution ([`solver`]) needs a river network, and the network is
//! drawn on the elevations the solution gives, so the two are iterated to a fixed point (§5.1):
//! route on z, solve on that routing, and move z halfway to the solution, the moving average of
//! weight ½ (Design note 9's, recorded: the paper averages "the topography predicted … with the
//! elevations resulting from the previous iteration" without a weight). The basins' edges move a
//! cell an iteration, so the iteration runs coarse to fine (§5.2) on the quadtree's own parents,
//! from [`MULTIGRID_FLOOR_LEVEL`] to the field's level, [`ITERATIONS_PER_LEVEL`] at each.
//!
//! - **Restriction**, from level k + 1 to k (a parent's children are 4n to 4n + 3): the
//!   construction z0, the runoff and K by the means of the children weighted by their solid angles,
//!   summed in child order; a water volume or inflow by the sum; a net loss rate by the mean; and
//!   the sea where at least half the parent's solid angle is sea and its ground is at or below the
//!   sea's level (so that a coarse sea cell is a valid flood seed).
//! - **Once per level, on its construction z0** (decision-r09-t14-choices Q1–Q3): the water bodies
//!   of the level's water, Fill–Spill–Merge's ([`depressions::water_bodies`]), every depression a
//!   dry, terminal playa where there is none; the bounds, the sea's cells (base: the sea's level),
//!   the bodies' flooded cells, lake beds (base: the body's level), and each terminal body's pit,
//!   a root of every routing (base: its level, a playa's being its floor), their ground untouched;
//!   and, for [`Uplift::BasinFreeboard`], the construction's own routing, which fixes each cell's
//!   basin b, the root it drains to, and the basin's base level `ℓ_b`.
//! - **Each iteration**: the routing with water on the current z ([`route_with_water`]) with the
//!   level's bodies; the equivalent area, the runoff times the cells' area accumulated and over
//!   [`REFERENCE_RUNOFF_M_PER_YEAR`] (Hergarten 2021, eqs. 16 and 20); the solve; for
//!   [`Uplift::BasinFreeboard`], each basin's `λ_b` refitted on it, so that the basin keeps its
//!   construction's mean height, a basin with no response keeping its construction; the residual,
//!   the root mean square by area of the solution less z over the cells not bound; and z ← ½ (z +
//!   s). On the finest level the last iteration's output is s itself, not the
//!   average, so that the final elevation is an analytic solution on its routing (the lane's
//!   choice).
//! - **The upsample** from level k to k + 1: R09.T4's interpolant
//!   ([`interpolate`]) of level k's z, each bound at its base (so that a sea floor's depth does not
//!   pull the coast down), at each child's centre jittered by a uniform offset in [−¼, ¼]² of the
//!   child cell (§5.2: "a uniform random offset in [−0.25, 0.25]² is added to the locations of the
//!   nodes before interpolation"), two uniforms on `surface.coarse.erosion` at instance
//!   [`JITTER_INSTANCE`] keyed by the child; then each bound back at its ground. The interpolant
//!   approximates (a cell's centre takes 4/9 of its own value), so the upsample smooths: recorded.
//!
//! Implemented from the paper alone; no code is copied or paraphrased from Tzathas et al.'s
//! repository (Inria research-only licence).

use std::borrow::Cow;

use hyperion_surface::cube::{FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use hyperion_surface::field::cell_index;
use hyperion_surface::num;
use hyperion_surface::synth::interp::{CellValues, interpolate};
use hyperion_surface::tags::SURFACE_COARSE_EROSION;

use super::super::plates::{cell_of, slot};
use super::depressions::{self, Measure, WaterBodies, route_with_water};
use super::drainage::{self, Routing, receiver_uniforms};
use super::mesh::Mesh;
use super::solver::{self, Cells, Time};
use crate::planetary::hooks::SurfaceSeed;
use crate::planetary::surface::grid::CellGraph;
use crate::rng::ObjectKey;
use crate::units::Metres;

/// The coarsest level of the multigrid: 4, 1,536 cells (Design note 9).
pub(crate) const MULTIGRID_FLOOR_LEVEL: u8 = 4;

/// The fixed point's iterations at each level: 6 (Design note 9, after Tzathas et al. 2024, §5.2:
/// "we mostly use around 4 − 5 levels and 5 − 6 iterations").
pub(crate) const ITERATIONS_PER_LEVEL: usize = 6;

/// The moving average's weight of the solution against the previous elevation: ½ (Design note 9,
/// recorded; Tzathas et al. 2024, §5.1, average the two without giving a weight).
pub(crate) const MOVING_AVERAGE_WEIGHT: f64 = 0.5;

/// The reference runoff `p₀` that turns a discharge into an equivalent area, `A_eq` = Q ÷ `p₀`: 1
/// m a⁻¹ (Hergarten 2021, Earth Surf. Dynam. 9, 937, eq. 16; Design note 9).
pub(crate) const REFERENCE_RUNOFF_M_PER_YEAR: f64 = 1.0;

/// The half-width of the upsample's jitter, in cells of the finer level: ¼ (Tzathas et al. 2024,
/// §5.2, "a uniform random offset in [−0.25, 0.25]²").
pub(crate) const JITTER_HALF_WIDTH: f64 = 0.25;

/// The instance of each cell's upsample jitter on `surface.coarse.erosion`: two uniforms a cell a
/// level, s then t, keyed by the cell's own face, level, i and j.
pub(crate) const JITTER_INSTANCE: u16 = 1;

/// The rock uplift (decision-r09-t14-choices Q1), which the caller's branch chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Uplift {
    /// None: the construction is "the initial surface rather than a rate" (a world dry now).
    None,
    /// u = `λ_b` max(0, z0 − `ℓ_b`) on the cells not bound, b each cell's basin on the level's
    /// construction routing and `ℓ_b` its base level, `λ_b` refitted at every iteration so that
    /// each basin keeps its construction's mean height above `ℓ_b` (a world wet now).
    BasinFreeboard,
}

/// The water Fill–Spill–Merge spreads on each level's construction, with each cell's input in
/// cell-index order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Water<'a> {
    /// None: every depression a dry, terminal playa (a world wet now until R09.T14.d's balance).
    None,
    /// A volume, m³ a cell ([`Measure::Volume`]; a paleo-inventory, decision-r09-t14-choices Q2).
    Volume {
        /// Each cell's water, m³.
        water_m3: &'a [f64],
    },
    /// A water balance ([`Measure::Balance`]; lakes on a world wet now, decision-r09-t14-choices
    /// Q3).
    Balance {
        /// Each cell's own inflow, m³ a⁻¹.
        inflow_m3_per_year: &'a [f64],
        /// Each cell's net loss over open liquid, m a⁻¹.
        net_loss_m_per_year: &'a [f64],
    },
}

/// The erosion's problem on the field's level, which the caller's branch builds (R09.T14.c): each
/// per-cell input one a cell in cell-index order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Problem<'a> {
    /// The surface seed, whose `surface.coarse.erosion` draws the receivers and the jitter.
    pub(crate) seed: SurfaceSeed,
    /// The body's radius.
    pub(crate) radius: Metres,
    /// The field's cells, the finest level.
    pub(crate) grid: &'a CellGraph,
    /// The cells one level up (the pass's climate grid), if the caller has them; else built.
    pub(crate) parent_grid: Option<&'a CellGraph>,
    /// The construction z0, the elevation entering the step, metres: finite.
    pub(crate) z0_m: &'a [f64],
    /// Each cell's own runoff, m a⁻¹: finite and non-negative.
    pub(crate) runoff_m_per_year: &'a [f64],
    /// Each cell's erodibility K, m^0.1 a⁻¹: finite and positive.
    pub(crate) erodibility_m0_1_per_year: &'a [f64],
    /// The sea's cells, each with its ground at or below the sea's level.
    pub(crate) sea: &'a [bool],
    /// The sea's level, metres: on a world with no sea, any finite value.
    pub(crate) sea_level_m: f64,
    /// The water on the land.
    pub(crate) water: Water<'a>,
    /// The rock uplift.
    pub(crate) uplift: Uplift,
    /// When the solution is taken.
    pub(crate) time: Time,
}

/// One level's residuals: at each iteration, the root mean square by area of the iteration's
/// solution less the elevation it started from, over the cells not bound, metres.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LevelResidual {
    /// The level.
    pub(crate) level: u8,
    /// Each iteration's residual, metres, in iteration order.
    pub(crate) by_iteration_m: Vec<f64>,
}

impl LevelResidual {
    /// The level's last residual, metres.
    ///
    /// # Panics
    ///
    /// Never: a level iterates [`ITERATIONS_PER_LEVEL`] times.
    #[must_use]
    pub(crate) fn last_m(&self) -> f64 {
        *self
            .by_iteration_m
            .last()
            .expect("a level iterates at least once")
    }
}

/// The distribution of the basins' `λ_b` at the finest level's last iteration, a⁻¹: over the basins
/// fitted (those with Σ Ω F above zero), unweighted, by nearest rank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LambdaSpread {
    /// The basins fitted.
    pub(crate) basins: usize,
    /// The least.
    pub(crate) min: f64,
    /// The 10th, 25th, 50th, 75th and 90th percentiles.
    pub(crate) percentiles: [f64; 5],
    /// The greatest.
    pub(crate) max: f64,
}

/// What the multigrid records (decision-r09-t14-choices Q1's records).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Diagnostics {
    /// Each level's residuals, coarsest first.
    pub(crate) residuals: Vec<LevelResidual>,
    /// `λ_b`'s distribution at the finest level, for [`Uplift::BasinFreeboard`] with a basin
    /// fitted.
    pub(crate) lambda: Option<LambdaSpread>,
    /// The implied mean uplift over the land (the cells off the sea), Σ Ω u ÷ Σ Ω at the finest
    /// level's last iteration, m a⁻¹: the steady state's denudation, against Earth's measured 0.05–
    /// 0.1 mm a⁻¹ (Milliman and Meade 1983, from memory; recorded, not asserted). Zero without
    /// uplift.
    pub(crate) mean_uplift_m_per_year: f64,
}

/// The multigrid's result on the field's level.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Outcome {
    /// The final elevation, metres, the last iteration's solution on [`routing`](Self::routing).
    pub(crate) z_m: Vec<f64>,
    /// The field's mesh.
    pub(crate) mesh: Mesh,
    /// The last iteration's routing, on whose network the final elevation is the solution.
    pub(crate) routing: Routing,
    /// The geometric drainage area on that routing, m², its own included.
    pub(crate) drainage_m2: Vec<f64>,
    /// The equivalent area on that routing, m².
    pub(crate) equivalent_area_m2: Vec<f64>,
    /// The water bodies on the field's construction.
    pub(crate) bodies: WaterBodies,
    /// The records.
    pub(crate) diagnostics: Diagnostics,
}

/// One level's owned water inputs.
#[derive(Debug, Clone, PartialEq)]
enum LevelWater {
    None,
    Volume(Vec<f64>),
    Balance(Vec<f64>, Vec<f64>),
}

/// One level's inputs.
struct Level<'a> {
    graph: Cow<'a, CellGraph>,
    mesh: Mesh,
    z0_m: Vec<f64>,
    runoff_m_per_year: Vec<f64>,
    erodibility_m0_1_per_year: Vec<f64>,
    sea: Vec<bool>,
    water: LevelWater,
}

impl Level<'_> {
    /// The parent level of `self`, on `graph` (one level up), each parent from its four children.
    fn restrict<'g>(
        &self,
        graph: Cow<'g, CellGraph>,
        radius: Metres,
        sea_level_m: f64,
    ) -> Level<'g> {
        let fine = &self.graph;
        let n = slot(graph.len());
        assert_eq!(
            n * 4,
            slot(fine.len()),
            "a parent level has a quarter of the cells"
        );
        let (omega, _) = fine.solid_angles_sr().as_chunks::<4>();
        let mean = |values: &[f64]| -> Vec<f64> {
            omega
                .iter()
                .zip(values.as_chunks::<4>().0)
                .map(|(weights, children)| {
                    let (mut weight, mut sum) = (0.0, 0.0);
                    for (&w, &x) in weights.iter().zip(children) {
                        weight += w;
                        sum += w * x;
                    }
                    sum / weight
                })
                .collect()
        };
        let total = |values: &[f64]| -> Vec<f64> {
            values
                .as_chunks::<4>()
                .0
                .iter()
                .map(|children| children.iter().sum())
                .collect()
        };
        let z0_m = mean(&self.z0_m);
        let sea = omega
            .iter()
            .zip(self.sea.as_chunks::<4>().0)
            .zip(&z0_m)
            .map(|((weights, children), &ground)| {
                let (mut wet, mut all) = (0.0, 0.0);
                for (&w, &child) in weights.iter().zip(children) {
                    all += w;
                    if child {
                        wet += w;
                    }
                }
                2.0 * wet >= all && ground.total_cmp(&sea_level_m).is_le()
            })
            .collect();
        let water = match &self.water {
            LevelWater::None => LevelWater::None,
            LevelWater::Volume(w) => LevelWater::Volume(total(w)),
            LevelWater::Balance(inflow, loss) => LevelWater::Balance(total(inflow), mean(loss)),
        };
        let mesh = Mesh::of(&graph, radius);
        Level {
            mesh,
            runoff_m_per_year: mean(&self.runoff_m_per_year),
            erodibility_m0_1_per_year: mean(&self.erodibility_m0_1_per_year),
            z0_m,
            sea,
            water,
            graph,
        }
    }
}

/// What a level fixes once, on its construction.
struct Setup {
    bodies: WaterBodies,
    /// Each cell's bound: its base, or `None` where it erodes.
    fixed: Vec<Option<f64>>,
    uniforms: Vec<f64>,
    /// Each cell's uplift at λ = 1 a⁻¹: its freeboard above its basin's base level, metres, or 0.
    freeboard_m: Vec<f64>,
    /// Each cell's basin, its root on the construction's routing, for [`Uplift::BasinFreeboard`].
    basin: Vec<u32>,
}

impl Setup {
    /// The setup of `level` for `problem` (see the module documentation).
    fn of(level: &Level<'_>, problem: &Problem<'_>) -> Self {
        let n = slot(level.mesh.len());
        let zeros;
        let measure = match &level.water {
            LevelWater::None => {
                zeros = vec![0.0; n];
                Measure::Volume { water_m3: &zeros }
            }
            LevelWater::Volume(water_m3) => Measure::Volume { water_m3 },
            LevelWater::Balance(inflow, loss) => Measure::Balance {
                inflow_m3_per_year: inflow,
                net_loss_m_per_year: loss,
            },
        };
        let bodies = depressions::water_bodies(&level.mesh, &level.z0_m, &level.sea, measure);
        let fixed = bounds(&level.sea, problem.sea_level_m, &bodies);
        let uniforms = receiver_uniforms(problem.seed, &level.graph);
        let (freeboard_m, basin) = match problem.uplift {
            Uplift::None => (vec![0.0; n], Vec::new()),
            Uplift::BasinFreeboard => {
                let routing = route_with_water(
                    &level.mesh,
                    &level.z0_m,
                    &level.sea,
                    problem.sea_level_m,
                    &bodies,
                    &uniforms,
                );
                basin_freeboard(&routing, &level.z0_m, &fixed)
            }
        };
        Self {
            bodies,
            fixed,
            uniforms,
            freeboard_m,
            basin,
        }
    }
}

/// Each cell's bound: the sea's cells at `sea_level_m`, each body's flooded cells at its level,
/// and each terminal body's pit at its level (a root of every routing with these bodies).
fn bounds(sea: &[bool], sea_level_m: f64, bodies: &WaterBodies) -> Vec<Option<f64>> {
    let mut fixed: Vec<Option<f64>> = sea.iter().map(|&wet| wet.then_some(sea_level_m)).collect();
    for body in bodies.bodies() {
        for &c in &body.cells {
            fixed[slot(c)] = Some(body.level_m);
        }
        if let Some(level_m) = body.base_level_m() {
            fixed[slot(body.pit)] = Some(level_m);
        }
    }
    fixed
}

/// Each cell's freeboard max(0, z0 − `ℓ_b`) on the construction's routing `routing`, 0 on a bound,
/// and its basin, the root it drains to (decision-r09-t14-choices Q1's u = `λ_b` max(0, z0 −
/// `ℓ_b`) at λ = 1 a⁻¹, b keyed by the root's cell): the uplift's pattern is fixed in space for the
/// level's iterations.
fn basin_freeboard(routing: &Routing, z0_m: &[f64], fixed: &[Option<f64>]) -> (Vec<f64>, Vec<u32>) {
    (0..routing.len())
        .map(|c| {
            let ci = slot(c);
            let freeboard = if fixed[ci].is_some() {
                0.0
            } else {
                num::max(0.0, z0_m[ci] - routing.base_level_m(c))
            };
            (freeboard, routing.root_of(c))
        })
        .unzip()
}

/// Level `level`'s values as R09.T4's interpolant reads them.
struct LevelValues<'a> {
    level: u8,
    values: &'a [f64],
}

impl CellValues for LevelValues<'_> {
    fn level(&self) -> u8 {
        self.level
    }

    fn value(&self, cell: PatchKey) -> Option<f64> {
        if cell.level() == self.level {
            self.values.get(slot(cell_index(cell))).copied()
        } else {
            None
        }
    }
}

/// Each cell of `graph`'s two jitter uniforms in [0, 1), s then t, from its own stream on
/// `surface.coarse.erosion` at instance [`JITTER_INSTANCE`], keyed by its `surface_cell`.
///
/// # Panics
///
/// Never: every cell of a coarse level is a surface cell.
#[must_use]
pub(crate) fn jitter_uniforms(seed: SurfaceSeed, graph: &CellGraph) -> Vec<[f64; 2]> {
    graph
        .cells()
        .iter()
        .map(|cell| {
            let mut stream = seed.stream(
                SURFACE_COARSE_EROSION,
                ObjectKey::surface_cell(
                    cell.face().index(),
                    cell.level(),
                    cell.i(),
                    cell.j(),
                    JITTER_INSTANCE,
                )
                .expect("a coarse cell is a surface cell"),
            );
            let s = stream.uniform();
            let t = stream.uniform();
            [s, t]
        })
        .collect()
}

/// `values`, one a cell of `coarse`, upsampled to `fine`, one level deeper: R09.T4's interpolant
/// at each fine cell's centre jittered by up to [`JITTER_HALF_WIDTH`] of the cell on each axis,
/// from its [`jitter_uniforms`].
///
/// # Panics
///
/// If `fine` is not one level below `coarse`, `values` is not one a coarse cell, or a value is not
/// finite.
#[must_use]
pub(crate) fn upsample(
    seed: SurfaceSeed,
    coarse: &CellGraph,
    values: &[f64],
    fine: &CellGraph,
) -> Vec<f64> {
    assert_eq!(fine.level(), coarse.level() + 1, "one level deeper");
    assert_eq!(values.len(), slot(coarse.len()), "one value a coarse cell");
    for &v in values {
        // Asserted for its panic: a height that is not finite is a solver's fault.
        let _ = num::assert_finite(v);
    }
    let read = LevelValues {
        level: coarse.level(),
        values,
    };
    let side = f64::from(1_u32 << fine.level());
    fine.cells()
        .iter()
        .zip(jitter_uniforms(seed, fine))
        .map(|(cell, [u1, u2])| {
            let ds = 2.0 * JITTER_HALF_WIDTH * (u1 - 0.5);
            let dt = 2.0 * JITTER_HALF_WIDTH * (u2 - 0.5);
            let s = (f64::from(cell.i()) + 0.5 + ds) / side;
            let t = (f64::from(cell.j()) + 0.5 + dt) / side;
            let dir = unit_dir(face_uv_to_xyz(FaceUv {
                face: cell.face(),
                u: st_to_uv(s),
                v: st_to_uv(t),
            }));
            interpolate(&read, dir)
                .expect("the coarse level holds every cell")
                .value
        })
        .collect()
}

/// The basins' `λ_b` on `solution`, a⁻¹, keyed by root, and whether each basin was fitted: `λ_b` =
/// `Σ_b` Ω (z0 − base) ÷ `Σ_b` Ω F over the basin's cells not bound and of equivalent area above zero,
/// in cell-index order, each value asserted finite; held at zero or above.
///
/// Lean, pending decision-r09-t14-choices Q1: the ruling's `Σ_b` Ω (z0 − `ℓ_b`) with each cell's own
/// base in place of `ℓ_b`, which it is wherever a cell drains to its basin's root through no bound
/// (design §4.2's form): it holds the basin's mean height above `ℓ_b` exactly also where a cell
/// drains through a full lake or, at a later iteration, to another basin's root.
fn basin_lambdas(
    mesh: &Mesh,
    z0_m: &[f64],
    setup: &Setup,
    equivalent_area_m2: &[f64],
    solution: &solver::Solution,
) -> (Vec<f64>, Vec<bool>) {
    let n = z0_m.len();
    let (mut excess, mut response) = (vec![0.0; n], vec![0.0; n]);
    for c in 0..n {
        if setup.fixed[c].is_some() || equivalent_area_m2[c] <= 0.0 {
            continue;
        }
        let b = slot(setup.basin[c]);
        let a = mesh.area_m2(cell_of(c));
        excess[b] += a * num::assert_finite(z0_m[c] - solution.base_m()[c]);
        response[b] += a * num::assert_finite(solution.per_lambda_m()[c]);
    }
    let fitted: Vec<bool> = response.iter().map(|&f| f > 0.0).collect();
    let lambda = excess
        .iter()
        .zip(&response)
        .map(|(&e, &f)| if f > 0.0 { num::max(0.0, e / f) } else { 0.0 })
        .collect();
    (lambda, fitted)
}

/// The distribution of `values`, each finite, or `None` if it is empty: the p-th percentile the
/// value of rank round(p × (count − 1) ÷ 100) in ascending order (by `total_cmp`).
fn spread(mut values: Vec<f64>) -> Option<LambdaSpread> {
    let last = values.len().checked_sub(1)?;
    for &v in &values {
        // Asserted for its panic before the sort: a value that is not finite is a fit's fault.
        let _ = num::assert_finite(v);
    }
    values.sort_unstable_by(f64::total_cmp);
    let rank = |percent: usize| values[(percent * last + 50) / 100];
    Some(LambdaSpread {
        basins: values.len(),
        min: values[0],
        percentiles: [rank(10), rank(25), rank(50), rank(75), rank(90)],
        max: values[last],
    })
}

/// The multigrid's levels for `problem`, coarsest first: the field's own, and its parents down to
/// [`MULTIGRID_FLOOR_LEVEL`], each restricted from the level below it.
///
/// # Panics
///
/// As [`erode`].
fn levels<'a>(problem: &Problem<'a>) -> Vec<Level<'a>> {
    let finest = problem.grid.level();
    let n = slot(problem.grid.len());
    for (what, len) in [
        ("height", problem.z0_m.len()),
        ("runoff", problem.runoff_m_per_year.len()),
        ("erodibility", problem.erodibility_m0_1_per_year.len()),
        ("sea flag", problem.sea.len()),
    ] {
        assert_eq!(len, n, "one {what} a cell");
    }
    assert!(
        problem
            .runoff_m_per_year
            .iter()
            .all(|r| r.is_finite() && *r >= 0.0),
        "each runoff is finite and non-negative"
    );
    let sea_level_m = num::assert_finite(problem.sea_level_m);
    let water = match problem.water {
        Water::None => LevelWater::None,
        Water::Volume { water_m3 } => LevelWater::Volume(water_m3.to_vec()),
        Water::Balance {
            inflow_m3_per_year,
            net_loss_m_per_year,
        } => LevelWater::Balance(inflow_m3_per_year.to_vec(), net_loss_m_per_year.to_vec()),
    };
    let mut levels = vec![Level {
        graph: Cow::Borrowed(problem.grid),
        mesh: Mesh::of(problem.grid, problem.radius),
        z0_m: problem
            .z0_m
            .iter()
            .map(|&z| num::assert_finite(z))
            .collect(),
        runoff_m_per_year: problem.runoff_m_per_year.to_vec(),
        erodibility_m0_1_per_year: problem.erodibility_m0_1_per_year.to_vec(),
        sea: problem.sea.to_vec(),
        water,
    }];
    for k in (MULTIGRID_FLOOR_LEVEL.min(finest)..finest).rev() {
        let graph = match problem.parent_grid {
            Some(grid) if k + 1 == finest => {
                assert_eq!(grid.level(), k, "the parent grid is one level up");
                Cow::Borrowed(grid)
            }
            _ => Cow::Owned(CellGraph::new(k).expect("a multigrid level is a coarse level")),
        };
        let next = levels.last().expect("the finest level is first").restrict(
            graph,
            problem.radius,
            sea_level_m,
        );
        levels.push(next);
    }
    levels.reverse();
    levels
}

/// The level below the one starting, coarser, as it finished.
struct Finished<'a> {
    /// Its cells.
    graph: &'a CellGraph,
    /// Its last elevation, metres.
    z_m: &'a [f64],
    /// Its bounds and their bases.
    fixed: &'a [Option<f64>],
}

/// The start of `level`'s iterations: its construction at the coarsest level; else the last
/// elevation of the level below it, each of its bounds at its base, upsampled ([`upsample`]); and
/// in either case each of the level's own bounds `bounds` at its ground.
fn start(
    seed: SurfaceSeed,
    level: &Level<'_>,
    bounds: &[Option<f64>],
    below: Option<Finished<'_>>,
) -> Vec<f64> {
    let mut z = match below {
        None => level.z0_m.clone(),
        Some(coarse) => {
            let surface: Vec<f64> = coarse
                .z_m
                .iter()
                .zip(coarse.fixed)
                .map(|(&h, bound)| bound.unwrap_or(h))
                .collect();
            upsample(seed, coarse.graph, &surface, &level.graph)
        }
    };
    for (h, (bound, &ground)) in z.iter_mut().zip(bounds.iter().zip(&level.z0_m)) {
        if bound.is_some() {
            *h = ground;
        }
    }
    z
}

/// The multigrid's fixed point for `problem` (see the module documentation).
///
/// # Panics
///
/// If an input is not one a cell of the problem's grid, a value is not finite, a runoff is
/// negative, an erodibility is not positive, a sea cell's ground lies above the sea's level, or the
/// parent grid is not one level up.
#[must_use]
pub(crate) fn erode(problem: &Problem<'_>) -> Outcome {
    let levels = levels(problem);
    let mut residuals = Vec::with_capacity(levels.len());
    let mut z: Vec<f64> = Vec::new();
    let mut below: Option<(usize, Vec<Option<f64>>)> = None;
    let mut finished = None;
    for (at, level) in levels.iter().enumerate() {
        let setup = Setup::of(level, problem);
        let coarse = below.as_ref().map(|(k, fixed)| Finished {
            graph: &levels[*k].graph,
            z_m: &z,
            fixed,
        });
        z = start(problem.seed, level, &setup.fixed, coarse);
        let finest = at + 1 == levels.len();
        let mut by_iteration_m = Vec::with_capacity(ITERATIONS_PER_LEVEL);
        let mut last = None;
        for iteration in 0..ITERATIONS_PER_LEVEL {
            let step = iterate(level, &setup, &z, problem);
            by_iteration_m.push(step.residual_m);
            if finest && iteration + 1 == ITERATIONS_PER_LEVEL {
                z.clone_from(&step.solution_m);
                last = Some(step);
            } else {
                for (h, &s) in z.iter_mut().zip(&step.solution_m) {
                    *h += MOVING_AVERAGE_WEIGHT * (s - *h);
                }
            }
        }
        residuals.push(LevelResidual {
            level: level.graph.level(),
            by_iteration_m,
        });
        match last {
            Some(step) => finished = Some((setup, step)),
            None => below = Some((at, setup.fixed)),
        }
    }
    let (setup, step) = finished.expect("the multigrid's finest level iterates");
    let mesh = levels
        .last()
        .expect("the multigrid has a finest level")
        .mesh
        .clone();
    let drainage_m2 = drainage::accumulate(&step.routing, mesh.areas_m2());
    Outcome {
        z_m: z,
        mesh,
        routing: step.routing,
        drainage_m2,
        equivalent_area_m2: step.equivalent_area_m2,
        bodies: setup.bodies,
        diagnostics: Diagnostics {
            residuals,
            lambda: step.lambda,
            mean_uplift_m_per_year: step.mean_uplift_m_per_year,
        },
    }
}

/// One iteration's results.
struct Iteration {
    routing: Routing,
    equivalent_area_m2: Vec<f64>,
    solution_m: Vec<f64>,
    residual_m: f64,
    lambda: Option<LambdaSpread>,
    mean_uplift_m_per_year: f64,
}

/// One iteration of the fixed point on `level` from the elevation `z`: route, accumulate, solve,
/// scale the uplift, and measure the residual.
fn iterate(level: &Level<'_>, setup: &Setup, z: &[f64], problem: &Problem<'_>) -> Iteration {
    let mesh = &level.mesh;
    let routing = route_with_water(
        mesh,
        z,
        &level.sea,
        problem.sea_level_m,
        &setup.bodies,
        &setup.uniforms,
    );
    let discharge: Vec<f64> = level
        .runoff_m_per_year
        .iter()
        .zip(mesh.areas_m2())
        .map(|(&r, &a)| r * a)
        .collect();
    let equivalent_area_m2: Vec<f64> = drainage::accumulate(&routing, &discharge)
        .into_iter()
        .map(|q| q / REFERENCE_RUNOFF_M_PER_YEAR)
        .collect();
    let cells = Cells {
        z0_m: &level.z0_m,
        uplift_m_per_year: &setup.freeboard_m,
        erodibility_m0_1_per_year: &level.erodibility_m0_1_per_year,
        equivalent_area_m2: &equivalent_area_m2,
        fixed: &setup.fixed,
    };
    let solution = solver::solve(&routing, &cells, problem.time);
    let n = z.len();
    let (solution_m, lambda, mean_uplift_m_per_year) = match problem.uplift {
        Uplift::None => {
            let s: Vec<f64> = (0..n)
                .map(|c| solution.elevation_m(cell_of(c), 0.0))
                .collect();
            (s, None, 0.0)
        }
        Uplift::BasinFreeboard => {
            let (lambdas, fitted) =
                basin_lambdas(mesh, &level.z0_m, setup, &equivalent_area_m2, &solution);
            let s: Vec<f64> = (0..n)
                .map(|c| {
                    let b = slot(setup.basin[c]);
                    if fitted[b] {
                        solution.elevation_m(cell_of(c), lambdas[b])
                    } else {
                        level.z0_m[c]
                    }
                })
                .collect();
            let (mut uplift, mut land) = (0.0, 0.0);
            for c in 0..n {
                if level.sea[c] {
                    continue;
                }
                let a = mesh.area_m2(cell_of(c));
                land += a;
                uplift += a * lambdas[slot(setup.basin[c])] * setup.freeboard_m[c];
            }
            let roots_fitted: Vec<f64> =
                (0..n).filter(|&b| fitted[b]).map(|b| lambdas[b]).collect();
            let mean = if land > 0.0 { uplift / land } else { 0.0 };
            (s, spread(roots_fitted), mean)
        }
    };
    let (mut squares, mut weight) = (0.0, 0.0);
    for c in 0..n {
        if setup.fixed[c].is_some() {
            continue;
        }
        let a = mesh.area_m2(cell_of(c));
        let d = num::assert_finite(solution_m[c] - z[c]);
        squares += a * d * d;
        weight += a;
    }
    let residual_m = if weight > 0.0 {
        (squares / weight).sqrt()
    } else {
        0.0
    };
    Iteration {
        routing,
        equivalent_area_m2,
        solution_m,
        residual_m,
        lambda,
        mean_uplift_m_per_year,
    }
}
