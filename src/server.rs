//! Contains methods and definitions pertaining to the Hyperion server.

use crate::server_routes;

/// Configures the server, returning a Rocket configuration file.
pub fn configure(address: &str, log_level: &str, port: u16) -> rocket::config::Config {
    use rocket::config::*;
    std::env::set_var("ROCKET_CLI_COLORS", "off");
    Config::build(Environment::Production)
        .address(address)
        .port(port)
        .log_level(match log_level {
            "disabled" => LoggingLevel::Off,
            "error"    => LoggingLevel::Critical,
            "warning"  => LoggingLevel::Critical,
            "info"     => LoggingLevel::Normal,
            _          => LoggingLevel::Debug
        })
        .finalize()
        .unwrap()
}


/// Starts a new server process.
pub fn start(server_config: rocket::config::Config) {
    rocket::custom(server_config)
        .mount("/", routes![
            server_routes::root,
            server_routes::v1
        ])
        .launch();
}
