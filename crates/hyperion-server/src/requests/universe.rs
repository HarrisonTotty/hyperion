//! The universe lifecycle's handlers, `create_universe`, `list_universes` and `open_universe`
//! (plan 04, P04.T14.a), and [`openable_universe`], the lookup that every request naming a universe
//! starts from.

use std::sync::Arc;

use hyperion_protocol::{
    CreateUniverseRequest, OpenUniverseRequest, RequestError, ResponseBody, SeedHex, UniverseIdHex,
    UniverseInfo,
};

use crate::AppState;
use crate::convert::{NewUniverse, universe_list};
use crate::universe::{Universe, UniverseId};

/// Creates a universe from the name and the seed asked for, or a drawn seed, and saves it.
///
/// Cancelling the request does not undo the create: the registry finishes it on a blocking task
/// and logs it there, so that a universe made after its requester gave up is still logged.
///
/// # Errors
///
/// `bad_request` naming `name` for a name that breaks the rules of design note 16, `name_taken`
/// (also naming `name`), `universe_limit_reached`, `storage_failed` when the save cannot be
/// written, and `internal` for a failure of the server's own.
pub(crate) async fn create(
    state: Arc<AppState>,
    request: CreateUniverseRequest,
) -> Result<ResponseBody, RequestError> {
    let (name, seed) = NewUniverse::try_from(request)?.into_parts();
    let universe = state.registry.create(name, seed).await?;
    Ok(ResponseBody::CreateUniverse(UniverseInfo::from(
        universe.as_ref(),
    )))
}

/// Every universe, sorted by name and then by ID, each with its status.
#[must_use]
pub(crate) fn list(state: &AppState) -> ResponseBody {
    ResponseBody::ListUniverses(universe_list(&state.registry.list()))
}

/// Opens a universe: checks that this server can run it, warms its galaxy and answers with its
/// identity.
///
/// Design note 6 has `open` mean "check it, load it, warm its galaxy and tell me about it", so the
/// galaxy is built before this answers and the requests that follow find it in the cache. A galaxy
/// takes about 130 ms to build, which is what `open` costs the first time a universe is opened
/// under this server.
///
/// # Errors
///
/// Those of [`openable_universe`], and those of
/// [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built: a full
/// interactive queue is `queue_full`, and a build the client cancelled is `cancelled`.
pub(crate) async fn open(
    state: Arc<AppState>,
    request: OpenUniverseRequest,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    state.galaxies.get(universe.key()).await?;
    tracing::info!(
        id = %universe.id(),
        name = %universe.name(),
        seed = %SeedHex::from_u64(universe.seed()),
        generator_version = %universe.generator_version(),
        "opened a universe"
    );
    Ok(ResponseBody::OpenUniverse(UniverseInfo::from(
        universe.as_ref(),
    )))
}

/// The universe `id` names, if this server can open and query it.
///
/// Every request that names a universe looks it up here first, before checking its other fields,
/// so that each refuses a universe it cannot serve alike.
///
/// # Errors
///
/// `unknown_universe` naming the `universe` field when no save has the ID,
/// `generator_version_mismatch` with both versions in the message for a save made under another
/// generator version (design note 18), and `unsupported_save_format` for a save in a later
/// format.
pub(crate) fn openable_universe(
    state: &AppState,
    id: &UniverseIdHex,
) -> Result<Arc<Universe>, RequestError> {
    state
        .registry
        .open(UniverseId::from(id))
        .map_err(RequestError::from)
}
