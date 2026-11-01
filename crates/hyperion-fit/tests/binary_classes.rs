//! The binary classes' fits (plan 15, P15.T10.b): `binary_awd`'s smoke manifest runs end to end
//! through the command line on one thread and on four, into a temporary directory, and writes the
//! same block both times, in the header grammar, declaring the task's items (Design notes 2 and
//! 11). The task cuts its censuses into fixed parts and joins them in order, so the pair checks
//! that the thread split leaves the bytes alone; `binary_xrb`, which runs the same censuses, has
//! its smoke run once. The full runs' acceptance, their samplers against the half of the census
//! they were not fitted to, is the sim's slow
//! `tables_hold::the_accreting_white_dwarf_samplers_against_the_engine` and
//! `tables_hold::the_x_ray_binary_sampler_against_the_engine`.
//!
//! `toolchain.rs` already checks every committed table's declared items and runs `fit-check`
//! without reruns, so neither is repeated here for the committed file.

mod common;

use std::fs;

use clap::Parser as _;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, TableText};
use hyperion_fit::task::find;

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

/// The X-ray binaries' smoke run writes its block, its header in the grammar and its items the
/// task's.
#[test]
fn the_binary_xrb_smoke_run_writes_its_block() {
    let dir = tempfile::tempdir().unwrap();
    assert_smoke_block(&smoke("binary_xrb", "4", dir.path()), "binary_xrb");
}
