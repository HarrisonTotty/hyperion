//! The Type Ia explosion mark's yield (plan 15, P15.T9.b): `type_ia_yield`'s smoke manifest runs
//! end to end through the command line on one thread and on four, into a temporary directory, and
//! writes the same block both times, in the header grammar, declaring the task's items (Design
//! notes 2 and 11). The task cuts its census into fixed parts and joins them in order, so the pair
//! checks that the thread split leaves the bytes alone, and η is the target over the pool in
//! every bin.
//!
//! `toolchain.rs` already checks every committed table's declared items and runs `fit-check`
//! without reruns, so neither is repeated here for the committed file.

mod common;

use std::fs;
use std::num::NonZeroUsize;

use clap::Parser as _;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, TableText};
use hyperion_fit::manifest::Manifest;
use hyperion_fit::task::find;
use hyperion_fit::tasks::type_ia_yield::{PASS_SIGMA, YieldParams, fit_yield, run_census};
use hyperion_sim::tables::binary::IA_DELAY_BINS;
use hyperion_sim::tables::type_ia_delay::YIELD_PER_SOLAR_MASS;
use hyperion_testkit::float::assert_same_bits;

/// The task's smoke run on `threads` threads into `dir`: the file it wrote.
fn smoke(threads: &str, dir: &std::path::Path) -> String {
    let out = dir.join(format!("type_ia_yield_{threads}.rs"));
    let cli = Cli::try_parse_from([
        "hyperion-fit",
        "run",
        "type_ia_yield",
        "--smoke",
        "--threads",
        threads,
        "--out",
        out.to_str().unwrap(),
    ])
    .unwrap();
    let mut log = Vec::new();
    run(&cli, &mut log).unwrap();
    fs::read_to_string(&out).unwrap()
}

/// The smoke run writes the same block on one thread and on four, its header in the grammar, from
/// its smoke manifest, `@generated`, and its items the task's.
#[test]
fn the_type_ia_yield_smoke_run_is_the_same_on_any_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let one = smoke("1", dir.path());
    let four = smoke("4", dir.path());
    assert_eq!(
        one, four,
        "type_ia_yield --smoke: --threads 1 and --threads 4 wrote different blocks"
    );
    let block = TableText::of_block(&one, "type_ia_yield")
        .expect("the smoke run's block parses")
        .expect("the smoke run writes the task's block");
    let header = block.header().unwrap();
    assert_eq!(header.task.as_deref(), Some("type_ia_yield"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/type_ia_yield.smoke.toml")
    );
    assert_eq!(header.kind, Some(HeaderKind::Generated));
    assert_eq!(
        common::declared(&block.body),
        find("type_ia_yield").unwrap().items()
    );
}

/// On the smoke census each bin's η is min(1, target ÷ pool) of its pool, alike for both channels,
/// some candidates are pooled, and the exploded yield is min(pool, target) to its error. Its few
/// records weigh each candidate heavily, a layer-E one at some 2 × 10⁻⁴ per M☉ formed against a
/// bin's target of under 7.1 × 10⁻⁵, so some bins are not capped there as they are in the full run.
#[test]
fn the_smoke_census_s_eta_is_the_target_over_its_pool() {
    let text = include_str!("../manifests/type_ia_yield.smoke.toml");
    let manifest = Manifest::from_bytes(
        "manifests/type_ia_yield.smoke.toml".as_ref(),
        text.as_bytes().to_vec(),
    )
    .unwrap();
    let params = YieldParams::of(&manifest).unwrap();
    let galaxy = params.galaxy();
    let censuses = run_census(&galaxy, &params, NonZeroUsize::new(4).unwrap()).unwrap();
    let fit = fit_yield(&galaxy, &params, &censuses);
    for (k, [merger, accretion]) in fit.eta.iter().enumerate() {
        let pool = fit.pool.both(k);
        let expected = if pool > 0.0 {
            (YIELD_PER_SOLAR_MASS[k] / pool).min(1.0)
        } else {
            1.0
        };
        assert_same_bits(*merger, expected);
        assert_same_bits(*accretion, expected);
    }
    assert_eq!(fit.eta.len(), IA_DELAY_BINS);
    assert!(fit.candidates.1 > 0, "the smoke census pools candidates");
    // Each candidate on its own pair's mark: the exploded yield is min(pool, target) in every
    // bin, and η of the even parts that of the odd, to their errors at α = 10⁻³ over the bins.
    assert!(fit.worst_exploded < PASS_SIGMA, "{:?}", fit.worst_exploded);
    assert!(fit.worst_halves < PASS_SIGMA, "{:?}", fit.worst_halves);
}
