//! P14.T24.h.a's floor over the passes (decision-p14-t54a-passes §G3.5): the Solar System's table
//! bodies under the floor's rules against the area mix's alone.

use hyperion_testkit::float::assert_same_bits;

use crate::planetary::derive::photometry::area_mix::{RayleighFloorRules, with_rayleigh_floor};
use crate::planetary::derive::solar::historic_table;
use crate::units::Pascals;

/// P14.T24.h.a's third test: Earth keeps its Bond albedo to the bit, since the floor does not bind
/// there (the table's Earth, 0.2798 at 23 under its 0.77 bar; the reference Earth's 0.294, which
/// calibrates the water cloud, reads no floor: `the_reference_earth_takes_0_294_by_its_cloud_s_
/// calibration`); the Moon, Mars, Venus and Titan move by under 0.005, stated, as does every other
/// body of the table; and a body whose Bond albedo the floor leaves keeps its equilibrium
/// temperature to the bit.
#[test]
fn earth_keeps_its_bond_to_the_bit_and_the_other_measured_bodies_move_by_under_0_005() {
    let held = historic_table();
    let floored = with_rayleigh_floor(RayleighFloorRules::Floored, historic_table);
    assert_eq!(held.len(), floored.len());
    for ((name, before), (again, after)) in held.iter().zip(&floored) {
        assert_eq!(name, again);
        let (a, b) = (before.albedo().value(), after.albedo().value());
        eprintln!(
            "{name}: Bond {a:.6} → {b:.6} ({:+.2e}), T_s {:.3} → {:.3} K, p_s {:.4e} Pa",
            b - a,
            before.atmosphere().surface_temperature().value(),
            after.atmosphere().surface_temperature().value(),
            after
                .atmosphere()
                .surface_pressure()
                .map_or(0.0, Pascals::value),
        );
        assert!((b - a).abs() < 0.005, "{name} moves {a} → {b}");
        if a.total_cmp(&b).is_eq() {
            assert_same_bits(
                before.equilibrium_temperature().value(),
                after.equilibrium_temperature().value(),
            );
        }
        if *name == "Earth" {
            assert_same_bits(a, b);
        }
    }
    for name in ["Moon", "Mars", "Venus", "Titan"] {
        assert!(
            held.iter().any(|(n, _)| *n == name),
            "{name} is a table body"
        );
    }
}

/// A Earth-mass body about a dark host, which has no light (P14.T24.i): derived over the passes'
/// stand-in, whose climate reads no heat flow.
fn dark_body() -> crate::planetary::derive::DerivedBody {
    use crate::planetary::derive::solar::{SOLAR_AGE, orbit, solar_disc};
    use crate::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
    use crate::stellar::Composition;
    use crate::stellar::draws::UnitUniform;
    use crate::time::UniverseTime;
    use crate::units::consts::SOLAR_MASS_KG;
    use crate::units::{EarthMasses, Kelvin, Kilograms, SolarLuminosities, SolarRadii};

    let disc = solar_disc();
    let dark = [HostLight::new(SolarLuminosities::ZERO, Kelvin::ZERO, SolarRadii::ZERO).unwrap()];
    let hosts = BodyHosts::new(
        Kilograms::new(SOLAR_MASS_KG),
        Composition::SOLAR,
        &dark,
        &[],
    )
    .unwrap();
    let o = orbit(1.0, 0.0);
    let placed = PlacedBody::new(
        EarthMasses::new(1.0),
        o,
        o.semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap();
    derive_body(&placed, &hosts, &disc, SOLAR_AGE, UniverseTime::EPOCH).unwrap()
}

/// P14.T24.i: an unlit airless body's mean surface temperature is its night floor, held at 24.
/// Held, it is its equilibrium temperature, 0 K; under the floor's rules the passes' stand-in,
/// which reads no heat flow, gives the cosmic background's 2.725 K, its state and equilibrium
/// temperature unchanged.
#[test]
fn an_unlit_airless_body_takes_its_night_floor() {
    use crate::planetary::derive::atmosphere::{SurfaceState, UnlitMeanRules, with_unlit_mean};
    use crate::planetary::hooks::surface::COSMIC_BACKGROUND_TEMPERATURE;

    let held = dark_body();
    assert_eq!(held.atmosphere().state(), SurfaceState::Airless);
    assert_same_bits(held.atmosphere().surface_temperature().value(), 0.0);
    let floored = with_unlit_mean(UnlitMeanRules::Floored, dark_body);
    assert_eq!(floored.atmosphere().state(), SurfaceState::Airless);
    assert_same_bits(
        floored.atmosphere().surface_temperature().value(),
        COSMIC_BACKGROUND_TEMPERATURE.value(),
    );
    assert_same_bits(
        floored.equilibrium_temperature().value(),
        held.equilibrium_temperature().value(),
    );
    // The area mix reads the mean through the partition (an ice's density at it), so the Bond
    // albedo may move in its last digits; nothing lights the body, so nothing reads it.
    let (a, b) = (held.albedo().value(), floored.albedo().value());
    assert!((a - b).abs() < 1e-6, "{a} → {b}");
}

/// P14.T24.i's second test, over the table: every body whose equilibrium temperature is above its
/// floor is unchanged to the bit under the unlit mean's rules (each of the eight is lit).
#[test]
fn a_lit_body_keeps_its_mean_to_the_bit_under_the_unlit_floor() {
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    let held = historic_table();
    let floored = with_unlit_mean(UnlitMeanRules::Floored, historic_table);
    for ((name, before), (_, after)) in held.iter().zip(&floored) {
        assert!(
            before.equilibrium_temperature().value() > 30.0,
            "{name} is lit"
        );
        assert_eq!(format!("{before:?}"), format!("{after:?}"), "{name}");
    }
}

/// P14.T24.i's follow-up on the escape's batch path, every 24 hold in force
/// (decision-t24i-airless-state D1.1, D1.3): the unlit airless body publishes its night floor there
/// too, the passes' stand-in's cosmic background, where the batch's airless arms once published
/// its `T_eq` of 0 K; with the unlit mean held it is 0 K, as at 23.
#[test]
fn under_every_24_hold_an_unlit_airless_body_takes_its_night_floor() {
    use crate::planetary::derive::atmosphere::{SurfaceState, UnlitMeanRules, with_unlit_mean};
    use crate::planetary::hooks::surface::COSMIC_BACKGROUND_TEMPERATURE;
    use crate::planetary::testing::with_every_24_hold;

    let held = with_every_24_hold(|| with_unlit_mean(UnlitMeanRules::Held, dark_body));
    assert_eq!(held.atmosphere().state(), SurfaceState::Airless);
    assert_same_bits(held.atmosphere().surface_temperature().value(), 0.0);
    let floored = with_every_24_hold(dark_body);
    assert_eq!(floored.atmosphere().state(), SurfaceState::Airless);
    assert_same_bits(
        floored.atmosphere().surface_temperature().value(),
        COSMIC_BACKGROUND_TEMPERATURE.value(),
    );
    assert_same_bits(
        floored.equilibrium_temperature().value(),
        held.equilibrium_temperature().value(),
    );
}

/// P14.T24.i's follow-up over the table, every 24 hold in force (decision-t24i-airless-state
/// D1.4): every body, each lit above its floor, Venus, Earth, Mars and Titan among them, is the
/// same to the bit, its whole derivation, with the unlit mean's rules floored as held, since its
/// base temperature, the greenhouse's start and the ice-held stand-in's temperature, is then its
/// `T_eq`.
#[test]
fn under_every_24_hold_a_lit_body_keeps_its_air_to_the_bit_under_the_unlit_floor() {
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    use crate::planetary::testing::with_every_24_hold;
    let held = with_every_24_hold(|| with_unlit_mean(UnlitMeanRules::Held, historic_table));
    let floored = with_every_24_hold(historic_table);
    assert_eq!(held.len(), floored.len());
    for ((name, before), (again, after)) in held.iter().zip(&floored) {
        assert_eq!(name, again);
        assert!(
            before.equilibrium_temperature().value() > 30.0,
            "{name} is lit"
        );
        assert_eq!(format!("{before:?}"), format!("{after:?}"), "{name}");
    }
    for name in ["Venus", "Earth", "Mars", "Titan"] {
        assert!(
            held.iter().any(|(n, _)| *n == name),
            "{name} is a table body"
        );
    }
}

/// The unlit mean is held until P14.T54.b's bump: below it every airless mean is its equilibrium
/// temperature, and the tests' switch reaches the floor ahead of it.
#[test]
fn the_unlit_mean_is_held_only_until_the_24_bump() {
    use super::UNLIT_MEAN_VERSION;
    use crate::GENERATOR_VERSION;
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    assert!(
        GENERATOR_VERSION < UNLIT_MEAN_VERSION,
        "the 23 → 24 bump (P14.T54.b) puts the unlit mean on the record: an airless body's mean \
         surface temperature is the larger of its equilibrium temperature and its night floor \
         (P14.T24.i). Delete `UNLIT_MEAN_VERSION`, `UnlitMeanRules` with its `Held` arm, \
         `with_unlit_mean` and its thread's rules, the `Held` branch of `surface_mix::\
         airless_mean`, and this test. Then settle the tests that read 23's 0 K: \
         `a_dark_host_leaves_a_body_frozen_and_airless` (the stand-in's T_CMB), \
         `under_the_batch_the_photometry_states_the_derivation_s_albedo` (an airless mean is its \
         T_eq only where it is lit), `a_surface_the_registry_or_the_record_does_not_model_answers_\
         not_modelled` and `every_generated_solid_body_reads_into_inputs_the_builder_takes` (no \
         unlit refusal), and `the_radiative_model_reads_its_record_s_inertia_on_every_body`. \
         The base temperature reads it on both paths (P14.T24.i's follow-up, \
         decision-t24i-airless-state D1.3): delete its switch from \
         `planetary::testing::with_every_24_hold`, and settle the follow-up's tests, which compare \
         every 24 hold with every 24 hold but this one: \
         `under_every_24_hold_an_unlit_airless_body_takes_its_night_floor`, \
         `under_every_24_hold_a_lit_body_keeps_its_air_to_the_bit_under_the_unlit_floor`, and in \
         `planetary::system`'s tests `under_every_24_hold_no_surface_lies_below_its_night_floor`, \
         `under_every_24_hold_a_body_above_its_floor_is_unchanged_by_it`, \
         `under_every_24_hold_the_four_split_bodies_read_their_floor` and \
         `under_every_24_hold_the_floor_s_moved_bodies_are_stated` (their `FloorCensus` then needs \
         no held run)"
    );
    assert_eq!(UnlitMeanRules::in_force(), UnlitMeanRules::Held);
    assert_eq!(
        with_unlit_mean(UnlitMeanRules::Floored, UnlitMeanRules::in_force),
        UnlitMeanRules::Floored
    );
    assert_eq!(UnlitMeanRules::in_force(), UnlitMeanRules::Held);
}
