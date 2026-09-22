//! The expected counts of a sphere against an independent reference (plan 03, P03.T9.c).
//!
//! `expected_counts` uses a fixed Gauss–Legendre rule of a few hundred to a few thousand nodes;
//! `common::reference_sphere_integral` is a midpoint sum in height, radius and azimuth with tens of
//! thousands, sharing no node with it. Agreement within a couple of per cent is what the census rule
//! needs: it decides whole layers from these numbers, and a layer is admitted or not by a factor of
//! the caller's limit, never by a per cent.

#[expect(
    dead_code,
    reason = "the expected-count tests use only the sphere helpers"
)]
mod common;

use common::{reference_sphere_integral, sunlike_point};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::query::expected_counts;
use hyperion_sim::units::LightYears;

/// The seed of the galaxy these counts are taken in.
const SEED: u64 = 0x0309_c007_0000_0001;

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
}

fn at(ly: [f64; 3]) -> GalacticPosition {
    GalacticPosition::from_light_years(ly).expect("inside the root cube")
}

/// Checks every layer's expected count against the reference over one sphere.
#[track_caller]
fn assert_layers_agree(
    galaxy: &Galaxy,
    what: &str,
    centre: &GalacticPosition,
    radius_ly: f64,
    steps: u32,
    tolerance: f64,
) {
    let radius = LightYears::new(radius_ly);
    let counts = expected_counts(galaxy, centre, radius);
    for band in MassBand::ALL {
        let reference = reference_sphere_integral(galaxy, band, centre, radius, steps);
        let taken = counts.get(band.layer());
        assert!(
            reference > 0.0,
            "{what}, R = {radius_ly} ly: the reference is {reference} in layer {}",
            band.layer().letter()
        );
        let error = (taken / reference - 1.0).abs();
        assert!(
            error < tolerance,
            "{what}, R = {radius_ly} ly, layer {}: {taken} against {reference}, off by {:.3}%",
            band.layer().letter(),
            100.0 * error
        );
    }
}

#[test]
fn expected_counts_agree_with_the_reference_at_the_sun_like_point() {
    let galaxy = galaxy();
    let sun = sunlike_point(&galaxy);
    for radius in [10.0, 50.0] {
        assert_layers_agree(&galaxy, "the Sun-like point", &sun, radius, 64, 0.02);
    }
}

#[test]
fn expected_counts_agree_with_the_reference_above_the_plane() {
    let galaxy = galaxy();
    let above = at([0.0, 26_000.0, 1_200.0]);
    assert_layers_agree(&galaxy, "above the plane", &above, 50.0, 64, 0.02);
}

#[test]
#[ignore = "slow: a 260,000-point reference sum per band, five bands a sphere, over twenty spheres"]
fn expected_counts_agree_with_the_reference_everywhere_over_every_radius() {
    let galaxy = galaxy();
    let sun = sunlike_point(&galaxy);
    // The places plan 03 names, with the tolerance it allows: 2% everywhere but the nuclear disc's
    // centre, where the sharpest vertical scale in the galaxy meets the coarsest panels.
    let places: [(&str, GalacticPosition, f64); 5] = [
        ("the Sun-like point", sun, 0.02),
        ("above the plane", at([0.0, 26_000.0, 1_200.0]), 0.02),
        // Straddling the plane, off centre, so the split at z = 0 makes two parts of unequal
        // width and therefore unequal panel counts.
        ("straddling the plane", at([0.0, 26_000.0, 20.0]), 0.02),
        ("the bulge", at([700.0, -800.0, 200.0]), 0.02),
        ("the nuclear disc's centre", at([0.0, 0.0, 0.0]), 0.05),
    ];
    for (what, centre, tolerance) in places {
        for radius in [10.0, 50.0, 500.0, 5_000.0] {
            assert_layers_agree(&galaxy, what, &centre, radius, 128, tolerance);
        }
    }
}

/// The counts are the same whichever sphere asked for them, and they grow with the volume where the
/// density is smooth.
#[test]
fn expected_counts_grow_with_the_volume_in_a_smooth_place() {
    let galaxy = galaxy();
    let sun = sunlike_point(&galaxy);
    let small = expected_counts(&galaxy, &sun, LightYears::new(10.0));
    let large = expected_counts(&galaxy, &sun, LightYears::new(20.0));
    for band in MassBand::ALL {
        let ratio = large.get(band.layer()) / small.get(band.layer());
        assert!(
            (ratio - 8.0).abs() < 0.1,
            "layer {}: {ratio} times as many in eight times the volume",
            band.layer().letter()
        );
    }
}
