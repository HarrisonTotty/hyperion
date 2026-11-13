//! Plan 09's feature handlers (P09.T40.b): `features_in_range`, `galaxy_features`,
//! `feature_detail` and `black_hole_state`.
//!
//! Each starts from [`openable_universe`], so that a universe this server cannot run is refused
//! before any other field is read, checks its fields in design note 24's order, and takes the
//! universe's galaxy from the [`GalaxyCache`](crate::compute::GalaxyCache). Each answer is built in
//! one interactive job on the CPU pool, never on the runtime: a range of features reads their cells
//! and counts their classes, a detail counts one feature's, and the black hole's state reads its
//! events. The galaxy map's globulars are found once per galaxy in a bulk job and kept
//! ([`GalaxyFeaturesService`](crate::compute::GalaxyFeaturesService)), as the density maps are; each
//! `galaxy_features` then describes them at its own time.

use std::sync::Arc;

use hyperion_protocol::{
    BlackHoleStateRequest, FeatureDetailRequest, FeaturesInRangeRequest, GalaxyFeaturesRequest,
    RequestError, ResponseBody,
};

use super::galaxy::run;
use super::universe::openable_universe;
use crate::AppState;
use crate::compute::CancelToken;
use crate::convert::{
    FeatureRangeQuery, black_hole_state, black_hole_time, centre_not_built, detail_of,
    feature_detail_target, features_in_range, galaxy_features, galaxy_features_time,
};

/// The large features within the request's radius of its centre at its time, of the kinds it
/// asks for, with a census line per kind.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve; `bad_request` naming
/// `time`, `centre`, `radius_ly` or `kinds` for a field it cannot use (`kinds` empty or with a
/// kind twice); those of [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy
/// cannot be built; `internal` if the galactic centre is asked for and cannot be built;
/// `queue_full` if the interactive queue has no room for the job; `internal` if the pool is
/// shutting down or the job panics; and `cancelled` if the client gave up before it ran.
pub(crate) async fn in_range(
    state: Arc<AppState>,
    request: FeaturesInRangeRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let query = FeatureRangeQuery::try_from(&request)?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        let cells = shared.feature_cells.handle(key);
        features_in_range(&galaxy, &cells, request, query).map_err(centre_not_built)
    };
    Ok(ResponseBody::FeaturesInRange(
        run(&state, &token, answer).await??,
    ))
}

/// The features the galaxy map draws at the request's time: one list per kind it asks for, in its
/// order.
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming `time` or `kinds`; those of the
/// [`GalaxyFeaturesService`](crate::compute::GalaxyFeaturesService) if the globulars cannot be
/// found, and of [`GalaxyCache::get`](crate::compute::GalaxyCache::get); and those of
/// [`in_range`]'s job.
pub(crate) async fn map(
    state: Arc<AppState>,
    request: GalaxyFeaturesRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let t = galaxy_features_time(&request)?;
    let key = universe.key();
    let globulars = state.galaxy_features.get(key).await?;
    let galaxy = state.galaxies.get(key).await?;
    let answer = move |_: &CancelToken| {
        galaxy_features(&galaxy, &globulars, request, t).map_err(centre_not_built)
    };
    Ok(ResponseBody::GalaxyFeatures(
        run(&state, &token, answer).await??,
    ))
}

/// One feature at the request's time: its summary and its structure.
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming `time`, or `feature` for a word that is no
/// feature's ID; `unknown_feature` naming `feature` for a feature's ID that names none of the
/// universe at the time; and those of [`in_range`]'s galaxy and job.
pub(crate) async fn detail(
    state: Arc<AppState>,
    request: FeatureDetailRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let (t, target) = feature_detail_target(&request)?;
    let galaxy = state.galaxies.get(universe.key()).await?;
    let answer = move |_: &CancelToken| detail_of(&galaxy, request, target, t);
    Ok(ResponseBody::FeatureDetail(Box::new(
        run(&state, &token, answer).await??,
    )))
}

/// The central black hole at the request's time: its mass, its luminosity and its disruptions so
/// far.
///
/// It is answered at every time of the clock window.
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming `time`; `internal` if the centre's events
/// cannot be built; and those of [`in_range`]'s galaxy and job.
pub(crate) async fn black_hole(
    state: Arc<AppState>,
    request: BlackHoleStateRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let t = black_hole_time(&request)?;
    let galaxy = state.galaxies.get(universe.key()).await?;
    let answer = move |_: &CancelToken| black_hole_state(&galaxy, request, t);
    Ok(ResponseBody::BlackHoleState(
        run(&state, &token, answer).await??,
    ))
}
