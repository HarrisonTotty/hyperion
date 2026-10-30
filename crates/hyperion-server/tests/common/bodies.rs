//! The planetary bodies' helpers, shared by the integration tests of `body_events` (plan 14,
//! P14.T36.b): the seed and the pinned systems of the bodies' tests, the sim's systems the answers
//! are held to, the times on the wire and in the sim, and a client with a universe of the seed.

use std::sync::OnceLock;

use hyperion_protocol::{RequestBody, RequestError, UniverseIdHex, UniverseTime};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::{PlanetarySystem, SystemContext, generate};
use hyperion_sim::{Seed, time};

use super::{TestClient, TestServer};

/// The seed of every universe the bodies' tests create, the other integration tests' own.
pub const SEED: u64 = 0x4d2;

/// Seconds in a Julian year, the sim's year: the wire carries a time as whole seconds.
pub const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

/// Pinned systems of layer C at the solar circle, `system_bodies.rs`'s: a single star with a rocky
/// planet and a gas giant; a K1 V and M1 V pair with eight planets about three zones; and a
/// white-dwarf triple with fifteen planets, one of them engulfed 250 years before the epoch.
pub const PINNED: [u64; 3] = [
    0x4200_2cb2_0000_0003,
    0x4200_2cb2_0000_0000,
    0x4200_acb2_0000_000d,
];

/// The galaxy the server builds for [`SEED`], to hold its answers to.
pub fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
}

/// The sim's context and planetary system of the system `raw` names.
pub fn sim_system(raw: u64) -> (SystemContext, PlanetarySystem) {
    let id = SystemId::from_raw(raw).expect("a pinned ID is a system ID");
    let ctx = SystemContext::for_system(galaxy(), id).expect("a pinned ID names a system");
    let planets = generate(Seed::new(SEED), &ctx);
    (ctx, planets)
}

/// The instant `years` Julian years from the epoch, on the wire.
pub fn at_years(years: i64) -> UniverseTime {
    UniverseTime {
        seconds: years * SECONDS_PER_JULIAN_YEAR,
        nanos: 0,
    }
}

/// The wire's `time` in the sim.
pub fn sim_time(time: UniverseTime) -> time::UniverseTime {
    time::UniverseTime::new(time.seconds, time.nanos).expect("a valid time")
}

/// A client of `server` that has said hello, and a universe of [`SEED`] of its own, named `name`.
pub async fn joined(server: &TestServer, name: &str) -> (TestClient, UniverseIdHex) {
    let mut client = server.connected().await;
    let universe = client.create_universe(name, SEED).await.id;
    (client, universe)
}

/// The refusal of `body`.
///
/// # Panics
///
/// If `body` is answered.
pub async fn refused(client: &mut TestClient, body: RequestBody) -> RequestError {
    match client.request(body).await {
        Err(error) => error,
        Ok(body) => panic!("expected a refusal, got {body:?}"),
    }
}
