//! Ship blueprint compilation
//!
//! Converts validated ship blueprints into active ships ready for deployment
//! in the simulation. Handles initialization of all ship systems, assignment
//! of player roles, and spawning ships into the game world.

use crate::models::{Ship, ShipBlueprint, ShipStatus, Inventory, CompiledModule};
use crate::models::blueprint::ModuleInstance;
use crate::config::{GameConfig, ShipClassConfig, ModuleStats};
use crate::blueprint::BlueprintValidator;
use crate::state::GameWorld;
use std::collections::HashMap;
use uuid::Uuid;

/// Errors that can occur during ship compilation
#[derive(Debug, Clone, PartialEq)]
pub enum CompilationError {
    /// Blueprint validation failed
    ValidationFailed(Vec<String>),
    /// Ship class not found in configuration
    ShipClassNotFound(String),
    /// Blueprint not found
    BlueprintNotFound(String),
    /// Team not found
    TeamNotFound(String),
    /// Failed to calculate ship systems
    SystemCalculationFailed(String),
}

impl std::fmt::Display for CompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilationError::ValidationFailed(errors) => {
                write!(f, "Blueprint validation failed: {}", errors.join(", "))
            }
            CompilationError::ShipClassNotFound(id) => {
                write!(f, "Ship class not found: {}", id)
            }
            CompilationError::BlueprintNotFound(id) => {
                write!(f, "Blueprint not found: {}", id)
            }
            CompilationError::TeamNotFound(id) => {
                write!(f, "Team not found: {}", id)
            }
            CompilationError::SystemCalculationFailed(msg) => {
                write!(f, "Failed to calculate ship systems: {}", msg)
            }
        }
    }
}

impl std::error::Error for CompilationError {}

/// Ship compiler for converting blueprints to active ships
pub struct ShipCompiler<'a> {
    config: &'a GameConfig,
}

impl<'a> ShipCompiler<'a> {
    /// Create a new ship compiler
    pub fn new(config: &'a GameConfig) -> Self {
        Self { config }
    }

    /// Compile a blueprint into an active ship
    ///
    /// This performs the following steps:
    /// 1. Validate the blueprint is ready for compilation
    /// 2. Look up ship class configuration
    /// 3. Calculate ship systems (power, shields, hull, cooling)
    /// 4. Initialize ship status
    /// 5. Create ship inventory
    /// 6. Generate ship entity
    ///
    /// # Arguments
    ///
    /// * `blueprint` - The blueprint to compile
    /// * `world` - The game world (used for validation)
    ///
    /// # Returns
    ///
    /// Returns the compiled ship on success, or a CompilationError on failure.
    pub fn compile(
        &self,
        blueprint: &ShipBlueprint,
        world: &GameWorld,
    ) -> Result<Ship, CompilationError> {
        // Validate blueprint
        let validator = BlueprintValidator::new(
            self.config,
            world.players(),
            world.teams(),
        );
        
        let validation_result = validator.validate(blueprint);
        if !validation_result.is_valid {
            let errors: Vec<String> = validation_result.errors
                .iter()
                .map(|e| format!("{:?}", e))
                .collect();
            return Err(CompilationError::ValidationFailed(errors));
        }

        // Get ship class configuration
        let ship_class = self.get_ship_class(&blueprint.class)?;
        
        // Compile modules with resolved stats
        let compiled_modules = self.compile_modules(blueprint)?;

        // Calculate ship systems from compiled modules
        let status = self.initialize_ship_status(blueprint, ship_class, &compiled_modules)?;

        // Create inventory
        let inventory = self.initialize_inventory(blueprint);

        // Generate ship ID
        let ship_id = Uuid::new_v4().to_string();

        // Create ship entity
        Ok(Ship {
            id: ship_id,
            name: blueprint.name.clone(),
            class: blueprint.class.clone(),
            team_id: blueprint.team_id.clone(),
            player_roles: blueprint.player_roles.clone(),
            status,
            modules: compiled_modules,
            weapons: blueprint.weapons.clone(),
            inventory,
        })
    }
    
    /// Compile modules by resolving stats from variant configuration
    ///
    /// For each module instance in the blueprint:
    /// 1. Look up the variant configuration if kind is specified
    /// 2. Resolve stats from the variant YAML
    /// 3. Initialize runtime state (health, power, cooling)
    /// 4. Create CompiledModule instance
    fn compile_modules(&self, blueprint: &ShipBlueprint) -> Result<Vec<CompiledModule>, CompilationError> {
        let mut compiled = Vec::new();
        
        for module in &blueprint.modules {
            let compiled_module = self.compile_module(module)?;
            compiled.push(compiled_module);
        }
        
        Ok(compiled)
    }
    
    /// Compile a single module instance
    fn compile_module(&self, module: &ModuleInstance) -> Result<CompiledModule, CompilationError> {
        // Try to get variant stats if variant_id is specified
        let (name, stats) = if let Some(variant_id) = &module.variant_id {
            // Module has a variant configured
            if let Some(variant) = self.config.get_module_variant(&module.module_slot_id, variant_id) {
                (variant.name.clone(), variant.stats.clone())
            } else {
                // Variant not found, use defaults
                (module.module_slot_id.clone(), ModuleStats::default())
            }
        } else {
            // No variant, check if module slot requires one
            if self.config.get_module_variants(&module.module_slot_id).is_some() {
                // This module slot requires a variant but none is configured
                // Use default empty stats (validation should have caught this)
                (module.module_slot_id.clone(), ModuleStats::default())
            } else {
                // Module slot doesn't use variants, use base slot data
                if let Some(template) = self.config.modules.modules.get(&module.module_slot_id) {
                    (template.name.clone(), ModuleStats::default())
                } else {
                    (module.module_slot_id.clone(), ModuleStats::default())
                }
            }
        };

        // Get max health from module slot template or default to 100
        let max_health = self.config.modules.modules
            .get(&module.module_slot_id)
            .map(|t| t.hp as f32)
            .unwrap_or(100.0);

        Ok(CompiledModule {
            instance_id: module.id.clone(),
            module_id: module.module_slot_id.clone(), // Map new field to legacy for now
            kind: module.variant_id.clone(), // Map new field to legacy for now
            name,
            stats,
            current_health: max_health,
            max_health,
            operational: true,
            power_allocated: 1.0, // Full power by default
            cooling_allocated: 1.0, // Full cooling by default
        })
    }

    /// Get ship class configuration
    fn get_ship_class(&self, class_id: &str) -> Result<&ShipClassConfig, CompilationError> {
        self.config.ship_classes
            .iter()
            .find(|sc| sc.id == class_id)
            .ok_or_else(|| CompilationError::ShipClassNotFound(class_id.to_string()))
    }

    /// Initialize ship status based on blueprint and ship class
    fn initialize_ship_status(
        &self,
        blueprint: &ShipBlueprint,
        ship_class: &ShipClassConfig,
        compiled_modules: &[CompiledModule],
    ) -> Result<ShipStatus, CompilationError> {
        // Calculate base weight from modules and weapons
        let base_weight = self.calculate_base_weight_from_compiled(compiled_modules);

        // Initialize module health tracking
        let mut module_health = HashMap::new();
        for module in compiled_modules {
            module_health.insert(module.instance_id.clone(), module.current_health);
        }

        // Calculate power generation and capacity from compiled modules
        let (power_generation, power_capacity) = self.calculate_power_systems_from_compiled(compiled_modules);

        // Calculate cooling capacity from compiled modules
        let cooling_capacity = self.calculate_cooling_capacity_from_compiled(compiled_modules);

        // Create ship status
        let mut status = ShipStatus {
            hull: ship_class.base_hull,
            max_hull: ship_class.base_hull,
            shields: ship_class.base_shields,
            max_shields: ship_class.base_shields,
            shields_raised: false,
            power_generation,
            power_capacity,
            power_usage: 0.0,
            cooling_capacity,
            heat_generation: 0.0,
            base_weight,
            effective_weight: base_weight,
            status_effects: Vec::new(),
            module_health,
        };

        status.update_effective_weight();

        Ok(status)
    }
    
    /// Calculate base weight from compiled modules
    fn calculate_base_weight_from_compiled(&self, modules: &[CompiledModule]) -> f32 {
        modules.iter()
            .filter_map(|m| m.get_stat_f64("mass"))
            .sum::<f64>() as f32
    }
    
    /// Calculate power generation and capacity from compiled power core modules
    fn calculate_power_systems_from_compiled(&self, modules: &[CompiledModule]) -> (f32, f32) {
        let mut total_generation = 0.0;
        let mut total_capacity = 0.0;
        
        for module in modules {
            // Check for power generation (production stat)
            if let Some(production) = module.get_stat_f64("production") {
                total_generation += production as f32;
            }
            
            // Check for power capacity (max_energy stat)
            if let Some(capacity) = module.get_stat_f64("max_energy") {
                total_capacity += capacity as f32;
            }
        }
        
        // If no power modules found, provide minimum defaults
        if total_generation == 0.0 {
            total_generation = 100.0; // Minimum emergency power
        }
        if total_capacity == 0.0 {
            total_capacity = 1000.0; // Minimum emergency capacity
        }
        
        (total_generation, total_capacity)
    }
    
    /// Calculate cooling capacity from compiled cooling modules
    fn calculate_cooling_capacity_from_compiled(&self, modules: &[CompiledModule]) -> f32 {
        let total_cooling: f64 = modules.iter()
            .filter_map(|m| m.get_stat_f64("cooling_capacity"))
            .sum();
        
        // If no cooling modules found, provide minimum default
        if total_cooling == 0.0 {
            300.0 // Minimum passive cooling
        } else {
            total_cooling as f32
        }
    }

    /// Calculate base weight of the ship
    fn calculate_base_weight(&self, blueprint: &ShipBlueprint) -> f32 {
    // TODO: Look up actual weights from configuration
    // For now, use placeholder values
    0.0 // Function not used; placeholder implementation
    }

    /// Calculate power generation and capacity from power core modules
    fn calculate_power_systems(&self, blueprint: &ShipBlueprint) -> (f32, f32) {
        // TODO: Look up power cores in module definitions and sum their values
        // For now, use placeholder values based on module count
        let power_cores = blueprint.modules.iter()
            .filter(|m| m.module_slot_id.contains("power") || m.module_slot_id.contains("reactor") || m.module_slot_id.contains("battery"))
            .count();
        
        let generation = power_cores as f32 * 50.0; // 50 units per power core
        let capacity = power_cores as f32 * 1000.0; // 1000 units capacity per power core
        
        (generation, capacity)
    }

    /// Calculate cooling capacity from cooling modules
    fn calculate_cooling_capacity(&self, blueprint: &ShipBlueprint) -> f32 {
        // TODO: Look up cooling modules in module definitions
        // For now, use placeholder value
        100.0 * blueprint.modules.len() as f32
    }

    /// Initialize ship inventory
    fn initialize_inventory(&self, _blueprint: &ShipBlueprint) -> Inventory {
        // Start with empty inventory
        // TODO: Add starting ammunition based on equipped weapons
        Inventory::new()
    }
}

/// Compile and spawn a ship into the game world
///
/// This is a convenience function that compiles a blueprint and adds
/// the resulting ship to the game world in one operation.
///
/// # Arguments
///
/// * `blueprint_id` - ID of the blueprint to compile
/// * `world` - Mutable reference to the game world
/// * `config` - Game configuration
///
/// # Returns
///
/// Returns the ID of the newly spawned ship on success.
pub fn compile_and_spawn(
    blueprint_id: &str,
    world: &mut GameWorld,
    config: &GameConfig,
) -> Result<String, CompilationError> {
    // Get blueprint
    let blueprint = world.get_blueprint(blueprint_id)
        .ok_or_else(|| CompilationError::BlueprintNotFound(blueprint_id.to_string()))?
        .clone();

    // Verify team exists
    if !world.teams().contains_key(&blueprint.team_id) {
        return Err(CompilationError::TeamNotFound(blueprint.team_id.clone()));
    }

    // Compile blueprint
    let compiler = ShipCompiler::new(config);
    let ship = compiler.compile(&blueprint, world)?;

    // Store ship ID before moving ship into world
    let ship_id = ship.id.clone();

    // Add ship to world
    world.register_ship(ship);

    // TODO: Trigger ship spawn event in simulation

    Ok(ship_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::role::ShipRole;
    use crate::config::{AiConfig, FactionsConfig, MapConfig, ModulesConfig, RacesConfig, SimulationConfig};

    fn create_test_config() -> GameConfig {
        use crate::config::{ShipClassConfig, ShipSize, ShipClassRole};
        use std::collections::HashMap;
        
        let mut ship_class = ShipClassConfig {
            name: "Test Cruiser".to_string(),
            description: "A test ship class".to_string(),
            base_hull: 1000.0,
            base_shields: 500.0,
            max_weight: 5000.0,
            max_modules: 10,
            size: ShipSize::Medium,
            role: ShipClassRole::Combat,
            build_points: 1000.0,
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
            notable_ships: vec![],
        };
        ship_class.set_id("test_cruiser".to_string());

        GameConfig {
            ai: AiConfig { difficulty: "medium".to_string(), response_time: 1.0 },
            factions: FactionsConfig { factions: vec![] },
            map: MapConfig { galaxy_size: 1000, star_density: 0.5 },
            modules: ModulesConfig { modules: HashMap::new() },
            races: RacesConfig { races: vec![] },
            simulation: SimulationConfig { tick_rate: 60.0, physics_enabled: true },
            ship_classes: vec![ship_class],
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

    fn create_test_world() -> GameWorld {
        let mut world = GameWorld::new();
        
        // Create player
        let player_id = world.register_player("TestPlayer".to_string()).unwrap();
        
        // Create team
        let team_id = world.create_team("TestTeam".to_string(), "alliance".to_string()).unwrap();
        
        // Add player to team
        world.add_player_to_team(&team_id, &player_id).unwrap();
        
        // Create blueprint
        let blueprint_id = world.create_blueprint(
            "Test Ship".to_string(),
            "test_cruiser".to_string(),
            team_id.clone(),
        ).unwrap();
        
        // Setup blueprint with player and roles
        let bp = world.get_blueprint_mut(&blueprint_id).unwrap();
        bp.set_player_roles(player_id.clone(), vec![ShipRole::Captain]);
        bp.mark_ready(player_id);
        
        world
    }

    #[test]
    fn test_compile_valid_blueprint() {
        let config = create_test_config();
        let world = create_test_world();
        
        let blueprint = world.get_all_blueprints()[0].clone();
        
        let compiler = ShipCompiler::new(&config);
        let result = compiler.compile(&blueprint, &world);
        
        assert!(result.is_ok());
        let ship = result.unwrap();
        assert_eq!(ship.name, "Test Ship");
        assert_eq!(ship.class, "test_cruiser");
        assert_eq!(ship.status.max_hull, 1000.0);
        assert_eq!(ship.status.max_shields, 500.0);
    }

    #[test]
    fn test_compile_invalid_ship_class() {
        let config = create_test_config();
        let world = create_test_world();
        
        let mut blueprint = world.get_all_blueprints()[0].clone();
        blueprint.class = "nonexistent".to_string();
        
        let compiler = ShipCompiler::new(&config);
        let result = compiler.compile(&blueprint, &world);
        
        assert!(result.is_err());
        // The validation will fail because ship class doesn't exist
        match result {
            Err(CompilationError::ValidationFailed(_)) => {
                // Expected - validation catches missing ship class
            }
            Err(CompilationError::ShipClassNotFound(id)) => {
                // Also acceptable if it gets past validation somehow
                assert_eq!(id, "nonexistent");
            }
            _ => panic!("Expected ValidationFailed or ShipClassNotFound error, got: {:?}", result),
        }
    }

    #[test]
    fn test_compile_and_spawn() {
        let config = create_test_config();
        let mut world = create_test_world();
        
        let blueprint_id = world.get_all_blueprints()[0].id.clone();
        
        let result = compile_and_spawn(&blueprint_id, &mut world, &config);
        
        assert!(result.is_ok());
        let ship_id = result.unwrap();
        
        // Verify ship was added to world
        let ship = world.get_ship(&ship_id);
        assert!(ship.is_some());
        assert_eq!(ship.unwrap().name, "Test Ship");
    }

    #[test]
    fn test_compile_and_spawn_blueprint_not_found() {
        let config = create_test_config();
        let mut world = create_test_world();
        
        let result = compile_and_spawn("nonexistent", &mut world, &config);
        
        assert!(result.is_err());
        match result {
            Err(CompilationError::BlueprintNotFound(_)) => {}
            _ => panic!("Expected BlueprintNotFound error"),
        }
    }

    #[test]
    fn test_ship_systems_initialization() {
        let config = create_test_config();
        let world = create_test_world();
        
        let blueprint = world.get_all_blueprints()[0].clone();
        
        let compiler = ShipCompiler::new(&config);
        let ship = compiler.compile(&blueprint, &world).unwrap();
        
        // Verify systems are initialized
        assert_eq!(ship.status.hull, ship.status.max_hull);
        assert_eq!(ship.status.shields, ship.status.max_shields);
        assert!(!ship.status.shields_raised);
        assert_eq!(ship.status.status_effects.len(), 0);
        assert!(ship.inventory.ammunition.is_empty());
    }
}
