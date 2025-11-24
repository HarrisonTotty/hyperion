//! Simulation loop for the HYPERION game.
//!
//! This module implements the main simulation loop that executes all systems
//! in the correct order each tick.

use bevy_ecs::prelude::*;
use bevy_ecs::world::World;
use bevy_ecs::system::RunSystemOnce;

use super::components::*;
use super::systems::*;
use super::physics::*;
use super::module_state::*;

/// Simulation state tracking
#[derive(Debug, Clone)]
pub struct SimulationState {
    /// Current simulation tick
    pub tick: u64,
    /// Simulation time in seconds
    pub time: f64,
    /// Fixed timestep in seconds (default: 1/60 = ~0.0167s)
    pub timestep: f32,
    /// Whether simulation is paused
    pub paused: bool,
}

impl SimulationState {
    /// Create a new simulation state with default timestep (60 ticks/second)
    pub fn new() -> Self {
        Self {
            tick: 0,
            time: 0.0,
            timestep: 1.0 / 60.0,
            paused: false,
        }
    }
    
    /// Create with custom timestep
    pub fn with_timestep(timestep: f32) -> Self {
        Self {
            tick: 0,
            time: 0.0,
            timestep,
            paused: false,
        }
    }
    
    /// Advance the simulation by one tick
    pub fn advance(&mut self) {
        if !self.paused {
            self.tick += 1;
            self.time += self.timestep as f64;
        }
    }
    
    /// Pause the simulation
    pub fn pause(&mut self) {
        self.paused = true;
    }
    
    /// Resume the simulation
    pub fn resume(&mut self) {
        self.paused = false;
    }
    
    /// Reset the simulation
    pub fn reset(&mut self) {
        self.tick = 0;
        self.time = 0.0;
        self.paused = false;
    }
}

impl Default for SimulationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Executes all systems in the correct order for one simulation tick.
///
/// Systems are organized into phases to ensure proper execution order:
/// 1. Physics forces (engines, drag)
/// 2. Physics integration (F=ma)
/// 3. Movement (position updates)
/// 4. Weapon systems (cooldown, firing, projectiles)
/// 5. Combat (damage, beams, countermeasures)
/// 6. Ship systems (power, cooling, shields)
/// 7. Status effects (decay/update)
/// 8. FTL (warp, jump)
/// 9. Communication & scanning
/// 10. Collision detection, repair, explosions, momentum
pub fn run_simulation_tick(world: &mut World, delta_time: f32) {
    // Phase 1: Physics Forces
    // Apply engine thrust and drag forces
    world.run_system_once(engine_force_system);
    world.run_system_once(drag_force_system);
    
    // Phase 2: Physics Integration
    // Apply forces to update velocity (F=ma)
    world.run_system_once(
        move |query: Query<(&mut Transform, &mut ForceAccumulator, &ShipData, &StatusEffects)>| {
            physics_integration_system(query, delta_time);
        }
    );
    
    // Phase 3: Movement
    // Update positions based on velocity
    world.run_system_once(
        move |query: Query<&mut Transform>| {
            movement_system(query, delta_time);
        }
    );
    
    // Phase 4: Weapon Systems
    // Update weapon cooldowns
    world.run_system_once(
        move |query: Query<&mut WeaponComponent>| {
            weapon_cooldown_system(query, delta_time);
        }
    );
    
    // Handle weapon firing
    world.run_system_once(weapon_fire_system);
    
    // Update projectile positions
    world.run_system_once(
        move |commands: Commands, query: Query<(Entity, &mut ProjectileComponent)>| {
            projectile_system(commands, query, delta_time);
        }
    );
    
    // Phase 5: Combat
    // Apply damage from projectile hits
    world.run_system_once(
        |commands: Commands,
         projectiles: Query<(Entity, &ProjectileComponent, &Transform)>,
         ships: Query<(Entity, &mut ShipData, &mut ShieldComponent, &Transform)>,
         status_effects: Query<&mut StatusEffects>| {
            damage_system(commands, projectiles, ships, status_effects);
        }
    );
    
    // Apply damage from beam weapons
    world.run_system_once(
        move |beams: Query<(&ProjectileComponent, &Transform)>,
         ships: Query<(Entity, &mut ShipData, &mut ShieldComponent, &Transform)>,
         status_effects: Query<&mut StatusEffects>| {
            beam_weapon_system(beams, ships, status_effects, delta_time);
        }
    );
    
    // Handle countermeasures vs projectiles  
    world.run_system_once(
        |commands: Commands,
         countermeasures: Query<(&WeaponComponent, &Transform), With<PointDefenseMarker>>,
         projectiles: Query<(Entity, &ProjectileComponent, &Transform)>| {
            countermeasure_system(commands, countermeasures, projectiles);
        }
    );
    
    // Phase 6: Ship Systems
    // Update power generation/consumption
    world.run_system_once(power_system);
    
    // Update heat generation/dissipation
    world.run_system_once(cooling_system);
    
    // Update module states (power/cooling allocation, heat, efficiency)
    world.run_system_once(
        move |query: Query<(&mut ModuleStateTracker, &PowerGrid, &CoolingSystem, &ShipData)>| {
            module_state_system(query, delta_time);
        }
    );
    
    // Propagate module damage to ship capabilities
    world.run_system_once(module_damage_propagation_system);
    
    // Regenerate shields
    world.run_system_once(
        move |query: Query<(&mut ShieldComponent, &PowerGrid)>| {
            shield_system(query, delta_time);
        }
    );
    
    // Phase 7: Status Effects
    // Update and decay status effects
    world.run_system_once(
        move |query: Query<&mut StatusEffects>| {
            status_effect_system(query, delta_time);
        }
    );
    
    // Phase 8: FTL Systems
    // Handle warp drive (disabled by Tachyon)
    world.run_system_once(
        move |query: Query<(&mut WarpDriveComponent, &mut Transform, &StatusEffects)>| {
            warp_system(query, delta_time);
        }
    );
    
    // Handle jump drives (disabled by Tachyon)
    world.run_system_once(
        move |query: Query<(&mut JumpDriveComponent, &mut Transform, &StatusEffects)>| {
            jump_system(query, delta_time);
        }
    );
    
    // Phase 9: Communication & Scanning
    world.run_system_once(communication_system);
    world.run_system_once(scanning_system);
    
    // Phase 10: Collision & Cleanup
    world.run_system_once(
        |ships: Query<(Entity, &Transform, &CollisionShape), With<ShipData>>,
         projectiles: Query<(Entity, &Transform, &CollisionShape), With<ProjectileComponent>>| {
            collision_detection_system(ships, projectiles);
        }
    );
    
    // Repair damaged ships
    world.run_system_once(repair_system);
    world.run_system_once(explosion_system);
    world.run_system_once(
        |ships: Query<(&mut Transform, &ShipData, &StatusEffects), Without<ProjectileComponent>>,
         projectiles: Query<(&Transform, &ProjectileComponent)>| {
            impact_momentum_system(ships, projectiles);
        }
    );
}

/// Main simulation loop.
///
/// This runs the simulation continuously, executing all systems each tick.
/// In a real game server, this would run in a separate thread.
///
/// # Arguments
/// * `world` - The ECS world
/// * `state` - Simulation state (time, tick counter, etc.)
/// * `max_ticks` - Maximum number of ticks to run (None = infinite)
///
/// # Returns
/// The number of ticks executed
pub fn run_simulation(world: &mut World, state: &mut SimulationState, max_ticks: Option<u64>) -> u64 {
    let mut ticks_executed = 0;
    
    loop {
        // Check if we should stop
        if let Some(max) = max_ticks {
            if ticks_executed >= max {
                break;
            }
        }
        
        // Skip tick if paused, but still count iterations
        if state.paused {
            ticks_executed += 1;
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
            continue;
        }
        
        // Run one simulation tick
        run_simulation_tick(world, state.timestep);
        
        // Advance simulation state
        state.advance();
        ticks_executed += 1;
        
        // In a real implementation, we'd use a proper timer here
        // For testing, we just run the specified number of ticks
    }
    
    ticks_executed
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;

    #[test]
    fn test_simulation_state_creation() {
        let state = SimulationState::new();
        assert_eq!(state.tick, 0);
        assert_eq!(state.time, 0.0);
        assert!((state.timestep - 1.0/60.0).abs() < 0.001);
        assert!(!state.paused);
    }

    #[test]
    fn test_simulation_state_advance() {
        let mut state = SimulationState::new();
        
        state.advance();
        assert_eq!(state.tick, 1);
        assert!((state.time - 1.0/60.0).abs() < 0.001);
        
        state.advance();
        assert_eq!(state.tick, 2);
        assert!((state.time - 2.0/60.0).abs() < 0.001);
    }

    #[test]
    fn test_simulation_state_pause() {
        let mut state = SimulationState::new();
        
        state.pause();
        assert!(state.paused);
        
        state.advance();
        assert_eq!(state.tick, 0); // Didn't advance when paused
        
        state.resume();
        assert!(!state.paused);
        
        state.advance();
        assert_eq!(state.tick, 1); // Advanced after resume
    }

    #[test]
    fn test_simulation_state_reset() {
        let mut state = SimulationState::new();
        
        state.advance();
        state.advance();
        state.pause();
        
        state.reset();
        assert_eq!(state.tick, 0);
        assert_eq!(state.time, 0.0);
        assert!(!state.paused);
    }

    #[test]
    fn test_simulation_state_custom_timestep() {
        let state = SimulationState::with_timestep(0.1);
        assert_eq!(state.timestep, 0.1);
    }

    #[test]
    fn test_run_simulation_tick() {
        let mut world = World::new();
        
        // Create a simple ship with transform
        world.spawn((
            Transform {
                position: Vector3::new(0.0, 0.0, 0.0),
                rotation: nalgebra::UnitQuaternion::identity(),
                velocity: Vector3::new(10.0, 0.0, 0.0),
                angular_velocity: Vector3::zeros(),
            },
            ShipData::new(
                "ship1".to_string(),
                "Test Ship".to_string(),
                "battleship".to_string(),
                "team1".to_string(),
                1000.0,
                500.0,
                1000.0,
            ),
            ForceAccumulator::new(),
            StatusEffects::new(),
            PowerGrid::new(100.0, 200.0),
            ShieldComponent::new(100.0, 10.0, 5.0),
        ));
        
        // Run one tick with 1/60 second timestep
        run_simulation_tick(&mut world, 1.0/60.0);
        
        // Verify simulation ran (we can check that systems executed)
        // In a real test, we'd verify specific state changes
    }

    #[test]
    fn test_run_simulation_limited_ticks() {
        let mut world = World::new();
        let mut state = SimulationState::new();
        
        // Run exactly 10 ticks
        let ticks = run_simulation(&mut world, &mut state, Some(10));
        
        assert_eq!(ticks, 10);
        assert_eq!(state.tick, 10);
    }

    #[test]
    fn test_run_simulation_with_pause() {
        let mut world = World::new();
        let mut state = SimulationState::new();
        state.pause();
        
        // This would normally run forever when paused, but we limit it
        // In real code, paused simulation would wait for external events
        let ticks = run_simulation(&mut world, &mut state, Some(5));
        
        assert_eq!(ticks, 5); // Loop ran 5 times
        assert_eq!(state.tick, 0); // But simulation didn't advance
    }

    #[test]
    fn test_simulation_integration() {
        let mut world = World::new();
        let mut state = SimulationState::with_timestep(1.0); // 1 second per tick for easy math
        
        // Create a ship moving in the +X direction
        let ship = world.spawn((
            Transform {
                position: Vector3::new(0.0, 0.0, 0.0),
                rotation: nalgebra::UnitQuaternion::identity(),
                velocity: Vector3::new(10.0, 0.0, 0.0), // 10 m/s
                angular_velocity: Vector3::zeros(),
            },
            ShipData::new(
                "ship1".to_string(),
                "Test Ship".to_string(),
                "battleship".to_string(),
                "team1".to_string(),
                1000.0,
                500.0,
                1000.0,
            ),
            ForceAccumulator::new(),
            StatusEffects::new(),
            PowerGrid::new(100.0, 200.0),
            ShieldComponent::new(100.0, 10.0, 5.0),
        )).id();
        
        // Run 5 ticks (5 seconds at 1 second per tick)
        run_simulation(&mut world, &mut state, Some(5));
        
        // Ship should have moved 50 meters in X direction (10 m/s * 5 s)
        let transform = world.get::<Transform>(ship).unwrap();
        assert!((transform.position.x - 50.0).abs() < 1.0); // Allow some tolerance
    }
}
