//! The system's handler: `system_summary` (plan 06, P06.T34, with plan 11's P11.T13 slice).
//!
//! It starts from [`openable_universe`](super::universe::openable_universe), as every handler that
//! names a universe does, checks the time and decodes the ID, takes the universe's galaxy from the
//! [`GalaxyCache`](crate::compute::GalaxyCache), and then answers on the CPU pool from the
//! [`SharedSystemCache`](crate::compute::SharedSystemCache).

use std::sync::Arc;

use hyperion_protocol::{RequestError, ResponseBody, SystemSummaryRequest};

use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{CancelToken, JobError, Priority};
use crate::convert::{SummaryRequest, system_summary, unknown_system};

/// Every star of one system at the request's time, and the orbits that hold them.
///
/// Resolving the ID, generating the system on a miss and summarising it at the time are one
/// interactive pool job: generation builds a track for every star, a millisecond or two for each
/// evolved one (ruling 46 of 2026-09-22), and even a summary from the cache classifies each star,
/// which is more than the runtime should carry. The job owns the state and takes the system cache
/// from it there, as the range query's job does with the cell cache. The small answer is
/// serialised on the runtime (design note 22).
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve; `bad_request` naming
/// `time` for a time outside the clock window; `unknown_system` naming `system` for an ID whose
/// bits are not a system ID, or that plan 03's `resolve` refuses in this universe's galaxy; those
/// of [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built;
/// `queue_full` if the interactive queue has no room for the job; `internal` if the pool is
/// shutting down or the job panics; and `cancelled` if the client gave up before it ran.
pub(crate) async fn summary(
    state: Arc<AppState>,
    request: SystemSummaryRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = SummaryRequest::try_from(&request)?;
    let galaxy = state.galaxies.get(universe.key()).await?;
    let key = universe.key();
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        let stars = shared
            .systems
            .get_or_generate(key, &galaxy, wanted.system())
            .map_err(|error| unknown_system(&request.system, error))?;
        Ok::<_, RequestError>(system_summary(request, &stars, wanted.time()))
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::SystemSummary(answered))
}
