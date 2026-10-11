//! Two clients of one scene agree (rendering plan R03, R03.T9; the rendering brainstorm's
//! Testing): two subscriptions to one universe's scene, the second opened a second or more after
//! the first, are told the same clock, the same ship, the same craft and the same system, for a
//! minute of scene time at 10× through a move of the stand-in out of its system and back.
//!
//! Each subscription's task reads the clock when it pushes, so their pushes are not the same
//! bytes; what must agree is what they describe. Every clock either client is told keeps to the
//! setting it was read under, with no tolerance: no earlier than the setting's time, no later than
//! the setting's line had reached when the client heard it, and never running back. The ship each
//! is told is the same, in the same order; every craft either is told is the craft the source
//! gives at the time the craft state; the contacts the server places are the same bodies for both;
//! and every system either is given, in its state or an arrival, is `system_bodies` answered for
//! the time its records state, which is that push's own time, or a time since the push before when
//! an arrival was merged with a later change before it was sent (Design note 5).
//!
//! Nothing here depends on how quickly the machine runs. The server's scene clock runs on the
//! runtime's monotonic clock, and pausing it cannot stand in for a loaded machine: a paused runtime
//! advances to its next timer whenever it waits on the pool's threads or a socket, so the server's
//! tick and the harness's patience would run out in no time. So the minute is paced by the clock
//! the clients hear, each stage waiting for both to be told its setting and its scene time, and
//! each clock is held between bounds that causality gives: the server anchors a setting's clock
//! after the request is sent, and reads a clock before the client hears it. How long a push waited
//! before it was heard has no causal bound, so a push's clock is bounded below only by its
//! setting's time; a state's, read while a request is answered, is bounded on both sides.

mod common;

use std::collections::BTreeSet;

use common::scene::{
    SEED, TIME, TenCraft, in_system, in_the_halo, next_scene, subscribe, systems, ten_craft_record,
};
use common::{TestClient, TestServer, patiently};
use hyperion_protocol::{
    BodyIdHex, DetailLevelDto, KinematicsDto, RequestBody, ResponseBody, SceneArrivalDto,
    SceneClockDto, SceneCraftDto, SceneNotificationDto, SceneShipRequest, SceneStateDto,
    SceneSystemDto, SystemBodiesRequest, SystemIdHex, UniverseIdHex, UniverseTime,
};
use hyperion_server::scene::{CraftState, SceneKnowledge};
use hyperion_sim::id::BodyId;
use hyperion_sim::planetary::record::DetailLevel;
// The clock the server's scene runs on, so that the test's instants and the server's are one
// clock's.
use tokio::time::Instant;

/// The rate the scene runs at.
const RATE: u32 = 10;

/// Nanoseconds in a second.
const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// The scene time each client hears of each stage of the minute, in nanoseconds: 20 s, two seconds
/// of the server's clock at [`RATE`], and so at least one heartbeat.
const STAGE_NS: i128 = 20 * NANOS_PER_SECOND;

/// Every craft is a contact, and so are the bodies of odd slots, whose positions the server places.
#[derive(Debug)]
struct Contacts;

impl SceneKnowledge for Contacts {
    fn grant(&self, body: BodyId, asked: DetailLevel) -> DetailLevel {
        if (body.body_index() >> 8) % 2 == 1 {
            DetailLevel::Contact
        } else {
            asked
        }
    }

    fn is_contact(&self, _craft: &CraftState) -> bool {
        true
    }
}

/// A setting the asker made, with the instants that bracket the server's taking it: the server
/// anchors the clock at the ship's time when it accepts the request, after `sent` and before
/// `answered`.
#[derive(Debug)]
struct Setting {
    ship: KinematicsDto,
    sent: Instant,
    answered: Instant,
}

/// A clock a client was told.
#[derive(Debug)]
struct Told {
    /// When the client heard it.
    at: Instant,
    /// The index of the setting it was read under: the first for the state, then one more for each
    /// ship pushed, since a setting always pushes its ship and the push that takes it is the first
    /// read under it.
    setting: usize,
    clock: SceneClockDto,
}

/// What a client was told.
#[derive(Debug)]
struct Heard {
    /// Which client it is, for the failures.
    who: &'static str,
    /// When the client sent its subscription.
    opened: Instant,
    /// Each clock, the state's first.
    clocks: Vec<Told>,
    /// Each ship pushed, in order.
    ships: Vec<KinematicsDto>,
    /// Each craft list, the state's first.
    craft: Vec<Vec<SceneCraftDto>>,
    /// The bodies placed by a seen position.
    seen: BTreeSet<BodyIdHex>,
    /// Each system given, the state's first, with the clocks of the push that gave it and of the
    /// one before.
    systems: Vec<Given>,
    /// Each departure.
    departures: usize,
    /// The clock of the latest state or push, the time from which the next push's changes date.
    latest: UniverseTime,
}

/// A system given in a state or an arrival.
#[derive(Debug)]
struct Given {
    /// When the changes the push holds began: the clock of the state or push before it, or the
    /// time a setting it carries restarted the clock at. Its records are evaluated no earlier.
    since: UniverseTime,
    /// The clock of the state or push that gave it.
    clock: UniverseTime,
    system: SceneSystemDto,
}

impl Heard {
    /// Takes a notification the client was pushed, as it hears it.
    fn hear(&mut self, notification: SceneNotificationDto) {
        let at = Instant::now();
        let previous = std::mem::replace(&mut self.latest, notification.clock.time);
        // A setting, which always pushes its ship, restarts the clock at the ship's time, which may
        // lie before the previous push's clock; without one the clock runs on from that push.
        let since = notification
            .ship
            .as_ref()
            .map_or(previous, |ship| ship.time);
        self.ships.extend(notification.ship);
        self.clocks.push(Told {
            at,
            setting: self.ships.len(),
            clock: notification.clock,
        });
        self.craft.extend(notification.craft);
        self.seen.extend(
            notification
                .bodies
                .iter()
                .filter(|body| body.seen.is_some())
                .map(|body| body.record.id.clone()),
        );
        match notification.arrival {
            Some(SceneArrivalDto::System { system, .. }) => {
                self.systems.push(Given {
                    since,
                    clock: notification.clock.time,
                    system: *system,
                });
            }
            Some(SceneArrivalDto::NoSystem) => self.departures += 1,
            None => {}
        }
    }
}

/// Nanoseconds from the epoch.
fn nanos(time: UniverseTime) -> i128 {
    i128::from(time.seconds) * NANOS_PER_SECOND + i128::from(time.nanos)
}

/// Nanoseconds of real time from `from` to `to`, 0 if `to` is earlier.
fn real_nanos(from: Instant, to: Instant) -> i128 {
    i128::try_from(to.saturating_duration_since(from).as_nanos()).unwrap()
}

/// Sets the ship stand-in to `ship` at [`RATE`], noting when the request went and when it was
/// answered.
async fn set_ship(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    ship: KinematicsDto,
) -> Setting {
    let sent = Instant::now();
    let (answer, _) = client
        .request_among_notifications(RequestBody::SceneShip(SceneShipRequest {
            universe: universe.clone(),
            ship: ship.clone(),
            time_rate: RATE,
        }))
        .await;
    let answered = Instant::now();
    assert!(
        matches!(answer, Ok(ResponseBody::SceneShip(_))),
        "{answer:?}"
    );
    Setting {
        ship,
        sent,
        answered,
    }
}

/// Subscribes `client`, called `who`, to the universe's scene at the level the scenes ask: what it
/// is told, the state so far, and the ship the state holds.
async fn open(
    client: &mut TestClient,
    who: &'static str,
    universe: &UniverseIdHex,
) -> (Heard, KinematicsDto) {
    let opened = Instant::now();
    let (_, state) = subscribe(client, universe, DetailLevelDto::Bulk).await;
    let at = Instant::now();
    let SceneStateDto {
        clock,
        ship,
        system,
        craft,
        ..
    } = state;
    let heard = Heard {
        who,
        opened,
        clocks: vec![Told {
            at,
            setting: 0,
            clock,
        }],
        ships: Vec::new(),
        craft: vec![craft],
        seen: BTreeSet::new(),
        systems: vec![Given {
            since: clock.time,
            clock: clock.time,
            system: system.expect("the stand-in is in the system"),
        }],
        departures: 0,
        latest: clock.time,
    };
    (heard, ship)
}

/// Whether `heard` has been told the latest of `settings` and has heard a stage of it: its ship,
/// then the departure for the move out and the return's arrival for the move back, and a clock
/// [`STAGE_NS`] past the first it heard under the setting, so that each client hears a whole stage
/// of its own, the second's first included.
fn has_run_its_stage(heard: &Heard, settings: &[Setting]) -> bool {
    let stage = settings.len() - 1;
    // At least, so that a client told more than it should be fails the assertions after the minute
    // rather than waiting out the patience.
    let arrived = match stage {
        0 => true,
        1 => heard.departures >= 1,
        _ => heard.systems.len() >= 2,
    };
    // The first clock under the setting is heard once its ship is.
    let began = heard.clocks.iter().find(|told| told.setting == stage);
    arrived && began.is_some_and(|began| nanos(heard.latest) >= nanos(began.clock.time) + STAGE_NS)
}

/// Checks that every clock `heard` was told keeps to the setting it was read under, with no
/// tolerance.
///
/// The server anchors a setting's clock at the ship's time when it accepts the request, after the
/// request was sent, and reads a clock before the client hears it, so a clock is no earlier than
/// its setting's time and no later than that time plus the rate times the real time from sending
/// the setting to hearing the clock. A setting's clock runs forward, so each clock is no earlier
/// than the one heard before it under the same setting. The state's clock is read while the
/// subscription is answered, after it was sent, on a clock anchored before the setting was
/// answered, which bounds it from below as well: for the second client, which subscribed a second
/// or more after that answer, [`RATE`] seconds of scene time past the first setting's time, which a
/// clock anchored at its own subscription would not reach.
fn assert_clocks_keep_to_their_settings(heard: &Heard, settings: &[Setting]) {
    let who = heard.who;
    let rate = i128::from(RATE);
    for (n, told) in heard.clocks.iter().enumerate() {
        let setting = told.setting;
        assert_eq!(told.clock.time_rate, RATE, "{who}'s clock {n}");
        let start_ns = nanos(settings[setting].ship.time);
        let time_ns = nanos(told.clock.time);
        let real_ns = real_nanos(settings[setting].sent, told.at);
        let reachable_ns = start_ns + rate * real_ns;
        assert!(
            start_ns <= time_ns && time_ns <= reachable_ns,
            "{who}'s clock {n} at {time_ns} ns, outside setting {setting}'s line from {start_ns} \
             ns, which reached {reachable_ns} ns in the {real_ns} ns from sending the setting to \
             hearing the clock"
        );
    }
    for (n, pair) in heard.clocks.windows(2).enumerate() {
        if pair[0].setting == pair[1].setting {
            assert!(
                pair[0].clock.time <= pair[1].clock.time,
                "{who}'s clock {} ran back under setting {} from {:?} to {:?}",
                n + 1,
                pair[0].setting,
                pair[0].clock.time,
                pair[1].clock.time
            );
        }
    }
    let state = &heard.clocks[0];
    let real_ns = real_nanos(settings[0].answered, heard.opened);
    let earliest_ns = nanos(settings[0].ship.time) + rate * real_ns;
    assert!(
        nanos(state.clock.time) >= earliest_ns,
        "{who}'s state's clock {:?} before {earliest_ns} ns, where setting 0's line stood when the \
         subscription was sent {real_ns} ns after the setting's answer",
        state.clock.time
    );
}

/// `system_bodies` of `system` at `time`, asked by a third client, at the level the scenes ask.
async fn system_bodies(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    system: &SystemIdHex,
    time: UniverseTime,
) -> hyperion_protocol::SystemBodiesDto {
    match client
        .request(RequestBody::SystemBodies(SystemBodiesRequest {
            universe: universe.clone(),
            system: system.clone(),
            time,
            detail: DetailLevelDto::Bulk,
        }))
        .await
    {
        Ok(ResponseBody::SystemBodies(bodies)) => *bodies,
        other => panic!("system_bodies answers: {other:?}"),
    }
}

/// Checks that every craft list `heard` holds is the source's at the time it states.
fn assert_craft_are_the_source_s(heard: &Heard, system: &SystemIdHex) {
    assert!(!heard.craft.is_empty());
    for list in &heard.craft {
        let t = list[0].state.time;
        let at = hyperion_sim::time::UniverseTime::new(t.seconds, t.nanos).unwrap();
        let expected: Vec<SceneCraftDto> =
            (0..10).map(|k| ten_craft_record(system, k, at)).collect();
        assert_eq!(list, &expected);
    }
}

/// Checks that each system `heard` was given is `system_bodies` answered for the time its records
/// state. That is its push's own time, or, for an arrival merged with a later change before the
/// connection took it (Design note 5: the merged push states the latest clock), a time since the
/// push before.
async fn assert_systems_are_system_bodies(
    heard: &Heard,
    asker: &mut TestClient,
    universe: &UniverseIdHex,
    system: &SystemIdHex,
) {
    assert_eq!(
        heard.systems.len(),
        2,
        "the state's, and the return's arrival"
    );
    for Given {
        since,
        clock,
        system: scene,
    } in &heard.systems
    {
        let time = scene.system.hosts.time;
        assert!(
            nanos(*since) <= nanos(time) && nanos(time) <= nanos(*clock),
            "records at {time:?}, outside the push's changes from {since:?} to {clock:?}"
        );
        let expected = system_bodies(asker, universe, system, time).await;
        // Each body's record is degraded to its own grant; its place and the rest are the
        // system's.
        assert_eq!(scene.system.hosts, expected.hosts, "at {time:?}");
        assert_eq!(scene.system.zones, expected.zones, "at {time:?}");
        for record in &scene.system.bodies {
            let full = expected
                .bodies
                .iter()
                .find(|full| full.id == record.id)
                .expect("every body the scene lists, system_bodies lists");
            assert_eq!(
                record.position_m, full.position_m,
                "{:?} at {time:?}",
                record.id
            );
        }
    }
}

#[tokio::test]
async fn two_clients_of_one_scene_are_told_the_same_scene() {
    let [system, _] = systems();
    let data_dir = tempfile::tempdir().unwrap();
    let server = TestServer::start_with(
        TestServer::config(data_dir.path())
            .scene_knowledge(Contacts)
            .craft_source(TenCraft(system.clone()))
            .build(),
    )
    .await;
    let mut asker = server.connected().await;
    let universe = asker.create_universe("Scene", SEED).await.id;
    let mut settings = vec![set_ship(&mut asker, &universe, in_system(&system)).await];

    // The second subscription opens once the first has heard a clock that the server read a second
    // or more after both the first setting's answer and the first client's state. The setting's
    // line reaches the first of those instants no sooner than the time the request took, and a
    // second, after the request was sent, and the second a second after the state's reading.
    let mut first = server.connected().await;
    let (mut heard_first, first_ship) = open(&mut first, "the first client", &universe).await;
    let a_second_ns = i128::from(RATE) * NANOS_PER_SECOND;
    let after_the_answer_ns = nanos(TIME)
        + i128::from(RATE) * real_nanos(settings[0].sent, settings[0].answered)
        + a_second_ns;
    let second_opens_ns = after_the_answer_ns.max(nanos(heard_first.latest) + a_second_ns);
    patiently("the first client to hear a second of the scene", async {
        while nanos(heard_first.latest) < second_opens_ns {
            heard_first.hear(next_scene(&mut first).await.1);
        }
    })
    .await;
    let mut second = server.connected().await;
    let (mut heard_second, second_ship) = open(&mut second, "the second client", &universe).await;
    assert_eq!(first_ship, in_system(&system));
    assert_eq!(second_ship, in_system(&system));

    // A minute of scene time at 10×, in three stages: in the system, out of it in the halo, and
    // back. Each stage runs until both clients have been told its setting and 20 s of scene time
    // on it from the first clock each heard under it, and the next setting continues the clock from
    // the latest either has heard.
    let mut back = in_system(&system);
    back.velocity_m_s = [0.0, 3.0e4, 0.0];
    let mut moves = [in_the_halo(), back].into_iter();
    patiently("both clients to hear the minute", async {
        loop {
            if has_run_its_stage(&heard_first, &settings)
                && has_run_its_stage(&heard_second, &settings)
            {
                let Some(mut ship) = moves.next() else { break };
                ship.time = heard_first.latest.max(heard_second.latest);
                settings.push(set_ship(&mut asker, &universe, ship).await);
            }
            // Reading a frame is cancel-safe, so the client not read keeps its next frame.
            tokio::select! {
                (_, notification) = next_scene(&mut first) => heard_first.hear(notification),
                (_, notification) = next_scene(&mut second) => heard_second.hear(notification),
            }
        }
    })
    .await;

    // The same ships, in the same order, and one departure each.
    let set: Vec<KinematicsDto> = settings[1..]
        .iter()
        .map(|setting| setting.ship.clone())
        .collect();
    assert_eq!(heard_first.ships, set);
    assert_eq!(heard_second.ships, heard_first.ships);
    assert_eq!((heard_first.departures, heard_second.departures), (1, 1));

    assert_clocks_keep_to_their_settings(&heard_first, &settings);
    assert_clocks_keep_to_their_settings(&heard_second, &settings);

    // The craft each is told are the source's at the time they state, and the same contacts are
    // placed by sight for both.
    assert_craft_are_the_source_s(&heard_first, &system);
    assert_craft_are_the_source_s(&heard_second, &system);
    assert!(!heard_first.seen.is_empty());
    assert_eq!(heard_first.seen, heard_second.seen);

    assert_systems_are_system_bodies(&heard_first, &mut asker, &universe, &system).await;
    assert_systems_are_system_bodies(&heard_second, &mut asker, &universe, &system).await;
}
