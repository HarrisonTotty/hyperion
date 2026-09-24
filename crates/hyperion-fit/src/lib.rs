//! Offline fitting for HYPERION: the tool that writes the generator's fitted constant tables.
//!
//! Some of the generator's numbers are the result of a fit that is too slow, or too fiddly, to run
//! each time a galaxy is built: the dimensionless coefficients of the Gaussian expansions behind
//! the potential, and later the samplers and constants of plan 15. This crate runs those fits and
//! renders each result as Rust source, which is committed under `crates/hyperion-sim/src/tables/`
//! with a header naming the tool, its inputs and its version.
//!
//! Nothing depends on this crate: it depends on `hyperion-sim`, never the reverse. Every
//! transcendental it evaluates goes through [`hyperion_sim::math`], every quadrature through
//! [`hyperion_sim::galaxy::quad`], and every fit takes a fixed number of steps with no randomness
//! and no threads, so a table comes out bit for bit the same on every platform. Each task's test
//! renders the fit again and compares it with the committed table, so a stale or hand-edited
//! table fails CI.
//!
//! Three tasks exist so far: plan 02's [`tasks::mge`] (P02.T6.a), run as `hyperion-fit run mge`,
//! plan 13's [`tasks::giant_cooling`] (P13.T5.b), run as `hyperion-fit run giant_cooling`, and plan
//! 06's [`tasks::wd_cooling`] (P06.T20.a), run as `hyperion-fit run wd_cooling [--data <dir>]`.
//! Plan 15 extends the crate.

pub mod tasks;

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// Where `run mge` writes its table unless `--out` says otherwise: the sim's `tables/mge.rs`,
/// fixed at compile time so that the tool works from any directory.
pub const DEFAULT_MGE_OUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../hyperion-sim/src/tables/mge.rs"
);

/// Where `run giant_cooling` writes its table unless `--out` says otherwise: the sim's
/// `tables/giant_cooling.rs`.
pub const DEFAULT_GIANT_COOLING_OUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../hyperion-sim/src/tables/giant_cooling.rs"
);

/// Where `run wd_cooling` writes its table unless `--out` says otherwise: the sim's
/// `tables/wd_cooling.rs`.
pub const DEFAULT_WD_COOLING_OUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../hyperion-sim/src/tables/wd_cooling.rs"
);

/// The command line's usage, for error messages.
pub const USAGE: &str = "usage: hyperion-fit run <mge | giant_cooling | wd_cooling> [--out <path>] \
                         [--data <dir>, wd_cooling only]";

/// What the command line asked for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Command {
    /// Fit the Gaussian expansions of the potential ([`tasks::mge`]) and write the table to
    /// `out`.
    RunMge {
        /// The file to write.
        out: PathBuf,
    },
    /// Fit the cooling of giant planets ([`tasks::giant_cooling`]) and write the table to `out`.
    RunGiantCooling {
        /// The file to write.
        out: PathBuf,
    },
    /// Fit the cooling of white dwarfs ([`tasks::wd_cooling`]) to the Montreal sequences in
    /// `data` and write the table to `out`.
    RunWdCooling {
        /// The file to write.
        out: PathBuf,
        /// The directory holding the sequences' files.
        data: PathBuf,
    },
}

impl Command {
    /// Parses the arguments after the program's name: `run <task> [--out <path>]`, the task
    /// `mge`, `giant_cooling` or `wd_cooling`, and for `wd_cooling` also `[--data <dir>]`.
    ///
    /// # Errors
    ///
    /// [`RunFitError::Usage`] for a malformed command line, [`RunFitError::UnknownTask`] for a
    /// task other than `mge`, `giant_cooling` and `wd_cooling`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_fit::{Command, RunFitError};
    ///
    /// let command = Command::parse(["run", "mge", "--out", "table.rs"].map(String::from))?;
    /// assert_eq!(command, Command::RunMge { out: "table.rs".into() });
    /// # Ok::<(), RunFitError>(())
    /// ```
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, RunFitError> {
        let mut args = args.into_iter();
        match args.next().as_deref() {
            Some("run") => {}
            Some(other) => return Err(RunFitError::Usage(format!("unknown command `{other}`"))),
            None => return Err(RunFitError::Usage("no command given".to_owned())),
        }
        let (task, default): (fn(PathBuf, Option<PathBuf>) -> Self, _) =
            match args.next().as_deref() {
                Some("mge") => (|out, _| Self::RunMge { out }, DEFAULT_MGE_OUT),
                Some("giant_cooling") => (
                    |out, _| Self::RunGiantCooling { out },
                    DEFAULT_GIANT_COOLING_OUT,
                ),
                Some("wd_cooling") => (
                    |out, data| Self::RunWdCooling {
                        out,
                        data: data
                            .unwrap_or_else(|| PathBuf::from(tasks::wd_cooling::DEFAULT_DATA_DIR)),
                    },
                    DEFAULT_WD_COOLING_OUT,
                ),
                Some(other) => return Err(RunFitError::UnknownTask(other.to_owned())),
                None => return Err(RunFitError::Usage("no task given".to_owned())),
            };
        let takes_data = default == DEFAULT_WD_COOLING_OUT;
        let mut out = PathBuf::from(default);
        let mut data = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--out" => match args.next() {
                    Some(path) => out = PathBuf::from(path),
                    None => return Err(RunFitError::Usage("`--out` needs a path".to_owned())),
                },
                "--data" if takes_data => match args.next() {
                    Some(path) => data = Some(PathBuf::from(path)),
                    None => return Err(RunFitError::Usage("`--data` needs a path".to_owned())),
                },
                other => {
                    return Err(RunFitError::Usage(format!("unexpected argument `{other}`")));
                }
            }
        }
        Ok(task(out, data))
    }
}

/// A fitting task could not run.
#[derive(Debug)]
pub enum RunFitError {
    /// The command line was malformed; the text says how.
    Usage(String),
    /// The command line named a task that does not exist.
    UnknownTask(String),
    /// The input of a fit could not be read.
    Read(tasks::wd_cooling::ReadSequencesError),
    /// The table could not be written.
    Write {
        /// The file being written.
        path: PathBuf,
        /// Why it failed.
        source: io::Error,
    },
}

impl RunFitError {
    /// The process exit code for this error: 2 for a bad command line, 1 for a failed write.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::UnknownTask(_) => 2,
            Self::Read(_) | Self::Write { .. } => 1,
        }
    }
}

impl fmt::Display for RunFitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(why) => write!(f, "{why}; {USAGE}"),
            Self::UnknownTask(task) => write!(f, "unknown task `{task}`; {USAGE}"),
            Self::Read(error) => write!(f, "cannot read the fit's input: {error}"),
            Self::Write { path, .. } => write!(f, "cannot write {}", path.display()),
        }
    }
}

impl Error for RunFitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Usage(_) | Self::UnknownTask(_) => None,
            Self::Read(error) => Some(error),
            Self::Write { source, .. } => Some(source),
        }
    }
}

/// Runs `command` and returns the path it wrote.
///
/// # Errors
///
/// [`RunFitError::Read`] if a fit's input cannot be read, and [`RunFitError::Write`] if the
/// output file cannot be written.
pub fn run(command: &Command) -> Result<PathBuf, RunFitError> {
    let (out, source) = match command {
        Command::RunMge { out } => (out, tasks::mge::render(&tasks::mge::fit())),
        Command::RunGiantCooling { out } => (
            out,
            tasks::giant_cooling::render(&tasks::giant_cooling::fit()),
        ),
        Command::RunWdCooling { out, data } => {
            let sequences = tasks::wd_cooling::read(data).map_err(RunFitError::Read)?;
            (
                out,
                tasks::wd_cooling::render(&tasks::wd_cooling::fit(&sequences)),
            )
        }
    };
    std::fs::write(out, source).map_err(|source| RunFitError::Write {
        path: out.clone(),
        source,
    })?;
    Ok(out.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, RunFitError> {
        Command::parse(args.iter().map(|&a| a.to_owned()))
    }

    #[test]
    fn run_mge_defaults_to_the_sims_table() {
        let Ok(Command::RunMge { out }) = parse(&["run", "mge"]) else {
            panic!("`run mge` parses to `RunMge`");
        };
        assert!(
            out.ends_with("hyperion-sim/src/tables/mge.rs"),
            "{}",
            out.display()
        );
    }

    #[test]
    fn run_giant_cooling_defaults_to_the_sims_table_and_takes_an_out() {
        let Ok(Command::RunGiantCooling { out }) = parse(&["run", "giant_cooling"]) else {
            panic!("`run giant_cooling` parses to `RunGiantCooling`");
        };
        assert!(
            out.ends_with("hyperion-sim/src/tables/giant_cooling.rs"),
            "{}",
            out.display()
        );
        assert_eq!(
            parse(&["run", "giant_cooling", "--out", "table.rs"]).unwrap(),
            Command::RunGiantCooling {
                out: "table.rs".into()
            }
        );
    }

    #[test]
    fn run_wd_cooling_defaults_to_the_sims_table_and_takes_its_data() {
        let Ok(Command::RunWdCooling { out, data }) = parse(&["run", "wd_cooling"]) else {
            panic!("`run wd_cooling` parses to `RunWdCooling`");
        };
        assert!(
            out.ends_with("hyperion-sim/src/tables/wd_cooling.rs"),
            "{}",
            out.display()
        );
        assert!(
            data.ends_with("target/data/montreal_cooling"),
            "{}",
            data.display()
        );
        assert_eq!(
            parse(&["run", "wd_cooling", "--data", "seqs", "--out", "t.rs"]).unwrap(),
            Command::RunWdCooling {
                out: "t.rs".into(),
                data: "seqs".into()
            }
        );
        assert!(matches!(
            parse(&["run", "mge", "--data", "seqs"]),
            Err(RunFitError::Usage(_))
        ));
    }

    #[test]
    fn missing_sequences_are_a_read_error() {
        let command = Command::RunWdCooling {
            out: "unused.rs".into(),
            data: "/nonexistent-directory/for/hyperion-fit".into(),
        };
        match run(&command) {
            Err(error @ RunFitError::Read(_)) => {
                assert_eq!(error.exit_code(), 1);
                assert!(error.to_string().contains("seq_020_thick.txt"), "{error}");
                assert!(error.source().is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_malformed_command_line_is_a_usage_error() {
        assert!(matches!(parse(&[]), Err(RunFitError::Usage(_))));
        assert!(matches!(parse(&["fit", "mge"]), Err(RunFitError::Usage(_))));
        assert!(matches!(parse(&["run"]), Err(RunFitError::Usage(_))));
        assert!(matches!(
            parse(&["run", "mge", "--out"]),
            Err(RunFitError::Usage(_))
        ));
        assert!(matches!(
            parse(&["run", "mge", "--verbose"]),
            Err(RunFitError::Usage(_))
        ));
        match parse(&["run", "kicks"]) {
            Err(error @ RunFitError::UnknownTask(_)) => {
                assert_eq!(error.exit_code(), 2);
                assert_eq!(error.to_string(), format!("unknown task `kicks`; {USAGE}"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_failed_write_names_the_path() {
        let out = PathBuf::from("/nonexistent-directory/for/hyperion-fit/mge.rs");
        match run(&Command::RunMge { out: out.clone() }) {
            Err(error @ RunFitError::Write { .. }) => {
                assert_eq!(error.exit_code(), 1);
                assert_eq!(error.to_string(), format!("cannot write {}", out.display()));
                assert!(error.source().is_some());
            }
            other => panic!("{other:?}"),
        }
    }
}
