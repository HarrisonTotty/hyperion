//! The catalogue features' and the galactic centre's lists as the alert walks read them, kept
//! between walks in one byte budget (the sim's `ListedHostsCache`; plan 09's P09.T36 in plan 12's
//! walks, fix-p09-alert-walk-features-2).
//!
//! From generator version 29 every class's walk reads the lists of the features within its reach
//! beside its cells, and a background walk of the whole cube reads every feature's: some
//! 2.5 × 10⁵ at the Milky Way fixture, whose lists cost the lists' walk of the core collapses 134
//! thread-seconds when each walk built them afresh (pod, 16 vCPU, dev). The server keeps each
//! feature cell's lists ([`ListedCell`]) and each galaxy's centre's ([`ListedHosts`]) in a
//! [`SharedByteLru`] keyed by the galaxy, so that every walk of a galaxy, interactive or a
//! background job, of any class or subscription, reads what another has built.
//!
//! An entry is a pure function of its galaxy and its key, and what a walk finds of it later the
//! same whenever it is found, so eviction is always safe and never changes an answer. A feature
//! cell spans the eight x-slabs of a 512 ly class's cells, whose jobs run side by side, and its
//! lists cost some 76 µs a feature, so an entry is built by one job while the others that need it
//! wait for it (a cold whole-cube walk otherwise built each cell up to eight times over). A galaxy
//! without its kinematic tables lists nothing and never reads the cache: its key is its
//! full-potential twin's, whose lists it must not be lent.

use std::collections::HashMap;
use std::mem::size_of;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use hyperion_sim::alerts::{ListedCell, ListedHosts, ListedHostsCache};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::catalogue::FeatureCellCache;
use hyperion_sim::id::FeatureCell;

#[cfg(test)]
use crate::cache::LruCounters;
use crate::cache::{HeapBytes, SharedByteLru};
use crate::compute::GalaxyKey;

/// What the cache keys an entry by: a feature cell's lists, or a galaxy's centre's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ListedKey {
    Cell(GalaxyKey, FeatureCell),
    Centre(GalaxyKey),
}

/// What the cache holds of one key.
#[derive(Debug)]
enum HeldLists {
    Cell(Arc<ListedCell>),
    Centre(Arc<ListedHosts>),
}

impl HeapBytes for HeldLists {
    /// The lists' own size, since each is lent behind an `Arc` of its own, and what they own on the
    /// heap, charged as if every class's hosts were found ([`ListedCell::heap_bytes`],
    /// [`ListedHosts::heap_bytes`]), so that an entry is charged the same whatever has read it.
    fn heap_bytes(&self) -> usize {
        match self {
            Self::Cell(cell) => size_of::<ListedCell>() + cell.heap_bytes(),
            Self::Centre(centre) => size_of::<ListedHosts>() + centre.heap_bytes(),
        }
    }
}

/// One entry being built, which the jobs that miss it meanwhile wait on.
type Flight = Arc<OnceLock<Arc<HeldLists>>>;

/// The lists of every galaxy the server holds as the alert walks read them, in one byte budget
/// (module documentation).
#[derive(Debug)]
pub(crate) struct SharedListedHostsCache {
    lists: SharedByteLru<ListedKey, HeldLists>,
    /// The entries being built, each by the job that first missed it: dropped once built.
    building: Mutex<HashMap<ListedKey, Flight>>,
}

impl SharedListedHostsCache {
    /// An empty cache that holds at most `budget_bytes` of charged lists.
    ///
    /// A budget of zero caches nothing: every cell's lists are built, lent and dropped.
    #[must_use]
    pub(crate) fn new(budget_bytes: usize) -> Self {
        Self {
            lists: SharedByteLru::new(budget_bytes),
            building: Mutex::new(HashMap::new()),
        }
    }

    /// The sim's [`ListedHostsCache`] over this cache, for the galaxy `galaxy` names.
    #[must_use]
    pub(crate) fn handle(&self, galaxy: GalaxyKey) -> ListedHostsHandle<'_> {
        ListedHostsHandle {
            cache: self,
            galaxy,
        }
    }

    /// The entries held, the bytes they are charged, and the hits, misses, evictions and refusals
    /// so far.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn counters(&self) -> LruCounters {
        self.lists.counters()
    }

    /// The lists of `cell` in `galaxy`, the galaxy `key` names: the cache's, or built of the cell's
    /// whole contents read through `features` and stored ([`ListedCell::of`]).
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of `key`, since its lists would be lent under another
    /// galaxy's key.
    fn cell(
        &self,
        key: GalaxyKey,
        galaxy: &Galaxy,
        cell: FeatureCell,
        features: &dyn FeatureCellCache,
    ) -> Arc<ListedCell> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a listed hosts cache handle lends the lists of one galaxy, and this is another's"
        );
        let build = || Arc::new(ListedCell::of(galaxy, &features.contents(galaxy, cell)));
        if galaxy.kinematics().is_none() {
            return build();
        }
        let held = self.held(ListedKey::Cell(key, cell), || HeldLists::Cell(build()));
        // A cell's key holds a cell's lists; anything else would be built again, to the same bits.
        match held.as_ref() {
            HeldLists::Cell(lists) => Arc::clone(lists),
            HeldLists::Centre(_) => build(),
        }
    }

    /// The galactic centre's list in `galaxy`, the galaxy `key` names: the cache's, or built and
    /// stored ([`ListedHosts::of_centre`]).
    ///
    /// # Panics
    ///
    /// As [`cell`](Self::cell), and as [`ListedHosts::of_centre`].
    fn centre(&self, key: GalaxyKey, galaxy: &Galaxy) -> Arc<ListedHosts> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a listed hosts cache handle lends the lists of one galaxy, and this is another's"
        );
        let build = || Arc::new(ListedHosts::of_centre(galaxy));
        if galaxy.kinematics().is_none() {
            return build();
        }
        let held = self.held(ListedKey::Centre(key), || HeldLists::Centre(build()));
        // A centre's key holds a centre's list; anything else would be built again, to the same
        // bits.
        match held.as_ref() {
            HeldLists::Centre(centre) => Arc::clone(centre),
            HeldLists::Cell(_) => build(),
        }
    }

    /// What the cache holds of `key`, or else what `build` makes, by this job or by the one
    /// already building it, which this one waits for, and stored (module documentation).
    fn held(&self, key: ListedKey, build: impl FnOnce() -> HeldLists) -> Arc<HeldLists> {
        if let Some(held) = self.lists.get(&key) {
            return held;
        }
        let flight = Arc::clone(
            self.building
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(key)
                .or_default(),
        );
        let held = Arc::clone(flight.get_or_init(|| {
            // Built meanwhile by a job whose flight has ended.
            self.lists.peek(&key).unwrap_or_else(|| {
                let held = Arc::new(build());
                // Lists larger than the whole budget are handed back and still lent.
                let _ = self.lists.insert(key, Arc::clone(&held));
                held
            })
        }));
        let mut building = self.building.lock().unwrap_or_else(PoisonError::into_inner);
        if building
            .get(&key)
            .is_some_and(|ongoing| Arc::ptr_eq(ongoing, &flight))
        {
            building.remove(&key);
        }
        held
    }
}

/// One galaxy's view of the [`SharedListedHostsCache`]: the sim's [`ListedHostsCache`], over the
/// cache every walk shares.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ListedHostsHandle<'a> {
    cache: &'a SharedListedHostsCache,
    galaxy: GalaxyKey,
}

impl ListedHostsCache for ListedHostsHandle<'_> {
    /// Lends the lists of `cell`, building them if the cache has not got them.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key.
    fn cell(
        &self,
        galaxy: &Galaxy,
        cell: FeatureCell,
        features: &dyn FeatureCellCache,
    ) -> Arc<ListedCell> {
        self.cache.cell(self.galaxy, galaxy, cell, features)
    }

    /// Lends the galactic centre's list, building it if the cache has not got it.
    ///
    /// # Panics
    ///
    /// As [`cell`](Self::cell), and as [`ListedHosts::of_centre`].
    fn centre(&self, galaxy: &Galaxy) -> Arc<ListedHosts> {
        self.cache.centre(self.galaxy, galaxy)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::alerts::NoListedHostsCache;
    use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;

    use super::*;

    /// The galaxy of seed 9 at the version, with its kinematic tables, and its key.
    fn galaxy() -> (Galaxy, GalaxyKey) {
        let seed = hyperion_sim::Seed::new(9);
        let version = hyperion_sim::GENERATOR_VERSION;
        let galaxy = Galaxy::from_params_at_version(
            seed,
            hyperion_sim::galaxy::params::GalaxyParams::from_seed(
                seed,
                hyperion_sim::galaxy::imf::MassFunctionKind::default(),
            ),
            version,
        )
        .unwrap()
        .with_full_potential();
        (galaxy, GalaxyKey::new(9, version))
    }

    /// A cell's lists and the centre's are built once and then lent from the cache, each equal to
    /// what the sim builds, and charged as their own size and heap bytes with the entry's overhead;
    /// a galaxy without its kinematic tables never reads the cache, and a handle of another seed's
    /// key is refused.
    #[test]
    fn a_cell_s_lists_and_the_centre_s_are_built_once_and_lent() {
        let (galaxy, key) = galaxy();
        let cache = SharedListedHostsCache::new(64 << 20);
        let handle = cache.handle(key);
        let cell = FeatureCell::new([0, 6, -1]).unwrap();
        let first = handle.cell(&galaxy, cell, &NoFeatureCache);
        let again = handle.cell(&galaxy, cell, &NoFeatureCache);
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(
            *first,
            *NoListedHostsCache.cell(&galaxy, cell, &NoFeatureCache)
        );
        let centre = handle.centre(&galaxy);
        assert!(Arc::ptr_eq(&centre, &handle.centre(&galaxy)));
        assert_eq!(*centre, ListedHosts::of_centre(&galaxy));
        let counters = cache.counters();
        assert_eq!(
            (counters.entries(), counters.hits(), counters.misses()),
            (2, 2, 2)
        );
        let charged = size_of::<ListedCell>()
            + first.heap_bytes()
            + size_of::<ListedHosts>()
            + centre.heap_bytes()
            + 2 * (size_of::<HeldLists>() + crate::cache::ENTRY_OVERHEAD_BYTES);
        assert_eq!(counters.bytes(), charged);
        let bare = Galaxy::from_params_at_version(
            galaxy.seed(),
            hyperion_sim::galaxy::params::GalaxyParams::from_seed(
                galaxy.seed(),
                hyperion_sim::galaxy::imf::MassFunctionKind::default(),
            ),
            hyperion_sim::GENERATOR_VERSION,
        )
        .unwrap();
        let unlisted = handle.cell(&bare, cell, &NoFeatureCache);
        assert!(!Arc::ptr_eq(&unlisted, &first));
        assert_eq!(
            cache.counters().entries(),
            2,
            "a bare galaxy stores nothing"
        );
        let other = cache.handle(GalaxyKey::new(10, hyperion_sim::GENERATOR_VERSION));
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            other.cell(&galaxy, cell, &NoFeatureCache)
        }));
        assert!(refused.is_err());
    }
}
