//! The galaxy's handlers: `galaxy_parameters`, `density_map` and `systems_in_range` (plan 04,
//! P04.T14.b to T14.d).
//!
//! Each starts from [`openable_universe`](super::universe::openable_universe), so that a universe
//! this server cannot run is refused before any other field is read, and then takes the universe's
//! galaxy from the [`GalaxyCache`](crate::compute::GalaxyCache), which builds it if `open_universe`
//! has not already.

use std::sync::Arc;

use hyperion_protocol::{
    DensityMapRequest, GalaxyParametersRequest, RequestError, ResponseBody, SystemsInRangeRequest,
};
use hyperion_sim::galaxy::query::range_query;

use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{CancelToken, JobError, Priority, quantise_map};
use crate::convert::{MapRequest, RangeRequest, density_map, galaxy_parameters, systems_in_range};

/// The galaxy's parameters: the groups of plan 04's P04.T14.b table, in its order.
///
/// The response is built on the runtime rather than on the CPU pool: the parameters are read
/// straight off the galaxy, some eighty numbers and two strings, and the pool job that could cost
/// anything is the galaxy build itself.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve, and those of
/// [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built.
pub(crate) async fn parameters(
    state: Arc<AppState>,
    request: GalaxyParametersRequest,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let galaxy = state.galaxies.get(universe.key()).await?;
    Ok(ResponseBody::GalaxyParameters(galaxy_parameters(
        &universe, &galaxy,
    )))
}

/// A column-density map of the universe's galaxy, quantised to the depth asked for.
///
/// The raster is the [`DensityMapService`](crate::compute::DensityMapService)'s, computed in bulk
/// bands so that an interactive request is never held up for longer than one band (design note 21),
/// and cached raw, so that both depths and every later request come from one computation.
/// Quantising and encoding run as an interactive pool job of their own: they are a pass over up to a
/// million pixels and a base64 encoding of up to 2 MiB, which is too much for the runtime, and the
/// frame this response becomes is serialised by a second job (design note 22).
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve, `bad_request` naming
/// `resolution` or `bits` for a raster or a depth it does not serve, those of
/// [`DensityMapService::get`](crate::compute::DensityMapService::get) if the map cannot be computed,
/// `queue_full` if the interactive queue has no room for the quantising job, `internal` if the pool
/// is shutting down or that job panics, and `cancelled` if the client gave up before it ran.
pub(crate) async fn map(
    state: Arc<AppState>,
    request: DensityMapRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = MapRequest::try_from(&request)?;
    let raw = state.maps.get(wanted.key(universe.key())).await?;
    let (view, depth) = (request.view, wanted.depth());
    // The whole answer is built in the job: quantising reads up to a million pixels and the base64
    // of it is a string of up to 2.8 MiB, neither of which belongs on the runtime.
    let answer = move |_: &CancelToken| {
        let quantised = quantise_map(&raw, view, depth);
        density_map(request, &raw, &quantised)
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
    Ok(ResponseBody::DensityMap(answered))
}

/// The systems within the request's radius of its centre at its time, with the census.
///
/// The query and the conversion of its hits are one interactive pool job, which owns the state and
/// takes its [`CellCacheHandle`](crate::compute::CellCacheHandle) from it there: the handle borrows
/// the cache and so cannot cross an `.await`, and the conversion is cheap beside the query but too
/// dear for the runtime, since a 20,000-record answer is megabytes of wire types. The frame that
/// answer becomes is serialised by a second job (design note 22). A running query is never stopped
/// (design note 5): its cell budget, [`MAX_QUERY_CELLS`](crate::limits::MAX_QUERY_CELLS), bounds
/// how long it can take.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve, `bad_request` naming
/// `time`, `centre`, `radius_ly` or `limit` for a field it cannot use (design note 24), those of
/// [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built,
/// `queue_full` if the interactive queue has no room for the query, `internal` if the pool is
/// shutting down or the query panics, and `cancelled` if the client gave up before it ran.
pub(crate) async fn systems(
    state: Arc<AppState>,
    request: SystemsInRangeRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let query = RangeRequest::try_from(&request)?.into_query();
    let galaxy = state.galaxies.get(universe.key()).await?;
    let key = universe.key();
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        let mut cells = shared.cells.handle(key);
        // No sources: the grid is the whole answer in the first milestone (plan 03's
        // `range_query`).
        let result = range_query(&galaxy, &mut cells, &[], &query);
        systems_in_range(request, &query, &result)
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
    Ok(ResponseBody::SystemsInRange(answered))
}
