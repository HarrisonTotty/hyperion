//! The shared cluster-dynamics file's three blocks are what their fits write (plan 15, P15.T8:
//! T8.a `cluster_bh`, T8.b `equipartition`, T8.c `pulsars`).
//!
//! `hyperion-fit check` reruns no fit: it holds each block to its inputs hash, its body hash and
//! its fingerprint's probes, so it cannot see the fit code drift away from the committed bytes.
//! Here the two fast fits rerun on their committed manifests and must give the whole file byte
//! for byte (Verification: fast tasks are tested on their real manifest). The slow fit,
//! `equipartition`, which `--rerun-fast` skips, does the same in the slow tier, and its smoke
//! manifest runs end to end through the command line on one thread and on four, into a temporary
//! directory, checking the header grammar and the declared items (Design notes 2 and 11). The task
//! reads no thread count, so Design note 2 holds for it trivially: the pair checks that two runs
//! agree, and it becomes the thread-split check if the task ever splits its work.
//!
//! Every block is provisional and holds plan 09's scratch constants while its fit misses its
//! acceptance; one test pins that without running a fit. A ruling that adopts fitted values
//! updates that test in its own lane.
//!
//! `toolchain.rs` already checks every committed table's declared items and runs `fit-check`
//! without reruns, so neither is repeated here for the committed file.

use std::fs;
use std::num::NonZeroUsize;

use clap::Parser as _;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, RustTable, TableText, Workspace};
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::{cluster_bh, equipartition, pulsars};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_testkit::float::assert_same_bits;

/// The file the sim compiles, as committed: the three blocks under the module's own docs.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/cluster_dynamics.rs");

/// Reruns task `name` on its committed manifest, on one thread, and compares the whole shared
/// file it would write, its own block replaced, with the committed file.
///
/// On a mismatch the panic names the first differing line, committed against rerendered.
fn assert_block_reproduced(name: &str) {
    let planned = rerender(
        registry(),
        find(name).expect("the task is registered"),
        &Workspace::repository(),
        NonZeroUsize::MIN,
        GENERATOR_VERSION.get(),
    )
    .expect("the task runs on its committed manifest");
    if planned.text != COMMITTED {
        let at = COMMITTED
            .lines()
            .zip(planned.text.lines())
            .enumerate()
            .find(|(_, (committed, rendered))| committed != rendered)
            .map_or_else(
                || "the length".to_owned(),
                |(n, (committed, rendered))| {
                    format!(
                        "line {}: committed `{committed}`, rerendered `{rendered}`",
                        n + 1
                    )
                },
            );
        panic!(
            "crates/hyperion-sim/src/tables/cluster_dynamics.rs differs from the {name} fit at \
             {at}: run `just fit {name}`"
        );
    }
    assert!(!planned.body_changed, "{name}: the body would change");
}

/// The black holes' loss (P15.T8.a, fast) reruns on its committed manifest, the CMC models in the
/// Milky Way the task builds at the manifest's seed, and gives the committed file byte for byte.
#[test]
fn cluster_bh_block_is_reproduced() {
    assert_block_reproduced("cluster_bh");
}

/// The pulsars' encounter-rate law (P15.T8.c, fast) reruns on its committed manifest and gives
/// the committed file byte for byte.
#[test]
fn pulsars_block_is_reproduced() {
    assert_block_reproduced("pulsars");
}

/// The mass-segregation exponent (P15.T8.b, slow) reruns on its committed manifest, the full grid
/// of 40 multimass King models, and gives the committed file byte for byte.
#[test]
#[ignore = "slow: the full equipartition fit, 40 multimass King models, about half a minute in a release build"]
fn equipartition_block_is_reproduced() {
    assert_block_reproduced("equipartition");
}

/// Each block is its task's rendering of plan 09's scratch constants, provisional by its own
/// subtask and written by its own task: today's state, while every fit misses its acceptance.
///
/// The scratch constants are pinned bit for bit at the values P15.T8 quotes from plan 09 (plan
/// 15, "P15.T8 Cluster dynamics constants"): β 2.8 × 10⁻³, ψ₁ 147, the clock factor 2.5 (ruling
/// 126.4) and Spitzer's (1987) prefactor 0.138; η = 1, δ = ½ mass segregation (ruling 139.2); 40
/// pulsars at 47 Tucanae's Γ, the exponent 0.7, and a cap of 40, the count at 47 Tucanae's Γ.
#[test]
fn the_committed_blocks_hold_plan_nines_scratch_values() {
    for (scratch, plan_nines) in cluster_bh::SCRATCH
        .into_iter()
        .zip([2.8e-3, 147.0, 2.5, 0.138])
        .chain([(equipartition::SCRATCH_ETA, 1.0)])
        .chain(pulsars::SCRATCH.into_iter().zip([40.0, 0.7, 40.0]))
    {
        assert_same_bits(scratch, plan_nines);
    }

    let blocks: [(&str, RustTable, &str); 3] = [
        (
            "cluster_bh",
            cluster_bh::render(cluster_bh::SCRATCH),
            "P15.T8.a",
        ),
        (
            "equipartition",
            equipartition::render(equipartition::SCRATCH_ETA),
            "P15.T8.b",
        ),
        ("pulsars", pulsars::render(pulsars::SCRATCH), "P15.T8.c"),
    ];
    for (name, table, by) in blocks {
        let block = TableText::of_block(COMMITTED, name)
            .expect("the shared file's blocks parse")
            .unwrap_or_else(|| panic!("the shared file holds a {name} block"));
        assert_eq!(table.body().unwrap(), block.body, "{name}");
        let header = block.header().unwrap();
        assert_eq!(
            header.kind,
            Some(HeaderKind::Provisional { by: by.to_owned() }),
            "{name}"
        );
        assert_eq!(header.task.as_deref(), Some(name));
    }
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

/// The equipartition smoke run goes end to end through the command line on one thread and on
/// four, into a temporary directory, and writes the same block both times, in the header grammar,
/// declaring the task's items (Design notes 2 and 11).
///
/// The task ignores `--threads`, so the two runs prove that the fit is deterministic and that the
/// command-line path works, not that a thread split leaves the bytes alone. Two models cannot pass
/// the acceptance, so the block is provisional by P15.T8.b.
#[test]
fn the_equipartition_smoke_run_passes_the_header_grammar_and_is_the_same_on_any_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut texts = Vec::new();
    for threads in ["1", "4"] {
        let out = dir.path().join(format!("equipartition_{threads}.rs"));
        let cli = Cli::try_parse_from([
            "hyperion-fit",
            "run",
            "equipartition",
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
        "equipartition --smoke: --threads 1 and --threads 4 wrote different blocks"
    );
    // `--out` writes the task's block alone, between its markers.
    let block = TableText::of_block(&texts[0], "equipartition")
        .expect("the smoke run's block parses")
        .expect("the smoke run writes the equipartition block");
    let header = block.header().unwrap();
    assert_eq!(header.task.as_deref(), Some("equipartition"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/equipartition.smoke.toml")
    );
    assert!(
        header
            .acceptance
            .as_deref()
            .is_some_and(|a| a.contains("multimass King models") && a.contains("W₀ [4.0, 8.0]")),
        "{header:?}"
    );
    assert_eq!(
        header.kind,
        Some(HeaderKind::Provisional {
            by: "P15.T8.b".to_owned()
        })
    );
    assert_eq!(
        declared(&block.body),
        find("equipartition").unwrap().items()
    );
}
