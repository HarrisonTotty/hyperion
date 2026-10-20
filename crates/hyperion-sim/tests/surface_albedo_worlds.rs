//! The class photometry on the reference worlds (rendering plan R10, T10.a): on each airless or
//! thin-aired world the area-weighted geometric albedo of its scaled classes is the body's to 1%
//! where the classes' ranges allow it, and where they do not, every material that could move it
//! towards p sits at its range's end, the residual being the solve's stated one; and no scaled
//! class passes its law's ceiling in any band. The reference Earth is thick-aired under Earth's
//! template, so the server sends no scale for it (`hyperion-server`'s `surface::service` tests the
//! gate itself).
//!
//! The worlds are R09's reference Earth, Mars, Moon and Ceres (`planetary::surface::reference`),
//! each run through the coarse pass once, under the sim's `testing` feature; until R09.T13.e–T16
//! fill the rest of the pass's steps they come out as the steps built so far leave them
//! (`surface_material_worlds`'s documentation). Each world's p is its state's template's, as the sim's photometry chooses it
//! (P14.T47.b): Mercury's for the airless rock of the Moon and Ceres, Mars's for the thin-aired
//! Mars, each the analogue's own, since each world's Bond albedo is the analogue's reference
//! (decision-p14-phase-j, ruling 9), which the test checks. The plan's cases on R09.T16's
//! `sample_worlds()` follow it under the same task ID.
#![cfg(feature = "testing")]

use hyperion_base::rng::DetailSeed;
use hyperion_sim::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use hyperion_sim::planetary::derive::photometry::{PhaseTemplate, THIN_ATMOSPHERE_PRESSURE};
use hyperion_sim::planetary::surface::{CoarseInputs, coarse_pass, reference};
use hyperion_surface::field::FieldView;
use hyperion_surface::material::{
    ALBEDO_SCALE_FLOOR, albedo_scale, class_fractions, properties, scale_range,
    scaled_normal_albedo,
};
use hyperion_surface::synth::{SynthCache, Synthesiser};

/// The detail seed the reference worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x616c_6265_646f_7773);

/// The relative distance within which a material counts as at its range's end, as the solve
/// counts it.
const AT_END: f64 = 1e-12;

/// The template the sim's photometry chooses for a world of `inputs`' state (P14.T47.b's order),
/// for the states the reference worlds are in.
fn template(inputs: &CoarseInputs) -> PhaseTemplate {
    let thin = inputs.surface_pressure().value() < THIN_ATMOSPHERE_PRESSURE.value();
    match (inputs.surface_state(), inputs.surface_material()) {
        (SurfaceState::Airless, SurfaceMaterial::Rock) => PhaseTemplate::Mercury,
        (SurfaceState::Airless, SurfaceMaterial::Ice) => PhaseTemplate::AirlessIce,
        (SurfaceState::Temperate, _) if thin => PhaseTemplate::Mars,
        (SurfaceState::Temperate, _) => PhaseTemplate::Earth,
        (state, material) => panic!("no reference world is {state:?} on {material:?}"),
    }
}

/// On the Mars-, Moon- and Ceres-like worlds, the scaled classes give the body's p in V to 1%, or
/// the ranges stop the scale: where the scaled classes fall short of p every material not black in
/// V is at its range's top, and where they pass it, every one is at its range's bottom or the
/// scale at its floor. No scaled class passes its ceiling in any band.
#[test]
fn the_scaled_classes_give_each_thin_aired_reference_world_s_p() {
    for (name, inputs) in [
        ("Mars-like", reference::mars_like()),
        ("Moon-like", reference::moon_like()),
        ("Ceres-like", reference::ceres_like()),
    ] {
        assert!(
            inputs.surface_pressure().value() < THIN_ATMOSPHERE_PRESSURE.value(),
            "{name}'s ground is seen"
        );
        let row = template(&inputs).row();
        assert!(
            row.lunar_lambert_share > 0.0,
            "{name}: a law with a ground term"
        );
        let bond = inputs.surface_state().albedo(inputs.surface_material());
        assert!(
            (row.reference_bond().value() - bond.value()).abs() < 1e-15,
            "{name}: the photometry draws it with its analogue's own p"
        );
        let target = [row.albedo.b, row.albedo.v, row.albedo.r];
        let field = coarse_pass(reference::SEED, &inputs);
        let synth = Synthesiser::new(&field, DETAIL);
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

/// The reference Earth is thick-aired (98.55 kPa against 30 kPa) under Earth's template, whose law
/// has no ground term: a world the server sends no albedo scale for (main's rulings, 2026-10-14).
#[test]
fn the_reference_earth_is_thick_aired_under_the_earth_template() {
    let inputs = reference::earth_like();
    assert!(inputs.surface_pressure().value() >= THIN_ATMOSPHERE_PRESSURE.value());
    assert_eq!(template(&inputs), PhaseTemplate::Earth);
    assert!(PhaseTemplate::Earth.row().lunar_lambert_share <= 0.0);
}
