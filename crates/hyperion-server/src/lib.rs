//! HYPERION game server: hosts the simulation and serves bridge clients.
//!
//! The server owns everything the sim may not: universes and their saves ([`universe`]), the CPU
//! pool that runs generation off the async runtime and the caches of galaxies, maps, cells and
//! systems ([`compute`]), the bodies' coarse surface fields ([`surface`]), the byte-bounded cache
//! they are built on ([`cache`]), and the WebSocket clients speak `hyperion-protocol` over. What
//! the crew has detected is in [`knowledge`], and what a sensor is told of transients as their
//! light arrives in [`alerts`]. Every limit it enforces is in [`limits`].
//!
//! [`Server::start`] builds the shared state from a [`ServerConfig`], [`Server::router`] serves
//! it, [`Server::stats`] reports on it, and [`Server::shutdown`] is the explicit teardown once
//! serving has stopped. What asks the process to stop, on each platform, is in [`stop`].

pub mod alerts;
pub(crate) mod bulk;
pub mod cache;
pub mod compute;
pub mod config;
mod connections;
mod convert;
pub mod knowledge;
pub mod limits;
mod outbound;
mod requests;
pub mod scene;
mod stats;
pub mod stop;
mod subscriptions;
pub mod surface;
#[cfg(test)]
mod testing;
pub mod universe;
mod ws;

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, io};

use axum::{Router, routing::get};

use crate::compute::{
    BulkGate, CpuPool, DensityMapService, ExtinctionMapService, GalaxyCache, GalaxyFeaturesService,
    GlobalListService, SharedBodyCache, SharedBriefCache, SharedCellCache, SharedClassCellCache,
    SharedClusterModelCache, SharedFeatureCellCache, SharedSightlineCache, SharedSkyCellCache,
    SharedSprayForces, SharedSystemCache, ShutDownPoolError, SkyCaps, SkyTablesService,
    StartPoolError, TubeTableCache,
};
use crate::connections::Connections;
use crate::limits::{
    BULK_QUEUE_CAPACITY, GALAXY_FEATURES_CACHE_BYTES, GLOBAL_LIST_CACHE_BYTES,
    INTERACTIVE_QUEUE_CAPACITY, SPRAY_FORCE_CACHE_BYTES, TUBE_TABLE_CACHE_BYTES,
};
use crate::requests::{Handler, Handlers};
use crate::scene::SceneService;
use crate::stats::{OutboundStats, RequestStats};
use crate::surface::SurfaceService;
use crate::universe::{LoadRegistryError, UniverseRegistry, UniverseStore};
use crate::ws::ConnectionLimits;

pub use config::{
    FEATURE_MEMBERS_FROM, FeatureMembers, GlobalListWarmUp, ServerArgs, ServerConfig,
    ServerConfigBuilder, SkyService,
};
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
    /// Shared with [`AppState::maps`] and [`AppState::extinction_maps`], which take the galaxy of
    /// each map they compute from it.
    pub(crate) galaxies: Arc<GalaxyCache>,
    /// The density maps computed so far, in the configured byte budget.
    pub(crate) maps: DensityMapService,
    /// The extinction maps computed so far, in their own configured byte budget (plan 07,
    /// P07.T10.a).
    pub(crate) extinction_maps: ExtinctionMapService,
    /// The lines of sight `extinction` has marched, in the configured byte budget (plan 07,
    /// P07.T10.c).
    pub(crate) sightlines: SharedSightlineCache,
    /// The generated cells a range query reads, in the configured byte budget.
    ///
    /// A query takes its own [`CellCacheHandle`](compute::CellCacheHandle) from this on the pool
    /// job that runs it, since the handle borrows the cache and cannot cross an `.await`.
    pub(crate) cells: Arc<SharedCellCache>,
    /// The systems' stars generated so far, in the configured byte budget: what a
    /// `system_summary` reads (plan 06, P06.T34).
    pub(crate) systems: SharedSystemCache,
    /// The range briefs' models built so far, in the configured byte budget: what a
    /// `systems_in_range` with `include_stellar` reads (plan 06, P06.T34).
    pub(crate) briefs: SharedBriefCache,
    /// The planetary systems generated so far, each with its context, in the configured byte
    /// budget: what `system_bodies` and `body_detail` read (plan 14, P14.T36).
    pub(crate) bodies: SharedBodyCache,
    /// Plan 09's feature cells generated so far, in the configured byte budget: what the feature
    /// requests, the range query's feature members and the sky's feature gas look features up in
    /// (P09.T40.a).
    ///
    /// Shared with the [`SkyTablesService`], whose tables lend it to the sky's jobs.
    pub(crate) feature_cells: Arc<SharedFeatureCellCache>,
    /// Plan 09's feature interiors built so far, in the configured byte budget: what the range
    /// query's feature members and a feature's detail read (P09.T40.a).
    ///
    /// Shared with [`AppState::systems`] and [`AppState::briefs`], which resolve a feature's
    /// members through it.
    pub(crate) cluster_models: Arc<SharedClusterModelCache>,
    /// Plan 09's catalogue class cells made so far, in the configured byte budget: what the
    /// range query's class hosts are read from, from generator version 27 (P09.T40).
    pub(crate) class_cells: Arc<SharedClassCellCache>,
    /// The galaxy map's globulars of every galaxy asked for, found once each in a bulk job and kept
    /// (P09.T40.b).
    pub(crate) galaxy_features: GalaxyFeaturesService,
    /// Plan 10's global list of every galaxy asked for, built once each in a bulk job and kept
    /// (P10.T10.a), and its streams' tube tables, kept in their own byte budget (P10.T10.b).
    pub(crate) global_lists: GlobalListService,
    /// Whether an open starts its galaxy's global list and warms its tube tables (P10.T10.b).
    pub(crate) global_list_warm_up: GlobalListWarmUp,
    /// Whether the range query serves plan 09's feature members, each row its origin, and the sky
    /// the features' gas: P09.T40.a's serve gate, held until
    /// [`FEATURE_MEMBERS_FROM`](config::FEATURE_MEMBERS_FROM).
    pub(crate) feature_members: FeatureMembers,
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
    /// Each open universe's scene clock and ship stand-in, which `scene_ship` sets (rendering
    /// plan R03, Design note 2).
    pub(crate) scene: SceneService,
    /// Whether `sky` is answered: the sky's switch (rendering plan R06, R06.T11.c), on by default.
    pub(crate) sky_service: SkyService,
    /// How far a sky's census looks, and which luminosity tables it reads (rendering plan R06,
    /// R06.T11.a).
    pub(crate) sky_caps: SkyCaps,
    /// Each galaxy's sky tables, built once on the pool and kept in the configured byte budget
    /// (rendering plan R06, R06.T11.c).
    pub(crate) sky_tables: SkyTablesService,
    /// The census cells' entries built so far, in blocks, in the configured byte budget: what a
    /// sky's census jobs read and share (rendering plan R06, R06.T11.b and T8.h; Design note 12).
    pub(crate) sky_cells: Arc<SharedSkyCellCache>,
    /// Each body's coarse surface field, computed once on the pool and kept in the configured byte
    /// budget (rendering plan R09, R09.T17; Design note 19).
    pub(crate) surface: SurfaceService,
    /// Each universe's contacts, opened by its first alerts subscription and kept (plan 12,
    /// P12.T9.a).
    pub(crate) contacts: alerts::UniverseContacts,
    /// The alert scans walked so far, shared by the alerts subscriptions, in the configured byte
    /// budget (plan 12, P12.T8.b).
    pub(crate) alert_scans: alerts::AlertScans,
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
        // Starting the pool waits for every worker to probe its floating-point mode, so it runs
        // off the runtime.
        let (workers, gate) = (config.workers(), config.bulk_gate().cloned());
        let pool = Arc::new(
            tokio::task::spawn_blocking(move || start_pool(workers, gate))
                .await
                .map_err(|_| StartServerError::Interrupted)?
                .map_err(StartServerError::StartPool)?,
        );
        let galaxies = Arc::new(GalaxyCache::new(Arc::clone(&pool)));
        let maps = DensityMapService::new(
            Arc::clone(&pool),
            Arc::clone(&galaxies),
            config.map_cache_bytes(),
        );
        let extinction_maps = ExtinctionMapService::new(
            Arc::clone(&pool),
            Arc::clone(&galaxies),
            config.extinction_map_cache_bytes(),
        );
        let sightlines = SharedSightlineCache::new(config.sightline_cache_bytes());
        let cells = Arc::new(SharedCellCache::new(config.cell_cache_bytes()));
        let (feature_cells, cluster_models, class_cells, map_lists, global_lists) =
            feature_caches(&config, Arc::clone(&pool), Arc::clone(&galaxies));
        let (systems, briefs) = system_caches(&config, &cluster_models);
        let bodies = SharedBodyCache::new(Arc::clone(&pool), config.body_cache_bytes());
        let sky_cells = Arc::new(SharedSkyCellCache::new(config.sky_cache_bytes()));
        let sky_tables = sky_tables_service(&config, Arc::clone(&pool), &feature_cells);
        let surface = SurfaceService::new(
            Arc::clone(&pool),
            Arc::clone(config.surface_inputs()),
            config.surface_cache_bytes(),
            config.surface_passes(),
        );
        log_start(&config);
        Ok(Self {
            state: Arc::new(AppState {
                registry,
                pool,
                galaxies,
                maps,
                extinction_maps,
                sightlines,
                cells,
                systems,
                briefs,
                bodies,
                feature_cells,
                cluster_models,
                class_cells,
                galaxy_features: map_lists,
                global_lists,
                global_list_warm_up: config.global_list_warm_up(),
                feature_members: config.feature_members(),
                handler,
                connections: Connections::new(),
                request_stats: RequestStats::new(),
                outbound_stats: OutboundStats::new(),
                connection_limits,
                scene: SceneService::new(
                    Arc::clone(config.scene_knowledge()),
                    Arc::clone(config.craft_source()),
                ),
                sky_service: config.sky_service(),
                sky_caps: config.sky_caps(),
                sky_tables,
                sky_cells,
                surface,
                contacts: alerts::UniverseContacts::new(),
                alert_scans: alerts::AlertScans::new(config.alert_cache_bytes()),
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
    /// Every WebSocket connection is closed with code 1001 (going away), and what it has in flight
    /// is cancelled. axum's graceful shutdown does neither, nor does it wait for those connections,
    /// so this does all three. A connection opened meanwhile is refused. Each connection has
    /// [`CLOSE_TIMEOUT`](limits::CLOSE_TIMEOUT) to finish closing, so a client that has stopped
    /// reading cannot hold this up. The sky tables' builds and the global lists' warm-ups that
    /// opens started, and the coarse passes that prefetches started, are given up. Then the CPU
    /// pool stops: queued jobs are dropped and the workers finish the jobs in hand.
    ///
    /// # Errors
    ///
    /// [`ShutDownServerError`] if the CPU pool does not stop cleanly.
    pub async fn shutdown(self) -> Result<(), ShutDownServerError> {
        self.state.connections.close_all().await;
        // The sky tables' builds started at opens wait on no connection.
        self.state.sky_tables.stop_prefetching();
        // Nor do the coarse passes started by prefetches.
        self.state.surface.stop_prefetching();
        // Nor do the global lists' warm-ups started at opens.
        self.state.global_lists.stop_warming();
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

/// Prepares a socket the server has accepted: sets `TCP_NOTSENT_LOWAT` to
/// [`TCP_NOTSENT_LOWAT_BYTES`](limits::TCP_NOTSENT_LOWAT_BYTES) on Linux and Android, so that the
/// kernel holds little unsent behind a scene push (rendering plan R03, Design note 11).
///
/// Every place that serves the router calls it, through axum's `ListenerExt::tap_io`, so that every
/// connection is prepared alike. A failure is logged at `warn` and the connection goes on.
///
/// Elsewhere it does nothing, a limit Design note 11 states (R03.T17; XNU and Microsoft Learn read
/// 2026-10-05, the crates rechecked at their locked versions 2026-10-20):
///
/// - **macOS.** XNU has the option (`TCP_NOTSENT_LOWAT`, 0x201, in `bsd/netinet/tcp.h`), but it
///   gates only the writability events: `tcp_notsent_lowat_check()` (`tcp_subr.c`) is read by
///   `sowriteable()` (`uipc_socket2.c`) and `filt_sowrite_common()` (`uipc_socket.c`) alone, while
///   `sosendcheck()` (`uipc_socket.c`) lets `send` fill the buffer to its high-water mark
///   (apple-oss-distributions/xnu, `main`). tokio writes until `WouldBlock` before it waits for
///   readiness (tokio 1.53.3, `Registration::poll_io`), so the mark would not bound the queue,
///   only the rhythm of the refills. Setting it would need `unsafe` besides: socket2 0.6.5 gates
///   it to Linux and Android, libc 0.2.191 has no Apple binding, and rustix 1.1.5 and nix 0.31.3
///   have none. XNU's send-buffer autosizing starts at `tcp_sendspace`, 128 KiB (`tcp_usrreq.c`),
///   and grows only while, among its conditions, `min(snd_wnd, snd_cwnd)` covers every unsent
///   byte (`tcp_output()`), so the unsent bytes stay near one send window.
/// - **Windows** has no such option. Its dynamic send buffering, on unless an application sets
///   `SO_SNDBUF`, keeps about one ideal send backlog outstanding, "the ideal value for the amount
///   of data outstanding (sent but not acknowledged yet)", at most 16 MB (Microsoft Learn,
///   `SIO_IDEAL_SEND_BACKLOG_QUERY`), so little of it is unsent.
///
/// So the kernel's share behind a push is at most 64 KiB unsent on Linux (13.1 ms at 40 Mbit/s),
/// plus the bytes in flight; on macOS at most max(128 KiB, about one send window) unsent, at
/// least 26.2 ms at 40 Mbit/s and growing only with the congestion window; on Windows about one
/// ideal send backlog outstanding in total. Each comes on top of the one-chunk bound,
/// [`BULK_QUEUED_BYTES`](limits::BULK_QUEUED_BYTES), which holds on every platform, and over
/// loopback all three are negligible. `SO_SNDBUF` is set on no platform: on macOS it clears
/// `SB_AUTOSIZE` (`uipc_socket.c`) and pins the buffer, and on Windows it turns dynamic send
/// buffering off.
///
/// # Examples
///
/// ```no_run
/// use axum::serve::ListenerExt;
/// use tokio::net::TcpListener;
///
/// # async fn serve(router: axum::Router) -> std::io::Result<()> {
/// let listener = TcpListener::bind("127.0.0.1:0").await?;
/// axum::serve(listener.tap_io(|tcp| hyperion_server::tap_socket(tcp)), router).await
/// # }
/// ```
pub fn tap_socket(tcp: &tokio::net::TcpStream) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if let Err(error) =
        socket2::SockRef::from(tcp).set_tcp_notsent_lowat(limits::TCP_NOTSENT_LOWAT_BYTES)
    {
        tracing::warn!(%error, "could not set TCP_NOTSENT_LOWAT on an accepted socket");
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let _ = tcp;
}

/// The server's CPU pool of `workers`, its bulk jobs held at `gate` if a test gave one
/// ([`ServerConfigBuilder::bulk_gate`]).
///
/// # Errors
///
/// As [`CpuPool::new`].
fn start_pool(workers: NonZeroUsize, gate: Option<BulkGate>) -> Result<CpuPool, StartPoolError> {
    match gate {
        Some(gate) => CpuPool::with_bulk_gate(
            workers,
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
            gate,
        ),
        None => CpuPool::new(workers, INTERACTIVE_QUEUE_CAPACITY, BULK_QUEUE_CAPACITY),
    }
}

/// The caches of plan 09's feature cells and interiors, in the budgets `config` gives (P09.T40.a),
/// its catalogue class cells (P09.T40), the galaxy map's
/// feature lists (P09.T40.b), and plan 10's global lists (P10.T10.a) with their streams' tube tables
/// (P10.T10.b), each list and table found on `pool` from the galaxies of `galaxies`.
#[must_use]
fn feature_caches(
    config: &ServerConfig,
    pool: Arc<CpuPool>,
    galaxies: Arc<GalaxyCache>,
) -> (
    Arc<SharedFeatureCellCache>,
    Arc<SharedClusterModelCache>,
    Arc<SharedClassCellCache>,
    GalaxyFeaturesService,
    GlobalListService,
) {
    (
        Arc::new(SharedFeatureCellCache::new(config.feature_cache_bytes())),
        Arc::new(SharedClusterModelCache::new(
            config.cluster_model_cache_bytes(),
        )),
        Arc::new(SharedClassCellCache::new(config.class_cell_cache_bytes())),
        GalaxyFeaturesService::new(
            Arc::clone(&pool),
            Arc::clone(&galaxies),
            GALAXY_FEATURES_CACHE_BYTES,
        ),
        GlobalListService::new(
            pool,
            galaxies,
            (
                Arc::new(SharedSprayForces::new(SPRAY_FORCE_CACHE_BYTES)),
                Arc::new(TubeTableCache::new(TUBE_TABLE_CACHE_BYTES)),
            ),
            GLOBAL_LIST_CACHE_BYTES,
        ),
    )
}

/// Logs the configuration a server starts with: where it keeps its data, its workers and every
/// cache's budget.
fn log_start(config: &ServerConfig) {
    tracing::info!(
        data_dir = %config.data_dir().display(),
        workers = config.workers().get(),
        cell_cache_mib = config.cell_cache_bytes() / (1 << 20),
        map_cache_mib = config.map_cache_bytes() / (1 << 20),
        system_cache_mib = config.system_cache_bytes() / (1 << 20),
        body_cache_mib = config.body_cache_bytes() / (1 << 20),
        brief_cache_mib = config.brief_cache_bytes() / (1 << 20),
        sky_cache_mib = config.sky_cache_bytes() / (1 << 20),
        sky_tables_mib = config.sky_tables_bytes() / (1 << 20),
        surface_cache_mib = config.surface_cache_bytes() / (1 << 20),
        surface_passes = config.surface_passes().get(),
        sky_service = ?config.sky_service(),
        extinction_map_cache_mib = config.extinction_map_cache_bytes() / (1 << 20),
        sightline_cache_mib = config.sightline_cache_bytes() / (1 << 20),
        alert_cache_mib = config.alert_cache_bytes() / (1 << 20),
        feature_cache_mib = config.feature_cache_bytes() / (1 << 20),
        cluster_model_cache_mib = config.cluster_model_cache_bytes() / (1 << 20),
        class_cell_cache_mib = config.class_cell_cache_bytes() / (1 << 20),
        feature_members = ?config.feature_members(),
        "server started"
    );
}

/// The caches of the systems' stars and of the range briefs' models, in the budgets `config`
/// gives.
///
/// Each resolves a feature member through the feature interiors `cluster_models`, so that a
/// feature's members share one build of its interior.
#[must_use]
fn system_caches(
    config: &ServerConfig,
    cluster_models: &Arc<SharedClusterModelCache>,
) -> (SharedSystemCache, SharedBriefCache) {
    (
        SharedSystemCache::new(config.system_cache_bytes())
            .with_interiors(Arc::clone(cluster_models)),
        SharedBriefCache::new(config.brief_cache_bytes())
            .with_interiors(Arc::clone(cluster_models)),
    )
}

/// The sky tables' service `config` asks for, on `pool`: one whose tables lend their jobs the
/// features' gas from `feature_cells` once the feature members are served (P09.T40.a's gate), so
/// that no sky moves before P09.T2.c.
#[must_use]
fn sky_tables_service(
    config: &ServerConfig,
    pool: Arc<CpuPool>,
    feature_cells: &Arc<SharedFeatureCellCache>,
) -> SkyTablesService {
    let service =
        SkyTablesService::new(pool, config.sky_caps().tables(), config.sky_tables_bytes());
    match config.feature_members() {
        FeatureMembers::Held => service,
        FeatureMembers::Served => service.with_features(Arc::clone(feature_cells)),
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
        let server = Server::start(
            ServerConfig::builder()
                .data_dir(&data_dir)
                .workers(std::num::NonZeroUsize::MIN)
                .build(),
        )
        .await
        .unwrap();
        let _router = server.router();
        server.shutdown().await.unwrap();
        assert!(!data_dir.exists());
    }

    /// Asserts that every cache of `fresh` holds nothing and has done nothing yet, its budget the
    /// configuration's.
    fn every_cache_is_empty(fresh: &ServerStats) {
        let defaults = ServerConfig::builder().build();
        for (cache, budget) in [
            (fresh.maps(), defaults.map_cache_bytes()),
            (fresh.cells(), defaults.cell_cache_bytes()),
            (fresh.systems(), defaults.system_cache_bytes()),
            (fresh.briefs(), defaults.brief_cache_bytes()),
            (fresh.sky_cells().cache(), defaults.sky_cache_bytes()),
            (fresh.sky_tables().cache(), defaults.sky_tables_bytes()),
            (fresh.surface().cache(), defaults.surface_cache_bytes()),
            (
                fresh.extinction_maps(),
                defaults.extinction_map_cache_bytes(),
            ),
            (fresh.sightlines(), defaults.sightline_cache_bytes()),
            (fresh.feature_cells(), defaults.feature_cache_bytes()),
            (fresh.cluster_models(), defaults.cluster_model_cache_bytes()),
            (fresh.class_cells(), defaults.class_cell_cache_bytes()),
            (fresh.galaxy_features(), GALAXY_FEATURES_CACHE_BYTES),
            (fresh.global_lists(), GLOBAL_LIST_CACHE_BYTES),
            (fresh.tube_tables(), TUBE_TABLE_CACHE_BYTES),
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
        every_cache_is_empty(&fresh);
        assert_eq!(fresh.sky_cells().rebuilt(), 0);
        assert_eq!(fresh.sky_tables().builds(), 0);
        assert_eq!(fresh.surface().computed(), 0);
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

    /// The gate decides whether the sky's tables lend the feature cells: held, the default at the
    /// version in force, a sky marches the smooth field alone; served, its jobs take the features'
    /// gas (P09.T40.a).
    #[tokio::test]
    async fn the_sky_takes_the_features_gas_only_once_the_gate_serves_them() {
        use hyperion_sim::galaxy::gas::modifiers::GasModifierSource;
        use hyperion_sim::time::UniverseTime;
        use hyperion_sim::units::LightYears;
        use hyperion_sim::{GENERATOR_VERSION, Seed};

        use crate::compute::GalaxyKey;
        use crate::compute::sky::tests::{an_ageing_nursery, feature_gas_at};

        let pool = Arc::new(
            CpuPool::new(
                std::num::NonZeroUsize::MIN,
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .unwrap(),
        );
        let galaxy = Arc::new(hyperion_sim::galaxy::Galaxy::new(Seed::new(0x4d2)));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let from = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 68.0])
            .unwrap();
        let to = an_ageing_nursery(&galaxy, &from);
        let t = UniverseTime::from_julian_years(1_000).unwrap();
        for members in [FeatureMembers::Held, FeatureMembers::Served] {
            let config = ServerConfig::builder()
                .sky_caps(SkyCaps::forced(LightYears::new(10.0)).unwrap())
                .feature_members(members)
                .build();
            let cells = Arc::new(SharedFeatureCellCache::new(config.feature_cache_bytes()));
            let service = sky_tables_service(&config, Arc::clone(&pool), &cells);
            let tables = service.get(key, &galaxy).await.unwrap();
            let mut found = Vec::new();
            tables
                .gas(&galaxy, t)
                .modifiers_near_segment(&from, &to, &mut found);
            let expected = match members {
                FeatureMembers::Held => Vec::new(),
                FeatureMembers::Served => feature_gas_at(&galaxy, 1_000.0, &from, &to),
            };
            assert_eq!(found, expected, "{members:?}");
        }
        pool.shutdown().await.unwrap();
    }
}
