//! The sky's cell cache: blocks of the census's cells, each cell's records an observer near the
//! block's builder could need, keyed by magnitude and bounded in bytes (rendering plan R06,
//! R06.T11.b and T8.h; Design note 12).
//!
//! Near the Sun the census's cost after the systems' own generation is placing each cell's records
//! and bounding them star by star. Records are epoch state, so an entry outlives the request that
//! built it: a sky asked again, or after a jump of up to 1,000 ly at any time within ±H, finds its
//! cells built (R06.T8.h). The sim builds every entry and applies the rule by which one serves a
//! query; this cache only holds them. [`SharedSkyCellCache`] is a [`SharedByteLru`] of the sim's
//! [`SkyBlock`]s, 4³ cells of one layer each, over `(GalaxyKey, BlockKey)`, so that two saves of
//! one seed share their cells and nothing generated under one generator version is lent for
//! another (plan 04, design note 23), under its own budget, `HYPERION_SKY_CACHE_MB`. It is not the
//! range queries' [`SharedCellCache`](super::SharedCellCache), which holds whole cells.
//!
//! A cell is kept by merging it into its block when the block was built for the same observer and
//! cut bit for bit, or by replacing the block otherwise ([`SkyBlock::with_cell`]). The merged block
//! is built outside the lock and stored only if the block it was built from is still the one held
//! ([`SharedByteLru::insert_if_unchanged`]), so two census jobs filling one block both land. The
//! census of a warm cell generates the same systems as a cold one, so the cache never changes a
//! reply.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::CellKey;
use hyperion_sim::sky::census::{
    BlockKey, BlockParams, CellOutcome, HeldRecord, Rebuild, SkyBlock, SkyCellCache,
};

use super::GalaxyKey;
use crate::cache::{HeapBytes, LruCounters, SharedByteLru};

/// What names a block in the cache: the galaxy it belongs to, and the block itself.
type SkyBlockKey = (GalaxyKey, BlockKey);

impl HeapBytes for SkyBlock {
    /// The held records' own bytes ([`SkyBlock::heap_bytes`]); they are kept at their length, with
    /// no spare capacity.
    fn heap_bytes(&self) -> usize {
        Self::heap_bytes(self)
    }
}

/// The sky cache's counters: its blocks and their use, and what the censuses did with each cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SkyCellCounters {
    cache: LruCounters,
    served: u64,
    missed: u64,
    rebuilt_key: u64,
    rebuilt_window: u64,
    rebuilt_parameters: u64,
}

impl SkyCellCounters {
    /// The blocks held, the bytes they are charged, the budget, and the hits, misses, evictions
    /// and refusals so far.
    ///
    /// A hit or a miss is one cell's lookup of its block, whether or not the block holds the cell
    /// or serves it ([`SkyCellCounters::served`]).
    #[must_use]
    pub fn cache(&self) -> LruCounters {
        self.cache
    }

    /// Cells served from their block.
    #[must_use]
    pub fn served(&self) -> u64 {
        self.served
    }

    /// Cells no block held, built at their block's parameters, or a new block's.
    #[must_use]
    pub fn missed(&self) -> u64 {
        self.missed
    }

    /// Cells their block held but could not serve, or whose block's parameters could not serve the
    /// query, rebuilt for `why` at the query's parameters, each replacing its block (Design note
    /// 12).
    #[must_use]
    pub fn rebuilt_for(&self, why: Rebuild) -> u64 {
        match why {
            Rebuild::Key => self.rebuilt_key,
            Rebuild::Window => self.rebuilt_window,
            Rebuild::Parameters => self.rebuilt_parameters,
        }
    }

    /// Cells rebuilt, whatever the cause ([`SkyCellCounters::rebuilt_for`]).
    #[must_use]
    pub fn rebuilt(&self) -> u64 {
        self.rebuilt_key + self.rebuilt_window + self.rebuilt_parameters
    }
}

/// The blocks of census cells of every galaxy the server holds, in one byte budget.
#[derive(Debug)]
pub(crate) struct SharedSkyCellCache {
    blocks: SharedByteLru<SkyBlockKey, SkyBlock>,
    served: AtomicU64,
    missed: AtomicU64,
    rebuilt_key: AtomicU64,
    rebuilt_window: AtomicU64,
    rebuilt_parameters: AtomicU64,
}

impl SharedSkyCellCache {
    /// An empty cache that holds at most `budget_bytes` of charged blocks.
    ///
    /// A budget of zero caches nothing: every cell is built, censused and dropped.
    #[must_use]
    pub(crate) fn new(budget_bytes: usize) -> Self {
        Self {
            blocks: SharedByteLru::new(budget_bytes),
            served: AtomicU64::new(0),
            missed: AtomicU64::new(0),
            rebuilt_key: AtomicU64::new(0),
            rebuilt_window: AtomicU64::new(0),
            rebuilt_parameters: AtomicU64::new(0),
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

    /// The blocks held, the bytes they are charged, the lookups, evictions and refusals so far,
    /// and the cells served, missed and rebuilt.
    #[must_use]
    pub(crate) fn counters(&self) -> SkyCellCounters {
        SkyCellCounters {
            cache: self.blocks.counters(),
            served: self.served.load(Ordering::Relaxed),
            missed: self.missed.load(Ordering::Relaxed),
            rebuilt_key: self.rebuilt_key.load(Ordering::Relaxed),
            rebuilt_window: self.rebuilt_window.load(Ordering::Relaxed),
            rebuilt_parameters: self.rebuilt_parameters.load(Ordering::Relaxed),
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

impl SkyCellCacheHandle<'_> {
    /// Asserts that `galaxy` is this handle's.
    ///
    /// # Panics
    ///
    /// If `galaxy`'s seed is not the seed of this handle's key, since its cells would then be kept
    /// under another galaxy's key. The server builds a handle per census job from the request's own
    /// universe, so a mismatch is a bug.
    fn assert_own(&self, galaxy: &Galaxy) {
        assert_eq!(
            galaxy.seed().get(),
            self.galaxy.seed(),
            "a sky cell cache handle lends the cells of one galaxy, and this is another's"
        );
    }
}

impl SkyCellCache for SkyCellCacheHandle<'_> {
    fn keeps_entries(&self) -> bool {
        true
    }

    /// The block `key` of this handle's galaxy, marked as the most recently used.
    ///
    /// # Panics
    ///
    /// If `galaxy` is not this handle's ([`SkyCellCacheHandle::assert_own`]).
    fn block(&self, galaxy: &Galaxy, key: BlockKey) -> Option<Arc<SkyBlock>> {
        self.assert_own(galaxy);
        self.cache.blocks.get(&(self.galaxy, key))
    }

    /// Keeps `cell` in its block: [`SkyBlock::with_cell`] of the block held, built with no lock
    /// held and stored only over the block it was built from, again from the block then held if
    /// another job replaced it meanwhile. A block larger than the whole budget is refused, and the
    /// cell was still censused.
    ///
    /// # Panics
    ///
    /// If `galaxy` is not this handle's ([`SkyCellCacheHandle::assert_own`]).
    fn keep(&self, galaxy: &Galaxy, cell: CellKey, params: &BlockParams, records: &[HeldRecord]) {
        self.assert_own(galaxy);
        let key = (self.galaxy, BlockKey::of(cell));
        // Each failed attempt means another job stored a block meanwhile, so the loop ends: jobs
        // that share a block are few, and each stores once a cell.
        loop {
            let held = self.cache.blocks.peek(&key);
            let Some(block) = SkyBlock::with_cell(held.as_deref(), cell, params, records) else {
                return;
            };
            if self
                .cache
                .blocks
                .insert_if_unchanged(key, held.as_ref(), Arc::new(block))
                .is_some()
            {
                return;
            }
        }
    }

    fn note(&self, _cell: CellKey, outcome: CellOutcome) {
        let counter = match outcome {
            CellOutcome::Served => &self.cache.served,
            CellOutcome::Missed => &self.cache.missed,
            CellOutcome::Rebuilt(Rebuild::Key) => &self.cache.rebuilt_key,
            CellOutcome::Rebuilt(Rebuild::Window) => &self.cache.rebuilt_window,
            CellOutcome::Rebuilt(Rebuild::Parameters) => &self.cache.rebuilt_parameters,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
    use hyperion_sim::galaxy::gas::noise::NoiseCache;
    use hyperion_sim::galaxy::placement::generate_cell;
    use hyperion_sim::id::Layer;
    use hyperion_sim::observe::Observer;
    use hyperion_sim::sky::census::{
        CellOffsets, CensusTallies, NoSkyCellCache, RecordLight, SkyContext, SkyQuery, SkyStar,
        census_cell,
    };
    use hyperion_sim::sky::envelope::BrightnessEnvelope;
    use hyperion_sim::sky::luminosity::LuminosityTables;
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::units::Magnitudes;
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

    /// The four layer-E cells along x of one block near the Sun's place.
    fn row() -> [CellKey; 4] {
        [0, 1, 2, 3].map(|x| CellKey::new(Layer::E, [x, 203, 0]).expect("a cell in the root cube"))
    }

    /// The census's parameters near the Sun at `cut`.
    fn params(cut: f64) -> BlockParams {
        let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).expect("in the cube");
        BlockParams::new(at, Magnitudes::new(cut))
    }

    /// `cell`'s records, held unbounded.
    fn held(galaxy: &Galaxy, cell: CellKey) -> Vec<HeldRecord> {
        let mut records = Vec::new();
        generate_cell(galaxy, cell, &mut records);
        records
            .iter()
            .map(|r| HeldRecord::new(*r, RecordLight::Unbounded))
            .collect()
    }

    /// A block's cells built at one census's parameters merge into one entry, charged its records,
    /// its size and the overhead; another census's parameters replace it; and another seed's cells
    /// are kept apart.
    #[test]
    fn cells_merge_into_their_block_and_other_parameters_replace_it() {
        let cache = SharedSkyCellCache::new(64 << 20);
        let handle = cache.handle(key());
        let [a, b, ..] = row();
        let (at_a, at_b) = (held(galaxy(), a), held(galaxy(), b));
        assert!(!at_a.is_empty() && !at_b.is_empty());
        handle.keep(galaxy(), a, &params(7.95), &at_a);
        handle.keep(galaxy(), b, &params(7.95), &at_b);
        let block = handle.block(galaxy(), BlockKey::of(a)).expect("kept");
        assert_eq!(
            (block.cell(a), block.cell(b)),
            (Some(&at_a[..]), Some(&at_b[..]))
        );
        let counters = cache.counters().cache();
        assert_eq!(counters.entries(), 1);
        assert_eq!(
            counters.bytes(),
            (at_a.len() + at_b.len()) * size_of::<HeldRecord>()
                + size_of::<SkyBlock>()
                + ENTRY_OVERHEAD_BYTES
        );
        // The same cell at the same parameters again changes nothing; other parameters replace
        // the block.
        handle.keep(galaxy(), a, &params(7.95), &at_a);
        assert!(Arc::ptr_eq(
            &block,
            &handle.block(galaxy(), BlockKey::of(a)).expect("kept")
        ));
        handle.keep(galaxy(), b, &params(9.0), &at_b);
        let replaced = handle.block(galaxy(), BlockKey::of(a)).expect("kept");
        assert_eq!((replaced.built_cells(), replaced.cell(a)), (1, None));

        let other = Galaxy::new(Seed::new(SEED + 1));
        let theirs = cache.handle(GalaxyKey::new(SEED + 1, GENERATOR_VERSION));
        assert!(theirs.block(&other, BlockKey::of(a)).is_none());
        theirs.keep(&other, a, &params(7.95), &held(&other, a));
        assert_eq!(cache.counters().cache().entries(), 2);
        assert_eq!(
            theirs.block(&other, BlockKey::of(a)).expect("kept").cell(a),
            Some(&held(&other, a)[..])
        );
    }

    /// Jobs filling one block's cells side by side all land in it.
    #[test]
    fn jobs_filling_one_block_side_by_side_all_land() {
        let cache = SharedSkyCellCache::new(64 << 20);
        let cells = row();
        let records: Vec<Vec<HeldRecord>> = cells.iter().map(|&c| held(galaxy(), c)).collect();
        std::thread::scope(|scope| {
            for (cell, records) in cells.iter().zip(&records) {
                let handle = cache.handle(key());
                scope.spawn(move || {
                    for _ in 0..50 {
                        handle.keep(galaxy(), *cell, &params(7.95), records);
                    }
                });
            }
        });
        let block = cache
            .handle(key())
            .block(galaxy(), BlockKey::of(cells[0]))
            .expect("kept");
        assert_eq!(block.built_cells(), 4);
        for (cell, records) in cells.iter().zip(&records) {
            assert_eq!(block.cell(*cell), Some(&records[..]));
        }
        assert_eq!(cache.counters().cache().entries(), 1);
    }

    /// With no budget nothing is kept.
    #[test]
    fn a_cache_of_no_bytes_keeps_no_block() {
        let cache = SharedSkyCellCache::new(0);
        let handle = cache.handle(key());
        let [a, ..] = row();
        handle.keep(galaxy(), a, &params(7.95), &held(galaxy(), a));
        assert!(handle.block(galaxy(), BlockKey::of(a)).is_none());
        let counters = cache.counters().cache();
        assert_eq!(
            (counters.entries(), counters.misses(), counters.refused()),
            (0, 1, 1)
        );
    }

    /// Each outcome the census reports is counted by its cause.
    #[test]
    fn the_counters_count_each_cells_outcome() {
        let cache = SharedSkyCellCache::new(64 << 20);
        let handle = cache.handle(key());
        let [a, ..] = row();
        for outcome in [
            CellOutcome::Served,
            CellOutcome::Served,
            CellOutcome::Missed,
            CellOutcome::Rebuilt(Rebuild::Key),
            CellOutcome::Rebuilt(Rebuild::Window),
            CellOutcome::Rebuilt(Rebuild::Window),
            CellOutcome::Rebuilt(Rebuild::Parameters),
        ] {
            handle.note(a, outcome);
        }
        let counters = cache.counters();
        assert_eq!(
            (
                counters.served(),
                counters.missed(),
                counters.rebuilt_for(Rebuild::Key),
                counters.rebuilt_for(Rebuild::Window),
                counters.rebuilt_for(Rebuild::Parameters),
                counters.rebuilt()
            ),
            (2, 1, 1, 2, 1, 4)
        );
    }

    /// What a census reads beside the cell cache: tables that hold no star, as a forced census's,
    /// the envelope and the cells' offset bounds, built once.
    fn census_tables() -> &'static (LuminosityTables, BrightnessEnvelope, CellOffsets) {
        static TABLES: OnceLock<(LuminosityTables, BrightnessEnvelope, CellOffsets)> =
            OnceLock::new();
        TABLES.get_or_init(|| {
            (
                LuminosityTables::dark(galaxy()),
                BrightnessEnvelope::build(galaxy()),
                CellOffsets::build(galaxy()),
            )
        })
    }

    /// One census cell's part as a reply carries it.
    type Part = (Vec<SkyStar>, CensusTallies);

    /// The census of `cell` for `query` through `cells`.
    fn census_of(cells: &dyn SkyCellCache, cell: CellKey, query: &SkyQuery) -> Part {
        let (tables, envelope, offsets) = census_tables();
        let mut ctx = SkyContext {
            tables,
            envelope,
            offsets,
            noise: NoiseCache::with_capacity(1 << 12),
            cells,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let mut stars = Vec::new();
        let tallies = census_cell(galaxy(), &mut ctx, cell, query, &mut stars);
        (stars, tallies)
    }

    /// Each census cell's stars and tallies through the server's own cache are what no cache
    /// gives, whatever was asked before (Design note 12, R06.T8.h): through a cache kept warm
    /// across the passes, one made fresh for every call, and one whose budget evicts under the
    /// walk, which stays within it. The queries are the Sun's at V 6, V 6 from 1,000 ly along x
    /// and the Sun's at V 9, so that the jump is served and the deeper cut rebuilds the cells about
    /// the Sun.
    #[test]
    fn a_cells_census_does_not_depend_on_what_was_asked_before() {
        let cells = [
            (Layer::A, [0, 3_250, 8]),
            (Layer::B, [0, 1_625, 4]),
            (Layer::C, [0, 812, 2]),
            (Layer::D, [0, 406, 1]),
            (Layer::E, [0, 203, 0]),
        ]
        .map(|(layer, index)| CellKey::new(layer, index).expect("a cell in the root cube"));
        let query = |x: f64, cut: f64| {
            let at = GalacticPosition::from_light_years([x, 26_000.0, 68.0]).expect("in the cube");
            SkyQuery::builder(
                Observer::new(at, UniverseTime::EPOCH).expect("an observer"),
                Magnitudes::new(cut),
            )
            .build()
            .expect("a query")
        };
        // The deeper cut after the shallower, so that a warm cache rebuilds the cells about the
        // Sun for its key.
        let queries = [query(0.0, 6.0), query(1_000.0, 6.0), query(0.0, 9.0)];
        let asked: Vec<(usize, usize)> = (0..cells.len())
            .flat_map(|c| (0..queries.len()).map(move |q| (c, q)))
            .collect();
        let expected: Vec<Part> = asked
            .iter()
            .map(|&(c, q)| census_of(&NoSkyCellCache, cells[c], &queries[q]))
            .collect();
        assert!(expected.iter().any(|(stars, _)| !stars.is_empty()));
        let serve = |cache: &SharedSkyCellCache, &(c, q): &(usize, usize)| {
            let part = census_of(&cache.handle(key()), cells[c], &queries[q]);
            let index = asked.iter().position(|&a| a == (c, q)).expect("asked");
            assert_eq!(part, expected[index], "{:?} for query {q}", cells[c]);
            part
        };
        let warm = SharedSkyCellCache::new(64 << 20);
        assert_order_independent(&asked, |asked| serve(&warm, asked));
        let counters = warm.counters();
        assert!(
            counters.served() > 0 && counters.rebuilt_for(Rebuild::Key) > 0,
            "{counters:?}"
        );
        assert_order_independent(&asked, |asked| {
            serve(&SharedSkyCellCache::new(64 << 20), asked)
        });
        // The charge of the largest block the walk keeps: every block fits, and no two do.
        let budget = cells
            .iter()
            .filter_map(|&cell| warm.blocks.peek(&(key(), BlockKey::of(cell))))
            .map(|block| block.heap_bytes() + size_of::<SkyBlock>() + ENTRY_OVERHEAD_BYTES)
            .max()
            .expect("five blocks");
        let tight = SharedSkyCellCache::new(budget);
        assert_order_independent(&asked, |asked| serve(&tight, asked));
        let counters = tight.counters().cache();
        assert!(counters.bytes() <= budget, "{counters:?}");
        assert!(counters.evictions() > 0, "the walk evicts: {counters:?}");
    }

    #[test]
    #[should_panic(expected = "lends the cells of one galaxy")]
    fn a_handle_refuses_another_galaxys_cells() {
        let cache = SharedSkyCellCache::new(64 << 20);
        let other = Galaxy::new(Seed::new(SEED + 1));
        let _ = cache.handle(key()).block(&other, BlockKey::of(row()[0]));
    }
}
