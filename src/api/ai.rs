//! AI Ship API endpoints
//!
//! This module provides REST API endpoints for managing AI-controlled ships.

use rocket::{State, get, post, delete};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::SharedGameWorld;
use crate::ai::{AIManager, AIPersonality, AIContextUpdate};

/// Request to create an AI-controlled ship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAIShipRequest {
    pub ship_id: Uuid,
    pub faction: String,
    pub personality: AIPersonality,
    pub patrol_route: Option<Vec<[f64; 3]>>,
}

/// Response for AI ship information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIShipResponse {
    pub ship_id: Uuid,
    pub personality: AIPersonality,
}

/// Request to update AI patrol route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatrolRouteRequest {
    pub waypoints: Vec<[f64; 3]>,
}

/// Request to add hostile faction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostileFactionRequest {
    pub faction: String,
}

/// List all AI-controlled ships
#[get("/v1/ai/ships")]
pub fn list_ai_ships(
    world: &State<SharedGameWorld>,
) -> Json<Vec<AIShipResponse>> {
    let world = world.read().unwrap();
    
    let ship_ids = world.ai_manager.get_ship_ids();
    let ships: Vec<AIShipResponse> = ship_ids
        .iter()
        .filter_map(|ship_id| {
            world.ai_manager.get_personality(*ship_id).map(|personality| {
                AIShipResponse {
                    ship_id: *ship_id,
                    personality,
                }
            })
        })
        .collect();
    
    Json(ships)
}

/// Get AI ship information
#[get("/v1/ai/ships/<ship_id>")]
pub fn get_ai_ship(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Option<Json<AIShipResponse>> {
    let ship_id = Uuid::parse_str(&ship_id).ok()?;
    let world = world.read().unwrap();
    
    world.ai_manager.get_personality(ship_id).map(|personality| {
        Json(AIShipResponse {
            ship_id,
            personality,
        })
    })
}

/// Register a new AI-controlled ship
#[post("/v1/ai/ships", data = "<request>")]
pub fn create_ai_ship(
    request: Json<CreateAIShipRequest>,
    world: &State<SharedGameWorld>,
) -> Json<AIShipResponse> {
    let world = world.read().unwrap();
    
    world.ai_manager.register_ship(
        request.ship_id,
        request.faction.clone(),
        request.personality,
    );
    
    if let Some(route) = &request.patrol_route {
        world.ai_manager.set_patrol_route(request.ship_id, route.clone());
    }
    
    Json(AIShipResponse {
        ship_id: request.ship_id,
        personality: request.personality,
    })
}

/// Remove an AI-controlled ship
#[delete("/v1/ai/ships/<ship_id>")]
pub fn delete_ai_ship(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Option<()> {
    let ship_id = Uuid::parse_str(&ship_id).ok()?;
    let world = world.read().unwrap();
    
    world.ai_manager.unregister_ship(ship_id);
    
    Some(())
}

/// Set patrol route for an AI ship
#[post("/v1/ai/ships/<ship_id>/patrol", data = "<request>")]
pub fn set_patrol_route(
    ship_id: String,
    request: Json<PatrolRouteRequest>,
    world: &State<SharedGameWorld>,
) -> Option<Json<AIShipResponse>> {
    let ship_id = Uuid::parse_str(&ship_id).ok()?;
    let world = world.read().unwrap();
    
    world.ai_manager.set_patrol_route(ship_id, request.waypoints.clone());
    
    world.ai_manager.get_personality(ship_id).map(|personality| {
        Json(AIShipResponse {
            ship_id,
            personality,
        })
    })
}

/// Add hostile faction for an AI ship
#[post("/v1/ai/ships/<ship_id>/hostile", data = "<request>")]
pub fn add_hostile_faction(
    ship_id: String,
    request: Json<HostileFactionRequest>,
    world: &State<SharedGameWorld>,
) -> Option<Json<AIShipResponse>> {
    let ship_id = Uuid::parse_str(&ship_id).ok()?;
    let world = world.read().unwrap();
    
    world.ai_manager.add_hostile_faction(ship_id, request.faction.clone());
    
    world.ai_manager.get_personality(ship_id).map(|personality| {
        Json(AIShipResponse {
            ship_id,
            personality,
        })
    })
}

/// Return all routes for this module
pub fn routes() -> Vec<rocket::Route> {
    rocket::routes![
        list_ai_ships,
        get_ai_ship,
        create_ai_ship,
        delete_ai_ship,
        set_patrol_route,
        add_hostile_faction,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameWorld;
    use std::sync::{Arc, RwLock};
    use rocket::local::blocking::Client;
    use rocket::http::{Status, ContentType};
    
    fn create_test_client() -> Client {
        let world = Arc::new(RwLock::new(GameWorld::new()));
        
        let rocket = rocket::build()
            .manage(world)
            .mount("/", routes());
        
        Client::tracked(rocket).expect("valid rocket instance")
    }
    
    #[test]
    fn test_create_ai_ship() {
        let client = create_test_client();
        
        let ship_id = Uuid::new_v4();
        let request = CreateAIShipRequest {
            ship_id,
            faction: "Federation".to_string(),
            personality: AIPersonality::Aggressive,
            patrol_route: None,
        };
        
        let response = client
            .post("/v1/ai/ships")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let ship: AIShipResponse = response.into_json().unwrap();
        assert_eq!(ship.ship_id, ship_id);
        assert_eq!(ship.personality, AIPersonality::Aggressive);
    }
    
    #[test]
    fn test_list_ai_ships() {
        let client = create_test_client();
        
        // Create a ship first
        let ship_id = Uuid::new_v4();
        let request = CreateAIShipRequest {
            ship_id,
            faction: "Federation".to_string(),
            personality: AIPersonality::Patrol,
            patrol_route: None,
        };
        
        client
            .post("/v1/ai/ships")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        // List ships
        let response = client.get("/v1/ai/ships").dispatch();
        assert_eq!(response.status(), Status::Ok);
        
        let ships: Vec<AIShipResponse> = response.into_json().unwrap();
        assert!(!ships.is_empty());
        assert!(ships.iter().any(|s| s.ship_id == ship_id));
    }
    
    #[test]
    fn test_get_ai_ship() {
        let client = create_test_client();
        
        let ship_id = Uuid::new_v4();
        let request = CreateAIShipRequest {
            ship_id,
            faction: "Klingon".to_string(),
            personality: AIPersonality::Aggressive,
            patrol_route: None,
        };
        
        client
            .post("/v1/ai/ships")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        let response = client.get(format!("/v1/ai/ships/{}", ship_id)).dispatch();
        assert_eq!(response.status(), Status::Ok);
        
        let ship: AIShipResponse = response.into_json().unwrap();
        assert_eq!(ship.ship_id, ship_id);
    }
    
    #[test]
    fn test_set_patrol_route() {
        let client = create_test_client();
        
        let ship_id = Uuid::new_v4();
        let request = CreateAIShipRequest {
            ship_id,
            faction: "Federation".to_string(),
            personality: AIPersonality::Patrol,
            patrol_route: None,
        };
        
        client
            .post("/v1/ai/ships")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        let patrol_request = PatrolRouteRequest {
            waypoints: vec![
                [0.0, 0.0, 0.0],
                [100.0, 0.0, 0.0],
                [100.0, 100.0, 0.0],
            ],
        };
        
        let response = client
            .post(format!("/v1/ai/ships/{}/patrol", ship_id))
            .header(ContentType::JSON)
            .json(&patrol_request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
    }
    
    #[test]
    fn test_delete_ai_ship() {
        let client = create_test_client();
        
        let ship_id = Uuid::new_v4();
        let request = CreateAIShipRequest {
            ship_id,
            faction: "Romulan".to_string(),
            personality: AIPersonality::Defensive,
            patrol_route: None,
        };
        
        client
            .post("/v1/ai/ships")
            .header(ContentType::JSON)
            .json(&request)
            .dispatch();
        
        let response = client.delete(format!("/v1/ai/ships/{}", ship_id)).dispatch();
        assert_eq!(response.status(), Status::Ok);
        
        // Verify it's deleted
        let get_response = client.get(format!("/v1/ai/ships/{}", ship_id)).dispatch();
        assert_eq!(get_response.status(), Status::NotFound);
    }
}
