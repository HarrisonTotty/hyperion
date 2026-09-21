//! WebSocket endpoint speaking the `hyperion-protocol` wire format.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use hyperion_protocol::{ClientMessage, ErrorCode, PROTOCOL_VERSION, RequestError, ServerMessage};

use crate::limits::MAX_INBOUND_FRAME_BYTES;

/// Accepts a WebSocket. A client message or frame above [`MAX_INBOUND_FRAME_BYTES`] fails the
/// read, which ends the connection.
pub(crate) async fn upgrade(ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(MAX_INBOUND_FRAME_BYTES)
        .max_frame_size(MAX_INBOUND_FRAME_BYTES)
        .on_upgrade(serve)
}

async fn serve(mut socket: WebSocket) {
    tracing::info!("client connected");
    while let Some(Ok(frame)) = socket.recv().await {
        let reply = match frame {
            Message::Text(text) => handle_text(&text),
            Message::Close(_) => break,
            // Binary frames are unused; axum answers pings for us.
            _ => continue,
        };
        let Some(reply) = reply else {
            continue;
        };
        let json = serde_json::to_string(&reply).expect("ServerMessage always serializes");
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
    tracing::info!("client disconnected");
}

fn handle_text(text: &str) -> Option<ServerMessage> {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => handle(message),
        Err(error) => Some(ServerMessage::Error {
            message: format!("malformed message: {error}"),
        }),
    }
}

fn handle(message: ClientMessage) -> Option<ServerMessage> {
    match message {
        ClientMessage::Hello { client_version } => {
            tracing::debug!(%client_version, "hello");
            Some(ServerMessage::Welcome {
                server_version: env!("CARGO_PKG_VERSION").to_owned(),
                protocol_version: PROTOCOL_VERSION,
                generator_version: hyperion_sim::GENERATOR_VERSION.get(),
            })
        }
        ClientMessage::Ping { nonce } => Some(ServerMessage::Pong { nonce }),
        // No request kind is served yet. Refusing each one still gives it the single terminal
        // message the request convention promises.
        ClientMessage::Request { id, body: _ } => Some(ServerMessage::RequestError {
            id,
            error: RequestError {
                code: ErrorCode::Unsupported,
                message: "this server does not serve requests yet".to_owned(),
                field: None,
            },
        }),
        // Every request is answered at once, so none is ever in flight to cancel.
        ClientMessage::Cancel { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{RequestBody, RequestId};

    use super::*;

    #[test]
    fn hello_is_welcomed_with_both_versions() {
        let reply = handle(ClientMessage::Hello {
            client_version: "test".to_owned(),
        });
        assert_eq!(
            reply,
            Some(ServerMessage::Welcome {
                server_version: env!("CARGO_PKG_VERSION").to_owned(),
                protocol_version: PROTOCOL_VERSION,
                generator_version: hyperion_sim::GENERATOR_VERSION.get(),
            })
        );
    }

    #[test]
    fn ping_is_answered_with_matching_pong() {
        assert_eq!(
            handle(ClientMessage::Ping { nonce: 9 }),
            Some(ServerMessage::Pong { nonce: 9 })
        );
    }

    #[test]
    fn malformed_input_yields_error_message() {
        assert!(matches!(
            handle_text("not json"),
            Some(ServerMessage::Error { .. })
        ));
    }

    #[test]
    fn request_is_refused_as_unsupported_under_its_id() {
        let reply = handle_text(r#"{"type":"request","id":7,"body":{"kind":"list_universes"}}"#);
        let Some(ServerMessage::RequestError { id, error }) = reply else {
            panic!("expected a request error, got {reply:?}");
        };
        assert_eq!(id, RequestId(7));
        assert_eq!(error.code, ErrorCode::Unsupported);
    }

    #[test]
    fn cancel_gets_no_reply() {
        assert_eq!(handle(ClientMessage::Cancel { id: RequestId(7) }), None);
        assert_ne!(
            handle(ClientMessage::Request {
                id: RequestId(8),
                body: RequestBody::ListUniverses,
            }),
            None,
            "a request is still answered"
        );
    }
}
