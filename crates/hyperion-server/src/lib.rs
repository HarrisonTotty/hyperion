//! HYPERION game server: hosts the simulation and serves bridge clients.

mod ws;

use axum::{Router, routing::get};

/// Address the server listens on when `HYPERION_ADDR` is not set.
pub const DEFAULT_ADDR: &str = "127.0.0.1:7878";

/// Builds the server's HTTP/WebSocket router.
pub fn app() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/ws", get(ws::upgrade))
}

async fn healthz() -> &'static str {
    "ok"
}
