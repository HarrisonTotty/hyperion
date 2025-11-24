//! Modules API
//!
//! Provides endpoints for retrieving module catalog information.

use rocket::{Route, State, get, routes};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use crate::config::{GameConfig, ModuleVariant};

/// Module catalog entry for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleCatalogEntry {
    /// Unique module ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Module category (based on groups)
    pub category: String,
    /// Build points cost
    pub build_points: u32,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Technical specifications
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specs: Option<serde_json::Value>,
    /// Whether this module is required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Maximum number allowed to be installed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_allowed: Option<u32>,
    /// Whether this module uses the variant system
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_variants: Option<bool>,
    /// Base power consumption in MW
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_power_consumption: Option<f32>,
    /// Base heat generation in K
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_heat_generation: Option<f32>,
    /// Base weight in tons
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_weight: Option<f32>,
}

/// Response containing all available modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulesResponse {
    /// List of modules
    pub modules: Vec<ModuleCatalogEntry>,
    /// Total count
    pub count: usize,
}

/// Response containing module variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleVariantsResponse {
    /// Module type ID
    pub module_id: String,
    /// List of available variants
    pub variants: Vec<ModuleVariant>,
    /// Total count of variants
    pub count: usize,
}

/// Get all available modules
///
/// # Returns
///
/// Returns a list of all modules available in the game configuration.
#[get("/v1/modules")]
fn get_modules(config: &State<GameConfig>) -> Json<ModulesResponse> {
    let mut modules: Vec<ModuleCatalogEntry> = config
        .modules
        .modules
        .iter()
        .map(|(id, template)| {
            
            ModuleCatalogEntry {
                id: id.clone(),
                name: template.name.clone(),
                category: template.module_type.clone().to_lowercase(),
                build_points: template.cost,
                description: Some(template.description.clone()),
                specs: None, // Could be expanded with more details
                required: Some(template.required),
                max_allowed: Some(template.max_allowed),
                has_variants: template.has_variants,
                base_power_consumption: template.base_power_consumption,
                base_heat_generation: template.base_heat_generation,
                base_weight: template.base_weight,
            }
        })
        .collect();
    
    // Sort by category then name for consistent ordering
    modules.sort_by(|a, b| {
        a.category.cmp(&b.category)
            .then_with(|| a.name.cmp(&b.name))
    });
    
    let count = modules.len();
    
    Json(ModulesResponse { modules, count })
}

/// Determine module category from groups and name
///
/// Maps module groups to frontend category names.
fn determine_category(groups: &[String], name: &str) -> String {
    // Special case for maneuvering thrusters
    if name.to_lowercase().contains("maneuvering") || name.to_lowercase().contains("thruster") {
        return "maneuvering-thrusters".to_string();
    }
    
    for group in groups {
        match group.as_str() {
            "power" => return "power-cores".to_string(),
            "engines" => return "impulse-engines".to_string(),
            "deweapons" => return "energy-weapons".to_string(),
            "kweapons" => return "kinetic-weapons".to_string(),
            "missiles" => return "missile-weapons".to_string(),
            "defense" => return "countermeasures".to_string(),
            "comms" | "sensors" => return "comms-systems".to_string(),
            _ => {}
        }
    }
    
    // Check for secondary categories
    for group in groups {
        match group.as_str() {
            "weapons" => return "weapons".to_string(),
            "support" => return "support".to_string(),
            _ => {}
        }
    }
    
    // Default to support if no specific category found
    "support".to_string()
}

/// Get a specific module by ID
///
/// # Arguments
///
/// * `id` - The module ID to retrieve
///
/// # Returns
///
/// Returns the module if found, or a 404 error if not found.
#[get("/v1/modules/<id>")]
fn get_module(id: String, config: &State<GameConfig>) -> Option<Json<ModuleCatalogEntry>> {
    config.modules.modules.get(&id).map(|template| {
        
        Json(ModuleCatalogEntry {
            id: id.clone(),
            name: template.name.clone(),
            category: template.module_type.clone().to_lowercase(),
            build_points: template.cost,
            description: Some(template.description.clone()),
            specs: None,
            required: Some(template.required),
            max_allowed: Some(template.max_allowed),
            has_variants: template.has_variants,
            base_power_consumption: template.base_power_consumption,
            base_heat_generation: template.base_heat_generation,
            base_weight: template.base_weight,
        })
    })
}

/// Get available variants for a module type
///
/// # Arguments
///
/// * `module_id` - The module type ID (e.g., "shield-generators", "warp-cores")
///
/// # Returns
///
/// Returns a list of variants for the specified module type, or 404 if not found.
///
/// # Example
///
/// GET /v1/modules/shield-generators/variants
/// ```json
/// {
///   "module_id": "shield-generators",
///   "variants": [
///     {
///       "id": "light-shield-mk1",
///       "name": "Light Shield Generator Mk1",
///       "description": "Basic energy shield for small vessels...",
///       "cost": 50,
///       "stats": { ... }
///     }
///   ],
///   "count": 4
/// }
/// ```
#[get("/v1/modules/<module_id>/variants")]
fn get_module_variants(
    module_id: String,
    config: &State<GameConfig>,
) -> Option<Json<ModuleVariantsResponse>> {
    config.get_module_variants(&module_id).map(|variants| {
        Json(ModuleVariantsResponse {
            module_id: module_id.clone(),
            variants: variants.clone(),
            count: variants.len(),
        })
    })
}

/// Returns all module routes
pub fn routes() -> Vec<Route> {
    routes![get_modules, get_module, get_module_variants]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_category() {
        assert_eq!(determine_category(&["power".to_string()], "Power Module"), "power-cores");
        assert_eq!(determine_category(&["engines".to_string()], "Impulse Engines"), "impulse-engines");
        assert_eq!(determine_category(&["deweapons".to_string()], "Energy Weapon"), "energy-weapons");
        assert_eq!(determine_category(&["kweapons".to_string()], "Kinetic Weapon"), "kinetic-weapons");
        assert_eq!(determine_category(&["missiles".to_string()], "Missile Tube"), "missile-weapons");
        assert_eq!(determine_category(&["defense".to_string()], "Shield"), "countermeasures");
        assert_eq!(determine_category(&["comms".to_string()], "Comms"), "comms-systems");
        assert_eq!(determine_category(&["engines".to_string()], "Maneuvering Thrusters"), "maneuvering-thrusters");
        assert_eq!(determine_category(&["unknown".to_string()], "Unknown"), "support");
    }
}
