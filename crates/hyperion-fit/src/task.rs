//! The fitting tasks' interface and their registry (plan 15, P15.T1).
//!
//! Each fit is a [`FitTask`]: a name, a class (fast enough for CI to rerun, or slow), the table it
//! writes, a revision bumped by hand whenever its algorithm changes, the sim probe values it
//! depends on, and a `run` that turns its manifest into a [`TaskOutput`]. The command line, the
//! emitter and the staleness check find every task through [`registry`].
//!
//! # Adding a task
//!
//! A later plan task that adds a fit (plan 06's `stellar_fates`, which the `briefs` lane adds, for
//! one) needs to touch nothing of
//! the toolchain:
//!
//! 1. a module `tasks/<name>.rs` with a unit struct implementing [`FitTask`], whose `run` reads
//!    its parameters from the manifest and returns the table as a [`RustTable`] of
//!    [`TableItem`](crate::emit::TableItem)s;
//! 2. its `pub mod` line in `tasks/mod.rs` and one entry in [`REGISTRY`], which is kept in name
//!    order;
//! 3. `manifests/<name>.toml`, and `manifests/<name>.smoke.toml` if the task is slow;
//! 4. `just fit <name>`, which writes the table, its `tables.lock` entry and its row of
//!    `tables::MANIFEST`, and the `pub mod` line of the table in the sim's `tables/mod.rs`.

use std::error::Error;
use std::num::NonZeroUsize;

use crate::data::LoadDatasetError;
use crate::emit::RustTable;
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::BuildThreadPoolError;
use crate::tasks;

/// Whether CI may rerun a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaskClass {
    /// Under a minute: `hyperion-fit check --rerun-fast` reruns it and compares bytes.
    Fast,
    /// Longer: CI relies on its inputs hash, its fingerprint and its smoke run.
    Slow,
}

impl TaskClass {
    /// `fast` or `slow`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Slow => "slow",
        }
    }
}

/// What a task's run produces: the table, and what its header says of it.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskOutput {
    /// The table's summary, notes and items.
    pub table: RustTable,
    /// The citations the fit was made against, for the header's `source`.
    pub source: String,
    /// The measured figures of the acceptance test, for the header's `acceptance` and the lock
    /// file.
    pub acceptance: String,
    /// `Some(plan task)` while the table is a scratch stand-in: the header says `@provisional`
    /// and names it.
    pub provisional: Option<&'static str>,
}

/// One offline fit.
pub trait FitTask: Sync + std::fmt::Debug {
    /// The task's name, which is also its table's name in the lock file and `MANIFEST`: `mge`,
    /// `kick_rank`, …
    fn name(&self) -> &'static str;

    /// Whether CI may rerun it.
    fn class(&self) -> TaskClass;

    /// Its table's file, relative to `crates/hyperion-sim/src/tables/`.
    fn table_path(&self) -> &'static str;

    /// Its revision, bumped by hand when the algorithm changes: the inputs hash covers it and not
    /// the source code.
    fn revision(&self) -> u32;

    /// The names of the items its table declares, in order, for the smoke run's check.
    fn items(&self) -> &'static [&'static str];

    /// The sim probe values its table depends on; empty for a task that uses only `math`.
    fn fingerprint(&self) -> SimFingerprint;

    /// Runs the fit on `manifest` with `threads` threads.
    ///
    /// # Errors
    ///
    /// [`RunTaskError`] if the manifest is not this task's or lacks a parameter, a dataset cannot
    /// be read, or the fit cannot run.
    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError>;
}

/// Every task, in name order. A new task is one more entry (see the module's documentation).
pub static REGISTRY: [&dyn FitTask; 5] = [
    &tasks::chabrier::ChabrierTask,
    &tasks::giant_cooling::GiantCoolingTask,
    &tasks::kick_rank::KickRankTask,
    &tasks::mge::MgeTask,
    &tasks::wd_cooling::WdCoolingTask,
];

/// Every task, in name order.
#[must_use]
pub fn registry() -> &'static [&'static dyn FitTask] {
    &REGISTRY
}

/// The task named `name`, if there is one.
#[must_use]
pub fn find(name: &str) -> Option<&'static dyn FitTask> {
    registry().iter().copied().find(|task| task.name() == name)
}

/// Checks that `manifest` is `task`'s.
///
/// # Errors
///
/// [`RunTaskError::WrongManifest`] if it names another task.
pub fn check_manifest(task: &dyn FitTask, manifest: &Manifest) -> Result<(), RunTaskError> {
    if manifest.task() == task.name() {
        Ok(())
    } else {
        Err(RunTaskError::WrongManifest {
            task: task.name(),
            found: manifest.task().to_owned(),
        })
    }
}

/// A task could not run.
#[derive(Debug, thiserror::Error)]
pub enum RunTaskError {
    /// The manifest is another task's.
    #[error("the manifest is for task `{found}`, not `{task}`")]
    WrongManifest {
        /// The task run.
        task: &'static str,
        /// The task the manifest names.
        found: String,
    },
    /// A parameter of the manifest is missing or wrong.
    #[error(transparent)]
    Param(#[from] ManifestParamError),
    /// A dataset could not be read or failed its hash.
    #[error(transparent)]
    Dataset(#[from] LoadDatasetError),
    /// The task's input could not be read once its files were loaded.
    #[error("task `{task}` cannot read its input")]
    Input {
        /// The task.
        task: &'static str,
        /// Why.
        source: Box<dyn Error + Send + Sync>,
    },
    /// The thread pool could not be built.
    #[error(transparent)]
    Threads(#[from] BuildThreadPoolError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_is_in_name_order_and_names_are_unique() {
        let names: Vec<&str> = registry().iter().map(|t| t.name()).collect();
        assert!(names.windows(2).all(|w| w[0] < w[1]), "{names:?}");
        assert_eq!(
            names,
            [
                "chabrier",
                "giant_cooling",
                "kick_rank",
                "mge",
                "wd_cooling"
            ]
        );
        assert_eq!(find("mge").map(FitTask::table_path), Some("mge.rs"));
        assert!(find("kicks").is_none());
    }
}
