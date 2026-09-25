//! `hyperion-fit`: runs an offline fit and writes its table, or checks the tables. See the
//! library's documentation.

use std::process::ExitCode;

use clap::Parser;
use hyperion_fit::cli::{Cli, run};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli, &mut std::io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hyperion-fit: {error}");
            let mut source = std::error::Error::source(&error);
            while let Some(cause) = source {
                eprintln!("  caused by: {cause}");
                source = cause.source();
            }
            ExitCode::from(error.exit_code())
        }
    }
}
