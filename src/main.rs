//! HYPERION Spaceship Bridge Simulation Game
//!
//! This is the main entry point for the HYPERION server.
//! It provides a GraphQL/REST API and streaming service for game clients.

use clap::{Parser, Subcommand};
use hyperion::config::GameConfig;
use hyperion::server;
use log::{error, info, LevelFilter};
use std::path::PathBuf;

/// HYPERION - Spaceship Bridge Simulation Game Server
#[derive(Parser, Debug)]
#[command(name = "hyperion")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the HYPERION server
    Start {
        /// Path to the data directory containing game configuration files
        #[arg(short, long, default_value = "./data")]
        data_dir: PathBuf,

        /// Log level (error, warn, info, debug, trace)
        #[arg(short, long, default_value = "info")]
        log_level: String,
    },
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            data_dir,
            log_level,
        } => {
            // Initialize logging
            let level_filter = match log_level.to_lowercase().as_str() {
                "error" => LevelFilter::Error,
                "warn" => LevelFilter::Warn,
                "info" => LevelFilter::Info,
                "debug" => LevelFilter::Debug,
                "trace" => LevelFilter::Trace,
                _ => {
                    eprintln!("Invalid log level '{}', defaulting to 'info'", log_level);
                    LevelFilter::Info
                }
            };

            env_logger::Builder::new()
                .filter_level(level_filter)
                .init();

            info!("Starting HYPERION server");
            info!("Data directory: {}", data_dir.display());

            // Load game configuration from data directory
            let config = match GameConfig::load_from_directory(&data_dir) {
                Ok(cfg) => {
                    info!("Game configuration loaded successfully");
                    cfg
                }
                Err(e) => {
                    error!("Failed to load game configuration: {}", e);
                    return Err(e.into());
                }
            };

            // Start the server
            info!("Launching Rocket server...");
            server::launch(config).await?;

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::parse_from(&["hyperion", "start"]);
        assert!(matches!(cli.command, Commands::Start { .. }));
    }

    #[test]
    fn test_cli_with_options() {
        let cli = Cli::parse_from(&[
            "hyperion",
            "start",
            "--data-dir",
            "/custom/path",
            "--log-level",
            "debug",
        ]);

        if let Commands::Start {
            data_dir,
            log_level,
        } = cli.command
        {
            assert_eq!(data_dir, PathBuf::from("/custom/path"));
            assert_eq!(log_level, "debug");
        } else {
            panic!("Expected Start command");
        }
    }
}
