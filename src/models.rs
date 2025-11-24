//! Models module
//!
//! Defines game data models and structures.
//!
//! This module is organized into focused submodules:
//! - `player` - Player and team management
//! - `role` - Ship crew roles
//! - `blueprint` - Ship design phase structures
//! - `ship` - Active ship structures
//! - `status` - Ship status, effects, and inventory
//! - `weapon` - Weapon tags and definitions

pub mod player;
pub mod role;
pub mod blueprint;
pub mod ship;
pub mod status;
pub mod weapon;

// Re-export commonly used types for convenience
pub use player::{Player, Team};
pub use role::ShipRole;
pub use blueprint::{ShipBlueprint, ModuleInstance, WeaponInstance};
pub use ship::{Ship, CompiledModule};
pub use status::{ShipStatus, StatusEffect, StatusEffectType, Inventory};
pub use weapon::{WeaponTag, WeaponFireMode, Weapon};

// Legacy export for backward compatibility
/// Represents a ship module slot (legacy, use ModuleInstance)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModuleSlot {
    pub slot_type: String,
    pub module_id: Option<String>,
}
