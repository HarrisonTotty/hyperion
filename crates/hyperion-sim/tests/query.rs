//! The range query assembled and verified: the grid alone, a source merged in, a source suppressing
//! part of the grid (plan 03, P03.T9.g), and then the whole answer against a brute-force scan,
//! against every cache state, and pinned (P03.T10).
//!
//! What P03.T10 adds here is the query's guarantees, one test each: the systems are exactly those a
//! scan of every cell finds, at the epoch and at both ends of the clock window; the census and the
//! result do not depend on what a cache held, including a cache that evicts at random; overlapping
//! queries agree on the systems they share, in any order; the census says what the brainstorm says it
//! should at the Sun, in the bulge and at the very centre; every ID returned resolves to the system
//! that was returned; and two queries are pinned in a golden file. The statistical check that the
//! realised counts follow `expected_counts` is in `query_stats.rs`, which is slow.

#[expect(
    dead_code,
    reason = "the query tests use the Sun-like point and the brute-force scan alone"
)]
mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;

use common::{brute_force_in_sphere, sunlike_point};
use hyperion_sim::coords::{GalacticPosition, LyCell};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CellCache, CellKey, Existence, NoCache, SystemOrigin, SystemRecord, generate_cell, resolve,
};
use hyperion_sim::galaxy::query::{
    Census, CensusStop, LayerCounts, LayerSet, MassFloor, PAD_SPEED, QuerySphere, RangeQuery,
    RangeResult, SystemHit, SystemSource, cells_in_sphere, count_cells_in_sphere, decide_census,
    expected_counts, pad_for, pad_speed, position_at, range_query,
};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::time::{ClockWindow, UniverseTime};
use hyperion_sim::units::{LightYears, SolarMasses, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

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
        // Nothing moves until plan 08, so no system can show whether a layer was walked with its
        // pad; the cells it visited can. Each layer is walked over its own padded sphere, and a
        // walk that dropped the pad passed every other check here when it was tried in validation.
        let padded_cells: u64 = [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A]
            .into_iter()
            .map(|layer| {
                let pad = pad_for(t, pad_speed(layer));
                let sphere = QuerySphere::new(centre, LightYears::new(20.0), t, pad)
                    .expect("a 20 ly sphere with a pad of a few light-years");
                count_cells_in_sphere(layer, &sphere)
            })
            .sum();
        assert_eq!(
            result.stats().cells_visited(),
            padded_cells,
            "at {years} yr the walk did not cover each layer's padded sphere"
        );
        assert!(result.stats().cells_visited() >= at_epoch.stats().cells_visited());
        if years.abs() == 1_000 {
            // 3.3 ly of pad on 20 ly reaches cells the epoch's sphere does not.
            assert!(
                result.stats().cells_visited() > at_epoch.stats().cells_visited(),
                "a pad of {:?} added no cell at {years} yr",
                pad_for(t, PAD_SPEED)
            );
        }
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

// --- P03.T10: the query verified ---

/// A dozen queries whose census admits every layer: offsets in light-years from the Sun-like point
/// and the radius to ask for.
///
/// The radii stay small enough that the whole answer can be found twice over, once by the query and
/// once by a scan of every cell that could hold a system, in an unoptimised build. Two of them sit
/// well above the plane and one far out in the disc, where the density is a fraction of the Sun's, so
/// that the comparison covers a sphere holding a handful of systems as well as one holding hundreds.
const SCAN_QUERIES: [([f64; 3], f64); 12] = [
    ([0.0, 0.0, 0.0], 6.0),
    ([0.0, 0.0, 0.0], 20.0),
    ([0.0, 0.0, 0.0], 31.0),
    ([4.5, -3.5, 1.5], 12.0),
    ([-60.0, 0.0, 0.0], 16.0),
    ([0.0, 120.0, 0.0], 24.0),
    ([0.0, 0.0, 300.0], 30.0),
    ([0.0, 0.0, -900.0], 40.0),
    ([-2_000.0, 1_000.0, 40.0], 18.0),
    ([12_000.0, 0.0, 0.0], 26.0),
    ([0.0, -12_000.0, 0.0], 22.0),
    ([0.0, 6_000.0, 120.0], 14.0),
];

/// The point `offset` light-years from the Sun-like point.
fn off_the_sun(galaxy: &Galaxy, offset: [f64; 3]) -> GalacticPosition {
    let sun = sunlike_point(galaxy).to_light_years_f64();
    let at = [sun[0] + offset[0], sun[1] + offset[1], sun[2] + offset[2]];
    GalacticPosition::from_light_years(at).expect("a point near the Sun is inside the cube")
}

/// A point in the outer bulge: in the plane, 3,200 ly from the centre at 45° to both axes, where the
/// bulge, the long bar and the nuclear disc still hold about half the systems.
fn outer_bulge() -> GalacticPosition {
    let r = 3_200.0 * 0.5_f64.sqrt();
    GalacticPosition::from_light_years([r, r, 0.0]).expect("3,200 ly is inside the cube")
}

/// The census of `query` decided from its own expected counts, without generating anything: what the
/// query decides before it walks a cell.
fn census_of(galaxy: &Galaxy, query: &RangeQuery) -> Census {
    let grid = expected_counts(galaxy, query.centre(), query.radius());
    decide_census(query, &grid, &LayerCounts::ZERO, |layer| {
        let pad = pad_for(query.time(), pad_speed(layer));
        let sphere = QuerySphere::new(*query.centre(), query.radius(), query.time(), pad)
            .expect("a built query has a finite positive radius");
        count_cells_in_sphere(layer, &sphere)
    })
}

/// The query returns exactly the systems a scan of every cell finds, at the epoch and at both ends of
/// the clock window (P03.T10).
#[test]
fn the_query_returns_exactly_what_a_scan_of_every_cell_finds() {
    let galaxy = galaxy();
    for (offset, radius_ly) in SCAN_QUERIES {
        let centre = off_the_sun(&galaxy, offset);
        let radius = LightYears::new(radius_ly);
        for t in [UniverseTime::EPOCH, ClockWindow::START, ClockWindow::END] {
            let query = RangeQuery::builder(centre, radius)
                .time(t)
                .build()
                .expect("a small sphere inside the clock window");
            let result = run(&galaxy, &[], &query);
            // Only a census that admits every layer can equal a scan that knows no census.
            assert_eq!(
                result.census().complete_down_to(),
                Some(Layer::A),
                "{offset:?} at {radius_ly} ly is not complete to layer A"
            );
            let scanned = brute_force_in_sphere(&galaxy, &centre, radius, t);
            assert_eq!(
                result.systems().len(),
                scanned.len(),
                "{offset:?} at {radius_ly} ly, t = {t}: {} systems against {} scanned",
                result.systems().len(),
                scanned.len()
            );
            for (found, expected) in result.systems().iter().zip(&scanned) {
                assert_eq!(
                    found.id(),
                    expected.id(),
                    "{offset:?} at {radius_ly} ly, t = {t}"
                );
                assert_same_bits(found.distance().value(), expected.distance().value());
                assert_eq!(found.record(), expected.record());
            }
        }
    }
}

/// A cache that keeps cells and drops a random one before every call, on a fixed seed, so that a
/// query meets an arbitrary mixture of hits and misses.
#[derive(Debug)]
struct RandomEvictingCache {
    store: BTreeMap<CellKey, Vec<SystemRecord>>,
    lcg: Lcg,
}

impl RandomEvictingCache {
    fn new(seed: u64) -> Self {
        Self {
            store: BTreeMap::new(),
            lcg: Lcg::new(seed),
        }
    }
}

impl CellCache for RandomEvictingCache {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let held = u64::try_from(self.store.len()).expect("a few thousand cells");
        if held > 0 && self.lcg.next_u64().is_multiple_of(2) {
            let victim = usize::try_from(self.lcg.next_below(held)).expect("an index of the store");
            let key = *self
                .store
                .keys()
                .nth(victim)
                .expect("the index is below the store's length");
            self.store.remove(&key);
        }
        let systems = self.store.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        });
        f(systems)
    }
}

/// The census, the systems, their order and the statistics are the same whatever a cache held, cold,
/// warm, evicting everything or evicting at random (P03.T10).
///
/// This is the brainstorm's "deterministic and does not depend on what happens to be cached", and
/// the reason a cache may evict at any moment, including between two cells of one query.
#[test]
fn the_census_and_the_result_do_not_depend_on_the_cache() {
    let galaxy = galaxy();
    for (offset, radius_ly) in [
        ([0.0, 0.0, 0.0], 31.0),
        ([0.0, 0.0, -900.0], 40.0),
        ([-2_000.0, 1_000.0, 40.0], 18.0),
    ] {
        let centre = off_the_sun(&galaxy, offset);
        let query = RangeQuery::builder(centre, LightYears::new(radius_ly))
            .time(ClockWindow::END)
            .build()
            .expect("a small sphere at the end of the clock window");
        let cold = range_query(&galaxy, &mut NoCache::new(), &[], &query);
        assert!(!cold.systems().is_empty(), "{offset:?} held no system");

        let mut warm = WarmCache::default();
        let first = range_query(&galaxy, &mut warm, &[], &query);
        let second = range_query(&galaxy, &mut warm, &[], &query);
        assert_eq!(first, cold, "a cold toy cache");
        assert_eq!(second, cold, "a warm toy cache");
        assert_eq!(
            range_query(&galaxy, &mut EvictingCache::default(), &[], &query),
            cold,
            "a cache that evicts everything it stores"
        );
        let mut random = RandomEvictingCache::new(0x0310_e71c);
        assert_eq!(
            range_query(&galaxy, &mut random, &[], &query),
            cold,
            "a cache that evicts at random"
        );
        assert_eq!(
            range_query(&galaxy, &mut random, &[], &query),
            cold,
            "a cache that evicts at random, warmed by the same query"
        );
        // And the census itself, decided before anything is generated, is the same number.
        assert_eq!(census_of(&galaxy, &query), *cold.census());
    }
}

/// Two overlapping queries agree on every system they share, in either order and alone (P03.T10).
#[test]
fn overlapping_queries_agree_on_the_systems_they_share() {
    let galaxy = galaxy();
    let radius = LightYears::new(25.0);
    let queries: Vec<RangeQuery> = [[0.0, 0.0, 0.0], [20.0, 0.0, 0.0], [0.0, -18.0, 6.0]]
        .into_iter()
        .map(|offset| {
            RangeQuery::builder(off_the_sun(&galaxy, offset), radius)
                .build()
                .expect("a 25 ly query near the Sun")
        })
        .collect();
    let alone: Vec<RangeResult> = queries
        .iter()
        .map(|query| range_query(&galaxy, &mut NoCache::new(), &[], query))
        .collect();

    // Every pair, each way round, through one cache the pair shares.
    for (i, first) in queries.iter().enumerate() {
        for (j, second) in queries.iter().enumerate() {
            if i == j {
                continue;
            }
            let mut shared = WarmCache::default();
            let led = range_query(&galaxy, &mut shared, &[], first);
            let followed = range_query(&galaxy, &mut shared, &[], second);
            assert_eq!(led, alone[i], "query {i} led");
            assert_eq!(followed, alone[j], "query {j} followed query {i}");
            // The systems the two spheres share come back identical, distance apart.
            let mut shared_systems = 0;
            for hit in led.systems() {
                if let Some(also) = followed
                    .systems()
                    .iter()
                    .find(|other| other.id() == hit.id())
                {
                    assert_eq!(also.record(), hit.record());
                    assert_eq!(also.position(), hit.position());
                    shared_systems += 1;
                }
            }
            assert!(
                shared_systems > 0,
                "queries {i} and {j} overlap but share no system"
            );
        }
    }
}

/// The census in place: complete to layer A at the Sun-like point, stopping short of it in the bulge,
/// and admitting nothing at the very centre (P03.T10; brainstorm, "The range query").
#[test]
fn the_census_follows_the_density_from_the_sun_to_the_galactic_centre() {
    let galaxy = galaxy();
    let fifty = LightYears::new(50.0);

    // At the Sun-like point a 50 ly query under the default limit is complete down to layer A, with
    // about 1,600 systems at the brainstorm's reference density and about a thousand at the Milky
    // Way's own, which is this fixture's.
    let at_sun = RangeQuery::builder(sunlike_point(&galaxy), fifty)
        .build()
        .expect("50 ly at the Sun-like point is a query");
    let result = run(&galaxy, &[], &at_sun);
    assert_eq!(result.census().complete_down_to(), Some(Layer::A));
    assert_eq!(result.census().stopped_by(), CensusStop::MassFloor);
    let expected: f64 = Layer::ALL.iter().fold(0.0, |sum, &layer| {
        sum + result.census().expected().get(layer)
    });
    let found = u64::try_from(result.systems().len()).expect("a few thousand systems");
    println!(
        "50 ly at the Sun-like point: {found} systems of {expected:.0} expected, \
         {} cells visited",
        result.stats().cells_visited()
    );
    assert!(
        (500.0..4_096.0).contains(&expected),
        "{expected} systems expected within 50 ly of the Sun-like point"
    );
    assert_poisson_count(
        "systems within 50 ly of the Sun-like point",
        found,
        expected,
        ALPHA,
    );
    assert_same_bits(
        result
            .census()
            .complete_above()
            .expect("layer A fits")
            .value(),
        0.08,
    );

    // In the outer bulge the same query stops above layer A and says which rule stopped it.
    let in_bulge = RangeQuery::builder(outer_bulge(), fifty)
        .build()
        .expect("50 ly in the outer bulge is a query");
    let result = run(&galaxy, &[], &in_bulge);
    let complete = result
        .census()
        .complete_down_to()
        .expect("the coarse layers fit in the outer bulge");
    assert_ne!(complete, Layer::A, "the census did not stop above layer A");
    assert_eq!(result.census().stopped_by(), CensusStop::Limit);
    assert!(
        result
            .census()
            .complete_above()
            .is_some_and(|m| m.value() > 0.5),
        "the bulge census is complete above {:?}",
        result.census().complete_above()
    );
    for hit in result.systems() {
        assert!(result.census().layers().contains(hit.record().layer()));
    }

    // Deeper in, where a 50 ly sphere holds hundreds of thousands of systems, the census is decided
    // from the expected counts alone and admits at most the coarsest layer or two.
    let inner = GalacticPosition::from_light_years([707.0, 707.0, 0.0]).expect("1,000 ly out");
    let in_inner_bulge = RangeQuery::builder(inner, fifty)
        .build()
        .expect("50 ly in the bulge is a query");
    let census = census_of(&galaxy, &in_inner_bulge);
    assert!(
        matches!(census.complete_down_to(), Some(Layer::E | Layer::D)),
        "the inner bulge census is complete down to {:?}",
        census.complete_down_to()
    );
    assert_eq!(census.stopped_by(), CensusStop::Limit);

    // At the very centre nothing fits, so nothing is generated and the caller must shrink the
    // radius (brainstorm, "The range query").
    let at_centre = RangeQuery::builder(GalacticPosition::ORIGIN, fifty)
        .build()
        .expect("50 ly at the centre is a query");
    let result = run(&galaxy, &[], &at_centre);
    assert_eq!(result.census().complete_down_to(), None);
    assert_eq!(result.census().complete_above(), None);
    assert_eq!(result.census().stopped_by(), CensusStop::Limit);
    assert_eq!(result.census().layers(), LayerSet::EMPTY);
    assert!(result.systems().is_empty());
    assert_eq!(result.stats().cells_visited(), 0, "a cell was generated");
    assert!(result.census().expected().get(Layer::E) > 4_096.0);
}

/// Every system a query returns resolves from its ID alone, is where the query says it is, and is
/// inside the sphere and born at the query's time (P03.T10).
#[test]
fn every_system_returned_resolves_and_passes_the_querys_own_tests() {
    let galaxy = galaxy();
    for (offset, radius_ly) in [([0.0, 0.0, 0.0], 31.0), ([0.0, 0.0, -900.0], 40.0)] {
        for t in [UniverseTime::EPOCH, ClockWindow::START, ClockWindow::END] {
            let centre = off_the_sun(&galaxy, offset);
            let query = RangeQuery::builder(centre, LightYears::new(radius_ly))
                .time(t)
                .build()
                .expect("a small sphere inside the clock window");
            let result = run(&galaxy, &[], &query);
            assert!(!result.systems().is_empty());
            for hit in result.systems() {
                // An ID is all a save keeps, and it must give the record back.
                assert_eq!(resolve(&galaxy, hit.id()), Ok(*hit.record()));
                assert_eq!(hit.record().existence_at(t), Existence::Exists);
                assert!(hit.record().age_at(t).value() > 0.0);
                assert_eq!(hit.position(), &position_at(&galaxy, hit.record(), t));
                assert_same_bits(
                    hit.distance().value(),
                    LightYears::from(centre.distance_to(hit.position())).value(),
                );
                assert!(
                    hit.distance().value() <= radius_ly,
                    "{:?} lies {:?} out, past {radius_ly} ly",
                    hit.id(),
                    hit.distance()
                );
                assert!(result.census().layers().contains(hit.record().layer()));
            }
        }
    }
}

/// Writes one query's census, statistics and systems in order.
fn write_query(w: &mut GoldenWriter, galaxy: &Galaxy, label: &str, query: &RangeQuery) {
    let result = range_query(galaxy, &mut NoCache::new(), &[], query);
    let [x, y, z] = query.centre().to_light_years_f64();
    let census = result.census();
    w.line(&format!(
        "# {label}: R = {} ly at ({x}, {y}, {z}) ly, t = {}, floor {:?}",
        query.radius().value(),
        query.time(),
        query.mass_floor()
    ));
    w.line(&format!(
        "{label}.complete_down_to = {}",
        census
            .complete_down_to()
            .map_or_else(|| "none".to_owned(), |layer| layer.letter().to_string())
    ));
    w.line(&format!("{label}.stopped_by = {:?}", census.stopped_by()));
    for layer in Layer::ALL {
        w.f64(
            &format!("{label}.expected.{}", layer.letter()),
            census.expected().get(layer),
        );
    }
    w.line(&format!(
        "{label}.cells_visited = {}",
        result.stats().cells_visited()
    ));
    w.line(&format!(
        "{label}.systems_examined = {}",
        result.stats().systems_examined()
    ));
    w.f64(
        &format!("{label}.padded_radius_ly"),
        result.stats().padded_radius().value(),
    );
    w.line(&format!("{label}.systems = {}", result.systems().len()));
    for (index, hit) in result.systems().iter().enumerate() {
        w.u64_hex(&format!("{label}.hit{index}.id"), hit.id().raw());
        w.f64(
            &format!("{label}.hit{index}.distance_ly"),
            hit.distance().value(),
        );
    }
}

/// Two pinned queries: their census, their expected counts, what they cost and every system they
/// return, in order (P03.T10).
///
/// The first is the query the plan's figures describe, a 20 ly sphere at the Sun-like point at the
/// epoch. The second is everything else at once: a denser place, a time away from the epoch, and a
/// mass floor, so that the pad, the floor's stop and a partial walk are pinned too.
#[test]
fn pinned_queries_are_golden() {
    let galaxy = galaxy();
    let local = RangeQuery::builder(sunlike_point(&galaxy), LightYears::new(20.0))
        .build()
        .expect("20 ly at the Sun-like point is a query");
    let five_centuries = UniverseTime::from_julian_years(500).expect("inside the clock window");
    let bulge = RangeQuery::builder(outer_bulge(), LightYears::new(12.0))
        .time(five_centuries)
        .mass_floor(MassFloor::LayerC)
        .build()
        .expect("12 ly in the outer bulge is a query");
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    write_query(&mut w, &galaxy, "sun_20ly", &local);
    write_query(&mut w, &galaxy, "bulge_12ly_floor_c_t500", &bulge);
    golden!("query/range", w.as_str());
}
