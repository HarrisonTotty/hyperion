//! The message envelope: the top-level messages, and the request convention built on them.
//!
//! Every frame is a JSON object with a `type`. Besides `hello`, `ping` and their answers, a client
//! makes requests: `{"type":"request","id":7,"body":{"kind":…}}`. The ID is the client's own, a
//! counter per connection. Every request the server accepts ends in exactly one terminal message
//! with the same ID, a `response` whose body has the request's `kind`, or a `request_error`, and
//! that holds for a cancelled request too. The client may reuse an ID once its terminal message has
//! arrived.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::galaxy::{
    DensityMap, DensityMapRequest, GalaxyParameters, GalaxyParametersRequest, SystemsInRange,
    SystemsInRangeRequest,
};
use crate::stellar::{SystemSummaryDto, SystemSummaryRequest};
use crate::universe::{CreateUniverseRequest, OpenUniverseRequest, UniverseInfo, UniverseList};

/// A request's ID, chosen by the client and unique among its requests in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct RequestId(pub u32);

/// Messages sent from a client to the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ClientMessage {
    /// First message on a new connection. Requests are refused until it has been sent.
    Hello {
        /// The client's version, for the server's log.
        client_version: String,
    },
    /// Liveness and latency probe, allowed at any time; the server echoes `nonce` back in a
    /// [`ServerMessage::Pong`].
    Ping {
        /// Echoed back unchanged.
        nonce: u32,
    },
    /// A request, answered by exactly one [`ServerMessage::Response`] or
    /// [`ServerMessage::RequestError`] with the same `id`.
    Request {
        /// The client's ID for the request, not in flight already.
        id: RequestId,
        /// What is asked.
        body: RequestBody,
    },
    /// Cancels the request with this ID. It still ends with one terminal message, `cancelled`
    /// unless its response was already on its way.
    Cancel {
        /// The request to cancel.
        id: RequestId,
    },
}

/// Messages sent from the server to a client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ServerMessage {
    /// Reply to [`ClientMessage::Hello`].
    Welcome {
        /// The server's version.
        server_version: String,
        /// The server's [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION). A client that speaks
        /// another version cannot use this server.
        protocol_version: u32,
        /// The generator version the server runs, which new universes are created with.
        generator_version: u32,
    },
    /// Reply to [`ClientMessage::Ping`].
    Pong {
        /// The ping's nonce.
        nonce: u32,
    },
    /// The client sent a frame that is not a message the server could attribute to a request.
    Error {
        /// What was wrong, for the client's log.
        message: String,
    },
    /// The successful end of a request.
    Response {
        /// The request's ID.
        id: RequestId,
        /// The answer, whose `kind` is the request's.
        body: ResponseBody,
    },
    /// The unsuccessful end of a request.
    RequestError {
        /// The request's ID.
        id: RequestId,
        /// Why it failed.
        error: RequestError,
    },
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RequestError {
    /// What kind of failure it was, for the client to act on.
    pub code: ErrorCode,
    /// A description for the operator or the log.
    pub message: String,
    /// The request field at fault, such as `name` or `centre`, when one is.
    pub field: Option<String>,
}

/// The kind of a request's failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ErrorCode {
    /// The request is malformed or a field is out of range; `field` names it where it can.
    BadRequest,
    /// The server does not know the request's kind.
    Unsupported,
    /// A request arrived before `hello`.
    HelloRequired,
    /// No universe has the given ID.
    UnknownUniverse,
    /// A well-formed system ID that names no system of the universe; `field` names the request's
    /// field.
    UnknownSystem,
    /// A well-formed body ID that names no body of its system; `field` names the request's field.
    UnknownBody,
    /// The universe was created with a generator version this server cannot run.
    GeneratorVersionMismatch,
    /// The universe's save is in a format this server cannot read.
    UnsupportedSaveFormat,
    /// Another universe already has that name, without regard to case.
    NameTaken,
    /// The server holds as many universes as it allows.
    UniverseLimitReached,
    /// The connection has as many requests in flight as it may.
    TooManyRequests,
    /// The server's work queue is full; the request may be retried later.
    QueueFull,
    /// The request was cancelled by the client.
    Cancelled,
    /// The server could not write to its data directory.
    StorageFailed,
    /// The server failed; the fault is its own.
    Internal,
}

/// What a request asks for, discriminated by `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum RequestBody {
    /// Create a universe from a given or drawn seed.
    CreateUniverse(CreateUniverseRequest),
    /// List every universe the server holds.
    ListUniverses,
    /// Check, load and warm one universe.
    OpenUniverse(OpenUniverseRequest),
    /// A universe's galaxy parameters.
    GalaxyParameters(GalaxyParametersRequest),
    /// A column-density map of a universe's galaxy.
    DensityMap(DensityMapRequest),
    /// The systems within range of a point at a time.
    SystemsInRange(SystemsInRangeRequest),
    /// Every star of one system at a time.
    SystemSummary(SystemSummaryRequest),
}

/// The answer to a request, with the same `kind` as the request it answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum ResponseBody {
    /// The universe created.
    CreateUniverse(UniverseInfo),
    /// Every universe the server holds.
    ListUniverses(UniverseList),
    /// The universe opened.
    OpenUniverse(UniverseInfo),
    /// The galaxy's parameters.
    GalaxyParameters(GalaxyParameters),
    /// The density map.
    DensityMap(DensityMap),
    /// The systems found, with their census.
    SystemsInRange(SystemsInRange),
    /// The system's stars and the orbits that hold them together.
    SystemSummary(SystemSummaryDto),
}

/// The `kind` string of every [`RequestBody`] variant, which is also that of the
/// [`ResponseBody`] variant answering it.
///
/// A server that cannot parse a request frame looks its `kind` up here to choose between
/// `unsupported`, for a kind it does not know, and `bad_request`.
pub const REQUEST_KINDS: &[&str] = &[
    "create_universe",
    "list_universes",
    "open_universe",
    "galaxy_parameters",
    "density_map",
    "systems_in_range",
    "system_summary",
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::{Value, json};

    use super::*;
    use crate::galaxy::{
        Census, MapPopulation, MapView, MassLayer, ParameterGroup, SystemsInRange,
    };
    use crate::orbit::HierarchyDto;
    use crate::primitives::{GalacticPosition, SeedHex, SystemIdHex, UniverseIdHex, UniverseTime};
    use crate::stellar::SystemExistenceDto;
    use crate::testing::{assert_wire_form, assert_wire_strings};
    use crate::universe::UniverseStatus;

    fn universe() -> UniverseIdHex {
        UniverseIdHex::from_u64(42)
    }

    fn universe_info() -> UniverseInfo {
        UniverseInfo {
            id: universe(),
            name: "Talos".to_owned(),
            seed: SeedHex::from_u64(1234),
            generator_version: 2,
            status: UniverseStatus::Compatible,
        }
    }

    /// The request that follows `previous` in a walk over every variant, starting from `None`.
    ///
    /// The match has no wildcard, so a new variant fails to compile here until it joins the walk.
    fn next_request(previous: Option<&RequestBody>) -> Option<RequestBody> {
        let universe = universe();
        match previous {
            None => Some(RequestBody::CreateUniverse(CreateUniverseRequest {
                name: "Talos".to_owned(),
                seed: None,
            })),
            Some(RequestBody::CreateUniverse(_)) => Some(RequestBody::ListUniverses),
            Some(RequestBody::ListUniverses) => {
                Some(RequestBody::OpenUniverse(OpenUniverseRequest { universe }))
            }
            Some(RequestBody::OpenUniverse(_)) => {
                Some(RequestBody::GalaxyParameters(GalaxyParametersRequest {
                    universe,
                }))
            }
            Some(RequestBody::GalaxyParameters(_)) => {
                Some(RequestBody::DensityMap(DensityMapRequest {
                    universe,
                    view: MapView::FaceOn,
                    population: MapPopulation::All,
                    resolution: 128,
                    bits: 8,
                }))
            }
            Some(RequestBody::DensityMap(_)) => {
                Some(RequestBody::SystemsInRange(SystemsInRangeRequest {
                    universe,
                    centre: GalacticPosition::default(),
                    radius_ly: 50.0,
                    time: UniverseTime::default(),
                    min_layer: MassLayer::A,
                    limit: 5_000,
                    include_stellar: false,
                }))
            }
            Some(RequestBody::SystemsInRange(_)) => {
                Some(RequestBody::SystemSummary(SystemSummaryRequest {
                    universe,
                    system: SystemIdHex::from_u64(0x0200_0800_2000_0000),
                    time: UniverseTime::default(),
                }))
            }
            Some(RequestBody::SystemSummary(_)) => None,
        }
    }

    /// The response that follows `previous` in a walk over every variant, as for requests.
    fn next_response(previous: Option<&ResponseBody>) -> Option<ResponseBody> {
        let universe = universe();
        match previous {
            None => Some(ResponseBody::CreateUniverse(universe_info())),
            Some(ResponseBody::CreateUniverse(_)) => {
                Some(ResponseBody::ListUniverses(UniverseList {
                    universes: Vec::new(),
                    server_generator_version: 2,
                }))
            }
            Some(ResponseBody::ListUniverses(_)) => {
                Some(ResponseBody::OpenUniverse(universe_info()))
            }
            Some(ResponseBody::OpenUniverse(_)) => {
                Some(ResponseBody::GalaxyParameters(GalaxyParameters {
                    universe,
                    seed: SeedHex::from_u64(1234),
                    generator_version: 2,
                    groups: vec![ParameterGroup {
                        key: "mass".to_owned(),
                        parameters: Vec::new(),
                    }],
                }))
            }
            Some(ResponseBody::GalaxyParameters(_)) => Some(ResponseBody::DensityMap(DensityMap {
                universe,
                view: MapView::EdgeOn,
                population: MapPopulation::Young,
                width_px: 2,
                height_px: 1,
                centre_ly: [0.0, 0.0],
                ly_per_px: 65_536.0,
                bits: 8,
                floor_log10_per_ly2: 0.0,
                ceiling_log10_per_ly2: 0.0,
                data_base64: "AAA=".to_owned(),
            })),
            Some(ResponseBody::DensityMap(_)) => {
                Some(ResponseBody::SystemsInRange(SystemsInRange {
                    universe,
                    centre: GalacticPosition::default(),
                    radius_ly: 50.0,
                    time: UniverseTime::default(),
                    census: Census {
                        limit: 5_000,
                        complete_above_msun: None,
                        layers: Vec::new(),
                    },
                    systems: Vec::new(),
                }))
            }
            Some(ResponseBody::SystemsInRange(_)) => {
                Some(ResponseBody::SystemSummary(SystemSummaryDto {
                    universe,
                    system: SystemIdHex::from_u64(0x0200_0800_2000_0000),
                    time: UniverseTime::default(),
                    existence: SystemExistenceDto::NotYetBorn,
                    age_myr: -0.5,
                    fe_h_dex: 0.0,
                    stars: Vec::new(),
                    hierarchy: HierarchyDto { nodes: Vec::new() },
                }))
            }
            Some(ResponseBody::SystemSummary(_)) => None,
        }
    }

    /// Walks `next` from `None` to the end, collecting every value.
    fn walk<T>(next: impl Fn(Option<&T>) -> Option<T>) -> Vec<T> {
        let mut all: Vec<T> = Vec::new();
        while let Some(value) = next(all.last()) {
            all.push(value);
        }
        all
    }

    /// The `kind` field of a serialized body.
    fn kind_of(body: &impl Serialize) -> String {
        match serde_json::to_value(body).unwrap().get("kind") {
            Some(Value::String(kind)) => kind.clone(),
            other => panic!("body has no string kind: {other:?}"),
        }
    }

    #[test]
    fn hello_wire_form() {
        assert_wire_form(
            &ClientMessage::Hello {
                client_version: "0.1.0".to_owned(),
            },
            json!({ "type": "hello", "client_version": "0.1.0" }),
        );
    }

    #[test]
    fn ping_wire_form() {
        assert_wire_form(
            &ClientMessage::Ping { nonce: 7 },
            json!({ "type": "ping", "nonce": 7 }),
        );
    }

    #[test]
    fn welcome_wire_form() {
        assert_wire_form(
            &ServerMessage::Welcome {
                server_version: "0.1.0".to_owned(),
                protocol_version: 2,
                generator_version: 3,
            },
            json!({
                "type": "welcome",
                "server_version": "0.1.0",
                "protocol_version": 2,
                "generator_version": 3,
            }),
        );
    }

    #[test]
    fn pong_wire_form() {
        assert_wire_form(
            &ServerMessage::Pong { nonce: 7 },
            json!({ "type": "pong", "nonce": 7 }),
        );
    }

    #[test]
    fn error_wire_form() {
        assert_wire_form(
            &ServerMessage::Error {
                message: "malformed message".to_owned(),
            },
            json!({ "type": "error", "message": "malformed message" }),
        );
    }

    #[test]
    fn unknown_message_type_is_rejected() {
        let error = serde_json::from_value::<ClientMessage>(json!({ "type": "warp" })).unwrap_err();
        assert!(
            error.to_string().contains("unknown variant `warp`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn request_wire_form() {
        assert_wire_form(
            &ClientMessage::Request {
                id: RequestId(7),
                body: RequestBody::OpenUniverse(OpenUniverseRequest {
                    universe: universe(),
                }),
            },
            json!({
                "type": "request",
                "id": 7,
                "body": { "kind": "open_universe", "universe": "000000000000002a" },
            }),
        );
    }

    #[test]
    fn request_id_is_a_bare_number() {
        assert_wire_form(&RequestId(u32::MAX), json!(4_294_967_295_u32));
    }

    #[test]
    fn cancel_wire_form() {
        assert_wire_form(
            &ClientMessage::Cancel { id: RequestId(7) },
            json!({ "type": "cancel", "id": 7 }),
        );
    }

    #[test]
    fn response_wire_form() {
        assert_wire_form(
            &ServerMessage::Response {
                id: RequestId(7),
                body: ResponseBody::ListUniverses(UniverseList {
                    universes: Vec::new(),
                    server_generator_version: 2,
                }),
            },
            json!({
                "type": "response",
                "id": 7,
                "body": {
                    "kind": "list_universes",
                    "universes": [],
                    "server_generator_version": 2,
                },
            }),
        );
    }

    #[test]
    fn request_error_wire_form() {
        assert_wire_form(
            &ServerMessage::RequestError {
                id: RequestId(7),
                error: RequestError {
                    code: ErrorCode::BadRequest,
                    message: "name must not be empty".to_owned(),
                    field: Some("name".to_owned()),
                },
            },
            json!({
                "type": "request_error",
                "id": 7,
                "error": {
                    "code": "bad_request",
                    "message": "name must not be empty",
                    "field": "name",
                },
            }),
        );
    }

    #[test]
    fn unknown_body_request_error_wire_form() {
        assert_wire_form(
            &ServerMessage::RequestError {
                id: RequestId(9),
                error: RequestError {
                    code: ErrorCode::UnknownBody,
                    message: "no body 0200080020000000.0300 in its system".to_owned(),
                    field: Some("body".to_owned()),
                },
            },
            json!({
                "type": "request_error",
                "id": 9,
                "error": {
                    "code": "unknown_body",
                    "message": "no body 0200080020000000.0300 in its system",
                    "field": "body",
                },
            }),
        );
    }

    #[test]
    fn request_error_wire_form_without_field() {
        assert_wire_form(
            &ServerMessage::RequestError {
                id: RequestId(8),
                error: RequestError {
                    code: ErrorCode::Unsupported,
                    message: "unsupported request kind `warp`".to_owned(),
                    field: None,
                },
            },
            json!({
                "type": "request_error",
                "id": 8,
                "error": {
                    "code": "unsupported",
                    "message": "unsupported request kind `warp`",
                    "field": null,
                },
            }),
        );
    }

    #[test]
    fn error_code_strings() {
        assert_wire_strings(&[
            (ErrorCode::BadRequest, "bad_request"),
            (ErrorCode::Unsupported, "unsupported"),
            (ErrorCode::HelloRequired, "hello_required"),
            (ErrorCode::UnknownUniverse, "unknown_universe"),
            (ErrorCode::UnknownSystem, "unknown_system"),
            (ErrorCode::UnknownBody, "unknown_body"),
            (
                ErrorCode::GeneratorVersionMismatch,
                "generator_version_mismatch",
            ),
            (ErrorCode::UnsupportedSaveFormat, "unsupported_save_format"),
            (ErrorCode::NameTaken, "name_taken"),
            (ErrorCode::UniverseLimitReached, "universe_limit_reached"),
            (ErrorCode::TooManyRequests, "too_many_requests"),
            (ErrorCode::QueueFull, "queue_full"),
            (ErrorCode::Cancelled, "cancelled"),
            (ErrorCode::StorageFailed, "storage_failed"),
            (ErrorCode::Internal, "internal"),
        ]);
    }

    #[test]
    fn request_kinds_lists_every_variant() {
        let requests = walk(next_request);
        let kinds: BTreeSet<String> = requests.iter().map(kind_of).collect();
        assert_eq!(kinds.len(), requests.len(), "two variants share a kind");
        let listed: BTreeSet<String> = REQUEST_KINDS.iter().map(|&kind| kind.to_owned()).collect();
        assert_eq!(
            listed.len(),
            REQUEST_KINDS.len(),
            "REQUEST_KINDS repeats a kind"
        );
        assert_eq!(kinds, listed);
    }

    #[test]
    fn response_kinds_are_the_request_kinds() {
        let responses = walk(next_response);
        let kinds: BTreeSet<String> = responses.iter().map(kind_of).collect();
        assert_eq!(kinds.len(), responses.len(), "two variants share a kind");
        let listed: BTreeSet<String> = REQUEST_KINDS.iter().map(|&kind| kind.to_owned()).collect();
        assert_eq!(kinds, listed);
    }

    #[test]
    fn request_kinds_are_pinned() {
        assert_eq!(
            REQUEST_KINDS,
            [
                "create_universe",
                "list_universes",
                "open_universe",
                "galaxy_parameters",
                "density_map",
                "systems_in_range",
                "system_summary",
            ]
        );
    }

    #[test]
    fn every_body_round_trips_inside_its_envelope() {
        for (index, body) in (1..).zip(walk(next_request)) {
            let message = ClientMessage::Request {
                id: RequestId(index),
                body,
            };
            let text = serde_json::to_string(&message).unwrap();
            assert_eq!(
                serde_json::from_str::<ClientMessage>(&text).unwrap(),
                message
            );
        }
        for (index, body) in (1..).zip(walk(next_response)) {
            let message = ServerMessage::Response {
                id: RequestId(index),
                body,
            };
            let text = serde_json::to_string(&message).unwrap();
            assert_eq!(
                serde_json::from_str::<ServerMessage>(&text).unwrap(),
                message
            );
        }
    }

    #[test]
    fn unknown_request_kind_is_rejected() {
        let error = serde_json::from_value::<ClientMessage>(json!({
            "type": "request",
            "id": 3,
            "body": { "kind": "warp", "factor": 9 },
        }))
        .unwrap_err();
        assert!(
            error.to_string().contains("unknown variant `warp`"),
            "unexpected error: {error}"
        );
    }
}
