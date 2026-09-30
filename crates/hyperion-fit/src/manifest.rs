//! Manifests, the inputs hash, sim fingerprints and the lock file (plan 15, P15.T2 and Design
//! notes 6, 7, 8 and 11).
//!
//! A task's *manifest* is a TOML file in `crates/hyperion-fit/manifests/`: its seeds, sample sizes,
//! grids and parameter ranges under `[params]`, and the datasets it reads under `datasets`. A slow
//! task also has `<name>.smoke.toml`, a reduced run of a few seconds that the crate's tests execute.
//!
//! The *inputs hash* is the SHA-256 of the task's name, its revision, the manifest file's bytes and
//! the recorded hash of each dataset the manifest names, in that order ([`InputsHash::compute`]).
//! It does not cover source code: a change to a task's algorithm bumps its revision.
//!
//! The *lock file*, `crates/hyperion-fit/tables.lock`, records for every table its revision,
//! inputs hash, body hash, fingerprint values, `since-generator-version`, whether it is
//! provisional, and its acceptance figures, in name order ([`LockFile`]).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::data::{Dataset, LoadDatasetError, Provenance, load_dataset};
use crate::emit::{Workspace, hex, sha256_hex};

/// A task's manifest, as loaded from its TOML file.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    path: PathBuf,
    bytes: Vec<u8>,
    task: String,
    datasets: Vec<String>,
    params: toml::Table,
    data_root: PathBuf,
    data_dirs: BTreeMap<String, PathBuf>,
}

/// The fields of a manifest file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    task: String,
    #[serde(default)]
    datasets: Vec<String>,
    #[serde(default)]
    params: toml::Table,
}

impl Manifest {
    /// Loads the manifest at `path`.
    ///
    /// # Errors
    ///
    /// [`LoadManifestError::Io`] if the file cannot be read, [`LoadManifestError::Parse`] if it is
    /// not a manifest.
    pub fn load(path: &Path) -> Result<Self, LoadManifestError> {
        let bytes = fs::read(path).map_err(|source| LoadManifestError::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::from_bytes(path, bytes)
    }

    /// A manifest from its file's bytes, `path` naming it.
    ///
    /// # Errors
    ///
    /// [`LoadManifestError::Parse`] if the bytes are not a manifest.
    pub fn from_bytes(path: &Path, bytes: Vec<u8>) -> Result<Self, LoadManifestError> {
        let parse = |why: String| LoadManifestError::Parse {
            path: path.to_owned(),
            why,
        };
        let text = std::str::from_utf8(&bytes).map_err(|e| parse(e.to_string()))?;
        let file: ManifestFile = toml::from_str(text).map_err(|e| parse(e.to_string()))?;
        Ok(Self {
            path: path.to_owned(),
            task: file.task,
            datasets: file.datasets,
            params: file.params,
            data_root: path
                .parent()
                .and_then(Path::parent)
                .map_or_else(|| PathBuf::from("data"), |crate_dir| crate_dir.join("data")),
            data_dirs: BTreeMap::new(),
            bytes,
        })
    }

    /// Where the manifest was read from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The manifest file's bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The task it is for.
    #[must_use]
    pub fn task(&self) -> &str {
        &self.task
    }

    /// The datasets the task reads, by name.
    #[must_use]
    pub fn datasets(&self) -> &[String] {
        &self.datasets
    }

    /// The `[params]` table.
    #[must_use]
    pub fn params(&self) -> &toml::Table {
        &self.params
    }

    /// The data directory whose `<dataset>/PROVENANCE.toml` describe the manifest's datasets: the
    /// `data/` beside the `manifests/` directory the manifest is in.
    #[must_use]
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    /// Reads dataset `name` and verifies it against its provenance, from the directory given by
    /// [`with_data_dir`](Self::with_data_dir) or else its default one.
    ///
    /// # Errors
    ///
    /// As [`load_dataset`].
    pub fn load_dataset(&self, name: &str) -> Result<Dataset, LoadDatasetError> {
        load_dataset(&self.data_root, name, self.data_dir(name))
    }

    /// Reads the files of `dataset` from `dir` instead of the workspace's data directory (the
    /// command line's `--data`).
    #[must_use]
    pub fn with_data_dir(mut self, dataset: &str, dir: PathBuf) -> Self {
        self.data_dirs.insert(dataset.to_owned(), dir);
        self
    }

    /// The directory given for `dataset` by [`with_data_dir`](Self::with_data_dir), if any.
    #[must_use]
    pub fn data_dir(&self, dataset: &str) -> Option<&Path> {
        self.data_dirs.get(dataset).map(PathBuf::as_path)
    }

    /// The integer parameter `key`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if it is missing or not a non-negative integer.
    pub fn u64(&self, key: &str) -> Result<u64, ManifestParamError> {
        self.params
            .get(key)
            .and_then(toml::Value::as_integer)
            .and_then(|v| u64::try_from(v).ok())
            .ok_or_else(|| ManifestParamError::new(key, "a non-negative integer"))
    }

    /// The float parameter `key` (an integer is read as a float).
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if it is missing or not a number.
    pub fn f64(&self, key: &str) -> Result<f64, ManifestParamError> {
        match self.params.get(key) {
            Some(toml::Value::Float(v)) => Ok(*v),
            Some(toml::Value::Integer(v)) => {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a manifest's integers are small counts, exact in an f64"
                )]
                let v = *v as f64;
                Ok(v)
            }
            _ => Err(ManifestParamError::new(key, "a number")),
        }
    }

    /// The string parameter `key`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if it is missing or not a string.
    pub fn str(&self, key: &str) -> Result<&str, ManifestParamError> {
        self.params
            .get(key)
            .and_then(toml::Value::as_str)
            .ok_or_else(|| ManifestParamError::new(key, "a string"))
    }

    /// Checks that `[params]` holds exactly `expected`, for a task whose parameters are constants
    /// of its code: its manifest records them, and a manifest edited without the code is caught.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] naming the first key that is missing, extra or different.
    pub fn expect_params(&self, expected: &toml::Table) -> Result<(), ManifestParamError> {
        for (key, value) in expected {
            match self.params.get(key) {
                Some(found) if found == value => {}
                Some(_) => {
                    return Err(ManifestParamError::new(
                        key,
                        &format!("{value}, the value in the task's code"),
                    ));
                }
                None => return Err(ManifestParamError::new(key, "present")),
            }
        }
        match self.params.keys().find(|k| !expected.contains_key(*k)) {
            Some(extra) => Err(ManifestParamError::new(
                extra,
                "absent: the task reads no such key",
            )),
            None => Ok(()),
        }
    }
}

/// A manifest could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum LoadManifestError {
    /// The file could not be read.
    #[error("cannot read the manifest {}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// Why.
        source: io::Error,
    },
    /// The file is not a manifest.
    #[error("the manifest {} is malformed: {why}", path.display())]
    Parse {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        why: String,
    },
}

/// A manifest's parameter is missing or of the wrong kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
#[error("the manifest's parameter `{key}` must be {expected}")]
pub struct ManifestParamError {
    key: String,
    expected: String,
}

impl ManifestParamError {
    /// Parameter `key` should have been `expected`.
    #[must_use]
    pub fn new(key: &str, expected: &str) -> Self {
        Self {
            key: key.to_owned(),
            expected: expected.to_owned(),
        }
    }
}

/// The SHA-256 of a task's inputs (Design note 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InputsHash([u8; 32]);

impl InputsHash {
    /// The hash of task `name` at `revision` run on `manifest`, whose datasets' recorded hashes are
    /// `dataset_hashes`, in the manifest's order. Each field is preceded by its length as eight
    /// little-endian bytes, so that no two different inputs run together into one byte string.
    ///
    /// # Panics
    ///
    /// Never: no field is 2⁶⁴ bytes long.
    #[must_use]
    pub fn compute(name: &str, revision: u32, manifest: &[u8], dataset_hashes: &[String]) -> Self {
        let mut hasher = Sha256::new();
        let mut field = |bytes: &[u8]| {
            let len = u64::try_from(bytes.len()).expect("a field is shorter than 2^64 bytes");
            hasher.update(len.to_le_bytes());
            hasher.update(bytes);
        };
        field(name.as_bytes());
        field(&revision.to_le_bytes());
        field(manifest);
        for hash in dataset_hashes {
            field(hash.as_bytes());
        }
        Self(hasher.finalize().into())
    }

    /// The hash of `task`'s inputs with `manifest`, the datasets' recorded hashes read from their
    /// `PROVENANCE.toml` under the workspace's data directory.
    ///
    /// # Errors
    ///
    /// As [`Provenance::load`].
    pub fn of_task(
        name: &str,
        revision: u32,
        manifest: &Manifest,
        workspace: &Workspace,
    ) -> Result<(Self, Vec<(String, String)>), LoadDatasetError> {
        let mut hashes = Vec::with_capacity(manifest.datasets().len());
        for dataset in manifest.datasets() {
            let provenance = Provenance::load(&workspace.data_dir, dataset)?;
            hashes.push((dataset.clone(), provenance.recorded_hash()));
        }
        let recorded: Vec<String> = hashes.iter().map(|(_, h)| h.clone()).collect();
        Ok((
            Self::compute(name, revision, manifest.bytes(), &recorded),
            hashes,
        ))
    }

    /// The hash as 64 hex digits.
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex(&self.0)
    }
}

/// A task's sim fingerprint (Design note 7): probe values of the generator's own code that its
/// table depends on, each named. Empty for a task that uses only `math`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimFingerprint {
    probes: Vec<(String, f64)>,
}

/// The relative tolerance within which a recomputed probe matches the lock file's (Design note 7).
pub const FINGERPRINT_TOLERANCE: f64 = 1e-9;

impl SimFingerprint {
    /// The empty fingerprint.
    #[must_use]
    pub const fn none() -> Self {
        Self { probes: Vec::new() }
    }

    /// A fingerprint of named probe values.
    #[must_use]
    pub fn new(probes: Vec<(String, f64)>) -> Self {
        Self { probes }
    }

    /// The probes, in order.
    #[must_use]
    pub fn probes(&self) -> &[(String, f64)] {
        &self.probes
    }

    /// The probe values, in order.
    #[must_use]
    pub fn values(&self) -> Vec<f64> {
        self.probes.iter().map(|&(_, v)| v).collect()
    }

    /// Whether there are no probes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }

    /// The SHA-256 of `values`, each written as its shortest round-trip decimal and a newline, as
    /// 64 hex digits; `None` for no values. Decimal text, not bits: it is the same on every
    /// platform, and float bits are never hashed.
    #[must_use]
    pub fn hash_of(values: &[f64]) -> Option<String> {
        if values.is_empty() {
            return None;
        }
        let text = values.iter().fold(String::new(), |mut text, v| {
            let _ = writeln!(text, "{v:?}");
            text
        });
        Some(sha256_hex(text.as_bytes()))
    }

    /// Whether `recomputed` matches `recorded` value by value within [`FINGERPRINT_TOLERANCE`],
    /// relative; the first mismatch's index if not.
    #[must_use]
    pub fn first_drift(recorded: &[f64], recomputed: &[f64]) -> Option<usize> {
        if recorded.len() != recomputed.len() {
            return Some(recorded.len().min(recomputed.len()));
        }
        recorded.iter().zip(recomputed).position(|(&a, &b)| {
            let scale = a.abs().max(b.abs());
            let within = (a - b).abs() <= FINGERPRINT_TOLERANCE * scale;
            !within
        })
    }
}

/// One table's entry in the lock file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockEntry {
    /// The table's name: its task's, or its module's for a provisional table no task makes.
    pub name: String,
    /// Its file, relative to the sim's `tables/`.
    pub path: String,
    /// The task that makes it, if one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The table's revision.
    pub revision: u32,
    /// The inputs hash in its header, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs_sha256: Option<String>,
    /// The SHA-256 of its body.
    pub body_sha256: String,
    /// The fingerprint's probe values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fingerprint: Vec<f64>,
    /// `GENERATOR_VERSION` at which this revision took effect.
    pub since_generator_version: u32,
    /// Whether a scratch stand-in is in place.
    pub provisional: bool,
    /// The acceptance test's measured figures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<String>,
}

/// The lock file: every table's entry, in name order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LockFile {
    entries: Vec<LockEntry>,
}

/// The lock file's TOML form.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockFileToml {
    #[serde(default)]
    table: Vec<LockEntry>,
}

/// The lock file's opening comment.
const LOCK_PREAMBLE: &str = "# The fitted tables' lock file (plan 15, P15.T2): written by `hyperion-fit run`, read by\n\
                             # `hyperion-fit check`. Do not edit.\n\n";

impl LockFile {
    /// Reads the lock file at `path`; a missing file is an empty lock.
    ///
    /// # Errors
    ///
    /// [`ReadLockError::Io`] if it cannot be read, [`ReadLockError::Parse`] if it is malformed or
    /// its entries are not in name order.
    pub fn read(path: &Path) -> Result<Self, ReadLockError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(ReadLockError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let parse = |why: String| ReadLockError::Parse {
            path: path.to_owned(),
            why,
        };
        let file: LockFileToml = toml::from_str(&text).map_err(|e| parse(e.to_string()))?;
        if !file.table.windows(2).all(|w| w[0].name < w[1].name) {
            return Err(parse("entries are not in strict name order".to_owned()));
        }
        Ok(Self {
            entries: file.table,
        })
    }

    /// The entries, in name order.
    #[must_use]
    pub fn entries(&self) -> &[LockEntry] {
        &self.entries
    }

    /// The entry of table `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&LockEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Adds `entry`, or replaces the entry of the same name, keeping name order.
    pub fn upsert(&mut self, entry: LockEntry) {
        match self
            .entries
            .binary_search_by(|e| e.name.as_str().cmp(&entry.name))
        {
            Ok(i) => self.entries[i] = entry,
            Err(i) => self.entries.insert(i, entry),
        }
    }

    /// The lock file's text.
    ///
    /// # Panics
    ///
    /// Never: every entry is plain TOML (a fingerprint value is finite, being a fit's input).
    #[must_use]
    pub fn to_text(&self) -> String {
        let body = toml::to_string(&LockFileToml {
            table: self.entries.clone(),
        })
        .expect("a lock entry serialises as TOML");
        format!("{LOCK_PREAMBLE}{body}")
    }

    /// Writes the lock file to `path`.
    ///
    /// # Errors
    ///
    /// [`ReadLockError::Io`] if it cannot be written.
    pub fn write(&self, path: &Path) -> Result<(), ReadLockError> {
        fs::write(path, self.to_text()).map_err(|source| ReadLockError::Io {
            path: path.to_owned(),
            source,
        })
    }
}

/// The lock file could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum ReadLockError {
    /// The file could not be read or written.
    #[error("cannot read or write the lock file {}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// Why.
        source: io::Error,
    },
    /// The file is malformed.
    #[error("the lock file {} is malformed: {why}", path.display())]
    Parse {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        why: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> LockEntry {
        LockEntry {
            name: name.to_owned(),
            path: format!("{name}.rs"),
            task: Some(name.to_owned()),
            revision: 1,
            inputs_sha256: Some("0".repeat(64)),
            body_sha256: "1".repeat(64),
            fingerprint: vec![1.5, -2.0e-9],
            since_generator_version: 11,
            provisional: false,
            acceptance: Some("within 1%".to_owned()),
        }
    }

    #[test]
    fn the_lock_file_round_trips_in_name_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tables.lock");
        let mut lock = LockFile::default();
        lock.upsert(entry("mge"));
        lock.upsert(entry("chabrier"));
        let mut replaced = entry("mge");
        replaced.revision = 2;
        lock.upsert(replaced);
        lock.write(&path).unwrap();
        let back = LockFile::read(&path).unwrap();
        assert_eq!(back, lock);
        let names: Vec<&str> = back.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["chabrier", "mge"]);
        assert_eq!(back.get("mge").unwrap().revision, 2);
        assert!(back.to_text().starts_with("# The fitted tables' lock file"));
    }

    #[test]
    fn a_missing_lock_file_is_empty_and_a_disordered_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tables.lock");
        assert_eq!(LockFile::read(&path).unwrap(), LockFile::default());
        let mut text = LockFile {
            entries: vec![entry("mge")],
        }
        .to_text();
        text.push_str(
            &toml::to_string(&LockFileToml {
                table: vec![entry("chabrier")],
            })
            .unwrap(),
        );
        fs::write(&path, text).unwrap();
        assert!(matches!(
            LockFile::read(&path),
            Err(ReadLockError::Parse { .. })
        ));
    }

    #[test]
    fn the_inputs_hash_covers_name_revision_manifest_and_data() {
        let base = InputsHash::compute("mge", 1, b"task = \"mge\"\n", &[]);
        assert_eq!(
            base,
            InputsHash::compute("mge", 1, b"task = \"mge\"\n", &[])
        );
        for other in [
            InputsHash::compute("mgf", 1, b"task = \"mge\"\n", &[]),
            InputsHash::compute("mge", 2, b"task = \"mge\"\n", &[]),
            InputsHash::compute("mge", 1, b"task = \"mge\" \n", &[]),
            InputsHash::compute("mge", 1, b"task = \"mge\"\n", &["ab".to_owned()]),
        ] {
            assert_ne!(base, other);
        }
        assert_eq!(base.to_hex().len(), 64);
    }

    #[test]
    fn a_manifests_params_are_read_and_checked() {
        let manifest = Manifest::from_bytes(
            Path::new("toy.toml"),
            b"task = \"toy\"\ndatasets = [\"census\"]\n[params]\ndraws = 200\nscale = 0.5\n"
                .to_vec(),
        )
        .unwrap();
        assert_eq!(manifest.task(), "toy");
        assert_eq!(manifest.datasets(), ["census"]);
        assert_eq!(manifest.u64("draws").unwrap(), 200);
        assert!(manifest.f64("scale").unwrap().total_cmp(&0.5).is_eq());
        assert!(manifest.f64("draws").unwrap().total_cmp(&200.0).is_eq());
        assert!(manifest.u64("scale").is_err());
        let mut expected = toml::Table::new();
        expected.insert("draws".to_owned(), toml::Value::Integer(200));
        expected.insert("scale".to_owned(), toml::Value::Float(0.5));
        manifest.expect_params(&expected).unwrap();
        expected.insert("scale".to_owned(), toml::Value::Float(0.25));
        assert!(manifest.expect_params(&expected).is_err());
        expected.remove("scale");
        assert!(manifest.expect_params(&expected).is_err());
        assert!(matches!(
            Manifest::from_bytes(Path::new("x.toml"), b"task = 3\n".to_vec()),
            Err(LoadManifestError::Parse { .. })
        ));
    }

    #[test]
    fn fingerprints_match_within_their_tolerance() {
        let recorded = [1.0, 2.0];
        assert_eq!(SimFingerprint::first_drift(&recorded, &[1.0, 2.0]), None);
        assert_eq!(
            SimFingerprint::first_drift(&recorded, &[1.0 + 1e-12, 2.0]),
            None
        );
        assert_eq!(
            SimFingerprint::first_drift(&recorded, &[1.0, 2.0 + 1e-6]),
            Some(1)
        );
        assert_eq!(SimFingerprint::first_drift(&recorded, &[1.0]), Some(1));
        assert_eq!(SimFingerprint::hash_of(&[]), None);
        assert_ne!(
            SimFingerprint::hash_of(&[1.0]),
            SimFingerprint::hash_of(&[1.0 + f64::EPSILON])
        );
    }
}
