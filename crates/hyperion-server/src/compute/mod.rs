//! Generation off the async runtime: the CPU pool, cancellation, and deduplication of work.
//!
//! Everything the server computes from the sim runs as a job on the [`CpuPool`], never on the
//! runtime and never under `spawn_blocking`, so that `hello` and `ping` are answered while a map
//! computes (plan 04, design notes 5 and 21). [`SingleFlight`] makes concurrent requests for one
//! expensive value share a single computation, and a [`CancelToken`] lets whoever waits on a job
//! give it up. Whatever is computed this way fails only as a [`ComputeError`]. The pool refuses to
//! generate on a thread whose floating-point mode flushes subnormals, which
//! [`probe_flush_to_zero`] detects (plan R04, design note 15).
//!
//! [`GalaxyCache`] holds the galaxies built from universes' seeds, [`SharedCellCache`] the
//! generated cells a range query reads, [`SharedSystemCache`] the systems' stars a
//! `system_summary` reads and [`SharedBodyCache`] the planetary systems `system_bodies` and
//! `body_detail` read, each keyed by a [`GalaxyKey`]. A density map is computed and cached as a
//! [`RawDensityMap`] and quantised for each response by [`quantise_map`]; an extinction map by
//! [`ExtinctionMapService`] into the same raw type, quantised by [`quantise_map_with_floor`]
//! (plan 07, P07.T10.a). [`SharedSightlineCache`] holds the lines of sight the `extinction`
//! request marches (P07.T10.c). [`SharedFeatureCellCache`] holds plan 09's feature cells and
//! [`SharedClusterModelCache`] their interiors (P09.T40.a), [`SharedCentreOrbitCache`] the
//! galactic centre's cells of members and their orbits (P09.T37.b), and [`served_range_query`]
//! generates a cold range query's cells into those caches on the pool's workers before it walks
//! them.
//! [`global_list_on`] and [`tube_table_on`] make plan 10's global list and a stream's tube table
//! on the pool's workers, each galaxy's spray table kept in [`SharedSprayForces`] (P10.T5.b). A sky's
//! caps, census, band and limit
//! map run as bulk jobs, as far as its [`SkyCaps`] reach, each census cell's bright subset kept in
//! the sky's own cell cache, and each galaxy's sky tables built once on the pool and kept in a cache
//! of their own (rendering plan R06).

mod bodies;
mod cancel;
mod cells;
mod centre_orbits;
mod class_cells;
mod density_map;
mod error;
mod extinction_map;
mod features;
mod float_mode;
mod galaxies;
mod global_list;
mod key;
mod pool;
mod range;
mod sightlines;
mod single_flight;
pub(crate) mod sky;
mod sky_cells;
mod sky_tables;
mod systems;
mod thread_priority;
mod tube_tables;

pub use bodies::{BodyCacheCounters, GenerateBodiesError, GeneratedSystem, SharedBodyCache};
pub use cancel::{CancelOnDrop, CancelToken};
pub use cells::{CachedCell, CellCacheHandle, SharedCellCache};
pub use centre_orbits::{CentreOrbitHandle, SharedCentreOrbitCache};
pub use class_cells::{ClassCellHandle, SharedClassCellCache};
pub use density_map::{
    BuildRawMapError, CodeDepth, DensityMapService, MAP_WIDTH_LY, MapKey, MapResolution,
    ParseCodeDepthError, ParseMapResolutionError, PerLy2, QuantisedMap, RawDensityMap, RawMap,
    quantise_map, quantise_map_with_floor,
};
pub use error::ComputeError;
pub use extinction_map::{
    EXTINCTION_FLOOR_LOG10_MAG, ExtinctionMapKey, ExtinctionMapService, Mag, RawExtinctionMap,
};
pub use features::{
    ClusterModelHandle, FeatureCellHandle, GalaxyFeaturesService, KeptInteriors, MapGlobular,
    MapGlobulars, SharedClusterModelCache, SharedFeatureCellCache,
};
pub(crate) use features::{counted_members, expected_members_of};
pub use float_mode::{FlushProbe, probe_flush_to_zero};
pub use galaxies::{GalaxyCache, GalaxyCounters};
pub use global_list::{
    BuildTubeOnPoolError, SharedSprayForces, Shares, TRACERS_PER_SHARE, global_list_on,
    tube_table_on,
};
pub use global_list::{GlobalListService, HeldGlobalList};
pub use key::GalaxyKey;
pub use pool::{
    BulkGate, CpuPool, JobError, JobReceiver, PoolCounters, Priority, Promotion, ShutDownPoolError,
    StartPoolError, SubmitJobError,
};
#[cfg(test)]
pub(crate) use range::served_sources;
pub(crate) use range::with_row_reads;
pub use range::{RangeCaches, RowReads, ServedQuery, served_range_query};
pub use sightlines::{LineGas, SharedSightlineCache, SightlineMarcher};
pub use single_flight::{Flight, SingleFlight};
pub use sky::{ForceSkyCapsError, SkyCaps, SkyTablesSource};
pub use sky_cells::SkyCellCounters;
pub use sky_tables::SkyTablesCounters;
pub use systems::{SharedBriefCache, SharedSystemCache, SystemStarsHandle};
pub use tube_tables::{ListedTubes, Listing, TubeKey, TubeTableCache};

pub(crate) use pool::panic_message;
pub(crate) use sky_cells::SharedSkyCellCache;
pub(crate) use sky_tables::SkyTablesService;
