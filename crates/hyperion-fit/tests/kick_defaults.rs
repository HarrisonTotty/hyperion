//! The kick law's defaults review (plan 15, P15.T5.b): the committed body is the rendering of its
//! committed constants, and the task's smoke run goes end to end through the command line, the
//! same bytes on one thread and on four.

mod common;

use std::fs;

use clap::Parser;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, RustTable, TableText};
use hyperion_fit::task::find;
use hyperion_fit::tasks::kick_defaults::{
    Figures, KickDefault, Observable, Sensitivity, Sweep, items,
};
use hyperion_sim::tables::kick_defaults as table;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/kick_defaults.rs");

/// A centre's fourteen figures: the twelve observables, then the two held ones.
fn centre(values: [f64; 14]) -> Figures {
    Figures {
        values: core::array::from_fn(|i| values[i]),
        held: [values[12], values[13]],
    }
}

/// The sweep of `default` over `points`, its figures read from the observed and held rows.
fn sweep<const N: usize>(
    default: KickDefault,
    points: [f64; N],
    observed: &[[f64; N]; 12],
    held: &[[f64; N]; 2],
) -> Sweep {
    let figures = (0..N)
        .map(|j| Figures {
            values: core::array::from_fn(|i| observed[i][j]),
            held: [held[0][j], held[1][j]],
        })
        .collect();
    Sweep::new(default, points.to_vec(), figures).expect("the committed grid holds its default")
}

/// The committed body is the task's rendering of the committed figures, byte for byte: the grids,
/// the intervals (which the rendering derives from the figures and the criteria), the defaults
/// (which it reads from `tables::kick_rank`) and every doc comment. The review itself takes the
/// better part of an hour, so it is not rerun here: `hyperion-fit check` holds the file to its
/// inputs hash, fingerprint and body hash.
#[test]
fn the_committed_body_is_the_rendering_of_its_constants() {
    let s = Sensitivity {
        committed: centre(table::CENTRE_ON_COMMITTED_TABLE),
        rebuild: centre(table::CENTRE_ON_REBUILD),
        sweeps: [
            sweep(
                KickDefault::LowRamp,
                table::LOW_RAMP_GRID.map(|(lower, _)| lower),
                &table::LOW_RAMP_OBSERVED,
                &table::LOW_RAMP_HELD,
            ),
            sweep(
                KickDefault::BhFactor,
                table::BH_FACTOR_GRID,
                &table::BH_FACTOR_OBSERVED,
                &table::BH_FACTOR_HELD,
            ),
            sweep(
                KickDefault::EcWindowSingle,
                table::EC_WINDOW_SINGLE_GRID,
                &table::EC_WINDOW_SINGLE_OBSERVED,
                &table::EC_WINDOW_SINGLE_HELD,
            ),
            sweep(
                KickDefault::EcWindowStripped,
                table::EC_WINDOW_STRIPPED_GRID,
                &table::EC_WINDOW_STRIPPED_OBSERVED,
                &table::EC_WINDOW_STRIPPED_HELD,
            ),
        ],
    };
    let rendered = RustTable {
        items: items(&s),
        ..RustTable::default()
    }
    .body()
    .unwrap();
    let committed = TableText::of_file(COMMITTED);
    common::assert_reproduced(
        "crates/hyperion-sim/src/tables/kick_defaults.rs",
        "kick_defaults",
        &committed.body,
        &rendered,
    );
    let header = committed.header().unwrap();
    assert_eq!(header.kind, Some(HeaderKind::Generated));
    assert_eq!(header.task.as_deref(), Some("kick_defaults"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/kick_defaults.toml")
    );
    // P15.T5.b's first acceptance criterion: at the committed defaults every observable passes.
    assert_eq!(s.committed.failing(), Vec::<Observable>::new());
}

#[test]
fn the_smoke_run_passes_the_header_grammar_and_is_the_same_on_any_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut texts = Vec::new();
    for threads in ["1", "4"] {
        let out = dir.path().join(format!("kick_defaults_{threads}.rs"));
        let cli = Cli::try_parse_from([
            "hyperion-fit",
            "run",
            "kick_defaults",
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
    assert_eq!(header.kind, Some(HeaderKind::Generated));
    assert_eq!(header.task.as_deref(), Some("kick_defaults"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/kick_defaults.smoke.toml")
    );
    assert!(
        header
            .acceptance
            .as_deref()
            .is_some_and(|a| a.contains("ruling 147.2") && a.contains("resolution")),
        "{header:?}"
    );
    let task = find("kick_defaults").unwrap();
    assert_eq!(common::declared(&split.body), task.items());
    assert_eq!(
        common::declared(&TableText::of_file(COMMITTED).body),
        task.items()
    );
}
