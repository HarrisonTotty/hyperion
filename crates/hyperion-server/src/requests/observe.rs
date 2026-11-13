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
use hyperion_sim::galaxy::placement::SystemKind;
use hyperion_sim::galaxy::query::{QueryMode, SystemHit};
use hyperion_sim::observe::{Observer, StarsCache, TraceMotionError};
use hyperion_sim::stellar::system::StellarBrief;
use hyperion_sim::units::LightYears;

use super::global_list::listing_refused;
use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{CancelToken, JobError, Priority, SystemStarsHandle};
use crate::convert::{
    MotionReads, ResolveRequest, SystemLine, observed_dto, resolved_system, system_motion,
    unknown_system, untraced_system,
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
/// the server does not trace below generator version 27, in either mode (a member of the galactic
/// centre, of the global list, or a catalogue system: [`SystemLine::of`]), or a tidal disruption's
/// victim after its disruption; `bad_request` naming `mode` for an
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
        // Lent so that a feature member's lines here and below do not build its interior again,
        // nor a centre member's its cell's orbits.
        let (interiors, orbits) = (
            shared.cluster_models.handle(key),
            shared.centre_orbits.handle(key),
        );
        let reads = MotionReads::new(&interiors, &orbits).with_listing(&listing);
        let time = wanted.time();
        // Every system a range query returns is placed as its row places it, bit for bit, but a
        // feature member, on its own line; below generator version 27 a centre member, a list
        // member and a catalogue system are refused at every time, as no query returns them.
        let (position, velocity) = system_motion(&galaxy, reads, &record, time)
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
                let (reading, _) = observe_row(&galaxy, reads, &mut stars, &hit, &observer)
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
/// The system is read on its path ([`SystemLine`]) through `reads`, which is row for row what the
/// sim's `range_query_observed` gives a grid system or a feature member (P12.T3's `query::mode`
/// test), and from generator version 27 a member of the galactic centre on its orbit, a stream's
/// or a dwarf core's member on its line and a catalogue class's host on its broken line (P10.T10.b's
/// registration member). Its stars come from the system cache `stars` lends: a grid system's and a
/// feature member's as the sim's `observe_hit_in` reads them, a list member's from its member
/// record and a host's from its class, where its class models them (a core collapse, not a Type
/// Ia). A rogue planet has no stars, and a member of the galactic centre none generated yet, so
/// each is read on its path alone, as `range_query_observed` reads a rogue planet: its retardation
/// and its stated error, and no brief.
///
/// # Errors
///
/// [`TraceMotionError`] for a system whose motion the server does not trace below generator
/// version 27 ([`SystemLine::of`]): a member of the galactic centre, of the global list, and a
/// catalogue system, each refused before any stars are built.
pub(super) fn observe_row(
    galaxy: &Galaxy,
    reads: MotionReads<'_>,
    stars: &mut SystemStarsHandle<'_>,
    hit: &SystemHit,
    observer: &Observer,
) -> Result<(ObservedDto, Option<StellarBrief>), TraceMotionError> {
    let record = hit.record();
    let line = SystemLine::of(galaxy, reads, record)?;
    let seen = match &line {
        _ if record.kind() == SystemKind::RoguePlanet => line.observe(galaxy, hit, None, observer),
        SystemLine::Grid(_) | SystemLine::Line(..) => stars.with_stars(galaxy, record, |system| {
            line.observe(galaxy, hit, Some(system), observer)
        }),
        SystemLine::Listed(member) => stars.with_stars_of(
            galaxy,
            record.id(),
            || Some(member.stars(galaxy)),
            |system| line.observe(galaxy, hit, system, observer),
        ),
        SystemLine::Catalogue(system) => stars.with_stars_of(
            galaxy,
            record.id(),
            || system.stars(galaxy),
            |built| line.observe(galaxy, hit, built, observer),
        ),
        SystemLine::Centre(_) => line.observe(galaxy, hit, None, observer),
    };
    Ok((
        observed_dto(seen.retardation(), seen.error()),
        seen.brief_then(),
    ))
}
