//! HYPERION game server: hosts the simulation and serves bridge clients.
//!
//! The server owns everything the sim may not: universes and their saves ([`universe`]), the CPU
//! pool that runs generation off the async runtime ([`compute`]), the caches of generated data
//! ([`cache`]), and the WebSocket clients speak `hyperion-protocol` over. Every limit it enforces
//! is in [`limits`].
//!
//! [`Server::start`] builds the shared state from a [`ServerConfig`], [`Server::router`] serves
//! it, [`Server::stats`] reports on it, and [`Server::shutdown`] is the explicit teardown once
//! serving has stopped.

pub mod cache;
pub mod compute;
pub mod config;
mod connections;
pub mod limits;
mod requests;
mod stats;
#[cfg(test)]
mod testing;
pub mod universe;
mod ws;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, io};

use axum::{Router, routing::get};

use crate::compute::{CpuPool, ShutDownPoolError, StartPoolError};
use crate::connections::Connections;
use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use crate::requests::{Handler, Handlers};
use crate::stats::RequestStats;

pub use config::{ParseConfigError, ServerConfig, ServerConfigBuilder};
pub use stats::{RequestCounters, ServerStats};

/// Address the server listens on when `HYPERION_ADDR` is not set.
pub const DEFAULT_ADDR: &str = "127.0.0.1:7878";

/// A running server's shared state and the router that serves it.
///
/// Serving is the caller's: hand [`Server::router`] to `axum::serve` with a listener (with
/// `into_make_service_with_connect_info::<SocketAddr>()` to have peers' addresses logged), and
/// once serving has stopped, call [`Server::shutdown`].
#[derive(Debug)]
pub struct Server {
    state: Arc<AppState>,
}

/// The state every connection and request shares.
///
/// The universe registry and the caches join it with the handlers that need them (P04.T14).
#[derive(Debug)]
pub(crate) struct AppState {
    /// Where generation, and the serialisation of large responses, runs: never on the runtime.
    pub(crate) pool: CpuPool,
    /// What answers each request: [`Handlers`], or a test's double.
    pub(crate) handler: Arc<dyn Handler>,
    /// The open WebSocket connections, which shutdown closes and waits for.
    pub(crate) connections: Connections,
    /// Requests in flight and how requests have ended.
    pub(crate) request_stats: RequestStats,
}

impl Server {
    /// Builds the server's state from `config` and starts its CPU pool.
    ///
    /// Nothing is written: the data directory is created with the first universe. A data
    /// directory that exists must be a directory.
    ///
    /// # Errors
    ///
    /// [`StartServerError`] if the data directory is not a directory or cannot be inspected, if
    /// the CPU pool cannot start, or if start-up is interrupted by the runtime shutting down.
    pub async fn start(config: ServerConfig) -> Result<Self, StartServerError> {
        Self::start_with_handler(config, Arc::new(Handlers)).await
    }

    /// [`Server::start`], with `handler` answering every request.
    pub(crate) async fn start_with_handler(
        config: ServerConfig,
        handler: Arc<dyn Handler>,
    ) -> Result<Self, StartServerError> {
        let data_dir = config.data_dir().to_path_buf();
        tokio::task::spawn_blocking(move || check_data_dir(&data_dir))
            .await
            .map_err(|_| StartServerError::Interrupted)??;
        let pool = CpuPool::new(
            config.workers(),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
        )
        .map_err(StartServerError::StartPool)?;
        tracing::info!(
            data_dir = %config.data_dir().display(),
            workers = config.workers().get(),
            "server started"
        );
        Ok(Self {
            state: Arc::new(AppState {
                pool,
                handler,
                connections: Connections::new(),
                request_stats: RequestStats::new(),
            }),
        })
    }

    /// The server's HTTP and WebSocket routes: `/healthz` and `/ws`.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/healthz", get(healthz))
            .route("/ws", get(ws::upgrade))
            .with_state(Arc::clone(&self.state))
    }

    /// What the server is doing now and has done so far.
    #[must_use]
    pub fn stats(&self) -> ServerStats {
        ServerStats::new(
            self.state.connections.open_count(),
            self.state.request_stats.snapshot(),
            self.state.pool.counters(),
        )
    }

    /// Tears the server down once serving has stopped.
    ///
    /// Every WebSocket connection is closed with code 1001 (going away), and what it has in
    /// flight is cancelled. axum's graceful shutdown does neither, nor does it wait for those
    /// connections, so this does all three. A connection opened meanwhile is refused. Each
    /// connection has [`CLOSE_TIMEOUT`](limits::CLOSE_TIMEOUT) to finish closing, so a client
    /// that has stopped reading cannot hold this up. Then the CPU pool stops: queued jobs are
    /// dropped and the workers finish the jobs in hand.
    ///
    /// # Errors
    ///
    /// [`ShutDownServerError`] if the CPU pool does not stop cleanly.
    pub async fn shutdown(self) -> Result<(), ShutDownServerError> {
        self.state.connections.close_all().await;
        self.state
            .pool
            .shutdown()
            .await
            .map_err(ShutDownServerError::CpuPool)?;
        tracing::info!("server shut down");
        Ok(())
    }

    /// The shared state, for the unit tests that reach into it.
    #[cfg(test)]
    pub(crate) fn state(&self) -> &Arc<AppState> {
        &self.state
    }
}

async fn healthz() -> &'static str {
    "ok"
}

/// Fails if `path` exists and is not a directory. A missing directory is fine: it is created with
/// the first universe.
fn check_data_dir(path: &Path) -> Result<(), StartServerError> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(StartServerError::DataDirNotADirectory {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(StartServerError::InspectDataDir {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// The server could not start.
#[derive(Debug)]
pub enum StartServerError {
    /// The data directory exists and is not a directory.
    DataDirNotADirectory {
        /// The configured data directory.
        path: PathBuf,
    },
    /// The data directory could not be inspected.
    InspectDataDir {
        /// The configured data directory.
        path: PathBuf,
        /// The operating system's error.
        source: io::Error,
    },
    /// The CPU pool could not start.
    StartPool(StartPoolError),
    /// A start-up task was cancelled by the runtime shutting down.
    Interrupted,
}

impl fmt::Display for StartServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataDirNotADirectory { path } => {
                write!(
                    f,
                    "the data directory {} is not a directory",
                    path.display()
                )
            }
            Self::InspectDataDir { path, .. } => {
                write!(f, "failed to inspect the data directory {}", path.display())
            }
            Self::StartPool(_) => f.write_str("failed to start the cpu pool"),
            Self::Interrupted => f.write_str("server start-up was interrupted"),
        }
    }
}

impl Error for StartServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InspectDataDir { source, .. } => Some(source),
            Self::StartPool(source) => Some(source),
            Self::DataDirNotADirectory { .. } | Self::Interrupted => None,
        }
    }
}

/// The server did not shut down cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShutDownServerError {
    /// The CPU pool did not stop cleanly.
    CpuPool(ShutDownPoolError),
}

impl fmt::Display for ShutDownServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CpuPool(_) => f.write_str("failed to stop the cpu pool"),
        }
    }
}

impl Error for ShutDownServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CpuPool(source) => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_creates_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("hyperion-data");
        let server = Server::start(ServerConfig::builder().data_dir(&data_dir).build())
            .await
            .unwrap();
        let _router = server.router();
        server.shutdown().await.unwrap();
        assert!(!data_dir.exists());
    }

    #[tokio::test]
    async fn counters_move_as_requests_are_accepted_run_and_finish() {
        use hyperion_protocol::ErrorCode;

        use crate::requests::request_error;
        use crate::testing::{Harness, Scripted, body, small_response};

        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let stats = || harness.server().stats();
        assert_eq!(stats(), ServerStats::default());
        let mut client = harness.connect().await;
        assert_eq!(stats().connections(), 1);
        client.hello().await;

        client.request(1, body(1)).await;
        let call = calls.next().await;
        let requests = stats().requests();
        assert_eq!((requests.accepted(), requests.in_flight()), (1, 1));
        client.request(1, body(1)).await;
        client.next_message().await;
        assert_eq!(stats().requests().refused(), 1);
        assert!(call.respond(small_response()));
        client.next_message().await;
        let requests = stats().requests();
        assert_eq!((requests.responded(), requests.in_flight()), (1, 0));

        client.request(2, body(2)).await;
        let cancelled = calls.next().await;
        client.cancel(2).await;
        client.next_message().await;
        assert_eq!(stats().requests().cancelled(), 1);
        drop(cancelled);

        client.request(3, body(3)).await;
        assert!(calls.next().await.fail(request_error(
            ErrorCode::UnknownUniverse,
            "no such universe"
        )));
        client.next_message().await;
        let requests = stats().requests();
        assert_eq!(
            (
                requests.accepted(),
                requests.in_flight(),
                requests.refused(),
                requests.responded(),
                requests.failed(),
                requests.cancelled(),
                requests.abandoned()
            ),
            (3, 0, 1, 1, 1, 1, 0)
        );

        client.close().await;
        harness.connections_until(0).await;
        assert_eq!(stats().connections(), 0);
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_data_directory_that_is_a_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("data");
        std::fs::write(&file, "").unwrap();
        let error = Server::start(ServerConfig::builder().data_dir(&file).build())
            .await
            .unwrap_err();
        assert!(
            matches!(&error, StartServerError::DataDirNotADirectory { path } if *path == file),
            "{error:?}"
        );
    }
}
