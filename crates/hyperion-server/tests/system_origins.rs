//! A client's system ID placed by its origin, over a real socket (plan 07, P07.T10.c): `extinction`
//! and `scene_ship`, and a scene framed on a feature member as its clock runs, place a grid system
//! as the range query does and a catalogue feature's member on its own line, and refuse the
//! galactic centre's members, at the epoch and away from it, the server answering on. `extinction`
//! refuses a catalogue class's system too (plan 09, P09.T33.a).

mod common;

use common::TestServer;
use common::origins::{
    GRID_SYSTEM, a_catalogue_system, a_feature_member, at_years, galaxy, the_centre_s_members,
};
use common::scene::{next_scene, set_ship, subscribe, universe_on};
use hyperion_protocol::{
    BodyIdHex, DetailLevelDto, ErrorCode, ExtinctionRequest, ExtinctionTarget, FramePositionDto,
    GalacticPosition, KinematicsDto, RequestBody, ResponseBody, SceneShipRequest,
    SystemBodiesRequest, SystemIdHex, TargetExtinction, UniverseTime,
};
use hyperion_sim::coords::{LyCell, SystemPosition};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
use hyperion_sim::galaxy::features::gas_overlay::FeatureGas;
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::extinction::{NoiseMode, SIGHTLINE_QUALITY, Sightline, sightline};
use hyperion_sim::galaxy::gas::modifiers::GasModifierSource;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::{SystemRecord, resolve};
use hyperion_sim::galaxy::query::position_at;
use hyperion_sim::id::SystemId;
use hyperion_sim::observe::{Drift, Trajectory};
use hyperion_sim::time;
use hyperion_sim::units::Years;
use hyperion_sim::units::consts::METRES_PER_AU;

fn sim_time(at: UniverseTime) -> time::UniverseTime {
    time::UniverseTime::new(at.seconds, at.nanos).expect("a valid time")
}

fn wire_position(position: &hyperion_sim::coords::GalacticPosition) -> GalacticPosition {
    GalacticPosition {
        cell_ly: position.cell().to_array(),
        offset_m: position.offset_metres(),
    }
}

fn hex(id: SystemId) -> SystemIdHex {
    SystemIdHex::from_u64(id.raw())
}

/// The systems these tests name, one of each origin, as the sim resolves them.
struct Origins {
    grid: SystemRecord,
    member: SystemRecord,
    centre: [SystemId; 3],
    catalogue: SystemId,
}

impl Origins {
    fn of(galaxy: &Galaxy) -> Self {
        let resolved = |id| resolve(galaxy, id).expect("the system resolves");
        Self {
            grid: resolved(SystemId::from_raw(GRID_SYSTEM).expect("a system ID")),
            member: *a_feature_member(galaxy).record(),
            centre: the_centre_s_members(galaxy),
            catalogue: a_catalogue_system(galaxy),
        }
    }
}

/// Every request that places a system from a client's ID, on one server: each takes a client and a
/// universe of its own, named apart, as `tests/scene.rs`'s share theirs.
///
/// Each of the server's galaxies is built with its full potential, 4.4 s in a test build, so the
/// properties share one server; none reads a count of the server's.
#[tokio::test]
async fn a_client_s_system_is_placed_by_its_origin_in_every_request() {
    let origins = Origins::of(galaxy());
    let server = TestServer::start().await;
    extinction_ends_each_line_where_the_target_s_origin_places_it(&server, &origins).await;
    scene_ship_places_a_system_frame_by_its_origin(&server, &origins).await;
    a_body_of_a_feature_member_s_system_frames_the_ship(&server, &origins).await;
    a_scene_framed_on_a_feature_member_runs_on_its_line(&server, &origins).await;
    server.stop().await;
}

/// Run by [`a_client_s_system_is_placed_by_its_origin_in_every_request`] on its shared server: a
/// grid target ends at `position_at`, its line through plan 09's features' gas at the request's
/// time as the server's lines take it from generator version 27 (P09.T40.a), a feature member at
/// its line's position, and the centre's member, its black hole and a catalogue system on their
/// lines, which the server traces from 27 (below it `no_such_system`), beside the others, the
/// black hole's tidal disruption victim `no_such_system` after its disruption, at the epoch and a
/// century after it; and the next request is answered.
async fn extinction_ends_each_line_where_the_target_s_origin_places_it(
    server: &TestServer,
    origins: &Origins,
) {
    let galaxy = galaxy();
    let (mut client, universe) = universe_on(server, "Origins: extinction").await;
    let origin = GalacticPosition {
        cell_ly: [100, 25_900, 0],
        offset_m: [1.0e15, 2.5e15, 0.5e15],
    };
    let line = Drift::of_record(galaxy, &origins.member).expect("a feature member's line");
    for time in [at_years(0), at_years(100)] {
        let t = sim_time(time);
        let mut targets = vec![
            ExtinctionTarget::System {
                id: hex(origins.grid.id()),
            },
            ExtinctionTarget::System {
                id: hex(origins.member.id()),
            },
            ExtinctionTarget::Position {
                position: wire_position(&line.position_at(t)),
            },
        ];
        targets.extend(
            origins
                .centre
                .map(|id| ExtinctionTarget::System { id: hex(id) }),
        );
        targets.push(ExtinctionTarget::System {
            id: hex(origins.catalogue),
        });
        let answer = client
            .request(RequestBody::Extinction(ExtinctionRequest {
                universe: universe.clone(),
                origin,
                time,
                targets,
            }))
            .await;
        let Ok(ResponseBody::Extinction(result)) = answer else {
            panic!("extinction at {time:?} answers: {answer:?}");
        };
        let to_grid = grid_line(galaxy, &origin, &origins.grid, t);
        let [grid, member, on_line, centre @ .., catalogue] = result.targets.as_slice() else {
            panic!("seven answers at {time:?}: {:?}", result.targets);
        };
        assert_eq!(
            *grid,
            TargetExtinction::Ok {
                a_v_mag: to_grid.a_v().value(),
                e_b_v_mag: to_grid.reddening().value(),
                a_k_mag: to_grid.in_band(Band::K).value(),
                hydrogen_column_per_cm2: to_grid.hydrogen_column().value(),
                neutral_hydrogen_column_per_cm2: to_grid.neutral_hydrogen_column().value(),
            },
            "the grid system at {time:?}"
        );
        assert!(matches!(member, TargetExtinction::Ok { .. }), "{member:?}");
        assert_eq!(member, on_line, "the feature member at {time:?}");
        // The centre's member and its black hole on their lines; its victim, disrupted before the
        // epoch, is no system then.
        assert!(
            matches!(
                centre,
                [
                    TargetExtinction::Ok { .. },
                    TargetExtinction::Ok { .. },
                    TargetExtinction::NoSuchSystem
                ]
            ),
            "the centre's members at {time:?}: {centre:?}"
        );
        assert!(
            matches!(catalogue, TargetExtinction::Ok { .. }),
            "the catalogue system at {time:?}: {catalogue:?}"
        );
    }
    let next = client
        .request(RequestBody::Extinction(ExtinctionRequest {
            universe,
            origin,
            time: at_years(0),
            targets: vec![ExtinctionTarget::Position {
                position: wire_position(origins.grid.epoch_position()),
            }],
        }))
        .await;
    assert!(
        matches!(&next, Ok(ResponseBody::Extinction(result))
            if matches!(result.targets.as_slice(), [TargetExtinction::Ok { .. }])),
        "the next request is answered: {next:?}"
    );
    client.close().await;
}

/// The sim's line from `origin` to `grid` at `t`, through plan 09's features' gas at `t`, as the
/// server's lines take it from generator version 27 (P09.T40.a).
fn grid_line(
    galaxy: &Galaxy,
    origin: &GalacticPosition,
    grid: &SystemRecord,
    t: time::UniverseTime,
) -> Sightline {
    let from =
        hyperion_sim::coords::GalacticPosition::new(LyCell::new(origin.cell_ly), origin.offset_m)
            .expect("a canonical position");
    let to = position_at(galaxy, grid, t);
    let mut modifiers = Vec::new();
    FeatureGas::new(
        galaxy,
        &NoFeatureCache,
        Years::new(t.since_epoch().as_julian_years_f64()),
    )
    .modifiers_near_segment(&from, &to, &mut modifiers);
    sightline(
        galaxy.gas(),
        &from,
        &to,
        NoiseMode::Realised,
        SIGHTLINE_QUALITY,
        &modifiers,
        &mut NoiseCache::with_capacity(64),
    )
}

/// The wire's pose 1 au from `system`'s barycentre in its frame, at rest, at `time`.
fn in_frame_of(system: SystemId, time: UniverseTime) -> KinematicsDto {
    KinematicsDto {
        position: FramePositionDto::System {
            system: hex(system),
            offset_m: [METRES_PER_AU, 0.0, 0.0],
        },
        velocity_m_s: [0.0; 3],
        time,
    }
}

/// Run by [`a_client_s_system_is_placed_by_its_origin_in_every_request`] on its shared server: a
/// system frame on the grid system, the feature member, the centre's member and black hole and the
/// catalogue system is accepted, at the epoch and a century after it: from generator version 27
/// the server traces the centre's members and the catalogue's hosts on their lines, where below it
/// a frame on them was `unknown_system` naming `ship.position`, as one on the black hole's victim,
/// disrupted before the epoch, still is.
async fn scene_ship_places_a_system_frame_by_its_origin(server: &TestServer, origins: &Origins) {
    let (mut client, universe) = universe_on(server, "Origins: scene_ship").await;
    for time in [at_years(0), at_years(100)] {
        for record in [origins.grid, origins.member] {
            set_ship(&mut client, &universe, in_frame_of(record.id(), time), 0).await;
        }
        let [member, black_hole, victim] = origins.centre;
        for id in [member, black_hole, origins.catalogue] {
            set_ship(&mut client, &universe, in_frame_of(id, time), 0).await;
        }
        let error = client
            .request(RequestBody::SceneShip(SceneShipRequest {
                universe: universe.clone(),
                ship: in_frame_of(victim, time),
                time_rate: 0,
            }))
            .await
            .expect_err("a victim after its disruption cannot be placed");
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::UnknownSystem, Some("ship.position")),
            "{victim:?} at {time:?}: {error:?}"
        );
    }
    client.close().await;
}

/// Run by [`a_client_s_system_is_placed_by_its_origin_in_every_request`] on its shared server: a
/// body frame in the feature member's system is accepted a century after the epoch.
async fn a_body_of_a_feature_member_s_system_frames_the_ship(
    server: &TestServer,
    origins: &Origins,
) {
    let (mut client, universe) = universe_on(server, "Origins: body frame").await;
    let time = at_years(100);
    let answer = client
        .request(RequestBody::SystemBodies(SystemBodiesRequest {
            universe: universe.clone(),
            system: hex(origins.member.id()),
            time,
            detail: DetailLevelDto::Bulk,
        }))
        .await;
    let Ok(ResponseBody::SystemBodies(bodies)) = answer else {
        panic!("system_bodies answers: {answer:?}");
    };
    let body: &BodyIdHex = &bodies
        .bodies
        .first()
        .expect("the feature member's system has a body")
        .id;
    let pose = KinematicsDto {
        position: FramePositionDto::Body {
            body: body.clone(),
            offset_m: [1.0e7, 0.0, 0.0],
        },
        velocity_m_s: [0.0; 3],
        time,
    };
    set_ship(&mut client, &universe, pose, 0).await;
    client.close().await;
}

/// Run by [`a_client_s_system_is_placed_by_its_origin_in_every_request`] on its shared server: a
/// ship framed on the feature member at the epoch, its offset taking it 1 au from the grid system,
/// subscribes with the clock running at 100,000 times and takes two advances.
///
/// The scene is in the grid system, so the ship is inside that system's entry sphere, 0.9 of a
/// tidal radius of over 10¹⁵ m: placed from the member, not misplaced grossly. That is as near as
/// the wire shows the ship's galactic position; over the seconds before `subscribe` the member's
/// line and its epoch position differ by under an au. The arrival's observer takes the member's
/// drift (the ship's frame is not the scene's system). The bit-for-bit placement and the drift's
/// value are `scene::core`'s `a_ship_framed_on_a_feature_member_rides_the_member_s_line`.
async fn a_scene_framed_on_a_feature_member_runs_on_its_line(
    server: &TestServer,
    origins: &Origins,
) {
    let (mut client, universe) = universe_on(server, "Origins: scene").await;
    let mut offset_m = SystemPosition::from_galactic(
        origins.grid.epoch_position(),
        origins.member.epoch_position(),
    )
    .metres();
    offset_m[0] += METRES_PER_AU;
    let pose = KinematicsDto {
        position: FramePositionDto::System {
            system: hex(origins.member.id()),
            offset_m,
        },
        velocity_m_s: [0.0; 3],
        time: at_years(0),
    };
    set_ship(&mut client, &universe, pose, 100_000).await;
    let (subscription, state) = subscribe(&mut client, &universe, DetailLevelDto::Bulk).await;
    let scene = state
        .system
        .expect("the ship is 1 au from the grid system's barycentre");
    assert_eq!(scene.system.hosts.system, hex(origins.grid.id()));
    let mut previous = state.clock.time;
    for _ in 0..2 {
        // A `subscription_ended` here would fail `next_scene`, which reads only notifications.
        let (on, notification) = next_scene(&mut client).await;
        assert_eq!(on, subscription);
        let now = notification.clock.time;
        assert!(
            (now.seconds, now.nanos) > (previous.seconds, previous.nanos),
            "the clock runs: {now:?} after {previous:?}"
        );
        previous = now;
    }
    client.close().await;
}
