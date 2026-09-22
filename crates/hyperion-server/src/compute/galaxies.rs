//! The cache of built galaxies: four of them, shared by every request that needs one.
//!
//! A [`Galaxy`] is where a universe's seed becomes a model: the drawn and derived parameters, the
//! mass model with its potential tables in the plane, the density fields and the layer shares.
//! Building one costs about 130 ms and about 1.4 MiB of heap (plan 02, Risks, R19), so it is built
//! once per universe on the CPU pool and then kept. Because a `Galaxy` is fixed-size, bounding the
//! entries bounds the bytes, so this cache holds [`GALAXY_CACHE_ENTRIES`] galaxies rather than a
//! budget in bytes (plan 04, design note 23): four of them, about 5.5 MiB, which is a bridge's worth
//! of universes open at once.
//!
//! The key is a [`GalaxyKey`], `(seed, generator_version)`, so two saves of one seed share one
//! galaxy. Callers that ask for one key at once share one build through a [`SingleFlight`], and if
//! every one of them gives up, the build is given up too: a job still queued is skipped.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use hyperion_protocol::SeedHex;
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;

use super::{
    CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyKey, JobError, Priority, SingleFlight,
};
use crate::limits::GALAXY_CACHE_ENTRIES;

/// How a galaxy is built from its seed: [`Galaxy::new`], or a test's double.
type Build = Arc<dyn Fn(Seed) -> Galaxy + Send + Sync>;

/// A snapshot of the galaxy cache's contents and use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct GalaxyCounters {
    entries: usize,
    bytes: usize,
    hits: u64,
    misses: u64,
    builds: u64,
    evictions: u64,
}

impl GalaxyCounters {
    /// Galaxies held now, at most [`GALAXY_CACHE_ENTRIES`].
    #[must_use]
    pub fn entries(&self) -> usize {
        self.entries
    }

    /// Heap bytes the galaxies held own, by [`Galaxy::heap_bytes`].
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Lookups that found their galaxy built already.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Lookups that did not, and so started a build or joined one.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Galaxies built and held. Callers that ask for one key at once count one build between them.
    #[must_use]
    pub fn builds(&self) -> u64 {
        self.builds
    }

    /// Galaxies dropped to make room for others.
    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.evictions
    }
}

/// The galaxies built, the most recently used last, with the counters they share.
#[derive(Debug, Default)]
struct Held {
    entries: Vec<(GalaxyKey, Arc<Galaxy>)>,
    counters: GalaxyCounters,
}

impl Held {
    /// The galaxy of `key`, which becomes the most recently used, counting the lookup.
    fn get(&mut self, key: GalaxyKey) -> Option<Arc<Galaxy>> {
        let found = self.look_up(key);
        if found.is_some() {
            self.counters.hits += 1;
        } else {
            self.counters.misses += 1;
        }
        found
    }

    /// The galaxy of `key`, which becomes the most recently used, without counting the lookup.
    fn look_up(&mut self, key: GalaxyKey) -> Option<Arc<Galaxy>> {
        let index = self.entries.iter().position(|(held, _)| *held == key)?;
        let entry = self.entries.remove(index);
        let galaxy = Arc::clone(&entry.1);
        self.entries.push(entry);
        Some(galaxy)
    }

    /// Holds a freshly built `galaxy` under `key` as the most recently used, evicting the least
    /// recently used entry if the cache is full.
    ///
    /// Returns whatever it dropped, for the caller to drop outside the lock.
    fn insert(&mut self, key: GalaxyKey, galaxy: &Arc<Galaxy>) -> Option<Arc<Galaxy>> {
        let replaced = self
            .entries
            .iter()
            .position(|(held, _)| *held == key)
            .map(|index| self.entries.remove(index).1);
        let evicted = if replaced.is_none() && self.entries.len() >= GALAXY_CACHE_ENTRIES.get() {
            self.counters.evictions += 1;
            Some(self.entries.remove(0).1)
        } else {
            None
        };
        self.entries.push((key, Arc::clone(galaxy)));
        self.counters.builds += 1;
        self.counters.entries = self.entries.len();
        self.counters.bytes = self
            .entries
            .iter()
            .map(|(_, galaxy)| galaxy.heap_bytes())
            .sum();
        replaced.or(evicted)
    }
}

/// The galaxies the server has built, bounded to [`GALAXY_CACHE_ENTRIES`] of them.
///
/// [`GalaxyCache::get`] is the whole interface: it hands back the galaxy of a key, building it on
/// the CPU pool if nobody has yet. This is where a universe's potential tables are "computed once"
/// (brainstorm, "Runtime and code shape").
pub struct GalaxyCache {
    pool: Arc<CpuPool>,
    build: Build,
    flights: SingleFlight<GalaxyKey, Galaxy, ComputeError>,
    held: Arc<Mutex<Held>>,
}

impl GalaxyCache {
    /// An empty cache that builds its galaxies with [`Galaxy::new`] on `pool`.
    #[must_use]
    pub fn new(pool: Arc<CpuPool>) -> Self {
        Self::with_build(pool, Arc::new(Galaxy::new))
    }

    /// An empty cache whose galaxies come from `build` rather than [`Galaxy::new`], so that a test
    /// can count builds and hand out clones of a galaxy it built once.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_builder(
        pool: Arc<CpuPool>,
        build: impl Fn(Seed) -> Galaxy + Send + Sync + 'static,
    ) -> Self {
        Self::with_build(pool, Arc::new(build))
    }

    fn with_build(pool: Arc<CpuPool>, build: Build) -> Self {
        Self {
            pool,
            build,
            flights: SingleFlight::new(),
            held: Arc::new(Mutex::new(Held::default())),
        }
    }

    /// The galaxy of `key`, built if it is not held already.
    ///
    /// Callers that ask for one key at once share a single build and its result. If every one of
    /// them drops this future the build is given up: a job still queued is skipped, and one a worker
    /// has taken runs to the end and its galaxy is dropped.
    ///
    /// # Errors
    ///
    /// [`ComputeError::Submit`] if the pool's interactive queue is full or the pool is shutting
    /// down, and [`ComputeError::Job`] if the build was cancelled, the pool stopped before it ran, or
    /// it panicked, which would be a bug in [`Galaxy::new`]. A build has no failure of its own: a
    /// galaxy is a pure function of its key.
    ///
    /// # Panics
    ///
    /// If `key`'s generator version is not one this build can generate. The registry refuses such a
    /// universe before any handler reaches this, so a key that arrives here is a bug.
    pub async fn get(&self, key: GalaxyKey) -> Result<Arc<Galaxy>, ComputeError> {
        assert!(
            key.generator_version().is_supported(),
            "asked for a galaxy of generator version {}, which this build cannot generate",
            key.generator_version()
        );
        if let Some(galaxy) = lock(&self.held).get(key) {
            return Ok(galaxy);
        }
        let pool = Arc::clone(&self.pool);
        let build = Arc::clone(&self.build);
        let held = Arc::clone(&self.held);
        self.flights
            .run(key, move || async move {
                // A caller that read the cache just before another flight finished, and reached the
                // registry just after it left, would otherwise build a galaxy that is held already.
                // This look does not count, so that one `get` counts one lookup.
                if let Some(galaxy) = lock(&held).look_up(key) {
                    return Ok(galaxy);
                }
                let galaxy = build_on_pool(&pool, build, key).await?;
                // The galaxy it replaces or evicts is dropped outside the lock: freeing 1.4 MiB is
                // not the business of a critical section every request passes through.
                let dropped = lock(&held).insert(key, &galaxy);
                drop(dropped);
                Ok(galaxy)
            })
            .await
    }

    /// What the cache holds and how it has been used.
    #[must_use]
    pub fn counters(&self) -> GalaxyCounters {
        lock(&self.held).counters
    }
}

/// Runs one galaxy build on the pool as interactive work and logs what it cost.
async fn build_on_pool(
    pool: &CpuPool,
    build: Build,
    key: GalaxyKey,
) -> Result<Arc<Galaxy>, ComputeError> {
    let token = CancelToken::new();
    // Cancels the build once every waiter on the flight this runs in has gone.
    let _cancel_on_drop = CancelOnDrop::new(token.clone());
    let receiver = pool.try_submit(Priority::Interactive, token, move |_| {
        let started = Instant::now();
        let galaxy = build(Seed::new(key.seed()));
        tracing::info!(
            seed = %SeedHex::from_u64(key.seed()),
            generator_version = %key.generator_version(),
            build_ms = started.elapsed().as_secs_f64() * 1e3,
            heap_kib = galaxy.heap_bytes() / 1024,
            "built a galaxy"
        );
        galaxy
    })?;
    let galaxy = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
    Ok(Arc::new(galaxy))
}

impl fmt::Debug for GalaxyCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GalaxyCache")
            .field("counters", &self.counters())
            .field("building", &self.flights.len())
            .finish_non_exhaustive()
    }
}

fn lock(held: &Mutex<Held>) -> MutexGuard<'_, Held> {
    // Every critical section is a few vector and counter updates that cannot panic part-way, so a
    // poisoned lock still guards consistent state.
    held.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion};
    use tokio::time::timeout;

    use super::*;
    use crate::compute::SubmitJobError;
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};

    /// Upper bound on any wait: generous, because a galaxy build is 130 ms optimised and several
    /// times that on a loaded machine or an unoptimised build.
    const WAIT: Duration = Duration::from_secs(120);

    fn pool(workers: usize) -> Arc<CpuPool> {
        Arc::new(
            CpuPool::new(
                NonZeroUsize::new(workers).expect("a test asks for at least one worker"),
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .expect("the pool starts"),
        )
    }

    fn key(seed: u64) -> GalaxyKey {
        GalaxyKey::new(seed, GENERATOR_VERSION)
    }

    /// One real galaxy, built once for the whole test binary: the tests that exercise the cache and
    /// not the model hand out clones of it rather than paying a build each time.
    fn prebuilt() -> &'static Galaxy {
        static PREBUILT: OnceLock<Galaxy> = OnceLock::new();
        PREBUILT.get_or_init(|| Galaxy::new(Seed::new(1)))
    }

    /// A cache that counts its builds and clones [`prebuilt`] for every key.
    fn counting(pool: Arc<CpuPool>) -> (GalaxyCache, Arc<AtomicUsize>) {
        let builds = Arc::new(AtomicUsize::new(0));
        let cache = GalaxyCache::with_builder(pool, {
            let builds = Arc::clone(&builds);
            move |_| {
                builds.fetch_add(1, Ordering::SeqCst);
                prebuilt().clone()
            }
        });
        (cache, builds)
    }

    async fn get(cache: &GalaxyCache, seed: u64) -> Arc<Galaxy> {
        timeout(WAIT, cache.get(key(seed)))
            .await
            .expect("timed out building a galaxy")
            .expect("the pool builds the galaxy")
    }

    #[tokio::test]
    async fn two_concurrent_gets_of_one_key_build_once_and_share_the_galaxy() {
        let pool = pool(2);
        let (cache, builds) = counting(Arc::clone(&pool));
        let (first, second) = tokio::join!(get(&cache, 42), get(&cache, 42));
        assert!(
            Arc::ptr_eq(&first, &second),
            "both callers hold the same galaxy"
        );
        assert_eq!(builds.load(Ordering::SeqCst), 1);
        let counters = cache.counters();
        assert_eq!((counters.builds(), counters.entries()), (1, 1));
        assert_eq!(counters.bytes(), first.heap_bytes(), "the galaxy held");
        assert!(
            (1 << 20..2 << 20).contains(&counters.bytes()),
            "a galaxy is about 1.4 MiB, not {} bytes",
            counters.bytes()
        );

        // A third call, after the build, is a hit on the same galaxy.
        let third = get(&cache, 42).await;
        assert!(Arc::ptr_eq(&first, &third));
        assert_eq!(builds.load(Ordering::SeqCst), 1);
        assert_eq!(cache.counters().hits(), 1);
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_fifth_key_evicts_the_least_recently_used() {
        let pool = pool(2);
        let (cache, builds) = counting(Arc::clone(&pool));
        for seed in 1..=4 {
            get(&cache, seed).await;
        }
        // Seed 1 is the least recently used; using it again leaves seed 2 the oldest.
        let first = get(&cache, 1).await;
        assert_eq!(cache.counters().hits(), 1);

        get(&cache, 5).await;
        let counters = cache.counters();
        assert_eq!(
            (counters.entries(), counters.evictions()),
            (GALAXY_CACHE_ENTRIES.get(), 1)
        );
        assert!(
            Arc::ptr_eq(&get(&cache, 1).await, &first),
            "seed 1 was kept, not evicted"
        );
        assert_eq!(builds.load(Ordering::SeqCst), 5);
        // Seed 2 was the one evicted, so asking for it again builds it afresh.
        get(&cache, 2).await;
        assert_eq!(builds.load(Ordering::SeqCst), 6);
        let counters = cache.counters();
        assert_eq!(
            (
                counters.entries(),
                counters.builds(),
                counters.hits(),
                counters.misses(),
                counters.evictions()
            ),
            (GALAXY_CACHE_ENTRIES.get(), 6, 2, 6, 2)
        );
        pool.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_galaxy_rebuilt_after_eviction_equals_the_first_build() {
        // The real builder: what this pins is that a key's galaxy is the galaxy of the key's seed,
        // rebuilt or not. The galaxies that evict it are built at once, on as many workers.
        let pool = pool(GALAXY_CACHE_ENTRIES.get());
        let cache = GalaxyCache::new(Arc::clone(&pool));
        let first = get(&cache, 0x4d2).await;
        let fillers: Vec<_> = (1..=u64::try_from(GALAXY_CACHE_ENTRIES.get()).unwrap())
            .map(|seed| get(&cache, seed))
            .collect();
        futures_util::future::join_all(fillers).await;
        assert_eq!(cache.counters().evictions(), 1);

        let rebuilt = get(&cache, 0x4d2).await;
        assert!(
            !Arc::ptr_eq(&first, &rebuilt),
            "the first build was dropped"
        );
        assert_eq!(rebuilt.params(), first.params());
        assert_eq!(
            cache.counters().builds(),
            u64::try_from(GALAXY_CACHE_ENTRIES.get()).unwrap() + 2
        );
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_build_the_pool_will_not_take_is_a_submit_error_and_the_key_stays_free() {
        let pool = pool(1);
        let (cache, builds) = counting(Arc::clone(&pool));
        pool.shutdown().await.unwrap();
        for _ in 0..2 {
            assert_eq!(
                cache.get(key(7)).await.unwrap_err(),
                ComputeError::Submit(SubmitJobError::ShutDown)
            );
        }
        assert_eq!(builds.load(Ordering::SeqCst), 0);
        let counters = cache.counters();
        assert_eq!(
            (counters.entries(), counters.builds(), counters.misses()),
            (0, 0, 2)
        );
    }

    #[tokio::test]
    async fn a_build_every_waiter_has_given_up_on_is_skipped() {
        let pool = pool(1);
        let (cache, builds) = counting(Arc::clone(&pool));
        // Occupy the only worker, so that the build waits in the queue where a cancel reaches it.
        let (started, has_started) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let held = pool
            .try_submit(Priority::Interactive, CancelToken::new(), move |_| {
                started.send(()).unwrap();
                // Blocks this worker, not the runtime.
                let _ = wait.recv();
            })
            .unwrap();
        timeout(WAIT, has_started)
            .await
            .expect("timed out waiting for the worker")
            .unwrap();

        let mut first = Box::pin(cache.get(key(9)));
        let mut second = Box::pin(cache.get(key(9)));
        assert!(futures_util::poll!(&mut first).is_pending());
        assert!(futures_util::poll!(&mut second).is_pending());
        assert_eq!(pool.counters().queued_interactive(), 1, "one build, shared");
        drop(first);
        assert_eq!(pool.counters().cancelled(), 0, "one waiter remains");
        drop(second);

        // A job queued behind the build: when its answer arrives, the worker has passed the build.
        let probe = pool
            .try_submit(Priority::Interactive, CancelToken::new(), |_| "probe")
            .unwrap();
        release.send(()).unwrap();
        assert_eq!(timeout(WAIT, held).await.unwrap().unwrap(), Ok(()));
        assert_eq!(timeout(WAIT, probe).await.unwrap().unwrap(), Ok("probe"));
        assert_eq!(pool.counters().cancelled(), 1, "the build was skipped");
        assert_eq!(builds.load(Ordering::SeqCst), 0);
        assert_eq!(cache.counters().builds(), 0);
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    #[should_panic(expected = "which this build cannot generate")]
    async fn a_key_of_an_unsupported_generator_version_is_a_bug() {
        let (cache, _builds) = counting(pool(1));
        let unsupported = GalaxyKey::new(1, GeneratorVersion::new(GENERATOR_VERSION.get() + 1));
        let _ = cache.get(unsupported).await;
    }
}
