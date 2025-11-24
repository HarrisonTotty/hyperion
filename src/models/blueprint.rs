//! Ship blueprint models
//!
//! Defines structures for ships in the design phase.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use super::role::ShipRole;

/// Represents a ship in the design phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipBlueprint {
    /// Unique blueprint identifier
    pub id: String,
    /// Ship name
    pub name: String,
    /// Ship class (from configuration)
    pub class: String,
    /// Team ID that owns this ship
    pub team_id: String,
    /// Map of player IDs to their requested roles
    pub player_roles: HashMap<String, Vec<ShipRole>>,
    /// Modules equipped on the ship
    pub modules: Vec<ModuleInstance>,
    /// Weapons equipped on the ship
    pub weapons: Vec<WeaponInstance>,
    /// Players who have marked themselves as ready
    pub ready_players: HashSet<String>,
}

/// Represents an instance of a module on a ship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInstance {
    /// Unique instance identifier
    pub id: String,
    /// Module slot type ID
    pub module_slot_id: String,
    /// Selected variant ID (None if no variant or not yet selected)
    pub variant_id: Option<String>,
}

/// Represents an instance of a weapon on a ship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponInstance {
    /// Unique instance identifier
    pub id: String,
    /// Weapon definition ID (from configuration)
    pub weapon_id: String,
    /// Optional "kind" for kinetic weapons (railgun, cannon, etc.)
    pub kind: Option<String>,
    /// Currently loaded ammunition (if applicable)
    pub loaded_ammunition: Option<String>,
}

impl ShipBlueprint {
    /// Create a new ship blueprint
    pub fn new(name: String, class: String, team_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            class,
            team_id,
            player_roles: HashMap::new(),
            modules: Vec::new(),
            weapons: Vec::new(),
            ready_players: HashSet::new(),
        }
    }
    
    /// Add or update player roles
    pub fn set_player_roles(&mut self, player_id: String, roles: Vec<ShipRole>) {
        self.player_roles.insert(player_id, roles);
    }
    
    /// Mark a player as ready
    pub fn mark_ready(&mut self, player_id: String) {
        self.ready_players.insert(player_id);
    }
    
    /// Unmark a player as ready
    pub fn unmark_ready(&mut self, player_id: &str) {
        self.ready_players.remove(player_id);
    }
    
    /// Check if all players are ready
    pub fn all_players_ready(&self) -> bool {
        if self.player_roles.is_empty() {
            return false;
        }
        self.player_roles.keys().all(|pid| self.ready_players.contains(pid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ship_blueprint_creation() {
        let mut blueprint = ShipBlueprint::new(
            "USS Enterprise".to_string(),
            "cruiser".to_string(),
            "team1".to_string(),
        );
        
        assert_eq!(blueprint.name, "USS Enterprise");
        assert_eq!(blueprint.class, "cruiser");
        assert!(blueprint.modules.is_empty());
        assert!(blueprint.weapons.is_empty());
        
        // Set player roles
        blueprint.set_player_roles("player1".to_string(), vec![ShipRole::Captain, ShipRole::Helm]);
        blueprint.set_player_roles("player2".to_string(), vec![ShipRole::Engineering]);
        assert_eq!(blueprint.player_roles.len(), 2);
        
        // Ready status
        assert!(!blueprint.all_players_ready());
        blueprint.mark_ready("player1".to_string());
        assert!(!blueprint.all_players_ready());
        blueprint.mark_ready("player2".to_string());
        assert!(blueprint.all_players_ready());
        
        // Unmark ready
        blueprint.unmark_ready("player1");
        assert!(!blueprint.all_players_ready());
    }

    #[test]
    fn test_module_instance() {
        let module = ModuleInstance {
            id: "module_1".to_string(),
            module_slot_id: "impulse_engine".to_string(),
            variant_id: Some("fusion".to_string()),
        };
        assert_eq!(module.variant_id, Some("fusion".to_string()));
    }
    
    #[test]
    fn test_weapon_instance() {
        let weapon = WeaponInstance {
            id: "weapon_1".to_string(),
            weapon_id: "kinetic_cannon".to_string(),
            kind: Some("railgun".to_string()),
            loaded_ammunition: Some("armor_piercing".to_string()),
        };
        assert_eq!(weapon.kind, Some("railgun".to_string()));
        assert_eq!(weapon.loaded_ammunition, Some("armor_piercing".to_string()));
    }
}
