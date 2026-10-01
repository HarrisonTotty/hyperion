//! A real server on a loopback port and a WebSocket client for it, shared by the integration
//! tests. Every wait is bounded by [`patience`], so that a hung server fails a test instead of the
//! suite.
#![allow(
    dead_code,
    reason = "each test binary compiles this module and uses its own subset of the helpers"
)]

use std::future::IntoFuture;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::serve::ListenerExt;
use futures_util::{SinkExt, StreamExt};
use hyperion_protocol::{
    ClientMessage, CreateUniverseRequest, NotificationBody, RequestBody, RequestError, RequestId,
    ResponseBody, SeedHex, ServerMessage, UniverseInfo,
};
use hyperion_server::universe::SequenceEntropy;
use hyperion_server::{Server, ServerConfig, ServerConfigBuilder, ServerStats};
use tempfile::TempDir;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

/// The environment variable that sets [`patience`], in whole seconds.
pub const PATIENCE_ENV: &str = "HYPERION_TEST_PATIENCE_SECS";

/// [`patience`] when [`PATIENCE_ENV`] is not set.
///
/// Every wait here is on work that a loaded machine only makes slower, never wrong: a galaxy build,
/// a query, a band of a map, a teardown. `open_universe` always builds a galaxy (plan 04, design
/// note 6), about 130 ms in a release build (plan 02, Risks, R19), but these tests run under
/// `just test`, where `hyperion-sim` is `opt-level = 2` with debug assertions and overflow checks
/// still on, and `cargo test` runs a binary's tests in parallel, so as many galaxies are built at
/// once as the binary has tests. Measured over a loopback socket on eight cores at load average 14
/// (2026-09-22): one cold `open_universe` took 0.85–0.93 s on its own and up to 2.04 s with twelve
/// running together. At load average 20–22 with other worktrees building (2026-09-29), the nine
/// tests of `system_bodies` each waited over 20 s for their first answers, and two runs in six
/// failed at the 20 s this bound once was. The longest job the server cannot abort, which a
/// teardown waits for, is one band of 16 rows of a 1,024-pixel edge-on map, about 13 s on an idle
/// machine (plan 04, "Measured map costs"), and so the longest wait here.
///
/// Three minutes is some six times the worst of those under the load that failed, and still turns
/// a hung server into a failed test rather than a hung suite. No assertion depends on it: a test of
/// the server's own timing states its bound itself (`tests/abuse.rs`'s shutdown bound).
pub const DEFAULT_PATIENCE: Duration = Duration::from_secs(180);

/// Upper bound on any single wait of the harness: a message, a connection, a state of the server's
/// counters, a teardown.
///
/// This is the harness's patience and not a property of the server, so it can be raised for a
/// machine slower or busier than any it was measured on: [`PATIENCE_ENV`] gives it in whole
/// seconds, and it is [`DEFAULT_PATIENCE`] otherwise. Read once per test binary.
///
/// # Panics
///
/// If [`PATIENCE_ENV`] is set and is not a whole number of seconds above zero.
pub fn patience() -> Duration {
    static PATIENCE: OnceLock<Duration> = OnceLock::new();
    *PATIENCE.get_or_init(|| {
        let Some(secs) = std::env::var_os(PATIENCE_ENV) else {
            return DEFAULT_PATIENCE;
        };
        match secs
            .to_str()
            .and_then(|secs| secs.trim().parse::<u64>().ok())
        {
            Some(secs) if secs > 0 => Duration::from_secs(secs),
            _ => panic!(
                "{PATIENCE_ENV} is {}, and must be a whole number of seconds above 0",
                secs.display()
            ),
        }
    })
}

/// Awaits `future` for up to [`patience`], and panics naming `what` was being waited for if it
/// takes longer.
pub async fn patiently<T>(what: &str, future: impl Future<Output = T>) -> T {
    let bound = patience();
    timeout(bound, future).await.unwrap_or_else(|_| {
        panic!("timed out after {bound:?} {what}; {PATIENCE_ENV} sets a longer wait, in seconds")
    })
}

/// How often [`TestServer::stats_until`] looks at the server's counters again.
///
/// Short beside every state it waits for (a job reaching a worker, a connection closing, a queue
/// draining), and long enough that the polling costs nothing measurable.
const STATS_POLL: Duration = Duration::from_millis(5);

/// The values a test server's entropy hands out, in order: seeds that a create leaves out, then
/// universe IDs, as they are drawn.
pub fn test_entropy() -> impl Iterator<Item = u64> {
    (1..=256).map(|n| 0x5eed_0000_0000_0000 | n)
}

/// A server listening on `127.0.0.1` at a port the OS chose.
#[derive(Debug)]
pub struct TestServer {
    url: String,
    data_dir: PathBuf,
    server: Option<Server>,
    stop_serving: Option<oneshot::Sender<()>>,
    serving: Option<JoinHandle<std::io::Result<()>>>,
    temp_dir: Option<TempDir>,
    /// The low-water mark of the socket accepted last (rendering plan R03, R03.T10.a).
    lowat: Arc<AtomicU32>,
}

/// A socket's `TCP_NOTSENT_LOWAT`, 0 where the option does not exist.
fn read_lowat(tcp: &tokio::net::TcpStream) -> u32 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    return socket2::SockRef::from(tcp)
        .tcp_notsent_lowat()
        .expect("the option can be read");
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = tcp;
        0
    }
}

impl TestServer {
    /// A server over a fresh temporary data directory, with [`test_entropy`].
    pub async fn start() -> Self {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");
        let mut server = Self::start_with(Self::config(temp_dir.path()).build()).await;
        server.temp_dir = Some(temp_dir);
        server
    }

    /// The configuration [`TestServer::start`] uses, over `data_dir`, for tests to adjust.
    pub fn config(data_dir: &Path) -> ServerConfigBuilder {
        ServerConfig::builder()
            .data_dir(data_dir)
            .entropy(SequenceEntropy::new(test_entropy()))
    }

    /// A server with `config`, listening on port 0 whatever its address says.
    pub async fn start_with(config: ServerConfig) -> Self {
        let data_dir = config.data_dir().to_path_buf();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind port 0");
        let addr = listener
            .local_addr()
            .expect("a bound listener has an address");
        let lowat = Arc::new(AtomicU32::new(0));
        let tapped = Arc::clone(&lowat);
        let listener = listener.tap_io(move |tcp| {
            hyperion_server::tap_socket(tcp);
            tapped.store(read_lowat(tcp), Ordering::Release);
        });
        let server = patiently("starting the server", Server::start(config))
            .await
            .expect("the server starts");
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
            url: format!("ws://{addr}/ws"),
            data_dir,
            server: Some(server),
            stop_serving: Some(stop_serving),
            serving: Some(serving),
            temp_dir: None,
            lowat,
        }
    }

    /// The `TCP_NOTSENT_LOWAT` read back from the socket the server accepted last, 0 before any
    /// or where the option does not exist.
    pub fn accepted_lowat(&self) -> u32 {
        self.lowat.load(Ordering::Acquire)
    }

    /// The WebSocket URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// A new client connected to this server.
    pub async fn connect(&self) -> TestClient {
        TestClient::connect(&self.url).await
    }

    /// A new client that has said hello, ready to make requests.
    pub async fn connected(&self) -> TestClient {
        let mut client = self.connect().await;
        client.hello().await;
        client
    }

    /// The running server's statistics.
    pub fn stats(&self) -> ServerStats {
        self.server
            .as_ref()
            .expect("the server is running until `stop`")
            .stats()
    }

    /// Waits until the running server's statistics satisfy `condition`, and returns the snapshot
    /// that did.
    ///
    /// The plan has these tests read [`ServerStats`] rather than guess at timing (P04.T13.c): a
    /// test waits until the server itself says a job has reached a worker, a queue has drained or a
    /// connection has gone. Most of those counters are a snapshot and not a watch, so this polls
    /// them every [`STATS_POLL`], bounded by [`patience`] so that a state which never arrives fails
    /// the test instead of hanging the suite. `what` is what the caller is waiting for, named for
    /// the panic message.
    pub async fn stats_until(
        &self,
        what: &str,
        condition: impl Fn(&ServerStats) -> bool,
    ) -> ServerStats {
        let bound = patience();
        let waited = timeout(bound, async {
            loop {
                let stats = self.stats();
                if condition(&stats) {
                    return stats;
                }
                sleep(STATS_POLL).await;
            }
        })
        .await;
        waited.unwrap_or_else(|_| {
            panic!(
                "timed out after {bound:?} waiting until {what}; the server reports {:?}",
                self.stats()
            )
        })
    }

    /// Stops the serving task and waits for it, which every teardown here does first. Serving ends
    /// without waiting for the connections it upgraded, which is why the server needs a teardown of
    /// its own (P04.T13).
    async fn end_serving(&mut self) {
        if let Some(stop_serving) = self.stop_serving.take() {
            // The serving task may have ended already, and then nobody listens.
            let _ = stop_serving.send(());
        }
        if let Some(serving) = self.serving.take() {
            patiently("stopping the server", serving)
                .await
                .expect("the serving task does not panic")
                .expect("serving ends without error");
        }
    }

    /// Stops serving, waits for the serving task, then shuts the server down. Returns the
    /// temporary data directory, if the server made one, so that another server can start on it.
    pub async fn stop(mut self) -> Option<TempDir> {
        self.end_serving().await;
        if let Some(server) = self.server.take() {
            patiently("shutting the server down", server.shutdown())
                .await
                .expect("the server shuts down cleanly");
        }
        self.temp_dir.take()
    }

    /// Stops serving, then shuts the server down within `bound` rather than [`patience`].
    ///
    /// The abuse suite holds a shutdown with work queued to a bound of its own (P04.T15), so the
    /// bound is that test's assertion and has to be the caller's to choose: this panics if the
    /// shutdown takes longer, or does not succeed. Only the shutdown is timed against `bound`; the
    /// serving task is stopped first, under [`patience`]. Clients need not have closed, since
    /// closing them is the shutdown's own work.
    pub async fn stop_within(mut self, bound: Duration) {
        self.end_serving().await;
        let server = self
            .server
            .take()
            .expect("the server is running until `stop`");
        timeout(bound, server.shutdown())
            .await
            .unwrap_or_else(|_| panic!("the server did not shut down within {bound:?}"))
            .expect("the server shuts down cleanly");
    }

    /// Stops this server and starts another on the same data directory with a fresh
    /// [`test_entropy`], as restarting the process would. The new server keeps the temporary data
    /// directory, if this one made it.
    pub async fn restart(self) -> Self {
        let data_dir = self.data_dir.clone();
        let temp_dir = self.stop().await;
        let mut server = Self::start_with(Self::config(&data_dir).build()).await;
        server.temp_dir = temp_dir;
        server
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // A test that did not call `stop` still leaves no serving task behind.
        if let Some(serving) = self.serving.take() {
            serving.abort();
        }
    }
}

/// A compact JSON frame laid out as `serde_json::to_string_pretty` lays it out, two spaces an
/// indent, with every string and number copied as it arrived.
///
/// This is how a golden pins a frame the server sent without a decoder in between: `serde_json`
/// without its `float_roundtrip` feature can read a float a last bit out, so a golden written from
/// a parsed and re-printed message can miss a one-ulp move on the server. The frame must be
/// compact, as the server writes it, with no whitespace outside strings.
///
/// # Panics
///
/// If the brackets are unbalanced.
pub fn pretty_json_frame(compact: &str) -> String {
    fn break_line(out: &mut String, depth: usize) {
        out.push('\n');
        for _ in 0..depth {
            out.push_str("  ");
        }
    }

    let mut out = String::with_capacity(2 * compact.len());
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = compact.chars().peekable();
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '{' | '[' => {
                out.push(c);
                let close = if c == '{' { '}' } else { ']' };
                if chars.next_if_eq(&close).is_some() {
                    out.push(close);
                } else {
                    depth += 1;
                    break_line(&mut out, depth);
                }
            }
            '}' | ']' => {
                depth = depth.checked_sub(1).expect("the frame's brackets balance");
                break_line(&mut out, depth);
                out.push(c);
            }
            ',' => {
                out.push(c);
                break_line(&mut out, depth);
            }
            ':' => out.push_str(": "),
            other => out.push(other),
        }
    }
    assert_eq!(depth, 0, "the frame's brackets balance");
    out
}

/// A WebSocket client speaking `hyperion-protocol`.
#[derive(Debug)]
pub struct TestClient {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// The ID of the next request, counting from 1 as the bridge client does.
    next_id: u32,
}

impl TestClient {
    /// Connects to `url`.
    pub async fn connect(url: &str) -> Self {
        let (socket, _) = patiently("connecting", connect_async(url))
            .await
            .expect("the server accepts the connection");
        Self { socket, next_id: 1 }
    }

    /// Sends `message` as JSON.
    pub async fn send(&mut self, message: &ClientMessage) {
        let json = serde_json::to_string(message).expect("client messages serialise");
        self.send_raw(&json).await;
    }

    /// Sends `text` as one text frame, as it is.
    pub async fn send_raw(&mut self, text: &str) {
        patiently("sending", self.socket.send(Message::text(text)))
            .await
            .expect("the connection is open");
    }

    /// The next message from the server, skipping WebSocket pings and pongs.
    pub async fn next_message(&mut self) -> ServerMessage {
        let text = self.next_text().await;
        serde_json::from_str(&text).expect("the server sends valid messages")
    }

    /// The text of the next frame from the server, skipping WebSocket pings and pongs: the bytes as
    /// they arrived, for a test that pins a response exactly rather than through a decoder.
    pub async fn next_text(&mut self) -> String {
        loop {
            let frame = patiently("waiting for a message", self.socket.next())
                .await
                .expect("the server closed the connection")
                .expect("the connection is healthy");
            match frame {
                Message::Text(text) => return text.to_string(),
                Message::Ping(_) | Message::Pong(_) => {}
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    /// The next message from the server, which must be a `notification`: its subscription and
    /// body (plan 12's P12.T9, built by rendering plan R03's R03.T5.b). Panics on any other
    /// message, since a test that expects one should read it itself.
    pub async fn next_notification(&mut self) -> (u32, NotificationBody) {
        match self.next_message().await {
            ServerMessage::Notification { subscription, body } => (subscription, body),
            other => panic!("expected a notification, got {other:?}"),
        }
    }

    /// Says hello and returns the server's answer.
    pub async fn hello(&mut self) -> ServerMessage {
        self.send(&ClientMessage::Hello {
            client_version: "test".to_owned(),
        })
        .await;
        self.next_message().await
    }

    /// Pings and returns the server's answer.
    pub async fn ping(&mut self, nonce: u32) -> ServerMessage {
        self.send(&ClientMessage::Ping { nonce }).await;
        self.next_message().await
    }

    /// Sends `bytes` as one binary frame.
    pub async fn send_binary(&mut self, bytes: &[u8]) {
        patiently("sending", self.socket.send(Message::binary(bytes.to_vec())))
            .await
            .expect("the connection is open");
    }

    /// Sends a request under the next ID and returns the ID, without waiting for the answer.
    pub async fn send_request(&mut self, body: RequestBody) -> RequestId {
        let id = RequestId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("fewer than 2³² requests");
        self.send(&ClientMessage::Request { id, body }).await;
        id
    }

    /// Makes a request and waits for its terminal message: the response's body, or the error.
    /// Panics on any other message, since a test that expects one should read it itself.
    pub async fn request(&mut self, body: RequestBody) -> Result<ResponseBody, RequestError> {
        let id = self.send_request(body).await;
        match self.next_message().await {
            ServerMessage::Response { id: answered, body } if answered == id => Ok(body),
            ServerMessage::RequestError {
                id: answered,
                error,
            } if answered == id => Err(error),
            other => panic!("expected the answer to request {}, got {other:?}", id.0),
        }
    }

    /// Makes a request and waits for its terminal message as [`TestClient::request`] does, keeping
    /// the notifications that arrive before it, in order, for a client with subscriptions open.
    pub async fn request_among_notifications(
        &mut self,
        body: RequestBody,
    ) -> (
        Result<ResponseBody, RequestError>,
        Vec<(u32, NotificationBody)>,
    ) {
        let id = self.send_request(body).await;
        let mut notifications = Vec::new();
        loop {
            match self.next_message().await {
                ServerMessage::Response { id: answered, body } if answered == id => {
                    return (Ok(body), notifications);
                }
                ServerMessage::RequestError {
                    id: answered,
                    error,
                } if answered == id => return (Err(error), notifications),
                ServerMessage::Notification { subscription, body } => {
                    notifications.push((subscription, body));
                }
                other => panic!("expected the answer to request {}, got {other:?}", id.0),
            }
        }
    }

    /// Creates a universe named `name` from `seed` and returns what the server made of it. Panics
    /// on a refusal, so a test that expects one makes the request itself.
    pub async fn create_universe(&mut self, name: &str, seed: u64) -> UniverseInfo {
        let body = RequestBody::CreateUniverse(CreateUniverseRequest {
            name: name.to_owned(),
            seed: Some(SeedHex::from_u64(seed)),
        });
        match self.request(body).await {
            Ok(ResponseBody::CreateUniverse(info)) => info,
            other => panic!("expected the created universe, got {other:?}"),
        }
    }

    /// Cancels request `id`.
    pub async fn cancel(&mut self, id: RequestId) {
        self.send(&ClientMessage::Cancel { id }).await;
    }

    /// Sends a close frame, then reads until the server has closed the connection. Panics on a
    /// text frame.
    pub async fn close(mut self) {
        patiently("closing", self.socket.close(None))
            .await
            .expect("the close frame is sent");
        self.closed().await;
    }

    /// Reads until the server has ended the connection, whether with a close frame, an error or
    /// the end of the stream, and returns the close code the server sent, if it sent one. Reading
    /// on after the server's close frame sends the answering one. Panics on a text frame.
    pub async fn closed(&mut self) -> Option<u16> {
        patiently("waiting for the connection to close", async {
            let mut code = None;
            loop {
                match self.socket.next().await {
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
    }
}
