//! Hyperion Server Program

#[macro_use] extern crate log;

pub mod cli;
pub mod logging;

/// The entrypoint of the program.
fn main() {
    // Parse CLI arguments.
    let args = cli::get_arguments();

    // Set-up logging.
    match logging::setup(
        args.value_of("log_file").unwrap(),
        args.value_of("log_level").unwrap(),
        args.value_of("log_mode").unwrap()
    ) {
        Ok(_)  => debug!("Initialized logging subsystem."),
        Err(e) => panic!("Unable to initialize logging subsystem - {}", e)
    }

    // Load data files.
}
