//! Tests of the crater density: the parameter, the polynomial's ends, monotonicity, screening
//! against Design note 12's figures and Venus's catalogue, the diameter map against Ivanov 2001's
//! Mars, and the inversion within an octave.

use super::*;
use crate::craters::{CraterParamsParts, SOLAR_SYSTEM_AGE, Screening, lunar_chronology};
use hyperion_base::units::{
    Gigayears, KilogramsPerCubicMetre, KilogramsPerSquareMetre, MetresPerSecond,
    MetresPerSecondSquared,
};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// A stony projectile's density, kg m⁻³: Design note 12's 3,000.
const PROJECTILE_DENSITY: f64 = 3_000.0;

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

/// A contract of N(>1 km) `n`, gravity `g`, the screening `screening`, target factor `k` and
/// impact velocity `v` (m/s).
fn params(n: f64, g: f64, screening: Screening, k: f64, v: Option<f64>) -> CraterParams {
    CraterParams::new(CraterParamsParts {
        n_1km: PerSquareKilometre::new(n),
        screening,
        gravity: MetresPerSecondSquared::new(g),
        k_target: k,
        impact_velocity: v.map(MetresPerSecond::new),
    })
    .unwrap()
}

/// An atmosphere of surface pressure `pressure` (Pa) at gravity `g`, over stony projectiles.
fn air(pressure: f64, g: f64) -> Screening {
    Screening::Atmosphere {
        column_mass: KilogramsPerSquareMetre::new(pressure / g),
        projectile_density: KilogramsPerCubicMetre::new(PROJECTILE_DENSITY),
    }
}

/// The Moon, unscreened, at saturation's N(>1 km).
fn moon() -> CraterParams {
    params(0.018, 1.62, Screening::None, 1.0, None)
}

/// Venus at Design note 12's fitted N(>1 km), 3.07 × 10⁻⁴ km⁻², under 92 bar (NASA's Venus fact
/// sheet), with no diameter map, as the fit had none.
fn venus() -> CraterParams {
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
/// 20 km/s, through the map.
fn icy() -> CraterParams {
    params(0.05, 1.428, Screening::None, 0.12, Some(20_000.0))
}

/// Every contract the tests sweep.
fn every_body() -> [(&'static str, CraterParams); 6] {
    [
        ("moon", moon()),
        ("venus", venus()),
        ("mars", mars()),
        ("earth", earth()),
        ("icy", icy()),
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
fn n_1km_is_the_parameter_exactly() {
    for (name, p) in every_body() {
        let n = p.n_1km().value();
        assert_same_bits(production_density(&p, km(1.0)).value(), n);
        // An unscreened body's density at 1 km is the published N(>1 km) (P14.T48.c's test).
        if crater_cutoff(&p).is_none() {
            assert_same_bits(cumulative_density(&p, km(1.0)).value(), n);
        }
        assert!(cumulative_density(&p, km(1.0)).value() <= n, "{name}");
    }
}

#[test]
fn the_polynomial_holds_neukums_coefficients_with_a0_as_the_parameter() {
    // Craterstats' a0, −3.0768, is log₁₀ of the chronology's N(>1 km) at 1 Gyr, 8.38 × 10⁻⁴: the
    // full published polynomial at a0 is the lunar shape times that density.
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
    // 0.4% of the production at 3 km, 4% at 5 km, 23% at 8 km, 46% at 10 km and 95% at 20 km.
    let ln_cutoff = math::ln(crater_cutoff(&venus()).unwrap().value() / 1e3);
    for (d_km, share) in [
        (3.0, 0.004),
        (5.0, 0.04),
        (8.0, 0.23),
        (10.0, 0.46),
        (20.0, 0.95),
    ] {
        let f = taper(math::ln(d_km), ln_cutoff);
        assert!(
            (f - share).abs() < 0.5 * share.min(0.1),
            "{d_km} km: {f} against {share}"
        );
    }
}

#[test]
fn venus_screened_counts_match_the_gazetteer() {
    // The IAU Gazetteer's named Venusian craters at or above 3, 5, 10, 20 and 40 km (Design note
    // 12): 878, 850, 642, 335 and 104.
    let area = 4.0 * core::f64::consts::PI * VENUS_RADIUS_KM * VENUS_RADIUS_KM;
    let p = venus();
    for (d_km, named) in [
        (3.0, 878.0),
        (5.0, 850.0),
        (10.0, 642.0),
        (20.0, 335.0),
        (40.0, 104.0),
    ] {
        let count = cumulative_density(&p, km(d_km)).value() * area;
        assert!(
            (count / named - 1.0).abs() < 0.10,
            "{d_km} km: {count:.0} craters against the Gazetteer's {named}"
        );
    }
}

#[test]
fn venus_screened_density_at_1_km_is_far_below_its_production() {
    let p = venus();
    let screened = cumulative_density(&p, km(1.0)).value();
    let production = production_density(&p, km(1.0)).value();
    assert!(
        screened < 0.01 * production,
        "{screened} against {production}"
    );
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
    let mut worst: f64 = 1.0;
    for k in 0..=40 {
        let d_km = math::exp10(f64::from(k) / 20.0);
        let ratio = production_density(p, km(d_km)).value() / ivanov_shape(d_km);
        worst = worst.max(ratio.max(1.0 / ratio));
    }
    worst
}

#[test]
fn the_map_at_mars_ratios_reproduces_ivanov_2001() {
    // Mars's gravity 3.71 m s⁻² (D_t 8.3 km), unscreened, at Ivanov 2001's velocity ratio,
    // U = 16.7 × 9.6 ÷ 16.1 km/s, and at P14.T48.f's 10.6 km/s: each within ×1.5 in N over
    // 1–100 km, both normalised at 1 km (science-r09-craters.md item 4 computes 0.97–1.03 and
    // 0.92–1.03). The lunar shape alone is not.
    let ivanov_ratio = LUNAR_IMPACT_VELOCITY.value() * 9.6 / 16.1;
    let at_ratio =
        worst_against_ivanov(&params(1.0, 3.71, Screening::None, 1.0, Some(ivanov_ratio)));
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
    // What the build gives, pinned so that a change is seen: within 3.3% and 9.2%.
    assert!(at_ratio < 1.04, "×{at_ratio}");
    assert!(modelled < 1.10, "×{modelled}");
}

#[test]
fn the_map_is_the_identity_at_unit_ratios_and_monotone() {
    let unit = params(
        0.02,
        1.62,
        Screening::None,
        1.0,
        Some(LUNAR_IMPACT_VELOCITY.value()),
    );
    let map = DiameterMap::of(&unit).unwrap();
    for (name, p) in every_body() {
        let Some(map_p) = DiameterMap::of(&p) else {
            continue;
        };
        let mut previous = 0.0;
        for k in 0..=800 {
            let d_km = math::exp10(-4.0 + f64::from(k) / 100.0);
            // The identity at unit ratios.
            let lunar = map.lunar(d_km);
            assert!(
                ((lunar - d_km) / d_km).abs() < 1e-12,
                "{d_km} km maps to {lunar}"
            );
            // Strictly increasing, and inverted by `body`.
            let image = map_p.lunar(d_km);
            assert!(image > previous, "{name} at {d_km} km");
            previous = image;
            let back = map_p.body(image);
            assert!(
                ((back - d_km) / d_km).abs() < 1e-12,
                "{name}: {d_km} km returns as {back}"
            );
        }
    }
    // Unit ratios give the lunar production itself.
    for d_km in [0.003, 0.5, 7.0, 40.0, 900.0] {
        let a = production_density(&unit, km(d_km)).value();
        let b = production_density(&moon_at(0.02), km(d_km)).value();
        assert!(((a - b) / b).abs() < 1e-11, "{d_km} km");
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
    // 1.7 m cutoff tapers the octave.
    let one_km = DiameterOctave::new(km(1.0)).unwrap();
    assert_octave_draws_follow_the_density("moon", &moon(), one_km, 100_000, 7);
    let small = DiameterOctave::new(Metres::new(2.0)).unwrap();
    assert_octave_draws_follow_the_density("mars", &mars(), small, 100_000, 11);
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
