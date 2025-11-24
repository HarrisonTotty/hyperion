//! Faction history generation
//!
//! This module generates historical events, wars, alliances, and conflicts between factions.

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};

use super::factions::{ProceduralFaction, Relationship};

/// A historical event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalEvent {
    /// Year (relative to "present")
    pub year: i32,
    /// Event type
    pub event_type: EventType,
    /// Factions involved
    pub factions: Vec<String>,
    /// Event description
    pub description: String,
    /// Impact on relationships
    pub relationship_changes: Vec<(String, String, i32)>, // (faction1, faction2, delta)
}

/// Type of historical event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// War between factions
    War,
    /// Peace treaty signed
    PeaceTreaty,
    /// Alliance formed
    Alliance,
    /// Alliance dissolved
    AllianceDissolved,
    /// First contact
    FirstContact,
    /// Trade agreement
    TradeAgreement,
    /// Border dispute
    BorderDispute,
    /// Technology exchange
    TechnologyExchange,
    /// Assassination/incident
    Incident,
    /// Cultural exchange
    CulturalExchange,
}

/// Faction history generator
pub struct HistoryGenerator {
    rng: StdRng,
}

impl HistoryGenerator {
    /// Create a new history generator
    pub fn new(seed: u64) -> Self {
        let rng = StdRng::seed_from_u64(seed);
        HistoryGenerator { rng }
    }
    
    /// Generate history for factions
    pub fn generate_history(&mut self, factions: &[ProceduralFaction], years: usize) -> Vec<HistoricalEvent> {
        let mut events = Vec::new();
        
        // Generate first contact events
        events.extend(self.generate_first_contacts(factions, years));
        
        // Generate random events over time
        for year in (1..=years).rev() {
            let year_events = self.rng.gen_range(0..=3); // 0-3 events per year
            
            for _ in 0..year_events {
                if let Some(event) = self.generate_random_event(factions, -(year as i32)) {
                    events.push(event);
                }
            }
        }
        
        // Sort by year
        events.sort_by_key(|e| e.year);
        
        events
    }
    
    fn generate_first_contacts(&mut self, factions: &[ProceduralFaction], years: usize) -> Vec<HistoricalEvent> {
        let mut events = Vec::new();
        
        for i in 0..factions.len() {
            for j in (i + 1)..factions.len() {
                // First contacts happen in the first half of history
                let max_year = years.max(2);
                let year = -(self.rng.gen_range(max_year/2..max_year) as i32);
                
                let event = HistoricalEvent {
                    year,
                    event_type: EventType::FirstContact,
                    factions: vec![factions[i].id.clone(), factions[j].id.clone()],
                    description: format!(
                        "First contact between {} and {}",
                        factions[i].name, factions[j].name
                    ),
                    relationship_changes: vec![
                        (factions[i].id.clone(), factions[j].id.clone(), 1),
                    ],
                };
                
                events.push(event);
            }
        }
        
        events
    }
    
    fn generate_random_event(&mut self, factions: &[ProceduralFaction], year: i32) -> Option<HistoricalEvent> {
        if factions.len() < 2 {
            return None;
        }
        
        // Select random factions
        let idx1 = self.rng.gen_range(0..factions.len());
        let mut idx2 = self.rng.gen_range(0..factions.len());
        while idx2 == idx1 {
            idx2 = self.rng.gen_range(0..factions.len());
        }
        
        let faction1 = &factions[idx1];
        let faction2 = &factions[idx2];
        
        // Get current relationship
        let relationship = faction1.relationships.get(&faction2.id)
            .copied()
            .unwrap_or(Relationship::Neutral);
        
        // Generate event based on relationship
        let event_type = self.choose_event_type(relationship);
        
        let (description, relationship_change) = match event_type {
            EventType::War => {
                (
                    format!("{} declares war on {}", faction1.name, faction2.name),
                    -3,
                )
            }
            EventType::PeaceTreaty => {
                (
                    format!("{} and {} sign peace treaty", faction1.name, faction2.name),
                    2,
                )
            }
            EventType::Alliance => {
                (
                    format!("{} and {} form alliance", faction1.name, faction2.name),
                    3,
                )
            }
            EventType::AllianceDissolved => {
                (
                    format!("Alliance between {} and {} dissolved", faction1.name, faction2.name),
                    -2,
                )
            }
            EventType::TradeAgreement => {
                (
                    format!("{} and {} sign trade agreement", faction1.name, faction2.name),
                    1,
                )
            }
            EventType::BorderDispute => {
                (
                    format!("Border dispute between {} and {}", faction1.name, faction2.name),
                    -1,
                )
            }
            EventType::TechnologyExchange => {
                (
                    format!("{} and {} exchange technology", faction1.name, faction2.name),
                    1,
                )
            }
            EventType::Incident => {
                (
                    format!("Diplomatic incident between {} and {}", faction1.name, faction2.name),
                    -1,
                )
            }
            EventType::CulturalExchange => {
                (
                    format!("{} and {} initiate cultural exchange", faction1.name, faction2.name),
                    1,
                )
            }
            EventType::FirstContact => {
                return None; // Already handled
            }
        };
        
        Some(HistoricalEvent {
            year,
            event_type,
            factions: vec![faction1.id.clone(), faction2.id.clone()],
            description,
            relationship_changes: vec![
                (faction1.id.clone(), faction2.id.clone(), relationship_change),
            ],
        })
    }
    
    fn choose_event_type(&mut self, relationship: Relationship) -> EventType {
        match relationship {
            Relationship::Allied => {
                // Allied factions might dissolve alliance or have incidents
                match self.rng.gen_range(0..10) {
                    0 => EventType::AllianceDissolved,
                    1 => EventType::Incident,
                    2..=5 => EventType::TradeAgreement,
                    6..=8 => EventType::TechnologyExchange,
                    _ => EventType::CulturalExchange,
                }
            }
            Relationship::Friendly => {
                // Friendly factions might form alliances or have positive interactions
                match self.rng.gen_range(0..10) {
                    0..=2 => EventType::Alliance,
                    3..=6 => EventType::TradeAgreement,
                    7..=8 => EventType::TechnologyExchange,
                    _ => EventType::CulturalExchange,
                }
            }
            Relationship::Neutral => {
                // Neutral factions can go either way
                match self.rng.gen_range(0..10) {
                    0 => EventType::BorderDispute,
                    1 => EventType::Incident,
                    2..=4 => EventType::TradeAgreement,
                    5..=7 => EventType::CulturalExchange,
                    _ => EventType::TechnologyExchange,
                }
            }
            Relationship::Unfriendly => {
                // Unfriendly factions might have disputes or wars
                match self.rng.gen_range(0..10) {
                    0..=3 => EventType::BorderDispute,
                    4..=6 => EventType::Incident,
                    7..=8 => EventType::War,
                    _ => EventType::TradeAgreement, // Rare positive event
                }
            }
            Relationship::Hostile => {
                // Hostile factions likely to have wars
                match self.rng.gen_range(0..10) {
                    0..=5 => EventType::War,
                    6..=8 => EventType::BorderDispute,
                    _ => EventType::Incident,
                }
            }
            Relationship::War => {
                // Already at war, might sign peace treaty
                match self.rng.gen_range(0..10) {
                    0..=2 => EventType::PeaceTreaty,
                    _ => EventType::War, // Continuing conflict
                }
            }
        }
    }
    
    /// Generate a timeline summary
    pub fn generate_timeline_summary(&self, events: &[HistoricalEvent], factions: &[ProceduralFaction]) -> String {
        let mut summary = String::new();
        
        summary.push_str("=== GALACTIC HISTORY TIMELINE ===\n\n");
        
        // Group events by decade
        let mut decades: std::collections::BTreeMap<i32, Vec<&HistoricalEvent>> = std::collections::BTreeMap::new();
        for event in events {
            let decade = (event.year / 10) * 10;
            decades.entry(decade).or_insert_with(Vec::new).push(event);
        }
        
        for (decade, events) in decades.iter() {
            summary.push_str(&format!("\n--- Year {} to {} ---\n", decade, decade + 9));
            
            for event in events {
                summary.push_str(&format!("  Year {}: {}\n", event.year, event.description));
            }
        }
        
        summary.push_str("\n=== PRESENT DAY ===\n");
        summary.push_str(&format!("Active Factions: {}\n", factions.len()));
        
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::factions::FactionGenerator;
    
    #[test]
    fn test_history_generation() {
        let mut faction_gen = FactionGenerator::new(42);
        let factions = faction_gen.generate_factions(3, &[]);
        
        let mut history_gen = HistoryGenerator::new(123);
        let history = history_gen.generate_history(&factions, 100);
        
        assert!(!history.is_empty());
    }
    
    #[test]
    fn test_first_contact_events() {
        let mut faction_gen = FactionGenerator::new(456);
        let factions = faction_gen.generate_factions(3, &[]);
        
        let mut history_gen = HistoryGenerator::new(789);
        let history = history_gen.generate_history(&factions, 100);
        
        // Should have first contact events for all faction pairs
        let first_contacts: Vec<_> = history.iter()
            .filter(|e| e.event_type == EventType::FirstContact)
            .collect();
        
        assert_eq!(first_contacts.len(), 3); // 3 factions = 3 pairs
    }
    
    #[test]
    fn test_chronological_order() {
        let mut faction_gen = FactionGenerator::new(321);
        let factions = faction_gen.generate_factions(2, &[]);
        
        let mut history_gen = HistoryGenerator::new(654);
        let history = history_gen.generate_history(&factions, 50);
        
        // Events should be in chronological order
        for i in 1..history.len() {
            assert!(history[i].year >= history[i - 1].year);
        }
    }
    
    #[test]
    fn test_timeline_summary() {
        let mut faction_gen = FactionGenerator::new(111);
        let factions = faction_gen.generate_factions(2, &[]);
        
        let mut history_gen = HistoryGenerator::new(222);
        let history = history_gen.generate_history(&factions, 50);
        
        let summary = history_gen.generate_timeline_summary(&history, &factions);
        
        assert!(summary.contains("GALACTIC HISTORY"));
        assert!(summary.contains("PRESENT DAY"));
    }
    
    #[test]
    fn test_event_types() {
        let mut faction_gen = FactionGenerator::new(333);
        let factions = faction_gen.generate_factions(4, &[]);
        
        let mut history_gen = HistoryGenerator::new(444);
        let history = history_gen.generate_history(&factions, 100);
        
        // Should have variety of event types
        let has_war = history.iter().any(|e| e.event_type == EventType::War);
        let has_alliance = history.iter().any(|e| e.event_type == EventType::Alliance);
        let has_trade = history.iter().any(|e| e.event_type == EventType::TradeAgreement);
        
        // With 100 years and 4 factions, should have some variety
        assert!(has_war || has_alliance || has_trade);
    }
}
