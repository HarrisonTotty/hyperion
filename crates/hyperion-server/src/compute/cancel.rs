//! Cancellation of CPU jobs: a shared flag that a job checks, and a guard that raises it when the
//! last party interested in the result goes away.
//!
//! No thread is ever killed (plan 04, design note 5). A job still queued when its token is
//! cancelled is skipped by the pool; a running job sees the token and may stop early, and one that
//! cannot check (a range query) finishes and its result is dropped.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A cancellation flag shared between whoever wants a result and the job computing it.
///
/// Clones share one flag. Once cancelled, a token stays cancelled.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    /// A token that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancels the token and every clone of it.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Whether the token has been cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
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
    fn a_dropped_guard_cancels_its_token() {
        let token = CancelToken::new();
        let guard = CancelOnDrop::new(token.clone());
        assert!(!guard.token().is_cancelled());
        drop(guard);
        assert!(token.is_cancelled());
    }
}
