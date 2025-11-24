//! Physics simulation for the HYPERION game.
//!
//! This module implements realistic 3D physics including forces, collisions,
//! and integration with game mechanics like weapon effects and ship systems.

use bevy_ecs::prelude::*;
use nalgebra::Vector3;

use super::components::*;
use crate::weapons::StatusEffectType;

/// Physics constants
pub mod constants {
    /// Speed of light (m/s) - used for warp calculations
    pub const SPEED_OF_LIGHT: f32 = 299_792_458.0;
    
    /// Space drag coefficient (very low in vacuum)
    pub const SPACE_DRAG: f32 = 0.0001;
    
    /// Graviton weight multiplier (doubles effective weight)
    pub const GRAVITON_WEIGHT_MULTIPLIER: f32 = 2.0;
    
    /// Default collision radius for ships (meters)
    pub const DEFAULT_SHIP_RADIUS: f32 = 50.0;
    
    /// Default collision radius for projectiles (meters)
    pub const DEFAULT_PROJECTILE_RADIUS: f32 = 1.0;
}

/// Physics forces that can be applied to entities
#[derive(Debug, Clone)]
pub struct Force {
    /// Direction and magnitude of force (Newtons)
    pub vector: Vector3<f32>,
    /// Point of application (for torque calculations)
    pub application_point: Option<Vector3<f32>>,
}

impl Force {
    /// Create a new force
    pub fn new(vector: Vector3<f32>) -> Self {
        Self {
            vector,
            application_point: None,
        }
    }
    
    /// Create a force at a specific point (for torque)
    pub fn at_point(vector: Vector3<f32>, point: Vector3<f32>) -> Self {
        Self {
            vector,
            application_point: Some(point),
        }
    }
}

/// Component to accumulate forces on an entity
#[derive(Component, Debug, Clone, Default)]
pub struct ForceAccumulator {
    /// Accumulated forces this frame
    pub forces: Vec<Force>,
}

impl ForceAccumulator {
    /// Create a new force accumulator
    pub fn new() -> Self {
        Self {
            forces: Vec::new(),
        }
    }
    
    /// Add a force
    pub fn add_force(&mut self, force: Force) {
        self.forces.push(force);
    }
    
    /// Clear all forces (called after integration)
    pub fn clear(&mut self) {
        self.forces.clear();
    }
    
    /// Get total force vector
    pub fn total_force(&self) -> Vector3<f32> {
        self.forces.iter().map(|f| f.vector).sum()
    }
}

/// Collision shape for physics
#[derive(Component, Debug, Clone)]
pub struct CollisionShape {
    /// Radius for sphere collision (simplified collision detection)
    pub radius: f32,
    /// Whether entity can collide with others
    pub enabled: bool,
}

impl CollisionShape {
    /// Create a new collision shape
    pub fn sphere(radius: f32) -> Self {
        Self {
            radius,
            enabled: true,
        }
    }
    
    /// Create a ship collision shape
    pub fn ship() -> Self {
        Self::sphere(constants::DEFAULT_SHIP_RADIUS)
    }
    
    /// Create a projectile collision shape
    pub fn projectile() -> Self {
        Self::sphere(constants::DEFAULT_PROJECTILE_RADIUS)
    }
}

/// Calculate effective weight considering status effects
pub fn calculate_effective_weight(base_weight: f32, effects: &StatusEffects) -> f32 {
    if effects.has_effect(StatusEffectType::GravitonWeight) {
        base_weight * constants::GRAVITON_WEIGHT_MULTIPLIER
    } else {
        base_weight
    }
}

/// Apply thrust force from engines
///
/// Engines provide thrust in the forward direction of the ship.
pub fn apply_engine_thrust(
    transform: &Transform,
    thrust: f32,
    accumulator: &mut ForceAccumulator,
) {
    // Thrust is applied in the forward direction (local Z axis)
    let forward = transform.rotation * Vector3::new(0.0, 0.0, 1.0);
    let force = Force::new(forward * thrust);
    accumulator.add_force(force);
}

/// Apply drag force
///
/// Drag opposes velocity and is proportional to velocity squared.
pub fn apply_drag(
    velocity: Vector3<f32>,
    drag_coefficient: f32,
    accumulator: &mut ForceAccumulator,
) {
    let speed = velocity.magnitude();
    if speed > 0.01 {
        let drag_magnitude = drag_coefficient * speed * speed;
        let drag_direction = -velocity.normalize();
        let force = Force::new(drag_direction * drag_magnitude);
        accumulator.add_force(force);
    }
}

/// System that applies forces from engines
///
/// This system reads engine module data and applies thrust forces.
pub fn engine_force_system(
    mut query: Query<(&Transform, &mut ForceAccumulator, &ModuleComponent)>,
) {
    for (transform, mut accumulator, module) in query.iter_mut() {
        // Only apply thrust for engine modules that are operational
        if module.kind.as_ref().map(|k| k.as_str()) == Some("engine") && module.is_operational() {
            // Calculate thrust based on efficiency and power allocation
            let max_thrust = 10000.0; // TODO: Get from configuration
            let thrust = max_thrust * module.efficiency * module.power_allocation;
            
            if thrust > 0.0 {
                apply_engine_thrust(transform, thrust, &mut accumulator);
            }
        }
    }
}

/// System that applies drag forces
///
/// Space has minimal drag, but we apply a small amount for gameplay.
pub fn drag_force_system(
    mut query: Query<(&Transform, &mut ForceAccumulator)>,
) {
    for (transform, mut accumulator) in query.iter_mut() {
        apply_drag(
            transform.velocity,
            constants::SPACE_DRAG,
            &mut accumulator,
        );
    }
}

/// System that integrates forces to update velocities
///
/// This system applies F = ma to update velocities based on accumulated forces.
pub fn physics_integration_system(
    mut query: Query<(
        &mut Transform,
        &mut ForceAccumulator,
        &ShipData,
        &StatusEffects,
    )>,
    delta_time: f32,
) {
    for (mut transform, mut accumulator, ship_data, effects) in query.iter_mut() {
        // Calculate effective mass
        let effective_weight = calculate_effective_weight(ship_data.base_weight, effects);
        let mass = effective_weight; // In space, weight ~= mass
        
        if mass > 0.0 {
            // Apply F = ma to get acceleration
            let total_force = accumulator.total_force();
            let acceleration = total_force / mass;
            
            // Update velocity: v = v + a * dt
            transform.velocity += acceleration * delta_time;
            
            // Clear forces for next frame
            accumulator.clear();
        }
    }
}

/// Check if two spheres are colliding
pub fn check_sphere_collision(
    pos1: Vector3<f32>,
    radius1: f32,
    pos2: Vector3<f32>,
    radius2: f32,
) -> bool {
    let distance = (pos1 - pos2).magnitude();
    distance < (radius1 + radius2)
}

/// System that detects collisions between entities
///
/// This system performs broad-phase collision detection and generates
/// collision events for narrow-phase processing.
pub fn collision_detection_system(
    ships: Query<(Entity, &Transform, &CollisionShape), With<ShipData>>,
    _projectiles: Query<(Entity, &Transform, &CollisionShape), With<ProjectileComponent>>,
) {
    // Ship-to-ship collisions
    let ship_vec: Vec<_> = ships.iter().collect();
    for i in 0..ship_vec.len() {
        for j in (i + 1)..ship_vec.len() {
            let (_entity1, transform1, shape1) = ship_vec[i];
            let (_entity2, transform2, shape2) = ship_vec[j];
            
            if shape1.enabled && shape2.enabled {
                if check_sphere_collision(
                    transform1.position,
                    shape1.radius,
                    transform2.position,
                    shape2.radius,
                ) {
                    // TODO: Emit collision event
                    // For now, we just detect the collision
                }
            }
        }
    }
    
    // Projectile-to-ship collisions are handled by damage_system
    // This is here for completeness and future expansion
}

/// System that applies momentum from weapon impacts
///
/// When a projectile hits a ship, it transfers momentum.
pub fn impact_momentum_system(
    mut ships: Query<(&mut Transform, &ShipData, &StatusEffects), Without<ProjectileComponent>>,
    projectiles: Query<(&Transform, &ProjectileComponent)>,
) {
    // TODO: This would be triggered by collision events
    // For now, it's a placeholder showing the concept
    
    for (_ship_transform, ship_data, effects) in ships.iter_mut() {
        for (projectile_transform, _projectile) in projectiles.iter() {
            // If projectile hit ship (detected by damage system)
            // Calculate momentum transfer
            let projectile_mass = 1.0; // kg
            let momentum_transfer = projectile_transform.velocity * projectile_mass;
            
            // Apply to ship
            let effective_weight = calculate_effective_weight(ship_data.base_weight, effects);
            if effective_weight > 0.0 {
                let _velocity_change = momentum_transfer / effective_weight;
                // This would be applied on actual collision
                // ship_transform.velocity += velocity_change;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;
    use bevy_ecs::system::RunSystemOnce;

    #[test]
    fn test_calculate_effective_weight() {
        let mut effects = StatusEffects::new();
        let base_weight = 1000.0;
        
        // No effects
        let weight = calculate_effective_weight(base_weight, &effects);
        assert_eq!(weight, 1000.0);
        
        // With Graviton effect
        effects.apply(StatusEffectType::GravitonWeight, 5.0);
        let weight = calculate_effective_weight(base_weight, &effects);
        assert_eq!(weight, 2000.0); // Doubled
    }

    #[test]
    fn test_apply_engine_thrust() {
        let transform = Transform {
            position: Vector3::zeros(),
            rotation: nalgebra::UnitQuaternion::identity(),
            velocity: Vector3::zeros(),
            angular_velocity: Vector3::zeros(),
        };
        
        let mut accumulator = ForceAccumulator::new();
        
        apply_engine_thrust(&transform, 1000.0, &mut accumulator);
        
        let total_force = accumulator.total_force();
        assert!((total_force.z - 1000.0).abs() < 0.01); // Force in Z direction
    }

    #[test]
    fn test_apply_drag() {
        let velocity = Vector3::new(100.0, 0.0, 0.0);
        let mut accumulator = ForceAccumulator::new();
        
        apply_drag(velocity, constants::SPACE_DRAG, &mut accumulator);
        
        let total_force = accumulator.total_force();
        assert!(total_force.x < 0.0); // Drag opposes velocity
    }

    #[test]
    fn test_check_sphere_collision() {
        let pos1 = Vector3::new(0.0, 0.0, 0.0);
        let pos2 = Vector3::new(10.0, 0.0, 0.0);
        
        // Colliding (overlap)
        assert!(check_sphere_collision(pos1, 6.0, pos2, 6.0));
        
        // Not colliding (gap between)
        assert!(!check_sphere_collision(pos1, 4.0, pos2, 4.0));
        
        // Just touching (distance = sum of radii, considered touching but not colliding)
        assert!(!check_sphere_collision(pos1, 5.0, pos2, 5.0));
        
        // Clearly colliding (one inside the other)
        assert!(check_sphere_collision(pos1, 10.0, pos2, 5.0));
    }

    #[test]
    fn test_force_accumulator() {
        let mut accumulator = ForceAccumulator::new();
        
        accumulator.add_force(Force::new(Vector3::new(10.0, 0.0, 0.0)));
        accumulator.add_force(Force::new(Vector3::new(0.0, 20.0, 0.0)));
        
        let total = accumulator.total_force();
        assert_eq!(total.x, 10.0);
        assert_eq!(total.y, 20.0);
        assert_eq!(total.z, 0.0);
        
        accumulator.clear();
        assert_eq!(accumulator.forces.len(), 0);
    }

    #[test]
    fn test_physics_integration_basic() {
        let mut world = World::new();
        
        let ship = world.spawn((
            Transform {
                position: Vector3::zeros(),
                rotation: nalgebra::UnitQuaternion::identity(),
                velocity: Vector3::zeros(),
                angular_velocity: Vector3::zeros(),
            },
            ForceAccumulator::new(),
            ShipData::new(
                "ship1".to_string(),
                "Test Ship".to_string(),
                "battleship".to_string(),
                "team1".to_string(),
                1000.0,
                500.0,
                1000.0, // 1000 kg
            ),
            StatusEffects::new(),
        )).id();
        
        // Apply a force
        world.get_mut::<ForceAccumulator>(ship).unwrap()
            .add_force(Force::new(Vector3::new(1000.0, 0.0, 0.0))); // 1000 N
        
        // Run integration with 1 second
        world.run_system_once(
            |query: Query<(&mut Transform, &mut ForceAccumulator, &ShipData, &StatusEffects)>| {
                physics_integration_system(query, 1.0);
            }
        );
        
        let transform = world.get::<Transform>(ship).unwrap();
        // F = ma, a = F/m = 1000/1000 = 1 m/s²
        // v = v0 + at = 0 + 1*1 = 1 m/s
        assert!((transform.velocity.x - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_physics_integration_with_graviton() {
        let mut world = World::new();
        
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffectType::GravitonWeight, 10.0);
        
        let ship = world.spawn((
            Transform {
                position: Vector3::zeros(),
                rotation: nalgebra::UnitQuaternion::identity(),
                velocity: Vector3::zeros(),
                angular_velocity: Vector3::zeros(),
            },
            ForceAccumulator::new(),
            ShipData::new(
                "ship1".to_string(),
                "Test Ship".to_string(),
                "battleship".to_string(),
                "team1".to_string(),
                1000.0,
                500.0,
                1000.0, // 1000 kg base
            ),
            effects,
        )).id();
        
        // Apply same force as before
        world.get_mut::<ForceAccumulator>(ship).unwrap()
            .add_force(Force::new(Vector3::new(1000.0, 0.0, 0.0)));
        
        // Run integration
        world.run_system_once(
            |query: Query<(&mut Transform, &mut ForceAccumulator, &ShipData, &StatusEffects)>| {
                physics_integration_system(query, 1.0);
            }
        );
        
        let transform = world.get::<Transform>(ship).unwrap();
        // Effective mass is doubled (2000 kg)
        // a = F/m = 1000/2000 = 0.5 m/s²
        // v = 0 + 0.5*1 = 0.5 m/s
        assert!((transform.velocity.x - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_collision_shape() {
        let shape = CollisionShape::sphere(10.0);
        assert_eq!(shape.radius, 10.0);
        assert!(shape.enabled);
        
        let ship_shape = CollisionShape::ship();
        assert_eq!(ship_shape.radius, constants::DEFAULT_SHIP_RADIUS);
        
        let proj_shape = CollisionShape::projectile();
        assert_eq!(proj_shape.radius, constants::DEFAULT_PROJECTILE_RADIUS);
    }
}
