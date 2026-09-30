//! Golden values of plan 14's giant planets (P14.T11.d): radii from plan 13's cooling fit,
//! inflated by Thorngren and Fortney's (2018) heating and capped, the blend below 0.414 Jupiter
//! masses, and the giants' heavy elements.
//!
//! Nothing generated reads these yet; they pin the arithmetic, so that a changed table entry or a
//! moved interpolation changes a line here, which is a generator-version change once P14.T16
//! calls them. CI checks the same file on 64-bit Arm and on wasm32.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::planetary::derive::composition::{SnowLineSide, composition, giant_composition};
use hyperion_sim::planetary::derive::radius::{heating_efficiency, radius_giant};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::substellar::giant_cooling;
use hyperion_sim::units::{
    Dex, EarthFluxes, EarthMasses, EarthRadii, HeliumExcess, JupiterMasses, Years,
};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

#[test]
fn giant_radii_and_compositions_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    // Nine giants, (Jupiter masses, Gyr, F⊕, [Fe/H]): Jupiter; HD 209458 b and HD 189733 b
    // (Torres, Winn and Holman 2008); a young hot Jupiter; an ultra-hot one; one at the cap; one
    // in the fade below 1,000 K; a massive hot one; and one inside the blend.
    for (m, gyr, s, fe_h) in [
        (1.0, 4.57, 0.036_93, 0.0),
        (0.685, 3.1, 732.07, 0.0),
        (1.144, 6.8, 344.66, -0.03),
        (1.0, 0.02, 700.0, 0.2),
        (1.47, 3.0, 8_200.0, 0.1),
        (0.42, 5.0, 5_000.0, 0.0),
        (2.0, 8.0, 150.0, -0.5),
        (11.0, 1.0, 2_000.0, 0.3),
        (0.35, 2.0, 600.0, 0.0),
    ] {
        let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let mass = JupiterMasses::new(m);
        let interior = giant_cooling(mass, Years::new(gyr * 1e9), &comp).unwrap();
        let flux = EarthFluxes::new(s);
        let giant = radius_giant(EarthMasses::from(mass), &interior, flux).unwrap();
        let label = format!("giant_{m}_{gyr}_{s}_{fe_h}");
        w.f64(&format!("{label}_radius"), giant.radius().value());
        w.f64(
            &format!("{label}_cooling_radius"),
            giant.cooling_radius().value(),
        );
        w.f64(
            &format!("{label}_inflated_radius"),
            giant.inflated_radius().value(),
        );
        w.f64(
            &format!("{label}_internal_luminosity"),
            giant.internal_luminosity().value(),
        );
        w.f64(
            &format!("{label}_heating_efficiency"),
            heating_efficiency(flux),
        );
        w.f64(&format!("{label}_share"), giant.share());
        w.line(&format!("{label}_inflated = {}", giant.is_inflated()));
    }
    // The blend at 0.35 Jupiter masses, from the composition solve's radius at the reference age.
    let mass = EarthMasses::from(JupiterMasses::new(0.35));
    let flux = EarthFluxes::new(600.0);
    let solved = composition(mass, EarthRadii::new(10.5), SnowLineSide::Beyond, flux).unwrap();
    let comp = Composition::SOLAR;
    let interior = giant_cooling(JupiterMasses::new(0.35), Years::new(5e9), &comp).unwrap();
    let giant = radius_giant(mass, &interior, flux).unwrap();
    w.f64("blend_0.35_radius", giant.blended(solved.radius()).value());
    let blended = giant_composition(mass, SnowLineSide::Beyond)
        .unwrap()
        .blended(solved.fractions());
    for (name, x) in [
        ("iron", blended.iron()),
        ("rock", blended.rock()),
        ("water", blended.water()),
        ("envelope", blended.envelope()),
    ] {
        w.f64(&format!("blend_0.35_{name}"), x);
    }
    for m in [0.3, 1.0, 13.0] {
        for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
            let giant = giant_composition(EarthMasses::from(JupiterMasses::new(m)), side).unwrap();
            let f = giant.fractions();
            let label = format!("composition_{m}_{side:?}");
            w.f64(
                &format!("{label}_heavy_elements"),
                giant.heavy_elements().value(),
            );
            w.f64(&format!("{label}_iron"), f.iron());
            w.f64(&format!("{label}_rock"), f.rock());
            w.f64(&format!("{label}_water"), f.water());
            w.f64(&format!("{label}_envelope"), f.envelope());
        }
    }
    golden!("planetary/derive_giants", w.as_str());
}

#[test]
fn a_giant_is_the_same_twice() {
    let comp = Composition::SOLAR;
    let mass = JupiterMasses::new(0.9);
    let flux = EarthFluxes::new(900.0);
    let run = || {
        let interior = giant_cooling(mass, Years::new(2e9), &comp).unwrap();
        let giant = radius_giant(EarthMasses::from(mass), &interior, flux).unwrap();
        let fractions = giant_composition(EarthMasses::from(mass), SnowLineSide::Beyond)
            .unwrap()
            .fractions();
        (giant, fractions)
    };
    assert_eq!(run(), run());
}
