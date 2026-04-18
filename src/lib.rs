//! HYPERION Library
//!
//! Core library for the HYPERION spaceship bridge simulation game.
//! Provides modules for configuration loading, simulation, API, and game logic.

pub mod ai;
pub mod api;
pub mod blueprint;
pub mod compiler;
pub mod config;
pub mod event_broadcaster;
pub mod events;
pub mod generation;
pub mod models;
pub mod server;
pub mod simulation;
pub mod state;
pub mod stations;
pub mod weapons;
pub mod websocket;

// Re-export commonly used items
pub use config::GameConfig;
pub use state::{GameWorld, SharedGameWorld};
