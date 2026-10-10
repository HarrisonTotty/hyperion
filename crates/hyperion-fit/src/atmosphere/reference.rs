//! The tracer's output: the reference radiances and flux aggregates of a case, as JSON for the
//! client (plan R08, R08.T12.a), and the running moments they are estimated from.
//!
//! ```text
//! {
//!   "format": 1, "case": "earth", "polarisation": "scalar", "samples": 100000, "seed": 0,
//!   "wavelengthsNm": [...],
//!   "geometries": [{
//!     "name": "...", "detectorHalfAngleDeg": 0.5,
//!     "radiance": [...], "standardError": [...],          // per wavelength
//!     "stokes": {                                          // only with --stokes
//!       "q": [...], "qStandardError": [...], "u": [...], "uStandardError": [...],
//!       "v": [...], "vStandardError": [...],
//!       "scalarRadiance": [...], "scalarStandardError": [...],
//!       "differenceStandardError": [...]                   // of radiance − scalarRadiance
//!     },
//!     "sunOpticalDepth": [[...]]                           // per sun, per wavelength; null where
//!   }],                                                    // the ground hides the sun
//!   "aggregates": [{
//!     "name": "...", "incidentTop": [...], "directGround": [...],
//!     "diffuseGround": [...], "diffuseGroundStandardError": [...],
//!     "upwellingTop": [...], "upwellingTopStandardError": [...],
//!     "scalar": {                                          // only with --stokes
//!       "diffuseGround": [...], "diffuseGroundStandardError": [...],
//!       "diffuseGroundDifferenceStandardError": [...],
//!       "upwellingTop": [...], "upwellingTopStandardError": [...],
//!       "upwellingTopDifferenceStandardError": [...]
//!     }
//!   }]
//! }
//! ```
//!
//! Radiances are in the unit of the case's irradiance per steradian, fluxes in its unit. In the
//! Stokes mode `radiance` and the fluxes are the Stokes vector's I, every gate's reference (R08
//! Design note 10), and `scalarRadiance` and `scalar` are the scalar estimate of the same paths,
//! for the diagnostics and the scalar gates; the standard error of their difference is given too,
//! since the two are strongly correlated. Q, U and V are in the frame (e₁, e₂) of the light's
//! direction k, the reverse of the view direction: e₁ lies in the view's vertical plane, towards
//! the larger zenith angle of the view, and e₂ = k × e₁ ([`super::transport`]). Q is positive for
//! light polarised along e₁, in that plane, and U for light polarised along e₁ + e₂; V's sign is
//! that of the case's b₂ ([`super::optics`]); azimuths run from north towards east. These are
//! Hovenier's conventions as Natraj and Hovenier 2012 (ApJ 748, 28) tabulate them, at the same
//! relative azimuth; Natraj, Li and Yung 2009 (ApJ 691, 1909), with Q = Iᵣ − Iₗ (their eq. 1b),
//! tabulate −Q and −U of these. A standard error is the sample standard deviation over √n; it
//! reads 0 from one sample, which carries no estimate of its own error.

use serde::{Deserialize, Serialize};

use super::Polarisation;

/// The output format this tracer writes.
pub const REFERENCE_FORMAT: u32 = 1;

/// The reference radiances of a case: per geometry and wavelength, the radiance, its standard
/// error and the sun's optical depth, with the samples and detector cone that made them; and the
/// two flux aggregates of each aggregate point. In the Stokes mode, also Q, U and V and the scalar
/// radiance and fluxes of the same paths, each with its error (module documentation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferenceRadiances {
    pub(crate) format: u32,
    pub(crate) case: String,
    pub(crate) polarisation: Polarisation,
    pub(crate) samples: u64,
    pub(crate) seed: u64,
    pub(crate) wavelengths_nm: Vec<f64>,
    pub(crate) geometries: Vec<GeometryRadiance>,
    pub(crate) aggregates: Vec<FluxAggregate>,
}

/// One geometry's reference radiances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeometryRadiance {
    pub(crate) name: String,
    pub(crate) detector_half_angle_deg: f64,
    pub(crate) radiance: Vec<f64>,
    pub(crate) standard_error: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) stokes: Option<StokesRadiance>,
    pub(crate) sun_optical_depth: Vec<Vec<Option<f64>>>,
}

/// One geometry's polarisation and scalar radiance, from the Stokes mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StokesRadiance {
    pub(crate) q: Vec<f64>,
    pub(crate) q_standard_error: Vec<f64>,
    pub(crate) u: Vec<f64>,
    pub(crate) u_standard_error: Vec<f64>,
    pub(crate) v: Vec<f64>,
    pub(crate) v_standard_error: Vec<f64>,
    pub(crate) scalar_radiance: Vec<f64>,
    pub(crate) scalar_standard_error: Vec<f64>,
    pub(crate) difference_standard_error: Vec<f64>,
}

/// One aggregate's scalar fluxes of the same paths, from the Stokes mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScalarFlux {
    pub(crate) diffuse_ground: Vec<f64>,
    pub(crate) diffuse_ground_standard_error: Vec<f64>,
    pub(crate) diffuse_ground_difference_standard_error: Vec<f64>,
    pub(crate) upwelling_top: Vec<f64>,
    pub(crate) upwelling_top_standard_error: Vec<f64>,
    pub(crate) upwelling_top_difference_standard_error: Vec<f64>,
}

/// The flux aggregates at one ground point and the top above it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FluxAggregate {
    pub(crate) name: String,
    pub(crate) incident_top: Vec<f64>,
    pub(crate) direct_ground: Vec<f64>,
    pub(crate) diffuse_ground: Vec<f64>,
    pub(crate) diffuse_ground_standard_error: Vec<f64>,
    pub(crate) upwelling_top: Vec<f64>,
    pub(crate) upwelling_top_standard_error: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scalar: Option<ScalarFlux>,
}

impl ReferenceRadiances {
    /// The traced case's name.
    #[must_use]
    pub fn case(&self) -> &str {
        &self.case
    }

    /// Whether the Stokes vector was traced.
    #[must_use]
    pub fn polarisation(&self) -> Polarisation {
        self.polarisation
    }

    /// Samples per geometry, aggregate and wavelength.
    #[must_use]
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// The traced wavelengths, nm.
    #[must_use]
    pub fn wavelengths_nm(&self) -> &[f64] {
        &self.wavelengths_nm
    }

    /// Every geometry's radiances, in the case's order.
    #[must_use]
    pub fn geometries(&self) -> &[GeometryRadiance] {
        &self.geometries
    }

    /// Every aggregate's fluxes, in the case's order.
    #[must_use]
    pub fn aggregates(&self) -> &[FluxAggregate] {
        &self.aggregates
    }

    /// The reference as pretty-printed JSON, with a final newline.
    ///
    /// Every number in it is finite: [`trace_reference`](super::trace_reference) checks, since
    /// `serde_json` would write a non-finite one as `null`.
    ///
    /// # Panics
    ///
    /// Never: serialising these plain structs of numbers and strings into a `String` cannot fail.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self)
            .expect("plain structs of numbers and strings serialise into a String");
        text.push('\n');
        text
    }
}

impl GeometryRadiance {
    /// The geometry's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The detector cone the radiance is averaged over, as a half-angle, degrees.
    #[must_use]
    pub fn detector_half_angle_deg(&self) -> f64 {
        self.detector_half_angle_deg
    }

    /// The diffuse radiance per wavelength, in the case's irradiance unit per steradian.
    #[must_use]
    pub fn radiance(&self) -> &[f64] {
        &self.radiance
    }

    /// The radiance's standard error per wavelength.
    #[must_use]
    pub fn standard_error(&self) -> &[f64] {
        &self.standard_error
    }

    /// The polarisation and the scalar radiance of the same paths, from the Stokes mode only.
    #[must_use]
    pub fn stokes(&self) -> Option<&StokesRadiance> {
        self.stokes.as_ref()
    }

    /// Each sun's optical depth from the observer to space, per wavelength: `None` where the
    /// ground hides the sun.
    #[must_use]
    pub fn sun_optical_depth(&self) -> &[Vec<Option<f64>>] {
        &self.sun_optical_depth
    }
}

impl StokesRadiance {
    /// Q per wavelength.
    #[must_use]
    pub fn q(&self) -> &[f64] {
        &self.q
    }

    /// Q's standard error per wavelength.
    #[must_use]
    pub fn q_standard_error(&self) -> &[f64] {
        &self.q_standard_error
    }

    /// U per wavelength.
    #[must_use]
    pub fn u(&self) -> &[f64] {
        &self.u
    }

    /// U's standard error per wavelength.
    #[must_use]
    pub fn u_standard_error(&self) -> &[f64] {
        &self.u_standard_error
    }

    /// V per wavelength.
    #[must_use]
    pub fn v(&self) -> &[f64] {
        &self.v
    }

    /// V's standard error per wavelength.
    #[must_use]
    pub fn v_standard_error(&self) -> &[f64] {
        &self.v_standard_error
    }

    /// The scalar radiance of the same paths per wavelength, as the scalar mode traces it from
    /// the same draws.
    #[must_use]
    pub fn scalar_radiance(&self) -> &[f64] {
        &self.scalar_radiance
    }

    /// The scalar radiance's standard error per wavelength.
    #[must_use]
    pub fn scalar_standard_error(&self) -> &[f64] {
        &self.scalar_standard_error
    }

    /// The standard error of the radiance less the scalar radiance, per wavelength.
    #[must_use]
    pub fn difference_standard_error(&self) -> &[f64] {
        &self.difference_standard_error
    }
}

impl ScalarFlux {
    /// The scalar diffuse downwelling flux at the ground per wavelength.
    #[must_use]
    pub fn diffuse_ground(&self) -> &[f64] {
        &self.diffuse_ground
    }

    /// Its standard error per wavelength.
    #[must_use]
    pub fn diffuse_ground_standard_error(&self) -> &[f64] {
        &self.diffuse_ground_standard_error
    }

    /// The standard error of the vector flux less the scalar one, per wavelength.
    #[must_use]
    pub fn diffuse_ground_difference_standard_error(&self) -> &[f64] {
        &self.diffuse_ground_difference_standard_error
    }

    /// The scalar upwelling flux at the top per wavelength.
    #[must_use]
    pub fn upwelling_top(&self) -> &[f64] {
        &self.upwelling_top
    }

    /// Its standard error per wavelength.
    #[must_use]
    pub fn upwelling_top_standard_error(&self) -> &[f64] {
        &self.upwelling_top_standard_error
    }

    /// The standard error of the vector flux less the scalar one, per wavelength.
    #[must_use]
    pub fn upwelling_top_difference_standard_error(&self) -> &[f64] {
        &self.upwelling_top_difference_standard_error
    }
}

impl FluxAggregate {
    /// The aggregate's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The suns' flux onto a horizontal surface at the top, Σ μ₀F, per wavelength.
    #[must_use]
    pub fn incident_top(&self) -> &[f64] {
        &self.incident_top
    }

    /// The direct beams' flux onto the ground, Σ μ₀F e^(−τ), per wavelength.
    #[must_use]
    pub fn direct_ground(&self) -> &[f64] {
        &self.direct_ground
    }

    /// The diffuse downwelling flux at the ground per wavelength; the downwelling flux is this
    /// plus [`direct_ground`](Self::direct_ground).
    #[must_use]
    pub fn diffuse_ground(&self) -> &[f64] {
        &self.diffuse_ground
    }

    /// The diffuse downwelling flux's standard error per wavelength.
    #[must_use]
    pub fn diffuse_ground_standard_error(&self) -> &[f64] {
        &self.diffuse_ground_standard_error
    }

    /// The upwelling flux at the top per wavelength; the plane albedo is this over
    /// [`incident_top`](Self::incident_top).
    #[must_use]
    pub fn upwelling_top(&self) -> &[f64] {
        &self.upwelling_top
    }

    /// The upwelling flux's standard error per wavelength.
    #[must_use]
    pub fn upwelling_top_standard_error(&self) -> &[f64] {
        &self.upwelling_top_standard_error
    }

    /// The scalar fluxes of the same paths, from the Stokes mode only.
    #[must_use]
    pub fn scalar(&self) -> Option<&ScalarFlux> {
        self.scalar.as_ref()
    }
}

/// A running count, mean and sum of squared deviations (Welford 1962), merged by Chan, Golub and
/// LeVeque 1979's update.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Moments {
    n: u64,
    mean: f64,
    m2: f64,
}

/// `n` as an `f64`.
#[expect(
    clippy::cast_precision_loss,
    reason = "a sample count stays far below 2⁵³, where every integer is an exact f64"
)]
fn count(n: u64) -> f64 {
    n as f64
}

impl Moments {
    /// Adds one sample.
    pub(crate) fn push(&mut self, x: f64) {
        self.n += 1;
        let delta = x - self.mean;
        self.mean += delta / count(self.n);
        self.m2 += delta * (x - self.mean);
    }

    /// Adds the samples of `other`.
    pub(crate) fn merge(&mut self, other: &Self) {
        if other.n == 0 {
            return;
        }
        if self.n == 0 {
            *self = *other;
            return;
        }
        let n = self.n + other.n;
        let delta = other.mean - self.mean;
        let (a, b, total) = (count(self.n), count(other.n), count(n));
        self.mean += delta * b / total;
        self.m2 += other.m2 + delta * delta * a * b / total;
        self.n = n;
    }

    /// The mean.
    #[must_use]
    pub(crate) fn mean(&self) -> f64 {
        self.mean
    }

    /// The standard error of the mean: 0 from fewer than two samples.
    #[must_use]
    pub(crate) fn standard_error(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        let n = count(self.n);
        (self.m2.max(0.0) / (n - 1.0) / n).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atmosphere_moments_merge_as_one_pass() {
        let xs: Vec<f64> = (0..1000)
            .map(|i| f64::from((i * 37) % 101) * 0.25)
            .collect();
        let mut whole = Moments::default();
        for &x in &xs {
            whole.push(x);
        }
        let mut parts = Moments::default();
        for chunk in xs.chunks(77) {
            let mut part = Moments::default();
            for &x in chunk {
                part.push(x);
            }
            parts.merge(&part);
        }
        let n = 1000.0;
        let mean = xs.iter().sum::<f64>() / n;
        let variance = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0);
        for m in [whole, parts] {
            assert!((m.mean() - mean).abs() < 1e-12);
            assert!((m.standard_error() - (variance / n).sqrt()).abs() < 1e-12);
        }
        let mut one = Moments::default();
        one.push(3.0);
        assert!(one.standard_error().abs() < f64::EPSILON);
    }
}
