//! The server binary run as a process, shared by the tests of its command line and of its stop
//! requests: the binary with none of its variables inherited, a server read up to its `listening`
//! line, and how a server ended.
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
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};
use tokio::time::timeout;

/// Upper bound on a start, to the `listening` line. Nothing is loaded, since the data directory is
/// empty, so this is margin for a loaded machine.
pub const START_TIMEOUT: Duration = Duration::from_secs(30);

/// Upper bound on a run of the binary that exits before serving, and on the exit after a stop
/// request or a kill: neither has connections to close or jobs to finish.
pub const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// The server binary, with none of the variables it reads inherited from the test's environment:
/// every option's, read from the parser itself, so that a new one is cleared too.
pub fn server_binary() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hyperion-server"));
    for name in ServerArgs::command()
        .get_arguments()
        .filter_map(clap::Arg::get_env)
    {
        command.env_remove(name);
    }
    command
}

/// A server process, read up to its `listening` line.
pub struct Running {
    /// The process.
    pub child: Child,
    stdout: Lines<BufReader<ChildStdout>>,
    /// The log lines read so far.
    lines: Vec<String>,
}

/// How a server ended: its status and everything it wrote.
pub struct Stopped {
    /// How it exited.
    pub status: ExitStatus,
    /// Its log lines, in order.
    pub lines: Vec<String>,
    /// Everything it wrote to stderr.
    pub stderr: String,
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
    pub fn line_with(&self, texts: &[&str]) -> Option<usize> {
        self.lines
            .iter()
            .position(|line| texts.iter().all(|text| line.contains(text)))
    }
}

impl Running {
    /// Starts the server on port 0 in `data_dir` with one worker, logging at `info` without
    /// colours, with a null stdin, and with none of the variables it reads inherited from the
    /// test's environment. `configure` then adds to those settings or overrides them.
    pub async fn start(data_dir: &Path, configure: impl FnOnce(&mut Command)) -> Self {
        let mut command = server_binary();
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
    pub async fn next_line(&mut self) -> Option<&str> {
        let line = self.stdout.next_line().await.expect("stdout reads")?;
        self.lines.push(line);
        self.lines.last().map(String::as_str)
    }

    /// The address the server listens on, read from its `listening` line's `addr` field.
    pub fn addr(&self) -> SocketAddr {
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

    /// Kills the server, as Node's `subprocess.kill()` does on Windows (`TerminateProcess`), and
    /// waits for it.
    pub async fn kill(mut self) -> Stopped {
        self.child.start_kill().expect("the server can be killed");
        self.finish().await
    }

    /// Reads the rest of the server's output and waits for it to exit.
    pub async fn finish(mut self) -> Stopped {
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
