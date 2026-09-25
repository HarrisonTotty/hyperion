//! External datasets and their provenance (plan 15, Design note 12).
//!
//! A dataset lives under `crates/hyperion-fit/data/<dataset>/` with a `PROVENANCE.toml` giving the
//! citation, the URL, the retrieval date, the release, the licence or terms of use as found, and
//! the SHA-256 of each file. It is of one of two classes:
//!
//! - *committed*: small files whose terms allow redistribution, or that hold only measured facts,
//!   kept beside their `PROVENANCE.toml`;
//! - *fetched*: large or restrictively licensed sets, of which only the `PROVENANCE.toml` is
//!   committed; the raw files go to `crates/hyperion-fit/data/cache/<dataset>/`, which git ignores.
//!
//! The *recorded hash* of a dataset, which a table's inputs hash covers, is the SHA-256 of its file
//! list as `PROVENANCE.toml` records it, so the inputs hash can be checked without the files.
//! [`load_dataset`] reads the files and verifies each against its recorded hash.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::emit::sha256_hex;

/// Where a dataset's files are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasetClass {
    /// Kept in the repository beside `PROVENANCE.toml`.
    Committed,
    /// Downloaded into the git-ignored `data/cache/<dataset>/`.
    Fetched,
}

/// One file of a dataset and its SHA-256.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetFile {
    /// The file's name, within the dataset's directory.
    pub name: String,
    /// Its SHA-256, 64 hex digits.
    pub sha256: String,
}

/// A dataset's `PROVENANCE.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// The dataset's name, which is its directory's.
    pub dataset: String,
    /// Committed or fetched.
    pub class: DatasetClass,
    /// The work to cite.
    pub citation: String,
    /// Where it was retrieved from.
    pub url: String,
    /// When, as an ISO 8601 date.
    pub retrieved: String,
    /// The release or version retrieved.
    pub release: String,
    /// The licence or terms of use, as found.
    pub licence: String,
    /// The files, in name order.
    pub files: Vec<DatasetFile>,
}

impl Provenance {
    /// Reads `<data_dir>/<dataset>/PROVENANCE.toml`.
    ///
    /// # Errors
    ///
    /// [`LoadDatasetError::Io`] if it cannot be read, [`LoadDatasetError::Provenance`] if it is
    /// malformed, names another dataset, or lists its files out of name order.
    pub fn load(data_dir: &Path, dataset: &str) -> Result<Self, LoadDatasetError> {
        let path = data_dir.join(dataset).join("PROVENANCE.toml");
        let text = fs::read_to_string(&path).map_err(|source| LoadDatasetError::Io {
            path: path.clone(),
            source,
        })?;
        let bad = |why: String| LoadDatasetError::Provenance {
            path: path.clone(),
            why,
        };
        let provenance: Self = toml::from_str(&text).map_err(|e| bad(e.to_string()))?;
        if provenance.dataset != dataset {
            return Err(bad(format!(
                "it names the dataset `{}`",
                provenance.dataset
            )));
        }
        if !provenance.files.windows(2).all(|w| w[0].name < w[1].name) {
            return Err(bad("its files are not in strict name order".to_owned()));
        }
        Ok(provenance)
    }

    /// The recorded hash: the SHA-256 of each file's name and hash, one `<name> <sha256>` line
    /// each, in the listed order.
    #[must_use]
    pub fn recorded_hash(&self) -> String {
        let list = self.files.iter().fold(String::new(), |mut list, f| {
            let _ = writeln!(list, "{} {}", f.name, f.sha256);
            list
        });
        sha256_hex(list.as_bytes())
    }

    /// The directory the files are read from, unless another is given: beside `PROVENANCE.toml`
    /// for a committed dataset, under `data/cache/` for a fetched one.
    #[must_use]
    pub fn default_dir(&self, data_dir: &Path) -> PathBuf {
        match self.class {
            DatasetClass::Committed => data_dir.join(&self.dataset),
            DatasetClass::Fetched => data_dir.join("cache").join(&self.dataset),
        }
    }
}

/// A dataset read and verified: its provenance and each file's bytes, in the listed order.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dataset {
    /// Its provenance.
    pub provenance: Provenance,
    /// The directory its files were read from.
    pub dir: PathBuf,
    /// Each file's name and bytes.
    pub files: Vec<(String, Vec<u8>)>,
}

/// Reads dataset `name` from `dir`, or from its default directory under `data_dir`, and checks
/// every file against the hash its `PROVENANCE.toml` records.
///
/// # Errors
///
/// As [`Provenance::load`]; [`LoadDatasetError::Io`] for a file that cannot be read;
/// [`LoadDatasetError::HashMismatch`] for a file whose bytes are not the recorded ones.
pub fn load_dataset(
    data_dir: &Path,
    name: &str,
    dir: Option<&Path>,
) -> Result<Dataset, LoadDatasetError> {
    let provenance = Provenance::load(data_dir, name)?;
    let dir = dir.map_or_else(|| provenance.default_dir(data_dir), Path::to_owned);
    let mut files = Vec::with_capacity(provenance.files.len());
    for file in &provenance.files {
        let path = dir.join(&file.name);
        let bytes = fs::read(&path).map_err(|source| LoadDatasetError::Io {
            path: path.clone(),
            source,
        })?;
        let found = sha256_hex(&bytes);
        if found != file.sha256 {
            return Err(LoadDatasetError::HashMismatch { path, found });
        }
        files.push((file.name.clone(), bytes));
    }
    Ok(Dataset {
        provenance,
        dir,
        files,
    })
}

/// A dataset could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum LoadDatasetError {
    /// A file could not be read.
    #[error("cannot read {}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// Why.
        source: io::Error,
    },
    /// The `PROVENANCE.toml` is malformed.
    #[error("{} is malformed: {why}", path.display())]
    Provenance {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        why: String,
    },
    /// A file's bytes are not the ones its provenance records.
    #[error("{} has SHA-256 {found}, not the one PROVENANCE.toml records", path.display())]
    HashMismatch {
        /// The file.
        path: PathBuf,
        /// The hash found.
        found: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance(dir: &Path, class: &str, sha: &str) {
        fs::create_dir_all(dir.join("census")).unwrap();
        fs::write(
            dir.join("census/PROVENANCE.toml"),
            format!(
                "dataset = \"census\"\nclass = \"{class}\"\ncitation = \"A census\"\n\
                 url = \"https://example.org\"\nretrieved = \"2026-09-25\"\nrelease = \"1\"\n\
                 licence = \"facts\"\n[[files]]\nname = \"counts.txt\"\nsha256 = \"{sha}\"\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn a_committed_dataset_is_verified_against_its_provenance() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path();
        provenance(data, "committed", &sha256_hex(b"1 2 3\n"));
        fs::write(data.join("census/counts.txt"), "1 2 3\n").unwrap();
        let dataset = load_dataset(data, "census", None).unwrap();
        assert_eq!(
            dataset.files,
            [("counts.txt".to_owned(), b"1 2 3\n".to_vec())]
        );
        assert_eq!(dataset.provenance.recorded_hash().len(), 64);
        fs::write(data.join("census/counts.txt"), "1 2 4\n").unwrap();
        assert!(matches!(
            load_dataset(data, "census", None),
            Err(LoadDatasetError::HashMismatch { .. })
        ));
    }

    #[test]
    fn a_fetched_dataset_is_read_from_the_cache_or_the_given_directory() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path();
        provenance(data, "fetched", &sha256_hex(b"x"));
        assert!(matches!(
            load_dataset(data, "census", None),
            Err(LoadDatasetError::Io { .. })
        ));
        fs::create_dir_all(data.join("cache/census")).unwrap();
        fs::write(data.join("cache/census/counts.txt"), "x").unwrap();
        assert_eq!(
            load_dataset(data, "census", None).unwrap().dir,
            data.join("cache/census")
        );
        let elsewhere = root.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(elsewhere.join("counts.txt"), "x").unwrap();
        assert_eq!(
            load_dataset(data, "census", Some(&elsewhere)).unwrap().dir,
            elsewhere
        );
        assert!(matches!(
            Provenance::load(data, "other"),
            Err(LoadDatasetError::Io { .. })
        ));
    }
}
