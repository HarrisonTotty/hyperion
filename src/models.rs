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

pub mod blueprint;
pub mod player;
pub mod role;
pub mod ship;
pub mod status;
pub mod weapon;

// Re-export commonly used types for convenience
pub use blueprint::{ModuleInstance, ShipBlueprint, WeaponInstance};
pub use player::{Player, Team};
pub use role::ShipRole;
pub use ship::{CompiledModule, Ship};
pub use status::{Inventory, ShipStatus, StatusEffect, StatusEffectType};
pub use weapon::{Weapon, WeaponFireMode, WeaponTag};
