//! The binary classes' fits (plan 15, P15.T10.b): `binary_awd`'s and `binary_xrb`'s smoke
//! manifests run end to end through the command line on one thread and on four, into a temporary
//! directory, and each writes the same block both times, in the header grammar, declaring the
//! task's items (Design notes 2 and 11). The tasks cut their censuses into fixed parts and join
//! them in order, so the pairs check that the thread split leaves the bytes alone. The full runs'
//! acceptance, their samplers against the half of the census they were not fitted to, is the sim's
//! slow
//! `tables_hold::the_accreting_white_dwarf_samplers_against_the_engine` and
//! `tables_hold::the_x_ray_binary_sampler_against_the_engine`.
//!
//! The accreting white dwarfs' rows by donor kind (P11.T5's fix, `donor_rows`), which the refit at
//! 27 runs, meet their cited figures on the smoke manifest's censuses, read through the galaxy's
//! own components as the class reads a row.
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
use hyperion_fit::tasks::binary_classes::donor_rows::{
    KindTarget, OBSERVED_DONOR_KINDS, SUN_LY, awd_share_rows,
};
use hyperion_fit::tasks::binary_classes::{AwdParams, ShareRow, run_censuses};
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_PARSEC;

/// Task `task`'s smoke run on `threads` threads into `dir`: the file it wrote.
fn smoke(task: &str, threads: &str, dir: &std::path::Path) -> String {
    let out = dir.join(format!("{task}_{threads}.rs"));
    let cli = Cli::try_parse_from([
        "hyperion-fit",
        "run",
        task,
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

/// `--out` writes task `task`'s block alone, between its markers: its header in the grammar, from
/// its smoke manifest, `@generated`, and its items the task's.
fn assert_smoke_block(text: &str, task: &str) {
    let block = TableText::of_block(text, task)
        .expect("the smoke run's block parses")
        .expect("the smoke run writes the task's block");
    let header = block.header().unwrap();
    assert_eq!(header.task.as_deref(), Some(task));
    assert_eq!(
        header.manifest.as_deref(),
        Some(format!("crates/hyperion-fit/manifests/{task}.smoke.toml").as_str())
    );
    assert_eq!(header.kind, Some(HeaderKind::Generated));
    assert_eq!(common::declared(&block.body), find(task).unwrap().items());
}

/// The accreting white dwarfs' smoke run writes the same block on one thread and on four, its
/// header in the grammar and its items the task's.
#[test]
fn the_binary_awd_smoke_run_is_the_same_on_any_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let one = smoke("binary_awd", "1", dir.path());
    let four = smoke("binary_awd", "4", dir.path());
    assert_eq!(
        one, four,
        "binary_awd --smoke: --threads 1 and --threads 4 wrote different blocks"
    );
    assert_smoke_block(&one, "binary_awd");
}

/// The X-ray binaries' smoke run writes the same block on one thread and on four, its header in
/// the grammar and its items the task's.
#[test]
fn the_binary_xrb_smoke_run_is_the_same_on_any_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let one = smoke("binary_xrb", "1", dir.path());
    let four = smoke("binary_xrb", "4", dir.path());
    assert_eq!(
        one, four,
        "binary_xrb --smoke: --threads 1 and --threads 4 wrote different blocks"
    );
    assert_smoke_block(&one, "binary_xrb");
}

/// On the smoke manifest's censuses each donor kind's rows give its cited figure to 1%, read as
/// the class reads a row: each component's hosts a system its population's hosts per solar mass
/// formed × the mass formed × its born fraction, times its density at the Sun-like point or its
/// systems in the galaxy. The smoke census at 27 finds members of the cataclysmic variables and
/// the AM Canum Venaticorum stars, and no symbiotic star, whose rows are then zero.
#[test]
fn the_donor_kinds_rows_meet_their_cited_figures() {
    let manifest = Manifest::from_bytes(
        "manifests/binary_awd.smoke.toml".as_ref(),
        include_bytes!("../manifests/binary_awd.smoke.toml").to_vec(),
    )
    .unwrap();
    let params = AwdParams::of(&manifest).unwrap();
    let galaxy = params.census.galaxy();
    let censuses = run_censuses(&galaxy, &params.census, NonZeroUsize::MIN).unwrap();
    // The symbiotic stars at Lü, Yungelson and Han's 10⁴ (research-leans-23).
    assert_eq!(OBSERVED_DONOR_KINDS[2], KindTarget::GalacticCount(1.0e4));
    let rows = awd_share_rows(&galaxy, &censuses, &OBSERVED_DONOR_KINDS);
    assert!(
        rows.scales[0].is_some() && rows.scales[1].is_some(),
        "{:?}",
        rows.scales
    );
    let [x, y, z] = SUN_LY;
    let sun = PointLy::new(x, y, z);
    let formed = galaxy.mean_formed_mass().value();
    let pc3 = LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC;
    for (k, target) in OBSERVED_DONOR_KINDS.iter().enumerate() {
        if rows.scales[k].is_none() {
            let zero = |row: &ShareRow| {
                row.hosts_per_formed_mass
                    .iter()
                    .all(|h| h.abs() < f64::MIN_POSITIVE)
            };
            assert!(rows.by_kind[k].iter().all(zero), "{k}");
            continue;
        }
        let (mut density, mut hosts) = (0.0, 0.0);
        for row in &rows.by_kind[k] {
            for c in galaxy.fields().components() {
                let per_system = row.hosts_per_formed_mass[c.population().index()]
                    * formed
                    * c.ages().born_fraction();
                density += per_system * c.density(&sun) * pc3;
                hosts += per_system * c.count_with_unborn();
            }
        }
        let (got, wanted) = match *target {
            KindTarget::DensityPerCubicParsec(n) => (density, n),
            KindTarget::GalacticCount(n) => (hosts, n),
        };
        println!("kind {k}: {got:.4e} against {wanted:.4e}, {hosts:.4e} hosts");
        assert!(
            (got / wanted - 1.0).abs() < 0.01,
            "kind {k}: {got} against {wanted}"
        );
    }
}
