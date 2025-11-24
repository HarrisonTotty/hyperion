//! Server module
//!
//! Handles the Rocket web server, GraphQL API, and client connections.

use crate::config::GameConfig;
use crate::state::GameWorld;
use crate::websocket::WebSocketManager;
use crate::event_broadcaster::EventBroadcaster;
use crate::api;
use crate::api::generation::UniverseState;
use log::info;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use rocket_cors::{AllowedOrigins, CorsOptions};

/// Launch the Rocket server with the given configuration
///
/// # Arguments
///
/// * `config` - The loaded game configuration
///
/// # Returns
///
/// Returns a `Result` indicating success or failure
pub async fn launch(config: GameConfig) -> Result<(), rocket::Error> {
    info!("Configuring Rocket server");

    // Initialize game world state
    let game_world = GameWorld::new_shared();
    info!("Game world initialized");
    
    // Initialize WebSocket manager
    let ws_manager = Arc::new(WebSocketManager::new());
    info!("WebSocket manager initialized");
    
    // Initialize procedural generation state
    let universe_state = Arc::new(RwLock::new(UniverseState::new()));
    info!("Procedural generation state initialized");
    
    // Start event broadcaster in background
    let broadcaster = EventBroadcaster::with_interval(
        game_world.clone(),
        ws_manager.clone(),
        Duration::from_millis(16), // ~60fps
    );
    
    tokio::spawn(async move {
        broadcaster.run().await;
    });
    info!("Event broadcaster started");

    // Configure CORS to allow all origins for development
    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::all())
        .allow_credentials(true)
        .to_cors()
        .expect("Failed to create CORS configuration");

    let rocket = rocket::build()
        .manage(config)
        .manage(game_world)
        .manage(ws_manager)
        .manage(universe_state)
        .attach(cors)
        .mount("/", api::routes())
        .mount("/graphql", api::graphql_routes());

    info!("Server ready to launch");
    rocket.launch().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_server_module() {
        // Placeholder test
        assert!(true);
    }
}
