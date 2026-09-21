//! The server binary's command line, run as a process: that options fall back to their
//! environment variables, and that an option given wins over its variable.
//!
//! Each run listens on port 0 and is killed if it does not exit within [`EXIT_TIMEOUT`], so a
//! regression that starts the server fails the test instead of hanging the suite.

use std::path::Path;
use std::process::Output;
use std::time::Duration;

use hyperion_server::config::{
    ENV_ADDR, ENV_CELL_CACHE_MB, ENV_DATA_DIR, ENV_MAP_CACHE_MB, ENV_PORT, ENV_WORKERS,
};
use tokio::process::Command;
use tokio::time::timeout;

/// Upper bound on a run of the binary, which exits before serving in every test here.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// The exit code for a command line clap refuses.
const USAGE_ERROR: i32 = 2;

/// The exit code for an error `main` returns.
const FAILURE: i32 = 1;

/// Runs the server in `dir` with `args` and `vars`, and none of the variables it reads inherited
/// from the test's environment.
async fn run(dir: &Path, args: &[&str], vars: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hyperion-server"));
    for name in [
        ENV_ADDR,
        ENV_PORT,
        ENV_DATA_DIR,
        ENV_WORKERS,
        ENV_CELL_CACHE_MB,
        ENV_MAP_CACHE_MB,
    ] {
        command.env_remove(name);
    }
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
