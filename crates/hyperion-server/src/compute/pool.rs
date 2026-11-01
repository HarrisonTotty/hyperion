//! The CPU pool: plain worker threads fed by three bounded queues.
//!
//! Generation is CPU work that must never run on the async runtime, and a running
//! `spawn_blocking` task cannot be stopped, so it runs here (plan 04, design note 21). The pool is
//! hand-written rather than rayon, whose queue is unbounded. There are three queues:
//!
//! - [`Priority::Interactive`]: range queries, galaxy builds, parameter reads and the alert scans'
//!   interactive walks. Workers always take interactive work first, and a submission that finds
//!   the queue full fails at once with [`SubmitJobError::QueueFull`], so that a client hears
//!   `queue_full` rather than waiting; an alert scan, which no request waits on, waits for a slot
//!   instead (plan 12, P12.T8.b).
//! - [`Priority::Bulk`]: density map bands, the alert scans' background walks in chunks of cells,
//!   and the coarse surface passes asked for. A submission waits for a slot, so a map cannot delay
//!   a chart by more than the band a worker is running.
//! - [`Priority::Background`]: work started before anyone asks, the surface prefetch's plans and
//!   passes (rendering plan R09, R09.T17), taken only when neither other queue holds a job, so
//!   that an asked pass queued after a prefetched one is taken first. It is as deep as the bulk
//!   queue, and a submission waits for a slot. A job queued with a [`Promotion`] moves to the back
//!   of the bulk queue when the promotion is raised ([`CpuPool::promote`]), as an asked request
//!   that joins a prefetched pass raises it, or goes there at once if it was raised first.
//!
//! Each queue is bounded by a [`Semaphore`]. A job holds a permit while it waits, and the permit
//! is released the moment a worker takes the job. Jobs run under `catch_unwind`, so a panic costs
//! one request and not a worker. A job's result, or why it has none, arrives on a
//! [`JobReceiver`], which is always sent exactly one message.
//!
//! Every worker checks its floating-point mode with a [`FlushProbe`] before its first job and after
//! each job, before the reply, so that no value computed under a mode that flushes subnormals ever
//! leaves it (plan R04, design note 15). A failed probe at start stops the pool from starting; a
//! failed probe after a job faults the pool, which then refuses all work.
//!
//! A test can hold the bulk and background jobs at a known point with a [`BulkGate`]
//! ([`CpuPool::with_bulk_gate`]), so that it asserts the order the workers take jobs in rather
//! than a margin of time.

use std::any::Any;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::io;
use std::num::NonZeroUsize;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError, oneshot};

use super::{CancelToken, FlushProbe, probe_flush_to_zero};

/// Where a job waits for a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// Work a client is waiting on: taken first, refused when the queue is full.
    Interactive,
    /// Background work that may wait: taken when no interactive work is queued.
    Bulk,
    /// Work started before anyone asks for it: taken only when no interactive or bulk work is
    /// queued.
    Background,
}

impl Priority {
    /// The name used in logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Bulk => "bulk",
            Self::Background => "background",
        }
    }
}

/// Why a submitted job produced no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobError {
    /// Its token was cancelled before a worker took it, so it never ran.
    Cancelled,
    /// It panicked. The panic was logged, and the worker went on to the next job.
    Panicked,
    /// The pool shut down before a worker took it.
    ShutDown,
    /// Its worker's floating-point mode was found to flush subnormals, after this job or an
    /// earlier one, so its value, if it ran, was withheld and the pool is faulted.
    FloatingPointMode,
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "the job was cancelled",
            Self::Panicked => "the job panicked",
            Self::ShutDown => "the cpu pool shut down before the job ran",
            Self::FloatingPointMode => "a cpu worker's floating-point mode flushes subnormals",
        })
    }
}

impl Error for JobError {}

impl From<oneshot::error::RecvError> for JobError {
    /// A job's reply is only ever dropped unsent with the pool itself.
    fn from(_: oneshot::error::RecvError) -> Self {
        Self::ShutDown
    }
}

/// A job could not be queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubmitJobError {
    /// The queue was full and the submission could not wait.
    QueueFull,
    /// The pool is shutting down.
    ShutDown,
    /// A worker's floating-point mode was found to flush subnormals, so the pool refuses all work.
    Faulted,
}

impl fmt::Display for SubmitJobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::QueueFull => "the cpu pool's queue is full",
            Self::ShutDown => "the cpu pool is shutting down",
            Self::Faulted => {
                "the cpu pool refuses work: a worker's floating-point mode flushes subnormals"
            }
        })
    }
}

impl Error for SubmitJobError {}

/// The pool could not be started.
#[derive(Debug)]
pub enum StartPoolError {
    /// A queue capacity is above what a semaphore can count.
    CapacityTooLarge {
        /// The capacity asked for.
        capacity: usize,
    },
    /// The operating system refused a worker thread. Workers already started were stopped.
    SpawnWorker {
        /// The operating system's error.
        source: io::Error,
    },
    /// A worker's floating-point mode flushes subnormals, so the pool must not generate. Every
    /// worker was stopped.
    FloatingPointMode {
        /// The failing worker's index: its thread is `hyperion-cpu-{worker}`. The lowest, if
        /// several failed.
        worker: usize,
        /// What its probe saw.
        probe: FlushProbe,
    },
}

impl fmt::Display for StartPoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityTooLarge { capacity } => write!(
                f,
                "a queue capacity of {capacity} is above the limit of {}",
                Semaphore::MAX_PERMITS
            ),
            Self::SpawnWorker { .. } => f.write_str("failed to spawn a cpu worker thread"),
            Self::FloatingPointMode { worker, probe } => write!(
                f,
                "the floating-point mode of the cpu worker hyperion-cpu-{worker} {probe}, so the \
                 pool refuses to generate"
            ),
        }
    }
}

impl Error for StartPoolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CapacityTooLarge { .. } | Self::FloatingPointMode { .. } => None,
            Self::SpawnWorker { source } => Some(source),
        }
    }
}

/// The pool did not shut down cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShutDownPoolError {
    /// Worker threads panicked outside a job, which is a bug in the pool.
    WorkersPanicked {
        /// How many.
        count: usize,
    },
    /// The task joining the workers was cancelled by the runtime shutting down.
    Interrupted,
}

impl fmt::Display for ShutDownPoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkersPanicked { count } => write!(f, "{count} cpu worker threads panicked"),
            Self::Interrupted => f.write_str("joining the cpu workers was interrupted"),
        }
    }
}

impl Error for ShutDownPoolError {}

/// Where a job's result arrives: its value, or the [`JobError`] saying why there is none.
///
/// Awaiting it gives `Result<Result<T, JobError>, RecvError>`; the outer error cannot happen
/// while the pool lives, and converts to [`JobError::ShutDown`].
pub type JobReceiver<T> = oneshot::Receiver<Result<T, JobError>>;

/// A snapshot of the pool's queues and totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PoolCounters {
    queued_interactive: usize,
    queued_bulk: usize,
    queued_background: usize,
    running: usize,
    completed: u64,
    cancelled: u64,
    panicked: u64,
}

impl PoolCounters {
    /// Interactive jobs waiting for a worker.
    #[must_use]
    pub fn queued_interactive(&self) -> usize {
        self.queued_interactive
    }

    /// Bulk jobs waiting for a worker.
    #[must_use]
    pub fn queued_bulk(&self) -> usize {
        self.queued_bulk
    }

    /// Background jobs waiting for a worker.
    #[must_use]
    pub fn queued_background(&self) -> usize {
        self.queued_background
    }

    /// Jobs a worker is running now.
    #[must_use]
    pub fn running(&self) -> usize {
        self.running
    }

    /// Jobs that ran to completion and returned a value, including a value then withheld because
    /// the probe after the job found the worker flushing subnormals. Jobs a faulted pool refused
    /// unrun are counted in no total.
    #[must_use]
    pub fn completed(&self) -> u64 {
        self.completed
    }

    /// Jobs skipped because their token was cancelled while they were queued.
    #[must_use]
    pub fn cancelled(&self) -> u64 {
        self.cancelled
    }

    /// Jobs that panicked.
    #[must_use]
    pub fn panicked(&self) -> u64 {
        self.panicked
    }
}

/// A fixed set of worker threads fed by an interactive, a bulk and a background queue.
///
/// [`CpuPool::shutdown`] is the teardown: it drops queued jobs and joins the workers. Dropping the
/// pool without it only tells the workers to stop; they finish the job in hand and exit on their
/// own, and jobs still queued then report [`JobError::ShutDown`].
///
/// Each worker probes its floating-point mode ([`probe_flush_to_zero`]) before its first job and
/// after every job, before the reply. A worker that fails at start keeps the pool from starting;
/// one that fails after a job answers that job [`JobError::FloatingPointMode`] and faults the pool:
/// every later submission is refused with [`SubmitJobError::Faulted`], and jobs already queued
/// answer [`JobError::FloatingPointMode`] unrun. Nothing runs on a worker between jobs, so the two
/// probes cover every job.
///
/// The server loads no native library today. Code that loads one must probe the loading thread
/// (a library's constructor runs there, and threads created after it inherit its mode) and then
/// every worker, by a job on each, before trusting the pool again, since a library may write the
/// control register of any thread that calls into it.
#[derive(Debug)]
pub struct CpuPool {
    shared: Arc<Shared>,
    interactive: Arc<Semaphore>,
    bulk: Arc<Semaphore>,
    background: Arc<Semaphore>,
    /// Shared with the blocking task that joins them, which holds the lock until every worker has
    /// exited.
    workers: Arc<Mutex<Vec<JoinHandle<()>>>>,
    /// How many workers it started.
    worker_count: NonZeroUsize,
}

impl CpuPool {
    /// Starts `workers` threads named `hyperion-cpu-N`, with room for `interactive_capacity`
    /// queued interactive jobs and `bulk_capacity` each of bulk and background ones.
    ///
    /// # Errors
    ///
    /// [`StartPoolError::CapacityTooLarge`] if a capacity exceeds [`Semaphore::MAX_PERMITS`],
    /// [`StartPoolError::SpawnWorker`] if a thread cannot be spawned, and
    /// [`StartPoolError::FloatingPointMode`] if a worker's floating-point mode flushes subnormals.
    pub fn new(
        workers: NonZeroUsize,
        interactive_capacity: NonZeroUsize,
        bulk_capacity: NonZeroUsize,
    ) -> Result<Self, StartPoolError> {
        Self::with_probe(
            workers,
            interactive_capacity,
            bulk_capacity,
            probe_flush_to_zero,
        )
    }

    /// [`CpuPool::new`] with `probe` in place of [`probe_flush_to_zero`], so that tests can make a
    /// worker's probe fail.
    ///
    /// Waits until every worker has probed its thread, which takes microseconds.
    ///
    /// # Errors
    ///
    /// As [`CpuPool::new`]; [`StartPoolError::FloatingPointMode`] when `probe` reports flushing on
    /// any worker, naming the lowest such worker, after every worker has been stopped and joined.
    ///
    /// # Panics
    ///
    /// If `probe` panics on a worker, which [`probe_flush_to_zero`] cannot. The other workers are
    /// left running until the pool's shared state is dropped with them.
    pub fn with_probe(
        workers: NonZeroUsize,
        interactive_capacity: NonZeroUsize,
        bulk_capacity: NonZeroUsize,
        probe: fn() -> FlushProbe,
    ) -> Result<Self, StartPoolError> {
        Self::start(workers, interactive_capacity, bulk_capacity, probe, None)
    }

    /// [`CpuPool::new`] with every bulk or background job, once a worker has taken it, waiting at
    /// `gate` before it runs: for a test that holds the bulk work while it queues other work
    /// ([`BulkGate`]).
    ///
    /// # Errors
    ///
    /// As [`CpuPool::new`].
    pub fn with_bulk_gate(
        workers: NonZeroUsize,
        interactive_capacity: NonZeroUsize,
        bulk_capacity: NonZeroUsize,
        gate: BulkGate,
    ) -> Result<Self, StartPoolError> {
        Self::start(
            workers,
            interactive_capacity,
            bulk_capacity,
            probe_flush_to_zero,
            Some(gate),
        )
    }

    /// Starts the workers, probing each with `probe`, their bulk jobs held at `gate` if there is
    /// one.
    fn start(
        workers: NonZeroUsize,
        interactive_capacity: NonZeroUsize,
        bulk_capacity: NonZeroUsize,
        probe: fn() -> FlushProbe,
        gate: Option<BulkGate>,
    ) -> Result<Self, StartPoolError> {
        for capacity in [interactive_capacity, bulk_capacity] {
            if capacity.get() > Semaphore::MAX_PERMITS {
                return Err(StartPoolError::CapacityTooLarge {
                    capacity: capacity.get(),
                });
            }
        }
        let shared = Arc::new(Shared::new(probe, gate));
        let mut handles = Vec::with_capacity(workers.get());
        // Bounded by the number of workers, each of which sends exactly one report.
        let (report, reports) = mpsc::sync_channel(workers.get());
        for index in 0..workers.get() {
            let worker_shared = Arc::clone(&shared);
            let worker_report = report.clone();
            let spawned = thread::Builder::new()
                .name(format!("hyperion-cpu-{index}"))
                .spawn(move || start_worker(index, &worker_shared, worker_report));
            match spawned {
                Ok(handle) => handles.push(handle),
                Err(source) => {
                    // The workers already running see the flag and exit on their own.
                    drop(shared.stop());
                    return Err(StartPoolError::SpawnWorker { source });
                }
            }
        }
        drop(report);
        let mut failed: Option<(usize, FlushProbe)> = None;
        for _ in 0..workers.get() {
            let (index, result) = reports
                .recv()
                .expect("each worker sends its report and drops its sender before working, so the channel closes early only if a probe panicked");
            if result.flushes() && failed.is_none_or(|(lowest, _)| index < lowest) {
                failed = Some((index, result));
            }
        }
        if let Some((worker, probe)) = failed {
            drop(shared.stop());
            // The workers are idle or already gone, so these joins return at once. A worker that
            // panicked could only have done so outside any job, and the error returned already
            // says that the pool is unusable.
            for handle in handles {
                let _ = handle.join();
            }
            return Err(StartPoolError::FloatingPointMode { worker, probe });
        }
        Ok(Self {
            shared,
            interactive: Arc::new(Semaphore::new(interactive_capacity.get())),
            bulk: Arc::new(Semaphore::new(bulk_capacity.get())),
            background: Arc::new(Semaphore::new(bulk_capacity.get())),
            workers: Arc::new(Mutex::new(handles)),
            worker_count: workers,
        })
    }

    /// How many worker threads the pool runs.
    #[must_use]
    pub(crate) fn workers(&self) -> NonZeroUsize {
        self.worker_count
    }

    /// Queues `job` if its queue has room, without waiting.
    ///
    /// The job is given `token` when it runs, and is skipped with [`JobError::Cancelled`] if the
    /// token is cancelled before a worker takes it.
    ///
    /// # Errors
    ///
    /// [`SubmitJobError::QueueFull`] if the queue is full, [`SubmitJobError::ShutDown`] once
    /// [`CpuPool::shutdown`] has begun, and [`SubmitJobError::Faulted`] once a worker's probe has
    /// failed.
    pub fn try_submit<F, T>(
        &self,
        priority: Priority,
        token: CancelToken,
        job: F,
    ) -> Result<JobReceiver<T>, SubmitJobError>
    where
        F: FnOnce(&CancelToken) -> T + Send + 'static,
        T: Send + 'static,
    {
        if self.shared.is_faulted() {
            return Err(SubmitJobError::Faulted);
        }
        let permit = Arc::clone(self.semaphore(priority))
            .try_acquire_owned()
            .map_err(|error| match error {
                TryAcquireError::NoPermits => SubmitJobError::QueueFull,
                TryAcquireError::Closed => SubmitJobError::ShutDown,
            })?;
        self.enqueue(priority, permit, token, job)
    }

    /// Queues `job` in the background queue, waiting for room there, to be moved to the back of
    /// the bulk queue when `promotion` is raised ([`promote`](Self::promote)), or queued there at
    /// once if it was raised already.
    ///
    /// Cancellation-safe: if this future is dropped while it waits, nothing is queued.
    ///
    /// # Errors
    ///
    /// As [`submit`](Self::submit).
    pub async fn submit_promotable<F, T>(
        &self,
        promotion: Promotion,
        token: CancelToken,
        job: F,
    ) -> Result<JobReceiver<T>, SubmitJobError>
    where
        F: FnOnce(&CancelToken) -> T + Send + 'static,
        T: Send + 'static,
    {
        if self.shared.is_faulted() {
            return Err(SubmitJobError::Faulted);
        }
        let permit = Arc::clone(self.semaphore(Priority::Background))
            .acquire_owned()
            .await
            .map_err(|_| SubmitJobError::ShutDown)?;
        self.queue(Priority::Background, permit, token, Some(promotion), job)
    }

    /// Raises `promotion`: the background job queued with it moves to the back of the bulk queue,
    /// to be taken as the bulk jobs queued before it are, and one queued with it later goes there
    /// at once. A job a worker has taken already, or one never queued, is left as it is.
    ///
    /// The job keeps the background queue's place it holds until a worker takes it, so the bulk
    /// queue may hold one job more than its depth for each promoted.
    pub fn promote(&self, promotion: &Promotion) {
        promotion.raised.store(true, AtomicOrdering::SeqCst);
        let mut state = self.shared.lock();
        let held = state
            .background
            .iter()
            .position(|queued| queued.is_raised_by(promotion));
        if let Some(mut queued) = held.and_then(|at| state.background.remove(at)) {
            queued.priority = Priority::Bulk;
            state.bulk.push_back(queued);
            state.refresh_depths();
        }
    }

    /// Queues `job`, waiting for room in its queue.
    ///
    /// Cancellation-safe: if this future is dropped while it waits, nothing is queued.
    ///
    /// # Errors
    ///
    /// [`SubmitJobError::ShutDown`] once [`CpuPool::shutdown`] has begun, including while this
    /// waits, and [`SubmitJobError::Faulted`] once a worker's probe has failed, checked before it
    /// waits and again once it has a place.
    pub async fn submit<F, T>(
        &self,
        priority: Priority,
        token: CancelToken,
        job: F,
    ) -> Result<JobReceiver<T>, SubmitJobError>
    where
        F: FnOnce(&CancelToken) -> T + Send + 'static,
        T: Send + 'static,
    {
        if self.shared.is_faulted() {
            return Err(SubmitJobError::Faulted);
        }
        let permit = Arc::clone(self.semaphore(priority))
            .acquire_owned()
            .await
            .map_err(|_| SubmitJobError::ShutDown)?;
        self.enqueue(priority, permit, token, job)
    }

    /// The queues' depths and the totals so far.
    #[must_use]
    pub fn counters(&self) -> PoolCounters {
        self.shared.lock().counters
    }

    /// Stops the pool: refuses new jobs, drops queued ones, whose receivers get
    /// [`JobError::ShutDown`], and waits for the workers to finish the jobs in hand and exit.
    ///
    /// The threads are joined on the blocking pool, never on the runtime. Cancellation-safe: if
    /// this future is dropped while it waits, the joins go on, and a later call returns once they
    /// are done. Once the workers are joined, calling it again does nothing.
    ///
    /// # Errors
    ///
    /// [`ShutDownPoolError`] if a worker thread panicked outside a job, which only the call that
    /// joined it reports, or the join was cancelled.
    pub async fn shutdown(&self) -> Result<(), ShutDownPoolError> {
        self.interactive.close();
        self.bulk.close();
        self.background.close();
        let dropped = self.shared.stop();
        let dropped_jobs = dropped.len();
        // Dropping a queued job sends its receiver `ShutDown`.
        drop(dropped);
        tracing::debug!(dropped_jobs, "shutting down the cpu pool");
        let workers = Arc::clone(&self.workers);
        let panicked = tokio::task::spawn_blocking(move || {
            // Held while the joins run, on this blocking thread, so that a second shutdown waits
            // for them rather than finding the handles gone and returning early. Workers never
            // take this lock.
            let mut workers = lock_workers(&workers);
            workers
                .drain(..)
                .map(JoinHandle::join)
                .filter(Result::is_err)
                .count()
        })
        .await
        .map_err(|_| ShutDownPoolError::Interrupted)?;
        if panicked == 0 {
            Ok(())
        } else {
            Err(ShutDownPoolError::WorkersPanicked { count: panicked })
        }
    }

    fn semaphore(&self, priority: Priority) -> &Arc<Semaphore> {
        match priority {
            Priority::Interactive => &self.interactive,
            Priority::Bulk => &self.bulk,
            Priority::Background => &self.background,
        }
    }

    fn enqueue<F, T>(
        &self,
        priority: Priority,
        permit: OwnedSemaphorePermit,
        token: CancelToken,
        job: F,
    ) -> Result<JobReceiver<T>, SubmitJobError>
    where
        F: FnOnce(&CancelToken) -> T + Send + 'static,
        T: Send + 'static,
    {
        self.queue(priority, permit, token, None, job)
    }

    /// Queues `job` at `priority`, or in the bulk queue for a background job whose `promotion` is
    /// raised, the flag read under the queues' lock so that [`promote`](Self::promote) either finds
    /// the job queued or has raised the flag first.
    fn queue<F, T>(
        &self,
        priority: Priority,
        permit: OwnedSemaphorePermit,
        token: CancelToken,
        promotion: Option<Promotion>,
        job: F,
    ) -> Result<JobReceiver<T>, SubmitJobError>
    where
        F: FnOnce(&CancelToken) -> T + Send + 'static,
        T: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let mut queued = Queued {
            permit,
            priority,
            promotion,
            enqueued: Instant::now(),
            job: Box::new(Job {
                token,
                work: job,
                reply: Reply(Some(sender)),
            }),
        };
        let refused = {
            let mut state = self.shared.lock();
            if state.faulted {
                Some((queued, SubmitJobError::Faulted))
            } else if state.stopping {
                Some((queued, SubmitJobError::ShutDown))
            } else {
                let raised = queued
                    .promotion
                    .as_ref()
                    .is_some_and(|promotion| promotion.raised.load(AtomicOrdering::SeqCst));
                match priority {
                    Priority::Interactive => state.interactive.push_back(queued),
                    Priority::Bulk => state.bulk.push_back(queued),
                    Priority::Background if raised => {
                        queued.priority = Priority::Bulk;
                        state.bulk.push_back(queued);
                    }
                    Priority::Background => state.background.push_back(queued),
                }
                state.refresh_depths();
                None
            }
        };
        if let Some((queued, error)) = refused {
            // Dropped outside the lock, as `shutdown` drops queued jobs.
            drop(queued);
            return Err(error);
        }
        self.shared.available.notify_one();
        Ok(receiver)
    }
}

fn lock_workers(workers: &Mutex<Vec<JoinHandle<()>>>) -> MutexGuard<'_, Vec<JoinHandle<()>>> {
    // Joining cannot panic part-way through the list, so a poisoned lock still guards the handles
    // not yet joined.
    workers.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Drop for CpuPool {
    /// Tells the workers to stop and returns at once. Queued jobs are released when the last
    /// worker exits.
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.stopping = true;
        drop(state);
        self.shared.available.notify_all();
        self.shared.open_gate();
    }
}

/// A test's hold on a pool's bulk jobs ([`CpuPool::with_bulk_gate`]): while it is closed, each
/// bulk job a worker takes waits before it runs until the test admits it, one at a time. A
/// background job counts as a bulk job here.
///
/// It is open when made. A worker holding a job at the closed gate counts it as running, as it
/// would be, so the pool's counters read as they do while the job runs. A pool that stops opens its
/// gate for good, so that no worker is held past a shutdown. Clones are one gate.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
///
/// use hyperion_server::compute::{BulkGate, CancelToken, CpuPool, Priority};
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let one = NonZeroUsize::MIN;
/// let gate = BulkGate::default();
/// let pool = CpuPool::with_bulk_gate(one, one, one, gate.clone())?;
/// gate.close();
/// let bulk = pool.submit(Priority::Bulk, CancelToken::new(), |_| "bulk").await?;
/// while gate.held() == 0 {
///     tokio::task::yield_now().await;
/// }
/// // The worker holds the bulk job at the gate until it is let through.
/// gate.admit();
/// assert_eq!(bulk.await?, Ok("bulk"));
/// pool.shutdown().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct BulkGate {
    inner: Arc<GateInner>,
}

#[derive(Debug, Default)]
struct GateInner {
    state: Mutex<GateState>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct GateState {
    closed: bool,
    /// Jobs waiting at the gate, admitted or not.
    waiting: usize,
    /// Admissions not yet taken by a waiting job.
    admitted: usize,
}

impl BulkGate {
    /// Holds every bulk job taken from now on until it is admitted. The jobs already running go on.
    pub fn close(&self) {
        self.lock().closed = true;
    }

    /// Lets every bulk job through, those waiting and those to come.
    pub fn open(&self) {
        let mut state = self.lock();
        state.closed = false;
        state.admitted = 0;
        drop(state);
        self.inner.changed.notify_all();
    }

    /// Lets one waiting bulk job through, or the next to arrive if none is waiting.
    pub fn admit(&self) {
        let mut state = self.lock();
        state.admitted = state.admitted.saturating_add(1);
        drop(state);
        self.inner.changed.notify_all();
    }

    /// How many bulk jobs wait at the gate and are not yet admitted.
    #[must_use]
    pub fn held(&self) -> usize {
        let state = self.lock();
        state.waiting.saturating_sub(state.admitted)
    }

    /// Waits, on the calling worker, until the gate is open or admits this job.
    fn pass(&self) {
        let mut state = self.lock();
        state.waiting += 1;
        while state.closed && state.admitted == 0 {
            state = self
                .inner
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        if state.closed {
            state.admitted -= 1;
        }
        state.waiting -= 1;
    }

    fn lock(&self) -> MutexGuard<'_, GateState> {
        // Each critical section only sets a flag or counts, so a poisoned lock still guards
        // consistent state.
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// What the pool and its workers share.
#[derive(Debug)]
struct Shared {
    state: Mutex<State>,
    available: Condvar,
    /// Checks a worker's floating-point mode: [`probe_flush_to_zero`] except in tests.
    probe: fn() -> FlushProbe,
    /// Where a taken bulk or background job waits before it runs: a test's, or none.
    gate: Option<BulkGate>,
}

#[derive(Debug, Default)]
struct State {
    interactive: VecDeque<Queued>,
    bulk: VecDeque<Queued>,
    background: VecDeque<Queued>,
    stopping: bool,
    /// A worker's probe failed after a job: no job runs and no submission is taken any more.
    faulted: bool,
    counters: PoolCounters,
}

impl State {
    fn refresh_depths(&mut self) {
        self.counters.queued_interactive = self.interactive.len();
        self.counters.queued_bulk = self.bulk.len();
        self.counters.queued_background = self.background.len();
    }
}

impl Shared {
    fn new(probe: fn() -> FlushProbe, gate: Option<BulkGate>) -> Self {
        Self {
            state: Mutex::default(),
            available: Condvar::new(),
            probe,
            gate,
        }
    }

    /// Opens the gate for good, if there is one, so that a stopping pool's workers are not held.
    fn open_gate(&self) {
        if let Some(gate) = &self.gate {
            gate.open();
        }
    }

    fn is_faulted(&self) -> bool {
        self.lock().faulted
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Every critical section is a few queue and counter updates that cannot panic part-way,
        // so a poisoned lock still guards consistent state.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets the flag, wakes every worker and hands back the queued jobs, to be dropped outside
    /// the lock.
    fn stop(&self) -> Vec<Queued> {
        let mut state = self.lock();
        state.stopping = true;
        let mut dropped: Vec<_> = state.interactive.drain(..).collect();
        dropped.extend(state.bulk.drain(..));
        dropped.extend(state.background.drain(..));
        state.refresh_depths();
        drop(state);
        self.available.notify_all();
        self.open_gate();
        dropped
    }

    /// Waits for the next job, interactive first and background last, or `None` once the pool is
    /// stopping, with whether it may run or must be refused because the pool is faulted.
    fn next(&self) -> Option<(Queued, Admission)> {
        let mut state = self.lock();
        loop {
            if state.stopping {
                return None;
            }
            let next = state
                .interactive
                .pop_front()
                .or_else(|| state.bulk.pop_front())
                .or_else(|| state.background.pop_front());
            if let Some(queued) = next {
                state.refresh_depths();
                state.counters.running += 1;
                let admission = if state.faulted {
                    Admission::Refuse
                } else {
                    Admission::Run
                };
                return Some((queued, admission));
            }
            state = self
                .available
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Records a finished job. Called before its reply is sent, so that a requester who has its
    /// result sees it counted.
    fn finish(&self, outcome: &Outcome) {
        let mut state = self.lock();
        state.counters.running = state.counters.running.saturating_sub(1);
        match outcome {
            Outcome::Completed => state.counters.completed += 1,
            Outcome::Cancelled => state.counters.cancelled += 1,
            Outcome::Panicked(_) => state.counters.panicked += 1,
            Outcome::Refused => {}
        }
    }

    /// Probes the calling worker after a job and, if its mode flushes, faults the pool, logging
    /// the fault the first time only. Says whether the job's result may leave the worker.
    fn probe_after_job(&self) -> Release {
        let probe = (self.probe)();
        if !probe.flushes() {
            return Release::Send;
        }
        let first = {
            let mut state = self.lock();
            !std::mem::replace(&mut state.faulted, true)
        };
        if first {
            let current = thread::current();
            tracing::error!(
                thread = current.name().unwrap_or("unnamed"),
                %probe,
                "a cpu worker's floating-point mode flushes subnormals; the server refuses to \
                 generate"
            );
        }
        Release::Withhold
    }
}

/// Whether a job taken from a queue may run, by the pool's fault flag when it was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Admission {
    /// The pool is sound: run the job.
    Run,
    /// The pool is faulted: answer [`JobError::FloatingPointMode`] without running it.
    Refuse,
}

/// Whether a job's result may be sent, by the probe run after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Release {
    /// The mode keeps subnormals: send the result.
    Send,
    /// The mode flushes, or the pool is faulted: answer [`JobError::FloatingPointMode`] instead.
    Withhold,
}

/// A job waiting in a queue, with the permit that holds its place.
struct Queued {
    permit: OwnedSemaphorePermit,
    priority: Priority,
    /// What moves a background job to the bulk queue ([`CpuPool::promote`]).
    promotion: Option<Promotion>,
    enqueued: Instant,
    job: Box<dyn Runnable>,
}

impl Queued {
    /// Whether this job was queued with `promotion`.
    #[must_use]
    fn is_raised_by(&self, promotion: &Promotion) -> bool {
        self.promotion
            .as_ref()
            .is_some_and(|held| held.same_as(promotion))
    }
}

/// A background job's claim to the bulk queue ([`CpuPool::submit_promotable`],
/// [`CpuPool::promote`]): raised once, and never lowered. Clones are one promotion.
#[derive(Debug, Clone, Default)]
pub struct Promotion {
    raised: Arc<AtomicBool>,
}

impl Promotion {
    /// A promotion not yet raised.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether it has been raised.
    #[must_use]
    pub fn is_raised(&self) -> bool {
        self.raised.load(AtomicOrdering::SeqCst)
    }

    /// Whether `other` is this promotion, or a clone of it.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.raised, &other.raised)
    }
}

impl fmt::Debug for Queued {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queued")
            .field("priority", &self.priority)
            .field("enqueued", &self.enqueued)
            .finish_non_exhaustive()
    }
}

/// How a job ended.
#[derive(Debug)]
enum Outcome {
    Completed,
    Cancelled,
    Panicked(String),
    /// Not run, because the pool is faulted. Counted in no total.
    Refused,
}

/// A job with its type erased, so that one queue holds jobs of every result type.
trait Runnable: Send {
    /// Runs the job, or skips it if `admission` refuses it or the job is cancelled, then calls
    /// `finish` and sends the reply. `finish` records the outcome and probes the worker; if it
    /// answers [`Release::Withhold`], the reply is [`JobError::FloatingPointMode`] whatever the
    /// job returned.
    fn run(self: Box<Self>, admission: Admission, finish: &dyn Fn(&Outcome) -> Release);
}

struct Job<F, T> {
    token: CancelToken,
    work: F,
    reply: Reply<T>,
}

impl<F, T> Runnable for Job<F, T>
where
    F: FnOnce(&CancelToken) -> T + Send,
    T: Send,
{
    fn run(self: Box<Self>, admission: Admission, finish: &dyn Fn(&Outcome) -> Release) {
        let Self { token, work, reply } = *self;
        let (result, outcome) = if admission == Admission::Refuse {
            (Err(JobError::FloatingPointMode), Outcome::Refused)
        } else if token.is_cancelled() {
            (Err(JobError::Cancelled), Outcome::Cancelled)
        } else {
            match panic::catch_unwind(AssertUnwindSafe(|| work(&token))) {
                Ok(value) => (Ok(value), Outcome::Completed),
                Err(payload) => (
                    Err(JobError::Panicked),
                    Outcome::Panicked(panic_message(payload.as_ref())),
                ),
            }
        };
        match finish(&outcome) {
            Release::Send => reply.send(result),
            Release::Withhold => {
                // A value computed under a flushing mode must not leave the worker. Answered
                // before the value is dropped, so that a panic in its drop cannot change the
                // answer.
                reply.send(Err(JobError::FloatingPointMode));
                drop(result);
            }
        }
    }
}

/// The sending half of a job's reply. Dropped unsent, it sends [`JobError::ShutDown`], so that a
/// job dropped from a queue for any reason still answers.
struct Reply<T>(Option<oneshot::Sender<Result<T, JobError>>>);

impl<T> Reply<T> {
    fn send(mut self, result: Result<T, JobError>) {
        if let Some(sender) = self.0.take() {
            // The requester may have gone, and then nobody wants the result.
            let _ = sender.send(result);
        }
    }
}

impl<T> Drop for Reply<T> {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            // As in `send`: a requester that has gone needs no answer.
            let _ = sender.send(Err(JobError::ShutDown));
        }
    }
}

/// A worker's life: probe the thread, report to the constructor, and work if the probe passed.
fn start_worker(index: usize, shared: &Shared, report: mpsc::SyncSender<(usize, FlushProbe)>) {
    let probe = (shared.probe)();
    // The channel has room for every worker's report. The send fails only when the constructor
    // has already returned because a later thread could not be spawned; the pool is then
    // stopping, and nobody needs the report.
    let _ = report.send((index, probe));
    // Dropped before working, so that the constructor's wait ends even if another worker's probe
    // panics instead of reporting.
    drop(report);
    if !probe.flushes() {
        work(shared);
    }
}

/// A worker's loop: take a job, run it, repeat until the pool stops.
fn work(shared: &Shared) {
    while let Some((
        Queued {
            permit,
            priority,
            promotion: _,
            enqueued,
            job,
        },
        admission,
    )) = shared.next()
    {
        // The queue slot is free as soon as a worker holds the job.
        drop(permit);
        if priority != Priority::Interactive
            && let Some(gate) = &shared.gate
        {
            gate.pass();
        }
        let span = tracing::debug_span!(
            "job",
            priority = priority.as_str(),
            queue_wait_ms = millis(enqueued.elapsed()),
            run_ms = tracing::field::Empty,
        );
        let _entered = span.enter();
        let started = Instant::now();
        // The job itself runs under its own `catch_unwind`. This one catches what is left: a panic
        // in the drop of a job skipped unrun, or of a value nobody waits for any more. Both come
        // after the job is counted and answered, so the worker need only go on to the next.
        let escaped = panic::catch_unwind(AssertUnwindSafe(|| {
            job.run(admission, &|outcome| {
                span.record("run_ms", millis(started.elapsed()));
                match outcome {
                    Outcome::Panicked(message) => {
                        tracing::error!(panic = %message, "a job panicked");
                    }
                    Outcome::Completed | Outcome::Cancelled | Outcome::Refused => {
                        tracing::debug!(outcome = ?outcome, "job finished");
                    }
                }
                // Probed before the job is counted, so that a requester who sees it counted also
                // sees the pool faulted. A refused job ran nothing, and the pool is faulted already.
                let release = match outcome {
                    Outcome::Refused => Release::Withhold,
                    Outcome::Completed | Outcome::Cancelled | Outcome::Panicked(_) => {
                        shared.probe_after_job()
                    }
                };
                shared.finish(outcome);
                release
            });
        }));
        if let Err(payload) = escaped {
            tracing::error!(
                panic = %panic_message(payload.as_ref()),
                "a job's closure or value panicked when dropped"
            );
        }
    }
}

/// A duration in milliseconds, for logs.
fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

/// The message a panic carried, when it was a string.
pub(crate) fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "a panic without a message".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::pin::pin;
    use std::sync::mpsc;

    use futures_util::poll;
    use tokio::time::timeout;

    use super::*;
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};

    /// Upper bound on any wait, so that a hung pool fails the test instead of the suite.
    const WAIT: Duration = Duration::from_secs(10);

    fn pool(workers: usize, interactive: usize, bulk: usize) -> CpuPool {
        let n = |value| NonZeroUsize::new(value).unwrap();
        CpuPool::new(n(workers), n(interactive), n(bulk)).unwrap()
    }

    async fn result<T: fmt::Debug>(receiver: JobReceiver<T>) -> Result<T, JobError> {
        timeout(WAIT, receiver)
            .await
            .expect("timed out waiting for a job")
            .unwrap_or_else(|error| Err(JobError::from(error)))
    }

    /// A job occupying a worker until the test releases it: the way to hold a queue still
    /// without sleeping.
    struct Held {
        release: mpsc::Sender<()>,
        done: JobReceiver<()>,
    }

    impl Held {
        async fn release(self) {
            self.release.send(()).unwrap();
            assert_eq!(result(self.done).await, Ok(()));
        }
    }

    /// Occupies one worker, returning once the worker has taken the job.
    async fn hold(pool: &CpuPool) -> Held {
        let (started_tx, started_rx) = oneshot::channel();
        let (release, release_rx) = mpsc::channel();
        let done = pool
            .try_submit(Priority::Interactive, CancelToken::new(), move |_| {
                started_tx.send(()).unwrap();
                // Blocks this worker, not the runtime. A sender dropped by a failed test also
                // releases it.
                let _ = release_rx.recv();
            })
            .unwrap();
        timeout(WAIT, started_rx)
            .await
            .expect("timed out waiting for the worker")
            .unwrap();
        Held { release, done }
    }

    /// A job that records `label` in `log` when it runs.
    fn record(
        log: &Arc<Mutex<Vec<&'static str>>>,
        label: &'static str,
    ) -> impl FnOnce(&CancelToken) + Send + 'static {
        let log = Arc::clone(log);
        move |_: &CancelToken| log.lock().unwrap().push(label)
    }

    #[tokio::test]
    async fn a_jobs_value_comes_back() {
        let pool = pool(2, 4, 4);
        let receiver = pool
            .try_submit(Priority::Interactive, CancelToken::new(), |_| 6 * 7)
            .unwrap();
        assert_eq!(result(receiver).await, Ok(42));
        let receiver = pool
            .submit(Priority::Bulk, CancelToken::new(), |_| "bulk")
            .await
            .unwrap();
        assert_eq!(result(receiver).await, Ok("bulk"));
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn interactive_work_queued_behind_bulk_work_runs_first() {
        let pool = pool(1, 4, 4);
        let log = Arc::new(Mutex::new(Vec::new()));
        let held = hold(&pool).await;
        let mut receivers = Vec::new();
        for label in ["bulk 1", "bulk 2"] {
            let job = record(&log, label);
            receivers.push(
                pool.submit(Priority::Bulk, CancelToken::new(), job)
                    .await
                    .unwrap(),
            );
        }
        for label in ["interactive 1", "interactive 2"] {
            let job = record(&log, label);
            receivers.push(
                pool.try_submit(Priority::Interactive, CancelToken::new(), job)
                    .unwrap(),
            );
        }
        assert_eq!(pool.counters().queued_bulk(), 2);
        assert_eq!(pool.counters().queued_interactive(), 2);
        held.release().await;
        for receiver in receivers {
            assert_eq!(result(receiver).await, Ok(()));
        }
        assert_eq!(
            *log.lock().unwrap(),
            ["interactive 1", "interactive 2", "bulk 1", "bulk 2"]
        );
        pool.shutdown().await.unwrap();
    }

    /// Background work queued before bulk and interactive work runs after both, in its own order
    /// (rendering plan R09, R09.T17: a prefetched pass never runs ahead of an asked one).
    #[tokio::test]
    async fn background_work_queued_first_runs_after_bulk_and_interactive_work() {
        let pool = pool(1, 4, 4);
        let log = Arc::new(Mutex::new(Vec::new()));
        let held = hold(&pool).await;
        let mut receivers = Vec::new();
        for (priority, label) in [
            (Priority::Background, "background 1"),
            (Priority::Background, "background 2"),
            (Priority::Bulk, "bulk"),
        ] {
            let job = record(&log, label);
            receivers.push(
                pool.submit(priority, CancelToken::new(), job)
                    .await
                    .unwrap(),
            );
        }
        let job = record(&log, "interactive");
        receivers.push(
            pool.try_submit(Priority::Interactive, CancelToken::new(), job)
                .unwrap(),
        );
        let counters = pool.counters();
        assert_eq!(
            (
                counters.queued_background(),
                counters.queued_bulk(),
                counters.queued_interactive()
            ),
            (2, 1, 1)
        );
        held.release().await;
        for receiver in receivers {
            assert_eq!(result(receiver).await, Ok(()));
        }
        assert_eq!(
            *log.lock().unwrap(),
            ["interactive", "bulk", "background 1", "background 2"]
        );
        pool.shutdown().await.unwrap();
    }

    /// A promoted background job moves to the back of the bulk queue: taken after the bulk jobs
    /// queued before its promotion and before those queued after it, and before the background
    /// jobs queued beside it (rendering plan R09, R09.T17).
    #[tokio::test]
    async fn a_promoted_background_job_is_taken_as_a_bulk_job_queued_then() {
        let pool = pool(1, 4, 4);
        let log = Arc::new(Mutex::new(Vec::new()));
        let held = hold(&pool).await;
        let promotion = Promotion::new();
        let mut receivers = vec![
            pool.submit_promotable(
                promotion.clone(),
                CancelToken::new(),
                record(&log, "promoted"),
            )
            .await
            .unwrap(),
        ];
        for (priority, label) in [
            (Priority::Background, "background"),
            (Priority::Bulk, "bulk before"),
        ] {
            let job = record(&log, label);
            receivers.push(
                pool.submit(priority, CancelToken::new(), job)
                    .await
                    .unwrap(),
            );
        }
        assert!(!promotion.is_raised());
        pool.promote(&promotion);
        assert!(promotion.is_raised());
        let counters = pool.counters();
        assert_eq!(
            (counters.queued_bulk(), counters.queued_background()),
            (2, 1)
        );
        let job = record(&log, "bulk after");
        receivers.push(
            pool.submit(Priority::Bulk, CancelToken::new(), job)
                .await
                .unwrap(),
        );
        held.release().await;
        for receiver in receivers {
            assert_eq!(result(receiver).await, Ok(()));
        }
        assert_eq!(
            *log.lock().unwrap(),
            ["bulk before", "promoted", "bulk after", "background"]
        );
        // Raised again with its job done, it moves nothing.
        pool.promote(&promotion);
        assert_eq!(pool.counters().queued_bulk(), 0);
        pool.shutdown().await.unwrap();
    }

    /// A promotion raised before its job is queued queues the job in the bulk queue at once.
    #[tokio::test]
    async fn a_promotion_raised_first_queues_its_job_in_the_bulk_queue() {
        let pool = pool(1, 4, 4);
        let held = hold(&pool).await;
        let promotion = Promotion::new();
        pool.promote(&promotion);
        let receiver = pool
            .submit_promotable(promotion, CancelToken::new(), |_| "promoted")
            .await
            .unwrap();
        let counters = pool.counters();
        assert_eq!(
            (counters.queued_bulk(), counters.queued_background()),
            (1, 0)
        );
        held.release().await;
        assert_eq!(result(receiver).await, Ok("promoted"));
        pool.shutdown().await.unwrap();
    }

    /// A background job still queued at shutdown is dropped unrun, and its queue refuses more.
    #[tokio::test]
    async fn a_shutdown_drops_queued_background_work() {
        let pool = Arc::new(pool(1, 4, 4));
        let held = hold(&pool).await;
        let background = pool
            .submit(Priority::Background, CancelToken::new(), |_| ())
            .await
            .unwrap();
        let shutting_down = tokio::spawn({
            let pool = Arc::clone(&pool);
            async move { pool.shutdown().await }
        });
        assert_eq!(result(background).await, Err(JobError::ShutDown));
        assert_eq!(pool.counters().queued_background(), 0);
        assert_eq!(
            pool.submit(Priority::Background, CancelToken::new(), |_| ())
                .await
                .unwrap_err(),
            SubmitJobError::ShutDown
        );
        held.release().await;
        timeout(WAIT, shutting_down)
            .await
            .expect("timed out shutting down")
            .unwrap()
            .unwrap();
    }

    /// Waits until `gate` holds one job.
    async fn held_at(gate: &BulkGate) {
        timeout(WAIT, async {
            while gate.held() != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("timed out waiting for a bulk job at the gate");
    }

    #[tokio::test]
    async fn a_closed_gate_holds_each_bulk_job_taken_until_it_is_admitted_and_a_shutdown_opens_it()
    {
        let n = |value| NonZeroUsize::new(value).unwrap();
        let gate = BulkGate::default();
        let pool = CpuPool::with_bulk_gate(n(1), n(4), n(4), gate.clone()).unwrap();
        gate.close();
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut bulk = Vec::new();
        for label in ["bulk 1", "bulk 2"] {
            let job = record(&log, label);
            bulk.push(
                pool.submit(Priority::Bulk, CancelToken::new(), job)
                    .await
                    .unwrap(),
            );
        }
        held_at(&gate).await;
        // The held job counts as running, as it would.
        assert_eq!(pool.counters().running(), 1);
        assert_eq!(pool.counters().queued_bulk(), 1);
        let interactive = pool
            .try_submit(
                Priority::Interactive,
                CancelToken::new(),
                record(&log, "interactive"),
            )
            .unwrap();
        gate.admit();
        assert_eq!(result(interactive).await, Ok(()));
        // The worker took the interactive job before the second bulk job, which it now holds.
        held_at(&gate).await;
        assert_eq!(*log.lock().unwrap(), ["bulk 1", "interactive"]);
        pool.shutdown().await.unwrap();
        for receiver in bulk {
            assert_eq!(result(receiver).await, Ok(()));
        }
        assert_eq!(*log.lock().unwrap(), ["bulk 1", "interactive", "bulk 2"]);
    }

    #[tokio::test]
    async fn the_interactive_submit_beyond_capacity_is_refused_while_the_worker_is_held() {
        let pool = pool(
            1,
            INTERACTIVE_QUEUE_CAPACITY.get(),
            BULK_QUEUE_CAPACITY.get(),
        );
        let held = hold(&pool).await;
        // The held job's permit was released when the worker took it, so the queue is empty.
        let receivers: Vec<_> = (0..INTERACTIVE_QUEUE_CAPACITY.get())
            .map(|index| {
                pool.try_submit(Priority::Interactive, CancelToken::new(), move |_| index)
                    .unwrap()
            })
            .collect();
        assert_eq!(receivers.len(), 64);
        assert_eq!(
            pool.try_submit(Priority::Interactive, CancelToken::new(), |_| ())
                .unwrap_err(),
            SubmitJobError::QueueFull
        );
        // Bulk work has a queue of its own.
        let bulk = pool
            .try_submit(Priority::Bulk, CancelToken::new(), |_| usize::MAX)
            .unwrap();
        held.release().await;
        for (index, receiver) in receivers.into_iter().enumerate() {
            assert_eq!(result(receiver).await, Ok(index));
        }
        assert_eq!(result(bulk).await, Ok(usize::MAX));
        // Taking the jobs freed their places.
        let again = pool
            .try_submit(Priority::Interactive, CancelToken::new(), |_| 1)
            .unwrap();
        assert_eq!(result(again).await, Ok(1));
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_bulk_submit_waits_for_a_free_place() {
        let pool = pool(1, 1, 2);
        let held = hold(&pool).await;
        let first = pool
            .submit(Priority::Bulk, CancelToken::new(), |_| 1)
            .await
            .unwrap();
        let second = pool
            .submit(Priority::Bulk, CancelToken::new(), |_| 2)
            .await
            .unwrap();
        let mut third = pin!(pool.submit(Priority::Bulk, CancelToken::new(), |_| 3));
        assert!(poll!(&mut third).is_pending(), "the bulk queue is full");
        assert!(poll!(&mut third).is_pending(), "and stays full while held");
        // Releasing the worker lets it take the first bulk job, which frees a place.
        held.release().await;
        let third = timeout(WAIT, third)
            .await
            .expect("timed out waiting for a place")
            .unwrap();
        assert_eq!(result(first).await, Ok(1));
        assert_eq!(result(second).await, Ok(2));
        assert_eq!(result(third).await, Ok(3));
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_bulk_submit_dropped_while_it_waits_queues_nothing() {
        let pool = pool(1, 1, 1);
        let held = hold(&pool).await;
        let queued = pool
            .submit(Priority::Bulk, CancelToken::new(), |_| "queued")
            .await
            .unwrap();
        let ran = Arc::new(Mutex::new(false));
        {
            let mut waiting = pin!(pool.submit(Priority::Bulk, CancelToken::new(), {
                let ran = Arc::clone(&ran);
                move |_| *ran.lock().unwrap() = true
            }));
            assert!(poll!(&mut waiting).is_pending(), "the bulk queue is full");
        }
        assert_eq!(pool.counters().queued_bulk(), 1);
        held.release().await;
        assert_eq!(result(queued).await, Ok("queued"));
        // The abandoned submission took no place: the queue has room at once.
        let next = pool
            .try_submit(Priority::Bulk, CancelToken::new(), |_| "next")
            .unwrap();
        assert_eq!(result(next).await, Ok("next"));
        assert!(!*ran.lock().unwrap());
        assert_eq!(pool.counters().completed(), 3);
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_job_cancelled_while_queued_never_runs() {
        let pool = pool(1, 4, 4);
        let held = hold(&pool).await;
        let ran = Arc::new(Mutex::new(false));
        let token = CancelToken::new();
        let receiver = pool
            .try_submit(Priority::Interactive, token.clone(), {
                let ran = Arc::clone(&ran);
                move |_| *ran.lock().unwrap() = true
            })
            .unwrap();
        token.cancel();
        held.release().await;
        assert_eq!(result(receiver).await, Err(JobError::Cancelled));
        assert!(!*ran.lock().unwrap());
        assert_eq!(pool.counters().cancelled(), 1);
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_running_job_is_handed_its_token() {
        let pool = pool(1, 4, 4);
        let token = CancelToken::new();
        let (started_tx, started_rx) = oneshot::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let receiver = pool
            .try_submit(Priority::Interactive, token.clone(), move |token| {
                started_tx.send(()).unwrap();
                let _ = release_rx.recv();
                token.is_cancelled()
            })
            .unwrap();
        timeout(WAIT, started_rx).await.unwrap().unwrap();
        token.cancel();
        release.send(()).unwrap();
        assert_eq!(
            result(receiver).await,
            Ok(true),
            "the job ran and saw the cancel"
        );
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_panicking_job_yields_panicked_and_the_same_worker_runs_the_next_job() {
        let pool = pool(1, 4, 4);
        let panicked_on = Arc::new(Mutex::new(None));
        let panicking = pool
            .try_submit(Priority::Interactive, CancelToken::new(), {
                let panicked_on = Arc::clone(&panicked_on);
                move |_| -> u32 {
                    *panicked_on.lock().unwrap() = Some(thread::current().id());
                    panic!("a deliberate panic in a job");
                }
            })
            .unwrap();
        let next = pool
            .try_submit(Priority::Interactive, CancelToken::new(), |_| {
                let current = thread::current();
                (current.id(), current.name().map(str::to_owned))
            })
            .unwrap();
        assert_eq!(result(panicking).await, Err(JobError::Panicked));
        let (id, name) = result(next).await.unwrap();
        assert_eq!(Some(id), *panicked_on.lock().unwrap());
        assert_eq!(name.as_deref(), Some("hyperion-cpu-0"));
        let counters = pool.counters();
        assert_eq!((counters.panicked(), counters.completed()), (1, 1));
        pool.shutdown().await.unwrap();
    }

    /// Panics when dropped, to reach the code around a job rather than the job itself.
    #[derive(Debug)]
    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("a deliberate panic in a drop");
        }
    }

    /// The name of the worker that runs the next interactive job.
    async fn next_worker_name(pool: &CpuPool) -> Option<String> {
        let receiver = pool
            .try_submit(Priority::Interactive, CancelToken::new(), |_| {
                thread::current().name().map(str::to_owned)
            })
            .unwrap();
        result(receiver).await.unwrap()
    }

    #[tokio::test]
    async fn a_panic_dropping_a_skipped_job_or_an_unwanted_value_does_not_kill_the_worker() {
        let pool = pool(1, 4, 4);
        // A cancelled job is dropped unrun, outside the job's own `catch_unwind`.
        let held = hold(&pool).await;
        let token = CancelToken::new();
        let bomb = PanicOnDrop;
        let skipped = pool
            .try_submit(Priority::Interactive, token.clone(), move |_| drop(bomb))
            .unwrap();
        token.cancel();
        held.release().await;
        assert_eq!(result(skipped).await, Err(JobError::Cancelled));
        assert_eq!(
            next_worker_name(&pool).await.as_deref(),
            Some("hyperion-cpu-0")
        );

        // A value nobody waits for any more is dropped by the worker after the job.
        let (started_tx, started_rx) = oneshot::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let orphan = pool
            .try_submit(Priority::Interactive, CancelToken::new(), move |_| {
                started_tx.send(()).unwrap();
                let _ = release_rx.recv();
                PanicOnDrop
            })
            .unwrap();
        timeout(WAIT, started_rx).await.unwrap().unwrap();
        drop(orphan);
        release.send(()).unwrap();
        assert_eq!(
            next_worker_name(&pool).await.as_deref(),
            Some("hyperion-cpu-0")
        );
        let counters = pool.counters();
        assert_eq!(
            (
                counters.running(),
                counters.cancelled(),
                counters.completed()
            ),
            (0, 1, 4)
        );
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_shutdown_abandoned_midway_is_finished_by_the_next_one() {
        let pool = pool(1, 4, 4);
        let held = hold(&pool).await;
        {
            let mut first = pin!(pool.shutdown());
            assert!(poll!(&mut first).is_pending(), "the worker is held");
        }
        let mut second = pin!(pool.shutdown());
        assert!(
            poll!(&mut second).is_pending(),
            "the worker is still held, so the pool has not shut down"
        );
        held.release().await;
        timeout(WAIT, second)
            .await
            .expect("timed out shutting down")
            .unwrap();
        assert_eq!(
            Arc::strong_count(&pool.shared),
            1,
            "every worker has exited"
        );
    }

    #[tokio::test]
    async fn shutdown_returns_with_jobs_still_queued() {
        let pool = Arc::new(pool(1, 4, 4));
        let held = hold(&pool).await;
        let ran = Arc::new(Mutex::new(Vec::new()));
        let interactive = pool
            .try_submit(Priority::Interactive, CancelToken::new(), record(&ran, "i"))
            .unwrap();
        let bulk = pool
            .submit(Priority::Bulk, CancelToken::new(), record(&ran, "b"))
            .await
            .unwrap();
        let shutting_down = tokio::spawn({
            let pool = Arc::clone(&pool);
            async move { pool.shutdown().await }
        });
        // The queued jobs are dropped at once, while the worker is still held.
        assert_eq!(result(interactive).await, Err(JobError::ShutDown));
        assert_eq!(result(bulk).await, Err(JobError::ShutDown));
        assert_eq!(pool.counters().queued_interactive(), 0);
        assert_eq!(pool.counters().queued_bulk(), 0);
        assert_eq!(
            pool.try_submit(Priority::Interactive, CancelToken::new(), |_| ())
                .unwrap_err(),
            SubmitJobError::ShutDown
        );
        assert_eq!(
            pool.submit(Priority::Bulk, CancelToken::new(), |_| ())
                .await
                .unwrap_err(),
            SubmitJobError::ShutDown
        );
        // The job in hand finishes, and then the worker is joined.
        held.release().await;
        timeout(WAIT, shutting_down)
            .await
            .expect("timed out shutting down")
            .unwrap()
            .unwrap();
        assert!(ran.lock().unwrap().is_empty());
        assert_eq!(
            pool.shutdown().await,
            Ok(()),
            "a second shutdown does nothing"
        );
    }

    #[tokio::test]
    async fn a_waiting_bulk_submit_is_refused_by_shutdown() {
        let pool = pool(1, 1, 1);
        let held = hold(&pool).await;
        let queued = pool
            .submit(Priority::Bulk, CancelToken::new(), |_| ())
            .await
            .unwrap();
        let mut waiting = pin!(pool.submit(Priority::Bulk, CancelToken::new(), |_| ()));
        assert!(poll!(&mut waiting).is_pending());
        let mut shutting_down = pin!(pool.shutdown());
        // The first poll closes the queues and drops queued work before joining.
        assert!(poll!(&mut shutting_down).is_pending());
        assert_eq!(
            timeout(WAIT, waiting).await.unwrap().unwrap_err(),
            SubmitJobError::ShutDown
        );
        assert_eq!(result(queued).await, Err(JobError::ShutDown));
        held.release().await;
        timeout(WAIT, shutting_down).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn dropping_the_pool_releases_queued_jobs() {
        let pool = pool(1, 4, 4);
        let held = hold(&pool).await;
        let queued = pool
            .try_submit(Priority::Interactive, CancelToken::new(), |_| ())
            .unwrap();
        let Held { release, done } = held;
        drop(pool);
        release.send(()).unwrap();
        assert_eq!(result(done).await, Ok(()));
        // The worker exits after the job in hand, and its queued work goes with it.
        assert_eq!(result(queued).await, Err(JobError::ShutDown));
    }

    fn probing_pool(workers: usize, probe: fn() -> FlushProbe) -> Result<CpuPool, StartPoolError> {
        let n = |value| NonZeroUsize::new(value).unwrap();
        CpuPool::with_probe(n(workers), n(4), n(4), probe)
    }

    /// Flushes on the workers `hyperion-cpu-1` and `hyperion-cpu-2` only.
    fn flushes_on_workers_1_and_2() -> FlushProbe {
        match thread::current().name() {
            Some("hyperion-cpu-1") => FlushProbe::FLUSHES_BOTH,
            Some("hyperion-cpu-2") => FlushProbe::FLUSHES_INPUTS,
            _ => FlushProbe::KEEPS_SUBNORMALS,
        }
    }

    #[test]
    fn a_pool_whose_probe_fails_at_start_refuses_to_start() {
        let error = probing_pool(4, flushes_on_workers_1_and_2).unwrap_err();
        let StartPoolError::FloatingPointMode { worker, probe } = error else {
            panic!("expected a floating-point mode error, got {error:?}");
        };
        assert_eq!((worker, probe), (1, FlushProbe::FLUSHES_BOTH));
        assert_eq!(
            error.to_string(),
            "the floating-point mode of the cpu worker hyperion-cpu-1 flushes subnormal results to \
             zero and reads subnormal operands as zero, so the pool refuses to generate"
        );
        // Worker 0 passed, and is stopped with the rest.
        let only = probing_pool(1, flushes_on_workers_1_and_2);
        assert!(only.is_ok(), "{only:?}");
    }

    #[test]
    fn the_real_probe_lets_the_pool_start() {
        assert!(probing_pool(2, probe_flush_to_zero).is_ok());
    }

    /// Passes for its first two calls (the worker's start and its first job), then flushes.
    fn flushes_from_the_third_call() -> FlushProbe {
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 {
            FlushProbe::KEEPS_SUBNORMALS
        } else {
            FlushProbe::FLUSHES_OUTPUTS
        }
    }

    #[tokio::test]
    async fn a_job_after_which_the_probe_fails_answers_floating_point_mode_and_faults_the_pool() {
        let pool = probing_pool(1, flushes_from_the_third_call).unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        // The first job runs while the next two queue behind it.
        let held = hold(&pool).await;
        let flushed = pool
            .try_submit(
                Priority::Interactive,
                CancelToken::new(),
                record(&log, "flushed"),
            )
            .unwrap();
        let queued = pool
            .submit(Priority::Bulk, CancelToken::new(), record(&log, "queued"))
            .await
            .unwrap();
        // Its probe is the second call, which passes.
        held.release().await;
        // The next job runs, and the probe after it fails: its value is withheld.
        assert_eq!(result(flushed).await, Err(JobError::FloatingPointMode));
        // The job queued behind it is refused unrun.
        assert_eq!(result(queued).await, Err(JobError::FloatingPointMode));
        assert_eq!(*log.lock().unwrap(), ["flushed"]);
        assert_eq!(
            pool.try_submit(Priority::Interactive, CancelToken::new(), |_| ())
                .unwrap_err(),
            SubmitJobError::Faulted
        );
        let counters = pool.counters();
        assert_eq!(
            (
                counters.running(),
                counters.completed(),
                counters.queued_bulk()
            ),
            (0, 2, 0)
        );
        pool.shutdown().await.unwrap();
    }

    /// Passes at the worker's start, then flushes (a twin of [`flushes_from_the_second_call`],
    /// whose counter another test owns).
    fn flushes_from_the_second_call_too() -> FlushProbe {
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 1 {
            FlushProbe::KEEPS_SUBNORMALS
        } else {
            FlushProbe::FLUSHES_BOTH
        }
    }

    #[tokio::test]
    async fn a_bulk_submit_waiting_when_the_pool_faults_is_refused() {
        let n = |value| NonZeroUsize::new(value).unwrap();
        let pool = CpuPool::with_probe(n(1), n(4), n(1), flushes_from_the_second_call_too).unwrap();
        let Held { release, done } = hold(&pool).await;
        let queued = pool
            .submit(Priority::Bulk, CancelToken::new(), |_| "queued")
            .await
            .unwrap();
        let mut waiting = pin!(pool.submit(Priority::Bulk, CancelToken::new(), |_| "waiting"));
        assert!(poll!(&mut waiting).is_pending(), "the bulk queue is full");
        // The probe after the held job faults the pool before the worker takes the queued job,
        // which frees the place the waiting submission then finds refused.
        release.send(()).unwrap();
        assert_eq!(result(done).await, Err(JobError::FloatingPointMode));
        assert_eq!(result(queued).await, Err(JobError::FloatingPointMode));
        assert_eq!(
            timeout(WAIT, waiting)
                .await
                .expect("timed out waiting for the refusal")
                .unwrap_err(),
            SubmitJobError::Faulted
        );
        pool.shutdown().await.unwrap();
    }

    /// Passes at the worker's start, then flushes.
    fn flushes_from_the_second_call() -> FlushProbe {
        static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 1 {
            FlushProbe::KEEPS_SUBNORMALS
        } else {
            FlushProbe::FLUSHES_INPUTS
        }
    }

    #[tokio::test]
    async fn a_faulted_pool_refuses_submissions() {
        let pool = probing_pool(1, flushes_from_the_second_call).unwrap();
        // A panicking job is probed too, and the fault outranks the panic.
        let panicking = pool
            .try_submit(Priority::Interactive, CancelToken::new(), |_| -> u32 {
                panic!("a deliberate panic in a job");
            })
            .unwrap();
        assert_eq!(result(panicking).await, Err(JobError::FloatingPointMode));
        for priority in [Priority::Interactive, Priority::Bulk] {
            assert_eq!(
                pool.try_submit(priority, CancelToken::new(), |_| ())
                    .unwrap_err(),
                SubmitJobError::Faulted,
                "{priority:?}"
            );
            assert_eq!(
                pool.submit(priority, CancelToken::new(), |_| ())
                    .await
                    .unwrap_err(),
                SubmitJobError::Faulted,
                "{priority:?}"
            );
        }
        assert_eq!(pool.counters().panicked(), 1);
        pool.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn counters_follow_jobs_through_the_queues() {
        let pool = pool(1, 4, 4);
        assert_eq!(pool.counters(), PoolCounters::default());
        let held = hold(&pool).await;
        assert_eq!(pool.counters().running(), 1);
        let receivers = [
            pool.try_submit(Priority::Interactive, CancelToken::new(), |_| ())
                .unwrap(),
            pool.submit(Priority::Bulk, CancelToken::new(), |_| ())
                .await
                .unwrap(),
        ];
        let counters = pool.counters();
        assert_eq!(
            (counters.queued_interactive(), counters.queued_bulk()),
            (1, 1)
        );
        held.release().await;
        for receiver in receivers {
            assert_eq!(result(receiver).await, Ok(()));
        }
        let counters = pool.counters();
        assert_eq!(counters.running(), 0);
        assert_eq!(counters.completed(), 3);
        assert_eq!(
            (counters.queued_interactive(), counters.queued_bulk()),
            (0, 0)
        );
        pool.shutdown().await.unwrap();
    }
}
