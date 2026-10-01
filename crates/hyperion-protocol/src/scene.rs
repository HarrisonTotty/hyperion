//! The scene's wire types (rendering plan R03, R03.T4): what a client subscribed to a universe's
//! scene receives, the ship stand-in and scene clock a client sets, and the cameras each view
//! reports.
//!
//! A subscription is answered with the whole [`SceneStateDto`]; each [`SceneNotificationDto`]
//! carries only what changed, and always the clock and a `sequence` that grows by exactly one per
//! notification sent (R03, Design notes 4 and 5). In a system the scene holds plan 14's
//! [`SystemBodiesDto`] built at the level asked, with each body's record degraded to its own grant
//! and the grants beside it ([`SceneSystemDto`], Design note 13). The kinds that carry these types
//! (`subscribe`'s scene topic, `scene_ship`, `scene_cameras`) are added by R03.T5.a.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::planetary::{BodySummaryDto, DetailLevelDto, SystemBodiesDto};
use crate::primitives::{BodyIdHex, GalacticPosition, SystemIdHex, UniverseIdHex, UniverseTime};

/// A position in one of plan 01's frames, tagged by `frame`.
///
/// A system frame's origin is the system's barycentre and a body frame's the body's centre; both
/// are non-rotating, along the galactic axes, in metres. A body's rotating, body-fixed frame is
/// not a wire frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "frame", rename_all = "snake_case")]
#[ts(export)]
pub enum FramePositionDto {
    /// A position in the galactic frame.
    Galactic {
        /// The position, exactly as a light-year cell plus a metre offset.
        position: GalacticPosition,
    },
    /// A position in a system's frame.
    System {
        /// The system.
        system: SystemIdHex,
        /// Metres from the system's barycentre, along the galactic axes.
        offset_m: [f64; 3],
    },
    /// A position in a body's non-rotating frame.
    Body {
        /// The body.
        body: BodyIdHex,
        /// Metres from the body's centre, along the galactic axes.
        offset_m: [f64; 3],
    },
}

/// A pose without attitude: a position in a frame, the velocity in that frame and the time both
/// hold at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KinematicsDto {
    /// Where.
    pub position: FramePositionDto,
    /// The velocity relative to the frame's origin, m/s along the galactic axes.
    pub velocity_m_s: [f64; 3],
    /// When.
    pub time: UniverseTime,
}

/// Whether the scene clock runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SceneClockStateDto {
    /// It advances at its rate.
    Running,
    /// Its rate is zero.
    Paused,
    /// It has reached the clock window's edge, ±1,000 Julian years about the epoch, and holds
    /// there.
    WindowLimit,
}

/// The scene clock as of a push: a client renders at `time` plus its own elapsed time since the
/// push arrived times `time_rate`, unless the clock is not running (R03, Design notes 2 and 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneClockDto {
    /// The scene time when the push was made.
    pub time: UniverseTime,
    /// Scene seconds per real second: 0 (paused) or a power of ten from 1 to 100,000 (the
    /// single-player brainstorm's The clock).
    pub time_rate: u32,
    /// Whether the clock runs.
    pub state: SceneClockStateDto,
}

/// One view's camera, as the client reports it: which view and where its camera is (R03, Design
/// note 6). Cameras bound the scene; they are never ship state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CameraReportDto {
    /// The view's number on this client, which the latest report for it replaces.
    pub view: u8,
    /// The camera's position, velocity and time.
    pub pose: KinematicsDto,
}

/// The scene topic of a `subscribe` request: the detail level asked for every body, and the
/// cameras of the client's views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneSubscribeRequest {
    /// The detail level asked for; each body's grant may be lower.
    pub detail: DetailLevelDto,
    /// The views' cameras, at most eight.
    pub cameras: Vec<CameraReportDto>,
}

/// Sets a universe's ship stand-in and scene clock (`scene_ship`), until sessions exist (R03,
/// Design note 2). The last one the server accepts stands, and every subscription of the universe
/// is pushed the change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneShipRequest {
    /// The universe.
    pub universe: UniverseIdHex,
    /// The ship's pose; its time becomes the scene time.
    pub ship: KinematicsDto,
    /// The clock's rate: 0 or a power of ten from 1 to 100,000.
    pub time_rate: u32,
}

/// Replaces a scene subscription's cameras (`scene_cameras`), answered with an empty body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneCamerasRequest {
    /// The subscription, as `subscribe` numbered it.
    pub subscription: u32,
    /// The views' cameras, at most eight.
    pub cameras: Vec<CameraReportDto>,
}

/// The answer to `scene_ship`: the clock it set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneShipSet {
    /// The scene clock as set.
    pub clock: SceneClockDto,
}

/// Where the ship sees a body it knows only as a contact, which the client cannot propagate: the
/// apparent position the server evaluated from the ship at the push's time (R03, Design note 13).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SeenPositionDto {
    /// The apparent position, light time and aberration together, in metres from the system's
    /// barycentre along the galactic axes.
    pub apparent_m: [f64; 3],
    /// When the light the ship sees left the body.
    pub emitted: UniverseTime,
}

/// The detail level granted for one body of a scene's system, and for a contact its seen position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyGrantDto {
    /// The body.
    pub body: BodyIdHex,
    /// The level its record is degraded to.
    pub level: DetailLevelDto,
    /// Where the ship sees it, for a body granted only `contact`; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub seen: Option<SeenPositionDto>,
}

/// A scene's system: plan 14's answer at the level asked, each record degraded to its own grant,
/// and the grants in index order (R03, Design note 13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneSystemDto {
    /// The system's bodies; its `granted` is the level asked, the most any record holds.
    pub system: SystemBodiesDto,
    /// One grant per body of `system.bodies`, in the same order.
    pub grants: Vec<BodyGrantDto>,
}

/// A body re-sent by a notification: its record at its grant, which supersedes every earlier one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneBodyDto {
    /// The level granted.
    pub level: DetailLevelDto,
    /// The body's record at that level.
    pub record: BodySummaryDto,
    /// Where the ship sees it, for a body granted only `contact`; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub seen: Option<SeenPositionDto>,
}

/// A change of the scene's system, tagged by `type`: the ship arrived in a system or left it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum SceneArrivalDto {
    /// The ship is in this system's frame now.
    System {
        /// The whole system, which replaces everything before it.
        system: Box<SceneSystemDto>,
        /// The radius of the system's sphere in the galaxy's tide at the arrival time, m (plan
        /// 03's `FrameCandidate::tidal_radius`), to which a free camera is held.
        tidal_radius_m: f64,
    },
    /// The ship is in the galactic frame: the scene holds no system.
    NoSystem,
}

/// A craft in the scene, the least a renderer needs of it (R03, Design note 4).
///
/// A draft: it belongs to the sessions plan, which may reshape it freely while nothing sends it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneCraftDto {
    /// The craft's identity.
    pub craft: String,
    /// The key of its hull definition.
    pub hull: String,
    /// Its position, velocity and time.
    pub state: KinematicsDto,
    /// Its attitude as a unit quaternion (x, y, z, w), body to frame axes.
    pub attitude: [f64; 4],
    /// Its angular velocity, rad/s about the frame's axes.
    pub angular_velocity_rad_s: [f64; 3],
    /// The flight computer's predicted path as poses at stated times, when one is planned; a
    /// client extrapolates in a straight line without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub planned_path: Option<Vec<KinematicsDto>>,
}

/// The whole scene, which answers a scene subscription.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneStateDto {
    /// The sequence number of this state; the first notification carries the next.
    ///
    /// A JSON number, not the crate's hexadecimal for a `u64`: it grows by one a push, and at 64
    /// pushes a second passes 2⁵³, what a JavaScript number holds exactly, only after some 4 × 10⁶
    /// years, while the client checks it arithmetically.
    #[ts(type = "number")]
    pub sequence: u64,
    /// The scene clock.
    pub clock: SceneClockDto,
    /// The ship (its stand-in until sessions exist).
    pub ship: KinematicsDto,
    /// The system the ship is in, or `null` in the galactic frame.
    pub system: Option<SceneSystemDto>,
    /// The system's sphere of influence at the state's time, metres: its tidal radius, as an
    /// arrival's `tidal_radius_m` states it, to which R02's free camera is clamped. Present
    /// whenever `system` is, so that a client subscribing inside a system has it; omitted in the
    /// galactic frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tidal_radius_m: Option<f64>,
    /// The craft that are the ship's contacts in the scene's reach.
    pub craft: Vec<SceneCraftDto>,
}

/// What changed in a scene since the last push. Every notification carries the clock; the other
/// fields are present only when they changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SceneNotificationDto {
    /// One more than the previous notification's (or the state's): a gap or a step back is a
    /// server bug. A JSON number, as [`SceneStateDto::sequence`] is.
    #[ts(type = "number")]
    pub sequence: u64,
    /// The scene clock.
    pub clock: SceneClockDto,
    /// The ship, when it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ship: Option<KinematicsDto>,
    /// An arrival in a system or a departure from it, which replaces everything before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub arrival: Option<SceneArrivalDto>,
    /// Bodies re-sent, each superseding its earlier record.
    pub bodies: Vec<SceneBodyDto>,
    /// The whole craft list, when it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub craft: Option<Vec<SceneCraftDto>>,
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::planetary::SectionDto;
    use crate::planetary::record_fixtures::{SYSTEM, planet_summary, planet_summary_json};
    use crate::planetary::requests_fixtures::{single_star_hosts, single_star_hosts_json};
    use crate::testing::{assert_wire_form, assert_wire_strings};

    fn time() -> UniverseTime {
        UniverseTime {
            seconds: 86_400,
            nanos: 250,
        }
    }

    fn time_json() -> Value {
        json!({ "seconds": 86_400, "nanos": 250 })
    }

    fn in_system() -> KinematicsDto {
        KinematicsDto {
            position: FramePositionDto::System {
                system: SystemIdHex::from_u64(SYSTEM),
                offset_m: [1.5e11, -2.0e9, 0.5],
            },
            velocity_m_s: [0.0, 29_780.0, -1.25],
            time: time(),
        }
    }

    fn in_system_json() -> Value {
        json!({
            "position": {
                "frame": "system",
                "system": "0200080020000000",
                "offset_m": [1.5e11, -2.0e9, 0.5],
            },
            "velocity_m_s": [0.0, 29_780.0, -1.25],
            "time": time_json(),
        })
    }

    fn clock() -> SceneClockDto {
        SceneClockDto {
            time: time(),
            time_rate: 1_000,
            state: SceneClockStateDto::Running,
        }
    }

    fn clock_json() -> Value {
        json!({ "time": time_json(), "time_rate": 1_000, "state": "running" })
    }

    fn seen() -> SeenPositionDto {
        SeenPositionDto {
            apparent_m: [1.0e11, 2.0e10, -3.0e8],
            emitted: UniverseTime {
                seconds: 86_000,
                nanos: 7,
            },
        }
    }

    fn seen_json() -> Value {
        json!({
            "apparent_m": [1.0e11, 2.0e10, -3.0e8],
            "emitted": { "seconds": 86_000, "nanos": 7 },
        })
    }

    fn scene_system() -> SceneSystemDto {
        SceneSystemDto {
            system: SystemBodiesDto {
                granted: DetailLevelDto::Full,
                hosts: single_star_hosts(),
                zones: Vec::new(),
                system_plane: None,
                belts: SectionDto::Ok(Vec::new()),
                halo: SectionDto::Ok(None),
                bodies: vec![planet_summary()],
            },
            grants: vec![BodyGrantDto {
                body: planet_summary().id,
                level: DetailLevelDto::Full,
                seen: None,
            }],
        }
    }

    fn scene_system_json() -> Value {
        json!({
            "system": {
                "granted": "full",
                "hosts": single_star_hosts_json(),
                "zones": [],
                "system_plane": null,
                "belts": { "state": "ok", "value": [] },
                "halo": { "state": "ok", "value": null },
                "bodies": [planet_summary_json()],
            },
            "grants": [{ "body": "0200080020000000.0300", "level": "full" }],
        })
    }

    fn craft() -> SceneCraftDto {
        SceneCraftDto {
            craft: "test-1".to_owned(),
            hull: "corvette".to_owned(),
            state: in_system(),
            attitude: [0.0, 0.0, 0.0, 1.0],
            angular_velocity_rad_s: [0.0, 0.01, 0.0],
            planned_path: None,
        }
    }

    fn craft_json() -> Value {
        json!({
            "craft": "test-1",
            "hull": "corvette",
            "state": in_system_json(),
            "attitude": [0.0, 0.0, 0.0, 1.0],
            "angular_velocity_rad_s": [0.0, 0.01, 0.0],
        })
    }

    #[test]
    fn a_frame_position_is_tagged_by_its_frame() {
        assert_wire_form(
            &FramePositionDto::Galactic {
                position: GalacticPosition {
                    cell_ly: [0, 26_000, -3],
                    offset_m: [1.0, 2.0, 3.0],
                },
            },
            json!({
                "frame": "galactic",
                "position": { "cell_ly": [0, 26_000, -3], "offset_m": [1.0, 2.0, 3.0] },
            }),
        );
        assert_wire_form(
            &FramePositionDto::Body {
                body: BodyIdHex::from_parts(SYSTEM, 0x0300),
                offset_m: [6.8e6, 0.0, 0.0],
            },
            json!({ "frame": "body", "body": "0200080020000000.0300", "offset_m": [6.8e6, 0.0, 0.0] }),
        );
    }

    #[test]
    fn kinematics_wire_form() {
        assert_wire_form(&in_system(), in_system_json());
    }

    #[test]
    fn the_clock_states_its_time_rate_and_state() {
        assert_wire_form(&clock(), clock_json());
        assert_wire_strings(&[
            (SceneClockStateDto::Running, "running"),
            (SceneClockStateDto::Paused, "paused"),
            (SceneClockStateDto::WindowLimit, "window_limit"),
        ]);
    }

    #[test]
    fn a_camera_report_names_its_view() {
        assert_wire_form(
            &CameraReportDto {
                view: 3,
                pose: in_system(),
            },
            json!({ "view": 3, "pose": in_system_json() }),
        );
    }

    #[test]
    fn scene_requests_wire_forms() {
        assert_wire_form(
            &SceneSubscribeRequest {
                detail: DetailLevelDto::Bulk,
                cameras: vec![CameraReportDto {
                    view: 0,
                    pose: in_system(),
                }],
            },
            json!({ "detail": "bulk", "cameras": [{ "view": 0, "pose": in_system_json() }] }),
        );
        assert_wire_form(
            &SceneShipRequest {
                universe: UniverseIdHex::from_u64(42),
                ship: in_system(),
                time_rate: 100_000,
            },
            json!({
                "universe": "000000000000002a",
                "ship": in_system_json(),
                "time_rate": 100_000,
            }),
        );
        assert_wire_form(
            &SceneCamerasRequest {
                subscription: 1,
                cameras: Vec::new(),
            },
            json!({ "subscription": 1, "cameras": [] }),
        );
        assert_wire_form(
            &SceneShipSet { clock: clock() },
            json!({ "clock": clock_json() }),
        );
    }

    #[test]
    fn a_grant_carries_a_seen_position_only_for_a_contact() {
        assert_wire_form(
            &BodyGrantDto {
                body: BodyIdHex::from_parts(SYSTEM, 0x0300),
                level: DetailLevelDto::Contact,
                seen: Some(seen()),
            },
            json!({ "body": "0200080020000000.0300", "level": "contact", "seen": seen_json() }),
        );
        assert_wire_form(
            &BodyGrantDto {
                body: BodyIdHex::from_parts(SYSTEM, 0x0300),
                level: DetailLevelDto::MassAndOrbit,
                seen: None,
            },
            json!({ "body": "0200080020000000.0300", "level": "mass_and_orbit" }),
        );
    }

    #[test]
    fn a_scene_system_is_plan_14_s_answer_with_the_grants() {
        assert_wire_form(&scene_system(), scene_system_json());
    }

    #[test]
    fn a_re_sent_body_states_its_level_and_omits_an_absent_seen_position() {
        assert_wire_form(
            &SceneBodyDto {
                level: DetailLevelDto::Full,
                record: planet_summary(),
                seen: None,
            },
            json!({ "level": "full", "record": planet_summary_json() }),
        );
        assert_wire_form(
            &SceneBodyDto {
                level: DetailLevelDto::Full,
                record: planet_summary(),
                seen: Some(seen()),
            },
            json!({ "level": "full", "record": planet_summary_json(), "seen": seen_json() }),
        );
    }

    #[test]
    fn an_arrival_is_tagged_by_its_type() {
        assert_wire_form(
            &SceneArrivalDto::System {
                system: Box::new(scene_system()),
                tidal_radius_m: 1.2e16,
            },
            json!({ "type": "system", "system": scene_system_json(), "tidal_radius_m": 1.2e16 }),
        );
        assert_wire_form(&SceneArrivalDto::NoSystem, json!({ "type": "no_system" }));
    }

    #[test]
    fn a_craft_omits_an_absent_planned_path() {
        assert_wire_form(&craft(), craft_json());
        let mut planned = craft_json();
        planned["planned_path"] = json!([in_system_json()]);
        assert_wire_form(
            &SceneCraftDto {
                planned_path: Some(vec![in_system()]),
                ..craft()
            },
            planned,
        );
    }

    #[test]
    fn scene_state_wire_form() {
        assert_wire_form(
            &SceneStateDto {
                sequence: 0,
                clock: clock(),
                ship: in_system(),
                system: Some(scene_system()),
                tidal_radius_m: Some(1.5e16),
                craft: vec![craft()],
            },
            json!({
                "sequence": 0,
                "clock": clock_json(),
                "ship": in_system_json(),
                "system": scene_system_json(),
                "tidal_radius_m": 1.5e16,
                "craft": [craft_json()],
            }),
        );
        assert_wire_form(
            &SceneStateDto {
                sequence: 7,
                clock: clock(),
                ship: in_system(),
                system: None,
                tidal_radius_m: None,
                craft: Vec::new(),
            },
            json!({
                "sequence": 7,
                "clock": clock_json(),
                "ship": in_system_json(),
                "system": null,
                "craft": [],
            }),
        );
    }

    #[test]
    fn a_heartbeat_carries_only_the_clock_and_its_sequence() {
        assert_wire_form(
            &SceneNotificationDto {
                sequence: 12,
                clock: clock(),
                ship: None,
                arrival: None,
                bodies: Vec::new(),
                craft: None,
            },
            json!({ "sequence": 12, "clock": clock_json(), "bodies": [] }),
        );
    }

    #[test]
    fn a_notification_carries_what_changed() {
        assert_wire_form(
            &SceneNotificationDto {
                sequence: 13,
                clock: clock(),
                ship: Some(in_system()),
                arrival: Some(SceneArrivalDto::NoSystem),
                bodies: vec![SceneBodyDto {
                    level: DetailLevelDto::Full,
                    record: planet_summary(),
                    seen: None,
                }],
                craft: Some(Vec::new()),
            },
            json!({
                "sequence": 13,
                "clock": clock_json(),
                "ship": in_system_json(),
                "arrival": { "type": "no_system" },
                "bodies": [{ "level": "full", "record": planet_summary_json() }],
                "craft": [],
            }),
        );
    }
}
