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
//! - `transport`: the estimator (absorption as a weight, delta tracking, a forced first
//!   collision, next-event estimation to every sun, a Lambertian ground, Russian roulette, Buras
//!   and Mayer 2011's VROOM where a medium is forward-peaked (R08.T12.f), the absorbers' curves of
//!   growth on each path's whole column (R08.T12.b.1) and the Stokes mode, which scores the scalar
//!   radiance of the same paths beside the vector one);
//! - `absorber`: a curve of growth read by the client's rule (R08.T12.b.1);
//! - `optics`: profiles, phase functions (closed forms, tables on u = √(Θ ÷ π) and Legendre
//!   series) and the block-diagonal scattering matrices;
//! - `geometry`: frames and the shells;
//! - `spheroid`: the level spheroid of an oblate body, its normal gravity, geodetic coordinates
//!   and quadric shells, over which the medium is read at the gravity-scaled height;
//! - [`mod@reference`]: the output and its moments;
//! - `metric`: the 5% metric's mirror, held to the client's `gate.ts` by shared values
//!   (R08.T13.a);
//! - [`sweep`]: R08.T13's sweep of single-layer media, `hyperion-fit atmosphere-sweep`, which
//!   walks each (ω, g) column of its grid to where the client's analytic multiple scattering
//!   leaves the metric (R08.T13.a);
//! - `benchmarks` (tests only): the published benchmarks the tracer is held to before its
//!   references are trusted (R08.T12.c), slow tests run by `just test-slow atmosphere::benchmarks`.
//!
//! **Reproducible for any thread count**, by the crate's convention: the samples of each geometry,
//! aggregate and wavelength are cut into blocks of [`BLOCK_SAMPLES`] mapped by
//! [`parallel::map_reduce_chunks`](crate::parallel::map_reduce_chunks), and every sample draws from
//! its own counter-based stream ([`Draws`], as the
//! displaced forms' orbits draw), so that a sample is a pure function of its key and the blocks'
//! moments are merged in index order. A sample's key is the case's seed salted with
//! [`ATMOSPHERE_STREAM`], its wavelength and its geometry or aggregate by their own indices (so
//! that adding a geometry leaves the others' draws alone), and its number. Every transcendental
//! goes through [`hyperion_sim::math`]. Nothing here is generated output: no domain tag and no
//! generator version is involved (R08's Generator version).
//!
//! **Samples per geometry** (R08.T12.b.1). A geometry or an aggregate traces its own `samples`
//! where the case gives them, and the command's count elsewhere; the key leaves one's count out of
//! the others' draws. Each records its count, and the standard error of its own first quarter of
//! samples beside the whole run's, for R08's N^(−1/2) check (`gate.ts`): the blocks are merged
//! in index order, so the first quarter's moments are a part of the run's. For the same reason a
//! geometry's run can be carried on from where it stopped, onto its moments so far, and read as one
//! traced to its new count (`trace_spans`, R08.T13.b's top-up).

mod absorber;
#[cfg(test)]
mod benchmarks;
pub mod case;
mod geometry;
pub(crate) mod metric;
mod optics;
pub mod reference;
mod spheroid;
pub mod sweep;
mod transport;

use std::num::{NonZeroU64, NonZeroUsize};

use serde::{Deserialize, Serialize};

pub use case::{AtmosphereCase, ReadCaseError, Shells};
pub use reference::{
    FluxAggregate, GeometryRadiance, ReferenceRadiances, ScalarFlux, StokesRadiance,
};

use geometry::ShellGrid;
use reference::{Moments, REFERENCE_FORMAT};
use transport::{COMPONENTS, Estimator, I, Level, Medium, Orders, SCALAR_I, Scratch, Source};

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
/// or the case's own count for a geometry or aggregate that gives one, on `threads` threads, with
/// or without the Stokes vector.
///
/// The output is the same, bit for bit, for any number of threads.
///
/// # Errors
///
/// [`TraceReferenceError::StokesNeedsMatrix`] for the Stokes mode on a case with a term that has
/// no scattering matrix and is not marked depolarising; [`TraceReferenceError::TooManySamples`]
/// past [`MAX_SAMPLES`], or for a run whose blocks of samples a `u64` cannot count;
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
    trace_reference_with(
        case,
        polarisation,
        samples,
        threads,
        Estimator::ByMedium,
        Orders::Every,
    )
}

/// [`trace_reference`] with the estimator `estimator` over the orders of scattering `orders`, so
/// that the tests can turn R08.T12.f's variance reduction on and off, and trace single scattering
/// alone (R08.T12.g).
///
/// # Errors
///
/// As [`trace_reference`].
///
/// # Panics
///
/// As [`trace_reference`].
pub(crate) fn trace_reference_with(
    case: &AtmosphereCase,
    polarisation: Polarisation,
    samples: NonZeroU64,
    threads: NonZeroUsize,
    estimator: Estimator,
    orders: Orders,
) -> Result<ReferenceRadiances, TraceReferenceError> {
    trace_counted_with(case, polarisation, samples, threads, estimator, orders)
        .map(|counted| counted.reference)
}

/// A traced reference with the vertices its samples reached (R08.T13.a's cost, main's reading 2):
/// every scattering and reflection of every branch, counted beside the draws, which it leaves
/// alone.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CountedTrace {
    /// The reference, as [`trace_reference`] traces it.
    pub(crate) reference: ReferenceRadiances,
    /// Each geometry's vertices over its samples and wavelengths, in the case's order.
    pub(crate) geometry_vertices: Vec<u64>,
    /// Each aggregate's vertices over both its fluxes' samples and the wavelengths.
    pub(crate) aggregate_vertices: Vec<u64>,
    /// Each geometry's radiance moments, by wavelength and then geometry in the case's order, from
    /// which [`trace_spans`] carries a geometry on (R08.T13.b's top-up).
    pub(crate) geometry_moments: Vec<Vec<GeometryMoments>>,
}

/// A geometry's radiance moments at one wavelength: over its whole run and over its first
/// quarter of samples, ⌊N ÷ 4⌋.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct GeometryMoments {
    /// Over the whole run.
    pub(crate) whole: Moments,
    /// Over the first quarter.
    pub(crate) quarter: Moments,
}

/// One span of a geometry's samples for [`trace_spans`].
///
/// It traces samples `N..end` of the geometry's own stream at one wavelength, merged onto
/// `prior`, the radiance moments of its samples `0..N`, N their count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Span {
    /// The case's wavelength, by index.
    pub(crate) wavelength: usize,
    /// The case's geometry, by index.
    pub(crate) geometry: usize,
    /// The radiance moments of the geometry's samples `0..N`: the span starts at N.
    pub(crate) prior: Moments,
    /// One past its last sample.
    pub(crate) end: u64,
}

impl Span {
    /// The span's first sample: its prior's count.
    #[must_use]
    pub(crate) fn start(&self) -> u64 {
        self.prior.samples()
    }
}

/// A span traced.
///
/// It holds the radiance moments of its geometry's samples `0..end`, its prior's and its own, and
/// the vertices its own samples reached.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SpanTrace {
    /// The moments of samples `0..end`.
    pub(crate) moments: Moments,
    /// The vertices of the span's own samples.
    pub(crate) vertices: u64,
}

/// [`trace_reference`] with each geometry's and aggregate's vertices counted.
///
/// # Errors
///
/// As [`trace_reference`].
///
/// # Panics
///
/// As [`trace_reference`].
pub(crate) fn trace_counted(
    case: &AtmosphereCase,
    polarisation: Polarisation,
    samples: NonZeroU64,
    threads: NonZeroUsize,
) -> Result<CountedTrace, TraceReferenceError> {
    trace_counted_with(
        case,
        polarisation,
        samples,
        threads,
        Estimator::ByMedium,
        Orders::Every,
    )
}

/// Traces each span of `case`'s geometries, each carried on from its prior.
///
/// It traces in the scalar mode under [`trace_counted`]'s estimator, on the case's seed and each
/// geometry's own stream, so that a geometry traced to N by [`trace_counted`] and carried on from
/// N to N′ onto its whole run's moments reads, bit for bit, as one traced to N′, for N in whole
/// blocks (R08.T13.b's top-up, `decision-r08-t13-scope.md`, "#112's floor refusals"). A span over
/// samples the geometry already holds gives a part of its run: from the moments of samples `0..b`
/// it gives those of `0..e`, as a fresh trace's first quarter folds them where b is in whole
/// blocks. The output is the same, bit for bit, for any number of threads.
///
/// # Errors
///
/// [`TraceReferenceError::TooManySamples`] for a span past [`MAX_SAMPLES`];
/// [`TraceReferenceError::ThreadPool`] if the threads cannot be started.
///
/// # Panics
///
/// If a span names a wavelength or a geometry the case does not have, or ends before it starts,
/// each a caller's bug; and as [`trace_reference`], if a traced value is not finite.
pub(crate) fn trace_spans(
    case: &AtmosphereCase,
    spans: &[Span],
    threads: NonZeroUsize,
) -> Result<Vec<SpanTrace>, TraceReferenceError> {
    if let Some(span) = spans.iter().find(|s| s.end > MAX_SAMPLES) {
        return Err(TraceReferenceError::TooManySamples { samples: span.end });
    }
    for span in spans {
        assert!(
            span.wavelength < case.wavelengths_nm().len() && span.geometry < case.geometry_count(),
            "a span names one of the case's own wavelengths and geometries"
        );
        assert!(
            span.start() <= span.end,
            "a span ends no earlier than its prior's samples"
        );
    }
    let grid = shell_grid(case);
    let media: Vec<Medium<'_>> = (0..case.wavelengths_nm().len())
        .map(|w| Medium::new(case, &grid, w, Estimator::ByMedium, Orders::Every))
        .collect();
    // Only the geometries the spans name, each keyed by its index in the case as
    // `trace_counted_with` keys it, so that its stream is its own whatever else is traced.
    let mut named: Vec<usize> = spans.iter().map(|s| s.geometry).collect();
    named.sort_unstable();
    named.dedup();
    let sources: Vec<(u64, Source)> = named
        .iter()
        .map(|&g| {
            (
                wide(g),
                Source::detector(case, &grid, &case.geometries()[g]),
            )
        })
        .collect();
    let jobs: Vec<Job> = spans
        .iter()
        .map(|s| {
            let mut prior = [Moments::default(); MOMENTS];
            prior[I] = s.prior;
            Job {
                wavelength: s.wavelength,
                source: named
                    .binary_search(&s.geometry)
                    .expect("every span's geometry is among those named"),
                start: s.start(),
                end: s.end,
                // Every sample of a span lies at or past its start: none is the first quarter's.
                quarter: s.start(),
                prior,
            }
        })
        .collect();
    let traced = trace_moments(
        case.seed(),
        &media,
        &sources,
        &jobs,
        Polarisation::Scalar,
        threads,
    )?;
    Ok(traced
        .whole
        .iter()
        .zip(&traced.vertices)
        .map(|(moments, &vertices)| {
            let moments = moments[I];
            finite(vec![moments.mean(), moments.standard_error()]);
            SpanTrace { moments, vertices }
        })
        .collect())
}

/// Whether the medium of `case` runs VROOM at wavelength index `wavelength` under the estimator
/// every reference runs (the `transport` module): where a scattering term is forward-peaked.
#[must_use]
pub(crate) fn runs_vroom(case: &AtmosphereCase, wavelength: usize) -> bool {
    let grid = shell_grid(case);
    Medium::new(case, &grid, wavelength, Estimator::ByMedium, Orders::Every).runs_vroom()
}

/// [`trace_counted`] with the estimator and orders of [`trace_reference_with`].
fn trace_counted_with(
    case: &AtmosphereCase,
    polarisation: Polarisation,
    samples: NonZeroU64,
    threads: NonZeroUsize,
    estimator: Estimator,
    orders: Orders,
) -> Result<CountedTrace, TraceReferenceError> {
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
        .map(|w| Medium::new(case, &grid, w, estimator, orders))
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
    // Each source's samples, in the sources' order: an aggregate's two fluxes share its count.
    let default = samples.get();
    let counts: Vec<u64> = case
        .geometries()
        .iter()
        .map(|g| g.samples.unwrap_or(default))
        .chain(
            case.aggregates()
                .iter()
                .flat_map(|a| [a.samples.unwrap_or(default); 2]),
        )
        .collect();
    let jobs = Job::whole_runs(media.len(), &counts);
    let traced = trace_moments(case.seed(), &media, &sources, &jobs, polarisation, threads)?;
    // Each source's vertices over the wavelengths: an aggregate's two fluxes are its own.
    let per_wavelength = sources.len();
    let source_vertices: Vec<u64> = (0..per_wavelength)
        .map(|k| traced.vertices.iter().skip(k).step_by(per_wavelength).sum())
        .collect();
    let geometry_count = case.geometry_count();
    let geometry_vertices = source_vertices[..geometry_count].to_vec();
    let aggregate_vertices = source_vertices[geometry_count..]
        .chunks(2)
        .map(|pair| pair.iter().sum())
        .collect();
    let geometry_moments = traced.geometry_moments(media.len(), per_wavelength, geometry_count);
    let run = Run {
        case,
        grid: &grid,
        media: &media,
        sources: &sources,
        counts: &counts,
        moments: traced.whole,
        quarters: traced.quarters,
    };
    let reference = ReferenceRadiances {
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
    };
    Ok(CountedTrace {
        reference,
        geometry_vertices,
        aggregate_vertices,
        geometry_moments,
    })
}

/// The moments a task keeps: those of each of a sample's [`COMPONENTS`], and of its radiance less
/// its scalar radiance.
const MOMENTS: usize = COMPONENTS + 1;

/// Where the moments of a sample's radiance less its scalar radiance are kept.
const DIFFERENCE: usize = COMPONENTS;

/// The moments of a traced case, per job in the jobs' order (for whole runs, per task,
/// wavelength-major): of each job's samples merged onto its prior, and of those below its quarter
/// (a whole run's first quarter of samples, ⌊N ÷ 4⌋); and the vertices each job's samples reached.
struct Traced {
    whole: Vec<[Moments; MOMENTS]>,
    quarters: Vec<[Moments; MOMENTS]>,
    vertices: Vec<u64>,
}

impl Traced {
    /// Whole runs' geometries' radiance moments, by wavelength and then geometry: the jobs laid
    /// out as [`Job::whole_runs`] lays them, `per_wavelength` sources a wavelength, the geometries
    /// first.
    #[must_use]
    fn geometry_moments(
        &self,
        wavelengths: usize,
        per_wavelength: usize,
        geometries: usize,
    ) -> Vec<Vec<GeometryMoments>> {
        (0..wavelengths)
            .map(|w| {
                (0..geometries)
                    .map(|g| GeometryMoments {
                        whole: self.whole[w * per_wavelength + g][I],
                        quarter: self.quarters[w * per_wavelength + g][I],
                    })
                    .collect()
            })
            .collect()
    }
}

/// Pushes a sample's components `x` into `moments`: I alone in the scalar mode, and in the Stokes
/// mode every component and the radiance less the scalar radiance.
fn push_sample(
    moments: &mut [Moments; MOMENTS],
    x: &[f64; COMPONENTS],
    polarisation: Polarisation,
) {
    match polarisation {
        Polarisation::Scalar => moments[I].push(x[I]),
        Polarisation::Stokes => {
            for (m, &value) in moments.iter_mut().zip(x) {
                m.push(value);
            }
            moments[DIFFERENCE].push(x[I] - x[SCALAR_I]);
        }
    }
}

/// One source's samples a trace takes at one wavelength.
///
/// They are `start..end` of its stream, those below `quarter` also kept apart, merged onto
/// `prior`, the moments of its samples before `start`. A whole run is `0..N` with its first
/// quarter `N ÷ 4` and no prior; [`trace_spans`] carries a run on from where it stopped.
#[derive(Debug, Clone, Copy)]
struct Job {
    /// The wavelength, by index into the media.
    wavelength: usize,
    /// The source, by index into the sources.
    source: usize,
    start: u64,
    end: u64,
    quarter: u64,
    prior: [Moments; MOMENTS],
}

impl Job {
    /// Every source's whole run at each of `wavelengths`, wavelength-major, source k `counts[k]`
    /// samples with its first quarter apart.
    #[must_use]
    fn whole_runs(wavelengths: usize, counts: &[u64]) -> Vec<Self> {
        (0..wavelengths)
            .flat_map(|w| {
                counts.iter().enumerate().map(move |(k, &n)| Self {
                    wavelength: w,
                    source: k,
                    start: 0,
                    end: n,
                    quarter: n / 4,
                    prior: [Moments::default(); MOMENTS],
                })
            })
            .collect()
    }
}

/// Traces each job.
///
/// It gives the moments of (I, Q, U, V), of the scalar radiance and of the radiance less it, per
/// job, over its samples merged onto its prior, and over those below its `quarter`. The scalar
/// mode fills the first alone.
fn trace_moments(
    seed: u64,
    media: &[Medium<'_>],
    sources: &[(u64, Source)],
    jobs: &[Job],
    polarisation: Polarisation,
    threads: NonZeroUsize,
) -> Result<Traced, TraceReferenceError> {
    // Each job's first item, its blocks laid out job after job, each block `start + 1,024 b` on:
    // for whole runs, the item `task × blocks + block` as before.
    let mut starts = Vec::with_capacity(jobs.len() + 1);
    let mut items = 0_u64;
    for job in jobs {
        starts.push(items);
        // Past 2⁶⁴ blocks the run's samples are more than any count, so it is refused as such.
        items = items
            .checked_add(job.end.saturating_sub(job.start).div_ceil(BLOCK_SAMPLES))
            .ok_or(TraceReferenceError::TooManySamples { samples: u64::MAX })?;
    }
    starts.push(items);
    let seed = seed ^ ATMOSPHERE_STREAM;
    // Each job's blocks merge onto its prior in index order, as a whole run's merge onto the
    // empty moments: a run carried on from N reads as one traced from 0.
    let mut whole: Vec<[Moments; MOMENTS]> = jobs.iter().map(|job| job.prior).collect();
    let mut quarters = vec![[Moments::default(); MOMENTS]; jobs.len()];
    let mut vertices = vec![0_u64; jobs.len()];
    map_reduce_chunks(
        items,
        1,
        threads,
        |range| {
            let mut scratch = Scratch::default();
            range
                .map(|item| {
                    // The last job starting at or before the item: an empty job shares its start
                    // with the next, which holds the item.
                    let index = starts.partition_point(|&start| start <= item) - 1;
                    let block = item - starts[index];
                    let job = &jobs[index];
                    let (key, source) = &sources[job.source];
                    // The case keeps its wavelengths below 2¹⁶ and its sources' keys below 2³⁴.
                    let stream = (wide(job.wavelength) << 34) | key;
                    let mut block_moments = [Moments::default(); MOMENTS];
                    let mut quarter_moments = [Moments::default(); MOMENTS];
                    // Below 2⁵³ + 2¹⁰: no sum here overflows.
                    let first = job.start + block * BLOCK_SAMPLES;
                    let end = (first + BLOCK_SAMPLES).min(job.end);
                    for sample in first..end {
                        let mut draws = Draws::new(seed, stream, sample);
                        let x = media[job.wavelength].sample(
                            source,
                            polarisation,
                            &mut draws,
                            &mut scratch,
                        );
                        push_sample(&mut block_moments, &x, polarisation);
                        if sample < job.quarter {
                            push_sample(&mut quarter_moments, &x, polarisation);
                        }
                    }
                    (
                        index,
                        block_moments,
                        quarter_moments,
                        scratch.take_vertices(),
                    )
                })
                .collect::<Vec<_>>()
        },
        |results| {
            for (index, block_moments, quarter_moments, block_vertices) in results {
                vertices[index] += block_vertices;
                for (total, part) in whole[index].iter_mut().zip(&block_moments) {
                    total.merge(part);
                }
                for (total, part) in quarters[index].iter_mut().zip(&quarter_moments) {
                    total.merge(part);
                }
            }
        },
    )?;
    Ok(Traced {
        whole,
        quarters,
        vertices,
    })
}

/// A traced case: its media per wavelength, its sources (the geometries' detectors, then each
/// aggregate's ground and top) with their samples, and the moments of each task, wavelength-major,
/// over the whole run and its first quarter.
struct Run<'a> {
    case: &'a AtmosphereCase,
    grid: &'a ShellGrid,
    media: &'a [Medium<'a>],
    sources: &'a [(u64, Source)],
    counts: &'a [u64],
    moments: Vec<[Moments; MOMENTS]>,
    quarters: Vec<[Moments; MOMENTS]>,
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

    /// The standard error of Stokes component `component` of source `source` over its first
    /// quarter of samples, per wavelength.
    #[must_use]
    fn quarter_error(&self, source: usize, component: usize) -> Vec<f64> {
        let per_wavelength = self.sources.len();
        finite(
            (0..self.media.len())
                .map(|w| self.quarters[w * per_wavelength + source][component].standard_error())
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
            samples: self.counts[g],
            quarter_samples: self.counts[g] / 4,
            radiance: self.column(g, I, Moments::mean),
            standard_error: self.column(g, I, Moments::standard_error),
            quarter_standard_error: self.quarter_error(g, I),
            stokes: (polarisation == Polarisation::Stokes).then(|| StokesRadiance {
                q: self.column(g, 1, Moments::mean),
                q_standard_error: self.column(g, 1, Moments::standard_error),
                u: self.column(g, 2, Moments::mean),
                u_standard_error: self.column(g, 2, Moments::standard_error),
                v: self.column(g, 3, Moments::mean),
                v_standard_error: self.column(g, 3, Moments::standard_error),
                scalar_radiance: self.column(g, SCALAR_I, Moments::mean),
                scalar_standard_error: self.column(g, SCALAR_I, Moments::standard_error),
                scalar_quarter_standard_error: self.quarter_error(g, SCALAR_I),
                difference_standard_error: self.column(g, DIFFERENCE, Moments::standard_error),
            }),
            sun_optical_depth: source
                .suns
                .iter()
                .enumerate()
                .map(|(s, &sun)| {
                    self.media
                        .iter()
                        .map(|m| m.beam_optical_depth(source.origin_m, sun, s))
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
        // Each sun above the horizon's μ₀F, with the transmittance of its beam to the ground, the
        // absorbers' curves read at the beam's own column.
        let beams = |w: usize| {
            source
                .suns
                .iter()
                .zip(self.case.suns())
                .enumerate()
                .filter(|(_, (sun, _))| up.dot(**sun) > 0.0)
                .map(move |(s, (&sun, spectrum))| {
                    let transmittance = self.media[w]
                        .beam_optical_depth(source.origin_m, sun, s)
                        .map_or(0.0, |depth| hyperion_sim::math::exp(-depth));
                    (up.dot(sun) * spectrum.irradiance[w], transmittance)
                })
        };
        let incident = |w: usize| beams(w).map(|(flux, _)| flux).sum();
        let direct = |w: usize| beams(w).map(|(flux, t)| flux * t).sum();
        let wavelengths = 0..self.media.len();
        FluxAggregate {
            name: self.case.aggregates()[a].name.clone(),
            samples: self.counts[ground],
            quarter_samples: self.counts[ground] / 4,
            incident_top: finite(wavelengths.clone().map(incident).collect()),
            direct_ground: finite(wavelengths.map(direct).collect()),
            diffuse_ground: self.column(ground, I, Moments::mean),
            diffuse_ground_standard_error: self.column(ground, I, Moments::standard_error),
            diffuse_ground_quarter_standard_error: self.quarter_error(ground, I),
            upwelling_top: self.column(top, I, Moments::mean),
            upwelling_top_standard_error: self.column(top, I, Moments::standard_error),
            upwelling_top_quarter_standard_error: self.quarter_error(top, I),
            scalar: (polarisation == Polarisation::Stokes).then(|| ScalarFlux {
                diffuse_ground: self.column(ground, SCALAR_I, Moments::mean),
                diffuse_ground_standard_error: self.column(
                    ground,
                    SCALAR_I,
                    Moments::standard_error,
                ),
                diffuse_ground_quarter_standard_error: self.quarter_error(ground, SCALAR_I),
                diffuse_ground_difference_standard_error: self.column(
                    ground,
                    DIFFERENCE,
                    Moments::standard_error,
                ),
                upwelling_top: self.column(top, SCALAR_I, Moments::mean),
                upwelling_top_standard_error: self.column(top, SCALAR_I, Moments::standard_error),
                upwelling_top_quarter_standard_error: self.quarter_error(top, SCALAR_I),
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
    use core::fmt;

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
        trace_with(value, polarisation, samples, threads, Estimator::ByMedium)
    }

    fn trace_with(
        value: &Value,
        polarisation: Polarisation,
        samples: u64,
        threads: usize,
        estimator: Estimator,
    ) -> ReferenceRadiances {
        trace_by(
            value,
            polarisation,
            samples,
            threads,
            estimator,
            Orders::Every,
        )
    }

    /// `value` traced by `estimator` over the orders of scattering `orders`.
    #[must_use]
    fn trace_by(
        value: &Value,
        polarisation: Polarisation,
        samples: u64,
        threads: usize,
        estimator: Estimator,
        orders: Orders,
    ) -> ReferenceRadiances {
        trace_reference_with(
            &case_of(value),
            polarisation,
            NonZeroU64::new(samples).unwrap(),
            NonZeroUsize::new(threads).unwrap(),
            estimator,
            orders,
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
        // And R08.T12.f's VROOM deep in a thick forward-peaked slab, τ 5 at g 0.9, where every
        // path makes clones and many split: 64 samples of eight sources and wavelengths, one block
        // each, shared among the four threads.
        let thick = thick_peaked_slab();
        let one = trace(&thick, Polarisation::Stokes, 64, 1).to_json();
        let four = trace(&thick, Polarisation::Stokes, 64, 4).to_json();
        assert_eq!(one, four);
        // And R08.T12.b.1's: an absorber's columns along every path, and a geometry and an
        // aggregate of their own counts, whose blocks lie unevenly among the threads.
        let mut absorbing = as_absorber(&case::tests::sample_case(), 2, 1e18, 1.5e22);
        absorbing["geometries"][1]["samples"] = json!(2500);
        absorbing["aggregates"][0]["samples"] = json!(1100);
        let one = trace(&absorbing, Polarisation::Scalar, 300, 1).to_json();
        let four = trace(&absorbing, Polarisation::Scalar, 300, 4).to_json();
        assert_eq!(one, four);
    }

    /// `value` traced with its vertices counted, `samples` a source, on `threads` threads.
    fn counted(value: &Value, samples: u64, threads: usize) -> CountedTrace {
        trace_counted(
            &case_of(value),
            Polarisation::Scalar,
            NonZeroU64::new(samples).unwrap(),
            NonZeroUsize::new(threads).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn atmosphere_counted_trace_counts_each_vertex_its_paths_reach() {
        // Nothing scatters and the ground is black: the view from orbit reaches the ground, one
        // vertex a sample at each of the two wavelengths, and the view of the sky reaches none; of
        // the aggregate's two fluxes, the ground's reaches nothing and the top's the ground, but
        // for the directions within the horizon's dip of 10° from 100 km, which pass over the
        // limb: about 3% of a cosine-weighted hemisphere.
        let mut absorbing = case::tests::sample_case();
        let ozone = absorbing["terms"][2].clone();
        absorbing["terms"] = json!([ozone]);
        absorbing["groundAlbedo"] = json!([0.0, 0.0]);
        let traced = counted(&absorbing, 300, 4);
        assert_eq!(traced.geometry_vertices, [0, 600]);
        let top = traced.aggregate_vertices[0];
        assert!((540..600).contains(&top), "{top}");
        // Where the air scatters, every sample's forced collision is a vertex, and the reference
        // is `trace_reference`'s.
        let sample = case::tests::sample_case();
        let traced = counted(&sample, 1500, 1);
        assert!(
            traced.geometry_vertices.iter().all(|&v| v >= 2 * 1500),
            "{:?}",
            traced.geometry_vertices
        );
        assert_eq!(
            traced.reference.to_json(),
            trace(&sample, Polarisation::Scalar, 1500, 1).to_json()
        );
    }

    #[test]
    fn atmosphere_counted_trace_counts_the_same_on_one_and_four_threads() {
        // Two wavelengths, a geometry and an aggregate of their own counts, blocks uneven among
        // the threads; and VROOM's clones and copies deep in a thick forward-peaked slab.
        let mut uneven = case::tests::sample_case();
        uneven["geometries"][1]["samples"] = json!(2500);
        uneven["aggregates"][0]["samples"] = json!(1100);
        assert_eq!(counted(&uneven, 300, 1), counted(&uneven, 300, 4));
        let thick = thick_peaked_slab();
        assert_eq!(counted(&thick, 64, 1), counted(&thick, 64, 4));
    }

    /// Each of `case`'s geometries carried on from `short` to `to`, with its first quarter carried
    /// on to `to ÷ 4`, on `threads` threads: per wavelength and geometry, the run's span and the
    /// quarter's.
    fn carried_on(
        case: &AtmosphereCase,
        short: &CountedTrace,
        to: u64,
        threads: usize,
    ) -> Vec<SpanTrace> {
        let mut spans = Vec::new();
        for (w, moments) in short.geometry_moments.iter().enumerate() {
            for (g, m) in moments.iter().enumerate() {
                spans.push(Span {
                    wavelength: w,
                    geometry: g,
                    prior: m.whole,
                    end: to,
                });
                spans.push(Span {
                    wavelength: w,
                    geometry: g,
                    prior: m.quarter,
                    end: to / 4,
                });
            }
        }
        trace_spans(case, &spans, NonZeroUsize::new(threads).unwrap()).unwrap()
    }

    #[test]
    fn atmosphere_spans_carry_a_geometry_on_as_one_traced_to_the_end() {
        // Traced to 4,096 and carried on to 8,192, its first quarter from 1,024 to 2,048, every
        // geometry at both wavelengths reads bit for bit as one traced to 8,192, and its vertices
        // add up to that trace's.
        let sample = case::tests::sample_case();
        let case = case_of(&sample);
        let (short, long) = (counted(&sample, 4096, 4), counted(&sample, 8192, 1));
        let spans = carried_on(&case, &short, 8192, 4);
        let geometries = case.geometry_count();
        assert_eq!(case.wavelengths_nm().len(), 2);
        assert_eq!(
            spans.len(),
            2 * 2 * geometries,
            "two spans a geometry and wavelength"
        );
        for (w, (moments, pairs)) in long
            .geometry_moments
            .iter()
            .zip(spans.chunks(2 * geometries))
            .enumerate()
        {
            for (g, (m, pair)) in moments.iter().zip(pairs.chunks(2)).enumerate() {
                assert_eq!(
                    pair[0].moments, m.whole,
                    "the run, wavelength {w}, geometry {g}"
                );
                assert_eq!(
                    pair[1].moments, m.quarter,
                    "the quarter, wavelength {w}, geometry {g}"
                );
            }
        }
        for g in 0..geometries {
            let carried: u64 = (0..2)
                .map(|w| spans[(w * geometries + g) * 2].vertices)
                .sum();
            assert_eq!(
                short.geometry_vertices[g] + carried,
                long.geometry_vertices[g],
                "geometry {g}'s vertices"
            );
        }
        // The same on one thread, and for one geometry named alone: its stream is its own.
        assert_eq!(carried_on(&case, &short, 8192, 1), spans);
        let alone = trace_spans(
            &case,
            &[Span {
                wavelength: 1,
                geometry: 1,
                prior: short.geometry_moments[1][1].whole,
                end: 8192,
            }],
            NonZeroUsize::new(4).unwrap(),
        )
        .unwrap();
        assert_eq!(alone[0], spans[(geometries + 1) * 2]);
        // From 3,072, whose quarter of 768 ends inside a block: the run is the trace's to the bit,
        // and so is the quarter carried on from the empty run, as the sweep's top-ups carry it.
        let (short, long) = (counted(&sample, 3072, 4), counted(&sample, 6144, 4));
        let mut spans = Vec::new();
        for (w, moments) in short.geometry_moments.iter().enumerate() {
            for (g, m) in moments.iter().enumerate() {
                spans.push(Span {
                    wavelength: w,
                    geometry: g,
                    prior: m.whole,
                    end: 6144,
                });
                spans.push(Span {
                    wavelength: w,
                    geometry: g,
                    prior: Moments::default(),
                    end: 1536,
                });
            }
        }
        let traced = trace_spans(&case, &spans, NonZeroUsize::new(4).unwrap()).unwrap();
        for (k, pair) in traced.chunks(2).enumerate() {
            let m = long.geometry_moments[k / geometries][k % geometries];
            assert_eq!(pair[0].moments, m.whole, "the run {k}");
            assert_eq!(pair[1].moments, m.quarter, "the quarter {k}");
        }
    }

    #[test]
    fn atmosphere_span_to_its_own_start_gives_its_prior() {
        let sample = case::tests::sample_case();
        let short = counted(&sample, 2048, 1);
        let prior = short.geometry_moments[0][0].whole;
        let traced = trace_spans(
            &case_of(&sample),
            &[Span {
                wavelength: 0,
                geometry: 0,
                prior,
                end: 2048,
            }],
            NonZeroUsize::MIN,
        )
        .unwrap();
        assert_eq!(
            traced,
            [SpanTrace {
                moments: prior,
                vertices: 0,
            }]
        );
    }

    #[test]
    #[should_panic(expected = "a span ends no earlier than its prior's samples")]
    fn atmosphere_spans_refuse_a_span_ending_before_its_prior() {
        let sample = case::tests::sample_case();
        let short = counted(&sample, 2048, 1);
        // It panics before it returns, so there is no result to read.
        let _ = trace_spans(
            &case_of(&sample),
            &[Span {
                wavelength: 0,
                geometry: 0,
                prior: short.geometry_moments[0][0].whole,
                end: 1024,
            }],
            NonZeroUsize::MIN,
        );
    }

    /// A conservative slab of τ 5 (550 nm) and 2.5 (440 nm) scattering by Henyey–Greenstein at g
    /// 0.9 as a table with a matrix, over a black ground and a white one, seen at the top and the
    /// bottom away from the sun, with an aggregate: VROOM's clones and splits in every sample.
    fn thick_peaked_slab() -> Value {
        let terms = json!([{
            "name": "cloud", "density": uniform(),
            "scattering": [5e-3, 2.5e-3], "absorption": [0.0, 0.0],
            "phase": case::tests::tabulated_phase_json(512, 2, |c| {
                let p = optics::tests::henyey_greenstein(0.9, c);
                let sin2 = 1.0 - c * c;
                [p, p, c * p, c * p, -0.2 * sin2 * p, 0.1 * sin2.sqrt() * p]
            })
        }]);
        let mut case = slab(
            &terms,
            &json!([0.0, 1.0]),
            &json!([
                geometry("down", 0.0, (40.0, 120.0), (30.0, 0.0)),
                geometry("up", 1000.0, (130.0, 60.0), (30.0, 0.0))
            ]),
        );
        case["aggregates"] =
            json!([{ "name": "sun", "sunDirections": [{ "zenithDeg": 30.0, "azimuthDeg": 0.0 }] }]);
        case
    }

    #[test]
    fn atmosphere_rayleigh_alone_keeps_the_sunward_draws_paths() {
        // Where no term is forward-peaked, `Estimator::ByMedium` runs R08.T12.c's estimator, so
        // that every Rayleigh case's paths are R08.T12.c's to the bit (R08.T12.f).
        let mut rayleigh = case::tests::sample_case();
        rayleigh["terms"].as_array_mut().unwrap().remove(1);
        for polarisation in [Polarisation::Scalar, Polarisation::Stokes] {
            assert_eq!(
                trace(&rayleigh, polarisation, 300, 2).to_json(),
                trace_with(&rayleigh, polarisation, 300, 2, Estimator::SunwardDraws).to_json()
            );
        }
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
        // series' proposal, the sunward draws, VROOM, the 4 × 4 product or the moments shows here
        // before it moves a committed reference.
        let pinned = |estimator: Estimator| {
            let reference = trace_with(&polarising_case(), Polarisation::Stokes, 64, 1, estimator);
            let ground = &reference.geometries()[0];
            let stokes = ground.stokes().unwrap();
            [
                format!("{:?}", ground.radiance()[0]),
                format!("{:?}", stokes.q()[0]),
                format!("{:?}", stokes.v()[0]),
                format!("{:?}", stokes.scalar_radiance()[1]),
                format!("{:?}", stokes.difference_standard_error()[0]),
                format!(
                    "{:?}",
                    reference.aggregates()[0].scalar().unwrap().upwelling_top()[0]
                ),
            ]
        };
        // R08.T12.c's estimator, unchanged to the bit by R08.T12.f.
        assert_eq!(
            pinned(Estimator::SunwardDraws),
            [
                "0.051914114460382",
                "-0.005734264674987712",
                "3.1510023629621357e-6",
                "0.09834999038845736",
                "0.0004741472848915612",
                "0.5466892681652658"
            ]
        );
        // VROOM, which the peaked table turns on, as every reference of the case now runs.
        assert_eq!(pinned(Estimator::ByMedium), pinned(Estimator::Vroom));
        assert_eq!(
            pinned(Estimator::ByMedium),
            [
                "0.050112677671275564",
                "-0.005845951069894733",
                "-7.921756602999342e-6",
                "0.09859597110749957",
                "0.00024901108218867773",
                "0.5249233156144391"
            ]
        );
    }

    /// Asserts that a standard error `full` falls as N^(−1/2) from `quarter`'s, over the run's
    /// first quarter, to 25% (`decision-r08-t4b-t12c.md`, follow-up, ruling 7): every σ a
    /// statistical test asserts must.
    fn assert_sigma_scales(full: f64, quarter: f64, what: &str) {
        assert!(
            benchmarks::sigma_scales(full, quarter),
            "{what}: σ {full} does not fall as N^(-1/2) from {quarter} at N ÷ 4"
        );
    }

    /// `value` traced in the scalar mode at `samples` on four threads, after asserting that every
    /// geometry's σ falls as N^(−1/2) from the run's first quarter.
    fn trace_scaling(value: &Value, samples: u64) -> ReferenceRadiances {
        let full = trace(value, Polarisation::Scalar, samples, 4);
        let quarter = trace(value, Polarisation::Scalar, samples / 4, 4);
        for (g, first) in full.geometries().iter().zip(quarter.geometries()) {
            for (sigma, quarter_sigma) in g.standard_error().iter().zip(first.standard_error()) {
                assert_sigma_scales(*sigma, *quarter_sigma, g.name());
            }
        }
        full
    }

    /// `value` traced at `samples` samples, a multiple of 4, and over their first quarter, which
    /// is the trace at `samples ÷ 4`: every sample draws from its own counter-based stream.
    #[must_use]
    fn trace_and_quarter(
        value: &Value,
        polarisation: Polarisation,
        samples: u64,
        threads: usize,
    ) -> [ReferenceRadiances; 2] {
        trace_and_quarter_over(value, polarisation, samples, threads, Orders::Every)
    }

    /// [`trace_and_quarter`] over the orders of scattering `orders`.
    #[must_use]
    fn trace_and_quarter_over(
        value: &Value,
        polarisation: Polarisation,
        samples: u64,
        threads: usize,
        orders: Orders,
    ) -> [ReferenceRadiances; 2] {
        assert_eq!(samples % 4, 0, "a test's samples are a multiple of 4");
        [samples, samples / 4]
            .map(|n| trace_by(value, polarisation, n, threads, Estimator::ByMedium, orders))
    }

    /// Design note 10's statistics in one of the tracer's unit tests (R08.T12.g;
    /// `decision-r08-t12f.md` ruling 4). Every estimated σ the test reads must fall as N^(−1/2)
    /// from its σ over the run's first quarter ([`assert_sigma_scales`]), and each of its values is
    /// held to the Bonferroni bound over them ([`benchmarks::bonferroni_bound`]) times its σ, or
    /// to 4σ where it holds 8 or fewer. It prints the σ ratios and the largest |z| when it is
    /// finished.
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct Agreement {
        name: &'static str,
        values: u32,
        bound: f64,
        held: u32,
        /// The σ checked, the smallest and largest ratio among them of σ at N to half σ at N ÷ 4,
        /// and how many were exactly 0 at both counts (a component the test's paths cannot make).
        sigmas: u32,
        ratios: (f64, f64),
        exact: u32,
        /// The largest |z| held, |difference| less the slack over σ, and what it was.
        largest: (f64, String),
    }

    impl Agreement {
        /// The statistics of test `name`, which holds `values` values.
        #[must_use]
        pub(crate) fn new(name: &'static str, values: u32) -> Self {
            Self {
                name,
                values,
                bound: if values <= 8 {
                    4.0
                } else {
                    benchmarks::bonferroni_bound(values)
                },
                held: 0,
                sigmas: 0,
                ratios: (f64::INFINITY, 0.0),
                exact: 0,
                largest: (0.0, "none".to_owned()),
            }
        }

        /// Asserts that `what`'s σ `full` at N falls as N^(−1/2) from `quarter`, its σ over the
        /// run's first N ÷ 4 samples or draws.
        pub(crate) fn sigma_scales(&mut self, full: f64, quarter: f64, what: impl fmt::Display) {
            assert_sigma_scales(full, quarter, &format!("{}, {what}", self.name));
            self.sigmas += 1;
            if quarter > 0.0 {
                let ratio = full / (0.5 * quarter);
                self.ratios = (self.ratios.0.min(ratio), self.ratios.1.max(ratio));
            } else {
                self.exact += 1;
            }
        }

        /// Asserts that `what`'s `difference` from its expected value is within `slack`, a
        /// deterministic tolerance beside the statistical one (0 for most), and the bound times
        /// its σ `sigma`.
        pub(crate) fn hold(
            &mut self,
            difference: f64,
            sigma: f64,
            slack: f64,
            what: impl fmt::Display,
        ) {
            assert!(
                difference.abs() <= slack + self.bound * sigma,
                "{}, {what}: {difference} off, past {slack} + {:.2} × σ {sigma}",
                self.name,
                self.bound
            );
            self.held += 1;
            let z = (difference.abs() - slack).max(0.0) / sigma;
            if sigma > 0.0 && z > self.largest.0 {
                self.largest = (z, what.to_string());
            }
        }

        /// Asserts that the test held as many values as it said, and prints its figures.
        pub(crate) fn finish(self) {
            assert_eq!(
                self.held, self.values,
                "{}: the values held, against those its bound is for",
                self.name
            );
            let scaling = if self.sigmas == 0 {
                "no σ held to N^(−1/2)".to_owned()
            } else {
                format!(
                    "{} σ falling as N^(−1/2), σ_N ÷ (σ_N/4 ÷ 2) from {:.3} to {:.3}, {} of them 0 \
                     at both counts",
                    self.sigmas, self.ratios.0, self.ratios.1, self.exact
                )
            };
            eprintln!(
                "{}: {} values to {:.2}σ, the largest |z| past any slack {:.2} ({}); {scaling}",
                self.name, self.values, self.bound, self.largest.0, self.largest.1
            );
        }
    }

    #[test]
    fn atmosphere_vroom_keeps_a_forward_peaked_slab_unbiased() {
        // Henyey–Greenstein at g 0.9 as a table, a conservative slab of τ 1 over a black ground
        // (550 nm) and a white one (440 nm), so that every vertex draws towards the suns: energy
        // is conserved to 4σ, as in the Rayleigh slab, and two suns in different directions give
        // the sum of the two lit alone, whose paths draw towards other suns, to 4σ. The peak
        // turns on R08.T12.f's VROOM, whose clones draw sunward at every vertex. R08.T12.c's
        // sunward draws alone passed this test as built (it was
        // `atmosphere_sunward_draws_keep_a_forward_peaked_slab_unbiased`), and are pinned to the
        // bit since (`atmosphere_reference_bits_are_pinned`).
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
        // do not aim at: σ is 0.68% of the incident flux at 80,000 samples with the sun high, and
        // 1.17% with it at 65° (measured; 0.70% and 1.30% by R08.T12.c's estimator), so the test
        // holds σ under 2%. A flaw in the sunward weights would be far larger: leaving out the
        // mixture's branch alone is 25%.
        let reference = trace(&case, Polarisation::Scalar, 80_000, 4);
        let quarter = trace(&case, Polarisation::Scalar, 20_000, 4);
        for (aggregate, first) in reference.aggregates().iter().zip(quarter.aggregates()) {
            let incident = aggregate.incident_top();
            let total = aggregate.upwelling_top()[0]
                + aggregate.direct_ground()[0]
                + aggregate.diffuse_ground()[0];
            let sigma = math::hypot(
                aggregate.upwelling_top_standard_error()[0],
                aggregate.diffuse_ground_standard_error()[0],
            );
            for w in 0..2 {
                assert_sigma_scales(
                    aggregate.upwelling_top_standard_error()[w],
                    first.upwelling_top_standard_error()[w],
                    aggregate.name(),
                );
            }
            assert_sigma_scales(
                aggregate.diffuse_ground_standard_error()[0],
                first.diffuse_ground_standard_error()[0],
                aggregate.name(),
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
            trace_scaling(&case, 60_000)
        };
        // The view down from the top is dim under so forward a haze: the comparison's σ is 1.7%
        // at 60,000 samples (measured; R08.T12.c's estimator, whose σ fell slowly there, 3.5% at
        // 20,000 and 2.7% at 60,000), so the test holds σ under 4%; the view up from the ground
        // is near 0.6%.
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
        let pinned = |estimator: Estimator| {
            let reference = trace_with(
                &case::tests::sample_case(),
                Polarisation::Scalar,
                64,
                1,
                estimator,
            );
            [
                format!("{:?}", reference.geometries()[0].radiance()[0]),
                format!("{:?}", reference.aggregates()[0].upwelling_top()[1]),
            ]
        };
        // The ground view moved with R08.T12.c's sun-directed draws, which its Cornette–Shanks
        // aerosol takes where it is forward-peaked; it was 0.039731668489903935 in R08.T12.a.
        // R08.T12.f leaves that estimator unchanged to the bit.
        assert_eq!(
            pinned(Estimator::SunwardDraws),
            ["0.04009082837728386", "0.5324772871217555"]
        );
        // Both values moved with R08.T12.f's VROOM, which the aerosol's forward value of
        // 4.1 sr⁻¹ turns on at both wavelengths.
        assert_eq!(
            pinned(Estimator::ByMedium),
            ["0.04122938235795757", "0.5181556568558089"]
        );
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

    /// A curve row's JSON on 241 nodes, forty to a decade over six decades about `vertical_per_m2`,
    /// whose optical depth at each wavelength is `depth(w, u)`.
    fn curve_json(
        vertical_per_m2: f64,
        wavelengths: usize,
        depth: impl Fn(usize, f64) -> f64,
    ) -> Value {
        let columns: Vec<f64> = (0..=240)
            .map(|k| vertical_per_m2 * math::powf(10.0, -3.0 + f64::from(k) / 40.0))
            .collect();
        json!({
            "logColumns": columns.iter().map(|&u| math::ln(u)).collect::<Vec<f64>>(),
            "opticalDepth": (0..wavelengths)
                .map(|w| columns.iter().map(|&u| depth(w, u)).collect::<Vec<f64>>())
                .collect::<Vec<Vec<f64>>>()
        })
    }

    /// `case` with term `term`'s absorption given instead as an absorber of density `n` per m³ at
    /// relative density 1, whose curve under every sun is τ = (absorption ÷ n) u, a column of
    /// `vertical_per_m2` its grid's centre.
    fn as_absorber(case: &Value, term: usize, n: f64, vertical_per_m2: f64) -> Value {
        let mut with = case.clone();
        let absorption: Vec<f64> = case["terms"][term]["absorption"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_f64().unwrap())
            .collect();
        let suns = case["suns"].as_array().unwrap().len();
        let curve = curve_json(vertical_per_m2, absorption.len(), |w, u| {
            absorption[w] / n * u
        });
        with["terms"][term]["absorption"] = json!(vec![0.0; absorption.len()]);
        with["terms"][term]["absorber"] = json!({
            "kind": "column", "numberDensityPerM3": n, "curves": vec![curve; suns]
        });
        with
    }

    /// Asserts that every radiance, flux and beam of `a` and `b` agrees to `relative`.
    fn assert_same_reference(a: &ReferenceRadiances, b: &ReferenceRadiances, relative: f64) {
        let close = |x: f64, y: f64, what: &str| {
            assert!(
                (x - y).abs() <= relative * x.abs().max(y.abs()),
                "{what}: {x} against {y}"
            );
        };
        for (x, y) in a.geometries().iter().zip(b.geometries()) {
            for w in 0..x.radiance().len() {
                close(x.radiance()[w], y.radiance()[w], x.name());
                close(x.standard_error()[w], y.standard_error()[w], x.name());
                if let (Some(p), Some(q)) = (x.stokes(), y.stokes()) {
                    close(p.q()[w], q.q()[w], x.name());
                    close(p.scalar_radiance()[w], q.scalar_radiance()[w], x.name());
                }
                for (dx, dy) in x.sun_optical_depth().iter().zip(y.sun_optical_depth()) {
                    match (dx[w], dy[w]) {
                        (Some(dx), Some(dy)) => close(dx, dy, x.name()),
                        (None, None) => {}
                        other => panic!("{}: {other:?}", x.name()),
                    }
                }
            }
        }
        for (x, y) in a.aggregates().iter().zip(b.aggregates()) {
            for w in 0..x.incident_top().len() {
                close(x.direct_ground()[w], y.direct_ground()[w], x.name());
                close(x.diffuse_ground()[w], y.diffuse_ground()[w], x.name());
                close(x.upwelling_top()[w], y.upwelling_top()[w], x.name());
            }
        }
    }

    #[test]
    fn atmosphere_an_absorber_whose_curve_is_beer_lambert_traces_as_its_absorption() {
        // R08.T12.b.1: an absorber whose curve is τ = σu, e^(−σu), gives what the same σ n given as
        // the term's absorption gives, to 10⁻¹²: the paths are the same (the term scatters
        // nothing, so its shells and majorants are), and T(u_path + u_leg) at each score is the
        // product of the flights' absorption weights and the leg's transmittance. Beer–Lambert:
        // R08.T12.a's absorbing-only case, vertical and slant.
        let absorbing = json!({
            "format": 1, "name": "absorbing",
            "wavelengthsNm": [550, 440],
            "shells": { "kind": "sphere", "radiusM": 6_371_000.0 },
            "topHeightM": 100_000.0,
            "groundAlbedo": [0.3, 0.6],
            "terms": [{
                "name": "absorber", "density": { "kind": "exponential", "scaleHeightM": 8000.0 },
                "scattering": [0.0, 0.0], "absorption": [1e-5, 3e-5], "phase": { "kind": "none" }
            }],
            "suns": [{ "name": "sun", "irradiance": [1.7, 2.0] }],
            "detectorHalfAngleDeg": 0.0,
            "geometries": [
                geometry("nadir", 400_000.0, (180.0, 0.0), (60.0, 0.0)),
                geometry("slant", 400_000.0, (150.0, 30.0), (70.0, 0.0)),
                geometry("ground", 0.0, (0.0, 0.0), (60.0, 0.0))
            ],
            "aggregates": [{ "name": "low", "sunDirections": [{ "zenithDeg": 70.0, "azimuthDeg": 0.0 }] }]
        });
        // n so that the vertical column is 8 × 10²³ m⁻², the grid's centre.
        let n = 1e20;
        let with = as_absorber(&absorbing, 0, n, n * 8000.0);
        for polarisation in [Polarisation::Scalar, Polarisation::Stokes] {
            let [a, b] = [&absorbing, &with].map(|c| trace(c, polarisation, 64, 2));
            assert_same_reference(&a, &b, 1e-12);
            assert!(a.geometries()[0].radiance()[0] > 0.0);
        }
        // A scattering case: Rayleigh over a black ground with a thin absorbing layer, one sun, so
        // that neither the roulette nor VROOM plays and the paths are the same draw for draw.
        let mut scattering = slab(
            &json!([
                {
                    "name": "gas", "density": uniform(),
                    "scattering": [3e-4, 5e-4], "absorption": [0.0, 0.0],
                    "phase": { "kind": "rayleigh", "depolarisation": [0.03, 0.03] }
                },
                {
                    "name": "layer",
                    "density": {
                        "kind": "tabulated", "altitudesM": [0.0, 200.0, 600.0, 1000.0],
                        "relative": [0.0, 1.0, 1.0, 0.2]
                    },
                    "scattering": [0.0, 0.0], "absorption": [2e-5, 4e-5], "phase": { "kind": "none" }
                }
            ]),
            &json!([0.0, 0.0]),
            &json!([
                geometry("down", 0.0, (40.0, 120.0), (30.0, 0.0)),
                geometry("up", 1000.0, (130.0, 60.0), (30.0, 0.0))
            ]),
        );
        scattering["aggregates"] =
            json!([{ "name": "sun", "sunDirections": [{ "zenithDeg": 30.0, "azimuthDeg": 0.0 }] }]);
        let with = as_absorber(&scattering, 1, n, n * 640.0);
        for polarisation in [Polarisation::Scalar, Polarisation::Stokes] {
            let [a, b] = [&scattering, &with].map(|c| trace(c, polarisation, 2000, 2));
            assert_same_reference(&a, &b, 1e-12);
        }
    }

    #[test]
    fn atmosphere_a_saturated_curve_takes_the_whole_paths_column() {
        // R08.T12.b.1: an absorbing-only slab seen from above on a grey ground, so that every
        // sample is the ground seen directly, its one next-event score at the end of two legs, the
        // view's column u₁ and the sun's u₂. Under a curve whose depth is √(u ÷ U), log–log linear
        // and so read exactly, the radiance is A ÷ π μ₀ F T(u₁ + u₂), not A ÷ π μ₀ F T(u₁) T(u₂).
        let (n, length, albedo, irradiance) = (1e20, 1000.0, 0.5, 1.6);
        let vertical = n * length;
        let mut case = slab(
            &json!([{
                "name": "layer", "density": uniform(),
                "scattering": [0.0, 0.0], "absorption": [0.0, 0.0], "phase": { "kind": "none" },
                "absorber": {
                    "kind": "column", "numberDensityPerM3": n,
                    "curves": [curve_json(vertical, 2, |w, u| {
                        (1.0 + f64::from(u8::try_from(w).unwrap())) * (u / vertical).sqrt()
                    })]
                }
            }]),
            &json!([albedo, albedo]),
            &json!([geometry("down", 1001.0, (150.0, 0.0), (60.0, 0.0))]),
        );
        case["suns"] = json!([{ "name": "sun", "irradiance": [irradiance, irradiance] }]);
        let reference = trace(&case, Polarisation::Scalar, 16, 1);
        let down = &reference.geometries()[0];
        let (mu_view, mu_sun) = (math::cos(radians(30.0)), math::cos(radians(60.0)));
        let (u1, u2) = (vertical / mu_view, vertical / mu_sun);
        for w in 0..2 {
            let k = [1.0, 2.0][w];
            let transmittance = |u: f64| math::exp(-k * (u / vertical).sqrt());
            let expected = albedo / PI * mu_sun * irradiance * transmittance(u1 + u2);
            let legs_apart =
                albedo / PI * mu_sun * irradiance * transmittance(u1) * transmittance(u2);
            // The slab lies on a sphere of 10¹² m, and the ground point lies 578 m from below the
            // observer, 5.8 × 10⁻¹⁰ rad round the sphere, so the chords and the sun's cosine there
            // are plane-parallel to about 2 × 10⁻⁹.
            assert!(
                (down.radiance()[w] / expected - 1.0).abs() < 1e-8,
                "{w}: {} against {expected}",
                down.radiance()[w]
            );
            assert!(
                legs_apart / expected - 1.0 < -0.2,
                "{w}: the curve saturates"
            );
            assert!(down.standard_error()[w] <= 1e-12 * expected);
            // The beam's depth is the curve's of its own column, as Beer–Lambert's would be.
            let beam = down.sun_optical_depth()[0][w].unwrap();
            assert!(
                beam.abs() < f64::MIN_POSITIVE,
                "the observer is above the slab: {beam}"
            );
        }
        // From the ground, the sun's beam crosses u₂ alone.
        let mut ground = case.clone();
        ground["geometries"] = json!([geometry("ground", 0.0, (0.0, 0.0), (60.0, 0.0))]);
        let reference = trace(&ground, Polarisation::Scalar, 4, 1);
        for w in 0..2 {
            let k = [1.0, 2.0][w];
            let beam = reference.geometries()[0].sun_optical_depth()[0][w].unwrap();
            let expected = k * (u2 / vertical).sqrt();
            assert!(
                (beam / expected - 1.0).abs() < 1e-8,
                "{w}: {beam} against {expected}"
            );
        }
    }

    #[test]
    fn atmosphere_a_geometrys_own_samples_change_its_draws_alone() {
        // R08.T12.b.1: a geometry's or an aggregate's own count leaves every other's draws alone,
        // and its values are those of a trace of the whole case at that count; each records its
        // count, and its first quarter's σ is the trace's at a quarter of it, across blocks too
        // (a quarter of 5,000 spans two of `BLOCK_SAMPLES`).
        let case = case::tests::sample_case();
        let mut own = case.clone();
        own["geometries"][1]["samples"] = json!(5000);
        own["aggregates"][0]["samples"] = json!(3000);
        let [base, mixed, at_5000, at_3000, at_1250, at_75] = [
            (&case, 300),
            (&own, 300),
            (&case, 5000),
            (&case, 3000),
            (&case, 1250),
            (&case, 75),
        ]
        .map(|(c, n)| trace(c, Polarisation::Scalar, n, 2));
        let json = |g: &GeometryRadiance| serde_json::to_string(g).unwrap();
        assert_eq!(json(&base.geometries()[0]), json(&mixed.geometries()[0]));
        assert_eq!(json(&at_5000.geometries()[1]), json(&mixed.geometries()[1]));
        let json = |a: &FluxAggregate| serde_json::to_string(a).unwrap();
        assert_eq!(json(&at_3000.aggregates()[0]), json(&mixed.aggregates()[0]));
        assert_ne!(json(&base.aggregates()[0]), json(&mixed.aggregates()[0]));
        assert_eq!(mixed.samples(), 300);
        assert_eq!(
            [
                mixed.geometries()[0].samples(),
                mixed.geometries()[1].samples()
            ],
            [300, 5000]
        );
        assert_eq!(mixed.geometries()[1].quarter_samples(), 1250);
        assert_eq!(mixed.aggregates()[0].samples(), 3000);
        assert_eq!(mixed.aggregates()[0].quarter_samples(), 750);
        // The first quarter's moments are the trace's at a quarter of the count, to the bit.
        let bits = |values: &[f64]| {
            values
                .iter()
                .map(|&v| hyperion_testkit::float::bits(v))
                .collect::<Vec<u64>>()
        };
        assert_eq!(
            bits(mixed.geometries()[1].quarter_standard_error()),
            bits(at_1250.geometries()[1].standard_error())
        );
        assert_eq!(
            bits(base.geometries()[0].quarter_standard_error()),
            bits(at_75.geometries()[0].standard_error())
        );
        let mut refused = case;
        refused["geometries"][0]["samples"] = json!(0);
        assert!(matches!(
            AtmosphereCase::from_json(&refused.to_string()),
            Err(ReadCaseError::Invalid { field, .. }) if field == "geometries[0].samples"
        ));
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
        // the magnetic one's in the plane: both are traced, by single scattering alone
        // ([`THIN_SLAB_SAMPLES`]). Q ÷ I, U ÷ I and V ÷ I of each view and wavelength, 48 values,
        // are each held to the table's interpolation, 2 × 10⁻⁴, and z_48 = 4.41σ, every σ falling
        // as N^(−1/2) over the run's first quarter (R08.T12.g).
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
        let mut agreement = Agreement::new("a sphere's single-scattering polarisation", 48);
        for (a, b, sign) in [(electric, magnetic, 1.0), (magnetic, electric, -1.0)] {
            let sphere = dipole_sphere(a, b);
            let terms = json!([{
                "name": "spheres", "density": uniform(),
                "scattering": [1e-8, 1e-8], "absorption": [0.0, 0.0],
                "phase": case::tests::tabulated_phase_json(512, 2, &sphere)
            }]);
            let [reference, quarter] = trace_and_quarter_over(
                &slab(&terms, &json!([0.0, 0.0]), &json!(geometries)),
                Polarisation::Stokes,
                THIN_SLAB_SAMPLES,
                2,
                Orders::First,
            );
            for ((traced, first), &view) in reference
                .geometries()
                .iter()
                .zip(quarter.geometries())
                .zip(&views)
            {
                let (towards_sun, _) = direction_and_e1(sun);
                let (look, _) = direction_and_e1(view);
                let [a1, _, _, _, b1, _] = sphere(dot3(towards_sun, look));
                let degree = -b1 / a1;
                assert!(sign * degree > 0.05, "{}: {degree}", traced.name());
                let (q_expected, u_expected) = single_scattering_polarisation(sun, view, degree);
                let (stokes, first) = (traced.stokes().unwrap(), first.stokes().unwrap());
                for w in 0..2 {
                    let intensity = traced.radiance()[w];
                    for (component, value, sigma, quarter_sigma, expected) in [
                        (
                            "Q",
                            stokes.q(),
                            stokes.q_standard_error(),
                            first.q_standard_error(),
                            q_expected,
                        ),
                        (
                            "U",
                            stokes.u(),
                            stokes.u_standard_error(),
                            first.u_standard_error(),
                            u_expected,
                        ),
                        (
                            "V",
                            stokes.v(),
                            stokes.v_standard_error(),
                            first.v_standard_error(),
                            0.0,
                        ),
                    ] {
                        let what =
                            format!("{}, {component} ÷ I at {w} (sign {sign})", traced.name());
                        agreement.sigma_scales(sigma[w], quarter_sigma[w], &what);
                        agreement.hold(
                            value[w] / intensity - expected,
                            sigma[w] / intensity,
                            2e-4,
                            what,
                        );
                    }
                }
            }
        }
        agreement.finish();
    }

    /// The samples of the two thin-slab polarisation tests, which trace single scattering alone
    /// ([`Orders::First`]; R08.T12.g, `decision-r08-t12f.md` §5). In a slab of τ 10⁻⁵ a rare
    /// second scattering, some 5 × 10⁻⁵ of a geometry's paths at a wavelength (measured), scores
    /// as much as a first, so that wherever one falls it is the σ: no standard error, and some
    /// 2 × 10⁻³ of I, which the Bonferroni bound widens to 40–80 times the tests' slack. Single
    /// scattering's σ, some 10⁻⁶ of I, is one: by it alone, the ratio of a σ at 500 samples to
    /// half its σ at 125 scattered by 4.2% at worst over 10 pilot seeds (V's σ is 0 at both). The
    /// second order, some 10⁻⁵ of I, was never resolved here.
    const THIN_SLAB_SAMPLES: u64 = 500;

    #[test]
    fn atmosphere_a_small_spheres_tabulated_matrix_traces_as_rayleigh() {
        // R08.T12.c: a thin slab of small spheres, their matrix tabulated from the amplitudes'
        // limit, against the built-in Rayleigh matrix at ρ = 0, by single scattering alone
        // ([`THIN_SLAB_SAMPLES`]): the same I, Q, U and V to the table's interpolation, 10⁻⁴ of I,
        // and z_24 = 4.26σ of the two runs, every σ falling as N^(−1/2) over the run's first
        // quarter (R08.T12.g). The scattering is the same in both, from the same draws.
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
            trace_and_quarter_over(
                &slab(&terms, &json!([0.0, 0.0]), &json!(geometries)),
                Polarisation::Stokes,
                THIN_SLAB_SAMPLES,
                2,
                Orders::First,
            )
        };
        let [built_in, built_in_quarter] =
            slab_of(json!({ "kind": "rayleigh", "depolarisation": [0.0, 0.0] }));
        let [tabulated, tabulated_quarter] = slab_of(case::tests::tabulated_phase_json(
            256,
            2,
            case::tests::small_sphere,
        ));
        // A run's I, Q, U and V and their σ at w, with its first quarter's σ.
        let components = |g: &GeometryRadiance, first: &GeometryRadiance, w: usize| {
            let (s, t) = (g.stokes().unwrap(), first.stokes().unwrap());
            [
                (
                    "I",
                    g.radiance()[w],
                    g.standard_error()[w],
                    first.standard_error()[w],
                ),
                (
                    "Q",
                    s.q()[w],
                    s.q_standard_error()[w],
                    t.q_standard_error()[w],
                ),
                (
                    "U",
                    s.u()[w],
                    s.u_standard_error()[w],
                    t.u_standard_error()[w],
                ),
                (
                    "V",
                    s.v()[w],
                    s.v_standard_error()[w],
                    t.v_standard_error()[w],
                ),
            ]
        };
        let mut agreement = Agreement::new("a small sphere's tabulated matrix", 24);
        for (((a, a_first), b), b_first) in built_in
            .geometries()
            .iter()
            .zip(built_in_quarter.geometries())
            .zip(tabulated.geometries())
            .zip(tabulated_quarter.geometries())
        {
            for w in 0..2 {
                let i = a.radiance()[w];
                for ((component, x, sx, qx), (_, y, sy, qy)) in components(a, a_first, w)
                    .into_iter()
                    .zip(components(b, b_first, w))
                {
                    let what = format!("{}, {component} at {w}", a.name());
                    agreement.sigma_scales(sx, qx, format_args!("{what}, Rayleigh's"));
                    agreement.sigma_scales(sy, qy, format_args!("{what}, the table's"));
                    agreement.hold(x - y, math::hypot(sx, sy), 1e-4 * i, what);
                }
            }
        }
        agreement.finish();
    }

    #[test]
    fn atmosphere_circular_polarisation_is_odd_in_azimuth_and_absent_without_b2() {
        // V arises only from U through b₂ in multiple scattering (the coupling itself is
        // `transport`'s test). Over a plane-parallel slab lit by unpolarised light, V and U are odd
        // in the relative azimuth and I and Q even (the mirror symmetry of the principal plane,
        // which turns e₂ = k × e₁ over): at ±60° they cancel, each of the 12 sums or differences
        // to z_12 = 4.10σ, every σ falling as N^(−1/2) over the run's first quarter (R08.T12.g).
        // V is small, some 10⁻³ of I here, as in Kokhanovsky et al. 2010's tables (10⁻⁴ to
        // 10⁻³). Rayleigh, whose b₂ is 0, makes none at all.
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
        // 256,000 samples, from a pilot at 10 other seeds (R08.T12.g): at R08.T12.c's 20,000 the
        // ratio of V's σ at N to half its σ at N ÷ 4 scattered by 12% over them, by 5.3% at
        // 160,000 and by 4.0% here.
        let [reference, quarter] = trace_and_quarter(
            &slab(&terms, &json!([0.0, 0.0]), &geometries),
            Polarisation::Stokes,
            256_000,
            4,
        );
        let mut agreement = Agreement::new("circular polarisation's symmetry", 12);
        for (pair, first) in reference
            .geometries()
            .chunks(2)
            .zip(quarter.geometries().chunks(2))
        {
            let [plus, minus, first_plus, first_minus] =
                [&pair[0], &pair[1], &first[0], &first[1]].map(|g| g.stokes().unwrap());
            for w in 0..2 {
                // Each component's value and σ at w.
                let components = |s: &StokesRadiance| {
                    [
                        (s.v()[w], s.v_standard_error()[w]),
                        (s.u()[w], s.u_standard_error()[w]),
                        (s.q()[w], s.q_standard_error()[w]),
                    ]
                };
                // V and U odd in the azimuth, so that a pair's sum is 0, and Q even, its
                // difference.
                for (k, (component, sign)) in [("V", 1.0), ("U", 1.0), ("Q", -1.0)]
                    .into_iter()
                    .enumerate()
                {
                    let [(x, sx), (y, sy), (_, qx), (_, qy)] =
                        [plus, minus, first_plus, first_minus].map(|s| components(s)[k]);
                    let what = format!("{}, {component} at {w}", pair[0].name());
                    agreement.sigma_scales(sx, qx, format_args!("{what}, at +60°"));
                    agreement.sigma_scales(sy, qy, format_args!("{what}, at −60°"));
                    agreement.hold(x + sign * y, math::hypot(sx, sy), 0.0, what);
                }
            }
        }
        agreement.finish();
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
        // slab of τ = 1 the radiances agree. (Both draw from tables, so their paths are close and
        // the comparison is lenient; the forward peak leaves σ near 2% away from the sun at
        // 40,000 samples.) Rayleigh's series, β = (1, 0, ½), against the built-in phase
        // function's exact draws is the independent check: there the paths differ, and the two
        // agree at σ near 0.5%. Each of the 12 values is held to z_12 = 4.10σ, and every σ falls
        // as N^(−1/2) over the run's first quarter (R08.T12.g).
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
            trace_and_quarter(
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
        let mut agreement = Agreement::new("a Legendre series against its table", 12);
        for (pair, other, [a, a_quarter], [b, b_quarter]) in [
            ("Henyey–Greenstein", "the table", &series, &tabulated),
            (
                "Rayleigh",
                "the built-in function",
                &rayleigh_series,
                &rayleigh,
            ),
        ] {
            for (((ga, qa), gb), qb) in a
                .geometries()
                .iter()
                .zip(a_quarter.geometries())
                .zip(b.geometries())
                .zip(b_quarter.geometries())
            {
                for w in 0..2 {
                    let (x, y) = (ga.radiance()[w], gb.radiance()[w]);
                    let (sa, sb) = (ga.standard_error()[w], gb.standard_error()[w]);
                    let what = format!("{pair}, {} at {w}", ga.name());
                    agreement.sigma_scales(
                        sa,
                        qa.standard_error()[w],
                        format_args!("{what}, the series"),
                    );
                    agreement.sigma_scales(
                        sb,
                        qb.standard_error()[w],
                        format_args!("{what}, {other}"),
                    );
                    let sigma = math::hypot(sa, sb);
                    assert!(sigma < 0.05 * x, "{what}: {x} ± {sigma}");
                    agreement.hold(x - y, sigma, 0.0, what);
                }
            }
        }
        agreement.finish();
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

    /// A uniform slab of Rayleigh scattering `sigma_s` and absorption `sigma_a` (m⁻¹, at both
    /// wavelengths, ρ 0 and 0.03) over a black ground, seen up from the ground, down from the
    /// top and horizontally from 500 m to a black target 1 km off, with each geometry's singly
    /// scattered radiance per wavelength.
    #[must_use]
    fn single_scattering_slab(sigma_s: f64, sigma_a: f64) -> (Value, [[f64; 3]; 2]) {
        let top = 1000.0;
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
        let tau = (sigma_s + sigma_a) * top;
        let omega = sigma_s / (sigma_s + sigma_a);
        let irradiance = [PI, 2.0];
        let expected = [0, 1].map(|w| {
            let f = irradiance[w] * omega;
            let phase = |view, sun| rayleigh(rho[w], cos_between(view, sun));
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
            [down, up, side]
        });
        (slab(&terms, &json!([0.0, 0.0]), &geometries), expected)
    }

    #[test]
    fn atmosphere_thin_layer_matches_single_scattering() {
        // τₛ = 10⁻⁵ and τₐ = 5 × 10⁻⁶: multiple scattering is some 10⁻⁵ of the radiance.
        let (case, expected) = single_scattering_slab(1e-8, 5e-9);
        let reference = trace(&case, Polarisation::Scalar, 2000, 4);
        for (w, expected) in expected.iter().enumerate() {
            for (traced, &expected) in reference.geometries().iter().zip(expected) {
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
    fn atmosphere_first_order_alone_is_single_scattering() {
        // R08.T12.g's `Orders::First` ends every path once it has made its first vertex's
        // estimates. In the thin-layer test's slab at τₛ 0.5 and τₐ 0.25, where the orders past
        // the first add 38–68% to the radiance (measured), single scattering's is left, to 2%,
        // each σ held under 0.25% of it (a fixed tolerance, so its σ is a cap and no agreement;
        // at most 0.214% at 40,000 samples in a pilot at 3 other seeds). In a slab of
        // Henyey–Greenstein g 0.9, which turns VROOM on, the first order is R08.T12.c's
        // estimator's to the bit: VROOM makes no clone, copy or later estimate there.
        let (case, expected) = single_scattering_slab(5e-4, 2.5e-4);
        let first = trace_by(
            &case,
            Polarisation::Scalar,
            FIRST_ORDER_SAMPLES,
            4,
            Estimator::ByMedium,
            Orders::First,
        );
        let every = trace(&case, Polarisation::Scalar, FIRST_ORDER_SAMPLES, 4);
        for (w, expected) in expected.iter().enumerate() {
            for ((traced, all), &expected) in first
                .geometries()
                .iter()
                .zip(every.geometries())
                .zip(expected)
            {
                let (radiance, sigma) = (traced.radiance()[w], traced.standard_error()[w]);
                assert!(
                    (radiance / expected - 1.0).abs() < 2e-2 && sigma < 2.5e-3 * expected,
                    "{} at {w}: {radiance} ± {sigma} against {expected}",
                    traced.name()
                );
                assert!(
                    all.radiance()[w] > 1.2 * radiance,
                    "{} at {w}: every order's {} against the first's {radiance}",
                    traced.name(),
                    all.radiance()[w]
                );
            }
        }
        let mut peaked = case;
        peaked["terms"][0]["phase"] = case::tests::tabulated_phase_json(512, 2, |c| {
            [
                optics::tests::henyey_greenstein(0.9, c),
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ]
        });
        let first_by = |estimator| {
            trace_by(
                &peaked,
                Polarisation::Scalar,
                2000,
                4,
                estimator,
                Orders::First,
            )
            .to_json()
        };
        assert_eq!(
            first_by(Estimator::Vroom),
            first_by(Estimator::SunwardDraws)
        );
    }

    /// The samples of `atmosphere_first_order_alone_is_single_scattering`.
    const FIRST_ORDER_SAMPLES: u64 = 40_000;

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
        // Each of the 4 balances to 4σ, every σ falling as N^(−1/2) over the run's first quarter
        // (R08.T12.g).
        let [reference, quarter] = trace_and_quarter(&case, Polarisation::Scalar, 20_000, 4);
        let mut agreement = Agreement::new("a conservative layer's energy", 4);
        for (aggregate, first) in reference.aggregates().iter().zip(quarter.aggregates()) {
            assert_balances(&mut agreement, aggregate, first, 0.01);
        }
        agreement.finish();
    }

    /// Asserts that `aggregate`'s fluxes balance its incident flux, with `first` its run's first
    /// quarter: over a black ground (the first wavelength) what is not reflected is transmitted,
    /// diffusely or in the direct beam, and over a white one (the second) everything comes back
    /// up. The first balance's σ must be under `sigma_limit` of the incident flux.
    fn assert_balances(
        agreement: &mut Agreement,
        aggregate: &FluxAggregate,
        first: &FluxAggregate,
        sigma_limit: f64,
    ) {
        let name = aggregate.name();
        let incident = aggregate.incident_top();
        let (up, down) = (
            aggregate.upwelling_top_standard_error(),
            aggregate.diffuse_ground_standard_error(),
        );
        agreement.sigma_scales(
            up[0],
            first.upwelling_top_standard_error()[0],
            format_args!("{name} up at 0"),
        );
        agreement.sigma_scales(
            down[0],
            first.diffuse_ground_standard_error()[0],
            format_args!("{name} down at 0"),
        );
        agreement.sigma_scales(
            up[1],
            first.upwelling_top_standard_error()[1],
            format_args!("{name} up at 1"),
        );
        let total = aggregate.upwelling_top()[0]
            + aggregate.direct_ground()[0]
            + aggregate.diffuse_ground()[0];
        let sigma = math::hypot(up[0], down[0]);
        assert!(
            sigma < sigma_limit * incident[0],
            "{name}: σ {sigma} of {}",
            incident[0]
        );
        agreement.hold(
            total - incident[0],
            sigma,
            0.0,
            format_args!("{name}, the black ground"),
        );
        agreement.hold(
            aggregate.upwelling_top()[1] - incident[1],
            up[1],
            0.0,
            format_args!("{name}, the white ground"),
        );
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

    /// Asserts that every radiance and flux of `a` and `b`, traced on the same draws, agrees to
    /// `relative` of the value, and that their direct beams agree.
    fn assert_traces_agree(a: &ReferenceRadiances, b: &ReferenceRadiances, relative: f64) {
        let close = |x: f64, y: f64| (x - y).abs() <= relative * x.abs();
        for (g, h) in a.geometries().iter().zip(b.geometries()) {
            for w in 0..g.radiance().len() {
                let (x, y) = (g.radiance()[w], h.radiance()[w]);
                let (sx, sy) = (g.standard_error()[w], h.standard_error()[w]);
                assert!(
                    close(x, y),
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
                    close(x, y),
                    "{} at {w}: {x} ± {sx} against {y} ± {sy}",
                    f.name()
                );
                let (x, sx) = (f.upwelling_top()[w], f.upwelling_top_standard_error()[w]);
                let (y, sy) = (k.upwelling_top()[w], k.upwelling_top_standard_error()[w]);
                assert!(
                    close(x, y),
                    "{} at {w}: {x} ± {sx} against {y} ± {sy}",
                    f.name()
                );
            }
        }
    }

    /// Every stochastic value of `reference` with its σ and what it is: each geometry's radiance
    /// and each aggregate's diffuse and upwelling fluxes, per wavelength.
    #[must_use]
    fn stochastic_values(reference: &ReferenceRadiances) -> Vec<(String, f64, f64)> {
        let mut values = Vec::new();
        for g in reference.geometries() {
            for (w, (&x, &sigma)) in g.radiance().iter().zip(g.standard_error()).enumerate() {
                values.push((format!("{} at {w}", g.name()), x, sigma));
            }
        }
        for f in reference.aggregates() {
            for w in 0..f.incident_top().len() {
                values.push((
                    format!("{} down at {w}", f.name()),
                    f.diffuse_ground()[w],
                    f.diffuse_ground_standard_error()[w],
                ));
                values.push((
                    format!("{} up at {w}", f.name()),
                    f.upwelling_top()[w],
                    f.upwelling_top_standard_error()[w],
                ));
            }
        }
        values
    }

    #[test]
    fn atmosphere_spheroid_with_a_equal_to_c_agrees_with_the_sphere() {
        // R08.T12.d: a = c with ω = 0 on the Earth-like sample case, against `Shells::Sphere` of
        // the same radius. On the same draws the paths are the sphere's to rounding (measured
        // 4 × 10⁻¹³), so they agree to 10⁻⁹, far inside the task's 3σ. The slow test below
        // repeats it at 10⁵ samples, and compares independent draws (R08.T12.g).
        let sphere = trace(&case::tests::sample_case(), Polarisation::Scalar, 1500, 4);
        let same = trace(&sample_case_as_a_spheroid(0), Polarisation::Scalar, 1500, 4);
        assert_traces_agree(&sphere, &same, 1e-9);
    }

    #[test]
    #[ignore = "slow: 10⁵ samples of each source over a sphere and a spheroid, and 5 × 10⁴ over another two; about 3 minutes on 4 threads"]
    fn atmosphere_spheroid_with_a_equal_to_c_follows_the_spheres_paths() {
        // The fast test's comparison over 10⁵ paths a source, so that rare paths (grazing,
        // reflected many times, rouletted) are held to the sphere's too.
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
        assert_traces_agree(&sphere, &spheroid, 1e-9);
        // On independent draws (the spheroid at seed 1), moved here from the fast test (R08.T12.g;
        // `decision-r08-t12f.md` §5): each of the 8 stochastic values to 4σ, a family-wise α of
        // 5 × 10⁻⁴ (the sim-determinism rule's α = 10⁻³; 3σ over 8 values would fail 2% of the
        // time), and every σ on both sides falling as N^(−1/2) over the run's first quarter. At
        // 40,000 samples, from a pilot at 10 other seeds: at R08.T12.d's 1,500 the ratio of a σ at
        // N to half its σ at N ÷ 4 scattered by 13% over them at worst (the view from orbit at
        // 550 nm), by 6.3% at 16,000 and by 2.7% here.
        let [sphere, sphere_quarter] =
            trace_and_quarter(&case::tests::sample_case(), Polarisation::Scalar, 40_000, 4);
        let [independent, independent_quarter] = trace_and_quarter(
            &sample_case_as_a_spheroid(1),
            Polarisation::Scalar,
            40_000,
            4,
        );
        let mut agreement = Agreement::new("a = c on independent draws", 8);
        for (((what, x, sx), (_, _, qx)), ((_, y, sy), (_, _, qy))) in stochastic_values(&sphere)
            .into_iter()
            .zip(stochastic_values(&sphere_quarter))
            .zip(
                stochastic_values(&independent)
                    .into_iter()
                    .zip(stochastic_values(&independent_quarter)),
            )
        {
            agreement.sigma_scales(sx, qx, format_args!("the sphere's {what}"));
            agreement.sigma_scales(sy, qy, format_args!("the spheroid's {what}"));
            agreement.hold(x - y, math::hypot(sx, sy), 0.0, what);
        }
        agreement.finish();
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
        // As the sphere's test: 4 balances to 4σ, every σ falling as N^(−1/2) (R08.T12.g).
        let [reference, quarter] = trace_and_quarter(&case, Polarisation::Scalar, 10_000, 4);
        let mut agreement = Agreement::new("a flattened conservative layer's energy", 4);
        for (aggregate, first) in reference.aggregates().iter().zip(quarter.aggregates()) {
            assert_balances(&mut agreement, aggregate, first, 0.015);
        }
        agreement.finish();
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
        let pinned = |estimator: Estimator| {
            let reference = trace_with(
                &case::tests::saturn_case(),
                Polarisation::Scalar,
                64,
                1,
                estimator,
            );
            [
                format!("{:?}", reference.geometries()[1].radiance()[0]),
                format!("{:?}", reference.aggregates()[0].upwelling_top()[1]),
                format!("{:?}", reference.geometries()[3].radiance()[0]),
                format!(
                    "{:?}",
                    reference.geometries()[2].sun_optical_depth()[0][0].unwrap()
                ),
            ]
        };
        // R08.T12.c's estimator, unchanged to the bit by R08.T12.f.
        assert_eq!(
            pinned(Estimator::SunwardDraws),
            [
                "0.2276473839689261",
                "0.8929036838129342",
                "0.2583970654014467",
                "1.3245982101763956"
            ]
        );
        // VROOM, which the haze's forward peak turns on, moved the three traced values; the beam
        // is a quadrature.
        assert_eq!(
            pinned(Estimator::ByMedium),
            [
                "0.23452698641642764",
                "0.7895400585805432",
                "0.25315517266447024",
                "1.3245982101763956"
            ]
        );
    }
}
