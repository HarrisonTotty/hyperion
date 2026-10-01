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
use super::{Clock, ClockReading, SceneSetting, Ship, ShipPosition};
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
    /// Drops every planetary system but those `keep` names.
    fn keep_only(&mut self, keep: &[Option<SystemId>]) {
        if self.systems.keys().any(|id| !keep.contains(&Some(*id))) {
            Arc::make_mut(&mut self.systems).retain(|id, _| keep.contains(&Some(*id)));
        }
    }

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
        core: Some(core),
        setting,
        current,
        commands,
        pusher: pusher.clone(),
    };
    let task = tokio::spawn(topic.run());
    pusher.attach(task.abort_handle());
    Ok(SubscriptionState::Scene(scene))
}

/// A push of `delta`, stating the clock `reading` it was evaluated at.
#[must_use]
fn push_of(reading: ClockReading, delta: SceneDelta) -> ScenePush {
    let mut push = ScenePush::heartbeat(SceneClockDto::from(reading));
    push.arrival = delta.arrival;
    push.bodies = delta.bodies;
    push
}

/// The system whose frame, or whose body's frame, a ship position is in.
#[must_use]
fn frame_system(position: &ShipPosition) -> Option<SystemId> {
    match position {
        ShipPosition::Galactic(_) => None,
        ShipPosition::System { system, .. } => Some(*system),
        ShipPosition::Body { body, .. } => Some(body.system()),
    }
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
    /// The core, home between pool jobs; a job takes it and gives it back, and a task that stops
    /// on a failed job ends without it.
    core: Option<SceneCore>,
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
                .core()
                .next_due()
                .and_then(|time| Clock::instant_of(&self.current.clock, time));
            let has_craft = self.core().has_craft();
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

    /// Takes a new scene setting: pushes the clock and the ship, and what moving the ship changed.
    async fn on_setting(&mut self) -> Result<(), RequestError> {
        self.current = self.setting.borrow_and_update().clone();
        let (reading, delta) = self.delta(Beat::Change).await?;
        let mut push = push_of(reading, delta);
        push.ship = Some(self.current.ship.kinematics());
        self.pusher.push(PendingPush::Scene(push));
        Ok(())
    }

    /// Advances the core and pushes what changed, if anything did.
    async fn advance(&mut self, beat: Beat) -> Result<(), RequestError> {
        let (reading, delta) = self.delta(beat).await?;
        if delta != SceneDelta::default() {
            let push = push_of(reading, delta);
            self.pusher.push(PendingPush::Scene(push));
        }
        Ok(())
    }

    /// The heartbeat: the clock, the contacts refreshed and anything else that changed, and the
    /// craft once some appear.
    async fn on_heartbeat(&mut self) -> Result<(), RequestError> {
        let (reading, delta) = self.delta(Beat::Heartbeat).await?;
        let push = push_of(reading, delta);
        self.pusher.push(PendingPush::Scene(push));
        if !self.core().has_craft() {
            self.push_craft();
        }
        Ok(())
    }

    /// Pushes the craft list, if the core has one to push.
    fn push_craft(&mut self) {
        let reading = self.current.clock.reading_at(Instant::now());
        let craft = self
            .source
            .state
            .scene
            .craft()
            .craft_at(self.source.id, reading.time());
        let knowledge = self.source.state.scene.knowledge();
        if let Some(list) = self.core_mut().craft(knowledge.as_ref(), craft) {
            // The clock read once, so that the craft are stated at the push's own time.
            let mut push = ScenePush::heartbeat(SceneClockDto::from(reading));
            push.craft = Some(list);
            self.pusher.push(PendingPush::Scene(push));
        }
    }

    /// Answers a command the connection routed here.
    async fn on_command(&mut self, command: SubscriptionCommand) -> Result<(), RequestError> {
        match command {
            SubscriptionCommand::SceneCameras { cameras, answer } => {
                let time = self.current.clock.reading_at(Instant::now()).time();
                // A refusal leaves the cameras as they were, so nothing needs undoing.
                let core = self.take_core();
                let (core, checked) = self
                    .source
                    .run(self.guard.token(), core, move |core, world| {
                        Ok(core.set_cameras(cameras.clone(), time, world))
                    })
                    .await?;
                self.core = Some(core);
                // The request may have been cancelled meanwhile; the answer then goes nowhere.
                let _ = answer.send(checked);
                Ok(())
            }
        }
    }

    /// Advances the core on the pool at the clock's present reading.
    /// Returns the reading it advanced to with what changed, so that the push states the time the
    /// changes were evaluated at.
    async fn delta(&mut self, beat: Beat) -> Result<(ClockReading, SceneDelta), RequestError> {
        let reading = self.current.clock.reading_at(Instant::now());
        let ship = self.current.ship.clone();
        let knowledge = self.source.state.scene.knowledge();
        let core = self.take_core();
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
        // Only the scene's system and the ship's frame's are kept: a stand-in moved from system
        // to system would otherwise hold every one it visited, outside the body cache's budget.
        let keep = [
            core.system_id(),
            frame_system(&self.current.ship.position_at(reading.time())),
        ];
        self.source.keep_only(&keep);
        self.core = Some(core);
        Ok((reading, delta))
    }

    /// The core, home between jobs.
    #[must_use]
    fn core(&self) -> &SceneCore {
        self.core
            .as_ref()
            .expect("the core is home between pool jobs")
    }

    /// The core, home between jobs, to change.
    #[must_use]
    fn core_mut(&mut self) -> &mut SceneCore {
        self.core
            .as_mut()
            .expect("the core is home between pool jobs")
    }

    /// The core, lent to a pool job.
    fn take_core(&mut self) -> SceneCore {
        self.core
            .take()
            .expect("the core is home between pool jobs")
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
    use hyperion_protocol::{
        DetailLevelDto, RequestBody, ResponseBody, SceneCraftDto, SubscribeRequest,
        SubscriptionState, SubscriptionTopic,
    };
    use hyperion_sim::id::BodyId;

    use crate::requests::{Handler, HandlerFuture, SubscribeFuture};
    use crate::scene::{CraftSource, CraftState, SceneKnowledge};
    use crate::testing::{Harness, NEVER, Scripted, WAIT};
    use crate::ws::ConnectionLimits;

    /// The stand-in 1 au from `system`'s barycentre at rest, the clock at `start` at `rate`.
    fn in_system(
        system: hyperion_sim::id::SystemId,
        start: UniverseTime,
        rate: u32,
    ) -> SceneSetting {
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
            clock: SceneClock::new(start, Instant::now(), TimeRate::new(rate).unwrap()).unwrap(),
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
        let setting = in_system(system, start, 100_000);
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
            core: Some(core),
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

    /// A universe of the tests' seed and the systems of its layer-C cell at the solar circle.
    async fn universe_and_cell(
        state: &Arc<AppState>,
    ) -> (
        Arc<crate::universe::Universe>,
        Vec<hyperion_sim::galaxy::placement::SystemRecord>,
    ) {
        let universe = state
            .registry
            .create("Scene".parse().unwrap(), Some(0x4d2))
            .await
            .unwrap();
        let galaxy = state.galaxies.get(universe.key()).await.unwrap();
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::new(Layer::C, [0, 812, 0]).unwrap(),
            &mut cell,
        );
        (universe, cell)
    }

    /// Requests answered by a script, subscriptions by the server's own topics: a scene behind a
    /// writer the test can stick.
    #[derive(Debug)]
    struct ScriptedRequests(Scripted);

    impl Handler for ScriptedRequests {
        fn handle(
            &self,
            state: Arc<AppState>,
            body: RequestBody,
            token: CancelToken,
        ) -> HandlerFuture {
            self.0.handle(state, body, token)
        }

        fn subscribe(
            &self,
            state: Arc<AppState>,
            request: SubscribeRequest,
            pusher: Pusher,
            token: CancelToken,
        ) -> SubscribeFuture {
            Handlers.subscribe(state, request, pusher, token)
        }
    }

    /// Every craft, and the bodies of odd slots, are contacts.
    #[derive(Debug)]
    struct Contacts;

    impl SceneKnowledge for Contacts {
        fn grant(&self, body: BodyId, asked: DetailLevel) -> DetailLevel {
            if (body.body_index() >> 8) % 2 == 1 {
                DetailLevel::Contact
            } else {
                asked
            }
        }

        fn is_contact(&self, _craft: &CraftState) -> bool {
            true
        }
    }

    /// Ten craft, each stating the scene time it was asked at.
    #[derive(Debug)]
    struct TenCraft;

    impl CraftSource for TenCraft {
        fn craft_at(&self, _universe: UniverseId, t: UniverseTime) -> Vec<CraftState> {
            (0..10_u8)
                .map(|k| {
                    CraftState::new(SceneCraftDto {
                        craft: format!("craft-{k}"),
                        hull: "test-hull".to_owned(),
                        state: KinematicsDto {
                            position: FramePositionDto::Galactic {
                                position: hyperion_protocol::GalacticPosition::default(),
                            },
                            velocity_m_s: [f64::from(k), 0.0, 0.0],
                            time: hyperion_protocol::UniverseTime {
                                seconds: t.seconds(),
                                nanos: t.subsec_nanos(),
                            },
                        },
                        attitude: [1.0, 0.0, 0.0, 0.0],
                        angular_velocity_rad_s: [0.0; 3],
                        planned_path: None,
                    })
                })
                .collect()
        }
    }

    /// With the writer stuck for a second, the craft pushed at 64 Hz and the heartbeat's contacts
    /// merge into one pending push, which the queue never holds; once the client reads, one
    /// notification carries the latest craft and the contacts (R03.T8.b).
    #[tokio::test]
    async fn a_stuck_writer_gets_one_notification_with_the_latest_craft_and_the_second_s_changes() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_configured(
            ScriptedRequests(handler),
            ConnectionLimits {
                outbound_bytes: 1 << 20,
                write_timeout: NEVER,
                close_timeout: WAIT,
            },
            |config| config.scene_knowledge(Contacts).craft_source(TenCraft),
        )
        .await;
        let state = Arc::clone(harness.state());
        let (universe, cell) = universe_and_cell(&state).await;
        let start = UniverseTime::new(3_600, 0).unwrap();
        state
            .scene
            .set(universe.id(), in_system(cell[0].id(), start, 1));

        let mut client = harness.connect_slow_reader().await;
        client.hello().await;
        let subscribe = RequestBody::Subscribe(SubscribeRequest {
            universe: universe.id().into(),
            topic: SubscriptionTopic::Scene(SceneSubscribeRequest {
                detail: DetailLevelDto::Bulk,
                cameras: Vec::new(),
            }),
        });
        client.request(1, subscribe).await;
        let clogging = harness.stick_writer(&mut calls, &mut client, 2).await;
        let stuck = harness
            .outbound_until(|counters| counters.queued_bytes() >= clogging)
            .await
            .queued_bytes();
        // The second the writer stays stuck: the scenario, not a wait for something to happen.
        tokio::time::sleep(SCENE_HEARTBEAT + CRAFT_PUSH_INTERVAL * 8).await;
        let still = harness.server().stats().outbound().queued_bytes();
        assert!(
            still <= stuck,
            "nothing queued while stuck: {still} > {stuck} bytes"
        );

        let mut last = 0;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            match client.next_message().await {
                ServerMessage::Response {
                    id: RequestId(2), ..
                } => break,
                ServerMessage::Response {
                    body: ResponseBody::Subscribe(subscribed),
                    ..
                } => {
                    let SubscriptionState::Scene(state) = subscribed.state;
                    let system = state.system.expect("the stand-in is in the system");
                    seen = system
                        .grants
                        .iter()
                        .filter(|grant| grant.seen.is_some())
                        .map(|grant| grant.body.clone())
                        .collect();
                }
                ServerMessage::Response { .. } => {}
                ServerMessage::Notification {
                    body: NotificationBody::Scene(notification),
                    ..
                } => last = notification.sequence,
                other => panic!("unexpected {other:?}"),
            }
        }
        let ServerMessage::Notification {
            body: NotificationBody::Scene(merged),
            ..
        } = client.next_message().await
        else {
            panic!("a notification after the clogging response");
        };
        assert_eq!(merged.sequence, last + 1, "numbered on from the last sent");
        // The latest craft: stated within a tick or two of the push's time, which a heartbeat
        // merged in after them may pass.
        let craft = merged.craft.expect("the craft ride with the merged push");
        assert_eq!(craft.len(), 10);
        let clock = UniverseTime::new(merged.clock.time.seconds, merged.clock.time.nanos).unwrap();
        let stated =
            UniverseTime::new(craft[0].state.time.seconds, craft[0].state.time.nanos).unwrap();
        let behind = clock.checked_since(stated).unwrap();
        assert!(
            !behind.is_negative() && behind < Span::from_seconds(1),
            "craft stated {behind} before the push"
        );
        // Every body the heartbeat moved in that second: each contact the ship sees.
        let moved: std::collections::BTreeSet<_> = merged
            .bodies
            .iter()
            .filter(|body| body.seen.is_some())
            .map(|body| body.record.id.clone())
            .collect();
        assert!(!seen.is_empty());
        assert!(
            seen.is_subset(&moved),
            "the heartbeat's contacts: {seen:?} in {moved:?}"
        );
        drop(client);
        harness.stop().await;
    }

    /// The task keeps only the systems it names, so that a stand-in moved from system to system
    /// does not hold each one it visited.
    #[tokio::test]
    async fn the_task_keeps_only_the_systems_it_names() {
        let harness = Harness::start(Handlers).await;
        let state = Arc::clone(harness.state());
        let universe = state
            .registry
            .create("Scene".parse().unwrap(), Some(0x4d2))
            .await
            .unwrap();
        let galaxy = state.galaxies.get(universe.key()).await.unwrap();
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::new(Layer::C, [0, 812, 0]).unwrap(),
            &mut cell,
        );
        let mut systems = BTreeMap::new();
        for record in cell.iter().take(3) {
            let generated = bodies_of(&state, universe.key(), &galaxy, record.id())
                .await
                .unwrap();
            systems.insert(record.id(), generated);
        }
        let ids: Vec<SystemId> = systems.keys().copied().collect();
        let mut source = SceneSource {
            state: Arc::clone(&state),
            universe: universe.id().into(),
            id: universe.id(),
            key: universe.key(),
            galaxy,
            systems: Arc::new(systems),
        };
        source.keep_only(&[Some(ids[2]), None]);
        assert_eq!(
            source.systems.keys().copied().collect::<Vec<_>>(),
            vec![ids[2]]
        );
        source.keep_only(&[None, None]);
        assert!(source.systems.is_empty());
        harness.stop().await;
    }
}
