//! Simulation module for the HYPERION game server.
//!
//! This module contains the ECS (Entity Component System) simulation logic,
//! including components for ships, modules, weapons, and other game entities,
//! as well as systems that operate on those components.

pub mod components;
pub mod r#loop;
pub mod module_state;
pub mod physics;
pub mod systems;

pub use components::*;
pub use r#loop::*;
pub use module_state::*;
pub use physics::*;
pub use systems::*;

use bevy_ecs::world::World;

/// Initialize the simulation world.
///
/// Creates a new Bevy ECS World for managing game entities and components.
pub fn init_simulation() -> World {
    World::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_init() {
        let world = init_simulation();
        assert!(world.entities().is_empty());
    }
}
