//! The census: every star brighter than a cut an observer sees, each at its own retarded time
//! (rendering plan R06, Design notes 9–13).
//!
//! - [`query`]: the [`SkyQuery`], its builder, the [`SkyContext`] a census job reads, and the
//!   [`CensusPlan`] of cells to open.
//! - [`cell`]: one cell's stars, each at its retarded time, and the census's tallies.
//! - [`cache`]: the per-cell cache's trait, which [`SkyContext`] holds, and its monotone rule.

pub mod cache;
pub mod cell;
pub mod query;

pub use cache::{NoSkyCellCache, Served, SkyCellCache, serve_from_entry};
pub use cell::{
    Bound, CensusTallies, EYE_OFFSET_BOUND_MAG, GRID_STAR_BOUND, LayerTally, SkyStar, cell_floor,
    cell_offset_bound, census_cell, census_record, flux_bound, star_offset_bound,
};
pub use query::{
    BuildSkyQueryError, CensusPlan, Cone, MAX_FORCED_CAP_LY, MAX_N_MAX, SkyContext, SkyQuery,
    SkyQueryBuilder, census_plan, plan_cells,
};
