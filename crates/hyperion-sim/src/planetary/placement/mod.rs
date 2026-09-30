//! Placement: where a system's planets go, under dynamical constraints (plan 14, phase B).
//!
//! Built so far:
//!
//! - [`spacing`]: the mutual Hill radius, the next orbit at a given spacing and the stability
//!   floor of design note 7 (P14.T6.a), and the spacing draw (P14.T6.b).
//! - [`zones`]: Holman and Wiegert's stability limits, the stable zones of a hierarchy, and each
//!   zone's disc and close-binary flag (P14.T9, design note 10).
//! - [`masses`]: the planets' masses, a characteristic mass per group with members correlated
//!   about it, each group held to its disc's solids or gas, and whether a disc can grow a giant's
//!   core (P14.T7).
//! - [`classes`]: the class placers (P14.T8), which turn an orbit host's class, disc and zone into
//!   planets on orbits, [`place`], with each planet's eccentricity, inclination and angles
//!   ([`classes::orbits`]) and the tidal circularisation the fate transform applies
//!   ([`classes::tides`]).

pub mod classes;
pub mod masses;
pub mod spacing;
pub mod zones;

pub use classes::{
    Commensurability, HostPlacement, PlacedPlanet, PlacementHost, Resonance, core_fallback, place,
};
pub use masses::{GiantCore, GroupCap, GroupMasses, MassDraws, giant_core, group_masses};
pub use spacing::{
    HillFactor, Neighbour, PairSpacing, SpacingDraws, SpacingKind, SpacingOutcome,
    draw_pair_spacing, mutual_hill_factor, mutual_hill_radius, next_semi_major_axis,
    satisfies_floor, spacing_floor,
};
pub use zones::{
    BuildZoneHierarchyError, OrbitHost, OrbitZone, ResolveZoneDiscError, ZoneDiscInputs,
    ZoneHierarchy, ZoneNode, ZoneStar, holman_wiegert_p_type, holman_wiegert_s_type, stable_zones,
};
