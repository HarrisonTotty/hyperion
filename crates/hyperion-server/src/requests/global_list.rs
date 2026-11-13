//! Plan 10's global-list handlers (P10.T10.a): `global_features` and `stream_track`.
//!
//! Each starts from [`openable_universe`], so that a universe this server cannot run is refused
//! before any other field is read, and takes the universe's galaxy from the
//! [`GalaxyCache`](crate::compute::GalaxyCache) and its global list from the
//! [`GlobalListService`](crate::compute::GlobalListService), which builds it once a galaxy on the
//! pool's workers (P10.T5.b), at interactive priority as a player waits on it, a build a sky or
//! the warm-up started at bulk joined and raised to it (P10.T10.b). `global_features` is answered
//! in one interactive job on the CPU pool, never on the runtime, each stream's track `ready` if the
//! service holds its tube table (P10.T10.b). `stream_track` reads the stream's table from the
//! service, which has its tracers sprayed on the pool's workers at interactive priority and keeps
//! it if it holds none, and thins its knots on the runtime, some 512 copied, which no job is worth.

use std::sync::Arc;

use hyperion_protocol::{GlobalFeaturesRequest, RequestError, ResponseBody, StreamTrackRequest};
use hyperion_sim::galaxy::global_list::StreamNumber;
use hyperion_sim::id::{SystemId, SystemIdKind};

use super::galaxy::run;
use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{BuildTubeOnPoolError, CancelToken, Priority};
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
    let list = state.global_lists.get(key, Priority::Interactive).await?;
    let galaxy = state.galaxies.get(key).await?;
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        global_features(&galaxy, list.list(), request, |number| {
            shared.global_lists.is_ready(key, number)
        })
    };
    Ok(ResponseBody::GlobalFeatures(
        run(&state, &token, answer).await?,
    ))
}

/// One stream's track: its tube table held, or built for it on the pool's workers and kept.
///
/// # Errors
///
/// Those of [`openable_universe`], of the list's and the galaxy's builds as for [`features`];
/// `unknown_feature` naming `stream` for a number the galaxy's list has no stream for; those of
/// the tube's jobs, as a pool job's; and `internal` if the stream's tube cannot be built. A
/// request cancelled stops waiting, and the build goes on while another waits on the same table.
pub(crate) async fn track(
    state: Arc<AppState>,
    request: StreamTrackRequest,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let key = universe.key();
    let list = state.global_lists.get(key, Priority::Interactive).await?;
    let number = stream_track_number(list.list(), &request)?;
    let galaxy = state.galaxies.get(key).await?;
    let spec = list.list().stream(number).expect("the stream was checked");
    let tube = state
        .global_lists
        .tube((&galaxy, key), spec, Priority::Interactive)
        .await
        .map_err(|error| tube_refused(number, &error))?;
    Ok(ResponseBody::StreamTrack(stream_track(&tube, request)))
}

/// The refusal of a request whose stream `number`'s table could not be built: as a pool job's
/// error, or `internal` for a table the sim refused ([`tube_not_built`]).
pub(crate) fn tube_refused(number: StreamNumber, error: &BuildTubeOnPoolError) -> RequestError {
    match error {
        BuildTubeOnPoolError::Compute(error) => (*error).into(),
        BuildTubeOnPoolError::Table(error) => tube_not_built(number, error),
    }
}

/// The refusal of a request whose listing for system `id` could not be made
/// ([`GlobalListService::listing`](crate::compute::GlobalListService::listing)): its list's
/// build failed, or a stream member's table could not be built.
pub(crate) fn listing_refused(id: SystemId, error: BuildTubeOnPoolError) -> RequestError {
    match (id.kind(), error) {
        (_, BuildTubeOnPoolError::Compute(error)) => error.into(),
        (SystemIdKind::Stream(member), error @ BuildTubeOnPoolError::Table(_)) => {
            tube_refused(StreamNumber::from(member), &error)
        }
        (
            SystemIdKind::Grid(_)
            | SystemIdKind::FeatureMember(_)
            | SystemIdKind::Centre(_)
            | SystemIdKind::DwarfCore(_)
            | SystemIdKind::Pinned(_)
            | SystemIdKind::Catalogue(_),
            BuildTubeOnPoolError::Table(_),
        ) => unreachable!("only a stream member's listing builds a table"),
    }
}
