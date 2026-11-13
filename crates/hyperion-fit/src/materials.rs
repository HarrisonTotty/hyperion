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
//!
//! # Phase tables
//!
//! Beside them, in `materials/phase/`, lie rendering plan R08.T5.c's and T5.d's reduced phase
//! tables of the non-spherical particles: `TAMUdust2020`'s irregular hexahedra (M. Saito, P. Yang,
//! J. Ding and X. Liu, J. Atmos. Sci. 78, 2089–2111, 2021) at each mineral's own index, and Yang
//! et al. 2013's roughened eight-column aggregate for water ice (J. Atmos. Sci. 70, 330–347), both
//! adapted CC BY 4.0 data (`NOTICE`). A manifest names those it reads in its `phase` list, and
//! the inputs hash covers each one's [`data hash`](PhaseTableFile::data_sha256), over the fields
//! P14.T49.e's `condensate_extinction` reads alone ([`PhaseTableFile`]): the scattering matrices,
//! the angles and the prose move no table.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::emit::sha256_hex;

/// Where R08's material files lie, relative to this crate's directory.
pub const MATERIALS_FROM_CRATE: &str =
    "../../apps/hyperion/src/renderer/src/view/atmosphere/materials";

/// Where R08's phase tables lie, relative to the material files' directory.
pub const PHASE_FROM_MATERIALS: &str = "phase";

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

    /// The temperature its constants are at, K, as the file writes it (a label to the kelvin where
    /// the file's reduction states the measured temperature, as R08.T5.g's and T5.h's liquids do;
    /// the state its index was computed at, to 0.01 K, for R08.T5.k's nine), or `None` where it
    /// states none.
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

/// The particle model a phase table was reduced from, its `model` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
pub enum PhaseModel {
    /// `TAMUdust2020`'s ensemble of 20 irregular hexahedra at sphericity 0.71 (`tamudust2020`):
    /// the minerals' table, whose particles below its smallest size the client draws as spheres
    /// of their own volume.
    #[serde(rename = "tamudust2020")]
    TamuDust2020,
    /// Yang et al. 2013's severely roughened aggregate of eight hexagonal columns, version 2
    /// (`yang2013`): water ice's table, whose crystals below its smallest the client draws as
    /// Grenfell and Warren 1999's spheres of their volume-to-area radius.
    #[serde(rename = "yang2013")]
    Yang2013,
}

impl PhaseModel {
    /// The name the files write.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TamuDust2020 => "tamudust2020",
            Self::Yang2013 => "yang2013",
        }
    }
}

/// One wavelength node of a phase table, as the JSON holds the fields the fit reads.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPhaseNode {
    wavelength_nm: f64,
    n: f64,
    #[serde(default)]
    n_table: Option<f64>,
    k: f64,
    k_table: f64,
    size_parameters: Vec<f64>,
    volumes: Vec<f64>,
    areas: Vec<f64>,
    extinction: Vec<f64>,
    scattering: Vec<f64>,
}

/// The fields of a phase table the fit reads, as the JSON holds them.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPhaseTableFile {
    key: String,
    phase: String,
    #[serde(default)]
    variant: Option<String>,
    model: PhaseModel,
    nodes: Vec<RawPhaseNode>,
}

/// One wavelength node of a phase table, checked: the material's index there and the index the
/// table was made at, and per size node the size parameter x = 2πD ÷ λ (D the maximum
/// dimension), the orientation-averaged volume Ṽ, projected area Ã and cross-sections `C̃_ext`
/// and `C̃_sca`, in powers of λ ÷ 2π at the node's wavelength (the files' `encoding`).
///
/// Every size has a value of each, positive and finite; the sizes ascend strictly in Ṽ ÷ Ã and in
/// x; and `C̃_sca` is at most `C̃_ext`, to the files' rounding of 10⁻⁸ (the client's
/// `parsePhaseFile`).
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseTableNode {
    wavelength_nm: f64,
    n: f64,
    n_table: Option<f64>,
    k: f64,
    k_table: f64,
    size_parameters: Vec<f64>,
    volumes: Vec<f64>,
    areas: Vec<f64>,
    extinction: Vec<f64>,
    scattering: Vec<f64>,
}

impl PhaseTableNode {
    /// The node's vacuum wavelength, nm.
    #[must_use]
    pub const fn wavelength_nm(&self) -> f64 {
        self.wavelength_nm
    }

    /// The material's n at the node, finite and positive.
    #[must_use]
    pub const fn n(&self) -> f64 {
        self.n
    }

    /// The n the table was made at: the material's, or the longwave kernel's nearer edge past its
    /// range, which the file writes as `nTable` only where it differs (decision-r08-t5d ruling 2).
    #[must_use]
    pub fn n_table(&self) -> f64 {
        self.n_table.unwrap_or(self.n)
    }

    /// The material's k at the node, at least zero.
    #[must_use]
    pub const fn k(&self) -> f64 {
        self.k
    }

    /// The k the table was made at, at least zero: the material's, or a kernel's floor beneath it
    /// (10⁻⁴ shortwave, 10⁻³ longwave), where the client scales the table's absorption by
    /// k ÷ `k_table`.
    #[must_use]
    pub const fn k_table(&self) -> f64 {
        self.k_table
    }

    /// x = 2πD ÷ λ of each size node, D the maximum dimension.
    #[must_use]
    pub fn size_parameters(&self) -> &[f64] {
        &self.size_parameters
    }

    /// Ṽ of each size node, in (λ ÷ 2π)³.
    #[must_use]
    pub fn volumes(&self) -> &[f64] {
        &self.volumes
    }

    /// Ã, the orientation-averaged projected area, of each size node, in (λ ÷ 2π)².
    #[must_use]
    pub fn areas(&self) -> &[f64] {
        &self.areas
    }

    /// `C̃_ext` of each size node, in (λ ÷ 2π)².
    #[must_use]
    pub fn extinction(&self) -> &[f64] {
        &self.extinction
    }

    /// `C̃_sca` of each size node, in (λ ÷ 2π)².
    #[must_use]
    pub fn scattering(&self) -> &[f64] {
        &self.scattering
    }
}

/// The fields of one of R08's phase tables that P14.T49.e's extinction reads, checked: the
/// material's key, phase and variant, the particle model, and the wavelength nodes in ascending
/// order, with their n, `n_table`, k, `k_table`, size parameters, V, A, `C_ext` and `C_sca`
/// ([`PhaseTableNode`]).
///
/// The file's other fields (its prose, its angles, its kernel, and the asymmetry and the
/// scattering matrix's elements) are the client's alone and are neither read nor hashed.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseTableFile {
    key: String,
    phase: String,
    variant: Option<String>,
    model: PhaseModel,
    nodes: Vec<PhaseTableNode>,
}

/// Whether `values` are all finite and positive.
#[must_use]
fn all_positive(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite() && *v > 0.0)
}

impl PhaseTableFile {
    /// Reads and checks the file `name` in `dir`.
    ///
    /// # Errors
    ///
    /// [`ReadMaterialError::Io`] if it cannot be read, [`ReadMaterialError::Parse`] if it is not a
    /// phase table, and [`ReadMaterialError::Shape`] if it has no node, its nodes' wavelengths are
    /// not finite, positive and ascending, or a node breaks [`PhaseTableNode`]'s rules (fewer than
    /// two sizes, a list of another length, a size, volume, area or cross-section that is not
    /// finite and positive, sizes that do not ascend, or a `C_sca` above its `C_ext`), has a k or
    /// `k_table` that is not finite and at least zero, or has an n or `n_table` that is not finite
    /// and positive.
    pub fn read(dir: &Path, name: &str) -> Result<Self, ReadMaterialError> {
        let path = dir.join(name);
        let text = fs::read_to_string(&path).map_err(|source| ReadMaterialError::Io {
            path: path.clone(),
            source,
        })?;
        let raw: RawPhaseTableFile =
            serde_json::from_str(&text).map_err(|e| ReadMaterialError::Parse {
                path: path.clone(),
                why: e.to_string(),
            })?;
        let shape = |why: String| ReadMaterialError::Shape {
            path: path.clone(),
            why,
        };
        if raw.nodes.is_empty() {
            return Err(shape("it has no wavelength node".to_owned()));
        }
        if !raw
            .nodes
            .windows(2)
            .all(|w| w[0].wavelength_nm < w[1].wavelength_nm)
            || !raw
                .nodes
                .iter()
                .all(|n| n.wavelength_nm.is_finite() && n.wavelength_nm > 0.0)
        {
            return Err(shape(
                "its nodes' wavelengths are not finite, positive and ascending".to_owned(),
            ));
        }
        let mut nodes = Vec::with_capacity(raw.nodes.len());
        for node in raw.nodes {
            let at = node.wavelength_nm;
            let sizes = node.size_parameters.len();
            let per_size = [
                &node.volumes,
                &node.areas,
                &node.extinction,
                &node.scattering,
            ];
            if sizes < 2 || per_size.iter().any(|v| v.len() != sizes) {
                return Err(shape(format!(
                    "its node at {at} nm needs two or more sizes and one value of each at each"
                )));
            }
            if !(all_positive(&node.size_parameters) && per_size.iter().all(|v| all_positive(v))) {
                return Err(shape(format!(
                    "its node at {at} nm has a size, volume, area or cross-section that is not \
                     finite and positive"
                )));
            }
            let ascending = (1..sizes).all(|s| {
                node.size_parameters[s] > node.size_parameters[s - 1]
                    && node.volumes[s] / node.areas[s] > node.volumes[s - 1] / node.areas[s - 1]
            });
            if !ascending {
                return Err(shape(format!(
                    "its node at {at} nm's sizes do not ascend strictly in x and in V ÷ A"
                )));
            }
            // Rounding to the files' figures may put C_sca a part in 10⁹ above C_ext for ω = 1
            // (the client's `parsePhaseFile`).
            if !(0..sizes).all(|s| node.scattering[s] <= node.extinction[s] * (1.0 + 1e-8)) {
                return Err(shape(format!(
                    "its node at {at} nm has a C_sca above its C_ext"
                )));
            }
            let at_least_zero = |v: f64| v.is_finite() && v >= 0.0;
            if !(at_least_zero(node.k) && at_least_zero(node.k_table)) {
                return Err(shape(format!(
                    "its node at {at} nm has a k or k_table that is not finite and at least zero"
                )));
            }
            if !all_positive(&[node.n, node.n_table.unwrap_or(node.n)]) {
                return Err(shape(format!(
                    "its node at {at} nm has an n or n_table that is not finite and positive"
                )));
            }
            nodes.push(PhaseTableNode {
                wavelength_nm: node.wavelength_nm,
                n: node.n,
                n_table: node.n_table,
                k: node.k,
                k_table: node.k_table,
                size_parameters: node.size_parameters,
                volumes: node.volumes,
                areas: node.areas,
                extinction: node.extinction,
                scattering: node.scattering,
            });
        }
        Ok(Self {
            key: raw.key,
            phase: raw.phase,
            variant: raw.variant,
            model: raw.model,
            nodes,
        })
    }

    /// The registry key of the material it was made for, as `mars_dust`.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// That material file's phase, `solid` or `liquid`.
    #[must_use]
    pub fn phase(&self) -> &str {
        &self.phase
    }

    /// That material file's variant, or `None` for the substance itself.
    #[must_use]
    pub fn variant(&self) -> Option<&str> {
        self.variant.as_deref()
    }

    /// The particle model it was reduced from.
    #[must_use]
    pub const fn model(&self) -> PhaseModel {
        self.model
    }

    /// Its wavelength nodes, ascending.
    #[must_use]
    pub fn nodes(&self) -> &[PhaseTableNode] {
        &self.nodes
    }

    /// Whether it was made for `file`: the same key, phase and variant (the client's
    /// `phaseFileOf`).
    #[must_use]
    pub fn serves(&self, file: &MaterialFile) -> bool {
        self.key == file.key() && self.phase == file.phase() && self.variant() == file.variant()
    }

    /// The SHA-256 of the fields it reads, 64 hex digits: `key`, `phase`, `variant` and `model`,
    /// then each node's `wavelengthNm`, `n`, `nTable`, `k`, `kTable`, `sizeParameters`, `volumes`,
    /// `areas`, `extinction` and `scattering`, each as `name=value` on a line of its own, a number
    /// as its shortest round-trip decimal, an absent value as `null` and a list's values joined by
    /// commas, each node's lines after a `node=<index>` line.
    #[must_use]
    pub fn data_sha256(&self) -> String {
        use std::fmt::Write as _;
        let list = |values: &[f64]| {
            values
                .iter()
                .map(|v| format!("{v:?}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut text = format!(
            "key={}\nphase={}\nvariant={}\nmodel={}\n",
            self.key,
            self.phase,
            self.variant.as_deref().unwrap_or("null"),
            self.model.name(),
        );
        for (i, node) in self.nodes.iter().enumerate() {
            // Writing to a String cannot fail.
            let _ = write!(
                text,
                "node={i}\nwavelengthNm={:?}\nn={:?}\nnTable={}\nk={:?}\nkTable={:?}\n\
                 sizeParameters={}\nvolumes={}\nareas={}\nextinction={}\nscattering={}\n",
                node.wavelength_nm,
                node.n,
                node.n_table
                    .map_or_else(|| "null".to_owned(), |n| format!("{n:?}")),
                node.k,
                node.k_table,
                list(&node.size_parameters),
                list(&node.volumes),
                list(&node.areas),
                list(&node.extinction),
                list(&node.scattering),
            );
        }
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

    /// `phase/mars-dust.json`'s JSON with `edit` applied, written to `dir` as `name`.
    fn write_edited_phase(dir: &Path, name: &str, edit: impl FnOnce(&mut Value)) {
        let text = fs::read_to_string(
            repository_dir()
                .join(PHASE_FROM_MATERIALS)
                .join("mars-dust.json"),
        )
        .unwrap();
        let mut value: Value = serde_json::from_str(&text).unwrap();
        edit(&mut value);
        fs::write(dir.join(name), serde_json::to_string(&value).unwrap()).unwrap();
    }

    /// A phase table reads with its nodes, serves its own material file alone, and its data hash
    /// covers the cross-sections, the sizes and the indices, and not its prose, its angles, its
    /// kernel or its matrix.
    #[test]
    fn a_phase_table_s_hash_covers_what_the_fit_reads_alone() {
        let dir = tempfile::tempdir().unwrap();
        write_edited_phase(dir.path(), "same.json", |_| {});
        write_edited_phase(dir.path(), "prose.json", |v| {
            v["particles"] = Value::from("other particles");
            v["licence"] = Value::from("another licence");
            v["phaseAnglesDeg"][1] = Value::from(0.09);
            v["nodes"][0]["a1"] = Value::from("AAAA");
            v["nodes"][0]["kernel"] = Value::from("longwave");
            v["nodes"][0]["scatteringAsymmetry"][0] = Value::from(1e-20);
        });
        let scaled = |field: &'static str, at: usize| {
            move |v: &mut Value| {
                let x = v["nodes"][0][field][at].as_f64().unwrap();
                v["nodes"][0][field][at] = Value::from(x * (1.0 + 1e-6));
            }
        };
        write_edited_phase(dir.path(), "extinction.json", scaled("extinction", 30));
        write_edited_phase(dir.path(), "scattering.json", scaled("scattering", 30));
        write_edited_phase(dir.path(), "volumes.json", scaled("volumes", 30));
        write_edited_phase(dir.path(), "areas.json", scaled("areas", 30));
        write_edited_phase(dir.path(), "sizes.json", scaled("sizeParameters", 30));
        write_edited_phase(dir.path(), "k-table.json", |v| {
            v["nodes"][0]["kTable"] = Value::from(1e-3);
        });
        write_edited_phase(dir.path(), "model.json", |v| {
            v["model"] = Value::from("yang2013");
        });
        write_edited_phase(dir.path(), "n.json", |v| {
            v["nodes"][0]["n"] = Value::from(1.6);
        });
        write_edited_phase(dir.path(), "n-table.json", |v| {
            v["nodes"][0]["nTable"] = Value::from(1.6);
        });
        let read = |name: &str| PhaseTableFile::read(dir.path(), name).unwrap();
        let dust = read("same.json");
        assert_eq!(dust.key(), "mars_dust");
        assert_eq!(dust.model(), PhaseModel::TamuDust2020);
        assert_eq!(dust.nodes().len(), 13);
        assert!((dust.nodes()[6].wavelength_nm() - 550.0).abs() < 1e-12);
        let hash = dust.data_sha256();
        assert_eq!(hash.len(), 64);
        assert_eq!(read("prose.json").data_sha256(), hash);
        for changed in [
            "extinction.json",
            "scattering.json",
            "volumes.json",
            "areas.json",
            "sizes.json",
            "k-table.json",
            "model.json",
            "n.json",
            "n-table.json",
        ] {
            assert_ne!(read(changed).data_sha256(), hash, "{changed}");
        }
        assert!((read("n-table.json").nodes()[0].n_table() - 1.6).abs() < 1e-15);
        assert!((dust.nodes()[0].n_table() - dust.nodes()[0].n()).abs() < 1e-15);
        let materials = repository_dir();
        let own = MaterialFile::read(&materials, "mars-dust.json").unwrap();
        let other = MaterialFile::read(&materials, "basalt.json").unwrap();
        assert!(dust.serves(&own));
        assert!(!dust.serves(&other));
    }

    /// A phase table is refused, one edited file per rule: with no node, wavelengths out of order, a
    /// node with fewer than two sizes, with a list of another length, with a non-positive area, with
    /// sizes that do not ascend, with a `C_sca` above its `C_ext`, with a negative k or `k_table`,
    /// or with a non-positive n or `n_table`.
    #[test]
    fn a_phase_table_out_of_shape_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_edited_phase(dir.path(), "empty.json", |v| {
            v["nodes"] = Value::Array(Vec::new());
        });
        write_edited_phase(dir.path(), "falling.json", |v| {
            let first = v["nodes"][0]["volumes"][0].clone();
            v["nodes"][0]["volumes"][1] = first;
        });
        write_edited_phase(dir.path(), "bright.json", |v| {
            let x = v["nodes"][0]["extinction"][5].as_f64().unwrap();
            v["nodes"][0]["scattering"][5] = Value::from(1.01 * x);
        });
        write_edited_phase(dir.path(), "red.json", |v| {
            v["nodes"].as_array_mut().unwrap().swap(0, 1);
        });
        write_edited_phase(dir.path(), "one-size.json", |v| {
            for field in [
                "sizeParameters",
                "volumes",
                "areas",
                "extinction",
                "scattering",
            ] {
                v["nodes"][0][field].as_array_mut().unwrap().truncate(1);
            }
        });
        write_edited_phase(dir.path(), "short.json", |v| {
            v["nodes"][0]["areas"].as_array_mut().unwrap().pop();
        });
        write_edited_phase(dir.path(), "flat.json", |v| {
            v["nodes"][0]["areas"][5] = Value::from(0.0);
        });
        write_edited_phase(dir.path(), "negative-k.json", |v| {
            v["nodes"][0]["k"] = Value::from(-1e-9);
        });
        write_edited_phase(dir.path(), "negative-k-table.json", |v| {
            v["nodes"][0]["kTable"] = Value::from(-1e-9);
        });
        write_edited_phase(dir.path(), "zero-n.json", |v| {
            v["nodes"][0]["n"] = Value::from(0.0);
        });
        write_edited_phase(dir.path(), "negative-n-table.json", |v| {
            v["nodes"][0]["nTable"] = Value::from(-1.5);
        });
        for (name, rule) in [
            ("empty.json", "no wavelength node"),
            ("red.json", "finite, positive and ascending"),
            ("one-size.json", "two or more sizes"),
            ("short.json", "two or more sizes"),
            ("flat.json", "size, volume, area or cross-section"),
            ("falling.json", "do not ascend strictly"),
            ("bright.json", "C_sca above its C_ext"),
            ("negative-k.json", "k or k_table"),
            ("negative-k-table.json", "k or k_table"),
            ("zero-n.json", "n or n_table"),
            ("negative-n-table.json", "n or n_table"),
        ] {
            match PhaseTableFile::read(dir.path(), name) {
                Err(ReadMaterialError::Shape { why, .. }) => {
                    assert!(why.contains(rule), "{name}: {why}");
                }
                other => panic!("{name}: {other:?}"),
            }
        }
        write_edited_phase(dir.path(), "model.json", |v| {
            v["model"] = Value::from("mie");
        });
        assert!(matches!(
            PhaseTableFile::read(dir.path(), "model.json"),
            Err(ReadMaterialError::Parse { .. })
        ));
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
