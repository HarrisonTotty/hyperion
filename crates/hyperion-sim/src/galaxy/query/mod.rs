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
/// answer in the first milestone. The result's systems are **not** bounded by the query's limit:
/// the limit bounds a layer's expected count, and the realised count is never truncated (plan 03,
/// Design note 9), so a caller sizing a buffer leaves room for the Poisson excess.
///
/// # Panics
///
/// If a component's expected count is not a number, which plan 02's densities cannot produce.
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
    let from_sources = sources.iter().fold(LayerCounts::ZERO, |sum, source| {
        sum + source.expected_in_sphere(galaxy, &widest)
    });
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
    RangeResult::new(systems, census, stats)
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
    use super::*;

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
