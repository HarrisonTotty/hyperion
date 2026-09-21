//! End to end over a real TCP listener and a real WebSocket client.

mod common;

use common::{TestClient, TestServer};
use hyperion_protocol::{PROTOCOL_VERSION, ServerMessage};
use hyperion_server::limits::MAX_INBOUND_FRAME_BYTES;

#[tokio::test]
async fn hello_then_ping_over_websocket() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;

    let welcome = client.hello().await;
    assert!(
        matches!(
            welcome,
            ServerMessage::Welcome {
                protocol_version: PROTOCOL_VERSION,
                ..
            }
        ),
        "{welcome:?}"
    );
    assert_eq!(client.ping(1234).await, ServerMessage::Pong { nonce: 1234 });

    drop(client);
    server.stop().await;
}

#[tokio::test]
async fn a_frame_at_the_limit_is_read() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    client.send_raw(&"x".repeat(MAX_INBOUND_FRAME_BYTES)).await;
    assert!(
        matches!(client.next_message().await, ServerMessage::Error { .. }),
        "a frame of exactly the limit is read, and answered as malformed"
    );
    assert_eq!(client.ping(7).await, ServerMessage::Pong { nonce: 7 });
    server.stop().await;
}

#[tokio::test]
async fn oversized_frame_closes_the_connection() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    client.hello().await;
    client
        .send_raw(&"x".repeat(MAX_INBOUND_FRAME_BYTES + 1024))
        .await;
    client.closed().await;
    // The server itself is unharmed.
    let mut other = server.connect().await;
    assert_eq!(other.ping(1).await, ServerMessage::Pong { nonce: 1 });
    server.stop().await;
}

#[tokio::test]
async fn the_server_stops_with_a_client_connected() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.url()).await;
    client.hello().await;
    server.stop().await;
}
