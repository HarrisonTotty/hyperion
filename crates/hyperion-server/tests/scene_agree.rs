//! Two clients of one scene agree (rendering plan R03, R03.T9; the rendering brainstorm's
//! Testing): two subscriptions to one universe's scene, the second opened a second after the
//! first, are told the same clock, the same ship and the same system, for a minute of scene time at
//! 10× through a move of the stand-in out of its system and back.
//!
//! Each subscription's task reads the clock when it pushes, so their pushes are not the same
//! bytes; what must agree is what they describe. Every clock either client is told lies on the
//! one line the stand-in set, to within the delivery of a push; the ship each is told is the same;
//! and every system either is given, in its state or an arrival, is `system_bodies` answered for
//! that push's own time.

mod common;

use std::time::{Duration, Instant};

use common::scene::{SEED, TIME, in_system, in_the_halo, next_scene, subscribe, systems};
use common::{TestClient, TestServer};
use hyperion_protocol::{
    DetailLevelDto, KinematicsDto, RequestBody, ResponseBody, SceneArrivalDto, SceneClockDto,
    SceneShipRequest, SceneSystemDto, SystemBodiesRequest, SystemIdHex, UniverseIdHex,
    UniverseTime,
};

/// The rate the scene runs at.
const RATE: u32 = 10;

/// What a client was told, with when it heard it.
#[derive(Debug, Default)]
struct Heard {
    /// Each clock, with the instant the client received it and the number of ship changes heard
    /// before it, which says which setting it runs from.
    clocks: Vec<(Instant, usize, SceneClockDto)>,
    /// Each ship pushed, in order.
    ships: Vec<KinematicsDto>,
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
        if let Some(ship) = notification.ship {
            heard.ships.push(ship);
        }
        heard
            .clocks
            .push((Instant::now(), heard.ships.len(), notification.clock));
        match notification.arrival {
            Some(SceneArrivalDto::System { system, .. }) => {
                heard.systems.push((notification.clock.time, *system));
            }
            Some(SceneArrivalDto::NoSystem) => heard.departures += 1,
            None => {}
        }
    }
}

/// The ship stand-in as `client` sets it, at [`RATE`]; returns when the answer arrived.
async fn set_ship(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    ship: KinematicsDto,
) -> Instant {
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
    Instant::now()
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

#[tokio::test]
async fn two_clients_of_one_scene_are_told_the_same_scene() {
    let server = TestServer::start().await;
    let mut asker = server.connected().await;
    let universe = asker.create_universe("Scene", SEED).await.id;
    let [system, _] = systems();
    let set_at = set_ship(&mut asker, &universe, in_system(&system)).await;

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
    }

    // A minute of scene time at 10×, with the stand-in leaving the system and coming back.
    let end = Instant::now() + Duration::from_secs(6);
    let mut moved = in_system(&system);
    moved.velocity_m_s = [0.0, 3.0e4, 0.0];
    // Each setting states the scene time it starts from, so the clock restarts at TIME each time.
    let stand_in_moves = async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let left = set_ship(&mut asker, &universe, in_the_halo()).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        let back = set_ship(&mut asker, &universe, moved.clone()).await;
        (asker, [set_at, left, back])
    };
    let ((mut asker, settings), (), ()) = tokio::join!(
        stand_in_moves,
        listen(&mut first, &mut heard_first, end),
        listen(&mut second, &mut heard_second, end)
    );

    // The same ships, in the same order, and one departure each.
    assert_eq!(heard_first.ships, vec![in_the_halo(), moved.clone()]);
    assert_eq!(heard_second.ships, heard_first.ships);
    assert_eq!((heard_first.departures, heard_second.departures), (1, 1));

    // Every clock on one line per setting, to within a push's delivery: each clock's time less
    // the rate times the real time since it was set differs from the setting's by at most the
    // rate times the delivery and the processing of a push, here bounded generously by a second.
    let signed_nanos = |later: Instant, earlier: Instant| {
        if later >= earlier {
            i128::try_from(later.duration_since(earlier).as_nanos()).unwrap()
        } else {
            -i128::try_from(earlier.duration_since(later).as_nanos()).unwrap()
        }
    };
    let offsets = |heard: &Heard| {
        heard
            .clocks
            .iter()
            .map(|&(at, setting, clock)| {
                assert_eq!(clock.time_rate, RATE);
                let real = signed_nanos(at, settings[setting]);
                nanos(clock.time) - i128::from(RATE) * real - nanos(TIME)
            })
            .collect::<Vec<_>>()
    };
    let all: Vec<i128> = offsets(&heard_first)
        .into_iter()
        .chain(offsets(&heard_second))
        .collect();
    assert!(all.len() > 10, "{} clocks", all.len());
    let push = i128::from(RATE) * 1_000_000_000;
    let worst = all.iter().map(|offset| offset.abs()).max().unwrap();
    assert!(
        worst <= push,
        "a clock {worst} ns of scene time off its setting's line"
    );

    // Each system given is system_bodies answered for its push's own time.
    for heard in [&heard_first, &heard_second] {
        assert_eq!(
            heard.systems.len(),
            2,
            "the state's, and the return's arrival"
        );
        for (time, scene) in &heard.systems {
            assert_eq!(
                scene.system,
                system_bodies(&mut asker, &universe, &system, *time).await,
                "at {time:?}"
            );
        }
    }
}
