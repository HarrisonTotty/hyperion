//! Blueprint business logic and validation
//!
//! Provides comprehensive validation for ship blueprints including:
//! - Ship class validation
//! - Team and player validation
//! - Role assignment validation
//! - Weight calculation and limits
//! - Module count and restrictions
//! - Module and weapon configuration validation

use crate::config::{GameConfig, ShipClassConfig};
use crate::models::{Player, ShipBlueprint, Team};
use std::collections::HashMap;

/// Validation error types
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Ship class not found in configuration
    InvalidShipClass(String),
    /// Team not found
    InvalidTeam(String),
    /// Player not found
    InvalidPlayer(String),
    /// Player not on the blueprint's team
    PlayerNotOnTeam(String),
    /// Player has no assigned roles
    NoRolesAssigned(String),
    /// Total weight exceeds ship class limit
    WeightLimitExceeded { current: u32, max: u32 },
    /// Module count exceeds ship class limit
    ModuleCountExceeded { current: usize, max: usize },
    /// Module configuration invalid
    InvalidModuleConfiguration(String),
    /// Weapon configuration invalid
    InvalidWeaponConfiguration(String),
    /// Required module missing
    RequiredModuleMissing(String),
    /// Module exceeds max_allowed count
    ModuleCountLimitExceeded {
        module_slot_id: String,
        current: usize,
        max: u32,
    },
    /// Module requires variant selection but variant_id is not configured
    VariantNotConfigured { module_slot_id: String },
    /// Not all players are ready
    PlayersNotReady,
    /// No players assigned to blueprint
    NoPlayers,
}

/// Validation warning types (non-blocking)
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationWarning {
    /// Blueprint has no modules
    NoModules,
    /// Blueprint has no weapons
    NoWeapons,
    /// Ship is under-equipped (low module count)
    UnderEquipped,
    /// Some modules lack configuration
    UnconfiguredModules(Vec<String>),
}

/// Complete validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// List of blocking errors
    pub errors: Vec<ValidationError>,
    /// List of non-blocking warnings
    pub warnings: Vec<ValidationWarning>,
    /// Total weight of all modules and weapons
    pub total_weight: u32,
    /// Whether the blueprint is valid (no errors)
    pub is_valid: bool,
}

impl ValidationResult {
    /// Create a new validation result
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            total_weight: 0,
            is_valid: true,
        }
    }

    /// Add an error to the result
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
        self.is_valid = false;
    }

    /// Add a warning to the result
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    /// Set the total weight
    pub fn set_total_weight(&mut self, weight: u32) {
        self.total_weight = weight;
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Blueprint validator
pub struct BlueprintValidator<'a> {
    config: &'a GameConfig,
    players: &'a HashMap<String, Player>,
    teams: &'a HashMap<String, Team>,
}

/// Context passed to every validation rule.
///
/// Holds the blueprint being checked plus data that multiple rules need
/// (currently just the resolved ship class). Rules mutate `result`.
struct RuleContext<'a> {
    blueprint: &'a ShipBlueprint,
    ship_class: &'a ShipClassConfig,
}

/// Signature shared by every validation rule.
///
/// Adding a new validation rule means writing one function with this
/// signature and appending it to [`RULES`].
type ValidationRule =
    for<'v, 'c> fn(&BlueprintValidator<'v>, &RuleContext<'c>, &mut ValidationResult);

/// All rules run after ship-class resolution, in order.
const RULES: &[ValidationRule] = &[
    rule_team_exists,
    rule_players_and_roles,
    rule_weight_limit,
    rule_module_count,
    rule_unconfigured_modules,
    rule_required_modules,
    rule_max_allowed,
    rule_module_variants,
    rule_no_modules_warning,
    rule_no_weapons_warning,
    rule_under_equipped,
    rule_ready_status,
];

// ==================== Validation Rules ====================
//
// Rules are free functions so they can be collected into a `fn`-pointer
// table that is generic over the validator's lifetime. Each rule inspects
// the blueprint via `ctx` and mutates `result` in place.

fn rule_team_exists(
    validator: &BlueprintValidator,
    ctx: &RuleContext,
    result: &mut ValidationResult,
) {
    if let Err(e) = validator.validate_team(&ctx.blueprint.team_id) {
        result.add_error(e);
    }
}

fn rule_players_and_roles(
    validator: &BlueprintValidator,
    ctx: &RuleContext,
    result: &mut ValidationResult,
) {
    validator.validate_players_and_roles(ctx.blueprint, result);
}

fn rule_weight_limit(_: &BlueprintValidator, ctx: &RuleContext, result: &mut ValidationResult) {
    if result.total_weight > ctx.ship_class.max_weight as u32 {
        result.add_error(ValidationError::WeightLimitExceeded {
            current: result.total_weight,
            max: ctx.ship_class.max_weight as u32,
        });
    }
}

fn rule_module_count(_: &BlueprintValidator, ctx: &RuleContext, result: &mut ValidationResult) {
    let module_count = ctx.blueprint.modules.len();
    if module_count > ctx.ship_class.max_modules as usize {
        result.add_error(ValidationError::ModuleCountExceeded {
            current: module_count,
            max: ctx.ship_class.max_modules as usize,
        });
    }
}

fn rule_unconfigured_modules(
    _: &BlueprintValidator,
    ctx: &RuleContext,
    result: &mut ValidationResult,
) {
    let unconfigured: Vec<String> = ctx
        .blueprint
        .modules
        .iter()
        .filter(|m| m.variant_id.is_none())
        .map(|m| m.module_slot_id.clone())
        .collect();
    if !unconfigured.is_empty() {
        result.add_warning(ValidationWarning::UnconfiguredModules(unconfigured));
    }
}

fn rule_required_modules(
    validator: &BlueprintValidator,
    ctx: &RuleContext,
    result: &mut ValidationResult,
) {
    validator.validate_required_modules(ctx.blueprint, result);
}

fn rule_max_allowed(
    validator: &BlueprintValidator,
    ctx: &RuleContext,
    result: &mut ValidationResult,
) {
    validator.validate_max_allowed(ctx.blueprint, result);
}

fn rule_module_variants(
    validator: &BlueprintValidator,
    ctx: &RuleContext,
    result: &mut ValidationResult,
) {
    validator.validate_module_variants(ctx.blueprint, result);
}

fn rule_no_modules_warning(
    _: &BlueprintValidator,
    ctx: &RuleContext,
    result: &mut ValidationResult,
) {
    if ctx.blueprint.modules.is_empty() {
        result.add_warning(ValidationWarning::NoModules);
    }
}

fn rule_no_weapons_warning(
    _: &BlueprintValidator,
    ctx: &RuleContext,
    result: &mut ValidationResult,
) {
    if ctx.blueprint.weapons.is_empty() {
        result.add_warning(ValidationWarning::NoWeapons);
    }
}

fn rule_under_equipped(_: &BlueprintValidator, ctx: &RuleContext, result: &mut ValidationResult) {
    if ctx.blueprint.modules.len() < ctx.ship_class.max_modules as usize / 2 {
        result.add_warning(ValidationWarning::UnderEquipped);
    }
}

fn rule_ready_status(_: &BlueprintValidator, ctx: &RuleContext, result: &mut ValidationResult) {
    if !ctx.blueprint.all_players_ready() {
        result.add_error(ValidationError::PlayersNotReady);
    }
}

impl<'a> BlueprintValidator<'a> {
    /// Create a new blueprint validator
    pub fn new(
        config: &'a GameConfig,
        players: &'a HashMap<String, Player>,
        teams: &'a HashMap<String, Team>,
    ) -> Self {
        Self {
            config,
            players,
            teams,
        }
    }

    /// Validate a complete blueprint
    pub fn validate(&self, blueprint: &ShipBlueprint) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Ship class is load-bearing for most rules: if we cannot resolve it,
        // no further rule can produce meaningful output.
        let ship_class = match self.validate_ship_class(&blueprint.class) {
            Ok(class) => class,
            Err(e) => {
                result.add_error(e);
                return result;
            }
        };

        result.set_total_weight(self.calculate_total_weight(blueprint));

        let ctx = RuleContext {
            blueprint,
            ship_class,
        };
        for rule in RULES {
            rule(self, &ctx, &mut result);
        }

        result
    }

    /// Validate ship class exists in configuration
    fn validate_ship_class(&self, class_id: &str) -> Result<&ShipClassConfig, ValidationError> {
        self.config
            .ship_classes
            .iter()
            .find(|sc| sc.id == class_id)
            .ok_or_else(|| ValidationError::InvalidShipClass(class_id.to_string()))
    }

    /// Validate team exists
    fn validate_team(&self, team_id: &str) -> Result<&Team, ValidationError> {
        self.teams
            .get(team_id)
            .ok_or_else(|| ValidationError::InvalidTeam(team_id.to_string()))
    }

    /// Validate player exists
    fn validate_player(&self, player_id: &str) -> Result<&Player, ValidationError> {
        self.players
            .get(player_id)
            .ok_or_else(|| ValidationError::InvalidPlayer(player_id.to_string()))
    }

    /// Validate players and their role assignments
    fn validate_players_and_roles(&self, blueprint: &ShipBlueprint, result: &mut ValidationResult) {
        // Check if there are any players
        if blueprint.player_roles.is_empty() {
            result.add_error(ValidationError::NoPlayers);
            return;
        }

        // Validate each player
        for (player_id, roles) in &blueprint.player_roles {
            // Check if player exists
            if let Err(e) = self.validate_player(player_id) {
                result.add_error(e);
                continue;
            }

            // Check if player is on the team
            let team = match self.teams.get(&blueprint.team_id) {
                Some(t) => t,
                None => continue, // Team validation error already added
            };

            if !team.members.contains(player_id) {
                result.add_error(ValidationError::PlayerNotOnTeam(player_id.clone()));
            }

            // Check if player has roles assigned
            if roles.is_empty() {
                result.add_error(ValidationError::NoRolesAssigned(player_id.clone()));
            }
        }
    }

    /// Validate that all required modules are present
    ///
    /// Checks modules.yaml for modules with `required: true` and ensures
    /// at least one instance exists in the blueprint.
    fn validate_required_modules(&self, blueprint: &ShipBlueprint, result: &mut ValidationResult) {
        for (module_slot_id, template) in &self.config.modules.modules {
            if template.required {
                // Check if this required module exists in the blueprint
                let has_module = blueprint
                    .modules
                    .iter()
                    .any(|m| &m.module_slot_id == module_slot_id);
                if !has_module {
                    result.add_error(ValidationError::RequiredModuleMissing(
                        template.name.clone(),
                    ));
                }
            }
        }
    }

    /// Validate that no module exceeds max_allowed count
    ///
    /// Checks modules.yaml for max_allowed constraints and counts instances
    /// in the blueprint to ensure limits are not exceeded.
    fn validate_max_allowed(&self, blueprint: &ShipBlueprint, result: &mut ValidationResult) {
        use std::collections::HashMap;

        // Count instances of each module type
        let mut module_counts: HashMap<String, usize> = HashMap::new();
        for module in &blueprint.modules {
            *module_counts
                .entry(module.module_slot_id.clone())
                .or_insert(0) += 1;
        }
        // Check against max_allowed constraints
        for (module_slot_id, count) in module_counts {
            if let Some(template) = self.config.modules.modules.get(&module_slot_id)
                && template.max_allowed > 0
                && count > template.max_allowed as usize
            {
                result.add_error(ValidationError::ModuleCountLimitExceeded {
                    module_slot_id: template.name.clone(),
                    current: count,
                    max: template.max_allowed,
                });
            }
        }
    }

    /// Validate that modules requiring variant selection have kind configured
    ///
    /// Checks if a module type has variants defined in config. If so, the module
    /// instance must have a `kind` field configured with a valid variant ID.
    fn validate_module_variants(&self, blueprint: &ShipBlueprint, result: &mut ValidationResult) {
        for module in &blueprint.modules {
            // Check if this module slot has variants
            if let Some(_variants) = self.config.get_module_variants(&module.module_slot_id) {
                // Module slot has variants, so variant_id must be configured
                if module.variant_id.is_none() {
                    result.add_error(ValidationError::VariantNotConfigured {
                        module_slot_id: module.module_slot_id.clone(),
                    });
                } else if let Some(variant_id) = &module.variant_id {
                    // Validate that the variant exists in available variants
                    if self
                        .config
                        .get_module_variant(&module.module_slot_id, variant_id)
                        .is_none()
                    {
                        result.add_error(ValidationError::InvalidModuleConfiguration(format!(
                            "Invalid variant '{}' for module slot '{}'",
                            variant_id, module.module_slot_id
                        )));
                    }
                }
            }
        }
    }

    /// Calculate total weight of all modules and weapons
    fn calculate_total_weight(&self, blueprint: &ShipBlueprint) -> u32 {
        let mut total_weight = 0u32;

        // Add module weights
        for _module in &blueprint.modules {
            // In a full implementation, we'd look up the module's weight from config
            // For now, use a placeholder value
            total_weight += 100; // Placeholder: 100kg per module
        }

        // Add weapon weights
        for _weapon in &blueprint.weapons {
            // In a full implementation, we'd look up the weapon's weight from config
            // For now, use a placeholder value
            total_weight += 50; // Placeholder: 50kg per weapon
        }

        total_weight
    }

    /// Quick validation check (just errors, no warnings)
    pub fn is_valid(&self, blueprint: &ShipBlueprint) -> bool {
        let result = self.validate(blueprint);
        result.is_valid
    }

    /// Get list of validation errors as strings
    pub fn get_error_messages(&self, blueprint: &ShipBlueprint) -> Vec<String> {
        let result = self.validate(blueprint);
        result.errors.iter().map(|e| format!("{:?}", e)).collect()
    }

    /// Get list of validation warnings as strings
    pub fn get_warning_messages(&self, blueprint: &ShipBlueprint) -> Vec<String> {
        let result = self.validate(blueprint);
        result.warnings.iter().map(|w| format!("{:?}", w)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_utils::{create_test_game_config, create_test_ship_class};
    use crate::models::role::ShipRole;

    // test_validate_weight_limit relies on a small max_weight (1000kg) so that
    // 15 placeholder modules at ~100kg each trip the weight-limit validator.
    fn create_test_config() -> GameConfig {
        let mut ship_class = create_test_ship_class("test_cruiser", "Test Cruiser");
        ship_class.base_hull = 500.0;
        ship_class.base_shields = 250.0;
        ship_class.max_weight = 1000.0;
        ship_class.build_points = 500.0;
        create_test_game_config().with_ship_class(ship_class)
    }

    fn create_test_player(id: &str) -> Player {
        Player {
            id: id.to_string(),
            name: format!("Player {}", id),
        }
    }

    fn create_test_team(id: &str) -> Team {
        Team {
            id: id.to_string(),
            name: format!("Team {}", id),
            faction: "alliance".to_string(),
            members: vec![],
            credits: 1_000_000,
        }
    }

    #[test]
    fn test_validate_ship_class_exists() {
        let config = create_test_config();
        let players = HashMap::new();
        let teams = HashMap::new();

        let validator = BlueprintValidator::new(&config, &players, &teams);
        assert!(validator.validate_ship_class("test_cruiser").is_ok());
        assert!(validator.validate_ship_class("nonexistent").is_err());
    }

    #[test]
    fn test_validate_no_players() {
        let config = create_test_config();
        let players = HashMap::new();
        let teams = HashMap::new();

        let blueprint = ShipBlueprint::new(
            "Test Ship".to_string(),
            "test_cruiser".to_string(),
            "team1".to_string(),
        );

        let validator = BlueprintValidator::new(&config, &players, &teams);
        let result = validator.validate(&blueprint);

        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::NoPlayers))
        );
    }

    #[test]
    fn test_validate_player_no_roles() {
        let config = create_test_config();

        let mut players = HashMap::new();
        players.insert("player1".to_string(), create_test_player("player1"));

        let mut teams = HashMap::new();
        let mut team = create_test_team("team1");
        team.members.push("player1".to_string());
        teams.insert("team1".to_string(), team);

        let mut blueprint = ShipBlueprint::new(
            "Test Ship".to_string(),
            "test_cruiser".to_string(),
            "team1".to_string(),
        );
        blueprint.set_player_roles("player1".to_string(), vec![]);

        let validator = BlueprintValidator::new(&config, &players, &teams);
        let result = validator.validate(&blueprint);

        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::NoRolesAssigned(_)))
        );
    }

    #[test]
    fn test_validate_players_not_ready() {
        let config = create_test_config();

        let mut players = HashMap::new();
        players.insert("player1".to_string(), create_test_player("player1"));

        let mut teams = HashMap::new();
        let mut team = create_test_team("team1");
        team.members.push("player1".to_string());
        teams.insert("team1".to_string(), team);

        let mut blueprint = ShipBlueprint::new(
            "Test Ship".to_string(),
            "test_cruiser".to_string(),
            "team1".to_string(),
        );
        blueprint.set_player_roles("player1".to_string(), vec![ShipRole::Captain]);

        let validator = BlueprintValidator::new(&config, &players, &teams);
        let result = validator.validate(&blueprint);

        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::PlayersNotReady))
        );
    }

    #[test]
    fn test_validate_weight_limit() {
        let config = create_test_config();

        let mut players = HashMap::new();
        players.insert("player1".to_string(), create_test_player("player1"));

        let mut teams = HashMap::new();
        let mut team = create_test_team("team1");
        team.members.push("player1".to_string());
        teams.insert("team1".to_string(), team);

        let mut blueprint = ShipBlueprint::new(
            "Test Ship".to_string(),
            "test_cruiser".to_string(),
            "team1".to_string(),
        );
        blueprint.set_player_roles("player1".to_string(), vec![ShipRole::Captain]);
        blueprint.mark_ready("player1".to_string());

        // Add many modules to exceed weight limit (each module = 100kg placeholder)
        for i in 0..15 {
            blueprint
                .modules
                .push(crate::models::blueprint::ModuleInstance {
                    id: format!("module_{}", i),
                    module_slot_id: "test_module_slot".to_string(),
                    variant_id: None,
                });
        }

        let validator = BlueprintValidator::new(&config, &players, &teams);
        let result = validator.validate(&blueprint);

        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::WeightLimitExceeded { .. }))
        );
    }

    #[test]
    fn test_validate_module_count_limit() {
        let config = create_test_config();

        let mut players = HashMap::new();
        players.insert("player1".to_string(), create_test_player("player1"));

        let mut teams = HashMap::new();
        let mut team = create_test_team("team1");
        team.members.push("player1".to_string());
        teams.insert("team1".to_string(), team);

        let mut blueprint = ShipBlueprint::new(
            "Test Ship".to_string(),
            "test_cruiser".to_string(),
            "team1".to_string(),
        );
        blueprint.set_player_roles("player1".to_string(), vec![ShipRole::Captain]);
        blueprint.mark_ready("player1".to_string());

        // Add more modules than allowed (max_modules = 10)
        for i in 0..12 {
            blueprint
                .modules
                .push(crate::models::blueprint::ModuleInstance {
                    id: format!("module_{}", i),
                    module_slot_id: "test_module_slot".to_string(),
                    variant_id: None,
                });
        }

        let validator = BlueprintValidator::new(&config, &players, &teams);
        let result = validator.validate(&blueprint);

        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::ModuleCountExceeded { .. }))
        );
    }

    #[test]
    fn test_validate_valid_blueprint() {
        let config = create_test_config();

        let mut players = HashMap::new();
        players.insert("player1".to_string(), create_test_player("player1"));

        let mut teams = HashMap::new();
        let mut team = create_test_team("team1");
        team.members.push("player1".to_string());
        teams.insert("team1".to_string(), team);

        let mut blueprint = ShipBlueprint::new(
            "Test Ship".to_string(),
            "test_cruiser".to_string(),
            "team1".to_string(),
        );
        blueprint.set_player_roles(
            "player1".to_string(),
            vec![ShipRole::Captain, ShipRole::Helm],
        );
        blueprint.mark_ready("player1".to_string());

        // Add a few modules (within limits)
        for i in 0..5 {
            blueprint
                .modules
                .push(crate::models::blueprint::ModuleInstance {
                    id: format!("module_{}", i),
                    module_slot_id: "test_module_slot".to_string(),
                    variant_id: Some("test_variant".to_string()),
                });
        }

        let validator = BlueprintValidator::new(&config, &players, &teams);
        let result = validator.validate(&blueprint);

        assert!(result.is_valid);
        assert_eq!(result.errors.len(), 0);
    }
}
