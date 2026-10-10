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
//! hyperion-fit atmosphere-reference <case.json> --out <reference.json> [--samples N]
//!                                   [--threads N] [--stokes]
//!                                        # an atmosphere case's reference radiances (plan R08,
//!                                        # R08.T12.a), not a fit: outside `tables.lock`
//! ```
//!
//! `run` writes into the sim's `tables/` unless `--out` says otherwise, and then needs `--since`
//! equal to the sim's `GENERATOR_VERSION` (Design note 10); `--out` elsewhere needs none, which is
//! how plan 02's `run mge --out <path>` keeps working. `--smoke` runs the task's smoke manifest,
//! and `--manifest` another manifest, and both need `--out`. `--data` reads the task's one dataset from another directory, for a fetched
//! dataset kept outside the cache.

use std::io::Write;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::PathBuf;
use std::time::Instant;

use clap::{Args, Parser, Subcommand};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::tables::MANIFEST;

use crate::RunFitError;
use crate::atmosphere::{AtmosphereCase, Polarisation, trace_reference};
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
    /// Trace an atmosphere case's reference radiances (plan R08, R08.T12.a) and write them as
    /// JSON.
    ///
    /// Not a fit: its output is a client fixture, outside `tables.lock`.
    AtmosphereReference(AtmosphereReferenceArgs),
}

/// `atmosphere-reference`'s arguments.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AtmosphereReferenceArgs {
    /// The case, JSON in the format of `atmosphere::case`.
    pub case: PathBuf,
    /// Where to write the reference.
    #[arg(long)]
    pub out: PathBuf,
    /// Samples per geometry, aggregate and wavelength.
    #[arg(long, default_value_t = DEFAULT_REFERENCE_SAMPLES)]
    pub samples: NonZeroU64,
    /// Threads for the run; the output is the same for any number.
    #[arg(long)]
    pub threads: Option<NonZeroUsize>,
    /// Trace the Stokes vector (I, Q, U, V) and the scalar radiance of the same paths: every
    /// scattering term needs its scattering matrix, or the case's `depolarising` mark.
    #[arg(long)]
    pub stokes: bool,
}

/// `atmosphere-reference`'s samples per geometry, aggregate and wavelength unless `--samples`
/// says otherwise.
const DEFAULT_REFERENCE_SAMPLES: NonZeroU64 = NonZeroU64::new(100_000).expect("not zero");

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
/// [`RunFitError::AtmosphereCase`] if `atmosphere-reference`'s case cannot be read,
/// [`RunFitError::AtmosphereReference`] if it cannot be traced and
/// [`RunFitError::WriteReference`] if its reference cannot be written;
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
        Command::AtmosphereReference(args) => atmosphere_reference(args, out),
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

/// `atmosphere-reference`: traces the case and writes its reference, printing what it traced and
/// how long it took.
fn atmosphere_reference(
    args: &AtmosphereReferenceArgs,
    out: &mut dyn Write,
) -> Result<(), RunFitError> {
    let case = AtmosphereCase::read(&args.case)?;
    let polarisation = if args.stokes {
        Polarisation::Stokes
    } else {
        Polarisation::Scalar
    };
    let (samples, threads) = (args.samples, threads_or_all(args.threads));
    let start = Instant::now();
    let reference = trace_reference(&case, polarisation, samples, threads)?;
    let seconds = start.elapsed().as_secs_f64();
    std::fs::write(&args.out, reference.to_json()).map_err(|source| {
        RunFitError::WriteReference {
            path: args.out.clone(),
            source,
        }
    })?;
    writeln!(
        out,
        "wrote {} ({} geometries and {} aggregates at {} wavelengths, {samples} samples each, \
         {threads} threads, {seconds:.1} s)",
        args.out.display(),
        case.geometry_count(),
        case.aggregate_count(),
        case.wavelengths_nm().len(),
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
    fn atmosphere_reference_parses() {
        assert_eq!(
            parse(&[
                "atmosphere-reference",
                "earth.case.json",
                "--out",
                "earth.reference.json",
                "--samples",
                "500",
                "--threads",
                "2",
                "--stokes"
            ])
            .unwrap()
            .command,
            Command::AtmosphereReference(AtmosphereReferenceArgs {
                case: "earth.case.json".into(),
                out: "earth.reference.json".into(),
                samples: NonZeroU64::new(500).unwrap(),
                threads: NonZeroUsize::new(2),
                stokes: true,
            })
        );
        assert_eq!(
            parse(&["atmosphere-reference", "earth.case.json"])
                .unwrap_err()
                .kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
        assert_eq!(
            parse(&[
                "atmosphere-reference",
                "a.json",
                "--out",
                "b.json",
                "--samples",
                "0"
            ])
            .unwrap_err()
            .kind(),
            clap::error::ErrorKind::ValueValidation
        );
    }

    #[test]
    fn atmosphere_reference_writes_its_reference_and_names_its_failures() {
        let dir = tempfile::tempdir().unwrap();
        let case_path = dir.path().join("sample.case.json");
        let out_path = dir.path().join("sample.reference.json");
        let case = crate::atmosphere::case::tests::sample_case();
        std::fs::write(&case_path, case.to_string()).unwrap();
        let path = |p: &std::path::Path| p.to_str().unwrap().to_owned();
        let args = [
            "atmosphere-reference".to_owned(),
            path(&case_path),
            "--out".to_owned(),
            path(&out_path),
            "--samples".to_owned(),
            "64".to_owned(),
            "--threads".to_owned(),
            "2".to_owned(),
        ];
        let cli =
            Cli::try_parse_from(std::iter::once("hyperion-fit".to_owned()).chain(args)).unwrap();
        let mut out = Vec::new();
        run(&cli, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("2 geometries and 1 aggregates at 2 wavelengths, 64 samples"),
            "{text}"
        );
        let written = std::fs::read_to_string(&out_path).unwrap();
        let reference: crate::atmosphere::ReferenceRadiances =
            serde_json::from_str(&written).unwrap();
        assert_eq!(reference.case(), "sample");
        assert_eq!(reference.geometries().len(), 2);
        assert_eq!(reference.to_json(), written);

        // The Stokes mode of a case with an aerosol is a bad command line; a missing case is not.
        let stokes = Cli::try_parse_from([
            "hyperion-fit",
            "atmosphere-reference",
            case_path.to_str().unwrap(),
            "--out",
            out_path.to_str().unwrap(),
            "--stokes",
        ])
        .unwrap();
        let error = run(&stokes, &mut Vec::new()).unwrap_err();
        assert_eq!(error.exit_code(), 2);
        match error {
            RunFitError::AtmosphereReference(
                crate::atmosphere::TraceReferenceError::StokesNeedsMatrix { term },
            ) => assert_eq!(term, "aerosol"),
            other => panic!("{other:?}"),
        }
        let missing = parse(&["atmosphere-reference", "missing.json", "--out", "x.json"]).unwrap();
        let error = run(&missing, &mut Vec::new()).unwrap_err();
        assert_eq!(error.exit_code(), 1);
        assert!(
            matches!(
                error,
                RunFitError::AtmosphereCase(crate::atmosphere::ReadCaseError::Read { .. })
            ),
            "{error:?}"
        );
        // A reference that cannot be written names its path.
        let nowhere = dir.path().join("missing-directory").join("out.json");
        let unwritable = Cli::try_parse_from([
            "hyperion-fit",
            "atmosphere-reference",
            case_path.to_str().unwrap(),
            "--out",
            nowhere.to_str().unwrap(),
            "--samples",
            "2",
        ])
        .unwrap();
        let error = run(&unwritable, &mut Vec::new()).unwrap_err();
        assert_eq!(error.exit_code(), 1);
        match error {
            RunFitError::WriteReference { path, .. } => assert_eq!(path, nowhere),
            other => panic!("{other:?}"),
        }
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
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4, "{text}");
        assert!(
            lines[0].starts_with("primary_shares(0.5)[A] = 0."),
            "{text}"
        );
        let mut out = Vec::new();
        run(&parse(&["fingerprint", "mge"]).unwrap(), &mut out).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "galaxy::fields::disc::THIN_DISC_HOLE_LENGTHS = 0.55\n"
        );
    }
}
