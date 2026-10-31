//! A served range query's cells, generated and walked ahead of its finish on the CPU pool's
//! workers (P09.T40.a).
//!
//! A cold 50 ly query at the Sun-like point reads some 1,750 cells nobody has generated: twenty
//! feature cells, 5.8 ms of the query's 14.8, and 1,732 grid cells, 7.6 ms (lane measurements of
//! 2026-10-30, pod, 16 vCPU). Each is a pure function of the galaxy and its key, so they may be
//! generated in any order on any thread, and each grid cell's walk with them
//! ([`RangePlan::walk_cells`]). [`served_range_query`] does so, in three steps of pool jobs:
//!
//! 1. One job asks what the query will read that the caches lack: the feature cells the members'
//!    source looks up ([`FeatureMemberSource::feature_cells`]), each with the last process it
//!    reads it through, and then the grid cells of the query's plan ([`RangePlan::cells`]). With
//!    its feature cells held, that is the plan with the sources, as a warm query plans once;
//!    otherwise it is the plan without them, which holds every cell the plan with them walks
//!    ([`RangePlan::with_sources`]), since the sources' counts need the cells being generated. If
//!    none is worth generating ahead, it walks there and then.
//! 2. Otherwise the cells it found lacking, both kinds at once, are shared out among jobs, about
//!    three for each worker: the feature cells' jobs read them into the shared caches, and the
//!    grid's generate theirs into the cell cache and walk each as it is made. Once the feature
//!    cells' jobs end, one more job plans the query with its sources, over the warm feature cells,
//!    if it was planned without them, and asks the sources for their hits
//!    ([`RangePlan::source_hits`]), while the last of the grid's jobs run.
//! 3. The last job merges the grid's walked parts and the sources' hits into the answer
//!    ([`RangePlan::finish`]).
//!
//! **No job waits on another.** The pool's queues are plain first-in, first-out, and a worker runs
//! one job to its end: a job that queued its cells and blocked on their replies would hold its
//! worker, and with every worker so held the cells would never run, so the pool would stall. So
//! every job here is queued by the request's own task, on the runtime, which awaits their replies
//! without holding a worker; a job only ever generates, asks, plans or walks. The sources' job is
//! queued only once the feature cells' jobs have ended, or beside the grid's if there are none, so
//! a query never holds more than three of the queue's places for each worker, as its cells' jobs
//! did before it.
//!
//! **Nothing here changes an answer.** The finish is [`RangePlan::finish`], which is
//! [`range_query`](hyperion_sim::galaxy::query::range_query) bit for bit whatever parts it is
//! given: it walks through the shared caches any cell no part holds, and plans with the sources
//! and asks them itself if their job failed. So a cell job that was cancelled or failed, a cell
//! evicted, and the number of workers change only how long the answer takes.

use std::cmp::Reverse;
use std::num::NonZeroUsize;
use std::sync::Arc;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
use hyperion_sim::galaxy::placement::CellKey;
use hyperion_sim::galaxy::query::{
    RangePlan, RangeQuery, RangeResult, SourceHits, SystemSource, WalkedCells,
};
use hyperion_sim::id::FeatureCell;

use super::{
    CancelToken, ComputeError, CpuPool, GalaxyKey, JobError, JobReceiver, KeptInteriors, Priority,
    SharedCellCache, SharedClusterModelCache, SharedFeatureCellCache, SubmitJobError,
};
use crate::FeatureMembers;

/// The fewest feature cells worth generating ahead of a query: two, since one takes 0.1–0.7 ms,
/// well above a job's round trip (lane measurements of 2026-10-30).
const AHEAD_FEATURE_CELLS: usize = 2;

/// The fewest grid cells worth generating ahead of a query: 64, some 0.28 ms of cells at the 4.4 µs
/// a grid cell near the Sun takes (1,732 in 7.6 ms, lane measurements of 2026-10-30), where a
/// round of jobs costs some tens of microseconds; fewer would cost more than they save.
const AHEAD_GRID_CELLS: usize = 64;

/// The grid's cell jobs for each worker: two, smaller than a feature cell's, to fill the workers
/// the feature cells leave idle. Two to six took the Sun-like 50 ly query the same time cold, 4.6
/// to 5.2 ms on 8 workers, and one some 0.3 ms longer (measured 2026-10-31, pod, 16 vCPU), so it is
/// the fewest of the queue's places that does as well.
const GRID_JOBS_PER_WORKER: usize = 2;

/// The stride of the grid cells a query first asks the cell cache for, to tell a warm or a cold
/// grid without asking for every cell: 16, 109 of the Sun-like 50 ly query's 1,732 cells. Asking for
/// all of them took 0.13 ms, a tenth of the warm query (measured 2026-10-31, pod, 16 vCPU).
const LACKING_SAMPLE_STRIDE: usize = 16;

/// The caches a served range query reads: the grid's cells, the feature cells and the features'
/// interiors, shared with the jobs that generate cells for it.
#[derive(Debug, Clone)]
pub struct RangeCaches {
    cells: Arc<SharedCellCache>,
    features: Arc<SharedFeatureCellCache>,
    interiors: Arc<SharedClusterModelCache>,
}

impl RangeCaches {
    /// The caches a query reads, as the server holds them.
    #[must_use]
    pub fn new(
        cells: Arc<SharedCellCache>,
        features: Arc<SharedFeatureCellCache>,
        interiors: Arc<SharedClusterModelCache>,
    ) -> Self {
        Self {
            cells,
            features,
            interiors,
        }
    }
}

/// One served range query: the galaxy its universe names and that galaxy's key, the query, and
/// whether the features' members are served beside the grid.
#[derive(Debug, Clone)]
pub struct ServedQuery {
    galaxy: Arc<Galaxy>,
    key: GalaxyKey,
    query: RangeQuery,
    members: FeatureMembers,
}

impl ServedQuery {
    /// `query` in `galaxy`, the galaxy `key` names, with the feature members `members` says.
    #[must_use]
    pub fn new(
        galaxy: Arc<Galaxy>,
        key: GalaxyKey,
        query: RangeQuery,
        members: FeatureMembers,
    ) -> Self {
        Self {
            galaxy,
            key,
            query,
            members,
        }
    }
}

/// The answer to `served` from `answer` over the query's result, its cells generated and walked
/// first on `pool`'s workers into `caches` (module documentation).
///
/// `answer` runs on the job that finishes the query, with the galaxy, the result and the job's
/// kept view of the interiors. The result is
/// [`range_query`](hyperion_sim::galaxy::query::range_query)'s over the same caches and sources,
/// bit for bit, at any number of workers.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the query's first job finds the interactive queue full (its later
/// jobs and its cells' wait for room, as the query has begun) or the pool shutting down or
/// faulted, and [`ComputeError::Job`] if one of the query's own jobs is cancelled, panics, is lost
/// with the pool or leaves its worker's floating-point mode flushing subnormals. A cell job's
/// failure, or that of the job asking the sources, is none of these: the finish does what they
/// did not.
pub async fn served_range_query<T, F>(
    pool: &CpuPool,
    caches: &RangeCaches,
    served: &ServedQuery,
    token: &CancelToken,
    answer: F,
) -> Result<T, ComputeError>
where
    F: FnOnce(&Galaxy, RangeResult, &KeptInteriors<'_>) -> T + Send + 'static,
    T: Send + 'static,
{
    let (job_caches, job_served, workers) = (caches.clone(), served.clone(), pool.workers());
    let asked = pool
        .try_submit(
            Priority::Interactive,
            token.clone(),
            move |_: &CancelToken| ask(&job_caches, &job_served, workers, answer),
        )
        .map_err(ComputeError::Submit)?;
    let Lacking {
        features,
        grid,
        plan,
        planned,
        answer,
    } = match reply(asked).await? {
        Asked::Answered(answered) => return Ok(answered),
        Asked::Lacking(lacking) => *lacking,
    };
    let ahead = ahead(
        pool,
        caches,
        served,
        (Arc::new(plan), planned),
        (features, grid),
        token,
    )
    .await;
    let (job_caches, job_served) = (caches.clone(), served.clone());
    // As the cells do, the finish waits for room in the queue, as the query has begun.
    let finished = pool
        .submit(
            Priority::Interactive,
            token.clone(),
            move |_: &CancelToken| finish(&job_caches, &job_served, ahead, answer),
        )
        .await
        .map_err(ComputeError::Submit)?;
    reply(finished).await
}

/// What a query's job sent back, or why it sent nothing.
async fn reply<R>(receiver: JobReceiver<R>) -> Result<R, ComputeError> {
    receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))
        .map_err(ComputeError::Job)
}

/// Whether a query's plan was made with its sources.
#[derive(Debug, Clone, Copy)]
enum Planned {
    /// With them: the plan is the finish's.
    WithSources,
    /// Without them: the finish's is made from it with them ([`RangePlan::with_sources`]).
    WithoutSources,
}

/// What a query's first job hands back: its answer, or the cells it found lacking.
enum Asked<T, F> {
    /// The answer, walked there and then.
    Answered(T),
    /// The cells the caches lack, with what the query is walked and answered with.
    Lacking(Box<Lacking<F>>),
}

/// One job's feature cells, each with the last process the query reads it through.
type FeatureShare = Vec<(FeatureCell, FeatureProcess)>;

/// The cells the caches lack for a query, dealt out among the jobs that generate them, and what
/// the query is then walked and answered with.
struct Lacking<F> {
    /// The feature cells, one share for each job, the fullest dealt first.
    features: Vec<FeatureShare>,
    /// The grid cells of the plan, in walk order, dealt out in turn, one share for each job.
    grid: Vec<Vec<CellKey>>,
    /// The query's plan.
    plan: RangePlan,
    /// Whether the plan was made with the query's sources.
    planned: Planned,
    /// What answers the query.
    answer: F,
}

/// What the jobs ahead of a query's finish made: the parts of its walk, and its plan with the
/// sources and their hits, or what they are made from if the job that makes them failed.
struct Ahead {
    parts: Vec<WalkedCells>,
    sourced: Sourced,
}

/// A query's plan with its sources and their hits, or what they are made from.
enum Sourced {
    /// Made ahead of the finish.
    Made(Box<(RangePlan, SourceHits)>),
    /// Left to the finish: the first job's plan, and whether it was made with the sources.
    Left(Arc<RangePlan>, Planned),
}

/// `f` of the feature member source over `caches` and the sources it is beside the grid as, with
/// the kept view of the interiors it reads.
///
/// One kept view for the whole job: its census, its hits and its rows' members read the same
/// interiors, each built at most once whatever the shared cache's budget.
fn with_sources<R>(
    caches: &RangeCaches,
    served: &ServedQuery,
    f: impl FnOnce(&[&dyn SystemSource], &KeptInteriors<'_>) -> R,
) -> R {
    let (features, interiors) = (
        caches.features.handle(served.key),
        caches.interiors.kept(served.key),
    );
    let source = FeatureMemberSource::new(&features, &interiors);
    // Held, no sources: the grid is the whole answer, as in plan 04 (P09.T40.a's gate).
    let sources: &[&dyn SystemSource] = match served.members {
        FeatureMembers::Held => &[],
        FeatureMembers::Served => &[&source],
    };
    f(sources, &interiors)
}

/// `plan` with `sources`, made from it if `planned` says it was made without them: the plan
/// itself if it was made with them, cloned only if another job still holds it.
#[must_use]
fn with_its_sources(
    galaxy: &Galaxy,
    plan: Arc<RangePlan>,
    planned: Planned,
    sources: &[&dyn SystemSource],
) -> RangePlan {
    match planned {
        Planned::WithSources => Arc::unwrap_or_clone(plan),
        Planned::WithoutSources => plan.with_sources(galaxy, sources),
    }
}

/// A query's first job: the cells the caches lack, dealt out among jobs for `workers` workers, or
/// the answer if too few are lacking to be worth generating ahead (module documentation).
fn ask<T, F>(
    caches: &RangeCaches,
    served: &ServedQuery,
    workers: NonZeroUsize,
    answer: F,
) -> Asked<T, F>
where
    F: FnOnce(&Galaxy, RangeResult, &KeptInteriors<'_>) -> T,
{
    let (galaxy, key, query) = (served.galaxy.as_ref(), served.key, &served.query);
    with_sources(caches, served, |sources, interiors| {
        let mut features: Vec<_> = match served.members {
            FeatureMembers::Held => Vec::new(),
            FeatureMembers::Served => FeatureMemberSource::feature_cells(galaxy, query)
                .into_iter()
                .filter(|&(cell, last)| !caches.features.holds(key, cell, last))
                .collect(),
        };
        // With too few feature cells to generate ahead, the sources' counts are as cheap now as
        // after, so the query plans once, as a warm one did before P09.T40.a.
        let planned = if features.len() < AHEAD_FEATURE_CELLS {
            Planned::WithSources
        } else {
            Planned::WithoutSources
        };
        let plan = match planned {
            Planned::WithSources => RangePlan::new(galaxy, sources, query),
            Planned::WithoutSources => RangePlan::new(galaxy, &[], query),
        };
        let grid = lacking_grid_cells(&caches.cells, key, &plan);
        // Nothing worth generating ahead is lacking only where the plan was made with the
        // sources, and so is the walk's.
        if let Planned::WithSources = planned
            && grid.len() < AHEAD_GRID_CELLS
        {
            let result = plan.run(galaxy, &mut caches.cells.handle(key), sources);
            return Asked::Answered(answer(galaxy, result, interiors));
        }
        // A feature cell read through more processes costs more, a whole one near the Sun some
        // 0.65 ms against 0.1 ms through the clusters alone, so they are dealt out from the
        // fullest.
        features.sort_by_key(|&(cell, last)| (Reverse(last.index()), cell));
        Asked::Lacking(Box::new(Lacking {
            features: shares(workers.get(), &features),
            grid: shares(GRID_JOBS_PER_WORKER * workers.get(), &grid),
            plan,
            planned,
            answer,
        }))
    })
}

/// A query's last job: its plan with the sources and their hits, from `ahead` or made now, the
/// grid's walked parts merged with them ([`RangePlan::finish`]), and the answer.
fn finish<T, F>(caches: &RangeCaches, served: &ServedQuery, ahead: Ahead, answer: F) -> T
where
    F: FnOnce(&Galaxy, RangeResult, &KeptInteriors<'_>) -> T,
{
    let (galaxy, key) = (served.galaxy.as_ref(), served.key);
    let Ahead { parts, sourced } = ahead;
    with_sources(caches, served, |sources, interiors| {
        let (plan, hits) = match sourced {
            Sourced::Made(made) => *made,
            Sourced::Left(plan, planned) => {
                let plan = with_its_sources(galaxy, plan, planned, sources);
                let hits = plan.source_hits(galaxy, sources);
                (plan, hits)
            }
        };
        let result = plan.finish(galaxy, &mut caches.cells.handle(key), sources, &parts, hits);
        answer(galaxy, result, interiors)
    })
}

/// The grid cells `plan` walks that `cells` lacks, in walk order.
///
/// The cache is first asked for every [`LACKING_SAMPLE_STRIDE`]th cell. If it holds them all, the
/// grid is taken as warm and none is lacking; if it lacks them all, the grid is taken as cold and
/// every cell is; only between the two is it asked for the rest. A cell taken as lacking that the
/// cache holds is generated again, and one taken as held that it lacks is the finish's to walk, as
/// every cell was before P09.T40.a: either costs time, never an answer.
fn lacking_grid_cells(cells: &SharedCellCache, key: GalaxyKey, plan: &RangePlan) -> Vec<CellKey> {
    let sample = cells.lacking(key, plan.cells().step_by(LACKING_SAMPLE_STRIDE));
    if sample.is_empty() {
        return Vec::new();
    }
    let walked: Vec<_> = plan.cells().collect();
    if sample.len() == walked.len().div_ceil(LACKING_SAMPLE_STRIDE) {
        walked
    } else {
        cells.lacking(key, walked)
    }
}

/// `items` shared out among `jobs` jobs, but never more jobs than items, each job taking every so
/// many in turn so that each kind of cell is spread across them, and each share is in the items'
/// order.
fn shares<T: Copy>(jobs: usize, items: &[T]) -> Vec<Vec<T>> {
    let jobs = jobs.min(items.len());
    (0..jobs)
        .map(|first| items.iter().skip(first).step_by(jobs).copied().collect())
        .collect()
}

/// `job` queued on `pool` behind `token`, waiting for room as the query has begun, or `None` if
/// the pool is shutting down or faulted: the query's last job is refused too, and reports it.
async fn queue<R: Send + 'static>(
    pool: &CpuPool,
    token: &CancelToken,
    job: impl FnOnce(&CancelToken) -> R + Send + 'static,
) -> Option<JobReceiver<R>> {
    #[expect(
        clippy::manual_ok_err,
        reason = "the match names every variant, so that one added must be weighed here"
    )]
    match pool.submit(Priority::Interactive, token.clone(), job).await {
        Ok(receiver) => Some(receiver),
        // `submit` waits out a full queue, and the other two are for good.
        Err(SubmitJobError::QueueFull | SubmitJobError::ShutDown | SubmitJobError::Faulted) => None,
    }
}

/// The jobs ahead of a query's finish, on `pool`'s workers (module documentation): each share of
/// `features` read through its processes, with the grid reach bounds and the interiors of their
/// features that the query reads ([`FeatureMemberSource::read_ahead`]); each share of `grid`
/// generated into the cell cache and walked with `plan` ([`SharedCellCache::walk_generated`]);
/// and, once the feature cells' jobs end, the plan with the sources and their hits. It waits for
/// every job it queued.
///
/// The feature cells' jobs, one for each worker, are queued first; the grid's, some microseconds
/// a cell, follow in twice as many jobs, which fill the workers the feature cells leave idle; and
/// the sources' job, queued once the feature cells' have left the queue, runs beside the last of
/// the grid's. A query without feature cells to wait for queues it ahead of the grid's jobs. A job
/// cancelled or failed is passed over: what it did not do is the finish's.
async fn ahead(
    pool: &CpuPool,
    caches: &RangeCaches,
    served: &ServedQuery,
    (plan, planned): (Arc<RangePlan>, Planned),
    (features, grid): (Vec<FeatureShare>, Vec<Vec<CellKey>>),
    token: &CancelToken,
) -> Ahead {
    let key = served.key;
    let sources_job = || {
        let (caches, served, plan) = (caches.clone(), served.clone(), Arc::clone(&plan));
        move |_: &CancelToken| {
            with_sources(&caches, &served, |sources, _| {
                let plan = with_its_sources(&served.galaxy, plan, planned, sources);
                let hits = plan.source_hits(&served.galaxy, sources);
                (plan, hits)
            })
        }
    };
    // Held, the sources are none, and the finish asks them at no cost.
    let asks_sources = matches!(served.members, FeatureMembers::Served);
    let mut feature_jobs = Vec::new();
    for share in features {
        let (caches, served) = (caches.clone(), served.clone());
        let job = move |_: &CancelToken| {
            let (cells, interiors) = (caches.features.handle(key), caches.interiors.handle(key));
            let source = FeatureMemberSource::new(&cells, &interiors);
            for cell in share {
                source.read_ahead(&served.galaxy, &served.query, cell);
            }
        };
        feature_jobs.extend(queue(pool, token, job).await);
    }
    let mut asking = None;
    if asks_sources && feature_jobs.is_empty() {
        asking = queue(pool, token, sources_job()).await;
    }
    let mut grid_jobs = Vec::new();
    for share in grid {
        let (caches, served, plan) = (caches.clone(), served.clone(), Arc::clone(&plan));
        let job = move |_: &CancelToken| {
            with_sources(&caches, &served, |sources, _| {
                caches
                    .cells
                    .walk_generated(key, &served.galaxy, &plan, sources, share)
            })
        };
        grid_jobs.extend(queue(pool, token, job).await);
    }
    if !feature_jobs.is_empty() {
        for receiver in feature_jobs {
            // Its cells are held now, or left to the jobs after it, as the documentation says.
            let _ = receiver.await;
        }
        if asks_sources {
            asking = queue(pool, token, sources_job()).await;
        }
    }
    let mut parts = Vec::with_capacity(grid_jobs.len());
    for receiver in grid_jobs {
        // A part lost is walked by the finish, as the documentation says.
        if let Ok(Ok(part)) = receiver.await {
            parts.push(part);
        }
    }
    let sourced = match asking {
        Some(receiver) => match receiver.await {
            Ok(Ok(made)) => Sourced::Made(Box::new(made)),
            Ok(Err(_)) | Err(_) => Sourced::Left(plan, planned),
        },
        None => Sourced::Left(plan, planned),
    };
    Ahead { parts, sourced }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;
    use std::time::Duration;

    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
    use hyperion_sim::galaxy::features::members::NoInteriorCache;
    use hyperion_sim::galaxy::placement::NoCache;
    use hyperion_sim::galaxy::query::range_query;
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::units::LightYears;
    use hyperion_sim::{GENERATOR_VERSION, Seed};

    use super::*;
    use crate::convert::origins::{SEED, galaxy};
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};

    fn key() -> GalaxyKey {
        GalaxyKey::new(SEED, GENERATOR_VERSION)
    }

    /// The server tests' galaxy, shared with the pools' jobs.
    fn shared_galaxy() -> Arc<Galaxy> {
        static GALAXY: OnceLock<Arc<Galaxy>> = OnceLock::new();
        Arc::clone(GALAXY.get_or_init(|| Arc::new(galaxy().clone())))
    }

    /// A pool of `workers` with room for `interactive` queued jobs.
    fn pool(workers: usize, interactive: NonZeroUsize) -> CpuPool {
        CpuPool::new(
            NonZeroUsize::new(workers).expect("at least one worker"),
            interactive,
            BULK_QUEUE_CAPACITY,
        )
        .expect("the pool starts")
    }

    /// Fresh caches of `bytes` each.
    fn caches(bytes: usize) -> RangeCaches {
        RangeCaches::new(
            Arc::new(SharedCellCache::new(bytes)),
            Arc::new(SharedFeatureCellCache::new(bytes)),
            Arc::new(SharedClusterModelCache::new(bytes)),
        )
    }

    /// The 50 ly query at the Sun-like point at `years` Julian years after the epoch.
    fn sun_like(years: i64) -> RangeQuery {
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
        RangeQuery::builder(sun, LightYears::new(50.0))
            .time(UniverseTime::from_julian_years(years).expect("inside the clock window"))
            .build()
            .expect("a query in the clock window")
    }

    /// `f` of the sources beside the grid as `members` says, through no cache.
    fn with_sources<R>(members: FeatureMembers, f: impl FnOnce(&[&dyn SystemSource]) -> R) -> R {
        let source = FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache);
        let sources: &[&dyn SystemSource] = match members {
            FeatureMembers::Held => &[],
            FeatureMembers::Served => &[&source],
        };
        f(sources)
    }

    /// `query`'s result as the sim's range query gives it through no cache, with the feature
    /// members as `members` says.
    fn uncached(query: &RangeQuery, members: FeatureMembers) -> RangeResult {
        with_sources(members, |sources| {
            range_query(galaxy(), &mut NoCache::new(), sources, query)
        })
    }

    /// `served` on `pool` over `caches`, within a minute, which a stalled pool would never meet.
    async fn served_on(pool: &CpuPool, caches: &RangeCaches, served: &ServedQuery) -> RangeResult {
        tokio::time::timeout(
            Duration::from_secs(60),
            served_range_query(pool, caches, served, &CancelToken::new(), |_, result, _| {
                result
            }),
        )
        .await
        .expect("the pool does not stall")
        .expect("the query's jobs run")
    }

    /// A served query's result is the sim's, bit for bit, cold and warm, on one worker and on
    /// eight, with the members served and held and away from the epoch; and cold, its finish
    /// takes every grid cell it reads from the cell jobs' walks, of the plan without the sources,
    /// whose cells hold the plan's with them, and asks the cell cache for none (P09.T40.a). One
    /// worker runs every cell job and then the finish in turn, so no job ever waits on another.
    #[tokio::test]
    async fn a_served_query_is_the_sim_s_at_one_and_eight_workers() {
        for (query, members) in [
            (sun_like(0), FeatureMembers::Served),
            (sun_like(400), FeatureMembers::Served),
            (sun_like(0), FeatureMembers::Held),
        ] {
            let expected = uncached(&query, members);
            assert_ne!(expected.systems(), &[]);
            let served = ServedQuery::new(shared_galaxy(), key(), query.clone(), members);
            // The cells generated ahead are those of the plan without the sources, which hold
            // those of the plan with them that the walk reads.
            let unsourced = RangePlan::new(galaxy(), &[], &query);
            let walked = with_sources(members, |sources| {
                let sourced = RangePlan::new(galaxy(), sources, &query);
                assert_eq!(unsourced.with_sources(galaxy(), sources), sourced);
                sourced.cells().collect::<Vec<_>>()
            });
            let ahead: std::collections::BTreeSet<_> = unsourced.cells().collect();
            assert!(walked.iter().all(|cell| ahead.contains(cell)));
            for workers in [1, 8] {
                let pool = pool(workers, INTERACTIVE_QUEUE_CAPACITY);
                let caches = caches(64 << 20);
                assert_eq!(served_on(&pool, &caches, &served).await, expected, "cold");
                let cold = caches.cells.counters();
                assert_eq!(
                    cold.entries(),
                    ahead.len(),
                    "every cell generated ahead, once"
                );
                assert_eq!(
                    (cold.hits(), cold.misses()),
                    (0, 0),
                    "the finish takes every cell it reads from a cell job's walk"
                );
                assert_eq!(served_on(&pool, &caches, &served).await, expected, "warm");
                pool.shutdown().await.unwrap();
            }
        }
    }

    /// The answer is the sim's whatever holds back or drops the cells generated ahead: a queue
    /// with room for one job, which the cell jobs wait on one at a time, and caches that keep
    /// nothing.
    #[tokio::test]
    async fn a_full_queue_or_no_cache_leaves_the_cells_to_the_walk() {
        let query = sun_like(0);
        let expected = uncached(&query, FeatureMembers::Served);
        let served = ServedQuery::new(shared_galaxy(), key(), query, FeatureMembers::Served);
        for (interactive, bytes) in [
            (NonZeroUsize::MIN, 64 << 20),
            (INTERACTIVE_QUEUE_CAPACITY, 0),
        ] {
            let pool = pool(8, interactive);
            let caches = caches(bytes);
            for pass in ["cold", "warm"] {
                assert_eq!(served_on(&pool, &caches, &served).await, expected, "{pass}");
            }
            pool.shutdown().await.unwrap();
        }
    }

    /// A full queue refuses a query's first job at once, as a range request's always was, so the
    /// client hears `queue_full`; and a cancelled query answers that it was cancelled, whatever
    /// of its cells was generated (P09.T40.a).
    #[tokio::test]
    async fn a_full_queue_refuses_the_query_and_a_cancelled_one_says_so() {
        let served = ServedQuery::new(shared_galaxy(), key(), sun_like(0), FeatureMembers::Served);
        let pool = pool(1, NonZeroUsize::MIN);
        let caches = caches(64 << 20);
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release, release_rx) = std::sync::mpsc::channel::<()>();
        let held = pool
            .try_submit(Priority::Interactive, CancelToken::new(), move |_| {
                started_tx.send(()).unwrap();
                // Blocks the one worker, not the runtime; a failed test's dropped sender also
                // releases it.
                let _ = release_rx.recv();
            })
            .unwrap();
        tokio::time::timeout(Duration::from_secs(60), started_rx)
            .await
            .expect("the worker takes the job")
            .unwrap();
        let queued = pool
            .try_submit(Priority::Interactive, CancelToken::new(), |_| ())
            .unwrap();
        let refused =
            served_range_query(&pool, &caches, &served, &CancelToken::new(), |_, r, _| r).await;
        assert_eq!(
            refused.err(),
            Some(ComputeError::Submit(SubmitJobError::QueueFull))
        );
        release.send(()).unwrap();
        for receiver in [held, queued] {
            receiver.await.unwrap().unwrap();
        }
        let cancelled = CancelToken::new();
        cancelled.cancel();
        let answer = tokio::time::timeout(
            Duration::from_secs(60),
            served_range_query(&pool, &caches, &served, &cancelled, |_, r, _| r),
        )
        .await
        .expect("the pool does not stall");
        assert_eq!(answer.err(), Some(ComputeError::Job(JobError::Cancelled)));
        pool.shutdown().await.unwrap();
    }

    /// With its feature cells held, a query plans with its sources at once and generates ahead
    /// only the grid cells that plan walks; and over a grid held in part, by a smaller query about
    /// the same centre, the rest are generated ahead (P09.T40.a). Each answer is the sim's, and
    /// each finish reads every cell it walks from the cache or a cell job's walk, generating none.
    #[tokio::test]
    async fn only_what_the_caches_lack_is_generated_ahead() {
        let query = sun_like(0);
        let expected = uncached(&query, FeatureMembers::Served);
        let served = ServedQuery::new(
            shared_galaxy(),
            key(),
            query.clone(),
            FeatureMembers::Served,
        );
        let walked = with_sources(FeatureMembers::Served, |sources| {
            RangePlan::new(galaxy(), sources, &query).cells().count()
        });
        let walked = u64::try_from(walked).unwrap();
        let pool = pool(8, INTERACTIVE_QUEUE_CAPACITY);
        let warm = caches(64 << 20);
        assert_eq!(served_on(&pool, &warm, &served).await, expected);
        // The feature cells and the interiors held, the grid not.
        let grid_cold = RangeCaches::new(
            Arc::new(SharedCellCache::new(64 << 20)),
            Arc::clone(&warm.features),
            Arc::clone(&warm.interiors),
        );
        assert_eq!(served_on(&pool, &grid_cold, &served).await, expected);
        let counters = grid_cold.cells.counters();
        assert_eq!(
            (
                u64::try_from(counters.entries()).unwrap(),
                counters.hits(),
                counters.misses()
            ),
            (walked, 0, 0),
            "the cells of the plan with the sources, each generated and walked ahead"
        );
        let in_part = caches(64 << 20);
        let smaller = RangeQuery::builder(*query.centre(), LightYears::new(30.0))
            .build()
            .expect("a query in the clock window");
        let smaller = ServedQuery::new(shared_galaxy(), key(), smaller, FeatureMembers::Served);
        drop(served_on(&pool, &in_part, &smaller).await);
        let before = in_part.cells.counters();
        assert_eq!(served_on(&pool, &in_part, &served).await, expected);
        let after = in_part.cells.counters();
        let held = after.hits() - before.hits();
        assert_eq!(
            after.misses(),
            before.misses(),
            "the finish generates no cell"
        );
        assert!(
            0 < held && held < walked,
            "{held} of {walked} cells held before, the rest walked ahead"
        );
        pool.shutdown().await.unwrap();
    }

    /// The seed the tests' universes are of.
    #[test]
    fn the_fixture_is_the_server_tests_galaxy() {
        assert_eq!(shared_galaxy().seed(), Seed::new(SEED));
    }
}
