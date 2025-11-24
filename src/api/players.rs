//! Player API endpoints
//!
//! REST API endpoints for player management operations.

use rocket::{routes, Route, State, get, post, delete};
use rocket::serde::json::Json;
use rocket::http::Status;
use serde::{Deserialize, Serialize};

use crate::state::SharedGameWorld;

/// Request body for creating a new player
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePlayerRequest {
    /// Player's display name
    pub name: String,
}

/// Response for player creation
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePlayerResponse {
    /// Newly created player ID
    pub id: String,
    /// Player name
    pub name: String,
}

/// Response for player details
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerResponse {
    /// Player ID
    pub id: String,
    /// Player name
    pub name: String,
}

/// Response for listing players
#[derive(Debug, Serialize, Deserialize)]
pub struct ListPlayersResponse {
    /// List of all players
    pub players: Vec<PlayerResponse>,
    /// Total count
    pub count: usize,
}

/// Error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
}

/// GET /v1/players - List all registered players
///
/// Returns a list of all players currently registered in the game.
#[get("/v1/players")]
pub fn list_players(world: &State<SharedGameWorld>) -> Json<ListPlayersResponse> {
    let world = world.read().unwrap();
    let players = world.get_all_players();
    
    let player_responses: Vec<PlayerResponse> = players
        .into_iter()
        .map(|p| PlayerResponse {
            id: p.id.clone(),
            name: p.name.clone(),
        })
        .collect();
    
    let count = player_responses.len();
    
    Json(ListPlayersResponse {
        players: player_responses,
        count,
    })
}

/// POST /v1/players - Register a new player
///
/// Creates a new player with the given name. The name must be unique,
/// 1-50 characters long, and contain only alphanumeric characters,
/// underscores, and hyphens.
#[post("/v1/players", data = "<request>")]
pub fn create_player(
    world: &State<SharedGameWorld>,
    request: Json<CreatePlayerRequest>,
) -> Result<Json<CreatePlayerResponse>, (Status, Json<ErrorResponse>)> {
    let mut world = world.write().unwrap();
    
    match world.register_player(request.name.clone()) {
        Ok(player_id) => {
            let player = world.get_player(&player_id).unwrap();
            Ok(Json(CreatePlayerResponse {
                id: player.id.clone(),
                name: player.name.clone(),
            }))
        }
        Err(err) => {
            Err((
                Status::BadRequest,
                Json(ErrorResponse { error: err }),
            ))
        }
    }
}

/// GET /v1/players/<id> - Get player details
///
/// Returns details for a specific player by ID.
#[get("/v1/players/<id>")]
pub fn get_player(
    world: &State<SharedGameWorld>,
    id: String,
) -> Result<Json<PlayerResponse>, (Status, Json<ErrorResponse>)> {
    let world = world.read().unwrap();
    
    match world.get_player(&id) {
        Some(player) => Ok(Json(PlayerResponse {
            id: player.id.clone(),
            name: player.name.clone(),
        })),
        None => Err((
            Status::NotFound,
            Json(ErrorResponse {
                error: format!("Player {} not found", id),
            }),
        )),
    }
}

/// DELETE /v1/players/<id> - Remove player (disconnect)
///
/// Removes a player from the game. This also removes the player from
/// any teams they are part of.
#[delete("/v1/players/<id>")]
pub fn delete_player(
    world: &State<SharedGameWorld>,
    id: String,
) -> Result<Status, (Status, Json<ErrorResponse>)> {
    let mut world = world.write().unwrap();
    
    match world.remove_player(&id) {
        Ok(_) => Ok(Status::NoContent),
        Err(err) => Err((
            Status::NotFound,
            Json(ErrorResponse { error: err }),
        )),
    }
}

/// Returns all player API routes
pub fn routes() -> Vec<Route> {
    routes![list_players, create_player, get_player, delete_player]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::blocking::Client;
    use rocket::Build;
    use crate::state::GameWorld;

    fn create_test_rocket() -> rocket::Rocket<Build> {
        let world = GameWorld::new_shared();
        rocket::build()
            .manage(world)
            .mount("/", routes())
    }

    #[test]
    fn test_list_players_empty() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        let response = client.get("/v1/players").dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: ListPlayersResponse = response.into_json().unwrap();
        assert_eq!(body.count, 0);
        assert_eq!(body.players.len(), 0);
    }

    #[test]
    fn test_create_player() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        let response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: CreatePlayerResponse = response.into_json().unwrap();
        assert_eq!(body.name, "Alice");
        assert!(!body.id.is_empty());
    }

    #[test]
    fn test_create_player_duplicate_name() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create first player
        client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        
        // Try to create duplicate
        let response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
        
        let body: ErrorResponse = response.into_json().unwrap();
        assert!(body.error.contains("already taken"));
    }

    #[test]
    fn test_create_player_invalid_name() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Empty name
        let response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "".to_string(),
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
        
        // Invalid characters
        let response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice@123".to_string(),
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
    }

    #[test]
    fn test_get_player() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create player
        let create_response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        
        let create_body: CreatePlayerResponse = create_response.into_json().unwrap();
        let player_id = create_body.id;
        
        // Get player
        let response = client
            .get(format!("/v1/players/{}", player_id))
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: PlayerResponse = response.into_json().unwrap();
        assert_eq!(body.id, player_id);
        assert_eq!(body.name, "Alice");
    }

    #[test]
    fn test_get_player_not_found() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        let response = client
            .get("/v1/players/nonexistent-id")
            .dispatch();
        
        assert_eq!(response.status(), Status::NotFound);
        
        let body: ErrorResponse = response.into_json().unwrap();
        assert!(body.error.contains("not found"));
    }

    #[test]
    fn test_delete_player() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create player
        let create_response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        
        let create_body: CreatePlayerResponse = create_response.into_json().unwrap();
        let player_id = create_body.id;
        
        // Delete player
        let response = client
            .delete(format!("/v1/players/{}", player_id))
            .dispatch();
        
        assert_eq!(response.status(), Status::NoContent);
        
        // Verify player is gone
        let response = client
            .get(format!("/v1/players/{}", player_id))
            .dispatch();
        
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn test_delete_player_not_found() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        let response = client
            .delete("/v1/players/nonexistent-id")
            .dispatch();
        
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn test_list_players_with_data() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create multiple players
        client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        
        client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Bob".to_string(),
            })
            .dispatch();
        
        // List players
        let response = client.get("/v1/players").dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: ListPlayersResponse = response.into_json().unwrap();
        assert_eq!(body.count, 2);
        assert_eq!(body.players.len(), 2);
    }
}
