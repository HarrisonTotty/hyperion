//! The census: every star brighter than a cut an observer sees, each at its own retarded time
//! (rendering plan R06, Design notes 9–13).
//!
//! - [`query`]: the [`SkyQuery`], its builder, the [`SkyContext`] a census job reads, and the
//!   [`CensusPlan`] of cells to open.
//! - [`cache`]: the per-cell cache's trait, which [`SkyContext`] holds, and its monotone rule.

pub mod cache;
pub mod query;

pub use cache::{NoSkyCellCache, Served, SkyCellCache, serve_from_entry};
pub use query::{
    BuildSkyQueryError, CensusPlan, Cone, MAX_FORCED_CAP_LY, MAX_N_MAX, SkyContext, SkyQuery,
    SkyQueryBuilder, census_plan, plan_cells,
};
