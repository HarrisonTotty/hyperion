//! A stream's tube table and the cache trait that hands it out (plan 10, P10.T6 and P10.T1).
//!
//! A tube table is measured, not assumed: about 2,000 tracers sprayed from the progenitor and
//! integrated in the potential, reduced to about 256 knots per arm (brainstorm, "Streams and
//! accreted structure"). It costs about 0.2 s and 80 kB, so it is a cache and never stored, and
//! the cache is the caller's ("Runtime and code shape"): the sim asks for a table through
//! [`TubeLookup`] and keeps none itself.

use std::sync::Arc;

use super::{GlobalList, StreamNumber};

/// One stream's tube table (plan 10, P10.T6).
///
/// The knots, frames, widths, dispersions and line density are P10.T6's. P10.T1 declares the
/// type so that [`TubeLookup`] can name it; no table can be built before that task, and nothing
/// reads one before P10.T8.b.
#[derive(Debug, Clone, PartialEq)]
pub struct TubeTable {
    _knots: (),
}

/// What a caller's cache of tube tables implements: the galaxy's global list, and each stream's
/// table on request (plan 10, "Provides").
///
/// The server's `TubeTableCache` (P10.T10.b) builds a table on a miss inside the job that asked
/// and evicts by bytes; eviction is safe, because a table is a pure function of the galaxy and
/// the stream number and is rebuilt to the same bits. An implementation must hand out the list
/// of the galaxy it is asked about, and the table of `stream` in it.
///
/// # Examples
///
/// A lookup for a caller that has not built the list yet, which no stream or core member can
/// resolve through:
///
/// ```
/// use std::sync::Arc;
///
/// use hyperion_sim::galaxy::global_list::{GlobalList, StreamNumber, TubeLookup, TubeTable};
///
/// struct Empty(GlobalList);
///
/// impl TubeLookup for Empty {
///     fn global_list(&self) -> &GlobalList {
///         &self.0
///     }
///
///     fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
///         unreachable!("an empty list has no stream {}", stream.get())
///     }
/// }
///
/// let lookup = Empty(GlobalList::default());
/// assert_eq!(lookup.global_list().kinds().count(), 0);
/// ```
pub trait TubeLookup {
    /// The galaxy's global list.
    fn global_list(&self) -> &GlobalList;

    /// The tube table of `stream`, built if the cache does not hold it. `stream` is below the
    /// list's stream count; callers check that against [`global_list`](Self::global_list) first.
    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable>;
}
