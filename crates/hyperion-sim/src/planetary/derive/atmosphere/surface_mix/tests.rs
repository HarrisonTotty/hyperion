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
         no held run), and T24.i's and P14.T49.n's record tests in the 24 combination, \
         `under_every_24_hold_the_golden_systems_unlit_records_read_their_night_floor`, \
         `under_every_24_hold_the_sample_s_unlit_records_read_their_night_floor`, \
         `under_24_s_densities_the_golden_systems_unlit_records_all_read` and \
         `under_24_s_densities_the_sample_s_unlit_records_all_read`, with their `unlit_moves` and \
         `unlit_moves_of` (each then compares its records with 23's, or states them alone)"
    );
    assert_eq!(UnlitMeanRules::in_force(), UnlitMeanRules::Held);
    assert_eq!(
        with_unlit_mean(UnlitMeanRules::Floored, UnlitMeanRules::in_force),
        UnlitMeanRules::Floored
    );
    assert_eq!(UnlitMeanRules::in_force(), UnlitMeanRules::Held);
}

/// P14.T53.b part 1 (a) at the passes' stand-in cover (the batch review's fixes): the stand-in's
/// partition (`given_partition`) gives its water's cover (`pass_water_cover`) the state the air's
/// water follows. Over [`past_only_runaways`]' cases, every 24 hold in force:
///
/// - each past-only runaway, published `Temperate` or `Snowball`, holds no surface water and no
///   ice in its pass's cover, and its published condensates no `H2O`;
/// - over its control's own partition and climate a runaway's water state alone takes the water
///   away: no surface water, no potential ocean and no ice, where the control, whose water is its
///   own, holds its surface water and, as a `Snowball`, its uncapped cold area as ice; so the
///   stand-in's partition of the control's air under a runaway's water publishes no `H2O`, where
///   the control's publishes its liquid.
///
/// [`past_only_runaways`]: crate::planetary::derive::atmosphere::tests::past_only_runaways
#[test]
fn under_every_24_hold_a_past_only_runaway_s_stand_in_cover_holds_no_water_or_ice() {
    use super::super::{Atmosphere, ClimateOf, Gas, SurfaceState};
    use super::{PassLight, PassSurface, given_partition, isothermal_climate, pass_water_cover};
    use crate::planetary::chem::partition::{CondensatePhase, Ground, partition};
    use crate::planetary::derive::atmosphere::tests::past_only_runaways;
    use crate::planetary::testing::with_every_24_hold;
    use crate::units::MetresPerSecondSquared;
    use crate::units::consts::GRAVITATIONAL_CONSTANT;
    let surface = PassSurface::stand_in(PassLight::HottestHost);
    let climate_of: &ClimateOf<'_> = &isothermal_climate;
    let water = Gas::Water.substance();
    for case in past_only_runaways() {
        with_every_24_hold(|| {
            let (m, r) = (case.inputs.mass.value(), case.inputs.radius.value());
            let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r));
            // The cover of the partition of `air`'s column at its climate, under `water_state`.
            let cover_of = |air: &Atmosphere, water_state: SurfaceState| {
                let climate = climate_of(air.surface_temperature, &air.partials);
                let ground = Ground::new(gravity, surface.basins, surface.water_at_surface);
                let solved = partition(&climate, &ground, &air.column_inventory());
                pass_water_cover(
                    &solved,
                    (air.state, water_state, case.inputs.material),
                    &surface,
                    &climate,
                )
            };
            let published = |air: &Atmosphere| {
                given_partition(air, &case.inputs, &surface, (climate_of, gravity)).1
            };
            let h2o = |air: &Atmosphere| {
                let condensates = published(air);
                let area = |phase| {
                    condensates
                        .entry(water, phase)
                        .map(crate::planetary::chem::partition::CondensateEntry::area_fraction)
                };
                (area(CondensatePhase::Solid), area(CondensatePhase::Liquid))
            };
            let now = cover_of(&case.air, case.air.water_state);
            let gated = cover_of(&case.control, SurfaceState::RunawayGreenhouse);
            let control = cover_of(&case.control, case.control.water_state);
            let dried = Atmosphere {
                water_state: SurfaceState::RunawayGreenhouse,
                ..case.control
            };
            eprintln!(
                "a past-only runaway published {:?}: its cover {now:?}, its H2O {:?}; over its \
                 control's partition under a runaway's water {gated:?}, its H2O {:?}; the \
                 control's {control:?}, its H2O {:?}",
                case.published,
                h2o(&case.air),
                h2o(&dried),
                h2o(&case.control),
            );
            for cover in [&now, &gated] {
                assert_same_bits(cover.surface_water.value(), 0.0);
                assert_same_bits(cover.potential_ocean, 0.0);
                assert_same_bits(cover.ice_fraction, 0.0);
            }
            assert_eq!(h2o(&case.air), (None, None), "{:?}", case.published);
            assert!(control.surface_water.value() > 0.0, "{control:?}");
            // The control publishes its water (a liquid entry, covering no area in the stand-in,
            // which knows no basins); under a runaway's water the same air publishes none.
            assert!(h2o(&case.control).1.is_some(), "{:?}", case.published);
            assert_eq!(h2o(&dried), (None, None), "{:?}", case.published);
            if case.published == SurfaceState::Snowball {
                assert_same_bits(control.ice_fraction, 1.0);
            }
        });
    }
}
