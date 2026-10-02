//! The scene's helpers, shared by the scene's integration tests (rendering plan R03): a pinned
//! universe and system, the stand-in's poses, and subscribing.

use hyperion_protocol::{
    DetailLevelDto, FramePositionDto, KinematicsDto, NotificationBody, RequestBody, ResponseBody,
    SceneCraftDto, SceneNotificationDto, SceneShipRequest, SceneStateDto, SceneSubscribeRequest,
    SubscribeRequest, SubscriptionState, SubscriptionTopic, SystemIdHex, UniverseIdHex,
    UniverseTime,
};
use hyperion_server::scene::{CraftSource, CraftState};
use hyperion_server::universe::UniverseId;
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
use hyperion_sim::id::Layer;

use super::{TestClient, TestServer};

/// The seed of the universe these scenes are in.
pub const SEED: u64 = 0x4d2;

/// The scene time the tests set, an hour after the epoch.
pub const TIME: UniverseTime = UniverseTime {
    seconds: 3_600,
    nanos: 0,
};

/// The first two systems of the layer-C cell at the solar circle, as the server generates them.
pub fn systems() -> [SystemIdHex; 2] {
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
pub fn in_system(system: &SystemIdHex) -> KinematicsDto {
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
pub fn in_the_halo() -> KinematicsDto {
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
pub async fn universe() -> (TestServer, TestClient, UniverseIdHex) {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Scene", SEED).await.id;
    (server, client, universe)
}

/// Sets the universe's ship stand-in and clock, keeping the notifications that arrive meanwhile.
pub async fn set_ship(
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
pub async fn subscribe(
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
pub async fn next_scene(client: &mut TestClient) -> (u32, SceneNotificationDto) {
    match client.next_notification().await {
        (subscription, NotificationBody::Scene(notification)) => (subscription, notification),
    }
}

/// Where craft `k` of [`TenCraft`] is in its system: 10⁹ m apart along a line 1 au out.
pub fn craft_offset(k: u8) -> [f64; 3] {
    [1.496e11, 1.0e9 * f64::from(k), 0.0]
}

/// Ten craft at rest in a system, `craft-0` to `craft-9`, each stating the scene time it was
/// asked at.
#[derive(Debug)]
pub struct TenCraft(pub SystemIdHex);

impl CraftSource for TenCraft {
    fn craft_at(
        &self,
        _universe: UniverseId,
        t: hyperion_sim::time::UniverseTime,
    ) -> Vec<CraftState> {
        (0..10_u8)
            .map(|k| CraftState::new(ten_craft_record(&self.0, k, t)))
            .collect()
    }
}

/// Craft `k` of [`TenCraft`] in `system` at `t`, as the wire carries it.
pub fn ten_craft_record(
    system: &SystemIdHex,
    k: u8,
    t: hyperion_sim::time::UniverseTime,
) -> SceneCraftDto {
    SceneCraftDto {
        craft: format!("craft-{k}"),
        hull: "test-hull".to_owned(),
        state: KinematicsDto {
            position: FramePositionDto::System {
                system: system.clone(),
                offset_m: craft_offset(k),
            },
            velocity_m_s: [0.0; 3],
            time: UniverseTime {
                seconds: t.seconds(),
                nanos: t.subsec_nanos(),
            },
        },
        attitude: [1.0, 0.0, 0.0, 0.0],
        angular_velocity_rad_s: [0.0; 3],
        planned_path: None,
    }
}
