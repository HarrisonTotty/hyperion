//! Contains definitions for various simulation systems.

use crate::ecs::components;
use crate::ecs::resources;
use crate::math::*;
use specs::prelude::*;

/// Detects collisions within the game world.
/// Objects which have collided are assigned a collision component.
pub struct CollisionDetection;
impl<'a> System<'a> for CollisionDetection {
    type SystemData = (
        Entities<'a>,
        Read<'a, resources::CollisionLimits>,
        ReadStorage<'a, components::Dynamics>,
        ReadStorage<'a, components::Orientation>,
        ReadStorage<'a, components::Physicality>,
        WriteStorage<'a, components::Collision>
    );
    fn run(&mut self, (entities, limits, dyns, ort, phys, mut collisions): Self::SystemData) {
        debug!("Detecting collisions...");
        for (i, (i_entity, i_dyns, _i_ort, i_phys)) in (&*entities, &dyns, &ort, &phys).join().enumerate() {
            if i_phys.collisions_enabled {
                for (j, (j_entity, j_dyns, _j_ort, j_phys)) in (&*entities, &dyns, &ort, &phys).join().enumerate() {
                    if i != j && j_phys.collisions_enabled {
                        let dist = (j_dyns.position - i_dyns.position).magnitude();
                        if dist < limits.maximum_detection_theshold {
                            if dist < limits.minimum_detection_theshold {
                                trace!("COLLISION: {:?} <-> {:?}", i_entity, j_entity);
                                if let Err(_msg) = collisions.insert(i_entity, components::Collision(j_entity)) {
                                    error!("Unable to assign collision to entity.");
                                }
                            } else {
                                match (i_phys.shape, j_phys.shape) {
                                    (Shape::Cuboid(_x1, _y1, _z1), Shape::Cuboid(_x2, _y2, _z2)) => {
                                    },
                                    (Shape::Cuboid(_x, _y, _z), Shape::Point) => {
                                    },
                                    (Shape::Cuboid(_x, _y, _z), Shape::Sphere(_r)) => {
                                    },
                                    (Shape::Sphere(_r), Shape::Cuboid(_x, _y, _z)) => {
                                    },
                                    (Shape::Sphere(r), Shape::Point) => {
                                        if dist - r <= 0.0 {
                                            trace!("COLLISION: {:?} <-> {:?}", i_entity, j_entity);
                                            if let Err(_msg) = collisions.insert(i_entity, components::Collision(j_entity)) {
                                                error!("Unable to assign collision to entity.");
                                            }
                                        }
                                    },
                                    (Shape::Sphere(r1), Shape::Sphere(r2)) => {
                                        if dist - (r1 + r2) <= 0.0 {
                                            trace!("COLLISION: {:?} <-> {:?}", i_entity, j_entity);
                                            if let Err(_msg) = collisions.insert(i_entity, components::Collision(j_entity)) {
                                                error!("Unable to assign collision to entity.");
                                            }
                                        }
                                    },
                                    (Shape::Point, Shape::Cuboid(_x, _y, _z)) => {
                                    },
                                    (Shape::Point, Shape::Point) => {
                                        // Points only collide when they are on top of each other, which should
                                        // be catched by `min_detection_theshold` above.
                                    },
                                    (Shape::Point, Shape::Sphere(r)) => {
                                        if dist - r <= 0.0 {
                                            trace!("COLLISION: {:?} <-> {:?}", i_entity, j_entity);
                                            if let Err(_msg) = collisions.insert(i_entity, components::Collision(j_entity)) {
                                                error!("Unable to assign collision to entity.");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}


/// Handles the entities which have been detected as collided.
pub struct HandleCollisions;
impl<'a> System<'a> for HandleCollisions {
    type SystemData = (
        ReadStorage<'a, components::Collision>
    );
    fn run(&mut self, _data: Self::SystemData) {
        debug!("Handling collisions...");
    }
}


/// Handles updating the position and velocity of an entity from its
/// acceleration.
///
/// This system will also automatically truncate the various values according to
/// their limits, with the exception of "position", which will be toroidally
/// wrapped because our universe has periodic boundary conditions.
pub struct HandleDynamics;
impl<'a> System<'a> for HandleDynamics {
    type SystemData = (
        Read<'a, resources::DeltaTime>,
        Read<'a, resources::DynamicsLimits>,
        WriteStorage<'a, components::Dynamics>
    );
    fn run(&mut self, data: Self::SystemData) {
        debug!("Updating newtonian dynamics...");
        let (dt, limits, mut objects) = data;
        for obj in (&mut objects).join() {
            trace!(
                "OLD DYNAMICS: [{:?}, {:?}, {:?}]",
                &obj.acceleration,
                &obj.velocity,
                &obj.position
            );
            let acc_mag = obj.acceleration.magnitude();
            if acc_mag < limits.minimum_acceleration {
                obj.acceleration *= limits.minimum_acceleration / acc_mag;
            } else if acc_mag > limits.maximum_acceleration {
                obj.acceleration *= limits.maximum_acceleration / acc_mag;
            }
            obj.velocity += obj.acceleration * dt.0;
            let vel_mag = obj.velocity.magnitude();
            if vel_mag < limits.minimum_velocity {
                obj.velocity *= limits.minimum_velocity / vel_mag;
            } else if vel_mag > limits.maximum_velocity {
                obj.velocity *= limits.maximum_velocity / vel_mag;
            }
            obj.position += obj.velocity * dt.0;
            let pos_mag = obj.position.magnitude();
            if pos_mag < limits.minimum_position {
                obj.position *= limits.minimum_position / pos_mag;
            } else if pos_mag > limits.maximum_position {
                obj.position *= limits.maximum_position / pos_mag;
                obj.position = -obj.position;
            }
            trace!(
                "NEW DYNAMICS: [{:?}, {:?}, {:?}]",
                &obj.acceleration,
                &obj.velocity,
                &obj.position
            );
        }
    }
}


/// Handles updating the angular position and velocity of an entity from its
/// angular acceleration. Note that the position vector is normalized to its
/// direction at the end.
pub struct HandleOrientation;
impl<'a> System<'a> for HandleOrientation {
    type SystemData = (
        Read<'a, resources::DeltaTime>,
        Read<'a, resources::OrientationLimits>,
        WriteStorage<'a, components::Orientation>
    );
    fn run(&mut self, data: Self::SystemData) {
        debug!("Updating angular dynamics (orientation)...");
        let (dt, limits, mut objects) = data;
        for obj in (&mut objects).join() {
            trace!(
                "OLD ORIENTATION: [{:?}, {:?}, {:?}]",
                &obj.angular_acceleration,
                &obj.angular_velocity,
                &obj.angular_position
            ); 
            let acc_mag = obj.angular_acceleration.magnitude();
            if acc_mag < limits.minimum_angular_acceleration {
                obj.angular_acceleration *= limits.minimum_angular_acceleration / acc_mag;
            } else if acc_mag > limits.maximum_angular_acceleration {
                obj.angular_acceleration *= limits.maximum_angular_acceleration / acc_mag;
            }
            obj.angular_velocity += obj.angular_acceleration * dt.0;
            let vec_mag = obj.angular_velocity.magnitude();
            if vec_mag < limits.minimum_angular_velocity {
                obj.angular_velocity *= limits.minimum_angular_velocity / vec_mag;
            } else if vec_mag > limits.maximum_angular_velocity {
                obj.angular_velocity *= limits.maximum_angular_velocity / vec_mag;
            }
            obj.angular_position += obj.angular_velocity * dt.0;
            obj.angular_position = obj.angular_position.direction();
            trace!(
                "NEW ORIENTATION: [{:?}, {:?}, {:?}]",
                &obj.angular_acceleration,
                &obj.angular_velocity,
                &obj.angular_position
            );
        }
    }
}
