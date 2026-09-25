//! Golden values of plan 14's rings, belts and cometary halo (P14.T20, T21): the rings of chosen
//! giants, the belts and members of a Solar System input and of a compact system, and a halo
//! before and after its host's mass loss.
//!
//! They pin the arithmetic and the words drawn: a reordered sum, a changed constant or a moved
//! draw number changes a line here, which is a generator-version change. CI checks the same file
//! on 64-bit Arm and on wasm32. Nothing generated reads these stages until P14.T22.a and T30.a
//! call them.

use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::belts::{BeltHost, FIRST_BELT_SLOT, host_belts};
use hyperion_sim::planetary::derive::PlanetClass;
use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, DiscProfile, Truncation};
use hyperion_sim::planetary::halo::{HaloHost, Scatterer, halo, surviving_share};
use hyperion_sim::planetary::placement::classes::orbits::SystemPlane;
use hyperion_sim::planetary::placement::{Neighbour, OrbitHost};
use hyperion_sim::planetary::rings::{RingMoon, RingParent, generate_rings};
use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::units::{
    AstronomicalUnits, EarthMasses, Kelvin, Kilograms, Megayears, Metres, Radians,
    SolarLuminosities, SolarMasses, SolarMassesPerYear, Years,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

const SEED: Seed = Seed::new(0x5eed_0000_0014_0021);

fn system() -> SystemId {
    SystemId::from_raw(0x0200_0800_2000_0000).unwrap()
}

fn au(x: f64) -> Metres {
    Metres::from(AstronomicalUnits::new(x))
}

/// The median disc of a solar-metallicity host of `mass` in its zero-age state.
fn median_disc(mass: f64) -> DiscProfile {
    let mass = SolarMasses::new(mass);
    let composition = Composition::SOLAR;
    let coeffs = ZCoeffs::new(composition.z_fit());
    let host = DiscHost::new(
        mass,
        composition.fe_h(),
        zams::luminosity(mass, &coeffs),
        zams::radius(mass, &coeffs),
    )
    .unwrap();
    *disc::derive(
        &host,
        Megayears::new(3.0),
        &DiscDraws::MEDIAN,
        Truncation::NONE,
    )
    .profile()
    .unwrap()
}

fn belts(w: &mut GoldenWriter, name: &str, disc: &DiscProfile, planets: &[Neighbour]) {
    let plane = SystemPlane::new(Radians::new(1.1), Radians::new(0.4)).unwrap();
    let host = BeltHost::new(OrbitHost::Star(0), disc, planets, plane);
    let belts = host_belts(SEED, system(), &host, FIRST_BELT_SLOT);
    w.line("");
    w.line(&format!("belts of {name}, next slot {}", belts.next_slot()));
    for belt in belts.belts() {
        w.line(&format!(
            "belt {:#06x} {:?} {:?} {:?}",
            belt.index().get(),
            belt.kind(),
            belt.site(),
            belt.composition()
        ));
        for part in belt.components() {
            w.line(&format!("  part {:?}", part.part()));
            w.f64("  inner edge m", part.inner_edge().value());
            w.f64("  outer edge m", part.outer_edge().value());
            w.f64("  initial mass", part.initial_mass().value());
        }
        for gap in belt.gaps() {
            w.f64(
                &format!("  gap {:?} m", gap.resonance()),
                gap.radius().value(),
            );
        }
        w.f64("  depletion", belt.depletion());
        w.f64("  size slope", belt.size_slope());
        w.f64("  largest diameter m", belt.largest_diameter().value());
        for age in [1e7, 1e9, 4.6e9] {
            w.f64(
                &format!("  mass at {age:e} yr"),
                belt.mass_at(Years::new(age)).value(),
            );
            w.f64(
                &format!("  f at {age:e} yr"),
                belt.fractional_luminosity(Years::new(age), SolarLuminosities::new(1.0)),
            );
        }
        for member in belt.members() {
            let orbit = member.orbit();
            w.line(&format!(
                "  member {:#06x} {:?}",
                member.index().get(),
                member.part()
            ));
            w.f64("    diameter m", member.diameter().value());
            w.f64("    mass", member.mass().value());
            w.f64("    a", orbit.semi_major_axis().value());
            w.f64("    e", orbit.eccentricity().value());
            w.f64("    i", orbit.orientation().inclination().value());
            w.f64("    node", orbit.orientation().ascending_node().value());
            w.f64(
                "    periapsis",
                orbit.orientation().argument_of_periapsis().value(),
            );
            w.f64("    mean anomaly", orbit.mean_anomaly_at_epoch().value());
            w.f64("    radius rank", member.radius_rank().value());
        }
    }
}

fn rings(w: &mut GoldenWriter) {
    let moons = [
        RingMoon::new(Metres::new(185_539e3), Kilograms::new(3.75e19)),
        RingMoon::new(Metres::new(238_042e3), Kilograms::new(1.08e20)),
    ];
    for (slot, class, mass, radius, t) in [
        (5, PlanetClass::GasGiant, 1.898e27, 69_911e3, 110.0),
        (6, PlanetClass::GasGiant, 5.683e26, 58_232e3, 95.0),
        (7, PlanetClass::IceGiant, 8.681e25, 25_362e3, 59.0),
        (1, PlanetClass::GasGiant, 1.898e27, 90_000e3, 1_400.0),
    ] {
        for n in 0..24_u64 {
            let id = SystemId::from_raw(0x0200_0800_2000_0000 + (n << 8)).unwrap();
            let index = BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary).unwrap();
            let parent = RingParent::new(
                index,
                class,
                Kilograms::new(mass),
                Metres::new(radius),
                Kelvin::new(t),
            )
            .unwrap();
            for ring in generate_rings(SEED, id, &parent, &moons) {
                w.line(&format!(
                    "ring {id} {:#06x} {:?} {:?}",
                    ring.index().get(),
                    ring.kind(),
                    ring.material()
                ));
                w.f64("  inner edge m", ring.inner_edge().value());
                w.f64("  outer edge m", ring.outer_edge().value());
                w.f64("  mass kg", ring.mass().value());
                w.f64("  optical depth", ring.optical_depth());
                for gap in ring.gaps() {
                    w.f64(
                        &format!("  gap {:?} of moon {}", gap.resonance(), gap.moon()),
                        gap.radius().value(),
                    );
                }
            }
        }
    }
}

fn halos(w: &mut GoldenWriter) {
    let host = HaloHost::new(
        SolarMasses::new(1.0),
        EarthMasses::new(32.2),
        au(1.39e5),
        None,
    );
    for n in 0..8_u64 {
        let id = SystemId::from_raw(0x0200_0800_2000_0000 + (n << 8)).unwrap();
        let halo = halo(SEED, id, &host, Scatterer::Present).unwrap();
        w.line(&format!("halo {id} {:#06x}", halo.index().get()));
        w.f64("  inner edge m", halo.inner_edge().value());
        w.f64("  outer edge m", halo.outer_edge().value());
        w.f64("  comets", halo.comets());
        w.f64(
            "  new comets per s",
            halo.new_comet_rate(SolarLuminosities::new(1.0)).value(),
        );
        w.f64(
            "  comets per s",
            halo.comet_rate(SolarLuminosities::new(1.0)).value(),
        );
        for rate in [5e-7, f64::INFINITY] {
            let later = halo
                .at(SolarMasses::new(0.54), SolarMassesPerYear::new(rate))
                .unwrap();
            w.f64(&format!("  comets at 0.54 after {rate:e}"), later.comets());
            w.f64(
                &format!("  outer edge at 0.54 after {rate:e} m"),
                later.outer_edge().value(),
            );
        }
    }
    for ratio in [0.1, 0.3, 0.5, 0.54, 0.7, 0.9] {
        w.f64(&format!("surviving share {ratio}"), surviving_share(ratio));
    }
}

#[test]
fn small_bodies_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let solar = [
        (0.0553, 0.387, 0.206),
        (0.815, 0.723, 0.007),
        (1.0, 1.000, 0.017),
        (0.107, 1.524, 0.093),
        (317.8, 5.203, 0.048),
        (95.16, 9.537, 0.054),
        (14.54, 19.19, 0.047),
        (17.15, 30.07, 0.009),
    ]
    .map(|(m, a, e)| Neighbour::new(EarthMasses::new(m), au(a), e));
    let compact = [(4.0, 0.08, 0.02), (6.0, 0.12, 0.03), (5.0, 0.18, 0.01)]
        .map(|(m, a, e)| Neighbour::new(EarthMasses::new(m), au(a), e));
    let wide = [(1.0, 0.8, 0.01), (1.2, 4.0, 0.02)]
        .map(|(m, a, e)| Neighbour::new(EarthMasses::new(m), au(a), e));
    belts(&mut w, "the Solar System", &median_disc(1.0), &solar);
    belts(&mut w, "a compact chain", &median_disc(0.8), &compact);
    belts(&mut w, "a wide pair", &median_disc(1.0), &wide);
    belts(&mut w, "no planets", &median_disc(0.5), &[]);
    w.line("");
    rings(&mut w);
    w.line("");
    halos(&mut w);
    golden!("planetary/small_bodies", w.as_str());
}
