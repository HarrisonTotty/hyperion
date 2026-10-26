//! P14.T51.g.a's tests: a regular moon's published ocean ends at the shallower of its rock and ice
//! V's melting pressure at its 273 K base (decision-p14-t51g Q9), behind its hold.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::hooks::figures::{OceanFloor, Subsurface, SubsurfaceOcean};
use crate::planetary::moons::regular::{
    ICE_CONDUCTIVITY_COEFFICIENT, ICE_MELTING_POINT, ocean_layers,
};
use crate::substance::MeltingCurve;
use crate::units::WattsPerSquareMetre;

/// Europa as the flag's doctest gives it: 1,560.8 km at 3,013 kg m⁻³, 8% water, 102 K, 50 mW m⁻².
const EUROPA: (f64, f64, f64, f64, f64) = (1.560_8e6, 3_013.0, 0.08, 102.0, 0.05);

/// The ruling's Ganymede (decision-p14-t51g Q9): 2,634.1 km and 1.482 × 10²³ kg, 45% water, under
/// a surface at 110 K with 4.36 mW m⁻² from below, its density from its mass and radius.
const GANYMEDE: (f64, f64, f64, f64, f64) = (
    2.634_1e6,
    1.482e23 / (4.0 / 3.0 * core::f64::consts::PI * 2.634_1e6 * 2.634_1e6 * 2.634_1e6),
    0.45,
    110.0,
    0.004_36,
);

/// The moon `(radius, density, water, surface, flux)`'s ocean under the rules `rules`.
fn ocean_under(rules: OceanFloorRules, moon: (f64, f64, f64, f64, f64)) -> Subsurface {
    let (radius, density, water, surface, flux) = moon;
    with_ocean_floor(rules, || {
        Subsurface::of_regular_moon(
            Metres::new(radius),
            KilogramsPerCubicMetre::new(density),
            water,
            Kelvin::new(surface),
            WattsPerSquareMetre::new(flux),
        )
    })
}

/// The moon's ocean under `rules`, which must be one.
fn an_ocean(rules: OceanFloorRules, moon: (f64, f64, f64, f64, f64)) -> SubsurfaceOcean {
    match ocean_under(rules, moon) {
        Subsurface::Ocean(ocean) => ocean,
        other => panic!("an ocean under {rules:?}: {other:?}"),
    }
}

/// The moon's flag's water column.
fn column_of(moon: (f64, f64, f64, f64, f64)) -> WaterColumn {
    let (radius, density, water, surface, flux) = moon;
    let layers = ocean_layers(
        Metres::new(radius),
        KilogramsPerCubicMetre::new(density),
        water,
        Kelvin::new(surface),
        WattsPerSquareMetre::new(flux),
    )
    .expect("the moon has layers");
    WaterColumn::new(
        Metres::new(radius),
        KilogramsPerCubicMetre::new(density),
        layers.water_depth,
    )
}

/// The ocean floor is held until P14.T54.b's bump: below it every published ocean is 23's, and the
/// tests' switch reaches 24's ahead of it.
#[test]
fn the_ocean_floor_is_held_only_until_the_24_bump() {
    assert!(
        GENERATOR_VERSION < OCEAN_FLOOR_VERSION,
        "the 23 → 24 bump (P14.T54.b) puts P14.T51.g.a on the record: a regular moon's published \
         ocean ends at the shallower of its rock and ice V's melting pressure at its 273 K base, \
         and is not modelled where its shell's base lies beyond it. Delete `OCEAN_FLOOR_VERSION`, \
         `OceanFloorRules` with its `Held` arm (`Subsurface::of_regular_moon`'s `Held` return and \
         the `Held` arm of `own_tides`' debug assertion), `with_ocean_floor` and its thread's \
         rules, the census's held twin and rock-floor check (`system/moon_ice/lag_tests.rs`), the \
         server test's version guard (`the_members_of_later_tasks_are_absent_and_a_moon_s_ocean_\
         is_computed`), and this test; and let system/tests.rs's two \"a regular moon's ocean is \
         computed\" assertions take the absent case. The column, the liquidus and `ocean_end` \
         stay, since they are the rule, and so does the lag's gate on the ocean branch \
         (`ocean_branch_open`), which rides `MOON_BULK_VERSION`"
    );
    assert_eq!(OceanFloorRules::in_force(), OceanFloorRules::Held);
    assert_eq!(
        with_ocean_floor(OceanFloorRules::Floored, OceanFloorRules::in_force),
        OceanFloorRules::Floored
    );
    assert_eq!(OceanFloorRules::in_force(), OceanFloorRules::Held);
}

/// The liquidus is the registry's: water's melting pressure at the ocean's 273 K base is ice V's
/// 626.09 MPa, and 629.34 MPa at the triple point's 273.16 K (IAPWS R14-08, its Eq. 3 for ice V:
/// 350.1 MPa × [1 − 1.187 21 (1 − (T ÷ 256.164 K)⁸)]; decision-p14-t51g Q6), on the piece of the
/// curve that starts at ice V's III–V–liquid point, 256.164 K, and ends at its V–VI–liquid point,
/// 273.31 K. So the floor the liquidus gives is ice V.
#[test]
fn the_base_s_liquidus_is_ice_v_s_melting_pressure() {
    let ice_v = |t: f64| 350.1e6 * (1.0 - 1.187_21 * (1.0 - crate::math::powi(t / 256.164, 8)));
    for (t, mpa) in [(273.0, 626.09), (273.16, 629.34)] {
        let p = base_liquidus(Kelvin::new(t)).value();
        assert!((p / ice_v(t) - 1.0).abs() < 1e-12, "{t} K: {p} Pa");
        assert!((p / 1e6 - mpa).abs() < 0.005, "{t} K: {p} Pa");
    }
    let water = Gas::Water.substance().substance().phase_diagram().unwrap();
    let Some(MeltingCurve::Falling { rising, .. }) = water.melting() else {
        panic!("water's curve falls from its triple point");
    };
    let at = ICE_MELTING_POINT;
    let index = rising
        .iter()
        .rposition(|piece| at >= piece.start())
        .expect("a piece holds 273 K");
    assert_same_bits(rising[index].start().value(), 256.164);
    assert_same_bits(rising[index + 1].start().value(), 273.31);
}

/// P(r) is the hydrostatic column's: 0 at the surface, falling monotonically from the base, the
/// uniform sphere's (2π/3) G `ρ_w`² (R² − r²) where the body is all water, and its slope −`ρ_w` g(r)
/// with g(r) the gravity of the mass inside r.
#[test]
fn the_column_s_pressure_is_hydrostatic() {
    let radius = 2.0e6;
    let rho = WATER_DENSITY.value();
    let all_water = WaterColumn::new(Metres::new(radius), WATER_DENSITY, Metres::new(radius));
    assert_same_bits(all_water.base().value(), LEAST_BASE_SHARE * radius);
    for share in [0.001, 0.1, 0.5, 0.9, 1.0] {
        let r = share * radius;
        let uniform = 2.0 * core::f64::consts::PI / 3.0
            * GRAVITATIONAL_CONSTANT
            * rho
            * rho
            * (radius * radius - r * r);
        let p = all_water.pressure_at(Metres::new(r)).value();
        assert!(
            (p - uniform).abs() <= 1e-9 * uniform.max(1.0),
            "{share}: {p} against {uniform}"
        );
    }
    let column = column_of(GANYMEDE);
    let (big_r, mass) = (column.radius().value(), column.mass().value());
    let mut last = f64::INFINITY;
    for k in 0..=100 {
        let r = column.base().value() + (big_r - column.base().value()) * f64::from(k) / 100.0;
        let p = column.pressure_at(Metres::new(r)).value();
        assert!(p < last, "{r} m: {p} Pa");
        last = p;
        if k > 0 && k < 100 {
            let h = 1.0;
            let slope = (column.pressure_at(Metres::new(r + h)).value()
                - column.pressure_at(Metres::new(r - h)).value())
                / (2.0 * h);
            let inside = mass
                - 4.0 / 3.0
                    * core::f64::consts::PI
                    * rho
                    * (crate::math::powi(big_r, 3) - crate::math::powi(r, 3));
            let g = GRAVITATIONAL_CONSTANT * inside / (r * r);
            assert!(
                (slope / (-rho * g) - 1.0).abs() < 1e-6,
                "{r} m: {slope} against {}",
                -rho * g
            );
        }
    }
    assert!(column.pressure_at(Metres::new(big_r)).value().abs() < 1e-6);
}

/// The radius at a pressure is found by the fixed count of halvings to the column's own
/// resolution: its pressure is the asked one to 10⁻¹², and never above it.
#[test]
fn the_radius_at_a_pressure_is_found_by_fixed_halvings() {
    let column = column_of(GANYMEDE);
    let top = column.radius();
    for mpa in [1.0, 100.0, 626.09, 2_000.0] {
        let p = Pascals::new(mpa * 1e6);
        let r = column.radius_at(p, column.base(), top);
        let at = column.pressure_at(r).value();
        assert!(at <= p.value(), "{mpa} MPa: {at} Pa");
        assert!((at / p.value() - 1.0).abs() < 1e-12, "{mpa} MPa: {at} Pa");
    }
}

/// The doctest's Europa keeps its ocean on rock, to the bit: its rock lies at 189 MPa, under ice
/// V's liquidus, so the whole layer below its 12.8 km shell is liquid, 124.3 km deep.
#[test]
fn europa_keeps_its_whole_ocean_on_rock() {
    let held = an_ocean(OceanFloorRules::Held, EUROPA);
    let floored = an_ocean(OceanFloorRules::Floored, EUROPA);
    assert_eq!(floored, held);
    assert_eq!(floored.floor(), OceanFloor::Rock);
    assert!(
        (floored.thickness().value() / 1e3 - 124.28).abs() < 0.01,
        "{floored:?}"
    );
    let column = column_of(EUROPA);
    let rock = column.pressure_at(column.base()).value();
    assert!((rock / 1e6 - 189.0).abs() < 1.0, "{rock} Pa");
}

/// The ruling's Ganymede: under 23's rule 1,167.8 km of water on rock under its 135.7 km shell, to
/// 2.5 GPa; under the floor it ends on ice V 280–285 km below its shell (the prototype's 282.5
/// km), at ice V's melting pressure to 10⁻⁹, its shell and its other members unchanged, its mass
/// at most its water's and its depth within its layer.
#[test]
fn a_ganymede_s_ocean_ends_on_ice_v() {
    let held = an_ocean(OceanFloorRules::Held, GANYMEDE);
    let floored = an_ocean(OceanFloorRules::Floored, GANYMEDE);
    eprintln!("Ganymede: 23's {held:?}\n  24's {floored:?}");
    let shell = ICE_CONDUCTIVITY_COEFFICIENT * crate::math::ln(273.0 / GANYMEDE.3) / GANYMEDE.4;
    assert_same_bits(held.shell_thickness().value(), shell);
    assert!((shell / 1e3 - 135.7).abs() < 0.05, "{shell}");
    assert_eq!(held.floor(), OceanFloor::Rock);
    assert!(
        (held.thickness().value() / 1e3 - 1_167.8).abs() < 0.1,
        "{held:?}"
    );
    assert_eq!(floored.floor(), OceanFloor::IceV);
    let km = floored.thickness().value() / 1e3;
    assert!((280.0..=285.0).contains(&km), "{km} km");
    assert_same_bits(
        floored.shell_thickness().value(),
        held.shell_thickness().value(),
    );
    assert_eq!(floored.liquid(), held.liquid());
    assert_same_bits(floored.solute_mass_fraction(), 0.0);
    assert_eq!(floored.base_temperature(), ICE_MELTING_POINT);
    let column = column_of(GANYMEDE);
    let r = GANYMEDE.0;
    let floor = r - floored.shell_thickness().value() - floored.thickness().value();
    let p_f = base_liquidus(ICE_MELTING_POINT).value();
    let at = column.pressure_at(Metres::new(floor)).value();
    assert!((at / p_f - 1.0).abs() < 1e-9, "{at} against {p_f} Pa");
    let rock = column.pressure_at(column.base()).value();
    assert!((rock / 1e6 - 2_501.0).abs() < 5.0, "{rock} Pa");
    let top = r - floored.shell_thickness().value();
    let ocean_mass = 4.0 / 3.0
        * core::f64::consts::PI
        * WATER_DENSITY.value()
        * (crate::math::powi(top, 3) - crate::math::powi(floor, 3));
    let mass = 1.482e23;
    assert!(ocean_mass <= GANYMEDE.2 * mass, "{ocean_mass} kg");
    assert!(floor >= column.base().value());
}

/// Where the shell's base itself lies at or beyond ice V's liquidus, the ocean is not modelled
/// (P14.T55.k's liquid under high-pressure ice): the ruling's Ganymede with less heat from below.
/// Its ice-V ocean thins to the edge, where the shell's base reaches the liquidus, and beyond it
/// the member is absent; under 23's rule the same moon keeps its ocean on rock. The flag holds an
/// ocean throughout.
#[test]
fn an_ocean_under_a_shell_past_ice_v_is_not_modelled() {
    let column = column_of(GANYMEDE);
    let p_f = base_liquidus(ICE_MELTING_POINT);
    let r_f = column.radius_at(p_f, column.base(), column.radius());
    let edge_shell = column.radius().value() - r_f.value();
    let edge = ICE_CONDUCTIVITY_COEFFICIENT * crate::math::ln(273.0 / GANYMEDE.3) / edge_shell;
    eprintln!(
        "Ganymede's edge: a shell of {:.1} km at {:.3} mW m⁻²",
        edge_shell / 1e3,
        edge * 1e3
    );
    let at = |flux: f64| (GANYMEDE.0, GANYMEDE.1, GANYMEDE.2, GANYMEDE.3, flux);
    let near = an_ocean(OceanFloorRules::Floored, at(edge * 1.000_1));
    assert_eq!(near.floor(), OceanFloor::IceV);
    assert!(
        near.thickness().value() < 2e-4 * near.shell_thickness().value(),
        "{near:?}"
    );
    for flux in [edge * 0.999_9, edge * 0.5] {
        assert_eq!(
            ocean_under(OceanFloorRules::Floored, at(flux)),
            Subsurface::NotComputed,
            "{flux} W m⁻²"
        );
        let none = with_ocean_floor(OceanFloorRules::Floored, || {
            SubsurfaceOcean::of_regular_moon(
                Metres::new(GANYMEDE.0),
                KilogramsPerCubicMetre::new(GANYMEDE.1),
                GANYMEDE.2,
                Kelvin::new(GANYMEDE.3),
                WattsPerSquareMetre::new(flux),
            )
        });
        assert_eq!(none, None);
        assert_eq!(
            an_ocean(OceanFloorRules::Held, at(flux)).floor(),
            OceanFloor::Rock
        );
    }
}

/// The floored ocean pinned to the bit (the determinism review): the ruling's Ganymede, whose
/// floor the 64 halvings find, its shell and 23's depth with it, and its absent case, which no
/// golden prints until the bump. A change to the column's arithmetic, the halvings or the liquidus
/// moves these bits.
#[test]
fn the_floored_ocean_is_pinned_to_the_bit() {
    let held = an_ocean(OceanFloorRules::Held, GANYMEDE);
    let floored = an_ocean(OceanFloorRules::Floored, GANYMEDE);
    assert_same_bits(
        floored.shell_thickness().value(),
        f64::from_bits(0x4100_915a_17bc_c186),
    );
    assert_same_bits(
        floored.thickness().value(),
        f64::from_bits(0x4111_3d6d_ef20_62c0),
    );
    assert_eq!(floored.floor(), OceanFloor::IceV);
    assert_same_bits(
        held.thickness().value(),
        f64::from_bits(0x4131_d1cb_f86e_b353),
    );
    let (r, rho, w, t, f) = GANYMEDE;
    assert_eq!(
        ocean_under(OceanFloorRules::Floored, (r, rho, w, t, f * 0.25)),
        Subsurface::NotComputed
    );
}

/// Where the flag holds no ocean the floor holds none either: no water, too little heat for the
/// layer, no heat from below, or a surface at the melting point.
#[test]
fn the_flag_s_none_is_none_under_both_rules() {
    let (r, rho, w, t, f) = EUROPA;
    for moon in [
        (r, rho, 0.0, t, f),
        (r, rho, w, t, 1e-4),
        (r, rho, w, t, 0.0),
        (r, rho, w, 273.0, f),
        (r, rho, w, t, f64::NAN),
    ] {
        for rules in [OceanFloorRules::Held, OceanFloorRules::Floored] {
            assert_eq!(
                ocean_under(rules, moon),
                Subsurface::NoOcean,
                "{moon:?} {rules:?}"
            );
        }
    }
}

/// The end of an ocean in a column, from each side of its three cases: on the rock where its base
/// lies at or under the liquidus, on the ice where the pressure reaches it below the shell, and
/// not modelled where the shell's base is at or past it or at a pressure that is not a number.
#[test]
fn an_ocean_ends_at_the_shallower_of_its_rock_and_its_liquidus() {
    let column = column_of(GANYMEDE);
    let rock = column.pressure_at(column.base());
    let shell_base = Metres::new(column.radius().value() - 135.7e3);
    let at_shell = column.pressure_at(shell_base);
    assert_eq!(ocean_end(&column, shell_base, rock), OceanEnd::Rock);
    assert_eq!(
        ocean_end(&column, shell_base, Pascals::new(rock.value() * 2.0)),
        OceanEnd::Rock
    );
    let OceanEnd::Ice(floor) = ocean_end(&column, shell_base, Pascals::new(626.09e6)) else {
        panic!("Ganymede's ocean ends on ice");
    };
    assert!(floor > column.base() && floor < shell_base, "{floor:?}");
    assert_eq!(ocean_end(&column, shell_base, at_shell), OceanEnd::Beyond);
    assert_eq!(
        ocean_end(&column, Metres::new(f64::NAN), Pascals::new(626.09e6)),
        OceanEnd::Beyond
    );
}
