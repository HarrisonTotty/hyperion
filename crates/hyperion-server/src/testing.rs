//! Support for the unit tests of connections and requests: a server whose handler the test
//! injects, on a loopback port the OS chose, and a WebSocket client for it.
//!
//! The integration tests' `tests/common/mod.rs` does the same over the public API, which cannot
//! inject a handler. Every wait here is bounded by [`WAIT`].

use std::future::IntoFuture;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use hyperion_protocol::{
    ClientMessage, RequestBody, RequestError, RequestId, ResponseBody, ServerMessage,
};
use tempfile::TempDir;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::compute::CancelToken;
use crate::requests::{Handler, HandlerFuture, to_frame};
use crate::stats::{OutboundCounters, RequestCounters};
use crate::ws::ConnectionLimits;
use crate::{AppState, Server, ServerConfig, ServerStats};

/// Upper bound on any single wait.
pub(crate) const WAIT: Duration = Duration::from_secs(5);

/// A server with an injected handler, serving on `127.0.0.1` at a port the OS chose.
#[derive(Debug)]
pub(crate) struct Harness {
    addr: SocketAddr,
    url: String,
    server: Server,
    stop_serving: oneshot::Sender<()>,
    serving: JoinHandle<io::Result<()>>,
    _data_dir: TempDir,
}

impl Harness {
    /// A server with one CPU worker whose every request `handler` answers.
    pub(crate) async fn start(handler: impl Handler + 'static) -> Self {
        Self::start_with_limits(handler, ConnectionLimits::default()).await
    }

    /// [`Harness::start`], with each connection held to `limits`.
    pub(crate) async fn start_with_limits(
        handler: impl Handler + 'static,
        limits: ConnectionLimits,
    ) -> Self {
        let data_dir = tempfile::tempdir().expect("a temporary directory");
        let config = ServerConfig::builder()
            .data_dir(data_dir.path())
            .workers(std::num::NonZeroUsize::MIN)
            .build();
        let server = timeout(
            WAIT,
            Server::start_with_handler(config, Arc::new(handler), limits),
        )
        .await
        .expect("timed out starting the server")
        .expect("the server starts");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind port 0");
        let addr = listener
            .local_addr()
            .expect("a bound listener has an address");
        let (stop_serving, stopped) = oneshot::channel::<()>();
        let serving = tokio::spawn(
            axum::serve(
                listener,
                server
                    .router()
                    .into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                // A dropped sender stops serving too.
                let _ = stopped.await;
            })
            .into_future(),
        );
        Self {
            addr,
            url: format!("ws://{addr}/ws"),
            server,
            stop_serving,
            serving,
            _data_dir: data_dir,
        }
    }

    /// The server's shared state.
    pub(crate) fn state(&self) -> &Arc<AppState> {
        self.server.state()
    }

    /// The server, for its statistics.
    pub(crate) fn server(&self) -> &Server {
        &self.server
    }

    /// The WebSocket URL.
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    /// The listening address.
    pub(crate) fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// A new client, connected.
    pub(crate) async fn connect(&self) -> Client {
        Client::connect(&self.url).await
    }

    /// A new client whose socket receives into a buffer of [`SLOW_READER_BUFFER_BYTES`], so that
    /// a few megabytes it does not read are enough to stop the server's writer.
    pub(crate) async fn connect_slow_reader(&self) -> Client {
        let socket = tokio::net::TcpSocket::new_v4().expect("a TCP socket");
        // Set before connecting, so that the window the client advertises is small from the start
        // and the kernel does not grow it.
        socket
            .set_recv_buffer_size(SLOW_READER_BUFFER_BYTES)
            .expect("the receive buffer can be set");
        let stream = timeout(WAIT, socket.connect(self.addr))
            .await
            .expect("timed out connecting")
            .expect("the server accepts the connection");
        let (socket, _) = timeout(
            WAIT,
            tokio_tungstenite::client_async(self.url.as_str(), MaybeTlsStream::Plain(stream)),
        )
        .await
        .expect("timed out upgrading")
        .expect("the server upgrades the connection");
        Client { socket }
    }

    /// Waits until the request counters satisfy `condition`, and returns them.
    pub(crate) async fn requests_until(
        &self,
        condition: impl FnMut(&RequestCounters) -> bool,
    ) -> RequestCounters {
        let mut counters = self.state().request_stats.subscribe();
        *timeout(WAIT, counters.wait_for(condition))
            .await
            .expect("timed out waiting on the request counters")
            .expect("the counters live as long as the server")
    }

    /// Waits until the outbound counters satisfy `condition`, and returns them.
    pub(crate) async fn outbound_until(
        &self,
        condition: impl FnMut(&OutboundCounters) -> bool,
    ) -> OutboundCounters {
        let mut counters = self.state().outbound_stats.subscribe();
        *timeout(WAIT, counters.wait_for(condition))
            .await
            .expect("timed out waiting on the outbound counters")
            .expect("the counters live as long as the server")
    }

    /// Makes `client`'s server-side writer stick: sends request `id`, answered through `calls`
    /// with a response of [`CLOGGING_BYTES`], which `client`, a slow reader, does not read.
    /// Returns once the response has been counted, just before it is queued, and gives the
    /// length of its frame, [`clogging_frame_len`].
    pub(crate) async fn stick_writer(
        &self,
        calls: &mut Calls,
        client: &mut Client,
        id: u32,
    ) -> usize {
        let responded = self.server().stats().requests().responded();
        client.request(id, body(id)).await;
        let call = calls.next().await;
        assert_eq!(call.id(), id, "the request that sticks the writer");
        assert!(call.respond(bulky_response(CLOGGING_BYTES)));
        self.requests_until(|counters| counters.responded() > responded)
            .await;
        clogging_frame_len(id)
    }

    /// Waits until `count` connections are open.
    pub(crate) async fn connections_until(&self, count: usize) {
        timeout(WAIT, self.state().connections.wait_until_open(count))
            .await
            .expect("timed out waiting for the connection count");
    }

    /// Stops serving and shuts the server down, as `main` does, then checks that nothing outlived
    /// it: no connection, no request in flight, nothing queued for a client or held back, no job
    /// queued or running, and every accepted request ended exactly one way.
    pub(crate) async fn stop(self) {
        let shared = Arc::clone(self.state());
        // The serving task may have ended already, and then nobody listens.
        let _ = self.stop_serving.send(());
        timeout(WAIT, self.serving)
            .await
            .expect("timed out stopping serving")
            .expect("the serving task does not panic")
            .expect("serving ends without error");
        timeout(WAIT, self.server.shutdown())
            .await
            .expect("timed out shutting the server down")
            .expect("the server shuts down cleanly");
        let stats = ServerStats::new(
            shared.connections.open_count(),
            shared.request_stats.snapshot(),
            shared.outbound_stats.snapshot(),
            shared.pool.counters(),
            shared.galaxies.counters(),
            shared.maps.counters(),
        );
        let (requests, outbound, pool) = (stats.requests(), stats.outbound(), stats.pool());
        assert_eq!(
            (
                stats.connections(),
                requests.in_flight(),
                outbound.queued_bytes(),
                outbound.held_requests(),
                pool.queued_interactive(),
                pool.queued_bulk(),
                pool.running(),
            ),
            (0, 0, 0, 0, 0, 0, 0),
            "something outlived the server: {stats:?}"
        );
        assert_eq!(
            requests.accepted(),
            requests.responded() + requests.failed() + requests.cancelled() + requests.abandoned(),
            "every accepted request ended exactly one way: {requests:?}"
        );
    }
}

/// The receive buffer of [`Harness::connect_slow_reader`]'s socket, in bytes. Linux doubles it
/// for its own bookkeeping and keeps it from growing.
pub(crate) const SLOW_READER_BUFFER_BYTES: u32 = 4096;

/// A response larger than a slow reader's socket and the server's send buffer hold together:
/// Linux grows a send buffer to 4 MiB at most by default, and the slow reader's receive buffer
/// does not grow. Written to a slow reader, it sticks the connection's writer.
pub(crate) const CLOGGING_BYTES: usize = 8 << 20;

/// The payload bytes of the frame that answers request `id` with a response of
/// [`CLOGGING_BYTES`], as [`response_frame_len`] gives them.
///
/// Worked out without serialising megabytes, which in a debug build takes long enough for a
/// test's write timeout to run out: the response's name is all `x`, which JSON does not escape,
/// so each byte of it is one byte of the frame.
pub(crate) fn clogging_frame_len(id: u32) -> usize {
    response_frame_len(id, bulky_response(0)) + CLOGGING_BYTES
}

/// The payload bytes of the frame that answers request `id` with `body`: what the outbound queue
/// is charged for it.
pub(crate) fn response_frame_len(id: u32, body: ResponseBody) -> usize {
    to_frame(&ServerMessage::Response {
        id: RequestId(id),
        body,
    })
    .len()
}

/// A WebSocket client of a [`Harness`].
#[derive(Debug)]
pub(crate) struct Client {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Client {
    async fn connect(url: &str) -> Self {
        let (socket, _) = timeout(WAIT, connect_async(url))
            .await
            .expect("timed out connecting")
            .expect("the server accepts the connection");
        Self { socket }
    }

    /// Sends `message` as JSON.
    pub(crate) async fn send(&mut self, message: &ClientMessage) {
        let json = serde_json::to_string(message).expect("client messages serialise");
        self.send_raw(&json).await;
    }

    /// Sends `text` as one text frame.
    pub(crate) async fn send_raw(&mut self, text: &str) {
        timeout(WAIT, self.socket.send(Message::text(text)))
            .await
            .expect("timed out sending")
            .expect("the connection is open");
    }

    /// Sends a request without waiting for its answer.
    pub(crate) async fn request(&mut self, id: u32, body: RequestBody) {
        self.send(&ClientMessage::Request {
            id: RequestId(id),
            body,
        })
        .await;
    }

    /// Cancels request `id`.
    pub(crate) async fn cancel(&mut self, id: u32) {
        self.send(&ClientMessage::Cancel { id: RequestId(id) })
            .await;
    }

    /// Says hello and checks the answer.
    pub(crate) async fn hello(&mut self) {
        self.send(&ClientMessage::Hello {
            client_version: "unit test".to_owned(),
        })
        .await;
        let welcome = self.next_message().await;
        assert!(
            matches!(welcome, ServerMessage::Welcome { .. }),
            "expected a welcome, got {welcome:?}"
        );
    }

    /// Pings and waits for the pong, returning every message that arrived before it.
    pub(crate) async fn ping(&mut self, nonce: u32) -> Vec<ServerMessage> {
        self.send(&ClientMessage::Ping { nonce }).await;
        let mut before = Vec::new();
        loop {
            match self.next_message().await {
                ServerMessage::Pong { nonce: echoed } if echoed == nonce => return before,
                other => before.push(other),
            }
        }
    }

    /// The next message from the server.
    pub(crate) async fn next_message(&mut self) -> ServerMessage {
        loop {
            let frame = timeout(WAIT, self.socket.next())
                .await
                .expect("timed out waiting for a message")
                .expect("the server closed the connection")
                .expect("the connection is healthy");
            match frame {
                Message::Text(text) => {
                    return serde_json::from_str(&text).expect("the server sends valid messages");
                }
                Message::Ping(_) | Message::Pong(_) => {}
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    /// Sends a close frame, without reading.
    pub(crate) async fn send_close(&mut self) {
        timeout(WAIT, self.socket.close(None))
            .await
            .expect("timed out closing")
            .expect("the close frame is sent");
    }

    /// Reads and discards every frame until the connection ends, answering the server's close
    /// frame if it sends one.
    pub(crate) async fn drain(&mut self) {
        timeout(WAIT, async {
            while let Some(Ok(_)) = self.socket.next().await {}
        })
        .await
        .expect("timed out waiting for the connection to end");
    }

    /// Closes the connection with a close frame, and reads until the server has closed it too.
    /// Panics on a text frame.
    pub(crate) async fn close(mut self) -> Option<u16> {
        timeout(WAIT, self.socket.close(None))
            .await
            .expect("timed out closing")
            .expect("the close frame is sent");
        self.closed().await
    }

    /// Reads until the server has closed the connection, answering its close frame, and returns
    /// the close code it sent, if it sent one. Panics on a text frame.
    pub(crate) async fn closed(&mut self) -> Option<u16> {
        timeout(WAIT, async {
            let mut code = None;
            loop {
                match self.socket.next().await {
                    // Reading on after the close frame sends the answering one.
                    Some(Ok(Message::Close(frame))) => {
                        code = frame.map(|frame| u16::from(frame.code));
                    }
                    None | Some(Err(_)) => return code,
                    Some(Ok(Message::Text(text))) => {
                        panic!("expected the connection to close, got {text}")
                    }
                    Some(Ok(_)) => {}
                }
            }
        })
        .await
        .expect("timed out waiting for the connection to close")
    }
}

/// A handler that hands every call to the test, which answers each one by hand.
///
/// A call the test drops unanswered never ends by itself, like a long computation.
#[derive(Debug)]
pub(crate) struct Scripted {
    calls: mpsc::Sender<Call>,
}

impl Scripted {
    /// The handler, and the calls it receives.
    pub(crate) fn new() -> (Self, Calls) {
        let (calls, received) = mpsc::channel(64);
        (Self { calls }, Calls(received))
    }
}

impl Handler for Scripted {
    fn handle(
        &self,
        _state: Arc<AppState>,
        body: RequestBody,
        token: CancelToken,
    ) -> HandlerFuture {
        let calls = self.calls.clone();
        Box::pin(async move {
            let (reply, answer) = oneshot::channel();
            calls
                .send(Call { body, token, reply })
                .await
                .expect("the test is taking calls");
            match answer.await {
                Ok(Answer::Respond(body)) => Ok(body),
                Ok(Answer::Fail(error)) => Err(error),
                Ok(Answer::Panic) => panic!("a deliberate panic in a handler"),
                Err(_) => std::future::pending().await,
            }
        })
    }
}

/// The calls a [`Scripted`] handler has received.
#[derive(Debug)]
pub(crate) struct Calls(mpsc::Receiver<Call>);

impl Calls {
    /// The next call.
    pub(crate) async fn next(&mut self) -> Call {
        timeout(WAIT, self.0.recv())
            .await
            .expect("timed out waiting for a call")
            .expect("the handler lives as long as the server")
    }

    /// Whether no call is waiting.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One request, as the handler received it.
#[derive(Debug)]
pub(crate) struct Call {
    /// What was asked.
    pub(crate) body: RequestBody,
    /// The request's cancellation token.
    pub(crate) token: CancelToken,
    reply: oneshot::Sender<Answer>,
}

#[derive(Debug)]
enum Answer {
    Respond(ResponseBody),
    Fail(RequestError),
    Panic,
}

impl Call {
    /// The request ID a test encoded in the call's body with [`body`].
    pub(crate) fn id(&self) -> u32 {
        match &self.body {
            RequestBody::OpenUniverse(request) => {
                u32::try_from(request.universe.to_u64()).expect("a test's request id")
            }
            other => panic!("not a body made by `body`: {other:?}"),
        }
    }

    /// Answers with `body`. Returns whether the request's task was still waiting.
    pub(crate) fn respond(self, body: ResponseBody) -> bool {
        self.reply.send(Answer::Respond(body)).is_ok()
    }

    /// Answers with `error`.
    pub(crate) fn fail(self, error: RequestError) -> bool {
        self.reply.send(Answer::Fail(error)).is_ok()
    }

    /// Makes the handler panic.
    pub(crate) fn panic(self) -> bool {
        self.reply.send(Answer::Panic).is_ok()
    }

    /// Waits until the request's task has let go of the call: it was cancelled, or its
    /// connection closed.
    pub(crate) async fn dropped(&mut self) {
        timeout(WAIT, self.reply.closed())
            .await
            .expect("timed out waiting for the request's task to stop");
    }
}

/// A request body that carries `id`, so that a test can tell which request a call is.
pub(crate) fn body(id: u32) -> RequestBody {
    RequestBody::OpenUniverse(hyperion_protocol::OpenUniverseRequest {
        universe: hyperion_protocol::UniverseIdHex::from_u64(u64::from(id)),
    })
}

/// A small response body.
pub(crate) fn small_response() -> ResponseBody {
    ResponseBody::ListUniverses(hyperion_protocol::UniverseList {
        universes: Vec::new(),
        server_generator_version: hyperion_sim::GENERATOR_VERSION.get(),
    })
}

/// A response body whose frame is a little over `bytes` long, serialised on the runtime (its kind
/// is not one of the large ones), for filling a socket.
pub(crate) fn bulky_response(bytes: usize) -> ResponseBody {
    ResponseBody::ListUniverses(hyperion_protocol::UniverseList {
        universes: vec![hyperion_protocol::UniverseInfo {
            id: hyperion_protocol::UniverseIdHex::from_u64(1),
            name: "x".repeat(bytes),
            seed: hyperion_protocol::SeedHex::from_u64(1),
            generator_version: hyperion_sim::GENERATOR_VERSION.get(),
            status: hyperion_protocol::UniverseStatus::Compatible,
        }],
        server_generator_version: hyperion_sim::GENERATOR_VERSION.get(),
    })
}
