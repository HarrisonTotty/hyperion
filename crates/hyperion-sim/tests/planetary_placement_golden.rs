//! Golden values of plan 14's placement: the spacing draw (P14.T6.b), Holman and Wiegert's fits,
//! and the stable zones, hosts and zone discs of hand-built hierarchies (P14.T9). The root pair's
//! zone is the barycentre's (ruling 52.4), with the host number it had as a pair.
//!
//! They pin the arithmetic and the words drawn: a reordered sum, a changed coefficient or a moved
//! draw number changes a line here, which is a generator-version change. CI checks the same files
//! on 64-bit Arm and on wasm32.

use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::orbit::Eccentricity;
use hyperion_sim::planetary::architecture::HostMultiplicity;
use hyperion_sim::planetary::placement::{
    OrbitHost, OrbitZone, SpacingDraws, SpacingKind, SpacingOutcome, ZoneDiscInputs, ZoneHierarchy,
    ZoneNode, ZoneStar, draw_pair_spacing, holman_wiegert_p_type, holman_wiegert_s_type,
    stable_zones,
};
use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::UnitUniform;
use hyperion_sim::stellar::multiplicity::SlotKind;
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::units::{
    AstronomicalUnits, Dex, Metres, SolarLuminosities, SolarMasses, SolarRadii,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

fn system(cell: [i32; 3], index: u32) -> SystemId {
    SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, cell).unwrap(), index).unwrap()
}

fn au(x: f64) -> Metres {
    Metres::from(AstronomicalUnits::new(x))
}

fn star(index: u8, mass: f64) -> ZoneNode {
    ZoneNode::component(index, SolarMasses::new(mass), SlotKind::Star)
}

fn pair(inner: ZoneNode, outer: ZoneNode, a_au: f64, e: f64) -> ZoneNode {
    ZoneNode::pair(inner, outer, au(a_au), Eccentricity::new(e).unwrap())
}

#[test]
fn spacing_draws_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let a = system([652, -4_584, 2_047], 7);
    let b = system([-3, 10, 0], 0);
    let cases = [
        (Seed::new(0x0123_4567_89ab_cdef), a, 0),
        (Seed::new(0x0123_4567_89ab_cdef), a, 1),
        (Seed::new(42), b, 0),
        (Seed::new(42), b, 17),
        (Seed::new(42), b, 255),
    ];
    for (seed, id, host) in cases {
        w.line("");
        w.line(&format!("means seed {seed} system {id} host {host}"));
        let draws = SpacingDraws::for_host(seed, id, host);
        for kind in SpacingKind::ALL {
            w.f64(&format!("{kind:?} normal"), draws.normal(kind).value());
            w.f64(&format!("{kind:?} mean"), draws.mean_spacing(kind));
        }
    }
    let seed = Seed::new(42);
    let slots = [
        BodySlot::Planet(1),
        BodySlot::Planet(2),
        BodySlot::Planet(7),
        BodySlot::Planet(191),
        BodySlot::SecondGeneration(0),
    ];
    for (mean, floor) in [
        (17.0, 10.0),
        (13.0, 12.0),
        (30.0, 10.0),
        (9.0, 7.0),
        (0.0, 30.0),
    ] {
        w.line("");
        w.line(&format!("pairs of {b} about {mean} above {floor}"));
        for slot in slots {
            let outer = BodyIndex::new(slot, BodySub::Primary).unwrap();
            let pair = draw_pair_spacing(seed, b, outer, mean, floor);
            let outcome = match pair.outcome() {
                SpacingOutcome::Drawn { redraws } => format!("drawn after {redraws}"),
                SpacingOutcome::Floor => "floor".to_owned(),
            };
            w.f64(&format!("{slot} {outcome}"), pair.spacing());
        }
    }
    golden!("planetary/spacing", w.as_str());
}

fn host_name(host: OrbitHost) -> String {
    match host {
        OrbitHost::Star(n) => format!("star {n}"),
        OrbitHost::Pair(k) => format!("pair {k}"),
        OrbitHost::Barycentre => "barycentre".to_owned(),
        OrbitHost::Body(index) => format!("body {:04x}", index.get()),
    }
}

fn write_zone(w: &mut GoldenWriter, zone: &OrbitZone) {
    let name = host_name(zone.host());
    let members: Vec<String> = zone.members().map(|m| m.to_string()).collect();
    w.line(&format!(
        "{name}: host {} members {} close {}",
        zone.host_number(),
        members.join(","),
        zone.host_multiplicity() == HostMultiplicity::CloseBinary
    ));
    w.f64(&format!("{name} mass"), zone.host_mass().value());
    match zone.inner() {
        Some(r) => w.f64(&format!("{name} inner_m"), r.value()),
        None => w.line(&format!("{name} inner none")),
    }
    match zone.outer() {
        Some(r) => w.f64(&format!("{name} outer_m"), r.value()),
        None => w.line(&format!("{name} outer none")),
    }
}

/// A star at the zero-age main sequence of plan 06's fits, at solar composition.
fn zams_star(mass: f64, rank: f64) -> ZoneStar {
    let m = SolarMasses::new(mass);
    let coeffs = ZCoeffs::new(Composition::SOLAR.z_fit());
    ZoneStar {
        zams_luminosity: zams::luminosity(m, &coeffs),
        zams_radius: zams::radius(m, &coeffs),
        disc_lifetime_rank: UnitUniform::new(rank).unwrap(),
    }
}

/// The hand-built hierarchies the zones golden pins, each with its components' zero-age states.
fn golden_hierarchies() -> Vec<(&'static str, ZoneNode, Vec<ZoneStar>)> {
    let dwarf = ZoneNode::component(1, SolarMasses::new(0.05), SlotKind::BrownDwarf);
    // A brown dwarf's disc will read its cooling fit at 10 Myr (P14.T27); these are stand-ins.
    let dwarf_state = ZoneStar {
        zams_luminosity: SolarLuminosities::new(3e-3),
        zams_radius: SolarRadii::new(0.3),
        disc_lifetime_rank: UnitUniform::new(0.7).unwrap(),
    };
    vec![
        ("single sun", star(0, 1.0), vec![zams_star(1.0, 0.5)]),
        (
            "alpha centauri ab",
            pair(star(0, 1.12), star(1, 0.95), 23.57, 0.516),
            vec![zams_star(1.12, 0.2), zams_star(0.95, 0.4)],
        ),
        (
            "alpha centauri with proxima",
            pair(
                pair(star(0, 1.1), star(1, 0.9), 23.5, 0.52),
                star(2, 0.12),
                8_700.0,
                0.5,
            ),
            vec![
                zams_star(1.1, 0.2),
                zams_star(0.9, 0.4),
                zams_star(0.12, 0.6),
            ],
        ),
        (
            "two close pairs",
            pair(
                pair(star(0, 1.0), star(1, 0.5), 0.3, 0.1),
                pair(star(2, 0.8), star(3, 0.7), 1.0, 0.3),
                200.0,
                0.4,
            ),
            vec![
                zams_star(1.0, 0.1),
                zams_star(0.5, 0.3),
                zams_star(0.8, 0.5),
                zams_star(0.7, 0.9),
            ],
        ),
        (
            "star and brown dwarf",
            pair(star(0, 1.0), dwarf, 10.0, 0.2),
            vec![zams_star(1.0, 0.5), dwarf_state],
        ),
        (
            "tight triple",
            pair(
                pair(star(0, 1.0), star(1, 1.0), 1.0, 0.0),
                star(2, 1.0),
                8.0,
                0.0,
            ),
            vec![
                zams_star(1.0, 0.2),
                zams_star(1.0, 0.5),
                zams_star(1.0, 0.8),
            ],
        ),
        (
            "eccentric pair",
            pair(star(0, 2.0), star(1, 0.1), 5.0, 0.9),
            vec![zams_star(2.0, 0.5), zams_star(0.1, 0.5)],
        ),
        // Masses add up the tree, 0.1 + (0.2 + 0.3) = 0.6, where adding them by index would give
        // 0.6000000000000001: this pins the order.
        (
            "masses summed up the tree",
            pair(
                star(0, 0.1),
                pair(star(1, 0.2), star(2, 0.3), 0.05, 0.0),
                2.0,
                0.1,
            ),
            vec![
                zams_star(0.1, 0.5),
                zams_star(0.2, 0.5),
                zams_star(0.3, 0.5),
            ],
        ),
    ]
}

#[test]
fn zones_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for mu in [0.02, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95, 0.999] {
        for e in [0.0, 0.3, 0.7, 0.8, 0.9, 0.99] {
            w.f64(&format!("s_type {mu} {e}"), holman_wiegert_s_type(mu, e));
            w.f64(&format!("p_type {mu} {e}"), holman_wiegert_p_type(mu, e));
        }
    }
    let hierarchies = golden_hierarchies();
    let (seed, id) = (Seed::new(42), system([-3, 10, 0], 0));
    for (name, root, stars) in hierarchies {
        let hierarchy = ZoneHierarchy::new(root).unwrap();
        let zones = stable_zones(&hierarchy);
        w.line("");
        w.line(&format!(
            "{name}: {} components, {} zones",
            hierarchy.component_count(),
            zones.len()
        ));
        for zone in &zones {
            write_zone(&mut w, zone);
            let label = host_name(zone.host());
            let inputs = ZoneDiscInputs::for_zone(seed, id, zone, &stars, Dex::new(0.1)).unwrap();
            w.f64(&format!("{label} lifetime_myr"), inputs.lifetime().value());
            w.f64(
                &format!("{label} disc_luminosity"),
                inputs.host().zams_luminosity().value(),
            );
            match inputs.derive().profile() {
                Some(profile) => {
                    w.f64(
                        &format!("{label} disc inner_m"),
                        profile.inner_edge().value(),
                    );
                    w.f64(
                        &format!("{label} disc outer_m"),
                        profile.outer_edge().value(),
                    );
                    w.f64(
                        &format!("{label} disc solids_mearth"),
                        profile.solid_mass().value(),
                    );
                }
                None => w.line(&format!("{label} disc none")),
            }
        }
    }
    golden!("planetary/zones", w.as_str());
}
