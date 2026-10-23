//! Globular-cluster streams per globular cluster (plan 15, P15.T11): the multiplier k by which the
//! sim's [`galaxy::global_list::orphans`](hyperion_sim::galaxy::global_list::orphans) draws
//! Poisson(max(0, k × expected globulars − living globulars with a tube)) orphan streams (plan 10,
//! Design note 10).
//!
//! **P15.T11.a, the move.** Plan 10's P10.T3.b, the multiplier's first reader, committed
//! [`tables::streams`](hyperion_sim::tables::streams) by hand at the brainstorm's 1.5, with no fit
//! header, lock entry or `MANIFEST` row. Revision 0 registers it at the same value, which changes
//! no output (plan 15, Design note 9): it emits the scratch value its manifest records,
//! provisionally. **P15.T11.b** replaces the run with the fit against the known streams, observed
//! through plan 10's spray and tubes (P10.T5–T6, which it waits on), bumps the revision, and bumps
//! the generator version if the value moves. The census holds over 120 streams, wide and thin
//! together (Bonaca and Price-Whelan 2025, New Astron. Rev. 100, 101713, §2.2), of which the fit
//! counts the globular-like ones.

use std::num::NonZeroUsize;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision: 0, plan 10's scratch value registered unchanged.
pub const VERSION: u32 = 0;

/// The manifest's one parameter: the scratch multiplier, streams per globular.
const SCRATCH_PARAM: &str = "scratch_streams_per_globular";

/// The table's contents for the multiplier `streams_per_globular`.
#[must_use]
pub fn render(streams_per_globular: f64) -> RustTable {
    RustTable {
        summary: vec![
            "The streams' multiplier: how many globular-cluster streams a galaxy has per globular (plan 15,"
                .to_owned(),
            "P15.T11; read by plan 10, Design note 10).".to_owned(),
        ],
        notes: vec![
            "Provisional (P15.T11.a): the brainstorm's value, which plan 10's P10.T3.b entered here by"
                .to_owned(),
            "hand as its first reader, before plan 15's P15.T11 (as plan 10's Consumes allows), and which"
                .to_owned(),
            "P15.T11.a registered with the fitting toolchain at the same value. P15.T11.b fits it against"
                .to_owned(),
            "the thin, globular-like streams of the known census: over 120 streams, wide and thin"
                .to_owned(),
            "together, and far from complete (Bonaca and Price-Whelan 2025, New Astron. Rev. 100, 101713,"
                .to_owned(),
            "§2.2; Mateu 2023, MNRAS 520, 5225, lists 95). Plan 15 holds the value uncertain threefold."
                .to_owned(),
            "It is a parameter of the generator version whatever the fit says.".to_owned(),
        ],
        items: vec![TableItem::Scalar {
            name: "ORPHAN_STREAMS_PER_GLOBULAR".to_owned(),
            doc: vec![
                "Globular-cluster streams per globular cluster of the untruncated system, living clusters'"
                    .to_owned(),
                "tubes and orphans together: 1.5 (brainstorm, \"Streams and accreted structure\": \"globular"
                    .to_owned(),
                "streams number about 1.5 per globular, most of them orphans whose cluster is gone\")."
                    .to_owned(),
            ],
            value: streams_per_globular,
        }],
    }
}

/// The task: P15.T11.a's registration of plan 10's scratch multiplier, fast and provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct StreamsTask;

impl FitTask for StreamsTask {
    fn name(&self) -> &'static str {
        "streams"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "streams.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &["ORPHAN_STREAMS_PER_GLOBULAR"]
    }

    /// None yet: the scratch value reads no generator code. P15.T11.b's fit will depend on plan
    /// 10's streams and plan 02's potential, and gains their probes.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(Vec::new())
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let streams_per_globular = manifest.f64(SCRATCH_PARAM)?;
        if !(streams_per_globular.is_finite() && streams_per_globular > 0.0) {
            return Err(ManifestParamError::new(SCRATCH_PARAM, "positive and finite").into());
        }
        Ok(TaskOutput {
            table: render(streams_per_globular),
            source:
                "the brainstorm's value, entered by plan 10's P10.T3.b (\"Streams and accreted \
                     structure\", `docs/agent/brainstorming/galaxy-generation.md`: \"globular \
                     streams number about 1.5 per globular, most of them orphans whose cluster is \
                     gone\")"
                    .to_owned(),
            acceptance: "scratch: the brainstorm's 1.5, unfitted until P15.T11.b".to_owned(),
            provisional: Some("P15.T11.a"),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// A manifest of `text`.
    fn manifest(text: &str) -> Manifest {
        Manifest::from_bytes(Path::new("streams.toml"), text.as_bytes().to_vec())
            .expect("the test's manifest parses")
    }

    /// The parameter error of a run on `text`.
    fn param_error(text: &str) -> ManifestParamError {
        match StreamsTask.run(&manifest(text), NonZeroUsize::MIN) {
            Err(RunTaskError::Param(e)) => e,
            other => panic!("expected a parameter error, got {other:?}"),
        }
    }

    #[test]
    fn the_table_is_the_manifests_multiplier() {
        let table = render(1.5);
        assert_eq!(table.item_names(), ["ORPHAN_STREAMS_PER_GLOBULAR"]);
        let body = table.body().unwrap();
        assert!(
            body.ends_with("gone\").\npub const ORPHAN_STREAMS_PER_GLOBULAR: f64 = 1.5;\n"),
            "{body}"
        );
    }

    /// The run emits the manifest's scratch value as a provisional table, and reads nothing of
    /// the generator.
    #[test]
    fn the_run_emits_the_scratch_value_provisionally() {
        let output = StreamsTask
            .run(
                &manifest("task = \"streams\"\n[params]\nscratch_streams_per_globular = 1.5\n"),
                NonZeroUsize::MIN,
            )
            .unwrap();
        assert_eq!(output.table, render(1.5));
        assert_eq!(output.provisional, Some("P15.T11.a"));
        assert!(StreamsTask.fingerprint().is_empty());
    }

    #[test]
    fn the_run_refuses_a_manifest_without_the_multiplier() {
        assert_eq!(
            param_error("task = \"streams\"\n[params]\nscratch_scale = 1.5\n"),
            ManifestParamError::new(SCRATCH_PARAM, "a number")
        );
    }

    #[test]
    fn the_run_refuses_a_multiplier_that_is_not_positive_and_finite() {
        for value in ["0.0", "-1.5", "inf", "nan"] {
            assert_eq!(
                param_error(&format!(
                    "task = \"streams\"\n[params]\nscratch_streams_per_globular = {value}\n"
                )),
                ManifestParamError::new(SCRATCH_PARAM, "positive and finite"),
                "{value}"
            );
        }
    }

    #[test]
    fn the_run_refuses_another_tasks_manifest() {
        let result = StreamsTask.run(
            &manifest("task = \"chabrier\"\n[params]\nscratch_streams_per_globular = 1.5\n"),
            NonZeroUsize::MIN,
        );
        assert!(
            matches!(
                &result,
                Err(RunTaskError::WrongManifest { task: "streams", found }) if found == "chabrier"
            ),
            "{result:?}"
        );
    }
}
