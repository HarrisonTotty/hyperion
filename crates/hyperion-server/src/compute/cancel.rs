//! Cancellation of CPU jobs: a shared flag that a job checks, and a guard that raises it when the
//! last party interested in the result goes away.
//!
//! No thread is ever killed (plan 04, design note 5). A job still queued when its token is
//! cancelled is skipped by the pool; a running job sees the token and may stop early, and one that
//! cannot check (a range query) finishes and its result is dropped.
//!
//! A token also counts the time the pool's workers have spent running the jobs queued under it, so
//! that a request that queues many, a sky, can log what it cost (rendering plan R06, R06.T11.m).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// A cancellation flag shared between whoever wants a result and the job computing it, with the
/// time spent running the jobs queued under it.
///
/// Clones share one flag and one count. Once cancelled, a token stays cancelled.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    shared: Arc<Shared>,
}

#[derive(Debug, Default)]
struct Shared {
    cancelled: AtomicBool,
    /// The jobs' running time, ns.
    worked_ns: AtomicU64,
}

impl CancelToken {
    /// A token that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancels the token and every clone of it.
    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Release);
    }

    /// Whether the token has been cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.shared.cancelled.load(Ordering::Acquire)
    }

    /// The time the pool's workers have spent running the jobs queued under this token or a clone
    /// of it, each job's from its start to its end: its CPU time, and any time its worker was
    /// preempted meanwhile. A job is counted before its result is sent, so whoever has the result
    /// sees its time counted.
    #[must_use]
    pub(crate) fn worked(&self) -> Duration {
        Duration::from_nanos(self.shared.worked_ns.load(Ordering::Relaxed))
    }

    /// Counts `ran` more of the jobs' running time.
    pub(crate) fn add_worked(&self, ran: Duration) {
        let ns = u64::try_from(ran.as_nanos()).unwrap_or(u64::MAX);
        // A u64 of nanoseconds holds some 584 years of work, which no request reaches.
        self.shared.worked_ns.fetch_add(ns, Ordering::Relaxed);
    }
}

/// Cancels its token when dropped.
///
/// A computation shared by several waiters owns one, so that the computation is cancelled exactly
/// when the last waiter drops it; see [`SingleFlight`](super::SingleFlight).
#[derive(Debug)]
pub struct CancelOnDrop {
    token: CancelToken,
}

impl CancelOnDrop {
    /// A guard over `token`.
    #[must_use]
    pub fn new(token: CancelToken) -> Self {
        Self { token }
    }

    /// The token this guard cancels.
    #[must_use]
    pub fn token(&self) -> &CancelToken {
        &self.token
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_one_clone_cancels_them_all() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!token.is_cancelled());
        clone.cancel();
        assert!(token.is_cancelled());
        assert!(clone.is_cancelled());
    }

    #[test]
    fn the_work_counted_by_one_clone_is_every_clones() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert_eq!(token.worked(), Duration::ZERO);
        clone.add_worked(Duration::from_millis(30));
        token.add_worked(Duration::from_millis(20));
        assert_eq!(token.worked(), Duration::from_millis(50));
        assert_eq!(clone.worked(), Duration::from_millis(50));
        assert_eq!(
            CancelToken::new().worked(),
            Duration::ZERO,
            "a new token is its own"
        );
    }

    #[test]
    fn a_dropped_guard_cancels_its_token() {
        let token = CancelToken::new();
        let guard = CancelOnDrop::new(token.clone());
        assert!(!guard.token().is_cancelled());
        drop(guard);
        assert!(token.is_cancelled());
    }
}
