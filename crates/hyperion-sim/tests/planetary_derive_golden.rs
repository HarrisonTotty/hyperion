//! Golden values of plan 14's derivation of radius, composition, irradiation and habitable zone
//! (P14.T11.a–c, T12): Chen and Kipping's radii, Zeng et al.'s curves, the envelope model, the
//! composition solve, fluxes, equilibrium temperatures and habitable zones.
//!
//! Nothing generated reads these yet; they pin the arithmetic, so that a reordered sum, a changed
//! table entry or a moved interpolation changes a line here, which is a generator-version change
//! once P14.T16 calls them. CI checks the same file on 64-bit Arm and on wasm32.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::planetary::derive::composition::{SnowLineSide, composition};
use hyperion_sim::planetary::derive::envelope::radius_with_envelope;
use hyperion_sim::planetary::derive::habitable_zone::{
    HabitableLimit, habitable_zone, habitable_zone_of,
};
use hyperion_sim::planetary::derive::irradiation::{
    BondAlbedo, HostLight, Illumination, equilibrium_temperature, total_flux,
};
use hyperion_sim::planetary::derive::radius::{CoreComposition, radius_chen_kipping, radius_zeng};
use hyperion_sim::stellar::draws::UnitUniform;
use hyperion_sim::units::consts::METRES_PER_AU;
use hyperion_sim::units::{
    EarthFluxes, EarthMasses, EarthRadii, Gigayears, Kelvin, Metres, SolarLuminosities, SolarRadii,
};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

fn host(l: f64, t: f64, r: f64) -> HostLight {
    HostLight::new(
        SolarLuminosities::new(l),
        Kelvin::new(t),
        SolarRadii::new(r),
    )
    .unwrap()
}

#[test]
fn radius_and_composition_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for m in [
        0.01, 0.5, 1.0, 2.04, 5.0, 50.0, 131.58, 317.83, 5_000.0, 50_000.0,
    ] {
        for q in [0.05, 0.5, 0.95] {
            let r = radius_chen_kipping(EarthMasses::new(m), UnitUniform::new(q).unwrap());
            w.f64(&format!("chen_kipping_{m}_{q}"), r.value());
        }
    }
    let curves = [
        ("iron", CoreComposition::IRON),
        ("earth_like", CoreComposition::EARTH_LIKE),
        ("rock", CoreComposition::ROCK),
        ("half_water", CoreComposition::HALF_WATER),
        ("water", CoreComposition::WATER),
    ];
    for (name, core) in curves {
        for m in [0.0553, 0.5, 1.0, 5.0, 50.0] {
            let r = radius_zeng(EarthMasses::new(m), core);
            w.f64(&format!("zeng_{name}_{m}"), r.value());
        }
    }
    let icy = CoreComposition::new(0.325, 0.5).unwrap();
    for (m, core, f, s, t) in [
        (5.0, CoreComposition::EARTH_LIKE, 5e-5, 100.0, 5.0),
        (5.0, CoreComposition::EARTH_LIKE, 0.02, 100.0, 5.0),
        (5.0, CoreComposition::EARTH_LIKE, 0.02, 100.0, 0.05),
        (12.0, icy, 0.1, 0.5, 2.0),
        (40.0, icy, 0.5, 3.0, 5.0),
        (95.0, icy, 0.8, 0.011, 4.5),
    ] {
        let r = radius_with_envelope(
            EarthMasses::new(m),
            core,
            f,
            EarthFluxes::new(s),
            Gigayears::new(t),
        );
        w.f64(&format!("envelope_{m}_{f}_{s}_{t}"), r.value());
    }
    for (m, r, side, s) in [
        (0.0553, 0.383, SnowLineSide::Inside, 6.7),
        (1.0, 0.5, SnowLineSide::Inside, 1.0),
        (1.0, 1.2, SnowLineSide::Inside, 1.0),
        (1.0, 1.2, SnowLineSide::Beyond, 0.1),
        (3.0, 1.9, SnowLineSide::Inside, 50.0),
        (5.0, 2.3, SnowLineSide::Beyond, 0.5),
        (17.15, 3.865, SnowLineSide::Beyond, 0.0011),
        (95.16, 9.14, SnowLineSide::Beyond, 0.011),
        (100.0, 20.0, SnowLineSide::Inside, 1.0),
    ] {
        let solved = composition(
            EarthMasses::new(m),
            EarthRadii::new(r),
            side,
            EarthFluxes::new(s),
        )
        .unwrap();
        let f = solved.fractions();
        let label = format!("composition_{m}_{r}_{side:?}_{s}");
        w.f64(&format!("{label}_iron"), f.iron());
        w.f64(&format!("{label}_rock"), f.rock());
        w.f64(&format!("{label}_water"), f.water());
        w.f64(&format!("{label}_envelope"), f.envelope());
        w.f64(&format!("{label}_radius"), solved.radius().value());
        w.line(&format!("{label}_adjustment = {:?}", solved.adjustment()));
    }
    golden!("planetary/derive_radius", w.as_str());
}

#[test]
fn irradiation_and_habitable_zones_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let sun = host(1.0, 5_772.0, 1.0);
    let au = |x: f64| Metres::new(x * METRES_PER_AU);
    for (name, a, e, albedo) in [
        ("earth", 1.000_000_11, 0.016_7, 0.294),
        ("venus", 0.723_331_99, 0.006_8, 0.76),
        ("mars", 1.523_662_31, 0.093_5, 0.250),
        ("jupiter", 5.203_363_01, 0.048_7, 0.343),
    ] {
        let flux = total_flux(&[Illumination::new(sun, au(a), e).unwrap()]);
        w.f64(&format!("flux_{name}"), flux.value());
        let t = equilibrium_temperature(flux, BondAlbedo::new(albedo).unwrap());
        w.f64(&format!("t_eq_{name}"), t.value());
    }
    let pair = [
        Illumination::new(sun, au(2.0), 0.1).unwrap(),
        Illumination::new(host(0.3, 4_500.0, 0.8), au(2.0), 0.1).unwrap(),
    ];
    let t = equilibrium_temperature(total_flux(&pair), BondAlbedo::BEFORE_ATMOSPHERES);
    w.f64("t_eq_circumbinary", t.value());
    for (name, l, t_eff) in [
        ("sun", 1.0, 5_772.0),
        ("m_dwarf", 0.02, 3_500.0),
        ("f_star", 3.0, 7_000.0),
        ("cool", 1e-3, 2_300.0),
        ("giant_tip", 2_751.621_903, 3_170.7),
    ] {
        let zone = habitable_zone(SolarLuminosities::new(l), Kelvin::new(t_eff));
        for limit in HabitableLimit::ALL {
            w.f64(&format!("zone_{name}_{limit:?}"), zone.limit(limit).value());
        }
        w.line(&format!(
            "zone_{name}_extrapolated = {}",
            zone.extrapolated()
        ));
    }
    let companion = Illumination::new(host(0.5, 5_000.0, 0.9), au(20.0), 0.4).unwrap();
    let pushed = habitable_zone_of(&[sun], &[companion]);
    let circumbinary = habitable_zone_of(&[sun, host(0.5, 5_000.0, 0.9)], &[]);
    for limit in HabitableLimit::ALL {
        w.f64(
            &format!("zone_with_companion_{limit:?}"),
            pushed.limit(limit).value(),
        );
        w.f64(
            &format!("zone_circumbinary_{limit:?}"),
            circumbinary.limit(limit).value(),
        );
    }
    golden!("planetary/derive_irradiation", w.as_str());
}
