//! Wire protocol shared by the HYPERION server and its clients.
//!
//! Every message is a JSON object discriminated by a `type` field. The
//! TypeScript bindings in `packages/protocol/src/generated` are generated from
//! these types by `ts-rs` when running `cargo test` — never edit them by hand.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Bumped whenever a breaking change is made to the wire format.
pub const PROTOCOL_VERSION: u32 = 1;

/// Messages sent from a client to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ClientMessage {
    /// First message on a new connection.
    Hello { client_version: String },
    /// Liveness/latency probe; the server echoes `nonce` back in a [`ServerMessage::Pong`].
    Ping { nonce: u32 },
}

/// Messages sent from the server to a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ServerMessage {
    /// Reply to [`ClientMessage::Hello`].
    Welcome {
        server_version: String,
        protocol_version: u32,
    },
    /// Reply to [`ClientMessage::Ping`].
    Pong { nonce: u32 },
    /// The client sent something the server could not process.
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use serde::de::DeserializeOwned;
    use serde_json::{Value, json};

    use super::*;

    /// Asserts that `message` serializes to exactly `wire` and that `wire` parses back to it.
    fn assert_wire_form<T>(message: &T, wire: Value)
    where
        T: Serialize + DeserializeOwned + PartialEq + Debug,
    {
        assert_eq!(serde_json::to_value(message).unwrap(), wire);
        assert_eq!(&serde_json::from_value::<T>(wire).unwrap(), message);
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
                protocol_version: 1,
            },
            json!({ "type": "welcome", "server_version": "0.1.0", "protocol_version": 1 }),
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
}
