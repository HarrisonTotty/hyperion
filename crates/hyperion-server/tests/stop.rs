//! The server binary's stop requests, run as a process: a stop request shuts it down gracefully and
//! it exits with status 0 (galaxy plan 04, P04.T17.a and P04.T17.c).
//!
//! The tests of the stop on standard input run on every platform. The signal tests are Unix only,
//! since they send real signals with `kill`. Windows' listeners (Ctrl-C, Ctrl-Break, a closed
//! console and a system shutdown) have no counterpart here: no Windows runner exists, and raising a
//! console event takes a Win32 call that needs `unsafe`. They are compiled by the Windows Clippy
//! and are untested.
//!
//! Each server listens on port 0 in a data directory of its own, and is killed when its test ends,
//! so a server that does not stop fails the test instead of outliving it ([`common::process`]).

mod common;

use std::process::Stdio;
use std::time::Duration;

use common::process::{Running, Stopped};
use hyperion_server::config::ENV_STOP_ON_STDIN_CLOSE;
use hyperion_server::stop::StopReason;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::timeout;

/// Upper bound on an answer from `/healthz`, which does no work.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// What these tests ask of a running server, beside [`common::process`]'s own.
impl Running {
    /// Asks `/healthz` over HTTP/1.1, and checks that the server answers `ok`.
    async fn assert_healthy(&self) {
        let addr = self.addr();
        let response = timeout(ANSWER_TIMEOUT, async {
            let mut tcp = TcpStream::connect(addr)
                .await
                .expect("the server accepts a connection");
            tcp.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await
                .expect("the request is sent");
            let mut response = String::new();
            tcp.read_to_string(&mut response)
                .await
                .expect("the response reads");
            response
        })
        .await
        .expect("the server answers within ANSWER_TIMEOUT");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response:?}");
        assert!(response.ends_with("\r\n\r\nok"), "{response:?}");
    }

    /// Closes the server's standard input, which the test piped.
    fn close_stdin(&mut self) {
        drop(self.child.stdin.take().expect("stdin is piped"));
    }

    /// Sends the server `signal`, named as `kill` names it (`TERM`, `INT`).
    #[cfg(unix)]
    async fn signal(&self, signal: &str) {
        let pid = self
            .child
            .id()
            .expect("the server has not been waited for")
            .to_string();
        let status = Command::new("kill")
            .arg(format!("-{signal}"))
            .arg(&pid)
            .status()
            .await
            .expect("kill runs");
        assert!(status.success(), "kill -{signal} {pid}: {status}");
    }
}

/// Checks that the server shut down gracefully, and that `reason` was what asked it to.
fn assert_shut_down_cleanly(stopped: &Stopped, reason: StopReason) {
    assert_eq!(stopped.status.code(), Some(0), "{stopped}");
    let reason = reason.to_string();
    let shutting_down = stopped
        .line_with(&["shutting down", &reason])
        .unwrap_or_else(|| panic!("no `shutting down` line naming the {reason}: {stopped}"));
    let shut_down = stopped
        .line_with(&["server shut down"])
        .unwrap_or_else(|| panic!("no `server shut down` line: {stopped}"));
    assert!(
        shutting_down < shut_down,
        "the shutdown is logged before its end: {stopped}"
    );
}

/// Starts a server, sends it `signal`, and checks that it shut down gracefully for `reason`.
#[cfg(unix)]
async fn shuts_down_cleanly_on(signal: &str, reason: StopReason) {
    let data_dir = tempfile::tempdir().unwrap();
    let server = Running::start(data_dir.path(), |_| {}).await;
    server.signal(signal).await;
    assert_shut_down_cleanly(&server.finish().await, reason);
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_shuts_the_server_down_cleanly() {
    shuts_down_cleanly_on("TERM", StopReason::Terminate).await;
}

#[cfg(unix)]
#[tokio::test]
async fn sigint_shuts_the_server_down_cleanly() {
    shuts_down_cleanly_on("INT", StopReason::Interrupt).await;
}

/// Starts a server with a piped stdin, which `ask` asks to stop when stdin closes. Checks that it
/// serves while stdin is open, then closes stdin and checks that it shut down gracefully for that.
async fn closing_stdin_shuts_down_cleanly(ask: impl FnOnce(&mut Command)) {
    let data_dir = tempfile::tempdir().unwrap();
    let mut server = Running::start(data_dir.path(), |command| {
        command.stdin(Stdio::piped());
        ask(command);
    })
    .await;
    server.assert_healthy().await;
    server.close_stdin();
    assert_shut_down_cleanly(&server.finish().await, StopReason::StdinClosed);
}

#[tokio::test]
async fn closing_stdin_stops_the_server_cleanly_when_asked() {
    closing_stdin_shuts_down_cleanly(|command| {
        command.arg("--stop-on-stdin-close");
    })
    .await;
}

#[tokio::test]
async fn closing_stdin_stops_the_server_cleanly_when_its_variable_asks() {
    closing_stdin_shuts_down_cleanly(|command| {
        command.env(ENV_STOP_ON_STDIN_CLOSE, "1");
    })
    .await;
}

#[tokio::test]
async fn a_closed_stdin_does_not_stop_the_server_by_default() {
    let data_dir = tempfile::tempdir().unwrap();
    // `start` gives the server a null stdin, which reads as closed from the first read.
    let server = Running::start(data_dir.path(), |_| {}).await;
    server.assert_healthy().await;

    #[cfg(unix)]
    {
        server.signal("TERM").await;
        // Had the closed stdin stopped the server, the signal would have been a second request
        // and forced the exit with status 1, and the `shutting down` line would name stdin.
        assert_shut_down_cleanly(&server.finish().await, StopReason::Terminate);
    }
    #[cfg(windows)]
    {
        let stopped = server.kill().await;
        assert_eq!(
            stopped.line_with(&["shutting down"]),
            None,
            "nothing but the kill was to stop the server: {stopped}"
        );
    }
}
