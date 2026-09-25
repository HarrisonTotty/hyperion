//! The planetary requests and their answers: `system_bodies`, `body_detail` and `body_events`
//! (plan 14, P14.T35.c), added by plan 04's "Extending the convention".
//!
//! Until their handlers land (P14.T36, and P14.T31 for `body_events`) the server answers each of
//! them `unsupported`, as an older server would.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::planetary::{BodySummaryDto, DetailLevelDto, SectionDto, SystemPlaneDto, ZoneDto};
use crate::primitives::{BodyIdHex, SystemIdHex, UniverseIdHex, UniverseTime};
use crate::stellar::SystemSummaryDto;

/// Asks for every body of one system as it is at one time, at a detail level (`system_bodies`),
/// answered with a [`SystemBodiesDto`].
///
/// The server resolves the system ID first: a well-formed ID that names no system is refused with
/// `unknown_system`, and a time outside the clock window with `bad_request` naming `time`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemBodiesRequest {
    /// The universe the system is in.
    pub universe: UniverseIdHex,
    /// The system.
    pub system: SystemIdHex,
    /// The instant at which the bodies are described, within 1,000 years of the epoch.
    pub time: UniverseTime,
    /// The detail level asked for; the answer's `granted` says which the server granted.
    pub detail: DetailLevelDto,
}

/// Every body of one system at one time: its hosts, its zones, its populations and a record of
/// each body (plan 14, P14.T35.c).
///
/// The universe, system and time are the hosts' summary's, which echoes the request. The bodies
/// are listed flat in index order; the tree is rebuilt from each body's `parent`. The stars are
/// the hosts, not bodies of the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemBodiesDto {
    /// The detail level every record holds, which may be below the one asked for.
    pub granted: DetailLevelDto,
    /// The system's stars and the orbits that hold them together, as `system_summary` answers.
    pub hosts: SystemSummaryDto,
    /// The system's stable zones, one per host of planets, in hierarchy order and inside out: a
    /// pair's members' zones before the pair's own. Empty when the system has not yet formed.
    pub zones: Vec<ZoneDto>,
    /// The plane an orbit map is drawn on (plan 14, design note 21): the plane of the primary
    /// host's planets, or for a close binary the binary's; `null` when the system has no zone.
    pub system_plane: Option<SystemPlaneDto>,
    /// The system's belts, by the ID of each belt's population, in index order (`mass_and_orbit`);
    /// `ok` with an empty list for a system with none. Each belt's extent is its record's
    /// `population` section.
    pub belts: SectionDto<Vec<BodyIdHex>>,
    /// The system's cometary halo, by its population's ID (`mass_and_orbit`); `ok` with `null`
    /// for a system with none. The halo's extent is its record's `population` section.
    pub halo: SectionDto<Option<BodyIdHex>>,
    /// Every body's record, in index order. Below the `bulk` level a belt's members are left out,
    /// since a population seen as a whole does not resolve them.
    pub bodies: Vec<BodySummaryDto>,
}

/// Asks for one body's whole record at one time, at a detail level (`body_detail`), answered with
/// a [`BodyDetailDto`](crate::BodyDetailDto).
///
/// The server resolves the body's system and decodes its index first: a well-formed ID that names
/// no body is refused with `unknown_body`, one whose system is unknown with `unknown_system`, and a
/// time outside the clock window with `bad_request` naming `time`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyDetailRequest {
    /// The universe the body is in.
    pub universe: UniverseIdHex,
    /// The body.
    pub body: BodyIdHex,
    /// The instant at which the body is described, within 1,000 years of the epoch.
    pub time: UniverseTime,
    /// The detail level asked for; the answer's `granted` says which the server granted.
    pub detail: DetailLevelDto,
}

/// Asks for the events on one system's bodies between two times (`body_events`), answered with a
/// [`BodyEventsDto`].
///
/// A window longer than 1,000 years is refused with `bad_request` (plan 14, P14.T36.b). The
/// window's ends, `from` inclusive and `to` exclusive, and the field a refusal names, `to`, are
/// provisional until P14.T31 and T36 serve the kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyEventsRequest {
    /// The universe the system is in.
    pub universe: UniverseIdHex,
    /// The system.
    pub system: SystemIdHex,
    /// The start of the window, inclusive.
    pub from: UniverseTime,
    /// The end of the window, exclusive, not before `from`.
    pub to: UniverseTime,
}

/// The events on one system's bodies in a window of time (plan 14, P14.T31).
///
/// `universe`, `system`, `from` and `to` echo the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyEventsDto {
    /// The universe the system is in.
    pub universe: UniverseIdHex,
    /// The system.
    pub system: SystemIdHex,
    /// The start of the window, inclusive.
    pub from: UniverseTime,
    /// The end of the window, exclusive.
    pub to: UniverseTime,
    /// Every event in the window, by time.
    pub events: Vec<BodyEventDto>,
}

/// One event on a body: an impact, an eruption, a giant storm, a global dust storm, or a comet's
/// apparition (plan 14, design note 15).
///
/// Its variants are P14.T31's, which the slice does not build, so the type has no value yet and
/// `body_events` is answered `unsupported` until then. TypeScript sees it as `never`. It derives
/// only what an event carrying elements and a sampled track can keep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum BodyEventDto {}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::envelope::{ClientMessage, ResponseBody, ServerMessage};
    use crate::orbit::{HierarchyDto, HierarchyNodeDto};
    use crate::planetary::record::tests::{
        SYSTEM, planet_record, planet_summary, planet_summary_json,
    };
    use crate::planetary::zones::tests::{solar_zone, solar_zone_json};
    use crate::planetary::{BodyKindDto, BodyRecordDto, BodyStateDto};
    use crate::stellar::SystemExistenceDto;
    use crate::stellar::tests::{sunlike, sunlike_json};
    use crate::testing::assert_wire_form;

    /// The hosts of a system of one Sun-like star, 4.57 Gyr old.
    pub(crate) fn single_star_hosts() -> SystemSummaryDto {
        SystemSummaryDto {
            universe: UniverseIdHex::from_u64(42),
            system: SystemIdHex::from_u64(SYSTEM),
            time: UniverseTime::default(),
            existence: SystemExistenceDto::Exists,
            age_myr: 4_570.0,
            fe_h_dex: 0.0,
            stars: vec![sunlike()],
            hierarchy: HierarchyDto {
                nodes: vec![HierarchyNodeDto::Star {
                    body_index: 0,
                    mass_msun: 1.0,
                }],
            },
        }
    }

    pub(crate) fn single_star_hosts_json() -> Value {
        json!({
            "universe": "000000000000002a",
            "system": "0200080020000000",
            "time": { "seconds": 0, "nanos": 0 },
            "existence": "exists",
            "age_myr": 4_570.0,
            "fe_h_dex": 0.0,
            "stars": [sunlike_json()],
            "hierarchy": { "nodes": [{ "type": "star", "body_index": 0, "mass_msun": 1.0 }] },
        })
    }

    #[test]
    fn system_bodies_request_wire_form() {
        assert_wire_form(
            &SystemBodiesRequest {
                universe: UniverseIdHex::from_u64(42),
                system: SystemIdHex::from_u64(SYSTEM),
                time: UniverseTime {
                    seconds: 86_400,
                    nanos: 0,
                },
                detail: DetailLevelDto::MassAndOrbit,
            },
            json!({
                "universe": "000000000000002a",
                "system": "0200080020000000",
                "time": { "seconds": 86_400, "nanos": 0 },
                "detail": "mass_and_orbit",
            }),
        );
    }

    #[test]
    fn system_bodies_wire_form() {
        assert_wire_form(
            &SystemBodiesDto {
                granted: DetailLevelDto::Full,
                hosts: single_star_hosts(),
                zones: vec![solar_zone()],
                system_plane: Some(solar_zone().plane),
                belts: SectionDto::NotModelled,
                halo: SectionDto::NotModelled,
                bodies: vec![planet_summary()],
            },
            json!({
                "granted": "full",
                "hosts": single_star_hosts_json(),
                "zones": [solar_zone_json()],
                "system_plane": { "inclination_rad": 1.0, "ascending_node_rad": 2.5 },
                "belts": { "state": "not_modelled" },
                "halo": { "state": "not_modelled" },
                "bodies": [planet_summary_json()],
            }),
        );
    }

    #[test]
    fn a_system_with_no_belts_and_no_halo_says_so() {
        let empty = SystemBodiesDto {
            granted: DetailLevelDto::Bulk,
            hosts: single_star_hosts(),
            zones: Vec::new(),
            system_plane: None,
            belts: SectionDto::Ok(Vec::new()),
            halo: SectionDto::Ok(None),
            bodies: Vec::new(),
        };
        let wire = serde_json::to_value(&empty).unwrap();
        assert_eq!(wire["belts"], json!({ "state": "ok", "value": [] }));
        assert_eq!(wire["halo"], json!({ "state": "ok", "value": null }));
        assert_eq!(wire["system_plane"], Value::Null);
        assert_eq!(
            serde_json::from_value::<SystemBodiesDto>(wire).unwrap(),
            empty
        );
    }

    #[test]
    fn a_system_s_populations_are_listed_by_id() {
        let belt = BodyIdHex::from_parts(SYSTEM, 0xe000);
        let halo = BodyIdHex::from_parts(SYSTEM, 0xe100);
        let populated = SystemBodiesDto {
            granted: DetailLevelDto::MassAndOrbit,
            hosts: single_star_hosts(),
            zones: Vec::new(),
            system_plane: None,
            belts: SectionDto::Ok(vec![belt]),
            halo: SectionDto::Ok(Some(halo)),
            bodies: Vec::new(),
        };
        let wire = serde_json::to_value(&populated).unwrap();
        assert_eq!(
            wire["belts"],
            json!({ "state": "ok", "value": ["0200080020000000.e000"] })
        );
        assert_eq!(
            wire["halo"],
            json!({ "state": "ok", "value": "0200080020000000.e100" })
        );
        assert_eq!(
            serde_json::from_value::<SystemBodiesDto>(wire).unwrap(),
            populated
        );
    }

    #[test]
    fn body_detail_request_wire_form() {
        assert_wire_form(
            &BodyDetailRequest {
                universe: UniverseIdHex::from_u64(42),
                body: BodyIdHex::from_parts(SYSTEM, 0x0300),
                time: UniverseTime::default(),
                detail: DetailLevelDto::Full,
            },
            json!({
                "universe": "000000000000002a",
                "body": "0200080020000000.0300",
                "time": { "seconds": 0, "nanos": 0 },
                "detail": "full",
            }),
        );
    }

    #[test]
    fn a_body_detail_request_with_a_malformed_body_id_is_rejected() {
        let error = serde_json::from_value::<BodyDetailRequest>(json!({
            "universe": "000000000000002a",
            "body": "0200080020000000",
            "time": { "seconds": 0, "nanos": 0 },
            "detail": "full",
        }))
        .unwrap_err();
        assert!(
            error.to_string().contains("a full stop and 4 lowercase"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn body_events_request_wire_form() {
        assert_wire_form(
            &BodyEventsRequest {
                universe: UniverseIdHex::from_u64(42),
                system: SystemIdHex::from_u64(SYSTEM),
                from: UniverseTime {
                    seconds: -1_577_880_000,
                    nanos: 0,
                },
                to: UniverseTime {
                    seconds: 1_577_880_000,
                    nanos: 0,
                },
            },
            json!({
                "universe": "000000000000002a",
                "system": "0200080020000000",
                "from": { "seconds": -1_577_880_000_i64, "nanos": 0 },
                "to": { "seconds": 1_577_880_000_i64, "nanos": 0 },
            }),
        );
    }

    #[test]
    fn body_events_wire_form() {
        assert_wire_form(
            &BodyEventsDto {
                universe: UniverseIdHex::from_u64(42),
                system: SystemIdHex::from_u64(SYSTEM),
                from: UniverseTime::default(),
                to: UniverseTime {
                    seconds: 3_155_760_000,
                    nanos: 0,
                },
                events: Vec::new(),
            },
            json!({
                "universe": "000000000000002a",
                "system": "0200080020000000",
                "from": { "seconds": 0, "nanos": 0 },
                "to": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "events": [],
            }),
        );
    }

    #[test]
    fn no_body_event_parses_before_its_task_lands() {
        let error = serde_json::from_value::<BodyEventDto>(json!({ "impact": {} })).unwrap_err();
        assert!(
            error.to_string().contains("there are no variants"),
            "unexpected error: {error}"
        );
    }

    /// The messages of `packages/protocol/fixtures/planetary.json`, which `@hyperion/protocol`'s
    /// tests decode (plan 14, P14.T37), by name.
    fn fixture() -> serde_json::Map<String, Value> {
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/protocol/fixtures/planetary.json"
        ));
        match serde_json::from_str(text).unwrap() {
            Value::Object(mut messages) => {
                messages.remove("description");
                messages
            }
            other => panic!("the fixture is not an object: {other}"),
        }
    }

    /// The server message the fixture holds under `name`.
    fn server_message(name: &str) -> ServerMessage {
        serde_json::from_value(fixture()[name].clone()).unwrap()
    }

    #[test]
    fn every_shared_fixture_is_exactly_a_wire_form() {
        // A message that parses and writes back the same holds no key the types lack and lacks
        // none they write, so the client decodes what the server sends.
        let messages = fixture();
        for (name, wire) in &messages {
            let again = if wire["type"] == "request" {
                serde_json::to_value(serde_json::from_value::<ClientMessage>(wire.clone()).unwrap())
            } else {
                serde_json::to_value(serde_json::from_value::<ServerMessage>(wire.clone()).unwrap())
            }
            .unwrap();
            assert_eq!(&again, wire, "{name}");
        }
        assert_eq!(messages.len(), 10);
    }

    #[test]
    fn the_shared_slice_fixture_holds_the_wire_forms_pinned_here() {
        let ServerMessage::Response {
            body: ResponseBody::SystemBodies(system),
            ..
        } = server_message("system_bodies_response")
        else {
            panic!("system_bodies_response is not a system_bodies response");
        };
        assert_eq!(system.hosts, single_star_hosts());
        assert_eq!(system.zones, vec![solar_zone()]);
        assert_eq!(system.system_plane, Some(solar_zone().plane));
        assert_eq!(
            (system.belts, system.halo),
            (SectionDto::NotModelled, SectionDto::NotModelled)
        );
        assert_eq!(system.bodies.first(), Some(&planet_summary()));
        let ServerMessage::Response {
            body: ResponseBody::BodyDetail(detail),
            ..
        } = server_message("body_detail_response")
        else {
            panic!("body_detail_response is not a body_detail response");
        };
        assert_eq!(detail.granted, DetailLevelDto::Full);
        assert_eq!(detail.record, planet_record());
    }

    #[test]
    fn the_shared_contact_fixture_withholds_every_section() {
        let ServerMessage::Response {
            body: ResponseBody::BodyDetail(contact),
            ..
        } = server_message("body_detail_contact")
        else {
            panic!("body_detail_contact is not a body_detail response");
        };
        let earth = planet_record();
        assert_eq!(
            contact.record,
            BodyRecordDto {
                kind: BodyKindDto::Unresolved,
                label: SectionDto::NotResolved,
                mass_kg: SectionDto::NotResolved,
                orbit: SectionDto::NotResolved,
                moons: SectionDto::NotResolved,
                rings: SectionDto::NotResolved,
                population: SectionDto::NotResolved,
                bulk: SectionDto::NotResolved,
                surface: SectionDto::NotResolved,
                hooks: SectionDto::NotResolved,
                ..earth
            }
        );
        assert_eq!(contact.record.state, BodyStateDto::Present);
    }
}
