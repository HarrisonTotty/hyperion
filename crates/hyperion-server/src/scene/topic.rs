//! The scene topic, live (rendering plan R03, R03.T8): a subscription's opening and the task that
//! keeps it.
//!
//! [`open`] builds the scene's core on the CPU pool, fetching any planetary system it asks for from
//! the body cache, and answers the whole state. A task per subscription, handed to its [`Pusher`]
//! so that it ends with the subscription, then waits on the universe's scene setting, the next
//! `valid_until` mapped to an instant by the clock, the cameras the connection routes to it, the
//! heartbeat and, while the scene holds craft, the 64 Hz tick, and merges each change into the
//! subscription's pending push (Design notes 4 and 5). The core's work runs on the pool; the task
//! only waits.

use std::collections::BTreeMap;
use std::sync::Arc;

use hyperion_protocol::{
    CameraReportDto, RequestError, SceneClockDto, SceneSubscribeRequest, SubscriptionState,
    UniverseIdHex,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::record::DetailLevel;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, interval, interval_at, sleep_until};

use super::core::{Beat, FetchSystemError, SceneCore, SceneDelta, SceneInputs, SceneWorld};
use super::{Clock, SceneSetting, Ship};
use crate::AppState;
use crate::compute::{
    CancelOnDrop, CancelToken, GalaxyKey, GenerateBodiesError, GeneratedSystem, JobError, Priority,
};
use crate::limits::{CRAFT_PUSH_INTERVAL, SCENE_HEARTBEAT};
use crate::requests::{bodies_of, openable_universe};
use crate::subscriptions::{PendingPush, Pusher, ScenePush, SubscriptionCommand};
use crate::universe::UniverseId;

/// Where a scene's computations run and what they read: the universe, its galaxy, and the
/// planetary systems fetched for it so far.
#[derive(Debug, Clone)]
pub(crate) struct SceneSource {
    state: Arc<AppState>,
    universe: UniverseIdHex,
    id: UniverseId,
    key: GalaxyKey,
    galaxy: Arc<Galaxy>,
    systems: Arc<BTreeMap<SystemId, Arc<GeneratedSystem>>>,
}

impl SceneSource {
    /// Runs `job` on the CPU pool over `core` and the scene's world, fetching from the body cache
    /// each planetary system it asks for and running it again, until it answers.
    ///
    /// # Errors
    ///
    /// `internal` if the pool is shutting down, and `cancelled` once `token` is; those of the body
    /// cache for a system it cannot generate.
    async fn run<C, T, J>(
        &mut self,
        token: &CancelToken,
        mut core: C,
        mut job: J,
    ) -> Result<(C, T), RequestError>
    where
        C: Send + 'static,
        T: Send + 'static,
        J: FnMut(&mut C, &SceneWorld<'_>) -> Result<T, FetchSystemError> + Send + 'static,
    {
        loop {
            let source = self.clone();
            let receiver = self
                .state
                .pool
                .submit(Priority::Interactive, token.clone(), move |_| {
                    let world = SceneWorld {
                        universe: &source.universe,
                        galaxy: &source.galaxy,
                        key: source.key,
                        cells: &source.state.cells,
                        stars: &source.state.systems,
                        systems: &source.systems,
                    };
                    let answer = job(&mut core, &world);
                    (core, job, answer)
                })
                .await?;
            let (returned, returned_job, answer) = receiver
                .await
                .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
            core = returned;
            job = returned_job;
            match answer {
                Ok(value) => return Ok((core, value)),
                Err(FetchSystemError(id)) => {
                    let system = bodies_of(&self.state, self.key, &self.galaxy, id)
                        .await
                        .map_err(|error| match error {
                            GenerateBodiesError::NoSuchSystem(error) => {
                                tracing::error!(%error, "a scene's system did not resolve");
                                crate::requests::request_error(
                                    hyperion_protocol::ErrorCode::Internal,
                                    "the scene's system could not be generated",
                                )
                            }
                            GenerateBodiesError::Compute(error) => error.into(),
                        })?;
                    Arc::make_mut(&mut self.systems).insert(id, system);
                }
            }
        }
    }
}

/// Opens a scene subscription: builds the core, checks the cameras, and starts the task that keeps
/// the subscription; answers the whole state.
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming `cameras` for cameras out of the scene's
/// reach (Design note 6); those of the galaxy cache; and `internal` or `cancelled` from the pool.
pub(crate) async fn open(
    state: Arc<AppState>,
    universe: UniverseIdHex,
    request: SceneSubscribeRequest,
    pusher: Pusher,
    token: CancelToken,
) -> Result<SubscriptionState, RequestError> {
    let opened = openable_universe(&state, &universe)?;
    let key = opened.key();
    let galaxy = state.galaxies.get(key).await?;
    let mut source = SceneSource {
        state: Arc::clone(&state),
        universe,
        id: opened.id(),
        key,
        galaxy,
        systems: Arc::new(BTreeMap::new()),
    };
    let mut setting = state.scene.watch(source.id);
    let current = setting.borrow_and_update().clone();
    let asked = crate::convert::detail_level(request.detail);
    let (core, scene) = build(&mut source, &token, asked, &current, request.cameras).await?;
    let commands = pusher.take_commands();
    let topic = Topic {
        source,
        guard: CancelOnDrop::new(CancelToken::new()),
        core,
        setting,
        current,
        commands,
        pusher: pusher.clone(),
    };
    let task = tokio::spawn(topic.run());
    pusher.attach(task.abort_handle());
    Ok(SubscriptionState::Scene(scene))
}

/// Builds a scene's core and its whole state, with `cameras` checked and set.
async fn build(
    source: &mut SceneSource,
    token: &CancelToken,
    asked: DetailLevel,
    setting: &SceneSetting,
    cameras: Vec<CameraReportDto>,
) -> Result<(SceneCore, hyperion_protocol::SceneStateDto), RequestError> {
    let reading = setting.clock.reading_at(Instant::now());
    let craft = source
        .state
        .scene
        .craft()
        .craft_at(source.id, reading.time());
    let knowledge = source.state.scene.knowledge();
    let ship = setting.ship.clone();
    let (built, cameras_set) = source
        .run(
            token,
            None::<(SceneCore, hyperion_protocol::SceneStateDto)>,
            {
                move |built, world| {
                    let inputs = SceneInputs {
                        clock: reading,
                        ship: &ship,
                        knowledge: knowledge.as_ref(),
                    };
                    let (mut core, scene) = SceneCore::build(asked, inputs, craft.clone(), world)?;
                    let cameras_set = core.set_cameras(cameras.clone(), reading.time(), world);
                    *built = Some((core, scene));
                    Ok(cameras_set)
                }
            },
        )
        .await?;
    cameras_set?;
    Ok(built.expect("the job built the core when it answered"))
}

/// One scene subscription's task.
struct Topic {
    source: SceneSource,
    /// Cancels the task's pool jobs when the task ends, so that one still queued is skipped.
    guard: CancelOnDrop,
    core: SceneCore,
    setting: watch::Receiver<SceneSetting>,
    current: SceneSetting,
    commands: Option<mpsc::Receiver<SubscriptionCommand>>,
    pusher: Pusher,
}

/// What woke a scene's task.
enum Wake {
    Ended,
    Setting,
    Due,
    Heartbeat,
    Craft,
    Command(SubscriptionCommand),
}

impl Topic {
    /// Keeps the subscription until it ends.
    async fn run(mut self) {
        let mut heartbeat = interval_at(Instant::now() + SCENE_HEARTBEAT, SCENE_HEARTBEAT);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut craft = interval(CRAFT_PUSH_INTERVAL);
        craft.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            let due = self
                .core
                .next_due()
                .and_then(|time| Clock::instant_of(&self.current.clock, time));
            let has_craft = self.core.has_craft();
            let commands = self.commands.as_mut();
            let wake = tokio::select! {
                biased;
                () = self.pusher.ended() => Wake::Ended,
                changed = self.setting.changed() => match changed {
                    Ok(()) => Wake::Setting,
                    Err(_) => Wake::Ended,
                },
                Some(command) = async { match commands {
                    Some(commands) => commands.recv().await,
                    None => std::future::pending().await,
                } } => Wake::Command(command),
                () = sleep_until(due.unwrap_or_else(Instant::now)), if due.is_some() => Wake::Due,
                _ = heartbeat.tick() => Wake::Heartbeat,
                _ = craft.tick(), if has_craft => Wake::Craft,
            };
            let kept = match wake {
                Wake::Ended => return,
                Wake::Setting => self.on_setting().await,
                Wake::Due => self.advance(Beat::Change).await,
                Wake::Heartbeat => self.on_heartbeat().await,
                Wake::Craft => {
                    self.push_craft();
                    Ok(())
                }
                Wake::Command(command) => self.on_command(command).await,
            };
            if let Err(error) = kept {
                tracing::warn!(code = ?error.code, message = %error.message, "a scene subscription stopped");
                return;
            }
        }
    }

    /// The clock as it reads now.
    fn clock(&self) -> SceneClockDto {
        SceneClockDto::from(self.current.clock.reading_at(Instant::now()))
    }

    /// Takes a new scene setting: pushes the clock and the ship, and what moving the ship changed.
    async fn on_setting(&mut self) -> Result<(), RequestError> {
        self.current = self.setting.borrow_and_update().clone();
        let delta = self.delta(Beat::Change).await?;
        let mut push = self.push(delta);
        push.ship = Some(self.current.ship.kinematics());
        self.pusher.push(PendingPush::Scene(push));
        Ok(())
    }

    /// Advances the core and pushes what changed, if anything did.
    async fn advance(&mut self, beat: Beat) -> Result<(), RequestError> {
        let delta = self.delta(beat).await?;
        if delta != SceneDelta::default() {
            let push = self.push(delta);
            self.pusher.push(PendingPush::Scene(push));
        }
        Ok(())
    }

    /// The heartbeat: the clock, the contacts refreshed and anything else that changed, and the
    /// craft once some appear.
    async fn on_heartbeat(&mut self) -> Result<(), RequestError> {
        let delta = self.delta(Beat::Heartbeat).await?;
        let push = self.push(delta);
        self.pusher.push(PendingPush::Scene(push));
        if !self.core.has_craft() {
            self.push_craft();
        }
        Ok(())
    }

    /// Pushes the craft list, if the core has one to push.
    fn push_craft(&mut self) {
        let time = self.current.clock.reading_at(Instant::now()).time();
        let craft = self
            .source
            .state
            .scene
            .craft()
            .craft_at(self.source.id, time);
        let knowledge = self.source.state.scene.knowledge();
        if let Some(list) = self.core.craft(knowledge.as_ref(), craft) {
            let mut push = ScenePush::heartbeat(self.clock());
            push.craft = Some(list);
            self.pusher.push(PendingPush::Scene(push));
        }
    }

    /// Answers a command the connection routed here.
    async fn on_command(&mut self, command: SubscriptionCommand) -> Result<(), RequestError> {
        match command {
            SubscriptionCommand::SceneCameras { cameras, answer } => {
                let time = self.current.clock.reading_at(Instant::now()).time();
                let core = self.core.clone();
                let (core, checked) = self
                    .source
                    .run(self.guard.token(), core, move |core, world| {
                        Ok(core.set_cameras(cameras.clone(), time, world))
                    })
                    .await?;
                if checked.is_ok() {
                    self.core = core;
                }
                // The request may have been cancelled meanwhile; the answer then goes nowhere.
                let _ = answer.send(checked);
                Ok(())
            }
        }
    }

    /// Advances the core on the pool at the clock's present reading.
    async fn delta(&mut self, beat: Beat) -> Result<SceneDelta, RequestError> {
        let reading = self.current.clock.reading_at(Instant::now());
        let ship = self.current.ship.clone();
        let knowledge = self.source.state.scene.knowledge();
        let core = self.core.clone();
        let (core, delta) = self
            .source
            .run(self.guard.token(), core, move |core, world| {
                let inputs = SceneInputs {
                    clock: reading,
                    ship: &ship,
                    knowledge: knowledge.as_ref(),
                };
                core.advance(inputs, world, beat)
            })
            .await?;
        self.core = core;
        Ok(delta)
    }

    /// A push of `delta` with the clock as it reads now.
    fn push(&self, delta: SceneDelta) -> ScenePush {
        let mut push = ScenePush::heartbeat(self.clock());
        push.arrival = delta.arrival;
        push.bodies = delta.bodies;
        push
    }
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{
        FramePositionDto, KinematicsDto, NotificationBody, RequestId, ServerMessage,
    };
    use hyperion_sim::coords::SystemPosition;
    use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    use hyperion_sim::id::Layer;
    use hyperion_sim::time::{Span, UniverseTime};
    use tokio::time::timeout;

    use super::*;
    use crate::requests::Handlers;
    use crate::scene::{SceneClock, ShipPosition, ShipStandIn, TimeRate};
    use crate::subscriptions::Subscriptions;
    use crate::testing::{Harness, WAIT};

    /// The stand-in 1 au from `system`'s barycentre at rest, the clock at `start` at 100,000x.
    fn in_system(system: hyperion_sim::id::SystemId, start: UniverseTime) -> SceneSetting {
        let wire = KinematicsDto {
            position: FramePositionDto::Galactic {
                position: hyperion_protocol::GalacticPosition::default(),
            },
            velocity_m_s: [0.0; 3],
            time: hyperion_protocol::UniverseTime {
                seconds: 3_600,
                nanos: 0,
            },
        };
        SceneSetting {
            clock: SceneClock::new(start, Instant::now(), TimeRate::new(100_000).unwrap()).unwrap(),
            ship: ShipStandIn::new(
                ShipPosition::System {
                    system,
                    offset: SystemPosition::new([1.496e11, 0.0, 0.0]),
                },
                [0.0; 3],
                start,
                wire,
            ),
        }
    }

    /// A body whose `valid_until` passes at 100,000× is pushed when the clock reaches it, before
    /// the first heartbeat (R03.T8.a).
    #[tokio::test]
    async fn a_body_whose_valid_until_passes_at_100_000x_is_pushed() {
        const SEED: u64 = 0x4d2;
        let harness = Harness::start(Handlers).await;
        let state = Arc::clone(harness.state());
        let universe = state
            .registry
            .create("Scene".parse().unwrap(), Some(SEED))
            .await
            .unwrap();
        let galaxy = state.galaxies.get(universe.key()).await.unwrap();
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::new(Layer::C, [0, 812, 0]).unwrap(),
            &mut cell,
        );
        let system = cell[0].id();
        let start = UniverseTime::new(3_600, 0).unwrap();
        let setting = in_system(system, start);
        state.scene.set(universe.id(), setting.clone());

        let mut subscriptions = Subscriptions::new();
        let (_, pusher) = subscriptions.reserve(RequestId(1)).unwrap();
        subscriptions.went_live(RequestId(1));
        let mut source = SceneSource {
            state: Arc::clone(&state),
            universe: universe.id().into(),
            id: universe.id(),
            key: universe.key(),
            galaxy,
            systems: Arc::new(BTreeMap::new()),
        };
        let token = CancelToken::new();
        let (mut core, scene) = build(&mut source, &token, DetailLevel::Full, &setting, Vec::new())
            .await
            .unwrap();
        assert!(
            scene.system.is_some(),
            "the stand-in is in the system's frame"
        );
        // Half a real second ahead at 100,000×: before the first heartbeat.
        let due = start.checked_add(Span::from_seconds(50_000)).unwrap();
        let planted = core
            .plant_valid_until(due)
            .expect("a body that is not a contact");
        let topic = Topic {
            source,
            guard: CancelOnDrop::new(CancelToken::new()),
            core,
            setting: state.scene.watch(universe.id()),
            current: setting,
            commands: pusher.take_commands(),
            pusher: pusher.clone(),
        };
        let task = tokio::spawn(topic.run());
        pusher.attach(task.abort_handle());

        let unsent = timeout(WAIT, async {
            loop {
                subscriptions.woken().await;
                if let Some(unsent) = subscriptions.next_unsent(None) {
                    break unsent;
                }
            }
        })
        .await
        .expect("a push within the wait");
        let ready = unsent.serialise();
        let ServerMessage::Notification {
            body: NotificationBody::Scene(notification),
            ..
        } = serde_json::from_str(&subscriptions.sent(ready)).unwrap()
        else {
            panic!("a scene notification");
        };
        assert_eq!(notification.sequence, 1);
        assert_eq!(
            notification.bodies.len(),
            1,
            "the body whose elements changed, alone"
        );
        assert_eq!(
            notification.bodies[0].record.id.to_parts(),
            (system.raw(), u16::from(planted))
        );
        let reached = UniverseTime::new(
            notification.clock.time.seconds,
            notification.clock.time.nanos,
        )
        .unwrap();
        assert!(
            reached >= due,
            "pushed once the clock reached it: {reached}"
        );
        drop(subscriptions);
        harness.stop().await;
    }
}
