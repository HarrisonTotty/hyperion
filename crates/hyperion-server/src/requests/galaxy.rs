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
use hyperion_sim::galaxy::global_list::{GlobalListSource, StreamSpec};
use hyperion_sim::galaxy::placement::{ResolveSystemError, SystemKind, SystemOrigin};
use hyperion_sim::galaxy::query::{QueryMode, RangeQuery, RangeResult, SystemHit};
use hyperion_sim::id::SystemIdKind;
use hyperion_sim::observe::{Observer, TraceMotionError};
use hyperion_sim::time::UniverseTime;

use super::global_list::tube_refused;
use super::observe::observe_row;
use super::universe::openable_universe;
use crate::compute::{
    CancelToken, EXTINCTION_FLOOR_LOG10_MAG, GalaxyKey, GlobalListService, JobError, LineGas,
    ListedTubes, Listing, Priority, RangeCaches, RowReads, ServedQuery, SharedBriefCache,
    SharedSystemCache, TubeTableCache, quantise_map, quantise_map_with_floor, served_range_query,
    with_row_reads,
};
use crate::convert::{
    ExtinctionMapQuery, ExtinctionQuery, LineEnd, ListMembers, MapRequest, MotionReads,
    RangeRequest, RowOrigins, RowPlacers, brief_dto, density_map,
    extinction_map as extinction_map_answer, extinction_result, galaxy_parameters, system_motion,
    systems_in_range, target_extinction, untraced,
};
use crate::limits::BRIEF_CHUNK_ROWS;
use crate::{AppState, FeatureMembers};

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
/// All the lines are one interactive pool job's: each system target is resolved by plan 03's
/// `resolve`, a stream's or a dwarf core's member's from generator version 27 through its galaxy's
/// list ([`targets_listing`]), and placed at the request's time by [`system_motion`], by its
/// origin, as every ID from a client is, and each line is the
/// [`SharedSightlineCache`](crate::compute::SharedSightlineCache)'s or is marched in the seed's own
/// gas at a fixed budget of steps, through one noise cache the job owns, and, once plan 09's feature
/// members are served, through the features' gas at the request's time (P09.T40.a). A target whose
/// ID names no
/// system, one of a kind or layer this generator does not place yet, or one `system_motion` does
/// not place (below generator version 27 a member of the galactic centre or of the global list, or
/// a catalogue system, each at every time, the epoch included; a tidal disruption's victim after
/// its disruption) is answered `no_such_system`, and the rest are still answered. The answer is small, so its frame is
/// serialised on the runtime.
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
    let listing = targets_listing(&state, (key, &galaxy), &wanted).await;
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        // Once the feature members are served, the lines take the features' gas at the request's
        // time, as the sky's do (P09.T40.a's gate); held, the smooth field alone, as before.
        let cells = shared.feature_cells.handle(key);
        let interiors = shared.cluster_models.handle(key);
        let orbits = shared.centre_orbits.handle(key);
        let reads = MotionReads::new(&interiors, &orbits).with_listing(&listing);
        let gas = match shared.feature_members {
            FeatureMembers::Held => LineGas::Smooth,
            FeatureMembers::Served => LineGas::Features {
                cells: &cells,
                t: wanted.time(),
            },
        };
        let mut lines = shared.sightlines.marcher(key, &galaxy, gas);
        let targets = wanted
            .ends()
            .iter()
            .map(|end| {
                let end = match *end {
                    LineEnd::Position(position) => position,
                    LineEnd::System(id) => match listing.resolve(&galaxy, id) {
                        Ok(record) => {
                            match system_motion(&galaxy, reads, &record, wanted.time()) {
                                Ok((position, _)) => position,
                                // Nor has a system the server does not place: below generator
                                // version 27 a centre member, a catalogue system (P09.T33.a) or a
                                // list member, refused at every time, and a tidal disruption's
                                // victim after its disruption, no system then.
                                Err(
                                    TraceMotionError::CentreOrbitNotBuilt(_)
                                    | TraceMotionError::CatalogueMotionNotTraced(_)
                                    | TraceMotionError::GlobalListMember(_)
                                    | TraceMotionError::Disrupted(_),
                                ) => {
                                    return TargetExtinction::NoSuchSystem;
                                }
                            }
                        }
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

/// What the extinction request's system targets resolve through: from generator version 27, when
/// a range query returns the global list's members, the galaxy's list if a target is a stream's or
/// a dwarf core's member, with each such target's table built ([`GlobalListService::listing`]);
/// otherwise nothing, and a list member's target is no system, as plan 03's `resolve` answers it.
///
/// A list or table that cannot be built is logged, and a target that then resolves through none is
/// answered `no_such_system`; the rest are still answered.
async fn targets_listing(
    state: &AppState,
    (key, galaxy): (GalaxyKey, &Galaxy),
    wanted: &ExtinctionQuery,
) -> Listing {
    let mut listing = Listing::Unlisted;
    if !GlobalListSource::is_registered_in(galaxy) {
        return listing;
    }
    for end in wanted.ends() {
        let LineEnd::System(id) = *end else {
            continue;
        };
        if !matches!(
            id.kind(),
            SystemIdKind::Stream(_) | SystemIdKind::DwarfCore(_)
        ) {
            continue;
        }
        match state.global_lists.listing(key, id).await {
            Ok(listed) => listing = listed,
            Err(error) => {
                tracing::warn!(%error, system = id.raw(), "an extinction target's list was not built");
            }
        }
    }
    listing
}

/// The systems within the request's radius of its centre at its time, with the census, with each
/// row's stellar brief when the request sets `include_stellar` (plan 06, P06.T34), and in observed
/// mode with what the request's observer receives from each (plan 12, P12.T6).
///
/// The query and the conversion of its hits are one interactive pool job, which takes its
/// [`CellCacheHandle`](crate::compute::CellCacheHandle) there: the handle borrows the cache and so
/// cannot cross an `.await`, and the conversion is cheap beside the query but too dear for the
/// runtime, since a 20,000-record answer is megabytes of wire types. A cold query's cells are
/// generated before it on the pool's workers, by jobs of their own that this request's task queues
/// and waits for, never one job waiting on another ([`served_range_query`]; P09.T40.a), and the
/// query then walks the warm caches. The frame that
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
/// Once P09.T40.a's gate is open ([`FeatureMembers::Served`]), the query takes plan 09's feature
/// members from a [`FeatureMemberSource`] beside the grid, reading the server's feature cells and
/// interiors, and from generator version 27 the catalogue classes' hosts beside them (P09.T37.a)
/// and plan 10's global list's members, the galactic centre's among them (P10.T10.b's registration,
/// P09.T37.b), and every row carries its origin, a member's with its class and its own velocity
/// ([`RowOrigins::Sent`]); every cache holds epoch state alone, so they change no byte either.
/// The list is built on the pool before the query if no request has built it, with the tube tables
/// of the streams the query reads ([`list_for`]): the answer waits for them rather than leave them
/// out, so it is the same whatever the server has built. Held, the default until P09.T2.c's
/// version, the grid is the whole answer, as in plan 04.
///
/// From 27 a row of a member of the galactic centre carries no brief, since the server generates
/// no centre member's stars yet (lean, 2026-11-01); a list member's is its member record's stars',
/// through the query's list. Observed mode reads every row on its path (`observe_row`): from 27 a
/// centre member on its orbit, a list member on its line and a class host on its broken line.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve, `bad_request` naming
/// `time`, `centre`, `radius_ly`, `limit` or `mode` for a field it cannot use (design note 24),
/// those of [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built,
/// those of [`list_for`] if its global list or a tube table it reads cannot be, `queue_full` if
/// the interactive queue has no room for the query (a chunk, queued after the query has run, waits
/// for room instead), `bad_request` naming `mode` if the observed answer would hold a system whose
/// motion the server does not trace ([`SystemLine::of`](crate::convert::SystemLine::of), which
/// below 27 no query returns), `internal` if the pool is shutting down or a job panics, and
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
    let served = served_query(&state, (&galaxy, key), &query).await?;
    // Observed mode finds the same systems (plan 12, Design note 4).
    let queried = move |job_galaxy: &Galaxy, result: RangeResult, reads: &RowReads<'_>| {
        if work.is_needed() && result.systems().len() > BRIEF_CHUNK_ROWS {
            return Ok(Queried::Chunked(Box::new((request, query, result))));
        }
        let extras = work
            .run(
                (&shared.briefs, &shared.systems, reads.motion()),
                key,
                job_galaxy,
                result.systems(),
            )
            .map_err(untraced)?;
        Ok::<_, RequestError>(Queried::Answered(systems_in_range(
            job_galaxy,
            request,
            &query,
            &result,
            extras.briefs,
            extras.observed,
            row_origins(shared.feature_members, reads),
        )))
    };
    let caches = range_caches(&state);
    let answered = served_range_query(&state.pool, &caches, &served, &token, queried).await?;
    let (request, query, result) = match answered? {
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
        let (chunk_caches, chunk_served) = (caches.clone(), served.clone());
        // The query has run, so a chunk waits for room in the queue rather than refusing it.
        chunks.push(
            state
                .pool
                .submit(
                    Priority::Interactive,
                    token.clone(),
                    move |_: &CancelToken| {
                        // The query's own reads, so that its list's members and the centre's are
                        // resolved as its rows place them.
                        with_row_reads(&chunk_caches, &chunk_served, |reads| {
                            let caches = (&shared.briefs, &shared.systems, reads.motion());
                            work.run(caches, key, &galaxy, &result.systems()[start..end])
                        })
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
    let members = state.feature_members;
    let convert = move |_: &CancelToken| {
        with_row_reads(&caches, &served, |reads| {
            systems_in_range(
                &galaxy,
                request,
                &query,
                &result,
                extras.briefs,
                extras.observed,
                row_origins(members, reads),
            )
        })
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

/// The caches a range query reads, as the server holds them.
#[must_use]
fn range_caches(state: &AppState) -> RangeCaches {
    RangeCaches::new(
        Arc::clone(&state.cells),
        Arc::clone(&state.feature_cells),
        Arc::clone(&state.cluster_models),
        (
            Arc::clone(&state.class_cells),
            Arc::clone(&state.centre_orbits),
        ),
    )
}

/// `query` in `galaxy`, the galaxy `key` names, as the server serves it: with the feature members
/// its gate says, and from generator version 27 the global list's ([`list_for`]).
///
/// # Errors
///
/// As [`list_for`].
async fn served_query(
    state: &AppState,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    query: &RangeQuery,
) -> Result<ServedQuery, RequestError> {
    let served = ServedQuery::new(
        Arc::clone(galaxy),
        key,
        query.clone(),
        state.feature_members,
    );
    let lists = (&state.global_lists, &state.tube_tables);
    Ok(
        match list_for(lists, state.feature_members, (galaxy, key), query).await? {
            Some(list) => served.with_list(list),
            None => served,
        },
    )
}

/// What a range answer's rows say of what placed them under the gate `members`: nothing while it
/// holds the feature members back, and every row's origin once it serves them, a member's resolved
/// through `reads`, the query's interiors and its global list's source (P09.T40.a, P10.T10.b).
#[must_use]
fn row_origins<'a>(members: FeatureMembers, reads: &RowReads<'a>) -> RowOrigins<'a> {
    match members {
        FeatureMembers::Held => RowOrigins::Unsent,
        FeatureMembers::Served => RowOrigins::Sent(RowPlacers::new(
            reads.interiors(),
            reads.orbits(),
            reads.list(),
        )),
    }
}

/// The global list a range query of `query` in `galaxy`, the galaxy `key` names, reads from
/// `lists`, the server's service and its tube tables, with the table of every stream it reads built
/// on the pool's workers and kept first ([`GlobalListSource::streams_read_with`]): from generator
/// version 27, whose query takes the list's members, the galactic centre's among them (P10.T10.b's
/// registration, P09.T37.b), and `None` below it or while `members` holds the feature members back,
/// when the query takes none.
///
/// The list and the tables are built at interactive priority, the query's own, the tables as
/// `stream_track` builds one, a few at once
/// ([`GlobalListService::tubes`](crate::compute::GlobalListService::tubes)): some 0.2–0.5 s a
/// stream on the pool, each once, and none for a query a warm-up has gone before. A build the
/// open's warm-up or a sky started at bulk priority is joined and raised to it, so that a range
/// query sent while a sky's jobs run is answered first, cold as warm (R06.T11.d). The query then
/// reads every table warm, since a job never waits on another. The query's source keeps each
/// table's envelope with the list's models as it reads it, and the warm-up as it builds it, so a
/// stream whose shell a later query meets and whose envelope it misses (111 of the 121 about the
/// Sun-like point, perf-range-census-streams) needs no table, even one the cache has evicted
/// since.
///
/// # Errors
///
/// As [`GlobalListService::get`](crate::compute::GlobalListService::get) for the list, and for a
/// table as `stream_track` refuses it: a pool's error as its jobs', and `internal` for a stream the
/// sim cannot build a tube for.
pub(crate) async fn list_for(
    (lists, tubes): (&GlobalListService, &Arc<TubeTableCache>),
    members: FeatureMembers,
    (galaxy, key): (&Arc<Galaxy>, GalaxyKey),
    query: &RangeQuery,
) -> Result<Option<ListedTubes>, RequestError> {
    if members == FeatureMembers::Held || !GlobalListSource::is_registered_in(galaxy) {
        return Ok(None);
    }
    let held = lists.get(key, Priority::Interactive).await?;
    let read = GlobalListSource::streams_read_with(galaxy, held.list(), held.models(), query);
    let specs: Vec<&StreamSpec> = read
        .iter()
        .map(|&number| {
            held.list()
                .stream(number)
                .expect("the list names its own streams")
        })
        .collect();
    lists
        .tubes((galaxy, key), &specs, Priority::Interactive)
        .await
        .map_err(|(number, error)| tube_refused(number, &error))?;
    Ok(Some(ListedTubes::new(
        (Arc::clone(galaxy), key),
        held,
        Arc::clone(tubes),
    )))
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
    #[must_use]
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
    #[must_use]
    fn is_needed(self) -> bool {
        self.include_stellar || self.observer.is_some()
    }

    /// The briefs and observations of `hits`, in order, in the galaxy `key` names.
    ///
    /// In `now` the briefs are the brief cache's at the query's time, a list member's from its
    /// member record through the query's list; in observed mode each row is read at its retarded
    /// time on its path ([`observe_row`]), through the query's reads, and its brief is the one
    /// then.
    fn run(
        self,
        caches: (&SharedBriefCache, &SharedSystemCache, MotionReads<'_>),
        key: GalaxyKey,
        galaxy: &Galaxy,
        hits: &[SystemHit],
    ) -> Result<RowExtras, TraceMotionError> {
        let (briefs, systems, reads) = caches;
        let Some(observer) = self.observer else {
            let briefs = self
                .include_stellar
                .then(|| briefs_of(briefs, (key, galaxy), reads.list(), hits, self.time));
            return Ok(RowExtras {
                briefs,
                observed: None,
            });
        };
        let mut stars = systems.handle(key);
        let mut extras = RowExtras::with_capacity(self, hits.len());
        for hit in hits {
            let (reading, brief_then) = observe_row(galaxy, reads, &mut stars, hit, &observer)?;
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
    #[must_use]
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
pub(super) async fn run<T: Send + 'static>(
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

/// The wire briefs of `hits` at `time`, in order, from `cache`'s models of `galaxy`, the galaxy
/// `key` names, a stream's or a dwarf core's member's through `list`, the query's.
///
/// A rogue planet has no brief, since it has no stellar state (plan 13, P13.T5.d), so its row
/// carries no `stellar` key; a brown dwarf's is its own, from the stellar stage. Nor has a
/// catalogue system whose class does not model its stars, a Type Ia's (plan 09, P09.T34.a), which
/// the brief cache refuses, nor a member of the galactic centre, whose stars the server does not
/// generate yet. A list member's is its member record's stars' (plan 10, P10.T10.b's registration
/// member), as `system_summary`'s.
fn briefs_of(
    cache: &SharedBriefCache,
    (key, galaxy): (GalaxyKey, &Galaxy),
    list: Option<&dyn ListMembers>,
    hits: &[SystemHit],
    time: UniverseTime,
) -> Vec<Option<StellarBriefDto>> {
    hits.iter()
        .map(|hit| match hit.record().kind() {
            SystemKind::RoguePlanet => None,
            // A member of the galactic centre has no brief yet (the handler's documentation, lean
            // 2026-11-01): the server generates no centre member's stars.
            SystemKind::Stellar | SystemKind::BrownDwarf
                if matches!(hit.record().origin(), SystemOrigin::CentreMember { .. }) =>
            {
                None
            }
            // Only a query that took the list returns its members.
            SystemKind::Stellar | SystemKind::BrownDwarf
                if matches!(hit.record().origin(), SystemOrigin::GlobalListMember { .. }) =>
            {
                let list = list.expect("only the global list's source places a list member");
                let member = list
                    .list_member(galaxy, hit.id())
                    .expect("a list member found by the range query resolves through its list");
                cache
                    .get_or_build_listed(key, galaxy, &member)
                    .brief_at(time)
                    .map(|brief| brief_dto(&brief))
            }
            SystemKind::Stellar | SystemKind::BrownDwarf => {
                match cache.get_or_build(key, galaxy, hit.record()) {
                    Ok(model) => model.brief_at(time).map(|brief| brief_dto(&brief)),
                    // The cache refuses only a class's stars, `KindNotGenerated`; every refusal is
                    // named, so that a new one is a decision here, and none is a row's error.
                    Err(
                        ResolveSystemError::KindNotGenerated
                        | ResolveSystemError::NoSuchSystem
                        | ResolveSystemError::LayerNotGenerated(_),
                    ) => None,
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod placed;

#[cfg(test)]
mod registration;

#[cfg(test)]
mod tests {
    use hyperion_sim::galaxy::placement::{NoCache, resolve};
    use hyperion_sim::galaxy::query::{RangeQuery, range_query};
    use hyperion_sim::id::SystemId;
    use hyperion_sim::units::LightYears;
    use hyperion_sim::{GENERATOR_VERSION, Seed};

    use super::*;

    /// A catalogue system's row (plan 09): a Type Ia's members have no brief, since their class
    /// does not model their stars (P09.T34.a), where a core collapse's has its stars'; and from
    /// generator version 27, when the range query serves the classes (P09.T37.a), the observed row
    /// traces either's line.
    #[test]
    fn a_catalogue_system_s_row_has_its_class_s_brief_and_its_traced_line() {
        use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
        use hyperion_sim::galaxy::catalogue_classes::type_ia::TypeIaProcess;
        use hyperion_sim::galaxy::catalogue_classes::{
            CatalogueCellKey, CatalogueClassCell, ClassId,
        };

        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let cell = |class: ClassId, cell: [i32; 3]| {
            CatalogueCellKey::new(class, cell).expect("a cell of the root cube")
        };
        let hit = |id: SystemId| {
            let record = resolve(&galaxy, id).expect("a member resolves");
            SystemHit::new(record, *record.epoch_position(), LightYears::ZERO)
        };
        let ia = TypeIaProcess::new(&galaxy);
        let entry = [[0, 0, 0], [-1, -1, -1], [0, 0, -1], [-1, 0, 0]]
            .into_iter()
            .flat_map(|c| {
                CatalogueClassCell::generate(&galaxy, &ia, cell(ClassId::TYPE_IA, c)).into_entries()
            })
            .find(|entry| entry.members() == 2)
            .expect("the centre has a recent Type Ia with a survivor");
        let core_collapses = CoreCollapseProcess::new(&galaxy);
        let collapse = (8..28)
            .flat_map(|y| {
                let key = cell(ClassId::CORE_COLLAPSE, [0, y, 0]);
                CatalogueClassCell::generate(&galaxy, &core_collapses, key).into_entries()
            })
            .next()
            .expect("the inner disc has recent deaths");
        let mut hits: Vec<SystemHit> = (0..entry.members())
            .map(|member| hit(SystemId::from(entry.member_id(member).unwrap())))
            .collect();
        hits.push(hit(SystemId::from(collapse.id())));
        let time = UniverseTime::default();
        let briefs = SharedBriefCache::new(64 << 20);
        let rows = briefs_of(&briefs, (key, &galaxy), None, &hits, time);
        assert_eq!(rows[..2], [None, None]);
        assert!(
            rows[2].is_some(),
            "a core collapse's progenitor shines at the epoch"
        );
        assert_eq!(briefs.counters().entries(), 1);
        let systems = SharedSystemCache::new(64 << 20);
        let observer =
            Observer::new(*hits[0].record().epoch_position(), time).expect("an observer");
        for hit in &hits {
            assert!(
                observe_row(
                    &galaxy,
                    MotionReads::uncached(),
                    &mut systems.handle(key),
                    hit,
                    &observer
                )
                .is_ok(),
                "{:?}",
                hit.id()
            );
        }
    }

    /// An observed query's rows, briefs and observations, are the same built whole or in four
    /// chunks appended in order, from an empty, a warm, a tight, a `system_summary`-filled or no
    /// cache, and are the sim's own `range_query_observed` rows (P12.T6). The second query asks
    /// for the substellar layers, so that a rogue planet, read on its own line, and a brown dwarf,
    /// whose stars [`SharedSystemCache::get_or_build`] builds, are held to the sim too.
    #[test]
    fn observed_rows_do_not_depend_on_chunks_or_the_cache() {
        use hyperion_sim::galaxy::query::{QueryMode, SubstellarRequest, range_query_observed};
        use hyperion_sim::observe::NoStarsCache;

        use crate::convert::observed_dto;

        /// The substellar query's radius: the rogue planets are dense enough that a few light-years
        /// hold dozens, and the radius is wide enough to hold a brown dwarf too.
        const SUBSTELLAR_RADIUS_LY: f64 = 3.0;

        let galaxy = Galaxy::new(Seed::new(0x4d2));
        let key = GalaxyKey::new(0x4d2, GENERATOR_VERSION);
        let centre = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
            .expect("in the root cube");
        let observer =
            hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 23_000.0, 0.0])
                .expect("in the root cube");
        let stellar = RangeQuery::builder(centre, LightYears::new(10.0));
        let substellar = RangeQuery::builder(centre, LightYears::new(SUBSTELLAR_RADIUS_LY))
            .substellar(SubstellarRequest::BrownDwarfsAndRoguePlanets);
        for builder in [stellar, substellar] {
            let query = builder
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
            let whole = work
                .run(
                    (&briefs, &warm, MotionReads::uncached()),
                    key,
                    &galaxy,
                    hits,
                )
                .unwrap();
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
            // The system cache as `system_summary` fills it, by ID, for every system with stars.
            let summarised = SharedSystemCache::new(64 << 20);
            for hit in hits {
                if hit.record().kind() != SystemKind::RoguePlanet {
                    let _ = summarised
                        .get_or_generate(key, &galaxy, hit.id(), &Listing::Unlisted)
                        .expect("a system found resolves");
                }
            }
            for systems in [
                warm,
                summarised,
                SharedSystemCache::new(0),
                SharedSystemCache::new(100_000),
                SharedSystemCache::new(64 << 20),
            ] {
                let mut chunked = RowExtras::with_capacity(work, hits.len());
                for chunk in hits.chunks(hits.len().div_ceil(4)) {
                    chunked.append(
                        work.run(
                            (&briefs, &systems, MotionReads::uncached()),
                            key,
                            &galaxy,
                            chunk,
                        )
                        .unwrap(),
                    );
                }
                assert_eq!(chunked.observed, whole.observed);
                assert_eq!(chunked.briefs, whole.briefs);
            }
            if query.substellar() == SubstellarRequest::BrownDwarfsAndRoguePlanets {
                let count = |kind| {
                    hits.iter()
                        .filter(|hit| hit.record().kind() == kind)
                        .count()
                };
                assert!(
                    count(SystemKind::RoguePlanet) > 0 && count(SystemKind::BrownDwarf) > 0,
                    "{} rogue planets and {} brown dwarfs",
                    count(SystemKind::RoguePlanet),
                    count(SystemKind::BrownDwarf)
                );
            }
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
            (key, &galaxy),
            None,
            hits,
            query.time(),
        );
        assert!(whole.iter().all(Option::is_some));
        let warm = SharedBriefCache::new(64 << 20);
        let _ = briefs_of(&warm, (key, &galaxy), None, hits, query.time());
        for cache in [
            warm,
            SharedBriefCache::new(0),
            SharedBriefCache::new(10_000),
        ] {
            let quarter = hits.len().div_ceil(4);
            let chunked: Vec<_> = hits
                .chunks(quarter)
                .flat_map(|chunk| briefs_of(&cache, (key, &galaxy), None, chunk, query.time()))
                .collect();
            assert_eq!(chunked, whole);
        }
    }
}
