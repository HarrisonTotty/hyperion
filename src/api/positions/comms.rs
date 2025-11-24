//! Communications Officer API endpoints
//!
//! Handles docking requests, hailing, and fighter commands.

use rocket::{Route, State, serde::json::Json, http::Status};
use rocket::{routes, post};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::state::SharedGameWorld;
use crate::models::ship::{DockingRequest, DockingStatus, HailMessage, FighterCommand, FighterCommandType};

/// Request to dock with a station
#[derive(Debug, Deserialize)]
pub struct DockRequestRequest {
    pub station_id: String,
}

/// Response for dock request
#[derive(Debug, Serialize, Deserialize)]
pub struct DockRequestResponse {
    pub request_id: String,
    pub status: DockingStatus,
}

/// Response for undock request
#[derive(Debug, Serialize, Deserialize)]
pub struct UndockResponse {
    pub success: bool,
}

/// Request to hail another ship
#[derive(Debug, Deserialize)]
pub struct HailRequest {
    pub target_ship_id: String,
    pub message: String,
}

/// Response for hail
#[derive(Debug, Serialize, Deserialize)]
pub struct HailResponse {
    pub message_id: String,
}

/// Request to respond to a hail
#[derive(Debug, Deserialize)]
pub struct RespondRequest {
    pub message_id: String,
    pub response: String,
}

/// Response for respond
#[derive(Debug, Serialize, Deserialize)]
pub struct RespondResponse {
    pub response_id: String,
}

/// Request to jam communications
#[derive(Debug, Deserialize)]
pub struct JamRequest {
    pub target_ship_id: String,
    pub duration: f64,
}

/// Response for jam
#[derive(Debug, Serialize, Deserialize)]
pub struct JamResponse {
    pub success: bool,
}

/// Request to command fighters
#[derive(Debug, Deserialize)]
pub struct FighterCommandRequest {
    pub fighter_ids: Vec<String>,
    pub command: FighterCommandTypeDto,
}

/// Fighter command type for API
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FighterCommandTypeDto {
    Launch,
    Recall,
    Attack { target_id: String },
    Defend { protect_id: String },
    Patrol { waypoints: Vec<(f64, f64, f64)> },
}

/// Response for fighter command
#[derive(Debug, Serialize, Deserialize)]
pub struct FighterCommandResponse {
    pub command_id: String,
}

/// Request docking with a station
#[post("/v1/ships/<ship_id>/dock-request", data = "<request>")]
pub fn dock_request(
    ship_id: String,
    request: Json<DockRequestRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<DockRequestResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Check if communications are jammed
    let ship = world.ships().get(&ship_id).unwrap();
    if ship.status.is_ion_jammed() {
        return Err(Status::BadRequest);
    }
    
    // Create docking request
    let request_id = Uuid::new_v4().to_string();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let docking_request = DockingRequest {
        id: request_id.clone(),
        ship_id: ship_id.clone(),
        station_id: request.station_id.clone(),
        timestamp,
        status: DockingStatus::Pending,
    };
    
    world.add_docking_request(docking_request.clone());
    
    Ok(Json(DockRequestResponse {
        request_id,
        status: DockingStatus::Pending,
    }))
}

/// Request undocking from a station
#[post("/v1/ships/<ship_id>/undock")]
pub fn undock(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<UndockResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Remove any docking requests for this ship
    world.remove_docking_requests(&ship_id);
    
    Ok(Json(UndockResponse {
        success: true,
    }))
}

/// Hail another vessel
#[post("/v1/ships/<ship_id>/hail", data = "<request>")]
pub fn hail(
    ship_id: String,
    request: Json<HailRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<HailResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Check if target ship exists
    if !world.ships().contains_key(&request.target_ship_id) {
        return Err(Status::BadRequest);
    }
    
    // Check if communications are jammed
    let ship = world.ships().get(&ship_id).unwrap();
    if ship.status.is_ion_jammed() {
        return Err(Status::BadRequest);
    }
    
    // Create hail message
    let message_id = Uuid::new_v4().to_string();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let hail_message = HailMessage {
        id: message_id.clone(),
        from_ship_id: ship_id.clone(),
        to_ship_id: request.target_ship_id.clone(),
        message: request.message.clone(),
        timestamp,
        in_response_to: None,
    };
    
    world.add_hail_message(hail_message);
    
    Ok(Json(HailResponse { message_id }))
}

/// Respond to an incoming hail
#[post("/v1/ships/<ship_id>/respond", data = "<request>")]
pub fn respond(
    ship_id: String,
    request: Json<RespondRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<RespondResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Check if original message exists
    let original_message = world.get_hail_message(&request.message_id);
    if original_message.is_none() {
        return Err(Status::BadRequest);
    }
    
    let original_message = original_message.unwrap();
    
    // Verify this ship is the recipient
    if original_message.to_ship_id != ship_id {
        return Err(Status::BadRequest);
    }
    
    // Check if communications are jammed
    let ship = world.ships().get(&ship_id).unwrap();
    if ship.status.is_ion_jammed() {
        return Err(Status::BadRequest);
    }
    
    // Create response message
    let response_id = Uuid::new_v4().to_string();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let response_message = HailMessage {
        id: response_id.clone(),
        from_ship_id: ship_id.clone(),
        to_ship_id: original_message.from_ship_id.clone(),
        message: request.response.clone(),
        timestamp,
        in_response_to: Some(request.message_id.clone()),
    };
    
    world.add_hail_message(response_message);
    
    Ok(Json(RespondResponse { response_id }))
}

/// Jam enemy communications
#[post("/v1/ships/<ship_id>/jam", data = "<request>")]
pub fn jam(
    ship_id: String,
    request: Json<JamRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<JamResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Check if target ship exists
    if !world.ships().contains_key(&request.target_ship_id) {
        return Err(Status::BadRequest);
    }
    
    // Apply jam (this would integrate with the Ion weapon effect system)
    // For now, we just record the jam attempt
    world.add_jam_attempt(&ship_id, &request.target_ship_id, request.duration);
    
    Ok(Json(JamResponse { success: true }))
}

/// Command fighters
#[post("/v1/ships/<ship_id>/fighters/command", data = "<request>")]
pub fn command_fighters(
    ship_id: String,
    request: Json<FighterCommandRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<FighterCommandResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Check if communications are jammed
    let ship = world.ships().get(&ship_id).unwrap();
    if ship.status.is_ion_jammed() {
        return Err(Status::BadRequest);
    }
    
    // Convert DTO to model
    let command_type = match &request.command {
        FighterCommandTypeDto::Launch => FighterCommandType::Launch,
        FighterCommandTypeDto::Recall => FighterCommandType::Recall,
        FighterCommandTypeDto::Attack { target_id } => {
            FighterCommandType::Attack { target_id: target_id.clone() }
        }
        FighterCommandTypeDto::Defend { protect_id } => {
            FighterCommandType::Defend { protect_id: protect_id.clone() }
        }
        FighterCommandTypeDto::Patrol { waypoints } => {
            FighterCommandType::Patrol { waypoints: waypoints.clone() }
        }
    };
    
    // Create fighter command
    let command_id = Uuid::new_v4().to_string();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let fighter_command = FighterCommand {
        id: command_id.clone(),
        ship_id: ship_id.clone(),
        fighter_ids: request.fighter_ids.clone(),
        command: command_type,
        timestamp,
    };
    
    world.add_fighter_command(fighter_command);
    
    Ok(Json(FighterCommandResponse { command_id }))
}

/// Returns all routes for the communications officer position
pub fn routes() -> Vec<Route> {
    routes![dock_request, undock, hail, respond, jam, command_fighters]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameWorld;
    use crate::models::ship::Ship;
    use crate::models::status::{ShipStatus, StatusEffect, StatusEffectType};
    use std::sync::{Arc, RwLock};
    
    fn setup_test_world() -> SharedGameWorld {
        Arc::new(RwLock::new(GameWorld::new()))
    }
    
    fn create_test_ship(id: &str, team_id: &str) -> Ship {
        Ship {
            id: id.to_string(),
            name: format!("Ship {}", id),
            class: "corvette".to_string(),
            team_id: team_id.to_string(),
            player_roles: std::collections::HashMap::new(),
            status: ShipStatus::default(),
            modules: vec![],
            weapons: vec![],
            inventory: Default::default(),
        }
    }
    
    #[test]
    fn test_dock_request() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(DockRequestRequest {
            station_id: "station1".to_string(),
        });
        
        let result = dock_request("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert_eq!(response.status, DockingStatus::Pending);
    }
    
    #[test]
    fn test_dock_request_ship_not_found() {
        let world = setup_test_world();
        
        let request = Json(DockRequestRequest {
            station_id: "station1".to_string(),
        });
        
        let result = dock_request("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::NotFound));
    }
    
    #[test]
    fn test_dock_request_jammed() {
        let world = setup_test_world();
        let mut ship = create_test_ship("ship1", "team1");
        ship.status.status_effects.push(StatusEffect {
            effect_type: StatusEffectType::Ion,
            duration: 5.0,
            magnitude: 1.0,
        });
        world.write().unwrap().add_ship(ship);
        
        let request = Json(DockRequestRequest {
            station_id: "station1".to_string(),
        });
        
        let result = dock_request("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::BadRequest));
    }
    
    #[test]
    fn test_undock() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = undock("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
    }
    
    #[test]
    fn test_hail() {
        let world = setup_test_world();
        let ship1 = create_test_ship("ship1", "team1");
        let ship2 = create_test_ship("ship2", "team2");
        world.write().unwrap().add_ship(ship1);
        world.write().unwrap().add_ship(ship2);
        
        let request = Json(HailRequest {
            target_ship_id: "ship2".to_string(),
            message: "Greetings".to_string(),
        });
        
        let result = hail("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_hail_target_not_found() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(HailRequest {
            target_ship_id: "ship2".to_string(),
            message: "Greetings".to_string(),
        });
        
        let result = hail("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::BadRequest));
    }
    
    #[test]
    fn test_respond() {
        let world = setup_test_world();
        let ship1 = create_test_ship("ship1", "team1");
        let ship2 = create_test_ship("ship2", "team2");
        world.write().unwrap().add_ship(ship1);
        world.write().unwrap().add_ship(ship2);
        
        // First, create a hail
        let hail_request = Json(HailRequest {
            target_ship_id: "ship2".to_string(),
            message: "Greetings".to_string(),
        });
        let hail_result = hail("ship1".to_string(), hail_request, &State::from(&world));
        let message_id = hail_result.unwrap().into_inner().message_id;
        
        // Now respond
        let respond_request = Json(RespondRequest {
            message_id,
            response: "Hello".to_string(),
        });
        
        let result = respond("ship2".to_string(), respond_request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_respond_wrong_recipient() {
        let world = setup_test_world();
        let ship1 = create_test_ship("ship1", "team1");
        let ship2 = create_test_ship("ship2", "team2");
        let ship3 = create_test_ship("ship3", "team3");
        world.write().unwrap().add_ship(ship1);
        world.write().unwrap().add_ship(ship2);
        world.write().unwrap().add_ship(ship3);
        
        // Create a hail from ship1 to ship2
        let hail_request = Json(HailRequest {
            target_ship_id: "ship2".to_string(),
            message: "Greetings".to_string(),
        });
        let hail_result = hail("ship1".to_string(), hail_request, &State::from(&world));
        let message_id = hail_result.unwrap().into_inner().message_id;
        
        // Try to respond from ship3 (not the recipient)
        let respond_request = Json(RespondRequest {
            message_id,
            response: "Hello".to_string(),
        });
        
        let result = respond("ship3".to_string(), respond_request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::BadRequest));
    }
    
    #[test]
    fn test_jam() {
        let world = setup_test_world();
        let ship1 = create_test_ship("ship1", "team1");
        let ship2 = create_test_ship("ship2", "team2");
        world.write().unwrap().add_ship(ship1);
        world.write().unwrap().add_ship(ship2);
        
        let request = Json(JamRequest {
            target_ship_id: "ship2".to_string(),
            duration: 10.0,
        });
        
        let result = jam("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
    }
    
    #[test]
    fn test_command_fighters() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(FighterCommandRequest {
            fighter_ids: vec!["fighter1".to_string(), "fighter2".to_string()],
            command: FighterCommandTypeDto::Attack {
                target_id: "enemy1".to_string(),
            },
        });
        
        let result = command_fighters("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_command_fighters_jammed() {
        let world = setup_test_world();
        let mut ship = create_test_ship("ship1", "team1");
        ship.status.status_effects.push(StatusEffect {
            effect_type: StatusEffectType::Ion,
            duration: 5.0,
            magnitude: 1.0,
        });
        world.write().unwrap().add_ship(ship);
        
        let request = Json(FighterCommandRequest {
            fighter_ids: vec!["fighter1".to_string()],
            command: FighterCommandTypeDto::Launch,
        });
        
        let result = command_fighters("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::BadRequest));
    }
}
