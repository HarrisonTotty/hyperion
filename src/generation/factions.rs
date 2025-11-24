//! NPC Faction generation
//!
//! This module generates procedural factions with traits, territories, and relationships.

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A procedurally generated faction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralFaction {
    /// Unique identifier
    pub id: String,
    /// Faction name
    pub name: String,
    /// Government type
    pub government: GovernmentType,
    /// Primary traits
    pub traits: Vec<FactionTrait>,
    /// Technology level (1-10)
    pub tech_level: u8,
    /// Military strength (1-10)
    pub military_strength: u8,
    /// Economic power (1-10)
    pub economic_power: u8,
    /// Controlled territories (star system IDs)
    pub territories: Vec<String>,
    /// Relationships with other factions
    pub relationships: HashMap<String, Relationship>,
}

/// Type of government
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernmentType {
    /// Democratic federation
    Democracy,
    /// Military dictatorship
    MilitaryDictatorship,
    /// Hereditary monarchy
    Monarchy,
    /// Corporate oligarchy
    Corporate,
    /// Collective hive mind
    Collective,
    /// Theocratic rule
    Theocracy,
    /// Anarchist confederation
    Anarchy,
}

/// Faction behavioral trait
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionTrait {
    /// Aggressive expansionists
    Expansionist,
    /// Defensive isolationists
    Isolationist,
    /// Focus on trade
    Mercantile,
    /// Scientific research focus
    Scientific,
    /// Religious zealots
    Zealous,
    /// Honor-bound warriors
    Honorable,
    /// Deceptive and cunning
    Cunning,
    /// Xenophobic
    Xenophobic,
    /// Welcoming to aliens
    Xenophilic,
    /// Militaristic
    Militaristic,
    /// Pacifist
    Pacifist,
}

/// Relationship between factions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relationship {
    /// Allied
    Allied,
    /// Friendly
    Friendly,
    /// Neutral
    Neutral,
    /// Unfriendly
    Unfriendly,
    /// Hostile
    Hostile,
    /// At war
    War,
}

impl Relationship {
    /// Get numeric value (-3 to +3)
    pub fn value(&self) -> i32 {
        match self {
            Relationship::Allied => 3,
            Relationship::Friendly => 2,
            Relationship::Neutral => 0,
            Relationship::Unfriendly => -1,
            Relationship::Hostile => -2,
            Relationship::War => -3,
        }
    }
    
    /// Create from numeric value
    pub fn from_value(value: i32) -> Self {
        match value {
            3.. => Relationship::Allied,
            2 => Relationship::Friendly,
            -1..=1 => Relationship::Neutral,
            -1 => Relationship::Unfriendly,
            -2 => Relationship::Hostile,
            ..=-3 => Relationship::War,
        }
    }
}

/// Faction generator
pub struct FactionGenerator {
    seed: u64,
    rng: StdRng,
}

impl FactionGenerator {
    /// Create a new faction generator
    pub fn new(seed: u64) -> Self {
        let rng = StdRng::seed_from_u64(seed);
        FactionGenerator { seed, rng }
    }
    
    /// Generate a set of factions
    pub fn generate_factions(&mut self, num_factions: usize, available_territories: &[String]) -> Vec<ProceduralFaction> {
        let mut factions = Vec::new();
        
        for i in 0..num_factions {
            let faction = self.generate_faction(i, available_territories);
            factions.push(faction);
        }
        
        // Generate relationships between factions
        self.generate_relationships(&mut factions);
        
        factions
    }
    
    fn generate_faction(&mut self, index: usize, available_territories: &[String]) -> ProceduralFaction {
        let id = format!("FACTION-{:03}", index);
        let name = self.generate_faction_name();
        let government = self.generate_government();
        let traits = self.generate_traits();
        
        let tech_level = self.rng.gen_range(1..=10);
        let military_strength = self.rng.gen_range(1..=10);
        let economic_power = self.rng.gen_range(1..=10);
        
        // Assign territories
        let num_territories = self.rng.gen_range(1..=10);
        let mut territories = Vec::new();
        if !available_territories.is_empty() {
            for _ in 0..num_territories.min(available_territories.len()) {
                let idx = self.rng.gen_range(0..available_territories.len());
                if !territories.contains(&available_territories[idx]) {
                    territories.push(available_territories[idx].clone());
                }
            }
        }
        
        ProceduralFaction {
            id,
            name,
            government,
            traits,
            tech_level,
            military_strength,
            economic_power,
            territories,
            relationships: HashMap::new(),
        }
    }
    
    fn generate_faction_name(&mut self) -> String {
        let prefixes = [
            "United", "Free", "Imperial", "Democratic", "People's", "Royal",
            "Corporate", "Galactic", "Star", "Cosmic", "Eternal", "Grand",
        ];
        
        let cores = [
            "Federation", "Empire", "Alliance", "Coalition", "Consortium",
            "Commonwealth", "Republic", "Dominion", "Confederacy", "Union",
        ];
        
        let suffixes = [
            "of Sol", "of Andromeda", "of the Outer Rim", "of the Core Worlds",
            "of Free Traders", "of Enlightened Minds", "of the Void", "",
        ];
        
        let prefix = prefixes[self.rng.gen_range(0..prefixes.len())];
        let core = cores[self.rng.gen_range(0..cores.len())];
        let suffix = suffixes[self.rng.gen_range(0..suffixes.len())];
        
        if suffix.is_empty() {
            format!("{} {}", prefix, core)
        } else {
            format!("{} {} {}", prefix, core, suffix)
        }
    }
    
    fn generate_government(&mut self) -> GovernmentType {
        match self.rng.gen_range(0..7) {
            0 => GovernmentType::Democracy,
            1 => GovernmentType::MilitaryDictatorship,
            2 => GovernmentType::Monarchy,
            3 => GovernmentType::Corporate,
            4 => GovernmentType::Collective,
            5 => GovernmentType::Theocracy,
            _ => GovernmentType::Anarchy,
        }
    }
    
    fn generate_traits(&mut self) -> Vec<FactionTrait> {
        let all_traits = [
            FactionTrait::Expansionist,
            FactionTrait::Isolationist,
            FactionTrait::Mercantile,
            FactionTrait::Scientific,
            FactionTrait::Zealous,
            FactionTrait::Honorable,
            FactionTrait::Cunning,
            FactionTrait::Xenophobic,
            FactionTrait::Xenophilic,
            FactionTrait::Militaristic,
            FactionTrait::Pacifist,
        ];
        
        // Select 2-4 non-conflicting traits
        let num_traits = self.rng.gen_range(2..=4);
        let mut traits = Vec::new();
        
        for _ in 0..num_traits {
            let trait_idx = self.rng.gen_range(0..all_traits.len());
            let new_trait = all_traits[trait_idx];
            
            // Check for conflicts
            if !self.conflicts_with_existing(&new_trait, &traits) {
                traits.push(new_trait);
            }
        }
        
        traits
    }
    
    fn conflicts_with_existing(&self, new_trait: &FactionTrait, existing: &[FactionTrait]) -> bool {
        for trait_val in existing {
            if self.traits_conflict(new_trait, trait_val) {
                return true;
            }
        }
        false
    }
    
    fn traits_conflict(&self, a: &FactionTrait, b: &FactionTrait) -> bool {
        matches!(
            (a, b),
            (FactionTrait::Expansionist, FactionTrait::Isolationist) |
            (FactionTrait::Isolationist, FactionTrait::Expansionist) |
            (FactionTrait::Xenophobic, FactionTrait::Xenophilic) |
            (FactionTrait::Xenophilic, FactionTrait::Xenophobic) |
            (FactionTrait::Militaristic, FactionTrait::Pacifist) |
            (FactionTrait::Pacifist, FactionTrait::Militaristic)
        )
    }
    
    fn generate_relationships(&mut self, factions: &mut [ProceduralFaction]) {
        let num_factions = factions.len();
        
        for i in 0..num_factions {
            for j in (i + 1)..num_factions {
                let relationship = self.calculate_relationship(&factions[i], &factions[j]);
                
                // Store bidirectional relationships
                let id_i = factions[i].id.clone();
                let id_j = factions[j].id.clone();
                
                factions[i].relationships.insert(id_j.clone(), relationship);
                factions[j].relationships.insert(id_i, relationship);
            }
        }
    }
    
    fn calculate_relationship(&mut self, a: &ProceduralFaction, b: &ProceduralFaction) -> Relationship {
        let mut score = 0;
        
        // Government compatibility
        if a.government == b.government {
            score += 1;
        }
        
        // Trait compatibility
        for trait_a in &a.traits {
            for trait_b in &b.traits {
                if trait_a == trait_b {
                    score += 1;
                }
                
                // Negative interactions
                if self.traits_conflict(trait_a, trait_b) {
                    score -= 2;
                }
                
                // Special cases
                if matches!(trait_a, FactionTrait::Xenophobic) && matches!(trait_b, FactionTrait::Xenophilic) {
                    score -= 3;
                }
                
                if matches!(trait_a, FactionTrait::Militaristic) && matches!(trait_b, FactionTrait::Pacifist) {
                    score -= 2;
                }
            }
        }
        
        // Territory proximity (if they share borders, more likely to conflict)
        let shared_borders = a.territories.iter().any(|t| b.territories.contains(t));
        if shared_borders {
            score -= 1;
        }
        
        // Random factor
        score += self.rng.gen_range(-2..=2);
        
        Relationship::from_value(score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_faction_generation() {
        let mut generator = FactionGenerator::new(42);
        let territories = vec!["STAR-001".to_string(), "STAR-002".to_string(), "STAR-003".to_string()];
        let factions = generator.generate_factions(3, &territories);
        
        assert_eq!(factions.len(), 3);
        assert!(!factions[0].name.is_empty());
        assert!(!factions[0].traits.is_empty());
    }
    
    #[test]
    fn test_no_conflicting_traits() {
        let mut generator = FactionGenerator::new(123);
        let factions = generator.generate_factions(10, &[]);
        
        for faction in factions {
            // Check for conflicting traits
            let has_expansionist = faction.traits.contains(&FactionTrait::Expansionist);
            let has_isolationist = faction.traits.contains(&FactionTrait::Isolationist);
            assert!(!(has_expansionist && has_isolationist));
            
            let has_xenophobic = faction.traits.contains(&FactionTrait::Xenophobic);
            let has_xenophilic = faction.traits.contains(&FactionTrait::Xenophilic);
            assert!(!(has_xenophobic && has_xenophilic));
        }
    }
    
    #[test]
    fn test_relationships() {
        let mut generator = FactionGenerator::new(456);
        let factions = generator.generate_factions(5, &[]);
        
        // Each faction should have relationships with all others
        for faction in &factions {
            assert_eq!(faction.relationships.len(), 4); // N-1 relationships
        }
    }
    
    #[test]
    fn test_relationship_symmetry() {
        let mut generator = FactionGenerator::new(789);
        let factions = generator.generate_factions(3, &[]);
        
        // Relationships should be symmetric
        let f0_to_f1 = factions[0].relationships.get(&factions[1].id);
        let f1_to_f0 = factions[1].relationships.get(&factions[0].id);
        
        assert_eq!(f0_to_f1, f1_to_f0);
    }
    
    #[test]
    fn test_territory_assignment() {
        let mut generator = FactionGenerator::new(321);
        let territories = vec![
            "STAR-001".to_string(),
            "STAR-002".to_string(),
            "STAR-003".to_string(),
            "STAR-004".to_string(),
            "STAR-005".to_string(),
        ];
        
        let factions = generator.generate_factions(3, &territories);
        
        // Each faction should have some territories
        for faction in factions {
            assert!(!faction.territories.is_empty());
        }
    }
    
    #[test]
    fn test_deterministic_generation() {
        let mut gen1 = FactionGenerator::new(42);
        let mut gen2 = FactionGenerator::new(42);
        
        let factions1 = gen1.generate_factions(5, &[]);
        let factions2 = gen2.generate_factions(5, &[]);
        
        // Same seed should produce same results
        assert_eq!(factions1[0].name, factions2[0].name);
        assert_eq!(factions1[0].government, factions2[0].government);
        assert_eq!(factions1[0].traits, factions2[0].traits);
    }
}
