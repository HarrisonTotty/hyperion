//! A range plan's answer in parts made apart from one another: the walk of some of its cells, and
//! the hits of its sources (P09.T40.a).
//!
//! [`RangePlan::run`] walks every planned cell and then asks every source. A server that generates
//! a cold query's cells on several threads can walk each cell there and then, while its records
//! are at hand ([`RangePlan::walk_cells`]), and ask the sources on another thread
//! ([`RangePlan::source_hits`]). [`RangePlan::finish`] then merges the parts into the answer `run`
//! gives, bit for bit: a cell's part is what the walk of that cell finds, in record order, so taken
//! in walk order the hits are collected in the order `run` collects them, before the sort.
//!
//! A part is taken only where it holds what the plan's own walk would find: a cell of a layer the
//! plan walks with the very sphere the part was walked with, met in walk order. Any other cell of a
//! part is passed over and walked again, so no choice of parts changes an answer, only how long it
//! takes. What `finish` cannot check is the caller's: the parts' cells come from caches that lend
//! each cell as [`generate_cell`](crate::galaxy::placement::generate_cell) makes it, as every
//! [`CellCache`] does, and they were walked with sources that suppress as the plan's do.

use crate::galaxy::Galaxy;
use crate::galaxy::placement::{CellCache, CellKey, SystemRecord};
use crate::id::Layer;

use super::{
    LayerSet, QuerySphere, QueryStats, RangePlan, RangeResult, SystemHit, SystemSource,
    cells_in_sphere, hit_at, ids_are_unique, reserve_for, sort_hits, suppressed,
};

/// One cell of a [`WalkedCells`]: its key, the end of its hits among the part's, and the records
/// the walk examined there.
#[derive(Debug, Clone, Copy, PartialEq)]
struct WalkedCell {
    key: CellKey,
    hits_end: usize,
    examined: u64,
}

/// The walk of some of a plan's cells, made apart from the plan's own: each cell walked, in the
/// order walked, with the hits the walk finds in it, in record order, and the records it examined
/// there ([`RangePlan::walk_cells`]; module documentation).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WalkedCells {
    /// The sphere each layer of the cells was walked with.
    spheres: Vec<(Layer, QuerySphere)>,
    cells: Vec<WalkedCell>,
    hits: Vec<SystemHit>,
}

impl WalkedCells {
    /// The cells walked, in the order walked.
    pub fn cells(&self) -> impl Iterator<Item = CellKey> + '_ {
        self.cells.iter().map(|cell| cell.key)
    }

    /// How many cells were walked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether no cell was walked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The sphere the part's cells of `layer` were walked with, if it holds any.
    fn sphere_of(&self, layer: Layer) -> Option<&QuerySphere> {
        self.spheres
            .iter()
            .find_map(|(walked, sphere)| (*walked == layer).then_some(sphere))
    }

    /// The hits of the part's `index`th cell.
    fn hits_of(&self, index: usize) -> &[SystemHit] {
        let start = index
            .checked_sub(1)
            .map_or(0, |before| self.cells[before].hits_end);
        &self.hits[start..self.cells[index].hits_end]
    }
}

/// What a plan's sources find over its widest sphere in the layers its census admits, asked apart
/// from its walk ([`RangePlan::source_hits`]; module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceHits {
    /// The layers the sources were asked for.
    layers: LayerSet,
    /// The sphere they were asked about.
    sphere: QuerySphere,
    hits: Vec<SystemHit>,
}

impl SourceHits {
    /// The hits, each source's in the order the sources were given.
    #[must_use]
    pub fn hits(&self) -> &[SystemHit] {
        &self.hits
    }
}

impl RangePlan {
    /// The walk of `cells`, of the plan's, through `cache` with `sources`, in the order given: what
    /// the plan's own walk finds in each, to be merged by [`finish`](Self::finish). A cell of a
    /// layer the plan does not walk is passed over (module documentation).
    ///
    /// # Panics
    ///
    /// If `galaxy` is not of the seed the plan was made for.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::placement::NoCache;
    /// use hyperion_sim::galaxy::query::{RangePlan, RangeQuery};
    /// use hyperion_sim::units::LightYears;
    ///
    /// let galaxy = Galaxy::new(Seed::new(42));
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    /// let query = RangeQuery::builder(sun, LightYears::new(20.0)).build()?;
    /// let plan = RangePlan::new(&galaxy, &[], &query);
    /// let cells: Vec<_> = plan.cells().collect();
    /// // Each half walked apart, as two threads could, then merged: the plan's answer.
    /// let (first, second) = cells.split_at(cells.len() / 2);
    /// let parts = [first, second].map(|half| {
    ///     plan.walk_cells(&galaxy, &mut NoCache::new(), &[], half.iter().copied())
    /// });
    /// let hits = plan.source_hits(&galaxy, &[]);
    /// let finished = plan.finish(&galaxy, &mut NoCache::new(), &[], &parts, hits);
    /// assert_eq!(finished, plan.run(&galaxy, &mut NoCache::new(), &[]));
    /// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
    /// ```
    pub fn walk_cells<C: CellCache>(
        &self,
        galaxy: &Galaxy,
        cache: &mut C,
        sources: &[&dyn SystemSource],
        cells: impl IntoIterator<Item = CellKey>,
    ) -> WalkedCells {
        self.check_galaxy(galaxy);
        let mut part = WalkedCells::default();
        for key in cells {
            let layer = key.layer();
            let Some(sphere) = self.sphere_of(layer) else {
                continue;
            };
            if part.sphere_of(layer).is_none() {
                part.spheres.push((layer, *sphere));
            }
            let examined = cache.with_cell(galaxy, key, |records| {
                walk_records(galaxy, sources, sphere, records, &mut part.hits)
            });
            part.cells.push(WalkedCell {
                key,
                hits_end: part.hits.len(),
                examined,
            });
        }
        part
    }

    /// What `sources` find over the plan's widest sphere in the layers its census admits, each
    /// source's appended in turn: the hits [`run`](Self::run) adds to the grid's, to be merged by
    /// [`finish`](Self::finish).
    ///
    /// # Panics
    ///
    /// - If `galaxy` is not of the seed the plan was made for.
    /// - In debug builds, if a source removes hits instead of appending to them.
    #[must_use]
    pub fn source_hits(&self, galaxy: &Galaxy, sources: &[&dyn SystemSource]) -> SourceHits {
        self.check_galaxy(galaxy);
        let layers = self.census.layers();
        let mut hits = Vec::new();
        for source in sources {
            let before = hits.len();
            source.systems_in_sphere(galaxy, &self.widest, layers, &mut hits);
            debug_assert!(
                hits.len() >= before,
                "a source removed hits from the buffer instead of appending to it"
            );
        }
        SourceHits {
            layers,
            sphere: self.widest,
            hits,
        }
    }

    /// The answer [`run`](Self::run) gives, bit for bit, from `walked` and `source_hits`: each
    /// planned cell's hits taken from the part that walked it, or the cell walked through `cache`
    /// with `sources` if none can be taken (module documentation), then the sources' hits, merged
    /// and sorted. Hits of `source_hits` asked for another plan's layers or sphere are asked for
    /// again.
    ///
    /// # Panics
    ///
    /// - If `galaxy` is not of the seed the plan was made for.
    /// - In debug builds, if a system ID appears twice among the merged hits, as [`run`](Self::run).
    #[must_use]
    pub fn finish<C: CellCache>(
        &self,
        galaxy: &Galaxy,
        cache: &mut C,
        sources: &[&dyn SystemSource],
        walked: &[WalkedCells],
        source_hits: SourceHits,
    ) -> RangeResult {
        self.check_galaxy(galaxy);
        let census = self.census;
        let source_hits =
            if source_hits.layers == census.layers() && source_hits.sphere == self.widest {
                source_hits
            } else {
                self.source_hits(galaxy, sources)
            };
        let mut stats = QueryStats {
            padded_radius: self.widest.padded_radius(),
            ..QueryStats::default()
        };
        let mut systems = Vec::with_capacity(reserve_for(&census));
        let mut merge = Merge::new(self, walked);
        for (layer, sphere) in &self.spheres {
            for key in cells_in_sphere(*layer, sphere) {
                stats.cells_visited += 1;
                stats.systems_examined += match merge.take(key) {
                    Some((hits, examined)) => {
                        systems.extend_from_slice(hits);
                        examined
                    }
                    None => cache.with_cell(galaxy, key, |records| {
                        walk_records(galaxy, sources, sphere, records, &mut systems)
                    }),
                };
            }
        }
        let from_sources = source_hits.hits.len();
        systems.extend(source_hits.hits);
        stats.systems_examined +=
            u64::try_from(from_sources).expect("a hit count fits in 64 bits on every target");
        sort_hits(&mut systems);
        // The sort is a strict order only while no ID appears twice, which the sources' contract
        // promises and a debug build checks.
        debug_assert!(
            ids_are_unique(&systems),
            "a system ID appears twice among the merged hits: a source broke the contract that its \
             IDs are its own"
        );
        RangeResult::new(systems, census, stats)
    }

    /// The sphere the plan walks `layer` with, if it walks it.
    fn sphere_of(&self, layer: Layer) -> Option<&QuerySphere> {
        self.spheres
            .iter()
            .find_map(|(walked, sphere)| (*walked == layer).then_some(sphere))
    }

    /// Panics unless `galaxy` is of the seed the plan was made for.
    fn check_galaxy(&self, galaxy: &Galaxy) {
        assert_eq!(
            galaxy.seed(),
            self.seed,
            "a range plan is run in the galaxy it was made for"
        );
    }
}

/// The hits among `records`, the systems of one cell, that `sphere` holds and no source of
/// `sources` suppresses, appended to `hits` in record order; and the records examined, all of
/// them: the walk of one cell, the same whether [`RangePlan::walk_cells`] or
/// [`RangePlan::finish`] makes it.
fn walk_records(
    galaxy: &Galaxy,
    sources: &[&dyn SystemSource],
    sphere: &QuerySphere,
    records: &[SystemRecord],
    hits: &mut Vec<SystemHit>,
) -> u64 {
    for record in records {
        if let Some(hit) = hit_at(galaxy, record, sphere)
            && !suppressed(sources, galaxy, record, sphere.time())
        {
            hits.push(hit);
        }
    }
    u64::try_from(records.len()).expect("a cell's record count fits in 64 bits on every target")
}

/// The parts a plan's [`finish`](RangePlan::finish) takes cells from: a cursor on each part, at the
/// first of its cells not yet taken, and the part to look in first.
struct Merge<'a> {
    parts: Vec<Cursor<'a>>,
    next: usize,
}

/// One part of a [`Merge`].
struct Cursor<'a> {
    part: &'a WalkedCells,
    /// The layers of the part that were walked with the plan's own sphere for them.
    usable: LayerSet,
    at: usize,
}

impl<'a> Merge<'a> {
    /// The parts of `walked` that `plan` may take cells from.
    fn new(plan: &RangePlan, walked: &'a [WalkedCells]) -> Self {
        let parts = walked
            .iter()
            .map(|part| Cursor {
                part,
                usable: part
                    .spheres
                    .iter()
                    .filter(|(layer, sphere)| plan.sphere_of(*layer) == Some(sphere))
                    .map(|(layer, _)| *layer)
                    .collect(),
                at: 0,
            })
            .collect();
        Self { parts, next: 0 }
    }

    /// The hits and the records examined of `key`, the plan's next cell in walk order, from the
    /// first part whose next usable cell it is, looking first in the part after the last one taken
    /// from, where a server's parts dealt out in turn hold it; or `None` if no part's is.
    fn take(&mut self, key: CellKey) -> Option<(&'a [SystemHit], u64)> {
        let count = self.parts.len();
        for offset in 0..count {
            let index = (self.next + offset) % count;
            let cursor = &mut self.parts[index];
            let cells = &cursor.part.cells;
            while cells
                .get(cursor.at)
                .is_some_and(|cell| !cursor.usable.contains(cell.key.layer()))
            {
                cursor.at += 1;
            }
            if cells.get(cursor.at).is_some_and(|cell| cell.key == key) {
                let taken = (cursor.part.hits_of(cursor.at), cells[cursor.at].examined);
                cursor.at += 1;
                self.next = (index + 1) % count;
                return Some(taken);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{NoCache, generate_cell};
    use crate::galaxy::query::{LayerCounts, RangeQuery, SubstellarRequest};
    use crate::rng::Seed;
    use crate::time::UniverseTime;
    use crate::units::LightYears;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(
            Seed::new(0x0309_0de4_0000_0000),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// The query of `radius` light-years about the Sun-like point, every stellar layer and the
    /// brown dwarfs, at `years` after the epoch.
    fn query(radius: f64, years: i64) -> RangeQuery {
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        RangeQuery::builder(sun, LightYears::new(radius))
            .limit(NonZeroU32::new(20_000).unwrap())
            .substellar(SubstellarRequest::BrownDwarfs)
            .time(UniverseTime::from_julian_years(years).unwrap())
            .build()
            .unwrap()
    }

    /// A source that places nothing, counts `counts`, and suppresses every grid system whose ID
    /// is odd, so that the walks' suppression is exercised.
    #[derive(Debug)]
    struct OddOut(LayerCounts);

    impl SystemSource for OddOut {
        fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
            self.0
        }

        fn systems_in_sphere(
            &self,
            _galaxy: &Galaxy,
            _sphere: &QuerySphere,
            _layers: LayerSet,
            _out: &mut Vec<SystemHit>,
        ) {
        }

        fn suppresses(&self, _galaxy: &Galaxy, record: &SystemRecord, _t: UniverseTime) -> bool {
            record.id().raw() % 2 == 1
        }
    }

    /// A cache that lends nothing and panics if asked: what a finish that takes every cell from its
    /// parts walks through.
    struct Refusing;

    impl CellCache for Refusing {
        fn with_cell<R>(
            &mut self,
            _galaxy: &Galaxy,
            key: CellKey,
            _f: impl FnOnce(&[SystemRecord]) -> R,
        ) -> R {
            panic!("the finish walked {key:?}, which a part holds");
        }
    }

    /// Split at every cell boundary, the walk's two halves walked apart and merged, or the first
    /// alone and the rest left to the finish, answer as the plan's run does, bit for bit, with and
    /// without a source that suppresses, at the epoch and away from it (P09.T40.a).
    #[test]
    fn a_walk_split_at_every_cell_boundary_is_the_plan_s_run() {
        let galaxy = galaxy();
        let suppressing = OddOut(LayerCounts::ZERO);
        let with: [&dyn SystemSource; 1] = [&suppressing];
        for (query, sources) in [
            (query(12.0, 0), &[][..]),
            (query(12.0, 0), &with[..]),
            (query(12.0, -700), &with[..]),
        ] {
            let plan = RangePlan::new(&galaxy, sources, &query);
            let expected = plan.run(&galaxy, &mut NoCache::new(), sources);
            assert_ne!(expected.systems(), &[]);
            let cells: Vec<_> = plan.cells().collect();
            assert!(cells.len() > 20, "{} cells", cells.len());
            for split in 0..=cells.len() {
                let (first, second) = cells.split_at(split);
                let walk = |half: &[CellKey]| {
                    plan.walk_cells(&galaxy, &mut NoCache::new(), sources, half.iter().copied())
                };
                let parts = [walk(first), walk(second)];
                let hits = || plan.source_hits(&galaxy, sources);
                assert_eq!(
                    plan.finish(&galaxy, &mut Refusing, sources, &parts, hits()),
                    expected,
                    "split at {split}"
                );
                assert_eq!(
                    plan.finish(&galaxy, &mut NoCache::new(), sources, &parts[..1], hits()),
                    expected,
                    "the first {split} cells walked apart"
                );
            }
        }
    }

    /// Every cell its own part, dealt out in turn, in reverse, or walked by the plan without the
    /// sources whose sources' counts drop layers, and parts of another query: each finish answers
    /// as the plan's run, taking from the parts only what its own walk would find (P09.T40.a).
    #[test]
    fn any_parts_finish_as_the_plan_s_run() {
        let galaxy = galaxy();
        let asked = query(12.0, 0);
        let mut heavy = LayerCounts::ZERO;
        for layer in Layer::ALL {
            heavy.set(layer, f64::from(asked.limit().get()) / 3.0);
        }
        let source = OddOut(heavy);
        let sources: [&dyn SystemSource; 1] = [&source];
        let unsourced = RangePlan::new(&galaxy, &[], &asked);
        let plan = unsourced.with_sources(&galaxy, &sources);
        assert_ne!(plan.census().layers(), unsourced.census().layers());
        let expected = plan.run(&galaxy, &mut NoCache::new(), &sources);
        let cells: Vec<_> = unsourced.cells().collect();
        let one_each: Vec<_> = cells
            .iter()
            .map(|&cell| unsourced.walk_cells(&galaxy, &mut NoCache::new(), &sources, [cell]))
            .collect();
        let reversed: Vec<_> = one_each.iter().rev().cloned().collect();
        let dealt: Vec<_> = (0..5)
            .map(|first| {
                let share = cells.iter().skip(first).step_by(5).copied();
                unsourced.walk_cells(&galaxy, &mut NoCache::new(), &sources, share)
            })
            .collect();
        let other_query = RangePlan::new(&galaxy, &[], &query(13.0, 0));
        let other =
            [other_query.walk_cells(&galaxy, &mut NoCache::new(), &sources, other_query.cells())];
        for (name, parts) in [
            ("one cell a part", &one_each[..]),
            ("reversed", &reversed[..]),
            ("dealt out in turn", &dealt[..]),
            ("another query's", &other[..]),
        ] {
            let hits = plan.source_hits(&galaxy, &sources);
            assert_eq!(
                plan.finish(&galaxy, &mut NoCache::new(), &sources, parts, hits),
                expected,
                "{name}"
            );
        }
        // The sources' hits of the plan without them are of other layers, and are asked again.
        let stale = unsourced.source_hits(&galaxy, &sources);
        assert_eq!(
            plan.finish(&galaxy, &mut NoCache::new(), &sources, &dealt, stale),
            expected
        );
        // Every cell but those of the layers the census drops is taken from a part.
        let walked: Vec<_> = plan.cells().collect();
        assert!(walked.len() < cells.len());
        let hits = plan.source_hits(&galaxy, &sources);
        assert_eq!(
            plan.finish(&galaxy, &mut Refusing, &sources, &dealt, hits),
            expected
        );
    }

    /// A part records what the walk of each cell finds, and passes over a cell of a layer its
    /// plan does not walk.
    #[test]
    fn a_part_holds_each_cell_s_hits_in_record_order() {
        let galaxy = galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        // No substellar layer asked for, so the brown dwarfs' is not walked.
        let asked = RangeQuery::builder(sun, LightYears::new(12.0))
            .build()
            .unwrap();
        let plan = RangePlan::new(&galaxy, &[], &asked);
        let cells: Vec<_> = plan.cells().collect();
        let part = plan.walk_cells(&galaxy, &mut NoCache::new(), &[], cells.iter().copied());
        assert_eq!(part.cells().collect::<Vec<_>>(), cells);
        assert_eq!((part.len(), part.is_empty()), (cells.len(), false));
        let mut found = 0;
        for (index, &key) in cells.iter().enumerate() {
            let mut records = Vec::new();
            generate_cell(&galaxy, key, &mut records);
            let sphere = plan.sphere_of(key.layer()).unwrap();
            let mut hits = Vec::new();
            let examined = walk_records(&galaxy, &[], sphere, &records, &mut hits);
            assert_eq!(part.hits_of(index), &hits[..]);
            assert_eq!(part.cells[index].examined, examined);
            found += hits.len();
        }
        assert!(found > 0);
        assert!(plan.sphere_of(Layer::BrownDwarf).is_none());
        let sphere =
            QuerySphere::new(sun, LightYears::new(12.0), asked.time(), LightYears::ZERO).unwrap();
        let unwalked = cells_in_sphere(Layer::BrownDwarf, &sphere).next().unwrap();
        assert!(
            plan.walk_cells(&galaxy, &mut NoCache::new(), &[], [unwalked])
                .is_empty()
        );
    }
}
