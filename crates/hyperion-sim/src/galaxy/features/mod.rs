//! Large features: the feature catalogue on its 4,096 ly grid, the share φ of each population in
//! features, the marks of each kind and the gas features add (plan 09, phase 1).
//!
//! Features are objects of their own, not bumps in the field (brainstorm, "Large features"). A
//! catalogue on its own coarse grid places globular clusters, open clusters, OB associations,
//! star-forming regions and molecular clouds by the same thinning as the systems
//! ([`catalogue`]), and the field gives up to them the share φ ([`shares`]), so that nothing is
//! counted twice. Every feature carries the marks of its kind ([`kinds`]), an emission class
//! ([`emission`]) and, for clouds, embedded regions and superbubbles, what it does to the gas
//! ([`gas_overlay`]).
//!
//! Phase 1 places no member: members, nested grids and the class tables are phases 2 and 5.

pub mod catalogue;
pub mod centre;
#[cfg(test)]
mod checks;
pub mod cluster;
pub mod emission;
pub mod gas_overlay;
pub mod ids;
pub mod interior;
pub mod kinds;
pub mod members;
pub mod shares;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use ids::{FeatureDesignation, FeatureId, FeatureKind, FeatureProcess};

// Plan 09, phase 5: nested grids and members (P09.T20–T23).
pub mod nested;
