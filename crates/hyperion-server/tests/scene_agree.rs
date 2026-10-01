//! Two clients of one scene agree (rendering plan R03, R03.T9; the rendering brainstorm's
//! Testing): two subscriptions to one universe's scene, the second opened a second after the
//! first, are told the same clock, the same ship, the same craft and the same system, for a minute
//! of scene time at 10× through a move of the stand-in out of its system and back.
//!
//! Each subscription's task reads the clock when it pushes, so their pushes are not the same
//! bytes; what must agree is what they describe. Every clock either client is told lies on the one
//! line the stand-in's settings draw, to within the delivery of a push; the ship each is told is
//! the same, in the same order; every craft either is told is the craft the source gives at the
//! time the craft state; the contacts the server places are the same bodies for both; and every
//! system either is given, in its state or an arrival, is `system_bodies` answered for that push's
//! own time.

mod common;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use common::scene::{
    SEED, TIME, TenCraft, in_system, in_the_halo, next_scene, subscribe, systems, ten_craft_record,
};
use common::{TestClient, TestServer};
use hyperion_protocol::{
    BodyIdHex, DetailLevelDto, KinematicsDto, RequestBody, ResponseBody, SceneArrivalDto,
    SceneClockDto, SceneCraftDto, SceneShipRequest, SceneSystemDto, SystemBodiesRequest,
    SystemIdHex, UniverseIdHex, UniverseTime,
};
use hyperion_server::scene::{CraftState, SceneKnowledge};
use hyperion_sim::id::BodyId;
use hyperion_sim::planetary::record::DetailLevel;

/// The rate the scene runs at.
const RATE: u32 = 10;

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

/// What a client was told, with when it heard it.
#[derive(Debug, Default)]
struct Heard {
    /// Each clock, with the instant the client received it.
    clocks: Vec<(Instant, SceneClockDto)>,
    /// Each ship pushed, in order.
    ships: Vec<KinematicsDto>,
    /// Each craft list pushed.
    craft: Vec<Vec<SceneCraftDto>>,
    /// The bodies placed by a seen position.
    seen: BTreeSet<BodyIdHex>,
    /// Each system given, with the time of the push that gave it.
    systems: Vec<(UniverseTime, SceneSystemDto)>,
    /// Each departure.
    departures: usize,
}

/// Nanoseconds from the epoch.
fn nanos(time: UniverseTime) -> i128 {
    i128::from(time.seconds) * 1_000_000_000 + i128::from(time.nanos)
}

/// Reads what `client` is pushed until `deadline`.
async fn listen(client: &mut TestClient, heard: &mut Heard, deadline: Instant) {
    while Instant::now() < deadline {
        let (_, notification) = next_scene(client).await;
        heard.clocks.push((Instant::now(), notification.clock));
        heard.ships.extend(notification.ship);
        heard.craft.extend(notification.craft);
        heard.seen.extend(
            notification
                .bodies
                .iter()
                .filter(|body| body.seen.is_some())
                .map(|body| body.record.id.clone()),
        );
        match notification.arrival {
            Some(SceneArrivalDto::System { system, .. }) => {
                heard.systems.push((notification.clock.time, *system));
            }
            Some(SceneArrivalDto::NoSystem) => heard.departures += 1,
            None => {}
        }
    }
}

/// The ship stand-in as `client` sets it, at [`RATE`].
async fn set_ship(client: &mut TestClient, universe: &UniverseIdHex, ship: KinematicsDto) {
    let (answer, _) = client
        .request_among_notifications(RequestBody::SceneShip(SceneShipRequest {
            universe: universe.clone(),
            ship,
            time_rate: RATE,
        }))
        .await;
    assert!(
        matches!(answer, Ok(ResponseBody::SceneShip(_))),
        "{answer:?}"
    );
}

/// The scene time on the line from [`TIME`] at `set_at` at [`RATE`], at `now`: where a later
/// setting continues the clock, so that the minute is one run of scene time.
fn continued(set_at: Instant, now: Instant) -> UniverseTime {
    let scene = TIME.seconds * 1_000_000_000
        + i64::try_from(now.duration_since(set_at).as_nanos()).unwrap() * i64::from(RATE);
    UniverseTime {
        seconds: scene.div_euclid(1_000_000_000),
        nanos: u32::try_from(scene.rem_euclid(1_000_000_000)).unwrap(),
    }
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
    set_ship(&mut asker, &universe, in_system(&system)).await;
    let set_at = Instant::now();

    let mut first = server.connected().await;
    let (_, first_state) = subscribe(&mut first, &universe, DetailLevelDto::Bulk).await;
    let mut heard_first = Heard::default();
    listen(
        &mut first,
        &mut heard_first,
        Instant::now() + Duration::from_secs(1),
    )
    .await;
    let mut second = server.connected().await;
    let (_, second_state) = subscribe(&mut second, &universe, DetailLevelDto::Bulk).await;
    let mut heard_second = Heard::default();
    for (state, heard) in [
        (&first_state, &mut heard_first),
        (&second_state, &mut heard_second),
    ] {
        assert_eq!(state.ship, in_system(&system));
        let scene = state.system.clone().expect("the stand-in is in the system");
        heard.systems.push((state.clock.time, scene));
        heard.craft.push(state.craft.clone());
    }

    // A minute of scene time at 10×, the stand-in leaving the system and coming back; each
    // setting continues the clock where it stands.
    let end = Instant::now() + Duration::from_secs(6);
    let mut left = in_the_halo();
    let mut back = in_system(&system);
    back.velocity_m_s = [0.0, 3.0e4, 0.0];
    let stand_in_moves = async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        left.time = continued(set_at, Instant::now());
        set_ship(&mut asker, &universe, left.clone()).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        back.time = continued(set_at, Instant::now());
        set_ship(&mut asker, &universe, back.clone()).await;
        (asker, left, back)
    };
    let ((mut asker, left, back), (), ()) = tokio::join!(
        stand_in_moves,
        listen(&mut first, &mut heard_first, end),
        listen(&mut second, &mut heard_second, end)
    );

    // The same ships, in the same order, and one departure each.
    assert_eq!(heard_first.ships, vec![left, back]);
    assert_eq!(heard_second.ships, heard_first.ships);
    assert_eq!((heard_first.departures, heard_second.departures), (1, 1));

    // Every clock on the settings' one line, to within a push's delivery: a clock's time less the
    // rate times the real time since the first setting was answered is the first setting's time,
    // to within the rate times a generous second of delivery and processing.
    let offsets = heard_first
        .clocks
        .iter()
        .chain(&heard_second.clocks)
        .map(|&(at, clock)| {
            assert_eq!(clock.time_rate, RATE);
            let real = i128::try_from(at.duration_since(set_at).as_nanos()).unwrap();
            nanos(clock.time) - i128::from(RATE) * real - nanos(TIME)
        })
        .collect::<Vec<_>>();
    assert!(offsets.len() > 10, "{} clocks", offsets.len());
    let worst = offsets.iter().map(|offset| offset.abs()).max().unwrap();
    assert!(
        worst <= i128::from(RATE) * 1_000_000_000,
        "a clock {worst} ns of scene time off the line"
    );

    // The craft each is told are the source's at the time they state, and the same contacts are
    // placed by sight for both.
    assert_craft_are_the_source_s(&heard_first, &system);
    assert_craft_are_the_source_s(&heard_second, &system);
    assert!(!heard_first.seen.is_empty());
    assert_eq!(heard_first.seen, heard_second.seen);

    // Each system given is system_bodies answered for its push's own time.
    for heard in [&heard_first, &heard_second] {
        assert_eq!(
            heard.systems.len(),
            2,
            "the state's, and the return's arrival"
        );
        for (time, scene) in &heard.systems {
            let expected = system_bodies(&mut asker, &universe, &system, *time).await;
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
}
