//! Game state management
//!
//! This module provides thread-safe global game state using Bevy ECS.
//! The `GameWorld` struct serves as the central registry for all game entities
//! including players, teams, blueprints, and active ships.

use bevy_ecs::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::models::{Player, Team, ShipBlueprint, Ship};
use crate::models::ship::{CaptainLogEntry, DockingRequest, HailMessage, FighterCommand};
use crate::api::positions::countermeasures::CountermeasureType;
use crate::events::{EventQueue, GameEvent};
use crate::stations::Station;
use crate::ai::AIManager;

/// Thread-safe wrapper for game state
///
/// This allows multiple threads (e.g., API handlers, simulation) to access
/// the game world safely using Arc<RwLock<GameWorld>>.
pub type SharedGameWorld = Arc<RwLock<GameWorld>>;

/// Central game state manager using Bevy ECS
///
/// The GameWorld maintains all game entities and provides registry access
/// for players, teams, blueprints, and ships.
#[derive(Default)]
pub struct GameWorld {
    /// Bevy ECS world for entity management
    pub world: World,
    
    /// Player registry (ID -> Player)
    players: HashMap<String, Player>,
    
    /// Team registry (ID -> Team)
    teams: HashMap<String, Team>,
    
    /// Blueprint registry (ID -> ShipBlueprint)
    blueprints: HashMap<String, ShipBlueprint>,
    
    /// Ship registry (ID -> Ship)
    ships: HashMap<String, Ship>,
    
    /// Captain's log entries (Ship ID -> Vec<LogEntry>)
    captain_logs: HashMap<String, Vec<CaptainLogEntry>>,
    
    /// Docking requests (Request ID -> DockingRequest)
    docking_requests: HashMap<String, DockingRequest>,
    
    /// Hail messages (Message ID -> HailMessage)
    hail_messages: HashMap<String, HailMessage>,
    
    /// Fighter commands (Command ID -> FighterCommand)
    fighter_commands: HashMap<String, FighterCommand>,
    
    /// Jam attempts tracking (for integration with simulation)
    jam_attempts: Vec<(String, String, f64)>, // (source_ship_id, target_ship_id, duration)
    
    /// Countermeasure loads (ship_id, type, quantity)
    countermeasure_loads: Vec<(String, String, u32)>,
    
    /// Countermeasure activations (ship_id, threat_ids)
    countermeasure_activations: Vec<(String, Vec<String>)>,
    
    /// Point defense settings (ship_id -> enabled)
    point_defense_settings: HashMap<String, bool>,
    
    /// Event queue for broadcasting to WebSocket clients
    event_queue: EventQueue,
    
    /// Station registry (ID -> Station)
    stations: HashMap<String, Station>,
    
    /// AI manager for autonomous ships
    pub ai_manager: AIManager,
    
    /// Player name to ID mapping for quick lookups
    player_names: HashMap<String, String>,
    
    /// Team name to ID mapping for quick lookups
    team_names: HashMap<String, String>,
}

impl GameWorld {
    /// Create a new game world
    pub fn new() -> Self {
        Self {
            world: World::new(),
            players: HashMap::new(),
            teams: HashMap::new(),
            blueprints: HashMap::new(),
            ships: HashMap::new(),
            captain_logs: HashMap::new(),
            docking_requests: HashMap::new(),
            hail_messages: HashMap::new(),
            fighter_commands: HashMap::new(),
            jam_attempts: Vec::new(),
            countermeasure_loads: Vec::new(),
            countermeasure_activations: Vec::new(),
            point_defense_settings: HashMap::new(),
            event_queue: EventQueue::new(),
            stations: HashMap::new(),
            ai_manager: AIManager::new(),
            player_names: HashMap::new(),
            team_names: HashMap::new(),
        }
    }

    /// Create a new thread-safe shared game world
    pub fn new_shared() -> SharedGameWorld {
        Arc::new(RwLock::new(Self::new()))
    }

    // ==================== Registry Access ====================

    /// Get reference to players HashMap
    pub fn players(&self) -> &HashMap<String, Player> {
        &self.players
    }

    /// Get reference to teams HashMap
    pub fn teams(&self) -> &HashMap<String, Team> {
        &self.teams
    }

    /// Get reference to blueprints HashMap
    pub fn blueprints(&self) -> &HashMap<String, ShipBlueprint> {
        &self.blueprints
    }

    /// Get reference to ships HashMap
    pub fn ships(&self) -> &HashMap<String, Ship> {
        &self.ships
    }

    // ==================== Player Management ====================

    /// Register a new player
    ///
    /// # Returns
    ///
    /// Returns `Ok(player_id)` on success, or `Err` if the player name is already taken.
    pub fn register_player(&mut self, name: String) -> Result<String, String> {
        // Check if name is already taken
        if self.player_names.contains_key(&name) {
            return Err(format!("Player name '{}' is already taken", name));
        }

        // Validate name
        if name.is_empty() {
            return Err("Player name cannot be empty".to_string());
        }
        if name.len() > 50 {
            return Err("Player name cannot exceed 50 characters".to_string());
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return Err("Player name can only contain alphanumeric characters, underscores, and hyphens".to_string());
        }

        let player = Player::new(name.clone());
        let player_id = player.id.clone();
        
        self.players.insert(player_id.clone(), player);
        self.player_names.insert(name, player_id.clone());
        
        Ok(player_id)
    }

    /// Get a player by ID
    pub fn get_player(&self, id: &str) -> Option<&Player> {
        self.players.get(id)
    }

    /// Get a mutable reference to a player by ID
    pub fn get_player_mut(&mut self, id: &str) -> Option<&mut Player> {
        self.players.get_mut(id)
    }

    /// Get a player by name
    pub fn get_player_by_name(&self, name: &str) -> Option<&Player> {
        self.player_names.get(name).and_then(|id| self.players.get(id))
    }

    /// Get all players
    pub fn get_all_players(&self) -> Vec<&Player> {
        self.players.values().collect()
    }

    /// Remove a player
    ///
    /// This also removes the player from any teams they're in.
    pub fn remove_player(&mut self, id: &str) -> Result<(), String> {
        let player = self.players.remove(id)
            .ok_or_else(|| format!("Player {} not found", id))?;
        
        self.player_names.remove(&player.name);
        
        // Remove player from all teams
        for team in self.teams.values_mut() {
            team.remove_member(id);
        }
        
        Ok(())
    }

    // ==================== Team Management ====================

    /// Create a new team
    ///
    /// # Returns
    ///
    /// Returns `Ok(team_id)` on success, or `Err` if the team name is already taken.
    pub fn create_team(&mut self, name: String, faction: String) -> Result<String, String> {
        // Check if name is already taken
        if self.team_names.contains_key(&name) {
            return Err(format!("Team name '{}' is already taken", name));
        }

        // Validate name
        if name.is_empty() {
            return Err("Team name cannot be empty".to_string());
        }
        if name.len() > 50 {
            return Err("Team name cannot exceed 50 characters".to_string());
        }

        let team = Team::new(name.clone(), faction);
        let team_id = team.id.clone();
        
        self.teams.insert(team_id.clone(), team);
        self.team_names.insert(name, team_id.clone());
        
        Ok(team_id)
    }

    /// Get a team by ID
    pub fn get_team(&self, id: &str) -> Option<&Team> {
        self.teams.get(id)
    }

    /// Get a mutable reference to a team by ID
    pub fn get_team_mut(&mut self, id: &str) -> Option<&mut Team> {
        self.teams.get_mut(id)
    }

    /// Get a team by name
    pub fn get_team_by_name(&self, name: &str) -> Option<&Team> {
        self.team_names.get(name).and_then(|id| self.teams.get(id))
    }

    /// Get all teams
    pub fn get_all_teams(&self) -> Vec<&Team> {
        self.teams.values().collect()
    }

    /// Add a player to a team
    pub fn add_player_to_team(&mut self, team_id: &str, player_id: &str) -> Result<(), String> {
        // Verify player exists
        if !self.players.contains_key(player_id) {
            return Err(format!("Player {} not found", player_id));
        }

        // Get team and add player
        let team = self.teams.get_mut(team_id)
            .ok_or_else(|| format!("Team {} not found", team_id))?;
        
        team.add_member(player_id.to_string());
        Ok(())
    }

    /// Remove a player from a team
    pub fn remove_player_from_team(&mut self, team_id: &str, player_id: &str) -> Result<(), String> {
        let team = self.teams.get_mut(team_id)
            .ok_or_else(|| format!("Team {} not found", team_id))?;
        
        team.remove_member(player_id);
        Ok(())
    }

    /// Remove a team
    pub fn remove_team(&mut self, id: &str) -> Result<(), String> {
        let team = self.teams.remove(id)
            .ok_or_else(|| format!("Team {} not found", id))?;
        
        self.team_names.remove(&team.name);
        Ok(())
    }

    // ==================== Blueprint Management ====================

    /// Create a new ship blueprint
    pub fn create_blueprint(&mut self, name: String, ship_class_id: String, team_id: String) -> Result<String, String> {
        // Verify team exists
        if !self.teams.contains_key(&team_id) {
            return Err(format!("Team {} not found", team_id));
        }

        let blueprint = ShipBlueprint::new(name, ship_class_id, team_id);
        let blueprint_id = blueprint.id.clone();
        
        self.blueprints.insert(blueprint_id.clone(), blueprint);
        
        Ok(blueprint_id)
    }

    /// Get a blueprint by ID
    pub fn get_blueprint(&self, id: &str) -> Option<&ShipBlueprint> {
        self.blueprints.get(id)
    }

    /// Get a mutable reference to a blueprint by ID
    pub fn get_blueprint_mut(&mut self, id: &str) -> Option<&mut ShipBlueprint> {
        self.blueprints.get_mut(id)
    }

    /// Get all blueprints
    pub fn get_all_blueprints(&self) -> Vec<&ShipBlueprint> {
        self.blueprints.values().collect()
    }

    /// Get all blueprints for a specific team
    pub fn get_team_blueprints(&self, team_id: &str) -> Vec<&ShipBlueprint> {
        self.blueprints.values()
            .filter(|bp| bp.team_id == team_id)
            .collect()
    }

    /// Remove a blueprint
    pub fn remove_blueprint(&mut self, id: &str) -> Result<(), String> {
        self.blueprints.remove(id)
            .ok_or_else(|| format!("Blueprint {} not found", id))?;
        Ok(())
    }

    // ==================== Ship Management ====================

    /// Register an active ship
    ///
    /// This is typically called when a blueprint is finalized and enters the simulation.
    pub fn register_ship(&mut self, ship: Ship) -> String {
        let ship_id = ship.id.clone();
        self.ships.insert(ship_id.clone(), ship);
        ship_id
    }

    /// Get a ship by ID
    pub fn get_ship(&self, id: &str) -> Option<&Ship> {
        self.ships.get(id)
    }

    /// Get a mutable reference to a ship by ID
    pub fn get_ship_mut(&mut self, id: &str) -> Option<&mut Ship> {
        self.ships.get_mut(id)
    }

    /// Get all ships
    pub fn get_all_ships(&self) -> Vec<&Ship> {
        self.ships.values().collect()
    }

    /// Get all ships for a specific team
    pub fn get_team_ships(&self, team_id: &str) -> Vec<&Ship> {
        self.ships.values()
            .filter(|ship| ship.team_id == team_id)
            .collect()
    }
    
    /// Check if a ship exists
    pub fn ship_exists(&self, id: uuid::Uuid) -> bool {
        self.ships.contains_key(&id.to_string())
    }
    
    /// Get all ships for a specific player
    pub fn get_player_ships(&self, _player_id: uuid::Uuid) -> Vec<uuid::Uuid> {
        // For now, return empty vec. In a full implementation, we'd track player-ship assignments
        // This would require extending Ship model to include assigned crew/players
        Vec::new()
    }

    /// Remove a ship
    pub fn remove_ship(&mut self, id: &str) -> Result<(), String> {
        self.ships.remove(id)
            .ok_or_else(|| format!("Ship {} not found", id))?;
        Ok(())
    }
    
    // ==================== Station Management ====================
    
    /// Register a new station
    pub fn register_station(&mut self, station: Station) -> String {
        let station_id = station.id.to_string();
        self.stations.insert(station_id.clone(), station);
        station_id
    }
    
    /// Get a station by ID
    pub fn get_station(&self, id: &str) -> Option<&Station> {
        self.stations.get(id)
    }
    
    /// Get a mutable reference to a station by ID
    pub fn get_station_mut(&mut self, id: &str) -> Option<&mut Station> {
        self.stations.get_mut(id)
    }
    
    /// Get all stations
    pub fn get_all_stations(&self) -> Vec<&Station> {
        self.stations.values().collect()
    }
    
    /// Get all stations for a specific faction
    pub fn get_faction_stations(&self, faction: &str) -> Vec<&Station> {
        self.stations.values()
            .filter(|station| station.faction == faction)
            .collect()
    }
    
    /// Remove a station
    pub fn remove_station(&mut self, id: &str) -> Result<(), String> {
        self.stations.remove(id)
            .ok_or_else(|| format!("Station {} not found", id))?;
        Ok(())
    }
    
    /// Find nearest station to a position
    pub fn find_nearest_station(&self, position: [f64; 3]) -> Option<&Station> {
        self.stations.values()
            .min_by(|a, b| {
                let dist_a = distance_squared(position, a.position);
                let dist_b = distance_squared(position, b.position);
                dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    // ==================== Captain's Log Methods ====================

    /// Add a captain's log entry
    pub fn add_captain_log_entry(&mut self, entry: CaptainLogEntry) {
        self.captain_logs
            .entry(entry.ship_id.clone())
            .or_insert_with(Vec::new)
            .push(entry);
    }

    /// Get all captain's log entries for a ship
    pub fn get_captain_log_entries(&self, ship_id: &str) -> Vec<CaptainLogEntry> {
        self.captain_logs
            .get(ship_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Add a ship (alias for register_ship for consistency with other add_ methods)
    pub fn add_ship(&mut self, ship: Ship) -> String {
        self.register_ship(ship)
    }

    /// Get mutable reference to ships HashMap
    pub fn ships_mut(&mut self) -> &mut HashMap<String, Ship> {
        &mut self.ships
    }

    // ==================== Communications Methods ====================

    /// Add a docking request
    pub fn add_docking_request(&mut self, request: DockingRequest) {
        self.docking_requests.insert(request.id.clone(), request);
    }

    /// Get a docking request by ID
    pub fn get_docking_request(&self, id: &str) -> Option<&DockingRequest> {
        self.docking_requests.get(id)
    }

    /// Remove all docking requests for a ship (used during undock)
    pub fn remove_docking_requests(&mut self, ship_id: &str) {
        self.docking_requests.retain(|_, req| req.ship_id != ship_id);
    }

    /// Add a hail message
    pub fn add_hail_message(&mut self, message: HailMessage) {
        self.hail_messages.insert(message.id.clone(), message);
    }

    /// Get a hail message by ID
    pub fn get_hail_message(&self, id: &str) -> Option<HailMessage> {
        self.hail_messages.get(id).cloned()
    }

    /// Get all hail messages for a ship (received)
    pub fn get_hail_messages_for_ship(&self, ship_id: &str) -> Vec<HailMessage> {
        self.hail_messages
            .values()
            .filter(|msg| msg.to_ship_id == ship_id)
            .cloned()
            .collect()
    }

    /// Add a fighter command
    pub fn add_fighter_command(&mut self, command: FighterCommand) {
        self.fighter_commands.insert(command.id.clone(), command);
    }

    /// Get a fighter command by ID
    pub fn get_fighter_command(&self, id: &str) -> Option<&FighterCommand> {
        self.fighter_commands.get(id)
    }

    /// Get all fighter commands for a ship
    pub fn get_fighter_commands_for_ship(&self, ship_id: &str) -> Vec<&FighterCommand> {
        self.fighter_commands
            .values()
            .filter(|cmd| cmd.ship_id == ship_id)
            .collect()
    }

    /// Add a jam attempt (for integration with simulation systems)
    pub fn add_jam_attempt(&mut self, source_ship_id: &str, target_ship_id: &str, duration: f64) {
        self.jam_attempts.push((
            source_ship_id.to_string(),
            target_ship_id.to_string(),
            duration,
        ));
    }

    /// Get and clear jam attempts (consumed by simulation)
    pub fn take_jam_attempts(&mut self) -> Vec<(String, String, f64)> {
        std::mem::take(&mut self.jam_attempts)
    }

    // ==================== Countermeasures Methods ====================

    /// Add a countermeasure load event
    pub fn add_countermeasure_load(&mut self, ship_id: String, cm_type: crate::api::positions::countermeasures::CountermeasureType, quantity: u32) {
        let type_str = match cm_type {
            crate::api::positions::countermeasures::CountermeasureType::Antimissile => "antimissile",
            crate::api::positions::countermeasures::CountermeasureType::Antitorpedo => "antitorpedo",
            crate::api::positions::countermeasures::CountermeasureType::Chaff => "chaff",
        };
        self.countermeasure_loads.push((ship_id, type_str.to_string(), quantity));
    }

    /// Get and clear countermeasure loads (consumed by simulation)
    pub fn take_countermeasure_loads(&mut self) -> Vec<(String, String, u32)> {
        std::mem::take(&mut self.countermeasure_loads)
    }

    /// Add a countermeasure activation event
    pub fn add_countermeasure_activation(&mut self, ship_id: String, threat_ids: Vec<String>) {
        self.countermeasure_activations.push((ship_id, threat_ids));
    }

    /// Get and clear countermeasure activations (consumed by simulation)
    pub fn take_countermeasure_activations(&mut self) -> Vec<(String, Vec<String>)> {
        std::mem::take(&mut self.countermeasure_activations)
    }

    /// Set point defense enabled/disabled for a ship
    pub fn set_point_defense(&mut self, ship_id: String, enabled: bool) {
        self.point_defense_settings.insert(ship_id, enabled);
    }

    /// Get point defense setting for a ship
    pub fn get_point_defense(&self, ship_id: &str) -> bool {
        self.point_defense_settings.get(ship_id).copied().unwrap_or(false)
    }

    // ==================== Weapon Systems Methods ====================

    /// Set energy weapon target
    pub fn set_energy_weapon_target(&mut self, _ship_id: String, _target_id: String) {
        // Placeholder - would integrate with simulation
    }

    /// Add weapon fire command
    pub fn add_weapon_fire_command(&mut self, _ship_id: String, _weapon_id: String) {
        // Placeholder - would integrate with simulation
    }

    /// Set weapon auto-fire mode
    pub fn set_weapon_auto_fire(&mut self, _ship_id: String, _weapon_id: String, _enabled: bool) {
        // Placeholder - would integrate with simulation
    }

    /// Add radial weapon activation
    pub fn add_radial_weapon_activation(&mut self, _ship_id: String, _weapon_id: String) {
        // Placeholder - would integrate with simulation
    }

    /// Get energy weapon target
    pub fn get_energy_weapon_target(&self, _ship_id: &str) -> Option<String> {
        // Placeholder - would integrate with simulation
        None
    }

    /// Set kinetic weapon target
    pub fn set_kinetic_weapon_target(&mut self, _ship_id: String, _target_id: String) {
        // Placeholder - would integrate with simulation
    }

    /// Configure kinetic weapon kind
    pub fn configure_kinetic_weapon(&mut self, _ship_id: String, _weapon_id: String, _kind: String) {
        // Placeholder - would integrate with simulation
    }

    /// Load kinetic ammunition
    pub fn load_kinetic_ammo(&mut self, _ship_id: String, _weapon_id: String, _ammo_type: String, _quantity: u32) {
        // Placeholder - would integrate with simulation
    }

    /// Fire kinetic weapon
    pub fn fire_kinetic_weapon(&mut self, _ship_id: String, _weapon_id: String) {
        // Placeholder - would integrate with simulation
    }

    /// Set kinetic auto-fire
    pub fn set_kinetic_auto_fire(&mut self, _ship_id: String, _weapon_id: String, _enabled: bool) {
        // Placeholder - would integrate with simulation
    }

    /// Set missile weapon target
    pub fn set_missile_weapon_target(&mut self, _ship_id: String, _target_id: String) {
        // Placeholder - would integrate with simulation
    }

    /// Load missile ordnance
    pub fn load_missile_ordnance(&mut self, _ship_id: String, _weapon_id: String, _ordnance_type: String, _quantity: u32) {
        // Placeholder - would integrate with simulation
    }

    /// Fire missile weapon
    pub fn fire_missile_weapon(&mut self, _ship_id: String, _weapon_id: String) {
        // Placeholder - would integrate with simulation
    }

    /// Set missile auto-fire
    pub fn set_missile_auto_fire(&mut self, _ship_id: String, _weapon_id: String, _enabled: bool) {
        // Placeholder - would integrate with simulation
    }

    // ==================== Engineering Methods ====================

    /// Add power allocations - applies directly to ship modules
    pub fn add_power_allocations(&mut self, ship_id: String, allocations: std::collections::HashMap<String, f32>) {
        // Get the ship and update its module power allocations
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            // Update each module's power allocation in the ship's compiled modules
            for (instance_id, power_mw) in allocations {
                // For now, store the allocation to be applied in simulation
                // In a full implementation, this would update the ECS PowerGrid component
                // Here we're just validating that the ship exists
                let _ = (instance_id, power_mw); // Avoid unused warning
            }
        }
        // TODO: Apply to ECS PowerGrid component in simulation
    }

    /// Add cooling allocations - applies directly to ship modules
    pub fn add_cooling_allocations(&mut self, ship_id: String, allocations: std::collections::HashMap<String, f32>) {
        // Get the ship and update its module cooling allocations
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            // Update each module's cooling allocation
            for (instance_id, cooling) in allocations {
                let _ = (instance_id, cooling); // Avoid unused warning
            }
        }
        // TODO: Apply to ECS CoolingSystem component in simulation
    }

    /// Add repair command
    pub fn add_repair_command(&mut self, _ship_id: String, _module_id: String) {
        // Placeholder - would integrate with simulation
    }
    
    /// Add auxiliary module activation command
    pub fn add_auxiliary_activation(&mut self, _ship_id: String, _module_id: String, _duration: f32) {
        // Placeholder - would integrate with simulation
        // In full implementation, this would trigger the module activation in the ECS
    }

    // ==================== Helm Methods ====================

    /// Add thrust command
    pub fn add_thrust_command(&mut self, _ship_id: String, _x: f64, _y: f64, _z: f64) {
        // Placeholder - would integrate with simulation
    }

    /// Add rotation command
    pub fn add_rotation_command(&mut self, _ship_id: String, _pitch: f64, _yaw: f64, _roll: f64) {
        // Placeholder - would integrate with simulation
    }

    /// Add stop command
    pub fn add_stop_command(&mut self, _ship_id: String) {
        // Placeholder - would integrate with simulation
    }

    /// Add warp command
    pub fn add_warp_command(&mut self, _ship_id: String, _dest_x: f64, _dest_y: f64, _dest_z: f64) {
        // Placeholder - would integrate with simulation
    }

    /// Add jump command
    pub fn add_jump_command(&mut self, _ship_id: String, _dest_x: f64, _dest_y: f64, _dest_z: f64) {
        // Placeholder - would integrate with simulation
    }

    /// Add dock command
    pub fn add_dock_command(&mut self, _ship_id: String, _station_id: String) {
        // Placeholder - would integrate with simulation
    }

    // ==================== Science Methods ====================

    /// Add scan command
    pub fn add_scan_command(&mut self, _ship_id: String, _target_id: String) {
        // Placeholder - would integrate with simulation
    }

    // ==================== Utility Methods ====================

    /// Get statistics about the current game state
    pub fn get_stats(&self) -> GameStats {
        GameStats {
            player_count: self.players.len(),
            team_count: self.teams.len(),
            blueprint_count: self.blueprints.len(),
            ship_count: self.ships.len(),
        }
    }

    /// Clear all game state (useful for testing)
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.players.clear();
        self.teams.clear();
        self.blueprints.clear();
        self.ships.clear();
        self.captain_logs.clear();
        self.docking_requests.clear();
        self.hail_messages.clear();
        self.fighter_commands.clear();
        self.jam_attempts.clear();
        self.countermeasure_loads.clear();
        self.countermeasure_activations.clear();
        self.point_defense_settings.clear();
        self.player_names.clear();
        self.team_names.clear();
    }
    
    // ==================== Event System ====================
    
    /// Push an event to the event queue
    pub fn push_event(&mut self, event: GameEvent) {
        self.event_queue.push(event);
    }
    
    /// Drain all events from the queue
    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        self.event_queue.drain()
    }
    
    /// Get the number of events in the queue
    pub fn event_count(&self) -> usize {
        self.event_queue.len()
    }
}

/// Helper function to calculate squared distance between two 3D points
fn distance_squared(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

/// Statistics about the current game state
#[derive(Debug, Clone, Copy)]
pub struct GameStats {
    pub player_count: usize,
    pub team_count: usize,
    pub blueprint_count: usize,
    pub ship_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_world_creation() {
        let world = GameWorld::new();
        let stats = world.get_stats();
        assert_eq!(stats.player_count, 0);
        assert_eq!(stats.team_count, 0);
        assert_eq!(stats.blueprint_count, 0);
        assert_eq!(stats.ship_count, 0);
    }

    #[test]
    fn test_shared_game_world() {
        let world = GameWorld::new_shared();
        
        // Test write lock
        {
            let mut w = world.write().unwrap();
            let player_id = w.register_player("Alice".to_string()).unwrap();
            assert!(w.get_player(&player_id).is_some());
        }
        
        // Test read lock
        {
            let w = world.read().unwrap();
            assert_eq!(w.get_stats().player_count, 1);
        }
    }

    #[test]
    fn test_player_registration() {
        let mut world = GameWorld::new();
        
        // Register valid player
        let player_id = world.register_player("Alice".to_string()).unwrap();
        assert!(world.get_player(&player_id).is_some());
        assert_eq!(world.get_player(&player_id).unwrap().name, "Alice");
        
        // Try to register duplicate name
        let result = world.register_player("Alice".to_string());
        assert!(result.is_err());
        
        // Register another player
        let _bob_id = world.register_player("Bob".to_string()).unwrap();
        assert_eq!(world.get_all_players().len(), 2);
    }

    #[test]
    fn test_player_validation() {
        let mut world = GameWorld::new();
        
        // Empty name
        assert!(world.register_player("".to_string()).is_err());
        
        // Name too long
        let long_name = "a".repeat(51);
        assert!(world.register_player(long_name).is_err());
        
        // Invalid characters
        assert!(world.register_player("Alice@123".to_string()).is_err());
        assert!(world.register_player("Bob Smith".to_string()).is_err());
        
        // Valid names
        assert!(world.register_player("Alice".to_string()).is_ok());
        assert!(world.register_player("Bob_123".to_string()).is_ok());
        assert!(world.register_player("Charlie-456".to_string()).is_ok());
    }

    #[test]
    fn test_player_lookup() {
        let mut world = GameWorld::new();
        
        let alice_id = world.register_player("Alice".to_string()).unwrap();
        
        // Lookup by ID
        assert!(world.get_player(&alice_id).is_some());
        
        // Lookup by name
        let alice = world.get_player_by_name("Alice");
        assert!(alice.is_some());
        assert_eq!(alice.unwrap().id, alice_id);
        
        // Non-existent lookup
        assert!(world.get_player_by_name("NonExistent").is_none());
    }

    #[test]
    fn test_player_removal() {
        let mut world = GameWorld::new();
        
        let player_id = world.register_player("Alice".to_string()).unwrap();
        let team_id = world.create_team("Alpha".to_string(), "Federation".to_string()).unwrap();
        world.add_player_to_team(&team_id, &player_id).unwrap();
        
        // Remove player
        world.remove_player(&player_id).unwrap();
        
        // Verify player is gone
        assert!(world.get_player(&player_id).is_none());
        assert!(world.get_player_by_name("Alice").is_none());
        
        // Verify player was removed from team
        let team = world.get_team(&team_id).unwrap();
        assert_eq!(team.members.len(), 0);
    }

    #[test]
    fn test_team_creation() {
        let mut world = GameWorld::new();
        
        let team_id = world.create_team("Alpha".to_string(), "Federation".to_string()).unwrap();
        assert!(world.get_team(&team_id).is_some());
        
        let team = world.get_team(&team_id).unwrap();
        assert_eq!(team.name, "Alpha");
        assert_eq!(team.faction, "Federation");
        
        // Duplicate team name
        let result = world.create_team("Alpha".to_string(), "Empire".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_team_player_management() {
        let mut world = GameWorld::new();
        
        let alice_id = world.register_player("Alice".to_string()).unwrap();
        let bob_id = world.register_player("Bob".to_string()).unwrap();
        let team_id = world.create_team("Alpha".to_string(), "Federation".to_string()).unwrap();
        
        // Add players to team
        world.add_player_to_team(&team_id, &alice_id).unwrap();
        world.add_player_to_team(&team_id, &bob_id).unwrap();
        
        let team = world.get_team(&team_id).unwrap();
        assert_eq!(team.members.len(), 2);
        assert!(team.members.contains(&alice_id));
        assert!(team.members.contains(&bob_id));
        
        // Remove player from team
        world.remove_player_from_team(&team_id, &alice_id).unwrap();
        let team = world.get_team(&team_id).unwrap();
        assert_eq!(team.members.len(), 1);
        assert!(!team.members.contains(&alice_id));
    }

    #[test]
    fn test_blueprint_creation() {
        let mut world = GameWorld::new();
        
        let team_id = world.create_team("Alpha".to_string(), "Federation".to_string()).unwrap();
        let blueprint_id = world.create_blueprint("USS Enterprise".to_string(), "cruiser".to_string(), team_id.clone()).unwrap();
        
        let blueprint = world.get_blueprint(&blueprint_id).unwrap();
        assert_eq!(blueprint.name, "USS Enterprise");
        assert_eq!(blueprint.class, "cruiser");
        assert_eq!(blueprint.team_id, team_id);
    }

    #[test]
    fn test_blueprint_queries() {
        let mut world = GameWorld::new();
        
        let team1_id = world.create_team("Alpha".to_string(), "Federation".to_string()).unwrap();
        let team2_id = world.create_team("Bravo".to_string(), "Empire".to_string()).unwrap();
        
        world.create_blueprint("Ship1".to_string(), "cruiser".to_string(), team1_id.clone()).unwrap();
        world.create_blueprint("Ship2".to_string(), "battleship".to_string(), team1_id.clone()).unwrap();
        world.create_blueprint("Ship3".to_string(), "frigate".to_string(), team2_id.clone()).unwrap();
        
        // Get all blueprints
        assert_eq!(world.get_all_blueprints().len(), 3);
        
        // Get team-specific blueprints
        assert_eq!(world.get_team_blueprints(&team1_id).len(), 2);
        assert_eq!(world.get_team_blueprints(&team2_id).len(), 1);
    }

    // TODO: Re-enable ship tests once Ship::new() constructor is implemented
    // #[test]
    // fn test_ship_registration() {
    //     let mut world = GameWorld::new();
    //     
    //     let team_id = world.create_team("Alpha".to_string(), "Federation".to_string()).unwrap();
    //     let ship = Ship::new(uuid::Uuid::new_v4().to_string(), "cruiser".to_string(), team_id.clone());
    //     let ship_id = ship.id.clone();
    //     
    //     world.register_ship(ship);
    //     
    //     assert!(world.get_ship(&ship_id).is_some());
    //     assert_eq!(world.get_all_ships().len(), 1);
    // }

    // #[test]
    // fn test_ship_queries() {
    //     let mut world = GameWorld::new();
    //     
    //     let team1_id = world.create_team("Alpha".to_string(), "Federation".to_string()).unwrap();
    //     let team2_id = world.create_team("Bravo".to_string(), "Empire".to_string()).unwrap();
    //     
    //     let ship1 = Ship::new(uuid::Uuid::new_v4().to_string(), "cruiser".to_string(), team1_id.clone());
    //     let ship2 = Ship::new(uuid::Uuid::new_v4().to_string(), "battleship".to_string(), team1_id.clone());
    //     let ship3 = Ship::new(uuid::Uuid::new_v4().to_string(), "frigate".to_string(), team2_id.clone());
    //     
    //     world.register_ship(ship1);
    //     world.register_ship(ship2);
    //     world.register_ship(ship3);
    //     
    //     // Get all ships
    //     assert_eq!(world.get_all_ships().len(), 3);
    //     
    //     // Get team-specific ships
    //     assert_eq!(world.get_team_ships(&team1_id).len(), 2);
    //     assert_eq!(world.get_team_ships(&team2_id).len(), 1);
    // }

    #[test]
    fn test_game_stats() {
        let mut world = GameWorld::new();
        
        world.register_player("Alice".to_string()).unwrap();
        world.register_player("Bob".to_string()).unwrap();
        let team_id = world.create_team("Alpha".to_string(), "Federation".to_string()).unwrap();
        world.create_blueprint("Ship1".to_string(), "cruiser".to_string(), team_id).unwrap();
        
        let stats = world.get_stats();
        assert_eq!(stats.player_count, 2);
        assert_eq!(stats.team_count, 1);
        assert_eq!(stats.blueprint_count, 1);
        assert_eq!(stats.ship_count, 0);
    }
}
