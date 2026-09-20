//! End-to-end test: real TCP listener, real WebSocket client.

use futures_util::{SinkExt, StreamExt};
use hyperion_protocol::{ClientMessage, PROTOCOL_VERSION, ServerMessage};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Upper bound on any single network wait, so a hung server fails the test instead of the suite.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(5);

async fn spawn_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, hyperion_server::app()).await.unwrap();
    });
    format!("ws://{addr}/ws")
}

#[tokio::test]
async fn hello_then_ping_over_websocket() {
    let url = spawn_server().await;
    let (mut socket, _) = timeout(NETWORK_TIMEOUT, connect_async(&url))
        .await
        .expect("timed out connecting")
        .unwrap();

    let mut exchange = async |message: ClientMessage| -> ServerMessage {
        let json = serde_json::to_string(&message).unwrap();
        timeout(NETWORK_TIMEOUT, socket.send(Message::text(json)))
            .await
            .expect("timed out sending")
            .unwrap();
        let reply = timeout(NETWORK_TIMEOUT, socket.next())
            .await
            .expect("timed out waiting for a reply")
            .expect("server closed the connection")
            .unwrap();
        serde_json::from_str(reply.to_text().unwrap()).unwrap()
    };

    let welcome = exchange(ClientMessage::Hello {
        client_version: "test".to_owned(),
    })
    .await;
    assert!(matches!(
        welcome,
        ServerMessage::Welcome {
            protocol_version: PROTOCOL_VERSION,
            ..
        }
    ));

    let pong = exchange(ClientMessage::Ping { nonce: 1234 }).await;
    assert_eq!(pong, ServerMessage::Pong { nonce: 1234 });
}
