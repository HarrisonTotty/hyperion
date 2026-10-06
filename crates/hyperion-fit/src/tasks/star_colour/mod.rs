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
use std::path::{Path, PathBuf};

use hyperion_sim::galaxy::gas::ccm::extinction_ratio;
use hyperion_sim::sky::colour::{SUN_LOG_G, SUN_TEFF_K};
use hyperion_sim::units::Micrometres;

use crate::data::{Dataset, LoadDatasetError, Provenance, load_dataset};
use crate::emit::{RustTable, TableItem, sha256_hex};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::map_reduce_chunks;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

pub use columns::{BAKE_WAVELENGTH_COUNT, Extras, Reddening, Sensor, bake_wavelengths_nm};
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
    /// The eye's and the camera's reddening, the `star_colour_reddening` table's (R06.T9.e).
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
    let Some(dataset) = source.dataset() else {
        return Ok(BTreeMap::new());
    };
    let provenance = Provenance::load(manifest.data_root(), dataset)?;
    let dir = manifest.data_dir(dataset).map_or_else(
        || provenance.default_dir(manifest.data_root()),
        PathBuf::from,
    );
    let files: Vec<(String, String, (u32, i32))> = provenance
        .files
        .iter()
        .filter_map(|f| {
            let model = source.model_of_file(&f.name)?;
            wanted
                .contains(&model.0)
                .then(|| (f.name.clone(), f.sha256.clone(), model))
        })
        .collect();
    let read_file = |name: &str, sha: &str| -> Result<Vec<u8>, RunTaskError> {
        let path = dir.join(name);
        let bytes = fs::read(&path).map_err(|source| LoadDatasetError::Io {
            path: path.clone(),
            source,
        })?;
        let found = sha256_hex(&bytes);
        if found != sha {
            return Err(LoadDatasetError::HashMismatch { path, found }.into());
        }
        Ok(bytes)
    };
    let input = |e: ReadSpectrumError| RunTaskError::Input {
        task: NAME,
        source: Box::new(e),
    };
    // PHOENIX's spectra share one wavelength file, in Å in vacuum; only 300–1,200 nm is kept.
    let phoenix_wave = if source == Source::Phoenix {
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
            spectrum::read_fits_vector(&read_file(&wave.name, &wave.sha256)?).map_err(input)?;
        let lo = angstrom.partition_point(|&w| w < 3_000.0);
        let hi = angstrom.partition_point(|&w| w <= 12_000.0);
        Some((
            lo,
            hi,
            angstrom[lo..hi]
                .iter()
                .map(|w| w / 10.0)
                .collect::<Vec<_>>(),
        ))
    } else {
        None
    };
    let count = u64::try_from(files.len()).expect("a file count fits in u64");
    let mut results = Vec::with_capacity(files.len());
    map_reduce_chunks(
        count,
        1,
        threads,
        |range| -> Result<Vec<ModelRow>, RunTaskError> {
            let mut out = Vec::new();
            for i in range {
                let (name, sha, model) =
                    &files[usize::try_from(i).expect("i is below files.len(), a usize")];
                let bytes = read_file(name, sha)?;
                let spectrum = if let Some((lo, hi, nm)) = &phoenix_wave {
                    let flux = spectrum::read_fits_vector(&bytes).map_err(input)?;
                    let flux = flux
                        .get(*lo..*hi)
                        .ok_or_else(|| {
                            input(ReadSpectrumError::Malformed(
                                "a PHOENIX spectrum is shorter than its wavelengths",
                            ))
                        })?
                        .to_vec();
                    Spectrum::new(nm.clone(), flux).map_err(input)?
                } else {
                    let text = std::str::from_utf8(&bytes).map_err(|_| {
                        input(ReadSpectrumError::Malformed("a spectrum file is not text"))
                    })?;
                    spectrum::parse_svo_ascii(text).map_err(input)?
                };
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

/// The reddening task's parameters as its manifest records them: the colour table's, and the
/// extinction in V of the camera's second column.
fn reddening_parameters(spectra: Spectra, v_band: VBand) -> toml::Table {
    let mut table = parameters(spectra, v_band);
    table.insert(
        "camera_reddening_a_v".to_owned(),
        toml::Value::Float(columns::CAMERA_REDDENING_A_V),
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

/// The citations of both colour tables' fits, for their headers' `source`.
const SOURCE: &str = "model spectra of Husser et al. (2013, A&A 553, A6), Castelli and Kurucz (2003, IAU \
                     Symp. 210, A20), Lanz and Hubeny (2003, ApJS 146, 417), TMAP (Werner et al. \
                     2003, ASP Conf. Ser. 288, 31; Rauch and Deetjen 2003, ibid. 103) and Koester (2010, Mem. S. A. It. 81, 921), as the \
                     Spanish Virtual Observatory and the PHOENIX site serve them; the CIE's 1931 2° \
                     colour-matching functions, 1924 V(λ), 1951 V′(λ) and D65 (CC BY-SA 4.0); \
                     Bessell and Murphy (2012, PASP 124, 140) V; retrieved 2026-10-02";

/// The extinction law's probes both colour tables' fingerprints read: plan 07's law at the blue,
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

/// The reddening table's name.
const REDDENING_NAME: &str = "star_colour_reddening";

/// The reddening table's revision, written into its header; bumped when anything below moves it.
pub const REDDENING_VERSION: u32 = 0;

/// The task `star_colour_reddening`: the colour table's four reddening columns (rendering plan
/// R06, R06.T9.e; decided 2026-10-06, `decision-r06-t9b-band.md`, item 3), slow, revision
/// [`REDDENING_VERSION`].
///
/// It integrates the same spectra on the same grids as [`StarColourTask`], through the same
/// [`fit`], and writes only `A_P ÷ A_V`, `A_S ÷ A_V` and the camera's two `A_cam ÷ A_V`
/// ([`Reddening`]), row for row with `star_colour`'s. They are a table of their own because the
/// colour table's file holds no more under the repository's 500 KB file limit (497 KB as T3 left
/// it; the four columns add some 70 KB), as the pair-evolved light's three tables are split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct StarColourReddeningTask;

impl FitTask for StarColourReddeningTask {
    fn name(&self) -> &'static str {
        REDDENING_NAME
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "star_colour_reddening.rs"
    }

    fn revision(&self) -> u32 {
        REDDENING_VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "CAMERA_REDDENING_A_V",
            "NORMAL_REDDENING",
            "WHITE_DWARF_REDDENING",
        ]
    }

    /// Plan 07's extinction law, which every column reads: [`StarColourTask`]'s probes, and the
    /// law across the camera's band, its infrared branch beyond 0.91 µm among them.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(
            [0.40, 0.45, 0.53, 0.55, 0.61, 0.80, 1.00]
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

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let (spectra, v_band) = read_params(manifest, reddening_parameters)?;
        let observer = read_observer(manifest, v_band)?;
        let sensor = read_sensor(manifest)?;
        let table = fit(manifest, &observer, &sensor, spectra, threads)?;
        Ok(TaskOutput {
            source: SOURCE.to_owned(),
            acceptance: reddening_acceptance(&observer, &sensor, &table),
            table: render_reddening(&table),
            provisional: match spectra {
                Spectra::Models => None,
                Spectra::Blackbody => Some("R06.T9.e smoke run"),
            },
        })
    }
}

/// The reddening table's contents: one `static` a grid, four columns a row, in the colour
/// table's row order.
#[must_use]
pub fn render_reddening(table: &StarColourTable) -> RustTable {
    let notes = format!(
        "\
Each row is the row of the same index in `star_colour`'s grid of the same name, from the same
model spectrum over the same 1 nm bins (R06.T9.e; `decision-r06-t9b-band.md`, item 3).
`photopic` and `scotopic` are `A_P ÷ A_V` and `A_S ÷ A_V` at `R_V` = 3.1: plan 07's
`ccm::extinction_ratio` at the effective wavelength of the spectrum under the CIE 1924 V(λ) and
the CIE 1951 V′(λ), photon-free, over the law at the V band's photon-weighted effective
wavelength, as `star_colour`'s display channels are taken. `camera_0` and `camera_2` are
`A_cam ÷ A_V` for the default sensor at `A_V` → 0 and at `A_V` = {a_v}, as two integrals over the
same dust, since the camera's band is too broad for one effective wavelength: for dust of d at
0.55 µm a band of weights w is dimmed by −2.5 log₁₀(∫S w 10^(−0.4 ℓ(λ) d) ÷ ∫S w), ℓ plan 07's
law, w = QE λ for the camera (400–1,100 nm) and `R_V` λ for V; `camera_0` is the ratio of the two
bands' mean ℓ, and `camera_2` the ratio at the d that dims V by {a_v}. The sim takes the camera's
ratio as linear in `A_V` between them. Values are rounded to seven significant digits. The
columns are a table of their own because `star_colour.rs` holds no more under the repository's
500 KB file limit.{smoke}",
        a_v = columns::CAMERA_REDDENING_A_V,
        smoke = match table.spectra {
            Spectra::Models => "",
            Spectra::Blackbody =>
                "\n\nSMOKE RUN: every node holds a blackbody at its temperature, not its model.",
        },
    );
    RustTable {
        summary: vec![
            "The extinction of the eye's photopic and scotopic light and of the default camera's band"
                .to_owned(),
            "over the V band's, for every row of the colour table (rendering plan R06, R06.T9.e)."
                .to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![
            TableItem::Scalar {
                name: "CAMERA_REDDENING_A_V".to_owned(),
                doc: vec![
                    "The extinction in V of the camera's second column, mag: `camera_2` is its"
                        .to_owned(),
                    "`A_cam ÷ A_V` there, and the sim takes the ratio as linear in `A_V` through it."
                        .to_owned(),
                ],
                value: columns::CAMERA_REDDENING_A_V,
            },
            TableItem::Source(reddening_source("NORMAL", "not-white-dwarf", &table.normal)),
            TableItem::Source(reddening_source(
                "WHITE_DWARF",
                "white-dwarf",
                &table.white_dwarf,
            )),
        ],
    }
}

/// A grid's reddening columns as a `static` array of four columns, one row a line: `photopic`,
/// `scotopic`, `camera_0` and `camera_2`.
#[must_use]
fn reddening_source(prefix: &str, what: &str, grid: &Grid) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(
        out,
        "/// The {what} grid's reddening, in the order of `star_colour`'s `{prefix}`: columns\n\
         /// `photopic` (`A_P ÷ A_V`), `scotopic` (`A_S ÷ A_V`), and `camera_0` and `camera_2`\n\
         /// (`A_cam ÷ A_V` at `A_V` → 0 and at `A_V` = 2).\n{ROW_ATTRIBUTES}\n\
         pub static {prefix}_REDDENING: [[f64; 4]; {}] = [",
        grid.rows.len()
    );
    for r in &grid.rows {
        let values = [
            rounded(r.reddening.photopic),
            rounded(r.reddening.scotopic),
            rounded(r.reddening.camera[0]),
            rounded(r.reddening.camera[1]),
        ]
        .map(plain);
        // Writing to a String cannot fail.
        let _ = writeln!(out, "    [{}],", values.join(","));
    }
    out.push_str("];\n");
    out
}

/// The ruling's ranges for the colour table's solar row (`decision-r06-t9b-band.md`, item 3, as
/// R06.T9.e's tests take them; the sim's `sky::colour` test repeats them), inclusive:
/// `A_P ÷ A_V`, `A_S ÷ A_V − A_P ÷ A_V`, and `A_cam ÷ A_V` at `A_V` → 0.
pub const SOLAR_REDDENING_RANGES: [(f64, f64); 3] = [(0.97, 1.00), (0.11, 0.15), (0.78, 0.88)];

/// The reddening columns at [`SUN`], interpolated as the sim's `sky::colour` interpolates the
/// table: bilinearly in log `T_eff` and log g between the rounded values of the four nodes about
/// it.
#[must_use]
pub fn solar_reddening(grid: &Grid) -> [f64; 4] {
    let log_t: Vec<f64> = grid
        .teff
        .iter()
        .map(|&(t, _)| rounded(hyperion_sim::math::log10(f64::from(t))))
        .collect();
    let (ti, tf) = pickles::bracket(&log_t, hyperion_sim::math::log10(SUN.0));
    let (gi, gf) = pickles::bracket(&grid.log_g, SUN.1);
    let column = |i: usize, j: usize, k: usize| {
        let r = grid.row(i, j).reddening;
        rounded([r.photopic, r.scotopic, r.camera[0], r.camera[1]][k])
    };
    std::array::from_fn(|k| {
        let low = column(ti, gi, k) * (1.0 - gf) + column(ti, gi + 1, k) * gf;
        let high = column(ti + 1, gi, k) * (1.0 - gf) + column(ti + 1, gi + 1, k) * gf;
        low * (1.0 - tf) + high * tf
    })
}

/// The reddening table's measured figures for its header: the solar row's columns against the
/// ruling's ranges, their spread over the dwarfs, and the effective-wavelength method against the
/// broadband integrals.
#[must_use]
fn reddening_acceptance(observer: &Observer, sensor: &Sensor, table: &StarColourTable) -> String {
    let [photopic, scotopic, camera_0, camera_2] = solar_reddening(&table.normal);
    let verdict = |value: f64, (lo, hi): (f64, f64)| {
        if (lo..=hi).contains(&value) {
            "passes"
        } else {
            "FAILS"
        }
    };
    let [p_range, s_range, c_range] = SOLAR_REDDENING_RANGES;
    let j = table
        .normal
        .log_g
        .iter()
        .position(|&g| (g - 4.5).abs() < 1e-9)
        .unwrap_or(0);
    let dwarfs: Vec<Reddening> = table
        .normal
        .teff
        .iter()
        .enumerate()
        .filter(|(_, (t, _))| (3_000..=30_000).contains(t))
        .map(|(i, _)| table.normal.row(i, j).reddening)
        .collect();
    let spread = |f: fn(&Reddening) -> f64| {
        dwarfs
            .iter()
            .map(f)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            })
    };
    let (p_lo, p_hi) = spread(|r| r.photopic);
    let (s_lo, s_hi) = spread(|r| r.scotopic);
    let (c_lo, c_hi) = spread(|r| r.camera[0]);
    let worst = method_error(observer, sensor);
    format!(
        "at 5,772 K and log g 4.438 `A_P ÷ A_V` {photopic:.4} (in {:.2}–{:.2}; {}), `A_S ÷ A_V` \
         {scotopic:.4}, their difference {:.4} (in {:.2}–{:.2}; {}), `A_cam ÷ A_V` {camera_0:.4} at \
         `A_V` → 0 (in {:.2}–{:.2}; {}) and {camera_2:.4} at `A_V` 2; over the dwarfs' rows (log g \
         4.5) from 3,000 to 30,000 K, `A_P ÷ A_V` {p_lo:.4}–{p_hi:.4}, `A_S ÷ A_V` \
         {s_lo:.4}–{s_hi:.4}, `A_cam ÷ A_V` at `A_V` → 0 {c_lo:.4}–{c_hi:.4}; the photopic and \
         scotopic ratios at their effective wavelengths against the broadband integrals at `A_V` 1, \
         blackbodies of 3,000–30,000 K: within {worst:.4} (the ruling's 0.003; {})",
        p_range.0,
        p_range.1,
        verdict(photopic, p_range),
        scotopic - photopic,
        s_range.0,
        s_range.1,
        verdict(scotopic - photopic, s_range),
        c_range.0,
        c_range.1,
        verdict(camera_0, c_range),
        if worst < 0.003 { "passes" } else { "FAILS" },
    )
}

/// The largest difference, over blackbodies of 3,000 to 30,000 K, between the photopic and
/// scotopic ratios at their effective wavelengths and their broadband values over the V band's at
/// `A_V` 1: the effective-wavelength method's error.
#[must_use]
fn method_error(observer: &Observer, sensor: &Sensor) -> f64 {
    let mut worst: f64 = 0.0;
    for teff in [3_000.0, 4_000.0, 5_772.0, 10_000.0, 30_000.0] {
        let bins = Spectrum::blackbody(teff).bin_means();
        let columns = columns::reddening(observer, sensor, &bins);
        let [photopic, scotopic] = columns::broadband_eye_reddening(observer, &bins, 1.0);
        worst = worst
            .max((photopic - columns.photopic).abs())
            .max((scotopic - columns.scotopic).abs());
    }
    worst
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
