//! Placement: where a system's planets go, under dynamical constraints (plan 14, phase B).
//!
//! Built so far:
//!
//! - [`spacing`]: the mutual Hill radius, the next orbit at a given spacing and the stability
//!   floor of design note 7 (P14.T6.a), and the spacing draw (P14.T6.b).
//! - [`zones`]: Holman and Wiegert's stability limits, the stable zones of a hierarchy, and each
//!   zone's disc and close-binary flag (P14.T9, design note 10).
//!
//! Masses (T7) and the class placers (T8) follow.

pub mod spacing;
pub mod zones;

pub use spacing::{
    HillFactor, Neighbour, PairSpacing, SpacingDraws, SpacingKind, SpacingOutcome,
    draw_pair_spacing, mutual_hill_factor, mutual_hill_radius, next_semi_major_axis,
    satisfies_floor, spacing_floor,
};
pub use zones::{
    BuildZoneHierarchyError, ComponentKind, OrbitHost, OrbitZone, ResolveZoneDiscError,
    ZoneDiscInputs, ZoneHierarchy, ZoneNode, ZoneStar, holman_wiegert_p_type,
    holman_wiegert_s_type, stable_zones,
};
