//! One computation per key, however many ask for it at once.
//!
//! A galaxy or a density map is expensive and several clients may want the same one at the same
//! moment. [`SingleFlight::run`] starts the computation for the first caller of a key and has every
//! later caller of that key wait on the same one, until it completes. The waiters share the result,
//! an error included; the next call after completion starts afresh, so an error is retried and a
//! value is recomputed (the caller's cache is what keeps it).
//!
//! A computation lives only as long as someone waits on it. The registry holds a weak handle, so
//! when every waiter has been dropped the computation is dropped too, and with it whatever it owns,
//! such as a [`CancelOnDrop`](super::CancelOnDrop) that cancels its pool jobs (plan 04, design
//! note 5: a map in progress stops only when every waiter has gone).
//!
//! A computation that panics is caught, and every waiter then panics with its message, so that no
//! waiter is left waiting for a result that will never come; the key is free again at once. The
//! registry's lock is never held while the caller's code runs or while a computation is dropped.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::hash::Hash;
use std::panic::{self, AssertUnwindSafe};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll};

use futures_util::FutureExt;
use futures_util::future::{BoxFuture, Shared, WeakShared};

use super::pool::panic_message;

/// What a computation ends with, as its waiters share it: its result, or the fact of its panic.
type Outcome<V, E> = Result<Result<Arc<V>, E>, Panicked>;

/// One computation, boxed so that a key's flights of any future type share a map.
type Computation<V, E> = BoxFuture<'static, Outcome<V, E>>;

/// A handle a waiter polls; clones share one computation.
type SharedFlight<V, E> = Shared<Computation<V, E>>;

/// The registry's handle, which does not keep the computation alive.
type WeakFlight<V, E> = WeakShared<Computation<V, E>>;

/// The flights in progress by key, each with the ID that tells a key's successive flights apart,
/// so that a finished one never removes its successor.
type Flights<K, V, E> = HashMap<K, (u64, WeakFlight<V, E>)>;

/// A computation panicked, with this message. Each waiter panics with it in turn.
#[derive(Debug, Clone)]
struct Panicked(Arc<str>);

/// Deduplicates concurrent computations of the same key.
pub struct SingleFlight<K, V, E> {
    next_id: AtomicU64,
    flights: Arc<Mutex<Flights<K, V, E>>>,
}

impl<K, V, E> SingleFlight<K, V, E> {
    /// No computations in progress.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            flights: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// How many keys have a computation that someone may still be waiting on.
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.flights).len()
    }

    /// Whether no computation is in progress.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K, V, E> SingleFlight<K, V, E>
where
    K: Eq + Hash + Clone + Send + 'static,
    V: Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
{
    /// Joins the computation of `key` in progress, or starts one with `make_future`.
    ///
    /// The caller joins at once, when `run` is called, not when the returned [`Flight`] is first
    /// polled, so a later caller of `key` shares this computation even before it has begun. The
    /// computation is `make_future`'s future, created and run by whichever waiter polls the
    /// flight first; `make_future` is never called with this registry's lock held, and never at
    /// all by a caller that joins a computation in progress.
    pub fn run<F, Fut>(&self, key: K, make_future: F) -> Flight<V, E>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Arc<V>, E>> + Send + 'static,
    {
        // The candidate flight is built before the lock is taken and dropped after it is released,
        // because dropping a flight takes the lock (see `Departure`).
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let departure = Departure {
            flights: Arc::clone(&self.flights),
            key: key.clone(),
            id,
        };
        let candidate: SharedFlight<V, E> = async move {
            // Dropped when the computation completes, before any waiter sees the result, or
            // when the last waiter drops it unfinished: either way the key is free again.
            let _departure = departure;
            AssertUnwindSafe(async move { make_future().await })
                .catch_unwind()
                .await
                .map_err(|payload| Panicked(panic_message(payload.as_ref()).into()))
        }
        .boxed()
        .shared();
        // A shared future that has never been polled always downgrades; were it not to, the
        // flight would simply not be joined by later callers.
        let Some(weak) = candidate.downgrade() else {
            return Flight { shared: candidate };
        };
        // Declared last, so that should the key's `Hash` or `Eq` panic, the guard is released
        // before the candidate is dropped.
        let mut flights = lock(&self.flights);
        let existing = flights.get(&key).and_then(|(_, weak)| weak.upgrade());
        if let Some(existing) = existing {
            drop(flights);
            // The unused candidate's departure finds the key held by another flight and leaves
            // it be.
            drop(candidate);
            return Flight { shared: existing };
        }
        flights.insert(key, (id, weak));
        drop(flights);
        Flight { shared: candidate }
    }
}

impl<K, V, E> Default for SingleFlight<K, V, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, E> fmt::Debug for SingleFlight<K, V, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SingleFlight")
            .field("in_progress", &self.len())
            .finish()
    }
}

/// One caller's wait on a shared computation: a future of its result.
///
/// Dropping it leaves the computation; the computation is dropped when its last waiter leaves.
///
/// # Panics
///
/// Polling it panics if the computation panicked, with the computation's message, so that every
/// waiter learns of the panic rather than waiting forever.
#[must_use = "a flight does nothing unless awaited"]
pub struct Flight<V, E> {
    shared: SharedFlight<V, E>,
}

impl<V, E: Clone> Future for Flight<V, E> {
    type Output = Result<Arc<V>, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.shared.poll_unpin(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            // `resume_unwind` skips the panic hook, which reported the panic once already.
            Poll::Ready(Err(Panicked(message))) => panic::resume_unwind(Box::new(format!(
                "a shared computation panicked: {message}"
            ))),
        }
    }
}

impl<V, E> fmt::Debug for Flight<V, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flight").finish_non_exhaustive()
    }
}

/// Removes a flight's entry when the flight ends, unless a later flight of the key has replaced
/// it.
struct Departure<K: Eq + Hash, V, E> {
    flights: Arc<Mutex<Flights<K, V, E>>>,
    key: K,
    id: u64,
}

impl<K: Eq + Hash, V, E> Drop for Departure<K, V, E> {
    fn drop(&mut self) {
        // `run` releases the lock before it drops a flight, and no other code holds it while
        // dropping one, so this cannot deadlock.
        let mut flights = lock(&self.flights);
        if flights.get(&self.key).is_some_and(|(id, _)| *id == self.id) {
            flights.remove(&self.key);
        }
    }
}

fn lock<K, V, E>(flights: &Mutex<Flights<K, V, E>>) -> MutexGuard<'_, Flights<K, V, E>> {
    // Every critical section is a lookup, an insert or a remove, which leave the map whole even if
    // the key's `Hash` panicked part-way, so a poisoned lock still guards a usable map.
    flights.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use tokio::sync::{mpsc, oneshot};
    use tokio::time::timeout;

    use super::*;
    use crate::compute::{CancelOnDrop, CancelToken};

    /// Upper bound on any wait, so that a hung flight fails the test instead of the suite.
    const WAIT: Duration = Duration::from_secs(10);

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestError(&'static str);

    type Flights = SingleFlight<u32, u64, TestError>;
    type TestComputation = BoxFuture<'static, Result<Arc<u64>, TestError>>;

    /// A computation that counts its start in `runs` and finishes with what the test sends.
    fn computation(
        runs: &Arc<AtomicUsize>,
    ) -> (
        impl FnOnce() -> TestComputation + Send + 'static,
        oneshot::Sender<Result<u64, TestError>>,
    ) {
        let (sender, receiver) = oneshot::channel::<Result<u64, TestError>>();
        let runs = Arc::clone(runs);
        let make = move || -> TestComputation {
            runs.fetch_add(1, Ordering::SeqCst);
            async move {
                receiver
                    .await
                    .expect("the test keeps the sender")
                    .map(Arc::new)
            }
            .boxed()
        };
        (make, sender)
    }

    async fn outcome(flight: Flight<u64, TestError>) -> Result<u64, TestError> {
        timeout(WAIT, flight)
            .await
            .expect("timed out waiting for a flight")
            .map(|value| *value)
    }

    #[tokio::test]
    async fn two_callers_of_one_key_run_the_computation_once() {
        let flights = Flights::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let (first_make, first_send) = computation(&runs);
        let (second_make, _second_send) = computation(&runs);
        let first = flights.run(1, first_make);
        let second = flights.run(1, second_make);
        assert_eq!(flights.len(), 1, "the second caller joined the first");
        assert_eq!(
            runs.load(Ordering::SeqCst),
            0,
            "nothing runs until a waiter polls"
        );
        first_send.send(Ok(42)).unwrap();
        let (first, second) = tokio::join!(outcome(first), outcome(second));
        assert_eq!((first, second), (Ok(42), Ok(42)));
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert!(flights.is_empty(), "a finished flight leaves the registry");
    }

    #[tokio::test]
    async fn different_keys_run_separately() {
        let flights = Flights::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let (one_make, one_send) = computation(&runs);
        let (two_make, two_send) = computation(&runs);
        let one = flights.run(1, one_make);
        let two = flights.run(2, two_make);
        assert_eq!(flights.len(), 2);
        two_send.send(Ok(2)).unwrap();
        one_send.send(Ok(1)).unwrap();
        assert_eq!(outcome(two).await, Ok(2));
        assert_eq!(outcome(one).await, Ok(1));
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn an_error_reaches_both_callers_and_the_next_call_retries() {
        let flights = Flights::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let (first_make, first_send) = computation(&runs);
        let (second_make, _second_send) = computation(&runs);
        let first = flights.run(1, first_make);
        let second = flights.run(1, second_make);
        first_send.send(Err(TestError("boom"))).unwrap();
        assert_eq!(outcome(first).await, Err(TestError("boom")));
        assert_eq!(outcome(second).await, Err(TestError("boom")));

        let (retry_make, retry_send) = computation(&runs);
        let retry = flights.run(1, retry_make);
        retry_send.send(Ok(7)).unwrap();
        assert_eq!(outcome(retry).await, Ok(7));
        assert_eq!(runs.load(Ordering::SeqCst), 2, "the retry ran afresh");
    }

    #[tokio::test]
    async fn a_call_after_completion_computes_again() {
        let flights = Flights::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let (first_make, first_send) = computation(&runs);
        let first = flights.run(1, first_make);
        first_send.send(Ok(1)).unwrap();
        assert_eq!(outcome(first).await, Ok(1));
        let (again_make, again_send) = computation(&runs);
        let again = flights.run(1, again_make);
        again_send.send(Ok(2)).unwrap();
        assert_eq!(outcome(again).await, Ok(2));
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn dropping_all_callers_cancels_the_token() {
        let flights = Flights::new();
        let token = CancelToken::new();
        let make = {
            let guard = CancelOnDrop::new(token.clone());
            move || -> TestComputation {
                async move {
                    let _guard = guard;
                    std::future::pending().await
                }
                .boxed()
            }
        };
        let runs = Arc::new(AtomicUsize::new(0));
        let (unused_make, _unused_send) = computation(&runs);
        let mut first = flights.run(1, make);
        let second = flights.run(1, unused_make);
        assert_eq!(runs.load(Ordering::SeqCst), 0, "the second caller joined");
        // Poll once, so that the computation is running rather than merely created.
        assert!(futures_util::poll!(&mut first).is_pending());
        drop(first);
        assert!(
            !token.is_cancelled(),
            "one waiter remains, so the computation runs on"
        );
        drop(second);
        assert!(token.is_cancelled());
        assert!(
            flights.is_empty(),
            "the abandoned flight leaves the registry"
        );

        // The key is free: the next caller starts a new computation.
        let (next_make, next_send) = computation(&runs);
        let next = flights.run(1, next_make);
        next_send.send(Ok(3)).unwrap();
        assert_eq!(outcome(next).await, Ok(3));
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn callers_that_leave_before_any_poll_start_nothing_and_free_the_key() {
        let flights = Flights::new();
        let token = CancelToken::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let make = {
            let guard = CancelOnDrop::new(token.clone());
            let runs = Arc::clone(&runs);
            move || -> TestComputation {
                runs.fetch_add(1, Ordering::SeqCst);
                let _guard = guard;
                std::future::pending().boxed()
            }
        };
        let first = flights.run(1, make);
        let (unused_make, _unused_send) = computation(&runs);
        let second = flights.run(1, unused_make);
        drop((first, second));
        assert_eq!(runs.load(Ordering::SeqCst), 0);
        assert!(
            token.is_cancelled(),
            "what the unstarted computation owned is dropped"
        );
        assert!(flights.is_empty());
    }

    /// A waker that records whether it was woken.
    struct Flag(std::sync::atomic::AtomicBool);

    impl std::task::Wake for Flag {
        fn wake(self: Arc<Self>) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_panicking_computation_reaches_every_waiter_and_frees_the_key() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        use std::sync::mpsc;
        use std::task::Waker;

        let flights = Flights::new();
        let (polling_tx, polling_rx) = mpsc::channel::<()>();
        let (registered_tx, registered_rx) = mpsc::channel::<()>();
        let mut leader = flights.run(1, move || -> TestComputation {
            async move {
                // Runs inside the leader's poll, while the flight is marked as being polled.
                polling_tx.send(()).unwrap();
                registered_rx.recv().unwrap();
                panic!("a deliberate panic in a computation");
            }
            .boxed()
        });
        let runs = Arc::new(AtomicUsize::new(0));
        let (unused_make, _unused_send) = computation(&runs);
        let mut follower = flights.run(1, unused_make);

        let leader_thread = std::thread::spawn(move || {
            let caught = catch_unwind(AssertUnwindSafe(|| {
                Pin::new(&mut leader).poll(&mut Context::from_waker(Waker::noop()))
            }));
            caught.map_err(|payload| panic_text(payload.as_ref()))
        });
        polling_rx
            .recv_timeout(WAIT)
            .expect("the leader polls the computation");
        // The follower arrives while the leader is inside the computation: it can only register
        // to be woken, and nothing else will wake it.
        let woken = Arc::new(Flag(std::sync::atomic::AtomicBool::new(false)));
        let follower_waker = Waker::from(Arc::clone(&woken));
        let mut follower_cx = Context::from_waker(&follower_waker);
        assert!(Pin::new(&mut follower).poll(&mut follower_cx).is_pending());
        registered_tx.send(()).unwrap();

        let leader_panic = leader_thread
            .join()
            .expect("the leader's thread catches the panic")
            .expect_err("the waiter that polls the computation panics with it");
        assert!(
            leader_panic.contains("a deliberate panic in a computation"),
            "{leader_panic:?}"
        );
        assert!(
            woken.0.load(Ordering::SeqCst),
            "the other waiter is woken, not left waiting forever"
        );
        let follower_panic = catch_unwind(AssertUnwindSafe(|| {
            Pin::new(&mut follower).poll(&mut follower_cx)
        }))
        .expect_err("the other waiter panics with the same message");
        assert!(
            panic_text(follower_panic.as_ref()).contains("a deliberate panic in a computation"),
            "{:?}",
            panic_text(follower_panic.as_ref())
        );
        assert_eq!(runs.load(Ordering::SeqCst), 0, "the follower only joined");
        assert!(flights.is_empty(), "the key is free again");
    }

    #[test]
    fn a_panicking_make_future_neither_deadlocks_nor_poisons_the_key() {
        let flights = Arc::new(Flights::new());
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        // On a thread of its own, so that a deadlock fails the test instead of hanging it.
        let _caller = std::thread::spawn({
            let flights = Arc::clone(&flights);
            move || {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let flight = flights.run(1, || -> TestComputation {
                        panic!("a deliberate panic in make_future")
                    });
                    flight.now_or_never()
                }));
                done_tx
                    .send(caught.map_err(|payload| panic_text(payload.as_ref())))
                    .unwrap();
            }
        });
        let outcome = done_rx
            .recv_timeout(WAIT)
            .expect("the caller deadlocked on the registry's lock");
        assert!(
            outcome
                .unwrap_err()
                .contains("a deliberate panic in make_future")
        );
        assert!(flights.is_empty());
        let runs = Arc::new(AtomicUsize::new(0));
        let (next_make, next_send) = computation(&runs);
        let next = flights.run(1, next_make);
        next_send.send(Ok(5)).unwrap();
        assert_eq!(next.now_or_never(), Some(Ok(Arc::new(5))));
    }

    #[tokio::test]
    async fn a_waiter_that_polled_first_may_leave_and_the_others_still_get_the_value() {
        let flights = Flights::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let (first_make, first_send) = computation(&runs);
        let (second_make, _second_send) = computation(&runs);
        let mut first = flights.run(1, first_make);
        let second = flights.run(1, second_make);
        // The first waiter drives the computation, then leaves before it finishes.
        assert!(futures_util::poll!(&mut first).is_pending());
        drop(first);
        first_send.send(Ok(11)).unwrap();
        assert_eq!(outcome(second).await, Ok(11));
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert!(flights.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_callers_across_threads_share_one_computation() {
        const CALLERS: usize = 16;
        let flights = Arc::new(Flights::new());
        let runs = Arc::new(AtomicUsize::new(0));
        let (joined_tx, mut joined_rx) = mpsc::channel(CALLERS);
        let (value_tx, value_rx) = oneshot::channel::<Result<u64, TestError>>();
        let value_rx = Arc::new(Mutex::new(Some(value_rx)));
        let callers: Vec<_> = (0..CALLERS)
            .map(|_| {
                let flights = Arc::clone(&flights);
                let runs = Arc::clone(&runs);
                let joined_tx = joined_tx.clone();
                let value_rx = Arc::clone(&value_rx);
                tokio::spawn(async move {
                    let flight = flights.run(1, move || -> TestComputation {
                        runs.fetch_add(1, Ordering::SeqCst);
                        let receiver = value_rx.lock().unwrap().take().expect("one computation");
                        async move { receiver.await.unwrap().map(Arc::new) }.boxed()
                    });
                    joined_tx.send(()).await.unwrap();
                    flight.await.map(|value| *value)
                })
            })
            .collect();
        for _ in 0..CALLERS {
            timeout(WAIT, joined_rx.recv()).await.unwrap().unwrap();
        }
        value_tx.send(Ok(99)).unwrap();
        for caller in callers {
            assert_eq!(timeout(WAIT, caller).await.unwrap().unwrap(), Ok(99));
        }
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert!(flights.is_empty());
    }
}
