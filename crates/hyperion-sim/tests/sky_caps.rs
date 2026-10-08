//! The layer caps against a finer count (rendering plan R06, R06.T7 and R06.T7.b; decided
//! 2026-10-03, `decision-r06-t7-caps.md`, and 2026-10-05, `decision-r06-census-cost-signoff.md`,
//! question 3). Slow.
//!
//! At six points across the galaxy, every layer's expected count of stars brighter than the cut
//! beyond its caps, one radius a ray, recounted with 3,072 rays and twice the radial steps, is under
//! 1.5: the caps' own claim (under 1 at their own resolution) holds against an independent finer
//! count, towards whose rays the caps' radii are read as the census reads them, each through its
//! own profile. It holds for the caps at the uniform cuts of the eye near the Sun (7.95), of the
//! eye 2,000 ly above it (8.54) and of a camera at 60° (10.06), and for the caps by the eye's
//! visibility at each point's own eye's cut. If it fails, the neighbour widening grows; the gate is
//! not loosened.

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::caps::{
    CAPPED_LAYERS, CapResolution, LayerCap, RADIAL_STEPS_PER_DECADE, expected_beyond_caps,
    expected_beyond_caps_by_visibility, layer_caps, layer_caps_by_visibility,
};
use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyContext};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::limits::{eye_cut, eye_visibility};
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

/// The median of a cap's rays' radii, ly.
fn median_ray(cap: &LayerCap) -> f64 {
    let mut radii = cap.rays().expect("one radius a ray").radii_ly().to_vec();
    radii.sort_by(f64::total_cmp);
    radii[radii.len() / 2]
}

/// Prints each layer's caps and recount at `point`, and adds each failure of the gate to
/// `failures`.
fn check(
    point: [f64; 3],
    what: &str,
    caps: &[LayerCap],
    beyond: &[f64],
    failures: &mut Vec<String>,
) {
    for ((layer, cap), fine_beyond) in CAPPED_LAYERS.iter().zip(caps).zip(beyond) {
        eprintln!(
            "{point:?} {what} {layer:?}: rays median {:.0} ly, largest {:.0} (rule {:.0}), beyond \
             {:.3}, at 3,072 rays {fine_beyond:.3}",
            median_ray(cap),
            cap.radius().value(),
            cap.rule_bound().value(),
            cap.expected_beyond()
        );
        if *fine_beyond >= 1.5 {
            failures.push(format!("{point:?} {what} {layer:?}: {fine_beyond}"));
        }
    }
}

#[test]
#[ignore = "slow: builds the luminosity tables and counts 3,072 rays at six points, four times"]
fn caps_converge_in_rays() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let tables = LuminosityTables::build(&galaxy);
    let envelope = BrightnessEnvelope::build(&galaxy);
    let offsets = CellOffsets::build(&galaxy);
    let fine = CapResolution::new(3_072, 2 * RADIAL_STEPS_PER_DECADE).expect("non-zero");
    let eye = EyeObserver::default();
    let mut failures = Vec::new();
    for point in POINTS {
        let observer = Observer::new(
            GalacticPosition::from_light_years(point).expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("an observer");
        let mut cache = NoiseCache::with_capacity(1 << 16);
        for cut in [7.95, 8.54, 10.06].map(Magnitudes::new) {
            let caps = layer_caps(&galaxy, &tables, &envelope, &observer, cut, &mut cache);
            let beyond = expected_beyond_caps(
                &galaxy, &tables, &envelope, &observer, cut, fine, &caps, &mut cache,
            );
            let what = format!("at V {:.2}", cut.value());
            check(point, &what, &caps, &beyond, &mut failures);
        }
        // The caps by the eye's visibility, at the point's own eye's cut.
        let mut ctx = SkyContext {
            tables: &tables,
            envelope: &envelope,
            offsets: &offsets,
            noise: NoiseCache::with_capacity(1 << 16),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let own = eye_cut(&galaxy, &mut ctx, &observer, &eye, None);
        let visibility = eye_visibility(&galaxy, &mut ctx, &observer, &eye, own, None);
        let seen = layer_caps_by_visibility(
            &galaxy,
            &tables,
            &envelope,
            &observer,
            &visibility,
            &mut cache,
        );
        let beyond = expected_beyond_caps_by_visibility(
            &galaxy,
            &tables,
            &envelope,
            &observer,
            &visibility,
            fine,
            &seen,
            &mut cache,
        );
        check(
            point,
            &format!("by the eye's visibility at V {:.3}", own.value()),
            &seen,
            &beyond,
            &mut failures,
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
