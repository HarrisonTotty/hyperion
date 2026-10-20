//! The erosion step's outputs (R09.T14.c; the lane's design §4.3): what the step writes into the
//! working state from the multigrid's outcome.
//!
//! - **The elevation**: each eroded cell's, the branch's (the solve's, on a world dry now through
//!   `branches::dry_now_elevation_m`); each bound keeps its ground to the bit.
//! - **The water surface**: the elevation on every cell off the sea, which is dry (a world dry now
//!   writes none: its paleo-lakes are gone); the sea's cells, bounds, keep theirs.
//! - **The routing**: the final elevation routed afresh with the field's water bodies and the
//!   solver's receiver uniforms ([`route_with_water`]), so that no cell drains uphill on the
//!   field the step leaves (T14.a's property), the fixed point's last routing being the previous
//!   iterate's (decision-r09-t14-choices, "T14.b follow-up" item 1): each cell's flow, its
//!   receiver's edge or terminal at a root; its geometric drainage area, its own included (Hack
//!   1957's, by solid angle); and its steepness index `k_s` = S A^θ, S = max(0, `W_c` − `W_r`) ÷ ℓ
//!   on the filled surface W and θ = [`LogSteepness::THETA`], zero at a root.
//! - **The runoffs**: each cell's own, the branch's output rule; and its catchment's, the own
//!   runoffs times the cells' areas accumulated over that routing, over its drainage area
//!   (Hergarten 2021, eq. 20), zero where that is zero.
//! - **The channel incision** ([`CellState::channel_incision`]): on a world dry now, z0 less the
//!   solve, before any closure (decision-r09-t14-choices F2); zero elsewhere.
//! - **The marker** [`Working::routed`], which R09.T14.d's re-routing after the late craters reads.
//!
//! [`CellState::channel_incision`]: super::super::CellState::channel_incision
//! [`Working::routed`]: super::super::Working::routed

use hyperion_surface::field::LogSteepness;
use hyperion_surface::num;

use super::super::Working;
use super::super::plates::{cell_of, slot};
use super::depressions::route_with_water;
use super::drainage::accumulate;
use super::multigrid::Outcome;
use crate::math;
use crate::units::{Metres, SquareMetres};

/// What a branch writes beside the outcome's routing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Written<'a> {
    /// Each cell's elevation, metres, one a cell: the branch's for an eroded cell; a bound's is
    /// not read.
    pub(crate) elevation_m: &'a [f64],
    /// The sea's cells, whose water surfaces stay.
    pub(crate) sea: &'a [bool],
    /// Each cell's own runoff in the output, m a⁻¹.
    pub(crate) runoff_m_per_year: &'a [f64],
    /// Each cell's channel incision, metres, on a world dry now; `None` elsewhere.
    pub(crate) channel_incision_m: Option<&'a [f64]>,
}

/// Writes the step's outputs into `working` from `outcome` and `written` (see the module
/// documentation), and marks the field routed.
///
/// # Panics
///
/// If an input is not one a cell of `working`, or a value is not finite.
pub(crate) fn write(
    working: &mut Working,
    sea_level_m: f64,
    outcome: &Outcome,
    written: &Written<'_>,
) {
    let n = working.cells.len();
    for (what, len) in [
        ("elevation", written.elevation_m.len()),
        ("sea flag", written.sea.len()),
        ("runoff", written.runoff_m_per_year.len()),
        ("bound", outcome.bounds.len()),
    ] {
        assert_eq!(len, n, "one {what} a cell");
    }
    for (c, state) in working.cells.iter_mut().enumerate() {
        if outcome.bounds[c].is_none() {
            state.elevation = Metres::new(num::assert_finite(written.elevation_m[c]));
        }
        if !written.sea[c] {
            state.water_surface = state.elevation;
        }
    }
    let z: Vec<f64> = working.cells.iter().map(|s| s.elevation.value()).collect();
    let mesh = &outcome.mesh;
    let routing = route_with_water(
        mesh,
        &z,
        written.sea,
        sea_level_m,
        &outcome.bodies,
        &outcome.uniforms,
    );
    let drainage = accumulate(&routing, mesh.areas_m2());
    let discharge: Vec<f64> = written
        .runoff_m_per_year
        .iter()
        .zip(mesh.areas_m2())
        .map(|(&r, &a)| r * a)
        .collect();
    let catchment = accumulate(&routing, &discharge);
    let filled = routing.filled_m();
    for (c, state) in working.cells.iter_mut().enumerate() {
        let cell = cell_of(c);
        let area = drainage[c];
        state.flow = routing.flow(cell);
        state.drainage = SquareMetres::new(num::assert_finite(area));
        state.steepness_index_m0_9 = routing.receiver(cell).map_or(0.0, |link| {
            let slope = num::max(0.0, filled[c] - filled[slot(link.cell)]) / link.length_m;
            num::assert_finite(slope * math::powf(area, LogSteepness::THETA))
        });
        state.runoff_m_per_year = num::assert_finite(written.runoff_m_per_year[c]);
        state.catchment_runoff_m_per_year = if area > 0.0 {
            num::assert_finite(catchment[c] / area)
        } else {
            0.0
        };
        state.channel_incision = Metres::new(
            written
                .channel_incision_m
                .map_or(0.0, |incision| num::assert_finite(incision[c])),
        );
    }
    working.routed = true;
}
