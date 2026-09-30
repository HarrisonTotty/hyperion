//! The extinction along lines of sight, and the extinction map, over a real socket (plan 07,
//! P07.T10.a and T10.c): two-way visibility, a system target against the sim's own line, a target
//! that names no system, the target limit, and the map's geometry and cache.

mod common;

use std::sync::OnceLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::{TestClient, TestServer};
use hyperion_protocol::{
    ErrorCode, ExtinctionMap, ExtinctionMapRequest, ExtinctionRequest, ExtinctionResult,
    ExtinctionTarget, GalacticPosition, MapView, RequestBody, ResponseBody, SystemIdHex,
    TargetExtinction, UniverseIdHex, UniverseTime,
};
use hyperion_server::compute::SIGHTLINE_QUALITY;
use hyperion_server::limits::MAX_EXTINCTION_TARGETS;
use hyperion_sim::Seed;
use hyperion_sim::coords::LyCell;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::extinction::{NoiseMode, sightline};
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::resolve;
use hyperion_sim::galaxy::query::position_at;
use hyperion_sim::id::SystemId;
use hyperion_sim::time;

/// The seed of every universe these tests create, the other integration tests' own.
const SEED: u64 = 0x4d2;

/// A layer-C system at the solar circle, one of `system_summary`'s pinned systems.
const SYSTEM: u64 = 0x4200_2cb2_0000_0001;

fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
}

fn position(cell_ly: [i32; 3]) -> GalacticPosition {
    GalacticPosition {
        cell_ly,
        offset_m: [1.0e15, 2.5e15, 0.5e15],
    }
}

fn sim_position(position: &GalacticPosition) -> hyperion_sim::coords::GalacticPosition {
    hyperion_sim::coords::GalacticPosition::new(LyCell::new(position.cell_ly), position.offset_m)
        .expect("a canonical position")
}

fn request(
    universe: &UniverseIdHex,
    origin: GalacticPosition,
    time: UniverseTime,
    targets: Vec<ExtinctionTarget>,
) -> RequestBody {
    RequestBody::Extinction(ExtinctionRequest {
        universe: universe.clone(),
        origin,
        time,
        targets,
    })
}

async fn extinction(client: &mut TestClient, body: RequestBody) -> ExtinctionResult {
    match client.request(body).await {
        Ok(ResponseBody::Extinction(result)) => result,
        other => panic!("expected the extinction, got {other:?}"),
    }
}

async fn universe(server: &TestServer) -> (TestClient, UniverseIdHex) {
    let mut client = server.connected().await;
    let info = client.create_universe("Talos", SEED).await;
    (client, info.id)
}

#[tokio::test]
async fn a_line_from_a_to_b_is_the_line_from_b_to_a() {
    let server = TestServer::start().await;
    let (mut client, id) = universe(&server).await;
    let (a, b) = (position([0, 26_000, 3]), position([1_200, 23_500, -40]));
    let time = UniverseTime::default();
    let there = extinction(
        &mut client,
        request(
            &id,
            a,
            time,
            vec![ExtinctionTarget::Position { position: b }],
        ),
    )
    .await;
    // Asked from a server that has cached nothing, so that the second answer is marched afresh.
    let other = TestServer::start().await;
    let (mut other_client, other_id) = universe(&other).await;
    let back = extinction(
        &mut other_client,
        request(
            &other_id,
            b,
            time,
            vec![ExtinctionTarget::Position { position: a }],
        ),
    )
    .await;
    assert_eq!(there.targets, back.targets);
    let [TargetExtinction::Ok { a_v_mag, .. }] = there.targets.as_slice() else {
        panic!("one line: {:?}", there.targets);
    };
    assert!(*a_v_mag > 0.0, "{a_v_mag}");
    client.close().await;
    other_client.close().await;
    other.stop().await;
    server.stop().await;
}

#[tokio::test]
async fn a_system_target_is_the_sims_line_to_where_the_system_is() {
    let server = TestServer::start().await;
    let (mut client, id) = universe(&server).await;
    let origin = position([100, 25_900, 0]);
    // At the epoch, where a system is its record's epoch position: at any other time it has
    // drifted by the full potential's velocity, which this test does not build (a debug build of
    // it is too slow for the fast suite), so the drift is left to `position_at`'s own tests.
    let time = UniverseTime::default();
    let result = extinction(
        &mut client,
        request(
            &id,
            origin,
            time,
            vec![ExtinctionTarget::System {
                id: SystemIdHex::from_u64(SYSTEM),
            }],
        ),
    )
    .await;
    assert_eq!(
        (result.universe.clone(), result.origin, result.time),
        (id, origin, time)
    );

    let record = resolve(galaxy(), SystemId::from_raw(SYSTEM).unwrap()).unwrap();
    let at = time::UniverseTime::new(time.seconds, time.nanos).unwrap();
    let line = sightline(
        galaxy().gas(),
        &sim_position(&origin),
        &position_at(galaxy(), &record, at),
        NoiseMode::Realised,
        SIGHTLINE_QUALITY,
        &[],
        &mut NoiseCache::with_capacity(64),
    );
    assert_eq!(
        result.targets,
        [TargetExtinction::Ok {
            a_v_mag: line.a_v().value(),
            e_b_v_mag: line.reddening().value(),
            a_k_mag: line.in_band(Band::K).value(),
            hydrogen_column_per_cm2: line.hydrogen_column().value(),
            neutral_hydrogen_column_per_cm2: line.neutral_hydrogen_column().value(),
        }]
    );
    // Asked again, the line is the cache's.
    let lines = server.stats().sightlines();
    let again = extinction(
        &mut client,
        request(
            &result.universe,
            origin,
            time,
            vec![ExtinctionTarget::System {
                id: SystemIdHex::from_u64(SYSTEM),
            }],
        ),
    )
    .await;
    assert_eq!(again.targets, result.targets);
    assert_eq!(server.stats().sightlines().hits(), lines.hits() + 1);
    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn an_unknown_system_is_answered_beside_the_good_targets() {
    let server = TestServer::start().await;
    let (mut client, id) = universe(&server).await;
    // A well-formed ID past its cell's candidates, and one whose spare bits are set.
    let unresolved = 0x4200_2cb2_0000_0fff;
    assert!(resolve(galaxy(), SystemId::from_raw(unresolved).unwrap()).is_err());
    let undecodable = 0x0200_0800_2000_0000 | (1 << 58);
    assert!(SystemId::from_raw(undecodable).is_err());
    let result = extinction(
        &mut client,
        request(
            &id,
            position([0, 26_000, 0]),
            UniverseTime::default(),
            vec![
                ExtinctionTarget::System {
                    id: SystemIdHex::from_u64(SYSTEM),
                },
                ExtinctionTarget::System {
                    id: SystemIdHex::from_u64(unresolved),
                },
                ExtinctionTarget::Position {
                    position: position([-500, 26_000, 0]),
                },
                ExtinctionTarget::System {
                    id: SystemIdHex::from_u64(undecodable),
                },
            ],
        ),
    )
    .await;
    assert!(
        matches!(
            result.targets.as_slice(),
            [
                TargetExtinction::Ok { .. },
                TargetExtinction::NoSuchSystem,
                TargetExtinction::Ok { .. },
                TargetExtinction::NoSuchSystem,
            ]
        ),
        "{:?}",
        result.targets
    );
    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn one_target_too_many_is_a_bad_request_naming_targets() {
    let server = TestServer::start().await;
    let (mut client, id) = universe(&server).await;
    let target = ExtinctionTarget::Position {
        position: position([10, 26_000, 0]),
    };
    let error = client
        .request(request(
            &id,
            position([0, 26_000, 0]),
            UniverseTime::default(),
            vec![target.clone(); MAX_EXTINCTION_TARGETS + 1],
        ))
        .await
        .unwrap_err();
    assert_eq!(MAX_EXTINCTION_TARGETS + 1, 65);
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("targets"))
    );
    // The largest request allowed fits the inbound frame and is answered.
    let result = extinction(
        &mut client,
        request(
            &id,
            position([0, 26_000, 0]),
            UniverseTime::default(),
            vec![target; MAX_EXTINCTION_TARGETS],
        ),
    )
    .await;
    assert_eq!(result.targets.len(), MAX_EXTINCTION_TARGETS);
    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn an_extinction_map_carries_its_geometry_and_the_second_ask_is_a_hit() {
    let server = TestServer::start().await;
    let (mut client, id) = universe(&server).await;
    let body = RequestBody::ExtinctionMap(ExtinctionMapRequest {
        universe: id.clone(),
        view: MapView::FaceOn,
        resolution: 128,
        bits: 8,
    });
    let map: ExtinctionMap = match client.request(body.clone()).await {
        Ok(ResponseBody::ExtinctionMap(map)) => map,
        other => panic!("expected the extinction map, got {other:?}"),
    };
    assert_eq!(
        (
            map.universe.clone(),
            map.view,
            map.width_px,
            map.height_px,
            map.bits
        ),
        (id.clone(), MapView::FaceOn, 128, 128, 8)
    );
    assert_eq!(map.ly_per_px.to_bits(), 1_024.0_f64.to_bits());
    assert_eq!(map.floor_log10_mag.to_bits(), (-2.0_f64).to_bits());
    // Face-on, the most obscured line through the disc is of order a magnitude or two, and the
    // corners of the root cube, far outside the gas disc, have nothing above the floor.
    assert!(
        (0.0..1.0).contains(&map.ceiling_log10_mag),
        "{}",
        map.ceiling_log10_mag
    );
    let codes = STANDARD.decode(&map.data_base64).unwrap();
    assert_eq!(codes.len(), 128 * 128);
    assert_eq!(codes.iter().max(), Some(&255));
    assert_eq!(codes[0], 0);

    let refused = client
        .request(RequestBody::ExtinctionMap(ExtinctionMapRequest {
            universe: id,
            view: MapView::EdgeOn,
            resolution: 100,
            bits: 8,
        }))
        .await
        .unwrap_err();
    assert_eq!(refused.field.as_deref(), Some("resolution"));
    let again = client.request(body).await.unwrap();
    assert_eq!(again, ResponseBody::ExtinctionMap(map));
    assert_eq!(server.stats().extinction_maps().hits(), 1);
    client.close().await;
    server.stop().await;
}
