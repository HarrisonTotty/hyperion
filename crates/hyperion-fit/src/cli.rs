//! The command line (plan 15, P15.T1):
//!
//! ```text
//! hyperion-fit list                      # tasks, class (fast or slow), table path, state
//! hyperion-fit run <task> [--since <generator version>] [--smoke] [--threads N] [--out PATH]
//!                         [--data DIR] [--manifest PATH]
//! hyperion-fit check [--rerun-fast]      # staleness
//! hyperion-fit fingerprint <task>        # prints the sim probe values a task depends on
//! hyperion-fit orbits [--smoke | --manifest PATH] [--threads N] [--out DIR] [--max-parts N]
//!                                        # the displaced form table's orbit run (P15.T6.b),
//!                                        # resumable from DIR/parts (ruling 120.4)
//! ```
//!
//! `run` writes into the sim's `tables/` unless `--out` says otherwise, and then needs `--since`
//! equal to the sim's `GENERATOR_VERSION` (Design note 10); `--out` elsewhere needs none, which is
//! how plan 02's `run mge --out <path>` keeps working. `--smoke` runs the task's smoke manifest,
//! and `--manifest` another manifest, and both need `--out`. `--data` reads the task's one dataset from another directory, for a fetched
//! dataset kept outside the cache.

use std::io::Write;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::tables::MANIFEST;

use crate::RunFitError;
use crate::check::{CheckInputs, Rerun, check};
use crate::emit::{Destination, Workspace, write_table};
use crate::manifest::Manifest;
use crate::pipeline::{ManifestKind, load_manifest, prepare};
use crate::task::{self, FitTask};

/// `hyperion-fit`: runs the offline fits and checks the tables they wrote.
#[derive(Debug, Clone, PartialEq, Eq, Parser)]
#[command(name = "hyperion-fit", version, about)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// A subcommand.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// List the tasks: name, class, table path and state.
    List,
    /// Run a task and write its table.
    Run {
        /// The task.
        task: String,
        /// The generator version the table takes effect at: required when writing into the sim's
        /// tables, and equal to its `GENERATOR_VERSION`.
        #[arg(long)]
        since: Option<u32>,
        /// Run the smoke manifest, a reduced run of a few seconds; needs `--out`.
        #[arg(long)]
        smoke: bool,
        /// Threads for the fit; the output is the same for any number.
        #[arg(long)]
        threads: Option<NonZeroUsize>,
        /// Write the table here instead of into the sim's tables.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Read the task's dataset from this directory.
        #[arg(long)]
        data: Option<PathBuf>,
        /// Run this manifest instead of the task's own; needs `--out`, since the committed table
        /// is always made from `manifests/<task>.toml`.
        #[arg(long, conflicts_with = "smoke")]
        manifest: Option<PathBuf>,
    },
    /// Check that no table is stale, hand-edited, ahead of the generator version or unregistered.
    Check {
        /// Also rerun every fast task and compare bytes.
        #[arg(long)]
        rerun_fast: bool,
        /// Threads for the reruns.
        #[arg(long)]
        threads: Option<NonZeroUsize>,
    },
    /// Print the sim probe values a task's table depends on.
    Fingerprint {
        /// The task.
        task: String,
    },
    /// Run the displaced form table's orbits (plan 15, P15.T6.b) and write their histograms.
    Orbits {
        /// Run the smoke manifest, a few orbits per class.
        #[arg(long)]
        smoke: bool,
        /// Run this manifest instead of `manifests/displaced_orbits.toml`.
        #[arg(long, conflicts_with = "smoke")]
        manifest: Option<PathBuf>,
        /// Threads for the run; the output is the same for any number.
        #[arg(long)]
        threads: Option<NonZeroUsize>,
        /// The directory to write `displaced_orbits.txt` into; `data/cache/displaced/` by default.
        /// Finished parts are kept in its `parts/`, and a later invocation resumes from them.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Compute at most this many parts, then stop; run again to resume (ruling 120.4).
        #[arg(long)]
        max_parts: Option<u64>,
    },
}

/// The threads to use: `threads`, or every core the machine offers.
fn threads_or_all(threads: Option<NonZeroUsize>) -> NonZeroUsize {
    threads.unwrap_or_else(|| std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN))
}

/// The task named `name`.
fn find(name: &str) -> Result<&'static dyn FitTask, RunFitError> {
    task::find(name).ok_or_else(|| RunFitError::UnknownTask(name.to_owned()))
}

/// Runs `cli` against the repository, printing to `out`.
///
/// # Errors
///
/// [`RunFitError::UnknownTask`] for a task that is not registered; the task's, the emitter's and
/// the manifest's errors from `run`; [`RunFitError::Check`] if `check` finds a stale table;
/// [`RunFitError::Output`] if `out` cannot be written.
pub fn run(cli: &Cli, out: &mut dyn Write) -> Result<(), RunFitError> {
    run_in(cli, &Workspace::repository(), out)
}

/// Runs `cli` against `workspace`, printing to `out`.
///
/// # Errors
///
/// As [`run`].
pub fn run_in(cli: &Cli, workspace: &Workspace, out: &mut dyn Write) -> Result<(), RunFitError> {
    match &cli.command {
        Command::List => list(workspace, out),
        Command::Run {
            task,
            since,
            smoke,
            threads,
            out: out_path,
            data,
            manifest,
        } => {
            let task = find(task)?;
            if (*smoke || manifest.is_some()) && out_path.is_none() {
                return Err(RunFitError::OtherManifestNeedsOut);
            }
            let mut loaded = match manifest {
                Some(path) => Manifest::load(path)?,
                None if *smoke => load_manifest(workspace, task, ManifestKind::Smoke)?,
                None => load_manifest(workspace, task, ManifestKind::Full)?,
            };
            if let Some(dir) = data {
                let dataset = match loaded.datasets() {
                    [one] => one.clone(),
                    _ => return Err(RunFitError::DataNeedsOneDataset(task.name().to_owned())),
                };
                loaded = loaded.with_data_dir(&dataset, dir.clone());
            }
            run_task(
                workspace,
                task,
                &loaded,
                *since,
                threads_or_all(*threads),
                out_path.as_deref(),
                out,
            )
        }
        Command::Check {
            rerun_fast,
            threads,
        } => {
            let report = check(
                &CheckInputs {
                    workspace,
                    tasks: task::registry(),
                    manifest: MANIFEST,
                    current: GENERATOR_VERSION.get(),
                },
                if *rerun_fast {
                    Rerun::Fast
                } else {
                    Rerun::None
                },
                threads_or_all(*threads),
            );
            writeln!(out, "{report}")?;
            if report.passed() {
                Ok(())
            } else {
                Err(RunFitError::Check(report))
            }
        }
        Command::Orbits {
            smoke,
            manifest,
            threads,
            out: out_dir,
            max_parts,
        } => orbits(
            workspace,
            OrbitsRun {
                smoke: *smoke,
                manifest: manifest.as_deref(),
                threads: threads_or_all(*threads),
                out_dir: out_dir.as_deref(),
                max_parts: *max_parts,
            },
            out,
        ),
        Command::Fingerprint { task } => {
            let task = find(task)?;
            let fingerprint = task.fingerprint();
            if fingerprint.is_empty() {
                writeln!(out, "{}: none (the task uses only `math`)", task.name())?;
            }
            for (probe, value) in fingerprint.probes() {
                writeln!(out, "{probe} = {value:?}")?;
            }
            Ok(())
        }
    }
}

/// What `orbits` was asked to run.
#[derive(Clone, Copy)]
struct OrbitsRun<'a> {
    smoke: bool,
    manifest: Option<&'a std::path::Path>,
    threads: NonZeroUsize,
    out_dir: Option<&'a std::path::Path>,
    max_parts: Option<u64>,
}

/// `orbits`: runs the displaced form table's orbits in resumable parts and, once every part is
/// done, writes their histograms, printing the file's SHA-256 (plan 15, P15.T6.b; ruling 120.4).
fn orbits(
    workspace: &Workspace,
    run: OrbitsRun<'_>,
    out: &mut dyn Write,
) -> Result<(), RunFitError> {
    use crate::tasks::displaced_forms;
    let path = match run.manifest {
        Some(path) => path.to_path_buf(),
        None if run.smoke => workspace.manifests_dir.join("displaced_orbits.smoke.toml"),
        None => workspace.manifests_dir.join("displaced_orbits.toml"),
    };
    let loaded = Manifest::load(&path).map_err(RunFitError::Manifest)?;
    let params = displaced_forms::RunParams::from_manifest(&loaded)
        .map_err(|e| RunFitError::Task(task::RunTaskError::Param(e)))?;
    let dir = run.out_dir.map_or_else(
        || workspace.data_dir.join("cache").join("displaced"),
        std::path::Path::to_path_buf,
    );
    let parts = dir.join("parts");
    let resume = displaced_forms::Resume {
        dir: &parts,
        max_parts: run.max_parts,
    };
    let hash = displaced_forms::manifest_hash(&loaded);
    let records = displaced_forms::run(&params, &hash, run.threads, Some(resume)).map_err(|e| {
        RunFitError::Task(task::RunTaskError::Input {
            task: displaced_forms::TASK,
            source: Box::new(e),
        })
    })?;
    let Some(records) = records else {
        writeln!(
            out,
            "stopped after {} parts; run again to resume from {}",
            run.max_parts.unwrap_or(0),
            parts.display()
        )?;
        return Ok(());
    };
    let text = displaced_forms::render(&loaded, &records);
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("displaced_orbits.txt");
    std::fs::write(&file, &text)?;
    writeln!(
        out,
        "wrote {} ({} classes), sha256 {}",
        file.display(),
        records.len(),
        displaced_forms::sha256_hex(text.as_bytes())
    )?;
    Ok(())
}

/// `list`: every task's name, class, table and state.
fn list(workspace: &Workspace, out: &mut dyn Write) -> Result<(), RunFitError> {
    let tasks = task::registry();
    let report = check(
        &CheckInputs {
            workspace,
            tasks,
            manifest: MANIFEST,
            current: GENERATOR_VERSION.get(),
        },
        Rerun::None,
        NonZeroUsize::MIN,
    );
    writeln!(out, "{:<16}{:<7}{:<20}state", "task", "class", "table")?;
    for task in tasks {
        let name = task.name();
        let state = if report.findings.iter().any(|f| f.table == name) {
            "stale"
        } else if report.warnings.iter().any(|(t, _)| t == name) {
            "provisional"
        } else {
            "fresh"
        };
        writeln!(
            out,
            "{name:<16}{:<7}{:<20}{state}",
            task.class().as_str(),
            task.table_path()
        )?;
    }
    Ok(())
}

/// `run`: runs `task` on `manifest` and writes its table to `out_path`, or into the tables.
fn run_task(
    workspace: &Workspace,
    task: &'static dyn FitTask,
    manifest: &Manifest,
    since: Option<u32>,
    threads: NonZeroUsize,
    out_path: Option<&std::path::Path>,
    out: &mut dyn Write,
) -> Result<(), RunFitError> {
    let current = GENERATOR_VERSION.get();
    let prepared = prepare(task::registry(), task, manifest, workspace, threads)?;
    let destination = match out_path {
        Some(path) => Destination::Out { path, current },
        None => Destination::Tables {
            workspace,
            since,
            current,
        },
    };
    let written = write_table(&prepared.emission(), destination)?;
    writeln!(
        out,
        "wrote {} (since generator version {}{})",
        written.path.display(),
        written.since,
        if written.body_changed {
            ""
        } else {
            "; body unchanged"
        }
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("hyperion-fit").chain(args.iter().copied()))
    }

    #[test]
    fn the_four_commands_parse() {
        assert_eq!(parse(&["list"]).unwrap().command, Command::List);
        assert_eq!(
            parse(&["run", "mge", "--out", "table.rs"]).unwrap().command,
            Command::Run {
                task: "mge".to_owned(),
                since: None,
                smoke: false,
                threads: None,
                out: Some("table.rs".into()),
                data: None,
                manifest: None,
            }
        );
        assert_eq!(
            parse(&[
                "run",
                "kick_rank",
                "--since",
                "11",
                "--threads",
                "4",
                "--smoke"
            ])
            .unwrap()
            .command,
            Command::Run {
                task: "kick_rank".to_owned(),
                since: Some(11),
                smoke: true,
                threads: NonZeroUsize::new(4),
                out: None,
                data: None,
                manifest: None,
            }
        );
        assert_eq!(
            parse(&["check", "--rerun-fast"]).unwrap().command,
            Command::Check {
                rerun_fast: true,
                threads: None
            }
        );
        assert_eq!(
            parse(&["fingerprint", "kick_rank"]).unwrap().command,
            Command::Fingerprint {
                task: "kick_rank".to_owned()
            }
        );
        assert!(parse(&[]).is_err());
        assert!(parse(&["fit", "mge"]).is_err());
        assert!(parse(&["run"]).is_err());
        assert!(parse(&["run", "mge", "--threads", "0"]).is_err());
    }

    #[test]
    fn list_prints_registered_tasks() {
        let mut out = Vec::new();
        run(&parse(&["list"]).unwrap(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let rows: Vec<&str> = text.lines().skip(1).collect();
        assert_eq!(rows.len(), task::registry().len(), "{text}");
        for (row, task) in rows.iter().zip(task::registry()) {
            assert!(row.starts_with(task.name()), "{row}");
            assert!(row.contains(task.table_path()), "{row}");
            assert!(row.contains(task.class().as_str()), "{row}");
        }
        assert!(text.contains("mge             fast   mge.rs"), "{text}");
    }

    #[test]
    fn an_unknown_task_and_a_smoke_run_into_the_tables_are_refused() {
        let mut out = Vec::new();
        match run(&parse(&["run", "kicks"]).unwrap(), &mut out) {
            Err(error @ RunFitError::UnknownTask(_)) => {
                assert_eq!(error.exit_code(), 2);
                assert!(error.to_string().contains("kicks"), "{error}");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            run(&parse(&["run", "mge", "--smoke"]).unwrap(), &mut out),
            Err(RunFitError::OtherManifestNeedsOut)
        ));
        assert!(matches!(
            run(
                &parse(&["run", "mge", "--manifest", "other.toml"]).unwrap(),
                &mut out
            ),
            Err(RunFitError::OtherManifestNeedsOut)
        ));
        assert!(matches!(
            run(&parse(&["fingerprint", "kicks"]).unwrap(), &mut out),
            Err(RunFitError::UnknownTask(_))
        ));
    }

    #[test]
    fn writing_into_the_tables_needs_since() {
        let mut out = Vec::new();
        match run(&parse(&["run", "mge"]).unwrap(), &mut out) {
            Err(RunFitError::Emit(crate::emit::EmitTableError::SinceRequired)) => {}
            other => panic!("{other:?}"),
        }
        match run(&parse(&["run", "mge", "--since", "1"]).unwrap(), &mut out) {
            Err(RunFitError::Emit(crate::emit::EmitTableError::SinceNotCurrent {
                since: 1,
                ..
            })) => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_fingerprint_is_printed_probe_by_probe() {
        let mut out = Vec::new();
        run(&parse(&["fingerprint", "chabrier"]).unwrap(), &mut out).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "chabrier: none (the task uses only `math`)\n"
        );
        let mut out = Vec::new();
        run(&parse(&["fingerprint", "mge"]).unwrap(), &mut out).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "galaxy::fields::disc::THIN_DISC_HOLE_LENGTHS = 0.55\n"
        );
    }
}
