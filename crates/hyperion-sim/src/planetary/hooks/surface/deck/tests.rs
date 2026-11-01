//! P14.T24.j's tests: the hold, the deck's pieces, and the table's Earth under the lock.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::atmosphere::{Crust, SurfaceMaterial};
use crate::planetary::derive::pass_record::{OpenWater, PassContext};
use crate::planetary::derive::passes::Context;
use crate::planetary::derive::solar::historic;
use crate::planetary::hooks::surface::tests::{inputs_of, spin_of};
use crate::planetary::hooks::surface::{
    ClimateBody, GroundInertia, Surface, SurfaceConditions, SurfaceInputs, climate_of, night_floor,
    step_climate,
};
use crate::planetary::surface::reference;
use crate::units::Radians;

/// The table's Earth's spin locked 1:1 about the reference locked Earth's M dwarf: no obliquity,
/// no solar day, its rotation its year of 21.10 days (`reference::locked_earth_like`).
fn locked_spin() -> SeasonalSpin {
    SeasonalSpin {
        obliquity: Radians::ZERO,
        rotation_period: reference::locked_earth_like().spin().rotation_period,
        solar_day: None,
    }
}

/// The deck of the column `inputs` describe, as [`SurfaceConditions::derive`] solves it.
fn decked_of(inputs: &SurfaceInputs<'_>) -> Decked {
    let column = Surface::of(inputs).unwrap().column;
    let floor = night_floor(inputs.heat_flow);
    let ground =
        GroundInertia::at_equilibrium(inputs.lid, inputs.material, inputs.equilibrium_temperature);
    under_deck(
        &column,
        &DeckBody {
            spin: inputs.spin,
            stellar_flux: inputs.stellar_flux,
            liquid_water: inputs.holds_liquid_water,
        },
        |column| climate_of(column, &inputs.spin, inputs.ocean_weight, ground, floor),
    )
}

/// The deck's albedo and the locked greenhouse's share are Yang et al. 2013's at each row of
/// their Table 1, to the bit, linear between, and held beyond.
#[test]
fn the_deck_s_albedo_and_greenhouse_share_are_the_table_s_at_its_rows() {
    let flux = WattsPerSquareMetre::new;
    for (s, albedo) in DECK_ALBEDO {
        assert_same_bits(deck_albedo(flux(s)), albedo);
    }
    for ((s, locked), (_, rotator)) in LOCKED_GREENHOUSE_K.into_iter().zip(ROTATOR_GREENHOUSE_K) {
        assert_same_bits(locked_greenhouse_share(flux(s)), locked / rotator);
    }
    let shares = [0.432, 0.288, 0.202, 0.190];
    for ((s, _), share) in LOCKED_GREENHOUSE_K.into_iter().zip(shares) {
        let at = locked_greenhouse_share(flux(s));
        assert!((at - share).abs() < 5e-4, "{s} W m⁻²: {at}");
    }
    // Held at the ends, and linear between the rows.
    for s in [0.0, 500.0, 999.9] {
        assert_same_bits(deck_albedo(flux(s)), 0.42);
        assert_same_bits(locked_greenhouse_share(flux(s)), 7.3 / 16.9);
    }
    for s in [2_200.1, 3_000.0, f64::INFINITY] {
        assert_same_bits(deck_albedo(flux(s)), 0.53);
    }
    for s in [1_600.1, 2_200.0, 5_000.0] {
        assert_same_bits(locked_greenhouse_share(flux(s)), 6.1 / 32.1);
    }
    assert!((deck_albedo(flux(1_100.0)) - 0.435).abs() < 1e-12);
    let mid = f64::midpoint(5.6 / 27.7, 6.1 / 32.1);
    assert!((locked_greenhouse_share(flux(1_500.0)) - mid).abs() < 1e-12);
}

/// The rotation regime's weight is none to the transition at 5 days per Earth radius and whole
/// from twice it, for an Earth and a world of twice its radius.
#[test]
fn the_rotation_weight_turns_on_between_the_transition_and_twice_it() {
    let day = SECONDS_PER_DAY;
    for scale in [1.0, 2.0, 0.5] {
        let radius = Metres::new(EARTH_RADIUS_M * scale);
        let transition = 5.0 * day * scale;
        for period in [0.1 * day, transition * 0.99, transition] {
            assert_same_bits(rotation_weight(Seconds::new(period), radius), 0.0);
        }
        for period in [
            transition * 2.0,
            transition * 2.01,
            365.25 * day,
            f64::INFINITY,
        ] {
            assert_same_bits(rotation_weight(Seconds::new(period), radius), 1.0);
        }
        let between = rotation_weight(Seconds::new(transition * core::f64::consts::SQRT_2), radius);
        assert!((between - 0.5).abs() < 1e-12, "{between}");
    }
    assert_same_bits(
        rotation_weight(Seconds::new(f64::NAN), Metres::new(EARTH_RADIUS_M)),
        0.0,
    );
    assert_same_bits(rotation_weight(Seconds::new(1e9), Metres::new(0.0)), 0.0);
}

/// The flux's, the pressure's and the open water's weights are none and whole at their ends.
#[test]
fn the_tapers_are_none_and_whole_at_their_ends() {
    let flux = |s: f64| flux_weight(WattsPerSquareMetre::new(s));
    for s in [0.0, 500.0, 800.0, f64::NAN] {
        assert_same_bits(flux(s), 0.0);
    }
    for s in [1_000.0, 1_361.0, 3_000.0] {
        assert_same_bits(flux(s), 1.0);
    }
    assert!((flux(900.0) - 0.5).abs() < 1e-12);
    let pressure = |p: f64| pressure_weight(Pascals::new(p));
    for p in [3.0e6, 5.0e6, 4.7e7, 0.0, -1.0, f64::NAN] {
        assert_same_bits(pressure(p), 0.0);
    }
    for p in [1.0, 1.0e5, 1.0e6] {
        assert_same_bits(pressure(p), 1.0);
    }
    // Halfway in ln p: 10 √3 bar.
    assert!((pressure(1.0e6 * 3_f64.sqrt()) - 0.5).abs() < 1e-12);
    let melting = Kelvin::new(273.16);
    let open = |t: f64| open_water_weight(Kelvin::new(t), melting);
    for t in [200.0, 273.0, 273.16, f64::NAN] {
        assert_same_bits(open(t), 0.0);
    }
    for t in [275.16, 300.0] {
        assert_same_bits(open(t), 1.0);
    }
    assert!((open(274.16) - 0.5).abs() < 1e-12);
}

/// The deck moves no column on a world that turns under its host, on a dry locked world, and on
/// a frozen one, and none at all under its hold: the column, and the conditions, to the bit.
#[test]
fn no_deck_lies_on_a_rotator_a_dry_world_or_a_frozen_one() {
    let earth = historic("Earth");
    let held = |inputs: &SurfaceInputs<'_>| {
        with_locked_climate(LockedClimateRules::Held, || {
            SurfaceConditions::derive(inputs).unwrap()
        })
    };
    let deck = |inputs: &SurfaceInputs<'_>| {
        {
            (
                decked_of(inputs),
                SurfaceConditions::derive(inputs).unwrap(),
            )
        }
    };
    let rotator = inputs_of(&earth, spin_of("Earth"));
    let dry = SurfaceInputs {
        holds_liquid_water: false,
        ..inputs_of(&earth, locked_spin())
    };
    for (name, inputs) in [("rotator", rotator), ("dry", dry)] {
        let (decked, conditions) = deck(&inputs);
        assert_same_bits(decked.weight, 0.0);
        assert_eq!(
            decked.column,
            Surface::of(&inputs).unwrap().column,
            "{name}"
        );
        assert_eq!(conditions, held(&inputs), "{name}");
    }
    // A frozen world in the deck's domain: the table's Earth locked, its column 70 K colder, so
    // that its warmest climate lies under the melting point without the deck.
    let inputs = inputs_of(&earth, locked_spin());
    let column = Surface::of(&inputs).unwrap().column;
    let frozen = Column {
        surface_temperature: Kelvin::new(column.surface_temperature.value() - 70.0),
        ..column
    };
    let body = DeckBody {
        spin: inputs.spin,
        stellar_flux: inputs.stellar_flux,
        liquid_water: true,
    };
    let floor = night_floor(inputs.heat_flow);
    let ground =
        GroundInertia::at_equilibrium(inputs.lid, inputs.material, inputs.equilibrium_temperature);
    let decked = {
        assert_same_bits(domain_weight(&frozen, &body), 1.0);
        under_deck(&frozen, &body, |column| {
            climate_of(column, &inputs.spin, inputs.ocean_weight, ground, floor)
        })
    };
    assert_same_bits(decked.weight, 0.0);
    assert_eq!(decked.column, frozen);
    let melting = ice_ih_melting_temperature(frozen.surface_pressure);
    assert!(
        decked.climate.range.1 < melting,
        "{:?}",
        decked.climate.range
    );
}

/// The table's Earth locked 1:1 about the reference locked Earth's dwarf (21.10 d, no obliquity)
/// under the deck: its mean 245.4 K, its day–night contrast 47.2 K and its equator–pole 25.2 K
/// (the pilot's, each to 1 K), the deck whole over its open water; and under the hold its mean
/// and day–night contrast as built, 290.38 and 59.85 K.
#[test]
fn the_table_s_earth_under_the_lock_takes_the_whole_deck() {
    let earth = historic("Earth");
    let inputs = inputs_of(&earth, locked_spin());
    let held = with_locked_climate(LockedClimateRules::Held, || {
        SurfaceConditions::derive(&inputs).unwrap()
    });
    let (decked, conditions) = {
        (
            decked_of(&inputs),
            SurfaceConditions::derive(&inputs).unwrap(),
        )
    };
    let melting = ice_ih_melting_temperature(decked.column.surface_pressure);
    eprintln!(
        "the table's Earth locked: held {:.3} K, {:.3} K day–night, {:.3} K equator–pole; deck \
         w {}, {:.3} K, {:.3} K day–night, {:.3} K equator–pole, range {:?}, T_eq {:.3} K, \
         open {}",
        held.mean_surface_temperature().value(),
        held.day_night_contrast_k(),
        held.equator_pole_contrast_k(),
        decked.weight,
        conditions.mean_surface_temperature().value(),
        conditions.day_night_contrast_k(),
        conditions.equator_pole_contrast_k(),
        conditions.climate_range(),
        decked.column.equilibrium_temperature.value(),
        open_water_weight(decked.climate.range.1, melting),
    );
    for (name, figure, pilot) in [
        ("mean", conditions.mean_surface_temperature().value(), 245.4),
        ("day–night", conditions.day_night_contrast_k(), 47.2),
        ("equator–pole", conditions.equator_pole_contrast_k(), 25.2),
    ] {
        assert!(
            (figure - pilot).abs() < 1.0,
            "{name}: {figure} K against {pilot} K"
        );
    }
    assert_same_bits(decked.weight, 1.0);
    assert_same_bits(open_water_weight(decked.climate.range.1, melting), 1.0);
    // The air's own mean, T13.c's, is not the deck's.
    assert_same_bits(
        earth.atmosphere().surface_temperature().value(),
        held.mean_surface_temperature().value(),
    );
    for (name, figure, built) in [
        ("mean", held.mean_surface_temperature().value(), 290.38),
        ("day–night", held.day_night_contrast_k(), 59.85),
    ] {
        assert!(
            (figure - built).abs() < 0.01,
            "{name}: {figure} K against {built} K"
        );
    }
}

/// Where the deck would freeze the open water under it, its weight is the root of
/// w = `w_d` `f_open`(w), and the warmest climate stays above water's melting point: the table's
/// Earth locked under a column cooled step by step through the ramp.
#[test]
fn a_deck_that_would_freeze_its_sea_stops_at_the_open_water_s_edge() {
    let earth = historic("Earth");
    let inputs = inputs_of(&earth, locked_spin());
    let column = Surface::of(&inputs).unwrap().column;
    let floor = night_floor(inputs.heat_flow);
    let ground =
        GroundInertia::at_equilibrium(inputs.lid, inputs.material, inputs.equilibrium_temperature);
    let body = DeckBody {
        spin: inputs.spin,
        stellar_flux: inputs.stellar_flux,
        liquid_water: true,
    };
    let climate =
        |column: &Column| climate_of(column, &inputs.spin, inputs.ocean_weight, ground, floor);
    let melting = ice_ih_melting_temperature(column.surface_pressure);
    let mut partial = 0;
    {
        for cooled in 0..60 {
            let cold = Column {
                surface_temperature: Kelvin::new(
                    column.surface_temperature.value() - f64::from(cooled),
                ),
                ..column
            };
            let domain = domain_weight(&cold, &body);
            let decked = under_deck(&cold, &body, climate);
            let open = open_water_weight(decked.climate.range.1, melting);
            if decked.weight > 0.0 && decked.weight < domain {
                partial += 1;
                assert!(decked.climate.range.1 > melting, "{cooled} K: {decked:?}");
                assert!(
                    (decked.weight - domain * open).abs() < 1e-9,
                    "{cooled} K: w {} against {}",
                    decked.weight,
                    domain * open
                );
            } else if decked.weight <= 0.0 {
                assert_eq!(decked.column, cold, "{cooled} K");
            } else {
                assert_same_bits(open, 1.0);
            }
        }
    };
    assert!(partial >= 2, "{partial} columns at the open water's edge");
}

/// The passes' greenhouse steps read the record's deck: at the table's Earth's own air locked
/// under the deck, a step's span is the record's decked range about the step's own mean, to the
/// bit, and over no open water (the previous pass's) the held climate's.
#[test]
fn the_passes_steps_read_the_record_s_deck() {
    let earth = historic("Earth");
    let spin = locked_spin();
    let inputs = inputs_of(&earth, spin);
    let air = earth.atmosphere();
    let body = |holds_liquid_water| ClimateBody {
        equilibrium_temperature: earth.equilibrium_temperature(),
        surface_gravity: earth.surface_gravity(),
        radius: Metres::from(earth.radius()),
        ground: GroundInertia::of_material(SurfaceMaterial::of(&earth.fractions())),
        spin,
        heat_flow: inputs.heat_flow,
        inventory: *air.inventory_weights(),
        ceiling: air.greenhouse_ceiling(),
        stellar_flux: inputs.stellar_flux,
        holds_liquid_water,
        crust: Crust::Solid,
    };
    let t = air.surface_temperature();
    let step = |holds| {
        step_climate(
            &body(holds),
            inputs.ocean_weight,
            t,
            air.partial_pressures(),
        )
    };
    let held = with_locked_climate(LockedClimateRules::Held, || step(true));
    {
        let conditions = SurfaceConditions::derive(&inputs).unwrap();
        let (cold, warm) = conditions.climate_range();
        let decked = step(true);
        assert_same_bits(decked.mean().value(), t.value());
        assert_same_bits(decked.coldest().value(), cold.value().min(t.value()));
        assert_same_bits(decked.warmest().value(), warm.value().max(t.value()));
        assert!(
            decked.coldest() < held.coldest(),
            "{decked:?} against {held:?}"
        );
        let dry = step(false);
        for (a, b) in [
            (dry.coldest(), held.coldest()),
            (dry.mean(), held.mean()),
            (dry.warmest(), held.warmest()),
        ] {
            assert_same_bits(a.value(), b.value());
        }
    };
}

/// A pass's context carries its open water to the next only where a deck reads it: under the
/// deck, on a world locked 1:1 in its domain, never under its hold, on a rotator, with no air or
/// under too faint a light; and the context reads open water only where its pass listed it.
#[test]
fn the_context_carries_open_water_only_where_a_deck_reads_it() {
    let earth = historic("Earth");
    let inputs = inputs_of(&earth, locked_spin());
    let (radius, flux) = (inputs.radius, inputs.stellar_flux);
    let p = Pascals::new(1e5);
    let reads = |spin: &SeasonalSpin, flux, p| reads_open_water(spin, radius, flux, p);
    assert!(!reads(&locked_spin(), flux, p), "under the hold");
    {
        assert!(reads(&locked_spin(), flux, p));
        assert!(!reads(&spin_of("Earth"), flux, p), "a rotator");
        assert!(!reads(&locked_spin(), flux, Pascals::ZERO), "no air");
        let faint = WattsPerSquareMetre::new(700.0);
        assert!(!reads(&locked_spin(), faint, p), "too faint");
    };
    let none = PassContext::NONE;
    assert_eq!(none.key().open_water, OpenWater::Dry);
    assert!(!none.holds_liquid_water());
    assert!(none.with_open_water(OpenWater::Open).holds_liquid_water());
}
