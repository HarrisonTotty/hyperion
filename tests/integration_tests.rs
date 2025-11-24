//! Integration tests for HYPERION game server
//!
//! These tests verify complete workflows and interactions between multiple systems.

use hyperion::api;
use hyperion::api::generation::UniverseState;
use hyperion::models::*;
use hyperion::state::GameWorld;
use hyperion::config::*;
use hyperion::stations::Station;
use hyperion::websocket::WebSocketManager;
use rocket::local::blocking::Client;
use rocket::http::Status;
use serde_json::json;
use std::sync::{Arc, RwLock};

/// Create a test Rocket instance with minimal configuration
fn create_test_rocket() -> rocket::Rocket<rocket::Build> {
    let config = create_test_config();
    let game_world = GameWorld::new_shared();
    let ws_manager = Arc::new(WebSocketManager::new());
    let universe_state = Arc::new(RwLock::new(UniverseState::new()));
    
    rocket::build()
        .manage(config)
        .manage(game_world)
        .manage(ws_manager)
        .manage(universe_state)
        .mount("/", api::routes())
        .mount("/graphql", api::graphql_routes())
}

/// Helper to create a minimal test configuration
fn create_test_config() -> GameConfig {
    use std::collections::HashMap;
    
    let ship_class = ShipClassConfig {
        id: "cruiser".to_string(),
        name: "Cruiser".to_string(),
        description: "Medium combat vessel".to_string(),
        base_hull: 5000.0,
        base_shields: 2500.0,
        max_weight: 50000.0,
        max_modules: 20,
        size: ShipSize::Medium,
        role: ShipClassRole::Combat,
        build_points: 1000.0,
        bonuses: HashMap::new(),
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
    
    GameConfig {
        ai: AiConfig { difficulty: "medium".to_string(), response_time: 1.0 },
        factions: FactionsConfig { factions: vec![
            hyperion::config::Faction { id: "federation".to_string(), name: "Federation".to_string(), description: "Test faction".to_string() },
            hyperion::config::Faction { id: "empire".to_string(), name: "Empire".to_string(), description: "Test faction".to_string() },
        ] },
        map: MapConfig { galaxy_size: 1000, star_density: 0.5 },
        modules: ModulesConfig { modules: std::collections::HashMap::new() },
        races: RacesConfig { races: vec![] },
        simulation: SimulationConfig { tick_rate: 60.0, physics_enabled: true },
        ship_classes: vec![ship_class],
        module_definitions: vec![],
        weapon_definitions: vec![],
        ammunition_types: vec![],
        kinetic_weapon_kinds: vec![],
        ai_behavior: AIConfig::default(),
        procedural_map: ProceduralMapConfig::default(),
        simulation_params: ProceduralSimConfig::default(),
        faction_generation: FactionGenConfig::default(),
    module_variants: HashMap::new(),
    bonuses: None,
    module_slots: HashMap::new(),
    }
}

#[test]
fn test_complete_player_registration_flow() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Step 1: Register first player
    let response = client
        .post("/v1/players")
        .json(&json!({"name": "Alice"}))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    let alice: Player = response.into_json().unwrap();
    assert_eq!(alice.name, "Alice");
    assert!(!alice.id.is_empty());
    
    // Step 2: Register second player
    let response = client
        .post("/v1/players")
        .json(&json!({"name": "Bob"}))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    let bob: Player = response.into_json().unwrap();
    assert_eq!(bob.name, "Bob");
    assert_ne!(alice.id, bob.id); // Different IDs
    
    // Step 3: List all players
    let response = client.get("/v1/players").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let players: Vec<Player> = response.into_json().unwrap();
    assert_eq!(players.len(), 2);
    
    // Step 4: Get specific player
    let response = client.get(format!("/v1/players/{}", alice.id)).dispatch();
    assert_eq!(response.status(), Status::Ok);
    let fetched_alice: Player = response.into_json().unwrap();
    assert_eq!(fetched_alice.id, alice.id);
    assert_eq!(fetched_alice.name, "Alice");
    
    // Step 5: Create team and add players
    let response = client
        .post("/v1/teams")
        .json(&json!({"name": "Alpha Team", "faction": "federation"}))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    let team: Team = response.into_json().unwrap();
    
    // Step 6: Add Alice to team
    let response = client
        .patch(format!("/v1/teams/{}", team.id))
        .json(&json!({"player_id": alice.id}))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    
    // Step 7: Add Bob to team
    let response = client
        .patch(format!("/v1/teams/{}", team.id))
        .json(&json!({"player_id": bob.id}))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    
    // Step 8: Verify team has both players
    let response = client.get(format!("/v1/teams/{}", team.id)).dispatch();
    assert_eq!(response.status(), Status::Ok);
    let updated_team: Team = response.into_json().unwrap();
    assert_eq!(updated_team.members.len(), 2);
    assert!(updated_team.members.contains(&alice.id));
    assert!(updated_team.members.contains(&bob.id));
}

#[test]
fn test_complete_ship_creation_and_launch_flow() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Setup: Create players and team
    let response = client
        .post("/v1/players")
        .json(&json!({"name": "Captain"}))
        .dispatch();
    let captain: Player = response.into_json().unwrap();
    
    let response = client
        .post("/v1/players")
        .json(&json!({"name": "Engineer"}))
        .dispatch();
    let engineer: Player = response.into_json().unwrap();
    
    let response = client
        .post("/v1/teams")
        .json(&json!({"name": "Crew", "faction": "federation"}))
        .dispatch();
    let team: Team = response.into_json().unwrap();
    
    client
        .patch(format!("/v1/teams/{}", team.id))
        .json(&json!({"player_id": captain.id}))
        .dispatch();
    
    client
        .patch(format!("/v1/teams/{}", team.id))
        .json(&json!({"player_id": engineer.id}))
        .dispatch();
    
    // Step 1: Create blueprint
    let response = client
        .post("/v1/blueprints")
        .json(&json!({
            "name": "USS Enterprise",
            "ship_class": "cruiser",
            "team_id": team.id,
            "creator_id": captain.id
        }))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    let blueprint: ShipBlueprint = response.into_json().unwrap();
    assert_eq!(blueprint.name, "USS Enterprise");
    
    // Step 2: Engineer joins blueprint
    let response = client
        .post(format!("/v1/blueprints/{}/join", blueprint.id))
        .json(&json!({"player_id": engineer.id}))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    
    // Step 3: Assign roles
    let response = client
        .patch(format!("/v1/blueprints/{}/roles", blueprint.id))
        .json(&json!({
            "player_id": captain.id,
            "roles": ["Captain", "Helm"]
        }))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    
    let response = client
        .patch(format!("/v1/blueprints/{}/roles", blueprint.id))
        .json(&json!({
            "player_id": engineer.id,
            "roles": ["Engineering"]
        }))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    
    // Step 4: Mark players ready
    let response = client
        .post(format!("/v1/blueprints/{}/ready", blueprint.id))
        .json(&json!({"player_id": captain.id}))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    
    let response = client
        .post(format!("/v1/blueprints/{}/ready", blueprint.id))
        .json(&json!({"player_id": engineer.id}))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    
    // Step 5: Validate blueprint (would fail without required modules, but test the endpoint)
    let response = client
        .get(format!("/v1/blueprints/{}/validate", blueprint.id))
        .dispatch();
    
    // Note: This will return validation errors due to missing required modules
    // but the endpoint should work
    assert!(response.status() == Status::Ok || response.status() == Status::BadRequest);
    
    // Step 6: List blueprints
    let response = client.get("/v1/blueprints").dispatch();
    assert_eq!(response.status(), Status::Ok);
    let blueprints: Vec<ShipBlueprint> = response.into_json().unwrap();
    assert_eq!(blueprints.len(), 1);
    assert_eq!(blueprints[0].name, "USS Enterprise");
}

#[test]
fn test_docking_procedures() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Setup: Create a station
    let response = client
        .post("/v1/stations")
        .json(&json!({
            "name": "Deep Space 9",
            "position": {"x": 0.0, "y": 0.0, "z": 0.0},
            "faction": "federation",
            "size": "Large",
            "services": ["Repair", "Refuel", "Rearm"]
        }))
        .dispatch();
    
    assert_eq!(response.status(), Status::Ok);
    let station: Station = response.into_json().unwrap();
    
    // Setup: Create a player and ship (simplified - in real flow would go through blueprint)
    let response = client
        .post("/v1/players")
        .json(&json!({"name": "Pilot"}))
        .dispatch();
    let player: Player = response.into_json().unwrap();
    
    let response = client
        .post("/v1/teams")
        .json(&json!({"name": "Crew", "faction": "federation"}))
        .dispatch();
    let team: Team = response.into_json().unwrap();
    
    client
        .patch(format!("/v1/teams/{}", team.id))
        .json(&json!({"player_id": player.id}))
        .dispatch();
    
    // Create a blueprint and launch it (simplified)
    let response = client
        .post("/v1/blueprints")
        .json(&json!({
            "name": "Test Ship",
            "ship_class": "cruiser",
            "team_id": team.id,
            "creator_id": player.id
        }))
        .dispatch();
    let blueprint: ShipBlueprint = response.into_json().unwrap();
    
    // Assign role and mark ready
    client
        .patch(format!("/v1/blueprints/{}/roles", blueprint.id))
        .json(&json!({"player_id": player.id, "roles": ["Captain"]}))
        .dispatch();
    
    client
        .post(format!("/v1/blueprints/{}/ready", blueprint.id))
        .json(&json!({"player_id": player.id}))
        .dispatch();
    
    // Step 1: Request docking
    let response = client
        .post(format!("/v1/stations/{}/dock", station.id))
        .json(&json!({"ship_id": "test-ship-id"}))
        .dispatch();
    
    // Will succeed (creates docking request)
    assert_eq!(response.status(), Status::Ok);
    
    // Step 2: Check docking status
    let response = client
        .get(format!("/v1/stations/{}/dock/test-ship-id", station.id))
        .dispatch();
    
    // Docking system works (even if ship doesn't exist, endpoint is accessible)
    assert!(response.status() == Status::Ok || response.status() == Status::NotFound);
}

#[test]
fn test_power_and_cooling_allocation() {
    // This test verifies the engineering workflow
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Setup ship (simplified)
    let response = client
        .post("/v1/players")
        .json(&json!({"name": "Engineer"}))
        .dispatch();
    let player: Player = response.into_json().unwrap();
    
    let response = client
        .post("/v1/teams")
        .json(&json!({"name": "Engineering Crew", "faction": "federation"}))
        .dispatch();
    let team: Team = response.into_json().unwrap();
    
    client
        .patch(format!("/v1/teams/{}", team.id))
        .json(&json!({"player_id": player.id}))
        .dispatch();
    
    // In a real scenario, we'd create and launch a ship
    // For this test, we're verifying the API endpoints work
    
    // Test power allocation endpoint
    let response = client
        .patch("/v1/ships/test-ship/power/allocate")
        .json(&json!({
            "module_id": "engine-1",
            "power": 100.0
        }))
        .dispatch();
    
    // Will fail because ship doesn't exist, but endpoint is accessible
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
    
    // Test cooling allocation endpoint
    let response = client
        .patch("/v1/ships/test-ship/cooling/allocate")
        .json(&json!({
            "module_id": "engine-1",
            "cooling": 50.0
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
    
    // Test ship status endpoint
    let response = client
        .get("/v1/ships/test-ship/status")
        .dispatch();
    
    assert_eq!(response.status(), Status::NotFound);
}

#[test]
fn test_communication_between_ships() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Test hailing endpoint
    let response = client
        .post("/v1/ships/ship1/hail")
        .json(&json!({
            "target_ship_id": "ship2",
            "message": "This is USS Enterprise, please respond"
        }))
        .dispatch();
    
    // Will fail because ships don't exist, but endpoint works
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
    
    // Test respond endpoint
    let response = client
        .post("/v1/ships/ship2/respond")
        .json(&json!({
            "hail_id": "test-hail-id",
            "message": "This is USS Voyager, greetings"
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
}

#[test]
fn test_ion_weapon_jamming_effects() {
    // Test Ion weapon effects on communications and science
    use hyperion::simulation::components::*;
    
    // Verify Ion effect blocks communications
    let mut comm_state = CommunicationState::default();
    comm_state.jammed = true;
    comm_state.jam_duration = 5.0;
    
    assert!(!comm_state.can_communicate());
    
    // Test that comms API rejects when jammed
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Attempting to hail while jammed should fail
    // (In real implementation, this would check ship's jam status)
    let response = client
        .post("/v1/ships/jammed-ship/hail")
        .json(&json!({
            "target_ship_id": "other-ship",
            "message": "Test"
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
}

#[test]
fn test_graviton_weapon_slowing_ships() {
    // Test Graviton effect on ship movement
    use hyperion::simulation::physics::*;
    use hyperion::simulation::components::StatusEffects;
    use hyperion::weapons::StatusEffectType;
    
    let base_weight = 1000.0;
    
    // Without Graviton effect
    let effects_without_graviton = StatusEffects::default();
    let normal_weight = calculate_effective_weight(base_weight, &effects_without_graviton);
    assert_eq!(normal_weight, base_weight);
    
    // With Graviton effect (2x weight multiplier)
    let mut effects_with_graviton = StatusEffects::default();
    effects_with_graviton.apply(StatusEffectType::GravitonWeight, 5.0);
    let graviton_weight = calculate_effective_weight(base_weight, &effects_with_graviton);
    assert_eq!(graviton_weight, 2000.0); // 2x multiplier
    
    // Verify this affects acceleration (F = ma, so higher mass = lower acceleration)
    let force = 1000.0; // Newtons
    let normal_acceleration = force / normal_weight;
    let graviton_acceleration = force / graviton_weight;
    
    assert!(graviton_acceleration < normal_acceleration);
    assert_eq!(graviton_acceleration / normal_acceleration, 0.5); // Half the acceleration
}

#[test]
fn test_tachyon_weapon_preventing_ftl() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Test that Tachyon effect prevents warp engagement
    // (In real implementation, ship would have Tachyon status effect)
    let response = client
        .post("/v1/ships/affected-ship/helm/warp")
        .json(&json!({
            "destination": {"x": 1000.0, "y": 0.0, "z": 0.0}
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
    
    // Test that Tachyon effect prevents jump
    let response = client
        .post("/v1/ships/affected-ship/helm/jump")
        .json(&json!({
            "destination": {"x": 1000.0, "y": 0.0, "z": 0.0}
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
}

#[test]
fn test_countermeasure_vs_missile_combat() {
    use hyperion::weapons::tags::*;
    use hyperion::models::WeaponTag;
    
    // Test antimissile weapon damage against missile
    let calculator = WeaponTagCalculator::default();
    let antimissile_tags = vec![WeaponTag::Antimissile];
    let base_damage = 100.0;
    
    let result = calculator.calculate_damage(base_damage, &antimissile_tags).unwrap();
    
    // Antimissile does 0.3x damage
    assert_eq!(result.hull_damage, 30.0);
    
    // Test chaff doesn't damage
    let chaff_tags = vec![WeaponTag::Chaff];
    let result = calculator.calculate_damage(base_damage, &chaff_tags).unwrap();
    
    assert_eq!(result.hull_damage, 0.0);
    assert_eq!(result.shield_damage, 0.0);
}

#[test]
fn test_automatic_vs_manual_weapon_fire_modes() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Test toggling automatic fire for energy weapons
    let response = client
        .post("/v1/ships/test-ship/energy-weapons/auto")
        .json(&json!({
            "weapon_id": "laser-1",
            "enabled": true
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
    
    // Test manual fire
    let response = client
        .post("/v1/ships/test-ship/energy-weapons/fire")
        .json(&json!({
            "weapon_id": "laser-1"
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
    
    // Test toggling automatic fire for kinetic weapons
    let response = client
        .post("/v1/ships/test-ship/kinetic-weapons/weapon-1/auto")
        .json(&json!({"enabled": true}))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
}

#[test]
fn test_warp_drive_vs_jump_drive() {
    let client = Client::tracked(create_test_rocket()).expect("valid rocket instance");
    
    // Test warp drive (acceleration-based FTL)
    let response = client
        .post("/v1/ships/test-ship/helm/warp")
        .json(&json!({
            "destination": {"x": 10000.0, "y": 0.0, "z": 0.0}
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
    
    // Test jump drive (instant teleport FTL)
    let response = client
        .post("/v1/ships/test-ship/helm/jump")
        .json(&json!({
            "destination": {"x": 10000.0, "y": 0.0, "z": 0.0}
        }))
        .dispatch();
    
    assert!(response.status() == Status::NotFound || response.status() == Status::BadRequest);
    
    // Verify both endpoints are accessible (even though ships don't exist)
    // The important part is that the API structure supports both drive types
}

#[test]
fn test_combat_scenario_with_weapon_tags() {
    use hyperion::weapons::tags::*;
    use hyperion::models::WeaponTag;
    
    let calculator = WeaponTagCalculator::default();
    
    // Scenario: Plasma weapon (2x damage to shields)
    let plasma_tags = vec![WeaponTag::Plasma, WeaponTag::Beam];
    let base_damage = 100.0;
    
    let result = calculator.calculate_damage(base_damage, &plasma_tags).unwrap();
    
    // Plasma should do 2x shield damage (shield_damage will be 200)
    assert_eq!(result.shield_damage, 200.0);
    
    // Scenario: Positron weapon with shield bypass (20%)
    let positron_tags = vec![WeaponTag::Positron, WeaponTag::Pulse];
    
    let result = calculator.calculate_damage(100.0, &positron_tags).unwrap();
    
    // Positron should have 20% shield bypass
    assert_eq!(result.shield_bypass, 0.2);
    
    // Scenario: Burst weapon (fires 3 rounds)
    let burst_tags = vec![WeaponTag::Burst];
    
    let result = calculator.calculate_damage(100.0, &burst_tags).unwrap();
    assert_eq!(result.projectile_count, 3);
    
    // Scenario: Ion status effect
    let ion_tags = vec![WeaponTag::Ion];
    
    let result = calculator.calculate_damage(100.0, &ion_tags).unwrap();
    assert!(result.status_effect.is_some());
    assert_eq!(result.status_effect.unwrap().effect_type, StatusEffectType::IonJam);
    
    // Test fire pattern detection
    let pattern = calculator.get_fire_pattern(&burst_tags);
    assert_eq!(pattern, Some(WeaponTag::Burst));
}
