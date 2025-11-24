//! Ship status and inventory models
//!
//! Defines structures for tracking ship status, effects, and inventory.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the current status of a ship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipStatus {
    /// Current hull integrity (0.0 to max)
    pub hull: f32,
    /// Maximum hull integrity
    pub max_hull: f32,
    /// Current shield strength (0.0 to max)
    pub shields: f32,
    /// Maximum shield strength
    pub max_shields: f32,
    /// Whether shields are raised
    pub shields_raised: bool,
    /// Current power generation
    pub power_generation: f32,
    /// Current power capacity
    pub power_capacity: f32,
    /// Current power usage
    pub power_usage: f32,
    /// Current cooling capacity
    pub cooling_capacity: f32,
    /// Current heat generation
    pub heat_generation: f32,
    /// Base weight of the ship in kg
    pub base_weight: f32,
    /// Effective weight (including status effects like Graviton)
    pub effective_weight: f32,
    /// Active status effects on the ship
    pub status_effects: Vec<StatusEffect>,
    /// Module health tracking (module_id -> health percentage)
    pub module_health: HashMap<String, f32>,
}

/// Represents a temporary status effect on a ship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffect {
    /// Type of status effect
    pub effect_type: StatusEffectType,
    /// Remaining duration in seconds
    pub duration: f32,
    /// Strength/magnitude of the effect
    pub magnitude: f32,
}

/// Types of status effects that can affect ships
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatusEffectType {
    /// Ion effect - jams communications and science, disables targeting
    Ion,
    /// Graviton effect - increases effective weight by 30%
    Graviton,
    /// Tachyon effect - disables warp and jump drives
    Tachyon,
}

/// Represents a ship's inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    /// Ammunition storage (ammo_type -> quantity)
    pub ammunition: HashMap<String, u32>,
    /// Cargo storage (item_id -> quantity)
    pub cargo: HashMap<String, u32>,
}

impl Default for ShipStatus {
    fn default() -> Self {
        Self::new(1000.0, 500.0, 1000.0)
    }
}

impl ShipStatus {
    /// Create a new ship status with default values
    pub fn new(max_hull: f32, max_shields: f32, base_weight: f32) -> Self {
        Self {
            hull: max_hull,
            max_hull,
            shields: max_shields,
            max_shields,
            shields_raised: false,
            power_generation: 0.0,
            power_capacity: 0.0,
            power_usage: 0.0,
            cooling_capacity: 0.0,
            heat_generation: 0.0,
            base_weight,
            effective_weight: base_weight,
            status_effects: Vec::new(),
            module_health: HashMap::new(),
        }
    }
    
    /// Update effective weight based on status effects
    pub fn update_effective_weight(&mut self) {
        self.effective_weight = self.base_weight;
        
        // Check for Graviton effect (30% additional weight, non-stacking)
        if self.status_effects.iter().any(|e| e.effect_type == StatusEffectType::Graviton) {
            self.effective_weight *= 1.3;
        }
    }
    
    /// Check if ship is affected by Ion (communications/science jammed)
    pub fn is_ion_jammed(&self) -> bool {
        self.status_effects.iter().any(|e| e.effect_type == StatusEffectType::Ion)
    }
    
    /// Check if ship can use FTL drives (not affected by Tachyon)
    pub fn can_use_ftl(&self) -> bool {
        !self.status_effects.iter().any(|e| e.effect_type == StatusEffectType::Tachyon)
    }
    
    /// Check if ship is affected by Tachyon (FTL drives disabled)
    pub fn is_tachyon_disabled(&self) -> bool {
        self.status_effects.iter().any(|e| e.effect_type == StatusEffectType::Tachyon)
    }
}

impl Inventory {
    /// Create a new empty inventory
    pub fn new() -> Self {
        Self {
            ammunition: HashMap::new(),
            cargo: HashMap::new(),
        }
    }
    
    /// Add ammunition to inventory
    pub fn add_ammunition(&mut self, ammo_type: String, quantity: u32) {
        *self.ammunition.entry(ammo_type).or_insert(0) += quantity;
    }
    
    /// Remove ammunition from inventory
    pub fn remove_ammunition(&mut self, ammo_type: &str, quantity: u32) -> Result<(), String> {
        let current = self.ammunition.get(ammo_type).copied().unwrap_or(0);
        if current < quantity {
            return Err(format!("Insufficient ammunition: {} (have {}, need {})", ammo_type, current, quantity));
        }
        *self.ammunition.get_mut(ammo_type).unwrap() -= quantity;
        Ok(())
    }
    
    /// Check if ammunition is available
    pub fn has_ammunition(&self, ammo_type: &str, quantity: u32) -> bool {
        self.ammunition.get(ammo_type).copied().unwrap_or(0) >= quantity
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ship_status_effective_weight() {
        let mut status = ShipStatus::new(1000.0, 500.0, 10000.0);
        assert_eq!(status.effective_weight, 10000.0);
        
        // Add Graviton effect
        status.status_effects.push(StatusEffect {
            effect_type: StatusEffectType::Graviton,
            duration: 5.0,
            magnitude: 0.3,
        });
        status.update_effective_weight();
        assert_eq!(status.effective_weight, 13000.0); // 30% increase
    }

    #[test]
    fn test_ship_status_effects() {
        let mut status = ShipStatus::new(1000.0, 500.0, 10000.0);
        
        // Ion effect
        assert!(!status.is_ion_jammed());
        status.status_effects.push(StatusEffect {
            effect_type: StatusEffectType::Ion,
            duration: 3.0,
            magnitude: 1.0,
        });
        assert!(status.is_ion_jammed());
        
        // Tachyon effect
        assert!(status.can_use_ftl());
        status.status_effects.push(StatusEffect {
            effect_type: StatusEffectType::Tachyon,
            duration: 10.0,
            magnitude: 1.0,
        });
        assert!(!status.can_use_ftl());
    }

    #[test]
    fn test_inventory_ammunition() {
        let mut inventory = Inventory::new();
        
        // Add ammunition
        inventory.add_ammunition("missiles".to_string(), 10);
        inventory.add_ammunition("missiles".to_string(), 5);
        assert_eq!(inventory.ammunition.get("missiles"), Some(&15));
        
        // Check availability
        assert!(inventory.has_ammunition("missiles", 15));
        assert!(!inventory.has_ammunition("missiles", 16));
        assert!(!inventory.has_ammunition("torpedos", 1));
        
        // Remove ammunition
        assert!(inventory.remove_ammunition("missiles", 5).is_ok());
        assert_eq!(inventory.ammunition.get("missiles"), Some(&10));
        
        // Try to remove more than available
        assert!(inventory.remove_ammunition("missiles", 11).is_err());
        assert_eq!(inventory.ammunition.get("missiles"), Some(&10));
    }
}
