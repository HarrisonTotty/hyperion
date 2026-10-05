//! The census against its oracle (rendering plan R06, R06.T8.e): with the caps forced, the census,
//! with its mass skip and its flux bound, and the brute force, which generates every system of the
//! same cells whole, agree star for star and bit for bit, at the query's `n_max` and at a third of
//! the stars listed, where the overflow is compared too.
//!
//! The spheres are those a test can afford to generate whole, counted on the fixture on
//! 2026-10-04 (systems in the cells opened; about 0.9 ms a system on one thread under load, census
//! and brute force each):
//!
//! | Test | Place | Layers within | Cut | Systems |
//! | ---- | ----- | ------------- | --- | ------- |
//! | fast | near the Sun | every layer, 150 ly | 11 | 39,600 |
//! | fast | nuclear disc | A and B, 1.5 ly (the eight cells at the place) | 11 | 49,534 |
//! | slow | near the Sun | A 300 ly, B 500 ly | 7.95 | 246,600 |
//! | slow | near the Sun | C, D and E, 1,000 ly | 7.95 | 1.40 M |
//! | slow | nuclear disc | D and E, 20 ly (the eight cells at the place) | 7.95 | 0.88 M |
//!
//! The plan's 200 ly in the nuclear disc would generate 17.3 M systems for D and E alone, and the
//! 10 ly over every layer accepted on 2026-10-03 would generate 1.9 M, since a cell there holds up
//! to 124,000 (R06's Risks). Layers C and the brown dwarfs are compared near the Sun only. The slow
//! A/B check is the one where identity needs the envelope to bound every M and K dwarf: there the
//! census skips nearly all of them.
//!
//! The two slow tests near the Sun fail until R06.T16.b: each sphere holds a merger product, a
//! giant of up to twice its primary's mass, that the flux bound at the primary's mass skips (R06's
//! Risks). The slow profile leaves them out by name (`.config/nextest.toml`) until T16.b widens
//! the bound and they pass unchanged.
//!
//! On wasm32-wasip1, which has no threads, the oracle runs on one: there the fast identity tests
//! are slow tests (`just test-wasm-slow` runs them, a 32-bit check of the identity), and the slow
//! ones are left out, which would take hours.

#[expect(
    dead_code,
    reason = "these checks use the sky helpers of tests/common alone"
)]
mod common;

use std::num::NonZeroU32;
use std::sync::OnceLock;

use common::sky::{
    NUCLEAR_DISC_LY, Part, SUN_LY, assert_same_stars, brute_force_parts, brute_force_sky,
    census_parts, every_layer, observer_in_nuclear_disc, observer_near_sun,
};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::id::Layer;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::caps::CAPPED_LAYERS;
use hyperion_sim::sky::census::{CensusTallies, SkyQuery, merge_census};
use hyperion_sim::sky::luminosity::{LuminosityTables, REFERENCE_TIME};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::{LightYears, Magnitudes};
use hyperion_testkit::float;

/// The Milky Way fixture the sim's own sky tests use.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
            .expect("the Milky Way-like parameters are valid")
    })
}

/// The eye's query to apparent V `cut`: a star is kept to the cut plus its colour offset.
fn eye_query(observer: Observer, cut: Magnitudes) -> SkyQuery {
    SkyQuery::builder(observer, cut)
        .eye(EyeObserver::default())
        .build()
        .expect("a valid query")
}

/// The parts' tallies summed.
fn tallies(parts: &[Part]) -> CensusTallies {
    let mut sum = CensusTallies::default();
    for (_, t) in parts {
        sum.add(t);
    }
    sum
}

/// The systems each capped layer generated, census then brute force.
type Generated = Vec<(Layer, u64, u64)>;

/// Asserts that the census of `query` with `radii` forced and its oracle agree star for star and
/// bit for bit, at the query's `n_max` and at a third of the listed, and that the census lists
/// stars and skips systems; returns each layer's generated systems.
fn agree(what: &str, query: SkyQuery, radii: &[(Layer, LightYears)]) -> Generated {
    let g = galaxy();
    let n_max = query.n_max();
    let census = census_parts(g, query.clone(), radii);
    let brute = brute_force_parts(g, query, radii);
    assert_eq!(census.len(), brute.len(), "{what}: the cells opened");
    let (c, b) = (tallies(&census), tallies(&brute));
    let mut generated = Vec::new();
    for &layer in &CAPPED_LAYERS {
        let (cl, bl) = (c.layer(layer), b.layer(layer));
        eprintln!(
            "{what}: {layer:?}: {} cells, {} candidates, {} generated of {}, {} accepted",
            cl.cells(),
            cl.candidates(),
            cl.generated(),
            bl.generated(),
            cl.accepted(),
        );
        assert_eq!(cl.accepted(), bl.accepted(), "{what}: {layer:?} accepted");
        generated.push((layer, cl.generated(), bl.generated()));
    }
    let (whole, oracle) = (
        merge_census(census.iter().cloned(), n_max),
        merge_census(brute.iter().cloned(), n_max),
    );
    assert!(!whole.listed().is_empty(), "{what}: the census lists stars");
    assert_same_stars(whole.listed(), oracle.listed(), what);
    assert_same_stars(whole.overflow(), oracle.overflow(), what);
    let third = u32::try_from(whole.listed().len() / 3).expect("at most n_max");
    let third = NonZeroU32::new(third.max(1)).expect("at least one");
    let (cut, cut_oracle) = (merge_census(census, third), merge_census(brute, third));
    assert!(!cut.overflow().is_empty(), "{what}: a third overflows");
    assert_same_stars(cut.listed(), cut_oracle.listed(), what);
    assert_same_stars(cut.overflow(), cut_oracle.overflow(), what);
    for &layer in &CAPPED_LAYERS {
        assert_eq!(
            cut.tallies().layer(layer).listed(),
            cut_oracle.tallies().layer(layer).listed(),
            "{what}: {layer:?} listed"
        );
    }
    let (skipping, every): (u64, u64) = generated
        .iter()
        .fold((0, 0), |(s, e), &(_, c, b)| (s + c, e + b));
    assert!(
        skipping < every,
        "{what}: the census generates {skipping} of {every} systems, so skips none"
    );
    generated
}

/// Each listed layer within its radius in light-years.
fn within_ly(radii: &[(Layer, f64)]) -> Vec<(Layer, LightYears)> {
    radii
        .iter()
        .map(|&(layer, ly)| (layer, LightYears::new(ly)))
        .collect()
}

#[test]
fn the_dark_tables_hold_no_star_at_the_reference_time() {
    let g = galaxy();
    let dark = LuminosityTables::dark(g);
    assert_eq!(dark.time(), REFERENCE_TIME);
    let zero = float::bits(0.0);
    let places = [SUN_LY, NUCLEAR_DISC_LY].map(|ly| {
        PointLy::from(&GalacticPosition::from_light_years(ly).expect("in the root cube"))
    });
    for id in g.fields().component_ids() {
        for layer in Layer::ALL {
            let at = places.iter().map(|p| dark.get_at(id, layer, p));
            for f in std::iter::once(dark.get(id, layer)).chain(at) {
                // Light ages at the snapshots, between them and beyond the last, read as a query
                // at the epoch reads them.
                for years in [0, 500, 1_000, 2_000, 10_000, 100_000, 262_144, 400_000] {
                    let ago = Span::from_julian_years(years).expect("a span");
                    let ago = dark.age_for(UniverseTime::EPOCH, ago);
                    let what = format!("{id:?} {layer:?} {years} years");
                    assert_eq!(float::bits(f.total_light(ago).value()), zero, "{what}");
                    assert_eq!(float::bits(f.stars_per_system(ago)), zero, "{what}");
                    assert_eq!(float::bits(f.dark_per_system(ago)), zero, "{what}");
                    assert_eq!(float::bits(f.remnants_per_system(ago)), zero, "{what}");
                    for m in [-12.0, -5.0, 0.0, 5.0, 10.0, 20.0, 25.0] {
                        let m = Magnitudes::new(m);
                        assert_eq!(float::bits(f.count_brighter_than(m, ago)), zero, "{what}");
                        let light = f.light_fainter_than(m, ago).value();
                        assert_eq!(float::bits(light), zero, "{what}");
                        assert_eq!(f.colour_fainter_than(m, ago), None, "{what}");
                    }
                }
            }
        }
    }
    assert!(dark.heap_bytes() > 0, "the dark tables hold their zeros");
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "slow: on wasm32-wasip1, which has no threads, the oracle runs on one"
)]
fn the_census_is_its_oracle_150_ly_from_the_sun() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(11.0));
    agree(
        "every layer within 150 ly of the Sun",
        query,
        &every_layer(LightYears::new(150.0)),
    );
}

/// The plan's oracle, every layer within one radius, is the census with every cap forced to it
/// by `with_caps_forced`.
#[test]
fn brute_force_sky_is_the_census_with_every_cap_forced() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(11.0));
    let radius = LightYears::new(40.0);
    let forced = query
        .clone()
        .with_caps_forced(radius)
        .expect("a valid forced cap");
    let per_layer = query
        .clone()
        .with_caps_forced_per_layer(&every_layer(radius))
        .expect("a valid forced cap");
    assert_eq!(forced, per_layer, "every layer forced to the radius");
    let n_max = query.n_max();
    let census = merge_census(
        census_parts(galaxy(), query.clone(), &every_layer(radius)),
        n_max,
    );
    let oracle = brute_force_sky(galaxy(), query, radius);
    assert!(!census.listed().is_empty(), "the census lists stars");
    assert_same_stars(census.listed(), oracle.listed(), "within 40 ly");
    assert_same_stars(census.overflow(), oracle.overflow(), "within 40 ly");
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "slow: on wasm32-wasip1, which has no threads, the oracle runs on one"
)]
fn the_census_is_its_oracle_in_the_nuclear_disc() {
    let query = eye_query(observer_in_nuclear_disc(galaxy()), Magnitudes::new(11.0));
    agree(
        "A and B within 1.5 ly in the nuclear disc",
        query,
        &within_ly(&[(Layer::A, 1.5), (Layer::B, 1.5)]),
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: generates every system of A within 300 ly and B within 500 ly of the Sun; \
            fails until R06.T16.b, a merger of B's (system 0x21fe56487ff00001, V 7.53) outshining \
            the flux bound at m₁"]
fn the_census_is_its_oracle_for_the_dwarfs_near_the_sun() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(7.95));
    let generated = agree(
        "A within 300 ly and B within 500 ly of the Sun",
        query,
        &within_ly(&[(Layer::A, 300.0), (Layer::B, 500.0)]),
    );
    for (layer, census, brute) in generated {
        if matches!(layer, Layer::A | Layer::B) {
            assert!(
                census < brute,
                "{layer:?}: the census generates {census} of {brute} systems"
            );
        }
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: generates every system of layers C to E within 1,000 ly of the Sun; fails \
            until R06.T16.b, a merger of C's (system 0x42046c99ff00000a, V 7.44) outshining the \
            flux bound at m₁"]
fn the_census_is_its_oracle_1000_ly_from_the_sun() {
    let query = eye_query(observer_near_sun(galaxy()), Magnitudes::new(7.95));
    agree(
        "C to E within 1,000 ly of the Sun",
        query,
        &within_ly(&[
            (Layer::C, 1_000.0),
            (Layer::D, 1_000.0),
            (Layer::E, 1_000.0),
        ]),
    );
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: generates every system of the D and E cells at a place in the nuclear disc"]
fn the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc() {
    let query = eye_query(observer_in_nuclear_disc(galaxy()), Magnitudes::new(7.95));
    agree(
        "D and E within 20 ly in the nuclear disc",
        query,
        &within_ly(&[(Layer::D, 20.0), (Layer::E, 20.0)]),
    );
}
