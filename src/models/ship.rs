//! Active ship models
//!
//! Defines structures for ships active in the simulation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::role::ShipRole;
use super::blueprint::{ModuleInstance, WeaponInstance};
use super::status::{ShipStatus, Inventory};
use crate::config::ModuleStats;

/// Compiled module with resolved stats and runtime state
///
/// Represents a module instance after blueprint compilation, with all stats
/// resolved from variant configuration and runtime state tracking enabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledModule {
    /// Unique instance identifier
    pub instance_id: String,
    /// Module type ID (e.g., "shield-generators", "warp-cores")
    pub module_id: String,
    /// Variant ID (e.g., "light-shield-mk1", "basic-warp-drive")
    pub kind: Option<String>,
    /// Display name (from variant or base module)
    pub name: String,
    /// Module statistics from variant configuration
    pub stats: ModuleStats,
    /// Current health (0.0 to max_health)
    pub current_health: f32,
    /// Maximum health
    pub max_health: f32,
    /// Whether the module is operational
    pub operational: bool,
    /// Power currently allocated to this module (0.0 to 1.0)
    pub power_allocated: f32,
    /// Cooling currently allocated to this module (0.0 to 1.0)
    pub cooling_allocated: f32,
}

impl CompiledModule {
    /// Get a stat value as f64
    pub fn get_stat_f64(&self, key: &str) -> Option<f64> {
        self.stats.get_f64(key)
    }
    
    /// Check if module is damaged (health below max)
    pub fn is_damaged(&self) -> bool {
        self.current_health < self.max_health
    }
    
    /// Check if module is destroyed (health at or below 0)
    pub fn is_destroyed(&self) -> bool {
        self.current_health <= 0.0
    }
    
    /// Get current efficiency based on health and power allocation
    pub fn get_efficiency(&self) -> f32 {
        if !self.operational || self.is_destroyed() {
            return 0.0;
        }
        
        // Efficiency scales with health and power allocation
        let health_factor = self.current_health / self.max_health;
        health_factor * self.power_allocated
    }
}

/// Represents an active ship in the simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ship {
    /// Unique ship identifier
    pub id: String,
    /// Ship name
    pub name: String,
    /// Ship class
    pub class: String,
    /// Team ID
    pub team_id: String,
    /// Player role assignments
    pub player_roles: HashMap<String, Vec<ShipRole>>,
    /// Current ship status
    pub status: ShipStatus,
    /// Compiled modules with resolved stats
    pub modules: Vec<CompiledModule>,
    /// Equipped weapons
    pub weapons: Vec<WeaponInstance>,
    /// Ship inventory
    pub inventory: Inventory,
}

/// A captain's log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptainLogEntry {
    /// Unique entry ID
    pub id: String,
    /// Ship ID
    pub ship_id: String,
    /// Stardate when entry was created
    pub stardate: f64,
    /// Entry content
    pub entry: String,
    /// Timestamp (Unix epoch)
    pub timestamp: i64,
}

/// A docking request from a ship to a station
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockingRequest {
    /// Unique request ID
    pub id: String,
    /// Ship requesting docking
    pub ship_id: String,
    /// Target station ID
    pub station_id: String,
    /// Timestamp of request
    pub timestamp: i64,
    /// Request status
    pub status: DockingStatus,
}

/// Status of a docking request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DockingStatus {
    Pending,
    Approved,
    Denied,
    Docked,
}

/// A hail message between ships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HailMessage {
    /// Unique message ID
    pub id: String,
    /// Ship sending the hail
    pub from_ship_id: String,
    /// Ship receiving the hail
    pub to_ship_id: String,
    /// Message content
    pub message: String,
    /// Timestamp
    pub timestamp: i64,
    /// Whether this is a response to another hail
    pub in_response_to: Option<String>,
}

/// A command to fighters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FighterCommand {
    /// Unique command ID
    pub id: String,
    /// Ship issuing the command
    pub ship_id: String,
    /// Fighter IDs to command
    pub fighter_ids: Vec<String>,
    /// Command type
    pub command: FighterCommandType,
    /// Timestamp
    pub timestamp: i64,
}

/// Type of fighter command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FighterCommandType {
    Launch,
    Recall,
    Attack { target_id: String },
    Defend { protect_id: String },
    Patrol { waypoints: Vec<(f64, f64, f64)> },
}
