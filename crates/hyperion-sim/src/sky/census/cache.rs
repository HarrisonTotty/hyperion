//! The census's per-cell cache: each cell's systems at or above a mass floor (rendering plan R06,
//! R06.T8.a and T8.d; Design note 12).
//!
//! The sim holds no cache; the server's is a byte-bounded LRU behind a lock (R06.T11.b). Whatever
//! holds one must keep it **monotone**: a cell's entry holds the records at or above the floor it
//! was built with, in candidate order, and serves a later query only if that query's floor is at or
//! above it, filtering the entry; a lower floor rebuilds the cell. So the cache never changes a
//! reply. [`serve_from_entry`] is that rule, written once for every implementation.
//!
//! Records are epoch state, so an entry outlives the query that built it: a jump of up to
//! 1,000 ly moves each cell's floor, up where the observer leaves it and down where it nears it,
//! and the rule serves the first and rebuilds the second. The tests check, through a bounded cache
//! of their own built on the rule, as the server's will be, that a query after a looser one, after
//! a tighter one and after a move gives the bits no cache gives, and that no entry is ever read
//! for a floor below its own.

#[cfg(doc)]
use crate::galaxy::placement::{CellCache, cell_heap_bytes};

use crate::galaxy::Galaxy;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell_where};
use crate::units::SolarMasses;

/// Where the census gets a cell's bright subset: every record of the cell whose primary's initial
/// mass is at least `floor`, in candidate order, as [`generate_cell_where`] makes it.
///
/// It takes `&self` so that the census's parallel jobs share one; an implementation that keeps
/// entries uses interior mutability behind a lock. Such an implementation:
///
/// - serves an entry only through [`serve_from_entry`], and keeps a rebuilt cell's records at the
///   floor it was rebuilt for (never at a NaN floor, where it holds no record);
/// - keys its entries by galaxy as well as by cell, as a [`CellCache`] does, since a [`CellKey`]
///   names the same cell in every galaxy;
/// - is bounded in bytes, an entry weighing at least its records ([`cell_heap_bytes`]): the
///   server's is a `ByteLru` under `HYPERION_SKY_CACHE_MB` (R06.T11.b). Eviction is always safe,
///   since an evicted cell is rebuilt.
///
/// # Examples
///
/// A cache of the last cell it built, keyed by the galaxy's seed too (every galaxy it serves is
/// [`Galaxy::new`] of its seed). It serves through the rule, and builds a cell outside its lock
/// and keeps it at the floor it was built for:
///
/// ```
/// use std::sync::Mutex;
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, SystemRecord};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::sky::census::{NoSkyCellCache, Served, SkyCellCache, serve_from_entry};
/// use hyperion_sim::units::SolarMasses;
///
/// #[derive(Debug, Default)]
/// struct LastBright {
///     held: Mutex<Option<(Seed, CellKey, SolarMasses, Vec<SystemRecord>)>>,
/// }
///
/// impl SkyCellCache for LastBright {
///     fn bright_subset(
///         &self,
///         galaxy: &Galaxy,
///         key: CellKey,
///         floor: SolarMasses,
///         out: &mut Vec<SystemRecord>,
///     ) {
///         {
///             let held = self.held.lock().expect("not poisoned");
///             if let Some((seed, held_key, at, records)) = &*held
///                 && *seed == galaxy.seed()
///                 && *held_key == key
///                 && serve_from_entry(*at, records, floor, out) == Served::Served
///             {
///                 return;
///             }
///         }
///         NoSkyCellCache.bright_subset(galaxy, key, floor, out);
///         let entry = (galaxy.seed(), key, floor, out.clone());
///         *self.held.lock().expect("not poisoned") = Some(entry);
///     }
/// }
///
/// let galaxy = Galaxy::new(Seed::new(19));
/// let key = CellKey::new(Layer::E, [0, 203, 0])?;
/// let cache = LastBright::default();
/// let (mut warm, mut cold) = (Vec::new(), Vec::new());
/// // Built at 12 M☉; 20 is served from it, filtered; 8 rebuilds the cell, which then serves 9.
/// for floor in [12.0, 20.0, 8.0, 9.0] {
///     let floor = SolarMasses::new(floor);
///     cache.bright_subset(&galaxy, key, floor, &mut warm);
///     NoSkyCellCache.bright_subset(&galaxy, key, floor, &mut cold);
///     assert_eq!(warm, cold, "the cache never changes a reply");
/// }
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
pub trait SkyCellCache: Sync {
    /// Writes the records of `key` at or above `floor` to `out` (cleared first).
    fn bright_subset(
        &self,
        galaxy: &Galaxy,
        key: CellKey,
        floor: SolarMasses,
        out: &mut Vec<SystemRecord>,
    );
}

/// No cache: every cell is generated afresh.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NoSkyCellCache;

impl SkyCellCache for NoSkyCellCache {
    fn bright_subset(
        &self,
        galaxy: &Galaxy,
        key: CellKey,
        floor: SolarMasses,
        out: &mut Vec<SystemRecord>,
    ) {
        generate_cell_where(galaxy, key, |m| m.value() >= floor.value(), out);
    }
}

/// What [`serve_from_entry`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use = "a refused entry leaves `out` untouched: the cell must be rebuilt"]
pub enum Served {
    /// `out` holds the entry's records at or above the floor.
    Served,
    /// The floor is below the entry's, or either is NaN: `out` is untouched, and the cell must be
    /// rebuilt.
    Rebuild,
}

/// Serves a cell at `floor` from an entry built at `held` with `records`, if the rule allows:
/// [`Served::Served`] with `out` filled when `floor` is at or above `held`, [`Served::Rebuild`]
/// (and `out` untouched) when the cell must be rebuilt. The one place the monotone rule is written,
/// for every implementation.
///
/// A NaN on either side refuses: an entry built at a NaN floor holds no record, so it must never
/// serve a finite floor.
pub fn serve_from_entry(
    held: SolarMasses,
    records: &[SystemRecord],
    floor: SolarMasses,
    out: &mut Vec<SystemRecord>,
) -> Served {
    // Not `floor < held`, which a NaN on either side would pass.
    let at_or_above = floor.value() >= held.value();
    if !at_or_above {
        return Served::Rebuild;
    }
    out.clear();
    out.extend(
        records
            .iter()
            .filter(|r| r.primary_initial_mass().value() >= floor.value())
            .copied(),
    );
    Served::Served
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::collections::btree_map::Entry as Slot;
    use std::sync::{Mutex, MutexGuard};

    use hyperion_testkit::float;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::imf::MassBand;
    use crate::galaxy::placement::generate_cell;
    use crate::id::Layer;
    use crate::observe::Observer;
    use crate::sky::census::cell::{CensusTallies, SkyStar, census_cell};
    use crate::sky::census::query::{SkyContext, SkyQuery};
    use crate::sky::eye::EyeObserver;
    use crate::sky::testing::{milky_way_dark_tables, milky_way_envelope, milky_way_offsets};
    use crate::time::UniverseTime;
    use crate::units::Magnitudes;

    /// The Sun's place in the fixture, ly.
    const SUN: [f64; 3] = [0.0, 26_000.0, 68.0];

    /// One cell's census: its stars and its tallies.
    type Part = (Vec<SkyStar>, CensusTallies);

    /// What the test cache did since its counts were last taken.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    struct Counts {
        /// Lookups of a cell with no entry.
        missed: usize,
        /// Lookups served from an entry.
        served: usize,
        /// Of those, the ones whose floor dropped some of the entry's records.
        filtered: usize,
        /// Lookups whose floor was below the entry's, which rebuilt the cell.
        rebuilt: usize,
        /// Entries evicted to keep within the bound.
        evicted: usize,
    }

    /// One cell's entry.
    #[derive(Debug)]
    struct Entry {
        /// The floor it was built at.
        held: SolarMasses,
        /// The cell's records at or above `held`, in candidate order.
        records: Vec<SystemRecord>,
        /// Its weight against the bound.
        bytes: usize,
        /// The cache's clock when it was last kept or served.
        used: u64,
    }

    /// What an entry costs beyond its records' heap: its key and itself.
    const ENTRY_OVERHEAD_BYTES: usize = size_of::<CellKey>() + size_of::<Entry>();

    /// What the lock guards.
    #[derive(Debug, Default)]
    struct Kept {
        entries: BTreeMap<CellKey, Entry>,
        bytes: usize,
        clock: u64,
        counts: Counts,
    }

    /// A per-cell cache built on [`serve_from_entry`], as the server's will be (R06.T11.b): one
    /// galaxy's entries behind a lock, shared through `&self`, evicting the least recently used.
    ///
    /// **Memory bound:** at most `max_entries` entries, together weighing at most `max_bytes`
    /// bytes, an entry weighing its records' heap (their capacity × the record's size, which is
    /// [`cell_heap_bytes`] once shrunk to fit) plus [`ENTRY_OVERHEAD_BYTES`]. The map's own nodes
    /// and the fixed-size [`Counts`] are not counted. An entry heavier than `max_bytes` alone is
    /// served and not kept.
    #[derive(Debug)]
    struct KeepBright<'g> {
        galaxy: &'g Galaxy,
        max_entries: usize,
        max_bytes: usize,
        kept: Mutex<Kept>,
    }

    impl<'g> KeepBright<'g> {
        fn new(galaxy: &'g Galaxy, max_entries: usize, max_bytes: usize) -> Self {
            Self {
                galaxy,
                max_entries,
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

        /// Asserts the entries and bytes held within the bound and the bytes the entries' own
        /// weights, and returns the entries and bytes held.
        fn assert_within_bound(&self) -> (usize, usize) {
            let (entries, bytes, weights) = {
                let kept = self.lock();
                let weights: usize = kept.entries.values().map(|e| e.bytes).sum();
                (kept.entries.len(), kept.bytes, weights)
            };
            assert_eq!(weights, bytes, "the bytes held are the entries' weights");
            assert!(entries <= self.max_entries, "{entries} entries");
            assert!(bytes <= self.max_bytes, "{bytes} bytes");
            (entries, bytes)
        }

        /// The floor `key`'s entry was built at, if it has one.
        fn floor_of(&self, key: CellKey) -> Option<SolarMasses> {
            self.lock().entries.get(&key).map(|e| e.held)
        }

        /// The cells held.
        fn keys(&self) -> Vec<CellKey> {
            self.lock().entries.keys().copied().collect()
        }

        /// Keeps `records` as `key`'s entry at `held`, unless it already has one at or below
        /// `held`, which serves at least as much. A cell built at a NaN floor holds no record and
        /// would serve nothing, so it is not kept.
        fn keep(&self, key: CellKey, held: SolarMasses, records: Vec<SystemRecord>) {
            if held.value().is_nan() {
                return;
            }
            self.store(key, held, records, |old| {
                old.value().is_nan() || held.value() < old.value()
            });
        }

        /// Replaces `key`'s entry with `records` at `held`, whatever it held: a planted entry.
        fn plant(&self, key: CellKey, held: SolarMasses, records: Vec<SystemRecord>) {
            self.store(key, held, records, |_| true);
        }

        /// Stores `records` as `key`'s entry at `held` if it has none or `replaces` its floor, then
        /// evicts the least recently used until within the bound.
        fn store(
            &self,
            key: CellKey,
            held: SolarMasses,
            records: Vec<SystemRecord>,
            replaces: impl Fn(SolarMasses) -> bool,
        ) {
            let bytes = records
                .capacity()
                .saturating_mul(size_of::<SystemRecord>())
                .saturating_add(ENTRY_OVERHEAD_BYTES);
            if bytes > self.max_bytes || self.max_entries == 0 {
                return;
            }
            let mut guard = self.lock();
            let kept = &mut *guard;
            kept.clock += 1;
            let entry = Entry {
                held,
                records,
                bytes,
                used: kept.clock,
            };
            match kept.entries.entry(key) {
                Slot::Occupied(slot) if !replaces(slot.get().held) => return,
                Slot::Occupied(mut slot) => {
                    kept.bytes -= slot.get().bytes;
                    slot.insert(entry);
                }
                Slot::Vacant(slot) => {
                    slot.insert(entry);
                }
            }
            kept.bytes += bytes;
            // The entry just stored is the most recent and within the bound alone, so it stays.
            while kept.entries.len() > self.max_entries || kept.bytes > self.max_bytes {
                let oldest = kept
                    .entries
                    .iter()
                    .min_by_key(|(_, e)| e.used)
                    .map(|(&k, _)| k)
                    .expect("over the bound, so not empty");
                let gone = kept.entries.remove(&oldest).expect("just found");
                kept.bytes -= gone.bytes;
                kept.counts.evicted += 1;
            }
        }
    }

    impl SkyCellCache for KeepBright<'_> {
        fn bright_subset(
            &self,
            galaxy: &Galaxy,
            key: CellKey,
            floor: SolarMasses,
            out: &mut Vec<SystemRecord>,
        ) {
            assert!(
                std::ptr::eq(galaxy, self.galaxy),
                "a cell cache is for the galaxy it was made for"
            );
            {
                let mut guard = self.lock();
                let kept = &mut *guard;
                kept.clock += 1;
                match kept.entries.get_mut(&key) {
                    None => kept.counts.missed += 1,
                    Some(entry) => match serve_from_entry(entry.held, &entry.records, floor, out) {
                        Served::Served => {
                            entry.used = kept.clock;
                            kept.counts.served += 1;
                            if out.len() < entry.records.len() {
                                kept.counts.filtered += 1;
                            }
                            return;
                        }
                        Served::Rebuild => kept.counts.rebuilt += 1,
                    },
                }
            }
            // Built outside the lock, so that the census's jobs build their cells side by side.
            NoSkyCellCache.bright_subset(galaxy, key, floor, out);
            self.keep(key, floor, out.clone());
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

    fn query_at(at: [f64; 3], cut: f64, eye: bool) -> SkyQuery {
        query_at_time(at, UniverseTime::EPOCH, cut, eye)
    }

    fn query_at_time(at: [f64; 3], t: UniverseTime, cut: f64, eye: bool) -> SkyQuery {
        let observer = Observer::new(position(at), t).expect("an observer");
        let mut builder = SkyQuery::builder(observer, Magnitudes::new(cut));
        if eye {
            builder = builder.eye(EyeObserver::default());
        }
        builder.build().expect("a valid query")
    }

    /// A looser query (deeper, and a 60° cone's along +x, which keeps only the stars of its
    /// region's texels but opens the cells of the same floors; the eye, which cannot ask a cone,
    /// keeps a star to the cut alone as any query does since R06.T8.k), a tighter one, and the
    /// tighter from 200 ly along x: every floor of the tighter is at or above the looser's, and the
    /// move lowers the floors of the cells it nears and raises the others'.
    fn queries() -> [SkyQuery; 3] {
        let cone = crate::sky::census::Cone::new(
            crate::coords::UnitVector::X,
            crate::units::Degrees::new(60.0),
        )
        .expect("a cone");
        let loose = SkyQuery::builder(
            Observer::new(position(SUN), UniverseTime::EPOCH).expect("an observer"),
            Magnitudes::new(9.0),
        )
        .cone(cone)
        .build()
        .expect("a valid query");
        [
            loose,
            query_at(SUN, 6.0, false),
            query_at([SUN[0] + 200.0, SUN[1], SUN[2]], 6.0, false),
        ]
    }

    /// Cells along x through the Sun: of layers A to C from 800 ly on one side to 800 ly on the
    /// other, and of D and E at the Sun and 200 ly along.
    ///
    /// Near the Sun every floor of C to E is its band's lower edge, the envelope allowing a giant
    /// of the band's least mass at some age, so the queries' floors part only in A from about
    /// 100 ly, in B from about 200 ly and in C from about 400 ly (measured 2026-10-04 on the
    /// fixture): there the tighter query filters what the looser kept, and the looser rebuilds
    /// what the tighter kept.
    fn cells() -> Vec<CellKey> {
        let wide: &[f64] = &[-800.0, -200.0, -100.0, 0.0, 100.0, 200.0, 800.0];
        let near: &[f64] = &[0.0, 200.0];
        let mut cells = Vec::new();
        for (layer, along) in [
            (Layer::A, wide),
            (Layer::B, wide),
            (Layer::C, wide),
            (Layer::D, near),
            (Layer::E, near),
        ] {
            for dx in along {
                let p = [SUN[0] + dx, SUN[1], SUN[2]];
                cells.push(CellKey::containing(layer, &position(p)).expect("in the cube"));
            }
        }
        cells
    }

    fn census_of(key: CellKey, query: &SkyQuery, ctx: &mut SkyContext<'_>) -> Part {
        let mut stars = Vec::new();
        let tallies = census_cell(milky_way_galaxy(), ctx, key, query, &mut stars);
        (stars, tallies)
    }

    fn census(cells: &[CellKey], query: &SkyQuery, ctx: &mut SkyContext<'_>) -> Vec<Part> {
        cells
            .iter()
            .map(|&key| census_of(key, query, ctx))
            .collect()
    }

    /// The census of `cells` for each query with no cache.
    fn uncached(cells: &[CellKey], queries: &[SkyQuery; 3]) -> [Vec<Part>; 3] {
        let parts = queries
            .each_ref()
            .map(|q| census(cells, q, &mut context(&NoSkyCellCache)));
        for (q, p) in parts.iter().enumerate() {
            assert!(
                p.iter().any(|(s, _)| !s.is_empty()),
                "query {q} lists stars"
            );
        }
        parts
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

    #[track_caller]
    fn assert_same_bits(got: &[Part], expected: &[Part], what: &str) {
        assert_eq!(got, expected, "{what}");
        assert_eq!(float_bits(got), float_bits(expected), "{what}");
    }

    #[test]
    fn the_monotone_rule_serves_at_or_above_the_floor_and_refuses_below() {
        let galaxy = milky_way_galaxy();
        let key = CellKey::new(Layer::C, [0, 26_000 / 32, 0]).expect("in the cube");
        let mut all = Vec::new();
        generate_cell(galaxy, key, &mut all);
        // Floors at records' own masses, where a strict filter would part from the cell's.
        let mut masses: Vec<f64> = all
            .iter()
            .map(|r| r.primary_initial_mass().value())
            .collect();
        masses.sort_by(f64::total_cmp);
        let at = |q: usize| masses[masses.len() * q / 4];
        let held = SolarMasses::new(at(1));
        let mut entry = Vec::new();
        NoSkyCellCache.bright_subset(galaxy, key, held, &mut entry);
        let mut expected = all.clone();
        expected.retain(|r| r.primary_initial_mass().value() >= held.value());
        assert_eq!(
            entry, expected,
            "the bright subset is the cell filtered by mass"
        );
        for floor in [at(1), at(2), at(3)] {
            let mut out = Vec::new();
            assert_eq!(
                serve_from_entry(held, &entry, SolarMasses::new(floor), &mut out),
                Served::Served
            );
            let mut fresh = Vec::new();
            NoSkyCellCache.bright_subset(galaxy, key, SolarMasses::new(floor), &mut fresh);
            assert_eq!(out, fresh, "floor {floor}");
            assert!(
                out.iter()
                    .any(|r| r.primary_initial_mass().value().total_cmp(&floor).is_eq()),
                "the record at the floor is kept"
            );
        }
        let nan = SolarMasses::new(f64::NAN);
        let below = SolarMasses::new(at(0));
        for (held, floor) in [(held, below), (held, nan), (nan, held)] {
            let mut out = vec![all[0]];
            assert_eq!(
                serve_from_entry(held, &entry, floor, &mut out),
                Served::Rebuild,
                "{held:?} {floor:?}"
            );
            assert_eq!(out, vec![all[0]], "a refusal leaves out untouched");
        }
    }

    /// The census of each query is the one no cache gives, bit for bit, through a warm cache built
    /// by a looser query, by a tighter one, and by both before a move.
    #[test]
    fn a_query_after_a_looser_or_a_tighter_one_gives_the_bits_no_cache_gives() {
        let galaxy = milky_way_galaxy();
        let cells = cells();
        let queries = queries();
        let [loose, tight, moved] = &queries;
        let expected = uncached(&cells, &queries);
        let n = cells.len();

        // Looser, then tighter: every floor rises or stays, so every lookup is served.
        let cache = KeepBright::roomy(galaxy);
        let mut ctx = context(&cache);
        assert_same_bits(
            &census(&cells, loose, &mut ctx),
            &expected[0],
            "loose, cold",
        );
        let cold = cache.take_counts();
        assert_eq!((cold.missed, cold.served), (n, 0), "{cold:?}");
        assert_same_bits(
            &census(&cells, tight, &mut ctx),
            &expected[1],
            "tight after loose",
        );
        let warm = cache.take_counts();
        assert_eq!(
            (warm.missed, warm.rebuilt, warm.served),
            (0, 0, n),
            "{warm:?}"
        );
        assert!(warm.filtered > 0, "the tight floors filter: {warm:?}");

        // Tighter, then looser: the lower floors rebuild their cells, which then serve the
        // tighter query again.
        let cache = KeepBright::roomy(galaxy);
        let mut ctx = context(&cache);
        assert_same_bits(
            &census(&cells, tight, &mut ctx),
            &expected[1],
            "tight, cold",
        );
        let _ = cache.take_counts();
        assert_same_bits(
            &census(&cells, loose, &mut ctx),
            &expected[0],
            "loose after tight",
        );
        let looser = cache.take_counts();
        assert_eq!(looser.missed, 0, "{looser:?}");
        assert!(looser.rebuilt > 0, "the loose floors rebuild: {looser:?}");
        assert_same_bits(
            &census(&cells, tight, &mut ctx),
            &expected[1],
            "tight after tight and loose",
        );

        // A move from the tighter query's place, at its cut, lowers the floors of the cells it
        // nears, which rebuild, and raises the others', which filter.
        let cache = KeepBright::roomy(galaxy);
        let mut ctx = context(&cache);
        let _ = census(&cells, tight, &mut ctx);
        let _ = cache.take_counts();
        assert_same_bits(
            &census(&cells, moved, &mut ctx),
            &expected[2],
            "moved after tight",
        );
        let after_move = cache.take_counts();
        assert!(
            after_move.rebuilt > 0 && after_move.filtered > 0,
            "the move both rebuilds and filters: {after_move:?}"
        );

        // The entries are epoch records, keyed by cell alone: a query 900 years before the epoch,
        // whose floors move with the motion pad, reads them too.
        let earlier = UniverseTime::from_julian_years(-900).expect("on the clock");
        let earlier = query_at_time(SUN, earlier, 6.0, false);
        let mut uncached_ctx = context(&NoSkyCellCache);
        assert_same_bits(
            &census(&cells, &earlier, &mut ctx),
            &census(&cells, &earlier, &mut uncached_ctx),
            "900 years earlier",
        );
        assert_eq!(cache.take_counts().missed, 0);
    }

    /// A planted entry whose records are another cell's is never read below its floor, where the
    /// cell is rebuilt and replaces it, and is read at or above it.
    #[test]
    fn no_entry_is_read_for_a_floor_below_its_own() {
        let galaxy = milky_way_galaxy();
        let key = CellKey::containing(Layer::C, &position(SUN)).expect("in the cube");
        let other = CellKey::containing(Layer::C, &position([SUN[0] + 32.0, SUN[1], SUN[2]]))
            .expect("in the cube");
        let mut whole = Vec::new();
        generate_cell(galaxy, key, &mut whole);
        let mut poison = Vec::new();
        generate_cell(galaxy, other, &mut poison);
        let mut masses: Vec<f64> = whole
            .iter()
            .map(|r| r.primary_initial_mass().value())
            .collect();
        masses.sort_by(f64::total_cmp);
        assert!(masses.len() >= 8 && !poison.is_empty(), "{}", masses.len());
        let quantile = |q: usize| masses[masses.len() * q / 4];
        let held = SolarMasses::new(quantile(2));
        let cache = KeepBright::roomy(galaxy);
        let fresh = |floor: SolarMasses| {
            let mut fresh = Vec::new();
            NoSkyCellCache.bright_subset(galaxy, key, floor, &mut fresh);
            fresh
        };
        let mut out = Vec::new();
        for floor in [MassBand::C.lo(), quantile(1), held.value() * (1.0 - 1e-12)] {
            let floor = SolarMasses::new(floor);
            cache.plant(key, held, poison.clone());
            cache.bright_subset(galaxy, key, floor, &mut out);
            assert_eq!(out, fresh(floor), "rebuilt at {floor:?}");
            assert!(
                !out.is_empty() && out.iter().all(|r| CellKey::of(r.id()) == Ok(key)),
                "the cell's own records, none of the planted entry's"
            );
            let counts = cache.take_counts();
            assert_eq!(
                counts,
                Counts {
                    rebuilt: 1,
                    ..Counts::default()
                }
            );
            assert_eq!(
                cache.floor_of(key),
                Some(floor),
                "the rebuilt cell replaces it"
            );
        }
        for floor in [held.value(), quantile(3), MassBand::C.hi()] {
            let floor = SolarMasses::new(floor);
            cache.plant(key, held, poison.clone());
            cache.bright_subset(galaxy, key, floor, &mut out);
            let mut read = poison.clone();
            read.retain(|r| r.primary_initial_mass().value() >= floor.value());
            assert_eq!(out, read, "read at {floor:?}");
            assert_eq!(cache.take_counts().served, 1);
            assert_eq!(cache.floor_of(key), Some(held), "a read keeps the entry");
        }
        // So a read of the planted entry is told apart from the cell's own records.
        cache.plant(key, held, poison.clone());
        cache.bright_subset(galaxy, key, held, &mut out);
        assert!(
            !out.is_empty(),
            "the planted entry has records at its floor"
        );
        assert_ne!(out, fresh(held));
        // An entry built at a NaN floor serves nothing, and a NaN floor reads no entry.
        let nan = SolarMasses::new(f64::NAN);
        for (planted, floor) in [(nan, held), (held, nan)] {
            cache.plant(key, planted, poison.clone());
            let _ = cache.take_counts();
            cache.bright_subset(galaxy, key, floor, &mut out);
            assert_eq!(out, fresh(floor), "{planted:?} {floor:?}");
            assert_eq!(cache.take_counts().rebuilt, 1);
            // The NaN entry is replaced, and a cell built at a NaN floor is not kept.
            assert_eq!(cache.floor_of(key), Some(held), "{planted:?} {floor:?}");
        }
    }

    /// The cache keeps within its entries and its bytes, evicting the least recently used and
    /// keeping no entry heavier than its whole bound, and no eviction changes a reply.
    #[test]
    fn the_cache_keeps_within_its_bound_and_evicting_changes_no_reply() {
        let galaxy = milky_way_galaxy();
        // The least recently used goes first.
        let e = |x: i32| CellKey::new(Layer::E, [x, 26_000 / 128, 0]).expect("in the cube");
        let floor = SolarMasses::new(MassBand::E.lo());
        let lru = KeepBright::new(galaxy, 3, usize::MAX);
        let mut out = Vec::new();
        for x in [0, 1, 2, 0, 3] {
            lru.bright_subset(galaxy, e(x), floor, &mut out);
            lru.assert_within_bound();
        }
        let mut survivors = vec![e(0), e(2), e(3)];
        survivors.sort();
        assert_eq!(lru.keys(), survivors, "e(1) was the least recent");
        assert_eq!(lru.take_counts().evicted, 1);

        // Weigh the loose query's entries, then bound a cache to a third of them in bytes and to
        // five entries.
        let cells = cells();
        let queries = queries();
        let expected = uncached(&cells, &queries);
        let weigh = KeepBright::roomy(galaxy);
        let _ = census(&cells, &queries[0], &mut context(&weigh));
        let (entries, total) = weigh.assert_within_bound();
        assert_eq!(entries, cells.len());
        let heaviest = {
            let kept = weigh.lock();
            kept.entries
                .iter()
                .max_by_key(|(_, e)| e.bytes)
                .map(|(&k, e)| (k, e.bytes))
                .expect("entries")
        };
        let max_bytes = (total / 3).max(heaviest.1);
        assert!(max_bytes < total, "{max_bytes} of {total} bytes");
        // Each bound alone: five entries, then a third of the bytes. Each cell is asked for
        // every query in turn, so that its entry serves the next query, and the cells' entries
        // push each other out.
        for (max_entries, max_bytes) in [(5, usize::MAX), (usize::MAX, max_bytes)] {
            let bounded = KeepBright::new(galaxy, max_entries, max_bytes);
            let mut ctx = context(&bounded);
            let mut got: [Vec<Part>; 3] = Default::default();
            for &key in &cells {
                for (q, query) in queries.iter().enumerate() {
                    got[q].push(census_of(key, query, &mut ctx));
                    bounded.assert_within_bound();
                }
            }
            for (q, parts) in got.iter().enumerate() {
                assert_same_bits(parts, &expected[q], "through the bounded cache");
            }
            let counts = bounded.take_counts();
            assert!(
                counts.evicted > 0 && counts.served > 0,
                "the bound of {max_entries} entries and {max_bytes} bytes bites and the cache \
                 serves: {counts:?}"
            );
        }

        // An entry heavier than the whole bound is served and not kept.
        let light = KeepBright::new(galaxy, usize::MAX, heaviest.1 - 1);
        let mut ctx = context(&light);
        for _ in 0..2 {
            assert_eq!(
                census_of(heaviest.0, &queries[0], &mut ctx),
                census_of(heaviest.0, &queries[0], &mut context(&NoSkyCellCache))
            );
        }
        assert_eq!(light.assert_within_bound(), (0, 0));
        assert_eq!(light.take_counts().missed, 2);
    }

    /// Through one cache shared by every job, each cell's part does not depend on what was asked
    /// before it, even as the cache evicts.
    #[test]
    fn a_shared_cache_is_order_independent() {
        let galaxy = milky_way_galaxy();
        let cells = cells();
        let queries = queries();
        let expected = uncached(&cells, &queries);
        // Cell by cell, so that forwards each query follows a looser one and backwards a
        // tighter one.
        let keys: Vec<(usize, usize)> = (0..cells.len())
            .flat_map(|c| (0..queries.len()).map(move |q| (q, c)))
            .collect();
        let shared = KeepBright::new(galaxy, 8, usize::MAX);
        let ctx = RefCell::new(context(&shared));
        assert_order_independent(&keys, |&(q, c)| {
            let part = census_of(cells[c], &queries[q], &mut ctx.borrow_mut());
            assert_eq!(part, expected[q][c], "query {q}, cell {c}");
            part
        });
        let counts = shared.take_counts();
        assert!(
            counts.evicted > 0 && counts.rebuilt > 0 && counts.filtered > 0,
            "{counts:?}"
        );
    }

    /// Two threads censusing the cells for different queries through one cache, side by side,
    /// get the bits no cache gives. Not on wasm32-wasip1, which has no threads; the other CI
    /// architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn a_cache_shared_between_threads_gives_the_bits_no_cache_gives() {
        let galaxy = milky_way_galaxy();
        let cells = cells();
        let queries = queries();
        let expected = uncached(&cells, &queries);
        let shared = KeepBright::roomy(galaxy);
        let parts: Vec<[Vec<Part>; 2]> = std::thread::scope(|scope| {
            let jobs: Vec<_> = [[0, 1], [1, 2]]
                .into_iter()
                .enumerate()
                .map(|(job, order)| {
                    let (cells, queries, shared) = (&cells, &queries, &shared);
                    scope.spawn(move || {
                        let mut ctx = context(shared);
                        let mut mine = cells.clone();
                        if job == 1 {
                            mine.reverse();
                        }
                        order.map(|q| {
                            let mut parts = census(&mine, &queries[q], &mut ctx);
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
        assert_same_bits(&parts[1][1], &expected[2], "job 1, moved");
    }
}
