//! AI system integration
//!
//! This module integrates AI behaviors with the game simulation.

use uuid::Uuid;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::behavior_tree::BehaviorNode;
use super::ships::{ShipAIContext, AIPersonality, AICommand, CombatAI, PatrolAI, TradingAI};

/// AI controller for a single ship
pub struct ShipAI {
    /// The behavior tree
    behavior_tree: Box<dyn BehaviorNode>,
    /// The AI context
    context: ShipAIContext,
}

impl ShipAI {
    /// Create a new AI controller
    pub fn new(ship_id: Uuid, faction: String, personality: AIPersonality) -> Self {
        let behavior_tree: Box<dyn BehaviorNode> = match personality {
            AIPersonality::Aggressive => CombatAI::build_aggressive(),
            AIPersonality::Defensive => CombatAI::build_defensive(),
            AIPersonality::Passive => CombatAI::build_defensive(), // Passive uses defensive tree
            AIPersonality::Trader => TradingAI::build(),
            AIPersonality::Patrol => PatrolAI::build(),
        };
        
        let mut context = ShipAIContext::new(ship_id, faction);
        context.personality = personality;
        
        Self {
            behavior_tree,
            context,
        }
    }
    
    /// Execute one AI tick
    pub fn tick(&mut self) -> Vec<AICommand> {
        use super::behavior_tree::BehaviorContext;
        self.context.update();
        self.behavior_tree.tick(&mut self.context);
        self.context.commands.clone()
    }
    
    /// Update context data from game world
    pub fn update_context(&mut self, update: AIContextUpdate) {
        self.context.nearby_ships = update.nearby_ships;
        self.context.under_attack = update.under_attack;
        self.context.hull_integrity = update.hull_integrity;
        self.context.shield_strength = update.shield_strength;
        self.context.nearest_station = update.nearest_station;
        
        if let Some(route) = update.patrol_route {
            self.context.patrol_route = route;
        }
    }
    
    /// Get the ship ID
    pub fn ship_id(&self) -> Uuid {
        self.context.ship_id
    }
    
    /// Get the AI personality
    pub fn personality(&self) -> AIPersonality {
        self.context.personality
    }
    
    /// Set patrol route
    pub fn set_patrol_route(&mut self, route: Vec<[f64; 3]>) {
        self.context.patrol_route = route;
    }
    
    /// Add hostile faction
    pub fn add_hostile_faction(&mut self, faction: String) {
        if !self.context.hostile_factions.contains(&faction) {
            self.context.hostile_factions.push(faction);
        }
    }
}

/// Update data for AI context
#[derive(Debug, Clone)]
pub struct AIContextUpdate {
    pub nearby_ships: Vec<Uuid>,
    pub under_attack: bool,
    pub hull_integrity: f32,
    pub shield_strength: f32,
    pub nearest_station: Option<Uuid>,
    pub patrol_route: Option<Vec<[f64; 3]>>,
}

impl Default for AIContextUpdate {
    fn default() -> Self {
        Self {
            nearby_ships: Vec::new(),
            under_attack: false,
            hull_integrity: 1.0,
            shield_strength: 1.0,
            nearest_station: None,
            patrol_route: None,
        }
    }
}

/// Manager for all AI-controlled ships
pub struct AIManager {
    /// Map of ship ID to AI controller
    ais: Arc<RwLock<HashMap<Uuid, ShipAI>>>,
}

impl AIManager {
    /// Create a new AI manager
    pub fn new() -> Self {
        Self {
            ais: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a new AI-controlled ship
    pub fn register_ship(&self, ship_id: Uuid, faction: String, personality: AIPersonality) {
        let mut ais = self.ais.write().unwrap();
        ais.insert(ship_id, ShipAI::new(ship_id, faction, personality));
    }
    
    /// Unregister an AI-controlled ship
    pub fn unregister_ship(&self, ship_id: Uuid) {
        let mut ais = self.ais.write().unwrap();
        ais.remove(&ship_id);
    }
    
    /// Update AI context for a ship
    pub fn update_ship_context(&self, ship_id: Uuid, update: AIContextUpdate) {
        let mut ais = self.ais.write().unwrap();
        if let Some(ai) = ais.get_mut(&ship_id) {
            ai.update_context(update);
        }
    }
    
    /// Run AI for all ships and return commands
    pub fn tick_all(&self) -> HashMap<Uuid, Vec<AICommand>> {
        let mut ais = self.ais.write().unwrap();
        let mut commands = HashMap::new();
        
        for (ship_id, ai) in ais.iter_mut() {
            let ship_commands = ai.tick();
            if !ship_commands.is_empty() {
                commands.insert(*ship_id, ship_commands);
            }
        }
        
        commands
    }
    
    /// Get all AI-controlled ship IDs
    pub fn get_ship_ids(&self) -> Vec<Uuid> {
        let ais = self.ais.read().unwrap();
        ais.keys().copied().collect()
    }
    
    /// Get ship AI personality
    pub fn get_personality(&self, ship_id: Uuid) -> Option<AIPersonality> {
        let ais = self.ais.read().unwrap();
        ais.get(&ship_id).map(|ai| ai.personality())
    }
    
    /// Set patrol route for a ship
    pub fn set_patrol_route(&self, ship_id: Uuid, route: Vec<[f64; 3]>) {
        let mut ais = self.ais.write().unwrap();
        if let Some(ai) = ais.get_mut(&ship_id) {
            ai.set_patrol_route(route);
        }
    }
    
    /// Add hostile faction for a ship
    pub fn add_hostile_faction(&self, ship_id: Uuid, faction: String) {
        let mut ais = self.ais.write().unwrap();
        if let Some(ai) = ais.get_mut(&ship_id) {
            ai.add_hostile_faction(faction);
        }
    }
}

impl Default for AIManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ship_ai_creation() {
        let ship_id = Uuid::new_v4();
        let ai = ShipAI::new(ship_id, "Federation".to_string(), AIPersonality::Aggressive);
        
        assert_eq!(ai.ship_id(), ship_id);
        assert_eq!(ai.personality(), AIPersonality::Aggressive);
    }
    
    #[test]
    fn test_ship_ai_tick() {
        let ship_id = Uuid::new_v4();
        let mut ai = ShipAI::new(ship_id, "Federation".to_string(), AIPersonality::Aggressive);
        
        // Add an enemy ship
        ai.update_context(AIContextUpdate {
            nearby_ships: vec![Uuid::new_v4()],
            ..Default::default()
        });
        
        let commands = ai.tick();
        
        // Aggressive AI should engage
        assert!(!commands.is_empty());
    }
    
    #[test]
    fn test_ai_manager_registration() {
        let manager = AIManager::new();
        let ship_id = Uuid::new_v4();
        
        manager.register_ship(ship_id, "Federation".to_string(), AIPersonality::Patrol);
        
        let ship_ids = manager.get_ship_ids();
        assert!(ship_ids.contains(&ship_id));
        
        manager.unregister_ship(ship_id);
        let ship_ids = manager.get_ship_ids();
        assert!(!ship_ids.contains(&ship_id));
    }
    
    #[test]
    fn test_ai_manager_tick() {
        let manager = AIManager::new();
        let ship_id = Uuid::new_v4();
        
        manager.register_ship(ship_id, "Federation".to_string(), AIPersonality::Aggressive);
        
        manager.update_ship_context(ship_id, AIContextUpdate {
            nearby_ships: vec![Uuid::new_v4()],
            ..Default::default()
        });
        
        let commands = manager.tick_all();
        
        assert!(commands.contains_key(&ship_id));
    }
    
    #[test]
    fn test_patrol_route() {
        let manager = AIManager::new();
        let ship_id = Uuid::new_v4();
        
        manager.register_ship(ship_id, "Federation".to_string(), AIPersonality::Patrol);
        
        let route = vec![
            [0.0, 0.0, 0.0],
            [100.0, 0.0, 0.0],
            [100.0, 100.0, 0.0],
        ];
        
        manager.set_patrol_route(ship_id, route);
        
        let commands = manager.tick_all();
        assert!(commands.contains_key(&ship_id));
    }
    
    #[test]
    fn test_hostile_factions() {
        let manager = AIManager::new();
        let ship_id = Uuid::new_v4();
        
        manager.register_ship(ship_id, "Federation".to_string(), AIPersonality::Defensive);
        manager.add_hostile_faction(ship_id, "Klingon".to_string());
        
        // In a real implementation, this would affect targeting decisions
        assert_eq!(manager.get_personality(ship_id), Some(AIPersonality::Defensive));
    }
}
