//! Energy Weapons Officer API endpoints
//!
//! Handles directed-energy weapons (lasers, beams, plasma, etc.)

use rocket::{Route, State, serde::json::Json, http::Status};
use rocket::{routes, post, get};
use serde::{Deserialize, Serialize};

use crate::state::SharedGameWorld;

/// Request to set target for energy weapons
#[derive(Debug, Deserialize)]
pub struct SetTargetRequest {
    pub target_id: String,
}

/// Response for setting target
#[derive(Debug, Serialize, Deserialize)]
pub struct SetTargetResponse {
    pub success: bool,
    pub target_id: String,
}

/// Request to fire a weapon
#[derive(Debug, Deserialize)]
pub struct FireWeaponRequest {
    pub weapon_id: String,
}

/// Response for firing weapon
#[derive(Debug, Serialize, Deserialize)]
pub struct FireWeaponResponse {
    pub success: bool,
    pub weapon_id: String,
}

/// Request to toggle automatic firing
#[derive(Debug, Deserialize)]
pub struct ToggleAutoRequest {
    pub weapon_id: String,
    pub enabled: bool,
}

/// Response for toggling automatic firing
#[derive(Debug, Serialize, Deserialize)]
pub struct ToggleAutoResponse {
    pub success: bool,
    pub weapon_id: String,
    pub auto_enabled: bool,
}

/// Request to activate radial weapon
#[derive(Debug, Deserialize)]
pub struct ActivateRadialRequest {
    pub weapon_id: String,
}

/// Response for activating radial weapon
#[derive(Debug, Serialize, Deserialize)]
pub struct ActivateRadialResponse {
    pub success: bool,
    pub weapon_id: String,
}

/// Energy weapon status
#[derive(Debug, Serialize, Deserialize)]
pub struct EnergyWeaponStatus {
    pub weapon_id: String,
    pub cooldown: f32,
    pub ready: bool,
    pub auto_fire: bool,
    pub tags: Vec<String>,
}

/// Response for getting weapon status
#[derive(Debug, Serialize, Deserialize)]
pub struct WeaponStatusResponse {
    pub weapons: Vec<EnergyWeaponStatus>,
    pub current_target: Option<String>,
}

/// Set target for energy weapons
#[post("/v1/ships/<ship_id>/energy-weapons/target", data = "<request>")]
pub fn set_target(
    ship_id: String,
    request: Json<SetTargetRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<SetTargetResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Set energy weapon target
    world.set_energy_weapon_target(ship_id, request.target_id.clone());
    
    Ok(Json(SetTargetResponse {
        success: true,
        target_id: request.target_id.clone(),
    }))
}

/// Fire an energy weapon manually
#[post("/v1/ships/<ship_id>/energy-weapons/fire", data = "<request>")]
pub fn fire_weapon(
    ship_id: String,
    request: Json<FireWeaponRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<FireWeaponResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Record weapon fire command
    world.add_weapon_fire_command(ship_id, request.weapon_id.clone());
    
    Ok(Json(FireWeaponResponse {
        success: true,
        weapon_id: request.weapon_id.clone(),
    }))
}

/// Toggle automatic firing for a weapon
#[post("/v1/ships/<ship_id>/energy-weapons/auto", data = "<request>")]
pub fn toggle_auto(
    ship_id: String,
    request: Json<ToggleAutoRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<ToggleAutoResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Set auto-fire mode
    world.set_weapon_auto_fire(ship_id, request.weapon_id.clone(), request.enabled);
    
    Ok(Json(ToggleAutoResponse {
        success: true,
        weapon_id: request.weapon_id.clone(),
        auto_enabled: request.enabled,
    }))
}

/// Activate a radial emission weapon
#[post("/v1/ships/<ship_id>/radial-weapons/activate", data = "<request>")]
pub fn activate_radial(
    ship_id: String,
    request: Json<ActivateRadialRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<ActivateRadialResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Record radial weapon activation
    world.add_radial_weapon_activation(ship_id, request.weapon_id.clone());
    
    Ok(Json(ActivateRadialResponse {
        success: true,
        weapon_id: request.weapon_id.clone(),
    }))
}

/// Get energy weapon status
#[get("/v1/ships/<ship_id>/energy-weapons/status")]
pub fn get_status(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<WeaponStatusResponse>, Status> {
    let world = world.read().unwrap();
    
    // Check if ship exists
    let ship = world.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    // Get energy weapons from ship
    let weapons: Vec<EnergyWeaponStatus> = ship.weapons.iter()
        .map(|w| EnergyWeaponStatus {
            weapon_id: w.id.clone(),
            cooldown: 0.0, // Would be retrieved from simulation
            ready: true,
            auto_fire: false,
            tags: vec![], // Tags would be looked up from WeaponConfig
        })
        .collect();
    
    let current_target = world.get_energy_weapon_target(&ship_id);
    
    Ok(Json(WeaponStatusResponse {
        weapons,
        current_target,
    }))
}

/// Returns all routes for the energy weapons officer position
pub fn routes() -> Vec<Route> {
    routes![
        set_target,
        fire_weapon,
        toggle_auto,
        activate_radial,
        get_status
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
    fn test_set_target() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(SetTargetRequest {
            target_id: "enemy1".to_string(),
        });
        
        let result = set_target("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.target_id, "enemy1");
    }
    
    #[test]
    fn test_set_target_not_found() {
        let world = setup_test_world();
        
        let request = Json(SetTargetRequest {
            target_id: "enemy1".to_string(),
        });
        
        let result = set_target("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::NotFound));
    }
    
    #[test]
    fn test_fire_weapon() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(FireWeaponRequest {
            weapon_id: "laser1".to_string(),
        });
        
        let result = fire_weapon("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.weapon_id, "laser1");
    }
    
    #[test]
    fn test_toggle_auto() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(ToggleAutoRequest {
            weapon_id: "laser1".to_string(),
            enabled: true,
        });
        
        let result = toggle_auto("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.weapon_id, "laser1");
        assert!(response.auto_enabled);
    }
    
    #[test]
    fn test_activate_radial() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(ActivateRadialRequest {
            weapon_id: "emp1".to_string(),
        });
        
        let result = activate_radial("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.weapon_id, "emp1");
    }
    
    #[test]
    fn test_get_status() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = get_status("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert_eq!(response.weapons.len(), 0);
        assert_eq!(response.current_target, None);
    }
}
