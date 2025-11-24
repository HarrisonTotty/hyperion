//! Faction API endpoints
//!
//! REST API endpoints for retrieving available factions from game configuration.

use rocket::{routes, Route, State, get};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};

use crate::config::GameConfig;

/// Response for a single faction
#[derive(Debug, Serialize, Deserialize)]
pub struct FactionResponse {
    /// Faction ID
    pub id: String,
    /// Faction name
    pub name: String,
    /// Faction description
    pub description: String,
}

/// Response for listing factions
#[derive(Debug, Serialize, Deserialize)]
pub struct ListFactionsResponse {
    /// List of all available factions
    pub factions: Vec<FactionResponse>,
    /// Total count
    pub count: usize,
}

/// GET /v1/factions - List available factions from config
///
/// Returns a list of all factions loaded from the game configuration.
/// Factions represent the different affiliations that teams can belong to.
#[get("/v1/factions")]
pub fn list_factions(config: &State<GameConfig>) -> Json<ListFactionsResponse> {
    let faction_responses: Vec<FactionResponse> = config
        .factions
        .factions
        .iter()
        .map(|f| FactionResponse {
            id: f.id.clone(),
            name: f.name.clone(),
            description: f.description.clone(),
        })
        .collect();
    
    let count = faction_responses.len();
    
    Json(ListFactionsResponse {
        factions: faction_responses,
        count,
    })
}

/// Returns all faction API routes
pub fn routes() -> Vec<Route> {
    routes![list_factions]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::blocking::Client;
    use rocket::Build;
    use crate::config::{
        GameConfig, AiConfig, FactionsConfig, Faction, MapConfig,
        ModulesConfig, RacesConfig, SimulationConfig,
    };

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
                        id: "federation".to_string(),
                        name: "United Federation".to_string(),
                        description: "A democratic alliance of peaceful worlds".to_string(),
                    },
                    Faction {
                        id: "empire".to_string(),
                        name: "Galactic Empire".to_string(),
                        description: "A militaristic expansionist power".to_string(),
                    },
                    Faction {
                        id: "alliance".to_string(),
                        name: "Free Alliance".to_string(),
                        description: "Independent systems united for mutual defense".to_string(),
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
        let config = create_test_config();
        rocket::build()
            .manage(config)
            .mount("/", routes())
    }

    #[test]
    fn test_list_factions() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        let response = client.get("/v1/factions").dispatch();
        
        assert_eq!(response.status(), rocket::http::Status::Ok);
        
        let body: ListFactionsResponse = response.into_json().unwrap();
        assert_eq!(body.count, 3);
        assert_eq!(body.factions.len(), 3);
        
        // Verify faction details
        let federation = body.factions.iter().find(|f| f.id == "federation").unwrap();
        assert_eq!(federation.name, "United Federation");
        assert!(federation.description.contains("democratic"));
        
        let empire = body.factions.iter().find(|f| f.id == "empire").unwrap();
        assert_eq!(empire.name, "Galactic Empire");
        assert!(empire.description.contains("militaristic"));
    }

    #[test]
    fn test_list_factions_empty() {
        let mut config = create_test_config();
        config.factions.factions = vec![];
        
        let rocket = rocket::build()
            .manage(config)
            .mount("/", routes());
        
        let client = Client::tracked(rocket).unwrap();
        let response = client.get("/v1/factions").dispatch();
        
        assert_eq!(response.status(), rocket::http::Status::Ok);
        
        let body: ListFactionsResponse = response.into_json().unwrap();
        assert_eq!(body.count, 0);
        assert_eq!(body.factions.len(), 0);
    }

    #[test]
    fn test_faction_response_structure() {
        let client = Client::tracked(create_test_rocket()).unwrap();
        let response = client.get("/v1/factions").dispatch();
        
        let body: ListFactionsResponse = response.into_json().unwrap();
        
        // Verify each faction has all required fields
        for faction in &body.factions {
            assert!(!faction.id.is_empty());
            assert!(!faction.name.is_empty());
            assert!(!faction.description.is_empty());
        }
    }

    #[test]
    fn test_factions_match_config() {
        let config = create_test_config();
        let expected_count = config.factions.factions.len();
        
        let rocket = rocket::build()
            .manage(config)
            .mount("/", routes());
        
        let client = Client::tracked(rocket).unwrap();
        let response = client.get("/v1/factions").dispatch();
        
        let body: ListFactionsResponse = response.into_json().unwrap();
        assert_eq!(body.count, expected_count);
    }
}
