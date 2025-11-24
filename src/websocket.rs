//! WebSocket support for real-time game updates
//!
//! This module provides WebSocket endpoints for clients to receive real-time
//! updates about game events. Clients can subscribe to specific ships or events
//! and receive filtered updates based on their permissions.

use rocket::{State, get};
use rocket::serde::json::Json;
use rocket_ws::{WebSocket, Channel, Message};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

use crate::events::GameEvent;
use crate::state::GameWorld;

/// WebSocket subscription request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubscriptionRequest {
    /// Subscribe to all events for a specific ship
    SubscribeToShip {
        ship_id: Uuid,
    },
    
    /// Subscribe to all events for a player's ships
    SubscribeToPlayer {
        player_id: Uuid,
    },
    
    /// Subscribe to all simulation events
    SubscribeToSimulation,
    
    /// Unsubscribe from a ship
    UnsubscribeFromShip {
        ship_id: Uuid,
    },
    
    /// Unsubscribe from player
    UnsubscribeFromPlayer {
        player_id: Uuid,
    },
    
    /// Authentication with player credentials
    Authenticate {
        player_id: Uuid,
        // In a real implementation, this would include a token or credentials
    },
}

/// WebSocket subscription response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubscriptionResponse {
    /// Subscription successful
    Subscribed {
        message: String,
    },
    
    /// Unsubscribed successfully
    Unsubscribed {
        message: String,
    },
    
    /// Authentication successful
    Authenticated {
        player_id: Uuid,
    },
    
    /// Error occurred
    Error {
        message: String,
    },
}

/// Client subscription state
#[derive(Debug, Clone)]
pub struct ClientSubscription {
    pub player_id: Option<Uuid>,
    pub ship_ids: Vec<Uuid>,
    pub subscribe_to_simulation: bool,
}

impl Default for ClientSubscription {
    fn default() -> Self {
        Self {
            player_id: None,
            ship_ids: Vec::new(),
            subscribe_to_simulation: false,
        }
    }
}

/// WebSocket connection manager
pub struct WebSocketManager {
    /// Event broadcaster
    event_tx: broadcast::Sender<GameEvent>,
    
    /// Connected clients and their subscriptions
    clients: Arc<RwLock<HashMap<String, ClientSubscription>>>,
}

impl WebSocketManager {
    /// Create a new WebSocket manager
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(1000);
        
        Self {
            event_tx,
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Get a new event receiver
    pub fn subscribe(&self) -> broadcast::Receiver<GameEvent> {
        self.event_tx.subscribe()
    }
    
    /// Broadcast an event to all subscribed clients
    pub fn broadcast(&self, event: GameEvent) {
        let _ = self.event_tx.send(event);
    }
    
    /// Register a new client
    pub fn register_client(&self, client_id: String) {
        let mut clients = self.clients.write().unwrap();
        clients.insert(client_id, ClientSubscription::default());
    }
    
    /// Unregister a client
    pub fn unregister_client(&self, client_id: &str) {
        let mut clients = self.clients.write().unwrap();
        clients.remove(client_id);
    }
    
    /// Update client subscription
    pub fn update_subscription(&self, client_id: &str, subscription: ClientSubscription) {
        let mut clients = self.clients.write().unwrap();
        clients.insert(client_id.to_string(), subscription);
    }
    
    /// Get client subscription
    pub fn get_subscription(&self, client_id: &str) -> Option<ClientSubscription> {
        let clients = self.clients.read().unwrap();
        clients.get(client_id).cloned()
    }
    
    /// Check if a client should receive an event
    pub fn should_receive_event(&self, client_id: &str, event: &GameEvent) -> bool {
        let subscription = match self.get_subscription(client_id) {
            Some(sub) => sub,
            None => return false,
        };
        
        // Always allow simulation ticks if subscribed
        if subscription.subscribe_to_simulation {
            if matches!(event, GameEvent::SimulationTick { .. }) {
                return true;
            }
        }
        
        // Check if event is related to subscribed ships
        let event_ship_id = match event {
            GameEvent::ShipMoved { ship_id, .. } => Some(*ship_id),
            GameEvent::WeaponFired { ship_id, .. } => Some(*ship_id),
            GameEvent::DamageTaken { ship_id, .. } => Some(*ship_id),
            GameEvent::ShieldChanged { ship_id, .. } => Some(*ship_id),
            GameEvent::StatusEffectApplied { ship_id, .. } => Some(*ship_id),
            GameEvent::StatusEffectRemoved { ship_id, .. } => Some(*ship_id),
            GameEvent::ModuleStatusChanged { ship_id, .. } => Some(*ship_id),
            GameEvent::PowerAllocationChanged { ship_id, .. } => Some(*ship_id),
            GameEvent::CoolingAllocationChanged { ship_id, .. } => Some(*ship_id),
            GameEvent::MessageSent { from_ship_id, .. } => Some(*from_ship_id),
            GameEvent::ShipDocked { ship_id, .. } => Some(*ship_id),
            GameEvent::ShipUndocked { ship_id, .. } => Some(*ship_id),
            GameEvent::ContactDetected { detecting_ship_id, .. } => Some(*detecting_ship_id),
            GameEvent::ContactLost { detecting_ship_id, .. } => Some(*detecting_ship_id),
            GameEvent::ShipDestroyed { ship_id, .. } => Some(*ship_id),
            GameEvent::CountermeasureActivated { ship_id, .. } => Some(*ship_id),
            GameEvent::PointDefenseEngaged { ship_id, .. } => Some(*ship_id),
            GameEvent::FtlEngaged { ship_id, .. } => Some(*ship_id),
            GameEvent::FtlDisengaged { ship_id, .. } => Some(*ship_id),
            GameEvent::SimulationTick { .. } => None,
        };
        
        if let Some(ship_id) = event_ship_id {
            subscription.ship_ids.contains(&ship_id)
        } else {
            subscription.subscribe_to_simulation
        }
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket endpoint handler
#[get("/ws")]
pub fn ws_handler(
    ws: WebSocket,
    game_world: &State<Arc<RwLock<GameWorld>>>,
    ws_manager: &State<Arc<WebSocketManager>>,
) -> Channel<'static> {
    let client_id = Uuid::new_v4().to_string();
    let ws_manager = ws_manager.inner().clone();
    let game_world = game_world.inner().clone();
    
    ws.channel(move |stream| {
        Box::pin(async move {
            use rocket_ws::stream::DuplexStream;
            use futures::{SinkExt, StreamExt};
            
            // Register client
            ws_manager.register_client(client_id.clone());
            
            // Subscribe to events
            let mut event_rx = ws_manager.subscribe();
            
            // Client subscription state
            let mut subscription = ClientSubscription::default();
            
            // Split stream for reading and writing
            let (mut sink, mut stream) = stream.split();
            
            loop {
                tokio::select! {
                    // Handle incoming messages from client
                    message = stream.next() => {
                        match message {
                            Some(Ok(msg)) => {
                                if let Message::Text(text) = msg {
                                    // Parse subscription request
                                    if let Ok(request) = serde_json::from_str::<SubscriptionRequest>(&text) {
                                        let response = handle_subscription_request(
                                            request,
                                            &mut subscription,
                                            &game_world,
                                        );
                                        
                                        // Update subscription in manager
                                        ws_manager.update_subscription(&client_id, subscription.clone());
                                        
                                        // Send response
                                        let response_json = serde_json::to_string(&response).unwrap();
                                        if sink.send(Message::Text(response_json)).await.is_err() {
                                            break;
                                        }
                                    }
                                } else if msg.is_close() {
                                    break;
                                }
                            }
                            Some(Err(_)) | None => break,
                        }
                    }
                    
                    // Handle events from broadcast channel
                    event = event_rx.recv() => {
                        match event {
                            Ok(game_event) => {
                                // Check if client should receive this event
                                if ws_manager.should_receive_event(&client_id, &game_event) {
                                    let event_json = serde_json::to_string(&game_event).unwrap();
                                    if sink.send(Message::Text(event_json)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                // Client lagged behind, send error
                                let error = SubscriptionResponse::Error {
                                    message: "Event stream lagged, some events may have been missed".to_string(),
                                };
                                let error_json = serde_json::to_string(&error).unwrap();
                                let _ = sink.send(Message::Text(error_json)).await;
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
            
            // Unregister client
            ws_manager.unregister_client(&client_id);
            
            Ok(())
        })
    })
}

/// Handle a subscription request
fn handle_subscription_request(
    request: SubscriptionRequest,
    subscription: &mut ClientSubscription,
    game_world: &Arc<RwLock<GameWorld>>,
) -> SubscriptionResponse {
    match request {
        SubscriptionRequest::SubscribeToShip { ship_id } => {
            // Verify ship exists
            let world = game_world.read().unwrap();
            if !world.ship_exists(ship_id) {
                return SubscriptionResponse::Error {
                    message: format!("Ship {} not found", ship_id),
                };
            }
            
            if !subscription.ship_ids.contains(&ship_id) {
                subscription.ship_ids.push(ship_id);
            }
            
            SubscriptionResponse::Subscribed {
                message: format!("Subscribed to ship {}", ship_id),
            }
        }
        
        SubscriptionRequest::SubscribeToPlayer { player_id } => {
            // Verify player exists and get their ships
            let world = game_world.read().unwrap();
            let player_id_str = player_id.to_string();
            if let Some(player) = world.get_player(&player_id_str) {
                subscription.player_id = Some(player_id);
                
                // Subscribe to all player's ships
                for ship_id in world.get_player_ships(player_id) {
                    if !subscription.ship_ids.contains(&ship_id) {
                        subscription.ship_ids.push(ship_id);
                    }
                }
                
                SubscriptionResponse::Subscribed {
                    message: format!("Subscribed to player {} and their ships", player.name),
                }
            } else {
                SubscriptionResponse::Error {
                    message: format!("Player {} not found", player_id),
                }
            }
        }
        
        SubscriptionRequest::SubscribeToSimulation => {
            subscription.subscribe_to_simulation = true;
            SubscriptionResponse::Subscribed {
                message: "Subscribed to simulation events".to_string(),
            }
        }
        
        SubscriptionRequest::UnsubscribeFromShip { ship_id } => {
            subscription.ship_ids.retain(|id| *id != ship_id);
            SubscriptionResponse::Unsubscribed {
                message: format!("Unsubscribed from ship {}", ship_id),
            }
        }
        
        SubscriptionRequest::UnsubscribeFromPlayer { player_id } => {
            if subscription.player_id == Some(player_id) {
                subscription.player_id = None;
                subscription.ship_ids.clear();
                SubscriptionResponse::Unsubscribed {
                    message: format!("Unsubscribed from player {}", player_id),
                }
            } else {
                SubscriptionResponse::Error {
                    message: "Not subscribed to this player".to_string(),
                }
            }
        }
        
        SubscriptionRequest::Authenticate { player_id } => {
            // In a real implementation, verify credentials here
            let world = game_world.read().unwrap();
            let player_id_str = player_id.to_string();
            if world.get_player(&player_id_str).is_some() {
                subscription.player_id = Some(player_id);
                SubscriptionResponse::Authenticated { player_id }
            } else {
                SubscriptionResponse::Error {
                    message: "Authentication failed".to_string(),
                }
            }
        }
    }
}

/// Get WebSocket connection info
#[derive(Debug, Serialize)]
pub struct WebSocketInfo {
    pub endpoint: String,
    pub connected_clients: usize,
}

/// Get WebSocket info endpoint
#[get("/ws/info")]
pub fn ws_info(ws_manager: &State<Arc<WebSocketManager>>) -> Json<WebSocketInfo> {
    let clients = ws_manager.clients.read().unwrap();
    Json(WebSocketInfo {
        endpoint: "/ws".to_string(),
        connected_clients: clients.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_websocket_manager_creation() {
        let manager = WebSocketManager::new();
        let clients = manager.clients.read().unwrap();
        assert_eq!(clients.len(), 0);
    }
    
    #[test]
    fn test_client_registration() {
        let manager = WebSocketManager::new();
        
        manager.register_client("client1".to_string());
        assert_eq!(manager.clients.read().unwrap().len(), 1);
        
        manager.register_client("client2".to_string());
        assert_eq!(manager.clients.read().unwrap().len(), 2);
        
        manager.unregister_client("client1");
        assert_eq!(manager.clients.read().unwrap().len(), 1);
    }
    
    #[test]
    fn test_subscription_update() {
        let manager = WebSocketManager::new();
        let client_id = "client1";
        
        manager.register_client(client_id.to_string());
        
        let ship_id = Uuid::new_v4();
        let mut subscription = ClientSubscription::default();
        subscription.ship_ids.push(ship_id);
        
        manager.update_subscription(client_id, subscription.clone());
        
        let retrieved = manager.get_subscription(client_id).unwrap();
        assert_eq!(retrieved.ship_ids.len(), 1);
        assert_eq!(retrieved.ship_ids[0], ship_id);
    }
    
    #[test]
    fn test_should_receive_event() {
        let manager = WebSocketManager::new();
        let client_id = "client1";
        let ship_id = Uuid::new_v4();
        
        manager.register_client(client_id.to_string());
        
        let mut subscription = ClientSubscription::default();
        subscription.ship_ids.push(ship_id);
        manager.update_subscription(client_id, subscription);
        
        // Should receive event for subscribed ship
        let event = GameEvent::ShipMoved {
            ship_id,
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        assert!(manager.should_receive_event(client_id, &event));
        
        // Should not receive event for other ship
        let other_ship_id = Uuid::new_v4();
        let other_event = GameEvent::ShipMoved {
            ship_id: other_ship_id,
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        assert!(!manager.should_receive_event(client_id, &other_event));
    }
    
    #[test]
    fn test_simulation_subscription() {
        let manager = WebSocketManager::new();
        let client_id = "client1";
        
        manager.register_client(client_id.to_string());
        
        let mut subscription = ClientSubscription::default();
        subscription.subscribe_to_simulation = true;
        manager.update_subscription(client_id, subscription);
        
        let event = GameEvent::SimulationTick {
            tick: 1,
            time: 0.016,
        };
        assert!(manager.should_receive_event(client_id, &event));
    }
    
    #[test]
    fn test_event_broadcast() {
        let manager = WebSocketManager::new();
        let mut rx = manager.subscribe();
        
        let ship_id = Uuid::new_v4();
        let event = GameEvent::WeaponFired {
            ship_id,
            weapon_id: "laser_1".to_string(),
            target_id: None,
            weapon_type: "energy".to_string(),
        };
        
        manager.broadcast(event.clone());
        
        // Should receive the broadcasted event
        let received = rx.try_recv().unwrap();
        match received {
            GameEvent::WeaponFired { weapon_id, .. } => {
                assert_eq!(weapon_id, "laser_1");
            }
            _ => panic!("Wrong event type"),
        }
    }
}
