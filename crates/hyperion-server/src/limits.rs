//! Every limit the server enforces, in one place.
//!
//! Each one protects the server, or the other clients of it, from a client that asks for too
//! much. The values are those of plan 04, design note 24, unless a constant says otherwise. A
//! query's time is held to the clock window, ±[`CLOCK_WINDOW_H`](hyperion_sim::time::CLOCK_WINDOW_H),
//! which the sim defines and which is therefore not repeated here.

use std::num::{NonZeroU32, NonZeroUsize};

/// Largest inbound WebSocket message or frame, in bytes: 16 KiB.
///
/// Every request a client sends is small; the large payloads (density maps, range results) only
/// ever flow from the server. A larger frame closes the connection.
pub const MAX_INBOUND_FRAME_BYTES: usize = 16 * 1024;

/// Requests one connection may have in flight at once. The next is refused with
/// `too_many_requests`.
pub const MAX_IN_FLIGHT_REQUESTS: usize = 8;

/// Malformed frames in a row after which the server closes the connection.
pub const MAX_CONSECUTIVE_MALFORMED_FRAMES: u32 = 16;

/// Universes the server holds, counting those on disk and those being created.
pub const MAX_UNIVERSES: usize = 256;

/// Longest universe name, in Unicode scalar values after trimming (plan 04, design note 16).
pub const MAX_UNIVERSE_NAME_CHARS: usize = 48;

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

/// Interactive jobs (range queries, galaxy builds, parameter reads) that may wait in the CPU
/// pool's queue. One more is refused at once with `queue_full`.
pub const INTERACTIVE_QUEUE_CAPACITY: NonZeroUsize = NonZeroUsize::new(64).expect("64 is not zero");

/// Bulk jobs (density map bands) that may wait in the CPU pool's queue. One more waits for a
/// slot.
pub const BULK_QUEUE_CAPACITY: NonZeroUsize = NonZeroUsize::new(256).expect("256 is not zero");
