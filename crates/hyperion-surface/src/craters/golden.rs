//! The golden file `tests/golden/craters/density_and_shapes.golden`: the crater physics the server
//! and the client both run, to the bit (plan R09, T7.a).
//!
//! The coarse pass (T12.d) places craters natively, and the client's synthesis (T7.b, T7.c) draws
//! them in WebAssembly through the same functions, so a split between targets would draw craters
//! the server does not collide with. The golden pins, as `f64` bits, the density, the production,
//! the inversion within an octave, the chronology and its inverse, and every shape's dimensions
//! and relief, on bodies screened in every way and in every morphological zone. They are
//! generated output: a change to them fails here, and is a generator-version change.

use hyperion_base::units::{
    Gigayears, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecond,
    MetresPerSecondSquared, PerSquareKilometre,
};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::{
    CraterParams, CraterParamsParts, DiameterOctave, SOLAR_SYSTEM_AGE, Screening, UnitUniform,
    chronology_age, crater_cutoff, crater_shape, cumulative_density, diameter_in_octave,
    lunar_chronology, production_density, saturation, transition_diameter,
};

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
        projectile_density: KilogramsPerCubicMetre::new(3_000.0),
    }
}

/// The bodies the golden pins: airless, under thin and thick air, under a given cutoff, icy and
/// mapped.
fn bodies() -> [(&'static str, CraterParams); 6] {
    [
        ("moon", params(0.018, 1.62, Screening::None, 1.0, None)),
        ("venus", params(3.07e-4, 8.87, air(9.2e6, 8.87), 1.0, None)),
        (
            "mars",
            params(2.0e-3, 3.73, air(636.0, 3.73), 1.0, Some(10_600.0)),
        ),
        (
            "earth",
            params(1.0e-3, 9.82, air(101_325.0, 9.82), 1.0, Some(19_900.0)),
        ),
        (
            "icy",
            params(0.05, 1.428, Screening::None, 0.12, Some(20_000.0)),
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

#[test]
fn crater_physics_matches_its_golden() {
    let mut w = GoldenWriter::new();
    w.header(crate::generator_version());
    let diameters_m = [
        1e-3, 0.05, 1.7, 6.0, 9.999, 10.0, 10.001, 99.0, 250.0, 1e3, 3e3, 8.3e3, 10.4e3, 19e3,
        50e3, 299.9e3, 300e3, 300.1e3, 1e6, 1e7,
    ];
    let shares = [0.0, 0.37, 1.0_f64.next_down()];
    for (name, p) in bodies() {
        w.f64(
            &format!("{name} transition"),
            transition_diameter(&p).value(),
        );
        if let Some(cutoff) = crater_cutoff(&p) {
            w.f64(&format!("{name} cutoff"), cutoff.value());
            for scale in [0.999_999, 1.0, 1.000_001] {
                let d = cutoff * (1e3 * scale);
                w.f64(
                    &format!("{name} N(>{scale} of 1000 cutoffs)"),
                    cumulative_density(&p, d).value(),
                );
            }
        }
        for d in diameters_m {
            let m = Metres::new(d);
            w.f64(
                &format!("{name} N(>{d:e} m)"),
                cumulative_density(&p, m).value(),
            );
            w.f64(
                &format!("{name} P(>{d:e} m)"),
                production_density(&p, m).value(),
            );
        }
        for lower in [2.0, 1e3, 8e3] {
            let octave = DiameterOctave::new(Metres::new(lower)).unwrap();
            for u in shares {
                let d = diameter_in_octave(&p, octave, UnitUniform::new(u).unwrap());
                w.f64(&format!("{name} octave {lower:e} m at {u:e}"), d.value());
            }
        }
        for d in [40.0, 141.0, 1e3, 0.9 * 19e3, 30e3, 200e3, 400e3, 2e6] {
            let shape = crater_shape(&p, Metres::new(d));
            let label = format!("{name} shape {d:e} m");
            w.line(&format!("{label} {:?}", shape.morphology()));
            w.f64(&format!("{label} depth"), shape.depth().value());
            w.f64(&format!("{label} rim"), shape.rim_height().value());
            w.f64(&format!("{label} floor"), shape.floor_radius().value());
            w.f64(&format!("{label} peak"), shape.peak_relief().value());
            w.f64(&format!("{label} peak radius"), shape.peak_radius().value());
            w.f64(&format!("{label} a"), shape.exterior_width());
            for r in [0.0, 0.1, 0.5, 0.95, 1.0, 1.3, 2.0, 2.6] {
                w.f64(&format!("{label} relief at {r}"), shape.relief(r).value());
            }
        }
    }
    for d in [1.0, 1e3, 1e5] {
        w.f64(
            &format!("saturation {d:e} m"),
            saturation(Metres::new(d)).value(),
        );
    }
    for (s, a) in [
        (0.1, 4.567),
        (3.9, 4.567),
        (4.4, 4.567),
        (1.0, 1.0),
        (2.0, 10.0),
        (10.0, 10.0),
    ] {
        let (s, a) = (Gigayears::new(s), Gigayears::new(a));
        let n = lunar_chronology(s, a);
        w.f64(&format!("chronology {s:?} of {a:?}"), n.value());
        w.f64(
            &format!("chronology age of {n:?} in {a:?}"),
            chronology_age(n, a).value(),
        );
    }
    let a = SOLAR_SYSTEM_AGE;
    w.f64(
        "chronology age of 0.5 per km2",
        chronology_age(PerSquareKilometre::new(0.5), a).value(),
    );
    golden!("craters/density_and_shapes", w.as_str());
}
