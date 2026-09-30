//! From a task to its emitted table: the manifest's path, the header, and the emission (plan 15,
//! P15.T1 and P15.T2). The command line and the staleness check share it.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::emit::{
    DATA_HASH_PREFIX, Emission, EmitTableError, Header, HeaderKind, Placement, PlannedWrite,
    Workspace, plan_write,
};
use crate::manifest::{InputsHash, LoadManifestError, Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskOutput, check_manifest};
use crate::{RunFitError, task};

/// Which of a task's manifests to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ManifestKind {
    /// `<name>.toml`, the run whose table is committed.
    Full,
    /// `<name>.smoke.toml`, a reduced run of a few seconds for the tests.
    Smoke,
}

/// The path of `task`'s manifest of `kind` in `workspace`.
#[must_use]
pub fn manifest_path(workspace: &Workspace, task: &dyn FitTask, kind: ManifestKind) -> PathBuf {
    let file = match kind {
        ManifestKind::Full => format!("{}.toml", task.name()),
        ManifestKind::Smoke => format!("{}.smoke.toml", task.name()),
    };
    workspace.manifests_dir.join(file)
}

/// How `task`'s table sits in its file among the registered tasks: in a block if another writes
/// the same file.
#[must_use]
pub fn placement(task: &dyn FitTask) -> Placement {
    placement_among(task::registry(), task)
}

/// How `task`'s table sits in its file among `tasks`.
#[must_use]
pub fn placement_among(tasks: &[&'static dyn FitTask], task: &dyn FitTask) -> Placement {
    let shared = tasks
        .iter()
        .any(|other| other.name() != task.name() && other.table_path() == task.table_path());
    if shared {
        Placement::Block
    } else {
        Placement::File
    }
}

/// A task's run, ready to emit: its output, its header less `since`, and its fingerprint.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    /// The task's name.
    pub name: &'static str,
    /// Its table's file.
    pub table_path: &'static str,
    /// What the run produced.
    pub output: TaskOutput,
    /// The header, less its `since-generator-version`.
    pub header: Header,
    /// The fingerprint's probe values.
    pub fingerprint: SimFingerprint,
    /// Where the table sits in its file.
    pub placement: Placement,
}

impl Prepared {
    /// The emission of this run.
    #[must_use]
    pub fn emission(&self) -> Emission<'_> {
        Emission {
            name: self.name,
            table_path: self.table_path,
            placement: self.placement,
            table: &self.output.table,
            header: self.header.clone(),
            fingerprint: self.fingerprint.values(),
        }
    }
}

/// The header `task` writes for a table made from `manifest`, whose datasets' recorded hashes
/// are read from `workspace`.
///
/// # Errors
///
/// [`RunFitError::Task`] if a dataset's provenance cannot be read.
pub fn header_of(
    task: &dyn FitTask,
    manifest: &Manifest,
    workspace: &Workspace,
    output: &TaskOutput,
    fingerprint: &SimFingerprint,
) -> Result<Header, RunFitError> {
    let (inputs, data) = InputsHash::of_task(task.name(), task.revision(), manifest, workspace)
        .map_err(|e| RunFitError::Task(RunTaskError::Dataset(e)))?;
    let file = manifest
        .path()
        .file_name()
        .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
    Ok(Header {
        kind: Some(match output.provisional {
            Some(by) => HeaderKind::Provisional { by: by.to_owned() },
            None => HeaderKind::Generated,
        }),
        task: Some(task.name().to_owned()),
        revision: Some(task.revision()),
        tool_version: None,
        inputs_sha256: Some(inputs.to_hex()),
        manifest: Some(format!("crates/hyperion-fit/manifests/{file}")),
        data: Some(
            data.into_iter()
                .map(|(name, hash)| (name, hash[..DATA_HASH_PREFIX].to_owned()))
                .collect(),
        ),
        sim_fingerprint: Some(SimFingerprint::hash_of(&fingerprint.values())),
        since_generator_version: None,
        source: Some(output.source.clone()),
        acceptance: Some(output.acceptance.clone()),
    })
}

/// Runs `task` on `manifest` with `threads` threads and prepares its emission, its placement
/// judged among `tasks`.
///
/// # Errors
///
/// [`RunFitError::Task`] if the manifest is another task's or the run fails.
pub fn prepare(
    tasks: &[&'static dyn FitTask],
    task: &'static dyn FitTask,
    manifest: &Manifest,
    workspace: &Workspace,
    threads: NonZeroUsize,
) -> Result<Prepared, RunFitError> {
    check_manifest(task, manifest)?;
    let output = task.run(manifest, threads)?;
    let fingerprint = task.fingerprint();
    let header = header_of(task, manifest, workspace, &output, &fingerprint)?;
    Ok(Prepared {
        name: task.name(),
        table_path: task.table_path(),
        output,
        header,
        fingerprint,
        placement: placement_among(tasks, task),
    })
}

/// Loads `task`'s manifest of `kind` from `workspace`.
///
/// # Errors
///
/// [`RunFitError::Manifest`] if it cannot be loaded.
pub fn load_manifest(
    workspace: &Workspace,
    task: &dyn FitTask,
    kind: ManifestKind,
) -> Result<Manifest, RunFitError> {
    Manifest::load(&manifest_path(workspace, task, kind)).map_err(RunFitError::Manifest)
}

/// What `run <task> --since <current>` would write into `workspace`'s tables, `task` placed among
/// `tasks`, without writing it:
/// the reproduction a fast task's test and `check --rerun-fast` compare with the committed file.
///
/// # Errors
///
/// As [`prepare`] and [`plan_write`].
pub fn rerender(
    tasks: &[&'static dyn FitTask],
    task: &'static dyn FitTask,
    workspace: &Workspace,
    threads: NonZeroUsize,
    current: u32,
) -> Result<PlannedWrite, RunFitError> {
    let manifest = load_manifest(workspace, task, ManifestKind::Full)?;
    let prepared = prepare(tasks, task, &manifest, workspace, threads)?;
    plan_write(&prepared.emission(), workspace, Some(current), current).map_err(RunFitError::Emit)
}

impl From<RunTaskError> for RunFitError {
    fn from(error: RunTaskError) -> Self {
        Self::Task(error)
    }
}

impl From<EmitTableError> for RunFitError {
    fn from(error: EmitTableError) -> Self {
        Self::Emit(error)
    }
}

impl From<LoadManifestError> for RunFitError {
    fn from(error: LoadManifestError) -> Self {
        Self::Manifest(error)
    }
}
