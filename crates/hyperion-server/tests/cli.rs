//! The server binary's command line, run as a process: that options fall back to their
//! environment variables, that an option given wins over its variable, and that the sky is served
//! unless it is turned off.
//!
//! Each run listens on port 0 and is killed if it does not exit within [`EXIT_TIMEOUT`], so a
//! regression that starts the server fails the test instead of hanging the suite. The tests that
//! let it start read its log up to its `listening` line and kill it ([`common::process`]).

mod common;

use std::path::Path;
use std::process::Output;

use common::process::{EXIT_TIMEOUT, Running, server_binary};
use hyperion_server::config::{
    ENV_DATA_DIR, ENV_SERVE_SKY, ENV_SKY_CACHE_MB, ENV_STOP_ON_STDIN_CLOSE, ENV_WORKERS,
};
use tokio::time::timeout;

/// The exit code for a command line clap refuses.
const USAGE_ERROR: i32 = 2;

/// The exit code for an error `main` returns.
const FAILURE: i32 = 1;

/// Runs the server in `dir` with `args` and `vars`, and none of the variables it reads inherited
/// from the test's environment.
async fn run(dir: &Path, args: &[&str], vars: &[(&str, &Path)]) -> Output {
    let mut command = server_binary();
    command
        .current_dir(dir)
        .args(["--port", "0"])
        .args(args)
        .envs(vars.iter().copied())
        .kill_on_drop(true);
    timeout(EXIT_TIMEOUT, command.output())
        .await
        .expect("the server exits by itself")
        .expect("the server runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[tokio::test]
async fn the_version_is_printed() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &["--version"], &[]).await;
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        concat!("hyperion-server ", env!("CARGO_PKG_VERSION"), "\n")
    );
}

#[tokio::test]
async fn an_option_falls_back_to_its_variable() {
    let dir = tempfile::tempdir().unwrap();
    // A file where the data directory should be stops the server once it has its configuration.
    let file = dir.path().join("not-a-directory");
    std::fs::write(&file, b"").unwrap();

    let output = run(dir.path(), &[], &[(ENV_DATA_DIR, &file)]).await;
    assert_eq!(output.status.code(), Some(FAILURE), "{}", stderr(&output));
    assert!(
        stderr(&output).contains(&format!(
            "the data directory {} is not a directory",
            file.display()
        )),
        "{}",
        stderr(&output)
    );
}

#[tokio::test]
async fn a_bad_variable_is_refused_as_its_option() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &[], &[(ENV_WORKERS, Path::new("0"))]).await;
    assert_eq!(
        output.status.code(),
        Some(USAGE_ERROR),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("invalid value '0' for '--num-workers <COUNT>'"),
        "{}",
        stderr(&output)
    );
}

#[tokio::test]
async fn the_stop_on_stdin_close_variable_takes_only_a_yes_or_a_no() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(
        dir.path(),
        &[],
        &[(ENV_STOP_ON_STDIN_CLOSE, Path::new("sometimes"))],
    )
    .await;
    assert_eq!(
        output.status.code(),
        Some(USAGE_ERROR),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("invalid value 'sometimes' for '--stop-on-stdin-close'"),
        "{}",
        stderr(&output)
    );
}

#[tokio::test]
async fn an_option_wins_over_its_variable() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("not-a-directory");
    std::fs::write(&file, b"").unwrap();

    // The variable alone is refused (above); given the option, the server gets as far as the
    // data directory.
    let output = run(
        dir.path(),
        &["--num-workers", "1"],
        &[(ENV_WORKERS, Path::new("0")), (ENV_DATA_DIR, &file)],
    )
    .await;
    assert_eq!(output.status.code(), Some(FAILURE), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("is not a directory"),
        "{}",
        stderr(&output)
    );
}

/// The sky's cell cache takes its budget from `HYPERION_SKY_CACHE_MB` when `--sky-cache` is not
/// given (rendering plan R06, R06.T11.b): the server states it in MiB as it starts.
#[tokio::test]
async fn the_sky_caches_budget_falls_back_to_its_variable() {
    let dir = tempfile::tempdir().unwrap();
    let server = Running::start(dir.path(), |command| {
        command.env(ENV_SKY_CACHE_MB, "3");
    })
    .await;
    let stopped = server.kill().await;
    let started = stopped
        .line_with(&["server started"])
        .unwrap_or_else(|| panic!("the server logs its start: {stopped}"));
    assert!(
        stopped.lines[started].contains("sky_cache_mib=3"),
        "the start states the variable's budget: {stopped}"
    );
}

/// The sky's service a server started with `configure` states as it starts.
async fn started_sky_service(configure: impl FnOnce(&mut tokio::process::Command)) -> String {
    let dir = tempfile::tempdir().unwrap();
    let stopped = Running::start(dir.path(), configure).await.kill().await;
    let started = stopped
        .line_with(&["server started"])
        .unwrap_or_else(|| panic!("the server logs its start: {stopped}"));
    stopped.lines[started]
        .split_whitespace()
        .find_map(|field| field.strip_prefix("sky_service="))
        .unwrap_or_else(|| panic!("the start states the sky's service: {stopped}"))
        .to_owned()
}

/// The server serves the sky by default, and `--serve-sky=false` or its variable turns it off
/// (rendering plan R06, R06.T11.c; on by default since rendering plan R13's R13.T2). The option
/// wins over its variable.
#[tokio::test]
async fn the_sky_is_served_unless_it_is_turned_off() {
    assert_eq!(started_sky_service(|_| {}).await, "Served");
    assert_eq!(
        started_sky_service(|command| {
            command.arg("--serve-sky=false");
        })
        .await,
        "Unsupported"
    );
    assert_eq!(
        started_sky_service(|command| {
            command.env(ENV_SERVE_SKY, "0");
        })
        .await,
        "Unsupported"
    );
    assert_eq!(
        started_sky_service(|command| {
            command.env(ENV_SERVE_SKY, "1").arg("--serve-sky=false");
        })
        .await,
        "Unsupported"
    );
}

#[tokio::test]
async fn the_serve_sky_variable_takes_only_a_yes_or_a_no() {
    let dir = tempfile::tempdir().unwrap();
    for value in ["maybe", ""] {
        let output = run(dir.path(), &[], &[(ENV_SERVE_SKY, Path::new(value))]).await;
        assert_eq!(
            output.status.code(),
            Some(USAGE_ERROR),
            "{value:?}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains(&format!("invalid value '{value}' for '--serve-sky")),
            "{value:?}: {}",
            stderr(&output)
        );
    }
}
