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
//! - **A tube** ([`tube_table_on`]). Its tracers are sprayed in shares of 250 indices
//!   ([`Spray::tracers_in`]), each job making its own [`Spray`] (the progenitor's two passes, some
//!   0.15 % of the spray), and a last job reduces them ([`TubeTable::from_tracers`]).
//!
//! **Bit for bit.** Each unit is a pure function of the galaxy and its index: an orphan of `k`,
//! every one of its draws keyed by `k`; a leg's radii and a tracer whatever they are stepped
//! beside. The gather is in index order, so the list and the tables are the serial builds', at any
//! number of workers and in any shares. A share whose job failed or was cancelled is drawn,
//! stepped or sprayed by the next job, so that no result depends on one. Each worker checks its
//! floating-point mode before every reply ([`CpuPool`]), and the galaxy and the force are shared
//! read-only.
//!
//! **No job waits on another.** As for a range query ([`served_range_query`]): the pool's queues
//! are first-in, first-out and a worker runs one job to its end, so a job that blocked on jobs it
//! had queued could stall every worker. Every job here is queued by the caller's own task, which
//! awaits their replies on the runtime, and a job only plans, draws, steps, sprays or assembles.
//! A build holds at most its shares' jobs and three more in the queue at once.
//!
//! [`served_range_query`]: super::served_range_query
//! [`DebrisPlan::orphan`]: hyperion_sim::galaxy::global_list::DebrisPlan::orphan

use std::num::NonZeroUsize;
use std::sync::Arc;

use hyperion_sim::galaxy::global_list::orbit::BuildLeapfrogError;
use hyperion_sim::galaxy::global_list::spray::{
    Leg, Spray, SprayForce, SprayParams, Tracer, radii_over_ly,
};
use hyperion_sim::galaxy::global_list::tube::BuildTubeTableError;
use hyperion_sim::galaxy::global_list::{Debris, GlobalList, ListPlan, StreamSpec, TubeTable};
use hyperion_sim::galaxy::{Galaxy, Population};

use super::{CancelToken, ComputeError, CpuPool, GalaxyKey, JobError, JobReceiver, Priority};
use crate::cache::{HeapBytes, SharedByteLru};

/// The tracers of one share of a tube's spray: 250, eight shares of a stream's 2,000 on eight
/// workers, about 0.25 s each (perf-spray's recorded design, 2026-10-30).
pub const TRACERS_PER_SHARE: u32 = 250;

/// The spray tables of the galaxies the server holds, one per galaxy, each some 160 kB and
/// 12 ms to build ([`SprayForce::new`]): what the list's shell orbits and every tube's spray step
/// in.
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
    /// An empty cache that holds at most `budget_bytes` of charged tables. Zero keeps none: each
    /// build then makes its own.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shares {
    orphans: NonZeroUsize,
    legs: NonZeroUsize,
    tracers_per_share: u32,
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
    ///
    /// # Panics
    ///
    /// If `tracers_per_share` is zero.
    #[must_use]
    pub const fn new(orphans: NonZeroUsize, legs: NonZeroUsize, tracers_per_share: u32) -> Self {
        assert!(tracers_per_share > 0, "a share holds at least one tracer");
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
            Self::Compute(error) => write!(f, "the tube's jobs could not run: {error}"),
            Self::Table(error) => write!(f, "the tube's table could not be made: {error}"),
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

/// The spray table of `galaxy`, the galaxy `key` names: `forces`' if it holds it, or built on a
/// job queued now and stored, its receiver given back to be awaited.
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
    let (forces, galaxy) = (Arc::clone(forces), Arc::clone(galaxy));
    let receiver = queue(pool, priority, token, move |_: &CancelToken| {
        let force = Arc::new(SprayForce::new(galaxy.potential())?);
        // A table the cache refuses is this build's alone.
        drop(forces.forces.insert(key, Arc::clone(&force)));
        Ok(force)
    })
    .await?;
    Ok(Force::Building(receiver))
}

/// A galaxy's spray table, held or being built.
enum Force {
    Held(Arc<SprayForce>),
    Building(JobReceiver<Result<Arc<SprayForce>, BuildLeapfrogError>>),
}

impl Force {
    /// The table, once built.
    async fn ready(self) -> Result<Result<Arc<SprayForce>, BuildLeapfrogError>, ComputeError> {
        match self {
            Self::Held(force) => Ok(Ok(force)),
            Self::Building(receiver) => reply(receiver).await,
        }
    }
}

/// `count` indices dealt out in turn among `shares` shares, but never more shares than indices:
/// share `j` holds `j`, `j + n`, `j + 2n` and so on, each in order.
#[must_use]
fn dealt(count: u64, shares: NonZeroUsize) -> Vec<Vec<u64>> {
    let n = u64::try_from(shares.get())
        .unwrap_or(u64::MAX)
        .min(count)
        .max(1);
    (0..n)
        .map(|first| {
            (first..count)
                .step_by(usize::try_from(n).expect("at most the shares, a usize"))
                .collect()
        })
        .filter(|share: &Vec<u64>| !share.is_empty())
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
/// `shares` (module documentation): [`GlobalList::build`]'s, bit for bit. The galaxy's centre is
/// built and kept in it, and its spray table kept in `forces`.
///
/// Each job waits for room in its queue, as a galaxy's build is no request of its own.
///
/// # Errors
///
/// [`ComputeError::Submit`] if the pool is shutting down or faulted, and [`ComputeError::Job`] if
/// one of the build's jobs that no later job redoes is cancelled, panics, is lost with the pool or
/// leaves its worker's floating-point mode flushing subnormals: a panic where
/// [`GlobalList::build`] panics, for a galaxy without its full potential among them.
///
/// # Panics
///
/// If the galaxy's spray table cannot be built though its debris could be planned, which a galaxy
/// with the kinematic tables the debris needs never gives.
pub async fn global_list_on(
    pool: &CpuPool,
    forces: &Arc<SharedSprayForces>,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    (priority, shares): (Priority, Shares),
    token: &CancelToken,
) -> Result<GlobalList, ComputeError> {
    let at = |galaxy: &Arc<Galaxy>| Arc::clone(galaxy);
    let centre = {
        let galaxy = at(galaxy);
        // Its model is kept in the galaxy, whatever the job sends back.
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
    let mut drawing = Vec::new();
    for share in dealt(plan.orphan_count(), shares.orphans) {
        let (galaxy, plan) = (at(galaxy), Arc::clone(&plan));
        let job = move |_: &CancelToken| {
            share
                .into_iter()
                .map(|k| (k, plan.orphan(&galaxy, k)))
                .collect::<Vec<_>>()
        };
        drawing.push(queue(pool, priority, token, job).await?);
    }
    let mut orphans: Vec<Option<Option<StreamSpec>>> =
        vec![
            None;
            usize::try_from(plan.orphan_count())
                .expect("a galaxy's orphans number far fewer than a usize holds")
        ];
    for receiver in drawing {
        // A share lost is drawn by the job after it, as the module documentation says.
        if let Ok(drawn) = reply(receiver).await {
            for (k, orphan) in drawn {
                orphans[usize::try_from(k).expect("an index of the vector above")] = Some(orphan);
            }
        }
    }
    let assembled = {
        let galaxy = at(galaxy);
        let legs_shares = shares.legs;
        queue(pool, priority, token, move |_: &CancelToken| {
            let plan = Arc::unwrap_or_clone(plan);
            let drawn = (0_u64..)
                .zip(orphans)
                .map(|(k, orphan)| orphan.unwrap_or_else(|| plan.orphan(&galaxy, k)));
            // A lost share's orphans are drawn here, before the plan is given up to its finish.
            let drawn: Vec<_> = drawn.collect();
            let list = ListPlan::of_debris(plan.finish(drawn));
            let dealt = by_steps(list.legs(), legs_shares);
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
    let mut radii: Vec<Option<[f64; 2]>> = vec![None; list.legs().len()];
    for receiver in stepping {
        // A share lost is stepped by the job after it, as the module documentation says.
        if let Ok(stepped) = reply(receiver).await {
            for (n, r) in stepped {
                radii[n] = Some(r);
            }
        }
    }
    // The finish reads the centre, which its job has built, or which the finish builds itself, and
    // fails as that job did, if that job failed: either way no job waits on another's, and what
    // the job sent back is not needed.
    let _ = reply(centre).await;
    let galaxy = at(galaxy);
    let finished = queue(pool, priority, token, move |_: &CancelToken| {
        let legs = list.legs();
        let radii: Vec<[f64; 2]> = radii
            .into_iter()
            .zip(legs)
            .map(|(r, leg)| r.unwrap_or_else(|| radii_over_ly(&force, &[*leg])[0]))
            .collect();
        Arc::unwrap_or_clone(list).finish(&galaxy, &radii)
    })
    .await?;
    reply(finished).await
}

/// The tube table of `spec`'s stream in `galaxy`, the galaxy `key` names, its tracers sprayed on
/// `pool`'s workers at `priority` in `shares`' shares of tracers (module documentation):
/// [`TubeTable::build`]'s, bit for bit. The galaxy's spray table is kept in `forces`.
///
/// # Errors
///
/// [`BuildTubeOnPoolError::Table`] as [`TubeTable::build`]'s error, and
/// [`BuildTubeOnPoolError::Compute`] as [`global_list_on`]'s.
pub async fn tube_table_on(
    pool: &CpuPool,
    forces: &Arc<SharedSprayForces>,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    spec: &StreamSpec,
    (priority, shares): (Priority, Shares),
    token: &CancelToken,
) -> Result<TubeTable, BuildTubeOnPoolError> {
    let params = SprayParams::default();
    let tracers = tracers_on(
        pool,
        forces,
        (galaxy, key),
        (spec, params),
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
/// sprayed on `pool`'s workers at `priority` in shares of `shares`' tracers a share, the last share
/// gathering any whose job failed: [`Spray::tracers`]'s, bit for bit.
///
/// # Errors
///
/// [`BuildTubeOnPoolError::Table`] with [`BuildTubeTableError::Spray`] as [`Spray::new`]'s error,
/// and [`BuildTubeOnPoolError::Compute`] as [`global_list_on`]'s.
///
/// # Panics
///
/// Never on the targets the server builds for, whose `usize` holds a `u32`.
pub async fn tracers_on(
    pool: &CpuPool,
    forces: &Arc<SharedSprayForces>,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    (spec, params): (&StreamSpec, SprayParams),
    (priority, shares): (Priority, Shares),
    token: &CancelToken,
) -> Result<Vec<Tracer>, BuildTubeOnPoolError> {
    let force = force_of(pool, forces, (galaxy, key), priority, token)
        .await?
        .ready()
        .await?
        .map_err(|error| BuildTubeOnPoolError::Table(BuildTubeTableError::Spray(error)))?;
    let count = params.tracers();
    let spray_in = {
        let (galaxy, force, spec) = (Arc::clone(galaxy), Arc::clone(&force), spec.clone());
        move |indices: core::ops::Range<u32>| {
            Spray::new(galaxy.seed(), &force, galaxy.potential(), &spec, &params)
                .map(|spray| spray.tracers_in(indices))
        }
    };
    let spray_in = Arc::new(spray_in);
    let mut spraying = Vec::new();
    for first in (0..count).step_by(
        usize::try_from(shares.tracers_per_share).expect("a u32 fits a usize on every target"),
    ) {
        let indices = first..count.min(first.saturating_add(shares.tracers_per_share));
        let spray_in = Arc::clone(&spray_in);
        let job = move |_: &CancelToken| spray_in(indices);
        spraying.push((first, queue(pool, priority, token, job).await?));
    }
    let mut parts: Vec<(core::ops::Range<u32>, Option<Vec<Tracer>>)> = Vec::new();
    for (first, receiver) in spraying {
        let end = count.min(first.saturating_add(shares.tracers_per_share));
        match reply(receiver).await {
            Ok(Ok(tracers)) => parts.push((first..end, Some(tracers))),
            Ok(Err(error)) => {
                return Err(BuildTubeOnPoolError::Table(BuildTubeTableError::Spray(
                    error,
                )));
            }
            // A share lost is sprayed by the job after it, as the module documentation says.
            Err(_) => parts.push((first..end, None)),
        }
    }
    if parts.iter().all(|(_, tracers)| tracers.is_some()) {
        return Ok(parts
            .into_iter()
            .flat_map(|(_, t)| t.into_iter().flatten())
            .collect());
    }
    let gathered = queue(pool, priority, token, move |_: &CancelToken| {
        let mut tracers = Vec::new();
        for (indices, sprayed) in parts {
            match sprayed {
                Some(sprayed) => tracers.extend(sprayed),
                None => tracers.extend(spray_in(indices)?),
            }
        }
        Ok(tracers)
    })
    .await?;
    reply(gathered)
        .await?
        .map_err(|error| BuildTubeOnPoolError::Table(BuildTubeTableError::Spray(error)))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hyperion_sim::GENERATOR_VERSION;
    use hyperion_sim::galaxy::global_list::testing::pal5_like;

    use super::*;
    use crate::convert::origins::{SEED, galaxy};
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

    /// The server tests' galaxy as a fresh copy, its centre not yet read.
    fn cold() -> Arc<Galaxy> {
        Arc::new(galaxy().clone())
    }

    /// On one worker, four and eight, in the design's shares, in one share of each kind and in
    /// more shares than units, the list made on the pool is the serial build's, bit for bit,
    /// whether the galaxy's centre and its spray table were held or not (P10.T5.b).
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
    /// (P10.T5.b).
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
            let shares = Shares::new(workers(n), workers(n), per);
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
