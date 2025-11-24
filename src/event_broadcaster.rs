//! Event broadcaster service
//!
//! This module provides a service that pulls events from GameWorld
//! and broadcasts them to WebSocket clients via the WebSocketManager.

use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time;

use crate::state::GameWorld;
use crate::websocket::WebSocketManager;
use crate::events::GameEvent;

/// Event broadcaster service
///
/// Periodically drains events from GameWorld and broadcasts them
/// to all connected WebSocket clients.
pub struct EventBroadcaster {
    game_world: Arc<RwLock<GameWorld>>,
    ws_manager: Arc<WebSocketManager>,
    interval: Duration,
}

impl EventBroadcaster {
    /// Create a new event broadcaster
    pub fn new(
        game_world: Arc<RwLock<GameWorld>>,
        ws_manager: Arc<WebSocketManager>,
    ) -> Self {
        Self {
            game_world,
            ws_manager,
            interval: Duration::from_millis(16), // ~60 fps
        }
    }
    
    /// Create with custom broadcast interval
    pub fn with_interval(
        game_world: Arc<RwLock<GameWorld>>,
        ws_manager: Arc<WebSocketManager>,
        interval: Duration,
    ) -> Self {
        Self {
            game_world,
            ws_manager,
            interval,
        }
    }
    
    /// Start the broadcaster service
    ///
    /// This runs in a background task and continuously drains events
    /// from GameWorld and broadcasts them to WebSocket clients.
    pub async fn run(self) {
        let mut interval = time::interval(self.interval);
        
        loop {
            interval.tick().await;
            
            // Drain events from GameWorld
            let events = {
                let mut world = self.game_world.write().unwrap();
                world.drain_events()
            };
            
            // Broadcast each event
            for event in events {
                self.ws_manager.broadcast(event);
            }
        }
    }
    
    /// Run for a limited number of iterations (useful for testing)
    pub async fn run_limited(self, iterations: usize) {
        let mut interval = time::interval(self.interval);
        
        for _ in 0..iterations {
            interval.tick().await;
            
            // Drain events from GameWorld
            let events = {
                let mut world = self.game_world.write().unwrap();
                world.drain_events()
            };
            
            // Broadcast each event
            for event in events {
                self.ws_manager.broadcast(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    
    #[tokio::test]
    async fn test_event_broadcaster_creation() {
        let game_world = Arc::new(RwLock::new(GameWorld::new()));
        let ws_manager = Arc::new(WebSocketManager::new());
        
        let _broadcaster = EventBroadcaster::new(game_world, ws_manager);
    }
    
    #[tokio::test]
    async fn test_event_broadcasting() {
        let game_world = Arc::new(RwLock::new(GameWorld::new()));
        let ws_manager = Arc::new(WebSocketManager::new());
        
        // Subscribe to events
        let mut rx = ws_manager.subscribe();
        
        // Add an event to GameWorld
        {
            let mut world = game_world.write().unwrap();
            world.push_event(GameEvent::SimulationTick {
                tick: 1,
                time: 0.016,
            });
        }
        
        // Create and run broadcaster for one iteration
        let broadcaster = EventBroadcaster::with_interval(
            game_world.clone(),
            ws_manager.clone(),
            Duration::from_millis(10),
        );
        
        // Run in background
        tokio::spawn(async move {
            broadcaster.run_limited(1).await;
        });
        
        // Wait for event
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Should receive the event
        let received = rx.try_recv();
        assert!(received.is_ok());
    }
    
    #[tokio::test]
    async fn test_multiple_events() {
        let game_world = Arc::new(RwLock::new(GameWorld::new()));
        let ws_manager = Arc::new(WebSocketManager::new());
        
        let mut rx = ws_manager.subscribe();
        
        // Add multiple events
        {
            let mut world = game_world.write().unwrap();
            let ship_id = Uuid::new_v4();
            
            world.push_event(GameEvent::ShipMoved {
                ship_id,
                position: [1.0, 2.0, 3.0],
                velocity: [0.1, 0.2, 0.3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            });
            
            world.push_event(GameEvent::WeaponFired {
                ship_id,
                weapon_id: "laser_1".to_string(),
                target_id: None,
                weapon_type: "energy".to_string(),
            });
        }
        
        let broadcaster = EventBroadcaster::with_interval(
            game_world.clone(),
            ws_manager.clone(),
            Duration::from_millis(10),
        );
        
        tokio::spawn(async move {
            broadcaster.run_limited(1).await;
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Should receive both events
        assert!(rx.try_recv().is_ok());
        assert!(rx.try_recv().is_ok());
    }
}
