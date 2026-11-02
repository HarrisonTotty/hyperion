//! The planetary requests and their answers: `system_bodies`, `body_detail` and `body_events`
//! (plan 14, P14.T35.c), added by plan 04's "Extending the convention".
//!
//! The server serves all three (P14.T36.b), `body_events` from generator version 25, which puts
//! P14.T31's events on the record; before it, `body_events` is answered `unsupported`, as an older
//! server answers it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::planetary::{
    BodySummaryDto, DetailLevelDto, OrbitHostDto, SectionDto, SystemPlaneDto, ZoneDto,
};
use crate::primitives::{BodyIdHex, EventIdHex, SystemIdHex, UniverseIdHex, UniverseTime};
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
/// Both ends must lie inside the clock window, `bad_request` naming the end that does not, and a
/// window that ends before it starts or is longer than 1,000 Julian years is refused with
/// `bad_request` naming `to` (plan 14, P14.T36.b). The window holds `from` and not `to`. Before
/// generator version 25, which puts P14.T31's events on the record, the server answers
/// `unsupported`, as a server built before the events does.
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

/// The events on one system's bodies in a window of time (plan 14, P14.T31, P14.T36.b).
///
/// `universe`, `system`, `from` and `to` echo the request. An answer lists at most 25,000 events:
/// where the window holds more, it lists every event of the widest span about the window's middle
/// that holds no more, which `listed_from` and `listed_to` state, so that a client asking about the
/// time at a window's middle has the events nearest it, and asks a narrower window for the rest.
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
    /// The start of the span every event of which is listed, inclusive: `from`, or later where the
    /// window holds more events than an answer lists.
    pub listed_from: UniverseTime,
    /// The end of that span, exclusive: `to`, or earlier where the window holds more events than an
    /// answer lists.
    pub listed_to: UniverseTime,
    /// Every event in the listed span, by time and then by event ID.
    pub events: Vec<BodyEventDto>,
}

/// One event on a body: an impact, an eruption, a great storm, a global dust storm, or a comet's
/// passage through perihelion (plan 14, design note 15, P14.T31; the simulation's
/// `planetary::events::BodyEvent`), tagged by its `type`.
///
/// Every event has its `id`, plan 01's event ID, from which the server regenerates it alone, and
/// its time: an impact's instant, or the onset of an eruption or a storm, or a comet's perihelion
/// passage. A comet carries its orbit and a track of positions the server samples, since a client
/// does not propagate an open orbit. The type is closed: it carries all five kinds from the first
/// build that sends a value of it (P14.T36.b), which sets its shape, as `BodySurfaceDto`'s first
/// value set its own (`PROTOCOL_VERSION`'s documentation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum BodyEventDto {
    /// An impact on a body above the energy its rate is counted from (P14.T31.a, `body.impact`).
    Impact {
        /// The event.
        id: EventIdHex,
        /// The body struck.
        body: BodyIdHex,
        /// The instant of the impact.
        time: UniverseTime,
        /// The projectile's diameter, m, positive.
        projectile_diameter_m: f64,
        /// The energy it brings, J, positive.
        energy_j: f64,
        /// The impact velocity, m/s, positive: the body's mean.
        impact_velocity_m_per_s: f64,
        /// The site's planetocentric latitude in the body's own frame, rad, in [−π ÷ 2, π ÷ 2].
        latitude_rad: f64,
        /// The site's east longitude in the body's own frame, rad, in [0, 2π).
        longitude_rad: f64,
    },
    /// An eruption on a body (P14.T31.a, `body.eruption`).
    Eruption {
        /// The event.
        id: EventIdHex,
        /// The body erupting.
        body: BodyIdHex,
        /// When it starts.
        onset: UniverseTime,
        /// How long it lasts, s, positive and at most 10 Julian years.
        duration_s: f64,
    },
    /// A great storm on a giant, after Saturn's great white spots (P14.T31.b, `body.storm`).
    Storm {
        /// The event.
        id: EventIdHex,
        /// The giant.
        body: BodyIdHex,
        /// When it breaks out.
        onset: UniverseTime,
        /// How long it lasts, s, positive.
        duration_s: f64,
        /// Its latitude band, in the northern hemisphere of the giant's own frame.
        band: StormBandDto,
    },
    /// A global dust storm on a dry, thin-aired body, after Mars's (P14.T31.b, `body.duststorm`).
    DustStorm {
        /// The event.
        id: EventIdHex,
        /// The body.
        body: BodyIdHex,
        /// When it breaks out, in the perihelion season.
        onset: UniverseTime,
        /// How long it lasts, s, positive.
        duration_s: f64,
        /// The true anomaly of the orbit its seasons follow at the onset, rad, in [−π, π]: the
        /// body's own, or a moon's planet's.
        onset_true_anomaly_rad: f64,
    },
    /// A long-period comet's passage through perihelion, sent in from the system's cometary halo
    /// (P14.T31.a, `system.comet`).
    Comet(Box<CometEventDto>),
}

/// The latitude band of a great storm, in the northern hemisphere of the giant's own frame (P14.T31.b),
/// as Saturn's six great white spots fell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StormBandDto {
    /// Near the equator.
    Equatorial,
    /// At mid-latitudes.
    MidLatitude,
    /// At high latitudes.
    Polar,
}

/// A long-period comet's passage through perihelion (P14.T31.a): its orbit about the halo's host,
/// its nucleus, when it is listed, and a track of its positions the server samples over that.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CometEventDto {
    /// The event, of the system's subject.
    pub id: EventIdHex,
    /// The cometary halo that sent it in.
    pub halo: BodyIdHex,
    /// What it goes round: the halo's host, a star, a pair or the system's barycentre.
    pub host: OrbitHostDto,
    /// When it passes perihelion: the event's time.
    pub perihelion_time: UniverseTime,
    /// Its perihelion distance from the host, m, positive.
    pub perihelion_m: f64,
    /// Its eccentricity, in (0.99, 1]: nearly parabolic.
    pub eccentricity: f64,
    /// Its orbit's inclination to the system frame's x-y plane, rad, in [0, π].
    pub inclination_rad: f64,
    /// Its longitude of the ascending node, rad, in [0, 2π).
    pub ascending_node_rad: f64,
    /// Its argument of perihelion, rad, in [0, 2π).
    pub argument_of_periapsis_rad: f64,
    /// The host's gravitational parameter μ, m³ s⁻², positive.
    pub mu_m3_s2: f64,
    /// Its nucleus's diameter, m, positive.
    pub nucleus_diameter_m: f64,
    /// When it is first listed: when it comes inside 30 au × √(L ÷ L☉) of the host.
    pub listed_from: UniverseTime,
    /// When it is no longer listed: when it leaves that radius, or reaches the host's surface if
    /// it plunges.
    pub listed_to: UniverseTime,
    /// Whether its perihelion lies inside the host, which it strikes before perihelion.
    pub plunges: bool,
    /// Its positions over its listing, in time order, from `listed_from` to `listed_to`, closer
    /// together about perihelion.
    pub track: Vec<CometTrackPointDto>,
}

/// One sampled position of a comet: where it is relative to its host at a time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CometTrackPointDto {
    /// The instant.
    pub time: UniverseTime,
    /// Its position relative to the host, m, along the system frame's axes.
    pub position_m: [f64; 3],
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::envelope::{ClientMessage, ResponseBody, ServerMessage};
    use crate::orbit::{HierarchyDto, HierarchyNodeDto};
    use crate::planetary::record::tests::{
        SYSTEM, earth_condensates, earth_crust, planet_record, planet_summary, planet_summary_json,
    };
    use crate::planetary::zones::tests::{solar_zone, solar_zone_json};
    use crate::planetary::{BodyKindDto, BodyRecordDto, BodyStateDto};
    use crate::stellar::SystemExistenceDto;
    use crate::stellar::tests::{sunlike, sunlike_json};
    use crate::testing::{assert_wire_form, assert_wire_strings};

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
            observed: None,
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
                listed_from: UniverseTime {
                    seconds: 1_262_304_000,
                    nanos: 0,
                },
                listed_to: UniverseTime {
                    seconds: 1_893_456_000,
                    nanos: 0,
                },
                events: Vec::new(),
            },
            json!({
                "universe": "000000000000002a",
                "system": "0200080020000000",
                "from": { "seconds": 0, "nanos": 0 },
                "to": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "listed_from": { "seconds": 1_262_304_000_i64, "nanos": 0 },
                "listed_to": { "seconds": 1_893_456_000_i64, "nanos": 0 },
                "events": [],
            }),
        );
    }

    /// An event ID of the test system: `subject` its body index, or `None` for the system's own.
    fn event_id(subject: Option<u16>, word: u64) -> EventIdHex {
        let subject = subject.map_or_else(
            || format!("{SYSTEM:016x}"),
            |index| BodyIdHex::from_parts(SYSTEM, index).to_string(),
        );
        EventIdHex::try_from(format!("{subject}:{word:016x}")).unwrap()
    }

    /// One event of each kind, illustrative values of the sizes the generator gives: the events
    /// of `packages/protocol/fixtures/planetary.json`'s `body_events_populated`, in time order.
    pub(crate) fn one_of_each_event() -> Vec<BodyEventDto> {
        let giant = BodyIdHex::from_parts(SYSTEM, 0x0500);
        vec![
            BodyEventDto::Comet(Box::new(CometEventDto {
                id: event_id(None, 0x0404_ffff_ffff_ce00),
                halo: BodyIdHex::from_parts(SYSTEM, 0xef00),
                host: OrbitHostDto::Star { body_index: 0 },
                perihelion_time: UniverseTime {
                    seconds: -1_577_489_772,
                    nanos: 190_519_035,
                },
                perihelion_m: 2.047_5e10,
                eccentricity: 0.999_975,
                inclination_rad: 1.25,
                ascending_node_rad: 3.5,
                argument_of_periapsis_rad: 0.75,
                mu_m3_s2: 1.327_124_400_18e20,
                nucleus_diameter_m: 3_184.5,
                listed_from: UniverseTime {
                    seconds: -1_888_438_790,
                    nanos: 238_587_678,
                },
                listed_to: UniverseTime {
                    seconds: -1_266_540_754,
                    nanos: 142_450_392,
                },
                plunges: false,
                track: vec![
                    CometTrackPointDto {
                        time: UniverseTime {
                            seconds: -1_888_438_790,
                            nanos: 238_587_678,
                        },
                        position_m: [-3.5e12, 2.25e12, -1.5e12],
                    },
                    CometTrackPointDto {
                        time: UniverseTime {
                            seconds: -1_577_489_772,
                            nanos: 190_519_035,
                        },
                        position_m: [1.25e10, -1.5e10, 5.0e9],
                    },
                    CometTrackPointDto {
                        time: UniverseTime {
                            seconds: -1_266_540_754,
                            nanos: 142_450_392,
                        },
                        position_m: [2.75e12, -1.75e12, 1.25e12],
                    },
                ],
            })),
            BodyEventDto::Eruption {
                id: event_id(Some(0x0300), 0x0401_ffff_fff6_0500),
                body: BodyIdHex::from_parts(SYSTEM, 0x0300),
                onset: UniverseTime {
                    seconds: -1_545_016_715,
                    nanos: 305_613_213,
                },
                duration_s: 852_234.5,
            },
            BodyEventDto::Impact {
                id: event_id(Some(0x0500), 0x0400_ffff_ffff_fe00),
                body: giant.clone(),
                time: UniverseTime {
                    seconds: -61_215_250,
                    nanos: 0,
                },
                projectile_diameter_m: 107.5,
                energy_j: 4.75e17,
                impact_velocity_m_per_s: 54_006.0,
                latitude_rad: -0.5,
                longitude_rad: 2.25,
            },
            BodyEventDto::DustStorm {
                id: event_id(Some(0x0400), 0x0403_0000_0000_0000),
                body: BodyIdHex::from_parts(SYSTEM, 0x0400),
                onset: UniverseTime {
                    seconds: 25_116_480,
                    nanos: 500_000_000,
                },
                duration_s: 12_096_000.0,
                onset_true_anomaly_rad: -0.375,
            },
            BodyEventDto::Storm {
                id: event_id(Some(0x0500), 0x0402_ffff_ffff_f300),
                body: giant,
                onset: UniverseTime {
                    seconds: 198_238_971,
                    nanos: 282_075_757,
                },
                duration_s: 805_426.5,
                band: StormBandDto::MidLatitude,
            },
        ]
    }

    /// [`one_of_each_event`]'s wire form.
    pub(crate) fn one_of_each_event_json() -> Value {
        json!([
            {
                "type": "comet",
                "id": "0200080020000000:0404ffffffffce00",
                "halo": "0200080020000000.ef00",
                "host": { "type": "star", "body_index": 0 },
                "perihelion_time": { "seconds": -1_577_489_772_i64, "nanos": 190_519_035 },
                "perihelion_m": 2.047_5e10,
                "eccentricity": 0.999_975,
                "inclination_rad": 1.25,
                "ascending_node_rad": 3.5,
                "argument_of_periapsis_rad": 0.75,
                "mu_m3_s2": 1.327_124_400_18e20,
                "nucleus_diameter_m": 3_184.5,
                "listed_from": { "seconds": -1_888_438_790_i64, "nanos": 238_587_678 },
                "listed_to": { "seconds": -1_266_540_754_i64, "nanos": 142_450_392 },
                "plunges": false,
                "track": [
                    {
                        "time": { "seconds": -1_888_438_790_i64, "nanos": 238_587_678 },
                        "position_m": [-3.5e12, 2.25e12, -1.5e12],
                    },
                    {
                        "time": { "seconds": -1_577_489_772_i64, "nanos": 190_519_035 },
                        "position_m": [1.25e10, -1.5e10, 5.0e9],
                    },
                    {
                        "time": { "seconds": -1_266_540_754_i64, "nanos": 142_450_392 },
                        "position_m": [2.75e12, -1.75e12, 1.25e12],
                    },
                ],
            },
            {
                "type": "eruption",
                "id": "0200080020000000.0300:0401fffffff60500",
                "body": "0200080020000000.0300",
                "onset": { "seconds": -1_545_016_715_i64, "nanos": 305_613_213 },
                "duration_s": 852_234.5,
            },
            {
                "type": "impact",
                "id": "0200080020000000.0500:0400fffffffffe00",
                "body": "0200080020000000.0500",
                "time": { "seconds": -61_215_250, "nanos": 0 },
                "projectile_diameter_m": 107.5,
                "energy_j": 4.75e17,
                "impact_velocity_m_per_s": 54_006.0,
                "latitude_rad": -0.5,
                "longitude_rad": 2.25,
            },
            {
                "type": "dust_storm",
                "id": "0200080020000000.0400:0403000000000000",
                "body": "0200080020000000.0400",
                "onset": { "seconds": 25_116_480, "nanos": 500_000_000 },
                "duration_s": 12_096_000.0,
                "onset_true_anomaly_rad": -0.375,
            },
            {
                "type": "storm",
                "id": "0200080020000000.0500:0402fffffffff300",
                "body": "0200080020000000.0500",
                "onset": { "seconds": 198_238_971, "nanos": 282_075_757 },
                "duration_s": 805_426.5,
                "band": "mid_latitude",
            },
        ])
    }

    #[test]
    fn body_event_wire_forms() {
        let Value::Array(wires) = one_of_each_event_json() else {
            panic!("the events' wire form is a list");
        };
        for (event, wire) in one_of_each_event().iter().zip(wires) {
            assert_wire_form(event, wire);
        }
    }

    #[test]
    fn storm_band_wire_strings() {
        assert_wire_strings(&[
            (StormBandDto::Equatorial, "equatorial"),
            (StormBandDto::MidLatitude, "mid_latitude"),
            (StormBandDto::Polar, "polar"),
        ]);
    }

    #[test]
    fn a_body_event_of_no_kind_or_a_malformed_id_is_refused() {
        for wire in [
            json!({ "type": "flare", "id": "0200080020000000:0404ffffffffce00" }),
            json!({
                "type": "eruption",
                "id": "0200080020000000.0300",
                "body": "0200080020000000.0300",
                "onset": { "seconds": 0, "nanos": 0 },
                "duration_s": 1.0,
            }),
        ] {
            let error = serde_json::from_value::<BodyEventDto>(wire.clone()).unwrap_err();
            assert!(
                error.to_string().contains("unknown variant `flare`")
                    || error
                        .to_string()
                        .contains("expected a system or body ID, a colon and 16"),
                "{wire}: {error}"
            );
        }
    }

    /// The shared fixture's populated answer holds the events pinned here, which the client's
    /// tests read (P14.T43.c).
    #[test]
    fn the_shared_events_fixture_holds_one_of_each_event() {
        let ServerMessage::Response {
            body: ResponseBody::BodyEvents(answer),
            ..
        } = server_message("body_events_populated")
        else {
            panic!("body_events_populated is not a body_events response");
        };
        assert_eq!(answer.events, one_of_each_event());
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
        assert_eq!(messages.len(), 16);
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
        // The fixture's Earth carries its condensates and its crust (P14.T54.a), which
        // `planet_record`'s surface, the later members' base, leaves absent.
        let mut record = planet_record();
        let SectionDto::Ok(surface) = &mut record.surface else {
            panic!("the slice's Earth's surface is ok");
        };
        surface.condensates = crate::Modelled::Value(earth_condensates());
        surface.crust = crate::Modelled::Value(earth_crust());
        assert_eq!(detail.record, record);
    }

    /// P14.T35.e: the shared Jupiter's record is the slice's list entry for it, its surface not
    /// applicable and its envelope `ok`, so that the client decodes an envelope the server is
    /// pinned to send. The envelope is read from the fixture itself (P14.T54.b.i), whose values
    /// `hyperion-server`'s `the_fixture_s_envelopes_are_the_generator_s` pins to the generator's:
    /// an H₂/He envelope's, `supercritical` `null`, on the generator's grey profile (κ 2.87 × 10⁻³
    /// m² kg⁻¹ at 1 bar, n 2), with an ammonia deck and the modes that name it, every deck mode
    /// naming one of its decks and no haze naming any, each over the whole sky.
    #[test]
    fn the_shared_jupiter_fixture_holds_its_envelope() {
        let ServerMessage::Response {
            body: ResponseBody::SystemBodies(system),
            ..
        } = server_message("system_bodies_response")
        else {
            panic!("system_bodies_response is not a system_bodies response");
        };
        let ServerMessage::Response {
            body: ResponseBody::BodyDetail(detail),
            ..
        } = server_message("body_detail_jupiter")
        else {
            panic!("body_detail_jupiter is not a body_detail response");
        };
        let listed = system
            .bodies
            .get(1)
            .expect("the slice lists its Jupiter")
            .clone();
        assert_eq!(
            detail.record,
            BodyRecordDto {
                id: listed.id,
                kind: listed.kind,
                label: listed.label,
                parent: listed.parent,
                state: listed.state,
                position_m: listed.position_m,
                mass_kg: listed.mass_kg,
                orbit: listed.orbit,
                moons: listed.moons,
                rings: listed.rings,
                population: listed.population,
                bulk: listed.bulk,
                rotation: listed.rotation,
                figure: listed.figure,
                photometry: listed.photometry,
                surface: SectionDto::NotApplicable,
                envelope: detail.record.envelope.clone(),
                hooks: SectionDto::NotModelled,
            }
        );
        let Some(SectionDto::Ok(envelope)) = detail.record.envelope else {
            panic!("the fixture's Jupiter's envelope is ok");
        };
        assert_eq!(envelope.supercritical, None);
        let profile = envelope.profile;
        assert!(
            (profile.thermal_opacity_m2_per_kg - 2.87e-3).abs() < 1e-15
                && (profile.optical_depth_exponent - 2.0).abs() < 1e-15,
            "{profile:?}"
        );
        assert!(
            envelope
                .condensate_decks
                .iter()
                .any(|deck| deck.condensate == "NH3"),
            "{:?}",
            envelope.condensate_decks
        );
        assert!(
            envelope
                .decks
                .iter()
                .any(|mode| mode.deck.as_deref() == Some("NH3")),
            "{:?}",
            envelope.decks
        );
        for mode in &envelope.decks {
            assert!(
                envelope
                    .condensate_decks
                    .iter()
                    .any(|deck| mode.deck.as_ref() == Some(&deck.condensate)),
                "{mode:?} names no listed deck"
            );
        }
        assert!(
            envelope.hazes.iter().all(|haze| haze.deck.is_none()),
            "{:?}",
            envelope.hazes
        );
        for mode in envelope.decks.iter().chain(&envelope.hazes) {
            assert!((mode.coverage - 1.0).abs() < 1e-15, "{mode:?}");
        }
    }

    /// P14.T35.e: the shared mass-and-orbit record withholds the envelope with the surface.
    #[test]
    fn the_shared_mass_and_orbit_fixture_withholds_both_sections() {
        let ServerMessage::Response {
            body: ResponseBody::BodyDetail(detail),
            ..
        } = server_message("body_detail_mass_and_orbit")
        else {
            panic!("body_detail_mass_and_orbit is not a body_detail response");
        };
        assert_eq!(detail.granted, DetailLevelDto::MassAndOrbit);
        assert_eq!(
            (detail.record.surface, detail.record.envelope),
            (SectionDto::NotResolved, Some(SectionDto::NotResolved))
        );
    }

    /// Decision-p14-t35e-wire §1.5 item 2: the shared contact record keeps the form of a server
    /// before P14.T35.e, with no `envelope` key.
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
                rotation: Some(SectionDto::NotResolved),
                figure: Some(SectionDto::NotResolved),
                photometry: Some(SectionDto::NotResolved),
                surface: SectionDto::NotResolved,
                envelope: None,
                hooks: SectionDto::NotResolved,
                ..earth
            }
        );
        assert!(
            fixture()["body_detail_contact"]["body"]["record"]
                .get("envelope")
                .is_none()
        );
        assert_eq!(contact.record.state, BodyStateDto::Present);
    }
}
