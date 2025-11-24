//! Procedural generation module
//!
//! This module provides procedural generation for galaxies, star systems,
//! factions, languages, and history.

pub mod galaxy;
pub mod systems;
pub mod factions;
pub mod languages;
pub mod history;

pub use galaxy::{Galaxy, GalaxySector, Star, StarType, SectorType};
pub use systems::{StarSystem, Planet, PlanetType, AsteroidBelt, StationInfo, StationType};
pub use factions::{ProceduralFaction, FactionGenerator, GovernmentType, FactionTrait, Relationship};
pub use languages::{AlienLanguage, Phonology, WordStructure, SyllablePattern};
pub use history::{HistoricalEvent, EventType, HistoryGenerator};

use serde::{Deserialize, Serialize};

/// Complete procedurally generated universe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralUniverse {
    /// Name of the universe
    pub name: String,
    /// Random seed used for generation
    pub seed: u64,
    /// The galaxy
    pub galaxy: Galaxy,
    /// All star systems
    pub systems: Vec<StarSystem>,
    /// All factions
    pub factions: Vec<ProceduralFaction>,
    /// Faction languages
    pub languages: Vec<AlienLanguage>,
    /// Historical events
    pub history: Vec<HistoricalEvent>,
}

impl ProceduralUniverse {
    /// Generate a complete universe
    pub fn generate(name: String, seed: u64, num_stars: usize, num_factions: usize) -> Self {
        // Generate galaxy
        let galaxy = Galaxy::generate(name.clone(), seed, 50000.0, num_stars);
        
        // Generate star systems for inhabited stars
        let mut systems = Vec::new();
        for star in &galaxy.stars {
            if star.inhabited {
                let system = StarSystem::generate(
                    star.id.clone(),
                    star.name.clone(),
                    star.star_type,
                    true,
                    seed + star.id.bytes().map(|b| b as u64).sum::<u64>(),
                );
                systems.push(system);
            }
        }
        
        // Generate factions
        let inhabited_stars: Vec<String> = systems.iter()
            .map(|s| s.id.clone())
            .collect();
        
        let mut faction_gen = FactionGenerator::new(seed + 1000);
        let factions = faction_gen.generate_factions(num_factions, &inhabited_stars);
        
        // Generate languages for each faction
        let mut languages = Vec::new();
        for (i, faction) in factions.iter().enumerate() {
            let language = AlienLanguage::generate(
                format!("{} Language", faction.name),
                seed + 2000 + i as u64,
            );
            languages.push(language);
        }
        
        // Generate history
        let mut history_gen = HistoryGenerator::new(seed + 3000);
        let history = history_gen.generate_history(&factions, 200); // 200 years of history
        
        ProceduralUniverse {
            name,
            seed,
            galaxy,
            systems,
            factions,
            languages,
            history,
        }
    }
    
    /// Get a star system by ID
    pub fn get_system(&self, id: &str) -> Option<&StarSystem> {
        self.systems.iter().find(|s| s.id == id)
    }
    
    /// Get a faction by ID
    pub fn get_faction(&self, id: &str) -> Option<&ProceduralFaction> {
        self.factions.iter().find(|f| f.id == id)
    }
    
    /// Get a faction's language
    pub fn get_faction_language(&self, faction_id: &str) -> Option<&AlienLanguage> {
        if let Some(faction_idx) = self.factions.iter().position(|f| f.id == faction_id) {
            self.languages.get(faction_idx)
        } else {
            None
        }
    }
    
    /// Get historical events involving a faction
    pub fn get_faction_history(&self, faction_id: &str) -> Vec<&HistoricalEvent> {
        self.history.iter()
            .filter(|e| e.factions.contains(&faction_id.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_universe_generation() {
        let universe = ProceduralUniverse::generate(
            "Test Universe".to_string(),
            42,
            100,
            3,
        );
        
        assert_eq!(universe.galaxy.name, "Test Universe");
        assert_eq!(universe.galaxy.stars.len(), 100);
        assert_eq!(universe.factions.len(), 3);
        assert_eq!(universe.languages.len(), 3);
        assert!(!universe.systems.is_empty());
        assert!(!universe.history.is_empty());
    }
    
    #[test]
    fn test_get_system() {
        let universe = ProceduralUniverse::generate(
            "Test".to_string(),
            123,
            50,
            2,
        );
        
        if !universe.systems.is_empty() {
            let system_id = &universe.systems[0].id;
            let found = universe.get_system(system_id);
            assert!(found.is_some());
            assert_eq!(found.unwrap().id, *system_id);
        }
    }
    
    #[test]
    fn test_get_faction() {
        let universe = ProceduralUniverse::generate(
            "Test".to_string(),
            456,
            50,
            3,
        );
        
        let faction_id = &universe.factions[0].id;
        let found = universe.get_faction(faction_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, *faction_id);
    }
    
    #[test]
    fn test_faction_language() {
        let universe = ProceduralUniverse::generate(
            "Test".to_string(),
            789,
            50,
            2,
        );
        
        let faction_id = &universe.factions[0].id;
        let language = universe.get_faction_language(faction_id);
        assert!(language.is_some());
    }
    
    #[test]
    fn test_faction_history() {
        let universe = ProceduralUniverse::generate(
            "Test".to_string(),
            321,
            50,
            3,
        );
        
        let faction_id = &universe.factions[0].id;
        let history = universe.get_faction_history(faction_id);
        assert!(!history.is_empty());
    }
}
