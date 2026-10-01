//! The galaxy's handlers: `galaxy_parameters`, `density_map` and `systems_in_range` (plan 04,
//! P04.T14.b to T14.d, with plan 12's observed mode, P12.T6), and `extinction_map` and
//! `extinction` (plan 07, P07.T10.a and T10.c).
//!
//! Each starts from [`openable_universe`](super::universe::openable_universe), so that a universe
//! this server cannot run is refused before any other field is read, and then takes the universe's
//! galaxy from the [`GalaxyCache`](crate::compute::GalaxyCache), which builds it if `open_universe`
//! has not already.

use std::sync::Arc;

use hyperion_protocol::{
    DensityMapRequest, ExtinctionMapRequest, ExtinctionRequest, GalaxyParametersRequest,
    ObservedDto, RequestError, ResponseBody, StellarBriefDto, SystemsInRange,
    SystemsInRangeRequest, TargetExtinction,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{ResolveSystemError, SystemKind, resolve};
use hyperion_sim::galaxy::query::{
    QueryMode, RangeQuery, RangeResult, SystemHit, position_at, range_query,
};
use hyperion_sim::observe::{Observer, TraceMotionError};
use hyperion_sim::time::UniverseTime;

use super::observe::observe_row;
use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{
    CancelToken, EXTINCTION_FLOOR_LOG10_MAG, GalaxyKey, JobError, Priority, SharedBriefCache,
    SharedSystemCache, quantise_map, quantise_map_with_floor,
};
use crate::convert::{
    ExtinctionMapQuery, ExtinctionQuery, LineEnd, MapRequest, RangeRequest, brief_dto, density_map,
    extinction_map as extinction_map_answer, extinction_result, galaxy_parameters,
    systems_in_range, target_extinction, untraced,
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

/// A map of the visual extinction through the universe's galaxy, quantised to the depth asked for
/// above the fixed floor of 0.01 mag (plan 07, P07.T10.a).
///
/// It is answered as [`map`] answers a density map: the raster is the
/// [`ExtinctionMapService`](crate::compute::ExtinctionMapService)'s, computed in bulk bands and
/// cached raw; quantising and encoding are an interactive job of their own; and the frame is
/// serialised by a second job, since the response is large.
///
/// # Errors
///
/// Those of [`map`], in its order: the universe; `bad_request` naming `resolution` or `bits`; the
/// map's computation; and `queue_full`, `internal` or `cancelled` from the quantising job.
pub(crate) async fn extinction_map(
    state: Arc<AppState>,
    request: ExtinctionMapRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = ExtinctionMapQuery::try_from(&request)?;
    let raw = state
        .extinction_maps
        .get(wanted.key(universe.key()))
        .await?;
    let depth = wanted.depth();
    let answer = move |_: &CancelToken| {
        let quantised = quantise_map_with_floor(&raw, EXTINCTION_FLOOR_LOG10_MAG, depth);
        extinction_map_answer(request, &raw, &quantised)
    };
    Ok(ResponseBody::ExtinctionMap(
        run(&state, &token, answer).await?,
    ))
}

/// The extinction from the request's origin to each of its targets, in its order (plan 07,
/// P07.T10.c).
///
/// Every line is one interactive pool job's: each system target is resolved by plan 03's
/// [`resolve`] and placed at the request's time by [`position_at`], as every ID from a client is,
/// and each line is the [`SharedSightlineCache`](crate::compute::SharedSightlineCache)'s or is
/// marched in the seed's own gas at a fixed budget of steps, through one noise cache the job owns.
/// A target whose ID names no system, or one of a kind or layer this generator does not place yet,
/// is answered `no_such_system`, and the rest are still answered.
/// The answer is small, so its frame is serialised on the runtime.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve; `bad_request` naming
/// `time`, `origin` or `targets` for a field it cannot use; those of
/// [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built;
/// `queue_full` if the interactive queue has no room for the job; `internal` if the pool is
/// shutting down or the job panics; and `cancelled` if the client gave up before it ran.
pub(crate) async fn extinction(
    state: Arc<AppState>,
    request: ExtinctionRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = ExtinctionQuery::try_from(&request)?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        let mut lines = shared.sightlines.marcher(key, &galaxy);
        let targets = wanted
            .ends()
            .iter()
            .map(|end| {
                let end = match *end {
                    LineEnd::Position(position) => position,
                    LineEnd::System(id) => match resolve(&galaxy, id) {
                        Ok(record) => position_at(&galaxy, &record, wanted.time()),
                        // A kind or layer this generator does not place yet has no position
                        // either, so to a line of sight it is no system at all.
                        Err(
                            ResolveSystemError::NoSuchSystem
                            | ResolveSystemError::KindNotGenerated
                            | ResolveSystemError::LayerNotGenerated(_),
                        ) => return TargetExtinction::NoSuchSystem,
                    },
                    LineEnd::NotASystem => return TargetExtinction::NoSuchSystem,
                };
                target_extinction(&lines.line(wanted.origin(), &end))
            })
            .collect();
        extinction_result(request, targets)
    };
    Ok(ResponseBody::Extinction(run(&state, &token, answer).await?))
}

/// The systems within the request's radius of its centre at its time, with the census, with each
/// row's stellar brief when the request sets `include_stellar` (plan 06, P06.T34), and in observed
/// mode with what the request's observer receives from each (plan 12, P12.T6).
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
/// full track until plan 06's fate table (ruling 90). In observed mode the query finds exactly what
/// it finds `now` (plan 12, Design note 4), and each row is then read at its retarded time by
/// `observe_hit` over the [`SharedSystemCache`](crate::compute::SharedSystemCache), lent as the
/// sim's `StarsCache` ([`SystemStarsHandle`](crate::compute::SystemStarsHandle)); a row's brief is
/// then its primary's when the light left it. Up to [`BRIEF_CHUNK_ROWS`] rows the briefs and the
/// observations are built in the query's own job; beyond it the rows are split, in order, into
/// chunks of that many, each an interactive job of its own so that the pool's workers share them,
/// and the answer is converted by one more job. A 20,000-row answer queues 20 chunks, each waiting
/// for room in the interactive queue, so neither makes a query that has run answer `queue_full`.
/// Each row's brief and observation are pure functions of its system, the time and the observer,
/// so neither the chunks nor the caches change a byte of the answer.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve, `bad_request` naming
/// `time`, `centre`, `radius_ly`, `limit` or `mode` for a field it cannot use (design note 24),
/// those of [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built,
/// `queue_full` if the interactive queue has no room for the query (a chunk, queued after the query
/// has run, waits for room instead), `bad_request` naming `mode` if the observed answer would hold
/// a system whose motion the sim cannot trace yet (a member of the galactic centre, which this
/// query's grid never places), `internal` if the pool is shutting down or a job panics, and
/// `cancelled` if the client gave up before a job ran.
pub(crate) async fn systems(
    state: Arc<AppState>,
    request: SystemsInRangeRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let query = RangeRequest::try_from(&request)?.into_query();
    let galaxy = state.galaxies.get(universe.key()).await?;
    let key = universe.key();
    let work = RowWork::of(&request, &query);
    let shared = Arc::clone(&state);
    let job_galaxy = Arc::clone(&galaxy);
    let queried = move |_: &CancelToken| {
        let mut cells = shared.cells.handle(key);
        // No sources: the grid is the whole answer in the first milestone (plan 03's
        // `range_query`). Observed mode finds the same systems (plan 12, Design note 4).
        let result = range_query(&job_galaxy, &mut cells, &[], &query);
        if work.is_needed() && result.systems().len() > BRIEF_CHUNK_ROWS {
            return Ok(Queried::Chunked(Box::new((request, query, result))));
        }
        let extras = work
            .run(
                (&shared.briefs, &shared.systems),
                key,
                &job_galaxy,
                result.systems(),
            )
            .map_err(untraced)?;
        Ok::<_, RequestError>(Queried::Answered(systems_in_range(
            &job_galaxy,
            request,
            &query,
            &result,
            extras.briefs,
            extras.observed,
        )))
    };
    let (request, query, result) = match run(&state, &token, queried).await?? {
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
        // The query has run, so a chunk waits for room in the queue rather than refusing it.
        chunks.push(
            state
                .pool
                .submit(
                    Priority::Interactive,
                    token.clone(),
                    move |_: &CancelToken| {
                        let caches = (&shared.briefs, &shared.systems);
                        work.run(caches, key, &galaxy, &result.systems()[start..end])
                    },
                )
                .await?,
        );
    }
    let mut extras = RowExtras::with_capacity(work, rows);
    for chunk in chunks {
        extras.append(
            chunk
                .await
                .unwrap_or_else(|closed| Err(JobError::from(closed)))?
                .map_err(untraced)?,
        );
    }
    let convert = move |_: &CancelToken| {
        systems_in_range(
            &galaxy,
            request,
            &query,
            &result,
            extras.briefs,
            extras.observed,
        )
    };
    // As a chunk does, the conversion waits for room rather than refusing a query that has run.
    let answered = state
        .pool
        .submit(Priority::Interactive, token, convert)
        .await?
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
    Ok(ResponseBody::SystemsInRange(answered))
}

/// What the query's job hands back: the answer, or the parts of one whose rows are chunked.
enum Queried {
    Answered(SystemsInRange),
    Chunked(Box<(SystemsInRangeRequest, RangeQuery, RangeResult)>),
}

/// What each row of a range answer needs beyond its hit: its brief when the request sets
/// `include_stellar`, and what the observer receives from it in observed mode.
#[derive(Debug, Clone, Copy)]
struct RowWork {
    time: UniverseTime,
    include_stellar: bool,
    observer: Option<Observer>,
}

impl RowWork {
    /// The work `request`'s rows need, `query` being its checked query.
    fn of(request: &SystemsInRangeRequest, query: &RangeQuery) -> Self {
        let observer = match query.mode() {
            QueryMode::Now => None,
            QueryMode::ObservedFrom(position) => {
                Some(Observer::new(position, query.time()).expect(
                    "a built query's observer is in the root cube and its time in the clock window",
                ))
            }
        };
        Self {
            time: query.time(),
            include_stellar: request.include_stellar,
            observer,
        }
    }

    /// Whether the rows need anything beyond their hits.
    fn is_needed(self) -> bool {
        self.include_stellar || self.observer.is_some()
    }

    /// The briefs and observations of `hits`, in order, in the galaxy `key` names.
    ///
    /// In `now` the briefs are the brief cache's at the query's time; in observed mode each row is
    /// read at its retarded time ([`observe_row`]), and its brief is the one then.
    fn run(
        self,
        caches: (&SharedBriefCache, &SharedSystemCache),
        key: GalaxyKey,
        galaxy: &Galaxy,
        hits: &[SystemHit],
    ) -> Result<RowExtras, TraceMotionError> {
        let (briefs, systems) = caches;
        let Some(observer) = self.observer else {
            let briefs = self
                .include_stellar
                .then(|| briefs_of(briefs, key, galaxy, hits, self.time));
            return Ok(RowExtras {
                briefs,
                observed: None,
            });
        };
        let mut stars = systems.handle(key);
        let mut extras = RowExtras::with_capacity(self, hits.len());
        for hit in hits {
            let (reading, brief_then) = observe_row(galaxy, &mut stars, hit, &observer)?;
            extras.push(reading, brief_then.map(|brief| brief_dto(&brief)));
        }
        Ok(extras)
    }
}

/// The briefs and the observations of a range answer's rows, each `None` when the request did not
/// ask for it.
#[derive(Debug, Default)]
struct RowExtras {
    briefs: Option<Vec<Option<StellarBriefDto>>>,
    observed: Option<Vec<ObservedDto>>,
}

impl RowExtras {
    /// No rows yet, with room for `rows` of what `work` builds.
    fn with_capacity(work: RowWork, rows: usize) -> Self {
        Self {
            briefs: work.include_stellar.then(|| Vec::with_capacity(rows)),
            observed: work.observer.map(|_| Vec::with_capacity(rows)),
        }
    }

    /// One observed row: its observation, and its brief then if the request asked for briefs.
    fn push(&mut self, observed: ObservedDto, brief: Option<StellarBriefDto>) {
        if let Some(briefs) = &mut self.briefs {
            briefs.push(brief);
        }
        if let Some(all) = &mut self.observed {
            all.push(observed);
        }
    }

    /// The rows of the next chunk, after these.
    fn append(&mut self, next: Self) {
        if let (Some(briefs), Some(more)) = (&mut self.briefs, next.briefs) {
            briefs.extend(more);
        }
        if let (Some(observed), Some(more)) = (&mut self.observed, next.observed) {
            observed.extend(more);
        }
    }
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
///
/// A rogue planet has no brief, since it has no stellar state (plan 13, P13.T5.d), so its row
/// carries no `stellar` key; a brown dwarf's is its own, from the stellar stage.
fn briefs_of(
    cache: &SharedBriefCache,
    key: GalaxyKey,
    galaxy: &Galaxy,
    hits: &[SystemHit],
    time: UniverseTime,
) -> Vec<Option<StellarBriefDto>> {
    hits.iter()
        .map(|hit| match hit.record().kind() {
            SystemKind::RoguePlanet => None,
            SystemKind::Stellar | SystemKind::BrownDwarf => cache
                .get_or_build(key, galaxy, hit.record())
                .brief_at(time)
                .map(|brief| brief_dto(&brief)),
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

    /// An observed query's rows, briefs and observations, are the same built whole or in four
    /// chunks appended in order, from an empty, a warm or no cache, and are the sim's own
    /// `range_query_observed` rows (P12.T6).
    #[test]
    fn observed_rows_do_not_depend_on_chunks_or_the_cache() {
        use hyperion_sim::galaxy::query::{QueryMode, range_query_observed};
        use hyperion_sim::observe::NoStarsCache;

        use crate::convert::observed_dto;

        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let centre = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
            .expect("in the root cube");
        let observer =
            hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 23_000.0, 0.0])
                .expect("in the root cube");
        let query = RangeQuery::builder(centre, LightYears::new(10.0))
            .mode(QueryMode::ObservedFrom(observer))
            .build()
            .expect("a query");
        let result = range_query(&galaxy, &mut NoCache::new(), &[], &query);
        let hits = result.systems();
        assert!(hits.len() > 8, "{} rows", hits.len());
        let work = RowWork {
            time: query.time(),
            include_stellar: true,
            observer: Some(Observer::new(observer, query.time()).expect("an observer")),
        };
        let briefs = SharedBriefCache::new(64 << 20);
        let warm = SharedSystemCache::new(64 << 20);
        let whole = work.run((&briefs, &warm), key, &galaxy, hits).unwrap();
        let sim =
            range_query_observed(&galaxy, &mut NoCache::new(), &mut NoStarsCache, &[], &query)
                .unwrap();
        let expected: Vec<_> = sim
            .observed()
            .iter()
            .map(|seen| observed_dto(seen.retardation(), seen.error()))
            .collect();
        assert_eq!(whole.observed.as_deref(), Some(&expected[..]));
        let expected_briefs: Vec<_> = sim
            .observed()
            .iter()
            .map(|seen| seen.brief_then().map(|brief| brief_dto(&brief)))
            .collect();
        assert_eq!(whole.briefs.as_deref(), Some(&expected_briefs[..]));
        for systems in [
            warm,
            SharedSystemCache::new(0),
            SharedSystemCache::new(64 << 20),
        ] {
            let mut chunked = RowExtras::with_capacity(work, hits.len());
            for chunk in hits.chunks(hits.len().div_ceil(4)) {
                chunked.append(work.run((&briefs, &systems), key, &galaxy, chunk).unwrap());
            }
            assert_eq!(chunked.observed, whole.observed);
            assert_eq!(chunked.briefs, whole.briefs);
        }
    }

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
