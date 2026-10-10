//! The offline reference for the client's atmospheres: a backward Monte Carlo path tracer in
//! `f64` over spherical shells, or over a level spheroid's, with no tables (plan R08, R08.T12.a
//! and R08.T12.d; Design notes 10 and 17).
//!
//! The client draws its atmospheres from Hillaire 2020's tables and, where his analytic multiple
//! scattering drifts, from a bake by discrete ordinates (R08 Design notes 9 and 10). Every one of
//! those tables is gated against this tracer to 5% on a stated metric. It is a different language
//! and a different algorithm from the client's, so that the two stay independent.
//!
//! This is a new kind of output for the crate: not a fitted table for the sim, but JSON fixtures
//! for the client. `hyperion-fit atmosphere-reference` traces a case ([`AtmosphereCase`], written
//! by the client's optics) into its reference ([`ReferenceRadiances`]); it is not a
//! [`FitTask`](crate::task::FitTask), so `tables.lock` and `just fit-check` do not cover its
//! output, and its runs are by hand, their outputs committed beside the cases (R08.T12.b).
//!
//! - [`case`]: the case format and its validation;
//! - [`transport`]: the estimator (absorption as a weight, delta tracking, a forced first
//!   collision, next-event estimation to every sun, a Lambertian ground, Russian roulette and the
//!   Stokes mode, which scores the scalar radiance of the same paths beside the vector one);
//! - [`optics`]: profiles, phase functions (closed forms, tables on u = √(Θ ÷ π) and Legendre
//!   series) and the block-diagonal scattering matrices;
//! - [`geometry`]: frames and the shells;
//! - [`spheroid`]: the level spheroid of an oblate body, its normal gravity, geodetic coordinates
//!   and quadric shells, over which the medium is read at the gravity-scaled height;
//! - [`reference`]: the output and its moments;
//! - `benchmarks` (tests only): the published benchmarks the tracer is held to before its
//!   references are trusted (R08.T12.c), slow tests run by `just test-slow atmosphere::benchmarks`.
//!
//! **Reproducible for any thread count**, by the crate's convention: the samples of each geometry,
//! aggregate and wavelength are cut into blocks of [`BLOCK_SAMPLES`] mapped by
//! [`parallel::map_reduce_chunks`](crate::parallel::map_reduce_chunks), and every sample draws from
//! its own counter-based stream ([`Draws`](crate::tasks::displaced_forms::births::Draws), as the
//! displaced forms' orbits draw), so that a sample is a pure function of its key and the blocks'
//! moments are merged in index order. A sample's key is the case's seed salted with
//! [`ATMOSPHERE_STREAM`], its wavelength and its geometry or aggregate by their own indices (so
//! that adding a geometry leaves the others' draws alone), and its number. Every transcendental
//! goes through [`hyperion_sim::math`]. Nothing here is generated output: no domain tag and no
//! generator version is involved (R08's Generator version).

#[cfg(test)]
mod benchmarks;
pub mod case;
mod geometry;
mod optics;
pub mod reference;
mod spheroid;
mod transport;

use std::num::{NonZeroU64, NonZeroUsize};

use serde::{Deserialize, Serialize};

pub use case::{AtmosphereCase, ReadCaseError, Shells};
pub use reference::{
    FluxAggregate, GeometryRadiance, ReferenceRadiances, ScalarFlux, StokesRadiance,
};

use geometry::ShellGrid;
use reference::{Moments, REFERENCE_FORMAT};
use transport::{COMPONENTS, I, Level, Medium, SCALAR_I, Scratch, Source};

use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::tasks::displaced_forms::births::Draws;

/// The samples of one geometry, aggregate and wavelength that one map step traces.
pub const BLOCK_SAMPLES: u64 = 1024;

/// The most samples a geometry, aggregate and wavelength may take: 2⁵³, below which every count
/// is an exact `f64`.
pub const MAX_SAMPLES: u64 = 1 << 53;

/// Mixed into the case's seed by exclusive or, so that the tracer's draws are its own and not
/// those of the displaced forms' orbits, which use the same generator: `atmosphe` in ASCII.
pub const ATMOSPHERE_STREAM: u64 = 0x6174_6d6f_7370_6865;

/// Whether the tracer follows the radiance alone or the Stokes vector.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Polarisation {
    /// The radiance alone, as the client draws it.
    #[default]
    Scalar,
    /// The Stokes vector (I, Q, U, V), with the scalar radiance of the same paths beside it: every
    /// gate's reference (R08 Design note 10). Every scattering term needs its scattering matrix,
    /// or the case's `depolarising` mark.
    Stokes,
}

/// A reference could not be traced.
#[derive(Debug, thiserror::Error)]
pub enum TraceReferenceError {
    /// The Stokes mode was asked of a case with a term that has no scattering matrix and is not
    /// marked depolarising.
    #[error(
        "the Stokes mode needs a scattering matrix for term `{term}`, or the term marked \
         depolarising"
    )]
    StokesNeedsMatrix {
        /// The term.
        term: String,
    },
    /// More samples than the tracer counts exactly.
    #[error("{samples} samples are more than the tracer counts exactly, 2^53")]
    TooManySamples {
        /// The samples asked for.
        samples: u64,
    },
    /// The thread pool could not be built.
    #[error(transparent)]
    ThreadPool(#[from] BuildThreadPoolError),
}

/// The case's shells: split at every kink of a term's profile and every scale height of an
/// exponential one.
fn shell_grid(case: &AtmosphereCase) -> ShellGrid {
    let mut heights_m = Vec::new();
    for term in case.terms() {
        term.density
            .shell_heights(case.top_height_m(), &mut heights_m);
    }
    ShellGrid::new(case.figure(), case.top_height_m(), heights_m)
}

/// `n` as a `u64`.
fn wide(n: usize) -> u64 {
    u64::try_from(n).expect("a usize fits in 64 bits on every platform HYPERION builds for")
}

/// Traces `case`'s reference radiances: `samples` samples per geometry, aggregate and wavelength,
/// on `threads` threads, with or without the Stokes vector.
///
/// The output is the same, bit for bit, for any number of threads.
///
/// # Errors
///
/// [`TraceReferenceError::StokesNeedsMatrix`] for the Stokes mode on a case with a term that has
/// no scattering matrix and is not marked depolarising; [`TraceReferenceError::TooManySamples`]
/// past [`MAX_SAMPLES`];
/// [`TraceReferenceError::ThreadPool`] if the threads cannot be started.
///
/// # Panics
///
/// If a traced value is not finite, which would be a bug in the tracer: `serde_json` would write
/// it as `null`.
pub fn trace_reference(
    case: &AtmosphereCase,
    polarisation: Polarisation,
    samples: NonZeroU64,
    threads: NonZeroUsize,
) -> Result<ReferenceRadiances, TraceReferenceError> {
    if polarisation == Polarisation::Stokes
        && let Some(term) = case
            .terms()
            .iter()
            .find(|t| !t.phase.has_matrix() && !t.depolarising)
    {
        return Err(TraceReferenceError::StokesNeedsMatrix {
            term: term.name.clone(),
        });
    }
    if samples.get() > MAX_SAMPLES {
        return Err(TraceReferenceError::TooManySamples {
            samples: samples.get(),
        });
    }
    let grid = shell_grid(case);
    let media: Vec<Medium<'_>> = (0..case.wavelengths_nm().len())
        .map(|w| Medium::new(case, &grid, w))
        .collect();
    // Each source with its key: its kind (0 a geometry, 1 an aggregate's ground, 2 its top) above
    // its index within the kind, which the case keeps below 2³².
    let mut sources: Vec<(u64, Source)> = case
        .geometries()
        .iter()
        .enumerate()
        .map(|(g, geometry)| (wide(g), Source::detector(case, &grid, geometry)))
        .collect();
    for (a, aggregate) in case.aggregates().iter().enumerate() {
        sources.push((
            (1 << 32) | wide(a),
            Source::flux(&grid, aggregate, Level::Ground),
        ));
        sources.push((
            (2 << 32) | wide(a),
            Source::flux(&grid, aggregate, Level::Top),
        ));
    }
    let moments = trace_moments(
        case.seed(),
        &media,
        &sources,
        polarisation,
        samples.get(),
        threads,
    )?;
    let run = Run {
        case,
        grid: &grid,
        media: &media,
        sources: &sources,
        moments,
    };
    Ok(ReferenceRadiances {
        format: REFERENCE_FORMAT,
        case: case.name().to_owned(),
        polarisation,
        samples: samples.get(),
        seed: case.seed(),
        wavelengths_nm: case.wavelengths_nm().to_vec(),
        geometries: (0..case.geometry_count())
            .map(|g| run.geometry(g, polarisation))
            .collect(),
        aggregates: (0..case.aggregate_count())
            .map(|a| run.aggregate(a, polarisation))
            .collect(),
    })
}

/// The moments a task keeps: those of each of a sample's [`COMPONENTS`], and of its radiance less
/// its scalar radiance.
const MOMENTS: usize = COMPONENTS + 1;

/// Where the moments of a sample's radiance less its scalar radiance are kept.
const DIFFERENCE: usize = COMPONENTS;

/// Traces `samples` samples of every source at every wavelength: the moments of (I, Q, U, V), of
/// the scalar radiance and of the radiance less it, per task, wavelength-major. The scalar mode
/// fills the first alone.
fn trace_moments(
    seed: u64,
    media: &[Medium<'_>],
    sources: &[(u64, Source)],
    polarisation: Polarisation,
    samples: u64,
    threads: NonZeroUsize,
) -> Result<Vec<[Moments; MOMENTS]>, TraceReferenceError> {
    let per_wavelength = sources.len();
    let tasks = media.len() * per_wavelength;
    let blocks = samples.div_ceil(BLOCK_SAMPLES);
    let items = wide(tasks)
        .checked_mul(blocks)
        .ok_or(TraceReferenceError::TooManySamples { samples })?;
    let seed = seed ^ ATMOSPHERE_STREAM;
    let mut moments = vec![[Moments::default(); MOMENTS]; tasks];
    map_reduce_chunks(
        items,
        1,
        threads,
        |range| {
            let mut scratch = Scratch::default();
            range
                .map(|item| {
                    let (task, block) = (item / blocks, item % blocks);
                    let index =
                        usize::try_from(task).expect("a task's index is below `tasks`, a usize");
                    let wavelength = index / per_wavelength;
                    let (key, source) = &sources[index % per_wavelength];
                    // The case keeps its wavelengths below 2¹⁶ and its sources' keys below 2³⁴.
                    let stream = (wide(wavelength) << 34) | key;
                    let mut block_moments = [Moments::default(); MOMENTS];
                    let end = ((block + 1) * BLOCK_SAMPLES).min(samples);
                    for sample in block * BLOCK_SAMPLES..end {
                        let mut draws = Draws::new(seed, stream, sample);
                        let x = media[wavelength].sample(
                            source,
                            polarisation,
                            &mut draws,
                            &mut scratch,
                        );
                        match polarisation {
                            Polarisation::Scalar => block_moments[I].push(x[I]),
                            Polarisation::Stokes => {
                                for (m, value) in block_moments.iter_mut().zip(x) {
                                    m.push(value);
                                }
                                block_moments[DIFFERENCE].push(x[I] - x[SCALAR_I]);
                            }
                        }
                    }
                    (index, block_moments)
                })
                .collect::<Vec<_>>()
        },
        |results| {
            for (index, block_moments) in results {
                for (total, part) in moments[index].iter_mut().zip(&block_moments) {
                    total.merge(part);
                }
            }
        },
    )?;
    Ok(moments)
}

/// A traced case: its media per wavelength, its sources (the geometries' detectors, then each
/// aggregate's ground and top) and the moments of each task, wavelength-major.
struct Run<'a> {
    case: &'a AtmosphereCase,
    grid: &'a ShellGrid,
    media: &'a [Medium<'a>],
    sources: &'a [(u64, Source)],
    moments: Vec<[Moments; MOMENTS]>,
}

/// `values`, after checking that each is finite.
///
/// # Panics
///
/// If one is not: a bug in the tracer.
fn finite(values: Vec<f64>) -> Vec<f64> {
    assert!(
        values.iter().all(|x| x.is_finite()),
        "the tracer produced a value that is not finite: {values:?}"
    );
    values
}

impl Run<'_> {
    /// Statistic `f` of Stokes component `component` of source `source`, per wavelength.
    fn column(&self, source: usize, component: usize, f: fn(&Moments) -> f64) -> Vec<f64> {
        let per_wavelength = self.sources.len();
        finite(
            (0..self.media.len())
                .map(|w| f(&self.moments[w * per_wavelength + source][component]))
                .collect(),
        )
    }

    /// Geometry `g`'s radiances.
    fn geometry(&self, g: usize, polarisation: Polarisation) -> GeometryRadiance {
        let geometry = &self.case.geometries()[g];
        let source = &self.sources[g].1;
        GeometryRadiance {
            name: geometry.name.clone(),
            detector_half_angle_deg: self.case.detector_half_angle_deg(geometry),
            radiance: self.column(g, I, Moments::mean),
            standard_error: self.column(g, I, Moments::standard_error),
            stokes: (polarisation == Polarisation::Stokes).then(|| StokesRadiance {
                q: self.column(g, 1, Moments::mean),
                q_standard_error: self.column(g, 1, Moments::standard_error),
                u: self.column(g, 2, Moments::mean),
                u_standard_error: self.column(g, 2, Moments::standard_error),
                v: self.column(g, 3, Moments::mean),
                v_standard_error: self.column(g, 3, Moments::standard_error),
                scalar_radiance: self.column(g, SCALAR_I, Moments::mean),
                scalar_standard_error: self.column(g, SCALAR_I, Moments::standard_error),
                difference_standard_error: self.column(g, DIFFERENCE, Moments::standard_error),
            }),
            sun_optical_depth: source
                .suns
                .iter()
                .map(|&sun| {
                    self.media
                        .iter()
                        .map(|m| m.optical_depth_to_space(source.origin_m, sun))
                        .collect()
                })
                .collect(),
        }
    }

    /// Aggregate `a`'s fluxes.
    fn aggregate(&self, a: usize, polarisation: Polarisation) -> FluxAggregate {
        let ground = self.case.geometry_count() + 2 * a;
        let top = ground + 1;
        let source = &self.sources[ground].1;
        let up = self.grid.vertical(source.origin_m);
        // Each sun above the horizon's μ₀F, with the transmittance of its beam to the ground.
        let beams = |w: usize| {
            source
                .suns
                .iter()
                .zip(self.case.suns())
                .filter(|(sun, _)| up.dot(**sun) > 0.0)
                .map(move |(&sun, spectrum)| {
                    let transmittance = self.media[w]
                        .optical_depth_to_space(source.origin_m, sun)
                        .map_or(0.0, |depth| hyperion_sim::math::exp(-depth));
                    (up.dot(sun) * spectrum.irradiance[w], transmittance)
                })
        };
        let incident = |w: usize| beams(w).map(|(flux, _)| flux).sum();
        let direct = |w: usize| beams(w).map(|(flux, t)| flux * t).sum();
        let wavelengths = 0..self.media.len();
        FluxAggregate {
            name: self.case.aggregates()[a].name.clone(),
            incident_top: finite(wavelengths.clone().map(incident).collect()),
            direct_ground: finite(wavelengths.map(direct).collect()),
            diffuse_ground: self.column(ground, I, Moments::mean),
            diffuse_ground_standard_error: self.column(ground, I, Moments::standard_error),
            upwelling_top: self.column(top, I, Moments::mean),
            upwelling_top_standard_error: self.column(top, I, Moments::standard_error),
            scalar: (polarisation == Polarisation::Stokes).then(|| ScalarFlux {
                diffuse_ground: self.column(ground, SCALAR_I, Moments::mean),
                diffuse_ground_standard_error: self.column(
                    ground,
                    SCALAR_I,
                    Moments::standard_error,
                ),
                diffuse_ground_difference_standard_error: self.column(
                    ground,
                    DIFFERENCE,
                    Moments::standard_error,
                ),
                upwelling_top: self.column(top, SCALAR_I, Moments::mean),
                upwelling_top_standard_error: self.column(top, SCALAR_I, Moments::standard_error),
                upwelling_top_difference_standard_error: self.column(
                    top,
                    DIFFERENCE,
                    Moments::standard_error,
                ),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use core::f64::consts::PI;

    use hyperion_sim::math;
    use serde_json::{Value, json};

    use super::*;

    pub(crate) fn case_of(value: &Value) -> AtmosphereCase {
        AtmosphereCase::from_json(&value.to_string()).unwrap()
    }

    fn trace(
        value: &Value,
        polarisation: Polarisation,
        samples: u64,
        threads: usize,
    ) -> ReferenceRadiances {
        trace_reference(
            &case_of(value),
            polarisation,
            NonZeroU64::new(samples).unwrap(),
            NonZeroUsize::new(threads).unwrap(),
        )
        .unwrap()
    }

    fn radians(degrees: f64) -> f64 {
        degrees.to_radians()
    }

    /// A geometry's JSON.
    fn geometry(name: &str, height_m: f64, view: (f64, f64), sun: (f64, f64)) -> Value {
        json!({
            "name": name,
            "observer": { "heightM": height_m },
            "view": { "zenithDeg": view.0, "azimuthDeg": view.1 },
            "sunDirections": [{ "zenithDeg": sun.0, "azimuthDeg": sun.1 }]
        })
    }

    /// A plane-parallel slab, 1 km of uniform medium over a ground of radius 10¹² m, where the
    /// sphere's curvature moves nothing at the precision tested.
    fn slab(terms: &Value, albedo: &Value, geometries: &Value) -> Value {
        json!({
            "format": 1,
            "name": "slab",
            "wavelengthsNm": [550, 440],
            "shells": { "kind": "sphere", "radiusM": 1e12 },
            "topHeightM": 1000.0,
            "groundAlbedo": albedo,
            "terms": terms,
            "suns": [{ "name": "sun", "irradiance": [PI, 2.0] }],
            "detectorHalfAngleDeg": 0.0,
            "geometries": geometries
        })
    }

    fn uniform() -> Value {
        json!({ "kind": "tabulated", "altitudesM": [0.0, 1000.0], "relative": [1.0, 1.0] })
    }

    #[test]
    fn atmosphere_absorbing_only_medium_gives_beer_lambert() {
        let (radius, top, height, sigma) = (6_371_000.0, 100_000.0, 8000.0, [1e-5, 3e-5]);
        let (albedo, irradiance) = ([0.3, 0.6], [1.7, 2.0]);
        let exponential = json!({ "kind": "exponential", "scaleHeightM": height });
        let shell =
            json!({ "kind": "tabulated", "altitudesM": [0.0, top], "relative": [1.0, 1.0] });
        let sun_zenith = 60.0;
        let case = |density: &Value, sun: f64| {
            json!({
                "format": 1, "name": "absorbing",
                "wavelengthsNm": [550, 440],
                "shells": { "kind": "sphere", "radiusM": radius },
                "topHeightM": top,
                "groundAlbedo": albedo,
                "terms": [{
                    "name": "absorber", "density": density,
                    "scattering": [0.0, 0.0], "absorption": sigma, "phase": { "kind": "none" }
                }],
                "suns": [{ "name": "sun", "irradiance": irradiance }],
                "detectorHalfAngleDeg": 0.0,
                "geometries": [
                    geometry("nadir", 400_000.0, (180.0, 0.0), (sun, 0.0)),
                    geometry("ground", 0.0, (0.0, 0.0), (sun, 0.0))
                ]
            })
        };
        // The exponential column, vertical: τ = σH(1 − e^(−top ÷ H)).
        let vertical = trace(&case(&exponential, 0.0), Polarisation::Scalar, 64, 2);
        // A uniform shell, the sun at 60°: τ = σ × the chord from the ground to the top.
        let slant = trace(&case(&shell, sun_zenith), Polarisation::Scalar, 64, 2);
        let mu0 = math::cos(radians(sun_zenith));
        let chord =
            -radius * mu0 + (radius * radius * mu0 * mu0 + top * (2.0 * radius + top)).sqrt();
        for w in 0..2 {
            let tau = sigma[w] * height * -math::exp_m1(-top / height);
            let reflected = albedo[w] / PI * irradiance[w];
            let expected = [
                (&vertical, reflected * math::exp(-2.0 * tau), tau),
                (
                    &slant,
                    reflected * mu0 * math::exp(-sigma[w] * (top + chord)),
                    sigma[w] * chord,
                ),
            ];
            for (reference, radiance, sun_depth) in expected {
                let [nadir, ground] = reference.geometries() else {
                    panic!("two geometries")
                };
                assert!(
                    (nadir.radiance()[w] / radiance - 1.0).abs() < 1e-9,
                    "{}: {} against {radiance}",
                    reference.case(),
                    nadir.radiance()[w]
                );
                assert!(nadir.standard_error()[w] < 1e-12 * radiance);
                assert!(ground.radiance()[w].abs() < f64::MIN_POSITIVE);
                let depth = ground.sun_optical_depth()[0][w].unwrap();
                assert!(
                    (depth / sun_depth - 1.0).abs() < 1e-9,
                    "{depth} against {sun_depth}"
                );
                assert!(nadir.sun_optical_depth()[0][w].unwrap().abs() < f64::MIN_POSITIVE);
            }
        }
    }

    #[test]
    fn atmosphere_reference_is_the_same_for_one_and_four_threads() {
        let case = case::tests::sample_case();
        let one = trace(&case, Polarisation::Scalar, 1500, 1).to_json();
        let four = trace(&case, Polarisation::Scalar, 1500, 4).to_json();
        assert_eq!(one, four);
        let mut rayleigh = case;
        rayleigh["terms"][1]["phase"] = json!({ "kind": "isotropic" });
        let one = trace(&rayleigh, Polarisation::Stokes, 1500, 1).to_json();
        let four = trace(&rayleigh, Polarisation::Stokes, 1500, 4).to_json();
        assert_eq!(one, four);
        assert!(one.contains("\"stokes\""));
        // And over a level spheroid.
        let saturn = case::tests::saturn_case();
        let one = trace(&saturn, Polarisation::Scalar, 300, 1).to_json();
        let four = trace(&saturn, Polarisation::Scalar, 300, 4).to_json();
        assert_eq!(one, four);
        // And R08.T12.c's paths: a table with its matrix, a Legendre series, two suns and the
        // directions drawn towards them.
        let polarising = polarising_case();
        let one = trace(&polarising, Polarisation::Stokes, 300, 1).to_json();
        let four = trace(&polarising, Polarisation::Stokes, 300, 4).to_json();
        assert_eq!(one, four);
    }

    /// The sample case with R08.T12.c's paths in it: its aerosol a forward-peaked table
    /// (Henyey–Greenstein at g 0.8) with a matrix whose every element is non-zero, so that the
    /// directions are drawn partly towards the suns near the ground; a dust of a Legendre series,
    /// marked depolarising; and a second sun.
    fn polarising_case() -> Value {
        let mut case = case::tests::sample_case();
        case["terms"][1]["scattering"] = json!([1e-5, 1.1e-5]);
        case["terms"][1]["phase"] = case::tests::tabulated_phase_json(256, 2, |c| {
            let p = optics::tests::henyey_greenstein(0.8, c);
            let sin2 = 1.0 - c * c;
            [
                p,
                p,
                c * p,
                0.9 * c * p,
                -0.3 * sin2 * p,
                0.2 * sin2.sqrt() * p,
            ]
        });
        case["terms"].as_array_mut().unwrap().push(json!({
            "name": "dust", "density": { "kind": "exponential", "scaleHeightM": 2000.0 },
            "scattering": [2e-6, 2e-6], "absorption": [1e-7, 1e-7],
            "phase": { "kind": "legendre", "coefficients": [[1.0, 1.2, 0.5], [1.0, 0.9]] },
            "depolarising": true
        }));
        case["suns"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "name": "second", "irradiance": [0.4, 0.3] }));
        let second = json!({ "zenithDeg": 70.0, "azimuthDeg": 120.0 });
        for g in 0..2 {
            case["geometries"][g]["sunDirections"]
                .as_array_mut()
                .unwrap()
                .push(second.clone());
        }
        case["aggregates"][0]["sunDirections"]
            .as_array_mut()
            .unwrap()
            .push(second);
        case
    }

    #[test]
    fn atmosphere_stokes_reference_bits_are_pinned() {
        // The Stokes mode's counterpart of `atmosphere_reference_bits_are_pinned`, on R08.T12.c's
        // paths (`polarising_case`): the ground view's I, Q, V, scalar radiance and the error of
        // their difference, and the scalar upwelling flux. A change to the tables' draws, the
        // series' proposal, the sunward draws, the 4 × 4 product or the moments shows here before
        // it moves a committed reference.
        let reference = trace(&polarising_case(), Polarisation::Stokes, 64, 1);
        let ground = &reference.geometries()[0];
        let stokes = ground.stokes().unwrap();
        let pinned = [
            format!("{:?}", ground.radiance()[0]),
            format!("{:?}", stokes.q()[0]),
            format!("{:?}", stokes.v()[0]),
            format!("{:?}", stokes.scalar_radiance()[1]),
            format!("{:?}", stokes.difference_standard_error()[0]),
            format!(
                "{:?}",
                reference.aggregates()[0].scalar().unwrap().upwelling_top()[0]
            ),
        ];
        assert_eq!(
            pinned,
            [
                "0.051914114460382",
                "-0.005734264674987712",
                "3.1510023629621357e-6",
                "0.09834999038845736",
                "0.0004741472848915612",
                "0.5466892681652658"
            ]
        );
    }

    #[test]
    fn atmosphere_sunward_draws_keep_a_forward_peaked_slab_unbiased() {
        // Henyey–Greenstein at g 0.9 as a table, a conservative slab of τ 1 over a black ground
        // (550 nm) and a white one (440 nm), so that every vertex draws towards the suns: energy
        // is conserved to 4σ, as in the Rayleigh slab, and two suns in different directions give
        // the sum of the two lit alone, whose paths draw towards other suns, to 4σ.
        let terms = json!([{
            "name": "haze", "density": uniform(),
            "scattering": [1e-3, 1e-3], "absorption": [0.0, 0.0],
            "phase": case::tests::tabulated_phase_json(512, 2, |c| {
                [optics::tests::henyey_greenstein(0.9, c), 0.0, 0.0, 0.0, 0.0, 0.0]
            })
        }]);
        let mut case = slab(&terms, &json!([0.0, 1.0]), &json!([]));
        case["aggregates"] = json!([
            { "name": "high", "sunDirections": [{ "zenithDeg": 20.0, "azimuthDeg": 0.0 }] },
            { "name": "low", "sunDirections": [{ "zenithDeg": 65.0, "azimuthDeg": 0.0 }] }
        ]);
        // The diffuse flux at the ground carries the aureole's noise, which the hemisphere's draws
        // do not aim at: σ is 1.4% of the incident flux at 20,000 samples with the sun high, and
        // 1.3% at 80,000 with it at 65° (measured), so the test holds σ under 2%. A flaw in the
        // sunward weights would be far larger: leaving out the mixture's branch alone is 25%.
        let reference = trace(&case, Polarisation::Scalar, 80_000, 4);
        for aggregate in reference.aggregates() {
            let incident = aggregate.incident_top();
            let total = aggregate.upwelling_top()[0]
                + aggregate.direct_ground()[0]
                + aggregate.diffuse_ground()[0];
            let sigma = math::hypot(
                aggregate.upwelling_top_standard_error()[0],
                aggregate.diffuse_ground_standard_error()[0],
            );
            assert!(
                (total - incident[0]).abs() < 4.0 * sigma && sigma < 0.02 * incident[0],
                "{}: {total} against {} ± {sigma}",
                aggregate.name(),
                incident[0]
            );
            let (up, sigma) = (
                aggregate.upwelling_top()[1],
                aggregate.upwelling_top_standard_error()[1],
            );
            assert!(
                (up - incident[1]).abs() < 4.0 * sigma,
                "{}: {up} against {} ± {sigma}",
                aggregate.name(),
                incident[1]
            );
        }
        let views = json!([
            geometry("down", 0.0, (30.0, 40.0), (20.0, 0.0)),
            geometry("up", 1000.0, (150.0, 200.0), (20.0, 0.0))
        ]);
        let with_suns = |suns: &[(f64, f64, f64)]| {
            let mut case = slab(&terms, &json!([0.0, 0.0]), &views);
            case["suns"] = json!(
                suns.iter()
                    .enumerate()
                    .map(|(k, s)| json!({ "name": format!("sun-{k}"), "irradiance": [s.2, s.2] }))
                    .collect::<Vec<Value>>()
            );
            for g in 0..2 {
                case["geometries"][g]["sunDirections"] = json!(
                    suns.iter()
                        .map(|s| json!({ "zenithDeg": s.0, "azimuthDeg": s.1 }))
                        .collect::<Vec<Value>>()
                );
            }
            trace(&case, Polarisation::Scalar, 60_000, 4)
        };
        // The view down from the top is dim under so forward a haze, and its σ falls slowly: 3.5%
        // at 20,000 samples and 2.7% at 60,000 (measured), so the test holds σ under 4%; the view
        // up from the ground is near 1%.
        let (a, b) = ((20.0, 0.0, 1.0), (60.0, 140.0, 0.5));
        let [both, only_a, only_b] = [vec![a, b], vec![a], vec![b]].map(|s| with_suns(&s));
        for ((g, ga), gb) in both
            .geometries()
            .iter()
            .zip(only_a.geometries())
            .zip(only_b.geometries())
        {
            for w in 0..2 {
                let sum = ga.radiance()[w] + gb.radiance()[w];
                let sigma = math::hypot(
                    g.standard_error()[w],
                    math::hypot(ga.standard_error()[w], gb.standard_error()[w]),
                );
                assert!(
                    (g.radiance()[w] - sum).abs() < 4.0 * sigma && sigma < 0.04 * sum,
                    "{} at {w}: {} against {sum} ± {sigma}",
                    g.name(),
                    g.radiance()[w]
                );
            }
        }
    }

    #[test]
    fn atmosphere_reference_bits_are_pinned() {
        // Two of the sample case's values to the last bit, so that a change to the estimator,
        // its draws or `births::Draws` shows here before it moves every committed reference
        // (R08.T12.b). A deliberate change updates both, and the references are traced again.
        let reference = trace(&case::tests::sample_case(), Polarisation::Scalar, 64, 1);
        let pinned = [
            format!("{:?}", reference.geometries()[0].radiance()[0]),
            format!("{:?}", reference.aggregates()[0].upwelling_top()[1]),
        ];
        // The ground view moved with R08.T12.c's sun-directed draws, which its Cornette–Shanks
        // aerosol takes where it is forward-peaked; it was 0.039731668489903935 in R08.T12.a.
        assert_eq!(pinned, ["0.04009082837728386", "0.5324772871217555"]);
    }

    #[test]
    fn atmosphere_adding_a_geometry_leaves_the_others_draws_alone() {
        let case = case::tests::sample_case();
        let mut more = case.clone();
        more["geometries"].as_array_mut().unwrap().push(geometry(
            "extra",
            0.0,
            (10.0, 0.0),
            (20.0, 0.0),
        ));
        let (before, after) = (
            trace(&case, Polarisation::Scalar, 300, 2),
            trace(&more, Polarisation::Scalar, 300, 2),
        );
        // Compared as JSON, which prints every bit.
        let json = |value: &[GeometryRadiance]| serde_json::to_string(value).unwrap();
        assert_eq!(json(before.geometries()), json(&after.geometries()[..2]));
        let json = |value: &[FluxAggregate]| serde_json::to_string(value).unwrap();
        assert_eq!(json(before.aggregates()), json(after.aggregates()));
    }

    #[test]
    fn atmosphere_two_suns_add() {
        // The draws do not depend on the suns' irradiance, so the paths are the same and the
        // radiances and fluxes add to rounding.
        let mut both = case::tests::sample_case();
        both["suns"] = json!([
            { "name": "a", "irradiance": [1.86, 1.95] },
            { "name": "b", "irradiance": [0.4, 0.2] }
        ]);
        let second = json!({ "zenithDeg": 70.0, "azimuthDeg": 120.0 });
        for g in 0..2 {
            both["geometries"][g]["sunDirections"]
                .as_array_mut()
                .unwrap()
                .push(second.clone());
        }
        both["aggregates"][0]["sunDirections"]
            .as_array_mut()
            .unwrap()
            .push(second);
        let mut only_a = both.clone();
        only_a["suns"][1]["irradiance"] = json!([0.0, 0.0]);
        let mut only_b = both.clone();
        only_b["suns"][0]["irradiance"] = json!([0.0, 0.0]);
        let [both, a, b] = [both, only_a, only_b].map(|c| trace(&c, Polarisation::Scalar, 400, 2));
        let close = |sum: f64, x: f64, y: f64| (x + y - sum).abs() <= 1e-12 * sum.abs();
        for ((g, ga), gb) in both
            .geometries()
            .iter()
            .zip(a.geometries())
            .zip(b.geometries())
        {
            for w in 0..2 {
                let (sum, x, y) = (g.radiance()[w], ga.radiance()[w], gb.radiance()[w]);
                assert!(
                    sum > x && sum > y && close(sum, x, y),
                    "{}: {sum} {x} {y}",
                    g.name()
                );
            }
            let json = |depths: &[Vec<Option<f64>>]| serde_json::to_string(depths).unwrap();
            assert_eq!(json(g.sun_optical_depth()), json(ga.sun_optical_depth()));
            assert_eq!(g.sun_optical_depth().len(), 2);
        }
        let (fa, fb, f) = (
            &a.aggregates()[0],
            &b.aggregates()[0],
            &both.aggregates()[0],
        );
        for w in 0..2 {
            assert!(close(
                f.incident_top()[w],
                fa.incident_top()[w],
                fb.incident_top()[w]
            ));
            assert!(close(
                f.direct_ground()[w],
                fa.direct_ground()[w],
                fb.direct_ground()[w]
            ));
            assert!(close(
                f.diffuse_ground()[w],
                fa.diffuse_ground()[w],
                fb.diffuse_ground()[w]
            ));
            assert!(close(
                f.upwelling_top()[w],
                fa.upwelling_top()[w],
                fb.upwelling_top()[w]
            ));
        }
    }

    #[test]
    fn atmosphere_stokes_intensity_equals_scalar_for_an_isotropic_scatterer() {
        let mut case = case::tests::sample_case();
        case["terms"] = json!([{
            "name": "dust",
            "density": { "kind": "exponential", "scaleHeightM": 3000.0 },
            "scattering": [2e-5, 3e-5],
            "absorption": [1e-6, 2e-6],
            "phase": { "kind": "isotropic" }
        }]);
        let scalar = trace(&case, Polarisation::Scalar, 2000, 4);
        let stokes = trace(&case, Polarisation::Stokes, 2000, 4);
        for (s, v) in scalar.geometries().iter().zip(stokes.geometries()) {
            let polarised = v.stokes().unwrap();
            for w in 0..2 {
                let i = s.radiance()[w];
                assert!(i > 0.0, "{}", s.name());
                assert!(
                    (v.radiance()[w] - i).abs() <= 1e-12 * i,
                    "{}: {} {i}",
                    s.name(),
                    v.radiance()[w]
                );
                assert!(polarised.q()[w].abs() <= 1e-15 * i && polarised.u()[w].abs() <= 1e-15 * i);
            }
        }
        for (s, v) in scalar.aggregates().iter().zip(stokes.aggregates()) {
            for w in 0..2 {
                let (a, b) = (s.upwelling_top()[w], v.upwelling_top()[w]);
                assert!((a - b).abs() <= 1e-12 * a, "{a} {b}");
                let (a, b) = (s.diffuse_ground()[w], v.diffuse_ground()[w]);
                assert!((a - b).abs() <= 1e-12 * a, "{a} {b}");
            }
        }
    }

    #[test]
    fn atmosphere_stokes_mode_and_sample_counts_are_refused() {
        let case = case_of(&case::tests::sample_case());
        let result = trace_reference(
            &case,
            Polarisation::Stokes,
            NonZeroU64::MIN,
            NonZeroUsize::MIN,
        );
        match result {
            Err(TraceReferenceError::StokesNeedsMatrix { term }) => assert_eq!(term, "aerosol"),
            other => panic!("{other:?}"),
        }
        // Marked depolarising, the same aerosol is traced.
        let mut marked = case::tests::sample_case();
        marked["terms"][1]["depolarising"] = json!(true);
        let reference = trace(&marked, Polarisation::Stokes, 8, 1);
        assert!(reference.geometries()[0].stokes().is_some());
        assert!(reference.aggregates()[0].scalar().is_some());
        let too_many = NonZeroU64::new(MAX_SAMPLES + 1).unwrap();
        let result = trace_reference(&case, Polarisation::Scalar, too_many, NonZeroUsize::MIN);
        match result {
            Err(error @ TraceReferenceError::TooManySamples { samples }) => {
                assert_eq!(samples, MAX_SAMPLES + 1);
                assert_eq!(crate::RunFitError::from(error).exit_code(), 2);
            }
            other => panic!("{other:?}"),
        }
    }

    /// Asserts that every radiance of the geometries of `a` and `b` agrees to `relative`.
    fn assert_same_radiances(a: &ReferenceRadiances, b: &ReferenceRadiances, relative: f64) {
        for (x, y) in a.geometries().iter().zip(b.geometries()) {
            for w in 0..x.radiance().len() {
                let (i, j) = (x.radiance()[w], y.radiance()[w]);
                assert!(
                    (i - j).abs() <= relative * i.abs(),
                    "{} at {w}: {i} {j}",
                    x.name()
                );
            }
        }
    }

    #[test]
    fn atmosphere_stokes_intensity_equals_scalar_for_depolarising_terms() {
        // R08.T12.c: a Cornette–Shanks aerosol, a table without a matrix and a Legendre series,
        // each marked depolarising beside an isotropic gas, scatter by a₁ alone: the Stokes
        // mode's I is the scalar mode's to rounding, and no polarisation arises.
        let mut case = case::tests::sample_case();
        let mut bare = case::tests::tabulated_phase_json(128, 2, |c| {
            let p = optics::tests::henyey_greenstein(0.6, c);
            [p, 0.0, 0.0, 0.0, 0.0, 0.0]
        });
        bare.as_object_mut().unwrap().remove("matrix");
        case["terms"][0]["phase"] = json!({ "kind": "isotropic" });
        case["terms"][1]["depolarising"] = json!(true);
        case["terms"][2] = json!({
            "name": "haze", "density": { "kind": "exponential", "scaleHeightM": 3000.0 },
            "scattering": [3e-6, 4e-6], "absorption": [1e-7, 1e-7],
            "phase": bare, "depolarising": true
        });
        case["terms"].as_array_mut().unwrap().push(json!({
            "name": "dust", "density": { "kind": "exponential", "scaleHeightM": 2000.0 },
            "scattering": [2e-6, 2e-6], "absorption": [0.0, 0.0],
            "phase": { "kind": "legendre", "coefficients": [[1.0, 1.2, 0.5], [1.0, 0.9]] },
            "depolarising": true
        }));
        let scalar = trace(&case, Polarisation::Scalar, 1500, 4);
        let stokes = trace(&case, Polarisation::Stokes, 1500, 4);
        assert_same_radiances(&scalar, &stokes, 1e-12);
        for g in stokes.geometries() {
            let polarised = g.stokes().unwrap();
            for w in 0..2 {
                let largest = polarised.q()[w]
                    .abs()
                    .max(polarised.u()[w].abs())
                    .max(polarised.v()[w].abs());
                assert!(largest <= 1e-15 * g.radiance()[w], "{}", g.name());
            }
        }
    }

    #[test]
    fn atmosphere_stokes_scalar_radiance_is_the_scalar_modes_to_the_bit() {
        // The scalar radiance a Stokes run reports beside I is the scalar mode's estimate from the
        // same draws, bit for bit, fluxes included; its difference from I has its own error.
        let mut case = case::tests::sample_case();
        case["terms"][1]["depolarising"] = json!(true);
        let scalar = trace(&case, Polarisation::Scalar, 600, 2);
        let stokes = trace(&case, Polarisation::Stokes, 600, 2);
        let json = |values: &[f64]| serde_json::to_string(values).unwrap();
        for (s, v) in scalar.geometries().iter().zip(stokes.geometries()) {
            let polarised = v.stokes().unwrap();
            assert_eq!(json(s.radiance()), json(polarised.scalar_radiance()));
            assert_eq!(
                json(s.standard_error()),
                json(polarised.scalar_standard_error())
            );
            for w in 0..2 {
                // Rayleigh's scalar error moves I, and the difference is far better resolved than
                // either radiance, since the two are scored on the same paths.
                let difference = polarised.difference_standard_error()[w];
                assert!(difference > 0.0 && difference < 0.5 * v.standard_error()[w]);
            }
        }
        for (s, v) in scalar.aggregates().iter().zip(stokes.aggregates()) {
            let flux = v.scalar().unwrap();
            assert_eq!(json(s.diffuse_ground()), json(flux.diffuse_ground()));
            assert_eq!(json(s.upwelling_top()), json(flux.upwelling_top()));
            assert_eq!(
                json(s.upwelling_top_standard_error()),
                json(flux.upwelling_top_standard_error())
            );
            assert!(flux.diffuse_ground_difference_standard_error()[0] > 0.0);
        }
        assert!(
            scalar.geometries()[0].stokes().is_none() && scalar.aggregates()[0].scalar().is_none()
        );
    }

    /// An independent frame for the polarisation tests: east, north and up the unit axes. The
    /// direction at `(zenith, azimuth)` degrees, and the detector's first Stokes axis e₁ for a
    /// view there, towards the larger zenith angle in the view's vertical plane.
    fn direction_and_e1(zenith_azimuth: (f64, f64)) -> ([f64; 3], [f64; 3]) {
        let (zenith, azimuth) = (radians(zenith_azimuth.0), radians(zenith_azimuth.1));
        (
            [
                math::sin(zenith) * math::sin(azimuth),
                math::sin(zenith) * math::cos(azimuth),
                math::cos(zenith),
            ],
            [
                math::cos(zenith) * math::sin(azimuth),
                math::cos(zenith) * math::cos(azimuth),
                -math::sin(zenith),
            ],
        )
    }

    fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    /// (Q ÷ I, U ÷ I) of light polarised to degree `degree` along the scattering plane's normal,
    /// for unpolarised light from `sun` singly scattered into the view `view`, in the detector's
    /// frame (e₁, e₂ = k × e₁, k = −look).
    fn single_scattering_polarisation(
        sun: (f64, f64),
        view: (f64, f64),
        degree: f64,
    ) -> (f64, f64) {
        let (towards_sun, _) = direction_and_e1(sun);
        let (look, e1) = direction_and_e1(view);
        let e2 = cross3(look.map(|x| -x), e1);
        let normal = cross3(towards_sun, look);
        let length = dot3(normal, normal).sqrt();
        let normal = normal.map(|x| x / length);
        let (along_e1, along_e2) = (dot3(normal, e1), dot3(normal, e2));
        (
            degree * (along_e1 * along_e1 - along_e2 * along_e2),
            degree * 2.0 * along_e1 * along_e2,
        )
    }

    /// The matrix of the dipole sphere of `optics`'s tests with amplitudes `a` and `b`
    /// (unnormalised).
    fn dipole_sphere(a: (f64, f64), b: (f64, f64)) -> impl Fn(f64) -> [f64; 6] {
        move |c| {
            let [a1, b1, a3, b2] = optics::tests::dipole_sphere(a, b, c);
            [a1, a1, a3, a3, b1, b2]
        }
    }

    #[test]
    fn atmosphere_a_spheres_single_scattering_degree_of_polarisation_is_minus_b1_over_a1() {
        // R08.T12.c: a thin slab of a sphere whose amplitudes are an electric and a magnetic
        // dipole's, tabulated with its matrix: singly scattered light is polarised to −b₁ ÷ a₁ at
        // its scattering angle and carries no V. The degree is (|a|² − |b|²) sin²Θ ÷ (|S₁|² +
        // |S₂|²), so the electric dipole's lead polarises along the scattering plane's normal and
        // the magnetic one's in the plane: both are traced.
        let sun = (40.0, 0.0);
        let views = [(45.0, 180.0), (50.0, 90.0), (70.0, 300.0), (130.0, 20.0)];
        let geometries: Vec<Value> = views
            .iter()
            .enumerate()
            .map(|(i, &view)| {
                let height = if view.0 > 90.0 { 1000.0 } else { 0.0 };
                geometry(&format!("view-{i}"), height, view, sun)
            })
            .collect();
        let (electric, magnetic) = ((0.4, -0.9), (0.3, 0.5));
        for (a, b, sign) in [(electric, magnetic, 1.0), (magnetic, electric, -1.0)] {
            let sphere = dipole_sphere(a, b);
            let terms = json!([{
                "name": "spheres", "density": uniform(),
                "scattering": [1e-8, 1e-8], "absorption": [0.0, 0.0],
                "phase": case::tests::tabulated_phase_json(512, 2, &sphere)
            }]);
            let reference = trace(
                &slab(&terms, &json!([0.0, 0.0]), &json!(geometries)),
                Polarisation::Stokes,
                500,
                2,
            );
            for (traced, &view) in reference.geometries().iter().zip(&views) {
                let (towards_sun, _) = direction_and_e1(sun);
                let (look, _) = direction_and_e1(view);
                let [a1, _, _, _, b1, _] = sphere(dot3(towards_sun, look));
                let degree = -b1 / a1;
                assert!(sign * degree > 0.05, "{}: {degree}", traced.name());
                let (q_expected, u_expected) = single_scattering_polarisation(sun, view, degree);
                let stokes = traced.stokes().unwrap();
                for w in 0..2 {
                    let intensity = traced.radiance()[w];
                    let (q, u, v) = (
                        stokes.q()[w] / intensity,
                        stokes.u()[w] / intensity,
                        stokes.v()[w] / intensity,
                    );
                    // The table's interpolation, 2 × 10⁻⁴, and 4σ for a rare second scattering.
                    let sigma = |e: &[f64]| 2e-4 + 4.0 * e[w] / intensity;
                    assert!(
                        (q - q_expected).abs() < sigma(stokes.q_standard_error())
                            && (u - u_expected).abs() < sigma(stokes.u_standard_error()),
                        "{} at {w}: ({q}, {u}) against ({q_expected}, {u_expected})",
                        traced.name()
                    );
                    assert!(
                        v.abs() < sigma(stokes.v_standard_error()),
                        "{}: V {v}",
                        traced.name()
                    );
                }
            }
        }
    }

    #[test]
    fn atmosphere_a_small_spheres_tabulated_matrix_traces_as_rayleigh() {
        // R08.T12.c: a thin slab of small spheres, their matrix tabulated from the amplitudes'
        // limit, against the built-in Rayleigh matrix at ρ = 0: the same I, Q, U and V to the
        // table's interpolation, 10⁻⁴ of I, and 4σ of the two runs. The first scattering is the
        // same in both, from the same draws; the paths part after it, and a rare second
        // scattering (some 10⁻⁵ of the paths) scores as much as a first, which σ carries.
        let views = [(45.0, 180.0), (60.0, 70.0), (120.0, 250.0)];
        let geometries: Vec<Value> = views
            .iter()
            .enumerate()
            .map(|(i, &view)| {
                let height = if view.0 > 90.0 { 1000.0 } else { 0.0 };
                geometry(&format!("view-{i}"), height, view, (35.0, 10.0))
            })
            .collect();
        let slab_of = |phase: Value| {
            let terms = json!([{
                "name": "scatterers", "density": uniform(),
                "scattering": [1e-8, 1e-8], "absorption": [0.0, 0.0], "phase": phase
            }]);
            trace(
                &slab(&terms, &json!([0.0, 0.0]), &json!(geometries)),
                Polarisation::Stokes,
                500,
                2,
            )
        };
        let built_in = slab_of(json!({ "kind": "rayleigh", "depolarisation": [0.0, 0.0] }));
        let tabulated = slab_of(case::tests::tabulated_phase_json(
            256,
            2,
            case::tests::small_sphere,
        ));
        for (a, b) in built_in.geometries().iter().zip(tabulated.geometries()) {
            let (sa, sb) = (a.stokes().unwrap(), b.stokes().unwrap());
            for w in 0..2 {
                let i = a.radiance()[w];
                for (x, y, sigma) in [
                    (
                        i,
                        b.radiance()[w],
                        math::hypot(a.standard_error()[w], b.standard_error()[w]),
                    ),
                    (
                        sa.q()[w],
                        sb.q()[w],
                        math::hypot(sa.q_standard_error()[w], sb.q_standard_error()[w]),
                    ),
                    (
                        sa.u()[w],
                        sb.u()[w],
                        math::hypot(sa.u_standard_error()[w], sb.u_standard_error()[w]),
                    ),
                    (
                        sa.v()[w],
                        sb.v()[w],
                        math::hypot(sa.v_standard_error()[w], sb.v_standard_error()[w]),
                    ),
                ] {
                    assert!(
                        (x - y).abs() < 1e-4 * i + 4.0 * sigma,
                        "{} at {w}: {x} {y} ± {sigma}",
                        a.name()
                    );
                }
            }
        }
    }

    #[test]
    fn atmosphere_circular_polarisation_is_odd_in_azimuth_and_absent_without_b2() {
        // V arises only from U through b₂ in multiple scattering (the coupling itself is
        // `transport`'s test). Over a plane-parallel slab lit by unpolarised light, V and U are odd
        // in the relative azimuth and I and Q even (the mirror symmetry of the principal plane,
        // which turns e₂ = k × e₁ over): at ±60° they cancel to 4σ. V is small, some 10⁻³ of I
        // here, as in Kokhanovsky et al. 2010's tables (10⁻⁴ to 10⁻³), and not resolved at this
        // cost. Rayleigh, whose b₂ is 0, makes none at all.
        let sphere = dipole_sphere((0.4, -0.9), (0.3, 0.5));
        let terms = json!([{
            "name": "spheres", "density": uniform(),
            "scattering": [1e-3, 1e-3], "absorption": [0.0, 0.0],
            "phase": case::tests::tabulated_phase_json(512, 2, &sphere)
        }]);
        let geometries = json!([
            geometry("bottom-plus", 0.0, (50.0, 60.0), (40.0, 0.0)),
            geometry("bottom-minus", 0.0, (50.0, -60.0), (40.0, 0.0)),
            geometry("top-plus", 1000.0, (130.0, 60.0), (40.0, 0.0)),
            geometry("top-minus", 1000.0, (130.0, -60.0), (40.0, 0.0))
        ]);
        let reference = trace(
            &slab(&terms, &json!([0.0, 0.0]), &geometries),
            Polarisation::Stokes,
            20_000,
            4,
        );
        for pair in reference.geometries().chunks(2) {
            let (plus, minus) = (pair[0].stokes().unwrap(), pair[1].stokes().unwrap());
            for w in 0..2 {
                let sigma = math::hypot(plus.v_standard_error()[w], minus.v_standard_error()[w]);
                let (v_plus, v_minus) = (plus.v()[w], minus.v()[w]);
                assert!(
                    (v_plus + v_minus).abs() < 4.0 * sigma,
                    "{}: V {v_plus} and {v_minus} ± {sigma}",
                    pair[0].name()
                );
                let sigma = math::hypot(plus.u_standard_error()[w], minus.u_standard_error()[w]);
                assert!((plus.u()[w] + minus.u()[w]).abs() < 4.0 * sigma);
                let sigma = math::hypot(plus.q_standard_error()[w], minus.q_standard_error()[w]);
                assert!((plus.q()[w] - minus.q()[w]).abs() < 4.0 * sigma);
            }
        }
        let rayleigh = json!([{
            "name": "gas", "density": uniform(),
            "scattering": [1e-3, 1e-3], "absorption": [0.0, 0.0],
            "phase": { "kind": "rayleigh", "depolarisation": [0.0, 0.03] }
        }]);
        let reference = trace(
            &slab(&rayleigh, &json!([0.0, 0.3]), &geometries),
            Polarisation::Stokes,
            500,
            2,
        );
        for g in reference.geometries() {
            assert!(
                g.stokes()
                    .unwrap()
                    .v()
                    .iter()
                    .all(|v| v.abs() < f64::MIN_POSITIVE)
            );
        }
    }

    #[test]
    fn atmosphere_a_legendre_series_traces_as_a_table_of_the_same_function() {
        // Henyey–Greenstein at g = 0.8 as a Legendre series (βₗ = (2l + 1) gˡ, 300 terms), drawn
        // from its proposal and weighted, and as a table of 1,024 entries, drawn exactly: in a
        // slab of τ = 1 the radiances agree to 4σ. (Both draw from tables, so their paths are
        // close and the comparison is lenient; the forward peak leaves σ near 2% away from the
        // sun at 40,000 samples.) Rayleigh's series, β = (1, 0, ½), against the built-in phase
        // function's exact draws is the independent check: there the paths differ, and the two
        // agree to 4σ at σ near 0.5%.
        let g: f64 = 0.8;
        let mut power = 1.0;
        let beta: Vec<f64> = (0..300)
            .map(|l| {
                let b = f64::from(2 * l + 1) * power;
                power *= g;
                b
            })
            .collect();
        let table = case::tests::tabulated_phase_json(1024, 2, |c| {
            [
                optics::tests::henyey_greenstein(g, c),
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ]
        });
        let geometries = json!([
            geometry("down-near", 0.0, (30.0, 20.0), (40.0, 0.0)),
            geometry("down-far", 0.0, (70.0, 180.0), (40.0, 0.0)),
            geometry("up", 1000.0, (140.0, 90.0), (40.0, 0.0))
        ]);
        let slab_of = |phase: Value| {
            let terms = json!([{
                "name": "haze", "density": uniform(),
                "scattering": [1e-3, 1e-3], "absorption": [0.0, 2e-4], "phase": phase
            }]);
            trace(
                &slab(&terms, &json!([0.0, 0.2]), &geometries),
                Polarisation::Scalar,
                40_000,
                4,
            )
        };
        let series = slab_of(json!({ "kind": "legendre", "coefficients": [beta.clone(), beta] }));
        let tabulated = slab_of(table);
        let rayleigh_series = slab_of(json!({
            "kind": "legendre", "coefficients": [[1.0, 0.0, 0.5], [1.0, 0.0, 0.5]]
        }));
        let rayleigh = slab_of(json!({ "kind": "rayleigh", "depolarisation": [0.0, 0.0] }));
        for (a, b) in series
            .geometries()
            .iter()
            .zip(tabulated.geometries())
            .chain(
                rayleigh_series
                    .geometries()
                    .iter()
                    .zip(rayleigh.geometries()),
            )
        {
            for w in 0..2 {
                let (x, y) = (a.radiance()[w], b.radiance()[w]);
                let sigma = math::hypot(a.standard_error()[w], b.standard_error()[w]);
                assert!(sigma < 0.05 * x, "{}: {x} ± {sigma}", a.name());
                assert!(
                    (x - y).abs() < 4.0 * sigma,
                    "{} at {w}: {x} against {y} ± {sigma}",
                    a.name()
                );
            }
        }
    }

    /// cos Θ between the directions at `(zenith, azimuth)` and `(zenith, azimuth)`, degrees, by
    /// the spherical law of cosines.
    fn cos_between(a: (f64, f64), b: (f64, f64)) -> f64 {
        let (za, zb) = (radians(a.0), radians(b.0));
        math::cos(za) * math::cos(zb)
            + math::sin(za) * math::sin(zb) * math::cos(radians(a.1 - b.1))
    }

    /// Rayleigh's phase function of depolarisation ρ, sr⁻¹ (Chandrasekhar 1950).
    fn rayleigh(rho: f64, cos_theta: f64) -> f64 {
        let gamma = rho / (2.0 - rho);
        3.0 / (4.0 * (1.0 + 2.0 * gamma))
            * ((1.0 + 3.0 * gamma) + (1.0 - gamma) * cos_theta * cos_theta)
            / (4.0 * PI)
    }

    #[test]
    fn atmosphere_thin_layer_matches_single_scattering() {
        // τₛ = 10⁻⁵ and τₐ = 5 × 10⁻⁶: multiple scattering is some 10⁻⁵ of the radiance.
        let (sigma_s, sigma_a, top) = (1e-8, 5e-9, 1000.0);
        let rho = [0.0, 0.03];
        let terms = json!([{
            "name": "gas", "density": uniform(),
            "scattering": [sigma_s, sigma_s], "absorption": [sigma_a, sigma_a],
            "phase": { "kind": "rayleigh", "depolarisation": rho }
        }]);
        let (down_view, down_sun) = ((60.0, 90.0), (30.0, 0.0));
        let (up_view, up_sun) = ((120.0, 45.0), (50.0, 200.0));
        let (side_view, side_sun, side_height, distance) =
            ((90.0, 0.0), (40.0, 30.0), 500.0, 1000.0);
        let mut side = geometry("side", side_height, side_view, side_sun);
        side["maxDistanceM"] = json!(distance);
        let geometries = json!([
            geometry("down", 0.0, down_view, down_sun),
            geometry("up", top, up_view, up_sun),
            side
        ]);
        let reference = trace(
            &slab(&terms, &json!([0.0, 0.0]), &geometries),
            Polarisation::Scalar,
            2000,
            4,
        );
        let tau = (sigma_s + sigma_a) * top;
        let omega = sigma_s / (sigma_s + sigma_a);
        let irradiance = [PI, 2.0];
        for (w, (&depolarisation, &irradiance)) in rho.iter().zip(&irradiance).enumerate() {
            let f = irradiance * omega;
            let phase = |view, sun| rayleigh(depolarisation, cos_between(view, sun));
            // Downwelling at the ground: F ω p μ₀ ÷ (μ₀ − μ) (e^(−τ/μ₀) − e^(−τ/μ)).
            let (mu, mu0) = (
                math::cos(radians(down_view.0)),
                math::cos(radians(down_sun.0)),
            );
            let down = f * phase(down_view, down_sun) * mu0 / (mu0 - mu)
                * (math::exp(-tau / mu0) - math::exp(-tau / mu));
            // Upwelling at the top: F ω p μ₀ ÷ (μ₀ + μ) (1 − e^(−τ(1/μ₀ + 1/μ))).
            let (mu, mu0) = (-math::cos(radians(up_view.0)), math::cos(radians(up_sun.0)));
            let up = f * phase(up_view, up_sun) * mu0 / (mu0 + mu)
                * -math::exp_m1(-tau * (1.0 / mu0 + 1.0 / mu));
            // Horizontally to a black target: F ω p e^(−τ_sun) (1 − e^(−σ_t D)).
            let mu0 = math::cos(radians(side_sun.0));
            let sun_depth = (sigma_s + sigma_a) * (top - side_height) / mu0;
            let side = f
                * phase(side_view, side_sun)
                * math::exp(-sun_depth)
                * -math::exp_m1(-(sigma_s + sigma_a) * distance);
            for (traced, expected) in reference.geometries().iter().zip([down, up, side]) {
                let radiance = traced.radiance()[w];
                assert!(
                    (radiance / expected - 1.0).abs() < 1e-4,
                    "{} at {w}: {radiance} against {expected}",
                    traced.name()
                );
            }
        }
    }

    #[test]
    fn atmosphere_single_scattering_polarisation_has_rayleighs_degree_and_plane() {
        let rho = [0.0, 0.03];
        let terms = json!([{
            "name": "gas", "density": uniform(),
            "scattering": [1e-8, 1e-8], "absorption": [0.0, 0.0],
            "phase": { "kind": "rayleigh", "depolarisation": rho }
        }]);
        let sun = (45.0, 0.0);
        let views = [(45.0, 180.0), (45.0, 90.0), (70.0, 300.0)];
        let geometries: Vec<Value> = views
            .iter()
            .enumerate()
            .map(|(i, &view)| geometry(&format!("view-{i}"), 0.0, view, sun))
            .collect();
        let reference = trace(
            &slab(&terms, &json!([0.0, 0.0]), &json!(geometries)),
            Polarisation::Stokes,
            500,
            2,
        );
        // An independent frame: east, north and up the unit axes.
        let direction = |(zenith, azimuth): (f64, f64)| {
            let (zenith, azimuth) = (radians(zenith), radians(azimuth));
            [
                math::sin(zenith) * math::sin(azimuth),
                math::sin(zenith) * math::cos(azimuth),
                math::cos(zenith),
            ]
        };
        let cross = |a: [f64; 3], b: [f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let towards_sun = direction(sun);
        for (traced, &view) in reference.geometries().iter().zip(&views) {
            let look = direction(view);
            // The detector's frame: e₁ towards the larger zenith angle in the view's vertical
            // plane, e₂ = k × e₁ for the light's direction k = −look.
            let (zenith, azimuth) = (radians(view.0), radians(view.1));
            let e1 = [
                math::cos(zenith) * math::sin(azimuth),
                math::cos(zenith) * math::cos(azimuth),
                -math::sin(zenith),
            ];
            let e2 = cross(look.map(|x| -x), e1);
            // Singly scattered light is polarised along the scattering plane's normal.
            let normal = cross(towards_sun, look);
            let length = dot(normal, normal).sqrt();
            let normal = normal.map(|x| x / length);
            let (along_e1, along_e2) = (dot(normal, e1), dot(normal, e2));
            let cos_theta = dot(towards_sun, look);
            let stokes = traced.stokes().unwrap();
            for (w, &depolarisation) in rho.iter().enumerate() {
                let delta = (1.0 - depolarisation) / (1.0 + 0.5 * depolarisation);
                let degree = 0.75 * delta * (1.0 - cos_theta * cos_theta)
                    / (0.75 * delta * (1.0 + cos_theta * cos_theta) + 1.0 - delta);
                let intensity = traced.radiance()[w];
                let (q, u) = (stokes.q()[w] / intensity, stokes.u()[w] / intensity);
                let q_expected = degree * (along_e1 * along_e1 - along_e2 * along_e2);
                let u_expected = degree * 2.0 * along_e1 * along_e2;
                assert!(
                    (q - q_expected).abs() < 1e-4 && (u - u_expected).abs() < 1e-4,
                    "{} at {w}: ({q}, {u}) against ({q_expected}, {u_expected})",
                    traced.name()
                );
            }
        }
        // At 90° in the principal plane, −(1 − ρ) ÷ (1 + ρ) and no U.
        let principal = &reference.geometries()[0];
        for (w, &depolarisation) in rho.iter().enumerate() {
            let q = principal.stokes().unwrap().q()[w] / principal.radiance()[w];
            let expected = -(1.0 - depolarisation) / (1.0 + depolarisation);
            assert!((q - expected).abs() < 1e-4, "{q} against {expected}");
        }
    }

    #[test]
    fn atmosphere_conservative_layer_conserves_energy() {
        // τ = 1 of Rayleigh scattering over a black ground (550 nm) and a white one (440 nm).
        let terms = json!([{
            "name": "gas", "density": uniform(),
            "scattering": [1e-3, 1e-3], "absorption": [0.0, 0.0],
            "phase": { "kind": "rayleigh", "depolarisation": [0.0, 0.0] }
        }]);
        let mut case = slab(&terms, &json!([0.0, 1.0]), &json!([]));
        case["aggregates"] = json!([
            { "name": "zenith", "sunDirections": [{ "zenithDeg": 0.0, "azimuthDeg": 0.0 }] },
            { "name": "low", "sunDirections": [{ "zenithDeg": 60.0, "azimuthDeg": 0.0 }] }
        ]);
        let reference = trace(&case, Polarisation::Scalar, 20_000, 4);
        for aggregate in reference.aggregates() {
            let incident = aggregate.incident_top();
            // Black ground: what is not reflected is transmitted.
            let total = aggregate.upwelling_top()[0]
                + aggregate.direct_ground()[0]
                + aggregate.diffuse_ground()[0];
            let sigma = math::hypot(
                aggregate.upwelling_top_standard_error()[0],
                aggregate.diffuse_ground_standard_error()[0],
            );
            assert!(
                (total - incident[0]).abs() < 4.0 * sigma,
                "{}: {total} against {} ± {sigma}",
                aggregate.name(),
                incident[0]
            );
            assert!(sigma < 0.01 * incident[0], "{sigma}");
            // White ground: everything comes back up.
            let up = aggregate.upwelling_top()[1];
            let sigma = aggregate.upwelling_top_standard_error()[1];
            assert!(
                (up - incident[1]).abs() < 4.0 * sigma,
                "{}: {up} against {} ± {sigma}",
                aggregate.name(),
                incident[1]
            );
        }
    }

    /// The Earth-like sample case over the spheroid a = c = its sphere's radius with ω = 0, at
    /// `seed`. R05's `tableRadiusM`, (2a + c) ÷ 3, is that radius too.
    fn sample_case_as_a_spheroid(seed: u64) -> Value {
        let mut case = case::tests::sample_case();
        case["shells"] = json!({
            "kind": "spheroid", "equatorialRadiusM": 6_371_000.0, "polarRadiusM": 6_371_000.0,
            "gmM3S2": 3.986_004_418e14, "angularVelocityRadS": 0.0
        });
        case["seed"] = json!(seed);
        case
    }

    /// Asserts that every radiance and flux of `a` and `b` agrees within `sigmas` of their
    /// combined standard errors plus `relative` of the value, and that their direct beams agree.
    fn assert_traces_agree(
        a: &ReferenceRadiances,
        b: &ReferenceRadiances,
        sigmas: f64,
        relative: f64,
    ) {
        let close = |x: f64, sx: f64, y: f64, sy: f64| {
            (x - y).abs() <= sigmas * math::hypot(sx, sy) + relative * x.abs()
        };
        for (g, h) in a.geometries().iter().zip(b.geometries()) {
            for w in 0..g.radiance().len() {
                let (x, y) = (g.radiance()[w], h.radiance()[w]);
                let (sx, sy) = (g.standard_error()[w], h.standard_error()[w]);
                assert!(
                    close(x, sx, y, sy),
                    "{} at {w}: {x} ± {sx} against {y} ± {sy}",
                    g.name()
                );
                let (dx, dy) = (
                    g.sun_optical_depth()[0][w].unwrap(),
                    h.sun_optical_depth()[0][w].unwrap(),
                );
                assert!(
                    (dx - dy).abs() <= 1e-9 * dx.max(dy),
                    "{} at {w}: the sun's depth {dx} against {dy}",
                    g.name()
                );
            }
        }
        for (f, k) in a.aggregates().iter().zip(b.aggregates()) {
            for w in 0..f.incident_top().len() {
                let (x, y) = (f.incident_top()[w], k.incident_top()[w]);
                assert!((x / y - 1.0).abs() < 1e-12, "{} at {w}: {x} {y}", f.name());
                let (x, y) = (f.direct_ground()[w], k.direct_ground()[w]);
                assert!((x / y - 1.0).abs() < 1e-9, "{} at {w}: {x} {y}", f.name());
                let (x, sx) = (f.diffuse_ground()[w], f.diffuse_ground_standard_error()[w]);
                let (y, sy) = (k.diffuse_ground()[w], k.diffuse_ground_standard_error()[w]);
                assert!(
                    close(x, sx, y, sy),
                    "{} at {w}: {x} ± {sx} against {y} ± {sy}",
                    f.name()
                );
                let (x, sx) = (f.upwelling_top()[w], f.upwelling_top_standard_error()[w]);
                let (y, sy) = (k.upwelling_top()[w], k.upwelling_top_standard_error()[w]);
                assert!(
                    close(x, sx, y, sy),
                    "{} at {w}: {x} ± {sx} against {y} ± {sy}",
                    f.name()
                );
            }
        }
    }

    #[test]
    fn atmosphere_spheroid_with_a_equal_to_c_agrees_with_the_sphere() {
        // R08.T12.d: a = c with ω = 0 on the Earth-like sample case, against `Shells::Sphere` of
        // the same radius. On the same draws the paths are the sphere's to rounding (measured
        // 4 × 10⁻¹³), so they agree to 10⁻⁹, far inside the task's 3σ. On independent draws (seed
        // 1) each of the 8 stochastic values is held to 4σ, a family-wise α of 5 × 10⁻⁴ (the
        // sim-determinism rule's α = 10⁻³; 3σ over 8 values would be 2%). The slow test below
        // repeats the first at 10⁵ samples.
        let sphere = trace(&case::tests::sample_case(), Polarisation::Scalar, 1500, 4);
        let same = trace(&sample_case_as_a_spheroid(0), Polarisation::Scalar, 1500, 4);
        assert_traces_agree(&sphere, &same, 0.0, 1e-9);
        let independent = trace(&sample_case_as_a_spheroid(1), Polarisation::Scalar, 1500, 4);
        assert_traces_agree(&sphere, &independent, 4.0, 0.0);
    }

    #[test]
    #[ignore = "slow: 10⁵ samples of each source, over a sphere and a spheroid; about 2 minutes on 4 threads"]
    fn atmosphere_spheroid_with_a_equal_to_c_follows_the_spheres_paths() {
        // The fast test's first comparison over 10⁵ paths a source, so that rare paths (grazing,
        // reflected many times, rouletted) are held to the sphere's too. Independent draws are not
        // compared here: their difference is the two runs' noise alone, since the paths are the
        // same, and 3σ over these 8 values fails by chance about 2% of the time (at 10⁵ samples
        // one stood at 3.2σ; R08's Risks).
        let sphere = trace(
            &case::tests::sample_case(),
            Polarisation::Scalar,
            100_000,
            4,
        );
        let spheroid = trace(
            &sample_case_as_a_spheroid(0),
            Polarisation::Scalar,
            100_000,
            4,
        );
        for g in sphere.geometries() {
            let (radiance, error) = (g.radiance()[0], g.standard_error()[0]);
            assert!(
                error < 2e-3 * radiance,
                "{}: {radiance} ± {error}",
                g.name()
            );
        }
        assert_traces_agree(&sphere, &spheroid, 0.0, 1e-9);
    }

    #[test]
    fn atmosphere_spheroid_absorbing_only_medium_gives_beer_lambert() {
        // An absorbing-only Saturn-class medium seen from orbit straight down at the equator, at
        // 45° and at the pole: the ground's reflection through the vertical column and the sun's
        // slant one, against e^(−τ) with the vertical τ = σH ÷ s(φ) (the column read at h* = s h)
        // and the sun's τ by `f64` quadrature along its chord (transport's tests).
        let (sigma, height, top) = ([3e-6, 1e-5], 47_000.0, 40.0 * 47_000.0);
        let (albedo, irradiance) = ([0.3, 0.6], [1.7, 2.0]);
        let (a, c, gm, omega) = spheroid::tests::SATURN;
        let places = [
            (0.0, (60.0, 90.0)),
            (45.0, (30.0, 200.0)),
            (90.0, (60.0, 0.0)),
        ];
        let geometries: Vec<Value> = places
            .iter()
            .map(|&(latitude, sun)| {
                let mut g = geometry(&format!("orbit-{latitude}"), 5e6, (180.0, 0.0), sun);
                g["observer"]["latitudeDeg"] = json!(latitude);
                g
            })
            .collect();
        let case = json!({
            "format": 1, "name": "saturn-absorbing",
            "wavelengthsNm": [550, 440],
            "shells": {
                "kind": "spheroid", "equatorialRadiusM": a, "polarRadiusM": c,
                "gmM3S2": gm, "angularVelocityRadS": omega
            },
            "topHeightM": top,
            "groundAlbedo": albedo,
            "terms": [{
                "name": "absorber",
                "density": { "kind": "exponential", "scaleHeightM": height },
                "scattering": [0.0, 0.0], "absorption": sigma, "phase": { "kind": "none" }
            }],
            "suns": [{ "name": "sun", "irradiance": irradiance }],
            "detectorHalfAngleDeg": 0.0,
            "geometries": geometries
        });
        let reference = trace(&case, Polarisation::Scalar, 64, 2);
        let grid = shell_grid(&case_of(&case));
        // s(φ) from the client's printed gravities (spheroid's tests).
        let (gamma_e, gamma_p) = transport::tests::SATURN_GAMMA;
        let g_ref = (gamma_e * gamma_p).sqrt();
        let s_at = [
            gamma_e / g_ref,
            10.466_679_804_152_758 / g_ref,
            gamma_p / g_ref,
        ];
        for ((traced, &(latitude, sun)), s) in reference.geometries().iter().zip(&places).zip(s_at)
        {
            let frame = geometry::LocalFrame::at_latitude(radians(latitude));
            let ground = grid.point(&frame, 0.0);
            let towards_sun = frame.direction(radians(sun.0), radians(sun.1));
            for w in 0..2 {
                let vertical = sigma[w] * height / s * -math::exp_m1(-top / height);
                let slant = transport::tests::saturn_chord_depth(ground, towards_sun, top, |h| {
                    sigma[w] * math::exp(-h.max(0.0) / height)
                });
                let expected = albedo[w] / PI
                    * irradiance[w]
                    * math::cos(radians(sun.0))
                    * math::exp(-vertical - slant);
                let radiance = traced.radiance()[w];
                assert!(
                    (radiance / expected - 1.0).abs() < 1e-6,
                    "{} at {w}: {radiance} against {expected}",
                    traced.name()
                );
                assert!(traced.standard_error()[w] < 1e-12 * radiance);
            }
        }
    }

    /// A plane-parallel layer over a level spheroid: 1 km (gravity-scaled) of uniform medium over
    /// a figure of a = 10¹² m and c = 0.8a turning so that s runs from 0.93 at the equator to 1.08
    /// at the poles, where its curvature moves nothing at the precision tested.
    fn spheroid_slab(terms: &Value, albedo: &Value, geometries: &Value) -> (Value, [f64; 2]) {
        let (a, c, gm, omega) = (1e12, 0.8e12, 1e30, 4e-4);
        let figure = spheroid::LevelSpheroid::new(a, c, gm, omega, 0.0).unwrap();
        let mut case = slab(terms, albedo, geometries);
        case["shells"] = json!({
            "kind": "spheroid", "equatorialRadiusM": a, "polarRadiusM": c,
            "gmM3S2": gm, "angularVelocityRadS": omega
        });
        (
            case,
            [
                figure.gravity_ratio(1.0, 0.0),
                figure.gravity_ratio(0.0, 1.0),
            ],
        )
    }

    #[test]
    fn atmosphere_spheroid_thin_layer_matches_single_scattering_at_its_scaled_depth() {
        // The single-scattering test of the sphere at the equator and the pole of a flattened
        // plane-parallel layer: the layer's depth there is τ ÷ s, read at h* = s h.
        let (sigma_s, sigma_a, top) = (1e-8, 5e-9, 1000.0);
        let rho = [0.0, 0.03];
        let terms = json!([{
            "name": "gas", "density": uniform(),
            "scattering": [sigma_s, sigma_s], "absorption": [sigma_a, sigma_a],
            "phase": { "kind": "rayleigh", "depolarisation": rho }
        }]);
        let (down_view, down_sun) = ((60.0, 90.0), (30.0, 0.0));
        let (up_view, up_sun) = ((120.0, 45.0), (50.0, 200.0));
        let (_, [s_equator, s_pole]) = spheroid_slab(&terms, &json!([0.0, 0.0]), &json!([]));
        assert!(s_equator < 0.95 && s_pole > 1.05, "{s_equator} {s_pole}");
        let mut geometries = Vec::new();
        for (place, latitude, s) in [("equator", 0.0, s_equator), ("pole", 90.0, s_pole)] {
            for (name, height, view, sun) in [
                ("down", 0.0, down_view, down_sun),
                ("up", top / s, up_view, up_sun),
            ] {
                let mut g = geometry(&format!("{place}-{name}"), height, view, sun);
                g["observer"]["latitudeDeg"] = json!(latitude);
                geometries.push(g);
            }
        }
        let (case, _) = spheroid_slab(&terms, &json!([0.0, 0.0]), &json!(geometries));
        let reference = trace(&case, Polarisation::Scalar, 2000, 4);
        let omega = sigma_s / (sigma_s + sigma_a);
        let irradiance = [PI, 2.0];
        for (w, (&depolarisation, &irradiance)) in rho.iter().zip(&irradiance).enumerate() {
            let f = irradiance * omega;
            let phase = |view, sun| rayleigh(depolarisation, cos_between(view, sun));
            let traced = reference.geometries();
            for (pair, s) in traced.chunks(2).zip([s_equator, s_pole]) {
                let tau = (sigma_s + sigma_a) * top / s;
                let (mu, mu0) = (
                    math::cos(radians(down_view.0)),
                    math::cos(radians(down_sun.0)),
                );
                let down = f * phase(down_view, down_sun) * mu0 / (mu0 - mu)
                    * (math::exp(-tau / mu0) - math::exp(-tau / mu));
                let (mu, mu0) = (-math::cos(radians(up_view.0)), math::cos(radians(up_sun.0)));
                let up = f * phase(up_view, up_sun) * mu0 / (mu0 + mu)
                    * -math::exp_m1(-tau * (1.0 / mu0 + 1.0 / mu));
                for (traced, expected) in pair.iter().zip([down, up]) {
                    let radiance = traced.radiance()[w];
                    assert!(
                        (radiance / expected - 1.0).abs() < 1e-4,
                        "{} at {w}: {radiance} against {expected}",
                        traced.name()
                    );
                }
            }
        }
    }

    #[test]
    fn atmosphere_spheroid_conservative_layer_conserves_energy() {
        // τ ÷ s of Rayleigh scattering over a black ground (550 nm) and a white one (440 nm), at
        // the equator and the pole of the flattened layer: the aggregates' sources stand on the
        // spheroid's ground and its top, and face its vertical.
        let terms = json!([{
            "name": "gas", "density": uniform(),
            "scattering": [1e-3, 1e-3], "absorption": [0.0, 0.0],
            "phase": { "kind": "rayleigh", "depolarisation": [0.0, 0.0] }
        }]);
        let (mut case, _) = spheroid_slab(&terms, &json!([0.0, 1.0]), &json!([]));
        case["aggregates"] = json!([
            { "name": "equator", "latitudeDeg": 0.0,
              "sunDirections": [{ "zenithDeg": 30.0, "azimuthDeg": 0.0 }] },
            { "name": "pole", "latitudeDeg": 90.0,
              "sunDirections": [{ "zenithDeg": 60.0, "azimuthDeg": 0.0 }] }
        ]);
        let reference = trace(&case, Polarisation::Scalar, 10_000, 4);
        for aggregate in reference.aggregates() {
            let incident = aggregate.incident_top();
            let total = aggregate.upwelling_top()[0]
                + aggregate.direct_ground()[0]
                + aggregate.diffuse_ground()[0];
            let sigma = math::hypot(
                aggregate.upwelling_top_standard_error()[0],
                aggregate.diffuse_ground_standard_error()[0],
            );
            assert!(
                (total - incident[0]).abs() < 4.0 * sigma,
                "{}: {total} against {} ± {sigma}",
                aggregate.name(),
                incident[0]
            );
            assert!(
                sigma < 0.015 * incident[0],
                "{}: {sigma} of {}",
                aggregate.name(),
                incident[0]
            );
            let up = aggregate.upwelling_top()[1];
            let sigma = aggregate.upwelling_top_standard_error()[1];
            assert!(
                (up - incident[1]).abs() < 4.0 * sigma,
                "{}: {up} against {} ± {sigma}",
                aggregate.name(),
                incident[1]
            );
        }
        // The direct beam is e^(−τ ÷ s μ₀) of the incident flux: the layer is thinner where
        // gravity is stronger.
        let (_, [s_equator, s_pole]) = spheroid_slab(&terms, &json!([0.0, 0.0]), &json!([]));
        for (aggregate, (s, zenith)) in reference
            .aggregates()
            .iter()
            .zip([(s_equator, 30.0), (s_pole, 60.0)])
        {
            let expected = math::exp(-1.0 / (s * math::cos(radians(zenith))));
            let direct = aggregate.direct_ground()[0] / aggregate.incident_top()[0];
            assert!(
                (direct / expected - 1.0).abs() < 1e-6,
                "{}: {direct} against {expected}",
                aggregate.name()
            );
        }
    }

    #[test]
    fn atmosphere_spheroid_reference_bits_are_pinned() {
        // The spheroid's counterpart of `atmosphere_reference_bits_are_pinned`, on the
        // Saturn-class sample case: a change to the quadrics, the geodetic conversion, the
        // normal gravity or the majorants shows here before it moves a committed reference. A
        // ground view at 60°, the top flux at 45°, the view down from orbit (the entry through
        // the top), and the pole's direct beam, which no draw touches.
        let reference = trace(&case::tests::saturn_case(), Polarisation::Scalar, 64, 1);
        let pinned = [
            format!("{:?}", reference.geometries()[1].radiance()[0]),
            format!("{:?}", reference.aggregates()[0].upwelling_top()[1]),
            format!("{:?}", reference.geometries()[3].radiance()[0]),
            format!(
                "{:?}",
                reference.geometries()[2].sun_optical_depth()[0][0].unwrap()
            ),
        ];
        assert_eq!(
            pinned,
            [
                "0.2276473839689261",
                "0.8929036838129342",
                "0.2583970654014467",
                "1.3245982101763956"
            ]
        );
    }
}
