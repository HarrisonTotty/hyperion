//! The system's handlers: `system_summary` (plan 06, P06.T34, with plan 11's P11.T13 slice), and
//! `system_bodies` and `body_detail` (plan 14, P14.T36; `body_events` waits for P14.T31).
//!
//! Each starts from [`openable_universe`](super::universe::openable_universe), as every handler
//! that names a universe does, checks the time and decodes the ID, and takes the universe's galaxy
//! from the [`GalaxyCache`](crate::compute::GalaxyCache). A summary is then answered on the CPU
//! pool from the [`SharedSystemCache`](crate::compute::SharedSystemCache). The planetary kinds take
//! the system from the [`SharedBodyCache`](crate::compute::SharedBodyCache), generated there on a
//! miss from the stars the system cache holds, and are answered by a second pool job that evaluates
//! it at the request's time.

use std::sync::Arc;

use hyperion_protocol::{
    BodyDetailRequest, RequestError, ResponseBody, SystemBodiesRequest, SystemSummaryRequest,
};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::{ResolveBodyError, SystemContext};

use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{
    CancelToken, GalaxyKey, GenerateBodiesError, GeneratedSystem, JobError, Priority,
};
use crate::convert::{
    BodiesRequest, DetailRequest, SummaryRequest, body_detail, body_refusal, hosts_request,
    system_bodies, system_summary, unknown_system,
};

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
        let stars = shared
            .systems
            .get_or_generate(key, &galaxy, wanted.system())
            .map_err(|error| unknown_system(&request.system, error))?;
        let hosts = system_summary(hosts_request(&request), &stars, wanted.time());
        let (ctx, planets) = system.as_ref();
        Ok::<_, RequestError>(system_bodies(
            wanted,
            hosts,
            ctx,
            planets,
            Seed::new(key.seed()),
        ))
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::SystemBodies(Box::new(answered)))
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

/// The planetary system `id` of `galaxy`: the body cache's, or generated there.
///
/// A miss builds the context from the stars the system cache holds, resolving and generating them
/// there first if it holds none, so that the stars are evolved once for both caches
/// ([`SystemContext::from_stars`]).
async fn bodies_of(
    state: &Arc<AppState>,
    key: GalaxyKey,
    galaxy: &Arc<Galaxy>,
    id: SystemId,
) -> Result<Arc<GeneratedSystem>, GenerateBodiesError> {
    let shared = Arc::clone(state);
    let galaxy = Arc::clone(galaxy);
    state
        .bodies
        .get_or_generate(key, id, move || {
            let stars = shared.systems.get_or_generate(key, &galaxy, id)?;
            Ok(SystemContext::from_stars(&galaxy, &stars))
        })
        .await
}
