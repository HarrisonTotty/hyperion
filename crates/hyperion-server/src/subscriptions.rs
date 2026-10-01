//! One connection's subscriptions: the table the connection task owns, each subscription's
//! pending push, and the merge that lets a slow reader receive the latest scene rather than a
//! backlog (rendering plan R03, R03.T5.b; plan 12's P12.T9 design).
//!
//! A `subscribe` request reserves a subscription, numbered from 1 per connection and never reused
//! on it, and runs the topic's opening as an ordinary request (its handler's
//! [`Handler::subscribe`](crate::requests::Handler::subscribe)). The topic keeps a [`Pusher`], into
//! which it merges each change as a [`PendingPush`]; the subscription holds one pending push at
//! most (Design note 5). Once the `subscribed` answer is queued, the subscription is live, and the
//! connection queues its pending push as a `notification` whenever the outbound queue has room for
//! it, numbering it with the subscription's next `sequence` only then (Design note 4). Without
//! room it leaves it pending, never holding it as it holds a finished request, so a slow reader
//! gets fewer, fuller notifications and the server holds one per subscription. `unsubscribe`
//! ends a subscription, and every subscription ends with its socket: the table is dropped with
//! the connection, which ends each topic's task and tells its [`Pusher`].

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use hyperion_protocol::{
    BodyIdHex, CameraReportDto, ErrorCode, KinematicsDto, NotificationBody, RequestError,
    RequestId, SceneArrivalDto, SceneBodyDto, SceneClockDto, SceneCraftDto, SceneNotificationDto,
    ServerMessage,
};
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tokio::task::AbortHandle;

use crate::compute::{CancelToken, CpuPool, Priority};
use crate::limits::{MAX_IN_FLIGHT_REQUESTS, MAX_SUBSCRIPTIONS};
use crate::requests::to_frame;
use crate::scene::is_large_notification;

/// A subscription's number on its connection, from 1, never reused on that connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SubscriptionId(u32);

impl SubscriptionId {
    /// The number on the wire.
    #[must_use]
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

/// Folding a later change into an earlier one, so that one pending push stands for all of them.
pub(crate) trait Merge {
    /// Folds `later` into `self`, the later winning wherever both say something.
    fn merge(&mut self, later: Self);
}

/// A subscription's changes not yet sent, by topic.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PendingPush {
    /// The scene's.
    Scene(ScenePush),
}

impl PendingPush {
    /// The push a notification's body was made from, its sequence dropped.
    #[must_use]
    fn from_body(body: NotificationBody) -> Self {
        match body {
            NotificationBody::Scene(notification) => Self::Scene(ScenePush {
                clock: notification.clock,
                ship: notification.ship,
                arrival: notification.arrival,
                bodies: notification.bodies,
                craft: notification.craft,
            }),
        }
    }

    /// The notification's body, numbered `sequence`.
    #[must_use]
    fn into_body(self, sequence: u64) -> NotificationBody {
        match self {
            Self::Scene(push) => NotificationBody::Scene(push.into_notification(sequence)),
        }
    }
}

impl Merge for PendingPush {
    fn merge(&mut self, later: Self) {
        match (self, later) {
            (Self::Scene(earlier), Self::Scene(later)) => earlier.merge(later),
        }
    }
}

/// The scene's changes not yet sent (Design note 5): the clock, the ship and the craft list are
/// the latest; bodies are merged by ID, the latest record winning; an arrival replaces everything
/// before it, bodies included.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScenePush {
    /// The scene clock at the latest change.
    pub(crate) clock: SceneClockDto,
    /// The ship, if it changed.
    pub(crate) ship: Option<KinematicsDto>,
    /// An arrival in a system or a departure from it.
    pub(crate) arrival: Option<SceneArrivalDto>,
    /// Bodies re-sent, one per ID, in the order first sent.
    pub(crate) bodies: Vec<SceneBodyDto>,
    /// The whole craft list, if it changed.
    pub(crate) craft: Option<Vec<SceneCraftDto>>,
}

impl ScenePush {
    /// A push of the clock alone: a heartbeat.
    #[must_use]
    pub(crate) fn heartbeat(clock: SceneClockDto) -> Self {
        Self {
            clock,
            ship: None,
            arrival: None,
            bodies: Vec::new(),
            craft: None,
        }
    }

    /// The notification, numbered `sequence`.
    #[must_use]
    pub(crate) fn into_notification(self, sequence: u64) -> SceneNotificationDto {
        SceneNotificationDto {
            sequence,
            clock: self.clock,
            ship: self.ship,
            arrival: self.arrival,
            bodies: self.bodies,
            craft: self.craft,
        }
    }
}

impl Merge for ScenePush {
    fn merge(&mut self, later: Self) {
        self.clock = later.clock;
        if later.ship.is_some() {
            self.ship = later.ship;
        }
        if later.craft.is_some() {
            self.craft = later.craft;
        }
        if later.arrival.is_some() {
            // The arrival carries the whole system: nothing before it still applies.
            self.arrival = later.arrival;
            self.bodies = later.bodies;
            return;
        }
        for body in later.bodies {
            let id: &BodyIdHex = &body.record.id;
            match self
                .bodies
                .iter_mut()
                .find(|earlier| &earlier.record.id == id)
            {
                Some(earlier) => *earlier = body,
                None => self.bodies.push(body),
            }
        }
    }
}

/// What a subscription and its topic share.
#[derive(Debug, Default)]
struct Shared {
    /// The changes not yet sent.
    pending: Mutex<Option<PendingPush>>,
    /// The topic's task, which ends with the subscription.
    task: Mutex<TaskSlot>,
    /// The commands the connection routes to the topic, until the topic takes them.
    commands: Mutex<Option<mpsc::Receiver<SubscriptionCommand>>>,
}

/// Commands a subscription's topic may queue: a client's requests in flight, at most
/// [`MAX_IN_FLIGHT_REQUESTS`], so that only a topic that has stopped answering fills it.
const COMMANDS: usize = MAX_IN_FLIGHT_REQUESTS;

/// A request the connection routes to a subscription's topic, with where the topic answers it
/// (rendering plan R03, R03.T8.a): requests that name a subscription cannot reach it through
/// [`Handler::handle`](crate::requests::Handler::handle).
#[derive(Debug)]
pub(crate) enum SubscriptionCommand {
    /// `scene_cameras`: replace the scene's cameras.
    SceneCameras {
        /// The views' cameras.
        cameras: Vec<CameraReportDto>,
        /// Where the topic answers: the empty answer, or why the cameras are refused.
        answer: oneshot::Sender<Result<(), RequestError>>,
    },
}

/// Where a topic's task stands against its subscription's end, under one lock, so that a task
/// attached as the subscription ends is aborted either way.
#[derive(Debug, Default)]
enum TaskSlot {
    /// No task attached yet.
    #[default]
    Empty,
    /// The topic's task, aborted when the subscription ends.
    Attached(AbortHandle),
    /// The subscription has ended: a task attached now is aborted at once.
    Ended,
}

impl Shared {
    fn take(&self) -> Option<PendingPush> {
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    fn merge(&self, change: PendingPush) {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        match pending.as_mut() {
            Some(earlier) => earlier.merge(change),
            None => *pending = Some(change),
        }
    }

    /// Puts back a push the connection took but could not queue, before anything merged since.
    fn restore(&self, mut taken: PendingPush) {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(since) = pending.take() {
            taken.merge(since);
        }
        *pending = Some(taken);
    }
}

/// A topic's end of a subscription: where it merges its changes, and how it learns that the
/// subscription has ended.
#[derive(Debug, Clone)]
pub(crate) struct Pusher {
    shared: Arc<Shared>,
    wake: Arc<Notify>,
    ended: watch::Receiver<()>,
}

impl Pusher {
    /// Merges `change` into the subscription's pending push and wakes the connection. Returns
    /// whether the subscription is still open; a push after it ended is dropped.
    pub(crate) fn push(&self, change: PendingPush) -> bool {
        if self.is_ended() {
            return false;
        }
        self.shared.merge(change);
        self.wake.notify_one();
        true
    }

    /// Hands the connection the topic's task, which it aborts when the subscription ends.
    /// A second task replaces the first, which is aborted.
    pub(crate) fn attach(&self, task: AbortHandle) {
        let mut slot = self
            .shared
            .task
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match std::mem::take(&mut *slot) {
            TaskSlot::Ended => {
                task.abort();
                *slot = TaskSlot::Ended;
            }
            TaskSlot::Attached(earlier) => {
                earlier.abort();
                *slot = TaskSlot::Attached(task);
            }
            TaskSlot::Empty => *slot = TaskSlot::Attached(task),
        }
    }

    /// The commands the connection routes to this subscription, for the topic to answer; `None`
    /// once taken. A topic that never takes them leaves each such request answered `internal`.
    pub(crate) fn take_commands(&self) -> Option<mpsc::Receiver<SubscriptionCommand>> {
        self.shared
            .commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Whether the subscription has ended: `unsubscribe`, a failed opening or the socket's close.
    #[must_use]
    pub(crate) fn is_ended(&self) -> bool {
        self.ended.has_changed().is_err()
    }

    /// Completes once the subscription has ended. Cancellation-safe.
    pub(crate) async fn ended(&self) {
        let mut ended = self.ended.clone();
        // Only the sender's drop wakes this: nothing is ever sent on it.
        while ended.changed().await.is_ok() {}
    }
}

/// Where a subscription is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Its `subscribe` request, this one, is in flight: nothing is sent on it yet.
    Opening(RequestId),
    /// Its `subscribed` answer is queued: its pushes are sent.
    Live,
}

/// One subscription, in its connection's table.
#[derive(Debug)]
struct Subscription {
    stage: Stage,
    /// The last sequence sent: the state's 0, then one more per notification.
    sequence: u64,
    shared: Arc<Shared>,
    /// Where the connection routes the requests that name this subscription.
    commands: mpsc::Sender<SubscriptionCommand>,
    /// Dropped with the subscription, which ends its [`Pusher`]s.
    _ending: watch::Sender<()>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let ended = std::mem::replace(
            &mut *self
                .shared
                .task
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
            TaskSlot::Ended,
        );
        if let TaskSlot::Attached(task) = ended {
            task.abort();
        }
    }
}

/// A notification taken from its subscription and not yet serialised.
#[derive(Debug)]
pub(crate) struct Unsent {
    id: SubscriptionId,
    body: NotificationBody,
}

impl Unsent {
    /// The subscription it is for.
    #[must_use]
    pub(crate) fn id(&self) -> SubscriptionId {
        self.id
    }

    /// Whether it is serialised on the CPU pool rather than the runtime (rendering plan R03,
    /// Design note 14): a scene notification carrying an arrival holds a whole system.
    #[must_use]
    pub(crate) fn is_large(&self) -> bool {
        match &self.body {
            NotificationBody::Scene(notification) => is_large_notification(notification),
        }
    }

    /// Serialises it here.
    #[must_use]
    pub(crate) fn serialise(self) -> Ready {
        let Self { id, body } = self;
        let (body, frame) = notification_frame(id, body);
        Ready {
            id,
            frame,
            body: Some(body),
        }
    }

    /// Serialises it on `pool`, as a large one is, or on a blocking thread if the pool refuses it
    /// (its interactive queue full, or the pool shutting down): the connection never waits for
    /// room in the pool, so that it goes on reading while the pool is busy, and never serialises
    /// a whole system on the runtime.
    pub(crate) async fn serialise_on(self, pool: &CpuPool) -> Ready {
        let Self { id, body } = self;
        // Lent to the job through a slot, so that a job the queue refuses gives the body back.
        let slot = Arc::new(Mutex::new(Some(body)));
        let lent = Arc::clone(&slot);
        let submitted = pool.try_submit(Priority::Interactive, CancelToken::new(), move |_| {
            let body = lent
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .expect("the body is lent to one job");
            notification_frame(id, body)
        });
        let Ok(receiver) = submitted else {
            let body = slot
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .expect("a job the queue refused never ran");
            return match tokio::task::spawn_blocking(move || Self { id, body }.serialise()).await {
                Ok(ready) => ready,
                Err(error) => {
                    tracing::warn!(%error, "a notification could not be serialised");
                    Ready::empty(id)
                }
            };
        };
        if let Ok(Ok((body, frame))) = receiver.await {
            Ready {
                id,
                frame,
                body: Some(body),
            }
        } else {
            // The job held the body and is gone with it: a panic in serialising, which `to_frame`
            // rules out, or the pool stopping with the server, after which nothing is sent.
            tracing::warn!("a notification could not be serialised on the pool");
            Ready::empty(id)
        }
    }
}

/// The frame of `body` as a notification on subscription `id`, and the body back.
#[must_use]
fn notification_frame(id: SubscriptionId, body: NotificationBody) -> (NotificationBody, String) {
    let message = ServerMessage::Notification {
        subscription: id.0,
        body,
    };
    let frame = to_frame(&message);
    let ServerMessage::Notification { body, .. } = message else {
        unreachable!("built as a notification just above");
    };
    (body, frame)
}

/// A notification ready to queue, and what to do with it once it is or is not.
#[derive(Debug)]
pub(crate) struct Ready {
    id: SubscriptionId,
    frame: String,
    /// The notification's body, from which the push is recovered if it has no room; `None` for
    /// one that could not be serialised, which is dropped.
    body: Option<NotificationBody>,
}

impl Ready {
    /// A notification that could not be serialised: nothing to send, nothing to restore.
    fn empty(id: SubscriptionId) -> Self {
        Self {
            id,
            frame: String::new(),
            body: None,
        }
    }

    /// Whether there is nothing to send.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.body.is_none()
    }

    /// The frame's payload bytes, for the outbound budget.
    #[must_use]
    pub(crate) fn frame_len(&self) -> usize {
        self.frame.len()
    }
}

/// One connection's subscriptions, owned by its connection task.
pub(crate) struct Subscriptions {
    /// The next number to give.
    next: u32,
    live: BTreeMap<SubscriptionId, Subscription>,
    /// Woken by every push, of any subscription.
    wake: Arc<Notify>,
}

impl fmt::Debug for Subscriptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Subscriptions")
            .field("next", &self.next)
            .field("subscriptions", &self.live.len())
            .finish_non_exhaustive()
    }
}

impl Default for Subscriptions {
    fn default() -> Self {
        Self::new()
    }
}

impl Subscriptions {
    /// An empty table.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            next: 1,
            live: BTreeMap::new(),
            wake: Arc::new(Notify::new()),
        }
    }

    /// Reserves a subscription for the `subscribe` request `request`, and returns its number and
    /// the topic's [`Pusher`].
    ///
    /// # Errors
    ///
    /// `bad_request` naming `topic` once the connection has [`MAX_SUBSCRIPTIONS`], opening ones
    /// included.
    pub(crate) fn reserve(
        &mut self,
        request: RequestId,
    ) -> Result<(SubscriptionId, Pusher), RequestError> {
        let full = || RequestError {
            code: ErrorCode::BadRequest,
            message: format!("a connection may have at most {MAX_SUBSCRIPTIONS} subscriptions"),
            field: Some("topic".to_owned()),
        };
        // A connection reaches 2³² subscriptions only by a client's abuse; it then numbers no
        // more and refuses as if full.
        let next = self.next.checked_add(1);
        let Some(next) = next.filter(|_| self.live.len() < MAX_SUBSCRIPTIONS) else {
            return Err(full());
        };
        let id = SubscriptionId(self.next);
        self.next = next;
        let (commands, inbox) = mpsc::channel(COMMANDS);
        let shared = Arc::new(Shared {
            commands: Mutex::new(Some(inbox)),
            ..Shared::default()
        });
        let (ending, ended) = watch::channel(());
        self.live.insert(
            id,
            Subscription {
                stage: Stage::Opening(request),
                sequence: 0,
                shared: Arc::clone(&shared),
                commands,
                _ending: ending,
            },
        );
        let pusher = Pusher {
            shared,
            wake: Arc::clone(&self.wake),
            ended,
        };
        Ok((id, pusher))
    }

    /// Makes live the subscription that `request` was opening, once its `subscribed` answer is
    /// queued: its pushes are sent from now on, after that answer.
    pub(crate) fn went_live(&mut self, request: RequestId) {
        if let Some(subscription) = self
            .live
            .values_mut()
            .find(|subscription| subscription.stage == Stage::Opening(request))
        {
            subscription.stage = Stage::Live;
            self.wake.notify_one();
        }
    }

    /// Ends the subscription that `request` was opening, which failed or was cancelled. Its
    /// number is not reused.
    pub(crate) fn failed(&mut self, request: RequestId) {
        self.live
            .retain(|_, subscription| subscription.stage != Stage::Opening(request));
    }

    /// Ends subscription `number`.
    ///
    /// # Errors
    ///
    /// `bad_request` naming `subscription` if the connection has no live subscription `number`.
    pub(crate) fn end(&mut self, number: u32) -> Result<(), RequestError> {
        let id = SubscriptionId(number);
        match self.live.get(&id) {
            Some(subscription) if subscription.stage == Stage::Live => {
                self.live.remove(&id);
                Ok(())
            }
            Some(_) | None => Err(unknown_subscription(number)),
        }
    }

    /// Routes `command` to live subscription `number`'s topic.
    ///
    /// # Errors
    ///
    /// `bad_request` naming `subscription` if the connection has no live subscription `number`,
    /// or its topic takes no commands; `queue_full` if its topic has [`COMMANDS`] waiting.
    pub(crate) fn command(
        &self,
        number: u32,
        command: SubscriptionCommand,
    ) -> Result<(), RequestError> {
        let subscription = self
            .live
            .get(&SubscriptionId(number))
            .filter(|subscription| subscription.stage == Stage::Live)
            .ok_or_else(|| unknown_subscription(number))?;
        subscription
            .commands
            .try_send(command)
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => RequestError {
                    code: ErrorCode::QueueFull,
                    message: format!("subscription {number} has too many requests waiting"),
                    field: None,
                },
                mpsc::error::TrySendError::Closed(_) => RequestError {
                    code: ErrorCode::BadRequest,
                    message: format!("subscription {number}'s topic takes no such request"),
                    field: Some("subscription".to_owned()),
                },
            })
    }

    /// Completes when a push has arrived since the last call. Cancellation-safe.
    pub(crate) async fn woken(&self) {
        self.wake.notified().await;
    }

    /// The next live subscription's pending push, as the notification it would be sent as,
    /// numbered with its next sequence but not yet serialised; `None` if nothing is pending. Every
    /// subscription is asked in turn, from the one after `after`.
    #[must_use]
    pub(crate) fn next_unsent(&self, after: Option<SubscriptionId>) -> Option<Unsent> {
        let start = after.map_or(0, |id| id.0.saturating_add(1));
        self.live
            .range(SubscriptionId(start)..)
            .filter(|(_, subscription)| subscription.stage == Stage::Live)
            .find_map(|(&id, subscription)| {
                let push = subscription.shared.take()?;
                let next = subscription.sequence.saturating_add(1);
                Some(Unsent {
                    id,
                    body: push.into_body(next),
                })
            })
    }

    /// Takes a ready notification's frame to queue, counting its sequence as sent.
    #[must_use]
    pub(crate) fn sent(&mut self, ready: Ready) -> String {
        if let Some(subscription) = self.live.get_mut(&ready.id) {
            subscription.sequence = subscription.sequence.saturating_add(1);
        }
        ready.frame
    }

    /// Puts back a ready notification that has no room, to be merged with what comes next.
    pub(crate) fn not_sent(&self, ready: Ready) {
        if let (Some(subscription), Some(body)) = (self.live.get(&ready.id), ready.body) {
            subscription.shared.restore(PendingPush::from_body(body));
        }
    }
}

/// The refusal of a request that names a subscription the connection does not have.
#[must_use]
fn unknown_subscription(number: u32) -> RequestError {
    RequestError {
        code: ErrorCode::BadRequest,
        message: format!("this connection has no subscription {number}"),
        field: Some("subscription".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{
        BodyKindDto, BodyStateDto, BodySummaryDto, ClientMessage, DetailLevelDto, FramePositionDto,
        GalacticPosition, RequestBody, ResponseBody, SceneClockStateDto, SceneStateDto,
        SceneSubscribeRequest, SectionDto, SubscribeRequest, SubscriptionState, SubscriptionTopic,
        UniverseIdHex, UniverseTime, UnsubscribeRequest,
    };

    use super::*;
    use tokio::time::timeout;

    use crate::testing::{Client, Harness, NEVER, Openings, Scripted, WAIT};
    use crate::ws::ConnectionLimits;

    /// The scene clock at `seconds` after the epoch, at 1×.
    fn clock(seconds: i64) -> SceneClockDto {
        SceneClockDto {
            time: UniverseTime { seconds, nanos: 0 },
            time_rate: 1,
            state: SceneClockStateDto::Running,
        }
    }

    /// A body record at the `contact` level, told apart by `index` and `x`.
    fn body(index: u16, x: f64) -> SceneBodyDto {
        SceneBodyDto {
            level: DetailLevelDto::Contact,
            record: BodySummaryDto {
                id: BodyIdHex::from_parts(0x0200_0800_2000_0000, index),
                kind: BodyKindDto::Unresolved,
                label: SectionDto::NotResolved,
                parent: None,
                state: BodyStateDto::Present,
                position_m: Some([x, 0.0, 0.0]),
                mass_kg: SectionDto::NotResolved,
                orbit: SectionDto::NotResolved,
                moons: SectionDto::NotResolved,
                rings: SectionDto::NotResolved,
                population: SectionDto::NotResolved,
                bulk: SectionDto::NotResolved,
            },
            seen: None,
        }
    }

    fn ship() -> KinematicsDto {
        KinematicsDto {
            position: FramePositionDto::Galactic {
                position: GalacticPosition::default(),
            },
            velocity_m_s: [0.0; 3],
            time: UniverseTime::default(),
        }
    }

    /// A push of the clock at `seconds` and the given bodies.
    fn push(seconds: i64, bodies: Vec<SceneBodyDto>) -> PendingPush {
        PendingPush::Scene(ScenePush {
            bodies,
            ..ScenePush::heartbeat(clock(seconds))
        })
    }

    fn scene_state() -> SubscriptionState {
        SubscriptionState::Scene(SceneStateDto {
            sequence: 0,
            clock: clock(0),
            ship: ship(),
            system: None,
            tidal_radius_m: None,
            craft: Vec::new(),
        })
    }

    fn subscribe() -> RequestBody {
        RequestBody::Subscribe(SubscribeRequest {
            universe: UniverseIdHex::from_u64(42),
            topic: SubscriptionTopic::Scene(SceneSubscribeRequest {
                detail: DetailLevelDto::Contact,
                cameras: Vec::new(),
            }),
        })
    }

    /// Subscribes as request `id`, opens the topic with the scene's state, and returns the
    /// subscription's number and the topic's pusher.
    async fn subscribed(client: &mut Client, openings: &mut Openings, id: u32) -> (u32, Pusher) {
        client.request(id, subscribe()).await;
        let opening = openings.next().await;
        assert!(matches!(
            opening.request.topic,
            SubscriptionTopic::Scene(SceneSubscribeRequest {
                detail: DetailLevelDto::Contact,
                ..
            })
        ));
        let pusher = opening.open(scene_state());
        match client.next_message().await {
            ServerMessage::Response {
                id: answered,
                body: ResponseBody::Subscribe(subscribed),
            } if answered == RequestId(id) => {
                assert_eq!(subscribed.state, scene_state());
                (subscribed.subscription, pusher)
            }
            other => panic!("expected `subscribed`, got {other:?}"),
        }
    }

    /// The next message, which must be a scene notification on `subscription`.
    async fn notification(client: &mut Client, subscription: u32) -> SceneNotificationDto {
        match client.next_message().await {
            ServerMessage::Notification {
                subscription: on,
                body: NotificationBody::Scene(notification),
            } if on == subscription => notification,
            other => panic!("expected a notification on {subscription}, got {other:?}"),
        }
    }

    #[test]
    fn bodies_are_merged_by_id_with_the_latest_winning() {
        let mut pending = push(1, vec![body(0x0100, 1.0), body(0x0200, 2.0)]);
        pending.merge(push(2, vec![body(0x0200, 20.0), body(0x0300, 3.0)]));
        let PendingPush::Scene(scene) = pending;
        assert_eq!(scene.clock, clock(2));
        assert_eq!(
            scene.bodies,
            vec![body(0x0100, 1.0), body(0x0200, 20.0), body(0x0300, 3.0)]
        );
    }

    #[test]
    fn an_arrival_clears_the_bodies_before_it() {
        let mut pending = PendingPush::Scene(ScenePush {
            ship: Some(ship()),
            bodies: vec![body(0x0100, 1.0)],
            ..ScenePush::heartbeat(clock(1))
        });
        pending.merge(PendingPush::Scene(ScenePush {
            arrival: Some(SceneArrivalDto::NoSystem),
            ..ScenePush::heartbeat(clock(2))
        }));
        pending.merge(push(3, vec![body(0x0200, 2.0)]));
        let PendingPush::Scene(scene) = pending;
        assert_eq!(scene.arrival, Some(SceneArrivalDto::NoSystem));
        assert_eq!(scene.bodies, vec![body(0x0200, 2.0)]);
        assert_eq!(scene.clock, clock(3));
        // The ship's change is not the arrival's to drop: no change is lost in a merge.
        assert_eq!(scene.ship, Some(ship()));
    }

    #[test]
    fn the_ship_and_the_craft_are_the_latest_and_kept_when_unchanged() {
        let mut pending = PendingPush::Scene(ScenePush {
            ship: Some(ship()),
            craft: Some(Vec::new()),
            ..ScenePush::heartbeat(clock(1))
        });
        pending.merge(push(2, Vec::new()));
        let PendingPush::Scene(scene) = pending;
        assert_eq!(scene.ship, Some(ship()));
        assert_eq!(scene.craft, Some(Vec::new()));
    }

    #[tokio::test]
    async fn sequences_rise_by_one_per_notification_sent() {
        let (handler, _calls, mut openings) = Scripted::with_openings();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        let (subscription, pusher) = subscribed(&mut client, &mut openings, 1).await;
        assert_eq!(subscription, 1, "numbered from 1");
        for expected in 1..=3 {
            assert!(pusher.push(push(expected, vec![body(0x0100, 1.0)])));
            let notification = notification(&mut client, subscription).await;
            assert_eq!(notification.sequence, u64::try_from(expected).unwrap());
            assert_eq!(notification.clock, clock(expected));
        }
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn ten_pushes_into_a_stuck_writer_become_one_notification_holding_all_ten() {
        let (handler, mut calls, mut openings) = Scripted::with_openings();
        // A budget below the clogging response, so that once it is queued nothing else fits.
        let harness = Harness::start_with_limits(
            handler,
            ConnectionLimits {
                outbound_bytes: 1 << 20,
                write_timeout: NEVER,
                close_timeout: WAIT,
            },
        )
        .await;
        let mut client = harness.connect_slow_reader().await;
        client.hello().await;
        let (subscription, pusher) = subscribed(&mut client, &mut openings, 1).await;
        let clogging = harness.stick_writer(&mut calls, &mut client, 2).await;
        harness
            .outbound_until(|counters| counters.queued_bytes() >= clogging)
            .await;
        for seconds in 1..=10 {
            let index = u16::try_from(seconds).unwrap() << 8;
            assert!(pusher.push(push(seconds, vec![body(index, 1.0)])));
        }
        // The clogging response, then one notification with every change merged.
        match client.next_message().await {
            ServerMessage::Response { id, .. } => assert_eq!(id, RequestId(2)),
            other => panic!("expected the clogging response, got {other:?}"),
        }
        let merged = notification(&mut client, subscription).await;
        assert_eq!(merged.sequence, 1);
        assert_eq!(merged.clock, clock(10));
        assert_eq!(merged.bodies.len(), 10);
        assert!(client.ping(1).await.is_empty(), "nothing more was queued");
        // The next push is numbered on from the merged one.
        assert!(pusher.push(push(11, Vec::new())));
        assert_eq!(notification(&mut client, subscription).await.sequence, 2);
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_push_made_while_opening_follows_the_subscribed_answer() {
        let (handler, _calls, mut openings) = Scripted::with_openings();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, subscribe()).await;
        let opening = openings.next().await;
        // The topic pushes before its opening is answered.
        assert!(opening.pusher.push(push(1, vec![body(0x0100, 1.0)])));
        let pusher = opening.open(scene_state());
        assert!(matches!(
            client.next_message().await,
            ServerMessage::Response {
                body: ResponseBody::Subscribe(_),
                ..
            }
        ));
        let first = notification(&mut client, 1).await;
        assert_eq!((first.sequence, first.clock), (1, clock(1)));
        assert!(!pusher.is_ended());
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_held_subscribed_answer_still_precedes_its_notifications() {
        let (handler, mut calls, mut openings) = Scripted::with_openings();
        let harness = Harness::start_with_limits(
            handler,
            ConnectionLimits {
                outbound_bytes: 1 << 20,
                write_timeout: NEVER,
                close_timeout: WAIT,
            },
        )
        .await;
        let mut client = harness.connect_slow_reader().await;
        client.hello().await;
        let clogging = harness.stick_writer(&mut calls, &mut client, 1).await;
        harness
            .outbound_until(|counters| counters.queued_bytes() >= clogging)
            .await;
        client.request(2, subscribe()).await;
        let pusher = openings.next().await.open(scene_state());
        // The answer is held for want of room; the push waits behind it.
        harness
            .outbound_until(|counters| counters.held_requests() == 1)
            .await;
        assert!(pusher.push(push(1, Vec::new())));
        assert!(matches!(
            client.next_message().await,
            ServerMessage::Response {
                id: RequestId(1),
                ..
            }
        ));
        assert!(matches!(
            client.next_message().await,
            ServerMessage::Response {
                id: RequestId(2),
                body: ResponseBody::Subscribe(_),
            }
        ));
        assert_eq!(notification(&mut client, 1).await.sequence, 1);
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn the_fifth_subscription_is_refused() {
        let (handler, _calls, mut openings) = Scripted::with_openings();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        let mut pushers = Vec::new();
        for id in 1..=4 {
            let (subscription, pusher) = subscribed(&mut client, &mut openings, id).await;
            assert_eq!(subscription, id);
            pushers.push(pusher);
        }
        client.request(5, subscribe()).await;
        match client.next_message().await {
            ServerMessage::RequestError { id, error } => {
                assert_eq!(id, RequestId(5));
                assert_eq!(error.code, ErrorCode::BadRequest);
                assert_eq!(error.field.as_deref(), Some("topic"));
            }
            other => panic!("expected the refusal, got {other:?}"),
        }
        // Once one ends, another opens, numbered on: numbers are never reused.
        client
            .request(
                6,
                RequestBody::Unsubscribe(UnsubscribeRequest { subscription: 2 }),
            )
            .await;
        assert!(matches!(
            client.next_message().await,
            ServerMessage::Response {
                body: ResponseBody::Unsubscribe,
                ..
            }
        ));
        assert!(pushers[1].is_ended());
        let (subscription, _) = subscribed(&mut client, &mut openings, 7).await;
        assert_eq!(subscription, 5);
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn an_unknown_subscription_is_a_bad_request_naming_it() {
        let (handler, _calls, _openings) = Scripted::with_openings();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client
            .request(
                1,
                RequestBody::Unsubscribe(UnsubscribeRequest { subscription: 9 }),
            )
            .await;
        match client.next_message().await {
            ServerMessage::RequestError { id, error } => {
                assert_eq!(id, RequestId(1));
                assert_eq!(error.code, ErrorCode::BadRequest);
                assert_eq!(error.field.as_deref(), Some("subscription"));
            }
            other => panic!("expected the refusal, got {other:?}"),
        }
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_push_after_unsubscribe_is_never_sent() {
        let (handler, _calls, mut openings) = Scripted::with_openings();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        let (subscription, pusher) = subscribed(&mut client, &mut openings, 1).await;
        client
            .request(
                2,
                RequestBody::Unsubscribe(UnsubscribeRequest { subscription }),
            )
            .await;
        assert!(matches!(
            client.next_message().await,
            ServerMessage::Response {
                body: ResponseBody::Unsubscribe,
                ..
            }
        ));
        assert!(!pusher.push(push(1, Vec::new())), "the push is dropped");
        assert!(client.ping(1).await.is_empty());
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn nothing_pending_survives_the_socket() {
        let (handler, _calls, mut openings) = Scripted::with_openings();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        let (_, pusher) = subscribed(&mut client, &mut openings, 1).await;
        let topic = tokio::spawn(std::future::pending::<()>());
        pusher.attach(topic.abort_handle());
        client.close().await;
        tokio::time::timeout(WAIT, pusher.ended())
            .await
            .expect("the subscription ends with its socket");
        assert!(!pusher.push(push(1, Vec::new())));
        let ended = tokio::time::timeout(WAIT, topic)
            .await
            .expect("the topic's task is ended");
        assert!(ended.unwrap_err().is_cancelled());
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_failed_or_cancelled_opening_leaves_no_subscription() {
        let (handler, _calls, mut openings) = Scripted::with_openings();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, subscribe()).await;
        let failed = openings.next().await.fail(RequestError {
            code: ErrorCode::UnknownUniverse,
            message: "no such universe".to_owned(),
            field: None,
        });
        assert!(matches!(
            client.next_message().await,
            ServerMessage::RequestError { error, .. } if error.code == ErrorCode::UnknownUniverse
        ));
        tokio::time::timeout(WAIT, failed.ended())
            .await
            .expect("a failed opening ends its subscription");
        client.request(2, subscribe()).await;
        let opening = openings.next().await;
        client.cancel(2).await;
        assert!(matches!(
            client.next_message().await,
            ServerMessage::RequestError { error, .. } if error.code == ErrorCode::Cancelled
        ));
        let cancelled = opening.open(scene_state());
        tokio::time::timeout(WAIT, cancelled.ended())
            .await
            .expect("a cancelled opening ends its subscription");
        // The numbers the two took are not reused.
        let (subscription, _) = subscribed(&mut client, &mut openings, 3).await;
        assert_eq!(subscription, 3);
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn the_server_s_scene_topic_refuses_a_universe_it_does_not_hold() {
        let harness = Harness::start(crate::requests::Handlers).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client
            .send(&ClientMessage::Request {
                id: RequestId(1),
                body: subscribe(),
            })
            .await;
        match client.next_message().await {
            ServerMessage::RequestError { error, .. } => {
                assert_eq!(error.code, ErrorCode::UnknownUniverse);
            }
            other => panic!("expected `unknown_universe`, got {other:?}"),
        }
        client.close().await;
        harness.stop().await;
    }

    fn cameras_command() -> (
        SubscriptionCommand,
        oneshot::Receiver<Result<(), RequestError>>,
    ) {
        let (answer, answered) = oneshot::channel();
        let command = SubscriptionCommand::SceneCameras {
            cameras: Vec::new(),
            answer,
        };
        (command, answered)
    }

    fn refusal(result: Result<(), RequestError>) -> (ErrorCode, Option<String>) {
        let error = result.unwrap_err();
        (error.code, error.field)
    }

    #[test]
    fn a_command_is_refused_unless_its_subscription_is_live_and_its_topic_takes_commands() {
        let mut subscriptions = Subscriptions::new();
        let (id, pusher) = subscriptions.reserve(RequestId(1)).unwrap();
        let named = (ErrorCode::BadRequest, Some("subscription".to_owned()));
        // Still opening: no request may name it yet.
        assert_eq!(
            refusal(subscriptions.command(id.get(), cameras_command().0)),
            named
        );
        subscriptions.went_live(RequestId(1));
        // A topic that has stopped taking them fills its channel, then is refused for want of room.
        for _ in 0..COMMANDS {
            subscriptions
                .command(id.get(), cameras_command().0)
                .unwrap();
        }
        assert_eq!(
            refusal(subscriptions.command(id.get(), cameras_command().0)),
            (ErrorCode::QueueFull, None)
        );
        // A topic that dropped its end takes none.
        drop(
            pusher
                .take_commands()
                .expect("the commands are the topic's to take, once"),
        );
        assert!(pusher.take_commands().is_none());
        assert_eq!(
            refusal(subscriptions.command(id.get(), cameras_command().0)),
            named
        );
    }

    #[tokio::test]
    async fn a_command_still_queued_when_its_subscription_ends_is_never_answered() {
        let mut subscriptions = Subscriptions::new();
        let (id, pusher) = subscriptions.reserve(RequestId(1)).unwrap();
        subscriptions.went_live(RequestId(1));
        let (command, answered) = cameras_command();
        subscriptions.command(id.get(), command).unwrap();
        subscriptions.end(id.get()).unwrap();
        drop(pusher);
        // The request awaiting it is then answered `internal` (`Requests::scene_cameras`).
        assert!(timeout(WAIT, answered).await.unwrap().is_err());
    }
}
