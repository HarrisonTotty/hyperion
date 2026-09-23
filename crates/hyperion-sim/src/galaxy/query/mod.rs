//! The range query: which systems lie within R light-years of a point at time t (plan 03).
//!
//! The brainstorm's "The range query". The query walks the stellar layers from the coarsest to the
//! finest, visiting only the cells that meet the sphere, and returns a complete census or nothing
//! per layer: whether a layer fits under the caller's limit is decided before anything is
//! generated, from its expected count over the sphere, so the answer never depends on what a
//! caller happens to have cached. Cells are chosen by epoch position, so the sphere is padded by
//! the largest speed times |t|; distances are then tested at t and systems not yet born are
//! dropped. Sources other than the grid (features, the global list, catalogue classes, pinned
//! content) are merged through a hook that later plans fill.
//!
//! Queries take a time inside the [`ClockWindow`](crate::time::ClockWindow), within
//! ±[`CLOCK_WINDOW_H`](crate::time::CLOCK_WINDOW_H) = ±1,000 Julian years of the epoch, where
//! present positions are guaranteed (plan 03, Design note 12).

mod census;
mod expected;
mod motion;
mod request;
mod result;
mod source;
mod walk;

use std::error::Error;
use std::fmt;

pub use census::decide_census;
pub use expected::expected_counts;
pub use motion::{PAD_SPEED, epoch_velocity, hit_at, pad_for, pad_speed, position_at};
pub use request::{
    DEFAULT_CELL_BUDGET, DEFAULT_CENSUS_LIMIT, MassFloor, RangeQuery, RangeQueryBuilder,
    SubstellarRequest,
};
pub use result::{Census, CensusStop, LayerCounts, LayerSet, QueryStats, RangeResult, SystemHit};
pub use source::SystemSource;
pub use walk::{BuildQuerySphereError, QuerySphere, cells_in_sphere, count_cells_in_sphere};

use crate::galaxy::Galaxy;
use crate::galaxy::placement::{CellCache, STELLAR_LAYERS, SystemRecord};
use crate::id::Layer;
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
    let widest = widest_sphere(query);
    let grid = expected_counts(galaxy, query.centre(), query.radius());
    let per_source: Vec<LayerCounts> = sources
        .iter()
        .map(|source| source.expected_in_sphere(galaxy, &widest))
        .collect();
    let from_sources = sum_source_counts(&per_source);
    let census = decide_census(query, &grid, &from_sources, |layer| {
        count_cells_in_sphere(layer, &sphere_for_layer(query, layer))
    });
    let layers = census.layers();

    let mut stats = QueryStats {
        padded_radius: widest.padded_radius(),
        ..QueryStats::default()
    };
    let mut systems = Vec::with_capacity(reserve_for(&census));
    // Coarsest first, as the brainstorm's walk goes; the result's order is the sort's, not this.
    for spec in STELLAR_LAYERS {
        let layer = spec.layer();
        if !layers.contains(layer) {
            continue;
        }
        // Each layer is walked with its own pad, so raising one layer's padding speed (plan 08's
        // unbound class) moves no other layer's cells (plan 03, Design note 13).
        let sphere = sphere_for_layer(query, layer);
        for key in cells_in_sphere(layer, &sphere) {
            stats.cells_visited += 1;
            cache.with_cell(galaxy, key, |cell| {
                for record in cell {
                    stats.systems_examined += 1;
                    if let Some(hit) = hit_at(galaxy, record, &sphere)
                        && !suppressed(sources, galaxy, record, sphere.time())
                    {
                        systems.push(hit);
                    }
                }
            });
        }
    }

    let from_grid = systems.len();
    for source in sources {
        source.systems_in_sphere(galaxy, &widest, layers, &mut systems);
    }
    // A source appends, by its contract, so this is what it added; `saturating_sub` keeps a source
    // that broke the contract from poisoning the count.
    debug_assert!(
        systems.len() >= from_grid,
        "a source removed hits from the buffer instead of appending to it"
    );
    stats.systems_examined += u64::try_from(systems.len().saturating_sub(from_grid))
        .expect("a hit count fits in 64 bits on every target");
    sort_hits(&mut systems);
    // The sort is a strict order only while no ID appears twice, which the sources' contract
    // promises and a debug build checks.
    debug_assert!(
        ids_are_unique(&systems),
        "a system ID appears twice among the merged hits: a source broke the contract that its IDs \
         are its own"
    );
    RangeResult::new(systems, census, stats)
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

/// The sphere a layer is walked with: the query's, padded by that layer's speed.
#[must_use]
fn sphere_for_layer(query: &RangeQuery, layer: Layer) -> QuerySphere {
    sphere_with_pad(query, pad_for(query.time(), pad_speed(layer)))
}

/// The widest sphere any layer is walked with: what the sources are asked about, since they are asked
/// once for the whole query, and what the statistics report.
#[must_use]
fn widest_sphere(query: &RangeQuery) -> QuerySphere {
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
    /// Substellar layers were asked for; plan 13 places them.
    SubstellarLayersUnavailable,
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
            Self::SubstellarLayersUnavailable => {
                f.write_str("substellar layers are not generated yet")
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

    /// Three sources whose plain fold depends on their order, 0.1, 0.2 and 0.3 in layers E and C,
    /// give a bit-identical census in every order (ruling 23 of 2026-09-22).
    #[test]
    fn the_census_does_not_depend_on_the_order_of_the_sources() {
        let galaxy = galaxy();
        let query = small_query();
        let counts = [0.1, 0.2, 0.3].map(|n| {
            let mut counts = LayerCounts::ZERO;
            counts.set(Layer::E, n);
            counts.set(Layer::C, 10.0 * n);
            counts
        });
        let sources = counts.map(Counted);
        let grid = expected_counts(&galaxy, query.centre(), query.radius());

        // The plain fold in list order, which the sum replaced, does depend on the order: at least
        // two orders give the census different bits, so the test can tell the two sums apart.
        let folded: Vec<f64> = permutations(counts)
            .iter()
            .map(|order| {
                let plain = order
                    .iter()
                    .fold(LayerCounts::ZERO, |sum, counts| sum + *counts);
                (grid + plain).get(Layer::E)
            })
            .collect();
        assert!(
            folded.iter().any(|sum| sum.total_cmp(&folded[0]).is_ne()),
            "the plain fold gave one census in every order, so this test proves nothing"
        );

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
            grid.get(Layer::E) + ((0.1 + 0.2) + 0.3),
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
            BuildRangeQueryError::SubstellarLayersUnavailable,
        ] {
            let message = error.to_string();
            let first = message.chars().next().unwrap();
            assert!(!first.is_uppercase(), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
    }
}
