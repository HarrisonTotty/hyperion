//! The scene's core (rendering plan R03, R03.T7): pure and synchronous, it builds a scene's whole
//! state and works out what changed as the scene clock runs.
//!
//! The core reads the clock, the ship, the knowledge and the craft through its [`SceneInputs`],
//! and the galaxy through a [`SceneWorld`], which lends the caches and the planetary systems the
//! caller has fetched; it does no I/O. A system the world does not hold is asked for with
//! [`FetchSystemError`], and the core is left as it was, so that the caller can fetch it from the
//! body cache and call again.
//!
//! **The system** (Design note 3) is the ship's frame by plan 03's `frame_at`, the previous scene
//! system as `current`, with a Schmitt band at the boundary: the scene enters a system `frame_at`
//! names only once the ship's ratio to that system's sphere ([`candidate_at`]) is at most
//! [`SCENE_FRAME_ENTRY`], and leaves it only when `frame_at` no longer names it. In a system it
//! holds plan 14's bodies at the level asked, each degraded to its own grant (Design note 13), a
//! body granted `contact` placed by the server's own retarded evaluation from the ship.
//!
//! **Advancing** re-sends a body whose `valid_until` the scene time has passed, evaluated at that
//! time, one whose grant changed, and at each heartbeat every body placed by a seen position
//! (Design note 4). The cameras are checked against the scene's reach (Design note 6).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use hyperion_protocol::{
    CameraReportDto, ErrorCode, FramePositionDto, RequestError, SceneArrivalDto, SceneBodyDto,
    SceneClockDto, SceneCraftDto, SceneNotificationDto, SceneStateDto, SceneSystemDto,
    SeenPositionDto, SystemIdHex, SystemSummaryDto, SystemSummaryRequest, UniverseIdHex,
};
use hyperion_sim::Seed;
use hyperion_sim::coords::{
    BodyPosition, GalacticPosition, LyCell, SystemPosition, SystemVelocity,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::frame::{candidate_at, frame_at};
use hyperion_sim::galaxy::placement::{SystemRecord, resolve};
use hyperion_sim::galaxy::query::{epoch_velocity, position_at};
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::observe::{BodyTrack, SystemObserver, retarded_in_system};
use hyperion_sim::planetary::BodyIndex;
use hyperion_sim::planetary::record::{BodyRecord, DetailLevel};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Metres;

use super::sensing::{CraftState, SceneKnowledge};
use super::{ClockReading, Ship, ShipPosition};
use crate::compute::{GalaxyKey, GeneratedSystem, SharedCellCache, SharedSystemCache};
use crate::convert::{BodiesRequest, ListedBody, scene_body, scene_system, system_summary};
use crate::limits::MAX_SCENE_CAMERAS;

/// The ratio of the ship's distance to a system's tidal radius at or below which the scene enters
/// the system `frame_at` names: 0.9, R02's `BODY_FRAME_ENTRY` (Design note 3). It leaves only
/// once `frame_at` no longer names the system, at a ratio above 1, so a stand-in drifting across
/// the boundary arrives once and leaves once.
pub(crate) const SCENE_FRAME_ENTRY: f64 = 0.9;

/// What the core reads besides the galaxy: the clock, the ship, the knowledge and the craft.
#[derive(Clone, Copy)]
pub(crate) struct SceneInputs<'a> {
    /// The scene clock's reading: the scene time.
    pub(crate) clock: ClockReading,
    /// The ship.
    pub(crate) ship: &'a dyn Ship,
    /// What the ship knows.
    pub(crate) knowledge: &'a dyn SceneKnowledge,
}

impl fmt::Debug for SceneInputs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SceneInputs")
            .field("clock", &self.clock)
            .field("knowledge", &self.knowledge)
            .finish_non_exhaustive()
    }
}

/// The galaxy as the core reads it: the caches and the planetary systems the caller holds.
pub(crate) struct SceneWorld<'a> {
    /// The universe, as its systems' summaries name it.
    pub(crate) universe: &'a UniverseIdHex,
    /// The galaxy.
    pub(crate) galaxy: &'a Galaxy,
    /// The galaxy's key in the caches.
    pub(crate) key: GalaxyKey,
    /// The cells `frame_at` walks.
    pub(crate) cells: &'a SharedCellCache,
    /// The systems' stars, for the hosts.
    pub(crate) stars: &'a SharedSystemCache,
    /// The planetary systems the caller has fetched from the body cache.
    pub(crate) systems: &'a BTreeMap<SystemId, Arc<GeneratedSystem>>,
}

impl fmt::Debug for SceneWorld<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SceneWorld")
            .field("universe", &self.universe)
            .field("key", &self.key)
            .field("systems", &self.systems.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl SceneWorld<'_> {
    /// The seed the planetary systems were generated in.
    #[must_use]
    fn seed(&self) -> Seed {
        Seed::new(self.key.seed())
    }

    /// The planetary system `id`, if the caller has fetched it.
    fn system(&self, id: SystemId) -> Result<&Arc<GeneratedSystem>, FetchSystemError> {
        self.systems.get(&id).ok_or(FetchSystemError(id))
    }

    /// The system plan 03's `frame_at` puts a ship at `ship` in at `t`, having been in `current`.
    #[must_use]
    fn frame_at(
        &self,
        ship: &GalacticPosition,
        t: UniverseTime,
        current: Option<SystemId>,
    ) -> Option<SystemId> {
        frame_at(
            self.galaxy,
            &mut self.cells.handle(self.key),
            &[],
            ship,
            t,
            current,
        )
        .expect("the scene time is inside the clock window")
    }

    /// The system's stars at `t`, as `system_summary` answers them: the hosts of its bodies.
    #[must_use]
    fn hosts(&self, record: &SystemRecord, t: UniverseTime) -> SystemSummaryDto {
        let stars = self
            .stars
            .get_or_generate(self.key, self.galaxy, record.id())
            .expect("a system frame_at named resolves");
        let request = SystemSummaryRequest {
            universe: self.universe.clone(),
            system: SystemIdHex::from_u64(record.id().raw()),
            time: wire_time(t),
        };
        system_summary(request, &stars, t)
    }
}

/// The core needs a planetary system the [`SceneWorld`] does not hold: the caller fetches it from
/// the body cache and calls again. Nothing in the core changed.
///
/// Named for what the caller must do: fetch the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FetchSystemError(pub(crate) SystemId);

/// Whether an advance is a heartbeat, at which every body placed by a seen position is re-sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Beat {
    /// A heartbeat: seen bodies are refreshed.
    Heartbeat,
    /// Any other advance.
    Change,
}

/// What changed in a scene: an arrival or a departure, and the bodies re-sent.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SceneDelta {
    /// An arrival in a system, or the departure from one.
    pub(crate) arrival: Option<SceneArrivalDto>,
    /// Bodies re-sent, in index order.
    pub(crate) bodies: Vec<SceneBodyDto>,
}

/// One body the scene has sent, as it was sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sent {
    level: DetailLevel,
    valid_until: Option<UniverseTime>,
    /// Whether it was placed by a seen position.
    seen: bool,
}

impl Sent {
    /// A body listed as `listed`, placed by a seen position if `seen`.
    #[must_use]
    fn of(listed: ListedBody, seen: bool) -> Self {
        Self {
            level: listed.level,
            valid_until: listed.valid_until,
            seen,
        }
    }
}

/// The system the scene is in.
#[derive(Debug, Clone)]
struct InSystem {
    record: SystemRecord,
    generated: Arc<GeneratedSystem>,
    /// Every body of the system, resolved or not, in index order.
    all: Vec<BodyIndex>,
    /// The bodies sent, by index.
    sent: BTreeMap<BodyIndex, Sent>,
}

/// The system a scene is to be in, worked out before anything changes.
struct Target<'w> {
    record: SystemRecord,
    generated: &'w Arc<GeneratedSystem>,
    tidal_radius: Metres,
}

/// One scene's core: the system it is in, what it has sent of it, and the cameras.
#[derive(Debug, Clone)]
pub(crate) struct SceneCore {
    asked: DetailLevel,
    system: Option<InSystem>,
    cameras: BTreeMap<u8, CameraReportDto>,
    /// Whether the last craft list sent held any craft.
    craft_sent: bool,
}

impl SceneCore {
    /// A scene at `asked` and its whole state, the craft of `craft` that are contacts in it.
    ///
    /// # Errors
    ///
    /// [`FetchSystemError`] if the scene's system, or the ship's frame's, is not in `world`.
    pub(crate) fn build(
        asked: DetailLevel,
        inputs: SceneInputs<'_>,
        craft: Vec<CraftState>,
        world: &SceneWorld<'_>,
    ) -> Result<(Self, SceneStateDto), FetchSystemError> {
        let t = inputs.clock.time();
        let ship = ship_galactic(world, inputs.ship.position_at(t), t)?;
        let target = select(world, None, &ship, t)?;
        let mut core = Self {
            asked,
            system: None,
            cameras: BTreeMap::new(),
            craft_sent: false,
        };
        let (system, tidal_radius_m) = match target {
            Some(target) => {
                let (system, tidal_radius) = core.arrive(&target, inputs, world)?;
                (Some(system), Some(tidal_radius.value()))
            }
            None => (None, None),
        };
        let craft = core.craft(inputs.knowledge, craft).unwrap_or_default();
        let state = SceneStateDto {
            sequence: 0,
            clock: SceneClockDto::from(inputs.clock),
            ship: inputs.ship.kinematics(),
            system,
            tidal_radius_m,
            craft,
        };
        Ok((core, state))
    }

    /// What changed since the last build or advance, at the inputs' time: an arrival or a
    /// departure, and the bodies to re-send.
    ///
    /// # Errors
    ///
    /// [`FetchSystemError`] if a system the scene needs is not in `world`; the core is unchanged.
    pub(crate) fn advance(
        &mut self,
        inputs: SceneInputs<'_>,
        world: &SceneWorld<'_>,
        beat: Beat,
    ) -> Result<SceneDelta, FetchSystemError> {
        let t = inputs.clock.time();
        let ship = ship_galactic(world, inputs.ship.position_at(t), t)?;
        let current = self.system.as_ref().map(|system| system.record.id());
        let target = select(world, current, &ship, t)?;
        match (target, current) {
            (Some(target), Some(id)) if target.record.id() == id => {
                match self.refresh(inputs, world, beat)? {
                    Some(delta) => Ok(delta),
                    // A body the client holds no longer resolves at its grant, and the wire has
                    // no withdrawal: the whole system arrives again, at the current grants (a
                    // delegated decision of 2026-09-30). `refresh` has changed `sent`, but
                    // `arrive` cannot fail after it: `ship_galactic` above has already fetched
                    // every system the observer reads, and `arrive` replaces `sent` whole.
                    None => self.arrival(&target, inputs, world),
                }
            }
            (Some(target), _) => self.arrival(&target, inputs, world),
            (None, Some(_)) => {
                self.system = None;
                Ok(SceneDelta {
                    arrival: Some(SceneArrivalDto::NoSystem),
                    bodies: Vec::new(),
                })
            }
            (None, None) => Ok(SceneDelta::default()),
        }
    }

    /// The system the scene is in, if it is in one.
    #[must_use]
    pub(crate) fn system_id(&self) -> Option<SystemId> {
        self.system.as_ref().map(|system| system.record.id())
    }

    /// The next scene time at which [`SceneCore::advance`] must be called for a body's
    /// `valid_until`: the earliest among the bodies sent, if any.
    #[must_use]
    pub(crate) fn next_due(&self) -> Option<UniverseTime> {
        self.system
            .as_ref()?
            .sent
            .values()
            .filter_map(|sent| sent.valid_until)
            .min()
    }

    /// The craft list to push, if it is to be pushed: the contacts among `craft` while there are
    /// any, and an empty list once, when the last leaves.
    #[must_use]
    pub(crate) fn craft(
        &mut self,
        knowledge: &dyn SceneKnowledge,
        craft: Vec<CraftState>,
    ) -> Option<Vec<SceneCraftDto>> {
        let contacts: Vec<SceneCraftDto> = craft
            .into_iter()
            .filter(|craft| knowledge.is_contact(craft))
            .map(SceneCraftDto::from)
            .collect();
        let was_sent = std::mem::replace(&mut self.craft_sent, !contacts.is_empty());
        (was_sent || !contacts.is_empty()).then_some(contacts)
    }

    /// Whether the scene holds any craft: the subscription pushes them at every tick while it does.
    #[must_use]
    pub(crate) fn has_craft(&self) -> bool {
        self.craft_sent
    }

    /// Replaces the cameras with `cameras`, the latest per view, if every one is within the
    /// scene's reach at `t` (Design note 6).
    ///
    /// # Errors
    ///
    /// `bad_request` naming `cameras` for more than [`MAX_SCENE_CAMERAS`], a component that is not
    /// finite, a position that is not a canonical galactic one or not inside the root cube, and a
    /// camera out of reach: in a system, outside the system's sphere of influence or in a frame of
    /// another system or of a body not in the scene; in the galactic frame, in any system's frame.
    /// The cameras are left as they were.
    pub(crate) fn set_cameras(
        &mut self,
        cameras: Vec<CameraReportDto>,
        t: UniverseTime,
        world: &SceneWorld<'_>,
    ) -> Result<(), RequestError> {
        if cameras.len() > MAX_SCENE_CAMERAS {
            return Err(camera_refusal(format!(
                "a scene subscription reports at most {MAX_SCENE_CAMERAS} cameras"
            )));
        }
        let reach = self
            .system
            .as_ref()
            .map(|system| Reach::of(system, world, t));
        for camera in &cameras {
            check_camera(camera, reach.as_ref(), t)?;
        }
        self.cameras = cameras
            .into_iter()
            .map(|camera| (camera.view, camera))
            .collect();
        Ok(())
    }

    /// The cameras, the latest per view.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the cameras bound nothing yet (Design note 6)")
    )]
    pub(crate) fn cameras(&self) -> impl Iterator<Item = &CameraReportDto> {
        self.cameras.values()
    }

    /// Plants a `valid_until` at `at` on the first body sent that is not a contact, and returns
    /// its index: no body of the pinned systems changes inside the clock window, so the tests of
    /// a re-send plant one on the core's record of what it sent.
    #[cfg(test)]
    pub(crate) fn plant_valid_until(&mut self, at: UniverseTime) -> Option<BodyIndex> {
        let system = self.system.as_mut()?;
        let (index, sent) = system
            .sent
            .iter_mut()
            .find(|(_, sent)| sent.level != DetailLevel::Contact)?;
        sent.valid_until = Some(at);
        Some(*index)
    }

    /// The arrival in `target`: the whole system at the scene time, entered.
    fn arrival(
        &mut self,
        target: &Target<'_>,
        inputs: SceneInputs<'_>,
        world: &SceneWorld<'_>,
    ) -> Result<SceneDelta, FetchSystemError> {
        let (system, tidal_radius) = self.arrive(target, inputs, world)?;
        Ok(SceneDelta {
            arrival: Some(SceneArrivalDto::System {
                system: Box::new(system),
                tidal_radius_m: tidal_radius.value(),
            }),
            bodies: Vec::new(),
        })
    }

    /// Enters `target`: the whole system at the scene time, its bodies recorded as sent.
    fn arrive(
        &mut self,
        target: &Target<'_>,
        inputs: SceneInputs<'_>,
        world: &SceneWorld<'_>,
    ) -> Result<(SceneSystemDto, Metres), FetchSystemError> {
        let t = inputs.clock.time();
        let &Target {
            record,
            generated,
            tidal_radius,
        } = target;
        let observer = observer(world, &record, generated, inputs.ship, t)?;
        let (ctx, planets) = generated.as_ref();
        let id = record.id();
        let (system, listed) = scene_system(
            BodiesRequest::new(id, t, self.asked),
            world.hosts(&record, t),
            ctx,
            planets,
            world.seed(),
            |index| inputs.knowledge.grant(index.body_id(id), self.asked),
            |index| seen(generated, observer.as_ref(), index),
        );
        let seen_bodies: BTreeSet<BodyIndex> = system
            .grants
            .iter()
            .filter(|grant| grant.seen.is_some())
            .filter_map(|grant| BodyIndex::try_from(grant.body.to_parts().1).ok())
            .collect();
        self.system = Some(InSystem {
            record,
            generated: Arc::clone(generated),
            all: planets
                .snapshot_at(ctx, t)
                .bodies()
                .iter()
                .map(BodyRecord::index)
                .collect(),
            sent: listed
                .into_iter()
                .map(|listed| {
                    (
                        listed.index,
                        Sent::of(listed, seen_bodies.contains(&listed.index)),
                    )
                })
                .collect(),
        });
        Ok((system, tidal_radius))
    }

    /// The bodies to re-send in the system the scene stays in: those whose `valid_until` has
    /// passed, evaluated when it passed; those whose grant changed; and at a heartbeat every
    /// contact the ship sees, or saw at the last push, with its seen position. `None` when a body
    /// sent before no longer resolves at its grant, which only a whole new arrival can withdraw.
    ///
    /// # Errors
    ///
    /// [`FetchSystemError`] if the ship's frame's system is not in `world`, before anything
    /// changes.
    fn refresh(
        &mut self,
        inputs: SceneInputs<'_>,
        world: &SceneWorld<'_>,
        beat: Beat,
    ) -> Result<Option<SceneDelta>, FetchSystemError> {
        let t = inputs.clock.time();
        let asked = self.asked;
        let Some(system) = self.system.as_ref() else {
            return Ok(Some(SceneDelta::default()));
        };
        let generated = Arc::clone(&system.generated);
        let id = system.record.id();
        let levels: Vec<(BodyIndex, DetailLevel)> = system
            .all
            .iter()
            .map(|&index| {
                let level = inputs.knowledge.grant(index.body_id(id), asked).min(asked);
                (index, level)
            })
            .collect();
        // The observer is needed only to place contacts, and is had before anything changes.
        let observer = if levels
            .iter()
            .any(|&(_, level)| level == DetailLevel::Contact)
        {
            observer(world, &system.record, &generated, inputs.ship, t)?
        } else {
            None
        };
        let system = self
            .system
            .as_mut()
            .expect("the scene's system was read just above");
        let (ctx, planets) = generated.as_ref();
        let mut bodies = Vec::new();
        let mut withdrawn = false;
        for (index, level) in levels {
            let sent = system.sent.get(&index).copied();
            let regranted = sent.is_none_or(|sent| sent.level != level);
            let lapsed = sent
                .and_then(|sent| sent.valid_until)
                .is_some_and(|until| until <= t);
            let contact = level == DetailLevel::Contact;
            let sighting = if contact && (regranted || lapsed || beat == Beat::Heartbeat) {
                seen(&generated, observer.as_ref(), index)
            } else {
                None
            };
            // A heartbeat moves the contacts the ship sees, and reports one it no longer does.
            let refreshed = beat == Beat::Heartbeat
                && contact
                && (sighting.is_some() || sent.is_some_and(|sent| sent.seen));
            if !(regranted || lapsed || refreshed) {
                continue;
            }
            // A body re-sent for its elements alone is evaluated when they changed, each change in
            // turn, so that advancing in one step or in many sends the same record. A contact is
            // always placed at the scene time, where the ship sees it now.
            let mut when = t;
            if lapsed && !regranted && !contact {
                let mut until = sent.and_then(|sent| sent.valid_until);
                while let Some(due) = until.filter(|&due| due <= t) {
                    when = due;
                    let next = scene_body(ctx, planets, index, due, level, None)
                        .and_then(|(_, listed)| listed.valid_until);
                    if next.is_none_or(|next| next <= due) {
                        break;
                    }
                    until = next;
                }
            }
            match scene_body(ctx, planets, index, when, level, sighting) {
                Some((body, listed)) => {
                    system
                        .sent
                        .insert(index, Sent::of(listed, body.seen.is_some()));
                    bodies.push(body);
                }
                // A level that no longer resolves a body the client holds withdraws it, which
                // the wire can say only by sending the whole system again; one never sent is
                // simply not sent.
                None => {
                    withdrawn |= system.sent.remove(&index).is_some();
                }
            }
        }
        Ok((!withdrawn).then_some(SceneDelta {
            arrival: None,
            bodies,
        }))
    }
}

/// Whether a scene notification is serialised on the CPU pool rather than the runtime (Design
/// note 14): one carrying an arrival holds a whole system; the rest are small.
#[must_use]
pub(crate) fn is_large_notification(notification: &SceneNotificationDto) -> bool {
    notification.arrival.is_some()
}

/// The system the scene is to be in: the one `frame_at` names from `current`, entered only at a
/// ratio of at most [`SCENE_FRAME_ENTRY`] unless it is `current` already.
fn select<'w>(
    world: &'w SceneWorld<'_>,
    current: Option<SystemId>,
    ship: &GalacticPosition,
    t: UniverseTime,
) -> Result<Option<Target<'w>>, FetchSystemError> {
    let Some(id) = world.frame_at(ship, t, current) else {
        return Ok(None);
    };
    let record = resolve(world.galaxy, id).expect("a system frame_at names resolves");
    let Some(candidate) = candidate_at(world.galaxy, &record, ship, t) else {
        return Ok(None);
    };
    if Some(id) != current && candidate.ratio() > SCENE_FRAME_ENTRY {
        return Ok(None);
    }
    Ok(Some(Target {
        record,
        generated: world.system(id)?,
        tidal_radius: candidate.tidal_radius(),
    }))
}

/// Where the ship is at `t` in the galactic frame.
fn ship_galactic(
    world: &SceneWorld<'_>,
    position: ShipPosition,
    t: UniverseTime,
) -> Result<GalacticPosition, FetchSystemError> {
    let (system, offset) = match position {
        ShipPosition::Galactic(position) => return Ok(position),
        ShipPosition::System { system, offset } => (system, offset),
        ShipPosition::Body { body, offset } => {
            let (centre, _) = body_state(world.system(body.system())?, body, t);
            (body.system(), offset.to_system(&centre))
        }
    };
    let record = resolve(world.galaxy, system).expect("scene_ship resolved the ship's frame");
    Ok(offset
        .to_galactic(&position_at(world.galaxy, &record, t))
        .expect("scene_ship placed the ship in the root cube, which drift cannot leave the range"))
}

/// A body's centre and velocity at `t` in its system's frame; the barycentre at rest for a body
/// not present then, so that a ship whose frame's body has gone stays where its offset puts it.
#[must_use]
fn body_state(
    generated: &GeneratedSystem,
    body: BodyId,
    t: UniverseTime,
) -> (SystemPosition, SystemVelocity) {
    let (ctx, planets) = generated;
    BodyIndex::try_from(body.body_index())
        .ok()
        .and_then(|index| planets.state_at(ctx, index, t).ok().flatten())
        .unwrap_or((SystemPosition::ORIGIN, SystemVelocity::ZERO))
}

/// The ship as an observer in the system `record` at `t`; `None` if it cannot observe, as at c.
fn observer(
    world: &SceneWorld<'_>,
    record: &SystemRecord,
    generated: &GeneratedSystem,
    ship: &dyn Ship,
    t: UniverseTime,
) -> Result<Option<SystemObserver>, FetchSystemError> {
    let velocity = SystemVelocity::new(ship.velocity_m_s());
    let (position, velocity) = match ship.position_at(t) {
        ShipPosition::System { system, offset } if system == record.id() => (offset, velocity),
        ShipPosition::Body { body, offset } if body.system() == record.id() => {
            let (centre, moving) = body_state(generated, body, t);
            (offset.to_system(&centre), velocity + moving)
        }
        elsewhere => {
            // Through the galactic frame: the frame's own drift, and a body's motion in it, are
            // added, and this system's drift taken away.
            let frame_velocity = match elsewhere {
                ShipPosition::Galactic(_) => SystemVelocity::ZERO,
                ShipPosition::System { system, .. } => drift(world.galaxy, system),
                ShipPosition::Body { body, .. } => {
                    let (_, moving) = body_state(world.system(body.system())?, body, t);
                    drift(world.galaxy, body.system()) + moving
                }
            };
            let galactic = ship_galactic(world, elsewhere, t)?;
            let barycentre = position_at(world.galaxy, record, t);
            (
                SystemPosition::from_galactic(&galactic, &barycentre),
                velocity + frame_velocity - drift(world.galaxy, record.id()),
            )
        }
    };
    Ok(SystemObserver::new(position, velocity, t).ok())
}

/// A system's drift through the galaxy, m/s along the galactic axes (plan 08's epoch velocity).
#[must_use]
fn drift(galaxy: &Galaxy, system: SystemId) -> SystemVelocity {
    let record = resolve(galaxy, system).expect("the scene's systems resolve");
    SystemVelocity::new(epoch_velocity(galaxy, &record).metres_per_second())
}

/// Where the ship sees body `index`: its apparent position, light time and aberration together,
/// from `retarded_in_system`; `None` if there is no observer or the body is not present then.
#[must_use]
fn seen(
    generated: &GeneratedSystem,
    observer: Option<&SystemObserver>,
    index: BodyIndex,
) -> Option<SeenPositionDto> {
    let (ctx, planets) = generated;
    let track = BodyTrack::new(planets, ctx, index).ok()?;
    let seen = retarded_in_system(observer?, &track).ok()?;
    Some(SeenPositionDto {
        apparent_m: seen.apparent().metres(),
        emitted: wire_time(seen.emitted()),
    })
}

/// A time as the wire carries it.
#[must_use]
fn wire_time(t: UniverseTime) -> hyperion_protocol::UniverseTime {
    hyperion_protocol::UniverseTime {
        seconds: t.seconds(),
        nanos: t.subsec_nanos(),
    }
}

/// What a camera in a system may reach at a time.
struct Reach<'a> {
    in_system: &'a InSystem,
    barycentre: GalacticPosition,
    tidal_radius: Metres,
}

impl<'a> Reach<'a> {
    #[must_use]
    fn of(in_system: &'a InSystem, world: &SceneWorld<'_>, t: UniverseTime) -> Self {
        let barycentre = position_at(world.galaxy, &in_system.record, t);
        let tidal_radius = candidate_at(world.galaxy, &in_system.record, &barycentre, t)
            .map_or(Metres::new(0.0), |candidate| candidate.tidal_radius());
        Self {
            in_system,
            barycentre,
            tidal_radius,
        }
    }

    #[must_use]
    fn holds(&self, position: SystemPosition) -> bool {
        position.distance_from_origin().value() <= self.tidal_radius.value()
    }
}

/// Refuses a camera the scene cannot reach (Design note 6).
fn check_camera(
    camera: &CameraReportDto,
    reach: Option<&Reach<'_>>,
    t: UniverseTime,
) -> Result<(), RequestError> {
    let pose = &camera.pose;
    if !pose.velocity_m_s.iter().all(|v| v.is_finite()) {
        return Err(camera_refusal(format!(
            "view {}'s velocity is not finite",
            camera.view
        )));
    }
    let out_of_reach = || {
        camera_refusal(format!(
            "view {}'s camera is outside the scene's reach",
            camera.view
        ))
    };
    match (&pose.position, reach) {
        (FramePositionDto::Galactic { position }, reach) => {
            let galactic = GalacticPosition::new(LyCell::new(position.cell_ly), position.offset_m)
                .ok()
                .filter(GalacticPosition::in_root_cube)
                .ok_or_else(|| {
                    camera_refusal(format!(
                        "view {}'s position is not a galactic position in the root cube",
                        camera.view
                    ))
                })?;
            match reach {
                Some(reach) => reach
                    .holds(SystemPosition::from_galactic(&galactic, &reach.barycentre))
                    .then_some(())
                    .ok_or_else(out_of_reach),
                None => Ok(()),
            }
        }
        (FramePositionDto::System { system, offset_m }, Some(reach)) => {
            finite(camera, *offset_m)?;
            (system.to_u64() == reach.in_system.record.id().raw()
                && reach.holds(SystemPosition::new(*offset_m)))
            .then_some(())
            .ok_or_else(out_of_reach)
        }
        (FramePositionDto::Body { body, offset_m }, Some(reach)) => {
            finite(camera, *offset_m)?;
            let (system, index) = body.to_parts();
            let in_scene = system == reach.in_system.record.id().raw()
                && BodyIndex::try_from(index)
                    .is_ok_and(|index| reach.in_system.sent.contains_key(&index));
            if !in_scene {
                return Err(out_of_reach());
            }
            let id = reach.in_system.record.id();
            let (centre, _) = body_state(&reach.in_system.generated, BodyId::new(id, index), t);
            reach
                .holds(BodyPosition::new(*offset_m).to_system(&centre))
                .then_some(())
                .ok_or_else(out_of_reach)
        }
        (FramePositionDto::System { .. } | FramePositionDto::Body { .. }, None) => {
            Err(out_of_reach())
        }
    }
}

/// Refuses an offset with a component that is not finite.
fn finite(camera: &CameraReportDto, offset_m: [f64; 3]) -> Result<(), RequestError> {
    if offset_m.iter().all(|m| m.is_finite()) {
        Ok(())
    } else {
        Err(camera_refusal(format!(
            "view {}'s offset is not finite",
            camera.view
        )))
    }
}

/// A camera report the scene refuses: `bad_request` naming `cameras`.
#[must_use]
fn camera_refusal(message: String) -> RequestError {
    RequestError {
        code: ErrorCode::BadRequest,
        message,
        field: Some("cameras".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use hyperion_protocol::{BodyIdHex, DetailLevelDto, KinematicsDto, SystemBodiesDto};
    use hyperion_sim::GENERATOR_VERSION;
    use hyperion_sim::coords::GalacticDisplacement;
    use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    use hyperion_sim::id::Layer;
    use hyperion_sim::planetary::{SystemContext, generate};
    use hyperion_sim::time::Span;
    use tokio::time::Instant;

    use super::*;
    use crate::convert::system_bodies;
    use crate::scene::sensing::GrantAsked;
    use crate::scene::{SceneClock, ShipStandIn, TimeRate};
    use crate::subscriptions::{Merge, ScenePush};

    /// The seed of the galaxy these scenes are in.
    const SEED: u64 = 0x4d2;

    /// A galaxy, its caches, and the planetary systems of one cell of the solar circle.
    struct Fixture {
        galaxy: Galaxy,
        universe: UniverseIdHex,
        cells: SharedCellCache,
        stars: SharedSystemCache,
        systems: BTreeMap<SystemId, Arc<GeneratedSystem>>,
        /// The first system of the cell with at least four bodies: the scene's.
        record: SystemRecord,
        /// Another system of the cell.
        other: SystemRecord,
    }

    impl Fixture {
        fn new() -> Self {
            let galaxy = Galaxy::new(Seed::new(SEED));
            let mut cell = Vec::new();
            generate_cell(
                &galaxy,
                CellKey::new(Layer::C, [0, 812, 0]).unwrap(),
                &mut cell,
            );
            let mut systems = BTreeMap::new();
            let mut record = None;
            for candidate in &cell {
                let Ok(ctx) = SystemContext::for_system(&galaxy, candidate.id()) else {
                    continue;
                };
                let planets = generate(Seed::new(SEED), &ctx);
                let enough = planets.bodies().len() >= 4;
                systems.insert(candidate.id(), Arc::new((ctx, planets)));
                if enough {
                    record = Some(*candidate);
                    break;
                }
            }
            let record = record.expect("a cell of the solar circle holds a system of four bodies");
            let other = *cell
                .iter()
                .find(|other| other.id() != record.id())
                .expect("the cell holds another system");
            Self {
                galaxy,
                universe: UniverseIdHex::from_u64(1),
                cells: SharedCellCache::new(64 << 20),
                stars: SharedSystemCache::new(64 << 20),
                systems,
                record,
                other,
            }
        }

        fn world(&self) -> SceneWorld<'_> {
            SceneWorld {
                universe: &self.universe,
                galaxy: &self.galaxy,
                key: GalaxyKey::new(SEED, GENERATOR_VERSION),
                cells: &self.cells,
                stars: &self.stars,
                systems: &self.systems,
            }
        }

        fn id(&self) -> SystemId {
            self.record.id()
        }

        fn generated(&self) -> &GeneratedSystem {
            self.systems[&self.id()].as_ref()
        }

        /// The scene system's tidal radius at `t`.
        fn tidal_radius(&self, t: UniverseTime) -> Metres {
            let barycentre = position_at(&self.galaxy, &self.record, t);
            candidate_at(&self.galaxy, &self.record, &barycentre, t)
                .unwrap()
                .tidal_radius()
        }

        /// A point `ratio` of the tidal radius from the barycentre along `direction`.
        fn at_ratio(&self, direction: [f64; 3], ratio: f64, t: UniverseTime) -> GalacticPosition {
            let reach = self.tidal_radius(t).value() * ratio;
            position_at(&self.galaxy, &self.record, t)
                .translated(GalacticDisplacement::new(direction.map(|d| d * reach)))
                .unwrap()
        }
    }

    fn at(seconds: i64) -> UniverseTime {
        UniverseTime::new(seconds, 0).unwrap()
    }

    fn reading(t: UniverseTime) -> ClockReading {
        let anchor = Instant::now();
        SceneClock::new(t, anchor, TimeRate::PAUSED)
            .unwrap()
            .at_instant(anchor)
    }

    fn wire() -> KinematicsDto {
        KinematicsDto {
            position: FramePositionDto::Galactic {
                position: hyperion_protocol::GalacticPosition::default(),
            },
            velocity_m_s: [0.0; 3],
            time: wire_time(UniverseTime::EPOCH),
        }
    }

    fn galactic_ship(position: GalacticPosition) -> ShipStandIn {
        ShipStandIn::new(
            ShipPosition::Galactic(position),
            [0.0; 3],
            UniverseTime::EPOCH,
            wire(),
        )
    }

    /// A ship 1 au from the scene system's barycentre, in its frame, moving at 30 km/s.
    fn system_ship(fixture: &Fixture) -> ShipStandIn {
        ShipStandIn::new(
            ShipPosition::System {
                system: fixture.id(),
                offset: SystemPosition::new([1.496e11, 0.0, 0.0]),
            },
            [0.0, 3.0e4, 0.0],
            UniverseTime::EPOCH,
            wire(),
        )
    }

    fn inputs<'a>(
        t: UniverseTime,
        ship: &'a dyn Ship,
        knowledge: &'a dyn SceneKnowledge,
    ) -> SceneInputs<'a> {
        SceneInputs {
            clock: reading(t),
            ship,
            knowledge,
        }
    }

    /// Grants `contact` to the bodies of odd slots and the level asked to the rest.
    #[derive(Debug)]
    struct OddSlotsContacts;

    impl SceneKnowledge for OddSlotsContacts {
        fn grant(&self, body: BodyId, asked: DetailLevel) -> DetailLevel {
            if (body.body_index() >> 8) % 2 == 1 {
                DetailLevel::Contact
            } else {
                asked
            }
        }

        fn is_contact(&self, _craft: &CraftState) -> bool {
            false
        }
    }

    /// [`OddSlotsContacts`] where `contact` still resolves the body, so that every body it lowers
    /// can be re-sent at its new level; the rest keep the level asked.
    #[derive(Debug)]
    struct OddSlotsResolvedContacts;

    impl OddSlotsResolvedContacts {
        fn lowers(index: BodyIndex) -> bool {
            (index.get() >> 8) % 2 == 1 && DetailLevel::Contact.resolves(index)
        }
    }

    impl SceneKnowledge for OddSlotsResolvedContacts {
        fn grant(&self, body: BodyId, asked: DetailLevel) -> DetailLevel {
            match BodyIndex::try_from(body.body_index()) {
                Ok(index) if Self::lowers(index) => DetailLevel::Contact,
                _ => asked,
            }
        }

        fn is_contact(&self, _craft: &CraftState) -> bool {
            false
        }
    }

    /// Grants `contact`, which resolves no member of a belt, to every body `contact` does not
    /// resolve, and the level asked to the rest.
    #[derive(Debug)]
    struct BeltMembersAsContacts;

    impl SceneKnowledge for BeltMembersAsContacts {
        fn grant(&self, body: BodyId, asked: DetailLevel) -> DetailLevel {
            match BodyIndex::try_from(body.body_index()) {
                Ok(index) if !DetailLevel::Contact.resolves(index) => DetailLevel::Contact,
                _ => asked,
            }
        }

        fn is_contact(&self, _craft: &CraftState) -> bool {
            false
        }
    }

    fn system_of(state: &SceneStateDto) -> &SceneSystemDto {
        state.system.as_ref().expect("the scene is in a system")
    }

    fn contact_indices(system: &SceneSystemDto) -> BTreeSet<BodyIdHex> {
        system
            .grants
            .iter()
            .filter(|grant| grant.level == DetailLevelDto::Contact)
            .map(|grant| grant.body.clone())
            .collect()
    }

    #[test]
    fn the_scene_arrives_once_at_0_9_and_leaves_once_above_1_across_the_boundary() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let t = UniverseTime::EPOCH;
        let named = |position: &GalacticPosition, current| world.frame_at(position, t, current);
        let directions = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [0.6, 0.8, 0.0],
            [-0.6, 0.0, 0.8],
            [0.0, -0.8, 0.6],
        ];
        let id = Some(fixture.id());
        let direction = directions
            .into_iter()
            .find(|&direction| {
                named(&fixture.at_ratio(direction, 0.95, t), None) == id
                    && named(&fixture.at_ratio(direction, 0.999, t), id) == id
                    && named(&fixture.at_ratio(direction, 1.05, t), None).is_none()
            })
            .expect("a direction out of the sphere into interstellar space");
        let knowledge = GrantAsked;
        let at_ratio = |ratio| galactic_ship(fixture.at_ratio(direction, ratio, t));

        let outside = at_ratio(1.05);
        let (mut core, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(t, &outside, &knowledge),
            Vec::new(),
            &world,
        )
        .unwrap();
        assert_eq!(state.system, None);
        let mut arrivals = Vec::new();
        let entry = 0.9 * (1.0 - 1e-9);
        for ratio in [1.0, 0.95, entry, 0.95, 0.999, 1.05, 0.95, 1.05] {
            let ship = at_ratio(ratio);
            let delta = core
                .advance(inputs(t, &ship, &knowledge), &world, Beat::Change)
                .unwrap();
            if let Some(arrival) = delta.arrival {
                arrivals.push((ratio, arrival));
            }
        }
        assert_eq!(arrivals.len(), 2, "one arrival and one departure");
        let (
            arrived_at,
            SceneArrivalDto::System {
                system,
                tidal_radius_m,
            },
        ) = &arrivals[0]
        else {
            panic!("the first change is the arrival: {:?}", arrivals[0]);
        };
        assert_eq!(arrived_at.to_bits(), entry.to_bits());
        assert_eq!(system.system.hosts.system.to_u64(), fixture.id().raw());
        assert_eq!(
            tidal_radius_m.to_bits(),
            fixture.tidal_radius(t).value().to_bits()
        );
        assert_eq!(arrivals[1].0.to_bits(), 1.05_f64.to_bits());
        assert_eq!(arrivals[1].1, SceneArrivalDto::NoSystem);
    }

    #[test]
    fn a_ship_in_a_system_s_frame_holds_its_bodies_and_stars_and_the_galactic_frame_none() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let t = at(1_000);
        let ship = system_ship(&fixture);
        let (_, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(t, &ship, &GrantAsked),
            Vec::new(),
            &world,
        )
        .unwrap();
        let system = system_of(&state);
        assert!(
            !system.system.hosts.stars.is_empty(),
            "the system's stars are its hosts"
        );
        let (ctx, planets) = fixture.generated();
        let hosts = world.hosts(&fixture.record, t);
        let expected: SystemBodiesDto = system_bodies(
            BodiesRequest::new(fixture.id(), t, DetailLevel::Full),
            hosts,
            ctx,
            planets,
            world.seed(),
        );
        assert_eq!(
            system.system, expected,
            "the whole system, as system_bodies answers it"
        );
        assert!(
            system
                .grants
                .iter()
                .all(|grant| grant.level == DetailLevelDto::Full)
        );
        assert_eq!(state.ship, wire());
        assert_eq!(state.clock, SceneClockDto::from(reading(t)));
        assert_eq!(
            state.tidal_radius_m.map(f64::to_bits),
            Some(fixture.tidal_radius(t).value().to_bits()),
            "the state states the sphere a client subscribing inside the system clamps to"
        );

        let halo = galactic_ship(GalacticPosition::from_light_years([0.0, 0.0, 60_000.0]).unwrap());
        let (_, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(t, &halo, &GrantAsked),
            Vec::new(),
            &world,
        )
        .unwrap();
        assert_eq!(state.system, None);
        assert_eq!(state.tidal_radius_m, None);
    }

    #[test]
    fn each_record_is_degraded_to_its_own_grant_and_contacts_are_placed_where_the_ship_sees_them() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let t = at(86_400);
        let ship = system_ship(&fixture);
        let (_, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(t, &ship, &OddSlotsContacts),
            Vec::new(),
            &world,
        )
        .unwrap();
        let system = system_of(&state);
        let (ctx, planets) = fixture.generated();
        let observer = SystemObserver::new(
            SystemPosition::new([1.496e11, 0.0, 0.0]).translated(
                SystemVelocity::new([0.0, 3.0e4, 0.0])
                    .displacement_over(hyperion_sim::units::Seconds::new(86_400.0)),
            ),
            SystemVelocity::new([0.0, 3.0e4, 0.0]),
            t,
        )
        .unwrap();
        let contacts = contact_indices(system);
        assert!(!contacts.is_empty() && contacts.len() < system.grants.len());
        let mut placed = 0;
        assert_eq!(system.grants.len(), system.system.bodies.len());
        for (grant, record) in system.grants.iter().zip(&system.system.bodies) {
            assert_eq!(grant.body, record.id, "grants in the bodies' order");
            let odd = (grant.body.to_parts().1 >> 8) % 2 == 1;
            let own = if odd {
                DetailLevel::Contact
            } else {
                DetailLevel::Full
            };
            assert_eq!(contacts.contains(&grant.body), odd);
            let alone = system_bodies(
                BodiesRequest::new(fixture.id(), t, own),
                world.hosts(&fixture.record, t),
                ctx,
                planets,
                world.seed(),
            );
            let expected = alone
                .bodies
                .iter()
                .find(|body| body.id == record.id)
                .unwrap();
            assert_eq!(record, expected, "{:?} at its own level", record.id);
            let index = BodyIndex::try_from(record.id.to_parts().1).unwrap();
            if own == DetailLevel::Contact {
                // A population, or a body absent then, has no single position to see.
                let track = BodyTrack::new(planets, ctx, index).unwrap();
                match retarded_in_system(&observer, &track) {
                    Ok(seen) => {
                        let carried = grant.seen.expect("a contact carries its seen position");
                        assert_eq!(
                            carried.apparent_m.map(f64::to_bits),
                            seen.apparent().metres().map(f64::to_bits)
                        );
                        assert_eq!(carried.emitted, wire_time(seen.emitted()));
                        placed += 1;
                    }
                    Err(_) => assert_eq!(grant.seen, None),
                }
                assert_eq!(record.orbit, hyperion_protocol::SectionDto::NotResolved);
                assert_eq!(record.mass_kg, hyperion_protocol::SectionDto::NotResolved);
            } else {
                assert_eq!(grant.seen, None);
            }
        }
        assert!(placed > 0, "some contacts are placed by the ship's sight");
    }

    #[test]
    fn a_knowledge_change_re_sends_exactly_the_bodies_it_touched_with_their_new_level() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let ship = system_ship(&fixture);
        let (mut core, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(at(10), &ship, &GrantAsked),
            Vec::new(),
            &world,
        )
        .unwrap();
        let unchanged = core
            .advance(inputs(at(20), &ship, &GrantAsked), &world, Beat::Change)
            .unwrap();
        assert_eq!(unchanged, SceneDelta::default());
        let delta = core
            .advance(
                inputs(at(30), &ship, &OddSlotsResolvedContacts),
                &world,
                Beat::Change,
            )
            .unwrap();
        assert_eq!(delta.arrival, None);
        let touched: BTreeSet<BodyIdHex> = system_of(&state)
            .grants
            .iter()
            .filter(|grant| {
                OddSlotsResolvedContacts::lowers(
                    BodyIndex::try_from(grant.body.to_parts().1).unwrap(),
                )
            })
            .map(|grant| grant.body.clone())
            .collect();
        assert!(!touched.is_empty());
        let re_sent: BTreeSet<BodyIdHex> = delta
            .bodies
            .iter()
            .map(|body| body.record.id.clone())
            .collect();
        assert_eq!(re_sent, touched);
        assert!(
            delta
                .bodies
                .iter()
                .all(|body| body.level == DetailLevelDto::Contact)
        );
        assert!(delta.bodies.iter().any(|body| body.seen.is_some()));
    }

    #[test]
    fn a_grant_that_withdraws_a_body_sent_brings_the_whole_system_again_without_it() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let ship = system_ship(&fixture);
        let (mut core, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(at(10), &ship, &GrantAsked),
            Vec::new(),
            &world,
        )
        .unwrap();
        let withdrawn: BTreeSet<BodyIdHex> = system_of(&state)
            .grants
            .iter()
            .filter(|grant| {
                !DetailLevel::Contact
                    .resolves(BodyIndex::try_from(grant.body.to_parts().1).unwrap())
            })
            .map(|grant| grant.body.clone())
            .collect();
        assert!(
            !withdrawn.is_empty(),
            "the fixture's system has a belt's members"
        );

        let delta = core
            .advance(
                inputs(at(30), &ship, &BeltMembersAsContacts),
                &world,
                Beat::Change,
            )
            .unwrap();
        let Some(SceneArrivalDto::System {
            system,
            tidal_radius_m,
        }) = delta.arrival
        else {
            panic!("the system arrives again: {:?}", delta.arrival);
        };
        assert!(delta.bodies.is_empty());
        assert_eq!(system.system.hosts.system.to_u64(), fixture.id().raw());
        assert_eq!(
            tidal_radius_m.to_bits(),
            fixture.tidal_radius(at(30)).value().to_bits()
        );
        let listed: BTreeSet<BodyIdHex> = system
            .system
            .bodies
            .iter()
            .map(|body| body.id.clone())
            .collect();
        assert!(listed.is_disjoint(&withdrawn), "no withdrawn body is sent");
        assert!(!listed.is_empty());
        // Once rebuilt, the scene goes on with nothing more to send.
        let next = core
            .advance(
                inputs(at(40), &ship, &BeltMembersAsContacts),
                &world,
                Beat::Change,
            )
            .unwrap();
        assert_eq!(next, SceneDelta::default());
    }

    #[test]
    fn a_heartbeat_re_sends_every_seen_body_and_only_those() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let ship = system_ship(&fixture);
        let (mut core, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(at(10), &ship, &OddSlotsContacts),
            Vec::new(),
            &world,
        )
        .unwrap();
        let contacts: BTreeSet<BodyIdHex> = system_of(&state)
            .grants
            .iter()
            .filter(|grant| grant.seen.is_some())
            .map(|grant| grant.body.clone())
            .collect();
        assert!(!contacts.is_empty());
        let beat = core
            .advance(
                inputs(at(11), &ship, &OddSlotsContacts),
                &world,
                Beat::Heartbeat,
            )
            .unwrap();
        let re_sent: BTreeSet<BodyIdHex> = beat
            .bodies
            .iter()
            .map(|body| body.record.id.clone())
            .collect();
        assert_eq!(
            re_sent, contacts,
            "the contacts the ship sees, and no other body"
        );
        assert!(beat.bodies.iter().all(|body| body.seen.is_some()));
        let quiet = core
            .advance(
                inputs(at(12), &ship, &OddSlotsContacts),
                &world,
                Beat::Change,
            )
            .unwrap();
        assert_eq!(quiet, SceneDelta::default());
    }

    /// Plants a `valid_until` on the first `n` non-contact bodies sent, `due` apart from `from`:
    /// no body of the pinned systems changes inside the clock window (plan 14's goldens hold none),
    /// so the re-send is tested on the core's own record of what it sent.
    fn plant(core: &mut SceneCore, n: usize, from: UniverseTime, every: i64) -> Vec<BodyIndex> {
        let system = core.system.as_mut().unwrap();
        let mut planted = Vec::new();
        for (k, (index, sent)) in system
            .sent
            .iter_mut()
            .filter(|(_, sent)| sent.level != DetailLevel::Contact)
            .take(n)
            .enumerate()
        {
            let k = i64::try_from(k).unwrap();
            sent.valid_until = from.checked_add(Span::from_seconds(every * (k + 1)));
            planted.push(*index);
        }
        planted
    }

    #[test]
    fn a_body_whose_valid_until_passes_is_re_sent_once_and_next_due_names_it() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let ship = system_ship(&fixture);
        let (mut core, _) = SceneCore::build(
            DetailLevel::Full,
            inputs(at(0), &ship, &GrantAsked),
            Vec::new(),
            &world,
        )
        .unwrap();
        let planted = plant(&mut core, 1, at(0), 10);
        assert_eq!(core.next_due(), Some(at(10)));
        let early = core
            .advance(inputs(at(5), &ship, &GrantAsked), &world, Beat::Change)
            .unwrap();
        assert_eq!(early, SceneDelta::default());
        let due = core
            .advance(inputs(at(12), &ship, &GrantAsked), &world, Beat::Change)
            .unwrap();
        assert_eq!(due.bodies.len(), 1);
        let index = u16::from(planted[0]);
        assert_eq!(
            due.bodies[0].record.id,
            BodyIdHex::from_parts(fixture.id().raw(), index)
        );
        let (ctx, planets) = fixture.generated();
        let (expected, _) =
            scene_body(ctx, planets, planted[0], at(10), DetailLevel::Full, None).unwrap();
        assert_eq!(
            due.bodies[0], expected,
            "evaluated when its elements changed"
        );
        assert_eq!(
            core.next_due(),
            None,
            "the body's own record holds no later change"
        );
        let after = core
            .advance(inputs(at(20), &ship, &GrantAsked), &world, Beat::Change)
            .unwrap();
        assert_eq!(after, SceneDelta::default());
    }

    #[test]
    fn advancing_in_one_step_or_ten_gives_the_same_merged_delta() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let ship = system_ship(&fixture);
        let build = || {
            let (mut core, _) = SceneCore::build(
                DetailLevel::Full,
                inputs(at(0), &ship, &GrantAsked),
                Vec::new(),
                &world,
            )
            .unwrap();
            plant(&mut core, 3, at(0), 3);
            core
        };
        let push = |delta: SceneDelta, t| {
            let mut push = ScenePush::heartbeat(SceneClockDto::from(reading(t)));
            push.arrival = delta.arrival;
            push.bodies = delta.bodies;
            push
        };
        let mut stepped = build();
        let mut merged: Option<ScenePush> = None;
        for second in 1..=10 {
            let delta = stepped
                .advance(inputs(at(second), &ship, &GrantAsked), &world, Beat::Change)
                .unwrap();
            let next = push(delta, at(second));
            match merged.as_mut() {
                Some(merged) => merged.merge(next),
                None => merged = Some(next),
            }
        }
        let mut jumped = build();
        let once = jumped
            .advance(inputs(at(10), &ship, &GrantAsked), &world, Beat::Change)
            .unwrap();
        let mut once = push(once, at(10));
        let mut merged = merged.unwrap();
        // Merged bodies keep the order each was first sent in; one step sends in index order.
        merged.bodies.sort_by(|a, b| a.record.id.cmp(&b.record.id));
        once.bodies.sort_by(|a, b| a.record.id.cmp(&b.record.id));
        assert_eq!(merged.bodies.len(), 3);
        assert_eq!(merged, once);
    }

    #[test]
    fn a_ship_in_a_body_s_frame_observes_from_the_body_plus_its_offset() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let t = at(3_600);
        let (ctx, planets) = fixture.generated();
        let body = planets
            .bodies()
            .iter()
            .map(hyperion_sim::planetary::Body::index)
            .find(|&index| planets.state_at(ctx, index, t).ok().flatten().is_some())
            .expect("a body present at the time");
        let offset = BodyPosition::new([7.0e6, 0.0, 0.0]);
        let ship = ShipStandIn::new(
            ShipPosition::Body {
                body: body.body_id(fixture.id()),
                offset,
            },
            [0.0, 7_500.0, 0.0],
            t,
            wire(),
        );
        let (centre, moving) = planets.state_at(ctx, body, t).unwrap().unwrap();
        let expected = observer(&world, &fixture.record, fixture.generated(), &ship, t)
            .unwrap()
            .unwrap();
        assert_eq!(
            expected.position().metres().map(f64::to_bits),
            offset.to_system(&centre).metres().map(f64::to_bits)
        );
        let velocity = moving + SystemVelocity::new([0.0, 7_500.0, 0.0]);
        assert_eq!(
            expected.velocity().metres_per_second().map(f64::to_bits),
            velocity.metres_per_second().map(f64::to_bits)
        );
        // Through the galactic frame the same ship lands within a metre of it, a light-year cell's
        // offsets resolving about that finely.
        let galactic = ship_galactic(&world, ship.position_at(t), t).unwrap();
        let barycentre = position_at(world.galaxy, &fixture.record, t);
        let through = SystemPosition::from_galactic(&galactic, &barycentre);
        let gap = through
            .displacement_to(expected.position())
            .length()
            .value();
        assert!(gap < 2.0, "{gap} m");
        // The scene is the body's system's.
        let (_, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(t, &ship, &GrantAsked),
            Vec::new(),
            &world,
        )
        .unwrap();
        assert_eq!(
            system_of(&state).system.hosts.system.to_u64(),
            fixture.id().raw()
        );
    }

    fn camera(view: u8, position: FramePositionDto) -> CameraReportDto {
        CameraReportDto {
            view,
            pose: KinematicsDto {
                position,
                velocity_m_s: [0.0; 3],
                time: wire_time(UniverseTime::EPOCH),
            },
        }
    }

    #[test]
    fn cameras_out_of_reach_and_a_ninth_camera_are_refused() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let t = at(0);
        let ship = system_ship(&fixture);
        let (mut core, state) = SceneCore::build(
            DetailLevel::Full,
            inputs(t, &ship, &GrantAsked),
            Vec::new(),
            &world,
        )
        .unwrap();
        let in_system = |view, offset_m| {
            camera(
                view,
                FramePositionDto::System {
                    system: SystemIdHex::from_u64(fixture.id().raw()),
                    offset_m,
                },
            )
        };
        let a_body = system_of(&state).system.bodies[0].id.clone();
        let good = vec![
            in_system(0, [1.0e12, 0.0, 0.0]),
            camera(
                1,
                FramePositionDto::Body {
                    body: a_body,
                    offset_m: [1.0e7, 0.0, 0.0],
                },
            ),
            in_system(0, [2.0e12, 0.0, 0.0]),
        ];
        core.set_cameras(good, t, &world).unwrap();
        let views: Vec<(u8, FramePositionDto)> = core
            .cameras()
            .map(|camera| (camera.view, camera.pose.position.clone()))
            .collect();
        assert_eq!(views.len(), 2, "the latest report per view");
        assert_eq!(views[0].1, in_system(0, [2.0e12, 0.0, 0.0]).pose.position);

        let refused = |core: &mut SceneCore, cameras| {
            let error = core.set_cameras(cameras, t, &world).unwrap_err();
            (error.code, error.field)
        };
        let named = (ErrorCode::BadRequest, Some("cameras".to_owned()));
        let other = camera(
            2,
            FramePositionDto::System {
                system: SystemIdHex::from_u64(fixture.other.id().raw()),
                offset_m: [0.0; 3],
            },
        );
        assert_eq!(refused(&mut core, vec![other]), named);
        let beyond = fixture.tidal_radius(t).value() * 1.01;
        assert_eq!(
            refused(&mut core, vec![in_system(3, [beyond, 0.0, 0.0])]),
            named
        );
        let nine = (0..9).map(|view| in_system(view, [0.0; 3])).collect();
        assert_eq!(refused(&mut core, nine), named);
        assert_eq!(
            refused(&mut core, vec![in_system(4, [f64::NAN, 0.0, 0.0])]),
            named
        );
        let mut moving = in_system(4, [0.0; 3]);
        moving.pose.velocity_m_s = [f64::INFINITY, 0.0, 0.0];
        assert_eq!(refused(&mut core, vec![moving]), named);
        let not_in_scene = camera(
            5,
            FramePositionDto::Body {
                body: BodyIdHex::from_parts(fixture.id().raw(), 0xCF00),
                offset_m: [0.0; 3],
            },
        );
        assert_eq!(refused(&mut core, vec![not_in_scene]), named);
        let off_the_cube = camera(
            6,
            FramePositionDto::Galactic {
                position: hyperion_protocol::GalacticPosition {
                    cell_ly: [0, 900_000, 0],
                    offset_m: [0.0; 3],
                },
            },
        );
        assert_eq!(refused(&mut core, vec![off_the_cube]), named);
        assert_eq!(
            core.cameras().count(),
            2,
            "a refused report leaves the cameras as they were"
        );
    }

    #[test]
    fn in_the_galactic_frame_only_a_galactic_camera_is_in_reach() {
        let fixture = Fixture::new();
        let world = fixture.world();
        let t = at(0);
        let named = (ErrorCode::BadRequest, Some("cameras".to_owned()));
        let in_system = camera(
            0,
            FramePositionDto::System {
                system: SystemIdHex::from_u64(fixture.id().raw()),
                offset_m: [0.0; 3],
            },
        );
        let halo = galactic_ship(GalacticPosition::from_light_years([0.0, 0.0, 60_000.0]).unwrap());
        let (mut galactic, _) = SceneCore::build(
            DetailLevel::Full,
            inputs(t, &halo, &GrantAsked),
            Vec::new(),
            &world,
        )
        .unwrap();
        let error = galactic
            .set_cameras(vec![in_system], t, &world)
            .unwrap_err();
        assert_eq!((error.code, error.field), named);
        let free = camera(
            0,
            FramePositionDto::Galactic {
                position: hyperion_protocol::GalacticPosition {
                    cell_ly: [0, 0, 60_000],
                    offset_m: [0.0; 3],
                },
            },
        );
        galactic.set_cameras(vec![free], t, &world).unwrap();
    }

    #[test]
    fn craft_are_the_contacts_and_an_empty_list_is_sent_once_when_the_last_leaves() {
        #[derive(Debug)]
        struct Even;
        impl SceneKnowledge for Even {
            fn grant(&self, _body: BodyId, asked: DetailLevel) -> DetailLevel {
                asked
            }
            fn is_contact(&self, craft: &CraftState) -> bool {
                craft.id().ends_with(['0', '2', '4', '6', '8'])
            }
        }
        let craft = |n: u32| {
            (0..n)
                .map(|k| {
                    CraftState::new(SceneCraftDto {
                        craft: format!("craft-{k}"),
                        hull: "test".to_owned(),
                        state: wire(),
                        attitude: [1.0, 0.0, 0.0, 0.0],
                        angular_velocity_rad_s: [0.0; 3],
                        planned_path: None,
                    })
                })
                .collect::<Vec<_>>()
        };
        let mut core = SceneCore {
            asked: DetailLevel::Full,
            system: None,
            cameras: BTreeMap::new(),
            craft_sent: false,
        };
        let listed = core.craft(&Even, craft(10)).unwrap();
        assert_eq!(listed.len(), 5);
        assert!(core.has_craft());
        assert_eq!(core.craft(&Even, Vec::new()), Some(Vec::new()));
        assert!(!core.has_craft());
        assert_eq!(core.craft(&Even, Vec::new()), None);
    }

    #[test]
    fn only_an_arrival_is_a_large_notification() {
        let quiet = SceneNotificationDto {
            sequence: 1,
            clock: SceneClockDto::from(reading(at(0))),
            ship: None,
            arrival: None,
            bodies: Vec::new(),
            craft: None,
        };
        assert!(!is_large_notification(&quiet));
        let leaving = SceneNotificationDto {
            arrival: Some(SceneArrivalDto::NoSystem),
            ..quiet
        };
        assert!(is_large_notification(&leaving));
    }
}
