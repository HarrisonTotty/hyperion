//! The planet viewer's handlers (rendering plan R15): `viewer_planet`, `viewer_randomize` and
//! `viewer_inputs`.
//!
//! Each starts from [`openable_universe`], as every handler that names a universe does. A
//! `viewer_planet` checks its inputs, generates the planet's system on one interactive pool job
//! (`hyperion_sim::planetary::viewer::ViewerSystem::generate`, some milliseconds), and keeps it in
//! the server's [`ViewerSystems`](crate::compute::ViewerSystems), from which [`bodies_of`] and the
//! sky's host discs read it. A `viewer_randomize` takes the universe's galaxy and draws on one
//! interactive job, which builds the galaxy's density grid first if none is kept. A `viewer_inputs`
//! reads its planet's system as `body_detail` does, through [`bodies_of`], and its inputs back on
//! one interactive job.

use std::sync::Arc;

use hyperion_protocol::{
    ErrorCode, RequestError, ResponseBody, SeedHex, ViewerDrawDto, ViewerInputsRequest,
    ViewerPlanetRequest, ViewerRandomizeRequest, ViewerReadDto,
};
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::index::BodyIndex;
use hyperion_sim::planetary::viewer::{ViewerSystem, draw_inputs, inputs_of};
use hyperion_sim::planetary::{ResolveBodyError, viewer};

use super::bodies_of;
use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{CancelToken, GenerateBodiesError, JobError, Priority};
use crate::convert::{
    body_refusal, generate_refusal, inputs_dto, planet_inputs, unknown_body_system, viewer_planet,
};

/// Generates one planet from its inputs and keeps its system (the [module](self) documentation).
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming the first input outside its range
/// (`inputs.star_mass_msun` and so on), and naming `inputs.semi_major_axis_au` for an orbit that
/// leaves its system's reach or enters its star, or `inputs.star_mass_msun` for a star with no
/// disc; and `queue_full`, `internal` or `cancelled` from the job.
pub(crate) async fn planet(
    state: Arc<AppState>,
    request: ViewerPlanetRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let inputs = planet_inputs(&request.inputs)?;
    let key = universe.key();
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        let system = ViewerSystem::generate(inputs).map_err(|error| generate_refusal(&error))?;
        let answered = viewer_planet(request.universe, &system);
        let id = system.id();
        shared.viewer.insert(key, id, Arc::new(system.into_parts()));
        Ok::<_, RequestError>(answered)
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::ViewerPlanet(Box::new(answered)))
}

/// Draws a real planet's inputs from the universe's galaxy (the [module](self) documentation).
///
/// # Errors
///
/// Those of [`openable_universe`]; the galaxy's; `bad_request` naming `draw_seed` where the draw
/// finds no single star with a present planet in its tries; and `queue_full`, `internal` or
/// `cancelled` from the job.
pub(crate) async fn randomize(
    state: Arc<AppState>,
    request: ViewerRandomizeRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        let grid = shared.viewer.grid(key, &galaxy);
        let drawn = draw_inputs(&galaxy, &grid, request.draw_seed.to_u64()).map_err(|error| {
            RequestError {
                code: ErrorCode::BadRequest,
                message: error.to_string(),
                field: Some("draw_seed".to_owned()),
            }
        })?;
        let (system, index) = (drawn.source.system(), drawn.source.body_index());
        Ok::<_, RequestError>(ViewerDrawDto {
            universe: request.universe,
            draw_seed: SeedHex::from_u64(request.draw_seed.to_u64()),
            inputs: inputs_dto(drawn.inputs.raw()),
            source: hyperion_protocol::BodyIdHex::from_parts(system.raw(), index),
        })
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::ViewerRandomize(Box::new(answered)))
}

/// A served planet's inputs, read back (the [module](self) documentation).
///
/// # Errors
///
/// Those of [`openable_universe`]; on the field `body`, `unknown_system`, `bad_request` for an index
/// outside plan 14's layout and `unknown_body` for one at which the system holds no body, as
/// `body_detail` refuses them, and `bad_request` for a body that is not a planet; the galaxy's; and
/// `queue_full`, `internal` or `cancelled` from the jobs.
pub(crate) async fn inputs(
    state: Arc<AppState>,
    request: ViewerInputsRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let (system, index) = request.body.to_parts();
    let system =
        SystemId::from_raw(system).map_err(|error| unknown_body_system(&request.body, error))?;
    let index = BodyIndex::try_from(index)
        .map_err(|error| body_refusal(&request.body, ResolveBodyError::MalformedIndex(error)))?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    let generated = bodies_of(&state, key, &galaxy, system)
        .await
        .map_err(|error| match error {
            GenerateBodiesError::NoSuchSystem(error) => {
                body_refusal(&request.body, ResolveBodyError::NoSuchSystem(error))
            }
            GenerateBodiesError::Compute(error) => error.into(),
        })?;
    let answer = move |_: &CancelToken| {
        let (ctx, planets) = generated.as_ref();
        let Some(raw) = inputs_of(ctx, planets, index) else {
            return Err(match planets.body(index) {
                None => body_refusal(&request.body, ResolveBodyError::NoSuchBody),
                Some(_) => RequestError {
                    code: ErrorCode::BadRequest,
                    message: format!(
                        "body {} is no planet: the viewer reads a planet's inputs",
                        request.body.as_str()
                    ),
                    field: Some("body".to_owned()),
                },
            });
        };
        Ok(ViewerReadDto {
            universe: request.universe,
            body: request.body,
            inputs: inputs_dto(&raw),
            multiple_stars: ctx.stars().len() > 1,
        })
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::ViewerInputs(Box::new(answered)))
}

/// Whether `id` names a viewer system, which the server's store answers in place of the galaxy.
#[must_use]
pub(crate) fn is_viewer(id: SystemId) -> bool {
    viewer::is_viewer_system(id)
}
