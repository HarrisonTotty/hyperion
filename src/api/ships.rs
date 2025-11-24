//! Ship compilation API endpoints
//!
//! Provides REST API endpoints for compiling blueprints into active ships.

use rocket::{State, serde::json::Json, http::Status, post, get, routes};
use serde::{Deserialize, Serialize};
use crate::state::SharedGameWorld;
use crate::config::GameConfig;
use crate::compiler;

// ==================== Request/Response Types ====================

/// Request to compile and spawn a ship
#[derive(Debug, Deserialize, Serialize)]
pub struct CompileRequest {
    pub blueprint_id: String,
}

/// Response for ship compilation
#[derive(Debug, Serialize, Deserialize)]
pub struct CompileResponse {
    pub ship_id: String,
    pub name: String,
    pub class: String,
    pub team_id: String,
}

/// Response for ship list
#[derive(Debug, Serialize, Deserialize)]
pub struct ListShipsResponse {
    pub ships: Vec<ShipResponse>,
}

/// Response for a single ship
#[derive(Debug, Serialize, Deserialize)]
pub struct ShipResponse {
    pub id: String,
    pub name: String,
    pub class: String,
    pub team_id: String,
    pub hull: f32,
    pub max_hull: f32,
    pub shields: f32,
    pub max_shields: f32,
    pub power_generation: f32,
    pub power_capacity: f32,
    pub module_count: usize,
    pub weapon_count: usize,
}

// ==================== API Endpoints ====================

/// POST /v1/ships/compile - Compile a blueprint into an active ship
#[post("/v1/ships/compile", data = "<request>")]
pub fn compile_ship(
    request: Json<CompileRequest>,
    world: &State<SharedGameWorld>,
    config: &State<GameConfig>,
) -> Result<Json<CompileResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Compile and spawn ship
    let ship_id = compiler::compile_and_spawn(&request.blueprint_id, &mut world, config)
        .map_err(|_| Status::BadRequest)?;
    
    // Get the newly created ship
    let ship = world.get_ship(&ship_id)
        .ok_or(Status::InternalServerError)?;
    
    Ok(Json(CompileResponse {
        ship_id: ship.id.clone(),
        name: ship.name.clone(),
        class: ship.class.clone(),
        team_id: ship.team_id.clone(),
    }))
}

/// GET /v1/ships - List all active ships
#[get("/v1/ships")]
pub fn list_ships(world: &State<SharedGameWorld>) -> Json<ListShipsResponse> {
    let world = world.read().unwrap();
    let ships = world.get_all_ships()
        .iter()
        .map(|ship| ship_to_response(ship))
        .collect();
    
    Json(ListShipsResponse { ships })
}

/// GET /v1/ships/<id> - Get details of a specific ship
#[get("/v1/ships/<id>")]
pub fn get_ship(
    id: &str,
    world: &State<SharedGameWorld>,
) -> Result<Json<ShipResponse>, Status> {
    let world = world.read().unwrap();
    
    let ship = world.get_ship(id)
        .ok_or(Status::NotFound)?;
    
    Ok(Json(ship_to_response(ship)))
}

// ==================== Helper Functions ====================

fn ship_to_response(ship: &crate::models::Ship) -> ShipResponse {
    ShipResponse {
        id: ship.id.clone(),
        name: ship.name.clone(),
        class: ship.class.clone(),
        team_id: ship.team_id.clone(),
        hull: ship.status.hull,
        max_hull: ship.status.max_hull,
        shields: ship.status.shields,
        max_shields: ship.status.max_shields,
        power_generation: ship.status.power_generation,
        power_capacity: ship.status.power_capacity,
        module_count: ship.modules.len(),
        weapon_count: ship.weapons.len(),
    }
}

/// Aggregate all ship routes
pub fn routes() -> Vec<rocket::Route> {
    routes![
        compile_ship,
        list_ships,
        get_ship,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::blocking::Client;
    use rocket::Build;
    use crate::state::GameWorld;
    use crate::config::{GameConfig, ShipClassConfig, AiConfig, FactionsConfig, MapConfig, ModulesConfig, RacesConfig, SimulationConfig};

    fn create_test_config() -> GameConfig {
        use std::collections::HashMap;
        
        let mut ship_class = ShipClassConfig {
            name: "Test Cruiser".to_string(),
            description: "A test ship class".to_string(),
            base_hull: 1000.0,
            base_shields: 500.0,
            max_weight: 5000.0,
            max_modules: 10,
            size: crate::config::ShipSize::Medium,
            role: crate::config::ShipClassRole::Combat,
            build_points: 1000.0,
            bonuses: HashMap::new(),
            id: String::new(),
            manufacturers: HashMap::new(),
            length: None,
            width: None,
            height: None,
            mass: None,
            crew_min: None,
            crew_max: None,
            cargo_capacity: None,
            max_acceleration: None,
            max_turn_rate: None,
            max_warp_speed: None,
            warp_efficiency: None,
            sensor_range: None,
            operational_range: None,
            build_time: None,
            maintenance_cost: None,
            fuel_capacity: None,
            fuel_consumption: None,
            lore: None,
            year_introduced: None,
            notable_ships: vec![],
        };
        ship_class.set_id("test_cruiser".to_string());

        GameConfig {
            ai: AiConfig { difficulty: "medium".to_string(), response_time: 1.0 },
            factions: FactionsConfig { factions: vec![] },
            map: MapConfig { galaxy_size: 1000, star_density: 0.5 },
            modules: ModulesConfig { modules: HashMap::new() },
            races: RacesConfig { races: vec![] },
            simulation: SimulationConfig { tick_rate: 60.0, physics_enabled: true },
            ship_classes: vec![ship_class],
            module_definitions: vec![],
            weapon_definitions: vec![],
            ammunition_types: vec![],
            kinetic_weapon_kinds: vec![],
            ai_behavior: crate::config::AIConfig::default(),
            procedural_map: crate::config::ProceduralMapConfig::default(),
            simulation_params: crate::config::ProceduralSimConfig::default(),
            faction_generation: crate::config::FactionGenConfig::default(),
            module_variants: HashMap::new(),
            module_slots: HashMap::new(),
            bonuses: None,
        }
    }

    fn create_test_rocket() -> rocket::Rocket<Build> {
        let world = GameWorld::new_shared();
        let config = create_test_config();
        rocket::build()
            .manage(world)
            .manage(config)
            .mount("/", routes())
            .mount("/", crate::api::players::routes())
            .mount("/", crate::api::teams::routes())
            .mount("/", crate::api::blueprints::routes())
    }

    #[test]
    fn test_list_ships_empty() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        let response = client.get("/v1/ships").dispatch();
        assert_eq!(response.status(), Status::Ok);
        
        let ships: ListShipsResponse = response.into_json().unwrap();
        assert_eq!(ships.ships.len(), 0);
    }

    #[test]
    fn test_compile_ship() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Create team
        let team_response = client.post("/v1/teams")
            .json(&serde_json::json!({"name": "Alpha Team", "faction": "alliance"}))
            .dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap();
        
        // Create player
        let player_response = client.post("/v1/players")
            .json(&serde_json::json!({"name": "Commander"}))
            .dispatch();
        let player_json: serde_json::Value = player_response.into_json().unwrap();
        let player_id = player_json["id"].as_str().unwrap();
        
        // Add player to team
        client.patch(format!("/v1/teams/{}", team_id))
            .json(&serde_json::json!({"player_id": player_id}))
            .dispatch();
        
        // Create blueprint
        let blueprint_response = client.post("/v1/blueprints")
            .json(&serde_json::json!({
                "name": "Enterprise",
                "ship_class": "test_cruiser",
                "team_id": team_id
            }))
            .dispatch();
        let blueprint_json: serde_json::Value = blueprint_response.into_json().unwrap();
        let blueprint_id = blueprint_json["id"].as_str().unwrap();
        
        // Join blueprint
        client.post(format!("/v1/blueprints/{}/join", blueprint_id))
            .json(&serde_json::json!({"player_id": player_id}))
            .dispatch();
        
        // Assign roles
        client.patch(format!("/v1/blueprints/{}/roles", blueprint_id))
            .json(&serde_json::json!({
                "player_id": player_id,
                "roles": ["Captain"]
            }))
            .dispatch();
        
        // Mark ready
        client.post(format!("/v1/blueprints/{}/ready", blueprint_id))
            .json(&serde_json::json!({"player_id": player_id}))
            .dispatch();
        
        // Compile ship
        let compile_response = client.post("/v1/ships/compile")
            .json(&CompileRequest {
                blueprint_id: blueprint_id.to_string(),
            })
            .dispatch();
        
        assert_eq!(compile_response.status(), Status::Ok);
        let ship: CompileResponse = compile_response.into_json().unwrap();
        assert_eq!(ship.name, "Enterprise");
        assert_eq!(ship.class, "test_cruiser");
        
        // Verify ship appears in list
        let list_response = client.get("/v1/ships").dispatch();
        let ships: ListShipsResponse = list_response.into_json().unwrap();
        assert_eq!(ships.ships.len(), 1);
        assert_eq!(ships.ships[0].name, "Enterprise");
    }

    #[test]
    fn test_get_ship() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Set up and compile a ship (abbreviated version)
        let team_response = client.post("/v1/teams")
            .json(&serde_json::json!({"name": "Bravo Team", "faction": "alliance"}))
            .dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap();
        
        let player_response = client.post("/v1/players")
            .json(&serde_json::json!({"name": "Captain"}))
            .dispatch();
        let player_json: serde_json::Value = player_response.into_json().unwrap();
        let player_id = player_json["id"].as_str().unwrap();
        
        client.patch(format!("/v1/teams/{}", team_id))
            .json(&serde_json::json!({"player_id": player_id}))
            .dispatch();
        
        let blueprint_response = client.post("/v1/blueprints")
            .json(&serde_json::json!({
                "name": "Voyager",
                "ship_class": "test_cruiser",
                "team_id": team_id
            }))
            .dispatch();
        let blueprint_json: serde_json::Value = blueprint_response.into_json().unwrap();
        let blueprint_id = blueprint_json["id"].as_str().unwrap();
        
        client.post(format!("/v1/blueprints/{}/join", blueprint_id))
            .json(&serde_json::json!({"player_id": player_id}))
            .dispatch();
        
        client.patch(format!("/v1/blueprints/{}/roles", blueprint_id))
            .json(&serde_json::json!({
                "player_id": player_id,
                "roles": ["Captain"]
            }))
            .dispatch();
        
        client.post(format!("/v1/blueprints/{}/ready", blueprint_id))
            .json(&serde_json::json!({"player_id": player_id}))
            .dispatch();
        
        let compile_response = client.post("/v1/ships/compile")
            .json(&CompileRequest {
                blueprint_id: blueprint_id.to_string(),
            })
            .dispatch();
        let ship: CompileResponse = compile_response.into_json().unwrap();
        
        // Get ship by ID
        let get_response = client.get(format!("/v1/ships/{}", ship.ship_id)).dispatch();
        assert_eq!(get_response.status(), Status::Ok);
        
        let ship_details: ShipResponse = get_response.into_json().unwrap();
        assert_eq!(ship_details.name, "Voyager");
        assert_eq!(ship_details.max_hull, 1000.0);
        assert_eq!(ship_details.max_shields, 500.0);
    }

    #[test]
    fn test_get_ship_not_found() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        let response = client.get("/v1/ships/nonexistent").dispatch();
        assert_eq!(response.status(), Status::NotFound);
    }
}
