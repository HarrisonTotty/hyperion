//! The layer caps against a finer count (rendering plan R06, R06.T7; decided 2026-10-03,
//! `decision-r06-t7-caps.md`). Slow.
//!
//! At six points across the galaxy and the eye's cut near the Sun (7.95), every layer's expected
//! count of stars brighter than the cut beyond its cap, recounted with 3,072 rays and twice the
//! radial steps, is under 1.5: the caps' own claim (under 1 at their own resolution) holds against
//! an independent finer count. If it fails, the caps' ray count is raised; the gate is not loosened.

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::caps::{
    CAPPED_LAYERS, CapResolution, RADIAL_STEPS_PER_DECADE, expected_beyond_caps, layer_caps,
};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Magnitudes;

/// The points: near the Sun, the nuclear disc, the solar circle a quarter turn round and on the
/// far side, the inner disc, and 2,000 ly above the Sun.
const POINTS: [[f64; 3]; 6] = [
    [0.0, 26_000.0, 68.0],
    [0.0, 150.0, 0.0],
    [26_000.0, 0.0, 68.0],
    [-18_385.0, -18_385.0, 68.0],
    [0.0, 8_000.0, 0.0],
    [0.0, 26_000.0, 2_000.0],
];

#[test]
#[ignore = "slow: builds the luminosity tables and counts 3,072 rays at six points"]
fn caps_converge_in_rays() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let tables = LuminosityTables::build(&galaxy);
    let envelope = BrightnessEnvelope::build(&galaxy);
    let cut = Magnitudes::new(7.95);
    let fine = CapResolution::new(3_072, 2 * RADIAL_STEPS_PER_DECADE).expect("non-zero");
    let mut failures = Vec::new();
    for point in POINTS {
        let observer = Observer::new(
            GalacticPosition::from_light_years(point).expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("an observer");
        let mut cache = NoiseCache::with_capacity(1 << 16);
        let caps = layer_caps(&galaxy, &tables, &envelope, &observer, cut, &mut cache);
        let beyond = expected_beyond_caps(
            &galaxy, &tables, &envelope, &observer, cut, fine, &caps, &mut cache,
        );
        for ((layer, cap), fine_beyond) in CAPPED_LAYERS.iter().zip(&caps).zip(&beyond) {
            eprintln!(
                "{point:?} {layer:?}: cap {:.0} ly (rule {:.0}), beyond {:.3}, at 3,072 rays \
                 {fine_beyond:.3}",
                cap.radius().value(),
                cap.rule_bound().value(),
                cap.expected_beyond()
            );
            if *fine_beyond >= 1.5 {
                failures.push(format!("{point:?} {layer:?}: {fine_beyond}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
