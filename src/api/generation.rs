//! REST API endpoints for procedural generation.
//!
//! This module provides HTTP endpoints for generating and querying procedurally generated
//! universes, including galaxies, star systems, factions, languages, and history.

use rocket::{State, get, post};
use rocket::serde::json::Json;
use rocket::http::Status;
use serde::{Serialize, Deserialize};
use std::sync::{Arc, RwLock};

use crate::generation::{ProceduralUniverse, Star, StarSystem, ProceduralFaction};

/// Application state for storing the current procedural universe
pub struct UniverseState {
    pub universe: Option<ProceduralUniverse>,
}

impl UniverseState {
    pub fn new() -> Self {
        Self { universe: None }
    }
}

/// Request to generate a new universe
#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateUniverseRequest {
    /// Name of the universe
    pub name: String,
    /// Random seed for deterministic generation
    pub seed: u64,
    /// Number of stars to generate
    pub num_stars: usize,
    /// Number of factions to generate
    pub num_factions: usize,
}

/// Response containing universe metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct UniverseResponse {
    pub name: String,
    pub seed: u64,
    pub num_stars: usize,
    pub num_systems: usize,
    pub num_factions: usize,
}

/// Response containing galaxy information
#[derive(Debug, Serialize)]
pub struct GalaxyResponse {
    pub radius: f64,
    pub stars: Vec<StarResponse>,
}

#[derive(Debug, Serialize)]
pub struct StarResponse {
    pub id: String,
    pub name: String,
    pub position: [f64; 3],
    pub star_type: String,
    pub sector: String,
}

/// Response containing star system information
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemResponse {
    pub star_id: String,
    pub star_name: String,
    pub num_planets: usize,
    pub num_moons: usize,
    pub num_asteroid_belts: usize,
    pub num_stations: usize,
}

/// Response containing detailed system information
#[derive(Debug, Serialize)]
pub struct SystemDetailResponse {
    pub star_id: String,
    pub star_name: String,
    pub planets: Vec<PlanetResponse>,
    pub asteroid_belts: Vec<AsteroidBeltResponse>,
    pub stations: Vec<StationResponse>,
}

#[derive(Debug, Serialize)]
pub struct PlanetResponse {
    pub name: String,
    pub planet_type: String,
    pub orbit_radius: f64,
    pub mass: f64,
    pub has_atmosphere: bool,
    pub moons: Vec<MoonResponse>,
}

#[derive(Debug, Serialize)]
pub struct MoonResponse {
    pub name: String,
    pub mass: f32,
    pub radius: f32,
}

#[derive(Debug, Serialize)]
pub struct AsteroidBeltResponse {
    pub inner_radius: f64,
    pub outer_radius: f64,
    pub density: f64,
}

#[derive(Debug, Serialize)]
pub struct StationResponse {
    pub name: String,
    pub station_type: String,
    pub location: StationLocation,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum StationLocation {
    Planetary { planet: String },
    Orbital,
}

/// Response containing faction information
#[derive(Debug, Serialize, Deserialize)]
pub struct FactionResponse {
    pub id: String,
    pub name: String,
    pub government: String,
    pub traits: Vec<String>,
    pub num_systems: usize,
}

/// Response containing detailed faction information
#[derive(Debug, Serialize)]
pub struct FactionDetailResponse {
    pub id: String,
    pub name: String,
    pub government: String,
    pub traits: Vec<String>,
    pub territories: Vec<String>,
    pub relationships: Vec<RelationshipResponse>,
}

#[derive(Debug, Serialize)]
pub struct RelationshipResponse {
    pub faction_id: String,
    pub faction_name: String,
    pub relationship: String,
}

/// Request to translate text using a faction's language
#[derive(Debug, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
}

/// Response containing translated text
#[derive(Debug, Serialize)]
pub struct TranslateResponse {
    pub original: String,
    pub translated: String,
}

/// Response containing language information
#[derive(Debug, Serialize)]
pub struct LanguageResponse {
    pub faction_id: String,
    pub faction_name: String,
    pub sample_words: Vec<(String, String)>,
    pub sample_phrase: String,
}

/// Response containing historical events
#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryResponse {
    pub events: Vec<EventResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventResponse {
    pub year: i32,
    pub event_type: String,
    pub factions: Vec<String>,
    pub description: String,
}

/// Response containing timeline summary
#[derive(Debug, Serialize)]
pub struct TimelineResponse {
    pub summary: String,
}

/// Generate a new procedural universe
#[post("/v1/generation/universe", format = "json", data = "<request>")]
pub fn generate_universe(
    request: Json<GenerateUniverseRequest>,
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<UniverseResponse>, Status> {
    let universe = ProceduralUniverse::generate(
        request.name.clone(),
        request.seed,
        request.num_stars,
        request.num_factions,
    );
    
    let response = UniverseResponse {
        name: universe.name.clone(),
        seed: universe.seed,
        num_stars: universe.galaxy.stars.len(),
        num_systems: universe.systems.len(),
        num_factions: universe.factions.len(),
    };
    
    state.write().unwrap().universe = Some(universe);
    
    Ok(Json(response))
}

/// Get current universe information
#[get("/v1/generation/universe")]
pub fn get_universe(
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<UniverseResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        Ok(Json(UniverseResponse {
            name: universe.name.clone(),
            seed: universe.seed,
            num_stars: universe.galaxy.stars.len(),
            num_systems: universe.systems.len(),
            num_factions: universe.factions.len(),
        }))
    } else {
        Err(Status::NotFound)
    }
}

/// Get galaxy information
#[get("/v1/generation/galaxy")]
pub fn get_galaxy(
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<GalaxyResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        let stars: Vec<StarResponse> = universe.galaxy.stars.iter()
            .map(|star| StarResponse {
                id: star.id.clone(),
                name: star.name.clone(),
                position: star.position,
                star_type: format!("{:?}", star.star_type),
                sector: format!("{:?}", star.sector),
            })
            .collect();
        
        Ok(Json(GalaxyResponse {
            radius: universe.galaxy.radius,
            stars,
        }))
    } else {
        Err(Status::NotFound)
    }
}

/// List all star systems
#[get("/v1/generation/systems")]
pub fn list_systems(
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<Vec<SystemResponse>>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        let systems: Vec<SystemResponse> = universe.systems.iter()
            .map(|system| {
                let star = universe.galaxy.stars.iter()
                    .find(|s| s.id == system.id)
                    .unwrap();
                
                let num_moons: usize = system.planets.iter()
                    .map(|p| p.moons.len())
                    .sum();
                
                SystemResponse {
                    star_id: system.id.clone(),
                    star_name: star.name.clone(),
                    num_planets: system.planets.len(),
                    num_moons,
                    num_asteroid_belts: system.asteroid_belts.len(),
                    num_stations: system.stations.len(),
                }
            })
            .collect();
        
        Ok(Json(systems))
    } else {
        Err(Status::NotFound)
    }
}

/// Get detailed information about a specific star system
#[get("/v1/generation/systems/<star_id>")]
pub fn get_system(
    star_id: String,
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<SystemDetailResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        if let Some(system) = universe.get_system(&star_id) {
            let star = universe.galaxy.stars.iter()
                .find(|s| s.id == star_id)
                .unwrap();
            
            let planets: Vec<PlanetResponse> = system.planets.iter()
                .map(|planet| PlanetResponse {
                    name: planet.name.clone(),
                    planet_type: format!("{:?}", planet.planet_type),
                    orbit_radius: planet.orbital_radius,
                    mass: planet.mass as f64,
                    has_atmosphere: planet.atmosphere,
                    moons: planet.moons.iter()
                        .map(|moon| MoonResponse {
                            name: moon.name.clone(),
                            mass: moon.mass,
                            radius: moon.radius,
                        })
                        .collect(),
                })
                .collect();
            
            let asteroid_belts: Vec<AsteroidBeltResponse> = system.asteroid_belts.iter()
                .map(|belt| AsteroidBeltResponse {
                    inner_radius: belt.inner_radius,
                    outer_radius: belt.outer_radius,
                    density: belt.density as f64,
                })
                .collect();
            
            let stations: Vec<StationResponse> = system.stations.iter()
                .map(|station| StationResponse {
                    name: station.name.clone(),
                    station_type: format!("{:?}", station.station_type),
                    location: if station.orbiting != "Star" {
                        StationLocation::Planetary { planet: station.orbiting.clone() }
                    } else {
                        StationLocation::Orbital
                    },
                })
                .collect();
            
            Ok(Json(SystemDetailResponse {
                star_id: star_id.clone(),
                star_name: star.name.clone(),
                planets,
                asteroid_belts,
                stations,
            }))
        } else {
            Err(Status::NotFound)
        }
    } else {
        Err(Status::NotFound)
    }
}

/// List all factions
#[get("/v1/generation/factions")]
pub fn list_factions(
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<Vec<FactionResponse>>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        let factions: Vec<FactionResponse> = universe.factions.iter()
            .map(|faction| FactionResponse {
                id: faction.id.clone(),
                name: faction.name.clone(),
                government: format!("{:?}", faction.government),
                traits: faction.traits.iter()
                    .map(|t| format!("{:?}", t))
                    .collect(),
                num_systems: faction.territories.len(),
            })
            .collect();
        
        Ok(Json(factions))
    } else {
        Err(Status::NotFound)
    }
}

/// Get detailed information about a specific faction
#[get("/v1/generation/factions/<faction_id>")]
pub fn get_faction(
    faction_id: String,
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<FactionDetailResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        if let Some(faction) = universe.get_faction(&faction_id) {
            let relationships: Vec<RelationshipResponse> = faction.relationships.iter()
                .map(|(other_id, relationship)| {
                    let other_faction = universe.get_faction(other_id).unwrap();
                    RelationshipResponse {
                        faction_id: other_id.clone(),
                        faction_name: other_faction.name.clone(),
                        relationship: format!("{:?}", relationship),
                    }
                })
                .collect();
            
            Ok(Json(FactionDetailResponse {
                id: faction.id.clone(),
                name: faction.name.clone(),
                government: format!("{:?}", faction.government),
                traits: faction.traits.iter()
                    .map(|t| format!("{:?}", t))
                    .collect(),
                territories: faction.territories.clone(),
                relationships,
            }))
        } else {
            Err(Status::NotFound)
        }
    } else {
        Err(Status::NotFound)
    }
}

/// Get language for a faction
#[get("/v1/generation/languages/<faction_id>")]
pub fn get_language(
    faction_id: String,
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<LanguageResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        if let Some(faction) = universe.get_faction(&faction_id) {
            if let Some(language) = universe.get_faction_language(&faction_id) {
                let sample_words: Vec<(String, String)> = language.vocabulary.iter()
                    .take(10)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                
                let sample_phrase = language.generate_phrase(12345);
                
                Ok(Json(LanguageResponse {
                    faction_id: faction.id.clone(),
                    faction_name: faction.name.clone(),
                    sample_words,
                    sample_phrase,
                }))
            } else {
                Err(Status::NotFound)
            }
        } else {
            Err(Status::NotFound)
        }
    } else {
        Err(Status::NotFound)
    }
}

/// Translate text to a faction's language
#[post("/v1/generation/languages/<faction_id>/translate", format = "json", data = "<request>")]
pub fn translate(
    faction_id: String,
    request: Json<TranslateRequest>,
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<TranslateResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        if let Some(language) = universe.get_faction_language(&faction_id) {
            let words: Vec<&str> = request.text.split_whitespace().collect();
            let translated_words: Vec<String> = words.iter()
                .filter_map(|word| language.translate(word).map(|s| s.clone()))
                .collect();
            
            Ok(Json(TranslateResponse {
                original: request.text.clone(),
                translated: translated_words.join(" "),
            }))
        } else {
            Err(Status::NotFound)
        }
    } else {
        Err(Status::NotFound)
    }
}

/// Get complete historical timeline
#[get("/v1/generation/history")]
pub fn get_history(
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<HistoryResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        let events: Vec<EventResponse> = universe.history.iter()
            .map(|event| {
                let faction_names: Vec<String> = event.factions.iter()
                    .filter_map(|id| universe.get_faction(id).map(|f| f.name.clone()))
                    .collect();
                
                EventResponse {
                    year: event.year,
                    event_type: format!("{:?}", event.event_type),
                    factions: faction_names,
                    description: event.description.clone(),
                }
            })
            .collect();
        
        Ok(Json(HistoryResponse { events }))
    } else {
        Err(Status::NotFound)
    }
}

/// Get history for a specific faction
#[get("/v1/generation/history/<faction_id>")]
pub fn get_faction_history(
    faction_id: String,
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<HistoryResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        let events: Vec<EventResponse> = universe.get_faction_history(&faction_id).iter()
            .map(|event| {
                let faction_names: Vec<String> = event.factions.iter()
                    .filter_map(|id| universe.get_faction(id).map(|f| f.name.clone()))
                    .collect();
                
                EventResponse {
                    year: event.year,
                    event_type: format!("{:?}", event.event_type),
                    factions: faction_names,
                    description: event.description.clone(),
                }
            })
            .collect();
        
        Ok(Json(HistoryResponse { events }))
    } else {
        Err(Status::NotFound)
    }
}

/// Get timeline summary
#[get("/v1/generation/timeline")]
pub fn get_timeline(
    state: &State<Arc<RwLock<UniverseState>>>,
) -> Result<Json<TimelineResponse>, Status> {
    let state = state.read().unwrap();
    
    if let Some(universe) = &state.universe {
        use crate::generation::HistoryGenerator;
        let mut history_gen = HistoryGenerator::new(universe.seed);
        let summary = history_gen.generate_timeline_summary(&universe.history, &universe.factions);
        
        Ok(Json(TimelineResponse { summary }))
    } else {
        Err(Status::NotFound)
    }
}

/// Returns all generation API routes
pub fn routes() -> Vec<rocket::Route> {
    rocket::routes![
        generate_universe,
        get_universe,
        get_galaxy,
        list_systems,
        get_system,
        list_factions,
        get_faction,
        get_language,
        translate,
        get_history,
        get_faction_history,
        get_timeline,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::local::blocking::Client;
    
    fn create_test_client() -> Client {
        let state = Arc::new(RwLock::new(UniverseState::new()));
        
        let rocket = rocket::build()
            .manage(state)
            .mount("/", rocket::routes![
                generate_universe,
                get_universe,
                get_galaxy,
                list_systems,
                get_system,
                list_factions,
                get_faction,
                get_language,
                translate,
                get_history,
                get_faction_history,
                get_timeline,
            ]);
        
        Client::tracked(rocket).expect("valid rocket instance")
    }
    
    #[test]
    fn test_generate_universe() {
        let client = create_test_client();
        
        let request = GenerateUniverseRequest {
            name: "Test Universe".to_string(),
            seed: 12345,
            num_stars: 100,
            num_factions: 3,
        };
        
        let response = client.post("/v1/generation/universe")
            .json(&request)
            .dispatch();
        
        assert_eq!(response.status(), Status::Ok);
        
        let universe: UniverseResponse = response.into_json().unwrap();
        assert_eq!(universe.name, "Test Universe");
        assert_eq!(universe.seed, 12345);
    }
    
    #[test]
    fn test_get_universe() {
        let client = create_test_client();
        
        // First generate a universe
        let request = GenerateUniverseRequest {
            name: "Test Universe".to_string(),
            seed: 12345,
            num_stars: 50,
            num_factions: 2,
        };
        
        client.post("/v1/generation/universe")
            .json(&request)
            .dispatch();
        
        // Then retrieve it
        let response = client.get("/v1/generation/universe").dispatch();
        assert_eq!(response.status(), Status::Ok);
    }
    
    #[test]
    fn test_list_systems() {
        let client = create_test_client();
        
        // Generate universe
        let request = GenerateUniverseRequest {
            name: "Test Universe".to_string(),
            seed: 12345,
            num_stars: 50,
            num_factions: 2,
        };
        
        client.post("/v1/generation/universe")
            .json(&request)
            .dispatch();
        
        // List systems
        let response = client.get("/v1/generation/systems").dispatch();
        assert_eq!(response.status(), Status::Ok);
        
        let systems: Vec<SystemResponse> = response.into_json().unwrap();
        assert!(!systems.is_empty());
    }
    
    #[test]
    fn test_list_factions() {
        let client = create_test_client();
        
        // Generate universe
        let request = GenerateUniverseRequest {
            name: "Test Universe".to_string(),
            seed: 12345,
            num_stars: 50,
            num_factions: 3,
        };
        
        client.post("/v1/generation/universe")
            .json(&request)
            .dispatch();
        
        // List factions
        let response = client.get("/v1/generation/factions").dispatch();
        assert_eq!(response.status(), Status::Ok);
        
        let factions: Vec<FactionResponse> = response.into_json().unwrap();
        assert_eq!(factions.len(), 3);
    }
    
    #[test]
    fn test_get_history() {
        let client = create_test_client();
        
        // Generate universe
        let request = GenerateUniverseRequest {
            name: "Test Universe".to_string(),
            seed: 12345,
            num_stars: 50,
            num_factions: 3,
        };
        
        client.post("/v1/generation/universe")
            .json(&request)
            .dispatch();
        
        // Get history
        let response = client.get("/v1/generation/history").dispatch();
        assert_eq!(response.status(), Status::Ok);
        
        let history: HistoryResponse = response.into_json().unwrap();
        assert!(!history.events.is_empty());
    }
}
