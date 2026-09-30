//! Benchmarks of the substellar layers (plan 13, P13.T9): a rogue-planet cell at the galactic
//! centre, and range queries that ask for the substellar layers, each beside the stellar query it
//! extends.
//!
//! All of them run on `GalaxyParams::milky_way_like()` with a fixed seed, cold, with no cache. A
//! miss is a finding to raise, not a CI failure: CI compiles these and never runs them.
//!
//! Measured on 2026-09-28 by `cargo bench -p hyperion-sim --bench substellar` under the heavy-test
//! lock, in the `sub13b` lane, at a load average of 13–22 on the development machine (other lanes'
//! builds running), so each figure is an upper bound; a miss is a finding.
//!
//! | Bench | What it does | Measured | Plan 13's target |
//! | ----- | ------------ | -------- | ---------------- |
//! | `rogue_cell_at_origin` | generates the rogue-planet cell whose corner is the origin, tens of thousands of candidates | 48.9 ms (load 20) | 20 ms: missed |
//! | `rogue_0.5ly_at_origin` | a 0.5 ly query at the origin with the rogue planets asked for | 22.6 s (load 20) | 150 ms: missed |
//! | `stellar_0.5ly_at_origin` | the same query of the stellar layers alone | 9.03 s (load 13–16) | — |
//! | `stellar_10ly_reference` | a 10 ly query where the system density is 0.003 per ly³ | 3.56 ms | — |
//! | `rogue_10ly_reference` | the same with the rogue planets asked for | 5.60 ms, +2.0 ms | 5 ms over the stellar: met |
//! | `stellar_50ly_reference` | a 50 ly query there | 35.5 ms | — |
//! | `brown_50ly_reference` | the same with the brown dwarfs asked for | 55.4 ms, +19.9 ms | 1 ms over the stellar: missed |
//!
//! The central query's cost is mostly the stellar layers': eight cells of every layer meet a
//! sphere at the origin, and the coarse layers' central cells hold some 10⁵ systems each, which
//! every query of the centre generates whole, substellar layers asked for or not. The plan's
//! remedy, testing a candidate's distance from the padded sphere before its density, would help
//! only a caller that does not cache whole cells, which the server does; it is not applied, and the
//! misses are the lane's findings.
//!
//! # Re-measured by perf08
//!
//! On 2026-09-29, at generator version 15, the builds before and after `perf08`'s exact speed-up of
//! the log-normal mass draw run alternately, three times, at a load average of 6 (other lanes'
//! tests running); the central queries once each. The two benches of cells measure what a request
//! adds alone, rather than as the difference of two queries that move by a quarter between runs.
//!
//! | Bench | What it does | Before | After | Plan 13's target |
//! | ----- | ------------ | ------ | ----- | ---------------- |
//! | `rogue_cell_at_origin` | as above; a path `perf08` did not touch | 32–39 ms | 38–48 ms | 20 ms: missed |
//! | `rogue_0.5ly_at_origin` | as above | 8.25 s | 7.51 s | 150 ms: missed |
//! | `stellar_0.5ly_at_origin` | as above | 7.01 s | 7.13 s | — |
//! | `substellar_cells_at_origin` | the eight central cells of layers F and G, what the request adds on a caching caller | 811–843 ms | 678–828 ms | 150 ms (its share): missed |
//! | `stellar_50ly_reference` | as above | 12.3–15.2 ms | 10.7–12.9 ms | — |
//! | `brown_cells_50ly_reference` | the 248 layer-F cells of that query, what the brown dwarfs add | 3.2–4.6 ms | 2.4–2.9 ms | 1 ms over the stellar: missed |
//! | `rogue_10ly_reference` | as above | +1.4–2.5 ms | +1.4–2.2 ms | 5 ms over the stellar: met |
//!
//! Most of `sub13b`'s misses were the load: at 6 rather than 20 the brown dwarfs add 3–5 ms, not
//! 19.9. What is left is the cost of a candidate, whole cells being what a caching caller
//! generates: 16 components' densities (1.1–1.5 µs), four streams and the age, and for a brown dwarf
//! the log-normal's inversion, which was 65 error functions (3.1 µs) and is now about 20 (1.8 µs),
//! bit for bit. A central rogue-planet cell is 35,312 candidates, so its densities alone take the 20
//! ms target; the 248 brown-dwarf cells' bounds alone are half the 1 ms. Plan 13's as-built note
//! (`perf08`) gives the choice of remedy.

use std::hint::black_box;
use std::num::NonZeroU32;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::fields::MAX_COMPONENTS;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, NoCache, generate_cell};
use hyperion_sim::galaxy::query::{
    QuerySphere, RangeQuery, SubstellarRequest, cells_in_sphere, range_query,
};
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::id::Layer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;

/// The seed every bench here runs in.
const SEED: u64 = 0x0d13_0900_0000_0000;

/// A limit that holds every layer these queries ask for, so that the census walks them all.
const LIMIT: NonZeroU32 = NonZeroU32::new(20_000).expect("20,000 is not zero");

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The point in the plane on the +y axis where the system density is the brainstorm's reference
/// 0.003 per ly³, by bisection between 16,000 and 26,000 ly: a copy of `tests/common`'s
/// `reference_density_point`, which a bench cannot reach.
fn reference_point(galaxy: &Galaxy) -> GalacticPosition {
    let density = |y: f64| {
        let mut per_component = [0.0; MAX_COMPONENTS];
        galaxy
            .fields()
            .densities(&PointLy::new(0.0, y, 0.0), &mut per_component)
    };
    let (mut inner, mut outer) = (16_000.0, 26_000.0);
    for _ in 0..40 {
        let mid = f64::midpoint(inner, outer);
        if density(mid) > 0.003 {
            inner = mid;
        } else {
            outer = mid;
        }
    }
    GalacticPosition::from_light_years([0.0, inner.round(), 0.0]).expect("in the root cube")
}

fn query(centre: GalacticPosition, radius: f64, request: SubstellarRequest) -> RangeQuery {
    RangeQuery::builder(centre, LightYears::new(radius))
        .limit(LIMIT)
        .substellar(request)
        .build()
        .expect("a query in the disc")
}

/// The central rogue-planet cell, generated whole: the most candidates any cell of the layer
/// draws in this galaxy.
fn central_cell(c: &mut Criterion) {
    let galaxy = galaxy();
    let key = CellKey::new(Layer::RoguePlanet, [0, 0, 0]).expect("the centre's cell");
    let mut records = Vec::new();
    generate_cell(&galaxy, key, &mut records);
    assert!(records.len() > 10_000, "{} rogue planets", records.len());
    let mut group = c.benchmark_group("substellar");
    group.sample_size(20);
    group.bench_function("rogue_cell_at_origin", |b| {
        b.iter(|| generate_cell(black_box(&galaxy), black_box(key), &mut records));
    });
    group.finish();
}

/// A 0.5 ly query at the origin with the rogue planets asked for, eight central cells of every
/// layer, and the same query of the stellar layers alone, which says how much of it is the stars'.
fn central_query(c: &mut Criterion) {
    let galaxy = galaxy();
    let mut group = c.benchmark_group("substellar");
    group.sample_size(10);
    for (name, request, finest) in [
        (
            "rogue_0.5ly_at_origin",
            SubstellarRequest::BrownDwarfsAndRoguePlanets,
            Layer::RoguePlanet,
        ),
        ("stellar_0.5ly_at_origin", SubstellarRequest::None, Layer::A),
    ] {
        let query = query(GalacticPosition::ORIGIN, 0.5, request);
        let walked = range_query(&galaxy, &mut NoCache::new(), &[], &query);
        assert_eq!(walked.census().complete_down_to(), Some(finest));
        group.bench_function(name, |b| {
            b.iter(|| {
                range_query(
                    black_box(&galaxy),
                    &mut NoCache::new(),
                    &[],
                    black_box(&query),
                )
            });
        });
    }
    group.finish();
}

/// The cells of `layers` that a query of `radius` light-years about `centre` walks, at the epoch.
fn walked_cells(centre: GalacticPosition, radius: f64, layers: &[Layer]) -> Vec<CellKey> {
    let sphere = QuerySphere::new(
        centre,
        LightYears::new(radius),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .expect("a sphere at the epoch");
    layers
        .iter()
        .flat_map(|&layer| cells_in_sphere(layer, &sphere))
        .collect()
}

/// What asking for the substellar layers adds to a query, measured alone rather than as the
/// difference of two noisy queries: the substellar cells the query walks, generated whole and
/// cold. On a caller that caches whole cells, as the server does, that is where the request's cost
/// lies, the per-record distance test being a few nanoseconds (perf08).
///
/// - `substellar_cells_at_origin`: the eight central cells of layers F and G, what the brown dwarfs
///   and rogue planets add to `rogue_0.5ly_at_origin`.
/// - `brown_cells_50ly_reference`: the 248 layer-F cells of the 50 ly query at the reference
///   density, what the brown dwarfs add to `stellar_50ly_reference`.
fn substellar_cells(c: &mut Criterion) {
    let galaxy = galaxy();
    let reference = reference_point(&galaxy);
    let cases = [
        (
            "substellar_cells_at_origin",
            walked_cells(
                GalacticPosition::ORIGIN,
                0.5,
                &[Layer::BrownDwarf, Layer::RoguePlanet],
            ),
            16,
        ),
        (
            "brown_cells_50ly_reference",
            walked_cells(reference, 50.0, &[Layer::BrownDwarf]),
            248,
        ),
    ];
    let mut records = Vec::new();
    let mut group = c.benchmark_group("substellar");
    group.sample_size(10);
    for (name, keys, count) in cases {
        assert_eq!(keys.len(), count, "{name}");
        group.bench_function(name, |b| {
            b.iter(|| {
                for &key in &keys {
                    generate_cell(black_box(&galaxy), key, &mut records);
                }
            });
        });
    }
    group.finish();
}

/// The 10 ly and 50 ly queries at the reference density, each without and with its substellar
/// layers.
fn reference_queries(c: &mut Criterion) {
    let galaxy = galaxy();
    let centre = reference_point(&galaxy);
    let cases = [
        ("stellar_10ly_reference", 10.0, SubstellarRequest::None),
        (
            "rogue_10ly_reference",
            10.0,
            SubstellarRequest::BrownDwarfsAndRoguePlanets,
        ),
        ("stellar_50ly_reference", 50.0, SubstellarRequest::None),
        ("brown_50ly_reference", 50.0, SubstellarRequest::BrownDwarfs),
    ];
    let mut group = c.benchmark_group("substellar");
    for (name, radius, request) in cases {
        let query = query(centre, radius, request);
        let walked = range_query(&galaxy, &mut NoCache::new(), &[], &query);
        assert_eq!(
            walked.census().complete_down_to(),
            Some(query.mass_floor().layer())
        );
        group.bench_function(name, |b| {
            b.iter(|| {
                range_query(
                    black_box(&galaxy),
                    &mut NoCache::new(),
                    &[],
                    black_box(&query),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(
    substellar_benches,
    central_cell,
    central_query,
    substellar_cells,
    reference_queries
);
criterion_main!(substellar_benches);
