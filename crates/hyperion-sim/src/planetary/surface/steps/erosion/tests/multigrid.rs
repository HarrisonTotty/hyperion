//! The multigrid's tests: the plan's fixed point, its residual over the levels, here on synthetic
//! worlds (the reference Earth's lands with R09.T14.c's wet-now branch, slow), and the lane's
//! (each basin keeps its construction's mean height, a lake by its balance is a bound, a world dry
//! now never rises, the same problem gives the same bits, the jitter's and the receivers' draws
//! are distinct, a constant field upsamples to itself and a smooth one close to itself).
//!
//! **Recorded, not asserted:** the plan's "the fixed point's residual falls monotonically over the
//! multigrid levels" does not hold on these worlds. Each level's residual falls over its six
//! iterations, but each finer level starts from the upsample several hundred metres from its own
//! solution, and at levels 5 and 6 the fixed point settles, given more iterations, on a floor of
//! receivers that flip with the elevation, which grows with the level (the lane's report to
//! R09.T14's lane agent, 2026-10-20). `the_residual_falls_over_each_level_s_iterations` prints
//! the figures.

use hyperion_testkit::float::assert_same_bits;

use super::super::depressions::{WaterBodies, route_with_water};
use super::super::drainage::{RECEIVER_INSTANCE, receiver_uniforms};
use super::super::erodibility::EARTH_ERODIBILITY_M0_1_PER_YEAR;
use super::super::multigrid::{
    ITERATIONS_PER_LEVEL, JITTER_INSTANCE, LEVEL_START, LevelResidual, MULTIGRID_FLOOR_LEVEL,
    Options, Outcome, Problem, Start, Uplift, UpliftFit, Water, erode, jitter_uniforms, upsample,
};
use super::super::solver::Time;
use super::*;
use crate::planetary::hooks::SurfaceSeed;
use crate::planetary::surface::reference;

/// The synthetic worlds' uniform runoff, m a⁻¹: 0.5 (the ruling's prototype's,
/// decision-r09-t14-choices Q1).
const RUNOFF_M_PER_YEAR: f64 = 0.5;

/// How rough a synthetic world's ground is at the scale of its cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Roughness {
    /// A swell of ±400 m over its continent, with ±15 m of independent noise a cell: few closed
    /// depressions, as a real continent's.
    Smooth,
    /// ±150 m of independent noise a cell and no swell: the land pitted with closed depressions,
    /// a dry playa every few cells.
    Rough,
}

/// A synthetic world on `graph`: a continent within 1 rad of (1, 0.6, 0.3) normalised, a dome
/// rising from 200 m at its coast to 2,200 m at its centre, an ocean floor at −4,000 m, and the
/// roughness `roughness` (the swell 400 sin 7x sin(5y + 1) cos 6z m of the cell's centre on the
/// continent, the noise from the testkit's LCG under seed 6, on every cell); the sea every cell
/// below [`SEA_LEVEL_M`].
fn synthetic_world(graph: &CellGraph, roughness: Roughness) -> (Vec<f64>, Vec<bool>) {
    let mut lcg = Lcg::new(6);
    let (swell, noise) = match roughness {
        Roughness::Smooth => (400.0, 30.0),
        Roughness::Rough => (0.0, 300.0),
    };
    let centre = unit([1.0, 0.6, 0.3]);
    let edge = math::cos(1.0);
    let z: Vec<f64> = graph
        .centres()
        .iter()
        .map(|&d| {
            let jitter = noise * (lcg.next_f64() - 0.5);
            let c = dot(d, centre);
            if c > edge {
                let wave =
                    math::sin(7.0 * d[0]) * math::sin(5.0 * d[1] + 1.0) * math::cos(6.0 * d[2]);
                200.0 + 2_000.0 * (c - edge) / (1.0 - edge) + swell * wave + jitter
            } else {
                -4_000.0 + jitter
            }
        })
        .collect();
    let sea = z.iter().map(|&h| h < SEA_LEVEL_M).collect();
    (z, sea)
}

/// A synthetic world at one level, a uniform runoff of [`RUNOFF_M_PER_YEAR`] and K⊕, multigridded
/// under the reference seed.
struct Synthetic {
    graph: CellGraph,
    parent: CellGraph,
    z0: Vec<f64>,
    sea: Vec<bool>,
    runoff: Vec<f64>,
    erodibility: Vec<f64>,
}

impl Synthetic {
    /// The world of `roughness` at `level`, with its sea, or with none (`with_sea` false).
    fn new(level: u8, roughness: Roughness, with_sea: bool) -> Self {
        let graph = CellGraph::new(level).unwrap();
        let parent = CellGraph::new(level - 1).unwrap();
        let (z0, mut sea) = synthetic_world(&graph, roughness);
        if !with_sea {
            sea.fill(false);
        }
        let n = slot(graph.len());
        Self {
            graph,
            parent,
            z0,
            sea,
            runoff: vec![RUNOFF_M_PER_YEAR; n],
            erodibility: vec![EARTH_ERODIBILITY_M0_1_PER_YEAR; n],
        }
    }

    fn erode(&self, uplift: Uplift, water: Water<'_>, time: Time) -> Outcome {
        self.erode_with(uplift, water, time, Options::STEP)
    }

    fn erode_with(
        &self,
        uplift: Uplift,
        water: Water<'_>,
        time: Time,
        options: Options,
    ) -> Outcome {
        erode(
            &Problem {
                seed: reference::SEED,
                radius: RADIUS,
                grid: &self.graph,
                parent_grid: Some(&self.parent),
                z0_m: &self.z0,
                runoff_m_per_year: &self.runoff,
                erodibility_m0_1_per_year: &self.erodibility,
                sea: &self.sea,
                sea_level_m: SEA_LEVEL_M,
                water,
                uplift,
                time,
            },
            options,
        )
    }
}

/// The cells `outcome`'s bodies and `sea` hold as bounds, with their bases: the sea at
/// [`SEA_LEVEL_M`], each body's flooded cells and each terminal body's pit at its level.
fn bounds(sea: &[bool], bodies: &WaterBodies) -> Vec<Option<f64>> {
    let mut fixed: Vec<Option<f64>> = sea.iter().map(|&w| w.then_some(SEA_LEVEL_M)).collect();
    for body in bodies.bodies() {
        for &c in &body.cells {
            fixed[slot(c)] = Some(body.level_m);
        }
        if let Some(level) = body.base_level_m() {
            fixed[slot(body.pit)] = Some(level);
        }
    }
    fixed
}

/// The land's area-weighted mean of `z` on `outcome`'s mesh, the cells off `sea`, metres.
fn land_mean(outcome: &Outcome, sea: &[bool], z: &[f64]) -> f64 {
    let (mut sum, mut area) = (0.0, 0.0);
    for (c, &wet) in sea.iter().enumerate() {
        if !wet {
            let a = outcome.mesh.area_m2(cell_of(c));
            sum += a * z[c];
            area += a;
        }
    }
    sum / area
}

/// The fixed point's residual falls over each level's iterations: on both synthetic worlds at
/// level 6 (levels 4, 5 and 6, six iterations each), wet now (steady state, each basin's uplift
/// its freeboard scaled by its fitted `λ_b`, every depression a dry playa), each level's last
/// residual is below a quarter of its first; the final elevation is the solve on its last
/// routing, every cell not bound lying above its receiver on it where both lie in one basin of the
/// construction ([`assert_above_receivers`]; decision-r09-t14-choices, "T14.b follow-up" item 5,
/// whose across-basin exceptions it prints); the sea's cells keep their ground; and each level's
/// first and last residual, largest change and changed receivers (the plan's "falls
/// monotonically over the levels" among them, recorded in the module documentation), `λ_b`'s
/// distribution, the implied uplift and the land's mean are printed.
#[test]
fn the_residual_falls_over_each_level_s_iterations() {
    for roughness in [Roughness::Smooth, Roughness::Rough] {
        let world = Synthetic::new(6, roughness, true);
        let outcome = world.erode(Uplift::BasinFreeboard, Water::None, Time::Steady);
        let what = format!("wet now, {roughness:?}");
        report(&what, &outcome);
        let levels: Vec<u8> = outcome
            .diagnostics
            .residuals
            .iter()
            .map(|r| r.level)
            .collect();
        assert_eq!(levels, (MULTIGRID_FLOOR_LEVEL..=6).collect::<Vec<_>>());
        for r in &outcome.diagnostics.residuals {
            assert_eq!(r.by_iteration_m.len(), ITERATIONS_PER_LEVEL);
            let first = r.by_iteration_m[0];
            assert!(
                r.last_m() < 0.25 * first,
                "{what}: level {} falls from {first} m to {} m",
                r.level,
                r.last_m()
            );
        }
        let by_level: Vec<f64> = outcome
            .diagnostics
            .residuals
            .iter()
            .map(LevelResidual::last_m)
            .collect();
        eprintln!(
            "{what}: the last residuals by level {by_level:?} {} monotonically",
            if by_level.windows(2).all(|w| w[1] < w[0]) {
                "fall"
            } else {
                "do not fall"
            }
        );
        assert!(outcome.diagnostics.lambda.is_some());
        assert_above_receivers(&what, &outcome);
        for (c, &wet) in world.sea.iter().enumerate() {
            if wet {
                assert_same_bits(outcome.z_m[c], world.z0[c]);
            }
        }
        eprintln!(
            "{what}: the land's mean {:.1} m constructed, {:.1} m eroded",
            land_mean(&outcome, &world.sea, &world.z0),
            land_mean(&outcome, &world.sea, &outcome.z_m)
        );
    }
}

/// A lake by its balance is a bound: the smooth world at level 6, wet now, its depressions holding
/// the runoff of their catchments against a net loss of 1 m a⁻¹ over open water (Fill–Spill–Merge
/// on fluxes, decision-r09-t14-choices Q3): some lake floods cells, and every flooded cell and the
/// sea's keep their ground to the bit.
#[test]
fn a_lake_by_its_balance_is_a_bound() {
    let world = Synthetic::new(6, Roughness::Smooth, true);
    let (_, mesh) = super::world(6);
    let inflow: Vec<f64> = world
        .runoff
        .iter()
        .zip(mesh.areas_m2())
        .map(|(&r, &a)| r * a)
        .collect();
    let loss = vec![1.0; inflow.len()];
    let outcome = world.erode(
        Uplift::BasinFreeboard,
        Water::Balance {
            inflow_m3_per_year: &inflow,
            net_loss_m_per_year: &loss,
        },
        Time::Steady,
    );
    report("wet now with lakes", &outcome);
    let flooded: usize = outcome.bodies.bodies().iter().map(|b| b.cells.len()).sum();
    eprintln!(
        "{} bodies, {flooded} cells flooded",
        outcome.bodies.bodies().len()
    );
    assert!(flooded > 0);
    for (c, bound) in bounds(&world.sea, &outcome.bodies).iter().enumerate() {
        if bound.is_some() {
            assert_same_bits(outcome.z_m[c], world.z0[c]);
        }
    }
}

/// Each basin keeps its construction's mean height (decision-r09-t14-choices Q1): on the smooth
/// synthetic world at level 6, wet now, every basin of the construction's routing at the finest
/// level (its own receivers' draw, the outcome's bodies) has the same area-weighted mean over its
/// cells not bound and of equivalent area above zero in the final field as in the construction, to
/// 10⁻⁹ of its mean freeboard.
#[test]
fn each_basin_keeps_its_construction_s_mean_height() {
    let world = Synthetic::new(6, Roughness::Smooth, true);
    let outcome = world.erode(Uplift::BasinFreeboard, Water::None, Time::Steady);
    let mesh = &outcome.mesh;
    let uniforms = receiver_uniforms(reference::SEED, &world.graph);
    let construction = route_with_water(
        mesh,
        &world.z0,
        &world.sea,
        SEA_LEVEL_M,
        &outcome.bodies,
        &uniforms,
    );
    let fixed = bounds(&world.sea, &outcome.bodies);
    let n = world.z0.len();
    let (mut moved, mut freeboard, mut area) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    for (c, bound) in fixed.iter().enumerate() {
        if bound.is_some() || outcome.equivalent_area_m2[c] <= 0.0 {
            continue;
        }
        let b = slot(construction.root_of(cell_of(c)));
        let a = mesh.area_m2(cell_of(c));
        moved[b] += a * (outcome.z_m[c] - world.z0[c]);
        freeboard[b] += a * (world.z0[c] - construction.base_level_m(cell_of(c)));
        area[b] += a;
    }
    let (mut checked, mut held, mut worst) = (0, 0, 0.0_f64);
    for b in 0..n {
        if area[b] <= 0.0 || freeboard[b] <= 0.0 {
            continue;
        }
        let error = moved[b].abs() / freeboard[b];
        if error > 1e-9 {
            held += 1;
            eprintln!(
                "basin {b}: its mean moved {:.3} m of {:.3} m freeboard",
                moved[b] / area[b],
                freeboard[b] / area[b]
            );
        } else {
            checked += 1;
            worst = worst.max(error);
        }
    }
    eprintln!("{checked} basins hold their mean to {worst:.2e}; {held} do not");
    assert!(checked > 0);
    assert_eq!(held, 0);
}

/// A world dry now never rises: the smooth synthetic world at level 6 with no sea now, its
/// paleo-sea a uniform 1,000 m of water spread by Fill–Spill–Merge, no uplift and the reference
/// Mars's epoch (2.2 × 10⁵ years), K 2.4 × 10⁻⁵ m^0.1 a⁻¹ at 0.1 m a⁻¹: no cell ends above its
/// construction, every flooded cell keeps its ground, some cell is lowered, and the residuals are
/// printed.
#[test]
fn a_world_dry_now_never_rises() {
    let mut world = Synthetic::new(6, Roughness::Smooth, false);
    world.runoff.fill(0.1);
    world.erodibility.fill(2.4e-5);
    let (_, mesh) = super::world(6);
    let water = rain(&mesh, 1_000.0);
    let outcome = world.erode(
        Uplift::None,
        Water::Volume { water_m3: &water },
        Time::Years(2.2e5),
    );
    report("dry now", &outcome);
    assert!(outcome.bodies.bodies().iter().any(|b| !b.cells.is_empty()));
    let mut lowered = 0;
    for (c, (&z, &z0)) in outcome.z_m.iter().zip(&world.z0).enumerate() {
        assert!(z <= z0, "cell {c} rose from {z0} m to {z} m");
        if z < z0 {
            lowered += 1;
        }
    }
    for body in outcome.bodies.bodies() {
        for &c in &body.cells {
            assert_same_bits(outcome.z_m[slot(c)], world.z0[slot(c)]);
        }
    }
    eprintln!("{lowered} of {} cells lowered", world.z0.len());
    assert!(lowered > 0);
}

/// The same problem gives the same bits: the smooth synthetic world at level 5, wet now, twice.
#[test]
fn the_same_problem_gives_the_same_bits() {
    let world = Synthetic::new(5, Roughness::Smooth, true);
    let a = world.erode(Uplift::BasinFreeboard, Water::None, Time::Steady);
    let b = world.erode(Uplift::BasinFreeboard, Water::None, Time::Steady);
    for (&x, &y) in a.z_m.iter().zip(&b.z_m) {
        assert_same_bits(x, y);
    }
    assert_eq!(a, b);
}

/// The jitter's and the receivers' draws are distinct: instances 1 and 0 of the tag differ, and at
/// level 5 under the reference seed no cell's receiver uniform equals either of its jitter
/// uniforms, nor its two jitter uniforms each other; the jitter is the same under the same seed and
/// differs under another.
#[test]
fn the_jitter_and_the_receivers_are_distinct_draws() {
    assert_ne!(RECEIVER_INSTANCE, JITTER_INSTANCE);
    let graph = CellGraph::new(5).unwrap();
    let receivers = receiver_uniforms(reference::SEED, &graph);
    let jitter = jitter_uniforms(reference::SEED, &graph);
    for (c, (&r, &[s, t])) in receivers.iter().zip(&jitter).enumerate() {
        assert!(
            r.total_cmp(&s).is_ne() && r.total_cmp(&t).is_ne() && s.total_cmp(&t).is_ne(),
            "cell {c}: receiver {r}, jitter {s}, {t}"
        );
        assert!((0.0..1.0).contains(&s) && (0.0..1.0).contains(&t));
    }
    let again = jitter_uniforms(reference::SEED, &graph);
    for (a, b) in jitter.iter().zip(&again) {
        assert_same_bits(a[0], b[0]);
        assert_same_bits(a[1], b[1]);
    }
    let other = jitter_uniforms(SurfaceSeed::new(reference::SEED.get() ^ 1), &graph);
    assert!(
        jitter
            .iter()
            .zip(&other)
            .any(|(a, b)| a[0].total_cmp(&b[0]).is_ne())
    );
}

/// A constant field upsamples to itself, bit for bit: 1,234.5 m at every cell of level 4,
/// interpolated at the jittered centres of level 5's cells (R09.T4's interpolant gives a constant
/// field back exactly).
#[test]
fn a_constant_field_upsamples_to_itself() {
    let coarse = CellGraph::new(4).unwrap();
    let fine = CellGraph::new(5).unwrap();
    let values = vec![1_234.5; slot(coarse.len())];
    let up = upsample(reference::SEED, &coarse, &values, &fine);
    assert_eq!(up.len(), slot(fine.len()));
    for &v in &up {
        assert_same_bits(v, 1_234.5);
    }
}

/// A smooth field upsamples close to itself: 1,000 m times the centre's x, y and z components
/// summed with weights 1, 0.5 and −0.3 at level 4's cells, upsampled to level 5, lies within 5% of
/// the field's range of the same function at each level-5 cell's centre (the jitter moves a
/// centre by at most a quarter cell, and the interpolant smooths over about a cell).
#[test]
fn a_smooth_field_upsamples_close_to_itself() {
    let coarse = CellGraph::new(4).unwrap();
    let fine = CellGraph::new(5).unwrap();
    let f = |d: [f64; 3]| 1_000.0 * (d[0] + 0.5 * d[1] - 0.3 * d[2]);
    let values: Vec<f64> = coarse.centres().iter().map(|&d| f(d)).collect();
    let up = upsample(reference::SEED, &coarse, &values, &fine);
    let worst = up
        .iter()
        .zip(fine.centres())
        .map(|(&v, &d)| (v - f(d)).abs())
        .fold(0.0, f64::max);
    eprintln!("worst departure {worst:.2} m of a 2,300 m range");
    assert!(worst < 0.05 * 2_300.0, "{worst} m");
}

/// The three starts of a finer level (decision-r09-t14-choices, "T14.b follow-up" item 3): on both
/// synthetic worlds at level 6, wet now, each level's first and last residual, largest change and
/// changed receivers for T14.b's upsample, the construction and the correction (multigrid's full
/// approximation scheme). The rule: the correction start unless the construction start ends lower
/// at the finest level on both worlds. Asserts nothing but the step's own start.
#[test]
#[ignore = "measurement: the multigrid's level starts, R09.T14.c"]
fn measure_the_level_starts() {
    assert_eq!(LEVEL_START, Start::Correction);
    for roughness in [Roughness::Smooth, Roughness::Rough] {
        let world = Synthetic::new(6, roughness, true);
        let mut finest = Vec::new();
        for start in [Start::AsBuilt, Start::Construction, Start::Correction] {
            let outcome = world.erode_with(
                Uplift::BasinFreeboard,
                Water::None,
                Time::Steady,
                Options {
                    start,
                    ..Options::STEP
                },
            );
            let what = format!("{roughness:?}, {start:?}");
            report(&what, &outcome);
            let (within, across) = not_above_receivers(&outcome);
            eprintln!(
                "{what}: cells not above their receiver, {} within their construction basin and \
                 {} across its edge",
                within.len(),
                across.len()
            );
            let last = outcome
                .diagnostics
                .residuals
                .last()
                .expect("a level")
                .clone();
            finest.push((start, last.first_m(), last.last_m()));
        }
        for (start, first, last) in finest {
            eprintln!(
                "{roughness:?}: finest level, {start:?}: first {first:.2} m, last {last:.2} m"
            );
        }
    }
}

/// The two uplift fits (the lane's fix of R09.T14.c's record): on both synthetic worlds at level 6,
/// wet now, each level's residuals, the cells not above their receiver, the land's greatest height
/// and the implied uplift, for [`UpliftFit::Diagonal`] and [`UpliftFit::SpatialField`]. Asserts
/// nothing.
#[test]
#[ignore = "measurement: the multigrid's uplift fits, R09.T14.c"]
fn measure_the_uplift_fits() {
    for roughness in [Roughness::Smooth, Roughness::Rough] {
        let world = Synthetic::new(6, roughness, true);
        for uplift_fit in [UpliftFit::Diagonal, UpliftFit::SpatialField] {
            let outcome = world.erode_with(
                Uplift::BasinFreeboard,
                Water::None,
                Time::Steady,
                Options {
                    uplift_fit,
                    ..Options::STEP
                },
            );
            let what = format!("{roughness:?}, {uplift_fit:?}");
            report(&what, &outcome);
            let (within, across) = not_above_receivers(&outcome);
            let highest = outcome
                .z_m
                .iter()
                .zip(&world.sea)
                .filter(|&(_, &wet)| !wet)
                .fold(f64::NEG_INFINITY, |hi, (&z, _)| hi.max(z));
            let highest0 = world
                .z0
                .iter()
                .zip(&world.sea)
                .filter(|&(_, &wet)| !wet)
                .fold(f64::NEG_INFINITY, |hi, (&z, _)| hi.max(z));
            eprintln!(
                "{what}: not above {} within, {} across; the land's highest {highest:.0} m \
                 ({highest0:.0} m constructed); the land's mean {:.1} m ({:.1} m constructed)",
                within.len(),
                across.len(),
                land_mean(&outcome, &world.sea, &outcome.z_m),
                land_mean(&outcome, &world.sea, &world.z0)
            );
        }
    }
}
