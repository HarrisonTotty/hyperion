//! What is inside a cluster today: member classes, their profiles and counts (plan 09, phase 2).
//!
//! A feature's members are split by class, as a layer's are split by population (brainstorm,
//! "What is inside a cluster today"). In each mass band the member density is the sum over classes
//! of expected count × profile; the counts are closed forms in the cluster's parameters
//! ([`ClusterModel`](super::cluster::ClusterModel), [`counts`]), and each profile is normalised and
//! never rises with radius ([`profile`]), so a cell's bound is the classes' sum at its nearest
//! corner ([`MemberClassTable`]). The class is a derived mark and never enters an ID.

pub mod abundances;
#[cfg(test)]
mod checks;
pub mod counts;
pub mod profile;
pub mod retention;
pub mod table;

pub use abundances::MemberAbundances;
pub use profile::{ClassProfile, ProfileShape};
pub use table::{CellProposal, LocalCell, MemberClassTable, TailClass};

use crate::galaxy::imf::MassBand;
use crate::units::SolarMasses;

/// What a member class is (P09.T9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClassKind {
    /// Living stars below the turn-off, their numbers depleted at the bottom (P09.T9.a).
    Living,
    /// White dwarfs, by their progenitors' band (P09.T9.a).
    WhiteDwarf,
    /// Neutron stars retained at birth (P09.T9.b).
    NeutronStar,
    /// Black holes still bound (P09.T9.c).
    BlackHole,
    /// The near tidal tail, first population only (P09.T9.g).
    Tail,
}

/// Which of a massive globular's populations a class belongs to (P09.T9.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Generation {
    /// The first population: every open cluster's and every globular's below 10⁵ M☉ at birth.
    First,
    /// The enriched second population.
    Second,
}

/// Whether a class's systems are single or binary, which changes their mass and so their profile
/// (P09.T9.e).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Multiplicity {
    /// Single systems.
    Single,
    /// Binaries, profiled by their system mass.
    Binary,
}

/// One member class: what it is, where its primaries' initial masses lie and how heavy its
/// systems are today.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemberClass {
    /// Its kind.
    pub kind: ClassKind,
    /// The band its members' primaries' initial masses fall in.
    pub band: MassBand,
    /// Its population.
    pub generation: Generation,
    /// Single or binary.
    pub multiplicity: Multiplicity,
    /// The range its primaries' initial masses are drawn from, M☉.
    pub initial_mass_range: (SolarMasses, SolarMasses),
    /// The mean present mass of its systems, M☉: what its profile's `q` reads.
    pub mean_mass: SolarMasses,
}
