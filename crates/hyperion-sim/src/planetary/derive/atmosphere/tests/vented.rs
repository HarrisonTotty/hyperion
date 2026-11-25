//! P14.T55.a.c's carriers on the snapshot path, under their hold at 36 ([`with_every_36_hold`]):
//! the vent's quenched gas on the Solar System's table and on hand-built bodies, its balance, its
//! held bodies and the carbon dioxide term's stand-in for the vent's methane.

use super::*;
use crate::planetary::chem::outgassing::{SpeciateVentGasError, VentElements, VentGas, VentInputs};
use crate::planetary::derive::atmosphere::{Outgassed, VentRoute};
use crate::planetary::testing::with_every_36_hold;

/// The table at 36, every other 36 hold in force.
fn table_at_36() -> Vec<(&'static str, DerivedBody)> {
    with_every_36_hold(table)
}

/// Whether two airs publish the same, to the bit: their states, surface temperatures and
/// pressures, partial pressures, Bond albedos and optical depths.
fn same_air(a: &Atmosphere, b: &Atmosphere) -> bool {
    let bits = |x: f64, y: f64| x.total_cmp(&y).is_eq();
    a.state() == b.state()
        && bits(
            a.surface_temperature().value(),
            b.surface_temperature().value(),
        )
        && match (a.surface_pressure(), b.surface_pressure()) {
            (Some(p), Some(q)) => bits(p.value(), q.value()),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
        && Gas::ALL.iter().all(|&gas| {
            bits(
                a.partial_pressures().of(gas).value(),
                b.partial_pressures().of(gas).value(),
            )
        })
        && bits(a.albedo().value(), b.albedo().value())
        && bits(a.optical_depth(), b.optical_depth())
}

/// The vent's record of `air`, which took the vent's route.
fn vented_of(name: &str, air: &Atmosphere) -> Outgassed {
    match air.vent() {
        Some(VentRoute::Vented(out)) => out,
        other => panic!("{name} took {other:?}"),
    }
}

/// The moles of carbon `air` publishes, as CO₂ and CH₄, per unit of weight (Pa ÷ g mol⁻¹).
fn carbon_moles(air: &Atmosphere) -> (f64, f64) {
    let p = air.partial_pressures();
    (
        p.of(Gas::CarbonDioxide).value() / Gas::CarbonDioxide.molar_mass_g_per_mol(),
        p.of(Gas::Methane).value() / Gas::Methane.molar_mass_g_per_mol(),
    )
}

/// The relative difference of `a` and `b`, 0 for equal bits.
fn relative(a: f64, b: f64) -> f64 {
    if a.total_cmp(&b).is_eq() {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// Below 36 no body takes the route and no air records one; under the hold the table's carriers
/// follow ruling 1: Earth and Mars vent, Venus and Mercury are hot airs held on P14.T24.f's
/// carriers, the cold ice bodies (Titan, Ganymede, Ceres) are on its methane arm, and the Moon
/// keeps no carbon; every body not vented publishes the same air to the bit.
#[test]
fn at_36_the_table_s_carriers_are_its_vents_but_its_hot_airs_and_its_ices() {
    let held = table();
    for (name, body) in &held {
        assert_eq!(body.atmosphere().vent(), None, "{name}");
    }
    let at_36 = table_at_36();
    for ((name, before), (_, after)) in held.iter().zip(&at_36) {
        let route = after.atmosphere().vent();
        let want = match *name {
            "Mercury" | "Venus" => Some(VentRoute::Hot),
            "Titan" | "Ganymede" | "Ceres" => Some(VentRoute::MethaneArm),
            "Moon" => Some(VentRoute::NoCarbon),
            "Earth" | "Mars" => {
                assert!(
                    matches!(route, Some(VentRoute::Vented(_))),
                    "{name}: {route:?}"
                );
                continue;
            }
            other => panic!("the table names {other}"),
        };
        assert_eq!(route, want, "{name}");
        assert!(
            same_air(before.atmosphere(), after.atmosphere()),
            "{name} moved off the route"
        );
    }
}

/// research-leans-58's vent figures, re-derived on the registry's CEA records and O'Neill's IW
/// (part b's gas), as the ruling computed them: at 1,473 K CO₂/CO 0.338 and H₂O/H₂ 0.838 at
/// IW+0.01 (Mars's model mantle), 21.1 and 52.2 at IW+3.6 (Earth's), each within 1%, and graphite's
/// cap 12.5 bar at IW−3 and 1.25 bar at IW−5 within 1%.
#[test]
fn the_ruling_s_vent_figures_hold_on_the_registry_s_records() {
    let vent = |delta_iw: f64| {
        VentGas::speciate(
            &VentInputs {
                delta_iw: Dex::new(delta_iw),
                temperature: Kelvin::new(1_473.0),
                pressure: Pascals::new(1e5),
            },
            &VentElements::new(2.0, 0.1, 0.01, 0.0),
        )
        .expect("a vent at 1 bar")
    };
    let near = |got: f64, want: f64| (got / want - 1.0).abs() < 0.01;
    for (delta_iw, carbon, water) in [(0.01, 0.338, 0.838), (3.6, 21.1, 52.2)] {
        let gas = vent(delta_iw);
        let (c, w) = (gas.carbon_dioxide_to_monoxide(), gas.water_to_hydrogen());
        eprintln!("IW{delta_iw:+}: CO2/CO {c:.4}, H2O/H2 {w:.4}");
        assert!(
            near(c, carbon) && near(w, water),
            "IW{delta_iw:+}: {c}, {w}"
        );
    }
    for (delta_iw, bar) in [(-3.0, 12.5), (-5.0, 1.25)] {
        let cap = vent(delta_iw).carbon_monoxide_at_graphite().value() / 1e5;
        eprintln!("IW{delta_iw:+}: graphite's cap {cap:.4} bar");
        assert!(near(cap, bar), "IW{delta_iw:+}: {cap} bar");
    }
}

/// Ruling 1 on the table's Earth: its vent, at its own air's weight, is CO₂-led at its mantle's
/// CO₂/CO; its CO shifts back to CO₂ with its water, so its carbon is unchanged (to 10⁻⁹), no
/// graphite, a methane share under 10⁻⁹, no dry CO; its water pays the shift mole for mole and its
/// hydrogen is lost; and its measured state (the table's inputs at Earth's measured Bond albedo)
/// stays 288 ± 2 K.
#[test]
fn earth_s_carbon_is_unchanged_its_co_shifted_by_its_water_and_its_hydrogen_lost() {
    let held = table();
    let at_36 = table_at_36();
    let (before, after) = (found(&held, "Earth"), found(&at_36, "Earth"));
    let air = after.atmosphere();
    let out = vented_of("Earth", air);
    eprintln!("Earth at 36: {out:?}");
    let shares = out.shares;
    assert!(
        shares.carbon_dioxide > 0.9 && shares.methane < 1e-9,
        "{shares:?}"
    );
    let vent = VentGas::speciate(
        &VentInputs {
            delta_iw: after.element_inventory().mantle_delta_iw(),
            temperature: crate::planetary::chem::outgassing::DEFAULT_VENT_TEMPERATURE,
            pressure: out.pressure,
        },
        &VentElements::new(2.0, 0.1, 0.01, 0.0),
    )
    .expect("Earth's vent");
    assert!(
        relative(
            shares.carbon_dioxide / shares.carbon_monoxide,
            vent.carbon_dioxide_to_monoxide()
        ) < 1e-9
    );
    assert!(
        (out.pressure.value() / 1e5 - 0.77).abs() < 0.1,
        "{:?}",
        out.pressure
    );
    assert!(out.holds_water);
    assert_eq!(out.graphite, Kilograms::ZERO);
    assert_eq!(out.dry_monoxide, Kilograms::ZERO);
    assert_eq!(out.hydrogen_kept, Kilograms::ZERO);
    assert_eq!(air.partial_pressures().of(Gas::Hydrogen), Pascals::ZERO);
    // The shift's water, mole for mole with the CO it shifted, and the methane's two.
    let shifted = out.shifted.value() / Gas::CarbonDioxide.molar_mass_g_per_mol();
    let methane = out.methane.value() / Gas::Methane.molar_mass_g_per_mol();
    let debit = out.water_debit.value() / Gas::Water.molar_mass_g_per_mol();
    assert!(relative(debit, shifted + 2.0 * methane) < 1e-12);
    let carbon =
        before.inventory().carbon_dioxide().value() / Gas::CarbonDioxide.molar_mass_g_per_mol();
    assert!(
        relative(shifted, shares.carbon_monoxide * carbon) < 1e-9,
        "{shifted} of {carbon}"
    );
    let ((co2, ch4), (co2_held, _)) = (carbon_moles(air), carbon_moles(before.atmosphere()));
    assert!(
        relative(co2 + ch4, co2_held) < 1e-9,
        "{co2} + {ch4} against {co2_held}"
    );
    let measured = with_every_36_hold(|| {
        atmosphere_under(
            &table_earth_inputs(after, at_measured_albedo(after, SurfaceState::Temperate)),
            &PassSurface::stand_in(PassLight::HottestHost),
        )
    });
    let t = measured.surface_temperature().value();
    eprintln!("the measured Earth at 36: {t:.3} K");
    assert!((t - EARTH_SURFACE_TEMPERATURE).abs() < 2.0, "{t} K");
    assert!(matches!(measured.vent(), Some(VentRoute::Vented(_))));
}

/// Ruling 2 on the table's Mars: its vent at its model mantle is CO-led, near ¾ of its carbon,
/// and its water's OH shifts all of it back to CO₂, so it is CO₂-led with no dry CO and its carbon
/// unchanged; its hydrogen is lost.
#[test]
fn mars_s_vent_is_co_led_and_its_water_makes_it_carbon_dioxide() {
    let held = table();
    let at_36 = table_at_36();
    let (before, after) = (found(&held, "Mars"), found(&at_36, "Mars"));
    let out = vented_of("Mars", after.atmosphere());
    eprintln!("Mars at 36: {out:?}");
    assert!(
        out.shares.carbon_monoxide > 0.7 && out.shares.carbon_monoxide < 0.8,
        "{:?}",
        out.shares
    );
    assert!(out.holds_water);
    assert_eq!(out.dry_monoxide, Kilograms::ZERO);
    assert_eq!(out.hydrogen_kept, Kilograms::ZERO);
    let ((co2, ch4), (co2_held, _)) = (
        carbon_moles(after.atmosphere()),
        carbon_moles(before.atmosphere()),
    );
    assert!(ch4 < 1e-6 * co2, "{ch4} against {co2}");
    assert!(
        relative(co2 + ch4, co2_held) < 1e-9,
        "{co2} + {ch4} against {co2_held}"
    );
}

/// A hand-built Earth with the vent's route in force, its inventory, ΔIW and vent as given.
fn vented_earth(
    inventory: impl FnOnce(&VolatileInventory) -> VolatileInventory,
    vent: VentConditions,
) -> (AtmosphereInputs, Atmosphere, Atmosphere) {
    vented_earth_at(255.0, Insolation::Habitable, inventory, vent)
}

/// [`vented_earth`] at the equilibrium temperature `equilibrium`, K, in the insolation class
/// `insolation` then and now.
fn vented_earth_at(
    equilibrium: f64,
    insolation: Insolation,
    inventory: impl FnOnce(&VolatileInventory) -> VolatileInventory,
    vent: VentConditions,
) -> (AtmosphereInputs, Atmosphere, Atmosphere) {
    let mut inputs = earth_inputs(equilibrium, insolation);
    inputs.inventory = inventory(&inputs.inventory);
    inputs.vent = vent;
    let held = atmosphere(&inputs);
    let at_36 = with_every_36_hold(|| atmosphere(&inputs));
    (inputs, held, at_36)
}

/// Ruling 2's dry arm: an Earth that draws no water keeps its vent's CO, published as P14.T24.f's
/// carbon dioxide stand-in, so that its air is the held one to the bit, the CO counted.
#[test]
fn a_dry_air_keeps_its_co_as_the_carbon_dioxide_stand_in() {
    let (_, held, dry) = vented_earth(
        |drawn| {
            VolatileInventory::new(
                Kilograms::ZERO,
                drawn.carbon_dioxide(),
                drawn.nitrogen(),
                drawn.argon(),
            )
        },
        VentConditions::basaltic(Dex::new(0.01)),
    );
    let out = vented_of("a dry Earth", &dry);
    eprintln!("a dry Earth: {out:?}");
    assert!(!out.holds_water);
    assert_eq!(out.shifted, Kilograms::ZERO);
    assert_eq!(out.water_debit, Kilograms::ZERO);
    assert!(out.dry_monoxide > Kilograms::ZERO);
    assert!(same_air(&held, &dry), "{held:?}\n{dry:?}");
}

/// Ruling 5: an Earth beyond the maximum-greenhouse limit, formed inside the snow line, its carbon
/// all airborne, with a reduced mantle (IW−5) caps its vent's CO at graphite's saturation, 1.25 bar
/// at its basaltic vent, as the air's CO weight; the carbon above it stays in the crust, the carbon
/// in the air and the crust together the drawn carbon kept. The same mantle under a temperate
/// world's thermostat, its vent under its own thin air, reaches no cap.
#[test]
fn a_reduced_vent_leaves_its_carbon_above_graphite_s_cap_in_the_crust() {
    let reduced = VentConditions::basaltic(Dex::new(-5.0));
    let (inputs, _, air) = vented_earth_at(
        230.0,
        Insolation::BeyondMaximumGreenhouse,
        |drawn| *drawn,
        reduced,
    );
    let out = vented_of("a cold reduced Earth", &air);
    eprintln!("a cold reduced Earth: {out:?}");
    assert!(out.graphite > Kilograms::ZERO);
    let atomic = |element: crate::substance::Element| element.standard_atomic_weight_g_per_mol();
    let (c, o) = (crate::substance::Element::C, crate::substance::Element::O);
    let drawn =
        inputs.inventory.carbon_dioxide().value() * 1e3 / Gas::CarbonDioxide.molar_mass_g_per_mol();
    let kept = drawn - out.graphite.value() * 1e3 / atomic(c);
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    let weight = crate::units::consts::GRAVITATIONAL_CONSTANT * m
        / (r * r)
        / (4.0 * core::f64::consts::PI * r * r);
    let shoreline = crate::planetary::chem::escape::Shoreline::of(
        inputs.xuv_fluence.value() / earth_xuv_fluence().value(),
        MetresPerSecond::new((2.0 * crate::units::consts::GRAVITATIONAL_CONSTANT * m / r).sqrt()),
        inputs.impact_speed,
        inputs.age,
    )
    .kept();
    let co_weight =
        kept * out.shares.carbon_monoxide * (atomic(c) + atomic(o)) * 1e-3 * weight * shoreline;
    let cap = VentGas::speciate(
        &VentInputs {
            delta_iw: reduced.delta_iw,
            temperature: reduced.temperature,
            pressure: out.pressure,
        },
        &crate::planetary::derive::atmosphere::vented::vent_elements(&inputs.inventory),
    )
    .expect("the reduced vent")
    .carbon_monoxide_at_graphite();
    assert!(
        relative(co_weight, cap.value()) < 1e-9,
        "{co_weight} Pa against {cap:?}"
    );
    assert!((cap.value() / 1.25e5 - 1.0).abs() < 0.02, "{cap:?}");
    let (_, _, temperate) = vented_earth(|drawn| *drawn, reduced);
    let out = vented_of("a temperate reduced Earth", &temperate);
    eprintln!("a temperate reduced Earth: {out:?}");
    assert_eq!(out.graphite, Kilograms::ZERO);
    assert!(out.holds_water && out.shifted > Kilograms::ZERO);
}

/// Ruling 1's methane: a reduced (IW−4), cool (1,000 K; O'Neill's IW, fitted over 1,184–1,450 K,
/// extrapolated below it) vent under a thick nitrogen air, thirty
/// times Earth's, makes CH₄, which the air publishes beside its CO₂, its hydrogen two waters a
/// methane. On a temperate world the weathering thermostat stores its carbon, the vent's methane
/// held on the carbon with the carbon dioxide (ruling 3's stand-in); beyond the maximum-greenhouse
/// limit, formed inside the snow line, its carbon is all airborne, and T24.c's methane haze reads
/// it by its index, x(CH₄) over x(CO₂).
#[test]
fn a_reduced_cool_thick_vent_makes_methane_from_the_water() {
    let (_, _, temperate) = vented_earth(
        |drawn| {
            VolatileInventory::new(
                drawn.water(),
                drawn.carbon_dioxide(),
                Kilograms::new(30.0 * drawn.nitrogen().value()),
                drawn.argon(),
            )
        },
        VentConditions {
            delta_iw: Dex::new(-4.0),
            temperature: Kelvin::new(1_000.0),
        },
    );
    let out = vented_of("a reduced thick Earth", &temperate);
    eprintln!(
        "a reduced thick Earth: {out:?}\n  {}",
        air_line("", &temperate)
    );
    assert!(escape_of(&temperate).airborne_carbon_share() < 1e-3);
    let (_, _, air) = vented_earth_at(
        230.0,
        Insolation::BeyondMaximumGreenhouse,
        |drawn| {
            VolatileInventory::new(
                drawn.water(),
                drawn.carbon_dioxide(),
                Kilograms::new(30.0 * drawn.nitrogen().value()),
                drawn.argon(),
            )
        },
        VentConditions {
            delta_iw: Dex::new(-4.0),
            temperature: Kelvin::new(1_000.0),
        },
    );
    let cold = vented_of("a cold reduced thick Earth", &air);
    eprintln!(
        "a cold reduced thick Earth: {cold:?}\n  {}, CH4 {:.4e} Pa",
        air_line("", &air),
        air.partial_pressures().of(Gas::Methane).value()
    );
    assert!(out.shares.methane > 0.5, "{:?}", out.shares);
    assert!(out.methane > Kilograms::ZERO);
    let p = air.partial_pressures();
    assert!(p.of(Gas::Methane) > Pascals::ZERO && p.of(Gas::CarbonDioxide) > Pascals::ZERO);
    let methane = out.methane.value() / Gas::Methane.molar_mass_g_per_mol();
    let shifted = out.shifted.value() / Gas::CarbonDioxide.molar_mass_g_per_mol();
    let debit = out.water_debit.value() / Gas::Water.molar_mass_g_per_mol();
    assert!(relative(debit, 2.0 * methane + shifted) < 1e-12);
    let index = crate::planetary::hooks::aerosols::haze_index(
        mole_fraction(&air, Gas::Methane),
        mole_fraction(&air, Gas::CarbonDioxide),
    );
    eprintln!("its haze index {index:.4}");
    assert!(index > 1.0, "{index}");
}

/// The balance: at 36 each element's outcome still sums to its inventory within 10⁻⁹ of the
/// largest, the graphite the crust holds and the oxygen the melt takes counted: over the table and
/// the hand-built dry, reduced and methane-making Earths.
#[test]
fn at_36_each_element_s_outcome_sums_to_its_inventory() {
    let check = |name: &str, air: &Atmosphere, inventory: &VolatileInventory, envelope: f64| {
        let expected = element_inventory_of(inventory, envelope);
        let largest = expected.iter().fold(0.0_f64, |a, &b| a.max(b));
        for (outcome, expected) in escape_of(air).elements().iter().zip(expected) {
            let sum = outcome.inventory().value();
            assert!(
                (sum - expected).abs() <= 1e-9 * largest,
                "{name}: {:?} sums to {sum} kg against {expected} kg: {outcome:?}",
                outcome.element()
            );
        }
    };
    for (name, body) in &table_at_36() {
        let formed = body.fractions().envelope() * Kilograms::from(body.mass()).value()
            + Kilograms::from(body.envelope_lost()).value();
        check(name, body.atmosphere(), body.inventory(), formed);
    }
    let dry = |drawn: &VolatileInventory| {
        VolatileInventory::new(
            Kilograms::ZERO,
            drawn.carbon_dioxide(),
            drawn.nitrogen(),
            drawn.argon(),
        )
    };
    let thick = |drawn: &VolatileInventory| {
        VolatileInventory::new(
            drawn.water(),
            drawn.carbon_dioxide(),
            Kilograms::new(30.0 * drawn.nitrogen().value()),
            drawn.argon(),
        )
    };
    let cool = VentConditions {
        delta_iw: Dex::new(-4.0),
        temperature: Kelvin::new(1_000.0),
    };
    for (name, (inputs, _, air)) in [
        (
            "a dry Earth",
            vented_earth(dry, VentConditions::basaltic(Dex::new(0.01))),
        ),
        (
            "a reduced Earth",
            vented_earth(|d| *d, VentConditions::basaltic(Dex::new(-5.0))),
        ),
        ("a reduced thick Earth", vented_earth(thick, cool)),
    ] {
        assert!(matches!(air.vent(), Some(VentRoute::Vented(_))), "{name}");
        check(name, &air, &inputs.inventory, 0.0);
    }
}

/// The vent's elements are the drawn carriers': H from the water, C from the carbon dioxide and N
/// from the nitrogen, mol of atoms, with no sulphur; their ratios set the shares.
#[test]
fn the_vent_s_elements_are_the_drawn_carriers() {
    let inventory = VolatileInventory::new(
        Kilograms::new(18.015_28),
        Kilograms::new(44.009_5),
        Kilograms::new(28.013_4),
        Kilograms::new(1.0),
    );
    let elements = crate::planetary::derive::atmosphere::vented::vent_elements(&inventory);
    let near = |got: f64, want: f64| (got / want - 1.0).abs() < 1e-3;
    assert!(near(elements.hydrogen_mol(), 2.0e3), "{elements:?}");
    assert!(near(elements.carbon_mol(), 1.0e3), "{elements:?}");
    assert!(near(elements.nitrogen_mol(), 2.0e3), "{elements:?}");
    assert_eq!(elements.sulphur_mol(), 0.0);
}

/// The route on the same inputs twice gives the same air and the same record, to the bit: the
/// table at 36, body by body (sim-determinism's same-twice test, for the held rules' other arm).
#[test]
fn at_36_the_table_is_the_same_twice() {
    for ((name, a), (_, b)) in table_at_36().iter().zip(&table_at_36()) {
        assert!(same_air(a.atmosphere(), b.atmosphere()), "{name}");
        assert_eq!(a.atmosphere().vent(), b.atmosphere().vent(), "{name}");
    }
}

/// An ice bulk in the habitable zone (a remnant's ice bulk, not a runaway) vents on its seafloor,
/// through an ocean not modelled: it keeps P14.T24.f's carriers, its air the held one to the bit.
#[test]
fn an_ice_bulk_keeps_its_carriers() {
    let inputs = AtmosphereInputs {
        insolation: Insolation::Habitable,
        present_insolation: Insolation::Habitable,
        ..remnant_ice_bulk(250.0)
    };
    let at_36 = with_every_36_hold(|| atmosphere(&inputs));
    assert_eq!(at_36.vent(), Some(VentRoute::IceShell));
    assert!(same_air(&atmosphere(&inputs), &at_36));
}

/// An oxidised mantle (IW+8) under so thin an air that its melt's f(O₂) reaches the vent's pressure
/// is refused by the vent's gas, and keeps P14.T24.f's carriers, to the bit.
#[test]
fn a_vent_its_gas_refuses_keeps_its_carriers() {
    let (_, held, air) = vented_earth(
        |drawn| {
            VolatileInventory::new(
                drawn.water(),
                Kilograms::new(drawn.carbon_dioxide().value() * 1e-12),
                Kilograms::new(drawn.nitrogen().value() * 1e-12),
                Kilograms::ZERO,
            )
        },
        VentConditions::basaltic(Dex::new(8.0)),
    );
    assert!(
        matches!(
            air.vent(),
            Some(VentRoute::Refused(
                SpeciateVentGasError::OxygenFugacity { .. }
            ))
        ),
        "{:?}",
        air.vent()
    );
    assert!(same_air(&held, &air));
}

/// Step 5 where Jeans's test retains hydrogen and the age is short against the diffusion limit: a
/// young (10⁴ yr), cold (100 K irradiation) super-Earth of 5 M⊕ keeps most of its shift's H₂,
/// which its air publishes beside the CO₂, each element's outcome still summing to its inventory.
#[test]
fn a_young_cold_super_earth_keeps_its_shift_s_hydrogen() {
    let mut inputs = earth_inputs(255.0, Insolation::Habitable);
    inputs.mass = Kilograms::new(5.0 * inputs.mass.value());
    inputs.radius = Metres::new(1.5 * inputs.radius.value());
    inputs.irradiation_temperature = Kelvin::new(100.0);
    inputs.age = Years::new(1e4);
    inputs.vent = VentConditions::basaltic(Dex::new(0.01));
    let air = with_every_36_hold(|| atmosphere(&inputs));
    let out = vented_of("a young cold super-Earth", &air);
    eprintln!("a young cold super-Earth: {out:?}");
    assert!(air.retention().retains(Gas::Hydrogen));
    assert!(out.hydrogen_kept > Kilograms::ZERO && out.hydrogen_kept < out.hydrogen);
    assert!(air.partial_pressures().of(Gas::Hydrogen) > Pascals::ZERO);
    let expected = element_inventory_of(&inputs.inventory, 0.0);
    let largest = expected.iter().fold(0.0_f64, |a, &b| a.max(b));
    for (outcome, expected) in escape_of(&air).elements().iter().zip(expected) {
        let sum = outcome.inventory().value();
        assert!(
            (sum - expected).abs() <= 1e-9 * largest,
            "{:?} sums to {sum} kg against {expected} kg",
            outcome.element()
        );
    }
}
