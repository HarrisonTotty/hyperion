//! Tests of the crater shapes against the sources of `decision-r09-t2.md` item 3, their gravity
//! scaling, their continuity across the transition zone and their volume balance.

use super::*;
use crate::craters::density::GAUSS_LEGENDRE_8;
use crate::craters::{CraterParamsParts, Screening};
use hyperion_base::units::{
    KilogramsPerCubicMetre, KilogramsPerSquareMetre, MetresPerSecondSquared, PerSquareKilometre,
};
use hyperion_testkit::float::assert_same_bits;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// A contract at gravity `g` under `screening` on a target of factor `k`.
fn body(g: f64, screening: Screening, k: f64) -> CraterParams {
    CraterParams::new(CraterParamsParts {
        n_1km: PerSquareKilometre::new(0.01),
        screening,
        gravity: MetresPerSecondSquared::new(g),
        k_target: k,
        impact_velocity: None,
    })
    .unwrap()
}

/// An atmosphere of 636 Pa at gravity `g` over stony projectiles: Mars's, enough to screen and so
/// to keep every bowl at 0.20 D.
fn thin_air(g: f64) -> Screening {
    Screening::Atmosphere {
        column_mass: KilogramsPerSquareMetre::new(636.0 / g),
        projectile_density: KilogramsPerCubicMetre::new(3_000.0),
    }
}

/// The airless Moon.
fn moon() -> CraterParams {
    body(1.62, Screening::None, 1.0)
}

/// Mercury, airless at 3.70 m s⁻² (Susorney et al. 2016's body).
fn mercury() -> CraterParams {
    body(3.70, Screening::None, 1.0)
}

/// Mars at 3.71 m s⁻² under its air.
fn mars() -> CraterParams {
    body(3.71, thin_air(3.71), 1.0)
}

/// The shape of a crater `d_km` kilometres wide on `p`.
fn shape_km(p: &CraterParams, d_km: f64) -> CraterShape {
    crater_shape(p, Metres::new(d_km * 1e3))
}

/// `coefficient` D^`exponent` in km for D in km, a published fit.
fn fit(law: (f64, f64), d_km: f64) -> f64 {
    law.0 * math::powf(d_km, law.1)
}

/// Asserts `value` lies within `tolerance` (a fraction) of `expected`.
fn assert_within(what: &str, value: f64, expected: f64, tolerance: f64) {
    assert!(
        (value / expected - 1.0).abs() <= tolerance,
        "{what}: {value} against {expected}, beyond {tolerance}"
    );
}

#[test]
fn the_transition_diameter_gives_pikes_moon_and_earth() {
    // Pike 1980a, Table 3: 19 km on the Moon and 3.1 km on Earth (its Mercury 16 km and Mars
    // 5.8 km are the plan's Mercury–Mars anomaly, Risks), within 10%.
    let moon = transition_diameter(&moon()).value();
    assert_within("the Moon's D_t", moon, 19_000.0, 0.10);
    let earth = transition_diameter(&body(9.81, thin_air(9.81), 1.0)).value();
    assert_within("Earth's D_t", earth, 3_100.0, 0.10);
    // An ice-rich crust's 0.12 at Ganymede's gravity: 2.6 km.
    let ganymede = transition_diameter(&body(1.428, Screening::None, 0.12)).value();
    assert_within("Ganymede's D_t", ganymede, 2_590.0, 0.01);
}

#[test]
fn the_morphology_follows_the_zone() {
    let p = mars();
    let t = transition_diameter(&p).value();
    for (x, expected) in [
        (0.79, Morphology::Simple),
        (0.8, Morphology::Transitional),
        (1.49, Morphology::Transitional),
        (1.5, Morphology::CentralPeak),
        (8.9, Morphology::CentralPeak),
        (9.0, Morphology::PeakRing),
        (15.9, Morphology::PeakRing),
        (16.0, Morphology::MultiRingBasin),
    ] {
        assert_eq!(
            crater_shape(&p, Metres::new(x * t)).morphology(),
            expected,
            "{x} D_t"
        );
    }
}

#[test]
fn the_bowl_is_pikes_and_stopars() {
    // 0.20 D deep with a rim of 0.0369 D and a = 1.5426 (the balance at d ÷ h = 5.42; the
    // plan's 1.54 rounded) to 10⁻³, on every body with air and on the Moon from 400 m.
    for (p, d_km) in [
        (mars(), 1.0),
        (moon(), 3.0),
        (body(9.81, thin_air(9.81), 1.0), 0.05),
    ] {
        let s = shape_km(&p, d_km);
        assert_eq!(s.morphology(), Morphology::Simple);
        let d = s.diameter().value();
        assert!((s.depth().value() / d - 0.20).abs() < 1e-12);
        assert!((s.rim_height().value() / d - 0.0369).abs() < 1e-5);
        assert!(
            (s.exterior_width() - 1.5426).abs() < 1e-3,
            "{}",
            s.exterior_width()
        );
        assert_same_bits(s.floor_radius().value(), 0.0);
        assert_same_bits(s.peak_relief().value(), 0.0);
    }
    // Within 12% of Pike 1980b's 0.196 D^1.010, Stopar et al.'s 0.209 D, Mercury's 0.199 D^0.995
    // (Pike 1988) and Robbins and Hynek's deepest Martian 0.179 D^1.012 at 1–5 km.
    for d_km in [1.0, 2.0, 3.0, 4.0, 5.0] {
        let depth = shape_km(&mars(), d_km).depth().value() / 1e3;
        for (source, law) in [
            ("Pike 1980b", (0.196, 1.010)),
            ("Stopar et al.", (0.209, 1.0)),
            ("Pike 1988", (0.199, 0.995)),
            ("Robbins and Hynek", (0.179, 1.012)),
        ] {
            assert_within(source, depth, fit(law, d_km), 0.12);
        }
        // The rim within 12% of Pike 1980b's 0.036 D^1.014 and Stopar et al.'s "∼0.04".
        let rim = shape_km(&mars(), d_km).rim_height().value() / 1e3;
        assert_within("Pike 1980b's rim", rim, fit((0.036, 1.014), d_km), 0.12);
        assert_within("Stopar et al.'s rim", rim, 0.04 * d_km, 0.12);
    }
}

#[test]
fn airless_bowls_are_stopars_a_class() {
    // On an airless Moon d ÷ D is 0.125, 0.152 and 0.166 at 63, 141 and 283 m, held below 63 m,
    // and 0.20 from 400 m.
    let ratio = |p: &CraterParams, d_m: f64| {
        let s = crater_shape(p, Metres::new(d_m));
        s.depth().value() / d_m
    };
    for (d_m, expected) in [
        (20.0, 0.125),
        (63.0, 0.125),
        (141.0, 0.152),
        (283.0, 0.166),
        (400.0, 0.20),
        (900.0, 0.20),
    ] {
        assert!((ratio(&moon(), d_m) - expected).abs() < 1e-12, "{d_m} m");
    }
    // Linear in log D between the bins: halfway in log between 63 and 141 m.
    let mid = (63.0_f64 * 141.0).sqrt();
    assert!((ratio(&moon(), mid) - 0.1385).abs() < 1e-12);
    // Off the Moon at D × g ÷ 1.62: Mercury's 63 m bin is at 63 × 1.62 ÷ 3.70 m.
    assert!((ratio(&mercury(), 63.0 * 1.62 / 3.70) - 0.125).abs() < 1e-12);
    // Bodies with air keep 0.20 at every size.
    assert!((ratio(&mars(), 5.0) - 0.20).abs() < 1e-12);
    // The rim stays 0.0369 D.
    let s = crater_shape(&moon(), Metres::new(63.0));
    assert!((s.rim_height().value() / 63.0 - 0.0369).abs() < 1e-5);
}

#[test]
fn the_moons_complex_craters_are_kalynns_and_pikes() {
    // Depth between Kalynn et al.'s mare 0.870 D^0.352 and highland 1.558 D^0.254 fits, rim within
    // 1% of Pike 1980b's 0.236 D^0.399, at 30, 50, 100 and 150 km.
    for d_km in [30.0, 50.0, 100.0, 150.0] {
        let s = shape_km(&moon(), d_km);
        let depth = s.depth().value() / 1e3;
        let mare = fit((0.870, 0.352), d_km);
        let highland = fit((1.558, 0.254), d_km);
        assert!(
            depth > mare.min(highland) && depth < mare.max(highland),
            "{d_km} km: {depth} km outside {mare}–{highland}"
        );
        let rim = s.rim_height().value() / 1e3;
        assert_within("Pike 1980b's rim", rim, fit((0.236, 0.399), d_km), 0.01);
    }
}

#[test]
fn mercurys_complex_craters_are_susorneys() {
    // Depth within 15% of 1.02 D^0.20 and rim within 15% of 0.25 D^0.28, at 30, 50 and 100 km.
    for d_km in [30.0, 50.0, 100.0] {
        let s = shape_km(&mercury(), d_km);
        assert_within(
            "depth",
            s.depth().value() / 1e3,
            fit((1.02, 0.20), d_km),
            0.15,
        );
        assert_within(
            "rim",
            s.rim_height().value() / 1e3,
            fit((0.25, 0.28), d_km),
            0.15,
        );
    }
}

#[test]
fn mars_complex_craters_are_robbins_and_hyneks_and_tornabenes() {
    // Depth within 30% of Robbins and Hynek's 0.250 D^0.527 and Tornabene et al.'s 0.323 D^0.538
    // at 25, 50 and 80 km; rim within 15% of Robbins and Hynek's 0.025 D^0.820 at 50 and 80 km.
    for d_km in [25.0, 50.0, 80.0] {
        let depth = shape_km(&mars(), d_km).depth().value() / 1e3;
        assert_within("Robbins and Hynek", depth, fit((0.250, 0.527), d_km), 0.30);
        assert_within("Tornabene et al.", depth, fit((0.323, 0.538), d_km), 0.30);
    }
    for d_km in [50.0, 80.0] {
        let rim = shape_km(&mars(), d_km).rim_height().value() / 1e3;
        assert_within(
            "Robbins and Hynek's rim",
            rim,
            fit((0.025, 0.820), d_km),
            0.15,
        );
    }
}

#[test]
fn complex_craters_scale_with_gravity_and_bowls_do_not() {
    // At a fixed D in the complex range, d ∝ g^−0.697 and h ∝ g^−0.601 (the ruling's g^−0.70 and
    // g^−0.60: 1 − 0.303 and 1 − 0.399), to 10⁻⁹.
    let d = Metres::new(200e3);
    let (g1, g2) = (1.62, 3.71);
    let a = crater_shape(&body(g1, Screening::None, 1.0), d);
    let b = crater_shape(&body(g2, Screening::None, 1.0), d);
    assert_eq!(a.morphology(), Morphology::PeakRing);
    assert_eq!(b.morphology(), Morphology::MultiRingBasin);
    let ratio = g2 / g1;
    let depth = b.depth().value() / a.depth().value();
    let rim = b.rim_height().value() / a.rim_height().value();
    assert!(
        (depth / math::powf(ratio, -0.697) - 1.0).abs() < 1e-9,
        "{depth}"
    );
    assert!(
        (rim / math::powf(ratio, -0.601) - 1.0).abs() < 1e-9,
        "{rim}"
    );
    // A bowl's d ÷ D and h ÷ D are the same at every gravity.
    let d = Metres::new(500.0);
    let bowls: Vec<CraterShape> = [1.62, 3.71, 9.81, 24.0]
        .iter()
        .map(|&g| crater_shape(&body(g, thin_air(g), 1.0), d))
        .collect();
    for s in &bowls {
        assert_eq!(s.morphology(), Morphology::Simple);
        assert_same_bits(s.depth().value(), bowls[0].depth().value());
        assert_same_bits(s.rim_height().value(), bowls[0].rim_height().value());
    }
}

#[test]
fn the_shape_is_continuous_and_rising_across_the_transition_zone() {
    for p in [moon(), mars(), body(1.428, Screening::None, 0.12)] {
        let t = transition_diameter(&p).value();
        // Continuous at 0.8 and 1.5 D_t.
        for x in [TRANSITIONAL_FROM, COMPLEX_FROM] {
            let below = crater_shape(&p, Metres::new(x * t * (1.0 - 1e-12)));
            let at = crater_shape(&p, Metres::new(x * t));
            for (what, f) in [
                ("depth", CraterShape::depth as fn(&CraterShape) -> Metres),
                ("rim", CraterShape::rim_height),
                ("floor", CraterShape::floor_radius),
            ] {
                let (u, v) = (f(&below).value(), f(&at).value());
                assert!(
                    (u - v).abs() <= 1e-9 * v.abs().max(1.0),
                    "{what} at {x} D_t: {u} {v}"
                );
            }
        }
        // Non-decreasing from 0.7 to 1.7 D_t.
        let mut previous = crater_shape(&p, Metres::new(0.7 * t));
        for k in 1..=1_000 {
            let x = 0.7 + f64::from(k) * 1e-3;
            let s = crater_shape(&p, Metres::new(x * t));
            assert!(s.depth() >= previous.depth(), "depth falls at {x} D_t");
            assert!(
                s.rim_height() >= previous.rim_height(),
                "rim falls at {x} D_t"
            );
            assert!(
                s.floor_radius() >= previous.floor_radius(),
                "floor falls at {x} D_t"
            );
            previous = s;
        }
    }
}

/// ∫ z 2π r dr of `s`'s profile over r from `a` to `b` rim radii, in units of R² metres, by
/// eight-point Gauss–Legendre, exact on each piece (polynomials of degree five at most).
fn volume(s: &CraterShape, a: f64, b: f64) -> f64 {
    let half = 0.5 * (b - a);
    let mid = f64::midpoint(a, b);
    let mut sum = 0.0;
    for (node, weight) in GAUSS_LEGENDRE_8 {
        let r = mid + half * node;
        sum += weight * s.relief(r).value() * 2.0 * core::f64::consts::PI * r;
    }
    half * sum
}

/// The profile's pieces' ends in rim radii, ascending.
fn pieces(s: &CraterShape) -> Vec<f64> {
    let radius = s.diameter().value() / 2.0;
    let mut ends = vec![
        0.0,
        s.peak_radius().value() / radius,
        s.floor_radius().value() / radius,
        1.0,
        1.0 + s.exterior_width(),
    ];
    ends.sort_by(f64::total_cmp);
    ends.dedup();
    ends
}

#[test]
fn every_shape_balances_its_volume() {
    let bodies = [
        ("moon", moon()),
        ("mars", mars()),
        ("mercury", mercury()),
        ("icy", body(1.428, Screening::None, 0.12)),
        ("earth", body(9.81, thin_air(9.81), 1.0)),
    ];
    let mut widest: f64 = 0.0;
    for (name, p) in bodies {
        let t = transition_diameter(&p).value();
        // D ÷ D_t from 10⁻⁴ to 100, 60 a decade.
        for k in 0..=360 {
            let x = math::exp10(-4.0 + f64::from(k) / 60.0);
            let s = crater_shape(&p, Metres::new(x * t));
            let ends = pieces(&s);
            let interior: f64 = ends
                .windows(2)
                .filter(|w| w[1] <= 1.0)
                .map(|w| volume(&s, w[0], w[1]))
                .sum();
            let exterior = volume(&s, 1.0, 1.0 + s.exterior_width());
            assert!(interior < 0.0, "{name} at {x} D_t holds no cavity");
            assert!(
                ((interior + exterior) / interior).abs() < 1e-9,
                "{name} at {x} D_t: interior {interior}, exterior {exterior}"
            );
            widest = widest.max(s.exterior_width());
            assert!(
                s.exterior_width() <= 1.543,
                "{name} at {x} D_t: a = {}",
                s.exterior_width()
            );
        }
    }
    assert!((widest - 1.5426).abs() < 1e-3, "{widest}");
}

#[test]
fn the_exterior_widths_are_the_rulings() {
    let p = mars();
    let t = transition_diameter(&p).value();
    let a = |x: f64| crater_shape(&p, Metres::new(x * t)).exterior_width();
    // Complex craters to 25 D_t lie in 1.28–1.36, and the widest basin's narrows to 1.05 at
    // 100 D_t; transitional ones fall from the bowl's 1.5426 at 0.8 D_t to 1.33 at 1.5 D_t.
    for k in 0..=100 {
        let x = 1.5 * math::powf(25.0 / 1.5, f64::from(k) / 100.0);
        assert!((1.28..=1.36).contains(&a(x)), "a = {} at {x} D_t", a(x));
        let x = 0.8 * math::powf(1.5 / 0.8, f64::from(k) / 101.0);
        assert!((1.33..=1.543).contains(&a(x)), "a = {} at {x} D_t", a(x));
    }
    assert!((a(100.0) - 1.05).abs() < 0.01, "{}", a(100.0));
    // The airless regolith's bowls span 0.73 (at 63 m and below) to the bowl's 1.54.
    let small = crater_shape(&moon(), Metres::new(40.0)).exterior_width();
    assert!((small - 0.73).abs() < 0.01, "{small}");
    // An absurd basin, beyond any body, holds no net cavity and has no exterior, never a NaN.
    let huge = crater_shape(&p, Metres::new(1e7 * t));
    assert_same_bits(huge.exterior_width(), 0.0);
}

#[test]
fn the_profile_meets_its_landmarks() {
    let s = shape_km(&body(1.62, Screening::None, 1.0), 60.0);
    assert_eq!(s.morphology(), Morphology::CentralPeak);
    let radius = s.diameter().value() / 2.0;
    let h = s.rim_height().value();
    let d0 = s.floor_depth().value();
    // The rim crest, the floor, the peak's apex, and nothing beyond the reach.
    assert!((s.relief(1.0).value() - h).abs() < 1e-9);
    let floor_r = s.floor_radius().value() / radius;
    assert!((s.relief(floor_r).value() + d0).abs() < 1e-9);
    assert!((s.relief(0.0).value() - (s.peak_relief().value() - d0)).abs() < 1e-9);
    let reach = s.reach().value() / radius;
    assert_same_bits(s.relief(reach).value(), 0.0);
    assert_same_bits(s.relief(reach + 1.0).value(), 0.0);
    // Continuous at the rim and the floor's edge.
    for r in [1.0, floor_r, s.peak_radius().value() / radius] {
        let below = s.relief(r * (1.0 - 1e-12)).value();
        let above = s.relief(r * (1.0 + 1e-12)).value();
        assert!(
            (below - above).abs() < 1e-6,
            "at {r} rim radii: {below} {above}"
        );
    }
    // The ejecta's edge at 1 + a is 2.54 rim radii at most.
    assert!(reach <= 2.543);
    assert_eq!(s.depth() - s.rim_height(), s.floor_depth());
}

#[test]
fn a_shape_is_the_same_twice() {
    let p = mars();
    for d in [Metres::new(3.0), Metres::new(9e3), Metres::new(400e3)] {
        let (a, b) = (crater_shape(&p, d), crater_shape(&p, d));
        assert_eq!(a.morphology(), b.morphology());
        for (x, y) in [
            (a.depth(), b.depth()),
            (a.rim_height(), b.rim_height()),
            (a.floor_radius(), b.floor_radius()),
            (a.peak_relief(), b.peak_relief()),
            (a.peak_radius(), b.peak_radius()),
            (a.relief(0.5), b.relief(0.5)),
            (a.relief(1.7), b.relief(1.7)),
        ] {
            assert_same_bits(x.value(), y.value());
        }
        assert_same_bits(a.exterior_width(), b.exterior_width());
    }
}
