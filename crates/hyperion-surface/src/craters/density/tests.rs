//! Tests of the crater density: the parameter, the polynomial's ends, monotonicity, screening
//! against Design note 12's figures and Venus's catalogue, the diameter map against Ivanov 2001's
//! Mars, its target classes and densities against Singer et al. 2019's Charon, and the inversion
//! within an octave.

use super::*;
use crate::craters::{
    CraterParamsParts, CraterTarget, SOLAR_SYSTEM_AGE, Screening, TargetClass, lunar_chronology,
};
use crate::test_sizes::per_target;
use hyperion_base::units::{
    Gigayears, KilogramsPerCubicMetre, KilogramsPerSquareMetre, MetresPerSecond,
    MetresPerSecondSquared,
};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Venus's volumetric mean radius, km (NASA's Venus fact sheet: 6,051.8 km).
const VENUS_RADIUS_KM: f64 = 6_051.8;

/// A surface gravity from GM, m³ s⁻², and a volumetric mean radius, m.
fn gravity(gm: f64, radius_m: f64) -> f64 {
    gm / (radius_m * radius_m)
}

/// Venus's surface gravity, m s⁻²: GM 3.248 59 × 10¹⁴ m³ s⁻² (JPL's DE440, which NASA's Venus
/// fact sheet rounds to 3.2486) over its radius (the fact sheet's), 8.87.
fn venus_gravity() -> f64 {
    gravity(3.248_59e14, VENUS_RADIUS_KM * 1e3)
}

/// Mars's surface gravity, m s⁻²: GM 4.282 837 × 10¹³ m³ s⁻² (JPL's DE440) over 3,389.5 km
/// (NASA's Mars fact sheet), 3.73, as the sim's reference Mars has it.
fn mars_gravity() -> f64 {
    gravity(4.282_837e13, 3_389.5e3)
}

/// Earth's surface gravity, m s⁻²: GM 3.986 004 418 × 10¹⁴ m³ s⁻² (the IERS Conventions 2010)
/// over 6,371.0 km (NASA's Earth fact sheet), 9.82.
fn earth_gravity() -> f64 {
    gravity(3.986_004_418e14, 6_371.0e3)
}

/// A contract of lunar-equivalent N(>1 km) `n`, gravity `g`, the screening `screening`, target
/// factor `k` and impact velocity `v` (m/s), on the lunar target, by stony projectiles, in a
/// system of the Solar System's age.
fn params(n: f64, g: f64, screening: Screening, k: f64, v: Option<f64>) -> CraterParams {
    CraterParams::new(CraterParamsParts {
        n_1km: PerSquareKilometre::new(n),
        screening,
        gravity: MetresPerSecondSquared::new(g),
        k_target: k,
        impact_velocity: v.map(MetresPerSecond::new),
        projectile_density: LUNAR_PROJECTILE_DENSITY,
        target: LUNAR_TARGET,
        system_age: SOLAR_SYSTEM_AGE,
    })
    .unwrap()
}

/// `p` with projectiles of `projectile` kg m⁻³ striking a target of `class` and `density` kg m⁻³.
fn struck(p: &CraterParams, projectile: f64, class: TargetClass, density: f64) -> CraterParams {
    CraterParams::new(CraterParamsParts {
        projectile_density: KilogramsPerCubicMetre::new(projectile),
        target: CraterTarget {
            class,
            density: KilogramsPerCubicMetre::new(density),
        },
        ..*p.parts()
    })
    .unwrap()
}

/// An atmosphere of surface pressure `pressure` (Pa) at gravity `g`.
fn air(pressure: f64, g: f64) -> Screening {
    Screening::Atmosphere {
        column_mass: KilogramsPerSquareMetre::new(pressure / g),
    }
}

/// The Moon, unscreened, at saturation's N(>1 km).
fn moon() -> CraterParams {
    params(0.018, 1.62, Screening::None, 1.0, None)
}

/// Venus at science-r09-craters.md F4.1's lunar-equivalent N☾(>1 km), 3.36 × 10⁻⁴ km⁻², under
/// 92 bar (NASA's Venus fact sheet), struck at P14.T48.f's 25.7 km/s, through the map.
fn venus() -> CraterParams {
    let g = venus_gravity();
    params(3.36e-4, g, air(9.2e6, g), 1.0, Some(25_700.0))
}

/// Venus as R09.T0.b fitted it, at N(>1 km) = 3.07 × 10⁻⁴ km⁻² with no diameter map (R09.T7.a's
/// check).
fn venus_unmapped() -> CraterParams {
    let g = venus_gravity();
    params(3.07e-4, g, air(9.2e6, g), 1.0, None)
}

/// Mars at NASA's mean surface pressure of 636 Pa, struck at P14.T48.f's 10.6 km/s
/// (science-r09-craters.md item 4), through the map.
fn mars() -> CraterParams {
    let g = mars_gravity();
    params(2.0e-3, g, air(636.0, g), 1.0, Some(10_600.0))
}

/// Earth at the standard atmosphere's 101,325 Pa.
fn earth() -> CraterParams {
    let g = earth_gravity();
    params(1.0e-3, g, air(101_325.0, g), 1.0, None)
}

/// An airless ice-rich body (`k_target` 0.12) at Ganymede's gravity, 1.428 m s⁻², struck at
/// 20 km/s, through the map, on non-porous ice of 940 kg m⁻³.
fn icy() -> CraterParams {
    struck(
        &params(0.05, 1.428, Screening::None, 0.12, Some(20_000.0)),
        3_000.0,
        TargetClass::NonPorous,
        940.0,
    )
}

/// A porous, ice-rich moon about Hyperion's (g 0.02 m s⁻², bulk density 544 kg m⁻³), struck by
/// comets of 500 kg m⁻³ at 5 km/s: illustrative.
fn porous() -> CraterParams {
    struck(
        &params(0.02, 0.02, Screening::None, 0.12, Some(5_000.0)),
        500.0,
        TargetClass::Porous,
        544.0,
    )
}

/// An icy moon about Titan's (g 1.352 m s⁻², 146.7 kPa), struck by comets of 500 kg m⁻³ at
/// 10.5 km/s on non-porous ice: illustrative, a thick air over light projectiles.
fn titan() -> CraterParams {
    struck(
        &params(1e-4, 1.352, air(146_700.0, 1.352), 0.12, Some(10_500.0)),
        500.0,
        TargetClass::NonPorous,
        940.0,
    )
}

/// Every contract the tests sweep.
fn every_body() -> [(&'static str, CraterParams); 10] {
    [
        ("moon", moon()),
        ("venus", venus()),
        ("venus unmapped", venus_unmapped()),
        ("mars", mars()),
        ("earth", earth()),
        ("icy", icy()),
        ("porous", porous()),
        ("titan", titan()),
        (
            "lunar reference",
            params(
                0.02,
                1.62,
                Screening::None,
                1.0,
                Some(LUNAR_IMPACT_VELOCITY.value()),
            ),
        ),
        (
            "cutoff",
            params(
                0.01,
                9.81,
                Screening::Cutoff {
                    diameter: Metres::new(250.0),
                },
                1.0,
                None,
            ),
        ),
    ]
}

/// `d_km` kilometres as metres.
fn km(d_km: f64) -> Metres {
    Metres::new(d_km * 1e3)
}

#[test]
fn n_1km_is_the_parameter_exactly_at_the_identity() {
    for (name, p) in every_body() {
        let n = p.n_1km().value();
        let own = own_density_at_1km(&p).value();
        assert_same_bits(production_density(&p, km(1.0)).value(), own);
        if DiameterMap::of(&p).is_none() {
            // Without a map, or at the lunar reference, N(>1 km) is the parameter itself.
            assert_same_bits(own, n);
            // An unscreened body's density at 1 km is the published N(>1 km) (P14.T48.c's test).
            if crater_cutoff(&p).is_none() {
                assert_same_bits(cumulative_density(&p, km(1.0)).value(), n);
            }
        } else {
            assert!(
                own.total_cmp(&n).is_ne(),
                "{name}'s map is not the identity"
            );
        }
        assert!(cumulative_density(&p, km(1.0)).value() <= own, "{name}");
    }
}

#[test]
fn the_map_is_the_identity_at_the_lunar_reference_exactly() {
    // The lunar target, stony projectiles, 16.7 km/s, 1.62 m s⁻² and k_target 1 are the
    // reference: no map, so every density is the unmapped one to the bit, screened or not.
    let reference = |screening: Screening| {
        params(
            0.02,
            1.62,
            screening,
            1.0,
            Some(LUNAR_IMPACT_VELOCITY.value()),
        )
    };
    let lunar = |screening: Screening| params(0.02, 1.62, screening, 1.0, None);
    for screening in [Screening::None, air(500.0, 1.62)] {
        let (a, b) = (reference(screening), lunar(screening));
        assert!(DiameterMap::of(&a).is_none());
        for d_km in [1e-3, 0.01, 0.5, 1.0, 7.0, 19.0, 40.0, 300.0, 900.0] {
            assert_same_bits(
                production_density(&a, km(d_km)).value(),
                production_density(&b, km(d_km)).value(),
            );
            assert_same_bits(
                cumulative_density(&a, km(d_km)).value(),
                cumulative_density(&b, km(d_km)).value(),
            );
        }
    }
    // Any departure from it maps: another speed, gravity, target, class or projectile.
    let base = reference(Screening::None);
    for p in [
        params(0.02, 1.62, Screening::None, 1.0, Some(16_000.0)),
        params(
            0.02,
            1.7,
            Screening::None,
            1.0,
            Some(LUNAR_IMPACT_VELOCITY.value()),
        ),
        params(
            0.02,
            1.62,
            Screening::None,
            0.5,
            Some(LUNAR_IMPACT_VELOCITY.value()),
        ),
        struck(&base, 3_000.0, TargetClass::NonPorous, 940.0),
        struck(&base, 3_000.0, TargetClass::Porous, 2_550.0),
        struck(&base, 500.0, TargetClass::NonPorous, 2_550.0),
    ] {
        assert!(DiameterMap::of(&p).is_some(), "{:?}", p.parts());
    }
}

#[test]
fn the_polynomial_holds_neukums_coefficients_with_a0_as_the_parameter() {
    // Neukum 1983's a0, −3.0768, is log₁₀ of the chronology's N(>1 km) at 1 Gyr, 8.38 × 10⁻⁴
    // (science-r09-craters.md F3): the full published polynomial at a0 is the lunar shape times
    // that density.
    let a0: f64 = -3.0768;
    let n_1gyr = 5.44e-14 * (math::exp(6.93) - 1.0) + 8.38e-4;
    assert!(
        (math::log10(n_1gyr) - a0).abs() < 5e-5,
        "{}",
        math::log10(n_1gyr)
    );
    let p = params(math::exp10(a0), 1.62, Screening::None, 1.0, None);
    // Craterstats' polynomial at 10 km, evaluated term by term from a0.
    let x: f64 = 1.0;
    let full = a0 + NEUKUM.iter().sum::<f64>() * x;
    let ours = math::log10(production_density(&p, km(10.0)).value());
    assert!((ours - full).abs() < 1e-12, "{ours} against {full}");
    // The upper end is log₁₀ 300.
    assert!((LOG_D_MAX - math::log10(300.0)).abs() < 1e-15);
}

#[test]
fn the_end_slopes_extend_the_polynomial_continuously() {
    let p = moon();
    let n = |d_km: f64| production_density(&p, km(d_km)).value();
    // Continuous at both ends.
    for end in [0.01, 300.0] {
        let below = n(end * (1.0 - 1e-9));
        let above = n(end * (1.0 + 1e-9));
        assert!(((below - above) / above).abs() < 1e-7, "{end} km");
    }
    // −2 below 10 m: halving D quadruples N.
    let ratio = n(0.0025) / n(0.005);
    assert!((ratio - 4.0).abs() < 1e-9, "{ratio}");
    // The polynomial's own slope at 300 km above it, −1.83, continuous in slope there.
    let slope = |a: f64, b: f64| math::log10(n(b) / n(a)) / math::log10(b / a);
    let above = slope(400.0, 800.0);
    let at = neukum_slope(LOG_D_MAX);
    assert!(
        (above - at).abs() < 1e-9 && (at + 1.826).abs() < 1e-3,
        "{above} {at}"
    );
    let (inside, outside) = (lunar_slope(LOG_D_MAX - 1e-9), lunar_slope(LOG_D_MAX + 1e-9));
    assert!((inside - outside).abs() < 1e-8, "{inside} {outside}");
    // Every slope inside the range is negative, so the production falls everywhere.
    for k in 0..=1_000 {
        let x = LOG_D_MIN + (LOG_D_MAX - LOG_D_MIN) * f64::from(k) / 1_000.0;
        assert!(neukum_slope(x) < 0.0, "slope at log D {x}");
    }
}

#[test]
fn the_density_is_monotone() {
    for (name, p) in every_body() {
        let mut previous = f64::INFINITY;
        let mut previous_production = f64::INFINITY;
        // 1 mm to 10,000 km, in steps of 2.3%, and the 1,000 D_c where the quadrature ends.
        let mut sizes: Vec<f64> = (0..=1_000)
            .map(|k| 1e-3 * math::exp(f64::from(k) * 0.023_025_850_929_940_46))
            .collect();
        if let Some(cutoff) = crater_cutoff(&p) {
            let end = cutoff.value() * SCREENED_REACH;
            sizes.extend([end * (1.0 - 1e-12), end, end * (1.0 + 1e-12)]);
            sizes.sort_by(f64::total_cmp);
        }
        for d in sizes.into_iter().map(Metres::new) {
            let n = cumulative_density(&p, d).value();
            assert!(n.is_finite() && n > 0.0, "{name} at {} m: {n}", d.value());
            // Non-increasing to the last bit: under a thick atmosphere the screened density is
            // flat in f64 far below the cutoff, where the taper leaves under 10⁻¹⁶ of it.
            assert!(
                n <= previous,
                "{name}: N(>{} m) = {n} rises above {previous}",
                d.value()
            );
            previous = n;
            let production = production_density(&p, d).value();
            assert!(
                production < previous_production,
                "{name}: the production at {} m does not fall",
                d.value()
            );
            previous_production = production;
            assert!(n <= production * (1.0 + 1e-12), "{name} at {} m", d.value());
        }
    }
}

#[test]
fn the_projectile_scales_and_cutoffs_are_design_note_12s() {
    // d* = 5.2 m, 0.52 km and 8.5 cm for Earth, Venus and Mars at 636 Pa.
    for (name, p, expected) in [
        ("earth", earth(), 5.2),
        ("venus", venus(), 520.0),
        ("mars", mars(), 0.085),
    ] {
        let d_star = projectile_scale(&p).unwrap().value();
        assert!(
            (d_star / expected - 1.0).abs() < 0.01,
            "{name}: d* = {d_star} m against {expected} m"
        );
        assert_same_bits(crater_cutoff(&p).unwrap().value(), 20.0 * d_star);
    }
    assert_eq!(projectile_scale(&moon()), None);
    assert_eq!(crater_cutoff(&moon()), None);
    // An atmosphere of no column screens nothing.
    let none = params(0.01, 1.62, air(0.0, 1.62), 1.0, None);
    assert_eq!(crater_cutoff(&none), None);
    assert_same_bits(
        cumulative_density(&none, km(0.1)).value(),
        production_density(&moon_at(0.01), km(0.1)).value(),
    );
    // A cutoff given directly names no projectile.
    let cutoff = params(
        0.01,
        9.81,
        Screening::Cutoff {
            diameter: Metres::new(250.0),
        },
        1.0,
        None,
    );
    assert_eq!(projectile_scale(&cutoff), None);
    assert_eq!(crater_cutoff(&cutoff), Some(Metres::new(250.0)));
}

/// The Moon at N(>1 km) `n`.
fn moon_at(n: f64) -> CraterParams {
    params(n, 1.62, Screening::None, 1.0, None)
}

#[test]
fn the_taper_passes_design_note_12s_shares_on_venus() {
    // 0.5% of the production at 3 km, 4.5% at 5 km, 25% at 8 km, 46% at 10 km and 94% at 20 km
    // (science-r09-craters.md F1).
    let ln_cutoff = math::ln(crater_cutoff(&venus()).unwrap().value() / 1e3);
    for (d_km, share) in [
        (3.0, 0.005),
        (5.0, 0.045),
        (8.0, 0.25),
        (10.0, 0.46),
        (20.0, 0.94),
    ] {
        let f = taper(math::ln(d_km), ln_cutoff);
        assert!(
            (f - share).abs() < 0.5 * share.min(0.1),
            "{d_km} km: {f} against {share}"
        );
    }
}

/// The IAU Gazetteer's named Venusian craters at or above 3, 5, 10, 20 and 40 km (Design note
/// 12): 878, 850, 642, 335 and 104.
const GAZETTEER: [(f64, f64); 5] = [
    (3.0, 878.0),
    (5.0, 850.0),
    (10.0, 642.0),
    (20.0, 335.0),
    (40.0, 104.0),
];

/// The worst relative error of `p`'s screened counts over Venus's area against [`GAZETTEER`].
fn worst_against_the_gazetteer(p: &CraterParams) -> f64 {
    let area = 4.0 * core::f64::consts::PI * VENUS_RADIUS_KM * VENUS_RADIUS_KM;
    let mut worst: f64 = 0.0;
    for (d_km, named) in GAZETTEER {
        let count = cumulative_density(p, km(d_km)).value() * area;
        worst = worst.max((count / named - 1.0).abs());
    }
    worst
}

#[test]
fn venus_screened_counts_match_the_gazetteer_through_the_map() {
    // At F4.1's N☾(>1 km) = 3.36 × 10⁻⁴ km⁻², 25.7 km/s and 20 d*: within 12% at 3–40 km (10.8%
    // computed by science-r09-craters.md F4.1, at 20 km).
    let worst = worst_against_the_gazetteer(&venus());
    assert!(worst < 0.12, "{worst}");
    // What the build gives, pinned so that a change is seen.
    assert!(worst < 0.11, "{worst}");
}

#[test]
fn venus_screened_counts_match_the_gazetteer_without_the_map() {
    // R09.T7.a's check, on the unmapped lunar shape at N(>1 km) = 3.07 × 10⁻⁴ km⁻², as R09.T0.b
    // fitted it: within 10%.
    let worst = worst_against_the_gazetteer(&venus_unmapped());
    assert!(worst < 0.10, "{worst}");
}

#[test]
fn venus_screened_density_at_1_km_is_far_below_its_production() {
    // Venus's own N(>1 km) is 0.53 of its N☾ (F4.1), and its air passes about 1% of it.
    let p = venus();
    let own = own_density_at_1km(&p).value();
    assert!((own / p.n_1km().value() - 0.532).abs() < 0.005, "{own}");
    let screened = cumulative_density(&p, km(1.0)).value();
    assert!(screened < 0.02 * own, "{screened} against {own}");
    assert!((1.8e-6..2.1e-6).contains(&screened), "{screened}");
}

/// The screened density by composite Simpson's rule in ln D, 256 points a decade, independent of
/// the panels of [`cumulative_density`], to 10,000 `D_c` with the closed form above.
fn simpson_density(p: &CraterParams, d_km: f64) -> f64 {
    let production = Production::of(p);
    let cutoff = crater_cutoff(p).unwrap().value() / 1e3;
    let (lo, hi) = (math::ln(d_km), math::ln(cutoff * 1e4));
    let integrand = |u: f64| taper(u, math::ln(cutoff)) * production.differential(math::exp(u));
    // Simpson's rule from a to b, an even number of intervals, at least 256 a decade. The ends are
    // the integrand's one-sided limits, read 10⁻⁹ inside them: at a kink the differential jumps,
    // and the kink's own value may fall on either side of it.
    let simpson = |a: f64, b: f64| {
        let steps = 2.0 * ((b - a) / (core::f64::consts::LN_10 / 256.0) / 2.0).ceil();
        let h = (b - a) / steps;
        let mut sum = integrand(a + 1e-9) + integrand(b - 1e-9);
        let mut k = 1.0;
        let mut odd = true;
        while k < steps {
            sum += if odd { 4.0 } else { 2.0 } * integrand(a + h * k);
            odd = !odd;
            k += 1.0;
        }
        sum * h / 3.0
    };
    // Split where the integrand jumps, at the production's kinks.
    let mut total = 0.0;
    let mut start = lo;
    for kink in production.kinks().map(math::ln) {
        if kink > start && kink < hi {
            total += simpson(start, kink);
            start = kink;
        }
    }
    total + simpson(start, hi) + production.cumulative(math::exp(hi))
}

#[test]
fn the_quadrature_agrees_with_an_independent_integration() {
    for (name, p) in [("venus", venus()), ("mars", mars()), ("earth", earth())] {
        let cutoff_km = crater_cutoff(&p).unwrap().value() / 1e3;
        for scale in [1e-3, 0.1, 0.5, 1.0, 2.0, 10.0, 300.0] {
            let d_km = cutoff_km * scale;
            let ours = cumulative_density(&p, km(d_km)).value();
            let reference = simpson_density(&p, d_km);
            assert!(
                ((ours - reference) / reference).abs() < 1e-6,
                "{name} at {d_km} km: {ours} against {reference}"
            );
        }
    }
}

/// Ivanov 2001's Mars production function as Craterstats lists it ("Mars, Ivanov (2001)"), a0
/// to a11, D in km.
const IVANOV_MARS: [f64; 12] = [
    -3.383_677,
    -3.197_453,
    1.256_814,
    7.915_374e-1,
    -4.860_814e-1,
    -3.630_098e-1,
    1.015_683e-1,
    6.755_923e-2,
    -1.180_639e-2,
    -4.753_462e-3,
    6.232_845e-4,
    5.805_492e-5,
];

/// Ivanov's Mars N(>D) over N(>1 km).
fn ivanov_shape(d_km: f64) -> f64 {
    let x = math::log10(d_km);
    let mut acc = 0.0;
    for a in IVANOV_MARS[1..].iter().rev() {
        acc = acc * x + a;
    }
    math::exp10(acc * x)
}

/// The worst factor between `p`'s production and Ivanov's Mars function over 1–100 km, both
/// normalised at 1 km, 20 diameters a decade.
fn worst_against_ivanov(p: &CraterParams) -> f64 {
    let own = own_density_at_1km(p).value();
    let mut worst: f64 = 1.0;
    for k in 0..=40 {
        let d_km = math::exp10(f64::from(k) / 20.0);
        let ratio = production_density(p, km(d_km)).value() / own / ivanov_shape(d_km);
        worst = worst.max(ratio.max(1.0 / ratio));
    }
    worst
}

/// Mars at 3.71 m s⁻² (`D_t` 8.3 km), unscreened, struck at Ivanov 2001's ratio of mean impact
/// velocities, U = 16.7 × 9.6 ÷ 16.1 km/s against the reference's 16.7.
fn mars_at_ivanovs_velocities() -> CraterParams {
    params(
        1.0,
        3.71,
        Screening::None,
        1.0,
        Some(LUNAR_IMPACT_VELOCITY.value() * 9.6 / 16.1),
    )
}

#[test]
fn the_map_at_mars_ratios_reproduces_ivanov_2001() {
    // Mars's gravity 3.71 m s⁻² (D_t 8.3 km), unscreened, at Ivanov 2001's velocity ratio and at
    // P14.T48.f's 10.6 km/s: each within ×1.5 in N over 1–100 km, both normalised at 1 km
    // (science-r09-craters.md item 4 computes 0.97–1.03 and 0.92–1.03). The lunar shape alone is
    // not.
    let at_ratio = worst_against_ivanov(&mars_at_ivanovs_velocities());
    let modelled = worst_against_ivanov(&params(1.0, 3.71, Screening::None, 1.0, Some(10_600.0)));
    let lunar = worst_against_ivanov(&moon_at(1.0));
    assert!(
        at_ratio < 1.5,
        "the map is off Ivanov's Mars by ×{at_ratio}"
    );
    assert!(
        modelled < 1.5,
        "the map at 10.6 km/s is off Ivanov's Mars by ×{modelled}"
    );
    assert!(lunar > 2.0, "the lunar shape is off by ×{lunar} only");
    // What the build gives, pinned so that a change is seen.
    assert!(at_ratio < PINNED_IVANOV.0, "×{at_ratio}");
    assert!(modelled < PINNED_IVANOV.1, "×{modelled}");
}

/// The worst factors against Ivanov's Mars that the build gives, at his velocity ratio and at
/// P14.T48.f's 10.6 km/s (×1.039 and ×1.099), rounded up.
const PINNED_IVANOV: (f64, f64) = (1.04, 1.10);

#[test]
fn mars_own_density_implies_ivanovs_impact_rate() {
    // Under Mars's map at Ivanov's velocities the body's own N(>1 km) is 0.254 of N☾ within 2%
    // (0.256 as built), and the reference Mars's R_b = 1.92 times it is Ivanov's Mars chronology,
    // 0.4928 of the Moon's, within 1% (science-r09-craters.md F1 and F4.1).
    let p = mars_at_ivanovs_velocities();
    let factor = own_density_at_1km(&p).value() / p.n_1km().value();
    assert!((factor / 0.254 - 1.0).abs() < 0.02, "{factor}");
    assert!((1.92 * factor / 0.4928 - 1.0).abs() < 0.01, "{factor}");
}

#[test]
fn a_non_porous_map_is_a_pure_scale() {
    // For a non-porous body the lunar cavity is the body's to the power exactly 1, and libm's
    // pow(x, 1) is x, so the map's scale is P☾ ÷ P and the cavities scale by it to the bit: a
    // libm without that shortcut fails here, by name.
    let p = mars();
    let map = DiameterMap::of(&p).unwrap();
    assert_same_bits(map.exponent, 1.0);
    let body = transient_coefficient(
        TargetClass::NonPorous,
        LUNAR_TARGET.density,
        LUNAR_PROJECTILE_DENSITY,
        p.impact_velocity().unwrap(),
        p.gravity(),
    );
    let moon = transient_coefficient(
        TargetClass::NonPorous,
        LUNAR_TARGET.density,
        LUNAR_PROJECTILE_DENSITY,
        LUNAR_IMPACT_VELOCITY,
        MetresPerSecondSquared::new(1.62),
    );
    assert_same_bits(map.cavity_scale, (moon / 1e3) / (body / 1e3));
    for d_km in [1e-3, 0.5, 3.0] {
        assert!(d_km < map.transition_km && map.cavity_scale * d_km < 19.0);
        assert_same_bits(map.lunar(d_km), map.cavity_scale * d_km);
        assert_same_bits(math::powf(d_km, 1.0), d_km);
    }
}

#[test]
fn the_map_is_monotone_and_inverted_for_both_classes() {
    let mut mapped = 0;
    for (name, p) in every_body() {
        let Some(map) = DiameterMap::of(&p) else {
            continue;
        };
        mapped += 1;
        let mut previous = 0.0;
        for k in 0..=800 {
            let d_km = math::exp10(-4.0 + f64::from(k) / 100.0);
            // Strictly increasing, and inverted by `body`.
            let image = map.lunar(d_km);
            assert!(image > previous, "{name} at {d_km} km");
            previous = image;
            let back = map.body(image);
            assert!(
                ((back - d_km) / d_km).abs() < 1e-12,
                "{name}: {d_km} km returns as {back}"
            );
        }
    }
    assert_eq!(mapped, 5, "the mapped bodies, the porous one among them");
    assert_eq!(porous().target().class, TargetClass::Porous);
    assert!(DiameterMap::of(&porous()).is_some());
}

/// A crater on Charon (g 0.288 m s⁻², U 2.0 km/s, ice of `target` kg m⁻³ in `class`, `k_target`
/// 0.12) struck by icy projectiles of 920 kg m⁻³: Singer et al. 2019's speed and densities
/// (supplement), and the gravity GM ÷ R² from Charon's GM of 105.88 km³ s⁻² (Brozović et al. 2015,
/// Icarus 246, 317) and radius of 606.0 km (Nimmo et al. 2017, Icarus 287, 12), both from memory.
fn charon(class: TargetClass, target: f64) -> CraterParams {
    struck(
        &params(0.01, 0.288, Screening::None, 0.12, Some(2_000.0)),
        920.0,
        class,
        target,
    )
}

/// The diameter, metres, of the projectile whose cavity is `cavity_m` metres wide on `p`'s target
/// (the module documentation, step 2): 2 (D ÷ P)^((2 + μ) ÷ 2).
fn impactor_diameter(p: &CraterParams, cavity_m: f64) -> f64 {
    let target = p.target();
    let (mu, _) = class_constants(target.class);
    let coefficient = transient_coefficient(
        target.class,
        target.density,
        p.projectile_density(),
        p.impact_velocity().unwrap(),
        p.gravity(),
    );
    2.0 * math::powf(cavity_m / coefficient, f64::midpoint(2.0, mu))
}

#[test]
fn charons_impactors_are_singers() {
    // A 10 km simple crater on Charon needs a 1.37 km impactor in non-porous ice (920 kg m⁻³) and
    // a 1.59 km one in 30%-porous ice (644 kg m⁻³), to 1% (science-r09-craters.md F4.2), both
    // within 15% of Singer et al. 2019's worked example, 1.3 and 1.8 km (supplement): the porous
    // target takes the larger impactor.
    let crater_m = 10e3;
    let solid = charon(TargetClass::NonPorous, 920.0);
    let porous = charon(TargetClass::Porous, 644.0);
    for p in [&solid, &porous] {
        let shape = crate::craters::crater_shape(p, Metres::new(crater_m));
        assert_eq!(shape.morphology(), crate::field::Morphology::Simple);
        assert!(crater_m < transition_diameter(p).value());
    }
    // The class constants, the paraboloids of the bowls' 0.20 and 0.14 D (F4.2).
    assert!((transient_shape(TargetClass::NonPorous) - 3.764).abs() < 5e-4);
    assert!((transient_shape(TargetClass::Porous) - 4.239).abs() < 5e-4);
    let (a, b) = (
        impactor_diameter(&solid, crater_m),
        impactor_diameter(&porous, crater_m),
    );
    assert!((a / 1_370.0 - 1.0).abs() < 0.01, "{a} m");
    assert!((b / 1_590.0 - 1.0).abs() < 0.01, "{b} m");
    assert!((a / 1_300.0 - 1.0).abs() < 0.15, "{a} m");
    assert!((b / 1_800.0 - 1.0).abs() < 0.15, "{b} m");
    assert!(b > a);
    // So one projectile, the one that makes a given lunar crater, makes a narrower crater in the
    // porous target.
    let lunar_km = 5.0;
    let (solid_km, porous_km) = (
        DiameterMap::of(&solid).unwrap().body(lunar_km),
        DiameterMap::of(&porous).unwrap().body(lunar_km),
    );
    assert!(porous_km < solid_km, "{porous_km} km against {solid_km} km");
}

#[test]
fn an_icy_target_widens_a_crater_by_its_density() {
    // For one projectile, non-porous ice of 940 kg m⁻³ makes a crater wider than rock's of 2,550
    // by (3,000 ÷ 940 against 3,000 ÷ 2,550)^(2 ÷ (3 × 2.55)), the cavity's density ratio to the
    // power 0.261 (science-r09-craters.md item 2), to 10⁻⁹; below both bodies' D_t and the
    // Moon's, where neither collapses.
    let rock = params(0.01, 1.428, Screening::None, 1.0, Some(20_000.0));
    let ice = struck(&rock, 3_000.0, TargetClass::NonPorous, 940.0);
    let expected = math::powf((3_000.0 / 940.0) / (3_000.0 / 2_550.0), 2.0 / (3.0 * 2.55));
    assert!((expected - 1.298).abs() < 1e-3, "{expected}");
    for lunar_km in [0.01, 0.3, 2.0] {
        let wider = DiameterMap::of(&ice).unwrap().body(lunar_km)
            / DiameterMap::of(&rock).unwrap().body(lunar_km);
        assert!(
            (wider / expected - 1.0).abs() < 1e-9,
            "{wider} at {lunar_km} km"
        );
    }
}

#[test]
fn saturation_is_trasks() {
    assert!((saturation(km(1.0)).value() - 0.079).abs() < 1e-15);
    assert!((saturation(km(2.0)).value() - 0.079 / 4.0).abs() < 1e-15);
    assert!((saturation(Metres::new(10.0)).value() - 790.0).abs() < 1e-9);
    // The reference Moon's production (its surface 4.4 Gyr old, R09.T10) exceeds saturation at
    // 100 km, as Design note 10 has it: about 2,500 craters above 100 km against the cap's 300.
    let area = 4.0 * core::f64::consts::PI * 1_737.4 * 1_737.4;
    let n = lunar_chronology(Gigayears::new(4.4), SOLAR_SYSTEM_AGE).value();
    let production = production_density(&moon_at(n), km(100.0)).value() * area;
    let cap = saturation(km(100.0)).value() * area;
    assert!((2_000.0..3_000.0).contains(&production), "{production}");
    assert!((250.0..350.0).contains(&cap), "{cap}");
}

#[test]
fn octaves_and_uniforms_refuse_values_out_of_range() {
    assert_eq!(
        DiameterOctave::new(Metres::ZERO),
        Err(BuildDiameterOctaveError(0.0))
    );
    assert_eq!(
        DiameterOctave::new(Metres::new(f64::MAX)),
        Err(BuildDiameterOctaveError(f64::MAX))
    );
    let octave = DiameterOctave::new(Metres::new(3.0)).unwrap();
    assert_eq!(octave.lower(), Metres::new(3.0));
    assert_eq!(octave.upper(), Metres::new(6.0));
    assert_eq!(UnitUniform::new(1.0), Err(BuildUnitUniformError(1.0)));
    assert_eq!(UnitUniform::new(-0.1), Err(BuildUnitUniformError(-0.1)));
    assert!(matches!(UnitUniform::new(f64::NAN), Err(e) if e.value().is_nan()));
    assert_same_bits(UnitUniform::new(0.25).unwrap().get(), 0.25);
}

/// The share of `octave`'s craters below `d` by the density itself.
fn octave_cdf(p: &CraterParams, octave: DiameterOctave, d: Metres) -> f64 {
    let lo = cumulative_density(p, octave.lower()).value();
    let hi = cumulative_density(p, octave.upper()).value();
    let at = cumulative_density(p, d).value();
    ((lo - at) / (lo - hi)).clamp(0.0, 1.0)
}

/// Draws `n` diameters in `octave` from uniforms of the testkit's generator seeded `seed`, checks
/// each lands in its octave, and tests their distribution against the density's by
/// Kolmogorov–Smirnov.
fn assert_octave_draws_follow_the_density(
    name: &str,
    p: &CraterParams,
    octave: DiameterOctave,
    count: usize,
    seed: u64,
) {
    let mut rng = Lcg::new(seed);
    let mut draws: Vec<f64> = (0..count)
        .map(|_| {
            let share = UnitUniform::new(rng.next_f64()).unwrap();
            let diameter = diameter_in_octave(p, octave, share);
            assert!(
                diameter >= octave.lower() && diameter < octave.upper(),
                "{name}: {} m is outside its octave",
                diameter.value()
            );
            diameter.value()
        })
        .collect();
    let ks = ks_one_sample(&mut draws, |d| octave_cdf(p, octave, Metres::new(d)));
    assert_p_value(name, ks.p_value, ALPHA);
}

#[test]
fn the_inversion_lands_in_its_octave_and_follows_the_density() {
    // 10⁵ draws on the unscreened Moon at 1–2 km and on mapped, screened Mars at 2–4 m, where its
    // 1.7 m cutoff tapers the octave; 2 × 10⁴ on the browser target, still four times the brief
    // Venus test's (`crate::test_sizes`).
    let draws = per_target(100_000, 20_000);
    let one_km = DiameterOctave::new(km(1.0)).unwrap();
    assert_octave_draws_follow_the_density("moon", &moon(), one_km, draws, 7);
    let small = DiameterOctave::new(Metres::new(2.0)).unwrap();
    assert_octave_draws_follow_the_density("mars", &mars(), small, draws, 11);
    // The ends: u = 0 gives the lower diameter, and the largest u stays below the upper.
    for (_, p) in every_body() {
        let d = diameter_in_octave(&p, small, UnitUniform::new(0.0).unwrap());
        assert!((d.value() / 2.0 - 1.0).abs() < 1e-11, "{}", d.value());
        let top = UnitUniform::new(1.0_f64.next_down()).unwrap();
        let d = diameter_in_octave(&p, small, top);
        assert!(d < small.upper() && d.value() > 3.999_999, "{}", d.value());
    }
}

#[test]
#[ignore = "slow: 10^5 inversions under Venus's screening, each tested against its full quadrature"]
fn the_inversion_follows_the_screened_density_on_venus() {
    // Venus at 8–16 km, where the taper passes 23% of the production at 8 km and 95% at 20 km.
    let octave = DiameterOctave::new(km(8.0)).unwrap();
    assert_octave_draws_follow_the_density("venus", &venus(), octave, 100_000, 13);
}

#[test]
fn the_inversion_follows_the_screened_density_on_venus_in_brief() {
    let octave = DiameterOctave::new(km(8.0)).unwrap();
    assert_octave_draws_follow_the_density("venus", &venus(), octave, 5_000, 17);
}

#[test]
fn the_density_is_the_same_twice() {
    for (name, p) in every_body() {
        for d in [Metres::new(0.5), km(1.0), km(37.0)] {
            assert_same_bits(
                cumulative_density(&p, d).value(),
                cumulative_density(&p, d).value(),
            );
            let u = UnitUniform::new(0.37).unwrap();
            let octave = DiameterOctave::new(d).unwrap();
            let (a, b) = (
                diameter_in_octave(&p, octave, u),
                diameter_in_octave(&p, octave, u),
            );
            assert_same_bits(a.value(), b.value());
            assert!(a.value().is_finite(), "{name}");
        }
    }
}
