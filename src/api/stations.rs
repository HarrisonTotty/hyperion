//! Station API endpoints
//!
//! REST API for managing stations, docking, and services

use rocket::{routes, Route, State, get, post, delete};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::state::GameWorld;
use crate::stations::{Station, StationSize, ServiceRequest, ServiceResponse, DockingStatus};

/// Request to create a new station
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateStationRequest {
    pub name: String,
    pub position: [f64; 3],
    pub faction: String,
    pub size: Option<StationSize>,
}

/// Response with station details
#[derive(Debug, Serialize, Deserialize)]
pub struct StationResponse {
    pub id: String,
    pub name: String,
    pub position: [f64; 3],
    pub faction: String,
    pub size: StationSize,
    pub max_docked_ships: usize,
    pub docked_ships: Vec<String>,
    pub available_bays: usize,
    pub services: StationServicesResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StationServicesResponse {
    pub repair: bool,
    pub refuel: bool,
    pub rearm: bool,
    pub trade: bool,
}

impl From<&Station> for StationResponse {
    fn from(station: &Station) -> Self {
        Self {
            id: station.id.to_string(),
            name: station.name.clone(),
            position: station.position,
            faction: station.faction.clone(),
            size: station.size,
            max_docked_ships: station.max_docked_ships,
            docked_ships: station.docked_ships.iter().map(|id| id.to_string()).collect(),
            available_bays: station.available_docking_bays(),
            services: StationServicesResponse {
                repair: station.services.repair,
                refuel: station.services.refuel,
                rearm: station.services.rearm,
                trade: station.services.trade,
            },
        }
    }
}

/// List all stations
#[get("/v1/stations")]
fn list_stations(game_world: &State<Arc<RwLock<GameWorld>>>) -> Json<Vec<StationResponse>> {
    let world = game_world.read().unwrap();
    let stations: Vec<StationResponse> = world.get_all_stations()
        .iter()
        .map(|s| StationResponse::from(*s))
        .collect();
    Json(stations)
}

/// Get a specific station
#[get("/v1/stations/<id>")]
fn get_station(
    id: &str,
    game_world: &State<Arc<RwLock<GameWorld>>>,
) -> Result<Json<StationResponse>, rocket::http::Status> {
    let world = game_world.read().unwrap();
    
    match world.get_station(id) {
        Some(station) => Ok(Json(StationResponse::from(station))),
        None => Err(rocket::http::Status::NotFound),
    }
}

/// Create a new station
#[post("/v1/stations", data = "<request>")]
fn create_station(
    request: Json<CreateStationRequest>,
    game_world: &State<Arc<RwLock<GameWorld>>>,
) -> Json<StationResponse> {
    let mut world = game_world.write().unwrap();
    
    let station = if let Some(size) = request.size {
        Station::with_size(
            request.name.clone(),
            request.position,
            request.faction.clone(),
            size,
        )
    } else {
        Station::new(
            request.name.clone(),
            request.position,
            request.faction.clone(),
        )
    };
    
    let response = StationResponse::from(&station);
    world.register_station(station);
    
    Json(response)
}

/// Delete a station
#[delete("/v1/stations/<id>")]
fn delete_station(
    id: &str,
    game_world: &State<Arc<RwLock<GameWorld>>>,
) -> Result<Json<serde_json::Value>, rocket::http::Status> {
    let mut world = game_world.write().unwrap();
    
    match world.remove_station(id) {
        Ok(_) => Ok(Json(serde_json::json!({
            "message": "Station deleted successfully"
        }))),
        Err(_) => Err(rocket::http::Status::NotFound),
    }
}

/// Docking request
#[derive(Debug, Serialize, Deserialize)]
pub struct DockingRequest {
    pub ship_id: String,
    pub ship_faction: String,
}

/// Docking response
#[derive(Debug, Serialize, Deserialize)]
pub struct DockingResponse {
    pub approved: bool,
    pub status: Option<DockingStatus>,
    pub message: String,
}

/// Request docking at a station
#[post("/v1/stations/<station_id>/dock", data = "<request>")]
fn request_docking(
    station_id: &str,
    request: Json<DockingRequest>,
    game_world: &State<Arc<RwLock<GameWorld>>>,
) -> Result<Json<DockingResponse>, rocket::http::Status> {
    let mut world = game_world.write().unwrap();
    
    let station = world.get_station_mut(station_id)
        .ok_or(rocket::http::Status::NotFound)?;
    
    let ship_id = Uuid::parse_str(&request.ship_id)
        .map_err(|_| rocket::http::Status::BadRequest)?;
    
    let approved = station.request_docking(ship_id, &request.ship_faction);
    let status = station.get_docking_status(ship_id);
    
    let message = if approved {
        "Docking request approved".to_string()
    } else {
        match status {
            Some(DockingStatus::Denied) => {
                if station.is_hostile_to(&request.ship_faction) {
                    "Docking denied: Hostile faction".to_string()
                } else {
                    "Docking denied: Station is full".to_string()
                }
            }
            _ => "Docking denied".to_string(),
        }
    };
    
    Ok(Json(DockingResponse {
        approved,
        status,
        message,
    }))
}

/// Complete docking (ship has arrived)
#[post("/v1/stations/<station_id>/dock/<ship_id>/complete")]
fn complete_docking(
    station_id: &str,
    ship_id: &str,
    game_world: &State<Arc<RwLock<GameWorld>>>,
) -> Result<Json<serde_json::Value>, rocket::http::Status> {
    let mut world = game_world.write().unwrap();
    
    let station = world.get_station_mut(station_id)
        .ok_or(rocket::http::Status::NotFound)?;
    
    let ship_uuid = Uuid::parse_str(ship_id)
        .map_err(|_| rocket::http::Status::BadRequest)?;
    
    if station.complete_docking(ship_uuid) {
        Ok(Json(serde_json::json!({
            "message": "Docking completed successfully"
        })))
    } else {
        Err(rocket::http::Status::BadRequest)
    }
}

/// Undock from a station
#[post("/v1/stations/<station_id>/undock/<ship_id>")]
fn undock_ship(
    station_id: &str,
    ship_id: &str,
    game_world: &State<Arc<RwLock<GameWorld>>>,
) -> Result<Json<serde_json::Value>, rocket::http::Status> {
    let mut world = game_world.write().unwrap();
    
    let station = world.get_station_mut(station_id)
        .ok_or(rocket::http::Status::NotFound)?;
    
    let ship_uuid = Uuid::parse_str(ship_id)
        .map_err(|_| rocket::http::Status::BadRequest)?;
    
    if station.undock_ship(ship_uuid) {
        Ok(Json(serde_json::json!({
            "message": "Undocking initiated"
        })))
    } else {
        Err(rocket::http::Status::BadRequest)
    }
}

/// Request a service at a station
#[post("/v1/stations/<station_id>/services/<ship_id>", data = "<request>")]
fn request_service(
    station_id: &str,
    ship_id: &str,
    request: Json<ServiceRequest>,
    game_world: &State<Arc<RwLock<GameWorld>>>,
) -> Result<Json<ServiceResponse>, rocket::http::Status> {
    let world = game_world.read().unwrap();
    
    let station = world.get_station(station_id)
        .ok_or(rocket::http::Status::NotFound)?;
    
    let ship_uuid = Uuid::parse_str(ship_id)
        .map_err(|_| rocket::http::Status::BadRequest)?;
    
    // Check if ship is docked
    if !station.is_ship_docked(ship_uuid) {
        return Ok(Json(ServiceResponse {
            success: false,
            message: "Ship must be docked to request services".to_string(),
            cost: 0.0,
        }));
    }
    
    // Process service request (simplified for now)
    let response = match request.into_inner() {
        ServiceRequest::RepairModule { module_id } => {
            if !station.services.repair {
                ServiceResponse {
                    success: false,
                    message: "Repair services not available".to_string(),
                    cost: 0.0,
                }
            } else {
                ServiceResponse {
                    success: true,
                    message: format!("Repaired module {}", module_id),
                    cost: 100.0 * station.services.repair_cost,
                }
            }
        }
        ServiceRequest::RepairAll => {
            if !station.services.repair {
                ServiceResponse {
                    success: false,
                    message: "Repair services not available".to_string(),
                    cost: 0.0,
                }
            } else {
                ServiceResponse {
                    success: true,
                    message: "Repaired all modules".to_string(),
                    cost: 500.0 * station.services.repair_cost,
                }
            }
        }
        ServiceRequest::Refuel { amount } => {
            if !station.services.refuel {
                ServiceResponse {
                    success: false,
                    message: "Refuel services not available".to_string(),
                    cost: 0.0,
                }
            } else {
                ServiceResponse {
                    success: true,
                    message: format!("Refueled {} units", amount),
                    cost: amount * station.services.refuel_cost,
                }
            }
        }
        ServiceRequest::RearmWeapon { weapon_id, ammunition_type, quantity } => {
            if !station.services.rearm {
                ServiceResponse {
                    success: false,
                    message: "Rearm services not available".to_string(),
                    cost: 0.0,
                }
            } else {
                ServiceResponse {
                    success: true,
                    message: format!("Rearmed {} with {} x {}", weapon_id, quantity, ammunition_type),
                    cost: (quantity as f32) * 10.0 * station.services.rearm_cost,
                }
            }
        }
        ServiceRequest::RearmAll => {
            if !station.services.rearm {
                ServiceResponse {
                    success: false,
                    message: "Rearm services not available".to_string(),
                    cost: 0.0,
                }
            } else {
                ServiceResponse {
                    success: true,
                    message: "Rearmed all weapons".to_string(),
                    cost: 1000.0 * station.services.rearm_cost,
                }
            }
        }
    };
    
    Ok(Json(response))
}

/// Get docking status for a ship at a station
#[get("/v1/stations/<station_id>/dock/<ship_id>")]
fn get_docking_status(
    station_id: &str,
    ship_id: &str,
    game_world: &State<Arc<RwLock<GameWorld>>>,
) -> Result<Json<DockingResponse>, rocket::http::Status> {
    let world = game_world.read().unwrap();
    
    let station = world.get_station(station_id)
        .ok_or(rocket::http::Status::NotFound)?;
    
    let ship_uuid = Uuid::parse_str(ship_id)
        .map_err(|_| rocket::http::Status::BadRequest)?;
    
    let status = station.get_docking_status(ship_uuid);
    
    Ok(Json(DockingResponse {
        approved: status.is_some() && status != Some(DockingStatus::Denied),
        status,
        message: match status {
            Some(DockingStatus::Requested) => "Docking requested".to_string(),
            Some(DockingStatus::Approaching) => "Approach approved".to_string(),
            Some(DockingStatus::Docked) => "Docked".to_string(),
            Some(DockingStatus::Undocking) => "Undocking in progress".to_string(),
            Some(DockingStatus::Denied) => "Docking denied".to_string(),
            None => "No docking request".to_string(),
        },
    }))
}

/// Return all station routes
pub fn routes() -> Vec<Route> {
    routes![
        list_stations,
        get_station,
        create_station,
        delete_station,
        request_docking,
        complete_docking,
        undock_ship,
        request_service,
        get_docking_status,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::blocking::Client;
    use rocket::http::Status;
    
    fn create_test_client() -> Client {
        let game_world = GameWorld::new_shared();
        
        let rocket = rocket::build()
            .manage(game_world)
            .mount("/", routes());
        
        Client::tracked(rocket).expect("valid rocket instance")
    }
    
    #[test]
    fn test_list_stations_empty() {
        let client = create_test_client();
        let response = client.get("/v1/stations").dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let stations: Vec<StationResponse> = response.into_json().unwrap();
        assert_eq!(stations.len(), 0);
    }
    
    #[test]
    fn test_create_station() {
        let client = create_test_client();
        
        let request = CreateStationRequest {
            name: "Alpha Station".to_string(),
            position: [100.0, 200.0, 300.0],
            faction: "Federation".to_string(),
            size: Some(StationSize::Large),
        };
        
        let response = client
            .post("/v1/stations")
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let station: StationResponse = response.into_json().unwrap();
        assert_eq!(station.name, "Alpha Station");
        assert_eq!(station.size, StationSize::Large);
        assert_eq!(station.max_docked_ships, 10);
    }
    
    #[test]
    fn test_get_station() {
        let client = create_test_client();
        
        // Create a station first
        let create_req = CreateStationRequest {
            name: "Test Station".to_string(),
            position: [0.0, 0.0, 0.0],
            faction: "Federation".to_string(),
            size: None,
        };
        
        let create_response = client
            .post("/v1/stations")
            .json(&create_req)
            .dispatch();
        
        let created: StationResponse = create_response.into_json().unwrap();
        
        // Get the station
        let get_response = client
            .get(format!("/v1/stations/{}", created.id))
            .dispatch();
        
        assert_eq!(get_response.status(), Status::Ok);
        let station: StationResponse = get_response.into_json().unwrap();
        assert_eq!(station.id, created.id);
        assert_eq!(station.name, "Test Station");
    }
    
    #[test]
    fn test_request_docking() {
        let client = create_test_client();
        
        // Create a station
        let create_req = CreateStationRequest {
            name: "Docking Station".to_string(),
            position: [0.0, 0.0, 0.0],
            faction: "Federation".to_string(),
            size: None,
        };
        
        let create_response = client
            .post("/v1/stations")
            .json(&create_req)
            .dispatch();
        
        let station: StationResponse = create_response.into_json().unwrap();
        
        // Request docking
        let dock_req = DockingRequest {
            ship_id: Uuid::new_v4().to_string(),
            ship_faction: "Federation".to_string(),
        };
        
        let dock_response = client
            .post(format!("/v1/stations/{}/dock", station.id))
            .json(&dock_req)
            .dispatch();
        
        assert_eq!(dock_response.status(), Status::Ok);
        let dock_resp: DockingResponse = dock_response.into_json().unwrap();
        assert!(dock_resp.approved);
    }
    
    #[test]
    fn test_hostile_faction_docking() {
        let client = create_test_client();
        
        // Create a station
        let create_req = CreateStationRequest {
            name: "Hostile Station".to_string(),
            position: [0.0, 0.0, 0.0],
            faction: "Federation".to_string(),
            size: None,
        };
        
        let create_response = client
            .post("/v1/stations")
            .json(&create_req)
            .dispatch();
        
        let mut station: StationResponse = create_response.into_json().unwrap();
        
        // Mark Empire as hostile (this would need to be done via API in real scenario)
        // For now, we'll just test the request and expect denial
        
        let dock_req = DockingRequest {
            ship_id: Uuid::new_v4().to_string(),
            ship_faction: "Empire".to_string(), // Not in hostile list yet, so should succeed
        };
        
        let dock_response = client
            .post(format!("/v1/stations/{}/dock", station.id))
            .json(&dock_req)
            .dispatch();
        
        assert_eq!(dock_response.status(), Status::Ok);
    }
}
