//! Free-floating brown dwarfs and rogue planets, and the small bodies between the stars (plan 13).
//!
//! The brainstorm's "Between the stars": brown dwarfs from 13 Jupiter masses to the hydrogen-burning
//! limit, at one for every five or six stars, and rogue planets from a third of an Earth mass to 13
//! Jupiter masses, about 21 per star on a mass function falling nearly as 1 ÷ mass. Both follow the
//! stars: the same thinning, populations and ages. A bound brown dwarf is a companion (plan 11) and
//! a bound planet is plan 14's; everything counted here is free-floating.
//!
//! This module holds the parameters ([`SubstellarParams`]), the two mass functions and their draws,
//! one galaxy's abundances per system with the rogue planets' cap ([`SubstellarAbundance`]), and
//! the density of interstellar comets and asteroids. Placement and resolution are plan 03's
//! [`placement`](crate::galaxy::placement), which places the two layers as it places the stars
//! (Design note 1).

mod abundance;
mod mass;
mod params;
mod small_bodies;

pub use abundance::SubstellarAbundance;
pub use mass::{draw_brown_dwarf_mass, draw_rogue_planet_mass, rogue_planets_above};
pub use params::{
    BROWN_DWARF_MAX, BROWN_DWARF_MIN, ROGUE_PLANET_MAX, ROGUE_PLANET_MIN, SubstellarParams,
};
pub use small_bodies::{
    REFERENCE_SYSTEMS_PER_LY3, SMALL_BODIES_PER_AU3_AT_REFERENCE, interstellar_small_body_density,
};

pub(crate) use params::{BROWN_DWARF_MIN_MSUN, ROGUE_PLANET_MAX_MSUN, ROGUE_PLANET_MIN_MSUN};
