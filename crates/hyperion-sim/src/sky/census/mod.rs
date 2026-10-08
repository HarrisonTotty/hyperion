//! The census: every star brighter than a cut an observer sees, each at its own retarded time
//! (rendering plan R06, Design notes 9–13).
//!
//! - [`query`]: the [`SkyQuery`], its builder, the [`SkyContext`] a census job reads, the
//!   [`CensusPlan`] of cells to open, shell by shell nearest first ([`Shell`]), and the
//!   [`Completeness`] a census of some of its shells reaches.
//! - [`cell`]: one cell's stars, each at its retarded time, each record's light bounded star by
//!   star first ([`StarBounds`]), and the census's tallies.
//! - [`merge`]: the parts merged in a total order and cut at `n_max`, the [`SkyCensus`], whole or
//!   to the shells done.
//! - [`cache`]: the cell cache's trait, which [`SkyContext`] holds, its blocks of cells keyed by
//!   magnitude ([`SkyBlock`]), and the rule by which an entry serves a query.

pub mod cache;
pub mod cell;
pub mod merge;
pub mod query;

pub use cache::{
    BlockKey, BlockParams, CellOutcome, HeldRecord, NoSkyCellCache, Rebuild, SkyBlock, SkyCellCache,
};
pub use cell::{
    Bound, BoundStar, CellOffsets, CensusCost, CensusTallies, GRID_STAR_BOUND, LayerCost,
    LayerTally, PairTally, RecordLight, SkyStar, StarBounds, StarLight, cell_floor,
    cell_offset_bound, census_cell, census_cell_with_cost, census_record, flux_bound,
    star_offset_bound,
};
pub use merge::{SkyCensus, merge_census, merge_shells, sky_order};
pub use query::{
    BuildSkyQueryError, CellSlab, CensusPlan, Completeness, Cone, ConeRegion, MAX_FORCED_CAP_LY,
    MAX_N_MAX, SHELL_EDGES_LY, SHELLED_LAYERS, Shell, SkyContext, SkyQuery, SkyQueryBuilder,
    census_plan, census_plan_of, plan_cells,
};
