//! Generation off the async runtime: the CPU pool, cancellation, and deduplication of work.
//!
//! Everything the server computes from the sim runs as a job on the [`CpuPool`], never on the
//! runtime and never under `spawn_blocking`, so that `hello` and `ping` are answered while a map
//! computes (plan 04, design notes 5 and 21). [`SingleFlight`] makes concurrent requests for one
//! expensive value share a single computation, and a [`CancelToken`] lets whoever waits on a job
//! give it up.

mod cancel;
mod key;
mod pool;
mod single_flight;

pub use cancel::{CancelOnDrop, CancelToken};
pub use key::GalaxyKey;
pub use pool::{
    CpuPool, JobError, JobReceiver, PoolCounters, Priority, ShutDownPoolError, StartPoolError,
    SubmitJobError,
};
pub use single_flight::{Flight, SingleFlight};
