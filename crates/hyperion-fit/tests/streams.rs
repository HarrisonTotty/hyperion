//! The committed streams table is what `run streams --since <current>` writes (plan 15, P15.T11.b),
//! its held multiplier is the brainstorm's 1.5 and its fitted one the fit's, bit for bit.
//!
//! The task is slow, so CI holds the table to its inputs hash and fingerprint
//! (`hyperion-fit check`), and the slow tier reruns it on its committed manifest and runs its
//! smoke manifest end to end through the command line, on one thread and on three.

mod common;

use std::fs;
use std::num::NonZeroUsize;

use clap::Parser as _;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, TableText, Workspace};
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::tables::streams::{ORPHAN_STREAMS_PER_GLOBULAR, ORPHAN_STREAMS_PER_GLOBULAR_27};
use hyperion_testkit::float::assert_same_bits;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/streams.rs");

#[test]
#[ignore = "slow: eight galaxies' sprays and tubes, about five minutes on 16 threads in a release build"]
fn streams_table_is_reproduced() {
    let threads = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let planned = rerender(
        registry(),
        find("streams").expect("streams is registered"),
        &Workspace::repository(),
        threads,
        GENERATOR_VERSION.get(),
    )
    .expect("the streams task runs");
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/streams.rs",
        "streams",
        COMMITTED,
        &planned.text,
    );
}

/// The held multiplier the sim compiles is the brainstorm's 1.5 ("Streams and accreted
/// structure"), bit for bit. The literal is the test's own, so that a changed task cannot pass by
/// its own re-emission.
#[test]
fn the_committed_multiplier_is_the_brainstorms_one_and_a_half() {
    assert_same_bits(ORPHAN_STREAMS_PER_GLOBULAR, 1.5);
}

/// The multiplier version 27 reads is P15.T11.b's fit as committed, bit for bit, inside the
/// plan's 0.5–4.5. The literal is the test's own: a refit that moves it changes this test.
#[test]
fn the_27_multiplier_is_the_fit_s() {
    assert_same_bits(ORPHAN_STREAMS_PER_GLOBULAR_27, FITTED);
    assert!((0.5..=4.5).contains(&ORPHAN_STREAMS_PER_GLOBULAR_27));
}

/// P15.T11.b's fitted multiplier, as its production run gave it.
const FITTED: f64 = 2.048_842_063_164_735_7;

/// The smoke run goes end to end through the command line on one thread and on three, into a
/// temporary directory, and writes the same table both times, in the header grammar, declaring
/// the task's items (Design notes 2 and 11): the tubes are built on the pool and gathered in
/// index order.
#[test]
#[ignore = "slow: one galaxy's sprays and tubes, twice, in a debug build"]
fn the_streams_smoke_run_passes_the_header_grammar_and_is_the_same_on_any_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut texts = Vec::new();
    for threads in ["1", "3"] {
        let out = dir.path().join(format!("streams_{threads}.rs"));
        let cli = Cli::try_parse_from([
            "hyperion-fit",
            "run",
            "streams",
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
        "streams --smoke: --threads 1 and --threads 3 wrote different tables"
    );
    let table = TableText::of_file(&texts[0]);
    let header = table.header().unwrap();
    assert_eq!(header.task.as_deref(), Some("streams"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/streams.smoke.toml")
    );
    assert!(
        header
            .acceptance
            .as_deref()
            .is_some_and(|a| a.contains("globular-like known streams") && a.contains("1 galaxies")),
        "{header:?}"
    );
    assert!(
        matches!(
            header.kind,
            Some(HeaderKind::Generated | HeaderKind::Provisional { .. })
        ),
        "{header:?}"
    );
    assert_eq!(
        common::declared(&table.body),
        find("streams").unwrap().items()
    );
}
