//! Offline fitting for HYPERION: the tool that writes the generator's fitted constant tables.
//!
//! Some of the generator's numbers are the result of a fit that is too slow, or too fiddly, to run
//! each time a galaxy is built: the dimensionless coefficients of the Gaussian expansions behind
//! the potential, the cooling tables of planets and white dwarfs, the kick law's rank table, and
//! later the samplers and constants of plan 15. This crate runs those fits and emits each result as
//! Rust source, committed under the `src/tables/` of the crate that reads it
//! ([`TableCrate`](task::TableCrate); `crates/hyperion-sim/src/tables/` for every table but the
//! surface crate's, until the crate split moves the sky's) with a header naming the tool, its
//! inputs and its revision (plan 15).
//!
//! Nothing depends on this crate: it depends on `hyperion-sim` and `hyperion-surface`, never the
//! reverse. Every
//! transcendental it evaluates goes through [`hyperion_sim::math`], every quadrature through
//! [`hyperion_sim::galaxy::quad`], and a fit's output is a pure function of its manifest, whatever
//! the thread count ([`parallel`]), so a table comes out bit for bit the same on every platform.
//!
//! The pieces:
//!
//! - [`atmosphere`]: the reference path tracer for the client's atmospheres (plan R08), a new kind
//!   of output for the crate, JSON fixtures for the client rather than tables for the sim, and not a
//!   [`FitTask`](task::FitTask);
//! - [`task`]: the [`FitTask`](task::FitTask) interface and the [`registry`](task::registry) of
//!   every fit, [`tasks`] their implementations;
//! - [`manifest`]: manifests, the inputs hash, sim fingerprints and the lock file `tables.lock`;
//! - [`emit`]: the Rust source emitter, the header grammar and the guards on writing a table;
//! - [`check`]: the staleness check, `just fit-check`;
//! - [`data`]: external datasets and their provenance;
//! - [`materials`]: rendering plan R08's committed material files, read as a fit's input;
//! - [`parallel`]: the deterministic map-reduce;
//! - [`optimise`]: hand-written optimisers (golden section, Nelder–Mead);
//! - [`cli`]: the command line, which `main.rs` parses and hands to [`cli::run`].

pub mod atmosphere;
pub mod check;
pub mod cli;
pub mod data;
pub mod emit;
pub mod manifest;
pub mod materials;
pub mod optimise;
pub mod parallel;
pub mod pipeline;
pub mod task;
pub mod tasks;

use std::io;

/// A command of `hyperion-fit` failed.
#[derive(Debug, thiserror::Error)]
pub enum RunFitError {
    /// The command line named a task that is not registered.
    #[error("unknown task `{0}`: `hyperion-fit list` lists the tasks")]
    UnknownTask(String),
    /// A task's manifest could not be loaded.
    #[error(transparent)]
    Manifest(manifest::LoadManifestError),
    /// The task could not run.
    #[error(transparent)]
    Task(task::RunTaskError),
    /// The table could not be emitted.
    #[error(transparent)]
    Emit(emit::EmitTableError),
    /// The staleness check found stale tables; the report lists them.
    #[error("{} stale tables", .0.findings.len())]
    Check(check::CheckReport),
    /// `--smoke` or `--manifest` was given without `--out`: a committed table is always made from
    /// the task's own manifest.
    #[error(
        "`--smoke` and `--manifest` need `--out`: a committed table is made from the task's own \
         manifest"
    )]
    OtherManifestNeedsOut,
    /// `--data` was given for a task that does not read exactly one dataset.
    #[error("`--data` needs a task that reads exactly one dataset, and `{0}` does not")]
    DataNeedsOneDataset(String),
    /// An atmosphere case could not be read.
    #[error(transparent)]
    AtmosphereCase(#[from] atmosphere::ReadCaseError),
    /// An atmosphere reference could not be traced.
    #[error(transparent)]
    AtmosphereReference(#[from] atmosphere::TraceReferenceError),
    /// An atmosphere reference could not be written.
    #[error("cannot write the atmosphere reference {}", path.display())]
    WriteReference {
        /// Where it was to go.
        path: std::path::PathBuf,
        /// Why it could not.
        #[source]
        source: io::Error,
    },
    /// An atmosphere sweep case could not be read.
    #[error(transparent)]
    SweepCase(#[from] atmosphere::sweep::ReadSweepError),
    /// An atmosphere sweep could not be run.
    #[error(transparent)]
    Sweep(#[from] atmosphere::sweep::RunSweepError),
    /// `atmosphere-sweep` was given too little to run without `--smoke`.
    #[error("`atmosphere-sweep` needs {0} unless it runs `--smoke`")]
    SweepNeeds(&'static str),
    /// The running binary could not be read for the SHA-256 that keys a sweep's work directory.
    #[error("cannot read the running binary {} to hash it", path.display())]
    HashBinary {
        /// The binary, empty where its path could not be found.
        path: std::path::PathBuf,
        /// Why it could not.
        #[source]
        source: io::Error,
    },
    /// An atmosphere sweep's output could not be written.
    #[error("cannot write the atmosphere sweep {}", path.display())]
    WriteSweep {
        /// Where it was to go.
        path: std::path::PathBuf,
        /// Why it could not.
        #[source]
        source: io::Error,
    },
    /// The command's output could not be written.
    #[error("cannot write the output")]
    Output(#[from] io::Error),
}

impl RunFitError {
    /// The process exit code for this error: 2 for a bad command line, 1 for everything else.
    ///
    /// For `atmosphere-reference`, `--stokes` on a case the Stokes mode cannot trace and a
    /// `--samples` past [`atmosphere::MAX_SAMPLES`] are bad command lines; for
    /// `atmosphere-sweep`, a missing argument and a point the case does not hold.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        use atmosphere::sweep::RunSweepError;
        match self {
            Self::UnknownTask(_)
            | Self::OtherManifestNeedsOut
            | Self::DataNeedsOneDataset(_)
            | Self::AtmosphereReference(
                atmosphere::TraceReferenceError::StokesNeedsMatrix { .. }
                | atmosphere::TraceReferenceError::TooManySamples { .. },
            )
            | Self::SweepNeeds(_)
            | Self::Sweep(RunSweepError::UnknownPoint(_)) => 2,
            Self::Manifest(_)
            | Self::Task(_)
            | Self::Emit(_)
            | Self::Check(_)
            | Self::AtmosphereCase(_)
            | Self::AtmosphereReference(atmosphere::TraceReferenceError::ThreadPool(_))
            | Self::WriteReference { .. }
            | Self::SweepCase(_)
            | Self::Sweep(
                RunSweepError::Point { .. }
                | RunSweepError::Trace(_)
                | RunSweepError::Work { .. }
                | RunSweepError::Stale { .. }
                | RunSweepError::CorruptPoint { .. }
                | RunSweepError::Progress(_),
            )
            | Self::HashBinary { .. }
            | Self::WriteSweep { .. }
            | Self::Output(_) => 1,
        }
    }
}
