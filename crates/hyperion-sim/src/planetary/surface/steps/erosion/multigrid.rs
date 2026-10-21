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
//!   the terminal bodies' flooded cells, lake beds (base: the body's level), and each terminal
//!   body's pit, a root of every routing (base: its level, a playa's being its floor), their
//!   ground untouched, and a full body's flooded cells as the problem's [`FullBodies`] says:
//!   bounds at the body's level, or routed through on the filled surface and solved like any
//!   other cell; and, for [`Uplift::BasinFreeboard`], the construction's own routing, which fixes
//!   each cell's basin b, the root it drains to, and the basin's base level `ℓ_b`, a full body
//!   draining through its outlet to the base downstream of it.
//! - **Each iteration**: the routing with water on the current z ([`route_with_water`]) with the
//!   level's bodies; the equivalent area, the runoff times the cells' area accumulated and over
//!   [`REFERENCE_RUNOFF_M_PER_YEAR`] (Hergarten 2021, eqs. 16 and 20); the solve; the residual,
//!   the root mean square by area of the solution less z over the cells not bound; and z ← ½ (z +
//!   s). On the finest level the last iteration's output is s itself, not the average, so that
//!   the final elevation is an analytic solution on its routing (the lane's choice).
//! - **The uplift as a field** for [`Uplift::BasinFreeboard`] (decision-r09-t14-choices Q1, as
//!   its "T14.c follow-up" item 1 rules it): the rock uplift is fixed in space, u(i) = `λ_b(i)`
//!   max(0, z0(i) − `ℓ_b(i)`), each cell's own construction basin's `λ_b` applied at that cell, so
//!   that the solve is the path integral of a non-negative field (Tzathas et al. 2024's eq. 9
//!   under an uplift that "varies in space but not in time", §4.1), each link's rise held to the
//!   problem's [`Threshold`]. Each level's first iteration takes `λ_b` from Q1's closed form,
//!   `Σ_b` Ω (z0 − `ℓ_b`) ÷ `Σ_b` Ω F on a solve at λ = 1 ([`closed_form`]), exact only where every
//!   path stays in its basin. After each solve each construction basin's mean elevation, which no
//!   base enters, is held by an additive (Jacobi) step ([`refit`]; decision-r09-t14-choices, "T14.c
//!   follow-up 2" §1): `λ_b` ← max(0, `λ_b` + ½ `Σ_b` Ω (z0 − s) ÷ `S_b`), `S_b` the basin's own
//!   sensitivity along the part of each path that lies in it, a basin of `S_b` zero keeping its
//!   `λ_b`. The finest level's last iteration refits once more and solves again on the same
//!   routing, so that the field takes the `λ_b` refitted on its own routing (the lane's choice).
//! - **The start of each finer level** ([`LEVEL_START`], decision-r09-t14-choices, "T14.b
//!   follow-up" item 3): the level's own construction plus the upsample of the change the coarser
//!   level made to its construction, z = `z0_{k+1}` + I(`z_k` − `z0_k`), each bound's change zero
//!   (multigrid's full approximation scheme, Brandt 1977, from memory), so that the finer level
//!   keeps its own fine relief and the coarser level's network moves. I is R09.T4's interpolant
//!   ([`interpolate`]) at each child's centre jittered by a uniform offset in [−¼, ¼]² of the child
//!   cell (§5.2: "a uniform random offset in [−0.25, 0.25]² is added to the locations of the
//!   nodes before interpolation"), two uniforms on `surface.coarse.erosion` at instance
//!   [`JITTER_INSTANCE`] keyed by the child; then each bound at its ground. The interpolant
//!   approximates (a cell's centre takes 4/9 of its own value), so it smooths the change: recorded.
//!   [`Start`] keeps T14.b's upsample of the coarser level's elevation and the plain construction
//!   start beside it, measured.
//! - **The records** of each level ([`LevelResidual`]): each iteration's residual and largest
//!   change, and the receivers that changed between its last two routings. The fixed point does
//!   not converge exactly: its receivers flip on a floor that grows with the level, and the field
//!   takes the last solve on its own routing (decision-r09-t14-choices, "T14.b follow-up" items
//!   1–2). On a world dry now the receivers are drawn afresh at every iteration; on a world wet
//!   now [`Receivers::Sticky`] keeps them at every level from its third iteration while they stay
//!   lower ([`Options::WET_NOW`]; "T14.c follow-up 2" §3).
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
use super::depressions::{self, Measure, WaterBodies, route_with_water, route_with_water_from};
use super::drainage::{self, Routing, canonical, receiver_marks};
use super::mesh::Mesh;
use super::solver::{self, Cells, Threshold, Time};
use crate::planetary::hooks::SurfaceSeed;
use crate::planetary::surface::grid::CellGraph;
use crate::rng::{Mark, ObjectKey};
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

/// Where each finer level's iterations start (decision-r09-t14-choices, "T14.b follow-up" item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    /// R09.T14.b's as built: the upsample of the coarser level's last elevation, each bound at its
    /// base, then each of the level's bounds at its ground. Only the measurement of the starts, a
    /// test, runs it.
    #[cfg(test)]
    AsBuilt,
    /// The level's own construction, discarding the coarser levels. Only the measurement of the
    /// starts, a test, runs it.
    #[cfg(test)]
    Construction,
    /// The level's own construction plus the upsample of the coarser level's change from its
    /// construction, each bound's change zero (multigrid's full approximation scheme).
    Correction,
}

/// The multigrid's start of each finer level: [`Start::Correction`] (decision-r09-t14-choices,
/// "T14.b follow-up" item 3, which adopts it unless the construction start ends lower at the
/// finest level on both of R09.T14.b's synthetic worlds). The lane's measurement, R09.T14.c, keeps
/// it: at level 6, wet now, the smooth world's last residual is 82.18 m from the correction start
/// against 82.40 m from the construction and 92.45 m as built, the rough world's 30.08 m against
/// 14.70 m and 38.61 m; the reference Mars, dry now, ends at 23.32 m against 22.51 m and 32.69 m.
pub(crate) const LEVEL_START: Start = Start::Correction;

/// How each iteration draws its receivers (decision-r09-t14-choices, "T14.b follow-up" item 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Receivers {
    /// Afresh at every iteration from each cell's stored mark, its uniform kept as an integer
    /// (Tzathas et al. 2024, §4.3, "we precompute and store the random variable used by each
    /// cell"): the step's.
    Fresh,
    /// Sticky at every level from its third iteration on: a cell keeps its previous receiver
    /// while that stays strictly lower on the filled surface, and draws afresh only when it does
    /// not (`drainage::route_from`). Not Tzathas et al.'s: the "T14.b follow-up" item 2's lever,
    /// the wet-now branch's default since the field formulation makes a flip carry another
    /// basin's `λ_b` (decision-r09-t14-choices, "T14.c follow-up 2" §3).
    Sticky,
}

/// Each level's first iteration from which [`Receivers::Sticky`] keeps receivers: the third
/// (index 2; decision-r09-t14-choices, "T14.b follow-up" item 2, and "T14.c follow-up 2" §3).
const STICKY_FROM_ITERATION: usize = 2;

/// The additive refit's weight w: ½, the moving average's ([`MOVING_AVERAGE_WEIGHT`];
/// decision-r09-t14-choices, "T14.c follow-up 2" §1). The step is linear in the `λ` vector, Jacobi
/// on a diagonally dominant system, and needs no damping beyond it.
const REFIT_WEIGHT: f64 = MOVING_AVERAGE_WEIGHT;

/// How the multigrid runs: the start of each finer level and the receivers' draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Options {
    /// Where each finer level starts.
    pub(crate) start: Start,
    /// How each iteration draws its receivers.
    pub(crate) receivers: Receivers,
}

impl Options {
    /// The wet-now branch's: [`LEVEL_START`], with sticky receivers (decision-r09-t14-choices,
    /// "T14.c follow-up 2" §3).
    pub(crate) const WET_NOW: Self = Self {
        start: LEVEL_START,
        receivers: Receivers::Sticky,
    };

    /// The dry-now branch's: [`LEVEL_START`], with receivers drawn afresh, as built.
    pub(crate) const DRY_NOW: Self = Self {
        start: LEVEL_START,
        receivers: Receivers::Fresh,
    };
}

/// The rock uplift (decision-r09-t14-choices Q1), which the caller's branch chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Uplift {
    /// None: the construction is "the initial surface rather than a rate" (a world dry now).
    None,
    /// u = `λ_b` max(0, z0 − `ℓ_b`) on the cells not bound, a field fixed in space, b each cell's
    /// basin on the level's construction routing and `ℓ_b` its base level, `λ_b` refitted at every
    /// iteration so that each basin keeps its construction's mean height above `ℓ_b` (a world wet
    /// now).
    BasinFreeboard,
}

/// The water Fill–Spill–Merge spreads on each level's construction, with each cell's input in
/// cell-index order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Water<'a> {
    /// None: every depression a dry, terminal playa (a world wet now with no main liquid, which
    /// has no lake).
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
        /// Each cell's net loss were the water to flood it, m a⁻¹ ([`Measure::Balance`]'s).
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
    /// The steepest a link rises, at steady state only.
    pub(crate) threshold: Threshold,
    /// What a full water body, one that spills, is to the solve.
    pub(crate) full_bodies: FullBodies,
}

/// What a full water body, at its spill level and spilling through its outlet, is to the solve
/// (decision-r09-t14-choices, "T14.d lakes"), which the caller's branch chooses. A terminal body's
/// flooded cells and its pit are bounds either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FullBodies {
    /// Its flooded cells are bounds at its level, their ground untouched (a transient: a world dry
    /// now's paleo-lakes, Q2).
    Bounds,
    /// Its cells are no bounds: routed through on the priority flood's filled surface, the
    /// pop-order flats of R09.T14.a, and solved like any other cell, so that the steady state
    /// grades through the old depression, its floor raised to the profile and its sill cut to it
    /// (a world wet now).
    RoutedThrough,
}

/// One level's residuals: at each iteration, the root mean square by area of the iteration's
/// solution less the elevation it started from, over the cells not bound, metres; with its
/// largest change and the receivers that changed between its last two routings
/// (decision-r09-t14-choices, "T14.b follow-up" item 5).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LevelResidual {
    /// The level.
    pub(crate) level: u8,
    /// Each iteration's residual, metres, in iteration order.
    pub(crate) by_iteration_m: Vec<f64>,
    /// Each iteration's largest change, the greatest |s − z| over the cells not bound, metres, in
    /// iteration order.
    pub(crate) largest_change_by_iteration_m: Vec<f64>,
    /// Each iteration's basin misfit for [`Uplift::BasinFreeboard`], metres, in iteration order:
    /// the root mean square by area, over the fitted basins, of each basin's mean height above
    /// `ℓ_b` in the iteration's solution less its construction's, over the cells [`refit`] counts.
    /// Empty without uplift.
    pub(crate) basin_misfit_by_iteration_m: Vec<f64>,
    /// The cells whose receiver differs between the level's last two routings.
    pub(crate) receivers_changed: u32,
}

impl LevelResidual {
    /// The level's first residual, metres.
    ///
    /// # Panics
    ///
    /// Never: a level iterates [`ITERATIONS_PER_LEVEL`] times.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn first_m(&self) -> f64 {
        *self
            .by_iteration_m
            .first()
            .expect("a level iterates at least once")
    }

    /// The level's last largest change, metres.
    ///
    /// # Panics
    ///
    /// Never: a level iterates [`ITERATIONS_PER_LEVEL`] times.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn largest_change_m(&self) -> f64 {
        *self
            .largest_change_by_iteration_m
            .last()
            .expect("a level iterates at least once")
    }

    /// The level's last residual, metres.
    ///
    /// # Panics
    ///
    /// Never: a level iterates [`ITERATIONS_PER_LEVEL`] times.
    #[cfg(test)]
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
    /// fitted: the `λ_b` of the delivered solve.
    pub(crate) lambda: Option<LambdaSpread>,
    /// The fitted basins of the finest level whose last refit found their sensitivity `S_b` zero
    /// and kept their `λ_b` ([`refit`]).
    pub(crate) insensitive_basins: usize,
    /// The cells of the delivered solve whose own link rises at the threshold slope.
    pub(crate) held_at_threshold: usize,
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
    /// Each cell's bound on the field's level, its base, or `None` where it erodes: the sea's
    /// cells, the terminal bodies' flooded cells and pits, and the full bodies' flooded cells
    /// under [`FullBodies::Bounds`].
    pub(crate) bounds: Vec<Option<f64>>,
    /// Each cell's receiver mark on the field's level ([`receiver_marks`]), for a routing of the
    /// final elevation.
    pub(crate) marks: Vec<Mark>,
    /// Each cell's basin on the field's construction routing, its root, for
    /// [`Uplift::BasinFreeboard`]; empty without uplift.
    pub(crate) basins: Vec<u32>,
    /// Each cell's basin's base level `ℓ_b` on the field's construction routing, metres, for
    /// [`Uplift::BasinFreeboard`]; empty without uplift.
    pub(crate) construction_base_m: Vec<f64>,
    /// Each cell's uplift in the delivered solve, m a⁻¹: `λ_b` max(0, z0 − `ℓ_b`) for
    /// [`Uplift::BasinFreeboard`], zero on a bound, in a basin not fitted and without uplift.
    pub(crate) uplift_m_per_year: Vec<f64>,
    /// Each cell's change at the field's last iteration, its solution less the elevation that
    /// iteration started from, metres (zero on a bound): the final residual's terms.
    pub(crate) last_change_m: Vec<f64>,
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
                // Signed zeros made positive, as the drainage's are (`drainage::canonical`), so
                // that a parent at +0 m under a sea at −0 m is the sea's.
                2.0 * wet >= all && canonical(ground).total_cmp(&canonical(sea_level_m)).is_le()
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
    /// Each cell's receiver mark ([`receiver_marks`]).
    marks: Vec<Mark>,
    /// Each cell's uplift at λ = 1 a⁻¹: its freeboard above its basin's base level, metres, or 0.
    freeboard_m: Vec<f64>,
    /// Each cell's basin, its root on the construction's routing, for [`Uplift::BasinFreeboard`].
    basin: Vec<u32>,
    /// Each cell's basin's base level `ℓ_b` on the construction's routing, metres, for
    /// [`Uplift::BasinFreeboard`].
    base_level_m: Vec<f64>,
}

impl Setup {
    /// The setup of `level` for `problem` (see the module documentation).
    #[must_use]
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
        let fixed = bounds(
            &level.sea,
            problem.sea_level_m,
            &bodies,
            problem.full_bodies,
        );
        let marks = receiver_marks(problem.seed, &level.graph);
        let (freeboard_m, basin, base_level_m) = match problem.uplift {
            Uplift::None => (vec![0.0; n], Vec::new(), Vec::new()),
            Uplift::BasinFreeboard => {
                let routing = route_with_water(
                    &level.mesh,
                    &level.z0_m,
                    &level.sea,
                    problem.sea_level_m,
                    &bodies,
                    &marks,
                );
                basin_freeboard(&routing, &level.z0_m, &fixed)
            }
        };
        Self {
            bodies,
            fixed,
            marks,
            freeboard_m,
            basin,
            base_level_m,
        }
    }
}

/// Each cell's bound: the sea's cells at `sea_level_m`, each terminal body's flooded cells at its
/// level, each full body's at its level under [`FullBodies::Bounds`] (none under
/// [`FullBodies::RoutedThrough`]: they are solved), and each terminal body's pit at its level (a
/// root of every routing with these bodies).
#[must_use]
fn bounds(
    sea: &[bool],
    sea_level_m: f64,
    bodies: &WaterBodies,
    full_bodies: FullBodies,
) -> Vec<Option<f64>> {
    let mut fixed: Vec<Option<f64>> = sea.iter().map(|&wet| wet.then_some(sea_level_m)).collect();
    for body in bodies.bodies() {
        let held = match full_bodies {
            FullBodies::Bounds => true,
            FullBodies::RoutedThrough => !body.full,
        };
        if held {
            for &c in &body.cells {
                fixed[slot(c)] = Some(body.level_m);
            }
        }
        if let Some(level_m) = body.base_level_m() {
            fixed[slot(body.pit)] = Some(level_m);
        }
    }
    fixed
}

/// Each cell's freeboard max(0, z0 − `ℓ_b`) on the construction's routing `routing`, 0 on a bound,
/// its basin, the root it drains to, and the basin's base level `ℓ_b` (decision-r09-t14-choices
/// Q1's u = `λ_b` max(0, z0 − `ℓ_b`) at λ = 1 a⁻¹, b keyed by the root's cell): the uplift's
/// pattern is fixed in space for the level's iterations.
#[must_use]
fn basin_freeboard(
    routing: &Routing,
    z0_m: &[f64],
    fixed: &[Option<f64>],
) -> (Vec<f64>, Vec<u32>, Vec<f64>) {
    let n = slot(routing.len());
    let (mut freeboard_m, mut basin, mut base_level_m) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    for c in 0..routing.len() {
        let ci = slot(c);
        let base = routing.base_level_m(c);
        freeboard_m.push(if fixed[ci].is_some() {
            0.0
        } else {
            num::max(0.0, z0_m[ci] - base)
        });
        basin.push(routing.root_of(c));
        base_level_m.push(base);
    }
    (freeboard_m, basin, base_level_m)
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

/// Each basin's `λ_b`, a⁻¹, keyed by root, and whether it is fitted.
#[derive(Debug, Clone, PartialEq)]
struct Lambdas {
    /// Each basin's `λ_b`, a⁻¹, zero or above.
    lambda: Vec<f64>,
    /// Whether the basin is fitted: its `Σ_b` Ω F at λ = 1 in the level's closed form above zero.
    fitted: Vec<bool>,
}

/// Whether [`closed_form`] and [`refit`] count cell `c` in its basin's sums: not bound and of
/// equivalent area above zero (decision-r09-t14-choices Q1's bounds).
fn counted(setup: &Setup, equivalent_area_m2: &[f64], c: usize) -> bool {
    setup.fixed[c].is_none() && equivalent_area_m2[c] > 0.0
}

/// Q1's closed form of the basins' `λ_b` on `unit`, the solve at λ = 1 a⁻¹ (decision-r09-t14-
/// choices Q1): `λ_b` = `Σ_b` Ω (z0 − `ℓ_b`) ÷ `Σ_b` Ω F over the cells [`counted`] in each
/// construction basin, in cell-index order, each value asserted finite and `λ_b` held at zero or
/// above; a basin is fitted where its `Σ_b` Ω F is above zero, and one that is not keeps its
/// construction. Exact only where every path stays in its basin (the "T14.c follow-up" item 1's
/// erratum), it starts each level's [`refit`].
#[must_use]
fn closed_form(
    mesh: &Mesh,
    z0_m: &[f64],
    setup: &Setup,
    equivalent_area_m2: &[f64],
    unit: &solver::Solution,
) -> Lambdas {
    let n = z0_m.len();
    let (mut excess, mut response) = (vec![0.0; n], vec![0.0; n]);
    for (c, &z0) in z0_m.iter().enumerate() {
        if !counted(setup, equivalent_area_m2, c) {
            continue;
        }
        let b = slot(setup.basin[c]);
        let a = mesh.area_m2(cell_of(c));
        excess[b] += a * num::assert_finite(z0 - setup.base_level_m[c]);
        response[b] += a * num::assert_finite(unit.per_lambda_m()[c]);
    }
    let fitted: Vec<bool> = response.iter().map(|&f| f > 0.0).collect();
    let lambda = excess
        .iter()
        .zip(&response)
        .map(|(&e, &f)| if f > 0.0 { num::max(0.0, e / f) } else { 0.0 })
        .collect();
    Lambdas { lambda, fitted }
}

/// The uplift field of `lambdas` on `setup`, m a⁻¹: u(i) = `λ_b(i)` × freeboard(i) where i's basin
/// is fitted, else zero.
#[must_use]
fn uplift_field(setup: &Setup, lambdas: &Lambdas) -> Vec<f64> {
    setup
        .freeboard_m
        .iter()
        .zip(&setup.basin)
        .map(|(&freeboard, &b)| {
            let b = slot(b);
            if lambdas.fitted[b] {
                lambdas.lambda[b] * freeboard
            } else {
                0.0
            }
        })
        .collect()
}

/// The solve under `field` on `routing` with `cells` at `time`, held to `threshold`: each cell's
/// elevation at λ = 1, its construction where its basin is not fitted, and the solve itself.
fn solve_field(
    level: &Level<'_>,
    setup: &Setup,
    lambdas: &Lambdas,
    field: &[f64],
    (routing, cells): (&Routing, &Cells<'_>),
    (time, threshold): (Time, Threshold),
) -> (Vec<f64>, solver::Solution) {
    let solution = solver::solve_with(
        routing,
        &Cells {
            uplift_m_per_year: field,
            ..*cells
        },
        time,
        threshold,
    );
    let s = (0..field.len())
        .map(|c| {
            if lambdas.fitted[slot(setup.basin[c])] {
                solution.elevation_m(cell_of(c), 1.0)
            } else {
                level.z0_m[c]
            }
        })
        .collect();
    (s, solution)
}

/// A refit's result.
struct Refitted {
    /// The basins' `λ_b` for the next solve.
    lambdas: Lambdas,
    /// The basin misfit of the solve refitted on, metres ([`LevelResidual`]'s).
    misfit_m: f64,
    /// The fitted basins whose sensitivity `S_b` is zero, which kept their `λ_b`.
    insensitive: usize,
}

/// The additive refit of `lambdas` on the solve `solution`, of elevations `s`, over `routing`
/// (decision-r09-t14-choices, "T14.c follow-up 2" §1): each construction basin's mean elevation,
/// which no base enters, is held by `λ_b` ← max(0, `λ_b` + w `Σ_b` Ω (z0 − s) ÷ `S_b`), w the
/// [`REFIT_WEIGHT`].
///
/// - `S_b` = `Σ_b` Ω P(c) is the basin's own sensitivity along the part of each path that lies in
///   it: P(c) = `g_c` + [b(r(c)) = b(c)] P(r(c)) in the routing's preorder, r(c) the cell's
///   receiver, and `g_c` its own link's rise per unit λ, freeboard × ℓ ÷ a
///   ([`solver::Solution::link_years`]), zero where the threshold holds the link.
/// - Both sums run over the cells [`counted`] in each construction basin, in cell-index order, each
///   value asserted finite.
/// - A fitted basin whose `S_b` is zero keeps its `λ_b`. No ratio can turn non-positive, so a basin
///   whose cells drain to a lower base, a perched playa captured by the sea's basins, keeps its
///   mean too.
#[must_use]
fn refit(
    mesh: &Mesh,
    (z0_m, setup, equivalent_area_m2): (&[f64], &Setup, &[f64]),
    lambdas: &Lambdas,
    routing: &Routing,
    (s, solution): (&[f64], &solver::Solution),
) -> Refitted {
    let n = z0_m.len();
    let mut own = vec![0.0; n];
    for &cell in routing.preorder() {
        let at = slot(cell);
        if !counted(setup, equivalent_area_m2, at) {
            continue;
        }
        let rise = if solution.held_at_threshold()[at] {
            0.0
        } else {
            setup.freeboard_m[at] * solution.link_years()[at]
        };
        let upstream = routing.receiver(cell).map_or(0.0, |link| {
            let receiver = slot(link.cell);
            if setup.basin[receiver] == setup.basin[at] {
                own[receiver]
            } else {
                0.0
            }
        });
        own[at] = num::assert_finite(rise + upstream);
    }
    let (mut gap, mut sensitivity, mut area) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    for (c, &z0) in z0_m.iter().enumerate() {
        if !counted(setup, equivalent_area_m2, c) {
            continue;
        }
        let b = slot(setup.basin[c]);
        let a = mesh.area_m2(cell_of(c));
        gap[b] += a * num::assert_finite(z0 - s[c]);
        sensitivity[b] += a * own[c];
        area[b] += a;
    }
    let (mut squares, mut weight, mut insensitive) = (0.0, 0.0, 0);
    let lambda = (0..n)
        .map(|b| {
            let held = lambdas.lambda[b];
            if !lambdas.fitted[b] {
                return held;
            }
            if area[b] > 0.0 {
                let misfit = gap[b] / area[b];
                squares += area[b] * misfit * misfit;
                weight += area[b];
            }
            if sensitivity[b] > 0.0 {
                num::max(0.0, held + REFIT_WEIGHT * gap[b] / sensitivity[b])
            } else {
                insensitive += 1;
                held
            }
        })
        .collect();
    Refitted {
        lambdas: Lambdas {
            lambda,
            fitted: lambdas.fitted.clone(),
        },
        misfit_m: if weight > 0.0 {
            (squares / weight).sqrt()
        } else {
            0.0
        },
        insensitive,
    }
}

/// The records of the basin fit `lambdas` under `field` on `level`: the implied mean uplift over
/// its land, Σ Ω u ÷ Σ Ω over the cells off the sea, m a⁻¹, and the fitted basins' `λ_b`
/// distribution.
fn uplift_records(
    level: &Level<'_>,
    lambdas: &Lambdas,
    field: &[f64],
) -> (f64, Option<LambdaSpread>) {
    let mesh = &level.mesh;
    let (mut uplift, mut land) = (0.0, 0.0);
    for (c, &u) in field.iter().enumerate() {
        if level.sea[c] {
            continue;
        }
        let a = mesh.area_m2(cell_of(c));
        land += a;
        uplift += a * u;
    }
    let roots_fitted: Vec<f64> = lambdas
        .lambda
        .iter()
        .zip(&lambdas.fitted)
        .filter(|&(_, &fitted)| fitted)
        .map(|(&l, _)| l)
        .collect();
    let mean = if land > 0.0 { uplift / land } else { 0.0 };
    (mean, spread(roots_fitted))
}

/// The distribution of `values`, each finite, or `None` if it is empty: the p-th percentile the
/// value of rank round(p × (count − 1) ÷ 100) in ascending order (by `total_cmp`).
#[must_use]
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
#[must_use]
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
    /// Its construction, metres.
    z0_m: &'a [f64],
    /// Its last elevation, metres.
    z_m: &'a [f64],
    /// Its bounds and their bases.
    fixed: &'a [Option<f64>],
}

/// The start of `level`'s iterations: its construction at the coarsest level; else, by `how`, the
/// last elevation of the level below it, each of its bounds at its base, upsampled
/// ([`upsample`]), the level's construction, or the construction plus the upsample of the level
/// below's change from its own, each of its bounds' change zero; and in every case each of the
/// level's own bounds `bounds` at its ground.
#[must_use]
fn start(
    seed: SurfaceSeed,
    level: &Level<'_>,
    bounds: &[Option<f64>],
    below: Option<Finished<'_>>,
    how: Start,
) -> Vec<f64> {
    let mut z = match below {
        None => level.z0_m.clone(),
        Some(coarse) => match how {
            #[cfg(test)]
            Start::Construction => level.z0_m.clone(),
            #[cfg(test)]
            Start::AsBuilt => {
                let surface: Vec<f64> = coarse
                    .z_m
                    .iter()
                    .zip(coarse.fixed)
                    .map(|(&h, bound)| bound.unwrap_or(h))
                    .collect();
                upsample(seed, coarse.graph, &surface, &level.graph)
            }
            Start::Correction => {
                let change: Vec<f64> = coarse
                    .z_m
                    .iter()
                    .zip(coarse.z0_m)
                    .zip(coarse.fixed)
                    .map(|((&h, &ground), bound)| if bound.is_some() { 0.0 } else { h - ground })
                    .collect();
                let change = upsample(seed, coarse.graph, &change, &level.graph);
                level
                    .z0_m
                    .iter()
                    .zip(change)
                    .map(|(&ground, d)| ground + d)
                    .collect()
            }
        },
    };
    for (h, (bound, &ground)) in z.iter_mut().zip(bounds.iter().zip(&level.z0_m)) {
        if bound.is_some() {
            *h = ground;
        }
    }
    z
}

/// The multigrid's fixed point for `problem` (see the module documentation), run as `options` say
/// ([`Options::WET_NOW`] and [`Options::DRY_NOW`] for the step's branches).
///
/// # Panics
///
/// If an input is not one a cell of the problem's grid, a value is not finite, a runoff is
/// negative, an erodibility is not positive, a sea cell's ground lies above the sea's level, the
/// parent grid is not one level up, or a threshold slope is not finite and positive or is given
/// off the steady state.
#[must_use]
pub(crate) fn erode(problem: &Problem<'_>, options: Options) -> Outcome {
    let levels = levels(problem);
    let mut residuals = Vec::with_capacity(levels.len());
    let mut z: Vec<f64> = Vec::new();
    let mut below: Option<(usize, Vec<Option<f64>>)> = None;
    let mut finished = None;
    for (at, level) in levels.iter().enumerate() {
        let setup = Setup::of(level, problem);
        let coarse = below.as_ref().map(|(k, fixed)| Finished {
            graph: &levels[*k].graph,
            z0_m: &levels[*k].z0_m,
            z_m: &z,
            fixed,
        });
        z = start(problem.seed, level, &setup.fixed, coarse, options.start);
        let finest = at + 1 == levels.len();
        let (residual, last) = iterate_level(level, &setup, &mut z, problem, options, finest);
        residuals.push(residual);
        match last {
            Some(step) => finished = Some((setup, step)),
            None => below = Some((at, setup.fixed)),
        }
    }
    let (setup, (step, last_change_m)) = finished.expect("the multigrid's finest level iterates");
    let z = step.solution_m;
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
        bounds: setup.fixed,
        marks: setup.marks,
        basins: setup.basin,
        construction_base_m: setup.base_level_m,
        uplift_m_per_year: step.uplift_m_per_year,
        last_change_m,
        diagnostics: Diagnostics {
            residuals,
            lambda: step.lambda,
            insensitive_basins: step.insensitive,
            held_at_threshold: step.held,
            mean_uplift_m_per_year: step.mean_uplift_m_per_year,
        },
    }
}

/// `level`'s [`ITERATIONS_PER_LEVEL`] iterations of the fixed point for `problem` from `z`, run as
/// `options` say, `finest` where the level is the field's: their records, and on the finest level
/// its last iteration with each cell's change at it, `z` taken; on any other, `z` left as the
/// level's last moving average.
fn iterate_level(
    level: &Level<'_>,
    setup: &Setup,
    z: &mut Vec<f64>,
    problem: &Problem<'_>,
    options: Options,
    finest: bool,
) -> (LevelResidual, Option<(Iteration, Vec<f64>)>) {
    let mut by_iteration_m = Vec::with_capacity(ITERATIONS_PER_LEVEL);
    let mut largest_change_by_iteration_m = Vec::with_capacity(ITERATIONS_PER_LEVEL);
    let mut basin_misfit_by_iteration_m = Vec::new();
    let mut receivers: Vec<Option<u32>> = Vec::new();
    let mut receivers_changed = 0;
    let mut last = None;
    let mut previous: Option<Routing> = None;
    let mut lambdas = None;
    for iteration in 0..ITERATIONS_PER_LEVEL {
        let sticky = options.receivers == Receivers::Sticky && iteration >= STICKY_FROM_ITERATION;
        let turn = if finest && iteration + 1 == ITERATIONS_PER_LEVEL {
            Turn::Final
        } else {
            Turn::Inner
        };
        let step = iterate(
            level,
            setup,
            z,
            problem,
            previous.as_ref().filter(|_| sticky),
            (turn, &mut lambdas),
        );
        by_iteration_m.push(step.residual_m);
        largest_change_by_iteration_m.push(step.largest_change_m);
        basin_misfit_by_iteration_m.extend(step.misfit_m);
        let now: Vec<Option<u32>> = (0..step.routing.len())
            .map(|c| step.routing.receiver(c).map(|link| link.cell))
            .collect();
        if iteration + 1 == ITERATIONS_PER_LEVEL {
            receivers_changed = cell_of(
                receivers
                    .iter()
                    .zip(&now)
                    .filter(|(before, after)| before != after)
                    .count(),
            );
        }
        receivers = now;
        match turn {
            Turn::Final => {
                for (h, &s) in z.iter_mut().zip(&step.solution_m) {
                    *h = s - *h;
                }
                last = Some((step, core::mem::take(z)));
            }
            Turn::Inner => {
                for (h, &s) in z.iter_mut().zip(&step.solution_m) {
                    *h += MOVING_AVERAGE_WEIGHT * (s - *h);
                }
                previous = Some(step.routing);
            }
        }
    }
    let residual = LevelResidual {
        level: level.graph.level(),
        by_iteration_m,
        largest_change_by_iteration_m,
        basin_misfit_by_iteration_m,
        receivers_changed,
    };
    (residual, last)
}

/// Which of the multigrid's iterations runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Turn {
    /// Any but the finest level's last.
    Inner,
    /// The finest level's last, whose solve the field takes.
    Final,
}

/// One iteration's results.
struct Iteration {
    routing: Routing,
    equivalent_area_m2: Vec<f64>,
    solution_m: Vec<f64>,
    uplift_m_per_year: Vec<f64>,
    residual_m: f64,
    largest_change_m: f64,
    lambda: Option<LambdaSpread>,
    mean_uplift_m_per_year: f64,
    /// The basin misfit of the solve, for [`Uplift::BasinFreeboard`].
    misfit_m: Option<f64>,
    /// The fitted basins whose refit on the solve found their sensitivity zero.
    insensitive: usize,
    /// The cells of the solve held at the threshold.
    held: usize,
}

/// One iteration of the fixed point on `level` from the elevation `z`: route (keeping the
/// receivers of `previous` that stay lower, where it is given), accumulate, solve, and measure the
/// residual. For [`Uplift::BasinFreeboard`] the solve is the uplift field's, its basins' `λ_b`
/// carried in `lambdas` from the level's previous iteration (Q1's closed form on its first) and
/// refitted on the solve for the next; on the [`Turn::Final`] iteration the solve is taken again
/// on the same routing with the refitted `λ_b`.
fn iterate(
    level: &Level<'_>,
    setup: &Setup,
    z: &[f64],
    problem: &Problem<'_>,
    previous: Option<&Routing>,
    (turn, lambdas): (Turn, &mut Option<Lambdas>),
) -> Iteration {
    let mesh = &level.mesh;
    let routing = route_with_water_from(
        mesh,
        z,
        &level.sea,
        problem.sea_level_m,
        &setup.bodies,
        &setup.marks,
        previous,
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
    let n = z.len();
    let fit = match problem.uplift {
        Uplift::None => {
            let solution = solver::solve_with(&routing, &cells, problem.time, problem.threshold);
            Fit {
                solution_m: (0..n)
                    .map(|c| solution.elevation_m(cell_of(c), 0.0))
                    .collect(),
                uplift_m_per_year: vec![0.0; n],
                lambda: None,
                mean_uplift_m_per_year: 0.0,
                misfit_m: None,
                insensitive: 0,
                held: count_held(&solution),
            }
        }
        Uplift::BasinFreeboard => {
            basin_fit(level, setup, (&routing, &cells), problem, (turn, lambdas))
        }
    };
    let (mut squares, mut weight, mut largest_change_m) = (0.0, 0.0, 0.0);
    for (c, (&s, &h)) in fit.solution_m.iter().zip(z).enumerate() {
        if setup.fixed[c].is_some() {
            continue;
        }
        let a = mesh.area_m2(cell_of(c));
        let d = num::assert_finite(s - h);
        squares += a * d * d;
        weight += a;
        largest_change_m = num::max(largest_change_m, d.abs());
    }
    let residual_m = if weight > 0.0 {
        (squares / weight).sqrt()
    } else {
        0.0
    };
    Iteration {
        routing,
        equivalent_area_m2,
        solution_m: fit.solution_m,
        uplift_m_per_year: fit.uplift_m_per_year,
        residual_m,
        largest_change_m,
        lambda: fit.lambda,
        mean_uplift_m_per_year: fit.mean_uplift_m_per_year,
        misfit_m: fit.misfit_m,
        insensitive: fit.insensitive,
        held: fit.held,
    }
}

/// One iteration's solve and its records.
struct Fit {
    solution_m: Vec<f64>,
    uplift_m_per_year: Vec<f64>,
    lambda: Option<LambdaSpread>,
    mean_uplift_m_per_year: f64,
    misfit_m: Option<f64>,
    insensitive: usize,
    held: usize,
}

/// The cells of `solution` held at the threshold.
#[must_use]
fn count_held(solution: &solver::Solution) -> usize {
    solution
        .held_at_threshold()
        .iter()
        .filter(|&&held| held)
        .count()
}

/// [`Uplift::BasinFreeboard`]'s solve on `routing` with `cells`, their uplift the freeboard at unit
/// λ, for `problem`: the basins' `λ_b` carried in `lambdas`, or Q1's closed form on a solve at unit
/// λ with no threshold where the level starts ([`closed_form`]); the solve under their uplift field
/// held to the problem's threshold ([`solve_field`]); the refit on it for the next iteration
/// ([`refit`]); and, on the [`Turn::Final`] iteration, the solve again with the refitted `λ_b` (see
/// the module documentation) and its own refit, for the records.
fn basin_fit(
    level: &Level<'_>,
    setup: &Setup,
    (routing, cells): (&Routing, &Cells<'_>),
    problem: &Problem<'_>,
    (turn, lambdas): (Turn, &mut Option<Lambdas>),
) -> Fit {
    let mesh = &level.mesh;
    let sums = (level.z0_m.as_slice(), setup, cells.equivalent_area_m2);
    let solve = |used: &Lambdas| {
        let field = uplift_field(setup, used);
        let (s, solution) = solve_field(
            level,
            setup,
            used,
            &field,
            (routing, cells),
            (problem.time, problem.threshold),
        );
        let refitted = refit(mesh, sums, used, routing, (&s, &solution));
        (field, s, count_held(&solution), refitted)
    };
    let mut used = lambdas.take().unwrap_or_else(|| {
        let unit = solver::solve(routing, cells, problem.time);
        closed_form(mesh, &level.z0_m, setup, cells.equivalent_area_m2, &unit)
    });
    let (mut field, mut s, mut held, mut refitted) = solve(&used);
    if turn == Turn::Final {
        used = refitted.lambdas;
        (field, s, held, refitted) = solve(&used);
    }
    let (mean_uplift_m_per_year, lambda) = uplift_records(level, &used, &field);
    let fit = Fit {
        solution_m: s,
        uplift_m_per_year: field,
        lambda,
        mean_uplift_m_per_year,
        misfit_m: Some(refitted.misfit_m),
        insensitive: refitted.insensitive,
        held,
    };
    *lambdas = Some(refitted.lambdas);
    fit
}
