//! The scene is bounded by Knowledge (rendering plan R03, R03.T9; the rendering brainstorm's
//! Testing): with a restrictive knowledge that grants `contact` for half a pinned system's bodies
//! and names three of ten craft as contacts, no state or notification, wherever the cameras are,
//! holds a craft that is not a contact or a section above its body's grant.
//!
//! Asserted on the JSON as it arrives, as plan 12 asserts that no bearing contact has a `host`
//! key: every body's `level` is the knowledge's grant, a contact carries no orbit and no mass (a
//! Hill radius's inputs), and no section above a body's level is `ok`.
//!
//! Nothing here depends on how quickly the machine runs: each move of the stand-in waits for the
//! client to hear what the move before it brought, not for a time or a number of notifications.

mod common;

use std::collections::BTreeMap;

use common::scene::{
    SEED, TIME, TenCraft, craft_offset, in_system, in_the_halo, subscribe, systems,
};
use common::{TestServer, patiently};
use hyperion_protocol::{
    BodyIdHex, CameraReportDto, DetailLevelDto, FramePositionDto, KinematicsDto, NotificationBody,
    RequestBody, ResponseBody, SceneArrivalDto, SceneCamerasRequest, SceneShipRequest,
    ServerMessage, SystemIdHex,
};
use hyperion_server::scene::{CraftState, SceneKnowledge};
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

/// What the test has checked so far.
#[derive(Debug, Default)]
struct Checked {
    /// Body records, of the state, of the systems that arrived and of the bodies re-sent.
    records: usize,
    /// Whole systems that arrived in a notification.
    arrivals: usize,
}

/// Checks a scene state's or notification's JSON, counting what it checked in `checked`.
fn check(scene: &Value, checked: &mut Checked) {
    for craft in scene["craft"].as_array().into_iter().flatten() {
        let id = craft["craft"].as_str().expect("a craft's ID");
        assert!(CONTACTS.contains(&id), "{id} is not a contact");
    }
    if scene["arrival"]["system"].is_object() {
        checked.arrivals += 1;
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
            checked.records += 1;
        }
    }
    for body in scene["bodies"].as_array().into_iter().flatten() {
        check_record(&body["record"], body["level"].as_str().expect("a level"));
        checked.records += 1;
    }
}

/// A notification as the client reads it: its subscription and its body.
type Notification = (u32, NotificationBody);

/// Whether a notification is the one a wait is for.
type Wanted = fn(&Notification) -> bool;

/// Checks every notification among `messages`.
fn check_all(messages: &[Notification], checked: &mut Checked) {
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

/// The arrival or departure a notification brings, if it brings one.
fn arrival_of((_, body): &Notification) -> Option<&SceneArrivalDto> {
    match body {
        NotificationBody::Scene(notification) => notification.arrival.as_ref(),
    }
}

/// Whether a notification brings a whole system.
fn is_arrival(notification: &Notification) -> bool {
    matches!(
        arrival_of(notification),
        Some(SceneArrivalDto::System { .. })
    )
}

/// Whether a notification tells of leaving the system.
fn is_departure(notification: &Notification) -> bool {
    matches!(arrival_of(notification), Some(SceneArrivalDto::NoSystem))
}

/// Reads and checks notifications until one is `wanted`, for at most the harness's patience;
/// `what` names it for the failure.
async fn read_until(
    client: &mut common::TestClient,
    checked: &mut Checked,
    what: &str,
    wanted: Wanted,
) {
    patiently(&format!("waiting for {what}"), async {
        loop {
            let notification = client.next_notification().await;
            let found = wanted(&notification);
            check_all(&[notification], checked);
            if found {
                return;
            }
        }
    })
    .await;
}

/// Moves the stand-in to interstellar space and back to `back`, checking everything pushed, until
/// the whole system has arrived again.
///
/// The move back is made only once the departure has been heard. The universe's scene setting
/// keeps only the latest, so a subscription's task still on the pool, at a heartbeat say, when
/// both moves are set would take only the move back: a move within the system, which brings no
/// arrival. The task pushes the departure after it has taken the move out, so once the departure
/// is heard the move back is a setting the task has yet to take, and taking it brings the system.
async fn leave_and_return(
    client: &mut common::TestClient,
    universe: &hyperion_protocol::UniverseIdHex,
    back: KinematicsDto,
    checked: &mut Checked,
) {
    let stages: [(KinematicsDto, &str, Wanted); 2] = [
        (in_the_halo(), "the departure for the halo", is_departure),
        (back, "the system's arrival on the return", is_arrival),
    ];
    for (pose, what, wanted) in stages {
        let (answer, notifications) = client
            .request_among_notifications(RequestBody::SceneShip(SceneShipRequest {
                universe: universe.clone(),
                ship: pose,
                time_rate: 10_000,
            }))
            .await;
        assert!(answer.is_ok(), "{answer:?}");
        // The task's push and the answer race, so what the move brings may be among these.
        let heard = notifications.iter().any(wanted);
        check_all(&notifications, checked);
        if !heard {
            read_until(client, checked, what, wanted).await;
        }
    }
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
    let mut checked = Checked::default();
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
        // somewhere else in it, so that the whole system arrives again, four times in all, each
        // arrival checked before the next placement.
        if placement % 50 == 49 {
            let offset = [0, 1, 2].map(|_| 20.0 * au * unit(&mut lcg));
            leave_and_return(&mut client, &universe, ship_at(offset), &mut checked).await;
        }
    }
    // A last heartbeat or two, with the contacts the server places.
    let (answer, notifications) = client
        .request_among_notifications(ship([au, 0.0, 0.0], 0))
        .await;
    assert!(answer.is_ok(), "{answer:?}");
    check_all(&notifications, &mut checked);
    for _ in 0..2 {
        let (_, body) = client.next_notification().await;
        check_all(&[(1, body)], &mut checked);
    }
    // At least: a body that no longer resolved at its grant would bring the whole system again.
    assert!(
        checked.arrivals >= 4,
        "{} arrivals of the whole system checked, where each of the four returns brings one",
        checked.arrivals
    );
    assert!(
        checked.records >= 5 * bodies.len(),
        "{} records checked: the state's and four arrivals'",
        checked.records
    );
}
