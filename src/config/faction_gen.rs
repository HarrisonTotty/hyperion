//! Faction generation configuration
//!
//! This module defines configuration for procedural faction generation and relationships.

use serde::{Deserialize, Serialize};

/// Faction generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionGenConfig {
    /// Government type generation
    pub governments: GovernmentGenConfig,
    /// Trait generation
    pub traits: TraitGenConfig,
    /// Relationship calculation
    pub relationships: RelationshipConfig,
    /// Territory assignment
    pub territory: TerritoryConfig,
}

/// Government type generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernmentGenConfig {
    /// Probabilities for each government type
    pub probabilities: GovernmentProbabilities,
}

/// Probabilities for government types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernmentProbabilities {
    pub democracy: f64,
    pub military_dictatorship: f64,
    pub monarchy: f64,
    pub corporate: f64,
    pub collective: f64,
    pub theocracy: f64,
    pub anarchy: f64,
}

/// Trait generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitGenConfig {
    /// Minimum traits per faction
    pub min_traits: usize,
    /// Maximum traits per faction
    pub max_traits: usize,
    /// Trait probabilities
    pub probabilities: TraitProbabilities,
    /// Conflicting trait pairs
    pub conflicts: Vec<TraitConflict>,
}

/// Probabilities for faction traits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitProbabilities {
    pub expansionist: f64,
    pub isolationist: f64,
    pub mercantile: f64,
    pub scientific: f64,
    pub zealous: f64,
    pub honorable: f64,
    pub cunning: f64,
    pub xenophobic: f64,
    pub xenophilic: f64,
    pub militaristic: f64,
    pub pacifist: f64,
}

/// Trait conflict definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitConflict {
    pub trait1: String,
    pub trait2: String,
}

/// Relationship calculation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipConfig {
    /// Government compatibility modifiers
    pub government_compatibility: GovernmentCompatibility,
    /// Trait interaction modifiers
    pub trait_interactions: TraitInteractions,
    /// Territorial proximity effects
    pub proximity: ProximityConfig,
    /// Relationship thresholds
    pub thresholds: RelationshipThresholds,
}

/// Government compatibility modifiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernmentCompatibility {
    /// Same government type bonus
    pub same_government_bonus: i32,
    /// Democracy vs Democracy
    pub democracy_democracy: i32,
    /// Democracy vs Dictatorship
    pub democracy_dictatorship: i32,
    /// Monarchy vs Corporate
    pub monarchy_corporate: i32,
    /// Collective vs Corporate
    pub collective_corporate: i32,
    /// Default compatibility
    pub default_compatibility: i32,
}

/// Trait interaction modifiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitInteractions {
    /// Xenophilic bonus
    pub xenophilic_bonus: i32,
    /// Xenophobic penalty
    pub xenophobic_penalty: i32,
    /// Expansionist conflict
    pub expansionist_conflict: i32,
    /// Mercantile bonus
    pub mercantile_bonus: i32,
    /// Militaristic vs Pacifist
    pub militaristic_pacifist: i32,
    /// Honorable bonus
    pub honorable_bonus: i32,
    /// Cunning penalty
    pub cunning_penalty: i32,
}

/// Territorial proximity configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProximityConfig {
    /// Penalty for neighboring territories
    pub neighbor_penalty: i32,
    /// Penalty for contested systems
    pub contested_penalty: i32,
    /// Distance threshold for neighbors (light-years)
    pub neighbor_threshold: f64,
}

/// Relationship state thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipThresholds {
    /// Minimum for Allied status
    pub allied_min: i32,
    /// Minimum for Friendly status
    pub friendly_min: i32,
    /// Range for Neutral status
    pub neutral_range: (i32, i32),
    /// Maximum for Unfriendly status
    pub unfriendly_max: i32,
    /// Maximum for Hostile status
    pub hostile_max: i32,
    /// Below this is War
    pub war_max: i32,
}

/// Territory assignment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerritoryConfig {
    /// Minimum systems per faction
    pub min_systems: usize,
    /// Maximum systems per faction
    pub max_systems: usize,
    /// Allow overlapping claims
    pub allow_overlapping: bool,
    /// Territory distribution fairness (0.0-1.0, higher = more equal)
    pub fairness: f64,
}

impl Default for FactionGenConfig {
    fn default() -> Self {
        Self {
            governments: GovernmentGenConfig {
                probabilities: GovernmentProbabilities {
                    democracy: 0.20,
                    military_dictatorship: 0.15,
                    monarchy: 0.15,
                    corporate: 0.15,
                    collective: 0.15,
                    theocracy: 0.10,
                    anarchy: 0.10,
                },
            },
            traits: TraitGenConfig {
                min_traits: 2,
                max_traits: 4,
                probabilities: TraitProbabilities {
                    expansionist: 0.25,
                    isolationist: 0.15,
                    mercantile: 0.30,
                    scientific: 0.25,
                    zealous: 0.15,
                    honorable: 0.20,
                    cunning: 0.20,
                    xenophobic: 0.15,
                    xenophilic: 0.20,
                    militaristic: 0.25,
                    pacifist: 0.15,
                },
                conflicts: vec![
                    TraitConflict {
                        trait1: "Expansionist".to_string(),
                        trait2: "Isolationist".to_string(),
                    },
                    TraitConflict {
                        trait1: "Xenophobic".to_string(),
                        trait2: "Xenophilic".to_string(),
                    },
                    TraitConflict {
                        trait1: "Militaristic".to_string(),
                        trait2: "Pacifist".to_string(),
                    },
                ],
            },
            relationships: RelationshipConfig {
                government_compatibility: GovernmentCompatibility {
                    same_government_bonus: 2,
                    democracy_democracy: 3,
                    democracy_dictatorship: -2,
                    monarchy_corporate: -1,
                    collective_corporate: -3,
                    default_compatibility: 0,
                },
                trait_interactions: TraitInteractions {
                    xenophilic_bonus: 2,
                    xenophobic_penalty: -2,
                    expansionist_conflict: -1,
                    mercantile_bonus: 1,
                    militaristic_pacifist: -3,
                    honorable_bonus: 1,
                    cunning_penalty: -1,
                },
                proximity: ProximityConfig {
                    neighbor_penalty: -1,
                    contested_penalty: -2,
                    neighbor_threshold: 100.0,
                },
                thresholds: RelationshipThresholds {
                    allied_min: 5,
                    friendly_min: 2,
                    neutral_range: (-1, 1),
                    unfriendly_max: -2,
                    hostile_max: -4,
                    war_max: -5,
                },
            },
            territory: TerritoryConfig {
                min_systems: 3,
                max_systems: 15,
                allow_overlapping: false,
                fairness: 0.7,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_faction_gen_config() {
        let config = FactionGenConfig::default();
        assert_eq!(config.traits.min_traits, 2);
        assert_eq!(config.traits.max_traits, 4);
        assert!(!config.territory.allow_overlapping);
    }

    #[test]
    fn test_government_probabilities_sum() {
        let config = FactionGenConfig::default();
        let sum = config.governments.probabilities.democracy
            + config.governments.probabilities.military_dictatorship
            + config.governments.probabilities.monarchy
            + config.governments.probabilities.corporate
            + config.governments.probabilities.collective
            + config.governments.probabilities.theocracy
            + config.governments.probabilities.anarchy;
        
        assert!((sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_serialization() {
        let config = FactionGenConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: FactionGenConfig = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(config.traits.min_traits, deserialized.traits.min_traits);
        assert_eq!(config.relationships.thresholds.allied_min, deserialized.relationships.thresholds.allied_min);
    }
}
