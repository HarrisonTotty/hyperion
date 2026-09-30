//! `run wd_cooling`: the cooling of white dwarfs from 0.2 to 1.3 M☉ (plan 06, P06.T20.a, ruling
//! 57.2 of 2026-09-22), which the sim's `stellar::remnant::cooling` reads under the generator's
//! default recipe.
//!
//! # Why a table
//!
//! P06.T20.a first built the two-piece modified Mestel law of Hurley and Shara (2003, ApJ 589,
//! 179). Against the 0.6 M☉ sequence of Bédard et al. (2020) its effective temperature runs 13–20%
//! cool from 0.05 to 1 Gyr and 11–17% cool from 5 to 10 Gyr, against the plan's 10%: the law misses
//! the slow early cooling of models with true neutrino losses and envelopes, and crystallisation's
//! latent heat and phase separation, which hold an old dwarf warm. So the cooling comes from the
//! detailed sequences themselves, fitted here to a table.
//!
//! # The sequences
//!
//! The evolutionary sequences of Bédard, Bergeron, Brassard and Fontaine (2020, ApJ 901, 93), as
//! the Montreal group distributes them at <https://www.astro.umontreal.ca/~bergeron/CoolingModels/>:
//! the 23 thick-hydrogen sequences (`q_H` = 10⁻⁴, `q_He` = 10⁻², the DA white dwarfs) from 0.2 to
//! 1.3 M☉ in steps of 0.05 M☉, each a homogeneous core of equal masses of carbon and oxygen, from
//! the start of each model's cooling to about 1,500 K.
//!
//! **The sequences are not committed.** The site states no licence; it asks users of its tables to
//! acknowledge the site and cite the papers, which the fitted table's header does. They are a
//! *fetched* dataset (plan 15, Design note 12): only `data/montreal_cooling/PROVENANCE.toml` is
//! committed, with each file's SHA-256, and the raw files are read from the git-ignored cache
//! ([`DEFAULT_DATA_DIR`], or `--data`), downloaded from [`DOWNLOAD_URL`] (the files [`file_name`]
//! names). The table's notes also record a digest of them ([`WdCoolingTable::digest`]).
//! `tests/wd_cooling.rs` renders the fit again when the files are present.
//!
//! # The fit
//!
//! The table's mass nodes are the 23 sequences. For each, it holds log₁₀ of the cooling age plus
//! [`CLOCK_OFFSET_YR`] (years) at [`LUMINOSITIES`] luminosities evenly spaced in log₁₀ L over
//! [`LOG_LUMINOSITY_RANGE`]: the age as a function of the luminosity, which is how cooling tables
//! are read, and which stays gentle where the luminosity plunges, as a crystallised dwarf's does in
//! the Debye regime. The node values are the least-squares solution for linear interpolation in
//! log₁₀ L through every model of the sequence except those held out, plus a penalty of weight
//! [`SMOOTHING`] on the second divided differences, which carries each column as a straight line
//! past the brighter end of its sequence; past the fainter end the column follows Mestel's
//! L ∝ t^−1.4 ([`FAINT_EXPONENT`]). A model is held out when its number is a multiple of
//! [`HOLD_OUT_EVERY`], so that the table can be checked against points it was not fitted to. The
//! normal equations are solved by Cholesky factorisation in a fixed order, so the table comes out
//! the same on every platform. The residuals, over the fitted and the held-out models apart, are
//! written into the table's header.
//!
//! The offset makes the clock finite at the sequence's first model, and is HPT's 0.1 Myr (Hurley,
//! Pols and Tout 2000, MNRAS 315, 543, section 6.2.1).

use std::fmt::{self, Write as _};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::{error::Error, fs, io};

use hyperion_sim::math;
use hyperion_sim::units::consts::{SOLAR_LUMINOSITY_W, SOLAR_RADIUS_M};

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput};

/// The task's version, written into the table's header. A change to anything below that moves
/// the table bumps it.
pub const VERSION: u32 = 0;

/// The number of sequences, and of the table's mass nodes.
pub const SEQUENCES: usize = 23;

/// The first sequence's mass in hundredths of a solar mass.
const FIRST_MASS_CENTI: u32 = 20;

/// The spacing of the sequences' masses in hundredths of a solar mass.
const MASS_STEP_CENTI: u32 = 5;

/// The number of luminosity nodes.
pub const LUMINOSITIES: usize = 96;

/// The luminosity nodes' range, log₁₀ L☉: brighter than every sequence's first model (log L =
/// 2.03 at most, at 0.75 M☉) and fainter than every last (−6.95 at 1.25 M☉).
pub const LOG_LUMINOSITY_RANGE: (f64, f64) = (-7.5, 2.5);

/// The offset of the clock the table holds, years: log₁₀(t + 10⁵ yr).
pub const CLOCK_OFFSET_YR: f64 = 1.0e5;

/// The weight of the second-difference penalty against the models' squared residuals in dex.
pub const SMOOTHING: f64 = 1.0e-6;

/// The exponent of the power law L ∝ t^−1.4 that carries each column past its sequence's
/// faintest model: Mestel cooling's, as HPT's equation 90 has it.
///
/// The sequences end at 1,460–1,630 K, and there the heavy dwarfs are in the Debye regime,
/// fading so fast that the last models' slope, carried on, would put a 1.3 M☉ dwarf 46 dex
/// fainter at 10 Gyr than at the end of its sequence, 5.6 Gyr. Nothing models that; the table
/// instead fades as Mestel's law does, which leaves a 1.3 M☉ dwarf at about 10⁻⁷ L☉ and
/// 1,300 K at 10 Gyr. A dwarf in the Debye regime really fades faster than Mestel's law, so past
/// the sequences this is an upper bound on the luminosity.
pub const FAINT_EXPONENT: f64 = 1.4;

/// The weight of the rows that hold the slope past the faintest model to [`FAINT_EXPONENT`]'s.
const FAINT_WEIGHT: f64 = 1.0;

/// A model whose number is a multiple of this is held out of the fit.
pub const HOLD_OUT_EVERY: u32 = 5;

/// Where the Montreal group publishes the sequences' files.
pub const DOWNLOAD_URL: &str =
    "https://www.astro.umontreal.ca/~bergeron/CoolingModels/CoolingModels/";

/// Where `run wd_cooling` reads the sequences unless `--data` says otherwise: the fetched
/// dataset's cache, `crates/hyperion-fit/data/cache/montreal_cooling/`, which git ignores (plan
/// 15, Design note 12).
pub const DEFAULT_DATA_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/data/cache/montreal_cooling");

/// The Montreal sequences' unit of luminosity, erg s⁻¹, in the IAU 2015 nominal solar luminosity.
const ERG_PER_S_PER_SOLAR_LUMINOSITY: f64 = SOLAR_LUMINOSITY_W * 1.0e7;

/// Centimetres in the IAU 2015 nominal solar radius.
const CM_PER_SOLAR_RADIUS: f64 = SOLAR_RADIUS_M * 100.0;

/// The mass of sequence `i`, M☉.
#[must_use]
pub fn sequence_mass(i: usize) -> f64 {
    f64::from(sequence_centi(i)) / 100.0
}

/// The masses of the sequences, M☉, ascending.
#[must_use]
pub fn sequence_masses() -> [f64; SEQUENCES] {
    std::array::from_fn(sequence_mass)
}

/// Sequence `i`'s mass in hundredths of a solar mass.
#[must_use]
fn sequence_centi(i: usize) -> u32 {
    FIRST_MASS_CENTI + MASS_STEP_CENTI * u32::try_from(i).expect("a sequence index fits in u32")
}

/// The file of sequence `i`, as the Montreal group names it: `seq_060_thick.txt` for 0.6 M☉.
#[must_use]
pub fn file_name(i: usize) -> String {
    format!("seq_{:03}_thick.txt", sequence_centi(i))
}

/// A small index as a float.
#[must_use]
fn index(i: usize) -> f64 {
    f64::from(u32::try_from(i).expect("a table index fits in u32"))
}

/// The luminosity nodes, log₁₀ L☉, evenly spaced and both ends exactly included.
#[must_use]
pub fn luminosity_nodes() -> [f64; LUMINOSITIES] {
    let (lo, hi) = LOG_LUMINOSITY_RANGE;
    let step = (hi - lo) / index(LUMINOSITIES - 1);
    let mut nodes: [f64; LUMINOSITIES] = std::array::from_fn(|i| lo + step * index(i));
    nodes[LUMINOSITIES - 1] = hi;
    nodes
}

/// One model of a sequence, in the sim's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoolingModel {
    /// The model's number in its sequence, from 1.
    pub number: u32,
    /// The effective temperature, K.
    pub teff_k: f64,
    /// The radius, nominal solar radii.
    pub radius_rsun: f64,
    /// The cooling age, Julian years.
    pub age_yr: f64,
    /// The photon luminosity, nominal solar luminosities.
    pub luminosity_lsun: f64,
}

impl CoolingModel {
    /// Whether the fit leaves this model out, to check the table against.
    #[must_use]
    pub fn is_held_out(&self) -> bool {
        self.number.is_multiple_of(HOLD_OUT_EVERY)
    }

    /// log₁₀ of the age plus the clock's offset: the quantity the table holds.
    #[must_use]
    pub fn log_clock(&self) -> f64 {
        math::log10(self.age_yr + CLOCK_OFFSET_YR)
    }
}

/// The 23 sequences, lightest first, and a digest of their files' bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequences {
    /// Each sequence's models, in the file's order.
    pub models: Vec<Vec<CoolingModel>>,
    /// FNV-1a (64-bit) over the files' bytes, in order of mass.
    pub digest: u64,
}

/// The sequences could not be read.
#[derive(Debug)]
pub enum ReadSequencesError {
    /// A file could not be read.
    Read {
        /// The file.
        path: PathBuf,
        /// Why it failed.
        source: io::Error,
    },
    /// A file is not in the Montreal format.
    Malformed {
        /// The file.
        path: PathBuf,
        /// The line, from 1.
        line: usize,
        /// What was wrong.
        why: &'static str,
    },
}

impl fmt::Display for ReadSequencesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(
                f,
                "cannot read {} (download the thick-hydrogen sequences from {DOWNLOAD_URL})",
                path.display()
            ),
            Self::Malformed { path, line, why } => {
                write!(f, "{} line {line}: {why}", path.display())
            }
        }
    }
}

impl Error for ReadSequencesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Malformed { .. } => None,
        }
    }
}

/// FNV-1a, 64-bit, continued from `hash` over `bytes`.
#[must_use]
fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// FNV-1a's offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// Reads the 23 thick-hydrogen sequences from `dir`.
///
/// # Errors
///
/// [`ReadSequencesError::Read`] if a file is missing or unreadable, and
/// [`ReadSequencesError::Malformed`] if one is not in the Montreal format.
pub fn read(dir: &Path) -> Result<Sequences, ReadSequencesError> {
    let mut models = Vec::with_capacity(SEQUENCES);
    let mut digest = FNV_OFFSET;
    for i in 0..SEQUENCES {
        let path = dir.join(file_name(i));
        let bytes = fs::read(&path).map_err(|source| ReadSequencesError::Read {
            path: path.clone(),
            source,
        })?;
        digest = fnv1a(digest, &bytes);
        let text = String::from_utf8_lossy(&bytes);
        let parsed = parse(&text).map_err(|(line, why)| ReadSequencesError::Malformed {
            path: path.clone(),
            line,
            why,
        })?;
        models.push(parsed);
    }
    Ok(Sequences { models, digest })
}

/// Parses one sequence's text: a header between two rules of `=`, then three lines per model,
/// the first holding its number, `T_eff` (K), log g, R (cm), the age (yr) and L (erg s⁻¹).
///
/// # Errors
///
/// The line (from 1) and what was wrong with it.
pub fn parse(text: &str) -> Result<Vec<CoolingModel>, (usize, &'static str)> {
    let mut lines = text.lines().enumerate();
    let mut rules = 0;
    for (_, line) in lines.by_ref() {
        if line.starts_with('=') {
            rules += 1;
            if rules == 2 {
                break;
            }
        }
    }
    if rules < 2 {
        return Err((text.lines().count(), "no header between two rules of `=`"));
    }
    let mut models = Vec::new();
    let mut body = lines.filter(|(_, line)| !line.trim().is_empty());
    while let Some((n, first)) = body.next() {
        let fields: Vec<&str> = first.split_whitespace().collect();
        let [number, teff, _log_g, radius, age, luminosity] = fields[..] else {
            return Err((n + 1, "a model's first line has six columns"));
        };
        let number: u32 = number
            .parse()
            .map_err(|_| (n + 1, "a model's number is an integer"))?;
        let float = |field: &str| {
            field
                .parse::<f64>()
                .map_err(|_| (n + 1, "a model's quantities are numbers"))
        };
        let model = CoolingModel {
            number,
            teff_k: float(teff)?,
            radius_rsun: float(radius)? / CM_PER_SOLAR_RADIUS,
            age_yr: float(age)?,
            luminosity_lsun: float(luminosity)? / ERG_PER_S_PER_SOLAR_LUMINOSITY,
        };
        if !(model.luminosity_lsun > 0.0 && model.age_yr >= 0.0) {
            return Err((
                n + 1,
                "a model has a positive luminosity and an age from zero",
            ));
        }
        for _ in 0..2 {
            if body.next().is_none() {
                return Err((n + 1, "a model has three lines"));
            }
        }
        models.push(model);
    }
    if models.is_empty() {
        return Err((text.lines().count(), "a sequence has models"));
    }
    Ok(models)
}

/// The largest residual over a set of models and where it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Worst {
    /// The residual in log₁₀ L at the model's age, fit minus model, dex.
    pub residual: f64,
    /// The sequence's mass, M☉.
    pub mass: f64,
    /// The model's age, Julian years.
    pub age_yr: f64,
}

/// How well the table reproduces one set of models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Residuals {
    /// The number of models.
    pub rows: u32,
    /// The root mean square of the residual in log₁₀ L, dex.
    pub rms: f64,
    /// The largest residual.
    pub worst: Worst,
}

/// The fitted table.
#[derive(Debug, Clone, PartialEq)]
pub struct WdCoolingTable {
    /// The mass nodes, M☉: the sequences' masses.
    pub masses: [f64; SEQUENCES],
    /// The luminosity nodes, log₁₀ L☉, ascending.
    pub log_luminosity: [f64; LUMINOSITIES],
    /// log₁₀ of the cooling age plus [`CLOCK_OFFSET_YR`], years, by mass node, then luminosity
    /// node; falling along each row.
    pub log_clock: Vec<[f64; LUMINOSITIES]>,
    /// The residuals over the models the fit read.
    pub fitted: Residuals,
    /// The residuals over the models it held out.
    pub held_out: Residuals,
    /// The input's digest ([`Sequences::digest`]).
    pub digest: u64,
}

/// The interval of `nodes` that holds `value` and the fraction of the way along it; a value
/// outside the nodes takes the first or the last interval, and a fraction outside [0, 1].
#[must_use]
fn locate(nodes: &[f64], value: f64) -> (usize, f64) {
    let n = nodes.len();
    let i = nodes[1..n - 1]
        .iter()
        .take_while(|&&node| value >= node)
        .count();
    (i, (value - nodes[i]) / (nodes[i + 1] - nodes[i]))
}

/// The luminosity, log₁₀ L☉, at which the falling `column` reaches `log_clock`, by linear
/// interpolation between the nodes that bracket it and extrapolation past the ends: the inverse
/// the sim evaluates.
#[must_use]
pub fn log_luminosity_at(
    nodes: &[f64; LUMINOSITIES],
    column: &[f64; LUMINOSITIES],
    log_clock: f64,
) -> f64 {
    let j = column[1..LUMINOSITIES - 1]
        .iter()
        .take_while(|&&c| log_clock <= c)
        .count();
    let f = (log_clock - column[j]) / (column[j + 1] - column[j]);
    nodes[j] + f * (nodes[j + 1] - nodes[j])
}

/// The dot product of two slices of equal length, summed in order.
#[must_use]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |sum, (x, y)| sum + x * y)
}

/// Solves the symmetric positive-definite `matrix` against `rhs` by Cholesky factorisation, in
/// place and in a fixed order.
#[must_use]
fn solve(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Vec<f64> {
    let n = rhs.len();
    for j in 0..n {
        let row_j = &mut matrix[j];
        let pivot = row_j[j] - dot(&row_j[..j], &row_j[..j]);
        assert!(pivot > 0.0, "the normal equations are positive definite");
        row_j[j] = pivot.sqrt();
        let (upper, lower) = matrix.split_at_mut(j + 1);
        let row_j = &upper[j];
        for row_i in lower {
            row_i[j] = (row_i[j] - dot(&row_i[..j], &row_j[..j])) / row_j[j];
        }
    }
    for i in 0..n {
        rhs[i] = (rhs[i] - dot(&matrix[i][..i], &rhs[..i])) / matrix[i][i];
    }
    for i in (0..n).rev() {
        let mut sum = rhs[i];
        for (k, row) in matrix.iter().enumerate().skip(i + 1) {
            sum -= row[i] * rhs[k];
        }
        rhs[i] = sum / matrix[i][i];
    }
    rhs
}

/// Adds the row `Σ coefficient × unknown ≈ value` to the normal equations.
fn add_row(matrix: &mut [Vec<f64>], rhs: &mut [f64], row: &[(usize, f64)], value: f64) {
    for &(a, ca) in row {
        for &(b, cb) in row {
            matrix[a][b] += ca * cb;
        }
        rhs[a] += ca * value;
    }
}

/// Fits one sequence's column: log₁₀ of the clock at each luminosity node.
#[must_use]
fn fit_column(nodes: &[f64; LUMINOSITIES], models: &[CoolingModel]) -> [f64; LUMINOSITIES] {
    let mut matrix = vec![vec![0.0; LUMINOSITIES]; LUMINOSITIES];
    let mut rhs = vec![0.0; LUMINOSITIES];
    for model in models.iter().filter(|m| !m.is_held_out()) {
        let (j, f) = locate(nodes, math::log10(model.luminosity_lsun));
        add_row(
            &mut matrix,
            &mut rhs,
            &[(j, 1.0 - f), (j + 1, f)],
            model.log_clock(),
        );
    }
    let faintest = models
        .iter()
        .filter(|m| !m.is_held_out())
        .map(|m| math::log10(m.luminosity_lsun))
        .fold(f64::INFINITY, f64::min);
    for j in 0..LUMINOSITIES - 1 {
        if nodes[j + 1] < faintest {
            let h = nodes[j + 1] - nodes[j];
            add_row(
                &mut matrix,
                &mut rhs,
                &[(j, -FAINT_WEIGHT / h), (j + 1, FAINT_WEIGHT / h)],
                -FAINT_WEIGHT / FAINT_EXPONENT,
            );
        }
    }
    let weight = SMOOTHING.sqrt();
    for j in 1..LUMINOSITIES - 1 {
        let (h1, h2) = (nodes[j] - nodes[j - 1], nodes[j + 1] - nodes[j]);
        let row = [
            (j - 1, weight / h1),
            (j, -weight * (1.0 / h1 + 1.0 / h2)),
            (j + 1, weight / h2),
        ];
        add_row(&mut matrix, &mut rhs, &row, 0.0);
    }
    let solution = solve(matrix, rhs);
    std::array::from_fn(|j| solution[j])
}

/// Fits the table. Single-threaded, with no randomness, so the result is the same on every run
/// and platform.
///
/// # Panics
///
/// If `sequences` does not hold [`SEQUENCES`] sequences, or if a fitted column does not fall
/// strictly with luminosity, which the sim's inversion needs; neither happens with the Montreal
/// files.
#[must_use]
pub fn fit(sequences: &Sequences) -> WdCoolingTable {
    assert_eq!(sequences.models.len(), SEQUENCES, "one sequence per mass");
    let nodes = luminosity_nodes();
    let mut log_clock = Vec::with_capacity(SEQUENCES);
    for models in &sequences.models {
        let column = fit_column(&nodes, models);
        assert!(
            column.windows(2).all(|pair| pair[1] < pair[0]),
            "the fitted clock falls with luminosity along every sequence"
        );
        log_clock.push(column);
    }
    let [fitted, held_out] =
        [false, true].map(|held| residuals(&nodes, &log_clock, sequences, held));
    WdCoolingTable {
        masses: sequence_masses(),
        log_luminosity: nodes,
        log_clock,
        fitted,
        held_out,
        digest: sequences.digest,
    }
}

/// The residuals in log₁₀ L at each model's age, over the models held out or over the rest.
#[must_use]
fn residuals(
    nodes: &[f64; LUMINOSITIES],
    log_clock: &[[f64; LUMINOSITIES]],
    sequences: &Sequences,
    held_out: bool,
) -> Residuals {
    let mut worst = Worst {
        residual: 0.0,
        mass: 0.0,
        age_yr: 0.0,
    };
    let (mut sum, mut rows) = (0.0, 0_u32);
    for (i, (column, models)) in log_clock.iter().zip(&sequences.models).enumerate() {
        for model in models.iter().filter(|m| m.is_held_out() == held_out) {
            let d = log_luminosity_at(nodes, column, model.log_clock())
                - math::log10(model.luminosity_lsun);
            sum += d * d;
            rows += 1;
            if d.abs() > worst.residual.abs() {
                worst = Worst {
                    residual: d,
                    mass: sequence_mass(i),
                    age_yr: model.age_yr,
                };
            }
        }
    }
    Residuals {
        rows,
        rms: (sum / f64::from(rows.max(1))).sqrt(),
        worst,
    }
}

/// `value` rounded to `digits` decimals, for prose.
#[must_use]
fn rounded(value: f64, digits: usize) -> String {
    format!("{value:.digits$}")
}

/// One set of residuals as prose.
#[must_use]
fn describe(r: &Residuals) -> String {
    format!(
        "within {} dex (rms {}), largest at {} M☉, {} Gyr",
        rounded(r.worst.residual.abs(), 3),
        rounded(r.rms, 4),
        rounded(r.worst.mass, 2),
        rounded(r.worst.age_yr * 1e-9, 4),
    )
}

/// The table's contents: its summary, its notes and its items, which are the committed bytes.
#[must_use]
pub fn render(table: &WdCoolingTable) -> RustTable {
    let (lo, hi) = LOG_LUMINOSITY_RANGE;
    let notes = format!(
        "\
The sequences are not redistributed here. Their provenance, with the SHA-256 of each file, is
`crates/hyperion-fit/data/{DATASET}/PROVENANCE.toml`; the FNV-1a digest of all {SEQUENCES} is
{digest:#018x}. Luminosities are converted with the IAU 2015 nominal solar luminosity.

Nodes: the {SEQUENCES} sequences' masses, and {LUMINOSITIES} luminosities evenly spaced in log₁₀ L
from {lo} to {hi}. Each value is log₁₀ of the cooling age plus {offset} yr, the least-squares
solution for linear interpolation in log₁₀ L through the sequence's models, less every model
whose number is a multiple of {HOLD_OUT_EVERY}, plus a penalty of weight {SMOOTHING:e} on the
second divided differences, which carries each column as a straight line past its sequence's
brighter end. Past its fainter end, about 1,500 K, the column falls as Mestel's L ∝ t^−{FAINT_EXPONENT}.

Residuals in log₁₀ L at the models' ages, fit minus model:
- the {fitted_rows} fitted models: {fitted};
- the {held_rows} held-out models: {held}.",
        digest = table.digest,
        offset = CLOCK_OFFSET_YR,
        fitted_rows = table.fitted.rows,
        fitted = describe(&table.fitted),
        held_rows = table.held_out.rows,
        held = describe(&table.held_out),
    );
    let mut out = String::new();
    write!(
        out,
        "/// The clock's offset, years: the table holds log₁₀(t + {offset} yr).\n\
         pub const CLOCK_OFFSET_YR: f64 = {offset_literal};\n\n",
        offset = CLOCK_OFFSET_YR,
        offset_literal = literal(CLOCK_OFFSET_YR),
    )
    .expect("writing to a String");
    out.push_str("/// The mass nodes, M☉: the sequences' masses, ascending.\n");
    writeln!(out, "pub const MASS_NODES: [f64; {SEQUENCES}] = [").expect("writing to a String");
    // rustfmt packs short literals several to a line, up to its width of 100.
    let mut line = String::from("   ");
    for &m in &table.masses {
        let item = format!(" {},", literal(m));
        if line.len() + item.len() > 100 {
            writeln!(out, "{line}").expect("writing to a String");
            line = String::from("   ");
        }
        line.push_str(&item);
    }
    writeln!(out, "{line}").expect("writing to a String");
    out.push_str("];\n\n");
    out.push_str("/// The luminosity nodes: log₁₀ of the luminosity in nominal solar luminosities, ascending.\n");
    writeln!(
        out,
        "pub const LOG_LUMINOSITY_NODES: [f64; {LUMINOSITIES}] = ["
    )
    .expect("writing to a String");
    for &y in &table.log_luminosity {
        writeln!(out, "    {},", literal(y)).expect("writing to a String");
    }
    out.push_str("];\n\n");
    out.push_str(
        "/// log₁₀ of the cooling age plus [`CLOCK_OFFSET_YR`], Julian years, by mass node, then\n\
         /// luminosity node; falling along each row.\n",
    );
    writeln!(
        out,
        "pub static LOG_CLOCK: [[f64; {LUMINOSITIES}]; {SEQUENCES}] = ["
    )
    .expect("writing to a String");
    for row in &table.log_clock {
        out.push_str("    [\n");
        for &value in row {
            writeln!(out, "        {},", literal(value)).expect("writing to a String");
        }
        out.push_str("    ],\n");
    }
    out.push_str("];\n");
    RustTable {
        summary: vec![
            "The cooling of white dwarfs from 0.2 to 1.3 M☉, as a table of the cooling age against log"
                .to_owned(),
            "luminosity at each mass (plan 06, P06.T20.a, ruling 57.2), which `stellar::remnant::cooling`"
                .to_owned(),
            "interpolates under the generator's default recipe.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![TableItem::Source(out)],
    }
}

/// The dataset the task reads: the Montreal sequences, fetched.
pub const DATASET: &str = "montreal_cooling";

/// The fit's parameters as its manifest records them.
fn parameters() -> toml::Table {
    let text = format!(
        "sequences = {SEQUENCES}\nluminosities = {LUMINOSITIES}\nlog_luminosity_range = [{:?}, \
         {:?}]\nclock_offset_yr = {CLOCK_OFFSET_YR:?}\nsmoothing = {SMOOTHING:?}\nfaint_exponent \
         = {FAINT_EXPONENT:?}\nhold_out_every = {HOLD_OUT_EVERY}\n",
        LOG_LUMINOSITY_RANGE.0, LOG_LUMINOSITY_RANGE.1,
    );
    toml::from_str(&text).expect("the parameters' own text is TOML")
}

/// The task: plan 06's P06.T20.a, fast, revision [`VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct WdCoolingTask;

impl FitTask for WdCoolingTask {
    fn name(&self) -> &'static str {
        "wd_cooling"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "wd_cooling.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "CLOCK_OFFSET_YR",
            "MASS_NODES",
            "LOG_LUMINOSITY_NODES",
            "LOG_CLOCK",
        ]
    }

    /// The sim's units the sequences are converted into: the nominal solar luminosity and radius.
    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(vec![
            (
                "units::consts::SOLAR_LUMINOSITY_W".to_owned(),
                SOLAR_LUMINOSITY_W,
            ),
            ("units::consts::SOLAR_RADIUS_M".to_owned(), SOLAR_RADIUS_M),
        ])
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        manifest.expect_params(&parameters())?;
        let dataset = manifest.load_dataset(DATASET)?;
        let sequences = read(&dataset.dir).map_err(|e| RunTaskError::Input {
            task: "wd_cooling",
            source: Box::new(e),
        })?;
        let table = fit(&sequences);
        Ok(TaskOutput {
            source: format!(
                "the evolutionary sequences of Bédard, Bergeron, Brassard and Fontaine (2020, ApJ \
                 901, 93), thick hydrogen layers (`q_H` = 10⁻⁴, `q_He` = 10⁻²) on equimassic \
                 carbon–oxygen cores, the {SEQUENCES} files `seq_020_thick.txt` to \
                 `seq_130_thick.txt` of <https://www.astro.umontreal.ca/~bergeron/CoolingModels/>, \
                 retrieved 2026-09-24; with thanks to the Montreal group"
            ),
            acceptance: format!(
                "log₁₀ L at the models' ages within {} dex (rms {}) over the {} fitted models and \
                 {} dex (rms {}) over the {} held out",
                rounded(table.fitted.worst.residual.abs(), 3),
                rounded(table.fitted.rms, 4),
                table.fitted.rows,
                rounded(table.held_out.worst.residual.abs(), 3),
                rounded(table.held_out.rms, 4),
                table.held_out.rows,
            ),
            table: render(&table),
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
============================================================================
  #Mod      Teff         Log(g)           R            Age            L
           Log(Tc)       Log(Pc)      Log(rhoc)       Mx/M         Log(qx)
             Lnu        Log(H/*)      Log(He/*)     Log(C/*)      Log(O/*)
============================================================================
     1    98454.6021    7.39369788  1.793422E+09  0.000000E+00  2.153481E+35
        7.983297E+00  2.299745E+01  6.375422E+00  0.0000000000  0.000000E+00
        1.718811E+35 -3.999873E+00 -1.999887E+00 -3.054398E-01 -3.054398E-01
     2    92964.8982    7.45862270  1.664256E+09  6.000000E+04  1.474172E+35
        7.973750E+00  2.302089E+01  6.391056E+00  0.0000000000  0.000000E+00
        1.578814E+35 -3.999873E+00 -1.999887E+00 -3.054398E-01 -3.054398E-01
";

    /// The first two models of `seq_060_thick.txt`, in the sim's units: 2.153 × 10³⁵ erg s⁻¹ is
    /// 56.26 L☉ and 1.793 × 10⁹ cm is 0.025 78 R☉.
    #[test]
    fn a_sequence_is_parsed_into_the_sims_units() {
        let models = parse(SAMPLE).unwrap();
        assert_eq!(models.len(), 2);
        let first = models[0];
        assert_eq!(first.number, 1);
        assert!((first.luminosity_lsun - 56.256_1).abs() < 1e-3, "{first:?}");
        assert!((first.radius_rsun - 0.025_778).abs() < 1e-5, "{first:?}");
        assert!((first.teff_k - 98_454.602_1).abs() < 1e-9);
        assert!((models[1].age_yr - 6.0e4).abs() < 1e-9);
        assert!((first.log_clock() - 5.0).abs() < 1e-15);
        assert!(!first.is_held_out());
    }

    #[test]
    fn a_malformed_sequence_is_refused() {
        assert_eq!(
            parse("no header\n").unwrap_err().1,
            "no header between two rules of `=`"
        );
        let short = SAMPLE.lines().take(7).collect::<Vec<_>>().join("\n");
        assert_eq!(parse(&short).unwrap_err().1, "a model has three lines");
        let narrow = SAMPLE.replace("  2.153481E+35", "");
        assert_eq!(
            parse(&narrow).unwrap_err(),
            (6, "a model's first line has six columns")
        );
        let bare = SAMPLE.lines().take(5).collect::<Vec<_>>().join("\n");
        assert_eq!(parse(&bare).unwrap_err().1, "a sequence has models");
    }

    #[test]
    fn a_missing_directory_is_a_read_error_naming_the_file() {
        let dir = Path::new("/nonexistent-directory/for/hyperion-fit");
        match read(dir) {
            Err(error @ ReadSequencesError::Read { .. }) => {
                assert!(error.to_string().contains("seq_020_thick.txt"), "{error}");
                assert!(error.source().is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_nodes_and_masses_are_the_sequences_and_an_even_grid() {
        let masses = sequence_masses();
        assert!((masses[0] - 0.2).abs() < 1e-15 && (masses[SEQUENCES - 1] - 1.3).abs() < 1e-15);
        assert_eq!(file_name(8), "seq_060_thick.txt");
        let nodes = luminosity_nodes();
        assert!(nodes[0].total_cmp(&-7.5).is_eq());
        assert!(nodes[LUMINOSITIES - 1].total_cmp(&2.5).is_eq());
        for pair in nodes.windows(2) {
            assert!((pair[1] - pair[0] - 10.0 / 95.0).abs() < 1e-12);
        }
    }

    /// A falling line in (log L, log clock) of Mestel's slope is reproduced exactly, since
    /// neither the penalty nor the faint end's slope rows move it, and the inverse reads it back
    /// at, between and beyond the nodes.
    #[test]
    fn a_line_is_fitted_exactly_and_inverted() {
        let nodes = luminosity_nodes();
        let line = |y: f64| 7.0 - y / FAINT_EXPONENT;
        let models: Vec<CoolingModel> = (1..400)
            .map(|k| {
                let y = -6.0 + 7.0 * f64::from(k) / 400.0;
                CoolingModel {
                    number: k,
                    teff_k: 1.0,
                    radius_rsun: 1.0,
                    age_yr: math::exp10(line(y)) - CLOCK_OFFSET_YR,
                    luminosity_lsun: math::exp10(y),
                }
            })
            .collect();
        let column = fit_column(&nodes, &models);
        for (j, value) in column.iter().enumerate() {
            assert!((value - line(nodes[j])).abs() < 1e-8, "{j}: {value}");
        }
        for y in [-9.0, -7.5, -3.21, 0.0, 2.5, 4.0] {
            let back = log_luminosity_at(&nodes, &column, line(y));
            assert!((back - y).abs() < 1e-8, "{y}: {back}");
        }
    }

    #[test]
    fn the_digest_is_fnv1a() {
        // The published test vectors: "" and "a".
        assert_eq!(fnv1a(FNV_OFFSET, b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(FNV_OFFSET, b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
