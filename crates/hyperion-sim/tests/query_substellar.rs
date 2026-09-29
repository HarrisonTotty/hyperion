//! The range query with the substellar layers (plan 13, P13.T4): the default request is untouched,
//! a lowered floor returns the brown dwarfs and the rogue planets in the counts the census expects,
//! the census drops a layer whole when its expectation would pass the limit, the rogue planets'
//! census counts their saturated density at the centre (ruling 125), and moving the query in time
//! moves only the objects crossing the sphere.

#[expect(
    dead_code,
    reason = "the substellar query tests use the Sun-like and reference points and the warm cache alone"
)]
mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::sync::OnceLock;

use common::{WarmCellCache, reference_density_point, sunlike_point};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{NoCache, SystemKind, rogue_planet_saturation_density};
use hyperion_sim::galaxy::query::{
    CensusStop, MassFloor, PAD_SPEED, QuerySphere, RangeQuery, RangeResult, SubstellarRequest,
    count_cells_in_sphere, pad_for, range_query,
};
use hyperion_sim::galaxy::substellar::SubstellarParams;
use hyperion_sim::id::Layer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

/// The seed of the galaxy these queries run in.
const SEED: u64 = 0x0d13_0400_0000_0000;

fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// The fixture with its velocities, for queries away from the epoch.
fn moving_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| galaxy().clone().with_full_potential())
}

/// The fixture at 60 rogue planets per star, whose centre saturates (ruling 125).
fn saturated_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        galaxy().clone().with_substellar_params(
            SubstellarParams::generator_default().with_rogue_planets_per_star(60.0),
        )
    })
}

fn query(
    centre: GalacticPosition,
    radius: f64,
    request: SubstellarRequest,
    limit: u32,
) -> RangeQuery {
    RangeQuery::builder(centre, LightYears::new(radius))
        .substellar(request)
        .limit(NonZeroU32::new(limit).expect("a positive limit"))
        .build()
        .expect("a query in the disc")
}

fn run(galaxy: &Galaxy, query: &RangeQuery) -> RangeResult {
    range_query(galaxy, &mut NoCache::new(), &[], query)
}

/// How many hits of each kind a result holds.
fn kinds(result: &RangeResult) -> BTreeMap<&'static str, u64> {
    let mut counts = BTreeMap::new();
    for hit in result.systems() {
        let kind = match hit.record().kind() {
            SystemKind::Stellar => "stellar",
            SystemKind::BrownDwarf => "brown dwarf",
            SystemKind::RoguePlanet => "rogue planet",
        };
        *counts.entry(kind).or_insert(0) += 1;
    }
    counts
}

/// The stellar hits' IDs, in the result's order.
fn stellar_ids(result: &RangeResult) -> Vec<u64> {
    result
        .systems()
        .iter()
        .filter(|hit| hit.record().kind() == SystemKind::Stellar)
        .map(|hit| hit.id().raw())
        .collect()
}

/// The cells the census's layers take in the query's sphere at the epoch.
fn cells_of(result: &RangeResult, centre: GalacticPosition, radius: f64) -> u64 {
    let sphere = QuerySphere::new(
        centre,
        LightYears::new(radius),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .expect("a sphere");
    result
        .census()
        .layers()
        .iter()
        .map(|layer| count_cells_in_sphere(layer, &sphere))
        .sum()
}

#[test]
fn the_default_request_walks_no_substellar_cell() {
    let galaxy = galaxy();
    let centre = sunlike_point(galaxy);
    let result = run(galaxy, &query(centre, 20.0, SubstellarRequest::None, 4_096));
    assert_eq!(result.census().complete_down_to(), Some(Layer::A));
    assert!(!result.census().layers().contains(Layer::BrownDwarf));
    assert!(!result.census().layers().contains(Layer::RoguePlanet));
    assert_same_bits(result.census().expected().get(Layer::BrownDwarf), 0.0);
    assert_same_bits(result.census().expected().get(Layer::RoguePlanet), 0.0);
    assert!(
        result
            .systems()
            .iter()
            .all(|hit| hit.record().kind() == SystemKind::Stellar)
    );
    // Every cell visited is a stellar layer's.
    assert_eq!(
        result.stats().cells_visited(),
        cells_of(&result, centre, 20.0)
    );
}

/// Plan 13, P13.T4: 50 ly where the density is 0.003 per ly³ returns about 390–450 brown dwarfs, in
/// the Poisson interval of the computed expectation, and the same stars as the default request.
#[test]
fn a_50_ly_query_with_brown_dwarfs_returns_them_and_the_same_stars() {
    let galaxy = galaxy();
    let centre = reference_density_point(galaxy);
    let stars = run(galaxy, &query(centre, 50.0, SubstellarRequest::None, 4_096));
    let with = run(
        galaxy,
        &query(centre, 50.0, SubstellarRequest::BrownDwarfs, 4_096),
    );
    assert_eq!(with.census().complete_down_to(), Some(Layer::BrownDwarf));
    assert_eq!(with.census().stopped_by(), CensusStop::MassFloor);
    let expected = with.census().expected().get(Layer::BrownDwarf);
    assert!(
        (390.0..=450.0).contains(&expected),
        "{expected} brown dwarfs expected in 50 ly"
    );
    let found = kinds(&with).get("brown dwarf").copied().unwrap_or(0);
    assert_poisson_count("brown dwarfs within 50 ly", found, expected, ALPHA);
    assert_eq!(kinds(&with).get("rogue planet"), None);
    assert_eq!(stellar_ids(&with), stellar_ids(&stars));
    // The stellar layers' expectations do not move by a bit.
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        assert_same_bits(
            with.census().expected().get(layer),
            stars.census().expected().get(layer),
        );
    }
    assert_eq!(with.stats().cells_visited(), cells_of(&with, centre, 50.0));
}

/// With the rogue planets asked for and a limit of 5,000 at 50 ly, their some 46,000 expected are
/// dropped whole, and the census names the brown dwarfs' step, before any of their cells is
/// generated.
#[test]
fn a_50_ly_query_drops_the_rogue_planets_whole() {
    let galaxy = galaxy();
    let centre = reference_density_point(galaxy);
    let result = run(
        galaxy,
        &query(
            centre,
            50.0,
            SubstellarRequest::BrownDwarfsAndRoguePlanets,
            5_000,
        ),
    );
    assert_eq!(result.census().complete_down_to(), Some(Layer::BrownDwarf));
    assert_eq!(result.census().stopped_by(), CensusStop::Limit);
    let complete_above = result.census().complete_above().expect("complete").value();
    assert!((0.012_40..0.012_42).contains(&complete_above));
    assert!(result.census().expected().get(Layer::RoguePlanet) > 5_000.0);
    assert_eq!(kinds(&result).get("rogue planet"), None);
    assert!(kinds(&result).get("brown dwarf").is_some_and(|&n| n > 300));
    assert_eq!(
        result.stats().cells_visited(),
        cells_of(&result, centre, 50.0)
    );
}

/// At 10 ly the rogue planets fit and come to about 350–400, in the Poisson interval of the
/// expectation; they are walked after the brown dwarfs and complete the census down to a third of
/// an Earth mass.
#[test]
fn a_10_ly_query_keeps_the_rogue_planets() {
    let galaxy = galaxy();
    let centre = reference_density_point(galaxy);
    let result = run(
        galaxy,
        &query(
            centre,
            10.0,
            SubstellarRequest::BrownDwarfsAndRoguePlanets,
            5_000,
        ),
    );
    assert_eq!(result.census().complete_down_to(), Some(Layer::RoguePlanet));
    assert_eq!(result.census().stopped_by(), CensusStop::MassFloor);
    let complete_above = result.census().complete_above().expect("complete").value();
    assert!((1.000e-6..1.002e-6).contains(&complete_above));
    let expected = result.census().expected().get(Layer::RoguePlanet);
    assert!(
        (350.0..=400.0).contains(&expected),
        "{expected} rogue planets expected in 10 ly"
    );
    let found = kinds(&result).get("rogue planet").copied().unwrap_or(0);
    assert_poisson_count("rogue planets within 10 ly", found, expected, ALPHA);
    for hit in result.systems() {
        assert!(hit.distance().value() <= 10.0);
    }
}

/// Near the galactic centre, 0.5 ly: the expected count drives the census, which counts the
/// saturated density where the rogue planets saturate (ruling 125), and the answer is the same
/// across runs and cache states.
///
/// At (2, 2, 2) ly the sphere lies inside one cell of every layer, some 63,500 rogue-planet
/// candidates; [`a_query_at_the_very_centre_counts_the_saturated_density`] takes the origin itself,
/// a corner of eight cells of every layer.
#[test]
fn a_query_near_the_centre_counts_the_saturated_density() {
    let centre = GalacticPosition::from_light_years([2.0, 2.0, 2.0]).expect("in the root cube");
    assert_centre_census(centre);
}

#[test]
#[ignore = "slow: eight saturated rogue-planet cells, half a million candidates, twice in debug"]
fn a_query_at_the_very_centre_counts_the_saturated_density() {
    assert_centre_census(GalacticPosition::ORIGIN);
}

fn assert_centre_census(centre: GalacticPosition) {
    let galaxy = saturated_galaxy();
    let every = SubstellarRequest::BrownDwarfsAndRoguePlanets;
    let volume = 4.0 / 3.0 * core::f64::consts::PI * 0.125;
    let saturation = rogue_planet_saturation_density();

    // Under a limit of 200 the rogue planets, some 520 expected, are dropped whole, and none of
    // their cells is generated.
    let small = run(galaxy, &query(centre, 0.5, every, 200));
    assert_eq!(small.census().complete_down_to(), Some(Layer::BrownDwarf));
    assert_eq!(small.census().stopped_by(), CensusStop::Limit);
    assert_eq!(small.stats().cells_visited(), cells_of(&small, centre, 0.5));

    let full = query(centre, 0.5, every, 4_096);
    let cold = run(galaxy, &full);
    assert_eq!(cold.census().complete_down_to(), Some(Layer::RoguePlanet));
    let expected = cold.census().expected().get(Layer::RoguePlanet);
    // Every node saturates, so the count is C times the volume; unsaturated, 60 per star would
    // give 1.66 times that.
    assert!(
        (expected / (saturation * volume) - 1.0).abs() < 1e-9,
        "{expected} against C × V = {}",
        saturation * volume
    );
    let found = kinds(&cold).get("rogue planet").copied().unwrap_or(0);
    assert_poisson_count(
        "rogue planets within 0.5 ly of the centre",
        found,
        expected,
        ALPHA,
    );

    // A cache warmed by the same query, and then all hits, gives the same answer.
    let mut cache = WarmCellCache::default();
    let warming = range_query(galaxy, &mut cache, &[], &full);
    let warm = range_query(galaxy, &mut cache, &[], &full);
    for result in [&warming, &warm] {
        assert_eq!(result.census(), cold.census());
        assert_eq!(result.systems(), cold.systems());
    }
}

/// The Milky Way fixture's own centre does not saturate (ruling 125): its count there is the
/// linear one, below C times the volume.
#[test]
fn the_fixture_s_centre_does_not_saturate() {
    let result = run(
        galaxy(),
        &query(
            GalacticPosition::ORIGIN,
            0.5,
            SubstellarRequest::BrownDwarfsAndRoguePlanets,
            4_096,
        ),
    );
    let volume = 4.0 / 3.0 * core::f64::consts::PI * 0.125;
    let expected = result.census().expected().get(Layer::RoguePlanet);
    assert!(
        expected < 0.8 * rogue_planet_saturation_density() * volume,
        "{expected}"
    );
    assert!(
        expected > 0.4 * rogue_planet_saturation_density() * volume,
        "{expected}"
    );
}

/// At t = ±1,000 yr a query returns the same objects as at the epoch apart from those crossing the
/// sphere: every ID in one answer and not the other was, at the epoch, within the pad of the radius.
#[test]
fn a_moving_query_changes_only_the_objects_crossing_the_sphere() {
    let galaxy = moving_galaxy();
    let centre = sunlike_point(galaxy);
    let radius = 10.0;
    let every = SubstellarRequest::BrownDwarfsAndRoguePlanets;
    let at = |years: i64| {
        let t = UniverseTime::from_julian_years(years).expect("in the window");
        let query = RangeQuery::builder(centre, LightYears::new(radius))
            .time(t)
            .substellar(every)
            .build()
            .expect("a query in the window");
        run(galaxy, &query)
    };
    let epoch = at(0);
    let epoch_distance: BTreeMap<u64, f64> = epoch
        .systems()
        .iter()
        .map(|hit| (hit.id().raw(), hit.distance().value()))
        .collect();
    let epoch_ids: BTreeSet<u64> = epoch_distance.keys().copied().collect();
    let pad = pad_for(
        UniverseTime::from_julian_years(1_000).expect("in the window"),
        PAD_SPEED,
    )
    .value();
    for years in [-1_000, 1_000] {
        let moved = at(years);
        assert_eq!(moved.census().complete_down_to(), Some(Layer::RoguePlanet));
        let ids: BTreeSet<u64> = moved.systems().iter().map(|hit| hit.id().raw()).collect();
        let kept = ids.intersection(&epoch_ids).count();
        assert!(
            kept * 10 > epoch_ids.len() * 9,
            "{kept} of {} kept",
            epoch_ids.len()
        );
        // Left the sphere: at the epoch it was within the pad of the edge.
        for id in epoch_ids.difference(&ids) {
            assert!(
                epoch_distance[id] > radius - pad,
                "{id:#x} left from {}",
                epoch_distance[id]
            );
        }
        // Entered it: now within the pad of the edge.
        for hit in moved
            .systems()
            .iter()
            .filter(|hit| !epoch_ids.contains(&hit.id().raw()))
        {
            assert!(
                hit.distance().value() > radius - pad,
                "{:#x}",
                hit.id().raw()
            );
        }
    }
}

#[test]
fn a_lowered_floor_needs_its_request() {
    use hyperion_sim::galaxy::query::BuildRangeQueryError::{
        SubstellarBelowFloor, SubstellarNotRequested,
    };
    let centre = sunlike_point(galaxy());
    let build = |floor, request| {
        RangeQuery::builder(centre, LightYears::new(10.0))
            .mass_floor(floor)
            .substellar(request)
            .build()
            .map(|query| query.mass_floor())
    };
    assert_eq!(
        build(MassFloor::BrownDwarfs, SubstellarRequest::None),
        Err(SubstellarNotRequested)
    );
    assert_eq!(
        build(MassFloor::LayerC, SubstellarRequest::BrownDwarfs),
        Err(SubstellarBelowFloor)
    );
    assert_eq!(
        build(MassFloor::LayerA, SubstellarRequest::BrownDwarfs),
        Ok(MassFloor::BrownDwarfs)
    );
}

/// Substellar range queries through one warm cache give each query the same answer whatever was
/// asked before it, and the same as a cold one.
#[test]
fn substellar_queries_are_the_same_whatever_the_order() {
    let galaxy = galaxy();
    let centre = sunlike_point(galaxy);
    let every = SubstellarRequest::BrownDwarfsAndRoguePlanets;
    let offsets = [
        [0.0, 0.0, 0.0],
        [3.0, 0.0, 0.0],
        [0.0, -4.0, 1.0],
        [6.0, 5.0, -2.0],
    ];
    let queries: Vec<RangeQuery> = offsets
        .iter()
        .map(|[x, y, z]| {
            let at = centre.to_light_years_f64();
            let moved = GalacticPosition::from_light_years([at[0] + x, at[1] + y, at[2] + z])
                .expect("in the root cube");
            query(moved, 4.0, every, 4_096)
        })
        .collect();
    let cache = std::cell::RefCell::new(WarmCellCache::default());
    hyperion_testkit::order::assert_order_independent(&queries, |query| {
        let result = range_query(galaxy, &mut *cache.borrow_mut(), &[], query);
        (*result.census(), result.into_systems())
    });
    for query in &queries {
        let warm = range_query(galaxy, &mut *cache.borrow_mut(), &[], query);
        assert_eq!(warm.systems(), run(galaxy, query).systems());
    }
}
