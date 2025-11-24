//! ECS systems for the HYPERION simulation.
//!
//! This module contains all the systems that operate on the simulation components,
//! implementing the game logic for movement, combat, power management, and more.

use bevy_ecs::prelude::*;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Commands, Query};
use nalgebra::Vector3;

use super::components::*;
use crate::weapons::{WeaponTagCalculator, StatusEffectType};

#[cfg(test)]
use crate::models::WeaponTag;

/// System that updates ship positions based on velocity and effective weight.
///
/// This system runs each tick and updates the `Transform` component based on
/// the ship's velocity. Heavier ships (affected by Graviton weapons) move slower.
pub fn movement_system(mut query: Query<&mut Transform>, delta_time: f32) {
    for mut transform in query.iter_mut() {
        // Apply velocity to position
        let velocity = transform.velocity;
        transform.position += velocity * delta_time;
        
        // Apply angular velocity to rotation
        if transform.angular_velocity.magnitude() > 0.0 {
            let axis_angle = transform.angular_velocity * delta_time;
            let rotation_delta = nalgebra::UnitQuaternion::from_scaled_axis(axis_angle);
            transform.rotation = rotation_delta * transform.rotation;
        }
    }
}

/// System that manages power generation and distribution to modules.
///
/// This system calculates total power generation from power cores and ensures
/// that the allocated power doesn't exceed available power.
pub fn power_system(mut power_grids: Query<&mut PowerGrid>) {
    for power_grid in power_grids.iter_mut() {
        // Calculate available power (capacity minus allocated)
        // Power generation and distribution is managed elsewhere
        // This system just tracks and validates
        let _allocated = power_grid.total_allocated();
        
        // TODO: Enforce power limits and redistribute if over capacity
    }
}

/// System that manages heat generation and cooling distribution.
///
/// This system calculates cooling capacity and ensures modules don't overheat.
pub fn cooling_system(mut cooling_systems: Query<&mut CoolingSystem>) {
    for cooling_sys in cooling_systems.iter_mut() {
        // Calculate total cooling allocated
        let _allocated = cooling_sys.total_allocated();
        
        // TODO: Enforce cooling limits and manage overheating
    }
}

/// System that regenerates shields when they are raised.
///
/// Shields only regenerate when raised, and regeneration rate is affected by
/// power allocation and ship damage.
pub fn shield_system(
    mut query: Query<(&mut ShieldComponent, &PowerGrid)>,
    delta_time: f32,
) {
    for (mut shield, power_grid) in query.iter_mut() {
        if shield.raised {
            // Calculate regeneration based on power available
            let power_efficiency = if shield.power_draw > 0.0 {
                (power_grid.available_power() / shield.power_draw).min(1.0)
            } else {
                1.0
            };
            
            // Regenerate shields using built-in method
            let regen = shield.regen_rate * power_efficiency * delta_time;
            shield.strength = (shield.strength + regen).min(shield.max_strength);
        }
    }
}

/// System that updates weapon cooldowns over time.
///
/// This system reduces the remaining cooldown for all weapons that are cooling down.
pub fn weapon_cooldown_system(
    mut query: Query<&mut WeaponComponent>,
    delta_time: f32,
) {
    for mut weapon in query.iter_mut() {
        weapon.update_cooldown(delta_time);
    }
}

/// System that handles automatic weapon firing when target is locked.
///
/// This system fires weapons that are set to automatic when they have a target lock
/// and are ready to fire (cooldown complete, ammunition available, etc.).
///
/// Note: This is a simplified version that doesn't use parent-child relationships.
/// In a full implementation, weapons would be children of ships.
pub fn weapon_fire_system(
    mut commands: Commands,
    mut weapons: Query<(Entity, &mut WeaponComponent, &Transform)>,
) {
    for (weapon_entity, mut weapon, weapon_transform) in weapons.iter_mut() {
        // Skip if not automatic or not active
        if !weapon.is_automatic || !weapon.is_active {
            continue;
        }
        
        // Check if weapon can fire
        if !weapon.can_fire() {
            continue;
        }
        
        // TODO: Get targeting component from parent ship
        // TODO: Check ammunition from parent ship inventory
        
        // For now, create a simple projectile
        let projectile = ProjectileComponent::kinetic(
            weapon_entity, // Use weapon entity as owner for now
            weapon.base_damage,
            weapon.tags.clone(),
            10.0,
        );
        
        // Create projectile entity with appropriate transform
        commands.spawn((
            projectile,
            Transform {
                position: weapon_transform.position,
                rotation: weapon_transform.rotation,
                velocity: weapon_transform.rotation * Vector3::new(0.0, 0.0, 1000.0), // Forward
                angular_velocity: Vector3::zeros(),
            },
        ));
        
        // Start weapon cooldown
        weapon.fire();
    }
}

/// System that applies damage to ships and modules using weapon tag calculations.
///
/// This system handles collision detection between projectiles and ships,
/// calculates damage using the weapon tag system, and applies it to shields/hull.
pub fn damage_system(
    mut commands: Commands,
    projectiles: Query<(Entity, &ProjectileComponent, &Transform)>,
    mut ships: Query<(
        Entity,
        &mut ShipData,
        &mut ShieldComponent,
        &Transform,
    )>,
    mut status_effects: Query<&mut StatusEffects>,
) {
    let calculator = WeaponTagCalculator::new();
    
    for (projectile_entity, projectile, proj_transform) in projectiles.iter() {
        // Skip beam weapons (handled by BeamWeaponSystem)
        if projectile.projectile_type == ProjectileType::Beam {
            continue;
        }
        
        // Check for collision with target ship (if projectile has a target)
        if let Some(target_entity) = projectile.target {
            if let Ok((ship_entity, mut ship_data, mut shield, ship_transform)) = 
                ships.get_mut(target_entity) {
                
                // Simple distance-based collision detection
                let distance = (proj_transform.position - ship_transform.position).magnitude();
                let collision_distance = 10.0; // TODO: Use actual ship/projectile radius
                
                if distance < collision_distance {
                    // Calculate damage using weapon tags
                    let damage_result = calculator.calculate_damage(
                        projectile.damage,
                        &projectile.tags,
                    );
                    
                    // Check for errors
                    let damage_result = match damage_result {
                        Ok(dr) => dr,
                        Err(_) => continue, // Skip on error
                    };
                    
                    // Apply damage to shields first, then hull
                    let remaining_damage = shield.apply_damage(damage_result.hull_damage);
                    if remaining_damage > 0.0 {
                        ship_data.hull -= remaining_damage;
                    }
                    
                    // Apply status effects
                    if let Ok(mut effects) = status_effects.get_mut(ship_entity) {
                        if let Some(status_effect) = damage_result.status_effect {
                            effects.apply(status_effect.effect_type, status_effect.duration);
                        }
                    }
                    
                    // Remove projectile
                    commands.entity(projectile_entity).despawn();
                }
            }
        }
    }
}

/// System that moves projectiles and updates their lifetime.
///
/// This system updates projectile positions and despawns projectiles that
/// have exceeded their lifetime.
pub fn projectile_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ProjectileComponent)>,
    delta_time: f32,
) {
    for (entity, mut projectile) in query.iter_mut() {
        // Skip beam weapons (they don't move)
        if projectile.projectile_type == ProjectileType::Beam {
            continue;
        }
        
        // Update lifetime
        projectile.update(delta_time);
        
        // Despawn if lifetime expired
        if projectile.is_expired() {
            commands.entity(entity).despawn();
            continue;
        }
        
        // Missiles track their target
        if let ProjectileType::Missile { thrust: _, turn_rate: _ } = projectile.projectile_type {
            // TODO: Implement target tracking/homing behavior
            // For now, just maintain current velocity
        }
        
        // Position already updated by movement_system
    }
}

/// System that handles beam weapons (continuous damage).
///
/// Beam weapons apply 1x damage per second while active and target is locked.
pub fn beam_weapon_system(
    beams: Query<(&ProjectileComponent, &Transform)>,
    mut ships: Query<(
        Entity,
        &mut ShipData,
        &mut ShieldComponent,
        &Transform,
    )>,
    mut status_effects: Query<&mut StatusEffects>,
    delta_time: f32,
) {
    let calculator = WeaponTagCalculator::new();
    
    for (beam, beam_transform) in beams.iter() {
        // Only process beam weapons
        if beam.projectile_type != ProjectileType::Beam {
            continue;
        }
        
        // Check if beam hits target
        if let Some(target_entity) = beam.target {
            if let Ok((ship_entity, mut ship_data, mut shield, ship_transform)) = 
                ships.get_mut(target_entity) {
                
                // Simple line-of-sight check (could be improved with raycasting)
                let distance = (beam_transform.position - ship_transform.position).magnitude();
                let max_range = 5000.0; // TODO: Make this configurable
                
                if distance < max_range {
                    // Calculate damage (1x per second = base_damage * delta_time)
                    let damage_result = calculator.calculate_damage(
                        beam.damage * delta_time,
                        &beam.tags,
                    );
                    
                    let damage_result = match damage_result {
                        Ok(dr) => dr,
                        Err(_) => continue,
                    };
                    
                    // Apply damage
                    let remaining_damage = shield.apply_damage(damage_result.hull_damage);
                    if remaining_damage > 0.0 {
                        ship_data.hull -= remaining_damage;
                    }
                    
                    // Apply status effects
                    if let Ok(mut effects) = status_effects.get_mut(ship_entity) {
                        if let Some(status_effect) = damage_result.status_effect {
                            effects.apply(status_effect.effect_type, status_effect.duration);
                        }
                    }
                }
            }
        }
    }
}

/// System that handles missile and torpedo explosions.
///
/// When missiles or torpedoes hit their target, they create an explosion
/// that can damage nearby ships.
pub fn explosion_system() {
    // TODO: Implement area-of-effect explosion damage
    // For now, single-target damage is handled by damage_system
}

/// System that applies and decays status effects.
///
/// This system updates the duration of all active status effects and
/// removes effects that have expired.
pub fn status_effect_system(
    mut query: Query<&mut StatusEffects>,
    delta_time: f32,
) {
    for mut effects in query.iter_mut() {
        effects.update(delta_time);
    }
}

/// System that handles anti-missile countermeasures.
///
/// This system allows point-defense weapons to intercept incoming missiles
/// and torpedoes.
pub fn countermeasure_system(
    mut commands: Commands,
    countermeasures: Query<(&WeaponComponent, &Transform), With<PointDefenseMarker>>,
    projectiles: Query<(Entity, &ProjectileComponent, &Transform)>,
) {
    // TODO: Implement point-defense logic
    // For now, this is a placeholder
    
    for (weapon, weapon_transform) in countermeasures.iter() {
        // Check for nearby enemy missiles/torpedoes
        for (projectile_entity, projectile, proj_transform) in projectiles.iter() {
            // TODO: Skip if projectile is from this ship (needs parent-child relationships)
            
            // Skip if not a missile or torpedo
            let is_missile_or_torpedo = matches!(
                projectile.projectile_type,
                ProjectileType::Missile { .. } | ProjectileType::Torpedo
            );
            
            if !is_missile_or_torpedo {
                continue;
            }
            
            // Check if in range
            let distance = (proj_transform.position - weapon_transform.position).magnitude();
            let pd_range = 1000.0; // TODO: Make configurable
            
            if distance < pd_range && weapon.can_fire() {
                // Intercept missile (simple success check for now)
                // TODO: Add accuracy calculations
                commands.entity(projectile_entity).despawn();
                break; // One missile per weapon per tick
            }
        }
    }
}

/// System that handles engineering repairs to damaged modules.
///
/// This system allows crew to repair damaged modules over time.
pub fn repair_system() {
    // TODO: Implement repair logic with crew assignments
    // For now, this is a placeholder
}

/// System that handles science officer scans.
///
/// This system processes scan requests and reveals information about
/// target ships.
pub fn scanning_system() {
    // TODO: Implement scanning logic
    // This would track scan progress and reveal ship data
}

/// System that handles ship-to-ship communication.
///
/// Communication is blocked when a ship is jammed by Ion weapons.
pub fn communication_system() {
    // TODO: Implement message passing between ships
    // For now, CommunicationState.can_communicate() provides the check
}

/// System that handles warp drive acceleration.
///
/// Warp drives provide continuous acceleration but are disabled by Tachyon weapons.
pub fn warp_system(
    mut query: Query<(&mut WarpDriveComponent, &mut Transform, &StatusEffects)>,
    delta_time: f32,
) {
    for (mut warp, mut transform, effects) in query.iter_mut() {
        // Check if disabled by Tachyon
        if effects.has_effect(StatusEffectType::TachyonWarpBlock) {
            warp.disabled = true;
            warp.active = false;
            continue;
        } else {
            warp.disabled = false;
        }
        
        // Handle startup
        if warp.startup_progress > 0.0 {
            warp.startup_progress -= delta_time;
            if warp.startup_progress <= 0.0 {
                warp.active = true;
                warp.startup_progress = 0.0;
            }
            continue;
        }
        
        // Handle cooldown
        if warp.cooldown_progress > 0.0 {
            warp.cooldown_progress = (warp.cooldown_progress - delta_time).max(0.0);
            continue;
        }
        
        // Apply warp acceleration
        if warp.active {
            // Warp increases speed based on warp factor
            let speed_multiplier = warp.current_warp_factor;
            if transform.velocity.magnitude() > 0.1 {
                let direction = transform.velocity.normalize();
                transform.velocity = direction * speed_multiplier * 100.0; // Base speed scaled by warp
            }
        }
    }
}

/// System that handles jump drive teleportation.
///
/// Jump drives provide instant teleportation but are disabled by Tachyon weapons.
pub fn jump_system(
    mut query: Query<(&mut JumpDriveComponent, &mut Transform, &StatusEffects)>,
    delta_time: f32,
) {
    for (mut jump, mut transform, effects) in query.iter_mut() {
        // Check if disabled by Tachyon
        if effects.has_effect(StatusEffectType::TachyonWarpBlock) {
            jump.disabled = true;
            jump.target_destination = None;
            continue;
        } else {
            jump.disabled = false;
        }
        
        // Handle charging
        if jump.target_destination.is_some() {
            jump.startup_progress -= delta_time;
            if jump.startup_progress <= 0.0 {
                // Execute jump
                if let Some(destination) = jump.target_destination {
                    transform.position = destination;
                    transform.velocity = Vector3::zeros(); // Stop when jumping
                }
                jump.target_destination = None;
                jump.startup_progress = 0.0;
                jump.cooldown_progress = jump.cooldown_time;
            }
            continue;
        }
        
        // Handle cooldown
        if jump.cooldown_progress > 0.0 {
            jump.cooldown_progress = (jump.cooldown_progress - delta_time).max(0.0);
            continue;
        }
        
        // Jump execution is triggered by player action, not automatic
        // This system just manages the charging state
    }
}

// Marker component for point-defense weapons
#[derive(Component)]
pub struct PointDefenseMarker;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    #[test]
    fn test_movement_system() {
        let mut world = World::new();
        
        let ship = world.spawn(
            Transform {
                position: Vector3::new(0.0, 0.0, 0.0),
                rotation: nalgebra::UnitQuaternion::identity(),
                velocity: Vector3::new(10.0, 0.0, 0.0),
                angular_velocity: Vector3::zeros(),
            },
        ).id();
        
        // Run system with 1 second delta
        world.run_system_once(|query: Query<&mut Transform>| {
            movement_system(query, 1.0);
        });
        
        let transform = world.get::<Transform>(ship).unwrap();
        assert!((transform.position.x - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_shield_system_regeneration() {
        let mut world = World::new();
        
        let ship = world.spawn((
            ShieldComponent::new(100.0, 10.0, 5.0),
            PowerGrid::new(100.0, 200.0),
        )).id();
        
        // Damage shields
        world.get_mut::<ShieldComponent>(ship).unwrap().strength = 50.0;
        
        // Run system with 1 second delta
        world.run_system_once(|query: Query<(&mut ShieldComponent, &PowerGrid)>| {
            shield_system(query, 1.0);
        });
        
        let shield = world.get::<ShieldComponent>(ship).unwrap();
        assert!(shield.strength > 50.0); // Should have regenerated
        assert!(shield.strength <= 100.0); // But not over max
    }

    #[test]
    fn test_shield_system_no_regen_when_lowered() {
        let mut world = World::new();
        
        let mut shield = ShieldComponent::new(100.0, 10.0, 5.0);
        shield.strength = 50.0;
        shield.raised = false; // Shields lowered
        
        let ship = world.spawn((
            shield,
            PowerGrid::new(100.0, 200.0),
        )).id();
        
        world.run_system_once(|query: Query<(&mut ShieldComponent, &PowerGrid)>| {
            shield_system(query, 1.0);
        });
        
        let shield = world.get::<ShieldComponent>(ship).unwrap();
        assert!((shield.strength - 50.0).abs() < 0.01); // No regeneration
    }

    #[test]
    fn test_weapon_cooldown_system() {
        let mut world = World::new();
        
        let mut weapon = WeaponComponent::new(
            "w1".to_string(),
            "laser_mk1".to_string(),
            "Laser".to_string(),
            vec![WeaponTag::SingleFire],
            10.0,
            50.0,
        );
        
        // Set cooldown directly
        weapon.cooldown = 5.0;
        
        let weapon_entity = world.spawn(weapon).id();
        
        world.run_system_once(|query: Query<&mut WeaponComponent>| {
            weapon_cooldown_system(query, 1.0);
        });
        
        let weapon = world.get::<WeaponComponent>(weapon_entity).unwrap();
        assert!((weapon.cooldown - 4.0).abs() < 0.01); // Cooldown should have decreased by 1 second
    }

    #[test]
    fn test_status_effect_system() {
        let mut world = World::new();
        
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffectType::IonJam, 5.0);
        
        let entity = world.spawn(effects).id();
        
        world.run_system_once(|query: Query<&mut StatusEffects>| {
            status_effect_system(query, 1.0);
        });
        
        let effects = world.get::<StatusEffects>(entity).unwrap();
        assert!(effects.has_effect(StatusEffectType::IonJam));
        let duration = effects.get_duration(StatusEffectType::IonJam).unwrap();
        assert!((duration - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_status_effect_system_removes_expired() {
        let mut world = World::new();
        
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffectType::IonJam, 0.5);
        
        let entity = world.spawn(effects).id();
        
        world.run_system_once(|query: Query<&mut StatusEffects>| {
            status_effect_system(query, 1.0);
        });
        
        let effects = world.get::<StatusEffects>(entity).unwrap();
        assert!(!effects.has_effect(StatusEffectType::IonJam));
    }

    #[test]
    fn test_warp_system_disabled_by_tachyon() {
        let mut world = World::new();
        
        let mut warp = WarpDriveComponent::new(9.9, 5.0, 10.0);
        warp.active = true;
        
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffectType::TachyonWarpBlock, 10.0);
        
        let ship = world.spawn((
            warp,
            Transform {
                position: Vector3::zeros(),
                rotation: nalgebra::UnitQuaternion::identity(),
                velocity: Vector3::new(10.0, 0.0, 0.0),
                angular_velocity: Vector3::zeros(),
            },
            effects,
        )).id();
        
        world.run_system_once(|query: Query<(&mut WarpDriveComponent, &mut Transform, &StatusEffects)>| {
            warp_system(query, 1.0);
        });
        
        let warp = world.get::<WarpDriveComponent>(ship).unwrap();
        assert!(!warp.active); // Disabled by Tachyon
        assert!(warp.disabled);
    }

    #[test]
    fn test_jump_system_disabled_by_tachyon() {
        let mut world = World::new();
        
        let mut jump = JumpDriveComponent::new(10000.0, 10.0, 30.0);
        jump.target_destination = Some(Vector3::new(5000.0, 0.0, 0.0));
        jump.startup_progress = 9.0;
        
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffectType::TachyonWarpBlock, 10.0);
        
        let ship = world.spawn((
            jump,
            Transform {
                position: Vector3::zeros(),
                rotation: nalgebra::UnitQuaternion::identity(),
                velocity: Vector3::zeros(),
                angular_velocity: Vector3::zeros(),
            },
            effects,
        )).id();
        
        world.run_system_once(|query: Query<(&mut JumpDriveComponent, &mut Transform, &StatusEffects)>| {
            jump_system(query, 1.0);
        });
        
        let jump = world.get::<JumpDriveComponent>(ship).unwrap();
        assert!(jump.target_destination.is_none()); // Jump cancelled by Tachyon
        assert!(jump.disabled);
    }

    #[test]
    fn test_projectile_system_lifetime() {
        let mut world = World::new();
        
        let projectile = ProjectileComponent::kinetic(
            Entity::from_raw(0),
            50.0,
            vec![WeaponTag::SingleFire],
            0.5,
        );
        
        let projectile_entity = world.spawn((
            projectile,
            Transform {
                position: Vector3::zeros(),
                rotation: nalgebra::UnitQuaternion::identity(),
                velocity: Vector3::new(1000.0, 0.0, 0.0),
                angular_velocity: Vector3::zeros(),
            },
        )).id();
        
        world.run_system_once(|mut commands: Commands, query: Query<(Entity, &mut ProjectileComponent)>| {
            projectile_system(commands, query, 1.0);
        });
        
        // Projectile should be despawned
        assert!(world.get::<ProjectileComponent>(projectile_entity).is_none());
    }
}
