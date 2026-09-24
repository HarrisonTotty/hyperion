//! HYPERION game server: hosts the simulation and serves bridge clients.
//!
//! The server owns everything the sim may not: universes and their saves ([`universe`]), the CPU
//! pool that runs generation off the async runtime and the caches of galaxies, maps, cells and
//! systems ([`compute`]), the byte-bounded cache they are built on ([`cache`]), and the WebSocket
//! clients speak `hyperion-protocol` over. Every limit it enforces is in [`limits`].
//!
//! [`Server::start`] builds the shared state from a [`ServerConfig`], [`Server::router`] serves
//! it, [`Server::stats`] reports on it, and [`Server::shutdown`] is the explicit teardown once
//! serving has stopped.

pub mod cache;
pub mod compute;
pub mod config;
mod connections;
mod convert;
pub mod limits;
mod outbound;
mod requests;
mod stats;
#[cfg(test)]
mod testing;
pub mod universe;
mod ws;

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, io};

use axum::{Router, routing::get};

use crate::compute::{
    CpuPool, DensityMapService, GalaxyCache, SharedBodyCache, SharedCellCache, SharedSystemCache,
    ShutDownPoolError, StartPoolError,
};
use crate::connections::Connections;
use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use crate::requests::{Handler, Handlers};
use crate::stats::{OutboundStats, RequestStats};
use crate::universe::{LoadRegistryError, UniverseRegistry, UniverseStore};
use crate::ws::ConnectionLimits;

pub use config::{ServerArgs, ServerConfig, ServerConfigBuilder};
pub use stats::{OutboundCounters, RequestCounters, ServerStats};

/// Address the server listens on when neither `--address` and `--port` nor their variables are
/// given.
pub const DEFAULT_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7878);

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
#[derive(Debug)]
pub(crate) struct AppState {
    /// Every universe the server holds, loaded from the data directory at start.
    pub(crate) registry: UniverseRegistry,
    /// Where generation, and the serialisation of large responses, runs: never on the runtime.
    ///
    /// Shared, because the caches submit their own jobs: a computation they hand to a
    /// [`SingleFlight`](compute::SingleFlight) outlives the request that started it.
    pub(crate) pool: Arc<CpuPool>,
    /// The galaxies built from the open universes' seeds, at most
    /// [`GALAXY_CACHE_ENTRIES`](limits::GALAXY_CACHE_ENTRIES) of them.
    ///
    /// Shared with [`AppState::maps`], which takes the galaxy of each map it computes from it.
    pub(crate) galaxies: Arc<GalaxyCache>,
    /// The density maps computed so far, in the configured byte budget.
    pub(crate) maps: DensityMapService,
    /// The generated cells a range query reads, in the configured byte budget.
    ///
    /// A query takes its own [`CellCacheHandle`](compute::CellCacheHandle) from this on the pool
    /// job that runs it, since the handle borrows the cache and cannot cross an `.await`.
    pub(crate) cells: SharedCellCache,
    /// The systems' stars generated so far, in the configured byte budget: what a
    /// `system_summary` reads (plan 06, P06.T34).
    pub(crate) systems: SharedSystemCache,
    /// The planetary systems generated so far, each with its context, in the configured byte
    /// budget: what `system_bodies` and `body_detail` read (plan 14, P14.T36).
    pub(crate) bodies: SharedBodyCache,
    /// What answers each request: [`Handlers`], or a test's double.
    pub(crate) handler: Arc<dyn Handler>,
    /// The open WebSocket connections, which shutdown closes and waits for.
    pub(crate) connections: Connections,
    /// Requests in flight and how requests have ended.
    pub(crate) request_stats: RequestStats,
    /// What the connections' outbound queues hold, and the connections closed for not reading.
    pub(crate) outbound_stats: OutboundStats,
    /// The limits each connection enforces on writing to its client.
    pub(crate) connection_limits: ConnectionLimits,
}

impl Server {
    /// Builds the server's state from `config`: loads the saved universes and starts the CPU pool.
    ///
    /// Nothing is written: the data directory is created with the first universe. A data
    /// directory that exists must be a directory, and its saves are scanned on a blocking task.
    ///
    /// # Errors
    ///
    /// [`StartServerError`] if the data directory is not a directory or cannot be inspected, if
    /// its saves cannot be listed, if the CPU pool cannot start, or if start-up is interrupted by
    /// the runtime shutting down.
    pub async fn start(config: ServerConfig) -> Result<Self, StartServerError> {
        Self::start_with_handler(config, Arc::new(Handlers), ConnectionLimits::default()).await
    }

    /// [`Server::start`], with `handler` answering every request and each connection held to
    /// `connection_limits`.
    pub(crate) async fn start_with_handler(
        config: ServerConfig,
        handler: Arc<dyn Handler>,
        connection_limits: ConnectionLimits,
    ) -> Result<Self, StartServerError> {
        let data_dir = config.data_dir().to_path_buf();
        tokio::task::spawn_blocking(move || check_data_dir(&data_dir))
            .await
            .map_err(|_| StartServerError::Interrupted)??;
        let registry = UniverseRegistry::load(
            UniverseStore::new(config.data_dir()),
            Arc::clone(config.entropy()),
        )
        .await
        .map_err(StartServerError::LoadRegistry)?;
        let pool = Arc::new(
            CpuPool::new(
                config.workers(),
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .map_err(StartServerError::StartPool)?,
        );
        let galaxies = Arc::new(GalaxyCache::new(Arc::clone(&pool)));
        let maps = DensityMapService::new(
            Arc::clone(&pool),
            Arc::clone(&galaxies),
            config.map_cache_bytes(),
        );
        let cells = SharedCellCache::new(config.cell_cache_bytes());
        let systems = SharedSystemCache::new(config.system_cache_bytes());
        let bodies = SharedBodyCache::new(Arc::clone(&pool), config.body_cache_bytes());
        tracing::info!(
            data_dir = %config.data_dir().display(),
            workers = config.workers().get(),
            cell_cache_mib = config.cell_cache_bytes() / (1 << 20),
            map_cache_mib = config.map_cache_bytes() / (1 << 20),
            system_cache_mib = config.system_cache_bytes() / (1 << 20),
            body_cache_mib = config.body_cache_bytes() / (1 << 20),
            "server started"
        );
        Ok(Self {
            state: Arc::new(AppState {
                registry,
                pool,
                galaxies,
                maps,
                cells,
                systems,
                bodies,
                handler,
                connections: Connections::new(),
                request_stats: RequestStats::new(),
                outbound_stats: OutboundStats::new(),
                connection_limits,
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
        ServerStats::of(&self.state)
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
    /// The saved universes could not be loaded.
    LoadRegistry(LoadRegistryError),
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
            Self::LoadRegistry(_) => f.write_str("failed to load the saved universes"),
            Self::StartPool(_) => f.write_str("failed to start the cpu pool"),
            Self::Interrupted => f.write_str("server start-up was interrupted"),
        }
    }
}

impl Error for StartServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InspectDataDir { source, .. } => Some(source),
            Self::LoadRegistry(source) => Some(source),
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
        // Every counter starts at zero. The map, cell and system caches' budgets are configuration
        // rather than counters, so they are compared with the configuration's.
        let fresh = stats();
        assert_eq!(
            (
                fresh.connections(),
                fresh.requests(),
                fresh.outbound(),
                fresh.pool(),
                fresh.galaxies()
            ),
            (
                0,
                RequestCounters::default(),
                OutboundCounters::default(),
                crate::compute::PoolCounters::default(),
                crate::compute::GalaxyCounters::default(),
            )
        );
        let defaults = ServerConfig::builder().build();
        for (cache, budget) in [
            (fresh.maps(), defaults.map_cache_bytes()),
            (fresh.cells(), defaults.cell_cache_bytes()),
            (fresh.systems(), defaults.system_cache_bytes()),
        ] {
            assert_eq!(
                (
                    cache.entries(),
                    cache.bytes(),
                    cache.hits(),
                    cache.misses(),
                    cache.evictions(),
                    cache.refused(),
                    cache.budget()
                ),
                (0, 0, 0, 0, 0, 0, budget)
            );
        }
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

    #[tokio::test]
    async fn saves_that_cannot_be_listed_stop_the_start() {
        let dir = tempfile::tempdir().unwrap();
        // The directory of universes is a file, so the scan cannot list it.
        std::fs::write(dir.path().join("universes"), "").unwrap();
        let error = Server::start(ServerConfig::builder().data_dir(dir.path()).build())
            .await
            .unwrap_err();
        assert!(
            matches!(
                &error,
                StartServerError::LoadRegistry(crate::universe::LoadRegistryError::Scan(_))
            ),
            "{error:?}"
        );
    }
}
