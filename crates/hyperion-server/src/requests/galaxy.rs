//! The galaxy's handlers: `galaxy_parameters`, `density_map` and `systems_in_range` (plan 04,
//! P04.T14.b to T14.d).
//!
//! Each starts from [`openable_universe`](super::universe::openable_universe), so that a universe
//! this server cannot run is refused before any other field is read, and then takes the universe's
//! galaxy from the [`GalaxyCache`](crate::compute::GalaxyCache), which builds it if `open_universe`
//! has not already.

use std::sync::Arc;

use hyperion_protocol::{
    DensityMapRequest, GalaxyParametersRequest, RequestError, ResponseBody, StellarBriefDto,
    SystemsInRange, SystemsInRangeRequest,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::query::{RangeQuery, RangeResult, SystemHit, range_query};
use hyperion_sim::time::UniverseTime;

use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{CancelToken, GalaxyKey, JobError, Priority, SharedBriefCache, quantise_map};
use crate::convert::{
    MapRequest, RangeRequest, brief_dto, density_map, galaxy_parameters, systems_in_range,
};
use crate::limits::BRIEF_CHUNK_ROWS;

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

/// The systems within the request's radius of its centre at its time, with the census, and with
/// each row's stellar brief when the request sets `include_stellar` (plan 06, P06.T34).
///
/// The query and the conversion of its hits are one interactive pool job, which owns the state and
/// takes its [`CellCacheHandle`](crate::compute::CellCacheHandle) from it there: the handle borrows
/// the cache and so cannot cross an `.await`, and the conversion is cheap beside the query but too
/// dear for the runtime, since a 20,000-record answer is megabytes of wire types. The frame that
/// answer becomes is serialised by a second job (design note 22). A running query is never stopped
/// (design note 5): its cell budget, [`MAX_QUERY_CELLS`](crate::limits::MAX_QUERY_CELLS), bounds
/// how long it can take.
///
/// Briefs come from the [`SharedBriefCache`](crate::compute::SharedBriefCache), one
/// [`BriefModel`](hyperion_sim::stellar::brief::BriefModel) per system, which takes a dead primary's
/// full track until plan 06's fate table (ruling 90). Up to [`BRIEF_CHUNK_ROWS`] rows they are
/// built in the query's own job; beyond it the rows are split, in order, into chunks of that many,
/// each an interactive job of its own so that the pool's workers share them, and the answer is
/// converted by one more job. A 20,000-row answer queues 20 chunks, each waiting for room in the
/// interactive queue, so briefs never make a query that has run answer `queue_full`. Each row's brief is a pure function of its system and the time, so
/// neither the chunks nor the cache change a byte of the answer.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve, `bad_request` naming
/// `time`, `centre`, `radius_ly` or `limit` for a field it cannot use (design note 24), those of
/// [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built,
/// `queue_full` if the interactive queue has no room for the query (a chunk of briefs, queued after
/// the query has run, waits for room instead), `internal`
/// if the pool is shutting down or a job panics, and `cancelled` if the client gave up before a job
/// ran.
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
    let job_galaxy = Arc::clone(&galaxy);
    let queried = move |_: &CancelToken| {
        let mut cells = shared.cells.handle(key);
        // No sources: the grid is the whole answer in the first milestone (plan 03's
        // `range_query`).
        let result = range_query(&job_galaxy, &mut cells, &[], &query);
        if request.include_stellar && result.systems().len() > BRIEF_CHUNK_ROWS {
            return Queried::Chunked(Box::new((request, query, result)));
        }
        let briefs = request.include_stellar.then(|| {
            briefs_of(
                &shared.briefs,
                key,
                &job_galaxy,
                result.systems(),
                query.time(),
            )
        });
        Queried::Answered(systems_in_range(
            &job_galaxy,
            request,
            &query,
            &result,
            briefs,
        ))
    };
    let (request, query, result) = match run(&state, &token, queried).await? {
        Queried::Answered(answer) => return Ok(ResponseBody::SystemsInRange(answer)),
        Queried::Chunked(parts) => *parts,
    };
    let result = Arc::new(result);
    let rows = result.systems().len();
    let mut chunks = Vec::with_capacity(rows.div_ceil(BRIEF_CHUNK_ROWS));
    for start in (0..rows).step_by(BRIEF_CHUNK_ROWS) {
        let end = (start + BRIEF_CHUNK_ROWS).min(rows);
        let (shared, galaxy, result) =
            (Arc::clone(&state), Arc::clone(&galaxy), Arc::clone(&result));
        let time = query.time();
        // The query has run, so a chunk waits for room in the queue rather than refusing it.
        chunks.push(
            state
                .pool
                .submit(
                    Priority::Interactive,
                    token.clone(),
                    move |_: &CancelToken| {
                        briefs_of(
                            &shared.briefs,
                            key,
                            &galaxy,
                            &result.systems()[start..end],
                            time,
                        )
                    },
                )
                .await?,
        );
    }
    let mut briefs = Vec::with_capacity(rows);
    for chunk in chunks {
        briefs.extend(
            chunk
                .await
                .unwrap_or_else(|closed| Err(JobError::from(closed)))?,
        );
    }
    let convert =
        move |_: &CancelToken| systems_in_range(&galaxy, request, &query, &result, Some(briefs));
    // As a chunk does, the conversion waits for room rather than refusing a query that has run.
    let answered = state
        .pool
        .submit(Priority::Interactive, token, convert)
        .await?
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
    Ok(ResponseBody::SystemsInRange(answered))
}

/// What the query's job hands back: the answer, or the parts of one whose briefs are chunked.
enum Queried {
    Answered(SystemsInRange),
    Chunked(Box<(SystemsInRangeRequest, RangeQuery, RangeResult)>),
}

/// Runs `job` as an interactive pool job and waits for its value.
async fn run<T: Send + 'static>(
    state: &AppState,
    token: &CancelToken,
    job: impl FnOnce(&CancelToken) -> T + Send + 'static,
) -> Result<T, RequestError> {
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token.clone(), job)?;
    Ok(receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?)
}

/// The wire briefs of `hits` at `time`, in order, from `cache`'s models of the galaxy `key` names.
fn briefs_of(
    cache: &SharedBriefCache,
    key: GalaxyKey,
    galaxy: &Galaxy,
    hits: &[SystemHit],
    time: UniverseTime,
) -> Vec<Option<StellarBriefDto>> {
    hits.iter()
        .map(|hit| {
            cache
                .get_or_build(key, galaxy, hit.record())
                .brief_at(time)
                .map(|brief| brief_dto(&brief))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use hyperion_sim::galaxy::placement::NoCache;
    use hyperion_sim::galaxy::query::RangeQuery;
    use hyperion_sim::units::LightYears;
    use hyperion_sim::{GENERATOR_VERSION, Seed};

    use super::*;

    /// The briefs of a query's rows are the same built whole or in four chunks, and from an empty,
    /// a warm, a tight or no cache.
    #[test]
    fn briefs_do_not_depend_on_chunks_or_the_cache() {
        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let centre = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
            .expect("in the root cube");
        let query = RangeQuery::builder(centre, LightYears::new(15.0))
            .build()
            .expect("a query");
        let result = range_query(&galaxy, &mut NoCache::new(), &[], &query);
        let hits = result.systems();
        assert!(hits.len() > 8, "{} rows", hits.len());
        let whole = briefs_of(
            &SharedBriefCache::new(64 << 20),
            key,
            &galaxy,
            hits,
            query.time(),
        );
        assert!(whole.iter().all(Option::is_some));
        let warm = SharedBriefCache::new(64 << 20);
        let _ = briefs_of(&warm, key, &galaxy, hits, query.time());
        for cache in [
            warm,
            SharedBriefCache::new(0),
            SharedBriefCache::new(10_000),
        ] {
            let quarter = hits.len().div_ceil(4);
            let chunked: Vec<_> = hits
                .chunks(quarter)
                .flat_map(|chunk| briefs_of(&cache, key, &galaxy, chunk, query.time()))
                .collect();
            assert_eq!(chunked, whole);
        }
    }
}
