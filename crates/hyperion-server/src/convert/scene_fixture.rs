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
