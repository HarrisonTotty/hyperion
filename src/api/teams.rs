//! Team API endpoints
//!
//! REST API endpoints for team management operations.

use rocket::{routes, Route, State, get, post, patch, delete};
use rocket::serde::json::Json;
use rocket::http::Status;
use serde::{Deserialize, Serialize};

use crate::state::SharedGameWorld;
use crate::config::GameConfig;

/// Request body for creating a new team
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTeamRequest {
    /// Team name
    pub name: String,
    /// Faction affiliation (accepts "faction_id" from client but stores as "faction")
    #[serde(alias = "faction_id")]
    pub faction: String,
    /// Optional player ID to automatically add as first team member
    pub player_id: Option<String>,
}

/// Response for team creation
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTeamResponse {
    /// Newly created team ID
    pub id: String,
    /// Team name
    pub name: String,
    /// Faction affiliation
    pub faction: String,
}

/// Response for team details
#[derive(Debug, Serialize, Deserialize)]
pub struct TeamResponse {
    /// Team ID
    pub id: String,
    /// Team name
    pub name: String,
    /// Faction affiliation
    pub faction: String,
    /// List of player IDs who are members
    pub members: Vec<String>,
}

/// Response for listing teams
#[derive(Debug, Serialize, Deserialize)]
pub struct ListTeamsResponse {
    /// List of all teams
    pub teams: Vec<TeamResponse>,
    /// Total count
    pub count: usize,
}

/// Request body for adding a player to a team
#[derive(Debug, Serialize, Deserialize)]
pub struct AddPlayerRequest {
    /// Player ID to add
    pub player_id: String,
}

/// Error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
}

/// GET /v1/teams - List all teams
///
/// Returns a list of all teams currently in the game.
#[get("/v1/teams")]
pub fn list_teams(world: &State<SharedGameWorld>) -> Json<ListTeamsResponse> {
    let world = world.read().unwrap();
    let teams = world.get_all_teams();
    
    let team_responses: Vec<TeamResponse> = teams
        .into_iter()
        .map(|t| TeamResponse {
            id: t.id.clone(),
            name: t.name.clone(),
            faction: t.faction.clone(),
            members: t.members.clone(),
        })
        .collect();
    
    let count = team_responses.len();
    
    Json(ListTeamsResponse {
        teams: team_responses,
        count,
    })
}

/// POST /v1/teams - Create a new team
///
/// Creates a new team with the given name and faction. The team name
/// must be unique and 1-50 characters long. Optionally adds the creating
/// player as the first team member.
#[post("/v1/teams", data = "<request>")]
pub fn create_team(
    world: &State<SharedGameWorld>,
    config: &State<GameConfig>,
    request: Json<CreateTeamRequest>,
) -> Result<Json<CreateTeamResponse>, (Status, Json<ErrorResponse>)> {
    // Validate faction exists in config
    let faction_exists = config.factions.factions
        .iter()
        .any(|f| f.id == request.faction);
    
    if !faction_exists {
        return Err((
            Status::BadRequest,
            Json(ErrorResponse {
                error: format!("Invalid faction_id: '{}' not found in configuration", request.faction),
            }),
        ));
    }

    let mut world = world.write().unwrap();
    
    // Validate player exists if player_id provided
    if let Some(ref player_id) = request.player_id {
        if world.get_player(player_id).is_none() {
            return Err((
                Status::BadRequest,
                Json(ErrorResponse {
                    error: format!("Player '{}' not found", player_id),
                }),
            ));
        }
    }
    
    match world.create_team(request.name.clone(), request.faction.clone()) {
        Ok(team_id) => {
            // Auto-add player to team if player_id provided
            if let Some(ref player_id) = request.player_id {
                if let Err(err) = world.add_player_to_team(&team_id, player_id) {
                    return Err((
                        Status::InternalServerError,
                        Json(ErrorResponse {
                            error: format!("Team created but failed to add player: {}", err),
                        }),
                    ));
                }
            }
            
            let team = world.get_team(&team_id).unwrap();
            Ok(Json(CreateTeamResponse {
                id: team.id.clone(),
                name: team.name.clone(),
                faction: team.faction.clone(),
            }))
        }
        Err(err) => {
            Err((
                Status::BadRequest,
                Json(ErrorResponse { error: err }),
            ))
        }
    }
}

/// GET /v1/teams/<id> - Get team details
///
/// Returns details for a specific team by ID, including all member player IDs.
#[get("/v1/teams/<id>")]
pub fn get_team(
    world: &State<SharedGameWorld>,
    id: String,
) -> Result<Json<TeamResponse>, (Status, Json<ErrorResponse>)> {
    let world = world.read().unwrap();
    
    match world.get_team(&id) {
        Some(team) => Ok(Json(TeamResponse {
            id: team.id.clone(),
            name: team.name.clone(),
            faction: team.faction.clone(),
            members: team.members.clone(),
        })),
        None => Err((
            Status::NotFound,
            Json(ErrorResponse {
                error: format!("Team {} not found", id),
            }),
        )),
    }
}

/// PATCH /v1/teams/<id> - Add player to team
///
/// Adds a player to an existing team. The player must exist and not
/// already be a member of the team.
#[patch("/v1/teams/<id>", data = "<request>")]
pub fn add_player_to_team(
    world: &State<SharedGameWorld>,
    id: String,
    request: Json<AddPlayerRequest>,
) -> Result<Json<TeamResponse>, (Status, Json<ErrorResponse>)> {
    let mut world = world.write().unwrap();
    
    match world.add_player_to_team(&id, &request.player_id) {
        Ok(_) => {
            let team = world.get_team(&id).unwrap();
            Ok(Json(TeamResponse {
                id: team.id.clone(),
                name: team.name.clone(),
                faction: team.faction.clone(),
                members: team.members.clone(),
            }))
        }
        Err(err) => {
            Err((
                Status::BadRequest,
                Json(ErrorResponse { error: err }),
            ))
        }
    }
}

/// DELETE /v1/teams/<team_id>/players/<player_id> - Remove player from team
///
/// Removes a player from a team. The player and team must both exist.
#[delete("/v1/teams/<team_id>/players/<player_id>")]
pub fn remove_player_from_team(
    world: &State<SharedGameWorld>,
    team_id: String,
    player_id: String,
) -> Result<Status, (Status, Json<ErrorResponse>)> {
    let mut world = world.write().unwrap();
    
    match world.remove_player_from_team(&team_id, &player_id) {
        Ok(_) => Ok(Status::NoContent),
        Err(err) => Err((
            Status::NotFound,
            Json(ErrorResponse { error: err }),
        )),
    }
}

/// Returns all team API routes
pub fn routes() -> Vec<Route> {
    routes![
        list_teams,
        create_team,
        get_team,
        add_player_to_team,
        remove_player_from_team
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::blocking::Client;
    use rocket::Build;
    use crate::state::GameWorld;
    use crate::api::players::CreatePlayerRequest;
    use crate::config::{
        GameConfig, AiConfig, FactionsConfig, Faction, MapConfig,
        ModulesConfig, RacesConfig, SimulationConfig,
    };
    use std::collections::HashMap;

    fn create_test_config() -> GameConfig {
        use std::collections::HashMap;
        
        GameConfig {
            ai: AiConfig {
                difficulty: "normal".to_string(),
                response_time: 1.0,
            },
            factions: FactionsConfig {
                factions: vec![
                    Faction {
                        id: "Federation".to_string(),
                        name: "United Federation".to_string(),
                        description: "A democratic alliance".to_string(),
                    },
                    Faction {
                        id: "Empire".to_string(),
                        name: "Galactic Empire".to_string(),
                        description: "A militaristic power".to_string(),
                    },
                ],
            },
            map: MapConfig {
                galaxy_size: 1000,
                star_density: 0.15,
            },
            modules: ModulesConfig {
                modules: HashMap::new(),
            },
            races: RacesConfig {
                races: vec![],
            },
            simulation: SimulationConfig {
                tick_rate: 60.0,
                physics_enabled: true,
            },
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
            .mount("/", routes())
            .mount("/", crate::api::players::routes())
    }

    #[test]
    fn test_list_teams_empty() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        let response = client.get("/v1/teams").dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: ListTeamsResponse = response.into_json().unwrap();
        assert_eq!(body.count, 0);
        assert_eq!(body.teams.len(), 0);
    }

    #[test]
    fn test_create_team() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        let response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: CreateTeamResponse = response.into_json().unwrap();
        assert_eq!(body.name, "Alpha Team");
        assert_eq!(body.faction, "Federation");
        assert!(!body.id.is_empty());
    }

    #[test]
    fn test_create_team_duplicate_name() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create first team
        client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        
        // Try to create duplicate
        let response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Empire".to_string(),
                player_id: None,
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
        
        let body: ErrorResponse = response.into_json().unwrap();
        assert!(body.error.contains("already taken"));
    }

    #[test]
    fn test_create_team_invalid_name() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Empty name
        let response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
    }

    #[test]
    fn test_get_team() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create team
        let create_response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        
        let create_body: CreateTeamResponse = create_response.into_json().unwrap();
        let team_id = create_body.id;
        
        // Get team
        let response = client
            .get(format!("/v1/teams/{}", team_id))
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: TeamResponse = response.into_json().unwrap();
        assert_eq!(body.id, team_id);
        assert_eq!(body.name, "Alpha Team");
        assert_eq!(body.faction, "Federation");
        assert_eq!(body.members.len(), 0);
    }

    #[test]
    fn test_get_team_not_found() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        let response = client
            .get("/v1/teams/nonexistent-id")
            .dispatch();
        
        assert_eq!(response.status(), Status::NotFound);
        
        let body: ErrorResponse = response.into_json().unwrap();
        assert!(body.error.contains("not found"));
    }

    #[test]
    fn test_add_player_to_team() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create player
        let player_response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        let player_body: crate::api::players::CreatePlayerResponse = player_response.into_json().unwrap();
        let player_id = player_body.id;
        
        // Create team
        let team_response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        let team_body: CreateTeamResponse = team_response.into_json().unwrap();
        let team_id = team_body.id;
        
        // Add player to team
        let response = client
            .patch(format!("/v1/teams/{}", team_id))
            .json(&AddPlayerRequest {
                player_id: player_id.clone(),
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: TeamResponse = response.into_json().unwrap();
        assert_eq!(body.members.len(), 1);
        assert!(body.members.contains(&player_id));
    }

    #[test]
    fn test_add_player_to_team_player_not_found() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create team
        let team_response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        let team_body: CreateTeamResponse = team_response.into_json().unwrap();
        let team_id = team_body.id;
        
        // Try to add non-existent player
        let response = client
            .patch(format!("/v1/teams/{}", team_id))
            .json(&AddPlayerRequest {
                player_id: "nonexistent-player".to_string(),
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
        
        let body: ErrorResponse = response.into_json().unwrap();
        assert!(body.error.contains("not found"));
    }

    #[test]
    fn test_add_player_to_team_team_not_found() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create player
        let player_response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        let player_body: crate::api::players::CreatePlayerResponse = player_response.into_json().unwrap();
        let player_id = player_body.id;
        
        // Try to add to non-existent team
        let response = client
            .patch("/v1/teams/nonexistent-team")
            .json(&AddPlayerRequest {
                player_id,
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
    }

    #[test]
    fn test_remove_player_from_team() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create player
        let player_response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        let player_body: crate::api::players::CreatePlayerResponse = player_response.into_json().unwrap();
        let player_id = player_body.id;
        
        // Create team
        let team_response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        let team_body: CreateTeamResponse = team_response.into_json().unwrap();
        let team_id = team_body.id;
        
        // Add player to team
        client
            .patch(format!("/v1/teams/{}", team_id))
            .json(&AddPlayerRequest {
                player_id: player_id.clone(),
            })
            .dispatch();
        
        // Remove player from team
        let response = client
            .delete(format!("/v1/teams/{}/players/{}", team_id, player_id))
            .dispatch();
        
        assert_eq!(response.status(), Status::NoContent);
        
        // Verify player was removed
        let team_response = client
            .get(format!("/v1/teams/{}", team_id))
            .dispatch();
        let team_body: TeamResponse = team_response.into_json().unwrap();
        assert_eq!(team_body.members.len(), 0);
    }

    #[test]
    fn test_remove_player_from_team_not_found() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        let response = client
            .delete("/v1/teams/nonexistent-team/players/nonexistent-player")
            .dispatch();
        
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn test_list_teams_with_data() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create multiple teams
        client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        
        client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Bravo Team".to_string(),
                faction: "Empire".to_string(),
                player_id: None,
            })
            .dispatch();
        
        // List teams
        let response = client.get("/v1/teams").dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: ListTeamsResponse = response.into_json().unwrap();
        assert_eq!(body.count, 2);
        assert_eq!(body.teams.len(), 2);
    }

    #[test]
    fn test_team_with_multiple_players() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create players
        let alice_response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        let alice_body: crate::api::players::CreatePlayerResponse = alice_response.into_json().unwrap();
        
        let bob_response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Bob".to_string(),
            })
            .dispatch();
        let bob_body: crate::api::players::CreatePlayerResponse = bob_response.into_json().unwrap();
        
        // Create team
        let team_response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: None,
            })
            .dispatch();
        let team_body: CreateTeamResponse = team_response.into_json().unwrap();
        let team_id = team_body.id;
        
        // Add both players
        client
            .patch(format!("/v1/teams/{}", team_id))
            .json(&AddPlayerRequest {
                player_id: alice_body.id.clone(),
            })
            .dispatch();
        
        client
            .patch(format!("/v1/teams/{}", team_id))
            .json(&AddPlayerRequest {
                player_id: bob_body.id.clone(),
            })
            .dispatch();
        
        // Verify team has both members
        let response = client
            .get(format!("/v1/teams/{}", team_id))
            .dispatch();
        
        let body: TeamResponse = response.into_json().unwrap();
        assert_eq!(body.members.len(), 2);
        assert!(body.members.contains(&alice_body.id));
        assert!(body.members.contains(&bob_body.id));
    }

    #[test]
    fn test_create_team_with_player_auto_join() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Create player
        let player_response = client
            .post("/v1/players")
            .json(&CreatePlayerRequest {
                name: "Alice".to_string(),
            })
            .dispatch();
        let player_body: crate::api::players::CreatePlayerResponse = player_response.into_json().unwrap();
        let player_id = player_body.id;
        
        // Create team with player_id
        let response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: Some(player_id.clone()),
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let create_body: CreateTeamResponse = response.into_json().unwrap();
        let team_id = create_body.id;
        
        // Verify player was automatically added to team
        let team_response = client
            .get(format!("/v1/teams/{}", team_id))
            .dispatch();
        
        let team_body: TeamResponse = team_response.into_json().unwrap();
        assert_eq!(team_body.members.len(), 1);
        assert!(team_body.members.contains(&player_id));
    }

    #[test]
    fn test_create_team_invalid_faction() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        let response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "InvalidFaction".to_string(),
                player_id: None,
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
        
        let body: ErrorResponse = response.into_json().unwrap();
        assert!(body.error.contains("Invalid faction_id"));
    }

    #[test]
    fn test_create_team_with_faction_id_field() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        // Test that faction_id is accepted as an alias for faction
        let response = client
            .post("/v1/teams")
            .json(&serde_json::json!({
                "name": "Alpha Team",
                "faction_id": "Federation",
            }))
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let body: CreateTeamResponse = response.into_json().unwrap();
        assert_eq!(body.name, "Alpha Team");
        assert_eq!(body.faction, "Federation");
    }

    #[test]
    fn test_create_team_with_invalid_player() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        
        let response = client
            .post("/v1/teams")
            .json(&CreateTeamRequest {
                name: "Alpha Team".to_string(),
                faction: "Federation".to_string(),
                player_id: Some("nonexistent-player".to_string()),
            })
            .dispatch();
        
        assert_eq!(response.status(), Status::BadRequest);
        
        let body: ErrorResponse = response.into_json().unwrap();
        assert!(body.error.contains("not found"));
    }
}

