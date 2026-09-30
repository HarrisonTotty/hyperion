//! The displaced form table (plan 15, P15.T6.c–f): the committed table declares the task's items
//! and is provisional, and the task's smoke run goes end to end through the command line from
//! freshly made smoke histograms, the same bytes on one thread and on three.

use std::fs;
use std::num::NonZeroUsize;
use std::path::Path;

use clap::Parser;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::data::Provenance;
use hyperion_fit::emit::{HeaderKind, TableText};
use hyperion_fit::manifest::Manifest;
use hyperion_fit::task::find;
use hyperion_fit::tasks::displaced_forms::table::{ORBITS_FILE, read_orbits};
use hyperion_fit::tasks::displaced_forms::{self, RunParams};

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/displaced_forms.rs");

/// The declared items of a table, in order: every `pub const` of its body.
fn declared(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|l| l.strip_prefix("pub const "))
        .filter_map(|l| l.split(':').next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_committed_table_is_provisional_and_declares_the_items() {
    let committed = TableText::of_file(COMMITTED);
    let header = committed.header().unwrap();
    assert!(
        matches!(header.kind, Some(HeaderKind::Provisional { .. })),
        "{header:?}"
    );
    assert_eq!(header.task.as_deref(), Some("displaced_forms"));
    assert_eq!(
        declared(&committed.body),
        find("displaced_forms").unwrap().items()
    );
}

/// The smoke histograms made again here are the bytes `data/displaced_smoke/PROVENANCE.toml`
/// records (so the committed table's input is reproducible, in this profile as in the one that
/// made it), and the smoke fit on them is the same on one thread and on three.
#[test]
#[ignore = "slow: the smoke orbits and two smoke fits in a debug build"]
fn the_smoke_fit_runs_end_to_end_on_fresh_smoke_histograms() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let orbit_manifest =
        Manifest::load(&crate_dir.join("manifests/displaced_orbits.smoke.toml")).unwrap();
    let params = RunParams::from_manifest(&orbit_manifest).unwrap();
    let hash = displaced_forms::manifest_hash(&orbit_manifest);
    let records = displaced_forms::run(&params, &hash, NonZeroUsize::new(3).unwrap(), None)
        .unwrap()
        .expect("a run without a limit completes");
    let text = displaced_forms::render(&orbit_manifest, &records);
    let provenance = Provenance::load(&crate_dir.join("data"), "displaced_smoke").unwrap();
    assert_eq!(
        displaced_forms::sha256_hex(text.as_bytes()),
        provenance.files[0].sha256
    );
    assert_eq!(read_orbits(&text).unwrap().1, records);
    // Under the workspace's target directory, not the system's temporary one.
    let dir = crate_dir.join("../../target/tmp/displaced_forms_smoke_fit");
    let _ = fs::remove_dir_all(&dir); // A leftover of an earlier run, if any.
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(ORBITS_FILE), &text).unwrap();
    let mut tables = Vec::new();
    for threads in ["1", "3"] {
        let out = dir.join(format!("displaced_forms_{threads}.rs"));
        let cli = Cli::try_parse_from([
            "hyperion-fit",
            "run",
            "displaced_forms",
            "--smoke",
            "--threads",
            threads,
            "--data",
            dir.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .unwrap();
        let mut log = Vec::new();
        run(&cli, &mut log).unwrap();
        tables.push(fs::read_to_string(&out).unwrap());
    }
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(tables[0], tables[1]);
    let split = TableText::of_file(&tables[0]);
    let header = split.header().unwrap();
    assert!(matches!(header.kind, Some(HeaderKind::Provisional { .. })));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/displaced_forms.smoke.toml")
    );
    assert!(
        header
            .acceptance
            .as_deref()
            .is_some_and(|a| a.contains("smoke histograms") && a.contains("misplaced")),
        "{header:?}"
    );
    assert_eq!(
        declared(&split.body),
        find("displaced_forms").unwrap().items()
    );
}
