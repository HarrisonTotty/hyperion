//! `hyperion-fit`: runs an offline fit and writes its table. See the library's documentation.

use std::process::ExitCode;

use hyperion_fit::{Command, run};

fn main() -> ExitCode {
    match Command::parse(std::env::args().skip(1)).and_then(|command| run(&command)) {
        Ok(path) => {
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("hyperion-fit: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
