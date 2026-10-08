//! The sky's work on the CPU pool (rendering plan R06, R06.T11.a–d; Design notes 5, 9–11 and
//! 14–16): the illumination, the eye's cut and visibility, the caps, the census shell by shell,
//! the band, the limit map and the eye offsets, as bulk jobs.
//!
//! Everything a `sky` request computes runs at [`Priority::Bulk`], never in the interactive queue,
//! so a chart's query is never queued behind it: the workers take interactive work first. The work
//! is split into jobs of a fraction of a second where the sim allows it, so that a query waits for
//! little on a busy worker. A few steps are single jobs of seconds, not yet split: the tables' plan
//! and assembly, the eye's cut and visibility, and each reply's merge, glare, eye offsets and
//! payload. In order:
//!
//! - the galaxy's tables are built once and kept ([`SkyTablesService`](super::SkyTablesService));
//! - the request's illumination, the observer's sky of all starlight that the dust scatters
//!   (R06.T9.g), is marched [`ILLUMINATION_JOB_ROWS`] rows of its 16² faces to a job, then
//!   assembled in one more;
//! - the eye's cut is one job, two coarse pre-passes of the band with that illumination, and an
//!   eye-only request's visibility (R06.T7.b) one more;
//! - the caps' 1,536 rays, each through its three sub-rays (R06.T7.b), are measured
//!   [`CAP_JOB_RAYS`] to a job, each with its own noise cache,
//!   then counted in ray order in one more job, which makes the census's plan (decided
//!   2026-10-03, `decision-r06-tables.md`, item B.4);
//! - the census runs its plan's shells nearest first, step by step ([`delivery_steps`]; R06.T11.d),
//!   each shell's slabs as jobs of about [`CENSUS_JOB_SLICE`] ([`census_in_steps`]);
//! - while it runs, the band's rays are marched [`BAND_JOB_ROWS`] rows of a face to a job (R06.T9.f's
//!   `march_rows`), once a request for every reply its shells can state, which reads no census;
//! - after each step, its census is merged ([`merge_step`]), and each march is summed with that
//!   census's overflow at its radii (`sum_rows`) and given the eye's limits against the listed
//!   stars' glare (`limit_rows`), a job each, and every listed star its eye offset in one more job
//!   (`eye_offsets`) ([`band`]).
//!
//! Every job runs under the request's [`CancelToken`]: a cancelled request's queued jobs are
//! skipped, and a running census job stops at its next cell. Work that ends early, because a job
//! failed or the request's future was dropped, abandons its other jobs too: each one not yet begun
//! returns at once. Each split is merged in a fixed order, so it cannot change the answer: the
//! census's merge is total, the rays and the band's rows are put back in order, and every texel and
//! ray is a function of its own direction. Neither a census's sources nor a noise cache can be
//! shared between threads, so each job builds its own [`SkyContext`] over the [`SkyTables`] they
//! all read, and the census's jobs over the server's [`SharedSkyCellCache`], which every job and
//! request shares (R06.T11.b; Design note 12): a census cancelled or superseded keeps the cells it
//! built there, so that a moving ship's outer shells still converge through the cache.
//!
//! [`SkyCaps`] says how far the census looks: each layer's derived cap (Design note 9), or radii
//! forced on the layers, which keep a test's census small, and, for a test alone, nearer shell
//! edges than the fixed ones.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::iter::Peekable;
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::CellKey;
use hyperion_sim::id::Layer;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::band::{
    BandMarch, BandSpec, BandTexel, CompleteTo, CubeFace, march_rows, sum_rows,
};
use hyperion_sim::sky::caps::{
    CAP_RAYS, CAPPED_LAYERS, CapLattice, RayExtinctions, SUB_RAYS, layer_caps_by_visibility_over,
    layer_caps_over,
};
use hyperion_sim::sky::census::{
    CellOffsets, CellSlab, CensusPlan, CensusTallies, MAX_FORCED_CAP_LY, NoSkyCellCache,
    SHELL_EDGES_LY, Shell, SkyCellCache, SkyCensus, SkyContext, SkyQuery, SkyStar, census_cell,
    census_plan_with_edges, merge_shells,
};
use hyperion_sim::sky::dgl::{ILLUMINATION_SPEC, Illumination};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::limits::{self, EyeVisibility, Glare};
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::units::{LightYears, Magnitudes};
use tokio::sync::mpsc;
use tokio::sync::oneshot::error::RecvError;

use super::{
    CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyKey, JobError, JobReceiver, Priority,
    SharedSkyCellCache,
};
use crate::limits::BULK_QUEUE_CAPACITY;

/// How long one census job runs before it hands the rest of its cells back, about 50 ms of a
/// worker (R06.T11.d; `decision-r06-census-cost-signoff.md`, question 2).
///
/// The pool never preempts a running job, so this bounds how long a chart's query waits behind
/// the census on a worker: a job checks the clock before each cell and stops once its slice is
/// spent, and the rest of its cells go back in the census's queue as another job. A cell's cost is
/// unknown until its records' bounds are read (most are skipped in microseconds, a few generated
/// for milliseconds), so a job is sized by the time it has run, not by an estimate of its cells.
/// Which cells a job holds never changes the census, whose merge is total.
pub(crate) const CENSUS_JOB_SLICE: Duration = Duration::from_millis(50);

/// The census jobs one sky keeps outstanding per worker: two, so that every worker has the next
/// in hand, while the work of the next reply and of other requests' bulk jobs waits behind few.
pub(crate) const CENSUS_JOBS_PER_WORKER: usize = 2;

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

/// The rows of a face of the illumination's 16² band one job marches: two, 32 rays, 48 jobs
/// (R06.T9.g).
pub(crate) const ILLUMINATION_JOB_ROWS: u16 = 2;

/// Noise-cache slots of a census job: 256 KiB, a cache's worth for the sightlines of a few hundred
/// cells' stars. The cache changes no value, only how often a normal is drawn again.
const CENSUS_NOISE_SLOTS: usize = 1 << 14;

/// Noise-cache slots of the jobs that march many rays (the eye's pre-pass, the illumination, the
/// caps and the band): 1 MiB.
const MARCH_NOISE_SLOTS: usize = 1 << 16;

/// How far a sky's census looks in each layer (Design note 9), and which luminosity tables it reads:
/// the caps the galaxy's tables and dust derive, or radii forced on the layers.
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
    /// Each layer's forced radius, in [`CAPPED_LAYERS`]' order.
    forced: Option<[LightYears; CAPPED_LAYERS.len()]>,
    tables: SkyTablesSource,
    /// A test's shell edges, ly, in place of [`SHELL_EDGES_LY`].
    shell_edges_ly: Option<&'static [u32]>,
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
        shell_edges_ly: None,
    };

    /// Every layer's cap forced to `radius`, over tables that hold no star.
    ///
    /// # Errors
    ///
    /// [`ForceSkyCapsError`] unless `radius` is finite, non-negative and within the root cube's
    /// diagonal ([`MAX_FORCED_CAP_LY`]).
    pub fn forced(radius: LightYears) -> Result<Self, ForceSkyCapsError> {
        Self::forced_per_layer([radius; CAPPED_LAYERS.len()])
    }

    /// Each layer's cap forced to its radius in `radii`, in [`CAPPED_LAYERS`]' order (A, B, C, D,
    /// E and the brown dwarfs), over tables that hold no star: a census of several shells in a
    /// layer that a test can afford, the others kept small (R06.T11.d).
    ///
    /// # Errors
    ///
    /// [`ForceSkyCapsError`] unless every radius is finite, non-negative and within the root cube's
    /// diagonal ([`MAX_FORCED_CAP_LY`]).
    pub(crate) fn forced_per_layer(
        radii: [LightYears; CAPPED_LAYERS.len()],
    ) -> Result<Self, ForceSkyCapsError> {
        if radii
            .iter()
            .all(|radius| (0.0..=MAX_FORCED_CAP_LY).contains(&radius.value()))
        {
            Ok(Self {
                forced: Some(radii),
                tables: SkyTablesSource::Dark,
                shell_edges_ly: None,
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

    /// The same caps with the census's shells to `edges_ly`, ascending, in place of
    /// [`SHELL_EDGES_LY`]: for a test, whose census of a few tens of light-years then still
    /// arrives in several replies (R06.T11.d). Layers C, D and E take a shell for each edge below
    /// their cap, as they do the fixed edges'. No sky a client asks is censused so: the fixed edges
    /// make every machine send the same replies.
    ///
    /// The sim's plan refuses edges that are not above nought or do not ascend
    /// (`census_plan_with_edges`): such a sky's plan job panics, answered `internal`.
    #[must_use]
    pub const fn with_shell_edges(self, edges_ly: &'static [u32]) -> Self {
        Self {
            shell_edges_ly: Some(edges_ly),
            ..self
        }
    }

    /// The census's shell edges, ly: [`SHELL_EDGES_LY`] unless a test's
    /// ([`with_shell_edges`](Self::with_shell_edges)).
    #[must_use]
    pub fn shell_edges_ly(&self) -> &'static [u32] {
        self.shell_edges_ly.unwrap_or(&SHELL_EDGES_LY)
    }

    /// The radius each layer's cap is forced to, in [`CAPPED_LAYERS`]' order, or `None` for the
    /// derived caps.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn forced_radii(&self) -> Option<[LightYears; CAPPED_LAYERS.len()]> {
        self.forced
    }

    /// The luminosity tables the sky reads.
    #[must_use]
    pub const fn tables(&self) -> SkyTablesSource {
        self.tables
    }

    /// `query` with these caps: its caps forced where these force them, and as it is otherwise.
    ///
    /// # Panics
    ///
    /// Never: a forced radius is checked when the caps are made, as the query checks it.
    #[must_use]
    pub(crate) fn on(&self, query: SkyQuery) -> SkyQuery {
        match self.forced {
            Some(radii) => {
                let per_layer: Vec<(Layer, LightYears)> =
                    CAPPED_LAYERS.iter().copied().zip(radii).collect();
                query
                    .with_caps_forced_per_layer(&per_layer)
                    .expect("a forced sky cap is checked when it is made")
            }
            None => query,
        }
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
    /// entries from `cells`, no sources beyond the grid and no gas modifiers (plan 09's
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

    /// A context for a job that marches many rays: the eye's pre-pass, the illumination and the
    /// band.
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

/// The request's illumination at `observer` (Design note 15; R06.T9.g): the observer's own sky of
/// all starlight, marched [`ILLUMINATION_JOB_ROWS`] rows of a face of its 16² band to a bulk job
/// under `token`, then its scattered field iterated in one more, which gives
/// [`Illumination::march`]'s bits.
///
/// It depends on the observer and time alone, so every reply of the request, the eye's cut and the
/// eye's visibility state the same one (`decision-r06-t9g-dgl.md`, §3.4).
///
/// # Errors
///
/// Those of [`bulk`] and [`BulkJobs`].
pub(crate) async fn illumination(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    tables: &Arc<SkyTables>,
    observer: Observer,
    token: &CancelToken,
) -> Result<Arc<Illumination>, ComputeError> {
    let (galaxy, tables) = (Arc::clone(galaxy), Arc::clone(tables));
    let jobs = row_jobs(ILLUMINATION_SPEC, ILLUMINATION_JOB_ROWS).map(move |(face, rows)| {
        let (galaxy, tables) = (Arc::clone(&galaxy), Arc::clone(&tables));
        move || {
            Illumination::march_rows(&galaxy, &mut tables.march_context(), &observer, face, rows)
        }
    });
    let parts = BulkJobs::submit(pool, token, jobs).await?.results().await?;
    bulk(pool, token, move |_: &CancelToken| {
        Arc::new(Illumination::assemble(&observer, parts))
    })
    .await
}

/// The eye's cut for `eye` at `observer` (Design note 5; R06.T9.d), in one bulk job under `token`:
/// two coarse pre-passes of the band, 1,536 rays each, with the request's `illumination`
/// (R06.T9.g), near the Sun.
///
/// # Errors
///
/// Those of [`bulk`].
pub(crate) async fn eye_cut(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    tables: &Arc<SkyTables>,
    eye: (Observer, EyeObserver),
    illumination: &Arc<Illumination>,
    token: &CancelToken,
) -> Result<Magnitudes, ComputeError> {
    let (galaxy, tables, light) = (
        Arc::clone(galaxy),
        Arc::clone(tables),
        Arc::clone(illumination),
    );
    let (observer, eye) = eye;
    bulk(pool, token, move |_: &CancelToken| {
        limits::eye_cut(
            &galaxy,
            &mut tables.march_context(),
            &observer,
            &eye,
            Some(&light),
        )
    })
    .await
}

/// The eye's visibility for `eye` at `observer` and its cut `cut` (R06.T7.b), with the request's
/// `illumination`, in one bulk job under `token`: the eye cut's 16² pre-pass at the cut, whose
/// limits an eye-only request's caps count each ray to.
///
/// Only a request with no camera part takes it (decided 2026-10-08, `decision-r06-t7b-brackets.md`):
/// a camera's cut, even a shallower one, is culled by the camera's own limit, which the eye's
/// visibility caps would not cover.
///
/// # Errors
///
/// Those of [`bulk`].
pub(crate) async fn eye_visibility(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    tables: &Arc<SkyTables>,
    eye: (Observer, EyeObserver),
    cut: Magnitudes,
    illumination: &Arc<Illumination>,
    token: &CancelToken,
) -> Result<EyeVisibility, ComputeError> {
    let (galaxy, tables, light) = (
        Arc::clone(galaxy),
        Arc::clone(tables),
        Arc::clone(illumination),
    );
    let (observer, eye) = eye;
    bulk(pool, token, move |_: &CancelToken| {
        limits::eye_visibility(
            &galaxy,
            &mut tables.march_context(),
            &observer,
            &eye,
            cut,
            Some(&light),
        )
    })
    .await
}

/// The census's plan for `query` (Design notes 9 and 10): each layer's cap and the cells they open,
/// in shells to `edges_ly` ([`SkyCaps::shell_edges_ly`]: [`SHELL_EDGES_LY`] for every sky a client
/// asks, R06.T8.i).
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
    edges_ly: &'static [u32],
    token: &CancelToken,
) -> Result<CensusPlan, ComputeError> {
    let (galaxy, tables, query) = (Arc::clone(galaxy), Arc::clone(tables), Arc::clone(query));
    if let Some(forced) = query.forced_caps() {
        // A forced query's plan reads no table and marches no ray: `census_plan`'s for it.
        let caps = forced.to_vec();
        return bulk(pool, token, move |_: &CancelToken| {
            census_plan_with_edges(&query, caps, edges_ly)
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
        census_plan_with_edges(&query, caps, edges_ly)
    })
    .await
}

/// Adds `tallies` to `sum`, starting the sum from the first, as the census's merge starts from its
/// first part's, so that a flag every cell clears stays clear.
fn add_tallies(sum: &mut Option<CensusTallies>, tallies: &CensusTallies) {
    match sum {
        Some(sum) => sum.add(tallies),
        None => *sum = Some(*tallies),
    }
}

/// The order a sky's shells are censused in, step by step, a reply after each (R06.T11.d; decided
/// 2026-10-05, `decision-r06-census-cost.md`, with the sign-off's condition 3,
/// `decision-r06-census-cost-signoff.md`).
///
/// Step k holds each layer's k-th shell, nearest first within each layer, as
/// [`CensusPlan::shells`] ranks them, except that layer C's shells beyond 2,000 ly (its fourth on)
/// come one step later: D's and E's shells to 4,000 ly then run before C's beyond 2,000 ly, whose
/// cells are mostly walk and hold few of the brightest distant stars. Within a step the shells
/// keep [`CensusPlan::shells`]' order. The first step holds every layer's first shell, so A, B and
/// the brown dwarfs, one shell each, are final from the first reply, and every layer states a
/// radius in it. The order is a function of the plan alone: every machine and worker count
/// censuses the same steps and sends the same replies, only at its own pace. A plan of no shell,
/// every cap of no radius, is one step of none, its one reply final.
#[must_use]
pub(crate) fn delivery_steps(plan: &CensusPlan) -> Vec<Vec<Shell>> {
    /// C's shells from this place on lie beyond 2,000 ly: the fourth, 2,000 to 4,000 ly.
    const C_DEFERRED_FROM: u8 = 3;
    let mut steps: Vec<Vec<Shell>> = Vec::new();
    for shell in plan.shells() {
        let deferred = shell.layer() == Layer::C && shell.index() >= C_DEFERRED_FROM;
        let step = usize::from(shell.index()) + usize::from(deferred);
        if steps.len() <= step {
            steps.resize_with(step + 1, Vec::new);
        }
        steps[step].push(shell);
    }
    steps.retain(|step| !step.is_empty());
    if steps.is_empty() {
        steps.push(Vec::new());
    }
    steps
}

/// One census job's share of a shell: cells of one of its slabs not yet censused, in order, the
/// slab still to walk or some of its cells walked already.
enum CellWork {
    /// A slab's cells, walked as they are taken.
    Slab(Peekable<Box<dyn Iterator<Item = CellKey> + Send>>),
    /// Cells of a slab walked already, in order.
    Cells(std::vec::IntoIter<CellKey>),
}

impl CellWork {
    /// The work of `slab`'s cells, not yet walked.
    fn of(slab: &CellSlab) -> Self {
        let cells: Box<dyn Iterator<Item = CellKey> + Send> = Box::new(slab.cells());
        Self::Slab(cells.peekable())
    }

    /// The next cell, walking the slab as far as it.
    fn next(&mut self) -> Option<CellKey> {
        match self {
            Self::Slab(cells) => cells.next(),
            Self::Cells(cells) => cells.next(),
        }
    }

    /// Whether no cell is left, walking the slab as far as the next.
    fn is_done(&mut self) -> bool {
        match self {
            Self::Slab(cells) => cells.peek().is_none(),
            Self::Cells(cells) => cells.len() == 0,
        }
    }

    /// The cells left, walked to the slab's end and split in two halves, so that another worker
    /// may take one: a slab whose cells each cost more than a job's slice, as the few cells nearest
    /// the observer in layers D and E do, then spreads over the workers in a few rounds.
    fn split(self) -> impl Iterator<Item = Self> {
        let mut cells: Vec<CellKey> = match self {
            Self::Slab(cells) => cells.collect(),
            Self::Cells(cells) => cells.collect(),
        };
        let back = cells.split_off(cells.len() / 2);
        [cells, back]
            .into_iter()
            .filter(|half| !half.is_empty())
            .map(|half| Self::Cells(half.into_iter()))
    }
}

/// The stars and tallies of one step's census jobs, joined as they arrived.
#[derive(Debug, Default)]
pub(crate) struct StepPart {
    stars: Vec<SkyStar>,
    tallies: Option<CensusTallies>,
}

impl StepPart {
    /// Adds a job's stars and tallies, starting the tallies from the first.
    fn absorb(&mut self, stars: Vec<SkyStar>, tallies: Option<&CensusTallies>) {
        self.stars.extend(stars);
        if let Some(tallies) = tallies {
            add_tallies(&mut self.tallies, tallies);
        }
    }
}

/// A census's progress after one of its steps ([`delivery_steps`]): every shell censused so far,
/// and the stars and tallies of each step to this one, from which [`merge_step`] makes the reply's
/// census.
#[derive(Debug, Clone)]
pub(crate) struct CensusStep {
    /// The shells of every step to this one.
    pub(crate) done: Vec<Shell>,
    /// Each step's part, from the first.
    pub(crate) parts: Vec<Arc<StepPart>>,
    /// Whether it is the last step: the census is then the one-shot census of its plan.
    pub(crate) last: bool,
    /// The time its census jobs ran, summed over them: the step's census work, for the server's
    /// log (T17 times each reply).
    pub(crate) worked: Duration,
}

/// What one census job did: its cells' stars and their tallies (`None` for no cell), and the cells
/// it had no time for.
struct Slice {
    stars: Vec<SkyStar>,
    tallies: Option<CensusTallies>,
    rest: Vec<CellWork>,
    /// How long the job ran.
    worked: Duration,
}

/// The census of the inputs' query over `plan`'s shells, nearest first in `steps` (made by
/// [`delivery_steps`]; R06.T11.d), as bulk jobs of about `slice` ([`CENSUS_JOB_SLICE`]) under
/// `token`, its progress sent to `sink` after each step, in order (Design notes 10 and 11).
///
/// Each shell's slabs are queued in its step's order, and a job takes one slab's cells, or a part
/// of them, censuses them in order until its slice is spent, and hands back the cells it had no
/// time for, walked and split in two halves, which go back in the queue behind the step's other
/// work and ahead of every later step's: a slab of dear cells so spreads over the workers in a few
/// rounds. (Jobs of several slabs, tried first, held the slabs a job had not begun until its slice
/// was spent, so a step ran on few workers at a time: R06's Risks, "Deviations in T11.d, as
/// built".) At most [`CENSUS_JOBS_PER_WORKER`] jobs a worker are outstanding, so a later step's
/// jobs never hold up an earlier step's for long, and the replies' own jobs and other requests'
/// bulk jobs wait behind few. A step is sent once its cells and every earlier step's are censused,
/// whatever later work is done already. Each job builds its own [`SkyContext`] over the inputs'
/// tables, and reads and keeps each cell's entry through their cell cache, blocks of cells keyed by
/// magnitude whose stored bound is a pre-filter only, which never changes the census and keeps
/// every cell it built if the census is cancelled or superseded (Design note 12; R06.T8.h). A job
/// stops at its next cell once `token` is cancelled, and the pool skips those still queued. A
/// census that ends early, because a job failed or this future was dropped, abandons its other
/// jobs too: each stops before its next cell. The census of the steps' shells does not depend on
/// which job held which cells, nor on their order, nor on what the cache held: the merge is total,
/// and the tallies are sums.
///
/// It returns once the last step is sent, or once `sink` has gone, the replies having been given
/// up.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the pool is shutting down or faulted, and [`ComputeError::Job`] if a
/// job was cancelled, panicked or was dropped with the pool.
///
/// # Panics
///
/// If `steps` holds a shell not of `plan`.
pub(crate) async fn census_in_steps(
    pool: &CpuPool,
    inputs: &CensusInputs,
    plan: &CensusPlan,
    steps: &[Vec<Shell>],
    slice: Duration,
    sink: mpsc::Sender<CensusStep>,
    token: &CancelToken,
) -> Result<(), ComputeError> {
    // Raised when this census ends, however it ends: its jobs still outstanding then are of no use.
    let abandoned = CancelOnDrop::new(CancelToken::new());
    let window = pool
        .workers()
        .get()
        .saturating_mul(CENSUS_JOBS_PER_WORKER)
        .min(BULK_QUEUE_CAPACITY.get());
    // The work not yet in a job, by step and then in the order queued.
    let mut queued: BTreeMap<(usize, u64), CellWork> = BTreeMap::new();
    let mut order = 0_u64;
    // Per step, its work queued and its jobs outstanding: done at nought.
    let mut open = vec![0_usize; steps.len()];
    for (step, shells) in steps.iter().enumerate() {
        for &shell in shells {
            for slab in plan.shell_slabs(shell) {
                queued.insert((step, order), CellWork::of(&slab));
                order += 1;
                open[step] += 1;
            }
        }
    }
    let mut outstanding = FuturesUnordered::new();
    let mut found: Vec<StepPart> = steps.iter().map(|_| StepPart::default()).collect();
    let mut worked = vec![Duration::ZERO; steps.len()];
    let (mut sent, mut done, mut parts) = (0, Vec::new(), Vec::new());
    loop {
        // Every step whose cells, and every earlier step's, are censused is sent, in order.
        while sent < steps.len() && open[sent] == 0 {
            done.extend_from_slice(&steps[sent]);
            parts.push(Arc::new(std::mem::take(&mut found[sent])));
            sent += 1;
            let step = CensusStep {
                done: done.clone(),
                parts: parts.clone(),
                last: sent == steps.len(),
                worked: worked[sent - 1],
            };
            if sink.send(step).await.is_err() {
                return Ok(());
            }
        }
        if sent == steps.len() {
            return Ok(());
        }
        while outstanding.len() < window {
            let Some(((step, _), work)) = queued.pop_first() else {
                break;
            };
            // The work, queued, is now a job: the step's count is unchanged.
            let job = census_job(inputs.clone(), work, abandoned.token().clone(), slice);
            let receiver = pool.submit(Priority::Bulk, token.clone(), job).await?;
            outstanding.push(async move { (step, receiver.await) });
        }
        let (step, finished) = outstanding
            .next()
            .await
            .expect("a step not yet censused has its work in jobs");
        let slice = finished
            .unwrap_or_else(|closed: RecvError| Err(JobError::from(closed)))?
            .ok_or(JobError::Cancelled)?;
        let Slice {
            stars,
            tallies,
            rest,
            worked: ran,
        } = slice;
        worked[step] += ran;
        open[step] = open[step] - 1 + rest.len();
        for cells in rest {
            queued.insert((step, order), cells);
            order += 1;
        }
        found[step].absorb(stars, tallies.as_ref());
    }
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
    /// The cells' entries every request shares (Design note 12, R06.T8.h).
    pub(crate) cells: Arc<SharedSkyCellCache>,
    /// The query censused.
    pub(crate) query: Arc<SkyQuery>,
}

/// One census job: `work`'s cells, in order, each with [`census_cell`], in a context of its own
/// over the shared cell cache, until `slice` is spent, and at least one cell. The cells it had no
/// time for come back, walked and split in two. `None` if the job stopped early because its
/// request's token was cancelled or its census `abandoned` it.
fn census_job(
    inputs: CensusInputs,
    mut work: CellWork,
    abandoned: CancelToken,
    slice: Duration,
) -> impl FnOnce(&CancelToken) -> Option<Slice> + Send + 'static {
    move |token: &CancelToken| {
        let started = Instant::now();
        let CensusInputs {
            galaxy,
            key,
            tables,
            cells,
            query,
        } = inputs;
        let handle = cells.handle(key);
        let mut ctx = tables.context(CENSUS_NOISE_SLOTS, &handle);
        let (mut stars, mut tallies) = (Vec::new(), None);
        loop {
            if token.is_cancelled() || abandoned.is_cancelled() {
                return None;
            }
            // At least one cell a job, so that every job moves the census on.
            if tallies.is_some() && started.elapsed() >= slice && !work.is_done() {
                // Out of time: the rest of these cells, split, go back.
                return Some(Slice {
                    stars,
                    tallies,
                    rest: work.split().collect(),
                    worked: started.elapsed(),
                });
            }
            let Some(cell) = work.next() else {
                break;
            };
            let cell = census_cell(&galaxy, &mut ctx, cell, &query, &mut stars);
            add_tallies(&mut tallies, &cell);
        }
        Some(Slice {
            stars,
            tallies,
            rest: Vec::new(),
            worked: started.elapsed(),
        })
    }
}

/// The census of a sky after `step` (R06.T8.i's `merge_shells`), in one bulk job under `token`:
/// every step's stars to it, merged at `n_max` and listed to the completeness of the shells done,
/// as `plan` states it. The last step's is the one-shot census of `plan`, star for star.
///
/// # Errors
///
/// Those of [`bulk`].
pub(crate) async fn merge_step(
    pool: &CpuPool,
    plan: &Arc<CensusPlan>,
    step: CensusStep,
    n_max: std::num::NonZeroU32,
    token: &CancelToken,
) -> Result<SkyCensus, ComputeError> {
    let plan = Arc::clone(plan);
    let CensusStep { done, parts, .. } = step;
    bulk(pool, token, move |_: &CancelToken| {
        let mut stars = Vec::with_capacity(parts.iter().map(|part| part.stars.len()).sum());
        let mut tallies = None;
        for part in &parts {
            stars.extend_from_slice(&part.stars);
            if let Some(part) = &part.tallies {
                add_tallies(&mut tallies, part);
            }
        }
        merge_shells(
            tallies.map(|tallies| (stars, tallies)),
            n_max,
            plan.completeness(done),
        )
    })
    .await
}

/// The jobs of `spec`'s texels, in the band's order: each face of [`CubeFace::ALL`] in turn,
/// `rows_a_job` of its rows at a time from the top.
fn row_jobs(spec: BandSpec, rows_a_job: u16) -> impl Iterator<Item = (CubeFace, Range<u16>)> {
    let side = spec.face_texels();
    CubeFace::ALL.into_iter().flat_map(move |face| {
        (0..side)
            .step_by(usize::from(rows_a_job))
            .map(move |first| (face, first..first.saturating_add(rows_a_job).min(side)))
    })
}

/// The band's jobs over `spec`'s texels, in the band's order: each face of [`CubeFace::ALL`] in
/// turn, [`BAND_JOB_ROWS`] of its rows at a time from the top.
fn band_jobs(spec: BandSpec) -> impl Iterator<Item = (CubeFace, Range<u16>)> {
    row_jobs(spec, BAND_JOB_ROWS)
}

/// The band's rays marched for every reply in `replies` (R06.T9.f's `march_rows`; Design notes 14
/// and 15), as bulk jobs of [`BAND_JOB_ROWS`] rows of a face under `token`, in the band's order:
/// one march per job, at the query's band ([`SkyQuery::band_spec`]), with its illumination.
///
/// A request marches once, for each reply its shells can state (its plan's
/// [`replies`](CensusPlan::replies)), so that every reply it sends is summed from the same nodes,
/// whatever replies it sends: one march a request (R06.T9.f). It reads no census, so it runs while
/// the census does. Each job marches in a context of its own over the inputs' tables. Where a
/// camera's deeper cut sets the query's, each march also keeps the light fainter than the eye's
/// cut, the eye's background (R06.T9.j).
///
/// # Errors
///
/// Those of [`BulkJobs`].
pub(crate) async fn march(
    pool: &CpuPool,
    inputs: &CensusInputs,
    replies: Vec<CompleteTo>,
    token: &CancelToken,
) -> Result<Vec<Arc<BandMarch>>, ComputeError> {
    let spec = inputs.query.band_spec();
    let replies: Arc<[CompleteTo]> = replies.into();
    let jobs = band_jobs(spec).map(|(face, rows)| {
        let (galaxy, tables, query, replies) = (
            Arc::clone(&inputs.galaxy),
            Arc::clone(&inputs.tables),
            Arc::clone(&inputs.query),
            Arc::clone(&replies),
        );
        move || {
            let mut ctx = tables.march_context();
            Arc::new(march_rows(
                &galaxy,
                &mut ctx,
                &query,
                replies.iter().cloned(),
                &spec,
                face,
                rows,
            ))
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

/// The band of a reply whose census is `census` from the request's `marches` ([`march`]'s), summed
/// at the radii the census is complete to, and where the query asks the eye, the limit map and the
/// eye offsets, as bulk jobs under `token` (Design notes 4 and 15; R06.T9.c, T9.h–T9.j; R06.T11.d).
///
/// Where the eye is asked, one job first resolves the listed stars' glare to the eye's cut
/// (`Glare::of_listed`: only the stars at or brighter than it glare, R06.T9.j). Then each march's
/// job sums its texels with the census's overflow (`sum_rows`), the light fainter than the cut
/// within the census's radii and all of the light beyond them, and sets their eye limits against
/// the light fainter than the eye's cut and the glare (`limit_rows`), which reads the glare's
/// pyramid in a fixed order, so any split of rows gives the same bits (R06.T9.i). Once every
/// texel's limit is set, one job gives each listed star its eye offset (`eye_offsets`). The eye's
/// map is so the eye-only request's at the census's radii, whatever the request's cut (decided
/// 2026-10-07, `decision-r06-t9c-glare.md`, addendum 2).
///
/// # Errors
///
/// Those of [`bulk`] and [`BulkJobs`], among them a job's panic, answered `internal`, if `census`
/// is not a census of shells ([`merge_step`]'s) or the marches do not keep its radii.
pub(crate) async fn band(
    pool: &CpuPool,
    query: &Arc<SkyQuery>,
    marches: &[Arc<BandMarch>],
    census: &Arc<SkyCensus>,
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
    // Collected first, so that no borrowing iterator is held across the queue's awaits.
    let jobs: Vec<_> = marches
        .iter()
        .map(|march| {
            let (march, census, eye) = (Arc::clone(march), Arc::clone(census), eye.clone());
            move || {
                let complete_to = census
                    .completeness()
                    .expect("a reply's census is a census of shells")
                    .complete_to();
                let mut texels =
                    Vec::with_capacity(march.rows().len() * usize::from(spec.face_texels()));
                sum_rows(&march, &census, complete_to, &mut texels);
                if let Some((eye, glare)) = &eye {
                    limits::limit_rows(eye, &spec, glare, march.face(), march.rows(), &mut texels);
                }
                texels
            }
        })
        .collect();
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
    use hyperion_sim::sky::caps::{LayerCap, layer_caps};
    use hyperion_sim::sky::census::{Completeness, census_plan, census_plan_of, merge_census};
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

    /// Every star's V, A<sub>V</sub> and distance, listed then overflowing, by their bits:
    /// `PartialEq` holds 0.0 and −0.0 equal.
    fn bits(census: &SkyCensus) -> Vec<u64> {
        census
            .listed()
            .iter()
            .chain(census.overflow())
            .flat_map(|s| [s.v().value(), s.a_v().value(), s.distance().value()])
            .map(f64::to_bits)
            .collect()
    }

    /// The sim's census of `plan`'s `shells`, serially in the shells' order with no cell cache,
    /// merged to the completeness of those shells.
    fn serial_census(
        galaxy: &Galaxy,
        tables: &SkyTables,
        query: &SkyQuery,
        plan: &CensusPlan,
        shells: &[Shell],
    ) -> SkyCensus {
        let mut ctx = tables.context(CENSUS_NOISE_SLOTS, &NoSkyCellCache);
        let mut stars = Vec::new();
        let mut tallies = None;
        for &shell in shells {
            for key in plan.shell_slabs(shell).flat_map(|slab| slab.cells()) {
                add_tallies(
                    &mut tallies,
                    &census_cell(galaxy, &mut ctx, key, query, &mut stars),
                );
            }
        }
        merge_shells(
            tallies.map(|tallies| (stars, tallies)),
            query.n_max(),
            plan.completeness(shells.iter().copied()),
        )
    }

    /// Runs `plan`'s census in `steps` on `pool`, each job's slice `slice`, and returns the
    /// census of each step as [`merge_step`] makes it.
    async fn steps_censused(
        pool: &CpuPool,
        inputs: &CensusInputs,
        plan: &Arc<CensusPlan>,
        steps: &[Vec<Shell>],
        slice: Duration,
    ) -> Vec<(CensusStep, SkyCensus)> {
        let token = CancelToken::new();
        let (sink, mut sent) = mpsc::channel(steps.len());
        timeout(
            WAIT,
            census_in_steps(pool, inputs, plan, steps, slice, sink, &token),
        )
        .await
        .expect("timed out censusing")
        .unwrap();
        let mut censused = Vec::new();
        while let Some(step) = sent.recv().await {
            let census = timeout(
                WAIT,
                merge_step(pool, plan, step.clone(), inputs.query.n_max(), &token),
            )
            .await
            .expect("timed out merging")
            .unwrap();
            censused.push((step, census));
        }
        censused
    }

    /// The census its jobs run step by step is, after each step, the sim's census of the shells
    /// done, serially, star for star and tally for tally, cold and warm, and its last step's is
    /// the sim's one pass over the whole plan: the split into steps and jobs, the jobs' slices
    /// and their order, the joined parts and the cache change nothing (R06.T11.d).
    ///
    /// The census to 30 ly is one shell a layer, so the steps here split its one rank three ways;
    /// a slice of nought hands each job's cells back after one, so that every cell is a job of its
    /// own, queued again behind its step's other slabs.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn each_step_of_the_jobs_census_is_the_sims_census_of_its_shells() {
        let galaxy = Arc::new(Galaxy::new(Seed::new(0x4d2)));
        let caps = SkyCaps::forced(LightYears::new(30.0)).unwrap();
        let tables = Arc::new(SkyTables::new(LuminosityTables::dark(&galaxy), &galaxy));
        let query = caps.on(SkyQuery::builder(sun(), Magnitudes::new(9.0))
            .build()
            .unwrap());
        let query = Arc::new(query);
        let pool = pool(3);
        let token = CancelToken::new();
        let plan = timeout(
            WAIT,
            plan(&pool, &galaxy, &tables, &query, &SHELL_EDGES_LY, &token),
        )
        .await
        .expect("timed out planning")
        .unwrap();
        let plan = Arc::new(plan);
        let inputs = CensusInputs {
            galaxy: Arc::clone(&galaxy),
            key: GalaxyKey::new(0x4d2, GENERATOR_VERSION),
            tables: Arc::clone(&tables),
            cells: Arc::new(SharedSkyCellCache::new(256 << 20)),
            query: Arc::clone(&query),
        };
        let shells: Vec<Shell> = plan.shells().collect();
        assert_eq!(
            shells.len(),
            CAPPED_LAYERS.len(),
            "one shell a layer to 30 ly"
        );
        assert_eq!(delivery_steps(&plan), vec![shells.clone()]);
        let steps: Vec<Vec<Shell>> = shells.chunks(2).map(<[Shell]>::to_vec).collect();

        let cold = steps_censused(&pool, &inputs, &plan, &steps, Duration::ZERO).await;
        let after_cold = inputs.cells.counters();
        let warm = steps_censused(&pool, &inputs, &plan, &steps, CENSUS_JOB_SLICE).await;
        let after_warm = inputs.cells.counters();
        // Every cell the cold census looked up, the warm one found built, its entry holding the
        // query's key and window (R06.T8.h).
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
        assert_eq!(
            (after_cold.missed(), after_warm.served()),
            (lookups, lookups),
            "every cell built cold and served warm: {after_cold:?}, then {after_warm:?}"
        );
        assert!(
            pool.counters().completed() > lookups,
            "a slice of nought takes a job a cell: {:?}",
            pool.counters()
        );

        for censused in [&cold, &warm] {
            assert_eq!(censused.len(), steps.len());
            for (k, (step, census)) in censused.iter().enumerate() {
                let done: Vec<Shell> = steps[..=k].concat();
                assert_eq!(step.done, done);
                assert_eq!(step.last, k + 1 == steps.len());
                let serial = serial_census(&galaxy, &tables, &query, &plan, &done);
                assert_eq!(census, &serial, "step {k}");
                assert_eq!(bits(census), bits(&serial), "step {k}");
            }
        }
        // The last step's is the one-shot census.
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
        let (_, last) = cold.last().expect("a step");
        assert!(!one_pass.listed().is_empty());
        assert!(last.completeness().is_some_and(Completeness::is_final));
        assert_eq!(
            (last.listed(), last.overflow(), last.tallies()),
            (one_pass.listed(), one_pass.overflow(), one_pass.tallies())
        );
        assert_eq!(bits(last), bits(&one_pass));
        pool.shutdown().await.unwrap();
    }

    /// Caps as the derived caps near the Sun put them at the eye's cut, uniform here (R06.T7.b's
    /// farthest rays): C has shells to 500 ly–8,000 ly and its cap, D the same, E to 32,000 ly
    /// and its cap, and A, B and the brown dwarfs one each.
    fn near_sun_plan() -> CensusPlan {
        let query = SkyQuery::builder(sun(), Magnitudes::new(7.95))
            .build()
            .unwrap();
        let caps = [11.0, 68.0, 14_563.0, 13_232.0, 61_341.0, 1.0]
            .iter()
            .zip(CAPPED_LAYERS)
            .map(|(&r, layer)| LayerCap::forced(layer, LightYears::new(r)))
            .collect();
        census_plan_of(&query, caps)
    }

    /// The delivery runs every shell once, nearest first within each layer, every layer's first in
    /// the first step, and D's and E's shells to 4,000 ly before C's beyond 2,000 ly (R06.T11.d).
    #[test]
    fn the_delivery_is_nearest_first_with_d_and_e_to_4000_ly_before_c_beyond_2000() {
        use Layer::{A, B, BrownDwarf, C, D, E};
        let plan = near_sun_plan();
        let steps = delivery_steps(&plan);
        let shape: Vec<Vec<(Layer, u8)>> = steps
            .iter()
            .map(|step| step.iter().map(|s| (s.layer(), s.index())).collect())
            .collect();
        assert_eq!(
            shape,
            vec![
                vec![(A, 0), (B, 0), (C, 0), (D, 0), (E, 0), (BrownDwarf, 0)],
                vec![(C, 1), (D, 1), (E, 1)],
                vec![(C, 2), (D, 2), (E, 2)],
                vec![(D, 3), (E, 3)],
                vec![(C, 3), (D, 4), (E, 4)],
                vec![(C, 4), (D, 5), (E, 5)],
                vec![(C, 5), (E, 6)],
                vec![(E, 7)],
            ]
        );
        let mut all: Vec<Shell> = steps.concat();
        let mut planned: Vec<Shell> = plan.shells().collect();
        all.sort_unstable();
        planned.sort_unstable();
        assert_eq!(all, planned, "every shell once");
        let edge = |shell: &Shell| shell.edge().map_or(f64::INFINITY, LightYears::value);
        // D's and E's shells to 4,000 ly all come before C's first beyond 2,000 ly.
        let c_beyond = steps
            .iter()
            .position(|step| step.iter().any(|s| s.layer() == C && s.index() >= 3))
            .unwrap();
        for (k, step) in steps.iter().enumerate() {
            for shell in step {
                if matches!(shell.layer(), D | E) && edge(shell) <= 4_000.0 {
                    assert!(k < c_beyond, "{shell:?} in step {k}");
                }
            }
        }
        // The first reply has every layer's first shell: A, B and the brown dwarfs are final.
        let first = plan.completeness(steps[0].iter().copied());
        assert_eq!(first.least_edge(), Some(LightYears::new(500.0)));
        for layer in [A, B, BrownDwarf] {
            assert_eq!(first.edge(layer), None, "{layer:?} is final");
        }
        let last = plan.completeness(steps.concat());
        assert!(last.is_final());
    }

    /// One march of the plan's replies holds every step's radii, whatever the delivery's order
    /// (R06.T9.f: one march a request; R06.T11.d).
    #[test]
    fn one_march_of_the_plans_replies_holds_every_steps_radii() {
        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let tables = SkyTables::new(LuminosityTables::dark(&galaxy), &galaxy);
        let plan = near_sun_plan();
        let query = SkyQuery::builder(sun(), Magnitudes::new(7.95))
            .build()
            .unwrap();
        let spec = BandSpec::new(2, 12).unwrap();
        let march = march_rows(
            &galaxy,
            &mut tables.march_context(),
            &query,
            plan.replies(),
            &spec,
            CubeFace::PosZ,
            0..1,
        );
        let mut done = Vec::new();
        for step in delivery_steps(&plan) {
            done.extend(step);
            let completeness = plan.completeness(done.iter().copied());
            assert!(
                march.holds(completeness.complete_to()),
                "after {} shells",
                done.len()
            );
        }
    }

    /// The derived caps, their rays measured in jobs and counted in ray order, are `layer_caps`'
    /// bit for bit, and so is the plan they make (decided 2026-10-03, `decision-r06-tables.md`,
    /// item B.4), with the eye's visibility as without (R06.T7.b), the eye's cut and visibility
    /// both taken in jobs with the request's illumination (R06.T9.g). Over tables that hold no
    /// star every count is nought, so each cap is the count's nearest radius, but each rule bound
    /// reads every ray's extinction.
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
        let planned = timeout(
            WAIT,
            plan(&pool, &galaxy, &tables, &query, &SHELL_EDGES_LY, &token),
        )
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
        // it, bit for bit, its cut and visibility from jobs, with the request's illumination.
        let eye = EyeObserver::default();
        let light = timeout(WAIT, illumination(&pool, &galaxy, &tables, sun(), &token))
            .await
            .expect("timed out lighting")
            .unwrap();
        let mut ctx = tables.march_context();
        assert_eq!(
            *light,
            Illumination::march(&galaxy, &mut ctx, &sun()),
            "the illumination's rows in jobs are its one march"
        );
        let cut = timeout(
            WAIT,
            eye_cut(&pool, &galaxy, &tables, (sun(), eye), &light, &token),
        )
        .await
        .expect("timed out on the eye's cut")
        .unwrap();
        assert_eq!(
            cut.value().to_bits(),
            limits::eye_cut(&galaxy, &mut ctx, &sun(), &eye, Some(&light))
                .value()
                .to_bits()
        );
        let visibility = timeout(
            WAIT,
            eye_visibility(&pool, &galaxy, &tables, (sun(), eye), cut, &light, &token),
        )
        .await
        .expect("timed out on the eye's visibility")
        .unwrap();
        assert_eq!(
            visibility,
            limits::eye_visibility(&galaxy, &mut ctx, &sun(), &eye, cut, Some(&light))
        );
        let seen = Arc::new(
            SkyQuery::builder(sun(), cut)
                .eye(eye)
                .illumination(Arc::clone(&light))
                .eye_visibility(visibility)
                .build()
                .unwrap(),
        );
        let planned = timeout(
            WAIT,
            plan(&pool, &galaxy, &tables, &seen, &SHELL_EDGES_LY, &token),
        )
        .await
        .expect("timed out planning")
        .unwrap();
        let sims = census_plan(&galaxy, &tables.tables, &tables.envelope, &seen, &mut noise);
        assert_eq!(planned, sims);
        pool.shutdown().await.unwrap();
    }

    /// The band's jobs cover each face's rows once, in the band's order, two rows a job, and the
    /// illumination's each of its 16² faces' rows, two a job.
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
        assert_eq!(
            row_jobs(ILLUMINATION_SPEC, ILLUMINATION_JOB_ROWS).count(),
            6 * 8
        );
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
        assert_eq!(SkyCaps::DERIVED.forced_radii(), None);
        assert_eq!(SkyCaps::DERIVED.tables(), SkyTablesSource::Galaxy);
        let forced = SkyCaps::forced(LightYears::new(40.0)).unwrap();
        assert_eq!(
            forced.forced_radii(),
            Some([LightYears::new(40.0); 6]),
            "the radius is kept"
        );
        assert_eq!(forced.tables(), SkyTablesSource::Dark);
        let lit = forced.with_galaxy_tables();
        assert_eq!(
            (lit.forced_radii(), lit.tables()),
            (forced.forced_radii(), SkyTablesSource::Galaxy)
        );
        assert!(SkyCaps::forced(LightYears::ZERO).is_ok());
        assert!(SkyCaps::forced(LightYears::new(MAX_FORCED_CAP_LY)).is_ok());
        for bad in [-1.0, f64::NAN, f64::INFINITY, MAX_FORCED_CAP_LY + 1.0] {
            assert_eq!(
                SkyCaps::forced(LightYears::new(bad)),
                Err(ForceSkyCapsError),
                "{bad}"
            );
            let mut radii = [LightYears::new(30.0); 6];
            radii[4] = LightYears::new(bad);
            assert_eq!(
                SkyCaps::forced_per_layer(radii),
                Err(ForceSkyCapsError),
                "{bad}"
            );
        }
        // Each layer forced to its own radius, as the query then forces it.
        let radii = [10.0, 20.0, 30.0, 40.0, 600.0, 5.0].map(LightYears::new);
        let per_layer = SkyCaps::forced_per_layer(radii).unwrap();
        let query = per_layer.on(SkyQuery::builder(sun(), Magnitudes::new(9.0))
            .build()
            .unwrap());
        let forced: Vec<(Layer, f64)> = query
            .forced_caps()
            .expect("forced")
            .iter()
            .map(|cap| (cap.layer(), cap.radius().value()))
            .collect();
        assert_eq!(
            forced,
            CAPPED_LAYERS
                .iter()
                .copied()
                .zip(radii.map(LightYears::value))
                .collect::<Vec<_>>()
        );
        let derived = SkyCaps::DERIVED.on(SkyQuery::builder(sun(), Magnitudes::new(9.0))
            .build()
            .unwrap());
        assert!(derived.forced_caps().is_none());
    }
}
