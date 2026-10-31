//! A served range query's cells, generated ahead of its walk on the CPU pool's workers
//! (P09.T40.a).
//!
//! A cold 50 ly query at the Sun-like point reads some 1,750 cells nobody has generated: twenty
//! feature cells, 5.8 ms of the query's 14.8, and 1,732 grid cells, 7.6 ms (lane measurements of
//! 2026-10-30, pod, 16 vCPU). Each is a pure function of the galaxy and its key, so they may be
//! generated in any order on any thread. [`served_range_query`] does so, in two steps of pool
//! jobs:
//!
//! 1. One job asks what the query will read that the caches lack: the feature cells the members'
//!    source looks up ([`FeatureMemberSource::feature_cells`]), each with the last process it
//!    reads it through, and then the grid cells of the query's plan ([`RangePlan::cells`]). With
//!    its feature cells held, that is the plan with the sources, as a warm query plans once;
//!    otherwise it is the plan without them, which holds every cell the plan with them walks
//!    ([`RangePlan::with_sources`]), since the sources' counts need the cells being generated. If
//!    none is worth generating ahead, it walks there and then.
//! 2. Otherwise the cells it found lacking, both kinds at once, are shared out among jobs, about
//!    three for each worker, which generate them into the shared caches, and the request waits
//!    for all of them. The next job then plans with the sources, over the warm feature cells, if
//!    the plan was made without them, and walks the warm grid.
//!
//! **No job waits on another.** The pool's queues are plain first-in, first-out, and a worker runs
//! one job to its end: a job that queued its cells and blocked on their replies would hold its
//! worker, and with every worker so held the cells would never run, so the pool would stall. So
//! every job here is queued by the request's own task, on the runtime, which awaits their replies
//! without holding a worker; a job only ever generates, asks or walks.
//!
//! **Nothing here changes an answer.** The walk reads every cell through the same caches as
//! before and generates any it still lacks, so a cell job that was cancelled or failed, a cell
//! evicted before the walk reads it, and the number of workers change only how long the answer
//! takes. The walk is [`RangePlan::run`], which is
//! [`range_query`](hyperion_sim::galaxy::query::range_query) bit for bit.

use std::sync::Arc;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord};
use hyperion_sim::galaxy::query::{RangePlan, RangeQuery, RangeResult, SystemSource};
use hyperion_sim::id::FeatureCell;

use super::{
    CancelToken, ComputeError, CpuPool, GalaxyKey, JobError, KeptInteriors, Priority,
    SharedCellCache, SharedClusterModelCache, SharedFeatureCellCache, SubmitJobError,
};
use crate::FeatureMembers;

/// The fewest feature cells worth generating ahead of a query: two, since one takes 0.1–0.7 ms,
/// well above a job's round trip (lane measurements of 2026-10-30).
const AHEAD_FEATURE_CELLS: usize = 2;

/// The fewest grid cells worth generating ahead of a query: 64, since a grid cell near the Sun
/// takes a few microseconds, and fewer would cost more in jobs' round trips than they save.
const AHEAD_GRID_CELLS: usize = 64;

/// The grid's cell jobs for each worker: two, smaller than a feature cell's, to fill the workers
/// the feature cells leave idle.
const GRID_JOBS_PER_WORKER: usize = 2;

/// The stride of the grid cells a query first asks the cell cache for, to tell a warm or a cold
/// grid without asking for every cell: 16, 109 of the Sun-like 50 ly query's 1,732 cells. Asking for
/// all of them took 0.13 ms, a tenth of the warm query (measured 2026-10-31, pod, 16 vCPU).
const LACKING_SAMPLE_STRIDE: usize = 16;

/// The share of the cell cache's budget that the grid cells generated ahead of a walk may take by
/// their estimate: a quarter, so that they are not evicted, by each other or by the walk's own
/// cells, before the walk reads them.
const AHEAD_BUDGET_SHARE: usize = 4;

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

/// The answer to `served` from `answer` over the query's result, its cells generated first on
/// `pool`'s workers into `caches` (module documentation).
///
/// `answer` runs on the job that walks the query, with the galaxy, the result and the job's kept
/// view of the interiors, which every interior the query read is kept in. The result is
/// [`range_query`](hyperion_sim::galaxy::query::range_query)'s over the same caches and sources,
/// bit for bit, at any number of workers.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the query's first job finds the interactive queue full (its later
/// jobs and its cells' wait for room, as the query has begun) or the pool shutting down or
/// faulted, and [`ComputeError::Job`] if one of the query's own jobs is cancelled, panics or is
/// lost with the pool. A cell job's failure is none of these: the walk generates what it did not.
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
    let (mut stage, mut answer, mut first) = (Stage::Ask, answer, true);
    loop {
        let (job_caches, job_served) = (caches.clone(), served.clone());
        let job = move |_: &CancelToken| prepare(&job_caches, &job_served, stage, answer);
        let receiver = if first {
            pool.try_submit(Priority::Interactive, token.clone(), job)
        } else {
            pool.submit(Priority::Interactive, token.clone(), job).await
        }
        .map_err(ComputeError::Submit)?;
        first = false;
        let prepared = receiver
            .await
            .unwrap_or_else(|closed| Err(JobError::from(closed)))
            .map_err(ComputeError::Job)?;
        match prepared {
            Prepared::Answered(answered) => return Ok(answered),
            Prepared::Lacking {
                features,
                grid,
                plan,
                planned,
                answer: back,
            } => {
                generate(pool, caches, served, (features, grid), token).await;
                (stage, answer) = (Stage::Walk(plan, planned), back);
            }
        }
    }
}

/// What a query's next job does, with what the job before it made.
enum Stage {
    /// Ask which cells the caches lack; if none is worth generating ahead, walk.
    Ask,
    /// Walk the query's plan, made with its sources first if it was made without them.
    Walk(Box<RangePlan>, Planned),
}

/// Whether a query's plan was made with its sources.
#[derive(Debug, Clone, Copy)]
enum Planned {
    /// With them: the plan is the walk's.
    WithSources,
    /// Without them: the walk plans with them first ([`RangePlan::with_sources`]).
    WithoutSources,
}

/// What a query's job hands back: its answer, or the cells it found lacking, with what it was
/// given to answer with.
enum Prepared<T, F> {
    /// The answer.
    Answered(T),
    /// The cells the caches lack.
    Lacking {
        /// The feature cells, each with the last process the query reads it through.
        features: Vec<(FeatureCell, FeatureProcess)>,
        /// The grid cells of the plan.
        grid: Vec<CellKey>,
        /// The query's plan.
        plan: Box<RangePlan>,
        /// Whether the plan was made with the query's sources.
        planned: Planned,
        /// What answers the query.
        answer: F,
    },
}

/// One job of a served query at `stage` (module documentation).
fn prepare<T, F>(
    caches: &RangeCaches,
    served: &ServedQuery,
    stage: Stage,
    answer: F,
) -> Prepared<T, F>
where
    F: FnOnce(&Galaxy, RangeResult, &KeptInteriors<'_>) -> T,
{
    let ServedQuery {
        galaxy,
        key,
        query,
        members,
    } = served;
    let (galaxy, key, members) = (galaxy.as_ref(), *key, *members);
    // One kept view for the whole job: its census, its hits and its rows' members read the same
    // interiors, each built at most once whatever the shared cache's budget.
    let (features, interiors) = (caches.features.handle(key), caches.interiors.kept(key));
    let source = FeatureMemberSource::new(&features, &interiors);
    // Held, no sources: the grid is the whole answer, as in plan 04 (P09.T40.a's gate).
    let sources: &[&dyn SystemSource] = match members {
        FeatureMembers::Held => &[],
        FeatureMembers::Served => &[&source],
    };
    let (plan, planned) = match stage {
        Stage::Walk(plan, planned) => (*plan, planned),
        Stage::Ask => {
            let lacking_features: Vec<_> = match members {
                FeatureMembers::Held => Vec::new(),
                FeatureMembers::Served => FeatureMemberSource::feature_cells(galaxy, query)
                    .into_iter()
                    .filter(|&(cell, last)| !caches.features.holds(key, cell, last))
                    .collect(),
            };
            // With too few feature cells to generate ahead, the sources' counts are as cheap now
            // as after, so the query plans once, as a warm one did before P09.T40.a.
            let (plan, planned) = if lacking_features.len() < AHEAD_FEATURE_CELLS {
                (RangePlan::new(galaxy, sources, query), Planned::WithSources)
            } else {
                (RangePlan::new(galaxy, &[], query), Planned::WithoutSources)
            };
            let lacking_grid = lacking_grid_cells(&caches.cells, key, &plan);
            if lacking_features.len() >= AHEAD_FEATURE_CELLS
                || lacking_grid.len() >= AHEAD_GRID_CELLS
            {
                return Prepared::Lacking {
                    features: lacking_features,
                    grid: lacking_grid,
                    plan: Box::new(plan),
                    planned,
                    answer,
                };
            }
            (plan, planned)
        }
    };
    let plan = match planned {
        Planned::WithSources => plan,
        Planned::WithoutSources => plan.with_sources(galaxy, sources),
    };
    let result = plan.run(galaxy, &mut caches.cells.handle(key), sources);
    Prepared::Answered(answer(galaxy, result, &interiors))
}

/// The grid cells `plan` walks that `cells` lacks, in walk order, or none if by their estimate
/// they would take more than [`AHEAD_BUDGET_SHARE`]'s share of the cache's budget, and so might
/// be evicted before the walk reads them.
///
/// The cache is first asked for every [`LACKING_SAMPLE_STRIDE`]th cell. If it holds them all, the
/// grid is taken as warm and none is lacking; if it lacks them all, the grid is taken as cold and
/// every cell is; only between the two is it asked for the rest. A cell taken as lacking that the
/// cache holds is generated again, and one taken as held that it lacks is the walk's to generate,
/// as every cell was before P09.T40.a: either costs time, never an answer.
///
/// A cell's records are estimated from its layer's expected count over the query's sphere,
/// spread over the cell's volume at the sphere's mean density.
fn lacking_grid_cells(cells: &SharedCellCache, key: GalaxyKey, plan: &RangePlan) -> Vec<CellKey> {
    let walked: Vec<_> = plan.cells().collect();
    let sample = cells.lacking(key, walked.iter().step_by(LACKING_SAMPLE_STRIDE).copied());
    if sample.is_empty() {
        return Vec::new();
    }
    let lacking = if sample.len() == walked.len().div_ceil(LACKING_SAMPLE_STRIDE) {
        walked
    } else {
        cells.lacking(key, walked)
    };
    let census = plan.census();
    let radius = plan.query().radius().value();
    let sphere = 4.0 / 3.0 * core::f64::consts::PI * radius * radius * radius;
    let records = lacking.iter().fold(0.0, |sum, cell| {
        let edge = f64::from(cell.size_ly());
        sum + census.expected().get(cell.layer()) * edge * edge * edge / sphere
    });
    #[expect(
        clippy::cast_precision_loss,
        reason = "a cache budget is far below 2^53 bytes, and this is an estimate"
    )]
    let room = (cells.budget_bytes() / AHEAD_BUDGET_SHARE) as f64;
    #[expect(clippy::cast_precision_loss, reason = "a record is some tens of bytes")]
    let bytes = records * size_of::<SystemRecord>() as f64;
    if bytes <= room { lacking } else { Vec::new() }
}

/// `items` shared out among `jobs` jobs, but never more jobs than items, each job taking every so
/// many in turn so that each kind of cell is spread across them.
fn shares<T: Copy>(jobs: usize, items: &[T]) -> Vec<Vec<T>> {
    let jobs = jobs.min(items.len());
    (0..jobs)
        .map(|first| items.iter().skip(first).step_by(jobs).copied().collect())
        .collect()
}

/// Generates the feature cells `features`, each through its last process, with the grid reach
/// bounds and the interiors of their features that the query reads
/// ([`FeatureMemberSource::read_ahead`]), and the grid cells `grid`, into `caches` on `pool`'s
/// workers, and waits for every job it queued.
///
/// A feature cell read through more processes costs more, a whole one near the Sun some 0.65 ms
/// against 0.1 ms through the clusters alone, so they are dealt out from the fullest, one job for
/// each worker, and queued first; the grid cells, some microseconds each, follow in twice as many
/// jobs, which fill the workers the feature cells leave idle. A job's grid cells are stored under
/// one hold of the cache's lock ([`SharedCellCache::generate_all`]).
///
/// A job cancelled or failed is passed over, and a pool shutting down or faulted stops the
/// queueing: the cells are then left to the query's walk, which generates any it lacks and reports
/// a failure of its own, so that no answer depends on these jobs.
async fn generate(
    pool: &CpuPool,
    caches: &RangeCaches,
    served: &ServedQuery,
    (mut features, grid): (Vec<(FeatureCell, FeatureProcess)>, Vec<CellKey>),
    token: &CancelToken,
) {
    let (key, workers) = (served.key, pool.workers().get());
    features.sort_by_key(|&(cell, last)| (std::cmp::Reverse(last.index()), cell));
    let feature_jobs = shares(workers, &features).into_iter().map(|share| {
        let (caches, served) = (caches.clone(), served.clone());
        Box::new(move |_: &CancelToken| {
            let (cells, interiors) = (caches.features.handle(key), caches.interiors.handle(key));
            let source = FeatureMemberSource::new(&cells, &interiors);
            for cell in share {
                source.read_ahead(&served.galaxy, &served.query, cell);
            }
        }) as Box<dyn FnOnce(&CancelToken) + Send>
    });
    let grid_jobs = shares(GRID_JOBS_PER_WORKER * workers, &grid)
        .into_iter()
        .map(|share| {
            let (cache, galaxy) = (Arc::clone(&caches.cells), Arc::clone(&served.galaxy));
            Box::new(move |_: &CancelToken| cache.generate_all(key, &galaxy, share))
                as Box<dyn FnOnce(&CancelToken) + Send>
        });
    let mut queued = Vec::new();
    for job in feature_jobs.chain(grid_jobs) {
        // A full queue is waited on, as the query has begun.
        match pool.submit(Priority::Interactive, token.clone(), job).await {
            Ok(receiver) => queued.push(receiver),
            // The query's next job is refused too, and reports it.
            Err(SubmitJobError::QueueFull | SubmitJobError::ShutDown | SubmitJobError::Faulted) => {
                break;
            }
        }
    }
    for receiver in queued {
        // Its cells are held now, or left to the walk, as the documentation says.
        let _ = receiver.await;
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
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
    /// eight, with the members served and held and away from the epoch; and cold, its walk
    /// finds every grid cell it reads generated ahead of it, by the plan without the sources,
    /// whose cells hold the plan's with them (P09.T40.a). One worker runs every cell job and then
    /// the walk in turn, so no job ever waits on another.
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
                    (u64::try_from(walked.len()).unwrap(), 0),
                    "the walk finds every cell it reads"
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

    /// With its feature cells held, a query plans with its sources at once and generates ahead
    /// only the grid cells that plan walks; and over a grid held in part, by a smaller query about
    /// the same centre, the rest are generated ahead (P09.T40.a). Each answer is the sim's, and
    /// each walk finds every cell it reads.
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
            (walked, walked, 0),
            "the cells of the plan with the sources, each generated ahead and found by the walk"
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
        assert_eq!(
            (
                after.hits() - before.hits(),
                after.misses() - before.misses()
            ),
            (walked, 0),
            "the walk finds every cell, held before or generated ahead"
        );
        pool.shutdown().await.unwrap();
    }

    /// The seed the tests' universes are of.
    #[test]
    fn the_fixture_is_the_server_tests_galaxy() {
        assert_eq!(shared_galaxy().seed(), Seed::new(SEED));
    }
}
