//! The golden vectors of in-system apparent positions (rendering plan R03, R03.T3): what two
//! observers see of every present body and star of three pinned systems, at the epoch and a
//! century on, which the client's apparent positions are checked against (R03.T13).
//!
//! The systems are three of plan 14's golden systems (P14.T32), in the same fixture: `solar_like`
//! (a giant with seven moons), `wide_binary` (a G and K pair 94 au apart, e 0.69, whose K dwarf
//! and its planets reach beyond 60 au from the barycentre, where an `f64`'s step is larger) and
//! `close_binary` (a 1.96 + 1.35 M☉ pair 0.93 au apart, e 0.54: a close fast stellar pair, whose
//! stars are in the golden too). The observers are one in a low circular orbit about the system's
//! first planet present, 1.1 of its radius from its centre, moving at the planet's velocity plus
//! the circular speed, and one 30 au from the barycentre moving at 5 km/s.
//!
//! For each source: its velocity now (plan 14's `state_at`, plan 11's `star_states_at`), the
//! emitted time, the light time, the corrections, the last change, the
//! geometric position then and the apparent position; and for every body with a mass and a bound
//! orbit its Hill radius at pericentre by plan 14's `hill_radius`, with the primary's mass taken
//! from the orbit's gravitational parameter as a client has it (μ ÷ G − m), which R02's frame
//! selection reads.

use hyperion_sim::Seed;
use hyperion_sim::coords::{SystemPosition, SystemVelocity};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::SystemId;
use hyperion_sim::observe::{
    BodyTrack, InSystemRetardation, StarTrack, SystemObserver, SystemTrajectory, retarded_in_system,
};
use hyperion_sim::planetary::derive::limits::hill_radius;
use hyperion_sim::planetary::record::BodyKind;
use hyperion_sim::planetary::{self, PlanetarySystem, SystemContext};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::consts::GRAVITATIONAL_CONSTANT;
use hyperion_sim::units::{Kilograms, Metres};
use hyperion_sim::version::GENERATOR_VERSION;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The universe of plan 14's golden systems (P14.T32): the Milky Way fixture at this seed.
const SYSTEMS_SEED: u64 = 0x5eed_0000_0014_0032;

/// The three pinned systems, by plan 14's golden names and IDs (`planetary_golden.rs`).
const SYSTEMS: [(&str, u64); 3] = [
    ("solar_like", 0x4200_6cba_0000_0009),
    ("wide_binary", 0x41ff_ecae_0000_0004),
    ("close_binary", 0x4200_2cb2_0000_0009),
];

/// The astronomical unit, m (IAU 2012 Resolution B2).
const AU: f64 = 1.495_978_707e11;

/// Seconds in a Julian year.
const JULIAN_YEAR_S: i64 = 31_557_600;

/// A pinned system's context and its generated bodies.
fn pinned(galaxy: &Galaxy, raw: u64) -> (SystemContext, PlanetarySystem) {
    let id = SystemId::from_raw(raw).expect("a pinned ID is well formed");
    let ctx = SystemContext::for_system(galaxy, id).expect("a pinned ID names a system");
    let system = planetary::generate(galaxy.seed(), &ctx);
    (ctx, system)
}

/// The observer in a low circular orbit about the system's first planet present at `t`: 1.1 of
/// its radius from its centre along +x, moving at the planet's velocity plus the circular speed
/// along +y. `None` if no planet with a mass and a radius is present.
fn low_orbit(
    ctx: &SystemContext,
    system: &PlanetarySystem,
    t: UniverseTime,
) -> Option<SystemObserver> {
    system.bodies().iter().find_map(|body| {
        let record = system.body_at(ctx, body.index(), t).ok()?;
        if record.identity().kind() != BodyKind::Planet {
            return None;
        }
        let mass = Kilograms::from(*record.mass().ok()?).value();
        let radius = Metres::from(record.bulk().ok()?.radius()).value();
        let (at, velocity) = system.state_at(ctx, body.index(), t).ok()??;
        let orbit_radius = 1.1 * radius;
        let speed = (GRAVITATIONAL_CONSTANT * mass / orbit_radius).sqrt();
        let [px, py, pz] = at.metres();
        let [vx, vy, vz] = velocity.metres_per_second();
        SystemObserver::new(
            SystemPosition::new([px + orbit_radius, py, pz]),
            SystemVelocity::new([vx, vy + speed, vz]),
            t,
        )
        .ok()
    })
}

/// The observer 30 au from the barycentre along +x, moving at 5 km/s along +y.
fn far_out(t: UniverseTime) -> SystemObserver {
    SystemObserver::new(
        SystemPosition::new([30.0 * AU, 0.0, 0.0]),
        SystemVelocity::new([0.0, 5e3, 0.0]),
        t,
    )
    .expect("a finite observer slower than light in the window")
}

fn write_time(w: &mut GoldenWriter, label: &str, t: UniverseTime) {
    w.line(&format!(
        "{label} = {}s {}ns",
        t.seconds(),
        t.subsec_nanos()
    ));
}

fn write_span(w: &mut GoldenWriter, label: &str, span: Span) {
    w.line(&format!(
        "{label} = {}s {}ns",
        span.seconds(),
        span.subsec_nanos()
    ));
}

fn write_position(w: &mut GoldenWriter, label: &str, position: &SystemPosition) {
    for (axis, value) in ["x", "y", "z"].into_iter().zip(position.metres()) {
        w.f64(&format!("{label}_{axis}_m"), value);
    }
}

fn write_velocity(w: &mut GoldenWriter, label: &str, velocity: SystemVelocity) {
    for (axis, value) in ["x", "y", "z"]
        .into_iter()
        .zip(velocity.metres_per_second())
    {
        w.f64(&format!("{label}_{axis}_m_s"), value);
    }
}

/// Writes one reading, and returns the source's distance from the barycentre then, m.
fn write_seen(w: &mut GoldenWriter, seen: &InSystemRetardation) -> f64 {
    write_time(w, "emitted", seen.emitted());
    write_span(w, "light_time", seen.light_time());
    w.line(&format!("corrections = {}", seen.corrections()));
    write_span(w, "residual", seen.residual());
    write_position(w, "geometric", seen.geometric_then());
    write_position(w, "apparent", seen.apparent());
    seen.geometric_then().distance_from_origin().value()
}

/// What the golden records, beyond the text: how hard it works the evaluation.
#[derive(Debug, Default)]
struct Reach {
    farthest_body_m: f64,
    most_corrections: u8,
    stars: usize,
}

/// Writes every present body and star of `system` at `t` as `observer` sees it.
fn write_system(
    w: &mut GoldenWriter,
    reach: &mut Reach,
    ctx: &SystemContext,
    system: &PlanetarySystem,
    observer: &SystemObserver,
) {
    let t = observer.time();
    write_position(w, "observer", observer.position());
    write_velocity(w, "observer_velocity", observer.velocity());
    for body in system.bodies() {
        let track = BodyTrack::new(system, ctx, body.index()).expect("a body of the system");
        if track.position_at(t).is_none() {
            continue;
        }
        w.line(&format!("body {}", body.index().get()));
        // The track's velocity now, which plan 14's `state_at` gives and nothing else pins.
        if let Some(velocity) = track.velocity_at(t) {
            write_velocity(w, "velocity_now", velocity);
        }
        match retarded_in_system(observer, &track) {
            Ok(seen) => {
                reach.farthest_body_m = reach.farthest_body_m.max(write_seen(w, &seen));
                reach.most_corrections = reach.most_corrections.max(seen.corrections());
            }
            Err(error) => w.line(&format!("not seen: {error}")),
        }
        let record = system
            .body_at(ctx, body.index(), t)
            .expect("a body of the system");
        if let (Some(mass), Some(orbit)) = (record.mass().ok(), record.orbit().ok()) {
            let elements = orbit.elements();
            let m = Kilograms::from(*mass).value();
            // The primary's mass as a client has it: μ ÷ G less the body's own.
            let primary = elements.gravitational_parameter().value() / GRAVITATIONAL_CONSTANT - m;
            let e = elements.eccentricity().value();
            if primary > 0.0 && e < 1.0 {
                let hill = hill_radius(
                    elements.semi_major_axis(),
                    e,
                    Kilograms::new(m),
                    Kilograms::new(primary),
                );
                w.f64("hill_radius_m", hill.value());
            }
        }
    }
    for slot in ctx.hierarchy().stars() {
        let track = StarTrack::new(ctx.hierarchy(), slot.body()).expect("a star of the hierarchy");
        w.line(&format!("star {}", slot.body().body_index()));
        if let Some(velocity) = track.velocity_at(t) {
            write_velocity(w, "velocity_now", velocity);
        }
        match retarded_in_system(observer, &track) {
            Ok(seen) => {
                write_seen(w, &seen);
                reach.most_corrections = reach.most_corrections.max(seen.corrections());
                reach.stars += 1;
            }
            Err(error) => w.line(&format!("not seen: {error}")),
        }
    }
}

#[test]
fn in_system_apparent_positions_are_the_golden_vectors() {
    let galaxy = Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let century = Span::from_seconds(100 * JULIAN_YEAR_S);
    let times = [
        ("epoch", UniverseTime::EPOCH),
        (
            "plus_100_years",
            UniverseTime::EPOCH
                .checked_add(century)
                .expect("in the window"),
        ),
    ];
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut reach = Reach::default();
    for (name, raw) in SYSTEMS {
        let (ctx, system) = pinned(&galaxy, raw);
        for (when, t) in times {
            let observers = [
                ("low_orbit", low_orbit(&ctx, &system, t)),
                ("at_30_au", Some(far_out(t))),
            ];
            for (who, observer) in observers {
                match observer {
                    Some(observer) => {
                        w.line(&format!("{name} {when} {who}"));
                        write_system(&mut w, &mut reach, &ctx, &system, &observer);
                    }
                    None => w.line(&format!("{name} {when} {who}: no planet present")),
                }
            }
        }
    }
    // The systems work the bound of Design note 7 where an `f64`'s step is larger, and the
    // iteration where it works harder, as R03.T3 asks.
    println!(
        "farthest body {:.1} au; most corrections {}; {} star readings",
        reach.farthest_body_m / AU,
        reach.most_corrections,
        reach.stars
    );
    assert!(
        reach.farthest_body_m > 60.0 * AU,
        "the farthest body is {} au out",
        reach.farthest_body_m / AU
    );
    assert!(reach.stars >= 20, "{} star readings", reach.stars);
    golden!("observe/in_system", w.as_str());
}
