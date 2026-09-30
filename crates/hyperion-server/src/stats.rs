//! What the server is doing now and has done so far, for operators and for tests.
//!
//! [`Server::stats`](crate::Server::stats) takes a [`ServerStats`] snapshot. Integration tests read
//! these counters instead of guessing at timing: a test waits until a counter says a request has
//! ended, or a queue has filled, rather than sleeping until it probably has. The caches add their
//! own counters to the snapshot as they join the server's state (plan 04, P04.T14).

use tokio::sync::watch;

use crate::AppState;
use crate::cache::LruCounters;
use crate::compute::{BodyCacheCounters, GalaxyCounters, PoolCounters};

/// A snapshot of the server's activity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ServerStats {
    connections: usize,
    requests: RequestCounters,
    outbound: OutboundCounters,
    pool: PoolCounters,
    galaxies: GalaxyCounters,
    maps: LruCounters,
    cells: LruCounters,
    systems: LruCounters,
    bodies: BodyCacheCounters,
    briefs: LruCounters,
}

impl ServerStats {
    /// A snapshot of `state`: its connections, requests and outbound queues now, and each part's
    /// own counters, the pool's and every cache's.
    pub(crate) fn of(state: &AppState) -> Self {
        Self {
            connections: state.connections.open_count(),
            requests: state.request_stats.snapshot(),
            outbound: state.outbound_stats.snapshot(),
            pool: state.pool.counters(),
            galaxies: state.galaxies.counters(),
            maps: state.maps.counters(),
            cells: state.cells.counters(),
            systems: state.systems.counters(),
            bodies: state.bodies.counters(),
            briefs: state.briefs.counters(),
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

    /// What the connections' outbound queues hold now, and the connections closed because their
    /// clients stopped reading.
    #[must_use]
    pub fn outbound(&self) -> OutboundCounters {
        self.outbound
    }

    /// The CPU pool's queue depths and the jobs it has run.
    #[must_use]
    pub fn pool(&self) -> PoolCounters {
        self.pool
    }

    /// The galaxy cache's contents and use: what a repeated request found built already.
    #[must_use]
    pub fn galaxies(&self) -> GalaxyCounters {
        self.galaxies
    }

    /// The density map cache's contents and use, including its byte budget: a repeated request for
    /// one map is a hit and computes nothing.
    #[must_use]
    pub fn maps(&self) -> LruCounters {
        self.maps
    }

    /// The cell cache's contents and use, including its byte budget: what a range query found
    /// generated already, cell by cell.
    #[must_use]
    pub fn cells(&self) -> LruCounters {
        self.cells
    }

    /// The system cache's contents and use, including its byte budget: a repeated
    /// `system_summary` for one system is a hit and builds no star.
    #[must_use]
    pub fn systems(&self) -> LruCounters {
        self.systems
    }

    /// The body cache's contents and use, including its byte budget, and the planetary systems it
    /// has generated: a repeated `system_bodies` or `body_detail` for one system is a hit and
    /// generates nothing (plan 14, P14.T36).
    #[must_use]
    pub fn bodies(&self) -> BodyCacheCounters {
        self.bodies
    }

    /// The brief cache's contents and use, including its byte budget: a repeated range request
    /// with `include_stellar` finds its rows' models built already (plan 06, P06.T34).
    #[must_use]
    pub fn briefs(&self) -> LruCounters {
        self.briefs
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

/// What the connections' outbound queues hold, over every connection (plan 04, P04.T15).
///
/// The queue of a connection holds the frames waiting for its socket, each counted in payload
/// bytes from the moment it is queued until it has been written. When a client reads too slowly
/// the queue reaches [`OUTBOUND_BYTES`](crate::limits::OUTBOUND_BYTES) and the connection holds
/// back its finished requests; when a frame waits longer than
/// [`WRITE_TIMEOUT`](crate::limits::WRITE_TIMEOUT) to be written, the connection is closed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct OutboundCounters {
    queued_bytes: usize,
    largest_queue_bytes: usize,
    held_requests: usize,
    write_timeouts: u64,
}

impl OutboundCounters {
    /// Payload bytes queued for clients now, counting each frame until it has been written.
    #[must_use]
    pub fn queued_bytes(&self) -> usize {
        self.queued_bytes
    }

    /// The most payload bytes one connection has had queued at once so far.
    #[must_use]
    pub fn largest_queue_bytes(&self) -> usize {
        self.largest_queue_bytes
    }

    /// Finished requests held back now, because their connection's queue has no room for their
    /// terminal frames.
    ///
    /// They are still in flight, and end when their frames are queued.
    #[must_use]
    pub fn held_requests(&self) -> usize {
        self.held_requests
    }

    /// Connections closed because a frame could not be written within
    /// [`WRITE_TIMEOUT`](crate::limits::WRITE_TIMEOUT).
    #[must_use]
    pub fn write_timeouts(&self) -> u64 {
        self.write_timeouts
    }
}

/// The live outbound counters, shared by every connection.
///
/// A watch channel, as [`RequestStats`] is. Clones share the counters, so that each connection's
/// queue can keep a handle to them.
#[derive(Debug, Clone)]
pub(crate) struct OutboundStats {
    counters: watch::Sender<OutboundCounters>,
}

impl OutboundStats {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            counters: watch::Sender::new(OutboundCounters::default()),
        }
    }

    /// The counters now.
    #[must_use]
    pub(crate) fn snapshot(&self) -> OutboundCounters {
        *self.counters.borrow()
    }

    /// A receiver of the counters, on which a test can wait for a queue to fill or drain.
    #[cfg(test)]
    pub(crate) fn subscribe(&self) -> watch::Receiver<OutboundCounters> {
        self.counters.subscribe()
    }

    /// A frame of `bytes` was queued on a connection, whose queue now holds `connection_bytes`.
    pub(crate) fn queued(&self, bytes: usize, connection_bytes: usize) {
        self.counters.send_modify(|counters| {
            counters.queued_bytes = counters.queued_bytes.saturating_add(bytes);
            counters.largest_queue_bytes = counters.largest_queue_bytes.max(connection_bytes);
        });
    }

    /// A frame of `bytes` left its queue: written, or dropped with its connection.
    pub(crate) fn dequeued(&self, bytes: usize) {
        self.counters.send_modify(|counters| {
            counters.queued_bytes = counters.queued_bytes.saturating_sub(bytes);
        });
    }

    /// A finished request is held back.
    pub(crate) fn held(&self) {
        self.counters.send_modify(|counters| {
            counters.held_requests = counters.held_requests.saturating_add(1);
        });
    }

    /// `count` held requests were let go: queued, cancelled, or abandoned with their connection.
    pub(crate) fn released(&self, count: usize) {
        if count == 0 {
            return;
        }
        self.counters.send_modify(|counters| {
            counters.held_requests = counters.held_requests.saturating_sub(count);
        });
    }

    /// A connection is closed because a frame could not be written in time.
    pub(crate) fn write_timed_out(&self) {
        self.counters
            .send_modify(|counters| counters.write_timeouts += 1);
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

    #[test]
    fn outbound_gauges_return_to_zero_and_the_largest_queue_is_kept() {
        let stats = OutboundStats::new();
        stats.queued(300, 300);
        stats.queued(500, 800);
        // Another connection's queue, smaller than the first one's.
        stats.queued(100, 100);
        stats.held();
        stats.held();
        stats.write_timed_out();
        assert_eq!(
            stats.snapshot(),
            OutboundCounters {
                queued_bytes: 900,
                largest_queue_bytes: 800,
                held_requests: 2,
                write_timeouts: 1,
            }
        );
        for bytes in [300, 500, 100] {
            stats.dequeued(bytes);
        }
        stats.released(2);
        stats.released(0);
        assert_eq!(
            stats.snapshot(),
            OutboundCounters {
                queued_bytes: 0,
                largest_queue_bytes: 800,
                held_requests: 0,
                write_timeouts: 1,
            }
        );
    }
}
