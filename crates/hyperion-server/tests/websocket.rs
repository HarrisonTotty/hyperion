//! End to end over a real TCP listener and a real WebSocket client: the connection, its limits
//! and the request convention as a client sees them.

mod common;

use common::{TestClient, TestServer};
use hyperion_protocol::{ErrorCode, PROTOCOL_VERSION, RequestBody, RequestId, ServerMessage};
use hyperion_server::limits::{MAX_CONSECUTIVE_MALFORMED_FRAMES, MAX_INBOUND_FRAME_BYTES};

/// The WebSocket close code for a policy violation (RFC 6455, section 7.4.1).
const POLICY_VIOLATION: u16 = 1008;

/// The WebSocket close code for an endpoint going away (RFC 6455, section 7.4.1).
const GOING_AWAY: u16 = 1001;

/// The code of a `request_error` for request `id`.
fn error_code(message: &ServerMessage, id: u32) -> ErrorCode {
    match message {
        ServerMessage::RequestError {
            id: answered,
            error,
        } if answered.0 == id => error.code,
        other => panic!("expected a request error for request {id}, got {other:?}"),
    }
}

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

    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn every_kind_is_unsupported_until_its_handler_exists() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    client.hello().await;
    let error = client
        .request(RequestBody::ListUniverses)
        .await
        .expect_err("no request is served yet");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert_eq!(
        error.message,
        "this server does not serve `list_universes` requests yet"
    );
    let requests = server.stats().requests();
    assert_eq!(
        (requests.accepted(), requests.failed(), requests.in_flight()),
        (1, 1, 0)
    );
    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_request_before_hello_is_refused_with_hello_required() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    let error = client
        .request(RequestBody::ListUniverses)
        .await
        .expect_err("hello comes first");
    assert_eq!(error.code, ErrorCode::HelloRequired);
    assert_eq!(server.stats().requests().refused(), 1);
    // After hello the same request is accepted.
    client.hello().await;
    assert_eq!(
        client
            .request(RequestBody::ListUniverses)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn malformed_json_gets_the_connection_level_error_and_the_connection_goes_on() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    client.hello().await;
    for text in ["not json", r#"{"type":"warp"}"#, r#"{"type":"request"}"#] {
        client.send_raw(text).await;
        match client.next_message().await {
            ServerMessage::Error { message } => {
                assert!(message.starts_with("malformed message: "), "{message}");
            }
            other => panic!("expected the connection-level error for {text}, got {other:?}"),
        }
    }
    assert_eq!(client.ping(5).await, ServerMessage::Pong { nonce: 5 });
    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_request_that_does_not_parse_is_answered_under_its_id() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    client.hello().await;
    client
        .send_raw(r#"{"type":"request","id":3,"body":{"kind":"open_universe","universe":"4D2"}}"#)
        .await;
    assert_eq!(
        error_code(&client.next_message().await, 3),
        ErrorCode::BadRequest
    );
    client
        .send_raw(r#"{"type":"request","id":4,"body":{"kind":"warp","factor":9}}"#)
        .await;
    assert_eq!(
        error_code(&client.next_message().await, 4),
        ErrorCode::Unsupported
    );
    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn a_cancel_for_a_request_that_has_ended_is_ignored() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    client.hello().await;
    let id = client.send_request(RequestBody::ListUniverses).await;
    assert_eq!(id, RequestId(1));
    assert_eq!(
        error_code(&client.next_message().await, 1),
        ErrorCode::Unsupported
    );
    client.cancel(id).await;
    // Nothing answers the cancel: the pong is the next message.
    assert_eq!(client.ping(6).await, ServerMessage::Pong { nonce: 6 });
    client.close().await;
    server.stop().await;
}

#[tokio::test]
async fn malformed_frames_in_a_row_close_the_connection_with_a_policy_violation() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    client.hello().await;
    for _ in 0..MAX_CONSECUTIVE_MALFORMED_FRAMES {
        client.send_raw("not json").await;
        assert!(matches!(
            client.next_message().await,
            ServerMessage::Error { .. }
        ));
    }
    assert_eq!(client.closed().await, Some(POLICY_VIOLATION));
    // The server is unharmed.
    let mut other = server.connect().await;
    assert_eq!(other.ping(1).await, ServerMessage::Pong { nonce: 1 });
    other.close().await;
    server.stop().await;
}

#[tokio::test]
async fn binary_frames_are_not_used_and_count_as_malformed() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    client.hello().await;
    for _ in 0..MAX_CONSECUTIVE_MALFORMED_FRAMES {
        client.send_binary(&[0x00, 0xff]).await;
        match client.next_message().await {
            ServerMessage::Error { message } => assert_eq!(message, "binary frames are not used"),
            other => panic!("expected the connection-level error, got {other:?}"),
        }
    }
    assert_eq!(client.closed().await, Some(POLICY_VIOLATION));
    server.stop().await;
}

#[tokio::test]
async fn a_well_formed_frame_starts_the_malformed_count_again() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    for round in 0..3 {
        for _ in 1..MAX_CONSECUTIVE_MALFORMED_FRAMES {
            client.send_raw("{").await;
            assert!(matches!(
                client.next_message().await,
                ServerMessage::Error { .. }
            ));
        }
        assert_eq!(
            client.ping(round).await,
            ServerMessage::Pong { nonce: round }
        );
    }
    client.close().await;
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
    client.close().await;
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
    // The server is unharmed.
    let mut other = server.connect().await;
    assert_eq!(other.ping(1).await, ServerMessage::Pong { nonce: 1 });
    other.close().await;
    server.stop().await;
}

#[tokio::test]
async fn the_server_stops_with_clients_connected_and_closes_them() {
    let server = TestServer::start().await;
    let mut greeted = TestClient::connect(server.url()).await;
    greeted.hello().await;
    let silent = server.connect().await;
    assert_eq!(server.stats().connections(), 2);
    // The clients read on, as a client does, and so answer the server's close.
    let closing =
        [greeted, silent].map(|mut client| tokio::spawn(async move { client.closed().await }));
    server.stop().await;
    for closed in closing {
        assert_eq!(closed.await.unwrap(), Some(GOING_AWAY));
    }
}
