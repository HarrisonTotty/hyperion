//! HYPERION game server: hosts the simulation and serves bridge clients.
//!
//! The server owns everything the sim may not: universes and their saves ([`universe`]), the CPU
//! pool that runs generation off the async runtime ([`compute`]), the caches of generated data
//! ([`cache`]), and the WebSocket clients speak `hyperion-protocol` over. Every limit it enforces
//! is in [`limits`].
//!
//! [`Server::start`] builds the shared state from a [`ServerConfig`], [`Server::router`] serves
//! it, and [`Server::shutdown`] is the explicit teardown once serving has stopped.

pub mod cache;
pub mod compute;
pub mod config;
pub mod limits;
pub mod universe;
mod ws;

use std::convert::Infallible;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, io};

use axum::{Router, routing::get};

pub use config::{ParseConfigError, ServerConfig, ServerConfigBuilder};

/// Address the server listens on when `HYPERION_ADDR` is not set.
pub const DEFAULT_ADDR: &str = "127.0.0.1:7878";

/// A running server's shared state and the router that serves it.
///
/// Serving is the caller's: hand [`Server::router`] to `axum::serve` with a listener, and once
/// serving has stopped, call [`Server::shutdown`].
#[derive(Debug)]
pub struct Server {
    state: Arc<AppState>,
}

/// The state every connection and request shares.
///
/// Empty until the pieces that need it arrive: the CPU pool (P04.T13) and the universe registry
/// and caches (P04.T14).
#[derive(Debug)]
pub(crate) struct AppState {}

impl Server {
    /// Builds the server's state from `config`.
    ///
    /// Nothing is written: the data directory is created with the first universe. A data
    /// directory that exists must be a directory.
    ///
    /// # Errors
    ///
    /// [`StartServerError`] if the data directory is not a directory or cannot be inspected.
    pub async fn start(config: ServerConfig) -> Result<Self, StartServerError> {
        let data_dir = config.data_dir().to_path_buf();
        tokio::task::spawn_blocking(move || check_data_dir(&data_dir))
            .await
            .map_err(|_| StartServerError::Interrupted)??;
        tracing::info!(
            data_dir = %config.data_dir().display(),
            workers = config.workers().get(),
            "server started"
        );
        Ok(Self {
            state: Arc::new(AppState {}),
        })
    }

    /// The server's HTTP and WebSocket routes: `/healthz` and `/ws`.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/healthz", get(healthz))
            .route("/ws", get(ws::upgrade))
            .with_state(Arc::clone(&self.state))
    }

    /// Tears the server down once serving has stopped.
    ///
    /// # Errors
    ///
    /// [`ShutDownServerError`] if part of the server fails to stop cleanly. Nothing it owns can
    /// fail to stop yet; the CPU pool (P04.T13) is the first thing that can.
    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "the CPU pool's asynchronous shutdown is awaited here from P04.T13"
    )]
    pub async fn shutdown(self) -> Result<(), ShutDownServerError> {
        drop(self.state);
        tracing::info!("server shut down");
        Ok(())
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
            Self::Interrupted => f.write_str("server start-up was interrupted"),
        }
    }
}

impl Error for StartServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InspectDataDir { source, .. } => Some(source),
            Self::DataDirNotADirectory { .. } | Self::Interrupted => None,
        }
    }
}

/// The server did not shut down cleanly.
///
/// Nothing the server owns can fail to stop yet, so no value of this type exists; the CPU pool
/// (P04.T13) adds the first way to fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShutDownServerError(Infallible);

impl fmt::Display for ShutDownServerError {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {}
    }
}

impl Error for ShutDownServerError {}

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
