//! The merge hook: everything a range query returns that the grid did not place (plan 03,
//! P03.T9.f).
//!
//! The grid places the field, and the brainstorm's other sources sit beside it: the members of large
//! features and of the galactic centre, the catalogue classes carved out of the field, the global
//! list's streams and dwarf cores, and pinned content. None of them is a cell of a layer, but all of
//! them must appear in the same answer, with the same census, so the query takes them through one
//! trait and merges their hits with the grid's.
//!
//! No source exists in the first milestone: a query is passed `&[]`, and the hook is here so that
//! plans 09, 10 and the overlay plan add a source instead of changing [`range_query`](super::range_query).

use super::result::{LayerCounts, LayerSet, SystemHit};
use super::walk::QuerySphere;
use crate::galaxy::Galaxy;
use crate::galaxy::placement::SystemRecord;
use crate::time::UniverseTime;

/// A source of systems the grid does not place.
///
/// The contract, which a query relies on and later plans must keep:
///
/// - **Expected counts are per layer, by the mass band a member would fall in.** A source reports
///   what it would contribute to each layer of the census
///   ([`expected_in_sphere`](Self::expected_in_sphere)), over the sphere's **unpadded** radius and at
///   the epoch, as the grid's counts are. A source that cannot count exactly errs high: that can
///   only drop a layer from the census early, which is the direction the brainstorm accepts for the
///   features' share.
/// - **Hits are already tested.** [`systems_in_sphere`](Self::systems_in_sphere) appends only
///   systems that exist at the sphere's time and lie inside its unpadded radius, with the position
///   they had then, exactly as [`hit_at`](super::hit_at) decides for a grid system. The query does
///   not re-test them; it only sorts them in with the rest.
/// - **A source is asked only for the layers the census admitted**, as a [`LayerSet`], and appends
///   nothing for any other layer. A source that has no member in those layers appends nothing.
/// - **Suppression is the one thing a source may say about another's systems.** A pinned volume
///   holds its own content, so [`suppresses`](Self::suppresses) lets it remove the grid systems
///   inside it. It is asked once per grid system that lies inside the sphere at its time, per
///   source, in the order the sources are given, and only until one says yes, so it must be a pure
///   test on the record and the time and must not depend on how many times or in what order it is
///   asked. It is not subtracted from the expected counts: that errs high, the same direction as
///   above.
/// - **Everything is a pure function of the galaxy and the sphere.** A source may hold precomputed
///   tables built from its galaxy, but nothing that depends on which queries have run.
///
/// The trait may still change before the first release, since the first implementors are plans 09
/// and 10.
///
/// # Examples
///
/// A pinned volume, the simplest source there is: it has no member of its own yet, and it removes the
/// grid systems inside it so that the two cannot both appear.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{NoCache, SystemRecord};
/// use hyperion_sim::galaxy::query::{
///     LayerCounts, LayerSet, QuerySphere, RangeQuery, SystemHit, SystemSource, range_query,
/// };
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::LightYears;
///
/// #[derive(Debug)]
/// struct Pinned {
///     centre: GalacticPosition,
///     radius: LightYears,
/// }
///
/// impl SystemSource for Pinned {
///     fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
///         LayerCounts::ZERO
///     }
///
///     fn systems_in_sphere(
///         &self,
///         _galaxy: &Galaxy,
///         _sphere: &QuerySphere,
///         _layers: LayerSet,
///         _out: &mut Vec<SystemHit>,
///     ) {
///     }
///
///     fn suppresses(&self, _galaxy: &Galaxy, record: &SystemRecord, _t: UniverseTime) -> bool {
///         let away = self.centre.distance_to(record.epoch_position());
///         LightYears::from(away).value() <= self.radius.value()
///     }
/// }
///
/// let galaxy = Galaxy::new(Seed::new(17));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
/// let query = RangeQuery::builder(sun, LightYears::new(20.0)).build()?;
/// let grid = range_query(&galaxy, &mut NoCache::new(), &[], &query);
///
/// let pinned = Pinned { centre: sun, radius: LightYears::new(12.0) };
/// let merged = range_query(&galaxy, &mut NoCache::new(), &[&pinned], &query);
/// // The census is untouched, because suppression is not subtracted from the expected counts, but
/// // the systems inside the volume are gone.
/// assert_eq!(merged.census(), grid.census());
/// assert!(merged.systems().len() < grid.systems().len());
/// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
/// ```
pub trait SystemSource {
    /// What this source expects to contribute to each layer inside the sphere's unpadded radius, at
    /// the epoch.
    fn expected_in_sphere(&self, galaxy: &Galaxy, sphere: &QuerySphere) -> LayerCounts;

    /// Appends this source's systems in `layers` that are inside the sphere at its time.
    fn systems_in_sphere(
        &self,
        galaxy: &Galaxy,
        sphere: &QuerySphere,
        layers: LayerSet,
        out: &mut Vec<SystemHit>,
    );

    /// Whether this source replaces the grid system `record` at time `t`, so that the query leaves
    /// it out.
    fn suppresses(&self, galaxy: &Galaxy, record: &SystemRecord, t: UniverseTime) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::query::{CensusStop, MassFloor, RangeQuery, decide_census};
    use crate::id::Layer;
    use crate::rng::Seed;
    use crate::units::LightYears;

    /// A source that claims a fixed number of systems in one layer and suppresses nothing.
    #[derive(Debug)]
    struct Crowd {
        layer: Layer,
        systems: f64,
    }

    impl SystemSource for Crowd {
        fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
            let mut counts = LayerCounts::ZERO;
            counts.set(self.layer, self.systems);
            counts
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

    fn galaxy() -> Galaxy {
        Galaxy::from_params(
            Seed::new(0x0309_f000_0000_0000),
            GalaxyParams::milky_way_like(),
        )
    }

    fn sphere() -> QuerySphere {
        let centre = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        QuerySphere::new(
            centre,
            LightYears::new(50.0),
            crate::time::UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .unwrap()
    }

    /// A source's counts reach the census and can tip a layer over the limit, which is the whole
    /// point of asking a source before anything is generated.
    #[test]
    fn source_counts_tip_a_layer_over_the_limit_and_reach_the_census() {
        let galaxy = galaxy();
        let sphere = sphere();
        let query = RangeQuery::builder(*sphere.centre(), sphere.radius())
            .mass_floor(MassFloor::LayerA)
            .build()
            .unwrap();
        // A grid that fits every layer comfortably: 1,000 systems in all.
        let grid = LayerCounts::from_array([600.0, 100.0, 200.0, 70.0, 30.0, 0.0, 0.0]);
        let alone = decide_census(&query, &grid, &LayerCounts::ZERO, |_| 1_000);
        assert_eq!(alone.complete_down_to(), Some(Layer::A));
        assert_eq!(alone.stopped_by(), CensusStop::MassFloor);

        // A crowd of 4,000 in layer B takes the running total past the default limit of 4,096.
        let crowd = Crowd {
            layer: Layer::B,
            systems: 4_000.0,
        };
        let counts = crowd.expected_in_sphere(&galaxy, &sphere);
        let with_crowd = decide_census(&query, &grid, &counts, |_| 1_000);
        assert_eq!(with_crowd.complete_down_to(), Some(Layer::C));
        assert_eq!(with_crowd.stopped_by(), CensusStop::Limit);
        // The census reports the grid and the sources together, for every layer.
        assert!((with_crowd.expected().get(Layer::B) - 4_100.0).abs() < 1e-9);
        assert!((with_crowd.expected().get(Layer::A) - 600.0).abs() < 1e-9);
        assert!(!with_crowd.layers().contains(Layer::B));
    }

    #[test]
    fn source_asked_for_no_layer_appends_nothing_and_suppresses_nothing() {
        let galaxy = galaxy();
        let sphere = sphere();
        let crowd = Crowd {
            layer: Layer::E,
            systems: 1.0,
        };
        let mut out = Vec::new();
        crowd.systems_in_sphere(&galaxy, &sphere, LayerSet::EMPTY, &mut out);
        assert!(out.is_empty());
        let mut cell = Vec::new();
        crate::galaxy::placement::generate_cell(
            &galaxy,
            crate::galaxy::placement::CellKey::containing(Layer::C, sphere.centre()).unwrap(),
            &mut cell,
        );
        assert!(!cell.is_empty());
        assert!(!crowd.suppresses(&galaxy, &cell[0], sphere.time()));
    }
}
