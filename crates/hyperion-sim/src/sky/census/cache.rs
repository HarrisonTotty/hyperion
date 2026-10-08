//! The census's cell cache: blocks of 4³ cells, each cell's records that an observer near the
//! block's builder could need, keyed by magnitude (rendering plan R06, R06.T8.a, T8.d and T8.h;
//! Design note 12).
//!
//! The sim holds no cache; the server's is a byte-bounded LRU behind a lock (R06.T11.b). Near the
//! Sun every C–E floor is its band's lower edge, so a key by mass would make an entry hold every
//! record of its cell (decided 2026-10-05, `decision-r06-census-cost.md`; amended 2026-10-08,
//! `decision-r06-t8h-warm.md`). So since R06.T8.h:
//!
//! - **An entry is a block** ([`SkyBlock`]) of 4³ cells of one layer ([`BlockKey`]), each built
//!   for one observer position and cut ([`BlockParams`]), so that an empty cell costs bytes, not a
//!   hundred. A cell's key and window are recomputed from those, so a cell stores neither:
//!   - its **key** is the faintest absolute V listable at its least distance from any observer
//!     within 1,000 ly of the builder's (`CACHE_APPROACH_LY`, the jump drive's range), at the cut
//!     plus 0.1 mag (`CACHE_CUT_SLACK_MAG`, provisional until R06.T8.n);
//!   - its **window** is every emitted time such an observer, at any time in ±H, can receive.
//! - **A cell holds** its records whose star-by-star bound over that window can be listed at the
//!   key, in candidate order, each with that bound ([`HeldRecord`]).
//! - **The rule** (`serve_from_block`): a held cell serves a query whose key is no fainter than
//!   its own and whose window it holds. Any other cell is rebuilt, at its block's parameters if
//!   they serve the query (and merged into the block), otherwise at the query's (replacing it).
//! - **The stored bound is a pre-filter.** Each held record it passes takes the census's own
//!   steps for the query, `hierarchy_bound` included, so a warm census generates the same systems
//!   as a cold one, and its stars and [`CensusTallies`](super::CensusTallies) are the cold ones'
//!   bit for bit. That rests on each bound only loosening as its window widens, which the tests
//!   hold. The counts that depend on the path go to [`CensusCost`](super::CensusCost).
//!
//! So the cache never changes a reply, and a jump of up to 1,000 ly within ±H rebuilds no cell.
//! The tests check it through a bounded cache of their own built on these types, as the server's
//! is: a query after a looser one, a tighter one, a move and a time of ±H gives the stars and
//! tallies no cache gives, and no entry is read for a fainter key or a window it does not hold.

use std::sync::Arc;

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::placement::{CellKey, SystemRecord};
use crate::id::Layer;
use crate::units::{Magnitudes, Years};

use super::cell::RecordLight;
use super::query::SkyQuery;

/// How far from its builder's observer, ly, an observer finds every cell of an entry it can hold
/// served: the jump drive's range (`single-player-experience.md`), so that a jump rebuilds no cell
/// its census opened before (R06.T8.h).
pub(crate) const CACHE_APPROACH_LY: f64 = 1_000.0;

/// How much fainter, mag, an entry's key is than its builder's cut needs, so that an observer whose
/// cut is a little fainter (the eye's cut moving between nearby places, R06.T9.d) is served:
/// provisional, R06.T8.n sets it in [0, 0.25] (`decision-r06-t8h-warm.md`).
pub(crate) const CACHE_CUT_SLACK_MAG: f64 = 0.1;

/// The cells along each side of a [`SkyBlock`].
pub(crate) const BLOCK_SIDE_CELLS: i32 = 4;

/// The cells of a [`SkyBlock`]: [`BLOCK_SIDE_CELLS`]³, one bit each of its `built` word.
const BLOCK_CELLS: usize = 64;

/// What an entry is built for (R06.T8.h): an observer's position and the cut it censuses to. A
/// cell's key and window follow from them, so the cell stores neither.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockParams {
    at: GalacticPosition,
    cut: Magnitudes,
}

impl BlockParams {
    /// An entry built for an observer at `at` censusing to apparent V `cut`.
    #[must_use]
    pub const fn new(at: GalacticPosition, cut: Magnitudes) -> Self {
        Self { at, cut }
    }

    /// The parameters `query` builds an entry at: its observer's position and its cut.
    #[must_use]
    pub fn of(query: &SkyQuery) -> Self {
        Self::new(*query.observer().position(), query.cut())
    }

    /// The builder's position.
    #[must_use]
    pub const fn at(&self) -> &GalacticPosition {
        &self.at
    }

    /// The builder's cut, apparent V.
    #[must_use]
    pub const fn cut(&self) -> Magnitudes {
        self.cut
    }

    /// Whether `other` is these parameters bit for bit, as a block's cells must share them to be
    /// merged into it.
    #[must_use]
    pub(crate) fn same_bits(&self, other: &Self) -> bool {
        // `total_cmp` is equal exactly when the bits are, signed zeros and NaN payloads apart.
        let same = |a: f64, b: f64| a.total_cmp(&b).is_eq();
        self.at.cell() == other.at.cell()
            && self
                .at
                .offset_metres()
                .into_iter()
                .zip(other.at.offset_metres())
                .all(|(a, b)| same(a, b))
            && same(self.cut.value(), other.cut.value())
    }
}

/// Which [`SkyBlock`] a cell belongs to: its layer, and its grid coordinates divided by
/// four, the cells along each side of a block, rounding down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockKey {
    layer: Layer,
    block: [i32; 3],
}

impl BlockKey {
    /// The block holding `cell`.
    #[must_use]
    pub fn of(cell: CellKey) -> Self {
        Self {
            layer: cell.layer(),
            block: cell
                .gen_cell()
                .to_array()
                .map(|c| c.div_euclid(BLOCK_SIDE_CELLS)),
        }
    }

    /// The block's layer.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// The block's coordinates: its cells' grid coordinates divided by four, rounding down.
    #[must_use]
    pub const fn coords(&self) -> [i32; 3] {
        self.block
    }

    /// `cell`'s index within the block, 0 to 63, or `None` if it is another block's.
    #[must_use]
    fn index_of(&self, cell: CellKey) -> Option<usize> {
        if Self::of(cell) != *self {
            return None;
        }
        let [x, y, z] = cell
            .gen_cell()
            .to_array()
            .map(|c| c.rem_euclid(BLOCK_SIDE_CELLS));
        Some(
            usize::try_from(x + BLOCK_SIDE_CELLS * (y + BLOCK_SIDE_CELLS * z))
                .expect("rem_euclid by BLOCK_SIDE_CELLS is in 0..4, so the index is in 0..64"),
        )
    }
}

/// A record a cell's entry holds, with its light star by star over the entry's window
/// ([`RecordLight`], the f64 never narrowed): 96 bytes, the record's 80 and its light's 16.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldRecord {
    record: SystemRecord,
    light: RecordLight,
}

impl HeldRecord {
    /// `record`, held with `light`.
    #[must_use]
    pub const fn new(record: SystemRecord, light: RecordLight) -> Self {
        Self { record, light }
    }

    /// The record.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// Its light over its entry's window: the pre-filter's bound.
    #[must_use]
    pub const fn light(&self) -> RecordLight {
        self.light
    }
}

/// One entry of a cell cache (R06.T8.h): a block of 4³ cells of one layer, each cell built or not,
/// all built for one observer position and cut ([`BlockParams`]), with each built cell's held
/// records, cell by cell in index order and each cell's in candidate order.
///
/// It weighs its held records (at most 96 bytes each, [`heap_bytes`](Self::heap_bytes)) and its
/// own size, some two hundred bytes, which a full block shares among its 64 cells.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyBlock {
    key: BlockKey,
    params: BlockParams,
    built: u64,
    counts: [u16; BLOCK_CELLS],
    records: Vec<HeldRecord>,
}

impl SkyBlock {
    /// The block `old` (if any) with `cell`'s held `records`, built at `params`, in it:
    ///
    /// - merged into `old` if `old` is `cell`'s block built at `params` bit for bit and does not
    ///   hold `cell` yet;
    /// - `None` if `old` already holds `cell` at `params` (the same records: a census is a pure
    ///   function of its query), or if `records` are more than a cell's count can say (65,535),
    ///   so that the block held stays;
    /// - otherwise a new block of `cell` alone, which replaces `old`.
    ///
    /// A cache keeps whatever this returns under `cell`'s [`BlockKey`], under its lock or by
    /// replacing `old` only if it is still the block held.
    #[must_use]
    pub fn with_cell(
        old: Option<&Self>,
        cell: CellKey,
        params: &BlockParams,
        records: &[HeldRecord],
    ) -> Option<Self> {
        let key = BlockKey::of(cell);
        let index = key.index_of(cell)?;
        let count = u16::try_from(records.len()).ok()?;
        let bit = 1_u64 << index;
        match old {
            Some(old) if old.key == key && old.params.same_bits(params) => {
                if old.built & bit != 0 {
                    return None;
                }
                let start = old.start_of(index);
                let mut held = Vec::with_capacity(old.records.len() + records.len());
                held.extend_from_slice(&old.records[..start]);
                held.extend_from_slice(records);
                held.extend_from_slice(&old.records[start..]);
                let mut counts = old.counts;
                counts[index] = count;
                Some(Self {
                    key,
                    params: *params,
                    built: old.built | bit,
                    counts,
                    records: held,
                })
            }
            _ => {
                let mut counts = [0; BLOCK_CELLS];
                counts[index] = count;
                Some(Self {
                    key,
                    params: *params,
                    built: bit,
                    counts,
                    records: records.to_vec(),
                })
            }
        }
    }

    /// The block's key.
    #[must_use]
    pub const fn key(&self) -> BlockKey {
        self.key
    }

    /// What its cells were built for.
    #[must_use]
    pub const fn params(&self) -> &BlockParams {
        &self.params
    }

    /// `cell`'s held records, in candidate order, if the block holds `cell`.
    #[must_use]
    pub fn cell(&self, cell: CellKey) -> Option<&[HeldRecord]> {
        let index = self.key.index_of(cell)?;
        if self.built & (1 << index) == 0 {
            return None;
        }
        let start = self.start_of(index);
        self.records
            .get(start..start + usize::from(self.counts[index]))
    }

    /// The cells built.
    #[must_use]
    pub const fn built_cells(&self) -> u32 {
        self.built.count_ones()
    }

    /// Every held record of every built cell.
    #[must_use]
    pub fn held(&self) -> &[HeldRecord] {
        &self.records
    }

    /// The bytes the block owns on the heap, beyond `size_of::<SkyBlock>()`: its held records, at
    /// their length, with no spare capacity.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.records
            .capacity()
            .saturating_mul(size_of::<HeldRecord>())
    }

    /// Where cell `index`'s records start: the records of the cells before it.
    #[must_use]
    fn start_of(&self, index: usize) -> usize {
        self.counts[..index].iter().map(|&c| usize::from(c)).sum()
    }
}

/// What an entry holds of a cell, or what a query needs of it (R06.T8.h): the faintest absolute V
/// listable at the cell's least distance (its key), and the emitted times its light can have left
/// it (its window, Julian years from the epoch, inclusive).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CellNeed {
    key: Magnitudes,
    /// The window's earliest and latest emitted times, Julian years from the epoch.
    window_years: (f64, f64),
}

impl CellNeed {
    /// A need of `key` over the emitted times `window_years`, Julian years from the epoch.
    #[must_use]
    pub(crate) const fn new(key: Magnitudes, window_years: (f64, f64)) -> Self {
        Self { key, window_years }
    }

    /// The key: the faintest absolute V listable.
    #[must_use]
    pub(crate) const fn key(&self) -> Magnitudes {
        self.key
    }

    /// Whether these, an entry's, hold `need`: its key no fainter and its window within. A NaN on
    /// either side refuses.
    #[must_use]
    pub(crate) fn holds(&self, need: &Self) -> bool {
        self.holds_key(need) && self.holds_window(need)
    }

    #[must_use]
    fn holds_key(&self, need: &Self) -> bool {
        // Not `need > self`, which a NaN on either side would pass.
        need.key.value() <= self.key.value()
    }

    #[must_use]
    fn holds_window(&self, need: &Self) -> bool {
        self.window_years.0 <= need.window_years.0 && need.window_years.1 <= self.window_years.1
    }

    /// `record`'s ages over the window, years, inclusive.
    #[must_use]
    pub(crate) fn ages_of(&self, record: &SystemRecord) -> (Years, Years) {
        let at_epoch = record.age_at_epoch().value();
        (
            Years::new(at_epoch + self.window_years.0),
            Years::new(at_epoch + self.window_years.1),
        )
    }
}

/// What [`serve_from_block`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use = "a refused cell leaves `out` untouched: it must be built"]
pub(crate) enum Lookup {
    /// `out` holds the cell's held records, in candidate order.
    Served,
    /// The block does not hold the cell.
    NotBuilt,
    /// The query's key is fainter than the entry's, or either is NaN.
    Key,
    /// The query's window is not within the entry's, or either has a NaN.
    Window,
}

/// Serves `cell` from `block` to a query that needs `need` of it, the entry holding `held` of it
/// (the census's [`entry_need`](super::cell::entry_need) at the block's parameters): the one place
/// the rule is written (R06.T8.h). [`Lookup::Served`], with `out` holding the cell's held records,
/// when the block holds the cell and `held` holds `need`; otherwise why not, and `out` untouched.
pub(crate) fn serve_from_block(
    block: &SkyBlock,
    cell: CellKey,
    held: &CellNeed,
    need: &CellNeed,
    out: &mut Vec<HeldRecord>,
) -> Lookup {
    let Some(records) = block.cell(cell) else {
        return Lookup::NotBuilt;
    };
    if !held.holds_key(need) {
        return Lookup::Key;
    }
    if !held.holds_window(need) {
        return Lookup::Window;
    }
    out.clear();
    out.extend_from_slice(records);
    Lookup::Served
}

/// Why a cell was rebuilt at the query's parameters, replacing its block (R06.T8.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rebuild {
    /// The cell's entry's key is brighter than the query's: the query can list fainter stars.
    Key,
    /// The cell's entry's window does not hold the query's.
    Window,
    /// The block does not hold the cell, and its parameters do not serve the query.
    Parameters,
}

/// What a census did with one cell, as a [`SkyCellCache`] is told it ([`SkyCellCache::note`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellOutcome {
    /// Served from its block.
    Served,
    /// No entry held it: built at its block's parameters and merged in, or into a new block.
    Missed,
    /// Rebuilt at the query's parameters, replacing its block.
    Rebuilt(Rebuild),
}

/// Where the census keeps and finds its cells' entries (R06.T8.h): blocks of cells, each built for
/// one observer and cut ([`SkyBlock`]). The census itself applies the rule, builds every entry and
/// decides every star; the cache only holds blocks, so it never changes a reply.
///
/// It takes `&self` so that the census's parallel jobs share one; an implementation that keeps
/// entries uses interior mutability behind a lock. Such an implementation:
///
/// - keeps what [`SkyBlock::with_cell`] returns for the block it holds, replacing that block only
///   if it is still the one held, so that two jobs filling one block both land;
/// - keys its blocks by galaxy as well as by [`BlockKey`], since a [`CellKey`] names the same cell
///   in every galaxy;
/// - is bounded in bytes, a block weighing at least its held records
///   ([`SkyBlock::heap_bytes`]): the server's is a `ByteLru` under `HYPERION_SKY_CACHE_MB`
///   (R06.T11.b). Eviction is always safe, since an evicted cell is rebuilt.
///
/// # Examples
///
/// A cache of the last block it was given, keyed by the galaxy's seed too (every galaxy it serves
/// is [`Galaxy::new`] of its seed). An empty cell is held, as a cell all of whose records failed
/// their bounds is, so that it is not bounded again:
///
/// ```
/// use std::sync::{Arc, Mutex};
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::sky::census::{
///     BlockKey, BlockParams, HeldRecord, RecordLight, SkyBlock, SkyCellCache,
/// };
/// use hyperion_sim::units::Magnitudes;
///
/// #[derive(Debug, Default)]
/// struct LastBlock {
///     held: Mutex<Option<(Seed, Arc<SkyBlock>)>>,
/// }
///
/// impl SkyCellCache for LastBlock {
///     fn keeps_entries(&self) -> bool {
///         true
///     }
///
///     fn block(&self, galaxy: &Galaxy, key: BlockKey) -> Option<Arc<SkyBlock>> {
///         let held = self.held.lock().expect("not poisoned");
///         let (seed, block) = held.as_ref()?;
///         (*seed == galaxy.seed() && block.key() == key).then(|| Arc::clone(block))
///     }
///
///     fn keep(&self, galaxy: &Galaxy, cell: CellKey, params: &BlockParams, records: &[HeldRecord]) {
///         let mut held = self.held.lock().expect("not poisoned");
///         let old = held
///             .as_ref()
///             .filter(|(seed, _)| *seed == galaxy.seed())
///             .map(|(_, block)| block.as_ref());
///         if let Some(block) = SkyBlock::with_cell(old, cell, params, records) {
///             *held = Some((galaxy.seed(), Arc::new(block)));
///         }
///     }
/// }
///
/// let galaxy = Galaxy::new(Seed::new(19));
/// let cache = LastBlock::default();
/// let (a, b) = (CellKey::new(Layer::E, [0, 203, 0])?, CellKey::new(Layer::E, [1, 203, 0])?);
/// let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let params = BlockParams::new(at, Magnitudes::new(7.95));
/// let mut records = Vec::new();
/// generate_cell(&galaxy, a, &mut records);
/// let held: Vec<HeldRecord> =
///     records.iter().map(|r| HeldRecord::new(*r, RecordLight::Unbounded)).collect();
/// cache.keep(&galaxy, a, &params, &held);
/// cache.keep(&galaxy, b, &params, &[]);
/// let block = cache.block(&galaxy, BlockKey::of(a)).ok_or("kept")?;
/// assert_eq!(block.cell(a), Some(&held[..]));
/// assert_eq!(block.cell(b), Some(&[][..]));
/// assert_eq!(block.built_cells(), 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait SkyCellCache: Sync {
    /// Whether the cache keeps entries. With none ([`NoSkyCellCache`]) the census places every
    /// cell and takes each record's hierarchy once, R06.T8.g's path, and builds no entry.
    fn keeps_entries(&self) -> bool;

    /// The block of `galaxy` that `key` names, if the cache holds it.
    fn block(&self, galaxy: &Galaxy, key: BlockKey) -> Option<Arc<SkyBlock>>;

    /// Keeps `cell`'s held `records`, built at `params`, in its block: what
    /// [`SkyBlock::with_cell`] gives of the block held. A cache may decline, which is always safe.
    fn keep(&self, galaxy: &Galaxy, cell: CellKey, params: &BlockParams, records: &[HeldRecord]);

    /// Told what the census did with `cell`, for the cache's counters. Nothing, by default.
    fn note(&self, _cell: CellKey, _outcome: CellOutcome) {}
}

/// No cache: every cell is placed afresh, as R06.T8.g's census places it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NoSkyCellCache;

impl SkyCellCache for NoSkyCellCache {
    fn keeps_entries(&self) -> bool {
        false
    }

    fn block(&self, _galaxy: &Galaxy, _key: BlockKey) -> Option<Arc<SkyBlock>> {
        None
    }

    fn keep(
        &self,
        _galaxy: &Galaxy,
        _cell: CellKey,
        _params: &BlockParams,
        _records: &[HeldRecord],
    ) {
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::sync::{Mutex, MutexGuard};

    use hyperion_testkit::float;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::{GalacticPosition, UnitVector};
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::placement::generate_cell;
    use crate::observe::Observer;
    use crate::sky::census::cell::{
        CensusCost, CensusTallies, LayerCost, SkyStar, census_cell_with_cost,
    };
    use crate::sky::census::query::{Cone, SkyContext, SkyQuery};
    use crate::sky::testing::{
        milky_way_dark_tables, milky_way_envelope, milky_way_offsets, moving_galaxy,
    };
    use crate::time::{ClockWindow, UniverseTime};
    use crate::units::Degrees;

    /// The Sun's place in the fixture, ly.
    const SUN: [f64; 3] = [0.0, 26_000.0, 68.0];

    /// One cell's census as a reply carries it: its stars and its tallies.
    type Part = (Vec<SkyStar>, CensusTallies);

    /// What the test cache was told and did since its counts were last taken.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    struct Counts {
        /// Cells served.
        served: usize,
        /// Cells with no entry, built at their block's parameters or a new block's.
        missed: usize,
        /// Cells rebuilt for their key, their window, or their block's parameters.
        key: usize,
        window: usize,
        parameters: usize,
        /// Blocks evicted to keep within the bound.
        evicted: usize,
    }

    impl Counts {
        /// Every cell rebuilt, whatever the cause.
        const fn rebuilt(&self) -> usize {
            self.key + self.window + self.parameters
        }
    }

    /// One block as the test cache holds it.
    #[derive(Debug)]
    struct Entry {
        block: Arc<SkyBlock>,
        /// Its weight against the bound.
        bytes: usize,
        /// The cache's clock when it was last kept or read.
        used: u64,
    }

    /// What a block costs beyond its held records' heap: its key, the map's entry and itself.
    const ENTRY_OVERHEAD_BYTES: usize =
        size_of::<BlockKey>() + size_of::<Entry>() + size_of::<SkyBlock>();

    /// What the lock guards.
    #[derive(Debug, Default)]
    struct Kept {
        blocks: BTreeMap<BlockKey, Entry>,
        bytes: usize,
        clock: u64,
        counts: Counts,
    }

    /// A cell cache built on [`SkyBlock::with_cell`], as the server's is (R06.T11.b, T8.h): one
    /// galaxy's blocks behind a lock, shared through `&self`, evicting the least recently used.
    ///
    /// **Memory bound:** at most `max_blocks` blocks, together weighing at most `max_bytes` bytes,
    /// a block weighing its held records' heap ([`SkyBlock::heap_bytes`]) plus
    /// [`ENTRY_OVERHEAD_BYTES`]. The map's own nodes and the fixed-size [`Counts`] are not
    /// counted. A block heavier than `max_bytes` alone is not kept, and the block it would replace
    /// stays.
    #[derive(Debug)]
    struct KeepBlocks<'g> {
        galaxy: &'g Galaxy,
        max_blocks: usize,
        max_bytes: usize,
        kept: Mutex<Kept>,
    }

    impl<'g> KeepBlocks<'g> {
        fn new(galaxy: &'g Galaxy, max_blocks: usize, max_bytes: usize) -> Self {
            Self {
                galaxy,
                max_blocks,
                max_bytes,
                kept: Mutex::new(Kept::default()),
            }
        }

        /// A cache bounded only by what the tests ask of it.
        fn roomy(galaxy: &'g Galaxy) -> Self {
            Self::new(galaxy, usize::MAX, usize::MAX)
        }

        fn lock(&self) -> MutexGuard<'_, Kept> {
            self.kept
                .lock()
                .expect("a poisoned lock means a test already failed")
        }

        /// The counts since they were last taken.
        fn take_counts(&self) -> Counts {
            std::mem::take(&mut self.lock().counts)
        }

        /// Asserts the blocks and bytes held within the bound and the bytes the blocks' own
        /// weights, and returns the blocks and bytes held.
        fn assert_within_bound(&self) -> (usize, usize) {
            let (blocks, bytes, weights) = {
                let kept = self.lock();
                let weights: usize = kept.blocks.values().map(|e| e.bytes).sum();
                (kept.blocks.len(), kept.bytes, weights)
            };
            assert_eq!(weights, bytes, "the bytes held are the blocks' weights");
            assert!(blocks <= self.max_blocks, "{blocks} blocks");
            assert!(bytes <= self.max_bytes, "{bytes} bytes");
            (blocks, bytes)
        }

        /// The block `key` names, without marking a use.
        fn held(&self, key: BlockKey) -> Option<Arc<SkyBlock>> {
            self.lock().blocks.get(&key).map(|e| Arc::clone(&e.block))
        }

        /// Replaces `block`'s key's block with it, whatever it held: a planted block.
        fn plant(&self, block: SkyBlock) {
            let mut kept = self.lock();
            self.store(&mut kept, block);
        }

        /// Stores `block` under its key, then evicts the least recently used others until within
        /// the bound.
        fn store(&self, kept: &mut Kept, block: SkyBlock) {
            let bytes = block.heap_bytes().saturating_add(ENTRY_OVERHEAD_BYTES);
            if bytes > self.max_bytes || self.max_blocks == 0 {
                return;
            }
            kept.clock += 1;
            let key = block.key();
            let entry = Entry {
                block: Arc::new(block),
                bytes,
                used: kept.clock,
            };
            if let Some(old) = kept.blocks.insert(key, entry) {
                kept.bytes -= old.bytes;
            }
            kept.bytes += bytes;
            // The block just stored is the most recent and within the bound alone, so it stays.
            while kept.blocks.len() > self.max_blocks || kept.bytes > self.max_bytes {
                let oldest = kept
                    .blocks
                    .iter()
                    .min_by_key(|(_, e)| e.used)
                    .map(|(&k, _)| k)
                    .expect("over the bound, so not empty");
                let gone = kept.blocks.remove(&oldest).expect("just found");
                kept.bytes -= gone.bytes;
                kept.counts.evicted += 1;
            }
        }
    }

    impl SkyCellCache for KeepBlocks<'_> {
        fn keeps_entries(&self) -> bool {
            true
        }

        fn block(&self, galaxy: &Galaxy, key: BlockKey) -> Option<Arc<SkyBlock>> {
            assert!(
                std::ptr::eq(galaxy, self.galaxy),
                "a cell cache is for the galaxy it was made for"
            );
            let mut guard = self.lock();
            let kept = &mut *guard;
            kept.clock += 1;
            let entry = kept.blocks.get_mut(&key)?;
            entry.used = kept.clock;
            Some(Arc::clone(&entry.block))
        }

        fn keep(
            &self,
            galaxy: &Galaxy,
            cell: CellKey,
            params: &BlockParams,
            records: &[HeldRecord],
        ) {
            assert!(std::ptr::eq(galaxy, self.galaxy));
            let mut kept = self.lock();
            let old = kept
                .blocks
                .get(&BlockKey::of(cell))
                .map(|e| Arc::clone(&e.block));
            if let Some(block) = SkyBlock::with_cell(old.as_deref(), cell, params, records) {
                self.store(&mut kept, block);
            }
        }

        fn note(&self, _cell: CellKey, outcome: CellOutcome) {
            let counts = &mut self.lock().counts;
            match outcome {
                CellOutcome::Served => counts.served += 1,
                CellOutcome::Missed => counts.missed += 1,
                CellOutcome::Rebuilt(Rebuild::Key) => counts.key += 1,
                CellOutcome::Rebuilt(Rebuild::Window) => counts.window += 1,
                CellOutcome::Rebuilt(Rebuild::Parameters) => counts.parameters += 1,
            }
        }
    }

    fn context(cells: &dyn SkyCellCache) -> SkyContext<'_> {
        SkyContext {
            tables: milky_way_dark_tables(),
            envelope: milky_way_envelope(),
            offsets: milky_way_offsets(),
            noise: NoiseCache::with_capacity(1 << 12),
            cells,
            sources: &[],
            modifiers: &NoModifiers,
        }
    }

    fn position(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("in the cube")
    }

    /// `SUN` moved `dx` light-years along x.
    fn along(dx: f64) -> [f64; 3] {
        [SUN[0] + dx, SUN[1], SUN[2]]
    }

    fn query_at_time(at: [f64; 3], t: UniverseTime, cut: f64) -> SkyQuery {
        let observer = Observer::new(position(at), t).expect("an observer");
        SkyQuery::builder(observer, Magnitudes::new(cut))
            .build()
            .expect("a valid query")
    }

    fn query_at(at: [f64; 3], cut: f64) -> SkyQuery {
        query_at_time(at, UniverseTime::EPOCH, cut)
    }

    /// The queries the tests ask, each at the Sun at the epoch unless named otherwise:
    /// - `loose`, deeper (V 9), with a 60° cone along +x, which keeps only the stars of its
    ///   region's texels but builds the same entries;
    /// - `tight`, V 6;
    /// - `toward` and `away`, V 6 from 1,000 ly along +x and −x: a jump the entries hold;
    /// - `late` and `early`, V 6 at +H and at −H;
    /// - `far`, V 6 from 1,500 ly along +x, nearer the far cells than their entries allow.
    struct Queries {
        loose: SkyQuery,
        tight: SkyQuery,
        toward: SkyQuery,
        away: SkyQuery,
        late: SkyQuery,
        early: SkyQuery,
        far: SkyQuery,
    }

    fn queries() -> Queries {
        let cone = Cone::new(UnitVector::X, Degrees::new(60.0)).expect("a cone");
        let loose = SkyQuery::builder(
            Observer::new(position(SUN), UniverseTime::EPOCH).expect("an observer"),
            Magnitudes::new(9.0),
        )
        .cone(cone)
        .build()
        .expect("a valid query");
        Queries {
            loose,
            tight: query_at(SUN, 6.0),
            toward: query_at(along(1_000.0), 6.0),
            away: query_at(along(-1_000.0), 6.0),
            late: query_at_time(SUN, ClockWindow::END, 6.0),
            early: query_at_time(SUN, ClockWindow::START, 6.0),
            far: query_at(along(1_500.0), 6.0),
        }
    }

    /// Cells along x through the Sun: of layers A and B from 800 ly on one side to 800 ly on the
    /// other, of C from 1,600 ly on one side to 2,600 ly on the other, and of D and E at 1,600 ly
    /// on one side, at the Sun and 200 ly along, and at 1,600 and 2,600 ly on the other. The far
    /// cells lie beyond the approach, so their keys part with the query's place and cut.
    fn cells() -> Vec<CellKey> {
        let wide: &[f64] = &[-800.0, -200.0, -100.0, 0.0, 100.0, 200.0, 800.0];
        let deep: &[f64] = &[-1_600.0, -800.0, 0.0, 200.0, 1_600.0, 2_600.0];
        let sparse: &[f64] = &[-1_600.0, 0.0, 200.0, 1_600.0, 2_600.0];
        let mut cells = Vec::new();
        for (layer, at) in [
            (Layer::A, wide),
            (Layer::B, wide),
            (Layer::C, deep),
            (Layer::D, sparse),
            (Layer::E, sparse),
        ] {
            for &dx in at {
                cells.push(CellKey::containing(layer, &position(along(dx))).expect("in the cube"));
            }
        }
        cells
    }

    fn census_of_in(
        galaxy: &Galaxy,
        key: CellKey,
        query: &SkyQuery,
        ctx: &mut SkyContext<'_>,
    ) -> (Part, CensusCost) {
        let mut stars = Vec::new();
        let (tallies, cost) = census_cell_with_cost(galaxy, ctx, key, query, &mut stars);
        ((stars, tallies), cost)
    }

    fn census_of(key: CellKey, query: &SkyQuery, ctx: &mut SkyContext<'_>) -> Part {
        census_of_in(milky_way_galaxy(), key, query, ctx).0
    }

    /// The census of `cells` for `query` in `galaxy`, and its cost summed.
    fn census_in(
        galaxy: &Galaxy,
        cells: &[CellKey],
        query: &SkyQuery,
        ctx: &mut SkyContext<'_>,
    ) -> (Vec<Part>, CensusCost) {
        let mut cost = CensusCost::default();
        let parts = cells
            .iter()
            .map(|&key| {
                let (part, c) = census_of_in(galaxy, key, query, ctx);
                cost.add(&c);
                part
            })
            .collect();
        (parts, cost)
    }

    fn census(cells: &[CellKey], query: &SkyQuery, ctx: &mut SkyContext<'_>) -> Vec<Part> {
        census_in(milky_way_galaxy(), cells, query, ctx).0
    }

    /// The census of `cells` for `query` with no cache.
    fn uncached(cells: &[CellKey], query: &SkyQuery) -> Vec<Part> {
        census(cells, query, &mut context(&NoSkyCellCache))
    }

    /// Asserts that `parts` list stars, so that their bits are checked.
    #[track_caller]
    fn assert_lists_stars(parts: &[Part], what: &str) {
        assert!(
            parts.iter().any(|(s, _)| !s.is_empty()),
            "{what} lists stars"
        );
    }

    /// One count of `cost` summed over its layers.
    fn total(cost: &CensusCost, count: fn(&LayerCost) -> u64) -> u64 {
        Layer::ALL.iter().map(|&l| count(cost.layer(l))).sum()
    }

    /// Every float of the stars of `parts`, as bits: `PartialEq` holds 0.0 and −0.0 equal.
    fn float_bits(parts: &[Part]) -> Vec<u64> {
        let mut bits = Vec::new();
        for star in parts.iter().flat_map(|(stars, _)| stars) {
            let c = star.colour();
            // The reddening tables are read at the colour's place in its grid: through a magnitude of
            // dust, its bits are that place's.
            let r = c.reddened(Magnitudes::new(1.0));
            let p = star.apparent().to_light_years_f64();
            let floats = [
                star.distance().value(),
                star.v().value(),
                star.a_v().value(),
                c.lux_per_v0(),
                c.sp_ratio(),
                c.camera_band_mag(),
                r.photopic_transmission(),
                r.scotopic_transmission(),
                r.v_extinction().value(),
                r.camera_band_mag(),
            ];
            let arrays = p
                .into_iter()
                .chain(c.red_green())
                .chain(c.extinction_ratio())
                .chain(r.red_green())
                .chain(c.bake_spectrum());
            bits.extend(floats.into_iter().chain(arrays).map(float::bits));
        }
        bits
    }

    /// The stars and tallies of `got` are `expected`'s, bit for bit: what a reply carries.
    #[track_caller]
    fn assert_same_bits(got: &[Part], expected: &[Part], what: &str) {
        assert_eq!(got, expected, "{what}");
        assert_eq!(float_bits(got), float_bits(expected), "{what}");
    }

    /// Some records of `key`, each held with a light of its own, in candidate order.
    fn held_records(key: CellKey) -> Vec<HeldRecord> {
        let mut records = Vec::new();
        generate_cell(milky_way_galaxy(), key, &mut records);
        assert!(records.len() >= 3, "{key:?}: {}", records.len());
        records
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let light = match i % 3 {
                    0 => RecordLight::Unbounded,
                    1 => RecordLight::Brightest(Magnitudes::new(-1.0)),
                    _ => RecordLight::Brightest(Magnitudes::new(4.5)),
                };
                HeldRecord::new(*r, light)
            })
            .collect()
    }

    #[test]
    fn a_held_record_is_96_bytes() {
        assert_eq!(size_of::<SystemRecord>(), 80);
        assert_eq!(size_of::<RecordLight>(), 16);
        assert_eq!(size_of::<HeldRecord>(), 96);
    }

    /// A C cell by the Sun, another cell of its block and a cell of the next block along x.
    fn block_cells() -> [CellKey; 3] {
        let [cell, unbuilt, elsewhere] = [[0, 812, 0], [1, 812, 0], [4, 812, 0]]
            .map(|at| CellKey::new(Layer::C, at).expect("in the cube"));
        assert_eq!(BlockKey::of(cell), BlockKey::of(unbuilt));
        assert_ne!(BlockKey::of(cell), BlockKey::of(elsewhere));
        [cell, unbuilt, elsewhere]
    }

    /// The rule's three conditions: the cell built in the block, the query's key no fainter than
    /// the entry's and its window within the entry's. A NaN refuses, a refusal leaves the output
    /// untouched, and a served cell's records are its own, in candidate order.
    #[test]
    fn the_rule_serves_a_key_no_fainter_and_a_window_it_holds() {
        let [cell, unbuilt, elsewhere] = block_cells();
        let records = held_records(cell);
        let params = BlockParams::new(position(SUN), Magnitudes::new(6.0));
        let block = SkyBlock::with_cell(None, cell, &params, &records).expect("a new block");
        let held = CellNeed::new(Magnitudes::new(5.0), (-100.0, 100.0));
        let need = |key: f64, lo: f64, hi: f64| CellNeed::new(Magnitudes::new(key), (lo, hi));
        let sentinel = HeldRecord::new(*records[0].record(), RecordLight::Dark);
        let serve = |c: CellKey, held: &CellNeed, asked: &CellNeed| {
            let mut out = vec![sentinel];
            let served = serve_from_block(&block, c, held, asked, &mut out);
            (served, out)
        };
        for asked in [
            need(5.0, -100.0, 100.0),
            need(4.0, -50.0, 50.0),
            need(f64::NEG_INFINITY, -100.0, -100.0),
        ] {
            let (served, out) = serve(cell, &held, &asked);
            assert_eq!(served, Lookup::Served, "{asked:?}");
            assert_eq!(out, records, "the cell's own records, in candidate order");
            assert!(held.holds(&asked));
        }
        let fainter = 5.0_f64.next_up();
        for (asked, why) in [
            (need(fainter, -100.0, 100.0), Lookup::Key),
            (need(f64::NAN, -100.0, 100.0), Lookup::Key),
            (need(5.0, (-100.0_f64).next_down(), 100.0), Lookup::Window),
            (need(5.0, -100.0, 100.0_f64.next_up()), Lookup::Window),
            (need(5.0, f64::NAN, 100.0), Lookup::Window),
            (need(5.0, -100.0, f64::NAN), Lookup::Window),
        ] {
            assert_eq!(
                serve(cell, &held, &asked),
                (why, vec![sentinel]),
                "{asked:?}"
            );
            assert!(!held.holds(&asked));
        }
        let nan_held = need(f64::NAN, -100.0, 100.0);
        assert_eq!(
            serve(cell, &nan_held, &need(5.0, -100.0, 100.0)).0,
            Lookup::Key
        );
        let nan_window = need(5.0, f64::NAN, 100.0);
        assert_eq!(
            serve(cell, &nan_window, &need(5.0, -100.0, 100.0)).0,
            Lookup::Window
        );
        for c in [unbuilt, elsewhere] {
            assert_eq!(
                serve(c, &held, &need(5.0, -100.0, 100.0)),
                (Lookup::NotBuilt, vec![sentinel])
            );
        }
    }

    /// A block merges a cell built at its parameters bit for bit, in the cell's place, whatever
    /// the order its cells arrive in; the same cell again is nothing new; other parameters, even by
    /// a bit, replace the block; and a cell's count is held to its 16 bits.
    #[test]
    fn a_block_merges_cells_at_its_parameters_and_others_replace_it() {
        let [cell, unbuilt, _] = block_cells();
        let records = held_records(cell);
        let params = BlockParams::new(position(SUN), Magnitudes::new(6.0));
        let block = SkyBlock::with_cell(None, cell, &params, &records).expect("a new block");
        let other = held_records(unbuilt);
        let merged =
            SkyBlock::with_cell(Some(&block), unbuilt, &params, &other).expect("merged in");
        assert_eq!(merged.built_cells(), 2);
        assert_eq!(merged.cell(cell), Some(&records[..]));
        assert_eq!(merged.cell(unbuilt), Some(&other[..]));
        let mut both = records.clone();
        both.extend_from_slice(&other);
        assert_eq!(merged.held(), &both[..], "cell by cell, in index order");
        assert_eq!(
            merged.heap_bytes(),
            both.len() * size_of::<HeldRecord>(),
            "no spare capacity"
        );
        let first = SkyBlock::with_cell(None, unbuilt, &params, &other).expect("a new block");
        let before = SkyBlock::with_cell(Some(&first), cell, &params, &records).expect("merged in");
        assert_eq!(
            before, merged,
            "the cells' order does not depend on their arrival's"
        );
        assert_eq!(
            SkyBlock::with_cell(Some(&merged), cell, &params, &records),
            None
        );
        let nudged = BlockParams::new(position(SUN), Magnitudes::new(6.0_f64.next_up()));
        let replaced =
            SkyBlock::with_cell(Some(&merged), unbuilt, &nudged, &other).expect("a new block");
        assert_eq!(
            (replaced.built_cells(), replaced.cell(cell)),
            (1, None),
            "a block of other parameters replaces it"
        );
        assert!(nudged.same_bits(replaced.params()));
        let third = CellKey::new(Layer::C, [2, 812, 0]).expect("in the cube");
        let many = vec![records[0]; usize::from(u16::MAX) + 1];
        assert_eq!(
            SkyBlock::with_cell(Some(&merged), third, &params, &many),
            None
        );
        // An empty cell is held: its records failed their bounds, which are not taken again.
        let empty = SkyBlock::with_cell(
            Some(&merged),
            CellKey::new(Layer::C, [3, 815, 3]).expect("in the cube"),
            &params,
            &[],
        )
        .expect("merged in");
        assert_eq!(empty.built_cells(), 3);
    }

    /// The census of each query is the one no cache gives, stars and tallies bit for bit, through a
    /// warm cache built by a looser query, by a tighter one, and before a move or another time; a
    /// jump the entries hold rebuilds no cell, and one beyond them rebuilds the cells it nears.
    #[test]
    fn a_query_after_a_looser_or_a_tighter_one_gives_the_bits_no_cache_gives() {
        let galaxy = milky_way_galaxy();
        let cells = cells();
        let n = cells.len();
        let q = queries();
        let expected = |query: &SkyQuery| uncached(&cells, query);
        let (loose, tight) = (expected(&q.loose), expected(&q.tight));
        assert_lists_stars(&loose, "loose");
        assert_lists_stars(&tight, "tight");

        // Looser, then tighter: every cell is served, and the served census bounds fewer records
        // star by star than the census with no cache.
        let cache = KeepBlocks::roomy(galaxy);
        let mut ctx = context(&cache);
        assert_same_bits(&census(&cells, &q.loose, &mut ctx), &loose, "loose, cold");
        let cold = cache.take_counts();
        assert_eq!(
            (cold.missed, cold.served, cold.rebuilt()),
            (n, 0, 0),
            "{cold:?}"
        );
        let (warm_parts, warm_cost) = census_in(galaxy, &cells, &q.tight, &mut ctx);
        assert_same_bits(&warm_parts, &tight, "tight after loose");
        let warm = cache.take_counts();
        assert_eq!(
            (warm.served, warm.missed, warm.rebuilt()),
            (n, 0, 0),
            "{warm:?}"
        );
        let (_, cold_cost) = census_in(galaxy, &cells, &q.tight, &mut context(&NoSkyCellCache));
        let bounded = |cost: &CensusCost| total(cost, LayerCost::star_bounded);
        assert!(
            bounded(&warm_cost) < bounded(&cold_cost),
            "served {} against cold {}",
            bounded(&warm_cost),
            bounded(&cold_cost)
        );
        assert!(total(&warm_cost, LayerCost::prefiltered) > 0);
        assert_eq!(
            total(&warm_cost, LayerCost::served),
            u64::try_from(n).expect("few")
        );

        // Tighter, then looser: the cells whose key the looser query passes are rebuilt, and then
        // serve the tighter query again.
        let cache = KeepBlocks::roomy(galaxy);
        let mut ctx = context(&cache);
        assert_same_bits(&census(&cells, &q.tight, &mut ctx), &tight, "tight, cold");
        let _ = cache.take_counts();
        assert_same_bits(
            &census(&cells, &q.loose, &mut ctx),
            &loose,
            "loose after tight",
        );
        let looser = cache.take_counts();
        assert!(looser.key > 0, "the loose keys rebuild: {looser:?}");
        assert_same_bits(
            &census(&cells, &q.tight, &mut ctx),
            &tight,
            "tight after tight and loose",
        );
        assert_eq!(cache.take_counts().rebuilt(), 0);

        // A jump of 1,000 ly either way, and the same place at either end of the clock window,
        // are served whole from the entries the tight census left; a jump of 1,500 ly nears the
        // far cells past their keys, which rebuild.
        let cache = KeepBlocks::roomy(galaxy);
        let mut ctx = context(&cache);
        let _ = census(&cells, &q.tight, &mut ctx);
        let _ = cache.take_counts();
        for (query, what) in [
            (&q.toward, "1,000 ly toward"),
            (&q.away, "1,000 ly away"),
            (&q.late, "at +H"),
            (&q.early, "at -H"),
        ] {
            assert_same_bits(&census(&cells, query, &mut ctx), &expected(query), what);
            let counts = cache.take_counts();
            assert_eq!(
                (counts.served, counts.missed, counts.rebuilt()),
                (n, 0, 0),
                "{what}: {counts:?}"
            );
        }
        assert_same_bits(
            &census(&cells, &q.far, &mut ctx),
            &expected(&q.far),
            "1,500 ly toward",
        );
        let beyond = cache.take_counts();
        assert!(
            beyond.key > 0 && beyond.served > 0,
            "the near cells rebuild and the others serve: {beyond:?}"
        );
    }

    /// A planted block whose cell holds another layer's records is never read for a fainter key
    /// or a window it does not hold, where the cell is rebuilt at the query's parameters and
    /// replaces it, and is read where the rule allows.
    #[test]
    fn no_entry_is_read_for_a_fainter_key_or_a_window_it_does_not_hold() {
        let galaxy = milky_way_galaxy();
        let key = CellKey::containing(Layer::C, &position(SUN)).expect("in the cube");
        let mut poison = Vec::new();
        generate_cell(
            galaxy,
            CellKey::containing(Layer::E, &position(SUN)).expect("in the cube"),
            &mut poison,
        );
        let poison: Vec<HeldRecord> = poison
            .iter()
            .map(|r| HeldRecord::new(*r, RecordLight::Unbounded))
            .collect();
        assert!(!poison.is_empty());
        let at = BlockParams::new(position(SUN), Magnitudes::new(6.0));
        let planted = |params: &BlockParams| {
            SkyBlock::with_cell(None, key, params, &poison).expect("a new block")
        };
        let cache = KeepBlocks::roomy(galaxy);
        let fresh = |query: &SkyQuery| census_of(key, query, &mut context(&NoSkyCellCache));
        let nan = BlockParams::new(position(SUN), Magnitudes::new(f64::NAN));
        // Refused: a deeper cut at the cell's own place (its key), the cell from 3,000 ly away
        // (its window), and an entry built at a NaN cut.
        for (params, query, why) in [
            (at, query_at(SUN, 9.0), "the key"),
            (at, query_at(along(3_000.0), 6.0), "the window"),
            (nan, query_at(SUN, 6.0), "a NaN key"),
        ] {
            cache.plant(planted(&params));
            let got = census_of(key, &query, &mut context(&cache));
            assert_eq!(got, fresh(&query), "rebuilt for {why}");
            let counts = cache.take_counts();
            let expected = if why == "the window" { (0, 1) } else { (1, 0) };
            assert_eq!((counts.key, counts.window), expected, "{why}: {counts:?}");
            let block = cache.held(BlockKey::of(key)).expect("rebuilt and kept");
            assert!(
                block.params().same_bits(&BlockParams::of(&query)),
                "{why}: the rebuilt cell replaces the block"
            );
            assert!(
                block
                    .cell(key)
                    .expect("built")
                    .iter()
                    .all(|h| CellKey::of(h.record().id()) == Ok(key)),
                "{why}: the cell's own records, none of the planted block's"
            );
        }
        // A block holding only a neighbouring cell, built from 3,000 ly away, whose parameters
        // cannot serve the cell's query: the cell is rebuilt at the query's, which replace it.
        let [x, y, z] = key.gen_cell().to_array();
        let neighbour = CellKey::new(Layer::C, [x ^ 1, y, z]).expect("in the cube");
        assert_eq!(BlockKey::of(neighbour), BlockKey::of(key));
        let far = BlockParams::new(position(along(3_000.0)), Magnitudes::new(6.0));
        cache.plant(SkyBlock::with_cell(None, neighbour, &far, &[]).expect("a new block"));
        let query = query_at(SUN, 6.0);
        assert_eq!(census_of(key, &query, &mut context(&cache)), fresh(&query));
        let counts = cache.take_counts();
        assert_eq!(
            (counts.parameters, counts.rebuilt()),
            (1, 1),
            "the block's parameters: {counts:?}"
        );
        let block = cache.held(BlockKey::of(key)).expect("rebuilt and kept");
        assert!(block.params().same_bits(&BlockParams::of(&query)));
        assert_eq!(
            (block.built_cells(), block.cell(neighbour)),
            (1, None),
            "the neighbour went with the block it replaced"
        );
        // Read: the same query, one 1,000 ly away, and one at +H, each holding the planted key
        // and window, read the planted records, which a census of the cell itself never lists.
        for query in [
            query_at(SUN, 6.0),
            query_at(along(-1_000.0), 6.0),
            query_at_time(SUN, ClockWindow::END, 6.0),
        ] {
            cache.plant(planted(&at));
            let got = census_of(key, &query, &mut context(&cache));
            assert_eq!(cache.take_counts().served, 1, "{query:?}");
            assert_ne!(
                got,
                fresh(&query),
                "the planted records are read: {query:?}"
            );
            assert!(
                got.1.layer(Layer::E).generated() > 0,
                "the planted E records are censused"
            );
            let block = cache.held(BlockKey::of(key)).expect("held");
            assert_eq!(block.cell(key), Some(&poison[..]), "a read keeps the block");
        }
    }

    /// In a galaxy whose systems move, a census at the Sun 900 years after the epoch, through a
    /// cache built at the epoch from 100 ly away, gives the stars and tallies no cache gives, and is
    /// served whole.
    #[test]
    fn a_cache_built_elsewhere_and_earlier_serves_a_moving_galaxy_unchanged() {
        let galaxy = moving_galaxy();
        let mut cells = Vec::new();
        for layer in [
            Layer::A,
            Layer::B,
            Layer::C,
            Layer::D,
            Layer::E,
            Layer::BrownDwarf,
        ] {
            let size = f64::from(layer.cell_size_ly());
            for step in [-1.0, 0.0, 1.0, 3.0] {
                let p = along(step * size);
                cells.push(CellKey::containing(layer, &position(p)).expect("in the cube"));
            }
        }
        let built = query_at(along(100.0), 7.95);
        let later = UniverseTime::from_julian_years(900).expect("on the clock");
        let read = query_at_time(SUN, later, 7.95);
        let cache = KeepBlocks::roomy(galaxy);
        let _ = census_in(galaxy, &cells, &built, &mut context(&cache));
        let _ = cache.take_counts();
        let (got, _) = census_in(galaxy, &cells, &read, &mut context(&cache));
        let (expected, _) = census_in(galaxy, &cells, &read, &mut context(&NoSkyCellCache));
        assert!(
            expected.iter().any(|(s, _)| !s.is_empty()),
            "the census lists stars"
        );
        assert_same_bits(&got, &expected, "900 years later, 100 ly away");
        let counts = cache.take_counts();
        assert_eq!(
            (counts.served, counts.rebuilt()),
            (cells.len(), 0),
            "{counts:?}"
        );
    }

    /// The cache keeps within its blocks and its bytes, evicting the least recently used and
    /// keeping no block heavier than its whole bound, and no eviction changes a reply.
    #[test]
    fn the_cache_keeps_within_its_bound_and_evicting_changes_no_reply() {
        let galaxy = milky_way_galaxy();
        let cells = cells();
        let q = queries();
        let asked = [&q.loose, &q.tight, &q.far];
        let expected: Vec<Vec<Part>> = asked.iter().map(|query| uncached(&cells, query)).collect();
        assert_lists_stars(&expected[0], "loose");
        // Weigh the loose query's blocks, then bound a cache to a third of them in bytes and to
        // three blocks.
        let weigh = KeepBlocks::roomy(galaxy);
        let _ = census(&cells, &q.loose, &mut context(&weigh));
        let (blocks, total_bytes) = weigh.assert_within_bound();
        assert!(blocks > 6, "{blocks} blocks");
        let heaviest = {
            let kept = weigh.lock();
            kept.blocks.values().map(|e| e.bytes).max().expect("blocks")
        };
        let max_bytes = (total_bytes / 3).max(heaviest);
        assert!(
            max_bytes < total_bytes,
            "{max_bytes} of {total_bytes} bytes"
        );
        // Each bound alone. Each cell is asked for every query in turn, so that its block serves
        // the next query, and the blocks push each other out.
        for (max_blocks, max_bytes) in [(3, usize::MAX), (usize::MAX, max_bytes)] {
            let bounded = KeepBlocks::new(galaxy, max_blocks, max_bytes);
            let mut ctx = context(&bounded);
            let mut got: [Vec<Part>; 3] = Default::default();
            for &key in &cells {
                for (i, query) in asked.iter().enumerate() {
                    got[i].push(census_of(key, query, &mut ctx));
                    bounded.assert_within_bound();
                }
            }
            for (i, parts) in got.iter().enumerate() {
                assert_same_bits(parts, &expected[i], "through the bounded cache");
            }
            let counts = bounded.take_counts();
            assert!(
                counts.evicted > 0 && counts.served > 0,
                "the bound of {max_blocks} blocks and {max_bytes} bytes bites and the cache \
                 serves: {counts:?}"
            );
        }

        // A block heavier than the whole bound is not kept, and its cell is censused unchanged:
        // the cell holding the most records, alone in its block.
        let (key, most) = cells
            .iter()
            .map(|&c| {
                let held = weigh.held(BlockKey::of(c)).expect("weighed");
                (c, held.cell(c).expect("built").len())
            })
            .max_by_key(|&(_, n)| n)
            .expect("cells");
        assert!(most > 0);
        let alone = most * size_of::<HeldRecord>() + ENTRY_OVERHEAD_BYTES;
        let light = KeepBlocks::new(galaxy, usize::MAX, alone - 1);
        let mut ctx = context(&light);
        for _ in 0..2 {
            assert_eq!(
                census_of(key, &q.loose, &mut ctx),
                census_of(key, &q.loose, &mut context(&NoSkyCellCache))
            );
        }
        assert_eq!(light.assert_within_bound(), (0, 0));
        assert_eq!(light.take_counts().missed, 2);
    }

    /// Through one cache shared by every job, each cell's part does not depend on what was asked
    /// before it, even as the cache evicts and rebuilds.
    #[test]
    fn a_shared_cache_is_order_independent() {
        let galaxy = milky_way_galaxy();
        let cells = cells();
        let q = queries();
        let asked = [&q.loose, &q.tight, &q.far];
        let expected: Vec<Vec<Part>> = asked.iter().map(|query| uncached(&cells, query)).collect();
        assert_lists_stars(&expected[0], "loose");
        // Cell by cell, so that forwards each query follows a looser one and backwards a tighter
        // one.
        let keys: Vec<(usize, usize)> = (0..cells.len())
            .flat_map(|c| (0..asked.len()).map(move |i| (i, c)))
            .collect();
        let shared = KeepBlocks::new(galaxy, 6, usize::MAX);
        let ctx = RefCell::new(context(&shared));
        assert_order_independent(&keys, |&(i, c)| {
            let part = census_of(cells[c], asked[i], &mut ctx.borrow_mut());
            assert_eq!(part, expected[i][c], "query {i}, cell {c}");
            part
        });
        let counts = shared.take_counts();
        assert!(
            counts.evicted > 0 && counts.rebuilt() > 0 && counts.served > 0,
            "{counts:?}"
        );
    }

    /// Two threads censusing the cells for different queries through one cache, side by side,
    /// get the bits no cache gives; and two threads filling one block's cells at one query's
    /// parameters both land in it, which a cell built at another query's then replaces. Not on
    /// wasm32-wasip1, which has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn a_cache_shared_between_threads_gives_the_bits_no_cache_gives() {
        let galaxy = milky_way_galaxy();
        let cells = cells();
        let q = queries();
        let asked = [&q.loose, &q.tight, &q.toward];
        let expected: Vec<Vec<Part>> = asked.iter().map(|query| uncached(&cells, query)).collect();
        assert_lists_stars(&expected[0], "loose");
        let shared = KeepBlocks::roomy(galaxy);
        let parts: Vec<[Vec<Part>; 2]> = std::thread::scope(|scope| {
            let jobs: Vec<_> = [[0, 1], [1, 2]]
                .into_iter()
                .enumerate()
                .map(|(job, order)| {
                    let (cells, asked, shared) = (&cells, &asked, &shared);
                    scope.spawn(move || {
                        let mut ctx = context(shared);
                        let mut mine = cells.clone();
                        if job == 1 {
                            mine.reverse();
                        }
                        order.map(|i| {
                            let mut parts = census(&mine, asked[i], &mut ctx);
                            if job == 1 {
                                parts.reverse();
                            }
                            parts
                        })
                    })
                })
                .collect();
            jobs.into_iter()
                .map(|job| job.join().expect("a census job completes"))
                .collect()
        });
        assert_same_bits(&parts[0][0], &expected[0], "job 0, loose");
        assert_same_bits(&parts[0][1], &expected[1], "job 0, tight");
        assert_same_bits(&parts[1][0], &expected[1], "job 1, tight");
        assert_same_bits(&parts[1][1], &expected[2], "job 1, toward");

        // One block's four cells along x, two to each thread, at the tight query's parameters.
        let first = CellKey::containing(Layer::C, &position(SUN)).expect("in the cube");
        let [x, y, z] = first.gen_cell().to_array();
        let x0 = x.div_euclid(BLOCK_SIDE_CELLS) * BLOCK_SIDE_CELLS;
        let row: Vec<CellKey> = (0..BLOCK_SIDE_CELLS)
            .map(|i| CellKey::new(Layer::C, [x0 + i, y, z]).expect("in the cube"))
            .collect();
        let block_key = BlockKey::of(row[0]);
        assert!(row.iter().all(|&c| BlockKey::of(c) == block_key));
        let alone: Vec<Vec<HeldRecord>> = row
            .iter()
            .map(|&c| {
                let one = KeepBlocks::roomy(galaxy);
                let _ = census_of(c, &q.tight, &mut context(&one));
                one.held(block_key)
                    .and_then(|b| b.cell(c).map(<[HeldRecord]>::to_vec))
                    .expect("built")
            })
            .collect();
        let filled = KeepBlocks::roomy(galaxy);
        std::thread::scope(|scope| {
            for half in [[0, 2], [1, 3]] {
                let (row, filled, tight) = (&row, &filled, &q.tight);
                scope.spawn(move || {
                    let mut ctx = context(filled);
                    for i in half {
                        let _ = census_of(row[i], tight, &mut ctx);
                    }
                });
            }
        });
        let block = filled.held(block_key).expect("filled");
        assert_eq!(block.built_cells(), 4, "both threads' cells landed");
        for (c, own) in row.iter().zip(&alone) {
            assert_eq!(block.cell(*c), Some(&own[..]), "{c:?}");
        }
        // The Sun's own cell, whose key the deeper cut passes.
        let _ = census_of(first, &q.loose, &mut context(&filled));
        let replaced = filled.held(block_key).expect("rebuilt");
        assert!(replaced.params().same_bits(&BlockParams::of(&q.loose)));
        assert_eq!(
            replaced.built_cells(),
            1,
            "another query's parameters replace the block"
        );
    }
}
