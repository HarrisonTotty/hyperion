//! Universe lifecycle: creating, listing and opening the universes a server holds.
//!
//! A universe is identified by `(seed, generator_version)` for what it generates, and by its own
//! [`UniverseIdHex`] as a save, since two campaigns may share a seed and differ in what play has
//! changed. Requests are stateless: each one names its universe, so nothing is lost when the client
//! reconnects.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::primitives::{SeedHex, UniverseIdHex};

/// Asks the server to create a universe (`create_universe`), answered with a [`UniverseInfo`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateUniverseRequest {
    /// The operator's name for the universe: 1–48 characters after trimming, no control
    /// characters, unique on the server without regard to case.
    pub name: String,
    /// The seed to generate from, or `null` for one drawn by the server.
    pub seed: Option<SeedHex>,
}

/// Asks the server to check, load and warm a universe (`open_universe`), answered with its
/// [`UniverseInfo`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OpenUniverseRequest {
    /// The universe to open.
    pub universe: UniverseIdHex,
}

/// Whether the server can run a universe's generator version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum UniverseStatus {
    /// Created with the server's generator version: it can be opened and queried.
    Compatible,
    /// Created with another generator version. It is listed but cannot be opened or queried, and
    /// every request for it fails with `generator_version_mismatch`.
    GeneratorMismatch,
}

/// What the server knows about one universe.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UniverseInfo {
    /// The universe's identity as a save.
    pub id: UniverseIdHex,
    /// The operator's name for it.
    pub name: String,
    /// The seed it generates from.
    pub seed: SeedHex,
    /// The generator version it was created with, and the only one it runs under.
    pub generator_version: u32,
    /// Whether this server can run it.
    pub status: UniverseStatus,
}

/// Every universe the server holds (`list_universes`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UniverseList {
    /// The universes, sorted by name and then by ID.
    pub universes: Vec<UniverseInfo>,
    /// The generator version this server runs, against which each universe's status is judged.
    pub server_generator_version: u32,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::envelope::{RequestBody, ResponseBody};
    use crate::testing::{assert_wire_form, assert_wire_strings};

    fn talos() -> UniverseInfo {
        UniverseInfo {
            id: UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
            name: "Talos".to_owned(),
            seed: SeedHex::from_u64(1234),
            generator_version: 2,
            status: UniverseStatus::Compatible,
        }
    }

    #[test]
    fn universe_status_strings() {
        assert_wire_strings(&[
            (UniverseStatus::Compatible, "compatible"),
            (UniverseStatus::GeneratorMismatch, "generator_mismatch"),
        ]);
    }

    #[test]
    fn universe_info_wire_form() {
        assert_wire_form(
            &talos(),
            json!({
                "id": "0123456789abcdef",
                "name": "Talos",
                "seed": "00000000000004d2",
                "generator_version": 2,
                "status": "compatible",
            }),
        );
    }

    #[test]
    fn universe_list_wire_form() {
        let old = UniverseInfo {
            id: UniverseIdHex::from_u64(7),
            name: "Vega".to_owned(),
            seed: SeedHex::from_u64(u64::MAX),
            generator_version: 1,
            status: UniverseStatus::GeneratorMismatch,
        };
        assert_wire_form(
            &ResponseBody::ListUniverses(UniverseList {
                universes: vec![talos(), old],
                server_generator_version: 2,
            }),
            json!({
                "kind": "list_universes",
                "universes": [
                    {
                        "id": "0123456789abcdef",
                        "name": "Talos",
                        "seed": "00000000000004d2",
                        "generator_version": 2,
                        "status": "compatible",
                    },
                    {
                        "id": "0000000000000007",
                        "name": "Vega",
                        "seed": "ffffffffffffffff",
                        "generator_version": 1,
                        "status": "generator_mismatch",
                    },
                ],
                "server_generator_version": 2,
            }),
        );
    }

    #[test]
    fn list_universes_request_wire_form() {
        assert_wire_form(
            &RequestBody::ListUniverses,
            json!({ "kind": "list_universes" }),
        );
    }

    #[test]
    fn create_universe_wire_form_with_seed() {
        assert_wire_form(
            &RequestBody::CreateUniverse(CreateUniverseRequest {
                name: "Talos".to_owned(),
                seed: Some(SeedHex::from_u64(1234)),
            }),
            json!({ "kind": "create_universe", "name": "Talos", "seed": "00000000000004d2" }),
        );
    }

    #[test]
    fn create_universe_wire_form_without_seed() {
        assert_wire_form(
            &RequestBody::CreateUniverse(CreateUniverseRequest {
                name: "Talos".to_owned(),
                seed: None,
            }),
            json!({ "kind": "create_universe", "name": "Talos", "seed": null }),
        );
    }

    #[test]
    fn create_universe_rejects_a_malformed_seed() {
        let error = serde_json::from_value::<RequestBody>(
            json!({ "kind": "create_universe", "name": "Talos", "seed": "4D2" }),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("expected 16 lowercase hexadecimal digits"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn open_universe_wire_form() {
        assert_wire_form(
            &RequestBody::OpenUniverse(OpenUniverseRequest {
                universe: UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
            }),
            json!({ "kind": "open_universe", "universe": "0123456789abcdef" }),
        );
    }

    #[test]
    fn create_universe_response_wire_form() {
        assert_wire_form(
            &ResponseBody::CreateUniverse(talos()),
            json!({
                "kind": "create_universe",
                "id": "0123456789abcdef",
                "name": "Talos",
                "seed": "00000000000004d2",
                "generator_version": 2,
                "status": "compatible",
            }),
        );
    }

    #[test]
    fn open_universe_response_wire_form() {
        assert_wire_form(
            &ResponseBody::OpenUniverse(talos()),
            json!({
                "kind": "open_universe",
                "id": "0123456789abcdef",
                "name": "Talos",
                "seed": "00000000000004d2",
                "generator_version": 2,
                "status": "compatible",
            }),
        );
    }
}
