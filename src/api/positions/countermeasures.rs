//! Countermeasures Officer API endpoints
//!
//! Handles shields, countermeasures, and point defense systems.

use rocket::{Route, State, serde::json::Json, http::Status};
use rocket::{routes, post, get};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::SharedGameWorld;

/// Response for shield operations
#[derive(Debug, Serialize, Deserialize)]
pub struct ShieldResponse {
    pub success: bool,
    pub shields_raised: bool,
}

/// Shield status information
#[derive(Debug, Serialize, Deserialize)]
pub struct ShieldStatusResponse {
    pub current: f32,
    pub max: f32,
    pub percentage: f32,
    pub raised: bool,
}

/// Request to load countermeasures
#[derive(Debug, Deserialize)]
pub struct LoadCountermeasuresRequest {
    pub countermeasure_type: CountermeasureType,
    pub quantity: u32,
}

/// Type of countermeasure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CountermeasureType {
    Antimissile,
    Antitorpedo,
    Chaff,
}

/// Response for loading countermeasures
#[derive(Debug, Serialize, Deserialize)]
pub struct LoadCountermeasuresResponse {
    pub success: bool,
    pub loaded: u32,
}

/// Request to activate countermeasures
#[derive(Debug, Deserialize)]
pub struct ActivateCountermeasuresRequest {
    pub target_threat_ids: Vec<String>,
}

/// Response for activating countermeasures
#[derive(Debug, Serialize, Deserialize)]
pub struct ActivateCountermeasuresResponse {
    pub success: bool,
    pub threats_engaged: u32,
}

/// Request to toggle point defense
#[derive(Debug, Deserialize)]
pub struct TogglePointDefenseRequest {
    pub enabled: bool,
}

/// Response for toggling point defense
#[derive(Debug, Serialize, Deserialize)]
pub struct TogglePointDefenseResponse {
    pub success: bool,
    pub enabled: bool,
}

/// Raise shields to enable regeneration
#[post("/v1/ships/<ship_id>/shields/raise")]
pub fn raise_shields(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<ShieldResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    let ship = world.ships_mut().get_mut(&ship_id)
        .ok_or(Status::NotFound)?;
    
    // Raise shields
    ship.status.shields_raised = true;
    
    Ok(Json(ShieldResponse {
        success: true,
        shields_raised: true,
    }))
}

/// Lower shields to stop regeneration
#[post("/v1/ships/<ship_id>/shields/lower")]
pub fn lower_shields(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<ShieldResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    let ship = world.ships_mut().get_mut(&ship_id)
        .ok_or(Status::NotFound)?;
    
    // Lower shields
    ship.status.shields_raised = false;
    
    Ok(Json(ShieldResponse {
        success: true,
        shields_raised: false,
    }))
}

/// Get shield status
#[get("/v1/ships/<ship_id>/shields/status")]
pub fn get_shield_status(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<ShieldStatusResponse>, Status> {
    let world = world.read().unwrap();
    
    // Check if ship exists
    let ship = world.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let percentage = if ship.status.max_shields > 0.0 {
        (ship.status.shields / ship.status.max_shields) * 100.0
    } else {
        0.0
    };
    
    Ok(Json(ShieldStatusResponse {
        current: ship.status.shields,
        max: ship.status.max_shields,
        percentage,
        raised: ship.status.shields_raised,
    }))
}

/// Load countermeasures into the ship's systems
#[post("/v1/ships/<ship_id>/countermeasures/load", data = "<request>")]
pub fn load_countermeasures(
    ship_id: String,
    request: Json<LoadCountermeasuresRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<LoadCountermeasuresResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Add countermeasure load record
    world.add_countermeasure_load(
        ship_id,
        request.countermeasure_type.clone(),
        request.quantity,
    );
    
    Ok(Json(LoadCountermeasuresResponse {
        success: true,
        loaded: request.quantity,
    }))
}

/// Activate countermeasures against incoming threats
#[post("/v1/ships/<ship_id>/countermeasures/activate", data = "<request>")]
pub fn activate_countermeasures(
    ship_id: String,
    request: Json<ActivateCountermeasuresRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<ActivateCountermeasuresResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    let threats_engaged = request.target_threat_ids.len() as u32;
    
    // Add countermeasure activation record
    world.add_countermeasure_activation(
        ship_id,
        request.target_threat_ids.clone(),
    );
    
    Ok(Json(ActivateCountermeasuresResponse {
        success: true,
        threats_engaged,
    }))
}

/// Toggle automated point defense weapons
#[post("/v1/ships/<ship_id>/point-defense/toggle", data = "<request>")]
pub fn toggle_point_defense(
    ship_id: String,
    request: Json<TogglePointDefenseRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<TogglePointDefenseResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Update point defense setting
    world.set_point_defense(ship_id.clone(), request.enabled);
    
    Ok(Json(TogglePointDefenseResponse {
        success: true,
        enabled: request.enabled,
    }))
}

/// Returns all routes for the countermeasures officer position
pub fn routes() -> Vec<Route> {
    routes![
        raise_shields,
        lower_shields,
        get_shield_status,
        load_countermeasures,
        activate_countermeasures,
        toggle_point_defense
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameWorld;
    use crate::models::ship::Ship;
    use crate::models::status::ShipStatus;
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
    fn test_raise_shields() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = raise_shields("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert!(response.shields_raised);
        
        // Verify ship state was updated
        let world_read = world.read().unwrap();
        let ship = world_read.ships().get("ship1").unwrap();
        assert!(ship.status.shields_raised);
    }
    
    #[test]
    fn test_lower_shields() {
        let world = setup_test_world();
        let mut ship = create_test_ship("ship1", "team1");
        ship.status.shields_raised = true;
        world.write().unwrap().add_ship(ship);
        
        let result = lower_shields("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert!(!response.shields_raised);
        
        // Verify ship state was updated
        let world_read = world.read().unwrap();
        let ship = world_read.ships().get("ship1").unwrap();
        assert!(!ship.status.shields_raised);
    }
    
    #[test]
    fn test_raise_shields_not_found() {
        let world = setup_test_world();
        
        let result = raise_shields("ship1".to_string(), &State::from(&world));
        assert_eq!(result.err(), Some(Status::NotFound));
    }
    
    #[test]
    fn test_get_shield_status() {
        let world = setup_test_world();
        let mut ship = create_test_ship("ship1", "team1");
        ship.status.shields = 750.0;
        ship.status.max_shields = 1000.0;
        ship.status.shields_raised = true;
        world.write().unwrap().add_ship(ship);
        
        let result = get_shield_status("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert_eq!(response.current, 750.0);
        assert_eq!(response.max, 1000.0);
        assert_eq!(response.percentage, 75.0);
        assert!(response.raised);
    }
    
    #[test]
    fn test_get_shield_status_not_found() {
        let world = setup_test_world();
        
        let result = get_shield_status("ship1".to_string(), &State::from(&world));
        assert_eq!(result.err(), Some(Status::NotFound));
    }
    
    #[test]
    fn test_load_countermeasures() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(LoadCountermeasuresRequest {
            countermeasure_type: CountermeasureType::Antimissile,
            quantity: 10,
        });
        
        let result = load_countermeasures("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.loaded, 10);
    }
    
    #[test]
    fn test_load_countermeasures_not_found() {
        let world = setup_test_world();
        
        let request = Json(LoadCountermeasuresRequest {
            countermeasure_type: CountermeasureType::Chaff,
            quantity: 5,
        });
        
        let result = load_countermeasures("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::NotFound));
    }
    
    #[test]
    fn test_activate_countermeasures() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(ActivateCountermeasuresRequest {
            target_threat_ids: vec!["missile1".to_string(), "missile2".to_string()],
        });
        
        let result = activate_countermeasures("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.threats_engaged, 2);
    }
    
    #[test]
    fn test_activate_countermeasures_not_found() {
        let world = setup_test_world();
        
        let request = Json(ActivateCountermeasuresRequest {
            target_threat_ids: vec!["missile1".to_string()],
        });
        
        let result = activate_countermeasures("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::NotFound));
    }
    
    #[test]
    fn test_toggle_point_defense() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(TogglePointDefenseRequest {
            enabled: true,
        });
        
        let result = toggle_point_defense("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert!(response.enabled);
    }
    
    #[test]
    fn test_toggle_point_defense_disable() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(TogglePointDefenseRequest {
            enabled: false,
        });
        
        let result = toggle_point_defense("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert!(!response.enabled);
    }
    
    #[test]
    fn test_toggle_point_defense_not_found() {
        let world = setup_test_world();
        
        let request = Json(TogglePointDefenseRequest {
            enabled: true,
        });
        
        let result = toggle_point_defense("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::NotFound));
    }
}
