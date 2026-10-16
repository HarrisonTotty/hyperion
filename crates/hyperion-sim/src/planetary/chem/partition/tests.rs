//! The partition's tests: plan 14's Solar System table (P14.T51.a), every registry condensable hot
//! and cold, the rules' invariants, and the golden that pins the partition across targets.

use std::cell::RefCell;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::substance::{by_key, diagram};

fn id(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

fn span(cold: f64, mean: f64, warm: f64) -> ClimateSpan {
    ClimateSpan::new(Kelvin::new(cold), Kelvin::new(mean), Kelvin::new(warm)).unwrap()
}

fn inventory(entries: &[(&str, f64)]) -> ColumnInventory {
    ColumnInventory::new(entries.iter().map(|&(k, w)| (id(k), Pascals::new(w)))).unwrap()
}

fn sat(key: &str, t: f64) -> f64 {
    diagram::saturation_pressure(id(key), Kelvin::new(t))
        .unwrap()
        .value()
}

fn molar_mass(key: &str) -> f64 {
    id(key).substance().molar_mass_g_per_mol().unwrap()
}

fn gravity(g: f64) -> MetresPerSecondSquared {
    MetresPerSecondSquared::new(g)
}

/// The column weights of an air of surface pressure `pressure`, Pa, and mole fractions
/// `fractions` (normalised here): each x μ ÷ μ̄ of the pressure (P14.T24.a's mole fractions,
/// inverted).
fn weights_of(pressure: f64, fractions: &[(&'static str, f64)]) -> Vec<(&'static str, f64)> {
    let sum = fractions.iter().fold(0.0, |s, &(_, x)| s + x);
    let mean = fractions
        .iter()
        .fold(0.0, |s, &(k, x)| s + x / sum * molar_mass(k));
    fractions
        .iter()
        .map(|&(k, x)| (k, pressure * x / sum * molar_mass(k) / mean))
        .collect()
}

fn of<'a>(partition: &'a Partition, key: &str) -> &'a Partitioned {
    partition
        .of(id(key))
        .unwrap_or_else(|| panic!("{key} is named"))
}

fn condensate<'a>(partition: &'a Partition, key: &str) -> &'a Condensate {
    partition
        .condensate(id(key))
        .unwrap_or_else(|| panic!("{key} has a condensate: {partition:?}"))
}

fn relative(ours: f64, theirs: f64) -> f64 {
    (ours / theirs - 1.0).abs()
}

/// Earth's surface gravity at its mean radius, m s⁻² (NASA's Earth fact sheet, 9.820).
const EARTH_GRAVITY: f64 = 9.82;

/// Mars's surface gravity at its mean radius, m s⁻² (NASA's Mars fact sheet, 3.73).
const MARS_GRAVITY: f64 = 3.73;

/// The column weight of `kg` spread over Earth's whole surface: kg g ÷ 4π R⊕², R⊕ 6,371 km.
fn earth_weight(kg: f64) -> f64 {
    kg * EARTH_GRAVITY / (4.0 * core::f64::consts::PI * 6.371e6 * 6.371e6)
}

/// A body of the plan's Solar System table, by hand: its climate, inventory and surface gravity.
struct Body {
    name: &'static str,
    climate: ClimateSpan,
    inventory: ColumnInventory,
    gravity: f64,
}

impl Body {
    fn partition(&self) -> Partition {
        partition(&self.climate, gravity(self.gravity), &self.inventory)
    }
}

/// Earth: its oceans' 1.4 × 10²¹ kg of water (plan 14's `EARTH_WATER_PER_MASS`), its 3.87 × 10¹⁸ kg
/// of nitrogen, 6.6 × 10¹⁶ kg of argon and 42 Pa of carbon dioxide (the generated Earth's), over
/// the generated Earth's climate's range, 261 to 305 K about its 288 K (science-p14-t24a-climate.md
/// §3).
fn earth() -> Body {
    Body {
        name: "Earth",
        climate: span(261.0, 288.0, 305.0),
        inventory: inventory(&[
            ("H2O", earth_weight(1.4e21)),
            ("N2", earth_weight(3.87e18)),
            ("CO2", 42.0),
            ("Ar", earth_weight(6.6e16)),
        ]),
        gravity: EARTH_GRAVITY,
    }
}

/// Mars's measured air (NASA's Mars fact sheet, read): 636 Pa at the mean radius, 95.1% carbon
/// dioxide, 2.59% nitrogen and 1.94% argon by volume, over a surface whose mean is 214 K and whose
/// warmest is the Viking 1 site's 242 K. Its coldest is its carbon dioxide's frost point at its
/// measured partial pressure, where the polar caps sit, 147.7 K on the registry's curve. Its carbon
/// dioxide beyond the air is a reservoir as large as the air's, the lane's for the residual cap and
/// the buried deposits; its water, 10⁵ Pa of weight, about 27 m of ice over the whole planet, the
/// lane's for its polar layered deposits.
fn mars() -> Body {
    let weights = weights_of(636.0, &[("CO2", 0.951), ("N2", 0.0259), ("Ar", 0.0194)]);
    let co2_partial = 636.0 * 0.951 / (0.951 + 0.0259 + 0.0194);
    let frost = id("CO2")
        .substance()
        .phase_diagram()
        .unwrap()
        .saturation_temperature(Pascals::new(co2_partial))
        .unwrap();
    let mut entries: Vec<(&str, f64)> = weights
        .iter()
        .map(|&(k, w)| (k, if k == "CO2" { 2.0 * w } else { w }))
        .collect();
    entries.push(("H2O", 1e5));
    Body {
        name: "Mars",
        climate: ClimateSpan::new(frost, Kelvin::new(214.0), Kelvin::new(242.0)).unwrap(),
        inventory: inventory(&entries),
        gravity: MARS_GRAVITY,
    }
}

/// Titan by hand: 146.7 kPa at the Huygens site's 93.65 K (Fulchignoni et al. 2005, Nature 438,
/// 785, as `METHANE_RELATIVE_HUMIDITY` cites it), taken as its mean, which is also its warmest
/// latitude's, held at 93.65 ± 0.15 K, with its south pole at 89.7 ± 0.5 K in 2013 (Jennings et al.
/// 2016, ApJ 816, L17, abstract, read). Its nitrogen is the weight of 146.7 kPa less its 5.65% of
/// methane, 1.418 × 10⁵ Pa; its methane a tenth more than its air holds, for its seas and lakes, of
/// the order of a tenth of its air's (from memory); its water a crust of ice.
fn titan() -> Body {
    Body {
        name: "Titan",
        climate: span(89.7, 93.65, 93.65),
        inventory: inventory(&[("CH4", 5.5e3), ("N2", 1.418e5), ("H2O", 1e9)]),
        gravity: 1.352,
    }
}

/// Nitrogen as dominant at its ice's temperature, with methane over the warmer, methane-rich
/// terrain at its mean (science-p14-t24f §1), for a body of surface gravity `gravity`: its
/// nitrogen ice a reservoir of 10⁵ Pa of weight, its methane ice 10³, and its carbon monoxide the
/// measured air's alone, since that lies dissolved in the nitrogen ice (below).
fn frost_world(
    name: &'static str,
    climate: ClimateSpan,
    gravity: f64,
    carbon_monoxide: f64,
) -> Body {
    Body {
        name,
        climate,
        inventory: inventory(&[("N2", 1e5), ("CH4", 1e3), ("CO", carbon_monoxide)]),
        gravity,
    }
}

/// Triton: nitrogen ice at 38 ± 1 K (Tryka et al. 1994, Icarus 112, 513, as Lellouch et al. 2010,
/// A&A 512, L8, Fig. 5, read, credits it), its methane over terrain at 40 K and its warmest pure
/// methane patches at 45 K (Lellouch et al. 2010, §3, "45–48 K"), its measured CO 24 nbar (their
/// §2, 2009); surface gravity 0.779 m s⁻².
fn triton() -> Body {
    frost_world("Triton", span(38.0, 40.0, 45.0), 0.779, 24e-4)
}

/// Pluto: nitrogen ice at 37 K, giving about the 11.5 µbar New Horizons measured (from memory),
/// its methane over terrain at 42 K (science-p14-t24f §1), its warmest 45 K; its CO about 5 × 10⁻⁴
/// of its nitrogen, 0.6 mPa (Lellouch et al. 2011, A&A 530, L4, and 2017, Icarus 286, 289, from
/// memory); surface gravity 0.62 m s⁻².
fn pluto() -> Body {
    frost_world("Pluto", span(37.0, 42.0, 45.0), 0.62, 6e-4)
}

#[test]
fn earth_holds_liquid_and_solid_water() {
    let earth = earth().partition();
    let water = of(&earth, "H2O");
    // A minor condensable: its partial pressure is Manabe and Wetherald's 0.77 of saturation at
    // the mean, to a part in a billion.
    assert_same_bits(water.dominance(), 0.0);
    let held = WATER_SURFACE_HUMIDITY * sat("H2O", 288.0);
    assert!(
        relative(water.partial_pressure().value(), held) < 1e-9,
        "{water:?} against {held}"
    );
    let ocean = condensate(&earth, "H2O");
    assert_eq!(ocean.phase(), CondensatePhase::Liquid);
    assert!(ocean.phases().contains(CondensatePhase::Solid));
    assert!(ocean.phases().contains(CondensatePhase::Liquid));
    // Its range crosses the melting line: its ice comes and goes.
    assert!(ocean.is_seasonal(), "{ocean:?}");
    // Its oceans, about 2.7 km over the whole surface.
    let kg = ocean.reservoir().value();
    assert!((2.6e6..2.8e6).contains(&kg), "{kg} kg m⁻²");
    // Nitrogen, carbon dioxide and argon do not condense on Earth.
    for gas in ["N2", "CO2", "Ar"] {
        assert!(earth.condensate(id(gas)).is_none(), "{gas}");
        assert_same_bits(of(&earth, gas).condensed().value(), 0.0);
    }
    let bars = earth.surface_pressure().value() / 1e5;
    assert!((0.74..0.80).contains(&bars), "{bars} bar");
}

/// The generated Mars stores its carbon in rock and keeps 0.11 bar of nitrogen until P14.T53.a's
/// escape by regime (plan 14's finding, P14.T13.b), so its carbon dioxide condenses nowhere; given
/// its measured air by hand, the partition holds it as Mars does.
#[test]
fn a_mars_given_its_measured_air_holds_carbon_dioxide_and_water_ice() {
    let body = mars();
    let (mars, climate) = (body.partition(), body.climate);
    let co2 = of(&mars, "CO2");
    assert_same_bits(co2.dominance(), 1.0);
    assert_same_bits(co2.lies_at().value(), climate.coldest().value());
    let cap = sat("CO2", climate.coldest().value());
    assert!(
        relative(co2.partial_pressure().value(), cap) < 1e-9,
        "{co2:?} against {cap}"
    );
    // The surface pressure stays the measured 636 Pa, with the water vapour the partition adds,
    // to two parts in a thousand.
    assert!(
        relative(mars.surface_pressure().value(), 636.0) < 2e-3,
        "{mars:?}"
    );
    // Perennial ice at the cold trap, with seasonal frost that the warmest zone takes off.
    let caps = condensate(&mars, "CO2");
    assert_eq!(caps.phase(), CondensatePhase::Solid);
    assert!(
        caps.reservoir().value() > 0.0 && caps.is_seasonal(),
        "{caps:?}"
    );
    // Water ice, a minor condensable held at the mean's humidity, frozen across the range.
    let ice = condensate(&mars, "H2O");
    assert_eq!(ice.phase(), CondensatePhase::Solid);
    assert!(!ice.phases().contains(CondensatePhase::Liquid));
    let water = of(&mars, "H2O");
    assert!(relative(water.partial_pressure().value(), 0.77 * sat("H2O", 214.0)) < 1e-9);
    // The deepest reservoir is the water's.
    assert_eq!(mars.condensates()[0].substance(), id("H2O"));
}

/// Titan holds liquid methane on the minor branch over a crust of water ice. Its poles, at 89.7 K,
/// lie below pure methane's 90.69 K triple point, where the partition, which takes methane as its
/// pure substance, reads a solid; its seas are a solution, liquid down to the methane–ethane
/// eutectic at 71.15 K (decision-p14-t49b-condensed §2.4), which T51.a's second part reads from
/// the registry's first melt. Neither the solid nor the seas' seasonality is asserted here.
#[test]
fn titan_holds_liquid_methane_over_a_water_ice_crust() {
    let titan = titan().partition();
    let methane = of(&titan, "CH4");
    // On the minor branch, at its 5.65% (Niemann et al. 2010) within T24.f's test.
    assert_same_bits(methane.dominance(), 0.0);
    let share = methane.partial_pressure().value() / titan.surface_pressure().value();
    assert!((0.0565 - share).abs() < 0.0036, "{share}");
    assert!(
        relative(titan.surface_pressure().value(), 146_700.0) < 0.01,
        "{titan:?}"
    );
    let seas = condensate(&titan, "CH4");
    assert_eq!(seas.phase(), CondensatePhase::Liquid);
    assert!(seas.reservoir().value() > 0.0);
    let crust = condensate(&titan, "H2O");
    assert_eq!(crust.phase(), CondensatePhase::Solid);
    assert!(!crust.is_seasonal() && !crust.phases().contains(CondensatePhase::Liquid));
    assert_eq!(titan.condensates()[0].substance(), id("H2O"));
}

/// Triton and Pluto given their measured air by hand ([`triton`], [`pluto`]) hold nitrogen ice
/// with methane ice, and their methane within a factor of 3 of the measured: Triton's 9.8 ± 3.7
/// nbar (Lellouch et al. 2010, §2, 2009, read), and Pluto's (3–8) × 10⁻³ Pa, plan 14's range.
///
/// The carbon monoxide is given as its measured air's alone: on both it lies dissolved in the
/// nitrogen ice at a part in a thousand, where Raoult's law holds it far below a pure ice's vapour
/// (Lellouch et al. 2010, §3), and the partition takes each condensable as its pure substance.
///
/// The methane is held at Titan's humidity, 0.5, over the methane ice's saturation (science-p14-t24f
/// §1), which halves it: Triton's 0.39 mPa is 0.40 of its measured 0.98 mPa, and Pluto's 1.6 mPa is
/// 0.53 of the plan's lower end, and 0.31–0.21 of the 5.1–7.5 mPa that Lellouch et al. (2015,
/// Icarus 246, 268, read) predicted for 2015, 0.44% of 11.5–17 µbar (plan 14's deferred findings,
/// P14.T51.a).
#[test]
fn triton_and_pluto_hold_nitrogen_ice_with_methane() {
    for (body, measured) in [(triton(), (9.8e-4, 9.8e-4)), (pluto(), (3e-3, 8e-3))] {
        let name = body.name;
        let world = body.partition();
        let nitrogen = of(&world, "N2");
        assert_same_bits(nitrogen.dominance(), 1.0);
        let ice = condensate(&world, "N2");
        assert_eq!(ice.phase(), CondensatePhase::Solid, "{name}");
        assert!(ice.is_seasonal(), "{name}: {ice:?}");
        let methane = condensate(&world, "CH4");
        assert_eq!(methane.phase(), CondensatePhase::Solid, "{name}");
        // Within a factor of 3 of the measured.
        let p = of(&world, "CH4").partial_pressure().value();
        assert!(
            (measured.0 / 3.0..3.0 * measured.1).contains(&p),
            "{name}: CH4 at {p} Pa against {measured:?}"
        );
        // Its carbon monoxide, all airborne, as measured.
        assert!(world.condensate(id("CO")).is_none(), "{name}");
    }
}

/// A cold world of 0.2 bar of nitrogen at 120 K, its contrasts by hand (115 to 124 K), with the
/// carbon of a body formed beyond the snow line as methane, some 60 times its nitrogen by moles
/// (science-p14-t24f §2).
fn cold_world() -> Body {
    let nitrogen = 2.0e4;
    let methane = 60.0 * nitrogen * molar_mass("CH4") / molar_mass("N2");
    Body {
        name: "cold world",
        climate: span(115.0, 120.0, 124.0),
        inventory: inventory(&[("CH4", methane), ("N2", nitrogen)]),
        gravity: 1.352,
    }
}

/// The cold world's methane dominates its air and is held at its cold trap.
#[test]
fn a_cold_world_holds_its_methane_at_its_cold_trap() {
    let world = cold_world().partition();
    let p = of(&world, "CH4").partial_pressure().value();
    let cold = sat("CH4", 115.0);
    assert!(relative(p, cold) < 0.1, "{p} against {cold}");
    assert!(of(&world, "CH4").dominance() > 0.9);
    // Its sea at the cold trap, above methane's triple point.
    assert_eq!(condensate(&world, "CH4").phase(), CondensatePhase::Liquid);
}

/// Venus (NASA's Venus fact sheet, read): `pressure`, Pa, of 96.5% carbon dioxide and 3.5% nitrogen
/// by volume, over a near-isothermal climate about its 737 K (733 to 740 K), at 8.87 m s⁻².
fn venus(name: &'static str, pressure: f64) -> Body {
    Body {
        name,
        climate: span(733.0, 737.0, 740.0),
        inventory: inventory(&weights_of(pressure, &[("CO2", 0.965), ("N2", 0.035)])),
        gravity: 8.87,
    }
}

/// The generated Venus's 57 bar (plan 14's table) holds none; the measured 92 bar a supercritical
/// carbon dioxide surface.
#[test]
fn venus_holds_none_and_at_92_bar_a_supercritical_surface() {
    let generated = venus("Venus at 57 bar", 5.7e6).partition();
    assert!(generated.condensates().is_empty(), "{generated:?}");
    let measured = venus("Venus at 92 bar", 9.2e6).partition();
    let [fluid] = measured.condensates() else {
        panic!("one condensate: {measured:?}");
    };
    assert_eq!(fluid.substance(), id("CO2"));
    assert_eq!(fluid.phase(), CondensatePhase::Supercritical);
    assert_same_bits(fluid.reservoir().value(), 0.0);
    // All of it is airborne: the surface lies in the fluid.
    assert!(relative(measured.surface_pressure().value(), 9.2e6) < 1e-12);
}

/// A world of 10 bar of carbon dioxide at 250 K, its contrasts by hand (225 to 265 K).
fn carbon_dioxide_sea() -> Body {
    Body {
        name: "10-bar CO2 world",
        climate: span(225.0, 250.0, 265.0),
        inventory: inventory(&[("CO2", 1e6)]),
        gravity: EARTH_GRAVITY,
    }
}

/// Its cold trap lies above carbon dioxide's triple point, 216.59 K and 5.18 bar, so it holds a
/// liquid sea there, which its warmest zone boils off.
#[test]
fn a_ten_bar_carbon_dioxide_world_at_250_k_holds_a_liquid_sea() {
    let world = carbon_dioxide_sea().partition();
    let sea = condensate(&world, "CO2");
    assert_eq!(sea.phase(), CondensatePhase::Liquid);
    assert!(
        sea.reservoir().value() > 0.0 && sea.is_seasonal(),
        "{sea:?}"
    );
    assert!(relative(world.surface_pressure().value(), sat("CO2", 225.0)) < 1e-9);
}

/// An icy moon with no air of its own: water ice alone, a Ganymede's 80 to 150 K about 110 K, by
/// hand.
fn icy_moon() -> Body {
    Body {
        name: "icy moon",
        climate: span(80.0, 110.0, 150.0),
        inventory: inventory(&[("H2O", 1e9)]),
        gravity: 1.428,
    }
}

/// Its water is dominant in an empty column, held at its coldest zone's frost point, its warmest
/// zone subliming it.
#[test]
fn an_airless_icy_moon_holds_its_water_as_ice_at_its_cold_trap() {
    let moon = icy_moon().partition();
    let water = of(&moon, "H2O");
    assert_same_bits(water.dominance(), 1.0);
    assert!(relative(water.partial_pressure().value(), sat("H2O", 80.0)) < 1e-9);
    let ice = condensate(&moon, "H2O");
    assert_eq!(ice.phase(), CondensatePhase::Solid);
    assert!(ice.is_seasonal());
}

/// A hot world's water at 700 K, above its 647.096 K critical point, with `bars` of steam.
fn steam(name: &'static str, bars: f64) -> Body {
    Body {
        name,
        climate: span(690.0, 700.0, 710.0),
        inventory: inventory(&[("H2O", bars * 1e5)]),
        gravity: EARTH_GRAVITY,
    }
}

/// 100 bar of steam is vapour and 300 bar, above its 220.64 bar critical pressure, a supercritical
/// fluid; neither holds a condensed reservoir.
#[test]
fn a_hot_world_s_water_is_vapour_or_supercritical() {
    for (body, supercritical) in [
        (steam("100 bar of steam", 100.0), false),
        (steam("300 bar of steam", 300.0), true),
    ] {
        let world = body.partition();
        assert_same_bits(of(&world, "H2O").condensed().value(), 0.0);
        assert_eq!(
            world
                .condensate(id("H2O"))
                .map(|c| c.phase() == CondensatePhase::Supercritical),
            supercritical.then_some(true),
            "{}",
            body.name
        );
    }
}

/// Every condensable of the registry, alone and as a minor gas in nitrogen, below its triple point,
/// between its triple and critical points and above its critical point.
#[test]
fn every_registry_condensable_partitions_hot_and_cold() {
    for substance in diagram::condensables() {
        let key = substance.substance().key().as_str().to_owned();
        let d = substance.substance().phase_diagram().unwrap();
        let (triple, critical) = (
            d.triple_temperature().value(),
            d.critical_temperature().value(),
        );
        let g = gravity(EARTH_GRAVITY);
        let alone =
            |weight: f64| ColumnInventory::new([(substance, Pascals::new(weight))]).unwrap();
        // Below the triple point, alone, with far more than its vapour: ice at its cold trap.
        let cold = span(0.7 * triple, 0.8 * triple, 0.9 * triple);
        let lots = 1e3 * (1.0 + d.critical_pressure().value());
        let ice = partition(&cold, g, &alone(lots));
        let it = ice.of(substance).unwrap();
        assert_same_bits(it.dominance(), 1.0);
        let cap = d.saturation_pressure(cold.coldest()).value();
        assert!(relative(it.airborne().value(), cap) < 1e-9, "{key}: {it:?}");
        assert_eq!(
            ice.condensate(substance).unwrap().phase(),
            CondensatePhase::Solid,
            "{key}"
        );
        // Between the triple and critical points: a liquid at its cold trap.
        let t = triple + 0.5 * (critical - triple);
        let sea = partition(&span(t, t + 1.0, t + 2.0), g, &alone(lots));
        assert_eq!(
            sea.condensate(substance).unwrap().phase(),
            CondensatePhase::Liquid,
            "{key}"
        );
        // Above the critical point: no condensed reservoir; supercritical above its pressure.
        let hot = span(1.1 * critical, 1.2 * critical, 1.3 * critical);
        let fluid = partition(&hot, g, &alone(lots));
        assert_same_bits(fluid.of(substance).unwrap().condensed().value(), 0.0);
        assert_eq!(
            fluid.condensate(substance).unwrap().phase(),
            CondensatePhase::Supercritical,
            "{key}"
        );
        // A minor gas in a bar of nitrogen, below its triple point but above nitrogen's critical
        // point, is held at its humidity at the mean, if it condenses at all.
        if substance != id("N2") && 0.7 * triple > 126.2 {
            let inv = ColumnInventory::new([
                (substance, Pascals::new(1.0)),
                (id("N2"), Pascals::new(1e5)),
            ])
            .unwrap();
            let minor = partition(&cold, g, &inv);
            let it = minor.of(substance).unwrap();
            let held = surface_humidity(substance) * d.saturation_pressure(cold.mean()).value();
            if it.condensed().value() > 0.0 {
                assert_same_bits(it.dominance(), 0.0);
                assert!(
                    relative(it.partial_pressure().value(), held) < 1e-9,
                    "{key}: {it:?}"
                );
            } else {
                assert!(
                    it.partial_pressure().value() <= held * (1.0 + 1e-12),
                    "{key}"
                );
            }
        }
    }
}

/// The mixes the invariants are checked over: every condensable with nitrogen, water or helium, at
/// inventories from a milligram's weight to a hundred bars, over cold, temperate and hot climates.
fn mixes() -> Vec<(ClimateSpan, ColumnInventory)> {
    let climates = [
        span(30.0, 40.0, 50.0),
        span(60.0, 90.0, 95.0),
        span(110.0, 120.0, 130.0),
        span(150.0, 214.0, 242.0),
        span(220.0, 250.0, 280.0),
        span(261.0, 288.0, 305.0),
        span(500.0, 700.0, 750.0),
    ];
    let mut out = Vec::new();
    for climate in climates {
        for substance in diagram::condensables() {
            for weight in [1e-3, 1.0, 1e3, 1e5, 1e7] {
                for (other, background) in [(id("N2"), 1e5), (id("H2O"), 1e6), (id("He"), 1e4)] {
                    if other == substance {
                        continue;
                    }
                    let mut entries = vec![
                        (substance, Pascals::new(weight)),
                        (other, Pascals::new(background)),
                    ];
                    if substance != id("CO2") && other != id("CO2") {
                        entries.push((id("CO2"), Pascals::new(30.0)));
                    }
                    out.push((climate, ColumnInventory::new(entries).unwrap()));
                }
            }
        }
    }
    out
}

/// The plan's bodies, as built by hand above.
fn bodies() -> Vec<Body> {
    vec![
        earth(),
        mars(),
        titan(),
        triton(),
        pluto(),
        cold_world(),
        venus("Venus at 57 bar", 5.7e6),
        venus("Venus at 92 bar", 9.2e6),
        carbon_dioxide_sea(),
        icy_moon(),
        steam("100 bar of steam", 100.0),
        steam("300 bar of steam", 300.0),
    ]
}

/// The mixes and the bodies' climates and inventories together.
fn cases() -> Vec<(ClimateSpan, ColumnInventory)> {
    let mut cases = mixes();
    cases.extend(bodies().into_iter().map(|b| (b.climate, b.inventory)));
    cases
}

#[test]
fn every_species_airborne_and_condensed_masses_sum_to_its_inventory() {
    let g = gravity(3.0);
    for (climate, inv) in cases() {
        let p = partition(&climate, g, &inv);
        let mut sum = 0.0;
        for (s, (substance, weight)) in p.species().iter().zip(inv.species()) {
            assert_eq!(s.substance(), substance);
            assert_same_bits(s.inventory().value(), weight.value());
            let (air, condensed) = (s.airborne().value(), s.condensed().value());
            assert!(air >= 0.0 && condensed >= 0.0, "{s:?}");
            assert!(
                (air + condensed - weight.value()).abs() <= 1e-9 * weight.value(),
                "{climate:?}: {s:?}"
            );
            if let Some(c) = p.condensate(substance) {
                let reservoir = c.reservoir().value() * g.value();
                assert!(
                    (reservoir - condensed).abs() <= 1e-9 * weight.value(),
                    "{c:?}"
                );
            }
            sum += air;
        }
        assert_same_bits(p.surface_pressure().value(), sum);
        // The partial pressures are the mole fractions of the surface pressure.
        let partials = p
            .species()
            .iter()
            .fold(0.0, |t, s| t + s.partial_pressure().value());
        assert!((partials - sum).abs() <= 1e-9 * sum.max(1e-300), "{p:?}");
    }
}

/// Wherever a condensable is capped, its partial pressure in the air, x p, meets its cap,
/// h^(1 − d) `p_sat`(T): a minor one's h `p_sat`(`T_mean`) (plan 14's test). Both meet it to a part
/// in a billion in every air with at most one condensable held toward its cold trap. In an air with
/// two or more, the fixed sweeps leave every species within a part in a thousand
/// ([`PARTITION_SWEEPS`]; plan 14's Risks, P14.T51.a as built).
#[test]
fn a_capped_condensable_meets_its_cap() {
    let g = gravity(3.0);
    let (mut minor, mut held, mut worst) = (0, 0, 0.0_f64);
    for (climate, inv) in cases() {
        let p = partition(&climate, g, &inv);
        let toward_cold_trap = p
            .species()
            .iter()
            .filter(|s| s.dominance() > 0.0 && s.condensed().value() > 0.0)
            .count();
        for s in p.species() {
            let Some(d) = s.substance().substance().phase_diagram() else {
                continue;
            };
            if s.condensed().value() <= 0.0 {
                continue;
            }
            let h = surface_humidity(s.substance());
            let dom = s.dominance();
            let cap = math::powf(h, 1.0 - dom) * d.saturation_pressure(s.lies_at()).value();
            let miss = relative(s.partial_pressure().value(), cap);
            let tolerance = if toward_cold_trap > 1 { 1e-3 } else { 1e-9 };
            worst = worst.max(miss);
            if dom <= 0.0 {
                minor += 1;
                let at_mean = h * d.saturation_pressure(climate.mean()).value();
                assert!(
                    relative(s.partial_pressure().value(), at_mean) < tolerance,
                    "{climate:?}: {s:?} against {at_mean}"
                );
            } else {
                held += 1;
                assert!(miss < tolerance, "{climate:?}: {s:?} against {cap}");
            }
        }
    }
    assert!(minor > 50 && held > 50, "{minor} minor, {held} held");
    eprintln!("the worst cap is missed by {worst:e}");
}

/// The fixed sweeps lie within a part in a thousand of 256 sweeps run out for every mix, and within
/// a part in 10¹² for the plan's bodies.
#[test]
fn the_fixed_sweeps_settle_on_the_partition() {
    let (mut fixed, mut run_out) = (AirborneWeights::new(), AirborneWeights::new());
    let check = |climate: &ClimateSpan, inv: &ColumnInventory, tolerance: f64| {
        let (mut a, mut b) = (AirborneWeights::new(), AirborneWeights::new());
        a.solve(climate, inv);
        b.solve_sweeping(climate, inv, 256);
        let mut worst = 0.0_f64;
        for (x, y) in a.weights().iter().zip(b.weights()) {
            let scale = x.value().abs().max(y.value().abs());
            let miss = (x.value() - y.value()).abs();
            assert!(
                miss <= tolerance * scale,
                "{climate:?} {inv:?}: {x:?} against {y:?}"
            );
            if scale > 0.0 {
                worst = worst.max(miss / scale);
            }
        }
        worst
    };
    let worst = mixes()
        .iter()
        .fold(0.0_f64, |w, (c, inv)| w.max(check(c, inv, 1e-3)));
    for body in bodies() {
        check(&body.climate, &body.inventory, 1e-12);
    }
    eprintln!("the fixed sweeps miss 256 by at most {worst:e}");
    // Two solves of one scratch read nothing of each other.
    let (climate, inv) = (earth().climate, earth().inventory);
    fixed.solve(&climate, &inv);
    run_out.solve(&span(30.0, 40.0, 50.0), &inv);
    run_out.solve(&climate, &inv);
    assert_eq!(format!("{fixed:?}"), format!("{run_out:?}"));
}

/// The partition is the same twice and in any order of calls, with or without a shared scratch
/// (the sim-determinism skill's order independence).
#[test]
fn the_partition_is_the_same_twice_and_in_any_order() {
    let cases = cases();
    let g = gravity(3.0);
    for (climate, inv) in &cases {
        assert_eq!(
            format!("{:?}", partition(climate, g, inv)),
            format!("{:?}", partition(climate, g, inv))
        );
    }
    let shared = RefCell::new(AirborneWeights::new());
    assert_order_independent(&cases, |(climate, inv)| {
        let mut weights = shared.borrow_mut();
        weights.solve(climate, inv);
        format!("{:?}", weights.weights())
    });
}

/// The partition is continuous in its inputs, so that a body's gases are continuous in time but
/// at its recorded state changes: across the dominance ramp in a condensable's inventory, and
/// across the triple point in the mean temperature, no step of a fine sweep moves an airborne weight
/// by more than a few times the step.
#[test]
fn the_partition_is_continuous_in_inventory_and_temperature() {
    let g = gravity(EARTH_GRAVITY);
    // Carbon dioxide from a tenth of a pascal to a thousand bars over a bar of nitrogen, at a cold
    // trap below its frost point: its share sweeps through the ramp.
    let climate = span(150.0, 190.0, 220.0);
    let steps = 4_000;
    let mut last: Option<f64> = None;
    for i in 0..=steps {
        let w = math::exp10(-1.0 + 8.0 * f64::from(i) / f64::from(steps));
        let p = partition(&climate, g, &inventory(&[("CO2", w), ("N2", 1e5)]));
        let air = of(&p, "CO2").airborne().value();
        if let Some(before) = last {
            // A step of 0.46% in the inventory moves the air by at most a few per cent.
            assert!(
                (air / before - 1.0).abs() < 0.03,
                "at {w}: {before} → {air}"
            );
        }
        last = Some(air);
    }
    // Water's mean temperature across its triple point, on a temperate world.
    let mut last: Option<f64> = None;
    for i in 0..=steps {
        let t = 268.0 + 10.0 * f64::from(i) / f64::from(steps);
        let p = partition(
            &span(t - 20.0, t, t + 15.0),
            g,
            &inventory(&[("H2O", 2.7e7), ("N2", 7.4e4)]),
        );
        let air = of(&p, "H2O").airborne().value();
        if let Some(before) = last {
            assert!(
                (air / before - 1.0).abs() < 1e-3,
                "at {t} K: {before} → {air}"
            );
        }
        last = Some(air);
    }
}

#[test]
fn the_dominance_is_a_smooth_step_on_the_air_s_share() {
    assert_same_bits(dominance(DOMINANCE_ONSET), 0.0);
    assert_same_bits(dominance(DOMINANCE_FULL), 1.0);
    assert_same_bits(dominance(f64::NAN), 0.0);
    assert_same_bits(dominance(-1.0), 0.0);
    assert_same_bits(dominance(2.0), 1.0);
    let mut last = 0.0;
    for i in 0..=1_000 {
        let d = dominance(f64::from(i) / 1_000.0);
        assert!(d >= last && (d - last) < 2.0e-3, "{i}");
        last = d;
    }
}

#[test]
fn the_condensates_sort_by_reservoir_then_registry_order() {
    let ice = |key: &str, kg: f64| Condensate {
        substance: id(key),
        phase: CondensatePhase::Solid,
        reservoir: KilogramsPerSquareMetre::new(kg),
        lies_at: Kelvin::new(40.0),
        seasonal: false,
        phases: CondensatePhases::default().with(CondensatePhase::Solid),
    };
    let mut list = [
        ice("Kr", 5.0),
        ice("H2O", 1.0),
        ice("Ar", 5.0),
        ice("N2", 9.0),
    ];
    list.sort_by(reservoir_order);
    let order: Vec<SubstanceId> = list.iter().map(Condensate::substance).collect();
    assert_eq!(order, [id("N2"), id("Ar"), id("Kr"), id("H2O")]);
    // A body's list is in that order.
    let p = partition(
        &span(40.0, 45.0, 50.0),
        gravity(1.0),
        &inventory(&[("Kr", 1e4), ("Ar", 1e4), ("H2O", 1e6)]),
    );
    assert_eq!(p.condensates()[0].substance(), id("H2O"));
    assert!(
        p.condensates()
            .windows(2)
            .all(|w| reservoir_order(&w[0], &w[1]) == Ordering::Less)
    );
}

#[test]
fn the_climate_and_the_inventory_are_validated() {
    let k = Kelvin::new;
    assert_eq!(
        ClimateSpan::new(k(100.0), k(90.0), k(110.0)),
        Err(BuildClimateSpanError::OutOfOrder)
    );
    assert_eq!(
        ClimateSpan::new(k(f64::NAN), k(90.0), k(110.0)),
        Err(BuildClimateSpanError::NotATemperature)
    );
    assert_eq!(
        ClimateSpan::new(k(-1.0), k(90.0), k(110.0)),
        Err(BuildClimateSpanError::NotATemperature)
    );
    assert_eq!(
        ClimateSpan::new(k(80.0), k(90.0), k(f64::INFINITY)),
        Err(BuildClimateSpanError::NotATemperature)
    );
    let zero = ClimateSpan::new(k(-0.0), k(0.0), k(0.0)).unwrap();
    assert_same_bits(zero.coldest().value(), 0.0);
    assert_same_bits(ClimateSpan::isothermal(k(f64::NAN)).mean().value(), 0.0);
    assert_same_bits(ClimateSpan::isothermal(k(-0.0)).mean().value(), 0.0);
    assert_same_bits(
        ClimateSpan::isothermal(k(f64::INFINITY)).mean().value(),
        f64::MAX,
    );
    let water = id("H2O");
    assert_eq!(
        ColumnInventory::new([(water, Pascals::new(1.0)), (water, Pascals::new(2.0))]),
        Err(BuildColumnInventoryError::Repeated(water))
    );
    assert_eq!(
        ColumnInventory::new([(water, Pascals::new(-1.0))]),
        Err(BuildColumnInventoryError::NotAWeight(water))
    );
    // Basalt has no molar mass; silica has one, but no gas columns.
    for key in ["basalt", "SiO2"] {
        assert_eq!(
            ColumnInventory::new([(id(key), Pascals::new(1.0))]),
            Err(BuildColumnInventoryError::NotAGas(id(key))),
            "{key}"
        );
    }
    // Taken in registry order whatever the order given.
    let inv = inventory(&[("N2", 1.0), ("H2O", 2.0)]);
    let order: Vec<SubstanceId> = inv.species().map(|(s, _)| s).collect();
    assert_eq!(order, [water, id("N2")]);
    // A gravity that is not positive leaves every reservoir zero.
    let mars = mars();
    let flat = partition(&mars.climate, gravity(0.0), &mars.inventory);
    assert!(!flat.condensates().is_empty());
    for c in flat.condensates() {
        assert_same_bits(c.reservoir().value(), 0.0);
    }
}

/// The plan's bodies' partitions, pinned bit for bit, so that the native and WebAssembly targets
/// are compared on them before the bump that first reads them (the sim-determinism skill). Nothing
/// on the record calls the partition at generator version 22, so this golden is re-blessed with the
/// rules it pins, at the version in force, until P14.T54.a.
#[test]
fn the_partition_is_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for body in bodies() {
        let p = body.partition();
        let name = body.name;
        w.f64(
            &format!("{name} surface pressure"),
            p.surface_pressure().value(),
        );
        for s in p.species() {
            let label = format!("{name} {}", s.substance().substance().key().as_str());
            w.f64(&format!("{label} airborne"), s.airborne().value());
            w.f64(&format!("{label} partial"), s.partial_pressure().value());
            w.f64(&format!("{label} dominance"), s.dominance());
            w.f64(&format!("{label} lies at"), s.lies_at().value());
        }
        for c in p.condensates() {
            let label = format!("{name} {}", c.substance().substance().key().as_str());
            w.f64(&format!("{label} reservoir"), c.reservoir().value());
            let phases: Vec<&str> = [
                (CondensatePhase::Solid, "solid"),
                (CondensatePhase::Liquid, "liquid"),
                (CondensatePhase::Supercritical, "supercritical"),
            ]
            .into_iter()
            .filter(|&(phase, _)| c.phases().contains(phase))
            .map(|(_, name)| name)
            .collect();
            w.line(&format!(
                "{label} condensate = {:?}, seasonal {}, phases {}",
                c.phase(),
                c.is_seasonal(),
                phases.join(" ")
            ));
        }
    }
    golden!("planetary/partition", w.as_str());
}
