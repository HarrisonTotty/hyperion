use anyhow::Context;
use hyperion_server::{Server, ServerConfig};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = ServerConfig::from_env().context("invalid configuration")?;
    // Bind first, so that nothing needs tearing down when the address is taken.
    let listener = TcpListener::bind(config.addr())
        .await
        .with_context(|| format!("failed to bind {}", config.addr()))?;
    let addr = listener.local_addr()?;
    let server = Server::start(config)
        .await
        .context("failed to start the server")?;
    tracing::info!(%addr, "listening");

    let serve_result = axum::serve(listener, server.router())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error");
    server
        .shutdown()
        .await
        .context("failed to shut down cleanly")?;
    serve_result
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for ctrl-c");
    }
    tracing::info!("shutting down");
}
