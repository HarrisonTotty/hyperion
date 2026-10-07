//! The sky's work on the CPU pool (rendering plan R06, R06.T11.a; Design notes 5 and 9–11): the
//! tables a census reads, and the census itself, as bulk jobs.
//!
//! Everything a `sky` request computes runs at [`Priority::Bulk`], never in the interactive queue,
//! so a chart's query is never queued behind it: the workers take interactive work first. The
//! census runs its plan's cells a few hundred to a job ([`CENSUS_JOB_CELLS`]), so a query waits for
//! at most one census job on a busy worker. The tables, the eye's cut and the plan are one job each
//! until R06.T11.c splits the tables and the caps into staged jobs, and a query may wait behind one
//! of them. Every job runs under the request's [`CancelToken`]: a cancelled request's queued jobs
//! are skipped, and a running census job stops at its next cell. The parts are merged once every
//! job has finished, in one more bulk job ([`merge_census`]'s order is total, so the split cannot
//! change the answer). Neither a census's sources nor its noise cache can be shared between
//! threads, so each job builds its own [`SkyContext`] over the [`SkyTables`] they all read, and
//! over the server's [`SharedSkyCellCache`], which every job and request shares (R06.T11.b; Design
//! note 12).
//!
//! [`SkyCaps`] says how far the census looks: each layer's derived cap (Design note 9), or one
//! forced radius, which keeps a test's census small.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::CellKey;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::census::{
    CellOffsets, CensusPlan, CensusTallies, MAX_FORCED_CAP_LY, NoSkyCellCache, SkyCellCache,
    SkyCensus, SkyContext, SkyQuery, SkyStar, census_cell, census_plan, merge_census,
};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::limits;
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::units::{LightYears, Magnitudes};
use tokio::sync::oneshot::error::RecvError;

use super::{
    CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyKey, JobError, Priority,
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

/// Noise-cache slots of a census job: 256 KiB, a cache's worth for the sightlines of a few hundred
/// cells' stars. The cache changes no value, only how often a normal is drawn again.
const CENSUS_NOISE_SLOTS: usize = 1 << 14;

/// Noise-cache slots of the jobs that march many rays (the eye's pre-pass and the caps): 1 MiB.
const MARCH_NOISE_SLOTS: usize = 1 << 16;

/// How far a sky's census looks in each layer (Design note 9): the caps the galaxy's luminosity
/// tables and dust derive, or one radius forced on every layer.
///
/// Forced caps are for tests and benches, which need a census of seconds rather than of the
/// thousands of CPU-seconds a census to the derived caps near the Sun takes (R06's Risks, the
/// census cost). A census with forced caps reads no luminosity table, so it is given tables that
/// hold no star ([`LuminosityTables::dark`]), built at once. The eye's cut is then a dark sky's:
/// Crumey's darkest-background limit plus the cut's colour offset and pad, V 8.54 for the default
/// eye. A reply states each layer's cap as the radius, with nothing expected beyond it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SkyCaps {
    forced: Option<LightYears>,
}

impl SkyCaps {
    /// Each layer's cap derived from the galaxy, the default.
    pub const DERIVED: Self = Self { forced: None };

    /// Every layer's cap forced to `radius`.
    ///
    /// # Errors
    ///
    /// [`ForceSkyCapsError`] unless `radius` is finite, non-negative and within the root cube's
    /// diagonal ([`MAX_FORCED_CAP_LY`]).
    pub fn forced(radius: LightYears) -> Result<Self, ForceSkyCapsError> {
        if (0.0..=MAX_FORCED_CAP_LY).contains(&radius.value()) {
            Ok(Self {
                forced: Some(radius),
            })
        } else {
            Err(ForceSkyCapsError)
        }
    }

    /// The radius every layer's cap is forced to, or `None` for the derived caps.
    #[must_use]
    pub const fn forced_radius(&self) -> Option<LightYears> {
        self.forced
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
/// Built for each request in one bulk job, from the galaxy and its [`SkyCaps`]. R06.T11.c builds
/// the tables once per galaxy instead, as staged jobs under a single flight, and caches them.
#[derive(Debug)]
pub(crate) struct SkyTables {
    tables: LuminosityTables,
    envelope: BrightnessEnvelope,
    offsets: CellOffsets,
}

impl SkyTables {
    /// The tables of `galaxy`: built from its components for the derived caps, which takes a minute
    /// or more of one worker, or holding no star for forced ones.
    #[must_use]
    pub(crate) fn build(galaxy: &Galaxy, caps: SkyCaps) -> Self {
        let tables = match caps.forced_radius() {
            None => LuminosityTables::build(galaxy),
            Some(_) => LuminosityTables::dark(galaxy),
        };
        Self {
            tables,
            envelope: BrightnessEnvelope::build(galaxy),
            offsets: CellOffsets::build(galaxy),
        }
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

    /// A context for a job that marches many rays: the eye's pre-pass and the plan's caps.
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
    let receiver = pool.submit(Priority::Bulk, token.clone(), job).await?;
    Ok(receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?)
}

/// The tables a sky of `galaxy` reads under `caps`, built in one bulk job under `token`.
///
/// # Errors
///
/// Those of [`bulk`].
pub(crate) async fn tables(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    caps: SkyCaps,
    token: &CancelToken,
) -> Result<Arc<SkyTables>, ComputeError> {
    let galaxy = Arc::clone(galaxy);
    bulk(pool, token, move |_: &CancelToken| {
        Arc::new(SkyTables::build(&galaxy, caps))
    })
    .await
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
    bulk(pool, token, move |_: &CancelToken| {
        limits::eye_cut(&galaxy, &mut tables.march_context(), &observer, &eye)
    })
    .await
}

/// The census's plan for `query` (Design notes 9 and 10), in one bulk job under `token`: each
/// layer's cap, from the caps' rays unless they are forced, and the cells they open.
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
    bulk(pool, token, move |_: &CancelToken| {
        let mut noise = NoiseCache::with_capacity(MARCH_NOISE_SLOTS);
        census_plan(
            &galaxy,
            &tables.tables,
            &tables.envelope,
            &query,
            &mut noise,
        )
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

/// What every job of one census reads, each shared: the galaxy and its key, the tables, the cell
/// cache and the query.
#[derive(Debug, Clone)]
pub(crate) struct CensusInputs {
    /// The galaxy censused.
    pub(crate) galaxy: Arc<Galaxy>,
    /// The key of `galaxy`, under which its cells are cached.
    pub(crate) key: GalaxyKey,
    /// The tables the census reads.
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

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use tokio::time::timeout;

    use super::*;
    use crate::limits::INTERACTIVE_QUEUE_CAPACITY;
    use crate::testing::WAIT;

    /// The census its jobs merge is the sim's one pass over the same plan with no cell cache, star
    /// for star and tally for tally, cold and warm: the split into jobs, their order, the joined
    /// parts and the cache change nothing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_jobs_census_is_the_one_pass_census_star_for_star() {
        let galaxy = Arc::new(Galaxy::new(Seed::new(0x4d2)));
        let caps = SkyCaps::forced(LightYears::new(30.0)).unwrap();
        let tables = Arc::new(SkyTables::build(&galaxy, caps));
        let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).unwrap();
        let observer = Observer::new(at, UniverseTime::EPOCH).unwrap();
        let query = SkyQuery::builder(observer, Magnitudes::new(9.0))
            .build()
            .unwrap()
            .with_caps_forced(caps.forced_radius().unwrap())
            .unwrap();
        let query = Arc::new(query);
        let pool = CpuPool::new(
            NonZeroUsize::new(3).unwrap(),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
        )
        .unwrap();
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

    #[test]
    fn a_forced_cap_is_held_to_the_root_cubes_diagonal() {
        assert_eq!(SkyCaps::default(), SkyCaps::DERIVED);
        assert_eq!(SkyCaps::DERIVED.forced_radius(), None);
        let forced = SkyCaps::forced(LightYears::new(40.0)).unwrap();
        assert_eq!(
            forced.forced_radius().map(LightYears::value),
            Some(40.0),
            "the radius is kept"
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
