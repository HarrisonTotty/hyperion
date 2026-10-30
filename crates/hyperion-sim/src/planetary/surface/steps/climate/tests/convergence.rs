//! The energy-balance model's seasonal runs converge under their cap (R09.T13.i; the lane's lean,
//! 2026-10-30): G1's drift stop and the held G2 on the small Earth, pinned in a golden on both
//! targets, every reference world's kept run within [`MAX_ORBITS`], and the held run's state from
//! two starts.

use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f64_digest};

use super::super::coordinates::Coordinates;
use super::super::ebm::{
    self, CONVERGENCE_K, CorrectionRun, Ground, KeptRun, MAX_ORBITS, MeanCorrection, Relief,
};
use super::super::grid::{GridClimate, LATITUDE_BANDS, ModelGrid};
use super::super::ocean::OceanTransport;
use super::super::sun::NOMINAL_STEP;
use super::super::{SurfaceMeans, point_sea_shares, reliefs, run_diagnosed, sea_transport};
use crate::GENERATOR_VERSION;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::planetary::surface::reference;
use crate::planetary::surface::steps::{Pass, Working, craters, plates, relief};
use hyperion_surface::field::coarse_level;

/// How near plan 14's mean a kept run's surface mean must lie, K: 0.3, the ruled Earth's (decision-
/// r09-t13i's §2.4 follow-up §3, test 8).
const KEPT_MEAN_K: f64 = 0.3;

/// `inputs`' world staged to its climate step under the reference seed at its own level, the
/// pass's steps before it in the pass's order, through `then` with the pass and the working state.
fn staged<T>(inputs: &CoarseInputs, then: impl FnOnce(&Pass<'_>, &mut Working) -> T) -> T {
    let level = coarse_level(inputs.radius());
    let (field, climate) = (CellGraph::for_field(level), CellGraph::for_climate(level));
    let pass = Pass::new(reference::SEED, inputs, &field, &climate);
    let mut working = Working::initial(&pass);
    plates::run(&pass, &mut working);
    relief::run(&pass, &mut working);
    craters::run(&pass, &mut working);
    relief::run_after_craters(&pass, &mut working);
    then(&pass, &mut working)
}

/// `inputs`' climate step's energy-balance surface means, `None` on a world of another model.
fn surface_means(inputs: &CoarseInputs) -> Option<SurfaceMeans> {
    staged(inputs, |pass, working| {
        run_diagnosed(pass, working).surface_means
    })
}

/// G1 and G2 of a correction on the whole grid, the run kept, and whether G2 took the defect's
/// step.
fn geographic_runs(correction: &MeanCorrection) -> (CorrectionRun, CorrectionRun, KeptRun, bool) {
    match correction {
        MeanCorrection::Searched {
            g1,
            g2,
            kept,
            defect,
            ..
        } => (*g1, *g2, *kept, defect.is_some()),
        MeanCorrection::None | MeanCorrection::Clamped | MeanCorrection::Zonal { .. } => {
            panic!("the whole grid's correction runs G1 and G2: {correction:?}")
        }
    }
}

/// Writes one correction run's record to `w` under `label`, its floats by their bits.
fn write_run(w: &mut GoldenWriter, label: &str, run: &CorrectionRun) {
    w.f64(&format!("{label}.u"), run.u);
    w.f64(&format!("{label}.mean_k"), run.mean_k);
    w.line(&format!(
        "{label}.orbits = {} jumps = {} rejects = {} start = {:?} converged = {} searched = {} \
         held = {}",
        run.orbits, run.jumps, run.rejects, run.start, run.converged, run.searched, run.held
    ));
    match run.drift_k {
        Some(drift) => w.f64(&format!("{label}.drift_k"), drift),
        None => w.line(&format!("{label}.drift_k = none")),
    }
}

/// The small Earth (`reference::small_earth`, the tests' stand-in) under the reference seed: the
/// zonal search's u lies past the geographic model's warm fold, where G1 slid toward the snowball
/// and both G runs ended at the cap unconverged (279.1 and 280.4 K after 40 orbits each). Now G1's
/// drift stop ends it within a few orbits, G2 is the held run, kept, converged under the cap within
/// 0.3 K of plan 14's mean, and the two take at most one cap's orbits together. The golden pins
/// every run of the correction and the climate cells' temperatures, so that the held run is the
/// same to the bit natively and as WebAssembly.
#[test]
fn the_small_earth_s_g1_drifts_and_its_held_g2_converges_under_the_cap() {
    let inputs = reference::small_earth();
    let (means, cells) = staged(&inputs, |pass, working| {
        let means = run_diagnosed(pass, working)
            .surface_means
            .expect("the small Earth runs the energy balance");
        let mut cells = Vec::new();
        for state in &working.climate {
            cells.push(state.sea_level_temperature.value());
            cells.extend(state.month_anomaly_k);
        }
        (means, cells)
    });
    let (g1, g2, kept, stepped) = geographic_runs(&means.correction);
    let mean = inputs.mean_surface_temperature().value();
    eprintln!("the small Earth: G1 {g1:?}; G2 {g2:?}; kept {kept:?}");
    assert!(g1.drift_k.is_some(), "G1's drift stop ends it: {g1:?}");
    assert!(!g1.converged, "a drifting G1 is not a climate: {g1:?}");
    assert!(g2.held && g2.converged && !stepped, "{g2:?}");
    assert_eq!(kept, KeptRun::G2);
    assert!(g2.orbits < MAX_ORBITS, "{g2:?}");
    assert!(g1.orbits + g2.orbits <= MAX_ORBITS, "{g1:?}; {g2:?}");
    assert!((g2.mean_k - mean).abs() <= KEPT_MEAN_K, "{g2:?}");
    assert_eq!(means.orbits, g2.orbits, "the kept run's orbits are G2's");
    let MeanCorrection::Searched { search, .. } = &means.correction else {
        unreachable!("geographic_runs read a search");
    };
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (i, record) in search.iter().enumerate() {
        write_run(&mut w, &format!("z{i}"), &record.run);
    }
    write_run(&mut w, "g1", &g1);
    write_run(&mut w, "g2", &g2);
    w.line(&format!("kept = {kept:?}"));
    w.f64("seasonal_k", means.seasonal_k);
    w.u64_hex("cells", f64_digest(&cells));
    golden!("planetary/surface/small_earth_climate", w.as_str());
}

/// Every reference world that runs the energy balance converges under the cap: the reference
/// Earth's kept G2 by the ruled defect's step, its G1 untouched by the drift stop; the small Earth's
/// held G2; the locked Earth in its one-month year, its steady state its climate. The other
/// reference worlds run other models.
#[test]
#[ignore = "slow: the reference Earth's pass to its climate at level 8 with the six other \
            reference worlds', about 85 s"]
fn every_reference_world_s_energy_balance_converges_under_the_cap() {
    let worlds = [
        ("Earth", reference::earth_like()),
        ("Mars", reference::mars_like()),
        ("Moon", reference::moon_like()),
        ("Ceres", reference::ceres_like()),
        ("Europa", reference::europa_like()),
        ("locked Earth", reference::locked_earth_like()),
        ("small Earth", reference::small_earth()),
    ];
    let mut ran = Vec::new();
    for (name, inputs) in worlds {
        let Some(means) = surface_means(&inputs) else {
            continue;
        };
        eprintln!(
            "{name}: {} orbits, seasonal {:.3} K; {:?}",
            means.orbits, means.seasonal_k, means.correction
        );
        match &means.correction {
            MeanCorrection::None => assert_eq!(means.orbits, 0, "{name}"),
            MeanCorrection::Clamped | MeanCorrection::Zonal { .. } => {
                panic!("{name}: a lit world's grid corrects its mean by G1 and G2")
            }
            correction @ MeanCorrection::Searched { .. } => {
                let (g1, g2, kept, stepped) = geographic_runs(correction);
                let kept_run = match kept {
                    KeptRun::G1 => g1,
                    KeptRun::G2 => g2,
                };
                assert!(kept_run.converged, "{name}: {kept_run:?}");
                assert!(kept_run.orbits < MAX_ORBITS, "{name}: {kept_run:?}");
                let mean = inputs.mean_surface_temperature().value();
                assert!(
                    (kept_run.mean_k - mean).abs() <= KEPT_MEAN_K,
                    "{name}: {kept_run:?}"
                );
                if name == "Earth" {
                    assert!(
                        g1.drift_k.is_none() && !g2.held && stepped,
                        "the reference Earth keeps the ruled defect's step: {g1:?}; {g2:?}"
                    );
                }
            }
        }
        ran.push(name);
    }
    assert_eq!(ran, ["Earth", "locked Earth", "small Earth"]);
}

/// The held run's annual climate from the fit's steady state and from it shifted by `shift_k` at
/// every point, over the set-up `(inputs, grid, sea_share, ground, ocean)`: both converge under the
/// cap within the climate's bound of plan 14's mean, and their annual fields agree within twice the
/// climate's bound at every point, the means within the bound.
fn assert_held_from_either_start(
    name: &str,
    (inputs, grid, sea_share, ground, ocean): (
        &CoarseInputs,
        &ModelGrid,
        &[f64],
        Ground<'_>,
        OceanTransport,
    ),
    shift_k: f64,
) {
    let held =
        |shift: f64| ebm::solve_held(inputs, grid, sea_share, NOMINAL_STEP, ground, ocean, shift);
    let ((from_fit, fit_run), (shifted, shifted_run)) = (held(0.0), held(shift_k));
    eprintln!("{name}: from the fit {fit_run:?}; shifted {shift_k:+} K {shifted_run:?}");
    let mean = inputs.mean_surface_temperature().value();
    for run in [fit_run, shifted_run] {
        assert!(
            run.held && run.converged && run.orbits < MAX_ORBITS,
            "{name}: {run:?}"
        );
        assert!(
            (run.mean_k - mean).abs() <= CONVERGENCE_K,
            "{name}: {run:?}"
        );
    }
    assert!(
        (fit_run.mean_k - shifted_run.mean_k).abs() <= CONVERGENCE_K,
        "{name}: {fit_run:?}; {shifted_run:?}"
    );
    let worst = largest_difference(&from_fit, &shifted);
    eprintln!("{name}: the two annual fields' largest difference {worst:.4} K");
    assert!(worst <= 2.0 * CONVERGENCE_K, "{name}: {worst} K");
}

/// The largest difference between two climates' annual means at any point, K.
fn largest_difference(a: &GridClimate, b: &GridClimate) -> f64 {
    a.annual_k
        .iter()
        .zip(&b.annual_k)
        .fold(0.0, |most: f64, (x, y)| most.max((x - y).abs()))
}

/// The held run is the same from either start where its warm branch lies far from its fold, so
/// that both starts lie in that branch's basin: the reference Earth's zonal model at the sea level,
/// 70% sea, from its fit's steady state and from it 2 K colder.
#[test]
fn the_zonal_earth_s_held_run_is_the_same_from_either_start() {
    let inputs = reference::earth_like();
    assert_held_from_either_start(
        "the zonal Earth",
        (
            &inputs,
            &ModelGrid::zonal(),
            &[0.7; LATITUDE_BANDS],
            Ground::SeaLevel,
            OceanTransport::None,
        ),
        -2.0,
    );
}

/// The small Earth's held run on its whole grid, its relief and its sea, is the same from its fit's
/// steady state and from it 1 K colder, near its warm fold, where a run at a fixed u from either
/// start slows critically or slides to the snowball.
#[test]
#[ignore = "slow: the small Earth's pass to its climate and two held runs on its whole grid, \
            about 35 s"]
fn the_small_earth_s_held_run_is_the_same_from_either_start() {
    let inputs = reference::small_earth();
    staged(&inputs, |pass, working| {
        let grid = ModelGrid::geographic();
        let sea_share = point_sea_shares(pass, working, &grid, Coordinates::Geographic);
        let (on_grid, zonal): (Relief, Relief) =
            reliefs(pass, working, &grid, Coordinates::Geographic);
        let ground = Ground::Relief {
            grid: &on_grid,
            zonal: &zonal,
        };
        let ocean = sea_transport(pass, working);
        assert_held_from_either_start(
            "the small Earth",
            (&inputs, &grid, &sea_share, ground, ocean),
            -1.0,
        );
    });
}
