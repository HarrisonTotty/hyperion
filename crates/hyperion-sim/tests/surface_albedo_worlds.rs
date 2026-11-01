//! The class photometry on the sample worlds (rendering plan R10, T10.a): on each airless or
//! thin-aired world the area-weighted geometric albedo of its scaled classes is the body's to 1%
//! where the classes' ranges allow it, and where they do not, every material that could move it
//! towards p sits at its range's end, the residual being the solve's stated one; and no scaled
//! class passes its law's ceiling in any band. The reference Earth and the locked Earth are
//! thick-aired under Earth's template, so the server sends no scale for them (`hyperion-server`'s
//! `surface::service` tests the gate itself).
//!
//! The worlds are R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`): the reference
//! Earth, Mars, Moon and Ceres, the icy moon `europa_like` and the locked `locked_earth_like`, each
//! run through the whole coarse pass under `reference::SEED`, behind the sim's `testing` feature.
//! The four `Tier::Fast` worlds are airless or thin-aired, and share one test; the two
//! `Tier::Slow` worlds are thick-aired, which their inputs show without a pass. Each world's p is
//! its state's template's, as the sim's photometry chooses and draws it (P14.T47.b): Mercury's
//! for the airless rock of the Moon and Ceres, the airless ice's for the icy moon and Mars's for
//! the thin-aired Mars, each scaled by the world's Bond albedo against the template's reference
//! (1 on the three whose Bond albedo is their analogue's, decision-p14-phase-j, ruling 9).
#![cfg(feature = "testing")]

use hyperion_base::rng::DetailSeed;
use hyperion_sim::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use hyperion_sim::planetary::derive::photometry::{PhaseTemplate, THIN_ATMOSPHERE_PRESSURE};
use hyperion_sim::planetary::surface::CoarseInputs;
use hyperion_surface::field::FieldView;
use hyperion_surface::material::{
    ALBEDO_SCALE_FLOOR, albedo_scale, class_fractions, properties, scale_range,
    scaled_normal_albedo,
};
use hyperion_surface::synth::{SynthCache, Synthesiser};

#[expect(
    dead_code,
    reason = "the class photometry uses only the sample worlds of the shared helpers"
)]
mod common;

use common::surface_worlds::{SampleWorld, Tier, sample_worlds};

/// The detail seed the sample worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x616c_6265_646f_7773);

/// The relative distance within which a material counts as at its range's end, as the solve
/// counts it.
const AT_END: f64 = 1e-12;

/// The template the sim's photometry chooses for a world of `inputs`' state (P14.T47.b's order),
/// for the states the sample worlds are in.
fn template(inputs: &CoarseInputs) -> PhaseTemplate {
    let thin = inputs.surface_pressure().value() < THIN_ATMOSPHERE_PRESSURE.value();
    match (inputs.surface_state(), inputs.surface_material()) {
        (SurfaceState::Airless, SurfaceMaterial::Rock) => PhaseTemplate::Mercury,
        (SurfaceState::Airless, SurfaceMaterial::Ice) => PhaseTemplate::AirlessIce,
        (SurfaceState::Temperate, _) if thin => PhaseTemplate::Mars,
        (SurfaceState::Temperate, _) => PhaseTemplate::Earth,
        (state, material) => panic!("no sample world is {state:?} on {material:?}"),
    }
}

/// The sample worlds of `tier`, which the test names as it expects them.
fn worlds_of(tier: Tier, expected: &[&str]) -> Vec<&'static SampleWorld> {
    let worlds: Vec<&SampleWorld> = sample_worlds()
        .iter()
        .filter(|world| world.tier() == tier)
        .collect();
    assert_eq!(
        worlds.iter().map(|world| world.name()).collect::<Vec<_>>(),
        expected,
        "the {tier:?} sample worlds"
    );
    worlds
}

/// On the Mars-, Moon-, Ceres- and Europa-like worlds, the scaled classes give the body's p in V to
/// 1%, or the ranges stop the scale: where the scaled classes fall short of p every material not
/// black in V is at its range's top, and where they pass it, every one is at its range's bottom or
/// the scale at its floor. No scaled class passes its ceiling in any band.
#[test]
fn the_scaled_classes_give_each_thin_aired_sample_world_s_p() {
    for world in worlds_of(
        Tier::Fast,
        &["mars_like", "moon_like", "ceres_like", "europa_like"],
    ) {
        let name = world.name();
        let inputs = world.inputs();
        assert!(
            inputs.surface_pressure().value() < THIN_ATMOSPHERE_PRESSURE.value(),
            "{name}'s ground is seen"
        );
        let row = template(&inputs).row();
        assert!(
            row.lunar_lambert_share > 0.0,
            "{name}: a law with a ground term"
        );
        // The body's p as the photometry draws it (P14.T47.b): the template's, scaled by the
        // world's Bond albedo against the template's reference and held under 1 ÷ q.
        let bond = inputs.surface_state().albedo(inputs.surface_material());
        let scale = bond.value() / row.reference_bond().value();
        let q = row.phase_integral;
        let target = [
            (row.albedo.b * scale).min(1.0 / q.b),
            (row.albedo.v * scale).min(1.0 / q.v),
            (row.albedo.r * scale).min(1.0 / q.r),
        ];
        let field = world.field();
        let synth = Synthesiser::new(field, DETAIL);
        let fractions = class_fractions(&synth, &mut SynthCache::new())
            .expect("a coarse field holds every cell");
        let palette = field.header().palette();
        let solved = albedo_scale(&fractions, palette, target).expect("a named palette");
        let c = solved.scale();
        let mut moving = Vec::new();
        for &(ground, _) in fractions.shares() {
            let Some(material) = ground.material() else {
                continue;
            };
            let p = properties(material, palette).expect("the field's own palette");
            let drawn = scaled_normal_albedo(&p, c);
            let ceiling = p.phase_law().normal_albedo_ceiling();
            assert!(
                drawn.iter().all(|&a| a <= ceiling),
                "{name}: {ground:?} at {drawn:?}"
            );
            if p.normal_albedo_bvr()[1] > 0.0 {
                moving.push((ground, c * p.normal_albedo_bvr()[1], scale_range(&p)));
            }
        }
        let residual = solved.residual()[1];
        if solved.relative_residual()[1].abs() >= 0.01 {
            let pinned = if residual < 0.0 {
                moving
                    .iter()
                    .all(|&(_, wanted, range)| wanted >= range.high() * (1.0 - AT_END))
            } else {
                c <= ALBEDO_SCALE_FLOOR
                    || moving
                        .iter()
                        .all(|&(_, wanted, range)| wanted <= range.low() * (1.0 + AT_END))
            };
            assert!(
                pinned,
                "{name}: residual {residual} with a material free to move: {solved:?}, {moving:?}"
            );
        }
    }
}

/// The reference Earth and the locked Earth are thick-aired (98.55 kPa against 30 kPa) under
/// Earth's template, whose law has no ground term: worlds the server sends no albedo scale for
/// (main's rulings, 2026-10-14). Their inputs say so without their slow passes.
#[test]
fn the_thick_aired_sample_worlds_are_under_the_earth_template() {
    for world in worlds_of(Tier::Slow, &["earth_like", "locked_earth_like"]) {
        let inputs = world.inputs();
        assert!(
            inputs.surface_pressure().value() >= THIN_ATMOSPHERE_PRESSURE.value(),
            "{} is thick-aired",
            world.name()
        );
        assert_eq!(template(&inputs), PhaseTemplate::Earth, "{}", world.name());
    }
    assert!(PhaseTemplate::Earth.row().lunar_lambert_share <= 0.0);
}
