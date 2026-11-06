//! The sky's tables on disk, under the data directory (rendering plan R06;
//! perf-sky-tables-on-disk): a galaxy whose sky tables were built before the server last stopped
//! reads them back when its universe is next opened, so that its first sky waits on no build.
//!
//! A galaxy's luminosity tables cost some 34 CPU-s (R06.T11.c), built again at every launch: 13.2
//! of the 23.0 s from a relaunched server's open to its first sky's first reply near the Sun, the
//! plan's one job 5.1 s of them; read back, 0.22 s of 10.7–11.0 s (release, pod, 16 vCPU, eight
//! workers, 2026-11-05). Like the kept skies' replies and the streams' envelopes, they are a cache
//! of generated data that the server may remove at any time and rebuilds on a miss.
//!
//! A galaxy's tables are written to [`SKY_TABLE_FILES_DIR`] under the data directory, beside the
//! kept skies' files, once they are built, one file a galaxy, named by a hash of its key: the
//! protocol's and the build's generator versions, then the galaxy's key ([`GalaxyKey`]), what the
//! tables are built from and whether they carry the features' gas index, then the formats of the
//! tables' and the index's words ([`LuminosityTables::FORMAT`], [`FeatureGasIndex::FORMAT`]), the
//! key rules of the kept skies' and the envelopes' files. No build ID joins them, since a
//! generator version's tables are pinned by its goldens (as the kept skies lean, 2026-11-05).
//!
//! A file holds a magic, its key, the tables' words ([`LuminosityTables::write_words`]), then the
//! index's where the tables carry one ([`FeatureGasIndex::write_words`]), each word eight
//! little-endian bytes, and an FNV-1a checksum of all of them. It is written whole under a
//! temporary name and renamed into place, never synced: a file torn by a crash fails its checksum.
//! Tables holding a NaN, whose bits no other machine need share, are not written. A file whose key
//! differs is a miss; one that is short, torn, or whose words do not read back is a miss too, and
//! is removed. Neither is ever an error: the tables are built, as they would have been.
//!
//! The store's thread checks a file's frame, its length, magic, checksum and key, and hands the
//! words back ([`TablesFile`]); the caller makes them whole on the pool ([`TablesFile::tables`]),
//! since that computes the sim's values (the index's bins, the brightness envelope and the cells'
//! offset bounds, some 30 ms beside the read), and only the pool's workers are probed for a
//! floating-point mode that flushes subnormals. Words that do not read back are reported
//! ([`SkyTableFiles::remove_bad`]), and their file is removed.
//!
//! The files are bounded by bytes ([`ServerConfig::kept_sky_tables_disk_bytes`]: 160 MiB by
//! default, the tables' budget in memory, `decision-r06-tables.md` item B.2: two galaxies' tables
//! of 68 MiB, or one with its features' gas index; 0 keeps none), the least recently used removed
//! first, by their modification times across restarts. Every read, write and removal runs on the
//! store's own thread, `hyperion-sky-table-files`, which encodes and decodes the files too, fed by
//! a bounded queue ([`SKY_TABLE_FILE_QUEUE`]), so none runs on the runtime or in a pool job. Tables
//! the queue has no room for are not written, and the server's shutdown waits for those queued
//! ([`SkyTableFiles::close`]).
//!
//! [`ServerConfig::kept_sky_tables_disk_bytes`]: crate::ServerConfig::kept_sky_tables_disk_bytes

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

use hyperion_protocol::PROTOCOL_VERSION;
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::gas_overlay::FeatureGasIndex;
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::words::{WordSink, WordSource};
use tokio::sync::{mpsc, oneshot};

use super::GalaxyKey;
use super::sky::{SkyTables, SkyTablesSource};
use super::sky_tables::TablesGas;
use crate::cache::fnv1a;
use crate::limits::SKY_TABLE_FILE_QUEUE;

/// The directory under the data directory that holds the sky tables' files.
pub(crate) const SKY_TABLE_FILES_DIR: &str = "sky-tables";

/// What a sky tables' file starts with: `HYPTAB`, a zero and its format, 1.
const MAGIC: [u8; 8] = *b"HYPTAB\x00\x01";

/// The extension of a sky tables' file.
const EXTENSION: &str = "tables";

/// The extension of a file being written, removed if found at start.
const TEMP_EXTENSION: &str = "tmp";

/// The bytes of a file's key: the protocol's and the build's generator versions, the galaxy's seed
/// and generator version, the tables' source and gas, and the tables' and the index's formats,
/// each little-endian.
const KEY_BYTES: usize = 4 + 4 + 8 + 4 + 1 + 1 + 4 + 4;

/// The bytes of a file besides its words: the magic, the key and the checksum.
const FRAME_BYTES: usize = MAGIC.len() + KEY_BYTES + 8;

/// The sky tables' files' size and use: all zeros where they are off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SkyTableFileCounters {
    entries: usize,
    bytes: u64,
    budget: u64,
    hits: u64,
    misses: u64,
    written: u64,
    evictions: u64,
    refused: u64,
    removed_bad: u64,
    dropped: u64,
}

impl SkyTableFileCounters {
    /// Files held.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.entries
    }

    /// Their bytes, never above [`SkyTableFileCounters::budget`].
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The byte budget.
    #[must_use]
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Galaxies whose tables were read from a file.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Galaxies whose tables were not, for want of a file, or of a good one with the key.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Tables written.
    #[must_use]
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Files removed to make room for others.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    /// Tables not written for being larger than the whole budget, or for holding a NaN.
    #[must_use]
    pub fn refused(&self) -> u64 {
        self.refused
    }

    /// Files removed on reading for being short, torn or unreadable, or once their words did not
    /// read back.
    #[must_use]
    pub fn removed_bad(&self) -> u64 {
        self.removed_bad
    }

    /// Tables not written because the store's queue was full.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// A file's key: its name, the bytes it must hold to be read, and whether the tables it holds carry
/// the features' gas index.
#[derive(Debug)]
struct FileKey {
    name: String,
    bytes: [u8; KEY_BYTES],
    gas: TablesGas,
}

impl FileKey {
    /// The key of the tables of the galaxy `key` names, built from `source` with `gas`.
    #[must_use]
    fn of(key: GalaxyKey, source: SkyTablesSource, gas: TablesGas) -> Self {
        let source = match source {
            SkyTablesSource::Galaxy => 0_u8,
            SkyTablesSource::Dark => 1,
        };
        let indexed = match gas {
            TablesGas::Smooth => 0_u8,
            TablesGas::Indexed => 1,
        };
        let mut bytes = [0; KEY_BYTES];
        let fields = [
            &PROTOCOL_VERSION.to_le_bytes()[..],
            &GENERATOR_VERSION.get().to_le_bytes(),
            &key.seed().to_le_bytes(),
            &key.generator_version().get().to_le_bytes(),
            &[source, indexed],
            &LuminosityTables::FORMAT.to_le_bytes(),
            &FeatureGasIndex::FORMAT.to_le_bytes(),
        ];
        let mut at = 0;
        for field in fields {
            bytes[at..at + field.len()].copy_from_slice(field);
            at += field.len();
        }
        debug_assert_eq!(at, KEY_BYTES, "the key's fields fill it");
        Self::named(bytes, gas)
    }

    /// The key of `bytes`, named by their hash.
    #[must_use]
    fn named(bytes: [u8; KEY_BYTES], gas: TablesGas) -> Self {
        Self {
            name: format!("{:016x}.{EXTENSION}", fnv1a(&bytes)),
            bytes,
            gas,
        }
    }
}

/// What the store's thread is asked.
enum Op {
    /// The file whose frame holds for the key, if one does.
    Get {
        key: FileKey,
        reply: oneshot::Sender<Option<TablesFile>>,
    },
    /// Removes the file of the key, whose words did not read back.
    Bad { key: FileKey },
    /// Writes the tables for the key, or marks its file used if one holds them already.
    Keep {
        key: FileKey,
        tables: Arc<SkyTables>,
    },
}

impl std::fmt::Debug for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get { key, .. } => f.debug_struct("Get").field("key", &key.name).finish(),
            Self::Bad { key } => f.debug_struct("Bad").field("key", &key.name).finish(),
            Self::Keep { key, .. } => f.debug_struct("Keep").field("key", &key.name).finish(),
        }
    }
}

/// A file whose frame holds for its key: the words of a galaxy's tables, made whole on the pool
/// ([`tables`](Self::tables)).
pub(crate) struct TablesFile {
    contents: Vec<u8>,
    gas: TablesGas,
}

impl std::fmt::Debug for TablesFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TablesFile")
            .field("bytes", &self.contents.len())
            .field("gas", &self.gas)
            .finish()
    }
}

impl TablesFile {
    /// The tables of `galaxy` the file's words hold, made whole with its envelope and offset
    /// bounds, the index read too where the key's tables carry one; or `None` if the words do not
    /// read back whole. It computes the sim's values, so it runs as a pool job.
    #[must_use]
    pub(crate) fn tables(&self, galaxy: &Galaxy) -> Option<SkyTables> {
        let words = &self.contents[MAGIC.len() + KEY_BYTES..self.contents.len() - 8];
        let mut reader = Reader(words);
        let luminosity = LuminosityTables::read_words(&mut reader, galaxy)?;
        let index = match self.gas {
            TablesGas::Smooth => None,
            TablesGas::Indexed => Some(FeatureGasIndex::read_words(&mut reader, galaxy)?),
        };
        if reader.words_left() > 0 {
            return None;
        }
        let tables = SkyTables::new(luminosity, galaxy);
        Some(match index {
            Some(index) => tables.with_features(index),
            None => tables,
        })
    }
}

/// The sky tables' files, or none where they are off.
#[derive(Debug)]
pub(crate) struct SkyTableFiles {
    store: Option<Store>,
}

/// The store's queue, its thread, and what it says of itself.
#[derive(Debug)]
struct Store {
    /// Taken by [`SkyTableFiles::close`], which ends the thread.
    queue: Mutex<Option<mpsc::Sender<Op>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    counters: Arc<Mutex<SkyTableFileCounters>>,
}

impl SkyTableFiles {
    /// The files in `dir`, at most `budget` bytes of them; none where the budget is 0.
    ///
    /// Nothing is read or written until the first lookup or tables, so that a server that builds
    /// no tables never creates the directory. A thread that cannot be started leaves the files
    /// off, which is logged.
    #[must_use]
    pub(crate) fn new(dir: PathBuf, budget: u64) -> Self {
        if budget == 0 {
            return Self::off();
        }
        let counters = Arc::new(Mutex::new(SkyTableFileCounters {
            budget,
            ..SkyTableFileCounters::default()
        }));
        let (queue, ops) = mpsc::channel(SKY_TABLE_FILE_QUEUE);
        let shared = Arc::clone(&counters);
        let thread = thread::Builder::new()
            .name("hyperion-sky-table-files".to_owned())
            .spawn(move || serve(&dir, budget, ops, &shared));
        match thread {
            Ok(thread) => Self {
                store: Some(Store {
                    queue: Mutex::new(Some(queue)),
                    thread: Mutex::new(Some(thread)),
                    counters,
                }),
            },
            Err(error) => {
                tracing::warn!(
                    %error,
                    "the sky tables' file thread did not start; none is kept on disk"
                );
                Self::off()
            }
        }
    }

    /// No files: nothing is written or read.
    #[must_use]
    pub(crate) fn off() -> Self {
        Self { store: None }
    }

    /// The file of the tables of the galaxy `key` names, built from `source` with `gas`, if one
    /// whose frame holds is kept: read, its checksum and key checked, and counted a hit. Its words
    /// are made whole by [`TablesFile::tables`] on the pool, and reported to
    /// [`remove_bad`](Self::remove_bad) if they do not read back.
    pub(crate) async fn get(
        &self,
        key: GalaxyKey,
        (source, gas): (SkyTablesSource, TablesGas),
    ) -> Option<TablesFile> {
        let queue = self.store.as_ref().and_then(Store::sender)?;
        let (reply, found) = oneshot::channel();
        let op = Op::Get {
            key: FileKey::of(key, source, gas),
            reply,
        };
        // A queue closed by the shutdown finds nothing, as the files' being off does.
        queue.send(op).await.ok()?;
        // The thread answers every lookup it takes; it drops one unanswered only by panicking, and
        // the tables are then built, as for any miss.
        found.await.ok().flatten()
    }

    /// Removes the file of the tables of the galaxy `key` names, built from `source` with `gas`,
    /// whose words [`get`](Self::get) handed back but did not read back as tables
    /// ([`TablesFile::tables`]): its hit is counted a miss, and the file removed as bad. It does
    /// not block; a queue with no room leaves the file, which is read and refused again.
    pub(crate) fn remove_bad(&self, key: GalaxyKey, (source, gas): (SkyTablesSource, TablesGas)) {
        if let Some(queue) = self.store.as_ref().and_then(Store::sender) {
            // A full queue leaves the file until the next read finds it again, and a closed one
            // ends with the server.
            let _ = queue.try_send(Op::Bad {
                key: FileKey::of(key, source, gas),
            });
        }
    }

    /// Writes `tables`, those of the galaxy `key` names, built from `source` with `gas`, unless the
    /// queue is full, when it is counted and dropped.
    ///
    /// It does not block; with the files off it does nothing.
    pub(crate) fn keep(
        &self,
        key: GalaxyKey,
        (source, gas): (SkyTablesSource, TablesGas),
        tables: &Arc<SkyTables>,
    ) {
        let Some(store) = &self.store else {
            return;
        };
        let Some(queue) = store.sender() else {
            return;
        };
        let op = Op::Keep {
            key: FileKey::of(key, source, gas),
            tables: Arc::clone(tables),
        };
        match queue.try_send(op) {
            // Closed by the shutdown, after which nothing more is kept.
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
            Err(mpsc::error::TrySendError::Full(op)) => {
                tracing::debug!(
                    ?op,
                    "the sky tables' file queue is full; one is not written"
                );
                lock(&store.counters).dropped += 1;
            }
        }
    }

    /// Ends the store's thread once it has done what is queued, and waits for it, off the runtime.
    /// A lookup or tables after it find nothing and write nothing.
    pub(crate) async fn close(&self) {
        let Some(store) = &self.store else {
            return;
        };
        drop(lock(&store.queue).take());
        let Some(thread) = lock(&store.thread).take() else {
            return;
        };
        match tokio::task::spawn_blocking(move || thread.join()).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => tracing::error!("the sky tables' file thread panicked"),
            Err(error) => {
                tracing::error!(%error, "the wait for the sky tables' file thread failed");
            }
        }
    }

    /// The files' size and use.
    #[must_use]
    pub(crate) fn counters(&self) -> SkyTableFileCounters {
        self.store
            .as_ref()
            .map_or_else(SkyTableFileCounters::default, |store| {
                *lock(&store.counters)
            })
    }
}

impl Store {
    /// The queue, unless the store is closed.
    #[must_use]
    fn sender(&self) -> Option<mpsc::Sender<Op>> {
        lock(&self.queue).clone()
    }
}

/// Locks `mutex`, whose data no panic can leave half-written: each holder only reads or assigns.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The store's thread: takes each op in turn until the queue closes, scanning `dir` first.
fn serve(
    dir: &Path,
    budget: u64,
    mut ops: mpsc::Receiver<Op>,
    counters: &Mutex<SkyTableFileCounters>,
) {
    let mut files: Option<Files> = None;
    while let Some(op) = ops.blocking_recv() {
        let files = files.get_or_insert_with(|| Files::scan(dir, budget));
        match op {
            Op::Get { key, reply } => {
                let found = files.read(&key);
                // Before the reply, so that whoever has it reads counters that count it.
                publish(&files.counters, counters);
                // The tables' build may have been given up, and then nobody wants them.
                let _ = reply.send(found);
            }
            Op::Bad { key } => {
                files.remove_bad(&key);
                publish(&files.counters, counters);
            }
            Op::Keep { key, tables } => {
                files.write(&key, &tables);
                publish(&files.counters, counters);
            }
        }
    }
}

/// Copies the store's `counters` to the `shared` ones the server reads, but for the tables
/// dropped, which `keep` counts there itself while the thread is busy.
fn publish(counters: &SkyTableFileCounters, shared: &Mutex<SkyTableFileCounters>) {
    let mut shared = lock(shared);
    *shared = SkyTableFileCounters {
        dropped: shared.dropped,
        ..*counters
    };
}

/// A file held: its bytes, and when it was last used, in the order of uses.
#[derive(Debug, Clone, Copy)]
struct Held {
    bytes: u64,
    used: u64,
}

/// The files in the directory, as the store's thread keeps them.
#[derive(Debug)]
struct Files {
    dir: PathBuf,
    held: HashMap<String, Held>,
    /// The names by when they were last used, the oldest first.
    by_use: BTreeMap<u64, String>,
    next_use: u64,
    /// Numbers each temporary file, so that two writes of one key never share one.
    next_temp: u64,
    counters: SkyTableFileCounters,
}

/// Why a file was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unread {
    /// It holds another key: a name's collision, or a file from another build.
    OtherKey,
    /// It is short, torn, or holds what does not read back.
    Bad(&'static str),
}

impl Files {
    /// The files `dir` holds, in `budget` bytes, the least recently modified removed first until
    /// they fit, and any temporary file a write left removed. A directory that does not exist, or
    /// cannot be read, holds none.
    #[must_use]
    fn scan(dir: &Path, budget: u64) -> Self {
        let mut files = Self {
            dir: dir.to_path_buf(),
            held: HashMap::new(),
            by_use: BTreeMap::new(),
            next_use: 0,
            next_temp: 0,
            counters: SkyTableFileCounters {
                budget,
                ..SkyTableFileCounters::default()
            },
        };
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return files,
            Err(error) => {
                tracing::warn!(
                    %error,
                    dir = %dir.display(),
                    "the sky tables' files cannot be listed"
                );
                return files;
            }
        };
        let mut found = Vec::new();
        // An entry that cannot be read, or whose metadata cannot, is not held: it costs only its
        // bytes, outside the budget, until a scan reads it, and a lookup of its key is a miss.
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            match path.extension().and_then(|extension| extension.to_str()) {
                Some(EXTENSION) => {
                    if let Ok(metadata) = entry.metadata()
                        && metadata.is_file()
                    {
                        // A file of no known time is the first removed, its use unknown.
                        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                        found.push((modified, name.to_owned(), metadata.len()));
                    }
                }
                Some(TEMP_EXTENSION) => remove(&path),
                _ => {}
            }
        }
        found.sort();
        for (_, name, bytes) in found {
            files.hold(name, bytes);
        }
        files.evict_to(budget);
        files
    }

    /// Holds `name`, of `bytes`, as the most recently used.
    fn hold(&mut self, name: String, bytes: u64) {
        let used = self.next_use;
        self.next_use += 1;
        if let Some(was) = self.held.get(&name).copied() {
            self.by_use.remove(&was.used);
            self.counters.bytes -= was.bytes;
        } else {
            self.counters.entries += 1;
        }
        self.held.insert(name.clone(), Held { bytes, used });
        self.by_use.insert(used, name);
        self.counters.bytes += bytes;
    }

    /// Forgets `name`, whose file is gone or is being removed.
    fn forget(&mut self, name: &str) {
        if let Some(was) = self.held.remove(name) {
            self.by_use.remove(&was.used);
            self.counters.bytes -= was.bytes;
            self.counters.entries -= 1;
        }
    }

    /// Removes the least recently used files until the rest fit in `bytes`.
    fn evict_to(&mut self, bytes: u64) {
        while self.counters.bytes > bytes {
            let Some((_, name)) = self.by_use.pop_first() else {
                break;
            };
            remove(&self.dir.join(&name));
            if let Some(was) = self.held.remove(&name) {
                self.counters.bytes -= was.bytes;
                self.counters.entries -= 1;
            }
            self.counters.evictions += 1;
        }
    }

    /// Marks the held file `name` used, now and in its file's time.
    fn used(&mut self, name: &str) {
        let bytes = self.held[name].bytes;
        self.hold(name.to_owned(), bytes);
        touch(&self.dir.join(name));
    }

    /// The file held for `key` whose frame holds, marked as used, or `None`.
    fn read(&mut self, key: &FileKey) -> Option<TablesFile> {
        if !self.held.contains_key(&key.name) {
            self.counters.misses += 1;
            return None;
        }
        let path = self.dir.join(&key.name);
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            Err(error) => {
                tracing::debug!(%error, file = %path.display(), "a sky tables' file is gone");
                self.forget(&key.name);
                self.counters.misses += 1;
                return None;
            }
        };
        match frame(&contents, &key.bytes) {
            Ok(()) => {
                self.counters.hits += 1;
                self.used(&key.name);
                Some(TablesFile {
                    contents,
                    gas: key.gas,
                })
            }
            Err(Unread::OtherKey) => {
                self.counters.misses += 1;
                None
            }
            Err(Unread::Bad(why)) => {
                self.removed(key, why);
                None
            }
        }
    }

    /// Removes the file of `key`, whose words did not read back as tables after its frame held:
    /// its hit counted a miss.
    fn remove_bad(&mut self, key: &FileKey) {
        if self.held.contains_key(&key.name) {
            self.counters.hits = self.counters.hits.saturating_sub(1);
            self.removed(key, "tables");
        }
    }

    /// Removes the bad file of `key`, and counts it a miss.
    fn removed(&mut self, key: &FileKey, why: &'static str) {
        let path = self.dir.join(&key.name);
        tracing::warn!(
            file = %path.display(),
            why,
            "a sky tables' file is bad, and is removed"
        );
        remove(&path);
        self.forget(&key.name);
        self.counters.removed_bad += 1;
        self.counters.misses += 1;
    }

    /// Writes `tables` for `key`, then removes the least recently used files until all fit; a
    /// file held for the key already, whose tables are the same build's, is only marked used, and
    /// tables larger than the whole budget, or holding a NaN, are refused.
    fn write(&mut self, key: &FileKey, tables: &SkyTables) {
        if self.held.contains_key(&key.name) {
            self.used(&key.name);
            return;
        }
        let Some(contents) = encode(&key.bytes, tables) else {
            tracing::warn!(file = %key.name, "a galaxy's sky tables hold a NaN, and are not written");
            self.counters.refused += 1;
            return;
        };
        let bytes = u64::try_from(contents.len()).unwrap_or(u64::MAX);
        if bytes > self.counters.budget {
            self.counters.refused += 1;
            return;
        }
        let temp = self
            .dir
            .join(format!("{}.{}.{TEMP_EXTENSION}", key.name, self.next_temp));
        self.next_temp += 1;
        let target = self.dir.join(&key.name);
        let written = fs::create_dir_all(&self.dir)
            .and_then(|()| fs::write(&temp, &contents))
            .and_then(|()| fs::rename(&temp, &target));
        if let Err(error) = written {
            tracing::warn!(
                %error,
                file = %target.display(),
                "a galaxy's sky tables could not be written"
            );
            remove(&temp);
            return;
        }
        self.hold(key.name.clone(), bytes);
        self.counters.written += 1;
        self.evict_to(self.counters.budget);
    }
}

/// Removes the file at `path`, logging a failure: a file left behind only costs its bytes until a
/// later scan or write finds it again.
fn remove(path: &Path) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        tracing::debug!(%error, file = %path.display(), "a sky tables' file could not be removed");
    }
}

/// Marks the file at `path` as modified now, so that its use orders it at the next start.
fn touch(path: &Path) {
    let touched = fs::File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(SystemTime::now()));
    if let Err(error) = touched {
        // Only the order of removal after a restart depends on it.
        tracing::debug!(
            %error,
            file = %path.display(),
            "a sky tables' file's time could not be set"
        );
    }
}

/// A file's bytes as they are written: each word eight little-endian bytes, a float's its bits;
/// and whether a float was a NaN.
struct Writer {
    bytes: Vec<u8>,
    nan: bool,
}

impl WordSink for Writer {
    fn word(&mut self, word: u64) {
        self.bytes.extend(word.to_le_bytes());
    }

    fn float(&mut self, value: f64) {
        self.nan |= value.is_nan();
        self.bytes.extend(value.to_le_bytes());
    }
}

/// A file's words as [`Writer`] wrote them, read from the first.
struct Reader<'a>(&'a [u8]);

impl WordSource for Reader<'_> {
    fn word(&mut self) -> Option<u64> {
        let (word, rest) = self.0.split_first_chunk::<8>()?;
        self.0 = rest;
        Some(u64::from_le_bytes(*word))
    }

    fn float(&mut self) -> Option<f64> {
        let (value, rest) = self.0.split_first_chunk::<8>()?;
        self.0 = rest;
        Some(f64::from_le_bytes(*value))
    }

    fn words_left(&self) -> usize {
        self.0.len() / 8
    }
}

/// A file's contents for `tables` under `key`: the magic, the key, the tables' words, the index's
/// where they carry one, and their checksum; or `None` if the tables hold a NaN.
#[must_use]
fn encode(key: &[u8; KEY_BYTES], tables: &SkyTables) -> Option<Vec<u8>> {
    let index = tables.features();
    let words = tables.luminosity().heap_bytes() + index.map_or(0, FeatureGasIndex::heap_bytes);
    let mut contents = Writer {
        bytes: Vec::with_capacity(FRAME_BYTES + words),
        nan: false,
    };
    contents.bytes.extend(MAGIC);
    contents.bytes.extend(key);
    tables.luminosity().write_words(&mut contents);
    if let Some(index) = index {
        index.write_words(&mut contents);
    }
    if contents.nan {
        return None;
    }
    let mut contents = contents.bytes;
    let check = fnv1a(&contents);
    contents.extend(check.to_le_bytes());
    Some(contents)
}

/// Whether a file's `contents` hold `key` whole: their length, magic, checksum and key, and words
/// of eight bytes each.
fn frame(contents: &[u8], key: &[u8; KEY_BYTES]) -> Result<(), Unread> {
    if contents.len() < FRAME_BYTES {
        return Err(Unread::Bad("length"));
    }
    let (body, check) = contents.split_at(contents.len() - 8);
    if body[..MAGIC.len()] != MAGIC {
        return Err(Unread::Bad("magic"));
    }
    if fnv1a(body).to_le_bytes() != check {
        return Err(Unread::Bad("checksum"));
    }
    let (held, words) = body[MAGIC.len()..].split_at(KEY_BYTES);
    if held != key {
        return Err(Unread::OtherKey);
    }
    if !words.len().is_multiple_of(8) {
        return Err(Unread::Bad("length"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;
    use std::time::Duration;

    use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
    use hyperion_sim::{GeneratorVersion, Seed};
    use tempfile::TempDir;

    use super::*;
    use crate::testing::WAIT;

    const SMOOTH: (SkyTablesSource, TablesGas) = (SkyTablesSource::Galaxy, TablesGas::Smooth);

    fn key() -> GalaxyKey {
        GalaxyKey::new(0x4d2, GENERATOR_VERSION)
    }

    /// The galaxy of [`key`], built once for the process.
    fn galaxy() -> &'static Arc<Galaxy> {
        static GALAXY: OnceLock<Arc<Galaxy>> = OnceLock::new();
        GALAXY.get_or_init(|| Arc::new(Galaxy::new(Seed::new(key().seed()))))
    }

    /// The galaxy's tables that hold no star, laid out as its own are: a store's tests need no
    /// track.
    fn tables() -> Arc<SkyTables> {
        Arc::new(SkyTables::new(LuminosityTables::dark(galaxy()), galaxy()))
    }

    fn store(dir: &TempDir, budget: u64) -> SkyTableFiles {
        SkyTableFiles::new(dir.path().join(SKY_TABLE_FILES_DIR), budget)
    }

    /// The tables a file of `files` holds for `key` and `kind`, made whole as the service makes
    /// them, words that do not read back reported.
    async fn get(
        files: &SkyTableFiles,
        key: GalaxyKey,
        kind: (SkyTablesSource, TablesGas),
    ) -> Option<SkyTables> {
        let file = tokio::time::timeout(WAIT, files.get(key, kind))
            .await
            .unwrap()?;
        let tables = file.tables(galaxy());
        if tables.is_none() {
            files.remove_bad(key, kind);
        }
        tables
    }

    /// Closes `files`, within the tests' wait.
    async fn closed(files: &SkyTableFiles) {
        tokio::time::timeout(WAIT, files.close()).await.unwrap();
    }

    /// Keeps `tables` for `key` and waits until the store has written them, by a lookup behind
    /// them.
    async fn kept(
        files: &SkyTableFiles,
        key: GalaxyKey,
        kind: (SkyTablesSource, TablesGas),
        tables: &Arc<SkyTables>,
    ) {
        files.keep(key, kind, tables);
        get(files, GalaxyKey::new(0, GeneratorVersion::new(1)), SMOOTH).await;
    }

    /// Every bit the tables hold, in a file's words: what a file is made of, and so the same
    /// bytes for tables read back exactly. Some 70 MB, so the tests compare them whole rather than
    /// print them.
    fn spelt(tables: &SkyTables) -> Vec<u8> {
        encode(&[0; KEY_BYTES], tables).expect("no NaN")
    }

    /// The bytes of a file holding `tables`.
    fn file_bytes(tables: &SkyTables) -> u64 {
        u64::try_from(spelt(tables).len()).unwrap()
    }

    /// The files in the store's directory.
    fn listed(dir: &TempDir) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir.path().join(SKY_TABLE_FILES_DIR))
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Tables kept are read back by a new store, as a restarted server's, bit for bit, owning the
    /// heap the kept ones do; the store writes nothing until it keeps some, and tables kept again
    /// are only marked used.
    #[tokio::test]
    async fn kept_tables_are_read_back_after_a_restart_bit_for_bit() {
        let dir = TempDir::new().unwrap();
        let files = store(&dir, 1 << 30);
        assert!(get(&files, key(), SMOOTH).await.is_none());
        assert!(
            !dir.path().join(SKY_TABLE_FILES_DIR).exists(),
            "nothing written"
        );
        let sent = tables();
        kept(&files, key(), SMOOTH, &sent).await;
        kept(&files, key(), SMOOTH, &sent).await;
        assert_eq!(files.counters().written(), 1);
        closed(&files).await;
        assert_eq!(listed(&dir).len(), 1, "{:?}", listed(&dir));

        let restarted = store(&dir, 1 << 30);
        let read = get(&restarted, key(), SMOOTH).await.expect("the file kept");
        assert!(spelt(&read) == spelt(&sent), "the tables read back differ");
        assert_eq!(read.heap_bytes(), sent.heap_bytes());
        assert!(read.features().is_none());
        let counters = restarted.counters();
        assert_eq!(
            (
                counters.entries(),
                counters.hits(),
                counters.misses(),
                counters.bytes()
            ),
            (1, 1, 0, file_bytes(&sent)),
            "{counters:?}"
        );
        closed(&restarted).await;
    }

    /// Tables that carry the features' gas index are read back with it, bit for bit, its spheres
    /// and bins made again, and finding the gas the kept index finds.
    #[tokio::test]
    async fn tables_with_the_features_gas_are_read_back_with_their_index() {
        use hyperion_sim::coords::GalacticPosition;
        use hyperion_sim::galaxy::features::gas_overlay::FeatureGas;
        use hyperion_sim::galaxy::gas::modifiers::GasModifierSource;
        use hyperion_sim::units::Years;

        let indexed = (SkyTablesSource::Galaxy, TablesGas::Indexed);
        let index = FeatureGasIndex::build(galaxy(), &NoFeatureCache);
        let sent = Arc::new(
            SkyTables::new(LuminosityTables::dark(galaxy()), galaxy()).with_features(index),
        );
        let dir = TempDir::new().unwrap();
        let files = store(&dir, 1 << 30);
        kept(&files, key(), indexed, &sent).await;
        closed(&files).await;

        let restarted = store(&dir, 1 << 30);
        assert!(
            get(&restarted, key(), SMOOTH).await.is_none(),
            "not the smooth field's"
        );
        let read = get(&restarted, key(), indexed)
            .await
            .expect("the file kept");
        assert!(spelt(&read) == spelt(&sent), "the tables read back differ");
        assert_eq!(read.heap_bytes(), sent.heap_bytes());
        let (read, sent) = (read.features().unwrap(), sent.features().unwrap());
        assert!(read == sent, "the index read back differs");
        let gas = |index| {
            let mut found = Vec::new();
            FeatureGas::indexed(galaxy(), index, Years::ZERO).modifiers_near_segment(
                &GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).unwrap(),
                &GalacticPosition::from_light_years([0.0, -60_000.0, 0.0]).unwrap(),
                &mut found,
            );
            found
        };
        let found = gas(read);
        assert_ne!(found.len(), 0, "the gas along the ray");
        assert_eq!(found, gas(sent));
        closed(&restarted).await;
    }

    /// Changing any field of the key is a miss: the galaxy's seed or generator version, the
    /// tables' source or gas, the build's generator or protocol version, which lead every key, and
    /// the tables' and the index's formats, which end it.
    #[tokio::test]
    async fn a_file_is_a_miss_for_each_field_of_its_key() {
        let dir = TempDir::new().unwrap();
        let files = store(&dir, 1 << 30);
        let sent = tables();
        kept(&files, key(), SMOOTH, &sent).await;
        assert!(get(&files, key(), SMOOTH).await.is_some(), "the key itself");
        let others = [
            ("seed", GalaxyKey::new(0x4d3, GENERATOR_VERSION), SMOOTH),
            (
                "the galaxy's version",
                GalaxyKey::new(0x4d2, GeneratorVersion::new(GENERATOR_VERSION.get() - 1)),
                SMOOTH,
            ),
            (
                "the source",
                key(),
                (SkyTablesSource::Dark, TablesGas::Smooth),
            ),
            (
                "the gas",
                key(),
                (SkyTablesSource::Galaxy, TablesGas::Indexed),
            ),
        ];
        for (what, key, kind) in others {
            assert!(get(&files, key, kind).await.is_none(), "{what}");
        }
        // The four, after the lookup behind the write.
        assert_eq!(files.counters().misses(), 1 + 4);
        closed(&files).await;

        // The build's versions lead the key and the formats end it: a build at another version,
        // protocol or format names other files, and a file under this one's name that holds them
        // is not read.
        let key = FileKey::of(key(), SMOOTH.0, SMOOTH.1);
        let mut prefix = PROTOCOL_VERSION.to_le_bytes().to_vec();
        prefix.extend(GENERATOR_VERSION.get().to_le_bytes());
        assert!(key.bytes.starts_with(&prefix));
        let mut formats = LuminosityTables::FORMAT.to_le_bytes().to_vec();
        formats.extend(FeatureGasIndex::FORMAT.to_le_bytes());
        assert!(key.bytes.ends_with(&formats));
        let contents = encode(&key.bytes, &sent).expect("no NaN");
        for at in 0..KEY_BYTES {
            let mut other = key.bytes;
            other[at] ^= 1;
            assert_eq!(frame(&contents, &other), Err(Unread::OtherKey), "byte {at}");
            assert_ne!(FileKey::named(other, key.gas).name, key.name, "byte {at}");
        }
    }

    /// How a test spoils a file's bytes.
    type Spoil = fn(&mut Vec<u8>);

    /// A short, torn or garbled file is a miss and is removed, and never an error; so is one whose
    /// words do not read back as tables, or hold more than them, under a good checksum.
    #[tokio::test]
    async fn a_bad_file_is_a_miss_and_is_removed() {
        /// Rewrites a file's checksum over what it now holds.
        fn checked(file: &mut Vec<u8>) {
            file.truncate(file.len() - 8);
            let check = fnv1a(file);
            file.extend(check.to_le_bytes());
        }
        let spoil: [(&str, Spoil); 7] = [
            ("empty", |file| file.clear()),
            ("short", |file| file.truncate(file.len() / 2)),
            ("a flipped word byte", |file| {
                let at = file.len() - 20;
                file[at] ^= 0x40;
            }),
            ("a flipped magic", |file| file[0] = b'X'),
            ("a lost checksum byte", |file| {
                file.pop();
            }),
            ("a layout too long, checked", |file| {
                // The layout's length, after the time's two words and the solar radius.
                let at = MAGIC.len() + KEY_BYTES + 3 * 8;
                let len = u64::from_le_bytes(file[at..at + 8].try_into().unwrap());
                file[at..at + 8].copy_from_slice(&(len + 1).to_le_bytes());
                checked(file);
            }),
            ("a word more, checked", |file| {
                let at = file.len() - 8;
                file.splice(at..at, [0; 8]);
                checked(file);
            }),
        ];
        let sent = tables();
        for (what, spoil) in spoil {
            let dir = TempDir::new().unwrap();
            let files = store(&dir, 1 << 30);
            kept(&files, key(), SMOOTH, &sent).await;
            let path = dir
                .path()
                .join(SKY_TABLE_FILES_DIR)
                .join(FileKey::of(key(), SMOOTH.0, SMOOTH.1).name);
            let mut file = fs::read(&path).unwrap();
            spoil(&mut file);
            fs::write(&path, &file).unwrap();
            assert!(get(&files, key(), SMOOTH).await.is_none(), "{what}");
            // A lookup behind the report of words that did not read back.
            get(&files, GalaxyKey::new(0, GeneratorVersion::new(1)), SMOOTH).await;
            assert!(!path.exists(), "{what}: removed");
            let counters = files.counters();
            assert_eq!(
                (counters.removed_bad(), counters.entries(), counters.bytes()),
                (1, 0, 0),
                "{what}: {counters:?}"
            );
            // Written again, it is read.
            kept(&files, key(), SMOOTH, &sent).await;
            assert!(get(&files, key(), SMOOTH).await.is_some(), "{what}");
            closed(&files).await;
        }
    }

    /// The files fit their budget, the least recently used removed first, in a restart's order too,
    /// which their files' times give; tables larger than the whole budget are refused, and a
    /// write's leftover is removed at start.
    #[tokio::test]
    async fn the_files_fit_their_budget_least_recently_used_first() {
        let dir = TempDir::new().unwrap();
        let sent = tables();
        let one = file_bytes(&sent);
        let galaxy_of = |seed| GalaxyKey::new(seed, GENERATOR_VERSION);
        let path = |seed| {
            dir.path()
                .join(SKY_TABLE_FILES_DIR)
                .join(FileKey::of(galaxy_of(seed), SMOOTH.0, SMOOTH.1).name)
        };
        let files = store(&dir, 2 * one + one / 2);
        for seed in 1..=2 {
            kept(&files, galaxy_of(seed), SMOOTH, &sent).await;
        }
        // The first is used again, so the third pushes out the second.
        assert!(get(&files, galaxy_of(1), SMOOTH).await.is_some());
        kept(&files, galaxy_of(3), SMOOTH, &sent).await;
        assert!(
            get(&files, galaxy_of(2), SMOOTH).await.is_none(),
            "the least recently used went"
        );
        assert!(!path(2).exists());
        let counters = files.counters();
        assert_eq!(
            (counters.entries(), counters.evictions(), counters.bytes()),
            (2, 1, 2 * one),
            "{counters:?}"
        );
        closed(&files).await;

        // A restart orders them by their files' times, the older first out.
        let at = |path: PathBuf, s| {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(s))
                .unwrap();
        };
        at(path(1), 1_000);
        at(path(3), 2_000);
        fs::write(
            dir.path().join(SKY_TABLE_FILES_DIR).join("leftover.0.tmp"),
            b"x",
        )
        .unwrap();
        let smaller = store(&dir, one);
        assert!(
            get(&smaller, galaxy_of(1), SMOOTH).await.is_none(),
            "the older went"
        );
        assert!(get(&smaller, galaxy_of(3), SMOOTH).await.is_some());
        assert_eq!(smaller.counters().evictions(), 1);
        closed(&smaller).await;
        assert_eq!(
            listed(&dir),
            [FileKey::of(galaxy_of(3), SMOOTH.0, SMOOTH.1).name],
            "the leftover removed too"
        );

        // Tables larger than the whole budget are not written.
        let small = TempDir::new().unwrap();
        let files = store(&small, one - 1);
        kept(&files, key(), SMOOTH, &sent).await;
        let counters = files.counters();
        assert_eq!(
            (counters.written(), counters.refused(), counters.entries()),
            (0, 1, 0),
            "{counters:?}"
        );
        closed(&files).await;
        assert_eq!(listed(&small), [] as [String; 0]);
    }

    /// A file's writer notes a NaN among the tables' values, whose tables are then not written
    /// ([`encode`]), and nothing else.
    #[test]
    fn a_nan_among_the_values_is_noted() {
        let mut writer = Writer {
            bytes: Vec::new(),
            nan: false,
        };
        for value in [0.0, -0.0, f64::INFINITY, f64::MIN_POSITIVE / 2.0] {
            writer.float(value);
        }
        writer.word(u64::MAX);
        assert!(!writer.nan);
        writer.float(f64::NAN);
        assert!(writer.nan);
        assert_eq!(writer.bytes.len(), 6 * 8);
    }

    /// Off, nothing is written or found; closed, nothing more is.
    #[tokio::test]
    async fn off_or_closed_nothing_is_kept() {
        let dir = TempDir::new().unwrap();
        let sent = tables();
        let off = store(&dir, 0);
        off.keep(key(), SMOOTH, &sent);
        assert!(get(&off, key(), SMOOTH).await.is_none());
        assert_eq!(off.counters(), SkyTableFileCounters::default());
        closed(&off).await;
        assert!(!dir.path().join(SKY_TABLE_FILES_DIR).exists());

        let files = store(&dir, 1 << 30);
        closed(&files).await;
        files.keep(key(), SMOOTH, &sent);
        assert!(get(&files, key(), SMOOTH).await.is_none());
        assert!(!dir.path().join(SKY_TABLE_FILES_DIR).exists());
    }
}
