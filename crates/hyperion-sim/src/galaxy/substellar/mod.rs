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
//!
//! # What each object carries (P13.T5)
//!
//! Both are [`SystemRecord`](crate::galaxy::placement::SystemRecord)s, with an ID, a position, a
//! population, an age and a mass, and body `0x0000` of each is the object itself
//! ([`SystemKind`](crate::galaxy::placement::SystemKind)).
//!
//! - A **brown dwarf** takes plan 06's stellar stage as a single star:
//!   [`SystemStars::generate`](crate::stellar::system::SystemStars::generate) draws its
//!   metallicity, evaluates P06.T13's cooling fits at its age plus the clock time and classifies
//!   it, with no companion (plan 13's Risks: every object here is single). Deuterium burning, which
//!   keeps objects above 13 Jupiter masses brighter for their first 10–100 Myr, is not modelled:
//!   Burrows et al. (2001, §II) describe it in words and model curves, not in closed form (plan 13,
//!   P13.T5.a).
//! - A **rogue planet** has its record and its metallicity
//!   ([`draw_metallicity`](crate::stellar::system::draw_metallicity)) and nothing derived. Its bulk
//!   properties, atmosphere and moons are plan 14's, which derives it as body `0x0000` with
//!   [`giant_cooling`](crate::stellar::substellar::giant_cooling) for a giant, and the consoles say
//!   `BULK PROPERTIES: NOT YET MODELLED` until then (Design note 12).

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
