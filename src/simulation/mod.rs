//! Simulation module for the HYPERION game server.
//!
//! This module contains the ECS (Entity Component System) simulation logic,
//! including components for ships, modules, weapons, and other game entities,
//! as well as systems that operate on those components.

pub mod components;
pub mod systems;
pub mod physics;
pub mod r#loop;
pub mod module_state;

pub use components::*;
pub use systems::*;
pub use physics::*;
pub use r#loop::*;
pub use module_state::*;

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
        assert!(world.entities().len() == 0);
    }
}
