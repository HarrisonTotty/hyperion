//! Simulation and physics configuration
//!
//! This module defines configuration for game physics, combat mechanics, and simulation parameters.

use serde::{Deserialize, Serialize};

/// Simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    /// Physics simulation settings
    pub physics: PhysicsConfig,
    /// Combat mechanics settings
    pub combat: CombatConfig,
    /// Ship systems settings
    pub systems: ShipSystemsConfig,
    /// Docking and stations
    pub docking: DockingConfig,
}

/// Physics simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsConfig {
    /// Simulation tick rate in Hz (updates per second)
    pub tick_rate: f64,
    /// Time step for physics calculations (seconds)
    pub time_step: f64,
    /// Effective weight calculation settings
    pub effective_weight: EffectiveWeightConfig,
    /// Movement physics
    pub movement: MovementConfig,
    /// Rotation physics
    pub rotation: RotationConfig,
}

/// Effective weight calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveWeightConfig {
    /// Base mass multiplier
    pub base_multiplier: f64,
    /// Cargo weight multiplier
    pub cargo_multiplier: f64,
    /// Module weight multiplier
    pub module_multiplier: f64,
    /// Damaged module weight penalty
    pub damage_penalty: f64,
}

/// Movement physics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementConfig {
    /// Maximum acceleration (m/s²)
    pub max_acceleration: f64,
    /// Maximum deceleration (m/s²)
    pub max_deceleration: f64,
    /// Drag coefficient
    pub drag_coefficient: f64,
    /// Thrust efficiency (0.0-1.0)
    pub thrust_efficiency: f64,
}

/// Rotation physics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationConfig {
    /// Maximum angular velocity (rad/s)
    pub max_angular_velocity: f64,
    /// Angular acceleration (rad/s²)
    pub angular_acceleration: f64,
    /// Rotational inertia factor
    pub inertia_factor: f64,
}

/// Combat mechanics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatConfig {
    /// Weapon settings
    pub weapons: WeaponConfig,
    /// Shield settings
    pub shields: ShieldConfig,
    /// Status effects
    pub status_effects: StatusEffectConfig,
    /// Countermeasures
    pub countermeasures: CountermeasureConfig,
}

/// Weapon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponConfig {
    /// Base damage multiplier
    pub base_damage_multiplier: f64,
    /// Energy weapon settings
    pub energy: EnergyWeaponConfig,
    /// Kinetic weapon settings
    pub kinetic: KineticWeaponConfig,
    /// Missile weapon settings
    pub missile: MissileWeaponConfig,
}

/// Energy weapon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyWeaponConfig {
    /// Beam weapon continuous damage per second
    pub beam_dps_multiplier: f64,
    /// Pulse weapon damage multiplier (2 rounds)
    pub pulse_damage_multiplier: f64,
    /// Burst weapon damage multiplier (3 rounds)
    pub burst_damage_multiplier: f64,
    /// Photon shield bonus damage (0.0-1.0 extra)
    pub photon_shield_bonus: f64,
    /// Plasma shield bonus damage
    pub plasma_shield_bonus: f64,
    /// Positron shield bypass chance (0.0-1.0)
    pub positron_bypass_chance: f64,
}

/// Kinetic weapon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KineticWeaponConfig {
    /// Railgun damage multiplier
    pub railgun_multiplier: f64,
    /// Cannon damage multiplier
    pub cannon_multiplier: f64,
    /// Gauss damage multiplier
    pub gauss_multiplier: f64,
    /// Armor penetration bonus (0.0-1.0)
    pub armor_penetration: f64,
}

/// Missile weapon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissileWeaponConfig {
    /// Missile damage multiplier
    pub missile_multiplier: f64,
    /// Torpedo damage multiplier
    pub torpedo_multiplier: f64,
    /// Tracking accuracy (0.0-1.0)
    pub tracking_accuracy: f64,
    /// Evasion difficulty (higher = harder to evade)
    pub evasion_difficulty: f64,
}

/// Shield configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldConfig {
    /// Shield regeneration rate per second (as fraction of max)
    pub regen_rate: f64,
    /// Regeneration delay after damage (seconds)
    pub regen_delay: f64,
    /// Damage absorption (fraction of damage absorbed, 0.0-1.0)
    pub absorption_rate: f64,
    /// Shield recharge rate when raised
    pub recharge_multiplier: f64,
}

/// Status effect configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffectConfig {
    /// Ion effect (power reduction)
    pub ion: StatusEffectParams,
    /// Graviton effect (movement impairment)
    pub graviton: StatusEffectParams,
    /// Tachyon effect (FTL blocking)
    pub tachyon: StatusEffectParams,
}

/// Parameters for a status effect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffectParams {
    /// Base duration in seconds
    pub duration: f64,
    /// Effect intensity (0.0-1.0)
    pub intensity: f64,
    /// Decay rate (intensity lost per second)
    pub decay_rate: f64,
    /// Stack limit (max simultaneous applications)
    pub stack_limit: u32,
}

/// Countermeasure configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountermeasureConfig {
    /// Antimissile effectiveness (0.0-1.0)
    pub antimissile_effectiveness: f64,
    /// Antitorpedo effectiveness
    pub antitorpedo_effectiveness: f64,
    /// Chaff jamming range (meters)
    pub chaff_range: f64,
    /// Chaff duration (seconds)
    pub chaff_duration: f64,
    /// Decoy distraction chance (0.0-1.0)
    pub decoy_effectiveness: f64,
}

/// Ship systems configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipSystemsConfig {
    /// Power system settings
    pub power: PowerConfig,
    /// Cooling system settings
    pub cooling: CoolingConfig,
    /// Repair system settings
    pub repair: RepairConfig,
}

/// Power system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerConfig {
    /// Power generation rate per second
    pub generation_rate: f64,
    /// Power capacity per power module
    pub module_capacity: f64,
    /// Power allocation update rate (Hz)
    pub allocation_rate: f64,
    /// Minimum power for critical systems (0.0-1.0)
    pub critical_minimum: f64,
}

/// Cooling system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolingConfig {
    /// Cooling rate per cooling module
    pub module_cooling_rate: f64,
    /// Heat generation per active module
    pub heat_per_module: f64,
    /// Overheat threshold (0.0-1.0 of max heat)
    pub overheat_threshold: f64,
    /// Damage from overheating (per second)
    pub overheat_damage: f64,
}

/// Repair system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairConfig {
    /// Repair rate (hull points per second)
    pub repair_rate: f64,
    /// Module repair time (seconds per 10% damage)
    pub module_repair_time: f64,
    /// Repair power cost multiplier
    pub power_cost: f64,
}

/// Docking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockingConfig {
    /// Docking request range (meters)
    pub request_range: f64,
    /// Docking approach speed limit (m/s)
    pub approach_speed: f64,
    /// Final docking range (meters)
    pub final_range: f64,
    /// Docking completion time (seconds)
    pub completion_time: f64,
    /// Undocking thrust distance (meters)
    pub undock_distance: f64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            physics: PhysicsConfig {
                tick_rate: 60.0,
                time_step: 1.0 / 60.0,
                effective_weight: EffectiveWeightConfig {
                    base_multiplier: 1.0,
                    cargo_multiplier: 1.0,
                    module_multiplier: 0.5,
                    damage_penalty: 0.1,
                },
                movement: MovementConfig {
                    max_acceleration: 10.0,
                    max_deceleration: 15.0,
                    drag_coefficient: 0.01,
                    thrust_efficiency: 0.85,
                },
                rotation: RotationConfig {
                    max_angular_velocity: 1.0,
                    angular_acceleration: 0.5,
                    inertia_factor: 1.0,
                },
            },
            combat: CombatConfig {
                weapons: WeaponConfig {
                    base_damage_multiplier: 1.0,
                    energy: EnergyWeaponConfig {
                        beam_dps_multiplier: 1.0,
                        pulse_damage_multiplier: 1.2,
                        burst_damage_multiplier: 1.5,
                        photon_shield_bonus: 0.25,
                        plasma_shield_bonus: 0.30,
                        positron_bypass_chance: 0.20,
                    },
                    kinetic: KineticWeaponConfig {
                        railgun_multiplier: 1.3,
                        cannon_multiplier: 1.0,
                        gauss_multiplier: 1.1,
                        armor_penetration: 0.15,
                    },
                    missile: MissileWeaponConfig {
                        missile_multiplier: 2.0,
                        torpedo_multiplier: 3.0,
                        tracking_accuracy: 0.85,
                        evasion_difficulty: 0.7,
                    },
                },
                shields: ShieldConfig {
                    regen_rate: 0.05,
                    regen_delay: 3.0,
                    absorption_rate: 0.8,
                    recharge_multiplier: 1.5,
                },
                status_effects: StatusEffectConfig {
                    ion: StatusEffectParams {
                        duration: 5.0,
                        intensity: 0.5,
                        decay_rate: 0.1,
                        stack_limit: 3,
                    },
                    graviton: StatusEffectParams {
                        duration: 8.0,
                        intensity: 0.6,
                        decay_rate: 0.075,
                        stack_limit: 5,
                    },
                    tachyon: StatusEffectParams {
                        duration: 10.0,
                        intensity: 1.0,
                        decay_rate: 0.1,
                        stack_limit: 1,
                    },
                },
                countermeasures: CountermeasureConfig {
                    antimissile_effectiveness: 0.8,
                    antitorpedo_effectiveness: 0.6,
                    chaff_range: 1000.0,
                    chaff_duration: 5.0,
                    decoy_effectiveness: 0.7,
                },
            },
            systems: ShipSystemsConfig {
                power: PowerConfig {
                    generation_rate: 100.0,
                    module_capacity: 1000.0,
                    allocation_rate: 10.0,
                    critical_minimum: 0.2,
                },
                cooling: CoolingConfig {
                    module_cooling_rate: 50.0,
                    heat_per_module: 10.0,
                    overheat_threshold: 0.9,
                    overheat_damage: 1.0,
                },
                repair: RepairConfig {
                    repair_rate: 5.0,
                    module_repair_time: 2.0,
                    power_cost: 0.5,
                },
            },
            docking: DockingConfig {
                request_range: 10000.0,
                approach_speed: 50.0,
                final_range: 100.0,
                completion_time: 5.0,
                undock_distance: 500.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_simulation_config() {
        let config = SimulationConfig::default();
        assert_eq!(config.physics.tick_rate, 60.0);
        assert!(config.combat.shields.absorption_rate > 0.0);
        assert!(config.combat.shields.absorption_rate <= 1.0);
    }

    #[test]
    fn test_weapon_multipliers() {
        let config = SimulationConfig::default();
        assert!(config.combat.weapons.missile.torpedo_multiplier > config.combat.weapons.missile.missile_multiplier);
        assert!(config.combat.weapons.energy.burst_damage_multiplier > config.combat.weapons.energy.pulse_damage_multiplier);
    }

    #[test]
    fn test_serialization() {
        let config = SimulationConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: SimulationConfig = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(config.physics.tick_rate, deserialized.physics.tick_rate);
        assert_eq!(config.combat.shields.regen_rate, deserialized.combat.shields.regen_rate);
    }
}
