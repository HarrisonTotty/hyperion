//! Statistics of the substellar layers over large volumes (plan 13, P13.T9): the abundances per
//! star, the Jupiter-mass limit, the counts against the integral of the density, and determinism.
//!
//! All slow: a 384 ly cube in the plane holds some three million rogue planets. The fast checks of
//! the same quantities over single cells and small spheres are `tests/substellar_placement.rs` and
//! `tests/query_substellar.rs`.

#[expect(
    dead_code,
    reason = "the substellar statistics use the block of cells and the count conversion alone"
)]
mod common;

use std::num::NonZeroU32;

use common::{objects_in_block, usize_as_f64};

use hyperion_sim::coords::{GalacticPosition, LyCell};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::NoCache;
use hyperion_sim::galaxy::placement::SystemRecord;
use hyperion_sim::galaxy::query::{
    RangeQuery, RangeResult, SubstellarRequest, expected_counts, range_query,
};
use hyperion_sim::galaxy::substellar::SubstellarParams;
use hyperion_sim::id::Layer;
use hyperion_sim::stellar::multiplicity::{MultiplicityContext, RedrawAttempt, draw_hierarchy};
use hyperion_sim::units::LightYears;
use hyperion_sim::units::consts::{JUPITER_MASS_KG, SOLAR_MASS_KG};
use hyperion_sim::{Seed, math};
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

/// The seed of the Milky Way fixture these statistics are taken over.
const SEED: u64 = 0x0d13_0900_0000_0001;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The edge of the cube, light-years: 384, three layer-E cells, the nearest to the plan's 400 ly
/// that every layer's cells tile exactly, so that each layer is counted over the same volume.
const CUBE_LY: i32 = 384;

/// How many of `layer`'s cells span the cube's edge.
fn cells(layer: Layer) -> i32 {
    CUBE_LY / i32::try_from(layer.cell_size_ly()).expect("a small cell")
}

/// Plan 13, P13.T9: in a cube in the plane at 26,000 ly, brown dwarfs per star within 10% of
/// 1 ÷ 5.5 and rogue planets per star within 10% of 21, stars being systems times the galaxy's mean
/// stars per system; rogue planets above 0.3 Jupiter masses under one per four stars (Mróz et al.
/// 2017). It prints the free-floating brown dwarfs plus plan 11's brown-dwarf companions per star,
/// for review against the brainstorm's "one for every four or five stars, companions included",
/// which is not asserted.
#[test]
#[ignore = "slow: every object of every layer in a 384 ly cube, some three million rogue planets"]
fn the_abundances_per_star_are_the_parameters() {
    let galaxy = milky_way();
    // About the Sun-like point, on layer E's grid: 25,856 ly is 202 cells of 128 ly.
    let corner = [-128.0, 25_856.0, -128.0];
    let systems: Vec<SystemRecord> = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .flat_map(|layer| objects_in_block(&galaxy, layer, corner, cells(layer)))
        .collect();
    let brown = objects_in_block(&galaxy, Layer::BrownDwarf, corner, cells(Layer::BrownDwarf));
    let rogue = objects_in_block(
        &galaxy,
        Layer::RoguePlanet,
        corner,
        cells(Layer::RoguePlanet),
    );
    let stars = usize_as_f64(systems.len()) * galaxy.mean_stars_per_system();
    let params = SubstellarParams::generator_default();

    let brown_per_star = usize_as_f64(brown.len()) / stars;
    let rogue_per_star = usize_as_f64(rogue.len()) / stars;
    let giant = 0.3 * JUPITER_MASS_KG / SOLAR_MASS_KG;
    let giants_per_star = usize_as_f64(
        rogue
            .iter()
            .filter(|record| record.primary_initial_mass().value() > giant)
            .count(),
    ) / stars;
    eprintln!(
        "{} systems ({stars:.0} stars), {} brown dwarfs ({brown_per_star:.4} per star), {} rogue \
         planets ({rogue_per_star:.3} per star), {giants_per_star:.4} above 0.3 M_Jup per star",
        systems.len(),
        brown.len(),
        rogue.len()
    );
    assert!(
        (brown_per_star / params.brown_dwarfs_per_star() - 1.0).abs() < 0.1,
        "{brown_per_star} brown dwarfs per star against {}",
        params.brown_dwarfs_per_star()
    );
    assert!(
        (rogue_per_star / params.rogue_planets_per_star() - 1.0).abs() < 0.1,
        "{rogue_per_star} rogue planets per star against {}",
        params.rogue_planets_per_star()
    );
    assert!(giants_per_star < 0.25, "{giants_per_star} giants per star");

    // Plan 11's brown-dwarf companions: the stellar slots below 0.08 M☉ of the systems' drawn
    // hierarchies, over every system of the cube. Printed, not asserted.
    let companions = systems
        .iter()
        .map(|record| {
            draw_hierarchy(
                &galaxy,
                record,
                MultiplicityContext::Free,
                RedrawAttempt::FIRST,
            )
            .stars()
            .iter()
            .skip(1)
            .filter(|slot| slot.initial_mass().value() < 0.08)
            .count()
        })
        .sum::<usize>();
    eprintln!(
        "brown dwarfs per star, companions included: {:.4} free-floating + {:.4} companions = {:.4} \
         (the brainstorm's one for every four or five stars is 0.20-0.25)",
        brown_per_star,
        usize_as_f64(companions) / stars,
        (usize_as_f64(brown.len()) + usize_as_f64(companions)) / stars
    );
}

/// The query at `centre` of `radius` asking for `request`, with room for every layer.
fn wide_query(centre: GalacticPosition, radius: f64, request: SubstellarRequest) -> RangeQuery {
    RangeQuery::builder(centre, LightYears::new(radius))
        .substellar(request)
        .limit(NonZeroU32::new(u32::MAX).expect("not zero"))
        .cell_budget(NonZeroU32::new(1 << 24).expect("not zero"))
        .build()
        .expect("a query inside the disc")
}

/// How many of a result's hits are of `layer`.
fn hits_of(result: &RangeResult, layer: Layer) -> u64 {
    u64::try_from(
        result
            .systems()
            .iter()
            .filter(|hit| hit.record().layer() == layer)
            .count(),
    )
    .expect("a count fits in 64 bits")
}

/// Ten centres drawn from an LCG over the disc and its surroundings: 4,000–40,000 ly from the axis
/// at any angle, within 1,500 ly of the plane.
fn random_centres() -> Vec<GalacticPosition> {
    let mut lcg = Lcg::new(0x0d13_0901);
    (0..10)
        .map(|_| {
            let radius = 4_000.0 + 36_000.0 * lcg.next_f64();
            let angle = core::f64::consts::TAU * lcg.next_f64();
            let height = 3_000.0 * lcg.next_f64() - 1_500.0;
            let (sin, cos) = math::sin_cos(angle);
            GalacticPosition::from_light_years([radius * cos, radius * sin, height])
                .expect("in the root cube")
        })
        .collect()
}

/// Plan 13, P13.T9: over ten random volumes, the summed count of each layer lies in the Poisson
/// interval of the summed integral of its density, the census's own expected count (the
/// brainstorm's density-against-the-field test, for these layers): 150 ly spheres for the stars and
/// the brown dwarfs, 25 ly spheres for the rogue planets.
#[test]
#[ignore = "slow: ten 150 ly spheres of stars and brown dwarfs and ten 25 ly spheres of rogue planets"]
fn summed_counts_match_the_integral_of_the_density() {
    let galaxy = milky_way();
    let layers = [
        Layer::A,
        Layer::B,
        Layer::C,
        Layer::D,
        Layer::E,
        Layer::BrownDwarf,
    ];
    let mut expected = [0.0; 7];
    let mut found = [0_u64; 7];
    for centre in random_centres() {
        let brown = wide_query(centre, 150.0, SubstellarRequest::BrownDwarfs);
        let result = range_query(&galaxy, &mut NoCache::new(), &[], &brown);
        assert_eq!(result.census().complete_down_to(), Some(Layer::BrownDwarf));
        let counts = expected_counts(
            &galaxy,
            &centre,
            LightYears::new(150.0),
            SubstellarRequest::BrownDwarfs,
        );
        for layer in layers {
            let slot = usize::from(layer.value());
            expected[slot] += counts.get(layer);
            found[slot] += hits_of(&result, layer);
        }
        let rogue = wide_query(centre, 25.0, SubstellarRequest::BrownDwarfsAndRoguePlanets);
        let result = range_query(&galaxy, &mut NoCache::new(), &[], &rogue);
        assert_eq!(result.census().complete_down_to(), Some(Layer::RoguePlanet));
        let slot = usize::from(Layer::RoguePlanet.value());
        expected[slot] += result.census().expected().get(Layer::RoguePlanet);
        found[slot] += hits_of(&result, Layer::RoguePlanet);
    }
    for layer in layers.into_iter().chain([Layer::RoguePlanet]) {
        let slot = usize::from(layer.value());
        eprintln!(
            "layer {}: {} found against {:.1} expected",
            layer.letter(),
            found[slot],
            expected[slot]
        );
        assert_poisson_count(
            &format!("layer {} over ten volumes", layer.letter()),
            found[slot],
            expected[slot],
            ALPHA,
        );
    }
}

/// Plan 13, P13.T9: two runs of a query with both substellar layers, in two galaxies built apart,
/// agree bit for bit.
#[test]
#[ignore = "slow: two 20 ly queries with rogue planets near the bulge, twice"]
fn two_runs_agree() {
    let centre = GalacticPosition::new(LyCell::new([0, 9_000, 40]), [0.0; 3]).expect("in the cube");
    let query = wide_query(centre, 20.0, SubstellarRequest::BrownDwarfsAndRoguePlanets);
    let first = range_query(&milky_way(), &mut NoCache::new(), &[], &query);
    let second = range_query(&milky_way(), &mut NoCache::new(), &[], &query);
    assert!(hits_of(&first, Layer::RoguePlanet) > 1_000);
    assert_eq!(first.census(), second.census());
    assert_eq!(first.systems(), second.systems());
}
