//! Placement: where a system's planets go, under dynamical constraints (plan 14, phase B).
//!
//! So far only [`spacing`] is built: the mutual Hill radius, the next orbit at a given spacing and
//! the stability floor of design note 7 (P14.T6.a). The spacing draw (T6.b), masses (T7), the class
//! placers (T8) and the stable zones of multiple systems (T9) follow.

pub mod spacing;

pub use spacing::{
    HillFactor, Neighbour, mutual_hill_factor, mutual_hill_radius, next_semi_major_axis,
    satisfies_floor, spacing_floor,
};
