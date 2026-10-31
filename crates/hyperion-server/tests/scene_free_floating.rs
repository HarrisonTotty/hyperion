//! A scene beside a free-floating object, over a real socket, with P14.T27's holds lifted: a ship
//! 1 au from a brown dwarf and from a rogue planet arrives in its frame, is served its bodies as
//! `system_bodies` answers them, with the rogue planet's starless hosts, and leaves.
//!
//! P14.T27.a and T27.b give brown dwarfs and rogue planets bodies from generator version 26, and
//! `frame_at` walks their layers under the same holds. So every test of this binary first admits
//! the free-floating hosts for the whole process ([`admit_free_floating_hosts_in_this_process`]):
//! the server answers on its pool's threads, which no thread's switch reaches. `scene.rs` checks
//! that at 24 no ship is in such a frame. 26's bump folds this binary into `scene.rs`.

mod common;

use common::TestServer;
use common::scene::{
    TIME, free_floating, in_system, in_the_halo, next_scene, set_ship, subscribe, universe_on,
};
use hyperion_protocol::{
    BodyKindDto, DetailLevelDto, NotificationBody, RequestBody, ResponseBody, SceneArrivalDto,
    SystemBodiesRequest, SystemExistenceDto,
};
use hyperion_sim::planetary::testing::admit_free_floating_hosts_in_this_process;

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
