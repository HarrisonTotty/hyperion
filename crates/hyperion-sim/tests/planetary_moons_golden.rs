//! Golden values of plan 14's moons (P14.T17–T19): the regular moons, giant-impact moons and
//! captures of chosen synthetic parents.
//!
//! They pin the arithmetic and the words drawn: a reordered sum, a changed constant or a moved
//! draw number changes a line here, which is a generator-version change. CI checks the same file
//! on 64-bit Arm and on wasm32.

use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{BodyId, Layer, SystemId};
use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
use hyperion_sim::planetary::derive::PlanetClass;
use hyperion_sim::planetary::moons::{
    BeltAdjacency, MoonParent, MoonParentParts, NearestBelt, ParentKind, captures,
    giant_impact_moon, regular_moons,
};
use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
use hyperion_sim::units::{
    EarthMasses, GravitationalParameter, Kilograms, Metres, Radians, SolarMasses, Years,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

fn planet(index: u32, slot: u8) -> BodyId {
    let system = SystemId::from_parts(
        Layer::A,
        GenCell::new(CellSize::Ly8, [4, -2, 1]).unwrap(),
        index,
    )
    .unwrap();
    BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary)
        .unwrap()
        .body_id(system)
}

/// A parent of `mass` M⊕ and `radius_km` of class `class`, at `a_au` and `e` about a Sun.
fn parent(
    id: BodyId,
    kind: ParentKind,
    mass: f64,
    radius_km: f64,
    class: PlanetClass,
    a_au: f64,
    e: f64,
) -> MoonParent {
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::new(a_au * METRES_PER_AU),
        GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
        Eccentricity::new(e).unwrap(),
        Orientation::new(Radians::new(0.02), Radians::new(1.1), Radians::new(2.3)).unwrap(),
        Radians::new(0.7),
    )
    .unwrap();
    MoonParent::new(MoonParentParts {
        id,
        kind,
        mass: EarthMasses::new(mass),
        radius: Metres::new(radius_km * 1e3),
        class,
        orbit,
        host_mass: Kilograms::new(SOLAR_MASS_KG),
        maximum_moon_mass: EarthMasses::new(1.0),
    })
    .unwrap()
}

fn orbit_lines(w: &mut GoldenWriter, label: &str, orbit: &KeplerElements) {
    w.f64(&format!("{label} a (m)"), orbit.semi_major_axis().value());
    w.f64(&format!("{label} e"), orbit.eccentricity().value());
    w.f64(&format!("{label} i (rad)"), orbit.inclination().value());
    w.f64(
        &format!("{label} node (rad)"),
        orbit.ascending_node().value(),
    );
    w.f64(
        &format!("{label} periapsis (rad)"),
        orbit.argument_of_periapsis().value(),
    );
    w.f64(
        &format!("{label} mean anomaly (rad)"),
        orbit.mean_anomaly_at_epoch().value(),
    );
}

/// The giants the golden pins, each with its name.
fn giants() -> [(&'static str, MoonParent); 4] {
    [
        (
            "jupiter",
            parent(
                planet(1, 5),
                ParentKind::Planet,
                317.83,
                69_911.0,
                PlanetClass::GasGiant,
                5.2029,
                0.0484,
            ),
        ),
        (
            "saturn",
            parent(
                planet(2, 6),
                ParentKind::Planet,
                95.16,
                58_232.0,
                PlanetClass::GasGiant,
                9.5367,
                0.0539,
            ),
        ),
        (
            "neptune",
            parent(
                planet(3, 8),
                ParentKind::Planet,
                17.15,
                24_622.0,
                PlanetClass::IceGiant,
                30.07,
                0.0086,
            ),
        ),
        (
            "warm giant",
            parent(
                planet(4, 2),
                ParentKind::Planet,
                600.0,
                75_000.0,
                PlanetClass::GasGiant,
                0.4,
                0.2,
            ),
        ),
    ]
}

/// Every giant's regular moons and captures under two seeds.
fn pin_giants(w: &mut GoldenWriter, belt: Option<NearestBelt>) {
    let giants = giants();
    for seed in [Seed::new(1), Seed::new(0x0123_4567_89ab_cdef)] {
        for (name, giant) in &giants {
            w.line("");
            w.line(&format!("regular moons of {name}, seed {seed}"));
            let system = regular_moons(seed, giant);
            w.f64("drawn mass (M⊕)", system.drawn_mass().value());
            w.line(&format!("moonlets = {}", system.moonlets()));
            for moon in system.moons() {
                let label = format!("moon {}", moon.ordinal());
                w.f64(&format!("{label} mass (M⊕)"), moon.mass().value());
                w.line(&format!("{label} resonance {:?}", moon.resonance()));
                orbit_lines(w, &label, moon.orbit());
            }
            w.line(&format!("captures of {name}, seed {seed}"));
            let caught = captures(seed, giant, belt);
            if let Some(population) = caught.population() {
                w.line(&format!("population = {}", population.count()));
                w.f64(
                    "largest diameter (m)",
                    population.largest_diameter().value(),
                );
                w.f64("break diameter (m)", population.break_diameter().value());
            }
            for moon in caught.moons() {
                let label = format!(
                    "capture {} {:?} {:?}",
                    moon.ordinal(),
                    moon.kind(),
                    moon.sense()
                );
                w.f64(&format!("{label} radius (m)"), moon.radius().value());
                w.f64(&format!("{label} mass (M⊕)"), moon.mass().value());
                orbit_lines(w, &label, moon.orbit());
            }
        }
    }
}

/// The first six giant-impact moons of Earths and Plutos.
fn pin_impacts(w: &mut GoldenWriter) {
    let mut found = 0;
    for i in 0..400 {
        let (kind, mass, radius, class, a) = if i % 2 == 0 {
            (ParentKind::Planet, 1.0, 6_371.0, PlanetClass::Rocky, 1.0)
        } else {
            (
                ParentKind::DwarfPlanet,
                0.0022,
                1_188.0,
                PlanetClass::Icy,
                39.5,
            )
        };
        let body = parent(planet(10 + i, 3), kind, mass, radius, class, a, 0.02);
        let Some(moon) = giant_impact_moon(Seed::new(1), &body) else {
            continue;
        };
        w.line("");
        w.line(&format!("giant-impact moon of {kind:?} {i}"));
        w.f64("mass (M⊕)", moon.mass().value());
        w.f64("density (kg/m³)", moon.density().value());
        orbit_lines(w, "formed", moon.formed());
        w.f64(
            "a at 4.5 Gyr (m)",
            moon.semi_major_axis_at(Years::new(4.5e9)).value(),
        );
        w.f64("lost at (yr)", moon.lost_at().value());
        found += 1;
        if found == 6 {
            break;
        }
    }
}

/// The small captures of Mars-like planets beside a belt.
fn pin_small_captures(w: &mut GoldenWriter, belt: Option<NearestBelt>) {
    for i in 0..200 {
        let body = parent(
            planet(500 + i, 4),
            ParentKind::Planet,
            0.107,
            3_389.5,
            PlanetClass::Rocky,
            1.524,
            0.093,
        );
        let caught = captures(Seed::new(1), &body, belt);
        for moon in caught.moons() {
            let label = format!("small capture {i}.{}", moon.ordinal());
            w.f64(&format!("{label} radius (m)"), moon.radius().value());
            orbit_lines(w, &label, moon.orbit());
        }
    }
}

#[test]
fn moons_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let belt = NearestBelt::new(EarthMasses::new(4e-4), BeltAdjacency::Neighbouring);
    pin_giants(&mut w, belt);
    pin_impacts(&mut w);
    pin_small_captures(&mut w, belt);
    golden!("planetary/moons", w.as_str());
}
