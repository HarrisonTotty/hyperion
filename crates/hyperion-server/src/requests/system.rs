//! The system's handlers: `system_summary` (plan 06, P06.T34, with plan 11's P11.T13 slice and
//! plan 12's observed mode, P12.T6.b), and `system_bodies`, `body_detail` and `body_events` (plan
//! 14, P14.T36).
//!
//! Each starts from [`openable_universe`](super::universe::openable_universe), as every handler
//! that names a universe does, checks the time and decodes the ID, and takes the universe's galaxy
//! from the [`GalaxyCache`](crate::compute::GalaxyCache). A summary is then answered on the CPU
//! pool from the [`SharedSystemCache`](crate::compute::SharedSystemCache), a rogue planet's, which
//! has no stars, from the body cache's context (P14.T27.b). The planetary kinds take
//! the system from the [`SharedBodyCache`](crate::compute::SharedBodyCache), generated there on a
//! miss from the stars the system cache holds, and are answered by a second pool job that evaluates
//! it at the request's time; `body_events`, served from generator version 25, takes the system's
//! event models from the same cache, built there on a miss from the cached system, between the
//! two.

use std::sync::Arc;

use hyperion_protocol::{
    BodyDetailRequest, BodyEventsRequest, RequestError, ResponseBody, SystemBodiesRequest,
    SystemSummaryDto, SystemSummaryRequest,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{ResolveSystemError, resolve};
use hyperion_sim::galaxy::query::QueryMode;
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::observe::{Drift, Observer, curvature_error, retarded};
use hyperion_sim::planetary::{HostKind, PlanetarySystem, ResolveBodyError, SystemContext};

use super::global_list::listing_refused;
use super::universe::openable_universe;
use super::viewer::is_viewer;
use crate::AppState;
use crate::compute::{
    CancelToken, GalaxyKey, GenerateBodiesError, GeneratedSystem, JobError, Listing, Priority,
};
use crate::convert::{
    BodiesRequest, DetailRequest, EventsRequest, MotionReads, SummaryRequest, SystemLine,
    body_detail, body_events, body_refusal, hosts_request, starless_summary,
    starless_summary_observed, system_bodies, system_summary, system_summary_observed,
    unknown_system, untraced, viewer_summary,
};

/// Every star of one system at the request's time, and the orbits that hold them; in observed mode
/// (plan 12, P12.T6.b), as the request's observer sees them then.
///
/// Resolving the ID, generating the system on a miss and summarising it at the time are one
/// interactive pool job: generation builds a track for every star, a millisecond or two for each
/// evolved one (ruling 46 of 2026-09-22), and even a summary from the cache classifies each star,
/// which is more than the runtime should carry. The job owns the state and takes the system cache
/// from it there, as the range query's job does with the cell cache. The small answer is
/// serialised on the runtime (design note 22).
///
/// In observed mode the same job reads the cached stars on the system's path ([`SystemLine`],
/// through the sim's `summary_observed_on`), a feature member's interior and the centre's orbits
/// from the server's caches, at
/// the time the light the observer receives at the request's time left the system, and states the
/// curvature error as a range row does; the emitted time is never held to the clock window, only
/// the request's time is.
///
/// A rogue planet has no stars, so the system cache refuses every ID of its layer: its summary is
/// [`starless`]'s, from the body cache, as its bodies' hosts are.
///
/// # Errors
///
/// In the order they are checked: those of [`openable_universe`] for a universe this server cannot
/// serve; `bad_request` naming `time` for a time outside the clock window; `unknown_system` naming
/// `system` for an ID whose bits are not a system ID; `bad_request` naming `mode` for an observer
/// that is not a canonical position inside the root cube; those of
/// [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built, and for a
/// stream's or a dwarf core's member those of
/// [`GlobalListService::listing`](crate::compute::GlobalListService::listing) if its galaxy's list
/// cannot be; `queue_full` if the interactive queue has no room for the job; then, on the job,
/// `unknown_system` naming `system` for an ID that plan 03's `resolve` refuses in this universe's
/// galaxy, or plan 10's `resolve_with` for a list member, or whose stars are not generated (a
/// member of the galactic centre), in either mode; in observed mode, `bad_request` naming `mode`
/// below generator version 27 for a list member or a catalogue system, whose motion the server
/// does not trace then ([`SystemLine::of`]); `internal` if the pool is shutting
/// down or the job panics; and `cancelled` if the client gave up before it ran. A rogue planet is
/// answered by [`starless`] (P14.T27.b).
pub(crate) async fn summary(
    state: Arc<AppState>,
    request: SystemSummaryRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = SummaryRequest::try_from(&request)?;
    let galaxy = state.galaxies.get(universe.key()).await?;
    let key = universe.key();
    if wanted.system().layer() == Some(Layer::RoguePlanet) {
        return starless(&state, (key, galaxy), request, wanted, token).await;
    }
    // A stream's or a dwarf core's member resolves through its galaxy's list (plan 10, P10.T10.b).
    let listing = state
        .global_lists
        .listing(key, wanted.system())
        .await
        .map_err(|error| listing_refused(wanted.system(), error))?;
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        // The cache's refusals come first, in either mode: a member of the galactic centre has no
        // stars generated.
        let stars = shared
            .systems
            .get_or_generate(key, &galaxy, wanted.system(), &listing)
            .map_err(|error| unknown_system(&request.system, error))?;
        let time = wanted.time();
        let answered = match wanted.mode() {
            QueryMode::Now => system_summary(request, &stars, time),
            QueryMode::ObservedFrom(position) => {
                let observer = Observer::new(position, time).expect(
                    "the request's observer is in the root cube and its time in the clock window",
                );
                // Lent so that a feature member's line does not build its interior again.
                let (interiors, orbits) = (
                    shared.cluster_models.handle(key),
                    shared.centre_orbits.handle(key),
                );
                let reads = MotionReads::new(&interiors, &orbits).with_listing(&listing);
                // A member of the galactic centre has no stars generated, and the cache has
                // refused it above; below generator version 27 a member of the global list and a
                // catalogue system are refused here, as no range query returns them (plan 10,
                // P10.T10.b).
                let line = SystemLine::of(&galaxy, reads, stars.record()).map_err(untraced)?;
                let (summary, retardation) = line.summary_observed(&galaxy, &stars, &observer);
                let error = curvature_error(&galaxy, observer.position(), &retardation);
                system_summary_observed(request, &stars, &summary, &retardation, error)
            }
        };
        Ok::<_, RequestError>(answered)
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::SystemSummary(answered))
}

/// A rogue planet's summary (P14.T27.b): its [`starless_summary`] from the context the body cache
/// holds ([`bodies_of`]), the hosts `system_bodies` sends with its bodies, so that the summary, the
/// frame and the bodies are answered from the same context.
///
/// In observed mode it is the starless summary at the time the light the observer receives left
/// the planet, with that light's `observed`, on the line `resolve_system` traces it on: a grid
/// system's, from its record alone (lean, 2026-11-01). A second interactive job answers either
/// mode; the answer is small and is serialised on the runtime.
///
/// # Errors
///
/// `unknown_system` naming `system` where [`bodies_of`] refuses the ID, in either mode: one that
/// `resolve` refuses; and `queue_full`, `internal` or `cancelled` from either job.
async fn starless(
    state: &Arc<AppState>,
    (key, galaxy): (GalaxyKey, Arc<Galaxy>),
    request: SystemSummaryRequest,
    wanted: SummaryRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let system = bodies_of(state, key, &galaxy, wanted.system())
        .await
        .map_err(|error| match error {
            GenerateBodiesError::NoSuchSystem(error) => unknown_system(&request.system, error),
            GenerateBodiesError::Compute(error) => error.into(),
        })?;
    let answer = move |_: &CancelToken| {
        let (ctx, _) = system.as_ref();
        let time = wanted.time();
        let answered = match wanted.mode() {
            QueryMode::Now => starless_summary(request, ctx, time),
            QueryMode::ObservedFrom(position) => {
                let observer = Observer::new(position, time).expect(
                    "the request's observer is in the root cube and its time in the clock window",
                );
                // The body cache holds only a system that resolved, and a grid system's line is
                // always traced, so neither refusal is reached.
                let record = resolve(&galaxy, ctx.id())
                    .map_err(|error| unknown_system(&request.system, error))?;
                let line = Drift::of_record(&galaxy, &record).map_err(untraced)?;
                let retardation = retarded(&observer, &line);
                let error = curvature_error(&galaxy, observer.position(), &retardation);
                starless_summary_observed(request, ctx, &retardation, error)
            }
        };
        Ok::<_, RequestError>(answered)
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::SystemSummary(answered))
}

/// Every body of one system at the request's time, degraded to the level asked for, with the
/// system's hosts and zones (plan 14, P14.T36.b).
///
/// The planetary system comes from the body cache, generated on a miss as one interactive pool
/// job shared by every request for it ([`bodies_of`]). A second interactive job then summarises the
/// hosts, evaluates every body at the time and converts the answer, which can be large (a belt's
/// members, once P14.T21 names them), so it is serialised on the pool too (design note 22).
///
/// # Errors
///
/// Those of [`summary`], in its order: the universe; `bad_request` naming `time`; `unknown_system`
/// naming `system`; the galaxy's; and `queue_full`, `internal` or `cancelled` from either job.
pub(crate) async fn bodies(
    state: Arc<AppState>,
    request: SystemBodiesRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = BodiesRequest::try_from(&request)?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    let system = bodies_of(&state, key, &galaxy, wanted.system())
        .await
        .map_err(|error| match error {
            GenerateBodiesError::NoSuchSystem(error) => unknown_system(&request.system, error),
            GenerateBodiesError::Compute(error) => error.into(),
        })?;
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        let (ctx, planets) = system.as_ref();
        let hosts = hosts_of(
            &shared,
            (key, &galaxy),
            (ctx, planets),
            &request,
            wanted.time(),
        )?;
        // The system's own seed: the universe's, but a viewer system's own (rendering plan R15).
        Ok::<_, RequestError>(system_bodies(wanted, hosts, ctx, planets, planets.seed()))
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::SystemBodies(Box::new(answered)))
}

/// The hosts of the planetary system `ctx` as `system_bodies` sends them at `time`: its stars'
/// summary from the system cache, or for a rogue planet, which has none, its starless summary
/// (P14.T27.b), so that an empty host list is an answer and never a refusal; and for a viewer
/// system (rendering plan R15), whose synthetic star no system cache holds, its context's star's
/// summary in the universe of its bodies' seed.
///
/// # Errors
///
/// `unknown_system` naming `system` where the system cache refuses the stars of a system with them,
/// which it never does for one `bodies_of` generated.
fn hosts_of(
    state: &AppState,
    (key, galaxy): (GalaxyKey, &Galaxy),
    (ctx, planets): (&SystemContext, &PlanetarySystem),
    request: &SystemBodiesRequest,
    time: hyperion_sim::time::UniverseTime,
) -> Result<SystemSummaryDto, RequestError> {
    if ctx.host_kind() == HostKind::RoguePlanet {
        return Ok(starless_summary(hosts_request(request), ctx, time));
    }
    if is_viewer(ctx.id()) {
        return Ok(viewer_summary(
            hosts_request(request),
            ctx,
            planets.seed(),
            time,
        ));
    }
    let stars = state
        .systems
        .get_or_generate(key, galaxy, ctx.id(), &Listing::Unlisted)
        .map_err(|error| unknown_system(&request.system, error))?;
    Ok(system_summary(hosts_request(request), &stars, time))
}

/// One body's whole record at the request's time, degraded to the level asked for (plan 14,
/// P14.T36.b).
///
/// The system is the body cache's, as for [`bodies`], and one interactive job evaluates the body
/// at the time; the answer is small and is serialised on the runtime.
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming `time`; on the field `body`,
/// `unknown_system` for a system part that is no system of this universe, `bad_request` for an
/// index outside plan 14's layout and `unknown_body` for an index at which the system holds no
/// planetary body; the galaxy's; and `queue_full`, `internal` or `cancelled` from either job.
pub(crate) async fn detail(
    state: Arc<AppState>,
    request: BodyDetailRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = DetailRequest::try_from(&request)?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    let system = bodies_of(&state, key, &galaxy, wanted.system())
        .await
        .map_err(|error| match error {
            GenerateBodiesError::NoSuchSystem(error) => {
                body_refusal(&request.body, ResolveBodyError::NoSuchSystem(error))
            }
            GenerateBodiesError::Compute(error) => error.into(),
        })?;
    let answer = move |_: &CancelToken| {
        let (ctx, planets) = system.as_ref();
        body_detail(request, wanted, ctx, planets)
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::BodyDetail(Box::new(answered)))
}

/// The events on one system's bodies in the request's window, each while its body is present, by
/// time and then by event ID (plan 14, P14.T36.b, from P14.T31.c's `events_between`).
///
/// The system is the body cache's, as for [`bodies`], and its event models the same cache's, built
/// on a miss as one interactive pool job shared by every request for them; a second interactive
/// job lists the window's events and converts them, and the answer, which can be large, is
/// serialised on the pool (design note 22). It is served from generator version 25, which puts
/// P14.T31.c's events on the record.
///
/// # Errors
///
/// In the order they are checked: those of [`openable_universe`]; `bad_request` naming `from` or
/// `to` for an end outside the clock window, and naming `to` for a window that ends before it
/// starts or is longer than 1,000 Julian years; `unknown_system` naming `system`, for an ID whose
/// bits are not a system ID and, on the job, for one that `resolve` refuses; the galaxy's; and
/// `queue_full`, `internal` or `cancelled` from any of the three jobs.
pub(crate) async fn events(
    state: Arc<AppState>,
    request: BodyEventsRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = EventsRequest::try_from(&request)?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    let refusal = |error| match error {
        GenerateBodiesError::NoSuchSystem(error) => unknown_system(&request.system, error),
        GenerateBodiesError::Compute(error) => error.into(),
    };
    let system = bodies_of(&state, key, &galaxy, wanted.system())
        .await
        .map_err(refusal)?;
    let models = state
        .bodies
        .events_of(key, wanted.system(), Arc::clone(&system))
        .await?;
    let answer = move |_: &CancelToken| body_events(request, wanted, &system.1, &models);
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
    Ok(ResponseBody::BodyEvents(answered))
}

/// The planetary system `id` of `galaxy`: the body cache's, or generated there.
///
/// A miss builds the context from the stars the system cache holds, resolving and generating them
/// there first if it holds none, so that the stars are evolved once for both caches. The context
/// is refused as [`SystemContext::for_system`] refuses the ID ([`SystemContext::for_stars`]). A
/// rogue planet, which has no stars for the system cache to hold, takes its context from
/// [`SystemContext::for_system`] itself (P14.T27.b). A viewer system (rendering plan R15) is the
/// server's viewer store's, refused `NoSuchSystem` once it is evicted, since its ID cannot be
/// generated again.
pub(crate) async fn bodies_of(
    state: &Arc<AppState>,
    key: GalaxyKey,
    galaxy: &Arc<Galaxy>,
    id: SystemId,
) -> Result<Arc<GeneratedSystem>, GenerateBodiesError> {
    // A viewer system is the server's store's, and no galaxy resolves its ID (rendering plan R15).
    if is_viewer(id) {
        return state
            .viewer
            .get(key, id)
            .ok_or(GenerateBodiesError::NoSuchSystem(
                ResolveSystemError::NoSuchSystem,
            ));
    }
    let shared = Arc::clone(state);
    let galaxy = Arc::clone(galaxy);
    state
        .bodies
        .get_or_generate(key, id, move || {
            match shared
                .systems
                .get_or_generate(key, &galaxy, id, &Listing::Unlisted)
            {
                Ok(stars) => SystemContext::for_stars(&galaxy, &stars),
                Err(ResolveSystemError::LayerNotGenerated(Layer::RoguePlanet)) => {
                    SystemContext::for_system(&galaxy, id)
                }
                Err(error) => Err(error),
            }
        })
        .await
}
