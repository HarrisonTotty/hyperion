//! The `system_bodies` answers for the three systems of the sim's in-system golden (rendering plan
//! R03, R03.T13), written by the server's own converters, so that the client can build the tracks
//! the sim's `retarded_in_system` read and check its apparent positions against
//! `crates/hyperion-sim/tests/golden/observe/in_system.golden`.
//!
//! That golden holds what two observers see, not the elements the client propagates, and its
//! galaxy is the Milky Way fixture at its seed, which no universe the server opens builds (a
//! universe draws its galaxy's parameters from its seed). So the answers are built here from that
//! galaxy, through the same steps as the `system_bodies` handler (the stars, the context from them,
//! the planetary system, the hosts' summary and the converter), at the `mass_and_orbit` level, all
//! an apparent position and a Hill radius need, and pinned as the JSON frames a client receives.
//!
//! It is a module of its own rather than a test in `planetary.rs`, since it covers the hosts'
//! summary (`stellar.rs`) and the bodies together, and it cannot be an integration test, since the
//! converters are the crate's own. Its seed and systems are those of the sim's test; the client's
//! test fails if the two goldens describe different systems.

use hyperion_protocol::{
    DetailLevelDto, RequestId, ResponseBody, ServerMessage, SystemBodiesRequest, SystemIdHex,
    UniverseIdHex, UniverseTime,
};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::resolve;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::{SystemContext, generate};
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::version::GENERATOR_VERSION;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

use super::{BodiesRequest, hosts_request, system_bodies, system_summary};

/// The seed of the sim's in-system golden (`observe_in_system_golden.rs`): plan 14's golden
/// systems' universe (P14.T32).
const SYSTEMS_SEED: u64 = 0x5eed_0000_0014_0032;

/// The three systems of the in-system golden, in its order.
const SYSTEMS: [u64; 3] = [
    0x4200_6cba_0000_0009,
    0x41ff_ecae_0000_0004,
    0x4200_2cb2_0000_0009,
];

/// Seconds in a Julian year, the sim's year.
const JULIAN_YEAR_S: i64 = 31_557_600;

#[test]
fn the_in_system_golden_systems_are_pinned_as_the_client_receives_them() {
    let galaxy = Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let universe = UniverseIdHex::from_u64(SYSTEMS_SEED);
    let times = [
        UniverseTime {
            seconds: 0,
            nanos: 0,
        },
        UniverseTime {
            seconds: 100 * JULIAN_YEAR_S,
            nanos: 0,
        },
    ];
    let mut golden = GoldenWriter::new();
    golden.header(GENERATOR_VERSION.get());
    let mut id = 0;
    for raw in SYSTEMS {
        let system = SystemId::from_raw(raw).expect("a pinned ID is a system ID");
        let record = resolve(&galaxy, system).expect("a pinned ID names a system");
        let stars = SystemStars::generate(&galaxy, &record);
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        let planets = generate(galaxy.seed(), &ctx);
        for time in times {
            let request = SystemBodiesRequest {
                universe: universe.clone(),
                system: SystemIdHex::from_u64(raw),
                time,
                detail: DetailLevelDto::MassAndOrbit,
            };
            let wanted = BodiesRequest::try_from(&request).expect("a time in the window");
            let hosts = system_summary(hosts_request(&request), &stars, wanted.time());
            let bodies = system_bodies(wanted, hosts, &ctx, &planets, galaxy.seed());
            assert!(!bodies.bodies.is_empty(), "system {raw:#x} has bodies");
            id += 1;
            let frame = serde_json::to_string(&ServerMessage::Response {
                id: RequestId(id),
                body: ResponseBody::SystemBodies(Box::new(bodies)),
            })
            .expect("a response serialises");
            golden.line(&frame);
        }
    }
    golden!("scene_systems", &golden.finish());
}

/// The JSON sizes of the scene's messages for the three systems (rendering plan R03, R03.T15;
/// Design note 4): a scene state and an arrival holding each system at each level, a
/// `BodySummaryDto` (mean and largest), and a push of ten craft, against the brainstorm's 250–300
/// bytes a body and 400 a craft. A measurement read by hand, so it reports and asserts nothing.
#[test]
#[ignore = "measurement, run by hand: cargo test -p hyperion-server --lib convert::scene_fixture::scene_message_sizes -- --ignored --nocapture"]
#[expect(
    clippy::too_many_lines,
    reason = "a measurement run by hand, read top to bottom as one procedure"
)]
fn scene_message_sizes() {
    use hyperion_protocol::{
        BodyGrantDto, FramePositionDto, KinematicsDto, NotificationBody, SceneArrivalDto,
        SceneClockDto, SceneClockStateDto, SceneCraftDto, SceneNotificationDto, SceneStateDto,
        SceneSystemDto, Subscribed, SubscriptionState,
    };

    let galaxy = Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let universe = UniverseIdHex::from_u64(SYSTEMS_SEED);
    let time = UniverseTime {
        seconds: 3_600,
        nanos: 123_456_789,
    };
    let clock = SceneClockDto {
        time,
        time_rate: 1_000,
        state: SceneClockStateDto::Running,
    };
    let frame = |message: &ServerMessage| {
        serde_json::to_string(message)
            .expect("a message serialises")
            .len()
    };
    let mut report = Vec::new();
    for raw in SYSTEMS {
        let system = SystemId::from_raw(raw).expect("a pinned ID is a system ID");
        let record = resolve(&galaxy, system).expect("a pinned ID names a system");
        let stars = SystemStars::generate(&galaxy, &record);
        let ctx = SystemContext::from_stars(&galaxy, &stars);
        let planets = generate(galaxy.seed(), &ctx);
        let ship = KinematicsDto {
            position: FramePositionDto::System {
                system: SystemIdHex::from_u64(raw),
                offset_m: [1.496e11, 2.0e9, -3.0e8],
            },
            velocity_m_s: [-1_234.5, 29_780.25, 12.75],
            time,
        };
        for detail in [
            DetailLevelDto::Contact,
            DetailLevelDto::MassAndOrbit,
            DetailLevelDto::Bulk,
            DetailLevelDto::Full,
        ] {
            let request = SystemBodiesRequest {
                universe: universe.clone(),
                system: SystemIdHex::from_u64(raw),
                time,
                detail,
            };
            let wanted = BodiesRequest::try_from(&request).expect("a time in the window");
            let hosts = system_summary(hosts_request(&request), &stars, wanted.time());
            let bodies = system_bodies(wanted, hosts, &ctx, &planets, galaxy.seed());
            let summaries: Vec<usize> = bodies
                .bodies
                .iter()
                .map(|body| {
                    serde_json::to_string(body)
                        .expect("a summary serialises")
                        .len()
                })
                .collect();
            let grants = bodies
                .bodies
                .iter()
                .map(|body| BodyGrantDto {
                    body: body.id.clone(),
                    level: detail,
                    seen: None,
                })
                .collect();
            let scene = SceneSystemDto {
                system: bodies,
                grants,
            };
            let state = frame(&ServerMessage::Response {
                id: RequestId(1),
                body: ResponseBody::Subscribe(Box::new(Subscribed {
                    subscription: 1,
                    state: SubscriptionState::Scene(SceneStateDto {
                        sequence: 0,
                        clock,
                        ship: ship.clone(),
                        system: Some(scene.clone()),
                        tidal_radius_m: Some(2.123_456_789e16),
                        craft: Vec::new(),
                    }),
                })),
            });
            let arrival = frame(&ServerMessage::Notification {
                subscription: 1,
                body: NotificationBody::Scene(SceneNotificationDto {
                    sequence: 12_345,
                    clock,
                    ship: Some(ship.clone()),
                    arrival: Some(SceneArrivalDto::System {
                        system: Box::new(scene),
                        tidal_radius_m: 2.123_456_789e16,
                    }),
                    bodies: Vec::new(),
                    craft: None,
                }),
            });
            let count = summaries.len();
            let mean = summaries.iter().sum::<usize>() / count.max(1);
            let largest = summaries.iter().copied().max().unwrap_or(0);
            report.push(format!(
                "{raw:#018x} {detail:?}: {count} bodies, state {state} B, arrival {arrival} B, \
                 summary mean {mean} B, largest {largest} B"
            ));
        }
    }
    let craft: Vec<SceneCraftDto> = (0..10_u8)
        .map(|k| SceneCraftDto {
            craft: format!("ISV-{k}"),
            hull: "corvette".to_owned(),
            state: KinematicsDto {
                position: FramePositionDto::System {
                    system: SystemIdHex::from_u64(SYSTEMS[0]),
                    offset_m: [1.496_123_456_7e11, 2.012_345_678e9, -3.098_765_432_1e8],
                },
                velocity_m_s: [-1_234.567_891, 29_780.123_456, 12.345_678_9],
                time,
            },
            attitude: [0.123_456_789, 0.234_567_891, 0.345_678_912, 0.901_234_567],
            angular_velocity_rad_s: [0.001_234_5, -0.002_345_6, 0.000_123_4],
            planned_path: None,
        })
        .collect();
    let craft_push = frame(&ServerMessage::Notification {
        subscription: 1,
        body: NotificationBody::Scene(SceneNotificationDto {
            sequence: 12_345,
            clock,
            ship: None,
            arrival: None,
            bodies: Vec::new(),
            craft: Some(craft.clone()),
        }),
    });
    let one_craft = serde_json::to_string(&craft[0])
        .expect("a craft serialises")
        .len();
    report.push(format!(
        "a push of ten craft {craft_push} B ({one_craft} B a craft); at 64 Hz {:.3} MB/s",
        f64::from(u32::try_from(craft_push).expect("a push is small")) * 64.0 / 1.0e6
    ));
    // A measurement's report, read by hand: there is no other channel for it.
    #[expect(
        clippy::print_stderr,
        reason = "a measurement run by hand reports its figures"
    )]
    {
        for line in report {
            eprintln!("{line}");
        }
    }
}
