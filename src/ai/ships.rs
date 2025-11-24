//! AI behaviors for ships
//!
//! This module implements various AI behaviors for NPC ships including
//! combat, patrol, and trading behaviors.

use uuid::Uuid;
use serde::{Deserialize, Serialize};

use super::behavior_tree::{BehaviorContext, BehaviorNode, BehaviorStatus, Selector, Sequence, Condition, Action};

/// AI personality type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AIPersonality {
    /// Aggressive - attacks on sight
    Aggressive,
    /// Defensive - only attacks when threatened
    Defensive,
    /// Passive - avoids combat
    Passive,
    /// Trader - focused on trade routes
    Trader,
    /// Patrol - follows patrol routes
    Patrol,
}

/// Ship AI context for behavior tree execution
pub struct ShipAIContext {
    /// The ship being controlled
    pub ship_id: Uuid,
    /// Detected contacts nearby
    pub nearby_ships: Vec<Uuid>,
    /// Current target (if any)
    pub target: Option<Uuid>,
    /// AI personality
    pub personality: AIPersonality,
    /// Faction
    pub faction: String,
    /// Hostile factions
    pub hostile_factions: Vec<String>,
    /// Current waypoint (for patrol)
    pub current_waypoint: Option<[f64; 3]>,
    /// Patrol route
    pub patrol_route: Vec<[f64; 3]>,
    /// Current waypoint index
    pub waypoint_index: usize,
    /// Distance to target
    pub distance_to_target: Option<f64>,
    /// Ship is under attack
    pub under_attack: bool,
    /// Hull integrity (0.0 - 1.0)
    pub hull_integrity: f32,
    /// Shield strength (0.0 - 1.0)
    pub shield_strength: f32,
    /// Nearest station
    pub nearest_station: Option<Uuid>,
    /// Commands to execute this tick
    pub commands: Vec<AICommand>,
}

impl BehaviorContext for ShipAIContext {
    fn update(&mut self) {
        // Clear commands from previous tick
        self.commands.clear();
    }
}

/// Commands that AI can issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AICommand {
    /// Set target for weapons
    SetTarget { target_id: Uuid },
    /// Fire weapons
    FireWeapons,
    /// Move to position
    MoveTo { position: [f64; 3] },
    /// Engage FTL drive
    EngageFTL { destination: [f64; 3] },
    /// Raise shields
    RaiseShields,
    /// Request docking at station
    DockAtStation { station_id: Uuid },
    /// Evade (random evasive maneuvers)
    Evade,
}

impl ShipAIContext {
    pub fn new(ship_id: Uuid, faction: String) -> Self {
        Self {
            ship_id,
            nearby_ships: Vec::new(),
            target: None,
            personality: AIPersonality::Defensive,
            faction,
            hostile_factions: Vec::new(),
            current_waypoint: None,
            patrol_route: Vec::new(),
            waypoint_index: 0,
            distance_to_target: None,
            under_attack: false,
            hull_integrity: 1.0,
            shield_strength: 1.0,
            nearest_station: None,
            commands: Vec::new(),
        }
    }
    
    pub fn add_command(&mut self, command: AICommand) {
        self.commands.push(command);
    }
}

/// Combat AI builder
pub struct CombatAI;

impl CombatAI {
    /// Build aggressive combat behavior tree
    pub fn build_aggressive() -> Box<dyn BehaviorNode> {
        Box::new(Selector::new(vec![
            // If low on hull, retreat to station
            Box::new(Sequence::new(vec![
                Box::new(Condition::new(Self::is_low_hull)),
                Box::new(Action::new(Self::retreat_to_station)),
            ])),
            // If enemy nearby, engage
            Box::new(Sequence::new(vec![
                Box::new(Condition::new(Self::has_enemy_nearby)),
                Box::new(Action::new(Self::select_target)),
                Box::new(Action::new(Self::engage_target)),
            ])),
            // Otherwise patrol
            Box::new(Action::new(Self::idle_patrol)),
        ]))
    }
    
    /// Build defensive combat behavior tree
    pub fn build_defensive() -> Box<dyn BehaviorNode> {
        Box::new(Selector::new(vec![
            // If low on hull, retreat
            Box::new(Sequence::new(vec![
                Box::new(Condition::new(Self::is_low_hull)),
                Box::new(Action::new(Self::retreat_to_station)),
            ])),
            // If under attack, fight back
            Box::new(Sequence::new(vec![
                Box::new(Condition::new(Self::is_under_attack)),
                Box::new(Action::new(Self::select_nearest_attacker)),
                Box::new(Action::new(Self::engage_target)),
            ])),
            // Otherwise patrol
            Box::new(Action::new(Self::idle_patrol)),
        ]))
    }
    
    fn is_low_hull(ctx: &dyn BehaviorContext) -> bool {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_ref::<ShipAIContext>() {
            ship_ctx.hull_integrity < 0.3
        } else {
            false
        }
    }
    
    fn has_enemy_nearby(ctx: &dyn BehaviorContext) -> bool {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_ref::<ShipAIContext>() {
            !ship_ctx.nearby_ships.is_empty()
        } else {
            false
        }
    }
    
    fn is_under_attack(ctx: &dyn BehaviorContext) -> bool {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_ref::<ShipAIContext>() {
            ship_ctx.under_attack
        } else {
            false
        }
    }
    
    fn select_target(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            if let Some(target) = ship_ctx.nearby_ships.first() {
                ship_ctx.target = Some(*target);
                ship_ctx.add_command(AICommand::SetTarget { target_id: *target });
                BehaviorStatus::Success
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn select_nearest_attacker(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            // In a real implementation, we'd select the closest attacker
            if let Some(target) = ship_ctx.nearby_ships.first() {
                ship_ctx.target = Some(*target);
                ship_ctx.add_command(AICommand::SetTarget { target_id: *target });
                BehaviorStatus::Success
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn engage_target(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            ship_ctx.add_command(AICommand::RaiseShields);
            ship_ctx.add_command(AICommand::FireWeapons);
            BehaviorStatus::Success
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn retreat_to_station(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            if let Some(station_id) = ship_ctx.nearest_station {
                ship_ctx.add_command(AICommand::Evade);
                ship_ctx.add_command(AICommand::DockAtStation { station_id });
                BehaviorStatus::Success
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn idle_patrol(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            // Simple idle behavior - could be extended
            BehaviorStatus::Success
        } else {
            BehaviorStatus::Failure
        }
    }
}

/// Patrol AI builder
pub struct PatrolAI;

impl PatrolAI {
    /// Build patrol behavior tree
    pub fn build() -> Box<dyn BehaviorNode> {
        Box::new(Selector::new(vec![
            // If low on hull, retreat
            Box::new(Sequence::new(vec![
                Box::new(Condition::new(Self::is_low_hull)),
                Box::new(Action::new(Self::retreat_to_station)),
            ])),
            // If under attack and aggressive, fight
            Box::new(Sequence::new(vec![
                Box::new(Condition::new(Self::is_under_attack)),
                Box::new(Action::new(Self::defend)),
            ])),
            // Otherwise follow patrol route
            Box::new(Action::new(Self::follow_patrol_route)),
        ]))
    }
    
    fn is_low_hull(ctx: &dyn BehaviorContext) -> bool {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_ref::<ShipAIContext>() {
            ship_ctx.hull_integrity < 0.3
        } else {
            false
        }
    }
    
    fn is_under_attack(ctx: &dyn BehaviorContext) -> bool {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_ref::<ShipAIContext>() {
            ship_ctx.under_attack
        } else {
            false
        }
    }
    
    fn defend(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            if let Some(target) = ship_ctx.nearby_ships.first() {
                ship_ctx.add_command(AICommand::SetTarget { target_id: *target });
                ship_ctx.add_command(AICommand::RaiseShields);
                ship_ctx.add_command(AICommand::FireWeapons);
            }
            BehaviorStatus::Success
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn retreat_to_station(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            if let Some(station_id) = ship_ctx.nearest_station {
                ship_ctx.add_command(AICommand::Evade);
                ship_ctx.add_command(AICommand::DockAtStation { station_id });
                BehaviorStatus::Success
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn follow_patrol_route(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            if ship_ctx.patrol_route.is_empty() {
                return BehaviorStatus::Failure;
            }
            
            let waypoint = ship_ctx.patrol_route[ship_ctx.waypoint_index];
            ship_ctx.current_waypoint = Some(waypoint);
            ship_ctx.add_command(AICommand::MoveTo { position: waypoint });
            
            // In a real implementation, we'd check if we reached the waypoint
            // For now, just cycle through waypoints
            ship_ctx.waypoint_index = (ship_ctx.waypoint_index + 1) % ship_ctx.patrol_route.len();
            
            BehaviorStatus::Success
        } else {
            BehaviorStatus::Failure
        }
    }
}

/// Trading AI builder
pub struct TradingAI;

impl TradingAI {
    /// Build trading behavior tree
    pub fn build() -> Box<dyn BehaviorNode> {
        Box::new(Selector::new(vec![
            // If low on hull, retreat
            Box::new(Sequence::new(vec![
                Box::new(Condition::new(Self::is_low_hull)),
                Box::new(Action::new(Self::retreat_to_station)),
            ])),
            // If under attack, evade
            Box::new(Sequence::new(vec![
                Box::new(Condition::new(Self::is_under_attack)),
                Box::new(Action::new(Self::evade)),
            ])),
            // Otherwise follow trade route
            Box::new(Action::new(Self::follow_trade_route)),
        ]))
    }
    
    fn is_low_hull(ctx: &dyn BehaviorContext) -> bool {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_ref::<ShipAIContext>() {
            ship_ctx.hull_integrity < 0.5 // Traders are more cautious
        } else {
            false
        }
    }
    
    fn is_under_attack(ctx: &dyn BehaviorContext) -> bool {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_ref::<ShipAIContext>() {
            ship_ctx.under_attack
        } else {
            false
        }
    }
    
    fn evade(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            ship_ctx.add_command(AICommand::Evade);
            ship_ctx.add_command(AICommand::RaiseShields);
            BehaviorStatus::Success
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn retreat_to_station(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            if let Some(station_id) = ship_ctx.nearest_station {
                ship_ctx.add_command(AICommand::DockAtStation { station_id });
                BehaviorStatus::Success
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
    
    fn follow_trade_route(ctx: &mut dyn BehaviorContext) -> BehaviorStatus {
        use super::behavior_tree::BehaviorContextExt;
        if let Some(ship_ctx) = ctx.downcast_mut::<ShipAIContext>() {
            if let Some(station_id) = ship_ctx.nearest_station {
                ship_ctx.add_command(AICommand::DockAtStation { station_id });
                BehaviorStatus::Success
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ship_ai_context_creation() {
        let ship_id = Uuid::new_v4();
        let ctx = ShipAIContext::new(ship_id, "Federation".to_string());
        
        assert_eq!(ctx.ship_id, ship_id);
        assert_eq!(ctx.faction, "Federation");
        assert_eq!(ctx.personality, AIPersonality::Defensive);
        assert_eq!(ctx.hull_integrity, 1.0);
    }
    
    #[test]
    fn test_aggressive_combat_ai() {
        let ship_id = Uuid::new_v4();
        let mut ctx = ShipAIContext::new(ship_id, "Federation".to_string());
        ctx.nearby_ships.push(Uuid::new_v4());
        
        let mut ai = CombatAI::build_aggressive();
        let status = ai.tick(&mut ctx);
        
        assert_eq!(status, BehaviorStatus::Success);
        assert!(!ctx.commands.is_empty());
    }
    
    #[test]
    fn test_defensive_combat_ai_not_under_attack() {
        let ship_id = Uuid::new_v4();
        let mut ctx = ShipAIContext::new(ship_id, "Federation".to_string());
        
        let mut ai = CombatAI::build_defensive();
        let status = ai.tick(&mut ctx);
        
        assert_eq!(status, BehaviorStatus::Success);
    }
    
    #[test]
    fn test_defensive_combat_ai_under_attack() {
        let ship_id = Uuid::new_v4();
        let mut ctx = ShipAIContext::new(ship_id, "Federation".to_string());
        ctx.under_attack = true;
        ctx.nearby_ships.push(Uuid::new_v4());
        
        let mut ai = CombatAI::build_defensive();
        let status = ai.tick(&mut ctx);
        
        assert_eq!(status, BehaviorStatus::Success);
        // Should have commands to engage
        assert!(ctx.commands.iter().any(|cmd| matches!(cmd, AICommand::FireWeapons)));
    }
    
    #[test]
    fn test_patrol_ai() {
        let ship_id = Uuid::new_v4();
        let mut ctx = ShipAIContext::new(ship_id, "Federation".to_string());
        ctx.patrol_route = vec![
            [0.0, 0.0, 0.0],
            [100.0, 0.0, 0.0],
            [100.0, 100.0, 0.0],
        ];
        
        let mut ai = PatrolAI::build();
        let status = ai.tick(&mut ctx);
        
        assert_eq!(status, BehaviorStatus::Success);
        assert!(ctx.commands.iter().any(|cmd| matches!(cmd, AICommand::MoveTo { .. })));
    }
    
    #[test]
    fn test_trading_ai() {
        let ship_id = Uuid::new_v4();
        let mut ctx = ShipAIContext::new(ship_id, "Federation".to_string());
        ctx.nearest_station = Some(Uuid::new_v4());
        
        let mut ai = TradingAI::build();
        let status = ai.tick(&mut ctx);
        
        assert_eq!(status, BehaviorStatus::Success);
        assert!(ctx.commands.iter().any(|cmd| matches!(cmd, AICommand::DockAtStation { .. })));
    }
    
    #[test]
    fn test_retreat_when_low_hull() {
        let ship_id = Uuid::new_v4();
        let mut ctx = ShipAIContext::new(ship_id, "Federation".to_string());
        ctx.hull_integrity = 0.2; // Low hull
        ctx.nearest_station = Some(Uuid::new_v4());
        
        let mut ai = CombatAI::build_aggressive();
        let status = ai.tick(&mut ctx);
        
        assert_eq!(status, BehaviorStatus::Success);
        assert!(ctx.commands.iter().any(|cmd| matches!(cmd, AICommand::DockAtStation { .. })));
    }
}
