//! `run star_colour`: the colour of every star the sky draws, from model spectra (rendering plan
//! R06, R06.T3.a, Design note 6), which the sim's `sky::colour` interpolates.
//!
//! # What a row holds
//!
//! For one model spectrum, integrated over 1 nm bins against the CIE 1931 2° colour-matching
//! functions, the CIE 1924 photopic V(λ), the CIE 1951 scotopic V′(λ) and Bessell and Murphy's
//! (2012) photonic V band ([`photometry`]): the linear Rec. 709 red and green of the spectrum at
//! unit luminance (D65 white, desaturated towards white out of gamut), the photopic illuminance of
//! a star of V = 0 with that spectrum over Allen's 2.54 µlx, and the scotopic-to-photopic ratio ρ.
//!
//! # The grids
//!
//! Two rectangular grids in `T_eff` and log g, which the sim reads bilinearly in log `T_eff` and log g
//! and clamps at their edges:
//!
//! - **Stars that are not white dwarfs**, log g 0–6 in steps of 0.5: PHOENIX-ACES (Husser et al.
//!   2013) at 2,300–3,400 K; ATLAS9 (Castelli and Kurucz 2003) at 3,500–27,000 K; TLUSTY
//!   OSTAR2002 (Lanz and Hubeny 2003) at 27,500–55,000 K; TMAP H+He (He mass fraction 0.3) at
//!   60,000–100,000 K; a blackbody above 100,000 K.
//! - **White dwarfs**, log g 6.5–9.5 in steps of 0.5: Koester's DA spectra (Koester 2010) at
//!   5,000–80,000 K; TMAP pure hydrogen at 90,000 and 100,000 K; a blackbody above 100,000 K.
//!
//! A grid node a model set does not hold is filled from the same set at the same temperature: by
//! the nearest gravity it holds beyond its range (ATLAS9 and TLUSTY hold no low gravities for hot
//! stars, ATLAS9 none above 5, TLUSTY none outside 3–4.75, TMAP none below 5 or above 9), as the
//! limb-darkening table is clamped (Design note 16).
//!
//! # The spectra are fetched
//!
//! None of the grids states a licence, so each is a fetched dataset (plan 15, Design note 12):
//! `data/<dataset>/PROVENANCE.toml` and `urls.txt` are committed, and the files are read from the
//! git-ignored `data/cache/<dataset>/`; so are Bessell and Murphy's V and Pickles' library. Only the
//! CIE tables (CC BY-SA 4.0) are committed. The smoke manifest runs the same grids with a blackbody
//! in place of every model spectrum and the photopic V(λ) in place of the V band, from the
//! committed CIE tables alone.

pub mod columns;
pub mod photometry;
pub mod pickles;
pub mod spectrum;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::num::NonZeroUsize;
use std::ops::Range;
use std::path::{Path, PathBuf};

use hyperion_sim::galaxy::gas::ccm::extinction_ratio;
use hyperion_sim::sky::colour::{SUN_LOG_G, SUN_TEFF_K, lift_into_gamut};
use hyperion_sim::units::Micrometres;

use crate::data::{Dataset, LoadDatasetError, Provenance, load_dataset};
use crate::emit::{RustTable, TableItem, sha256_hex};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::map_reduce_chunks;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

pub use columns::{
    BAKE_WAVELENGTH_COUNT, BAND_COUNT, CAMERA_BAND, Extras, PART_COUNT, REDDENING_A_V_NODES,
    REDDENING_NODE_COUNT, Reddening, SCOTOPIC_BAND, Sensor, V_BAND, bake_wavelengths_nm,
};
pub use photometry::{ColourRow, Observer, VBandSource};
pub use spectrum::{ReadSpectrumError, Spectrum};

/// The task's revision, written into the table's header; bumped when anything below moves the
/// table.
pub const VERSION: u32 = 0;

/// The task's name.
const NAME: &str = "star_colour";

/// The gravities of the grid of stars that are not white dwarfs: log₁₀ g (cgs) 0–6 by 0.5.
pub const NORMAL_LOG_G: [f64; 13] = [
    0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0,
];

/// The gravities of the white dwarfs' grid: log₁₀ g (cgs) 6.5–9.5 by 0.5.
pub const WHITE_DWARF_LOG_G: [f64; 7] = [6.5, 7.0, 7.5, 8.0, 8.5, 9.0, 9.5];

/// The blackbody nodes above the hottest model, K, shared by both grids.
pub const BLACKBODY_TEFF: [u32; 5] = [120_000, 150_000, 200_000, 300_000, 500_000];

/// Koester's DA temperatures, K, as SVO serves the grid.
const KOESTER_TEFF: [u32; 82] = [
    5_000, 5_250, 5_500, 5_750, 6_000, 6_250, 6_500, 6_750, 7_000, 7_250, 7_500, 7_750, 8_000,
    8_250, 8_500, 8_750, 9_000, 9_250, 9_500, 9_750, 10_000, 10_250, 10_500, 10_750, 11_000,
    11_250, 11_500, 11_750, 12_000, 12_250, 12_500, 12_750, 13_000, 13_250, 13_500, 13_750, 14_000,
    14_250, 14_500, 14_750, 15_000, 15_250, 15_500, 15_750, 16_000, 16_250, 16_500, 16_750, 17_000,
    17_250, 17_500, 17_750, 18_000, 18_250, 18_500, 18_750, 19_000, 19_250, 19_500, 19_750, 20_000,
    21_000, 22_000, 23_000, 24_000, 25_000, 26_000, 27_000, 28_000, 29_000, 30_000, 32_000, 34_000,
    35_000, 36_000, 38_000, 40_000, 45_000, 50_000, 60_000, 70_000, 80_000,
];

/// Where a grid node's spectra come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    /// PHOENIX-ACES-AGSS-COND-2011 (Husser et al. 2013).
    Phoenix,
    /// ATLAS9 ODFNEW (Castelli and Kurucz 2003).
    Atlas9,
    /// TLUSTY OSTAR2002 (Lanz and Hubeny 2003).
    Tlusty,
    /// TMAP H+He, He mass fraction 0.3.
    TmapHelium,
    /// TMAP pure hydrogen.
    TmapHydrogen,
    /// Koester's DA white dwarfs.
    Koester,
    /// A Planck spectrum.
    Blackbody,
}

impl Source {
    /// The dataset the source's spectra are in.
    #[must_use]
    pub const fn dataset(self) -> Option<&'static str> {
        match self {
            Self::Phoenix => Some("phoenix_husser2013"),
            Self::Atlas9 => Some("atlas9_ck04"),
            Self::Tlusty => Some("tlusty_ostar2002"),
            Self::TmapHelium | Self::TmapHydrogen => Some("tmap"),
            Self::Koester => Some("wd_koester_da"),
            Self::Blackbody => None,
        }
    }

    /// A short name for the table's notes.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Phoenix => "PHOENIX-ACES",
            Self::Atlas9 => "ATLAS9",
            Self::Tlusty => "TLUSTY OSTAR2002",
            Self::TmapHelium => "TMAP H+He (Y = 0.3)",
            Self::TmapHydrogen => "TMAP H",
            Self::Koester => "Koester DA",
            Self::Blackbody => "blackbody",
        }
    }

    /// The temperature (K) and gravity (log g in hundredths) of one of the source's files, or
    /// `None` for a file of its dataset that is not one of its spectra.
    #[must_use]
    pub fn model_of_file(self, name: &str) -> Option<(u32, i32)> {
        let svo = |rest: &str| -> Option<(u32, i32)> {
            let rest = rest.strip_prefix('t')?.strip_suffix(".txt")?;
            let (t, g) = rest.split_once("_g")?;
            Some((t.parse().ok()?, centi(g)?))
        };
        match self {
            Self::Phoenix => {
                let rest = name
                    .strip_prefix("lte")?
                    .strip_suffix("-0.0.PHOENIX-ACES-AGSS-COND-2011-HiRes.fits")?;
                let (t, g) = rest.split_once('-')?;
                Some((t.parse().ok()?, centi(g)?))
            }
            Self::Atlas9 | Self::Tlusty | Self::Koester => svo(name),
            Self::TmapHelium => svo(name.strip_prefix("he30_")?),
            Self::TmapHydrogen => svo(name.strip_prefix("h_")?),
            Self::Blackbody => None,
        }
    }
}

/// `"4.50"` as 450.
#[must_use]
fn centi(text: &str) -> Option<i32> {
    let (whole, frac) = text.split_once('.')?;
    let frac = format!("{frac:0<2}");
    Some(whole.parse::<i32>().ok()? * 100 + frac.get(..2)?.parse::<i32>().ok()?)
}

/// `0.5` as 50, for a gravity node.
#[must_use]
fn centi_of(log_g: f64) -> i32 {
    let mut c = 0;
    while f64::from(c) < log_g * 100.0 - 0.5 {
        c += 50;
    }
    c
}

/// The temperature nodes of the grid of stars that are not white dwarfs, K, with their source.
#[must_use]
pub fn normal_teff_nodes() -> Vec<(u32, Source)> {
    let mut nodes: Vec<(u32, Source)> = (2_300..=3_400)
        .step_by(100)
        .map(|t| (t, Source::Phoenix))
        .collect();
    nodes.extend((3_500..=13_000).step_by(250).map(|t| (t, Source::Atlas9)));
    nodes.extend(
        (14_000..=27_000)
            .step_by(1_000)
            .map(|t| (t, Source::Atlas9)),
    );
    nodes.extend(
        (27_500..=55_000)
            .step_by(2_500)
            .map(|t| (t, Source::Tlusty)),
    );
    nodes.extend(
        (60_000..=100_000)
            .step_by(10_000)
            .map(|t| (t, Source::TmapHelium)),
    );
    nodes.extend(BLACKBODY_TEFF.map(|t| (t, Source::Blackbody)));
    nodes
}

/// The temperature nodes of the white dwarfs' grid, K, with their source.
#[must_use]
pub fn white_dwarf_teff_nodes() -> Vec<(u32, Source)> {
    let mut nodes: Vec<(u32, Source)> = KOESTER_TEFF.map(|t| (t, Source::Koester)).to_vec();
    nodes.extend([90_000, 100_000].map(|t| (t, Source::TmapHydrogen)));
    nodes.extend(BLACKBODY_TEFF.map(|t| (t, Source::Blackbody)));
    nodes
}

/// Whether the model spectra or a blackbody at each node's temperature are integrated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Spectra {
    /// The model atmospheres: the table.
    Models,
    /// A blackbody at every node: the smoke run, and the comparison the notes report.
    Blackbody,
}

/// Everything one spectrum gives the table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableRow {
    /// Chroma, photopic flux and ρ (R06.T3.a).
    pub colour: ColourRow,
    /// The camera, reddening and bake columns (R06.T3.c).
    pub extras: Extras,
    /// The light's reddening through dust, the five `star_colour_reddening` tables' columns
    /// (R06.T9.e).
    pub reddening: Reddening,
}

/// The table's row for `spectrum`.
#[must_use]
pub fn table_row(observer: &Observer, sensor: &Sensor, spectrum: &Spectrum) -> TableRow {
    let bins = spectrum.bin_means();
    TableRow {
        colour: observer.row(&bins),
        extras: columns::extras(observer, sensor, spectrum, &bins),
        reddening: columns::reddening(observer, sensor, &bins),
    }
}

/// One grid: its nodes and a row per node, temperature-major.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// The temperature nodes, K, rising, with their sources.
    pub teff: Vec<(u32, Source)>,
    /// The gravity nodes, log₁₀ g (cgs), rising.
    pub log_g: Vec<f64>,
    /// The rows: `rows[i * log_g.len() + j]` is temperature `i`, gravity `j`.
    pub rows: Vec<TableRow>,
    /// The nodes filled from another gravity of the same temperature, as (`T_eff`, log g node, log
    /// g of the model used).
    pub clamped: Vec<(u32, f64, f64)>,
}

impl Grid {
    /// The row at temperature node `i` and gravity node `j`.
    #[must_use]
    pub fn row(&self, i: usize, j: usize) -> TableRow {
        self.rows[i * self.log_g.len() + j]
    }
}

/// The fitted table.
#[derive(Debug, Clone, PartialEq)]
pub struct StarColourTable {
    /// Stars that are not white dwarfs.
    pub normal: Grid,
    /// White dwarfs.
    pub white_dwarf: Grid,
    /// The Rec. 709 luminance of unit red, green and blue: the middle row of the matrix from
    /// linear Rec. 709 to XYZ.
    pub luminance: [f64; 3],
    /// Which spectra were integrated.
    pub spectra: Spectra,
    /// η☉: the default sensor's electrons per V-band photon at 5,772 K and log g 4.438, from the
    /// grid as the sim interpolates it (geometrically, so that the Sun's interpolated term is 0).
    pub camera_eta_sun: f64,
}

/// The camera band term's range on the wire, mag: an `i8` in 1/32 mag steps, saturating
/// (decision-camera-eta, Design note 17). The table saturates at it too.
pub const CAMERA_TERM_RANGE: (f64, f64) = (-4.0, 3.968_75);

/// The Sun's effective temperature, K (IAU 2015 Resolution B3), and log g from the nominal GM☉ and
/// R☉: the point whose camera band term is zero, the sim's `sky::colour::{SUN_TEFF_K, SUN_LOG_G}`,
/// at which the band takes its reddening.
pub const SUN: (f64, f64) = (SUN_TEFF_K, SUN_LOG_G);

/// η☉ from `grid`: 10 to the bilinear mean of log₁₀ η over the four nodes about [`SUN`], with the
/// weights the sim's `sky::colour` takes from the table's rounded nodes, so that the interpolated
/// camera band term at the Sun is zero to rounding.
#[must_use]
pub fn camera_eta_sun(grid: &Grid) -> f64 {
    let log_t: Vec<f64> = grid
        .teff
        .iter()
        .map(|&(t, _)| rounded(hyperion_sim::math::log10(f64::from(t))))
        .collect();
    let (ti, tf) = pickles::bracket(&log_t, hyperion_sim::math::log10(SUN.0));
    let (gi, gf) = pickles::bracket(&grid.log_g, SUN.1);
    let log_eta = |i: usize, j: usize| hyperion_sim::math::log10(grid.row(i, j).extras.camera_eta);
    let low = log_eta(ti, gi) * (1.0 - gf) + log_eta(ti, gi + 1) * gf;
    let high = log_eta(ti + 1, gi) * (1.0 - gf) + log_eta(ti + 1, gi + 1) * gf;
    hyperion_sim::math::exp10(low * (1.0 - tf) + high * tf)
}

/// The files of one model set and how to read them: its directory, each file of a wanted
/// temperature with its hash and model, and PHOENIX's shared wavelengths.
struct ModelFiles {
    dir: PathBuf,
    files: Vec<(String, String, (u32, i32))>,
    /// PHOENIX's wavelength file's kept range and its wavelengths, nm.
    phoenix_wave: Option<(usize, usize, Vec<f64>)>,
}

impl ModelFiles {
    /// The files of `source`'s dataset whose models are at the temperatures `wanted`, with the
    /// PHOENIX wavelengths read and checked if the source is PHOENIX's.
    ///
    /// # Errors
    ///
    /// [`RunTaskError`] if the provenance cannot be read, or PHOENIX's wavelength file cannot be
    /// read, fails its hash or is malformed.
    fn open(
        manifest: &Manifest,
        source: Source,
        wanted: &[u32],
    ) -> Result<Option<Self>, RunTaskError> {
        let Some(dataset) = source.dataset() else {
            return Ok(None);
        };
        let provenance = Provenance::load(manifest.data_root(), dataset)?;
        let dir = manifest.data_dir(dataset).map_or_else(
            || provenance.default_dir(manifest.data_root()),
            PathBuf::from,
        );
        let files = provenance
            .files
            .iter()
            .filter_map(|f| {
                let model = source.model_of_file(&f.name)?;
                wanted
                    .contains(&model.0)
                    .then(|| (f.name.clone(), f.sha256.clone(), model))
            })
            .collect();
        let mut out = Self {
            dir,
            files,
            phoenix_wave: None,
        };
        // PHOENIX's spectra share one wavelength file, in Å in vacuum; only 300–1,200 nm is kept.
        if source == Source::Phoenix {
            let wave = provenance
                .files
                .iter()
                .find(|f| f.name == PHOENIX_WAVE)
                .ok_or_else(|| {
                    input(ReadSpectrumError::Malformed(
                        "the PHOENIX wavelength file is not listed",
                    ))
                })?;
            let angstrom =
                spectrum::read_fits_vector(&out.read(&wave.name, &wave.sha256)?).map_err(input)?;
            let lo = angstrom.partition_point(|&w| w < 3_000.0);
            let hi = angstrom.partition_point(|&w| w <= 12_000.0);
            out.phoenix_wave = Some((
                lo,
                hi,
                angstrom[lo..hi]
                    .iter()
                    .map(|w| w / 10.0)
                    .collect::<Vec<_>>(),
            ));
        }
        Ok(Some(out))
    }

    /// The bytes of file `name`, checked against its hash `sha`.
    ///
    /// # Errors
    ///
    /// [`RunTaskError::Dataset`] if it cannot be read or fails its hash.
    fn read(&self, name: &str, sha: &str) -> Result<Vec<u8>, RunTaskError> {
        let path = self.dir.join(name);
        let bytes = fs::read(&path).map_err(|source| LoadDatasetError::Io {
            path: path.clone(),
            source,
        })?;
        let found = sha256_hex(&bytes);
        if found != sha {
            return Err(LoadDatasetError::HashMismatch { path, found }.into());
        }
        Ok(bytes)
    }

    /// The spectrum of file `name`, checked against its hash `sha`.
    ///
    /// # Errors
    ///
    /// [`RunTaskError`] if the file cannot be read, fails its hash or is malformed.
    fn spectrum(&self, name: &str, sha: &str) -> Result<Spectrum, RunTaskError> {
        let bytes = self.read(name, sha)?;
        if let Some((lo, hi, nm)) = &self.phoenix_wave {
            let flux = spectrum::read_fits_vector(&bytes).map_err(input)?;
            let flux = flux
                .get(*lo..*hi)
                .ok_or_else(|| {
                    input(ReadSpectrumError::Malformed(
                        "a PHOENIX spectrum is shorter than its wavelengths",
                    ))
                })?
                .to_vec();
            Spectrum::new(nm.clone(), flux).map_err(input)
        } else {
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| input(ReadSpectrumError::Malformed("a spectrum file is not text")))?;
            spectrum::parse_svo_ascii(text).map_err(input)
        }
    }
}

/// A spectrum that cannot be read, as the task's input error.
fn input(e: ReadSpectrumError) -> RunTaskError {
    RunTaskError::Input {
        task: NAME,
        source: Box::new(e),
    }
}

/// The rows of every model of `source` at the temperatures `wanted`, by (`T_eff`, log g in
/// hundredths), each file read and checked against its provenance.
///
/// # Errors
///
/// [`RunTaskError`] if the provenance cannot be read, a file cannot be read or fails its hash, or
/// a spectrum is malformed.
fn read_models(
    manifest: &Manifest,
    observer: &Observer,
    sensor: &Sensor,
    source: Source,
    wanted: &[u32],
    threads: NonZeroUsize,
) -> Result<BTreeMap<(u32, i32), TableRow>, RunTaskError> {
    let Some(models) = ModelFiles::open(manifest, source, wanted)? else {
        return Ok(BTreeMap::new());
    };
    let count = u64::try_from(models.files.len()).expect("a file count fits in u64");
    let mut results = Vec::with_capacity(models.files.len());
    map_reduce_chunks(
        count,
        1,
        threads,
        |range| -> Result<Vec<ModelRow>, RunTaskError> {
            let mut out = Vec::new();
            for i in range {
                let (name, sha, model) =
                    &models.files[usize::try_from(i).expect("i is below files.len(), a usize")];
                let spectrum = models.spectrum(name, sha)?;
                out.push((*model, table_row(observer, sensor, &spectrum)));
            }
            Ok(out)
        },
        |part| results.push(part),
    )?;
    let mut rows = BTreeMap::new();
    for part in results {
        rows.extend(part?);
    }
    Ok(rows)
}

/// The spectrum of `source`'s model at `teff` K and log g `log_g_centi` hundredths, read and
/// checked as the fit reads it: for the tests that integrate one grid node's own spectrum.
///
/// # Errors
///
/// [`RunTaskError`] if the source holds no such model or a blackbody, or as [`read_models`].
pub fn read_model_spectrum(
    manifest: &Manifest,
    source: Source,
    teff: u32,
    log_g_centi: i32,
) -> Result<Spectrum, RunTaskError> {
    let missing = || RunTaskError::Input {
        task: NAME,
        source: format!(
            "{} holds no model at {teff} K and log g {log_g_centi}/100",
            source.label()
        )
        .into(),
    };
    let models = ModelFiles::open(manifest, source, &[teff])?.ok_or_else(missing)?;
    let (name, sha, _) = models
        .files
        .iter()
        .find(|(_, _, model)| *model == (teff, log_g_centi))
        .ok_or_else(missing)?;
    models.spectrum(name, sha)
}

/// A model's (`T_eff` in K, log g in hundredths) and its row.
type ModelRow = ((u32, i32), TableRow);

/// PHOENIX's wavelength file.
const PHOENIX_WAVE: &str = "WAVE_PHOENIX-ACES-AGSS-COND-2011.fits";

/// Builds one grid from the rows read, or from blackbodies.
///
/// # Errors
///
/// [`RunTaskError::Input`] if a temperature node's source holds no model at it.
fn build_grid(
    teff: Vec<(u32, Source)>,
    log_g: &[f64],
    models: &BTreeMap<Source, BTreeMap<(u32, i32), TableRow>>,
    observer: &Observer,
    sensor: &Sensor,
    spectra: Spectra,
) -> Result<Grid, RunTaskError> {
    let mut rows = Vec::with_capacity(teff.len() * log_g.len());
    let mut clamped = Vec::new();
    for &(t, source) in &teff {
        if spectra == Spectra::Blackbody || source == Source::Blackbody {
            let row = table_row(observer, sensor, &Spectrum::blackbody(f64::from(t)));
            rows.extend(std::iter::repeat_n(row, log_g.len()));
            continue;
        }
        let held: Vec<(i32, TableRow)> = models
            .get(&source)
            .into_iter()
            .flat_map(|m| m.range((t, i32::MIN)..=(t, i32::MAX)))
            .map(|(&(_, g), &row)| (g, row))
            .collect();
        let (Some(&(least, least_row)), Some(&(most, most_row))) = (held.first(), held.last())
        else {
            return Err(RunTaskError::Input {
                task: NAME,
                source: format!("{} holds no model at {t} K", source.label()).into(),
            });
        };
        for &g in log_g {
            let c = centi_of(g);
            let row = if let Some(&(_, row)) = held.iter().find(|(h, _)| *h == c) {
                row
            } else if c < least {
                clamped.push((t, g, f64::from(least) / 100.0));
                least_row
            } else if c > most {
                clamped.push((t, g, f64::from(most) / 100.0));
                most_row
            } else {
                return Err(RunTaskError::Input {
                    task: NAME,
                    source: format!("{} has a gap at {t} K, log g {g}", source.label()).into(),
                });
            };
            rows.push(row);
        }
    }
    Ok(Grid {
        teff,
        log_g: log_g.to_vec(),
        rows,
        clamped,
    })
}

/// Fits the table: reads every model the grids need and integrates it.
///
/// # Errors
///
/// [`RunTaskError`] as [`read_models`] and [`build_grid`].
pub fn fit(
    manifest: &Manifest,
    observer: &Observer,
    sensor: &Sensor,
    spectra: Spectra,
    threads: NonZeroUsize,
) -> Result<StarColourTable, RunTaskError> {
    let normal = normal_teff_nodes();
    let white_dwarf = white_dwarf_teff_nodes();
    let mut models = BTreeMap::new();
    if spectra == Spectra::Models {
        for source in [
            Source::Phoenix,
            Source::Atlas9,
            Source::Tlusty,
            Source::TmapHelium,
            Source::TmapHydrogen,
            Source::Koester,
        ] {
            let wanted: Vec<u32> = normal
                .iter()
                .chain(&white_dwarf)
                .filter(|(_, s)| *s == source)
                .map(|(t, _)| *t)
                .collect();
            models.insert(
                source,
                read_models(manifest, observer, sensor, source, &wanted, threads)?,
            );
        }
    }
    let normal = build_grid(normal, &NORMAL_LOG_G, &models, observer, sensor, spectra)?;
    let white_dwarf = build_grid(
        white_dwarf,
        &WHITE_DWARF_LOG_G,
        &models,
        observer,
        sensor,
        spectra,
    )?;
    Ok(StarColourTable {
        camera_eta_sun: camera_eta_sun(&normal),
        normal,
        white_dwarf,
        luminance: observer.luminance(),
        spectra,
    })
}

/// `value` to seven significant digits: the table's precision, finer than any model's.
#[must_use]
pub(crate) fn rounded(value: f64) -> f64 {
    format!("{value:.6e}")
        .parse()
        .expect("a formatted float parses")
}

/// The significant digits of the reddening tables' parts ([`ReddeningFile::columns`]).
const PART_DIGITS: usize = 9;

/// `value` to `digits` significant digits.
#[must_use]
fn rounded_to(value: f64, digits: usize) -> f64 {
    format!("{value:.*e}", digits - 1)
        .parse()
        .expect("a formatted float parses")
}

/// The items of one grid.
fn grid_items(prefix: &str, what: &str, grid: &Grid, eta_sun: f64) -> Vec<TableItem> {
    vec![
        TableItem::Array {
            name: format!("{prefix}_LOG_TEFF"),
            doc: vec![format!(
                "The temperature nodes of the {what} grid: log₁₀ of the effective temperature (K), rising."
            )],
            values: grid
                .teff
                .iter()
                .map(|&(t, _)| rounded(hyperion_sim::math::log10(f64::from(t))))
                .collect(),
        },
        TableItem::Array {
            name: format!("{prefix}_LOG_G"),
            doc: vec![format!(
                "The gravity nodes of the {what} grid: log₁₀ g (cgs), rising."
            )],
            values: grid.log_g.clone(),
        },
        TableItem::Source(rows_source(prefix, what, grid, eta_sun)),
        TableItem::Source(bake_source(prefix, what, grid)),
    ]
}

/// A value as the shortest decimal that reads back to it, with no digit separators: the table's
/// arrays are compact for the repository's 500 KB file limit and allow `unreadable_literal`.
#[must_use]
fn plain(value: f64) -> String {
    format!("{value:?}")
}

/// The attributes of every array of rows: unformatted, and their values data, not constants.
const ROW_ATTRIBUTES: &str = "#[rustfmt::skip]\n#[allow(\n    clippy::unreadable_literal,\n    \
    clippy::approx_constant,\n    reason = \"fitted data, compact for the file-size limit; a value near a constant is not one\"\n)]";

/// A grid's bake spectra as a `static` array, one row of fifteen a line, to seven significant digits, comma-separated with no spaces for the file-size limit.
fn bake_source(prefix: &str, what: &str, grid: &Grid) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(
        out,
        "/// The {what} grid's bake spectra, in the order of its rows: each row's mean spectral\n\
         /// irradiance over the fifteen bins of `BAKE_WAVELENGTHS_NM` (W m⁻² nm⁻¹) per lux of\n\
         /// photopic illuminance, so that the bins hold 1 lx.\n\
         {ROW_ATTRIBUTES}\npub static {prefix}_BAKE: [[f64; {BAKE_WAVELENGTH_COUNT}]; {}] = [",
        grid.rows.len()
    );
    for r in &grid.rows {
        let values: Vec<String> = r
            .extras
            .bake
            .iter()
            .map(|&v| {
                plain(
                    format!("{v:.6e}")
                        .parse()
                        .expect("a formatted float parses"),
                )
            })
            .collect();
        // Writing to a String cannot fail.
        let _ = writeln!(out, "    [{}],", values.join(","));
    }
    out.push_str("];\n");
    out
}

/// A grid's rows as a `static` array of eight columns, one row a line (the column order of
/// `sky::colour`'s `COLUMNS`): `r`, `g`, `lux_per_v0`, `sp_ratio`, `camera_band_mag`, and the red,
/// green and blue `A_c ÷ A_V`.
fn rows_source(prefix: &str, what: &str, grid: &Grid, eta_sun: f64) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(
        out,
        "/// The {what} grid's rows, temperature-major: row `i × {} + j` is temperature node `i`,\n\
         /// gravity node `j`; columns `r`, `g`, `lux_per_v0`, `sp_ratio`, `camera_band_mag`,\n\
         /// `extinction_r`, `extinction_g`, `extinction_b`.\n{ROW_ATTRIBUTES}\n\
         pub static {prefix}: [[f64; 8]; {}] = [",
        grid.log_g.len(),
        grid.rows.len()
    );
    for r in &grid.rows {
        // The camera term is unrounded, so that the Sun's interpolated term is zero to 10⁻¹⁵, and
        // saturated at the wire's range, −4.0 to +3.97 (decision-camera-eta), which only
        // PHOENIX's 2,300 K nodes at log g 0–0.5, gravities no star that cool has, pass.
        let camera = (-2.5 * hyperion_sim::math::log10(r.extras.camera_eta / eta_sun))
            .clamp(CAMERA_TERM_RANGE.0, CAMERA_TERM_RANGE.1);
        let values = [
            rounded(r.colour.r),
            rounded(r.colour.g),
            rounded(r.colour.lux_per_v0),
            rounded(r.colour.sp_ratio),
            camera,
            rounded(r.extras.extinction[0]),
            rounded(r.extras.extinction[1]),
            rounded(r.extras.extinction[2]),
        ]
        .map(plain);
        // Writing to a String cannot fail.
        let _ = writeln!(out, "    [{}],", values.join(","));
    }
    out.push_str("];\n");
    out
}

/// The sources of a grid's temperature ranges, as notes.
fn describe_sources(grid: &Grid) -> String {
    let mut out = String::new();
    let mut start = 0;
    for i in 1..=grid.teff.len() {
        if i == grid.teff.len() || grid.teff[i].1 != grid.teff[start].1 {
            // Writing to a String cannot fail.
            let _ = write!(
                out,
                "{}{} {}–{} K",
                if start == 0 { "" } else { "; " },
                grid.teff[start].1.label(),
                grid.teff[start].0,
                grid.teff[i - 1].0
            );
            start = i;
        }
    }
    out
}

/// The table's contents.
#[must_use]
pub fn render(table: &StarColourTable) -> RustTable {
    let clamps = |grid: &Grid| grid.clamped.len();
    let notes = format!(
        "\
Each row integrates one model spectrum over 1 nm bins from 360 to 1,100 nm against the CIE 1931
2° colour-matching functions, the CIE 1924 V(λ) and the CIE 1951 V′(λ) (CIE datasets, CC BY-SA
4.0) and Bessell and Murphy's (2012) photonic V: `r` and `g` are linear Rec. 709 (D65 white) at
unit luminance, desaturated towards white of the same luminance where out of gamut; `lux_per_v0`
is the photopic illuminance of a star of V = 0 with that spectrum (V zero point 3.631 × 10⁻⁹ erg
cm⁻² s⁻¹ Å⁻¹, Bessell, Castelli and Plez 1998) over 2.54 µlx; `sp_ratio` is ρ = 1,700 ∫S V′ ÷
(683 ∫S V). `camera_band_mag` is −2.5 log₁₀(η ÷ η☉), η = ∫S QE λ ÷ ∫S `R_V` λ the default
sensor's electrons per V-band photon (QE = 0.60 (1 − exp(−α 16 µm)) over 400–1,100 nm, α from
Green 2008's k for silicon) and η☉ = `CAMERA_ETA_SUN`; `extinction_r`, `_g` and `_b` are `A_c ÷ A_V`
at `R_V` = 3.1 from plan 07's `ccm::extinction_ratio` at each Rec. 709 channel's effective
wavelength over the V band's; the bake spectra are each spectrum's means over the fifteen 25.33 nm
bins from 380 to 760 nm, normalised to 1 lx through the bins' mean V(λ). Values are rounded to
seven significant digits, but for `camera_band_mag`.

Not white dwarfs: {normal}. White dwarfs: {wd}. A node a model set does not hold takes the
nearest gravity it holds at the same temperature ({nc} such nodes in the first grid, {wc} in the
second). The spectra are not redistributed; their provenance is
`crates/hyperion-fit/data/<dataset>/PROVENANCE.toml`.{smoke}",
        normal = describe_sources(&table.normal),
        wd = describe_sources(&table.white_dwarf),
        nc = clamps(&table.normal),
        wc = clamps(&table.white_dwarf),
        smoke = match table.spectra {
            Spectra::Models => "",
            Spectra::Blackbody =>
                "\n\nSMOKE RUN: every node holds a blackbody at its temperature, not its model.",
        },
    );
    let mut items = vec![TableItem::Array {
        name: "LUMINANCE_RGB".to_owned(),
        doc: vec![
            "The luminance of unit linear Rec. 709 red, green and blue: the Y row of the matrix"
                .to_owned(),
            "from the primaries and D65 (ITU-R BT.709-6), so b = (1 − `Y_r` r − `Y_g` g) ÷ `Y_b`."
                .to_owned(),
        ],
        values: table.luminance.to_vec(),
    }];
    items.push(TableItem::Scalar {
        name: "CAMERA_ETA_SUN".to_owned(),
        doc: vec![
            "η☉: the default sensor's electrons per V-band photon at 5,772 K and log g 4.438, the"
                .to_owned(),
            "quantity every row's `camera_band_mag` is relative to (decision-camera-eta)."
                .to_owned(),
        ],
        value: table.camera_eta_sun,
    });
    items.extend(grid_items(
        "NORMAL",
        "not-white-dwarf",
        &table.normal,
        table.camera_eta_sun,
    ));
    items.extend(grid_items(
        "WHITE_DWARF",
        "white-dwarf",
        &table.white_dwarf,
        table.camera_eta_sun,
    ));
    RustTable {
        summary: vec![
            "The colour, photopic flux and scotopic ratio of stars by effective temperature and gravity,"
                .to_owned(),
            "from model spectra (rendering plan R06, Design note 6), which `sky::colour` interpolates."
                .to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items,
    }
}

/// The reddening tasks' parameters as their manifests record them: the colour table's, and the
/// nodes of the secants, the sim's `sky::colour::REDDENING_A_V_NODES`.
fn reddening_parameters(spectra: Spectra, v_band: VBand) -> toml::Table {
    let mut table = parameters(spectra, v_band);
    table.insert(
        "a_v_nodes".to_owned(),
        toml::Value::Array(
            REDDENING_A_V_NODES
                .iter()
                .map(|&a| toml::Value::Float(a))
                .collect(),
        ),
    );
    table
}

/// The fit's parameters as its manifest records them.
fn parameters(spectra: Spectra, v_band: VBand) -> toml::Table {
    let text = format!(
        "spectra = \"{}\"\nv_band = \"{}\"\nbins_nm = [360, 1100]\nnormal_log_g = [0.0, 6.0]\n\
         white_dwarf_log_g = [6.5, 9.5]\n",
        match spectra {
            Spectra::Models => "models",
            Spectra::Blackbody => "blackbody",
        },
        v_band.as_str(),
    );
    toml::from_str(&text).expect("the parameters' own text is TOML")
}

/// Which V band the observer integrates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VBand {
    /// Bessell and Murphy's (2012) photonic V, fetched: the table.
    BessellMurphy,
    /// The CIE 1924 V(λ), photon-weighted: the smoke run's stand-in from committed data alone.
    CiePhotopic,
}

impl VBand {
    /// The manifest's name for it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BessellMurphy => "bessell_murphy_2012",
            Self::CiePhotopic => "cie_photopic",
        }
    }
}

/// Reads the default sensor from the committed `green2008_si` dataset.
///
/// # Errors
///
/// [`RunTaskError`] if the dataset cannot be read or is malformed.
pub fn read_sensor(manifest: &Manifest) -> Result<Sensor, RunTaskError> {
    sensor_of(&manifest.load_dataset("green2008_si")?)
}

/// The default sensor from the Green 2008 dataset.
fn sensor_of(set: &Dataset) -> Result<Sensor, RunTaskError> {
    let text = set
        .files
        .iter()
        .find(|(n, _)| n == "Green-2008.yml")
        .and_then(|(_, b)| std::str::from_utf8(b).ok())
        .ok_or_else(|| RunTaskError::Input {
            task: NAME,
            source: "`Green-2008.yml` is missing or not text".into(),
        })?;
    Sensor::from_green_2008(text).map_err(|e| RunTaskError::Input {
        task: NAME,
        source: Box::new(e),
    })
}

/// Reads the default sensor from the datasets under `data_dir`: for tests.
///
/// # Errors
///
/// [`RunTaskError`] if the dataset cannot be read or is malformed.
pub fn read_sensor_from_dir(data_dir: &Path) -> Result<Sensor, RunTaskError> {
    sensor_of(&load_dataset(data_dir, "green2008_si", None)?)
}

/// Reads the observer from the manifest's datasets.
///
/// # Errors
///
/// [`RunTaskError`] if a dataset cannot be read or a table is malformed.
pub fn read_observer(manifest: &Manifest, v_band: VBand) -> Result<Observer, RunTaskError> {
    let cie = manifest.load_dataset("cie_cmf")?;
    let bessell = match v_band {
        VBand::BessellMurphy => Some(manifest.load_dataset("bessell_murphy_2012")?),
        VBand::CiePhotopic => None,
    };
    observer_of(&cie, bessell.as_ref())
}

/// Reads the observer from the datasets under `data_dir` (the crate's `data/`), with Bessell and
/// Murphy's V if it is fetched and the photopic stand-in if not: for tests.
///
/// # Errors
///
/// [`RunTaskError`] if a dataset cannot be read or a table is malformed.
pub fn read_observer_from_dir(data_dir: &Path) -> Result<Observer, RunTaskError> {
    let cie = load_dataset(data_dir, "cie_cmf", None)?;
    // Only a V band that is not fetched falls back to the stand-in; a file that fails its hash or
    // a malformed provenance is an error.
    let bessell = match load_dataset(data_dir, "bessell_murphy_2012", None) {
        Ok(set) => Some(set),
        Err(LoadDatasetError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            None
        }
        Err(e) => return Err(e.into()),
    };
    observer_of(&cie, bessell.as_ref())
}

/// The observer from the CIE dataset and, if given, Bessell and Murphy's.
fn observer_of(cie: &Dataset, bessell: Option<&Dataset>) -> Result<Observer, RunTaskError> {
    let file = |set: &crate::data::Dataset, name: &str| -> Result<String, RunTaskError> {
        set.files
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, b)| String::from_utf8(b.clone()).ok())
            .ok_or_else(|| RunTaskError::Input {
                task: NAME,
                source: format!("`{name}` is missing or not text").into(),
            })
    };
    let table1 = bessell.map(|b| file(b, "table1.dat")).transpose()?;
    Observer::read(
        &file(cie, "CIE_xyz_1931_2deg.csv")?,
        &file(cie, "CIE_sle_photopic.csv")?,
        &file(cie, "CIE_sle_scotopic.csv")?,
        &file(cie, "CIE_std_illum_D65.csv")?,
        table1
            .as_deref()
            .map_or(VBandSource::CiePhotopic, VBandSource::BessellMurphy),
    )
    .map_err(|e| RunTaskError::Input {
        task: NAME,
        source: Box::new(e),
    })
}

/// The task: rendering plan R06's R06.T3.a, slow, revision [`VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct StarColourTask;

impl FitTask for StarColourTask {
    fn name(&self) -> &'static str {
        NAME
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "star_colour.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "LUMINANCE_RGB",
            "CAMERA_ETA_SUN",
            "NORMAL_LOG_TEFF",
            "NORMAL_LOG_G",
            "NORMAL",
            "NORMAL_BAKE",
            "WHITE_DWARF_LOG_TEFF",
            "WHITE_DWARF_LOG_G",
            "WHITE_DWARF",
            "WHITE_DWARF_BAKE",
        ]
    }

    /// Plan 07's extinction law, which the reddening columns read, at the blue, green and red
    /// channels' and the V band's typical effective wavelengths.
    fn fingerprint(&self) -> SimFingerprint {
        law_fingerprint()
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let (spectra, v_band) = read_params(manifest, parameters)?;
        let observer = read_observer(manifest, v_band)?;
        let sensor = read_sensor(manifest)?;
        let table = fit(manifest, &observer, &sensor, spectra, threads)?;
        let mut acceptance = acceptance(&observer, &table);
        if spectra == Spectra::Models {
            acceptance.push_str(&pickles_acceptance(manifest, &observer, &table)?);
        }
        Ok(TaskOutput {
            source: SOURCE.to_owned(),
            acceptance,
            table: render(&table),
            provisional: match spectra {
                Spectra::Models => None,
                Spectra::Blackbody => Some("R06.T3.a smoke run"),
            },
        })
    }
}

/// The spectra and the V band a manifest asks for, its parameters checked against `expected`, the
/// code's.
///
/// # Errors
///
/// [`RunTaskError`] if a parameter is missing, not one the task knows, or disagrees with the
/// code's constants.
fn read_params(
    manifest: &Manifest,
    expected: fn(Spectra, VBand) -> toml::Table,
) -> Result<(Spectra, VBand), RunTaskError> {
    let spectra = match manifest.str("spectra")? {
        "models" => Spectra::Models,
        "blackbody" => Spectra::Blackbody,
        _ => {
            return Err(ManifestParamError::new("spectra", "\"models\" or \"blackbody\"").into());
        }
    };
    let v_band = match manifest.str("v_band")? {
        "bessell_murphy_2012" => VBand::BessellMurphy,
        "cie_photopic" => VBand::CiePhotopic,
        _ => {
            return Err(ManifestParamError::new(
                "v_band",
                "\"bessell_murphy_2012\" or \"cie_photopic\"",
            )
            .into());
        }
    };
    manifest.expect_params(&expected(spectra, v_band))?;
    Ok((spectra, v_band))
}

/// The citations of the colour tables' fits, for their headers' `source`.
const SOURCE: &str = "model spectra of Husser et al. (2013, A&A 553, A6), Castelli and Kurucz (2003, IAU \
                     Symp. 210, A20), Lanz and Hubeny (2003, ApJS 146, 417), TMAP (Werner et al. \
                     2003, ASP Conf. Ser. 288, 31; Rauch and Deetjen 2003, ibid. 103) and Koester (2010, Mem. S. A. It. 81, 921), as the \
                     Spanish Virtual Observatory and the PHOENIX site serve them; the CIE's 1931 2° \
                     colour-matching functions, 1924 V(λ), 1951 V′(λ) and D65 (CC BY-SA 4.0); \
                     Bessell and Murphy (2012, PASP 124, 140) V; retrieved 2026-10-02";

/// The extinction law's probes the colour table's fingerprint reads: plan 07's law at the blue,
/// green and red channels' and the V band's typical effective wavelengths.
#[must_use]
fn law_fingerprint() -> SimFingerprint {
    SimFingerprint::new(
        [0.45, 0.53, 0.55, 0.61]
            .iter()
            .map(|&um| {
                (
                    format!("galaxy::gas::ccm::extinction_ratio({um} µm)"),
                    extinction_ratio(Micrometres::new(um))
                        .expect("an optical wavelength is in the law's range"),
                )
            })
            .collect(),
    )
}

/// The reddening tables' revision, written into each header; bumped when anything below moves
/// them. Revision 0, R06.T9.e's first four columns, never reached the integration branch; revision
/// 1 is the addendum's, with the node at `A_V` 7.5 (none has reached it either).
pub const REDDENING_VERSION: u32 = 1;

/// One of the five reddening tables (R06.T9.e; decided 2026-10-06, `decision-r06-t9b-band.md`,
/// addendum, with the node at `A_V` 7.5 ruled the same day): the 78 columns a row are some 1.4 MB
/// at seven significant digits, so they are split by node to keep each file under the repository's
/// 500 KB limit, as the pair-evolved light's three tables are split. Every table is row for row
/// `star_colour`'s, from the same spectra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReddeningFile {
    /// `star_colour_reddening`: each part's value at unit luminance and each band's moment, the
    /// curves at `A_V` → 0 (15 columns).
    AtZero,
    /// `star_colour_reddening_av02_05`: every band's secant at `A_V` 2 and 5 (18 columns).
    Av02To05,
    /// `star_colour_reddening_av07p5`: at 7.5 (9 columns), the node ruled for the camera term
    /// between 5 and 10.
    Av07p5,
    /// `star_colour_reddening_av10_15`: at 10 and 15.
    Av10To15,
    /// `star_colour_reddening_av20_30`: at 20 and 30.
    Av20To30,
}

impl ReddeningFile {
    /// Every file, in name order.
    pub const ALL: [Self; 5] = [
        Self::AtZero,
        Self::Av02To05,
        Self::Av07p5,
        Self::Av10To15,
        Self::Av20To30,
    ];

    /// The table's name, which is its task's.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AtZero => "star_colour_reddening",
            Self::Av02To05 => "star_colour_reddening_av02_05",
            Self::Av07p5 => "star_colour_reddening_av07p5",
            Self::Av10To15 => "star_colour_reddening_av10_15",
            Self::Av20To30 => "star_colour_reddening_av20_30",
        }
    }

    /// The table's file, relative to the sim's `tables/`.
    #[must_use]
    pub const fn table_path(self) -> &'static str {
        match self {
            Self::AtZero => "star_colour_reddening.rs",
            Self::Av02To05 => "star_colour_reddening_av02_05.rs",
            Self::Av07p5 => "star_colour_reddening_av07p5.rs",
            Self::Av10To15 => "star_colour_reddening_av10_15.rs",
            Self::Av20To30 => "star_colour_reddening_av20_30.rs",
        }
    }

    /// The indices in [`REDDENING_A_V_NODES`] of a node file's nodes; `None` for the curves at
    /// `A_V` → 0.
    #[must_use]
    pub const fn nodes(self) -> Option<Range<usize>> {
        match self {
            Self::AtZero => None,
            Self::Av02To05 => Some(0..2),
            Self::Av07p5 => Some(2..3),
            Self::Av10To15 => Some(3..5),
            Self::Av20To30 => Some(5..7),
        }
    }

    /// The columns of `r`'s row in this table, as written: the parts and the moments, or the
    /// secants at the file's nodes, node-major. The parts take nine significant digits, so that
    /// the raw colour they give, a difference of two, lifts to the colour table's chroma within
    /// 10⁻⁶ (the addendum's test 4); every other column seven.
    #[must_use]
    pub fn columns(self, r: &Reddening) -> Vec<f64> {
        match self.nodes() {
            None => r
                .parts
                .iter()
                .map(|&p| rounded_to(p, PART_DIGITS))
                .chain(r.moments.iter().map(|&m| rounded(m)))
                .collect(),
            Some(nodes) => r.secants[nodes]
                .iter()
                .flatten()
                .map(|&k| rounded(k))
                .collect(),
        }
    }
}

/// The task writing one of the reddening tables ([`ReddeningFile`]): rendering plan R06's
/// R06.T9.e, slow, revision [`REDDENING_VERSION`].
///
/// It integrates the same spectra on the same grids as [`StarColourTask`], through the same
/// [`fit`], and writes only its file's columns of [`Reddening`], row for row with `star_colour`'s.
/// The five tasks share their manifests' parameters, their fingerprint and the fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StarColourReddeningTask {
    /// The table it writes.
    pub file: ReddeningFile,
}

/// The curves at `A_V` → 0, `star_colour_reddening`.
pub static REDDENING_TASK: StarColourReddeningTask = StarColourReddeningTask {
    file: ReddeningFile::AtZero,
};

/// The secants at `A_V` 2 and 5.
pub static REDDENING_AV02_05_TASK: StarColourReddeningTask = StarColourReddeningTask {
    file: ReddeningFile::Av02To05,
};

/// The secants at `A_V` 7.5.
pub static REDDENING_AV07P5_TASK: StarColourReddeningTask = StarColourReddeningTask {
    file: ReddeningFile::Av07p5,
};

/// The secants at `A_V` 10 and 15.
pub static REDDENING_AV10_15_TASK: StarColourReddeningTask = StarColourReddeningTask {
    file: ReddeningFile::Av10To15,
};

/// The secants at `A_V` 20 and 30.
pub static REDDENING_AV20_30_TASK: StarColourReddeningTask = StarColourReddeningTask {
    file: ReddeningFile::Av20To30,
};

impl FitTask for StarColourReddeningTask {
    fn name(&self) -> &'static str {
        self.file.name()
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        self.file.table_path()
    }

    fn revision(&self) -> u32 {
        REDDENING_VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        match self.file {
            ReddeningFile::AtZero => &["NORMAL_REDDENING", "WHITE_DWARF_REDDENING"],
            ReddeningFile::Av02To05
            | ReddeningFile::Av07p5
            | ReddeningFile::Av10To15
            | ReddeningFile::Av20To30 => &["A_V_NODES", "NORMAL_SECANTS", "WHITE_DWARF_SECANTS"],
        }
    }

    /// The sim probes every reddening table reads ([`reddening_fingerprint`]).
    fn fingerprint(&self) -> SimFingerprint {
        reddening_fingerprint()
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let (spectra, v_band) = read_params(manifest, reddening_parameters)?;
        let observer = read_observer(manifest, v_band)?;
        let sensor = read_sensor(manifest)?;
        let table = fit(manifest, &observer, &sensor, spectra, threads)?;
        Ok(TaskOutput {
            source: SOURCE.to_owned(),
            acceptance: reddening_acceptance(&observer, &table, self.file),
            table: render_reddening(&table, self.file),
            provisional: match spectra {
                Spectra::Models => None,
                Spectra::Blackbody => Some("R06.T9.e smoke run"),
            },
        })
    }
}

/// Three fixed colours of unit luminance for the fingerprint's probes of the sim's lift: one in
/// gamut, one with blue below zero and one with green below zero. The lowest channel, lifted to
/// zero, is not probed.
const LIFT_PROBES: [(&str, [f64; 3], [usize; 2]); 3] = [
    ("in gamut", [1.2, 0.9, 1.0], [0, 1]),
    ("blue below zero", [2.5, 0.8, -1.0], [0, 1]),
    ("green below zero", [1.4, -0.2, 3.0], [0, 2]),
];

/// The sim probes every reddening table reads: plan 07's law across the bins' range, its infrared
/// branch beyond 0.91 µm and the blend to 1.1 µm among them; the nodes, the sim's
/// `REDDENING_A_V_NODES`, the dust's transmission through each at 0.8 µm by the sim's `exp10`
/// and its `log10`, through which every secant is taken; and the sim's `lift_into_gamut` of
/// [`LIFT_PROBES`], which the parts are lifted by.
#[must_use]
fn reddening_fingerprint() -> SimFingerprint {
    let law = |um: f64| {
        extinction_ratio(Micrometres::new(um)).expect("an optical wavelength is in the law's range")
    };
    let mut probes: Vec<(String, f64)> = [
        0.38, 0.40, 0.45, 0.50, 0.53, 0.55, 0.61, 0.70, 0.80, 0.90, 1.00, 1.10,
    ]
    .iter()
    .map(|&um| {
        (
            format!("galaxy::gas::ccm::extinction_ratio({um} µm)"),
            law(um),
        )
    })
    .collect();
    for a in REDDENING_A_V_NODES {
        probes.push((format!("sky::colour::REDDENING_A_V_NODES ({a})"), a));
        let through = hyperion_sim::math::exp10(-0.4 * law(0.80) * a);
        probes.push((
            format!("math::exp10(−0.4 extinction_ratio(0.8 µm) × {a})"),
            through,
        ));
        // Every secant is a log₁₀ of a transmission.
        probes.push((
            format!("math::log10(exp10(−0.4 extinction_ratio(0.8 µm) × {a}))"),
            hyperion_sim::math::log10(through),
        ));
    }
    for (name, rgb, channels) in LIFT_PROBES {
        let lifted = lift_into_gamut(rgb);
        for c in channels {
            probes.push((
                format!("sky::colour::lift_into_gamut({name})[{c}]"),
                lifted[c],
            ));
        }
    }
    SimFingerprint::new(probes)
}

/// A reddening table's contents: one `static` a grid, row for row the colour table's.
#[must_use]
pub fn render_reddening(table: &StarColourTable, file: ReddeningFile) -> RustTable {
    let smoke = match table.spectra {
        Spectra::Models => "",
        Spectra::Blackbody => {
            "\n\nSMOKE RUN: every node holds a blackbody at its temperature, not its model."
        }
    };
    let common = "\
Each row is the row of the same index in `star_colour`'s grid of the same name, from the same
model spectrum over the same 1 nm bins (R06.T9.e; `decision-r06-t9b-band.md`, item 3 and its
addendum). Every extinction is over plan 07's sightline `A_V`, the law's normalisation at x = 1.82
µm⁻¹ (0.549 µm) times the dust's column. The nine bands are the six parts of the Rec. 709
colour-matching functions (the matrix applied to x̄, ȳ and z̄, each split into its positive and
negative part: r̄⁺, r̄⁻, ḡ⁺, ḡ⁻, b̄⁺ and b̄⁻), photon-free as the tristimulus integrals are; Bessell
and Murphy's photonic V, `R_V` λ; the CIE 1951 V′(λ); and the default camera's QE λ, QE = 0.60 (1 −
exp(−α 16 µm)) over 400–1,100 nm with α from Green's (2008, Sol. Energ. Mat. Sol. Cells 92, 1305)
k for silicon. A band of weights w is dimmed by T(A) = ∫S w 10^(−0.4 ℓ A) ÷ ∫S w, ℓ plan 07's
`ccm::extinction_ratio` (Cardelli, Clayton and Mathis 1989, ApJ 345, 245, at `R_V` 3.1 to
0.9 µm, joined to Gordon et al. 2023, ApJ 950, 86, by 1.1 µm; ℓ = 1 at x = 1.82 µm⁻¹), and its
secant is k(A) = −2.5 log₁₀ T(A) ÷ A; the sim takes each secant as piecewise linear in A through
its moment and the nodes, and held beyond 30. The photopic light has no column: it is the parts'
Rec. 709 luminance. Values are rounded to seven significant digits, the parts to nine. The
columns are split by node across five tables, each under the repository's 500 KB file limit.";
    let own = match file.nodes() {
        None => "\
This table holds, per row, each part's value per unit of the unreddened luminance, ∫S c̄± ÷ ∫S
ȳ (the raw colour of unit luminance, before T3's lift into gamut, is c⁺ − c⁻), and each band's
first moment of the law, ∫S w ℓ ÷ ∫S w, its secant's limit as `A_V` → 0."
            .to_owned(),
        Some(held) => format!(
            "This table holds, per row, every band's secant at `A_V` {} (`A_V_NODES`), of\nthe sim's `REDDENING_A_V_NODES`.",
            node_list(held, " and at ")
        ),
    };
    let notes = format!("{common}\n\n{own}{smoke}");
    let summary = match file.nodes() {
        None => vec![
            "The reddening of the colour table's light as `A_V` → 0: each colour-matching function's parts"
                .to_owned(),
            "and each band's moment, for every row (rendering plan R06, R06.T9.e).".to_owned(),
        ],
        Some(held) => vec![
            format!(
                "The reddening of the colour table's light at `A_V` {}: each band's secant, for every",
                node_list(held, " and ")
            ),
            "row (rendering plan R06, R06.T9.e).".to_owned(),
        ],
    };
    let (normal, white_dwarf) = match file.nodes() {
        None => ("NORMAL_REDDENING", "WHITE_DWARF_REDDENING"),
        Some(_) => ("NORMAL_SECANTS", "WHITE_DWARF_SECANTS"),
    };
    let mut items = Vec::new();
    if let Some(held) = file.nodes() {
        let first = held.start;
        items.push(TableItem::Array {
            name: "A_V_NODES".to_owned(),
            doc: vec![
                if held.len() == 2 {
                    format!(
                        "The table's two nodes, mag of the sightline's `A_V`: nodes {first} and {} of the sim's",
                        first + 1
                    )
                } else {
                    format!("The table's node, mag of the sightline's `A_V`: node {first} of the sim's")
                },
                "`REDDENING_A_V_NODES`.".to_owned(),
            ],
            values: REDDENING_A_V_NODES[held].to_vec(),
        });
    }
    items.push(TableItem::Source(reddening_source(
        normal,
        "NORMAL",
        "not-white-dwarf",
        &table.normal,
        file,
    )));
    items.push(TableItem::Source(reddening_source(
        white_dwarf,
        "WHITE_DWARF",
        "white-dwarf",
        &table.white_dwarf,
        file,
    )));
    RustTable {
        summary,
        notes: notes.lines().map(str::to_owned).collect(),
        items,
    }
}

/// The `A_V` of `nodes` of [`REDDENING_A_V_NODES`], joined by `and`: "2 and 5", or "7.5".
#[must_use]
fn node_list(nodes: Range<usize>, and: &str) -> String {
    REDDENING_A_V_NODES[nodes]
        .iter()
        .map(f64::to_string)
        .collect::<Vec<_>>()
        .join(and)
}

/// The names of the nine bands, in their column order, for the tables' documentation.
const BAND_NAMES: &str = "`r⁺`, `r⁻`, `g⁺`, `g⁻`, `b⁺`, `b⁻`, `v`, `scotopic`, `camera`";

/// A grid's columns of `file` as a `static` array, one row a line.
#[must_use]
fn reddening_source(
    name: &str,
    prefix: &str,
    what: &str,
    grid: &Grid,
    file: ReddeningFile,
) -> String {
    let (count, columns) = match file.nodes() {
        None => (
            PART_COUNT + BAND_COUNT,
            format!(
                "the six parts' values at unit luminance, `r⁺`, `r⁻`, `g⁺`, `g⁻`, `b⁺` and `b⁻`, then\n\
                 /// the nine bands' moments, {BAND_NAMES}"
            ),
        ),
        Some(nodes) => (
            nodes.len() * BAND_COUNT,
            if nodes.len() == 2 {
                format!(
                    "the nine bands' secants at `A_V` {}, {BAND_NAMES}, then the same at\n/// `A_V` {}",
                    REDDENING_A_V_NODES[nodes.start],
                    REDDENING_A_V_NODES[nodes.start + 1]
                )
            } else {
                format!(
                    "the nine bands' secants at `A_V` {}, {BAND_NAMES}",
                    REDDENING_A_V_NODES[nodes.start]
                )
            },
        ),
    };
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(
        out,
        "/// The {what} grid's rows, in the order of `star_colour`'s `{prefix}`: columns\n\
         /// {columns}.\n{ROW_ATTRIBUTES}\n\
         pub static {name}: [[f64; {count}]; {}] = [",
        grid.rows.len()
    );
    for r in &grid.rows {
        let values: Vec<String> = file.columns(&r.reddening).into_iter().map(plain).collect();
        // Writing to a String cannot fail.
        let _ = writeln!(out, "    [{}],", values.join(","));
    }
    out.push_str("];\n");
    out
}

/// The ruling's ranges for the colour table's solar row as `A_V` → 0, inclusive
/// (`decision-r06-t9b-band.md`, item 3, as its addendum corrects them, items 3 and 4; the sim's
/// `sky::colour` test repeats them): `A_P ÷ A_V`, `A_S ÷ A_V − A_P ÷ A_V`, `A_cam ÷ A_V` and the
/// V band's own.
pub const SOLAR_REDDENING_RANGES: [(f64, f64); 4] =
    [(0.97, 1.01), (0.11, 0.15), (0.84, 0.92), (0.99, 1.02)];

/// The photopic, scotopic, camera and V bands' extinction over the sightline's `A_V` as `A_V` →
/// 0, from a row's `parts` and `moments`: the scotopic's, the camera's and V's are their moments,
/// and the photopic's is the parts' moments weighted by their luminance, Σ Y<sub>c</sub> (c⁺ m⁺ −
/// c⁻ m⁻) ÷ Σ Y<sub>c</sub> (c⁺ − c⁻), `luminance` the Rec. 709 Y<sub>c</sub>.
#[must_use]
pub fn ratios_at_zero(
    parts: &[f64; PART_COUNT],
    moments: &[f64; BAND_COUNT],
    luminance: [f64; 3],
) -> [f64; 4] {
    let (mut moment, mut light) = (0.0, 0.0);
    for c in 0..3 {
        moment +=
            luminance[c] * (parts[2 * c] * moments[2 * c] - parts[2 * c + 1] * moments[2 * c + 1]);
        light += luminance[c] * (parts[2 * c] - parts[2 * c + 1]);
    }
    [
        moment / light,
        moments[SCOTOPIC_BAND],
        moments[CAMERA_BAND],
        moments[V_BAND],
    ]
}

/// A row's columns of `file`, at [`SUN`], interpolated as the sim's `sky::colour` interpolates
/// the table: bilinearly in log `T_eff` and log g between the rounded values of the four nodes about
/// it.
#[must_use]
fn solar_columns(grid: &Grid, file: ReddeningFile) -> Vec<f64> {
    let log_t: Vec<f64> = grid
        .teff
        .iter()
        .map(|&(t, _)| rounded(hyperion_sim::math::log10(f64::from(t))))
        .collect();
    let (ti, tf) = pickles::bracket(&log_t, hyperion_sim::math::log10(SUN.0));
    let (gi, gf) = pickles::bracket(&grid.log_g, SUN.1);
    let row = |i: usize, j: usize| file.columns(&grid.row(i, j).reddening);
    let (a, b, c, d) = (
        row(ti, gi),
        row(ti, gi + 1),
        row(ti + 1, gi),
        row(ti + 1, gi + 1),
    );
    (0..a.len())
        .map(|k| {
            let low = a[k] * (1.0 - gf) + b[k] * gf;
            let high = c[k] * (1.0 - gf) + d[k] * gf;
            low * (1.0 - tf) + high * tf
        })
        .collect()
}

/// The photopic light's secant at `a` mag from a row's parts and the parts' secants there: the
/// parts' luminance, −2.5 log₁₀(Σ Y<sub>c</sub> (c⁺ T⁺ − c⁻ T⁻) ÷ Σ Y<sub>c</sub> (c⁺ − c⁻)) ÷ a.
#[must_use]
fn photopic_secant(parts: &[f64], secants: &[f64], luminance: [f64; 3], a: f64) -> f64 {
    let (mut left, mut light) = (0.0, 0.0);
    for c in 0..3 {
        let through = |p: usize| parts[p] * hyperion_sim::math::exp10(-0.4 * secants[p] * a);
        left += luminance[c] * (through(2 * c) - through(2 * c + 1));
        light += luminance[c] * (parts[2 * c] - parts[2 * c + 1]);
    }
    -2.5 * hyperion_sim::math::log10(left / light) / a
}

/// A node file's measured figures for its header: the solar row's secants at its two nodes,
/// the photopic light's from the parts.
#[must_use]
fn node_acceptance(
    observer: &Observer,
    table: &StarColourTable,
    file: ReddeningFile,
    nodes: Range<usize>,
) -> String {
    let luminance = observer.luminance();
    let sun = solar_columns(&table.normal, file);
    let at_zero = solar_columns(&table.normal, ReddeningFile::AtZero);
    let mut out = String::from("at 5,772 K and log g 4.438, the secants");
    for (k, &a) in REDDENING_A_V_NODES[nodes].iter().enumerate() {
        let s = &sun[k * BAND_COUNT..(k + 1) * BAND_COUNT];
        // Writing to a String cannot fail.
        let _ = write!(
            out,
            "{} at `A_V` {a}: photopic {:.4}, scotopic {:.4}, V {:.4}, camera {:.4}",
            if k == 0 { "" } else { ";" },
            photopic_secant(&at_zero[..PART_COUNT], s, luminance, a),
            s[SCOTOPIC_BAND],
            s[V_BAND],
            s[CAMERA_BAND],
        );
    }
    out
}

/// The reddening's checks over every row of both grids, for the header of the curves at `A_V`
/// → 0.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RowChecks {
    /// The rows checked.
    rows: usize,
    /// The largest photopic identity's difference (the addendum's test 5).
    identity: f64,
    /// The rows out of gamut unreddened, which T3's lift moved.
    lifted: usize,
    /// The rows out of gamut behind the last node's dust.
    lifted_at_last_node: usize,
    /// Whether the sim's `lift_into_gamut` of every row's raw colour, unreddened and behind the
    /// last node's dust, is `unit_rgb`'s, bit for bit (test 4's fit half, on both of the lift's
    /// branches).
    lift_holds: bool,
    /// Whether every band's depth k A rises through the nodes on every row.
    rising: bool,
}

impl RowChecks {
    /// The checks of `table`, whose rows `observer` integrated.
    #[must_use]
    fn of(observer: &Observer, table: &StarColourTable) -> Self {
        let rows: Vec<&TableRow> = table
            .normal
            .rows
            .iter()
            .chain(&table.white_dwarf.rows)
            .collect();
        Self {
            rows: rows.len(),
            identity: rows
                .iter()
                .map(|r| r.reddening.photopic_identity)
                .fold(0.0, f64::max),
            lifted: rows
                .iter()
                .filter(|r| observer.raw_rgb(r.reddening.xyz).iter().any(|&c| c < 0.0))
                .count(),
            lifted_at_last_node: rows
                .iter()
                .filter(|r| {
                    observer
                        .raw_rgb(r.reddening.xyz_at_last_node)
                        .iter()
                        .any(|&c| c < 0.0)
                })
                .count(),
            // `total_cmp` is equal exactly when the bits are.
            lift_holds: rows.iter().all(|r| {
                [r.reddening.xyz, r.reddening.xyz_at_last_node]
                    .iter()
                    .all(|&xyz| {
                        lift_into_gamut(observer.raw_rgb(xyz))
                            .iter()
                            .zip(&observer.unit_rgb(xyz))
                            .all(|(a, b)| a.total_cmp(b).is_eq())
                    })
            }),
            rising: rows.iter().all(|r| {
                let s = r.reddening.secants;
                (0..BAND_COUNT).all(|b| {
                    (1..REDDENING_NODE_COUNT).all(|n| {
                        s[n][b] * REDDENING_A_V_NODES[n] > s[n - 1][b] * REDDENING_A_V_NODES[n - 1]
                    })
                })
            }),
        }
    }
}

/// A reddening table's measured figures for its header.
///
/// For the curves at `A_V` → 0: the solar row's ratios against the ruling's ranges, their spread
/// over the dwarfs, and three checks over every row of both grids: the photopic identity (the
/// addendum's test 5), the sim's lift against `unit_rgb` bit for bit (test 4's fit half), and every
/// band's depth k A rising through the nodes. For a node file: the solar row's secants there.
#[must_use]
fn reddening_acceptance(
    observer: &Observer,
    table: &StarColourTable,
    file: ReddeningFile,
) -> String {
    if let Some(nodes) = file.nodes() {
        return node_acceptance(observer, table, file, nodes);
    }
    let luminance = observer.luminance();
    let verdict = |pass: bool| if pass { "passes" } else { "FAILS" };
    let sun = solar_columns(&table.normal, file);
    let parts: [f64; PART_COUNT] = std::array::from_fn(|p| sun[p]);
    let moments: [f64; BAND_COUNT] = std::array::from_fn(|b| sun[PART_COUNT + b]);
    let [photopic, scotopic, camera, v] = ratios_at_zero(&parts, &moments, luminance);
    let [p_range, s_range, c_range, v_range] = SOLAR_REDDENING_RANGES;
    let within = |value: f64, (lo, hi): (f64, f64)| (lo..=hi).contains(&value);
    let j = table
        .normal
        .log_g
        .iter()
        .position(|&g| (g - 4.5).abs() < 1e-9)
        .unwrap_or(0);
    let dwarfs: Vec<[f64; 4]> = table
        .normal
        .teff
        .iter()
        .enumerate()
        .filter(|(_, (t, _))| (3_000..=30_000).contains(t))
        .map(|(i, _)| {
            let r = table.normal.row(i, j).reddening;
            ratios_at_zero(&r.parts, &r.moments, luminance)
        })
        .collect();
    let spread = |k: usize| {
        dwarfs
            .iter()
            .map(|r| r[k])
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            })
    };
    let RowChecks {
        rows,
        identity,
        lifted,
        lifted_at_last_node,
        lift_holds,
        rising,
    } = RowChecks::of(observer, table);
    let ((p_lo, p_hi), (s_lo, s_hi), (c_lo, c_hi), (v_lo, v_hi)) =
        (spread(0), spread(1), spread(2), spread(3));
    format!(
        "at 5,772 K and log g 4.438, as `A_V` → 0, `A_P ÷ A_V` {photopic:.4} (in {:.2}–{:.2}; {}), \
         `A_S ÷ A_V` {scotopic:.4}, their difference {:.4} (in {:.2}–{:.2}; {}), `A_cam ÷ A_V` \
         {camera:.4} (in {:.2}–{:.2}; {}) and V's own {v:.4} (in {:.2}–{:.2}; {}); over the dwarfs' \
         rows (log g 4.5) from 3,000 to 30,000 K, `A_P ÷ A_V` {p_lo:.4}–{p_hi:.4}, `A_S ÷ A_V` \
         {s_lo:.4}–{s_hi:.4}, `A_cam ÷ A_V` {c_lo:.4}–{c_hi:.4}, V's {v_lo:.4}–{v_hi:.4}; over \
         every row of both grids, the parts' photopic transmission against the direct V(λ) \
         integral at every node within {identity:.1e} (the addendum's 10⁻⁴; {}), the sim's \
         `lift_into_gamut` of the raw colour `unit_rgb`'s colour bit for bit on all {} rows, {lifted} \
         of them lifted, and behind `A_V` 30, {lifted_at_last_node} lifted ({}), and every band's \
         depth k A rising through the nodes ({})",
        p_range.0,
        p_range.1,
        verdict(within(photopic, p_range)),
        scotopic - photopic,
        s_range.0,
        s_range.1,
        verdict(within(scotopic - photopic, s_range)),
        c_range.0,
        c_range.1,
        verdict(within(camera, c_range)),
        v_range.0,
        v_range.1,
        verdict(within(v, v_range)),
        verdict(identity < 1e-4),
        rows,
        verdict(lift_holds),
        verdict(rising),
    )
}

/// The measured figures the header records: ρ of Illuminant A and the Sun's blackbody, D65's
/// white, and the spread of `lux_per_v0` over the main sequence.
fn acceptance(observer: &Observer, table: &StarColourTable) -> String {
    let bb = |t: f64| observer.row(&Spectrum::blackbody(t).bin_means());
    let eta = table.camera_eta_sun;
    let d65 = observer.unit_rgb(observer.integrals(observer.d65()).xyz);
    let d65_off = d65.iter().map(|c| (c - 1.0).abs()).fold(0.0, f64::max);
    let (lo, hi) = lux_range(&table.normal);
    format!(
        "ρ of a 2,856 K blackbody {:.3} (Illuminant A: 1.41), of 5,772 K {:.3} (2.32); D65 to \
         r = g = b within {d65_off:.1e}; `lux_per_v0` over the dwarfs' rows (log g 4.5) from \
         2,300 to 45,000 K spans {lo:.3}–{hi:.3}; η☉ {eta:.4} (decision-camera-eta: 3.0 ± 0.1)",
        bb(2_856.0).sp_ratio,
        bb(5_772.0).sp_ratio,
    )
}

/// The Pickles comparison's figures for the header, each type's Δ(u′, v′) and the exceptions'
/// reasons.
///
/// # Errors
///
/// [`RunTaskError`] if the library cannot be read.
fn pickles_acceptance(
    manifest: &Manifest,
    observer: &Observer,
    table: &StarColourTable,
) -> Result<String, RunTaskError> {
    let library = manifest.load_dataset("pickles1998")?;
    let spectra = pickles::PICKLES_TYPES
        .iter()
        .map(|ty| {
            let text = library
                .files
                .iter()
                .find(|(n, _)| n == ty.file)
                .and_then(|(_, b)| std::str::from_utf8(b).ok())
                .ok_or_else(|| RunTaskError::Input {
                    task: NAME,
                    source: format!("Pickles' `{}` is missing or not text", ty.file).into(),
                })?;
            pickles::parse_pickles(text).map_err(|e| RunTaskError::Input {
                task: NAME,
                source: Box::new(e),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let deltas = pickles::compare(observer, table, &spectra);
    let mut out = String::from(
        "; Pickles (1998) against the table at each type's temperature and gravity, Δ(u′, v′):",
    );
    for (ty, d) in pickles::PICKLES_TYPES.iter().zip(&deltas) {
        let verdict = if *d < ty.limit { "passes" } else { "FAILS" };
        // Writing to a String cannot fail.
        let _ = write!(
            out,
            " {} at {} K {d:.4} (under {}; {verdict}{})",
            ty.file
                .trim_start_matches("uk")
                .trim_end_matches(".dat")
                .to_uppercase(),
            ty.teff,
            ty.limit,
            ty.exception
                .map_or_else(String::new, |why| format!(": an exception, {why}")),
        );
        out.push(',');
    }
    out.pop();
    Ok(out)
}

/// The least and greatest `lux_per_v0` at log g 4.5 from 2,300 to 45,000 K.
fn lux_range(grid: &Grid) -> (f64, f64) {
    let j = grid
        .log_g
        .iter()
        .position(|&g| (g - 4.5).abs() < 1e-9)
        .unwrap_or(0);
    grid.teff
        .iter()
        .enumerate()
        .filter(|(_, (t, _))| (2_300..=45_000).contains(t))
        .map(|(i, _)| grid.row(i, j).colour.lux_per_v0)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_give_their_models() {
        assert_eq!(
            Source::Atlas9.model_of_file("t005750_g4.50.txt"),
            Some((5_750, 450))
        );
        assert_eq!(
            Source::TmapHelium.model_of_file("he30_t060000_g5.00.txt"),
            Some((60_000, 500))
        );
        assert_eq!(
            Source::TmapHydrogen.model_of_file("he30_t060000_g5.00.txt"),
            None
        );
        assert_eq!(
            Source::Phoenix
                .model_of_file("lte02300-0.50-0.0.PHOENIX-ACES-AGSS-COND-2011-HiRes.fits"),
            Some((2_300, 50))
        );
        assert_eq!(Source::Phoenix.model_of_file(PHOENIX_WAVE), None);
    }

    #[test]
    fn a_grid_fills_an_edge_from_the_nearest_gravity_and_refuses_a_gap() {
        let observer = photometry::tests::observer();
        let sensor = columns::tests::sensor();
        let row = |t: f64| table_row(&observer, &sensor, &Spectrum::blackbody(t));
        let held = BTreeMap::from([(
            Source::Atlas9,
            BTreeMap::from([((5_000, 200), row(4_000.0)), ((5_000, 400), row(6_000.0))]),
        )]);
        let build = |log_g: &[f64]| {
            build_grid(
                vec![(5_000, Source::Atlas9)],
                log_g,
                &held,
                &observer,
                &sensor,
                Spectra::Models,
            )
        };
        let grid = build(&[1.0, 2.0, 4.0, 5.0]).unwrap();
        assert_eq!(grid.rows[0], row(4_000.0));
        assert_eq!(grid.rows[3], row(6_000.0));
        assert_eq!(grid.clamped, [(5_000, 1.0, 2.0), (5_000, 5.0, 4.0)]);
        assert!(matches!(build(&[3.0]), Err(RunTaskError::Input { .. })));
        let none = build_grid(
            vec![(9_000, Source::Atlas9)],
            &[4.0],
            &held,
            &observer,
            &sensor,
            Spectra::Models,
        );
        assert!(matches!(none, Err(RunTaskError::Input { .. })));
    }

    #[test]
    fn the_nodes_rise_and_join_their_sources() {
        for nodes in [normal_teff_nodes(), white_dwarf_teff_nodes()] {
            assert!(nodes.windows(2).all(|w| w[0].0 < w[1].0), "{nodes:?}");
            assert_eq!(nodes.last().map(|n| n.1), Some(Source::Blackbody));
        }
        assert_eq!(normal_teff_nodes().len(), 12 + 39 + 14 + 12 + 5 + 5);
        assert_eq!(centi_of(4.5), 450);
        assert_eq!(centi_of(0.0), 0);
        assert_eq!(centi("6.50"), Some(650));
    }
}
