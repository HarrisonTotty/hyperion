//! The scene subscription, live, over a real socket (rendering plan R03, R03.T8.a): subscribe in a
//! pinned system and receive its bodies as `system_bodies` answers them, a heartbeat each second,
//! the ship stand-in's changes pushed to every subscription of the universe, a departure from the
//! system, and the refusals of an unknown subscription and of a camera out of reach.

mod common;

use common::TestServer;
use common::scene::{
    SEED, TIME, TenCraft, in_system, in_the_halo, next_scene, set_ship, subscribe, systems,
    universe,
};
use hyperion_protocol::{
    CameraReportDto, DetailLevelDto, ErrorCode, FramePositionDto, KinematicsDto, NotificationBody,
    RequestBody, ResponseBody, SceneArrivalDto, SceneCamerasRequest, SceneClockStateDto,
    SystemBodiesRequest,
};
use hyperion_server::scene::{CraftState, SceneKnowledge};
use hyperion_sim::id::BodyId;
use hyperion_sim::planetary::record::DetailLevel;

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

/// Every craft is a contact; every body is granted the level asked.
#[derive(Debug)]
struct EveryCraft;

impl SceneKnowledge for EveryCraft {
    fn grant(&self, _body: BodyId, asked: DetailLevel) -> DetailLevel {
        asked
    }

    fn is_contact(&self, _craft: &CraftState) -> bool {
        true
    }
}

/// A scene with ten craft is pushed them at the 64 Hz tick, each push stating its time, at about
/// 0.25 MB/s (rendering plan R03, R03.T8.b; Design note 4).
#[tokio::test]
async fn craft_are_pushed_at_64_hz_each_push_stating_its_time() {
    let [system, _] = systems();
    let data_dir = tempfile::tempdir().unwrap();
    let server = TestServer::start_with(
        TestServer::config(data_dir.path())
            .scene_knowledge(EveryCraft)
            .craft_source(TenCraft(system.clone()))
            .build(),
    )
    .await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Scene", SEED).await.id;
    let [system, _] = systems();
    set_ship(&mut client, &universe, in_system(&system), 1).await;
    let (_, state) = subscribe(&mut client, &universe, DetailLevelDto::Bulk).await;
    assert_eq!(
        state.craft.len(),
        10,
        "the craft are in the scene from the start"
    );

    // A second of scene time at 1x, counted from the first craft push's time.
    let mut first: Option<f64> = None;
    let mut pushes = 0_u32;
    let mut bytes = 0_usize;
    let mut previous = f64::NEG_INFINITY;
    loop {
        let (_, notification) = next_scene(&mut client).await;
        let Some(craft) = notification.craft.as_ref() else {
            continue;
        };
        // Seconds after the scene's start, which an `f64` holds to the nanosecond here.
        let since = notification.clock.time.seconds - TIME.seconds;
        let time = f64::from(i32::try_from(since).unwrap())
            + f64::from(notification.clock.time.nanos) * 1e-9;
        assert!(time > previous, "each push states a later time");
        previous = time;
        assert_eq!(craft.len(), 10);
        // Stated at the craft push's own time, which a later push merged into it may pass.
        let stated = &craft[0].state.time;
        assert!(
            (stated.seconds, stated.nanos)
                <= (
                    notification.clock.time.seconds,
                    notification.clock.time.nanos
                ),
            "the craft are stated at or before the push's time"
        );
        let start = *first.get_or_insert(time);
        if time - start >= 1.0 {
            break;
        }
        pushes += 1;
        bytes += serde_json::to_string(&notification).unwrap().len();
    }
    // 64 a second when the runtime keeps up; missed ticks are skipped, so a loaded machine sends
    // fewer, never more. The rate measured on 2026-09-30 (64, 0.191 MB/s) is in the plan's notes.
    assert!(
        (1..=66).contains(&pushes),
        "{pushes} craft pushes in a second"
    );
    let rate_mb_s = f64::from(u32::try_from(bytes).unwrap()) / 1e6;
    // The figures R03.T15 records, read with `--nocapture`.
    #[expect(
        clippy::print_stderr,
        reason = "the measured rate is reported for the plan"
    )]
    {
        eprintln!("{pushes} craft pushes in a second of scene time, {rate_mb_s:.3} MB/s");
    }
    assert!(
        rate_mb_s < 0.5,
        "{rate_mb_s} MB/s, over Design note 4's finding threshold"
    );
}
