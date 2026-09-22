//! The range query assembled: the grid alone, a source merged in, and a source suppressing part of
//! the grid (plan 03, P03.T9.g).
//!
//! The wider verification — equality with a brute-force scan, the census in place, the same answer
//! whatever the cache, the statistics and the goldens — is P03.T10's.

#[expect(dead_code, reason = "the query tests use only the Sun-like point")]
mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;

use common::sunlike_point;
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, LyCell};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CellCache, CellKey, NoCache, SystemOrigin, SystemRecord, generate_cell,
};
use hyperion_sim::galaxy::query::{
    CensusStop, LayerCounts, LayerSet, QuerySphere, RangeQuery, RangeResult, SystemHit,
    SystemSource, cells_in_sphere, expected_counts, range_query,
};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, SolarMasses, Years};
use hyperion_testkit::order::assert_order_independent;

/// The seed of the galaxy these queries run in.
const SEED: u64 = 0x0309_9000_0000_0000;

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
}

/// A 20 ly query at the Sun-like point, at the epoch.
fn local_query(galaxy: &Galaxy) -> RangeQuery {
    RangeQuery::builder(sunlike_point(galaxy), LightYears::new(20.0))
        .build()
        .expect("20 ly at the Sun-like point is a query")
}

fn run(galaxy: &Galaxy, sources: &[&dyn SystemSource], query: &RangeQuery) -> RangeResult {
    range_query(galaxy, &mut NoCache::new(), sources, query)
}

/// A source with a fixed list of systems, which it reports in the layer of each one's ID.
#[derive(Debug, Default)]
struct Fake {
    hits: Vec<SystemHit>,
    /// Grid systems within this many light-years of the sphere's centre are suppressed.
    suppress_within: Option<LightYears>,
}

impl SystemSource for Fake {
    fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
        let mut counts = LayerCounts::ZERO;
        for hit in &self.hits {
            let layer = hit.record().layer();
            counts.set(layer, counts.get(layer) + 1.0);
        }
        counts
    }

    fn systems_in_sphere(
        &self,
        _galaxy: &Galaxy,
        sphere: &QuerySphere,
        layers: LayerSet,
        out: &mut Vec<SystemHit>,
    ) {
        for hit in &self.hits {
            if layers.contains(hit.record().layer())
                && hit.distance().value() <= sphere.radius().value()
            {
                out.push(*hit);
            }
        }
    }

    fn suppresses(&self, _galaxy: &Galaxy, record: &SystemRecord, _t: UniverseTime) -> bool {
        self.suppress_within.is_some_and(|ball| {
            let centre = GalacticPosition::new(LyCell::new([0, 26_000, 0]), [0.0; 3])
                .expect("the Sun-like light-year");
            LightYears::from(centre.distance_to(record.epoch_position())).value() <= ball.value()
        })
    }
}

/// A hit of `layer` at `distance` light-years, with an ID from a cell far from the query.
fn fake_hit(galaxy: &Galaxy, layer: Layer, index: u32, distance: f64) -> SystemHit {
    let key = CellKey::new(layer, [-100, -100, -100]).expect("inside the cube");
    let position = GalacticPosition::from_light_years([0.0, 26_000.0 + distance, 0.0])
        .expect("inside the cube");
    let record = SystemRecord::from_parts(
        key.candidate_id(index).expect("a small index"),
        position,
        SystemOrigin::Grid(galaxy.fields().component_id(0).expect("a first component")),
        Population::OldThinDisc,
        SolarMasses::new(1.0),
        Years::new(5.0e9),
    );
    SystemHit::new(record, position, LightYears::new(distance))
}

#[test]
fn with_no_source_the_query_is_the_grid_alone() {
    let galaxy = galaxy();
    let query = local_query(&galaxy);
    let result = run(&galaxy, &[], &query);

    // The census admits every layer down to A, and its counts are the grid's alone.
    assert_eq!(result.census().complete_down_to(), Some(Layer::A));
    assert_eq!(result.census().stopped_by(), CensusStop::MassFloor);
    let grid = expected_counts(&galaxy, query.centre(), query.radius());
    for layer in Layer::ALL {
        let (taken, expected) = (result.census().expected().get(layer), grid.get(layer));
        assert!(
            (taken - expected).abs() <= 1e-9 * expected.max(1.0),
            "{layer:?}: {taken} against {expected}"
        );
    }

    // Every hit is a grid system of an admitted layer, inside the radius, in distance order.
    assert!(!result.systems().is_empty());
    let mut nearer = 0.0;
    for hit in result.systems() {
        assert!(result.census().layers().contains(hit.record().layer()));
        assert!(hit.distance().value() <= 20.0);
        assert!(hit.distance().value() >= nearer, "hits are not sorted");
        nearer = hit.distance().value();
        assert_eq!(hit.position(), hit.record().epoch_position());
        assert!(hit.record().component().is_some(), "a grid system's origin");
    }

    // And it is exactly what walking the cells by hand gives.
    let mut by_hand = Vec::new();
    let mut cell = Vec::new();
    let sphere = QuerySphere::new(
        *query.centre(),
        query.radius(),
        query.time(),
        LightYears::ZERO,
    )
    .expect("a sphere at the epoch needs no pad");
    for layer in [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A] {
        for key in cells_in_sphere(layer, &sphere) {
            generate_cell(&galaxy, key, &mut cell);
            for record in &cell {
                let distance =
                    LightYears::from(query.centre().distance_to(record.epoch_position()));
                if distance.value() <= 20.0 {
                    by_hand.push(record.id());
                }
            }
        }
    }
    by_hand.sort_unstable_by_key(|id| id.raw());
    let mut found: Vec<_> = result.systems().iter().map(SystemHit::id).collect();
    found.sort_unstable_by_key(|id| id.raw());
    assert_eq!(found, by_hand);
    assert_eq!(result.stats().cells_visited(), {
        let mut cells = 0;
        for layer in [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A] {
            cells += cells_in_sphere(layer, &sphere).count();
        }
        u64::try_from(cells).unwrap()
    });
}

#[test]
fn a_source_adds_its_hits_to_the_layers_the_census_admitted() {
    let galaxy = galaxy();
    let query = local_query(&galaxy);
    let grid_only = run(&galaxy, &[], &query);

    let source = Fake {
        hits: vec![
            fake_hit(&galaxy, Layer::E, 1, 3.5),
            fake_hit(&galaxy, Layer::A, 2, 0.25),
            // Outside the radius: a source's own test must drop it, and so must the merge.
            fake_hit(&galaxy, Layer::C, 3, 40.0),
        ],
        suppress_within: None,
    };
    let merged = run(&galaxy, &[&source], &query);

    assert_eq!(merged.systems().len(), grid_only.systems().len() + 2);
    // The source's two are in the result, at the right distances, and sorted in with the grid's.
    let ids: Vec<_> = merged.systems().iter().map(SystemHit::id).collect();
    assert!(ids.contains(&source.hits[0].id()));
    assert!(ids.contains(&source.hits[1].id()));
    assert!(!ids.contains(&source.hits[2].id()));
    // The nearest system in the result is the source's 0.25 ly one.
    assert_eq!(merged.systems()[0].id(), source.hits[1].id());
    let mut nearer = 0.0;
    for hit in merged.systems() {
        assert!(hit.distance().value() >= nearer);
        nearer = hit.distance().value();
    }
    // Its counts reach the census, layer by layer, above the grid's.
    for layer in [Layer::A, Layer::E] {
        let with = merged.census().expected().get(layer);
        let without = grid_only.census().expected().get(layer);
        assert!((with - without - 1.0).abs() < 1e-9, "{layer:?}");
    }
    assert!(
        (merged.census().expected().get(Layer::C)
            - grid_only.census().expected().get(Layer::C)
            - 1.0)
            .abs()
            < 1e-9
    );
    // Both sources' systems and the grid's are counted as examined.
    assert!(merged.stats().systems_examined() > grid_only.stats().systems_examined());
}

#[test]
fn a_suppressor_removes_the_grid_systems_inside_its_ball_and_nothing_else() {
    let galaxy = galaxy();
    let query = local_query(&galaxy);
    let grid_only = run(&galaxy, &[], &query);

    let ball = LightYears::new(12.0);
    let source = Fake {
        hits: Vec::new(),
        suppress_within: Some(ball),
    };
    let suppressed = run(&galaxy, &[&source], &query);

    let kept: Vec<_> = suppressed.systems().iter().map(SystemHit::id).collect();
    let mut removed = 0;
    for hit in grid_only.systems() {
        let centre = GalacticPosition::new(LyCell::new([0, 26_000, 0]), [0.0; 3]).unwrap();
        let inside = LightYears::from(centre.distance_to(hit.record().epoch_position())).value()
            <= ball.value();
        assert_eq!(
            !kept.contains(&hit.id()),
            inside,
            "{:?} at {:?} was wrongly kept or dropped",
            hit.id(),
            hit.distance()
        );
        removed += usize::from(inside);
    }
    assert!(removed > 0, "the ball held no system");
    assert_eq!(
        suppressed.systems().len(),
        grid_only.systems().len() - removed
    );
    // Suppression is not subtracted from the expected counts, which err high by design.
    for layer in Layer::ALL {
        let (with, without) = (
            suppressed.census().expected().get(layer),
            grid_only.census().expected().get(layer),
        );
        assert!((with - without).abs() < 1e-9, "{layer:?}");
    }
}

#[test]
fn a_query_at_the_edges_of_the_clock_window_still_finds_its_systems() {
    let galaxy = galaxy();
    let centre = sunlike_point(&galaxy);
    let at_epoch = run(&galaxy, &[], &local_query(&galaxy));
    for years in [-1_000_i64, -100, 100, 1_000] {
        let t = UniverseTime::from_julian_years(years).expect("inside the window");
        let query = RangeQuery::builder(centre, LightYears::new(20.0))
            .time(t)
            .build()
            .expect("a time inside the window");
        let result = run(&galaxy, &[], &query);
        // Nothing moves yet, so the same systems are found; the pad only widens the walk.
        let ids: Vec<_> = result.systems().iter().map(SystemHit::id).collect();
        let epoch_ids: Vec<_> = at_epoch.systems().iter().map(SystemHit::id).collect();
        assert_eq!(ids, epoch_ids, "at {years} yr");
        assert!(
            result.stats().padded_radius().value() > 20.0,
            "the sphere was not padded at {years} yr"
        );
        assert!(result.stats().cells_visited() >= at_epoch.stats().cells_visited());
    }
}

/// A cache that keeps every cell, so a second query over the same ground is all hits.
#[derive(Debug, Default)]
struct WarmCache {
    store: BTreeMap<CellKey, Vec<SystemRecord>>,
}

impl CellCache for WarmCache {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let systems = self.store.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        });
        f(systems)
    }
}

/// A cache that stores each cell and drops it again, so every call is a miss.
#[derive(Debug, Default)]
struct EvictingCache {
    held: Option<Vec<SystemRecord>>,
}

impl CellCache for EvictingCache {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let mut systems = self.held.take().unwrap_or_default();
        generate_cell(galaxy, key, &mut systems);
        self.held = Some(systems);
        let result = f(self.held.as_deref().expect("just stored"));
        self.held = None;
        result
    }
}

/// The census, the systems and their order do not depend on what a cache held, and asking for one
/// sphere does not change the answer for an overlapping one.
///
/// P03.T10 owns the full set of these checks, over many centres and radii and with a cache that
/// evicts at random; this is the guard that comes with the assembly itself.
#[test]
fn the_answer_does_not_depend_on_the_cache_or_on_what_was_asked_before() {
    let galaxy = galaxy();
    // Four overlapping 20 ly spheres in a row at the Sun-like point, which share most of their cells.
    let sun = sunlike_point(&galaxy).to_light_years_f64();
    let offsets = [0.0, 8.0, -8.0, 24.0];
    let queries: Vec<RangeQuery> = offsets
        .iter()
        .map(|&dx| {
            let at = GalacticPosition::from_light_years([sun[0] + dx, sun[1], sun[2]])
                .expect("in the cube");
            RangeQuery::builder(at, LightYears::new(20.0))
                .build()
                .expect("a 20 ly query")
        })
        .collect();

    for query in &queries {
        let cold = range_query(&galaxy, &mut NoCache::new(), &[], query);
        // A cache warmed by the same query, one warmed by its neighbours first, and one that evicts
        // everything it stores: all three give the same result, census and order included.
        let mut warm = WarmCache::default();
        let first = range_query(&galaxy, &mut warm, &[], query);
        let second = range_query(&galaxy, &mut warm, &[], query);
        assert_eq!(first, cold);
        assert_eq!(second, cold);
        let mut neighbours = WarmCache::default();
        for other in &queries {
            let _ = range_query(&galaxy, &mut neighbours, &[], other);
        }
        assert_eq!(range_query(&galaxy, &mut neighbours, &[], query), cold);
        assert_eq!(
            range_query(&galaxy, &mut EvictingCache::default(), &[], query),
            cold
        );
    }

    // And the same query gives the same answer whatever was asked before it, in any order.
    let shared = RefCell::new(WarmCache::default());
    assert_order_independent(&queries, |query| {
        range_query(&galaxy, &mut *shared.borrow_mut(), &[], query)
    });
}
