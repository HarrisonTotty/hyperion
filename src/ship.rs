//! Contains the definitions of a ship.

use specs::{Component, VecStorage};


/// Represents the "ship" component. Entities which inherit this component are
/// a player-controlled ship.
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Ship {
}


/// Represents a structure which is used to build a ship.
pub struct ShipBuilder {
}
