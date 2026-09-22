//! Generation off the async runtime: the CPU pool, cancellation, and deduplication of work.
//!
//! Everything the server computes from the sim runs as a job on the [`CpuPool`], never on the
//! runtime and never under `spawn_blocking`, so that `hello` and `ping` are answered while a map
//! computes (plan 04, design notes 5 and 21). [`SingleFlight`] makes concurrent requests for one
//! expensive value share a single computation, and a [`CancelToken`] lets whoever waits on a job
//! give it up. A density map is computed and cached as a [`RawDensityMap`] and quantised for each
//! response by [`quantise_map`].

mod cancel;
mod density_map;
mod key;
mod pool;
mod single_flight;

pub use cancel::{CancelOnDrop, CancelToken};
pub use density_map::{
    BuildRawMapError, CodeDepth, ParseCodeDepthError, QuantisedMap, RawDensityMap, quantise_map,
};
pub use key::GalaxyKey;
pub use pool::{
    CpuPool, JobError, JobReceiver, PoolCounters, Priority, ShutDownPoolError, StartPoolError,
    SubmitJobError,
};
pub use single_flight::{Flight, SingleFlight};

pub(crate) use pool::panic_message;
