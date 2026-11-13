//! The alert service's scans on the CPU pool (plan 12, P12.T8.b; Design notes 7 and 8).
//!
//! [`AlertScans`] runs the walks a subscription's scans ask for ([`scan`](super::scan)), each
//! shared by every subscription that asks for the same one: a [`SharedByteLru`] of their results
//! keyed by [`ScanKey`], under a [`SingleFlight`], so two subscriptions at one place with one
//! sensor, classes and window share one walk. A scan's interactive classes and local events are
//! one `Priority::Interactive` job. A class its census sends to the background is planned in one
//! `Priority::Bulk` job, which builds its process once; counted in at most
//! [`MAX_BACKGROUND_JOBS`] bulk jobs of [`SLABS_PER_JOB`] x-slabs each, which list their slabs'
//! occupied cells and split a merger class's dense ones by candidate index; and walked by bulk
//! jobs that take its pieces in turn, as many at once as the pool has workers, each for about
//! [`JOB_BUDGET`] before it hands its worker back and another is queued (plan 12, the mergers'
//! balance). All run under one [`CancelToken`] and share the process (P12.T8.b.2). It is complete
//! only when every job has finished: one cancelled or failed fails the class's walk, which yields
//! nothing (Design note 7). A walk whose last waiter gives up is cancelled, its jobs still queued
//! skipped and its running ones stopped at their next slab or piece.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::Duration;

use futures_util::future::{self, BoxFuture};
use futures_util::stream::FuturesUnordered;
use futures_util::{FutureExt, StreamExt};
use hyperion_sim::alerts::LocalEventsCache;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::ClassId;

use super::scan::{
    BackgroundWork, CellParts, JOB_BUDGET, MAX_BACKGROUND_JOBS, SLABS_PER_JOB, ScanKey, ScanResult,
    ScanSpec, background_plan, census_slabs, walk_interactive, walk_pieces,
};
use crate::AppState;
use crate::cache::{LruCounters, SharedByteLru};
use crate::compute::{CancelOnDrop, CancelToken, ComputeError, JobError, Priority, SingleFlight};

/// A walk's key in the service's cache: a scan's interactive walk, or one class's background walk
/// for a scan.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum WalkKey {
    /// The scan's interactive classes and local events.
    Interactive(ScanKey),
    /// One class of the scan's, in the background.
    Background(ScanKey, ClassId),
}

/// A walk's result, as the service hands it back.
pub(crate) type Walked = Result<Arc<ScanResult>, ComputeError>;

/// The alert scans the server has walked, in one byte budget, shared by every subscription (module
/// documentation).
pub(crate) struct AlertScans {
    walks: Arc<SharedByteLru<WalkKey, ScanResult>>,
    flights: SingleFlight<WalkKey, ScanResult, ComputeError>,
    /// The x-slabs each background census job counts, at the least.
    slabs_per_job: usize,
    /// How long each background walk's job takes pieces for.
    job_budget: Duration,
    /// The walks run so far, interactive and background, each counted once its first job starts.
    walked: Arc<AtomicU64>,
}

impl AlertScans {
    /// A service keeping at most `budget_bytes` of walks, whose background census jobs count
    /// [`SLABS_PER_JOB`] x-slabs each.
    #[must_use]
    pub(crate) fn new(budget_bytes: usize) -> Self {
        Self::with_slabs_per_job(budget_bytes, SLABS_PER_JOB)
    }

    /// [`new`](Self::new), with `slabs_per_job` x-slabs in each background census job, at least
    /// one.
    #[must_use]
    pub(crate) fn with_slabs_per_job(budget_bytes: usize, slabs_per_job: usize) -> Self {
        Self {
            walks: Arc::new(SharedByteLru::new(budget_bytes)),
            flights: SingleFlight::new(),
            slabs_per_job: slabs_per_job.max(1),
            job_budget: JOB_BUDGET,
            walked: Arc::new(AtomicU64::new(0)),
        }
    }

    /// This service, its background walks' jobs each taking pieces for `budget`: one piece each
    /// for none.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn with_job_budget(mut self, budget: Duration) -> Self {
        self.job_budget = budget;
        self
    }

    /// The interactive walk of `spec` in `galaxy`, the galaxy its key names, through `state`'s
    /// caches and a subscription's `local` cache: held, joined, or run as one interactive job.
    ///
    /// `local` is the asking subscription's; a walk another subscription started uses that one's,
    /// and a walk that finds its cache in use by another walk uses none. A cache changes no result,
    /// only its cost (P12.T5.e).
    ///
    /// The future fails with [`ComputeError::Submit`] if the pool refuses the job and
    /// [`ComputeError::Job`] if it was cancelled, panicked or lost.
    pub(crate) fn interactive(
        &self,
        state: &Arc<AppState>,
        galaxy: &Arc<Galaxy>,
        spec: &ScanSpec,
        local: &Arc<Mutex<LocalEventsCache>>,
    ) -> BoxFuture<'static, Walked> {
        let key = WalkKey::Interactive(spec.key());
        if let Some(walked) = self.walks.get(&key) {
            return future::ready(Ok(walked)).boxed();
        }
        let (state, galaxy, spec, local) = (
            Arc::clone(state),
            Arc::clone(galaxy),
            spec.clone(),
            Arc::clone(local),
        );
        let (walks, walked) = (Arc::clone(&self.walks), Arc::clone(&self.walked));
        self.flights
            .run(key.clone(), move || async move {
                // As in `DensityMapService::get`: a caller can arrive just after another flight
                // has stored its walk and left the registry.
                if let Some(held) = walks.get(&key) {
                    return Ok(held);
                }
                let token = CancelToken::new();
                // Skips the job if every waiter gives up before a worker takes it.
                let _cancel_on_drop = CancelOnDrop::new(token.clone());
                let pool = Arc::clone(&state.pool);
                let job = move |_: &CancelToken| {
                    walked.fetch_add(1, Ordering::Relaxed);
                    let mut spare = LocalEventsCache::default();
                    let mut held = match local.try_lock() {
                        Ok(guard) => Some(guard),
                        // A walk that panicked left the cache's map whole, and a cache changes
                        // no result.
                        Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
                        Err(TryLockError::WouldBlock) => None,
                    };
                    let cache = held.as_deref_mut().unwrap_or(&mut spare);
                    walk_interactive(&state, &galaxy, &spec, cache)
                };
                let receiver = pool.submit(Priority::Interactive, token, job).await?;
                let result = Arc::new(
                    receiver
                        .await
                        .unwrap_or_else(|closed| Err(JobError::from(closed)))?,
                );
                // A walk larger than the whole budget is handed back and not stored.
                drop(walks.insert(key, Arc::clone(&result)));
                Ok(result)
            })
            .boxed()
    }

    /// The background walk of `class` for `spec` in `galaxy`, through `state`'s caches: held,
    /// joined, or planned in one bulk job, counted in bulk jobs of the service's slabs each, and
    /// walked in bulk jobs that take its pieces in turn (module documentation).
    ///
    /// The future fails as [`interactive`](Self::interactive)'s does, for any of the jobs: the
    /// class's walk is whole or not at all. A class no background walk reads in cells is not
    /// walked ([`ScanResult::unwalked`]).
    pub(crate) fn background(
        &self,
        state: &Arc<AppState>,
        galaxy: &Arc<Galaxy>,
        spec: &ScanSpec,
        class: ClassId,
    ) -> BoxFuture<'static, Walked> {
        let key = WalkKey::Background(spec.key(), class);
        if let Some(walked) = self.walks.get(&key) {
            return future::ready(Ok(walked)).boxed();
        }
        let (state, galaxy, spec) = (Arc::clone(state), Arc::clone(galaxy), spec.clone());
        let (walks, walked) = (Arc::clone(&self.walks), Arc::clone(&self.walked));
        let (per_job, budget) = (self.slabs_per_job, self.job_budget);
        self.flights
            .run(key.clone(), move || async move {
                if let Some(held) = walks.get(&key) {
                    return Ok(held);
                }
                let token = CancelToken::new();
                // Skips the jobs still queued, and stops those running at their next cell, once
                // every waiter has gone.
                let _cancel_on_drop = CancelOnDrop::new(token.clone());
                let pool = Arc::clone(&state.pool);
                let plan = {
                    let (galaxy, spec) = (Arc::clone(&galaxy), spec.clone());
                    pool.submit(Priority::Bulk, token.clone(), move |_: &CancelToken| {
                        walked.fetch_add(1, Ordering::Relaxed);
                        background_plan(&galaxy, &spec, class, per_job)
                    })
                    .await?
                };
                let planned = plan
                    .await
                    .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
                let Some(plan) = planned.map(Arc::new) else {
                    tracing::error!(
                        ?class,
                        "a background walk of a class no walk reads in cells"
                    );
                    let unwalked = Arc::new(ScanResult::unwalked());
                    drop(walks.insert(key, Arc::clone(&unwalked)));
                    return Ok(unwalked);
                };
                // The census: each job its slabs' occupied cells, a few milliseconds a slab.
                let mut census = Vec::with_capacity(plan.jobs().len());
                for slabs in plan.jobs().iter().cloned() {
                    let (galaxy, plan) = (Arc::clone(&galaxy), Arc::clone(&plan));
                    census.push(
                        pool.submit(Priority::Bulk, token.clone(), move |token: &CancelToken| {
                            census_slabs(&galaxy, &plan, slabs, CellParts::SERVICE, token)
                        })
                        .await?,
                    );
                }
                let mut counted = Vec::with_capacity(census.len());
                for job in census {
                    counted.push(
                        job.await
                            .unwrap_or_else(|closed| Err(JobError::from(closed)))?
                            .ok_or(JobError::Cancelled)?,
                    );
                }
                let work = Arc::new(BackgroundWork::new(counted));
                // The walk: as many jobs at once as the pool has workers, each taking pieces for
                // its budget, and one more queued as each hands its worker back with pieces left.
                let submit = || {
                    let (state, galaxy, spec, plan, work) = (
                        Arc::clone(&state),
                        Arc::clone(&galaxy),
                        spec.clone(),
                        Arc::clone(&plan),
                        Arc::clone(&work),
                    );
                    pool.submit(Priority::Bulk, token.clone(), move |token: &CancelToken| {
                        walk_pieces(&state, &galaxy, &spec, &plan, &work, budget, token)
                    })
                };
                let at_once = pool
                    .workers()
                    .get()
                    .min(MAX_BACKGROUND_JOBS)
                    .min(work.pieces().len());
                let mut running = FuturesUnordered::new();
                for _ in 0..at_once {
                    running.push(submit().await?);
                }
                let mut parts = Vec::new();
                while let Some(job) = running.next().await {
                    parts.push(
                        job.unwrap_or_else(|closed| Err(JobError::from(closed)))?
                            .ok_or(JobError::Cancelled)?,
                    );
                    if !work.is_taken() {
                        running.push(submit().await?);
                    }
                }
                let result = Arc::new(ScanResult::of_pieces(&work, parts));
                drop(walks.insert(key, Arc::clone(&result)));
                Ok(result)
            })
            .boxed()
    }

    /// The walks held, the bytes they are charged, and the hits, misses, evictions and refusals so
    /// far.
    #[must_use]
    pub(crate) fn counters(&self) -> LruCounters {
        self.walks.counters()
    }

    /// The walks run so far, interactive and background.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn walked(&self) -> u64 {
        self.walked.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for AlertScans {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AlertScans")
            .field("counters", &self.counters())
            .field("walking", &self.flights.len())
            .field("slabs_per_job", &self.slabs_per_job)
            .field("job_budget", &self.job_budget)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
