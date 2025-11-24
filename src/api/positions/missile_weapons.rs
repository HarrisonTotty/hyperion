//! Missile Weapons Officer API endpoints
//!
//! Handles missiles and torpedoes

use rocket::{Route, State, serde::json::Json, http::Status};
use rocket::{routes, post, get};
use serde::{Deserialize, Serialize};

use crate::state::SharedGameWorld;

/// Request to set target
#[derive(Debug, Deserialize)]
pub struct SetTargetRequest {
    pub target_id: String,
}

/// Request to load ordnance
#[derive(Debug, Deserialize)]
pub struct LoadOrdnanceRequest {
    pub weapon_id: String,
    pub ordnance_type: String,
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
pub struct MissileResponse {
    pub success: bool,
}

/// Weapon status
#[derive(Debug, Serialize, Deserialize)]
pub struct MissileWeaponStatus {
    pub weapon_id: String,
    pub loaded_ordnance: String,
    pub quantity: u32,
    pub ready: bool,
}

/// Set target for missile weapons
#[post("/v1/ships/<ship_id>/missile-weapons/target", data = "<request>")]
pub fn set_target(
    ship_id: String,
    request: Json<SetTargetRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<MissileResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.set_missile_weapon_target(ship_id, request.target_id.clone());
    
    Ok(Json(MissileResponse { success: true }))
}

/// Load ordnance
#[post("/v1/ships/<ship_id>/missile-weapons/<weapon_id>/load", data = "<request>")]
pub fn load_ordnance(
    ship_id: String,
    weapon_id: String,
    request: Json<LoadOrdnanceRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<MissileResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.load_missile_ordnance(ship_id, weapon_id, request.ordnance_type.clone(), request.quantity);
    
    Ok(Json(MissileResponse { success: true }))
}

/// Fire weapon
#[post("/v1/ships/<ship_id>/missile-weapons/<weapon_id>/fire")]
pub fn fire_weapon(
    ship_id: String,
    weapon_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<MissileResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.fire_missile_weapon(ship_id, weapon_id);
    
    Ok(Json(MissileResponse { success: true }))
}

/// Toggle auto-fire
#[post("/v1/ships/<ship_id>/missile-weapons/<weapon_id>/auto", data = "<request>")]
pub fn toggle_auto(
    ship_id: String,
    weapon_id: String,
    request: Json<ToggleAutoRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<MissileResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.set_missile_auto_fire(ship_id, weapon_id, request.enabled);
    
    Ok(Json(MissileResponse { success: true }))
}

/// Get weapon status
#[get("/v1/ships/<ship_id>/missile-weapons/status")]
pub fn get_status(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<Vec<MissileWeaponStatus>>, Status> {
    let world = world.read().unwrap();
    
    let ship = world.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let weapons = ship.weapons.iter()
        .map(|w| MissileWeaponStatus {
            weapon_id: w.id.clone(),
            loaded_ordnance: "none".to_string(),
            quantity: 0,
            ready: true,
        })
        .collect();
    
    Ok(Json(weapons))
}

/// Returns all routes for the missile weapons officer position
pub fn routes() -> Vec<Route> {
    routes![
        set_target,
        load_ordnance,
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
    fn test_load_ordnance() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(LoadOrdnanceRequest {
            weapon_id: "missile1".to_string(),
            ordnance_type: "photon_torpedo".to_string(),
            quantity: 10,
        });
        
        let result = load_ordnance("ship1".to_string(), "missile1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_fire_weapon() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = fire_weapon("ship1".to_string(), "missile1".to_string(), &State::from(&world));
        assert!(result.is_ok());
    }
}
