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

fn gravity(g: f64) -> Ground {
    Ground::without_basins(MetresPerSecondSquared::new(g))
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
        partition(&self.climate, &gravity(self.gravity), &self.inventory)
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
/// lie below pure methane's 90.69 K triple point; its seas are a solution, liquid down to the
/// methane–ethane eutectic at 71.15 K (decision-p14-t49b-condensed §2.4), which the partition reads
/// from the registry's first melt ([`titan_s_seas_are_liquid_down_to_their_first_melt`]).
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
        let ice = partition(&cold, &g, &alone(lots));
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
        let sea = partition(&span(t, t + 1.0, t + 2.0), &g, &alone(lots));
        assert_eq!(
            sea.condensate(substance).unwrap().phase(),
            CondensatePhase::Liquid,
            "{key}"
        );
        // Above the critical point: no condensed reservoir; supercritical above its pressure.
        let hot = span(1.1 * critical, 1.2 * critical, 1.3 * critical);
        let fluid = partition(&hot, &g, &alone(lots));
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
            let minor = partition(&cold, &g, &inv);
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
        let p = partition(&climate, &g, &inv);
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
                let reservoir = c.reservoir().value() * g.gravity().value();
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
        let p = partition(&climate, &g, &inv);
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
        let ground = gravity(3.0);
        a.solve(climate, &ground, inv);
        b.solve_sweeping(climate, &ground, inv, 256);
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
    // With the own seas weighed: the ocean worlds and the cold methane world over Earth's basins.
    let seas = |climate: &ClimateSpan, inv: &ColumnInventory| {
        let (mut a, mut b) = (AirborneWeights::new(), AirborneWeights::new());
        let ground = on_earth(EARTH_GRAVITY);
        a.solve(climate, &ground, inv);
        b.solve_sweeping(climate, &ground, inv, 256);
        for (x, y) in a.weights().iter().zip(b.weights()) {
            let scale = x.value().abs().max(y.value().abs());
            assert!(
                (x.value() - y.value()).abs() <= 1e-12 * scale,
                "{x:?} {y:?}"
            );
        }
    };
    for (t, n) in [(360.0, 1e5), (373.0, 1e5), (310.0, 1e4)] {
        let (climate, inv) = ocean_world(t, n);
        seas(&climate, &inv);
    }
    let cold = cold_world();
    seas(&cold.climate, &cold.inventory);
    eprintln!("the fixed sweeps miss 256 by at most {worst:e}");
    // Two solves of one scratch read nothing of each other.
    let (climate, inv) = (earth().climate, earth().inventory);
    let ground = gravity(EARTH_GRAVITY);
    fixed.solve(&climate, &ground, &inv);
    run_out.solve(&span(30.0, 40.0, 50.0), &ground, &inv);
    run_out.solve(&climate, &ground, &inv);
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
            format!("{:?}", partition(climate, &g, inv)),
            format!("{:?}", partition(climate, &g, inv))
        );
    }
    let shared = RefCell::new(AirborneWeights::new());
    assert_order_independent(&cases, |(climate, inv)| {
        let mut weights = shared.borrow_mut();
        weights.solve(climate, &gravity(3.0), inv);
        format!("{:?}", weights.weights())
    });
    // With the own seas weighed, and their published entries.
    let mut seas = cases;
    seas.extend([(360.0, 1e5), (373.0, 1e5), (310.0, 1e4)].map(|(t, n)| ocean_world(t, n)));
    let ground = on_earth(EARTH_GRAVITY);
    assert_order_independent(&seas, |(climate, inv)| {
        let mut weights = shared.borrow_mut();
        weights.solve(climate, &ground, inv);
        format!("{:?}", weights.weights())
    });
    let profile = linear_cold_area(100.0, 300.0);
    for (climate, inv) in &seas {
        let published = || {
            let p = partition(climate, &ground, inv);
            let water = water_cover(&p, &profile, 1.0);
            let cover = Cover {
                cold_area: &profile,
                state: SurfaceState::Temperate,
                water,
            };
            format!("{:?}", p.publish(&cover))
        };
        assert_eq!(published(), published());
    }
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
        let p = partition(&climate, &g, &inventory(&[("CO2", w), ("N2", 1e5)]));
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
            &g,
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
        seasonal_phases: CondensatePhases::default(),
        liquid: id(key),
        first_melt: Kelvin::new(100.0),
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
        &gravity(1.0),
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
    let flat = partition(&mars.climate, &gravity(0.0), &mars.inventory);
    assert_ne!(flat.condensates(), []);
    for c in flat.condensates() {
        assert_same_bits(c.reservoir().value(), 0.0);
    }
}

/// The inventory a partition `p` was solved for: each species' inventory weight.
fn inventory_of(p: &Partition) -> ColumnInventory {
    ColumnInventory::new(p.species().iter().map(|s| (s.substance(), s.inventory()))).unwrap()
}

/// The plan's bodies' partitions, pinned bit for bit, so that the native and WebAssembly targets
/// are compared on them before the bump that first reads them (the sim-determinism skill). Nothing
/// on the record calls the partition at generator version 22, so this golden is re-blessed with the
/// rules it pins, at the version in force, until P14.T54.a.
#[test]
fn the_partition_is_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut cases: Vec<(String, Partition)> = bodies()
        .into_iter()
        .map(|body| (body.name.to_owned(), body.partition()))
        .collect();
    // P14.T51.a's second part: the ocean worlds over Earth's basins, their own seas weighed.
    for (mean, nitrogen) in [(360.0, 1e5), (373.0, 1e5), (310.0, 1e4)] {
        let (climate, inv) = ocean_world(mean, nitrogen);
        let name = format!("ocean world at {mean} K under {nitrogen} Pa");
        cases.push((name, partition(&climate, &on_earth(EARTH_GRAVITY), &inv)));
    }
    let profile = linear_cold_area(260.0, 325.0);
    for (name, p) in &cases {
        let name = name.as_str();
        w.f64(
            &format!("{name} surface pressure"),
            p.surface_pressure().value(),
        );
        for s in p.species() {
            let label = format!("{name} {}", s.substance().substance().key().as_str());
            w.f64(&format!("{label} airborne"), s.airborne().value());
            w.f64(&format!("{label} partial"), s.partial_pressure().value());
            w.f64(&format!("{label} dominance"), s.dominance());
            w.f64(&format!("{label} own ocean weight"), s.own_ocean_weight());
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
        // Its published entries over Earth's basins and the by-hand cover.
        let on_basins = partition(
            p.climate(),
            &on_earth(p.ground().gravity().value()),
            &inventory_of(p),
        );
        let cover = Cover {
            cold_area: &profile,
            state: SurfaceState::Temperate,
            water: water_cover(&on_basins, &profile, 1.0),
        };
        for e in on_basins.publish(&cover).entries() {
            let label = format!(
                "{name} published {} {:?}",
                e.substance().substance().key().as_str(),
                e.phase()
            );
            w.f64(&format!("{label} reservoir"), e.reservoir().value());
            match e.reservoir_gel() {
                Some(gel) => w.f64(&format!("{label} layer"), gel.value()),
                None => w.line(&format!("{label} layer = None")),
            }
            w.f64(&format!("{label} area"), e.area_fraction());
            w.line(&format!("{label} seasonal = {}", e.is_seasonal()));
        }
    }
    golden!("planetary/partition", w.as_str());
}

// P14.T51.a's second part: the own-liquid ocean weight, the solutions' first melt, the per-phase
// seasonal flag and the published condensates.

use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::hooks::figures::cover;
use crate::planetary::hooks::figures::relief::{Relief, ReliefInputs};
use crate::planetary::hooks::figures::{Crust, TectonicRegime};
use crate::substance::{SubstanceKind, ice_ih_density};
use crate::units::PerSquareKilometre;

/// Earth's basins: its relief as P14.T48.a builds it, Earth2014's two crustal populations at its
/// measured continental fraction (`Hypsometry::fill`'s example), which its 2.6 km of ocean fills
/// to 73%.
fn earth_basins() -> Basins {
    Basins::of(earth_relief().hypsometry())
}

/// Earth's relief ([`earth_basins`]).
fn earth_relief() -> Relief {
    Relief::of(&ReliefInputs {
        regime: TectonicRegime::MobileLid,
        continental_fraction: 0.405,
        crust: Crust::Silicate,
        surface_gravity: MetresPerSecondSquared::new(EARTH_GRAVITY),
        radius: Metres::new(6.371e6),
        crater_density: PerSquareKilometre::ZERO,
        volcanism: 0.0,
        lithosphere: Metres::ZERO,
    })
}

/// The ground of a body of surface gravity `g` over Earth's basins, its water all at the surface.
fn on_earth(g: f64) -> Ground {
    Ground::new(MetresPerSecondSquared::new(g), earth_basins(), 1.0)
}

/// P14.T24.b's water cover of the partition `p` at the surface share `share`, under air on a
/// silicate crust over the profile `profile`: its surface water, its potential ocean on Earth's
/// hypsometry, and its perennial ice below 263.15 K, capped at its sheet of ice Ih at the mean
/// (`cover::ice_fraction`'s rule, by hand).
fn water_cover(p: &Partition, profile: &dyn Fn(Kelvin) -> f64, share: f64) -> WaterCover {
    let sigma = p
        .condensate(id("H2O"))
        .map_or(0.0, |c| c.reservoir().value())
        * share;
    let rho = ice_ih_density(p.climate().mean());
    let cap = sigma / (rho.value() * ICE_SHEET_THICKNESS.value());
    let cold = profile(PERENNIAL_ICE_TEMPERATURE);
    WaterCover {
        surface_water: KilogramsPerSquareMetre::new(sigma),
        potential_ocean: cover::potential_ocean(
            earth_relief().hypsometry(),
            KilogramsPerSquareMetre::new(sigma),
        ),
        ice_fraction: if cold < cap { cold } else { cap },
        ice_density: rho,
        crust_water: CrustWater::Unlisted,
    }
}

/// An ocean world at a mean of `mean`, its climate Earth's, 27 K colder at its coldest zone and 17
/// K warmer at its warmest (science-p14-t24a-climate.md §3's 261 to 305 K about 288 K, Earth's with
/// its full ocean in its contrasts), with Earth's oceans and `nitrogen` Pa of nitrogen's weight.
fn ocean_world(mean: f64, nitrogen: f64) -> (ClimateSpan, ColumnInventory) {
    (
        span(mean - 27.0, mean, mean + 17.0),
        inventory(&[("H2O", earth_weight(1.4e21)), ("N2", nitrogen)]),
    )
}

/// Ocean worlds hot or under a thin air hold their water's vapour over their own ocean at the mean,
/// between Manabe and Wetherald's 0.77 and Pierrehumbert and Ding's 1 of its saturation there
/// (decision-p14-t51a-partition §4): at 360 and 373 K under a bar of nitrogen, and at 310 K under
/// 0.1 bar. Without its basins, the same water is pulled toward the cold trap, below 0.77.
#[test]
fn an_ocean_world_holds_its_water_over_its_own_ocean() {
    for (mean, nitrogen) in [(360.0, 1e5), (373.0, 1e5), (310.0, 1e4)] {
        let (climate, inv) = ocean_world(mean, nitrogen);
        let world = partition(&climate, &on_earth(EARTH_GRAVITY), &inv);
        let water = of(&world, "H2O");
        assert!(water.dominance() > 0.0, "{mean} K: {water:?}");
        assert_same_bits(water.own_ocean_weight(), 1.0);
        assert_same_bits(water.lies_at().value(), mean);
        let held = water.partial_pressure().value() / sat("H2O", mean);
        assert!((0.77..=1.0).contains(&held), "{mean} K: {held} of p_sat");
        let pulled = partition(&climate, &gravity(EARTH_GRAVITY), &inv);
        let pulled = of(&pulled, "H2O").partial_pressure().value() / sat("H2O", mean);
        assert!(pulled < 0.77, "{mean} K without its basins: {pulled}");
    }
}

/// The plan's bodies hold as in part 1 (decision-p14-t51a-partition §4): none of them has a
/// condensable both held toward its cold trap and liquid at its mean, so its basins move nothing.
#[test]
fn the_plan_s_bodies_are_unmoved_by_their_own_seas() {
    for body in bodies() {
        let without = partition(&body.climate, &gravity(body.gravity), &body.inventory);
        let with = partition(&body.climate, &on_earth(body.gravity), &body.inventory);
        assert_eq!(
            format!("{:?} {:?}", without.species(), without.condensates()),
            format!("{:?} {:?}", with.species(), with.condensates()),
            "{}",
            body.name
        );
        for s in with.species() {
            assert_same_bits(s.own_ocean_weight(), 0.0);
        }
    }
}

/// The partition is continuous in inventory through a sea's first formation (§4): a hot ocean
/// world's water from a dry world's to an ocean's, its weight c rising from 0 with its sea.
#[test]
fn the_partition_is_continuous_through_a_sea_s_first_formation() {
    let ground = on_earth(EARTH_GRAVITY);
    let climate = span(346.0, 373.0, 390.0);
    let steps = 4_000;
    let (mut last, mut weighed) = (None::<f64>, (false, false));
    for i in 0..=steps {
        let w = math::exp10(3.0 + 5.0 * f64::from(i) / f64::from(steps));
        let p = partition(&climate, &ground, &inventory(&[("H2O", w), ("N2", 1e5)]));
        let water = of(&p, "H2O");
        let c = water.own_ocean_weight();
        weighed = (weighed.0 || c <= 0.0, weighed.1 || c >= 1.0);
        let air = water.airborne().value();
        if let Some(before) = last {
            assert!(
                (air / before - 1.0).abs() < 0.03,
                "at {w}: {before} → {air}"
            );
        }
        last = Some(air);
    }
    assert_eq!(
        weighed,
        (true, true),
        "the sweep crosses the sea's whole weight"
    );
}

/// Titan's seas are its methane's solution with its air's nitrogen and the ethane its methane
/// makes, `ch4_c2h6_n2`, liquid down to the methane–ethane eutectic, 71.15 K
/// (decision-p14-t49b-condensed §2.4): its poles at 89.7 K hold liquid, not ice, and the seas are
/// perennial. Below the eutectic they freeze.
#[test]
fn titan_s_seas_are_liquid_down_to_their_first_melt() {
    let body = titan();
    let titan = body.partition();
    let seas = condensate(&titan, "CH4");
    assert_eq!(seas.liquid(), id("ch4_c2h6_n2"));
    assert_same_bits(seas.first_melt().value(), 71.15);
    assert_eq!(seas.phase(), CondensatePhase::Liquid);
    assert!(!seas.phases().contains(CondensatePhase::Solid), "{seas:?}");
    assert!(!seas.is_seasonal() && !seas.is_seasonal_as(CondensatePhase::Liquid));
    // A colder Titan's poles, below the eutectic, freeze its seas there.
    let cold = partition(&span(70.0, 93.65, 93.65), &gravity(1.352), &body.inventory);
    let seas = condensate(&cold, "CH4");
    assert!(seas.phases().contains(CondensatePhase::Solid), "{seas:?}");
    // Pure methane, with no nitrogen to dissolve, is its own liquid from its triple point.
    let pure = partition(&body.climate, &gravity(1.352), &inventory(&[("CH4", 1e6)]));
    let pure = condensate(&pure, "CH4");
    assert_eq!(pure.liquid(), id("CH4"));
    assert_same_bits(pure.first_melt().value(), triple("CH4"));
}

/// The triple temperature of the row `key`'s phase diagram, K.
fn triple(key: &str) -> f64 {
    id(key)
        .substance()
        .phase_diagram()
        .unwrap()
        .triple_temperature()
        .value()
}

/// Methane's liquid is Titan's solution only where nitrogen is the largest gas of its air besides
/// it (decision-p14-t51a-part2 §2.4): at a mean of 85 K under 1.5 bar of nitrogen its sea is liquid,
/// where pure methane would be ice; under a bar of hydrogen with 1% of nitrogen its sea is pure
/// methane at 95 K, and ice at 85 K. Its ice is methane's under any air, and water's entries are
/// water under every air of the plan's bodies.
#[test]
fn methane_s_liquid_is_titan_s_solution_only_under_nitrogen() {
    let profile = linear_cold_area(60.0, 100.0);
    let entries = |climate: ClimateSpan, inv: &ColumnInventory| {
        let p = partition(&climate, &gravity(1.352), inv);
        let water = water_cover(&p, &profile, 1.0);
        p.publish(&Cover {
            cold_area: &profile,
            state: SurfaceState::Snowball,
            water,
        })
    };
    let under_nitrogen = inventory(&[("CH4", 1e4), ("N2", 1.5e5)]);
    let sea = entries(span(80.0, 85.0, 90.0), &under_nitrogen);
    let solution = sea.entry(id("ch4_c2h6_n2"), CondensatePhase::Liquid);
    assert!(solution.is_some(), "{sea:?}");
    let alone = entries(span(80.0, 85.0, 90.0), &inventory(&[("CH4", 1e6)]));
    assert!(
        alone.entry(id("CH4"), CondensatePhase::Solid).is_some(),
        "{alone:?}"
    );
    let mut hydrogen = weights_of(1e5, &[("H2", 0.99), ("N2", 0.01)]);
    hydrogen.push(("CH4", 1e6));
    let hydrogen = inventory(&hydrogen);
    let warm = entries(span(93.0, 95.0, 97.0), &hydrogen);
    assert!(
        warm.entry(id("CH4"), CondensatePhase::Liquid).is_some(),
        "{warm:?}"
    );
    assert!(
        warm.entry(id("ch4_c2h6_n2"), CondensatePhase::Liquid)
            .is_none()
    );
    let cold = entries(span(80.0, 85.0, 90.0), &hydrogen);
    assert!(
        cold.entry(id("CH4"), CondensatePhase::Solid).is_some(),
        "{cold:?}"
    );
    for body in [triton(), pluto()] {
        let p = body.partition();
        let published = p.publish(&Cover {
            cold_area: &profile,
            state: SurfaceState::Airless,
            water: water_cover(&p, &profile, 1.0),
        });
        let ice = published.entry(id("CH4"), CondensatePhase::Solid);
        assert!(ice.is_some(), "{}: {published:?}", body.name);
    }
    for body in bodies() {
        let p = partition(&body.climate, &on_earth(body.gravity), &body.inventory);
        let published = p.publish(&Cover {
            cold_area: &profile,
            state: SurfaceState::Temperate,
            water: water_cover(&p, &profile, 1.0),
        });
        for e in published.entries() {
            let key = e.substance().substance().key();
            assert!(!key.as_str().starts_with("brine"), "{}: {e:?}", body.name);
            if e.phase() == CondensatePhase::Liquid && key.as_str().contains("H2O") {
                assert_eq!(e.substance(), id("H2O"));
            }
        }
    }
}

/// Each row of the air's solutions is a solution of the registry whose first melt lies below its
/// solvent's triple point, so that it extends its liquid (decision-p14-t51a-part2 §2.2).
#[test]
fn the_air_solutions_extend_their_solvents_liquid() {
    for row in AIR_SOLUTIONS {
        let solution = id(row.solution).substance();
        assert_eq!(solution.kind(), SubstanceKind::Solution, "{row:?}");
        let melt = solution.condensed().unwrap().transition().temperature();
        assert!(melt.value() < triple(row.solvent), "{row:?}");
        assert!(id(row.background).gas().is_some(), "{row:?}");
    }
}

/// An entry is seasonal where the warmest zone takes its phase off the surface
/// (decision-p14-t51a-partition §3): Earth's ocean is perennial and its ice seasonal, Mars's carbon
/// dioxide ice sublimes in its warmest zone, and the 10-bar carbon dioxide world's sea boils there.
#[test]
fn each_phase_is_seasonal_where_the_warmest_zone_takes_it_off() {
    let earth = earth().partition();
    let water = condensate(&earth, "H2O");
    assert!(!water.is_seasonal_as(CondensatePhase::Liquid));
    assert!(water.is_seasonal_as(CondensatePhase::Solid));
    let mars = mars().partition();
    assert!(condensate(&mars, "CO2").is_seasonal_as(CondensatePhase::Solid));
    assert!(!condensate(&mars, "CO2").is_seasonal_as(CondensatePhase::Liquid));
    let sea = carbon_dioxide_sea().partition();
    let sea = condensate(&sea, "CO2");
    assert_eq!(
        sea.is_seasonal_as(CondensatePhase::Liquid),
        sea.is_seasonal()
    );
    let venus = venus("Venus at 92 bar", 9.2e6).partition();
    let fluid = condensate(&venus, "CO2");
    assert!(!fluid.is_seasonal_as(CondensatePhase::Supercritical));
}

/// A cover whose annual-mean profile runs linearly in area from `cold` at its pole to `warm` at its
/// equator, by hand: its share below a temperature.
fn linear_cold_area(cold: f64, warm: f64) -> impl Fn(Kelvin) -> f64 {
    move |t: Kelvin| ((t.value() - cold) / (warm - cold)).clamp(0.0, 1.0)
}

/// Earth publishes its ocean and its ice as P14.T24.b's to the bit (decision-p14-t51a-part2 §4.2):
/// the ice T24.b's perennial ice below 263.15 K at its sheet's mass, the ocean the rest, over the
/// share of the basins its own layer fills, to the bit, below the potential ocean of the whole
/// (decision-p14-t54a-water-consistency §1.4); the two summing to its surface water; each entry's
/// layer its reservoir over its grain density. A dry Earth's water at T24.b's 2% surface share
/// publishes 2% of it.
#[test]
fn earth_publishes_its_ocean_and_its_ice() {
    let body = earth();
    let earth = partition(&body.climate, &on_earth(EARTH_GRAVITY), &body.inventory);
    // About 5% of the surface below 263.15 K, as Earth's perennial ice.
    let profile = linear_cold_area(260.0, 325.0);
    let publish = |share: f64| {
        let water = water_cover(&earth, &profile, share);
        let cover = Cover {
            cold_area: &profile,
            state: SurfaceState::Temperate,
            water,
        };
        (earth.publish(&cover), water)
    };
    let (published, cover) = publish(1.0);
    let water = id("H2O");
    let ocean = published.entry(water, CondensatePhase::Liquid).unwrap();
    let ice = published.entry(water, CondensatePhase::Solid).unwrap();
    assert!(!ocean.is_seasonal() && ice.is_seasonal());
    assert_same_bits(ice.area_fraction(), cover.ice_fraction);
    let own = ocean
        .reservoir_gel()
        .expect("liquid water's density is read");
    assert_same_bits(ocean.area_fraction(), earth_basins().area_filled(own));
    assert!(
        ocean.area_fraction() < cover.potential_ocean,
        "{ocean:?}: {cover:?}"
    );
    eprintln!(
        "Earth's sea {:.6} against the whole water's potential ocean {:.6}",
        ocean.area_fraction(),
        cover.potential_ocean
    );
    let sheet = cover.ice_fraction * cover.ice_density.value() * ICE_SHEET_THICKNESS.value();
    assert_same_bits(ice.reservoir().value(), sheet);
    let layer = ice.reservoir_gel().unwrap().value();
    assert!(relative(layer, cover.ice_fraction * ICE_SHEET_THICKNESS.value()) < 1e-12);
    let whole = condensate(&earth, "H2O").reservoir().value();
    let total = ocean.reservoir().value() + ice.reservoir().value();
    assert!(relative(total, whole) < 1e-12);
    assert!((0.70..0.76).contains(&ocean.area_fraction()), "{ocean:?}");
    assert_eq!(
        published.surface_liquids(),
        [(water, ocean.area_fraction())]
    );
    assert_eq!(published.surface_ices(), [(water, ice.area_fraction())]);
    assert_eq!(published.entries()[0].phase(), CondensatePhase::Liquid);
    let (dry, _) = publish(0.02);
    let total = dry
        .entries()
        .iter()
        .fold(0.0, |sum, e| sum + e.reservoir().value());
    assert!(relative(total, 0.02 * whole) < 1e-12, "{dry:?}");
}

/// Earth's partition, whose water lies as a liquid, published under the profile `profile` with
/// the surface water `sigma`, kg m⁻², all of it at the surface: T24.b's water cover by hand (the
/// ice capped at its sheet on a silicate crust, `cover::ice_fraction_over`'s rule), over Earth's
/// basins.
fn earth_with_water(profile: &dyn Fn(Kelvin) -> f64, sigma: f64) -> (Condensates, WaterCover) {
    let body = earth();
    let earth = partition(&body.climate, &on_earth(EARTH_GRAVITY), &body.inventory);
    let rho = ice_ih_density(earth.climate().mean());
    let cap = sigma / (rho.value() * ICE_SHEET_THICKNESS.value());
    let cold = profile(PERENNIAL_ICE_TEMPERATURE);
    let water = WaterCover {
        surface_water: KilogramsPerSquareMetre::new(sigma),
        potential_ocean: cover::potential_ocean(
            earth_relief().hypsometry(),
            KilogramsPerSquareMetre::new(sigma),
        ),
        ice_fraction: if cold < cap { cold } else { cap },
        ice_density: rho,
        crust_water: CrustWater::Unlisted,
    };
    let published = earth.publish(&Cover {
        cold_area: profile,
        state: SurfaceState::Temperate,
        water,
    });
    (published, water)
}

/// Decision-p14-t54a-water-consistency §1.4 (b): where T24.b's cap binds, a silicate temperate
/// body's ice holds its whole surface water, to the bit, and it publishes no sea, so that its
/// ocean fraction is 0: at 200 reservoirs from one like `01ffeb2ce0000000.0300`'s,
/// 1.68 × 10⁵ kg m⁻², and one ulp either side of each, where the sheet's mass alone, the rule
/// before, left some a sea of an ulp over the whole water's potential ocean.
#[test]
fn the_ice_holds_the_whole_water_where_its_cap_binds() {
    // Two-thirds of the surface below 263.15 K: the cap binds.
    let profile = linear_cold_area(200.0, 300.0);
    let water = id("H2O");
    let reservoirs = (0..200_u32).flat_map(|k| {
        let sigma = 1.68e5 * (1.0 + f64::from(k) * 1e-6);
        [f64::next_down(sigma), sigma, f64::next_up(sigma)]
    });
    let mut seas_before = 0;
    for sigma in reservoirs {
        let (published, cover) = earth_with_water(&profile, sigma);
        let rho_h = cover.ice_density.value() * ICE_SHEET_THICKNESS.value();
        assert!(profile(PERENNIAL_ICE_TEMPERATURE) >= sigma / rho_h);
        let ice = published.entry(water, CondensatePhase::Solid).unwrap();
        assert_same_bits(ice.reservoir().value(), sigma);
        assert_same_bits(ice.area_fraction(), cover.ice_fraction);
        assert!(
            published.entry(water, CondensatePhase::Liquid).is_none(),
            "{sigma}: {published:?}"
        );
        assert!(published.surface_liquids().is_empty(), "{sigma}");
        // The rule before: the sheet's mass, then the rest as sea.
        let sheet = cover.ice_fraction * cover.ice_density.value() * ICE_SHEET_THICKNESS.value();
        seas_before += usize::from(sheet < sigma);
    }
    eprintln!("the sheet's mass alone left a sea of an ulp at {seas_before} of 600 reservoirs");
    assert!(seas_before > 0, "the regression is exercised");
}

/// Decision-p14-t54a-water-consistency §1.4 (a): with no ice, water's sea is the share of the
/// basins its whole surface water fills at its liquid's density where it lies, to the bit, its
/// own layer's fill.
#[test]
fn with_no_ice_the_sea_is_the_whole_water_s_fill() {
    // No zone below 263.15 K.
    let profile = linear_cold_area(270.0, 330.0);
    let sigma = 2.7e6;
    let (published, cover) = earth_with_water(&profile, sigma);
    assert_same_bits(cover.ice_fraction, 0.0);
    let water = id("H2O");
    assert!(published.entry(water, CondensatePhase::Solid).is_none());
    let sea = published.entry(water, CondensatePhase::Liquid).unwrap();
    let body = earth();
    let earth = partition(&body.climate, &on_earth(EARTH_GRAVITY), &body.inventory);
    let c = earth.condensate(water).unwrap();
    let diagram = water.substance().phase_diagram().unwrap();
    let rho_l = liquid_density_kg_per_m3(c, diagram, earth.surface_pressure()).unwrap();
    assert_same_bits(sea.reservoir().value(), sigma);
    assert_same_bits(
        sea.area_fraction(),
        earth_basins().area_filled(Metres::new(sigma / rho_l)),
    );
    assert!(
        sea.area_fraction() > 0.0 && sea.area_fraction() < 1.0,
        "{sea:?}"
    );
}

/// Decision-p14-t54a-water-consistency §1.4 (a)–(c): as the surface water passes the ice's cap
/// over a fixed cold area, the sea rises from none, never falling, with no step: none at and below
/// the cap's water, some just above it, its own layer's fill.
#[test]
fn the_sea_rises_from_none_as_the_water_passes_the_ice_s_cap() {
    let profile = linear_cold_area(200.0, 300.0);
    let cold = profile(PERENNIAL_ICE_TEMPERATURE);
    let (_, cover) = earth_with_water(&profile, 1.0);
    let crossing = cold * cover.ice_density.value() * ICE_SHEET_THICKNESS.value();
    let water = id("H2O");
    let sea = |sigma: f64| {
        let (published, _) = earth_with_water(&profile, sigma);
        published
            .entry(water, CondensatePhase::Liquid)
            .map_or(0.0, CondensateEntry::area_fraction)
    };
    let mut last = 0.0;
    for step in 0..=400 {
        let sigma = crossing * (0.5 + f64::from(step) / 200.0);
        let area = sea(sigma);
        assert!(area >= last, "{sigma}: {area} after {last}");
        if sigma <= crossing {
            assert_same_bits(area, 0.0);
        }
        last = area;
    }
    let just_above = sea(crossing * (1.0 + 1e-6));
    assert!(just_above > 0.0 && just_above < 1e-3, "{just_above}");
    assert!(last > just_above, "{last}");
}

/// Titan publishes its seas as `ch4_c2h6_n2`, its layer over that liquid's grain density, and its
/// water-ice crust; a Venus at 92 bar its supercritical carbon dioxide with no reservoir or area.
#[test]
fn titan_publishes_its_seas_and_venus_its_fluid() {
    let body = titan();
    let titan = partition(&body.climate, &on_earth(body.gravity), &body.inventory);
    let profile = linear_cold_area(85.0, 95.0);
    let cover = Cover {
        cold_area: &profile,
        state: SurfaceState::Snowball,
        water: water_cover(&titan, &profile, 1.0),
    };
    let published = titan.publish(&cover);
    let seas = published
        .entry(id("ch4_c2h6_n2"), CondensatePhase::Liquid)
        .unwrap();
    let rho = id("ch4_c2h6_n2")
        .substance()
        .grain_density(CondensedPhase::Liquid, Kelvin::new(93.65))
        .unwrap()
        .value();
    let layer = seas.reservoir_gel().unwrap().value();
    assert!(relative(layer, seas.reservoir().value() / rho) < 1e-12);
    assert!(seas.area_fraction() > 0.0 && !seas.is_seasonal());
    assert!(published.entry(id("H2O"), CondensatePhase::Solid).is_some());
    assert!(
        published
            .entry(id("CH4"), CondensatePhase::Liquid)
            .is_none()
    );
    let venus = venus("Venus at 92 bar", 9.2e6);
    let venus = partition(&venus.climate, &on_earth(8.87), &venus.inventory);
    let published = venus.publish(&Cover {
        state: SurfaceState::RunawayGreenhouse,
        ..cover
    });
    let [fluid] = published.entries() else {
        panic!("{published:?}");
    };
    assert_eq!(fluid.phase(), CondensatePhase::Supercritical);
    assert_same_bits(fluid.reservoir().value(), 0.0);
    assert_same_bits(fluid.area_fraction(), 0.0);
}

/// A Mars given its measured air by hand publishes a carbon dioxide ice whose reservoir caps it
/// within (0, 10⁻³] of its surface, against its residual cap's 6 × 10⁻⁴, seasonal, and water ice
/// within T24.b's 0.005–0.02, perennial, a minor condensable's (decision-p14-t51a-part2 §3.7;
/// decision-p14-t51a-partition §3).
#[test]
fn mars_publishes_its_carbon_dioxide_and_its_water_ice() {
    let body = mars();
    let mars = partition(&body.climate, &on_earth(MARS_GRAVITY), &body.inventory);
    // Mars's annual mean, everywhere below 263.15 K and its carbon dioxide's edge.
    let profile = linear_cold_area(150.0, 225.0);
    let published = mars.publish(&Cover {
        cold_area: &profile,
        state: SurfaceState::Temperate,
        water: water_cover(&mars, &profile, 1.0),
    });
    let caps = published.entry(id("CO2"), CondensatePhase::Solid).unwrap();
    let area = caps.area_fraction();
    assert!(area > 0.0 && area <= 1e-3, "{caps:?}");
    let ice = published.entry(id("H2O"), CondensatePhase::Solid).unwrap();
    assert!((0.005..=0.02).contains(&ice.area_fraction()), "{ice:?}");
    assert!(caps.is_seasonal() && !ice.is_seasonal());
    eprintln!(
        "Mars: CO2 ice {:e}, water ice {:e}",
        caps.area_fraction(),
        ice.area_fraction()
    );
}

/// Pluto, airless, given its nitrogen, methane and carbon monoxide by hand, publishes its
/// nitrogen ice over 0.02–0.2 of its surface, its reservoir's at 1.93 km, against Sputnik
/// Planitia's 0.049 (about 8.7 × 10⁵ km² of 1.774 × 10⁷: Moore et al. 2016, via Glein and Waite
/// 2018, Icarus 313, 79; decision-p14-t51a-part2 §8); its methane's edge lies within 0.5 K of its
/// frost point at its own partial pressure (decision-p14-t51a-part2 §3.7).
#[test]
fn pluto_keeps_its_nitrogen_ice() {
    let pluto = pluto().partition();
    let profile = linear_cold_area(35.0, 47.0);
    let published = pluto.publish(&Cover {
        cold_area: &profile,
        state: SurfaceState::Airless,
        water: water_cover(&pluto, &profile, 1.0),
    });
    let nitrogen = published.entry(id("N2"), CondensatePhase::Solid).unwrap();
    let area = nitrogen.area_fraction();
    assert!((0.02..=0.2).contains(&area), "{nitrogen:?}");
    let methane = of(&pluto, "CH4");
    let d = id("CH4").substance().phase_diagram().unwrap();
    let edge = sublimation_edge(
        d,
        molar_mass("CH4"),
        Kelvin::new(triple("CH4")),
        methane.partial_pressure(),
        AIRLESS_ICE_LOSS_KG_PER_M2_GYR,
    );
    let frost = d
        .saturation_temperature(methane.partial_pressure())
        .unwrap();
    let off = (edge.value() - frost.value()).abs();
    assert!(off < 0.5, "{edge:?} against {frost:?}");
    eprintln!("Pluto: N2 ice {area}, CH4 edge {edge:?} against its frost point {frost:?}");
}

/// The solids' edges (decision-p14-t51a-part2 §3.2–3.3): under air, r × the first melt, r =
/// 263.15 ÷ 273.16, stated for N₂, CH₄, CO₂ and NH₃; airless, with no air of their own, the net
/// Hertz–Knudsen edge, water's within 1 K of T24.b's 110 K and carbon dioxide's at 55–59 K (T51.f's
/// about 57 K); continuous in the dominance and the partial pressure.
#[test]
fn the_solids_edges_scale_with_their_first_melt() {
    let edge_of = |key: &str, held: (f64, Pascals), state: SurfaceState| {
        let d = id(key).substance().phase_diagram().unwrap();
        ice_line((d, molar_mass(key)), d.triple_temperature(), held, state).value()
    };
    let none = (0.0, Pascals::ZERO);
    for (key, edge) in [
        ("N2", 60.84),
        ("CH4", 87.37),
        ("CO2", 208.65),
        ("NH3", 188.33),
    ] {
        let line = edge_of(key, none, SurfaceState::Temperate);
        assert!((line - edge).abs() < 0.01, "{key}: {line}");
        assert_same_bits(edge_of(key, none, SurfaceState::Snowball), line);
    }
    let at_vacuum = |key: &str| {
        let d = id(key).substance().phase_diagram().unwrap();
        sublimation_edge(
            d,
            molar_mass(key),
            d.triple_temperature(),
            Pascals::ZERO,
            AIRLESS_ICE_LOSS_KG_PER_M2_GYR,
        )
        .value()
    };
    assert!(
        (at_vacuum("H2O") - 110.0).abs() < 1.0,
        "{}",
        at_vacuum("H2O")
    );
    assert!(
        (55.0..=59.0).contains(&at_vacuum("CO2")),
        "{}",
        at_vacuum("CO2")
    );
    // Continuous in the dominance and in the partial pressure.
    let mut last: Option<f64> = None;
    for i in 0..=1_000 {
        let share = f64::from(i) / 1_000.0;
        let partial = Pascals::new(math::exp10(-12.0 + 12.0 * share));
        let line = edge_of("N2", (share, partial), SurfaceState::Airless);
        if let Some(before) = last {
            assert!((line - before).abs() < 0.2, "at {share}: {before} → {line}");
        }
        last = Some(line);
    }
}

/// A reservoir beyond its sheet over its cold area takes the whole cold area, and a smaller one the
/// area its sheet covers (decision-p14-t51a-part2 §3.7).
#[test]
fn a_solid_s_area_is_capped_by_its_reservoir() {
    let profile = linear_cold_area(100.0, 200.0);
    let area = |kg: f64| {
        let p = partition(
            &span(100.0, 150.0, 200.0),
            &gravity(EARTH_GRAVITY),
            &inventory(&[("CO2", kg * EARTH_GRAVITY), ("N2", 1e5)]),
        );
        let published = p.publish(&Cover {
            cold_area: &profile,
            state: SurfaceState::Temperate,
            water: water_cover(&p, &profile, 1.0),
        });
        published
            .entry(id("CO2"), CondensatePhase::Solid)
            .map_or(0.0, CondensateEntry::area_fraction)
    };
    // The cold area below carbon dioxide's 208.65 K edge is the whole profile.
    assert_same_bits(area(1e9), 1.0);
    let small = area(1e4);
    assert!(small > 0.0 && small < 0.01, "{small}");
}

/// A dry body's published water is T24.b's 2% of its condensed water, and its air the same water
/// to the bit as a wet one's (decision-p14-t51a-part2 §4.4).
#[test]
fn a_dry_body_publishes_two_per_cent_of_its_water() {
    let body = mars();
    let ground = |share: f64| {
        Ground::new(
            MetresPerSecondSquared::new(MARS_GRAVITY),
            earth_basins(),
            share,
        )
    };
    let wet = partition(&body.climate, &ground(1.0), &body.inventory);
    let dry = partition(&body.climate, &ground(0.02), &body.inventory);
    assert_same_bits(
        of(&wet, "H2O").airborne().value(),
        of(&dry, "H2O").airborne().value(),
    );
    let profile = linear_cold_area(150.0, 225.0);
    let published = dry.publish(&Cover {
        cold_area: &profile,
        state: SurfaceState::Temperate,
        water: water_cover(&dry, &profile, 0.02),
    });
    let water = published
        .entries()
        .iter()
        .filter(|e| e.substance() == id("H2O"))
        .fold(0.0, |sum, e| sum + e.reservoir().value());
    let condensed = condensate(&dry, "H2O").reservoir().value();
    assert!(
        relative(water, 0.02 * condensed) < 1e-12,
        "{water} against {condensed}"
    );
}

/// The published list holds at most eight entries, one for each substance and phase, by reservoir
/// mass, ties by `SubstanceId` then phase, and every share is in [0, 1].
#[test]
fn the_published_list_is_bounded_and_sorted() {
    let profile = linear_cold_area(30.0, 300.0);
    let ground = on_earth(3.0);
    let mut longest = 0;
    for (climate, inv) in cases() {
        let p = partition(&climate, &ground, &inv);
        let published = p.publish(&Cover {
            cold_area: &profile,
            state: SurfaceState::Temperate,
            water: water_cover(&p, &profile, 1.0),
        });
        let entries = published.entries();
        longest = longest.max(entries.len());
        assert!(entries.len() <= MAX_CONDENSATES);
        for pair in entries.windows(2) {
            assert_eq!(
                entry_order(&pair[0], &pair[1]),
                Ordering::Less,
                "{entries:?}"
            );
        }
        for e in entries {
            assert!((0.0..=1.0).contains(&e.area_fraction()), "{e:?}");
            assert!(e.reservoir().value() >= 0.0);
        }
    }
    assert!(longest >= 2);
}

/// The basins weigh a sea continuously from none to a full ocean, at 0.708 of the surface
/// (`FULL_OCEAN_FRACTION`), and the layer they find for it fills that share. A first sea's area
/// grows as the square root of its volume, a Gaussian's deepest floor filling as a basin of
/// finite slope, so its weight is continuous but steep at none.
#[test]
fn the_basins_weigh_a_sea_from_none_to_a_full_ocean() {
    let basins = earth_basins();
    assert_same_bits(Basins::UNKNOWN.ocean_weight(Metres::new(3e3)), 0.0);
    assert_same_bits(basins.ocean_weight(Metres::ZERO), 0.0);
    assert!(basins.ocean_weight(Metres::new(1e-3)) < 1e-3);
    let mut last = 0.0;
    for i in 1..=2_000 {
        let depth = Metres::new(3.0 * f64::from(i));
        let c = basins.ocean_weight(depth);
        assert!(c >= last && c - last < 0.02, "at {depth:?}: {last} → {c}");
        last = c;
    }
    assert_same_bits(last, 1.0);
    let Some(filled) = basins.of else {
        panic!("known basins");
    };
    let area = filled.hypsometry.fill(filled.full_ocean).1;
    assert!((area - FULL_OCEAN_FRACTION).abs() < 1e-9, "{area}");
}

/// A solid whose density the registry has not read (argon's, under 23's rules: P14.T49.n's read
/// is held until 24) is capped at its liquid's density at its triple point and publishes no layer
/// (decision-p14-t51a-part2 §3.4).
#[test]
fn a_solid_whose_density_was_not_read_has_a_stand_in_cap_and_no_layer() {
    let argon = id("Ar");
    let d = argon.substance().phase_diagram().unwrap();
    assert!(
        argon
            .substance()
            .grain_density(CondensedPhase::Solid, Kelvin::new(50.0))
            .is_none()
    );
    let p = partition(
        &span(40.0, 50.0, 60.0),
        &gravity(EARTH_GRAVITY),
        &inventory(&[("Ar", 1e4)]),
    );
    let profile = linear_cold_area(30.0, 70.0);
    let published = p.publish(&Cover {
        cold_area: &profile,
        state: SurfaceState::Temperate,
        water: water_cover(&p, &profile, 1.0),
    });
    let ice = published.entry(argon, CondensatePhase::Solid).unwrap();
    assert_eq!(ice.reservoir_gel(), None);
    let stand_in = argon
        .substance()
        .grain_density(CondensedPhase::Liquid, d.triple_temperature())
        .unwrap()
        .value();
    let cap = ice.reservoir().value() / (stand_in * ICE_SHEET_THICKNESS.value());
    assert!(
        relative(ice.area_fraction(), cap) < 1e-12,
        "{ice:?} against {cap}"
    );
}

/// P14.T49.n's solid argon (fcc), held until 24 (`READ_DENSITIES_VERSION`): under 24's rules the
/// same argon ice is capped at its solid's density where it lies (its `lies_at`, held at most its
/// first melt) and publishes its layer at that density, to the bit; its reservoir is unchanged and
/// its area smaller than the stand-in's by the two densities' ratio. Under 23's rules it is the
/// stand-in's (`a_solid_whose_density_was_not_read_has_a_stand_in_cap_and_no_layer`).
#[test]
fn under_24_s_densities_an_argon_ice_has_its_solid_s_cap_and_layer() {
    use crate::substance::{ReadDensitiesRules, with_read_densities};
    let argon = id("Ar");
    let profile = linear_cold_area(30.0, 70.0);
    let read = || {
        let p = partition(
            &span(40.0, 50.0, 60.0),
            &gravity(EARTH_GRAVITY),
            &inventory(&[("Ar", 1e4)]),
        );
        let published = p.publish(&Cover {
            cold_area: &profile,
            state: SurfaceState::Temperate,
            water: water_cover(&p, &profile, 1.0),
        });
        let c = p.condensate(argon).unwrap();
        let at = if c.lies_at() < c.first_melt() {
            c.lies_at()
        } else {
            c.first_melt()
        };
        let rho = argon
            .substance()
            .grain_density(CondensedPhase::Solid, at)
            .map(KilogramsPerCubicMetre::value);
        let ice = *published.entry(argon, CondensatePhase::Solid).unwrap();
        (ice, at, rho)
    };
    let (held, held_at, none) = read();
    assert_eq!(none, None);
    let (ice, at, rho) = with_read_densities(ReadDensitiesRules::Read, read);
    assert_same_bits(at.value(), held_at.value());
    let rho = rho.expect("solid argon's density is read at 24");
    assert_same_bits(ice.reservoir().value(), held.reservoir().value());
    assert_same_bits(
        ice.reservoir_gel().expect("its layer is published").value(),
        ice.reservoir().value() / rho,
    );
    let cap = ice.reservoir().value() / (rho * ICE_SHEET_THICKNESS.value());
    assert!(
        relative(ice.area_fraction(), cap) < 1e-12,
        "{ice:?} against {cap}"
    );
    let triple = argon
        .substance()
        .phase_diagram()
        .unwrap()
        .triple_temperature();
    let stand_in = argon
        .substance()
        .grain_density(CondensedPhase::Liquid, triple)
        .unwrap()
        .value();
    assert!((1_550.0..1_800.0).contains(&rho), "{rho} kg m⁻³ at {at:?}");
    assert!(
        relative(ice.area_fraction() / held.area_fraction(), stand_in / rho) < 1e-12,
        "{ice:?} against {held:?}"
    );
    eprintln!(
        "an argon ice at {:.3} K: cap {stand_in:.1} → {rho:.1} kg m⁻³, area {:.6e} → {:.6e}, \
         layer none → {:.6e} m",
        at.value(),
        held.area_fraction(),
        ice.area_fraction(),
        ice.reservoir().value() / rho
    );
}

// P14.T51.b: the area-mix Bond albedo over the published condensates of T51.a's bodies by hand.

use crate::planetary::derive::atmosphere::{RunawayDeck, SurfaceMaterial};
use crate::planetary::derive::photometry::area_mix::{
    HostShade, OPTICS_BANDS, Regolith, cloud_albedo, cloud_fraction, solid_bond, surface_bond,
    under_cloud,
};

/// The area-mix Bond albedo of the partition `p`, published over `cover`, of a body of `material`
/// in the cover's state about a Sun: the published condensates over the material's regolith, under
/// the state's cloud.
fn area_mix(p: &Partition, cover: &Cover<'_>, material: SurfaceMaterial) -> (f64, f64) {
    let sun = HostShade::sun();
    let published = p.publish(cover);
    let regolith = Regolith::of_material(material)
        .bond(cover.state, sun.bands())
        .bond;
    let ground = surface_bond(&published, p, (regolith, &[]), &sun);
    let cloud = cloud_fraction((cover.state, RunawayDeck::Kept), p, &sun);
    let bond = under_cloud(
        ground,
        cloud,
        cloud_albedo((cover.state, RunawayDeck::Kept), &sun),
    );
    (bond, cloud)
}

/// T51.a's by-hand Earth over Earth's basins, its cover as `earth_publishes_its_ocean_and_its_ice`
/// builds it (an ocean of 0.70–0.76 and 5% of ice), takes a Bond albedo within 0.01 of NASA's
/// 0.294, by the water cloud's calibration on the reference Earth (`area_mix`'s 0.708 of ocean and
/// 0.0289 of land ice): its ice's larger share and its ocean's own make the difference. Its cloud is
/// Earth's 0.67, its water's column well above the reference's.
#[test]
fn the_by_hand_earth_takes_a_bond_albedo_near_0_294() {
    let body = earth();
    let earth = partition(&body.climate, &on_earth(EARTH_GRAVITY), &body.inventory);
    let profile = linear_cold_area(260.0, 325.0);
    let cover = Cover {
        cold_area: &profile,
        state: SurfaceState::Temperate,
        water: water_cover(&earth, &profile, 1.0),
    };
    let (bond, cloud) = area_mix(&earth, &cover, SurfaceMaterial::Rock);
    assert_same_bits(cloud, SurfaceState::Temperate.cloud_fraction());
    assert!((bond - 0.294).abs() < 0.01, "{bond}");
    eprintln!("T51.a's Earth by hand: Bond albedo {bond:.5} under {cloud} of cloud");
}

/// What a by-hand body's Bond albedo is held to against its measured range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    /// Inside the range.
    Inside,
    /// At or above the range's top: a pure-frost upper bound, the body's darkening material not
    /// modelled, its miss recorded (plan 14's T49.i record, 2026-10-19).
    AtOrAboveItsTop,
}

/// A Pluto and a Triton given their measured N₂, CH₄ and CO by hand (T51.a's fixtures), airless
/// over water frost's regolith about a Sun, against 0.6–0.8 and 0.72–0.92 (Buratti et al. 2017,
/// Icarus 287, 207: Pluto 0.72 ± 0.07; Hillier et al. 1990, Science 250, 419: Triton 0.82 ± 0.05;
/// decision-triton-and-e101 §1.6), asserted since T51.b's area mix weights P14.T49.i's bins over
/// the host's whole spectrum (decision-p14-t49c-split §1.3; main's ruling, 2026-10-19). Their
/// nitrogen ice's frost albedo is the `N2` row's own, not a stand-in. Their covers are by hand,
/// their annual means linear in area from 35 to 47 K, as `pluto_keeps_its_nitrogen_ice`'s.
///
/// - **Triton** lies inside its range, 0.843, but by its regolith's stand-in: its N₂ ice covers
///   0.066 of it (at `N2`'s 1.000) and its CH₄ ice 0.001 (at 0.928), and the rest is the icy
///   regolith's fresh `H2O` frost (`RowByMaterial`, 0.832), not the N₂-covered surface Triton has.
/// - **Pluto** lies above its range, 0.846, a miss recorded and not tuned: fresh frost of 100 µm
///   grains over its whole surface, its N₂ ice over 0.083 and its CH₄ ice over 0.002 and the rest
///   `H2O` frost, with none of the dark, red tholin terrain that lowers the measured body (Cthulhu
///   Macula's, from memory). It is asserted as the bound that holds, at or above the range's top, a
///   pure-frost upper bound, as T49.i's rows are (`N2`, `CH4` and `CO` at or above both tops under
///   a 5,772 K black body).
#[test]
fn a_pluto_and_a_triton_given_their_ices_take_their_bond_albedos() {
    let profile = linear_cold_area(35.0, 47.0);
    for (body, (low, high), anchor) in [
        (pluto(), (0.6, 0.8), Anchor::AtOrAboveItsTop),
        (triton(), (0.72, 0.92), Anchor::Inside),
    ] {
        let p = body.partition();
        let cover = Cover {
            cold_area: &profile,
            state: SurfaceState::Airless,
            water: water_cover(&p, &profile, 1.0),
        };
        let published = p.publish(&cover);
        let nitrogen = published.entry(id("N2"), CondensatePhase::Solid).unwrap();
        let frost = solid_bond(id("N2"), HostShade::sun().bands());
        assert_eq!(frost.stand_in, None, "{}", body.name);
        let (bond, cloud) = area_mix(&p, &cover, SurfaceMaterial::Ice);
        assert_same_bits(cloud, 0.0);
        let entries: Vec<String> = published
            .entries()
            .iter()
            .map(|e| {
                format!(
                    "{} {:?} over {:.4} at {:.5}",
                    e.substance().substance().key(),
                    e.phase(),
                    e.area_fraction(),
                    solid_bond(e.substance(), HostShade::sun().bands()).bond
                )
            })
            .collect();
        eprintln!(
            "{} by hand, over {OPTICS_BANDS} bins: Bond albedo {bond:.5} against {low}–{high}; \
             its N2 ice over {:.3} at {:.5}; its entries {entries:?}",
            body.name,
            nitrogen.area_fraction(),
            frost.bond
        );
        match anchor {
            Anchor::Inside => assert!((low..=high).contains(&bond), "{}: {bond}", body.name),
            Anchor::AtOrAboveItsTop => assert!(bond >= high, "{}: {bond}", body.name),
        }
    }
}

/// T51.a's Mars given its measured air by hand, with its carbon dioxide caps and its water ice,
/// takes almost no cloud: its cloud-forming condensable is carbon dioxide (P14.T48.d's, the largest
/// partial pressure whose phase changes inside the climate, its caps a reservoir), whose airborne
/// column is its whole air's, some 160 kg m⁻², but which is dominant, cold-trapped at its surface
/// (Wordsworth 2015), and weighs by 1 − d (the lane's rule, 2026-10-16), so its cloud falls near
/// none, as the measured Mars's thin water-ice and carbon dioxide clouds have it.
#[test]
fn a_mars_with_its_caps_takes_almost_no_cloud() {
    let body = mars();
    let mars = body.partition();
    let profile = linear_cold_area(140.0, 250.0);
    let cover = Cover {
        cold_area: &profile,
        state: SurfaceState::Temperate,
        water: water_cover(&mars, &profile, cover::DRY_SURFACE_WATER_SHARE),
    };
    let (bond, cloud) = area_mix(&mars, &cover, SurfaceMaterial::Rock);
    let dominance = of(&mars, "CO2").dominance();
    eprintln!(
        "Mars by hand, with its caps: CO2's dominance {dominance:.4}, cloud {cloud:.2e}, Bond \
         albedo {bond:.5}"
    );
    assert!(cloud < 0.05, "{cloud}");
}

/// Lever (p)'s exit from the sweeps (decision-p14-t54a-passes, "Cost escalation (after Batch B)"):
/// a solve stops after a sweep that moves no weight, bit for bit, at most [`PARTITION_SWEEPS`] of
/// them, and is the full count's to the bit. Over every mix, the plan's bodies, the ocean worlds
/// and an empty column, on a bare ground and over Earth's basins, each solve is checked against the
/// full count, weights and caps (`Exits::Checked`), and each partition is the same with the exits
/// and without, whole. Some solves settle short of the cap.
#[test]
fn a_solve_stops_at_its_bit_exact_fixed_point() {
    use std::collections::BTreeMap;

    let mut cases = cases();
    cases.extend([(360.0, 1e5), (373.0, 1e5), (310.0, 1e4)].map(|(t, n)| ocean_world(t, n)));
    let empty = [(id("N2"), Pascals::ZERO), (id("H2O"), Pascals::ZERO)];
    cases.push((span(30.0, 40.0, 50.0), ColumnInventory::new(empty).unwrap()));
    let grounds = [gravity(3.0), on_earth(EARTH_GRAVITY)];
    let run = || -> Vec<String> {
        cases
            .iter()
            .flat_map(|(climate, inv)| {
                grounds
                    .iter()
                    .map(move |ground| format!("{:?}", partition(climate, ground, inv)))
            })
            .collect()
    };
    let (checked, counts) = with_exits(Exits::Checked, run);
    let (unlevered, uncounted) = with_exits(Exits::Unlevered, run);
    assert_eq!(uncounted, ExitCounts::default());
    assert_eq!(checked.len(), unlevered.len());
    for (k, (with, without)) in checked.iter().zip(&unlevered).enumerate() {
        assert_eq!(with, without, "case {k}");
    }
    let solves = u64::try_from(checked.len()).unwrap();
    assert_eq!(counts.solves, solves);
    assert_eq!(counts.checked_solves, solves);
    let mut sweeps: BTreeMap<u64, u32> = BTreeMap::new();
    for (climate, inv) in &cases {
        for ground in &grounds {
            let (_, one) = with_exits(Exits::Taken, || partition(climate, ground, inv));
            assert_eq!(one.solves, 1);
            *sweeps.entry(one.sweeps).or_default() += 1;
        }
    }
    eprintln!("{solves} solves checked; the sweeps a solve takes, by count: {sweeps:?}");
    let least = sweeps.keys().next().copied().unwrap();
    assert!(least < u64::from(PARTITION_SWEEPS), "{sweeps:?}");
    assert_eq!(sweeps.values().sum::<u32>(), u32::try_from(solves).unwrap());
}

/// Decision-p14-t54a-ganymede §C3.1 (Batch D's fix commit, RG1): the record lists an ice bulk's
/// crust water where the row its albedo read is `H2O`, its lid `H2O` or not known, and nowhere
/// else: the ruling's four cases, and every other lid of T51.c's closed set on ice and on rock.
#[test]
fn the_record_lists_an_ice_bulk_s_crust_water_only_under_an_h2o_lid() {
    use crate::planetary::hooks::crust::CrustLithology;

    assert_eq!(
        CrustWater::of_record(SurfaceMaterial::Ice, None),
        CrustWater::Listed
    );
    assert_eq!(
        CrustWater::of_record(SurfaceMaterial::Ice, Some(CrustLithology::Water)),
        CrustWater::Listed
    );
    assert_eq!(
        CrustWater::of_record(SurfaceMaterial::Ice, Some(CrustLithology::Cometary)),
        CrustWater::Unlisted
    );
    assert_eq!(
        CrustWater::of_record(SurfaceMaterial::Rock, Some(CrustLithology::Water)),
        CrustWater::Unlisted
    );
    assert_eq!(
        CrustWater::of_record(SurfaceMaterial::Rock, None),
        CrustWater::Unlisted
    );
    for lid in CrustLithology::ALL {
        let listed = lid == CrustLithology::Water;
        assert_eq!(
            CrustWater::of_record(SurfaceMaterial::Ice, Some(lid)) == CrustWater::Listed,
            listed,
            "{lid:?}"
        );
        assert_eq!(
            CrustWater::of_record(SurfaceMaterial::Rock, Some(lid)),
            CrustWater::Unlisted,
            "{lid:?}"
        );
    }
}

/// A dry cover over the profile `profile`: no surface water, ice Ih's density for a cap.
fn dry_cover(profile: &dyn Fn(Kelvin) -> f64) -> Cover<'_> {
    Cover {
        cold_area: profile,
        state: SurfaceState::Temperate,
        water: WaterCover {
            surface_water: KilogramsPerSquareMetre::ZERO,
            potential_ocean: 0.0,
            ice_fraction: 0.0,
            ice_density: KilogramsPerCubicMetre::new(917.0),
            crust_water: CrustWater::Unlisted,
        },
    }
}

/// P14.T49.n's compression, held until 24 (`COMPRESSED_LIQUIDS_VERSION`), moves the partition's
/// readers of a liquid's density together, each to the registry's law at its own temperature under
/// its own pressure, to the bit (W4: the layer and the area read one volume). Earth's sea, with no
/// ice: its layer the whole surface water over its liquid's density where it lies under the
/// surface pressure, its area that layer's fill of the basins, denser than the saturated liquid's
/// by `κ_T` (`p_s` − `p_sat`) within 1% (3.5 × 10⁻⁵ under the by-hand Earth's 0.77 bar) and so a
/// little smaller. Under 23's rules it is the saturated liquid's, as every other test of this file
/// holds.
#[test]
fn under_compression_earth_s_sea_reads_one_density() {
    use crate::substance::{CompressedLiquidsRules, with_compressed_liquids};
    // Earth's sea: no zone below 263.15 K.
    let profile = linear_cold_area(270.0, 330.0);
    let sigma = 2.7e6;
    let water = id("H2O");
    let sea = || {
        let (published, _) = earth_with_water(&profile, sigma);
        let body = earth();
        let earth = partition(&body.climate, &on_earth(EARTH_GRAVITY), &body.inventory);
        let c = earth.condensate(water).unwrap();
        let diagram = water.substance().phase_diagram().unwrap();
        let rho = liquid_density_kg_per_m3(c, diagram, earth.surface_pressure()).unwrap();
        let law = water
            .substance()
            .compressed_liquid_density(c.lies_at(), earth.surface_pressure())
            .unwrap()
            .value();
        let entry = *published.entry(water, CondensatePhase::Liquid).unwrap();
        (entry, rho, law)
    };
    let (held, held_rho, _) = sea();
    let (moved, rho, law) = with_compressed_liquids(CompressedLiquidsRules::Compressed, sea);
    assert_same_bits(rho, law);
    for (entry, rho) in [(held, held_rho), (moved, rho)] {
        assert_same_bits(entry.reservoir().value(), sigma);
        assert_same_bits(entry.reservoir_gel().unwrap().value(), sigma / rho);
        assert_same_bits(
            entry.area_fraction(),
            earth_basins().area_filled(Metres::new(sigma / rho)),
        );
    }
    let relative = rho / held_rho - 1.0;
    let body = earth();
    let earth = partition(&body.climate, &on_earth(EARTH_GRAVITY), &body.inventory);
    let lies_at = earth.condensate(water).unwrap().lies_at();
    let equation = water
        .substance()
        .liquid()
        .and_then(|columns| columns.scattering())
        .unwrap()
        .equation();
    let kappa =
        equation.isothermal_compressibility_per_pa(KilogramsPerCubicMetre::new(held_rho), lies_at);
    let excess = earth.surface_pressure().value() - sat("H2O", lies_at.value());
    assert!(
        (relative / (kappa * excess) - 1.0).abs() < 1e-2,
        "{relative:e} against κ_T Δp {:e}",
        kappa * excess
    );
    assert!(moved.area_fraction() < held.area_fraction());
    eprintln!(
        "Earth's sea at {:.2} K under {:.0} Pa: ρ {held_rho:.6} → {rho:.6} kg m⁻³ \
         ({relative:.3e}), area {:.9} → {:.9}",
        lies_at.value(),
        earth.surface_pressure().value(),
        held.area_fraction(),
        moved.area_fraction()
    );
}

/// P14.T49.n's compression of a non-water liquid, held until 24: a carbon dioxide sea at 250 K
/// under 30 bar of nitrogen takes its layer and its share of the basins' fill at the same
/// compressed density, about 1.3% denser than the saturated liquid's, to the bit; and that carbon
/// dioxide's own-ocean weight at its mean, under the pressure its minor branch gives, is its layer
/// over the compressed density weighed by the basins, to the bit, and so lighter. Under 23's rules
/// each is the saturated liquid's.
#[test]
fn under_compression_a_carbon_dioxide_sea_and_its_ocean_weight_read_one_density() {
    use crate::substance::{CompressedLiquidsRules, with_compressed_liquids};
    let co2 = id("CO2");
    let climate = span(240.0, 250.0, 260.0);
    let inv = inventory(&[("CO2", 1e7), ("N2", 3e6)]);
    let cold = linear_cold_area(240.0, 260.0);
    let carbon_sea = || {
        let world = partition(&climate, &on_earth(EARTH_GRAVITY), &inv);
        let c = world.condensate(co2).unwrap();
        assert_eq!(c.phase(), CondensatePhase::Liquid, "{world:?}");
        let published = world.publish(&dry_cover(&cold));
        let entry = *published.entry(co2, CondensatePhase::Liquid).unwrap();
        let law = co2
            .substance()
            .compressed_liquid_density(c.lies_at(), world.surface_pressure())
            .unwrap()
            .value();
        let diagram = co2.substance().phase_diagram().unwrap();
        let rho = liquid_density_kg_per_m3(c, diagram, world.surface_pressure()).unwrap();
        (entry, rho, law)
    };
    let (held, held_rho, _) = carbon_sea();
    let (moved, rho, law) = with_compressed_liquids(CompressedLiquidsRules::Compressed, carbon_sea);
    assert_same_bits(rho, law);
    for (entry, rho) in [(held, held_rho), (moved, rho)] {
        let mass = entry.reservoir().value();
        let layer = mass / rho;
        assert_same_bits(entry.reservoir_gel().unwrap().value(), layer);
        // The only liquid but water's: the whole fill is its share.
        assert_same_bits(
            entry.area_fraction(),
            earth_basins().area_filled(Metres::new(layer)),
        );
    }
    let relative = rho / held_rho - 1.0;
    assert!((0.005..0.05).contains(&relative), "{relative:e}");
    assert!(moved.area_fraction() < held.area_fraction());
    eprintln!(
        "a CO2 sea at 250 K under 30 bar of N2: ρ {held_rho:.4} → {rho:.4} kg m⁻³ \
         ({relative:.3e}), area {:.6} → {:.6}",
        held.area_fraction(),
        moved.area_fraction()
    );
    // Its own-ocean weight, by hand: at the mean, under its minor branch's pressure.
    let k = inv.species.iter().position(|s| s.substance == co2).unwrap();
    let species = &inv.species[k];
    let diagram = co2.substance().phase_diagram().unwrap();
    let ground = on_earth(EARTH_GRAVITY);
    let minor = diagram.saturation_pressure(climate.mean());
    let others = Pascals::new(3e6);
    let weight = || {
        own_ocean_weight(
            (species, diagram),
            &climate,
            &ground,
            others,
            minor,
            CriticalCap::Uncapped,
        )
    };
    let held = weight();
    let moved = with_compressed_liquids(CompressedLiquidsRules::Compressed, weight);
    let rho = co2
        .substance()
        .compressed_liquid_density(climate.mean(), others + minor)
        .unwrap();
    let depth = (species.weight - minor).value() / EARTH_GRAVITY / rho.value();
    assert_same_bits(moved, ground.basins.ocean_weight(Metres::new(depth)));
    assert!(
        0.0 < moved && moved < held && held < 1.0,
        "{moved} against {held}"
    );
    eprintln!("the CO2 sea's own-ocean weight: {held:.9} → {moved:.9}");
}

/// P14.T49.n's record test over a golden system: `close_binary`'s `.0800`, a steam world whose
/// water sea lies under 213 bar at about 583 K on the record at 23 (decision-p14-t54a-water-
/// consistency's figures, 225 bar and 596 K, a sea of 0.419 against 0.406 compressed, are an
/// earlier record's), takes a compressed sea under 24's rules: its layer's density about 4%
/// above the saturated liquid's, its area smaller, and the palette's liquid entry, read inside the
/// switch, the registry's law at its record's temperature and the surface pressure, to the bit.
#[test]
fn close_binary_s_sea_shrinks_under_its_compressed_water() {
    use hyperion_surface::field::PaletteRole;

    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;
    use crate::id::SystemId;
    use crate::planetary::context::SystemContext;
    use crate::planetary::surface::{CoarseInputs, RECORD_TIME};
    use crate::planetary::system::generate;
    use crate::substance::{CompressedLiquidsRules, with_compressed_liquids};
    // tests/planetary_golden.rs's `SYSTEMS_SEED` and close_binary's pinned ID.
    let galaxy = Galaxy::from_params(
        Seed::new(0x5eed_0000_0014_0032),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let system = SystemId::from_raw(0x4200_2cb2_0000_0009).expect("a pinned ID is well formed");
    let ctx = SystemContext::for_system(&galaxy, system).expect("a pinned ID names a system");
    let water = id("H2O");
    let read = || {
        let snapshot = generate(galaxy.seed(), &ctx).snapshot_at(&ctx, RECORD_TIME);
        let record = snapshot
            .bodies()
            .iter()
            .find(|record| record.index().get() == 0x0800)
            .expect("close_binary holds its .0800");
        let surface = record.surface().ok().expect(".0800 has a surface");
        let sea = *surface
            .condensates()
            .entry(water, CondensatePhase::Liquid)
            .expect(".0800 holds a water sea");
        let conditions = surface.conditions();
        let pressure = conditions.surface_pressure();
        let inputs = CoarseInputs::for_body(record, None, &ctx).expect(".0800 reads its inputs");
        let entry = inputs
            .palette()
            .entries()
            .iter()
            .find(|e| e.role == PaletteRole::Liquid && e.substance.as_str() == "H2O")
            .expect(".0800's palette holds its sea");
        let t = entry
            .liquid
            .as_ref()
            .expect("a sea carries its record")
            .temperature;
        let palette = (entry.grain_density_kg_m3, t);
        (
            sea,
            pressure,
            conditions.mean_surface_temperature(),
            palette,
        )
    };
    let (held, pressure, mean, held_palette) = read();
    let (moved, moved_pressure, moved_mean, palette) =
        with_compressed_liquids(CompressedLiquidsRules::Compressed, read);
    let density = |entry: &CondensateEntry| {
        entry.reservoir().value() / entry.reservoir_gel().unwrap().value()
    };
    let (held_rho, rho) = (density(&held), density(&moved));
    let relative = rho / held_rho - 1.0;
    eprintln!(
        "close_binary .0800: {:.4} → {:.4} bar, T_s {:.3} → {:.3} K; the sea's layer density \
         {held_rho:.3} → {rho:.3} kg m⁻³ ({relative:+.4}), area {:.6} → {:.6}; the palette's \
         {held_palette:?} → {palette:?}",
        pressure.value() / 1e5,
        moved_pressure.value() / 1e5,
        mean.value(),
        moved_mean.value(),
        held.area_fraction(),
        moved.area_fraction(),
    );
    assert!((0.02..0.07).contains(&relative), "{relative}");
    assert!(moved.area_fraction() < held.area_fraction());
    let ((held_grain, t_held), (grain, t)) = (held_palette, palette);
    let saturated = water
        .substance()
        .grain_density(CondensedPhase::Liquid, t_held)
        .unwrap();
    assert_same_bits(held_grain, saturated.value());
    let law = water
        .substance()
        .compressed_liquid_density(t, moved_pressure)
        .unwrap();
    assert_same_bits(grain, law.value());
}

/// P14.T53.c's ice-held cold trap ([`ColumnInventory::with_ice_temperature`]): a species the
/// inventory does not name, and a temperature that is negative or not finite, leave it as it was;
/// a −0 is written +0; the species' controlling temperature runs from the mean at no pull to its
/// ice at a full one; and an ice at the climate's own coldest zone gives the plain inventory's
/// partition to the bit, so that the two cold traps cannot drift apart.
#[test]
fn an_ice_held_cold_trap_replaces_the_coldest_zone_alone() {
    let (n2, co2) = (id("N2"), id("CO2"));
    let climate =
        ClimateSpan::new(Kelvin::new(30.0), Kelvin::new(34.0), Kelvin::new(40.0)).unwrap();
    let ground = Ground::without_basins(MetresPerSecondSquared::new(0.78));
    let inventory = ColumnInventory::new([(n2, Pascals::new(5e4))]).unwrap();
    for (substance, t) in [(co2, 37.7), (n2, -1.0), (n2, f64::NAN), (n2, f64::INFINITY)] {
        assert_eq!(
            inventory
                .clone()
                .with_ice_temperature(substance, Kelvin::new(t)),
            inventory,
            "{t}"
        );
    }
    let zero = inventory
        .clone()
        .with_ice_temperature(n2, Kelvin::new(-0.0));
    assert_same_bits(zero.species[0].ice.unwrap().value(), 0.0);
    let held = inventory
        .clone()
        .with_ice_temperature(n2, Kelvin::new(37.7));
    let species = held.species[0];
    assert_same_bits(species.controlling(&climate, 0.0).value(), 34.0);
    assert_same_bits(species.controlling(&climate, 1.0).value(), 37.7);
    assert_same_bits(
        species.controlling(&climate, 0.5).value(),
        34.0 - 0.5 * (34.0 - 37.7),
    );
    let at_coldest = inventory
        .clone()
        .with_ice_temperature(n2, climate.coldest());
    assert_eq!(
        partition(&climate, &ground, &at_coldest).species(),
        partition(&climate, &ground, &inventory).species()
    );
    assert_eq!(
        partition(&climate, &ground, &at_coldest).condensates(),
        partition(&climate, &ground, &inventory).condensates()
    );
}

/// The phase diagram of the row `key`.
fn phase_diagram(key: &str) -> &'static PhaseDiagram {
    id(key).substance().phase_diagram().unwrap()
}

/// The methane of a column `kilobars` deep alone over an isothermal climate at `t`, K, at Earth's
/// gravity (P14.T53.b part 3's hand cases).
fn methane_column(t: f64, kilobars: f64) -> (ClimateSpan, Ground, ColumnInventory) {
    (
        ClimateSpan::isothermal(Kelvin::new(t)),
        gravity(EARTH_GRAVITY),
        inventory(&[("CH4", kilobars * 1e8)]),
    )
}

/// P14.T53.b part 3 (decision-p14-t51a-partition §5): with every landed 25 hold in force, a methane
/// column above its 190.564 K critical point keeps its own critical isochore's pressure airborne,
/// about 60 bar at 200 K, and the rest lies at the surface as a liquid-like supercritical
/// reservoir; under part 3's hold all of it is airborne, as at version 24.
#[test]
fn under_every_25_hold_a_methane_column_above_its_critical_point_is_capped_at_its_isochore() {
    use crate::planetary::derive::atmosphere::{SteamEnvelopeRules, with_steam_envelope};
    use crate::planetary::testing::with_every_25_hold;
    let ch4 = phase_diagram("CH4");
    let (climate, ground, column) = methane_column(200.0, 1.0);
    let isochore = ch4.condensation_cap(Kelvin::new(200.0)).value();
    assert!((59e5..61e5).contains(&isochore), "{isochore}");
    with_every_25_hold(|| {
        let world = partition(&climate, &ground, &column);
        let methane = of(&world, "CH4");
        assert_same_bits(methane.dominance(), 1.0);
        assert!(relative(methane.airborne().value(), isochore) < 1e-12);
        assert!(relative(methane.partial_pressure().value(), isochore) < 1e-12);
        assert!(relative(methane.condensed().value(), 1e8 - isochore) < 1e-12);
        let fluid = condensate(&world, "CH4");
        assert_eq!(fluid.phase(), CondensatePhase::Supercritical);
        assert_same_bits(
            fluid.reservoir().value(),
            methane.condensed().value() / EARTH_GRAVITY,
        );
        // A supercritical reservoir is never seasonal (decision-p14-t53b-part3 §9).
        assert!(!fluid.is_seasonal(), "{fluid:?}");
        // Its column with the most fluid is the partition's own reservoir, below the melting
        // curve whole.
        let mut weights = AirborneWeights::new();
        weights.solve(&climate, &ground, &column);
        let reservoir = weights.supercritical_column(&climate, &column).unwrap();
        assert_eq!(reservoir.substance(), id("CH4"));
        assert_same_bits(reservoir.reservoir().value(), methane.condensed().value());
        assert_same_bits(reservoir.fluid().value(), methane.condensed().value());
        assert_same_bits(
            reservoir.surface_pressure().value(),
            world.surface_pressure().value(),
        );
        assert_same_bits(reservoir.lies_at().value(), 200.0);
        let melting = reservoir.melting_pressure().unwrap().value();
        // Setzmann and Wagner 1991's melting equation: 5.94 kbar at 200 K.
        assert!((5.8e8..6.1e8).contains(&melting), "{melting}");
        // Under part 3's hold the column is all airborne, as at version 24.
        let held = with_steam_envelope(SteamEnvelopeRules::Held, || {
            partition(&climate, &ground, &column)
        });
        assert_same_bits(of(&held, "CH4").condensed().value(), 0.0);
        assert_same_bits(condensate(&held, "CH4").reservoir().value(), 0.0);
        let mut held_weights = AirborneWeights::new();
        with_steam_envelope(SteamEnvelopeRules::Held, || {
            held_weights.solve(&climate, &ground, &column);
        });
        assert!(
            held_weights
                .supercritical_column(&climate, &column)
                .is_none()
        );
    });
}

/// P14.T53.b part 3: the reservoir's fluid ends at its melting pressure, the solid beneath it; and
/// where the surface pressure itself reaches the melting curve, the reservoir is solid
/// (decision-absurd-states-24 R2.2: methane at 41.8–68.3 kbar is solid above about 5.4–5.7 kbar
/// at 192–197 K, Setzmann and Wagner 1991's melting equation, not supercritical).
#[test]
fn under_every_25_hold_the_melting_curve_comes_first_above_the_critical_point() {
    use crate::planetary::testing::with_every_25_hold;
    with_every_25_hold(|| {
        let (climate, ground, deep) = methane_column(200.0, 10.0);
        let mut weights = AirborneWeights::new();
        weights.solve(&climate, &ground, &deep);
        let reservoir = weights.supercritical_column(&climate, &deep).unwrap();
        let melting = reservoir.melting_pressure().unwrap();
        assert_same_bits(
            reservoir.fluid().value(),
            (melting - reservoir.surface_pressure()).value(),
        );
        assert!(reservoir.fluid() < reservoir.reservoir(), "{reservoir:?}");
        let world = partition(&climate, &ground, &deep);
        assert_eq!(
            condensate(&world, "CH4").phase(),
            CondensatePhase::Supercritical
        );
        // Under ten kilobars of helium the surface pressure lies above methane's melting pressure
        // at 200 K, so its reservoir lies there as a solid.
        let buried = inventory(&[("CH4", 1e8), ("He", 1e9)]);
        let world = partition(&climate, &ground, &buried);
        assert!(world.surface_pressure() >= melting, "{world:?}");
        let solid = condensate(&world, "CH4");
        assert!(solid.reservoir().value() > 0.0, "{solid:?}");
        assert_eq!(solid.phase(), CondensatePhase::Solid);
        let mut weights = AirborneWeights::new();
        weights.solve(&climate, &ground, &buried);
        let column = weights.supercritical_column(&climate, &buried).unwrap();
        assert_same_bits(column.fluid().value(), 0.0);
    });
}

/// P14.T53.b part 3: the cap and the airborne methane are continuous across its critical point
/// (decision-p14-t51a-partition §5, where the curve meets the isochore in value and slope).
#[test]
fn under_every_25_hold_the_cap_is_continuous_across_the_critical_point() {
    use crate::planetary::testing::with_every_25_hold;
    with_every_25_hold(|| {
        let t_c = phase_diagram("CH4").critical_temperature().value();
        let airborne = |t: f64| {
            let (climate, ground, column) = methane_column(t, 1.0);
            of(&partition(&climate, &ground, &column), "CH4")
                .airborne()
                .value()
        };
        let (below, at, above) = (
            airborne(t_c * (1.0 - 1e-12)),
            airborne(t_c),
            airborne(t_c * (1.0 + 1e-12)),
        );
        assert!(relative(below, at) < 1e-9, "{below} below against {at}");
        assert!(relative(above, at) < 1e-9, "{above} above against {at}");
        assert!(below < at && at < above);
    });
}

/// P14.T53.b part 3: a supercritical reservoir is published with its reservoir, its layer over its
/// liquid's density at its critical temperature, and its share of the basins as a liquid's; a
/// supercritical fluid that is the air at the surface (the measured Venus's carbon dioxide) keeps
/// none.
#[test]
fn under_every_25_hold_a_supercritical_reservoir_is_published_as_a_layer() {
    use crate::planetary::testing::with_every_25_hold;
    with_every_25_hold(|| {
        let (climate, ground, column) = methane_column(200.0, 1.0);
        let world = partition(&climate, &ground, &column);
        let cold_area = |_: Kelvin| 0.0;
        let water = WaterCover {
            surface_water: KilogramsPerSquareMetre::ZERO,
            potential_ocean: 0.0,
            ice_fraction: 0.0,
            ice_density: KilogramsPerCubicMetre::new(917.0),
            crust_water: CrustWater::Unlisted,
        };
        let cover = Cover {
            cold_area: &cold_area,
            state: SurfaceState::Snowball,
            water,
        };
        let published = world.publish(&cover);
        let entry = published
            .entry(id("CH4"), CondensatePhase::Supercritical)
            .unwrap();
        let reservoir = condensate(&world, "CH4").reservoir().value();
        assert_same_bits(entry.reservoir().value(), reservoir);
        let rho = id("CH4")
            .substance()
            .liquid_density_at_pressure(
                phase_diagram("CH4").critical_temperature(),
                world.surface_pressure(),
            )
            .unwrap()
            .value();
        assert!((150.0..250.0).contains(&rho), "{rho} kg m⁻³");
        assert_same_bits(entry.reservoir_gel().unwrap().value(), reservoir / rho);
        assert!(!entry.is_seasonal());
        assert_eq!(
            published.surface_liquids(),
            Vec::<(SubstanceId, f64)>::new()
        );
        let venus = venus("Venus at 92 bar", 9.2e6).partition();
        let fluid = venus.publish(&cover);
        let entry = fluid
            .entry(id("CO2"), CondensatePhase::Supercritical)
            .unwrap();
        assert_same_bits(entry.reservoir().value(), 0.0);
        assert_same_bits(entry.area_fraction(), 0.0);
    });
}

/// P14.T53.b part 3 (the determinism review): with every 25 hold in force, the partition above
/// the critical point is the same twice, and a shared scratch, its supercritical column read
/// with its weights, is the same in any order of cases, a version 24 solve among them.
#[test]
fn under_every_25_hold_the_cap_and_its_column_are_the_same_twice_and_in_any_order() {
    use crate::planetary::derive::atmosphere::{SteamEnvelopeRules, with_steam_envelope};
    use crate::planetary::testing::with_every_25_hold;
    with_every_25_hold(|| {
        let (climate, ground, _) = methane_column(200.0, 1.0);
        let below = ClimateSpan::isothermal(Kelvin::new(180.0));
        let cases: Vec<(ClimateSpan, ColumnInventory)> = vec![
            (climate, methane_column(200.0, 1.0).2),
            (climate, methane_column(200.0, 10.0).2),
            (climate, inventory(&[("CH4", 1e8), ("He", 1e9)])),
            (climate, inventory(&[("N2", 1e8), ("CH4", 1e6)])),
            (below, methane_column(180.0, 1.0).2),
        ];
        for (climate, inv) in &cases {
            assert_eq!(
                format!("{:?}", partition(climate, &ground, inv)),
                format!("{:?}", partition(climate, &ground, inv))
            );
        }
        let shared = RefCell::new(AirborneWeights::new());
        let solve_once = |held: bool, (climate, inv): &(ClimateSpan, ColumnInventory)| {
            let mut weights = shared.borrow_mut();
            if held {
                with_steam_envelope(SteamEnvelopeRules::Held, || {
                    weights.solve(climate, &ground, inv);
                });
            } else {
                weights.solve(climate, &ground, inv);
            }
            format!(
                "{:?} {:?}",
                weights.weights(),
                weights.supercritical_column(climate, inv)
            )
        };
        assert_order_independent(&cases, |case| solve_once(false, case));
        // A version 24 solve between two leaves no trace in the next.
        let first = solve_once(false, &cases[0]);
        let between = solve_once(true, &cases[1]);
        assert!(between.ends_with("None"), "{between}");
        assert_eq!(solve_once(false, &cases[0]), first);
    });
}

/// P14.T53.b part 3 (the Rust review): a supercritical reservoir shares the basins by volume with
/// the other liquids, as a liquid does: methane above its critical point beside a sea of ammonia
/// at 250 K, on Earth's basins, each entry's area its layer's share of the basins' fill of their
/// summed layers.
#[test]
fn under_every_25_hold_a_supercritical_reservoir_shares_the_basins_with_the_liquids() {
    use crate::planetary::testing::with_every_25_hold;
    with_every_25_hold(|| {
        let climate = ClimateSpan::isothermal(Kelvin::new(250.0));
        let ground = on_earth(EARTH_GRAVITY);
        // 160 bar of methane, some 27 bar above its isochore's 133 bar at 250 K: a layer of about
        // 1.7 km at its critical density, which fills Earth's basins in part.
        let world = partition(
            &climate,
            &ground,
            &inventory(&[("CH4", 1.6e7), ("NH3", 1e6)]),
        );
        let cold_area = |_: Kelvin| 0.0;
        let cover = Cover {
            cold_area: &cold_area,
            state: SurfaceState::Snowball,
            water: WaterCover {
                surface_water: KilogramsPerSquareMetre::ZERO,
                potential_ocean: 0.0,
                ice_fraction: 0.0,
                ice_density: KilogramsPerCubicMetre::new(917.0),
                crust_water: CrustWater::Unlisted,
            },
        };
        let published = world.publish(&cover);
        let fluid = published
            .entry(id("CH4"), CondensatePhase::Supercritical)
            .unwrap();
        let sea = published.entry(id("NH3"), CondensatePhase::Liquid).unwrap();
        assert!(fluid.reservoir() > sea.reservoir(), "{published:?}");
        let (d_fluid, d_sea) = (
            fluid.reservoir_gel().unwrap().value(),
            sea.reservoir_gel().unwrap().value(),
        );
        // The liquids are summed in the condensates' order, the larger reservoir first.
        let volume = d_fluid + d_sea;
        let filled = earth_basins().area_filled(Metres::new(volume));
        assert!(filled > 0.0 && filled < 1.0, "{filled}");
        assert_same_bits(fluid.area_fraction(), filled * d_fluid / volume);
        assert_same_bits(sea.area_fraction(), filled * d_sea / volume);
        assert_eq!(
            published.surface_liquids(),
            vec![(id("NH3"), sea.area_fraction())]
        );
    });
}
