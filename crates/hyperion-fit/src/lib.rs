//! Offline fitting for HYPERION: the tool that writes the generator's fitted constant tables.
//!
//! Some of the generator's numbers are the result of a fit that is too slow, or too fiddly, to run
//! each time a galaxy is built: the dimensionless coefficients of the Gaussian expansions behind
//! the potential, the cooling tables of planets and white dwarfs, the kick law's rank table, and
//! later the samplers and constants of plan 15. This crate runs those fits and emits each result as
//! Rust source, committed under `crates/hyperion-sim/src/tables/` with a header naming the tool,
//! its inputs and its revision (plan 15).
//!
//! Nothing depends on this crate: it depends on `hyperion-sim`, never the reverse. Every
//! transcendental it evaluates goes through [`hyperion_sim::math`], every quadrature through
//! [`hyperion_sim::galaxy::quad`], and a fit's output is a pure function of its manifest, whatever
//! the thread count ([`parallel`]), so a table comes out bit for bit the same on every platform.
//!
//! The pieces:
//!
//! - [`task`]: the [`FitTask`](task::FitTask) interface and the [`registry`](task::registry) of
//!   every fit, [`tasks`] their implementations;
//! - [`manifest`]: manifests, the inputs hash, sim fingerprints and the lock file `tables.lock`;
//! - [`emit`]: the Rust source emitter, the header grammar and the guards on writing a table;
//! - [`check`]: the staleness check, `just fit-check`;
//! - [`data`]: external datasets and their provenance;
//! - [`parallel`]: the deterministic map-reduce;
//! - [`cli`]: the command line, which `main.rs` parses and hands to [`cli::run`].

pub mod check;
pub mod cli;
pub mod data;
pub mod emit;
pub mod manifest;
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
    /// The command's output could not be written.
    #[error("cannot write the output")]
    Output(#[from] io::Error),
}

impl RunFitError {
    /// The process exit code for this error: 2 for a bad command line, 1 for everything else.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::UnknownTask(_) | Self::OtherManifestNeedsOut | Self::DataNeedsOneDataset(_) => 2,
            Self::Manifest(_)
            | Self::Task(_)
            | Self::Emit(_)
            | Self::Check(_)
            | Self::Output(_) => 1,
        }
    }
}
