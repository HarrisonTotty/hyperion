//! Event system for real-time updates to clients
//!
//! This module defines all game events that can be broadcast to clients via WebSocket.
//! Events are serialized to JSON and sent to subscribed clients based on their visibility
//! and permissions.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Types of events that can occur in the game
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameEvent {
    /// A ship has moved or updated its position
    ShipMoved {
        ship_id: Uuid,
        position: [f64; 3],
        velocity: [f64; 3],
        rotation: [f64; 4], // Quaternion
    },
    
    /// A weapon was fired
    WeaponFired {
        ship_id: Uuid,
        weapon_id: String,
        target_id: Option<Uuid>,
        weapon_type: String,
    },
    
    /// A ship took damage
    DamageTaken {
        ship_id: Uuid,
        damage_type: DamageType,
        amount: f32,
        hull_percent: f32,
        shield_percent: f32,
    },
    
    /// Shield status changed
    ShieldChanged {
        ship_id: Uuid,
        raised: bool,
        current: f32,
        max: f32,
    },
    
    /// Status effect applied to ship
    StatusEffectApplied {
        ship_id: Uuid,
        effect_type: String,
        strength: f32,
        duration: f32,
    },
    
    /// Status effect removed from ship
    StatusEffectRemoved {
        ship_id: Uuid,
        effect_type: String,
    },
    
    /// Module damaged or repaired
    ModuleStatusChanged {
        ship_id: Uuid,
        module_id: String,
        health_percent: f32,
        operational: bool,
    },
    
    /// Power allocation changed
    PowerAllocationChanged {
        ship_id: Uuid,
        allocations: Vec<ModuleAllocation>,
    },
    
    /// Cooling allocation changed
    CoolingAllocationChanged {
        ship_id: Uuid,
        allocations: Vec<ModuleAllocation>,
    },
    
    /// Communication message sent
    MessageSent {
        from_ship_id: Uuid,
        to_ship_id: Option<Uuid>, // None for broadcasts
        message: String,
    },
    
    /// Ship docked at station
    ShipDocked {
        ship_id: Uuid,
        station_id: Uuid,
    },
    
    /// Ship undocked from station
    ShipUndocked {
        ship_id: Uuid,
        station_id: Uuid,
    },
    
    /// New contact detected
    ContactDetected {
        detecting_ship_id: Uuid,
        contact_id: Uuid,
        contact_type: ContactType,
    },
    
    /// Contact lost
    ContactLost {
        detecting_ship_id: Uuid,
        contact_id: Uuid,
    },
    
    /// Ship destroyed
    ShipDestroyed {
        ship_id: Uuid,
        destroyed_by: Option<Uuid>,
    },
    
    /// Countermeasure activated
    CountermeasureActivated {
        ship_id: Uuid,
        countermeasure_type: String,
    },
    
    /// Point defense engaged
    PointDefenseEngaged {
        ship_id: Uuid,
        target_id: Uuid,
        success: bool,
    },
    
    /// FTL drive engaged
    FtlEngaged {
        ship_id: Uuid,
        drive_type: FtlDriveType,
        destination: Option<[f64; 3]>,
    },
    
    /// FTL drive disengaged
    FtlDisengaged {
        ship_id: Uuid,
        drive_type: FtlDriveType,
    },
    
    /// Simulation tick completed
    SimulationTick {
        tick: u64,
        time: f64,
    },
}

/// Type of damage dealt
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageType {
    Energy,
    Kinetic,
    Missile,
    Torpedo,
}

/// Module allocation info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleAllocation {
    pub module_id: String,
    pub amount: f32,
}

/// Type of contact detected
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactType {
    Ship,
    Station,
    Missile,
    Torpedo,
    Fighter,
    Unknown,
}

/// Type of FTL drive
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FtlDriveType {
    Warp,
    Jump,
}

/// Event queue for collecting events during simulation
#[derive(Debug, Default)]
pub struct EventQueue {
    events: Vec<GameEvent>,
}

impl EventQueue {
    /// Create a new empty event queue
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }
    
    /// Add an event to the queue
    pub fn push(&mut self, event: GameEvent) {
        self.events.push(event);
    }
    
    /// Get all events and clear the queue
    pub fn drain(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }
    
    /// Get the number of events in the queue
    pub fn len(&self) -> usize {
        self.events.len()
    }
    
    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_event_queue_push_and_drain() {
        let mut queue = EventQueue::new();
        assert!(queue.is_empty());
        
        let ship_id = Uuid::new_v4();
        queue.push(GameEvent::ShipMoved {
            ship_id,
            position: [1.0, 2.0, 3.0],
            velocity: [0.1, 0.2, 0.3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        });
        
        assert_eq!(queue.len(), 1);
        
        let events = queue.drain();
        assert_eq!(events.len(), 1);
        assert!(queue.is_empty());
    }
    
    #[test]
    fn test_event_serialization() {
        let ship_id = Uuid::new_v4();
        let event = GameEvent::WeaponFired {
            ship_id,
            weapon_id: "laser_1".to_string(),
            target_id: Some(Uuid::new_v4()),
            weapon_type: "energy".to_string(),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("weapon_fired"));
        assert!(json.contains("laser_1"));
    }
    
    #[test]
    fn test_damage_taken_event() {
        let ship_id = Uuid::new_v4();
        let event = GameEvent::DamageTaken {
            ship_id,
            damage_type: DamageType::Energy,
            amount: 50.0,
            hull_percent: 75.0,
            shield_percent: 50.0,
        };
        
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("damage_taken"));
        assert!(json.contains("energy"));
    }
    
    #[test]
    fn test_shield_changed_event() {
        let ship_id = Uuid::new_v4();
        let event = GameEvent::ShieldChanged {
            ship_id,
            raised: true,
            current: 100.0,
            max: 100.0,
        };
        
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("shield_changed"));
        assert!(json.contains("true"));
    }
    
    #[test]
    fn test_status_effect_events() {
        let ship_id = Uuid::new_v4();
        
        let applied = GameEvent::StatusEffectApplied {
            ship_id,
            effect_type: "Ion".to_string(),
            strength: 5.0,
            duration: 10.0,
        };
        
        let removed = GameEvent::StatusEffectRemoved {
            ship_id,
            effect_type: "Ion".to_string(),
        };
        
        let json1 = serde_json::to_string(&applied).unwrap();
        assert!(json1.contains("status_effect_applied"));
        
        let json2 = serde_json::to_string(&removed).unwrap();
        assert!(json2.contains("status_effect_removed"));
    }
    
    #[test]
    fn test_communication_events() {
        let from_ship = Uuid::new_v4();
        let to_ship = Uuid::new_v4();
        
        let event = GameEvent::MessageSent {
            from_ship_id: from_ship,
            to_ship_id: Some(to_ship),
            message: "Hello!".to_string(),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("message_sent"));
        assert!(json.contains("Hello!"));
    }
    
    #[test]
    fn test_contact_events() {
        let ship_id = Uuid::new_v4();
        let contact_id = Uuid::new_v4();
        
        let detected = GameEvent::ContactDetected {
            detecting_ship_id: ship_id,
            contact_id,
            contact_type: ContactType::Ship,
        };
        
        let lost = GameEvent::ContactLost {
            detecting_ship_id: ship_id,
            contact_id,
        };
        
        let json1 = serde_json::to_string(&detected).unwrap();
        assert!(json1.contains("contact_detected"));
        
        let json2 = serde_json::to_string(&lost).unwrap();
        assert!(json2.contains("contact_lost"));
    }
    
    #[test]
    fn test_ftl_events() {
        let ship_id = Uuid::new_v4();
        
        let engaged = GameEvent::FtlEngaged {
            ship_id,
            drive_type: FtlDriveType::Warp,
            destination: Some([100.0, 200.0, 300.0]),
        };
        
        let disengaged = GameEvent::FtlDisengaged {
            ship_id,
            drive_type: FtlDriveType::Jump,
        };
        
        let json1 = serde_json::to_string(&engaged).unwrap();
        assert!(json1.contains("ftl_engaged"));
        assert!(json1.contains("warp"));
        
        let json2 = serde_json::to_string(&disengaged).unwrap();
        assert!(json2.contains("ftl_disengaged"));
        assert!(json2.contains("jump"));
    }
}
