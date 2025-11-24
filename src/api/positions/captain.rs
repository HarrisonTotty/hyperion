//! Captain position API endpoints
//!
//! Provides REST API endpoints for captain functions including:
//! - Crew reassignment
//! - Captain's log management

use rocket::{State, serde::json::Json, http::Status, post, get, routes};
use serde::{Deserialize, Serialize};
use crate::state::SharedGameWorld;
use crate::models::role::ShipRole;
use crate::models::ship::CaptainLogEntry;

// ==================== Request/Response Types ====================

/// Request to reassign crew members to positions
#[derive(Debug, Deserialize, Serialize)]
pub struct ReassignCrewRequest {
    /// Map of player IDs to their new role assignments
    pub assignments: std::collections::HashMap<String, Vec<ShipRole>>,
}

/// Response for crew reassignment
#[derive(Debug, Serialize, Deserialize)]
pub struct ReassignCrewResponse {
    pub message: String,
    pub updated_assignments: std::collections::HashMap<String, Vec<ShipRole>>,
}

/// Request to add a captain's log entry
#[derive(Debug, Deserialize, Serialize)]
pub struct AddLogEntryRequest {
    /// The log entry content
    pub entry: String,
    /// Optional stardate (if not provided, server generates one)
    pub stardate: Option<f64>,
}

/// Response for adding a log entry
#[derive(Debug, Serialize, Deserialize)]
pub struct AddLogEntryResponse {
    pub message: String,
    pub entry_id: String,
    pub stardate: f64,
}

/// Response for retrieving captain's log
#[derive(Debug, Serialize, Deserialize)]
pub struct GetLogResponse {
    pub ship_id: String,
    pub entries: Vec<CaptainLogEntry>,
    pub total: usize,
}

// ==================== API Endpoints ====================

/// Reassign crew members to positions
///
/// POST /v1/ships/<id>/reassign
#[post("/v1/ships/<ship_id>/reassign", data = "<request>")]
pub fn reassign_crew(
    ship_id: String,
    request: Json<ReassignCrewRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<ReassignCrewResponse>, Status> {
    let mut game_world = world.write().map_err(|_| Status::InternalServerError)?;
    
    // Get the ship and team_id
    let team_id = {
        let ship = game_world.ships().get(&ship_id)
            .ok_or(Status::NotFound)?;
        ship.team_id.clone()
    };
    
    // Validate all player IDs exist in the team
    let team = game_world.teams().get(&team_id)
        .ok_or(Status::InternalServerError)?;
    
    for player_id in request.assignments.keys() {
        if !team.members.contains(player_id) {
            return Err(Status::BadRequest);
        }
    }
    
    // Update player role assignments
    let ship = game_world.ships_mut().get_mut(&ship_id)
        .ok_or(Status::NotFound)?;
    ship.player_roles = request.assignments.clone();
    
    Ok(Json(ReassignCrewResponse {
        message: "Crew reassigned successfully".to_string(),
        updated_assignments: ship.player_roles.clone(),
    }))
}

/// Add a captain's log entry
///
/// POST /v1/ships/<id>/log
#[post("/v1/ships/<ship_id>/log", data = "<request>")]
pub fn add_log_entry(
    ship_id: String,
    request: Json<AddLogEntryRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<AddLogEntryResponse>, Status> {
    let mut game_world = world.write().map_err(|_| Status::InternalServerError)?;
    
    // Verify ship exists
    if !game_world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Generate stardate if not provided
    let stardate = request.stardate.unwrap_or_else(|| {
        // Simple stardate: Unix timestamp / 1000
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as f64 / 1000.0
    });
    
    let entry = CaptainLogEntry {
        id: uuid::Uuid::new_v4().to_string(),
        ship_id: ship_id.clone(),
        stardate,
        entry: request.entry.clone(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    };
    
    // Store the log entry
    game_world.add_captain_log_entry(entry.clone());
    
    Ok(Json(AddLogEntryResponse {
        message: "Log entry added successfully".to_string(),
        entry_id: entry.id,
        stardate: entry.stardate,
    }))
}

/// Get captain's log
///
/// GET /v1/ships/<id>/log
#[get("/v1/ships/<ship_id>/log")]
pub fn get_log(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<GetLogResponse>, Status> {
    let game_world = world.read().map_err(|_| Status::InternalServerError)?;
    
    // Verify ship exists
    if !game_world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Get log entries for this ship
    let entries = game_world.get_captain_log_entries(&ship_id);
    let total = entries.len();
    
    Ok(Json(GetLogResponse {
        ship_id,
        entries,
        total,
    }))
}

/// Returns all captain position routes
pub fn routes() -> Vec<rocket::Route> {
    routes![reassign_crew, add_log_entry, get_log]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameWorld;
    use crate::models::Ship;
    use crate::models::status::{ShipStatus, Inventory};
    use rocket::local::blocking::Client;
    use rocket::http::{Status, ContentType};

    fn setup_test_world() -> SharedGameWorld {
        let world = GameWorld::new_shared();
        let mut game_world = world.write().unwrap();
        
        // Create a player
        let player_id = game_world.register_player("Captain_Kirk".to_string()).unwrap();
        
        // Create a team
        let team_id = game_world.create_team("Starfleet".to_string(), "Federation".to_string()).unwrap();
        
        // Add player to team
        game_world.add_player_to_team(&team_id, &player_id).unwrap();
        
        // Create a ship
        let ship = Ship {
            id: "ship1".to_string(),
            name: "USS Enterprise".to_string(),
            class: "Constitution".to_string(),
            team_id: team_id.clone(),
            player_roles: std::collections::HashMap::new(),
            status: ShipStatus::default(),
            modules: vec![],
            weapons: vec![],
            inventory: Inventory::default(),
        };
        game_world.add_ship(ship);
        
        drop(game_world);
        world
    }

    fn build_test_client(world: SharedGameWorld) -> Client {
        let rocket = rocket::build()
            .manage(world)
            // Mount at root because test request paths already include /v1
            .mount("/", routes());
        Client::tracked(rocket).expect("valid rocket instance")
    }

    #[test]
    fn test_reassign_crew() {
        let world = setup_test_world();
        let client = build_test_client(world.clone());
        
        // Get the player ID
        let player_id = {
            let game_world = world.read().unwrap();
            game_world.players().values().next().unwrap().id.clone()
        };
        
        let mut assignments = std::collections::HashMap::new();
        assignments.insert(player_id.clone(), vec![ShipRole::Captain, ShipRole::Helm]);
        
        let request = ReassignCrewRequest { assignments };
        
        let response = client.post("/v1/ships/ship1/reassign")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: ReassignCrewResponse = response.into_json().unwrap();
        assert_eq!(body.message, "Crew reassigned successfully");
        assert_eq!(body.updated_assignments.get(&player_id).unwrap().len(), 2);
    }

    #[test]
    fn test_reassign_crew_invalid_ship() {
        let world = setup_test_world();
        let client = build_test_client(world);
        
        let assignments = std::collections::HashMap::new();
        let request = ReassignCrewRequest { assignments };
        
        let response = client.post("/v1/ships/nonexistent/reassign")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn test_reassign_crew_invalid_player() {
        let world = setup_test_world();
        let client = build_test_client(world);
        
        let mut assignments = std::collections::HashMap::new();
        assignments.insert("invalid_player".to_string(), vec![ShipRole::Captain]);
        
        let request = ReassignCrewRequest { assignments };
        
        let response = client.post("/v1/ships/ship1/reassign")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
    }

    #[test]
    fn test_add_log_entry() {
        let world = setup_test_world();
        let client = build_test_client(world);
        
        let request = AddLogEntryRequest {
            entry: "Captain's log, stardate 41153.7. Our destination is planet Deneb IV.".to_string(),
            stardate: Some(41153.7),
        };
        
        let response = client.post("/v1/ships/ship1/log")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: AddLogEntryResponse = response.into_json().unwrap();
        assert_eq!(body.message, "Log entry added successfully");
        assert_eq!(body.stardate, 41153.7);
        assert!(!body.entry_id.is_empty());
    }

    #[test]
    fn test_add_log_entry_auto_stardate() {
        let world = setup_test_world();
        let client = build_test_client(world);
        
        let request = AddLogEntryRequest {
            entry: "Routine patrol sector".to_string(),
            stardate: None,
        };
        
        let response = client.post("/v1/ships/ship1/log")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: AddLogEntryResponse = response.into_json().unwrap();
        assert!(body.stardate > 0.0);
    }

    #[test]
    fn test_add_log_entry_invalid_ship() {
        let world = setup_test_world();
        let client = build_test_client(world);
        
        let request = AddLogEntryRequest {
            entry: "Test entry".to_string(),
            stardate: None,
        };
        
        let response = client.post("/v1/ships/nonexistent/log")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn test_get_log() {
        let world = setup_test_world();
        let client = build_test_client(world.clone());
        
        // Add a log entry first
        let request = AddLogEntryRequest {
            entry: "First log entry".to_string(),
            stardate: Some(41000.0),
        };
        
        client.post("/v1/ships/ship1/log")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        // Retrieve the log
        let response = client.get("/v1/ships/ship1/log").dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: GetLogResponse = response.into_json().unwrap();
        assert_eq!(body.ship_id, "ship1");
        assert_eq!(body.total, 1);
        assert_eq!(body.entries[0].entry, "First log entry");
        assert_eq!(body.entries[0].stardate, 41000.0);
    }

    #[test]
    fn test_get_log_invalid_ship() {
        let world = setup_test_world();
        let client = build_test_client(world);
        
        let response = client.get("/v1/ships/nonexistent/log").dispatch();
        
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn test_get_log_empty() {
        let world = setup_test_world();
        let client = build_test_client(world);
        
        let response = client.get("/v1/ships/ship1/log").dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: GetLogResponse = response.into_json().unwrap();
        assert_eq!(body.total, 0);
        assert!(body.entries.is_empty());
    }
}
