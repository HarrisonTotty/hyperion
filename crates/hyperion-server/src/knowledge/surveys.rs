//! Survey passes and the coverage they give (plan R09, R09.T18; Design note 16).
//!
//! A [`SurveyPass`] records that the ship measured part of a body's surface: over what span of
//! the universe clock, by what means ([`SurveySource`]), how finely (a resolution in metres, which
//! codes as a [`ResolutionCode`]), and where: half-open runs of [`cell_index`] values at the
//! level of the body's coarse field, which [`coarse_level`] gives for its radius. A body's passes,
//! folded together, are its [`Coverage`]: a [`ResolutionCode`] byte per cell of the field, the
//! finest any pass reached there, 384 KiB at level 8. Coverage gates which of the field's cells
//! the server sends (any pass) and what a readout may quote (the code), R09.T19 and plan R10.
//!
//! The passes are Knowledge and are kept as the contacts are
//! ([`PersistedKnowledge`](super::PersistedKnowledge)): in
//! `<data_dir>/universes/<id>/knowledge/surveys.v1.jsonl`, a `{"format":1}` header and then one
//! pass a line, each appended and synced before it is applied, every file operation on
//! [`tokio::task::spawn_blocking`]. A header of another format, or a log of a later format beside
//! this one (`surveys.v2.jsonl` and on), is [`LoadKnowledgeError::UnsupportedFormat`]; a torn last
//! line is dropped with a warning and written over by the next pass; any other line that does not
//! parse or does not fit the passes before it is [`LoadKnowledgeError::MalformedLine`]. Coverage
//! is folded on load and never stored: the log is the record.
//!
//! [`SurveyLog::record`] is the only way coverage grows. Until sensors exist, the `survey_pass`
//! request will call it (R09.T19.a), a stand-in for an honest client as plan 12's client-set
//! observer is; the sensors plan replaces the caller, not the core. Nothing holds a universe's
//! Knowledge yet (P12.T8 will), so the surface service will open each universe's log on its first
//! `survey_pass` or `surface_field` and keep it (R09.T19.a).
//!
//! [`cell_index`]: hyperion_surface::field::cell_index
//! [`coarse_level`]: hyperion_surface::field::coarse_level

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use hyperion_sim::id::BodyId;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Metres;
use hyperion_surface::field::{CoarseLevel, ResolutionCode};
use serde::{Deserialize, Serialize};

use super::jsonl::{self, KnowledgeLog, LogName, TimeLine};
use super::{LoadKnowledgeError, SaveKnowledgeError};
use crate::universe::{UniverseId, UniverseStore};

/// The survey log's format: the one this build writes and the only one it reads.
const SURVEYS_FORMAT: u32 = 1;

/// The survey log, `surveys.v1.jsonl` for [`SURVEYS_FORMAT`], a name R09 reserves with the
/// generator version.
const SURVEYS: LogName = LogName::new("surveys", SURVEYS_FORMAT);

/// How a pass measured the surface (Design note 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurveySource {
    /// From orbit.
    Orbital,
    /// From close range, below orbit.
    CloseRange,
    /// From a craft landed on the surface.
    Landed,
}

/// What a [`SurveyPass`] is made of, checked by [`SurveyPass::new`].
#[derive(Debug, Clone, PartialEq)]
pub struct SurveyPassParts {
    /// The body surveyed.
    pub body: BodyId,
    /// When the pass began.
    pub from: UniverseTime,
    /// When it ended: no earlier than `from`.
    pub to: UniverseTime,
    /// How it measured.
    pub source: SurveySource,
    /// How finely it measured, metres: finite, positive, and no coarser than the coarsest
    /// [`ResolutionCode`]'s, about 36,000 km.
    pub resolution: Metres,
    /// The level of the body's coarse field, which the cells index:
    /// [`coarse_level`](hyperion_surface::field::coarse_level) of the body's radius.
    pub level: CoarseLevel,
    /// The cells it covered: half-open runs of
    /// [`cell_index`](hyperion_surface::field::cell_index) values at `level`, at least one, each
    /// non-empty and below the level's cell count, in order and not overlapping (runs may meet).
    pub cells: Vec<Range<u32>>,
}

/// One survey pass: a body's cells measured at one resolution over a span of time (Design note
/// 16).
///
/// It is valid by construction ([`SurveyPass::new`]).
#[derive(Debug, Clone, PartialEq)]
pub struct SurveyPass {
    body: BodyId,
    from: UniverseTime,
    to: UniverseTime,
    source: SurveySource,
    resolution: Metres,
    /// The resolution's code, which the coverage keeps.
    code: ResolutionCode,
    level: CoarseLevel,
    cells: Box<[Range<u32>]>,
}

impl SurveyPass {
    /// The pass `parts` describes.
    ///
    /// # Errors
    ///
    /// [`BuildSurveyPassError`]: the pass ends before it begins, its resolution has no code, it
    /// covers no cells, or a run of its cells is empty, out of order, overlaps the one before it,
    /// or reaches past the field's last cell.
    pub fn new(parts: SurveyPassParts) -> Result<Self, BuildSurveyPassError> {
        let SurveyPassParts {
            body,
            from,
            to,
            source,
            resolution,
            level,
            cells,
        } = parts;
        if to < from {
            return Err(BuildSurveyPassError::Span { from, to });
        }
        let code = ResolutionCode::from_resolution(resolution).map_err(|_| {
            BuildSurveyPassError::Resolution {
                resolution_m: resolution.value(),
            }
        })?;
        if cells.is_empty() {
            return Err(BuildSurveyPassError::NoCells);
        }
        let count = level.cell_count();
        let mut previous_end = 0;
        for (run, range) in cells.iter().enumerate() {
            if range.end <= range.start {
                return Err(BuildSurveyPassError::EmptyRun {
                    start: range.start,
                    end: range.end,
                });
            }
            if range.start < previous_end {
                return Err(BuildSurveyPassError::Unsorted { run });
            }
            if range.end > count {
                return Err(BuildSurveyPassError::OutsideField {
                    end: range.end,
                    cells: count,
                });
            }
            previous_end = range.end;
        }
        Ok(Self {
            body,
            from,
            to,
            source,
            resolution,
            code,
            level,
            cells: cells.into_boxed_slice(),
        })
    }

    /// The body surveyed.
    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    /// When the pass began.
    #[must_use]
    pub const fn from(&self) -> UniverseTime {
        self.from
    }

    /// When it ended, no earlier than [`from`](Self::from).
    #[must_use]
    pub const fn to(&self) -> UniverseTime {
        self.to
    }

    /// How it measured.
    #[must_use]
    pub const fn source(&self) -> SurveySource {
        self.source
    }

    /// How finely it measured, metres.
    #[must_use]
    pub const fn resolution(&self) -> Metres {
        self.resolution
    }

    /// The code of its resolution, max(1, ⌈8 log₂(r ÷ 1 cm)⌉ + 1), never
    /// [`ResolutionCode::NONE`].
    #[must_use]
    pub const fn code(&self) -> ResolutionCode {
        self.code
    }

    /// The level of the body's coarse field, which its cells index.
    #[must_use]
    pub const fn level(&self) -> CoarseLevel {
        self.level
    }

    /// The cells it covered, as half-open runs of cell indices in order.
    #[must_use]
    pub fn cells(&self) -> &[Range<u32>] {
        &self.cells
    }

    /// How many cells it covered.
    #[must_use]
    pub fn cell_count(&self) -> u32 {
        // The runs are disjoint and inside the field, whose 6 · 4ᴸ cells fit a `u32`.
        self.cells.iter().map(|run| run.end - run.start).sum()
    }
}

/// Why a [`SurveyPass`] could not be made.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildSurveyPassError {
    /// The pass ends before it begins.
    Span {
        /// When it began.
        from: UniverseTime,
        /// When it ended.
        to: UniverseTime,
    },
    /// The resolution is not finite and positive, or is coarser than the coarsest code's.
    Resolution {
        /// The resolution, metres.
        resolution_m: f64,
    },
    /// The pass covers no cells.
    NoCells,
    /// A run of cells is empty: its end is not above its start.
    EmptyRun {
        /// The run's first index.
        start: u32,
        /// The index past its last.
        end: u32,
    },
    /// A run begins before the one before it ends.
    Unsorted {
        /// The run's place in the list, from 0.
        run: usize,
    },
    /// A run reaches past the field's last cell.
    OutsideField {
        /// The index past the run's last.
        end: u32,
        /// The field's cell count at the pass's level.
        cells: u32,
    },
}

impl fmt::Display for BuildSurveyPassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Span { from, to } => {
                write!(f, "survey pass ends at {to}, before it begins at {from}")
            }
            Self::Resolution { resolution_m } => {
                write!(
                    f,
                    "survey resolution {resolution_m} m has no resolution code"
                )
            }
            Self::NoCells => f.write_str("survey pass covers no cells"),
            Self::EmptyRun { start, end } => write!(f, "cell run {start}..{end} is empty"),
            Self::Unsorted { run } => {
                write!(f, "cell run {run} begins before the one before it ends")
            }
            Self::OutsideField { end, cells } => {
                write!(
                    f,
                    "cell run ending at {end} reaches past the field's {cells} cells"
                )
            }
        }
    }
}

impl Error for BuildSurveyPassError {}

/// A body's coverage: at each cell of its coarse field, the finest resolution any of its passes
/// reached there, as a [`ResolutionCode`] byte (Design note 16).
///
/// It is folded from the passes, the finer of two codes winning and a survey beating none, so it
/// does not depend on the order the passes were made or recorded in. It is never stored.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Coverage {
    level: CoarseLevel,
    revision: CoverageRevision,
    /// One code a cell, in cell-index order.
    codes: Box<[u8]>,
}

/// A body's coverage revision: how many of its passes are folded in, from 1 after its first.
///
/// Revision n is always the fold of the body's first n passes in the log's order, since the log
/// folds each pass under the file's lock as it appends it; R09.T19.b sends it with the field and
/// takes it back as `have_revision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoverageRevision(NonZeroU32);

impl CoverageRevision {
    /// The revision of a body's first pass.
    pub const FIRST: Self = Self(NonZeroU32::MIN);

    /// Revision `number`, or `None` for 0, which no coverage has.
    #[must_use]
    pub const fn new(number: u32) -> Option<Self> {
        match NonZeroU32::new(number) {
            Some(number) => Some(Self(number)),
            None => None,
        }
    }

    /// The number, from 1.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// The revision after this one, or `None` past 2³² − 1.
    #[must_use]
    const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(number) => Some(Self(number)),
            None => None,
        }
    }
}

impl fmt::Display for CoverageRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The codes are 6,144 to 393,216 bytes: their count of surveyed cells stands for them.
        f.debug_struct("Coverage")
            .field("level", &self.level)
            .field("revision", &self.revision)
            .field("surveyed", &self.surveyed())
            .finish_non_exhaustive()
    }
}

impl Coverage {
    /// The coverage of a body's first pass, `pass`.
    #[must_use]
    fn first(pass: &SurveyPass) -> Self {
        let mut coverage = Self {
            level: pass.level,
            revision: CoverageRevision::FIRST,
            codes: vec![0; to_usize(pass.level.cell_count())].into_boxed_slice(),
        };
        coverage.fold_codes(pass);
        coverage
    }

    /// The level of the body's coarse field.
    #[must_use]
    pub const fn level(&self) -> CoarseLevel {
        self.level
    }

    /// The body's revision: how many passes are folded in.
    #[must_use]
    pub const fn revision(&self) -> CoverageRevision {
        self.revision
    }

    /// The code of the cell whose cell index is `index`: [`ResolutionCode::NONE`] if no pass
    /// covered it, and `None` past the field's last cell.
    #[must_use]
    pub fn code(&self, index: u32) -> Option<ResolutionCode> {
        self.codes
            .get(to_usize(index))
            .map(|&code| ResolutionCode::new(code))
    }

    /// The codes as bytes, one a cell in cell-index order, 6 · 4ᴸ of them.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.codes
    }

    /// How many cells any pass covered.
    #[must_use]
    pub fn surveyed(&self) -> u32 {
        // At most 6 · 4⁸ cells, so the count cannot overflow.
        self.codes.iter().map(|&code| u32::from(code != 0)).sum()
    }

    /// The revision `pass` would fold in at: refused unless it is at this field's level and a
    /// revision is left to count it.
    fn next_revision(&self, pass: &SurveyPass) -> Result<CoverageRevision, FitPassError> {
        if pass.level != self.level {
            return Err(FitPassError::Level {
                body: pass.body,
                recorded: self.level,
                pass: pass.level,
            });
        }
        self.revision
            .next()
            .ok_or(FitPassError::Revisions { body: pass.body })
    }

    /// Folds `pass` in at revision `revision`, which [`next_revision`](Self::next_revision) gave.
    fn fold(&mut self, pass: &SurveyPass, revision: CoverageRevision) {
        self.fold_codes(pass);
        self.revision = revision;
    }

    /// Keeps the finer of each cell's code and `pass`'s over the pass's cells.
    ///
    /// # Panics
    ///
    /// If the pass is at another level, which the callers have refused.
    fn fold_codes(&mut self, pass: &SurveyPass) {
        assert_eq!(pass.level, self.level, "a pass folds at its field's level");
        for run in &pass.cells {
            for code in &mut self.codes[to_usize(run.start)..to_usize(run.end)] {
                *code = ResolutionCode::new(*code).finer(pass.code).get();
            }
        }
    }
}

/// A cell index as a `usize`, which the server's 32- and 64-bit targets hold.
#[must_use]
fn to_usize(index: u32) -> usize {
    usize::try_from(index).expect("a u32 fits in usize on the server's targets")
}

/// Why a pass does not fit the coverage it would fold into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FitPassError {
    Level {
        body: BodyId,
        recorded: CoarseLevel,
        pass: CoarseLevel,
    },
    Revisions {
        body: BodyId,
    },
}

/// Every body's coverage, as the log has it.
#[derive(Debug, Default)]
struct Surveys {
    /// Shared with readers' snapshots: a fold copies a coverage only while a reader holds it.
    bodies: BTreeMap<BodyId, Arc<Coverage>>,
}

impl Surveys {
    /// The revision `pass` would fold in at: refused if it does not fit its body's coverage.
    fn next_revision(&self, pass: &SurveyPass) -> Result<CoverageRevision, FitPassError> {
        self.bodies
            .get(&pass.body)
            .map_or(Ok(CoverageRevision::FIRST), |coverage| {
                coverage.next_revision(pass)
            })
    }

    /// Folds `pass` into its body's coverage and returns the body's new revision.
    fn apply(&mut self, pass: &SurveyPass) -> Result<CoverageRevision, FitPassError> {
        match self.bodies.entry(pass.body) {
            Entry::Vacant(vacant) => Ok(vacant.insert(Arc::new(Coverage::first(pass))).revision),
            Entry::Occupied(mut occupied) => {
                let revision = occupied.get().next_revision(pass)?;
                Arc::make_mut(occupied.get_mut()).fold(pass, revision);
                Ok(revision)
            }
        }
    }
}

/// A universe's survey log, kept in step with its file (plan R09, R09.T18).
///
/// Passes are recorded through [`record`](Self::record), which appends to the file on a blocking
/// task and then folds the pass in; [`coverage`](Self::coverage) gives a body's coverage as it
/// stands. Cloning shares the log.
#[derive(Debug, Clone)]
pub struct SurveyLog {
    shared: Arc<Shared>,
}

/// The coverage and its file. The file's lock is taken first and held over a whole pass, so that
/// passes reach the file in the order they reach the coverage; the coverage's is held only to
/// check and to fold one, so that readers wait for no disk.
#[derive(Debug)]
struct Shared {
    log: Mutex<KnowledgeLog>,
    surveys: Mutex<Surveys>,
}

impl SurveyLog {
    /// Loads universe `universe`'s survey passes from `store`'s data directory and folds them into
    /// each body's coverage, none if it has no file yet.
    ///
    /// Nothing is created until the first pass. A universe's file has one writer: the caller keeps
    /// one `SurveyLog` per universe and shares it by cloning, since two opened from one file would
    /// each cut the other's lines off at their own length (the surface service, R09.T19.a, is to
    /// hold them until P12.T8's holder exists).
    ///
    /// # Errors
    ///
    /// [`LoadKnowledgeError`]: the directory or the file could not be read, the file is of
    /// another format or a later one lies beside it, a line other than a torn last one is
    /// malformed or does not fit the passes before it, or the blocking task did not finish.
    pub async fn open(
        store: &UniverseStore,
        universe: UniverseId,
    ) -> Result<Self, LoadKnowledgeError> {
        let dir = store.knowledge_dir(universe);
        let (surveys, log) = tokio::task::spawn_blocking(move || load(dir))
            .await
            .map_err(|error| {
                tracing::error!(%error, "a survey log load's blocking task did not finish");
                LoadKnowledgeError::Interrupted
            })??;
        Ok(Self {
            shared: Arc::new(Shared {
                log: Mutex::new(log),
                surveys: Mutex::new(surveys),
            }),
        })
    }

    /// Records `pass`, appending it to the file first, and returns its body's new revision.
    ///
    /// The revision is [`CoverageRevision::FIRST`] for the body's first pass, and one more for
    /// each pass after.
    ///
    /// # Errors
    ///
    /// [`RecordSurveyError::Level`] if the body's earlier passes index another level, and
    /// [`RecordSurveyError::Revisions`] if it has 2³² − 1 passes already; nothing is written.
    /// [`RecordSurveyError::Save`] if the append failed or its blocking task did not finish: the
    /// coverage is then unchanged, and the file may hold the pass if the failure came after its
    /// write (a failed sync), which the next append cuts off again. Once an earlier pass panicked
    /// part-way it is [`SaveKnowledgeError::Poisoned`], and the log must be opened again.
    pub async fn record(&self, pass: SurveyPass) -> Result<CoverageRevision, RecordSurveyError> {
        let shared = Arc::clone(&self.shared);
        tokio::task::spawn_blocking(move || shared.record(&pass))
            .await
            .map_err(|error| {
                tracing::error!(%error, "a survey log append's blocking task did not finish");
                RecordSurveyError::Save(SaveKnowledgeError::Interrupted)
            })?
    }

    /// Body `body`'s coverage as it stands, or `None` if no pass has covered it.
    ///
    /// The coverage is a snapshot, shared until the next pass of the body folds into a copy, so
    /// its codes and its revision always agree.
    #[must_use]
    pub fn coverage(&self, body: BodyId) -> Option<Arc<Coverage>> {
        lock(&self.shared.surveys).bodies.get(&body).map(Arc::clone)
    }
}

impl Shared {
    fn record(&self, pass: &SurveyPass) -> Result<CoverageRevision, RecordSurveyError> {
        // Refused once a pass panicked while holding it: the pass may be on disk and not in
        // memory, and the revisions handed out would then fall behind the file's.
        let mut log = self
            .log
            .lock()
            .map_err(|_| RecordSurveyError::Save(SaveKnowledgeError::Poisoned))?;
        let planned = lock(&self.surveys).next_revision(pass)?;
        log.append(&PassLine::from(pass))
            .map_err(RecordSurveyError::Save)?;
        let revision = lock(&self.surveys)
            .apply(pass)
            .expect("a pass that fitted under the file's lock still fits");
        debug_assert_eq!(revision, planned, "only this lock's holder folds passes");
        Ok(revision)
    }
}

/// Takes the coverage's lock whether or not a holder panicked: the coverage is changed only by
/// one fold at a time, which leaves it whole, so readers may go on.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reads the passes in `dir` and folds them (module documentation).
fn load(dir: PathBuf) -> Result<(Surveys, KnowledgeLog), LoadKnowledgeError> {
    let mut surveys = Surveys::default();
    let log = jsonl::load(dir, SURVEYS, |line: PassLine| {
        let pass = SurveyPass::try_from(line)?;
        surveys
            .apply(&pass)
            .map_err(|_| "does not fit the passes before it")?;
        Ok(())
    })?;
    Ok((surveys, log))
}

/// A pass could not be recorded, and the coverage is unchanged.
#[derive(Debug)]
pub enum RecordSurveyError {
    /// The pass indexes another level than the body's earlier passes.
    ///
    /// A body's level is fixed within a universe, whose generator version is, so only a caller's
    /// bug makes this.
    Level {
        /// The body.
        body: BodyId,
        /// The level of its earlier passes.
        recorded: CoarseLevel,
        /// The level of this one.
        pass: CoarseLevel,
    },
    /// The body has 2³² − 1 passes, the most a revision counts.
    Revisions {
        /// The body.
        body: BodyId,
    },
    /// The pass could not be saved.
    Save(SaveKnowledgeError),
}

impl From<FitPassError> for RecordSurveyError {
    fn from(error: FitPassError) -> Self {
        match error {
            FitPassError::Level {
                body,
                recorded,
                pass,
            } => Self::Level {
                body,
                recorded,
                pass,
            },
            FitPassError::Revisions { body } => Self::Revisions { body },
        }
    }
}

impl fmt::Display for RecordSurveyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Level {
                body,
                recorded,
                pass,
            } => write!(
                f,
                "survey pass of {body} is at level {}, its earlier passes at level {}",
                pass.get(),
                recorded.get()
            ),
            Self::Revisions { body } => {
                write!(f, "{body} has as many survey passes as revisions count")
            }
            Self::Save(_) => f.write_str("failed to save a survey pass"),
        }
    }
}

impl Error for RecordSurveyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Save(source) => Some(source),
            Self::Level { .. } | Self::Revisions { .. } => None,
        }
    }
}

/// One pass, as a line of format 1. Field order is the order on disk.
#[derive(Debug, Serialize, Deserialize)]
struct PassLine {
    /// The body in the sim's text form.
    body: String,
    from: TimeLine,
    to: TimeLine,
    source: SourceLine,
    resolution_m: f64,
    level: u8,
    /// Half-open runs of cell indices, `[start, end]`.
    cells: Vec<[u32; 2]>,
}

/// A [`SurveySource`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SourceLine {
    Orbital,
    CloseRange,
    Landed,
}

impl From<&SurveyPass> for PassLine {
    fn from(pass: &SurveyPass) -> Self {
        Self {
            body: pass.body.to_string(),
            from: TimeLine::from(pass.from),
            to: TimeLine::from(pass.to),
            source: SourceLine::from(pass.source),
            resolution_m: pass.resolution.value(),
            level: pass.level.get(),
            cells: pass.cells.iter().map(|run| [run.start, run.end]).collect(),
        }
    }
}

impl TryFrom<PassLine> for SurveyPass {
    /// The field that does not hold a valid value.
    type Error = &'static str;

    fn try_from(line: PassLine) -> Result<Self, Self::Error> {
        let parts = SurveyPassParts {
            // The field's name is the whole diagnosis: the text was written by this build's
            // `Display` and only damage can make it fail to parse.
            body: line.body.parse().map_err(|_| "body")?,
            from: line.from.to_time().ok_or("from")?,
            to: line.to.to_time().ok_or("to")?,
            source: SurveySource::from(line.source),
            resolution: Metres::new(line.resolution_m),
            level: CoarseLevel::new(line.level).ok_or("level")?,
            cells: line.cells.iter().map(|&[start, end]| start..end).collect(),
        };
        Self::new(parts).map_err(|error| match error {
            BuildSurveyPassError::Span { .. } => "to",
            BuildSurveyPassError::Resolution { .. } => "resolution_m",
            BuildSurveyPassError::NoCells
            | BuildSurveyPassError::EmptyRun { .. }
            | BuildSurveyPassError::Unsorted { .. }
            | BuildSurveyPassError::OutsideField { .. } => "cells",
        })
    }
}

impl From<SurveySource> for SourceLine {
    fn from(source: SurveySource) -> Self {
        match source {
            SurveySource::Orbital => Self::Orbital,
            SurveySource::CloseRange => Self::CloseRange,
            SurveySource::Landed => Self::Landed,
        }
    }
}

impl From<SourceLine> for SurveySource {
    fn from(source: SourceLine) -> Self {
        match source {
            SourceLine::Orbital => Self::Orbital,
            SourceLine::CloseRange => Self::CloseRange,
            SourceLine::Landed => Self::Landed,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::iter::once;

    use hyperion_sim::GeneratorVersion;
    use hyperion_surface::cube::{Face, PatchKey};
    use hyperion_surface::field::{cell_index, coarse_level};

    use super::*;
    use crate::knowledge::testing::{host, other_host};
    use crate::universe::SavedUniverse;

    const UNIVERSE: UniverseId = UniverseId::new(0xab);

    /// The survey log's name for [`SURVEYS_FORMAT`].
    const SURVEYS_FILE: &str = "surveys.v1.jsonl";

    /// A data directory holding the saves of universes [`UNIVERSE`] and `UNIVERSE + 1`.
    fn universe() -> (tempfile::TempDir, UniverseStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = UniverseStore::new(dir.path());
        for (id, name) in [
            (UNIVERSE, "Kepler Reach"),
            (other_universe(), "Kepler Drift"),
        ] {
            store
                .write(&SavedUniverse::new(
                    id,
                    name.parse().unwrap(),
                    0x4d2,
                    GeneratorVersion::new(7),
                ))
                .unwrap();
        }
        (dir, store)
    }

    const fn other_universe() -> UniverseId {
        UniverseId::new(0xac)
    }

    fn surveys_file(store: &UniverseStore) -> PathBuf {
        store.knowledge_dir(UNIVERSE).join(SURVEYS_FILE)
    }

    /// An Earth-sized planet, whose field is at level 8.
    fn earth() -> BodyId {
        BodyId::new(host(), 0x0003)
    }

    /// A Moon-sized moon, whose field is at level 6.
    fn moon() -> BodyId {
        BodyId::new(other_host(), 0x0103)
    }

    fn earth_level() -> CoarseLevel {
        let level = coarse_level(Metres::new(6_371_000.0));
        assert_eq!(level.get(), 8);
        level
    }

    fn moon_level() -> CoarseLevel {
        let level = coarse_level(Metres::new(1_737_400.0));
        assert_eq!(level.get(), 6);
        level
    }

    /// The cells at `level` under the patch `(face, depth, i, j)`: one run of cell indices, as
    /// every quadtree subtree is.
    fn region(face: Face, depth: u8, i: u32, j: u32, level: CoarseLevel) -> Range<u32> {
        let shift = level.get() - depth;
        let first = PatchKey::new(face, level.get(), i << shift, j << shift).unwrap();
        let last = PatchKey::new(
            face,
            level.get(),
            ((i + 1) << shift) - 1,
            ((j + 1) << shift) - 1,
        )
        .unwrap();
        let run = cell_index(first)..cell_index(last) + 1;
        assert_eq!(run.len(), 1 << (2 * shift), "a subtree is one run");
        run
    }

    /// A pass of `body` at `level` in year `year`.
    fn pass(
        body: BodyId,
        level: CoarseLevel,
        year: i64,
        source: SurveySource,
        resolution_m: f64,
        cells: impl IntoIterator<Item = Range<u32>>,
    ) -> SurveyPass {
        let from = UniverseTime::from_julian_years(year).unwrap();
        SurveyPass::new(SurveyPassParts {
            body,
            from,
            // About one low orbit, ending part-way through a second.
            to: UniverseTime::new(from.seconds() + 5_400, 250_000_000).unwrap(),
            source,
            resolution: Metres::new(resolution_m),
            level,
            cells: cells.into_iter().collect(),
        })
        .unwrap()
    }

    /// Four passes of the Earth that overlap, at four resolutions, and one of the Moon.
    fn passes() -> Vec<SurveyPass> {
        let earth_level = earth_level();
        let wide = region(Face::PosZ, 4, 3, 7, earth_level);
        let inner = region(Face::PosZ, 5, 6, 14, earth_level);
        let edge = region(Face::PosX, 6, 0, 63, earth_level);
        let half = wide.start..wide.start + len(&wide) / 2;
        vec![
            pass(
                earth(),
                earth_level,
                0,
                SurveySource::Orbital,
                2_500.0,
                // Face-major order: face 0's cells come before face 2's.
                [edge.clone(), wide.clone()],
            ),
            pass(
                earth(),
                earth_level,
                1,
                SurveySource::CloseRange,
                2.0,
                once(inner.clone()),
            ),
            pass(
                earth(),
                earth_level,
                2,
                SurveySource::Orbital,
                40_000.0,
                [edge, half],
            ),
            pass(
                earth(),
                earth_level,
                3,
                SurveySource::Landed,
                0.004,
                once(inner.start..inner.start + 3),
            ),
            pass(
                moon(),
                moon_level(),
                1,
                SurveySource::Orbital,
                120.0,
                [0..96, 1_000..1_001],
            ),
        ]
    }

    /// The length of a run of cells.
    fn len(run: &Range<u32>) -> u32 {
        run.end - run.start
    }

    /// Records `passes` in order and returns the revisions handed out.
    async fn record_all(log: &SurveyLog, passes: &[SurveyPass]) -> Vec<u32> {
        let mut revisions = Vec::new();
        for pass in passes {
            revisions.push(log.record(pass.clone()).await.unwrap().get());
        }
        revisions
    }

    /// The coverage of each of the passes' bodies, as bytes and revision.
    fn coverages(log: &SurveyLog) -> Vec<(BodyId, u32, Vec<u8>)> {
        [earth(), moon()]
            .into_iter()
            .filter_map(|body| {
                log.coverage(body).map(|coverage| {
                    (
                        body,
                        coverage.revision().get(),
                        coverage.as_bytes().to_vec(),
                    )
                })
            })
            .collect()
    }

    /// R09.T18: what is written is what is read back, every pass to the bit, under a header of
    /// format 1.
    #[tokio::test]
    async fn survey_passes_round_trip_through_their_file() {
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        assert_eq!(log.coverage(earth()), None);
        assert!(
            !store.knowledge_dir(UNIVERSE).exists(),
            "nothing is created before the first pass"
        );
        let passes = passes();
        assert_eq!(record_all(&log, &passes).await, [1, 2, 3, 4, 1]);
        drop(log);

        let text = fs::read_to_string(surveys_file(&store)).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 1 + passes.len());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(lines[0]).unwrap(),
            serde_json::json!({ "format": 1 })
        );
        let read: Vec<SurveyPass> = lines[1..]
            .iter()
            .map(|line| {
                SurveyPass::try_from(serde_json::from_str::<PassLine>(line).unwrap()).unwrap()
            })
            .collect();
        assert_eq!(read, passes);
        let second: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(second["body"], earth().to_string());
        assert_eq!(second["source"], "close_range");
        assert_eq!(second["resolution_m"], 2.0);
        assert_eq!(second["level"], 8);
        let inner = passes[1].cells()[0].clone();
        assert_eq!(
            second["cells"],
            serde_json::json!([[inner.start, inner.end]])
        );
        assert_eq!(
            second["to"],
            serde_json::json!({ "seconds": passes[1].to().seconds(), "nanos": 250_000_000 })
        );
    }

    /// R09.T18: reopening a universe restores the same coverage bytes and revisions, and the
    /// revisions go on from there.
    #[tokio::test]
    async fn survey_reopening_a_universe_restores_the_same_coverage_bytes() {
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        record_all(&log, &passes()).await;
        let written = coverages(&log);
        assert_eq!(written.len(), 2);
        drop(log);
        assert_eq!(store.scan().unwrap().saves().len(), 2);

        let reopened = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        assert_eq!(coverages(&reopened), written);
        let earth_coverage = reopened.coverage(earth()).unwrap();
        assert_eq!(earth_coverage.level(), earth_level());
        assert_eq!(earth_coverage.as_bytes().len(), 393_216);
        assert_eq!(reopened.coverage(moon()).unwrap().as_bytes().len(), 24_576);

        let again = pass(
            earth(),
            earth_level(),
            9,
            SurveySource::Orbital,
            10.0,
            once(0..1),
        );
        assert_eq!(reopened.record(again).await.unwrap().get(), 5);
        assert_eq!(
            earth_coverage.revision().get(),
            4,
            "a snapshot keeps the coverage it was taken at"
        );
        assert_eq!(reopened.coverage(earth()).unwrap().revision().get(), 5);
        assert_eq!(
            SurveyLog::open(&store, other_universe())
                .await
                .unwrap()
                .coverage(earth()),
            None,
            "each universe has its own log"
        );
    }

    /// R09.T18: at each cell the finest resolution of every pass over it wins, whatever came
    /// first, and a cell no pass covered has no code.
    #[tokio::test]
    async fn survey_the_finest_resolution_wins() {
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        let passes = passes();
        record_all(&log, &passes).await;
        let coverage = log.coverage(earth()).unwrap();
        let code = |metres: f64| ResolutionCode::from_resolution(Metres::new(metres)).unwrap();
        let (edge, wide) = (passes[0].cells()[0].clone(), passes[0].cells()[1].clone());
        let inner = passes[1].cells()[0].clone();
        assert_eq!(code(0.004), ResolutionCode::FINEST);
        assert!(code(2.0) < code(2_500.0), "a smaller code is finer");
        for index in 0..earth_level().cell_count() {
            let expected = if (inner.start..inner.start + 3).contains(&index) {
                code(0.004)
            } else if inner.contains(&index) {
                code(2.0)
            } else if wide.contains(&index) || edge.contains(&index) {
                // The 40 km pass over half of these is coarser than the 2.5 km one.
                code(2_500.0)
            } else {
                ResolutionCode::NONE
            };
            assert_eq!(coverage.code(index), Some(expected), "cell {index}");
        }
        assert_eq!(coverage.code(earth_level().cell_count()), None);
        assert_eq!(
            coverage.surveyed(),
            len(&wide) + len(&edge),
            "the inner region lies inside the wide one"
        );
        let moon_coverage = log.coverage(moon()).unwrap();
        assert_eq!(moon_coverage.surveyed(), 97);
        assert_eq!(moon_coverage.code(96), Some(ResolutionCode::NONE));
        assert_eq!(moon_coverage.code(1_000), Some(code(120.0)));
    }

    /// The orders of `0..n`, every one.
    fn permutations(n: usize) -> Vec<Vec<usize>> {
        if n == 0 {
            return vec![Vec::new()];
        }
        permutations(n - 1)
            .into_iter()
            .flat_map(|order| {
                (0..n).map(move |at| {
                    let mut order = order.clone();
                    order.insert(at, n - 1);
                    order
                })
            })
            .collect()
    }

    /// R09.T18: folding gives the same coverage whatever order the passes come in, both folded
    /// in memory in every order and recorded in two orders through the file.
    #[tokio::test]
    async fn survey_folding_is_independent_of_pass_order() {
        let passes = passes();
        let fold = |order: &[usize]| {
            let mut surveys = Surveys::default();
            for &i in order {
                surveys.apply(&passes[i]).unwrap();
            }
            surveys.bodies
        };
        let orders = permutations(passes.len());
        assert_eq!(orders.len(), 120);
        let first = fold(&orders[0]);
        for order in &orders[1..] {
            assert_eq!(fold(order), first, "order {order:?}");
        }

        let (_dir, store) = universe();
        let forwards = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        record_all(&forwards, &passes).await;
        let mut reversed = passes.clone();
        reversed.reverse();
        let backwards = SurveyLog::open(&store, other_universe()).await.unwrap();
        record_all(&backwards, &reversed).await;
        drop((forwards, backwards));
        let forwards = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        let backwards = SurveyLog::open(&store, other_universe()).await.unwrap();
        assert_eq!(coverages(&forwards), coverages(&backwards));
        assert_eq!(*forwards.coverage(earth()).unwrap(), *first[&earth()]);
    }

    /// A pass that ends before it begins, has no code for its resolution, or whose cells are
    /// empty, out of order, overlapping or outside the field is refused.
    #[test]
    fn survey_pass_refuses_a_bad_span_resolution_or_cells() {
        fn parts(cells: impl IntoIterator<Item = Range<u32>>) -> SurveyPassParts {
            SurveyPassParts {
                body: moon(),
                from: UniverseTime::EPOCH,
                to: UniverseTime::EPOCH,
                source: SurveySource::Landed,
                resolution: Metres::new(0.5),
                level: moon_level(),
                cells: cells.into_iter().collect(),
            }
        }
        let good = SurveyPass::new(parts([0..4, 4..9, 20..24_576])).unwrap();
        assert_eq!(good.cell_count(), 9 + 24_556);
        assert_eq!(
            good.code(),
            ResolutionCode::from_resolution(Metres::new(0.5)).unwrap()
        );

        let early = UniverseTime::new(-1, 0).unwrap();
        assert_eq!(
            SurveyPass::new(SurveyPassParts {
                to: early,
                ..parts(once(0..1))
            }),
            Err(BuildSurveyPassError::Span {
                from: UniverseTime::EPOCH,
                to: early
            })
        );
        for metres in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e9] {
            assert!(
                matches!(
                    SurveyPass::new(SurveyPassParts {
                        resolution: Metres::new(metres),
                        ..parts(once(0..1))
                    }),
                    Err(BuildSurveyPassError::Resolution { resolution_m })
                        if resolution_m.to_bits() == metres.to_bits()
                ),
                "{metres} m"
            );
        }
        assert_eq!(
            SurveyPass::new(parts(None)),
            Err(BuildSurveyPassError::NoCells)
        );
        assert_eq!(
            SurveyPass::new(parts([0..4, 7..7])),
            Err(BuildSurveyPassError::EmptyRun { start: 7, end: 7 })
        );
        assert_eq!(
            SurveyPass::new(parts([0..4, 9..12, 11..20])),
            Err(BuildSurveyPassError::Unsorted { run: 2 })
        );
        assert_eq!(
            SurveyPass::new(parts([9..12, 0..4])),
            Err(BuildSurveyPassError::Unsorted { run: 1 })
        );
        assert_eq!(
            SurveyPass::new(parts(once(24_000..24_577))),
            Err(BuildSurveyPassError::OutsideField {
                end: 24_577,
                cells: 24_576
            })
        );
        assert_eq!(
            BuildSurveyPassError::OutsideField {
                end: 24_577,
                cells: 24_576
            }
            .to_string(),
            "cell run ending at 24577 reaches past the field's 24576 cells"
        );
    }

    /// A pass at another level than its body's earlier passes is refused, and nothing is written.
    #[tokio::test]
    async fn survey_pass_at_another_level_is_refused() {
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        record_all(&log, &passes()).await;
        let before = fs::read_to_string(surveys_file(&store)).unwrap();
        let stray = pass(
            moon(),
            earth_level(),
            5,
            SurveySource::Orbital,
            50.0,
            once(0..1),
        );
        let error = log.record(stray).await.unwrap_err();
        assert!(
            matches!(
                error,
                RecordSurveyError::Level { body, recorded, pass }
                    if body == moon() && recorded == moon_level() && pass == earth_level()
            ),
            "{error:?}"
        );
        assert_eq!(fs::read_to_string(surveys_file(&store)).unwrap(), before);
        assert_eq!(
            log.coverage(moon()).unwrap().revision(),
            CoverageRevision::FIRST
        );
    }

    /// A header of another format is refused, and so is a survey log of a later format beside
    /// this build's; neither is touched, and a later contacts file does not refuse the passes.
    #[tokio::test]
    async fn survey_log_of_an_unknown_format_is_refused() {
        let (_dir, store) = universe();
        let dir = store.knowledge_dir(UNIVERSE);
        fs::create_dir(&dir).unwrap();
        let path = surveys_file(&store);
        fs::write(&path, "{\"format\":2}\n").unwrap();
        let error = SurveyLog::open(&store, UNIVERSE).await.unwrap_err();
        assert!(
            matches!(&error, LoadKnowledgeError::UnsupportedFormat { format: 2, path: p } if *p == path),
            "{error:?}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"format\":2}\n");

        fs::write(&path, "{\"format\":1}\n").unwrap();
        fs::write(dir.join("contacts.v9.jsonl"), "{\"format\":9}\n").unwrap();
        SurveyLog::open(&store, UNIVERSE).await.unwrap();
        let later = dir.join("surveys.v2.jsonl");
        fs::write(&later, "{\"format\":2}\n").unwrap();
        let error = SurveyLog::open(&store, UNIVERSE).await.unwrap_err();
        assert!(
            matches!(&error, LoadKnowledgeError::UnsupportedFormat { format: 2, path: p } if *p == later),
            "{error:?}"
        );
    }

    /// A torn last line is dropped, and the next pass writes over it, so that the file reads
    /// cleanly again with the coverage it had.
    #[tokio::test]
    async fn survey_log_torn_last_line_is_dropped_and_written_over() {
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        record_all(&log, &passes()).await;
        let written = coverages(&log);
        drop(log);
        let path = surveys_file(&store);
        let whole = fs::read_to_string(&path).unwrap();
        // Half of one more line, as a crash in the middle of an append leaves it.
        fs::write(
            &path,
            format!("{whole}{{\"body\":\"0200080020000000.0003\",\"fr"),
        )
        .unwrap();
        let reopened = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        assert_eq!(coverages(&reopened), written);
        let more = pass(
            earth(),
            earth_level(),
            7,
            SurveySource::Orbital,
            30.0,
            once(5..6),
        );
        assert_eq!(reopened.record(more).await.unwrap().get(), 5);
        let after = coverages(&reopened);
        drop(reopened);
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with(&whole) && !text.contains("\"fr{"),
            "{text}"
        );
        assert_eq!(text.lines().count(), 7);
        assert_eq!(
            coverages(&SurveyLog::open(&store, UNIVERSE).await.unwrap()),
            after
        );
    }

    /// A complete line that does not parse, holds a bad value, or does not fit the passes before
    /// it refuses the file, naming the line and what is wrong.
    #[tokio::test]
    async fn survey_log_malformed_line_is_refused() {
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        record_all(&log, &passes()).await;
        drop(log);
        let path = surveys_file(&store);
        let whole = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = whole.lines().collect();
        let refused = |line: &str, number: usize| {
            let mut damaged = lines.clone();
            damaged[number - 1] = line;
            fs::write(&path, format!("{}\n", damaged.join("\n"))).unwrap();
            let store = &store;
            async move { SurveyLog::open(store, UNIVERSE).await.unwrap_err() }
        };
        let reason_of = |error: LoadKnowledgeError| match error {
            LoadKnowledgeError::MalformedLine { line, reason, .. } => (line, reason),
            other => panic!("not a malformed line: {other:?}"),
        };

        let garbled = lines[2].replace("\"cells\"", "\"cell\"");
        assert_eq!(reason_of(refused(&garbled, 3).await), (3, "not an entry"));
        let body = lines[1].replace(&earth().to_string(), "0200080020000000.3");
        assert_eq!(reason_of(refused(&body, 2).await), (2, "body"));
        let level = lines[1].replace("\"level\":8", "\"level\":9");
        assert_eq!(reason_of(refused(&level, 2).await), (2, "level"));
        let coarse = lines[1].replace("\"resolution_m\":2500.0", "\"resolution_m\":0.0");
        assert_eq!(reason_of(refused(&coarse, 2).await), (2, "resolution_m"));
        let cells = lines[5].replace("[[0,96],", "[[96,0],");
        assert_eq!(reason_of(refused(&cells, 6).await), (6, "cells"));
        // The Moon's pass, said to be at level 7 and put before its own line: its cells lie inside
        // a level-7 field, so it loads, but the Moon's own pass at level 6 then does not fit.
        let moved = lines[5].replace("\"level\":6", "\"level\":7");
        let mut misfit = lines.clone();
        misfit.insert(5, &moved);
        fs::write(&path, format!("{}\n", misfit.join("\n"))).unwrap();
        assert_eq!(
            reason_of(SurveyLog::open(&store, UNIVERSE).await.unwrap_err()),
            (7, "does not fit the passes before it")
        );
    }

    /// No directory appears for a universe with no save: the first pass fails instead, and the
    /// coverage stays empty.
    #[tokio::test]
    async fn survey_log_of_a_universe_with_no_save_is_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let store = UniverseStore::new(dir.path());
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        let error = log
            .record(pass(
                earth(),
                earth_level(),
                0,
                SurveySource::Orbital,
                10.0,
                once(0..1),
            ))
            .await
            .unwrap_err();
        assert!(
            matches!(
                &error,
                RecordSurveyError::Save(SaveKnowledgeError::Io {
                    operation: "create directory",
                    ..
                })
            ),
            "{error:?}"
        );
        assert_eq!(log.coverage(earth()), None, "a failed save changes nothing");
        assert!(!store.knowledge_dir(UNIVERSE).exists());
    }

    /// Passes recorded at once through clones reach the file in the order they reach the
    /// coverage: each body's revisions are handed out once each, and reopening gives the coverage
    /// as it stood.
    #[tokio::test]
    async fn survey_concurrent_passes_reach_the_file_in_the_coverage_s_order() {
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        let mut tasks = tokio::task::JoinSet::new();
        for i in 0..48_u32 {
            let log = log.clone();
            let (body, level) = if i % 3 == 0 {
                (moon(), moon_level())
            } else {
                (earth(), earth_level())
            };
            let resolution_m = f64::from(1 + i % 7) * 10.0;
            let run = i * 97..i * 97 + 50;
            tasks.spawn(async move {
                let pass = pass(
                    body,
                    level,
                    i64::from(i),
                    SurveySource::Orbital,
                    resolution_m,
                    once(run),
                );
                (body, log.record(pass).await.unwrap())
            });
        }
        let mut revisions: BTreeMap<BodyId, Vec<u32>> = BTreeMap::new();
        let joined = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            while let Some(done) = tasks.join_next().await {
                let (body, revision) = done.unwrap();
                revisions.entry(body).or_default().push(revision.get());
            }
        })
        .await;
        assert!(joined.is_ok(), "the passes did not finish");
        for (body, count) in [(moon(), 16), (earth(), 32)] {
            let given = revisions.get_mut(&body).unwrap();
            given.sort_unstable();
            assert_eq!(*given, (1..=count).collect::<Vec<u32>>(), "{body}");
        }
        let live = coverages(&log);
        drop(log);
        assert_eq!(
            coverages(&SurveyLog::open(&store, UNIVERSE).await.unwrap()),
            live
        );
    }

    /// Format 1's lines are pinned, every source and field in its order on disk, so that a
    /// renamed field or source cannot leave the saves already written unreadable.
    #[tokio::test]
    async fn survey_lines_of_format_1_are_pinned() {
        let at = |seconds, nanos| UniverseTime::new(seconds, nanos).unwrap();
        let parts = |body, span: [UniverseTime; 2], source, metres, level, cells| SurveyPassParts {
            body,
            from: span[0],
            to: span[1],
            source,
            resolution: Metres::new(metres),
            level,
            cells,
        };
        let passes = [
            parts(
                earth(),
                [at(31_557_600, 0), at(31_563_000, 250_000_000)],
                SurveySource::Orbital,
                2_500.0,
                earth_level(),
                vec![0..64, 100..101],
            ),
            parts(
                earth(),
                [at(31_600_000, 0), at(31_600_600, 0)],
                SurveySource::CloseRange,
                2.0,
                earth_level(),
                vec![393_000..393_100, 393_215..393_216],
            ),
            parts(
                moon(),
                [at(-86_400, 5), at(-86_400, 5)],
                SurveySource::Landed,
                0.004,
                moon_level(),
                vec![0..1, 1..2],
            ),
        ];
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        for parts in passes {
            log.record(SurveyPass::new(parts).unwrap()).await.unwrap();
        }
        assert_eq!(
            fs::read_to_string(surveys_file(&store)).unwrap(),
            concat!(
                "{\"format\":1}\n",
                r#"{"body":"0200080020000000.0003","from":{"seconds":31557600,"nanos":0},"#,
                r#""to":{"seconds":31563000,"nanos":250000000},"source":"orbital","#,
                r#""resolution_m":2500.0,"level":8,"cells":[[0,64],[100,101]]}"#,
                "\n",
                r#"{"body":"0200080020000000.0003","from":{"seconds":31600000,"nanos":0},"#,
                r#""to":{"seconds":31600600,"nanos":0},"source":"close_range","#,
                r#""resolution_m":2.0,"level":8,"cells":[[393000,393100],[393215,393216]]}"#,
                "\n",
                r#"{"body":"0200080020000001.0103","from":{"seconds":-86400,"nanos":5},"#,
                r#""to":{"seconds":-86400,"nanos":5},"source":"landed","#,
                r#""resolution_m":0.004,"level":6,"cells":[[0,1],[1,2]]}"#,
                "\n",
            )
        );
    }

    /// Revisions count from 1, and a body with 2³² − 1 passes is refused one more, with nothing
    /// written and its coverage as it was.
    #[tokio::test]
    async fn survey_pass_past_the_last_revision_is_refused() {
        assert_eq!(CoverageRevision::new(0), None);
        assert_eq!(CoverageRevision::new(1), Some(CoverageRevision::FIRST));
        assert_eq!(
            CoverageRevision::FIRST.next().map(CoverageRevision::get),
            Some(2)
        );
        let last = CoverageRevision::new(u32::MAX).unwrap();
        assert_eq!(last.next(), None);
        assert_eq!(last.to_string(), "4294967295");

        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        let first = pass(
            moon(),
            moon_level(),
            0,
            SurveySource::Orbital,
            50.0,
            once(0..1),
        );
        log.record(first.clone()).await.unwrap();
        {
            // As if 2³² − 2 more passes had followed the first.
            let mut surveys = lock(&log.shared.surveys);
            let coverage = surveys.bodies.get_mut(&moon()).unwrap();
            Arc::make_mut(coverage).revision = last;
        }
        let before = fs::read_to_string(surveys_file(&store)).unwrap();
        let error = log.record(first).await.unwrap_err();
        assert!(
            matches!(error, RecordSurveyError::Revisions { body } if body == moon()),
            "{error:?}"
        );
        assert_eq!(fs::read_to_string(surveys_file(&store)).unwrap(), before);
        assert_eq!(log.coverage(moon()).unwrap().revision(), last);
    }

    /// Once a pass panicked while holding the file's lock, every later pass is refused as
    /// poisoned, and the coverage still reads.
    #[tokio::test]
    async fn survey_log_refuses_passes_after_a_panic_but_still_reads() {
        let (_dir, store) = universe();
        let log = SurveyLog::open(&store, UNIVERSE).await.unwrap();
        record_all(&log, &passes()).await;
        let written = coverages(&log);
        let shared = Arc::clone(&log.shared);
        let panicked = tokio::task::spawn_blocking(move || {
            let _held = shared.log.lock().unwrap();
            panic!("a pass panics part-way");
        })
        .await;
        assert!(panicked.unwrap_err().is_panic());
        let more = pass(
            earth(),
            earth_level(),
            8,
            SurveySource::Orbital,
            10.0,
            once(0..1),
        );
        let error = log.record(more).await.unwrap_err();
        assert!(
            matches!(error, RecordSurveyError::Save(SaveKnowledgeError::Poisoned)),
            "{error:?}"
        );
        assert_eq!(coverages(&log), written, "the coverage still reads");
    }
}
