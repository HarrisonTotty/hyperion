//! Saves on disk: one directory per universe, holding its identity file.
//!
//! The layout is `<data_dir>/universes/<id as 16 hex digits>/universe.json` (plan 04, design note
//! 17). The file holds the save format, the ID, the name, the seed and the generator version, with
//! the two `u64`s in the wire's 16-hex-digit form, and nothing generated: a universe is
//! `(seed, generator_version)`. Unknown fields are ignored, so that a later format 1 writer may add
//! some; a format above 1 is refused.
//!
//! The directory is also the home of the overlays of later plans. The names `pinned/`, `deltas/`,
//! `enrichment/`, `knowledge/` and `session.json` are reserved for them and not created here.
//!
//! Everything here is blocking file I/O. The registry calls it through
//! [`tokio::task::spawn_blocking`], never on the async runtime.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::{error::Error, fmt};

use hyperion_protocol::SeedHex;
use hyperion_sim::GeneratorVersion;
use serde::{Deserialize, Serialize};

use super::{UniverseId, UniverseName};

/// The save format this build writes and the only one it reads.
pub const SAVE_FORMAT: u32 = 1;

/// The directory under the data directory that holds one directory per universe.
const UNIVERSES_DIR: &str = "universes";

/// The identity file inside a universe's directory.
const SAVE_FILE: &str = "universe.json";

/// Where the identity file is written before it is renamed into place.
const TEMP_FILE: &str = "universe.json.tmp";

/// A universe's identity as stored on disk.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SavedUniverse {
    id: UniverseId,
    name: UniverseName,
    seed: u64,
    generator_version: GeneratorVersion,
}

impl SavedUniverse {
    /// A save of the universe `(seed, generator_version)`, named `name`, with the identity `id`.
    #[must_use]
    pub fn new(
        id: UniverseId,
        name: UniverseName,
        seed: u64,
        generator_version: GeneratorVersion,
    ) -> Self {
        Self {
            id,
            name,
            seed,
            generator_version,
        }
    }

    /// The save's identity.
    #[must_use]
    pub fn id(&self) -> UniverseId {
        self.id
    }

    /// The operator-given name.
    #[must_use]
    pub fn name(&self) -> &UniverseName {
        &self.name
    }

    /// The seed the galaxy is generated from.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The generator version the universe was created with.
    #[must_use]
    pub fn generator_version(&self) -> GeneratorVersion {
        self.generator_version
    }
}

/// A save written in a format this build cannot read. It is not loaded, and its directory is
/// left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnsupportedSave {
    id: UniverseId,
    format: u32,
}

impl UnsupportedSave {
    /// The ID named by the save's directory.
    #[must_use]
    pub fn id(&self) -> UniverseId {
        self.id
    }

    /// The format the file declares.
    #[must_use]
    pub fn format(&self) -> u32 {
        self.format
    }
}

/// What a scan of the store found, each list sorted by ID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreScan {
    saves: Vec<SavedUniverse>,
    unsupported: Vec<UnsupportedSave>,
}

impl StoreScan {
    /// The saves this build can read.
    #[must_use]
    pub fn saves(&self) -> &[SavedUniverse] {
        &self.saves
    }

    /// The saves written in a later format.
    #[must_use]
    pub fn unsupported(&self) -> &[UnsupportedSave] {
        &self.unsupported
    }

    /// Both lists, by value.
    pub(crate) fn into_parts(self) -> (Vec<SavedUniverse>, Vec<UnsupportedSave>) {
        (self.saves, self.unsupported)
    }
}

/// The saves under one data directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UniverseStore {
    data_dir: PathBuf,
}

impl UniverseStore {
    /// The store under `data_dir`. Nothing is read or created until it is used.
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }

    /// The data directory.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Where the identity file of universe `id` lives.
    #[must_use]
    pub fn save_path(&self, id: UniverseId) -> PathBuf {
        self.universe_dir(id).join(SAVE_FILE)
    }

    /// Reads every save under the data directory.
    ///
    /// A directory whose name is not 16 hex digits, whose file is missing, unreadable or
    /// malformed, or whose file names another ID is skipped and logged at `warn`: one damaged
    /// save must not keep the others from loading. A save in a later format is logged and listed
    /// in [`StoreScan::unsupported`]. A data directory that does not exist yet holds no saves, and
    /// the scan creates nothing.
    ///
    /// # Errors
    ///
    /// [`ScanStoreError`] if the directory of universes exists but cannot be listed.
    pub fn scan(&self) -> Result<StoreScan, ScanStoreError> {
        let universes = self.universes_dir();
        let entries = match fs::read_dir(&universes) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(StoreScan::default());
            }
            Err(source) => {
                return Err(ScanStoreError {
                    path: universes,
                    source,
                });
            }
        };

        let mut scan = StoreScan::default();
        for entry in entries {
            let entry = entry.map_err(|source| ScanStoreError {
                path: universes.clone(),
                source,
            })?;
            let path = entry.path();
            let Some(dir_id) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<UniverseId>().ok())
            else {
                tracing::warn!(path = %path.display(), "skipping a directory not named by a universe id");
                continue;
            };
            match read_save(&path.join(SAVE_FILE), dir_id) {
                Ok(save) => scan.saves.push(save),
                Err(ReadSaveError::UnsupportedFormat { format }) => {
                    tracing::warn!(
                        path = %path.display(),
                        format,
                        supported = SAVE_FORMAT,
                        "not loading a save written in a later format"
                    );
                    scan.unsupported
                        .push(UnsupportedSave { id: dir_id, format });
                }
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "skipping an unreadable save");
                }
            }
        }
        scan.saves.sort_by_key(SavedUniverse::id);
        scan.unsupported.sort_by_key(UnsupportedSave::id);
        Ok(scan)
    }

    /// Writes a new save.
    ///
    /// The file is written to a temporary name, synced, renamed into place, and the directories
    /// are synced, so that a crash leaves either the whole save or none of it. A save is never
    /// rewritten: if the universe's directory exists already, nothing is touched. When a step
    /// fails, what was written is removed again.
    ///
    /// # Errors
    ///
    /// [`WriteSaveError::AlreadyExists`] if the universe's directory exists, and
    /// [`WriteSaveError::Io`] if a step of the write fails.
    pub fn write(&self, save: &SavedUniverse) -> Result<(), WriteSaveError> {
        let universes = self.universes_dir();
        fs::create_dir_all(&universes).map_err(io_error("create directory", &universes))?;
        let dir = self.universe_dir(save.id);
        if let Err(source) = fs::create_dir(&dir) {
            return Err(if source.kind() == io::ErrorKind::AlreadyExists {
                WriteSaveError::AlreadyExists { id: save.id }
            } else {
                WriteSaveError::Io {
                    operation: "create directory",
                    path: dir,
                    source,
                }
            });
        }
        let written = write_file_durably(&dir, &render(save)).and_then(|()| {
            // The new directory's entry lives in its parent.
            sync_directory(&universes)
        });
        if written.is_err() {
            remove_partial_save(&dir);
        }
        written
    }

    fn universes_dir(&self) -> PathBuf {
        self.data_dir.join(UNIVERSES_DIR)
    }

    fn universe_dir(&self, id: UniverseId) -> PathBuf {
        self.universes_dir().join(id.to_string())
    }
}

/// The file format, version 1. Field order is the order on disk.
#[derive(Debug, Serialize, Deserialize)]
struct SaveFileV1 {
    format: u32,
    id: String,
    name: String,
    seed: String,
    generator_version: u32,
}

/// Just the format, read first, so that a later format's other fields need not parse as ours.
#[derive(Debug, Deserialize)]
struct FormatProbe {
    format: u32,
}

/// The text of `save`'s identity file: pretty-printed, since operators read and edit it.
fn render(save: &SavedUniverse) -> String {
    let file = SaveFileV1 {
        format: SAVE_FORMAT,
        id: save.id.to_string(),
        name: save.name.as_str().to_owned(),
        seed: SeedHex::from_u64(save.seed).to_string(),
        generator_version: save.generator_version.get(),
    };
    let mut text =
        serde_json::to_string_pretty(&file).expect("a struct of strings and integers serialises");
    text.push('\n');
    text
}

/// Reads and checks the identity file at `path` in the directory of `dir_id`.
fn read_save(path: &Path, dir_id: UniverseId) -> Result<SavedUniverse, ReadSaveError> {
    let text = fs::read_to_string(path).map_err(ReadSaveError::Read)?;
    let probe: FormatProbe = serde_json::from_str(&text).map_err(ReadSaveError::Malformed)?;
    match probe.format {
        SAVE_FORMAT => {}
        0 => return Err(ReadSaveError::InvalidField { field: "format" }),
        format => return Err(ReadSaveError::UnsupportedFormat { format }),
    }
    let file: SaveFileV1 = serde_json::from_str(&text).map_err(ReadSaveError::Malformed)?;
    let id = file
        .id
        .parse::<UniverseId>()
        .map_err(|_| ReadSaveError::InvalidField { field: "id" })?;
    if id != dir_id {
        return Err(ReadSaveError::IdMismatch { file: id });
    }
    let name = file
        .name
        .parse::<UniverseName>()
        .map_err(|_| ReadSaveError::InvalidField { field: "name" })?;
    let seed = SeedHex::try_from(file.seed)
        .map_err(|_| ReadSaveError::InvalidField { field: "seed" })?
        .to_u64();
    Ok(SavedUniverse {
        id,
        name,
        seed,
        generator_version: GeneratorVersion::new(file.generator_version),
    })
}

/// Why one save was not loaded. Logged, never returned: one bad save does not fail the scan.
#[derive(Debug)]
enum ReadSaveError {
    Read(io::Error),
    Malformed(serde_json::Error),
    UnsupportedFormat { format: u32 },
    InvalidField { field: &'static str },
    IdMismatch { file: UniverseId },
}

impl fmt::Display for ReadSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read the file: {error}"),
            Self::Malformed(error) => write!(f, "malformed json: {error}"),
            Self::UnsupportedFormat { format } => write!(f, "unsupported format {format}"),
            Self::InvalidField { field } => write!(f, "invalid field `{field}`"),
            Self::IdMismatch { file } => {
                write!(f, "the file names universe {file}, not its directory's")
            }
        }
    }
}

/// Writes `universe.json` in `dir` by way of a synced temporary file and a rename, then syncs
/// `dir` so that the rename is durable.
fn write_file_durably(dir: &Path, contents: &str) -> Result<(), WriteSaveError> {
    let temp = dir.join(TEMP_FILE);
    let target = dir.join(SAVE_FILE);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(io_error("create", &temp))?;
    file.write_all(contents.as_bytes())
        .map_err(io_error("write", &temp))?;
    file.sync_all().map_err(io_error("sync", &temp))?;
    drop(file);
    fs::rename(&temp, &target).map_err(io_error("rename", &temp))?;
    sync_directory(dir)
}

/// Makes the entries of `dir` durable.
#[cfg(unix)]
fn sync_directory(dir: &Path) -> Result<(), WriteSaveError> {
    File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(io_error("sync directory", dir))
}

/// Makes the entries of `dir` durable. Other platforms cannot open a directory as a file; their
/// file systems journal the rename.
#[cfg(not(unix))]
fn sync_directory(_dir: &Path) -> Result<(), WriteSaveError> {
    Ok(())
}

/// Removes what a failed write left in `dir`, and `dir` itself. Best effort: a failure is logged,
/// and the scan skips a directory without a valid file.
fn remove_partial_save(dir: &Path) {
    for file in [TEMP_FILE, SAVE_FILE] {
        let path = dir.join(file);
        if let Err(error) = fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(path = %path.display(), %error, "failed to remove a partial save");
        }
    }
    if let Err(error) = fs::remove_dir(dir) {
        tracing::warn!(path = %dir.display(), %error, "failed to remove a partial save");
    }
}

fn io_error(operation: &'static str, path: &Path) -> impl FnOnce(io::Error) -> WriteSaveError {
    let path = path.to_path_buf();
    move |source| WriteSaveError::Io {
        operation,
        path,
        source,
    }
}

/// The directory of universes exists but could not be listed.
#[derive(Debug)]
pub struct ScanStoreError {
    path: PathBuf,
    source: io::Error,
}

impl ScanStoreError {
    /// The directory that could not be listed.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for ScanStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to list saves in {}", self.path.display())
    }
}

impl Error for ScanStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// A save could not be written.
#[derive(Debug)]
pub enum WriteSaveError {
    /// The universe's directory exists already; nothing was written.
    AlreadyExists {
        /// The universe whose directory exists.
        id: UniverseId,
    },
    /// A step of the write failed, and what it had written was removed.
    Io {
        /// What was being done: `"create directory"`, `"write"`, `"sync"`, `"rename"` and so on.
        operation: &'static str,
        /// The file or directory it was done to.
        path: PathBuf,
        /// The operating system's error.
        source: io::Error,
    },
}

impl fmt::Display for WriteSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists { id } => write!(f, "a save for universe {id} exists already"),
            Self::Io {
                operation, path, ..
            } => write!(f, "failed to {operation} {}", path.display()),
        }
    }
}

impl Error for WriteSaveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AlreadyExists { .. } => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save(id: u64, name: &str, seed: u64) -> SavedUniverse {
        SavedUniverse::new(
            UniverseId::new(id),
            name.parse().unwrap(),
            seed,
            GeneratorVersion::new(7),
        )
    }

    fn store() -> (tempfile::TempDir, UniverseStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = UniverseStore::new(dir.path());
        (dir, store)
    }

    /// Writes `text` as the identity file of the directory named `dir_name`.
    fn plant(store: &UniverseStore, dir_name: &str, text: &str) {
        let dir = store.universes_dir().join(dir_name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SAVE_FILE), text).unwrap();
    }

    #[test]
    fn write_then_scan_round_trips() {
        let (_dir, store) = store();
        let saves = [save(0xab, "Alpha", 1234), save(0x12, "Beta", u64::MAX)];
        for save in &saves {
            store.write(save).unwrap();
        }
        let scan = store.scan().unwrap();
        assert_eq!(scan.saves(), [saves[1].clone(), saves[0].clone()]);
        assert!(scan.unsupported().is_empty());
    }

    #[test]
    fn the_json_on_disk_equals_the_pinned_form() {
        let (_dir, store) = store();
        store.write(&save(0xab, "Kepler Reach", 0x4d2)).unwrap();
        let text = fs::read_to_string(store.save_path(UniverseId::new(0xab))).unwrap();
        assert_eq!(
            text,
            r#"{
  "format": 1,
  "id": "00000000000000ab",
  "name": "Kepler Reach",
  "seed": "00000000000004d2",
  "generator_version": 7
}
"#
        );
    }

    #[test]
    fn no_temporary_file_is_left_behind() {
        let (_dir, store) = store();
        store.write(&save(0xab, "Alpha", 1)).unwrap();
        let dir = store.universe_dir(UniverseId::new(0xab));
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [SAVE_FILE]);
    }

    #[test]
    fn a_file_with_an_unknown_field_loads() {
        let (_dir, store) = store();
        plant(
            &store,
            "00000000000000ab",
            r#"{"format":1,"id":"00000000000000ab","name":"Alpha","seed":"00000000000004d2",
                "generator_version":7,"crew":["Ada"]}"#,
        );
        assert_eq!(store.scan().unwrap().saves(), [save(0xab, "Alpha", 0x4d2)]);
    }

    #[test]
    fn a_later_format_is_reported_and_not_loaded() {
        let (_dir, store) = store();
        plant(
            &store,
            "00000000000000ab",
            r#"{"format":2,"identity":{"id":"00000000000000ab"}}"#,
        );
        store.write(&save(0x12, "Beta", 1)).unwrap();
        let scan = store.scan().unwrap();
        assert_eq!(scan.saves(), [save(0x12, "Beta", 1)]);
        assert_eq!(
            scan.unsupported(),
            [UnsupportedSave {
                id: UniverseId::new(0xab),
                format: 2
            }]
        );
    }

    #[test]
    fn a_truncated_file_is_skipped_and_the_rest_load() {
        let (_dir, store) = store();
        store.write(&save(0x12, "Beta", 1)).unwrap();
        store.write(&save(0xab, "Alpha", 2)).unwrap();
        let path = store.save_path(UniverseId::new(0xab));
        let text = fs::read_to_string(&path).unwrap();
        fs::write(&path, &text[..text.len() / 2]).unwrap();
        let scan = store.scan().unwrap();
        assert_eq!(scan.saves(), [save(0x12, "Beta", 1)]);
        assert!(scan.unsupported().is_empty());
    }

    #[test]
    fn saves_with_bad_names_or_contents_are_skipped() {
        let (_dir, store) = store();
        store.write(&save(0x12, "Beta", 1)).unwrap();
        let good = fs::read_to_string(store.save_path(UniverseId::new(0x12))).unwrap();
        // Not named by an ID: upper case, wrong length.
        plant(&store, "00000000000000AB", &good);
        plant(&store, "12", &good);
        // Named by another ID than the one in its file.
        plant(&store, "0000000000000013", &good);
        // No file at all.
        fs::create_dir_all(store.universes_dir().join("0000000000000014")).unwrap();
        // Fields that do not parse.
        plant(
            &store,
            "0000000000000015",
            r#"{"format":1,"id":"0000000000000015","name":"","seed":"0000000000000001",
                "generator_version":7}"#,
        );
        plant(
            &store,
            "0000000000000016",
            r#"{"format":1,"id":"0000000000000016","name":"Gamma","seed":"1",
                "generator_version":7}"#,
        );
        plant(&store, "0000000000000017", r#"{"format":0}"#);
        let scan = store.scan().unwrap();
        assert_eq!(scan.saves(), [save(0x12, "Beta", 1)]);
        assert!(scan.unsupported().is_empty());
    }

    #[test]
    fn scanning_a_missing_data_directory_finds_nothing_and_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("hyperion-data");
        let store = UniverseStore::new(&data_dir);
        assert_eq!(store.scan().unwrap(), StoreScan::default());
        assert!(!data_dir.exists());
    }

    #[test]
    fn an_existing_save_is_never_rewritten() {
        let (_dir, store) = store();
        store.write(&save(0xab, "Alpha", 1)).unwrap();
        let path = store.save_path(UniverseId::new(0xab));
        let before = fs::read(&path).unwrap();
        let error = store.write(&save(0xab, "Other", 2)).unwrap_err();
        assert!(
            matches!(error, WriteSaveError::AlreadyExists { id } if id == UniverseId::new(0xab)),
            "{error:?}"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn a_failed_write_reports_the_operation_and_path() {
        let (dir, store) = store();
        // A file where the directory of universes belongs.
        fs::write(dir.path().join(UNIVERSES_DIR), "").unwrap();
        let error = store.write(&save(0xab, "Alpha", 1)).unwrap_err();
        match error {
            WriteSaveError::Io {
                operation, path, ..
            } => {
                assert_eq!(operation, "create directory");
                assert_eq!(path, dir.path().join(UNIVERSES_DIR));
            }
            WriteSaveError::AlreadyExists { .. } => panic!("unexpected {error:?}"),
        }
    }
}
