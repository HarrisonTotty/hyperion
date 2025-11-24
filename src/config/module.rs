//! Module configuration
//!
//! Defines ship module specifications and constraints.

use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Module slot definition
///
/// Represents a type of module slot that can be added to a ship.
/// Defined in `data/module-slots/*.yaml` files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSlot {
    /// Unique identifier for this module slot type
    pub id: String,
    
    /// Display name of the module slot
    pub name: String,
    
    /// Brief description of the module slot
    #[serde(rename = "desc")]
    pub description: String,
    
    /// Extended description for lore/details
    pub extended_desc: String,
    
    /// Groups for UI filtering and ship bonuses
    #[serde(default)]
    pub groups: Vec<String>,
    
    /// Whether at least one of this module type is required on a ship
    pub required: bool,
    
    /// Whether this module slot has different variants
    pub has_varients: bool,
    
    /// Base cost in build points to add this slot to a ship
    pub base_cost: i32,
    
    /// Maximum number of slots of this type allowed on a ship
    pub max_slots: i32,
    
    /// Base hit points allocated to a module of this type
    pub base_hp: i32,
    
    /// Base power consumption at 100% power, per second
    pub base_power_consumption: f32,
    
    /// Base heat generation at 100% power, per second
    pub base_heat_generation: f32,
    
    /// Base weight of the module slot, in kg
    pub base_weight: i32,
}

impl ModuleSlot {
    /// Validate module slot configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Module slot ID cannot be empty".to_string());
        }
        if self.name.is_empty() {
            return Err("Module slot name cannot be empty".to_string());
        }
        if self.base_cost < 0 {
            return Err(format!("Module slot {} cannot have negative base_cost", self.id));
        }
        if self.max_slots < 1 {
            return Err(format!("Module slot {} must allow at least 1 slot (max_slots >= 1)", self.id));
        }
        if self.base_hp < 0 {
            return Err(format!("Module slot {} cannot have negative base_hp", self.id));
        }
        if self.base_weight < 0 {
            return Err(format!("Module slot {} cannot have negative base_weight", self.id));
        }
        Ok(())
    }
}

/// Base module configuration fields common to all modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    /// Display name
    pub name: String,
    /// Model designation
    pub model: String,
    /// Module kind/category
    pub kind: String,
    /// Manufacturer
    pub manufacturer: String,
    /// Description (field name 'desc' in YAML)
    #[serde(rename = "desc")]
    pub description: String,
    /// Build cost
    pub cost: f32,
    /// Weight in kg
    pub weight: f32,
    
    // Power core specific fields
    #[serde(default)]
    pub max_energy: f32,
    #[serde(default)]
    pub production: f32,
    
    // Engine specific fields
    #[serde(default)]
    pub thrust: f32,
    #[serde(default)]
    pub energy_consumption: f32,
    
    // Derived field (not in YAML, populated from filename)
    #[serde(skip)]
    pub id: String,
}

impl ModuleConfig {
    /// Set the ID from the filename
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }
    
    /// Validate module configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Module ID cannot be empty".to_string());
        }
        if self.name.is_empty() {
            return Err("Module name cannot be empty".to_string());
        }
        if self.kind.is_empty() {
            return Err(format!("Module {} must have a kind", self.id));
        }
        if self.weight < 0.0 {
            return Err(format!("Module {} cannot have negative weight", self.id));
        }
        if self.cost < 0.0 {
            return Err(format!("Module {} cannot have negative cost", self.id));
        }
        Ok(())
    }
    
    /// Check if this is a power core module
    pub fn is_power_core(&self) -> bool {
        self.max_energy > 0.0 || self.production > 0.0
    }
    
    /// Check if this is an engine module
    pub fn is_engine(&self) -> bool {
        self.thrust > 0.0
    }
}

/// Flexible stats structure for module variants
///
/// Uses a HashMap to support arbitrary module-specific stats without
/// requiring schema changes for new module types.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleStats {
    #[serde(flatten)]
    pub stats: HashMap<String, serde_json::Value>,
}

impl ModuleStats {
    /// Get a stat value as f64
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.stats.get(key)?.as_f64()
    }
    
    /// Get a stat value as i64
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.stats.get(key)?.as_i64()
    }
    
    /// Get a stat value as String
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.stats.get(key)?.as_str().map(String::from)
    }
    
    /// Get a stat value as bool
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.stats.get(key)?.as_bool()
    }
    
    /// Check if a stat exists
    pub fn has(&self, key: &str) -> bool {
        self.stats.contains_key(key)
    }
}

/// Module variant definition
///
/// Represents a specific implementation of a module type (e.g., "Light Shield Mk1"
/// is a variant of the "shield-generator" module type).
///
/// Defined in `data/modules/**/*.yaml` files. The `type` field determines which
/// module slot this variant fits into.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleVariant {
    /// Unique identifier for this variant
    pub id: String,
    
    /// The kind of module slot this variant fits into (e.g., "power-core", "impulse-engine")
    /// This must match one of the module slot IDs defined in data/module-slots/*.yaml
    #[serde(rename = "type")]
    pub module_type: String,
    
    /// Display name - the "title" shown to users
    pub name: String,
    
    /// Model designation for lore/details
    pub model: String,
    
    /// Manufacturer name for lore/details
    pub manufacturer: String,
    
    /// Brief description (field name 'desc' in YAML)
    #[serde(rename = "desc")]
    pub description: String,
    
    /// Extended description/backstory for lore purposes
    pub lore: String,
    
    /// Build cost in build points
    pub cost: i32,
    
    /// Additional HP beyond base module HP
    pub additional_hp: i32,
    
    /// Additional power consumption beyond base (MW at 100% power, per second)
    pub additional_power_consumption: f32,
    
    /// Additional heat generation beyond base (K at 100% power, per second)
    pub additional_heat_generation: f32,
    
    /// Additional weight beyond base (kg)
    pub additional_weight: i32,
    
    /// Module-type-specific fields (energy_production, max_thrust, etc.)
    /// These are flattened into the root of the YAML for clean authoring
    #[serde(flatten)]
    pub stats: ModuleStats,
}

impl ModuleVariant {
    /// Validate module variant configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Module variant ID cannot be empty".to_string());
        }
        if self.module_type.is_empty() {
            return Err(format!("Module variant {} must have a type", self.id));
        }
        if self.name.is_empty() {
            return Err(format!("Module variant {} must have a name", self.id));
        }
        if self.cost < 0 {
            return Err(format!("Module variant {} cannot have negative cost", self.id));
        }
        Ok(())
    }
    
    /// Validate that all numeric fields are within reasonable ranges
    pub fn validate_numeric_ranges(&self) -> Result<(), String> {
        // Validate additional fields
        if self.additional_hp < 0 {
            return Err(format!("Module variant {} cannot have negative additional_hp", self.id));
        }
        if self.additional_power_consumption < 0.0 {
            return Err(format!("Module variant {} cannot have negative additional_power_consumption", self.id));
        }
        if self.additional_heat_generation < 0.0 {
            return Err(format!("Module variant {} cannot have negative additional_heat_generation", self.id));
        }
        if self.additional_weight < 0 {
            return Err(format!("Module variant {} cannot have negative additional_weight", self.id));
        }
        
        // Warn about suspiciously high values
        if self.cost > 100000 {
            warn!("Module variant {} has very high cost: {}", self.id, self.cost);
        }
        if self.additional_weight > 1000000 {
            warn!("Module variant {} has very high weight: {}", self.id, self.additional_weight);
        }
        
        Ok(())
    }
    
    /// Validate type-specific fields based on module_type
    pub fn validate_type_specific_fields(&self) -> Result<(), String> {
        match self.module_type.as_str() {
            "power-core" => self.validate_power_core_fields(),
            "impulse-engine" => self.validate_impulse_engine_fields(),
            "maneuvering-thruster" => self.validate_maneuvering_thruster_fields(),
            "shield-generator" => self.validate_shield_generator_fields(),
            "comms-system" => self.validate_comms_system_fields(),
            "cooling-system" => self.validate_cooling_system_fields(),
            "sensor-array" => self.validate_sensor_array_fields(),
            "stealth-system" => self.validate_stealth_system_fields(),
            "aux-support-system" => self.validate_aux_support_system_fields(),
            "warp-jump-core" => self.validate_warp_jump_core_fields(),
            "de-weapon" => self.validate_de_weapon_fields(),
            "kinetic-weapon" => self.validate_kinetic_weapon_fields(),
            "missile-launcher" => self.validate_missile_launcher_fields(),
            "radial-emission-system" => self.validate_radial_emission_system_fields(),
            // Module types with no variants don't need type-specific validation
            "cargo-bay" | "deflector-plating" | "torpedo-tube" | "countermeasure-system" => Ok(()),
            _ => {
                warn!("Unknown module type '{}' for variant {}", self.module_type, self.id);
                Ok(())
            }
        }
    }
    
    /// Validate power core specific fields
    fn validate_power_core_fields(&self) -> Result<(), String> {
        let energy_production = self.stats.get_i64("energy_production")
            .ok_or_else(|| format!("Power core {} missing required field 'energy_production'", self.id))?;
        let energy_capacity = self.stats.get_i64("energy_capacity")
            .ok_or_else(|| format!("Power core {} missing required field 'energy_capacity'", self.id))?;
        
        if energy_production <= 0 {
            return Err(format!("Power core {} must have positive energy_production", self.id));
        }
        if energy_capacity <= 0 {
            return Err(format!("Power core {} must have positive energy_capacity", self.id));
        }
        
        Ok(())
    }
    
    /// Validate impulse engine specific fields
    fn validate_impulse_engine_fields(&self) -> Result<(), String> {
        let max_thrust = self.stats.get_i64("max_thrust")
            .ok_or_else(|| format!("Impulse engine {} missing required field 'max_thrust'", self.id))?;
        
        if max_thrust <= 0 {
            return Err(format!("Impulse engine {} must have positive max_thrust", self.id));
        }
        
        Ok(())
    }
    
    /// Validate maneuvering thruster specific fields
    fn validate_maneuvering_thruster_fields(&self) -> Result<(), String> {
        let angular_thrust = self.stats.get_i64("angular_thrust")
            .ok_or_else(|| format!("Maneuvering thruster {} missing required field 'angular_thrust'", self.id))?;
        
        if angular_thrust <= 0 {
            return Err(format!("Maneuvering thruster {} must have positive angular_thrust", self.id));
        }
        
        Ok(())
    }
    
    /// Validate shield generator specific fields
    fn validate_shield_generator_fields(&self) -> Result<(), String> {
        let max_shield_strength = self.stats.get_i64("max_shield_strength")
            .ok_or_else(|| format!("Shield generator {} missing required field 'max_shield_strength'", self.id))?;
        let shield_recharge_rate = self.stats.get_i64("shield_recharge_rate")
            .ok_or_else(|| format!("Shield generator {} missing required field 'shield_recharge_rate'", self.id))?;
        
        if max_shield_strength <= 0 {
            return Err(format!("Shield generator {} must have positive max_shield_strength", self.id));
        }
        if shield_recharge_rate <= 0 {
            return Err(format!("Shield generator {} must have positive shield_recharge_rate", self.id));
        }
        
        Ok(())
    }
    
    /// Validate communications system specific fields
    fn validate_comms_system_fields(&self) -> Result<(), String> {
        let comm_range = self.stats.get_i64("comm_range")
            .ok_or_else(|| format!("Communications system {} missing required field 'comm_range'", self.id))?;
        let encryption_lvl = self.stats.get_i64("encryption_lvl")
            .ok_or_else(|| format!("Communications system {} missing required field 'encryption_lvl'", self.id))?;
        
        if comm_range <= 0 {
            return Err(format!("Communications system {} must have positive comm_range", self.id));
        }
        if encryption_lvl < 1 || encryption_lvl > 10 {
            return Err(format!("Communications system {} encryption_lvl must be between 1 and 10", self.id));
        }
        
        Ok(())
    }
    
    /// Validate cooling system specific fields
    fn validate_cooling_system_fields(&self) -> Result<(), String> {
        let maximum_coolant = self.stats.get_i64("maximum_coolant")
            .ok_or_else(|| format!("Cooling system {} missing required field 'maximum_coolant'", self.id))?;
        let generated_cooling = self.stats.get_i64("generated_cooling")
            .ok_or_else(|| format!("Cooling system {} missing required field 'generated_cooling'", self.id))?;
        
        if maximum_coolant <= 0 {
            return Err(format!("Cooling system {} must have positive maximum_coolant", self.id));
        }
        if generated_cooling <= 0 {
            return Err(format!("Cooling system {} must have positive generated_cooling", self.id));
        }
        
        Ok(())
    }
    
    /// Validate sensor array specific fields
    fn validate_sensor_array_fields(&self) -> Result<(), String> {
        let scan_range = self.stats.get_i64("scan_range")
            .ok_or_else(|| format!("Sensor array {} missing required field 'scan_range'", self.id))?;
        let detail_level = self.stats.get_i64("detail_level")
            .ok_or_else(|| format!("Sensor array {} missing required field 'detail_level'", self.id))?;
        let scan_time = self.stats.get_f64("scan_time")
            .ok_or_else(|| format!("Sensor array {} missing required field 'scan_time'", self.id))?;
        
        if scan_range <= 0 {
            return Err(format!("Sensor array {} must have positive scan_range", self.id));
        }
        if detail_level < 1 || detail_level > 10 {
            return Err(format!("Sensor array {} detail_level must be between 1 and 10", self.id));
        }
        if scan_time <= 0.0 {
            return Err(format!("Sensor array {} must have positive scan_time", self.id));
        }
        
        Ok(())
    }
    
    /// Validate stealth system specific fields
    fn validate_stealth_system_fields(&self) -> Result<(), String> {
        let detectability_reduction = self.stats.get_f64("detectability_reduction")
            .ok_or_else(|| format!("Stealth system {} missing required field 'detectability_reduction'", self.id))?;
        let scan_time_increase = self.stats.get_f64("scan_time_increase")
            .ok_or_else(|| format!("Stealth system {} missing required field 'scan_time_increase'", self.id))?;
        
        if detectability_reduction < 0.0 || detectability_reduction > 1.0 {
            return Err(format!("Stealth system {} detectability_reduction must be between 0.0 and 1.0", self.id));
        }
        if scan_time_increase < 0.0 {
            return Err(format!("Stealth system {} must have non-negative scan_time_increase", self.id));
        }
        
        Ok(())
    }
    
    /// Validate auxiliary support system specific fields
    fn validate_aux_support_system_fields(&self) -> Result<(), String> {
        let hp_regained = self.stats.get_i64("hp_regained")
            .ok_or_else(|| format!("Auxiliary support system {} missing required field 'hp_regained'", self.id))?;
        let energy_restored = self.stats.get_i64("energy_restored")
            .ok_or_else(|| format!("Auxiliary support system {} missing required field 'energy_restored'", self.id))?;
        let heat_dissipated = self.stats.get_i64("heat_dissipated")
            .ok_or_else(|| format!("Auxiliary support system {} missing required field 'heat_dissipated'", self.id))?;
        let num_uses = self.stats.get_i64("num_uses")
            .ok_or_else(|| format!("Auxiliary support system {} missing required field 'num_uses'", self.id))?;
        
        if hp_regained < 0 {
            return Err(format!("Auxiliary support system {} cannot have negative hp_regained", self.id));
        }
        if energy_restored < 0 {
            return Err(format!("Auxiliary support system {} cannot have negative energy_restored", self.id));
        }
        if heat_dissipated < 0 {
            return Err(format!("Auxiliary support system {} cannot have negative heat_dissipated", self.id));
        }
        if num_uses <= 0 {
            return Err(format!("Auxiliary support system {} must have positive num_uses", self.id));
        }
        
        Ok(())
    }
    
    /// Validate warp/jump core specific fields
    fn validate_warp_jump_core_fields(&self) -> Result<(), String> {
        let warp_type = self.stats.get_string("warp_type")
            .ok_or_else(|| format!("Warp/jump core {} missing required field 'warp_type'", self.id))?;
        let warp_delay = self.stats.get_f64("warp_delay")
            .ok_or_else(|| format!("Warp/jump core {} missing required field 'warp_delay'", self.id))?;
        
        if warp_type != "warp" && warp_type != "jump" {
            return Err(format!("Warp/jump core {} warp_type must be 'warp' or 'jump'", self.id));
        }
        if warp_delay <= 0.0 {
            return Err(format!("Warp/jump core {} must have positive warp_delay", self.id));
        }
        
        // If jump type, validate jump_distance
        if warp_type == "jump" {
            let jump_distance = self.stats.get_i64("jump_distance")
                .ok_or_else(|| format!("Jump core {} missing required field 'jump_distance'", self.id))?;
            if jump_distance <= 0 {
                return Err(format!("Jump core {} must have positive jump_distance", self.id));
            }
        }
        
        Ok(())
    }
    
    /// Validate directed energy weapon specific fields
    fn validate_de_weapon_fields(&self) -> Result<(), String> {
        let damage = self.stats.get_i64("damage")
            .ok_or_else(|| format!("DE weapon {} missing required field 'damage'", self.id))?;
        let recharge_time = self.stats.get_f64("recharge_time")
            .ok_or_else(|| format!("DE weapon {} missing required field 'recharge_time'", self.id))?;
        let max_range = self.stats.get_i64("max_range")
            .ok_or_else(|| format!("DE weapon {} missing required field 'max_range'", self.id))?;
        let projectile_speed = self.stats.get_i64("projectile_speed")
            .ok_or_else(|| format!("DE weapon {} missing required field 'projectile_speed'", self.id))?;
        
        if damage <= 0 {
            return Err(format!("DE weapon {} must have positive damage", self.id));
        }
        if recharge_time <= 0.0 {
            return Err(format!("DE weapon {} must have positive recharge_time", self.id));
        }
        if max_range <= 0 {
            return Err(format!("DE weapon {} must have positive max_range", self.id));
        }
        if projectile_speed <= 0 {
            return Err(format!("DE weapon {} must have positive projectile_speed", self.id));
        }
        
        Ok(())
    }
    
    /// Validate kinetic weapon specific fields
    fn validate_kinetic_weapon_fields(&self) -> Result<(), String> {
        let _ammo_type = self.stats.get_string("ammo_type")
            .ok_or_else(|| format!("Kinetic weapon {} missing required field 'ammo_type'", self.id))?;
        let _ammo_size = self.stats.get_string("ammo_size")
            .ok_or_else(|| format!("Kinetic weapon {} missing required field 'ammo_size'", self.id))?;
        let reload_time = self.stats.get_f64("reload_time")
            .ok_or_else(|| format!("Kinetic weapon {} missing required field 'reload_time'", self.id))?;
        let num_projectiles = self.stats.get_i64("num_projectiles")
            .ok_or_else(|| format!("Kinetic weapon {} missing required field 'num_projectiles'", self.id))?;
        let ammo_consumed = self.stats.get_i64("ammo_consumed")
            .ok_or_else(|| format!("Kinetic weapon {} missing required field 'ammo_consumed'", self.id))?;
        let accuracy = self.stats.get_f64("accuracy")
            .ok_or_else(|| format!("Kinetic weapon {} missing required field 'accuracy'", self.id))?;
        let effective_range = self.stats.get_i64("effective_range")
            .ok_or_else(|| format!("Kinetic weapon {} missing required field 'effective_range'", self.id))?;
        
        if reload_time <= 0.0 {
            return Err(format!("Kinetic weapon {} must have positive reload_time", self.id));
        }
        if num_projectiles <= 0 {
            return Err(format!("Kinetic weapon {} must have positive num_projectiles", self.id));
        }
        if ammo_consumed <= 0 {
            return Err(format!("Kinetic weapon {} must have positive ammo_consumed", self.id));
        }
        if accuracy <= 0.0 || accuracy > 1.0 {
            return Err(format!("Kinetic weapon {} accuracy must be between 0.0 and 1.0", self.id));
        }
        if effective_range <= 0 {
            return Err(format!("Kinetic weapon {} must have positive effective_range", self.id));
        }
        
        Ok(())
    }
    
    /// Validate missile launcher specific fields
    fn validate_missile_launcher_fields(&self) -> Result<(), String> {
        let reload_time = self.stats.get_f64("reload_time")
            .ok_or_else(|| format!("Missile launcher {} missing required field 'reload_time'", self.id))?;
        let missile_volley_delay = self.stats.get_f64("missile_volley_delay")
            .ok_or_else(|| format!("Missile launcher {} missing required field 'missile_volley_delay'", self.id))?;
        let num_launched = self.stats.get_i64("num_launched")
            .ok_or_else(|| format!("Missile launcher {} missing required field 'num_launched'", self.id))?;
        let ammo_capacity = self.stats.get_i64("ammo_capacity")
            .ok_or_else(|| format!("Missile launcher {} missing required field 'ammo_capacity'", self.id))?;
        
        if reload_time <= 0.0 {
            return Err(format!("Missile launcher {} must have positive reload_time", self.id));
        }
        if missile_volley_delay < 0.0 {
            return Err(format!("Missile launcher {} cannot have negative missile_volley_delay", self.id));
        }
        if num_launched <= 0 {
            return Err(format!("Missile launcher {} must have positive num_launched", self.id));
        }
        if ammo_capacity <= 0 {
            return Err(format!("Missile launcher {} must have positive ammo_capacity", self.id));
        }
        
        Ok(())
    }
    
    /// Validate radial emission system specific fields
    fn validate_radial_emission_system_fields(&self) -> Result<(), String> {
        let max_pulse_range = self.stats.get_i64("max_pulse_range")
            .ok_or_else(|| format!("Radial emission system {} missing required field 'max_pulse_range'", self.id))?;
        let pulse_speed = self.stats.get_i64("pulse_speed")
            .ok_or_else(|| format!("Radial emission system {} missing required field 'pulse_speed'", self.id))?;
        
        if max_pulse_range <= 0 {
            return Err(format!("Radial emission system {} must have positive max_pulse_range", self.id));
        }
        if pulse_speed <= 0 {
            return Err(format!("Radial emission system {} must have positive pulse_speed", self.id));
        }
        
        Ok(())
    }
    
    /// Extract power core specific fields
    pub fn as_power_core(&self) -> Option<PowerCoreFields> {
        if self.module_type != "power-core" {
            return None;
        }
        Some(PowerCoreFields {
            energy_production: self.stats.get_i64("energy_production")? as i32,
            energy_capacity: self.stats.get_i64("energy_capacity")? as i32,
        })
    }
    
    /// Extract impulse engine specific fields
    pub fn as_impulse_engine(&self) -> Option<ImpulseEngineFields> {
        if self.module_type != "impulse-engine" {
            return None;
        }
        Some(ImpulseEngineFields {
            max_thrust: self.stats.get_i64("max_thrust")? as i32,
        })
    }
    
    /// Extract maneuvering thruster specific fields
    pub fn as_maneuvering_thruster(&self) -> Option<ManeuveringThrusterFields> {
        if self.module_type != "maneuvering-thruster" {
            return None;
        }
        Some(ManeuveringThrusterFields {
            angular_thrust: self.stats.get_i64("angular_thrust")? as i32,
        })
    }
    
    /// Extract shield generator specific fields
    pub fn as_shield_generator(&self) -> Option<ShieldGeneratorFields> {
        if self.module_type != "shield-generator" {
            return None;
        }
        Some(ShieldGeneratorFields {
            max_shield_strength: self.stats.get_i64("max_shield_strength")? as i32,
            shield_recharge_rate: self.stats.get_i64("shield_recharge_rate")? as i32,
        })
    }
    
    /// Extract communications system specific fields
    pub fn as_comms_system(&self) -> Option<CommsSystemFields> {
        if self.module_type != "comms-system" {
            return None;
        }
        Some(CommsSystemFields {
            comm_range: self.stats.get_i64("comm_range")? as i32,
            encryption_lvl: self.stats.get_i64("encryption_lvl")? as i32,
        })
    }
    
    /// Extract cooling system specific fields
    pub fn as_cooling_system(&self) -> Option<CoolingSystemFields> {
        if self.module_type != "cooling-system" {
            return None;
        }
        Some(CoolingSystemFields {
            maximum_coolant: self.stats.get_i64("maximum_coolant")? as i32,
            generated_cooling: self.stats.get_i64("generated_cooling")? as i32,
        })
    }
    
    /// Extract sensor array specific fields
    pub fn as_sensor_array(&self) -> Option<SensorArrayFields> {
        if self.module_type != "sensor-array" {
            return None;
        }
        Some(SensorArrayFields {
            scan_range: self.stats.get_i64("scan_range")? as i32,
            detail_level: self.stats.get_i64("detail_level")? as i32,
            scan_time: self.stats.get_f64("scan_time")? as f32,
        })
    }
    
    /// Extract stealth system specific fields
    pub fn as_stealth_system(&self) -> Option<StealthSystemFields> {
        if self.module_type != "stealth-system" {
            return None;
        }
        Some(StealthSystemFields {
            detectability_reduction: self.stats.get_f64("detectability_reduction")? as f32,
            scan_time_increase: self.stats.get_f64("scan_time_increase")? as f32,
        })
    }
    
    /// Extract auxiliary support system specific fields
    pub fn as_aux_support_system(&self) -> Option<AuxSupportSystemFields> {
        if self.module_type != "aux-support-system" {
            return None;
        }
        Some(AuxSupportSystemFields {
            hp_regained: self.stats.get_i64("hp_regained")? as i32,
            energy_restored: self.stats.get_i64("energy_restored")? as i32,
            heat_dissipated: self.stats.get_i64("heat_dissipated")? as i32,
            num_uses: self.stats.get_i64("num_uses")? as i32,
        })
    }
    
    /// Extract warp/jump core specific fields
    pub fn as_warp_jump_core(&self) -> Option<WarpJumpCoreFields> {
        if self.module_type != "warp-jump-core" {
            return None;
        }
        let warp_type_str = self.stats.get_string("warp_type")?;
        let warp_type = match warp_type_str.as_str() {
            "warp" => WarpType::Warp,
            "jump" => WarpType::Jump,
            _ => return None,
        };
        
        Some(WarpJumpCoreFields {
            warp_type,
            warp_delay: self.stats.get_f64("warp_delay")? as f32,
            jump_distance: self.stats.get_i64("jump_distance").map(|v| v as i32),
        })
    }
}

// Type-specific field structures for better type safety

/// Power core specific fields (type: power-core)
#[derive(Debug, Clone)]
pub struct PowerCoreFields {
    /// Amount of energy produced per second
    pub energy_production: i32,
    /// Total energy capacity
    pub energy_capacity: i32,
}

/// Impulse engine specific fields (type: impulse-engine)
#[derive(Debug, Clone)]
pub struct ImpulseEngineFields {
    /// Maximum thrust output at 100% power (Newtons)
    pub max_thrust: i32,
}

/// Maneuvering thruster specific fields (type: maneuvering-thruster)
#[derive(Debug, Clone)]
pub struct ManeuveringThrusterFields {
    /// Amount of angular thrust at 100% power (Newtons)
    pub angular_thrust: i32,
}

/// Shield generator specific fields (type: shield-generator)
#[derive(Debug, Clone)]
pub struct ShieldGeneratorFields {
    /// Maximum shield strength
    pub max_shield_strength: i32,
    /// Shield recharge rate when not taking damage (points per second at 100% power)
    pub shield_recharge_rate: i32,
}

/// Communications system specific fields (type: comms-system)
#[derive(Debug, Clone)]
pub struct CommsSystemFields {
    /// Effective range at 100% power (meters)
    pub comm_range: i32,
    /// Encryption level (1-10)
    pub encryption_lvl: i32,
}

/// Cooling system specific fields (type: cooling-system)
#[derive(Debug, Clone)]
pub struct CoolingSystemFields {
    /// Maximum coolant capable of being allocated at 100% power
    pub maximum_coolant: i32,
    /// Amount of coolant generated per second at 100% power
    pub generated_cooling: i32,
}

/// Sensor array specific fields (type: sensor-array)
#[derive(Debug, Clone)]
pub struct SensorArrayFields {
    /// Scan range at 100% power (meters)
    pub scan_range: i32,
    /// Detail level (1-10)
    pub detail_level: i32,
    /// Time to scan individual objects at 100% power (seconds)
    pub scan_time: f32,
}

/// Stealth system specific fields (type: stealth-system)
#[derive(Debug, Clone)]
pub struct StealthSystemFields {
    /// Reduction in detectability (percentage, e.g., 0.5 = 50%)
    pub detectability_reduction: f32,
    /// Increase in enemy scan time at 100% power (seconds)
    pub scan_time_increase: f32,
}

/// Auxiliary support system specific fields (type: aux-support-system)
#[derive(Debug, Clone)]
pub struct AuxSupportSystemFields {
    /// HP regained when "using" the module
    pub hp_regained: i32,
    /// Energy restored when "using" the module
    pub energy_restored: i32,
    /// Heat dissipated when "using" the module
    pub heat_dissipated: i32,
    /// Number of times the module can be used
    pub num_uses: i32,
}

/// Warp/Jump core specific fields (type: warp-jump-core)
#[derive(Debug, Clone)]
pub struct WarpJumpCoreFields {
    /// Type of FTL travel
    pub warp_type: WarpType,
    /// Time to engage drive at 100% power (seconds)
    pub warp_delay: f32,
    /// Maximum jump distance at 100% power (km) - only for jump type
    pub jump_distance: Option<i32>,
}

/// Type of FTL travel
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarpType {
    /// Accelerates to FTL speeds
    Warp,
    /// Instantaneous teleport
    Jump,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_validation() {
        let mut valid = ModuleConfig {
            name: "Test Reactor".to_string(),
            model: "Mark III".to_string(),
            kind: "fission".to_string(),
            manufacturer: "Test Corp".to_string(),
            description: "A test power core".to_string(),
            cost: 1000.0,
            weight: 500.0,
            max_energy: 1000.0,
            production: 100.0,
            thrust: 0.0,
            energy_consumption: 0.0,
            id: String::new(),
        };
        valid.set_id("test-reactor".to_string());
        assert!(valid.validate().is_ok());
        assert!(valid.is_power_core());
        assert!(!valid.is_engine());
    }

    #[test]
    fn test_empty_id() {
        let invalid = ModuleConfig {
            name: "Test".to_string(),
            model: "v1".to_string(),
            kind: "test".to_string(),
            manufacturer: "Test".to_string(),
            description: "test".to_string(),
            cost: 0.0,
            weight: 0.0,
            max_energy: 0.0,
            production: 0.0,
            thrust: 0.0,
            energy_consumption: 0.0,
            id: String::new(),
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_negative_weight() {
        let mut invalid = ModuleConfig {
            name: "Test".to_string(),
            model: "v1".to_string(),
            kind: "test".to_string(),
            manufacturer: "Test".to_string(),
            description: "test".to_string(),
            cost: 0.0,
            weight: -1.0,
            max_energy: 0.0,
            production: 0.0,
            thrust: 0.0,
            energy_consumption: 0.0,
            id: String::new(),
        };
        invalid.set_id("test".to_string());
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_negative_power_draw() {
        let mut invalid = ModuleConfig {
            name: "Test".to_string(),
            model: "v1".to_string(),
            kind: "test".to_string(),
            manufacturer: "Test".to_string(),
            description: "test".to_string(),
            cost: -1.0,
            weight: 0.0,
            max_energy: 0.0,
            production: 0.0,
            thrust: 0.0,
            energy_consumption: 0.0,
            id: String::new(),
        };
        invalid.set_id("test".to_string());
        assert!(invalid.validate().is_err());
    }
    
    #[test]
    fn test_module_variant_enhanced_fields() {
        // Test that ModuleVariant can deserialize with all required fields per spec
        let yaml_complete = r#"
id: "advanced-reactor"
type: "power-core"
name: "Advanced Fusion Reactor"
model: "AFR-2000"
manufacturer: "Mars Heavy Industries"
desc: >-
  High-performance fusion reactor for capital ships.
lore: >-
  Developed by the Martian Collective in 2187, this reactor represents
  the cutting edge of fusion technology.
cost: 500
additional_hp: 20
additional_power_consumption: 0.0
additional_heat_generation: 30.0
additional_weight: 15
energy_production: 150
energy_capacity: 2000
"#;
        
        let variant: Result<ModuleVariant, _> = serde_yaml::from_str(yaml_complete);
        assert!(variant.is_ok(), "Failed to parse variant: {:?}", variant.err());
        
        let variant = variant.unwrap();
        assert_eq!(variant.id, "advanced-reactor");
        assert_eq!(variant.module_type, "power-core");
        assert_eq!(variant.name, "Advanced Fusion Reactor");
        assert_eq!(variant.model, "AFR-2000");
        assert_eq!(variant.manufacturer, "Mars Heavy Industries");
        assert!(variant.description.contains("High-performance"));
        assert!(variant.lore.contains("Martian Collective"));
        assert_eq!(variant.cost, 500);
        assert_eq!(variant.additional_hp, 20);
        assert_eq!(variant.additional_power_consumption, 0.0);
        assert_eq!(variant.additional_heat_generation, 30.0);
        assert_eq!(variant.additional_weight, 15);
        
        // Check type-specific fields are in stats
        assert!(variant.stats.has("energy_production"));
        assert!(variant.stats.has("energy_capacity"));
        assert_eq!(variant.stats.get_i64("energy_production"), Some(150));
        assert_eq!(variant.stats.get_i64("energy_capacity"), Some(2000));
        
        // Test validation
        assert!(variant.validate().is_ok());
    }
    
    #[test]
    fn test_module_variant_validation() {
        let valid_variant = ModuleVariant {
            id: "test-variant".to_string(),
            module_type: "power-core".to_string(),
            name: "Test Variant".to_string(),
            model: "TV-100".to_string(),
            manufacturer: "Test Corp".to_string(),
            description: "A test variant".to_string(),
            lore: "Test lore".to_string(),
            cost: 100,
            additional_hp: 10,
            additional_power_consumption: 5.0,
            additional_heat_generation: 3.0,
            additional_weight: 20,
            stats: ModuleStats::default(),
        };
        
        assert!(valid_variant.validate().is_ok());
    }
    
    #[test]
    fn test_module_variant_validation_empty_id() {
        let invalid = ModuleVariant {
            id: String::new(),
            module_type: "power-core".to_string(),
            name: "Test".to_string(),
            model: "T".to_string(),
            manufacturer: "T".to_string(),
            description: "T".to_string(),
            lore: "T".to_string(),
            cost: 0,
            additional_hp: 0,
            additional_power_consumption: 0.0,
            additional_heat_generation: 0.0,
            additional_weight: 0,
            stats: ModuleStats::default(),
        };
        
        assert!(invalid.validate().is_err());
        assert!(invalid.validate().unwrap_err().contains("ID cannot be empty"));
    }
    
    #[test]
    fn test_module_variant_validation_negative_cost() {
        let invalid = ModuleVariant {
            id: "test".to_string(),
            module_type: "power-core".to_string(),
            name: "Test".to_string(),
            model: "T".to_string(),
            manufacturer: "T".to_string(),
            description: "T".to_string(),
            lore: "T".to_string(),
            cost: -100,
            additional_hp: 0,
            additional_power_consumption: 0.0,
            additional_heat_generation: 0.0,
            additional_weight: 0,
            stats: ModuleStats::default(),
        };
        
        assert!(invalid.validate().is_err());
        assert!(invalid.validate().unwrap_err().contains("negative cost"));
    }
    
    #[test]
    fn test_module_slot_deserialization() {
        // Test that ModuleSlot can deserialize from YAML
        let yaml = r#"
id: power-core
name: "Power Core"
desc: >-
  Provides the ship with power generation and capacity.
extended_desc: >-
  Different power cores provide varying levels of power output and
  capacity. The power core is essential for running all ship systems,
  and its performance directly affects the ship's overall capabilities.
groups: ["Essential", "Power", "Support"]
required: true
has_varients: true
base_cost: 10
max_slots: 2
base_hp: 10
base_power_consumption: 0.0
base_heat_generation: 5.0
base_weight: 100
"#;
        
        let slot: ModuleSlot = serde_yaml::from_str(yaml).expect("Failed to deserialize ModuleSlot");
        
        assert_eq!(slot.id, "power-core");
        assert_eq!(slot.name, "Power Core");
        assert!(slot.description.contains("power generation"));
        assert!(slot.extended_desc.contains("power output"));
        assert_eq!(slot.groups.len(), 3);
        assert_eq!(slot.groups[0], "Essential");
        assert_eq!(slot.required, true);
        assert_eq!(slot.has_varients, true);
        assert_eq!(slot.base_cost, 10);
        assert_eq!(slot.max_slots, 2);
        assert_eq!(slot.base_hp, 10);
        assert_eq!(slot.base_power_consumption, 0.0);
        assert_eq!(slot.base_heat_generation, 5.0);
        assert_eq!(slot.base_weight, 100);
    }
    
    #[test]
    fn test_module_slot_validation() {
        let valid_slot = ModuleSlot {
            id: "test-slot".to_string(),
            name: "Test Slot".to_string(),
            description: "A test module slot".to_string(),
            extended_desc: "Extended description".to_string(),
            groups: vec!["Test".to_string()],
            required: false,
            has_varients: true,
            base_cost: 100,
            max_slots: 4,
            base_hp: 50,
            base_power_consumption: 10.0,
            base_heat_generation: 5.0,
            base_weight: 200,
        };
        
        assert!(valid_slot.validate().is_ok());
    }
    
    #[test]
    fn test_module_slot_validation_empty_id() {
        let invalid_slot = ModuleSlot {
            id: String::new(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            extended_desc: "Test".to_string(),
            groups: vec![],
            required: false,
            has_varients: false,
            base_cost: 0,
            max_slots: 1,
            base_hp: 0,
            base_power_consumption: 0.0,
            base_heat_generation: 0.0,
            base_weight: 0,
        };
        
        assert!(invalid_slot.validate().is_err());
        assert!(invalid_slot.validate().unwrap_err().contains("ID cannot be empty"));
    }
    
    #[test]
    fn test_module_slot_validation_negative_cost() {
        let invalid_slot = ModuleSlot {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            extended_desc: "Test".to_string(),
            groups: vec![],
            required: false,
            has_varients: false,
            base_cost: -10,
            max_slots: 1,
            base_hp: 0,
            base_power_consumption: 0.0,
            base_heat_generation: 0.0,
            base_weight: 0,
        };
        
        assert!(invalid_slot.validate().is_err());
        assert!(invalid_slot.validate().unwrap_err().contains("negative base_cost"));
    }
    
    #[test]
    fn test_module_slot_validation_zero_max_slots() {
        let invalid_slot = ModuleSlot {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            extended_desc: "Test".to_string(),
            groups: vec![],
            required: false,
            has_varients: false,
            base_cost: 0,
            max_slots: 0,
            base_hp: 0,
            base_power_consumption: 0.0,
            base_heat_generation: 0.0,
            base_weight: 0,
        };
        
        assert!(invalid_slot.validate().is_err());
        assert!(invalid_slot.validate().unwrap_err().contains("at least 1 slot"));
    }
    
    #[test]
    fn test_power_core_extraction() {
        let yaml = r#"
id: "test-reactor"
type: "power-core"
name: "Test Reactor"
model: "TR-100"
manufacturer: "Test Corp"
desc: "A test reactor"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 20
energy_production: 150
energy_capacity: 2000
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        let fields = variant.as_power_core().expect("Should extract power core fields");
        
        assert_eq!(fields.energy_production, 150);
        assert_eq!(fields.energy_capacity, 2000);
    }
    
    #[test]
    fn test_impulse_engine_extraction() {
        let yaml = r#"
id: "test-engine"
type: "impulse-engine"
name: "Test Engine"
model: "TE-100"
manufacturer: "Test Corp"
desc: "A test engine"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 50.0
additional_heat_generation: 40.0
additional_weight: 30
max_thrust: 50000
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        let fields = variant.as_impulse_engine().expect("Should extract impulse engine fields");
        
        assert_eq!(fields.max_thrust, 50000);
    }
    
    #[test]
    fn test_maneuvering_thruster_extraction() {
        let yaml = r#"
id: "test-thruster"
type: "maneuvering-thruster"
name: "Test Thruster"
model: "TT-100"
manufacturer: "Test Corp"
desc: "A test thruster"
lore: "Test lore"
cost: 100
additional_hp: 0
additional_power_consumption: 10.0
additional_heat_generation: 5.0
additional_weight: 10
angular_thrust: 100000
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        let fields = variant.as_maneuvering_thruster().expect("Should extract thruster fields");
        
        assert_eq!(fields.angular_thrust, 100000);
    }
    
    #[test]
    fn test_shield_generator_extraction() {
        let yaml = r#"
id: "test-shield"
type: "shield-generator"
name: "Test Shield"
model: "TS-100"
manufacturer: "Test Corp"
desc: "A test shield"
lore: "Test lore"
cost: 100
additional_hp: 0
additional_power_consumption: 80.0
additional_heat_generation: 60.0
additional_weight: 15
max_shield_strength: 10000
shield_recharge_rate: 500
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        let fields = variant.as_shield_generator().expect("Should extract shield fields");
        
        assert_eq!(fields.max_shield_strength, 10000);
        assert_eq!(fields.shield_recharge_rate, 500);
    }
    
    #[test]
    fn test_comms_system_extraction() {
        let yaml = r#"
id: "test-comms"
type: "comms-system"
name: "Test Comms"
model: "TC-100"
manufacturer: "Test Corp"
desc: "A test comms system"
lore: "Test lore"
cost: 100
additional_hp: 0
additional_power_consumption: 10.0
additional_heat_generation: 5.0
additional_weight: 10
comm_range: 1000000
encryption_lvl: 5
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        let fields = variant.as_comms_system().expect("Should extract comms fields");
        
        assert_eq!(fields.comm_range, 1000000);
        assert_eq!(fields.encryption_lvl, 5);
    }
    
    #[test]
    fn test_aux_support_system_extraction() {
        let yaml = r#"
id: "test-aux"
type: "aux-support-system"
name: "Test Aux System"
model: "TA-100"
manufacturer: "Test Corp"
desc: "A test aux system"
lore: "Test lore"
cost: 50
additional_hp: 0
additional_power_consumption: 0.0
additional_heat_generation: 0.0
additional_weight: 5
hp_regained: 100
energy_restored: 200
heat_dissipated: 50
num_uses: 5
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        let fields = variant.as_aux_support_system().expect("Should extract aux system fields");
        
        assert_eq!(fields.hp_regained, 100);
        assert_eq!(fields.energy_restored, 200);
        assert_eq!(fields.heat_dissipated, 50);
        assert_eq!(fields.num_uses, 5);
    }
    
    #[test]
    fn test_warp_jump_core_extraction_warp_type() {
        let yaml = r#"
id: "test-warp"
type: "warp-jump-core"
name: "Test Warp Drive"
model: "TW-100"
manufacturer: "Test Corp"
desc: "A test warp drive"
lore: "Test lore"
cost: 150
additional_hp: 0
additional_power_consumption: 500.0
additional_heat_generation: 400.0
additional_weight: 80
warp_type: "warp"
warp_delay: 5.0
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        let fields = variant.as_warp_jump_core().expect("Should extract warp core fields");
        
        assert_eq!(fields.warp_type, WarpType::Warp);
        assert_eq!(fields.warp_delay, 5.0);
        assert_eq!(fields.jump_distance, None);
    }
    
    #[test]
    fn test_warp_jump_core_extraction_jump_type() {
        let yaml = r#"
id: "test-jump"
type: "warp-jump-core"
name: "Test Jump Drive"
model: "TJ-100"
manufacturer: "Test Corp"
desc: "A test jump drive"
lore: "Test lore"
cost: 200
additional_hp: 0
additional_power_consumption: 1200.0
additional_heat_generation: 1500.0
additional_weight: 60
warp_type: "jump"
warp_delay: 5.0
jump_distance: 50000
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        let fields = variant.as_warp_jump_core().expect("Should extract jump core fields");
        
        assert_eq!(fields.warp_type, WarpType::Jump);
        assert_eq!(fields.warp_delay, 5.0);
        assert_eq!(fields.jump_distance, Some(50000));
    }
    
    #[test]
    fn test_wrong_type_returns_none() {
        let yaml = r#"
id: "test-reactor"
type: "power-core"
name: "Test Reactor"
model: "TR-100"
manufacturer: "Test Corp"
desc: "A test reactor"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 20
energy_production: 150
energy_capacity: 2000
"#;
        
        let variant: ModuleVariant = serde_yaml::from_str(yaml).expect("Failed to parse");
        
        // Should not extract as wrong type
        assert!(variant.as_impulse_engine().is_none());
        assert!(variant.as_shield_generator().is_none());
        assert!(variant.as_comms_system().is_none());
    }
    
    #[test]
    fn test_validate_numeric_ranges() {
        let yaml = r#"
id: "test-module"
type: "power-core"
name: "Test Module"
model: "TM-100"
manufacturer: "Test Corp"
desc: "Test module"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 5.0
additional_heat_generation: 3.0
additional_weight: 50
energy_production: 1000
energy_capacity: 5000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_numeric_ranges().is_ok());
    }
    
    #[test]
    fn test_validate_numeric_ranges_negative_hp() {
        let yaml = r#"
id: "test-module"
type: "power-core"
name: "Test Module"
model: "TM-100"
manufacturer: "Test Corp"
desc: "Test module"
lore: "Test lore"
cost: 100
additional_hp: -10
additional_power_consumption: 5.0
additional_heat_generation: 3.0
additional_weight: 50
energy_production: 1000
energy_capacity: 5000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_numeric_ranges().is_err());
        assert!(variant.validate_numeric_ranges().unwrap_err().contains("negative additional_hp"));
    }
    
    #[test]
    fn test_validate_numeric_ranges_negative_power() {
        let yaml = r#"
id: "test-module"
type: "power-core"
name: "Test Module"
model: "TM-100"
manufacturer: "Test Corp"
desc: "Test module"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: -5.0
additional_heat_generation: 3.0
additional_weight: 50
energy_production: 1000
energy_capacity: 5000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_numeric_ranges().is_err());
        assert!(variant.validate_numeric_ranges().unwrap_err().contains("negative additional_power_consumption"));
    }
    
    #[test]
    fn test_validate_power_core_fields() {
        let yaml = r#"
id: "test-reactor"
type: "power-core"
name: "Test Reactor"
model: "TR-100"
manufacturer: "Test Corp"
desc: "A test reactor"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 20
energy_production: 1000
energy_capacity: 5000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_power_core_missing_field() {
        let yaml = r#"
id: "test-reactor"
type: "power-core"
name: "Test Reactor"
model: "TR-100"
manufacturer: "Test Corp"
desc: "A test reactor"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 20
energy_production: 1000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        let result = variant.validate_type_specific_fields();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing required field 'energy_capacity'"));
    }
    
    #[test]
    fn test_validate_power_core_negative_production() {
        let yaml = r#"
id: "test-reactor"
type: "power-core"
name: "Test Reactor"
model: "TR-100"
manufacturer: "Test Corp"
desc: "A test reactor"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 20
energy_production: -1000
energy_capacity: 5000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        let result = variant.validate_type_specific_fields();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must have positive energy_production"));
    }
    
    #[test]
    fn test_validate_impulse_engine_fields() {
        let yaml = r#"
id: "test-engine"
type: "impulse-engine"
name: "Test Engine"
model: "TE-100"
manufacturer: "Test Corp"
desc: "A test engine"
lore: "Test lore"
cost: 150
additional_hp: 15
additional_power_consumption: 100.0
additional_heat_generation: 50.0
additional_weight: 80
max_thrust: 50000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_shield_generator_fields() {
        let yaml = r#"
id: "test-shield"
type: "shield-generator"
name: "Test Shield"
model: "TS-100"
manufacturer: "Test Corp"
desc: "A test shield"
lore: "Test lore"
cost: 200
additional_hp: 20
additional_power_consumption: 80.0
additional_heat_generation: 40.0
additional_weight: 100
max_shield_strength: 10000
shield_recharge_rate: 500
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_comms_system_fields() {
        let yaml = r#"
id: "test-comms"
type: "comms-system"
name: "Test Comms"
model: "TC-100"
manufacturer: "Test Corp"
desc: "A test comms system"
lore: "Test lore"
cost: 120
additional_hp: 10
additional_power_consumption: 15.0
additional_heat_generation: 8.0
additional_weight: 40
comm_range: 1000000
encryption_lvl: 7
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_comms_invalid_encryption() {
        let yaml = r#"
id: "test-comms"
type: "comms-system"
name: "Test Comms"
model: "TC-100"
manufacturer: "Test Corp"
desc: "A test comms system"
lore: "Test lore"
cost: 120
additional_hp: 10
additional_power_consumption: 15.0
additional_heat_generation: 8.0
additional_weight: 40
comm_range: 1000000
encryption_lvl: 15
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        let result = variant.validate_type_specific_fields();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("encryption_lvl must be between 1 and 10"));
    }
    
    #[test]
    fn test_validate_sensor_array_fields() {
        let yaml = r#"
id: "test-sensor"
type: "sensor-array"
name: "Test Sensor"
model: "TS-100"
manufacturer: "Test Corp"
desc: "A test sensor"
lore: "Test lore"
cost: 180
additional_hp: 12
additional_power_consumption: 25.0
additional_heat_generation: 15.0
additional_weight: 60
scan_range: 1000000
detail_level: 5
scan_time: 3.0
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_sensor_invalid_detail_level() {
        let yaml = r#"
id: "test-sensor"
type: "sensor-array"
name: "Test Sensor"
model: "TS-100"
manufacturer: "Test Corp"
desc: "A test sensor"
lore: "Test lore"
cost: 180
additional_hp: 12
additional_power_consumption: 25.0
additional_heat_generation: 15.0
additional_weight: 60
scan_range: 1000000
detail_level: 0
scan_time: 3.0
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        let result = variant.validate_type_specific_fields();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("detail_level must be between 1 and 10"));
    }
    
    #[test]
    fn test_validate_kinetic_weapon_fields() {
        let yaml = r#"
id: "test-cannon"
type: "kinetic-weapon"
name: "Test Cannon"
model: "TC-100"
manufacturer: "Test Corp"
desc: "A test cannon"
lore: "Test lore"
cost: 280
additional_hp: 25
additional_power_consumption: 18.0
additional_heat_generation: 35.0
additional_weight: 250
ammo_type: shell
ammo_size: 200mm
reload_time: 5.0
num_projectiles: 1
ammo_consumed: 1
accuracy: 0.82
effective_range: 4000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_kinetic_weapon_invalid_accuracy() {
        let yaml = r#"
id: "test-cannon"
type: "kinetic-weapon"
name: "Test Cannon"
model: "TC-100"
manufacturer: "Test Corp"
desc: "A test cannon"
lore: "Test lore"
cost: 280
additional_hp: 25
additional_power_consumption: 18.0
additional_heat_generation: 35.0
additional_weight: 250
ammo_type: shell
ammo_size: 200mm
reload_time: 5.0
num_projectiles: 1
ammo_consumed: 1
accuracy: 1.5
effective_range: 4000
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        let result = variant.validate_type_specific_fields();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("accuracy must be between 0.0 and 1.0"));
    }
    
    #[test]
    fn test_validate_missile_launcher_fields() {
        let yaml = r#"
id: "test-launcher"
type: "missile-launcher"
name: "Test Launcher"
model: "TL-100"
manufacturer: "Test Corp"
desc: "A test launcher"
lore: "Test lore"
cost: 220
additional_hp: 25
additional_power_consumption: 15.0
additional_heat_generation: 28.0
additional_weight: 200
reload_time: 15.0
missile_volley_delay: 1.5
num_launched: 1
ammo_capacity: 24
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_de_weapon_fields() {
        let yaml = r#"
id: "test-laser"
type: "de-weapon"
name: "Test Laser"
model: "TL-100"
manufacturer: "Test Corp"
desc: "A test laser"
lore: "Test lore"
cost: 200
additional_hp: 16
additional_power_consumption: 12.0
additional_heat_generation: 22.0
additional_weight: 140
damage: 45
recharge_time: 3.5
max_range: 10000
projectile_speed: 299792458
weapon_tags: ["Beam", "Photon"]
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_stealth_system_fields() {
        let yaml = r#"
id: "test-stealth"
type: "stealth-system"
name: "Test Stealth"
model: "TS-100"
manufacturer: "Test Corp"
desc: "A test stealth system"
lore: "Test lore"
cost: 250
additional_hp: 15
additional_power_consumption: 40.0
additional_heat_generation: 10.0
additional_weight: 120
detectability_reduction: 0.5
scan_time_increase: 2.0
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        assert!(variant.validate_type_specific_fields().is_ok());
    }
    
    #[test]
    fn test_validate_stealth_invalid_detectability() {
        let yaml = r#"
id: "test-stealth"
type: "stealth-system"
name: "Test Stealth"
model: "TS-100"
manufacturer: "Test Corp"
desc: "A test stealth system"
lore: "Test lore"
cost: 250
additional_hp: 15
additional_power_consumption: 40.0
additional_heat_generation: 10.0
additional_weight: 120
detectability_reduction: 1.5
scan_time_increase: 2.0
"#;
        let variant: ModuleVariant = serde_yaml::from_str(yaml).unwrap();
        let result = variant.validate_type_specific_fields();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("detectability_reduction must be between 0.0 and 1.0"));
    }
}
