//! The infrared sets (plan 14, P14.T49.i): each row's optical constants beyond 780 nm, read from
//! the task's datasets and joined to its R08 file there, as a [`SpectralIndex`].
//!
//! # The joins
//!
//! Every row's index runs from its R08 file's first wavelength, 380 nm, to 780 nm, where the
//! files end, by the file's own reduction (n linear and k linear in ln k in wavelength, as
//! [`super::index_at`] reads it). Beyond it, each set of `substance::optics::InfraredIndex`, as
//! its primary's reduction asks, to 5 µm or the set's end:
//!
//! - `WaterIce`: Warren and Brandt 2008 at 266 K, n and ln k linear in ln λ (their table's
//!   recommendation, as R08's file reproduces it to the row), with Grundy and Schmitt 1998's k at
//!   100 K over 962.3–2,742 nm in place of theirs, k = αλ ÷ 4π at each row and linear in λ between
//!   rows, n Warren and Brandt's; the steps where the sets meet are stated by
//!   `substance_albedos_each_infrared_set_joins_its_file_at_780_nm`;
//! - `CarbonDioxideIce`, `Tholin`, `EnstatiteGlass`, `AmorphousForsterite` and `ZincSulphide`:
//!   their source's n and k, n linear and k linear in ln k in λ, R08's reduction of the same
//!   tables;
//! - `Basalt` and `BasalticGlass`: likewise, n from the rows that give n and k from the rows that
//!   give k, as R08's reduction;
//! - `MethaneIce`: Grundy, Schmitt and Quirico 2002's k at 40 K, k = αλ ÷ 4π at each row and linear
//!   in λ between rows, to its last row at 4,912.66 nm, with Martonchik and Orton 1994's n linear
//!   in λ between its rows to 2,381 nm and held there beyond: past it the table's next rows are the
//!   3.3 µm band's extrema (3.315–3.330 µm), three rows at 3.534–3.565 µm (n 1.318–1.321, within
//!   0.005 of the held 1.316) and then 7.675 µm, between which a straight line is not the
//!   continuum, and the grains are opaque inside the band whatever n is;
//! - `NitrogenIce`: Grundy, Schmitt and Quirico 1993's k at 41 K over its band, 1,999.9–2,300.2
//!   nm, k = αλ ÷ 4π, and 0 elsewhere, with n held at the file's 780 nm value;
//! - `CarbonMonoxideIce`: Gerakines, Materese and Hudson 2023's n and k at 25 K, linear in
//!   wavenumber between their 0.48 cm⁻¹ blocks, from 3,333.5 nm, their k taken as 0 below its
//!   noise floor ([`CO_NOISE_FLOOR`]); and k = 0 with n held at the file's 780 nm value below
//!   3,333.5 nm;
//! - `MarsDust`: the index table Wolff et al. distribute, R08's `mars-dust.json`'s own source, n
//!   linear and k linear in ln k in λ, as R08's reduction of it;
//! - `AmmoniaIce`: Martonchik, Orton and Appleby 1984's n linear in λ between their rows from
//!   0.668 µm, and their k linear in ln k from 1,462 nm, their measured infrared; k = 0 below it,
//!   continuing the file's statement.
//!
//! A negative k or α elsewhere, noise about zero, is kept in the knots and taken as 0 where w is
//! computed ([`super::spectral::spectral_single_scattering_albedo`]).
//!
//! Every reader checks its knots before it builds a [`Piece`] or a [`Curve`]: a set whose rows do
//! not lie where its joins need them is refused with [`FitAlbedoError::Infrared`], never a panic.

use std::collections::BTreeMap;

use hyperion_sim::substance::optics::{
    CondensedOptics, InfraredIndex, SPECTRAL_BIN_EDGES_NM, SPECTRAL_BINS,
};

use super::spectral::{Curve, Interpolation, Piece, SpectralIndex};
use super::{FitAlbedoError, NAME};
use crate::manifest::Manifest;
use crate::materials::MaterialFile;
use crate::task::RunTaskError;

/// Where R08's index files end and each infrared set joins its file, nm: 780.
pub(crate) const JOIN_NM: f64 = 780.0;

/// Where every row's data end at the latest, nm: the bins' end, 5,000.
pub(crate) const REACH_END_NM: f64 = SPECTRAL_BIN_EDGES_NM[SPECTRAL_BINS];

/// Where Martonchik and Orton 1994's real index of methane ice is held, nm: its last row before
/// the 3.3 µm band, 2,381.
pub(crate) const METHANE_N_HELD_FROM_NM: f64 = 2_381.0;

/// Where Martonchik, Orton and Appleby 1984's measured infrared of ammonia ice begins, nm: their
/// 6,840 cm⁻¹ row, 1,462.
pub(crate) const AMMONIA_MEASURED_FROM_NM: f64 = 1_462.0;

/// The noise floor of Gerakines, Materese and Hudson 2023's k, below which `CarbonMonoxideIce`
/// takes it as 0, in place of clipping the noise at 0: three times the rms of their k over
/// [`CO_NOISE_WINDOW_PER_CM`], 3.40 × 10⁻⁴ over its 1,245 rows (their mean there −1.3 × 10⁻⁴),
/// measured on the dataset by this lane: 1.02 × 10⁻³. Above it stay the fundamental (2,135–2,143
/// cm⁻¹), ¹³CO's band (2,088–2,093 cm⁻¹), the fundamental's high-wavenumber wing to 2,233 cm⁻¹ and
/// scattered rows of the low-wavenumber wing, where k is near the floor
/// (`substance_albedos_co_s_noise_floor_is_three_times_its_baseline_rms`).
///
/// The floor discards absorption that matters: at 4.6 µm a k at the floor would give 100 µm grains
/// Θ ≈ 0.80 and w ≈ 0.71, so `CO`'s 3.5–5 µm bin is an upper bound (the row's sources).
pub(crate) const CO_NOISE_FLOOR: f64 = 1.02e-3;

/// The band-free window over which [`CO_NOISE_FLOOR`] is measured, cm⁻¹: 2,400–3,000 (3.33–4.17
/// µm), above α-CO's fundamental and its wing, and above 2,300–2,400 cm⁻¹, where the table's
/// baseline dips to −1.5 × 10⁻³ (its mean there −5.8 × 10⁻⁴).
#[cfg(test)]
pub(crate) const CO_NOISE_WINDOW_PER_CM: [f64; 2] = [2_400.0, 3_000.0];

/// The text of each of the task's datasets, by name: each of its files' text, by file name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct InfraredData {
    texts: BTreeMap<String, BTreeMap<String, String>>,
}

impl InfraredData {
    /// Reads each of `manifest`'s datasets, each file of UTF-8 text, verified against its
    /// provenance.
    ///
    /// # Errors
    ///
    /// [`RunTaskError::Dataset`] if a dataset cannot be read or fails its hash, and
    /// [`RunTaskError::Input`] if a file is not UTF-8 text.
    pub(crate) fn load(manifest: &Manifest) -> Result<Self, RunTaskError> {
        let mut texts = BTreeMap::new();
        for name in manifest.datasets() {
            let dataset = manifest.load_dataset(name)?;
            let mut files = BTreeMap::new();
            for (file, bytes) in dataset.files {
                let text = String::from_utf8(bytes).map_err(|_| input(name, "not UTF-8"))?;
                files.insert(file, text);
            }
            texts.insert(name.clone(), files);
        }
        Ok(Self { texts })
    }

    /// The text of dataset `name`, a dataset of one file.
    fn text(&self, name: &'static str) -> Result<&str, FitAlbedoError> {
        let mut files = self
            .texts
            .get(name)
            .ok_or_else(|| malformed(name, "it is not one of the manifest's datasets"))?
            .values();
        match (files.next(), files.next()) {
            (Some(text), None) => Ok(text.as_str()),
            _ => Err(malformed(name, "it is not one file")),
        }
    }

    /// Every file's text of dataset `name`, by file name, or `None` if the manifest has no such
    /// dataset.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn files(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        self.texts.get(name)
    }

    /// Takes dataset `name` out, every file's text by file name, or `None` if it holds no such
    /// dataset: for a reader that owns them, as the laboratory library does splib07a's.
    pub(crate) fn take(&mut self, name: &str) -> Option<BTreeMap<String, String>> {
        self.texts.remove(name)
    }
}

/// The task's input error for dataset `name`, `why`.
fn input(name: &str, why: &str) -> RunTaskError {
    RunTaskError::Input {
        task: NAME,
        source: format!("the dataset `{name}` cannot be read: {why}").into(),
    }
}

/// The error of an infrared dataset that is not as its reader expects.
fn malformed(dataset: &'static str, why: &str) -> FitAlbedoError {
    FitAlbedoError::Infrared {
        dataset,
        why: why.to_owned(),
    }
}

/// The rows of a refractiveindex.info file's `data: |` block: (λ µm, n, k), λ rising strictly.
///
/// # Errors
///
/// [`FitAlbedoError::Infrared`] if it has no such block, a row is not three numbers, or the
/// wavelengths do not rise.
pub(crate) fn refractiveindex_rows(
    dataset: &'static str,
    text: &str,
) -> Result<Vec<[f64; 3]>, FitAlbedoError> {
    let (before, block) = text
        .split_once("data: |")
        .ok_or_else(|| malformed(dataset, "it has no `data: |` block"))?;
    let key_indent = before.len() - before.rfind('\n').map_or(0, |i| i + 1);
    let indent = |line: &str| line.len() - line.trim_start().len();
    let mut rows = Vec::new();
    for line in block
        .lines()
        .skip(1)
        .take_while(|line| line.trim().is_empty() || indent(line) > key_indent)
        .filter(|line| !line.trim().is_empty())
    {
        let fields: Vec<f64> = line
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|_| malformed(dataset, "a row is not numbers"))?;
        let [um, n, k] = fields[..] else {
            return Err(malformed(dataset, "a row is not λ, n and k"));
        };
        rows.push([um, n, k]);
    }
    rising(dataset, &rows)?;
    Ok(rows)
}

/// The rows of one of the task's reduced tables: a header line `header`, then rows of as many
/// comma-separated numbers (`NaN` for a value the source does not give), the first rising
/// strictly.
///
/// # Errors
///
/// [`FitAlbedoError::Infrared`] if the header is not `header`, a row is not numbers, or the first
/// column does not rise.
pub(crate) fn table_rows(
    dataset: &'static str,
    text: &str,
    header: &str,
) -> Result<Vec<Vec<f64>>, FitAlbedoError> {
    let mut lines = text.lines();
    if lines.next() != Some(header) {
        return Err(malformed(dataset, &format!("its header is not `{header}`")));
    }
    let columns = header.split(',').count();
    let mut rows = Vec::new();
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let row: Vec<f64> = line
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|_| malformed(dataset, "a row is not numbers"))?;
        if row.len() != columns || !row[0].is_finite() {
            return Err(malformed(dataset, "a row is not its header's columns"));
        }
        rows.push(row);
    }
    rising(dataset, &rows)?;
    Ok(rows)
}

/// Refuses rows whose first column does not rise strictly.
fn rising<R: AsRef<[f64]>>(dataset: &'static str, rows: &[R]) -> Result<(), FitAlbedoError> {
    if rows.len() < 2
        || !rows
            .windows(2)
            .all(|pair| pair[0].as_ref()[0] < pair[1].as_ref()[0])
    {
        return Err(malformed(
            dataset,
            "its rows are fewer than two or do not rise",
        ));
    }
    Ok(())
}

/// The imaginary index k = αλ ÷ 4π of a Lambert absorption coefficient `alpha_per_cm` (cm⁻¹) at
/// `wavelength_nm`.
#[must_use]
pub(crate) fn k_of_alpha(alpha_per_cm: f64, wavelength_nm: f64) -> f64 {
    alpha_per_cm * 100.0 * wavelength_nm * 1.0e-9 / (4.0 * core::f64::consts::PI)
}

/// The knots (λ nm, k) of one of Grundy's tables of α (cm⁻¹) by λ (µm).
fn absorption_knots(dataset: &'static str, text: &str) -> Result<Vec<(f64, f64)>, FitAlbedoError> {
    Ok(table_rows(dataset, text, "wavelength_um,alpha_per_cm")?
        .into_iter()
        .map(|row| {
            let nm = row[0] * 1_000.0;
            (nm, k_of_alpha(row[1], nm))
        })
        .collect())
}

/// The (λ nm, value) knots of column `column` of `rows` of λ in µm, leaving out the rows that do
/// not give it.
fn knots(rows: &[Vec<f64>], column: usize) -> Vec<(f64, f64)> {
    rows.iter()
        .filter(|row| row[column].is_finite())
        .map(|row| (row[0] * 1_000.0, row[column]))
        .collect()
}

/// The piece of R08's file: its n linear and k linear in ln k in λ, from its first wavelength to
/// its last, which is [`JOIN_NM`].
///
/// # Errors
///
/// [`FitAlbedoError::OffGrid`] if the file does not end at [`JOIN_NM`].
pub(crate) fn visible_piece(
    file_name: &'static str,
    file: &MaterialFile,
) -> Result<Piece, FitAlbedoError> {
    let grid = file.wavelengths_nm();
    let (first, last) = (grid[0], grid[grid.len() - 1]);
    if last.total_cmp(&JOIN_NM).is_ne() {
        return Err(FitAlbedoError::OffGrid {
            file: file_name,
            wavelength_nm: JOIN_NM,
        });
    }
    let pairs = |values: &[f64]| -> Vec<(f64, f64)> {
        grid.iter().copied().zip(values.iter().copied()).collect()
    };
    Ok(Piece::new(
        first,
        last,
        Curve::new(&pairs(file.n()), Interpolation::Linear),
        Curve::new(&pairs(file.k()), Interpolation::LogLinear),
    ))
}

/// A table of λ (µm), n and k read as R08 reduced its visible: n linear and k linear in ln k in λ,
/// each from the rows that give it, from [`JOIN_NM`] to [`REACH_END_NM`].
fn nk_piece(dataset: &'static str, text: &str) -> Result<Piece, FitAlbedoError> {
    let rows = table_rows(dataset, text, "wavelength_um,n,k")?;
    let (n, k) = (knots(&rows, 1), knots(&rows, 2));
    for column in [&n, &k] {
        let covers = column.first().is_some_and(|f| f.0 <= JOIN_NM)
            && column.last().is_some_and(|l| l.0 >= REACH_END_NM);
        if !covers {
            return Err(malformed(dataset, "its rows do not span 780-5,000 nm"));
        }
    }
    Ok(Piece::new(
        JOIN_NM,
        REACH_END_NM,
        Curve::new(&n, Interpolation::Linear),
        Curve::new(&k, Interpolation::LogLinear),
    ))
}

/// Refuses a set's band over `lo`–`hi` nm, the knots of dataset `dataset` inside its pieces,
/// unless it lies strictly between [`JOIN_NM`] and [`REACH_END_NM`]: the pieces before and after
/// it would be empty.
fn inside_the_joins(dataset: &'static str, lo: f64, hi: f64) -> Result<(), FitAlbedoError> {
    if lo > JOIN_NM && hi < REACH_END_NM {
        Ok(())
    } else {
        Err(malformed(
            dataset,
            "its band does not lie between 780 and 5,000 nm",
        ))
    }
}

/// The pieces of `set` beyond [`JOIN_NM`], from `data`, for a row whose R08 file is `file`.
///
/// # Errors
///
/// [`FitAlbedoError::Infrared`] if a dataset is missing or not as its reader expects: a reduced
/// table not as [`table_rows`] or [`refractiveindex_rows`] reads it, or its rows not where the
/// set's joins need them (a band that reaches 780 or 5,000 nm, a set that ends before 780 nm, or no
/// row where a column is read).
pub(crate) fn infrared_pieces(
    set: InfraredIndex,
    file: &MaterialFile,
    data: &InfraredData,
) -> Result<Vec<Piece>, FitAlbedoError> {
    let file_n = Curve::constant(file.n()[file.n().len() - 1]);
    let zero = || Curve::constant(0.0);
    let one = |dataset: &'static str| -> Result<Vec<Piece>, FitAlbedoError> {
        Ok(vec![nk_piece(dataset, data.text(dataset)?)?])
    };
    match set {
        InfraredIndex::WaterIce => water_pieces(data),
        InfraredIndex::MethaneIce => methane_pieces(data),
        InfraredIndex::NitrogenIce => {
            let grundy = "grundy_1993_n2";
            let band = absorption_knots(grundy, data.text(grundy)?)?;
            let (lo, hi) = (band[0].0, band[band.len() - 1].0);
            inside_the_joins(grundy, lo, hi)?;
            Ok(vec![
                Piece::new(JOIN_NM, lo, file_n.clone(), zero()),
                Piece::new(
                    lo,
                    hi,
                    file_n.clone(),
                    Curve::new(&band, Interpolation::Linear),
                ),
                Piece::new(hi, REACH_END_NM, file_n, zero()),
            ])
        }
        InfraredIndex::CarbonMonoxideIce => carbon_monoxide_pieces(file_n, data),
        InfraredIndex::AmmoniaIce => ammonia_pieces(data),
        InfraredIndex::CarbonDioxideIce => one("warren_1986_co2"),
        InfraredIndex::Tholin => one("khare_1984_tholin"),
        InfraredIndex::EnstatiteGlass => one("dorschner_1995_mgsio3"),
        InfraredIndex::AmorphousForsterite => one("jager_2003_mg2sio4"),
        InfraredIndex::Basalt => one("pollack_1973_basalt"),
        InfraredIndex::BasalticGlass => one("pollack_1973_basaltic_glass"),
        InfraredIndex::ZincSulphide => one("palik_1985_zns"),
        InfraredIndex::MarsDust => one("wolff_2009_mars_dust"),
    }
}

/// `MethaneIce`'s piece: Grundy et al. 2002's k from [`JOIN_NM`] to its last row, with Martonchik
/// and Orton 1994's n to [`METHANE_N_HELD_FROM_NM`], held beyond.
fn methane_pieces(data: &InfraredData) -> Result<Vec<Piece>, FitAlbedoError> {
    let (grundy, martonchik) = ("grundy_2002_ch4", "martonchik_orton_1994_ch4");
    let k = absorption_knots(grundy, data.text(grundy)?)?;
    let n: Vec<(f64, f64)> = refractiveindex_rows(martonchik, data.text(martonchik)?)?
        .iter()
        .map(|r| (r[0] * 1_000.0, r[1]))
        // Half a nanometre's margin, for the rows' micrometres in binary.
        .filter(|&(nm, _)| (399.5..=METHANE_N_HELD_FROM_NM + 0.5).contains(&nm))
        .collect();
    if n.is_empty() {
        return Err(malformed(
            martonchik,
            "it has no row over 400-2,381 nm, where its n is read",
        ));
    }
    let end = k[k.len() - 1].0;
    if end <= JOIN_NM {
        return Err(malformed(grundy, "its rows end at or before 780 nm"));
    }
    Ok(vec![Piece::new(
        JOIN_NM,
        end,
        Curve::new(&n, Interpolation::Linear),
        Curve::new(&k, Interpolation::Linear),
    )])
}

/// `WaterIce`'s pieces: Warren and Brandt 2008, with Grundy and Schmitt 1998's k over its range.
fn water_pieces(data: &InfraredData) -> Result<Vec<Piece>, FitAlbedoError> {
    let (warren, grundy) = ("warren_brandt_2008_h2o", "grundy_schmitt_1998_h2o");
    let rows = refractiveindex_rows(warren, data.text(warren)?)?;
    let n_knots: Vec<(f64, f64)> = rows.iter().map(|r| (r[0] * 1_000.0, r[1])).collect();
    let k_knots: Vec<(f64, f64)> = rows.iter().map(|r| (r[0] * 1_000.0, r[2])).collect();
    let n = Curve::new(&n_knots, Interpolation::LinearInLogWavelength);
    let k = Curve::new(&k_knots, Interpolation::LogLog);
    let cold = absorption_knots(grundy, data.text(grundy)?)?;
    let (lo, hi) = (cold[0].0, cold[cold.len() - 1].0);
    inside_the_joins(grundy, lo, hi)?;
    Ok(vec![
        Piece::new(JOIN_NM, lo, n.clone(), k.clone()),
        Piece::new(lo, hi, n.clone(), Curve::new(&cold, Interpolation::Linear)),
        Piece::new(hi, REACH_END_NM, n, k),
    ])
}

/// `CarbonMonoxideIce`'s pieces: k = 0 with the file's n `file_n`, then Gerakines et al. 2023,
/// their k below [`CO_NOISE_FLOOR`] taken as 0.
fn carbon_monoxide_pieces(
    file_n: Curve,
    data: &InfraredData,
) -> Result<Vec<Piece>, FitAlbedoError> {
    let zero = || Curve::constant(0.0);
    let gerakines = "gerakines_2023_co";
    let mut rows = table_rows(gerakines, data.text(gerakines)?, "wavenumber_per_cm,n,k")?;
    if rows[0][0] <= 0.0 {
        return Err(malformed(gerakines, "a wavenumber is not positive"));
    }
    rows.reverse();
    let at = |column: usize| -> Vec<(f64, f64)> {
        rows.iter().map(|r| (1.0e7 / r[0], r[column])).collect()
    };
    let n = at(1);
    let k: Vec<(f64, f64)> = at(2)
        .into_iter()
        .map(|(nm, k)| (nm, if k < CO_NOISE_FLOOR { 0.0 } else { k }))
        .collect();
    let (lo, last) = (n[0].0, n[n.len() - 1].0);
    if last < REACH_END_NM {
        return Err(malformed(gerakines, "its rows do not reach 5,000 nm"));
    }
    if lo <= JOIN_NM || lo >= REACH_END_NM {
        return Err(malformed(
            gerakines,
            "its first row does not lie between 780 and 5,000 nm",
        ));
    }
    Ok(vec![
        Piece::new(JOIN_NM, lo, file_n, zero()),
        Piece::new(
            lo,
            REACH_END_NM,
            Curve::new(&n, Interpolation::LinearInWavenumber),
            Curve::new(&k, Interpolation::LinearInWavenumber),
        ),
    ])
}

/// `AmmoniaIce`'s pieces: Martonchik et al. 1984's n throughout, k = 0 to their measured infrared.
fn ammonia_pieces(data: &InfraredData) -> Result<Vec<Piece>, FitAlbedoError> {
    let zero = || Curve::constant(0.0);
    let martonchik = "martonchik_1984_nh3";
    let rows = table_rows(martonchik, data.text(martonchik)?, "wavelength_um,n,k")?;
    let n_knots = knots(&rows, 1);
    if n_knots.is_empty() {
        return Err(malformed(martonchik, "no row gives n"));
    }
    let n = Curve::new(&n_knots, Interpolation::Linear);
    // Half a nanometre's margin, for the rows' micrometres in binary.
    let measured: Vec<(f64, f64)> = knots(&rows, 2)
        .into_iter()
        .filter(|&(nm, _)| nm >= AMMONIA_MEASURED_FROM_NM - 0.5)
        .collect();
    let from = measured
        .first()
        .map(|f| f.0)
        .filter(|nm| (nm - AMMONIA_MEASURED_FROM_NM).abs() < 0.5)
        .ok_or_else(|| malformed(martonchik, "it has no row at 1,462 nm"))?;
    Ok(vec![
        Piece::new(JOIN_NM, from, n.clone(), zero()),
        Piece::new(
            from,
            REACH_END_NM,
            n,
            Curve::new(&measured, Interpolation::LogLinear),
        ),
    ])
}

/// The spectral index of a row of `optics` whose R08 file, `file`, is read: the file's piece to
/// [`JOIN_NM`], then its infrared set's from `data`, if any.
///
/// # Errors
///
/// As [`visible_piece`] and [`infrared_pieces`].
pub(crate) fn spectral_index(
    optics: &CondensedOptics,
    file_name: &'static str,
    file: &MaterialFile,
    data: &InfraredData,
) -> Result<SpectralIndex, FitAlbedoError> {
    let mut pieces = vec![visible_piece(file_name, file)?];
    if let Some(set) = optics.infrared_source() {
        pieces.extend(infrared_pieces(set, file, data)?);
    }
    Ok(SpectralIndex::new(pieces))
}

#[cfg(test)]
mod tests {
    use super::super::tests::manifest;
    use super::*;

    /// The infrared error of `dataset` for `why`.
    fn refusal(dataset: &'static str, why: &str) -> FitAlbedoError {
        FitAlbedoError::Infrared {
            dataset,
            why: why.to_owned(),
        }
    }

    /// The datasets `files`, each (dataset, its one file's text).
    fn data(files: &[(&str, &str)]) -> InfraredData {
        InfraredData {
            texts: files
                .iter()
                .map(|&(dataset, text)| {
                    (
                        dataset.to_owned(),
                        BTreeMap::from([("table".to_owned(), text.to_owned())]),
                    )
                })
                .collect(),
        }
    }

    /// The pieces of `set` from `files`, for the row of R08's file `file_name`.
    fn pieces(
        set: InfraredIndex,
        file_name: &str,
        files: &[(&str, &str)],
    ) -> Result<Vec<Piece>, FitAlbedoError> {
        let file = manifest().load_material(file_name).unwrap();
        infrared_pieces(set, &file, &data(files))
    }

    /// A refractiveindex.info table with rows `rows`, its block indented as the database's are.
    fn refractiveindex(rows: &str) -> String {
        format!("DATA:\n  - type: tabulated nk\n    data: |\n{rows}")
    }

    /// A refractiveindex.info file is refused, by its specific reason, without its `data: |` block,
    /// with a row that is not numbers or not three of them, and with fewer than two rows or rows
    /// that do not rise; a well-formed one reads.
    #[test]
    fn substance_albedos_a_malformed_refractiveindex_file_is_refused() {
        let good = refractiveindex("        0.5 1.3 0.0\n        0.6 1.3 0.0\n");
        assert_eq!(refractiveindex_rows("x", &good).unwrap().len(), 2);
        let not_rising = "its rows are fewer than two or do not rise";
        for (text, why) in [
            (
                "DATA:\n  - type: tabulated nk\n".to_owned(),
                "it has no `data: |` block",
            ),
            (
                refractiveindex("        0.5 1.3 zero\n        0.6 1.3 0.0\n"),
                "a row is not numbers",
            ),
            (
                refractiveindex("        0.5 1.3\n        0.6 1.3 0.0\n"),
                "a row is not λ, n and k",
            ),
            (
                refractiveindex("        0.6 1.3 0.0\n        0.5 1.3 0.0\n"),
                not_rising,
            ),
            (refractiveindex("        0.6 1.3 0.0\n"), not_rising),
        ] {
            assert_eq!(
                refractiveindex_rows("x", &text),
                Err(refusal("x", why)),
                "{text}"
            );
        }
    }

    /// A reduced table is refused, by its specific reason, under another header, with a row that is
    /// not numbers, not its header's columns or of no first value, and with rows that do not rise;
    /// the reader's rows rise strictly, so equal wavelengths are refused too.
    #[test]
    fn substance_albedos_a_malformed_reduced_table_is_refused() {
        let header = "wavelength_um,n,k";
        let good = "wavelength_um,n,k\n0.5,1.3,NaN\n0.6,1.3,0\n";
        assert_eq!(table_rows("x", good, header).unwrap().len(), 2);
        let not_rising = "its rows are fewer than two or do not rise";
        for (text, why) in [
            (
                "wavenumber_per_cm,n,k\n0.5,1.3,0\n0.6,1.3,0\n",
                "its header is not `wavelength_um,n,k`",
            ),
            (
                "wavelength_um,n,k\n0.5,1.3,x\n0.6,1.3,0\n",
                "a row is not numbers",
            ),
            (
                "wavelength_um,n,k\n0.5,1.3\n0.6,1.3,0\n",
                "a row is not its header's columns",
            ),
            (
                "wavelength_um,n,k\nNaN,1.3,0\n0.6,1.3,0\n",
                "a row is not its header's columns",
            ),
            ("wavelength_um,n,k\n0.6,1.3,0\n0.5,1.3,0\n", not_rising),
            ("wavelength_um,n,k\n0.6,1.3,0\n0.6,1.3,0\n", not_rising),
            ("wavelength_um,n,k\n0.6,1.3,0\n", not_rising),
        ] {
            assert_eq!(
                table_rows("x", text, header),
                Err(refusal("x", why)),
                "{text}"
            );
        }
        assert_eq!(rising::<[f64; 1]>("x", &[]), Err(refusal("x", not_rising)));
        assert_eq!(rising("x", &[[1.0], [2.0]]), Ok(()));
    }

    /// A table of Grundy's α by λ in µm, of rows `rows`.
    fn alpha(rows: &str) -> String {
        format!("wavelength_um,alpha_per_cm\n{rows}")
    }

    /// `CO`'s set is refused, with its dataset and reason, never a panic, unless its rows reach
    /// 5,000 nm, begin beyond 780 nm and run on positive wavenumbers.
    #[test]
    fn substance_albedos_carbon_monoxide_s_set_off_its_joins_is_refused() {
        let co = "gerakines_2023_co";
        for (text, why) in [
            (
                "wavenumber_per_cm,n,k\n2100,1.3,0\n2900,1.3,0\n",
                "its rows do not reach 5,000 nm",
            ),
            (
                "wavenumber_per_cm,n,k\n1900,1.3,0\n13000,1.3,0\n",
                "its first row does not lie between 780 and 5,000 nm",
            ),
            (
                "wavenumber_per_cm,n,k\n0,1.3,0\n2900,1.3,0\n",
                "a wavenumber is not positive",
            ),
        ] {
            assert_eq!(
                pieces(
                    InfraredIndex::CarbonMonoxideIce,
                    "carbon-monoxide-ice.json",
                    &[(co, text)]
                ),
                Err(refusal(co, why)),
                "{text}"
            );
        }
    }

    /// `NH3`'s set is refused unless it has a row at 1,462 nm and a row that gives n.
    #[test]
    fn substance_albedos_ammonia_s_set_without_its_rows_is_refused() {
        let nh3 = "martonchik_1984_nh3";
        for (text, why) in [
            (
                "wavelength_um,n,k\n0.668,1.4,NaN\n1.5,1.4,0.001\n5.1,1.4,0.01\n",
                "it has no row at 1,462 nm",
            ),
            (
                "wavelength_um,n,k\n0.668,NaN,0\n1.462,NaN,0.001\n",
                "no row gives n",
            ),
        ] {
            assert_eq!(
                pieces(
                    InfraredIndex::AmmoniaIce,
                    "ammonia-ice.json",
                    &[(nh3, text)]
                ),
                Err(refusal(nh3, why)),
                "{text}"
            );
        }
    }

    /// `CH4`'s set is refused unless its n has a row over 400–2,381 nm and its k runs past
    /// 780 nm.
    #[test]
    fn substance_albedos_methane_s_set_without_its_rows_is_refused() {
        let (ch4_k, ch4_n) = ("grundy_2002_ch4", "martonchik_orton_1994_ch4");
        let methane_n = refractiveindex("        1.0 1.31 0.0\n        2.0 1.31 0.0\n");
        let far_n = refractiveindex("        3.0 1.31 0.0\n        4.0 1.31 0.0\n");
        for (k, n, dataset, why) in [
            (
                alpha("1.0,1\n4.0,1\n"),
                &far_n,
                ch4_n,
                "it has no row over 400-2,381 nm, where its n is read",
            ),
            (
                alpha("0.5,1\n0.7,1\n"),
                &methane_n,
                ch4_k,
                "its rows end at or before 780 nm",
            ),
        ] {
            assert_eq!(
                pieces(
                    InfraredIndex::MethaneIce,
                    "methane-ice.json",
                    &[(ch4_k, k.as_str()), (ch4_n, n.as_str())]
                ),
                Err(refusal(dataset, why))
            );
        }
    }

    /// `N2`'s and `H2O`'s sets are refused unless their bands lie strictly between 780 and 5,000
    /// nm, where the pieces before and after them are not empty.
    #[test]
    fn substance_albedos_a_band_reaching_a_join_is_refused() {
        let outside = "its band does not lie between 780 and 5,000 nm";
        for band in [alpha("0.7,1\n2.1,1\n"), alpha("2.0,1\n5.0,1\n")] {
            let n2 = "grundy_1993_n2";
            assert_eq!(
                pieces(
                    InfraredIndex::NitrogenIce,
                    "nitrogen-ice.json",
                    &[(n2, band.as_str())]
                ),
                Err(refusal(n2, outside)),
                "{band}"
            );
            let (warren, grundy) = ("warren_brandt_2008_h2o", "grundy_schmitt_1998_h2o");
            let ice = refractiveindex("        0.7 1.31 1e-8\n        5.1 1.31 1e-2\n");
            assert_eq!(
                pieces(
                    InfraredIndex::WaterIce,
                    "water-ice.json",
                    &[(warren, ice.as_str()), (grundy, band.as_str())]
                ),
                Err(refusal(grundy, outside)),
                "{band}"
            );
        }
    }

    /// A set of one table is refused unless its rows span 780–5,000 nm, and a set missing from
    /// the data is refused by name.
    #[test]
    fn substance_albedos_a_one_table_set_short_of_its_span_is_refused() {
        let tholin = "khare_1984_tholin";
        assert_eq!(
            pieces(
                InfraredIndex::Tholin,
                "tholin.json",
                &[(tholin, "wavelength_um,n,k\n0.7,1.6,0.01\n4.9,1.6,0.01\n")]
            ),
            Err(refusal(tholin, "its rows do not span 780-5,000 nm"))
        );
        assert_eq!(
            pieces(InfraredIndex::Tholin, "tholin.json", &[]),
            Err(refusal(tholin, "it is not one of the manifest's datasets"))
        );
    }
}
