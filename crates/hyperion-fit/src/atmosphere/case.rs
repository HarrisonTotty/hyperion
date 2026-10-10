//! The case format: a spectral medium as the client writes it, its shells, and its geometry set
//! (plan R08, R08.T12.a; the client's writer is R08.T12.b's).
//!
//! A case is one JSON object, read with `serde_json` and validated once, in
//! [`AtmosphereCase::from_json`]. Field names are the client's camel case, and the medium's terms
//! are the client's `MediumTerm`s (`view/atmosphere/medium.ts`) with every per-channel array
//! widened to one entry per traced wavelength:
//!
//! ```text
//! {
//!   "format": 1,
//!   "name": "earth",
//!   "seed": 0,                                   // optional, 0 by default
//!   "wavelengthsNm": [680, 550, 440],
//!   "shells": { "kind": "sphere", "radiusM": 6371000 },  // or a level spheroid, below
//!   "topHeightM": 100000,
//!   "groundAlbedo": [0.3, 0.3, 0.3],             // Lambertian, per wavelength
//!   "terms": [{
//!     "name": "rayleigh",
//!     "density": { "kind": "exponential", "scaleHeightM": 8000 },
//!     "scattering": [...], "absorption": [...],  // m⁻¹ at relative density 1, per wavelength
//!     "phase": { "kind": "rayleigh", "depolarisation": [...] }
//!   }],
//!   "suns": [{ "name": "primary", "irradiance": [...] }],
//!   "detectorHalfAngleDeg": 0.5,
//!   "geometries": [{
//!     "name": "ground-60-90-30",
//!     "observer": { "heightM": 0, "latitudeDeg": 0 },
//!     "view": { "zenithDeg": 60, "azimuthDeg": 90 },
//!     "sunDirections": [{ "zenithDeg": 30, "azimuthDeg": 0 }],
//!     "maxDistanceM": 10000,                     // optional: aerial perspective to a black target
//!     "detectorHalfAngleDeg": 0.5                // optional: the case's by default
//!   }],
//!   "aggregates": [{ "name": "noon", "latitudeDeg": 0, "sunDirections": [...] }]
//! }
//! ```
//!
//! - **Shells.** `sphere`, concentric spheres about a ground of radius `radiusM`, or `spheroid`,
//!   a body's level spheroid as the client's `bodyGravity` gives it (`view/atmosphere/oblate.ts`,
//!   R08.T3.d), `{ "kind": "spheroid", "equatorialRadiusM": a, "polarRadiusM": c, "gmM3S2": GM,
//!   "angularVelocityRadS": ω, "gravityOffsetMS2": 0 }`: its `LevelSpheroid`, ω being Design note
//!   17's `ω_fig`, and its `gravityOffsetMS2`, optional and 0 by default; 0 < c ≤ a, ω below
//!   breakup, and the gravity positive at the equator and the poles. On a spheroid every profile
//!   is read at the gravity-scaled height h\* = h g(φ) ÷ `g_ref`, and `topHeightM` and the
//!   profiles' heights are gravity-scaled heights (Design note 17; the `spheroid` module); an
//!   observer's height is its geodetic height, and every latitude is geodetic.
//! - **Density profiles** are the client's: `exponential` (e^(−h ÷ H)), `tent` (Bruneton 2017's
//!   ozone layer), and `tabulated`, linear in relative density between nodes and constant beyond
//!   the first and last, the rule a texture sampled with linear filtering and clamped edges
//!   follows. Heights are above the ground; a negative height reads as the ground.
//! - **Phase functions**: `rayleigh` with the depolarisation factor ρ per wavelength (Hansen and
//!   Travis 1974, eq. 2.15), `cornette-shanks` with its parameter g (Cornette and Shanks 1992,
//!   eq. 8; R05's aerosol), `isotropic`, and `none` for an absorbing-only term, whose scattering
//!   must be zero.
//! - **Directions** are given in the observer's local frame: zenith angle from the local vertical,
//!   azimuth from local north towards east. A sun's direction is the direction towards it, the
//!   same at every point, since the sun is at infinity. The relative azimuth of the benchmark
//!   tables is the view's azimuth less the sun's.
//! - **Irradiance** is each sun's spectral irradiance on a surface normal to its beam at the top
//!   of the atmosphere, in any unit; radiances come out in that unit per steradian.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::spheroid::{BuildLevelSpheroidError, LevelSpheroid};

/// The case format this tracer reads.
pub const CASE_FORMAT: u32 = 1;

/// The widest detector cone a case may ask for, as a half-angle, degrees.
///
/// Design note 10's cones are 0.5°–1°, and a cone much wider than the structure it averages is a
/// mistake in the case; 0° is a pencil beam, for the analytic tests and the point benchmarks.
pub const MAX_DETECTOR_HALF_ANGLE_DEG: f64 = 10.0;

/// The most wavelengths a case may trace: each sample's random stream is keyed by its wavelength
/// in 16 bits.
pub const MAX_WAVELENGTHS: usize = 1 << 16;
/// The body's shells: the figure the medium is layered on (Design note 17).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Shells {
    /// Concentric spheres about a ground of radius `radius_m`, m.
    Sphere {
        /// The ground's radius, m: for a body the client draws on its per-planet tables, R05's
        /// `tableRadiusM`, (2a + c) ÷ 3.
        radius_m: f64,
    },
    /// A body's level spheroid, the medium read at the gravity-scaled height h·g(φ) ÷ `g_ref`.
    ///
    /// The ellipsoid of revolution (a, a, c) about the z axis that carries GM and turns at ω, with
    /// g(φ) = γ(φ) + the added gravity, γ Somigliana's normal gravity at geodetic latitude φ, and
    /// `g_ref` = √(gₑ gₚ), free paths by delta tracking (R08.T12.d). The fields are the client's
    /// `LevelSpheroid`'s and `BodyGravity.gravityOffsetMS2`.
    Spheroid {
        /// a, m, positive.
        equatorial_radius_m: f64,
        /// c, m, in (0, a].
        polar_radius_m: f64,
        /// GM, m³ s⁻², positive.
        gm_m3_s2: f64,
        /// ω, rad s⁻¹, below breakup (γₑ > 0); only ω² enters. Design note 17's `ω_fig`, the spin
        /// under which the figure is level, which for a `rotational_and_tidal` or `capped` figure
        /// is not the body's own. Written `angularVelocityRadS`, as the client's `LevelSpheroid`
        /// has it.
        #[serde(rename = "angularVelocityRadS")]
        omega_rad_s: f64,
        /// The gravity added to γ(φ) at every latitude, m s⁻²: ω²R on a `rotational_and_tidal`
        /// figure (Design note 17), 0 by default.
        #[serde(default)]
        gravity_offset_m_s2: f64,
    },
}

/// A case's figure, validated: what its shells are built over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Figure {
    /// A sphere about a ground of radius `radius_m`, m.
    Sphere { radius_m: f64 },
    /// A level spheroid.
    Spheroid(LevelSpheroid),
}

/// A term's relative density as a function of height above the ground (the client's
/// `DensityProfile`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum DensityProfile {
    /// e^(−h ÷ H), 1 at the ground.
    Exponential { scale_height_m: f64 },
    /// Zero below `bottom_m` and above `top_m`, rising linearly to 1 at `peak_m` and falling back.
    Tent {
        bottom_m: f64,
        peak_m: f64,
        top_m: f64,
    },
    /// Linear in relative density between the nodes, constant beyond the first and the last.
    Tabulated {
        altitudes_m: Vec<f64>,
        relative: Vec<f64>,
    },
}

/// A term's phase function (the client's `PhaseFunction`, with `isotropic` added).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum PhaseFunction {
    /// Rayleigh scattering by molecules of depolarisation factor ρ, per wavelength.
    Rayleigh { depolarisation: Vec<f64> },
    /// Cornette and Shanks's form with parameter g, which is not its mean cosine.
    CornetteShanks { asymmetry: f64 },
    /// The same in every direction, and fully depolarising. (Braces, not a unit variant, so that
    /// `deny_unknown_fields` refuses a stray key beside the tag.)
    Isotropic {},
    /// An absorbing-only term.
    None {},
}

/// One constituent of the medium: a gas, an aerosol or an absorbing layer.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Term {
    pub(crate) name: String,
    pub(crate) density: DensityProfile,
    /// The scattering coefficient at relative density 1, per wavelength, m⁻¹.
    #[serde(rename = "scattering")]
    pub(crate) scattering_per_m: Vec<f64>,
    /// The absorption coefficient at relative density 1, per wavelength, m⁻¹.
    #[serde(rename = "absorption")]
    pub(crate) absorption_per_m: Vec<f64>,
    pub(crate) phase: PhaseFunction,
}

/// A sun's spectrum.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Sun {
    pub(crate) name: String,
    /// Irradiance normal to the beam at the top of the atmosphere, per wavelength.
    pub(crate) irradiance: Vec<f64>,
}

/// A direction in the local frame: zenith angle from the vertical and azimuth from north towards
/// east, degrees.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LocalDirection {
    pub(crate) zenith_deg: f64,
    pub(crate) azimuth_deg: f64,
}

/// Where an observer stands.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Observer {
    /// Height above the ground along its normal, m: 0 on it, above the top of the atmosphere in
    /// orbit; on a spheroid the geodetic height, not the gravity-scaled one.
    pub(crate) height_m: f64,
    /// Geodetic latitude, degrees; on a sphere it changes nothing but the frame.
    #[serde(default)]
    pub(crate) latitude_deg: f64,
}

/// One radiance to trace: an observer, a view direction and where each sun is.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Geometry {
    pub(crate) name: String,
    pub(crate) observer: Observer,
    /// Where the detector looks.
    pub(crate) view: LocalDirection,
    /// Where each of the case's suns is, in its order.
    pub(crate) sun_directions: Vec<LocalDirection>,
    /// The view ray ends here at a black target, for aerial perspective, m.
    #[serde(default)]
    pub(crate) max_distance_m: Option<f64>,
    /// This geometry's detector cone, if not the case's, as a half-angle, degrees.
    #[serde(default)]
    pub(crate) detector_half_angle_deg: Option<f64>,
}

/// One pair of flux aggregates: the downwelling flux at a ground point and the upwelling flux at
/// the top above it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Aggregate {
    pub(crate) name: String,
    /// Geodetic latitude, degrees.
    #[serde(default)]
    pub(crate) latitude_deg: f64,
    pub(crate) sun_directions: Vec<LocalDirection>,
}

/// The case file as written.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CaseFile {
    format: u32,
    name: String,
    #[serde(default)]
    seed: u64,
    wavelengths_nm: Vec<f64>,
    shells: Shells,
    top_height_m: f64,
    ground_albedo: Vec<f64>,
    terms: Vec<Term>,
    suns: Vec<Sun>,
    detector_half_angle_deg: f64,
    geometries: Vec<Geometry>,
    #[serde(default)]
    aggregates: Vec<Aggregate>,
}

/// An atmosphere to trace: a spectral medium, its [`Shells`], and its geometry set (module
/// documentation).
///
/// It is made only by [`from_json`](Self::from_json) or [`read`](Self::read), which validate it,
/// so the tracer can take its contents as given.
#[derive(Debug, Clone, PartialEq)]
pub struct AtmosphereCase {
    file: CaseFile,
    figure: Figure,
}

/// An atmosphere case could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ReadCaseError {
    /// The file could not be read.
    #[error("cannot read the atmosphere case {}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// Why.
        #[source]
        source: std::io::Error,
    },
    /// The text is not JSON of the case format.
    #[error("the atmosphere case is not JSON of the case format")]
    Json(#[source] serde_json::Error),
    /// The case is of a format this tracer does not read.
    #[error("the atmosphere case's format is {0}, and this tracer reads format {CASE_FORMAT}")]
    UnsupportedFormat(u32),
    /// A value is out of its range or an array has the wrong length.
    #[error("the atmosphere case's {field} {reason}")]
    Invalid {
        /// Where, as a path into the case: `terms[1].scattering[2]`.
        field: String,
        /// What is wrong with it.
        reason: &'static str,
    },
}

impl AtmosphereCase {
    /// Reads and validates a case from its JSON text.
    ///
    /// # Errors
    ///
    /// [`ReadCaseError::Json`] for text that is not JSON of the case format, unknown fields
    /// included; [`ReadCaseError::UnsupportedFormat`] for another format; and
    /// [`ReadCaseError::Invalid`] for a value out of its range or an array whose length is not
    /// the number of wavelengths or of suns.
    pub fn from_json(text: &str) -> Result<Self, ReadCaseError> {
        let file: CaseFile = serde_json::from_str(text).map_err(ReadCaseError::Json)?;
        let figure = validate(&file)?;
        Ok(Self { file, figure })
    }

    /// Reads and validates the case in the file at `path`.
    ///
    /// # Errors
    ///
    /// [`ReadCaseError::Read`] if the file cannot be read, and the errors of
    /// [`from_json`](Self::from_json).
    pub fn read(path: &Path) -> Result<Self, ReadCaseError> {
        let text = std::fs::read_to_string(path).map_err(|source| ReadCaseError::Read {
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

    /// The seed of the case's random draws.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.file.seed
    }

    /// The traced wavelengths, nm.
    #[must_use]
    pub fn wavelengths_nm(&self) -> &[f64] {
        &self.file.wavelengths_nm
    }

    /// The body's shells.
    #[must_use]
    pub fn shells(&self) -> &Shells {
        &self.file.shells
    }

    /// The figure the shells are built over, validated.
    #[must_use]
    pub(crate) fn figure(&self) -> &Figure {
        &self.figure
    }

    /// The top of the atmosphere above the ground, m: on a spheroid a gravity-scaled height.
    #[must_use]
    pub fn top_height_m(&self) -> f64 {
        self.file.top_height_m
    }

    /// How many geometries the case traces.
    #[must_use]
    pub fn geometry_count(&self) -> usize {
        self.file.geometries.len()
    }

    /// How many pairs of flux aggregates the case traces.
    #[must_use]
    pub fn aggregate_count(&self) -> usize {
        self.file.aggregates.len()
    }

    #[must_use]
    pub(crate) fn ground_albedo(&self) -> &[f64] {
        &self.file.ground_albedo
    }

    #[must_use]
    pub(crate) fn terms(&self) -> &[Term] {
        &self.file.terms
    }

    #[must_use]
    pub(crate) fn suns(&self) -> &[Sun] {
        &self.file.suns
    }

    #[must_use]
    pub(crate) fn geometries(&self) -> &[Geometry] {
        &self.file.geometries
    }

    #[must_use]
    pub(crate) fn aggregates(&self) -> &[Aggregate] {
        &self.file.aggregates
    }

    /// A geometry's detector cone, as a half-angle, degrees.
    #[must_use]
    pub(crate) fn detector_half_angle_deg(&self, geometry: &Geometry) -> f64 {
        geometry
            .detector_half_angle_deg
            .unwrap_or(self.file.detector_half_angle_deg)
    }
}

/// An [`ReadCaseError::Invalid`] at `field`.
fn invalid(field: impl Into<String>, reason: &'static str) -> ReadCaseError {
    ReadCaseError::Invalid {
        field: field.into(),
        reason,
    }
}

/// Fails unless `ok`.
fn require(
    ok: bool,
    field: impl FnOnce() -> String,
    reason: &'static str,
) -> Result<(), ReadCaseError> {
    if ok {
        Ok(())
    } else {
        Err(invalid(field(), reason))
    }
}

/// Fails unless `values` has one finite entry per wavelength, each in `range`.
fn per_wavelength(
    values: &[f64],
    count: usize,
    field: &str,
    range: impl Fn(f64) -> bool,
    reason: &'static str,
) -> Result<(), ReadCaseError> {
    require(
        values.len() == count,
        || field.to_owned(),
        "must have one entry per wavelength",
    )?;
    for (i, &value) in values.iter().enumerate() {
        require(
            value.is_finite() && range(value),
            || format!("{field}[{i}]"),
            reason,
        )?;
    }
    Ok(())
}

/// Fails unless `direction` is a direction: zenith in [0°, 180°], azimuth finite.
fn local_direction(direction: &LocalDirection, field: &str) -> Result<(), ReadCaseError> {
    require(
        direction.zenith_deg.is_finite() && (0.0..=180.0).contains(&direction.zenith_deg),
        || format!("{field}.zenithDeg"),
        "must lie in [0, 180]",
    )?;
    require(
        direction.azimuth_deg.is_finite(),
        || format!("{field}.azimuthDeg"),
        "must be finite",
    )
}

/// Fails unless `directions` gives one direction for each of `suns` suns.
fn sun_directions(
    directions: &[LocalDirection],
    suns: usize,
    field: &str,
) -> Result<(), ReadCaseError> {
    require(
        directions.len() == suns,
        || format!("{field}.sunDirections"),
        "must have one direction per sun",
    )?;
    for (i, direction) in directions.iter().enumerate() {
        local_direction(direction, &format!("{field}.sunDirections[{i}]"))?;
    }
    Ok(())
}

/// Fails unless `latitude_deg` lies in [−90°, 90°].
fn latitude(latitude_deg: f64, field: &str) -> Result<(), ReadCaseError> {
    require(
        latitude_deg.is_finite() && (-90.0..=90.0).contains(&latitude_deg),
        || format!("{field}.latitudeDeg"),
        "must lie in [-90, 90]",
    )
}

/// Fails unless `half_angle_deg` is a detector cone this tracer takes.
fn cone(half_angle_deg: f64, field: impl FnOnce() -> String) -> Result<(), ReadCaseError> {
    require(
        half_angle_deg.is_finite() && (0.0..=MAX_DETECTOR_HALF_ANGLE_DEG).contains(&half_angle_deg),
        field,
        "must lie in [0, 10] degrees",
    )
}

/// The figure of `shells`, if it is one the tracer walks.
fn shells(shells: &Shells) -> Result<Figure, ReadCaseError> {
    use BuildLevelSpheroidError as E;
    match *shells {
        Shells::Sphere { radius_m } => {
            require(
                radius_m.is_finite() && radius_m > 0.0,
                || "shells.radiusM".to_owned(),
                "must be positive",
            )?;
            Ok(Figure::Sphere { radius_m })
        }
        Shells::Spheroid {
            equatorial_radius_m,
            polar_radius_m,
            gm_m3_s2,
            omega_rad_s,
            gravity_offset_m_s2,
        } => LevelSpheroid::new(
            equatorial_radius_m,
            polar_radius_m,
            gm_m3_s2,
            omega_rad_s,
            gravity_offset_m_s2,
        )
        .map(Figure::Spheroid)
        .map_err(|error| {
            let (field, reason) = match error {
                E::Radii { .. }
                    if !(equatorial_radius_m.is_finite() && equatorial_radius_m > 0.0) =>
                {
                    ("equatorialRadiusM", "must be positive")
                }
                E::Radii { .. } => (
                    "polarRadiusM",
                    "must be positive and at most the equatorial radius",
                ),
                E::GravitationalParameter(_) => ("gmM3S2", "must be positive"),
                E::Spin(_) => ("angularVelocityRadS", "must be finite"),
                E::PastBreakup(_) => ("angularVelocityRadS", "spins the equator past breakup"),
                E::GravityOffset(_) => (
                    "gravityOffsetMS2",
                    "must be finite and leave the gravity positive at the equator and the poles",
                ),
                E::Gravity => (
                    "gmM3S2",
                    "gives a normal gravity that is not finite and positive",
                ),
            };
            invalid(format!("shells.{field}"), reason)
        }),
    }
}

/// Fails unless the profile is well formed.
fn density(profile: &DensityProfile, field: &str) -> Result<(), ReadCaseError> {
    match profile {
        DensityProfile::Exponential { scale_height_m } => require(
            scale_height_m.is_finite() && *scale_height_m > 0.0,
            || format!("{field}.scaleHeightM"),
            "must be positive",
        ),
        DensityProfile::Tent {
            bottom_m,
            peak_m,
            top_m,
        } => require(
            [bottom_m, peak_m, top_m].iter().all(|h| h.is_finite())
                && bottom_m < peak_m
                && peak_m < top_m,
            || field.to_owned(),
            "must have bottomM < peakM < topM",
        ),
        DensityProfile::Tabulated {
            altitudes_m,
            relative,
        } => {
            require(
                altitudes_m.len() >= 2 && altitudes_m.len() == relative.len(),
                || field.to_owned(),
                "must have at least two nodes, as many relative densities as altitudes",
            )?;
            require(
                altitudes_m.iter().all(|h| h.is_finite())
                    && altitudes_m.windows(2).all(|pair| pair[0] < pair[1]),
                || format!("{field}.altitudesM"),
                "must be finite and strictly ascending",
            )?;
            require(
                relative.iter().all(|r| r.is_finite() && *r >= 0.0),
                || format!("{field}.relative"),
                "must be finite and not negative",
            )
        }
    }
}

/// Validates a term against the case's wavelength count.
fn term(term: &Term, count: usize, field: &str) -> Result<(), ReadCaseError> {
    require(
        !term.name.is_empty(),
        || format!("{field}.name"),
        "must not be empty",
    )?;
    density(&term.density, &format!("{field}.density"))?;
    let non_negative = |v: f64| v >= 0.0;
    per_wavelength(
        &term.scattering_per_m,
        count,
        &format!("{field}.scattering"),
        non_negative,
        "must be finite and not negative",
    )?;
    per_wavelength(
        &term.absorption_per_m,
        count,
        &format!("{field}.absorption"),
        non_negative,
        "must be finite and not negative",
    )?;
    match &term.phase {
        PhaseFunction::Rayleigh { depolarisation } => per_wavelength(
            depolarisation,
            count,
            &format!("{field}.phase.depolarisation"),
            |rho| (0.0..1.0).contains(&rho),
            "must lie in [0, 1)",
        ),
        PhaseFunction::CornetteShanks { asymmetry } => require(
            asymmetry.is_finite() && asymmetry.abs() < 1.0,
            || format!("{field}.phase.asymmetry"),
            "must lie in (-1, 1)",
        ),
        PhaseFunction::Isotropic {} => Ok(()),
        PhaseFunction::None {} => require(
            term.scattering_per_m.iter().all(|&s| s <= 0.0),
            || format!("{field}.scattering"),
            "must be zero for a term whose phase is none",
        ),
    }
}

/// Checks everything [`AtmosphereCase`] promises of its contents, and returns its figure.
fn validate(file: &CaseFile) -> Result<Figure, ReadCaseError> {
    if file.format != CASE_FORMAT {
        return Err(ReadCaseError::UnsupportedFormat(file.format));
    }
    require(
        !file.name.is_empty(),
        || "name".to_owned(),
        "must not be empty",
    )?;
    let count = file.wavelengths_nm.len();
    require(
        count > 0,
        || "wavelengthsNm".to_owned(),
        "must not be empty",
    )?;
    require(
        count <= MAX_WAVELENGTHS,
        || "wavelengthsNm".to_owned(),
        "must have at most 65,536 entries",
    )?;
    per_wavelength(
        &file.wavelengths_nm,
        count,
        "wavelengthsNm",
        |nm| nm > 0.0,
        "must be positive",
    )?;
    let figure = shells(&file.shells)?;
    require(
        file.top_height_m.is_finite() && file.top_height_m > 0.0,
        || "topHeightM".to_owned(),
        "must be positive",
    )?;
    per_wavelength(
        &file.ground_albedo,
        count,
        "groundAlbedo",
        |a| (0.0..=1.0).contains(&a),
        "must lie in [0, 1]",
    )?;
    let mut names = BTreeSet::new();
    for (i, t) in file.terms.iter().enumerate() {
        term(t, count, &format!("terms[{i}]"))?;
        require(
            names.insert(t.name.as_str()),
            || format!("terms[{i}].name"),
            "must be unique",
        )?;
    }
    require(
        !file.suns.is_empty(),
        || "suns".to_owned(),
        "must not be empty",
    )?;
    for (i, sun) in file.suns.iter().enumerate() {
        per_wavelength(
            &sun.irradiance,
            count,
            &format!("suns[{i}].irradiance"),
            |f| f >= 0.0,
            "must be finite and not negative",
        )?;
    }
    cone(file.detector_half_angle_deg, || {
        "detectorHalfAngleDeg".to_owned()
    })?;
    require(
        !file.geometries.is_empty() || !file.aggregates.is_empty(),
        || "geometries".to_owned(),
        "must not be empty when there are no aggregates",
    )?;
    observations(file)?;
    Ok(figure)
}

/// Checks the case's geometries and aggregates.
fn observations(file: &CaseFile) -> Result<(), ReadCaseError> {
    // Each sample's random stream is keyed by its geometry or aggregate in 32 bits.
    for (field, len) in [
        ("geometries", file.geometries.len()),
        ("aggregates", file.aggregates.len()),
    ] {
        require(
            u32::try_from(len).is_ok(),
            || field.to_owned(),
            "must have fewer than 4,294,967,296 entries",
        )?;
    }
    let mut names = BTreeSet::new();
    for (i, g) in file.geometries.iter().enumerate() {
        let field = format!("geometries[{i}]");
        require(
            !g.name.is_empty() && names.insert(g.name.as_str()),
            || format!("{field}.name"),
            "must be unique and not empty",
        )?;
        require(
            g.observer.height_m.is_finite() && g.observer.height_m >= 0.0,
            || format!("{field}.observer.heightM"),
            "must be finite and not negative",
        )?;
        latitude(g.observer.latitude_deg, &format!("{field}.observer"))?;
        local_direction(&g.view, &format!("{field}.view"))?;
        sun_directions(&g.sun_directions, file.suns.len(), &field)?;
        if let Some(distance) = g.max_distance_m {
            require(
                distance.is_finite() && distance > 0.0,
                || format!("{field}.maxDistanceM"),
                "must be positive",
            )?;
        }
        if let Some(half_angle) = g.detector_half_angle_deg {
            cone(half_angle, || format!("{field}.detectorHalfAngleDeg"))?;
        }
    }
    let mut names = BTreeSet::new();
    for (i, a) in file.aggregates.iter().enumerate() {
        let field = format!("aggregates[{i}]");
        require(
            !a.name.is_empty() && names.insert(a.name.as_str()),
            || format!("{field}.name"),
            "must be unique and not empty",
        )?;
        latitude(a.latitude_deg, &field)?;
        sun_directions(&a.sun_directions, file.suns.len(), &field)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// A small valid case: Rayleigh and an absorbing aerosol over a grey ground, two wavelengths,
    /// one sun, a ground and an orbit geometry and one aggregate.
    pub(crate) fn sample_case() -> Value {
        json!({
            "format": 1,
            "name": "sample",
            "wavelengthsNm": [550, 440],
            "shells": { "kind": "sphere", "radiusM": 6_371_000.0 },
            "topHeightM": 100_000.0,
            "groundAlbedo": [0.3, 0.25],
            "terms": [
                {
                    "name": "rayleigh",
                    "density": { "kind": "exponential", "scaleHeightM": 8000.0 },
                    "scattering": [1.15e-5, 2.87e-5],
                    "absorption": [0.0, 0.0],
                    "phase": { "kind": "rayleigh", "depolarisation": [0.0279, 0.0284] }
                },
                {
                    "name": "aerosol",
                    "density": { "kind": "exponential", "scaleHeightM": 1200.0 },
                    "scattering": [4.0e-6, 4.4e-6],
                    "absorption": [4.4e-7, 4.8e-7],
                    "phase": { "kind": "cornette-shanks", "asymmetry": 0.8 }
                },
                {
                    "name": "ozone",
                    "density": { "kind": "tent", "bottomM": 10_000.0, "peakM": 25_000.0, "topM": 40_000.0 },
                    "scattering": [0.0, 0.0],
                    "absorption": [3.4e-6, 0.2e-6],
                    "phase": { "kind": "none" }
                }
            ],
            "suns": [{ "name": "sun", "irradiance": [1.86, 1.95] }],
            "detectorHalfAngleDeg": 0.5,
            "geometries": [
                {
                    "name": "ground",
                    "observer": { "heightM": 0.0 },
                    "view": { "zenithDeg": 60.0, "azimuthDeg": 90.0 },
                    "sunDirections": [{ "zenithDeg": 30.0, "azimuthDeg": 0.0 }]
                },
                {
                    "name": "orbit-nadir",
                    "observer": { "heightM": 400_000.0, "latitudeDeg": 20.0 },
                    "view": { "zenithDeg": 180.0, "azimuthDeg": 0.0 },
                    "sunDirections": [{ "zenithDeg": 40.0, "azimuthDeg": 10.0 }],
                    "detectorHalfAngleDeg": 1.0
                }
            ],
            "aggregates": [
                { "name": "noon", "sunDirections": [{ "zenithDeg": 0.0, "azimuthDeg": 0.0 }] }
            ]
        })
    }

    /// A small valid case over a Saturn-class level spheroid (the spheroid module's `SATURN`):
    /// H₂–He Rayleigh of a 47 km scale height, a haze and a methane-like absorber over a
    /// Lambertian 1-bar boundary, two wavelengths, one sun; ground views at the equator looking
    /// north, at 60° looking east and at the pole, a view down from orbit at 30°, and one
    /// aggregate at 45°. Illustrative figures, not Saturn's.
    pub(crate) fn saturn_case() -> Value {
        let (a, c, gm, omega) = super::super::spheroid::tests::SATURN;
        json!({
            "format": 1,
            "name": "saturn-sample",
            "wavelengthsNm": [550, 440],
            "shells": {
                "kind": "spheroid", "equatorialRadiusM": a, "polarRadiusM": c,
                "gmM3S2": gm, "angularVelocityRadS": omega
            },
            "topHeightM": 40.0 * 47_000.0,
            "groundAlbedo": [0.5, 0.45],
            "terms": [
                {
                    "name": "h2-he",
                    "density": { "kind": "exponential", "scaleHeightM": 47_000.0 },
                    "scattering": [6.0e-6, 1.5e-5],
                    "absorption": [0.0, 0.0],
                    "phase": { "kind": "rayleigh", "depolarisation": [0.02, 0.02] }
                },
                {
                    "name": "haze",
                    "density": { "kind": "tent", "bottomM": 60_000.0, "peakM": 140_000.0, "topM": 240_000.0 },
                    "scattering": [1.0e-6, 1.2e-6],
                    "absorption": [1.0e-7, 2.0e-7],
                    "phase": { "kind": "cornette-shanks", "asymmetry": 0.6 }
                },
                {
                    "name": "absorber",
                    "density": { "kind": "exponential", "scaleHeightM": 47_000.0 },
                    "scattering": [0.0, 0.0],
                    "absorption": [4.0e-7, 1.0e-8],
                    "phase": { "kind": "none" }
                }
            ],
            "suns": [{ "name": "sun", "irradiance": [1.86, 1.95] }],
            "detectorHalfAngleDeg": 0.5,
            "geometries": [
                {
                    "name": "equator-north",
                    "observer": { "heightM": 0.0, "latitudeDeg": 0.0 },
                    "view": { "zenithDeg": 80.0, "azimuthDeg": 0.0 },
                    "sunDirections": [{ "zenithDeg": 40.0, "azimuthDeg": 150.0 }]
                },
                {
                    "name": "sixty-east",
                    "observer": { "heightM": 0.0, "latitudeDeg": 60.0 },
                    "view": { "zenithDeg": 85.0, "azimuthDeg": 90.0 },
                    "sunDirections": [{ "zenithDeg": 50.0, "azimuthDeg": 200.0 }]
                },
                {
                    "name": "pole",
                    "observer": { "heightM": 0.0, "latitudeDeg": 90.0 },
                    "view": { "zenithDeg": 70.0, "azimuthDeg": 0.0 },
                    "sunDirections": [{ "zenithDeg": 75.0, "azimuthDeg": 180.0 }]
                },
                {
                    "name": "orbit-thirty",
                    "observer": { "heightM": 5.0e6, "latitudeDeg": 30.0 },
                    "view": { "zenithDeg": 180.0, "azimuthDeg": 0.0 },
                    "sunDirections": [{ "zenithDeg": 30.0, "azimuthDeg": 0.0 }]
                }
            ],
            "aggregates": [
                {
                    "name": "forty-five",
                    "latitudeDeg": 45.0,
                    "sunDirections": [{ "zenithDeg": 20.0, "azimuthDeg": 0.0 }]
                }
            ]
        })
    }

    fn read(value: &Value) -> Result<AtmosphereCase, ReadCaseError> {
        AtmosphereCase::from_json(&value.to_string())
    }

    fn invalid_field(value: &Value) -> String {
        match read(value) {
            Err(ReadCaseError::Invalid { field, .. }) => field,
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    #[test]
    fn an_atmosphere_case_is_read_with_its_defaults() {
        let case = read(&sample_case()).unwrap();
        assert_eq!(case.name(), "sample");
        assert_eq!(case.seed(), 0);
        assert_eq!(case.wavelengths_nm().len(), 2);
        assert_eq!(case.geometry_count(), 2);
        assert_eq!(case.aggregate_count(), 1);
        assert_eq!(case.terms().len(), 3);
        let ground = &case.geometries()[0];
        assert!(ground.observer.latitude_deg.abs() < f64::EPSILON);
        assert!((case.detector_half_angle_deg(ground) - 0.5).abs() < f64::EPSILON);
        assert!((case.detector_half_angle_deg(&case.geometries()[1]) - 1.0).abs() < f64::EPSILON);
        assert!(
            matches!(case.shells(), Shells::Sphere { radius_m } if (*radius_m - 6.371e6).abs() < 1e-6)
        );
    }

    #[test]
    fn an_atmosphere_case_refuses_bad_lengths_and_ranges() {
        let mut case = sample_case();
        case["terms"][1]["scattering"] = json!([1e-6]);
        assert_eq!(invalid_field(&case), "terms[1].scattering");

        let mut case = sample_case();
        case["groundAlbedo"][1] = json!(1.5);
        assert_eq!(invalid_field(&case), "groundAlbedo[1]");

        let mut case = sample_case();
        case["terms"][2]["scattering"][0] = json!(1e-7);
        assert_eq!(invalid_field(&case), "terms[2].scattering");

        let mut case = sample_case();
        case["terms"][1]["name"] = json!("rayleigh");
        assert_eq!(invalid_field(&case), "terms[1].name");

        let mut case = sample_case();
        case["geometries"][0]["sunDirections"] = json!([]);
        assert_eq!(invalid_field(&case), "geometries[0].sunDirections");

        let mut case = sample_case();
        case["geometries"][1]["view"]["zenithDeg"] = json!(181.0);
        assert_eq!(invalid_field(&case), "geometries[1].view.zenithDeg");

        let mut case = sample_case();
        case["terms"][0]["density"] = json!({
            "kind": "tabulated", "altitudesM": [0.0, 5.0, 5.0], "relative": [1.0, 0.5, 0.2]
        });
        assert_eq!(invalid_field(&case), "terms[0].density.altitudesM");

        let mut case = sample_case();
        case["detectorHalfAngleDeg"] = json!(12.0);
        assert_eq!(invalid_field(&case), "detectorHalfAngleDeg");
    }

    #[test]
    fn an_atmosphere_case_refuses_unknown_fields_and_formats() {
        let mut case = sample_case();
        case["terms"][0]["colour"] = json!("blue");
        assert!(matches!(read(&case), Err(ReadCaseError::Json(_))));

        let mut case = sample_case();
        case["shells"] = json!({ "kind": "torus", "radiusM": 1.0 });
        assert!(matches!(read(&case), Err(ReadCaseError::Json(_))));

        // Inside the tagged kinds too: a spheroid's fields under a sphere's tag, a misspelt key.
        let mut case = sample_case();
        case["shells"]["polarRadiusM"] = json!(6_356_752.0);
        assert!(matches!(read(&case), Err(ReadCaseError::Json(_))));
        let mut case = sample_case();
        case["terms"][0]["density"]["scaleHeight"] = json!(8000.0);
        assert!(matches!(read(&case), Err(ReadCaseError::Json(_))));
        let mut case = sample_case();
        case["terms"][2]["phase"]["asymmetry"] = json!(0.5);
        assert!(matches!(read(&case), Err(ReadCaseError::Json(_))));

        let mut case = sample_case();
        case["format"] = json!(2);
        assert!(matches!(
            read(&case),
            Err(ReadCaseError::UnsupportedFormat(2))
        ));
    }

    #[test]
    fn an_atmosphere_case_reads_a_level_spheroid_and_refuses_bad_ones() {
        let case = read(&saturn_case()).unwrap();
        assert!(matches!(
            case.shells(),
            Shells::Spheroid { polar_radius_m, omega_rad_s, .. }
                if (*polar_radius_m - 54_364e3).abs() < 1e-6 && *omega_rad_s > 1.6e-4
        ));
        assert!(matches!(case.figure(), Figure::Spheroid(_)));
        // The sphere is the spheroid with a = c; an added gravity is optional, 0 by default.
        let mut round = saturn_case();
        round["shells"]["polarRadiusM"] = round["shells"]["equatorialRadiusM"].clone();
        round["shells"]["angularVelocityRadS"] = json!(0.0);
        read(&round).unwrap();
        let mut tidal = saturn_case();
        tidal["shells"]["gravityOffsetMS2"] = json!(0.04);
        assert!(matches!(
            read(&tidal).unwrap().shells(),
            Shells::Spheroid { gravity_offset_m_s2, .. } if (*gravity_offset_m_s2 - 0.04).abs() < 1e-15
        ));
        for (key, value, field) in [
            ("polarRadiusM", json!(60_300e3), "shells.polarRadiusM"),
            ("polarRadiusM", json!(0.0), "shells.polarRadiusM"),
            ("equatorialRadiusM", json!(-1.0), "shells.equatorialRadiusM"),
            ("gmM3S2", json!(0.0), "shells.gmM3S2"),
            // Ten times Saturn's spin flings its equator off.
            (
                "angularVelocityRadS",
                json!(1.64e-3),
                "shells.angularVelocityRadS",
            ),
            // An added gravity that leaves the equator's below zero.
            ("gravityOffsetMS2", json!(-9.1), "shells.gravityOffsetMS2"),
        ] {
            let mut case = saturn_case();
            case["shells"][key] = value;
            assert_eq!(invalid_field(&case), field, "{key}");
        }
        // The plan's Rust name is not the case's key, and a sphere's radius is not a spheroid's.
        let mut case = saturn_case();
        case["shells"]["omegaRadS"] = json!(1e-4);
        assert!(matches!(read(&case), Err(ReadCaseError::Json(_))));
        let mut case = saturn_case();
        case["shells"]["radiusM"] = json!(6e7);
        assert!(matches!(read(&case), Err(ReadCaseError::Json(_))));
        let mut case = saturn_case();
        case["shells"].as_object_mut().unwrap().remove("gmM3S2");
        assert!(matches!(read(&case), Err(ReadCaseError::Json(_))));
    }
}
