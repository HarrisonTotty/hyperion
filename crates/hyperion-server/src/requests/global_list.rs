//! Plan 10's global-list handlers (P10.T10.a): `global_features` and `stream_track`.
//!
//! Each starts from [`openable_universe`], so that a universe this server cannot run is refused
//! before any other field is read, and takes the universe's galaxy from the
//! [`GalaxyCache`](crate::compute::GalaxyCache) and its global list from the
//! [`GlobalListService`](crate::compute::GlobalListService), which builds it once a galaxy on the
//! pool's workers (P10.T5.b). `global_features` is answered in one interactive job on the CPU pool,
//! never on the runtime. `stream_track` has its tube's tracers sprayed on the pool's workers at
//! interactive priority, since this server holds no tube between requests (P10.T10.b's cache
//! will), and thins its knots on the runtime, some 512 copied, which no job is worth.

use std::sync::Arc;

use hyperion_protocol::{GlobalFeaturesRequest, RequestError, ResponseBody, StreamTrackRequest};

use super::galaxy::run;
use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{BuildTubeOnPoolError, CancelToken};
use crate::convert::{global_features, stream_track, stream_track_number, tube_not_built};

/// The galaxy's global list: its entries and its accreted progenitors.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve; those of
/// [`GlobalListService::get`](crate::compute::GlobalListService::get) if the list cannot be built,
/// and of [`GalaxyCache::get`](crate::compute::GalaxyCache::get); `queue_full` if the interactive
/// queue has no room for the job; `internal` if the pool is shutting down or the job panics; and
/// `cancelled` if the client gave up before it ran.
pub(crate) async fn features(
    state: Arc<AppState>,
    request: GlobalFeaturesRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let key = universe.key();
    let list = state.global_lists.get(key).await?;
    let galaxy = state.galaxies.get(key).await?;
    let answer = move |_: &CancelToken| global_features(&galaxy, list.list(), request);
    Ok(ResponseBody::GlobalFeatures(
        run(&state, &token, answer).await?,
    ))
}

/// One stream's track, its tube table built for it on the pool's workers.
///
/// # Errors
///
/// Those of [`openable_universe`], of the list's and the galaxy's builds as for [`features`];
/// `unknown_feature` naming `stream` for a number the galaxy's list has no stream for; those of
/// the tube's jobs, as a pool job's; and `internal` if the stream's tube cannot be built.
pub(crate) async fn track(
    state: Arc<AppState>,
    request: StreamTrackRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let key = universe.key();
    let list = state.global_lists.get(key).await?;
    let number = stream_track_number(list.list(), &request)?;
    let galaxy = state.galaxies.get(key).await?;
    let spec = list.list().stream(number).expect("the stream was checked");
    let tube = state
        .global_lists
        .tube((&galaxy, key), spec, &token)
        .await
        .map_err(|error| match error {
            BuildTubeOnPoolError::Compute(error) => error.into(),
            BuildTubeOnPoolError::Table(error) => tube_not_built(number, &error),
        })?;
    Ok(ResponseBody::StreamTrack(stream_track(&tube, request)))
}
