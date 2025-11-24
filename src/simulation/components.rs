//! ECS Components for Simulation
//!
//! Defines all Bevy ECS components used in the HYPERION simulation.
//! These components represent various aspects of ships, weapons, modules,
//! and other entities in the game world.

use bevy_ecs::prelude::*;
use nalgebra::{Vector3, UnitQuaternion};
use std::collections::HashMap;
use crate::models::WeaponTag;
use crate::weapons::StatusEffectType;

/// 3D position, rotation, and velocity
#[derive(Component, Debug, Clone)]
pub struct Transform {
    /// Position in 3D space (meters)
    pub position: Vector3<f32>,
    /// Rotation as a unit quaternion
    pub rotation: UnitQuaternion<f32>,
    /// Linear velocity (meters/second)
    pub velocity: Vector3<f32>,
    /// Angular velocity (radians/second)
    pub angular_velocity: Vector3<f32>,
}

impl Transform {
    /// Create a new transform at the origin
    pub fn new() -> Self {
        Self {
            position: Vector3::zeros(),
            rotation: UnitQuaternion::identity(),
            velocity: Vector3::zeros(),
            angular_velocity: Vector3::zeros(),
        }
    }
    
    /// Create a transform at a specific position
    pub fn at_position(position: Vector3<f32>) -> Self {
        Self {
            position,
            rotation: UnitQuaternion::identity(),
            velocity: Vector3::zeros(),
            angular_velocity: Vector3::zeros(),
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

/// Core ship data and status
#[derive(Component, Debug, Clone)]
pub struct ShipData {
    /// Ship identifier
    pub id: String,
    /// Ship name
    pub name: String,
    /// Ship class identifier
    pub class_id: String,
    /// Team identifier
    pub team_id: String,
    /// Current hull integrity (0.0 = destroyed)
    pub hull: f32,
    /// Maximum hull integrity
    pub max_hull: f32,
    /// Current shield strength (0.0 = shields down)
    pub shields: f32,
    /// Maximum shield strength
    pub max_shields: f32,
    /// Current power available (units)
    pub power: f32,
    /// Maximum power capacity (units)
    pub max_power: f32,
    /// Current cooling available (units)
    pub cooling: f32,
    /// Maximum cooling capacity (units)
    pub max_cooling: f32,
    /// Base weight of the ship (kg)
    pub base_weight: f32,
    /// Effective weight with status effects applied (kg)
    pub effective_weight: f32,
}

impl ShipData {
    /// Create new ship data
    pub fn new(id: String, name: String, class_id: String, team_id: String,
               max_hull: f32, max_shields: f32, base_weight: f32) -> Self {
        Self {
            id,
            name,
            class_id,
            team_id,
            hull: max_hull,
            max_hull,
            shields: max_shields,
            max_shields,
            power: 0.0,
            max_power: 0.0,
            cooling: 0.0,
            max_cooling: 0.0,
            base_weight,
            effective_weight: base_weight,
        }
    }
    
    /// Check if ship is destroyed
    pub fn is_destroyed(&self) -> bool {
        self.hull <= 0.0
    }
    
    /// Check if shields are up
    pub fn shields_up(&self) -> bool {
        self.shields > 0.0
    }
    
    /// Get hull percentage (0.0 to 1.0)
    pub fn hull_percentage(&self) -> f32 {
        if self.max_hull > 0.0 {
            (self.hull / self.max_hull).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
    
    /// Get shield percentage (0.0 to 1.0)
    pub fn shield_percentage(&self) -> f32 {
        if self.max_shields > 0.0 {
            (self.shields / self.max_shields).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Module state and configuration
#[derive(Component, Debug, Clone)]
pub struct ModuleComponent {
    /// Module identifier
    pub id: String,
    /// Module configuration ID (references data files)
    pub config_id: String,
    /// Module type (power-core, impulse-engine, etc.)
    pub module_type: String,
    /// Selected "kind" for modules that support it
    pub kind: Option<String>,
    /// Current health (0.0 = destroyed, 1.0 = perfect)
    pub health: f32,
    /// Current efficiency (0.0 to 1.0, affected by damage)
    pub efficiency: f32,
    /// Power allocated to this module
    pub power_allocation: f32,
    /// Cooling allocated to this module
    pub cooling_allocation: f32,
    /// Whether module is currently active
    pub active: bool,
}

impl ModuleComponent {
    /// Create a new module
    pub fn new(id: String, config_id: String, module_type: String, kind: Option<String>) -> Self {
        Self {
            id,
            config_id,
            module_type,
            kind,
            health: 1.0,
            efficiency: 1.0,
            power_allocation: 0.0,
            cooling_allocation: 0.0,
            active: true,
        }
    }
    
    /// Check if module is operational
    pub fn is_operational(&self) -> bool {
        self.active && self.health > 0.0
    }
    
    /// Calculate effective output based on health and efficiency
    pub fn effective_output(&self, base_output: f32) -> f32 {
        if self.is_operational() {
            base_output * self.health * self.efficiency
        } else {
            0.0
        }
    }
}

/// Weapon state and configuration
#[derive(Component, Debug, Clone)]
pub struct WeaponComponent {
    /// Weapon identifier
    pub id: String,
    /// Weapon configuration ID (references data files)
    pub config_id: String,
    /// Weapon type (missile, directed-energy, kinetic)
    pub weapon_type: String,
    /// Weapon tags
    pub tags: Vec<WeaponTag>,
    /// Base damage before modifiers
    pub base_damage: f32,
    /// Current cooldown timer (seconds)
    pub cooldown: f32,
    /// Maximum cooldown time (seconds)
    pub max_cooldown: f32,
    /// Loaded ammunition (for kinetic/missile weapons)
    pub ammunition: Option<String>,
    /// Ammunition count in magazine
    pub ammo_count: u32,
    /// Whether weapon can fire automatically when target locked
    pub is_automatic: bool,
    /// Whether weapon is currently active/enabled
    pub is_active: bool,
}

impl WeaponComponent {
    /// Create a new weapon
    pub fn new(id: String, config_id: String, weapon_type: String, 
               tags: Vec<WeaponTag>, base_damage: f32, max_cooldown: f32) -> Self {
        let is_automatic = tags.contains(&WeaponTag::Automatic);
        
        Self {
            id,
            config_id,
            weapon_type,
            tags,
            base_damage,
            cooldown: 0.0,
            max_cooldown,
            ammunition: None,
            ammo_count: 0,
            is_automatic,
            is_active: true,
        }
    }
    
    /// Check if weapon can fire
    pub fn can_fire(&self) -> bool {
        self.is_active && self.cooldown <= 0.0 && self.has_ammunition()
    }
    
    /// Check if weapon has ammunition (always true for energy weapons)
    pub fn has_ammunition(&self) -> bool {
        if self.weapon_type == "kinetic" || self.weapon_type == "missile" {
            self.ammo_count > 0
        } else {
            true // Energy weapons don't need ammo
        }
    }
    
    /// Update cooldown
    pub fn update_cooldown(&mut self, delta_time: f32) {
        if self.cooldown > 0.0 {
            self.cooldown = (self.cooldown - delta_time).max(0.0);
        }
    }
    
    /// Fire the weapon (sets cooldown and consumes ammo)
    pub fn fire(&mut self) {
        self.cooldown = self.max_cooldown;
        if self.weapon_type == "kinetic" || self.weapon_type == "missile" {
            self.ammo_count = self.ammo_count.saturating_sub(1);
        }
    }
}

/// Targeting system state
#[derive(Component, Debug, Clone)]
pub struct TargetingComponent {
    /// Current target entity (if any)
    pub target: Option<Entity>,
    /// Whether target is locked (required for automatic weapons)
    pub is_locked: bool,
    /// Lock progress (0.0 to 1.0)
    pub lock_progress: f32,
    /// Time to achieve lock (seconds)
    pub lock_time: f32,
    /// Whether targeting is disabled (by Ion weapons)
    pub disabled: bool,
}

impl TargetingComponent {
    /// Create a new targeting component
    pub fn new(lock_time: f32) -> Self {
        Self {
            target: None,
            is_locked: false,
            lock_progress: 0.0,
            lock_time,
            disabled: false,
        }
    }
    
    /// Check if can engage target
    pub fn can_engage(&self) -> bool {
        !self.disabled && self.target.is_some() && self.is_locked
    }
    
    /// Clear target
    pub fn clear_target(&mut self) {
        self.target = None;
        self.is_locked = false;
        self.lock_progress = 0.0;
    }
}

impl Default for TargetingComponent {
    fn default() -> Self {
        Self::new(3.0) // Default 3 second lock time
    }
}

/// Shield system state
#[derive(Component, Debug, Clone)]
pub struct ShieldComponent {
    /// Current shield strength
    pub strength: f32,
    /// Maximum shield strength
    pub max_strength: f32,
    /// Regeneration rate per second (when raised)
    pub regen_rate: f32,
    /// Whether shields are raised (only regenerate when raised)
    pub raised: bool,
    /// Power required to maintain shields
    pub power_draw: f32,
}

impl ShieldComponent {
    /// Create new shield component
    pub fn new(max_strength: f32, regen_rate: f32, power_draw: f32) -> Self {
        Self {
            strength: max_strength,
            max_strength,
            regen_rate,
            raised: true,
            power_draw,
        }
    }
    
    /// Check if shields are active
    pub fn is_active(&self) -> bool {
        self.raised && self.strength > 0.0
    }
    
    /// Regenerate shields
    pub fn regenerate(&mut self, delta_time: f32) {
        if self.raised {
            self.strength = (self.strength + self.regen_rate * delta_time).min(self.max_strength);
        }
    }
    
    /// Apply damage to shields
    pub fn apply_damage(&mut self, damage: f32) -> f32 {
        if !self.raised || self.strength <= 0.0 {
            return damage; // All damage passes through
        }
        
        if damage >= self.strength {
            let overflow = damage - self.strength;
            self.strength = 0.0;
            overflow
        } else {
            self.strength -= damage;
            0.0 // No damage passed through
        }
    }
}

/// Power grid system
#[derive(Component, Debug, Clone)]
pub struct PowerGrid {
    /// Total power generation (units/second)
    pub generation: f32,
    /// Power distribution to modules
    pub distribution: HashMap<String, f32>,
    /// Current power capacity
    pub capacity: f32,
}

impl PowerGrid {
    /// Create new power grid
    pub fn new(generation: f32, capacity: f32) -> Self {
        Self {
            generation,
            distribution: HashMap::new(),
            capacity,
        }
    }
    
    /// Get total allocated power
    pub fn total_allocated(&self) -> f32 {
        self.distribution.values().sum()
    }
    
    /// Get available power
    pub fn available_power(&self) -> f32 {
        (self.generation - self.total_allocated()).max(0.0)
    }
    
    /// Allocate power to a module
    pub fn allocate(&mut self, module_id: String, amount: f32) {
        self.distribution.insert(module_id, amount);
    }
}

/// Cooling system
#[derive(Component, Debug, Clone)]
pub struct CoolingSystem {
    /// Heat dissipation rate (units/second)
    pub dissipation: f32,
    /// Cooling distribution to modules
    pub distribution: HashMap<String, f32>,
    /// Current cooling capacity
    pub capacity: f32,
}

impl CoolingSystem {
    /// Create new cooling system
    pub fn new(dissipation: f32, capacity: f32) -> Self {
        Self {
            dissipation,
            distribution: HashMap::new(),
            capacity,
        }
    }
    
    /// Get total allocated cooling
    pub fn total_allocated(&self) -> f32 {
        self.distribution.values().sum()
    }
    
    /// Get available cooling
    pub fn available_cooling(&self) -> f32 {
        (self.dissipation - self.total_allocated()).max(0.0)
    }
    
    /// Allocate cooling to a module
    pub fn allocate(&mut self, module_id: String, amount: f32) {
        self.distribution.insert(module_id, amount);
    }
}

/// Damage tracking per module
#[derive(Component, Debug, Clone, Default)]
pub struct DamageComponent {
    /// Module damage map (module_id -> damage amount)
    pub module_damage: HashMap<String, f32>,
    /// Total hull damage taken
    pub hull_damage: f32,
}

impl DamageComponent {
    /// Create new damage component
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Apply damage to a module
    pub fn damage_module(&mut self, module_id: String, damage: f32) {
        *self.module_damage.entry(module_id).or_insert(0.0) += damage;
    }
    
    /// Apply hull damage
    pub fn damage_hull(&mut self, damage: f32) {
        self.hull_damage += damage;
    }
    
    /// Get damage to specific module
    pub fn get_module_damage(&self, module_id: &str) -> f32 {
        self.module_damage.get(module_id).copied().unwrap_or(0.0)
    }
}

/// Inventory for ammunition and cargo
#[derive(Component, Debug, Clone, Default)]
pub struct InventoryComponent {
    /// Ammunition storage (ammo_id -> count)
    pub ammunition: HashMap<String, u32>,
    /// Cargo storage (item_id -> count)
    pub cargo: HashMap<String, u32>,
}

impl InventoryComponent {
    /// Create new inventory
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add ammunition
    pub fn add_ammo(&mut self, ammo_id: String, count: u32) {
        *self.ammunition.entry(ammo_id).or_insert(0) += count;
    }
    
    /// Remove ammunition
    pub fn remove_ammo(&mut self, ammo_id: &str, count: u32) -> bool {
        if let Some(current) = self.ammunition.get_mut(ammo_id) {
            if *current >= count {
                *current -= count;
                return true;
            }
        }
        false
    }
    
    /// Get ammunition count
    pub fn get_ammo_count(&self, ammo_id: &str) -> u32 {
        self.ammunition.get(ammo_id).copied().unwrap_or(0)
    }
}

/// Active status effects on a ship
#[derive(Component, Debug, Clone, Default)]
pub struct StatusEffects {
    /// Active effects (effect_type -> remaining duration)
    pub effects: HashMap<StatusEffectType, f32>,
}

impl StatusEffects {
    /// Create new status effects component
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Apply a status effect
    pub fn apply(&mut self, effect_type: StatusEffectType, duration: f32) {
        // Status effects don't stack - replace with longer duration
        self.effects.entry(effect_type)
            .and_modify(|d| *d = d.max(duration))
            .or_insert(duration);
    }
    
    /// Update status effects (decay over time)
    pub fn update(&mut self, delta_time: f32) {
        self.effects.retain(|_, duration| {
            *duration -= delta_time;
            *duration > 0.0
        });
    }
    
    /// Check if effect is active
    pub fn has_effect(&self, effect_type: StatusEffectType) -> bool {
        self.effects.contains_key(&effect_type)
    }
    
    /// Get remaining duration of effect
    pub fn get_duration(&self, effect_type: StatusEffectType) -> Option<f32> {
        self.effects.get(&effect_type).copied()
    }
}

/// Projectile component for missiles, torpedoes, kinetic rounds, and beams
#[derive(Component, Debug, Clone)]
pub struct ProjectileComponent {
    /// Projectile type
    pub projectile_type: ProjectileType,
    /// Owner entity (who fired it)
    pub owner: Entity,
    /// Target entity (if applicable)
    pub target: Option<Entity>,
    /// Damage to deal on impact
    pub damage: f32,
    /// Weapon tags from source weapon
    pub tags: Vec<WeaponTag>,
    /// Remaining lifetime (seconds)
    pub lifetime: f32,
}

/// Type of projectile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileType {
    /// Kinetic round (bullet, shell, slug)
    Kinetic,
    /// Guided missile
    Missile {
        /// Thrust force
        thrust: u32,
        /// Turn rate (degrees/second)
        turn_rate: u32,
    },
    /// Unguided torpedo
    Torpedo,
    /// Energy beam
    Beam,
}

impl ProjectileComponent {
    /// Create a kinetic projectile
    pub fn kinetic(owner: Entity, damage: f32, tags: Vec<WeaponTag>, lifetime: f32) -> Self {
        Self {
            projectile_type: ProjectileType::Kinetic,
            owner,
            target: None,
            damage,
            tags,
            lifetime,
        }
    }
    
    /// Create a missile projectile
    pub fn missile(owner: Entity, target: Entity, damage: f32, 
                   tags: Vec<WeaponTag>, thrust: u32, turn_rate: u32, lifetime: f32) -> Self {
        Self {
            projectile_type: ProjectileType::Missile { thrust, turn_rate },
            owner,
            target: Some(target),
            damage,
            tags,
            lifetime,
        }
    }
    
    /// Check if projectile is expired
    pub fn is_expired(&self) -> bool {
        self.lifetime <= 0.0
    }
    
    /// Update lifetime
    pub fn update(&mut self, delta_time: f32) {
        self.lifetime -= delta_time;
    }
}

/// Communication state
#[derive(Component, Debug, Clone)]
pub struct CommunicationState {
    /// Whether communications are jammed (by Ion weapons)
    pub jammed: bool,
    /// Jamming duration remaining (seconds)
    pub jam_duration: f32,
}

impl CommunicationState {
    /// Create new communication state
    pub fn new() -> Self {
        Self {
            jammed: false,
            jam_duration: 0.0,
        }
    }
    
    /// Check if can communicate
    pub fn can_communicate(&self) -> bool {
        !self.jammed
    }
    
    /// Apply jamming
    pub fn jam(&mut self, duration: f32) {
        self.jammed = true;
        self.jam_duration = self.jam_duration.max(duration);
    }
    
    /// Update jamming state
    pub fn update(&mut self, delta_time: f32) {
        if self.jammed {
            self.jam_duration -= delta_time;
            if self.jam_duration <= 0.0 {
                self.jammed = false;
                self.jam_duration = 0.0;
            }
        }
    }
}

impl Default for CommunicationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Warp drive component (FTL acceleration drive)
#[derive(Component, Debug, Clone)]
pub struct WarpDriveComponent {
    /// Maximum warp speed multiplier
    pub max_warp_factor: f32,
    /// Current warp factor (1.0 = normal speed, higher = faster)
    pub current_warp_factor: f32,
    /// Startup time before warp engages (seconds)
    pub startup_time: f32,
    /// Current startup progress (0.0 to startup_time)
    pub startup_progress: f32,
    /// Cooldown time after dropping out of warp (seconds)
    pub cooldown_time: f32,
    /// Current cooldown progress (0.0 to cooldown_time)
    pub cooldown_progress: f32,
    /// Whether warp drive is currently active
    pub active: bool,
    /// Whether warp drive is disabled (by Tachyon weapons)
    pub disabled: bool,
}

impl WarpDriveComponent {
    /// Create new warp drive
    pub fn new(max_warp_factor: f32, startup_time: f32, cooldown_time: f32) -> Self {
        Self {
            max_warp_factor,
            current_warp_factor: 1.0,
            startup_time,
            startup_progress: 0.0,
            cooldown_time,
            cooldown_progress: 0.0,
            active: false,
            disabled: false,
        }
    }
    
    /// Check if can engage warp
    pub fn can_engage(&self) -> bool {
        !self.disabled && !self.active && self.cooldown_progress <= 0.0
    }
    
    /// Check if currently in warp
    pub fn is_in_warp(&self) -> bool {
        self.active && self.current_warp_factor > 1.0
    }
}

/// Jump drive component (instant teleport drive)
#[derive(Component, Debug, Clone)]
pub struct JumpDriveComponent {
    /// Maximum jump range (meters)
    pub max_range: f32,
    /// Startup time before jump executes (seconds)
    pub startup_time: f32,
    /// Current startup progress (0.0 to startup_time)
    pub startup_progress: f32,
    /// Cooldown time between jumps (seconds)
    pub cooldown_time: f32,
    /// Current cooldown progress (0.0 to cooldown_time)
    pub cooldown_progress: f32,
    /// Target jump destination (if jump is charging)
    pub target_destination: Option<Vector3<f32>>,
    /// Whether jump drive is disabled (by Tachyon weapons)
    pub disabled: bool,
}

impl JumpDriveComponent {
    /// Create new jump drive
    pub fn new(max_range: f32, startup_time: f32, cooldown_time: f32) -> Self {
        Self {
            max_range,
            startup_time,
            startup_progress: 0.0,
            cooldown_time,
            cooldown_progress: 0.0,
            target_destination: None,
            disabled: false,
        }
    }
    
    /// Check if can initiate jump
    pub fn can_jump(&self) -> bool {
        !self.disabled && self.target_destination.is_none() && self.cooldown_progress <= 0.0
    }
    
    /// Check if jump is charging
    pub fn is_charging(&self) -> bool {
        self.target_destination.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transform_creation() {
        let transform = Transform::new();
        assert_eq!(transform.position, Vector3::zeros());
        assert_eq!(transform.velocity, Vector3::zeros());
        
        let pos = Vector3::new(100.0, 200.0, 300.0);
        let transform = Transform::at_position(pos);
        assert_eq!(transform.position, pos);
    }
    
    #[test]
    fn test_ship_data() {
        let ship = ShipData::new(
            "ship1".to_string(),
            "USS Enterprise".to_string(),
            "cruiser".to_string(),
            "team1".to_string(),
            1000.0,
            500.0,
            50000.0,
        );
        
        assert_eq!(ship.hull, 1000.0);
        assert_eq!(ship.max_hull, 1000.0);
        assert!(!ship.is_destroyed());
        assert!(ship.shields_up());
        assert_eq!(ship.hull_percentage(), 1.0);
        assert_eq!(ship.shield_percentage(), 1.0);
    }
    
    #[test]
    fn test_module_component() {
        let mut module = ModuleComponent::new(
            "mod1".to_string(),
            "reactor".to_string(),
            "power-core".to_string(),
            Some("fusion".to_string()),
        );
        
        assert!(module.is_operational());
        assert_eq!(module.effective_output(100.0), 100.0);
        
        module.health = 0.5;
        assert_eq!(module.effective_output(100.0), 50.0);
        
        module.active = false;
        assert!(!module.is_operational());
        assert_eq!(module.effective_output(100.0), 0.0);
    }
    
    #[test]
    fn test_weapon_component() {
        let mut weapon = WeaponComponent::new(
            "weap1".to_string(),
            "phaser".to_string(),
            "directed-energy".to_string(),
            vec![WeaponTag::Beam, WeaponTag::Automatic],
            100.0,
            1.5,
        );
        
        assert!(weapon.is_automatic);
        assert!(weapon.can_fire());
        
        weapon.fire();
        assert!(!weapon.can_fire());
        assert_eq!(weapon.cooldown, 1.5);
        
        weapon.update_cooldown(1.0);
        assert_eq!(weapon.cooldown, 0.5);
        
        weapon.update_cooldown(1.0);
        assert_eq!(weapon.cooldown, 0.0);
        assert!(weapon.can_fire());
    }
    
    #[test]
    fn test_targeting_component() {
        let mut targeting = TargetingComponent::new(3.0);
        
        assert!(!targeting.can_engage());
        
        targeting.target = Some(Entity::from_raw(1));
        targeting.is_locked = true;
        assert!(targeting.can_engage());
        
        targeting.disabled = true;
        assert!(!targeting.can_engage());
        
        targeting.disabled = false;
        targeting.clear_target();
        assert!(!targeting.can_engage());
    }
    
    #[test]
    fn test_shield_component() {
        let mut shields = ShieldComponent::new(1000.0, 50.0, 10.0);
        
        assert!(shields.is_active());
        
        // Regenerate shields
        shields.strength = 500.0;
        shields.regenerate(10.0); // 50/s * 10s = 500
        assert_eq!(shields.strength, 1000.0);
        
        // Apply damage
        let overflow = shields.apply_damage(600.0);
        assert_eq!(shields.strength, 400.0);
        assert_eq!(overflow, 0.0);
        
        // Damage exceeds shields
        let overflow = shields.apply_damage(500.0);
        assert_eq!(shields.strength, 0.0);
        assert_eq!(overflow, 100.0);
    }
    
    #[test]
    fn test_power_grid() {
        let mut grid = PowerGrid::new(1000.0, 1000.0);
        
        assert_eq!(grid.available_power(), 1000.0);
        
        grid.allocate("module1".to_string(), 300.0);
        grid.allocate("module2".to_string(), 200.0);
        
        assert_eq!(grid.total_allocated(), 500.0);
        assert_eq!(grid.available_power(), 500.0);
    }
    
    #[test]
    fn test_cooling_system() {
        let mut cooling = CoolingSystem::new(500.0, 500.0);
        
        assert_eq!(cooling.available_cooling(), 500.0);
        
        cooling.allocate("module1".to_string(), 150.0);
        cooling.allocate("module2".to_string(), 100.0);
        
        assert_eq!(cooling.total_allocated(), 250.0);
        assert_eq!(cooling.available_cooling(), 250.0);
    }
    
    #[test]
    fn test_damage_component() {
        let mut damage = DamageComponent::new();
        
        damage.damage_module("engine".to_string(), 50.0);
        damage.damage_hull(100.0);
        
        assert_eq!(damage.get_module_damage("engine"), 50.0);
        assert_eq!(damage.hull_damage, 100.0);
    }
    
    #[test]
    fn test_inventory_component() {
        let mut inventory = InventoryComponent::new();
        
        inventory.add_ammo("torpedo".to_string(), 10);
        assert_eq!(inventory.get_ammo_count("torpedo"), 10);
        
        assert!(inventory.remove_ammo("torpedo", 5));
        assert_eq!(inventory.get_ammo_count("torpedo"), 5);
        
        assert!(!inventory.remove_ammo("torpedo", 10));
        assert_eq!(inventory.get_ammo_count("torpedo"), 5);
    }
    
    #[test]
    fn test_status_effects() {
        let mut effects = StatusEffects::new();
        
        effects.apply(StatusEffectType::IonJam, 10.0);
        assert!(effects.has_effect(StatusEffectType::IonJam));
        assert_eq!(effects.get_duration(StatusEffectType::IonJam), Some(10.0));
        
        effects.update(5.0);
        assert_eq!(effects.get_duration(StatusEffectType::IonJam), Some(5.0));
        
        effects.update(10.0);
        assert!(!effects.has_effect(StatusEffectType::IonJam));
    }
    
    #[test]
    fn test_projectile_component() {
        let owner = Entity::from_raw(1);
        let target = Entity::from_raw(2);
        
        let mut projectile = ProjectileComponent::missile(
            owner,
            target,
            100.0,
            vec![WeaponTag::Missile],
            500,
            45,
            30.0,
        );
        
        assert!(!projectile.is_expired());
        projectile.update(35.0);
        assert!(projectile.is_expired());
    }
    
    #[test]
    fn test_communication_state() {
        let mut comms = CommunicationState::new();
        
        assert!(comms.can_communicate());
        
        comms.jam(10.0);
        assert!(!comms.can_communicate());
        
        comms.update(5.0);
        assert!(!comms.can_communicate());
        
        comms.update(6.0);
        assert!(comms.can_communicate());
    }
    
    #[test]
    fn test_warp_drive_component() {
        let warp = WarpDriveComponent::new(9.9, 5.0, 3.0);
        
        assert!(warp.can_engage());
        assert!(!warp.is_in_warp());
        
        let mut warp = warp;
        warp.disabled = true;
        assert!(!warp.can_engage());
    }
    
    #[test]
    fn test_jump_drive_component() {
        let jump = JumpDriveComponent::new(10000.0, 10.0, 30.0);
        
        assert!(jump.can_jump());
        assert!(!jump.is_charging());
        
        let mut jump = jump;
        jump.target_destination = Some(Vector3::new(5000.0, 0.0, 0.0));
        assert!(jump.is_charging());
        assert!(!jump.can_jump());
    }
}
