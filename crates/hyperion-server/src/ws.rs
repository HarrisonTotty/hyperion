//! The WebSocket endpoint: one task per connection, speaking `hyperion-protocol`.
//!
//! The connection task owns everything about its socket (plan 04, design note 22). It reads the
//! client's frames, answers `hello` and `ping` itself, and runs each request as a task in a
//! [`JoinSet`](tokio::task::JoinSet) through [`Requests`], waiting on frames, finished requests,
//! room in its outbound queue, its writer and the server's shutdown in one `select!`. A writer
//! task, owned and joined by the connection task, sends what the connection queues for it
//! ([`outbound`](crate::outbound)). When the client reads too slowly, the queue's byte budget
//! holds finished requests back while the connection reads on, and a full count of queued frames
//! stops it reading until the queue drains; a frame not written within the write timeout closes
//! the connection with a policy violation. Closing the connection, for whatever reason, cancels
//! every request it has in flight.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Extension;
use axum::extract::State;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade, close_code};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use futures_util::stream::SplitStream;
use hyperion_protocol::{ClientMessage, PROTOCOL_VERSION, ServerMessage};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::Instrument;

use crate::AppState;
use crate::connections::{Closing, ConnectionGuard};
use crate::limits::{
    CLOSE_TIMEOUT, MAX_CONSECUTIVE_MALFORMED_FRAMES, MAX_INBOUND_FRAME_BYTES, OUTBOUND_BYTES,
    WRITE_TIMEOUT,
};
use crate::outbound::{self, Held, Outbound, WriterStopped};
use crate::requests::{self, Handshake, Inbound, Requests, Tally, to_frame};

/// The limits a connection enforces on writing to its client.
///
/// The server's are those of [`limits`](crate::limits), which [`ConnectionLimits::default`]
/// gives; unit tests shorten them, so that a slow reader's test need not wait out
/// [`WRITE_TIMEOUT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ConnectionLimits {
    /// Payload bytes the outbound queue holds before finished requests are held back.
    pub(crate) outbound_bytes: usize,
    /// Longest one frame may take to be written.
    pub(crate) write_timeout: Duration,
    /// Longest a closing connection may take to send what it has queued and finish closing.
    pub(crate) close_timeout: Duration,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            outbound_bytes: OUTBOUND_BYTES,
            write_timeout: WRITE_TIMEOUT,
            close_timeout: CLOSE_TIMEOUT,
        }
    }
}

/// Accepts a WebSocket, unless the server is shutting down. A client message or frame above
/// [`MAX_INBOUND_FRAME_BYTES`] fails the read, which ends the connection.
pub(crate) async fn upgrade(
    State(state): State<Arc<AppState>>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    ws: WebSocketUpgrade,
) -> Response {
    // Taken before the upgrade, so that shutdown also waits for a connection still upgrading. If
    // the upgrade fails, axum drops the callback and the guard with it.
    let Some(guard) = state.connections.open() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "the server is shutting down",
        )
            .into_response();
    };
    let peer = connect_info.map(|Extension(ConnectInfo(peer))| peer);
    ws.max_message_size(MAX_INBOUND_FRAME_BYTES)
        .max_frame_size(MAX_INBOUND_FRAME_BYTES)
        .on_upgrade(move |socket| serve(socket, state, guard, peer))
}

/// The connection task.
async fn serve(
    socket: WebSocket,
    state: Arc<AppState>,
    guard: ConnectionGuard,
    peer: Option<SocketAddr>,
) {
    let span = tracing::info_span!("connection", id = guard.id(), peer = tracing::field::Empty);
    if let Some(peer) = peer {
        span.record("peer", tracing::field::display(peer));
    }
    Connection::run(socket, state, guard.closing())
        .instrument(span)
        .await;
    // Dropped only now, with the writer joined, so that shutdown waits for all of it.
    drop(guard);
}

/// The answer to `hello`.
fn welcome() -> ServerMessage {
    ServerMessage::Welcome {
        server_version: env!("CARGO_PKG_VERSION").to_owned(),
        protocol_version: PROTOCOL_VERSION,
        generator_version: hyperion_sim::GENERATOR_VERSION.get(),
    }
}

/// Why a connection ended.
#[derive(Debug)]
enum End {
    /// The client sent a close frame or the stream ended.
    ClientClosed,
    /// Reading failed, as it does for a frame above [`MAX_INBOUND_FRAME_BYTES`].
    ReadFailed(axum::Error),
    /// The writer stopped, because writing to the socket failed.
    WriterGone,
    /// A frame could not be written within the write timeout: the client has stopped reading.
    WriteTimedOut,
    /// [`MAX_CONSECUTIVE_MALFORMED_FRAMES`] arrived in a row.
    TooManyMalformed,
    /// The server is shutting down.
    ShuttingDown,
}

impl End {
    /// The close frame the server sends, when it is the one closing.
    fn close_frame(&self) -> Option<CloseFrame> {
        match self {
            Self::TooManyMalformed => Some(CloseFrame {
                code: close_code::POLICY,
                reason: Utf8Bytes::from_static("too many malformed frames"),
            }),
            Self::WriteTimedOut => Some(CloseFrame {
                code: close_code::POLICY,
                reason: Utf8Bytes::from_static("the client is not reading"),
            }),
            Self::ShuttingDown => Some(CloseFrame {
                code: close_code::AWAY,
                reason: Utf8Bytes::from_static("the server is shutting down"),
            }),
            Self::ClientClosed | Self::ReadFailed(_) | Self::WriterGone => None,
        }
    }
}

impl From<WriterStopped> for End {
    fn from(stopped: WriterStopped) -> Self {
        match stopped {
            WriterStopped::Failed => Self::WriterGone,
            WriterStopped::TimedOut => Self::WriteTimedOut,
        }
    }
}

impl fmt::Display for End {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientClosed => f.write_str("closed by the client"),
            Self::ReadFailed(error) => write!(f, "reading failed: {error}"),
            Self::WriterGone => f.write_str("writing failed"),
            Self::WriteTimedOut => f.write_str("a frame was not written in time"),
            Self::TooManyMalformed => f.write_str("too many malformed frames"),
            Self::ShuttingDown => f.write_str("the server is shutting down"),
        }
    }
}

/// What woke the connection.
enum Event {
    /// The oldest held request's frame fits the outbound queue now.
    Room,
    Finished(Result<(tokio::task::Id, requests::Finished), tokio::task::JoinError>),
    Frame(Option<Result<Message, axum::Error>>),
}

/// The reading side of a connection and everything it owns.
struct Connection {
    requests: Requests,
    outbound: Outbound,
    /// Finished requests waiting for room in `outbound`.
    held: Held,
    closing: Closing,
    handshake: Handshake,
    malformed_in_a_row: u32,
}

impl Connection {
    /// Runs a connection from its upgrade to its close.
    async fn run(socket: WebSocket, state: Arc<AppState>, closing: Closing) {
        tracing::info!("client connected");
        let limits = state.connection_limits;
        let (sink, mut stream) = socket.split();
        let (outbound, writer) = outbound::open(
            state.outbound_stats.clone(),
            limits.outbound_bytes,
            limits.write_timeout,
        );
        let mut writer = tokio::spawn(writer.run(sink).in_current_span());
        let mut connection = Self {
            requests: Requests::new(Arc::clone(&state)),
            outbound,
            held: Held::new(state.outbound_stats.clone()),
            closing,
            handshake: Handshake::Pending,
            malformed_in_a_row: 0,
        };
        let end = connection.read(&mut stream).await;
        if matches!(end, End::WriteTimedOut) {
            state.outbound_stats.write_timed_out();
        }
        let Self {
            mut requests,
            outbound,
            held,
            ..
        } = connection;
        // Held requests are still in flight, and are abandoned with the rest.
        drop(held);
        requests.close().await;
        let closed = timeout(
            limits.close_timeout,
            close(end.close_frame(), outbound, &mut stream, &mut writer),
        )
        .await;
        let joined = if let Ok(joined) = closed {
            joined
        } else {
            tracing::debug!("the client did not finish closing in time; dropping the socket");
            writer.abort();
            // An aborted task ends at its next `.await`, so this returns at once.
            writer.await
        };
        if let Err(error) = joined
            && error.is_panic()
        {
            tracing::error!(%error, "the connection's writer panicked");
        }
        tracing::info!(reason = %end, "client disconnected");
    }

    /// Reads and answers frames until the connection ends, and says why it did.
    async fn read(&mut self, stream: &mut SplitStream<WebSocket>) -> End {
        loop {
            let next_held = self.held.next_len();
            let room = self.outbound.room_for(next_held.unwrap_or_default());
            let event = tokio::select! {
                biased;
                () = self.closing.wait() => return End::ShuttingDown,
                stopped = self.outbound.writer_stopped() => return End::from(stopped),
                // Finished requests first, so that their places are free before more are read,
                // as long as the queue has room for them. Frames are read on regardless, so that
                // `ping` and `cancel` are answered while it has none.
                () = room, if next_held.is_some() => Event::Room,
                Some(joined) = self.requests.join_next(), if self.requests.has_tasks() => {
                    Event::Finished(joined)
                }
                frame = stream.next() => Event::Frame(frame),
            };
            let handled = match event {
                Event::Room => match self.held.pop().and_then(|held| self.requests.end(held)) {
                    Some(frame) => self.push(Message::Text(frame.into())).await,
                    None => Ok(()),
                },
                Event::Finished(joined) => match self.requests.settle(joined) {
                    // Queued at once only if nothing is held before it, so that terminal frames
                    // are queued in the order their requests finished.
                    Some(settled)
                        if self.held.is_empty()
                            && self.outbound.has_room_for(settled.frame_len()) =>
                    {
                        match self.requests.end(settled) {
                            Some(frame) => self.push(Message::Text(frame.into())).await,
                            None => Ok(()),
                        }
                    }
                    Some(settled) => {
                        self.held.push(settled);
                        Ok(())
                    }
                    None => Ok(()),
                },
                Event::Frame(None | Some(Ok(Message::Close(_)))) => Err(End::ClientClosed),
                Event::Frame(Some(Err(error))) => Err(End::ReadFailed(error)),
                Event::Frame(Some(Ok(Message::Text(text)))) => {
                    self.on_inbound(requests::parse(text.as_str())).await
                }
                // Binary frames are reserved for bulk payloads from the server (plan 04,
                // Extending the convention); a client has no use for them yet.
                Event::Frame(Some(Ok(Message::Binary(_)))) => {
                    self.on_inbound(Inbound::Malformed {
                        message: "binary frames are not used".to_owned(),
                    })
                    .await
                }
                // The WebSocket layer answers pings itself.
                Event::Frame(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => Ok(()),
            };
            if let Err(end) = handled {
                return end;
            }
        }
    }

    /// Answers one frame, and closes the connection once too many malformed ones have arrived in
    /// a row.
    async fn on_inbound(&mut self, inbound: Inbound) -> Result<(), End> {
        match inbound.tally() {
            Tally::Reset => self.malformed_in_a_row = 0,
            Tally::Count => self.malformed_in_a_row = self.malformed_in_a_row.saturating_add(1),
            Tally::Leave => {}
        }
        let reply = match inbound {
            Inbound::Message(message) => self.on_message(message),
            Inbound::Unparsed { id, error } => {
                Some(self.requests.reject(id, error, self.handshake))
            }
            Inbound::Malformed { message } => {
                tracing::warn!(%message, "malformed frame");
                Some(to_frame(&ServerMessage::Error { message }))
            }
        };
        if let Some(frame) = reply {
            self.push(Message::Text(frame.into())).await?;
        }
        if self.malformed_in_a_row >= MAX_CONSECUTIVE_MALFORMED_FRAMES {
            tracing::warn!(
                count = self.malformed_in_a_row,
                "closing after too many malformed frames in a row"
            );
            return Err(End::TooManyMalformed);
        }
        Ok(())
    }

    /// Answers a well-formed message, if it has an immediate answer.
    fn on_message(&mut self, message: ClientMessage) -> Option<String> {
        match message {
            ClientMessage::Hello { client_version } => {
                tracing::debug!(%client_version, "hello");
                self.handshake = Handshake::Done;
                Some(to_frame(&welcome()))
            }
            ClientMessage::Ping { nonce } => Some(to_frame(&ServerMessage::Pong { nonce })),
            ClientMessage::Request { id, body } => self.requests.submit(id, body, self.handshake),
            ClientMessage::Cancel { id } => {
                let cancelled = self.requests.cancel(id);
                // A held frame of the request just cancelled is dropped: `cancelled` ended it.
                self.held
                    .retain(|settled| self.requests.is_in_flight(settled));
                cancelled
            }
        }
    }

    /// Queues a frame for the writer, waiting while the queue holds its full count of frames,
    /// unless the server starts shutting down meanwhile.
    async fn push(&mut self, message: Message) -> Result<(), End> {
        tokio::select! {
            biased;
            () = self.closing.wait() => Err(End::ShuttingDown),
            sent = self.outbound.send(message) => sent.map_err(|_| End::WriterGone),
        }
    }
}

/// Finishes closing a connection whose requests have been cancelled: sends the server's close
/// frame if it is the one closing, lets the writer drain, and joins it.
///
/// After its close frame the server reads on until the client's answering close. That consumes
/// whatever the client sent meanwhile, so that the socket is shut and not reset, which could
/// lose the close frame on its way to the client.
async fn close(
    frame: Option<CloseFrame>,
    outbound: Outbound,
    stream: &mut SplitStream<WebSocket>,
    writer: &mut JoinHandle<()>,
) -> Result<(), tokio::task::JoinError> {
    if let Some(frame) = frame
        && outbound.send(Message::Close(Some(frame))).await.is_ok()
    {
        while let Some(Ok(message)) = stream.next().await {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    }
    // The writer ends once its queue is empty and closed.
    drop(outbound);
    writer.await
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{ErrorCode, RequestId};

    use super::*;
    use crate::limits::OUTBOUND_QUEUE_FRAMES;
    use crate::testing::{
        CLOGGING_BYTES, Call, Calls, Client, Harness, NEVER, Scripted, body, bulky_response,
        small_response,
    };
    use crate::{Server, ServerConfig};

    #[test]
    fn hello_is_welcomed_with_both_versions() {
        assert_eq!(
            welcome(),
            ServerMessage::Welcome {
                server_version: env!("CARGO_PKG_VERSION").to_owned(),
                protocol_version: PROTOCOL_VERSION,
                generator_version: hyperion_sim::GENERATOR_VERSION.get(),
            }
        );
    }

    #[tokio::test]
    async fn the_server_holds_its_connections_to_the_limits_of_the_limits_module() {
        let data_dir = tempfile::tempdir().unwrap();
        let server = Server::start(ServerConfig::builder().data_dir(data_dir.path()).build())
            .await
            .unwrap();
        assert_eq!(
            server.state().connection_limits,
            ConnectionLimits {
                outbound_bytes: OUTBOUND_BYTES,
                write_timeout: WRITE_TIMEOUT,
                close_timeout: CLOSE_TIMEOUT,
            }
        );
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn ping_is_answered_with_matching_pong_before_and_after_hello() {
        let (handler, _calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.send(&ClientMessage::Ping { nonce: u32::MAX }).await;
        assert_eq!(
            client.next_message().await,
            ServerMessage::Pong { nonce: u32::MAX }
        );
        client.hello().await;
        client.send(&ClientMessage::Ping { nonce: 0 }).await;
        assert_eq!(
            client.next_message().await,
            ServerMessage::Pong { nonce: 0 }
        );
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_connection_is_refused_once_the_server_is_closing() {
        let (handler, _calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        // Serving goes on, as it does between shutdown's start and the listener's close.
        tokio::time::timeout(
            crate::testing::WAIT,
            harness.state().connections.close_all(),
        )
        .await
        .expect("no connection is open");
        let refused = tokio::time::timeout(
            crate::testing::WAIT,
            tokio_tungstenite::connect_async(harness.url()),
        )
        .await
        .expect("timed out connecting");
        match refused {
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("expected 503, got {other:?}"),
        }
        assert_eq!(harness.server().stats().connections(), 0);
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_client_that_disconnects_mid_request_cancels_it() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        let mut call = calls.next().await;
        // No close frame: the TCP connection simply ends.
        drop(client);
        call.dropped().await;
        assert!(call.token.is_cancelled());
        harness.connections_until(0).await;
        let counters = harness.server().stats().requests();
        assert_eq!(
            (
                counters.accepted(),
                counters.abandoned(),
                counters.in_flight()
            ),
            (1, 1, 0)
        );
        // The server is unharmed.
        let mut other = harness.connect().await;
        other.hello().await;
        assert_eq!(other.ping(1).await, []);
        other.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_request_in_flight_when_the_client_closes_is_cancelled_and_never_answered() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        let mut call = calls.next().await;
        // `close` fails the test on any text frame, the request's answer included.
        assert_eq!(
            client.close().await,
            None,
            "the server echoes the empty close"
        );
        call.dropped().await;
        assert!(call.token.is_cancelled());
        assert!(!call.respond(crate::testing::small_response()));
        harness.connections_until(0).await;
        assert_eq!(harness.server().stats().requests().abandoned(), 1);
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_request_sent_just_before_the_close_is_never_answered() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        client.close().await;
        harness.connections_until(0).await;
        let counters = harness.server().stats().requests();
        assert_eq!((counters.accepted(), counters.abandoned()), (1, 1));
        // The handler may or may not have been reached before the close; if it was, it has been
        // dropped.
        while !calls.is_empty() {
            let mut call = calls.next().await;
            call.dropped().await;
            assert!(call.token.is_cancelled());
        }
        harness.stop().await;
    }

    #[tokio::test]
    async fn too_many_malformed_frames_close_with_a_policy_violation_and_cancel_requests() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        client.request(2, body(2)).await;
        let mut running = [calls.next().await, calls.next().await];
        for _ in 1..MAX_CONSECUTIVE_MALFORMED_FRAMES {
            client.send_raw("not json").await;
            assert!(matches!(
                client.next_message().await,
                ServerMessage::Error { .. }
            ));
        }
        // A kind this server does not know neither counts nor starts the count again.
        client
            .send_raw(r#"{"type":"request","id":50,"body":{"kind":"warp"}}"#)
            .await;
        assert!(matches!(
            client.next_message().await,
            ServerMessage::RequestError { id: RequestId(50), error } if error.code == ErrorCode::Unsupported
        ));
        client.send_raw("not json").await;
        assert!(matches!(
            client.next_message().await,
            ServerMessage::Error { .. }
        ));
        assert_eq!(client.closed().await, Some(close_code::POLICY));
        for call in &mut running {
            call.dropped().await;
            assert!(call.token.is_cancelled());
        }
        harness.connections_until(0).await;
        assert_eq!(harness.server().stats().requests().abandoned(), 2);
        harness.stop().await;
    }

    #[tokio::test]
    async fn an_oversized_frame_closes_the_connection_and_cancels_its_requests() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        let mut call = calls.next().await;
        client
            .send_raw(&"x".repeat(MAX_INBOUND_FRAME_BYTES + 1))
            .await;
        // The read fails, so there is no close code to report, only the end of the connection.
        client.drain().await;
        call.dropped().await;
        assert!(call.token.is_cancelled());
        harness.connections_until(0).await;
        assert_eq!(harness.server().stats().requests().abandoned(), 1);
        harness.stop().await;
    }

    #[tokio::test]
    async fn shutdown_closes_every_connection_and_cancels_its_requests() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let state = Arc::clone(harness.state());
        let mut busy = harness.connect().await;
        busy.hello().await;
        busy.request(1, body(1)).await;
        let mut call = calls.next().await;
        let mut idle = harness.connect().await;
        idle.hello().await;
        // The clients read on, as a client does, and so answer the server's close.
        let busy = tokio::spawn(async move { busy.closed().await });
        let idle = tokio::spawn(async move { idle.closed().await });

        harness.stop().await;
        assert_eq!(busy.await.unwrap(), Some(close_code::AWAY));
        assert_eq!(idle.await.unwrap(), Some(close_code::AWAY));
        call.dropped().await;
        assert!(call.token.is_cancelled());
        assert_eq!(state.connections.open_count(), 0);
        let counters = state.request_stats.snapshot();
        assert_eq!((counters.abandoned(), counters.in_flight()), (1, 0));
        assert!(
            state.connections.open().is_none(),
            "no connection opens after shutdown"
        );
    }

    #[tokio::test]
    async fn shutdown_does_not_wait_for_a_client_that_never_answers_the_close() {
        let (handler, _calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        // The client reads nothing until the server has shut down, which gives up on the close
        // handshake after `CLOSE_TIMEOUT`, well within the harness's own time limit.
        harness.stop().await;
        assert_eq!(client.closed().await, Some(close_code::AWAY));
    }

    /// Duplicates of an ID in flight sent behind the clogging response: twice what the outbound
    /// queue holds. Each is refused with the connection-level `error`, and counted as refused, as
    /// the connection reads it.
    const CLOGGING_DUPLICATES: usize = 2 * OUTBOUND_QUEUE_FRAMES;

    /// The frames a connection whose writer is stuck reads before it stops: as many as its queue
    /// holds, and the one it is waiting to queue.
    fn frames_read_until_full() -> u64 {
        u64::try_from(OUTBOUND_QUEUE_FRAMES + 1).expect("the queue is small")
    }

    /// A harness whose connections never time a write out. A test that sticks a writer is about
    /// the queue behind it, not the write timeout, and under load the steps it takes before the
    /// client reads again can outlast [`WRITE_TIMEOUT`]; the write timeout's own tests are in
    /// [`outbound`](crate::outbound).
    async fn harness_without_write_timeout(handler: Scripted) -> Harness {
        let limits = ConnectionLimits {
            write_timeout: NEVER,
            ..ConnectionLimits::default()
        };
        Harness::start_with_limits(handler, limits).await
    }

    /// Makes `client`'s server-side writer stick on a response the client does not read. Then
    /// sends request 2, which stays in flight, [`CLOGGING_DUPLICATES`] duplicates of it and
    /// request 3, and waits until the connection has stopped reading, part-way through the
    /// duplicates, with its queue full. Returns request 2's call.
    async fn clog(harness: &Harness, calls: &mut Calls, client: &mut Client) -> Call {
        client.hello().await;
        // Every refusal is queued behind the response, where the writer, stuck on it, takes none
        // of them.
        harness.stick_writer(calls, client, 1).await;
        client.request(2, body(2)).await;
        let second = calls.next().await;
        assert_eq!(second.id(), 2);
        for _ in 0..CLOGGING_DUPLICATES {
            client.request(2, body(2)).await;
        }
        client.request(3, body(3)).await;
        harness
            .requests_until(|counters| counters.refused() >= frames_read_until_full())
            .await;
        second
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_full_outbound_queue_stops_the_reader_and_loses_nothing() {
        let (handler, mut calls) = Scripted::new();
        let harness = harness_without_write_timeout(handler).await;
        let mut slow = harness.connect_slow_reader().await;
        let second = clog(&harness, &mut calls, &mut slow).await;

        // Everyone else is served meanwhile.
        let mut other = harness.connect().await;
        other.hello().await;
        assert_eq!(other.ping(0).await, []);
        other.close().await;
        // The slow client's connection has read no further: not the rest of the duplicates, and
        // not request 3.
        assert_eq!(
            harness.server().stats().requests().refused(),
            frames_read_until_full(),
            "the connection read on with its queue full"
        );
        assert!(calls.is_empty(), "request 3 was read with the queue full");

        // Reading again, the client gets every frame, in order, and the connection goes on.
        assert_eq!(
            slow.next_message().await,
            ServerMessage::Response {
                id: RequestId(1),
                body: bulky_response(CLOGGING_BYTES),
            }
        );
        for _ in 0..CLOGGING_DUPLICATES {
            assert_eq!(
                slow.next_message().await,
                ServerMessage::Error {
                    message: "request 2 is already in flight; the new request was dropped"
                        .to_owned(),
                }
            );
        }
        let third = calls.next().await;
        assert_eq!(third.id(), 3);
        for (call, id) in [(second, 2), (third, 3)] {
            assert!(call.respond(small_response()));
            assert_eq!(
                slow.next_message().await,
                ServerMessage::Response {
                    id: RequestId(id),
                    body: small_response(),
                }
            );
        }
        slow.close().await;
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn shutdown_is_not_held_up_by_a_connection_whose_queue_is_full() {
        let (handler, mut calls) = Scripted::new();
        let harness = harness_without_write_timeout(handler).await;
        let mut slow = harness.connect_slow_reader().await;
        let mut second = clog(&harness, &mut calls, &mut slow).await;
        // Neither the queued frames nor the close frame behind them can be sent, and no write
        // times out; the connection gives up after `CLOSE_TIMEOUT`, and `stop` checks that
        // nothing is left.
        let state = Arc::clone(harness.state());
        harness.stop().await;
        second.dropped().await;
        assert!(second.token.is_cancelled());
        assert!(calls.is_empty(), "request 3 was never read");
        assert_eq!(state.request_stats.snapshot().abandoned(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_client_that_stops_reading_and_then_disconnects_frees_its_connection() {
        let (handler, mut calls) = Scripted::new();
        let harness = harness_without_write_timeout(handler).await;
        let mut slow = harness.connect_slow_reader().await;
        let mut second = clog(&harness, &mut calls, &mut slow).await;
        // Unread data makes the client's end reset the connection, which fails the stuck write.
        drop(slow);
        harness.connections_until(0).await;
        second.dropped().await;
        assert!(second.token.is_cancelled());
        let counters = harness.server().stats().requests();
        assert_eq!(
            (
                counters.accepted(),
                counters.responded(),
                counters.abandoned(),
                counters.in_flight()
            ),
            (2, 1, 1, 0)
        );
        assert!(calls.is_empty(), "request 3 was never read");
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_connection_upgrading_as_shutdown_starts_is_closed_going_away() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (handler, _calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let addr = harness.addr();
        let mut tcp =
            tokio::time::timeout(crate::testing::WAIT, tokio::net::TcpStream::connect(addr))
                .await
                .expect("timed out connecting")
                .unwrap();
        let upgrade = format!(
            "GET /ws HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        tokio::time::timeout(crate::testing::WAIT, tcp.write_all(upgrade.as_bytes()))
            .await
            .expect("timed out sending the upgrade")
            .unwrap();
        // The connection is counted from the moment its upgrade is accepted, before its task
        // runs, and shutdown starts at once.
        harness.connections_until(1).await;
        let stopping = tokio::spawn(harness.stop());
        // The client never answers the close, so the server drops the socket after
        // `CLOSE_TIMEOUT` and the stream ends.
        let mut received = Vec::new();
        tokio::time::timeout(crate::testing::WAIT, tcp.read_to_end(&mut received))
            .await
            .expect("timed out reading")
            .expect("the server ends the stream cleanly");
        tokio::time::timeout(crate::testing::WAIT, stopping)
            .await
            .expect("timed out stopping")
            .expect("stopping does not panic");
        let head_end = received
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("a complete response head")
            + 4;
        assert!(
            received.starts_with(b"HTTP/1.1 101 "),
            "{:?}",
            String::from_utf8_lossy(&received)
        );
        // One unmasked close frame: FIN and opcode 8, the payload's length, then the code.
        let reason = b"the server is shutting down";
        let mut close = vec![0x88, u8::try_from(2 + reason.len()).unwrap()];
        close.extend_from_slice(&close_code::AWAY.to_be_bytes());
        close.extend_from_slice(reason);
        assert_eq!(received[head_end..], close);
    }
}
