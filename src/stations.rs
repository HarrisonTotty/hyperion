//! Station system for docking, services, and trade
//!
//! This module provides the station entity type and related functionality
//! for ships to dock at stations, receive repairs, refuel, rearm, and trade.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Docking status for a ship at a station
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockingStatus {
    /// Ship has requested docking permission
    Requested,
    /// Docking request approved, ship approaching
    Approaching,
    /// Ship is docked at the station
    Docked,
    /// Ship is undocking
    Undocking,
    /// Docking request denied
    Denied,
}

/// Services available at a station
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationServices {
    /// Can repair damaged modules
    pub repair: bool,
    /// Can refuel ships (restore power)
    pub refuel: bool,
    /// Can rearm weapons (replenish ammunition)
    pub rearm: bool,
    /// Has a trade market
    pub trade: bool,
    /// Repair cost per health point
    pub repair_cost: f32,
    /// Refuel cost per unit
    pub refuel_cost: f32,
    /// Rearm cost multiplier
    pub rearm_cost: f32,
}

impl Default for StationServices {
    fn default() -> Self {
        Self {
            repair: true,
            refuel: true,
            rearm: true,
            trade: true,
            repair_cost: 10.0,
            refuel_cost: 5.0,
            rearm_cost: 1.5,
        }
    }
}

/// A space station entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    /// Unique identifier
    pub id: Uuid,
    /// Station name
    pub name: String,
    /// Position in 3D space [x, y, z]
    pub position: [f64; 3],
    /// Owning faction
    pub faction: String,
    /// Available services
    pub services: StationServices,
    /// Maximum number of docked ships
    pub max_docked_ships: usize,
    /// Currently docked ship IDs
    pub docked_ships: Vec<Uuid>,
    /// Pending docking requests (ship_id, status)
    pub docking_requests: Vec<(Uuid, DockingStatus)>,
    /// Station is hostile to certain factions
    pub hostile_factions: Vec<String>,
    /// Station size (affects docking bay capacity)
    pub size: StationSize,
}

/// Station size classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationSize {
    Small,
    Medium,
    Large,
    Massive,
}

impl Station {
    /// Create a new station
    pub fn new(name: String, position: [f64; 3], faction: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            position,
            faction,
            services: StationServices::default(),
            max_docked_ships: 5,
            docked_ships: Vec::new(),
            docking_requests: Vec::new(),
            hostile_factions: Vec::new(),
            size: StationSize::Medium,
        }
    }
    
    /// Create a new station with custom size
    pub fn with_size(name: String, position: [f64; 3], faction: String, size: StationSize) -> Self {
        let max_docked = match size {
            StationSize::Small => 2,
            StationSize::Medium => 5,
            StationSize::Large => 10,
            StationSize::Massive => 20,
        };
        
        Self {
            id: Uuid::new_v4(),
            name,
            position,
            faction,
            services: StationServices::default(),
            max_docked_ships: max_docked,
            docked_ships: Vec::new(),
            docking_requests: Vec::new(),
            hostile_factions: Vec::new(),
            size,
        }
    }
    
    /// Request docking permission
    ///
    /// Returns true if request is approved, false if denied
    pub fn request_docking(&mut self, ship_id: Uuid, ship_faction: &str) -> bool {
        // Check if already docked or has pending request
        if self.docked_ships.contains(&ship_id) {
            return false;
        }
        
        if self.docking_requests.iter().any(|(id, _)| *id == ship_id) {
            return false;
        }
        
        // Check if hostile
        if self.hostile_factions.contains(&ship_faction.to_string()) {
            self.docking_requests.push((ship_id, DockingStatus::Denied));
            return false;
        }
        
        // Check if station is full
        if self.docked_ships.len() >= self.max_docked_ships {
            self.docking_requests.push((ship_id, DockingStatus::Denied));
            return false;
        }
        
        // Approve docking
        self.docking_requests.push((ship_id, DockingStatus::Requested));
        true
    }
    
    /// Approve a docking request
    pub fn approve_docking(&mut self, ship_id: Uuid) -> bool {
        if let Some((_, status)) = self.docking_requests.iter_mut().find(|(id, _)| *id == ship_id) {
            *status = DockingStatus::Approaching;
            true
        } else {
            false
        }
    }
    
    /// Complete docking (ship has arrived)
    pub fn complete_docking(&mut self, ship_id: Uuid) -> bool {
        if let Some(index) = self.docking_requests.iter().position(|(id, status)| {
            *id == ship_id && *status == DockingStatus::Approaching
        }) {
            self.docking_requests.remove(index);
            self.docked_ships.push(ship_id);
            true
        } else {
            false
        }
    }
    
    /// Undock a ship
    pub fn undock_ship(&mut self, ship_id: Uuid) -> bool {
        if let Some(index) = self.docked_ships.iter().position(|id| *id == ship_id) {
            self.docked_ships.remove(index);
            self.docking_requests.push((ship_id, DockingStatus::Undocking));
            true
        } else {
            false
        }
    }
    
    /// Complete undocking (ship has departed)
    pub fn complete_undocking(&mut self, ship_id: Uuid) -> bool {
        if let Some(index) = self.docking_requests.iter().position(|(id, status)| {
            *id == ship_id && *status == DockingStatus::Undocking
        }) {
            self.docking_requests.remove(index);
            true
        } else {
            false
        }
    }
    
    /// Check if a ship is docked
    pub fn is_ship_docked(&self, ship_id: Uuid) -> bool {
        self.docked_ships.contains(&ship_id)
    }
    
    /// Get docking status for a ship
    pub fn get_docking_status(&self, ship_id: Uuid) -> Option<DockingStatus> {
        if self.docked_ships.contains(&ship_id) {
            return Some(DockingStatus::Docked);
        }
        
        self.docking_requests
            .iter()
            .find(|(id, _)| *id == ship_id)
            .map(|(_, status)| *status)
    }
    
    /// Get available docking bays
    pub fn available_docking_bays(&self) -> usize {
        self.max_docked_ships.saturating_sub(self.docked_ships.len())
    }
    
    /// Add a hostile faction
    pub fn add_hostile_faction(&mut self, faction: String) {
        if !self.hostile_factions.contains(&faction) {
            self.hostile_factions.push(faction);
        }
    }
    
    /// Remove a hostile faction
    pub fn remove_hostile_faction(&mut self, faction: &str) {
        self.hostile_factions.retain(|f| f != faction);
    }
    
    /// Check if a faction is hostile
    pub fn is_hostile_to(&self, faction: &str) -> bool {
        self.hostile_factions.contains(&faction.to_string())
    }
}

/// Service request for a docked ship
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServiceRequest {
    /// Repair a specific module
    RepairModule {
        module_id: String,
    },
    /// Repair all damaged modules
    RepairAll,
    /// Refuel the ship
    Refuel {
        amount: f32,
    },
    /// Rearm a specific weapon
    RearmWeapon {
        weapon_id: String,
        ammunition_type: String,
        quantity: u32,
    },
    /// Rearm all weapons
    RearmAll,
}

/// Service response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceResponse {
    pub success: bool,
    pub message: String,
    pub cost: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_station_creation() {
        let station = Station::new(
            "Alpha Station".to_string(),
            [100.0, 200.0, 300.0],
            "Federation".to_string(),
        );
        
        assert_eq!(station.name, "Alpha Station");
        assert_eq!(station.position, [100.0, 200.0, 300.0]);
        assert_eq!(station.faction, "Federation");
        assert_eq!(station.max_docked_ships, 5);
        assert_eq!(station.docked_ships.len(), 0);
    }
    
    #[test]
    fn test_station_with_size() {
        let small = Station::with_size(
            "Small Station".to_string(),
            [0.0, 0.0, 0.0],
            "Federation".to_string(),
            StationSize::Small,
        );
        assert_eq!(small.max_docked_ships, 2);
        
        let massive = Station::with_size(
            "Massive Station".to_string(),
            [0.0, 0.0, 0.0],
            "Federation".to_string(),
            StationSize::Massive,
        );
        assert_eq!(massive.max_docked_ships, 20);
    }
    
    #[test]
    fn test_docking_request() {
        let mut station = Station::new(
            "Test Station".to_string(),
            [0.0, 0.0, 0.0],
            "Federation".to_string(),
        );
        
        let ship_id = Uuid::new_v4();
        
        // Request docking
        let approved = station.request_docking(ship_id, "Federation");
        assert!(approved);
        assert_eq!(station.get_docking_status(ship_id), Some(DockingStatus::Requested));
        
        // Approve docking
        assert!(station.approve_docking(ship_id));
        assert_eq!(station.get_docking_status(ship_id), Some(DockingStatus::Approaching));
        
        // Complete docking
        assert!(station.complete_docking(ship_id));
        assert_eq!(station.get_docking_status(ship_id), Some(DockingStatus::Docked));
        assert!(station.is_ship_docked(ship_id));
    }
    
    #[test]
    fn test_hostile_faction_docking() {
        let mut station = Station::new(
            "Test Station".to_string(),
            [0.0, 0.0, 0.0],
            "Federation".to_string(),
        );
        
        station.add_hostile_faction("Empire".to_string());
        
        let ship_id = Uuid::new_v4();
        let approved = station.request_docking(ship_id, "Empire");
        
        assert!(!approved);
        assert_eq!(station.get_docking_status(ship_id), Some(DockingStatus::Denied));
    }
    
    #[test]
    fn test_station_capacity() {
        let mut station = Station::with_size(
            "Small Station".to_string(),
            [0.0, 0.0, 0.0],
            "Federation".to_string(),
            StationSize::Small,
        );
        
        assert_eq!(station.available_docking_bays(), 2);
        
        // Dock two ships
        let ship1 = Uuid::new_v4();
        let ship2 = Uuid::new_v4();
        
        station.request_docking(ship1, "Federation");
        station.approve_docking(ship1);
        station.complete_docking(ship1);
        
        station.request_docking(ship2, "Federation");
        station.approve_docking(ship2);
        station.complete_docking(ship2);
        
        assert_eq!(station.available_docking_bays(), 0);
        
        // Try to dock a third ship - should be denied
        let ship3 = Uuid::new_v4();
        let approved = station.request_docking(ship3, "Federation");
        assert!(!approved);
        assert_eq!(station.get_docking_status(ship3), Some(DockingStatus::Denied));
    }
    
    #[test]
    fn test_undocking() {
        let mut station = Station::new(
            "Test Station".to_string(),
            [0.0, 0.0, 0.0],
            "Federation".to_string(),
        );
        
        let ship_id = Uuid::new_v4();
        
        // Dock the ship
        station.request_docking(ship_id, "Federation");
        station.approve_docking(ship_id);
        station.complete_docking(ship_id);
        assert!(station.is_ship_docked(ship_id));
        
        // Undock
        assert!(station.undock_ship(ship_id));
        assert_eq!(station.get_docking_status(ship_id), Some(DockingStatus::Undocking));
        assert!(!station.is_ship_docked(ship_id));
        
        // Complete undocking
        assert!(station.complete_undocking(ship_id));
        assert_eq!(station.get_docking_status(ship_id), None);
    }
    
    #[test]
    fn test_hostile_faction_management() {
        let mut station = Station::new(
            "Test Station".to_string(),
            [0.0, 0.0, 0.0],
            "Federation".to_string(),
        );
        
        assert!(!station.is_hostile_to("Empire"));
        
        station.add_hostile_faction("Empire".to_string());
        assert!(station.is_hostile_to("Empire"));
        
        station.remove_hostile_faction("Empire");
        assert!(!station.is_hostile_to("Empire"));
    }
    
    #[test]
    fn test_station_services() {
        let services = StationServices::default();
        assert!(services.repair);
        assert!(services.refuel);
        assert!(services.rearm);
        assert!(services.trade);
        assert!(services.repair_cost > 0.0);
        assert!(services.refuel_cost > 0.0);
        assert!(services.rearm_cost > 0.0);
    }
}
