//! A served range query's cells, generated and walked ahead of its finish on the CPU pool's
//! workers (P09.T40.a).
//!
//! A cold 50 ly query at the Sun-like point reads some 1,750 cells nobody has generated: twenty
//! feature cells, 5.8 ms of the query's 14.8, and 1,732 grid cells, 7.6 ms (lane measurements of
//! 2026-10-30, pod, 16 vCPU). From generator version 27 it reads some 8,700 catalogue-class cells
//! besides, 105 ms of the 190 under every 27 hold, in which the grid takes 72 (measured
//! 2026-11-01, pod, 16 vCPU). Each is a pure function of the galaxy and its key, so they may be
//! generated in any order on any thread, and each grid cell's walk with them
//! ([`RangePlan::walk_cells`]). [`served_range_query`] does so, in three steps of pool jobs:
//!
//! 1. One job asks what the query will read that the caches lack: the feature cells the members'
//!    source looks up ([`FeatureMemberSource::feature_cells`]), each with the last process it
//!    reads it through, then the grid cells of the query's plan ([`RangePlan::cells`]), and the
//!    class cells the classes' hits read in its layers ([`CatalogueClassSource::cells_read`]).
//!    With its feature cells held, that is the plan with the sources, as a warm query plans once;
//!    otherwise it is the plan without them, which holds every cell and layer the plan with them
//!    walks ([`RangePlan::with_sources`]), since the sources' counts need the cells being
//!    generated. If none is worth generating ahead, it walks there and then, without listing the
//!    class cells: a query whose feature cells are held and whose grid is warm was asked before,
//!    with its class cells.
//! 2. Otherwise the cells it found lacking, every kind at once, are shared out among jobs, about
//!    three for each worker: the sources' jobs read the feature cells and make the class cells
//!    into the shared caches, and the grid's generate theirs into the cell cache and walk each as
//!    it is made. Once the sources' cell jobs end, one more job plans the query with its sources,
//!    over the warm feature cells, if it was planned without them, and asks the sources for their
//!    hits over the warm class cells ([`RangePlan::source_hits`]), while the last of the grid's
//!    jobs run.
//! 3. The last job merges the grid's walked parts and the sources' hits into the answer
//!    ([`RangePlan::finish`]).
//!
//! **Layer E is read in part.** A layer-E cell the cell cache lacks is not generated whole: its
//! walk reads only the systems that can reach the sphere, and counts the rest (P08.T12), since the
//! exact fates and lifetimes of its records were most of a cold query's grid, 85 of 110 ms in one
//! thread at the Sun-like point, where 9 of its 384 records are hits; read in part its eight cells
//! take some 18 ms, the grid 43 (measured 2026-11-05, pod, 16 vCPU). So any such cell is worth
//! walking ahead, and is: its walk finds the same hits, and stores nothing, every job reading the
//! cell cache through a handle that lends no layer-E cell it lacks
//! ([`SharedCellCache::handle_reading_in_part`]). Once the query is answered, background jobs
//! ([`Priority::Background`]), a cell each up to [`KEEP_WHOLE_JOBS`] of them, generate those cells
//! whole into the cell cache, off every query's path, so that the queries after it near the same
//! place find them held and walk them from their records, as a warm query did before.
//!
//! **No job waits on another.** The pool's queues are plain first-in, first-out, and a worker runs
//! one job to its end: a job that queued its cells and blocked on their replies would hold its
//! worker, and with every worker so held the cells would never run, so the pool would stall. So
//! every job here is queued by the request's own task, on the runtime, which awaits their replies
//! without holding a worker; a job only ever generates, asks, plans or walks. The sources' job is
//! queued only once the sources' cell jobs have ended, or beside the grid's if there are none, so
//! a query never holds more than three of the queue's places for each worker, as its cells' jobs
//! did before it.
//!
//! **Nothing here changes an answer.** The finish is [`RangePlan::finish`], which is
//! [`range_query`](hyperion_sim::galaxy::query::range_query) bit for bit whatever parts it is
//! given: it walks through the shared caches any cell no part holds, and plans with the sources
//! and asks them itself if their job failed. So a cell job that was cancelled or failed, a cell
//! evicted, and the number of workers change only how long the answer takes.

use std::cmp::Reverse;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCellCache, CatalogueClassSource,
};
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::centre::CentreOrbitCache;
use hyperion_sim::galaxy::features::members::source::{CellPart, FeatureMemberSource};
use hyperion_sim::galaxy::global_list::{GlobalListSource, TubeLookup};
use hyperion_sim::galaxy::placement::CellKey;
use hyperion_sim::galaxy::query::{
    RangePlan, RangeQuery, RangeResult, SourceHits, SystemSource, WalkedCells,
};
use hyperion_sim::id::{FeatureCell, Layer};

use super::{
    CancelToken, CentreOrbitHandle, ComputeError, CpuPool, GalaxyKey, JobError, JobReceiver,
    KeptInteriors, ListedTubes, Priority, SharedCellCache, SharedCentreOrbitCache,
    SharedClassCellCache, SharedClusterModelCache, SharedFeatureCellCache, SubmitJobError,
};
use crate::FeatureMembers;
use crate::convert::MotionReads;

/// The fewest feature cells worth generating ahead of a query: two, since one takes 0.1–0.7 ms,
/// well above a job's round trip (lane measurements of 2026-10-30).
const AHEAD_FEATURE_CELLS: usize = 2;

/// The fewest grid cells worth generating ahead of a query: 64, some 0.28 ms of cells at the 4.4 µs
/// a grid cell near the Sun takes (1,732 in 7.6 ms, lane measurements of 2026-10-30), where a
/// round of jobs costs some tens of microseconds; fewer would cost more than they save.
const AHEAD_GRID_CELLS: usize = 64;

/// The most parts a feature cell is read ahead in ([`FeatureMemberSource::read_ahead_part`]): four.
/// Four of the Sun-like 50 ly query's twenty feature cells hold nearly all the features whose
/// lists it reads, some 55 ms each under every 27 hold, which kept their jobs the query's longest.
const FEATURE_CELL_PARTS: u32 = 4;

/// The grid's cell jobs for each worker: two, smaller than a feature cell's, to fill the workers
/// the feature cells leave idle. Two to six took the Sun-like 50 ly query the same time cold, 4.6
/// to 5.2 ms on 8 workers, and one some 0.3 ms longer (measured 2026-10-31, pod, 16 vCPU), so it is
/// the fewest of the queue's places that does as well.
const GRID_JOBS_PER_WORKER: usize = 2;

/// The stride of the grid cells a query first asks the cell cache for, to tell a warm or a cold
/// grid without asking for every cell: 16, 109 of the Sun-like 50 ly query's 1,732 cells. Asking for
/// all of them took 0.13 ms, a tenth of the warm query (measured 2026-10-31, pod, 16 vCPU).
const LACKING_SAMPLE_STRIDE: usize = 16;

/// The most background jobs a query queues to keep the layer-E cells it read in part
/// ([`keep_whole`]): 64, a quarter of the background queue, which is as deep as the bulk queue
/// ([`BULK_QUEUE_CAPACITY`](crate::limits::BULK_QUEUE_CAPACITY)), so that the 412 cells of a 500 ly
/// query about the Sun-like point are all kept, some seven a job, and three such queries at once
/// leave room for the surface prefetch's passes.
const KEEP_WHOLE_JOBS: usize = 64;

/// The caches a served range query reads: the grid's cells, the feature cells, the features'
/// interiors, the catalogue classes' cells and the galactic centre's orbits, shared with the jobs
/// that generate cells for it.
#[derive(Debug, Clone)]
pub struct RangeCaches {
    cells: Arc<SharedCellCache>,
    features: Arc<SharedFeatureCellCache>,
    interiors: Arc<SharedClusterModelCache>,
    classes: Arc<SharedClassCellCache>,
    centre: Arc<SharedCentreOrbitCache>,
}

impl RangeCaches {
    /// The caches a query reads, as the server holds them: the grid's cells, the feature cells,
    /// the features' interiors, the catalogue classes' cells (P09.T40) and the galactic centre's
    /// cells of members and their orbits (P09.T37.b).
    #[must_use]
    pub fn new(
        cells: Arc<SharedCellCache>,
        features: Arc<SharedFeatureCellCache>,
        interiors: Arc<SharedClusterModelCache>,
        (classes, centre): (Arc<SharedClassCellCache>, Arc<SharedCentreOrbitCache>),
    ) -> Self {
        Self {
            cells,
            features,
            interiors,
            classes,
            centre,
        }
    }
}

/// One served range query: the galaxy its universe names and that galaxy's key, the query,
/// whether the features' members are served beside the grid, and the galaxy's global list with its
/// tube tables if the query takes the list's members.
#[derive(Debug, Clone)]
pub struct ServedQuery {
    galaxy: Arc<Galaxy>,
    key: GalaxyKey,
    query: RangeQuery,
    members: FeatureMembers,
    list: Option<ListedTubes>,
}

impl ServedQuery {
    /// `query` in `galaxy`, the galaxy `key` names, with the feature members `members` says, and
    /// without the global list's.
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
            list: None,
        }
    }

    /// The same query reading the global list through `list`, the galaxy's list and tube tables:
    /// from generator version 27 the list's members, the centre's among them, are served beside
    /// the grid in place of the centre's bare source (P10.T10.b's registration, P09.T37.b), and
    /// below it `list` is read for nothing (the crate's `beside_the_grid`).
    ///
    /// The list is built on the pool before the query, since a job never waits on another, so the
    /// caller takes it from the [`GlobalListService`](super::GlobalListService) first, with the
    /// tables of the streams the query reads ([`GlobalListSource::streams_read_with`]).
    #[must_use]
    pub fn with_list(self, list: ListedTubes) -> Self {
        Self {
            list: Some(list),
            ..self
        }
    }
}

/// What a served query's rows are converted with beyond its hits: the features' interiors, kept
/// for the whole job, the galactic centre's cells of members and their orbits, and the global
/// list's source if the query took it, which resolves the list's members' rows (P10.T10.b).
pub struct RowReads<'a> {
    interiors: &'a KeptInteriors<'a>,
    orbits: &'a CentreOrbitHandle<'a>,
    list: Option<&'a GlobalListSource<'a>>,
}

impl<'a> RowReads<'a> {
    /// The features' interiors the query's members are resolved through.
    #[must_use]
    pub fn interiors(&self) -> &'a KeptInteriors<'a> {
        self.interiors
    }

    /// The global list's source the query took, if it took one.
    #[must_use]
    pub fn list(&self) -> Option<&'a GlobalListSource<'a>> {
        self.list
    }

    /// The centre's orbits the query read, its centre members' paths found through.
    #[must_use]
    pub fn orbits(&self) -> &'a CentreOrbitHandle<'a> {
        self.orbits
    }

    /// What the rows' systems are placed and observed through: the interiors, the centre's
    /// orbits and the list's source, the ones the query read.
    #[must_use]
    pub(crate) fn motion(&self) -> MotionReads<'a> {
        let reads = MotionReads::new(self.interiors, self.orbits);
        match self.list {
            Some(list) => reads.with_list(list),
            None => reads,
        }
    }
}

impl std::fmt::Debug for RowReads<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowReads")
            .field("list", &self.list.is_some())
            .finish_non_exhaustive()
    }
}

/// What a range query in a galaxy reads its global list through: the galaxy's list and its tube
/// tables, and the galactic centre's cells of members and their orbits (P10.T10.b, P09.T37.b).
#[derive(Clone, Copy)]
pub(crate) struct ListReads<'a> {
    tubes: &'a dyn TubeLookup,
    orbits: &'a dyn CentreOrbitCache,
}

impl<'a> ListReads<'a> {
    /// The list and tables of `tubes`, and the centre's orbits through `orbits`.
    #[must_use]
    pub(crate) fn new(tubes: &'a dyn TubeLookup, orbits: &'a dyn CentreOrbitCache) -> Self {
        Self { tubes, orbits }
    }
}

/// The answer to `served` from `answer` over the query's result, its cells generated and walked
/// first on `pool`'s workers into `caches` (module documentation).
///
/// `answer` runs on the job that finishes the query, with the galaxy, the result and what its rows
/// are converted with: the job's kept view of the interiors and the global list's source, if the
/// query took it ([`RowReads`]). The result is
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
    F: FnOnce(&Galaxy, RangeResult, &RowReads<'_>) -> T + Send + 'static,
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
        sources,
        grid,
        plan,
        planned,
        answer,
    } = match reply(asked).await? {
        Asked::Answered(answered) => return Ok(answered),
        Asked::Lacking(lacking) => *lacking,
    };
    let read_in_part: Vec<CellKey> = grid
        .iter()
        .flatten()
        .copied()
        .filter(|cell| cell.layer() == Layer::E)
        .collect();
    let ahead = ahead(
        pool,
        caches,
        served,
        (Arc::new(plan), planned),
        (sources, grid),
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
    let answered = reply(finished).await?;
    keep_whole(pool, caches, served, &read_in_part);
    Ok(answered)
}

/// Queues in the background `cells`, the layer-E cells a query's walk read in part, to be generated
/// whole into the cell cache ([`SharedCellCache::generate_whole`]; module documentation): a job a
/// cell, so that one cell's generation is the most a request that finds every worker busy waits for
/// one, as it waited for one before P08.T12, but never more jobs than [`KEEP_WHOLE_JOBS`], each
/// taking every so many cells in turn. A queue that is full, or a pool shutting down, leaves the
/// rest to be read in part again by the next query: only time is lost.
fn keep_whole(pool: &CpuPool, caches: &RangeCaches, served: &ServedQuery, cells: &[CellKey]) {
    for share in shares(KEEP_WHOLE_JOBS, cells) {
        let (cache, galaxy, key) = (
            Arc::clone(&caches.cells),
            Arc::clone(&served.galaxy),
            served.key,
        );
        let job = move |_: &CancelToken| cache.generate_whole(key, &galaxy, share);
        match pool.try_submit(Priority::Background, CancelToken::new(), job) {
            // Nobody waits on the reply: the cell is the cache's once made.
            Ok(_) => {}
            // The query is answered: a queue full or a pool shutting down costs the next query
            // the cells' reading in part again, and a pool faulted refuses that query itself.
            Err(SubmitJobError::QueueFull | SubmitJobError::ShutDown | SubmitJobError::Faulted) => {
                break;
            }
        }
    }
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

/// One job's feature cells, each with the last process the query reads it through and the part of
/// its features the job reads ([`FeatureMemberSource::read_ahead_part`]).
type FeatureShare = Vec<((FeatureCell, FeatureProcess), CellPart)>;

/// One job's share of the cells the query's sources read: its feature cells and its class cells.
#[derive(Debug, Default)]
struct SourceCells {
    features: FeatureShare,
    classes: Vec<CatalogueCellKey>,
}

/// The cells the caches lack for a query, dealt out among the jobs that generate them, and what
/// the query is then walked and answered with.
struct Lacking<F> {
    /// The sources' cells, one share for each job: the feature cells, the fullest dealt first,
    /// and the class cells, in the order they are read, dealt out in turn.
    sources: Vec<SourceCells>,
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

/// `f` of the feature member source over `caches` and the sources it is beside the grid as
/// ([`beside_the_grid`]), with what the query's rows are converted with ([`RowReads`]).
///
/// One kept view for the whole job: its census, its hits and its rows' members read the same
/// interiors, each built at most once whatever the shared cache's budget.
fn with_sources<R>(
    caches: &RangeCaches,
    served: &ServedQuery,
    f: impl FnOnce(&[&dyn SystemSource], &RowReads<'_>) -> R,
) -> R {
    let (features, interiors) = (
        caches.features.handle(served.key),
        caches.interiors.kept(served.key),
    );
    let source = FeatureMemberSource::new(&features, &interiors);
    let classes = caches.classes.handle(served.key);
    let orbits = caches.centre.handle(served.key);
    let list = served
        .list
        .as_ref()
        .map(|tubes| ListReads::new(tubes, &orbits));
    beside_the_grid(
        &served.galaxy,
        served.members,
        (&source, &classes),
        list,
        |sources, list| {
            f(
                sources,
                &RowReads {
                    interiors: &interiors,
                    orbits: &orbits,
                    list,
                },
            )
        },
    )
}

/// `f` of the sources `served` takes beside the grid over `caches`, as each of its jobs takes
/// them: for a test that asks them for their hits alone.
#[cfg(test)]
pub(crate) fn served_sources<R>(
    caches: &RangeCaches,
    served: &ServedQuery,
    f: impl FnOnce(&[&dyn SystemSource]) -> R,
) -> R {
    with_sources(caches, served, |sources, _| f(sources))
}

/// `f` of what `served`'s rows are converted with over `caches` ([`RowReads`]), as the jobs that
/// answered it had them: for a job of the request's after the query's own, which converts its rows
/// in chunks.
pub(crate) fn with_row_reads<R>(
    caches: &RangeCaches,
    served: &ServedQuery,
    f: impl FnOnce(&RowReads<'_>) -> R,
) -> R {
    with_sources(caches, served, |_, reads| f(reads))
}

/// `f` of the sources a range query in `galaxy` takes beside the grid as `members` says, and of
/// the global list's source among them: held, none, and the grid is the whole answer, as in plan
/// 04 (P09.T40.a's gate); served, the features' members (`sources`' first); from generator version
/// 27 the global list's members, the galactic centre's among them, beside them, read through `list`
/// if the caller took the list ([`GlobalListSource::is_registered_in`], P10.T10.b's registration
/// and P09.T37.b), in place of the centre's bare source and never beside it, since their IDs would
/// repeat; and from 27 the catalogue classes' hosts ([`CatalogueClassSource::registered_in`],
/// P09.T37.a). Below 27 there are neither, and `list` is read for nothing.
///
/// The classes' source reads its class cells through `sources`' second (P09.T40): a cold query
/// makes each cell it reads on the job that asks the sources for their hits, and a warm one finds
/// them held. The list's source reads its tube tables and the centre's orbits through `list`.
///
/// # Panics
///
/// If the galaxy's centre cannot be built ([`GlobalListSource::new`]), which no galaxy the server
/// builds reaches, as [`Galaxy::centre`]'s documentation says.
pub(crate) fn beside_the_grid<R>(
    galaxy: &Galaxy,
    members: FeatureMembers,
    (features, class_cells): (&dyn SystemSource, &dyn CatalogueClassCellCache),
    list: Option<ListReads<'_>>,
    f: impl FnOnce(&[&dyn SystemSource], Option<&GlobalListSource<'_>>) -> R,
) -> R {
    if members == FeatureMembers::Held {
        return f(&[], None);
    }
    let classes =
        CatalogueClassSource::registered_in(galaxy).map(|classes| classes.with_cells(class_cells));
    let listed = list
        .filter(|_| GlobalListSource::is_registered_in(galaxy))
        .map(|reads| {
            GlobalListSource::new(galaxy, reads.tubes, reads.orbits)
                .expect("a galaxy the server builds has a centre that builds")
        });
    match (&listed, &classes) {
        (None, None) => f(&[features], None),
        (None, Some(classes)) => f(&[features, classes], None),
        (Some(listed), None) => f(&[features, listed], Some(listed)),
        (Some(listed), Some(classes)) => f(&[features, listed, classes], Some(listed)),
    }
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
    F: FnOnce(&Galaxy, RangeResult, &RowReads<'_>) -> T,
{
    let (galaxy, key, query) = (served.galaxy.as_ref(), served.key, &served.query);
    with_sources(caches, served, |sources, reads| {
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
        // sources, and so is the walk's. Such a query, its feature cells held and its grid warm,
        // was asked before, and its class cells with it, so they are not listed: some 8,700 about
        // the Sun-like point, 0.28 ms of a warm query's 4.5 (measured 2026-11-01, pod, 16 vCPU).
        // Any its hits lack, evicted since, they make. A layer-E cell lacking is worth walking
        // ahead alone, some 2.2 ms read in part at the Sun-like point (module documentation).
        if let Planned::WithSources = planned
            && grid.len() < AHEAD_GRID_CELLS
            && grid.iter().all(|cell| cell.layer() != Layer::E)
        {
            let result = plan.run(
                galaxy,
                &mut caches.cells.handle_reading_in_part(key),
                sources,
            );
            return Asked::Answered(answer(galaxy, result, reads));
        }
        let classes = lacking_class_cells(caches, served, &plan);
        // A feature cell read through more processes costs more, a whole one near the Sun some
        // 0.65 ms against 0.1 ms through the clusters alone, so they are dealt out from the
        // fullest, each in parts, since a few hold nearly all the features whose lists are read.
        features.sort_by_key(|&(cell, last)| (Reverse(last.index()), cell));
        let parts = feature_parts(workers);
        let features: FeatureShare = features
            .into_iter()
            .flat_map(|cell| CellPart::all(parts).map(move |part| (cell, part)))
            .collect();
        let mut features = shares(workers.get(), &features).into_iter();
        let mut classes = shares(workers.get(), &classes).into_iter();
        let source_cells = std::iter::from_fn(|| match (features.next(), classes.next()) {
            (None, None) => None,
            (features, classes) => Some(SourceCells {
                features: features.unwrap_or_default(),
                classes: classes.unwrap_or_default(),
            }),
        })
        .collect();
        Asked::Lacking(Box::new(Lacking {
            sources: source_cells,
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
    F: FnOnce(&Galaxy, RangeResult, &RowReads<'_>) -> T,
{
    let (galaxy, key) = (served.galaxy.as_ref(), served.key);
    let Ahead { parts, sourced } = ahead;
    with_sources(caches, served, |sources, reads| {
        let (plan, hits) = match sourced {
            Sourced::Made(made) => *made,
            Sourced::Left(plan, planned) => {
                let plan = with_its_sources(galaxy, plan, planned, sources);
                let hits = plan.source_hits(galaxy, sources);
                (plan, hits)
            }
        };
        let cells = &mut caches.cells.handle_reading_in_part(key);
        let result = plan.finish(galaxy, cells, sources, &parts, hits);
        answer(galaxy, result, reads)
    })
}

/// The parts each feature cell is read ahead in on a pool of `workers` workers: as many as the
/// workers, to [`FEATURE_CELL_PARTS`].
#[must_use]
fn feature_parts(workers: NonZeroUsize) -> NonZeroU32 {
    let workers = u32::try_from(workers.get()).unwrap_or(u32::MAX);
    NonZeroU32::new(workers.min(FEATURE_CELL_PARTS)).expect("a pool has a worker")
}

/// The grid cells `plan` walks that `cells` lacks, in walk order: layer E's each asked for, since
/// one lacking is read in part and kept whole only if it is found lacking (module documentation),
/// and the rest sampled ([`sampled_lacking`]), one taken as held that the cache lacks being the
/// finish's to walk, as every cell was before P09.T40.a. Layer E is walked first, so the order
/// holds.
fn lacking_grid_cells(cells: &SharedCellCache, key: GalaxyKey, plan: &RangePlan) -> Vec<CellKey> {
    let (layer_e, rest): (Vec<_>, Vec<_>) = plan.cells().partition(|cell| cell.layer() == Layer::E);
    let mut lacking = cells.lacking(key, layer_e);
    lacking.extend(sampled_lacking(rest, |asked| cells.lacking(key, asked)));
    lacking
}

/// The class cells `served`'s sources' hits read in `plan`'s layers
/// ([`CatalogueClassSource::cells_read`]) that the class cell cache lacks, in the order they are
/// read, sampled ([`sampled_lacking`]); none where the query takes no class. One taken as held that
/// the cache lacks is made by the hits, as every class cell was before.
fn lacking_class_cells(
    caches: &RangeCaches,
    served: &ServedQuery,
    plan: &RangePlan,
) -> Vec<CatalogueCellKey> {
    let classes = match served.members {
        FeatureMembers::Held => None,
        FeatureMembers::Served => CatalogueClassSource::registered_in(&served.galaxy),
    };
    classes.map_or_else(Vec::new, |classes| {
        let read = classes.cells_read(&served.galaxy, &served.query, plan.census().layers());
        sampled_lacking(read, |asked| caches.classes.lacking(served.key, asked))
    })
}

/// The cells of `read` that a cache lacks, in order, as `lacking` asks it for the cells it is
/// given.
///
/// The cache is first asked for every [`LACKING_SAMPLE_STRIDE`]th cell. If it holds them all, the
/// cells are taken as warm and none is lacking; if it lacks them all, they are taken as cold and
/// every cell is; only between the two is it asked for the rest. A cell taken as lacking that the
/// cache holds is generated again, and one taken as held that it lacks is generated where it is
/// read: either costs time, never an answer.
fn sampled_lacking<T: Copy>(
    read: Vec<T>,
    lacking: impl Fn(&mut dyn Iterator<Item = T>) -> Vec<T>,
) -> Vec<T> {
    let sample = lacking(&mut read.iter().copied().step_by(LACKING_SAMPLE_STRIDE));
    if sample.is_empty() {
        Vec::new()
    } else if sample.len() == read.len().div_ceil(LACKING_SAMPLE_STRIDE) {
        read
    } else {
        lacking(&mut read.into_iter())
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
/// `source_cells`, its feature cells read through their processes, with the grid reach bounds and the
/// interiors of their features that the query reads ([`FeatureMemberSource::read_ahead`]), and its
/// class cells made into the class cell cache ([`SharedClassCellCache::read_ahead`]); each share
/// of `grid` generated into the cell cache and walked with `plan`
/// ([`SharedCellCache::walk_generated`]); and, once the sources' cell jobs end, the plan with the
/// sources and their hits. It waits for every job it queued.
///
/// The sources' cell jobs, one for each worker, are queued first; the grid's, some microseconds a
/// cell, follow in twice as many jobs, which fill the workers the sources' cells leave idle; and
/// the sources' job, queued once the sources' cell jobs have left the queue, runs beside the last
/// of the grid's. A query without sources' cells to wait for queues it ahead of the grid's jobs. A
/// job cancelled or failed is passed over: what it did not do is the finish's, or its hits'.
async fn ahead(
    pool: &CpuPool,
    caches: &RangeCaches,
    served: &ServedQuery,
    (plan, planned): (Arc<RangePlan>, Planned),
    (source_cells, grid): (Vec<SourceCells>, Vec<Vec<CellKey>>),
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
    let mut cell_jobs = Vec::new();
    for share in source_cells {
        let (caches, served) = (caches.clone(), served.clone());
        let job = move |_: &CancelToken| {
            let (cells, interiors) = (caches.features.handle(key), caches.interiors.handle(key));
            let source = FeatureMemberSource::new(&cells, &interiors);
            for (cell, part) in share.features {
                source.read_ahead_part(&served.galaxy, &served.query, cell, part);
            }
            caches
                .classes
                .read_ahead(key, &served.galaxy, &share.classes);
        };
        cell_jobs.extend(queue(pool, token, job).await);
    }
    let mut asking = None;
    if asks_sources && cell_jobs.is_empty() {
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
    if !cell_jobs.is_empty() {
        for receiver in cell_jobs {
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
    use hyperion_sim::galaxy::catalogue_classes::NoClassCellCache;
    use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
    use hyperion_sim::galaxy::features::members::NoInteriorCache;
    use hyperion_sim::galaxy::placement::NoCache;
    use hyperion_sim::galaxy::query::range_query;
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::units::LightYears;
    use hyperion_sim::{GENERATOR_VERSION, Seed};

    use super::*;
    use crate::compute::BulkGate;
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

    /// A pool of `workers` whose bulk and background jobs `gate` may hold, with the server's room
    /// for queued jobs.
    fn gated_pool(workers: usize, gate: &BulkGate) -> CpuPool {
        CpuPool::with_bulk_gate(
            NonZeroUsize::new(workers).expect("at least one worker"),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
            gate.clone(),
        )
        .expect("the pool starts")
    }

    /// Returns once every job `pool`, of `workers` workers, took before it has ended: one
    /// background job a worker at the back of the queue, each held until all are taken, so that
    /// each worker is through what it took before them, the background jobs that keep a query's
    /// layer-E cells whole among them.
    async fn drained(pool: &CpuPool, workers: usize) {
        let barrier = Arc::new(std::sync::Barrier::new(workers));
        let mut held = Vec::with_capacity(workers);
        for _ in 0..workers {
            let barrier = Arc::clone(&barrier);
            let job = move |_: &CancelToken| {
                barrier.wait();
            };
            held.push(
                pool.submit(Priority::Background, CancelToken::new(), job)
                    .await
                    .expect("room for the job"),
            );
        }
        for receiver in held {
            tokio::time::timeout(Duration::from_secs(60), receiver)
                .await
                .expect("the pool does not stall")
                .expect("the job replies")
                .expect("the job runs");
        }
    }

    /// Fresh caches of `bytes` each.
    fn caches(bytes: usize) -> RangeCaches {
        RangeCaches::new(
            Arc::new(SharedCellCache::new(bytes)),
            Arc::new(SharedFeatureCellCache::new(bytes)),
            Arc::new(SharedClusterModelCache::new(bytes)),
            (
                Arc::new(SharedClassCellCache::new(bytes)),
                Arc::new(SharedCentreOrbitCache::new(bytes)),
            ),
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
        beside_the_grid(
            galaxy(),
            members,
            (&source, &NoClassCellCache),
            None,
            |sources, _| f(sources),
        )
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
    /// The cell jobs read layer E's cells in part and store none of them; once the query is
    /// answered, background jobs, held here until it is, keep them whole, and the warm query finds
    /// them held (P08.T12).
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
            let layer_e = ahead.iter().filter(|cell| cell.layer() == Layer::E).count();
            assert!(layer_e > 0, "the query reads layer E");
            for workers in [1, 8] {
                let gate = BulkGate::default();
                let pool = gated_pool(workers, &gate);
                let caches = caches(64 << 20);
                gate.close();
                assert_eq!(served_on(&pool, &caches, &served).await, expected, "cold");
                let cold = caches.cells.counters();
                assert_eq!(
                    cold.entries(),
                    ahead.len() - layer_e,
                    "every cell generated ahead, once, but layer E's, read in part"
                );
                assert_eq!(
                    (cold.hits(), cold.misses()),
                    (0, 0),
                    "the finish takes every cell it reads from a cell job's walk"
                );
                gate.open();
                drained(&pool, workers).await;
                let kept = caches.cells.counters();
                assert_eq!(
                    (kept.entries(), kept.hits(), kept.misses()),
                    (ahead.len(), 0, 0),
                    "layer E's cells kept whole in the background"
                );
                assert_eq!(served_on(&pool, &caches, &served).await, expected, "warm");
                assert_eq!(
                    caches.cells.counters().misses(),
                    0,
                    "the warm query finds every cell held"
                );
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
    /// only the grid cells that plan walks, layer E's read in part and kept whole after the answer
    /// (P08.T12); and over a grid held in part, by a smaller query about the same centre, the rest
    /// are generated ahead (P09.T40.a). Each answer is the sim's, and each finish reads every cell
    /// it walks from the cache or a cell job's walk, generating none.
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
        let (walked, layer_e) = with_sources(FeatureMembers::Served, |sources| {
            let plan = RangePlan::new(galaxy(), sources, &query);
            let layer_e = plan.cells().filter(|cell| cell.layer() == Layer::E).count();
            (plan.cells().count(), layer_e)
        });
        let (walked, layer_e) = (
            u64::try_from(walked).unwrap(),
            u64::try_from(layer_e).unwrap(),
        );
        let gate = BulkGate::default();
        let pool = gated_pool(8, &gate);
        let warm = caches(64 << 20);
        assert_eq!(served_on(&pool, &warm, &served).await, expected);
        drained(&pool, 8).await;
        // The feature cells and the interiors held, the grid not.
        let grid_cold = RangeCaches::new(
            Arc::new(SharedCellCache::new(64 << 20)),
            Arc::clone(&warm.features),
            Arc::clone(&warm.interiors),
            (Arc::clone(&warm.classes), Arc::clone(&warm.centre)),
        );
        gate.close();
        assert_eq!(served_on(&pool, &grid_cold, &served).await, expected);
        let counters = grid_cold.cells.counters();
        assert_eq!(
            (
                u64::try_from(counters.entries()).unwrap(),
                counters.hits(),
                counters.misses()
            ),
            (walked - layer_e, 0, 0),
            "the cells of the plan with the sources, each generated and walked ahead, but layer \
             E's, read in part"
        );
        gate.open();
        drained(&pool, 8).await;
        assert_eq!(
            u64::try_from(grid_cold.cells.counters().entries()).unwrap(),
            walked,
            "layer E's cells kept whole in the background"
        );
        let in_part = caches(64 << 20);
        let smaller = RangeQuery::builder(*query.centre(), LightYears::new(30.0))
            .build()
            .expect("a query in the clock window");
        let smaller = ServedQuery::new(shared_galaxy(), key(), smaller, FeatureMembers::Served);
        drop(served_on(&pool, &in_part, &smaller).await);
        drained(&pool, 8).await;
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

    /// A query whose grid is held but for some of its layer-E cells, fewer than are worth
    /// generating ahead and missed by the sample of the grid (its first cell, of layer E, held),
    /// still walks them ahead, read in part, and finds the rest held, and once answered keeps them
    /// whole; the answer is the sim's (P08.T12).
    #[tokio::test]
    async fn a_grid_held_but_for_layer_e_walks_it_ahead_and_keeps_it_whole() {
        let query = sun_like(0);
        let expected = uncached(&query, FeatureMembers::Held);
        let served = ServedQuery::new(shared_galaxy(), key(), query.clone(), FeatureMembers::Held);
        let cells: Vec<_> = RangePlan::new(galaxy(), &[], &query).cells().collect();
        let layer_e = cells.iter().filter(|cell| cell.layer() == Layer::E).count();
        assert!((2..AHEAD_GRID_CELLS).contains(&layer_e), "{layer_e} cells");
        assert_eq!(cells[0].layer(), Layer::E, "layer E is walked first");
        let gate = BulkGate::default();
        let pool = gated_pool(8, &gate);
        let caches = caches(64 << 20);
        let held = cells.len() - layer_e + 1;
        let held_cells = cells[..1].iter().chain(&cells[layer_e..]).copied();
        caches.cells.generate_whole(key(), galaxy(), held_cells);
        assert_eq!(caches.cells.counters().entries(), held);
        gate.close();
        assert_eq!(served_on(&pool, &caches, &served).await, expected);
        let counters = caches.cells.counters();
        assert_eq!(
            (counters.entries(), counters.misses()),
            (held, 0),
            "the lacking layer-E cells walked ahead, read in part, none asked for and missed"
        );
        assert_eq!(
            counters.hits(),
            u64::try_from(held).unwrap(),
            "the finish finds the rest held"
        );
        gate.open();
        drained(&pool, 8).await;
        assert_eq!(
            caches.cells.counters().entries(),
            cells.len(),
            "and keeps them whole once answered"
        );
        pool.shutdown().await.unwrap();
    }

    /// The server tests' galaxy as `version`'s rules build it, every hold that version turns on in
    /// force.
    fn galaxy_at(version: u32) -> Galaxy {
        use hyperion_sim::GeneratorVersion;
        use hyperion_sim::galaxy::imf::MassFunctionKind;
        use hyperion_sim::galaxy::params::GalaxyParams;
        let seed = Seed::new(SEED);
        Galaxy::from_params_at_version(
            seed,
            GalaxyParams::from_seed(seed, MassFunctionKind::default()),
            GeneratorVersion::new(version),
        )
        .expect("a drawn galaxy builds")
    }

    /// A served query takes the catalogue classes' hosts beside the features' members from 27
    /// (P09.T37.a): the classes' source follows the members', counting what
    /// [`CatalogueClassSource`] counts; held, there is none. The finish and the cells' jobs ask the
    /// same list (`super::with_sources`). With the global list taken, from 27 its source is beside
    /// them, in place of the centre's bare one (P10.T10.b's registration), the rows read it, and
    /// its census is the list's own.
    #[test]
    fn from_27_a_served_query_takes_the_classes_beside_the_members() {
        use hyperion_sim::galaxy::query::QuerySphere;
        let caches = caches(1 << 20);
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
        let sphere = QuerySphere::new(
            sun,
            LightYears::new(50.0),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a sphere");
        // How many sources, and the last one's counts about the Sun-like point.
        let asked = |galaxy: Galaxy, members: FeatureMembers| {
            let served = ServedQuery::new(Arc::new(galaxy), key(), sun_like(0), members);
            super::with_sources(&caches, &served, |sources, _| {
                let last = sources.last().map(|source| {
                    let counts = source.expected_in_sphere(&served.galaxy, &sphere);
                    counts.to_array().map(f64::to_bits)
                });
                (sources.len(), last)
            })
        };
        let at_27 = galaxy_at(27);
        let classes = CatalogueClassSource::new()
            .expected_in_sphere(&at_27, &sphere)
            .to_array()
            .map(f64::to_bits);
        assert_eq!(
            asked(at_27.clone(), FeatureMembers::Served),
            (2, Some(classes))
        );
        assert_eq!(asked(at_27.clone(), FeatureMembers::Held), (0, None));
        // With the list taken: an empty one, whose source holds no centre and counts nothing.
        let listed = |galaxy: Galaxy, members: FeatureMembers| {
            let galaxy = Arc::new(galaxy);
            let list = ListedTubes::new(
                (Arc::clone(&galaxy), key()),
                Arc::new(super::super::HeldGlobalList::default()),
                Arc::new(super::super::TubeTableCache::new(0)),
            );
            let served = ServedQuery::new(galaxy, key(), sun_like(0), members).with_list(list);
            super::with_sources(&caches, &served, |sources, reads| {
                let counts: Vec<_> = sources
                    .iter()
                    .map(|source| source.expected_in_sphere(&served.galaxy, &sphere))
                    .collect();
                (sources.len(), reads.list().is_some(), counts)
            })
        };
        let (n, read, counts) = listed(at_27.clone(), FeatureMembers::Served);
        assert_eq!((n, read), (3, true));
        assert_eq!(counts[1], hyperion_sim::galaxy::query::LayerCounts::ZERO);
        assert_eq!(
            counts[2].to_array().map(f64::to_bits),
            classes,
            "the classes' census, beside the list's"
        );
        assert_eq!(listed(at_27, FeatureMembers::Held).0, 0);
    }

    /// The server tests' galaxy under every 27 hold, shared with the pools' jobs.
    fn shared_galaxy_27() -> Arc<Galaxy> {
        static GALAXY: OnceLock<Arc<Galaxy>> = OnceLock::new();
        Arc::clone(GALAXY.get_or_init(|| Arc::new(galaxy_at(27))))
    }

    /// From 27 a cold query's class cells are made on the pool's workers ahead of its hits
    /// (P09.T37's bench): every cell the hits read, each once, so the hits make none, and the
    /// answer is the sim's bit for bit, cold and warm, on one worker and on eight.
    #[tokio::test]
    async fn from_27_the_class_cells_are_made_ahead_of_the_hits() {
        use hyperion_sim::GeneratorVersion;
        let galaxy = shared_galaxy_27();
        let key = GalaxyKey::new(SEED, GeneratorVersion::new(27));
        let query = sun_like(0);
        let source = FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache);
        let expected = beside_the_grid(
            &galaxy,
            FeatureMembers::Served,
            (&source, &NoClassCellCache),
            None,
            |sources, _| {
                assert_eq!(sources.len(), 2, "the members and the classes");
                range_query(&galaxy, &mut NoCache::new(), sources, &query)
            },
        );
        let layers = RangePlan::new(&galaxy, &[], &query).census().layers();
        let read = CatalogueClassSource::new()
            .cells_read(&galaxy, &query, layers)
            .len();
        assert!(read > 1_000, "{read}");
        let served = ServedQuery::new(
            Arc::clone(&galaxy),
            key,
            query.clone(),
            FeatureMembers::Served,
        );
        for workers in [1, 8] {
            let pool = pool(workers, INTERACTIVE_QUEUE_CAPACITY);
            let caches = caches(64 << 20);
            assert_eq!(served_on(&pool, &caches, &served).await, expected, "cold");
            let cold = caches.classes.counters();
            assert_eq!(cold.entries(), read, "every class cell made ahead, once");
            assert_eq!(cold.misses(), 0, "the hits make no class cell");
            assert!(cold.hits() > 0);
            assert_eq!(served_on(&pool, &caches, &served).await, expected, "warm");
            assert_eq!(caches.classes.counters().misses(), 0);
            pool.shutdown().await.unwrap();
        }
    }

    /// From 27, with the full potential, a served query reads its features' lists, of their recent
    /// and binary classes, through the interiors' cache, each feature cell read ahead in parts, and
    /// its answer is the sim's bit for bit, cold and warm, on one worker and on eight, through
    /// caches that keep everything and through caches that keep nothing (P09.T37's bench).
    #[tokio::test]
    async fn from_27_a_served_query_reads_its_features_lists_whatever_is_cached() {
        use hyperion_sim::GeneratorVersion;
        use hyperion_sim::galaxy::features::catalogue::FeatureCatalogue;
        use hyperion_sim::galaxy::features::members::FeatureLevelList;
        let galaxy = Arc::new(galaxy_at(27).with_full_potential());
        let key = GalaxyKey::new(SEED, GeneratorVersion::new(27));
        let query = sun_like(0);
        assert!(
            FeatureCatalogue::near(&galaxy, query.centre(), query.radius(), &NoFeatureCache)
                .any(|feature| FeatureLevelList::listed_of(&galaxy, &feature).is_some()),
            "features about the Sun-like point list classes"
        );
        let source = FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache);
        let expected = beside_the_grid(
            &galaxy,
            FeatureMembers::Served,
            (&source, &NoClassCellCache),
            None,
            |sources, _| range_query(&galaxy, &mut NoCache::new(), sources, &query),
        );
        let served = ServedQuery::new(Arc::clone(&galaxy), key, query, FeatureMembers::Served);
        for workers in [1, 8] {
            let pool = pool(workers, INTERACTIVE_QUEUE_CAPACITY);
            for budget in [64 << 20, 0] {
                let caches = caches(budget);
                for pass in ["cold", "warm"] {
                    assert_eq!(
                        served_on(&pool, &caches, &served).await,
                        expected,
                        "{pass}, {workers} workers, budgets of {budget} bytes"
                    );
                }
            }
            pool.shutdown().await.unwrap();
        }
    }

    /// The seed the tests' universes are of.
    #[test]
    fn the_fixture_is_the_server_tests_galaxy() {
        assert_eq!(shared_galaxy().seed(), Seed::new(SEED));
    }
}
