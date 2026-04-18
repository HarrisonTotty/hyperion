//! AI module for ship behaviors
//!
//! This module provides behavior tree-based AI for NPC ships, including
//! combat, patrol, and trading behaviors.

pub mod behavior_tree;
pub mod ships;
pub mod system;

pub use behavior_tree::{BehaviorContext, BehaviorNode, BehaviorStatus};
pub use ships::{AICommand, AIPersonality, CombatAI, PatrolAI, ShipAIContext, TradingAI};
pub use system::{AIContextUpdate, AIManager, ShipAI};
