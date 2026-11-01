//! Plan 12's observation handlers (P12.T6): `resolve_system`, one system by its ID now or as an
//! observer receives it, and the reading of one row at its retarded time, which the observed range
//! query shares.
//!
//! Every system's stars come from the [`SharedSystemCache`](crate::compute::SharedSystemCache),
//! lent as the sim's `StarsCache`: the brief at the emitted time needs them, and building them
//! is the whole cost of an observed row that the cache has not seen (P12.T3 as built).

use std::sync::Arc;

use hyperion_protocol::{ObservedDto, RequestError, ResolveSystemRequest, ResponseBody};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::members::source::FeatureInteriorCache;
use hyperion_sim::galaxy::placement::{SystemKind, SystemOrigin};
use hyperion_sim::galaxy::query::{QueryMode, SystemHit};
use hyperion_sim::observe::{
    Drift, Observer, StarsCache, TraceMotionError, curvature_error, observe_hit_in, retarded,
};
use hyperion_sim::stellar::system::StellarBrief;
use hyperion_sim::units::LightYears;

use super::global_list::listing_refused;
use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{CancelToken, JobError, Priority, SystemStarsHandle};
use crate::convert::{
    ResolveRequest, observed_dto, resolved_system, system_motion, unknown_system, untraced_system,
};

/// One system by its ID at the request's time: its range row, and in observed mode what the
/// request's observer receives from it (plan 12, P12.T6).
///
/// Resolving the ID, placing the system at the time and, in observed mode, reading it at its
/// retarded time are one interactive pool job: the reading may build the system's stars, a
/// millisecond or two for each evolved star (ruling 46 of 2026-09-22), which the system cache then
/// keeps for `system_summary` too. The ID goes through plan 03's `resolve` before use, as every ID
/// from a client does (plan 04's "Extending the convention"), or a stream's or a dwarf core's
/// member's through plan 10's `resolve_with` and its galaxy's list (P10.T10.b). The row is the
/// system as a range query finds it, at its present position; the answer is small and is serialised
/// on the runtime.
///
/// # Errors
///
/// Those of [`openable_universe`] for a universe this server cannot serve; `bad_request` naming
/// `time` for a time outside the clock window (the light's emission time is never checked);
/// `unknown_system` naming `system` for an ID whose bits are not a system ID, or that `resolve`
/// refuses in this universe's galaxy, as `system_summary` answers it, and for a system whose motion
/// the sim cannot trace yet, in either mode (a member of the galactic centre until plan 09's
/// P09.T28, which `system_summary` refuses too, a catalogue system, whose line breaks at its
/// explosion, P09.T33.a, and a member of the global list); `bad_request` naming `mode` for an
/// observer that is not a canonical position inside the root cube; those of
/// [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be built, and of
/// [`GlobalListService::listing`](crate::compute::GlobalListService::listing) if a list member's
/// galaxy's list cannot be; `queue_full` if the interactive queue has no room for the job;
/// `internal` if the pool is shutting down or the job panics; and `cancelled` if the client gave up
/// before it ran.
pub(crate) async fn resolve_system(
    state: Arc<AppState>,
    request: ResolveSystemRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = ResolveRequest::try_from(&request)?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    // A stream's or a dwarf core's member resolves through its galaxy's list (plan 10, P10.T10.b).
    let listing = state
        .global_lists
        .listing(key, wanted.system())
        .await
        .map_err(|error| listing_refused(wanted.system(), error))?;
    let shared = Arc::clone(&state);
    let answer = move |_: &CancelToken| {
        let record = listing
            .resolve(&galaxy, wanted.system())
            .map_err(|error| unknown_system(&request.system, error))?;
        // Lent so that a feature member's lines here and below do not build its interior again.
        let interiors = shared.cluster_models.handle(key);
        let time = wanted.time();
        // A grid system is placed as the range query places it, bit for bit; a feature member moves
        // on its own line, which plan 08's draw is not. A centre member is refused at every time,
        // as `system_summary` refuses it. A catalogue system, whose line breaks at its explosion,
        // is refused at every time too, and so is a member of the global list, whose line the sim
        // does not trace yet.
        let (position, velocity) = system_motion(&galaxy, &interiors, &record, time)
            .map_err(|error| untraced_system(&request.system, error))?;
        // The system's own sphere: its hit is at its present position, no distance from the centre.
        let hit = SystemHit::new(record, position, LightYears::ZERO);
        let observed = match wanted.mode() {
            QueryMode::Now => None,
            QueryMode::ObservedFrom(position) => {
                let observer = Observer::new(position, time).expect(
                    "the request's observer is in the root cube and its time in the clock window",
                );
                let mut stars = shared.systems.handle(key);
                let (reading, _) = observe_row(&galaxy, &interiors, &mut stars, &hit, &observer)
                    .map_err(|error| untraced_system(&request.system, error))?;
                Some(reading)
            }
        };
        Ok::<_, RequestError>(resolved_system(
            &galaxy,
            request,
            time,
            (&hit, velocity),
            observed,
        ))
    };
    let receiver = state
        .pool
        .try_submit(Priority::Interactive, token, answer)?;
    let answered = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))??;
    Ok(ResponseBody::ResolveSystem(Box::new(answered)))
}

/// What `observer` receives from the system `hit`: the wire's observation, and the system's brief
/// when the light left it, `None` if it was not yet born then or has no stars.
///
/// A system with stars is read by the sim's `observe_hit_in` over the stars `stars` lends, a
/// feature member's interior from `interiors`, which is
/// row for row what the sim's `range_query_observed` gives it (P12.T3's `query::mode` test). A
/// rogue planet has no stars, and a member of the galactic centre none generated yet, so each is
/// read on its own line alone, as `range_query_observed` reads a rogue planet: its retardation and
/// its stated error, and no brief. A catalogue system's line is refused there too, before its
/// stars are built, as `range_query_observed` refuses it.
///
/// # Errors
///
/// [`TraceMotionError`] for a system whose motion the sim cannot trace yet: a member of the
/// galactic centre until plan 09's P09.T28 builds its orbit, and a catalogue system, whose line
/// breaks at its explosion (plan 09, P09.T33.a).
pub(super) fn observe_row(
    galaxy: &Galaxy,
    interiors: &dyn FeatureInteriorCache,
    stars: &mut SystemStarsHandle<'_>,
    hit: &SystemHit,
    observer: &Observer,
) -> Result<(ObservedDto, Option<StellarBrief>), TraceMotionError> {
    let record = hit.record();
    // A catalogue system's line is refused below before any stars are built, whether its class
    // models them (a core collapse) or not (a Type Ia; plan 09, P09.T34.a).
    let has_stars = record.kind() != SystemKind::RoguePlanet
        && !matches!(
            record.origin(),
            SystemOrigin::CentreMember { .. } | SystemOrigin::CatalogueSystem(_)
        );
    if has_stars {
        let seen = stars.with_stars(galaxy, record, |system| {
            observe_hit_in(galaxy, interiors, hit, system, observer)
        })?;
        Ok((
            observed_dto(seen.retardation(), seen.error()),
            seen.brief_then(),
        ))
    } else {
        let line = Drift::of_record_in(galaxy, interiors, record)?;
        let retardation = retarded(observer, &line);
        let error = curvature_error(galaxy, observer.position(), &retardation);
        Ok((observed_dto(&retardation, error), None))
    }
}
