//! R08.T13's sweep: where Hillaire's analytic multiple-scattering term drifts from the reference
//! (plan R08, R08.T13.a and its top-ups, built in R08.T13.b; Design notes 9 and 10;
//! `decision-r08-t13-scope.md`, rulings 2–8 and "#112's floor refusals").
//!
//! `hyperion-fit atmosphere-sweep` reads a sweep case ([`SweepCase`], `SWEEP_FORMAT` 1, written by
//! the client's `reference/sweepCases.test.ts`), expands each of its points into an
//! [`AtmosphereCase`], traces it with the reference tracer, holds it to the CPU twin's radiance the
//! case carries by the metric's mirror (the `metric` module), and walks each column of the
//! grid to the τ*(ω, g) at which the twin first leaves the metric. It is not a
//! [`FitTask`](crate::task::FitTask): its outputs are fixtures for the client, outside
//! `tables.lock`, and its runs are by hand.
//!
//! # The sweep case (`SWEEP_FORMAT` 1)
//!
//! One JSON object, camel case, validated once by [`SweepCase::from_json`]:
//!
//! ```text
//! {
//!   "format": 1, "name": "ms-sweep-w950", "description": "...",
//!   "seed": 10000,                 // a point's final trace: seed + its index
//!   "pilotSeed": 1000000,          // a point's pilot: pilotSeed + its index
//!   "pilotSamples": 2048,          // a geometry's pilot count
//!   "template": { "radiusM", "scaleHeightM", "topHeightM", "groundAlbedo", "wavelengthNm",
//!                 "sunIrradiance", "detectorHalfAngleDeg", "rayleighDepolarisation" },
//!   "singleScatteringAlbedo": 0.95,
//!   "tau": [...],                  // the τ candidates, ascending: τ, vertical, ground to top
//!   "tauRoles": ["floor", "half-octave", "octave", ...],
//!   "extinctionPerM": [...],       // β₀ at each candidate: β₀ H (1 − e^(−top ÷ H)) = τ
//!   "phaseU": [...],               // the tables' u = √(Θ ÷ π), 0 to 1
//!   "geometries": [{ "name", "image", "kind": "ground" | "disc" | "limb" | "aerial",
//!                    "observer": { "heightM", "latitudeDeg" }, "view": { "zenithDeg", "azimuthDeg" },
//!                    "sun": { "zenithDeg", "azimuthDeg" }, "maxDistanceM"?,
//!                    "groundConeShare" }],     // its cone's share that meets the ground
//!   "aggregate": { "name", "latitudeDeg", "sun" },   // optional: recorded, never sets τ*
//!   "scanGroundAlbedos": [0, 0.02, ...],             // the floor scan's grounds, ascending from 0
//!   "columns": [{ "asymmetry": 0.5,
//!                 "phase": { "kind": "rayleigh" } | { "kind": "tabulated", "a1": [...] },
//!                 "points": [...],                     // each candidate's point index
//!                 "twin": [[...], ...],                // the twin's radiance, candidate by geometry
//!                 "scanPoints": [...],                 // each scan ground's point index
//!                 "scanTwin": [[...], ...] }]          // the twin's radiance, scan ground by geometry
//! }
//! ```
//!
//! A point is one candidate τ of one column over the template's ground, or the column's floor τ
//! over one of the scan's grounds. Its [`AtmosphereCase`] is built as JSON and read by
//! [`AtmosphereCase::from_json`], so that every point passes the tracer's own validation: one term,
//! `medium`, exponential at `scaleHeightM` to `topHeightM`, scattering ω β₀ and absorption
//! (1 − ω) β₀ at the one wavelength, its phase Rayleigh's with `rayleighDepolarisation` at g 0 or
//! the column's table on `phaseU` with `asymmetry` its source's mean cosine; a grey ground; one sun
//! of `sunIrradiance`; each geometry with its count; and the aggregate's fluxes. The case's β₀ is
//! the file's, so that the tracer and the twin read one β₀.
//!
//! # The walk (rulings 3, 5 and 6)
//!
//! Per column: the floor point; the octave points in ascending order to the first **stop**, a
//! point that fails or that a refusal or the point cap stops; then the half-octave point below that
//! stop, if its projection fits the cap. The must-have cells climb on past a drift
//! ("#112's floor refusals", item 3; `WalkPast`): (1, 0) to a reach or its last octave, and
//! ruling 3's other must-have cells through τ 1, each over the walk's ground; the points past the
//! first stop add verdicts alone, and the half-octave is still the one below it.
//!
//! Each point is piloted at `N_p` a geometry, at its pilot seed, and its final counts planned from
//! the pilot: `N_p (σ_p ÷ (T_p ÷ 8))² × 1.15` (T12.e's rule), `T_p` the pilot's own, in whole
//! blocks of [`BLOCK_SAMPLES`] and at least 8,192 (`MIN_FINAL_SAMPLES`, so that the first quarter
//! holds 2,048); the aggregate's alike at `σ_F ≤ 0.0025 F`. The cost is projected from
//! deterministic figures alone (main's reading 2): the pilot's counted vertices a sample at each
//! geometry (the tracer's `trace_counted`), at least one, times a fixed CPU per vertex, given to the run by
//! estimator ([`SweepSettings`]), so that no decision reads the clock. A geometry projected past
//! [`SweepSettings::geometry_cap_cpu_h`] is traced at that cap; a point whose summed projection,
//! pilot and final, passes [`SweepSettings::point_cap_cpu_h`] is not traced, and stops its column
//! as `reach`. The final trace is fresh, at the point's own seed.
//!
//! **The top-ups** ("#112's floor refusals", item 1). A geometry the metric refuses after the final
//! trace, `noisy` or `unscaled`, is carried on, on its own seed and stream (the tracer's
//! `trace_spans`), its count doubled in whole blocks, and every geometry held to the metric again
//! at each doubling, until it passes both checks or reaches its geometry cap, to which it is then
//! traced and checked once more. A point's top-ups stop too where the next would take its
//! projection, pilot, final and top-ups, past the point cap. Both caps are counted in samples from
//! the pilot's vertices and the run's CPU a vertex. A geometry carried on from N to N′ reads, bit
//! for bit, as one traced to N′, its first quarter too, which is carried on from the longest run
//! in whole blocks it holds below N′ ÷ 4 (every count is in whole blocks, the pilot's among them).
//! The σ target and the refusals are `gate.ts`'s, unmoved.
//!
//! A traced point's geometries are held to the twin by the metric's mirror. A geometry that fails
//! at its column's floor is a **floor miss**, listed and never a drift; a refused geometry within
//! 15° of the sun at a point that runs VROOM (`NEAR_SUN_DEG`; Buras and Mayer's §2.5 is unbuilt)
//! is listed only; any other refusal, which stands only at a cap, stops the column as `reach`; any
//! other failure is a `drift`. τ* is the least traced τ that fails (`drift`), else the last
//! verified passing τ (`reach`, 0 if none), else the last octave (`top`).
//!
//! # The floor scan (`decision-r08-t13-scope.md`, "#107's floor misses")
//!
//! Each column's floor τ is traced again over each of `scanGroundAlbedos`, whatever its walk did and
//! under the same rules, never cut. A geometry that fails over the black ground (the scan's first,
//! 0) is a **fixed miss**; one that passes there and fails over some brighter ground, the scan's or
//! the walk's own floor, is a **ground miss**: Hillaire's isotropic ground bounce, about 1 + c A of
//! the sky, which no τ shrinks. A*(ω, g) is the least albedo at which a ground miss fails, or 1
//! where none does. A geometry that fails somewhere but is refused, or not traced, over the black
//! ground is listed as unsorted and sets no A*. The walk is not changed by the scan: its floor
//! misses set no τ*, ground misses among them.
//!
//! # The work directory and the output
//!
//! Each finished point is written whole to `<work>/<case>-<key>/point-<index>.json` (to a
//! temporary name beside it, then renamed), the key the first 16 hex digits of the SHA-256 of
//! [`SWEEP_FORMAT`], the case's SHA-256 (its line ends taken as LF), the running binary's SHA-256,
//! the mode (the walk or the pilot alone) and every setting the walk decides by; a rerun reads
//! what is there and goes on (ruling 6; main's reading 4). The output (`--out`) is one JSON object, a
//! cell or a point a line, its numbers rounded to seven significant figures:
//!
//! ```text
//! { "format": 1, "kind": "sweep", "case", "caseSha256", "binarySha256", "settings": {...},
//!   "pilotSamples",             // N_p
//!   "geometries": [names],      // the order of every per-geometry array
//!   "cells": [{ "singleScatteringAlbedo", "asymmetry", "groundAlbedo", "vroom",
//!               "walkPast": { "kind": "no" | "through" | "top", "tau"? },   // WalkPast
//!               "kind": "drift" | "reach" | "top", "tauStar", "points": [indices, walk order],
//!               "floorMisses": [names],                       // the walk's, which set no τ*
//!               "listedRefusals": [{ "index", "geometry", "refusal" }],
//!               "scanPoints": [indices], "fixedMisses": [names],
//!               "groundMisses": [{ "geometry", "groundAlbedo" }],   // the least it fails over
//!               "unsortedMisses": [names], "albedoStar" }],
//!   "points": [{ "index", "tau", "singleScatteringAlbedo", "asymmetry", "groundAlbedo",
//!                "vroom", "pilotSeed", "class": "pass" | "drift" | "reach" | "scan",
//!                "projectedCpuH", "pilotVerticesPerSample", "cpuS", "wallS",
//!                "pilot": { "seed", "samples", "radiance", "standardError", "plannedSamples" }
//!                                              // a point the cap stopped, instead of "traced"
//!                "traced": { "seed", "samples",     // each geometry's, its top-ups' included
//!                            "doublings",           // each geometry's top-ups
//!                            "radiance", "standardError", "quarterStandardError", "twin",
//!                            "failing": [geometry indices],
//!                            "refused": [{ "geometry": index, "refusal", "listed",
//!                                          "cap": "geometry" | "point" }],
//!                            "aggregate": { "samples", "downwelling", "downwellingStandardError",
//!                                           "upwelling", "upwellingStandardError", "vertices",
//!                                           "refused" } } }] }
//! ```
//!
//! A row's twin is the case's at its column and candidate or scan ground, echoed (ruling 8). A
//! row's failing and refused geometries are indices into the head's `geometries`, a cell's misses
//! names, so that a file of 7 columns walked to the top and scanned stays under the 450 kB split
//! line (ruling 8).
//!
//! `cpuS` (the process's CPU time over the point's traces, Linux alone, else `null`) and `wallS`
//! differ by run, and a resumed point keeps its first run's; so do a pilot-only row's
//! `cpuPerVertexUs` and `cpuPerSampleUs`, worked out from them. Every other byte is the same for
//! any `--threads` (main's reading 3). `--pilot-only` writes `"kind": "pilot"` and a row a point with
//! its pilot whole: each geometry's radiance, σ, vertices and planned count, the aggregate's, and the
//! projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::time::Instant;

use hyperion_sim::math;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::case::{AtmosphereCase, ReadCaseError};
use super::metric::{self, AGGREGATE_SIGMA_CAP, Reading, Refusal};
use super::reference::{Moments, ReferenceRadiances};
use super::{BLOCK_SAMPLES, MAX_SAMPLES, Polarisation, Span, TraceReferenceError};
use crate::emit::sha256_hex;

/// The sweep case format this command reads, and the format of its output.
pub const SWEEP_FORMAT: u32 = 1;

/// The most CPU a geometry's final trace is projected to take, CPU-h: 4, about 1 h on 4 threads
/// (ruling 6).
pub(crate) const GEOMETRY_CAP_CPU_H: PositiveFigure = figure(4.0);

/// The most CPU a point's pilot and final trace are projected to take before it is traced, CPU-h:
/// 24, about 6 h on 4 threads (ruling 6), the ladder's first step's to lower.
pub(crate) const POINT_CAP_CPU_H: PositiveFigure = figure(24.0);

/// The margin on a planned count: 1.15 (R08.T12.e's rule).
pub(crate) const COUNT_MARGIN: PositiveFigure = figure(1.15);

/// The least final count of a geometry or the aggregate: 8,192, so that the first quarter that
/// `unscaled` reads holds 2,048 samples (`decision-r08-t13-scope.md`, "#112's floor refusals",
/// item 1).
pub(crate) const MIN_FINAL_SAMPLES: u64 = 8192;

/// The ground over which the must-have cells are walked on: the walk's 0.3, ruling 2's hand value.
const WALK_GROUND_ALBEDO: f64 = 0.3;

/// The must-have cells walked on through τ 1 past their first drift, by (ω, g).
///
/// Each is over the walk's ground (ruling 3's table; "#112's floor refusals", item 3): Earth's
/// ω 0.95, 0.99 and 1 at g 0 and 0.3,
/// but (1, 0), which is walked to its reach or top ([`WalkPast::Top`]); Mars's ω 0.9 and 0.95 at
/// g 0.5, 0.65 and 0.75; the Titan class's ω 0.9 and 0.95 at g 0.3, 0.5 and 0.65; and the Venus
/// class's (1, 0.75) and (1, 0.85). A cell two worlds share is listed once.
const WALKED_THROUGH_ONE: [(f64, f64); 14] = [
    (0.95, 0.0),
    (0.95, 0.3),
    (0.99, 0.0),
    (0.99, 0.3),
    (1.0, 0.3),
    (0.9, 0.3),
    (0.9, 0.5),
    (0.9, 0.65),
    (0.9, 0.75),
    (0.95, 0.5),
    (0.95, 0.65),
    (0.95, 0.75),
    (1.0, 0.75),
    (1.0, 0.85),
];

/// A refused geometry this close to the sun at a VROOM point is listed only, degrees: 15
/// (ruling 5; Buras and Mayer 2011's §2.5, circumsolar virtual importance sampling, is unbuilt).
pub(crate) const NEAR_SUN_DEG: f64 = 15.0;

/// Microseconds in an hour.
const MICROSECONDS_PER_HOUR: f64 = 3.6e9;

/// The smoke's sweep (`--smoke`): `ms-sweep-w900`'s columns g 0 and g 0.75 at its first five τ
/// candidates with the whole floor scan, and four geometries, two ground views and two disc
/// phases, which converge in a few thousand samples, at 1,024 samples a pilot, written by the
/// client's `reference/sweepCases.test.ts`; run under [`SweepSettings::SMOKE`].
pub const SMOKE_CASE: &str = include_str!("sweep_smoke.case.json");

/// A τ candidate's place in the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TauRole {
    /// The column's first point, 1/32, where the multiple scattering is a few per cent of the sky.
    Floor,
    /// A point the walk climbs through: a whole power of 2.
    Octave,
    /// A point the walk takes below its stop.
    HalfOctave,
}

/// A geometry's kind, which the case records for the pilot's and the record's grouping of σ and
/// costs; the walk does not read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GeometryKind {
    /// A view of the sky from the ground.
    Ground,
    /// The disc seen from orbit.
    Disc,
    /// The limb seen from orbit.
    Limb,
    /// Aerial perspective, a path to a black target.
    Aerial,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Template {
    radius_m: f64,
    scale_height_m: f64,
    top_height_m: f64,
    ground_albedo: f64,
    wavelength_nm: f64,
    sun_irradiance: f64,
    detector_half_angle_deg: f64,
    rayleigh_depolarisation: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Direction {
    zenith_deg: f64,
    azimuth_deg: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Observer {
    height_m: f64,
    latitude_deg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SweepGeometry {
    name: String,
    image: String,
    kind: GeometryKind,
    observer: Observer,
    view: Direction,
    sun: Direction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_distance_m: Option<f64>,
    /// The share of its cone that meets the ground within the view: recorded, never gated.
    ground_cone_share: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SweepAggregate {
    name: String,
    latitude_deg: f64,
    sun: Direction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum ColumnPhase {
    Rayleigh {},
    Tabulated { a1: Vec<f64> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Column {
    asymmetry: f64,
    phase: ColumnPhase,
    points: Vec<u64>,
    twin: Vec<Vec<f64>>,
    scan_points: Vec<u64>,
    scan_twin: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SweepFile {
    format: u32,
    name: String,
    description: String,
    seed: u64,
    pilot_seed: u64,
    pilot_samples: u64,
    template: Template,
    single_scattering_albedo: f64,
    tau: Vec<f64>,
    tau_roles: Vec<TauRole>,
    extinction_per_m: Vec<f64>,
    phase_u: Vec<f64>,
    geometries: Vec<SweepGeometry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aggregate: Option<SweepAggregate>,
    /// The floor scan's grey grounds, ascending from 0 (module documentation).
    scan_ground_albedos: Vec<f64>,
    columns: Vec<Column>,
}

/// A point of a column: a τ candidate of the walk over the template's ground, or the floor's τ
/// over one of the scan's grounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Spot {
    /// The walk's τ candidate, by index.
    Walk(usize),
    /// The floor scan's ground, by index.
    Scan(usize),
}

/// A sweep case, validated (module documentation): made only by [`from_json`](Self::from_json) or
/// [`read`](Self::read).
#[derive(Debug, Clone, PartialEq)]
pub struct SweepCase {
    file: SweepFile,
    /// Each geometry's view within [`NEAR_SUN_DEG`] of its sun.
    near_sun: Vec<bool>,
    /// Each column's points run VROOM, by the tracer's own rule.
    vroom: Vec<bool>,
    /// The SHA-256 of the text the case was read from, its line ends taken as LF.
    sha256: String,
}

/// A sweep case could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ReadSweepError {
    /// The file could not be read.
    #[error("cannot read the sweep case {}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// Why.
        #[source]
        source: std::io::Error,
    },
    /// The text is not JSON of the sweep format.
    #[error("the sweep case is not JSON of the sweep format")]
    Json(#[source] serde_json::Error),
    /// The case is of a format this command does not read.
    #[error("the sweep case's format is {0}, and this command reads format {SWEEP_FORMAT}")]
    UnsupportedFormat(u32),
    /// A value is out of its range, or an array has the wrong length.
    #[error("the sweep case's {field} {reason}")]
    Invalid {
        /// Where, as a path into the case: `columns[2].twin[4]`.
        field: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// A column's point is not a case the tracer reads.
    #[error("the sweep case's columns[{column}] expands to a case the tracer refuses")]
    Point {
        /// The column.
        column: usize,
        /// The tracer's reason.
        #[source]
        source: ReadCaseError,
    },
}

/// An [`ReadSweepError::Invalid`] at `field` unless `ok`.
fn require(
    ok: bool,
    field: impl FnOnce() -> String,
    reason: &'static str,
) -> Result<(), ReadSweepError> {
    if ok {
        Ok(())
    } else {
        Err(ReadSweepError::Invalid {
            field: field(),
            reason,
        })
    }
}

/// Whether `x` is finite and in `low..=high`.
#[must_use]
fn within(x: f64, low: f64, high: f64) -> bool {
    x.is_finite() && (low..=high).contains(&x)
}

/// A local direction's unit vector: (sin θ cos φ, sin θ sin φ, cos θ) in its frame.
#[must_use]
fn unit(direction: Direction) -> [f64; 3] {
    let (zenith, azimuth) = (
        direction.zenith_deg.to_radians(),
        direction.azimuth_deg.to_radians(),
    );
    let s = math::sin(zenith);
    [
        s * math::cos(azimuth),
        s * math::sin(azimuth),
        math::cos(zenith),
    ]
}

/// Whether a geometry's view lies within [`NEAR_SUN_DEG`] of its sun, both in the observer's
/// frame, to rounding.
#[must_use]
fn near_sun(geometry: &SweepGeometry) -> bool {
    let (v, s) = (unit(geometry.view), unit(geometry.sun));
    let cos = v[0] * s[0] + v[1] * s[1] + v[2] * s[2];
    cos >= math::cos(NEAR_SUN_DEG.to_radians()) - 1e-12
}

/// Checks the format, the name, the pilot's count, the template and ω.
fn check_head(file: &SweepFile) -> Result<(), ReadSweepError> {
    if file.format != SWEEP_FORMAT {
        return Err(ReadSweepError::UnsupportedFormat(file.format));
    }
    // The name is a path part of the work directory, so it keeps to letters every file system
    // takes.
    require(
        !file.name.is_empty()
            && file
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
        || "name".to_owned(),
        "must not be empty, and only lowercase letters, digits and hyphens",
    )?;
    // In whole blocks, so that every count planned from it is, as the top-ups' exact carry needs.
    require(
        (1..=MAX_SAMPLES).contains(&file.pilot_samples)
            && file.pilot_samples.is_multiple_of(BLOCK_SAMPLES),
        || "pilotSamples".to_owned(),
        "must lie in 1 to 2^53, in whole blocks of 1,024",
    )?;
    let template = &file.template;
    for (value, field) in [
        (template.radius_m, "radiusM"),
        (template.scale_height_m, "scaleHeightM"),
        (template.top_height_m, "topHeightM"),
        (template.wavelength_nm, "wavelengthNm"),
        (template.sun_irradiance, "sunIrradiance"),
    ] {
        require(
            value.is_finite() && value > 0.0,
            || format!("template.{field}"),
            "must be positive",
        )?;
    }
    require(
        within(template.ground_albedo, 0.0, 1.0),
        || "template.groundAlbedo".to_owned(),
        "must lie in [0, 1]",
    )?;
    require(
        within(template.rayleigh_depolarisation, 0.0, 0.999),
        || "template.rayleighDepolarisation".to_owned(),
        "must lie in [0, 1)",
    )?;
    require(
        within(file.single_scattering_albedo, 0.0, 1.0),
        || "singleScatteringAlbedo".to_owned(),
        "must lie in [0, 1]",
    )
}

/// Checks the τ candidates, their roles and their β₀, and the floor scan's grounds.
fn check_candidates(file: &SweepFile) -> Result<(), ReadSweepError> {
    let candidates = file.tau.len();
    require(candidates > 0, || "tau".to_owned(), "must not be empty")?;
    require(
        file.tau.iter().all(|&x| x.is_finite() && x > 0.0)
            && file.tau.windows(2).all(|pair| pair[0] < pair[1]),
        || "tau".to_owned(),
        "must be positive and strictly ascending",
    )?;
    require(
        file.tau_roles.len() == candidates && file.extinction_per_m.len() == candidates,
        || "tauRoles".to_owned(),
        "and extinctionPerM must have one entry a candidate",
    )?;
    require(
        file.tau_roles[0] == TauRole::Floor
            && file.tau_roles[1..]
                .iter()
                .all(|&role| role != TauRole::Floor),
        || "tauRoles".to_owned(),
        "must lead with the one floor",
    )?;
    let grounds = &file.scan_ground_albedos;
    require(
        grounds.first().is_some_and(|&a| a <= 0.0)
            && grounds.iter().all(|&a| within(a, 0.0, 1.0))
            && grounds.windows(2).all(|pair| pair[0] < pair[1]),
        || "scanGroundAlbedos".to_owned(),
        "must lie in [0, 1], strictly ascending from 0",
    )?;
    let template = &file.template;
    let height = template.scale_height_m;
    let column_of_top = height * (1.0 - math::exp(-template.top_height_m / height));
    for (k, (&tau, &beta)) in file.tau.iter().zip(&file.extinction_per_m).enumerate() {
        require(
            beta.is_finite() && (beta * column_of_top - tau).abs() <= 1e-12 * tau,
            || format!("extinctionPerM[{k}]"),
            "must give its tau to 1e-12",
        )?;
    }
    Ok(())
}

/// Checks the geometries' names, images and cone shares.
fn check_geometries(file: &SweepFile) -> Result<(), ReadSweepError> {
    require(
        !file.geometries.is_empty(),
        || "geometries".to_owned(),
        "must not be empty",
    )?;
    let mut names = BTreeSet::new();
    for (i, geometry) in file.geometries.iter().enumerate() {
        require(
            !geometry.name.is_empty()
                && !geometry.image.is_empty()
                && names.insert(geometry.name.as_str()),
            || format!("geometries[{i}].name"),
            "must be unique and not empty, with an image",
        )?;
        require(
            within(geometry.ground_cone_share, 0.0, 1.0),
            || format!("geometries[{i}].groundConeShare"),
            "must lie in [0, 1]",
        )?;
    }
    Ok(())
}

/// Checks a column's points and twin rows, `name` and `twin_name` their fields, one a `rows`
/// entry: each index unique in the case (`indices`, which it fills), its seeds below 2⁶⁴, and each
/// row one finite radiance at least 0 a geometry.
fn check_points(
    file: &SweepFile,
    (points, twin): (&[u64], &[Vec<f64>]),
    (name, twin_name): (&str, &str),
    rows: usize,
    indices: &mut BTreeSet<u64>,
) -> Result<(), ReadSweepError> {
    require(
        points.len() == rows && points.iter().all(|&p| indices.insert(p)),
        || name.to_owned(),
        "must have one index a candidate or scan ground, unique in the case",
    )?;
    require(
        points.iter().all(|&p| {
            file.seed.checked_add(p).is_some() && file.pilot_seed.checked_add(p).is_some()
        }),
        || name.to_owned(),
        "must leave every seed below 2^64",
    )?;
    require(
        twin.len() == rows,
        || twin_name.to_owned(),
        "must have one row a candidate or scan ground",
    )?;
    for (k, row) in twin.iter().enumerate() {
        require(
            row.len() == file.geometries.len() && row.iter().all(|&x| x.is_finite() && x >= 0.0),
            || format!("{twin_name}[{k}]"),
            "must have one finite radiance at least 0 a geometry",
        )?;
    }
    Ok(())
}

/// Checks each column's g, phase, point indices, seeds and twin, the walk's and the scan's.
fn check_columns(file: &SweepFile) -> Result<(), ReadSweepError> {
    require(
        !file.columns.is_empty(),
        || "columns".to_owned(),
        "must not be empty",
    )?;
    let mut indices = BTreeSet::new();
    for (c, column) in file.columns.iter().enumerate() {
        let field = |rest: &str| format!("columns[{c}].{rest}");
        require(
            within(column.asymmetry, 0.0, 0.999),
            || field("asymmetry"),
            "must lie in [0, 1)",
        )?;
        match &column.phase {
            ColumnPhase::Rayleigh {} => require(
                column.asymmetry <= 0.0,
                || field("phase"),
                "must be a table where the asymmetry is not 0",
            )?,
            ColumnPhase::Tabulated { a1 } => require(
                column.asymmetry > 0.0 && a1.len() == file.phase_u.len(),
                || field("phase.a1"),
                "must be Rayleigh's at g 0, and have one entry a u otherwise",
            )?,
        }
        check_points(
            file,
            (&column.points, &column.twin),
            (&field("points"), &field("twin")),
            file.tau.len(),
            &mut indices,
        )?;
        check_points(
            file,
            (&column.scan_points, &column.scan_twin),
            (&field("scanPoints"), &field("scanTwin")),
            file.scan_ground_albedos.len(),
            &mut indices,
        )?;
    }
    Ok(())
}

impl SweepCase {
    /// Reads and validates a sweep case from its JSON text.
    ///
    /// # Errors
    ///
    /// [`ReadSweepError::Json`] for text that is not JSON of the format, unknown fields included;
    /// [`ReadSweepError::UnsupportedFormat`] for another format; [`ReadSweepError::Invalid`] for a
    /// value out of its range: a template's length or albedo, an ω outside [0, 1], τ candidates
    /// that are not positive and strictly ascending, or not led by the one floor, a β₀ that does not
    /// give its τ to 10⁻¹², scan grounds that are not in [0, 1] and strictly ascending from 0, a
    /// column's phase that is not Rayleigh's at g 0 and a table on `phaseU` otherwise, point
    /// indices that are not one a candidate or scan ground and unique, a twin that is not one
    /// finite radiance at least 0 a candidate or scan ground and geometry, geometry names that
    /// repeat, or a pilot count of 0 or not in whole blocks; and [`ReadSweepError::Point`] for a
    /// column whose floor point the tracer's own validation refuses.
    pub fn from_json(text: &str) -> Result<Self, ReadSweepError> {
        let file: SweepFile = serde_json::from_str(text).map_err(ReadSweepError::Json)?;
        // The committed files are LF; a checkout that writes CRLF keeps the same hash, and so the
        // same work directory and `caseSha256`.
        Self::validated(file, sha256_hex(text.replace("\r\n", "\n").as_bytes()))
    }

    /// Reads and validates the sweep case in the file at `path`.
    ///
    /// # Errors
    ///
    /// [`ReadSweepError::Read`] if the file cannot be read, and the errors of
    /// [`from_json`](Self::from_json).
    pub fn read(path: &Path) -> Result<Self, ReadSweepError> {
        let text = std::fs::read_to_string(path).map_err(|source| ReadSweepError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_json(&text)
    }

    /// The case's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.file.name
    }

    /// The SHA-256 of the text the case was read from, its line ends taken as LF.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Validates `file` (module documentation), and finds what the walk reads of it.
    fn validated(file: SweepFile, sha256: String) -> Result<Self, ReadSweepError> {
        check_head(&file)?;
        check_candidates(&file)?;
        check_geometries(&file)?;
        check_columns(&file)?;
        let near_sun = file.geometries.iter().map(near_sun).collect();
        let mut case = Self {
            file,
            near_sun,
            vroom: Vec::new(),
            sha256,
        };
        let pilot = case.file.pilot_samples;
        let mut vroom = Vec::with_capacity(case.file.columns.len());
        for c in 0..case.file.columns.len() {
            let samples = vec![pilot; case.file.geometries.len()];
            let point = case
                .point_case(c, Spot::Walk(0), case.file.seed, &samples, Some(pilot))
                .map_err(|source| ReadSweepError::Point { column: c, source })?;
            vroom.push(super::runs_vroom(&point, 0));
        }
        case.vroom = vroom;
        Ok(case)
    }

    /// The τ candidate a spot is traced at: its own, or the floor for the scan.
    #[must_use]
    pub(crate) fn candidate(spot: Spot) -> usize {
        match spot {
            Spot::Walk(k) => k,
            Spot::Scan(_) => 0,
        }
    }

    /// A spot's τ.
    #[must_use]
    pub(crate) fn tau(&self, spot: Spot) -> f64 {
        self.file.tau[Self::candidate(spot)]
    }

    /// A spot's ground albedo: the template's, or the scan's.
    #[must_use]
    pub(crate) fn ground_albedo(&self, spot: Spot) -> f64 {
        match spot {
            Spot::Walk(_) => self.file.template.ground_albedo,
            Spot::Scan(a) => self.file.scan_ground_albedos[a],
        }
    }

    /// The point index of `spot` in column `column`.
    #[must_use]
    pub(crate) fn index(&self, column: usize, spot: Spot) -> u64 {
        let col = &self.file.columns[column];
        match spot {
            Spot::Walk(k) => col.points[k],
            Spot::Scan(a) => col.scan_points[a],
        }
    }

    /// The twin's radiance of `spot` in column `column`, by geometry.
    #[must_use]
    pub(crate) fn twin(&self, column: usize, spot: Spot) -> &[f64] {
        let col = &self.file.columns[column];
        match spot {
            Spot::Walk(k) => &col.twin[k],
            Spot::Scan(a) => &col.scan_twin[a],
        }
    }

    /// How far column `column`'s walk climbs past its first drift (module documentation).
    #[must_use]
    pub(crate) fn walk_past(&self, column: usize) -> WalkPast {
        WalkPast::of(
            self.file.single_scattering_albedo,
            self.file.columns[column].asymmetry,
            self.file.template.ground_albedo,
        )
    }

    /// The scan's spots, black ground first.
    pub(crate) fn scan_spots(&self) -> impl Iterator<Item = Spot> + use<> {
        (0..self.file.scan_ground_albedos.len()).map(Spot::Scan)
    }

    /// The column and spot of the point of index `index`, if the case holds one.
    #[must_use]
    pub(crate) fn place(&self, index: u64) -> Option<(usize, Spot)> {
        self.file
            .columns
            .iter()
            .enumerate()
            .find_map(|(c, column)| {
                let walk = column
                    .points
                    .iter()
                    .position(|&p| p == index)
                    .map(Spot::Walk);
                let scan = || {
                    column
                        .scan_points
                        .iter()
                        .position(|&p| p == index)
                        .map(Spot::Scan)
                };
                walk.or_else(scan).map(|spot| (c, spot))
            })
    }

    /// The point at `spot` of column `column`: its JSON case, with each geometry's count and the
    /// aggregate's, at `seed` (module documentation).
    #[must_use]
    fn point_json(
        &self,
        column: usize,
        spot: Spot,
        seed: u64,
        samples: &[u64],
        aggregate_samples: Option<u64>,
    ) -> Value {
        let file = &self.file;
        let template = &file.template;
        let col = &file.columns[column];
        let beta = file.extinction_per_m[Self::candidate(spot)];
        let omega = file.single_scattering_albedo;
        let phase = match &col.phase {
            ColumnPhase::Rayleigh {} => json!({
                "kind": "rayleigh",
                "depolarisation": [template.rayleigh_depolarisation],
            }),
            ColumnPhase::Tabulated { a1 } => json!({
                "kind": "tabulated",
                "u": file.phase_u,
                "a1": [a1],
                "asymmetry": [col.asymmetry],
            }),
        };
        let direction =
            |to: Direction| json!({ "zenithDeg": to.zenith_deg, "azimuthDeg": to.azimuth_deg });
        let geometries: Vec<Value> = file
            .geometries
            .iter()
            .zip(samples)
            .map(|(geometry, &n)| {
                let mut value = json!({
                    "name": geometry.name,
                    "observer": {
                        "heightM": geometry.observer.height_m,
                        "latitudeDeg": geometry.observer.latitude_deg,
                    },
                    "view": direction(geometry.view),
                    "sunDirections": [direction(geometry.sun)],
                    "samples": n,
                });
                if let Some(distance) = geometry.max_distance_m {
                    value["maxDistanceM"] = json!(distance);
                }
                value
            })
            .collect();
        let aggregates: Vec<Value> = match (&file.aggregate, aggregate_samples) {
            (Some(aggregate), Some(n)) => vec![json!({
                "name": aggregate.name,
                "latitudeDeg": aggregate.latitude_deg,
                "sunDirections": [direction(aggregate.sun)],
                "samples": n,
            })],
            _ => Vec::new(),
        };
        json!({
            "format": super::case::CASE_FORMAT,
            "name": format!("{}-p{}", file.name, self.index(column, spot)),
            "seed": seed,
            "wavelengthsNm": [template.wavelength_nm],
            "shells": { "kind": "sphere", "radiusM": template.radius_m },
            "topHeightM": template.top_height_m,
            "groundAlbedo": [self.ground_albedo(spot)],
            "terms": [{
                "name": "medium",
                "density": { "kind": "exponential", "scaleHeightM": template.scale_height_m },
                "scattering": [omega * beta],
                "absorption": [(1.0 - omega) * beta],
                "phase": phase,
            }],
            "suns": [{ "name": "sun", "irradiance": [template.sun_irradiance] }],
            "detectorHalfAngleDeg": template.detector_half_angle_deg,
            "geometries": geometries,
            "aggregates": aggregates,
        })
    }

    /// The point at `spot` of column `column` as the tracer's case (module documentation), each
    /// geometry at its count in `samples`, the aggregate at its own, at `seed`.
    ///
    /// # Errors
    ///
    /// The tracer's own [`ReadCaseError`], for a count outside 1 to 2⁵³ or a point the format let
    /// through and the tracer refuses.
    pub(crate) fn point_case(
        &self,
        column: usize,
        spot: Spot,
        seed: u64,
        samples: &[u64],
        aggregate_samples: Option<u64>,
    ) -> Result<AtmosphereCase, ReadCaseError> {
        AtmosphereCase::from_json(
            &self
                .point_json(column, spot, seed, samples, aggregate_samples)
                .to_string(),
        )
    }
}

/// What the evaluation of one point gave the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PointOutcome {
    /// Its projection passed the point cap: piloted, not traced.
    OverCap,
    /// Traced, with the metric's verdicts.
    Traced(PointVerdicts),
}

/// A traced point's verdicts by geometry index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PointVerdicts {
    /// The geometries that fail where the reference is not refused.
    pub(crate) failing: BTreeSet<usize>,
    /// The refused geometries.
    pub(crate) refused: Vec<RefusedGeometry>,
}

/// A geometry at which the reference is refused, once its top-ups have stopped at a cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RefusedGeometry {
    pub(crate) geometry: usize,
    pub(crate) refusal: Refusal,
    /// Within [`NEAR_SUN_DEG`] of the sun at a VROOM point: listed, not a stop.
    pub(crate) listed: bool,
    /// The cap its top-ups stopped at.
    pub(crate) cap: TopUpCap,
}

/// The cap at which a refused geometry's top-ups stopped ("#112's floor refusals", item 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TopUpCap {
    /// Its own, [`SweepSettings::geometry_cap_cpu_h`], in samples: traced to it.
    Geometry,
    /// The point's, [`SweepSettings::point_cap_cpu_h`]: its next top-up would pass it.
    Point,
}

/// A point's class in its column's walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PointClass {
    /// Every geometry passes, or fails as a floor miss, or is listed.
    Pass,
    /// Some other geometry fails.
    Drift,
    /// The point cap or a refusal stopped it.
    Reach,
}

/// A cell's τ* and what set it (ruling 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CellKind {
    /// The least traced τ at which the column fails.
    Drift,
    /// The last verified passing τ, where the cap or a refusal stopped the walk.
    Reach,
    /// The last octave: no stop came.
    Top,
}

/// How far a column's walk climbs past its first drift ("#112's floor refusals", item 3): its τ*
/// stays the first drift's, and the points past it add verdicts alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WalkPast {
    /// No farther: the climb ends at its first stop (ruling 3).
    No,
    /// Past every drift at a τ below this one; once a drift has come, the climb ends at this τ.
    Through(f64),
    /// Past every drift, to a reach or the last octave.
    Top,
}

impl WalkPast {
    /// A column's, by its ω, g and ground (module documentation): [`WalkPast::Top`] for (1, 0),
    /// [`WalkPast::Through`] τ 1 for [`WALKED_THROUGH_ONE`]'s cells, each over the walk's ground,
    /// and [`WalkPast::No`] for every other.
    #[must_use]
    fn of(omega: f64, g: f64, ground_albedo: f64) -> Self {
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-9;
        if !near(ground_albedo, WALK_GROUND_ALBEDO) {
            Self::No
        } else if near(omega, 1.0) && near(g, 0.0) {
            Self::Top
        } else if WALKED_THROUGH_ONE
            .iter()
            .any(|&(w, a)| near(omega, w) && near(g, a))
        {
            Self::Through(1.0)
        } else {
            Self::No
        }
    }

    /// The output's record of it: `{ "kind": "no" | "through" | "top", "tau"? }`.
    #[must_use]
    fn to_json(self) -> Value {
        match self {
            Self::No => json!({ "kind": "no" }),
            Self::Through(tau) => json!({ "kind": "through", "tau": rounded(tau) }),
            Self::Top => json!({ "kind": "top" }),
        }
    }
}

/// One column's walk (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnWalk {
    /// The candidates evaluated, in walk order, with their classes.
    pub(crate) visited: Vec<(usize, PointClass)>,
    pub(crate) kind: CellKind,
    pub(crate) tau_star: f64,
    /// The geometries that fail at the floor, which set no τ*.
    pub(crate) floor_misses: BTreeSet<usize>,
    /// The listed refusals, by candidate.
    pub(crate) listed: Vec<(usize, RefusedGeometry)>,
}

/// A point's class, given its column's floor misses.
#[must_use]
fn classify(outcome: &PointOutcome, floor_misses: &BTreeSet<usize>) -> PointClass {
    match outcome {
        PointOutcome::OverCap => PointClass::Reach,
        PointOutcome::Traced(v) => {
            if v.failing.iter().any(|g| !floor_misses.contains(g)) {
                PointClass::Drift
            } else if v.refused.iter().any(|r| !r.listed) {
                PointClass::Reach
            } else {
                PointClass::Pass
            }
        }
    }
}

/// Walks one column (module documentation), past its first drift as `past` says:
/// `evaluate(candidate)` pilots and traces a point.
///
/// # Errors
///
/// The first error of `evaluate`, which stops the walk there.
pub(crate) fn walk_column<E>(
    taus: &[f64],
    roles: &[TauRole],
    past: WalkPast,
    mut evaluate: impl FnMut(usize) -> Result<PointOutcome, E>,
) -> Result<ColumnWalk, E> {
    let mut walk = ColumnWalk {
        visited: Vec::new(),
        kind: CellKind::Top,
        tau_star: 0.0,
        floor_misses: BTreeSet::new(),
        listed: Vec::new(),
    };
    let mut visit = |candidate: usize, walk: &mut ColumnWalk| -> Result<PointClass, E> {
        let outcome = evaluate(candidate)?;
        if let PointOutcome::Traced(v) = &outcome {
            if roles[candidate] == TauRole::Floor {
                walk.floor_misses.clone_from(&v.failing);
            }
            walk.listed.extend(
                v.refused
                    .iter()
                    .filter(|r| r.listed)
                    .map(|&r| (candidate, r)),
            );
        }
        let class = classify(&outcome, &walk.floor_misses);
        walk.visited.push((candidate, class));
        Ok(class)
    };
    let climb = std::iter::once(0).chain((1..roles.len()).filter(|&k| roles[k] == TauRole::Octave));
    // The first point that does not pass, where ruling 3's climb ends and below which the
    // half-octave is taken; the climb goes on past a drift only as `past` says.
    let mut stop = None;
    let mut drifted = false;
    for candidate in climb {
        let class = visit(candidate, &mut walk)?;
        if class != PointClass::Pass && stop.is_none() {
            stop = Some(candidate);
        }
        let tau = taus[candidate];
        let goes_on = match class {
            PointClass::Pass => match past {
                WalkPast::Through(t) => !drifted || tau < t,
                WalkPast::No | WalkPast::Top => true,
            },
            PointClass::Drift => {
                drifted = true;
                match past {
                    WalkPast::No => false,
                    WalkPast::Through(t) => tau < t,
                    WalkPast::Top => true,
                }
            }
            PointClass::Reach => false,
        };
        if !goes_on {
            break;
        }
    }
    if let Some(at) = stop
        && at > 0
        && roles[at - 1] == TauRole::HalfOctave
    {
        visit(at - 1, &mut walk)?;
    }
    let tau_of = |classes: PointClass| {
        walk.visited
            .iter()
            .filter(move |(_, c)| *c == classes)
            .map(|&(k, _)| taus[k])
    };
    if let Some(least) = tau_of(PointClass::Drift).reduce(f64::min) {
        walk.kind = CellKind::Drift;
        walk.tau_star = least;
    } else if stop.is_some() {
        walk.kind = CellKind::Reach;
        walk.tau_star = tau_of(PointClass::Pass).reduce(f64::max).unwrap_or(0.0);
    } else {
        walk.kind = CellKind::Top;
        walk.tau_star = walk.visited.last().map_or(0.0, |&(k, _)| taus[k]);
    }
    Ok(walk)
}

/// A column's misses at its floor τ sorted by the floor scan (module documentation).
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct FloorSort {
    /// The geometries that fail over the black ground.
    pub(crate) fixed: BTreeSet<usize>,
    /// The geometries that pass over the black ground and fail over a brighter one, each with the
    /// least albedo it fails over.
    pub(crate) ground: BTreeMap<usize, f64>,
    /// The geometries that fail over some ground but are refused, or not traced, over the black.
    pub(crate) unsorted: BTreeSet<usize>,
}

impl FloorSort {
    /// A*: the least albedo at which a ground miss fails, or 1 where none does.
    #[must_use]
    pub(crate) fn albedo_star(&self) -> f64 {
        self.ground.values().copied().fold(1.0, f64::min)
    }
}

/// Sorts a column's misses at its floor τ (module documentation): `black` the point over the
/// black ground, `brighter` each other point at the floor τ with its ground's albedo, the scan's
/// and the walk's own floor. A point not traced, or over a ground of 0, the black's, sorts nothing.
#[must_use]
pub(crate) fn sort_floor<'o>(
    black: &PointOutcome,
    brighter: impl IntoIterator<Item = (f64, &'o PointOutcome)>,
) -> FloorSort {
    let mut sort = FloorSort::default();
    let black = match black {
        PointOutcome::Traced(v) => {
            sort.fixed.clone_from(&v.failing);
            Some(v)
        }
        PointOutcome::OverCap => None,
    };
    for (albedo, outcome) in brighter {
        let failing = match outcome {
            PointOutcome::Traced(v) if albedo > 0.0 => &v.failing,
            PointOutcome::Traced(_) | PointOutcome::OverCap => continue,
        };
        for &g in failing {
            match black {
                Some(b) if b.failing.contains(&g) => {}
                Some(b) if !b.refused.iter().any(|r| r.geometry == g) => {
                    let least = sort.ground.entry(g).or_insert(albedo);
                    *least = least.min(albedo);
                }
                Some(_) | None => {
                    sort.unsorted.insert(g);
                }
            }
        }
    }
    sort
}

/// A positive, finite figure: a cap in CPU-h, or a CPU time a vertex in µs.
#[derive(Debug, Clone, Copy)]
pub struct PositiveFigure(f64);

impl PositiveFigure {
    /// `x` as a figure, if it is positive and finite.
    #[must_use]
    pub const fn new(x: f64) -> Option<Self> {
        if x.is_finite() && x > 0.0 {
            Some(Self(x))
        } else {
            None
        }
    }

    /// The figure.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// `x` as a figure, at compile time: `x` is a positive literal.
const fn figure(x: f64) -> PositiveFigure {
    match PositiveFigure::new(x) {
        Some(figure) => figure,
        None => panic!("a figure's literal is positive and finite"),
    }
}

/// Figures are equal where they are the same number: no figure is NaN.
impl PartialEq for PositiveFigure {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for PositiveFigure {}

impl PartialOrd for PositiveFigure {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PositiveFigure {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl std::fmt::Display for PositiveFigure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for PositiveFigure {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.0)
    }
}

/// A figure is not a positive, finite number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, thiserror::Error)]
#[error("must be a positive, finite number")]
pub struct ParseFigureError;

impl std::str::FromStr for PositiveFigure {
    type Err = ParseFigureError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        // A text that is no number and a number out of range are refused alike.
        text.parse::<f64>()
            .ok()
            .and_then(Self::new)
            .ok_or(ParseFigureError)
    }
}

/// What the walk decides its counts and caps by (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepSettings {
    point_cap_cpu_h: PositiveFigure,
    geometry_cap_cpu_h: PositiveFigure,
    cpu_per_vertex_us: PositiveFigure,
    vroom_cpu_per_vertex_us: PositiveFigure,
    /// The least final count, samples.
    min_final_samples: u64,
    /// The margin on a planned count.
    count_margin: PositiveFigure,
}

impl SweepSettings {
    /// The smoke's: 50 µs a vertex under either estimator, a geometry capped at 2 CPU-s and a
    /// point at 20 CPU-s as projected, a final count of at least its pilot's 1,024, so that it
    /// runs in seconds; and a margin of 0.25 on a planned count, under R08.T12.e's 1.15, so that
    /// its first traces miss their σ target and its top-ups run (the forced top-up, "#112's floor
    /// refusals", item 1).
    pub const SMOKE: Self = Self {
        min_final_samples: BLOCK_SAMPLES,
        count_margin: figure(0.25),
        ..Self::new(
            figure(20.0 / 3600.0),
            figure(2.0 / 3600.0),
            figure(50.0),
            figure(50.0),
        )
    };

    /// The settings of a run: a point's cap and a geometry's, CPU-h (24 and 4 as ruled, the point
    /// cap the ladder's first step's to lower), and the CPU a vertex takes under R08.T12.c's
    /// estimator and under VROOM, µs, the pilot's measured figures (main's reading 2); with the
    /// ruled final count of at least 8,192 samples and margin of 1.15.
    #[must_use]
    pub const fn new(
        point_cap_cpu_h: PositiveFigure,
        geometry_cap_cpu_h: PositiveFigure,
        cpu_per_vertex_us: PositiveFigure,
        vroom_cpu_per_vertex_us: PositiveFigure,
    ) -> Self {
        Self {
            point_cap_cpu_h,
            geometry_cap_cpu_h,
            cpu_per_vertex_us,
            vroom_cpu_per_vertex_us,
            min_final_samples: MIN_FINAL_SAMPLES,
            count_margin: COUNT_MARGIN,
        }
    }

    /// A run at the ruled caps, 24 and 4 CPU-h, with the CPU a vertex takes under each estimator.
    #[must_use]
    pub const fn ruled(
        cpu_per_vertex_us: PositiveFigure,
        vroom_cpu_per_vertex_us: PositiveFigure,
    ) -> Self {
        Self::new(
            POINT_CAP_CPU_H,
            GEOMETRY_CAP_CPU_H,
            cpu_per_vertex_us,
            vroom_cpu_per_vertex_us,
        )
    }

    /// This run with its point cap at `point_cap_cpu_h`: the ladder's first step lowers it.
    #[must_use]
    pub const fn with_point_cap(self, point_cap_cpu_h: PositiveFigure) -> Self {
        Self {
            point_cap_cpu_h,
            ..self
        }
    }

    /// A point's cap, CPU-h.
    #[must_use]
    pub const fn point_cap_cpu_h(&self) -> f64 {
        self.point_cap_cpu_h.get()
    }

    /// A geometry's cap, CPU-h.
    #[must_use]
    pub const fn geometry_cap_cpu_h(&self) -> f64 {
        self.geometry_cap_cpu_h.get()
    }

    /// The CPU a vertex takes under R08.T12.c's estimator, µs.
    #[must_use]
    pub const fn cpu_per_vertex_us(&self) -> f64 {
        self.cpu_per_vertex_us.get()
    }

    /// The CPU a vertex takes under VROOM, µs.
    #[must_use]
    pub const fn vroom_cpu_per_vertex_us(&self) -> f64 {
        self.vroom_cpu_per_vertex_us.get()
    }

    /// The least final count of a geometry or the aggregate, samples.
    #[must_use]
    pub const fn min_final_samples(&self) -> u64 {
        self.min_final_samples
    }

    /// The margin on a planned count.
    #[must_use]
    pub const fn count_margin(&self) -> f64 {
        self.count_margin.get()
    }
}

/// A sweep could not be run.
#[derive(Debug, thiserror::Error)]
pub enum RunSweepError {
    /// A point could not be expanded into a case.
    #[error("the sweep's point {index} is not a case the tracer reads")]
    Point {
        /// The point.
        index: u64,
        /// The tracer's reason.
        #[source]
        source: ReadCaseError,
    },
    /// A point could not be traced.
    #[error(transparent)]
    Trace(#[from] TraceReferenceError),
    /// `--points` named a point the case does not hold.
    #[error("the sweep case holds no point {0}")]
    UnknownPoint(u64),
    /// The work directory could not be read or written.
    #[error("cannot use the sweep's work directory {}", path.display())]
    Work {
        /// The file.
        path: PathBuf,
        /// Why.
        #[source]
        source: std::io::Error,
    },
    /// A file in the work directory is not a finished point of this run: another point's, or
    /// another τ's or ground's.
    #[error("{} in the sweep's work directory is not a point of this run", path.display())]
    Stale {
        /// The file.
        path: PathBuf,
    },
    /// A file in the work directory is not a point record at all: cut short, or of another format.
    #[error("{} in the sweep's work directory is not a point record", path.display())]
    CorruptPoint {
        /// The file.
        path: PathBuf,
        /// Why it does not read.
        #[source]
        source: serde_json::Error,
    },
    /// The progress could not be written.
    #[error("cannot write the sweep's progress")]
    Progress(#[source] std::io::Error),
}

/// What the run is asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SweepMode {
    /// The whole walk.
    Walk,
    /// The pilot stage alone, on these points, in this order.
    PilotOnly(Vec<u64>),
}

/// A sweep to run (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct SweepRun<'a> {
    case: &'a SweepCase,
    settings: SweepSettings,
    mode: SweepMode,
    threads: NonZeroUsize,
    /// The work directory's root, where each finished point is kept, or `None` to keep nothing.
    work: Option<&'a Path>,
    /// The running binary's SHA-256, which keys the work directory and is recorded.
    binary_sha256: &'a str,
    /// Stop after this many points traced or piloted in this invocation, to resume later.
    max_points: Option<usize>,
}

impl<'a> SweepRun<'a> {
    /// A run of `case` in `mode` under `settings` on `threads` threads (the output is the same for
    /// any number), by the binary of SHA-256 `binary_sha256`, keeping nothing and running to the
    /// end.
    #[must_use]
    pub fn new(
        case: &'a SweepCase,
        settings: SweepSettings,
        mode: SweepMode,
        threads: NonZeroUsize,
        binary_sha256: &'a str,
    ) -> Self {
        Self {
            case,
            settings,
            mode,
            threads,
            work: None,
            binary_sha256,
            max_points: None,
        }
    }

    /// This run keeping each finished point under `root`, and resuming from what is there.
    #[must_use]
    pub fn with_work(self, root: &'a Path) -> Self {
        Self {
            work: Some(root),
            ..self
        }
    }

    /// This run stopping after `most` points traced or piloted in this invocation.
    #[must_use]
    pub fn with_max_points(self, most: usize) -> Self {
        Self {
            max_points: Some(most),
            ..self
        }
    }
}

/// What a run gave.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SweepOutcome {
    text: Option<String>,
    computed: usize,
    resumed: usize,
}

impl SweepOutcome {
    /// The output's text, or `None` where the run stopped at its most points before the end.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// The output's text, taken.
    #[must_use]
    pub fn into_text(self) -> Option<String> {
        self.text
    }

    /// The points traced or piloted in this invocation.
    #[must_use]
    pub fn computed(&self) -> usize {
        self.computed
    }

    /// The points read back from the work directory.
    #[must_use]
    pub fn resumed(&self) -> usize {
        self.resumed
    }
}

/// The process's CPU time so far, s: every thread's user and system time, `/proc/self/stat`'s
/// 14th and 15th fields (`proc_pid_stat(5)`), the whole thread group's (Linux `fs/proc/array.c`,
/// `proc_tgid_stat` through `thread_group_cputime_adjusted`), in clock ticks of `USER_HZ`, 100 a
/// second on every architecture (`__USER_HZ`, `include/uapi/asm-generic/param.h`), which
/// `sysconf(_SC_CLK_TCK)` would read only through a dependency the crate does not take; `None`
/// where there is no such file, as on macOS and Windows, or where it cannot be read.
#[must_use]
fn process_cpu_s() -> Option<f64> {
    // The time is recorded and printed alone: no decision reads it, so a missing, unreadable or
    // strange file costs a figure, never a result.
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let after = &stat[stat.rfind(')')? + 1..];
    let fields: Vec<&str> = after.split_whitespace().collect();
    let user: u64 = fields.get(11)?.parse().ok()?;
    let system: u64 = fields.get(12)?.parse().ok()?;
    Some(count(user + system) / 100.0)
}

/// `n` as an `f64`: a sample count, exact below 2⁵³, or a sum of counts or vertices, a projection
/// or a reported figure, which rounding above it moves by an ulp at most.
#[expect(
    clippy::cast_precision_loss,
    reason = "exact below 2⁵³; above it the figure is a projection or a report, and an ulp is no error"
)]
#[must_use]
fn count(n: u64) -> f64 {
    n as f64
}

/// `x` rounded up to whole blocks of [`BLOCK_SAMPLES`], held to `low..=`[`MAX_SAMPLES`].
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "x is held to [0, 2⁵³] first, where an f64 is an exact u64"
)]
#[must_use]
fn blocks_up(x: f64, low: u64) -> u64 {
    let blocks = (x / count(BLOCK_SAMPLES)).ceil();
    let samples = (blocks * count(BLOCK_SAMPLES)).clamp(0.0, count(MAX_SAMPLES));
    (samples as u64).max(low)
}

/// `x` rounded down to whole blocks, held to `low..=`[`MAX_SAMPLES`].
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "x is held to [0, 2⁵³] first, where an f64 is an exact u64"
)]
#[must_use]
fn blocks_down(x: f64, low: u64) -> u64 {
    let blocks = (x / count(BLOCK_SAMPLES)).floor();
    let samples = (blocks * count(BLOCK_SAMPLES)).clamp(0.0, count(MAX_SAMPLES));
    (samples as u64).max(low)
}

/// The CPU a sample takes, µs, as projected from a pilot (main's reading 2): its `vertices` over
/// `samples`, at least one a sample (the forced first collision's, so that no geometry's cap is
/// unbounded), times `cpu_per_vertex_us`.
#[must_use]
fn sample_cost_us(vertices: u64, samples: u64, cpu_per_vertex_us: f64) -> f64 {
    (count(vertices) / count(samples)).max(1.0) * cpu_per_vertex_us
}

/// The least final count from a pilot of `n_p` samples: `N_p`, or the settings' least if more.
#[must_use]
fn least_count(n_p: u64, settings: &SweepSettings) -> u64 {
    n_p.max(settings.min_final_samples())
}

/// A geometry's cap in samples (module documentation): the count `cpu_per_sample_us`, the pilot's
/// vertices a sample times the CPU a vertex, projects to the settings' geometry cap, in whole
/// blocks and at least [`least_count`]; [`MAX_SAMPLES`] where the pilot counted no vertex.
#[must_use]
fn cap_count(n_p: u64, cpu_per_sample_us: f64, settings: &SweepSettings) -> u64 {
    let low = least_count(n_p, settings);
    if cpu_per_sample_us > 0.0 {
        blocks_down(
            settings.geometry_cap_cpu_h() * MICROSECONDS_PER_HOUR / cpu_per_sample_us,
            low,
        )
    } else {
        MAX_SAMPLES.max(low)
    }
}

/// A final count planned from a pilot of `n_p` samples (module documentation): R08.T12.e's
/// `N_p (σ_p ÷ target)² × 1.15` (the settings' margin) for a σ target `target` (`T_p ÷ 8`, or
/// `0.0025 F_p` for a flux), in whole blocks and at least [`least_count`]; held at the geometry's
/// [`cap_count`] where it would pass it, and whether it was.
#[must_use]
fn planned_count(
    n_p: u64,
    sigma: f64,
    target: f64,
    cpu_per_sample_us: f64,
    settings: &SweepSettings,
) -> (u64, bool) {
    let need = if sigma > 0.0 && target > 0.0 {
        let ratio = sigma / target;
        count(n_p) * ratio * ratio * settings.count_margin()
    } else {
        count(n_p)
    };
    let n = blocks_up(need, least_count(n_p, settings));
    let cap = cap_count(n_p, cpu_per_sample_us, settings);
    if n > cap { (cap, true) } else { (n, false) }
}

/// A pilot's aggregate: the two fluxes and their σ, and their vertices.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FluxRecord {
    samples: u64,
    /// The downwelling flux at the ground, direct and diffuse.
    downwelling: f64,
    downwelling_standard_error: f64,
    downwelling_quarter_standard_error: f64,
    /// The upwelling flux at the top.
    upwelling: f64,
    upwelling_standard_error: f64,
    upwelling_quarter_standard_error: f64,
    vertices: u64,
}

/// A trace's aggregate in one channel, if the case has one: the downwelling flux at the ground,
/// direct and diffuse, and the upwelling flux at the top, each with its σ, and its vertices.
#[must_use]
fn flux_record(reference: &ReferenceRadiances, vertices: Option<u64>) -> Option<FluxRecord> {
    let flux = reference.aggregates().first()?;
    Some(FluxRecord {
        samples: flux.samples(),
        downwelling: flux.direct_ground()[0] + flux.diffuse_ground()[0],
        downwelling_standard_error: flux.diffuse_ground_standard_error()[0],
        downwelling_quarter_standard_error: flux.diffuse_ground_quarter_standard_error()[0],
        upwelling: flux.upwelling_top()[0],
        upwelling_standard_error: flux.upwelling_top_standard_error()[0],
        upwelling_quarter_standard_error: flux.upwelling_top_quarter_standard_error()[0],
        vertices: vertices.unwrap_or(0),
    })
}

/// A point's pilot, its planned counts and its projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PilotRecord {
    seed: u64,
    samples: u64,
    radiance: Vec<f64>,
    standard_error: Vec<f64>,
    vertices: Vec<u64>,
    aggregate: Option<FluxRecord>,
    planned_samples: Vec<u64>,
    planned_aggregate_samples: Option<u64>,
    /// The geometries the geometry cap held.
    capped: Vec<usize>,
    projected_cpu_h: f64,
    cpu_s: Option<f64>,
    wall_s: f64,
}

/// A point's final trace, its top-ups and its verdicts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TracedRecord {
    seed: u64,
    /// Each geometry's final count, its top-ups' included.
    samples: Vec<u64>,
    radiance: Vec<f64>,
    standard_error: Vec<f64>,
    quarter_standard_error: Vec<f64>,
    /// Each geometry's top-ups: its count doubled, or the last raised to its cap.
    doublings: Vec<u32>,
    /// The vertices of each geometry's final count of samples.
    vertices: Vec<u64>,
    aggregate: Option<FluxRecord>,
    /// The aggregate's reference past `σ_F ≤ 0.0025 F` in either flux: recorded only.
    aggregate_refused: bool,
    failing: Vec<usize>,
    refused: Vec<RefusedGeometry>,
    cpu_s: Option<f64>,
    wall_s: f64,
}

/// A finished point, as the work directory keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PointRecord {
    index: u64,
    tau: f64,
    ground_albedo: f64,
    vroom: bool,
    pilot: PilotRecord,
    traced: Option<TracedRecord>,
}

impl PointRecord {
    #[must_use]
    fn outcome(&self) -> PointOutcome {
        match &self.traced {
            None => PointOutcome::OverCap,
            Some(t) => PointOutcome::Traced(PointVerdicts {
                failing: t.failing.iter().copied().collect(),
                refused: t.refused.clone(),
            }),
        }
    }
}

/// The work directory of one run's key (module documentation).
#[derive(Debug, Clone)]
struct WorkDir {
    dir: PathBuf,
}

impl WorkDir {
    #[must_use]
    fn path(&self, index: u64) -> PathBuf {
        self.dir.join(format!("point-{index:04}.json"))
    }

    fn load(
        &self,
        index: u64,
        tau: f64,
        ground_albedo: f64,
    ) -> Result<Option<PointRecord>, RunSweepError> {
        let path = self.path(index);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(RunSweepError::Work { path, source }),
        };
        let record: PointRecord =
            serde_json::from_str(&text).map_err(|source| RunSweepError::CorruptPoint {
                path: path.clone(),
                source,
            })?;
        if record.index != index
            || record.tau.total_cmp(&tau).is_ne()
            || record.ground_albedo.total_cmp(&ground_albedo).is_ne()
        {
            return Err(RunSweepError::Stale { path });
        }
        Ok(Some(record))
    }

    fn save(&self, record: &PointRecord) -> Result<(), RunSweepError> {
        let io = |path: &Path| {
            let path = path.to_path_buf();
            move |source| RunSweepError::Work { path, source }
        };
        std::fs::create_dir_all(&self.dir).map_err(io(&self.dir))?;
        let path = self.path(record.index);
        let temporary = self
            .dir
            .join(format!("point-{:04}.json.partial", record.index));
        let text = serde_json::to_string(record)
            .expect("a record of plain numbers, strings and vectors serialises into a String");
        std::fs::write(&temporary, text).map_err(io(&temporary))?;
        std::fs::rename(&temporary, &path).map_err(io(&path))
    }
}

/// Why the walk of a run stopped short.
enum Halt {
    /// At `max_points`.
    Stopped,
    Error(RunSweepError),
}

impl From<RunSweepError> for Halt {
    fn from(error: RunSweepError) -> Self {
        Self::Error(error)
    }
}

/// A run in progress: the case, its settings and the points finished so far.
struct Runner<'a, 'w> {
    run: &'a SweepRun<'a>,
    work: Option<WorkDir>,
    progress: &'w mut dyn Write,
    records: BTreeMap<u64, PointRecord>,
    computed: usize,
    resumed: usize,
}

/// A trace's results with its time.
struct Timed {
    reference: ReferenceRadiances,
    geometry_vertices: Vec<u64>,
    aggregate_vertices: Option<u64>,
    /// Each geometry's radiance moments at the case's one wavelength.
    geometry_moments: Vec<super::GeometryMoments>,
    cpu_s: Option<f64>,
    wall_s: f64,
}

/// A refused geometry's next top-up ([`Carried::next_step`]), or the cap it stops at.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// Its run carried on to `end`, and its first quarter to `end ÷ 4` from `quarter`, the moments
    /// of the longest run it holds below that, where it holds none to there; projected to take
    /// `cost_us` of CPU.
    Carry {
        end: u64,
        quarter: Option<Moments>,
        cost_us: f64,
    },
    /// No top-up: it stops at this cap.
    Stop(TopUpCap),
}

/// A geometry's run as its top-ups carry it on (module documentation): its whole run's moments,
/// the moments of each run of samples `0..b` it holds by `b` (the empty run's, its first quarter's,
/// and each count it was traced to, and each quarter its top-ups traced), its top-ups, its
/// vertices, and the cap its top-ups stopped at.
#[derive(Debug, Clone)]
struct Carried {
    whole: Moments,
    prefixes: BTreeMap<u64, Moments>,
    doublings: u32,
    vertices: u64,
    stopped: Option<TopUpCap>,
}

impl Carried {
    /// A geometry traced once, `moments` its run's and its first quarter's.
    #[must_use]
    fn new(moments: super::GeometryMoments, vertices: u64) -> Self {
        let n = moments.whole.samples();
        let prefixes = BTreeMap::from([
            (0, Moments::default()),
            (n / 4, moments.quarter),
            (n, moments.whole),
        ]);
        Self {
            whole: moments.whole,
            prefixes,
            doublings: 0,
            vertices,
            stopped: None,
        }
    }

    /// Its count.
    #[must_use]
    fn samples(&self) -> u64 {
        self.whole.samples()
    }

    /// The moments of its first quarter, ⌊N ÷ 4⌋, which each top-up keeps.
    #[must_use]
    fn quarter(&self) -> Moments {
        self.prefixes[&(self.samples() / 4)]
    }

    /// Its next top-up, or the cap it stops at (module documentation).
    ///
    /// From N, its run is carried on to 2N, or to `cap` where that is less, and its first quarter
    /// from the longest run in whole blocks it holds below the new N ÷ 4, so that the quarter too
    /// reads, bit for bit, as a fresh trace's; at `cpu_per_sample_us` a sample, a step that would
    /// take the point's projection, `projected_us`, past `point_cap_us` is not taken.
    #[must_use]
    fn next_step(
        &self,
        cap: u64,
        cpu_per_sample_us: f64,
        projected_us: f64,
        point_cap_us: f64,
    ) -> Step {
        let n = self.samples();
        if n >= cap {
            return Step::Stop(TopUpCap::Geometry);
        }
        let end = n.saturating_mul(2).min(cap);
        let quarter = end / 4;
        // A run in whole blocks folds as a fresh trace's first blocks do: one that ends inside a
        // block would merge that block's samples in two parts.
        let (&base, &prior) = self
            .prefixes
            .range(..=quarter)
            .rev()
            .find(|&(&b, _)| b.is_multiple_of(BLOCK_SAMPLES))
            .expect("every geometry holds the empty run's moments, at 0");
        let cost_us = count(end - n + (quarter - base)) * cpu_per_sample_us;
        if projected_us + cost_us > point_cap_us {
            return Step::Stop(TopUpCap::Point);
        }
        Step::Carry {
            end,
            quarter: (base < quarter).then_some(prior),
            cost_us,
        }
    }

    /// Its reading in `image` for the metric.
    #[must_use]
    fn reading<'a>(&self, image: &'a str) -> Reading<'a> {
        let n = self.samples();
        Reading {
            image,
            radiance: self.whole.mean(),
            standard_error: self.whole.standard_error(),
            samples: n,
            quarter_samples: n / 4,
            quarter_standard_error: self.quarter().standard_error(),
        }
    }
}

impl Runner<'_, '_> {
    #[must_use]
    fn case(&self) -> &SweepCase {
        self.run.case
    }

    /// Traces `case` at its own counts on the run's threads, timed.
    fn trace(&self, case: &AtmosphereCase) -> Result<Timed, RunSweepError> {
        let (cpu, wall) = (process_cpu_s(), Instant::now());
        let counted = super::trace_counted(
            case,
            Polarisation::Scalar,
            NonZeroU64::MIN,
            self.run.threads,
        )?;
        let wall_s = wall.elapsed().as_secs_f64();
        let cpu_s = cpu.zip(process_cpu_s()).map(|(a, b)| b - a);
        Ok(Timed {
            reference: counted.reference,
            geometry_vertices: counted.geometry_vertices,
            aggregate_vertices: counted.aggregate_vertices.first().copied(),
            geometry_moments: counted
                .geometry_moments
                .into_iter()
                .next()
                .unwrap_or_default(),
            cpu_s,
            wall_s,
        })
    }

    /// The CPU each geometry's sample takes at `column`'s points, µs, as projected from `pilot`:
    /// its vertices a sample times the CPU a vertex (main's reading 2).
    #[must_use]
    fn cpu_per_sample_us(&self, column: usize, pilot: &PilotRecord) -> Vec<f64> {
        let c_v = self.cpu_per_vertex_us(column);
        pilot
            .vertices
            .iter()
            .map(|&v| sample_cost_us(v, pilot.samples, c_v))
            .collect()
    }

    /// The CPU a vertex takes at `column`'s points, µs, by its estimator.
    fn cpu_per_vertex_us(&self, column: usize) -> f64 {
        if self.case().vroom[column] {
            self.run.settings.vroom_cpu_per_vertex_us()
        } else {
            self.run.settings.cpu_per_vertex_us()
        }
    }

    /// The point at `spot` of `column` piloted, its counts planned (module documentation).
    fn pilot(&self, column: usize, spot: Spot) -> Result<PilotRecord, RunSweepError> {
        let file = &self.case().file;
        let index = self.case().index(column, spot);
        let n_p = file.pilot_samples;
        let seed = file.pilot_seed + index;
        let samples = vec![n_p; file.geometries.len()];
        let aggregate_samples = file.aggregate.as_ref().map(|_| n_p);
        let case = self
            .case()
            .point_case(column, spot, seed, &samples, aggregate_samples)
            .map_err(|source| RunSweepError::Point { index, source })?;
        let timed = self.trace(&case)?;
        let reference = &timed.reference;
        let radiance: Vec<f64> = reference
            .geometries()
            .iter()
            .map(|g| g.radiance()[0])
            .collect();
        let standard_error: Vec<f64> = reference
            .geometries()
            .iter()
            .map(|g| g.standard_error()[0])
            .collect();
        let readings: Vec<Reading<'_>> = file
            .geometries
            .iter()
            .zip(&radiance)
            .map(|(g, &l)| Reading {
                image: &g.image,
                radiance: l,
                standard_error: 0.0,
                samples: n_p,
                quarter_samples: n_p / 4,
                quarter_standard_error: 0.0,
            })
            .collect();
        let maxima = metric::image_maxima(&readings);
        let c_v = self.cpu_per_vertex_us(column);
        let settings = &self.run.settings;
        let per_sample = |vertices: u64| sample_cost_us(vertices, n_p, c_v);
        let mut capped = Vec::new();
        let mut planned = Vec::with_capacity(radiance.len());
        for (g, ((r, &sigma), &vertices)) in readings
            .iter()
            .zip(&standard_error)
            .zip(&timed.geometry_vertices)
            .enumerate()
        {
            let target = metric::metric_tolerance(r.radiance, maxima[r.image])
                / metric::METRIC_SIGMA_DIVISOR;
            let (n, held) = planned_count(n_p, sigma, target, per_sample(vertices), settings);
            if held {
                capped.push(g);
            }
            planned.push(n);
        }
        let aggregate = flux_record(reference, timed.aggregate_vertices);
        let planned_aggregate = aggregate.map(|a| {
            let cost = per_sample(a.vertices);
            let (down, _) = planned_count(
                n_p,
                a.downwelling_standard_error,
                AGGREGATE_SIGMA_CAP * a.downwelling,
                cost,
                settings,
            );
            let (up, _) = planned_count(
                n_p,
                a.upwelling_standard_error,
                AGGREGATE_SIGMA_CAP * a.upwelling,
                cost,
                settings,
            );
            down.max(up)
        });
        let mut projected_us = 0.0;
        for (&n, &vertices) in planned.iter().zip(&timed.geometry_vertices) {
            projected_us += count(n_p + n) * per_sample(vertices);
        }
        if let (Some(a), Some(n)) = (aggregate, planned_aggregate) {
            projected_us += count(n_p + n) * per_sample(a.vertices);
        }
        Ok(PilotRecord {
            seed,
            samples: n_p,
            radiance,
            standard_error,
            vertices: timed.geometry_vertices,
            aggregate,
            planned_samples: planned,
            planned_aggregate_samples: planned_aggregate,
            capped,
            projected_cpu_h: projected_us / MICROSECONDS_PER_HOUR,
            cpu_s: timed.cpu_s,
            wall_s: timed.wall_s,
        })
    }

    /// The point's final trace at its planned counts, its top-ups, and its verdicts (module
    /// documentation).
    fn final_trace(
        &self,
        column: usize,
        spot: Spot,
        pilot: &PilotRecord,
    ) -> Result<TracedRecord, RunSweepError> {
        let case = self.case();
        let file = &case.file;
        let index = case.index(column, spot);
        let seed = file.seed + index;
        let point = case
            .point_case(
                column,
                spot,
                seed,
                &pilot.planned_samples,
                pilot.planned_aggregate_samples,
            )
            .map_err(|source| RunSweepError::Point { index, source })?;
        let (cpu, wall) = (process_cpu_s(), Instant::now());
        let timed = self.trace(&point)?;
        let mut carried: Vec<Carried> = timed
            .geometry_moments
            .iter()
            .zip(&timed.geometry_vertices)
            .map(|(&moments, &vertices)| Carried::new(moments, vertices))
            .collect();
        let verdicts = self.top_up(column, pilot, &point, &mut carried, case.twin(column, spot))?;
        let wall_s = wall.elapsed().as_secs_f64();
        let cpu_s = cpu.zip(process_cpu_s()).map(|(a, b)| b - a);
        let vroom = case.vroom[column];
        let failing = verdicts
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.passes && v.refusal.is_none())
            .map(|(g, _)| g)
            .collect();
        let refused = verdicts
            .iter()
            .enumerate()
            .filter_map(|(g, v)| {
                v.refusal.map(|refusal| RefusedGeometry {
                    geometry: g,
                    refusal,
                    listed: vroom && case.near_sun[g],
                    cap: carried[g]
                        .stopped
                        .expect("the top-ups leave every refused geometry stopped at a cap"),
                })
            })
            .collect();
        let aggregate = flux_record(&timed.reference, timed.aggregate_vertices);
        let aggregate_refused = aggregate.is_some_and(|a| {
            a.downwelling_standard_error > AGGREGATE_SIGMA_CAP * a.downwelling
                || a.upwelling_standard_error > AGGREGATE_SIGMA_CAP * a.upwelling
        });
        Ok(TracedRecord {
            seed,
            samples: carried.iter().map(Carried::samples).collect(),
            radiance: carried.iter().map(|c| c.whole.mean()).collect(),
            standard_error: carried.iter().map(|c| c.whole.standard_error()).collect(),
            quarter_standard_error: carried
                .iter()
                .map(|c| c.quarter().standard_error())
                .collect(),
            doublings: carried.iter().map(|c| c.doublings).collect(),
            vertices: carried.iter().map(|c| c.vertices).collect(),
            aggregate,
            aggregate_refused,
            failing,
            refused,
            cpu_s,
            wall_s,
        })
    }

    /// Tops up `point`'s refused geometries, `carried` as its final trace left them, and gives the
    /// metric's verdicts at the end (module documentation; "#112's floor refusals", item 1).
    ///
    /// Each round holds every geometry to `twin` by the metric, and carries each refused geometry
    /// on, on its own seed and stream, from its count N to 2N, or to its cap where that is less,
    /// with its first quarter. A geometry at its cap stops there, and so does one whose round
    /// would take the point past its cap: the pilot's and the planned counts' projection, with
    /// every top-up so far. Both caps are counted in samples, at the pilot's vertices a sample and
    /// the run's CPU a vertex, never by the clock. The rounds end when no refused geometry can go
    /// on.
    fn top_up(
        &self,
        column: usize,
        pilot: &PilotRecord,
        point: &AtmosphereCase,
        carried: &mut [Carried],
        twin: &[f64],
    ) -> Result<Vec<metric::Verdict>, RunSweepError> {
        let settings = &self.run.settings;
        let geometries = &self.case().file.geometries;
        let per_sample = self.cpu_per_sample_us(column, pilot);
        let caps: Vec<u64> = per_sample
            .iter()
            .map(|&us| cap_count(pilot.samples, us, settings))
            .collect();
        let point_cap_us = settings.point_cap_cpu_h() * MICROSECONDS_PER_HOUR;
        let mut projected_us = pilot.projected_cpu_h * MICROSECONDS_PER_HOUR;
        loop {
            let readings: Vec<Reading<'_>> = geometries
                .iter()
                .zip(carried.iter())
                .map(|(g, c)| c.reading(&g.image))
                .collect();
            let verdicts = metric::verdicts(&readings, twin);
            // This round's spans in geometry order: each geometry's run carried on and then,
            // where it holds no run to its new quarter, the part of the quarter it lacks.
            let mut spans = Vec::new();
            let mut taken = Vec::new();
            for (g, verdict) in verdicts.iter().enumerate() {
                let c = &mut carried[g];
                if verdict.refusal.is_none() || c.stopped.is_some() {
                    continue;
                }
                match c.next_step(caps[g], per_sample[g], projected_us, point_cap_us) {
                    Step::Stop(cap) => c.stopped = Some(cap),
                    Step::Carry {
                        end,
                        quarter,
                        cost_us,
                    } => {
                        projected_us += cost_us;
                        spans.push(Span {
                            wavelength: 0,
                            geometry: g,
                            prior: c.whole,
                            end,
                        });
                        if let Some(prior) = quarter {
                            spans.push(Span {
                                wavelength: 0,
                                geometry: g,
                                prior,
                                end: end / 4,
                            });
                        }
                        taken.push((g, quarter.is_some()));
                    }
                }
            }
            if spans.is_empty() {
                return Ok(verdicts);
            }
            let mut traced = super::trace_spans(point, &spans, self.run.threads)?.into_iter();
            let mut next_span = || {
                traced
                    .next()
                    .expect("the tracer gives one result a span, in order")
            };
            for (g, piece) in taken {
                let c = &mut carried[g];
                let run = next_span();
                c.whole = run.moments;
                c.vertices += run.vertices;
                c.prefixes.insert(run.moments.samples(), run.moments);
                if piece {
                    let quarter = next_span();
                    c.prefixes
                        .insert(quarter.moments.samples(), quarter.moments);
                }
                c.doublings += 1;
            }
        }
    }

    /// Whether this invocation has done its `max_points`.
    #[must_use]
    fn at_limit(&self) -> bool {
        self.run
            .max_points
            .is_some_and(|most| self.computed >= most)
    }

    /// The point at `spot` of `column`, from the work directory or computed: piloted, and at
    /// [`Stage::Walk`] traced unless over the point cap.
    fn point(&mut self, column: usize, spot: Spot, stage: Stage) -> Result<&PointRecord, Halt> {
        let case = self.run.case;
        let file = &case.file;
        let index = case.index(column, spot);
        let tau = case.tau(spot);
        let ground = case.ground_albedo(spot);
        if self.records.contains_key(&index) {
            return Ok(&self.records[&index]);
        }
        if let Some(work) = &self.work
            && let Some(record) = work.load(index, tau, ground)?
        {
            self.resumed += 1;
            return Ok(self.records.entry(index).or_insert(record));
        }
        if self.at_limit() {
            return Err(Halt::Stopped);
        }
        let omega = file.single_scattering_albedo;
        let g = file.columns[column].asymmetry;
        let pilot = self.pilot(column, spot)?;
        let mut line = format!(
            "point {index} (ω {omega}, g {g}, τ {tau:.4}, A {ground}): pilot {:.1} s, projected \
             {:.3} CPU-h",
            pilot.wall_s, pilot.projected_cpu_h
        );
        let traced = if stage == Stage::PilotOnly {
            None
        } else if pilot.projected_cpu_h > self.run.settings.point_cap_cpu_h() {
            line.push_str("; over the point cap, not traced");
            None
        } else {
            let traced = self.final_trace(column, spot, &pilot)?;
            // Writing into a `String` cannot fail.
            let _ = write!(
                line,
                "; traced {:.1} s, {} top-ups: {} failing, {} refused",
                traced.wall_s,
                traced.doublings.iter().sum::<u32>(),
                traced.failing.len(),
                traced.refused.len()
            );
            Some(traced)
        };
        writeln!(self.progress, "{line}").map_err(RunSweepError::Progress)?;
        let record = PointRecord {
            index,
            tau,
            ground_albedo: ground,
            vroom: case.vroom[column],
            pilot,
            traced,
        };
        if let Some(work) = &self.work {
            work.save(&record)?;
        }
        self.computed += 1;
        Ok(self.records.entry(index).or_insert(record))
    }
}

/// How far a run takes each point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The pilot alone.
    PilotOnly,
    /// The pilot, and the final trace where the point fits its cap.
    Walk,
}

/// What a run writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    /// The walk's cells and points.
    Sweep,
    /// The pilots alone.
    Pilot,
}

/// The work directory's key (module documentation): the SHA-256 of the format, the case's
/// SHA-256, the binary's and the settings and mode the walk decides by.
#[must_use]
fn work_key(run: &SweepRun<'_>) -> String {
    let mode = match &run.mode {
        SweepMode::Walk => "walk",
        SweepMode::PilotOnly(_) => "pilot",
    };
    let settings = serde_json::to_string(&run.settings)
        .expect("a struct of plain numbers serialises into a String");
    let key = format!(
        "sweep-format {SWEEP_FORMAT}\ncase {}\nbinary {}\nmode {mode}\nsettings {settings}\n",
        run.case.sha256(),
        run.binary_sha256
    );
    sha256_hex(key.as_bytes())
}

/// A figure rounded to seven significant figures (ruling 8), as JSON.
#[must_use]
fn rounded(x: f64) -> Value {
    let text = format!("{x:.6e}");
    json!(
        text.parse::<f64>()
            .expect("a finite f64 in exponent form reads back as an f64")
    )
}

/// Figures rounded to seven significant figures, as a JSON array.
#[must_use]
fn rounded_all(values: &[f64]) -> Value {
    Value::Array(values.iter().map(|&x| rounded(x)).collect())
}

/// An optional time, rounded.
#[must_use]
fn time(seconds: Option<f64>) -> Value {
    seconds.map_or(Value::Null, rounded)
}

/// Geometry names by index.
#[must_use]
fn names(case: &SweepCase, indices: impl IntoIterator<Item = usize>) -> Value {
    Value::Array(
        indices
            .into_iter()
            .map(|g| json!(case.file.geometries[g].name))
            .collect(),
    )
}

/// An aggregate's record, rounded.
#[must_use]
fn flux_json(flux: &FluxRecord) -> Value {
    json!({
        "samples": flux.samples,
        "downwelling": rounded(flux.downwelling),
        "downwellingStandardError": rounded(flux.downwelling_standard_error),
        "upwelling": rounded(flux.upwelling),
        "upwellingStandardError": rounded(flux.upwelling_standard_error),
        "vertices": flux.vertices,
    })
}

/// A traced aggregate's record, rounded, with whether its reference misses its σ cap.
#[must_use]
fn traced_flux_json(flux: &FluxRecord, refused: bool) -> Value {
    let mut value = flux_json(flux);
    value["refused"] = json!(refused);
    value
}

/// A row of the output's points: a walk point with its class, or a scan point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    /// The walk's τ candidate, with the point's class in the walk.
    Walk(usize, PointClass),
    /// The floor scan's ground.
    Scan(usize),
}

impl Row {
    /// The row's point in its column.
    #[must_use]
    fn spot(self) -> Spot {
        match self {
            Self::Walk(k, _) => Spot::Walk(k),
            Self::Scan(a) => Spot::Scan(a),
        }
    }
}

/// A point's row of the output (module documentation), `row` its place in `column`.
#[must_use]
fn point_row(case: &SweepCase, column: usize, record: &PointRecord, row: Row) -> Value {
    let file = &case.file;
    let pilot = &record.pilot;
    let pilot_vertices: u64 = pilot.vertices.iter().sum();
    let per_sample = count(pilot_vertices)
        / (count(pilot.samples) * count(super::wide(pilot.vertices.len().max(1))));
    let cpu = match (pilot.cpu_s, record.traced.as_ref().map(|t| t.cpu_s)) {
        (Some(p), Some(Some(t))) => Some(p + t),
        (p, None) => p,
        _ => None,
    };
    let wall = pilot.wall_s + record.traced.as_ref().map_or(0.0, |t| t.wall_s);
    let class = match row {
        Row::Walk(_, class) => json!(class),
        Row::Scan(_) => json!("scan"),
    };
    let mut value = json!({
        "index": record.index,
        "tau": rounded(record.tau),
        "singleScatteringAlbedo": file.single_scattering_albedo,
        "asymmetry": file.columns[column].asymmetry,
        "groundAlbedo": record.ground_albedo,
        "vroom": record.vroom,
        "pilotSeed": pilot.seed,
        "class": class,
        "projectedCpuH": rounded(pilot.projected_cpu_h),
        "pilotVerticesPerSample": rounded(per_sample),
        "cpuS": time(cpu),
        "wallS": rounded(wall),
    });
    match &record.traced {
        None => {
            value["pilot"] = json!({
                "seed": pilot.seed,
                "samples": pilot.samples,
                "radiance": rounded_all(&pilot.radiance),
                "standardError": rounded_all(&pilot.standard_error),
                "plannedSamples": pilot.planned_samples,
            });
        }
        Some(t) => {
            let refused: Vec<Value> = t
                .refused
                .iter()
                .map(|r| {
                    json!({
                        "geometry": r.geometry,
                        "refusal": r.refusal,
                        "listed": r.listed,
                        "cap": r.cap,
                    })
                })
                .collect();
            value["traced"] = json!({
                "seed": t.seed,
                "samples": t.samples,
                "doublings": t.doublings,
                "radiance": rounded_all(&t.radiance),
                "standardError": rounded_all(&t.standard_error),
                "quarterStandardError": rounded_all(&t.quarter_standard_error),
                "twin": rounded_all(case.twin(column, row.spot())),
                "failing": t.failing,
                "refused": refused,
                "aggregate": t.aggregate.as_ref().map(|a| traced_flux_json(a, t.aggregate_refused)),
            });
        }
    }
    value
}

/// A pilot-only row (module documentation): the pilot whole.
#[must_use]
fn pilot_row(case: &SweepCase, column: usize, record: &PointRecord) -> Value {
    let file = &case.file;
    let pilot = &record.pilot;
    let vertices: u64 =
        pilot.vertices.iter().sum::<u64>() + pilot.aggregate.map_or(0, |a| a.vertices);
    let sources = pilot.vertices.len() + pilot.aggregate.map_or(0, |_| 2);
    let samples = pilot.samples * super::wide(sources);
    json!({
        "index": record.index,
        "tau": rounded(record.tau),
        "singleScatteringAlbedo": file.single_scattering_albedo,
        "asymmetry": file.columns[column].asymmetry,
        "groundAlbedo": record.ground_albedo,
        "vroom": record.vroom,
        "seed": pilot.seed,
        "samples": pilot.samples,
        "cpuS": time(pilot.cpu_s),
        "wallS": rounded(pilot.wall_s),
        "vertices": vertices,
        "sampleSources": samples,
        "cpuPerVertexUs": time(pilot.cpu_s.map(|s| s * 1e6 / count(vertices.max(1)))),
        "cpuPerSampleUs": time(pilot.cpu_s.map(|s| s * 1e6 / count(samples.max(1)))),
        "projectedCpuH": rounded(pilot.projected_cpu_h),
        "radiance": rounded_all(&pilot.radiance),
        "standardError": rounded_all(&pilot.standard_error),
        "geometryVertices": pilot.vertices,
        "plannedSamples": pilot.planned_samples,
        "capped": names(case, pilot.capped.iter().copied()),
        "aggregate": pilot.aggregate.as_ref().map(flux_json),
        "plannedAggregateSamples": pilot.planned_aggregate_samples,
    })
}

/// A cell's row of the output (module documentation): its walk's kind and τ*, its points, its
/// floor misses and its listed refusals, and its floor scan's points, sorted misses and A*.
#[must_use]
fn cell_row(case: &SweepCase, column: usize, walk: &ColumnWalk, sort: &FloorSort) -> Value {
    let file = &case.file;
    let points = &file.columns[column].points;
    let indices: Vec<u64> = walk.visited.iter().map(|&(k, _)| points[k]).collect();
    let ground: Vec<Value> = sort
        .ground
        .iter()
        .map(|(&g, &albedo)| json!({ "geometry": file.geometries[g].name, "groundAlbedo": albedo }))
        .collect();
    let listed: Vec<Value> = walk
        .listed
        .iter()
        .map(|&(k, refused)| {
            json!({
                "index": points[k],
                "geometry": file.geometries[refused.geometry].name,
                "refusal": refused.refusal,
            })
        })
        .collect();
    json!({
        "singleScatteringAlbedo": file.single_scattering_albedo,
        "asymmetry": file.columns[column].asymmetry,
        "groundAlbedo": file.template.ground_albedo,
        "vroom": case.vroom[column],
        "walkPast": case.walk_past(column).to_json(),
        "kind": walk.kind,
        "tauStar": rounded(walk.tau_star),
        "points": indices,
        "floorMisses": names(case, walk.floor_misses.iter().copied()),
        "listedRefusals": listed,
        "scanPoints": file.columns[column].scan_points,
        "fixedMisses": names(case, sort.fixed.iter().copied()),
        "groundMisses": ground,
        "unsortedMisses": names(case, sort.unsorted.iter().copied()),
        "albedoStar": sort.albedo_star(),
    })
}

/// The output's text: its head, then a cell or a point a line (module documentation).
#[must_use]
fn render(run: &SweepRun<'_>, kind: OutputKind, cells: &[Value], points: &[Value]) -> String {
    let case = run.case;
    let head = |value: &Value| {
        serde_json::to_string(value).expect("a JSON value of finite numbers serialises")
    };
    let name = match kind {
        OutputKind::Sweep => "sweep",
        OutputKind::Pilot => "pilot",
    };
    let mut text = String::new();
    text.push_str("{\n");
    for (key, value) in [
        ("format", json!(SWEEP_FORMAT)),
        ("kind", json!(name)),
        ("case", json!(case.name())),
        ("caseSha256", json!(case.sha256())),
        ("binarySha256", json!(run.binary_sha256)),
        (
            "settings",
            serde_json::to_value(run.settings)
                .expect("a struct of plain numbers serialises into a JSON value"),
        ),
        ("pilotSamples", json!(case.file.pilot_samples)),
        (
            "geometries",
            Value::Array(case.file.geometries.iter().map(|g| json!(g.name)).collect()),
        ),
    ] {
        // Writing into a `String` cannot fail.
        let _ = writeln!(text, "  {}: {},", head(&json!(key)), head(&value));
    }
    let block = |text: &mut String, key: &str, rows: &[Value], last: bool| {
        // Writing into a `String` cannot fail.
        let _ = write!(text, "  {}: [", head(&json!(key)));
        for (i, row) in rows.iter().enumerate() {
            text.push_str(if i == 0 { "\n    " } else { ",\n    " });
            text.push_str(&head(row));
        }
        text.push_str(if rows.is_empty() { "]" } else { "\n  ]" });
        text.push_str(if last { "\n" } else { ",\n" });
    };
    if kind == OutputKind::Sweep {
        block(&mut text, "cells", cells, false);
    }
    block(&mut text, "points", points, true);
    text.push_str("}\n");
    text
}

/// Runs a sweep (module documentation), printing each point to `progress` as it finishes.
///
/// # Errors
///
/// [`RunSweepError::UnknownPoint`] for a `--points` index the case does not hold;
/// [`RunSweepError::Point`] and [`RunSweepError::Trace`] for a point that cannot be traced;
/// [`RunSweepError::Work`], [`RunSweepError::Stale`] and [`RunSweepError::CorruptPoint`] for a work
/// directory that cannot be used; [`RunSweepError::Progress`] if `progress` cannot be written.
///
/// # Panics
///
/// As [`trace_reference`](super::trace_reference), if a traced value is not finite: a bug in the
/// tracer.
pub fn run_sweep(
    run: &SweepRun<'_>,
    progress: &mut dyn Write,
) -> Result<SweepOutcome, RunSweepError> {
    let case = run.case;
    let key = work_key(run);
    let mut runner = Runner {
        run,
        work: run.work.map(|root| WorkDir {
            dir: root.join(format!("{}-{}", case.name(), &key[..16])),
        }),
        progress,
        records: BTreeMap::new(),
        computed: 0,
        resumed: 0,
    };
    let outcome = |runner: &Runner<'_, '_>, text: Option<String>| SweepOutcome {
        text,
        computed: runner.computed,
        resumed: runner.resumed,
    };
    match &run.mode {
        SweepMode::PilotOnly(indices) => {
            let places = indices
                .iter()
                .map(|&i| case.place(i).ok_or(RunSweepError::UnknownPoint(i)))
                .collect::<Result<Vec<_>, _>>()?;
            let mut rows = Vec::with_capacity(places.len());
            for (column, spot) in places {
                match runner.point(column, spot, Stage::PilotOnly) {
                    Ok(record) => rows.push(pilot_row(case, column, record)),
                    Err(Halt::Stopped) => return Ok(outcome(&runner, None)),
                    Err(Halt::Error(error)) => return Err(error),
                }
            }
            let text = render(run, OutputKind::Pilot, &[], &rows);
            Ok(outcome(&runner, Some(text)))
        }
        SweepMode::Walk => {
            let mut cells = Vec::with_capacity(case.file.columns.len());
            let mut points = Vec::new();
            for column in 0..case.file.columns.len() {
                let past = case.walk_past(column);
                let walked = walk_column(&case.file.tau, &case.file.tau_roles, past, |candidate| {
                    runner
                        .point(column, Spot::Walk(candidate), Stage::Walk)
                        .map(PointRecord::outcome)
                });
                let walk = match walked {
                    Ok(walk) => walk,
                    Err(Halt::Stopped) => return Ok(outcome(&runner, None)),
                    Err(Halt::Error(error)) => return Err(error),
                };
                // The floor scan, whatever the walk did (module documentation).
                let mut scan = Vec::new();
                for spot in case.scan_spots() {
                    match runner.point(column, spot, Stage::Walk) {
                        Ok(record) => scan.push(record.outcome()),
                        Err(Halt::Stopped) => return Ok(outcome(&runner, None)),
                        Err(Halt::Error(error)) => return Err(error),
                    }
                }
                let floor = runner.records[&case.index(column, Spot::Walk(0))].outcome();
                let ground = |spot| case.ground_albedo(spot);
                let brighter = case
                    .scan_spots()
                    .zip(&scan)
                    .skip(1)
                    .map(|(spot, o)| (ground(spot), o))
                    .chain(std::iter::once((ground(Spot::Walk(0)), &floor)));
                let sort = sort_floor(&scan[0], brighter);
                cells.push(cell_row(case, column, &walk, &sort));
                let walked = walk.visited.iter().map(|&(k, class)| Row::Walk(k, class));
                let scanned = (0..case.file.scan_ground_albedos.len()).map(Row::Scan);
                for row in walked.chain(scanned) {
                    let record = &runner.records[&case.index(column, row.spot())];
                    points.push(point_row(case, column, record, row));
                }
            }
            let text = render(run, OutputKind::Sweep, &cells, &points);
            Ok(outcome(&runner, Some(text)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic point: these geometries fail, these are refused.
    fn traced(failing: &[usize], refused: &[(usize, bool)]) -> PointOutcome {
        PointOutcome::Traced(PointVerdicts {
            failing: failing.iter().copied().collect(),
            refused: refused
                .iter()
                .map(|&(geometry, listed)| RefusedGeometry {
                    geometry,
                    refusal: Refusal::Noisy,
                    listed,
                    cap: TopUpCap::Geometry,
                })
                .collect(),
        })
    }

    /// The ruled ladder: the floor, then 2^(k ÷ 2) from 2^−3.5 to 8.
    fn ladder() -> (Vec<f64>, Vec<TauRole>) {
        let mut taus = vec![1.0 / 32.0];
        let mut roles = vec![TauRole::Floor];
        for k in -7..=6_i32 {
            taus.push(math::powf(2.0, f64::from(k) / 2.0));
            roles.push(if k % 2 == 0 {
                TauRole::Octave
            } else {
                TauRole::HalfOctave
            });
        }
        (taus, roles)
    }

    /// Walks the ladder with `outcome(candidate)`, recording what it asked for.
    fn walk_ladder(outcome: impl Fn(usize) -> PointOutcome) -> (ColumnWalk, Vec<usize>) {
        walk_ladder_past(WalkPast::No, outcome)
    }

    /// Walks the ladder past its first drift as `past` says.
    fn walk_ladder_past(
        past: WalkPast,
        outcome: impl Fn(usize) -> PointOutcome,
    ) -> (ColumnWalk, Vec<usize>) {
        let (taus, roles) = ladder();
        let mut asked = Vec::new();
        let walk = walk_column::<()>(&taus, &roles, past, |k| {
            asked.push(k);
            Ok(outcome(k))
        })
        .unwrap();
        (walk, asked)
    }

    #[test]
    fn atmosphere_sweep_walk_through_one_climbs_past_a_drift_to_tau_1() {
        // Fails from τ ½ (candidate 6) up: the climb goes on to τ 1 (8) and ends there, then takes
        // the half-octave below its first drift; τ* stays the first drift's, ½.
        let fails_from =
            |from: usize| move |k: usize| traced(if k >= from { &[3] } else { &[] }, &[]);
        let (walk, asked) = walk_ladder_past(WalkPast::Through(1.0), fails_from(6));
        assert_eq!(asked, [0, 2, 4, 6, 8, 5]);
        assert_eq!(walk.kind, CellKind::Drift);
        assert!((walk.tau_star - 0.5).abs() < 1e-12, "{}", walk.tau_star);
        // Passing at τ 1 after a drift ends the climb there all the same.
        let (_, asked) = walk_ladder_past(WalkPast::Through(1.0), |k| {
            traced(if k == 6 { &[3] } else { &[] }, &[])
        });
        assert_eq!(asked, [0, 2, 4, 6, 8, 5]);
        // With no drift through τ 1, the climb ends at its first stop past it, as ruling 3's.
        let (walk, asked) = walk_ladder_past(WalkPast::Through(1.0), fails_from(10));
        assert_eq!(asked, [0, 2, 4, 6, 8, 10, 9]);
        assert!((walk.tau_star - 2.0).abs() < 1e-12, "{}", walk.tau_star);
        // A reach ends it below τ 1, as ever.
        let (walk, asked) = walk_ladder_past(WalkPast::Through(1.0), |k| {
            if k >= 4 {
                traced(&[], &[(4, false)])
            } else {
                traced(&[], &[])
            }
        });
        assert_eq!(asked, [0, 2, 4, 3]);
        assert_eq!(walk.kind, CellKind::Reach);
    }

    #[test]
    fn atmosphere_sweep_walk_to_the_top_climbs_past_every_drift_to_a_reach_or_the_top() {
        // Fails from τ 1/8 (candidate 2): every octave is traced, then the half-octave below the
        // first drift; τ* stays 1/8's.
        let (walk, asked) = walk_ladder_past(WalkPast::Top, |k| {
            traced(if k >= 2 { &[3] } else { &[] }, &[])
        });
        assert_eq!(asked, [0, 2, 4, 6, 8, 10, 12, 14, 1]);
        assert_eq!(walk.kind, CellKind::Drift);
        assert!((walk.tau_star - 0.125).abs() < 1e-12, "{}", walk.tau_star);
        // A reach past the drift ends the climb, and leaves the kind and τ* the drift's.
        let (walk, asked) = walk_ladder_past(WalkPast::Top, |k| match k {
            0..=3 => traced(&[], &[]),
            4..=9 => traced(&[3], &[]),
            _ => PointOutcome::OverCap,
        });
        assert_eq!(asked, [0, 2, 4, 6, 8, 10, 3]);
        assert_eq!(walk.kind, CellKind::Drift);
        assert!((walk.tau_star - 0.25).abs() < 1e-12, "{}", walk.tau_star);
    }

    #[test]
    fn atmosphere_sweep_walks_on_the_must_have_cells_over_the_walk_ground_alone() {
        assert_eq!(WalkPast::of(1.0, 0.0, 0.3), WalkPast::Top);
        for (omega, g) in WALKED_THROUGH_ONE {
            assert_eq!(
                WalkPast::of(omega, g, 0.3),
                WalkPast::Through(1.0),
                "({omega}, {g})"
            );
        }
        // Mars's and Earth's, Titan's and Venus's: 14 cells besides (1, 0), each once.
        for (k, &(w, g)) in WALKED_THROUGH_ONE.iter().enumerate() {
            assert!(
                WALKED_THROUGH_ONE[k + 1..]
                    .iter()
                    .all(|&(v, a)| (v - w).abs() > 1e-9 || (a - g).abs() > 1e-9),
                "({w}, {g}) listed twice"
            );
        }
        // Not a must-have cell, nor the bright column's (1, 0) over 0.8.
        assert_eq!(WalkPast::of(0.8, 0.5, 0.3), WalkPast::No);
        assert_eq!(WalkPast::of(1.0, 0.9, 0.3), WalkPast::No);
        assert_eq!(WalkPast::of(1.0, 0.0, 0.8), WalkPast::No);
        assert_eq!(WalkPast::of(0.99, 0.3, 0.8), WalkPast::No);
        // The smoke's g 0.75 column at ω 0.9 is Mars's, its g 0 column none.
        let smoke = SweepCase::from_json(SMOKE_CASE).unwrap();
        assert_eq!(smoke.walk_past(0), WalkPast::No);
        assert_eq!(smoke.walk_past(1), WalkPast::Through(1.0));
    }

    #[test]
    fn atmosphere_sweep_walk_climbs_the_octaves_to_the_top() {
        let (walk, asked) = walk_ladder(|_| traced(&[], &[]));
        assert_eq!(asked, [0, 2, 4, 6, 8, 10, 12, 14]);
        assert_eq!(walk.kind, CellKind::Top);
        assert!((walk.tau_star - 8.0).abs() < 1e-12, "{}", walk.tau_star);
    }

    #[test]
    fn atmosphere_sweep_walk_takes_the_half_octave_below_a_drift() {
        // Fails from τ 2 (candidate 10) up; the half-octave below it, √2, passes.
        let (walk, asked) = walk_ladder(|k| {
            if k >= 10 {
                traced(&[3], &[])
            } else {
                traced(&[], &[])
            }
        });
        assert_eq!(asked, [0, 2, 4, 6, 8, 10, 9]);
        assert_eq!(walk.kind, CellKind::Drift);
        assert!((walk.tau_star - 2.0).abs() < 1e-12);
        // And where it fails too, τ* is the half-octave's.
        let (walk, _) = walk_ladder(|k| {
            if k >= 9 {
                traced(&[3], &[])
            } else {
                traced(&[], &[])
            }
        });
        assert_eq!(walk.kind, CellKind::Drift);
        assert!((walk.tau_star - 2.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn atmosphere_sweep_walk_lists_a_floor_miss_and_sets_no_tau_star_by_it() {
        // Geometry 7 fails at the floor and everywhere: a floor miss, so the column tops out.
        let (walk, _) = walk_ladder(|_| traced(&[7], &[]));
        assert_eq!(walk.kind, CellKind::Top);
        assert_eq!(walk.floor_misses, BTreeSet::from([7]));
        assert_eq!(walk.visited[0].1, PointClass::Pass);
        // Another geometry failing later is a drift all the same.
        let (walk, _) = walk_ladder(|k| {
            if k >= 6 {
                traced(&[7, 2], &[])
            } else {
                traced(&[7], &[])
            }
        });
        assert_eq!(walk.kind, CellKind::Drift);
        assert!((walk.tau_star - 0.5).abs() < 1e-12);
    }

    #[test]
    fn atmosphere_sweep_walk_lists_a_refusal_near_the_sun_at_a_vroom_point() {
        let (walk, asked) = walk_ladder(|_| traced(&[], &[(4, true)]));
        assert_eq!(walk.kind, CellKind::Top);
        assert_eq!(asked.len(), 8);
        assert_eq!(walk.listed.len(), 8);
    }

    #[test]
    fn atmosphere_sweep_walk_stops_as_reach_at_any_other_refusal() {
        let (walk, asked) = walk_ladder(|k| {
            if k >= 8 {
                traced(&[], &[(4, false)])
            } else {
                traced(&[], &[])
            }
        });
        assert_eq!(asked, [0, 2, 4, 6, 8, 7]);
        assert_eq!(walk.kind, CellKind::Reach);
        assert!(
            (walk.tau_star - 2.0_f64.sqrt().recip()).abs() < 1e-12,
            "{}",
            walk.tau_star
        );
    }

    #[test]
    fn atmosphere_sweep_walk_stops_as_reach_at_the_point_cap() {
        // Over the cap from τ 4; the half-octave below, 2√2, over it too: τ* the last passing, 2.
        let (walk, asked) = walk_ladder(|k| {
            if k >= 11 {
                PointOutcome::OverCap
            } else {
                traced(&[], &[])
            }
        });
        assert_eq!(asked, [0, 2, 4, 6, 8, 10, 12, 11]);
        assert_eq!(walk.kind, CellKind::Reach);
        assert!((walk.tau_star - 2.0).abs() < 1e-12);
        // Stopped at the floor, nothing is verified.
        let (walk, asked) = walk_ladder(|_| traced(&[], &[(1, false)]));
        assert_eq!(asked, [0]);
        assert_eq!(walk.kind, CellKind::Reach);
        assert!(walk.tau_star.total_cmp(&0.0).is_eq(), "{}", walk.tau_star);
    }

    #[test]
    fn atmosphere_sweep_floor_scan_sorts_fixed_and_ground_misses_and_finds_a_star() {
        // Over the black ground geometry 1 fails and 6 is refused; 0.02 fails nothing, 0.1 fails
        // 1 and 2, 0.5 fails 2, 3 and 6, and the walk's floor at 0.3 fails 2 and 4.
        // A point over the cap at 0.2 sorts nothing, nor one over the black ground's own 0.
        let black = traced(&[1], &[(6, false)]);
        let (clear, at_tenth, at_half, floor, again) = (
            traced(&[], &[]),
            traced(&[1, 2], &[]),
            traced(&[2, 3, 6], &[(5, false)]),
            traced(&[2, 4], &[]),
            traced(&[7], &[]),
        );
        let sort = sort_floor(
            &black,
            [
                (0.02, &clear),
                (0.1, &at_tenth),
                (0.2, &PointOutcome::OverCap),
                (0.5, &at_half),
                (0.3, &floor),
                (0.0, &again),
            ],
        );
        assert_eq!(sort.fixed, BTreeSet::from([1]));
        assert_eq!(
            sort.ground,
            BTreeMap::from([(2, 0.1), (3, 0.5), (4, 0.3)]),
            "each at the least ground it fails over"
        );
        assert_eq!(sort.unsorted, BTreeSet::from([6]));
        let star = sort.albedo_star();
        assert!(star.total_cmp(&0.1).is_eq(), "A* {star}");
        // No ground miss: A* is 1.
        let none = sort_floor(&black, [(0.3, &traced(&[1], &[]))]);
        assert_eq!(none.ground, BTreeMap::new());
        assert_eq!(none.unsorted, BTreeSet::new());
        let star = none.albedo_star();
        assert!(star.total_cmp(&1.0).is_eq(), "A* {star}");
        // A black ground not traced sorts every miss as unsorted, and none as fixed.
        let over = sort_floor(&PointOutcome::OverCap, [(0.3, &floor)]);
        assert_eq!(over.unsorted, BTreeSet::from([2, 4]));
        assert_eq!(over.fixed, BTreeSet::new());
        let star = over.albedo_star();
        assert!(star.total_cmp(&1.0).is_eq(), "A* {star}");
    }

    #[test]
    fn atmosphere_sweep_counts_take_whole_blocks_from_n_p_up() {
        assert_eq!(blocks_up(1.0, 2048), 2048);
        assert_eq!(blocks_up(2049.0, 2048), 3072);
        assert_eq!(blocks_up(1e30, 2048), MAX_SAMPLES);
        assert_eq!(blocks_down(5000.0, 2048), 4096);
        assert_eq!(blocks_down(100.0, 2048), 2048);
    }

    /// The ruled settings with a geometry cap of `cap_cpu_h`.
    fn capped_at(cap_cpu_h: f64) -> SweepSettings {
        SweepSettings::new(
            POINT_CAP_CPU_H,
            PositiveFigure::new(cap_cpu_h).unwrap(),
            figure(8.79),
            figure(4.0),
        )
    }

    #[test]
    fn atmosphere_sweep_counts_follow_t12e_rule_and_the_geometry_cap() {
        let ruled = capped_at(4.0);
        // Four times its target: 2,048 × 16 × 1.15 = 37,683.2, in 37 blocks.
        assert_eq!(
            planned_count(2048, 0.04, 0.01, 0.0, &ruled),
            (37_888, false)
        );
        // Never under 8,192 ("#112's floor refusals"), nor from an exact pilot.
        assert_eq!(MIN_FINAL_SAMPLES, 8192);
        assert_eq!(planned_count(2048, 0.01, 0.01, 0.0, &ruled), (8192, false));
        assert_eq!(planned_count(2048, 0.001, 0.01, 0.0, &ruled), (8192, false));
        assert_eq!(planned_count(2048, 0.0, 0.01, 0.0, &ruled), (8192, false));
        // 100 µs a sample under a cap of 10⁻² CPU-h, 36 s: 360,000 samples, held at 351 blocks.
        let tenth = capped_at(1e-2);
        assert_eq!(cap_count(2048, 100.0, &tenth), 359_424);
        assert_eq!(
            planned_count(2048, 0.4, 0.01, 100.0, &tenth),
            (359_424, true)
        );
        // A cap under 8,192 holds the count at 8,192.
        let tiny = capped_at(1e-6);
        assert_eq!(cap_count(2048, 100.0, &tiny), 8192);
        assert_eq!(planned_count(2048, 0.04, 0.01, 100.0, &tiny), (8192, true));
        // No vertex counted: no cap below the tracer's most.
        assert_eq!(cap_count(2048, 0.0, &ruled), MAX_SAMPLES);
        // The smoke's: its pilot's 1,024 at least, and a margin of 0.25, a quarter's count of
        // σ at its target: 1,024 × 16 × 0.25 = 4,096.
        let smoke = SweepSettings::SMOKE;
        assert_eq!(planned_count(1024, 0.01, 0.01, 0.0, &smoke), (1024, false));
        assert_eq!(planned_count(1024, 0.04, 0.01, 0.0, &smoke), (4096, false));
    }

    /// The moments of a run of `n` samples, each 0.
    fn run_of(n: u64) -> Moments {
        let mut moments = Moments::default();
        for _ in 0..n {
            moments.push(0.0);
        }
        moments
    }

    #[test]
    fn atmosphere_sweep_top_ups_double_to_the_cap_within_the_point_budget() {
        let first = Carried::new(
            crate::atmosphere::GeometryMoments {
                whole: run_of(8192),
                quarter: run_of(2048),
            },
            0,
        );
        let ample = 1e12;
        // Doubled, its first quarter carried on from 2,048 to 4,096: 8,192 + 2,048 samples at
        // 1 µs.
        let Step::Carry {
            end,
            quarter,
            cost_us,
        } = first.next_step(1 << 20, 1.0, 0.0, ample)
        else {
            panic!("a geometry below its cap is carried on");
        };
        assert_eq!((end, quarter.map(|q| q.samples())), (16_384, Some(2048)));
        assert!((cost_us - 10_240.0).abs() < 1e-9, "{cost_us}");
        // Once it holds 16,384 and its 4,096, the next quarter is a count it already holds.
        let mut on = first.clone();
        on.whole = run_of(16_384);
        on.prefixes.insert(16_384, on.whole);
        on.prefixes.insert(4096, run_of(4096));
        assert_eq!(on.quarter().samples(), 4096);
        assert_eq!(
            on.next_step(1 << 20, 1.0, 0.0, ample),
            Step::Carry {
                end: 32_768,
                quarter: None,
                cost_us: 16_384.0,
            }
        );
        // Short of double by its cap: raised to the cap, its quarter from the longest run below.
        let Step::Carry {
            end,
            quarter,
            cost_us,
        } = first.next_step(10_240, 1.0, 0.0, ample)
        else {
            panic!("a geometry below its cap is carried on");
        };
        assert_eq!((end, quarter.map(|q| q.samples())), (10_240, Some(2048)));
        assert!((cost_us - 2560.0).abs() < 1e-9, "{cost_us}");
        // From 9,216, whose quarter of 2,304 ends inside a block: the new quarter, 4,608, is carried
        // on from the empty run, so that it folds as a fresh trace's.
        let odd = Carried::new(
            crate::atmosphere::GeometryMoments {
                whole: run_of(9216),
                quarter: run_of(2304),
            },
            0,
        );
        let Step::Carry {
            end,
            quarter,
            cost_us,
        } = odd.next_step(1 << 20, 1.0, 0.0, ample)
        else {
            panic!("a geometry below its cap is carried on");
        };
        assert_eq!((end, quarter.map(|q| q.samples())), (18_432, Some(0)));
        assert!((cost_us - 13_824.0).abs() < 1e-9, "{cost_us}");
        // At its cap it stops there; a step past the point's budget stops at the point's, and one
        // to the budget is taken.
        assert_eq!(
            first.next_step(8192, 1.0, 0.0, ample),
            Step::Stop(TopUpCap::Geometry)
        );
        assert_eq!(
            first.next_step(1 << 20, 1.0, 5000.0, 15_239.0),
            Step::Stop(TopUpCap::Point)
        );
        assert!(matches!(
            first.next_step(1 << 20, 1.0, 5000.0, 15_240.0),
            Step::Carry { end: 16_384, .. }
        ));
    }

    #[test]
    fn atmosphere_sweep_projects_a_sample_at_one_vertex_at_least() {
        assert!((sample_cost_us(4096, 1024, 2.5) - 10.0).abs() < 1e-12);
        // A pilot that counted no vertex: the forced first collision's one, so its cap is bounded.
        assert!((sample_cost_us(0, 1024, 2.5) - 2.5).abs() < 1e-12);
        let ruled = capped_at(4.0);
        assert!(cap_count(2048, sample_cost_us(0, 2048, 8.79), &ruled) < MAX_SAMPLES);
    }

    /// The smoke's sweep as JSON, to be changed.
    fn smoke_json() -> Value {
        serde_json::from_str(SMOKE_CASE).unwrap()
    }

    /// The field a changed sweep is refused at.
    fn refused_field(value: &Value) -> String {
        match SweepCase::from_json(&value.to_string()) {
            Err(ReadSweepError::Invalid { field, .. }) => field,
            Err(ReadSweepError::Point { column, source }) => {
                format!("point of columns[{column}]: {source}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn atmosphere_sweep_case_names_the_field_it_refuses() {
        assert!(SweepCase::from_json(SMOKE_CASE).is_ok());
        let mut v = smoke_json();
        v["format"] = json!(2);
        assert!(matches!(
            SweepCase::from_json(&v.to_string()),
            Err(ReadSweepError::UnsupportedFormat(2))
        ));
        let mut v = smoke_json();
        v["stray"] = json!(1);
        assert!(matches!(
            SweepCase::from_json(&v.to_string()),
            Err(ReadSweepError::Json(_))
        ));
        let mut v = smoke_json();
        v["extinctionPerM"][2] = json!(v["extinctionPerM"][2].as_f64().unwrap() * 1.001);
        assert_eq!(refused_field(&v), "extinctionPerM[2]");
        let mut v = smoke_json();
        v["tauRoles"][0] = json!("octave");
        assert_eq!(refused_field(&v), "tauRoles");
        // A pilot count off the blocks, from which the counts would leave them.
        let mut v = smoke_json();
        v["pilotSamples"] = json!(1000);
        assert_eq!(refused_field(&v), "pilotSamples");
        let mut v = smoke_json();
        v["columns"][1]["twin"][0][0] = json!(-1.0);
        assert_eq!(refused_field(&v), "columns[1].twin[0]");
        let mut v = smoke_json();
        v["columns"][1]["points"][0] = v["columns"][0]["points"][0].clone();
        assert_eq!(refused_field(&v), "columns[1].points");
        let mut v = smoke_json();
        v["columns"][0]["phase"] = v["columns"][1]["phase"].clone();
        assert_eq!(refused_field(&v), "columns[0].phase.a1");
        // The scan: led by the black ground, its points unique with the walk's, a row a ground.
        let mut v = smoke_json();
        v["scanGroundAlbedos"][0] = json!(0.01);
        assert_eq!(refused_field(&v), "scanGroundAlbedos");
        let mut v = smoke_json();
        v["scanGroundAlbedos"][2] = v["scanGroundAlbedos"][1].clone();
        assert_eq!(refused_field(&v), "scanGroundAlbedos");
        let mut v = smoke_json();
        v["columns"][0]["scanPoints"][3] = v["columns"][0]["points"][0].clone();
        assert_eq!(refused_field(&v), "columns[0].scanPoints");
        let mut v = smoke_json();
        v["columns"][1]["scanTwin"].as_array_mut().unwrap().pop();
        assert_eq!(refused_field(&v), "columns[1].scanTwin");
        let mut v = smoke_json();
        v["columns"][1]["scanTwin"][2][1] = json!(-1.0);
        assert_eq!(refused_field(&v), "columns[1].scanTwin[2]");
        // A table 1% off its integral: the tracer's own check refuses it.
        let mut v = smoke_json();
        let scaled: Vec<f64> = v["columns"][1]["phase"]["a1"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() * 1.01)
            .collect();
        v["columns"][1]["phase"]["a1"] = json!(scaled);
        let field = refused_field(&v);
        assert!(
            field.starts_with("point of columns[1]") && field.contains("terms[0].phase"),
            "{field}"
        );
    }

    #[test]
    fn atmosphere_sweep_point_expands_to_its_case() {
        let case = SweepCase::from_json(SMOKE_CASE).unwrap();
        let f = &case.file;
        let (column, candidate) = (1, 3);
        let samples: Vec<u64> = (1..=4).map(|k| 1000 * k).collect();
        let json = case.point_json(column, Spot::Walk(candidate), 77, &samples, Some(5000));
        let term = &json["terms"][0];
        let scattering = term["scattering"][0].as_f64().unwrap();
        let absorption = term["absorption"][0].as_f64().unwrap();
        let beta = f.extinction_per_m[candidate];
        let omega = f.single_scattering_albedo;
        assert!(scattering.total_cmp(&(omega * beta)).is_eq());
        assert!(absorption.total_cmp(&((1.0 - omega) * beta)).is_eq());
        // β₀ gives τ: β₀ H (1 − e^(−top ÷ H)).
        let t = &f.template;
        let tau = beta * t.scale_height_m * (1.0 - math::exp(-t.top_height_m / t.scale_height_m));
        assert!(
            (tau / f.tau[candidate] - 1.0).abs() < 1e-12,
            "{tau} against {}",
            f.tau[candidate]
        );
        assert_eq!(term["phase"]["kind"], "tabulated");
        assert_eq!(
            term["phase"]["asymmetry"][0],
            json!(f.columns[column].asymmetry)
        );
        assert_eq!(
            term["phase"]["u"].as_array().unwrap().len(),
            f.phase_u.len()
        );
        assert_eq!(json["seed"], 77);
        assert_eq!(json["geometries"][2]["samples"], 3000);
        assert_eq!(json["aggregates"][0]["samples"], 5000);
        assert_eq!(json["groundAlbedo"][0], json!(t.ground_albedo));
        let rayleigh = case.point_json(0, Spot::Walk(0), 1, &samples, None);
        assert_eq!(rayleigh["terms"][0]["phase"]["kind"], "rayleigh");
        assert_eq!(rayleigh["aggregates"], json!([]));
        assert!(
            case.point_case(column, Spot::Walk(candidate), 77, &samples, Some(5000))
                .is_ok()
        );
        // A scan point: the floor's β₀ over the scan's ground, named by its own index.
        let scan = Spot::Scan(4);
        let json = case.point_json(column, scan, 78, &samples, Some(5000));
        let scattering = json["terms"][0]["scattering"][0].as_f64().unwrap();
        assert!(
            scattering
                .total_cmp(&(omega * f.extinction_per_m[0]))
                .is_eq()
        );
        assert_eq!(json["groundAlbedo"][0], json!(f.scan_ground_albedos[4]));
        let index = f.columns[column].scan_points[4];
        assert_eq!(json["name"], format!("{}-p{index}", f.name));
        assert_eq!(case.place(index), Some((column, scan)));
        assert_eq!(case.place(f.columns[0].points[2]), Some((0, Spot::Walk(2))));
        assert_eq!(case.place(u64::MAX), None);
    }

    #[test]
    fn atmosphere_sweep_cases_run_vroom_exactly_from_g_0_65() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/hyperion/src/renderer/src/view/atmosphere/reference");
        let mut columns = 0;
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if !(name.starts_with("ms-sweep-") && name.ends_with(".case.json")) {
                continue;
            }
            let case = SweepCase::read(&path).unwrap();
            for (column, &vroom) in case.file.columns.iter().zip(&case.vroom) {
                assert_eq!(
                    vroom,
                    column.asymmetry >= 0.65,
                    "{name}, g {}",
                    column.asymmetry
                );
                columns += 1;
            }
        }
        assert_eq!(columns, 7 * 7 + 1, "the eight files' columns");
    }

    /// A committed sweep case, read from the client's tree.
    fn committed(name: &str) -> SweepCase {
        SweepCase::read(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/hyperion/src/renderer/src/view/atmosphere/reference")
                .join(format!("{name}.case.json")),
        )
        .unwrap()
    }

    #[test]
    fn atmosphere_sweep_finds_the_views_within_15_degrees_of_their_sun_inclusively() {
        // 75° from the zenith under a sun at 60° is exactly 15° from it; under one at 80°, 5°.
        let case = committed("ms-sweep-w1000");
        let near: Vec<&str> = case
            .file
            .geometries
            .iter()
            .zip(&case.near_sun)
            .filter(|&(_, &near)| near)
            .map(|(g, _)| g.name.as_str())
            .collect();
        assert_eq!(
            near,
            [
                "ground-s60-v75-a0",
                "ground-s80-v75-a0",
                "ground-s80-v85-a0"
            ]
        );
    }

    #[test]
    fn atmosphere_sweep_case_keeps_its_name_to_a_portable_path_part() {
        for name in ["../escape", "/root", "a\\b", "Upper", "a:b", ""] {
            let mut v = smoke_json();
            v["name"] = json!(name);
            assert_eq!(refused_field(&v), "name", "{name:?}");
        }
        // A checkout that writes CRLF hashes as the committed LF file.
        let crlf = SMOKE_CASE.replace('\n', "\r\n");
        assert_eq!(
            SweepCase::from_json(&crlf).unwrap().sha256(),
            SweepCase::from_json(SMOKE_CASE).unwrap().sha256()
        );
    }

    /// A point record of no traces, for the work directory's tests.
    fn bare_record(index: u64, tau: f64) -> PointRecord {
        PointRecord {
            index,
            tau,
            ground_albedo: 0.3,
            vroom: false,
            pilot: PilotRecord {
                seed: 1,
                samples: 1024,
                radiance: vec![0.5],
                standard_error: vec![0.01],
                vertices: vec![2048],
                aggregate: None,
                planned_samples: vec![4096],
                planned_aggregate_samples: None,
                capped: Vec::new(),
                projected_cpu_h: 0.25,
                cpu_s: None,
                wall_s: 1.5,
            },
            traced: None,
        }
    }

    #[test]
    fn atmosphere_sweep_work_directory_keeps_a_point_and_refuses_another() {
        let root = tempfile::tempdir().unwrap();
        let work = WorkDir {
            dir: root.path().join("case-key"),
        };
        let record = bare_record(7, 0.5);
        assert_eq!(work.load(7, 0.5, 0.3).unwrap(), None, "nothing kept yet");
        work.save(&record).unwrap();
        assert_eq!(work.load(7, 0.5, 0.3).unwrap(), Some(record));
        let kept: Vec<_> = std::fs::read_dir(&work.dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            kept,
            ["point-0007.json"],
            "the point whole, no partial file"
        );
        // Another τ's point, another ground's, and another index's file under this one's name.
        assert!(matches!(
            work.load(7, 0.25, 0.3),
            Err(RunSweepError::Stale { .. })
        ));
        assert!(matches!(
            work.load(7, 0.5, 0.2),
            Err(RunSweepError::Stale { .. })
        ));
        std::fs::copy(work.path(7), work.path(9)).unwrap();
        assert!(matches!(
            work.load(9, 0.5, 0.3),
            Err(RunSweepError::Stale { .. })
        ));
        // A file cut short.
        std::fs::write(work.path(7), "{\"index\": 7,").unwrap();
        assert!(matches!(
            work.load(7, 0.5, 0.3),
            Err(RunSweepError::CorruptPoint { .. })
        ));
    }
}
