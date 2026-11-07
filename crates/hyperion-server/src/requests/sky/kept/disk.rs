//! The kept skies' final replies on disk, under the data directory (rendering plan R06, R06.T11.n;
//! the owner's priority, 2026-11-05): a sky asked again exactly as one finished before the server
//! last stopped is answered from its file, as [`SkyReplyCache`](super::SkyReplyCache) answers one
//! finished since it started, with no sky job on the pool.
//!
//! Near the Sun a camera's sky costs some 2.5 × 10⁴ CPU-s, about an hour on an M5's four workers,
//! and a restarted server paid it all again. Each final reply is written to [`SKY_FILES_DIR`] under
//! the data directory, one file a sky, named by a hash of its key: [`SkyReplyKey`]'s fields after
//! the build's generator and protocol versions and the server's sky settings ([`SkySettings`]), so
//! that no file is read by a build or a configuration that could have answered otherwise. No build
//! ID joins them, since a generator version's output is pinned by its goldens (lean, 2026-11-05).
//!
//! The name leads with a hash of the key's arrival ([`SkyArrival`]) after the same versions and
//! settings, so that the directory's listing is the index of each arrival's files (R06.T11.p): a
//! sky that no file holds exactly is answered from a file of its arrival that holds for it
//! ([`SkyHold`]), read and checked against its own key, the latest seen and then the nearest of
//! several. A file's hold is kept in memory once read, so each is read for it once at most.
//!
//! A file holds a magic, its key, the response's JSON, the payload, and an FNV-1a checksum of all
//! of them. It is written whole under a temporary name and renamed into place, and never synced:
//! it is a cache, and a file torn by a crash fails its checksum. A file whose key differs is a
//! miss; one that is short, torn, or whose JSON or payload does not read back is a miss too, and is
//! removed. Neither is ever an error. A reply is written only if its JSON reads back to the same
//! bytes, so that a reply answered from a file is the one sent when it was made, its JSON and its
//! payload byte for byte.
//!
//! The files are bounded by bytes, `--kept-skies-disk` (256 MiB by default; 0 keeps none), the
//! least recently used removed first, by their modification times across restarts. Every read,
//! write and removal runs on the store's own thread, `hyperion-sky-files`, fed by a bounded queue
//! ([`SKY_FILE_QUEUE`]), so none runs on the runtime. A reply the queue has no room for is not
//! written, and the server's shutdown waits for those queued ([`SkyReplyFiles::close`]).

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

use axum::body::Bytes;
use hyperion_protocol::{PROTOCOL_VERSION, ResponseBody};
use hyperion_sim::GENERATOR_VERSION;
use tokio::sync::{mpsc, oneshot};

use hyperion_sim::coords::GalacticPosition as SimPosition;

use super::{HeldSky, SkyArrival, SkyHold, SkyReplyKey};
use crate::bulk::{Answer, BulkPayload};
use crate::cache::fnv1a;
use crate::compute::SkyCaps;
use crate::config::FeatureMembers;
use crate::limits::SKY_FILE_QUEUE;

/// The directory under the data directory that holds the kept skies' files.
pub(crate) const SKY_FILES_DIR: &str = "skies";

/// What a kept sky's file starts with: `HYPSKY`, a zero and its format, 1.
const MAGIC: [u8; 8] = *b"HYPSKY\x00\x01";

/// The extension of a kept sky's file.
const EXTENSION: &str = "sky";

/// The extension of a file being written, removed if found at start.
const TEMP_EXTENSION: &str = "tmp";

/// What a sky's final reply depends on besides its request and its galaxy: the server's sky
/// settings, fixed for its life but not across restarts.
///
/// The cell cache's budget and the workers are not among them: neither changes a final reply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SkySettings {
    /// How far the census looks.
    pub(crate) caps: SkyCaps,
    /// Whether the census takes plan 09's feature members, and the sightlines march the features'
    /// gas.
    pub(crate) members: FeatureMembers,
}

impl SkySettings {
    /// What every file's key starts with: the protocol's and the build's generator versions, then
    /// these settings by their `Debug` form. A form that changed with the toolchain would only
    /// make every file a miss.
    #[must_use]
    fn key_prefix(&self) -> Vec<u8> {
        let mut prefix = Vec::new();
        prefix.extend(PROTOCOL_VERSION.to_le_bytes());
        prefix.extend(GENERATOR_VERSION.get().to_le_bytes());
        put_bytes(&mut prefix, format!("{self:?}").as_bytes());
        prefix
    }
}

/// The kept skies' files' size and use: all zeros where they are off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SkyFileCounters {
    entries: usize,
    bytes: u64,
    budget: u64,
    hits: u64,
    held: u64,
    misses: u64,
    written: u64,
    evictions: u64,
    refused: u64,
    removed_bad: u64,
    dropped: u64,
}

impl SkyFileCounters {
    /// Files held.
    #[must_use]
    pub fn entries(&self) -> usize {
        self.entries
    }

    /// Their bytes, never above [`SkyFileCounters::budget`].
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The byte budget.
    #[must_use]
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Lookups answered from a file.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Of the hits, those answered from a file of the sky's arrival seen earlier or from a little
    /// way off, which holds for it (R06.T11.p).
    #[must_use]
    pub fn held(&self) -> u64 {
        self.held
    }

    /// Lookups that were not, for want of a file, or of a good one with the key.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Replies written.
    #[must_use]
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Files removed to make room for others.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    /// Replies not written: larger than the whole budget, or whose JSON does not read back.
    #[must_use]
    pub fn refused(&self) -> u64 {
        self.refused
    }

    /// Files removed on reading for being short, torn or unreadable.
    #[must_use]
    pub fn removed_bad(&self) -> u64 {
        self.removed_bad
    }

    /// Replies not written because the store's queue was full.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// A file's key: its name, and the bytes it must hold to be read.
#[derive(Debug)]
struct FileKey {
    name: String,
    bytes: Vec<u8>,
}

/// What the store's thread is asked.
enum Op {
    /// The reply kept for the key, if a good file holds it.
    Get {
        key: FileKey,
        reply: oneshot::Sender<Option<Answer>>,
    },
    /// Writes the reply for the key.
    Keep { key: FileKey, answer: Box<Answer> },
    /// The reply a good file of the ask's arrival holds that holds for it, if one does.
    Held {
        ask: Box<HeldAsk>,
        reply: oneshot::Sender<Option<HeldSky>>,
    },
}

/// A sky asked, as a lookup of its arrival's files reads it: its key and the observer it asks,
/// checked, with the prefix its files' keys lead with, and the name its arrival's files lead with.
#[derive(Debug)]
struct HeldAsk {
    key: SkyReplyKey,
    at: SimPosition,
    prefix: Vec<u8>,
    arrival: String,
}

impl HeldAsk {
    /// The file key of the sky of the ask's arrival that `hold` holds.
    fn file_key(&self, hold: &SkyHold) -> FileKey {
        file_key(&self.prefix, &hold.key(self.key.arrival()))
    }
}

/// The kept skies' files, or none where they are off.
#[derive(Debug)]
pub(crate) struct SkyReplyFiles {
    store: Option<Store>,
}

/// The store's queue, its thread, what it says of itself, and its keys' prefix.
#[derive(Debug)]
struct Store {
    /// Taken by [`SkyReplyFiles::close`], which ends the thread.
    queue: Mutex<Option<mpsc::Sender<Op>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    counters: Arc<Mutex<SkyFileCounters>>,
    prefix: Vec<u8>,
}

impl std::fmt::Debug for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get { key, .. } => f.debug_struct("Get").field("key", &key.name).finish(),
            Self::Keep { key, .. } => f.debug_struct("Keep").field("key", &key.name).finish(),
            Self::Held { ask, .. } => f
                .debug_struct("Held")
                .field("arrival", &ask.arrival)
                .finish(),
        }
    }
}

impl SkyReplyFiles {
    /// The files in `dir`, at most `budget` bytes of them, keyed under `settings`; none where the
    /// budget is 0.
    ///
    /// Nothing is read or written until the first lookup or reply, so that a server that serves
    /// no sky never creates the directory. A thread that cannot be started leaves the files off,
    /// which is logged.
    #[must_use]
    pub(crate) fn new(dir: PathBuf, budget: u64, settings: &SkySettings) -> Self {
        if budget == 0 {
            return Self::off();
        }
        let counters = Arc::new(Mutex::new(SkyFileCounters {
            budget,
            ..SkyFileCounters::default()
        }));
        let (queue, ops) = mpsc::channel(SKY_FILE_QUEUE);
        let shared = Arc::clone(&counters);
        let thread = thread::Builder::new()
            .name("hyperion-sky-files".to_owned())
            .spawn(move || serve(&dir, budget, ops, &shared));
        match thread {
            Ok(thread) => Self {
                store: Some(Store {
                    queue: Mutex::new(Some(queue)),
                    thread: Mutex::new(Some(thread)),
                    counters,
                    prefix: settings.key_prefix(),
                }),
            },
            Err(error) => {
                tracing::warn!(
                    %error,
                    "the kept skies' file thread did not start; none is kept on disk"
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

    /// The final reply a good file holds for `key`, if one does.
    pub(crate) async fn get(&self, key: &SkyReplyKey) -> Option<Answer> {
        let store = self.store.as_ref()?;
        let queue = store.sender()?;
        let (reply, found) = oneshot::channel();
        let key = store.file_key(key);
        // A queue closed by the shutdown finds nothing, as the files' being off does.
        queue.send(Op::Get { key, reply }).await.ok()?;
        // The thread answers every lookup it takes; it drops one unanswered only by panicking, and
        // the sky is then computed, as for any miss.
        found.await.ok().flatten()
    }

    /// The final reply a good file of `asked`'s arrival holds that holds for it seen from `at`, the
    /// observer it asks checked ([`SkyHold::holds`]; R06.T11.p): of several, the latest seen, then
    /// the nearest.
    pub(crate) async fn held(&self, asked: &SkyReplyKey, at: &SimPosition) -> Option<HeldSky> {
        let store = self.store.as_ref()?;
        let queue = store.sender()?;
        let (reply, found) = oneshot::channel();
        let ask = Box::new(HeldAsk {
            key: asked.clone(),
            at: *at,
            prefix: store.prefix.clone(),
            arrival: arrival_name(&store.prefix, asked.arrival()),
        });
        // As for `get`: a closed queue, or a thread that panicked, finds nothing.
        queue.send(Op::Held { ask, reply }).await.ok()?;
        found.await.ok().flatten()
    }

    /// Writes `answer`, the final reply of the sky `key`, unless the queue is full, when it is
    /// counted and dropped.
    pub(crate) fn keep(&self, key: &SkyReplyKey, answer: &Answer) {
        let Some(store) = &self.store else {
            return;
        };
        let Some(queue) = store.sender() else {
            return;
        };
        let key = store.file_key(key);
        match queue.try_send(Op::Keep {
            key,
            answer: Box::new(answer.clone()),
        }) {
            // Closed by the shutdown, after which nothing more is kept.
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
            Err(mpsc::error::TrySendError::Full(op)) => {
                tracing::debug!(
                    ?op,
                    "the kept skies' file queue is full; a reply is not written"
                );
                lock(&store.counters).dropped += 1;
            }
        }
    }

    /// Ends the store's thread once it has done what is queued, and waits for it, off the runtime.
    /// A lookup or reply after it finds nothing and writes nothing.
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
            Ok(Err(_)) => tracing::error!("the kept skies' file thread panicked"),
            Err(error) => {
                tracing::error!(%error, "the wait for the kept skies' file thread failed");
            }
        }
    }

    /// The files' size and use.
    #[must_use]
    pub(crate) fn counters(&self) -> SkyFileCounters {
        self.store
            .as_ref()
            .map_or_else(SkyFileCounters::default, |store| *lock(&store.counters))
    }
}

impl Store {
    /// The queue, unless the store is closed.
    fn sender(&self) -> Option<mpsc::Sender<Op>> {
        lock(&self.queue).clone()
    }

    /// The file key of `key` under the store's prefix.
    fn file_key(&self, key: &SkyReplyKey) -> FileKey {
        file_key(&self.prefix, key)
    }
}

/// The file key of `key` under `prefix`: the prefix and the key's bytes, named by the hash of its
/// arrival's ([`arrival_name`]) and then theirs.
fn file_key(prefix: &[u8], key: &SkyReplyKey) -> FileKey {
    let mut bytes = prefix.to_vec();
    key.write_bytes(&mut bytes);
    FileKey {
        name: format!(
            "{}{:016x}.{EXTENSION}",
            arrival_name(prefix, key.arrival()),
            fnv1a(&bytes)
        ),
        bytes,
    }
}

/// What the names of `arrival`'s files under `prefix` lead with: the hash of the prefix and the
/// arrival's bytes, and a dash.
fn arrival_name(prefix: &[u8], arrival: &SkyArrival) -> String {
    let mut bytes = prefix.to_vec();
    arrival.write_bytes(&mut bytes);
    format!("{:016x}-", fnv1a(&bytes))
}

/// Locks `mutex`, whose data no panic can leave half-written: each holder only reads or assigns.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The store's thread: takes each op in turn until the queue closes, scanning `dir` first.
fn serve(dir: &Path, budget: u64, mut ops: mpsc::Receiver<Op>, counters: &Mutex<SkyFileCounters>) {
    let mut files: Option<Files> = None;
    while let Some(op) = ops.blocking_recv() {
        let files = files.get_or_insert_with(|| Files::scan(dir, budget));
        // Each op's counters are published before its reply, so that whoever has the reply reads
        // counters that count it.
        match op {
            Op::Get { key, reply } => {
                let found = files.read(&key);
                publish(&files.counters, counters);
                // The handler may have gone, and then nobody wants the reply.
                let _ = reply.send(found);
            }
            Op::Keep { key, answer } => {
                files.write(&key, &answer);
                publish(&files.counters, counters);
            }
            Op::Held { ask, reply } => {
                let found = files.read_held(&ask);
                publish(&files.counters, counters);
                // As for `Get`, nobody may want it.
                let _ = reply.send(found);
            }
        }
    }
}

/// Copies the store's `counters` to the `shared` ones the server reads, but for the replies
/// dropped, which `keep` counts there itself while the thread is busy.
fn publish(counters: &SkyFileCounters, shared: &Mutex<SkyFileCounters>) {
    let mut shared = lock(shared);
    *shared = SkyFileCounters {
        dropped: shared.dropped,
        ..*counters
    };
}

/// A file held: its bytes, when it was last used, in the order of uses, and its sky's hold once it
/// has been read or written since the scan.
#[derive(Debug, Clone, Copy)]
struct Held {
    bytes: u64,
    used: u64,
    hold: Option<SkyHold>,
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
    counters: SkyFileCounters,
}

/// Why a file was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unread {
    /// It holds another key: a name's collision, or a file from another build or configuration.
    OtherKey,
    /// It is short, torn, or holds what does not read back.
    Bad(&'static str),
}

impl Files {
    /// The files `dir` holds, in `budget` bytes, the least recently modified removed first until
    /// they fit, and any temporary file a write left removed. A directory that does not exist, or
    /// cannot be read, holds none.
    fn scan(dir: &Path, budget: u64) -> Self {
        let mut files = Self {
            dir: dir.to_path_buf(),
            held: HashMap::new(),
            by_use: BTreeMap::new(),
            next_use: 0,
            next_temp: 0,
            counters: SkyFileCounters {
                budget,
                ..SkyFileCounters::default()
            },
        };
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return files,
            Err(error) => {
                tracing::warn!(
                    %error,
                    dir = %dir.display(),
                    "the kept skies' files cannot be listed"
                );
                return files;
            }
        };
        let mut found = Vec::new();
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
            files.hold(name, bytes, None);
        }
        files.evict_to(budget);
        files
    }

    /// Holds `name`, of `bytes`, as the most recently used, with its sky's hold `hold`, or the one
    /// known already.
    fn hold(&mut self, name: String, bytes: u64, hold: Option<SkyHold>) {
        let used = self.next_use;
        self.next_use += 1;
        let mut entry = Held { bytes, used, hold };
        if let Some(was) = self.held.get(&name).copied() {
            self.by_use.remove(&was.used);
            self.counters.bytes -= was.bytes;
            entry.hold = hold.or(was.hold);
        } else {
            self.counters.entries += 1;
        }
        self.held.insert(name.clone(), entry);
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

    /// The reply a good file holds for `key`, marked as used, or `None`.
    fn read(&mut self, key: &FileKey) -> Option<Answer> {
        if !self.held.contains_key(&key.name) {
            self.counters.misses += 1;
            return None;
        }
        let path = self.dir.join(&key.name);
        let read = match fs::read(&path) {
            Ok(bytes) => decode(bytes, &key.bytes),
            Err(error) => {
                tracing::debug!(%error, file = %path.display(), "a kept sky's file is gone");
                self.forget(&key.name);
                self.counters.misses += 1;
                return None;
            }
        };
        match read {
            Ok(answer) => {
                self.used(&key.name, SkyHold::of(&answer));
                Some(answer)
            }
            Err(Unread::OtherKey) => {
                self.counters.misses += 1;
                None
            }
            Err(Unread::Bad(why)) => {
                self.remove_bad(&path, &key.name, why);
                self.counters.misses += 1;
                None
            }
        }
    }

    /// Counts a hit of the file `name`, held, and marks it used, now and in its file's time, with its
    /// sky's hold `hold`.
    fn used(&mut self, name: &str, hold: Option<SkyHold>) {
        self.counters.hits += 1;
        let bytes = self.held[name].bytes;
        self.hold(name.to_owned(), bytes, hold);
        touch(&self.dir.join(name));
    }

    /// The reply a good file of `ask`'s arrival holds that holds for it, the latest seen and then
    /// the nearest of several, marked as used; or `None`, which counts no miss, the ask's own key's
    /// lookup having counted it.
    ///
    /// A file whose hold is not yet known is read whole for it, and checked against its own key,
    /// the ask's arrival seen where and when its reply says; one that is bad is removed, as `read`
    /// removes it.
    fn read_held(&mut self, ask: &HeldAsk) -> Option<HeldSky> {
        let mut names: Vec<String> = self
            .held
            .keys()
            .filter(|name| name.starts_with(&ask.arrival))
            .cloned()
            .collect();
        // The map's order is not the same from run to run; the choice must be.
        names.sort_unstable();
        let mut best: Option<(String, SkyHold, f64, Option<Answer>)> = None;
        for name in names {
            let Some(known) = self.held.get(&name).map(|held| held.hold) else {
                continue;
            };
            let (hold, answer) = match known {
                Some(hold) => (hold, None),
                None => match self.read_hold(&name, ask) {
                    Some((hold, answer)) => (hold, Some(answer)),
                    None => continue,
                },
            };
            let Some(offset_m) = hold.holds(ask.key.time(), &ask.at) else {
                continue;
            };
            if best.as_ref().is_none_or(|(_, was, was_offset_m, _)| {
                hold.rank(offset_m) > was.rank(*was_offset_m)
            }) {
                best = Some((name, hold, offset_m, answer));
            }
        }
        let (name, hold, offset_m, answer) = best?;
        let key = ask.file_key(&hold);
        let answer = match answer {
            Some(answer) => {
                self.used(&name, Some(hold));
                answer
            }
            // Read whole and checked against its key, as any file is; one gone bad since its hold
            // was known is a miss, which `read` counts.
            None => self.read(&key)?,
        };
        self.counters.held += 1;
        Some(HeldSky {
            key: hold.key(ask.key.arrival()),
            answer,
            offset_m,
        })
    }

    /// The hold and the reply of the file `name`, read whole and checked against its own key, the
    /// ask's arrival seen where and when its reply says; `None` for a file that is gone, holds
    /// another key or is bad, which is removed.
    fn read_hold(&mut self, name: &str, ask: &HeldAsk) -> Option<(SkyHold, Answer)> {
        let path = self.dir.join(name);
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            Err(error) => {
                tracing::debug!(%error, file = %path.display(), "a kept sky's file is gone");
                self.forget(name);
                return None;
            }
        };
        let expected = |hold: &SkyHold| {
            let key = ask.file_key(hold);
            (key.name == name).then_some(key.bytes)
        };
        match decode_held(contents, expected) {
            Ok((hold, answer)) => {
                if let Some(held) = self.held.get_mut(name) {
                    held.hold = Some(hold);
                }
                Some((hold, answer))
            }
            Err(Unread::OtherKey) => None,
            Err(Unread::Bad(why)) => {
                self.remove_bad(&path, name, why);
                None
            }
        }
    }

    /// Removes the bad file `name`, at `path`, for `why`, and forgets it.
    fn remove_bad(&mut self, path: &Path, name: &str, why: &'static str) {
        tracing::warn!(
            file = %path.display(),
            why,
            "a kept sky's file is bad, and is removed"
        );
        remove(path);
        self.forget(name);
        self.counters.removed_bad += 1;
    }

    /// Writes `answer` for `key`, then removes the least recently used files until all fit.
    fn write(&mut self, key: &FileKey, answer: &Answer) {
        let Some(contents) = encode(&key.bytes, answer) else {
            tracing::debug!(
                file = %key.name,
                "a sky's final reply does not read back, and is not written"
            );
            self.counters.refused += 1;
            return;
        };
        let bytes = u64::try_from(contents.len()).unwrap_or(u64::MAX);
        if bytes > self.counters.budget {
            tracing::debug!(
                bytes,
                budget = self.counters.budget,
                "a sky's final reply is over the kept skies' files' budget, and is not written"
            );
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
                "a sky's final reply could not be written"
            );
            remove(&temp);
            return;
        }
        self.hold(key.name.clone(), bytes, SkyHold::of(answer));
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
        tracing::debug!(%error, file = %path.display(), "a kept sky's file could not be removed");
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
            "a kept sky's file's time could not be set"
        );
    }
}

/// A file's contents for `answer` under `key`: the magic, the key, the body's JSON, the payload and
/// their checksum; `None` unless `answer` is a final sky reply with a payload whose JSON reads back
/// to the same bytes.
#[must_use]
fn encode(key: &[u8], answer: &Answer) -> Option<Vec<u8>> {
    let ResponseBody::Sky(sky) = &answer.body else {
        return None;
    };
    let payload = answer.bulk.as_ref().filter(|_| sky.is_final)?;
    // A body that does not serialise, or does not read back to the same bytes, is only not
    // written: its sky is computed again after a restart, and the caller counts it refused.
    let json = serde_json::to_vec(&answer.body).ok()?;
    let back: ResponseBody = serde_json::from_slice(&json).ok()?;
    if serde_json::to_vec(&back).ok()? != json {
        return None;
    }
    let mut contents =
        Vec::with_capacity(MAGIC.len() + key.len() + json.len() + payload.len() + 32);
    contents.extend(MAGIC);
    put_bytes(&mut contents, key);
    put_bytes(&mut contents, &json);
    put_bytes(&mut contents, payload.bytes());
    let check = fnv1a(&contents);
    contents.extend(check.to_le_bytes());
    Some(contents)
}

/// The reply in a file's `contents`, if they hold `key` and read back whole.
fn decode(contents: Vec<u8>, key: &[u8]) -> Result<Answer, Unread> {
    let (held, json, payload) = frame(contents)?;
    if held.as_ref() != key {
        return Err(Unread::OtherKey);
    }
    reply(&json, payload)
}

/// The hold and the reply in a file's `contents`, if they read back whole and hold the key that
/// `expected` gives of the reply's hold: the key it must have been written under.
fn decode_held(
    contents: Vec<u8>,
    expected: impl FnOnce(&SkyHold) -> Option<Vec<u8>>,
) -> Result<(SkyHold, Answer), Unread> {
    let (written, json, payload) = frame(contents)?;
    let answer = reply(&json, payload)?;
    let hold = SkyHold::of(&answer).ok_or(Unread::Bad("hold"))?;
    if expected(&hold).is_none_or(|key| written.as_ref() != key.as_slice()) {
        return Err(Unread::OtherKey);
    }
    Ok((hold, answer))
}

/// The key, the JSON and the payload a file's `contents` hold, if their magic, checksum and
/// lengths are good.
fn frame(contents: Vec<u8>) -> Result<(Bytes, Bytes, Bytes), Unread> {
    let file = Bytes::from(contents);
    let body_end = file
        .len()
        .checked_sub(8)
        .filter(|&end| end >= MAGIC.len())
        .ok_or(Unread::Bad("short"))?;
    let check = u64::from_le_bytes(file[body_end..].try_into().expect("8 bytes"));
    if file[..MAGIC.len()] != MAGIC {
        return Err(Unread::Bad("magic"));
    }
    if fnv1a(&file[..body_end]) != check {
        return Err(Unread::Bad("checksum"));
    }
    let mut at = MAGIC.len();
    let mut next = || -> Result<Bytes, Unread> {
        let bytes = take_bytes(&file, at, body_end).ok_or(Unread::Bad("lengths"))?;
        at += 8 + bytes.len();
        Ok(bytes)
    };
    let (held, json, payload) = (next()?, next()?, next()?);
    if at != body_end {
        return Err(Unread::Bad("lengths"));
    }
    Ok((held, json, payload))
}

/// The final sky reply of a file's `json` and `payload`, if they read back as one.
fn reply(json: &[u8], payload: Bytes) -> Result<Answer, Unread> {
    let body: ResponseBody = serde_json::from_slice(json).map_err(|_| Unread::Bad("json"))?;
    let ResponseBody::Sky(sky) = &body else {
        return Err(Unread::Bad("not a sky"));
    };
    let payload = BulkPayload::new(payload).map_err(|_| Unread::Bad("payload"))?;
    if !sky.is_final || payload.manifest() != sky.bulk {
        return Err(Unread::Bad("manifest"));
    }
    Ok(Answer {
        body,
        bulk: Some(payload),
    })
}

/// Appends `bytes` after their length, a `u64`.
fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    out.extend(bytes);
}

/// The bytes `put_bytes` wrote at `at` in `file`, ending by `end`, or `None` if they do not fit.
fn take_bytes(file: &Bytes, at: usize, end: usize) -> Option<Bytes> {
    let start = at.checked_add(8).filter(|&start| start <= end)?;
    let len = usize::try_from(u64::from_le_bytes(file[at..start].try_into().ok()?)).ok()?;
    let stop = start.checked_add(len).filter(|&stop| stop <= end)?;
    Some(file.slice(start..stop))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hyperion_sim::GeneratorVersion;
    use hyperion_sim::units::LightYears;
    use tempfile::TempDir;

    use hyperion_protocol::SkyRequest;

    use super::super::tests::{
        YEAR_S, answer, asked, bound_m, each_field_changed, galaxy, moved, position, reply_of,
        request, sun,
    };
    use super::*;
    use crate::compute::GalaxyKey;
    use crate::testing::WAIT;

    fn settings() -> SkySettings {
        SkySettings {
            caps: SkyCaps::DERIVED,
            members: FeatureMembers::Served,
        }
    }

    fn store(dir: &TempDir, budget: u64) -> SkyReplyFiles {
        SkyReplyFiles::new(dir.path().join(SKY_FILES_DIR), budget, &settings())
    }

    fn key() -> SkyReplyKey {
        SkyReplyKey::of(galaxy(), &request())
    }

    /// The reply's JSON, as the server writes its body.
    fn json(answer: &Answer) -> Vec<u8> {
        serde_json::to_vec(&answer.body).unwrap()
    }

    async fn get(files: &SkyReplyFiles, key: &SkyReplyKey) -> Option<Answer> {
        tokio::time::timeout(WAIT, files.get(key)).await.unwrap()
    }

    /// Keeps `answer` for `key` and waits until the store has written it, by a lookup behind it.
    async fn kept(files: &SkyReplyFiles, key: &SkyReplyKey, answer: &Answer) {
        files.keep(key, answer);
        get(
            files,
            &SkyReplyKey::of(GalaxyKey::new(0, GeneratorVersion::new(1)), &request()),
        )
        .await;
    }

    /// The reply a file holds that holds for `request`, asked from its observer.
    async fn held(files: &SkyReplyFiles, request: &SkyRequest) -> Option<HeldSky> {
        let key = SkyReplyKey::of(galaxy(), request);
        tokio::time::timeout(WAIT, files.held(&key, &position(request.observer)))
            .await
            .unwrap()
    }

    /// The files in the store's directory.
    fn listed(dir: &TempDir) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir.path().join(SKY_FILES_DIR))
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// A reply kept is read back from its file by a new store, as a restarted server's, with the
    /// same JSON and payload bytes; and the store writes nothing until it keeps one.
    #[tokio::test]
    async fn a_kept_reply_is_read_back_after_a_restart_with_the_same_bytes() {
        let dir = TempDir::new().unwrap();
        let sent = answer(1_000);
        let files = store(&dir, 1 << 30);
        assert_eq!(get(&files, &key()).await, None);
        assert!(!dir.path().join(SKY_FILES_DIR).exists(), "nothing written");
        kept(&files, &key(), &sent).await;
        assert_eq!(get(&files, &key()).await.as_ref(), Some(&sent));
        files.close().await;
        assert_eq!(listed(&dir).len(), 1, "{:?}", listed(&dir));

        let restarted = store(&dir, 1 << 30);
        let read = get(&restarted, &key()).await.expect("read from its file");
        assert_eq!(json(&read), json(&sent));
        assert_eq!(
            read.bulk.as_ref().unwrap().bytes(),
            sent.bulk.as_ref().unwrap().bytes()
        );
        let counters = restarted.counters();
        assert_eq!(
            (counters.entries(), counters.hits(), counters.misses()),
            (1, 1, 0),
            "{counters:?}"
        );
        let payload = u64::try_from(sent.bulk.as_ref().unwrap().len()).unwrap();
        assert!(counters.bytes() > payload, "{counters:?}");
        restarted.close().await;
    }

    /// Changing any field of the request, the galaxy, the versions or the settings is a miss.
    #[tokio::test]
    async fn a_file_is_a_miss_for_each_field_of_its_key() {
        let dir = TempDir::new().unwrap();
        let files = store(&dir, 1 << 30);
        kept(&files, &key(), &answer(10)).await;
        for (what, galaxy, request) in each_field_changed() {
            assert_eq!(
                get(&files, &SkyReplyKey::of(galaxy, &request)).await,
                None,
                "{what}"
            );
        }
        files.close().await;

        let files = store(&dir, 1 << 30);
        let base = files.store.as_ref().unwrap();
        let others = [
            (
                "caps",
                SkySettings {
                    caps: SkyCaps::forced(LightYears::new(10.0)).unwrap(),
                    ..settings()
                },
            ),
            (
                "members",
                SkySettings {
                    members: FeatureMembers::Held,
                    ..settings()
                },
            ),
        ];
        for (what, other) in others {
            assert_ne!(other.key_prefix(), base.prefix, "{what}");
            let files = SkyReplyFiles::new(dir.path().join(SKY_FILES_DIR), 1 << 30, &other);
            assert_eq!(get(&files, &key()).await, None, "{what}");
            files.close().await;
        }
        // The versions lead the prefix: a build at another version or protocol names other files.
        let mut prefix = PROTOCOL_VERSION.to_le_bytes().to_vec();
        prefix.extend(GENERATOR_VERSION.get().to_le_bytes());
        assert!(base.prefix.starts_with(&prefix));
        let key = base.file_key(&key());
        for at in 0..prefix.len() {
            let mut other = key.bytes.clone();
            other[at] ^= 1;
            assert_eq!(
                decode(encode(&key.bytes, &answer(10)).unwrap(), &other),
                Err(Unread::OtherKey),
                "byte {at}"
            );
        }
        files.close().await;
    }

    /// How a test spoils a file's bytes.
    type Spoil = fn(&mut Vec<u8>);

    /// A short, torn or garbled file is a miss and is removed, and never an error.
    #[tokio::test]
    async fn a_bad_file_is_a_miss_and_is_removed() {
        let sent = answer(100);
        let good = || {
            let dir = TempDir::new().unwrap();
            let files = store(&dir, 1 << 30);
            let name = files.store.as_ref().unwrap().file_key(&key()).name;
            (dir, files, name)
        };
        let spoil: [(&str, Spoil); 5] = [
            ("empty", |file| file.clear()),
            ("short", |file| file.truncate(file.len() / 2)),
            ("a flipped payload byte", |file| {
                let at = file.len() - 20;
                file[at] ^= 0x40;
            }),
            ("a flipped magic", |file| file[0] = b'X'),
            ("a lost checksum byte", |file| {
                file.pop();
            }),
        ];
        for (what, spoil) in spoil {
            let (dir, files, name) = good();
            kept(&files, &key(), &sent).await;
            let path = dir.path().join(SKY_FILES_DIR).join(&name);
            let mut file = fs::read(&path).unwrap();
            spoil(&mut file);
            fs::write(&path, &file).unwrap();
            assert_eq!(get(&files, &key()).await, None, "{what}");
            assert!(!path.exists(), "{what}: removed");
            let counters = files.counters();
            assert_eq!(
                (counters.removed_bad(), counters.entries(), counters.bytes()),
                (1, 0, 0),
                "{what}: {counters:?}"
            );
            files.close().await;
        }
        // A file another writes in its place is read as the bytes it holds.
        let (dir, files, name) = good();
        kept(&files, &key(), &sent).await;
        files.close().await;
        let path = dir.path().join(SKY_FILES_DIR).join(&name);
        fs::write(&path, b"not a sky").unwrap();
        let restarted = store(&dir, 1 << 30);
        assert_eq!(get(&restarted, &key()).await, None);
        assert_eq!(restarted.counters().removed_bad(), 1);
        restarted.close().await;
    }

    /// The files fit their budget, the least recently used removed first, in a restart's order too,
    /// which their files' times give; a reply larger than the whole budget is not written; a
    /// write's leftover is removed at start.
    #[tokio::test]
    async fn the_files_fit_their_budget_least_recently_used_first() {
        let dir = TempDir::new().unwrap();
        let reply = answer(1_000);
        let keyed = |n_max| {
            SkyReplyKey::of(
                galaxy(),
                &hyperion_protocol::SkyRequest {
                    n_max: Some(n_max),
                    ..request()
                },
            )
        };
        let probe = store(&dir, 1 << 30);
        let file_key = |n_max| probe.store.as_ref().unwrap().file_key(&keyed(n_max));
        let one = u64::try_from(encode(&file_key(1).bytes, &reply).unwrap().len()).unwrap();
        let path = |n_max| dir.path().join(SKY_FILES_DIR).join(file_key(n_max).name);

        let files = store(&dir, 2 * one + one / 2);
        for n_max in 1..=2 {
            kept(&files, &keyed(n_max), &reply).await;
        }
        // The first is used again, so the third pushes out the second.
        assert!(get(&files, &keyed(1)).await.is_some());
        kept(&files, &keyed(3), &reply).await;
        assert!(
            get(&files, &keyed(2)).await.is_none(),
            "the least recently used went"
        );
        assert!(!path(2).exists());
        assert!(get(&files, &keyed(1)).await.is_some());
        let counters = files.counters();
        assert_eq!(
            (counters.entries(), counters.evictions(), counters.bytes()),
            (2, 1, 2 * one),
            "{counters:?}"
        );
        files.close().await;
        assert_eq!(listed(&dir).len(), 2);

        // A restart orders them by their files' times, the older first out, and a read marks its
        // file used now.
        let at = |path: PathBuf, s| {
            let time = SystemTime::UNIX_EPOCH + Duration::from_secs(s);
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(time)
                .unwrap();
            time
        };
        at(path(3), 1_000);
        let read_at = at(path(1), 2_000);
        fs::write(dir.path().join(SKY_FILES_DIR).join("leftover.0.tmp"), b"x").unwrap();
        let smaller = store(&dir, one);
        assert!(get(&smaller, &keyed(3)).await.is_none(), "the older went");
        assert!(get(&smaller, &keyed(1)).await.is_some());
        let counters = smaller.counters();
        assert_eq!(
            (counters.entries(), counters.evictions()),
            (1, 1),
            "{counters:?}"
        );
        assert!(fs::metadata(path(1)).unwrap().modified().unwrap() > read_at);
        smaller.close().await;
        assert_eq!(listed(&dir), [file_key(1).name], "the leftover removed too");

        let tiny = store(&dir, one - 1);
        kept(&tiny, &keyed(4), &reply).await;
        assert!(get(&tiny, &keyed(4)).await.is_none());
        assert_eq!(tiny.counters().refused(), 1);
        tiny.close().await;
        probe.close().await;
    }

    /// Off, nothing is written or found; closed, nothing more is.
    #[tokio::test]
    async fn off_or_closed_nothing_is_kept() {
        let dir = TempDir::new().unwrap();
        let off = store(&dir, 0);
        off.keep(&key(), &answer(10));
        assert_eq!(get(&off, &key()).await, None);
        assert_eq!(off.counters(), SkyFileCounters::default());
        off.close().await;
        assert!(!dir.path().join(SKY_FILES_DIR).exists());

        let files = store(&dir, 1 << 30);
        files.close().await;
        files.keep(&key(), &answer(10));
        assert_eq!(get(&files, &key()).await, None);
        assert!(!dir.path().join(SKY_FILES_DIR).exists());
    }

    /// Only a final sky reply with a payload is written.
    #[test]
    fn only_a_final_sky_reply_is_encoded() {
        let mut partial = answer(10);
        if let ResponseBody::Sky(sky) = &mut partial.body {
            sky.is_final = false;
        }
        assert_eq!(encode(b"k", &partial), None);
        let mut bare = answer(10);
        bare.bulk = None;
        assert_eq!(encode(b"k", &bare), None);
        let file = encode(b"k", &answer(10)).unwrap();
        assert_eq!(decode(file.clone(), b"k"), Ok(answer(10)));
        assert_eq!(decode(file, b"j"), Err(Unread::OtherKey));
    }

    /// A file of the arrival that holds for a sky asked later, or from a little way off, answers
    /// it, after a restart too, under its own key with its own JSON and payload byte for byte;
    /// none answers past its `valid_until` or its bound, or for another arrival. The files of one
    /// arrival share their names' lead, and of two that hold the latest seen answers.
    #[tokio::test]
    async fn a_file_of_the_arrival_that_holds_answers_a_later_sky_after_a_restart() {
        let dir = TempDir::new().unwrap();
        let first = asked(sun(), 0);
        let first_key = SkyReplyKey::of(galaxy(), &first);
        let sent = reply_of(&first, YEAR_S, &[8.6, 4.37]);
        let files = store(&dir, 1 << 30);
        kept(&files, &first_key, &sent).await;
        let found = held(&files, &asked(sun(), 3_600)).await;
        assert_eq!(found.expect("its hold known from its write").key, first_key);
        files.close().await;

        let restarted = store(&dir, 1 << 30);
        let later = asked(moved(sun(), 0.5 * bound_m()), 3_600);
        let exactly = get(&restarted, &SkyReplyKey::of(galaxy(), &later)).await;
        assert_eq!(exactly, None, "no file holds it exactly");
        let found = held(&restarted, &later).await.expect("held from its file");
        assert_eq!(found.key, first_key);
        assert_eq!(json(&found.answer), json(&sent));
        assert_eq!(
            found.answer.bulk.as_ref().unwrap().bytes(),
            sent.bulk.as_ref().unwrap().bytes()
        );
        assert!((found.offset_m / bound_m() - 0.5).abs() < 1e-9, "{found:?}");
        let refused = [
            ("at its valid_until", asked(sun(), YEAR_S)),
            ("before it was seen", asked(sun(), -1)),
            (
                "past its bound",
                asked(moved(sun(), 1.01 * bound_m()), 3_600),
            ),
            (
                "another arrival",
                SkyRequest {
                    camera_limit_v: Some(9.0),
                    ..asked(sun(), 3_600)
                },
            ),
        ];
        for (what, request) in refused {
            assert!(held(&restarted, &request).await.is_none(), "{what}");
        }
        // The file was read once for its hold, and the misses are the exact lookup's alone.
        let counters = restarted.counters();
        assert_eq!(
            (
                counters.hits(),
                counters.held(),
                counters.misses(),
                counters.entries()
            ),
            (1, 1, 1, 1),
            "{counters:?}"
        );
        restarted.close().await;

        let files = store(&dir, 1 << 30);
        let second = asked(sun(), 600);
        let other = SkyRequest {
            n_max: Some(9),
            ..first.clone()
        };
        for kept_request in [&second, &other] {
            let reply = reply_of(kept_request, YEAR_S, &[4.37]);
            kept(&files, &SkyReplyKey::of(galaxy(), kept_request), &reply).await;
        }
        assert_eq!(listed(&dir).len(), 3, "{:?}", listed(&dir));
        let lead = |request: &SkyRequest| {
            let name = files
                .store
                .as_ref()
                .unwrap()
                .file_key(&SkyReplyKey::of(galaxy(), request))
                .name;
            name[..17].to_owned()
        };
        assert_eq!(lead(&first), lead(&second));
        assert_ne!(lead(&first), lead(&other));
        let latest = held(&files, &asked(sun(), 3_600)).await.unwrap();
        assert_eq!(latest.key, SkyReplyKey::of(galaxy(), &second));
        files.close().await;
    }

    /// A file of the arrival read for its hold that is bad is removed, as any bad file is; one
    /// under the arrival's lead that holds another key is only not read.
    #[tokio::test]
    async fn a_bad_file_of_the_arrival_is_removed_and_another_keys_is_not_read() {
        let dir = TempDir::new().unwrap();
        let first = asked(sun(), 0);
        let key = SkyReplyKey::of(galaxy(), &first);
        let files = store(&dir, 1 << 30);
        kept(&files, &key, &reply_of(&first, YEAR_S, &[4.37])).await;
        let other = SkyRequest {
            n_max: Some(9),
            ..first.clone()
        };
        let (name, other_name) = {
            let store = files.store.as_ref().unwrap();
            let other_key = SkyReplyKey::of(galaxy(), &other);
            (store.file_key(&key).name, store.file_key(&other_key).name)
        };
        files.close().await;
        let skies = dir.path().join(SKY_FILES_DIR);
        // The first's file under the other arrival's lead: its key is not the one it would hold.
        let impostor = skies.join(format!("{}0000000000000000.{EXTENSION}", &other_name[..17]));
        fs::copy(skies.join(&name), &impostor).unwrap();
        let restarted = store(&dir, 1 << 30);
        assert!(held(&restarted, &asked_as(&other, 3_600)).await.is_none());
        assert!(impostor.exists(), "not bad, only another key's");
        restarted.close().await;

        let path = skies.join(&name);
        let mut file = fs::read(&path).unwrap();
        let at = file.len() - 20;
        file[at] ^= 0x40;
        fs::write(&path, &file).unwrap();
        let restarted = store(&dir, 1 << 30);
        assert!(held(&restarted, &asked(sun(), 3_600)).await.is_none());
        assert!(!path.exists(), "removed");
        assert_eq!(restarted.counters().removed_bad(), 1);
        restarted.close().await;
    }

    /// `request` asked again `seconds` later.
    fn asked_as(request: &SkyRequest, seconds: i64) -> SkyRequest {
        SkyRequest {
            time: hyperion_protocol::UniverseTime {
                seconds: request.time.seconds + seconds,
                ..request.time
            },
            ..request.clone()
        }
    }
}
