//! Module state tracking for the simulation
//!
//! This module handles runtime state tracking for ship modules, including
//! power allocation, cooling allocation, health, and operational status.
//! It bridges the gap between CompiledModule (ship data) and the ECS simulation.

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use super::components::{PowerGrid, CoolingSystem, ShipData};
use crate::models::CompiledModule;

/// Runtime state for a single module
///
/// Tracks dynamic state that changes during gameplay. This complements
/// the CompiledModule structure which contains static stats and configuration.
#[derive(Debug, Clone)]
pub struct ModuleRuntimeState {
    /// Module instance ID
    pub instance_id: String,
    /// Module type ID (shield-generators, warp-cores, etc.)
    pub module_id: String,
    /// Current operational status
    pub operational: bool,
    /// Current health (0.0 to max_health)
    pub current_health: f32,
    /// Maximum health
    pub max_health: f32,
    /// Power allocation (0.0 to 1.0)
    pub power_allocated: f32,
    /// Cooling allocation (0.0 to 1.0)
    pub cooling_allocated: f32,
    /// Current heat level (0.0 to 1.0+, can exceed 1.0 for overheating)
    pub heat: f32,
    /// Whether module is overheated (performance degraded)
    pub overheated: bool,
    /// Current efficiency (0.0 to 1.0, affected by damage/power/heat)
    pub efficiency: f32,
    /// Power requirement (MW) - from module stats
    pub power_requirement: f32,
    /// Power generation (MW) - from module stats (for power cores)
    pub power_generation: f32,
    /// Cooling capacity (K/s) - from module stats (for cooling systems)
    pub cooling_capacity: f32,
    /// Heat generation rate (K/s) - from module stats
    pub heat_generation: f32,
    /// Maximum uses before recharge (for auxiliary modules)
    pub max_uses: u32,
    /// Remaining uses available
    pub remaining_uses: u32,
    /// Cooldown time between activations (seconds)
    pub cooldown_time: f32,
    /// Current cooldown remaining (seconds)
    pub current_cooldown: f32,
    /// Whether module is currently active
    pub is_active: bool,
    /// Time remaining for current activation (seconds)
    pub activation_time_remaining: f32,
}

impl ModuleRuntimeState {
    /// Create new runtime state from a compiled module
    pub fn from_compiled(module: &CompiledModule) -> Self {
        // Extract power stats from module
        let power_requirement = module.get_stat_f64("power_consumption").unwrap_or(0.0) as f32;
        let power_generation = module.get_stat_f64("production").unwrap_or(0.0) as f32;
        let cooling_capacity = module.get_stat_f64("cooling_capacity").unwrap_or(0.0) as f32;
        let heat_generation = module.get_stat_f64("heat_generation").unwrap_or(0.0) as f32;
        
        // Extract auxiliary module stats
        let max_uses = module.get_stat_f64("max_uses").unwrap_or(0.0) as u32;
        let cooldown_time = module.get_stat_f64("recharge_time").unwrap_or(0.0) as f32;
        
        Self {
            instance_id: module.instance_id.clone(),
            module_id: module.module_id.clone(),
            operational: module.operational,
            current_health: module.current_health,
            max_health: module.max_health,
            power_allocated: module.power_allocated,
            cooling_allocated: module.cooling_allocated,
            heat: 0.0,
            overheated: false,
            efficiency: module.get_efficiency(),
            power_requirement,
            power_generation,
            cooling_capacity,
            heat_generation,
            max_uses,
            remaining_uses: max_uses, // Start with full charges
            cooldown_time,
            current_cooldown: 0.0,
            is_active: false,
            activation_time_remaining: 0.0,
        }
    }
    
    /// Update efficiency based on current state
    ///
    /// Efficiency is affected by:
    /// - Health: Damaged modules are less efficient
    /// - Power: Low power reduces performance
    /// - Heat: Overheated modules are less efficient
    pub fn update_efficiency(&mut self) {
        if !self.operational || self.current_health <= 0.0 {
            self.efficiency = 0.0;
            return;
        }
        
        // Health factor: Linear scaling from damage
        let health_factor = self.current_health / self.max_health;
        
        // Power factor: Power allocation directly affects efficiency
        let power_factor = self.power_allocated;
        
        // Heat factor: Graduated penalties based on heat level
        let heat_factor = if !self.overheated {
            1.0 // Normal operation below 1000 K
        } else {
            // Overheated: efficiency degrades with temperature
            // 1000 K: 100% efficiency
            // 1500 K: 50% efficiency
            // 2000 K: 10% efficiency (minimum)
            let overheat_threshold = 1000.0;
            let critical_threshold = 2000.0;
            
            if self.heat < critical_threshold {
                // Linear degradation from 100% to 10%
                let overheat_amount = (self.heat - overheat_threshold) / (critical_threshold - overheat_threshold);
                (1.0 - overheat_amount * 0.9).max(0.1)
            } else {
                // Critical overheat: 10% minimum efficiency
                0.1
            }
        };
        
        // Combined efficiency
        self.efficiency = health_factor * power_factor * heat_factor;
    }
    
    /// Apply damage to the module
    ///
    /// Returns true if the module was destroyed by this damage.
    pub fn apply_damage(&mut self, amount: f32) -> bool {
        self.current_health = (self.current_health - amount).max(0.0);
        
        // Module becomes non-operational if destroyed
        if self.current_health <= 0.0 {
            self.operational = false;
            self.efficiency = 0.0;
            return true;
        }
        
        // Update efficiency after damage
        self.update_efficiency();
        false
    }
    
    /// Repair the module
    pub fn repair(&mut self, amount: f32) {
        self.current_health = (self.current_health + amount).min(self.max_health);
        
        // Module becomes operational again if repaired above 0
        if self.current_health > 0.0 && !self.operational {
            self.operational = true;
        }
        
        self.update_efficiency();
    }
    
    /// Set power allocation (0.0 to 1.0)
    pub fn set_power_allocation(&mut self, allocation: f32) {
        self.power_allocated = allocation.clamp(0.0, 1.0);
        self.update_efficiency();
    }
    
    /// Set cooling allocation (0.0 to 1.0)
    pub fn set_cooling_allocation(&mut self, allocation: f32) {
        self.cooling_allocated = allocation.clamp(0.0, 1.0);
    }
    
    /// Update heat level based on heat generation and cooling
    ///
    /// # Arguments
    /// * `heat_generated` - Heat generated this tick (K/s)
    /// * `cooling_applied` - Cooling applied this tick (K/s)
    /// * `delta_time` - Time step in seconds
    pub fn update_heat(&mut self, heat_generated: f32, cooling_applied: f32, delta_time: f32) {
        // Add generated heat
        self.heat += heat_generated * delta_time;
        
        // Apply cooling
        self.heat = (self.heat - cooling_applied * delta_time).max(0.0);
        
        // Overheating threshold: heat > 1000 K is considered overheated
        // This is a normalized value where:
        // - 0-1000 K: Normal operation
        // - 1000-2000 K: Overheated (reduced efficiency)
        // - >2000 K: Critical overheat (severe penalties)
        let overheat_threshold = 1000.0;
        let was_overheated = self.overheated;
        self.overheated = self.heat > overheat_threshold;
        
        // Update efficiency if overheat state changed or if already overheated
        if self.overheated || was_overheated != self.overheated {
            self.update_efficiency();
        }
    }
    
    /// Check if module is damaged
    pub fn is_damaged(&self) -> bool {
        self.current_health < self.max_health
    }
    
    /// Check if module is destroyed
    pub fn is_destroyed(&self) -> bool {
        self.current_health <= 0.0
    }
    
    /// Get health percentage (0.0 to 1.0)
    pub fn health_percentage(&self) -> f32 {
        if self.max_health > 0.0 {
            self.current_health / self.max_health
        } else {
            0.0
        }
    }
    
    /// Check if this is an auxiliary module (has limited uses)
    pub fn is_auxiliary(&self) -> bool {
        self.max_uses > 0
    }
    
    /// Check if auxiliary module can be activated
    pub fn can_activate(&self) -> bool {
        self.is_auxiliary() 
            && self.operational 
            && !self.is_destroyed()
            && self.remaining_uses > 0
            && self.current_cooldown <= 0.0
            && !self.is_active
    }
    
    /// Activate auxiliary module
    ///
    /// Returns Ok(duration) if activation succeeded, Err(message) otherwise
    pub fn activate(&mut self, duration: f32) -> Result<f32, String> {
        if !self.can_activate() {
            if !self.is_auxiliary() {
                return Err("Not an auxiliary module".to_string());
            }
            if !self.operational {
                return Err("Module not operational".to_string());
            }
            if self.remaining_uses == 0 {
                return Err("No charges remaining".to_string());
            }
            if self.current_cooldown > 0.0 {
                return Err(format!("Cooldown active: {:.1}s remaining", self.current_cooldown));
            }
            if self.is_active {
                return Err("Module already active".to_string());
            }
            return Err("Cannot activate module".to_string());
        }
        
        // Consume a charge
        self.remaining_uses -= 1;
        
        // Activate the module
        self.is_active = true;
        self.activation_time_remaining = duration;
        
        // Start cooldown
        self.current_cooldown = self.cooldown_time;
        
        Ok(duration)
    }
    
    /// Deactivate auxiliary module
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.activation_time_remaining = 0.0;
    }
    
    /// Update auxiliary module timers
    ///
    /// Should be called each simulation tick for auxiliary modules
    pub fn update_auxiliary_timers(&mut self, delta_time: f32) {
        // Update activation timer
        if self.is_active {
            self.activation_time_remaining -= delta_time;
            if self.activation_time_remaining <= 0.0 {
                self.deactivate();
            }
        }
        
        // Update cooldown timer
        if self.current_cooldown > 0.0 {
            self.current_cooldown = (self.current_cooldown - delta_time).max(0.0);
        }
    }
    
    /// Recharge auxiliary module (restores all uses)
    ///
    /// Called when docked at a station
    pub fn recharge(&mut self) {
        if self.is_auxiliary() {
            self.remaining_uses = self.max_uses;
            self.current_cooldown = 0.0;
            self.deactivate();
        }
    }
}

/// Component that tracks module states for a ship
#[derive(Component, Debug, Clone)]
pub struct ModuleStateTracker {
    /// Map of instance_id to runtime state
    pub states: HashMap<String, ModuleRuntimeState>,
}

impl ModuleStateTracker {
    /// Create new module state tracker from compiled modules
    pub fn from_compiled_modules(modules: &[CompiledModule]) -> Self {
        let states = modules
            .iter()
            .map(|m| (m.instance_id.clone(), ModuleRuntimeState::from_compiled(m)))
            .collect();
        
        Self { states }
    }
    
    /// Get a module state by instance ID
    pub fn get(&self, instance_id: &str) -> Option<&ModuleRuntimeState> {
        self.states.get(instance_id)
    }
    
    /// Get a mutable module state by instance ID
    pub fn get_mut(&mut self, instance_id: &str) -> Option<&mut ModuleRuntimeState> {
        self.states.get_mut(instance_id)
    }
    
    /// Get all modules of a specific type
    pub fn get_modules_by_type(&self, module_id: &str) -> Vec<&ModuleRuntimeState> {
        self.states
            .values()
            .filter(|m| m.module_id == module_id)
            .collect()
    }
    
    /// Get all operational modules
    pub fn get_operational_modules(&self) -> Vec<&ModuleRuntimeState> {
        self.states
            .values()
            .filter(|m| m.operational && !m.is_destroyed())
            .collect()
    }
    
    /// Calculate total efficiency for a module type
    ///
    /// Sums the efficiency of all modules of the given type.
    /// Useful for calculating aggregate capabilities (e.g., total shield strength).
    pub fn total_efficiency_for_type(&self, module_id: &str) -> f32 {
        self.get_modules_by_type(module_id)
            .iter()
            .map(|m| m.efficiency)
            .sum()
    }
}

/// System that updates module states each tick
///
/// This system handles:
/// - Heat generation and dissipation
/// - Efficiency recalculation
/// - Power/cooling distribution
pub fn module_state_system(
    mut query: Query<(&mut ModuleStateTracker, &PowerGrid, &CoolingSystem, &ShipData)>,
    delta_time: f32,
) {
    for (mut tracker, power_grid, cooling_system, _ship_data) in query.iter_mut() {
        // Update each module's state
        for state in tracker.states.values_mut() {
            if !state.operational || state.is_destroyed() {
                continue;
            }
            
            // Get power allocated to this module (in MW)
            let power_allocated_mw = power_grid.distribution
                .get(&state.instance_id)
                .copied()
                .unwrap_or(0.0);
            
            // Get cooling allocated to this module
            let cooling_allocated = cooling_system.distribution
                .get(&state.instance_id)
                .copied()
                .unwrap_or(0.0);
            
            // Calculate power allocation as fraction of requirement
            // If module requires 100 MW and gets 50 MW, allocation is 0.5
            state.power_allocated = if state.power_requirement > 0.0 {
                (power_allocated_mw / state.power_requirement).min(1.0)
            } else {
                1.0 // Modules with no power requirement are always at 100%
            };
            
            // Get cooling allocated to this module (in K/s)
            let cooling_allocated_ks = cooling_system.distribution
                .get(&state.instance_id)
                .copied()
                .unwrap_or(0.0);
            
            // Calculate cooling allocation as fraction of heat generation
            // Module generates 100 K/s heat, gets 50 K/s cooling → 0.5 allocation
            state.cooling_allocated = if state.heat_generation > 0.0 {
                (cooling_allocated_ks / state.heat_generation).min(1.0)
            } else {
                1.0 // Modules with no heat generation always at 100%
            };
            
            // Calculate actual heat generation based on power usage and operational state
            // Heat generation scales with power allocation (module only generates heat when powered)
            let actual_heat_generation = state.heat_generation * state.power_allocated;
            
            // Calculate cooling effectiveness
            let cooling_effectiveness = cooling_allocated_ks;
            
            // Update heat
            state.update_heat(actual_heat_generation, cooling_effectiveness, delta_time);
            
            // Update efficiency
            state.update_efficiency();
            
            // Update auxiliary module timers (activation duration and cooldown)
            if state.is_auxiliary() {
                state.update_auxiliary_timers(delta_time);
            }
        }
    }
}

/// System that propagates module damage to ship capabilities
///
/// When modules are damaged or destroyed, this affects ship-level capabilities.
/// For example, destroyed power cores reduce available power generation.
pub fn module_damage_propagation_system(
    mut query: Query<(&ModuleStateTracker, &mut PowerGrid, &mut CoolingSystem, &mut ShipData)>,
) {
    for (tracker, mut power_grid, mut cooling_system, mut ship_data) in query.iter_mut() {
        // Recalculate power generation from operational power cores
        let power_cores = tracker.get_modules_by_type("power-cores");
        let total_power_generation: f32 = power_cores
            .iter()
            .filter_map(|m| {
                if m.operational && !m.is_destroyed() {
                    // Power generation scales with efficiency
                    // Use actual power_generation stat from module
                    Some(m.power_generation * m.efficiency)
                } else {
                    None
                }
            })
            .sum();
        
        power_grid.generation = total_power_generation;
        
        // Recalculate cooling from operational cooling systems
        let cooling_modules = tracker.get_modules_by_type("cooling-systems");
        let total_cooling: f32 = cooling_modules
            .iter()
            .filter_map(|m| {
                if m.operational && !m.is_destroyed() {
                    // Cooling scales with efficiency
                    // Use actual cooling_capacity stat from module
                    Some(m.cooling_capacity * m.efficiency)
                } else {
                    None
                }
            })
            .sum();
        
        cooling_system.dissipation = total_cooling.max(10.0); // Minimum baseline cooling
        
        // Update ship power and cooling display values
        ship_data.power = power_grid.available_power();
        ship_data.max_power = power_grid.generation;
        ship_data.cooling = cooling_system.available_cooling();
        ship_data.max_cooling = cooling_system.dissipation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModuleStats;
    
    fn create_test_module(instance_id: &str, module_id: &str, health: f32) -> CompiledModule {
        CompiledModule {
            instance_id: instance_id.to_string(),
            module_id: module_id.to_string(),
            kind: None,
            name: "Test Module".to_string(),
            stats: ModuleStats::default(),
            current_health: health,
            max_health: 100.0,
            operational: true,
            power_allocated: 1.0,
            cooling_allocated: 1.0,
        }
    }
    
    #[test]
    fn test_module_runtime_state_creation() {
        let module = create_test_module("mod1", "power-cores", 100.0);
        let state = ModuleRuntimeState::from_compiled(&module);
        
        assert_eq!(state.instance_id, "mod1");
        assert_eq!(state.module_id, "power-cores");
        assert_eq!(state.current_health, 100.0);
        assert_eq!(state.max_health, 100.0);
        assert!(state.operational);
        assert_eq!(state.power_allocated, 1.0);
        assert_eq!(state.efficiency, 1.0);
    }
    
    #[test]
    fn test_module_damage() {
        let module = create_test_module("mod1", "shield-generators", 100.0);
        let mut state = ModuleRuntimeState::from_compiled(&module);
        
        // Apply damage
        let destroyed = state.apply_damage(30.0);
        assert!(!destroyed);
        assert_eq!(state.current_health, 70.0);
        assert!(state.is_damaged());
        assert!(!state.is_destroyed());
        
        // Efficiency should be reduced
        assert!(state.efficiency < 1.0);
        assert!(state.efficiency > 0.0);
    }
    
    #[test]
    fn test_module_destruction() {
        let module = create_test_module("mod1", "impulse-engines", 100.0);
        let mut state = ModuleRuntimeState::from_compiled(&module);
        
        // Destroy module
        let destroyed = state.apply_damage(150.0);
        assert!(destroyed);
        assert_eq!(state.current_health, 0.0);
        assert!(state.is_destroyed());
        assert!(!state.operational);
        assert_eq!(state.efficiency, 0.0);
    }
    
    #[test]
    fn test_module_repair() {
        let module = create_test_module("mod1", "warp-cores", 50.0);
        let mut state = ModuleRuntimeState::from_compiled(&module);
        
        state.repair(30.0);
        assert_eq!(state.current_health, 80.0);
        
        // Can't repair beyond max
        state.repair(50.0);
        assert_eq!(state.current_health, 100.0);
    }
    
    #[test]
    fn test_power_allocation_affects_efficiency() {
        let module = create_test_module("mod1", "shield-generators", 100.0);
        let mut state = ModuleRuntimeState::from_compiled(&module);
        
        // Reduce power allocation
        state.set_power_allocation(0.5);
        assert_eq!(state.power_allocated, 0.5);
        assert_eq!(state.efficiency, 0.5);
        
        // Zero power = zero efficiency
        state.set_power_allocation(0.0);
        assert_eq!(state.efficiency, 0.0);
    }
    
    #[test]
    fn test_heat_and_overheating() {
        let module = create_test_module("mod1", "impulse-engines", 100.0);
        let mut state = ModuleRuntimeState::from_compiled(&module);
        
        // Generate heat without cooling (using realistic Kelvin values)
        // Heat generation of 1200 K/s for 1 second exceeds threshold of 1000 K
        state.update_heat(1200.0, 0.0, 1.0); // 1200 heat/sec for 1 second
        assert_eq!(state.heat, 1200.0);
        assert!(state.overheated); // Should be overheated above 1000 K
        
        // Efficiency should be reduced when overheated
        assert!(state.efficiency < 1.0);
        
        // Apply sufficient cooling to bring below threshold
        state.update_heat(0.0, 1500.0, 1.0); // 1500 cooling/sec for 1 second
        assert!(state.heat < 1000.0); // Should be below overheat threshold
        assert!(!state.overheated);
    }
    
    #[test]
    fn test_combined_efficiency_factors() {
        let module = create_test_module("mod1", "warp-cores", 100.0);
        let mut state = ModuleRuntimeState::from_compiled(&module);
        
        // Start at full efficiency
        assert_eq!(state.efficiency, 1.0);
        
        // Take damage (50% health)
        state.apply_damage(50.0);
        assert_eq!(state.current_health, 50.0);
        assert!((state.efficiency - 0.5).abs() < 0.01);
        
        // Add heat beyond threshold (using realistic values in Kelvin)
        state.update_heat(1500.0, 0.0, 1.0); // Generate 1500 K (overheated)
        assert!(state.overheated);
        
        // Efficiency should be reduced by both damage and heat
        // At 1500 K:
        //   overheat_amount = (1500 - 1000) / (2000 - 1000) = 0.5
        //   heat_factor = 1.0 - 0.5 * 0.9 = 0.55
        // Combined: 0.5 (health) * 1.0 (power, default) * 0.55 (heat) = 0.275
        assert!((state.efficiency - 0.275).abs() < 0.01);
    }
    
    #[test]
    fn test_module_state_tracker() {
        let modules = vec![
            create_test_module("mod1", "power-cores", 100.0),
            create_test_module("mod2", "power-cores", 80.0),
            create_test_module("mod3", "shield-generators", 100.0),
        ];
        
        let tracker = ModuleStateTracker::from_compiled_modules(&modules);
        
        assert_eq!(tracker.states.len(), 3);
        assert!(tracker.get("mod1").is_some());
        
        let power_cores = tracker.get_modules_by_type("power-cores");
        assert_eq!(power_cores.len(), 2);
        
        let operational = tracker.get_operational_modules();
        assert_eq!(operational.len(), 3);
    }
    
    #[test]
    fn test_total_efficiency_calculation() {
        let modules = vec![
            create_test_module("mod1", "shield-generators", 100.0),
            create_test_module("mod2", "shield-generators", 50.0), // 50% health = 50% efficiency
        ];
        
        let mut tracker = ModuleStateTracker::from_compiled_modules(&modules);
        
        // Update efficiency for damaged module
        if let Some(state) = tracker.get_mut("mod2") {
            state.update_efficiency();
        }
        
        let total_eff = tracker.total_efficiency_for_type("shield-generators");
        // mod1: 1.0 efficiency, mod2: 0.5 efficiency = 1.5 total
        assert!((total_eff - 1.5).abs() < 0.01);
    }
}
