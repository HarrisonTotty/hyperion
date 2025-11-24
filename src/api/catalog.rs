//! Module Catalog API
//!
//! This module provides REST API endpoints for querying the ship module catalog,
//! including module slots, module variants, and ammunition types.
//!
//! ## Endpoints
//!
//! ### Module Slots
//! - `GET /v1/catalog/module-slots` - List all module slot IDs
//! - `GET /v1/catalog/module-slots/<slot_id>` - Get detailed slot information
//!
//! ### Module Variants
//! - `GET /v1/catalog/modules/<slot_id>` - List variant IDs for a slot
//! - `GET /v1/catalog/modules/<slot_id>/<module_id>` - Get detailed variant information
//!
//! ### Ammunition
//! - `GET /v1/catalog/ammo` - List ammunition categories
//! - `GET /v1/catalog/ammo/<category>` - List ammo IDs in category
//! - `GET /v1/catalog/ammo/<category>/<ammo_id>` - Get detailed ammo information

use rocket::{Route, State, get, routes};
use rocket::serde::json::Json;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use crate::config::{GameConfig, ModuleSlot, ModuleVariant, AmmunitionConfig};

/// Response containing a list of module slot IDs
#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleSlotListResponse {
    pub slots: Vec<String>,
}

/// Response containing a list of module variant IDs
#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleVariantListResponse {
    pub variants: Vec<String>,
}

/// Response containing a list of ammunition categories
#[derive(Debug, Serialize, Deserialize)]
pub struct AmmoCategoryListResponse {
    pub categories: Vec<String>,
}

/// Response containing a list of ammunition IDs
#[derive(Debug, Serialize, Deserialize)]
pub struct AmmoListResponse {
    pub ammunition: Vec<String>,
}

/// Error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// GET /v1/catalog/module-slots
///
/// Lists all available ship module slots.
///
/// # Returns
///
/// JSON array of module slot IDs.
///
/// # Example
///
/// ```json
/// {
///   "slots": [
///     "power-core",
///     "impulse-engine",
///     "shield-generator",
///     ...
///   ]
/// }
/// ```
#[get("/v1/catalog/module-slots")]
pub fn list_module_slots(config: &State<GameConfig>) -> Json<ModuleSlotListResponse> {
    let slots: Vec<String> = config.module_slots.keys().cloned().collect();
    Json(ModuleSlotListResponse { slots })
}

/// GET /v1/catalog/module-slots/<slot_id>
///
/// Gets detailed information about a specific module slot.
///
/// # Arguments
///
/// * `slot_id` - The module slot identifier (e.g., "impulse-engine")
///
/// # Returns
///
/// JSON object containing all module slot fields from the YAML definition.
///
/// # Errors
///
/// Returns 404 if the slot_id is not found.
///
/// # Example
///
/// ```json
/// {
///   "id": "impulse-engine",
///   "name": "Impulse Engine",
///   "desc": "Provides the ship with sublight propulsion.",
///   "extended_desc": "Different impulse engines provide varying levels...",
///   "groups": ["Essential", "Propulsion"],
///   "required": true,
///   "has_varients": true,
///   "base_cost": 15,
///   "max_slots": 3,
///   "base_hp": 20,
///   "base_power_consumption": 50.0,
///   "base_heat_generation": 30.0,
///   "base_weight": 500
/// }
/// ```
#[get("/v1/catalog/module-slots/<slot_id>")]
pub fn get_module_slot(
    slot_id: &str,
    config: &State<GameConfig>,
) -> Result<Json<ModuleSlot>, Json<ErrorResponse>> {
    match config.get_module_slot(&slot_id) {
        Some(slot) => Ok(Json(slot.clone())),
        None => Err(Json(ErrorResponse {
            error: format!("Module slot '{}' not found", slot_id),
        })),
    }
}

/// GET /v1/catalog/modules/<slot_id>
///
/// Lists all available module variants for a specific module slot.
///
/// # Arguments
///
/// * `slot_id` - The module slot identifier (e.g., "impulse-engine")
///
/// # Returns
///
/// JSON array of module variant IDs for the specified slot type.
///
/// # Errors
///
/// Returns 404 if the slot_id is not found.
/// Returns empty array if the slot exists but has no variants.
///
/// # Example
///
/// ```json
/// {
///   "variants": [
///     "ion-engines",
///     "plasma-induction-engines",
///     "scram-pulse-engines",
///     ...
///   ]
/// }
/// ```
#[get("/v1/catalog/modules/<slot_id>")]
pub fn list_module_variants(
    slot_id: String,
    config: &State<GameConfig>,
) -> Result<Json<ModuleVariantListResponse>, Json<ErrorResponse>> {
    // First verify the slot exists
    if !config.module_slots.contains_key(&slot_id) {
        return Err(Json(ErrorResponse {
            error: format!("Module slot '{}' not found", slot_id),
        }));
    }

    // Get variants for this slot type
    let variants: Vec<String> = config
        .module_variants
        .get(&slot_id)
        .map(|variants| variants.iter().map(|v| v.id.clone()).collect())
        .unwrap_or_default();

    Ok(Json(ModuleVariantListResponse { variants }))
}

/// GET /v1/catalog/modules/<slot_id>/<module_id>
///
/// Gets detailed information about a specific module variant.
///
/// # Arguments
///
/// * `slot_id` - The module slot identifier (e.g., "impulse-engine")
/// * `module_id` - The module variant identifier (e.g., "ion-engines")
///
/// # Returns
///
/// JSON object containing all module variant fields, including type-specific stats.
///
/// # Errors
///
/// Returns 404 if the slot_id or module_id is not found.
///
/// # Example
///
/// ```json
/// {
///   "id": "ion-engines",
///   "type": "impulse-engine",
///   "name": "Ion Engines",
///   "model": "IV8X",
///   "manufacturer": "Azul Deep Space Industries",
///   "desc": "Uses ion propulsion for efficient sublight travel.",
///   "lore": "Ion engines have been the workhorse...",
///   "cost": 250,
///   "additional_hp": 5,
///   "additional_power_consumption": 30.0,
///   "additional_heat_generation": 20.0,
///   "additional_weight": 200,
///   "stats": {
///     "max_thrust": 50000
///   }
/// }
/// ```
#[get("/v1/catalog/modules/<slot_id>/<module_id>")]
pub fn get_module_variant(
    slot_id: String,
    module_id: String,
    config: &State<GameConfig>,
) -> Result<Json<ModuleVariant>, Json<ErrorResponse>> {
    // First verify the slot exists
    if !config.module_slots.contains_key(&slot_id) {
        return Err(Json(ErrorResponse {
            error: format!("Module slot '{}' not found", slot_id),
        }));
    }

    // Get the specific variant
    match config.get_module_variant(&slot_id, &module_id) {
        Some(variant) => Ok(Json(variant.clone())),
        None => Err(Json(ErrorResponse {
            error: format!(
                "Module variant '{}' not found in slot '{}'",
                module_id, slot_id
            ),
        })),
    }
}

/// GET /v1/catalog/ammo
///
/// Returns list of all available ammunition categories.
///
/// # Returns
///
/// JSON array of ammunition category names.
///
/// # Example
///
/// ```json
/// {
///   "categories": ["kinetic", "missiles", "torpedos"]
/// }
/// ```
#[get("/v1/catalog/ammo")]
pub fn list_ammo_categories(config: &State<GameConfig>) -> Json<AmmoCategoryListResponse> {
    // Get all unique ammunition categories from the loaded ammunition
    let mut categories = std::collections::HashSet::new();
    
    // Scan ammunition configs to find categories
    // For now, return hardcoded categories since we know the structure
    // TODO: This could be dynamic based on what's actually loaded
    categories.insert("kinetic".to_string());
    categories.insert("missiles".to_string());
    categories.insert("torpedos".to_string());
    
    let mut category_list: Vec<String> = categories.into_iter().collect();
    category_list.sort();
    
    Json(AmmoCategoryListResponse {
        categories: category_list,
    })
}

/// GET /v1/catalog/ammo/<category>
///
/// Lists all ammunition types in a specific category.
///
/// # Arguments
///
/// * `category` - Ammunition category ("kinetic", "missiles", or "torpedos")
///
/// # Returns
///
/// JSON array of ammunition IDs for the specified category.
///
/// # Errors
///
/// Returns 404 if the category is not found.
///
/// # Example
///
/// ```json
/// {
///   "ammunition": [
///     "shell-100mm-st",
///     "shell-100mm-ap",
///     "shell-100mm-he",
///     ...
///   ]
/// }
/// ```
#[get("/v1/catalog/ammo/<category>")]
pub fn list_ammunition(
    category: String,
    config: &State<GameConfig>,
) -> Result<Json<AmmoListResponse>, Json<ErrorResponse>> {
    // Validate category
    let valid_categories = vec!["kinetic", "missiles", "torpedos"];
    if !valid_categories.contains(&category.as_str()) {
        return Err(Json(ErrorResponse {
            error: format!(
                "Invalid ammunition category '{}'. Valid categories: {}",
                category,
                valid_categories.join(", ")
            ),
        }));
    }

    // Get ammunition IDs for this category
    let ammunition: Vec<String> = config
        .ammunition_types
        .iter()
        .filter(|ammo| {
            // Filter by category based on the ammo ID pattern
            // This is a simple heuristic - could be improved with explicit category field
            match category.as_str() {
                "kinetic" => ammo.id.starts_with("shell-") || ammo.id.starts_with("slug-"),
                "missiles" => ammo.id.contains("missile") && !ammo.id.contains("torpedo"),
                "torpedos" => ammo.id.contains("torpedo"),
                _ => false,
            }
        })
        .map(|ammo| ammo.id.clone())
        .collect();

    Ok(Json(AmmoListResponse { ammunition }))
}

/// GET /v1/catalog/ammo/<category>/<ammo_id>
///
/// Gets detailed information about specific ammunition.
///
/// # Arguments
///
/// * `category` - Ammunition category ("kinetic", "missiles", or "torpedos")
/// * `ammo_id` - The ammunition identifier (e.g., "shell-100mm-ap")
///
/// # Returns
///
/// JSON object containing all ammunition fields.
///
/// # Errors
///
/// Returns 404 if the category or ammo_id is not found.
///
/// # Example
///
/// ```json
/// {
///   "id": "shell-100mm-ap",
///   "name": "100mm Armor-Piercing Shell",
///   "desc": "Armor-piercing shell designed to penetrate heavy plating.",
///   "cost": 25,
///   "weight": 15,
///   "impact_damage": 80,
///   "blast_radius": 2,
///   "blast_damage": 10,
///   "velocity": 1200,
///   "armor_penetration": 0.85
/// }
/// ```
#[get("/v1/catalog/ammo/<category>/<ammo_id>")]
pub fn get_ammunition(
    category: String,
    ammo_id: String,
    config: &State<GameConfig>,
) -> Result<Json<AmmunitionConfig>, Json<ErrorResponse>> {
    // Validate category
    let valid_categories = vec!["kinetic", "missiles", "torpedos"];
    if !valid_categories.contains(&category.as_str()) {
        return Err(Json(ErrorResponse {
            error: format!(
                "Invalid ammunition category '{}'. Valid categories: {}",
                category,
                valid_categories.join(", ")
            ),
        }));
    }

    // Find the ammunition
    match config
        .ammunition_types
        .iter()
        .find(|ammo| ammo.id == ammo_id)
    {
        Some(ammo) => {
            // Verify it matches the category
            let matches_category = match category.as_str() {
                "kinetic" => ammo.id.starts_with("shell-") || ammo.id.starts_with("slug-"),
                "missiles" => ammo.id.contains("missile") && !ammo.id.contains("torpedo"),
                "torpedos" => ammo.id.contains("torpedo"),
                _ => false,
            };

            if matches_category {
                Ok(Json(ammo.clone()))
            } else {
                Err(Json(ErrorResponse {
                    error: format!(
                        "Ammunition '{}' exists but is not in category '{}'",
                        ammo_id, category
                    ),
                }))
            }
        }
        None => Err(Json(ErrorResponse {
            error: format!("Ammunition '{}' not found", ammo_id),
        })),
    }
}

/// Returns all catalog API routes
pub fn routes() -> Vec<Route> {
    routes![
        list_module_slots,
        get_module_slot,
        list_module_variants,
        get_module_variant,
        list_ammo_categories,
        list_ammunition,
        get_ammunition,
    ]
}
