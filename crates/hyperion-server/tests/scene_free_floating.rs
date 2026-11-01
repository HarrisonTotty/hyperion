//! A scene beside a free-floating object, over a real socket, with P14.T27's holds lifted: a ship
//! 1 au from a brown dwarf and from a rogue planet arrives in its frame, is served its bodies as
//! `system_bodies` answers them, with the rogue planet's starless hosts, and leaves; and the rogue
//! planet's `system_summary`, those hosts, in both of plan 12's modes.
//!
//! P14.T27.a and T27.b give brown dwarfs and rogue planets bodies from generator version 26, and
//! `frame_at` walks their layers under the same holds. So every test of this binary first admits
//! the free-floating hosts for the whole process ([`admit_free_floating_hosts_in_this_process`]):
//! the server answers on its pool's threads, which no thread's switch reaches. `scene.rs` checks
//! that at 24 no ship is in such a frame, and `system_summary.rs` that a rogue planet's summary is
//! refused. 26's bump folds this binary into `scene.rs`, and its summary tests into
//! `system_summary.rs`.

mod common;

use common::scene::{
    TIME, free_floating, in_system, in_the_halo, next_scene, set_ship, subscribe, universe_on,
};
use common::{TestClient, TestServer};
use hyperion_protocol::{
    BodyKindDto, DetailLevelDto, GalacticPosition, NotificationBody, QueryModeDto, RequestBody,
    ResolveSystemRequest, ResolvedSystem, ResponseBody, SceneArrivalDto, SystemBodiesRequest,
    SystemExistenceDto, SystemIdHex, SystemSummaryDto, SystemSummaryRequest, UniverseIdHex,
    UniverseTime,
};
use hyperion_sim::planetary::testing::admit_free_floating_hosts_in_this_process;

/// Seconds in a Julian year, the sim's year: the wire carries a time as whole seconds.
const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

/// A ship beside each free-floating object arrives in its frame at the scene's entry ratio, holds
/// its bodies as `system_bodies` answers them, the brown dwarf with its one star as its host and the
/// rogue planet with none, and departs when it moves to the halo.
#[tokio::test]
async fn a_ship_beside_a_brown_dwarf_or_a_rogue_planet_arrives_is_served_its_bodies_and_leaves() {
    admit_free_floating_hosts_in_this_process();
    let server = TestServer::start().await;
    let [dwarf, rogue] = free_floating();
    for (object, name) in [(&dwarf, "brown dwarf"), (&rogue, "rogue planet")] {
        let (mut client, universe) = universe_on(&server, &format!("Scene: {name}")).await;
        set_ship(&mut client, &universe, in_system(object), 0).await;
        let (_, state) = subscribe(&mut client, &universe, DetailLevelDto::Full).await;
        let scene = state.system.expect("the ship is in the object's frame");
        assert_eq!(&scene.system.hosts.system, object, "{name}");
        assert!(state.tidal_radius_m.is_some_and(|radius| radius > 0.0));
        let (answer, _) = client
            .request_among_notifications(RequestBody::SystemBodies(SystemBodiesRequest {
                universe: universe.clone(),
                system: object.clone(),
                time: TIME,
                detail: DetailLevelDto::Full,
            }))
            .await;
        let Ok(ResponseBody::SystemBodies(expected)) = answer else {
            panic!("system_bodies answers for the {name}: {answer:?}");
        };
        assert_eq!(scene.system, *expected, "{name}");
        assert_eq!(scene.grants.len(), scene.system.bodies.len());
        let hosts = &scene.system.hosts;
        assert_eq!(hosts.existence, SystemExistenceDto::Exists);
        if object == &rogue {
            // A rogue planet has no star: an empty host list, its own age and [Fe/H].
            assert!(hosts.stars.is_empty() && hosts.hierarchy.nodes.is_empty());
            assert!(scene.system.zones.is_empty() && scene.system.system_plane.is_none());
            let first = scene.system.bodies.first().expect("the object is a body");
            assert_eq!(first.id.to_parts().1, 0x0000);
            assert_eq!(first.kind, BodyKindDto::Planet);
            assert_eq!(first.parent, None, "the object is its system's root");
        } else {
            assert_eq!(
                hosts.stars.len(),
                1,
                "a brown dwarf is its system's one star"
            );
        }
        let mut seen = set_ship(&mut client, &universe, in_the_halo(), 0).await;
        let departure = loop {
            let notification = match seen.pop() {
                Some((_, NotificationBody::Scene(notification))) => notification,
                // No alerts subscription is open, so none is sent; skip one all the same.
                Some((_, NotificationBody::Alerts(_))) => continue,
                None => next_scene(&mut client).await.1,
            };
            if notification.arrival.is_some() {
                break notification;
            }
        };
        assert_eq!(departure.arrival, Some(SceneArrivalDto::NoSystem), "{name}");
        client.close().await;
    }
    server.stop().await;
}

/// `system`'s summary at `time` in `mode`, as the server answers it.
async fn summary_in(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    system: &SystemIdHex,
    time: UniverseTime,
    mode: QueryModeDto,
) -> SystemSummaryDto {
    let request = RequestBody::SystemSummary(SystemSummaryRequest {
        universe: universe.clone(),
        system: system.clone(),
        time,
        mode,
    });
    match client.request(request).await {
        Ok(ResponseBody::SystemSummary(summary)) => summary,
        other => panic!("expected the summary of {system:?}, got {other:?}"),
    }
}

/// `system` at [`TIME`] in `mode`, as `resolve_system` answers it.
async fn resolved_in(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    system: &SystemIdHex,
    mode: QueryModeDto,
) -> ResolvedSystem {
    let request = RequestBody::ResolveSystem(ResolveSystemRequest {
        universe: universe.clone(),
        system: system.clone(),
        time: TIME,
        mode,
    });
    match client.request(request).await {
        Ok(ResponseBody::ResolveSystem(resolved)) => *resolved,
        other => panic!("expected {system:?} resolved, got {other:?}"),
    }
}

/// P14.T27.b: a rogue planet's `system_summary` is the hosts `system_bodies` answers with its
/// bodies at the same time, its starless summary, at the scene's time and 900 years before it.
/// 26's bump moves this test into `system_summary.rs`.
#[tokio::test]
async fn a_rogue_planet_s_summary_is_the_hosts_of_its_bodies() {
    admit_free_floating_hosts_in_this_process();
    let server = TestServer::start().await;
    let [_, rogue] = free_floating();
    let (mut client, universe) = universe_on(&server, "Summary: rogue planet").await;
    let before = UniverseTime {
        seconds: TIME.seconds - 900 * SECONDS_PER_JULIAN_YEAR,
        nanos: 0,
    };
    for time in [TIME, before] {
        // The summary first, so that it is answered from a body cache that has not seen the planet.
        let summary = summary_in(&mut client, &universe, &rogue, time, QueryModeDto::Now).await;
        let answer = client
            .request(RequestBody::SystemBodies(SystemBodiesRequest {
                universe: universe.clone(),
                system: rogue.clone(),
                time,
                detail: DetailLevelDto::Bulk,
            }))
            .await;
        let Ok(ResponseBody::SystemBodies(bodies)) = answer else {
            panic!("system_bodies answers for the rogue planet: {answer:?}");
        };
        assert_eq!(summary, bodies.hosts, "{time:?}");
        assert_eq!(summary.existence, SystemExistenceDto::Exists);
        assert!(summary.stars.is_empty() && summary.hierarchy.nodes.is_empty());
        assert_eq!(summary.observed, None);
    }
    client.close().await;
    server.stop().await;
}

/// P14.T27.b: a rogue planet's summary asked from 300 ly away is its starless summary when the light
/// the observer receives left it, as `now` answers it at that time, with the observation
/// `resolve_system` gives the same observer, on the same line (lean, 2026-11-01). 26's bump moves
/// this test into `system_summary.rs`.
#[tokio::test]
async fn a_rogue_planet_s_observed_summary_is_its_summary_when_its_light_left() {
    admit_free_floating_hosts_in_this_process();
    let server = TestServer::start().await;
    let [_, rogue] = free_floating();
    let (mut client, universe) = universe_on(&server, "Observed summary: rogue planet").await;
    let here = resolved_in(&mut client, &universe, &rogue, QueryModeDto::Now).await;
    let [x, y, z] = here.record.position.cell_ly;
    let mode = QueryModeDto::Observed {
        observer: GalacticPosition {
            cell_ly: [x, y + 300, z],
            offset_m: [0.0; 3],
        },
    };
    let seen = summary_in(&mut client, &universe, &rogue, TIME, mode).await;
    assert_eq!(
        (&seen.universe, &seen.system, seen.time),
        (&universe, &rogue, TIME)
    );
    let Some(light) = seen.observed else {
        panic!("an observed summary carries its observation: {seen:?}");
    };
    let traced = resolved_in(&mut client, &universe, &rogue, mode).await;
    assert_eq!(traced.record.observed.as_ref(), Some(&light));
    assert!(
        (299.0..301.0).contains(&light.light_age_yr),
        "{} years",
        light.light_age_yr
    );
    // The light left inside the clock window, so `now` is asked at the emitted time itself.
    let then = summary_in(
        &mut client,
        &universe,
        &rogue,
        light.emitted,
        QueryModeDto::Now,
    )
    .await;
    assert_eq!(
        SystemSummaryDto {
            time: light.emitted,
            observed: None,
            ..seen
        },
        then
    );
    client.close().await;
    server.stop().await;
}
