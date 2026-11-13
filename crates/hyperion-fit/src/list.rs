//! The tables' listing, `hyperion-fit list` (plan 15, P15.T1 and P15.T12).
//!
//! One row for each registered task's table: its name, its revision and acceptance figures from
//! its lock entry, its task's class and the run time the class bounds, its state as the staleness
//! check finds it without reruns, and its file. Then one row for each lock entry that no registered
//! task makes, and one for each table file whose own header neither a registered task nor a lock
//! entry writes: the hand-made `binary` (`tables/binary.rs`, whose blocks are two tasks' but whose
//! header is plan 11's scratch, until P11.T15 registers it), so that plan 15's close sees every
//! table that is not yet a fit's.

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use crate::check::{CheckInputs, CheckReport, Problem, Rerun, check};
use crate::emit::{Placement, TableText};
use crate::manifest::{LockFile, ReadLockError};
use crate::pipeline::placement_among;
use crate::task::{TableCrate, TaskClass, table_crate_of};

/// A table's state in the listing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ListState {
    /// The check passes it.
    Fresh,
    /// The check fails it for being provisional, and for nothing else.
    Provisional,
    /// The check fails it for another reason.
    Stale,
    /// Neither a registered task nor a lock entry writes its header, which declares the kind
    /// held: the word after its `@`, `provisional` for the hand-made `binary`.
    Unregistered(String),
}

impl fmt::Display for ListState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fresh => write!(f, "fresh"),
            Self::Provisional => write!(f, "provisional"),
            Self::Stale => write!(f, "stale"),
            Self::Unregistered(kind) => write!(f, "unregistered {kind}"),
        }
    }
}

/// One table's row of the listing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ListRow {
    /// The table's name: its task's, its lock entry's, or its file's stem for an unregistered
    /// table.
    pub name: String,
    /// Its revision: its lock entry's, or its header's for an unregistered table, if either has
    /// one.
    pub revision: Option<u32>,
    /// Its task's class, if a registered task makes it.
    pub class: Option<TaskClass>,
    /// Its state.
    pub state: ListState,
    /// Its file, relative to the `tables/` of its crate.
    pub path: String,
    /// The acceptance test's measured figures: its lock entry's, or its header's for an
    /// unregistered table, if either holds them.
    pub acceptance: Option<String>,
}

/// The listing's columns, in order.
const COLUMNS: [&str; 7] = [
    "name",
    "revision",
    "class",
    "runtime",
    "state",
    "table",
    "acceptance",
];

impl ListRow {
    /// The row's cells under [`COLUMNS`], `-` for what it lacks.
    #[must_use]
    fn cells(&self) -> [String; 7] {
        let or_dash = |cell: Option<String>| cell.unwrap_or_else(|| "-".to_owned());
        [
            self.name.clone(),
            or_dash(self.revision.map(|revision| revision.to_string())),
            or_dash(self.class.map(|class| class.as_str().to_owned())),
            or_dash(self.class.and_then(TaskClass::runtime).map(str::to_owned)),
            self.state.to_string(),
            self.path.clone(),
            or_dash(self.acceptance.clone()),
        ]
    }
}

/// The listing: the registered tasks' tables in the registry's order, then the lock entries that
/// no registered task makes, then the unregistered tables by crate and file name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Listing {
    /// The rows, in order.
    pub rows: Vec<ListRow>,
}

impl fmt::Display for Listing {
    /// A heading and one line per row, in columns two spaces apart; the last column, the
    /// acceptance figures, runs on unpadded.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines: Vec<[String; 7]> = std::iter::once(COLUMNS.map(str::to_owned))
            .chain(self.rows.iter().map(ListRow::cells))
            .collect();
        let mut widths = [0; 7];
        for cells in &lines {
            for (width, cell) in widths.iter_mut().zip(cells) {
                *width = (*width).max(cell.chars().count());
            }
        }
        for (i, cells) in lines.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            let [padded @ .., last] = cells;
            for (cell, width) in padded.iter().zip(widths) {
                write!(f, "{cell:<width$}  ")?;
            }
            write!(f, "{last}")?;
        }
        Ok(())
    }
}

/// The tables could not be listed.
#[derive(Debug, thiserror::Error)]
pub enum ListTablesError {
    /// The lock file could not be read.
    #[error(transparent)]
    Lock(#[from] ReadLockError),
    /// A crate's `tables/` directory, or a file in it, could not be read.
    #[error("cannot read {}", path.display())]
    Io {
        /// The directory or file.
        path: PathBuf,
        /// Why.
        #[source]
        source: io::Error,
    },
}

/// Lists every table of `inputs`' workspace ([`Listing`]), its states from a staleness check
/// without reruns.
///
/// # Errors
///
/// [`ListTablesError::Lock`] if the lock file cannot be read, and [`ListTablesError::Io`] if a
/// crate's `tables/` directory exists but it or a table file in it cannot be read.
pub fn listing(inputs: &CheckInputs<'_>) -> Result<Listing, ListTablesError> {
    let lock = LockFile::read(&inputs.workspace.lock_path)?;
    let report = check(inputs, Rerun::None, NonZeroUsize::MIN);
    let mut rows: Vec<ListRow> = inputs
        .tasks
        .iter()
        .map(|task| {
            let entry = lock.get(task.name());
            ListRow {
                name: task.name().to_owned(),
                revision: entry.map(|entry| entry.revision),
                class: Some(task.class()),
                state: state_of(&report, task.name()),
                path: task.table_path().to_owned(),
                acceptance: entry.and_then(|entry| entry.acceptance.clone()),
            }
        })
        .collect();
    rows.extend(
        lock.entries()
            .iter()
            .filter(|entry| !inputs.tasks.iter().any(|task| task.name() == entry.name))
            .map(|entry| ListRow {
                name: entry.name.clone(),
                revision: Some(entry.revision),
                class: None,
                state: state_of(&report, &entry.name),
                path: entry.path.clone(),
                acceptance: entry.acceptance.clone(),
            }),
    );
    rows.extend(unregistered(inputs, &lock)?);
    Ok(Listing { rows })
}

/// The state the check's `report` gives table `name`.
#[must_use]
fn state_of(report: &CheckReport, name: &str) -> ListState {
    let mut state = ListState::Fresh;
    for finding in report.findings.iter().filter(|f| f.table == name) {
        match finding.problem {
            Problem::Provisional { .. } => state = ListState::Provisional,
            Problem::Missing
            | Problem::Header(_)
            | Problem::ChangedInputs
            | Problem::HandEdited
            | Problem::FingerprintDrift { .. }
            | Problem::SinceAhead { .. }
            | Problem::Disagrees(_)
            | Problem::NotRegistered(_)
            | Problem::RerunDiffers
            | Problem::Unreadable(_) => return ListState::Stale,
        }
    }
    state
}

/// The rows of the table files, crate by crate in [`TableCrate::ALL`]'s order and by name within
/// a crate, whose leading comment holds a header that neither a registered task nor a lock entry
/// writes.
fn unregistered(
    inputs: &CheckInputs<'_>,
    lock: &LockFile,
) -> Result<Vec<ListRow>, ListTablesError> {
    let mut rows = Vec::new();
    for table_crate in TableCrate::ALL {
        let dir = inputs.workspace.tables_dir(table_crate);
        let unreadable = |path: &Path| {
            let path = path.to_owned();
            move |source| ListTablesError::Io { path, source }
        };
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            // A crate that holds no table yet has no `tables/`: the sky's, until the crate
            // split's move (CS.T2).
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(unreadable(dir)(e)),
        };
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(unreadable(dir))?;
            // A table's file is a Rust module: a UTF-8 name whose extension is `rs`, which rustc
            // matches case-sensitively.
            if entry.path().extension() == Some(OsStr::new("rs"))
                && let Some(file) = entry.file_name().to_str()
            {
                files.push(file.to_owned());
            }
        }
        files.sort();
        for file in files {
            if header_is_registered(inputs, lock, table_crate, &file) {
                continue;
            }
            let path = dir.join(&file);
            let text = fs::read_to_string(&path).map_err(unreadable(&path))?;
            let split = TableText::of_file(&text);
            let Some(first) = split.header_lines.first() else {
                continue;
            };
            let kind = first
                .strip_prefix('@')
                .unwrap_or(first)
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .next()
                .unwrap_or_default();
            // A hand-made header need not be in the grammar (the `binary` one is not): its kind
            // still shows, and its revision and figures where it parses.
            let header = split.header().ok();
            rows.push(ListRow {
                name: file.strip_suffix(".rs").unwrap_or(&file).to_owned(),
                revision: header.as_ref().and_then(|header| header.revision),
                class: None,
                state: ListState::Unregistered(kind.to_owned()),
                acceptance: header.and_then(|header| header.acceptance),
                path: file,
            });
        }
    }
    Ok(rows)
}

/// Whether `file` of `table_crate`'s tables opens with a registered table's header: a registered
/// task's that writes the whole file, or a lock entry's that no registered task makes.
#[must_use]
fn header_is_registered(
    inputs: &CheckInputs<'_>,
    lock: &LockFile,
    table_crate: TableCrate,
    file: &str,
) -> bool {
    inputs.tasks.iter().any(|task| {
        task.table_crate() == table_crate
            && task.table_path() == file
            && placement_among(inputs.tasks, *task) == Placement::File
    }) || lock.entries().iter().any(|entry| {
        entry.path == file
            && table_crate_of(inputs.tasks, entry) == table_crate
            && !inputs.tasks.iter().any(|task| task.name() == entry.name)
    })
}
