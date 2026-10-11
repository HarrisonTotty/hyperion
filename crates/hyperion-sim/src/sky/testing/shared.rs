//! A fixture built once per nextest run and test binary, then shared with that run's other
//! processes of the binary through a file.
//!
//! nextest runs every test in a process of its own, so a fixture held in a `OnceLock` is built
//! once per test, not once per binary. The full luminosity tables are the costly case: 40–53 CPU-s
//! natively and 50–75 s under wasmtime, rebuilt by each of the 67 fast sky tests that read them,
//! which held 87% of the sky's test time (`perf-sim-sky.md`, 2026-10-10). [`shared`] builds a
//! fixture in the first process of a run that asks for it and writes its bytes to a file. The
//! run's later processes of that binary read the file instead of building. Outside nextest
//! (`cargo test`, one process per binary, with no `NEXTEST_RUN_ID`) it builds in the process, as
//! before.
//!
//! # The key
//!
//! A copy lives at
//! `<profile directory>/sky-fixtures/<run>-<binary>-<fingerprint>/<name>`:
//!
//! - `<profile directory>` is the parent of the `deps/` directory holding the test binary, such as
//!   `target/debug` or `target/wasm32-wasip1/debug`. Each target and each cargo profile gets its
//!   own directory, so wasip1 always builds and reads its own copy and never the native one.
//! - `<run>` is `NEXTEST_RUN_ID`, a fresh UUID that nextest makes for every run, so a process
//!   never looks at another run's copy. Each run builds its own, and a copy from another build,
//!   commit or profile, which another run made, is never opened.
//! - `<binary>` is the test binary's file name. Its hash is cargo's metadata for the crate,
//!   profile, target and features.
//! - `<fingerprint>` is a hash of the binary file's bytes ([`Fingerprint`]), taken when the
//!   process first asks for a fixture. Two builds of one binary under one run id get different
//!   names, which covers a test binary run by hand with a `NEXTEST_RUN_ID` left in its
//!   environment. Rebuilding a binary into its own target directory while a run of it is going
//!   is unsupported, as it is for nextest, which runs the binaries it listed at the start. On
//!   Linux the running process's executable is then gone, and it builds in its process. On macOS
//!   and under wasmtime the fingerprint would be the new file's.
//!
//! Inside, the file's framing ([`WordWriter`], [`WordReader`]) holds a magic word, the format's
//! version, the payload and a checksum. A reader takes only a whole file whose checksum holds, so
//! it cannot read a damaged one.
//!
//! # Parallel safety
//!
//! The run's processes may start together. The final path is only ever created by renaming a
//! temporary file of the writer's own, created with `create_new`, after its every byte has been
//! written and the file closed. A rename replaces the name in one step: `std::fs::rename` is
//! POSIX `rename(2)` on Unix and `MoveFileExW` or `SetFileInformationByHandle` with replacement on
//! Windows (its documentation, Rust 1.98), and under wasmtime WASI's `path_rename`, which wasmtime
//! performs as the host's `renameat`. So a reader that opens the path opens either no file or a
//! whole one, never a part. A reader that opened an earlier copy keeps reading that copy's own
//! bytes. Natively, an exclusive [`File::lock`] on a lock file beside it serialises the builders:
//! one process builds, the others wait for the lock and then read its file, so the run builds
//! once. A builder leaves a marker while it builds and removes it once its file is in place. A
//! process that takes the lock and finds the marker without the file knows the builder before it
//! failed (its write failed, or it panicked or was killed). It then builds in its process at once,
//! so that a failure leaves the run building side by side, as before, rather than one test after
//! another.
//!
//! wasip1 has no file locks (`File::lock` returns `Unsupported` there), so under wasmtime the
//! processes that find no file each build, and each renames its own temporary file into place.
//! Racing writers write identical bytes: the same binary building the same deterministic fixture,
//! which `sky::luminosity`'s test of the shared tables against a fresh build checks bit for bit.
//! So whichever rename lands last, every reader decodes the same fixture. Any I/O failure (no
//! target directory, a read-only or full disk, a variable the runner did not pass) falls back to
//! building in the process. The worst case is one build per test, which is how it was before.
//!
//! A wasip1 guest sees none of the host's environment unless wasmtime is told to pass it, so it
//! shares only when its runner passes the run's id (`--env NEXTEST_RUN_ID`). Without it, the guest
//! builds in its process. The guest's `argv[0]` is its module's bare file name, so [`this_binary`]
//! looks for that name among the workspace target's wasip1 profiles. Those lie under the
//! repository, which the runner opens to the guest (`--dir`).
//!
//! # I/O
//!
//! `hyperion-sim` does no I/O (`.claude/rules/rust-dev.md`). This module is test support: it is
//! compiled only under `cfg(test)`, with [`super`], so no path of the library reaches it.
//!
//! Disk: a run writes one copy of the full tables per target, 68.7 MB (68,669,112 bytes, measured
//! 2026-10-11), beside a lock file of no bytes. The process that builds a copy removes the run
//! directories in its profile directory that are a day old ([`MAX_AGE`], as
//! `decision-test-coverage.md` §4 binds) or older than the newest [`KEEP_RUNS`], so a profile
//! directory holds at most four copies, 275 MB. Aging them out is the one place this module reads
//! the clock; the clock decides only which old directories are removed, never what a test reads.
//!
//! nextest's setup scripts were the other way to build once per run. They run one command per
//! profile before the tests, not once per test binary and target, so they would need a separate
//! binary to build the tables. They would also need an edit to `.config/nextest.toml`, and a
//! wasip1 guest still could not see the variables they set unless its runner passed them. The
//! lock and rename here need nothing outside test code but that one runner flag.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

/// The directory, in a profile directory, that holds the shared copies of every run.
const ROOT: &str = "sky-fixtures";

/// How many run directories a profile directory keeps: the newest, by when a copy was last written
/// in them. It is more than one so that two runs going at once in one target directory never
/// remove each other's copy. A removed copy would only be rebuilt, never misread.
const KEEP_RUNS: usize = 4;

/// How old a run directory may grow, by when a copy was last written in it, before the next build
/// in its profile directory removes it: a day (`decision-test-coverage.md` §4). No run of the fast
/// suites lasts an hour.
const MAX_AGE: Duration = Duration::from_hours(24);

/// The first word of a shared file: "HYSKYFIX" in ASCII, little-endian.
const MAGIC: u64 = u64::from_le_bytes(*b"HYSKYFIX");

/// The version of the framing and of every payload written through it. Bump it when a codec's
/// layout changes, so that a file of the old layout is refused rather than misread. The binary's
/// fingerprint already keys copies by build, so this is a second guard.
const FORMAT_VERSION: u64 = 1;

/// How a process came by a shared fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sharing {
    /// Read from the run's file, which another process of the binary wrote.
    Read,
    /// Built in this process and written to the run's file for the others.
    Written,
    /// Built in this process and not shared: outside nextest, or a step of the sharing failed.
    Unshared,
}

/// `build()`, built once per nextest run and test binary and shared with the run's other processes
/// of the binary through a file named `name` (see the [module](self)), and how this process came
/// by it. It builds in the process when it runs outside nextest, and when any step of the sharing
/// fails.
///
/// `encode` writes the fixture's every bit through a [`WordWriter`]. `decode` reads them back
/// through a [`WordReader`], or gives `None` for a file that is not one of its own. `decode` of
/// `encode`'s bytes must be the fixture bit for bit: each codec has a round-trip test.
///
/// # Panics
///
/// If `build` or `encode` panics. Natively the builder holds the run's lock while it calls them,
/// and the marker it leaves lets the processes after it build at once.
pub(crate) fn shared<T>(
    name: &str,
    build: impl FnOnce() -> T,
    encode: impl FnOnce(&T) -> Vec<u8>,
    decode: impl Fn(&[u8]) -> Option<T>,
) -> (T, Sharing) {
    match run_dir() {
        Some(dir) => share_in(dir, name, build, encode, decode),
        None => (build(), Sharing::Unshared),
    }
}

/// The copy of the fixture `name` that this run's processes of this binary share, if one has been
/// written and decodes. A test that a shared fixture is a fresh build reads it through this.
#[must_use]
pub(crate) fn read_shared<T>(name: &str, decode: impl Fn(&[u8]) -> Option<T>) -> Option<T> {
    read(&run_dir()?.join(name), &decode)
}

/// [`shared`] in `dir`.
fn share_in<T>(
    dir: &Path,
    name: &str,
    build: impl FnOnce() -> T,
    encode: impl FnOnce(&T) -> Vec<u8>,
    decode: impl Fn(&[u8]) -> Option<T>,
) -> (T, Sharing) {
    let path = dir.join(name);
    if let Some(fixture) = read(&path, &decode) {
        return (fixture, Sharing::Read);
    }
    if fs::create_dir_all(dir).is_err() {
        // No directory to share through: build here, as outside nextest.
        return (build(), Sharing::Unshared);
    }
    // Held until this function returns: the run's other processes wait on it natively, then read
    // what this one wrote. Where the platform has no file locks (wasip1) or the lock fails, the
    // processes build side by side, which the rename keeps safe (see the module's notes). It is
    // opened without truncating, since on Windows a file another process holds locked cannot be
    // truncated.
    let lock = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join(format!("{name}.lock")))
        .ok()
        .filter(|file| file.lock().is_ok());
    if let Some(fixture) = read(&path, &decode) {
        return (fixture, Sharing::Read);
    }
    let marker = dir.join(format!("{name}.building"));
    if lock.is_some() {
        if marker.exists() {
            // The builder before this one left without putting its file in place: build here
            // without the lock, so that the run's other processes need not wait for this build.
            drop(lock);
            return (build(), Sharing::Unshared);
        }
        // If the marker cannot be made, a failure below only leaves the next process to build
        // under the lock in its turn, which is slower but still correct.
        let _ = File::create(&marker);
    }
    let fixture = build();
    if !write(dir, name, &encode(&fixture)) {
        return (fixture, Sharing::Unshared);
    }
    // The file is in place, and every later process reads it before looking for the marker, so
    // a marker left behind changes nothing; its run directory is pruned in time.
    let _ = fs::remove_file(&marker);
    prune(dir);
    (fixture, Sharing::Written)
}

/// The fixture in the file at `path`, if it exists and `decode` takes its bytes.
fn read<T>(path: &Path, decode: &impl Fn(&[u8]) -> Option<T>) -> Option<T> {
    // A missing file is the usual case (no process has written it yet). Any other error reads as
    // missing too: the caller then builds, which is always correct.
    let bytes = fs::read(path).ok()?;
    decode(&bytes)
}

/// Writes `bytes` to `dir/name` by way of a temporary file of this process's own, renamed into
/// place once it is whole, and says whether the copy was put in place.
fn write(dir: &Path, name: &str, bytes: &[u8]) -> bool {
    let temporary = dir.join(format!("{name}.{:016x}.tmp", process_tag()));
    // `create_new`: two processes never share a temporary file. If one with this name exists,
    // another process holds it, and this one leaves the sharing to it.
    let Ok(mut file) = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
    else {
        return false;
    };
    let written = file.write_all(bytes).is_ok();
    drop(file);
    if written && fs::rename(&temporary, dir.join(name)).is_ok() {
        return true;
    }
    // Nothing reads a temporary file, and its run directory is pruned in time. Removing it here
    // only saves the space early, so a failure to remove it is safe to ignore.
    let _ = fs::remove_file(&temporary);
    false
}

/// A tag for this process among the run's: its id natively. On wasip1, which has no process ids
/// (`std::process::id` panics there), it is a hash of the process's arguments, which nextest makes
/// distinct for each test of a binary (the test's name). [`write`] creates its temporary file with
/// `create_new`, so a repeated tag only skips the sharing.
#[must_use]
fn process_tag() -> u64 {
    let mut tag = Fingerprint::new();
    #[cfg(not(target_family = "wasm"))]
    tag.update(&std::process::id().to_le_bytes());
    for argument in std::env::args_os() {
        tag.update(argument.as_encoded_bytes());
        tag.update(&[0]);
    }
    tag.finish()
}

/// Removes the run directories beside `dir` that are older than [`MAX_AGE`] or than the newest
/// [`KEEP_RUNS`].
///
/// Pruning is best effort. A directory that cannot be listed, or whose entries cannot be dated,
/// keeps its runs, which costs only disk until a later build prunes it.
fn prune(dir: &Path) {
    let Some(root) = dir.parent() else {
        return;
    };
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let runs: Vec<(PathBuf, SystemTime)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((entry.path(), modified))
        })
        .collect();
    for stale in stale_runs(runs, KEEP_RUNS, SystemTime::now()) {
        if stale.as_path() != dir {
            // A run directory that cannot be removed now (a reader holds a file of it open on
            // Windows) is removed by a later build. Removing one that is still in use costs only
            // a rebuild in that run.
            let _ = fs::remove_dir_all(&stale);
        }
    }
}

/// The runs of `runs` older than the newest `keep`, by their modification times, or older than
/// [`MAX_AGE`] at `now`. A run dated after `now` (a clock set back) is not aged.
#[must_use]
fn stale_runs(mut runs: Vec<(PathBuf, SystemTime)>, keep: usize, now: SystemTime) -> Vec<PathBuf> {
    // Newest first; ties broken by path, so that the choice is the same however the directory
    // lists them.
    runs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    runs.into_iter()
        .enumerate()
        .filter(|(i, (_, modified))| {
            *i >= keep || now.duration_since(*modified).is_ok_and(|age| age > MAX_AGE)
        })
        .map(|(_, (path, _))| path)
        .collect()
}

/// This process's run directory (see the [module](self)'s key), or `None` outside nextest or if
/// the binary is not found or cannot be read: found once per process.
fn run_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let run = std::env::var("NEXTEST_RUN_ID").ok()?;
        let binary = this_binary()?;
        let fingerprint = fingerprint_of(&binary)?;
        run_dir_for(&run, &binary, fingerprint)
    })
    .as_deref()
}

/// The run directory of the run `run` of the binary at `binary` whose bytes have `fingerprint`,
/// or `None` if `run` is not a nextest run id (a UUID: letters, digits and hyphens) or the binary
/// does not lie in a profile's `deps/` directory or similar.
#[must_use]
fn run_dir_for(run: &str, binary: &Path, fingerprint: u64) -> Option<PathBuf> {
    // Anything but a run id must not reach a path.
    if run.is_empty()
        || run.len() > 64
        || !run.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    let name = binary.file_name()?.to_str()?;
    let profile = binary.parent()?.parent()?;
    Some(
        profile
            .join(ROOT)
            .join(format!("{run}-{name}-{fingerprint:016x}")),
    )
}

/// The file of this test binary: the running executable natively.
#[cfg(not(target_family = "wasm"))]
fn this_binary() -> Option<PathBuf> {
    // Without its path the process cannot key a copy, and builds in its process.
    std::env::current_exe().ok()
}

/// The file of this test binary: under wasmtime, whose guest sees its module's bare file name as
/// `argv[0]` and has no `current_exe`, the one module of that name among the workspace target's
/// wasip1 profiles, `target/wasm32-wasip1/<profile>/deps/<name>`. The name holds cargo's
/// metadata hash, which differs between profiles, so at most one profile holds it. A target
/// directory elsewhere (`CARGO_TARGET_DIR`) is not searched, and the guest then builds in its
/// process.
#[cfg(target_family = "wasm")]
fn this_binary() -> Option<PathBuf> {
    let argv0 = std::env::args_os().next()?;
    let name = Path::new(&argv0).file_name()?.to_owned();
    // `crates/<crate>/../../target`: the workspace's target directory.
    let wasip1 = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wasm32-wasip1");
    // A profile directory that cannot be listed is passed over: if it held the module, the module
    // is not found, and the guest builds in its process.
    let mut found = fs::read_dir(wasip1)
        .ok()?
        .filter_map(Result::ok)
        .map(|profile| profile.path().join("deps").join(&name))
        .filter(|path| path.is_file());
    let binary = found.next()?;
    found.next().is_none().then_some(binary)
}

/// The [`Fingerprint`] of the bytes of the file at `path`, read in chunks of 1 MiB so that an
/// 80 MB test binary costs no 80 MB of memory, or `None` if it cannot be read: the caller then
/// builds in its process.
#[must_use]
fn fingerprint_of(path: &Path) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let mut buffer = vec![0_u8; 1 << 20];
    let mut fingerprint = Fingerprint::new();
    loop {
        match file.read(&mut buffer) {
            Ok(0) => return Some(fingerprint.finish()),
            Ok(n) => fingerprint.update(&buffer[..n]),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

/// A 64-bit hash of a stream of bytes, for telling apart files and refusing damaged ones, not for
/// any generated value. It is rustc's `FxHasher` step over little-endian 8-byte words, the last
/// zero-padded: `h = (h.rotl(5) ^ word) · 0x517c_c1b7_2722_0a95` (`rustc-hash` 1.1, after
/// Firefox's hasher). Its finish mixes in the byte count with `SplitMix64`'s finaliser (Vigna
/// 2015, `splitmix64.c`: Stafford's 2011 Mix13). Each step is a bijection of the state for a given
/// word, so changing any one word of a stream of a given length changes its hash. It takes eight
/// bytes a step, so an 80 MB binary costs some tens of milliseconds.
#[derive(Debug, Clone)]
struct Fingerprint {
    hash: u64,
    pending: [u8; 8],
    filled: usize,
    length: u64,
}

impl Fingerprint {
    const SEED: u64 = 0x517c_c1b7_2722_0a95;

    #[must_use]
    const fn new() -> Self {
        Self {
            hash: 0,
            pending: [0; 8],
            filled: 0,
            length: 0,
        }
    }

    fn step(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
    }

    fn update(&mut self, mut bytes: &[u8]) {
        self.length = self
            .length
            .wrapping_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if self.filled > 0 {
            let take = (8 - self.filled).min(bytes.len());
            self.pending[self.filled..self.filled + take].copy_from_slice(&bytes[..take]);
            self.filled += take;
            bytes = &bytes[take..];
            if self.filled < 8 {
                return;
            }
            self.step(u64::from_le_bytes(self.pending));
            self.filled = 0;
        }
        let (words, rest) = bytes.as_chunks::<8>();
        for &word in words {
            self.step(u64::from_le_bytes(word));
        }
        self.pending[..rest.len()].copy_from_slice(rest);
        self.filled = rest.len();
    }

    #[must_use]
    fn finish(mut self) -> u64 {
        if self.filled > 0 {
            self.pending[self.filled..].fill(0);
            self.step(u64::from_le_bytes(self.pending));
        }
        let mut z = self.hash ^ self.length;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// A shared file's bytes as it is written: a magic word and the format's version, the payload's
/// little-endian 8-byte words, and a checksum of everything before it ([`WordWriter::finish`]).
#[derive(Debug)]
pub(crate) struct WordWriter {
    bytes: Vec<u8>,
}

impl WordWriter {
    /// A writer with room for a payload of about `payload_bytes` bytes.
    #[must_use]
    pub(crate) fn with_capacity(payload_bytes: usize) -> Self {
        let mut writer = Self {
            bytes: Vec::with_capacity(payload_bytes.saturating_add(24)),
        };
        writer.word(MAGIC);
        writer.word(FORMAT_VERSION);
        writer
    }

    pub(crate) fn word(&mut self, word: u64) {
        self.bytes.extend_from_slice(&word.to_le_bytes());
    }

    /// `value`'s bits, exactly.
    ///
    /// # Panics
    ///
    /// If `value` is a NaN, whose bits are unspecified ([`hyperion_testkit::float::bits`]).
    pub(crate) fn float(&mut self, value: f64) {
        self.word(hyperion_testkit::float::bits(value));
    }

    /// A length or an index.
    pub(crate) fn count(&mut self, n: usize) {
        self.word(u64::try_from(n).expect("a length fits 64 bits on every target"));
    }

    /// A list's length and capacity, which [`WordReader::list`] reads back and allocates it at, so
    /// that a decoded fixture owns the heap the encoded one did.
    pub(crate) fn lengths(&mut self, len: usize, capacity: usize) {
        self.count(len);
        self.count(capacity);
    }

    /// `values`' length and capacity ([`lengths`](Self::lengths)), then each value's bits.
    ///
    /// # Panics
    ///
    /// If a value is a NaN, whose bits are unspecified ([`hyperion_testkit::float::bits`]).
    pub(crate) fn floats(&mut self, values: &Vec<f64>) {
        self.lengths(values.len(), values.capacity());
        for &value in values {
            self.float(value);
        }
    }

    /// The file's bytes: what was written, then its checksum.
    #[must_use]
    pub(crate) fn finish(mut self) -> Vec<u8> {
        let mut checksum = Fingerprint::new();
        checksum.update(&self.bytes);
        self.word(checksum.finish());
        self.bytes
    }
}

/// A reader of a [`WordWriter`]'s file: it opens only a whole file whose magic, version and checksum
/// hold, and gives `None` from any read past the payload's end. Each list is allocated at the
/// capacity written, so that a decoded fixture owns the heap the encoded one did.
#[derive(Debug)]
pub(crate) struct WordReader<'a> {
    rest: &'a [u8],
}

impl<'a> WordReader<'a> {
    /// The payload of `bytes`, if they are a whole file of this format.
    #[must_use]
    pub(crate) fn open(bytes: &'a [u8]) -> Option<Self> {
        if !bytes.len().is_multiple_of(8) {
            return None;
        }
        let (body, sum) = bytes.split_last_chunk::<8>()?;
        let mut checksum = Fingerprint::new();
        checksum.update(body);
        if checksum.finish() != u64::from_le_bytes(*sum) {
            return None;
        }
        let mut reader = Self { rest: body };
        (reader.word()? == MAGIC && reader.word()? == FORMAT_VERSION).then_some(reader)
    }

    pub(crate) fn word(&mut self) -> Option<u64> {
        let (word, rest) = self.rest.split_first_chunk::<8>()?;
        self.rest = rest;
        Some(u64::from_le_bytes(*word))
    }

    /// A value of the bits written.
    pub(crate) fn float(&mut self) -> Option<f64> {
        self.word().map(f64::from_bits)
    }

    /// A length or an index.
    pub(crate) fn count(&mut self) -> Option<usize> {
        usize::try_from(self.word()?).ok()
    }

    /// A list's length and capacity ([`WordWriter::lengths`]), then that many items, each read by
    /// `item`, which reads at least one word; allocated at the capacity written. Before anything
    /// is allocated, the length is checked against the words left, and the capacity against
    /// twice the length and some: a vector grown by doubling holds no more. A list that fails
    /// either check reads as none, so its fixture is built instead.
    pub(crate) fn list<T>(
        &mut self,
        mut item: impl FnMut(&mut Self) -> Option<T>,
    ) -> Option<Vec<T>> {
        let (len, capacity) = (self.count()?, self.count()?);
        if len > self.rest.len() / 8
            || capacity < len
            || capacity > len.saturating_mul(2).saturating_add(64)
        {
            return None;
        }
        let mut items = Vec::with_capacity(capacity);
        for _ in 0..len {
            items.push(item(self)?);
        }
        Some(items)
    }

    /// A length and capacity, then that many values ([`list`](Self::list)).
    pub(crate) fn floats(&mut self) -> Option<Vec<f64>> {
        self.list(Self::float)
    }

    /// `Some` if the payload has been read to its end, and nothing is left over.
    #[must_use]
    pub(crate) fn end(self) -> Option<()> {
        self.rest.is_empty().then_some(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn encode(values: &[u64]) -> Vec<u8> {
        let mut w = WordWriter::with_capacity(8 * values.len() + 16);
        w.lengths(values.len(), values.len());
        for &v in values {
            w.word(v);
        }
        w.finish()
    }

    fn decode(bytes: &[u8]) -> Option<Vec<u64>> {
        let mut r = WordReader::open(bytes)?;
        let values = r.list(WordReader::word)?;
        r.end()?;
        Some(values)
    }

    /// A directory of this test's own under the workspace's target directory, emptied: the run's
    /// id and this process's tag keep runs and targets apart.
    fn test_dir(name: &str) -> PathBuf {
        let run = std::env::var("NEXTEST_RUN_ID").unwrap_or_default();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("{ROOT}-tests"))
            .join(format!(
                "{name}-{}-{run}-{:016x}",
                std::env::consts::ARCH,
                process_tag()
            ));
        // Left over only by a killed earlier run with the same tag; absent is the usual case.
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// The first process builds the fixture and writes it, and a later one reads it without
    /// building. A damaged file is refused, rebuilt and replaced. No temporary file or marker is
    /// left behind.
    #[test]
    fn a_fixture_is_built_once_read_back_and_rebuilt_over_a_damaged_file() {
        let dir = test_dir("built-once");
        let builds = Cell::new(0);
        let fixture = vec![1, u64::MAX, 0x8000_0000_0000_0000];
        let build = || {
            builds.set(builds.get() + 1);
            fixture.clone()
        };
        let share = || share_in(&dir, "probe", build, |v: &Vec<u64>| encode(v), decode);
        assert_eq!(share(), (fixture.clone(), Sharing::Written));
        assert_eq!(builds.get(), 1);
        assert_eq!(share(), (fixture.clone(), Sharing::Read));
        assert_eq!(builds.get(), 1, "the second read the file");
        let mut damaged = fs::read(dir.join("probe")).expect("the shared file");
        damaged[20] ^= 0x10;
        fs::write(dir.join("probe"), &damaged).expect("written");
        assert_eq!(share(), (fixture.clone(), Sharing::Written));
        assert_eq!(builds.get(), 2, "the damaged file was refused");
        assert_eq!(read(&dir.join("probe"), &decode), Some(fixture));
        let mut left: Vec<String> = fs::read_dir(&dir)
            .expect("the directory")
            .map(|e| {
                e.expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["probe", "probe.lock"],
            "no temporary file or marker is left"
        );
        fs::remove_dir_all(&dir).expect("removed");
    }

    /// A process that takes the lock and finds the marker of a builder that left without its file
    /// builds without sharing, and so without making the run's other processes wait. wasip1 has no
    /// lock, so its processes build and write side by side whatever the marker says.
    #[test]
    fn after_a_failed_builder_a_process_builds_without_waiting() {
        let dir = test_dir("failed-builder");
        fs::create_dir_all(&dir).expect("the directory");
        fs::write(dir.join("probe.building"), b"").expect("the marker");
        let (fixture, sharing) = share_in(
            &dir,
            "probe",
            || vec![7_u64],
            |v: &Vec<u64>| encode(v),
            decode,
        );
        assert_eq!(fixture, vec![7]);
        if cfg!(target_family = "wasm") {
            assert_eq!(sharing, Sharing::Written);
        } else {
            assert_eq!(sharing, Sharing::Unshared);
            assert!(!dir.join("probe").exists(), "nothing was written");
        }
        fs::remove_dir_all(&dir).expect("removed");
    }

    /// The framing takes back exactly what was written, and refuses a file with any bit changed,
    /// cut short, lengthened or of another format.
    #[test]
    fn the_framing_refuses_a_damaged_short_long_or_foreign_file() {
        let values = vec![3, 1 << 40, 7];
        let bytes = encode(&values);
        assert_eq!(decode(&bytes), Some(values));
        for i in 0..bytes.len() {
            for bit in 0..8 {
                let mut damaged = bytes.clone();
                damaged[i] ^= 1 << bit;
                assert_eq!(decode(&damaged), None, "byte {i}, bit {bit}");
            }
        }
        assert_eq!(decode(&bytes[..bytes.len() - 8]), None);
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode(&[]), None);
        let mut long = bytes.clone();
        long.extend_from_slice(&[0; 8]);
        assert_eq!(decode(&long), None);
        // A payload that reads short of its end, whose checksum holds, is refused by `end`.
        let mut w = WordWriter::with_capacity(32);
        w.lengths(1, 1);
        w.word(5);
        w.word(6);
        assert_eq!(decode(&w.finish()), None);
        // A length beyond the payload, or a capacity below the length or far above it, allocates
        // nothing and reads as none.
        for (len, capacity) in [(usize::MAX, usize::MAX), (2, 1), (2, 69), (0, 65)] {
            let mut w = WordWriter::with_capacity(32);
            w.lengths(len, capacity);
            w.word(1);
            w.word(2);
            let file = w.finish();
            let mut r = WordReader::open(&file).expect("a whole file");
            assert_eq!(r.floats(), None, "{len} values in {capacity}");
        }
        // A list keeps its capacity.
        let mut values = Vec::with_capacity(10);
        values.extend([1.5, -0.0]);
        let mut w = WordWriter::with_capacity(64);
        w.floats(&values);
        let file = w.finish();
        let mut r = WordReader::open(&file).expect("a whole file");
        let back = r.floats().expect("the values");
        assert_eq!(back.capacity(), 10);
        assert_eq!(
            back.iter()
                .map(|&v| hyperion_testkit::float::bits(v))
                .collect::<Vec<_>>(),
            [
                hyperion_testkit::float::bits(1.5),
                hyperion_testkit::float::bits(-0.0)
            ]
        );
    }

    /// The fingerprint is the same however the bytes arrive, and differs for one byte changed,
    /// added or removed.
    #[test]
    fn the_fingerprint_is_of_the_bytes_however_they_arrive() {
        let bytes: Vec<u8> = (0..1000_u32)
            .map(|i| u8::try_from(i % 251).expect("below 251"))
            .collect();
        let of = |b: &[u8]| {
            let mut f = Fingerprint::new();
            f.update(b);
            f.finish()
        };
        let whole = of(&bytes);
        for split in [1, 3, 7, 8, 9, 500, 999] {
            let mut f = Fingerprint::new();
            for chunk in bytes.chunks(split) {
                f.update(chunk);
            }
            assert_eq!(f.finish(), whole, "chunks of {split}");
        }
        let mut changed = bytes.clone();
        changed[500] ^= 1;
        assert_ne!(of(&changed), whole);
        assert_ne!(of(&bytes[..999]), whole);
        let mut longer = bytes.clone();
        longer.push(0);
        assert_ne!(of(&longer), whole, "a zero byte added");
    }

    /// A run directory is a nextest run id's, the binary's name and its fingerprint, in the
    /// binary's profile directory; anything but a run id is refused before it reaches a path.
    #[test]
    fn a_run_directory_takes_only_a_run_id() {
        let binary = Path::new("target/debug/deps/hyperion_sim-0123");
        let run = "db70d0c7-e81a-432e-944c-e3bc8afdd282";
        assert_eq!(
            run_dir_for(run, binary, 0xab),
            Some(PathBuf::from(format!(
                "target/debug/{ROOT}/{run}-hyperion_sim-0123-00000000000000ab"
            )))
        );
        for refused in ["", "../x", "a/b", "a\\b", "a.b", &"a".repeat(65)] {
            assert_eq!(run_dir_for(refused, binary, 0xab), None, "{refused:?}");
        }
        assert_eq!(run_dir_for(run, Path::new("hyperion_sim"), 0xab), None);
    }

    /// Pruning keeps the newest runs, by their times, whatever order they are listed in, and none
    /// a day old.
    #[test]
    fn pruning_keeps_the_newest_runs_and_none_a_day_old() {
        let at = |s: u64| SystemTime::UNIX_EPOCH + Duration::from_secs(s);
        let runs = vec![
            (PathBuf::from("b"), at(20)),
            (PathBuf::from("a"), at(50)),
            (PathBuf::from("d"), at(10)),
            (PathBuf::from("c"), at(40)),
            (PathBuf::from("e"), at(40)),
            (PathBuf::from("f"), at(30)),
        ];
        let now = at(60);
        assert_eq!(
            stale_runs(runs.clone(), 4, now),
            vec![PathBuf::from("b"), PathBuf::from("d")]
        );
        assert_eq!(stale_runs(runs.clone(), 6, now), Vec::<PathBuf>::new());
        assert_eq!(stale_runs(runs.clone(), 0, now).len(), 6);
        // A day and a second after `b` was written, `b` and the older `d` are aged out; a day to
        // the second after `f` was, `f` is kept.
        let day = MAX_AGE.as_secs();
        assert_eq!(
            stale_runs(runs.clone(), 6, at(21 + day)),
            vec![PathBuf::from("b"), PathBuf::from("d")]
        );
        assert_eq!(stale_runs(runs.clone(), 6, at(30 + day)).len(), 2);
        // A clock set back ages nothing.
        assert_eq!(stale_runs(runs, 6, at(0)), Vec::<PathBuf>::new());
    }
}
