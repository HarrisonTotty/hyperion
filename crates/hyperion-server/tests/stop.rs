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
//! so a server that does not stop fails the test instead of outliving it.

use std::fmt;
use std::net::SocketAddr;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use clap::CommandFactory;
use hyperion_server::ServerArgs;
use hyperion_server::config::ENV_STOP_ON_STDIN_CLOSE;
use hyperion_server::stop::StopReason;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::TcpStream;
use tokio::process::{Child, ChildStdout, Command};
use tokio::time::timeout;

/// Upper bound on a start, to the `listening` line. Nothing is loaded, since the data directory is
/// empty, so this is margin for a loaded machine.
const START_TIMEOUT: Duration = Duration::from_secs(30);

/// Upper bound on the exit after a stop request: the shutdown has no connections to close and no
/// jobs to finish.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Upper bound on an answer from `/healthz`, which does no work.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// A server process, read up to its `listening` line.
struct Running {
    child: Child,
    stdout: Lines<BufReader<ChildStdout>>,
    /// The log lines read so far.
    lines: Vec<String>,
}

/// How a server ended: its status and everything it wrote.
struct Stopped {
    status: ExitStatus,
    lines: Vec<String>,
    stderr: String,
}

impl fmt::Display for Stopped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}; stdout:", self.status)?;
        for line in &self.lines {
            writeln!(f, "  {line}")?;
        }
        write!(f, "stderr:\n{}", self.stderr)
    }
}

impl Stopped {
    /// The index of the first line holding every one of `texts`.
    fn line_with(&self, texts: &[&str]) -> Option<usize> {
        self.lines
            .iter()
            .position(|line| texts.iter().all(|text| line.contains(text)))
    }
}

impl Running {
    /// Starts the server on port 0 in `data_dir` with one worker, logging at `info` without
    /// colours, with a null stdin, and with none of the variables it reads inherited from the
    /// test's environment. `configure` then adds to those settings or overrides them.
    async fn start(data_dir: &Path, configure: impl FnOnce(&mut Command)) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_hyperion-server"));
        // Every option's variable, read from the parser itself, so that a new one is cleared too.
        for name in ServerArgs::command()
            .get_arguments()
            .filter_map(clap::Arg::get_env)
        {
            command.env_remove(name);
        }
        command
            .args(["--port", "0", "--num-workers", "1", "--data-dir"])
            .arg(data_dir)
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        configure(&mut command);
        let mut child = command.spawn().expect("the server runs");
        let stdout = child.stdout.take().expect("stdout is piped");
        let mut running = Self {
            child,
            stdout: BufReader::new(stdout).lines(),
            lines: Vec::new(),
        };
        let listening = timeout(START_TIMEOUT, async {
            while let Some(line) = running.next_line().await {
                if line.contains("listening") {
                    return true;
                }
            }
            false
        })
        .await
        .expect("the server starts listening within START_TIMEOUT");
        if listening {
            return running;
        }
        panic!(
            "the server ended before listening: {}",
            running.finish().await
        );
    }

    /// Reads the next log line, or `None` once the server has closed its output.
    async fn next_line(&mut self) -> Option<&str> {
        let line = self.stdout.next_line().await.expect("stdout reads")?;
        self.lines.push(line);
        self.lines.last().map(String::as_str)
    }

    /// The address the server listens on, read from its `listening` line's `addr` field.
    fn addr(&self) -> SocketAddr {
        let line = self
            .lines
            .iter()
            .find(|line| line.contains("listening"))
            .expect("the server has started listening");
        line.split_whitespace()
            .find_map(|field| field.strip_prefix("addr="))
            .and_then(|addr| addr.parse().ok())
            .unwrap_or_else(|| panic!("no address in the `listening` line {line:?}"))
    }

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

    /// Ends the server by `TerminateProcess`, as Node's `subprocess.kill()` does on Windows, and
    /// waits for it.
    #[cfg(windows)]
    async fn kill(mut self) -> Stopped {
        self.child.start_kill().expect("the server can be killed");
        self.finish().await
    }

    /// Reads the rest of the server's output and waits for it to exit.
    async fn finish(mut self) -> Stopped {
        timeout(EXIT_TIMEOUT, async {
            while self.next_line().await.is_some() {}
            let mut stderr = String::new();
            if let Some(mut pipe) = self.child.stderr.take() {
                pipe.read_to_string(&mut stderr)
                    .await
                    .expect("stderr reads");
            }
            let status = self.child.wait().await.expect("the server is waited for");
            Stopped {
                status,
                lines: self.lines,
                stderr,
            }
        })
        .await
        .expect("the server exits within EXIT_TIMEOUT")
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
