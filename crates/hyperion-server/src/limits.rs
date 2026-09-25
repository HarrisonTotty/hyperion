//! Every limit the server enforces, in one place.
//!
//! Each one protects the server, or the other clients of it, from a client that asks for too
//! much. The values are those of plan 04, design note 24, unless a constant says otherwise. A
//! query's time is held to the clock window, ±[`CLOCK_WINDOW_H`](hyperion_sim::time::CLOCK_WINDOW_H),
//! which the sim defines and which is therefore not repeated here.

use std::num::{NonZeroU32, NonZeroUsize};
use std::time::Duration;

/// Largest inbound WebSocket message or frame, in bytes: 16 KiB.
///
/// Every request a client sends is small; the large payloads (density maps, range results) only
/// ever flow from the server. A larger frame closes the connection.
pub const MAX_INBOUND_FRAME_BYTES: usize = 16 * 1024;

/// Requests one connection may have in flight at once. The next is refused with
/// `too_many_requests`.
///
/// A cancelled request frees its place the moment `cancelled` is sent, although work it started
/// on the CPU pool may run on; what that can waste is bounded by the pool's queues.
pub const MAX_IN_FLIGHT_REQUESTS: usize = 8;

/// Malformed frames in a row after which the server closes the connection with a policy
/// violation (close code 1008).
///
/// A frame counts when it is not a client message: text that does not parse, a request whose body
/// does not, or a binary frame. A request of a kind this server does not know is not counted,
/// since a newer client may send one in good faith and is answered `unsupported`. Any well-formed
/// message resets the count.
pub const MAX_CONSECUTIVE_MALFORMED_FRAMES: u32 = 16;

/// Frames one connection may have queued for the socket (plan 04, P04.T13.a).
///
/// When the queue is full the connection stops reading the client's frames until the client has
/// read some of its own: the back-pressure that keeps a slow reader from growing the server's
/// memory. [`OUTBOUND_BYTES`] bounds the same queue in bytes.
pub const OUTBOUND_QUEUE_FRAMES: usize = 32;

/// Payload bytes one connection may have queued for the socket before it stops queueing the
/// answers of finished requests: 16 MiB (plan 04, P04.T15).
///
/// A frame counts from the moment it is queued until it has been written. While a finished
/// request's terminal frame would take the queue past the budget, the connection holds the frame
/// back but goes on reading, so that `ping` and `cancel` are still answered; a held request is
/// still in flight and can be cancelled. A frame larger than the whole budget is sent once the queue
/// is empty. The connection's own small answers (`pong`, `welcome`, refusals, `cancelled`) are not
/// held, and [`OUTBOUND_QUEUE_FRAMES`] bounds them. A slow reader therefore makes the server hold
/// the finished frames of its [`MAX_IN_FLIGHT_REQUESTS`], plus a queue of this much, or of one
/// larger frame alone, plus those small answers, until [`WRITE_TIMEOUT`] closes it.
pub const OUTBOUND_BYTES: usize = 16 * 1024 * 1024;

/// Longest one frame may take to be written to a client: 10 s (plan 04, P04.T15).
///
/// A frame that takes longer means the client has stopped reading, or reads a 4 MB range result
/// at under about 3 Mbit/s. The server then closes the connection with a policy violation (close
/// code 1008), cancels everything it had in flight, and sends nothing more but the close frame.
/// Without this, a client that stops reading would keep its queue and its finished requests until
/// shutdown.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest a closing connection may spend sending what it has queued and completing the close
/// handshake before its socket is dropped.
///
/// This is what bounds [`Server::shutdown`](crate::Server::shutdown) for a client that has stopped
/// reading. It is this server's choice, not a figure from the plan: long enough for a client on a
/// LAN to take a few megabytes and answer the close, short enough that a stuck one cannot hold up
/// a shutdown.
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

/// Universes the server holds, counting those on disk and those being created.
pub const MAX_UNIVERSES: usize = 256;

/// Longest universe name, in Unicode scalar values after trimming (plan 04, design note 16).
pub const MAX_UNIVERSE_NAME_CHARS: usize = 48;

/// The most rows whose stellar briefs a range answer builds in its query's own pool job; above it
/// they are built in chunks of this many rows, one interactive job each, in row order (plan 06,
/// P06.T34).
///
/// At P06.T38.e's measured costs a chunk of main-sequence rows is 10–25 ms of work, and one of dead
/// rows, each a full track until plan 06's fate table, about a second, so a 20,000-row answer is
/// spread over the pool's workers rather than held by one.
pub const BRIEF_CHUNK_ROWS: usize = 1_024;

/// Largest census limit a range query may ask for: the most systems one response returns.
pub const MAX_CENSUS_LIMIT: u32 = 20_000;

/// Largest radius a range query may ask for, in light-years: the width of the face-on map.
pub const MAX_QUERY_RADIUS_LY: f64 = 131_072.0;

/// Most generation cells one range query may visit: 2¹⁸.
///
/// Passed to plan 03's `RangeQueryBuilder::cell_budget`, whose own default of 2²⁰ is sized for a
/// caller with no clients to protect. The census limit bounds the systems returned, not the work:
/// 2,000 ly in the outer halo expects a few thousand layer-A systems while visiting 10⁸ cells.
pub const MAX_QUERY_CELLS: NonZeroU32 = NonZeroU32::new(262_144).expect("262,144 is not zero");

/// Galaxies the server keeps built at once (plan 04, design note 23).
///
/// A `Galaxy` is fixed-size, about 1.4 MiB of heap (plan 02, Risks, R19), so bounding the entries
/// bounds the bytes: four of them are about 5.5 MiB, and are a bridge's worth of universes open at
/// once. The caches whose entries vary in size by orders of magnitude, cells and density maps, are
/// bounded in bytes instead.
pub const GALAXY_CACHE_ENTRIES: NonZeroUsize = NonZeroUsize::new(4).expect("4 is not zero");

/// Interactive jobs (range queries, galaxy builds, parameter reads) that may wait in the CPU
/// pool's queue. One more is refused at once with `queue_full`.
pub const INTERACTIVE_QUEUE_CAPACITY: NonZeroUsize = NonZeroUsize::new(64).expect("64 is not zero");

/// Bulk jobs (density map bands) that may wait in the CPU pool's queue. One more waits for a
/// slot.
pub const BULK_QUEUE_CAPACITY: NonZeroUsize = NonZeroUsize::new(256).expect("256 is not zero");

#[cfg(test)]
mod tests {
    use super::*;

    /// Every limit holds the figure plan 04 gives it: design note 24's, T13.a's frame count, T15's
    /// byte budget and write timeout, design note 16's name length and design note 23's four
    /// galaxies; `CLOSE_TIMEOUT` and the queue capacities are this server's own choice (T13, T8).
    ///
    /// The tests that exercise a limit read its value from here, so that a frame one byte over the
    /// limit is over it whatever the limit is; this is the test that notices the limit itself move.
    #[test]
    fn every_limit_is_the_plans() {
        assert_eq!(MAX_INBOUND_FRAME_BYTES, 16_384, "16 KiB");
        assert_eq!(MAX_IN_FLIGHT_REQUESTS, 8);
        assert_eq!(MAX_CONSECUTIVE_MALFORMED_FRAMES, 16);
        assert_eq!(OUTBOUND_QUEUE_FRAMES, 32);
        assert_eq!(OUTBOUND_BYTES, 16_777_216, "16 MiB");
        assert_eq!(WRITE_TIMEOUT, Duration::from_secs(10));
        assert_eq!(CLOSE_TIMEOUT, Duration::from_secs(1));
        assert_eq!(MAX_UNIVERSES, 256);
        assert_eq!(MAX_UNIVERSE_NAME_CHARS, 48);
        assert_eq!(MAX_CENSUS_LIMIT, 20_000);
        assert_eq!(MAX_QUERY_RADIUS_LY.to_bits(), 131_072.0_f64.to_bits());
        assert_eq!(MAX_QUERY_CELLS.get(), 1 << 18);
        assert_eq!(GALAXY_CACHE_ENTRIES.get(), 4);
        assert_eq!(INTERACTIVE_QUEUE_CAPACITY.get(), 64);
        assert_eq!(BULK_QUEUE_CAPACITY.get(), 256);
    }
}
