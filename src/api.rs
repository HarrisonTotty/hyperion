//! API module
//!
//! Defines REST and GraphQL API endpoints for game clients.

pub mod players;
pub mod teams;
pub mod factions;
pub mod blueprints;
pub mod ships;
pub mod positions;
pub mod stations;
pub mod ai;
pub mod generation;
pub mod ship_classes;
pub mod modules;
pub mod catalog;

use rocket::{routes, Route, State, get, options};
use rocket::serde::json::Json;
use serde_json;
use crate::config::GameConfig;
use crate::websocket;

/// Catch-all OPTIONS handler for CORS preflight requests
#[options("/<_..>")]
fn options_handler() -> &'static str {
    ""
}

/// Health check endpoint
///
/// Returns a simple status message to verify the server is running.
#[get("/health")]
fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "message": "HYPERION server is running"
    }))
}

/// Get server info
///
/// Returns information about the server and loaded configuration.
#[get("/info")]
fn server_info(config: &State<GameConfig>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "server": "HYPERION",
        "version": env!("CARGO_PKG_VERSION"),
        "factions_count": config.factions.factions.len(),
        "races_count": config.races.races.len(),
        "ship_classes_count": config.ship_classes.len(),
    }))
}

/// Returns all REST API routes
pub fn routes() -> Vec<Route> {
    let mut api_routes = routes![health_check, server_info, options_handler];
    api_routes.extend(players::routes());
    api_routes.extend(teams::routes());
    api_routes.extend(factions::routes());
    api_routes.extend(blueprints::routes());
    api_routes.extend(ships::routes());
    api_routes.extend(ship_classes::routes());
    api_routes.extend(modules::routes());
    api_routes.extend(catalog::routes());
    api_routes.extend(stations::routes());
    api_routes.extend(ai::routes());
    api_routes.extend(generation::routes());
    api_routes.extend(positions::captain::routes());
    api_routes.extend(positions::comms::routes());
    api_routes.extend(positions::countermeasures::routes());
    api_routes.extend(positions::energy_weapons::routes());
    api_routes.extend(positions::engineering::routes());
    api_routes.extend(positions::helm::routes());
    api_routes.extend(positions::kinetic_weapons::routes());
    api_routes.extend(positions::missile_weapons::routes());
    api_routes.extend(positions::science::routes());
    api_routes.extend(websocket_routes());
    api_routes
}

/// Returns WebSocket routes
pub fn websocket_routes() -> Vec<Route> {
    routes![websocket::ws_handler, websocket::ws_info]
}

/// Returns all GraphQL API routes (placeholder)
pub fn graphql_routes() -> Vec<Route> {
    // GraphQL routes will be implemented here
    routes![]
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_api_module() {
        // Placeholder test
        assert!(true);
    }
}
