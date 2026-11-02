//! The caches of plan 09's large features, bounded in bytes: each feature cell's contents and each
//! feature's interior (P09.T40.a), and the galaxy map's globulars (P09.T40.b).
//!
//! A query near a feature looks up every feature cell within the feature reach of its sphere
//! ([`FeatureCatalogue::near`]), and a feature with members whose nested grid may meet the sphere,
//! by a bound from its cluster model ([`FeatureInterior::grid_reach_bound`]), builds its interior,
//! its cluster model, class table and nested grid ([`FeatureInterior::of`]): 14–17 ms each by plan
//! 09's Risks ("Ruling 139 as built", 139.5). A 50 ly query at the Sun-like point looks up 115
//! features with members and builds one (measured 2026-10-30). Two queries near one another ask
//! for the same ones, so the server keeps both:
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
//! the same time; the second insert replaces an equal value, or for a cell is dropped, which is
//! cheaper than coordinating (the same ruling as [`SharedByteLru`]'s). A cell a lookup reads only
//! through some of its processes ([`FeatureCellCache::contents_through`]) is held so until a
//! lookup needs more of it.
//!
//! The galaxy map's globulars are every globular of a galaxy with its expected members, found once
//! in a bulk job and kept per galaxy by the [`GalaxyFeaturesService`], as the density maps are;
//! each request describes them at its own time.
//!
//! Like the cell cache's handle, a handle cannot check that it is lent the galaxy its key names
//! beyond its seed. A bare galaxy and its [`with_full_potential`](Galaxy::with_full_potential)
//! share a seed and differ in their members; what keeps them apart is that only
//! [`GalaxyCache`](super::GalaxyCache) builds the server's galaxies, always with their kinematic
//! tables.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::catalogue::{
    FeatureCatalogue, FeatureCellCache, FeatureCellContents, FeatureRecord,
};
use hyperion_sim::galaxy::features::cluster::ClusterModel;
use hyperion_sim::galaxy::features::interior::counts::{ClassCounts, class_counts};
use hyperion_sim::galaxy::features::members::source::FeatureInteriorCache;
use hyperion_sim::galaxy::features::members::{FeatureInterior, FeatureLevelList};
use hyperion_sim::galaxy::features::{FeatureId, FeatureProcess};
use hyperion_sim::id::FeatureCell;
use hyperion_sim::units::LightYears;

use super::sky::bulk;
use super::{
    CancelOnDrop, CancelToken, ComputeError, CpuPool, GalaxyCache, GalaxyKey, SingleFlight,
};
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

impl HeapBytes for FeatureCellContents {
    /// The sim's own count of what the cell owns on the heap: its features' buffer
    /// ([`FeatureCellContents::heap_bytes`]).
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// What [`SharedClusterModelCache`] holds of one feature: its interior once a query has needed
/// it, and before that the bound on its nested grid's reach that the query tested it by
/// ([`FeatureInterior::grid_reach_bound`]; P09.T40.a).
#[derive(Debug)]
enum HeldFeature {
    /// Its interior.
    Interior(Arc<FeatureInterior>),
    /// The bound on its grid's reach, or `None` for a feature with no members.
    Reach(Option<LightYears>),
}

impl HeapBytes for HeldFeature {
    /// An interior's own size, since it is lent behind an `Arc` of its own, and what it owns on the
    /// heap: its class table's rows and profiles ([`FeatureInterior::heap_bytes`]). A bound owns
    /// nothing.
    fn heap_bytes(&self) -> usize {
        match self {
            Self::Interior(interior) => size_of::<FeatureInterior>() + interior.heap_bytes(),
            Self::Reach(_) => 0,
        }
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

    /// Whether the cache holds `cell` of the galaxy `galaxy` names through at least `last`
    /// ([`FeatureCellContents::holds`]), without counting a lookup or marking a use: what a served
    /// query asks before it generates its cells ahead of its lookups
    /// ([`served_range_query`](super::served_range_query); P09.T40.a).
    #[must_use]
    pub fn holds(&self, galaxy: GalaxyKey, cell: FeatureCell, last: FeatureProcess) -> bool {
        self.cells
            .peek(&(galaxy, cell))
            .is_some_and(|contents| contents.holds(last))
    }

    /// Contents of `cell` in `galaxy`, the galaxy `key` names, that hold its processes up to
    /// `last`: the cache's if they hold them, or generated through `last` and stored
    /// ([`FeatureCatalogue::cell_through`]; the whole cell for [`FeatureProcess::Cloud`]).
    ///
    /// A cell made through fewer processes than a query needs is made again through more and
    /// replaces the one held, which is part of it; one is never stored over a cell holding more
    /// that another job stored meanwhile (P09.T40.a). A held cell that holds too little counts as
    /// a hit in the [`counters`](Self::counters), as it is found, though it is then made again.
    fn contents(
        &self,
        key: GalaxyKey,
        galaxy: &Galaxy,
        cell: FeatureCell,
        last: FeatureProcess,
    ) -> Arc<FeatureCellContents> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a feature cell cache handle lends the cells of one galaxy, and this is another's"
        );
        let entry_key = (key, cell);
        let held = self.cells.get(&entry_key);
        if let Some(contents) = &held
            && contents.holds(last)
        {
            return Arc::clone(contents);
        }
        let contents = Arc::new(FeatureCatalogue::cell_through(galaxy, cell, last));
        // A cell larger than the whole budget is handed back and still lent, and one is lent and
        // not stored where another job replaced the cell meanwhile: what it stored is the
        // catalogue's too, through at least as many processes or as few, and either is lent right.
        let _ = self
            .cells
            .insert_if_unchanged(entry_key, held.as_ref(), Arc::clone(&contents));
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
    /// Lends the whole contents of `cell`, generating the cell if the cache has not got it whole.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key, since its cells would then be lent
    /// under another galaxy's key (the module documentation says what keeps the rest apart).
    fn contents(&self, galaxy: &Galaxy, cell: FeatureCell) -> Arc<FeatureCellContents> {
        self.cache
            .contents(self.galaxy, galaxy, cell, FeatureProcess::Cloud)
    }

    /// Lends contents of `cell` that hold its processes up to `last`: the cell the cache holds if
    /// it holds them, or the cell made through `last` otherwise, then held in its place.
    ///
    /// # Panics
    ///
    /// As [`contents`](Self::contents).
    fn contents_through(
        &self,
        galaxy: &Galaxy,
        cell: FeatureCell,
        last: FeatureProcess,
    ) -> Arc<FeatureCellContents> {
        self.cache.contents(self.galaxy, galaxy, cell, last)
    }
}

/// The feature interiors of every galaxy the server holds, in one byte budget: each feature's
/// cluster model with the class table and nested grid built on it (P09.T40.a).
///
/// A feature a range query looked up but whose grid its bound kept clear of the sphere
/// ([`FeatureInteriorCache::grid_reach`]) is held by that bound alone, a hundred bytes or so
/// against the interior's tens of kilobytes, so that a query near it again tests it without its
/// cluster model. A feature with no members (a cloud, an association, a dissolved cluster) has no
/// interior: it is held by its bound's `None` once a query has tested it, and nothing is stored
/// for it otherwise. A lookup that finds a feature's bound counts as a hit, the interior lookup
/// that then builds it too.
#[derive(Debug)]
pub struct SharedClusterModelCache {
    interiors: SharedByteLru<(GalaxyKey, FeatureId), HeldFeature>,
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
            reaches: Mutex::new(BTreeMap::new()),
            lists: Mutex::new(BTreeMap::new()),
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
        if let Some(held) = self.interiors.get(&entry_key) {
            match &*held {
                HeldFeature::Interior(interior) => return Some(Arc::clone(interior)),
                // The bound is `None` only where the sim builds no interior.
                HeldFeature::Reach(None) => return None,
                HeldFeature::Reach(Some(_)) => {}
            }
        }
        let interior = Arc::new(FeatureInterior::of(galaxy, feature)?);
        // An interior larger than the whole budget is handed back and still lent. It replaces the
        // feature's bound, which its grid's reach supersedes.
        let _ = self.interiors.insert(
            entry_key,
            Arc::new(HeldFeature::Interior(Arc::clone(&interior))),
        );
        Some(interior)
    }

    /// The reach of the grid of `feature`'s interior in `galaxy`, the galaxy `key` names, if the
    /// cache holds it, or else the bound on it, the cache's or computed and stored; `None` for a
    /// feature with no members.
    ///
    /// As [`interior`](Self::interior), a galaxy without its kinematic tables never reads the cache.
    fn grid_reach(
        &self,
        key: GalaxyKey,
        galaxy: &Galaxy,
        feature: &FeatureRecord,
    ) -> Option<LightYears> {
        assert_eq!(
            galaxy.seed().get(),
            key.seed(),
            "a cluster model cache handle lends the interiors of one galaxy, and this is another's"
        );
        galaxy.kinematics()?;
        let entry_key = (key, feature.id());
        if let Some(held) = self.interiors.get(&entry_key) {
            return match &*held {
                HeldFeature::Interior(interior) => Some(interior.grid().reach()),
                HeldFeature::Reach(reach) => *reach,
            };
        }
        let reach = FeatureInterior::grid_reach_bound(galaxy, feature);
        // Only if nothing was stored meanwhile, so that a bound never displaces an interior another
        // job has just built.
        let _ = self.interiors.insert_if_unchanged(
            entry_key,
            None,
            Arc::new(HeldFeature::Reach(reach)),
        );
        reach
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

    /// Lends the reach of the grid of `feature`'s interior if the cache holds it, or else the
    /// bound on it, computing the bound if the cache has not got it.
    ///
    /// # Panics
    ///
    /// As [`interior`](Self::interior).
    fn grid_reach(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<LightYears> {
        self.cache.grid_reach(self.galaxy, galaxy, feature)
    }
}

/// One job's view of the [`SharedClusterModelCache`]: every interior the job reads is kept for as
/// long as the job holds this, over the shared cache, so that a job builds each feature's interior
/// at most once whatever the shared budget, and so is every bound on a grid's reach it tests a
/// feature by. A range query's census, its hits and its rows' members all read the same
/// interiors, and its census and hits the bounds of some hundred features near the Sun.
///
/// What it keeps is bounded by the features one job reads, and is dropped with it.
#[derive(Debug)]
pub struct KeptInteriors<'a> {
    shared: ClusterModelHandle<'a>,
    kept: Mutex<BTreeMap<FeatureId, Option<Arc<FeatureInterior>>>>,
    reaches: Mutex<BTreeMap<FeatureId, Option<LightYears>>>,
    /// The features' lists the job reads (P09.T37), built once a job: its census and its hits read
    /// the same, some thousand of them near the inner disc.
    lists: Mutex<BTreeMap<FeatureId, Option<Arc<FeatureLevelList>>>>,
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

    /// Lends the reach of the grid of the interior the job keeps, or else the bound on it: the
    /// job's own, or the shared cache's, kept.
    ///
    /// # Panics
    ///
    /// As [`ClusterModelHandle`]'s lookup, if `galaxy` is another's.
    fn grid_reach(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<LightYears> {
        if let Some(interior) = self
            .kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&feature.id())
        {
            return interior.as_ref().map(|interior| interior.grid().reach());
        }
        *self
            .reaches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(feature.id())
            .or_insert_with(|| self.shared.grid_reach(galaxy, feature))
    }

    /// Lends `feature`'s feature-level list: the job's own, built once.
    fn list(&self, galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Arc<FeatureLevelList>> {
        self.lists
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(feature.id())
            .or_insert_with(|| self.shared.list(galaxy, feature))
            .clone()
    }
}

/// The members a cluster's class counts expect: their sum, its tails' included.
///
/// Summed in a fixed order, the classes in their order and then the tails in band order, which
/// the answers' bits follow.
#[must_use]
pub(crate) fn counted_members(counts: &ClassCounts) -> f64 {
    let classes: f64 = counts.classes.iter().map(|class| class.expected).sum();
    let tails: f64 = counts
        .tails
        .iter()
        .flatten()
        .map(|tail| tail.expected)
        .sum();
    classes + tails
}

/// The members a cluster of model `model` and reach `reach` expects ([`counted_members`] of plan
/// 09's `class_counts`).
///
/// About a millisecond a cluster in a test build (measured 2026-10-30), against 19 ms for its whole
/// interior.
#[must_use]
pub(crate) fn expected_members_of(galaxy: &Galaxy, model: &ClusterModel, reach: LightYears) -> f64 {
    counted_members(&class_counts(galaxy, model, reach))
}

/// A catalogue feature's expected members at the epoch, from its cluster model.
///
/// None for a feature with no cluster model: a cloud, an association, an unbound embedded region.
#[must_use]
pub(crate) fn expected_members(galaxy: &Galaxy, feature: &FeatureRecord) -> f64 {
    ClusterModel::from_record(galaxy, feature).map_or(0.0, |model| {
        expected_members_of(galaxy, &model, feature.reach())
    })
}

/// One globular of the galaxy map's list: its record and its expected members, as epoch state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapGlobular {
    record: FeatureRecord,
    expected_members: f64,
}

impl MapGlobular {
    /// The globular's record.
    #[must_use]
    pub fn record(&self) -> &FeatureRecord {
        &self.record
    }

    /// The members its class counts expect.
    #[must_use]
    pub fn expected_members(&self) -> f64 {
        self.expected_members
    }
}

/// The galaxy map's globulars, as the map's list keeps them between requests (P09.T40.b).
///
/// Every globular of the catalogue, in its walk order, with its expected members, as epoch state;
/// each request describes them at its own time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapGlobulars(Vec<MapGlobular>);

impl MapGlobulars {
    /// Every globular of `galaxy`, by the catalogue's walk over its cells, with its expected
    /// members.
    ///
    /// The walk is [`FeatureCatalogue::walk_process`]'s: 142 globulars in the server's tests'
    /// galaxy, walked in 17 ms and counted in 56 ms in a test build (measured 2026-10-30).
    #[must_use]
    pub fn of(galaxy: &Galaxy) -> Self {
        let mut globulars: Vec<_> =
            FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
                .map(|record| MapGlobular {
                    expected_members: expected_members(galaxy, &record),
                    record,
                })
                .collect();
        globulars.shrink_to_fit();
        Self(globulars)
    }

    /// The globulars, in the catalogue's walk order.
    #[must_use]
    pub fn globulars(&self) -> &[MapGlobular] {
        &self.0
    }
}

impl HeapBytes for MapGlobulars {
    /// The entries' buffer: a record owns nothing on the heap itself.
    fn heap_bytes(&self) -> usize {
        self.0.capacity() * size_of::<MapGlobular>()
    }
}

/// The galaxy map's feature lists of every galaxy the server holds, computed in the background and
/// kept per galaxy, as the density maps are (P09.T40.b).
///
/// [`GalaxyFeaturesService::get`] hands back a galaxy's globulars, finding them in a bulk job on
/// the CPU pool if no one has yet, the galaxy taken from the [`GalaxyCache`]. Callers that ask for
/// one galaxy at once share one job through a [`SingleFlight`], and if every one of them gives up,
/// the job still queued is skipped.
pub struct GalaxyFeaturesService {
    pool: Arc<CpuPool>,
    galaxies: Arc<GalaxyCache>,
    held: Arc<SharedByteLru<GalaxyKey, MapGlobulars>>,
    flights: SingleFlight<GalaxyKey, MapGlobulars, ComputeError>,
}

impl GalaxyFeaturesService {
    /// A service that finds its lists on `pool` from the galaxies of `galaxies`, keeping at most
    /// `budget_bytes` of them.
    ///
    /// A budget of zero keeps none.
    #[must_use]
    pub fn new(pool: Arc<CpuPool>, galaxies: Arc<GalaxyCache>, budget_bytes: usize) -> Self {
        Self {
            pool,
            galaxies,
            held: Arc::new(SharedByteLru::new(budget_bytes)),
            flights: SingleFlight::new(),
        }
    }

    /// The globulars of the galaxy `key` names, found if they are not held already.
    ///
    /// # Errors
    ///
    /// [`ComputeError`] as [`GalaxyCache::get`] gives it for the galaxy, and for the job:
    /// [`ComputeError::Submit`] if the pool is shutting down, and [`ComputeError::Job`] if it was
    /// cancelled, panicked, or was still queued when the pool stopped.
    pub async fn get(&self, key: GalaxyKey) -> Result<Arc<MapGlobulars>, ComputeError> {
        if let Some(globulars) = self.held.get(&key) {
            return Ok(globulars);
        }
        let (pool, galaxies, held) = (
            Arc::clone(&self.pool),
            Arc::clone(&self.galaxies),
            Arc::clone(&self.held),
        );
        self.flights
            .run(key, move || async move {
                // As the density maps' service: a caller can arrive just after another flight has
                // finished, and the list it found is worth more than a second walk.
                if let Some(globulars) = held.get(&key) {
                    return Ok(globulars);
                }
                let galaxy = galaxies.get(key).await?;
                let token = CancelToken::new();
                // Cancels the queued job once every waiter on this flight has gone.
                let _cancel_on_drop = CancelOnDrop::new(token.clone());
                let globulars = Arc::new(
                    bulk(&pool, &token, move |_: &CancelToken| {
                        MapGlobulars::of(&galaxy)
                    })
                    .await?,
                );
                // A list larger than the whole budget is handed back and not stored.
                drop(held.insert(key, Arc::clone(&globulars)));
                Ok(globulars)
            })
            .await
    }

    /// The lists held, the bytes they are charged, and the hits, misses, evictions and refusals
    /// so far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.held.counters()
    }
}

impl std::fmt::Debug for GalaxyFeaturesService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GalaxyFeaturesService")
            .field("counters", &self.counters())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::galaxy::ages::SubDisc;
    use hyperion_sim::galaxy::catalogue_classes::NoClassCellCache;
    use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
    use hyperion_sim::galaxy::features::members::NoInteriorCache;
    use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
    use hyperion_sim::galaxy::placement::NoCache;
    use hyperion_sim::galaxy::query::{RangeQuery, RangeResult, range_query};
    use hyperion_sim::id::SystemIdKind;
    use hyperion_sim::{GENERATOR_VERSION, Seed};

    use super::super::range::beside_the_grid;
    use super::*;
    use crate::FeatureMembers;
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

    /// A cell asked for through a process is the catalogue's through it and is held; asked for
    /// through no more processes, the held cell is lent; asked for whole, it is made again whole
    /// and replaces the part, which is then never lent again (P09.T40.a).
    #[test]
    fn a_cell_held_in_part_is_lent_for_less_and_made_again_for_more() {
        let cache = SharedFeatureCellCache::new(64 << 20);
        let handle = cache.handle(key());
        let cell = a_cluster().id().cell();
        let part = handle.contents_through(galaxy(), cell, FeatureProcess::Nursery);
        assert_eq!(
            *part,
            FeatureCatalogue::cell_through(galaxy(), cell, FeatureProcess::Nursery)
        );
        assert!(!part.holds(FeatureProcess::Cloud));
        let fewer = handle.contents_through(
            galaxy(),
            cell,
            FeatureProcess::OldOpenCluster(SubDisc::First),
        );
        assert!(Arc::ptr_eq(&fewer, &part), "the held part is lent for less");
        let whole = FeatureCellCache::contents(&handle, galaxy(), cell);
        assert_eq!(*whole, FeatureCatalogue::cell(galaxy(), cell));
        assert_eq!(
            cache.counters().entries(),
            1,
            "the whole cell replaced the part"
        );
        let again = handle.contents_through(galaxy(), cell, FeatureProcess::Nursery);
        assert!(
            Arc::ptr_eq(&again, &whole),
            "the whole cell is lent for a part"
        );
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
            expected.heap_bytes()
                + size_of::<FeatureInterior>()
                + size_of::<HeldFeature>()
                + ENTRY_OVERHEAD_BYTES
        );
        // A bare galaxy has no members: its features have no interior, and nothing is stored.
        let bare = hyperion_sim::galaxy::Galaxy::new(Seed::new(SEED));
        let empty = SharedClusterModelCache::new(64 << 20);
        assert!(empty.handle(key()).interior(&bare, feature).is_none());
        assert_eq!(empty.counters().entries(), 0);
    }

    /// A feature is held by the bound on its grid's reach until its interior is built, which
    /// replaces it and is lent its grid's own reach; a feature with no members is held by its
    /// `None`, and has no interior (P09.T40.a).
    #[test]
    fn a_feature_is_held_by_its_bound_until_its_interior_is_built() {
        let cache = SharedClusterModelCache::new(64 << 20);
        let handle = cache.handle(key());
        let feature = a_cluster();
        let bound = FeatureInterior::grid_reach_bound(galaxy(), feature);
        assert!(bound.is_some(), "the cluster has members");
        assert_eq!(handle.grid_reach(galaxy(), feature), bound);
        assert_eq!(handle.grid_reach(galaxy(), feature), bound, "the cache's");
        let counters = cache.counters();
        assert_eq!(
            (counters.misses(), counters.hits(), counters.entries()),
            (1, 1, 1)
        );
        assert_eq!(
            counters.bytes(),
            size_of::<HeldFeature>() + ENTRY_OVERHEAD_BYTES
        );
        let interior = handle
            .interior(galaxy(), feature)
            .expect("the cluster has members");
        assert_eq!(
            cache.counters().entries(),
            1,
            "the interior replaces the bound"
        );
        let reach = handle.grid_reach(galaxy(), feature).expect("an interior");
        assert_eq!(reach, interior.grid().reach());
        assert!(reach.value() <= bound.expect("a bound").value());
        // A feature with no members (a cloud, an association): its `None` is held, and it has no
        // interior.
        let memberless =
            FeatureCatalogue::near(galaxy(), &sun(), LightYears::new(3_000.0), &NoFeatureCache)
                .find(|feature| FeatureInterior::of(galaxy(), feature).is_none())
                .expect("a feature with no members lies within 3,000 ly of the Sun");
        let kept = cache.kept(key());
        assert_eq!(kept.grid_reach(galaxy(), &memberless), None);
        assert!(handle.interior(galaxy(), &memberless).is_none());
        assert_eq!(cache.counters().entries(), 2);
    }

    /// The features' members as the range query finds them about `feature`, through `cells` and
    /// `interiors`, beside the sources a served query takes with them (from 27, the classes').
    fn members_about(
        feature: &FeatureRecord,
        cells: &dyn FeatureCellCache,
        interiors: &dyn FeatureInteriorCache,
    ) -> RangeResult {
        let source = FeatureMemberSource::new(cells, interiors);
        let query = RangeQuery::builder(*feature.position(), LightYears::new(2.0))
            .build()
            .expect("a query");
        beside_the_grid(
            galaxy(),
            FeatureMembers::Served,
            (&source, &NoClassCellCache),
            None,
            |sources, _| range_query(galaxy(), &mut NoCache::new(), sources, &query),
        )
    }

    /// The census, bit for bit, and the hits of queries about a cluster do not depend on the order
    /// they are asked in through one shared cache, which tests a feature by its held bound or its
    /// interior's grid (P09.T40.a).
    #[test]
    fn a_range_query_s_members_do_not_depend_on_the_order_of_queries() {
        let feature = a_cluster();
        let [x, y, z] = feature.position().to_light_years_f64();
        let queries = [(0.0, 2.0), (6.0, 4.0), (-40.0, 30.0)].map(|(dx, radius)| {
            let centre = GalacticPosition::from_light_years([x + dx, y, z]).expect("in the cube");
            RangeQuery::builder(centre, LightYears::new(radius))
                .build()
                .expect("a query")
        });
        let cells = SharedFeatureCellCache::new(64 << 20);
        let interiors = SharedClusterModelCache::new(64 << 20);
        hyperion_testkit::order::assert_order_independent(&queries, |query| {
            let (cells, interiors) = (cells.handle(key()), interiors.handle(key()));
            let source = FeatureMemberSource::new(&cells, &interiors);
            let classes = NoClassCellCache;
            let result = beside_the_grid(
                galaxy(),
                FeatureMembers::Served,
                (&source, &classes),
                None,
                |sources, _| range_query(galaxy(), &mut NoCache::new(), sources, query),
            );
            let census: Vec<u64> = result
                .census()
                .expected()
                .to_array()
                .into_iter()
                .map(hyperion_testkit::float::bits)
                .collect();
            (census, result.systems().to_vec())
        });
    }

    /// The features a lookup finds do not depend on the parts of cells one shared cache holds from
    /// the lookups before it: at radii and centres that read the cells about the Sun-like point
    /// through the clusters only, through the nurseries and whole, so that cells held in part are
    /// lent for less and made again for more, and each is what it is uncached (P09.T40.a).
    #[test]
    fn a_lookup_s_features_do_not_depend_on_the_parts_of_cells_held() {
        let lookups = [
            ([0.0, 26_000.0, 0.0], 2.0),
            ([0.0, 26_000.0, 0.0], 400.0),
            ([2_000.0, 26_000.0, 300.0], 50.0),
            ([0.0, 26_000.0, 0.0], 1_500.0),
        ]
        .map(|(centre, radius)| {
            let centre = GalacticPosition::from_light_years(centre).expect("in the cube");
            (centre, LightYears::new(radius))
        });
        let near = |(centre, radius): &(GalacticPosition, LightYears),
                    cells: &dyn FeatureCellCache| {
            FeatureCatalogue::near(galaxy(), centre, *radius, cells).collect::<Vec<_>>()
        };
        let cells = SharedFeatureCellCache::new(64 << 20);
        hyperion_testkit::order::assert_order_independent(&lookups, |lookup| {
            let found = near(lookup, &cells.handle(key()));
            assert_eq!(found, near(lookup, &NoFeatureCache), "{lookup:?}");
            found
        });
    }

    /// Eviction never changes a reply: a range query about a cluster with its members' source
    /// finds the same hits and census through the caches, cold and warm, as with none, whether
    /// they keep nothing, evict all the time (a few kilobytes of cells, a few interiors of the
    /// hundred or so the query builds) or keep everything.
    ///
    /// Each cold query builds the interior of every feature whose grid may meet its padded sphere
    /// by the bound on its reach, at some 90 ms each in a test build, so the budgets come in three
    /// pairs rather than all their combinations.
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
