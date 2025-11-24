//! Science Officer API endpoints
//!
//! Handles scanning, contacts, threat detection, and analysis

use rocket::{Route, State, serde::json::Json, http::Status};
use rocket::{routes, post, get};
use serde::{Deserialize, Serialize};

use crate::state::SharedGameWorld;

/// Request to scan a target
#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub target_id: String,
}

/// Response for scan
#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResponse {
    pub success: bool,
    pub ion_jammed: bool,
}

/// Request to analyze a target
#[derive(Debug, Deserialize)]
pub struct AnalyzeRequest {
    pub target_id: String,
}

/// Response for deep analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResponse {
    pub target_id: String,
    pub class: String,
    pub faction: String,
    pub hull_percentage: f32,
    pub shields_percentage: f32,
}

/// Contact information
#[derive(Debug, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub distance: f64,
    pub bearing: f64,
}

/// Contacts response
#[derive(Debug, Serialize, Deserialize)]
pub struct ContactsResponse {
    pub contacts: Vec<Contact>,
}

/// Threat information
#[derive(Debug, Serialize, Deserialize)]
pub struct Threat {
    pub id: String,
    pub threat_type: String, // "missile", "torpedo", etc.
    pub distance: f64,
    pub time_to_impact: f64,
}

/// Threats response
#[derive(Debug, Serialize, Deserialize)]
pub struct ThreatsResponse {
    pub threats: Vec<Threat>,
}

/// Navigation information
#[derive(Debug, Serialize, Deserialize)]
pub struct NavigationInfo {
    pub target_id: String,
    pub heading: f64,
    pub distance: f64,
    pub eta: f64,
}

/// Scan target ship
#[post("/v1/ships/<ship_id>/scan", data = "<request>")]
pub fn scan_target(
    ship_id: String,
    request: Json<ScanRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<ScanResponse>, Status> {
    let world_read = world.read().unwrap();
    
    let ship = world_read.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let ion_jammed = ship.status.is_ion_jammed();
    drop(world_read);
    
    if ion_jammed {
        return Ok(Json(ScanResponse {
            success: false,
            ion_jammed: true,
        }));
    }
    
    let mut world_write = world.write().unwrap();
    world_write.add_scan_command(ship_id, request.target_id.clone());
    
    Ok(Json(ScanResponse {
        success: true,
        ion_jammed: false,
    }))
}

/// Get detected contacts
#[get("/v1/ships/<ship_id>/contacts")]
pub fn get_contacts(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<ContactsResponse>, Status> {
    let world = world.read().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // In a real implementation, this would query the simulation
    let contacts = vec![];
    
    Ok(Json(ContactsResponse { contacts }))
}

/// Get incoming threats
#[get("/v1/ships/<ship_id>/threats")]
pub fn get_threats(
    ship_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<ThreatsResponse>, Status> {
    let world = world.read().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // In a real implementation, this would query the simulation
    let threats = vec![];
    
    Ok(Json(ThreatsResponse { threats }))
}

/// Get navigation info to target
#[get("/v1/ships/<ship_id>/navigation/<target_id>")]
pub fn get_navigation(
    ship_id: String,
    target_id: String,
    world: &State<SharedGameWorld>,
) -> Result<Json<NavigationInfo>, Status> {
    let world = world.read().unwrap();
    
    if !world.ships().contains_key(&ship_id) {
        return Err(Status::NotFound);
    }
    
    // In a real implementation, this would calculate actual navigation data
    Ok(Json(NavigationInfo {
        target_id,
        heading: 0.0,
        distance: 0.0,
        eta: 0.0,
    }))
}

/// Deep analysis of target
#[post("/v1/ships/<ship_id>/analyze", data = "<request>")]
pub fn analyze_target(
    ship_id: String,
    request: Json<AnalyzeRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<AnalysisResponse>, Status> {
    let world_read = world.read().unwrap();
    
    let ship = world_read.ships().get(&ship_id)
        .ok_or(Status::NotFound)?;
    
    let ion_jammed = ship.status.is_ion_jammed();
    
    if ion_jammed {
        return Err(Status::BadRequest);
    }
    
    // Try to find target ship
    let target = world_read.ships().get(&request.target_id)
        .ok_or(Status::NotFound)?;
    
    let hull_pct = if target.status.max_hull > 0.0 {
        (target.status.hull / target.status.max_hull) * 100.0
    } else {
        0.0
    };
    
    let shields_pct = if target.status.max_shields > 0.0 {
        (target.status.shields / target.status.max_shields) * 100.0
    } else {
        0.0
    };
    
    Ok(Json(AnalysisResponse {
        target_id: target.id.clone(),
        class: target.class.clone(),
        faction: target.team_id.clone(),
        hull_percentage: hull_pct,
        shields_percentage: shields_pct,
    }))
}

/// Returns all routes for the science officer position
pub fn routes() -> Vec<Route> {
    routes![
        scan_target,
        get_contacts,
        get_threats,
        get_navigation,
        analyze_target
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
    fn test_scan_target() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let request = Json(ScanRequest {
            target_id: "enemy1".to_string(),
        });
        
        let result = scan_target("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(response.success);
        assert!(!response.ion_jammed);
    }
    
    #[test]
    fn test_scan_target_ion_jammed() {
        let world = setup_test_world();
        let mut ship = create_test_ship("ship1", "team1");
        ship.status.status_effects.push(StatusEffect {
            effect_type: StatusEffectType::Ion,
            duration: 5.0,
            magnitude: 1.0,
        });
        world.write().unwrap().add_ship(ship);
        
        let request = Json(ScanRequest {
            target_id: "enemy1".to_string(),
        });
        
        let result = scan_target("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert!(!response.success);
        assert!(response.ion_jammed);
    }
    
    #[test]
    fn test_get_contacts() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = get_contacts("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_get_threats() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = get_threats("ship1".to_string(), &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_get_navigation() {
        let world = setup_test_world();
        let ship = create_test_ship("ship1", "team1");
        world.write().unwrap().add_ship(ship);
        
        let result = get_navigation("ship1".to_string(), "target1".to_string(), &State::from(&world));
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_analyze_target() {
        let world = setup_test_world();
        let ship1 = create_test_ship("ship1", "team1");
        let ship2 = create_test_ship("ship2", "team2");
        world.write().unwrap().add_ship(ship1);
        world.write().unwrap().add_ship(ship2);
        
        let request = Json(AnalyzeRequest {
            target_id: "ship2".to_string(),
        });
        
        let result = analyze_target("ship1".to_string(), request, &State::from(&world));
        assert!(result.is_ok());
        
        let response = result.unwrap().into_inner();
        assert_eq!(response.target_id, "ship2");
    }
    
    #[test]
    fn test_analyze_target_ion_jammed() {
        let world = setup_test_world();
        let mut ship1 = create_test_ship("ship1", "team1");
        ship1.status.status_effects.push(StatusEffect {
            effect_type: StatusEffectType::Ion,
            duration: 5.0,
            magnitude: 1.0,
        });
        let ship2 = create_test_ship("ship2", "team2");
        world.write().unwrap().add_ship(ship1);
        world.write().unwrap().add_ship(ship2);
        
        let request = Json(AnalyzeRequest {
            target_id: "ship2".to_string(),
        });
        
        let result = analyze_target("ship1".to_string(), request, &State::from(&world));
        assert_eq!(result.err(), Some(Status::BadRequest));
    }
}
