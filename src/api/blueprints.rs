//! Blueprint API endpoints
//!
//! Provides REST API endpoints for managing ship blueprints.

use rocket::{State, serde::json::Json, http::Status, get, post, patch, delete, routes};
use serde::{Deserialize, Serialize};
use crate::state::SharedGameWorld;
use crate::models::role::ShipRole;
use crate::config::GameConfig;
use crate::blueprint::BlueprintValidator;

// ==================== Request/Response Types ====================

/// Request to create a new ship blueprint
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateBlueprintRequest {
    pub name: String,
    pub ship_class: String,
    pub team_id: String,
}

/// Request to join an existing blueprint
#[derive(Debug, Deserialize, Serialize)]
pub struct JoinBlueprintRequest {
    pub player_id: String,
}

/// Request to update player roles
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateRolesRequest {
    pub player_id: String,
    pub roles: Vec<ShipRole>,
}

/// Request to add a module
#[derive(Debug, Deserialize, Serialize)]
pub struct AddModuleRequest {
    /// Module slot type ID to add
    pub module_slot_id: String,
    /// Optional: variant to select immediately
    pub variant_id: Option<String>,
}

/// Request to configure a module
#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigureModuleRequest {
    pub kind: Option<String>,
}

/// Request to mark player as ready
#[derive(Debug, Deserialize, Serialize)]
pub struct ReadyRequest {
    pub player_id: String,
}

/// Response for a single blueprint
#[derive(Debug, Serialize, Deserialize)]
pub struct BlueprintResponse {
    pub id: String,
    pub name: String,
    pub class: String,
    pub team_id: String,
    pub crew: Vec<CrewAssignment>,
    pub player_roles: Vec<PlayerRoleInfo>,
    pub modules: Vec<ModuleInfo>,
    pub weapons: Vec<WeaponInfo>,
    pub ready_players: Vec<String>,
    pub all_ready: bool,
}

/// Crew assignment for frontend compatibility
#[derive(Debug, Serialize, Deserialize)]
pub struct CrewAssignment {
    pub player_id: String,
    pub role: String,
    pub ready: bool,
}

/// Player role information
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerRoleInfo {
    pub player_id: String,
    pub roles: Vec<ShipRole>,
}

/// Module instance information
#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub id: String,
    pub module_slot_id: String,
    pub variant_id: Option<String>,
}

/// Weapon instance information
#[derive(Debug, Serialize, Deserialize)]
pub struct WeaponInfo {
    pub id: String,
    pub weapon_id: String,
    pub kind: Option<String>,
    pub loaded_ammunition: Option<String>,
}

/// Response for blueprint list
#[derive(Debug, Serialize, Deserialize)]
pub struct ListBlueprintsResponse {
    pub blueprints: Vec<BlueprintResponse>,
}

/// Response for blueprint validation
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResponse {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

// ==================== Helper Functions ====================

/// Convert a ShipBlueprint to a BlueprintResponse
fn blueprint_to_response(bp: &crate::models::ShipBlueprint) -> BlueprintResponse {
    // Convert player_roles HashMap to crew assignments
    let mut crew = Vec::new();
    for (player_id, roles) in &bp.player_roles {
        for role in roles {
            let role_str = format!("{:?}", role).to_lowercase();
            crew.push(CrewAssignment {
                player_id: player_id.clone(),
                role: role_str,
                ready: bp.ready_players.contains(player_id),
            });
        }
    }

    BlueprintResponse {
        id: bp.id.clone(),
        name: bp.name.clone(),
        class: bp.class.clone(),
        team_id: bp.team_id.clone(),
        crew,
        player_roles: bp.player_roles.iter()
            .map(|(pid, roles)| PlayerRoleInfo {
                player_id: pid.clone(),
                roles: roles.clone(),
            })
            .collect(),
        modules: bp.modules.iter()
            .map(|m| ModuleInfo {
                id: m.id.clone(),
                module_slot_id: m.module_slot_id.clone(),
                variant_id: m.variant_id.clone(),
            })
            .collect(),
        weapons: bp.weapons.iter()
            .map(|w| WeaponInfo {
                id: w.id.clone(),
                weapon_id: w.weapon_id.clone(),
                kind: w.kind.clone(),
                loaded_ammunition: w.loaded_ammunition.clone(),
            })
            .collect(),
        ready_players: bp.ready_players.iter().cloned().collect(),
        all_ready: bp.all_players_ready(),
    }
}

// ==================== API Endpoints ====================

/// GET /v1/blueprints - List all ship blueprints
#[get("/v1/blueprints")]
pub fn list_blueprints(world: &State<SharedGameWorld>) -> Json<ListBlueprintsResponse> {
    let world = world.read().unwrap();
    let blueprints = world.get_all_blueprints()
        .into_iter()
        .map(blueprint_to_response)
        .collect();
    
    Json(ListBlueprintsResponse { blueprints })
}

/// POST /v1/blueprints - Create new ship blueprint
#[post("/v1/blueprints", data = "<request>")]
pub fn create_blueprint(
    request: Json<CreateBlueprintRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<BlueprintResponse>, Status> {
    let mut world = world.write().unwrap();
    
    // Validate ship name
    if request.name.is_empty() || request.name.len() > 50 {
        return Err(Status::BadRequest);
    }
    
    // Create blueprint
    let blueprint_id = world.create_blueprint(
        request.name.clone(),
        request.ship_class.clone(),
        request.team_id.clone(),
    ).map_err(|_| Status::BadRequest)?;
    
    let blueprint = world.get_blueprint(&blueprint_id).unwrap();
    Ok(Json(blueprint_to_response(blueprint)))
}

/// GET /v1/blueprints/<id> - Get blueprint details
#[get("/v1/blueprints/<id>")]
pub fn get_blueprint(
    id: &str,
    world: &State<SharedGameWorld>,
) -> Result<Json<BlueprintResponse>, Status> {
    let world = world.read().unwrap();
    
    let blueprint = world.get_blueprint(id)
        .ok_or(Status::NotFound)?;
    
    Ok(Json(blueprint_to_response(blueprint)))
}

/// POST /v1/blueprints/<id>/join - Join existing blueprint
#[post("/v1/blueprints/<id>/join", data = "<request>")]
pub fn join_blueprint(
    id: &str,
    request: Json<JoinBlueprintRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<BlueprintResponse>, Status> {
    // Verify player exists
        {
            let world_guard = world.read().unwrap();
            if world_guard.get_player(&request.player_id).is_none() {
                return Err(Status::BadRequest);
            }
        }
        let mut game_world = world.write().unwrap();

    // Get blueprint and add player with empty roles
        let blueprint = game_world.get_blueprint_mut(id)
        .ok_or(Status::NotFound)?;

    blueprint.set_player_roles(request.player_id.clone(), vec![]);

    let blueprint = game_world.get_blueprint(id).unwrap();
    Ok(Json(blueprint_to_response(blueprint)))
}

/// PATCH /v1/blueprints/<id>/roles - Update player roles
#[patch("/v1/blueprints/<id>/roles", data = "<request>")]
pub fn update_roles(
    id: &str,
    request: Json<UpdateRolesRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<BlueprintResponse>, Status> {
    eprintln!("Received role update request: player_id={}, roles={:?}", request.player_id, request.roles);
    
        let mut game_world = world.write().unwrap();
    
    // Verify player exists
        {
            let world_guard = world.read().unwrap();
            if world_guard.get_player(&request.player_id).is_none() {
                eprintln!("Player not found: {}", request.player_id);
                return Err(Status::BadRequest);
            }
        }
    
        let blueprint = game_world.get_blueprint_mut(id)
        .ok_or_else(|| {
            eprintln!("Blueprint not found: {}", id);
            Status::NotFound
        })?;
    
    // Add player if not already in blueprint (auto-join)
    if !blueprint.player_roles.contains_key(&request.player_id) {
        eprintln!("Auto-adding player to blueprint: {}", request.player_id);
        blueprint.set_player_roles(request.player_id.clone(), vec![]);
    }
    
    eprintln!("Setting player roles: player_id={}, roles={:?}", request.player_id, request.roles);
    blueprint.set_player_roles(request.player_id.clone(), request.roles.clone());
    
    let blueprint = game_world.get_blueprint(id).unwrap();
    Ok(Json(blueprint_to_response(blueprint)))
}

/// POST /v1/blueprints/<id>/modules - Add module to ship
#[post("/v1/blueprints/<id>/modules", data = "<request>")]
pub fn add_module(
    id: &str,
    request: Json<AddModuleRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<BlueprintResponse>, Status> {
    let mut world = world.write().unwrap();
    
    let blueprint = world.get_blueprint_mut(id)
        .ok_or(Status::NotFound)?;
    
    // Create module instance
    let module = crate::models::blueprint::ModuleInstance {
        id: uuid::Uuid::new_v4().to_string(),
        module_slot_id: request.module_slot_id.clone(),
        variant_id: request.variant_id.clone(),
    };
    blueprint.modules.push(module);
    
    let blueprint = world.get_blueprint(id).unwrap();
    Ok(Json(blueprint_to_response(blueprint)))
}

/// DELETE /v1/blueprints/<id>/modules/<module_id> - Remove module
#[delete("/v1/blueprints/<id>/modules/<module_id>")]
pub fn remove_module(
    id: &str,
    module_id: &str,
    world: &State<SharedGameWorld>,
) -> Result<Status, Status> {
    let mut world = world.write().unwrap();
    
    let blueprint = world.get_blueprint_mut(id)
        .ok_or(Status::NotFound)?;
    
    let initial_len = blueprint.modules.len();
    blueprint.modules.retain(|m| m.id != module_id);
    
    if blueprint.modules.len() == initial_len {
        return Err(Status::NotFound);
    }
    
    Ok(Status::NoContent)
}

/// PATCH /v1/blueprints/<id>/modules/<module_id> - Configure module
#[patch("/v1/blueprints/<id>/modules/<module_id>", data = "<request>")]
pub fn configure_module(
    id: &str,
    module_id: &str,
    request: Json<ConfigureModuleRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<BlueprintResponse>, Status> {
    let mut world = world.write().unwrap();
    
    let blueprint = world.get_blueprint_mut(id)
        .ok_or(Status::NotFound)?;
    
    let module = blueprint.modules.iter_mut()
        .find(|m| m.id == module_id)
        .ok_or(Status::NotFound)?;
    
    // This endpoint is deprecated; use update_module_variant instead
    // For backward compatibility, do nothing or log a warning
    
    let blueprint = world.get_blueprint(id).unwrap();
    Ok(Json(blueprint_to_response(blueprint)))
}

/// POST /v1/blueprints/<id>/ready - Mark player as ready
#[post("/v1/blueprints/<id>/ready", data = "<request>")]
pub fn mark_ready(
    id: &str,
    request: Json<ReadyRequest>,
    world: &State<SharedGameWorld>,
) -> Result<Json<BlueprintResponse>, Status> {
    let mut world = world.write().unwrap();
    
    let blueprint = world.get_blueprint_mut(id)
        .ok_or(Status::NotFound)?;
    
    // Verify player is part of this blueprint
    if !blueprint.player_roles.contains_key(&request.player_id) {
        return Err(Status::BadRequest);
    }
    
    blueprint.mark_ready(request.player_id.clone());
    
    let blueprint = world.get_blueprint(id).unwrap();
    Ok(Json(blueprint_to_response(blueprint)))
}

/// DELETE /v1/blueprints/<id>/ready - Unmark player as ready
#[delete("/v1/blueprints/<id>/ready/<player_id>")]
pub fn unmark_ready(
    id: &str,
    player_id: &str,
    world: &State<SharedGameWorld>,
) -> Result<Json<BlueprintResponse>, Status> {
    let mut world = world.write().unwrap();
    
    let blueprint = world.get_blueprint_mut(id)
        .ok_or(Status::NotFound)?;
    
    blueprint.unmark_ready(player_id);
    
    let blueprint = world.get_blueprint(id).unwrap();
    Ok(Json(blueprint_to_response(blueprint)))
}

/// GET /v1/blueprints/<id>/validate - Validate blueprint completeness
#[get("/v1/blueprints/<id>/validate")]
pub fn validate_blueprint(
    id: &str,
    world: &State<SharedGameWorld>,
    config: &State<GameConfig>,
) -> Result<Json<ValidationResponse>, Status> {
    let world_lock = world.read().unwrap();
    
    let blueprint = world_lock.get_blueprint(id)
        .ok_or(Status::NotFound)?;
    
    // Create validator
    let validator = BlueprintValidator::new(
        config,
        world_lock.players(),
        world_lock.teams(),
    );
    
    // Perform validation
    let result = validator.validate(blueprint);
    
    // Convert errors and warnings to strings
    let errors: Vec<String> = result.errors.iter()
        .map(|e| format!("{:?}", e))
        .collect();
    
    let warnings: Vec<String> = result.warnings.iter()
        .map(|w| format!("{:?}", w))
        .collect();
    
    Ok(Json(ValidationResponse {
        valid: result.is_valid,
        errors,
        warnings,
    }))
}

/// Aggregate all blueprint routes
pub fn routes() -> Vec<rocket::Route> {
    routes![
        list_blueprints,
        create_blueprint,
        get_blueprint,
        join_blueprint,
        update_roles,
        add_module,
        remove_module,
        configure_module,
        mark_ready,
        unmark_ready,
        validate_blueprint,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::blocking::Client;
    use rocket::Build;
    use crate::state::GameWorld;
    use crate::config::{GameConfig, AiConfig, FactionsConfig, MapConfig, ModulesConfig, RacesConfig, SimulationConfig};

    fn create_test_config() -> GameConfig {
        use std::collections::HashMap;
        
        GameConfig {
            ai: AiConfig { difficulty: "medium".to_string(), response_time: 1.0 },
            // Provide test factions used by blueprint tests (e.g. "alliance")
            factions: FactionsConfig { factions: vec![
                crate::config::Faction { id: "alliance".to_string(), name: "Alliance".to_string(), description: "Test faction".to_string() },
                crate::config::Faction { id: "federation".to_string(), name: "Federation".to_string(), description: "Test faction".to_string() },
            ] },
            map: MapConfig { galaxy_size: 1000, star_density: 0.5 },
            modules: ModulesConfig { modules: HashMap::new() },
            races: RacesConfig { races: vec![] },
            simulation: SimulationConfig { tick_rate: 60.0, physics_enabled: true },
            ship_classes: vec![],
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
            .mount("/", crate::api::routes![
                list_blueprints,
                create_blueprint,
                get_blueprint,
                join_blueprint,
                update_roles,
                add_module,
                remove_module,
                configure_module,
                mark_ready,
                unmark_ready,
                validate_blueprint,
            ])
            .mount("/", crate::api::players::routes())
            .mount("/", crate::api::teams::routes())
    }

    #[test]
    fn test_list_blueprints_empty() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        let response = client.get("/v1/blueprints").dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let body: ListBlueprintsResponse = response.into_json().unwrap();
        assert_eq!(body.blueprints.len(), 0);
    }

    #[test]
    fn test_create_blueprint() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Create a team first
        let team_request = serde_json::json!({
            "name": "Alpha Team",
            "faction": "alliance"
        });
        let team_response = client.post("/v1/teams")
            .json(&team_request)
            .dispatch();
        assert_eq!(team_response.status(), Status::Ok);
        
        // Parse as generic JSON first to see what we got
        let team_json: serde_json::Value = team_response.into_json().expect("Failed to parse JSON");
        let team_id = team_json["id"].as_str().expect("No id field").to_string();
        // Create blueprint
        let blueprint_request = CreateBlueprintRequest {
            name: "USS Enterprise".to_string(),
            ship_class: "cruiser".to_string(),
            team_id: team_id.clone(),
        };
        
        let response = client.post("/v1/blueprints")
            .json(&blueprint_request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let blueprint: BlueprintResponse = response.into_json().unwrap();
        assert_eq!(blueprint.name, "USS Enterprise");
        assert_eq!(blueprint.class, "cruiser");
        assert_eq!(blueprint.team_id, team_id);
    }

    #[test]
    fn test_get_blueprint() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Create team and blueprint
        let team_request = serde_json::json!({"name": "Beta Team", "faction": "alliance"});
        let team_response = client.post("/v1/teams").json(&team_request).dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap().to_string();
        
        let blueprint_request = CreateBlueprintRequest {
            name: "Defiant".to_string(),
            ship_class: "destroyer".to_string(),
            team_id: team_id.clone(),
        };
        let create_response = client.post("/v1/blueprints").json(&blueprint_request).dispatch();
        let created: BlueprintResponse = create_response.into_json().unwrap();
        
        // Get blueprint
        let response = client.get(format!("/v1/blueprints/{}", created.id)).dispatch();
        assert_eq!(response.status(), Status::Ok);
        let blueprint: BlueprintResponse = response.into_json().unwrap();
        assert_eq!(blueprint.id, created.id);
        assert_eq!(blueprint.name, "Defiant");
    }

    #[test]
    fn test_join_blueprint() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Create player, team, and blueprint
        let player_request = serde_json::json!({"name": "Kirk"});
        let player_response = client.post("/v1/players").json(&player_request).dispatch();
        let player_json: serde_json::Value = player_response.into_json().unwrap();
        let player_id = player_json["id"].as_str().unwrap().to_string();
        
        let team_request = serde_json::json!({"name": "Gamma Team", "faction": "alliance"});
        let team_response = client.post("/v1/teams").json(&team_request).dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap().to_string();
        
        let blueprint_request = CreateBlueprintRequest {
            name: "Voyager".to_string(),
            ship_class: "cruiser".to_string(),
            team_id: team_id.clone(),
        };
        let create_response = client.post("/v1/blueprints").json(&blueprint_request).dispatch();
        let blueprint: BlueprintResponse = create_response.into_json().unwrap();
        
        // Join blueprint
        let join_request = JoinBlueprintRequest {
            player_id: player_id.clone(),
        };
        let response = client.post(format!("/v1/blueprints/{}/join", blueprint.id))
            .json(&join_request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let updated: BlueprintResponse = response.into_json().unwrap();
        assert_eq!(updated.player_roles.len(), 1);
        assert_eq!(updated.player_roles[0].player_id, player_id);
    }

    #[test]
    fn test_update_roles() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Setup: player, team, blueprint, and join
        let player_response = client.post("/v1/players")
            .json(&serde_json::json!({"name": "Picard"}))
            .dispatch();
        let player_json: serde_json::Value = player_response.into_json().unwrap();
        let player_id = player_json["id"].as_str().unwrap().to_string();
        
        let team_response = client.post("/v1/teams")
            .json(&serde_json::json!({"name": "Delta Team", "faction": "alliance"}))
            .dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap().to_string();
        
        let blueprint_response = client.post("/v1/blueprints")
            .json(&CreateBlueprintRequest {
                name: "Enterprise-D".to_string(),
                ship_class: "battleship".to_string(),
                team_id: team_id.clone(),
            })
            .dispatch();
        let blueprint: BlueprintResponse = blueprint_response.into_json().unwrap();
        
        client.post(format!("/v1/blueprints/{}/join", blueprint.id))
            .json(&JoinBlueprintRequest { player_id: player_id.clone() })
            .dispatch();
        
        // Update roles
        let update_request = UpdateRolesRequest {
            player_id: player_id.clone(),
            roles: vec![ShipRole::Captain, ShipRole::Helm],
        };
        let response = client.patch(format!("/v1/blueprints/{}/roles", blueprint.id))
            .json(&update_request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let updated: BlueprintResponse = response.into_json().unwrap();
        assert_eq!(updated.player_roles[0].roles.len(), 2);
    }

    #[test]
    fn test_add_and_remove_module() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Create team and blueprint
        let team_response = client.post("/v1/teams")
            .json(&serde_json::json!({"name": "Epsilon Team", "faction": "alliance"}))
            .dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap().to_string();
        
        let blueprint_response = client.post("/v1/blueprints")
            .json(&CreateBlueprintRequest {
                name: "Discovery".to_string(),
                ship_class: "corvette".to_string(),
                team_id: team_id.clone(),
            })
            .dispatch();
        let blueprint: BlueprintResponse = blueprint_response.into_json().unwrap();
        
        // Add module
        let add_request = AddModuleRequest {
            module_slot_id: "power_core_1".to_string(),
            variant_id: Some("fusion".to_string()),
        };
        let response = client.post(format!("/v1/blueprints/{}/modules", blueprint.id))
            .json(&add_request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let updated: BlueprintResponse = response.into_json().unwrap();
        assert_eq!(updated.modules.len(), 1);
        
        let module_id = updated.modules[0].id.clone();
        
        // Remove module
        let response = client.delete(format!("/v1/blueprints/{}/modules/{}", blueprint.id, module_id))
            .dispatch();
        assert_eq!(response.status(), Status::NoContent);
    }

    #[test]
    fn test_configure_module() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Create team, blueprint, and add module
        let team_response = client.post("/v1/teams")
            .json(&serde_json::json!({"name": "Zeta Team", "faction": "alliance"}))
            .dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap().to_string();
        
        let blueprint_response = client.post("/v1/blueprints")
            .json(&CreateBlueprintRequest {
                name: "Cerritos".to_string(),
                ship_class: "frigate".to_string(),
                team_id: team_id.clone(),
            })
            .dispatch();
        let blueprint: BlueprintResponse = blueprint_response.into_json().unwrap();
        
        let add_response = client.post(format!("/v1/blueprints/{}/modules", blueprint.id))
            .json(&AddModuleRequest {
                module_slot_id: "impulse_engine".to_string(),
                variant_id: None,
            })
            .dispatch();
        let updated: BlueprintResponse = add_response.into_json().unwrap();
        let module_id = updated.modules[0].id.clone();
        
        // Configure module
        let config_request = ConfigureModuleRequest {
            kind: Some("ion".to_string()),
        };
        let response = client.patch(format!("/v1/blueprints/{}/modules/{}", blueprint.id, module_id))
            .json(&config_request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let configured: BlueprintResponse = response.into_json().unwrap();
    assert_eq!(configured.modules[0].variant_id, Some("ion".to_string()));
    }

    #[test]
    fn test_ready_workflow() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Create player, team, blueprint, and join
        let player_response = client.post("/v1/players")
            .json(&serde_json::json!({"name": "Sisko"}))
            .dispatch();
        let player_json: serde_json::Value = player_response.into_json().unwrap();
        let player_id = player_json["id"].as_str().unwrap().to_string();
        
        let team_response = client.post("/v1/teams")
            .json(&serde_json::json!({"name": "Eta Team", "faction": "alliance"}))
            .dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap().to_string();
        
        let blueprint_response = client.post("/v1/blueprints")
            .json(&CreateBlueprintRequest {
                name: "DS9".to_string(),
                ship_class: "defender".to_string(),
                team_id: team_id.clone(),
            })
            .dispatch();
        let blueprint: BlueprintResponse = blueprint_response.into_json().unwrap();
        
        client.post(format!("/v1/blueprints/{}/join", blueprint.id))
            .json(&JoinBlueprintRequest { player_id: player_id.clone() })
            .dispatch();
        
        // Mark ready
        let ready_request = ReadyRequest {
            player_id: player_id.clone(),
        };
        let response = client.post(format!("/v1/blueprints/{}/ready", blueprint.id))
            .json(&ready_request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let ready_bp: BlueprintResponse = response.into_json().unwrap();
        assert_eq!(ready_bp.ready_players.len(), 1);
        assert_eq!(ready_bp.all_ready, true);
        
        // Unmark ready
        let response = client.delete(format!("/v1/blueprints/{}/ready/{}", blueprint.id, player_id))
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        let unready_bp: BlueprintResponse = response.into_json().unwrap();
        assert_eq!(unready_bp.ready_players.len(), 0);
        assert_eq!(unready_bp.all_ready, false);
    }

    #[test]
    fn test_validate_blueprint() {
        let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
        
        // Create team and blueprint
        let team_response = client.post("/v1/teams")
            .json(&serde_json::json!({"name": "Theta Team", "faction": "alliance"}))
            .dispatch();
        let team_json: serde_json::Value = team_response.into_json().unwrap();
        let team_id = team_json["id"].as_str().unwrap().to_string();
        
        let blueprint_response = client.post("/v1/blueprints")
            .json(&CreateBlueprintRequest {
                name: "Titan".to_string(),
                ship_class: "dreadnought".to_string(),
                team_id: team_id.clone(),
            })
            .dispatch();
        let blueprint: BlueprintResponse = blueprint_response.into_json().unwrap();
        
        // Validate empty blueprint
        let response = client.get(format!("/v1/blueprints/{}/validate", blueprint.id))
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        let validation: ValidationResponse = response.into_json().unwrap();
        assert_eq!(validation.valid, false);
        assert!(validation.errors.len() > 0);
    }
}
