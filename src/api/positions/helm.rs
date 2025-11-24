//! Helm Officer API endpoints
//!
//! Handles navigation, thrust, rotation, FTL drives, and docking

use rocket::{Route, State, serde::json::Json, http::Status};
use rocket::{routes, post, get};
use serde::{Deserialize, Serialize};

use crate::state::SharedGameWorld;

/// Request to set thrust vector
#[derive(Debug, Deserialize)]
pub struct SetThrustRequest {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Response for thrust command
#[derive(Debug, Serialize, Deserialize)]
pub struct ThrustResponse {
    pub success: bool,
}

/// Request to set rotation
#[derive(Debug, Deserialize)]
pub struct SetRotationRequest {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

/// Response for rotation command
#[derive(Debug, Serialize, Deserialize)]
pub struct RotationResponse {
    pub success: bool,
}

/// Request to engage warp drive
#[derive(Debug, Deserialize)]
pub struct WarpRequest {
    pub destination_x: f64,
    pub destination_y: f64,
    pub destination_z: f64,
}

/// Response for warp command
#[derive(Debug, Serialize, Deserialize)]
pub struct WarpResponse {
    pub success: bool,
    pub tachyon_disabled: bool,
}

/// Request to engage jump drive
#[derive(Debug, Deserialize)]
pub struct JumpRequest {
    pub destination_x: f64,
    pub destination_y: f64,
    pub destination_z: f64,
}

/// Response for jump command
#[derive(Debug, Serialize, Deserialize)]
pub struct JumpResponse {
    pub success: bool,
    pub tachyon_disabled: bool,
}

/// Request to initiate docking
#[derive(Debug, Deserialize)]
pub struct DockRequest {
    pub station_id: String,
}

/// Response for dock command
#[derive(Debug, Serialize, Deserialize)]
pub struct DockResponse {
    pub success: bool,
}

/// Helm status information
#[derive(Debug, Serialize, Deserialize)]
pub struct HelmStatusResponse {
    pub effective_weight: f32,
    pub warp_available: bool,
    pub jump_available: bool,
    pub tachyon_effect: bool,
}

/// Set thrust vector
#[post("/v1/ships/<ship_id>/helm/thrust", data = "<request>")]
pub fn set_thrust(
    ship_id: String,
    request: Json<SetThrustRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<ThrustResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.add_thrust_command(ship_id, request.x, request.y, request.z);
    
    Ok(Json(ThrustResponse { success: true }))
}

/// Set rotation
#[post("/v1/ships/<ship_id>/helm/rotate", data = "<request>")]
pub fn set_rotation(
    ship_id: String,
    request: Json<SetRotationRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<RotationResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.add_rotation_command(ship_id, request.pitch, request.yaw, request.roll);
    
    Ok(Json(RotationResponse { success: true }))
}

/// Full stop
#[post("/v1/ships/<ship_id>/helm/stop")]
pub fn full_stop(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<ThrustResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.add_stop_command(ship_id);
    
    Ok(Json(ThrustResponse { success: true }))
}

/// Engage warp drive
#[post("/v1/ships/<ship_id>/helm/warp", data = "<request>")]
pub fn engage_warp(
    ship_id: String,
    request: Json<WarpRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<WarpResponse>, Status> {
    let world_read = world.read().unwrap();
    
    let ship = world_read.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let tachyon_disabled = ship.status.is_tachyon_disabled();
    drop(world_read);
    
    if tachyon_disabled {
        return Ok(Json(WarpResponse {
            success: false,
            tachyon_disabled: true,
        }));
    }
    
    let mut world_write = world.write().unwrap();
    world_write.add_warp_command(ship_id, request.destination_x, request.destination_y, request.destination_z);
    
    Ok(Json(WarpResponse {
        success: true,
        tachyon_disabled: false,
    }))
}

/// Engage jump drive
#[post("/v1/ships/<ship_id>/helm/jump", data = "<request>")]
pub fn engage_jump(
    ship_id: String,
    request: Json<JumpRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<JumpResponse>, Status> {
    let world_read = world.read().unwrap();
    
    let ship = world_read.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let tachyon_disabled = ship.status.is_tachyon_disabled();
    drop(world_read);
    
    if tachyon_disabled {
        return Ok(Json(JumpResponse {
            success: false,
            tachyon_disabled: true,
        }));
    }
    
    let mut world_write = world.write().unwrap();
    world_write.add_jump_command(ship_id, request.destination_x, request.destination_y, request.destination_z);
    
    Ok(Json(JumpResponse {
        success: true,
        tachyon_disabled: false,
    }))
}

/// Initiate docking
#[post("/v1/ships/<ship_id>/helm/dock", data = "<request>")]
pub fn initiate_dock(
    ship_id: String,
    request: Json<DockRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<DockResponse>, Status> {
    let mut world = world.write().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    world.add_dock_command(ship_id, request.station_id.clone());
    
    Ok(Json(DockResponse { success: true }))
}

/// Get helm status
#[get("/v1/ships/<ship_id>/helm/status")]
pub fn get_helm_status(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<HelmStatusResponse>, Status> {
    let world = world.read().unwrap();
    
    let ship = world.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let tachyon_effect = ship.status.is_tachyon_disabled();
    
    Ok(Json(HelmStatusResponse {
        effective_weight: ship.status.effective_weight,
        warp_available: !tachyon_effect,
        jump_available: !tachyon_effect,
        tachyon_effect,
    }))
}

/// Returns all routes for the helm officer position
pub fn routes() -> Vec<Route> {
    routes![
        set_thrust,
        set_rotation,
        full_stop,
        engage_warp,
        engage_jump,
        initiate_dock,
        get_helm_status
    ]
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
    fn test_set_thrust() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(SetThrustRequest {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        
        let result = set_thrust("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_set_rotation() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(SetRotationRequest {
            pitch: 0.1,
            yaw: 0.0,
            roll: 0.0,
        });
        
        let result = set_rotation("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_full_stop() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = full_stop("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_engage_warp() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(WarpRequest {
            destination_x: 1000.0,
            destination_y: 2000.0,
            destination_z: 3000.0,
        });
        
        let result = engage_warp("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert!(!response.tachyon_disabled);
    }
    
    #[test]
    fn test_engage_warp_tachyon_disabled() {
        let world = setup_test_world();
        let mut ship = create_test_ship("ship1", "team1");
        ship.status.status_effects.push(StatusEffect {
            effect_type: StatusEffectType::Tachyon,
            duration: 5.0,
            magnitude: 1.0,
        });
        world.write().unwrap().add_ship(ship);
        
        let request = Json(WarpRequest {
            destination_x: 1000.0,
            destination_y: 2000.0,
            destination_z: 3000.0,
        });
        
        let result = engage_warp("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(!response.success);
        assert!(response.tachyon_disabled);
    }
    
    #[test]
    fn test_engage_jump() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(JumpRequest {
            destination_x: 1000.0,
            destination_y: 2000.0,
            destination_z: 3000.0,
        });
        
        let result = engage_jump("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert!(!response.tachyon_disabled);
    }
    
    #[test]
    fn test_initiate_dock() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(DockRequest {
            station_id: "station1".to_string(),
        });
        
        let result = initiate_dock("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_get_helm_status() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = get_helm_status("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.warp_available);
        assert!(response.jump_available);
        assert!(!response.tachyon_effect);
    }
}
