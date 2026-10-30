//! The caches of plan 09's large features, bounded in bytes (P09.T40.a): each feature cell's
//! contents, and each feature's interior.
//!
//! A query near a feature looks up every feature cell within the feature reach of its sphere
//! ([`FeatureCatalogue::near`]), and a feature with members builds its interior, its cluster model,
//! class table and nested grid ([`FeatureInterior::of`]): 14–17 ms each by plan 09's Risks
//! ("Ruling 139 as built", 139.5), and about 88 ms each in a test build (115 in 10.1 s, measured
//! 2026-10-30 for a 50 ly query at the Sun-like point). Two queries near one another ask for the same ones, so the server keeps both:
//! [`SharedFeatureCellCache`] is a [`SharedByteLru`] over `(GalaxyKey, FeatureCell)` and
//! [`SharedClusterModelCache`] one over `(GalaxyKey, FeatureId)`, so that two saves of one seed
//! share their entries and nothing generated under one generator version is lent for another
//! (plan 04, design note 23).
//!
//! An entry is epoch state, never anything at a time: a cell's features as the catalogue makes
//! them, an interior as [`FeatureInterior::of`] builds it. Each is a pure function of the galaxy
//! and its key, so eviction is always safe and never changes an answer. Each cache lends itself to
//! the sim through a handle for one galaxy, which is what the sim's caller-supplied traits take
//! ([`FeatureCellCache`] and [`FeatureInteriorCache`]): they take `&self`, so the handles reach
//! the shared caches through the [`SharedByteLru`]'s own lock. Two threads may build one entry at
//! the same time; the second insert replaces an equal value, which is cheaper than coordinating
//! (the same ruling as [`SharedByteLru`]'s).
//!
//! Like the cell cache's handle, a handle cannot check that it is lent the galaxy its key names
//! beyond its seed. A bare galaxy and its [`with_full_potential`](Galaxy::with_full_potential)
//! share a seed and differ in their members; what keeps them apart is that only
//! [`GalaxyCache`](super::GalaxyCache) builds the server's galaxies, always with their kinematic
//! tables.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::FeatureId;
use hyperion_sim::galaxy::features::catalogue::{
    FeatureCatalogue, FeatureCellCache, FeatureCellContents, FeatureRecord,
};
use hyperion_sim::galaxy::features::members::FeatureInterior;
use hyperion_sim::galaxy::features::members::source::FeatureInteriorCache;
use hyperion_sim::id::FeatureCell;

use super::GalaxyKey;
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

impl HeapBytes for FeatureCellContents {
    /// The sim's own count of what the cell owns on the heap: its features' buffer
    /// ([`FeatureCellContents::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

impl HeapBytes for FeatureInterior {
    /// The sim's own count of what the interior owns on the heap: its class table's rows and
    /// profiles ([`FeatureInterior::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// The feature cells of every galaxy the server holds, in one byte budget (P09.T40.a).
#[derive(Debug)]
pub struct SharedFeatureCellCache {
    cells: SharedByteLru<(GalaxyKey, FeatureCell), FeatureCellContents>,
}

impl SharedFeatureCellCache {
    /// An empty cache that holds at most `budget_bytes` of charged cells.
    ///
    /// A budget of zero caches nothing: every cell is generated, lent and dropped.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            cells: SharedByteLru::new(budget_bytes),
        }
    }

    /// The sim's [`FeatureCellCache`] over this cache, for the galaxy `galaxy` names.
    #[must_use]
    pub fn handle(&self, galaxy: GalaxyKey) -> FeatureCellHandle<'_> {
        FeatureCellHandle {
            cache: self,
            galaxy,
        }
    }

    /// The cells held, the bytes they are charged, and the hits, misses, evictions and refusals so
    /// far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.cells.counters()
    }

    /// The contents of `cell` in `galaxy`, the galaxy `key` names: the cache's, or generated and
    /// stored.
    fn contents(
        &self,
        key: GalaxyKey,
        galaxy: &Galaxy,
        cell: FeatureCell,
    ) -> Arc<FeatureCellContents> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a feature cell cache handle lends the cells of one galaxy, and this is another's"
        );
        let entry_key = (key, cell);
        if let Some(contents) = self.cells.get(&entry_key) {
            return contents;
        }
        let contents = Arc::new(FeatureCatalogue::cell(galaxy, cell));
        // A cell larger than the whole budget is handed back and still lent.
        let _ = self.cells.insert(entry_key, Arc::clone(&contents));
        contents
    }
}

/// One galaxy's view of the [`SharedFeatureCellCache`]: the sim's [`FeatureCellCache`], over the
/// cache every request shares.
#[derive(Debug, Clone, Copy)]
pub struct FeatureCellHandle<'a> {
    cache: &'a SharedFeatureCellCache,
    galaxy: GalaxyKey,
}

impl FeatureCellHandle<'_> {
    /// Which galaxy's cells this handle lends.
    #[must_use]
    pub fn galaxy(&self) -> GalaxyKey {
        self.galaxy
    }
}

impl FeatureCellCache for FeatureCellHandle<'_> {
    /// Lends the contents of `cell`, generating the cell if the cache has not got it.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key, since its cells would then be lent
    /// under another galaxy's key (the module documentation says what keeps the rest apart).
    fn contents(&self, galaxy: &Galaxy, cell: FeatureCell) -> Arc<FeatureCellContents> {
        self.cache.contents(self.galaxy, galaxy, cell)
    }
}

/// The feature interiors of every galaxy the server holds, in one byte budget: each feature's
/// cluster model with the class table and nested grid built on it (P09.T40.a).
///
/// A feature with no members (a cloud, an association, a dissolved cluster) has no interior, and
/// is not stored: the sim answers it from its marks in microseconds
/// ([`ClusterModel::from_record`](hyperion_sim::galaxy::features::cluster::ClusterModel::from_record)).
#[derive(Debug)]
pub struct SharedClusterModelCache {
    interiors: SharedByteLru<(GalaxyKey, FeatureId), FeatureInterior>,
}

impl SharedClusterModelCache {
    /// An empty cache that holds at most `budget_bytes` of charged interiors.
    ///
    /// A budget of zero caches nothing: every interior is built, lent and dropped.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            interiors: SharedByteLru::new(budget_bytes),
        }
    }

    /// The sim's [`FeatureInteriorCache`] over this cache, for the galaxy `galaxy` names.
    #[must_use]
    pub fn handle(&self, galaxy: GalaxyKey) -> ClusterModelHandle<'_> {
        ClusterModelHandle {
            cache: self,
            galaxy,
        }
    }

    /// One job's view of this cache for the galaxy `galaxy` names, which keeps every interior the
    /// job reads for as long as it lives ([`KeptInteriors`]).
    #[must_use]
    pub fn kept(&self, galaxy: GalaxyKey) -> KeptInteriors<'_> {
        KeptInteriors {
            shared: self.handle(galaxy),
            kept: Mutex::new(BTreeMap::new()),
        }
    }

    /// The interiors held, the bytes they are charged, and the hits, misses, evictions and
    /// refusals so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.interiors.counters()
    }

    /// The interior of `feature` in `galaxy`, the galaxy `key` names: the cache's, or built and
    /// stored; `None` for a feature with no members, which is not stored.
    ///
    /// A galaxy built without its kinematic tables has no members at all
    /// ([`FeatureInterior::of`]), so it never reads the cache: its key is its full-potential
    /// twin's, whose interiors it must not be lent.
    fn interior(
        &self,
        key: GalaxyKey,
        galaxy: &Galaxy,
        feature: &FeatureRecord,
    ) -> Option<Arc<FeatureInterior>> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a cluster model cache handle lends the interiors of one galaxy, and this is another's"
        );
        galaxy.kinematics()?;
        let entry_key = (key, feature.id());
        if let Some(interior) = self.interiors.get(&entry_key) {
            return Some(interior);
        }
        let interior = Arc::new(FeatureInterior::of(galaxy, feature)?);
        // An interior larger than the whole budget is handed back and still lent.
        let _ = self.interiors.insert(entry_key, Arc::clone(&interior));
        Some(interior)
    }
}

/// One galaxy's view of the [`SharedClusterModelCache`]: the sim's [`FeatureInteriorCache`], over
/// the cache every request shares.
#[derive(Debug, Clone, Copy)]
pub struct ClusterModelHandle<'a> {
    cache: &'a SharedClusterModelCache,
    galaxy: GalaxyKey,
}

impl ClusterModelHandle<'_> {
    /// Which galaxy's interiors this handle lends.
    #[must_use]
    pub fn galaxy(&self) -> GalaxyKey {
        self.galaxy
    }
}

impl FeatureInteriorCache for ClusterModelHandle<'_> {
    /// Lends the interior of `feature`, building it if the cache has not got it.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key, as
    /// [`FeatureCellHandle`]'s lookup.
    fn interior(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Arc<FeatureInterior>> {
        self.cache.interior(self.galaxy, galaxy, feature)
    }
}

/// One job's view of the [`SharedClusterModelCache`]: every interior the job reads is kept for as
/// long as the job holds this, over the shared cache, so that a job builds each feature's interior
/// at most once whatever the shared budget. A range query's census, its hits and its rows' members
/// all read the same interiors, a hundred or so near the Sun.
///
/// What it keeps is bounded by the features one job reads, and is dropped with it.
#[derive(Debug)]
pub struct KeptInteriors<'a> {
    shared: ClusterModelHandle<'a>,
    kept: Mutex<BTreeMap<FeatureId, Option<Arc<FeatureInterior>>>>,
}

impl FeatureInteriorCache for KeptInteriors<'_> {
    /// Lends the interior of `feature`: the job's own, or the shared cache's, kept.
    ///
    /// # Panics
    ///
    /// As [`ClusterModelHandle`]'s lookup, if `galaxy` is another's.
    fn interior(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Arc<FeatureInterior>> {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        kept.entry(feature.id())
            .or_insert_with(|| self.shared.interior(galaxy, feature))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
    use hyperion_sim::galaxy::features::members::NoInteriorCache;
    use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
    use hyperion_sim::galaxy::placement::NoCache;
    use hyperion_sim::galaxy::query::{RangeQuery, RangeResult, range_query};
    use hyperion_sim::id::SystemIdKind;
    use hyperion_sim::units::LightYears;
    use hyperion_sim::{GENERATOR_VERSION, Seed};

    use super::*;
    use crate::cache::ENTRY_OVERHEAD_BYTES;
    use crate::convert::origins::{SEED, galaxy};

    fn key() -> GalaxyKey {
        GalaxyKey::new(SEED, GENERATOR_VERSION)
    }

    fn sun() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube")
    }

    /// The first feature with members within 3,000 ly of the Sun-like point, as the server's origin
    /// tests find it.
    fn a_cluster() -> &'static FeatureRecord {
        static FEATURE: OnceLock<FeatureRecord> = OnceLock::new();
        FEATURE.get_or_init(|| {
            FeatureCatalogue::near(galaxy(), &sun(), LightYears::new(3_000.0), &NoFeatureCache)
                .find(|feature| FeatureInterior::of(galaxy(), feature).is_some())
                .expect("a feature with members lies within 3,000 ly of the Sun")
        })
    }

    /// The cells a 3,000 ly lookup about the Sun-like point reads are the catalogue's, through
    /// every budget, and a cache roomy enough lends the second lookup from what it stored.
    #[test]
    fn a_feature_cell_is_the_catalogue_s_and_the_second_ask_is_a_hit() {
        let uncached: Vec<_> =
            FeatureCatalogue::near(galaxy(), &sun(), LightYears::new(3_000.0), &NoFeatureCache)
                .collect();
        assert!(
            !uncached.is_empty(),
            "features lie within 3,000 ly of the Sun"
        );
        let roomy = SharedFeatureCellCache::new(64 << 20);
        for cache in [
            &roomy,
            &SharedFeatureCellCache::new(0),
            &SharedFeatureCellCache::new(4_096),
        ] {
            let handle = cache.handle(key());
            assert_eq!(handle.galaxy(), key());
            let cached: Vec<_> =
                FeatureCatalogue::near(galaxy(), &sun(), LightYears::new(3_000.0), &handle)
                    .collect();
            assert_eq!(cached, uncached);
        }
        let first = roomy.counters();
        assert!(first.entries() > 0 && first.misses() > 0);
        let cell = a_cluster().id().cell();
        let stored = FeatureCellCache::contents(&roomy.handle(key()), galaxy(), cell);
        assert_eq!(*stored, FeatureCatalogue::cell(galaxy(), cell));
        let again = roomy.counters();
        assert_eq!(again.hits(), first.hits() + 1, "the second ask is a hit");
        assert_eq!(again.misses(), first.misses());
        // Nothing cached at a budget of zero.
        let none = SharedFeatureCellCache::new(0);
        let _ = FeatureCellCache::contents(&none.handle(key()), galaxy(), cell);
        assert_eq!(none.counters().entries(), 0);
    }

    /// An interior is [`FeatureInterior::of`]'s, is charged its heap bytes, and is lent again from
    /// the cache; a feature with no members has none, and nothing is stored for it.
    #[test]
    fn an_interior_is_the_sim_s_charged_and_lent_again() {
        let cache = SharedClusterModelCache::new(64 << 20);
        let handle = cache.handle(key());
        assert_eq!(handle.galaxy(), key());
        let feature = a_cluster();
        let built = handle
            .interior(galaxy(), feature)
            .expect("the cluster has members");
        let expected = FeatureInterior::of(galaxy(), feature).expect("the cluster has members");
        assert_eq!(*built, expected);
        assert!(expected.heap_bytes() > 0);
        let again = handle
            .interior(galaxy(), feature)
            .expect("the cluster has members");
        assert!(Arc::ptr_eq(&built, &again), "the second ask is the cache's");
        let counters = cache.counters();
        assert_eq!(
            (counters.misses(), counters.hits(), counters.entries()),
            (1, 1, 1)
        );
        assert_eq!(
            counters.bytes(),
            expected.heap_bytes() + size_of::<FeatureInterior>() + ENTRY_OVERHEAD_BYTES
        );
        // A bare galaxy has no members: its features have no interior, and nothing is stored.
        let bare = hyperion_sim::galaxy::Galaxy::new(Seed::new(SEED));
        let empty = SharedClusterModelCache::new(64 << 20);
        assert!(empty.handle(key()).interior(&bare, feature).is_none());
        assert_eq!(empty.counters().entries(), 0);
    }

    /// The features' members as the range query finds them about `feature`, through `cells` and
    /// `interiors`.
    fn members_about(
        feature: &FeatureRecord,
        cells: &dyn FeatureCellCache,
        interiors: &dyn FeatureInteriorCache,
    ) -> RangeResult {
        let source = FeatureMemberSource::new(cells, interiors);
        let query = RangeQuery::builder(*feature.position(), LightYears::new(2.0))
            .build()
            .expect("a query");
        range_query(galaxy(), &mut NoCache::new(), &[&source], &query)
    }

    /// Eviction never changes a reply: a range query about a cluster with its members' source
    /// finds the same hits and census through the caches, cold and warm, as with none, whether
    /// they keep nothing, evict all the time (a few kilobytes of cells, a few interiors of the
    /// hundred or so the query builds) or keep everything.
    ///
    /// Each cold query builds the interior of every feature whose reach touches its padded sphere,
    /// about a hundred near the Sun at some 90 ms each in a test build, so the budgets come in
    /// three pairs rather than all their combinations.
    #[test]
    fn eviction_never_changes_a_range_query_s_members() {
        let feature = a_cluster();
        let expected = members_about(feature, &NoFeatureCache, &NoInteriorCache);
        assert!(
            expected
                .systems()
                .iter()
                .any(|hit| matches!(hit.id().kind(), SystemIdKind::FeatureMember(_))),
            "the cluster's members are found"
        );
        for (cell_budget, interior_budget) in [(0, 0), (4_096, 100_000), (64 << 20, 64 << 20)] {
            let cells = SharedFeatureCellCache::new(cell_budget);
            let interiors = SharedClusterModelCache::new(interior_budget);
            for pass in ["cold", "warm"] {
                let found = members_about(feature, &cells.handle(key()), &interiors.handle(key()));
                assert_eq!(
                    found.systems(),
                    expected.systems(),
                    "{pass}, budgets {cell_budget} and {interior_budget}"
                );
                assert_eq!(found.census(), expected.census());
            }
            // A job's kept view over a cache that keeps nothing builds each interior once.
            if interior_budget == 0 {
                let kept = interiors.kept(key());
                let found = members_about(feature, &cells.handle(key()), &kept);
                assert_eq!(found.systems(), expected.systems(), "kept");
            }
            if interior_budget == 100_000 {
                assert!(
                    interiors.counters().evictions() > 0,
                    "the tight budget evicts: {:?}",
                    interiors.counters()
                );
            }
        }
    }
}
