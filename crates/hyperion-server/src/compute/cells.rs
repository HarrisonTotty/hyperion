//! The cache of generated cells: whole cells of accepted systems, bounded in bytes.
//!
//! A range query visits thousands of generation cells, and two queries near one another visit mostly
//! the same ones, so the cells are what the server keeps (brainstorm, "Runtime and code shape"). The
//! bound is in bytes rather than entries, because a bulge cell holds ten thousand times what a rim
//! cell does: [`SharedCellCache`] is a [`SharedByteLru`] over
//! `(GalaxyKey, CellKey)`, so two saves of one seed share their cells and nothing generated under one
//! generator version is lent for another (plan 04, design note 23).
//!
//! An entry is a cell's accepted systems as state at the epoch, never positions at some time: those
//! belong to the query that asked for a time. A query therefore reuses the cells of a query at
//! another time, and eviction is always safe, since nothing generated is remembered anywhere else.
//!
//! [`SharedCellCache::handle`] gives one query its [`CellCacheHandle`], which is what plan 03's
//! [`CellCache`] wants: a `&mut` value for the length of one query, over a cache many queries share.
//! Two threads may generate one cell at the same time; the second insert replaces an equal value,
//! which is cheaper than coordinating (the same ruling as [`SharedByteLru`]'s).

use std::mem;
use std::sync::Arc;

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{
    CellCache, CellKey, SystemRecord, cell_heap_bytes, generate_cell,
};

use super::GalaxyKey;
use crate::cache::{HeapBytes, Insertion, LruCounters, SharedByteLru};

/// What names a cell in the cache: the galaxy it belongs to, and the cell itself.
type CellEntryKey = (GalaxyKey, CellKey);

/// One generation cell's accepted systems, as the cache holds them.
///
/// The systems are in candidate-index order, exactly as
/// [`generate_cell`] produces them, and describe the systems at the epoch.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedCell(Vec<SystemRecord>);

impl CachedCell {
    /// Takes `systems` as a cell's contents, releasing whatever spare capacity they arrived with.
    ///
    /// [`generate_cell`] reserves for every candidate of the cell and keeps only those it accepts,
    /// and [`cell_heap_bytes`] charges the records and not the buffer's spare capacity, so the
    /// capacity is given back rather than held and charged to nobody.
    #[must_use]
    fn new(mut systems: Vec<SystemRecord>) -> Self {
        systems.shrink_to_fit();
        Self(systems)
    }

    /// The cell's systems, in candidate-index order.
    #[must_use]
    pub fn systems(&self) -> &[SystemRecord] {
        &self.0
    }

    /// The systems, to be generated into again.
    #[must_use]
    fn into_systems(self) -> Vec<SystemRecord> {
        self.0
    }
}

impl HeapBytes for CachedCell {
    fn heap_bytes(&self) -> usize {
        cell_heap_bytes(&self.0)
    }
}

/// The generated cells of every galaxy the server holds, in one byte budget.
#[derive(Debug)]
pub struct SharedCellCache {
    cells: SharedByteLru<CellEntryKey, CachedCell>,
}

impl SharedCellCache {
    /// An empty cache that holds at most `budget_bytes` of charged cells.
    ///
    /// A budget of zero caches nothing: every cell is generated, lent and dropped.
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            cells: SharedByteLru::new(budget_bytes),
        }
    }

    /// A handle for one query over the galaxy `galaxy` names.
    ///
    /// The handle is the `&mut` value plan 03's [`CellCache`] takes, and lives for one query: it
    /// holds the buffer that cells are generated into, and the galaxy key that every cell it stores
    /// is keyed by. A query on a pool job makes its own from the state it owns.
    #[must_use]
    pub fn handle(&self, galaxy: GalaxyKey) -> CellCacheHandle<'_> {
        CellCacheHandle {
            cells: &self.cells,
            galaxy,
            scratch: Vec::new(),
        }
    }

    /// The cells held, the bytes they are charged, and the hits, misses, evictions and refusals so
    /// far.
    #[must_use]
    pub fn counters(&self) -> LruCounters {
        self.cells.counters()
    }
}

/// One query's view of the cell cache: plan 03's [`CellCache`], over the cache queries share.
///
/// Dropping it leaves the cells it generated in the cache and gives up its buffer.
#[derive(Debug)]
pub struct CellCacheHandle<'a> {
    cells: &'a SharedByteLru<CellEntryKey, CachedCell>,
    galaxy: GalaxyKey,
    /// The buffer a missing cell is generated into. It is moved into the cache with the cell, and a
    /// cell the cache refuses hands it back.
    scratch: Vec<SystemRecord>,
}

impl CellCacheHandle<'_> {
    /// Which galaxy's cells this handle lends.
    #[must_use]
    pub fn galaxy(&self) -> GalaxyKey {
        self.galaxy
    }
}

impl CellCache for CellCacheHandle<'_> {
    /// Lends the systems of `key`, generating the cell if the cache has not got it.
    ///
    /// `f` runs with no lock held, on a cell this handle holds an `Arc` of, so the cache may evict
    /// anything meanwhile without touching what `f` sees.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key, since its cells would then be lent
    /// under another galaxy's key. The key's other half, the generator version, cannot be checked
    /// against a [`Galaxy`], which carries none; what guarantees it is that only
    /// [`GalaxyCache`](super::GalaxyCache) builds galaxies, and only for a supported version. The
    /// server builds a handle per query from the query's own universe, so either mismatch is a bug.
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        assert_eq!(
            galaxy.seed().get(),
            self.galaxy.seed(),
            "a cell cache handle lends the cells of one galaxy, and this is another's"
        );
        let entry_key = (self.galaxy, key);
        if let Some(cell) = self.cells.get(&entry_key) {
            return f(cell.systems());
        }
        let mut systems = mem::take(&mut self.scratch);
        generate_cell(galaxy, key, &mut systems);
        let cell = Arc::new(CachedCell::new(systems));
        let result = f(cell.systems());
        // Inserted after the closure has run, so that nothing waits on the lock while the query
        // reads the cell. A cell larger than the whole budget is handed back, and its buffer serves
        // the next cell rather than being freed and allocated again.
        if let Insertion::Refused(refused) = self.cells.insert(entry_key, cell)
            && let Some(refused) = Arc::into_inner(refused)
        {
            self.scratch = refused.into_systems();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::OnceLock;

    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::galaxy::placement::NoCache;
    use hyperion_sim::galaxy::query::{QuerySphere, RangeQuery, cells_in_sphere, range_query};
    use hyperion_sim::id::Layer;
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::units::LightYears;
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::cache::ENTRY_OVERHEAD_BYTES;

    /// The seed of the galaxy these tests walk. Its cells are those the server would generate.
    const SEED: u64 = 0x4d2;

    /// A budget that holds every cell any of these walks generates.
    const ROOMY: usize = 8 << 20;

    /// A count of cells as the cache counts its lookups.
    fn count(cells: usize) -> u64 {
        u64::try_from(cells).expect("a walk of a few hundred cells")
    }

    /// One real galaxy, built once for the whole test binary.
    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
    }

    fn key() -> GalaxyKey {
        GalaxyKey::new(SEED, GENERATOR_VERSION)
    }

    /// A sphere of `radius_ly` about a point in the plane at 26,000 ly, at `time`.
    fn sphere(centre_ly: [f64; 3], radius_ly: f64, time: UniverseTime) -> QuerySphere {
        let centre = GalacticPosition::from_light_years(centre_ly).expect("inside the root cube");
        // A pad for the systems' motion between the epoch and `time`: 1,000 km/s over 500 years is
        // 1.7 ly, and a round 2 ly covers it.
        let pad = if time == UniverseTime::EPOCH {
            LightYears::ZERO
        } else {
            LightYears::new(2.0)
        };
        QuerySphere::new(centre, LightYears::new(radius_ly), time, pad).expect("a valid sphere")
    }

    /// The cells of `layer` that meet `sphere`.
    fn walk(layer: Layer, sphere: &QuerySphere) -> Vec<CellKey> {
        cells_in_sphere(layer, sphere).collect()
    }

    /// Every cell's systems, as `cache` lends them, keyed by cell.
    fn lend(cache: &mut impl CellCache, cells: &[CellKey]) -> BTreeMap<CellKey, Vec<SystemRecord>> {
        cells
            .iter()
            .map(|&key| {
                (
                    key,
                    cache.with_cell(galaxy(), key, <[SystemRecord]>::to_vec),
                )
            })
            .collect()
    }

    /// What plan 03's cacheless implementation lends for the same cells.
    fn uncached(cells: &[CellKey]) -> BTreeMap<CellKey, Vec<SystemRecord>> {
        lend(&mut NoCache::new(), cells)
    }

    /// A walk that reaches cells with systems in them: layer C, 32 ly cells at the solar circle.
    fn disc_cells() -> Vec<CellKey> {
        let cells = walk(
            Layer::C,
            &sphere([0.0, 26_000.0, 0.0], 60.0, UniverseTime::EPOCH),
        );
        assert!(cells.len() > 8, "{} cells", cells.len());
        cells
    }

    #[test]
    fn a_walk_through_the_handle_lends_what_no_cache_lends_cold_and_warm() {
        let cells = disc_cells();
        let expected = uncached(&cells);
        let total: usize = expected.values().map(Vec::len).sum();
        assert!(total > 0, "the walk reaches systems");

        let cache = SharedCellCache::new(ROOMY);
        let mut handle = cache.handle(key());
        assert_eq!(lend(&mut handle, &cells), expected, "cold");
        let counters = cache.counters();
        assert_eq!(
            (counters.hits(), counters.misses(), counters.refused()),
            (0, count(cells.len()), 0)
        );
        assert_eq!(counters.entries(), cells.len());

        // Warm: the same handle, then a fresh one, both from the cache.
        assert_eq!(lend(&mut handle, &cells), expected, "warm");
        drop(handle);
        let mut second = cache.handle(key());
        assert_eq!(lend(&mut second, &cells), expected, "another query");
        let counters = cache.counters();
        assert_eq!(counters.hits(), 2 * count(cells.len()));
        assert_eq!(counters.misses(), count(cells.len()));
        // The entries are charged the records, plus the value and the per-entry overhead.
        let records = total * size_of::<SystemRecord>();
        let overhead = cells.len() * (size_of::<CachedCell>() + ENTRY_OVERHEAD_BYTES);
        assert_eq!(counters.bytes(), records + overhead);
    }

    /// A whole range query, census included, through the handle equals the same query through
    /// plan 03's `NoCache`, with a roomy budget cold and warm and with a budget of 1 KiB, which
    /// evicts on almost every cell (P04.T12; `range_query` has been in the tree since T14.d).
    #[test]
    fn a_range_query_through_the_handle_answers_as_no_cache_does() {
        let centre = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
        let query = RangeQuery::builder(centre, LightYears::new(60.0))
            .build()
            .expect("a valid query");
        let uncached = range_query(galaxy(), &mut NoCache::new(), &[], &query);
        assert!(
            !uncached.systems().is_empty(),
            "a 60 ly sphere at the Sun is not empty"
        );
        for budget in [ROOMY, 1 << 10] {
            let cache = SharedCellCache::new(budget);
            for pass in ["cold", "warm"] {
                let cached = range_query(galaxy(), &mut cache.handle(key()), &[], &query);
                assert_eq!(cached, uncached, "{pass}, with a budget of {budget} bytes");
            }
            assert!(cache.counters().misses() > 0);
        }
    }

    #[test]
    fn a_budget_of_one_kibibyte_evicts_constantly_and_still_lends_the_same_systems() {
        let cells = disc_cells();
        let expected = uncached(&cells);
        let cache = SharedCellCache::new(1024);
        let mut handle = cache.handle(key());
        assert_eq!(lend(&mut handle, &cells), expected, "cold");
        assert_eq!(lend(&mut handle, &cells), expected, "and again");
        let counters = cache.counters();
        assert!(counters.bytes() <= 1024, "{} bytes held", counters.bytes());
        assert!(
            counters.evictions() > 0 || counters.refused() > 0,
            "a 1 KiB budget cannot hold these cells: {counters:?}"
        );
    }

    #[test]
    fn a_cell_larger_than_the_whole_budget_is_lent_and_not_stored() {
        let cells = disc_cells();
        let expected = uncached(&cells);
        // Smaller than one record, so every cell with a system in it is refused.
        let cache = SharedCellCache::new(size_of::<SystemRecord>());
        let mut handle = cache.handle(key());
        assert_eq!(lend(&mut handle, &cells), expected);
        let counters = cache.counters();
        assert!(counters.refused() > 0, "{counters:?}");
        assert_eq!(counters.hits(), 0, "nothing large enough was ever stored");
        // A refused cell hands its buffer back, so a walk of refused cells allocates once.
        assert!(
            handle.scratch.capacity() > 0,
            "the handle kept no buffer to generate the next cell into"
        );
    }

    #[test]
    fn queries_at_two_times_share_the_cells_between_them() {
        let epoch = walk(
            Layer::D,
            &sphere([0.0, 26_000.0, 0.0], 80.0, UniverseTime::EPOCH),
        );
        let later = walk(
            Layer::D,
            &sphere(
                [0.0, 26_000.0, 0.0],
                80.0,
                UniverseTime::from_julian_years(500).expect("inside the clock window"),
            ),
        );
        let shared: BTreeSet<CellKey> = epoch
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .intersection(&later.iter().copied().collect())
            .copied()
            .collect();
        assert!(!shared.is_empty(), "the two walks overlap");

        let cache = SharedCellCache::new(ROOMY);
        let at_epoch = lend(&mut cache.handle(key()), &epoch);
        let at_later = lend(&mut cache.handle(key()), &later);
        let counters = cache.counters();
        assert_eq!(
            counters.hits(),
            count(shared.len()),
            "the later query generated only the cells the first had not"
        );
        // Entries are the systems at the epoch, so the shared cells are lent unchanged.
        for cell in &shared {
            assert_eq!(at_later[cell], at_epoch[cell], "{cell:?}");
        }
    }

    #[test]
    fn two_walks_lend_the_same_systems_in_either_order() {
        let first = disc_cells();
        let second = walk(
            Layer::E,
            &sphere([1_000.0, 25_000.0, 30.0], 200.0, UniverseTime::EPOCH),
        );
        let forwards = SharedCellCache::new(ROOMY);
        let a_then_b = (
            lend(&mut forwards.handle(key()), &first),
            lend(&mut forwards.handle(key()), &second),
        );
        let backwards = SharedCellCache::new(ROOMY);
        let b_then_a = (
            lend(&mut backwards.handle(key()), &second),
            lend(&mut backwards.handle(key()), &first),
        );
        assert_eq!(a_then_b.0, b_then_a.1);
        assert_eq!(a_then_b.1, b_then_a.0);
        // And both agree with no cache at all.
        assert_eq!(a_then_b.0, uncached(&first));
        assert_eq!(a_then_b.1, uncached(&second));
    }

    /// Lends `cell` through a handle of `cache`, as one call of an order-independence pass.
    fn lend_one(cache: &SharedCellCache, cell: CellKey) -> Vec<SystemRecord> {
        cache
            .handle(key())
            .with_cell(galaxy(), cell, <[SystemRecord]>::to_vec)
    }

    #[test]
    fn a_cell_is_the_same_whatever_was_asked_before_it() {
        let cells = disc_cells();
        // The testkit's check runs every cell forwards, backwards, in a fixed permutation and alone,
        // which is what the determinism rules ask of anything sitting behind a cache. Three caches:
        // one kept warm across the passes, one built fresh for every single call, and one whose
        // budget evicts under the walk.
        let warm = SharedCellCache::new(ROOMY);
        assert_order_independent(&cells, |&cell| lend_one(&warm, cell));
        assert_order_independent(&cells, |&cell| lend_one(&SharedCellCache::new(ROOMY), cell));
        let tight = SharedCellCache::new(1024);
        assert_order_independent(&cells, |&cell| lend_one(&tight, cell));
    }

    #[test]
    fn threads_walking_one_cache_at_once_lend_the_same_systems() {
        let cells = disc_cells();
        let expected = uncached(&cells);
        // Roomy, so the walks race on hits and inserts, and then a budget below one cell, so they
        // race on eviction and refusal too: a lent cell is an `Arc` of its own, so whatever the
        // others evict meanwhile cannot change what a walk reads.
        for budget in [ROOMY, 1024, size_of::<SystemRecord>()] {
            let cache = SharedCellCache::new(budget);
            std::thread::scope(|scope| {
                for _ in 0..4 {
                    scope.spawn(|| {
                        let mut handle = cache.handle(key());
                        assert_eq!(lend(&mut handle, &cells), expected, "budget {budget}");
                    });
                }
            });
            let counters = cache.counters();
            assert!(
                counters.bytes() <= budget,
                "{} bytes held of {budget}",
                counters.bytes()
            );
            assert_eq!(
                counters.hits() + counters.misses(),
                4 * count(cells.len()),
                "every lookup is counted once"
            );
        }
    }

    #[test]
    fn cells_of_one_galaxy_are_never_lent_for_another() {
        let cells = disc_cells();
        let expected = uncached(&cells);
        let cache = SharedCellCache::new(ROOMY);
        assert_eq!(lend(&mut cache.handle(key()), &cells), expected);
        // Another universe of another seed shares the cache and none of its entries.
        let other = GalaxyKey::new(SEED + 1, GENERATOR_VERSION);
        let mut handle = cache.handle(other);
        assert_eq!(handle.galaxy(), other);
        let other_galaxy = Galaxy::new(Seed::new(SEED + 1));
        let lent: Vec<_> = cells
            .iter()
            .map(|&key| handle.with_cell(&other_galaxy, key, <[SystemRecord]>::to_vec))
            .collect();
        let mut no_cache = NoCache::new();
        for (cell, lent) in cells.iter().zip(&lent) {
            let expected = no_cache.with_cell(&other_galaxy, *cell, <[SystemRecord]>::to_vec);
            assert_eq!(*lent, expected, "{cell:?}");
        }
        let counters = cache.counters();
        assert_eq!(
            counters.hits(),
            0,
            "no entry of one galaxy served the other"
        );
        assert_eq!(counters.entries(), 2 * cells.len());
    }

    #[test]
    #[should_panic(expected = "lends the cells of one galaxy")]
    fn a_handle_refuses_a_galaxy_that_is_not_its_own() {
        let cache = SharedCellCache::new(ROOMY);
        let mut handle = cache.handle(GalaxyKey::new(SEED + 1, GENERATOR_VERSION));
        let cell = disc_cells()[0];
        handle.with_cell(galaxy(), cell, <[SystemRecord]>::len);
    }

    #[test]
    fn a_handle_gives_its_buffer_to_the_cell_it_stores() {
        let cells = disc_cells();
        let cache = SharedCellCache::new(ROOMY);
        let mut handle = cache.handle(key());
        for &cell in &cells {
            handle.with_cell(galaxy(), cell, <[SystemRecord]>::len);
        }
        // Every cell was stored, so the buffer went with the last of them.
        assert_eq!(handle.scratch.capacity(), 0);
        let stored = cache
            .cells
            .get(&(key(), cells[0]))
            .expect("the first cell is held");
        // The stored records carry no spare capacity, which is what `cell_heap_bytes` assumes.
        assert_eq!(stored.0.capacity(), stored.0.len());
        assert_eq!(stored.heap_bytes(), cell_heap_bytes(stored.systems()));
    }
}
