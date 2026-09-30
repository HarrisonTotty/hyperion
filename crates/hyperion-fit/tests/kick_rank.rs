//! The kick law's rank table (plan 06, P06.T19.b; plan 15, P15.T5.a): the committed body is the
//! rendering of its quantiles, and the task's smoke run goes end to end through the command line,
//! the same bytes on one thread and on four.

use std::fs;

use clap::Parser;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, TableText};
use hyperion_fit::task::find;
use hyperion_fit::tasks::kick_rank::{DRAWS, KickRankFit, SEED, render};
use hyperion_sim::tables::kick_rank;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/kick_rank.rs");

/// The committed body is the task's rendering of the committed quantiles, byte for byte. The fit
/// itself takes a quarter of an hour, so it is not rerun here: `hyperion-fit check` holds the
/// file to its inputs hash and body hash, and the sim's slow test
/// `ranks_of_fresh_reference_scores_are_uniform` checks the quantiles against fresh scores.
#[test]
fn the_committed_body_is_the_rendering_of_its_quantiles() {
    let fit = KickRankFit {
        quantiles: kick_rank::SCORE_QUANTILES,
        draws: DRAWS,
        seed: SEED,
    };
    let committed = TableText::of_file(COMMITTED);
    assert_eq!(render(&fit).body().unwrap(), committed.body);
    let header = committed.header().unwrap();
    assert_eq!(
        header.kind,
        Some(HeaderKind::Provisional {
            by: "P06.T19.b".to_owned()
        })
    );
    assert_eq!(header.task.as_deref(), Some("kick_rank"));
}

/// The declared items of a table, in order: every `pub const` or `pub static` of its body.
fn declared(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|l| {
            l.strip_prefix("pub const ")
                .or_else(|| l.strip_prefix("pub static "))
        })
        .filter_map(|l| l.split(':').next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_smoke_run_passes_the_header_grammar_and_is_the_same_on_any_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut texts = Vec::new();
    for threads in ["1", "4"] {
        let out = dir.path().join(format!("kick_rank_{threads}.rs"));
        let cli = Cli::try_parse_from([
            "hyperion-fit",
            "run",
            "kick_rank",
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
    assert_eq!(texts[0], texts[1]);
    let split = TableText::of_file(&texts[0]);
    let header = split.header().unwrap();
    assert!(matches!(header.kind, Some(HeaderKind::Provisional { .. })));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/kick_rank.smoke.toml")
    );
    assert!(
        header
            .acceptance
            .as_deref()
            .is_some_and(|a| a.contains("300 fresh scores") && a.contains("strictly increasing")),
        "{header:?}"
    );
    let task = find("kick_rank").unwrap();
    assert_eq!(declared(&split.body), task.items());
    assert_eq!(declared(&TableText::of_file(COMMITTED).body), task.items());
}
