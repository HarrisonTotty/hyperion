//! Observation messages: the mode a query is asked in, what an observed row carries, and
//! `resolve_system`, one system by its ID (plan 12, P12.T6).
//!
//! A query in `now` describes the galaxy at the query's time. A query `observed` from a position
//! finds the same systems, on their present positions, and each row also carries what a sensor at
//! that position receives from it at the query's time: the light's emission time and age, where the
//! system was when the light left it, and the stated error from the curvature the straight line
//! neglects ([`ObservedDto`]). The mode is an optional field that reads as `now` when absent, so a
//! request written before plan 12 is unchanged.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::galaxy::SystemRecord;
use crate::primitives::{GalacticPosition, SystemIdHex, UniverseIdHex, UniverseTime};

/// The mode a query is asked in: the present, or what an observer receives (plan 12, Design note
/// 4).
///
/// Observed mode changes what is reported, never what is found: the systems, the census and the
/// positions are the present's in both.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum QueryModeDto {
    /// The state of the galaxy at the query's time.
    #[default]
    Now,
    /// The present as found, and with each system what an observer at `observer` receives from it
    /// at the query's time.
    Observed {
        /// Where the observer is, in the `GALACTIC` frame, inside the root cube.
        observer: GalacticPosition,
    },
}

impl QueryModeDto {
    /// Whether this is [`Now`](Self::Now), which the wire leaves out so that a request of the
    /// form before plan 12 is written as it was.
    #[must_use]
    pub(crate) const fn is_now(&self) -> bool {
        matches!(self, Self::Now)
    }
}

/// What an observer receives from one system: its light, and when and where that light left it
/// (plan 12, P12.T6).
///
/// Carried by a row only in observed mode. The row's own `position` stays the present one, since
/// the chart is the navigation computer's view of the present; this says where the system appears.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ObservedDto {
    /// When the light the observer receives at the query's time left the system. It can lie
    /// before the clock window, back to the source horizon, 1,000 years plus the root cube's light
    /// crossing time before the epoch.
    pub emitted: UniverseTime,
    /// The light's age, the query's time less `emitted`, in Julian years.
    pub light_age_yr: f64,
    /// Where the system was at `emitted`, in the `GALACTIC` frame: where the observer sees it.
    pub apparent_position: GalacticPosition,
    /// The stated bound on the apparent position's error from the neglected curvature of the
    /// system's galactic orbit, in light-years at the system; zero for an orbit followed in full.
    pub curvature_error_ly: f64,
    /// The same bound as the angle it subtends at the observer, in arcseconds.
    pub curvature_error_arcsec: f64,
}

/// Asks for one system by its ID at a time (`resolve_system`), answered with a
/// [`ResolvedSystem`].
///
/// The server resolves the ID first: a well-formed ID that names no system of the universe is
/// refused with `unknown_system` naming `system`, and a time outside the clock window with
/// `bad_request` naming `time`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResolveSystemRequest {
    /// The universe the system is in.
    pub universe: UniverseIdHex,
    /// The system.
    pub system: SystemIdHex,
    /// The instant at which it is placed and aged, within 1,000 years of the epoch.
    pub time: UniverseTime,
    /// The mode to answer in: `now` when absent, and written only when observed.
    #[serde(default, skip_serializing_if = "QueryModeDto::is_now")]
    #[ts(as = "Option<QueryModeDto>", optional)]
    pub mode: QueryModeDto,
}

/// One system at a time, as a range query's row describes it (`resolve_system`).
///
/// `universe` and `time` echo the request. The row is a range row without a brief: its position
/// and age at `time`, and in observed mode its `observed`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResolvedSystem {
    /// The universe the system is in.
    pub universe: UniverseIdHex,
    /// The instant at which it is described.
    pub time: UniverseTime,
    /// The system.
    pub record: SystemRecord,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::envelope::{RequestBody, ResponseBody};
    use crate::galaxy::{MassLayer, Population};
    use crate::testing::assert_wire_form;

    fn universe() -> UniverseIdHex {
        UniverseIdHex::from_u64(0x0123_4567_89ab_cdef)
    }

    fn system() -> SystemIdHex {
        SystemIdHex::from_u64(0x0200_0800_2000_0000)
    }

    fn a_century() -> UniverseTime {
        UniverseTime {
            seconds: 3_155_760_000,
            nanos: 0,
        }
    }

    /// An observer 3,000 ly coreward of the Sun-like point.
    fn observer() -> GalacticPosition {
        GalacticPosition {
            cell_ly: [0, 23_000, 0],
            offset_m: [0.0, 0.0, 0.0],
        }
    }

    fn observed() -> ObservedDto {
        ObservedDto {
            emitted: UniverseTime {
                seconds: -91_512_883_200,
                nanos: 250,
            },
            light_age_yr: 3_000.5,
            apparent_position: GalacticPosition {
                cell_ly: [26_011, -4, -2],
                offset_m: [2.5e15, 0.0, 9.0e15],
            },
            curvature_error_ly: 0.000_125,
            curvature_error_arcsec: 0.008_5,
        }
    }

    fn observed_json() -> serde_json::Value {
        json!({
            "emitted": { "seconds": -91_512_883_200_i64, "nanos": 250 },
            "light_age_yr": 3_000.5,
            "apparent_position": {
                "cell_ly": [26_011, -4, -2],
                "offset_m": [2.5e15, 0.0, 9.0e15],
            },
            "curvature_error_ly": 0.000_125,
            "curvature_error_arcsec": 0.008_5,
        })
    }

    #[test]
    fn query_mode_wire_forms() {
        assert_wire_form(&QueryModeDto::Now, json!({ "type": "now" }));
        assert_wire_form(
            &QueryModeDto::Observed {
                observer: observer(),
            },
            json!({
                "type": "observed",
                "observer": { "cell_ly": [0, 23_000, 0], "offset_m": [0.0, 0.0, 0.0] },
            }),
        );
        assert_eq!(QueryModeDto::default(), QueryModeDto::Now);
    }

    #[test]
    fn query_mode_refuses_an_unknown_type_and_an_observed_mode_without_its_observer() {
        let unknown =
            serde_json::from_value::<QueryModeDto>(json!({ "type": "then" })).unwrap_err();
        assert!(
            unknown.to_string().contains("unknown variant `then`"),
            "unexpected error: {unknown}"
        );
        let bare =
            serde_json::from_value::<QueryModeDto>(json!({ "type": "observed" })).unwrap_err();
        assert!(
            bare.to_string().contains("missing field `observer`"),
            "unexpected error: {bare}"
        );
    }

    #[test]
    fn observed_wire_form() {
        assert_wire_form(&observed(), observed_json());
    }

    #[test]
    fn resolve_system_request_wire_form() {
        assert_wire_form(
            &RequestBody::ResolveSystem(ResolveSystemRequest {
                universe: universe(),
                system: system(),
                time: a_century(),
                mode: QueryModeDto::Now,
            }),
            json!({
                "kind": "resolve_system",
                "universe": "0123456789abcdef",
                "system": "0200080020000000",
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
            }),
        );
    }

    #[test]
    fn resolve_system_request_wire_form_observed() {
        assert_wire_form(
            &RequestBody::ResolveSystem(ResolveSystemRequest {
                universe: universe(),
                system: system(),
                time: a_century(),
                mode: QueryModeDto::Observed {
                    observer: observer(),
                },
            }),
            json!({
                "kind": "resolve_system",
                "universe": "0123456789abcdef",
                "system": "0200080020000000",
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "mode": {
                    "type": "observed",
                    "observer": { "cell_ly": [0, 23_000, 0], "offset_m": [0.0, 0.0, 0.0] },
                },
            }),
        );
    }

    #[test]
    fn resolve_system_request_reads_an_explicit_now() {
        let body: RequestBody = serde_json::from_value(json!({
            "kind": "resolve_system",
            "universe": "0123456789abcdef",
            "system": "0200080020000000",
            "time": { "seconds": 0, "nanos": 0 },
            "mode": { "type": "now" },
        }))
        .unwrap();
        match body {
            RequestBody::ResolveSystem(request) => assert_eq!(request.mode, QueryModeDto::Now),
            other => panic!("expected a resolve request, got {other:?}"),
        }
    }

    #[test]
    fn resolve_system_response_wire_form() {
        assert_wire_form(
            &ResponseBody::ResolveSystem(Box::new(ResolvedSystem {
                universe: universe(),
                time: a_century(),
                record: SystemRecord {
                    id: system(),
                    designation: "Vorth AB-C e4-17".to_owned(),
                    position: GalacticPosition {
                        cell_ly: [26_012, -3, -2],
                        offset_m: [1.5e15, 0.0, 9.0e15],
                    },
                    layer: MassLayer::E,
                    initial_mass_msun: 11.25,
                    age_myr: 7_250.5,
                    population: Population::OldThinDisc,
                    velocity_km_s: [-12.5, 231.25, 7.0],
                    stellar: None,
                    fe_h_dex: None,
                    observed: Some(observed()),
                },
            })),
            json!({
                "kind": "resolve_system",
                "universe": "0123456789abcdef",
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "record": {
                    "id": "0200080020000000",
                    "designation": "Vorth AB-C e4-17",
                    "position": {
                        "cell_ly": [26_012, -3, -2],
                        "offset_m": [1.5e15, 0.0, 9.0e15],
                    },
                    "layer": "e",
                    "initial_mass_msun": 11.25,
                    "age_myr": 7_250.5,
                    "population": "old_thin_disc",
                    "velocity_km_s": [-12.5, 231.25, 7.0],
                    "observed": observed_json(),
                },
            }),
        );
    }
}
