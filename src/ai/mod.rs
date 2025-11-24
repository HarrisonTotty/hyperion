//! AI module for ship behaviors
//!
//! This module provides behavior tree-based AI for NPC ships, including
//! combat, patrol, and trading behaviors.

pub mod behavior_tree;
pub mod ships;
pub mod system;

pub use behavior_tree::{BehaviorContext, BehaviorNode, BehaviorStatus};
pub use ships::{AIPersonality, ShipAIContext, AICommand, CombatAI, PatrolAI, TradingAI};
pub use system::{ShipAI, AIManager, AIContextUpdate};
