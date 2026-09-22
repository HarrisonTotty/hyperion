//! The cache interface: the caller's, never the sim's (plan 03, P03.T6).
//!
//! The brainstorm settles what a cache of cells is ("Runtime and code shape"):
//!
//! - **Generation is lazy, with bounded caches per level.** A cell is generated when something asks
//!   for it, and [`generate_cell`] is a pure function of the galaxy and the cell, so a hit and a
//!   miss are indistinguishable in the answer.
//! - **Eviction is always safe.** Nothing that is generated is remembered anywhere else, so
//!   dropping a cell can only cost time. A cache may evict at any moment, including between two
//!   cells of one query, and neither the systems nor a query's census change.
//! - **A cache holds accepted systems, as state at the epoch.** Never positions at some time: those
//!   belong to the query that asked for a time ([`SystemRecord::epoch_position`]).
//! - **Bounds are in bytes, not entries**, because a bulge cell is ten thousand times heavier than
//!   a rim cell. [`cell_heap_bytes`] is what a cell weighs.
//! - **Whole cells only.** [`generate_cell`] produces every accepted system of its cell, so a
//!   half-generated cell is never stored and eviction needs no bookkeeping.
//!
//! `hyperion-sim` holds no cache of its own: it does no I/O, spawns no threads and keeps no state
//! between calls, so the server owns the caches and calls these pure functions from whatever thread
//! pool it likes (plan 04). Everything here that takes a cache takes it as a [`CellCache`], and
//! [`NoCache`] is the implementation for callers that have none.

#[cfg(doc)]
use super::cell_heap_bytes;
use super::{CellKey, SystemRecord, generate_cell};
use crate::galaxy::Galaxy;

/// A caller's store of generated cells.
///
/// The one operation is "give me this cell's systems", which the cache answers from its own store or
/// by calling [`generate_cell`]. The systems are lent for the length of the call instead of returned
/// so that a cache can hand out a slice it owns without cloning it, and so that it may drop or evict
/// anything as soon as the call ends.
///
/// An implementation must be indistinguishable from [`NoCache`] in what it lends: the same records
/// of the same cell of the same galaxy, in the same order. A cache that lends a stale cell of
/// another galaxy, or reorders one, breaks every guarantee the range query makes.
///
/// # Examples
///
/// A cache that keeps the last cell it was asked for, which is enough for a walk that visits a
/// cell's neighbours in turn:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellCache, CellKey, SystemRecord, generate_cell};
/// use hyperion_sim::id::Layer;
///
/// #[derive(Debug, Default)]
/// struct LastCell {
///     held: Option<(CellKey, Vec<SystemRecord>)>,
/// }
///
/// impl CellCache for LastCell {
///     fn with_cell<R>(
///         &mut self,
///         galaxy: &Galaxy,
///         key: CellKey,
///         f: impl FnOnce(&[SystemRecord]) -> R,
///     ) -> R {
///         let (_, systems) = match self.held.take() {
///             Some((held, systems)) if held == key => (held, systems),
///             _ => {
///                 let mut systems = Vec::new();
///                 generate_cell(galaxy, key, &mut systems);
///                 (key, systems)
///             }
///         };
///         let result = f(&systems);
///         self.held = Some((key, systems));
///         result
///     }
/// }
///
/// let galaxy = Galaxy::new(Seed::new(23));
/// let key = CellKey::new(Layer::D, [0, 406, 0])?;
/// let mut cache = LastCell::default();
/// let count = cache.with_cell(&galaxy, key, <[SystemRecord]>::len);
/// // The second answer comes from the cache and is the same.
/// assert_eq!(cache.with_cell(&galaxy, key, <[SystemRecord]>::len), count);
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
pub trait CellCache {
    /// Calls `f` with the systems of `key` in `galaxy`, generating the cell if the cache has not
    /// got it.
    ///
    /// The slice holds every accepted system of the cell in candidate-index order, exactly as
    /// [`generate_cell`] produces it.
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R;
}

/// The cache that caches nothing: it regenerates each cell into a scratch buffer.
///
/// It is the right choice for a one-off query, for a test, and for a caller that wants no memory at
/// all; it keeps the buffer's allocation between calls, so a walk over many cells allocates once.
/// Every result is identical to a real cache's, which is what the order-independence tests compare
/// against.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellCache, CellKey, NoCache, SystemRecord};
/// use hyperion_sim::id::Layer;
///
/// let galaxy = Galaxy::new(Seed::new(29));
/// let mut cache = NoCache::new();
/// let key = CellKey::new(Layer::E, [0, 203, 0])?;
/// let first = cache.with_cell(&galaxy, key, <[SystemRecord]>::to_vec);
/// assert_eq!(cache.with_cell(&galaxy, key, <[SystemRecord]>::to_vec), first);
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoCache {
    scratch: Vec<SystemRecord>,
}

impl NoCache {
    /// A `NoCache` with an empty scratch buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl CellCache for NoCache {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        generate_cell(galaxy, key, &mut self.scratch);
        f(&self.scratch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::params::GalaxyParams;
    use crate::id::Layer;
    use crate::rng::Seed;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(
            Seed::new(0x0300_cac4_0000_0000),
            GalaxyParams::milky_way_like(),
        )
    }

    #[test]
    fn no_cache_lends_what_generate_cell_produces() {
        let galaxy = galaxy();
        let mut cache = NoCache::new();
        let mut expected = Vec::new();
        for cell in [[0, 812, 0], [1, 812, 0], [0, 812, 0]] {
            let key = CellKey::new(Layer::C, cell).unwrap();
            generate_cell(&galaxy, key, &mut expected);
            let lent = cache.with_cell(&galaxy, key, <[SystemRecord]>::to_vec);
            assert_eq!(lent, expected, "{cell:?}");
        }
    }

    #[test]
    fn no_cache_keeps_its_buffer_between_calls() {
        let galaxy = galaxy();
        let mut cache = NoCache::new();
        let key = CellKey::new(Layer::C, [0, 812, 0]).unwrap();
        let count = cache.with_cell(&galaxy, key, <[SystemRecord]>::len);
        assert!(count > 0);
        assert!(cache.scratch.capacity() >= count);
        // An empty cell leaves the buffer empty but allocated.
        let empty = CellKey::new(Layer::A, [-8_000, 40, 6_000]).unwrap();
        assert_eq!(cache.with_cell(&galaxy, empty, <[SystemRecord]>::len), 0);
        assert!(cache.scratch.capacity() > 0);
    }
}
