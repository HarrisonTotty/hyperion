//! Configuration module
//!
//! Handles loading and parsing of game configuration files from the data directory.
//!
//! This module is organized into focused submodules:
//! - `ship_class` - Ship class configurations
//! - `module` - Module configurations
//! - `weapon` - Weapon and ammunition configurations
//! - `ai` - AI behavior and personality configurations
//! - `map` - Galaxy and procedural generation configurations
//! - `simulation` - Physics and combat simulation configurations
//! - `faction_gen` - Faction generation and relationship configurations

pub mod ship_class;
pub mod module;
pub mod weapon;
pub mod ai;
pub mod map;
pub mod simulation;
pub mod faction_gen;
pub mod bonus;

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// Re-export config types
pub use ship_class::{ShipClassConfig, ShipSize, ShipClassRole};
pub use module::{
    ModuleConfig, ModuleSlot, ModuleStats, ModuleVariant,
    PowerCoreFields, ImpulseEngineFields, ManeuveringThrusterFields,
    ShieldGeneratorFields, CommsSystemFields, CoolingSystemFields,
    SensorArrayFields, StealthSystemFields, AuxSupportSystemFields,
    WarpJumpCoreFields, WarpType,
};
pub use weapon::{WeaponConfig, AmmunitionConfig, KineticWeaponKind, WeaponTagConfig, StatusEffectConfig};
pub use ai::AIConfig;
pub use map::MapConfig as ProceduralMapConfig;
pub use simulation::SimulationConfig as ProceduralSimConfig;
pub use faction_gen::FactionGenConfig;
pub use bonus::{BonusConfig, BonusMetadata, BonusFormat, CategoryMetadata, FormattedBonus};

/// Main game configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub ai: AiConfig,
    pub factions: FactionsConfig,
    pub map: MapConfig,
    pub modules: ModulesConfig,
    pub races: RacesConfig,
    pub simulation: SimulationConfig,
    
    // Enhanced configuration
    pub ship_classes: Vec<ShipClassConfig>,
    pub module_definitions: Vec<ModuleConfig>,
    pub weapon_definitions: Vec<WeaponConfig>,
    pub ammunition_types: Vec<AmmunitionConfig>,
    pub kinetic_weapon_kinds: Vec<KineticWeaponKind>,
    
    // New Phase 7.5 configurations
    #[serde(default)]
    pub ai_behavior: AIConfig,
    #[serde(default)]
    pub procedural_map: ProceduralMapConfig,
    #[serde(default)]
    pub simulation_params: ProceduralSimConfig,
    #[serde(default)]
    pub faction_generation: FactionGenConfig,
    
    // Phase 2.6: Bonus metadata for ship classes
    #[serde(skip)]
    pub bonuses: Option<BonusConfig>,
    
    // Phase 1.2: Module variants for selection workflow
    #[serde(skip)]
    pub module_variants: HashMap<String, Vec<ModuleVariant>>,
    
    // Phase 2.1: Module slot definitions
    #[serde(skip)]
    pub module_slots: HashMap<String, ModuleSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// AI difficulty level
    pub difficulty: String,
    /// AI response time in seconds
    pub response_time: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionsConfig {
    pub factions: Vec<Faction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Faction {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfig {
    /// Size of the galaxy in units
    pub galaxy_size: u32,
    /// Density of stars (0.0 to 1.0)
    pub star_density: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulesConfig {
    /// Map of module ID to module template definition
    pub modules: HashMap<String, ModuleTemplate>,
}

/// Module template definition from modules.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleTemplate {
    /// Display name
    pub name: String,
    /// Description
    #[serde(rename = "desc")]
    pub description: String,
    /// Module groups (for bonuses and categorization)
    pub groups: Vec<String>,
    /// Whether this module is required for all ships
    pub required: bool,
    /// Module type (Defense, Offense, Support)
    #[serde(rename = "type")]
    pub module_type: String,
    /// Build cost
    pub cost: u32,
    /// Maximum allowed instances on a ship
    pub max_allowed: u32,
    /// Hit points
    pub hp: u32,
    
    // Phase 1.1: Slot metadata fields for new architecture
    /// Whether this module uses the variant system
    #[serde(default)]
    pub has_variants: Option<bool>,
    /// Base power consumption in MW (before variant modifiers)
    #[serde(default)]
    pub base_power_consumption: Option<f32>,
    /// Base heat generation in K (before variant modifiers)
    #[serde(default)]
    pub base_heat_generation: Option<f32>,
    /// Base weight in tons (before variant modifiers)
    #[serde(default)]
    pub base_weight: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RacesConfig {
    pub races: Vec<Race>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Race {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    /// Simulation tick rate in Hz
    pub tick_rate: f32,
    /// Whether realistic physics is enabled
    pub physics_enabled: bool,
}

// Legacy structures for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipClass {
    pub id: String,
    pub name: String,
    pub description: String,
    pub base_hull: f32,
    pub base_shields: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDefinition {
    pub id: String,
    pub name: String,
    pub module_type: String,
    pub description: String,
}

impl GameConfig {
    /// Load game configuration from a directory
    ///
    /// # Arguments
    ///
    /// * `data_dir` - Path to the data directory containing configuration files
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the loaded `GameConfig` or an error
    pub fn load_from_directory(data_dir: &Path) -> Result<Self, String> {
        info!("Loading game configuration from: {}", data_dir.display());

        // Load individual configuration files
        let ai = Self::load_yaml::<AiConfig>(data_dir.join("ai.yaml"))?;
        let factions = Self::load_yaml::<FactionsConfig>(data_dir.join("factions.yaml"))?;
        let map = Self::load_yaml::<MapConfig>(data_dir.join("map.yaml"))?;
        let modules = Self::load_yaml::<ModulesConfig>(data_dir.join("modules.yaml"))?;
        let races = Self::load_yaml::<RacesConfig>(data_dir.join("races.yaml"))?;
        let simulation = Self::load_yaml::<SimulationConfig>(data_dir.join("simulation.yaml"))?;

        // Load Phase 7.5 enhanced configurations (with defaults if files don't exist)
        let ai_behavior = Self::load_yaml_optional::<AIConfig>(data_dir.join("ai.yaml"))
            .unwrap_or_default();
        let procedural_map = Self::load_yaml_optional::<ProceduralMapConfig>(data_dir.join("procedural_generation.yaml"))
            .unwrap_or_default();
        let simulation_params = Self::load_yaml_optional::<ProceduralSimConfig>(data_dir.join("simulation.yaml"))
            .unwrap_or_default();
        let faction_generation = Self::load_yaml_optional::<FactionGenConfig>(data_dir.join("faction_generation.yaml"))
            .unwrap_or_default();
        
        // Load Phase 2.6 bonus metadata
        let bonuses = Self::load_yaml_optional::<BonusConfig>(data_dir.join("bonuses.yaml"));

        // Load ship classes from ship-classes directory
        let ship_classes = Self::load_ship_classes(data_dir.join("ship-classes"))?;

        // Load module definitions from modules directory
        let module_definitions = Self::load_modules(data_dir.join("modules"))?;

        // TODO: Remove weapon_definitions once weapon migration to ModuleVariants is complete
        // Old WeaponConfig files have been migrated to ModuleVariant format
        let weapon_definitions = Vec::new();

        // Load ammunition types
        let ammunition_types = Self::load_yaml_optional::<AmmunitionTypesConfig>(
            data_dir.join("ammunition.yaml")
        )
        .map(|c| c.ammunition)
        .unwrap_or_default();

        // Load kinetic weapon kinds
        let kinetic_weapon_kinds = Self::load_yaml_optional::<KineticWeaponKindsConfig>(
            data_dir.join("kinetic_weapons.yaml")
        )
        .map(|c| c.kinds)
        .unwrap_or_default();
        
        // Load module slot definitions first (needed for variant validation)
        let module_slots = Self::load_module_slots(data_dir.to_path_buf())?;
        info!("Loaded {} module slot definitions", module_slots.len());
        
        // Load module variants from subdirectories (with validation against slots)
        let module_variants = Self::load_module_variants(data_dir.join("modules"), Some(&module_slots))?;
        info!("Loaded {} module variant types", module_variants.len());

        let config = GameConfig {
            ai,
            factions,
            map,
            modules,
            races,
            simulation,
            ship_classes,
            module_definitions,
            weapon_definitions,
            ammunition_types,
            kinetic_weapon_kinds,
            ai_behavior,
            procedural_map,
            simulation_params,
            faction_generation,
            bonuses,
            module_variants,
            module_slots,
        };

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate the entire configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate ship classes
        for ship_class in &self.ship_classes {
            ship_class.validate()?;
        }

        // Validate modules
        for module in &self.module_definitions {
            module.validate()?;
        }

        // Validate weapons
        for weapon in &self.weapon_definitions {
            weapon.validate()?;
        }
        
        // Validate module slots
        for slot in self.module_slots.values() {
            slot.validate()?;
        }
        
        // Validate module variants (basic + numeric ranges + type-specific)
        for variants in self.module_variants.values() {
            for variant in variants {
                variant.validate()?;
                variant.validate_numeric_ranges()?;
                variant.validate_type_specific_fields()?;
            }
        }

        // Check for duplicate IDs
        self.check_duplicate_ids()?;

        info!("Configuration validation passed");
        Ok(())
    }

    /// Check for duplicate IDs across configurations
    fn check_duplicate_ids(&self) -> Result<(), String> {
        let mut ship_class_ids = HashMap::new();
        for sc in &self.ship_classes {
            if ship_class_ids.insert(&sc.id, &sc.name).is_some() {
                return Err(format!("Duplicate ship class ID '{}' found", sc.id));
            }
        }

        let mut module_ids = HashMap::new();
        for m in &self.module_definitions {
            if module_ids.insert(&m.id, &m.name).is_some() {
                return Err(format!("Duplicate module ID '{}' found", m.id));
            }
        }

        let mut weapon_ids = HashMap::new();
        for w in &self.weapon_definitions {
            if weapon_ids.insert(&w.id, &w.name).is_some() {
                return Err(format!("Duplicate weapon ID '{}' found", w.id));
            }
        }
        
        // Check for duplicate module slot IDs (already done during loading via HashMap)
        // But we can validate they're reasonable
        if self.module_slots.is_empty() {
            warn!("No module slots loaded");
        }
        
        // Check for duplicate module variant IDs within each type
        for (module_type, variants) in &self.module_variants {
            let mut variant_ids = HashMap::new();
            for variant in variants {
                if variant_ids.insert(&variant.id, &variant.name).is_some() {
                    return Err(format!(
                        "Duplicate module variant ID '{}' found in type '{}'",
                        variant.id, module_type
                    ));
                }
            }
        }

        Ok(())
    }

    /// Find a ship class by ID
    pub fn get_ship_class(&self, id: &str) -> Option<&ShipClassConfig> {
        self.ship_classes.iter().find(|sc| sc.id == id)
    }

    /// Find a module by ID
    pub fn get_module(&self, id: &str) -> Option<&ModuleConfig> {
        self.module_definitions.iter().find(|m| m.id == id)
    }

    /// Find a weapon by ID
    pub fn get_weapon(&self, id: &str) -> Option<&WeaponConfig> {
        self.weapon_definitions.iter().find(|w| w.id == id)
    }

    /// Find ammunition by ID
    pub fn get_ammunition(&self, id: &str) -> Option<&AmmunitionConfig> {
        self.ammunition_types.iter().find(|a| a.id == id)
    }

    /// Find kinetic weapon kind by ID
    pub fn get_kinetic_weapon_kind(&self, id: &str) -> Option<&KineticWeaponKind> {
        self.kinetic_weapon_kinds.iter().find(|k| k.id == id)
    }
    
    /// Get all variants for a module type
    pub fn get_module_variants(&self, module_type: &str) -> Option<&Vec<ModuleVariant>> {
        self.module_variants.get(module_type)
    }
    
    /// Get a specific module variant by type and variant ID
    pub fn get_module_variant(&self, module_type: &str, variant_id: &str) -> Option<&ModuleVariant> {
        self.module_variants
            .get(module_type)?
            .iter()
            .find(|v| v.id == variant_id)
    }
    
    /// Get a module slot definition by ID
    pub fn get_module_slot(&self, id: &str) -> Option<&ModuleSlot> {
        self.module_slots.get(id)
    }

    /// Load a YAML file and deserialize it
    fn load_yaml<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, String> {
        debug!("Loading YAML file: {}", path.display());
        let contents =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

        serde_yaml::from_str(&contents)
            .map_err(|e| format!("Failed to parse {:?}: {}", path, e))
    }

    /// Load a YAML file optionally (returns None if file doesn't exist)
    fn load_yaml_optional<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Option<T> {
        if !path.exists() {
            return None;
        }
        match Self::load_yaml(path) {
            Ok(val) => Some(val),
            Err(e) => {
                warn!("Failed to load optional config: {}", e);
                None
            }
        }
    }

    /// Load all YAML files from a directory
    fn load_directory<T: for<'de> Deserialize<'de>>(dir: PathBuf) -> Result<Vec<T>, String> {
        debug!("Loading directory: {}", dir.display());

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut items = Vec::new();

        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let item = Self::load_yaml::<T>(path)?;
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Load ship classes from a directory, setting IDs from filenames
    fn load_ship_classes(dir: PathBuf) -> Result<Vec<ShipClassConfig>, String> {
        debug!("Loading ship classes from: {}", dir.display());

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut items = Vec::new();

        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let mut item = Self::load_yaml::<ShipClassConfig>(path.clone())?;
                
                // Extract filename without extension as ID
                let id = path.file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| format!("Invalid filename: {:?}", path))?
                    .to_string();
                
                item.set_id(id);
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Load modules from a directory (recursively), setting IDs from filenames
    /// Skips weapon directories (*-weapons)
    fn load_modules(dir: PathBuf) -> Result<Vec<ModuleConfig>, String> {
        debug!("Loading modules from: {}", dir.display());

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut items = Vec::new();

        fn load_recursive(dir: &Path, items: &mut Vec<ModuleConfig>) -> Result<(), String> {
            // Hardcoded list of variant directories to skip (loaded separately as ModuleVariants)
            // Note: Weapon directories (*-weapons) are also skipped below and loaded as WeaponConfig
            let variant_dirs = vec![
                "aux-support-systems",
                "comms-systems",
                "cooling-systems",
                "countermeasures",
                "de-weapons",
                "impulse-engines",
                "kinetic-weapons",
                "maneuvering-thrusters",
                "missile-launchers",
                "power-cores",
                "radial-emission-systems",
                "sensor-arrays",
                "shield-generators",
                "stealth-systems",
                "warp-cores",
            ];
            
            let entries = fs::read_dir(dir)
                .map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))?;

            for entry in entries {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let path = entry.path();

                if path.is_dir() {
                    // Check if this directory should be skipped
                    let should_skip = if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                        // Skip weapon directories
                        if dir_name.ends_with("-weapons") {
                            debug!("Skipping weapon directory: {}", dir_name);
                            true
                        }
                        // Skip variant directories (loaded separately)
                        else if variant_dirs.contains(&dir_name) {
                            debug!("Skipping variant directory: {}", dir_name);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    if !should_skip {
                        load_recursive(&path, items)?;
                    }
                } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    let mut item = GameConfig::load_yaml::<ModuleConfig>(path.clone())?;
                    
                    // Extract filename without extension as ID
                    let id = path.file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| format!("Invalid filename: {:?}", path))?
                        .to_string();
                    
                    item.set_id(id);
                    items.push(item);
                }
            }

            Ok(())
        }

        load_recursive(&dir, &mut items)?;
        Ok(items)
    }

    /// Load weapons from modules/*-weapons directories (recursively), setting IDs from filenames
    fn load_weapons(dir: PathBuf) -> Result<Vec<WeaponConfig>, String> {
        debug!("Loading weapons from: {}", dir.display());

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut items = Vec::new();

        fn load_recursive(dir: &Path, items: &mut Vec<WeaponConfig>) -> Result<(), String> {
            let entries = fs::read_dir(dir)
                .map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))?;

            for entry in entries {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let path = entry.path();

                if path.is_dir() {
                    // Only recurse into weapon directories
                    if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                        if dir_name.ends_with("-weapons") {
                            debug!("Loading weapons from: {}", dir_name);
                            load_recursive(&path, items)?;
                        }
                    }
                } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    let mut item = GameConfig::load_yaml::<WeaponConfig>(path.clone())?;
                    
                    // Extract filename without extension as ID
                    let id = path.file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| format!("Invalid filename: {:?}", path))?
                        .to_string();
                    
                    item.set_id(id);
                    items.push(item);
                }
            }

            Ok(())
        }

        load_recursive(&dir, &mut items)?;
        Ok(items)
    }
    
    /// Load module slots from data/module-slots/*.yaml
    /// 
    /// Scans the module-slots directory and loads all module slot definitions.
    /// Returns a HashMap keyed by module slot ID.
    /// 
    /// Each slot definition is validated after loading.
    fn load_module_slots(data_dir: PathBuf) -> Result<HashMap<String, ModuleSlot>, String> {
        let slots_dir = data_dir.join("module-slots");
        debug!("Loading module slots from: {}", slots_dir.display());
        
        if !slots_dir.exists() {
            debug!("Module slots directory does not exist, returning empty slots");
            return Ok(HashMap::new());
        }
        
        let mut slots = HashMap::new();
        
        let entries = fs::read_dir(&slots_dir)
            .map_err(|e| format!("Failed to read module-slots directory: {}", e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                debug!("Loading module slot: {}", path.display());
                let slot = Self::load_yaml::<ModuleSlot>(path.clone())?;
                
                // Validate the slot
                slot.validate()?;
                
                // Check for duplicate IDs
                if slots.contains_key(&slot.id) {
                    return Err(format!("Duplicate module slot ID '{}' found in {:?}", slot.id, path));
                }
                
                slots.insert(slot.id.clone(), slot);
            }
        }
        
        if !slots.is_empty() {
            info!("Loaded {} module slot definitions", slots.len());
        } else {
            warn!("No module slot definitions found in {:?}", slots_dir);
        }
        
        Ok(slots)
    }
    
    /// Load module variants from subdirectories
    /// 
    /// Recursively scans `data/modules/**/*.yaml` and loads all module variant files.
    /// Returns a HashMap keyed by the variant's `type` field (which must match a module slot ID).
    /// 
    /// Excludes weapon directories (*-weapons) as those are loaded separately as WeaponConfig.
    /// 
    /// If module_slots is provided, validates that each variant's `type` field matches a known slot ID.
    fn load_module_variants(
        modules_dir: PathBuf, 
        module_slots: Option<&HashMap<String, ModuleSlot>>
    ) -> Result<HashMap<String, Vec<ModuleVariant>>, String> {
        debug!("Loading module variants from: {}", modules_dir.display());
        
        if !modules_dir.exists() {
            debug!("Modules directory does not exist, returning empty variants");
            return Ok(HashMap::new());
        }
        
        let mut variants_by_type: HashMap<String, Vec<ModuleVariant>> = HashMap::new();
        
        // Recursive helper function to scan directories
        fn scan_directory(
            dir: &Path, 
            variants_by_type: &mut HashMap<String, Vec<ModuleVariant>>,
            module_slots: Option<&HashMap<String, ModuleSlot>>
        ) -> Result<(), String> {
            let entries = fs::read_dir(dir)
                .map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))?;
            
            for entry in entries {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let path = entry.path();
                
                if path.is_dir() {
                    // Skip weapon directories - they use WeaponConfig
                    if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                        if dir_name.ends_with("-weapons") {
                            debug!("Skipping weapon directory: {}", dir_name);
                            continue;
                        }
                    }
                    
                    // Recursively scan subdirectories
                    scan_directory(&path, variants_by_type, module_slots)?;
                } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                    debug!("Loading variant: {}", path.display());
                    let variant = GameConfig::load_yaml::<ModuleVariant>(path.clone())?;
                    
                    // Validate the variant
                    variant.validate()?;
                    
                    // Validate that the variant's type matches a known module slot (if slots provided)
                    if let Some(slots) = module_slots {
                        if !slots.contains_key(&variant.module_type) {
                            return Err(format!(
                                "Module variant '{}' in {:?} has type '{}' which does not match any known module slot",
                                variant.id, path, variant.module_type
                            ));
                        }
                    }
                    
                    // Group by type
                    variants_by_type
                        .entry(variant.module_type.clone())
                        .or_insert_with(Vec::new)
                        .push(variant);
                }
            }
            
            Ok(())
        }
        
        scan_directory(&modules_dir, &mut variants_by_type, module_slots)?;
        
        // Log summary
        for (module_type, variants) in &variants_by_type {
            info!("Loaded {} variants for module type '{}'", variants.len(), module_type);
        }
        
        Ok(variants_by_type)
    }
}

// Helper structures for loading list configs
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AmmunitionTypesConfig {
    ammunition: Vec<AmmunitionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KineticWeaponKindsConfig {
    kinds: Vec<KineticWeaponKind>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Helper function to create a minimal test ShipClassConfig
    pub fn create_test_ship_class(id: &str, name: &str) -> ShipClassConfig {
        let mut ship = ShipClassConfig {
            name: name.to_string(),
            description: "Test ship class".to_string(),
            base_hull: 1000.0,
            base_shields: 500.0,
            max_weight: 5000.0,
            max_modules: 10,
            size: ShipSize::Medium,
            role: ShipClassRole::Combat,
            build_points: 1000.0,
            bonuses: std::collections::HashMap::new(),
            id: String::new(),
            manufacturers: std::collections::HashMap::new(),
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
        ship.set_id(id.to_string());
        ship
    }
    
    /// Helper function to create a minimal test GameConfig
    pub fn create_test_game_config() -> GameConfig {
        GameConfig {
            ai: AiConfig {
                difficulty: "normal".to_string(),
                response_time: 1.0,
            },
            factions: FactionsConfig { factions: vec![] },
            map: MapConfig { galaxy_size: 1000, star_density: 0.15 },
            modules: ModulesConfig { modules: std::collections::HashMap::new() },
            races: RacesConfig { races: vec![] },
            simulation: SimulationConfig { tick_rate: 60.0, physics_enabled: true },
            ai_behavior: AIConfig::default(),
            procedural_map: ProceduralMapConfig::default(),
            simulation_params: ProceduralSimConfig::default(),
            faction_generation: FactionGenConfig::default(),
            bonuses: None,
            module_variants: std::collections::HashMap::new(),
            module_slots: std::collections::HashMap::new(),
            ship_classes: vec![create_test_ship_class("cruiser", "Cruiser")],
            module_definitions: vec![],
            weapon_definitions: vec![],
            ammunition_types: vec![],
            kinetic_weapon_kinds: vec![],
        }
    }

    #[test]
    fn test_config_structure() {
        let ai = AiConfig {
            difficulty: "normal".to_string(),
            response_time: 1.0,
        };
        assert_eq!(ai.difficulty, "normal");
    }

    #[test]
    fn test_config_lookups() {
        let config = create_test_game_config();
        assert!(config.get_ship_class("cruiser").is_some());
        assert!(config.get_ship_class("invalid").is_none());
    }
    
    #[test]
    fn test_module_template_slot_metadata() {
        // Test that ModuleTemplate can deserialize with optional slot metadata fields
        let yaml_with_variants = r#"
name: Test Power Core
desc: A test power module
groups: ["power"]
required: true
type: Support
cost: 100
max_allowed: 2
hp: 40
has_variants: true
base_power_consumption: 0
base_heat_generation: 50
base_weight: 80
"#;
        
        let template: Result<ModuleTemplate, _> = serde_yaml::from_str(yaml_with_variants);
        assert!(template.is_ok(), "Failed to parse module with slot metadata");
        
        let template = template.unwrap();
        assert_eq!(template.has_variants, Some(true));
        assert_eq!(template.base_power_consumption, Some(0.0));
        assert_eq!(template.base_heat_generation, Some(50.0));
        assert_eq!(template.base_weight, Some(80.0));
        
        // Test backward compatibility - module without new fields
        let yaml_without_variants = r#"
name: Test Module
desc: A test module
groups: ["support"]
required: false
type: Support
cost: 50
max_allowed: 1
hp: 25
"#;
        
        let template: Result<ModuleTemplate, _> = serde_yaml::from_str(yaml_without_variants);
        assert!(template.is_ok(), "Failed to parse module without slot metadata");
        
        let template = template.unwrap();
        assert_eq!(template.has_variants, None);
        assert_eq!(template.base_power_consumption, None);
        assert_eq!(template.base_heat_generation, None);
        assert_eq!(template.base_weight, None);
    }
    
    #[test]
    fn test_load_module_slots_empty_dir() {
        use std::env;
        
        // Create a temporary directory
        let temp_dir = env::temp_dir().join("hyperion_test_no_slots");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        
        // Load from empty directory (no module-slots subdir)
        let result = GameConfig::load_module_slots(temp_dir.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_slots_valid() {
        use std::env;
        
        // Create a temporary directory structure
        let temp_dir = env::temp_dir().join("hyperion_test_slots");
        let _ = fs::remove_dir_all(&temp_dir);
        let slots_dir = temp_dir.join("module-slots");
        fs::create_dir_all(&slots_dir).unwrap();
        
        // Create a test power-core slot YAML file
        let power_core_yaml = r#"
id: power-core
name: "Power Core"
desc: "Provides the ship with power generation and capacity."
extended_desc: "Different power cores provide varying levels of power output."
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
        fs::write(slots_dir.join("power-core.yaml"), power_core_yaml).unwrap();
        
        // Create a test impulse-engine slot YAML file
        let impulse_engine_yaml = r#"
id: impulse-engine
name: "Impulse Engine"
desc: "Provides thrust for the ship."
extended_desc: "The impulse engine determines the ship's acceleration."
groups: ["Essential", "Propulsion"]
required: true
has_varients: true
base_cost: 15
max_slots: 1
base_hp: 20
base_power_consumption: 10.0
base_heat_generation: 8.0
base_weight: 200
"#;
        fs::write(slots_dir.join("impulse-engine.yaml"), impulse_engine_yaml).unwrap();
        
        // Load module slots
        let result = GameConfig::load_module_slots(temp_dir.clone());
        assert!(result.is_ok(), "Failed to load module slots: {:?}", result.err());
        
        let slots = result.unwrap();
        assert_eq!(slots.len(), 2);
        
        // Verify power-core slot
        let power_core = slots.get("power-core");
        assert!(power_core.is_some());
        let power_core = power_core.unwrap();
        assert_eq!(power_core.id, "power-core");
        assert_eq!(power_core.name, "Power Core");
        assert_eq!(power_core.required, true);
        assert_eq!(power_core.has_varients, true);
        assert_eq!(power_core.base_cost, 10);
        assert_eq!(power_core.max_slots, 2);
        assert_eq!(power_core.base_hp, 10);
        assert_eq!(power_core.base_power_consumption, 0.0);
        assert_eq!(power_core.base_heat_generation, 5.0);
        assert_eq!(power_core.base_weight, 100);
        
        // Verify impulse-engine slot
        let impulse_engine = slots.get("impulse-engine");
        assert!(impulse_engine.is_some());
        let impulse_engine = impulse_engine.unwrap();
        assert_eq!(impulse_engine.id, "impulse-engine");
        assert_eq!(impulse_engine.name, "Impulse Engine");
        assert_eq!(impulse_engine.max_slots, 1);
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_slots_duplicate_id() {
        use std::env;
        
        // Create a temporary directory structure
        let temp_dir = env::temp_dir().join("hyperion_test_duplicate_slots");
        let _ = fs::remove_dir_all(&temp_dir);
        let slots_dir = temp_dir.join("module-slots");
        fs::create_dir_all(&slots_dir).unwrap();
        
        // Create two files with the same ID
        let slot_yaml_1 = r#"
id: power-core
name: "Power Core 1"
desc: "Test"
extended_desc: "Test"
groups: ["Essential"]
required: true
has_varients: true
base_cost: 10
max_slots: 2
base_hp: 10
base_power_consumption: 0.0
base_heat_generation: 5.0
base_weight: 100
"#;
        let slot_yaml_2 = r#"
id: power-core
name: "Power Core 2"
desc: "Test"
extended_desc: "Test"
groups: ["Essential"]
required: true
has_varients: true
base_cost: 10
max_slots: 2
base_hp: 10
base_power_consumption: 0.0
base_heat_generation: 5.0
base_weight: 100
"#;
        fs::write(slots_dir.join("power-core-1.yaml"), slot_yaml_1).unwrap();
        fs::write(slots_dir.join("power-core-2.yaml"), slot_yaml_2).unwrap();
        
        // Load should fail with duplicate ID error
        let result = GameConfig::load_module_slots(temp_dir.clone());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate module slot ID"));
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_slots_invalid_yaml() {
        use std::env;
        
        // Create a temporary directory structure
        let temp_dir = env::temp_dir().join("hyperion_test_invalid_slot");
        let _ = fs::remove_dir_all(&temp_dir);
        let slots_dir = temp_dir.join("module-slots");
        fs::create_dir_all(&slots_dir).unwrap();
        
        // Create an invalid YAML file (missing required fields)
        let invalid_yaml = r#"
id: power-core
name: "Power Core"
# Missing many required fields
"#;
        fs::write(slots_dir.join("power-core.yaml"), invalid_yaml).unwrap();
        
        // Load should fail with parse error
        let result = GameConfig::load_module_slots(temp_dir.clone());
        assert!(result.is_err());
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_slots_validation_failure() {
        use std::env;
        
        // Create a temporary directory structure
        let temp_dir = env::temp_dir().join("hyperion_test_invalid_validation");
        let _ = fs::remove_dir_all(&temp_dir);
        let slots_dir = temp_dir.join("module-slots");
        fs::create_dir_all(&slots_dir).unwrap();
        
        // Create a YAML file that parses but fails validation (negative cost)
        let invalid_yaml = r#"
id: power-core
name: "Power Core"
desc: "Test"
extended_desc: "Test"
groups: ["Essential"]
required: true
has_varients: true
base_cost: -10
max_slots: 2
base_hp: 10
base_power_consumption: 0.0
base_heat_generation: 5.0
base_weight: 100
"#;
        fs::write(slots_dir.join("power-core.yaml"), invalid_yaml).unwrap();
        
        // Load should fail with validation error
        let result = GameConfig::load_module_slots(temp_dir.clone());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("negative base_cost"));
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_get_module_slot() {
        let mut config = create_test_game_config();
        
        // Add a test module slot
        let power_core_slot = ModuleSlot {
            id: "power-core".to_string(),
            name: "Power Core".to_string(),
            description: "Provides power".to_string(),
            extended_desc: "Extended description".to_string(),
            groups: vec!["Essential".to_string()],
            required: true,
            has_varients: true,
            base_cost: 10,
            max_slots: 2,
            base_hp: 10,
            base_power_consumption: 0.0,
            base_heat_generation: 5.0,
            base_weight: 100,
        };
        config.module_slots.insert("power-core".to_string(), power_core_slot);
        
        // Test successful lookup
        assert!(config.get_module_slot("power-core").is_some());
        assert_eq!(config.get_module_slot("power-core").unwrap().name, "Power Core");
        
        // Test failed lookup
        assert!(config.get_module_slot("invalid-slot").is_none());
    }
    
    #[test]
    fn test_load_module_variants_empty_dir() {
        use std::env;
        
        // Create a temporary directory
        let temp_dir = env::temp_dir().join("hyperion_test_no_variants");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        
        // Load from empty directory (no modules subdir)
        let result = GameConfig::load_module_variants(temp_dir.clone(), None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_variants_recursive() {
        use std::env;
        
        // Create a temporary directory structure with nested subdirectories
        let temp_dir = env::temp_dir().join("hyperion_test_variants_recursive");
        let _ = fs::remove_dir_all(&temp_dir);
        let modules_dir = temp_dir.join("modules");
        let power_cores_dir = modules_dir.join("power-cores");
        let engines_dir = modules_dir.join("propulsion").join("impulse-engines");
        fs::create_dir_all(&power_cores_dir).unwrap();
        fs::create_dir_all(&engines_dir).unwrap();
        
        // Create test variant YAML files in different locations
        let power_core_yaml = r#"
id: test-reactor
type: power-core
name: "Test Reactor"
model: "Test Model"
manufacturer: "Test Corp"
desc: "A test power core"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 50
energy_production: 1000
energy_capacity: 5000
"#;
        fs::write(power_cores_dir.join("test-reactor.yaml"), power_core_yaml).unwrap();
        
        let impulse_engine_yaml = r#"
id: test-engine
type: impulse-engine
name: "Test Engine"
model: "Test Model"
manufacturer: "Test Corp"
desc: "A test impulse engine"
lore: "Test lore"
cost: 150
additional_hp: 15
additional_power_consumption: 10.0
additional_heat_generation: 8.0
additional_weight: 100
max_thrust: 50000
"#;
        fs::write(engines_dir.join("test-engine.yaml"), impulse_engine_yaml).unwrap();
        
        // Load module variants (no validation)
        let result = GameConfig::load_module_variants(modules_dir.clone(), None);
        assert!(result.is_ok(), "Failed to load variants: {:?}", result.err());
        
        let variants = result.unwrap();
        assert_eq!(variants.len(), 2, "Expected 2 variant types");
        
        // Verify power-core variants
        assert!(variants.contains_key("power-core"));
        assert_eq!(variants.get("power-core").unwrap().len(), 1);
        assert_eq!(variants.get("power-core").unwrap()[0].id, "test-reactor");
        
        // Verify impulse-engine variants
        assert!(variants.contains_key("impulse-engine"));
        assert_eq!(variants.get("impulse-engine").unwrap().len(), 1);
        assert_eq!(variants.get("impulse-engine").unwrap()[0].id, "test-engine");
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_variants_skips_weapons() {
        use std::env;
        
        // Create a temporary directory structure
        let temp_dir = env::temp_dir().join("hyperion_test_variants_skip_weapons");
        let _ = fs::remove_dir_all(&temp_dir);
        let modules_dir = temp_dir.join("modules");
        let power_cores_dir = modules_dir.join("power-cores");
        let weapons_dir = modules_dir.join("kinetic-weapons");
        fs::create_dir_all(&power_cores_dir).unwrap();
        fs::create_dir_all(&weapons_dir).unwrap();
        
        // Create a valid variant
        let power_core_yaml = r#"
id: test-reactor
type: power-core
name: "Test Reactor"
model: "Test Model"
manufacturer: "Test Corp"
desc: "A test power core"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 50
energy_production: 1000
energy_capacity: 5000
"#;
        fs::write(power_cores_dir.join("test-reactor.yaml"), power_core_yaml).unwrap();
        
        // Create a weapon file (should be skipped)
        let weapon_yaml = r#"
id: test-weapon
type: kinetic-weapon
name: "Test Weapon"
"#;
        fs::write(weapons_dir.join("test-weapon.yaml"), weapon_yaml).unwrap();
        
        // Load module variants
        let result = GameConfig::load_module_variants(modules_dir.clone(), None);
        assert!(result.is_ok());
        
        let variants = result.unwrap();
        // Should only have power-core, not kinetic-weapon (skipped)
        assert_eq!(variants.len(), 1);
        assert!(variants.contains_key("power-core"));
        assert!(!variants.contains_key("kinetic-weapon"));
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_variants_validation_success() {
        use std::env;
        
        // Create module slots
        let mut slots = HashMap::new();
        slots.insert("power-core".to_string(), ModuleSlot {
            id: "power-core".to_string(),
            name: "Power Core".to_string(),
            description: "Provides power".to_string(),
            extended_desc: "Extended".to_string(),
            groups: vec![],
            required: true,
            has_varients: true,
            base_cost: 10,
            max_slots: 2,
            base_hp: 10,
            base_power_consumption: 0.0,
            base_heat_generation: 5.0,
            base_weight: 100,
        });
        
        // Create a temporary directory
        let temp_dir = env::temp_dir().join("hyperion_test_variants_validation_ok");
        let _ = fs::remove_dir_all(&temp_dir);
        let modules_dir = temp_dir.join("modules");
        let power_cores_dir = modules_dir.join("power-cores");
        fs::create_dir_all(&power_cores_dir).unwrap();
        
        // Create a valid variant matching the slot
        let power_core_yaml = r#"
id: test-reactor
type: power-core
name: "Test Reactor"
model: "Test Model"
manufacturer: "Test Corp"
desc: "A test power core"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 50
energy_production: 1000
energy_capacity: 5000
"#;
        fs::write(power_cores_dir.join("test-reactor.yaml"), power_core_yaml).unwrap();
        
        // Load with validation - should succeed
        let result = GameConfig::load_module_variants(modules_dir.clone(), Some(&slots));
        assert!(result.is_ok(), "Validation should succeed: {:?}", result.err());
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_variants_validation_failure() {
        use std::env;
        
        // Create module slots (only power-core)
        let mut slots = HashMap::new();
        slots.insert("power-core".to_string(), ModuleSlot {
            id: "power-core".to_string(),
            name: "Power Core".to_string(),
            description: "Provides power".to_string(),
            extended_desc: "Extended".to_string(),
            groups: vec![],
            required: true,
            has_varients: true,
            base_cost: 10,
            max_slots: 2,
            base_hp: 10,
            base_power_consumption: 0.0,
            base_heat_generation: 5.0,
            base_weight: 100,
        });
        
        // Create a temporary directory
        let temp_dir = env::temp_dir().join("hyperion_test_variants_validation_fail");
        let _ = fs::remove_dir_all(&temp_dir);
        let modules_dir = temp_dir.join("modules");
        let engines_dir = modules_dir.join("impulse-engines");
        fs::create_dir_all(&engines_dir).unwrap();
        
        // Create a variant with type that doesn't match any slot
        let engine_yaml = r#"
id: test-engine
type: impulse-engine
name: "Test Engine"
model: "Test Model"
manufacturer: "Test Corp"
desc: "A test engine"
lore: "Test lore"
cost: 150
additional_hp: 15
additional_power_consumption: 10.0
additional_heat_generation: 8.0
additional_weight: 100
max_thrust: 50000
"#;
        fs::write(engines_dir.join("test-engine.yaml"), engine_yaml).unwrap();
        
        // Load with validation - should fail
        let result = GameConfig::load_module_variants(modules_dir.clone(), Some(&slots));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not match any known module slot"));
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_load_module_variants_groups_by_type() {
        use std::env;
        
        // Create a temporary directory
        let temp_dir = env::temp_dir().join("hyperion_test_variants_grouping");
        let _ = fs::remove_dir_all(&temp_dir);
        let modules_dir = temp_dir.join("modules");
        let power_cores_dir = modules_dir.join("power-cores");
        fs::create_dir_all(&power_cores_dir).unwrap();
        
        // Create multiple variants of the same type
        let reactor1_yaml = r#"
id: fission-reactor
type: power-core
name: "Fission Reactor"
model: "Fission Model"
manufacturer: "Test Corp"
desc: "A fission reactor"
lore: "Test lore"
cost: 100
additional_hp: 10
additional_power_consumption: 0.0
additional_heat_generation: 5.0
additional_weight: 50
energy_production: 800
energy_capacity: 4000
"#;
        fs::write(power_cores_dir.join("fission-reactor.yaml"), reactor1_yaml).unwrap();
        
        let reactor2_yaml = r#"
id: fusion-reactor
type: power-core
name: "Fusion Reactor"
model: "Fusion Model"
manufacturer: "Test Corp"
desc: "A fusion reactor"
lore: "Test lore"
cost: 200
additional_hp: 20
additional_power_consumption: 0.0
additional_heat_generation: 10.0
additional_weight: 100
energy_production: 2000
energy_capacity: 10000
"#;
        fs::write(power_cores_dir.join("fusion-reactor.yaml"), reactor2_yaml).unwrap();
        
        // Load variants
        let result = GameConfig::load_module_variants(modules_dir.clone(), None);
        assert!(result.is_ok());
        
        let variants = result.unwrap();
        // Both variants should be grouped under power-core
        assert_eq!(variants.len(), 1);
        assert!(variants.contains_key("power-core"));
        assert_eq!(variants.get("power-core").unwrap().len(), 2);
        
        let power_core_variants = variants.get("power-core").unwrap();
        let ids: Vec<&str> = power_core_variants.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&"fission-reactor"));
        assert!(ids.contains(&"fusion-reactor"));
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}


