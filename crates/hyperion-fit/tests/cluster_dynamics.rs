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
//! Every block is production at plan 09's values, adopted by ruling (decision-p15-t8-blocks):
//! each task, at revision 1, emits those values `@generated` once its gates pass, with its fit's
//! own figures reported in the acceptance line (P15.T8.d). One test pins that without running a
//! fit.
//!
//! `toolchain.rs` already checks every committed table's declared items and runs `fit-check`
//! without reruns, so neither is repeated here for the committed file.

mod common;

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
        let at = common::first_difference(COMMITTED, &planned.text).map_or_else(
            || "the length".to_owned(),
            |(n, committed, rendered)| {
                format!("line {n}: committed `{committed}`, rerendered `{rendered}`")
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

/// Each block is its task's rendering of the adopted values, `@generated` by its own task at
/// revision 1: production at plan 09's values, adopted by ruling (decision-p15-t8-blocks,
/// P15.T8.d).
///
/// The adopted values are pinned bit for bit at the values P15.T8 quotes from plan 09 (plan 15,
/// "P15.T8 Cluster dynamics constants"), so that a moved constant cannot pass with its own
/// re-emission: β 2.8 × 10⁻³ and ψ₁ 147 (Antonini and Gieles 2020), the clock factor 2.5 (ruling
/// 126.4) and Spitzer's (1987) prefactor 0.138; η = 1, δ = ½ mass segregation (ruling 139.2); 40
/// pulsars at 47 Tucanae's Γ, the exponent 0.7, and a cap of 40, the count at 47 Tucanae's Γ.
#[test]
fn the_committed_blocks_are_generated_at_the_adopted_values() {
    for (scratch, plan_nines) in cluster_bh::SCRATCH
        .into_iter()
        .zip([2.8e-3, 147.0, 2.5, 0.138])
        .chain([(equipartition::SCRATCH_ETA, 1.0)])
        .chain(pulsars::SCRATCH.into_iter().zip([40.0, 0.7, 40.0]))
    {
        assert_same_bits(scratch, plan_nines);
    }

    let blocks: [(&str, RustTable); 3] = [
        ("cluster_bh", cluster_bh::render(cluster_bh::SCRATCH)),
        (
            "equipartition",
            equipartition::render(equipartition::SCRATCH_ETA),
        ),
        ("pulsars", pulsars::render(pulsars::SCRATCH)),
    ];
    for (name, table) in blocks {
        let block = TableText::of_block(COMMITTED, name)
            .expect("the shared file's blocks parse")
            .unwrap_or_else(|| panic!("the shared file holds a {name} block"));
        assert_eq!(table.body().unwrap(), block.body, "{name}");
        let header = block.header().unwrap();
        assert_eq!(header.kind, Some(HeaderKind::Generated), "{name}");
        assert_eq!(header.task.as_deref(), Some(name));
        assert_eq!(header.revision, Some(1), "{name}");
    }
}

/// The equipartition smoke run goes end to end through the command line on one thread and on
/// four, into a temporary directory, and writes the same block both times, in the header grammar,
/// declaring the task's items (Design notes 2 and 11).
///
/// The task ignores `--threads`, so the two runs prove that the fit is deterministic and that the
/// command-line path works, not that a thread split leaves the bytes alone. Revision 1's gates, the
/// adopted η's window and the models' convergence, hold on any grid that converges, so the two
/// models' block is `@generated`, as the full run's is.
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
    assert_eq!(header.kind, Some(HeaderKind::Generated));
    assert_eq!(
        common::declared(&block.body),
        find("equipartition").unwrap().items()
    );
}
