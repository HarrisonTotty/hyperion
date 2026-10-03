//! `gpu-replay`: the command line of the descent spike's native replayer (`just replay`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use gpu_replay::capture::Capture;
use gpu_replay::results::{Setting, write_results};
use gpu_replay::run::{replay_offscreen, replay_presented};
use gpu_replay::validate::validate_capture;

/// The descent spike's native replayer (plan R05, Design note 22).
#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// The setting a capture was taken at, when it does not record one.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum SettingArg {
    High,
    Low,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Reads a capture and validates every WGSL module in it with naga.
    Validate {
        /// The capture's directory (holding `capture.json` and `capture.bin`).
        capture: PathBuf,
    },
    /// Validates a capture, replays its frames and writes a results file.
    Replay {
        /// The capture's directory.
        capture: PathBuf,
        /// Present the frames in a window with FIFO presentation (a visible window: by hand only).
        /// Without it the frames are replayed offscreen and timed by the GPU's completions.
        #[arg(long)]
        present: bool,
        /// The setting the capture was taken at, when the capture does not record it.
        #[arg(long, value_enum)]
        setting: Option<SettingArg>,
        /// Where the results file goes.
        #[arg(long, default_value = "docs/measurements/descent-spike")]
        out: PathBuf,
    },
}

fn read(capture: &Path) -> anyhow::Result<Capture> {
    let read = Capture::read(capture)
        .with_context(|| format!("reading the capture in {}", capture.display()))?;
    println!(
        "{} calls, {} frames, {} surfaces, {} resources skipped by the snapshot",
        read.calls().len(),
        read.frames().len(),
        read.surfaces().len(),
        read.skipped().len()
    );
    print!("{}", validate_capture(&read).to_text());
    Ok(read)
}

fn main() -> anyhow::Result<ExitCode> {
    match Cli::parse().command {
        // A rejected module is a finding for the verdict, not a failure of the tool.
        Command::Validate { capture } => read(&capture).map(|_| ExitCode::SUCCESS),
        Command::Replay {
            capture,
            present,
            setting,
            out,
        } => {
            let read = read(&capture)?;
            let setting = setting.map(|setting| match setting {
                SettingArg::High => Setting::High,
                SettingArg::Low => Setting::Low,
            });
            let figures = if present {
                replay_presented(&read, &capture, setting)?
            } else {
                replay_offscreen(&read, &capture, setting)?
            };
            if !figures.errors.is_empty() {
                println!(
                    "{} validation errors during the replay:",
                    figures.errors.len()
                );
                for error in &figures.errors {
                    println!("  {error}");
                }
            }
            let path = write_results(&out, &figures)
                .with_context(|| format!("writing the results into {}", out.display()))?;
            println!("results: {}", path.display());
            Ok(ExitCode::SUCCESS)
        }
    }
}
