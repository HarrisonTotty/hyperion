//! Plan 10's tube tables, kept in one byte budget for every galaxy the server holds (P10.T10.b).
//!
//! A stream's [`TubeTable`] is what its members are placed and resolved from, some 90 kB and
//! 1.3–2.1 s to build on one thread (plan 10's Risks, "P10.T6.a as built"). [`TubeTableCache`] is
//! a [`SharedByteLru`] over `(GalaxyKey, StreamNumber)`, so that two saves of one seed share their
//! tables and nothing built under one generator version is lent for another (plan 04, design
//! note 23). A table is a pure function of its galaxy and its stream, so eviction never changes an
//! answer: a table evicted is built again, to the same bits.
//!
//! [`ListedTubes`] is one galaxy's view of the cache with its global list: the sim's
//! [`TubeLookup`], which [`resolve_with`](hyperion_sim::galaxy::placement::resolve_with) and the
//! range query's [`GlobalListSource`](hyperion_sim::galaxy::global_list::GlobalListSource) read
//! through. It is made on the runtime and moved into the job that resolves, and a table it lacks is
//! built inside that job on one thread, since a pool job never waits on others
//! ([`global_list_on`](super::global_list_on)'s documentation). The service's own builds, for
//! `stream_track` and the warm-up, spray on the pool's workers instead
//! ([`GlobalListService::tube`](super::GlobalListService::tube)).

use std::sync::Arc;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::global_list::{
    GlobalList, GlobalListModels, StreamNumber, TubeLookup, TubeTable,
};
use hyperion_sim::galaxy::placement::{ResolveSystemError, SystemRecord, resolve, resolve_with};
use hyperion_sim::id::SystemId;

use super::{GalaxyKey, HeldGlobalList};
use crate::cache::{HeapBytes, Insertion, LruCounters, SharedByteLru};

/// What names a tube table in the cache: its galaxy, and its stream.
pub type TubeKey = (GalaxyKey, StreamNumber);

impl HeapBytes for TubeTable {
    /// The sim's own count of the table's bytes, its knots and cells ([`TubeTable::byte_size`]).
    fn heap_bytes(&self) -> usize {
        self.byte_size()
    }
}

/// The tube tables of every galaxy the server holds, in one byte budget.
#[derive(Debug)]
pub struct TubeTableCache {
    tables: SharedByteLru<TubeKey, TubeTable>,
}

impl TubeTableCache {
    /// An empty cache that holds at most `budget_bytes` of charged tables.
    ///
    /// A budget of zero keeps none: each table is built, read and dropped.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            tables: SharedByteLru::new(budget_bytes),
        }
    }

    /// The table `key` names, if held, counted as a hit or a miss.
    #[must_use]
    pub fn get(&self, key: &TubeKey) -> Option<Arc<TubeTable>> {
        self.tables.get(key)
    }

    /// Whether the table `key` names is held, uncounted: what a stream's track status reads.
    #[must_use]
    pub fn contains(&self, key: &TubeKey) -> bool {
        self.tables.contains_key(key)
    }

    /// Keeps `table` under `key`, evicting the least recently used tables to make room; a table
    /// larger than the whole budget is not kept.
    pub fn insert(&self, key: TubeKey, table: Arc<TubeTable>) -> Insertion<TubeTable> {
        self.tables.insert(key, table)
    }

    /// The tables held, the bytes they are charged, and the hits, misses, evictions and refusals
    /// so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.tables.counters()
    }
}

/// One galaxy's global list and its streams' tube tables from a [`TubeTableCache`]: the sim's
/// [`TubeLookup`] for that galaxy (module documentation).
///
/// A table the cache lacks is built on the calling thread with [`TubeTable::build`] and kept.
/// Two jobs may build one table at once; the second insert replaces an equal value, which is
/// cheaper than coordinating (the ruling the server's other caches follow).
#[derive(Clone)]
pub struct ListedTubes {
    galaxy: Arc<Galaxy>,
    key: GalaxyKey,
    list: Arc<HeldGlobalList>,
    tables: Arc<TubeTableCache>,
}

impl ListedTubes {
    /// The view of `tables` for `galaxy`, the galaxy `key` names, whose global list is `list`.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of `key`, since its tables would then be kept under
    /// another galaxy's key. The server takes the galaxy, the key and the list from the request's
    /// own universe, so a mismatch is a bug.
    #[must_use]
    pub fn new(
        (galaxy, key): (Arc<Galaxy>, GalaxyKey),
        list: Arc<HeldGlobalList>,
        tables: Arc<TubeTableCache>,
    ) -> Self {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a galaxy's tube tables are kept under its own key, and this is another's"
        );
        Self {
            galaxy,
            key,
            list,
            tables,
        }
    }
}

impl std::fmt::Debug for ListedTubes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListedTubes")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl TubeLookup for ListedTubes {
    fn global_list(&self) -> &GlobalList {
        self.list.list()
    }

    /// The table of `stream`: the cache's, or built here and kept.
    ///
    /// # Panics
    ///
    /// If the list has no `stream`, which the sim never asks for, and if the table cannot be built:
    /// a galaxy without its full potential, which the server's galaxy cache never builds, or a
    /// stream whose spray keeps too few tracers, which no stream of a drawn galaxy has met (plan
    /// 10's Risks, "P10.T6.a as built", "For T9 and T10.b").
    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        let key = (self.key, stream);
        if let Some(table) = self.tables.get(&key) {
            return table;
        }
        let spec = self
            .list
            .list()
            .stream(stream)
            .expect("the sim asks only for a stream of the list");
        let table = Arc::new(
            TubeTable::build(&self.galaxy, spec)
                .expect("a stream of a galaxy with its full potential builds its tube"),
        );
        // A table larger than the whole budget is handed back and still read.
        drop(self.tables.insert(key, Arc::clone(&table)));
        table
    }

    /// The list's models the service keeps with it ([`HeldGlobalList::models`]), which every
    /// query of the galaxy reads (P09.T37's bench).
    fn models(&self) -> Option<&GlobalListModels> {
        Some(self.list.models())
    }
}

/// What resolving a system's ID reads of its galaxy's global list (P10.T10.b): what
/// [`GlobalListService::listing`](super::GlobalListService::listing) gives a request for the job
/// that resolves the ID.
#[derive(Debug, Clone)]
pub enum Listing {
    /// Nothing: the ID names no member of the list, and plan 03's [`resolve`] resolves it.
    Unlisted,
    /// The galaxy's list and its tables, for a stream's or a dwarf core's member.
    Listed(ListedTubes),
}

impl Listing {
    /// The system `id` names in `galaxy`: through the list with [`resolve_with`] if listed, and
    /// with plan 03's [`resolve`] if not.
    ///
    /// # Errors
    ///
    /// As [`resolve_with`] or [`resolve`]: [`ResolveSystemError::KindNotGenerated`] for a list
    /// member's ID resolved unlisted.
    pub fn resolve(
        &self,
        galaxy: &Galaxy,
        id: SystemId,
    ) -> Result<SystemRecord, ResolveSystemError> {
        match self {
            Self::Unlisted => resolve(galaxy, id),
            Self::Listed(tubes) => resolve_with(galaxy, id, tubes),
        }
    }
}
