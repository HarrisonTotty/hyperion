/// Ship classes API endpoints
///
/// Provides REST API for querying available ship classes with detailed specifications.

use rocket::{get, State, Route, routes};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::{BonusConfig, FormattedBonus, ShipClassConfig, ShipSize, ShipClassRole, GameConfig};
use crate::state::SharedGameWorld;

/// Returns all ship class API routes
pub fn routes() -> Vec<Route> {
    routes![get_ship_classes, get_ship_class]
}

/// Response for a single ship class with full details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipClassResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size: ShipSize,
    pub role: ShipClassRole,
    
    // Build constraints
    pub max_weight: f32,
    pub max_modules: u32,
    pub base_hull: f32,
    pub base_shields: f32,
    pub build_points: f32,
    
    // Bonuses formatted by category
    pub bonuses: HashMap<String, Vec<FormattedBonus>>,
    
    // Technical specifications
    pub technical_specs: HashMap<String, String>,
    
    // Faction-specific manufacturers
    pub manufacturers: HashMap<String, ManufacturerInfo>,
    
    // Lore and flavor
    pub lore: Option<String>,
    pub year_introduced: Option<u32>,
    pub notable_ships: Vec<String>,
}

/// Manufacturer information for API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufacturerInfo {
    pub manufacturer: String,
    pub variant: Option<String>,
    pub lore: Option<String>,
}

/// Summary response for ship class list (lighter than full details)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipClassSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size: ShipSize,
    pub role: ShipClassRole,
    pub max_weight: f32,
    pub max_modules: u32,
    pub build_points: f32,
}

/// Get all available ship classes
///
/// Optional query parameters:
/// - faction: Filter by faction ID to only show ships with that faction's manufacturer
///
/// # Example
///
/// ```
/// GET /v1/ship-classes
/// GET /v1/ship-classes?faction=terran-federation
/// ```
#[get("/v1/ship-classes?<faction>")]
pub fn get_ship_classes(
    config: &State<GameConfig>,
    faction: Option<String>,
) -> Json<Vec<ShipClassSummary>> {
    let mut ship_classes: Vec<ShipClassSummary> = config
        .ship_classes
        .iter()
        .filter(|sc| {
            // If faction filter is provided, only include ships with that faction's manufacturer
            if let Some(ref faction_id) = faction {
                sc.manufacturers.contains_key(faction_id)
            } else {
                true
            }
        })
        .map(|sc| ShipClassSummary {
            id: sc.id.clone(),
            name: sc.name.clone(),
            description: sc.description.clone(),
            size: sc.size,
            role: sc.role,
            max_weight: sc.max_weight,
            max_modules: sc.max_modules,
            build_points: sc.build_points,
        })
        .collect();
    
    // Sort by build points for consistent ordering
    ship_classes.sort_by(|a, b| a.build_points.partial_cmp(&b.build_points).unwrap());
    
    Json(ship_classes)
}

/// Get detailed information for a specific ship class
///
/// # Arguments
///
/// * `id` - Ship class ID (e.g., "frigate", "cruiser")
///
/// # Example
///
/// ```
/// GET /v1/ship-classes/frigate
/// ```
#[get("/v1/ship-classes/<id>")]
pub fn get_ship_class(
    config: &State<GameConfig>,
    id: &str,
) -> Option<Json<ShipClassResponse>> {
    let ship_class = config.get_ship_class(&id)?;
    
    // Format bonuses using bonus metadata if available
    let bonuses = if let Some(ref bonus_config) = config.bonuses {
        ship_class.get_formatted_bonuses(bonus_config)
    } else {
        // Fallback to simple formatting without metadata
        HashMap::new()
    };
    
    // Convert manufacturers to API format
    let manufacturers: HashMap<String, ManufacturerInfo> = ship_class
        .manufacturers
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                ManufacturerInfo {
                    manufacturer: v.manufacturer.clone(),
                    variant: v.variant.clone(),
                    lore: v.lore.clone(),
                },
            )
        })
        .collect();
    
    Some(Json(ShipClassResponse {
        id: ship_class.id.clone(),
        name: ship_class.name.clone(),
        description: ship_class.description.clone(),
        size: ship_class.size,
        role: ship_class.role,
        max_weight: ship_class.max_weight,
        max_modules: ship_class.max_modules,
        base_hull: ship_class.base_hull,
        base_shields: ship_class.base_shields,
        build_points: ship_class.build_points,
        bonuses,
        technical_specs: ship_class.get_technical_specs(),
        manufacturers,
        lore: ship_class.lore.clone(),
        year_introduced: ship_class.year_introduced,
        notable_ships: ship_class.notable_ships.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GameConfig, ShipClassConfig, ShipSize, ShipClassRole};
    use std::sync::Mutex;
    use rocket::local::blocking::Client;
    use rocket::{routes, Build, Rocket};

    fn create_test_rocket() -> Rocket<Build> {
        let mut ship_class = ShipClassConfig {
            name: "Test Frigate".to_string(),
            description: "A test ship".to_string(),
            size: ShipSize::Medium,
            role: ShipClassRole::Combat,
            max_weight: 270.0,
            max_modules: 15,
            base_hull: 420.0,
            base_shields: 200.0,
            build_points: 640.0,
            bonuses: HashMap::new(),
            id: String::new(),
            manufacturers: HashMap::new(),
            length: Some(150.0),
            width: Some(45.0),
            height: Some(30.0),
            mass: Some(50000.0),
            crew_min: Some(25),
            crew_max: Some(40),
            cargo_capacity: Some(500.0),
            max_acceleration: Some(35.0),
            max_turn_rate: Some(25.0),
            max_warp_speed: Some(5.0),
            warp_efficiency: Some(0.75),
            sensor_range: Some(25000.0),
            operational_range: Some(15.0),
            build_time: Some(180),
            maintenance_cost: Some(500.0),
            fuel_capacity: Some(10000.0),
            fuel_consumption: Some(100.0),
            lore: Some("A versatile test ship".to_string()),
            year_introduced: Some(2350),
            notable_ships: vec!["USS Test".to_string()],
        };
        ship_class.set_id("test-frigate".to_string());

        // Note: We'd need to create a proper GameConfig here for full testing
        // This is simplified for demonstration
        
        rocket::build()
            .mount("/v1", routes![get_ship_classes, get_ship_class])
    }

    #[test]
    fn test_ship_class_response_structure() {
        let response = ShipClassResponse {
            id: "frigate".to_string(),
            name: "Frigate".to_string(),
            description: "Test".to_string(),
            size: ShipSize::Medium,
            role: ShipClassRole::Defense,
            max_weight: 270.0,
            max_modules: 15,
            base_hull: 420.0,
            base_shields: 200.0,
            build_points: 640.0,
            bonuses: HashMap::new(),
            technical_specs: HashMap::new(),
            manufacturers: HashMap::new(),
            lore: None,
            year_introduced: None,
            notable_ships: Vec::new(),
        };
        
        assert_eq!(response.id, "frigate");
        assert_eq!(response.name, "Frigate");
    }
}
