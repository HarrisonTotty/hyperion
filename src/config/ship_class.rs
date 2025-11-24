//! Ship class configuration
//!
//! Defines ship class specifications and constraints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::config::bonus::{BonusConfig, FormattedBonus};

/// Ship size categories
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShipSize {
    Small,
    Medium,
    Large,
}

/// Ship role categories
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShipClassRole {
    Versatile,
    Combat,
    Support,
    Transport,
    Exploration,
    Offense,
    Defense,
}

/// Ship class configuration from YAML files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipClassConfig {
    /// Display name
    pub name: String,
    /// Description of the ship class (field name 'desc' in YAML)
    #[serde(rename = "desc")]
    pub description: String,
    /// Ship size category
    pub size: ShipSize,
    /// Ship role category
    pub role: ShipClassRole,
    /// Maximum weight in kg the ship can support
    pub max_weight: f32,
    /// Maximum number of modules that can be equipped
    pub max_modules: u32,
    /// Base hull integrity (field name 'base_hp' in YAML)
    #[serde(rename = "base_hp")]
    pub base_hull: f32,
    /// Build points available for this ship class
    pub build_points: f32,
    /// Bonuses provided by this ship class
    #[serde(default)]
    pub bonuses: HashMap<String, f32>,
    
    // Derived field (not in YAML, populated from filename)
    #[serde(skip)]
    pub id: String,
    
    // Default to 0 if not specified in YAML
    #[serde(default)]
    pub base_shields: f32,
    
    // ========== Phase 2.6: Enhanced Fields ==========
    
    // Faction-Specific Data
    /// Primary manufacturer by faction ID
    #[serde(default)]
    pub manufacturers: HashMap<String, FactionManufacturer>,
    
    // Technical Specifications
    /// Length in meters
    #[serde(default)]
    pub length: Option<f32>,
    /// Width in meters
    #[serde(default)]
    pub width: Option<f32>,
    /// Height in meters
    #[serde(default)]
    pub height: Option<f32>,
    /// Mass in metric tons (empty)
    #[serde(default)]
    pub mass: Option<f32>,
    /// Typical crew complement
    #[serde(default)]
    pub crew_min: Option<u32>,
    /// Maximum crew capacity
    #[serde(default)]
    pub crew_max: Option<u32>,
    /// Cargo capacity in cubic meters
    #[serde(default)]
    pub cargo_capacity: Option<f32>,
    
    // Performance Characteristics
    /// Maximum sublight acceleration in m/s²
    #[serde(default)]
    pub max_acceleration: Option<f32>,
    /// Maximum turn rate in degrees/second
    #[serde(default)]
    pub max_turn_rate: Option<f32>,
    /// Maximum warp speed (multiples of light speed)
    #[serde(default)]
    pub max_warp_speed: Option<f32>,
    /// Warp drive efficiency rating (0.0-1.0)
    #[serde(default)]
    pub warp_efficiency: Option<f32>,
    /// Sensor range in kilometers
    #[serde(default)]
    pub sensor_range: Option<f32>,
    
    // Operational Metadata
    /// Typical operational range in AU
    #[serde(default)]
    pub operational_range: Option<f32>,
    /// Construction time in days
    #[serde(default)]
    pub build_time: Option<u32>,
    /// Maintenance cost per day
    #[serde(default)]
    pub maintenance_cost: Option<f32>,
    /// Fuel capacity in units
    #[serde(default)]
    pub fuel_capacity: Option<f32>,
    /// Fuel consumption rate per hour
    #[serde(default)]
    pub fuel_consumption: Option<f32>,
    
    // Lore and Flavor
    /// Historical background or design notes
    #[serde(default)]
    pub lore: Option<String>,
    /// Year of introduction (in-universe)
    #[serde(default)]
    pub year_introduced: Option<u32>,
    /// Notable ships of this class
    #[serde(default)]
    pub notable_ships: Vec<String>,
}

/// Faction-specific manufacturer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionManufacturer {
    /// Manufacturer name
    pub manufacturer: String,
    /// Faction-specific variant name
    pub variant: Option<String>,
    /// Faction-specific lore or design philosophy
    pub lore: Option<String>,
}

impl ShipClassConfig {
    /// Set the ID from the filename
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }
    
    /// Validate ship class configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Ship class ID cannot be empty".to_string());
        }
        if self.name.is_empty() {
            return Err("Ship class name cannot be empty".to_string());
        }
        if self.base_hull <= 0.0 {
            return Err(format!("Ship class {} must have positive base_hull", self.id));
        }
        if self.base_shields < 0.0 {
            return Err(format!("Ship class {} cannot have negative base_shields", self.id));
        }
        if self.max_weight <= 0.0 {
            return Err(format!("Ship class {} must have positive max_weight", self.id));
        }
        if self.max_modules == 0 {
            return Err(format!("Ship class {} must allow at least one module", self.id));
        }
        Ok(())
    }
    
    /// Get formatted bonuses grouped by category
    ///
    /// # Arguments
    ///
    /// * `bonus_config` - Reference to the bonus metadata configuration
    ///
    /// # Returns
    ///
    /// HashMap of category ID to list of formatted bonuses
    pub fn get_formatted_bonuses(
        &self,
        bonus_config: &BonusConfig,
    ) -> HashMap<String, Vec<FormattedBonus>> {
        bonus_config.format_bonuses_by_category(&self.bonuses)
    }
    
    /// Get technical specifications as a structured map
    pub fn get_technical_specs(&self) -> HashMap<String, String> {
        let mut specs = HashMap::new();
        
        if let Some(length) = self.length {
            specs.insert("Length".to_string(), format!("{:.1} m", length));
        }
        if let Some(width) = self.width {
            specs.insert("Width".to_string(), format!("{:.1} m", width));
        }
        if let Some(height) = self.height {
            specs.insert("Height".to_string(), format!("{:.1} m", height));
        }
        if let Some(mass) = self.mass {
            specs.insert("Mass".to_string(), format!("{:.0} tonnes", mass));
        }
        if let (Some(min), Some(max)) = (self.crew_min, self.crew_max) {
            specs.insert("Crew".to_string(), format!("{}-{}", min, max));
        }
        if let Some(cargo) = self.cargo_capacity {
            specs.insert("Cargo".to_string(), format!("{:.0} m³", cargo));
        }
        if let Some(accel) = self.max_acceleration {
            specs.insert("Max Acceleration".to_string(), format!("{:.1} m/s²", accel));
        }
        if let Some(turn) = self.max_turn_rate {
            specs.insert("Turn Rate".to_string(), format!("{:.1}°/s", turn));
        }
        if let Some(warp) = self.max_warp_speed {
            specs.insert("Max Warp".to_string(), format!("{:.1}c", warp));
        }
        if let Some(sensor) = self.sensor_range {
            specs.insert("Sensor Range".to_string(), format!("{:.0} km", sensor));
        }
        if let Some(range) = self.operational_range {
            specs.insert("Range".to_string(), format!("{:.1} AU", range));
        }
        
        specs
    }
    
    /// Get manufacturer info for a specific faction
    pub fn get_manufacturer(&self, faction_id: &str) -> Option<&FactionManufacturer> {
        self.manufacturers.get(faction_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ship_class_validation() {
        let mut valid = ShipClassConfig {
            name: "Cruiser".to_string(),
            description: "Medium warship".to_string(),
            size: ShipSize::Medium,
            role: ShipClassRole::Combat,
            base_hull: 5000.0,
            base_shields: 2500.0,
            max_weight: 50000.0,
            max_modules: 20,
            build_points: 500.0,
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
            notable_ships: Vec::new(),
        };
        valid.set_id("cruiser".to_string());
        assert!(valid.validate().is_ok());

        let mut invalid_hull = ShipClassConfig {
            base_hull: -100.0,
            ..valid.clone()
        };
        invalid_hull.set_id("test".to_string());
        assert!(invalid_hull.validate().is_err());

        let mut invalid_weight = ShipClassConfig {
            max_weight: 0.0,
            ..valid.clone()
        };
        invalid_weight.set_id("test2".to_string());
        assert!(invalid_weight.validate().is_err());
    }
}
