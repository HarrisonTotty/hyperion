//! HYPERION Library
//!
//! Core library for the HYPERION spaceship bridge simulation game.
//! Provides modules for configuration loading, simulation, API, and game logic.

pub mod config;
pub mod server;
pub mod simulation;
pub mod api;
pub mod models;
pub mod state;
pub mod blueprint;
pub mod compiler;
pub mod weapons;
pub mod events;
pub mod websocket;
pub mod event_broadcaster;
pub mod stations;
pub mod ai;
pub mod generation;

// Re-export commonly used items
pub use config::GameConfig;
pub use state::{GameWorld, SharedGameWorld};
