//! The scene subscription, live, over a real socket (rendering plan R03, R03.T8.a): subscribe in a
//! pinned system and receive its bodies as `system_bodies` answers them, a heartbeat each second,
//! the ship stand-in's changes pushed to every subscription of the universe, a departure from the
//! system, and the refusals of an unknown subscription and of a camera out of reach.

mod common;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    CameraReportDto, DetailLevelDto, ErrorCode, FramePositionDto, KinematicsDto, NotificationBody,
    RequestBody, ResponseBody, SceneArrivalDto, SceneCamerasRequest, SceneClockStateDto,
    SceneNotificationDto, SceneShipRequest, SceneStateDto, SceneSubscribeRequest, SubscribeRequest,
    SubscriptionState, SubscriptionTopic, SystemBodiesRequest, SystemIdHex, UniverseIdHex,
    UniverseTime,
};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
use hyperion_sim::id::Layer;

/// The seed of the universe these scenes are in.
const SEED: u64 = 0x4d2;

/// The scene time the tests set, an hour after the epoch.
const TIME: UniverseTime = UniverseTime {
    seconds: 3_600,
    nanos: 0,
};

/// The first two systems of the layer-C cell at the solar circle, as the server generates them.
fn systems() -> [SystemIdHex; 2] {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let mut cell = Vec::new();
    generate_cell(
        &galaxy,
        CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid"),
        &mut cell,
    );
    [0, 1].map(|n| SystemIdHex::from_u64(cell[n].id().raw()))
}

/// A pose 1 au from `system`'s barycentre, in its frame, at rest, at [`TIME`].
fn in_system(system: &SystemIdHex) -> KinematicsDto {
    KinematicsDto {
        position: FramePositionDto::System {
            system: system.clone(),
            offset_m: [1.496e11, 0.0, 0.0],
        },
        velocity_m_s: [0.0; 3],
        time: TIME,
    }
}

/// A pose far above the disc, in no system's sphere.
fn in_the_halo() -> KinematicsDto {
    KinematicsDto {
        position: FramePositionDto::Galactic {
            position: hyperion_protocol::GalacticPosition {
                cell_ly: [0, 0, 60_000],
                offset_m: [0.0; 3],
            },
        },
        velocity_m_s: [0.0; 3],
        time: TIME,
    }
}

/// A server, a client said hello, and a universe of [`SEED`].
async fn universe() -> (TestServer, TestClient, UniverseIdHex) {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Scene", SEED).await.id;
    (server, client, universe)
}

/// Sets the universe's ship stand-in and clock, keeping the notifications that arrive meanwhile.
async fn set_ship(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    ship: KinematicsDto,
    time_rate: u32,
) -> Vec<(u32, NotificationBody)> {
    let (answer, notifications) = client
        .request_among_notifications(RequestBody::SceneShip(SceneShipRequest {
            universe: universe.clone(),
            ship,
            time_rate,
        }))
        .await;
    assert!(
        matches!(answer, Ok(ResponseBody::SceneShip(_))),
        "{answer:?}"
    );
    notifications
}

/// Subscribes to the universe's scene at `detail` and returns the subscription and its state.
async fn subscribe(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    detail: DetailLevelDto,
) -> (u32, SceneStateDto) {
    let (answer, _) = client
        .request_among_notifications(RequestBody::Subscribe(SubscribeRequest {
            universe: universe.clone(),
            topic: SubscriptionTopic::Scene(SceneSubscribeRequest {
                detail,
                cameras: Vec::new(),
            }),
        }))
        .await;
    match answer {
        Ok(ResponseBody::Subscribe(subscribed)) => match subscribed.state {
            SubscriptionState::Scene(state) => (subscribed.subscription, state),
        },
        other => panic!("expected the scene's state, got {other:?}"),
    }
}

/// The next scene notification on the client.
async fn next_scene(client: &mut TestClient) -> (u32, SceneNotificationDto) {
    match client.next_notification().await {
        (subscription, NotificationBody::Scene(notification)) => (subscription, notification),
    }
}

#[tokio::test]
async fn a_scene_in_a_pinned_system_holds_every_body_as_system_bodies_answers_them() {
    let (_server, mut client, universe) = universe().await;
    let [system, _] = systems();
    set_ship(&mut client, &universe, in_system(&system), 0).await;
    let (_, state) = subscribe(&mut client, &universe, DetailLevelDto::Full).await;
    let scene = state.system.expect("the ship is in the system's frame");
    let (answer, _) = client
        .request_among_notifications(RequestBody::SystemBodies(SystemBodiesRequest {
            universe: universe.clone(),
            system: system.clone(),
            time: TIME,
            detail: DetailLevelDto::Full,
        }))
        .await;
    let Ok(ResponseBody::SystemBodies(expected)) = answer else {
        panic!("system_bodies answers: {answer:?}");
    };
    assert_eq!(scene.system, *expected);
    assert_eq!(scene.grants.len(), scene.system.bodies.len());
    assert!(
        scene
            .grants
            .iter()
            .all(|grant| grant.level == DetailLevelDto::Full && grant.seen.is_none())
    );
    assert!(state.tidal_radius_m.is_some_and(|radius| radius > 1.0e15));
    assert_eq!(state.clock.time, TIME);
    assert_eq!(state.clock.state, SceneClockStateDto::Paused);
    assert_eq!(state.sequence, 0);
}

#[tokio::test]
async fn a_heartbeat_comes_each_second_its_sequence_rising_by_one() {
    let (_server, mut client, universe) = universe().await;
    let [system, _] = systems();
    set_ship(&mut client, &universe, in_system(&system), 0).await;
    let (subscription, _) = subscribe(&mut client, &universe, DetailLevelDto::Bulk).await;
    let started = std::time::Instant::now();
    for sequence in 1..=3 {
        let (on, beat) = next_scene(&mut client).await;
        assert_eq!(on, subscription);
        assert_eq!(beat.sequence, sequence);
        assert_eq!(
            (beat.clock.time, beat.clock.time_rate),
            (TIME, 0),
            "a paused clock"
        );
        assert!(beat.bodies.is_empty() && beat.arrival.is_none() && beat.ship.is_none());
    }
    // Three heartbeats a second apart; generous above, for a loaded machine.
    let elapsed = started.elapsed().as_secs_f64();
    assert!((2.5..30.0).contains(&elapsed), "{elapsed} s");
}

#[tokio::test]
async fn a_new_ship_and_rate_are_pushed_at_once_to_every_subscription_of_the_universe() {
    let (server, mut first, universe) = universe().await;
    let [system, _] = systems();
    set_ship(&mut first, &universe, in_system(&system), 0).await;
    let mut second = server.connected().await;
    let (one, _) = subscribe(&mut first, &universe, DetailLevelDto::Bulk).await;
    let (two, _) = subscribe(&mut second, &universe, DetailLevelDto::Bulk).await;

    let mut moved = in_system(&system);
    moved.velocity_m_s = [0.0, 3.0e4, 0.0];
    let before = set_ship(&mut first, &universe, moved.clone(), 1_000).await;
    for (client, subscription, mut seen) in
        [(&mut first, one, before), (&mut second, two, Vec::new())]
    {
        // The change comes before the next heartbeat could carry the new rate on its own.
        let change = loop {
            let (on, notification) = match seen.pop() {
                Some((on, NotificationBody::Scene(notification))) => (on, notification),
                None => next_scene(client).await,
            };
            assert_eq!(on, subscription);
            if notification.clock.time_rate == 1_000 {
                break notification;
            }
        };
        assert_eq!(
            change.ship.as_ref(),
            Some(&moved),
            "the ship rides with the change"
        );
        assert_eq!(change.clock.state, SceneClockStateDto::Running);
    }
}

#[tokio::test]
async fn moving_the_stand_in_out_of_the_system_pushes_no_system() {
    let (_server, mut client, universe) = universe().await;
    let [system, _] = systems();
    set_ship(&mut client, &universe, in_system(&system), 0).await;
    let (_, state) = subscribe(&mut client, &universe, DetailLevelDto::Bulk).await;
    assert!(state.system.is_some());
    let mut seen = set_ship(&mut client, &universe, in_the_halo(), 0).await;
    let departure = loop {
        let notification = match seen.pop() {
            Some((_, NotificationBody::Scene(notification))) => notification,
            None => next_scene(&mut client).await.1,
        };
        if notification.arrival.is_some() {
            break notification;
        }
    };
    assert_eq!(departure.arrival, Some(SceneArrivalDto::NoSystem));
    assert_eq!(departure.ship, Some(in_the_halo()));
}

#[tokio::test]
async fn an_unknown_subscription_and_a_camera_out_of_reach_are_refused_naming_their_fields() {
    let (_server, mut client, universe) = universe().await;
    let [system, other] = systems();
    set_ship(&mut client, &universe, in_system(&system), 0).await;
    let (subscription, _) = subscribe(&mut client, &universe, DetailLevelDto::Bulk).await;
    let cameras = |position| {
        vec![CameraReportDto {
            view: 0,
            pose: KinematicsDto {
                position,
                velocity_m_s: [0.0; 3],
                time: TIME,
            },
        }]
    };
    let report = |subscription, position| {
        RequestBody::SceneCameras(SceneCamerasRequest {
            subscription,
            cameras: cameras(position),
        })
    };
    let near = FramePositionDto::System {
        system: system.clone(),
        offset_m: [3.0e11, 0.0, 0.0],
    };
    let (answer, _) = client
        .request_among_notifications(report(subscription + 7, near.clone()))
        .await;
    let error = answer.unwrap_err();
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("subscription"))
    );
    let elsewhere = FramePositionDto::System {
        system: other,
        offset_m: [0.0; 3],
    };
    let (answer, _) = client
        .request_among_notifications(report(subscription, elsewhere))
        .await;
    let error = answer.unwrap_err();
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("cameras"))
    );
    let (answer, _) = client
        .request_among_notifications(report(subscription, near))
        .await;
    assert_eq!(answer, Ok(ResponseBody::SceneCameras));
}
