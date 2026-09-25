//! A system's bodies and one body's record over a real socket: pinned systems against the sim's
//! own `snapshot_at` and `body_at`, the refusals, the detail levels, and the cache (plan 14,
//! P14.T36).
//!
//! Every universe here has one fixed seed, so the systems are the ones the sim generates for that
//! seed: the test builds the same galaxy and the same planetary systems, and checks the wire
//! against them bit for bit, which the server's exact float parsing (ruling 64.7) allows.

mod common;

use std::sync::OnceLock;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    BodyDetailDto, BodyDetailRequest, BodyIdHex, BodyKindDto, BodyStateDto, DetailLevelDto,
    ErrorCode, OrbitHostDto, RequestBody, RequestError, ResponseBody, SectionDto, SystemBodiesDto,
    SystemBodiesRequest, SystemIdHex, SystemSummaryRequest, UniverseIdHex, UniverseTime,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, candidate_count};
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::architecture::HostMultiplicity;
use hyperion_sim::planetary::disc::snow_line;
use hyperion_sim::planetary::fate::BodyState;
use hyperion_sim::planetary::placement::{OrbitHost, ZoneDiscInputs};
use hyperion_sim::planetary::record::{BodyRecord, Section};
use hyperion_sim::planetary::{PlanetaryHost, PlanetarySystem, SystemContext, generate};
use hyperion_sim::units::{Kilograms, Metres};
use hyperion_sim::{Seed, time};

/// The seed of every universe these tests create, the other integration tests' own.
const SEED: u64 = 0x4d2;

/// Seconds in a Julian year, the sim's year: the wire carries a time as whole seconds.
const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

/// Pinned systems of layer C at the solar circle, 26,000 ly out on the +y axis: a single star with
/// a rocky planet and a gas giant; a K1 V and M1 V pair with eight planets about three zones, one
/// a gas giant; and a triple whose primary is a white dwarf, fifteen planets about five zones, one
/// of them engulfed before the epoch.
const PINNED: [u64; 3] = [
    0x4200_2cb2_0000_0003,
    0x4200_2cb2_0000_0000,
    0x4200_2cb2_0000_0005,
];

/// A pinned single star with no planets.
const EMPTY: u64 = 0x4200_2cb2_0000_0001;

/// The galaxy the server builds for [`SEED`], to hold its answers to.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
}

/// The sim's context and planetary system of the system `raw` names.
fn sim_system(raw: u64) -> (SystemContext, PlanetarySystem) {
    let id = SystemId::from_raw(raw).expect("a pinned ID is a system ID");
    let ctx = SystemContext::for_system(galaxy(), id).expect("a pinned ID names a system");
    let planets = generate(Seed::new(SEED), &ctx);
    (ctx, planets)
}

fn at_years(years: i64) -> UniverseTime {
    UniverseTime {
        seconds: years * SECONDS_PER_JULIAN_YEAR,
        nanos: 0,
    }
}

fn sim_time(time: UniverseTime) -> time::UniverseTime {
    time::UniverseTime::new(time.seconds, time.nanos).expect("a valid time")
}

fn bodies_request(
    universe: &UniverseIdHex,
    system: u64,
    time: UniverseTime,
    detail: DetailLevelDto,
) -> RequestBody {
    RequestBody::SystemBodies(SystemBodiesRequest {
        universe: universe.clone(),
        system: SystemIdHex::from_u64(system),
        time,
        detail,
    })
}

fn detail_request(
    universe: &UniverseIdHex,
    body: BodyIdHex,
    time: UniverseTime,
    detail: DetailLevelDto,
) -> RequestBody {
    RequestBody::BodyDetail(BodyDetailRequest {
        universe: universe.clone(),
        body,
        time,
        detail,
    })
}

/// A server, a client that has said hello, and a universe of [`SEED`].
async fn started() -> (TestServer, TestClient, UniverseIdHex) {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach", SEED).await.id;
    (server, client, universe)
}

async fn bodies(client: &mut TestClient, body: RequestBody) -> SystemBodiesDto {
    match client.request(body).await {
        Ok(ResponseBody::SystemBodies(answer)) => *answer,
        other => panic!("expected a system's bodies, got {other:?}"),
    }
}

async fn detail(client: &mut TestClient, body: RequestBody) -> BodyDetailDto {
    match client.request(body).await {
        Ok(ResponseBody::BodyDetail(answer)) => *answer,
        other => panic!("expected a body's record, got {other:?}"),
    }
}

async fn refused(client: &mut TestClient, body: RequestBody) -> RequestError {
    match client.request(body).await {
        Err(error) => error,
        Ok(body) => panic!("expected a refusal, got {body:?}"),
    }
}

/// Holds a section's state on the wire to the sim's, whatever its value.
fn assert_same_state<T, U>(what: &str, wire: &SectionDto<T>, sim: &Section<U>) {
    let same = matches!(
        (wire, sim),
        (SectionDto::Ok(_), Section::Ok(_))
            | (SectionDto::NotResolved, Section::NotResolved)
            | (SectionDto::NotModelled, Section::NotModelled)
            | (SectionDto::NotApplicable, Section::NotApplicable)
    );
    assert!(same, "{what}: the wire's state is not the sim's");
}

/// The wire's form of what a body or zone orbits.
fn host_dto(system: SystemId, host: OrbitHost) -> OrbitHostDto {
    match host {
        OrbitHost::Star(n) => OrbitHostDto::Star {
            body_index: u16::from(n),
        },
        OrbitHost::Pair(k) => OrbitHostDto::Pair {
            key_body_index: u16::from(k),
        },
        OrbitHost::Barycentre => OrbitHostDto::Barycentre,
        OrbitHost::Body(index) => OrbitHostDto::Body {
            id: BodyIdHex::from_parts(system.raw(), index.get()),
        },
    }
}

/// Holds one body on the wire, its fields shared by the list and the whole record, to the sim's
/// record of it, bit for bit.
#[expect(
    clippy::too_many_arguments,
    reason = "the fields of the summary and of the record are passed apart, since they are two types"
)]
fn assert_body_is_the_sims(
    sim: &BodyRecord,
    id: &BodyIdHex,
    kind: BodyKindDto,
    label: &SectionDto<String>,
    parent: Option<&OrbitHostDto>,
    state: BodyStateDto,
    position_m: Option<[f64; 3]>,
    mass_kg: &SectionDto<f64>,
    orbit: &SectionDto<hyperion_protocol::BodyOrbitDto>,
    bulk: &SectionDto<hyperion_protocol::BulkPropertiesDto>,
) {
    let identity = sim.identity();
    let system = identity.system();
    let what = id.as_str();
    assert_eq!(id.to_parts(), (system.raw(), identity.index().get()));
    assert_eq!(kind, BodyKindDto::Planet, "{what}: every body of the slice");
    match (label, identity.label()) {
        (SectionDto::Ok(wire), Section::Ok(sim)) => assert_eq!(wire, sim.as_str(), "{what}"),
        (wire, sim) => assert_same_state(what, wire, sim),
    }
    assert_eq!(
        parent,
        identity
            .parent()
            .map(|host| host_dto(system, host))
            .as_ref(),
        "{what}"
    );
    match (state, identity.state()) {
        (BodyStateDto::Present, BodyState::Present)
        | (BodyStateDto::NotYetFormed, BodyState::NotYetFormed) => {}
        (
            BodyStateDto::Destroyed { cause, at },
            BodyState::Destroyed {
                cause: sim_cause,
                at: sim,
            },
        ) => {
            assert_eq!(sim_time(at), sim, "{what}");
            assert_eq!(
                serde_json::to_value(cause).unwrap(),
                snake_case(&format!("{sim_cause:?}")),
                "{what}: cause"
            );
        }
        (BodyStateDto::Unbound { at }, BodyState::Unbound { at: sim }) => {
            assert_eq!(sim_time(at), sim, "{what}");
        }
        (wire, sim) => panic!("{what}: the wire says {wire:?}, the sim {sim:?}"),
    }
    assert_eq!(
        position_m.map(|p| p.map(f64::to_bits)),
        sim.position().map(|p| p.metres().map(f64::to_bits)),
        "{what}: position"
    );
    match (mass_kg, sim.mass()) {
        (SectionDto::Ok(wire), Section::Ok(sim)) => {
            assert_eq!(
                wire.to_bits(),
                Kilograms::from(*sim).value().to_bits(),
                "{what}"
            );
        }
        (wire, sim) => assert_same_state(what, wire, sim),
    }
    assert_orbit_is_the_sims(what, orbit, sim.orbit(), parent);
    assert_bulk_is_the_sims(what, bulk, sim.bulk());
}

/// Holds a body's orbit section on the wire to the sim's, bit for bit, about `parent`.
fn assert_orbit_is_the_sims(
    what: &str,
    orbit: &SectionDto<hyperion_protocol::BodyOrbitDto>,
    sim: &Section<hyperion_sim::planetary::record::BodyOrbit>,
    parent: Option<&OrbitHostDto>,
) {
    match (orbit, sim) {
        (SectionDto::Ok(wire), Section::Ok(sim)) => {
            let elements = sim.elements();
            assert_eq!(Some(&wire.parent), parent, "{what}: the orbit's parent");
            for (wire, sim) in [
                (wire.orbit.period_s, elements.period().value()),
                (
                    wire.orbit.semi_major_axis_m,
                    elements.semi_major_axis().value(),
                ),
                (wire.orbit.eccentricity, elements.eccentricity().value()),
                (wire.orbit.inclination_rad, elements.inclination().value()),
                (
                    wire.orbit.ascending_node_rad,
                    elements.ascending_node().value(),
                ),
                (
                    wire.orbit.argument_of_periapsis_rad,
                    elements.argument_of_periapsis().value(),
                ),
                (
                    wire.orbit.mean_anomaly_at_epoch_rad,
                    elements.mean_anomaly_at_epoch().value(),
                ),
                (
                    wire.orbit.mu_m3_s2,
                    elements.gravitational_parameter().value(),
                ),
            ] {
                assert_eq!(wire.to_bits(), sim.to_bits(), "{what}: orbit");
            }
            assert_eq!(
                wire.valid_until.map(sim_time),
                sim.valid_until(),
                "{what}: valid until"
            );
        }
        (wire, sim) => assert_same_state(what, wire, sim),
    }
}

/// Holds a body's bulk section on the wire to the sim's, bit for bit.
fn assert_bulk_is_the_sims(
    what: &str,
    bulk: &SectionDto<hyperion_protocol::BulkPropertiesDto>,
    sim: &Section<hyperion_sim::planetary::record::BulkProperties>,
) {
    match (bulk, sim) {
        (SectionDto::Ok(wire), Section::Ok(sim)) => {
            let fractions = sim.fractions();
            for (wire, sim) in [
                (wire.radius_m, Metres::from(sim.radius()).value()),
                (wire.density_kg_m3, sim.density().value()),
                (wire.surface_gravity_m_s2, sim.surface_gravity().value()),
                (wire.mass_fractions.iron, fractions.iron()),
                (wire.mass_fractions.rock, fractions.rock()),
                (wire.mass_fractions.water, fractions.water()),
                (wire.mass_fractions.envelope, fractions.envelope()),
                (
                    wire.equilibrium_temperature_k,
                    sim.equilibrium_temperature().value(),
                ),
            ] {
                assert_eq!(wire.to_bits(), sim.to_bits(), "{what}: bulk");
            }
            assert_eq!(
                serde_json::to_value(wire.class).unwrap(),
                snake_case(&format!("{:?}", sim.class())),
                "{what}: class"
            );
        }
        (wire, sim) => assert_same_state(what, wire, sim),
    }
}

/// A Rust variant's name as the wire's snake-case string: `GasGiant` is `gas_giant`.
fn snake_case(name: &str) -> String {
    name.chars()
        .enumerate()
        .fold(String::new(), |mut out, (i, c)| {
            if c.is_ascii_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            out
        })
}

/// Holds a `system_bodies` answer to the sim's own system at the answer's time.
fn assert_bodies_are_the_sims(
    answer: &SystemBodiesDto,
    ctx: &SystemContext,
    planets: &PlanetarySystem,
) {
    let t = sim_time(answer.hosts.time);
    let snapshot = planets.snapshot_at(ctx, t);
    assert_eq!(answer.bodies.len(), snapshot.bodies().len());
    for (wire, sim) in answer.bodies.iter().zip(snapshot.bodies()) {
        assert_body_is_the_sims(
            sim,
            &wire.id,
            wire.kind,
            &wire.label,
            wire.parent.as_ref(),
            wire.state,
            wire.position_m,
            &wire.mass_kg,
            &wire.orbit,
            &wire.bulk,
        );
        assert_same_state(wire.id.as_str(), &wire.moons, sim.moons());
        assert_same_state(wire.id.as_str(), &wire.rings, sim.rings());
    }
    assert_same_state("belts", &answer.belts, snapshot.belts());
    assert_same_state("halo", &answer.halo, snapshot.halo());
    assert_zones_are_the_sims(answer, ctx, planets, t);
    assert_system_plane_is_the_sims(answer, planets);
}

/// Holds a `system_bodies` answer's zones to the sim's at `t`: their hosts, limits, classes, snow
/// lines, planes and habitable zones, bit for bit.
fn assert_zones_are_the_sims(
    answer: &SystemBodiesDto,
    ctx: &SystemContext,
    planets: &PlanetarySystem,
    t: time::UniverseTime,
) {
    assert_eq!(answer.zones.len(), planets.zones().len());
    for ((wire, zone), host) in answer
        .zones
        .iter()
        .zip(planets.zones())
        .zip(planets.hosts())
    {
        assert_eq!(wire.host, host_dto(planets.system(), zone.host()));
        assert_eq!(
            wire.inner_m.map(f64::to_bits),
            zone.inner().map(|m| m.value().to_bits())
        );
        assert_eq!(
            wire.outer_m.map(f64::to_bits),
            zone.outer().map(|m| m.value().to_bits())
        );
        assert_eq!(
            serde_json::to_value(wire.architecture).unwrap(),
            snake_case(&format!("{:?}", host.class())),
            "the zone's class is its host's as placed"
        );
        let inputs = ZoneDiscInputs::for_zone(
            Seed::new(SEED),
            planets.system(),
            zone,
            &ctx.zone_stars(),
            ctx.fe_h(),
        )
        .unwrap();
        assert_eq!(
            wire.snow_line_m.to_bits(),
            snow_line(inputs.host().zams_luminosity()).value().to_bits(),
            "the zone's snow line is its host's at zero age"
        );
        if let Some(disc) = host.disc().profile() {
            assert_eq!(
                wire.snow_line_m.to_bits(),
                disc.snow_line().value().to_bits(),
                "the zone's snow line is its disc's"
            );
        }
        assert_eq!(
            wire.plane.inclination_rad.to_bits(),
            host.plane().inclination().value().to_bits()
        );
        assert_eq!(
            wire.plane.ascending_node_rad.to_bits(),
            host.plane().node().value().to_bits()
        );
        let habitable = planets.habitable_zone_at(ctx, zone.host(), t);
        assert_eq!(wire.habitable_zone.is_some(), habitable.is_some());
        if let (Some(wire), Some(sim)) = (&wire.habitable_zone, habitable) {
            let finite = |m: Metres| Some(m.value()).filter(|v| v.is_finite()).map(f64::to_bits);
            for (wire, sim) in [
                (wire.recent_venus_m, sim.recent_venus()),
                (wire.runaway_greenhouse_m, sim.runaway_greenhouse()),
                (wire.moist_greenhouse_m, sim.moist_greenhouse()),
                (wire.maximum_greenhouse_m, sim.maximum_greenhouse()),
                (wire.early_mars_m, sim.early_mars()),
            ] {
                assert_eq!(
                    wire.map(f64::to_bits),
                    finite(sim),
                    "a habitable zone's limit"
                );
            }
            assert_eq!(wire.extrapolated, sim.extrapolated());
        }
    }
}

/// Holds a `system_bodies` answer's system plane to the sim's host planes by ruling 69.4's rule.
fn assert_system_plane_is_the_sims(answer: &SystemBodiesDto, planets: &PlanetarySystem) {
    // Plan 14's reference plane (ruling 69.4): the innermost zone holding star 0, or the next one
    // out when that zone's host is a member of a close binary, or failing both the first zone.
    let mut holding = planets
        .zones()
        .iter()
        .zip(planets.hosts())
        .filter(|(zone, _)| zone.members().any(|member| member == 0));
    let primary = match holding.next() {
        Some((zone, _)) if zone.host_multiplicity() == HostMultiplicity::CloseBinary => {
            holding.next()
        }
        innermost => innermost,
    };
    let plane = primary
        .map(|(_, host)| host)
        .or_else(|| planets.hosts().first())
        .map(PlanetaryHost::plane);
    let born = !answer.zones.is_empty();
    assert_eq!(
        answer
            .system_plane
            .map(|p| (p.inclination_rad.to_bits(), p.ascending_node_rad.to_bits())),
        plane.filter(|_| born).map(|p| (
            p.inclination().value().to_bits(),
            p.node().value().to_bits()
        )),
        "the system plane"
    );
}

/// P14.T36's golden-style test: pinned systems' bodies, at three times, are the sim's own snapshot
/// bit for bit, and their hosts are the `system_summary` answer.
#[tokio::test]
async fn a_pinned_systems_bodies_are_the_sims_snapshot() {
    let (server, mut client, universe) = started().await;
    let mut shapes = Vec::new();
    for raw in PINNED {
        let (ctx, planets) = sim_system(raw);
        for years in [0, -500, 731] {
            let time = at_years(years);
            let answer = bodies(
                &mut client,
                bodies_request(&universe, raw, time, DetailLevelDto::Full),
            )
            .await;
            assert_eq!(answer.granted, DetailLevelDto::Full);
            assert_eq!(answer.hosts.system, SystemIdHex::from_u64(raw));
            assert_eq!(answer.hosts.time, time);
            let hosts = match client
                .request(RequestBody::SystemSummary(SystemSummaryRequest {
                    universe: universe.clone(),
                    system: SystemIdHex::from_u64(raw),
                    time,
                }))
                .await
            {
                Ok(ResponseBody::SystemSummary(hosts)) => hosts,
                other => panic!("expected a summary, got {other:?}"),
            };
            assert_eq!(answer.hosts, hosts, "the hosts are the system's summary");
            assert_bodies_are_the_sims(&answer, &ctx, &planets);
        }
        shapes.push((ctx.stars().len(), planets.zones().len()));
    }
    assert_eq!(
        shapes,
        [(1, 1), (2, 3), (3, 5)],
        "the pinned systems are what their names say"
    );
    client.close().await;
    server.stop().await;
}

/// Systems whose bodies are in every state a record can carry, one found for each by sampling the
/// universe of [`SEED`] (`val14`, round 8): a black hole whose survivors a supernova left on
/// eccentric orbits, a neutron star whose planets it unbound, a white dwarf that engulfed a planet,
/// and a young star with giants still forming. The neutron star is re-picked after plan 11's
/// ruling 81 (`0x81ff_b2a0_0000_0002` no longer unbinds a planet): `0x81fa_b2e0_0000_0002`, a
/// neutron star and its companion, the first such system of layer E at the solar circle.
const VARIED: [u64; 4] = [
    0x81ff_b29f_f000_0007,
    0x81fa_b2e0_0000_0002,
    0x41ff_6cae_0000_0005,
    0x01ff_fb2b_2005_0000,
];

/// The wire carries every state a body can be in exactly as the sim has it, at both ends of the
/// clock window and the epoch: the states and their times and causes, the orbits after a
/// supernova, and the zones and their habitable zones about remnants.
#[tokio::test]
async fn bodies_in_every_state_are_the_sims_snapshot() {
    let (server, mut client, universe) = started().await;
    let mut seen = [false; 4];
    for raw in VARIED {
        let (ctx, planets) = sim_system(raw);
        for years in [-1_000, 0, 1_000] {
            let time = at_years(years);
            let answer = bodies(
                &mut client,
                bodies_request(&universe, raw, time, DetailLevelDto::Full),
            )
            .await;
            assert_bodies_are_the_sims(&answer, &ctx, &planets);
            for body in &answer.bodies {
                let state = match body.state {
                    BodyStateDto::Present => 0,
                    BodyStateDto::Unbound { .. } => 1,
                    BodyStateDto::Destroyed { .. } => 2,
                    BodyStateDto::NotYetFormed => 3,
                };
                seen[state] = true;
            }
        }
    }
    assert_eq!(
        seen, [true; 4],
        "present, unbound, destroyed and unformed bodies"
    );
    client.close().await;
    server.stop().await;
}

/// Every body's `body_detail` record is the sim's `body_at`, with its surface and hooks tagged as
/// the sim tags them, and agrees with the system's list.
#[tokio::test]
async fn each_bodys_record_is_the_sims_and_the_lists() {
    let (server, mut client, universe) = started().await;
    let raw = PINNED[2];
    let (ctx, planets) = sim_system(raw);
    let time = at_years(-250);
    let list = bodies(
        &mut client,
        bodies_request(&universe, raw, time, DetailLevelDto::Full),
    )
    .await;
    let mut states = Vec::new();
    for (body, summary) in planets.bodies().iter().zip(&list.bodies) {
        let answer = detail(
            &mut client,
            detail_request(&universe, summary.id.clone(), time, DetailLevelDto::Full),
        )
        .await;
        assert_eq!(
            (answer.universe.clone(), answer.time),
            (universe.clone(), time)
        );
        assert_eq!(answer.granted, DetailLevelDto::Full);
        let sim = planets.body_at(&ctx, body.index(), sim_time(time)).unwrap();
        let record = &answer.record;
        assert_body_is_the_sims(
            &sim,
            &record.id,
            record.kind,
            &record.label,
            record.parent.as_ref(),
            record.state,
            record.position_m,
            &record.mass_kg,
            &record.orbit,
            &record.bulk,
        );
        assert_same_state(record.id.as_str(), &record.surface, sim.surface());
        assert_same_state(record.id.as_str(), &record.hooks, sim.hooks());
        assert_eq!(record.hooks, SectionDto::NotModelled);
        // The list's entry is the record, less its surface and hooks.
        assert_eq!(
            serde_json::to_value(summary).unwrap(),
            {
                let mut whole = serde_json::to_value(record).unwrap();
                let fields = whole.as_object_mut().unwrap();
                fields.remove("surface");
                fields.remove("hooks");
                whole
            },
            "{}",
            record.id
        );
        states.push(record.state);
    }
    assert!(
        states
            .iter()
            .any(|state| matches!(state, BodyStateDto::Destroyed { .. })),
        "the pinned triple's engulfed planet is among them: {states:?}"
    );
    client.close().await;
    server.stop().await;
}

/// A well-formed body ID whose index is in plan 14's layout but names no body is `unknown_body`:
/// an unused planet slot, a planet of a system that has none, and a star, whose record is the
/// system summary's.
#[tokio::test]
async fn a_body_the_system_does_not_hold_is_unknown_body() {
    let (server, mut client, universe) = started().await;
    let (_, planets) = sim_system(PINNED[0]);
    let unused = planets
        .bodies()
        .last()
        .expect("the pinned single star has planets")
        .index()
        .get()
        + 0x0100;
    for body in [
        BodyIdHex::from_parts(PINNED[0], unused),
        BodyIdHex::from_parts(EMPTY, 0x0100),
        BodyIdHex::from_parts(PINNED[0], 0x0000),
        BodyIdHex::from_parts(PINNED[1], 0x0001),
    ] {
        let error = refused(
            &mut client,
            detail_request(&universe, body.clone(), at_years(0), DetailLevelDto::Full),
        )
        .await;
        assert_eq!(error.code, ErrorCode::UnknownBody, "{body}: {error:?}");
        assert_eq!(error.field.as_deref(), Some("body"), "{error:?}");
    }
    client.close().await;
    server.stop().await;
}

/// A body index outside plan 14's layout is a `bad_request` naming `body`; a system part that names
/// no system is `unknown_system` naming `body`; and the time and the system of `system_bodies` are
/// refused as `system_summary` refuses them.
#[tokio::test]
async fn a_malformed_index_or_an_unknown_system_is_refused_naming_the_field() {
    let (server, mut client, universe) = started().await;
    let pinned = PINNED[0];
    // A reserved slot, and a sub-index a planet's slot reserves.
    for index in [0xd000, 0x0190] {
        let error = refused(
            &mut client,
            detail_request(
                &universe,
                BodyIdHex::from_parts(pinned, index),
                at_years(0),
                DetailLevelDto::Full,
            ),
        )
        .await;
        assert_eq!(error.code, ErrorCode::BadRequest, "{index:#06x}: {error:?}");
        assert_eq!(error.field.as_deref(), Some("body"), "{error:?}");
    }

    // Bits that are no system ID, and the index after a cell's last candidate.
    let cell = CellKey::of(SystemId::from_raw(pinned).expect("a system ID")).expect("a grid ID");
    let beyond = cell
        .candidate_id(candidate_count(galaxy(), cell))
        .expect("a cell at the solar circle has room for another index")
        .raw();
    for raw in [u64::MAX, beyond] {
        let error = refused(
            &mut client,
            detail_request(
                &universe,
                BodyIdHex::from_parts(raw, 0x0100),
                at_years(0),
                DetailLevelDto::Full,
            ),
        )
        .await;
        assert_eq!(
            error.code,
            ErrorCode::UnknownSystem,
            "{raw:#018x}: {error:?}"
        );
        assert_eq!(error.field.as_deref(), Some("body"), "{error:?}");
        let error = refused(
            &mut client,
            bodies_request(&universe, raw, at_years(0), DetailLevelDto::Full),
        )
        .await;
        assert_eq!(
            error.code,
            ErrorCode::UnknownSystem,
            "{raw:#018x}: {error:?}"
        );
        assert_eq!(error.field.as_deref(), Some("system"), "{error:?}");
    }

    // A time outside the clock window is named before the body or the system.
    for body in [
        detail_request(
            &universe,
            BodyIdHex::from_parts(pinned, 0xd000),
            at_years(1_001),
            DetailLevelDto::Full,
        ),
        bodies_request(&universe, u64::MAX, at_years(-1_001), DetailLevelDto::Full),
    ] {
        let error = refused(&mut client, body).await;
        assert_eq!(error.code, ErrorCode::BadRequest, "{error:?}");
        assert_eq!(error.field.as_deref(), Some("time"), "{error:?}");
    }

    // A body ID that is not one does not parse: plan 04's `bad_request`, for the request's own ID.
    client
        .send_raw(&format!(
            r#"{{"type":"request","id":90,"body":{{"kind":"body_detail","universe":"{}","body":"4200.0100","time":{{"seconds":0,"nanos":0}},"detail":"full"}}}}"#,
            universe.as_str()
        ))
        .await;
    match client.next_message().await {
        hyperion_protocol::ServerMessage::RequestError { id, error } => {
            assert_eq!(id.0, 90);
            assert_eq!(error.code, ErrorCode::BadRequest, "{error:?}");
        }
        other => panic!("expected a bad request, got {other:?}"),
    }

    // Refusals are answers: nothing refused was generated, and the connection carries on.
    let answer = bodies(
        &mut client,
        bodies_request(&universe, pinned, at_years(0), DetailLevelDto::Full),
    )
    .await;
    assert!(!answer.bodies.is_empty());
    let counters = server.stats().bodies();
    assert_eq!(
        (counters.generated(), counters.cache().entries()),
        (1, 1),
        "no refused ID was generated or cached"
    );
    client.close().await;
    server.stop().await;
}

/// `mass_and_orbit` sends no bulk section and no key of one, and `contact` withholds the kind and
/// the label too; the level granted is the level asked for.
#[tokio::test]
async fn mass_and_orbit_gives_no_bulk_section() {
    let (server, mut client, universe) = started().await;
    let raw = PINNED[1];
    let id = client
        .send_request(bodies_request(
            &universe,
            raw,
            at_years(0),
            DetailLevelDto::MassAndOrbit,
        ))
        .await;
    let frame = client.next_text().await;
    assert!(frame.contains(&format!(r#""id":{}"#, id.0)), "{frame}");
    for key in [
        "radius_m",
        "density_kg_m3",
        "equilibrium_temperature_k",
        "mass_fractions",
    ] {
        assert!(
            !frame.contains(key),
            "{key} is sent at mass_and_orbit: {frame}"
        );
    }
    let answer = match serde_json::from_str(&frame).unwrap() {
        hyperion_protocol::ServerMessage::Response {
            body: ResponseBody::SystemBodies(answer),
            ..
        } => *answer,
        other => panic!("expected a system's bodies, got {other:?}"),
    };
    assert_eq!(answer.granted, DetailLevelDto::MassAndOrbit);
    assert!(!answer.bodies.is_empty());
    for body in &answer.bodies {
        assert_eq!(body.bulk, SectionDto::NotResolved, "{}", body.id);
        assert!(matches!(body.mass_kg, SectionDto::Ok(_)), "{}", body.id);
        assert!(matches!(body.label, SectionDto::Ok(_)), "{}", body.id);
    }

    let first = answer.bodies[0].id.clone();
    let record = detail(
        &mut client,
        detail_request(
            &universe,
            first.clone(),
            at_years(0),
            DetailLevelDto::MassAndOrbit,
        ),
    )
    .await;
    assert_eq!(record.granted, DetailLevelDto::MassAndOrbit);
    assert_eq!(record.record.bulk, SectionDto::NotResolved);
    assert_eq!(record.record.surface, SectionDto::NotResolved);
    assert_eq!(record.record.hooks, SectionDto::NotResolved);

    let contact = detail(
        &mut client,
        detail_request(&universe, first, at_years(0), DetailLevelDto::Contact),
    )
    .await;
    assert_eq!(contact.granted, DetailLevelDto::Contact);
    assert_eq!(contact.record.kind, BodyKindDto::Unresolved);
    assert_eq!(contact.record.label, SectionDto::NotResolved);
    assert_eq!(contact.record.mass_kg, SectionDto::NotResolved);
    assert!(
        contact.record.position_m.is_some(),
        "a contact keeps its position"
    );
    client.close().await;
    server.stop().await;
}

/// The frame of a response, from its body on: what two answers to one request share.
fn body_of(frame: &str) -> &str {
    frame.split_once(r#""body":"#).map_or_else(
        || panic!("a response frame has a body: {frame}"),
        |(_, body)| body,
    )
}

/// The same request twice gives the same frame, byte for byte but for its ID, the first generated
/// and the second from the cache; another time, a body's record and a second universe of the seed
/// generate nothing more.
#[tokio::test]
async fn a_repeat_is_served_from_the_cache() {
    let (server, mut client, universe) = started().await;
    let raw = PINNED[1];
    let mut frames = Vec::new();
    for _ in 0..2 {
        let id = client
            .send_request(bodies_request(
                &universe,
                raw,
                at_years(250),
                DetailLevelDto::Full,
            ))
            .await;
        let frame = client.next_text().await;
        assert!(
            frame.starts_with(&format!(r#"{{"type":"response","id":{}"#, id.0)),
            "{frame}"
        );
        frames.push(frame);
    }
    assert_eq!(body_of(&frames[0]), body_of(&frames[1]));
    let counters = server.stats().bodies();
    assert_eq!(counters.generated(), 1);
    assert_eq!(
        (
            counters.cache().misses(),
            counters.cache().hits(),
            counters.cache().entries()
        ),
        (1, 1, 1)
    );
    assert!(counters.cache().bytes() > 0 && counters.cache().bytes() <= counters.cache().budget());
    // The stars were evolved once, for both caches.
    assert_eq!(server.stats().systems().misses(), 1);

    let (ctx, planets) = sim_system(raw);
    let earlier = bodies(
        &mut client,
        bodies_request(&universe, raw, at_years(-250), DetailLevelDto::Full),
    )
    .await;
    assert_bodies_are_the_sims(&earlier, &ctx, &planets);
    let first = earlier.bodies[0].id.clone();
    let _ = detail(
        &mut client,
        detail_request(&universe, first, at_years(10), DetailLevelDto::Bulk),
    )
    .await;
    let twin = client.create_universe("Kepler Reach II", SEED).await.id;
    let id = client
        .send_request(bodies_request(
            &twin,
            raw,
            at_years(250),
            DetailLevelDto::Full,
        ))
        .await;
    let frame = client.next_text().await;
    assert!(frame.contains(&format!(r#""id":{}"#, id.0)), "{frame}");
    assert_eq!(
        body_of(&frame).replace(twin.as_str(), universe.as_str()),
        body_of(&frames[0]),
        "a second universe of the seed shares the entry (plan 04, design note 23)"
    );
    let counters = server.stats().bodies();
    assert_eq!((counters.generated(), counters.cache().hits()), (1, 4));
    client.close().await;
    server.stop().await;
}
