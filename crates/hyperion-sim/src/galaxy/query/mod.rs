//! The range query: which systems lie within R light-years of a point at time t (plan 03).
//!
//! The brainstorm's "The range query". The query walks the stellar layers from the coarsest to the
//! finest, then the brown dwarfs and the rogue planets when the caller asks for them
//! ([`SubstellarRequest`], plan 13), visiting only the cells that meet the sphere, and returns a
//! complete census or nothing per layer: whether a layer fits under the caller's limit is decided
//! before anything is generated, from its expected count over the sphere, so the answer never
//! depends on what a caller happens to have cached. Cells are chosen by epoch position, so the sphere is padded by
//! the largest speed times |t|; distances are then tested at t and systems not yet born are
//! dropped. Sources other than the grid (features, the global list, catalogue classes, pinned
//! content) are merged through a hook that later plans fill.
//!
//! Queries take a time inside the [`ClockWindow`](crate::time::ClockWindow), within
//! ±[`CLOCK_WINDOW_H`](crate::time::CLOCK_WINDOW_H) = ±1,000 Julian years of the epoch, where
//! present positions are guaranteed (plan 03, Design note 12).

mod census;
mod expected;
mod mode;
mod motion;
mod parts;
mod request;
mod result;
mod segment;
mod source;
mod walk;

use std::error::Error;
use std::fmt;

pub use census::decide_census;
pub use expected::expected_counts;
pub use mode::{QueryMode, range_query_observed};
pub use motion::{
    PAD_SPEED, UNBOUND_PAD_SPEED, epoch_velocity, escape_cut_holds, hit_at, kepler_pad_for,
    pad_for, pad_speed, position_at,
};
pub(crate) use motion::{drifted, walk_pad};
pub use parts::{SourceHits, WalkedCells};
pub use request::{
    DEFAULT_CELL_BUDGET, DEFAULT_CENSUS_LIMIT, MassFloor, RangeQuery, RangeQueryBuilder,
    SubstellarRequest,
};
pub use result::{Census, CensusStop, LayerCounts, LayerSet, QueryStats, RangeResult, SystemHit};
pub use segment::cells_along_segment;
pub use source::SystemSource;
// `pub` for `hyperion-sky` alone (the crate split, CS.T1); not part of the sim's API.
#[doc(hidden)]
pub use walk::cells_in_shell_slab;
pub use walk::{
    BuildQuerySphereError, QuerySphere, cells_in_sphere, cells_in_sphere_slab,
    count_cells_in_sphere, sphere_slabs,
};

use crate::galaxy::Galaxy;
use crate::galaxy::placement::{CellCache, CellKey, SystemRecord};
use crate::id::Layer;
use crate::rng::Seed;
use crate::time::UniverseTime;
use crate::units::LightYears;

/// Which systems lie within the query's radius of its centre at its time.
///
/// The walk is the brainstorm's: the layers from the coarsest to the finest, only the cells that
/// meet the padded sphere, and a complete census or nothing per layer. What the result holds is
/// decided before a cell is generated, from expected counts over the sphere and the number of cells
/// each layer would take ([`decide_census`]), so two callers with different caches — or the same
/// caller twice — get the same systems, the same census and the same order.
///
/// `cache` is the caller's; `sources` are everything that is not the grid, and `&[]` is the whole
/// answer in the first milestone. The answer does not depend on the order the sources are listed
/// in: their expected counts are summed layer by layer in value order, each layer's contributions
/// sorted with [`f64::total_cmp`] and added from the smallest, suppression is a pure OR, and the
/// hits are sorted by a total order over IDs that are unique by [`SystemSource`]'s contract (ruling
/// 23 of 2026-09-22). The result's systems are **not** bounded
/// by the query's limit:
/// the limit bounds a layer's expected count, and the realised count is never truncated (plan 03,
/// Design note 9), so a caller sizing a buffer leaves room for the Poisson excess.
///
/// # Panics
///
/// - If a component's expected count is not a number, which plan 02's densities cannot produce.
/// - If the sources' expected counts sum past the largest `f64` in some layer, which no sphere the
///   root cube holds can reach.
/// - In debug builds, if a system ID appears twice among the merged hits, which only a source
///   that breaks [`SystemSource`]'s contract can cause.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::NoCache;
/// use hyperion_sim::galaxy::query::{RangeQuery, range_query};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
/// let query = RangeQuery::builder(sun, LightYears::new(20.0)).build()?;
/// // No cache: every cell is generated into a scratch buffer and dropped.
/// let result = range_query(&galaxy, &mut NoCache::new(), &[], &query);
///
/// // Twenty light-years at the Sun's density is complete down to the M dwarfs.
/// assert_eq!(result.census().complete_down_to(), Some(Layer::A));
/// assert!(!result.systems().is_empty());
/// // Nearest first, and every system is inside the radius at the query's time.
/// let mut nearer = 0.0;
/// for hit in result.systems() {
///     assert!(hit.distance().value() >= nearer && hit.distance().value() <= 20.0);
///     nearer = hit.distance().value();
/// }
/// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
/// ```
#[must_use]
pub fn range_query<C: CellCache>(
    galaxy: &Galaxy,
    cache: &mut C,
    sources: &[&dyn SystemSource],
    query: &RangeQuery,
) -> RangeResult {
    RangePlan::new(galaxy, sources, query).run(galaxy, cache, sources)
}

/// What [`range_query`] decides before it reads a cell: the widest sphere the sources are asked
/// about, the census from the grid's and the sources' expected counts, and each walked layer's
/// sphere (P09.T40.a).
///
/// A plan is a pure function of the galaxy, the sources' counts and the query, as the census is,
/// so it may be made once and run later: a server makes it, generates the cells it will walk
/// ([`cells`](Self::cells)) on several threads, and then [`run`](Self::run)s it over the warm
/// cache, for the answer [`range_query`] gives, bit for bit. It must be run in the galaxy it was
/// made for, with sources that count and find as the ones it was made with.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::NoCache;
/// use hyperion_sim::galaxy::query::{RangePlan, RangeQuery, range_query};
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
/// let query = RangeQuery::builder(sun, LightYears::new(20.0)).build()?;
/// let plan = RangePlan::new(&galaxy, &[], &query);
/// // The cells the walk will read, layer by layer from the coarsest.
/// assert!(plan.cells().count() > 100);
/// let planned = plan.run(&galaxy, &mut NoCache::new(), &[]);
/// assert_eq!(planned, range_query(&galaxy, &mut NoCache::new(), &[], &query));
/// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RangePlan {
    query: RangeQuery,
    seed: Seed,
    widest: QuerySphere,
    /// The grid's expected counts over the query's sphere, which no source changes.
    grid: LayerCounts,
    census: Census,
    /// Each layer the census admits with the sphere it is walked with, in the walk's order.
    spheres: Vec<(Layer, QuerySphere)>,
}

impl RangePlan {
    /// The plan of `query` in `galaxy` with `sources` beside the grid: their expected counts and
    /// the census they decide, before anything is generated.
    ///
    /// # Panics
    ///
    /// As [`range_query`]: if a component's expected count is not a number, or the sources' counts
    /// sum past the largest `f64` in some layer.
    #[must_use]
    pub fn new(galaxy: &Galaxy, sources: &[&dyn SystemSource], query: &RangeQuery) -> Self {
        let grid = expected_counts(galaxy, query.centre(), query.radius(), query.substellar());
        Self::decided(galaxy, sources, query, grid)
    }

    /// The plan of the same query with `sources` beside the grid instead, the grid's expected
    /// counts kept: [`new`](Self::new) with `sources`, bit for bit.
    ///
    /// A plan with no sources walks every layer, and so every cell, that a plan with sources does,
    /// since the sources' counts only add to the layers' and the census admits layers from the
    /// coarsest while they fit ([`decide_census`]): a server generates the cells of the plan
    /// without sources while the sources' own cells are generated, and then plans with them.
    ///
    /// # Panics
    ///
    /// As [`new`](Self::new), and if `galaxy` is not of the seed the plan was made for.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
    /// use hyperion_sim::galaxy::features::members::source::{FeatureMemberSource, NoInteriorCache};
    /// use hyperion_sim::galaxy::query::{RangePlan, RangeQuery, SystemSource};
    /// use hyperion_sim::units::LightYears;
    ///
    /// let galaxy = Galaxy::new(Seed::new(9));
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    /// let query = RangeQuery::builder(sun, LightYears::new(50.0)).build()?;
    /// // The grid's cells can be generated from the plan without the features' members...
    /// let unsourced = RangePlan::new(&galaxy, &[], &query);
    /// let members = FeatureMemberSource::new(&NoFeatureCache, &NoInteriorCache);
    /// let sources: [&dyn SystemSource; 1] = [&members];
    /// // ...while their feature cells are, and the plan then made with them is the same.
    /// let sourced = unsourced.with_sources(&galaxy, &sources);
    /// assert_eq!(sourced, RangePlan::new(&galaxy, &sources, &query));
    /// assert!(sourced.cells().count() <= unsourced.cells().count());
    /// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
    /// ```
    #[must_use]
    pub fn with_sources(&self, galaxy: &Galaxy, sources: &[&dyn SystemSource]) -> Self {
        assert_eq!(
            galaxy.seed(),
            self.seed,
            "a range plan is made again in the galaxy it was made for"
        );
        Self::decided(galaxy, sources, &self.query, self.grid)
    }

    /// The plan of `query` with the grid's expected counts `grid` and `sources` beside it.
    #[must_use]
    fn decided(
        galaxy: &Galaxy,
        sources: &[&dyn SystemSource],
        query: &RangeQuery,
        grid: LayerCounts,
    ) -> Self {
        let widest = widest_sphere(query);
        let per_source: Vec<LayerCounts> = sources
            .iter()
            .map(|source| source.expected_in_sphere(galaxy, &widest))
            .collect();
        let from_sources = sum_source_counts(&per_source);
        let census = decide_census(query, &grid, &from_sources, |layer| {
            count_cells_in_sphere(layer, &sphere_for_layer(galaxy, query, layer))
        });
        let layers = census.layers();
        // Coarsest first and the substellar layers last, as the brainstorm's walk goes; the
        // result's order is the sort's, not this. Each layer is walked with its own pad, so
        // raising one layer's padding speed (plan 08's unbound class) moves no other layer's cells
        // (plan 03, Design note 13).
        let spheres = result::WALK_ORDER
            .into_iter()
            .filter(|&layer| layers.contains(layer))
            .map(|layer| (layer, sphere_for_layer(galaxy, query, layer)))
            .collect();
        Self {
            query: query.clone(),
            seed: galaxy.seed(),
            widest,
            grid,
            census,
            spheres,
        }
    }

    /// The query planned.
    #[must_use]
    pub const fn query(&self) -> &RangeQuery {
        &self.query
    }

    /// The census the plan walks by.
    #[must_use]
    pub const fn census(&self) -> &Census {
        &self.census
    }

    /// The grid cells [`run`](Self::run) reads, in the order it reads them: each layer the census
    /// admits, from the coarsest, and its cells that meet the layer's sphere
    /// ([`cells_in_sphere`]).
    pub fn cells(&self) -> impl Iterator<Item = CellKey> + '_ {
        self.spheres
            .iter()
            .flat_map(|(layer, sphere)| cells_in_sphere(*layer, sphere))
    }

    /// The walk: every planned cell through `cache`, and the sources' hits, merged and sorted: the
    /// answer [`range_query`] gives for the plan's query, bit for bit. It is
    /// [`finish`](Self::finish) of no walked part and of the sources' hits
    /// ([`source_hits`](Self::source_hits)), which a server may make apart.
    ///
    /// # Panics
    ///
    /// - If `galaxy` is not of the seed the plan was made for.
    /// - In debug builds, if a system ID appears twice among the merged hits, as [`range_query`].
    #[must_use]
    pub fn run<C: CellCache>(
        &self,
        galaxy: &Galaxy,
        cache: &mut C,
        sources: &[&dyn SystemSource],
    ) -> RangeResult {
        let hits = self.source_hits(galaxy, sources);
        self.finish(galaxy, cache, sources, &[], hits)
    }
}

/// The sources' expected counts summed layer by layer in value order, so that the sum depends on
/// which counts the sources report and not on the order they are listed in (ruling 23 of
/// 2026-09-22).
///
/// Floating-point addition is not associative: folded in list order, 0.1, 0.2 and 0.3 give
/// 0.600 000 000 000 000 1 in one order and 0.6 in another, and a census near its limit could flip
/// with the list. So each layer's contributions are sorted with [`f64::total_cmp`] and then added
/// from the smallest, starting at 0. Sources then need no identity to be ordered by. An empty list
/// gives [`LayerCounts::ZERO`], as the fold did.
///
/// # Panics
///
/// If a layer's sum is too large for an `f64`, since an expected count must be finite: no sphere
/// the root cube holds has 10³⁰⁸ systems in it, so only a broken source can reach it.
#[must_use]
pub(crate) fn sum_source_counts(per_source: &[LayerCounts]) -> LayerCounts {
    let mut total = LayerCounts::ZERO;
    let mut column = Vec::with_capacity(per_source.len());
    for layer in Layer::ALL {
        column.clear();
        column.extend(per_source.iter().map(|counts| counts.get(layer)));
        column.sort_by(f64::total_cmp);
        total.set(layer, column.iter().fold(0.0, |sum, &count| sum + count));
    }
    total
}

/// Whether no system ID appears twice among `systems`, wherever the two copies were sorted.
///
/// The hits are ordered by distance first, so two copies of one ID need not be neighbours; this
/// sorts a copy of the IDs instead. Only debug builds call it.
#[must_use]
fn ids_are_unique(systems: &[SystemHit]) -> bool {
    let mut ids: Vec<u64> = systems.iter().map(|hit| hit.id().raw()).collect();
    ids.sort_unstable();
    ids.windows(2).all(|pair| pair[0] != pair[1])
}

/// The sphere a layer is walked with: the query's, padded by that layer's speed ([`walk_pad`]),
/// and from generator version 25 for the Kepler regime's orbits near the black hole (plan 09,
/// P09.T28.b.b, held below it).
#[must_use]
pub(crate) fn sphere_for_layer(galaxy: &Galaxy, query: &RangeQuery, layer: Layer) -> QuerySphere {
    sphere_with_pad(
        query,
        walk_pad(galaxy, layer, query.centre(), query.radius(), query.time()),
    )
}

/// The widest sphere any layer is walked with at the straight lines' pads: what the sources are
/// asked about, since they are asked once for the whole query, and what the statistics report.
///
/// The sources pad for their own members' motion, so the grid's Kepler-regime pad, which only the
/// grid's records need, does not widen it (plan 09, P09.T28.b.b).
#[must_use]
pub(crate) fn widest_sphere(query: &RangeQuery) -> QuerySphere {
    let pad = Layer::ALL
        .into_iter()
        .fold(LightYears::ZERO, |most, layer| {
            let pad = pad_for(query.time(), pad_speed(layer));
            if pad.total_cmp(&most).is_gt() {
                pad
            } else {
                most
            }
        });
    sphere_with_pad(query, pad)
}

/// The query's sphere with `pad` light-years of room for motion.
///
/// # Panics
///
/// Never: [`RangeQuery::build`] has already checked that the radius is finite and positive, and a pad
/// over the clock window is a few light-years.
#[must_use]
fn sphere_with_pad(query: &RangeQuery, pad: LightYears) -> QuerySphere {
    QuerySphere::new(*query.centre(), query.radius(), query.time(), pad)
        .expect("a built query has a finite positive radius, and the pad is a few light-years")
}

/// Whether any source replaces this grid system, asked in the order the sources were given and only
/// until one says yes.
#[must_use]
fn suppressed(
    sources: &[&dyn SystemSource],
    galaxy: &Galaxy,
    record: &SystemRecord,
    t: UniverseTime,
) -> bool {
    sources
        .iter()
        .any(|source| source.suppresses(galaxy, record, t))
}

/// How many hits to reserve: the expected total of the admitted layers with room for the Poisson
/// excess, since the realised count is never truncated (plan 03, Design note 9).
#[must_use]
fn reserve_for(census: &Census) -> usize {
    let expected = census
        .layers()
        .iter()
        .fold(0.0, |sum, layer| sum + census.expected().get(layer));
    let wanted = 1.25 * expected + 16.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "this is only a Vec capacity: dropping the fraction is the point, the clamp holds \
                  the magnitude at 2^20, and a total that is not a number saturates to 0"
    )]
    let reserve = wanted.clamp(0.0, f64::from(1_u32 << 20)) as usize;
    reserve
}

/// Orders the hits by distance at the query's time, then by ID, both by a total order, so that the
/// result does not depend on the walk or on the cache (plan 03, Design note 14).
fn sort_hits(systems: &mut [SystemHit]) {
    systems.sort_by(|a, b| {
        a.distance()
            .total_cmp(&b.distance())
            .then_with(|| a.id().raw().cmp(&b.id().raw()))
    });
}

/// A range query's parameters were rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildRangeQueryError {
    /// The radius is NaN or infinite.
    RadiusNotFinite,
    /// The radius is zero or negative.
    RadiusNotPositive,
    /// The radius exceeds the root cube's diagonal, 131,072 × √3 ≈ 227,023 ly, beyond which a
    /// sphere inside the cube can hold nothing more.
    RadiusBeyondRootCube,
    /// The centre lies outside the root cube.
    CentreOutsideRootCube,
    /// The time lies outside the [`ClockWindow`](crate::time::ClockWindow), ±1,000 Julian years
    /// about the epoch, where present positions are guaranteed.
    TimeOutsideClockWindow(UniverseTime),
    /// The mass floor lies below layer A, at a substellar layer the query's
    /// [`SubstellarRequest`] does not ask for (plan 13, Design note 9).
    SubstellarNotRequested,
    /// Substellar layers were asked for with a mass floor that would cut them off: a stellar floor
    /// above layer A, or the brown dwarfs' step with the rogue planets asked for (plan 13, Design
    /// note 9).
    SubstellarBelowFloor,
    /// The observer of an observed mode ([`QueryMode::ObservedFrom`]) lies outside the root cube
    /// (plan 12, P12.T3).
    ObserverOutsideRootCube,
}

impl fmt::Display for BuildRangeQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RadiusNotFinite => f.write_str("the query radius is not finite"),
            Self::RadiusNotPositive => f.write_str("the query radius is not positive"),
            Self::RadiusBeyondRootCube => {
                f.write_str("the query radius exceeds the root cube's diagonal")
            }
            Self::CentreOutsideRootCube => {
                f.write_str("the query centre lies outside the root cube")
            }
            Self::TimeOutsideClockWindow(t) => {
                write!(f, "the query time {t} lies outside the clock window")
            }
            Self::SubstellarNotRequested => {
                f.write_str("the mass floor reaches a substellar layer that was not requested")
            }
            Self::SubstellarBelowFloor => {
                f.write_str("the requested substellar layers lie below the mass floor")
            }
            Self::ObserverOutsideRootCube => {
                f.write_str("the query's observer lies outside the root cube")
            }
        }
    }
}

impl Error for BuildRangeQueryError {}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::NoCache;
    use crate::rng::Seed;

    /// A source that reports fixed expected counts, places nothing and suppresses nothing.
    #[derive(Debug)]
    struct Counted(LayerCounts);

    impl SystemSource for Counted {
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

        fn suppresses(&self, _galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
            false
        }
    }

    /// A source that breaks the contract: it hands back the grid's own nearest system as its member.
    #[derive(Debug)]
    struct Duplicating(SystemHit);

    impl SystemSource for Duplicating {
        fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
            LayerCounts::ZERO
        }

        fn systems_in_sphere(
            &self,
            _galaxy: &Galaxy,
            _sphere: &QuerySphere,
            layers: LayerSet,
            out: &mut Vec<SystemHit>,
        ) {
            if layers.contains(self.0.record().layer()) {
                out.push(self.0);
            }
        }

        fn suppresses(&self, _galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
            false
        }
    }

    fn galaxy() -> Galaxy {
        Galaxy::from_params(
            Seed::new(0x0309_0de4_0000_0000),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// Twelve light-years of the Sun-like point, every layer: about fourteen systems from a few
    /// dozen cells.
    fn small_query() -> RangeQuery {
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        RangeQuery::builder(sun, LightYears::new(12.0))
            .build()
            .unwrap()
    }

    /// Every ordering of three items.
    fn permutations<T: Copy>([a, b, c]: [T; 3]) -> [[T; 3]; 6] {
        [
            [a, b, c],
            [a, c, b],
            [b, a, c],
            [b, c, a],
            [c, a, b],
            [c, b, a],
        ]
    }

    /// Three sources whose plain fold depends on their order, in layers E and C, give a
    /// bit-identical census in every order (ruling 23 of 2026-09-22). The sources' E counts are the
    /// first of a few fixed triples whose plain fold onto the grid's count does depend on the order,
    /// so that the test can tell the two sums apart whatever the grid's count is.
    #[test]
    fn the_census_does_not_depend_on_the_order_of_the_sources() {
        let galaxy = galaxy();
        let query = small_query();
        let grid = expected_counts(
            &galaxy,
            query.centre(),
            query.radius(),
            SubstellarRequest::None,
        );
        let counts_of = |values: [f64; 3]| {
            values.map(|n| {
                let mut counts = LayerCounts::ZERO;
                counts.set(Layer::E, n);
                counts.set(Layer::C, 10.0 * n);
                counts
            })
        };
        // The plain fold in list order, which the sum replaced, does depend on the order for the
        // triple chosen: at least two orders give the census different bits.
        let order_dependent = |values: [f64; 3]| {
            let folded: Vec<f64> = permutations(counts_of(values))
                .iter()
                .map(|order| {
                    let plain = order
                        .iter()
                        .fold(LayerCounts::ZERO, |sum, counts| sum + *counts);
                    (grid + plain).get(Layer::E)
                })
                .collect();
            folded.iter().any(|sum| sum.total_cmp(&folded[0]).is_ne())
        };
        let values = [
            [0.1, 0.2, 0.3],
            [0.1, 0.2, 0.7],
            [0.3, 0.6, 0.7],
            [1e-3, 2e-3, 3e-3],
            [0.01, 0.02, 0.07],
        ]
        .into_iter()
        .find(|&v| order_dependent(v))
        .expect("a plain fold of one of the triples depends on the order, so the test proves it");
        let sources = counts_of(values).map(Counted);

        let reference = range_query(
            &galaxy,
            &mut NoCache::new(),
            &[&sources[0], &sources[1], &sources[2]],
            &query,
        );
        for [a, b, c] in permutations([0, 1, 2]) {
            let listed: [&dyn SystemSource; 3] = [&sources[a], &sources[b], &sources[c]];
            let result = range_query(&galaxy, &mut NoCache::new(), &listed, &query);
            assert_eq!(result.census(), reference.census(), "order {a}, {b}, {c}");
            for (x, y) in result
                .census()
                .expected()
                .to_array()
                .into_iter()
                .zip(reference.census().expected().to_array())
            {
                assert_same_bits(x, y);
            }
            assert_eq!(result.systems(), reference.systems());
        }
        // And the sum is the grid's plus the sources' in value order.
        assert_same_bits(
            reference.census().expected().get(Layer::E),
            grid.get(Layer::E) + ((values[0] + values[1]) + values[2]),
        );
    }

    #[test]
    fn no_source_sums_to_zero_and_one_source_to_its_own_counts() {
        assert_eq!(sum_source_counts(&[]), LayerCounts::ZERO);
        let one = LayerCounts::from_array([1.5, 0.25, 3.0, 0.0, 7.0, 0.0, 0.0]);
        for (x, y) in sum_source_counts(&[one])
            .to_array()
            .into_iter()
            .zip(one.to_array())
        {
            assert_same_bits(x, y);
        }
    }

    #[test]
    fn distinct_ids_are_unique_and_a_repeated_one_is_not() {
        let galaxy = galaxy();
        let result = range_query(&galaxy, &mut NoCache::new(), &[], &small_query());
        let hits = result.systems();
        assert!(
            hits.len() >= 2,
            "twelve light-years of the Sun hold a few systems"
        );
        assert!(ids_are_unique(hits));
        // The repeat lands far from its original, since the hits are ordered by distance first.
        let mut repeated = hits.to_vec();
        repeated.push(hits[0]);
        assert!(!ids_are_unique(&repeated));
    }

    /// A source that returns an ID the grid already placed trips the debug check on the merged
    /// hits.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "a system ID appears twice among the merged hits")]
    fn a_source_that_repeats_a_grid_id_trips_the_debug_check() {
        let galaxy = galaxy();
        let query = small_query();
        let grid = range_query(&galaxy, &mut NoCache::new(), &[], &query);
        let nearest = *grid
            .systems()
            .first()
            .expect("twelve light-years of the Sun hold a system");
        let source = Duplicating(nearest);
        let _merged = range_query(&galaxy, &mut NoCache::new(), &[&source], &query);
    }

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        let t = UniverseTime::from_julian_years(2_000).unwrap();
        for error in [
            BuildRangeQueryError::RadiusNotFinite,
            BuildRangeQueryError::RadiusNotPositive,
            BuildRangeQueryError::RadiusBeyondRootCube,
            BuildRangeQueryError::CentreOutsideRootCube,
            BuildRangeQueryError::TimeOutsideClockWindow(t),
            BuildRangeQueryError::SubstellarNotRequested,
            BuildRangeQueryError::SubstellarBelowFloor,
            BuildRangeQueryError::ObserverOutsideRootCube,
        ] {
            let message = error.to_string();
            let first = message.chars().next().unwrap();
            assert!(!first.is_uppercase(), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
    }

    /// A plan's cells are the cells its walk reads, in the order it reads them, and its walk is
    /// the range query's answer, for queries whose census admits every stellar layer, drops the
    /// finest and reads the substellar layers (P09.T40.a).
    #[test]
    fn a_plan_s_cells_are_its_walk_s_and_its_walk_the_query_s_answer() {
        use std::num::NonZeroU32;

        use crate::galaxy::placement::{CellKey, generate_cell};

        /// Every cell asked for, in order, each generated afresh.
        #[derive(Default)]
        struct Recording(Vec<CellKey>);
        impl CellCache for Recording {
            fn with_cell<R>(
                &mut self,
                galaxy: &Galaxy,
                key: CellKey,
                f: impl FnOnce(&[SystemRecord]) -> R,
            ) -> R {
                self.0.push(key);
                let mut systems = Vec::new();
                generate_cell(galaxy, key, &mut systems);
                f(&systems)
            }
        }
        let galaxy = galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        for (radius, limit, substellar) in [
            (30.0, 20_000, SubstellarRequest::None),
            (200.0, 2_000, SubstellarRequest::None),
            (20.0, 20_000, SubstellarRequest::BrownDwarfs),
        ] {
            let query = RangeQuery::builder(sun, LightYears::new(radius))
                .limit(NonZeroU32::new(limit).unwrap())
                .substellar(substellar)
                .build()
                .unwrap();
            let plan = RangePlan::new(&galaxy, &[], &query);
            let mut walked = Recording::default();
            let result = plan.run(&galaxy, &mut walked, &[]);
            assert_eq!(plan.cells().collect::<Vec<_>>(), walked.0, "{radius} ly");
            assert_eq!(
                u64::try_from(walked.0.len()).unwrap(),
                result.stats().cells_visited()
            );
            assert_eq!(
                result,
                range_query(&galaxy, &mut NoCache::new(), &[], &query)
            );
            assert_eq!(plan.census(), result.census());
            assert_eq!(plan.query(), &query);
        }
    }

    /// A plan made without its sources and then with them is the plan made with them, its census
    /// bit for bit, and walks only cells the plan without them walks, whether the sources' counts
    /// leave the census's layers as they are or drop some (P09.T40.a): what lets a server generate
    /// the cells of the plan without the sources while the sources' own cells are generated.
    #[test]
    fn a_plan_made_with_its_sources_later_is_the_plan_made_with_them() {
        let galaxy = galaxy();
        let query = small_query();
        let unsourced = RangePlan::new(&galaxy, &[], &query);
        let cells: std::collections::BTreeSet<_> = unsourced.cells().collect();
        let mut light = LayerCounts::ZERO;
        light.set(Layer::C, 0.3);
        light.set(Layer::E, 0.1);
        let mut heavy = LayerCounts::ZERO;
        for layer in Layer::ALL {
            heavy.set(layer, f64::from(query.limit().get()) / 3.0);
        }
        for (counts, drops) in [(light, false), (heavy, true)] {
            let source = Counted(counts);
            let sources: [&dyn SystemSource; 1] = [&source];
            let with_them = RangePlan::new(&galaxy, &sources, &query);
            let later = unsourced.with_sources(&galaxy, &sources);
            assert_eq!(later, with_them);
            for (x, y) in later
                .census()
                .expected()
                .to_array()
                .into_iter()
                .zip(with_them.census().expected().to_array())
            {
                assert_same_bits(x, y);
            }
            assert!(with_them.cells().all(|cell| cells.contains(&cell)));
            assert_eq!(
                with_them.census().layers() != unsourced.census().layers(),
                drops,
                "{:?}",
                with_them.census()
            );
        }
    }
}
