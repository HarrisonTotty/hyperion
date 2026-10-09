use std::net::SocketAddr;

use anyhow::Context;
use axum::serve::ListenerExt;
use clap::Parser;
use hyperion_server::stop::{self, Shutdown, StopRequests};
use hyperion_server::{Server, ServerArgs, ServerConfig};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parsing exits with the help, the version or a usage error before anything starts.
    let config = ServerConfig::from(ServerArgs::parse());
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Bind first, so that nothing needs tearing down when the address is taken.
    let listener = TcpListener::bind(config.addr())
        .await
        .with_context(|| format!("failed to bind {}", config.addr()))?;
    let addr = listener.local_addr()?;
    let stdin = config.stdin_stop();
    let server = Server::start(config)
        .await
        .context("failed to start the server")?;
    // Only now, and before the `listening` line: a start writes nothing, so until here a stop
    // request may end the process at once, by the platform's default, even if the start hangs.
    // Standard input is not read until here either, so an end that came earlier stops it now.
    let mut requests = StopRequests::listen(stdin);
    tracing::info!(%addr, "listening");

    // Graceful shutdown stops accepting and drains HTTP requests; `Server::shutdown` then closes
    // the WebSockets, which axum leaves running. axum's signal must be `'static`, so the requests
    // stay here and the first of them reaches axum through a channel.
    let (stop_serving, serving_stopped) = oneshot::channel::<()>();
    let serving = axum::serve(
        listener.tap_io(|tcp| hyperion_server::tap_socket(tcp)),
        server
            .router()
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        // The first stop request. An error would mean the sender was dropped unsent, which
        // happens only once serving is over, so it too means "stop serving".
        serving_stopped.await.ok();
    });
    let serve_then_shut_down = async {
        let serve_result = serving.await.context("server error");
        server
            .shutdown()
            .await
            .context("failed to shut down cleanly")?;
        serve_result
    };
    let stopped_then_again = async {
        let reason = requests.next().await;
        tracing::info!(%reason, "shutting down");
        // axum serves until this is sent, so its receiver is still waiting for it.
        stop_serving.send(()).ok();
        requests.next().await
    };

    match stop::unless_stopped_again(serve_then_shut_down, stopped_then_again).await {
        Shutdown::Finished(result) => result,
        Shutdown::Forced(reason) => {
            tracing::error!(
                %reason,
                "a second stop request: exiting before the shutdown finished"
            );
            std::process::exit(1)
        }
    }
}
