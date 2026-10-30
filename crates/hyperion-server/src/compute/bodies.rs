//! The cache of generated planetary systems: each system's context and its bodies as they were
//! born, bounded in bytes (plan 14, P14.T36.a).
//!
//! A system's bodies are plan 14's [`generate`] of its [`SystemContext`], which walks every zone's
//! disc, class and placement, and the `SYSTEM` display asks for the same system at every time it is
//! scrubbed to and for each body it selects. So the server keeps what it built:
//! [`SharedBodyCache`] is a [`SharedByteLru`] over `(GalaxyKey, SystemId)` holding
//! `Arc<(SystemContext, PlanetarySystem)>`, the context beside the system because every query at a
//! time reads both (P14.T30.b). An entry is primordial state, never a record at some time, so a
//! request at another time reuses it, and eviction is always safe.
//!
//! Unlike [`SharedSystemCache`](super::SharedSystemCache), a miss is filled under a
//! [`SingleFlight`], so that two consoles opening one system generate it once: the generation runs
//! as one interactive job on the [`CpuPool`], and every caller that misses while it runs waits on
//! the same job. The job is the caller's closure, so that the handler decides where the stars come
//! from; the server's lends them from the `SystemStars` cache, so the stars are not evolved twice.
//!
//! # The budget
//!
//! Its budget is its own, `--body-cache` (`HYPERION_BODY_CACHE_MB`, default 128 MiB), not shared
//! with the `SystemStars` cache. The two hold different things for different displays: a sweep of
//! the `GALAXY` display's contacts fills the stars cache with hundreds of systems nobody opens,
//! which must not evict the few systems a `SYSTEM` display has open, and one [`SharedByteLru`]
//! holds one kind of value. An entry is charged the context's heap (its stars' models and tracks,
//! copied from the stars cache's entry) and the system's zones, hosts and bodies by count, so it
//! costs about as much as the same system's stars plus a few hundred bytes a body.
//!
//! # The events' models
//!
//! A system's events (`body_events`, P14.T36.b) are listed from its event models, read from its
//! records at the epoch ([`PlanetarySystem::event_models`], a whole snapshot's work), so the cache
//! keeps one [`SystemEvents`] per system beside it ([`SharedBodyCache::events_of`]), under the same
//! key, built from the cached system under its own [`SingleFlight`] as one interactive job, and
//! in an LRU of its own, since one [`SharedByteLru`] holds one kind of value. It takes a budget of
//! the same size as the systems': a system's models are one entry a planet, member and moon, about
//! what its bodies cost, and an evicted model is built again from its system. The server asks for
//! them from generator version 25, P14.T31.c's hold; below it they are empty.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use hyperion_sim::galaxy::placement::ResolveSystemError;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::system::SystemEvents;
use hyperion_sim::planetary::{PlanetarySystem, SystemContext, generate};

use super::{CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyKey, JobError, Priority};
use super::{SingleFlight, SubmitJobError};
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// What names a system in the cache: the galaxy it belongs to, and the system itself.
type BodyEntryKey = (GalaxyKey, SystemId);

/// One cached system: the context its bodies were generated from, and the bodies as they were
/// born (plan 14's `PlanetarySystem`).
pub type GeneratedSystem = (SystemContext, PlanetarySystem);

impl HeapBytes for GeneratedSystem {
    /// The context's stars and hierarchy ([`SystemContext::heap_bytes`]) and the system's zones,
    /// hosts and bodies ([`PlanetarySystem::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        self.0.heap_bytes().saturating_add(self.1.heap_bytes())
    }
}

impl HeapBytes for SystemEvents {
    /// One entry a body with models ([`SystemEvents::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        SystemEvents::heap_bytes(self)
    }
}

/// Why a system's bodies could not be had.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GenerateBodiesError {
    /// The ID names no system of the galaxy (plan 03's `resolve`).
    NoSuchSystem(ResolveSystemError),
    /// The generation's pool job could not run or did not finish.
    Compute(ComputeError),
}

impl From<ComputeError> for GenerateBodiesError {
    fn from(error: ComputeError) -> Self {
        Self::Compute(error)
    }
}

impl From<SubmitJobError> for GenerateBodiesError {
    fn from(error: SubmitJobError) -> Self {
        Self::Compute(ComputeError::Submit(error))
    }
}

impl From<JobError> for GenerateBodiesError {
    fn from(error: JobError) -> Self {
        Self::Compute(ComputeError::Job(error))
    }
}

/// A snapshot of the body cache: its systems' and its event models' [`LruCounters`], and how many
/// of each it has built.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BodyCacheCounters {
    cache: LruCounters,
    generated: u64,
    events: LruCounters,
    modelled: u64,
}

impl BodyCacheCounters {
    /// The systems held, the bytes they are charged, the budget, and the hits, misses, evictions
    /// and refusals so far.
    #[must_use]
    pub fn cache(&self) -> LruCounters {
        self.cache
    }

    /// Systems generated so far: one per [`SingleFlight`] that ran its job to the end, however
    /// many callers waited on it. A miss that joins a generation in progress generates nothing.
    #[must_use]
    pub fn generated(&self) -> u64 {
        self.generated
    }

    /// The event models held, the bytes they are charged, the budget, and their hits, misses,
    /// evictions and refusals so far ([`SharedBodyCache::events_of`]).
    #[must_use]
    pub fn events(&self) -> LruCounters {
        self.events
    }

    /// Systems' event models built so far, one per flight run to the end, as
    /// [`generated`](Self::generated) counts systems.
    #[must_use]
    pub fn modelled(&self) -> u64 {
        self.modelled
    }
}

/// The generated planetary systems of every galaxy the server holds, in one byte budget, with
/// their event models in another.
#[derive(Debug)]
pub struct SharedBodyCache {
    pool: Arc<CpuPool>,
    systems: Arc<SharedByteLru<BodyEntryKey, GeneratedSystem>>,
    flights: SingleFlight<BodyEntryKey, GeneratedSystem, GenerateBodiesError>,
    generated: Arc<AtomicU64>,
    events: Arc<SharedByteLru<BodyEntryKey, SystemEvents>>,
    event_flights: SingleFlight<BodyEntryKey, SystemEvents, ComputeError>,
    modelled: Arc<AtomicU64>,
}

impl SharedBodyCache {
    /// An empty cache that holds at most `budget_bytes` of charged systems, and as much again of
    /// their event models, building both on `pool`.
    ///
    /// A budget of zero caches nothing: every request generates its system, and concurrent ones
    /// still share one generation.
    #[must_use]
    pub fn new(pool: Arc<CpuPool>, budget_bytes: usize) -> Self {
        Self {
            pool,
            systems: Arc::new(SharedByteLru::new(budget_bytes)),
            flights: SingleFlight::new(),
            generated: Arc::new(AtomicU64::new(0)),
            events: Arc::new(SharedByteLru::new(budget_bytes)),
            event_flights: SingleFlight::new(),
            modelled: Arc::new(AtomicU64::new(0)),
        }
    }

    /// The system `id` of the galaxy `key` names: the cache's, or generated on the pool, then
    /// stored.
    ///
    /// On a miss `context` is run as one interactive pool job to give the system's context, and
    /// the same job generates its bodies from it with plan 14's [`generate`] in the universe of
    /// the key's seed. Callers that miss one key at once share that job; if every one of them
    /// gives up, a job still queued is skipped. Only a system that resolved is ever stored.
    ///
    /// # Errors
    ///
    /// [`GenerateBodiesError::NoSuchSystem`] with `context`'s refusal, which is not stored; and
    /// [`GenerateBodiesError::Compute`] if the interactive queue is full (`queue_full`), the pool
    /// is shutting down, or the job is cancelled or panics.
    pub async fn get_or_generate<F>(
        &self,
        key: GalaxyKey,
        id: SystemId,
        context: F,
    ) -> Result<Arc<GeneratedSystem>, GenerateBodiesError>
    where
        F: FnOnce() -> Result<SystemContext, ResolveSystemError> + Send + 'static,
    {
        let entry_key = (key, id);
        if let Some(system) = self.systems.get(&entry_key) {
            return Ok(system);
        }
        let pool = Arc::clone(&self.pool);
        let systems = Arc::clone(&self.systems);
        let generated = Arc::clone(&self.generated);
        self.flights
            .run(entry_key, move || async move {
                // A caller that missed just before another flight stored the system, and reached
                // the registry just after that flight left it, would otherwise generate it again.
                if systems.contains_key(&entry_key)
                    && let Some(system) = systems.get(&entry_key)
                {
                    return Ok(system);
                }
                let token = CancelToken::new();
                // Cancels the job once every waiter on this flight has gone.
                let _cancel_on_drop = CancelOnDrop::new(token.clone());
                let receiver = pool.try_submit(Priority::Interactive, token, move |_| {
                    let ctx = context()?;
                    let planets = generate(hyperion_sim::Seed::new(key.seed()), &ctx);
                    Ok::<_, ResolveSystemError>((ctx, planets))
                })?;
                let system = receiver
                    .await
                    .unwrap_or_else(|closed| Err(JobError::from(closed)))?
                    .map_err(GenerateBodiesError::NoSuchSystem)?;
                generated.fetch_add(1, Ordering::Relaxed);
                let system = Arc::new(system);
                // A system larger than the whole budget is handed back and still answered from.
                let _ = systems.insert(entry_key, Arc::clone(&system));
                Ok(system)
            })
            .await
    }

    /// The event models of the system `id` of the galaxy `key` names, whose bodies are `system`:
    /// the cache's, or built on the pool, then stored (plan 14, P14.T36.b).
    ///
    /// On a miss one interactive pool job reads the system's records at the epoch into its models
    /// ([`PlanetarySystem::event_models`]); callers that miss one key at once share it, and if every
    /// one of them gives up, a job still queued is skipped. Before generator version 25 the models
    /// are empty (P14.T31.c's hold).
    ///
    /// # Errors
    ///
    /// A [`ComputeError`] if the interactive queue is full (`queue_full`), the pool is shutting
    /// down, or the job is cancelled or panics.
    pub async fn events_of(
        &self,
        key: GalaxyKey,
        id: SystemId,
        system: Arc<GeneratedSystem>,
    ) -> Result<Arc<SystemEvents>, ComputeError> {
        let entry_key = (key, id);
        if let Some(models) = self.events.get(&entry_key) {
            return Ok(models);
        }
        let pool = Arc::clone(&self.pool);
        let events = Arc::clone(&self.events);
        let modelled = Arc::clone(&self.modelled);
        self.event_flights
            .run(entry_key, move || async move {
                // As for the systems: another flight may have stored the models just before.
                if events.contains_key(&entry_key)
                    && let Some(models) = events.get(&entry_key)
                {
                    return Ok(models);
                }
                let token = CancelToken::new();
                // Cancels the job once every waiter on this flight has gone.
                let _cancel_on_drop = CancelOnDrop::new(token.clone());
                let receiver = pool
                    .try_submit(Priority::Interactive, token, move |_| {
                        let (ctx, planets) = system.as_ref();
                        planets.event_models(ctx)
                    })
                    .map_err(ComputeError::Submit)?;
                let models = receiver
                    .await
                    .unwrap_or_else(|closed| Err(JobError::from(closed)))
                    .map_err(ComputeError::Job)?;
                modelled.fetch_add(1, Ordering::Relaxed);
                let models = Arc::new(models);
                // Models larger than the whole budget are handed back and still answered from.
                let _ = events.insert(entry_key, Arc::clone(&models));
                Ok(models)
            })
            .await
    }

    /// The systems held, the bytes they are charged, their use, and the generations so far, with
    /// the event models'.
    #[must_use]
    pub fn counters(&self) -> BodyCacheCounters {
        BodyCacheCounters {
            cache: self.systems.counters(),
            generated: self.generated.load(Ordering::Relaxed),
            events: self.events.counters(),
            modelled: self.modelled.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::OnceLock;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    use hyperion_sim::galaxy::Galaxy;
    use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, candidate_count, generate_cell};
    use hyperion_sim::id::Layer;
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use tokio::time::timeout;

    use super::*;
    use crate::cache::ENTRY_OVERHEAD_BYTES;
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};

    /// The seed of the galaxy these tests generate systems in, the integration tests' own.
    const SEED: u64 = 0x4d2;

    /// Upper bound on any wait: a system generates in milliseconds, far less on an idle machine.
    const WAIT: Duration = Duration::from_secs(120);

    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
    }

    fn key() -> GalaxyKey {
        GalaxyKey::new(SEED, GENERATOR_VERSION)
    }

    fn pool() -> Arc<CpuPool> {
        Arc::new(
            CpuPool::new(
                NonZeroUsize::new(2).expect("two workers"),
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .expect("the pool starts"),
        )
    }

    /// The first `n` systems of layer C at the solar circle.
    fn systems(n: usize) -> Vec<SystemId> {
        let mut cell = Vec::new();
        generate_cell(
            galaxy(),
            CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid"),
            &mut cell,
        );
        assert!(cell.len() >= n, "the cell holds {} systems", cell.len());
        cell.iter().take(n).map(SystemRecord::id).collect()
    }

    /// The system's context, as the server builds it, counting each call in `calls`.
    fn counted(
        id: SystemId,
        calls: &Arc<AtomicUsize>,
    ) -> impl FnOnce() -> Result<SystemContext, ResolveSystemError> + Send + 'static {
        let calls = Arc::clone(calls);
        move || {
            calls.fetch_add(1, Ordering::SeqCst);
            SystemContext::for_system(galaxy(), id)
        }
    }

    async fn get(
        cache: &SharedBodyCache,
        id: SystemId,
        calls: &Arc<AtomicUsize>,
    ) -> Result<Arc<GeneratedSystem>, GenerateBodiesError> {
        timeout(WAIT, cache.get_or_generate(key(), id, counted(id, calls)))
            .await
            .expect("timed out generating a system")
    }

    /// What the cache charges an entry.
    fn charge(system: &GeneratedSystem) -> usize {
        system.heap_bytes() + size_of::<GeneratedSystem>() + ENTRY_OVERHEAD_BYTES
    }

    #[tokio::test]
    async fn two_concurrent_requests_for_one_system_generate_it_once() {
        let pool = pool();
        let cache = SharedBodyCache::new(Arc::clone(&pool), 64 << 20);
        let calls = Arc::new(AtomicUsize::new(0));
        let id = systems(1)[0];
        // The generation is held until both requests have missed: a system can generate before
        // the first request first awaits its job, which would then answer the second from the
        // cache, and the two would no longer be concurrent.
        let (release, held) = std::sync::mpsc::channel::<()>();
        let gated = {
            let calls = Arc::clone(&calls);
            move || {
                held.recv_timeout(WAIT)
                    .expect("the test releases the generation");
                calls.fetch_add(1, Ordering::SeqCst);
                SystemContext::for_system(galaxy(), id)
            }
        };
        let first = timeout(WAIT, cache.get_or_generate(key(), id, gated));
        let second = get(&cache, id, &calls);
        let both_missed = async {
            while cache.counters().cache().misses() < 2 {
                tokio::task::yield_now().await;
            }
            release.send(()).expect("the generation is waiting");
        };
        let (first, second, ()) = tokio::join!(first, second, both_missed);
        let (first, second) = (
            first.expect("timed out generating a system").unwrap(),
            second.unwrap(),
        );
        assert!(Arc::ptr_eq(&first, &second), "both hold the one system");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "one generation");
        let counters = cache.counters();
        assert_eq!(counters.generated(), 1);
        assert_eq!(
            (counters.cache().misses(), counters.cache().entries()),
            (2, 1)
        );
        // What was generated is the sim's own, and it is charged what it holds.
        let ctx = SystemContext::for_system(galaxy(), id).unwrap();
        assert_eq!(first.0, ctx);
        assert_eq!(first.1, generate(Seed::new(SEED), &ctx));
        assert_eq!(counters.cache().bytes(), charge(&first));

        // A later request is a hit, and generates nothing.
        let third = get(&cache, id, &calls).await.unwrap();
        assert!(Arc::ptr_eq(&first, &third));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(cache.counters().cache().hits(), 1);
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn systems_past_the_byte_bound_are_evicted_and_the_bound_is_never_exceeded() {
        let pool = pool();
        let ids = systems(12);
        let calls = Arc::new(AtomicUsize::new(0));
        // Measure every system's charge with a roomy cache, then hold them to about three.
        let roomy = SharedBodyCache::new(Arc::clone(&pool), 64 << 20);
        let mut charges = Vec::new();
        for &id in &ids {
            charges.push(charge(&get(&roomy, id, &calls).await.unwrap()));
        }
        let budget = charges.iter().copied().max().unwrap() * 3;
        let tight = SharedBodyCache::new(Arc::clone(&pool), budget);
        for round in 0..2 {
            for &id in &ids {
                let system = get(&tight, id, &calls).await.unwrap();
                assert_eq!(*system, *get(&roomy, id, &calls).await.unwrap(), "{id:?}");
                let counters = tight.counters().cache();
                assert!(
                    counters.bytes() <= budget,
                    "round {round}: {} bytes held against {budget}",
                    counters.bytes()
                );
            }
        }
        let counters = tight.counters();
        assert!(counters.cache().evictions() > 0, "{counters:?}");
        assert!(counters.cache().entries() < ids.len());
        // Every miss of the tight cache generated, since nothing ran concurrently.
        assert_eq!(counters.generated(), counters.cache().misses());
        pool.shutdown().await.unwrap();
    }

    /// What the cache charges an entry of event models.
    fn events_charge(models: &SystemEvents) -> usize {
        models.heap_bytes() + size_of::<SystemEvents>() + ENTRY_OVERHEAD_BYTES
    }

    async fn events(
        cache: &SharedBodyCache,
        id: SystemId,
        system: &Arc<GeneratedSystem>,
    ) -> Arc<SystemEvents> {
        timeout(WAIT, cache.events_of(key(), id, Arc::clone(system)))
            .await
            .expect("timed out building a system's event models")
            .unwrap()
    }

    /// P14.T36.b, test (a): two requests for one system's events at once build its models once
    /// (a counter, not timing), the sim's own, and a later one is a hit.
    #[tokio::test]
    async fn two_concurrent_requests_for_one_system_s_events_model_it_once() {
        let pool = pool();
        let cache = SharedBodyCache::new(Arc::clone(&pool), 64 << 20);
        let calls = Arc::new(AtomicUsize::new(0));
        let id = systems(1)[0];
        let system = get(&cache, id, &calls).await.unwrap();
        // Whether the second joins the first's flight or finds what it stored, one model is built.
        let (first, second) =
            tokio::join!(events(&cache, id, &system), events(&cache, id, &system));
        assert!(Arc::ptr_eq(&first, &second), "both hold the one model");
        let counters = cache.counters();
        assert_eq!(counters.modelled(), 1);
        assert_eq!(counters.events().entries(), 1);
        assert_eq!(counters.events().bytes(), events_charge(&first));
        let (ctx, planets) = system.as_ref();
        assert_eq!(*first, planets.event_models(ctx));
        let third = events(&cache, id, &system).await;
        assert!(Arc::ptr_eq(&first, &third));
        assert_eq!(cache.counters().modelled(), 1);
        assert!(cache.counters().events().hits() >= 1);
        // The systems' own counters are untouched by the models.
        assert_eq!(cache.counters().generated(), 1);
        pool.shutdown().await.unwrap();
    }

    /// P14.T36.b, test (a): event models past their byte bound are evicted, the bound is never
    /// exceeded, and an evicted system's models are built again, the same.
    #[tokio::test]
    async fn event_models_past_the_byte_bound_are_evicted_and_the_bound_is_never_exceeded() {
        let pool = pool();
        let ids = systems(12);
        let calls = Arc::new(AtomicUsize::new(0));
        let roomy = SharedBodyCache::new(Arc::clone(&pool), 64 << 20);
        let mut generated = Vec::new();
        let mut charges = Vec::new();
        for &id in &ids {
            let system = get(&roomy, id, &calls).await.unwrap();
            charges.push(events_charge(&*events(&roomy, id, &system).await));
            generated.push(system);
        }
        let budget = charges.iter().copied().max().unwrap() * 3;
        let tight = SharedBodyCache::new(Arc::clone(&pool), budget);
        for round in 0..2 {
            for (&id, system) in ids.iter().zip(&generated) {
                let models = events(&tight, id, system).await;
                assert_eq!(*models, *events(&roomy, id, system).await, "{id:?}");
                let counters = tight.counters().events();
                assert!(
                    counters.bytes() <= budget,
                    "round {round}: {} bytes held against {budget}",
                    counters.bytes()
                );
            }
        }
        let counters = tight.counters();
        assert!(counters.events().evictions() > 0, "{counters:?}");
        assert!(counters.events().entries() < ids.len());
        assert_eq!(counters.modelled(), counters.events().misses());
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn an_id_that_names_no_system_is_refused_and_not_stored() {
        let pool = pool();
        let cache = SharedBodyCache::new(Arc::clone(&pool), 64 << 20);
        let calls = Arc::new(AtomicUsize::new(0));
        let cell = CellKey::of(systems(1)[0]).expect("a grid ID");
        let beyond = cell
            .candidate_id(candidate_count(galaxy(), cell))
            .expect("a cell at the solar circle has room for another index");
        assert_eq!(
            get(&cache, beyond, &calls).await.unwrap_err(),
            GenerateBodiesError::NoSuchSystem(ResolveSystemError::NoSuchSystem)
        );
        let counters = cache.counters();
        assert_eq!((counters.cache().entries(), counters.generated()), (0, 0));
        pool.shutdown().await.unwrap();
    }
}
