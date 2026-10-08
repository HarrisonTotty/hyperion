//! The sky's work on the CPU pool (rendering plan R06, R06.T11.a and T11.c; Design notes 5, 9–11
//! and 14–16): the caps, the census, the band, the limit map and the eye offsets, as bulk jobs.
//!
//! Everything a `sky` request computes runs at [`Priority::Bulk`], never in the interactive queue,
//! so a chart's query is never queued behind it: the workers take interactive work first. The work
//! is split into jobs of a fraction of a second where the sim allows it, so that a query waits for
//! little on a busy worker. A few steps are single jobs of seconds, which T11.d sizes: the tables'
//! plan and assembly, the eye's cut, the glare and the eye offsets. In order:
//!
//! - the galaxy's tables are built once and kept ([`SkyTablesService`](super::SkyTablesService));
//! - the eye's cut is one job, two coarse pre-passes of the band;
//! - the caps' 1,536 rays, each through its three sub-rays (R06.T7.b), are measured
//!   [`CAP_JOB_RAYS`] to a job, each with its own noise cache,
//!   then counted in ray order in one more job, which makes the census's plan (decided
//!   2026-10-03, `decision-r06-tables.md`, item B.4);
//! - the census runs its plan's cells a few hundred to a job ([`CENSUS_JOB_CELLS`]);
//! - while it runs, the band's rays are marched [`BAND_JOB_ROWS`] rows of a face to a job (R06.T9.f's
//!   `march_rows`), which reads no census;
//! - once the census is merged, each march is summed with the census's overflow (`sum_rows`) and
//!   given the eye's limits against the listed stars' glare (`limit_rows`), a job each, and every
//!   listed star its eye offset in one more job (`eye_offsets`).
//!
//! Every job runs under the request's [`CancelToken`]: a cancelled request's queued jobs are
//! skipped, and a running census job stops at its next cell. Work that ends early, because a job
//! failed or the request's future was dropped, abandons its other jobs too: each one not yet begun
//! returns at once. Each split is merged in a fixed order, so it cannot change the answer: the
//! census's merge is total, the rays and the band's rows are put back in order, and every texel and
//! ray is a function of its own direction. Neither a census's sources nor a noise cache can be
//! shared between threads, so each job builds its own [`SkyContext`] over the [`SkyTables`] they
//! all read, and the census's jobs over the server's [`SharedSkyCellCache`], which every job and
//! request shares (R06.T11.b; Design note 12).
//!
//! [`SkyCaps`] says how far the census looks: each layer's derived cap (Design note 9), or one
//! forced radius, which keeps a test's census small.

use std::error::Error;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::CellKey;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::band::{
    BandMarch, BandSpec, BandTexel, CompleteTo, CubeFace, march_rows, sum_rows,
};
use hyperion_sim::sky::caps::{
    CAP_RAYS, CapLattice, RayExtinctions, SUB_RAYS, layer_caps_by_visibility_over, layer_caps_over,
};
use hyperion_sim::sky::census::{
    CellOffsets, CensusPlan, CensusTallies, MAX_FORCED_CAP_LY, NoSkyCellCache, SkyCellCache,
    SkyCensus, SkyContext, SkyQuery, SkyStar, census_cell, census_plan, census_plan_of,
    merge_census,
};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::limits::{self, Glare};
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::units::{LightYears, Magnitudes};
use tokio::sync::oneshot::error::RecvError;

use super::{
    CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyKey, JobError, JobReceiver, Priority,
    SharedSkyCellCache,
};
use crate::limits::BULK_QUEUE_CAPACITY;

/// The cells one census job takes: a few hundred (R06.T11.a).
///
/// The pool never preempts a running job, so this bounds how long a chart's query can wait behind
/// the census on a worker. Near the Sun most cells are skipped by their floor in microseconds and a
/// few take milliseconds, so a job is tens of milliseconds; T11.d sizes them by expected cost
/// (`decision-r06-census-cost-signoff.md`, item 3).
pub(crate) const CENSUS_JOB_CELLS: usize = 256;

/// The caps' rays one job measures: 8 of the 1,536, 192 jobs, each ray the clearest of its three
/// sub-rays' realised profiles (R06.T7.b), some 24 profiles a job, as R06.T7's 24 rays a job of
/// 768 were.
pub(crate) const CAP_JOB_RAYS: usize = 8;

/// The rows of a face one band job marches, then sums and maps: two of the server's 64, 128 rays,
/// some 0.1 s of a worker, 192 jobs for the band (R06.T9.f's bench: about 0.8 ms a ray in release
/// and 1.1 ms in the test profile, four fifths of it the luminosity functions' reads).
///
/// Each job's sum also walks the census's whole overflow to find its texels' stars, some 15 ms at
/// 10⁶ overflowing stars, a camera's sky at V 11 near the Sun; at the eye's cut nothing overflows.
pub(crate) const BAND_JOB_ROWS: u16 = 2;

/// Noise-cache slots of a census job: 256 KiB, a cache's worth for the sightlines of a few hundred
/// cells' stars. The cache changes no value, only how often a normal is drawn again.
const CENSUS_NOISE_SLOTS: usize = 1 << 14;

/// Noise-cache slots of the jobs that march many rays (the eye's pre-pass, the caps and the band):
/// 1 MiB.
const MARCH_NOISE_SLOTS: usize = 1 << 16;

/// How far a sky's census looks in each layer (Design note 9), and which luminosity tables it reads:
/// the caps the galaxy's tables and dust derive, or one radius forced on every layer.
///
/// Forced caps are for tests and benches, which need a census of seconds rather than of the
/// thousands of CPU-seconds a census to the derived caps near the Sun takes (R06's Risks, the
/// census cost). A census with forced caps reads no luminosity table, so by default it is given
/// tables that hold no star ([`LuminosityTables::dark`]), built at once; its band then holds only
/// the overflow's stars, and its eye's cut is a dark sky's: Crumey's darkest-background limit plus
/// the cut's colour offset and pad, V 8.54 for the default eye. A test of the band's light asks for
/// the galaxy's own tables instead ([`with_galaxy_tables`](Self::with_galaxy_tables)), built on the
/// pool as the derived caps' are. A reply states each forced layer's cap as the radius, with
/// nothing expected beyond it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SkyCaps {
    forced: Option<LightYears>,
    tables: SkyTablesSource,
}

/// The luminosity tables a sky reads (Design note 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum SkyTablesSource {
    /// The galaxy's own, built once per galaxy on the pool: the server's, whatever its caps.
    #[default]
    Galaxy,
    /// Tables that hold no star ([`LuminosityTables::dark`]), built at once: a test's, under forced
    /// caps, whose census reads no table.
    Dark,
}

impl SkyCaps {
    /// Each layer's cap derived from the galaxy's own tables, the default.
    pub const DERIVED: Self = Self {
        forced: None,
        tables: SkyTablesSource::Galaxy,
    };

    /// Every layer's cap forced to `radius`, over tables that hold no star.
    ///
    /// # Errors
    ///
    /// [`ForceSkyCapsError`] unless `radius` is finite, non-negative and within the root cube's
    /// diagonal ([`MAX_FORCED_CAP_LY`]).
    pub fn forced(radius: LightYears) -> Result<Self, ForceSkyCapsError> {
        if (0.0..=MAX_FORCED_CAP_LY).contains(&radius.value()) {
            Ok(Self {
                forced: Some(radius),
                tables: SkyTablesSource::Dark,
            })
        } else {
            Err(ForceSkyCapsError)
        }
    }

    /// The same caps over the galaxy's own luminosity tables, built on the pool: for a test of the
    /// band's light and of the tables' build, under forced caps.
    #[must_use]
    pub const fn with_galaxy_tables(self) -> Self {
        Self {
            tables: SkyTablesSource::Galaxy,
            ..self
        }
    }

    /// The radius every layer's cap is forced to, or `None` for the derived caps.
    #[must_use]
    pub const fn forced_radius(&self) -> Option<LightYears> {
        self.forced
    }

    /// The luminosity tables the sky reads.
    #[must_use]
    pub const fn tables(&self) -> SkyTablesSource {
        self.tables
    }
}

/// A forced sky cap that is not finite, is negative, or reaches past the root cube's diagonal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ForceSkyCapsError;

impl fmt::Display for ForceSkyCapsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a forced sky cap is not within 0 to {MAX_FORCED_CAP_LY} ly, the root cube's diagonal"
        )
    }
}

impl Error for ForceSkyCapsError {}

/// What a sky's jobs read beside the galaxy: its luminosity tables, the brightness envelope and its
/// cells' offset bounds (Design notes 7, 8 and 10).
///
/// Built once per galaxy and kept by the [`SkyTablesService`](super::SkyTablesService).
#[derive(Debug)]
pub(crate) struct SkyTables {
    tables: LuminosityTables,
    envelope: BrightnessEnvelope,
    offsets: CellOffsets,
}

impl SkyTables {
    /// The tables `tables` of `galaxy`, with the galaxy's envelope, the fitted table, and its
    /// cells' offset bounds, a few thousand tidal radii.
    #[must_use]
    pub(crate) fn new(tables: LuminosityTables, galaxy: &Galaxy) -> Self {
        Self {
            tables,
            envelope: BrightnessEnvelope::build(galaxy),
            offsets: CellOffsets::build(galaxy),
        }
    }

    /// The bytes the tables, the envelope and the offset bounds own on the heap.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.tables.heap_bytes() + self.envelope.heap_bytes() + self.offsets.heap_bytes()
    }

    /// A job's own context over these tables, with a noise cache of `noise_slots`, the cells'
    /// bright subsets from `cells`, no sources beyond the grid and no gas modifiers (plan 09's
    /// feature members and their gas, from R06.T16.a).
    #[must_use]
    fn context<'a>(&'a self, noise_slots: usize, cells: &'a dyn SkyCellCache) -> SkyContext<'a> {
        SkyContext {
            tables: &self.tables,
            envelope: &self.envelope,
            offsets: &self.offsets,
            noise: NoiseCache::with_capacity(noise_slots),
            cells,
            sources: &[],
            modifiers: &NoModifiers,
        }
    }

    /// A context for a job that marches many rays: the eye's pre-pass and the band.
    ///
    /// They read no cell, so it has no cell cache.
    #[must_use]
    pub(crate) fn march_context(&self) -> SkyContext<'_> {
        self.context(MARCH_NOISE_SLOTS, &NoSkyCellCache)
    }
}

/// Runs `job` as a bulk job under `token` and waits for it, waiting for a place in the bulk queue
/// first.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the pool is shutting down or faulted, and [`ComputeError::Job`] if
/// the job was cancelled, panicked or was dropped with the pool.
pub(crate) async fn bulk<F, T>(
    pool: &CpuPool,
    token: &CancelToken,
    job: F,
) -> Result<T, ComputeError>
where
    F: FnOnce(&CancelToken) -> T + Send + 'static,
    T: Send + 'static,
{
    received(pool.submit(Priority::Bulk, token.clone(), job).await?).await
}

/// What a job sent back, or why it sent nothing.
///
/// # Errors
///
/// [`ComputeError::Job`] if the job was cancelled, panicked or was dropped with the pool.
async fn received<T>(receiver: JobReceiver<T>) -> Result<T, ComputeError> {
    Ok(receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?)
}

/// Bulk jobs queued and not yet waited for, whose results come back in the order they were queued.
///
/// Dropping it, or a failed job, abandons the rest: each job not yet begun then returns at once.
pub(crate) struct BulkJobs<T> {
    pending: Vec<JobReceiver<Option<T>>>,
    /// Raised when these jobs are given up, however that happens.
    abandoned: CancelOnDrop,
}

impl<T: Send + 'static> BulkJobs<T> {
    /// Queues each of `jobs` as a bulk job under `token`, in order, waiting for a place in the bulk
    /// queue for each.
    ///
    /// # Errors
    ///
    /// [`ComputeError::Submit`] if the pool is shutting down or faulted. The jobs queued before then
    /// are abandoned.
    pub(crate) async fn submit<J>(
        pool: &CpuPool,
        token: &CancelToken,
        jobs: impl IntoIterator<Item = J>,
    ) -> Result<Self, ComputeError>
    where
        J: FnOnce() -> T + Send + 'static,
    {
        let abandoned = CancelOnDrop::new(CancelToken::new());
        let mut pending = Vec::new();
        for job in jobs {
            let given_up = abandoned.token().clone();
            pending.push(
                pool.submit(Priority::Bulk, token.clone(), move |_: &CancelToken| {
                    (!given_up.is_cancelled()).then(job)
                })
                .await?,
            );
        }
        Ok(Self { pending, abandoned })
    }

    /// Each job's result, in the order the jobs were queued.
    ///
    /// # Errors
    ///
    /// [`ComputeError::Job`] if a job was cancelled, panicked or was dropped with the pool. The
    /// jobs not yet begun are abandoned.
    pub(crate) async fn results(self) -> Result<Vec<T>, ComputeError> {
        let Self { pending, abandoned } = self;
        let mut results = Vec::with_capacity(pending.len());
        for receiver in pending {
            results.push(received(receiver).await?.ok_or(JobError::Cancelled)?);
        }
        drop(abandoned);
        Ok(results)
    }
}

/// The eye's cut for `eye` at `observer` (Design note 5; R06.T9.d), in one bulk job under `token`:
/// two coarse pre-passes of the band, 1,536 rays each, near the Sun.
///
/// # Errors
///
/// Those of [`bulk`].
pub(crate) async fn eye_cut(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    tables: &Arc<SkyTables>,
    observer: Observer,
    eye: EyeObserver,
    token: &CancelToken,
) -> Result<Magnitudes, ComputeError> {
    let (galaxy, tables) = (Arc::clone(galaxy), Arc::clone(tables));
    // No illumination yet: R06.T11.d states the request's (decision-r06-t9g-dgl.md §3.4), so
    // until then the eye's cut is reckoned against the starlight alone, as before R06.T9.g.
    bulk(pool, token, move |_: &CancelToken| {
        limits::eye_cut(&galaxy, &mut tables.march_context(), &observer, &eye, None)
    })
    .await
}

/// The census's plan for `query` (Design notes 9 and 10): each layer's cap and the cells they open.
///
/// The query's forced caps are the plan's at once, in one bulk job. Derived caps count the galaxy's
/// stars along [`CAP_RAYS`] rays, each through the clearest of its [`SUB_RAYS`] sub-rays, whose
/// extinction profiles are measured [`CAP_JOB_RAYS`] to a bulk job after one job sets the rays'
/// lattice, each with its own noise cache, and joined in ray order; one more job counts them,
/// serially in ray order, by the eye's visibility where the query asks it, and makes the plan
/// (decided 2026-10-03, `decision-r06-tables.md`, item B.4; R06.T7.b). The caps are so
/// `layer_caps`' (or `layer_caps_by_visibility`'s) bit for bit.
///
/// # Errors
///
/// Those of [`bulk`].
pub(crate) async fn plan(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    tables: &Arc<SkyTables>,
    query: &Arc<SkyQuery>,
    token: &CancelToken,
) -> Result<CensusPlan, ComputeError> {
    let (galaxy, tables, query) = (Arc::clone(galaxy), Arc::clone(tables), Arc::clone(query));
    if query.forced_caps().is_some() {
        // A forced query's plan reads no table and marches no ray.
        return bulk(pool, token, move |_: &CancelToken| {
            let mut noise = NoiseCache::with_capacity(0);
            census_plan(
                &galaxy,
                &tables.tables,
                &tables.envelope,
                &query,
                &mut noise,
            )
        })
        .await;
    }
    let origin = *query.observer().position();
    // The lattice's spacing, which sets each ray's sub-rays, is some 0.1 s: one job, shared.
    let lattice = bulk(pool, token, |_: &CancelToken| {
        Arc::new(CapLattice::new(CAP_RAYS))
    })
    .await?;
    let shares = (0..CAP_RAYS).step_by(CAP_JOB_RAYS).map(|first| {
        let which = first..(first + CAP_JOB_RAYS).min(CAP_RAYS);
        let (galaxy, lattice) = (Arc::clone(&galaxy), Arc::clone(&lattice));
        move || {
            let mut noise = NoiseCache::with_capacity(MARCH_NOISE_SLOTS);
            RayExtinctions::measure_clearest_rays(
                &galaxy, &origin, lattice, SUB_RAYS, which, &mut noise,
            )
        }
    });
    let shares = BulkJobs::submit(pool, token, shares)
        .await?
        .results()
        .await?;
    bulk(pool, token, move |_: &CancelToken| {
        let rays = RayExtinctions::join(shares);
        let (tables, envelope, observer) = (&tables.tables, &tables.envelope, query.observer());
        let caps = match query.eye_visibility() {
            Some(visibility) => layer_caps_by_visibility_over(
                &galaxy, tables, envelope, observer, visibility, &rays,
            ),
            None => layer_caps_over(&galaxy, tables, envelope, observer, query.cut(), &rays),
        };
        census_plan_of(&query, caps)
    })
    .await
}

/// One census job's part: its cells' stars and their tallies, `None` for a job that held no cell.
type Part = (Vec<SkyStar>, Option<CensusTallies>);

/// Adds `tallies` to `sum`, starting the sum from the first, as [`merge_census`] starts from its
/// first part's, so that a flag every cell clears stays clear.
fn add_tallies(sum: &mut Option<CensusTallies>, tallies: &CensusTallies) {
    match sum {
        Some(sum) => sum.add(tallies),
        None => *sum = Some(*tallies),
    }
}

/// The census of the inputs' query over `plan`'s cells, as bulk jobs of [`CENSUS_JOB_CELLS`] cells
/// under `token`, merged once every job has finished (Design notes 10 and 11).
///
/// The cells are handed out in the plan's canonical order, and at most [`BULK_QUEUE_CAPACITY`] jobs
/// are outstanding at once, so the parts held while the census runs stay bounded. Each job builds
/// its own [`SkyContext`] over the inputs' tables, and reads each cell's bright subset through
/// their cell cache, which never changes the census (Design note 12). A job stops at its next cell
/// once `token` is cancelled, and the pool skips those still queued. A census that ends early,
/// because a job failed or this future was dropped, abandons its other jobs too: each stops before
/// its next cell. The parts are joined as they arrive and merged in one more bulk job, which is the
/// same census as merging them part by part: the merge's order is total, and its tallies are sums.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the pool is shutting down or faulted, and [`ComputeError::Job`] if a
/// job was cancelled, panicked or was dropped with the pool.
pub(crate) async fn census(
    pool: &CpuPool,
    inputs: &CensusInputs,
    plan: &CensusPlan,
    token: &CancelToken,
) -> Result<SkyCensus, ComputeError> {
    // Raised when this census ends, however it ends: its jobs still outstanding then are of no use.
    let abandoned = CancelOnDrop::new(CancelToken::new());
    let mut planned = plan.cells();
    let mut pending = FuturesUnordered::new();
    let mut stars = Vec::new();
    let mut tallies = None;
    let mut absorb = |part: Part| {
        let (part_stars, part_tallies) = part;
        stars.extend(part_stars);
        if let Some(part_tallies) = part_tallies {
            add_tallies(&mut tallies, &part_tallies);
        }
    };
    loop {
        let chunk: Vec<CellKey> = planned.by_ref().take(CENSUS_JOB_CELLS).collect();
        if chunk.is_empty() {
            break;
        }
        while pending.len() >= BULK_QUEUE_CAPACITY.get() {
            let done = pending.next().await;
            absorb(finished(
                done.expect("it waits only while jobs are outstanding"),
            )?);
        }
        let job = census_job(inputs.clone(), chunk, abandoned.token().clone());
        pending.push(pool.submit(Priority::Bulk, token.clone(), job).await?);
    }
    while let Some(done) = pending.next().await {
        absorb(finished(done)?);
    }
    let n_max = inputs.query.n_max();
    bulk(pool, token, move |_: &CancelToken| {
        merge_census(tallies.map(|tallies| (stars, tallies)), n_max)
    })
    .await
}

/// The part a finished census job returned, or why it has none.
fn finished(done: Result<Result<Option<Part>, JobError>, RecvError>) -> Result<Part, ComputeError> {
    let part = done.unwrap_or_else(|closed| Err(JobError::from(closed)))?;
    Ok(part.ok_or(JobError::Cancelled)?)
}

/// What every job of one sky reads, each shared: the galaxy and its key, the tables, the cell
/// cache and the query.
#[derive(Debug, Clone)]
pub(crate) struct CensusInputs {
    /// The galaxy censused.
    pub(crate) galaxy: Arc<Galaxy>,
    /// The key of `galaxy`, under which its cells are cached.
    pub(crate) key: GalaxyKey,
    /// The tables the census and the band read.
    pub(crate) tables: Arc<SkyTables>,
    /// The cells' bright subsets every request shares (Design note 12).
    pub(crate) cells: Arc<SharedSkyCellCache>,
    /// The query censused.
    pub(crate) query: Arc<SkyQuery>,
}

/// One census job: `chunk`'s cells, in order, each with [`census_cell`], in a context of its own
/// over the shared cell cache. `None` if the job stopped early because its request's token was
/// cancelled or its census `abandoned` it.
fn census_job(
    inputs: CensusInputs,
    chunk: Vec<CellKey>,
    abandoned: CancelToken,
) -> impl FnOnce(&CancelToken) -> Option<Part> + Send + 'static {
    move |token: &CancelToken| {
        let CensusInputs {
            galaxy,
            key,
            tables,
            cells,
            query,
        } = inputs;
        let handle = cells.handle(key);
        let mut ctx = tables.context(CENSUS_NOISE_SLOTS, &handle);
        let mut stars = Vec::new();
        let mut tallies = None;
        for cell in chunk {
            if token.is_cancelled() || abandoned.is_cancelled() {
                return None;
            }
            let cell = census_cell(&galaxy, &mut ctx, cell, &query, &mut stars);
            add_tallies(&mut tallies, &cell);
        }
        Some((stars, tallies))
    }
}

/// The band's jobs over `spec`'s texels, in the band's order: each face of [`CubeFace::ALL`] in
/// turn, [`BAND_JOB_ROWS`] of its rows at a time from the top.
fn band_jobs(spec: BandSpec) -> impl Iterator<Item = (CubeFace, Range<u16>)> {
    let side = spec.face_texels();
    CubeFace::ALL.into_iter().flat_map(move |face| {
        (0..side)
            .step_by(usize::from(BAND_JOB_ROWS))
            .map(move |first| (face, first..first.saturating_add(BAND_JOB_ROWS).min(side)))
    })
}

/// The band's rays marched for a reply complete to `complete_to` (R06.T9.f's `march_rows`; Design
/// notes 14 and 15), as bulk jobs of [`BAND_JOB_ROWS`] rows of a face under `token`, in the band's
/// order: one march per job, at the query's band ([`SkyQuery::band_spec`]).
///
/// It reads no census, so it runs while the census does. Each job marches in a context of its own
/// over the inputs' tables. Where a camera's deeper cut sets the query's, each march also keeps
/// the light fainter than the eye's cut, the eye's background (R06.T9.j).
///
/// # Errors
///
/// Those of [`BulkJobs`].
pub(crate) async fn march(
    pool: &CpuPool,
    inputs: &CensusInputs,
    complete_to: CompleteTo,
    token: &CancelToken,
) -> Result<Vec<BandMarch>, ComputeError> {
    let spec = inputs.query.band_spec();
    let jobs = band_jobs(spec).map(|(face, rows)| {
        let (galaxy, tables, query, complete_to) = (
            Arc::clone(&inputs.galaxy),
            Arc::clone(&inputs.tables),
            Arc::clone(&inputs.query),
            complete_to.clone(),
        );
        move || {
            let mut ctx = tables.march_context();
            march_rows(&galaxy, &mut ctx, &query, [complete_to], &spec, face, rows)
        }
    });
    BulkJobs::submit(pool, token, jobs).await?.results().await
}

/// A sky's band as the reply carries it: every texel of the six faces, and each listed star's eye
/// offset, its own eye limit less its texel's, where the eye was asked. Made by [`band`] alone.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SkyBand {
    texels: Vec<BandTexel>,
    eye_offsets: Option<Vec<Magnitudes>>,
}

impl SkyBand {
    /// The texels, in the cube's face order ([`CubeFace::ALL`]), each face's rows from the top and
    /// each row from its left, with their eye limits where the eye was asked.
    #[must_use]
    pub(crate) fn texels(&self) -> &[BandTexel] {
        &self.texels
    }

    /// Each listed star's eye offset, mag (R06.T9.h), one per listed star of the census the band
    /// was made with, in its order: `None` where the eye was not asked.
    #[must_use]
    pub(crate) fn eye_offsets(&self) -> Option<&[Magnitudes]> {
        self.eye_offsets.as_deref()
    }
}

/// The band of a reply complete to `complete_to` from its `marches` ([`march`]'s) and its
/// `census`, and where the query asks the eye, the limit map and the eye offsets, as bulk jobs
/// under `token` (Design notes 4 and 15; R06.T9.c, T9.h–T9.j).
///
/// Where the eye is asked, one job first resolves the listed stars' glare to the eye's cut
/// (`Glare::of_listed`: only the stars at or brighter than it glare, R06.T9.j). Then each march's
/// job sums its texels with the census's overflow (`sum_rows`) and sets their eye limits against
/// the light fainter than the eye's cut and the glare (`limit_rows`), which reads the glare's
/// pyramid in a fixed order, so any split of rows gives the same bits (R06.T9.i). Once every
/// texel's limit is set, one job gives each listed star its eye offset (`eye_offsets`). The eye's
/// map is so the eye-only request's at the census's radii, whatever the request's cut (decided
/// 2026-10-07, `decision-r06-t9c-glare.md`, addendum 2).
///
/// # Errors
///
/// Those of [`bulk`] and [`BulkJobs`].
pub(crate) async fn band(
    pool: &CpuPool,
    query: &Arc<SkyQuery>,
    marches: Vec<BandMarch>,
    census: &Arc<SkyCensus>,
    complete_to: CompleteTo,
    token: &CancelToken,
) -> Result<SkyBand, ComputeError> {
    let spec = query.band_spec();
    let eye = match (query.eye(), query.eye_cut()) {
        (Some(&eye), Some(eye_cut)) => {
            let (observer, listed) = (*query.observer(), Arc::clone(census));
            let glare = bulk(pool, token, move |_: &CancelToken| {
                Glare::of_listed(&observer, listed.listed(), &spec, eye_cut)
            })
            .await?;
            Some((eye, Arc::new(glare)))
        }
        _ => None,
    };
    let jobs = marches.into_iter().map(|march| {
        let (census, eye, complete_to) = (Arc::clone(census), eye.clone(), complete_to.clone());
        move || {
            let mut texels =
                Vec::with_capacity(march.rows().len() * usize::from(spec.face_texels()));
            sum_rows(&march, &census, &complete_to, &mut texels);
            if let Some((eye, glare)) = &eye {
                limits::limit_rows(eye, &spec, glare, march.face(), march.rows(), &mut texels);
            }
            texels
        }
    });
    let rows = BulkJobs::submit(pool, token, jobs).await?.results().await?;
    let side = usize::from(spec.face_texels());
    let mut texels = Vec::with_capacity(CubeFace::ALL.len() * side * side);
    for row in rows {
        texels.extend(row);
    }
    match eye {
        Some((eye, glare)) => {
            bulk(pool, token, move |_: &CancelToken| {
                let eye_offsets = limits::eye_offsets(&eye, &spec, &glare, &texels);
                SkyBand {
                    texels,
                    eye_offsets: Some(eye_offsets),
                }
            })
            .await
        }
        None => Ok(SkyBand {
            texels,
            eye_offsets: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::sky::caps::layer_caps;
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use tokio::time::timeout;

    use super::*;
    use crate::limits::INTERACTIVE_QUEUE_CAPACITY;
    use crate::testing::WAIT;

    fn sun() -> Observer {
        let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).unwrap();
        Observer::new(at, UniverseTime::EPOCH).unwrap()
    }

    fn pool(workers: usize) -> CpuPool {
        CpuPool::new(
            NonZeroUsize::new(workers).unwrap(),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
        )
        .unwrap()
    }

    /// The census its jobs merge is the sim's one pass over the same plan with no cell cache, star
    /// for star and tally for tally, cold and warm: the split into jobs, their order, the joined
    /// parts and the cache change nothing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_jobs_census_is_the_one_pass_census_star_for_star() {
        let galaxy = Arc::new(Galaxy::new(Seed::new(0x4d2)));
        let caps = SkyCaps::forced(LightYears::new(30.0)).unwrap();
        let tables = Arc::new(SkyTables::new(LuminosityTables::dark(&galaxy), &galaxy));
        let query = SkyQuery::builder(sun(), Magnitudes::new(9.0))
            .build()
            .unwrap()
            .with_caps_forced(caps.forced_radius().unwrap())
            .unwrap();
        let query = Arc::new(query);
        let pool = pool(3);
        let token = CancelToken::new();
        let plan = timeout(WAIT, plan(&pool, &galaxy, &tables, &query, &token))
            .await
            .expect("timed out planning")
            .unwrap();
        let inputs = CensusInputs {
            galaxy: Arc::clone(&galaxy),
            key: GalaxyKey::new(0x4d2, GENERATOR_VERSION),
            tables: Arc::clone(&tables),
            cells: Arc::new(SharedSkyCellCache::new(256 << 20)),
            query: Arc::clone(&query),
        };
        let cold = timeout(WAIT, census(&pool, &inputs, &plan, &token))
            .await
            .expect("timed out censusing")
            .unwrap();
        let after_cold = inputs.cells.counters();
        let warm = timeout(WAIT, census(&pool, &inputs, &plan, &token))
            .await
            .expect("timed out censusing again")
            .unwrap();
        let after_warm = inputs.cells.counters();
        assert!(
            usize::try_from(plan.cell_count()).unwrap() > 2 * CENSUS_JOB_CELLS,
            "the census is split into several jobs"
        );
        // Every cell the cold census looked up, the warm one found built at its own floor.
        let lookups = after_cold.cache().hits() + after_cold.cache().misses();
        assert!(lookups > 0);
        assert_eq!(
            (
                after_warm.cache().hits() - after_cold.cache().hits(),
                after_warm.cache().misses() - after_cold.cache().misses(),
                after_warm.rebuilt()
            ),
            (lookups, 0, 0),
            "the warm census is served from the cache: {after_cold:?}, then {after_warm:?}"
        );

        let mut ctx = tables.context(CENSUS_NOISE_SLOTS, &NoSkyCellCache);
        let mut stars = Vec::new();
        let mut tallies = None;
        for key in plan.cells() {
            add_tallies(
                &mut tallies,
                &census_cell(&galaxy, &mut ctx, key, &query, &mut stars),
            );
        }
        let one_pass = merge_census(tallies.map(|tallies| (stars, tallies)), query.n_max());
        assert!(!one_pass.listed().is_empty());
        assert_eq!(cold, one_pass);
        assert_eq!(warm, one_pass);
        pool.shutdown().await.unwrap();
    }

    /// The derived caps, their rays measured in jobs and counted in ray order, are `layer_caps`'
    /// bit for bit, and so is the plan they make (decided 2026-10-03, `decision-r06-tables.md`,
    /// item B.4), with the eye's visibility as without (R06.T7.b). Over tables that hold no star
    /// every count is nought, so each cap is the count's nearest radius, but each rule bound reads
    /// every ray's extinction.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_caps_rays_measured_in_jobs_are_the_sims_caps() {
        let galaxy = Arc::new(Galaxy::new(Seed::new(0x4d2)));
        let tables = Arc::new(SkyTables::new(LuminosityTables::dark(&galaxy), &galaxy));
        let query = Arc::new(
            SkyQuery::builder(sun(), Magnitudes::new(8.0))
                .build()
                .unwrap(),
        );
        let pool = pool(3);
        let token = CancelToken::new();
        let planned = timeout(WAIT, plan(&pool, &galaxy, &tables, &query, &token))
            .await
            .expect("timed out planning")
            .unwrap();
        let mut noise = NoiseCache::with_capacity(MARCH_NOISE_SLOTS);
        let caps = layer_caps(
            &galaxy,
            &tables.tables,
            &tables.envelope,
            query.observer(),
            query.cut(),
            &mut noise,
        );
        assert_eq!(planned.caps(), caps.as_slice());
        assert!(
            caps.iter().any(|cap| cap.rule_bound().value() > 1.0),
            "a rule bound reads the rays: {caps:?}"
        );
        assert_eq!(planned, census_plan_of(&query, caps));
        // An eye-only request capped by the eye's visibility (R06.T7.b) takes the sim's caps by
        // it, bit for bit.
        let eye = EyeObserver::default();
        let mut ctx = tables.march_context();
        let cut = limits::eye_cut(&galaxy, &mut ctx, &sun(), &eye, None);
        let visibility = limits::eye_visibility(&galaxy, &mut ctx, &sun(), &eye, cut, None);
        let seen = Arc::new(
            SkyQuery::builder(sun(), cut)
                .eye(eye)
                .eye_visibility(visibility)
                .build()
                .unwrap(),
        );
        let planned = timeout(WAIT, plan(&pool, &galaxy, &tables, &seen, &token))
            .await
            .expect("timed out planning")
            .unwrap();
        let sims = census_plan(&galaxy, &tables.tables, &tables.envelope, &seen, &mut noise);
        assert_eq!(planned, sims);
        pool.shutdown().await.unwrap();
    }

    /// The band's jobs cover each face's rows once, in the band's order, two rows a job.
    #[test]
    fn the_bands_jobs_cover_every_row_of_every_face_in_order() {
        let jobs: Vec<_> = band_jobs(BandSpec::STANDARD).collect();
        assert_eq!(jobs.len(), 6 * 32);
        let mut expected = Vec::new();
        for face in CubeFace::ALL {
            for first in (0..64).step_by(2) {
                expected.push((face, first..first + 2));
            }
        }
        assert_eq!(jobs, expected);
        let odd = BandSpec::new(5, 12).unwrap();
        let rows: Vec<_> = band_jobs(odd)
            .filter(|(face, _)| *face == CubeFace::PosX)
            .map(|(_, rows)| rows)
            .collect();
        assert_eq!(rows, [0..2, 2..4, 4..5]);
    }

    /// Jobs queued together come back in their order, and a request's cancelled token skips them.
    #[tokio::test]
    async fn bulk_jobs_come_back_in_order() {
        let pool = pool(2);
        let token = CancelToken::new();
        let jobs = (0..40_u32).map(|n| move || n * n);
        let squares = timeout(WAIT, async {
            BulkJobs::submit(&pool, &token, jobs).await?.results().await
        })
        .await
        .expect("timed out on the jobs")
        .unwrap();
        assert_eq!(squares, (0..40).map(|n| n * n).collect::<Vec<_>>());
        let cancelled = CancelToken::new();
        cancelled.cancel();
        let refused = timeout(WAIT, async {
            BulkJobs::submit(&pool, &cancelled, [|| 1_u8])
                .await?
                .results()
                .await
        })
        .await
        .expect("timed out on the cancelled job");
        assert_eq!(refused, Err(ComputeError::Job(JobError::Cancelled)));
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// Jobs given up before they begin, their `BulkJobs` dropped, return at once without running.
    #[tokio::test]
    async fn dropped_bulk_jobs_that_have_not_begun_do_not_run() {
        let pool = pool(1);
        let token = CancelToken::new();
        // The one worker waits on this job until it is released, so the jobs below stay queued.
        let (release, gate) = std::sync::mpsc::channel::<()>();
        let held = pool
            .submit(Priority::Bulk, token.clone(), move |_: &CancelToken| {
                gate.recv().expect("the gate is released");
            })
            .await
            .unwrap();
        let ran = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let jobs = (0..8).map(|_| {
            let ran = Arc::clone(&ran);
            move || ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        });
        let queued = BulkJobs::submit(&pool, &token, jobs).await.unwrap();
        drop(queued);
        release.send(()).unwrap();
        // The one worker takes its jobs in order, so this one runs after all of them.
        timeout(WAIT, async {
            received(held).await?;
            bulk(&pool, &token, |_: &CancelToken| ()).await
        })
        .await
        .expect("timed out draining the pool")
        .unwrap();
        assert_eq!(ran.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(pool.counters().completed(), 10, "{:?}", pool.counters());
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    #[test]
    fn a_forced_cap_is_held_to_the_root_cubes_diagonal() {
        assert_eq!(SkyCaps::default(), SkyCaps::DERIVED);
        assert_eq!(SkyCaps::DERIVED.forced_radius(), None);
        assert_eq!(SkyCaps::DERIVED.tables(), SkyTablesSource::Galaxy);
        let forced = SkyCaps::forced(LightYears::new(40.0)).unwrap();
        assert_eq!(
            forced.forced_radius().map(LightYears::value),
            Some(40.0),
            "the radius is kept"
        );
        assert_eq!(forced.tables(), SkyTablesSource::Dark);
        let lit = forced.with_galaxy_tables();
        assert_eq!(
            (lit.forced_radius(), lit.tables()),
            (forced.forced_radius(), SkyTablesSource::Galaxy)
        );
        assert!(SkyCaps::forced(LightYears::ZERO).is_ok());
        assert!(SkyCaps::forced(LightYears::new(MAX_FORCED_CAP_LY)).is_ok());
        for bad in [-1.0, f64::NAN, f64::INFINITY, MAX_FORCED_CAP_LY + 1.0] {
            assert_eq!(
                SkyCaps::forced(LightYears::new(bad)),
                Err(ForceSkyCapsError),
                "{bad}"
            );
        }
    }
}
