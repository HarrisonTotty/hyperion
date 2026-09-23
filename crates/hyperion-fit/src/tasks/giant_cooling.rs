//! `run giant_cooling`: the cooling of giant planets from 0.3 to 13 Jupiter masses (plan 13,
//! P13.T5.b), which the sim's `stellar::substellar::giant_cooling` interpolates and plan 14 reads.
//!
//! # Why a table
//!
//! The brainstorm cites Burrows, Hubbard, Lunine and Liebert (2001, Rev. Mod. Phys. 73, 719) for
//! this range, and plan 13 asks first whether their power laws, which "characterize older SMOs",
//! will do. They do not. At 1 Jupiter mass and 4.6 Gyr their equation 1 gives L = 1.59 × 10⁻¹⁰ L☉,
//! 13–18% of Jupiter's own internal luminosity (5.4–7.5 W m⁻² over its surface: Hanel et al. 1981;
//! Li et al. 2018); equation 2 gives an effective temperature of 36 K against the plan's 100–160 K
//! (Jupiter's internal flux is 99–107 K); and equation 5, at the gravity of equation 3, a radius of
//! 1.42 Jupiter radii against the plan's 10%. The laws describe the late-time cooling of the whole
//! substellar family but are normalised at 0.05 M☉, and fifty times below that they fail in every
//! quantity. So the cooling comes from a published grid of evolution models, fitted here to a
//! table.
//!
//! # The grid
//!
//! The Sonora Bobcat evolution tables (Marley et al. 2021, ApJ 920, 85), cloudless, at \[M/H\] = 0
//! and solar C/O, hot start: hydrogen–helium objects from 0.0005 to 0.08 M☉ (0.52 to 84 Jupiter
//! masses) and 1 Myr to 15 Gyr, down to effective temperatures of 100 K. They were chosen over the
//! other candidates the plan names for two reasons.
//!
//! - **Their licence allows committing their numbers.** The Zenodo record (doi:10.5281/
//!   zenodo.5063476) is Creative Commons Attribution 4.0. Burrows et al.'s (1997) model files ask
//!   to be told of any use and state no licence, and Baraffe et al.'s (2003) COND tracks sit in a
//!   directory listing with none.
//! - **They reach old, cold planets.** Their tables run down to 100 K, so a Jupiter-mass track
//!   lasts to 6 Gyr and every track from 2 Jupiter masses up to 15 Gyr. Below 200 K and log g = 3
//!   that rests on the models' continuation of their atmosphere grid (Marley et al. 2021, §2.7),
//!   which a Jupiter reaches at about 0.7 Gyr. The Sonora Diamondback
//!   tables (Morley et al. 2024) stop at 200 K, a Jupiter at 0.66 Gyr; ATMO 2020 (Phillips et al.
//!   2020) starts at 0.001 M☉, 1.05 Jupiter masses; and Linder et al. (2019) reach Saturn's mass
//!   but stop at 2 Jupiter masses.
//!
//! No candidate reaches 0.3 Jupiter masses at every age. Bobcat's lightest track is 0.52 Jupiter
//! masses, and it ends at 3 Gyr; the fit carries the table into those corners as straight lines in
//! log–log, which is what old, degenerate objects' cooling laws are (below).
//!
//! The fit reads only Bobcat's tracks up to 0.011 M☉ (11.5 Jupiter masses), committed in
//! [`GRID_FILE`] with their source. The heavier tracks burn deuterium, 0.47 dex brighter than
//! plan 06's cooling fit at 12.6 Jupiter masses and 0.1 Gyr, and neither that fit nor this one
//! models it (plan 06, P06.T13 as built; plan 13's P13.T5.a may add it). Bobcat's solar mass,
//! radius and luminosity (1.989 × 10³³ g, 6.9599 × 10¹⁰ cm and 10^33.5827 erg s⁻¹, from the tables'
//! README) are converted to the IAU 2015 nominal units the sim uses.
//!
//! # The fit
//!
//! The table holds log₁₀ L (L☉) and log₁₀ R (R☉) at [`MASSES`] masses log-spaced over
//! [`MASS_RANGE_MJUP`] and [`AGES`] ages log-spaced over [`AGE_RANGE_YR`], and the sim
//! interpolates it bilinearly in log₁₀ mass and log₁₀ age. The node values are the least-squares
//! solution for that interpolation through every row the fit reads, plus a penalty of weight
//! [`SMOOTHING`] on the second divided differences along each axis. Where there are rows the
//! penalty moves nothing that matters; where there are none, below 0.52 Jupiter masses and past the
//! ends of the lightest tracks, it makes the table a straight line in log–log, continuing the grid
//! as a power law in mass and age. The normal equations are solved by Cholesky factorisation in a
//! fixed order, so the table comes out the same on every platform. The residuals are written into
//! the table's header.

use std::fmt::Write as _;

use hyperion_sim::math;
use hyperion_sim::units::consts::{JUPITER_MASS_KG, SOLAR_LUMINOSITY_W, SOLAR_RADIUS_M};

use super::render::literal;

/// The task's version, written into the table's header. A change to anything below that moves
/// the table bumps it.
pub const VERSION: u32 = 0;

/// The number of mass nodes. Plan 13 allows at most 12.
pub const MASSES: usize = 12;

/// The number of age nodes. Plan 13 allows at most 12.
pub const AGES: usize = 12;

/// The table's masses, in Jupiter masses (IAU 2015 nominal): plan 13's giant-planet range.
pub const MASS_RANGE_MJUP: (f64, f64) = (0.3, 13.0);

/// The table's ages, in Julian years: from 1 Myr, where plan 06's cooling fit starts too, to
/// 15 Gyr, where Bobcat's tracks end.
pub const AGE_RANGE_YR: (f64, f64) = (1.0e6, 1.5e10);

/// The weight of the second-difference penalty against the rows' squared residuals in dex.
pub const SMOOTHING: f64 = 1.0e-4;

/// The committed grid, relative to the repository's root.
pub const GRID_FILE: &str =
    "crates/hyperion-fit/data/giant_cooling/sonora_bobcat_nc+0.0_co1.0_mass.txt";

/// The committed grid's text.
const GRID: &str = include_str!("../../data/giant_cooling/sonora_bobcat_nc+0.0_co1.0_mass.txt");

/// Bobcat's solar mass, 1.989 × 10³³ g, in kilograms.
const BOBCAT_SOLAR_MASS_KG: f64 = 1.989e30;

/// Bobcat's solar radius, 6.9599 × 10¹⁰ cm, in metres.
const BOBCAT_SOLAR_RADIUS_M: f64 = 6.9599e8;

/// log₁₀ of Bobcat's solar luminosity in erg s⁻¹.
const BOBCAT_LOG_SOLAR_LUMINOSITY_ERG_S: f64 = 33.5827;

/// Ergs in one joule.
const ERGS_PER_JOULE: f64 = 1.0e7;

/// Years in one gigayear, Bobcat's unit of age.
const YEARS_PER_GIGAYEAR: f64 = 1.0e9;

/// The number of unknowns, one per node and quantity.
const UNKNOWNS: usize = MASSES * AGES;

/// One row of the grid, in the sim's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridPoint {
    /// log₁₀ of the mass in Jupiter masses.
    pub log_mass: f64,
    /// log₁₀ of the age in Julian years.
    pub log_age: f64,
    /// log₁₀ of the luminosity in nominal solar luminosities.
    pub log_luminosity: f64,
    /// log₁₀ of the radius in nominal solar radii.
    pub log_radius: f64,
}

/// The largest residual of one quantity and where it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Worst {
    /// The residual, fit minus grid: dex for the luminosity, a fraction for the radius.
    pub residual: f64,
    /// The grid row's mass, Jupiter masses.
    pub mass_mjup: f64,
    /// The grid row's age, Julian years.
    pub age_yr: f64,
}

/// How well the table reproduces the grid's rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Residuals {
    /// The number of rows.
    pub rows: u32,
    /// The root mean square of the residual in log₁₀ L, dex.
    pub rms_log_luminosity: f64,
    /// The largest residual in log₁₀ L.
    pub worst_log_luminosity: Worst,
    /// The root mean square of the fractional residual in R.
    pub rms_radius: f64,
    /// The largest fractional residual in R.
    pub worst_radius: Worst,
}

/// The fitted table.
#[derive(Debug, Clone, PartialEq)]
pub struct GiantCoolingTable {
    /// The mass nodes: log₁₀ of the mass in Jupiter masses, ascending.
    pub log_mass: [f64; MASSES],
    /// The age nodes: log₁₀ of the age in Julian years, ascending.
    pub log_age: [f64; AGES],
    /// log₁₀ of the luminosity in L☉, by mass node, then age node.
    pub log_luminosity: [[f64; AGES]; MASSES],
    /// log₁₀ of the radius in R☉, by mass node, then age node.
    pub log_radius: [[f64; AGES]; MASSES],
    /// The residuals against the grid.
    pub residuals: Residuals,
}

/// A small index as a float.
#[must_use]
fn index(i: usize) -> f64 {
    f64::from(u32::try_from(i).expect("a table index fits in u32"))
}

/// `N` points evenly spaced from `lo` to `hi`, both exactly included.
#[must_use]
fn even<const N: usize>(lo: f64, hi: f64) -> [f64; N] {
    let step = (hi - lo) / index(N - 1);
    let mut points = [0.0; N];
    for (i, point) in points.iter_mut().enumerate() {
        *point = lo + step * index(i);
    }
    points[N - 1] = hi;
    points
}

/// The mass nodes, log₁₀ of Jupiter masses.
#[must_use]
pub fn mass_nodes() -> [f64; MASSES] {
    even(
        math::log10(MASS_RANGE_MJUP.0),
        math::log10(MASS_RANGE_MJUP.1),
    )
}

/// The age nodes, log₁₀ of Julian years.
#[must_use]
pub fn age_nodes() -> [f64; AGES] {
    even(math::log10(AGE_RANGE_YR.0), math::log10(AGE_RANGE_YR.1))
}

/// Parses one number of the committed grid.
#[must_use]
fn number(field: &str) -> f64 {
    field
        .parse()
        .expect("the committed grid holds only decimal numbers")
}

/// The committed grid's rows, in the file's order, converted to the sim's units.
///
/// # Panics
///
/// If the committed file is malformed: a block whose count does not match its rows, or a row of
/// other than seven numbers. The file is compiled in, and the crate's tests read it.
#[must_use]
pub fn grid() -> Vec<GridPoint> {
    parse(GRID)
}

/// Parses the grid's text: `#` comments, the column header, then blocks of a row count and that
/// many rows of seven columns.
#[must_use]
fn parse(text: &str) -> Vec<GridPoint> {
    let mass_per_bobcat = BOBCAT_SOLAR_MASS_KG / JUPITER_MASS_KG;
    let radius_per_bobcat = BOBCAT_SOLAR_RADIUS_M / SOLAR_RADIUS_M;
    let luminosity_offset =
        BOBCAT_LOG_SOLAR_LUMINOSITY_ERG_S - math::log10(SOLAR_LUMINOSITY_W * ERGS_PER_JOULE);
    let mut lines = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
        .peekable();
    let mut points = Vec::new();
    while let Some(count) = lines.next() {
        let count: usize = count
            .trim()
            .parse()
            .expect("each block of the committed grid starts with its row count");
        for _ in 0..count {
            let row = lines
                .next()
                .expect("a block of the committed grid holds as many rows as it says");
            let fields: Vec<f64> = row.split_whitespace().map(number).collect();
            let [mass, age_gyr, log_l, _teff, _log_g, radius, _log_i] = fields[..] else {
                panic!("a row of the committed grid has seven columns: {row}");
            };
            points.push(GridPoint {
                log_mass: math::log10(mass * mass_per_bobcat),
                log_age: math::log10(age_gyr * YEARS_PER_GIGAYEAR),
                log_luminosity: log_l + luminosity_offset,
                log_radius: math::log10(radius * radius_per_bobcat),
            });
        }
    }
    points
}

/// The interval of `nodes` that holds `value` and the fraction of the way along it; a value
/// outside the nodes takes the first or the last interval, and a fraction outside [0, 1].
#[must_use]
fn locate<const N: usize>(nodes: &[f64; N], value: f64) -> (usize, f64) {
    let i = nodes[1..N - 1]
        .iter()
        .take_while(|&&node| value >= node)
        .count();
    (i, (value - nodes[i]) / (nodes[i + 1] - nodes[i]))
}

/// The four unknowns of a bilinear interpolation at (`log_mass`, `log_age`) and their weights.
#[must_use]
fn bilinear_weights(
    masses: &[f64; MASSES],
    ages: &[f64; AGES],
    log_mass: f64,
    log_age: f64,
) -> [(usize, f64); 4] {
    let (i, f) = locate(masses, log_mass);
    let (j, g) = locate(ages, log_age);
    [
        (i * AGES + j, (1.0 - f) * (1.0 - g)),
        ((i + 1) * AGES + j, f * (1.0 - g)),
        (i * AGES + j + 1, (1.0 - f) * g),
        ((i + 1) * AGES + j + 1, f * g),
    ]
}

/// The normal equations of the least-squares problem, for both quantities at once: they share
/// their matrix and differ in the right-hand side.
struct NormalEquations {
    matrix: Vec<Vec<f64>>,
    rhs: [Vec<f64>; 2],
}

impl NormalEquations {
    fn new() -> Self {
        Self {
            matrix: vec![vec![0.0; UNKNOWNS]; UNKNOWNS],
            rhs: [vec![0.0; UNKNOWNS], vec![0.0; UNKNOWNS]],
        }
    }

    /// Adds the row `Σ coefficient × unknown ≈ values`, for the two quantities' values.
    fn add(&mut self, row: &[(usize, f64)], values: [f64; 2]) {
        for &(a, ca) in row {
            for &(b, cb) in row {
                self.matrix[a][b] += ca * cb;
            }
            for (rhs, value) in self.rhs.iter_mut().zip(values) {
                rhs[a] += ca * value;
            }
        }
    }

    /// Adds a penalty on the second divided difference of three unknowns spaced `h1` and `h2`.
    fn add_curvature(&mut self, unknowns: [usize; 3], h1: f64, h2: f64) {
        let weight = SMOOTHING.sqrt();
        let row = [
            (unknowns[0], weight / h1),
            (unknowns[1], -weight * (1.0 / h1 + 1.0 / h2)),
            (unknowns[2], weight / h2),
        ];
        self.add(&row, [0.0, 0.0]);
    }

    /// Solves by Cholesky factorisation, in place and in a fixed order: the matrix's lower
    /// triangle becomes the factor L of L Lᵀ, then each right-hand side is solved forwards
    /// through L and backwards through Lᵀ.
    fn solve(mut self) -> [Vec<f64>; 2] {
        let a = &mut self.matrix;
        for j in 0..UNKNOWNS {
            let row_j = &mut a[j];
            let pivot = row_j[j] - dot(&row_j[..j], &row_j[..j]);
            assert!(pivot > 0.0, "the normal equations are positive definite");
            row_j[j] = pivot.sqrt();
            let (upper, lower) = a.split_at_mut(j + 1);
            let row_j = &upper[j];
            for row_i in lower {
                row_i[j] = (row_i[j] - dot(&row_i[..j], &row_j[..j])) / row_j[j];
            }
        }
        let factor = &self.matrix;
        self.rhs.map(|mut b| {
            for i in 0..UNKNOWNS {
                b[i] = (b[i] - dot(&factor[i][..i], &b[..i])) / factor[i][i];
            }
            for i in (0..UNKNOWNS).rev() {
                let mut sum = b[i];
                for (k, row) in factor.iter().enumerate().skip(i + 1) {
                    sum -= row[i] * b[k];
                }
                b[i] = sum / factor[i][i];
            }
            b
        })
    }
}

/// The dot product of two slices of equal length, summed in order.
#[must_use]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |sum, (x, y)| sum + x * y)
}

/// Fits the table. Single-threaded, with no randomness, so the result is the same on every run
/// and platform.
#[must_use]
pub fn fit() -> GiantCoolingTable {
    let masses = mass_nodes();
    let ages = age_nodes();
    let points = grid();
    let mut normal = NormalEquations::new();
    for point in &points {
        let row = bilinear_weights(&masses, &ages, point.log_mass, point.log_age);
        normal.add(&row, [point.log_luminosity, point.log_radius]);
    }
    for i in 0..MASSES {
        for j in 1..AGES - 1 {
            let unknowns = [i * AGES + j - 1, i * AGES + j, i * AGES + j + 1];
            normal.add_curvature(unknowns, ages[j] - ages[j - 1], ages[j + 1] - ages[j]);
        }
    }
    for j in 0..AGES {
        for i in 1..MASSES - 1 {
            let unknowns = [(i - 1) * AGES + j, i * AGES + j, (i + 1) * AGES + j];
            normal.add_curvature(
                unknowns,
                masses[i] - masses[i - 1],
                masses[i + 1] - masses[i],
            );
        }
    }
    let [luminosity, radius] = normal.solve();
    let unflatten = |values: &[f64]| {
        let mut table = [[0.0; AGES]; MASSES];
        for (row, chunk) in table.iter_mut().zip(values.as_chunks::<AGES>().0) {
            *row = *chunk;
        }
        table
    };
    let log_luminosity = unflatten(&luminosity);
    let log_radius = unflatten(&radius);
    let residuals = residuals(&masses, &ages, &log_luminosity, &log_radius, &points);
    GiantCoolingTable {
        log_mass: masses,
        log_age: ages,
        log_luminosity,
        log_radius,
        residuals,
    }
}

/// The value of the nodes' `values` at (`log_mass`, `log_age`), by the interpolation the fit
/// solves for.
#[must_use]
fn evaluate(
    masses: &[f64; MASSES],
    ages: &[f64; AGES],
    values: &[[f64; AGES]; MASSES],
    log_mass: f64,
    log_age: f64,
) -> f64 {
    bilinear_weights(masses, ages, log_mass, log_age)
        .iter()
        .fold(0.0, |sum, &(k, weight)| {
            sum + weight * values[k / AGES][k % AGES]
        })
}

/// The residuals of the fitted table against the grid's rows.
#[must_use]
fn residuals(
    masses: &[f64; MASSES],
    ages: &[f64; AGES],
    log_luminosity: &[[f64; AGES]; MASSES],
    log_radius: &[[f64; AGES]; MASSES],
    points: &[GridPoint],
) -> Residuals {
    let at = |values: &[[f64; AGES]; MASSES], point: &GridPoint| {
        evaluate(masses, ages, values, point.log_mass, point.log_age)
    };
    let origin = Worst {
        residual: 0.0,
        mass_mjup: 0.0,
        age_yr: 0.0,
    };
    let (mut worst_l, mut worst_r) = (origin, origin);
    let (mut sum_l, mut sum_r) = (0.0, 0.0);
    for point in points {
        let d_l = at(log_luminosity, point) - point.log_luminosity;
        let d_r = math::exp10(at(log_radius, point) - point.log_radius) - 1.0;
        sum_l += d_l * d_l;
        sum_r += d_r * d_r;
        let here = |residual| Worst {
            residual,
            mass_mjup: math::exp10(point.log_mass),
            age_yr: math::exp10(point.log_age),
        };
        if d_l.abs() > worst_l.residual.abs() {
            worst_l = here(d_l);
        }
        if d_r.abs() > worst_r.residual.abs() {
            worst_r = here(d_r);
        }
    }
    let n = index(points.len());
    Residuals {
        rows: u32::try_from(points.len()).expect("the grid has fewer than 2³² rows"),
        rms_log_luminosity: (sum_l / n).sqrt(),
        worst_log_luminosity: worst_l,
        rms_radius: (sum_r / n).sqrt(),
        worst_radius: worst_r,
    }
}

/// `value` rounded to `digits` decimals, for prose.
#[must_use]
fn rounded(value: f64, digits: usize) -> String {
    format!("{value:.digits$}")
}

/// Writes one array of nodes as a `pub const`.
fn write_nodes(out: &mut String, doc: &str, name: &str, nodes: &[f64]) {
    out.push_str(doc);
    writeln!(out, "pub const {name}: [f64; {}] = [", nodes.len()).expect("writing to a String");
    for &node in nodes {
        writeln!(out, "    {},", literal(node)).expect("writing to a String");
    }
    out.push_str("];\n");
}

/// Writes one table of node values as a `pub const`, by mass node, then age node.
fn write_values(out: &mut String, doc: &str, name: &str, values: &[[f64; AGES]; MASSES]) {
    out.push_str(doc);
    writeln!(out, "pub const {name}: [[f64; {AGES}]; {MASSES}] = [").expect("writing to a String");
    for row in values {
        out.push_str("    [\n");
        for &value in row {
            writeln!(out, "        {},", literal(value)).expect("writing to a String");
        }
        out.push_str("    ],\n");
    }
    out.push_str("];\n");
}

/// The committed source of `crates/hyperion-sim/src/tables/giant_cooling.rs`.
#[must_use]
pub fn render(table: &GiantCoolingTable) -> String {
    let r = &table.residuals;
    let mut out = format!(
        "\
//! The cooling of giant planets from {lo} to {hi} Jupiter masses, as a table in log mass and log
//! age (plan 13, P13.T5.b), which `stellar::substellar::giant_cooling` interpolates. Provisional:
//! plan 15's P15.T2 registers it under its own grammar.
//!
//! Generated by `hyperion-fit run giant_cooling`, version {VERSION}. Do not edit: `cargo test -p
//! hyperion-fit` renders the fit again and fails unless this file matches it byte for byte.
//!
//! Source: the Sonora Bobcat evolution tables (Marley et al. 2021, ApJ 920, 85), cloudless, at
//! \\[M/H\\] = 0 and solar C/O, from Zenodo record 5063476 (doi:10.5281/zenodo.5063476, licence
//! CC BY 4.0), retrieved 2026-09-23: their {rows} rows from 0.0005 to 0.011 M☉, with masses, radii
//! and luminosities converted to the IAU 2015 nominal units, committed as
//! `{GRID_FILE}`.
//!
//! Nodes: {MASSES} masses log-spaced from {lo} to {hi} Jupiter masses and {AGES} ages log-spaced from
//! 1 Myr to 15 Gyr. The node values are the least-squares solution for bilinear interpolation in
//! (log₁₀ mass, log₁₀ age) through every row, plus a penalty of weight {SMOOTHING:e} on the second
//! divided differences along each axis, which carries the table as straight lines in log–log into
//! the corners the grid does not reach: below 0.52 Jupiter masses, and past the ends of its
//! lightest tracks (3 Gyr at 0.52, 6 Gyr at 1.05 and 10 Gyr at 1.57 Jupiter masses).
//!
//! Residuals over the {rows} rows, fit minus grid: log₁₀ L within {l_worst} dex (rms {l_rms}), the
//! largest at {l_mass} Jupiter masses and {l_age} Gyr; R within {r_worst}% (rms {r_rms}%), the
//! largest at {r_mass} Jupiter masses and {r_age} Gyr.
",
        lo = MASS_RANGE_MJUP.0,
        hi = MASS_RANGE_MJUP.1,
        rows = r.rows,
        l_worst = rounded(r.worst_log_luminosity.residual.abs(), 3),
        l_rms = rounded(r.rms_log_luminosity, 3),
        l_mass = rounded(r.worst_log_luminosity.mass_mjup, 2),
        l_age = rounded(r.worst_log_luminosity.age_yr / YEARS_PER_GIGAYEAR, 3),
        r_worst = rounded(100.0 * r.worst_radius.residual.abs(), 2),
        r_rms = rounded(100.0 * r.rms_radius, 2),
        r_mass = rounded(r.worst_radius.mass_mjup, 2),
        r_age = rounded(r.worst_radius.age_yr / YEARS_PER_GIGAYEAR, 3),
    );
    out.push('\n');
    write_nodes(
        &mut out,
        "/// The mass nodes: log₁₀ of the mass in Jupiter masses, ascending.\n",
        "LOG_MASS_NODES",
        &table.log_mass,
    );
    out.push('\n');
    write_nodes(
        &mut out,
        "/// The age nodes: log₁₀ of the age in Julian years, ascending.\n",
        "LOG_AGE_NODES",
        &table.log_age,
    );
    out.push('\n');
    write_values(
        &mut out,
        "/// log₁₀ of the luminosity in nominal solar luminosities, by mass node, then age node.\n",
        "LOG_LUMINOSITY",
        &table.log_luminosity,
    );
    out.push('\n');
    write_values(
        &mut out,
        "/// log₁₀ of the radius in nominal solar radii, by mass node, then age node.\n",
        "LOG_RADIUS",
        &table.log_radius,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_holds_sixteen_tracks_in_its_range() {
        let points = grid();
        assert_eq!(points.len(), 455);
        let (lo, hi) = (math::log10(0.5), math::log10(11.6));
        assert!(
            points
                .iter()
                .all(|p| (lo..hi).contains(&p.log_mass) && (5.999..10.177).contains(&p.log_age))
        );
        // The first row, 0.0005 M☉ at 1 Myr: log L = −5.361, R = 0.1743 in Bobcat's units.
        let first = points[0];
        let mass_mjup = 0.0005 * BOBCAT_SOLAR_MASS_KG / JUPITER_MASS_KG;
        assert!((math::exp10(first.log_mass) / mass_mjup - 1.0).abs() < 1e-12);
        assert!((first.log_luminosity + 5.361).abs() < 1e-3);
        assert!((math::exp10(first.log_radius) - 0.1743).abs() < 1e-4);
    }

    #[test]
    #[should_panic(expected = "a block of the committed grid holds as many rows as it says")]
    fn a_short_block_is_refused() {
        let _rows = parse("# comment\n header\n  2\n 0.0005 0.001 -5.361 631. 2.654 0.1743 49.4\n");
    }

    #[test]
    #[should_panic(expected = "a row of the committed grid has seven columns")]
    fn a_narrow_row_is_refused() {
        let _rows = parse(" header\n  1\n 0.0005 0.001 -5.361\n");
    }

    #[test]
    fn nodes_are_evenly_spaced_in_log_and_end_exactly() {
        let masses = mass_nodes();
        assert!(masses[0].total_cmp(&math::log10(0.3)).is_eq());
        assert!(masses[MASSES - 1].total_cmp(&math::log10(13.0)).is_eq());
        let step = masses[1] - masses[0];
        for pair in masses.windows(2) {
            assert!((pair[1] - pair[0] - step).abs() < 1e-12);
        }
        let ages = age_nodes();
        assert!(ages[0].total_cmp(&6.0).is_eq());
        assert!(ages[AGES - 1].total_cmp(&math::log10(1.5e10)).is_eq());
    }

    #[test]
    fn locate_finds_the_interval_and_extrapolates_past_the_ends() {
        let nodes = [0.0, 1.0, 2.0, 4.0];
        assert_eq!(locate(&nodes, 0.5), (0, 0.5));
        assert_eq!(locate(&nodes, 1.0), (1, 0.0));
        assert_eq!(locate(&nodes, 3.0), (2, 0.5));
        assert_eq!(locate(&nodes, 4.0), (2, 1.0));
        assert_eq!(locate(&nodes, 6.0), (2, 2.0));
        assert_eq!(locate(&nodes, -1.0), (0, -1.0));
    }

    /// The solver against a problem with a known answer: data on a plane in (log mass, log age)
    /// are reproduced exactly, since the penalty vanishes on it.
    #[test]
    fn a_plane_is_fitted_exactly() {
        let masses = mass_nodes();
        let ages = age_nodes();
        let plane = |x: f64, y: f64| 1.5 * x - 0.75 * y + 2.0;
        let mut normal = NormalEquations::new();
        for i in 0..40 {
            for j in 0..40 {
                let x = masses[0] + (masses[MASSES - 1] - masses[0]) * index(i) / 39.0;
                let y = ages[0] + (ages[AGES - 1] - ages[0]) * index(j) / 39.0;
                normal.add(&bilinear_weights(&masses, &ages, x, y), [plane(x, y), 0.0]);
            }
        }
        for i in 0..MASSES {
            for j in 1..AGES - 1 {
                normal.add_curvature(
                    [i * AGES + j - 1, i * AGES + j, i * AGES + j + 1],
                    ages[j] - ages[j - 1],
                    ages[j + 1] - ages[j],
                );
            }
        }
        let [solution, zero] = normal.solve();
        for (k, value) in solution.iter().enumerate() {
            let expected = plane(masses[k / AGES], ages[k % AGES]);
            assert!(
                (value - expected).abs() < 1e-9,
                "{k}: {value} against {expected}"
            );
        }
        assert!(zero.iter().all(|z| z.abs() < 1e-12));
    }
}
