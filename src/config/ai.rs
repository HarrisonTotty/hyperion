//! AI configuration
//!
//! This module defines configuration for AI behavior, personalities, and decision-making.

use serde::{Deserialize, Serialize};

/// AI system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    /// AI update settings
    pub update: AIUpdateConfig,
    /// Personality configurations
    pub personalities: AIPersonalityConfigs,
    /// Combat settings
    pub combat: AICombatConfig,
    /// Movement and navigation
    pub navigation: AINavigationConfig,
}

/// AI update and tick configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIUpdateConfig {
    /// AI tick rate in milliseconds (default: 100ms = 10 ticks/second)
    pub tick_rate_ms: u64,
    /// Maximum AI ships to process per tick (for performance)
    pub max_ships_per_tick: usize,
    /// Behavior tree evaluation depth limit
    pub max_behavior_depth: usize,
}

/// Configuration for all AI personalities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIPersonalityConfigs {
    pub aggressive: PersonalityConfig,
    pub defensive: PersonalityConfig,
    pub passive: PersonalityConfig,
    pub trader: PersonalityConfig,
    pub patrol: PersonalityConfig,
}

/// Configuration for a specific personality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityConfig {
    /// How likely to engage in combat (0.0 - 1.0)
    pub aggression: f32,
    /// Distance to maintain from enemies (in meters)
    pub preferred_range: f32,
    /// Health percentage to retreat at (0.0 - 1.0)
    pub retreat_threshold: f32,
    /// Shield percentage to consider raising shields (0.0 - 1.0)
    pub shield_raise_threshold: f32,
    /// How likely to use special abilities (0.0 - 1.0)
    pub ability_usage: f32,
    /// Patrol radius for patrol routes (in meters)
    pub patrol_radius: f32,
}

/// Combat-related AI settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AICombatConfig {
    /// Target selection settings
    pub target_selection: TargetSelectionConfig,
    /// Weapon usage settings
    pub weapons: WeaponUsageConfig,
    /// Defensive maneuvers
    pub defense: DefensiveConfig,
}

/// Target selection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetSelectionConfig {
    /// Weight for distance in target selection (closer = higher priority)
    pub distance_weight: f32,
    /// Weight for threat level (more dangerous = higher priority)
    pub threat_weight: f32,
    /// Weight for hull integrity (weaker = higher priority)
    pub vulnerability_weight: f32,
    /// Maximum targeting range (meters)
    pub max_range: f32,
    /// Minimum time between target switches (seconds)
    pub retarget_cooldown: f32,
}

/// Weapon usage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponUsageConfig {
    /// Optimal firing range as fraction of max range (0.0 - 1.0)
    pub optimal_range_fraction: f32,
    /// Energy weapon priority (0-10, higher = prefer energy weapons)
    pub energy_priority: u8,
    /// Kinetic weapon priority (0-10)
    pub kinetic_priority: u8,
    /// Missile weapon priority (0-10)
    pub missile_priority: u8,
    /// Minimum power level to fire weapons (0.0 - 1.0)
    pub min_power_to_fire: f32,
}

/// Defensive configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefensiveConfig {
    /// Distance to start evasive maneuvers (meters)
    pub evasion_range: f32,
    /// Evasion pattern complexity (1-5, higher = more erratic)
    pub evasion_complexity: u8,
    /// Countermeasure usage threshold (0.0 - 1.0, incoming missile count ratio)
    pub countermeasure_threshold: f32,
    /// Point defense activation range (meters)
    pub point_defense_range: f32,
}

/// Navigation and movement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AINavigationConfig {
    /// Approach speed as fraction of max speed (0.0 - 1.0)
    pub approach_speed: f32,
    /// Combat speed as fraction of max speed
    pub combat_speed: f32,
    /// Retreat speed (typically max speed = 1.0)
    pub retreat_speed: f32,
    /// Patrol speed as fraction of max speed
    pub patrol_speed: f32,
    /// Waypoint arrival threshold (meters)
    pub waypoint_threshold: f32,
    /// Time to wait at patrol waypoint (seconds)
    pub patrol_wait_time: f32,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            update: AIUpdateConfig {
                tick_rate_ms: 100,
                max_ships_per_tick: 100,
                max_behavior_depth: 10,
            },
            personalities: AIPersonalityConfigs {
                aggressive: PersonalityConfig {
                    aggression: 0.9,
                    preferred_range: 2000.0,
                    retreat_threshold: 0.2,
                    shield_raise_threshold: 0.8,
                    ability_usage: 0.8,
                    patrol_radius: 5000.0,
                },
                defensive: PersonalityConfig {
                    aggression: 0.3,
                    preferred_range: 5000.0,
                    retreat_threshold: 0.5,
                    shield_raise_threshold: 0.9,
                    ability_usage: 0.6,
                    patrol_radius: 3000.0,
                },
                passive: PersonalityConfig {
                    aggression: 0.1,
                    preferred_range: 8000.0,
                    retreat_threshold: 0.7,
                    shield_raise_threshold: 0.95,
                    ability_usage: 0.3,
                    patrol_radius: 2000.0,
                },
                trader: PersonalityConfig {
                    aggression: 0.1,
                    preferred_range: 10000.0,
                    retreat_threshold: 0.8,
                    shield_raise_threshold: 0.95,
                    ability_usage: 0.2,
                    patrol_radius: 15000.0,
                },
                patrol: PersonalityConfig {
                    aggression: 0.5,
                    preferred_range: 3000.0,
                    retreat_threshold: 0.4,
                    shield_raise_threshold: 0.85,
                    ability_usage: 0.5,
                    patrol_radius: 10000.0,
                },
            },
            combat: AICombatConfig {
                target_selection: TargetSelectionConfig {
                    distance_weight: 1.0,
                    threat_weight: 1.5,
                    vulnerability_weight: 1.2,
                    max_range: 50000.0,
                    retarget_cooldown: 5.0,
                },
                weapons: WeaponUsageConfig {
                    optimal_range_fraction: 0.7,
                    energy_priority: 7,
                    kinetic_priority: 5,
                    missile_priority: 6,
                    min_power_to_fire: 0.3,
                },
                defense: DefensiveConfig {
                    evasion_range: 3000.0,
                    evasion_complexity: 3,
                    countermeasure_threshold: 0.5,
                    point_defense_range: 5000.0,
                },
            },
            navigation: AINavigationConfig {
                approach_speed: 0.8,
                combat_speed: 0.9,
                retreat_speed: 1.0,
                patrol_speed: 0.6,
                waypoint_threshold: 100.0,
                patrol_wait_time: 10.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ai_config() {
        let config = AIConfig::default();
        assert_eq!(config.update.tick_rate_ms, 100);
        assert!(config.personalities.aggressive.aggression > 0.8);
        assert!(config.personalities.passive.aggression < 0.2);
    }

    #[test]
    fn test_personality_ranges() {
        let config = AIConfig::default();
        
        // Aggression should be 0.0-1.0
        assert!(config.personalities.aggressive.aggression <= 1.0);
        assert!(config.personalities.defensive.aggression <= 1.0);
        
        // Retreat thresholds should be reasonable
        assert!(config.personalities.aggressive.retreat_threshold < 0.5);
        assert!(config.personalities.defensive.retreat_threshold > 0.3);
    }

    #[test]
    fn test_serialization() {
        let config = AIConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: AIConfig = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(config.update.tick_rate_ms, deserialized.update.tick_rate_ms);
        assert_eq!(config.personalities.aggressive.aggression, deserialized.personalities.aggressive.aggression);
    }
}
