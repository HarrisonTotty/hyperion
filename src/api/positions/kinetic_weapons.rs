//! Kinetic Weapons Officer API endpoints
//!
//! Handles kinetic weapons (railguns, cannons, etc.)

use rocket::{Route, State, serde::json::Json, http::Status};
use rocket::{routes, post, get};
use serde::{Deserialize, Serialize};

use crate::state::SharedGameWorld;

/// Request to set target
#[derive(Debug, Deserialize)]
pub struct SetTargetRequest {
    pub target_id: String,
}

/// Request to configure weapon kind
#[derive(Debug, Deserialize)]
pub struct ConfigureWeaponRequest {
    pub weapon_id: String,
    pub kind: String,
}

/// Request to load ammunition
#[derive(Debug, Deserialize)]
pub struct LoadAmmoRequest {
    pub weapon_id: String,
    pub ammo_type: String,
    pub quantity: u32,
}

/// Request to fire weapon
#[derive(Debug, Deserialize)]
pub struct FireWeaponRequest {
    pub weapon_id: String,
}

/// Request to toggle auto-fire
#[derive(Debug, Deserialize)]
pub struct ToggleAutoRequest {
    pub weapon_id: String,
    pub enabled: bool,
}

/// Generic success response
#[derive(Debug, Serialize, Deserialize)]
pub struct KineticResponse {
    pub success: bool,
}

/// Weapon status
#[derive(Debug, Serialize, Deserialize)]
pub struct KineticWeaponStatus {
    pub weapon_id: String,
    pub kind: Option<String>,
    pub loaded_ammo: u32,
    pub ready: bool,
}

/// Set target for kinetic weapons
#[post("/v1/ships/<ship_id>/kinetic-weapons/target", data = "<request>")]
pub fn set_target(
    ship_id: String,
    request: Json<SetTargetRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<KineticResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.set_kinetic_weapon_target(ship_id, request.target_id.clone());
    
    Ok(Json(KineticResponse { success: true }))
}

/// Configure weapon kind
#[post("/v1/ships/<ship_id>/kinetic-weapons/<weapon_id>/configure", data = "<request>")]
pub fn configure_weapon(
    ship_id: String,
    weapon_id: String,
    request: Json<ConfigureWeaponRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<KineticResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.configure_kinetic_weapon(ship_id, weapon_id, request.kind.clone());
    
    Ok(Json(KineticResponse { success: true }))
}

/// Load ammunition
#[post("/v1/ships/<ship_id>/kinetic-weapons/<weapon_id>/load", data = "<request>")]
pub fn load_ammo(
    ship_id: String,
    weapon_id: String,
    request: Json<LoadAmmoRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<KineticResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.load_kinetic_ammo(ship_id, weapon_id, request.ammo_type.clone(), request.quantity);
    
    Ok(Json(KineticResponse { success: true }))
}

/// Fire weapon
#[post("/v1/ships/<ship_id>/kinetic-weapons/<weapon_id>/fire")]
pub fn fire_weapon(
    ship_id: String,
    weapon_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<KineticResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.fire_kinetic_weapon(ship_id, weapon_id);
    
    Ok(Json(KineticResponse { success: true }))
}

/// Toggle auto-fire
#[post("/v1/ships/<ship_id>/kinetic-weapons/<weapon_id>/auto", data = "<request>")]
pub fn toggle_auto(
    ship_id: String,
    weapon_id: String,
    request: Json<ToggleAutoRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<KineticResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.set_kinetic_auto_fire(ship_id, weapon_id, request.enabled);
    
    Ok(Json(KineticResponse { success: true }))
}

/// Get weapon status
#[get("/v1/ships/<ship_id>/kinetic-weapons/status")]
pub fn get_status(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<Vec<KineticWeaponStatus>>, Status> {
    let world = world.read().unwrap();
    
    let ship = world.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let weapons = ship.weapons.iter()
        .map(|w| KineticWeaponStatus {
            weapon_id: w.id.clone(),
            kind: w.kind.clone(),
            loaded_ammo: 0,
            ready: true,
        })
        .collect();
    
    Ok(Json(weapons))
}

/// Returns all routes for the kinetic weapons officer position
pub fn routes() -> Vec<Route> {
    routes![
        set_target,
        configure_weapon,
        load_ammo,
        fire_weapon,
        toggle_auto,
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
    }
    
    #[test]
    fn test_configure_weapon() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(ConfigureWeaponRequest {
            weapon_id: "kinetic1".to_string(),
            kind: "railgun".to_string(),
        });
        
        let result = configure_weapon("ship1".to_string(), "kinetic1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_load_ammo() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(LoadAmmoRequest {
            weapon_id: "kinetic1".to_string(),
            ammo_type: "sabot".to_string(),
            quantity: 100,
        });
        
        let result = load_ammo("ship1".to_string(), "kinetic1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_fire_weapon() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = fire_weapon("ship1".to_string(), "kinetic1".to_string(), &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_toggle_auto() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(ToggleAutoRequest {
            weapon_id: "kinetic1".to_string(),
            enabled: true,
        });
        
        let result = toggle_auto("ship1".to_string(), "kinetic1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
}
