//! The coarse pass's spread on the CPU pool (rendering plan R09; R09.T17's follow-up, the cold
//! field's cost): the climate step's months' orographic solves, run on the pool's free workers
//! beside the pass's own.
//!
//! A pass runs as one job on one worker, and hands its independent tasks to a
//! [`Spread`] ([`coarse_pass_spread`](hyperion_sim::planetary::surface::coarse_pass_spread)).
//! [`PoolSpread`] queues helper jobs on the pool that take tasks from a queue of their own, and the
//! pass's worker takes them too. So the pass never waits on a helper still queued: with no worker
//! free, it runs every task itself, in order, as the sim's own spread would. It waits only for the
//! tasks a helper has taken, which run. Each task writes its own result, so the field is the same
//! bits however the tasks fall.
//!
//! A helper's worker probes its floating-point mode after its job, as every job's does (plan R04,
//! Design note 15). A task's result leaves the helper through the pass, not through the job's
//! reply, so the pass reads the reply of every helper that took a task: where one flushed
//! subnormals, [`PoolSpread::flushed`] holds, and the server withholds the field.

use std::any::Any;
use std::collections::VecDeque;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use hyperion_sim::planetary::surface::Spread;

use crate::compute::{CancelToken, CpuPool, JobError, JobReceiver, Priority};

/// The most tasks a pass's spread runs at once: four months' orographic solves.
///
/// Why: each month solved at once holds its own working state. On the reference Earth at level 8
/// (pod, 16 vCPU, release, 2026-11-01), the climate step took 30.9 s with the months in order, and
/// 27.3, 26.9 and 27.0 s four, six and twelve at a time. The pass's peak resident set was 416 MB in
/// order, and 762, 884 and 1,393 MB. Four take most of the time for under a quarter of the twelve's
/// memory, beside R09.T17.a's bound of passes (decision-r09-pass-cost §C).
const WIDTH: usize = 4;

/// One task of a pass.
type Task = Box<dyn FnOnce() + Send>;

/// The pass's tasks, run on the CPU pool: its helpers at `priority`, beside the worker that runs
/// the pass (the [module](self) documentation).
pub(crate) struct PoolSpread {
    pool: Arc<CpuPool>,
    priority: Priority,
    /// The tasks it takes at once.
    width: usize,
    /// Whether a helper's worker flushed subnormals after it took a task.
    flushed: AtomicBool,
}

impl PoolSpread {
    /// The spread of a pass run at `priority` on `pool`.
    ///
    /// It takes [`WIDTH`] tasks at once, or as many as the pool has workers where that is fewer.
    #[must_use]
    pub(crate) fn new(pool: Arc<CpuPool>, priority: Priority) -> Self {
        let width = pool.workers().get().min(WIDTH);
        Self {
            pool,
            priority,
            width,
            flushed: AtomicBool::new(false),
        }
    }

    /// Whether a helper that took one of the pass's tasks ran on a worker whose floating-point mode
    /// flushes subnormals, so that the pass's field must be withheld
    /// ([`JobError::FloatingPointMode`]).
    #[must_use]
    pub(crate) fn flushed(&self) -> bool {
        self.flushed.load(Ordering::SeqCst)
    }
}

impl fmt::Debug for PoolSpread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolSpread")
            .field("priority", &self.priority)
            .field("flushed", &self.flushed())
            .finish_non_exhaustive()
    }
}

impl Spread for PoolSpread {
    fn width(&self) -> usize {
        self.width
    }

    fn run(&self, tasks: Vec<Task>) {
        if tasks.len() <= 1 {
            for task in tasks {
                task();
            }
            return;
        }
        let helpers = tasks.len() - 1;
        let queue = Arc::new(Queue::new(tasks));
        // A helper the pool cannot queue at once is not needed: the pass's worker runs its share.
        let queued: Vec<(Arc<AtomicBool>, JobReceiver<()>)> = (0..helpers)
            .filter_map(|_| {
                let (queue, took) = (Arc::clone(&queue), Arc::new(AtomicBool::new(false)));
                let marks = Arc::clone(&took);
                self.pool
                    .try_submit(self.priority, CancelToken::new(), move |_: &CancelToken| {
                        queue.drain(&marks);
                    })
                    .ok()
                    .map(|receiver| (took, receiver))
            })
            .collect();
        queue.drain(&AtomicBool::new(false));
        queue.wait_until_idle();
        for (took, receiver) in queued {
            if took.load(Ordering::SeqCst) {
                // The helper ran a task and has nothing left to take, so its reply comes. Only its
                // worker's floating-point mode matters: its tasks' panics were caught in `drain`.
                if let Ok(Err(JobError::FloatingPointMode)) = receiver.blocking_recv() {
                    self.flushed.store(true, Ordering::SeqCst);
                }
            }
        }
        if let Some(payload) = lock(&queue.panicked).take() {
            panic::resume_unwind(payload);
        }
    }
}

/// The tasks not yet taken, the count of those taken and running, and the first panic.
struct Queue {
    tasks: Mutex<VecDeque<Task>>,
    running: Mutex<usize>,
    idle: Condvar,
    panicked: Mutex<Option<Box<dyn Any + Send>>>,
}

impl Queue {
    fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks: Mutex::new(tasks.into()),
            running: Mutex::new(0),
            idle: Condvar::new(),
            panicked: Mutex::new(None),
        }
    }

    /// Takes and runs tasks until none is left, marking `took` once it has taken one. A task's
    /// panic is caught and kept, the first, for the pass to resume.
    fn drain(&self, took: &AtomicBool) {
        loop {
            let task = {
                let mut tasks = lock(&self.tasks);
                let Some(task) = tasks.pop_front() else {
                    return;
                };
                // Counted while the queue is locked, so that a worker that finds it empty finds
                // this task counted as running.
                *lock(&self.running) += 1;
                took.store(true, Ordering::SeqCst);
                task
            };
            if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(task)) {
                lock(&self.panicked).get_or_insert(payload);
            }
            let mut running = lock(&self.running);
            *running -= 1;
            if *running == 0 {
                self.idle.notify_all();
            }
        }
    }

    /// Waits, on the calling worker, until no task taken is still running: with the queue empty,
    /// every task has run.
    fn wait_until_idle(&self) {
        let mut running = lock(&self.running);
        while *running > 0 {
            running = self
                .idle
                .wait(running)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// Takes a lock whether or not a holder panicked: each critical section leaves its value whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::atomic::AtomicUsize;
    use std::thread::{self, ThreadId};

    use tokio::time::timeout;

    use super::*;
    use crate::compute::FlushProbe;
    use crate::compute::sky::bulk;
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
    use crate::testing::WAIT;

    fn pool(workers: usize) -> Arc<CpuPool> {
        Arc::new(
            CpuPool::new(
                NonZeroUsize::new(workers).unwrap(),
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .unwrap(),
        )
    }

    /// Runs `tasks` through a spread of `pool`'s at bulk priority, from a job on the pool as a
    /// pass does, and returns whether a helper flushed.
    async fn spread_on(pool: &Arc<CpuPool>, tasks: Vec<Task>) -> Result<bool, ComputeErrorKind> {
        let spread_pool = Arc::clone(pool);
        let token = CancelToken::new();
        timeout(
            WAIT,
            bulk(pool, &token, move |_: &CancelToken| {
                let spread = PoolSpread::new(spread_pool, Priority::Bulk);
                spread.run(tasks);
                spread.flushed()
            }),
        )
        .await
        .expect("timed out spreading the tasks")
        .map_err(|error| ComputeErrorKind(format!("{error:?}")))
    }

    /// A job's failure, by its debug text, for the assertions.
    #[derive(Debug, PartialEq)]
    struct ComputeErrorKind(String);

    /// Two tasks that meet: each marks its arrival and waits until the other has, so that both
    /// return only where two workers ran them at once; each records its thread.
    fn meeting(threads: &Arc<Mutex<Vec<ThreadId>>>) -> Vec<Task> {
        let met = Arc::new((Mutex::new(0_usize), Condvar::new()));
        (0..2)
            .map(|_| {
                let (met, threads) = (Arc::clone(&met), Arc::clone(threads));
                Box::new(move || {
                    lock(&threads).push(thread::current().id());
                    let (count, arrived) = &*met;
                    let mut count = lock(count);
                    *count += 1;
                    arrived.notify_all();
                    let (count, waited) = arrived
                        .wait_timeout_while(count, WAIT, |count| *count < 2)
                        .unwrap_or_else(PoisonError::into_inner);
                    assert!(!waited.timed_out(), "the other task ran at once");
                    drop(count);
                }) as Task
            })
            .collect()
    }

    /// The pass's tasks run on the pass's worker and a helper at once, each once, with the pool's
    /// free workers (rendering plan R09; R09.T17's follow-up).
    #[tokio::test]
    async fn the_pass_s_tasks_run_at_once_on_its_worker_and_a_helper() {
        let pool = pool(3);
        let threads = Arc::new(Mutex::new(Vec::new()));
        let ran = Arc::new(AtomicUsize::new(0));
        let mut tasks = meeting(&threads);
        tasks.extend((0..10).map(|_| {
            let ran = Arc::clone(&ran);
            Box::new(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            }) as Task
        }));
        assert_eq!(spread_on(&pool, tasks).await, Ok(false));
        let threads = lock(&threads).clone();
        assert_eq!(threads.len(), 2);
        assert_ne!(threads[0], threads[1], "two workers");
        assert_eq!(ran.load(Ordering::SeqCst), 10);
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// With no worker free, the pass's worker runs every task itself, in order, and the helpers it
    /// queued find nothing left.
    #[tokio::test]
    async fn with_no_worker_free_the_pass_runs_its_tasks_in_order() {
        let pool = pool(1);
        let order = Arc::new(Mutex::new(Vec::new()));
        let tasks = (0..12)
            .map(|n| {
                let order = Arc::clone(&order);
                Box::new(move || lock(&order).push(n)) as Task
            })
            .collect();
        assert_eq!(spread_on(&pool, tasks).await, Ok(false));
        assert_eq!(*lock(&order), (0..12).collect::<Vec<_>>());
        let token = CancelToken::new();
        timeout(WAIT, bulk(&pool, &token, |_: &CancelToken| ()))
            .await
            .unwrap()
            .unwrap();
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A task's panic is resumed in the pass once every task has run: the pass's job panics.
    #[tokio::test]
    async fn a_task_s_panic_is_the_pass_s_once_every_task_has_run() {
        let pool = pool(2);
        let ran = Arc::new(AtomicUsize::new(0));
        let tasks = (0..6)
            .map(|n| {
                let ran = Arc::clone(&ran);
                Box::new(move || {
                    ran.fetch_add(1, Ordering::SeqCst);
                    assert_ne!(n, 2, "the test makes this task panic");
                }) as Task
            })
            .collect();
        assert_eq!(
            spread_on(&pool, tasks).await,
            Err(ComputeErrorKind("Job(Panicked)".to_owned()))
        );
        assert_eq!(ran.load(Ordering::SeqCst), 6);
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// Passes for the two workers' starts, then flushes: the probe after the first job fails.
    fn flushes_after_the_starts() -> FlushProbe {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        if CALLS.fetch_add(1, Ordering::SeqCst) < 2 {
            FlushProbe::KEEPS_SUBNORMALS
        } else {
            FlushProbe::FLUSHES_OUTPUTS
        }
    }

    /// A helper that took a task on a worker found to flush subnormals after it marks the spread
    /// flushed, so that the server withholds the field the task fed (plan R04, Design note 15).
    #[tokio::test]
    async fn a_helper_on_a_flushing_worker_marks_the_spread_flushed() {
        let pool = Arc::new(
            CpuPool::with_probe(
                NonZeroUsize::new(2).unwrap(),
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
                flushes_after_the_starts,
            )
            .unwrap(),
        );
        let threads = Arc::new(Mutex::new(Vec::new()));
        let (spread_pool, seen) = (Arc::clone(&pool), Arc::new(AtomicBool::new(false)));
        let marked = Arc::clone(&seen);
        let receiver = pool
            .submit(
                Priority::Bulk,
                CancelToken::new(),
                move |_: &CancelToken| {
                    let spread = PoolSpread::new(spread_pool, Priority::Bulk);
                    spread.run(meeting(&threads));
                    marked.store(spread.flushed(), Ordering::SeqCst);
                },
            )
            .await
            .unwrap();
        // The pass's own worker flushes too after its job, so its answer is withheld; the flag is
        // what the server reads before that, inside the job.
        let answer = timeout(WAIT, receiver).await.unwrap().unwrap();
        assert!(seen.load(Ordering::SeqCst), "the helper's worker flushed");
        assert_eq!(answer, Err(JobError::FloatingPointMode));
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }
}
