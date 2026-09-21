//! What the server is doing now and has done so far, for operators and for tests.
//!
//! [`Server::stats`](crate::Server::stats) takes a [`ServerStats`] snapshot. Integration tests read
//! these counters instead of guessing at timing: a test waits until a counter says a request has
//! ended rather than sleeping until it probably has. The caches add their own counters to the
//! snapshot as they join the server's state (plan 04, P04.T14).

use tokio::sync::watch;

use crate::compute::PoolCounters;

/// A snapshot of the server's activity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ServerStats {
    connections: usize,
    requests: RequestCounters,
    pool: PoolCounters,
}

impl ServerStats {
    pub(crate) fn new(connections: usize, requests: RequestCounters, pool: PoolCounters) -> Self {
        Self {
            connections,
            requests,
            pool,
        }
    }

    /// WebSocket connections open now, counting those upgraded and not yet closed.
    #[must_use]
    pub fn connections(&self) -> usize {
        self.connections
    }

    /// Requests in flight now and the totals so far, over every connection.
    #[must_use]
    pub fn requests(&self) -> RequestCounters {
        self.requests
    }

    /// The CPU pool's queue depths and the jobs it has run.
    #[must_use]
    pub fn pool(&self) -> PoolCounters {
        self.pool
    }
}

/// Requests in flight now, and how the requests so far have ended.
///
/// Every accepted request is in flight until it ends in exactly one way, so
/// `accepted = in_flight + responded + failed + cancelled + abandoned` whenever a snapshot is
/// taken.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RequestCounters {
    in_flight: usize,
    accepted: u64,
    refused: u64,
    responded: u64,
    failed: u64,
    cancelled: u64,
    abandoned: u64,
}

impl RequestCounters {
    /// Requests accepted and not yet ended.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight
    }

    /// Requests accepted and handed to their handler.
    #[must_use]
    pub fn accepted(&self) -> u64 {
        self.accepted
    }

    /// Requests answered at once without being run: before `hello`, over the in-flight limit, with
    /// an ID already in flight, or with a body that does not parse.
    #[must_use]
    pub fn refused(&self) -> u64 {
        self.refused
    }

    /// Accepted requests that ended with a `response`.
    #[must_use]
    pub fn responded(&self) -> u64 {
        self.responded
    }

    /// Accepted requests that ended with a `request_error` other than `cancelled`, a failure of
    /// the server's own (`internal`) included.
    #[must_use]
    pub fn failed(&self) -> u64 {
        self.failed
    }

    /// Accepted requests the client cancelled, which ended with `cancelled`.
    #[must_use]
    pub fn cancelled(&self) -> u64 {
        self.cancelled
    }

    /// Accepted requests still in flight when their connection closed.
    ///
    /// They were cancelled and ended with no message, since there was no one left to send one to.
    #[must_use]
    pub fn abandoned(&self) -> u64 {
        self.abandoned
    }
}

/// How an accepted request ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Ending {
    Responded,
    Failed,
    Cancelled,
    Abandoned,
}

/// The live request counters, shared by every connection.
///
/// A watch channel rather than a lock, so that a test can wait for a condition on them instead of
/// polling.
#[derive(Debug)]
pub(crate) struct RequestStats {
    counters: watch::Sender<RequestCounters>,
}

impl RequestStats {
    pub(crate) fn new() -> Self {
        Self {
            counters: watch::Sender::new(RequestCounters::default()),
        }
    }

    /// The counters now.
    pub(crate) fn snapshot(&self) -> RequestCounters {
        *self.counters.borrow()
    }

    /// A receiver of the counters, on which a test can wait for a request to end.
    #[cfg(test)]
    pub(crate) fn subscribe(&self) -> watch::Receiver<RequestCounters> {
        self.counters.subscribe()
    }

    pub(crate) fn accepted(&self) {
        self.counters.send_modify(|counters| {
            counters.accepted += 1;
            counters.in_flight += 1;
        });
    }

    pub(crate) fn refused(&self) {
        self.counters.send_modify(|counters| counters.refused += 1);
    }

    pub(crate) fn ended(&self, ending: Ending) {
        self.counters.send_modify(|counters| {
            counters.in_flight = counters.in_flight.saturating_sub(1);
            let total = match ending {
                Ending::Responded => &mut counters.responded,
                Ending::Failed => &mut counters.failed,
                Ending::Cancelled => &mut counters.cancelled,
                Ending::Abandoned => &mut counters.abandoned,
            };
            *total += 1;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_accepted_request_is_in_flight_until_it_ends_once() {
        let stats = RequestStats::new();
        for _ in 0..5 {
            stats.accepted();
        }
        stats.refused();
        for ending in [
            Ending::Responded,
            Ending::Failed,
            Ending::Cancelled,
            Ending::Abandoned,
        ] {
            stats.ended(ending);
        }
        let counters = stats.snapshot();
        assert_eq!(
            counters,
            RequestCounters {
                in_flight: 1,
                accepted: 5,
                refused: 1,
                responded: 1,
                failed: 1,
                cancelled: 1,
                abandoned: 1,
            }
        );
        assert_eq!(
            counters.accepted(),
            u64::try_from(counters.in_flight()).unwrap()
                + counters.responded()
                + counters.failed()
                + counters.cancelled()
                + counters.abandoned()
        );
    }
}
