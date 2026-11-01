//! The scene's handlers (rendering plan R03): `scene_ship`, which sets a universe's ship stand-in
//! and scene clock (R03.T6).

use std::sync::Arc;

use hyperion_protocol::{
    ErrorCode, RequestError, ResponseBody, SceneClockDto, SceneShipRequest, SceneShipSet,
};
use hyperion_sim::coords::SystemPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::global_list::GlobalListSource;
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::planetary::{BodyIndex, ResolveBodyError};
use hyperion_sim::time::{ClockWindow, UniverseTime};
use tokio::time::Instant;

use super::global_list::listing_refused;
use super::system::bodies_of;
use super::universe::openable_universe;
use crate::AppState;
use crate::compute::{CancelToken, GalaxyKey, GenerateBodiesError, JobError, Listing, Priority};
use crate::convert::{
    MotionReads, ShipRequest, SystemLine, unknown_frame_body, unknown_frame_system,
    untraced_frame_system,
};
use crate::scene::{SceneClock, SceneSetting, ShipPosition, ShipStandIn};

/// Sets the universe's ship stand-in and scene clock, and answers the clock as set (Design
/// note 2).
///
/// The last `scene_ship` the server accepts stands, and every subscription of the universe sees
/// it. The clock is anchored at the moment the request is accepted, reading the pose's time.
///
/// # Errors
///
/// Those of [`openable_universe`]; the field checks of [`ShipRequest`] (`bad_request` naming
/// `time_rate`, `ship.time`, `ship.velocity_m_s` or `ship.position`); for a system frame,
/// `unknown_system` naming `ship.position` if plan 03's `resolve` refuses the ID (from generator
/// version 27 a list member's through plan 10's `resolve_with`) or the system cannot be placed at
/// every time of the clock window (below 27 a member of the galactic centre or the global list, or
/// a catalogue system, at every time, the epoch included; P07.T10.c and P09.T33.a; a tidal
/// disruption's victim disrupted inside the window), and `bad_request` if the offset takes the
/// ship out of the galactic frame's range; for a body frame, `unknown_system` likewise and
/// `unknown_body` if the system holds no such body or the body is not present at the pose's time;
/// those of the galaxy cache, and of the global list's service for a list member's frame; and
/// `queue_full`, `internal` or `cancelled` from the pool job that resolves the frame.
pub(crate) async fn ship(
    state: Arc<AppState>,
    request: SceneShipRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let checked = ShipRequest::try_from(&request)?;
    let key = universe.key();
    let galaxy = state.galaxies.get(key).await?;
    let frame = check_frame(&state, key, &galaxy, &checked, token).await?;
    let anchor = Instant::now();
    let clock = SceneClock::new(checked.time(), anchor, checked.rate())
        .expect("the request's time was checked to lie inside the clock window");
    let set = clock.at_instant(anchor);
    let ship = ShipStandIn::from(checked).with_frame(frame);
    state.scene.set(universe.id(), SceneSetting { clock, ship });
    tracing::debug!(time = %set.time(), rate = set.rate().get(), "scene ship set");
    Ok(ResponseBody::SceneShip(SceneShipSet {
        clock: SceneClockDto::from(set),
    }))
}

/// Resolves the frame the pose is in, as plan 04 requires of every ID read from a client, finds
/// its system's path by its origin ([`SystemLine::of`], as
/// [`system_motion`](crate::convert::system_motion) does), and checks that
/// the pose can be placed in the galactic frame at its time; answers the path, `None` for a pose
/// in the galactic frame.
///
/// The path is placed at the clock window's end too, since the scene's core places the frame at
/// every time its clock reads, and the only system whose place ends is a tidal disruption's victim,
/// at its disruption: one disrupted before the window's end is refused, at every time.
async fn check_frame(
    state: &Arc<AppState>,
    key: GalaxyKey,
    galaxy: &Arc<Galaxy>,
    checked: &ShipRequest,
    token: CancelToken,
) -> Result<Option<Arc<SystemLine>>, RequestError> {
    let t = checked.time();
    let (system, offset, body) = match checked.position() {
        ShipPosition::Galactic(_) => return Ok(None),
        ShipPosition::System { system, offset } => (system, offset, None),
        ShipPosition::Body { body, offset } => {
            let generated = bodies_of(state, key, galaxy, body.system()).await.map_err(
                |error| match error {
                    GenerateBodiesError::NoSuchSystem(error) => {
                        unknown_frame_system(&body_name(body), error)
                    }
                    GenerateBodiesError::Compute(error) => error.into(),
                },
            )?;
            let index = BodyIndex::try_from(body.body_index())
                .expect("the request's checks decoded the index");
            let (ctx, planets) = generated.as_ref();
            let centre = match planets.position_at(ctx, index, t) {
                Ok(Some(centre)) => centre,
                Ok(None) => {
                    return Err(unknown_frame_body(
                        body,
                        "it is not present at the ship's time",
                    ));
                }
                Err(ResolveBodyError::NoSuchBody) => {
                    return Err(unknown_frame_body(body, "its system holds no such body"));
                }
                Err(
                    error @ (ResolveBodyError::NoSuchSystem(_)
                    | ResolveBodyError::MalformedIndex(_)),
                ) => {
                    unreachable!(
                        "the system was generated and the index decoded before this: {error}"
                    )
                }
            };
            (body.system(), offset.to_system(&centre), Some(body))
        }
    };
    // A stream's or a dwarf core's member resolves through its galaxy's list, from generator
    // version 27, when the server places it (plan 10, P10.T10.b).
    let listing = if GlobalListSource::is_registered_in(galaxy) {
        state
            .global_lists
            .listing(key, system)
            .await
            .map_err(|error| listing_refused(system, error))?
    } else {
        Listing::Unlisted
    };
    let (galaxy, shared) = (Arc::clone(galaxy), Arc::clone(state));
    let check = move |_: &CancelToken| {
        let what = body.map_or_else(|| format!("system {:016x}", system.raw()), body_name);
        let (interiors, orbits) = (
            shared.cluster_models.handle(key),
            shared.centre_orbits.handle(key),
        );
        let reads = MotionReads::new(&interiors, &orbits).with_listing(&listing);
        frame_line(&galaxy, (&listing, reads), (system, &what), offset, t).map(Some)
    };
    let receiver = state.pool.try_submit(Priority::Interactive, token, check)?;
    receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?
}

/// The path of the system `system` a pose names as its frame, which the refusals call `what`,
/// resolved through `listing` and found through `reads` ([`SystemLine::of`]), placed at the pose's
/// time `t` and at the clock window's end, and the pose `offset` from it in the galactic frame's
/// range ([`check_frame`]).
///
/// # Errors
///
/// `unknown_system` naming `ship.position` if the ID resolves to no system or the system cannot be
/// placed at both times, and `bad_request` naming it if the offset takes the ship out of the
/// galactic frame's range.
pub(super) fn frame_line(
    galaxy: &Galaxy,
    (listing, reads): (&Listing, MotionReads<'_>),
    (system, what): (SystemId, &str),
    offset: SystemPosition,
    t: UniverseTime,
) -> Result<Arc<SystemLine>, RequestError> {
    let record = listing
        .resolve(galaxy, system)
        .map_err(|error| unknown_frame_system(what, error))?;
    // Placed by its origin, as every system named by a client: a feature member on its own line,
    // the rest as their range rows; below generator version 27 a member of the galactic centre or
    // the global list or a catalogue system is refused at every time, the epoch included, so that
    // the scene's core can place every frame this accepts at every later time.
    let untraced = |error| untraced_frame_system(what, error);
    let line = SystemLine::of(galaxy, reads, &record).map_err(untraced)?;
    let (barycentre, _) = line.at(galaxy, t).map_err(untraced)?;
    line.at(galaxy, ClockWindow::END).map_err(untraced)?;
    offset
        .to_galactic(&barycentre)
        .map(|_| Arc::new(line))
        .ok_or_else(|| RequestError {
            code: ErrorCode::BadRequest,
            message: "invalid ship.position: the offset takes the ship out of the galactic \
                      frame's range"
                .to_owned(),
            field: Some("ship.position".to_owned()),
        })
}

/// A body's ID as the refusals name it.
fn body_name(body: BodyId) -> String {
    format!(
        "body {:016x}:{:04x}",
        body.system().raw(),
        body.body_index()
    )
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{
        BodyIdHex, CreateUniverseRequest, FramePositionDto, KinematicsDto, SceneClockStateDto,
        SystemIdHex, UniverseIdHex,
    };
    use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    use hyperion_sim::id::{Layer, SystemId};
    use hyperion_sim::planetary::{self, SystemContext};

    use super::*;
    use crate::requests::Handlers;
    use crate::testing::Harness;
    use crate::universe::UniverseId;

    /// The seed of the galaxy these tests set ships in.
    const SEED: u64 = 0x4d2;

    async fn universe(harness: &Harness) -> UniverseIdHex {
        let created = super::super::universe::create(
            Arc::clone(harness.state()),
            CreateUniverseRequest {
                name: "Scene".to_owned(),
                seed: Some(hyperion_protocol::SeedHex::from_u64(SEED)),
            },
        )
        .await
        .unwrap();
        let ResponseBody::CreateUniverse(info) = created else {
            panic!("create_universe answers with the universe: {created:?}");
        };
        info.id
    }

    fn request(
        universe: &UniverseIdHex,
        position: FramePositionDto,
        rate: u32,
    ) -> SceneShipRequest {
        SceneShipRequest {
            universe: universe.clone(),
            ship: KinematicsDto {
                position,
                velocity_m_s: [0.0, 1_000.0, 0.0],
                time: hyperion_protocol::UniverseTime {
                    seconds: 3_600,
                    nanos: 5,
                },
            },
            time_rate: rate,
        }
    }

    fn galactic() -> FramePositionDto {
        FramePositionDto::Galactic {
            position: hyperion_protocol::GalacticPosition {
                cell_ly: [0, 26_000, 0],
                offset_m: [0.0; 3],
            },
        }
    }

    /// A system of the solar circle with at least one planetary body, and that body's index.
    async fn system_with_a_body(harness: &Harness) -> (SystemId, u16) {
        let key = crate::compute::GalaxyKey::new(SEED, hyperion_sim::GENERATOR_VERSION);
        let galaxy = harness.state().galaxies.get(key).await.unwrap();
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::new(Layer::C, [0, 812, 0]).unwrap(),
            &mut cell,
        );
        cell.iter()
            .find_map(|record| {
                let ctx = SystemContext::for_system(&galaxy, record.id()).ok()?;
                let system = planetary::generate(galaxy.seed(), &ctx);
                let body = system.bodies().first()?;
                Some((record.id(), u16::from(body.index())))
            })
            .expect("a cell of the solar circle holds a system with planets")
    }

    async fn set(
        harness: &Harness,
        request: SceneShipRequest,
    ) -> Result<ResponseBody, RequestError> {
        ship(Arc::clone(harness.state()), request, CancelToken::new()).await
    }

    fn refused(result: Result<ResponseBody, RequestError>) -> (ErrorCode, Option<String>) {
        let error = result.unwrap_err();
        (error.code, error.field)
    }

    #[tokio::test]
    async fn scene_ship_answers_the_clock_it_set_and_a_second_supersedes_the_first() {
        let harness = Harness::start(Handlers).await;
        let id = universe(&harness).await;
        let mut first = harness.state().scene.watch(UniverseId::from(&id));
        let mut second = harness.state().scene.watch(UniverseId::from(&id));

        let answer = set(&harness, request(&id, galactic(), 1_000))
            .await
            .unwrap();
        assert_eq!(
            answer,
            ResponseBody::SceneShip(SceneShipSet {
                clock: SceneClockDto {
                    time: hyperion_protocol::UniverseTime {
                        seconds: 3_600,
                        nanos: 5
                    },
                    time_rate: 1_000,
                    state: SceneClockStateDto::Running,
                },
            })
        );
        for receiver in [&mut first, &mut second] {
            assert!(receiver.has_changed().unwrap());
            let setting = receiver.borrow_and_update().clone();
            assert_eq!(setting.clock.at_instant(Instant::now()).rate().get(), 1_000);
        }

        let mut paused = request(&id, galactic(), 0);
        paused.ship.velocity_m_s = [5.0, 0.0, 0.0];
        set(&harness, paused.clone()).await.unwrap();
        for receiver in [&mut first, &mut second] {
            assert!(receiver.has_changed().unwrap());
            let setting = receiver.borrow_and_update().clone();
            let reading = setting.clock.now();
            assert_eq!(
                (reading.rate().get(), reading.time().seconds()),
                (0, 3_600),
                "the later setting stands"
            );
            assert_eq!(crate::scene::Ship::kinematics(&setting.ship), paused.ship);
        }
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_bad_rate_time_velocity_or_frame_is_refused_naming_its_field() {
        let harness = Harness::start(Handlers).await;
        let id = universe(&harness).await;
        let bad = |field: &str| (ErrorCode::BadRequest, Some(field.to_owned()));
        assert_eq!(
            refused(set(&harness, request(&id, galactic(), 50)).await),
            bad("time_rate")
        );
        let mut late = request(&id, galactic(), 1);
        late.ship.time.seconds = 40_000_000_000;
        assert_eq!(refused(set(&harness, late).await), bad("ship.time"));
        let mut fast = request(&id, galactic(), 1);
        fast.ship.velocity_m_s = [3.0e8, 0.0, 0.0];
        assert_eq!(refused(set(&harness, fast).await), bad("ship.velocity_m_s"));

        let (system, body) = system_with_a_body(&harness).await;
        let in_system = FramePositionDto::System {
            system: SystemIdHex::from_u64(system.raw()),
            offset_m: [1.0e11, 0.0, 0.0],
        };
        assert!(set(&harness, request(&id, in_system, 1)).await.is_ok());
        let in_body = FramePositionDto::Body {
            body: BodyIdHex::from_parts(system.raw(), body),
            offset_m: [1.0e7, 0.0, 0.0],
        };
        assert!(set(&harness, request(&id, in_body, 1)).await.is_ok());
        // Finite offsets that no galactic position holds, from a system and from a body.
        let too_far = [
            FramePositionDto::System {
                system: SystemIdHex::from_u64(system.raw()),
                offset_m: [1.0e30, 0.0, 0.0],
            },
            FramePositionDto::Body {
                body: BodyIdHex::from_parts(system.raw(), body),
                offset_m: [0.0, -1.0e30, 0.0],
            },
        ];
        for position in too_far {
            assert_eq!(
                refused(set(&harness, request(&id, position, 1)).await),
                bad("ship.position")
            );
        }

        // A grid ID whose candidate index is past any cell's count resolves to no system.
        let key = CellKey::new(Layer::C, [0, 812, 0]).unwrap();
        let missing = key
            .candidate_id(60_000)
            .expect("an index the layer's IDs can hold");
        let nowhere = FramePositionDto::System {
            system: SystemIdHex::from_u64(missing.raw()),
            offset_m: [0.0; 3],
        };
        assert_eq!(
            refused(set(&harness, request(&id, nowhere, 1)).await),
            (ErrorCode::UnknownSystem, Some("ship.position".to_owned()))
        );
        // Slot 0xCF's first planet: inside plan 14's layout, and held by no system this small.
        let no_body = FramePositionDto::Body {
            body: BodyIdHex::from_parts(system.raw(), 0xCF00),
            offset_m: [0.0; 3],
        };
        assert_eq!(
            refused(set(&harness, request(&id, no_body, 1)).await),
            (ErrorCode::UnknownBody, Some("ship.position".to_owned()))
        );
        harness.stop().await;
    }
}
