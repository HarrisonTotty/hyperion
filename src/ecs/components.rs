//! Contains definitions for the various simulation entity components.

use crate::math::*;
use specs::{Component, Entity, VecStorage};


/// Represents the "Bridge" component. All objects with this component are
/// considered player-controllable.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Bridge {
    /// The password necessary to connect to this bridge.
    pub password: String
}


/// Represents the "camera" component.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Camera {
    /// The field of view of the camera, in degrees.
    pub fov: u8,

    /// The angular position of the camera.
    pub orientation: Vector
}


/// Represents a collision reference to another entity.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Collision(pub Entity);


/// Represent the "description" component. All objects with this component
/// have a short description and long description.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Description {
    /// The long description of the object.
    pub long_desc: String,

    /// The short description of the object.
    pub short_desc: String
}


/// Represents the "dynamics" component. All objects which inherit this
/// component are subject to the laws of newtonian dynamics.
#[derive(Clone, Component, Debug, Default)]
#[storage(VecStorage)]
pub struct Dynamics {
    /// The acceleration of the object.
    pub acceleration: Vector,
    
    /// The position of the object.
    pub position: Vector,

    /// The velocity of the object.
    pub velocity: Vector
}


/// Represents the "mass" component.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Mass(pub f64);


/// Represents the "name" component.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Name(pub String);


/// Represents the "orientation" component. All objects which inherit this
/// component are subject to things like angular acceleration.
#[derive(Clone, Component, Debug, Default)]
#[storage(VecStorage)]
pub struct Orientation {
    /// The angular acceleration of the object.
    pub angular_acceleration: Vector,

    /// The angular position (orientation) of the object.
    pub angular_position: Vector,

    /// The angular velocity of the object.
    pub angular_velocity: Vector
}


/// Represents the "physicality" component. All objects with physicality have a
/// bounding/size definition and may or may not be subject to collision detection.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Physicality {
    /// The shape of the object.
    pub shape: Shape,

    /// Whether collision detection is enabled for this object.
    pub collisions_enabled: bool
}

/// Implements `std::default::Default` for `Physicality`.
impl std::default::Default for Physicality {
    fn default() -> Self { Physicality { shape: Shape::Point, collisions_enabled: true } }
}

/// Represents the "type" component. All selectable objects should have this
/// component.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub enum Type {
    Other(String),
    PlayerShip,
    Ship,
    Unknown
}
