//! The sky's tables, built once per galaxy and kept (rendering plan R06, R06.T11.c; Design notes 7
//! and 13; decided 2026-10-03, `decision-r06-tables.md`, item B.2).
//!
//! A galaxy's luminosity tables depend on its seed alone: one table per galaxy, built at the
//! reference time +H, serves every query of the clock window, each reading its own light ages
//! through [`LuminosityTables::age_for`]. So [`SkyTablesService`] keys them by [`GalaxyKey`] alone,
//! never by a request's time or place, and keeps them in a [`SharedByteLru`] of their own budget,
//! `HYPERION_SKY_TABLES_MB` (160 MiB by default: two galaxies), separate from the census cells'
//! `HYPERION_SKY_CACHE_MB`. Callers that ask for one galaxy's tables at once share one build
//! through a [`SingleFlight`], and if every one of them gives up, the build is given up too: its
//! jobs still queued are skipped.
//!
//! The build runs as bulk jobs from [`LuminosityTables::plan`], which the sim splits so that the
//! server's pool can run it (the sim spawns no threads): one job makes the plan, some 5 CPU-s for
//! the Milky Way; then, stage by stage, one metallicity at a time from the lowest, the stage's
//! track samples, a dozen jobs of at most 64 mass nodes, and its accumulation, one job per
//! component bin that reads the stage; then one job assembles the tables. While a stage
//! accumulates, the next stage's samples are made, one stage ahead, which holds two stages' samples
//! at once and keeps every worker busy through each stage's few accumulation jobs (R06's Risks,
//! "The job split in stages": about 440 MiB on 16 threads, against 240 MiB without). A bin adds its
//! stages in their order, so the tables are the serial build's bit for bit, as the sim's
//! `parallel_build_equals_serial` holds for any partition and order of the jobs. The brightness
//! envelope is a fitted table, with nothing to build, and the cells' offset bounds a few thousand
//! tidal radii; both are kept beside the tables.
//!
//! A server whose sky's caps are forced for a test ([`SkyCaps::forced`](super::SkyCaps::forced))
//! is given tables that hold no star, built in one job at once, unless its caps ask for the
//! galaxy's own ([`SkyCaps::with_galaxy_tables`](super::SkyCaps::with_galaxy_tables)). The service
//! is made with the one source its server's caps name, so its cache holds one kind of tables.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use hyperion_protocol::SeedHex;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::sky::luminosity::{
    BinSums, LuminosityTables, SampleChunk, Stage, TablesPlan, TrackSamples,
};

use super::sky::{BulkJobs, SkyTables, SkyTablesSource, bulk};
use super::{CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyKey, SingleFlight};
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

impl HeapBytes for SkyTables {
    /// The tables', the envelope's and the offset bounds' own heap bytes, by the sim's counts.
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// The sky tables' counters: the cache's size and use, and the builds that filled it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SkyTablesCounters {
    cache: LruCounters,
    builds: u64,
}

impl SkyTablesCounters {
    /// The galaxies' tables held, the bytes they are charged, the budget, and the hits, misses,
    /// evictions and refusals so far.
    ///
    /// A request that has to build its tables counts two misses, one before its flight and one
    /// inside it, which looks again in case another flight finished meanwhile; one that joins a
    /// flight in progress counts one.
    #[must_use]
    pub fn cache(&self) -> LruCounters {
        self.cache
    }

    /// Tables built so far: callers that ask for one galaxy's at once count one build between them,
    /// and so do two skies of one galaxy at any times, places and cuts.
    #[must_use]
    pub fn builds(&self) -> u64 {
        self.builds
    }
}

/// The sky tables of every galaxy the server holds, in one byte budget, each built once on the
/// CPU pool.
pub(crate) struct SkyTablesService {
    pool: Arc<CpuPool>,
    /// What every table this service builds is built from: the galaxy's components, or none.
    source: SkyTablesSource,
    held: Arc<SharedByteLru<GalaxyKey, SkyTables>>,
    flights: SingleFlight<GalaxyKey, SkyTables, ComputeError>,
    builds: Arc<AtomicU64>,
}

impl SkyTablesService {
    /// A service that builds its tables from `source` on `pool`, keeping at most `budget_bytes` of
    /// them.
    ///
    /// A budget of zero caches nothing: every sky builds its galaxy's tables, though skies asked at
    /// once still share one build.
    #[must_use]
    pub(crate) fn new(pool: Arc<CpuPool>, source: SkyTablesSource, budget_bytes: usize) -> Self {
        Self {
            pool,
            source,
            held: Arc::new(SharedByteLru::new(budget_bytes)),
            flights: SingleFlight::new(),
            builds: Arc::new(AtomicU64::new(0)),
        }
    }

    /// The tables of `galaxy`, the galaxy `key` names: those held, or built from the service's
    /// source.
    ///
    /// # Errors
    ///
    /// [`ComputeError::Submit`] if the pool is shutting down or faulted, and [`ComputeError::Job`]
    /// if a job of the build was cancelled, panicked or was dropped with the pool.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of `key`, since its tables would then be kept under
    /// another galaxy's key. The server takes both from the request's own universe, so a mismatch
    /// is a bug.
    pub(crate) async fn get(
        &self,
        key: GalaxyKey,
        galaxy: &Arc<Galaxy>,
    ) -> Result<Arc<SkyTables>, ComputeError> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a galaxy's sky tables are kept under its own key, and this is another's"
        );
        if let Some(tables) = self.held.get(&key) {
            return Ok(tables);
        }
        let source = self.source;
        let (pool, held, builds, galaxy) = (
            Arc::clone(&self.pool),
            Arc::clone(&self.held),
            Arc::clone(&self.builds),
            Arc::clone(galaxy),
        );
        self.flights
            .run(key, move || async move {
                // As in `GalaxyCache::get`: a caller can arrive just after another flight has
                // finished and left the registry, and the tables it built are worth more than a
                // second build. The look is counted, as the density maps' is, since
                // `SharedByteLru` has no uncounted lookup.
                if let Some(tables) = held.get(&key) {
                    return Ok(tables);
                }
                let token = CancelToken::new();
                // Cancels the build's queued jobs once every waiter on this flight has gone.
                let _cancel_on_drop = CancelOnDrop::new(token.clone());
                let started = Instant::now();
                let tables = Arc::new(match source {
                    SkyTablesSource::Galaxy => build_on_pool(&pool, &galaxy, &token).await?,
                    SkyTablesSource::Dark => {
                        let galaxy = Arc::clone(&galaxy);
                        bulk(&pool, &token, move |_: &CancelToken| {
                            SkyTables::new(LuminosityTables::dark(&galaxy), &galaxy)
                        })
                        .await?
                    }
                });
                builds.fetch_add(1, Ordering::Relaxed);
                tracing::info!(
                    seed = %SeedHex::from_u64(key.seed()),
                    generator_version = %key.generator_version(),
                    ?source,
                    build_ms = started.elapsed().as_secs_f64() * 1e3,
                    heap_mib = SkyTables::heap_bytes(&tables) / (1 << 20),
                    "built a galaxy's sky tables"
                );
                // Tables larger than the whole budget are handed back and not stored; the waiters
                // get them either way.
                drop(held.insert(key, Arc::clone(&tables)));
                Ok(tables)
            })
            .await
    }

    /// The tables held, the bytes they are charged, the hits, misses, evictions and refusals so
    /// far, and the builds.
    #[must_use]
    pub(crate) fn counters(&self) -> SkyTablesCounters {
        SkyTablesCounters {
            cache: self.held.counters(),
            builds: self.builds.load(Ordering::Relaxed),
        }
    }
}

impl fmt::Debug for SkyTablesService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkyTablesService")
            .field("counters", &self.counters())
            .field("building", &self.flights.len())
            .finish_non_exhaustive()
    }
}

/// The tables of `galaxy`, built as bulk jobs under `token` from [`LuminosityTables::plan`] (see
/// the [module](self) documentation): the plan, then stage by stage its samples and their
/// accumulation, the next stage's samples made while a stage accumulates, then the assembly.
///
/// # Errors
///
/// Those of [`bulk`].
async fn build_on_pool(
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    token: &CancelToken,
) -> Result<SkyTables, ComputeError> {
    let plan = {
        let galaxy = Arc::clone(galaxy);
        Arc::new(
            bulk(pool, token, move |_: &CancelToken| {
                LuminosityTables::plan(&galaxy)
            })
            .await?,
        )
    };
    let mut sums: Vec<Option<BinSums>> = plan.bin_sums().into_iter().map(Some).collect();
    let stages: Vec<Stage> = plan.stages().collect();
    let mut sampling = match stages.first() {
        Some(&first) => Some(samples(pool, &plan, first, token).await?),
        None => None,
    };
    for (index, &stage) in stages.iter().enumerate() {
        let made = sampling
            .take()
            .expect("each stage's samples are asked for before its turn")
            .results()
            .await?;
        let made = Arc::new(plan.track_samples(made));
        let accumulating = accumulate(pool, &plan, &made, stage, &mut sums, token).await?;
        // The accumulation is queued first, so that the workers take it before the next stage's
        // samples, which fill the workers this stage's few bins leave idle.
        if let Some(&next) = stages.get(index + 1) {
            sampling = Some(samples(pool, &plan, next, token).await?);
        }
        for (bin, sum) in accumulating.results().await? {
            sums[bin] = Some(sum);
        }
    }
    let sums: Vec<BinSums> = sums
        .into_iter()
        .map(|sum| sum.expect("every bin's sums come back from its last stage"))
        .collect();
    let galaxy = Arc::clone(galaxy);
    bulk(pool, token, move |_: &CancelToken| {
        SkyTables::new(plan.assemble(sums), &galaxy)
    })
    .await
}

/// Queues `stage`'s sample jobs, in their order.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the pool is shutting down or faulted.
async fn samples(
    pool: &CpuPool,
    plan: &Arc<TablesPlan>,
    stage: Stage,
    token: &CancelToken,
) -> Result<BulkJobs<SampleChunk>, ComputeError> {
    let jobs = plan.sample_jobs(stage).map(|job| {
        let plan = Arc::clone(plan);
        move || plan.run_samples(job)
    });
    BulkJobs::submit(pool, token, jobs).await
}

/// Queues `stage`'s accumulation jobs over its samples `made`, each taking its bin's running sums
/// out of `sums` and handing them back with the bin's place.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the pool is shutting down or faulted.
///
/// # Panics
///
/// If a bin's sums are not back from its previous stage, which [`build_on_pool`] waits for.
async fn accumulate(
    pool: &CpuPool,
    plan: &Arc<TablesPlan>,
    made: &Arc<TrackSamples>,
    stage: Stage,
    sums: &mut [Option<BinSums>],
    token: &CancelToken,
) -> Result<BulkJobs<(usize, BinSums)>, ComputeError> {
    let jobs: Vec<_> = plan
        .accumulate_jobs(stage)
        .map(|job| {
            let mut sum = sums[job.bin()]
                .take()
                .expect("a bin's sums are back from its previous stage");
            let (plan, made) = (Arc::clone(plan), Arc::clone(made));
            move || {
                plan.run_accumulate(&made, job, &mut sum);
                (job.bin(), sum)
            }
        })
        .collect();
    BulkJobs::submit(pool, token, jobs).await
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::mpsc;

    use futures_util::FutureExt;
    use futures_util::future::join3;
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use tokio::time::timeout;

    use super::*;
    use crate::compute::{JobReceiver, Priority};
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
    use crate::testing::WAIT;

    /// The seed of the galaxy whose tables these tests build.
    const SEED: u64 = 0x4d2;

    /// A pool of one worker, which takes its jobs in order.
    fn one_worker() -> Arc<CpuPool> {
        Arc::new(
            CpuPool::new(
                NonZeroUsize::MIN,
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .unwrap(),
        )
    }

    fn galaxy() -> (GalaxyKey, Arc<Galaxy>) {
        (
            GalaxyKey::new(SEED, GENERATOR_VERSION),
            Arc::new(Galaxy::new(Seed::new(SEED))),
        )
    }

    /// Holds the pool's one worker on a job until the sender returned is sent to, so that the jobs
    /// queued meanwhile wait.
    async fn hold(pool: &CpuPool) -> (mpsc::Sender<()>, JobReceiver<()>) {
        let (release, gate) = mpsc::channel::<()>();
        let held = pool
            .submit(
                Priority::Bulk,
                CancelToken::new(),
                move |_: &CancelToken| {
                    gate.recv().expect("the gate is released");
                },
            )
            .await
            .unwrap();
        (release, held)
    }

    /// Waits until the one worker has run every job queued before now.
    async fn drain(pool: &CpuPool) {
        let token = CancelToken::new();
        let fence = bulk(pool, &token, |_: &CancelToken| ());
        timeout(WAIT, fence)
            .await
            .expect("timed out draining the pool")
            .unwrap();
    }

    /// Two skies of one galaxy asked at once share one build of its tables, and a third finds
    /// them held. The first asks before its flight and again inside it; the second joins the
    /// flight, so the three lookups miss.
    #[tokio::test]
    async fn skies_asking_at_once_share_one_build() {
        let pool = one_worker();
        let service = SkyTablesService::new(Arc::clone(&pool), SkyTablesSource::Dark, 64 << 20);
        let (key, galaxy) = galaxy();
        let (release, held) = hold(&pool).await;
        // `join3` polls in order: the first queues the build behind the held worker, the second
        // joins its flight, and only then is the worker released.
        let (one, other, ()) = timeout(
            WAIT,
            join3(
                service.get(key, &galaxy),
                service.get(key, &galaxy),
                async {
                    release.send(()).unwrap();
                },
            ),
        )
        .await
        .expect("timed out building the tables");
        let (one, other) = (one.unwrap(), other.unwrap());
        assert!(Arc::ptr_eq(&one, &other), "one build, shared");
        timeout(WAIT, held).await.unwrap().unwrap().unwrap();
        let counters = service.counters();
        assert_eq!(counters.builds(), 1, "{counters:?}");
        assert_eq!(
            (
                counters.cache().misses(),
                counters.cache().hits(),
                counters.cache().entries()
            ),
            (3, 0, 1),
            "{counters:?}"
        );
        let again = timeout(WAIT, service.get(key, &galaxy))
            .await
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&one, &again), "held");
        let counters = service.counters();
        assert_eq!((counters.builds(), counters.cache().hits()), (1, 1));
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A build that every sky has given up is given up too: its job still queued is skipped, and
    /// nothing is built or held.
    #[tokio::test]
    async fn a_build_no_sky_waits_for_is_given_up() {
        let pool = one_worker();
        let service = SkyTablesService::new(Arc::clone(&pool), SkyTablesSource::Dark, 64 << 20);
        let (key, galaxy) = galaxy();
        let (release, held) = hold(&pool).await;
        let mut asking = Box::pin(service.get(key, &galaxy));
        assert!(
            (&mut asking).now_or_never().is_none(),
            "the build waits behind the held worker"
        );
        drop(asking);
        release.send(()).unwrap();
        timeout(WAIT, held).await.unwrap().unwrap().unwrap();
        drain(&pool).await;
        assert_eq!(pool.counters().cancelled(), 1, "{:?}", pool.counters());
        let counters = service.counters();
        assert_eq!(
            (counters.builds(), counters.cache().entries()),
            (0, 0),
            "{counters:?}"
        );
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A budget of nought keeps no tables: each sky builds its own.
    #[tokio::test]
    async fn a_budget_of_nought_keeps_no_tables() {
        let pool = one_worker();
        let service = SkyTablesService::new(Arc::clone(&pool), SkyTablesSource::Dark, 0);
        let (key, galaxy) = galaxy();
        for builds in 1..=2 {
            timeout(WAIT, service.get(key, &galaxy))
                .await
                .unwrap()
                .unwrap();
            let counters = service.counters();
            assert_eq!(
                (
                    counters.builds(),
                    counters.cache().entries(),
                    counters.cache().refused()
                ),
                (builds, 0, builds),
                "{counters:?}"
            );
        }
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// The tables of one galaxy are kept under its key alone, and another galaxy's key is refused.
    #[tokio::test]
    #[should_panic(expected = "kept under its own key")]
    async fn another_galaxys_key_is_refused() {
        let pool = one_worker();
        let service = SkyTablesService::new(pool, SkyTablesSource::Dark, 64 << 20);
        let (_, galaxy) = galaxy();
        let other = GalaxyKey::new(SEED + 1, GENERATOR_VERSION);
        drop(service.get(other, &galaxy).await);
    }
}
