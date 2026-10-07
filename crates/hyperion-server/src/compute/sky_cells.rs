//! The sky's per-cell cache: each census cell's bright subset, the systems at or above a mass
//! floor, bounded in bytes (rendering plan R06, R06.T11.b; Design note 12).
//!
//! The census asks each cell for its records whose primary's initial mass is at least the cell's
//! floor ([`SkyCellCache::bright_subset`]), and near the Sun that is most of the census's cost
//! after the systems' own generation. Records are epoch state, so an entry outlives the request
//! that built it: a sky asked again, or after a jump of up to 1,000 ly, finds most of its cells
//! built. [`SharedSkyCellCache`] is a [`SharedByteLru`] over `(GalaxyKey, CellKey)`, so that two
//! saves of one seed share their cells and nothing generated under one generator version is lent
//! for another (plan 04, design note 23), under its own budget, `HYPERION_SKY_CACHE_MB`. It is not
//! the range queries' [`SharedCellCache`](super::SharedCellCache), which holds whole cells where
//! the sky keeps only the bright subset above each floor.
//!
//! The cache is **monotone**, as the sim requires of every [`SkyCellCache`]: an entry holds a
//! cell's records at or above the floor it was built at, in candidate order, and serves a later
//! floor only if that floor is at or above it, through the sim's [`serve_from_entry`]; a lower
//! floor rebuilds the cell and replaces the entry with the one that serves more. So the cache never
//! changes a reply. Two census jobs never share a cell, but two requests may build one cell at
//! once; the second insert replaces the first, and either entry serves by the same rule (the same
//! ruling as [`SharedByteLru`]'s).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, cell_heap_bytes};
use hyperion_sim::sky::census::{NoSkyCellCache, Served, SkyCellCache, serve_from_entry};
use hyperion_sim::units::SolarMasses;

use super::GalaxyKey;
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// What names a cell in the cache: the galaxy it belongs to, and the cell itself.
type SkyCellKey = (GalaxyKey, CellKey);

/// One cell's bright subset as the cache holds it: the floor it was built at, and the cell's
/// records whose primary's initial mass is at or above it, in candidate order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SkyCellEntry {
    floor: SolarMasses,
    records: Vec<SystemRecord>,
}

impl HeapBytes for SkyCellEntry {
    /// The records' own bytes ([`cell_heap_bytes`]); they are kept at their length, with no spare
    /// capacity.
    fn heap_bytes(&self) -> usize {
        cell_heap_bytes(&self.records)
    }
}

/// The sky cache's counters: its size and use, and the entries it found but could not serve.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SkyCellCounters {
    cache: LruCounters,
    rebuilt: u64,
}

impl SkyCellCounters {
    /// The cells held, the bytes they are charged, the budget, and the hits, misses, evictions and
    /// refusals so far.
    ///
    /// A hit is a lookup that found its cell's entry, whether or not the entry could serve it
    /// ([`SkyCellCounters::rebuilt`]).
    #[must_use]
    pub fn cache(&self) -> LruCounters {
        self.cache
    }

    /// Hits whose entry was built at a floor above the one asked, so that the cell was rebuilt at
    /// the lower floor (Design note 12).
    ///
    /// The lookups served from the cache are the hits less these.
    #[must_use]
    pub fn rebuilt(&self) -> u64 {
        self.rebuilt
    }
}

/// The bright subsets of the census cells of every galaxy the server holds, in one byte budget.
#[derive(Debug)]
pub(crate) struct SharedSkyCellCache {
    cells: SharedByteLru<SkyCellKey, SkyCellEntry>,
    rebuilt: AtomicU64,
}

impl SharedSkyCellCache {
    /// An empty cache that holds at most `budget_bytes` of charged entries.
    ///
    /// A budget of zero caches nothing: every cell is generated, served and dropped.
    #[must_use]
    pub(crate) fn new(budget_bytes: usize) -> Self {
        Self {
            cells: SharedByteLru::new(budget_bytes),
            rebuilt: AtomicU64::new(0),
        }
    }

    /// The cache as the census over the galaxy `galaxy` names reads it: the sim's
    /// [`SkyCellCache`], for one census job's
    /// [`SkyContext`](hyperion_sim::sky::census::SkyContext).
    #[must_use]
    pub(crate) fn handle(&self, galaxy: GalaxyKey) -> SkyCellCacheHandle<'_> {
        SkyCellCacheHandle {
            cache: self,
            galaxy,
        }
    }

    /// The cells held, the bytes they are charged, and the hits, misses, rebuilds, evictions and
    /// refusals so far.
    #[must_use]
    pub(crate) fn counters(&self) -> SkyCellCounters {
        SkyCellCounters {
            cache: self.cells.counters(),
            rebuilt: self.rebuilt.load(Ordering::Relaxed),
        }
    }
}

/// One galaxy's view of the sky's cell cache: the sim's [`SkyCellCache`], over the cache every
/// request shares.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SkyCellCacheHandle<'a> {
    cache: &'a SharedSkyCellCache,
    galaxy: GalaxyKey,
}

impl SkyCellCache for SkyCellCacheHandle<'_> {
    /// Writes the records of `key` at or above `floor` to `out`: from the cell's entry if it was
    /// built at or below `floor`, or generated and kept at `floor` otherwise.
    ///
    /// The cell is generated with no lock held, and kept afterwards. A cell asked at a NaN floor
    /// holds no record and would serve nothing, so it is not kept.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key, since its cells would then be kept
    /// under another galaxy's key. The server builds a handle per census job from the request's own
    /// universe, so a mismatch is a bug.
    fn bright_subset(
        &self,
        galaxy: &Galaxy,
        key: CellKey,
        floor: SolarMasses,
        out: &mut Vec<SystemRecord>,
    ) {
        assert_eq!(
            galaxy.seed().get(),
            self.galaxy.seed(),
            "a sky cell cache handle lends the cells of one galaxy, and this is another's"
        );
        let entry_key = (self.galaxy, key);
        if let Some(entry) = self.cache.cells.get(&entry_key) {
            match serve_from_entry(entry.floor, &entry.records, floor, out) {
                Served::Served => return,
                Served::Rebuild => {
                    self.cache.rebuilt.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        NoSkyCellCache.bright_subset(galaxy, key, floor, out);
        if floor.value().is_nan() {
            return;
        }
        // A clone holds exactly the records, with no spare capacity to be charged to nobody. An
        // entry larger than the whole budget is refused, and the cell is still served.
        let entry = SkyCellEntry {
            floor,
            records: out.clone(),
        };
        let _ = self.cache.cells.insert(entry_key, Arc::new(entry));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_sim::galaxy::placement::{cell_heap_bytes, generate_cell};
    use hyperion_sim::id::Layer;
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::cache::ENTRY_OVERHEAD_BYTES;

    /// The seed of the galaxy these tests read cells of.
    const SEED: u64 = 0x4d2;

    /// One real galaxy, built once for the whole test binary.
    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
    }

    fn key() -> GalaxyKey {
        GalaxyKey::new(SEED, GENERATOR_VERSION)
    }

    /// A layer-E cell near the Sun's place, which holds systems over a wide range of masses.
    fn cell() -> CellKey {
        CellKey::new(Layer::E, [0, 203, 0]).expect("a cell in the root cube")
    }

    /// A cell of each stellar layer about the Sun's place.
    fn sun_cells() -> [CellKey; 5] {
        [
            (Layer::A, [0, 3_250, 8]),
            (Layer::B, [0, 1_625, 4]),
            (Layer::C, [0, 812, 2]),
            (Layer::D, [0, 406, 1]),
            (Layer::E, [0, 203, 0]),
        ]
        .map(|(layer, index)| CellKey::new(layer, index).expect("a cell in the root cube"))
    }

    /// What no cache serves for `key` at `floor`.
    fn uncached(key: CellKey, floor: f64) -> Vec<SystemRecord> {
        let mut out = Vec::new();
        NoSkyCellCache.bright_subset(galaxy(), key, SolarMasses::new(floor), &mut out);
        out
    }

    /// The masses of `key`'s records, in increasing order.
    fn masses_of(key: CellKey) -> Vec<f64> {
        let mut records = Vec::new();
        generate_cell(galaxy(), key, &mut records);
        let mut masses: Vec<f64> = records
            .iter()
            .map(|record| record.primary_initial_mass().value())
            .collect();
        masses.sort_by(f64::total_cmp);
        masses
    }

    /// The masses at which `key`'s records split: a floor every record passes, then two at records'
    /// own masses, the second at or above the first, or 1 and 2 M☉ in a cell that holds none.
    fn floors_of(key: CellKey) -> [f64; 3] {
        let masses = masses_of(key);
        match (masses.get(masses.len() / 4), masses.get(masses.len() / 2)) {
            (Some(&low), Some(&high)) => [0.0, low, high],
            _ => [0.0, 1.0, 2.0],
        }
    }

    /// [`floors_of`] the E cell, whose floors each drop some of its records.
    fn floors() -> [f64; 3] {
        let held = masses_of(cell()).len();
        assert!(held >= 4, "{held} records");
        floors_of(cell())
    }

    /// Whatever the order of floors asked, the cache serves what no cache serves (Design note 12),
    /// rebuilding only for a floor below its entry's, which it then keeps.
    #[test]
    fn the_cache_never_changes_a_cells_bright_subset() {
        let [all, low, high] = floors();
        let cache = SharedSkyCellCache::new(64 << 20);
        let handle = cache.handle(key());
        let mut out = Vec::new();
        // (floor, then the counters' hits, misses and rebuilds so far)
        let asked = [
            (low, (0, 1, 0)),  // built at `low`
            (high, (1, 1, 0)), // served from it, filtered
            (low, (2, 1, 0)),  // served whole
            (all, (3, 1, 1)),  // below the entry: rebuilt at `all`
            (high, (4, 1, 1)), // served from the rebuilt entry
            (all, (5, 1, 1)),
        ];
        for (floor, (hits, misses, rebuilt)) in asked {
            handle.bright_subset(galaxy(), cell(), SolarMasses::new(floor), &mut out);
            assert_eq!(out, uncached(cell(), floor), "at {floor} M☉");
            let counters = cache.counters();
            assert_eq!(
                (
                    counters.cache().hits(),
                    counters.cache().misses(),
                    counters.rebuilt()
                ),
                (hits, misses, rebuilt),
                "after {floor} M☉"
            );
        }
        // One entry, at the lowest floor asked, charged its records.
        let counters = cache.counters().cache();
        let records = uncached(cell(), all);
        assert_eq!(counters.entries(), 1);
        assert_eq!(
            counters.bytes(),
            cell_heap_bytes(&records) + size_of::<SkyCellEntry>() + ENTRY_OVERHEAD_BYTES
        );
    }

    /// An entry is the galaxy's own: another seed's census misses it.
    #[test]
    fn a_cell_is_kept_per_galaxy() {
        let cache = SharedSkyCellCache::new(64 << 20);
        let mut out = Vec::new();
        let floor = SolarMasses::new(0.5);
        cache
            .handle(key())
            .bright_subset(galaxy(), cell(), floor, &mut out);
        let other = Galaxy::new(Seed::new(SEED + 1));
        let mut theirs = Vec::new();
        cache
            .handle(GalaxyKey::new(SEED + 1, GENERATOR_VERSION))
            .bright_subset(&other, cell(), floor, &mut theirs);
        let mut expected = Vec::new();
        NoSkyCellCache.bright_subset(&other, cell(), floor, &mut expected);
        assert_eq!(theirs, expected);
        let counters = cache.counters().cache();
        assert_eq!((counters.entries(), counters.misses()), (2, 2));
    }

    /// With no budget nothing is kept, and every cell is still served.
    #[test]
    fn a_cache_of_no_bytes_serves_every_cell_and_keeps_none() {
        let cache = SharedSkyCellCache::new(0);
        let handle = cache.handle(key());
        let mut out = Vec::new();
        for _ in 0..2 {
            handle.bright_subset(galaxy(), cell(), SolarMasses::new(0.0), &mut out);
            assert_eq!(out, uncached(cell(), 0.0));
        }
        let counters = cache.counters().cache();
        assert_eq!(
            (counters.entries(), counters.misses(), counters.refused()),
            (0, 2, 2)
        );
    }

    /// A NaN floor is served as no cache serves it, and kept nowhere.
    #[test]
    fn a_nan_floor_is_not_kept() {
        let cache = SharedSkyCellCache::new(64 << 20);
        let mut out = vec![];
        cache
            .handle(key())
            .bright_subset(galaxy(), cell(), SolarMasses::new(f64::NAN), &mut out);
        assert!(out.is_empty(), "no mass is at or above NaN");
        assert_eq!(cache.counters().cache().entries(), 0);
    }

    #[test]
    #[should_panic(expected = "lends the cells of one galaxy")]
    fn a_handle_refuses_another_galaxys_cells() {
        let cache = SharedSkyCellCache::new(64 << 20);
        let other = Galaxy::new(Seed::new(SEED + 1));
        cache
            .handle(key())
            .bright_subset(&other, cell(), SolarMasses::new(1.0), &mut Vec::new());
    }

    /// A planted entry is what the cache serves at or above its floor, filtered, and a floor below
    /// it rebuilds the cell and replaces the entry: the cache reads its entries, and only through
    /// the sim's rule.
    #[test]
    fn a_planted_entry_is_served_at_or_above_its_floor_and_rebuilt_below_it() {
        let [all, low, high] = floors();
        let cache = SharedSkyCellCache::new(64 << 20);
        let handle = cache.handle(key());
        // Every other record of the cell at or above `low`: a subset no generation gives.
        let planted: Vec<SystemRecord> = uncached(cell(), low).into_iter().step_by(2).collect();
        let entry = SkyCellEntry {
            floor: SolarMasses::new(low),
            records: planted.clone(),
        };
        let _ = cache.cells.insert((key(), cell()), Arc::new(entry));
        let mut out = Vec::new();
        handle.bright_subset(galaxy(), cell(), SolarMasses::new(high), &mut out);
        let filtered: Vec<SystemRecord> = planted
            .iter()
            .filter(|record| record.primary_initial_mass().value() >= high)
            .copied()
            .collect();
        assert_eq!(out, filtered, "served from the planted entry");
        handle.bright_subset(galaxy(), cell(), SolarMasses::new(all), &mut out);
        assert_eq!(
            out,
            uncached(cell(), all),
            "rebuilt below the planted floor"
        );
        handle.bright_subset(galaxy(), cell(), SolarMasses::new(high), &mut out);
        assert_eq!(out, uncached(cell(), high), "served from the rebuilt entry");
        assert_eq!(cache.counters().rebuilt(), 1);
    }

    /// Each cell's bright subset at each floor is what no cache serves, whatever was asked before
    /// it: through a cache kept warm across the passes, one made fresh for every call, and one
    /// whose budget evicts under the walk, which stays within it.
    #[test]
    fn a_cells_bright_subset_does_not_depend_on_what_was_asked_before() {
        let asked: Vec<(CellKey, f64)> = sun_cells()
            .into_iter()
            .flat_map(|key| floors_of(key).map(|floor| (key, floor)))
            .collect();
        let serve = |cache: &SharedSkyCellCache, &(key, floor): &(CellKey, f64)| {
            let mut out = Vec::new();
            cache.handle(self::key()).bright_subset(
                galaxy(),
                key,
                SolarMasses::new(floor),
                &mut out,
            );
            assert_eq!(out, uncached(key, floor), "{key:?} at {floor} M☉");
            out
        };
        let warm = SharedSkyCellCache::new(64 << 20);
        assert_order_independent(&asked, |asked| serve(&warm, asked));
        assert_order_independent(&asked, |asked| {
            serve(&SharedSkyCellCache::new(64 << 20), asked)
        });
        // The charge of the largest entry the walk keeps: every entry fits, and no two do.
        let budget = sun_cells()
            .into_iter()
            .map(|key| {
                cell_heap_bytes(&uncached(key, 0.0))
                    + size_of::<SkyCellEntry>()
                    + ENTRY_OVERHEAD_BYTES
            })
            .max()
            .expect("five cells");
        let tight = SharedSkyCellCache::new(budget);
        assert_order_independent(&asked, |asked| serve(&tight, asked));
        let counters = tight.counters().cache();
        assert!(counters.bytes() <= budget, "{counters:?}");
        assert!(counters.evictions() > 0, "the walk evicts: {counters:?}");
    }
}
