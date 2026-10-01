//! The scene is bounded by Knowledge (rendering plan R03, R03.T9; the rendering brainstorm's
//! Testing): with a restrictive knowledge that grants `contact` for half a pinned system's bodies
//! and names three of ten craft as contacts, no state or notification, wherever the cameras are,
//! holds a craft that is not a contact or a section above its body's grant.
//!
//! Asserted on the JSON as it arrives, as plan 12 asserts that no bearing contact has a `host`
//! key: every body's `level` is the knowledge's grant, a contact carries no orbit and no mass (a
//! Hill radius's inputs), and no section above a body's level is `ok`.

mod common;

use std::collections::BTreeMap;

use common::TestServer;
use common::scene::{SEED, TIME, in_system, in_the_halo, subscribe, systems};
use hyperion_protocol::{
    BodyIdHex, CameraReportDto, DetailLevelDto, FramePositionDto, KinematicsDto, RequestBody,
    ResponseBody, SceneCamerasRequest, SceneCraftDto, SceneShipRequest, ServerMessage, SystemIdHex,
};
use hyperion_server::scene::{CraftSource, CraftState, SceneKnowledge};
use hyperion_server::universe::UniverseId;
use hyperion_sim::id::BodyId;
use hyperion_sim::planetary::record::{DetailLevel, RecordSection};
use hyperion_testkit::lcg::Lcg;
use serde_json::Value;

/// The craft the restrictive knowledge names as contacts.
const CONTACTS: [&str; 3] = ["craft-1", "craft-4", "craft-7"];

/// Grants `contact` for the bodies of odd slots and the level asked for the rest; three of ten
/// craft are contacts.
#[derive(Debug)]
struct Restrictive;

/// Whether the restrictive knowledge grants a body of index `index` only `contact`.
fn is_hidden(index: u16) -> bool {
    (index >> 8) % 2 == 1
}

impl SceneKnowledge for Restrictive {
    fn grant(&self, body: BodyId, asked: DetailLevel) -> DetailLevel {
        if is_hidden(body.body_index()) {
            DetailLevel::Contact
        } else {
            asked
        }
    }

    fn is_contact(&self, craft: &CraftState) -> bool {
        CONTACTS.contains(&craft.id())
    }
}

/// Where craft `k` is: 10⁹ m apart along a line 1 au out.
fn craft_offset(k: u8) -> [f64; 3] {
    [1.496e11, 1.0e9 * f64::from(k), 0.0]
}

/// Ten craft in a system.
#[derive(Debug)]
struct TenCraft(SystemIdHex);

impl CraftSource for TenCraft {
    fn craft_at(
        &self,
        _universe: UniverseId,
        t: hyperion_sim::time::UniverseTime,
    ) -> Vec<CraftState> {
        (0..10_u8)
            .map(|k| {
                CraftState::new(SceneCraftDto {
                    craft: format!("craft-{k}"),
                    hull: "test-hull".to_owned(),
                    state: KinematicsDto {
                        position: FramePositionDto::System {
                            system: self.0.clone(),
                            offset_m: craft_offset(k),
                        },
                        velocity_m_s: [0.0; 3],
                        time: hyperion_protocol::UniverseTime {
                            seconds: t.seconds(),
                            nanos: t.subsec_nanos(),
                        },
                    },
                    attitude: [1.0, 0.0, 0.0, 0.0],
                    angular_velocity_rad_s: [0.0; 3],
                    planned_path: None,
                })
            })
            .collect()
    }
}

/// The JSON field of each section a body's summary carries, and the section it is.
const SECTIONS: [(&str, RecordSection); 7] = [
    ("label", RecordSection::Label),
    ("mass_kg", RecordSection::Mass),
    ("orbit", RecordSection::Orbit),
    ("moons", RecordSection::Moons),
    ("rings", RecordSection::Rings),
    ("population", RecordSection::Population),
    ("bulk", RecordSection::Bulk),
];

/// The level the restrictive knowledge grants the body `id` names, at `full` asked.
fn granted(id: &str) -> &'static str {
    let index = id
        .split('.')
        .nth(1)
        .and_then(|index| u16::from_str_radix(index, 16).ok())
        .unwrap_or_else(|| panic!("a body ID: {id}"));
    if is_hidden(index) { "contact" } else { "full" }
}

/// The level, as the wire names it, of a detail level.
fn level_of(level: DetailLevel) -> &'static str {
    match level {
        DetailLevel::Contact => "contact",
        DetailLevel::MassAndOrbit => "mass_and_orbit",
        DetailLevel::Bulk => "bulk",
        DetailLevel::Surface => "surface",
        DetailLevel::Full => "full",
    }
}

/// Checks one body's summary against `level`, the level it was granted.
fn check_record(record: &Value, level: &str) {
    let id = record["id"].as_str().expect("a body's ID");
    assert_eq!(level, granted(id), "{id}'s level is the knowledge's grant");
    for (field, section) in SECTIONS {
        let above = DetailLevel::ALL
            .iter()
            .find(|candidate| level_of(**candidate) == level)
            .is_some_and(|granted| section.level() > *granted);
        if above {
            assert_ne!(
                record[field]["state"], "ok",
                "{id}: {field} is above its grant {level}"
            );
        }
    }
    if level == "contact" {
        for field in ["mass_kg", "orbit"] {
            assert_ne!(record[field]["state"], "ok", "{id}: a contact's {field}");
        }
        assert_eq!(record["kind"]["type"], "unresolved", "{id}");
    }
}

/// Checks a scene state's or notification's JSON; returns the body records it checked.
fn check(scene: &Value, checked: &mut usize) {
    for craft in scene["craft"].as_array().into_iter().flatten() {
        let id = craft["craft"].as_str().expect("a craft's ID");
        assert!(CONTACTS.contains(&id), "{id} is not a contact");
    }
    let systems = [&scene["system"], &scene["arrival"]["system"]];
    for system in systems.into_iter().filter(|system| system.is_object()) {
        let grants: BTreeMap<&str, &str> = system["grants"]
            .as_array()
            .expect("the grants")
            .iter()
            .map(|grant| {
                let id = grant["body"].as_str().expect("a body's ID");
                assert_eq!(grant["level"].as_str(), Some(granted(id)), "{id}");
                (id, grant["level"].as_str().expect("a level"))
            })
            .collect();
        for record in system["system"]["bodies"].as_array().expect("the bodies") {
            let id = record["id"].as_str().expect("a body's ID");
            check_record(record, grants[id]);
            *checked += 1;
        }
    }
    for body in scene["bodies"].as_array().into_iter().flatten() {
        check_record(&body["record"], body["level"].as_str().expect("a level"));
        *checked += 1;
    }
}

/// Checks every notification among `messages`.
fn check_all(messages: &[(u32, hyperion_protocol::NotificationBody)], checked: &mut usize) {
    for (_, body) in messages {
        let message = ServerMessage::Notification {
            subscription: 1,
            body: body.clone(),
        };
        let json = serde_json::to_value(&message).expect("a notification serialises");
        check(&json["body"], checked);
    }
}

/// One of `items`, drawn uniformly.
fn pick<'a, T>(lcg: &mut Lcg, items: &'a [T]) -> &'a T {
    let n = u64::try_from(items.len()).expect("a short list");
    &items[usize::try_from(lcg.next_below(n)).expect("an index of the list")]
}

/// A uniform draw in `[-1, 1)`.
fn unit(lcg: &mut Lcg) -> f64 {
    2.0 * lcg.next_f64() - 1.0
}

/// Camera placement `placement` of the test, drawn from `lcg`: anywhere within 40 au of the
/// barycentre, beside a craft that is not a contact, inside a hidden body's Hill sphere, or near
/// any body of the scene, in turn.
fn placement_of(
    placement: u32,
    lcg: &mut Lcg,
    system: &SystemIdHex,
    bodies: &[String],
    hidden: &[String],
) -> FramePositionDto {
    let au = 1.496e11;
    match placement % 4 {
        // Anywhere within 40 au, in the system's frame.
        0 => FramePositionDto::System {
            system: system.clone(),
            offset_m: [0, 1, 2].map(|_| 40.0 * au * unit(lcg)),
        },
        // Beside a craft that is not a contact.
        1 => {
            let [x, y, z] = craft_offset(*pick(lcg, &[0, 2, 3, 5, 6, 8, 9]));
            FramePositionDto::System {
                system: system.clone(),
                offset_m: [x + 1.0e5 * unit(lcg), y + 1.0e5 * unit(lcg), z],
            }
        }
        // Inside a hidden body's Hill sphere, in its frame.
        2 => FramePositionDto::Body {
            body: BodyIdHex::try_from(pick(lcg, hidden).clone()).expect("a body ID"),
            offset_m: [0, 1, 2].map(|_| 1.0e8 * unit(lcg)),
        },
        // Near any body of the scene, in its frame.
        _ => FramePositionDto::Body {
            body: BodyIdHex::try_from(pick(lcg, bodies).clone()).expect("a body ID"),
            offset_m: [0, 1, 2].map(|_| 1.0e7 * unit(lcg)),
        },
    }
}

/// A server with the restrictive knowledge and ten craft in `system`, and its data directory.
async fn restricted_server(system: &SystemIdHex) -> (tempfile::TempDir, TestServer) {
    let data_dir = tempfile::tempdir().unwrap();
    let server = TestServer::start_with(
        TestServer::config(data_dir.path())
            .scene_knowledge(Restrictive)
            .craft_source(TenCraft(system.clone()))
            .build(),
    )
    .await;
    (data_dir, server)
}

#[tokio::test]
async fn no_scene_holds_what_the_ship_does_not_know_wherever_the_cameras_are() {
    let [system, _] = systems();
    let (_data_dir, server) = restricted_server(&system).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Scene", SEED).await.id;
    let ship_at = |offset_m: [f64; 3]| KinematicsDto {
        position: FramePositionDto::System {
            system: system.clone(),
            offset_m,
        },
        ..in_system(&system)
    };
    let ship = |offset_m: [f64; 3], time_rate| {
        RequestBody::SceneShip(SceneShipRequest {
            universe: universe.clone(),
            ship: ship_at(offset_m),
            time_rate,
        })
    };
    let (answer, _) = client
        .request_among_notifications(ship([1.496e11, 0.0, 0.0], 1_000))
        .await;
    assert!(answer.is_ok(), "{answer:?}");
    let (subscription, state) = subscribe(&mut client, &universe, DetailLevelDto::Full).await;
    let mut checked = 0;
    let state = serde_json::to_value(&state).expect("the state serialises");
    check(&state, &mut checked);
    let bodies: Vec<String> = state["system"]["grants"]
        .as_array()
        .expect("the ship is in the system")
        .iter()
        .map(|grant| grant["body"].as_str().expect("an ID").to_owned())
        .collect();
    let hidden: Vec<String> = bodies
        .iter()
        .filter(|id| granted(id) == "contact")
        .cloned()
        .collect();
    assert!(!hidden.is_empty() && hidden.len() < bodies.len());
    assert_eq!(
        state["craft"].as_array().map(Vec::len),
        Some(CONTACTS.len())
    );

    let mut arrivals = 0;
    let mut lcg = Lcg::new(0x0003_9000_0000_0009);
    let au = 1.496e11;
    for placement in 0..200_u32 {
        let position = placement_of(placement, &mut lcg, &system, &bodies, &hidden);
        let view = u8::try_from(placement % 8).unwrap();
        let report = RequestBody::SceneCameras(SceneCamerasRequest {
            subscription,
            cameras: vec![CameraReportDto {
                view,
                pose: KinematicsDto {
                    position,
                    velocity_m_s: [0.0; 3],
                    time: TIME,
                },
            }],
        });
        let (answer, notifications) = client.request_among_notifications(report).await;
        assert_eq!(
            answer,
            Ok(ResponseBody::SceneCameras),
            "placement {placement}"
        );
        check_all(&notifications, &mut checked);
        // Now and then the stand-in leaves the system for interstellar space and comes back to
        // somewhere else in it, so that the whole system arrives again.
        if placement % 50 == 49 {
            let offset = [0, 1, 2].map(|_| 20.0 * au * unit(&mut lcg));
            for (pose, time_rate) in [(in_the_halo(), 10_000), (ship_at(offset), 10_000)] {
                let (answer, notifications) = client
                    .request_among_notifications(RequestBody::SceneShip(SceneShipRequest {
                        universe: universe.clone(),
                        ship: pose,
                        time_rate,
                    }))
                    .await;
                assert!(answer.is_ok(), "{answer:?}");
                check_all(&notifications, &mut checked);
            }
            // The arrival, which follows the answer.
            let (_, body) = client.next_notification().await;
            check_all(&[(1, body)], &mut checked);
            arrivals += 1;
        }
    }
    // A last heartbeat or two, with the contacts the server places.
    let (_, notifications) = client
        .request_among_notifications(ship([au, 0.0, 0.0], 0))
        .await;
    check_all(&notifications, &mut checked);
    for _ in 0..2 {
        let (_, body) = client.next_notification().await;
        check_all(&[(1, body)], &mut checked);
    }
    assert_eq!(arrivals, 4);
    assert!(
        checked >= 5 * bodies.len(),
        "{checked} records checked: the state's and four arrivals'"
    );
}
