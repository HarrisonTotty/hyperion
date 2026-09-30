//! Golden values of the giant-planet cooling fit (plan 13, P13.T5.c).
//!
//! The fit interpolates a committed table, scales it by metallicity and blends it into plan 06's
//! cooling fit, and plan 14 reads its radii, so its values are pinned at nine (mass, age) points,
//! each at solar composition and at \[Fe/H\] = −1: the 1 Myr hold and Saturn's mass at the table's
//! extrapolated low-mass end, Jupiter young and at the Solar System's age, two interior points, the
//! blend's start and middle, and the join at 13 Jupiter masses past the table's last age.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::substellar::giant_cooling;
use hyperion_sim::units::{Dex, HeliumExcess, JupiterMasses, Years};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The nine points: (Jupiter masses, years).
const POINTS: [(f64, f64); 9] = [
    (0.3, 5.0e5),
    (0.3, 4.6e9),
    (1.0, 1.0e7),
    (1.0, 4.6e9),
    (3.0, 1.0e8),
    (6.0, 1.38e10),
    (10.0, 1.0e9),
    (11.5, 1.0e8),
    (13.0, 2.0e10),
];

#[test]
fn giant_cooling_is_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for fe_h in [0.0, -1.0] {
        let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        for (m, age) in POINTS {
            let s = giant_cooling(JupiterMasses::new(m), Years::new(age), &comp)
                .expect("inside the fit");
            let label = format!("fe_h={fe_h}.m={m}.t={age:e}");
            w.f64(
                &format!("{label}.log_l"),
                math::log10(s.luminosity().value()),
            );
            w.f64(&format!("{label}.r"), s.radius().value());
            w.f64(&format!("{label}.teff"), s.effective_temperature().value());
        }
    }
    golden!("stellar/giant_cooling", w.as_str());
}
