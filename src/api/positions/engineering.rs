//! Engineering Officer API endpoints
//!
//! Handles power allocation, cooling allocation, and repairs

use rocket::{Route, State, serde::json::Json, http::Status};
use rocket::{routes, post, patch, get};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::state::SharedGameWorld;

/// Request to allocate power to modules
#[derive(Debug, Deserialize)]
pub struct AllocatePowerRequest {
    pub allocations: HashMap<String, f32>, // module_id -> power_amount
}

/// Response for power allocation
#[derive(Debug, Serialize, Deserialize)]
pub struct AllocatePowerResponse {
    pub success: bool,
    pub allocated_modules: usize,
}

/// Request to allocate cooling to modules
#[derive(Debug, Deserialize)]
pub struct AllocateCoolingRequest {
    pub allocations: HashMap<String, f32>, // module_id -> cooling_amount
}

/// Response for cooling allocation
#[derive(Debug, Serialize, Deserialize)]
pub struct AllocateCoolingResponse {
    pub success: bool,
    pub allocated_modules: usize,
}

/// Request to repair a module
#[derive(Debug, Deserialize)]
pub struct RepairRequest {
    pub module_id: String,
}

/// Response for repair
#[derive(Debug, Serialize, Deserialize)]
pub struct RepairResponse {
    pub success: bool,
    pub module_id: String,
}

/// Ship status information
#[derive(Debug, Serialize, Deserialize)]
pub struct ShipStatusInfo {
    pub hull: f32,
    pub max_hull: f32,
    pub hull_percentage: f32,
    pub shields: f32,
    pub max_shields: f32,
    pub shields_percentage: f32,
    pub power_generation: f32,
    pub power_capacity: f32,
    pub power_usage: f32,
    pub cooling_capacity: f32,
    pub heat_generation: f32,
}

/// Module status information
#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleStatusInfo {
    pub module_id: String,
    pub health: f32,
    pub operational: bool,
}

/// Response for module status
#[derive(Debug, Serialize, Deserialize)]
pub struct ModulesStatusResponse {
    pub modules: Vec<ModuleStatusInfo>,
}

/// Request to activate an auxiliary module
#[derive(Debug, Deserialize)]
pub struct ActivateModuleRequest {
    // Empty for now - activation parameters could be added later
}

/// Response for auxiliary module activation
#[derive(Debug, Serialize, Deserialize)]
pub struct ActivateModuleResponse {
    pub success: bool,
    pub message: String,
    pub duration: Option<f32>,
    pub remaining_uses: Option<u32>,
}

/// Allocate power to modules
#[patch("/v1/ships/<ship_id>/power/allocate", data = "<request>")]
pub fn allocate_power(
    ship_id: String,
    request: Json<AllocatePowerRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<AllocatePowerResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    let allocated_count = request.allocations.len();
    
    // Record power allocations
    world.add_power_allocations(ship_id, request.allocations.clone());
    
    Ok(Json(AllocatePowerResponse {
        success: true,
        allocated_modules: allocated_count,
    }))
}

/// Allocate cooling to modules
#[patch("/v1/ships/<ship_id>/cooling/allocate", data = "<request>")]
pub fn allocate_cooling(
    ship_id: String,
    request: Json<AllocateCoolingRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<AllocateCoolingResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    let allocated_count = request.allocations.len();
    
    // Record cooling allocations
    world.add_cooling_allocations(ship_id, request.allocations.clone());
    
    Ok(Json(AllocateCoolingResponse {
        success: true,
        allocated_modules: allocated_count,
    }))
}

/// Initiate repair on a module
#[post("/v1/ships/<ship_id>/repair", data = "<request>")]
pub fn repair_module(
    ship_id: String,
    request: Json<RepairRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<RepairResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // Record repair command
    world.add_repair_command(ship_id, request.module_id.clone());
    
    Ok(Json(RepairResponse {
        success: true,
        module_id: request.module_id.clone(),
    }))
}

/// Get ship damage and power status
#[get("/v1/ships/<ship_id>/status")]
pub fn get_ship_status(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<ShipStatusInfo>, Status> {
    let world = world.read().unwrap();
    
    // Check if ship exists
    let ship = world.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let hull_pct = if ship.status.max_hull > 0.0 {
        (ship.status.hull / ship.status.max_hull) * 100.0
    } else {
        0.0
    };
    
    let shields_pct = if ship.status.max_shields > 0.0 {
        (ship.status.shields / ship.status.max_shields) * 100.0
    } else {
        0.0
    };
    
    Ok(Json(ShipStatusInfo {
        hull: ship.status.hull,
        max_hull: ship.status.max_hull,
        hull_percentage: hull_pct,
        shields: ship.status.shields,
        max_shields: ship.status.max_shields,
        shields_percentage: shields_pct,
        power_generation: ship.status.power_generation,
        power_capacity: ship.status.power_capacity,
        power_usage: ship.status.power_usage,
        cooling_capacity: ship.status.cooling_capacity,
        heat_generation: ship.status.heat_generation,
    }))
}

/// Get all module statuses
#[get("/v1/ships/<ship_id>/modules/status")]
pub fn get_modules_status(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<ModulesStatusResponse>, Status> {
    let world = world.read().unwrap();
    
    // Check if ship exists
    let ship = world.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let modules: Vec<ModuleStatusInfo> = ship.modules.iter()
        .map(|m| {
            let health = ship.status.module_health.get(&m.instance_id).copied().unwrap_or(100.0);
            ModuleStatusInfo {
                module_id: m.instance_id.clone(),
                health,
                operational: health > 0.0,
            }
        })
        .collect();
    
    Ok(Json(ModulesStatusResponse { modules }))
}

/// Activate an auxiliary module
#[post("/v1/ships/<ship_id>/modules/<module_id>/activate", data = "<_request>")]
pub fn activate_auxiliary_module(
    ship_id: String,
    module_id: String,
    _request: Json<ActivateModuleRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<ActivateModuleResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Check if ship exists
    let ship = world.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    // Find the module
    let module = ship.modules.iter()
        .find(|m| m.instance_id == module_id)
        .ok_or(Status::NotFound)?;
    
    // Get the module's duration stat
    let duration = module.get_stat_f64("duration").unwrap_or(10.0) as f32;
    
    // Record the activation command
    world.add_auxiliary_activation(ship_id, module_id.clone(), duration);
    
    Ok(Json(ActivateModuleResponse {
        success: true,
        message: "Auxiliary module activation queued".to_string(),
        duration: Some(duration),
        remaining_uses: None, // Will be updated by simulation
    }))
}

/// Returns all routes for the engineering officer position
pub fn routes() -> Vec<Route> {
    routes![
        allocate_power,
        allocate_cooling,
        repair_module,
        get_ship_status,
        get_modules_status,
        activate_auxiliary_module
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
    fn test_allocate_power() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let mut allocations = HashMap::new();
        allocations.insert("engine1".to_string(), 100.0);
        allocations.insert("shield1".to_string(), 50.0);
        
        let request = Json(AllocatePowerRequest { allocations });
        
        let result = allocate_power("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.allocated_modules, 2);
    }
    
    #[test]
    fn test_allocate_cooling() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let mut allocations = HashMap::new();
        allocations.insert("weapon1".to_string(), 30.0);
        
        let request = Json(AllocateCoolingRequest { allocations });
        
        let result = allocate_cooling("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.allocated_modules, 1);
    }
    
    #[test]
    fn test_repair_module() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(RepairRequest {
            module_id: "engine1".to_string(),
        });
        
        let result = repair_module("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert_eq!(response.module_id, "engine1");
    }
    
    #[test]
    fn test_get_ship_status() {
        let world = setup_test_world();
        let mut ship = create_test_ship("ship1", "team1");
        ship.status.hull = 800.0;
        ship.status.max_hull = 1000.0;
        world.write().unwrap().add_ship(ship);
        
        let result = get_ship_status("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert_eq!(response.hull, 800.0);
        assert_eq!(response.max_hull, 1000.0);
        assert_eq!(response.hull_percentage, 80.0);
    }
    
    #[test]
    fn test_get_modules_status() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = get_modules_status("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert_eq!(response.modules.len(), 0);
    }
}
