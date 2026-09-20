//! WebSocket endpoint speaking the `hyperion-protocol` wire format.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use hyperion_protocol::{ClientMessage, PROTOCOL_VERSION, ServerMessage};

pub(crate) async fn upgrade(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(serve)
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
        let json = serde_json::to_string(&reply).expect("ServerMessage always serializes");
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
    tracing::info!("client disconnected");
}

fn handle_text(text: &str) -> ServerMessage {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => handle(message),
        Err(error) => ServerMessage::Error {
            message: format!("malformed message: {error}"),
        },
    }
}

fn handle(message: ClientMessage) -> ServerMessage {
    match message {
        ClientMessage::Hello { client_version } => {
            tracing::debug!(%client_version, "hello");
            ServerMessage::Welcome {
                server_version: env!("CARGO_PKG_VERSION").to_owned(),
                protocol_version: PROTOCOL_VERSION,
            }
        }
        ClientMessage::Ping { nonce } => ServerMessage::Pong { nonce },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_is_answered_with_matching_pong() {
        assert_eq!(
            handle(ClientMessage::Ping { nonce: 9 }),
            ServerMessage::Pong { nonce: 9 }
        );
    }

    #[test]
    fn malformed_input_yields_error_message() {
        assert!(matches!(
            handle_text("not json"),
            ServerMessage::Error { .. }
        ));
    }
}
