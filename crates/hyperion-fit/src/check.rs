//! The staleness check, `hyperion-fit check` (plan 15, P15.T2 and Design note 8).
//!
//! It reruns no fit. For every table in the lock file it fails, naming the table and the reason, if
//! the file is missing; its header is not in the grammar; its header's `inputs-sha256` differs
//! from the hash of the task's name, revision, manifest and datasets computed now; its body's
//! SHA-256 differs from the lock file's (a hand edit); its fingerprint's probes, recomputed, differ
//! from the lock file's by more than 10⁻⁹ relative; its `since-generator-version` exceeds
//! `GENERATOR_VERSION`; or the lock file and the header, or the lock file and the sim's
//! `tables::MANIFEST`, disagree. A registered task with no lock entry fails too. With
//! `--rerun-fast` it also reruns every fast task and compares the bytes it would write with the
//! committed file. Provisional tables are listed as warnings until P15.T12 makes them errors.

use std::fmt;
use std::fs;
use std::num::NonZeroUsize;

use hyperion_sim::tables::TableInfo;

use crate::data::{DatasetClass, Provenance};
use crate::emit::{HeaderKind, Placement, TableText, Workspace};
use crate::manifest::{InputsHash, LockEntry, LockFile, SimFingerprint};
use crate::pipeline::{ManifestKind, load_manifest, placement_among, rerender};
use crate::task::{FitTask, TaskClass};

/// What is wrong with one table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Problem {
    /// Its file, or its block of a shared file, is missing.
    Missing,
    /// Its header is not in the grammar; the text says how.
    Header(String),
    /// Its header's inputs hash is not the one computed now: its manifest, a dataset or the task's
    /// revision changed without a rerun.
    ChangedInputs,
    /// Its body's hash is not the lock file's: the table was edited by hand.
    HandEdited,
    /// A probe of its fingerprint, recomputed, is off the lock file's value.
    FingerprintDrift {
        /// The probe's name.
        probe: String,
    },
    /// Its `since-generator-version` is ahead of the sim's `GENERATOR_VERSION`.
    SinceAhead {
        /// The table's.
        since: u32,
        /// The sim's.
        current: u32,
    },
    /// The lock file and the table's header, or the lock file and `tables::MANIFEST`, disagree;
    /// the text says on what.
    Disagrees(String),
    /// A registered task has no lock entry, or a lock entry names a task that is not registered.
    NotRegistered(String),
    /// A fast task's rerun does not reproduce the committed file.
    RerunDiffers,
    /// The table could not be checked; the text says why.
    Unreadable(String),
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(f, "the table's file or block is missing"),
            Self::Header(why) => write!(f, "the header is malformed: {why}"),
            Self::ChangedInputs => write!(
                f,
                "the inputs hash differs from the one computed now (manifest, data or revision \
                 changed): rerun the task"
            ),
            Self::HandEdited => write!(f, "the body differs from tables.lock: a hand edit"),
            Self::FingerprintDrift { probe } => write!(
                f,
                "sim fingerprint probe `{probe}` has moved: the generator code it depends on \
                 changed, rerun the task"
            ),
            Self::SinceAhead { since, current } => write!(
                f,
                "since-generator-version {since} is ahead of GENERATOR_VERSION {current}"
            ),
            Self::Disagrees(what) | Self::NotRegistered(what) => write!(f, "{what}"),
            Self::RerunDiffers => write!(
                f,
                "a rerun of this fast task does not reproduce the committed file"
            ),
            Self::Unreadable(why) => write!(f, "cannot be checked: {why}"),
        }
    }
}

/// One table's finding.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Finding {
    /// The table.
    pub table: String,
    /// What is wrong.
    pub problem: Problem,
}

/// The check's result: every table's state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckReport {
    /// The tables that passed, by name, with a one-line note.
    pub fresh: Vec<(String, String)>,
    /// Warnings: provisional tables, and fast tasks not rerun because their data is not fetched.
    pub warnings: Vec<(String, String)>,
    /// Failures.
    pub findings: Vec<Finding>,
}

impl CheckReport {
    /// Whether nothing failed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.findings.is_empty()
    }

    fn fail(&mut self, table: &str, problem: Problem) {
        self.findings.push(Finding {
            table: table.to_owned(),
            problem,
        });
    }
}

impl fmt::Display for CheckReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (table, note) in &self.fresh {
            writeln!(f, "fresh    {table}: {note}")?;
        }
        for (table, note) in &self.warnings {
            writeln!(f, "warning  {table}: {note}")?;
        }
        for finding in &self.findings {
            writeln!(f, "STALE    {}: {}", finding.table, finding.problem)?;
        }
        write!(
            f,
            "{} fresh, {} warnings, {} failures",
            self.fresh.len(),
            self.warnings.len(),
            self.findings.len()
        )
    }
}

/// Whether `check` reruns the fast tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rerun {
    /// Rerun nothing: `just fit-check`.
    None,
    /// Rerun every fast task and compare bytes: `--rerun-fast`.
    Fast,
}

/// What the check is run against.
#[derive(Debug, Clone, Copy)]
pub struct CheckInputs<'a> {
    /// The paths.
    pub workspace: &'a Workspace,
    /// The registered tasks.
    pub tasks: &'a [&'static dyn FitTask],
    /// The sim's `tables::MANIFEST`.
    pub manifest: &'a [TableInfo],
    /// The sim's `GENERATOR_VERSION`.
    pub current: u32,
}

/// Runs the staleness check. A stale table is a [`Finding`] of the report, and so is a lock file
/// that cannot be read at all, on the table `tables.lock`.
#[must_use]
pub fn check(inputs: &CheckInputs<'_>, rerun: Rerun, threads: NonZeroUsize) -> CheckReport {
    let mut report = CheckReport::default();
    let lock = match LockFile::read(&inputs.workspace.lock_path) {
        Ok(lock) => lock,
        Err(e) => {
            report.fail("tables.lock", Problem::Unreadable(e.to_string()));
            return report;
        }
    };
    check_manifest_agrees(inputs.manifest, &lock, &mut report);
    for task in inputs.tasks {
        if lock.get(task.name()).is_none() {
            report.fail(
                task.name(),
                Problem::NotRegistered(format!(
                    "task `{}` has no entry in tables.lock: run `just fit {}`",
                    task.name(),
                    task.name()
                )),
            );
        }
    }
    for entry in lock.entries() {
        let task = entry
            .task
            .as_ref()
            .map(|name| inputs.tasks.iter().find(|t| t.name() == name));
        let task = match task {
            Some(Some(task)) => Some(*task),
            Some(None) => {
                report.fail(
                    &entry.name,
                    Problem::NotRegistered(format!(
                        "task `{}` is not registered",
                        entry.task.as_deref().unwrap_or_default()
                    )),
                );
                continue;
            }
            None => None,
        };
        let before = report.findings.len();
        check_entry(inputs, entry, task, &mut report);
        if report.findings.len() > before {
            continue;
        }
        if let Some(task) = task
            && rerun == Rerun::Fast
            && task.class() == TaskClass::Fast
            && !rerun_fast(inputs, entry, task, threads, &mut report)
        {
            continue;
        }
        match (&entry.provisional, &entry.acceptance) {
            (true, _) => report.warnings.push((
                entry.name.clone(),
                format!(
                    "provisional, since generator version {}",
                    entry.since_generator_version
                ),
            )),
            (false, acceptance) => report.fresh.push((
                entry.name.clone(),
                format!(
                    "revision {}, since generator version {}{}",
                    entry.revision,
                    entry.since_generator_version,
                    acceptance
                        .as_ref()
                        .map_or_else(String::new, |a| format!("; {a}"))
                ),
            )),
        }
    }
    report
}

/// Checks that `tables::MANIFEST` lists the lock file's tables, in order, with the same revision,
/// `since` and provisional flag.
fn check_manifest_agrees(manifest: &[TableInfo], lock: &LockFile, report: &mut CheckReport) {
    let listed: Vec<&str> = manifest.iter().map(|t| t.name).collect();
    let locked: Vec<&str> = lock.entries().iter().map(|e| e.name.as_str()).collect();
    if listed != locked {
        report.fail(
            "tables::MANIFEST",
            Problem::Disagrees(format!(
                "tables::MANIFEST lists {listed:?} but tables.lock holds {locked:?}"
            )),
        );
        return;
    }
    for (info, entry) in manifest.iter().zip(lock.entries()) {
        if info.revision != entry.revision
            || info.since_generator_version != entry.since_generator_version
            || info.provisional != entry.provisional
        {
            report.fail(
                &entry.name,
                Problem::Disagrees(format!(
                    "tables::MANIFEST has revision {}, since {}, provisional {}; tables.lock has \
                     {}, {}, {}",
                    info.revision,
                    info.since_generator_version,
                    info.provisional,
                    entry.revision,
                    entry.since_generator_version,
                    entry.provisional
                )),
            );
        }
    }
}

/// Checks one lock entry against its file, its task and the generator version.
fn check_entry(
    inputs: &CheckInputs<'_>,
    entry: &LockEntry,
    task: Option<&'static dyn FitTask>,
    report: &mut CheckReport,
) {
    let name = entry.name.as_str();
    if entry.since_generator_version > inputs.current {
        report.fail(
            name,
            Problem::SinceAhead {
                since: entry.since_generator_version,
                current: inputs.current,
            },
        );
    }
    let path = inputs.workspace.tables_dir.join(&entry.path);
    let Ok(text) = fs::read_to_string(&path) else {
        report.fail(name, Problem::Missing);
        return;
    };
    let block = task.map_or(Placement::File, |task| placement_among(inputs.tasks, task))
        == Placement::Block;
    let split = if block {
        match TableText::of_block(&text, name) {
            Ok(Some(split)) => split,
            Ok(None) => {
                report.fail(name, Problem::Missing);
                return;
            }
            Err(e) => {
                report.fail(name, Problem::Header(e.to_string()));
                return;
            }
        }
    } else {
        TableText::of_file(&text)
    };
    let header = match split.header() {
        Ok(header) => header,
        Err(e) => {
            report.fail(name, Problem::Header(e.to_string()));
            return;
        }
    };
    if split.body_sha256() != entry.body_sha256 {
        report.fail(name, Problem::HandEdited);
    }
    let disagree =
        |what: &str| Problem::Disagrees(format!("the header and tables.lock disagree on {what}"));
    if header.since_generator_version != Some(entry.since_generator_version) {
        report.fail(name, disagree("since-generator-version"));
    }
    if matches!(header.kind, Some(HeaderKind::Provisional { .. })) != entry.provisional {
        report.fail(name, disagree("whether the table is provisional"));
    }
    if header.inputs_sha256 != entry.inputs_sha256 {
        report.fail(name, disagree("inputs-sha256"));
    }
    if header.task != entry.task || header.revision.unwrap_or_default() != entry.revision {
        report.fail(name, disagree("the task and revision"));
    }
    let Some(task) = task else {
        return;
    };
    match load_manifest(inputs.workspace, task, ManifestKind::Full)
        .map_err(|e| e.to_string())
        .and_then(|manifest| {
            InputsHash::of_task(task.name(), task.revision(), &manifest, inputs.workspace)
                .map_err(|e| e.to_string())
        }) {
        Ok((hash, _)) => {
            if header.inputs_sha256.as_deref() != Some(hash.to_hex().as_str()) {
                report.fail(name, Problem::ChangedInputs);
            }
        }
        Err(why) => report.fail(name, Problem::Unreadable(why)),
    }
    let fingerprint = task.fingerprint();
    let recomputed = fingerprint.values();
    if let Some(i) = SimFingerprint::first_drift(&entry.fingerprint, &recomputed) {
        let probe = fingerprint
            .probes()
            .get(i)
            .map_or_else(|| format!("#{i}"), |(p, _)| p.clone());
        report.fail(name, Problem::FingerprintDrift { probe });
    }
    if header.sim_fingerprint != Some(SimFingerprint::hash_of(&entry.fingerprint)) {
        report.fail(name, disagree("sim-fingerprint"));
    }
}

/// Reruns fast `task` and compares what it would write with the committed file; false if it
/// failed. A task whose fetched dataset is absent is not rerun, with a warning.
fn rerun_fast(
    inputs: &CheckInputs<'_>,
    entry: &LockEntry,
    task: &'static dyn FitTask,
    threads: NonZeroUsize,
    report: &mut CheckReport,
) -> bool {
    let manifest = match load_manifest(inputs.workspace, task, ManifestKind::Full) {
        Ok(manifest) => manifest,
        Err(e) => {
            report.fail(&entry.name, Problem::Unreadable(e.to_string()));
            return false;
        }
    };
    for dataset in manifest.datasets() {
        let Ok(provenance) = Provenance::load(&inputs.workspace.data_dir, dataset) else {
            continue;
        };
        let dir = provenance.default_dir(&inputs.workspace.data_dir);
        if provenance.class == DatasetClass::Fetched && !dir.exists() {
            report.warnings.push((
                entry.name.clone(),
                format!(
                    "not rerun: dataset `{dataset}` is not fetched into {}",
                    dir.display()
                ),
            ));
            return true;
        }
    }
    let committed = fs::read_to_string(inputs.workspace.tables_dir.join(&entry.path));
    let rerun = rerender(
        inputs.tasks,
        task,
        inputs.workspace,
        threads,
        inputs.current,
    );
    match (rerun, committed) {
        (Ok(planned), Ok(committed)) if planned.text == committed => true,
        (Ok(_), Ok(_)) => {
            report.fail(&entry.name, Problem::RerunDiffers);
            false
        }
        (Err(e), _) => {
            report.fail(&entry.name, Problem::Unreadable(e.to_string()));
            false
        }
        (_, Err(e)) => {
            report.fail(&entry.name, Problem::Unreadable(e.to_string()));
            false
        }
    }
}
