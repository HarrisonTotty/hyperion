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
//! A cell of layer E that the cache does not hold is read in part, by either walk: only the systems
//! whose epoch positions let them reach the sphere by its time, by what bounds their speeds, are
//! made whole and tested, and the rest are counted ([`visit_cell`]), since their records are most
//! of a layer-E cell's cost (the exact fates and lifetimes of its dead and its living, P08.T12) and
//! a 50 ly sphere holds some 2% of its eight cells' systems. The hits and the records examined are
//! those of the whole cell. What is read in part is never stored, and a cell the cache holds
//! ([`CellCache::with_cell_if_held`], every cell for a cache that does not say) is walked from its
//! records.
//!
//! A part is taken only where it holds what the plan's own walk would find: a cell of a layer the
//! plan walks with the very sphere the part was walked with, met in walk order. Any other cell of a
//! part is passed over and walked again, so no choice of parts changes an answer, only how long it
//! takes. What `finish` cannot check is the caller's: the parts' cells come from caches that lend
//! each cell as [`generate_cell`](crate::galaxy::placement::generate_cell) makes it, as every
//! [`CellCache`] does, and they were walked with sources that suppress as the plan's do.

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::placement::{CellCache, CellKey, SystemRecord, Visited, visit_cell};
use crate::id::Layer;
use crate::rng::Seed;
use crate::units::{LightYears, MetresPerSecond};

use super::{
    LayerSet, PAD_SPEED, QuerySphere, QueryStats, RangePlan, RangeResult, SpeedCut, SystemHit,
    SystemSource, cells_in_sphere, epoch_velocity, hit_at, ids_are_unique, kepler_pad_below,
    pad_speed, reserve_for, sort_hits, suppressed,
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
#[derive(Debug, Clone, PartialEq)]
pub struct WalkedCells {
    /// The seed of the galaxy the cells are of.
    seed: Seed,
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
    #[must_use]
    fn sphere_of(&self, layer: Layer) -> Option<&QuerySphere> {
        self.spheres
            .iter()
            .find_map(|(walked, sphere)| (*walked == layer).then_some(sphere))
    }

    /// The hits of the part's `index`th cell.
    #[must_use]
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
    /// The seed of the galaxy the sources were asked in.
    seed: Seed,
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
    /// the plan's own walk finds in each, to be merged by [`finish`](Self::finish), whose example
    /// shows the three together. A cell of a layer the plan does not walk is passed over (module
    /// documentation).
    ///
    /// # Panics
    ///
    /// If `galaxy` is not of the seed the plan was made for.
    pub fn walk_cells<C: CellCache>(
        &self,
        galaxy: &Galaxy,
        cache: &mut C,
        sources: &[&dyn SystemSource],
        cells: impl IntoIterator<Item = CellKey>,
    ) -> WalkedCells {
        self.check_galaxy(galaxy);
        let mut part = WalkedCells {
            seed: self.seed,
            spheres: Vec::new(),
            cells: Vec::new(),
            hits: Vec::new(),
        };
        for key in cells {
            let layer = key.layer();
            let Some(sphere) = self.sphere_of(layer) else {
                continue;
            };
            if part.sphere_of(layer).is_none() {
                part.spheres.push((layer, *sphere));
            }
            let examined = walk_cell(galaxy, cache, sources, sphere, key, &mut part.hits);
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
    /// [`finish`](Self::finish), whose example shows the three together.
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
            seed: self.seed,
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
    /// - In debug builds, if a source removes hits instead of appending to them, where it asks
    ///   them again, or if a system ID appears twice among the merged hits, as [`run`](Self::run).
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
        let asked_for_this_plan = source_hits.seed == self.seed
            && source_hits.layers == census.layers()
            && source_hits.sphere == self.widest;
        let source_hits = if asked_for_this_plan {
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
                    None => walk_cell(galaxy, cache, sources, sphere, key, &mut systems),
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
    #[must_use]
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

/// The walk of the cell `key` through `cache`, the same whether [`RangePlan::walk_cells`] or
/// [`RangePlan::finish`] makes it: [`walk_records`] of the cell's systems, or, for a cell of layer
/// E that `cache` does not hold, the cell read in part ([`walk_in_part`]), which finds the same
/// hits and examines the same number of records (module documentation).
fn walk_cell<C: CellCache>(
    galaxy: &Galaxy,
    cache: &mut C,
    sources: &[&dyn SystemSource],
    sphere: &QuerySphere,
    key: CellKey,
    hits: &mut Vec<SystemHit>,
) -> u64 {
    if key.layer() != Layer::E {
        return cache.with_cell(galaxy, key, |records| {
            walk_records(galaxy, sources, sphere, records, hits)
        });
    }
    cache
        .with_cell_if_held(galaxy, key, |records| {
            walk_records(galaxy, sources, sphere, records, hits)
        })
        .unwrap_or_else(|| walk_in_part(galaxy, sources, sphere, key, hits))
}

/// The hits among `records`, the systems of one cell, that `sphere` holds and no source of
/// `sources` suppresses, appended to `hits` in record order; and the records examined, all of
/// them.
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

/// [`walk_records`] of the systems of the cell `key`, bit for bit, the cell read in part
/// ([`visit_cell`]): only a system whose epoch position lets it reach `sphere` by its time
/// ([`Reach`]) is made whole and tested, and the others are counted as examined from what decides
/// that they are systems. Nothing is generated whole, so nothing is left to store.
fn walk_in_part(
    galaxy: &Galaxy,
    sources: &[&dyn SystemSource],
    sphere: &QuerySphere,
    key: CellKey,
    hits: &mut Vec<SystemHit>,
) -> u64 {
    let reach = Reach::of(galaxy, key.layer(), sphere);
    let mut examined = 0_u64;
    visit_cell(
        galaxy,
        key,
        |position, cut| reach.reads(sphere, position, cut),
        |system| {
            examined += 1;
            let Visited::Read(record, cut) = system else {
                return;
            };
            debug_assert!(
                cut == SpeedCut::Pad
                    || epoch_velocity(galaxy, &record).speed().value()
                        < MetresPerSecond::from(PAD_SPEED).value(),
                "{:?} moves at the escape cut's speed or faster, which its reach reads it by",
                record.id()
            );
            if let Some(hit) = hit_at(galaxy, &record, sphere)
                && !suppressed(sources, galaxy, &record, sphere.time())
            {
                hits.push(hit);
            }
        },
    );
    examined
}

/// What a walk of a cell read in part adds to a system's reach beyond the sphere's radius and pad,
/// light-years: 10⁻⁶ ly, some 0.06 AU, far above the rounding of the distances, drifts and pads it
/// compares (parts in 10¹⁵ of at most a few hundred thousand light-years), so that the epoch
/// position's test reads every system [`hit_at`] can find in the sphere.
const READ_MARGIN_LY: f64 = 1e-6;

/// How far from a sphere's centre the epoch position of a system of one layer may lie for the
/// system to be in the sphere at its time, light-years, by what bounds its speed ([`SpeedCut`]):
/// the radius, the pad of the Kepler regime's bound at that speed ([`kepler_pad_below`], at least
/// the straight lines' speed times |t|), and [`READ_MARGIN_LY`].
///
/// Plan 08 draws every grid velocity below [`PAD_SPEED`] but for the classes the escape cut
/// exempts, which are below their layer's [`pad_speed`] (Design note 7; P08.T17), and the Kepler
/// regime's bound holds of any record below the speed it is given. So a system the test refuses
/// cannot be in the sphere at its time: [`hit_at`] would find no hit in it. At the epoch both pads
/// are nothing and the test is [`hit_at`]'s own distance, with the margin.
#[derive(Debug, Clone, Copy)]
struct Reach {
    /// The reach of a system below the escape speed and [`PAD_SPEED`], ly.
    escape_ly: f64,
    /// The reach of a system below its layer's [`pad_speed`] alone, ly.
    pad_ly: f64,
}

impl Reach {
    /// The reaches of `layer`'s systems about `sphere`.
    #[must_use]
    fn of(galaxy: &Galaxy, layer: Layer, sphere: &QuerySphere) -> Self {
        let reach = |speed| {
            let pad = kepler_pad_below(
                galaxy,
                speed,
                sphere.centre(),
                sphere.radius(),
                sphere.time(),
            );
            sphere.radius().value() + pad.value() + READ_MARGIN_LY
        };
        Self {
            escape_ly: reach(PAD_SPEED),
            pad_ly: reach(pad_speed(layer)),
        }
    }

    /// Whether a system at the epoch position `position`, whose speed `cut` bounds, may be in
    /// `sphere` at its time.
    #[must_use]
    fn reads(&self, sphere: &QuerySphere, position: &GalacticPosition, cut: SpeedCut) -> bool {
        let reach_ly = match cut {
            SpeedCut::Escape => self.escape_ly,
            SpeedCut::Pad => self.pad_ly,
        };
        LightYears::from(sphere.centre().distance_to(position)).value() <= reach_ly
    }
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
    /// The parts of `walked` that `plan` may take cells from: of its galaxy, and of its layers
    /// walked with its own spheres.
    #[must_use]
    fn new(plan: &RangePlan, walked: &'a [WalkedCells]) -> Self {
        let parts = walked
            .iter()
            .map(|part| Cursor {
                part,
                usable: part
                    .spheres
                    .iter()
                    .filter(|(layer, sphere)| {
                        part.seed == plan.seed && plan.sphere_of(*layer) == Some(sphere)
                    })
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
    use std::collections::BTreeMap;
    use std::num::NonZeroU32;
    use std::sync::{Arc, Mutex, OnceLock};

    use super::*;
    use crate::coords::{GalacticDisplacement, GalacticPosition};
    use crate::galaxy::catalogue_classes::{
        CatalogueCellKey, CatalogueClassCellCache, CatalogueClassSource, ClassCellSystems,
    };
    use crate::galaxy::features::FeatureProcess;
    use crate::galaxy::features::catalogue::{
        FeatureCatalogue, FeatureCellCache, FeatureCellContents, FeatureRecord,
    };
    use crate::galaxy::features::members::FeatureInterior;
    use crate::galaxy::features::members::source::{FeatureMemberSource, KeepInteriors};
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{NoCache, generate_cell};
    use crate::galaxy::query::{LayerCounts, RangeQuery, SubstellarRequest, position_at};
    use crate::galaxy::rules::GalaxyRules;
    use crate::id::FeatureCell;
    use crate::rng::Seed;
    use crate::time::UniverseTime;
    use crate::units::consts::METRES_PER_LIGHT_YEAR;

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

    /// A source that counts `counts`, suppresses every grid system whose ID is odd, so that the
    /// walks' suppression is exercised, and places one of its own systems for each layer asked,
    /// at half the sphere's radius and a light-year more for each, so that its hits depend on the
    /// layers and the sphere it is asked for.
    #[derive(Debug)]
    struct OddOut {
        counts: LayerCounts,
        /// Its own systems, of a cell far from every query here.
        own: Vec<SystemRecord>,
    }

    impl OddOut {
        fn new(galaxy: &Galaxy, counts: LayerCounts) -> Self {
            let far = GalacticPosition::from_light_years([0.0, 26_000.0, 2_000.0]).unwrap();
            let sphere = QuerySphere::new(
                far,
                LightYears::new(5.0),
                UniverseTime::EPOCH,
                LightYears::ZERO,
            )
            .unwrap();
            let mut own = Vec::new();
            for layer in Layer::ALL {
                for key in cells_in_sphere(layer, &sphere) {
                    let mut records = Vec::new();
                    crate::galaxy::placement::generate_cell(galaxy, key, &mut records);
                    own.extend(records);
                }
            }
            assert!(
                own.len() >= Layer::ALL.len(),
                "{} systems of its own",
                own.len()
            );
            Self { counts, own }
        }
    }

    impl SystemSource for OddOut {
        fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
            self.counts
        }

        fn systems_in_sphere(
            &self,
            _galaxy: &Galaxy,
            sphere: &QuerySphere,
            layers: LayerSet,
            out: &mut Vec<SystemHit>,
        ) {
            for (step, (_, record)) in layers.iter().zip(&self.own).enumerate() {
                let distance =
                    sphere.radius().value() / 2.0 + f64::from(u8::try_from(step).unwrap());
                out.push(SystemHit::new(
                    *record,
                    *record.epoch_position(),
                    LightYears::new(distance),
                ));
            }
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
        let suppressing = OddOut::new(&galaxy, LayerCounts::ZERO);
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
        let source = OddOut::new(&galaxy, heavy);
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
        // The sources' hits of the plan without them are of other layers, and those of the 13 ly
        // query's plan with them of another sphere: each is asked again.
        let fresh = plan.source_hits(&galaxy, &sources);
        let other_sourced = RangePlan::new(&galaxy, &sources, &query(13.0, 0));
        assert_eq!(other_sourced.census().layers(), plan.census().layers());
        for stale in [
            unsourced.source_hits(&galaxy, &sources),
            other_sourced.source_hits(&galaxy, &sources),
        ] {
            assert_ne!(stale.hits(), fresh.hits());
            assert_eq!(
                plan.finish(&galaxy, &mut NoCache::new(), &sources, &dealt, stale),
                expected
            );
        }
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

    /// The parts tests' Milky Way fixture with its kinematic tables, so that its systems move away
    /// from the epoch, built once.
    fn moving() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| galaxy().with_full_potential())
    }

    /// A cache that holds no cell and says so, as a server's that is cold does: it generates whole
    /// any cell it is asked for whole, and counts the layer-E cells it is asked for each way.
    #[derive(Debug, Default)]
    struct Lacking {
        /// The layer-E cells it was asked to lend whole, and generated.
        whole_e: usize,
        /// The layer-E cells it was asked for if held.
        asked_e: usize,
    }

    impl CellCache for Lacking {
        fn with_cell<R>(
            &mut self,
            galaxy: &Galaxy,
            key: CellKey,
            f: impl FnOnce(&[SystemRecord]) -> R,
        ) -> R {
            self.whole_e += usize::from(key.layer() == Layer::E);
            let mut records = Vec::new();
            generate_cell(galaxy, key, &mut records);
            f(&records)
        }

        fn with_cell_if_held<R>(
            &mut self,
            _galaxy: &Galaxy,
            key: CellKey,
            _f: impl FnOnce(&[SystemRecord]) -> R,
        ) -> Option<R> {
            self.asked_e += usize::from(key.layer() == Layer::E);
            None
        }
    }

    /// A cache that holds every other layer-E cell, by the parity of its coordinates' sum, as a
    /// server's does with some evicted or some not yet kept: it lends those whole, and says it
    /// lacks the rest.
    #[derive(Debug, Default)]
    struct Partly {
        /// The layer-E cells it lent whole and those it said it lacks.
        held_and_lacking: [usize; 2],
    }

    impl CellCache for Partly {
        fn with_cell<R>(
            &mut self,
            galaxy: &Galaxy,
            key: CellKey,
            f: impl FnOnce(&[SystemRecord]) -> R,
        ) -> R {
            NoCache::new().with_cell(galaxy, key, f)
        }

        fn with_cell_if_held<R>(
            &mut self,
            galaxy: &Galaxy,
            key: CellKey,
            f: impl FnOnce(&[SystemRecord]) -> R,
        ) -> Option<R> {
            let held = key.gen_cell().to_array().iter().sum::<i32>().rem_euclid(2) == 0;
            self.held_and_lacking[usize::from(!held)] += 1;
            held.then(|| self.with_cell(galaxy, key, f))
        }
    }

    /// `plan`'s answer walked through [`Lacking`], by its own walk and by parts dealt out in turn
    /// among three, each finished with the sources' hits `hits`: each the answer `expected`, and no
    /// layer-E cell generated whole.
    fn assert_read_in_part(
        galaxy: &Galaxy,
        plan: &RangePlan,
        sources: &[&dyn SystemSource],
        hits: &SourceHits,
        expected: &RangeResult,
        label: &str,
    ) {
        let layer_e = plan.cells().filter(|cell| cell.layer() == Layer::E).count();
        assert!(layer_e > 0, "{label}: the plan walks layer E");
        let mut lacking = Lacking::default();
        assert_eq!(
            &plan.finish(galaxy, &mut lacking, sources, &[], hits.clone()),
            expected,
            "{label}: the plan's walk"
        );
        assert_eq!(
            (lacking.whole_e, lacking.asked_e),
            (0, layer_e),
            "{label}: each layer-E cell asked for if held, and read in part"
        );
        let cells: Vec<_> = plan.cells().collect();
        let parts: Vec<_> = (0..3)
            .map(|first| {
                let share = cells.iter().skip(first).step_by(3).copied();
                plan.walk_cells(galaxy, &mut Lacking::default(), sources, share)
            })
            .collect();
        assert_eq!(
            &plan.finish(galaxy, &mut Refusing, sources, &parts, hits.clone()),
            expected,
            "{label}: the parts' walks"
        );
        let mut partly = Partly::default();
        assert_eq!(
            &plan.finish(galaxy, &mut partly, sources, &[], hits.clone()),
            expected,
            "{label}: every other layer-E cell held"
        );
        assert_eq!(partly.held_and_lacking.iter().sum::<usize>(), layer_e);
    }

    /// A layer-E cell the cache lacks is read in part, by the plan's walk and by its parts', and
    /// none is generated whole: each answer is the plan's through no cache, which walks every cell
    /// whole, bit for bit, hits and statistics, at the epoch and either side of it, with and
    /// without a source that suppresses (P08.T12).
    #[test]
    fn a_layer_e_cell_the_cache_lacks_is_read_in_part_for_the_whole_cell_s_answer() {
        let galaxy = moving();
        let suppressing = OddOut::new(galaxy, LayerCounts::ZERO);
        let with: [&dyn SystemSource; 1] = [&suppressing];
        for (query, sources) in [
            (query(50.0, 0), &[][..]),
            (query(50.0, -1_000), &with[..]),
            (query(50.0, 700), &with[..]),
        ] {
            let plan = RangePlan::new(galaxy, sources, &query);
            let expected = plan.run(galaxy, &mut NoCache::new(), sources);
            assert_ne!(expected.systems(), &[]);
            let hits = plan.source_hits(galaxy, sources);
            let label = format!("{} yr", query.time());
            assert_read_in_part(galaxy, &plan, sources, &hits, &expected, &label);
        }
    }

    /// The epoch position's test reads every system that can be in a sphere at its time: for each
    /// system of four layer-E cells at the Sun-like point, and each of a class the escape cut
    /// exempts in 96 more along the solar circle and 12 in the bulge, at ±100 and ±1,000 yr, a sphere of 0.5 or 50 ly
    /// whose edge passes through where the system is then, on the side away from its epoch
    /// position, so that its epoch position is the farthest from the centre it can be, reads it
    /// wherever it is a hit; and the test reads none whose epoch position is beyond the reach by a
    /// light-year (P08.T12).
    #[test]
    fn the_epoch_position_s_test_reads_every_system_a_sphere_can_hold() {
        let galaxy = moving();
        let mut records = Vec::new();
        let mut cell = Vec::new();
        for x in -1..=1 {
            for y in (195..=210).chain([23, 24]) {
                for z in [-1, 0] {
                    generate_cell(
                        galaxy,
                        CellKey::new(Layer::E, [x, y, z]).unwrap(),
                        &mut cell,
                    );
                    let near = (0..=1).contains(&x) && y == 203;
                    records.extend(cell.iter().filter(|record| {
                        near || SpeedCut::of_record(galaxy, record) == SpeedCut::Pad
                    }));
                }
            }
        }
        let exempt = records
            .iter()
            .filter(|record| SpeedCut::of_record(galaxy, record) == SpeedCut::Pad)
            .count();
        let (mut hits, mut pads) = (0_u32, [0_u32; 2]);
        for record in &records {
            let cut = SpeedCut::of_record(galaxy, record);
            let epoch = record.epoch_position();
            for years in [-1_000, -100, 100, 1_000] {
                let t = UniverseTime::from_julian_years(years).unwrap();
                let there = position_at(galaxy, record, t);
                let away = epoch.displacement_to(&there).metres();
                let moved_m = epoch.displacement_to(&there).length().value();
                assert!(moved_m > 0.0, "{:?} moves", record.id());
                for radius in [0.5, 50.0] {
                    let out = radius * METRES_PER_LIGHT_YEAR / moved_m;
                    let centre = there
                        .translated(GalacticDisplacement::new(away.map(|m| m * out)))
                        .unwrap();
                    let sphere =
                        QuerySphere::new(centre, LightYears::new(radius), t, LightYears::ZERO)
                            .unwrap();
                    let reach = Reach::of(galaxy, Layer::E, &sphere);
                    if hit_at(galaxy, record, &sphere).is_some() {
                        hits += 1;
                        pads[usize::from(cut == SpeedCut::Pad)] += 1;
                        assert!(
                            reach.reads(&sphere, epoch, cut),
                            "{:?} at {years} yr, {radius} ly: {} ly from its epoch position",
                            record.id(),
                            moved_m / METRES_PER_LIGHT_YEAR
                        );
                    }
                    let beyond_ly = match cut {
                        SpeedCut::Escape => reach.escape_ly,
                        SpeedCut::Pad => reach.pad_ly,
                    } + 1.0;
                    let far = sphere
                        .centre()
                        .translated(GalacticDisplacement::new([
                            beyond_ly * METRES_PER_LIGHT_YEAR,
                            0.0,
                            0.0,
                        ]))
                        .unwrap();
                    assert!(!reach.reads(&sphere, &far, cut));
                }
            }
        }
        println!(
            "{hits} hits of {} records, {pads:?} bounded by the escape cut and by the pad, of \
             {exempt} records the escape cut exempts",
            records.len()
        );
        assert!(
            hits > 500 && pads.iter().all(|&n| n > 0),
            "{hits} hits, {pads:?}"
        );
        assert!(exempt > 2, "{exempt} records the escape cut exempts");
    }

    /// The point of the Sun-like queries.
    fn sun_like() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    /// The core of the globular with members nearest the Sun-like point, as the server's benches
    /// choose it.
    fn globular_core(galaxy: &Galaxy) -> GalacticPosition {
        let sun = sun_like();
        let mut globulars: Vec<FeatureRecord> =
            FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular).collect();
        globulars.sort_by(|a, b| {
            let away = |f: &FeatureRecord| f.position().distance_to(&sun).value();
            away(a).total_cmp(&away(b))
        });
        *globulars
            .iter()
            .find_map(|feature| FeatureInterior::of(galaxy, feature))
            .expect("the galaxy has a globular with members")
            .feature()
            .position()
    }

    /// A feature-cell cache that keeps every cell whole, for one galaxy.
    #[derive(Debug, Default)]
    struct KeepFeatureCells(Mutex<BTreeMap<[i32; 3], Arc<FeatureCellContents>>>);

    impl FeatureCellCache for KeepFeatureCells {
        fn contents(&self, galaxy: &Galaxy, cell: FeatureCell) -> Arc<FeatureCellContents> {
            let mut cells = self.0.lock().unwrap();
            Arc::clone(
                cells
                    .entry(cell.to_array())
                    .or_insert_with(|| Arc::new(FeatureCatalogue::cell(galaxy, cell))),
            )
        }
    }

    /// A class-cell cache that keeps every cell's systems, for one galaxy.
    #[derive(Debug, Default)]
    struct KeepClassCells(Mutex<BTreeMap<CatalogueCellKey, Arc<ClassCellSystems>>>);

    impl CatalogueClassCellCache for KeepClassCells {
        fn systems(
            &self,
            _galaxy: &Galaxy,
            key: CatalogueCellKey,
            make: &mut dyn FnMut() -> ClassCellSystems,
        ) -> Arc<ClassCellSystems> {
            let mut kept = self.0.lock().unwrap();
            Arc::clone(kept.entry(key).or_insert_with(|| Arc::new(make())))
        }
    }

    /// The server's galaxy, of its tests' seed, under `rules`, with its kinematic tables: the
    /// galaxy of the queries the server's benches time.
    fn served_under(rules: GalaxyRules) -> Galaxy {
        let seed = Seed::new(0x4d2);
        Galaxy::from_params_with_rules(
            seed,
            GalaxyParams::from_seed(seed, MassFunctionKind::default()),
            rules,
        )
        .expect("the server's galaxy builds")
        .with_full_potential()
    }

    /// The fifteen whole queries of P08.T12's walk read in part, as the server's benches ask them,
    /// in its galaxy: the core of the globular nearest the Sun-like point at 1 and 50 ly with
    /// every source a served query takes beside the grid (the members' and the catalogue
    /// classes') and with none, and the Sun-like point's at 50 ly with them, each at the epoch, 700
    /// years before it and 400 after under the generator version's rules, the tree holding no
    /// galaxy-side hold. Each layer-E cell is read in part, by the plan's walk and by its parts',
    /// and each answer, hits and statistics, is the plan's through no cache, bit for bit:
    /// [`range_query`](super::super::range_query)'s.
    #[test]
    #[ignore = "slow: fifteen queries with the globular's members and the classes, each walked \
                thrice, some minutes"]
    fn fifteen_whole_queries_read_layer_e_in_part_for_the_whole_answer() {
        let at_version = served_under(GalaxyRules::of_version(crate::GENERATOR_VERSION));
        let mut queries = 0;
        for (galaxy, years, rules) in [(&at_version, [0, -700, 400].as_slice(), "at the version")] {
            let core = globular_core(galaxy);
            let (features, interiors) = (KeepFeatureCells::default(), KeepInteriors::new(galaxy));
            let members = FeatureMemberSource::new(&features, &interiors);
            let class_cells = KeepClassCells::default();
            let classes = CatalogueClassSource::registered_in(galaxy)
                .map(|classes| classes.with_cells(&class_cells));
            let mut served: Vec<&dyn SystemSource> = vec![&members];
            served.extend(classes.as_ref().map(|classes| classes as &dyn SystemSource));
            for &years in years {
                for (name, centre, radius, sources) in [
                    ("globular core 1 ly", core, 1.0, &served[..]),
                    ("globular core 1 ly, no members", core, 1.0, &[][..]),
                    ("globular core 50 ly", core, 50.0, &served[..]),
                    ("globular core 50 ly, no members", core, 50.0, &[][..]),
                    ("Sun-like 50 ly", sun_like(), 50.0, &served[..]),
                ] {
                    let query = RangeQuery::builder(centre, LightYears::new(radius))
                        .limit(NonZeroU32::new(20_000).unwrap())
                        .time(UniverseTime::from_julian_years(years).unwrap())
                        .build()
                        .unwrap();
                    let plan = RangePlan::new(galaxy, sources, &query);
                    let hits = plan.source_hits(galaxy, sources);
                    let expected =
                        plan.finish(galaxy, &mut NoCache::new(), sources, &[], hits.clone());
                    let label = format!("{name} at {years} yr {rules}");
                    println!(
                        "{label}: {} hits, {:?}",
                        expected.systems().len(),
                        expected.stats()
                    );
                    assert_read_in_part(galaxy, &plan, sources, &hits, &expected, &label);
                    queries += 1;
                }
            }
        }
        assert_eq!(queries, 15);
    }
}
