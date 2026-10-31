//! A galaxy's global list and its streams' tube tables, made on the CPU pool's workers (plan 10,
//! P10.T5.b).
//!
//! [`GlobalList::build`] takes some 0.49 s on the Milky Way fixture, and [`TubeTable::build`]
//! 1.3–2.0 s a stream, on one thread (pod, 16 vCPU, 2026-10-30). The sim spawns no threads, so it
//! hands out pure units of work, and these functions run them on the pool's workers and gather
//! them by index:
//!
//! - **The list** ([`global_list_on`]). The galaxy's centre ([`Galaxy::centre`], some 0.11 s cold)
//!   is built on one worker. Beside it, the debris is planned ([`Debris::plan`]) and the galaxy's
//!   [`SprayForce`] built if [`SharedSprayForces`] lacks it. Then the orphans are drawn in shares
//!   of their indices ([`DebrisPlan::orphan`]), dealt out in turn. One job then assembles the
//!   debris and its list's plan ([`ListPlan::of_debris`]), and deals the plan's shell orbits out
//!   by their steps, the longest first, to the share with the fewest. The legs are stepped in
//!   those shares ([`radii_over_ly`]), and once the centre is built a last job finishes the list
//!   ([`ListPlan::finish`]).
//! - **A tube** ([`tube_table_on`]). Its tracers are sprayed in shares of 250 indices dealt out in
//!   turn ([`Spray::tracers_at`]), so that each share holds early releases, which have the most
//!   steps to take, and late ones alike. Each job makes its own [`Spray`] (the progenitor's two
//!   passes, some 0.15 % of the spray), and a last job reduces them ([`TubeTable::from_tracers`]).
//!   Shares of runs of 250 indices took stream 0 of the Milky Way fixture 0.47 s on eight workers,
//!   the first run holding some 1.9 times the mean's steps (pod, 16 vCPU, 2026-10-31).
//!
//! **Bit for bit.** Each unit is a pure function of the galaxy and its index: an orphan of `k`,
//! every one of its draws keyed by `k`; a leg's radii and a tracer whatever they are stepped
//! beside. The gather is in index order, so the list and the tables are the serial builds', at any
//! number of workers and in any shares. Each worker checks its floating-point mode before every
//! reply ([`CpuPool`]), so a spray table is kept only once its job's reply has passed, and the
//! galaxy and the table are shared read-only.
//!
//! **No job waits on another.** As for a range query ([`served_range_query`]): the pool's queues
//! are first-in, first-out and a worker runs one job to its end, so a job that blocked on jobs it
//! had queued could stall every worker. Every job here is queued by the caller's own task, which
//! awaits their replies on the runtime, and a job only plans, draws, steps, sprays or assembles.
//! No kind of unit is dealt into more shares than the pool has workers, so a build holds at most
//! its workers' count and three more jobs in the queue at once.
//!
//! **Kept** ([`GlobalListService`], P10.T10.a). A galaxy's list is built once, on its first
//! `global_features` or `stream_track`, and kept within a byte budget; a stream's tube is built for
//! the `stream_track` that asks, at interactive priority, and not kept (P10.T10.b's cache will). A
//! list is a pure function of its galaxy, so eviction never changes an answer.
//!
//! [`served_range_query`]: super::served_range_query
//! [`DebrisPlan::orphan`]: hyperion_sim::galaxy::global_list::DebrisPlan::orphan

use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;

use hyperion_sim::galaxy::global_list::orbit::BuildLeapfrogError;
use hyperion_sim::galaxy::global_list::spray::{
    Leg, Spray, SprayForce, SprayParams, Tracer, radii_over_ly,
};
use hyperion_sim::galaxy::global_list::tube::BuildTubeTableError;
use hyperion_sim::galaxy::global_list::{Debris, GlobalList, ListPlan, StreamSpec, TubeTable};
use hyperion_sim::galaxy::{Galaxy, Population};

use super::{
    CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyCache, GalaxyKey, JobError,
    JobReceiver, Priority, SingleFlight,
};
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// The tracers of one share of a tube's spray: 250, eight shares of a stream's 2,000 on eight
/// workers, about 0.25 s each (perf-spray's recorded design, 2026-10-30).
pub const TRACERS_PER_SHARE: NonZeroU32 = NonZeroU32::new(250).expect("250 is not zero");

/// The spray tables of the galaxies the server holds, one per galaxy, by its key.
///
/// A table is some 160 kB and 12 ms to build ([`SprayForce::new`]): what the list's shell orbits
/// and every tube's spray step in.
#[derive(Debug)]
pub struct SharedSprayForces {
    forces: SharedByteLru<GalaxyKey, SprayForce>,
}

impl HeapBytes for SprayForce {
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

impl SharedSprayForces {
    /// An empty cache that holds at most `budget_bytes` of charged tables.
    ///
    /// Zero keeps none: each build then makes its own.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            forces: SharedByteLru::new(budget_bytes),
        }
    }

    /// The table of the galaxy `key` names, if held.
    #[must_use]
    pub fn get(&self, key: GalaxyKey) -> Option<Arc<SprayForce>> {
        self.forces.get(&key)
    }
}

/// How many shares each kind of unit is dealt into: the orphans, the legs and a tube's tracers.
///
/// A build deals no kind into more shares than its pool has workers, whatever these ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shares {
    orphans: NonZeroUsize,
    legs: NonZeroUsize,
    tracers_per_share: NonZeroU32,
}

impl Shares {
    /// One share of orphans and of legs for each of `workers` workers, and
    /// [`TRACERS_PER_SHARE`] tracers a share: the design's schedule.
    #[must_use]
    pub const fn for_workers(workers: NonZeroUsize) -> Self {
        Self {
            orphans: workers,
            legs: workers,
            tracers_per_share: TRACERS_PER_SHARE,
        }
    }

    /// `orphans` shares of orphans and `legs` of legs, and `tracers_per_share` tracers a share.
    #[must_use]
    pub const fn new(
        orphans: NonZeroUsize,
        legs: NonZeroUsize,
        tracers_per_share: NonZeroU32,
    ) -> Self {
        Self {
            orphans,
            legs,
            tracers_per_share,
        }
    }
}

/// A tube's table could not be made on the pool.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildTubeOnPoolError {
    /// One of the build's own jobs could not run.
    Compute(ComputeError),
    /// The table could not be made, as [`TubeTable::build`]'s error.
    Table(BuildTubeTableError),
}

impl std::fmt::Display for BuildTubeOnPoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compute(_) => f.write_str("the tube's jobs could not run"),
            Self::Table(_) => f.write_str("the tube's table could not be made"),
        }
    }
}

impl std::error::Error for BuildTubeOnPoolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compute(error) => Some(error),
            Self::Table(error) => Some(error),
        }
    }
}

impl From<ComputeError> for BuildTubeOnPoolError {
    fn from(error: ComputeError) -> Self {
        Self::Compute(error)
    }
}

impl From<BuildLeapfrogError> for BuildTubeOnPoolError {
    fn from(error: BuildLeapfrogError) -> Self {
        Self::Table(BuildTubeTableError::Spray(error))
    }
}

/// Panics unless `key` names `galaxy`: its spray table is kept under the key.
fn check_key(galaxy: &Galaxy, key: GalaxyKey) {
    assert_eq!(
        galaxy.seed().get(),
        key.seed(),
        "a galaxy's spray table is kept under its own key, and this is another's"
    );
}

/// `job` queued on `pool` at `priority` behind `token`, waiting for room in its queue.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the pool is shutting down or faulted.
async fn queue<R: Send + 'static>(
    pool: &CpuPool,
    priority: Priority,
    token: &CancelToken,
    job: impl FnOnce(&CancelToken) -> R + Send + 'static,
) -> Result<JobReceiver<R>, ComputeError> {
    pool.submit(priority, token.clone(), job)
        .await
        .map_err(ComputeError::Submit)
}

/// What a job sent back, or why it sent nothing.
async fn reply<R>(receiver: JobReceiver<R>) -> Result<R, ComputeError> {
    receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))
        .map_err(ComputeError::Job)
}

/// The spray table of `galaxy`, the galaxy `key` names: `forces`' if it holds it, or one built on
/// a job queued now, kept in `forces` once ready.
async fn force_of(
    pool: &CpuPool,
    forces: &Arc<SharedSprayForces>,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    priority: Priority,
    token: &CancelToken,
) -> Result<Force, ComputeError> {
    if let Some(force) = forces.get(key) {
        return Ok(Force::Held(force));
    }
    let galaxy = Arc::clone(galaxy);
    let receiver = queue(pool, priority, token, move |_: &CancelToken| {
        SprayForce::new(galaxy.potential()).map(Arc::new)
    })
    .await?;
    Ok(Force::Building {
        receiver,
        forces: Arc::clone(forces),
        key,
    })
}

/// A galaxy's spray table, held or being built.
enum Force {
    Held(Arc<SprayForce>),
    Building {
        receiver: JobReceiver<Result<Arc<SprayForce>, BuildLeapfrogError>>,
        /// Where it is kept once its job's reply has passed the pool's floating-point check.
        forces: Arc<SharedSprayForces>,
        key: GalaxyKey,
    },
}

impl Force {
    /// The table, once built and kept.
    async fn ready(self) -> Result<Result<Arc<SprayForce>, BuildLeapfrogError>, ComputeError> {
        match self {
            Self::Held(force) => Ok(Ok(force)),
            Self::Building {
                receiver,
                forces,
                key,
            } => {
                let built = reply(receiver).await?;
                if let Ok(force) = &built {
                    // A table the cache refuses is this build's alone.
                    drop(forces.forces.insert(key, Arc::clone(force)));
                }
                Ok(built)
            }
        }
    }
}

/// `count` indices dealt out in turn among `shares` shares, but never more shares than indices:
/// share `j` holds `j`, `j + n`, `j + 2n` and so on, each in order.
#[must_use]
fn dealt(count: u64, shares: NonZeroUsize) -> Vec<Vec<u64>> {
    let n = u64::try_from(shares.get()).unwrap_or(u64::MAX).min(count);
    let stride = usize::try_from(n).expect("at most the shares, a usize");
    (0..n)
        .map(|first| (first..count).step_by(stride).collect())
        .collect()
}

/// `legs` dealt out among `shares` shares by their steps, each leg from the longest in turn to
/// the share with the fewest steps so far, the first such on a tie: indices into `legs`, each
/// share in the order dealt, the longest first.
#[must_use]
fn by_steps(legs: &[Leg], shares: NonZeroUsize) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..legs.len()).collect();
    order.sort_by_key(|&n| (std::cmp::Reverse(legs[n].steps()), n));
    let mut dealt: Vec<(u64, Vec<usize>)> = vec![(0, Vec::new()); shares.get().min(legs.len())];
    for n in order {
        let Some(lightest) = dealt.iter_mut().min_by_key(|(steps, _)| *steps) else {
            break;
        };
        lightest.0 += u64::from(legs[n].steps());
        lightest.1.push(n);
    }
    dealt.into_iter().map(|(_, share)| share).collect()
}

/// The global list of `galaxy`, the galaxy `key` names, made on `pool`'s workers at `priority` in
/// `shares`, but never more shares than workers (module documentation).
///
/// It is [`GlobalList::build`]'s, bit for bit. The galaxy's centre is built and kept in it, and
/// its spray table kept in `forces`. Each job waits for room in its queue, as a galaxy's build is
/// no request of its own.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the pool is shutting down or faulted, and [`ComputeError::Job`] if
/// one of the build's jobs is cancelled, panics, is lost with the pool or leaves its worker's
/// floating-point mode flushing subnormals: a panic where [`GlobalList::build`] panics, for a
/// galaxy without its full potential among them.
///
/// # Panics
///
/// If `key` is not of `galaxy`'s seed, or if the galaxy's spray table cannot be built though its
/// debris could be planned, which a galaxy with the kinematic tables the debris needs never gives.
pub async fn global_list_on(
    pool: &CpuPool,
    forces: &Arc<SharedSprayForces>,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    (priority, shares): (Priority, Shares),
    token: &CancelToken,
) -> Result<GlobalList, ComputeError> {
    check_key(galaxy, key);
    let workers = pool.workers();
    let at = |galaxy: &Arc<Galaxy>| Arc::clone(galaxy);
    let centre = {
        let galaxy = at(galaxy);
        // Its model is kept in the galaxy; the finish reads it, and fails as it does if it fails.
        queue(pool, priority, token, move |_: &CancelToken| {
            galaxy.centre().is_ok()
        })
        .await?
    };
    let planned = {
        let galaxy = at(galaxy);
        queue(pool, priority, token, move |_: &CancelToken| {
            Arc::new(Debris::plan(&galaxy))
        })
        .await?
    };
    let force = force_of(pool, forces, (galaxy, key), priority, token).await?;
    let plan = reply(planned).await?;
    let orphan_shares = dealt(plan.orphan_count(), shares.orphans.min(workers));
    let mut drawing = Vec::with_capacity(orphan_shares.len());
    for share in orphan_shares {
        let (galaxy, plan) = (at(galaxy), Arc::clone(&plan));
        let job = move |_: &CancelToken| {
            share
                .into_iter()
                .map(|k| (k, plan.orphan(&galaxy, k)))
                .collect::<Vec<_>>()
        };
        drawing.push(queue(pool, priority, token, job).await?);
    }
    let mut orphans: Vec<Option<StreamSpec>> = vec![
        None;
        usize::try_from(plan.orphan_count()).expect(
            "a galaxy's orphans number far fewer than a usize holds"
        )
    ];
    for receiver in drawing {
        for (k, orphan) in reply(receiver).await? {
            orphans[usize::try_from(k).expect("an index of the vector above")] = orphan;
        }
    }
    let assembled = {
        let legs = shares.legs.min(workers);
        queue(pool, priority, token, move |_: &CancelToken| {
            let list = ListPlan::of_debris(Arc::unwrap_or_clone(plan).finish(orphans));
            let dealt = by_steps(list.legs(), legs);
            (Arc::new(list), dealt)
        })
        .await?
    };
    let (list, dealt) = reply(assembled).await?;
    let force = force
        .ready()
        .await?
        .expect("a galaxy whose debris is planned has the kinematic tables the force needs");
    let mut stepping = Vec::with_capacity(dealt.len());
    for share in dealt {
        let (list, force) = (Arc::clone(&list), Arc::clone(&force));
        let job = move |_: &CancelToken| {
            let legs: Vec<Leg> = share.iter().map(|&n| list.legs()[n]).collect();
            share
                .into_iter()
                .zip(radii_over_ly(&force, &legs))
                .collect::<Vec<_>>()
        };
        stepping.push(queue(pool, priority, token, job).await?);
    }
    let mut radii = vec![[f64::NAN; 2]; list.legs().len()];
    for receiver in stepping {
        for (n, r) in reply(receiver).await? {
            radii[n] = r;
        }
    }
    // The centre is built before the finish reads it, so that no job waits on another's; what its
    // job sent back is the finish's to find again.
    let _ = reply(centre).await?;
    let galaxy = at(galaxy);
    let finished = queue(pool, priority, token, move |_: &CancelToken| {
        Arc::unwrap_or_clone(list).finish(&galaxy, &radii)
    })
    .await?;
    reply(finished).await
}

/// The tube table of `spec`'s stream in `galaxy`, the galaxy `key` names, its tracers sprayed on
/// `pool`'s workers at `priority` in `shares`' shares of tracers (module documentation).
///
/// It is [`TubeTable::build`]'s, bit for bit. The galaxy's spray table is kept in `forces`.
///
/// # Errors
///
/// [`BuildTubeOnPoolError::Table`] as [`TubeTable::build`]'s error, and
/// [`BuildTubeOnPoolError::Compute`] as [`global_list_on`]'s.
///
/// # Panics
///
/// If `key` is not of `galaxy`'s seed.
pub async fn tube_table_on(
    pool: &CpuPool,
    forces: &Arc<SharedSprayForces>,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    spec: &StreamSpec,
    (priority, shares): (Priority, Shares),
    token: &CancelToken,
) -> Result<TubeTable, BuildTubeOnPoolError> {
    let tracers = tracers_on(
        pool,
        forces,
        (galaxy, key),
        (spec, SprayParams::default()),
        (priority, shares),
        token,
    )
    .await?;
    let (galaxy, spec) = (Arc::clone(galaxy), spec.clone());
    let reduced = queue(pool, priority, token, move |_: &CancelToken| {
        TubeTable::from_tracers(
            galaxy.seed(),
            galaxy.mean_system_mass(Population::Halo),
            &spec,
            &tracers,
        )
    })
    .await?;
    reply(reduced).await?.map_err(BuildTubeOnPoolError::Table)
}

/// The tracers of `spec`'s stream in `galaxy`, the galaxy `key` names, at `params`, in index order,
/// sprayed on `pool`'s workers at `priority` in shares of at most `shares`' tracers a share, or as
/// many as keep the shares to the workers' count, the indices dealt out in turn and gathered by
/// index: [`Spray::tracers`]'s, bit for bit.
///
/// # Errors
///
/// [`BuildTubeOnPoolError::Table`] with [`BuildTubeTableError::Spray`] as [`Spray::new`]'s error,
/// and [`BuildTubeOnPoolError::Compute`] as [`global_list_on`]'s.
///
/// # Panics
///
/// If `key` is not of `galaxy`'s seed.
pub(crate) async fn tracers_on(
    pool: &CpuPool,
    forces: &Arc<SharedSprayForces>,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    (spec, params): (&StreamSpec, SprayParams),
    (priority, shares): (Priority, Shares),
    token: &CancelToken,
) -> Result<Vec<Tracer>, BuildTubeOnPoolError> {
    check_key(galaxy, key);
    let force = force_of(pool, forces, (galaxy, key), priority, token)
        .await?
        .ready()
        .await??;
    let count = params.tracers();
    let workers = u64::try_from(pool.workers().get()).unwrap_or(u64::MAX);
    let wanted = u64::from(count.div_ceil(shares.tracers_per_share.get()));
    let share_count = NonZeroUsize::new(
        usize::try_from(wanted.min(workers)).expect("at most the workers, a usize"),
    )
    .unwrap_or(NonZeroUsize::MIN);
    let dealt = dealt(u64::from(count), share_count);
    let mut spraying = Vec::with_capacity(dealt.len());
    for share in dealt {
        let indices: Vec<u32> = share
            .into_iter()
            .map(|i| u32::try_from(i).expect("an index below the tracers' u32 count"))
            .collect();
        let (galaxy, force, spec) = (Arc::clone(galaxy), Arc::clone(&force), spec.clone());
        let job = move |_: &CancelToken| {
            Spray::new(galaxy.seed(), &force, galaxy.potential(), &spec, &params)
                .map(|spray| indices.iter().copied().zip(spray.tracers_at(&indices)))
                .map(Iterator::collect::<Vec<_>>)
        };
        spraying.push(queue(pool, priority, token, job).await?);
    }
    let mut tracers = vec![None; usize::try_from(count).expect("a u32 fits a usize")];
    for receiver in spraying {
        for (i, tracer) in reply(receiver).await?? {
            tracers[usize::try_from(i).expect("a u32 fits a usize")] = Some(tracer);
        }
    }
    Ok(tracers
        .into_iter()
        .map(|tracer| tracer.expect("every index is dealt to a share"))
        .collect())
}

/// A galaxy's global list, as the [`GlobalListService`] keeps it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeldGlobalList(GlobalList);

impl HeldGlobalList {
    /// The list.
    #[must_use]
    pub fn list(&self) -> &GlobalList {
        &self.0
    }
}

impl HeapBytes for HeldGlobalList {
    /// The list's entries and its debris' specifications, by their counts: none of them owns
    /// anything on the heap itself, and the vectors' capacities are the sim's to keep.
    fn heap_bytes(&self) -> usize {
        let list = &self.0;
        size_of_val(list.entries())
            + size_of_val(list.debris().streams())
            + size_of_val(list.debris().cores())
    }
}

/// The global list of every galaxy the server holds, built on the pool's workers and kept per
/// galaxy, and the streams' tubes built for their requests (module documentation, "Kept").
///
/// [`GlobalListService::get`] hands back a galaxy's list, building it with [`global_list_on`] at
/// [`Priority::Bulk`] if no one has yet, the galaxy taken from the [`GalaxyCache`], which builds
/// every galaxy with the kinematic tables the list needs. Callers that ask for one galaxy at once
/// share one build through a [`SingleFlight`], and if every one of them gives up, its jobs still
/// queued are skipped. [`GlobalListService::tube`] builds a stream's tube with [`tube_table_on`].
/// Both keep each galaxy's spray table in the service's [`SharedSprayForces`].
pub struct GlobalListService {
    pool: Arc<CpuPool>,
    galaxies: Arc<GalaxyCache>,
    forces: Arc<SharedSprayForces>,
    held: Arc<SharedByteLru<GalaxyKey, HeldGlobalList>>,
    flights: SingleFlight<GalaxyKey, HeldGlobalList, ComputeError>,
}

impl GlobalListService {
    /// A service that builds its lists and tubes on `pool` from the galaxies of `galaxies`,
    /// keeping at most `budget_bytes` of lists and their spray tables in `forces`.
    ///
    /// A budget of zero keeps no list.
    #[must_use]
    pub fn new(
        pool: Arc<CpuPool>,
        galaxies: Arc<GalaxyCache>,
        forces: Arc<SharedSprayForces>,
        budget_bytes: usize,
    ) -> Self {
        Self {
            pool,
            galaxies,
            forces,
            held: Arc::new(SharedByteLru::new(budget_bytes)),
            flights: SingleFlight::new(),
        }
    }

    /// The global list of the galaxy `key` names, built if it is not held already.
    ///
    /// # Errors
    ///
    /// [`ComputeError`] as [`GalaxyCache::get`] gives it for the galaxy, and as
    /// [`global_list_on`] gives it for the build.
    pub async fn get(&self, key: GalaxyKey) -> Result<Arc<HeldGlobalList>, ComputeError> {
        if let Some(list) = self.held.get(&key) {
            return Ok(list);
        }
        let (pool, galaxies, forces, held) = (
            Arc::clone(&self.pool),
            Arc::clone(&self.galaxies),
            Arc::clone(&self.forces),
            Arc::clone(&self.held),
        );
        self.flights
            .run(key, move || async move {
                // As the feature lists' service: a caller can arrive just after another flight has
                // finished, and the list it built is worth more than a second build.
                if let Some(list) = held.get(&key) {
                    return Ok(list);
                }
                let galaxy = galaxies.get(key).await?;
                let token = CancelToken::new();
                // Cancels the jobs still queued once every waiter on this flight has gone.
                let _cancel_on_drop = CancelOnDrop::new(token.clone());
                let shares = Shares::for_workers(pool.workers());
                let list = global_list_on(
                    &pool,
                    &forces,
                    (&galaxy, key),
                    (Priority::Bulk, shares),
                    &token,
                )
                .await?;
                let list = Arc::new(HeldGlobalList(list));
                // A list larger than the whole budget is handed back and not stored.
                drop(held.insert(key, Arc::clone(&list)));
                Ok(list)
            })
            .await
    }

    /// The tube of `spec`'s stream in `galaxy`, the galaxy `key` names, its tracers sprayed on the
    /// pool's workers at [`Priority::Interactive`] behind `token`, the request's: not kept.
    ///
    /// # Errors
    ///
    /// As [`tube_table_on`].
    ///
    /// # Panics
    ///
    /// If `key` is not of `galaxy`'s seed.
    pub async fn tube(
        &self,
        (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
        spec: &StreamSpec,
        token: &CancelToken,
    ) -> Result<TubeTable, BuildTubeOnPoolError> {
        let shares = Shares::for_workers(self.pool.workers());
        tube_table_on(
            &self.pool,
            &self.forces,
            (galaxy, key),
            spec,
            (Priority::Interactive, shares),
            token,
        )
        .await
    }

    /// The lists held, the bytes they are charged, and the hits, misses, evictions and refusals
    /// so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.held.counters()
    }
}

impl std::fmt::Debug for GlobalListService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalListService")
            .field("counters", &self.counters())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hyperion_sim::GENERATOR_VERSION;
    use hyperion_sim::galaxy::global_list::testing::pal5_like;

    use std::sync::OnceLock;

    use hyperion_sim::Seed;

    use super::*;
    use crate::convert::origins::SEED;
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};

    fn key() -> GalaxyKey {
        GalaxyKey::new(SEED, GENERATOR_VERSION)
    }

    fn workers(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).expect("at least one")
    }

    /// A pool of `n` workers.
    fn pool(n: usize) -> CpuPool {
        CpuPool::new(workers(n), INTERACTIVE_QUEUE_CAPACITY, BULK_QUEUE_CAPACITY)
            .expect("the pool starts")
    }

    /// `future` within ten minutes, which a stalled pool would never meet.
    async fn within<T>(future: impl Future<Output = T>) -> T {
        tokio::time::timeout(Duration::from_secs(600), future)
            .await
            .expect("the pool does not stall")
    }

    /// A fresh copy of the server tests' galaxy, its centre not yet read: of a galaxy of this
    /// module's own, whose centre nothing reads, so that no other test's reading it can warm it.
    fn cold() -> Arc<Galaxy> {
        static COLD: OnceLock<Galaxy> = OnceLock::new();
        Arc::new(
            COLD.get_or_init(|| Galaxy::new(Seed::new(SEED)).with_full_potential())
                .clone(),
        )
    }

    fn tracers(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).expect("at least one")
    }

    /// On one worker, four and eight, in the design's shares, in one share of each kind and in
    /// more shares than units or workers, the list made on the pool is the serial build's, bit for
    /// bit, whether the galaxy's centre and its spray table were held or not (P10.T5.b).
    #[tokio::test]
    async fn the_list_made_on_the_pool_is_the_built_one() {
        let fresh = cold();
        let expected = GlobalList::build(&cold());
        let mut cases = vec![
            (1, Shares::for_workers(workers(1))),
            (4, Shares::for_workers(workers(4))),
        ];
        cases.extend(
            [
                Shares::for_workers(workers(8)),
                Shares::new(workers(1), workers(1), TRACERS_PER_SHARE),
                Shares::new(workers(3), workers(7), TRACERS_PER_SHARE),
                Shares::new(workers(10_000), workers(10_000), TRACERS_PER_SHARE),
            ]
            .map(|shares| (8, shares)),
        );
        let forces = Arc::new(SharedSprayForces::new(1 << 20));
        for (n, shares) in cases {
            let pool = pool(n);
            for galaxy in [cold(), Arc::clone(&fresh)] {
                let list = within(global_list_on(
                    &pool,
                    &forces,
                    (&galaxy, key()),
                    (Priority::Interactive, shares),
                    &CancelToken::new(),
                ))
                .await
                .expect("the list's jobs run");
                assert_eq!(list, expected, "{n} workers, {shares:?}");
            }
            pool.shutdown().await.unwrap();
        }
        assert!(forces.get(key()).is_some(), "the spray table is kept");
    }

    /// A spray of six tracers an arm made on one worker, four and eight in shares of one, five and
    /// twelve tracers, and on eight in shares of every size, is the spray's own, bit for bit
    /// (P10.T5.b); the shares are widened as far as keeps them to the workers' count.
    #[tokio::test]
    async fn tracers_sprayed_on_the_pool_in_any_shares_are_the_spray_s() {
        let galaxy = cold();
        let spec = pal5_like(galaxy.potential());
        let params = SprayParams::new(std::num::NonZeroU32::new(6).expect("six"));
        let force = SprayForce::new(galaxy.potential()).expect("the full potential");
        let expected = Spray::new(galaxy.seed(), &force, galaxy.potential(), &spec, &params)
            .expect("a finite stripping time")
            .tracers();
        assert_eq!(expected.len(), 12);
        let mut cases: Vec<(usize, u32)> = [1, 4, 8]
            .into_iter()
            .flat_map(|n| [1, 5, 12].map(|per| (n, per)))
            .collect();
        cases.extend((1..=13).map(|per| (8, per)));
        let forces = Arc::new(SharedSprayForces::new(0));
        for (n, per) in cases {
            let pool = pool(n);
            let shares = Shares::new(workers(n), workers(n), tracers(per));
            let tracers = within(tracers_on(
                &pool,
                &forces,
                (&galaxy, key()),
                (&spec, params),
                (Priority::Bulk, shares),
                &CancelToken::new(),
            ))
            .await
            .expect("the spray's jobs run");
            assert_eq!(tracers, expected, "{n} workers, {per} tracers a share");
            pool.shutdown().await.unwrap();
        }
    }

    /// On one worker, four and eight, `pal5_like`'s tube table made on the pool is
    /// [`TubeTable::build`]'s, bit for bit (P10.T5.b).
    #[tokio::test]
    #[ignore = "slow: a tube's 2,000 tracers four times, some 8 core-seconds"]
    async fn a_tube_made_on_the_pool_is_the_built_one() {
        let galaxy = cold();
        let spec = pal5_like(galaxy.potential());
        let expected = TubeTable::build(&galaxy, &spec).expect("the full potential");
        let forces = Arc::new(SharedSprayForces::new(1 << 20));
        for n in [1, 4, 8] {
            let pool = pool(n);
            let table = within(tube_table_on(
                &pool,
                &forces,
                (&galaxy, key()),
                &spec,
                (Priority::Interactive, Shares::for_workers(workers(n))),
                &CancelToken::new(),
            ))
            .await
            .expect("the tube's jobs run");
            assert_eq!(table, expected, "{n} workers");
            pool.shutdown().await.unwrap();
        }
    }
}
