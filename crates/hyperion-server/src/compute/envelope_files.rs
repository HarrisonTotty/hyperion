//! The streams' tube envelopes on disk, under the data directory (plan 10, P10.T10.b;
//! perf-range-envelopes-on-disk): a galaxy whose list is built again after a restart keeps the
//! envelopes its tables had before, so that its first range query builds only the tables it may
//! touch.
//!
//! A [`TubeEnvelope`] comes only from its table, some 0.5 CPU-s to build, and no cheaper exact
//! bound exists (plan 10's records, "a cold query's tables"). So a galaxy's first Sun-like 50 ly
//! query builds every table whose shell it meets, 121 of them, where 10 reach it: 9.2 s on eight
//! workers once the galaxy and its list are built, and 1.2 s after a relaunch with the envelopes
//! read back (release, pod, 16 vCPU, 2026-11-05). Like the kept skies' replies, they are a cache
//! of generated data that the server may remove at any time and rebuilds on a miss.
//!
//! Each envelope is written to [`ENVELOPE_FILES_DIR`] under the data directory when its table is
//! kept, one file a stream, named by a hash of its key: the protocol's and the build's generator
//! versions, then the galaxy's key ([`GalaxyKey`]) and the stream's number, the key rules of the
//! kept skies' files (rendering plan R06, R06.T11.n), and the envelope's
//! [`FORMAT`](TubeEnvelope::FORMAT), which the sim raises when what its boxes hold changes. No
//! build ID joins them, since a generator version's tables are pinned by its goldens.
//!
//! A file holds a magic, its key, the envelope's parts ([`TubeEnvelope::boxes_ly`] and its speed
//! bound, 193 little-endian `f64`s, which [`TubeEnvelope::from_parts`] rebuilds it from bit for
//! bit) and an FNV-1a checksum of all of them, written whole under a temporary name and renamed
//! into place, never synced: a file torn by a crash fails its checksum. A file whose key differs is
//! a miss; one that is short, torn, or whose envelope does not read back is a miss too, and is
//! removed. Neither is ever an error: the table is built, as it would have been.
//!
//! The files are bounded by bytes ([`ServerConfig::kept_envelopes_disk_bytes`]: 16 MiB by
//! default, some 10,000 envelopes of 1,586 bytes, 32 galaxies of the server tests' 321 streams;
//! 0 keeps none), the least recently used removed first, by their modification times across
//! restarts. Every read, write and removal runs on the store's own thread, `hyperion-envelope-files`,
//! fed by a bounded queue ([`ENVELOPE_FILE_QUEUE`]), so none runs on the runtime or in a pool job.
//! An envelope the queue has no room for is not written, and the server's shutdown waits for
//! those queued ([`EnvelopeFiles::close`]).
//!
//! [`ServerConfig::kept_envelopes_disk_bytes`]: crate::ServerConfig::kept_envelopes_disk_bytes

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

use hyperion_protocol::PROTOCOL_VERSION;
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::galaxy::global_list::{StreamNumber, TubeEnvelope, TubeTable};
use hyperion_sim::units::KilometresPerSecond;
use tokio::sync::{mpsc, oneshot};

use super::GalaxyKey;
use crate::cache::fnv1a;
use crate::limits::ENVELOPE_FILE_QUEUE;

/// The directory under the data directory that holds the envelopes' files.
pub(crate) const ENVELOPE_FILES_DIR: &str = "envelopes";

/// What an envelope's file starts with: `HYPENV`, a zero and its format, 1.
const MAGIC: [u8; 8] = *b"HYPENV\x00\x01";

/// The extension of an envelope's file.
const EXTENSION: &str = "env";

/// The extension of a file being written, removed if found at start.
const TEMP_EXTENSION: &str = "tmp";

/// The bytes of a file's key: the protocol's and the build's generator versions, the galaxy's seed
/// and generator version, the stream's number and the envelope's format, each little-endian.
const KEY_BYTES: usize = 4 + 4 + 8 + 4 + 2 + 4;

/// The bytes of an envelope's parts: each box's six coordinates, then the speed bound.
const ENVELOPE_BYTES: usize = 8 * (6 * TubeEnvelope::BOXES + 1);

/// The bytes of a file: the magic, the key, the envelope's parts and the checksum.
const FILE_BYTES: usize = MAGIC.len() + KEY_BYTES + ENVELOPE_BYTES + 8;

/// The envelopes' files' size and use: all zeros where they are off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EnvelopeFileCounters {
    entries: usize,
    bytes: u64,
    budget: u64,
    hits: u64,
    misses: u64,
    written: u64,
    evictions: u64,
    removed_bad: u64,
    dropped: u64,
}

impl EnvelopeFileCounters {
    /// Files held.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.entries
    }

    /// Their bytes, never above [`EnvelopeFileCounters::budget`].
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The byte budget.
    #[must_use]
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Streams whose envelope was read from a file.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Streams whose envelope was not, for want of a file, or of a good one with the key.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Envelopes written.
    #[must_use]
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Files removed to make room for others.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    /// Files removed on reading for being short, torn or unreadable.
    #[must_use]
    pub fn removed_bad(&self) -> u64 {
        self.removed_bad
    }

    /// Envelopes not written because the store's queue was full.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// A file's key: its name, and the bytes it must hold to be read.
#[derive(Debug)]
struct FileKey {
    name: String,
    bytes: [u8; KEY_BYTES],
}

impl FileKey {
    /// The key of stream `number`'s envelope in the galaxy `key` names.
    #[must_use]
    fn of(key: GalaxyKey, number: StreamNumber) -> Self {
        let mut bytes = [0; KEY_BYTES];
        let fields = [
            &PROTOCOL_VERSION.to_le_bytes()[..],
            &GENERATOR_VERSION.get().to_le_bytes(),
            &key.seed().to_le_bytes(),
            &key.generator_version().get().to_le_bytes(),
            &number.get().to_le_bytes(),
            &TubeEnvelope::FORMAT.to_le_bytes(),
        ];
        let mut at = 0;
        for field in fields {
            bytes[at..at + field.len()].copy_from_slice(field);
            at += field.len();
        }
        debug_assert_eq!(at, KEY_BYTES, "the key's fields fill it");
        Self::named(bytes)
    }

    /// The key of `bytes`, named by their hash.
    #[must_use]
    fn named(bytes: [u8; KEY_BYTES]) -> Self {
        Self {
            name: format!("{:016x}.{EXTENSION}", fnv1a(&bytes)),
            bytes,
        }
    }
}

/// What the store's thread is asked.
enum Op {
    /// The envelopes good files hold, of the streams keyed.
    Get {
        keys: Vec<(StreamNumber, FileKey)>,
        reply: oneshot::Sender<Vec<(StreamNumber, TubeEnvelope)>>,
    },
    /// Writes the envelope for the key, or marks its file used if one holds it already.
    Keep {
        key: FileKey,
        envelope: Box<TubeEnvelope>,
    },
}

impl std::fmt::Debug for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get { keys, .. } => f.debug_struct("Get").field("keys", &keys.len()).finish(),
            Self::Keep { key, .. } => f.debug_struct("Keep").field("key", &key.name).finish(),
        }
    }
}

/// The envelopes' files, or none where they are off.
#[derive(Debug)]
pub(crate) struct EnvelopeFiles {
    store: Option<Store>,
}

/// The store's queue, its thread, and what it says of itself.
#[derive(Debug)]
struct Store {
    /// Taken by [`EnvelopeFiles::close`], which ends the thread.
    queue: Mutex<Option<mpsc::Sender<Op>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    counters: Arc<Mutex<EnvelopeFileCounters>>,
}

impl EnvelopeFiles {
    /// The files in `dir`, at most `budget` bytes of them; none where the budget is 0.
    ///
    /// Nothing is read or written until the first lookup or envelope, so that a server that builds
    /// no table never creates the directory. A thread that cannot be started leaves the files
    /// off, which is logged.
    #[must_use]
    pub(crate) fn new(dir: PathBuf, budget: u64) -> Self {
        if budget == 0 {
            return Self::off();
        }
        let counters = Arc::new(Mutex::new(EnvelopeFileCounters {
            budget,
            ..EnvelopeFileCounters::default()
        }));
        let (queue, ops) = mpsc::channel(ENVELOPE_FILE_QUEUE);
        let shared = Arc::clone(&counters);
        let thread = thread::Builder::new()
            .name("hyperion-envelope-files".to_owned())
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
                    "the envelopes' file thread did not start; none is kept on disk"
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

    /// The envelopes good files hold of `streams`, streams of the galaxy `key` names, in the order
    /// asked; a stream no good file holds is left out.
    pub(crate) async fn get(
        &self,
        key: GalaxyKey,
        streams: impl IntoIterator<Item = StreamNumber>,
    ) -> Vec<(StreamNumber, TubeEnvelope)> {
        let Some(queue) = self.store.as_ref().and_then(Store::sender) else {
            return Vec::new();
        };
        let keys = streams
            .into_iter()
            .map(|number| (number, FileKey::of(key, number)))
            .collect();
        let (reply, found) = oneshot::channel();
        // A queue closed by the shutdown finds nothing, as the files' being off does.
        if queue.send(Op::Get { keys, reply }).await.is_err() {
            return Vec::new();
        }
        // The thread answers every lookup it takes; it drops one unanswered only by panicking, and
        // the tables are then built, as for any miss.
        found.await.unwrap_or_default()
    }

    /// Writes the envelope of `table`, a stream's of the galaxy `key` names, unless the queue is
    /// full, when it is counted and dropped.
    ///
    /// It does not block, so a pool job may call it; with the files off it does nothing, not even
    /// the envelope.
    pub(crate) fn keep(&self, key: GalaxyKey, table: &TubeTable) {
        self.keep_with(key, table.stream(), || TubeEnvelope::of(table));
    }

    /// [`keep`](Self::keep), of stream `number`'s envelope as `envelope` makes it, only once the
    /// store is found open.
    fn keep_with(
        &self,
        key: GalaxyKey,
        number: StreamNumber,
        envelope: impl FnOnce() -> TubeEnvelope,
    ) {
        let Some(store) = &self.store else {
            return;
        };
        let Some(queue) = store.sender() else {
            return;
        };
        let op = Op::Keep {
            key: FileKey::of(key, number),
            envelope: Box::new(envelope()),
        };
        match queue.try_send(op) {
            // Closed by the shutdown, after which nothing more is kept.
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
            Err(mpsc::error::TrySendError::Full(op)) => {
                tracing::debug!(?op, "the envelopes' file queue is full; one is not written");
                lock(&store.counters).dropped += 1;
            }
        }
    }

    /// Ends the store's thread once it has done what is queued, and waits for it, off the runtime.
    /// A lookup or envelope after it finds nothing and writes nothing.
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
            Ok(Err(_)) => tracing::error!("the envelopes' file thread panicked"),
            Err(error) => {
                tracing::error!(%error, "the wait for the envelopes' file thread failed");
            }
        }
    }

    /// The files' size and use.
    #[must_use]
    pub(crate) fn counters(&self) -> EnvelopeFileCounters {
        self.store
            .as_ref()
            .map_or_else(EnvelopeFileCounters::default, |store| {
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
    counters: &Mutex<EnvelopeFileCounters>,
) {
    let mut files: Option<Files> = None;
    while let Some(op) = ops.blocking_recv() {
        let files = files.get_or_insert_with(|| Files::scan(dir, budget));
        match op {
            Op::Get { keys, reply } => {
                let found = keys
                    .into_iter()
                    .filter_map(|(number, key)| Some((number, files.read(&key)?)))
                    .collect();
                // Before the reply, so that whoever has it reads counters that count it.
                publish(&files.counters, counters);
                // The list's build may have been given up, and then nobody wants them.
                let _ = reply.send(found);
            }
            Op::Keep { key, envelope } => {
                files.write(&key, &envelope);
                publish(&files.counters, counters);
            }
        }
    }
}

/// Copies the store's `counters` to the `shared` ones the server reads, but for the envelopes
/// dropped, which `keep` counts there itself while the thread is busy.
fn publish(counters: &EnvelopeFileCounters, shared: &Mutex<EnvelopeFileCounters>) {
    let mut shared = lock(shared);
    *shared = EnvelopeFileCounters {
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
    counters: EnvelopeFileCounters,
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
            counters: EnvelopeFileCounters {
                budget,
                ..EnvelopeFileCounters::default()
            },
        };
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return files,
            Err(error) => {
                tracing::warn!(
                    %error,
                    dir = %dir.display(),
                    "the envelopes' files cannot be listed"
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

    /// The envelope a good file holds for `key`, marked as used, or `None`.
    fn read(&mut self, key: &FileKey) -> Option<TubeEnvelope> {
        if !self.held.contains_key(&key.name) {
            self.counters.misses += 1;
            return None;
        }
        let path = self.dir.join(&key.name);
        let read = match fs::read(&path) {
            Ok(contents) => decode(&contents, &key.bytes),
            Err(error) => {
                tracing::debug!(%error, file = %path.display(), "an envelope's file is gone");
                self.forget(&key.name);
                self.counters.misses += 1;
                return None;
            }
        };
        match read {
            Ok(envelope) => {
                self.counters.hits += 1;
                self.used(&key.name);
                Some(envelope)
            }
            Err(Unread::OtherKey) => {
                self.counters.misses += 1;
                None
            }
            Err(Unread::Bad(why)) => {
                tracing::warn!(
                    file = %path.display(),
                    why,
                    "an envelope's file is bad, and is removed"
                );
                remove(&path);
                self.forget(&key.name);
                self.counters.removed_bad += 1;
                self.counters.misses += 1;
                None
            }
        }
    }

    /// Writes `envelope` for `key`, then removes the least recently used files until all fit; a
    /// file held for the key already, whose envelope is the same table's, is only marked used.
    fn write(&mut self, key: &FileKey, envelope: &TubeEnvelope) {
        if self.held.contains_key(&key.name) {
            self.used(&key.name);
            return;
        }
        let contents = encode(&key.bytes, envelope);
        let bytes = u64::try_from(contents.len()).unwrap_or(u64::MAX);
        if bytes > self.counters.budget {
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
                "a stream's envelope could not be written"
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
        tracing::debug!(%error, file = %path.display(), "an envelope's file could not be removed");
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
            "an envelope's file's time could not be set"
        );
    }
}

/// A file's contents for `envelope` under `key`: the magic, the key, the envelope's parts and
/// their checksum.
#[must_use]
fn encode(key: &[u8; KEY_BYTES], envelope: &TubeEnvelope) -> Vec<u8> {
    let mut contents = Vec::with_capacity(FILE_BYTES);
    contents.extend(MAGIC);
    contents.extend(key);
    let boxes = envelope.boxes_ly().iter().flatten().flatten().copied();
    for value in boxes.chain([envelope.member_speed_bound().value()]) {
        contents.extend(value.to_le_bytes());
    }
    let check = fnv1a(&contents);
    contents.extend(check.to_le_bytes());
    contents
}

/// The envelope in a file's `contents`, if they hold `key` and read back whole.
fn decode(contents: &[u8], key: &[u8; KEY_BYTES]) -> Result<TubeEnvelope, Unread> {
    if contents.len() != FILE_BYTES {
        return Err(Unread::Bad("length"));
    }
    let (body, check) = contents.split_at(FILE_BYTES - 8);
    if body[..MAGIC.len()] != MAGIC {
        return Err(Unread::Bad("magic"));
    }
    if fnv1a(body).to_le_bytes() != check {
        return Err(Unread::Bad("checksum"));
    }
    let (held, parts) = body[MAGIC.len()..].split_at(KEY_BYTES);
    if held != key {
        return Err(Unread::OtherKey);
    }
    let (values, _) = parts.as_chunks::<8>();
    let mut values = values.iter().map(|&value| f64::from_le_bytes(value));
    let mut boxes_ly = [[[0.0; 3]; 2]; TubeEnvelope::BOXES];
    for value in boxes_ly.iter_mut().flatten().flatten() {
        *value = values.next().ok_or(Unread::Bad("length"))?;
    }
    let speed = values.next().ok_or(Unread::Bad("length"))?;
    TubeEnvelope::from_parts(boxes_ly, KilometresPerSecond::new(speed))
        .map_err(|_| Unread::Bad("envelope"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hyperion_sim::GeneratorVersion;
    use tempfile::TempDir;

    use super::*;
    use crate::testing::WAIT;

    fn key() -> GalaxyKey {
        GalaxyKey::new(0x4d2, GENERATOR_VERSION)
    }

    fn number(n: u16) -> StreamNumber {
        StreamNumber::new(n).expect("below the limit")
    }

    /// An envelope whose boxes and speed bound are set by `seed` alone: no table is needed to
    /// store one.
    fn envelope(seed: f64) -> TubeEnvelope {
        let mut boxes_ly = [[[0.0; 3]; 2]; TubeEnvelope::BOXES];
        for (k, [low, high]) in (0_u32..).zip(&mut boxes_ly) {
            for axis in 0..3 {
                let wide = f64::from(k) + 1.0;
                low[axis] = seed - wide;
                high[axis] = seed + wide;
            }
        }
        TubeEnvelope::from_parts(boxes_ly, KilometresPerSecond::new(100.0 + seed))
            .expect("good parts")
    }

    fn store(dir: &TempDir, budget: u64) -> EnvelopeFiles {
        EnvelopeFiles::new(dir.path().join(ENVELOPE_FILES_DIR), budget)
    }

    async fn get(
        files: &EnvelopeFiles,
        key: GalaxyKey,
        streams: &[u16],
    ) -> Vec<(StreamNumber, TubeEnvelope)> {
        tokio::time::timeout(WAIT, files.get(key, streams.iter().map(|&n| number(n))))
            .await
            .unwrap()
    }

    /// Closes `files`, within the tests' wait.
    async fn closed(files: &EnvelopeFiles) {
        tokio::time::timeout(WAIT, files.close()).await.unwrap();
    }

    /// Keeps `envelope` for stream `n` and waits until the store has written it, by a lookup
    /// behind it.
    async fn kept(files: &EnvelopeFiles, key: GalaxyKey, n: u16, envelope: &TubeEnvelope) {
        files.keep_with(key, number(n), || envelope.clone());
        get(files, GalaxyKey::new(0, GeneratorVersion::new(1)), &[]).await;
    }

    /// Every value of `envelope` by its bits, the boxes' in order and then the speed bound's.
    fn spelt(envelope: &TubeEnvelope) -> Vec<u64> {
        let boxes = envelope.boxes_ly().iter().flatten().flatten().copied();
        boxes
            .chain([envelope.member_speed_bound().value()])
            .map(f64::to_bits)
            .collect()
    }

    /// The files in the store's directory.
    fn listed(dir: &TempDir) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir.path().join(ENVELOPE_FILES_DIR))
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Envelopes kept are read back by a new store, as a restarted server's, bit for bit, in the
    /// order asked, a stream with no file left out; and the store writes nothing until it keeps one.
    /// One kept again is only marked used.
    #[tokio::test]
    async fn kept_envelopes_are_read_back_after_a_restart_bit_for_bit() {
        let dir = TempDir::new().unwrap();
        let files = store(&dir, 1 << 20);
        assert_eq!(get(&files, key(), &[0, 1]).await.len(), 0);
        assert!(
            !dir.path().join(ENVELOPE_FILES_DIR).exists(),
            "nothing written"
        );
        for n in [3, 1] {
            kept(&files, key(), n, &envelope(f64::from(n))).await;
        }
        kept(&files, key(), 3, &envelope(3.0)).await;
        assert_eq!(files.counters().written(), 2);
        closed(&files).await;
        assert_eq!(listed(&dir).len(), 2, "{:?}", listed(&dir));

        let restarted = store(&dir, 1 << 20);
        let read = get(&restarted, key(), &[0, 1, 2, 3]).await;
        assert_eq!(
            read.iter().map(|(n, _)| n.get()).collect::<Vec<_>>(),
            [1, 3]
        );
        for (n, read) in &read {
            let sent = envelope(f64::from(n.get()));
            assert_eq!(spelt(read), spelt(&sent), "{}", n.get());
        }
        let counters = restarted.counters();
        assert_eq!(
            (
                counters.entries(),
                counters.hits(),
                counters.misses(),
                counters.bytes()
            ),
            (2, 2, 2, 2 * u64::try_from(FILE_BYTES).unwrap()),
            "{counters:?}"
        );
        closed(&restarted).await;
    }

    /// Changing any field of the key is a miss: the galaxy's seed or generator version, the
    /// stream's number, the build's generator or protocol version, which lead every key, and the
    /// envelope's format, which ends it.
    #[tokio::test]
    async fn a_file_is_a_miss_for_each_field_of_its_key() {
        let dir = TempDir::new().unwrap();
        let files = store(&dir, 1 << 20);
        kept(&files, key(), 5, &envelope(5.0)).await;
        assert_eq!(get(&files, key(), &[5]).await.len(), 1, "the key itself");
        let others = [
            ("seed", GalaxyKey::new(0x4d3, GENERATOR_VERSION), 5),
            (
                "the galaxy's version",
                GalaxyKey::new(0x4d2, GeneratorVersion::new(GENERATOR_VERSION.get() - 1)),
                5,
            ),
            ("stream", key(), 6),
        ];
        for (what, key, n) in others {
            assert_eq!(get(&files, key, &[n]).await.len(), 0, "{what}");
        }
        closed(&files).await;

        // The build's versions lead the key and the format ends it: a build at another version,
        // protocol or format names other files, and a file under this one's name that holds them
        // is not read.
        let key = FileKey::of(key(), number(5));
        let mut prefix = PROTOCOL_VERSION.to_le_bytes().to_vec();
        prefix.extend(GENERATOR_VERSION.get().to_le_bytes());
        assert!(key.bytes.starts_with(&prefix));
        assert!(key.bytes.ends_with(&TubeEnvelope::FORMAT.to_le_bytes()));
        for at in 0..KEY_BYTES {
            let mut other = key.bytes;
            other[at] ^= 1;
            assert_eq!(
                decode(&encode(&key.bytes, &envelope(5.0)), &other),
                Err(Unread::OtherKey),
                "byte {at}"
            );
            assert_ne!(FileKey::named(other).name, key.name, "byte {at}");
        }
    }

    /// How a test spoils a file's bytes.
    type Spoil = fn(&mut Vec<u8>);

    /// A short, torn or garbled file is a miss and is removed, and never an error; so is one whose
    /// envelope does not read back, under a good checksum.
    #[tokio::test]
    async fn a_bad_file_is_a_miss_and_is_removed() {
        let spoil: [(&str, Spoil); 6] = [
            ("empty", |file| file.clear()),
            ("short", |file| file.truncate(file.len() / 2)),
            ("a flipped envelope byte", |file| {
                let at = file.len() - 20;
                file[at] ^= 0x40;
            }),
            ("a flipped magic", |file| file[0] = b'X'),
            ("a lost checksum byte", |file| {
                file.pop();
            }),
            ("a box that is not one, checked", |file| {
                let at = MAGIC.len() + KEY_BYTES;
                file[at..at + 8].copy_from_slice(&f64::NAN.to_le_bytes());
                file.truncate(FILE_BYTES - 8);
                let check = fnv1a(file);
                file.extend(check.to_le_bytes());
            }),
        ];
        for (what, spoil) in spoil {
            let dir = TempDir::new().unwrap();
            let files = store(&dir, 1 << 20);
            kept(&files, key(), 2, &envelope(2.0)).await;
            let path = dir
                .path()
                .join(ENVELOPE_FILES_DIR)
                .join(FileKey::of(key(), number(2)).name);
            let mut file = fs::read(&path).unwrap();
            spoil(&mut file);
            fs::write(&path, &file).unwrap();
            assert_eq!(get(&files, key(), &[2]).await.len(), 0, "{what}");
            assert!(!path.exists(), "{what}: removed");
            let counters = files.counters();
            assert_eq!(
                (counters.removed_bad(), counters.entries(), counters.bytes()),
                (1, 0, 0),
                "{what}: {counters:?}"
            );
            // Written again, it is read.
            kept(&files, key(), 2, &envelope(2.0)).await;
            assert_eq!(get(&files, key(), &[2]).await.len(), 1, "{what}");
            closed(&files).await;
        }
    }

    /// The files fit their budget, the least recently used removed first, in a restart's order too,
    /// which their files' times give; a write's leftover is removed at start.
    #[tokio::test]
    async fn the_files_fit_their_budget_least_recently_used_first() {
        let dir = TempDir::new().unwrap();
        let one = u64::try_from(FILE_BYTES).unwrap();
        let path = |n| {
            dir.path()
                .join(ENVELOPE_FILES_DIR)
                .join(FileKey::of(key(), number(n)).name)
        };
        let files = store(&dir, 2 * one + one / 2);
        for n in 1..=2 {
            kept(&files, key(), n, &envelope(f64::from(n))).await;
        }
        // The first is used again, so the third pushes out the second.
        assert_eq!(get(&files, key(), &[1]).await.len(), 1);
        kept(&files, key(), 3, &envelope(3.0)).await;
        assert_eq!(
            get(&files, key(), &[2]).await.len(),
            0,
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
            dir.path().join(ENVELOPE_FILES_DIR).join("leftover.0.tmp"),
            b"x",
        )
        .unwrap();
        let smaller = store(&dir, one);
        assert_eq!(get(&smaller, key(), &[1]).await.len(), 0, "the older went");
        assert_eq!(get(&smaller, key(), &[3]).await.len(), 1);
        assert_eq!(smaller.counters().evictions(), 1);
        closed(&smaller).await;
        assert_eq!(
            listed(&dir),
            [FileKey::of(key(), number(3)).name],
            "the leftover removed too"
        );
    }

    /// Off, nothing is written or found; closed, nothing more is.
    #[tokio::test]
    async fn off_or_closed_nothing_is_kept() {
        let dir = TempDir::new().unwrap();
        let off = store(&dir, 0);
        off.keep_with(key(), number(1), || envelope(1.0));
        assert_eq!(get(&off, key(), &[1]).await.len(), 0);
        assert_eq!(off.counters(), EnvelopeFileCounters::default());
        closed(&off).await;
        assert!(!dir.path().join(ENVELOPE_FILES_DIR).exists());

        let files = store(&dir, 1 << 20);
        closed(&files).await;
        files.keep_with(key(), number(1), || envelope(1.0));
        assert_eq!(get(&files, key(), &[1]).await.len(), 0);
        assert!(!dir.path().join(ENVELOPE_FILES_DIR).exists());
    }
}
