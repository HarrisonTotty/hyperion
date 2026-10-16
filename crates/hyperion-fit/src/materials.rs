//! Rendering plan R08's committed material files, read as a fit's input (galaxy plan 14,
//! P14.T49.h; the reading P14.T49.e's `condensate_extinction` shares).
//!
//! A material file is one JSON document in
//! `apps/hyperion/src/renderer/src/view/atmosphere/materials/`: a substance's optical constants n
//! and k on a wavelength grid, with its key, phase, variant, shape, temperature and provenance, and
//! prose (its name, paper, source, licence, reduction and stand-in text) that the client shows and
//! no fit reads. Those files are the one source of a material's n and k for the client's Mie and
//! the registry alike (decision-composition §1.1; decision-r11-palette-liquids §1.8), so a fit
//! reads them where they lie rather than copying them into a dataset.
//!
//! A manifest names the files it reads in its `materials` list. The inputs hash covers each
//! file's [`data hash`](MaterialFile::data_sha256), the SHA-256 of its data fields alone (key,
//! phase, variant, shape, temperature, provenance, wavelengths, n and k), so that an edit to a
//! file's prose stales no table, and an edit to its numbers stales every table made from it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::emit::sha256_hex;

/// Where R08's material files lie, relative to this crate's directory.
pub const MATERIALS_FROM_CRATE: &str =
    "../../apps/hyperion/src/renderer/src/view/atmosphere/materials";

/// The data fields of one of R08's material files, as the JSON holds them: what
/// [`MaterialFile::read`] parses and then checks.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMaterialFile {
    key: String,
    phase: String,
    #[serde(default)]
    variant: Option<String>,
    shape: String,
    #[serde(default)]
    temperature_k: Option<f64>,
    provenance: String,
    wavelengths_nm: Vec<f64>,
    n: Vec<f64>,
    k: Vec<f64>,
}

/// The data fields of one of R08's material files, checked: a grid of two or more finite,
/// positive, increasing wavelengths, with one finite positive n and one finite k of at least zero
/// at each.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialFile {
    key: String,
    phase: String,
    variant: Option<String>,
    shape: String,
    temperature_k: Option<f64>,
    provenance: String,
    wavelengths_nm: Vec<f64>,
    n: Vec<f64>,
    k: Vec<f64>,
}

impl MaterialFile {
    /// Reads and checks the file `name` in `dir`.
    ///
    /// # Errors
    ///
    /// [`ReadMaterialError::Io`] if it cannot be read, [`ReadMaterialError::Parse`] if it is not a
    /// material file, and [`ReadMaterialError::Shape`] if its grid is not increasing and finite or
    /// its n and k are not one value per wavelength, finite, n positive and k at least zero.
    pub fn read(dir: &Path, name: &str) -> Result<Self, ReadMaterialError> {
        let path = dir.join(name);
        let text = fs::read_to_string(&path).map_err(|source| ReadMaterialError::Io {
            path: path.clone(),
            source,
        })?;
        let raw: RawMaterialFile =
            serde_json::from_str(&text).map_err(|e| ReadMaterialError::Parse {
                path: path.clone(),
                why: e.to_string(),
            })?;
        let shape = |why: &str| ReadMaterialError::Shape {
            path: path.clone(),
            why: why.to_owned(),
        };
        let grid = &raw.wavelengths_nm;
        if grid.len() < 2 || !grid.iter().all(|w| w.is_finite() && *w > 0.0) {
            return Err(shape(
                "its wavelengths are not two or more finite positive values",
            ));
        }
        if !grid.windows(2).all(|w| w[0] < w[1]) {
            return Err(shape("its wavelengths do not increase"));
        }
        if raw.n.len() != grid.len() || raw.k.len() != grid.len() {
            return Err(shape("its n and k are not one value per wavelength"));
        }
        if !raw.n.iter().all(|n| n.is_finite() && *n > 0.0) {
            return Err(shape("an n is not finite and positive"));
        }
        if !raw.k.iter().all(|k| k.is_finite() && *k >= 0.0) {
            return Err(shape("a k is not finite and at least zero"));
        }
        Ok(Self {
            key: raw.key,
            phase: raw.phase,
            variant: raw.variant,
            shape: raw.shape,
            temperature_k: raw.temperature_k,
            provenance: raw.provenance,
            wavelengths_nm: raw.wavelengths_nm,
            n: raw.n,
            k: raw.k,
        })
    }

    /// The substance's registry key, as `H2O`.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// `solid` or `liquid`.
    #[must_use]
    pub fn phase(&self) -> &str {
        &self.phase
    }

    /// The composition's variant, as `75wt%` or `S35`, or `None` for the substance itself.
    #[must_use]
    pub fn variant(&self) -> Option<&str> {
        self.variant.as_deref()
    }

    /// The particle shape the client's Mie takes for it.
    #[must_use]
    pub fn shape(&self) -> &str {
        &self.shape
    }

    /// The temperature its constants are at, K, as the file writes it (a label to the kelvin:
    /// the file's reduction states the measured temperature), or `None` where it states none.
    #[must_use]
    pub fn temperature_k(&self) -> Option<f64> {
        self.temperature_k
    }

    /// `measured`, `derived` or `standIn`.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// The wavelengths, nm: two or more, finite, positive and increasing.
    #[must_use]
    pub fn wavelengths_nm(&self) -> &[f64] {
        &self.wavelengths_nm
    }

    /// The real index at each wavelength, finite and positive.
    #[must_use]
    pub fn n(&self) -> &[f64] {
        &self.n
    }

    /// The imaginary index at each wavelength, finite and at least zero.
    #[must_use]
    pub fn k(&self) -> &[f64] {
        &self.k
    }

    /// The SHA-256 of the data fields, 64 hex digits: each field as `name=value` on a line of its
    /// own, in the struct's order, a number as its shortest round-trip decimal and an absent value
    /// as `null`, and each list's values joined by commas.
    #[must_use]
    pub fn data_sha256(&self) -> String {
        let list = |values: &[f64]| {
            values
                .iter()
                .map(|v| format!("{v:?}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let temperature = self
            .temperature_k
            .map_or_else(|| "null".to_owned(), |t| format!("{t:?}"));
        let text = format!(
            "key={}\nphase={}\nvariant={}\nshape={}\ntemperatureK={temperature}\nprovenance={}\n\
             wavelengthsNm={}\nn={}\nk={}\n",
            self.key,
            self.phase,
            self.variant.as_deref().unwrap_or("null"),
            self.shape,
            self.provenance,
            list(&self.wavelengths_nm),
            list(&self.n),
            list(&self.k),
        );
        sha256_hex(text.as_bytes())
    }
}

/// A material file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ReadMaterialError {
    /// The file could not be read.
    #[error("cannot read the material file {}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// Why.
        source: io::Error,
    },
    /// The file is not a material file.
    #[error("the material file {} is malformed: {why}", path.display())]
    Parse {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        why: String,
    },
    /// The file's grid or constants are out of shape.
    #[error("the material file {}'s constants are out of shape: {why}", path.display())]
    Shape {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        why: String,
    },
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    /// The repository's material directory.
    fn repository_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(MATERIALS_FROM_CRATE)
    }

    /// `water.json`'s JSON with `edit` applied, written to `dir` as `name`.
    fn write_edited(dir: &Path, name: &str, edit: impl FnOnce(&mut Value)) {
        let text = fs::read_to_string(repository_dir().join("water.json")).unwrap();
        let mut value: Value = serde_json::from_str(&text).unwrap();
        edit(&mut value);
        fs::write(dir.join(name), serde_json::to_string(&value).unwrap()).unwrap();
    }

    /// A file reads with its data, and its data hash covers its numbers, its provenance and its
    /// temperature, and not its prose (its name, paper, source, licence and reduction).
    #[test]
    fn a_material_file_s_hash_covers_its_data_alone() {
        let dir = tempfile::tempdir().unwrap();
        write_edited(dir.path(), "same.json", |_| {});
        write_edited(dir.path(), "prose.json", |v| {
            v["name"] = Value::from("another name");
            v["reduction"] = Value::from("another reduction");
            v["licence"] = Value::from("another licence");
        });
        write_edited(dir.path(), "number.json", |v| {
            let n = v["n"][10].as_f64().unwrap();
            v["n"][10] = Value::from(n + 1e-6);
        });
        write_edited(dir.path(), "provenance.json", |v| {
            v["provenance"] = Value::from("derived");
        });
        write_edited(dir.path(), "temperature.json", |v| {
            v["temperatureK"] = Value::from(300);
        });
        let read = |name: &str| MaterialFile::read(dir.path(), name).unwrap();
        let water = read("same.json");
        assert_eq!(water.key(), "H2O");
        assert_eq!(water.phase(), "liquid");
        assert_eq!(water.variant(), None);
        assert_eq!(water.wavelengths_nm().len(), water.n().len());
        let hash = water.data_sha256();
        assert_eq!(hash.len(), 64);
        assert_eq!(read("prose.json").data_sha256(), hash);
        for changed in ["number.json", "provenance.json", "temperature.json"] {
            assert_ne!(read(changed).data_sha256(), hash, "{changed}");
        }
    }

    /// A file whose grid falls, or whose k is negative, is refused.
    #[test]
    fn a_material_file_out_of_shape_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, body: &str| fs::write(dir.path().join(name), body).unwrap();
        write(
            "falling.json",
            r#"{"key":"X","phase":"liquid","shape":"sphere","provenance":"measured",
               "wavelengthsNm":[400,380],"n":[1.3,1.3],"k":[0,0]}"#,
        );
        write(
            "negative.json",
            r#"{"key":"X","phase":"liquid","shape":"sphere","provenance":"measured",
               "wavelengthsNm":[380,400],"n":[1.3,1.3],"k":[0,-1e-9]}"#,
        );
        for name in ["falling.json", "negative.json"] {
            assert!(
                matches!(
                    MaterialFile::read(dir.path(), name),
                    Err(ReadMaterialError::Shape { .. })
                ),
                "{name}"
            );
        }
        assert!(matches!(
            MaterialFile::read(dir.path(), "absent.json"),
            Err(ReadMaterialError::Io { .. })
        ));
    }
}
