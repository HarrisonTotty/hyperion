//! What the range query returns, layer by layer, against what the field says it should (plan 03,
//! P03.T10).
//!
//! `placement_stats.rs` counts systems in blocks of whole cells against a midpoint sum of the
//! density; this counts them as the query returns them, inside spheres, against
//! `expected_counts`'s Gauss–Legendre rule — the same number the census decides a layer from. So it
//! closes the loop the census rule depends on: if the expected counts were biased, a chart would say
//! "complete above 0.5 M☉" while holding the wrong number of stars, and no golden file would notice.
//!
//! The spheres are disjoint, so their counts are independent Poisson variables and their sums are
//! Poisson with the sum of the means.

#[expect(
    dead_code,
    reason = "the statistical query checks use the Sun-like point alone"
)]
mod common;

use common::sunlike_point;
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{NoCache, STELLAR_LAYERS};
use hyperion_sim::galaxy::query::{RangeQuery, range_query};
use hyperion_sim::id::Layer;
use hyperion_sim::units::LightYears;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

/// The seeds the queries run under: three independent placements of the same field, since the
/// fixture's parameters do not depend on the seed and only the thinning does. Three of them make the
/// sums tight enough that a per-cent bias in the quadrature would show.
const SEEDS: [u64; 3] = [
    0x0310_5747_0000_0000,
    0x0310_5747_0000_0001,
    0x0310_5747_0000_0002,
];

/// The radius of each sphere, light-years.
const RADIUS_LY: f64 = 20.0;

/// How far apart the spheres' centres sit on every axis, light-years: more than twice the radius, so
/// that no two spheres overlap and their counts are independent.
const SPACING_LY: f64 = 100.0;

/// The spheres: ten steps in x, ten in y and two in z about the Sun-like point.
const STEPS: [u32; 3] = [10, 10, 2];

/// The realised count of each layer over 200 disjoint spheres against the expected counts the census
/// is decided from (P03.T10).
#[test]
#[ignore = "slow: 200 range queries and their expected counts"]
fn realised_counts_per_layer_follow_the_expected_counts() {
    const { assert!(SPACING_LY > 2.0 * RADIUS_LY, "the spheres overlap") }
    let radius = LightYears::new(RADIUS_LY);
    let mut cache = NoCache::new();
    let mut found = [0_u64; 5];
    let mut expected = [0.0; 5];
    let mut spheres = 0_u32;
    for seed in SEEDS {
        let galaxy = Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral");
        let sun = sunlike_point(&galaxy).to_light_years_f64();
        let mut per_seed = [0_u64; 5];
        for i in 0..STEPS[0] {
            for j in 0..STEPS[1] {
                for k in 0..STEPS[2] {
                    let step = |n: u32, steps: u32| {
                        (f64::from(n) - f64::from(steps - 1) / 2.0) * SPACING_LY
                    };
                    let centre = GalacticPosition::from_light_years([
                        sun[0] + step(i, STEPS[0]),
                        sun[1] + step(j, STEPS[1]),
                        sun[2] + step(k, STEPS[2]),
                    ])
                    .expect("a point near the Sun-like point is inside the cube");
                    let query = RangeQuery::builder(centre, radius)
                        .build()
                        .expect("a 20 ly sphere near the Sun is a query");
                    let result = range_query(&galaxy, &mut cache, &[], &query);
                    // Only a census that admits every layer can be counted layer by layer.
                    assert_eq!(
                        result.census().complete_down_to(),
                        Some(Layer::A),
                        "the census of sphere ({i}, {j}, {k}) stopped short of layer A"
                    );
                    for hit in result.systems() {
                        per_seed[usize::from(hit.record().layer().value())] += 1;
                    }
                    // With no sources this is `expected_counts` over the unpadded sphere.
                    for spec in STELLAR_LAYERS {
                        let layer = spec.layer();
                        expected[usize::from(layer.value())] +=
                            result.census().expected().get(layer);
                    }
                    spheres += 1;
                }
            }
        }
        for (total, count) in found.iter_mut().zip(per_seed) {
            *total += count;
        }
        println!("seed {seed:#018x}: {per_seed:?} systems by layer, A to E");
    }
    assert_eq!(
        spheres,
        STEPS[0] * STEPS[1] * STEPS[2] * u32::try_from(SEEDS.len()).expect("three seeds")
    );

    let mut total_found = 0;
    let mut total_expected = 0.0;
    for spec in STELLAR_LAYERS {
        let layer = spec.layer();
        let index = usize::from(layer.value());
        println!(
            "layer {}: {} systems of {:.1} expected over {spheres} spheres of {RADIUS_LY} ly",
            layer.letter(),
            found[index],
            expected[index]
        );
        assert!(
            expected[index] > 20.0,
            "layer {} expects only {} systems over {spheres} spheres, which is no test",
            layer.letter(),
            expected[index]
        );
        assert_poisson_count(
            &format!("layer {} over {spheres} disjoint spheres", layer.letter()),
            found[index],
            expected[index],
            ALPHA,
        );
        total_found += found[index];
        total_expected += expected[index];
    }
    assert_poisson_count(
        "every layer over the disjoint spheres",
        total_found,
        total_expected,
        ALPHA,
    );
}
