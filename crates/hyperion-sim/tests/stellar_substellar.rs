//! Golden values of the cooling fits below 0.1 M☉ (plan 06, P06.T13).
//!
//! The fits are closed forms whose arithmetic is output, and plan 13 builds its giant-planet fit
//! against them (its P13.T5.c requires plan 06's goldens unchanged), so their values are pinned on a
//! grid of mass, age and metallicity that crosses the hydrogen-burning limit, the 1 Myr hold and
//! the join with the backbone at 0.1 M☉.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::substellar::{cooling, hydrogen_burning_limit};
use hyperion_sim::units::{Dex, HeliumExcess, SolarMasses, Years};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

#[test]
fn substellar_cooling_is_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for fe_h in [-2.3, -1.0, 0.0, 0.18] {
        let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        w.f64(
            &format!("fe_h={fe_h}.hydrogen_burning_limit"),
            hydrogen_burning_limit(&comp).value(),
        );
        for m in [0.01, 0.0124, 0.02, 0.05, 0.07, 0.075, 0.08, 0.09, 0.1] {
            for age in [0.0, 5.0e5, 1.0e7, 1.0e8, 1.0e9, 4.6e9, 1.38e10] {
                let s =
                    cooling(SolarMasses::new(m), Years::new(age), &comp).expect("inside the fits");
                let label = format!("fe_h={fe_h}.m={m}.t={age:e}");
                w.f64(
                    &format!("{label}.log_l"),
                    math::log10(s.luminosity().value()),
                );
                w.f64(&format!("{label}.r"), s.radius().value());
                w.f64(&format!("{label}.teff"), s.effective_temperature().value());
            }
        }
    }
    golden!("stellar/substellar_cooling", w.as_str());
}
