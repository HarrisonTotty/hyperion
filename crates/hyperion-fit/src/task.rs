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
//!    `tables::MANIFEST`, and the `pub mod` line of the table in its crate's `tables/mod.rs`.
//!
//! A table lives in the `src/tables/` of the crate that reads it, which the task names
//! ([`FitTask::table_crate`]; the sim's unless it says otherwise), and that crate's
//! `tables::MANIFEST` lists it. `tables.lock` stays one file, whose entries name a table's file
//! alone, found in its task's crate.

use std::error::Error;
use std::num::NonZeroUsize;

use crate::data::LoadDatasetError;
use crate::emit::RustTable;
use crate::manifest::{LockEntry, Manifest, ManifestParamError, SimFingerprint};
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

/// The crate whose `src/tables/` holds a table, and whose `tables::MANIFEST` lists it.
///
/// A table is the sim's unless its task says otherwise. The crate split moves the sky's twelve,
/// with their `MANIFEST` rows, into `hyperion-sky` (CS.T2), and their tasks then name
/// [`TableCrate::Sky`]. The surface crate's tables, which its `cold_trap` reads for the sim and
/// which no export of the client's module reaches, name [`TableCrate::Surface`] (R09.T7.f).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum TableCrate {
    /// `hyperion-sim`, which also holds every table no registered task makes.
    #[default]
    Sim,
    /// `hyperion-sky`.
    Sky,
    /// `hyperion-surface`.
    Surface,
}

impl TableCrate {
    /// Every crate that holds tables, in the order the check reports them.
    pub const ALL: [Self; 3] = [Self::Sim, Self::Sky, Self::Surface];

    /// The crate's name in Rust paths: `hyperion_sim`, `hyperion_sky` or `hyperion_surface`.
    #[must_use]
    pub const fn rust_name(self) -> &'static str {
        match self {
            Self::Sim => "hyperion_sim",
            Self::Sky => "hyperion_sky",
            Self::Surface => "hyperion_surface",
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

    /// The crate whose `src/tables/` holds its table: the sim's unless the task says otherwise.
    fn table_crate(&self) -> TableCrate {
        TableCrate::Sim
    }

    /// Its table's file, relative to its crate's `src/tables/` ([`FitTask::table_crate`]).
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
pub static REGISTRY: [&dyn FitTask; 37] = [
    &tasks::binary_cooling_floor::BinaryCoolingFloorTask,
    &tasks::binary_pair_light::C_TASK,
    &tasks::binary_pair_light::D_TASK,
    &tasks::binary_pair_light::E_TASK,
    &tasks::binary_reach::BinaryReachTask,
    &tasks::bowl_permanent_shadow::BowlPermanentShadowTask,
    &tasks::chabrier::ChabrierTask,
    &tasks::cluster_bh::ClusterBhTask,
    &tasks::cluster_retention::ClusterRetentionTask,
    &tasks::displaced_forms::DisplacedFormsTask,
    &tasks::equipartition::EquipartitionTask,
    &tasks::giant_cooling::GiantCoolingTask,
    &tasks::kick_rank::KickRankTask,
    &tasks::lifetime_bracket::LifetimeBracketTask,
    &tasks::limb_darkening::LimbDarkeningTask,
    &tasks::liquid_optics::LiquidOpticsTask,
    &tasks::mge::MgeTask,
    &tasks::period_correction::PeriodCorrectionTask,
    &tasks::pulsars::PulsarsTask,
    &tasks::saturation_curves::SaturationCurvesTask,
    &tasks::sky_binary_light::C_TASK,
    &tasks::sky_binary_light::D_TASK,
    &tasks::sky_binary_light::E_TASK,
    &tasks::sky_envelope::SkyEnvelopeTask,
    &tasks::sky_phase_envelope::SkyPhaseEnvelopeTask,
    &tasks::star_colour::StarColourTask,
    &tasks::star_colour::REDDENING_TASK,
    &tasks::star_colour::REDDENING_AV02_05_TASK,
    &tasks::star_colour::REDDENING_AV07P5_TASK,
    &tasks::star_colour::REDDENING_AV10_15_TASK,
    &tasks::star_colour::REDDENING_AV20_30_TASK,
    &tasks::stellar_fates::HIGH_TASK,
    &tasks::stellar_fates::LOW_TASK,
    &tasks::stellar_fates::MID_TASK,
    &tasks::stripping::StrippingTask,
    &tasks::type_ia_delay::TypeIaDelayTask,
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

/// The crate of the table that lock entry `entry` names: its task's crate, or the sim's for an
/// entry that no task of `tasks` makes.
#[must_use]
pub fn table_crate_of(tasks: &[&'static dyn FitTask], entry: &LockEntry) -> TableCrate {
    entry
        .task
        .as_deref()
        .and_then(|name| tasks.iter().find(|task| task.name() == name))
        .map_or(TableCrate::Sim, |task| task.table_crate())
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
                "binary_cooling_floor",
                "binary_pair_light_c",
                "binary_pair_light_d",
                "binary_pair_light_e",
                "binary_reach",
                "bowl_permanent_shadow",
                "chabrier",
                "cluster_bh",
                "cluster_retention",
                "displaced_forms",
                "equipartition",
                "giant_cooling",
                "kick_rank",
                "lifetime_bracket",
                "limb_darkening",
                "liquid_optics",
                "mge",
                "period_correction",
                "pulsars",
                "saturation_curves",
                "sky_binary_light_c",
                "sky_binary_light_d",
                "sky_binary_light_e",
                "sky_envelope",
                "sky_phase_envelope",
                "star_colour",
                "star_colour_reddening",
                "star_colour_reddening_av02_05",
                "star_colour_reddening_av07p5",
                "star_colour_reddening_av10_15",
                "star_colour_reddening_av20_30",
                "stellar_fates_high",
                "stellar_fates_low",
                "stellar_fates_mid",
                "stripping",
                "type_ia_delay",
                "wd_cooling"
            ]
        );
        assert_eq!(find("mge").map(FitTask::table_path), Some("mge.rs"));
        assert!(find("kicks").is_none());
    }

    /// Each task names the crate its table lives in: the sky's for the tasks listed here, which
    /// are none until the crate split's move (CS.T2) gives it the sky's twelve, the surface
    /// crate's for the bowl's permanent shadow, and the sim's for every other.
    #[test]
    fn each_task_names_its_tables_crate() {
        let sky: [&str; 0] = [];
        let surface = ["bowl_permanent_shadow"];
        for task in registry() {
            let expected = if sky.contains(&task.name()) {
                TableCrate::Sky
            } else if surface.contains(&task.name()) {
                TableCrate::Surface
            } else {
                TableCrate::Sim
            };
            assert_eq!(task.table_crate(), expected, "{}", task.name());
        }
    }
}
