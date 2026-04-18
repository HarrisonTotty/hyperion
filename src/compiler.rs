//! Ship blueprint compilation
//!
//! Converts validated ship blueprints into active ships ready for deployment
//! in the simulation. Handles initialization of all ship systems, assignment
//! of player roles, and spawning ships into the game world.

use crate::blueprint::BlueprintValidator;
use crate::config::{GameConfig, ModuleStats, ShipClassConfig};
use crate::models::blueprint::ModuleInstance;
use crate::models::{CompiledModule, Inventory, Ship, ShipBlueprint, ShipStatus};
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
    /// Team has insufficient credits to compile ship
    InsufficientCredits {
        /// Credits required to compile the ship
        required: i64,
        /// Credits currently available to the team
        available: i64,
    },
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
            CompilationError::InsufficientCredits {
                required,
                available,
            } => {
                write!(
                    f,
                    "Insufficient credits: need {}, have {}",
                    required, available
                )
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
        let validator = BlueprintValidator::new(self.config, world.players(), world.teams());

        let validation_result = validator.validate(blueprint);
        if !validation_result.is_valid {
            let errors: Vec<String> = validation_result
                .errors
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
    fn compile_modules(
        &self,
        blueprint: &ShipBlueprint,
    ) -> Result<Vec<CompiledModule>, CompilationError> {
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
            if let Some(variant) = self
                .config
                .get_module_variant(&module.module_slot_id, variant_id)
            {
                (variant.name.clone(), variant.stats.clone())
            } else {
                // Variant not found, use defaults
                (module.module_slot_id.clone(), ModuleStats::default())
            }
        } else {
            // No variant, check if module slot requires one
            if self
                .config
                .get_module_variants(&module.module_slot_id)
                .is_some()
            {
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
        let max_health = self
            .config
            .modules
            .modules
            .get(&module.module_slot_id)
            .map(|t| t.hp as f32)
            .unwrap_or(100.0);

        Ok(CompiledModule {
            instance_id: module.id.clone(),
            module_id: module.module_slot_id.clone(), // Map new field to legacy for now
            kind: module.variant_id.clone(),          // Map new field to legacy for now
            name,
            stats,
            current_health: max_health,
            max_health,
            operational: true,
            power_allocated: 1.0,   // Full power by default
            cooling_allocated: 1.0, // Full cooling by default
        })
    }

    /// Get ship class configuration
    fn get_ship_class(&self, class_id: &str) -> Result<&ShipClassConfig, CompilationError> {
        self.config
            .ship_classes
            .iter()
            .find(|sc| sc.id == class_id)
            .ok_or_else(|| CompilationError::ShipClassNotFound(class_id.to_string()))
    }

    /// Initialize ship status based on blueprint and ship class
    fn initialize_ship_status(
        &self,
        _blueprint: &ShipBlueprint,
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
        let (power_generation, power_capacity) =
            self.calculate_power_systems_from_compiled(compiled_modules);

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
        modules
            .iter()
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
        let total_cooling: f64 = modules
            .iter()
            .filter_map(|m| m.get_stat_f64("cooling_capacity"))
            .sum();

        // If no cooling modules found, provide minimum default
        if total_cooling == 0.0 {
            300.0 // Minimum passive cooling
        } else {
            total_cooling as f32
        }
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
    let blueprint = world
        .get_blueprint(blueprint_id)
        .ok_or_else(|| CompilationError::BlueprintNotFound(blueprint_id.to_string()))?
        .clone();

    // Verify team exists and get current credits
    let team = world
        .get_team(&blueprint.team_id)
        .ok_or_else(|| CompilationError::TeamNotFound(blueprint.team_id.clone()))?;
    let available_credits = team.credits;

    // Calculate total credit cost
    let total_cost = calculate_blueprint_cost(&blueprint, config)?;

    // Check if team has enough credits
    if available_credits < total_cost {
        return Err(CompilationError::InsufficientCredits {
            required: total_cost,
            available: available_credits,
        });
    }

    // Compile blueprint
    let compiler = ShipCompiler::new(config);
    let ship = compiler.compile(&blueprint, world)?;

    // Store ship ID before moving ship into world
    let ship_id = ship.id.clone();

    // Deduct credits from team
    world
        .deduct_team_credits(&blueprint.team_id, total_cost)
        .map_err(CompilationError::SystemCalculationFailed)?;

    // Add ship to world
    world.register_ship(ship);

    // TODO: Trigger ship spawn event in simulation

    Ok(ship_id)
}

/// Calculate the total credit cost of a blueprint
///
/// This includes:
/// - Ship class cost
/// - Module slot costs (credit_cost for each installed module)
/// - Module variant costs (credit_cost for each selected variant)
///
/// # Arguments
///
/// * `blueprint` - The blueprint to calculate cost for
/// * `config` - Game configuration
///
/// # Returns
///
/// Returns the total credit cost, or an error if ship class is not found.
pub fn calculate_blueprint_cost(
    blueprint: &ShipBlueprint,
    config: &GameConfig,
) -> Result<i64, CompilationError> {
    // Get ship class cost
    let ship_class = config
        .get_ship_class(&blueprint.class)
        .ok_or_else(|| CompilationError::ShipClassNotFound(blueprint.class.clone()))?;

    let mut total_cost: i64 = ship_class.cost;

    // Add module slot and variant costs
    for module in &blueprint.modules {
        // Add module slot cost
        if let Some(slot) = config.get_module_slot(&module.module_slot_id) {
            total_cost += slot.credit_cost;
        }

        // Add variant cost if selected
        if let Some(ref variant_id) = module.variant_id
            && let Some(variant) = config.get_module_variant(&module.module_slot_id, variant_id)
        {
            total_cost += variant.credit_cost;
        }
    }

    Ok(total_cost)
}

/// Calculate the credit value of an active ship (for refunds)
///
/// This includes:
/// - Ship class cost
/// - Module slot costs (credit_cost for each module)
/// - Module variant costs (credit_cost for each variant)
///
/// # Arguments
///
/// * `ship` - The active ship to calculate value for
/// * `config` - Game configuration
///
/// # Returns
///
/// Returns the total credit value (100% refund rate).
pub fn calculate_ship_value(ship: &Ship, config: &GameConfig) -> i64 {
    // Get ship class cost
    let ship_class_cost = config
        .get_ship_class(&ship.class)
        .map(|sc| sc.cost)
        .unwrap_or(0);

    let mut total_value: i64 = ship_class_cost;

    // Add module slot and variant costs
    for module in &ship.modules {
        // Add module slot cost (module_id is the slot type)
        if let Some(slot) = config.get_module_slot(&module.module_id) {
            total_value += slot.credit_cost;
        }

        // Add variant cost if present (kind is the variant_id)
        if let Some(ref variant_id) = module.kind
            && let Some(variant) = config.get_module_variant(&module.module_id, variant_id)
        {
            total_value += variant.credit_cost;
        }
    }

    total_value
}

/// Remove a ship and refund its credit value to the team
///
/// This is a convenience function that calculates the ship's value,
/// refunds the credits to the team, and removes the ship from the world.
///
/// # Arguments
///
/// * `ship_id` - ID of the ship to remove
/// * `world` - Mutable reference to the game world
/// * `config` - Game configuration
///
/// # Returns
///
/// Returns the amount refunded on success, or an error message.
pub fn remove_ship_with_refund(
    ship_id: &str,
    world: &mut GameWorld,
    config: &GameConfig,
) -> Result<i64, String> {
    // Get ship to calculate refund
    let ship = world
        .get_ship(ship_id)
        .ok_or_else(|| format!("Ship {} not found", ship_id))?;

    let team_id = ship.team_id.clone();
    let refund_amount = calculate_ship_value(ship, config);

    // Remove ship from world
    world.remove_ship(ship_id)?;

    // Refund credits to team
    world.refund_team_credits(&team_id, refund_amount)?;

    Ok(refund_amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_utils::{create_test_game_config, create_test_ship_class};
    use crate::models::role::ShipRole;

    // Tests assert `ship.class == "test_cruiser"`, so preserve the legacy id.
    fn create_test_config() -> GameConfig {
        create_test_game_config()
            .with_ship_class(create_test_ship_class("test_cruiser", "Test Cruiser"))
    }

    fn create_test_world() -> GameWorld {
        create_test_world_with_credits(1_000_000) // Default 1M credits
    }

    fn create_test_world_with_credits(starting_credits: i64) -> GameWorld {
        let mut world = GameWorld::new();

        // Create player
        let player_id = world.register_player("TestPlayer".to_string()).unwrap();

        // Create team with credits
        let team_id = world
            .create_team_with_credits(
                "TestTeam".to_string(),
                "alliance".to_string(),
                starting_credits,
            )
            .unwrap();

        // Add player to team
        world.add_player_to_team(&team_id, &player_id).unwrap();

        // Create blueprint
        let blueprint_id = world
            .create_blueprint(
                "Test Ship".to_string(),
                "test_cruiser".to_string(),
                team_id.clone(),
            )
            .unwrap();

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
            _ => panic!(
                "Expected ValidationFailed or ShipClassNotFound error, got: {:?}",
                result
            ),
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

    #[test]
    fn test_calculate_blueprint_cost() {
        let config = create_test_config();
        let world = create_test_world();

        let blueprint = world.get_all_blueprints()[0].clone();

        // Blueprint has ship class cost = 50000, no modules
        let cost = calculate_blueprint_cost(&blueprint, &config).unwrap();
        assert_eq!(cost, 50000);
    }

    #[test]
    fn test_compile_and_spawn_deducts_credits() {
        let config = create_test_config();
        let mut world = create_test_world_with_credits(100_000);

        let team_id = {
            let blueprint = world.get_all_blueprints()[0].clone();
            blueprint.team_id.clone()
        };
        let blueprint_id = world.get_all_blueprints()[0].id.clone();

        // Verify initial credits
        assert_eq!(world.get_team(&team_id).unwrap().credits, 100_000);

        // Compile ship (costs 50000)
        let result = compile_and_spawn(&blueprint_id, &mut world, &config);
        assert!(result.is_ok());

        // Verify credits were deducted
        let team = world.get_team(&team_id).unwrap();
        assert_eq!(team.credits, 50_000); // 100000 - 50000
    }

    #[test]
    fn test_compile_and_spawn_insufficient_credits() {
        let config = create_test_config();
        let mut world = create_test_world_with_credits(10_000); // Not enough for 50000 cost ship

        let blueprint_id = world.get_all_blueprints()[0].id.clone();

        let result = compile_and_spawn(&blueprint_id, &mut world, &config);

        assert!(result.is_err());
        match result {
            Err(CompilationError::InsufficientCredits {
                required,
                available,
            }) => {
                assert_eq!(required, 50_000);
                assert_eq!(available, 10_000);
            }
            _ => panic!("Expected InsufficientCredits error, got: {:?}", result),
        }

        // Verify no ship was created
        assert_eq!(world.get_all_ships().len(), 0);
    }

    #[test]
    fn test_calculate_ship_value() {
        let config = create_test_config();
        let mut world = create_test_world_with_credits(100_000);

        let blueprint_id = world.get_all_blueprints()[0].id.clone();

        // Compile ship
        let ship_id = compile_and_spawn(&blueprint_id, &mut world, &config).unwrap();

        // Get ship value
        let ship = world.get_ship(&ship_id).unwrap();
        let value = calculate_ship_value(ship, &config);

        // Ship class cost = 50000, no modules
        assert_eq!(value, 50_000);
    }

    #[test]
    fn test_remove_ship_with_refund() {
        let config = create_test_config();
        let mut world = create_test_world_with_credits(100_000);

        let team_id = {
            let blueprint = world.get_all_blueprints()[0].clone();
            blueprint.team_id.clone()
        };
        let blueprint_id = world.get_all_blueprints()[0].id.clone();

        // Compile ship (costs 50000)
        let ship_id = compile_and_spawn(&blueprint_id, &mut world, &config).unwrap();
        assert_eq!(world.get_team(&team_id).unwrap().credits, 50_000);

        // Remove ship with refund
        let refund = remove_ship_with_refund(&ship_id, &mut world, &config).unwrap();
        assert_eq!(refund, 50_000);

        // Verify credits were refunded
        assert_eq!(world.get_team(&team_id).unwrap().credits, 100_000);

        // Verify ship was removed
        assert!(world.get_ship(&ship_id).is_none());
    }
}
