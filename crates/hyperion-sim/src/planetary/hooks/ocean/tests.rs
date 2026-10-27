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

// P14.T51.g.b's tests: every body's ocean by the solve (decision-p14-t51g), its inputs by hand,
// chlorine at CI chondrites' per unit rock and ammonia at `ACCRETED_AMMONIA` where the body formed
// beyond ammonia hydrate's line; the observational figures are the plan's, from memory.

use crate::planetary::chem::inventory::CI_CHLORINE_PER_MASS;
use crate::planetary::derive::atmosphere::{
    EARTH_NITROGEN_PER_MASS, VolatileDraws, volatile_inventory,
};
use crate::planetary::derive::composition::{MassFractions, SnowLineSide};
use crate::planetary::hooks::crust::AmmoniaIce;
use crate::substance::by_key;
use crate::units::EarthMasses;
use crate::units::consts::EARTH_MASS_KG;

/// A body as the plan's tests give it by hand: its mean radius, km, its mass, kg, its water's mass
/// fraction, its surface's temperature, K, and whether it formed beyond ammonia hydrate's line;
/// its chlorine at CI chondrites' per unit of its rock, or `chlorine_per_body` of its whole mass.
#[derive(Debug, Clone, Copy)]
struct ByHand {
    radius_km: f64,
    mass: f64,
    water: f64,
    surface: f64,
    ammonia: bool,
    chlorine_per_body: Option<f64>,
}

impl ByHand {
    const fn of(radius_km: f64, mass: f64, water: f64, surface: f64, ammonia: bool) -> Self {
        Self {
            radius_km,
            mass,
            water,
            surface,
            ammonia,
            chlorine_per_body: None,
        }
    }

    fn radius(&self) -> Metres {
        Metres::new(self.radius_km * 1e3)
    }

    fn area(&self) -> f64 {
        let r = self.radius().value();
        4.0 * core::f64::consts::PI * r * r
    }

    fn solutes(&self) -> OceanSolutes {
        let chlorine = self.chlorine_per_body.map_or(
            CI_CHLORINE_PER_MASS * (1.0 - self.water) * self.mass,
            |share| share * self.mass,
        );
        let cl = Element::Cl.standard_atomic_weight_g_per_mol();
        let salt = Element::Na.standard_atomic_weight_g_per_mol() + cl;
        OceanSolutes {
            sodium_chloride: Kilograms::new(chlorine * salt / cl),
            ammonia_per_water: if self.ammonia { ACCRETED_AMMONIA } else { 0.0 },
        }
    }

    /// Its inputs under the heat flow `flux`, W m⁻².
    fn at(&self, flux: f64) -> OceanInputs {
        OceanInputs {
            radius: self.radius(),
            mass: Kilograms::new(self.mass),
            water_fraction: self.water,
            surface_temperature: Kelvin::new(self.surface),
            heat_flow: WattsPerSquareMetre::new(flux),
            solutes: self.solutes(),
        }
    }

    fn ocean(&self, flux: f64) -> Subsurface {
        subsurface_ocean(&self.at(flux))
    }

    /// Its ocean under `flux`, which must be one and plausible, printed for the record.
    fn an_ocean(&self, name: &str, flux: f64) -> SubsurfaceOcean {
        let Subsurface::Ocean(ocean) = self.ocean(flux) else {
            panic!("{name} at {flux} W m⁻²: {:?}", self.ocean(flux));
        };
        assert_eq!(implausibility(&self.at(flux), &ocean), None, "{name}");
        eprintln!(
            "{name} at {:.3} mW m⁻²: under {:.1} km, {:.1} km thick on {:?}, {}, solutes {:.2} g/kg, \
             base {:.2} K",
            flux * 1e3,
            ocean.shell_thickness().value() / 1e3,
            ocean.thickness().value() / 1e3,
            ocean.floor(),
            ocean.liquid().substance().key().as_str(),
            ocean.solute_mass_fraction() * 1e3,
            ocean.base_temperature().value()
        );
        ocean
    }
}

/// The plan's bodies (P14.T51.g.b's tests).
mod solar {
    use super::ByHand;

    pub(super) const EUROPA: ByHand = ByHand::of(1_560.8, 4.80e22, 0.08, 102.0, false);
    pub(super) const GANYMEDE: ByHand = ByHand::of(2_634.1, 1.482e23, 0.45, 110.0, false);
    pub(super) const CALLISTO: ByHand = ByHand::of(2_410.3, 1.076e23, 0.45, 125.0, true);
    pub(super) const TITAN: ByHand = ByHand::of(2_574.7, 1.345e23, 0.42, 93.7, true);
    pub(super) const ENCELADUS: ByHand = ByHand::of(252.1, 1.08e20, 0.40, 75.0, true);
    pub(super) const PLUTO: ByHand = ByHand::of(1_188.3, 1.303e22, 0.32, 40.0, true);
    pub(super) const TRITON: ByHand = ByHand::of(1_353.4, 2.139e22, 0.28, 38.0, true);
    pub(super) const CERES: ByHand = ByHand::of(469.7, 9.39e20, 0.25, 160.0, false);
    pub(super) const RHEA: ByHand = ByHand::of(763.8, 2.307e21, 0.62, 76.0, true);
    /// A snowball Earth: Earth's radius and mass, its water's 2.3 × 10⁻⁴, a 230 K surface and the
    /// bulk silicate Earth's chlorine, 17 ppm of the body.
    pub(super) const SNOWBALL_EARTH: ByHand = ByHand {
        chlorine_per_body: Some(17e-6),
        ..ByHand::of(6_371.0, 5.972e24, 2.3e-4, 230.0, false)
    };
}

/// The rock's radiogenic heat flow of `body` today, W m⁻²: the bulk silicate Earth's 4.9 × 10⁻¹²
/// W kg⁻¹ of rock (science-p14-subsurface-ocean §3).
fn radiogenic(body: &ByHand) -> f64 {
    4.9e-12 * (1.0 - body.water) * body.mass / body.area()
}

/// The ocean is held until its bump: below it every body's ocean is version 23's, and the tests'
/// switch reaches the solve ahead of it.
#[test]
fn the_ocean_is_held_only_until_its_bump() {
    assert!(
        GENERATOR_VERSION < OCEAN_VERSION,
        "P14.T51.g.b's bump (24 with P14.T54.b, or 25) puts every body's subsurface ocean on the \
         record by the solve, and T56.d's ocean branch on its spherical shell. Delete \
         `OCEAN_VERSION`, `OceanRules` with its `Held` arms (`MoonTides::with_ocean`'s, with \
         `on_an_ocean`, the planar `shell_stiffness` and `ocean_heat_flow` arm it reads and 23's \
         `shell_modulus` to 273 K; `own_tides`' flag arm and its debug assertion's; `moon_ocean`'s; \
         `published_surface`'s), `with_ocean_rules` and its thread's rules, its arm in \
         `with_every_24_hold`, and this test; with them P14.T17.b's flag, `has_subsurface_ocean`, \
         `ocean_layers` and `OceanLayers`, `SubsurfaceOcean::of_regular_moon`, \
         `Subsurface::of_regular_moon` and `DerivedMoon::subsurface_ocean`, which the plan says go, \
         and P14.T51.g.a's floored arm (`OCEAN_FLOOR_VERSION`'s tripwire). Let system/tests.rs's \
         two \"a regular moon's ocean is computed\" assertions read every body's, and the server \
         test's version guard (`the_members_of_later_tasks_are_absent_and_a_moon_s_ocean_is_\
         computed`) every kind's"
    );
    assert_eq!(OceanRules::in_force(), OceanRules::Held);
    assert_eq!(
        with_ocean_rules(OceanRules::Solved, OceanRules::in_force),
        OceanRules::Solved
    );
    assert_eq!(OceanRules::in_force(), OceanRules::Held);
}

/// The ammonia's nitrogen multiple is the volatile inventory's own, to the bit, on a grid of
/// ranks (decision-p14-t51g's files: the helper lives outside `derive/atmosphere*`).
#[test]
fn the_nitrogen_multiple_is_the_inventory_s() {
    let fractions = MassFractions::solid(0.325, 0.675, 0.0);
    for step in 1..100_u32 {
        let rank = UnitUniform::new(f64::from(step) / 100.0).unwrap();
        let draws = VolatileDraws {
            nitrogen: rank,
            ..VolatileDraws::MEDIAN
        };
        let mass = EarthMasses::new(0.7);
        let inventory = volatile_inventory(
            mass,
            &fractions,
            SnowLineSide::Inside,
            Kelvin::new(300.0),
            &draws,
            crate::units::Years::new(4.5e9),
        );
        let kg = mass.value() * EARTH_MASS_KG;
        let expected = EARTH_NITROGEN_PER_MASS * 1.0 * nitrogen_multiple(rank) * kg;
        assert_same_bits(inventory.nitrogen().value(), expected);
    }
    // A body whose ice carries no ammonia dissolves none; one that does takes the multiple.
    let rank = UnitUniform::new(0.9).unwrap();
    let chlorine = Kilograms::new(1e18);
    let none = OceanSolutes::of_body(chlorine, AmmoniaIce::Absent, rank);
    let some = OceanSolutes::of_body(chlorine, AmmoniaIce::Carried, rank);
    assert_same_bits(none.ammonia_per_water, 0.0);
    assert_same_bits(
        some.ammonia_per_water,
        ACCRETED_AMMONIA * nitrogen_multiple(rank),
    );
    assert!((some.sodium_chloride.value() / 1e18 - 58.44 / 35.45).abs() < 1e-3);
}

/// ∫k dT is the plan's 651 ln(T ÷ `T_s`) from a top above 30 K, linear below it, and its inverse
/// returns the temperature it was taken to; from an unlit 0 K surface it is finite.
#[test]
fn the_shell_s_temperature_inverts_its_conduction() {
    for (t_s, t) in [
        (100.0, 260.0),
        (40.0, 250.0),
        (10.0, 25.0),
        (10.0, 270.0),
        (0.0, 251.0),
    ] {
        let x = conducted_w_per_m(t_s, t);
        assert!(x.is_finite() && x > 0.0, "{t_s} → {t}: {x}");
        let back = conducted_temperature_k(t_s, x);
        assert!((back / t - 1.0).abs() < 1e-12, "{t_s} → {t}: {back}");
    }
    assert_same_bits(
        conducted_w_per_m(100.0, 260.0),
        651.0 * crate::math::ln(2.6),
    );
    assert!((conducted_w_per_m(10.0, 25.0) - 651.0 * 15.0 / 30.0).abs() < 1e-9);
}

/// Europa (1,560.8 km, 4.80 × 10²² kg, 8% water, 102 K, 50 mW m⁻²) holds a brine on its rock
/// under 10–16 km of ice (the prototype's 12.6 km, 124.5 km deep, 14.5 g kg⁻¹, 271.0 K).
#[test]
fn europa_holds_a_brine_on_rock_under_its_shell() {
    let ocean = solar::EUROPA.an_ocean("Europa", 0.05);
    let shell = ocean.shell_thickness().value() / 1e3;
    assert!((10.0..=16.0).contains(&shell), "{shell} km");
    assert_eq!(ocean.floor(), OceanFloor::Rock);
    assert_eq!(Some(ocean.liquid()), by_key("brine_nacl"));
}

/// Ganymede (2,634.1 km, 1.482 × 10²³ kg, 45%, 110 K, 4.6 mW m⁻², no NH₃) holds its ocean under
/// 95–135 km of ice, on ice V, a brine (the prototype's 114.5 km, 130.8 km thick).
#[test]
fn ganymede_s_ocean_lies_on_ice_v() {
    let ocean = solar::GANYMEDE.an_ocean("Ganymede", 0.004_6);
    let shell = ocean.shell_thickness().value() / 1e3;
    assert!((95.0..=135.0).contains(&shell), "{shell} km");
    assert_eq!(ocean.floor(), OceanFloor::IceV);
    assert_eq!(Some(ocean.liquid()), by_key("brine_nacl"));
}

/// Callisto (2,410.3 km, 1.076 × 10²³ kg, 45%, 125 K, 4.0 mW m⁻², NH₃) holds an ammonia ocean
/// under 95–135 km of ice, on ice V (the prototype's 112.0 km, 201.4 km thick).
#[test]
fn callisto_s_ammonia_ocean_lies_on_ice_v() {
    let ocean = solar::CALLISTO.an_ocean("Callisto", 0.004);
    let shell = ocean.shell_thickness().value() / 1e3;
    assert!((95.0..=135.0).contains(&shell), "{shell} km");
    assert_eq!(ocean.floor(), OceanFloor::IceV);
    assert_eq!(Some(ocean.liquid()), by_key("nh3_h2o"));
}

/// Titan (2,574.7 km, 1.345 × 10²³ kg, 42%, 93.7 K, NH₃) holds its ocean under 110–150 km of ice
/// at its radiogenic 4.6 mW m⁻² and 55–80 km at T56.d's heated 9.9, both inside Mitri et al.
/// 2014's 50–200 km (the prototype's 130.8 km on ice III and 66.2 km on ice V).
#[test]
fn titan_s_shell_lies_inside_mitri_s_range() {
    let cold = solar::TITAN.an_ocean("Titan", 0.004_6);
    let warm = solar::TITAN.an_ocean("Titan", 0.009_9);
    let (cold, warm) = (
        cold.shell_thickness().value() / 1e3,
        warm.shell_thickness().value() / 1e3,
    );
    assert!((110.0..=150.0).contains(&cold), "{cold} km");
    assert!((55.0..=80.0).contains(&warm), "{warm} km");
}

/// Enceladus (252.1 km, 1.08 × 10²⁰ kg, 40%, 75 K, NH₃) holds an ocean on its rock under 20–35 km
/// of ice at 22 GW of tidal heat (P14.T56.d's; the prototype's 27.2 km, 46.2 km thick), and none
/// at its rock's radiogenic heat alone.
#[test]
fn enceladus_holds_an_ocean_at_its_tidal_heat_alone() {
    let body = solar::ENCELADUS;
    let tidal = 22e9 / body.area();
    let ocean = body.an_ocean("Enceladus", tidal);
    let shell = ocean.shell_thickness().value() / 1e3;
    assert!((20.0..=35.0).contains(&shell), "{shell} km");
    assert_eq!(ocean.floor(), OceanFloor::Rock);
    assert_eq!(body.ocean(radiogenic(&body)), Subsurface::NoOcean);
}

/// The least heat flow at which `body` publishes an ocean, W m⁻², between `cold` (none) and `warm`
/// (an ocean), by 60 halvings, with the thickness it publishes there and what it publishes just
/// under it: the edge that ends it.
fn threshold(body: &ByHand, cold: f64, warm: f64) -> (f64, f64, Subsurface) {
    let (mut lo, mut hi) = (cold, warm);
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if matches!(body.ocean(mid), Subsurface::Ocean(_)) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let Subsurface::Ocean(ocean) = body.ocean(hi) else {
        panic!("an ocean at the threshold's warm end");
    };
    (hi, ocean.thickness().value(), body.ocean(lo))
}

/// Pluto (1,188.3 km, 1.303 × 10²² kg, 32%, 40 K, NH₃) is absent at its rock's production, 2.45
/// mW m⁻² (its base in ice II); at 3.09 it holds an ocean 15–35 km thick under 270–300 km of ice,
/// on rock, of ammonia water, and at 3.5 one of 35–55 km under 250–280 km (Hammond et al. 2016's
/// warm state, 50 km under 250 km). Its threshold, the least heat flow at which it publishes an
/// ocean, is stated with its last thickness and the edge that ends it (decision-p14-t51g Q5): the
/// plan's 2.84 ± 0.10 mW m⁻² is where its base reaches ice II's field, and its water clears that
/// field to its floor on the rock only from 3.03 (the review's ice-II floor, recorded).
#[test]
fn pluto_is_absent_at_its_production_and_holds_oceans_at_its_lag() {
    let body = solar::PLUTO;
    assert_eq!(body.ocean(0.002_45), Subsurface::NotComputed);
    let lagged = body.an_ocean("Pluto", 0.003_09);
    let (shell, thick) = (
        lagged.shell_thickness().value() / 1e3,
        lagged.thickness().value() / 1e3,
    );
    assert!((15.0..=35.0).contains(&thick), "{thick} km");
    assert!((270.0..=300.0).contains(&shell), "{shell} km");
    assert_eq!(lagged.floor(), OceanFloor::Rock);
    assert_eq!(Some(lagged.liquid()), by_key("nh3_h2o"));
    let warm = body.an_ocean("Pluto", 0.003_5);
    let (shell, thick) = (
        warm.shell_thickness().value() / 1e3,
        warm.thickness().value() / 1e3,
    );
    assert!((35.0..=55.0).contains(&thick), "{thick} km");
    assert!((250.0..=280.0).contains(&shell), "{shell} km");
    let (least, at_least, under) = threshold(&body, 0.002_45, 0.003_09);
    eprintln!(
        "Pluto's threshold: {:.4} mW m⁻², its last ocean {:.1} km thick, under it {under:?}",
        least * 1e3,
        at_least / 1e3
    );
    // The plan's 2.84 ± 0.10 is where the base enters ice II's field; the whole ocean's water,
    // its floor included, leaves it only from 3.03 (the review's ice-II floor, recorded).
    assert!(
        (least * 1e3 - 3.028).abs() <= 0.01,
        "{} mW m⁻²",
        least * 1e3
    );
    assert_eq!(under, Subsurface::NotComputed, "the ice-II edge ends it");
}

/// Triton (1,353.4 km, 2.139 × 10²² kg, 28%, 38 K, NH₃) is absent at its rock's production, 3.28
/// mW m⁻², its base in ice II. The plan has it hold an ocean at 4.03 under 225–260 km of ice (the
/// prototype's 242.2 km, 24.2 km thick on ice III); there its 236.0 K ammonia water reaches 224
/// MPa, inside ice II's field (its warm edge 241.3 K there), so its floor would be ice II, which
/// meets no liquid and which the floors do not name: it is absent too (the review's ice-II floor,
/// recorded). The least heat flow at which it publishes an ocean is stated with what ends it.
#[test]
fn triton_is_absent_at_its_production_and_at_its_lag_its_floor_in_ice_ii() {
    let body = solar::TRITON;
    assert_eq!(body.ocean(0.003_28), Subsurface::NotComputed);
    assert_eq!(body.ocean(0.004_03), Subsurface::NotComputed);
    let (least, at_least, under) = threshold(&body, 0.003_28, 0.010);
    eprintln!(
        "Triton's threshold: {:.4} mW m⁻², its last ocean {:.1} km thick, under it {under:?}",
        least * 1e3,
        at_least / 1e3
    );
}

/// Ceres (469.7 km, 9.39 × 10²⁰ kg, 25%, 160 K, 1.24 mW m⁻², no NH₃) and Rhea (763.8 km, 2.307 ×
/// 10²¹ kg, 62%, 76 K, 0.59 mW m⁻²) hold none.
#[test]
fn ceres_and_rhea_hold_none() {
    assert_eq!(solar::CERES.ocean(0.001_24), Subsurface::NoOcean);
    assert_eq!(solar::RHEA.ocean(0.000_59), Subsurface::NoOcean);
}

/// A snowball Earth (Earth's R and M, 2.3 × 10⁻⁴ water, 230 K, 87 mW m⁻², the bulk silicate Earth's
/// chlorine at 17 ppm) keeps a sea under 0.5–1.5 km of ice (the prototype's 0.9 km, 1.8 km deep).
#[test]
fn a_snowball_earth_keeps_a_sea_under_its_ice() {
    let ocean = solar::SNOWBALL_EARTH.an_ocean("a snowball Earth", 0.087);
    let shell = ocean.shell_thickness().value() / 1e3;
    assert!((0.5..=1.5).contains(&shell), "{shell} km");
}

/// A deterministic stream of uniform draws in [0, 1) for the property tests (`SplitMix64`).
struct Draws(u64);

impl Draws {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        f64::from(u32::try_from(z >> 32).unwrap()) / 4_294_967_296.0
    }

    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }
}

/// A random body as the science ruling's `checks.py` draws them: 10⁻⁵–10 Earth masses, a dry or
/// a wet water fraction, a radius by mass, a 15–272 K surface and 0.1–316 mW m⁻², chlorine and
/// ammonia over wide ranges, either absent.
fn random_body(draws: &mut Draws) -> OceanInputs {
    let mass = crate::math::exp10(draws.between(-5.0, 1.0)) * EARTH_MASS_KG;
    let water = if draws.uniform() < 0.5 {
        draws.between(1e-4, 0.01)
    } else {
        draws.between(0.05, 0.54)
    };
    let radius = if mass > 6e22 {
        6.371e6 * crate::math::powf(mass / EARTH_MASS_KG, 0.27) * (1.0 + 0.5 * water)
    } else {
        let density = 1.0 / ((1.0 - water) / 3_300.0 + water / 940.0);
        crate::math::cbrt(3.0 * mass / (4.0 * core::f64::consts::PI * density))
    };
    let surface = draws.between(15.0, 272.0);
    let flux = crate::math::exp10(draws.between(-4.0, -0.5));
    let salt = if draws.uniform() < 0.7 {
        crate::math::exp10(draws.between(-6.0, -3.0)) * mass
    } else {
        0.0
    };
    let ammonia = if draws.uniform() < 0.4 {
        crate::math::exp10(draws.between(-4.0, -1.5))
    } else {
        0.0
    };
    OceanInputs {
        radius: Metres::new(radius),
        mass: Kilograms::new(mass),
        water_fraction: water,
        surface_temperature: Kelvin::new(surface),
        heat_flow: WattsPerSquareMetre::new(flux),
        solutes: OceanSolutes {
            sodium_chloride: Kilograms::new(salt),
            ammonia_per_water: ammonia,
        },
    }
}

/// The plan's closed-form presence test, `has_ocean`, agrees with the solve's root over 10⁴
/// random bodies, and every ocean the solve publishes over them is plausible (the plan's
/// invariants, [`implausibility`]).
#[test]
fn has_ocean_agrees_with_the_solve() {
    let mut draws = Draws(0x0051_0060_b000_0001);
    let (mut oceans, mut absent, mut none) = (0, 0, 0);
    for index in 0..10_000 {
        let inputs = random_body(&mut draws);
        let solved = Hydrosphere::of(&inputs).map(|water| water.base_under(inputs.heat_flow));
        assert_eq!(
            has_ocean(&inputs),
            matches!(solved, Some(Ok(_))),
            "{index}: {inputs:?} {solved:?}"
        );
        match subsurface_ocean(&inputs) {
            Subsurface::Ocean(ocean) => {
                oceans += 1;
                assert_eq!(implausibility(&inputs, &ocean), None, "{index}: {inputs:?}");
            }
            Subsurface::NotComputed => absent += 1,
            Subsurface::NoOcean => none += 1,
        }
    }
    eprintln!("10⁴ bodies: {oceans} oceans, {absent} absent, {none} none");
    assert!(
        oceans > 500 && absent > 100 && none > 500,
        "{oceans} {absent} {none}"
    );
}

/// Why `body`'s member is absent under `flux`: the column's foot, or its base or floor outside its
/// ice's field ([`Hydrosphere::absence`]).
fn absence_of(body: &ByHand, flux: f64) -> String {
    let inputs = body.at(flux);
    let water = Hydrosphere::of(&inputs).expect("a body with water");
    match water.base_under(inputs.heat_flow) {
        Ok(base) => format!("{:?}", absence(&base)),
        Err(error) => format!("{error:?}"),
    }
}

/// The thickness is continuous where published: as the heat flow falls it either thins to 0 at
/// the threshold where the base there lies in ice Ih (Europa), or ends finite, in absence, at ice
/// II's edge (Pluto, Ganymede, Callisto), each stated with its edge and last thickness
/// (decision-p14-t51g Q5). Absence may come in windows, its floor entering and leaving ice II's
/// field (decision-p14-t51gb-ice-ii §4.4 (4)): each window is listed with its cause, and the
/// thickness is continuous within each run of oceans.
#[test]
fn the_thickness_is_continuous_where_published() {
    for (name, body, from, ends_absent) in [
        ("Europa", solar::EUROPA, 0.05, false),
        ("Pluto", solar::PLUTO, 0.004, true),
        ("Ganymede", solar::GANYMEDE, 0.006, true),
        ("Callisto", solar::CALLISTO, 0.005, true),
    ] {
        let step = from / 2_000.0;
        let (mut first_end, mut last) = (None, None::<(f64, f64)>);
        let mut windows = Vec::new();
        for k in 0..2_000_u32 {
            let flux = from - step * f64::from(k);
            match body.ocean(flux) {
                Subsurface::Ocean(ocean) => {
                    let thickness = ocean.thickness().value();
                    if let Some((at, before)) = last
                        && (at - flux - step).abs() < step * 1e-6
                    {
                        assert!(
                            (thickness - before).abs() < 2e3,
                            "{name} at {flux}: {thickness} m after {before} m"
                        );
                    }
                    last = Some((flux, thickness));
                }
                other => {
                    if first_end.is_none() {
                        first_end = Some((other, last.expect("an ocean at the scan's start")));
                    }
                    if other == Subsurface::NotComputed && windows.len() < 8 {
                        windows.push(format!("{:.4}: {}", flux * 1e3, absence_of(&body, flux)));
                    }
                }
            }
        }
        let (end, (edge, thickness)) = first_end.expect("an ocean ends");
        eprintln!(
            "{name}: its first run of oceans ends at {:.4} mW m⁻², {:.2} km thick, then {end:?}; \
             absence windows from (mW m⁻²: cause) {windows:?}",
            edge * 1e3,
            thickness / 1e3
        );
        if ends_absent {
            assert_eq!(end, Subsurface::NotComputed, "{name}");
            assert!(thickness > 1e3, "{name}: {thickness} m");
        } else {
            assert_eq!(end, Subsurface::NoOcean, "{name}");
            assert!(thickness < 5e3, "{name}: {thickness} m");
        }
    }
}

/// The shell thickens as the surface or the heat flow falls.
#[test]
fn the_shell_thickens_as_the_surface_or_the_heat_falls() {
    let shell = |body: &ByHand, flux: f64| match body.ocean(flux) {
        Subsurface::Ocean(ocean) => ocean.shell_thickness().value(),
        other => panic!("{body:?} at {flux}: {other:?}"),
    };
    for body in [solar::EUROPA, solar::GANYMEDE, solar::TITAN] {
        let mut before = 0.0;
        for step in (0..40).rev() {
            let flux = 0.006 + f64::from(step) * 0.001;
            let now = shell(&body, flux);
            assert!(now > before, "{body:?} at {flux}");
            before = now;
        }
        let mut before = 0.0;
        for step in (0..40).rev() {
            let surface = 60.0 + f64::from(step) * 4.0;
            let now = shell(&ByHand { surface, ..body }, 0.01);
            assert!(now > before, "{body:?} at {surface} K");
            before = now;
        }
    }
}

/// Without solutes the base is ice Ih's melting point at its pressure, to 10⁻⁹ K, and its liquid
/// water; with them it is under it, and the liquid is a registry key.
#[test]
fn a_fresh_ocean_s_base_lies_on_ice_ih_s_curve() {
    for (body, flux) in [
        (solar::EUROPA, 0.01),
        (solar::GANYMEDE, 0.006),
        (solar::TITAN, 0.01),
    ] {
        let fresh = OceanInputs {
            solutes: OceanSolutes::FRESH,
            ..body.at(flux)
        };
        let water = Hydrosphere::of(&fresh).unwrap();
        let base = water.base_under(fresh.heat_flow).unwrap();
        let melting = ice_ih_melting_temperature(base.pressure).value();
        assert!(
            (base.temperature.value() - melting).abs() < 1e-9,
            "{body:?}"
        );
        let Subsurface::Ocean(ocean) = water.publish(&base) else {
            panic!("{body:?}: an ocean");
        };
        assert_eq!(ocean.liquid(), Gas::Water.substance());
        assert_same_bits(ocean.solute_mass_fraction(), 0.0);
        let salted = Hydrosphere::of(&body.at(flux)).unwrap();
        let base = salted.base_under(fresh.heat_flow).unwrap();
        assert!(
            base.temperature.value() < base.melting_point.value(),
            "{body:?}"
        );
    }
    let mut draws = Draws(0x0051_0060_b000_0002);
    for _ in 0..2_000 {
        let inputs = random_body(&mut draws);
        if let Subsurface::Ocean(ocean) = subsurface_ocean(&inputs) {
            let key = ocean.liquid().substance().key();
            assert_eq!(by_key(key.as_str()), Some(ocean.liquid()));
        }
    }
}

/// None where the surface is at or above water's triple point, with no water or heat, and where a
/// lone sea lies on the surface; and the presence test agrees.
#[test]
fn an_ocean_needs_water_heat_and_a_frozen_surface() {
    let europa = solar::EUROPA.at(0.05);
    for inputs in [
        OceanInputs {
            surface_temperature: Kelvin::new(273.16),
            ..europa
        },
        OceanInputs {
            water_fraction: 0.0,
            ..europa
        },
        OceanInputs {
            heat_flow: WattsPerSquareMetre::new(0.0),
            ..europa
        },
        OceanInputs {
            surface_temperature: Kelvin::new(f64::NAN),
            ..europa
        },
    ] {
        assert_eq!(subsurface_ocean(&inputs), Subsurface::NoOcean, "{inputs:?}");
        assert!(!has_ocean(&inputs), "{inputs:?}");
    }
    // Eutectic brine all the way: its surface at 250 K is above its whole column's liquidus.
    let brine = OceanInputs {
        surface_temperature: Kelvin::new(255.0),
        solutes: OceanSolutes {
            sodium_chloride: Kilograms::new(0.3 * 0.08 * 4.8e22),
            ammonia_per_water: 0.0,
        },
        ..europa
    };
    assert_eq!(subsurface_ocean(&brine), Subsurface::NoOcean);
    assert!(!has_ocean(&brine));
}

/// At T24.b's heat flow: Pluto and Triton by hand, all their solids rock, as the plan's production
/// counts them, their conducting rock's radiogenic heat at its conductive lag (decision-p14-moons
/// §F-ocean F1's 1.26 and 1.23 times production). Pluto holds an ocean 15–35 km thick; Triton's
/// member, as at 4.03 mW m⁻², is absent by the ruled edge, about 0.6 K inside it, so it is printed
/// with its cause and not asserted to a side, the golden pinning it (the plan's 15–35 km for it
/// superseded; decision-p14-t51gb-ice-ii §4.2). (T24.b's production is its rock's alone, so a body with an iron share
/// makes less: at 10% iron Pluto lies at 1.16 times the whole solids' production.)
#[test]
fn pluto_holds_an_ocean_and_triton_is_absent_at_their_rock_s_lag() {
    use crate::planetary::derive::PlanetClass;
    use crate::planetary::derive::atmosphere::SurfaceState;
    use crate::planetary::derive::solar::SOLAR_AGE;
    use crate::planetary::hooks::figures::{HeatInputs, non_tidal_heat_flow};
    use crate::units::{Dex, MetresPerSecondSquared};
    for (name, body) in [("Pluto", solar::PLUTO), ("Triton", solar::TRITON)] {
        let mass = EarthMasses::from(Kilograms::new(body.mass));
        let fractions = MassFractions::solid(0.0, 1.0 - body.water, body.water);
        let r = body.radius().value();
        let heat = non_tidal_heat_flow(
            &HeatInputs {
                mass,
                radius: body.radius(),
                surface_gravity: MetresPerSecondSquared::new(
                    GRAVITATIONAL_CONSTANT * body.mass / (r * r),
                ),
                class: PlanetClass::of(mass, &fractions),
                fractions,
                inventory_water: Kilograms::new(body.water * body.mass),
                fe_h: Dex::new(0.0),
                tidal_heat_flow: WattsPerSquareMetre::new(0.0),
                subsurface_ocean: Subsurface::NotComputed,
            },
            SOLAR_AGE,
            SurfaceState::Airless,
        )
        .expect("an icy body has a solid heat flow");
        eprintln!(
            "{name}: T24.b's {:.3} mW m⁻² against its production's {:.3}",
            heat.value() * 1e3,
            radiogenic(&body) * 1e3
        );
        eprintln!(
            "{name}: at T24.b's heat flow, {:.3} times its production: {:?}",
            heat.value() / radiogenic(&body),
            body.ocean(heat.value())
        );
        if name == "Triton" {
            eprintln!(
                "Triton at T24.b's heat flow: {}",
                absence_of(&body, heat.value())
            );
            continue;
        }
        let ocean = body.an_ocean(name, heat.value());
        let thickness = ocean.thickness().value() / 1e3;
        assert!((15.0..=35.0).contains(&thickness), "{name}: {thickness} km");
    }
}

/// `publish` on hand-built bases (decision-p14-t51gb-ice-ii §4.4 (2)), their T* and pressures from
/// R14-08's curves and not through ice II's edge: Triton's 4.03 mW m⁻² base, its ice III floor in
/// ice II's field, is absent; a rock floor at 206.87 MPa is absent at 233.0 K and published at
/// 236.35 K; an ice V floor at 500 MPa at 235 K, whose column crosses ice II's field, is
/// published; an ice III floor at 347.06 MPa at 250 K, in ice V's field, and an ice V floor at
/// 625 MPa at 235 K, in ice VI's, are absent.
#[test]
fn a_floor_is_judged_at_its_own_pressure_and_temperature() {
    let water = Hydrosphere::of(&OceanInputs {
        solutes: OceanSolutes::FRESH,
        ..solar::GANYMEDE.at(0.004_6)
    })
    .unwrap();
    let r = solar::GANYMEDE.radius().value();
    let base = |t_star: f64, p_b_mpa: f64, floor: OceanFloor, p_f_mpa: f64, t_b: f64| ShellBase {
        melting_point: Kelvin::new(t_star),
        radius: Metres::new(r - 100e3),
        pressure: Pascals::new(p_b_mpa * 1e6),
        floor_radius: Metres::new(r - 150e3),
        floor,
        floor_pressure: Pascals::new(p_f_mpa * 1e6),
        sodium_chloride: 0.0,
        ammonia: 0.0,
        temperature: Kelvin::new(t_b),
        heat_flow: WattsPerSquareMetre::new(0.004),
    };
    let rock = ice_ih_melting_temperature(Pascals::new(190e6)).value();
    for (case, built, expected) in [
        (
            "Triton's 4.03",
            base(252.09, 201.73, OceanFloor::IceIii, 223.9, 236.0),
            Some(Absence::Floor(ForeignIce::IceIi)),
        ),
        (
            "rock at 233.0 K",
            base(rock, 190.0, OceanFloor::Rock, 206.87, 233.0),
            Some(Absence::Floor(ForeignIce::IceIi)),
        ),
        (
            "rock at 236.35 K",
            base(rock, 190.0, OceanFloor::Rock, 206.87, 236.35),
            None,
        ),
        (
            "ice V crossing",
            base(266.22, 80.21, OceanFloor::IceV, 500.0, 235.0),
            None,
        ),
        (
            "ice III in ice V's field",
            base(256.10, 170.70, OceanFloor::IceIii, 347.06, 250.0),
            Some(Absence::Floor(ForeignIce::IceV)),
        ),
        (
            "ice V in ice VI's field",
            base(272.95, 2.86, OceanFloor::IceV, 625.0, 235.0),
            Some(Absence::Floor(ForeignIce::IceVi)),
        ),
        (
            "a base in ice II",
            base(251.5, 205.0, OceanFloor::Rock, 206.0, 225.0),
            Some(Absence::BaseInIceIi),
        ),
    ] {
        assert_eq!(absence(&built), expected, "{case}");
        let published = water.publish(&built);
        match expected {
            Some(_) => assert_eq!(published, Subsurface::NotComputed, "{case}"),
            None => assert!(
                matches!(published, Subsurface::Ocean(_)),
                "{case}: {published:?}"
            ),
        }
    }
}

/// The plan's bodies' oceans pinned to the bit (the determinism review: no golden prints a solved
/// ocean until the bump re-blesses): each at its tests' heat flows, its outcome and, for an ocean,
/// its shell, depth, floor, liquid, solutes and base temperature, so that a moved curve, halving
/// count or solute law moves a line on either target.
#[test]
fn the_plan_s_bodies_oceans_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    let enceladus = 22e9 / solar::ENCELADUS.area();
    for (name, body, flux) in [
        ("europa", solar::EUROPA, 0.05),
        ("ganymede", solar::GANYMEDE, 0.004_6),
        ("callisto", solar::CALLISTO, 0.004),
        ("titan_cold", solar::TITAN, 0.004_6),
        ("titan_warm", solar::TITAN, 0.009_9),
        ("enceladus", solar::ENCELADUS, enceladus),
        ("pluto_production", solar::PLUTO, 0.002_45),
        ("pluto_lag", solar::PLUTO, 0.003_09),
        ("pluto_warm", solar::PLUTO, 0.003_5),
        ("triton_lag", solar::TRITON, 0.004_03),
        ("ceres", solar::CERES, 0.001_24),
        ("snowball_earth", solar::SNOWBALL_EARTH, 0.087),
    ] {
        match body.ocean(flux) {
            Subsurface::Ocean(ocean) => {
                w.line(&format!(
                    "{name} = {:?} {}",
                    ocean.floor(),
                    ocean.liquid().substance().key().as_str()
                ));
                w.f64(&format!("{name}_shell"), ocean.shell_thickness().value());
                w.f64(&format!("{name}_thickness"), ocean.thickness().value());
                w.f64(&format!("{name}_solutes"), ocean.solute_mass_fraction());
                w.f64(&format!("{name}_base"), ocean.base_temperature().value());
            }
            other => w.line(&format!("{name} = {other:?}")),
        }
    }
    golden!("planetary/ocean_solve", w.as_str());
}
