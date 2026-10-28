//! The helium-excess table (plan 06, P06.T17; plan 15, P15.T7.b): the committed table is exactly
//! what `run helium --since <current>` would write, P06.T17's identity bit for bit; the reduced
//! grid parses and is the provenance's; the fit from `helium.production.toml` is measured against
//! every criterion of P15.T7.b's acceptance and runs end to end; and the sim's builder under a
//! candidate table is the generator's track wherever the table cannot act.
//!
//! CI runs this, so a table that is stale, or edited by hand, fails it.

mod common;

use std::fs;
use std::num::NonZeroUsize;

use clap::Parser;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::data::{Provenance, load_dataset};
use hyperion_fit::emit::{HeaderKind, TableText, Workspace};
use hyperion_fit::manifest::{Manifest, ManifestParamError};
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{RunTaskError, find, registry};
use hyperion_fit::tasks::helium::cluster::ClusterFigures;
use hyperion_fit::tasks::helium::figures::{Figures, IDENTITY_PROBES, probe_ages_yr};
use hyperion_fit::tasks::helium::fit::{DATASET, Fit, FitParams, fit, slope_at};
use hyperion_fit::tasks::helium::grid::Grid;
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::sse::{CandidateHeliumTable, Track, track_with_helium_table};
use hyperion_sim::tables::helium::{HB_TEMPERATURE_SHIFT, LIFETIME_SLOPE, LOG_Z_NODES};
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses, Years};
use hyperion_testkit::float::assert_same_bits;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/helium.rs");

/// The production manifest's path.
fn production_path() -> std::path::PathBuf {
    Workspace::repository()
        .manifests_dir
        .join("helium.production.toml")
}

/// The production manifest, loaded with the repository's data directory.
fn production() -> Manifest {
    Manifest::load(&production_path()).expect("the production manifest loads")
}

/// The reduced grid, read and verified against its provenance.
fn grid() -> Grid {
    let dataset = load_dataset(&Workspace::repository().data_dir, DATASET, None)
        .expect("the reduced grid is committed and matches its provenance");
    let text = |name: &str| {
        let (_, bytes) = dataset
            .files
            .iter()
            .find(|(n, _)| n == name)
            .expect("the dataset lists the file");
        std::str::from_utf8(bytes)
            .expect("the reduction writes UTF-8")
            .to_owned()
    };
    Grid::parse(&text("turnoff.csv"), &text("zahb.csv")).expect("the reduced grid parses")
}

/// The fit and its parameters from the production manifest.
fn production_fit() -> (Fit, FitParams, Grid) {
    let params =
        FitParams::from_manifest(&production()).expect("the production manifest is the fit's");
    let grid = grid();
    let fitted = fit(&grid, &params).expect("the grid determines the fit");
    (fitted, params, grid)
}

#[test]
fn helium_table_is_reproduced() {
    let planned = rerender(
        registry(),
        find("helium").expect("helium is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the helium task runs");
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/helium.rs",
        "helium",
        COMMITTED,
        &planned.text,
    );
}

/// The table the sim compiles is P06.T17's identity, bit for bit: plan 06's four nodes, at
/// [Fe/H] −2.2, −1.6, −1.0 and −0.5, and every coefficient zero. The literals are the test's own,
/// so that a changed manifest cannot pass by its own re-emission; the swap changes this test.
#[test]
fn the_committed_table_is_p06_t17s_identity() {
    let nodes = [
        -3.898_970_004_336_019,
        -3.298_970_004_336_019,
        -2.698_970_004_336_018_7,
        -2.198_970_004_336_018_7,
    ];
    for (committed, literal) in LOG_Z_NODES.iter().zip(nodes) {
        assert_same_bits(*committed, literal);
    }
    for row in LIFETIME_SLOPE {
        for c in row {
            assert_same_bits(c, 0.0);
        }
    }
    for c in HB_TEMPERATURE_SHIFT {
        assert_same_bits(c, 0.0);
    }
    let split = TableText::of_file(COMMITTED);
    let header = split.header().unwrap();
    assert_eq!(
        header.kind,
        Some(HeaderKind::Provisional {
            by: "P06.T17".to_owned()
        })
    );
    assert_eq!(header.task.as_deref(), Some("helium"));
    assert_eq!(
        common::declared(&split.body),
        find("helium").unwrap().items()
    );
}

/// The reduced output parses, holds every (Z, Y) of the grid (seven canonical sets and twenty
/// helium-enhanced ones, the solar row having only Y = 0.35 and 0.40) with every one of the
/// 0.6, 0.7, 0.8 and 0.9 M☉ tracks and a zero-age horizontal branch, and its files are the ones
/// the provenance records (`load_dataset` checks their SHA-256s).
#[test]
fn the_reduced_grid_holds_every_set_and_is_the_provenances() {
    let grid = grid();
    let sets = grid.sets();
    assert_eq!(sets.len(), 27);
    assert_eq!(grid.canonical().len(), 7);
    for set in &sets {
        for mass in [0.6, 0.7, 0.8, 0.9] {
            assert!(grid.log_age_yr(set, mass).is_some(), "{set:?} at {mass} M☉");
        }
        let branch = grid.branch(set);
        assert!(branch.len() >= 20, "{set:?}: {} points", branch.len());
        assert!(
            branch
                .iter()
                .all(|p| p.core_mass_msun > 0.45 && p.envelope_msun >= 0.0)
        );
    }
    let ys: Vec<f64> = sets
        .iter()
        .filter(|s| !s.is_canonical())
        .map(|s| s.y)
        .collect();
    assert_eq!(ys.iter().filter(|&&y| (y - 0.3).abs() < 1e-12).count(), 6);
    assert_eq!(ys.iter().filter(|&&y| (y - 0.35).abs() < 1e-12).count(), 7);
    assert_eq!(ys.iter().filter(|&&y| (y - 0.4).abs() < 1e-12).count(), 7);
}

/// Under the committed table at any ΔY, and under the fitted table at ΔY = 0, the builder's track
/// is the generator's, `Track::full`'s, bit for bit, over a spread of masses, metallicities and
/// ages.
#[test]
fn the_builder_is_the_generators_track_where_the_table_cannot_act() {
    let (fitted, _, _) = production_fit();
    let ages = probe_ages_yr();
    for &(m, fe_h) in &IDENTITY_PROBES {
        for (delta_y, table) in [
            (0.0, CandidateHeliumTable::COMMITTED),
            (0.1, CandidateHeliumTable::COMMITTED),
            (0.0, fitted.table),
        ] {
            let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(delta_y));
            let draws = StarDraws::median();
            let full = Track::full(SolarMasses::new(m), &composition, &draws);
            let built = track_with_helium_table(SolarMasses::new(m), &composition, &draws, &table);
            assert_eq!(full.lifetime(), built.lifetime(), "{m} M☉ at ΔY {delta_y}");
            for &age in &ages {
                let (a, b) = (
                    full.state_at(Years::new(age)),
                    built.state_at(Years::new(age)),
                );
                assert_eq!(a.phase(), b.phase());
                assert_same_bits(a.mass().value(), b.mass().value());
                assert_same_bits(a.luminosity().value(), b.luminosity().value());
                assert_same_bits(a.radius().value(), b.radius().value());
            }
        }
    }
}

/// Two fits from the production manifest give the same bits, and the same figures: the
/// acceptance line the header and the lock record, character for character.
#[test]
fn the_fit_is_deterministic() {
    let (a, params, grid) = production_fit();
    let (b, _, _) = production_fit();
    for (ra, rb) in a
        .table
        .lifetime_slope()
        .iter()
        .zip(&b.table.lifetime_slope())
    {
        for (x, y) in ra.iter().zip(rb) {
            assert_same_bits(*x, *y);
        }
    }
    for (x, y) in a
        .table
        .hb_temperature_shift()
        .iter()
        .zip(&b.table.hb_temperature_shift())
    {
        assert_same_bits(*x, *y);
    }
    for (x, y) in a.table.log_z_nodes().iter().zip(&LOG_Z_NODES) {
        assert_same_bits(*x, *y);
    }
    let line = |fit: &Fit| {
        let mut out = String::new();
        Figures::of(fit, &grid, &params).describe(fit, &params, &mut out);
        out
    };
    assert_eq!(line(&a), line(&b));
}

/// The fit from the production manifest, built in memory, against every criterion of P15.T7.b's
/// acceptance (plan 15, as ruled on 2026-10-23 and 2026-10-26, decision-p15-t7b-leans), measured
/// on plan 06's tracks under the fitted table. All pass, so the run is `@generated`.
#[test]
fn the_fit_meets_every_figure_as_ruled() {
    let (fitted, params, grid) = production_fit();
    let figures = Figures::of(&fitted, &grid, &params);
    // 72 points: six rows of [Fe/H] −2.40 to −0.42, which bracket −2.2 to −0.5 since each set
    // sits at its own Z, by three Y and four masses.
    assert_eq!(figures.lifetime.len(), 72);
    assert!(
        figures.worst_lifetime() <= 0.03,
        "{}",
        figures.worst_lifetime()
    );
    assert!(
        figures
            .lifetime
            .iter()
            .map(|c| c.point.delta_y)
            .fold(0.0, f64::max)
            > 0.15,
        "the checks reach the grid's top ΔY"
    );
    assert!(figures.identity_at_zero);
    // s at 0.1 and 150 M☉ at each node: eight values, all negative.
    assert_eq!(figures.slope_reach.len(), 8);
    assert!(
        figures.slope_reach.iter().all(|&s| s < 0.0),
        "{:?}",
        figures.slope_reach
    );
    // About a half at ΔY = 0.1 (L3): s is −6.5 to −7.4 at 0.8 M☉.
    assert_eq!(figures.shortening.len(), 4);
    for &(fe_h, shortening) in &figures.shortening {
        assert!(
            (0.42..=0.55).contains(&shortening),
            "{shortening} at [Fe/H] {fe_h}"
        );
    }
    let shift = figures
        .branch_shift
        .expect("the top Y has fitted branch points");
    assert!(
        (shift.fitted - shift.grid).abs() <= 0.1 * shift.grid.abs(),
        "{shift:?}"
    );
    let (own, grid_amount) = figures
        .cluster
        .as_ref()
        .and_then(ClusterFigures::heating)
        .expect("the cluster has a blue end in both populations");
    // Hotter at the grid's top ΔY (L4); plan 06's branch heats more steeply than BaSTI's as its
    // envelope thins, the recorded finding, so the cluster's rise exceeds the grid's.
    assert!(
        own > 0.0 && own > grid_amount,
        "{own} against {grid_amount}"
    );
    assert_eq!(figures.verdicts(&params), [true; 6]);
    assert!(figures.passed(&params));
}

/// The production fit's slope is linear in log₁₀ m, every c₂ exactly 0.0, and negative at 0.1
/// and 150 M☉ at each node, which bounds it at every mass between and, through the linear
/// interpolation in log₁₀ Z, at every Z (ruled 2026-10-26, decision-p15-t7b-leans L2).
#[test]
fn the_production_slope_is_linear_and_negative_at_every_mass() {
    let (fitted, _, _) = production_fit();
    let table = fitted.table;
    for row in table.lifetime_slope() {
        assert_same_bits(row[2], 0.0);
    }
    let nodes = table.log_z_nodes();
    for &log_z in &nodes {
        for m in [0.1, 150.0] {
            let s = slope_at(&table.lifetime_slope(), &nodes, log_z, m);
            assert!(s < 0.0, "s = {s} at {m} M☉ and log₁₀ Z {log_z}");
        }
    }
}

/// The production manifest end to end, through the command line with `--manifest` and `--out`
/// into a scratch directory: the header parses, names the task, the manifest and the dataset,
/// declares the three items and is `@generated`, every figure as ruled passing.
#[test]
fn the_production_manifest_runs_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("helium.rs");
    let manifest = production_path();
    // A fixed few threads, not every core. The task ignores the count, which is named so that the
    // test stays small beside the others if the task ever uses it.
    let cli = Cli::try_parse_from([
        "hyperion-fit",
        "run",
        "helium",
        "--manifest",
        manifest.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--threads",
        "2",
    ])
    .unwrap();
    let mut log = Vec::new();
    run(&cli, &mut log).unwrap();
    let text = fs::read_to_string(&out).unwrap();
    let split = TableText::of_file(&text);
    let header = split.header().unwrap();
    assert_eq!(header.task.as_deref(), Some("helium"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/helium.production.toml")
    );
    assert_eq!(header.kind, Some(HeaderKind::Generated));
    let data = header.data.as_deref().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].0, "basti_helium");
    let recorded = Provenance::load(&Workspace::repository().data_dir, DATASET)
        .unwrap()
        .recorded_hash();
    assert!(
        recorded.starts_with(&data[0].1),
        "{} against {recorded}",
        data[0].1
    );
    assert!(
        header
            .acceptance
            .as_deref()
            .is_some_and(|a| a.contains("lifetime ratios")
                && a.contains("(3% allowed; passes)")
                && !a.contains("MISSES")
                && a.contains("extrapolated to ΔY = 0.18")),
        "{header:?}"
    );
    assert_eq!(
        common::declared(&split.body),
        find("helium").unwrap().items()
    );
    // Plan 06's nodes keep their committed text, and so their bits.
    let committed = TableText::of_file(COMMITTED);
    let nodes = |body: &str| body.split("\n\n").next().map(str::to_owned);
    assert_eq!(nodes(&split.body), nodes(&committed.body));
}

/// A run on a manifest whose method is neither the identity nor the fit is refused, naming the
/// two.
#[test]
fn a_manifest_of_another_method_is_refused() {
    let manifest = Manifest::from_bytes(
        &production_path(),
        b"task = \"helium\"\ndatasets = [\"basti_helium\"]\n[params]\nmethod = \"spline\"\n"
            .to_vec(),
    )
    .unwrap();
    let result = find("helium").unwrap().run(&manifest, NonZeroUsize::MIN);
    match result {
        Err(RunTaskError::Param(e)) => assert_eq!(
            e,
            ManifestParamError::new(
                "method",
                "\"identity\" (P06.T17's table) or \"basti_least_squares\" (P15.T7.b's fit)"
            )
        ),
        other => panic!("expected a parameter error, got {other:?}"),
    }
}
