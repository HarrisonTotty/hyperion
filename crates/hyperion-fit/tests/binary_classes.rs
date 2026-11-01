//! The binary classes' fits (plan 15, P15.T10.b): `binary_awd`'s smoke manifest runs end to end
//! through the command line on one thread and on four, into a temporary directory, and writes the
//! same block both times, in the header grammar, declaring the task's items (Design notes 2 and
//! 11). The task cuts its censuses into fixed parts and joins them in order, so the pair checks
//! that the thread split leaves the bytes alone. The full run's acceptance, its samplers against
//! the half of the census it was not fitted to, is the sim's slow
//! `tables_hold::the_accreting_white_dwarf_samplers_against_the_engine`.
//!
//! `toolchain.rs` already checks every committed table's declared items and runs `fit-check`
//! without reruns, so neither is repeated here for the committed file.

mod common;

use std::fs;

use clap::Parser as _;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, TableText};
use hyperion_fit::task::find;

/// The accreting white dwarfs' smoke run writes the same block on one thread and on four, its
/// header in the grammar and its items the task's.
#[test]
fn the_binary_awd_smoke_run_is_the_same_on_any_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut texts = Vec::new();
    for threads in ["1", "4"] {
        let out = dir.path().join(format!("binary_awd_{threads}.rs"));
        let cli = Cli::try_parse_from([
            "hyperion-fit",
            "run",
            "binary_awd",
            "--smoke",
            "--threads",
            threads,
            "--out",
            out.to_str().unwrap(),
        ])
        .unwrap();
        let mut log = Vec::new();
        run(&cli, &mut log).unwrap();
        texts.push(fs::read_to_string(&out).unwrap());
    }
    assert_eq!(
        texts[0], texts[1],
        "binary_awd --smoke: --threads 1 and --threads 4 wrote different blocks"
    );
    // `--out` writes the task's block alone, between its markers.
    let block = TableText::of_block(&texts[0], "binary_awd")
        .expect("the smoke run's block parses")
        .expect("the smoke run writes the binary_awd block");
    let header = block.header().unwrap();
    assert_eq!(header.task.as_deref(), Some("binary_awd"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/binary_awd.smoke.toml")
    );
    assert_eq!(header.kind, Some(HeaderKind::Generated));
    assert_eq!(
        common::declared(&block.body),
        find("binary_awd").unwrap().items()
    );
}
