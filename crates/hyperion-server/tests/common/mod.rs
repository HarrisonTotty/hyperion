//! A real server on a loopback port and a WebSocket client for it, shared by the integration
//! tests. Every wait is bounded by [`NETWORK_TIMEOUT`], so that a hung server fails a test instead
//! of the suite.
#![allow(
    dead_code,
    reason = "each test binary compiles this module and uses its own subset of the helpers"
)]

use std::future::IntoFuture;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use hyperion_protocol::{
    ClientMessage, RequestBody, RequestError, RequestId, ResponseBody, ServerMessage,
};
use hyperion_server::universe::SequenceEntropy;
use hyperion_server::{Server, ServerConfig, ServerConfigBuilder, ServerStats};
use tempfile::TempDir;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

/// Upper bound on any single network wait.
///
/// Generous, because the answer waited for may be one that builds a galaxy, and `open_universe`
/// always is (plan 04, design note 6). A build is about 130 ms in a release build (plan 02, Risks,
/// R19), but these tests run under `just test`, where `hyperion-sim` is `opt-level = 2` with debug
/// assertions and overflow checks still on, and `cargo test` runs a binary's tests in parallel, so
/// several galaxies are built at once. Measured over a loopback socket on eight cores at load
/// average 14 (2026-09-22): one cold `open_universe` took 0.85–0.93 s on its own and up to 2.04 s
/// with twelve running together, against 0.5 ms warm and under 20 ms for every other wait here.
/// Twenty seconds is ten times the worst of that, and still turns a hung server into a failed test
/// rather than a hung suite.
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(20);

/// Upper bound on a server's teardown, which is not a network wait.
///
/// Shutting the CPU pool down drops what is queued and then waits for the jobs in hand, and a job
/// cannot be aborted (plan 04, design note 5). One band of a 1,024-pixel edge-on map is the longest
/// of them, seconds of work and more on a loaded machine, so a shutdown is given far longer than a
/// message.
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(300);

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
        let server = timeout(NETWORK_TIMEOUT, Server::start(config))
            .await
            .expect("timed out starting the server")
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
        }
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

    /// The running server's statistics.
    pub fn stats(&self) -> ServerStats {
        self.server
            .as_ref()
            .expect("the server is running until `stop`")
            .stats()
    }

    /// Stops serving, waits for the serving task, then shuts the server down. Returns the
    /// temporary data directory, if the server made one, so that another server can start on it.
    pub async fn stop(mut self) -> Option<TempDir> {
        if let Some(stop_serving) = self.stop_serving.take() {
            // The serving task may have ended already, and then nobody listens.
            let _ = stop_serving.send(());
        }
        if let Some(serving) = self.serving.take() {
            timeout(NETWORK_TIMEOUT, serving)
                .await
                .expect("timed out stopping the server")
                .expect("the serving task does not panic")
                .expect("serving ends without error");
        }
        if let Some(server) = self.server.take() {
            timeout(SHUTDOWN_TIMEOUT, server.shutdown())
                .await
                .expect("timed out shutting the server down")
                .expect("the server shuts down cleanly");
        }
        self.temp_dir.take()
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
        let (socket, _) = timeout(NETWORK_TIMEOUT, connect_async(url))
            .await
            .expect("timed out connecting")
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
        timeout(NETWORK_TIMEOUT, self.socket.send(Message::text(text)))
            .await
            .expect("timed out sending")
            .expect("the connection is open");
    }

    /// The next message from the server, skipping WebSocket pings and pongs.
    pub async fn next_message(&mut self) -> ServerMessage {
        loop {
            let frame = timeout(NETWORK_TIMEOUT, self.socket.next())
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
        timeout(
            NETWORK_TIMEOUT,
            self.socket.send(Message::binary(bytes.to_vec())),
        )
        .await
        .expect("timed out sending")
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

    /// Cancels request `id`.
    pub async fn cancel(&mut self, id: RequestId) {
        self.send(&ClientMessage::Cancel { id }).await;
    }

    /// Sends a close frame, then reads until the server has closed the connection. Panics on a
    /// text frame.
    pub async fn close(mut self) {
        timeout(NETWORK_TIMEOUT, self.socket.close(None))
            .await
            .expect("timed out closing")
            .expect("the close frame is sent");
        self.closed().await;
    }

    /// Reads until the server has ended the connection, whether with a close frame, an error or
    /// the end of the stream, and returns the close code the server sent, if it sent one. Reading
    /// on after the server's close frame sends the answering one. Panics on a text frame.
    pub async fn closed(&mut self) -> Option<u16> {
        timeout(NETWORK_TIMEOUT, async {
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
        .expect("timed out waiting for the connection to close")
    }
}
