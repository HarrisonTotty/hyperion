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
//! [`RawDensityMap`] and quantised for each response by [`quantise_map`]. A sky's tables and
//! census run as bulk jobs, as far as its [`SkyCaps`] reach, each census cell's bright subset kept in
//! the sky's own cell cache (rendering plan R06).

mod bodies;
mod cancel;
mod cells;
mod density_map;
mod error;
mod float_mode;
mod galaxies;
mod key;
mod pool;
mod single_flight;
pub(crate) mod sky;
mod sky_cells;
mod systems;

pub use bodies::{BodyCacheCounters, GenerateBodiesError, GeneratedSystem, SharedBodyCache};
pub use cancel::{CancelOnDrop, CancelToken};
pub use cells::{CachedCell, CellCacheHandle, SharedCellCache};
pub use density_map::{
    BuildRawMapError, CodeDepth, DensityMapService, MAP_WIDTH_LY, MapKey, MapResolution,
    ParseCodeDepthError, ParseMapResolutionError, QuantisedMap, RawDensityMap, quantise_map,
};
pub use error::ComputeError;
pub use float_mode::{FlushProbe, probe_flush_to_zero};
pub use galaxies::{GalaxyCache, GalaxyCounters};
pub use key::GalaxyKey;
pub use pool::{
    CpuPool, JobError, JobReceiver, PoolCounters, Priority, ShutDownPoolError, StartPoolError,
    SubmitJobError,
};
pub use single_flight::{Flight, SingleFlight};
pub use sky::{ForceSkyCapsError, SkyCaps};
pub use sky_cells::SkyCellCounters;
pub use systems::{SharedBriefCache, SharedSystemCache};

pub(crate) use pool::panic_message;
pub(crate) use sky_cells::SharedSkyCellCache;
