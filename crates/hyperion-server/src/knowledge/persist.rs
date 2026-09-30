//! The Knowledge store on disk: versioned JSON lines in the universe's reserved `knowledge/`
//! directory (plan 12, P12.T7.b; Design note 10).
//!
//! The file is `<data_dir>/universes/<id>/knowledge/contacts.v1.jsonl`. Its first line is the
//! header, `{"format":1}`; every other line is one change, a sighting or an acknowledgement, in
//! the order it was made, and the store is rebuilt by applying them in order. A change is
//! appended, synced, and only then applied in memory, so memory never runs ahead of the disk.
//!
//! On load, a header of another format, or a file of a later format beside this one
//! (`contacts.v2.jsonl` and on), is [`LoadKnowledgeError::UnsupportedFormat`]. A last line with no
//! newline is what a crash in the middle of an append leaves: it is dropped with a warning, and the
//! next append writes over it. Any other line that does not parse, or that does not fit the
//! contacts before it, is [`LoadKnowledgeError::MalformedLine`], and nothing is loaded.
//!
//! Every file operation is blocking and runs on [`tokio::task::spawn_blocking`], never on the
//! runtime; the blocking task finishes even if the future awaiting it is dropped, so the store and
//! its file stay in step.

use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use hyperion_sim::alerts::AlertBand;
use hyperion_sim::coords::{GalacticPosition, LyCell};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::id::EventId;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::WattsPerSquareMetre;
use serde::{Deserialize, Serialize};

use super::record::{ContactId, KnowledgeLevel, Sighting, SightingParts};
use super::store::{AcknowledgeContactError, KnowledgeEntry, KnowledgeStore};
use crate::universe::{UniverseId, UniverseStore};

/// The file format this build writes and the only one it reads.
pub const KNOWLEDGE_FORMAT: u32 = 1;

/// The contacts file's name for [`KNOWLEDGE_FORMAT`].
const CONTACTS_FILE: &str = "contacts.v1.jsonl";

/// A universe's Knowledge store, kept in step with its file (plan 12, P12.T7.b).
///
/// Changes are made through [`record_sighting`](Self::record_sighting) and
/// [`acknowledge`](Self::acknowledge), which append to the file on a blocking task and then apply
/// the change; [`read`](Self::read) lends the store for its views. Cloning shares the store.
#[derive(Debug, Clone)]
pub struct PersistedKnowledge {
    shared: Arc<Shared>,
}

/// The store and its file. The file's lock is taken first and held over a whole change, so that
/// changes reach the file in the order they reach the store; the store's is held only to plan and
/// to apply one, so that readers wait for no disk.
#[derive(Debug)]
struct Shared {
    log: Mutex<KnowledgeLog>,
    store: Mutex<KnowledgeStore>,
}

impl PersistedKnowledge {
    /// Loads universe `universe`'s Knowledge from `store`'s data directory: empty if it has no
    /// file yet. Nothing is created until the first change.
    ///
    /// A universe's file has one writer: the caller keeps one `PersistedKnowledge` per universe
    /// and shares it by cloning, since two opened from one file would each cut the other's lines
    /// off at their own length (the alert service, P12.T8, owns them).
    ///
    /// # Errors
    ///
    /// [`LoadKnowledgeError`]: the directory or the file could not be read, the file is of
    /// another format or a later one lies beside it, a line other than a torn last one is
    /// malformed, or the blocking task did not finish.
    pub async fn open(
        store: &UniverseStore,
        universe: UniverseId,
    ) -> Result<Self, LoadKnowledgeError> {
        let dir = store.knowledge_dir(universe);
        let (store, log) = tokio::task::spawn_blocking(move || load(dir))
            .await
            .map_err(|error| {
                tracing::error!(%error, "a knowledge load's blocking task did not finish");
                LoadKnowledgeError::Interrupted
            })??;
        Ok(Self {
            shared: Arc::new(Shared {
                log: Mutex::new(log),
                store: Mutex::new(store),
            }),
        })
    }

    /// Records a sighting of `event` ([`KnowledgeStore::record_sighting`]), appending it to the
    /// file first.
    ///
    /// # Errors
    ///
    /// [`SaveKnowledgeError`] if the append failed or the blocking task did not finish. The store
    /// in memory is then unchanged; the file may hold the change if the failure came after its
    /// write (a failed sync), and the next append cuts it off again.
    /// [`SaveKnowledgeError::Poisoned`] once an earlier change panicked: the universe's Knowledge
    /// must be opened again.
    pub async fn record_sighting(
        &self,
        event: EventId,
        sighting: Sighting,
    ) -> Result<ContactId, SaveKnowledgeError> {
        let shared = Arc::clone(&self.shared);
        tokio::task::spawn_blocking(move || shared.record_sighting(event, sighting))
            .await
            .map_err(|error| {
                tracing::error!(%error, "a knowledge append's blocking task did not finish");
                SaveKnowledgeError::Interrupted
            })?
    }

    /// Acknowledges `contact` ([`KnowledgeStore::acknowledge`]), appending the acknowledgement to
    /// the file first; one already acknowledged writes nothing and returns `false`.
    ///
    /// # Errors
    ///
    /// [`AcknowledgeContactError::UnknownContact`] if the store has no such contact, and
    /// [`AcknowledgeContactError::Save`] as [`record_sighting`](Self::record_sighting) fails.
    pub async fn acknowledge(&self, contact: ContactId) -> Result<bool, AcknowledgeContactError> {
        let shared = Arc::clone(&self.shared);
        tokio::task::spawn_blocking(move || shared.acknowledge(contact))
            .await
            .map_err(|error| {
                tracing::error!(%error, "a knowledge append's blocking task did not finish");
                AcknowledgeContactError::Save(SaveKnowledgeError::Interrupted)
            })?
    }

    /// Calls `f` with the store, for its [`contacts`](KnowledgeStore::contacts) and
    /// [`view`](KnowledgeStore::view)s. The lock is held only while `f` runs and never over a
    /// write, so `f` should be short.
    pub fn read<R>(&self, f: impl FnOnce(&KnowledgeStore) -> R) -> R {
        f(&lock(&self.shared.store))
    }
}

impl Shared {
    fn record_sighting(
        &self,
        event: EventId,
        sighting: Sighting,
    ) -> Result<ContactId, SaveKnowledgeError> {
        let mut log = self.writer()?;
        let entry = lock(&self.store).sighting_entry(event, sighting);
        let contact = entry.contact();
        log.append(&entry)?;
        lock(&self.store)
            .apply(entry)
            .expect("an entry the store planned under the file's lock still fits it");
        Ok(contact)
    }

    fn acknowledge(&self, contact: ContactId) -> Result<bool, AcknowledgeContactError> {
        let mut log = self.writer().map_err(AcknowledgeContactError::Save)?;
        match lock(&self.store).view(contact) {
            None => return Err(AcknowledgeContactError::UnknownContact(contact)),
            Some(view) if view.seen().acknowledged() == super::Acknowledgement::Acknowledged => {
                return Ok(false);
            }
            Some(_) => {}
        }
        let entry = KnowledgeEntry::Acknowledged { contact };
        log.append(&entry).map_err(AcknowledgeContactError::Save)?;
        lock(&self.store)
            .apply(entry)
            .expect("a contact the store had under the file's lock still has it");
        Ok(true)
    }
}

impl Shared {
    /// The file's lock, refused once a change panicked while holding it: the change may be on
    /// disk and not in memory, and a further change could then take a contact number the file has
    /// given already.
    fn writer(&self) -> Result<MutexGuard<'_, KnowledgeLog>, SaveKnowledgeError> {
        self.log.lock().map_err(|_| SaveKnowledgeError::Poisoned)
    }
}

/// Takes the store's lock whether or not a holder panicked: the store is changed only by one
/// `apply` at a time, which leaves it whole, so readers may go on.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The contacts file, as far as it holds good lines.
#[derive(Debug)]
struct KnowledgeLog {
    dir: PathBuf,
    path: PathBuf,
    /// Open once the first append needs it.
    file: Option<File>,
    /// The length of the good lines: an append starts here, cutting off whatever a failed append
    /// or a crash left beyond it.
    len: u64,
}

impl KnowledgeLog {
    /// Appends `entry`, after the header if the file holds nothing yet, and syncs it.
    fn append(&mut self, entry: &KnowledgeEntry) -> Result<(), SaveKnowledgeError> {
        let mut text = String::new();
        if self.len == 0 {
            push_line(
                &mut text,
                &HeaderLine {
                    format: KNOWLEDGE_FORMAT,
                },
            );
        }
        push_line(&mut text, &EntryLine::from(entry));
        let len = self.len;
        let path = self.path.clone();
        let file = self.file()?;
        file.set_len(len).map_err(save_io("truncate", &path))?;
        file.seek(SeekFrom::Start(len))
            .map_err(save_io("seek", &path))?;
        file.write_all(text.as_bytes())
            .map_err(save_io("write", &path))?;
        file.sync_data().map_err(save_io("sync", &path))?;
        self.len = len + u64::try_from(text.len()).expect("a line's length fits in 64 bits");
        Ok(())
    }

    /// The open file, created with its directory if need be. The directory is created inside the
    /// universe's, never the universe's itself, so that no directory appears for a universe with
    /// no save.
    fn file(&mut self) -> Result<&mut File, SaveKnowledgeError> {
        if self.file.is_none() {
            let made_dir = match fs::create_dir(&self.dir) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(save_io("create directory", &self.dir)(error)),
            };
            let existed = self.path.exists();
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(&self.path)
                .map_err(save_io("open", &self.path))?;
            if !existed {
                sync_directory(&self.dir).map_err(save_io("sync directory", &self.dir))?;
            }
            if made_dir && let Some(universe) = self.dir.parent() {
                sync_directory(universe).map_err(save_io("sync directory", universe))?;
            }
            self.file = Some(file);
        }
        Ok(self.file.as_mut().expect("the file was opened above"))
    }
}

/// The error of a failed step of an append.
fn save_io(operation: &'static str, path: &Path) -> impl FnOnce(io::Error) -> SaveKnowledgeError {
    let path = path.to_path_buf();
    move |source| SaveKnowledgeError::Io {
        operation,
        path,
        source,
    }
}

/// Appends `line` as JSON and a newline.
fn push_line(text: &mut String, line: &impl Serialize) {
    text.push_str(&serde_json::to_string(line).expect("a line of strings and numbers serialises"));
    text.push('\n');
}

/// Makes the entries of `dir` durable.
#[cfg(unix)]
fn sync_directory(dir: &Path) -> io::Result<()> {
    File::open(dir).and_then(|handle| handle.sync_all())
}

/// Makes the entries of `dir` durable. Other platforms cannot open a directory as a file.
#[cfg(not(unix))]
fn sync_directory(_dir: &Path) -> io::Result<()> {
    Ok(())
}

/// Reads the store in `dir` (module documentation).
fn load(dir: PathBuf) -> Result<(KnowledgeStore, KnowledgeLog), LoadKnowledgeError> {
    refuse_later_formats(&dir)?;
    let path = dir.join(CONTACTS_FILE);
    let mut log = KnowledgeLog {
        dir,
        path,
        file: None,
        len: 0,
    };
    let bytes = match fs::read(&log.path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok((KnowledgeStore::new(), log));
        }
        Err(source) => {
            return Err(LoadKnowledgeError::Io {
                operation: "read",
                path: log.path,
                source,
            });
        }
    };
    let mut store = KnowledgeStore::new();
    let mut good = 0;
    let mut rest = bytes.as_slice();
    let mut number = 0;
    while !rest.is_empty() {
        number += 1;
        let Some(end) = rest.iter().position(|&byte| byte == b'\n') else {
            tracing::warn!(
                path = %log.path.display(),
                line = number,
                bytes = rest.len(),
                "dropping a torn last line of a knowledge file"
            );
            break;
        };
        let line = &rest[..end];
        let malformed = |reason, source| LoadKnowledgeError::MalformedLine {
            path: log.path.clone(),
            line: number,
            reason,
            source,
        };
        if number == 1 {
            let header: HeaderLine = serde_json::from_slice(line)
                .map_err(|error| malformed("not a header", Some(error)))?;
            match header.format {
                KNOWLEDGE_FORMAT => {}
                0 => return Err(malformed("format 0 is never written", None)),
                format => {
                    return Err(LoadKnowledgeError::UnsupportedFormat {
                        path: log.path,
                        format,
                    });
                }
            }
        } else {
            let parsed: EntryLine = serde_json::from_slice(line)
                .map_err(|error| malformed("not an entry", Some(error)))?;
            let entry = KnowledgeEntry::try_from(parsed).map_err(|field| malformed(field, None))?;
            store
                .apply(entry)
                .map_err(|_| malformed("does not fit the contacts before it", None))?;
        }
        good += end + 1;
        rest = &rest[end + 1..];
    }
    log.len = u64::try_from(good).expect("a file's length fits in 64 bits");
    Ok((store, log))
}

/// Refuses a directory that holds a contacts file of a later format than this build's.
fn refuse_later_formats(dir: &Path) -> Result<(), LoadKnowledgeError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(LoadKnowledgeError::Io {
                operation: "list",
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    let mut latest = None;
    for entry in entries {
        let entry = entry.map_err(|source| LoadKnowledgeError::Io {
            operation: "list",
            path: dir.to_path_buf(),
            source,
        })?;
        let format = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix("contacts.v"))
            .and_then(|name| name.strip_suffix(".jsonl"))
            .and_then(|number| number.parse::<u32>().ok());
        if let Some(format) = format
            && format > KNOWLEDGE_FORMAT
        {
            latest = latest.max(Some((format, entry.path())));
        }
    }
    match latest {
        Some((format, path)) => Err(LoadKnowledgeError::UnsupportedFormat { path, format }),
        None => Ok(()),
    }
}

/// The header line.
#[derive(Debug, Serialize, Deserialize)]
struct HeaderLine {
    format: u32,
}

/// One change, as a line of format 1. Field order is the order on disk.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum EntryLine {
    Sighting {
        contact: u32,
        /// The event in the sim's text form, which names the host.
        event: String,
        observer: PositionLine,
        observed: TimeLine,
        emitted: TimeLine,
        apparent_position: PositionLine,
        present_position: PositionLine,
        flux_w_m2: f64,
        band: BandLine,
        level: LevelLine,
    },
    Acknowledged {
        contact: u32,
    },
}

/// A galactic position, exactly: its light-year cell and the offset in metres.
#[derive(Debug, Serialize, Deserialize)]
struct PositionLine {
    cell_ly: [i32; 3],
    offset_m: [f64; 3],
}

/// An instant of the universe clock, exactly.
#[derive(Debug, Serialize, Deserialize)]
struct TimeLine {
    seconds: i64,
    nanos: u32,
}

/// An [`AlertBand`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BandLine {
    U,
    B,
    V,
    R,
    I,
    J,
    H,
    K,
    MidInfrared,
    Radio,
    XRay,
}

/// A [`KnowledgeLevel`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LevelLine {
    Bearing,
    Resolved,
}

impl From<&KnowledgeEntry> for EntryLine {
    fn from(entry: &KnowledgeEntry) -> Self {
        match entry {
            KnowledgeEntry::Sighting {
                contact,
                event,
                sighting,
            } => {
                let parts = sighting.parts();
                Self::Sighting {
                    contact: contact.get(),
                    event: event.to_string(),
                    observer: PositionLine::from(&parts.observer),
                    observed: TimeLine::from(parts.observed),
                    emitted: TimeLine::from(parts.emitted),
                    apparent_position: PositionLine::from(&parts.apparent_position),
                    present_position: PositionLine::from(&parts.present_position),
                    flux_w_m2: parts.flux.value(),
                    band: BandLine::from(parts.band),
                    level: LevelLine::from(parts.level),
                }
            }
            KnowledgeEntry::Acknowledged { contact } => Self::Acknowledged {
                contact: contact.get(),
            },
        }
    }
}

impl TryFrom<EntryLine> for KnowledgeEntry {
    /// The field that does not hold a valid value.
    type Error = &'static str;

    fn try_from(line: EntryLine) -> Result<Self, Self::Error> {
        let contact_of = |number| ContactId::new(number).ok_or("contact");
        match line {
            EntryLine::Sighting {
                contact,
                event,
                observer,
                observed,
                emitted,
                apparent_position,
                present_position,
                flux_w_m2,
                band,
                level,
            } => {
                if !(flux_w_m2.is_finite() && flux_w_m2 >= 0.0) {
                    return Err("flux_w_m2");
                }
                let sighting = Sighting::from_parts(SightingParts {
                    observer: observer.to_position().ok_or("observer")?,
                    observed: observed.to_time().ok_or("observed")?,
                    emitted: emitted.to_time().ok_or("emitted")?,
                    apparent_position: apparent_position
                        .to_position()
                        .ok_or("apparent_position")?,
                    present_position: present_position.to_position().ok_or("present_position")?,
                    flux: WattsPerSquareMetre::new(flux_w_m2),
                    band: AlertBand::from(band),
                    level: KnowledgeLevel::from(level),
                });
                Ok(Self::Sighting {
                    contact: contact_of(contact)?,
                    // The field's name is the whole diagnosis: the text was written by this
                    // build's `Display` and only damage can make it fail to parse.
                    event: event.parse().map_err(|_| "event")?,
                    sighting,
                })
            }
            EntryLine::Acknowledged { contact } => Ok(Self::Acknowledged {
                contact: contact_of(contact)?,
            }),
        }
    }
}

impl From<&GalacticPosition> for PositionLine {
    fn from(position: &GalacticPosition) -> Self {
        Self {
            cell_ly: position.cell().to_array(),
            offset_m: position.offset_metres(),
        }
    }
}

impl PositionLine {
    /// The position, or `None` for an offset outside a light-year cell, which only damage makes;
    /// the caller names the field.
    fn to_position(&self) -> Option<GalacticPosition> {
        GalacticPosition::new(LyCell::new(self.cell_ly), self.offset_m).ok()
    }
}

impl From<UniverseTime> for TimeLine {
    fn from(time: UniverseTime) -> Self {
        Self {
            seconds: time.seconds(),
            nanos: time.subsec_nanos(),
        }
    }
}

impl TimeLine {
    /// The time, or `None` for nanoseconds past a second, which only damage makes; the caller
    /// names the field.
    fn to_time(&self) -> Option<UniverseTime> {
        UniverseTime::new(self.seconds, self.nanos).ok()
    }
}

impl From<AlertBand> for BandLine {
    fn from(band: AlertBand) -> Self {
        match band {
            AlertBand::Photometric(Band::U) => Self::U,
            AlertBand::Photometric(Band::B) => Self::B,
            AlertBand::Photometric(Band::V) => Self::V,
            AlertBand::Photometric(Band::R) => Self::R,
            AlertBand::Photometric(Band::I) => Self::I,
            AlertBand::Photometric(Band::J) => Self::J,
            AlertBand::Photometric(Band::H) => Self::H,
            AlertBand::Photometric(Band::K) => Self::K,
            AlertBand::Photometric(Band::MidInfrared) => Self::MidInfrared,
            AlertBand::Photometric(Band::Radio) => Self::Radio,
            AlertBand::XRay => Self::XRay,
        }
    }
}

impl From<BandLine> for AlertBand {
    fn from(band: BandLine) -> Self {
        match band {
            BandLine::U => Self::Photometric(Band::U),
            BandLine::B => Self::Photometric(Band::B),
            BandLine::V => Self::Photometric(Band::V),
            BandLine::R => Self::Photometric(Band::R),
            BandLine::I => Self::Photometric(Band::I),
            BandLine::J => Self::Photometric(Band::J),
            BandLine::H => Self::Photometric(Band::H),
            BandLine::K => Self::Photometric(Band::K),
            BandLine::MidInfrared => Self::Photometric(Band::MidInfrared),
            BandLine::Radio => Self::Photometric(Band::Radio),
            BandLine::XRay => Self::XRay,
        }
    }
}

impl From<KnowledgeLevel> for LevelLine {
    fn from(level: KnowledgeLevel) -> Self {
        match level {
            KnowledgeLevel::Bearing => Self::Bearing,
            KnowledgeLevel::Resolved => Self::Resolved,
        }
    }
}

impl From<LevelLine> for KnowledgeLevel {
    fn from(level: LevelLine) -> Self {
        match level {
            LevelLine::Bearing => Self::Bearing,
            LevelLine::Resolved => Self::Resolved,
        }
    }
}

/// A universe's Knowledge could not be loaded.
#[derive(Debug)]
pub enum LoadKnowledgeError {
    /// The directory could not be listed or the file could not be read.
    Io {
        /// What was being done: `"list"` or `"read"`.
        operation: &'static str,
        /// The directory or file.
        path: PathBuf,
        /// The operating system's error.
        source: io::Error,
    },
    /// The file is of a format this build cannot read, or a file of a later format lies beside
    /// it. Nothing is loaded and nothing is rewritten.
    UnsupportedFormat {
        /// The file.
        path: PathBuf,
        /// The format it declares.
        format: u32,
    },
    /// A line other than a torn last one does not parse or does not fit the contacts before it.
    MalformedLine {
        /// The file.
        path: PathBuf,
        /// The line's number, from 1 for the header.
        line: usize,
        /// What is wrong: the field that does not hold a valid value, or the line as a whole.
        reason: &'static str,
        /// The parser's error, if the line is not the JSON it should be.
        source: Option<serde_json::Error>,
    },
    /// The blocking task that read the file did not finish.
    Interrupted,
}

impl fmt::Display for LoadKnowledgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation, path, ..
            } => write!(f, "failed to {operation} {}", path.display()),
            Self::UnsupportedFormat { path, format } => write!(
                f,
                "{} is in knowledge format {format}, which this build cannot read",
                path.display()
            ),
            Self::MalformedLine {
                path, line, reason, ..
            } => write!(
                f,
                "line {line} of {} is malformed: {reason}",
                path.display()
            ),
            Self::Interrupted => f.write_str("the knowledge file's read did not finish"),
        }
    }
}

impl Error for LoadKnowledgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::MalformedLine {
                source: Some(source),
                ..
            } => Some(source),
            Self::UnsupportedFormat { .. }
            | Self::MalformedLine { source: None, .. }
            | Self::Interrupted => None,
        }
    }
}

/// A change to a universe's Knowledge could not be saved, and was not made in memory.
#[derive(Debug)]
pub enum SaveKnowledgeError {
    /// A step of the append failed.
    Io {
        /// What was being done: `"create directory"`, `"open"`, `"write"`, `"sync"` and so on.
        operation: &'static str,
        /// The file or directory it was done to.
        path: PathBuf,
        /// The operating system's error.
        source: io::Error,
    },
    /// The blocking task that wrote the change did not finish.
    Interrupted,
    /// An earlier change panicked, so the file and the store may disagree: nothing more is
    /// written until the universe's Knowledge is opened again.
    Poisoned,
}

impl fmt::Display for SaveKnowledgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation, path, ..
            } => write!(f, "failed to {operation} {}", path.display()),
            Self::Interrupted => f.write_str("the knowledge file's append did not finish"),
            Self::Poisoned => f.write_str("an earlier knowledge change failed part-way"),
        }
    }
}

impl Error for SaveKnowledgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Interrupted | Self::Poisoned => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::GeneratorVersion;
    use hyperion_sim::coords::GalacticVelocity;
    use hyperion_sim::observe::{Drift, Observer, retarded};

    use super::*;
    use crate::knowledge::testing::{event_of, host, other_host, third_host};
    use crate::knowledge::{Acknowledgement, ContactView};
    use crate::universe::SavedUniverse;

    const UNIVERSE: UniverseId = UniverseId::new(0xab);

    /// A data directory holding universe [`UNIVERSE`]'s save.
    fn universe() -> (tempfile::TempDir, UniverseStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = UniverseStore::new(dir.path());
        store
            .write(&SavedUniverse::new(
                UNIVERSE,
                "Kepler Reach".parse().unwrap(),
                0x4d2,
                GeneratorVersion::new(7),
            ))
            .unwrap();
        (dir, store)
    }

    fn contacts_file(store: &UniverseStore) -> PathBuf {
        store.knowledge_dir(UNIVERSE).join(CONTACTS_FILE)
    }

    /// A sighting from `from` at `years` of a star about 1,000 ly from the Sun.
    fn sighting(from: [f64; 3], years: i64, band: AlertBand, level: KnowledgeLevel) -> Sighting {
        let star = Drift::new(
            GalacticPosition::from_light_years([100.0, 26_000.0, 5.0]).unwrap(),
            GalacticVelocity::new([12e3, -220e3, 7e3]),
        );
        let observer = Observer::new(
            GalacticPosition::from_light_years(from).unwrap(),
            UniverseTime::from_julian_years(years).unwrap(),
        )
        .unwrap();
        Sighting::new(
            &observer,
            &retarded(&observer, &star),
            band,
            // A flux with every digit in use, so that the round trip is tested to the bit.
            WattsPerSquareMetre::new(
                1.234_567_890_123_456_7e-15 * f64::from(u32::try_from(years).unwrap() + 1),
            ),
            level,
        )
    }

    /// Records two contacts, one resolved and acknowledged, and returns the store as it stands.
    async fn fill(knowledge: &PersistedKnowledge) -> KnowledgeStore {
        let here = [0.0, 25_000.0, 0.0];
        let there = [-3_000.0, 21_000.0, 40.0];
        let v = AlertBand::Photometric(Band::V);
        let first = knowledge
            .record_sighting(
                event_of(host()),
                sighting(there, 0, v, KnowledgeLevel::Bearing),
            )
            .await
            .unwrap();
        let second = knowledge
            .record_sighting(
                event_of(other_host()),
                sighting(here, 3, AlertBand::XRay, KnowledgeLevel::Bearing),
            )
            .await
            .unwrap();
        knowledge
            .record_sighting(
                event_of(host()),
                sighting(here, 10, v, KnowledgeLevel::Resolved),
            )
            .await
            .unwrap();
        assert!(knowledge.acknowledge(first).await.unwrap());
        assert!(!knowledge.acknowledge(first).await.unwrap());
        assert_eq!(
            (first.get(), second.get()),
            (1, 2),
            "contacts are numbered in order"
        );
        knowledge.read(Clone::clone)
    }

    /// P12.T7.b: what is written is what is read back, bit for bit.
    #[tokio::test]
    async fn knowledge_round_trips_through_its_file() {
        let (_dir, store) = universe();
        let knowledge = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        assert!(knowledge.read(KnowledgeStore::is_empty));
        assert!(
            !store.knowledge_dir(UNIVERSE).exists(),
            "nothing is created before the first change"
        );
        let written = fill(&knowledge).await;
        drop(knowledge);
        let reread = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        assert_eq!(reread.read(Clone::clone), written);
        // One header and four changes: three sightings and one acknowledgement.
        let text = fs::read_to_string(contacts_file(&store)).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines[0], serde_json::json!({ "format": 1 }));
        let kinds: Vec<_> = lines[1..].iter().map(|line| line["kind"].clone()).collect();
        assert_eq!(
            kinds,
            ["sighting", "sighting", "sighting", "acknowledged"].map(serde_json::Value::from)
        );
        assert_eq!(lines[2]["band"], "x_ray");
        assert_eq!(lines[3]["level"], "resolved");
        assert_eq!(lines[1]["event"], event_of(host()).to_string());
    }

    /// P12.T7.b: reopening a universe restores its contacts and acknowledgements, and the
    /// universe's save still loads beside its `knowledge/` directory.
    #[tokio::test]
    async fn knowledge_reopening_a_universe_restores_contacts_and_acknowledgements() {
        let (_dir, store) = universe();
        let knowledge = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        fill(&knowledge).await;
        drop(knowledge);
        assert_eq!(store.scan().unwrap().saves().len(), 1);
        let reopened = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        let first = ContactId::new(1).unwrap();
        let second = ContactId::new(2).unwrap();
        let views = reopened.read(|known| {
            assert_eq!(known.contacts().collect::<Vec<_>>(), [first, second]);
            [known.view(first).unwrap(), known.view(second).unwrap()]
        });
        let ContactView::Resolved(resolved) = views[0] else {
            panic!("the first contact was resolved: {:?}", views[0]);
        };
        assert_eq!(resolved.host(), host());
        assert_eq!(
            resolved.seen().acknowledged(),
            Acknowledgement::Acknowledged
        );
        assert_eq!(resolved.seen().sightings(), 2);
        assert!(matches!(views[1], ContactView::Bearing(seen)
            if seen.acknowledged() == Acknowledgement::Pending && seen.band() == AlertBand::XRay));
        // A sighting after the reopening continues the numbering.
        let third = reopened
            .record_sighting(
                event_of(third_host()),
                sighting(
                    [0.0, 25_000.0, 0.0],
                    20,
                    AlertBand::XRay,
                    KnowledgeLevel::Bearing,
                ),
            )
            .await
            .unwrap();
        assert_eq!(third.get(), 3);
        assert!(matches!(
            reopened.acknowledge(ContactId::new(9).unwrap()).await,
            Err(AcknowledgeContactError::UnknownContact(id)) if id.get() == 9
        ));
    }

    /// P12.T7.b: a header of another format is `UnsupportedFormat`, and so is a file of a later
    /// format beside this build's; neither is touched.
    #[tokio::test]
    async fn knowledge_of_an_unknown_format_is_refused() {
        let (_dir, store) = universe();
        let dir = store.knowledge_dir(UNIVERSE);
        fs::create_dir(&dir).unwrap();
        let path = contacts_file(&store);
        fs::write(&path, "{\"format\":2}\n").unwrap();
        let error = PersistedKnowledge::open(&store, UNIVERSE)
            .await
            .unwrap_err();
        assert!(
            matches!(&error, LoadKnowledgeError::UnsupportedFormat { format: 2, path: p } if *p == path),
            "{error:?}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"format\":2}\n");

        fs::write(&path, "{\"format\":1}\n").unwrap();
        let later = dir.join("contacts.v3.jsonl");
        fs::write(&later, "{\"format\":3}\n").unwrap();
        fs::write(dir.join("contacts.v2.jsonl"), "{\"format\":2}\n").unwrap();
        let error = PersistedKnowledge::open(&store, UNIVERSE)
            .await
            .unwrap_err();
        assert!(
            matches!(&error, LoadKnowledgeError::UnsupportedFormat { format: 3, path: p } if *p == later),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            format!(
                "{} is in knowledge format 3, which this build cannot read",
                later.display()
            )
        );
    }

    /// P12.T7.b: a torn last line is dropped, and the next append writes over it, so that the
    /// file reads cleanly again.
    #[tokio::test]
    async fn knowledge_torn_last_line_is_dropped_and_written_over() {
        let (_dir, store) = universe();
        let knowledge = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        let written = fill(&knowledge).await;
        drop(knowledge);
        let path = contacts_file(&store);
        let whole = fs::read_to_string(&path).unwrap();
        // Half of one more line, as a crash in the middle of an append leaves it.
        fs::write(
            &path,
            format!("{whole}{{\"kind\":\"sighting\",\"contact\":3,\"eve"),
        )
        .unwrap();
        let reopened = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        assert_eq!(reopened.read(Clone::clone), written);
        let third = reopened
            .record_sighting(
                event_of(third_host()),
                sighting(
                    [0.0, 25_000.0, 0.0],
                    20,
                    AlertBand::XRay,
                    KnowledgeLevel::Bearing,
                ),
            )
            .await
            .unwrap();
        let after = reopened.read(Clone::clone);
        drop(reopened);
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with(&whole) && !text.contains("\"eve{"),
            "{text}"
        );
        assert_eq!(text.lines().count(), 6);
        let again = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        assert_eq!(again.read(Clone::clone), after);
        assert_eq!(third.get(), 3);

        // A torn header alone leaves an empty store, and the header is written again.
        fs::write(&path, "{\"form").unwrap();
        let fresh = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        assert!(fresh.read(KnowledgeStore::is_empty));
        fill(&fresh).await;
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\"format\":1}\n"), "{text}");
    }

    /// A complete line that does not parse, or that does not fit the contacts before it, refuses
    /// the file: only a torn last line is dropped.
    #[tokio::test]
    async fn knowledge_malformed_line_is_refused() {
        let (_dir, store) = universe();
        let knowledge = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        fill(&knowledge).await;
        drop(knowledge);
        let path = contacts_file(&store);
        let whole = fs::read_to_string(&path).unwrap();
        let mut lines: Vec<&str> = whole.lines().collect();
        let damaged = |lines: &[&str]| format!("{}\n", lines.join("\n"));

        let garbled = lines[2].replace("\"kind\"", "\"kin\"");
        let mut bad = lines.clone();
        bad[2] = &garbled;
        fs::write(&path, damaged(&bad)).unwrap();
        let error = PersistedKnowledge::open(&store, UNIVERSE)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                LoadKnowledgeError::MalformedLine {
                    line: 3,
                    reason: "not an entry",
                    source: Some(_),
                    ..
                }
            ),
            "{error:?}"
        );

        // The first contact's first sighting, dropped: the second contact's sighting, now the
        // first, names contact 2 where a first contact is 1.
        lines.remove(1);
        fs::write(&path, damaged(&lines)).unwrap();
        let error = PersistedKnowledge::open(&store, UNIVERSE)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                LoadKnowledgeError::MalformedLine {
                    line: 2,
                    reason: "does not fit the contacts before it",
                    source: None,
                    ..
                }
            ),
            "{error:?}"
        );

        let zero = whole.replace("\"contact\":1,", "\"contact\":0,");
        fs::write(&path, zero).unwrap();
        let error = PersistedKnowledge::open(&store, UNIVERSE)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                LoadKnowledgeError::MalformedLine {
                    line: 2,
                    reason: "contact",
                    ..
                }
            ),
            "{error:?}"
        );
    }

    /// No directory appears for a universe with no save: the first append fails instead.
    #[tokio::test]
    async fn knowledge_of_a_universe_with_no_save_is_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let store = UniverseStore::new(dir.path());
        let knowledge = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        let error = knowledge
            .record_sighting(
                event_of(host()),
                sighting(
                    [0.0, 25_000.0, 0.0],
                    0,
                    AlertBand::XRay,
                    KnowledgeLevel::Bearing,
                ),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                &error,
                SaveKnowledgeError::Io {
                    operation: "create directory",
                    ..
                }
            ),
            "{error:?}"
        );
        assert!(
            knowledge.read(KnowledgeStore::is_empty),
            "a failed save changes nothing"
        );
        assert!(!store.knowledge_dir(UNIVERSE).exists());
    }

    /// Changes made at once through clones reach the file in the order they reach the store:
    /// reopening gives the store as it stood. (A write that fails part-way cannot be injected
    /// here; `append` cuts the file back to its good length before each write.)
    #[tokio::test]
    async fn knowledge_concurrent_changes_reach_the_file_in_the_store_s_order() {
        let (_dir, store) = universe();
        let knowledge = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        let hosts = [host(), other_host(), third_host()];
        let mut tasks = tokio::task::JoinSet::new();
        for i in 0..48_i64 {
            let knowledge = knowledge.clone();
            let event = event_of(hosts[usize::try_from(i % 3).unwrap()]);
            tasks.spawn(async move {
                let level = if i % 5 == 0 {
                    KnowledgeLevel::Resolved
                } else {
                    KnowledgeLevel::Bearing
                };
                let contact = knowledge
                    .record_sighting(
                        event,
                        sighting([0.0, 25_000.0, 0.0], i, AlertBand::XRay, level),
                    )
                    .await
                    .unwrap();
                if i % 7 == 0 {
                    knowledge.acknowledge(contact).await.unwrap();
                }
            });
        }
        let joined = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            while let Some(done) = tasks.join_next().await {
                done.unwrap();
            }
        })
        .await;
        assert!(joined.is_ok(), "the changes did not finish");
        let live = knowledge.read(Clone::clone);
        assert_eq!(live.len(), 3);
        drop(knowledge);
        let reread = PersistedKnowledge::open(&store, UNIVERSE).await.unwrap();
        assert_eq!(reread.read(Clone::clone), live);
    }
}
