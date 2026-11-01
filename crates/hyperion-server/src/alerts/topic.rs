//! The `alerts` topic (plan 12, P12.T9.a, P12.T8.b): an alerts subscription's opening, and the task
//! that runs its scans and answers the requests naming it.
//!
//! A subscription holds P12.T8.a's [`Subscription`] for its observer and sensor, and its
//! universe's contacts. Its state is the census and every contact the universe holds. Its task runs
//! the scans its schedule asks for on the [`AlertScans`](super::AlertScans) and merges each in the
//! order asked, empty if it failed; a class the census sends to the background is merged late,
//! once its walk ends, and is `pending` until then (Design note 7). After each merge and each move
//! of the observer, the pushes that fell due are recorded in the universe's Knowledge (P12.T8.c):
//! a detect or a resolve as a sighting at its own time and its own light ([`Sighting::new`]), a
//! detection's light at the limit's crossing from its host's line as the walk read it, and a clear
//! as its contact's clear. The census is pushed to the subscriber when it changes. The contacts
//! changed are pushed to every subscription of the universe, whichever's push or acknowledgement
//! changed them: each reads the universe's `ChangedContacts` and pushes the contacts changed since
//! it last looked, as a scene reads the surveys' revisions.

use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};

use futures_util::future::BoxFuture;
use futures_util::stream::FuturesOrdered;
use futures_util::{FutureExt, StreamExt};
use hyperion_protocol::{
    AlertsSubscribeRequest, AlertsSubscribed, ClassCensusDto, ClassCensusStatusDto, ContactDto,
    ErrorCode, RequestError, SubscriptionState, UniverseIdHex,
};
use hyperion_sim::alerts::LocalEventsCache;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::ClassId;
use hyperion_sim::observe::{Drift, Observer, curvature_error, extrapolate_to_present, retarded};
use hyperion_sim::time::UniverseTime;
use tokio::sync::{mpsc, watch};

use super::scan::{LOCAL_EVENTS_STARS, ScanSpec};
use super::service::Walked;
use super::{Advance, MergeScanError, Push, Rescan, ScanRequest, Subscription};
use crate::AppState;
use crate::compute::{CancelToken, GalaxyKey, panic_message};
use crate::convert::{
    AlertsAsked, acknowledge_refusal, census_dto, contact_dto, open_contacts_refusal,
    pending_census,
};
use crate::knowledge::{ChangedContacts, ContactId, KnowledgeLevel, PersistedKnowledge, Sighting};
use crate::requests::{openable_universe, request_error};
use crate::subscriptions::{
    AlertsPush, PendingPush, Pusher, SubscriptionCommand, not_of_this_topic,
};

/// Opens an alerts subscription: checks the request, opens the universe's contacts and its
/// galaxy, starts the first scan, and starts the task that runs the scans and answers the requests
/// naming the subscription.
///
/// # Errors
///
/// As [`openable_universe`]; `bad_request` naming the field at fault for a request no sensor or
/// observer has ([`AlertsAsked`]); [`open_contacts_refusal`]'s for contacts that cannot be read;
/// and those of [`GalaxyCache::get`](crate::compute::GalaxyCache::get) if the galaxy cannot be
/// built.
pub(crate) async fn open(
    state: Arc<AppState>,
    universe: UniverseIdHex,
    request: AlertsSubscribeRequest,
    pusher: Pusher,
    _token: CancelToken,
) -> Result<SubscriptionState, RequestError> {
    let opened = openable_universe(&state, &universe)?;
    let asked = AlertsAsked::try_from(&request)?;
    let contacts = state
        .contacts
        .of(state.registry.store(), opened.id())
        .await
        .map_err(|error| open_contacts_refusal(&error))?;
    let key = opened.key();
    let galaxy = state.galaxies.get(key).await?;
    // Looked at before the contacts are read, so that a change between is pushed again rather
    // than missed.
    let changes = contacts.changes();
    let told = changes.borrow().clone();
    let known = known_contacts(&contacts);
    let (subscription, first) = Subscription::new(
        asked.observer,
        asked.sensor,
        asked.classes,
        asked.host_limit,
    );
    let mut topic = Topic {
        state,
        galaxy: Arc::clone(&galaxy),
        key,
        census: BTreeMap::new(),
        subscription,
        local: Arc::new(Mutex::new(LocalEventsCache::with_capacity(
            LOCAL_EVENTS_STARS,
        ))),
        scans: FuturesOrdered::new(),
        backgrounds: FuturesOrdered::new(),
        launched: 0,
        background: BTreeMap::new(),
        commands: pusher.take_commands(),
        changes,
        told,
        crew: Crew {
            galaxy,
            contacts,
            pusher: pusher.clone(),
        },
    };
    topic.census = topic.opening_census();
    let census = topic.census_dtos();
    topic.launch(&first);
    spawn(topic, &pusher);
    Ok(SubscriptionState::Alerts(AlertsSubscribed {
        census,
        contacts: known,
    }))
}

/// Every contact `contacts` holds, as the wire carries it, in contact order.
#[must_use]
fn known_contacts(contacts: &PersistedKnowledge) -> Vec<ContactDto> {
    contacts.read(|store| {
        store
            .contacts()
            .filter_map(|contact| store.view(contact))
            .map(|view| contact_dto(&view))
            .collect()
    })
}

/// A scan's interactive walk, ended: its spec, its number among the subscription's scans, and
/// what it found.
struct Scanned {
    spec: ScanSpec,
    number: u64,
    walked: Walked,
}

/// A class's background walk for a scan, ended.
struct BackgroundWalked {
    spec: ScanSpec,
    number: u64,
    class: ClassId,
    walked: Walked,
}

/// What woke a subscription's task.
enum Wake {
    Command(SubscriptionCommand),
    Scanned(Scanned),
    BackgroundWalked(BackgroundWalked),
    ContactsChanged,
}

/// One alerts subscription's task: its schedule, its scans in flight, the census, and what it
/// tells its crew.
struct Topic {
    state: Arc<AppState>,
    galaxy: Arc<Galaxy>,
    key: GalaxyKey,
    subscription: Subscription,
    /// The local events cache of this subscription's walks (P12.T5.e's record).
    local: Arc<Mutex<LocalEventsCache>>,
    /// Each class's census as the wire carries it, by class.
    census: BTreeMap<ClassId, ClassCensusDto>,
    /// The interactive walks in flight, in the order their scans were asked for.
    scans: FuturesOrdered<BoxFuture<'static, Scanned>>,
    /// The background walks in flight, in the order they were started.
    backgrounds: FuturesOrdered<BoxFuture<'static, BackgroundWalked>>,
    /// The scans started so far.
    launched: u64,
    /// For each class walked in the background, the scan whose walk of it is the latest: the
    /// census is complete when that one ends.
    background: BTreeMap<ClassId, u64>,
    commands: Option<mpsc::Receiver<SubscriptionCommand>>,
    /// The universe's contacts changed, by any of its subscriptions.
    changes: watch::Receiver<ChangedContacts>,
    /// The changes this subscription has pushed.
    told: ChangedContacts,
    crew: Crew,
}

/// What a subscription tells its crew: the universe's contacts, recorded in its Knowledge and
/// pushed to the subscriber. Apart from the scans in flight, which are not `Sync`, so that a
/// borrow of it may be held over an await.
struct Crew {
    galaxy: Arc<Galaxy>,
    contacts: PersistedKnowledge,
    pusher: Pusher,
}

impl Topic {
    /// Runs the scans and answers the requests that name the subscription until it ends.
    async fn run(mut self) {
        loop {
            let commands = self.commands.as_mut();
            let wake = tokio::select! {
                biased;
                () = self.crew.pusher.ended() => return,
                Some(command) = async { match commands {
                    Some(commands) => commands.recv().await,
                    None => std::future::pending().await,
                } } => Wake::Command(command),
                Some(scanned) = self.scans.next(), if !self.scans.is_empty() => {
                    Wake::Scanned(scanned)
                }
                Some(walked) = self.backgrounds.next(), if !self.backgrounds.is_empty() => {
                    Wake::BackgroundWalked(walked)
                }
                // The universe's Knowledge outlives every topic that holds it, so its sender
                // never closes.
                Ok(()) = self.changes.changed() => Wake::ContactsChanged,
            };
            match wake {
                Wake::Command(command) => self.on_command(command).await,
                Wake::Scanned(scanned) => self.on_scanned(scanned).await,
                Wake::BackgroundWalked(walked) => self.on_background(walked).await,
                Wake::ContactsChanged => self.push_changed(),
            }
        }
    }

    /// Answers a request the connection routed here.
    async fn on_command(&mut self, command: SubscriptionCommand) {
        match command {
            SubscriptionCommand::AlertsObserver { observer, answer } => {
                let advance = self.subscription.advance(observer);
                self.after(advance, false).await;
                // A request cancelled meanwhile has nowhere to be answered, and needs nothing.
                let _ = answer.send(Ok(()));
            }
            SubscriptionCommand::AlertsAcknowledge { contact, answer } => {
                let acknowledged = self.crew.acknowledge(contact).await;
                self.push_changed();
                // As above: a cancelled request needs no answer.
                let _ = answer.send(acknowledged);
            }
            other @ SubscriptionCommand::SceneCameras { .. } => {
                let refusal = not_of_this_topic(other.kind(), "alerts");
                other.refuse(refusal);
            }
        }
    }

    /// Merges a scan's interactive walk, empty if it failed, starts the background walks its
    /// census asks for, and makes what fell due.
    async fn on_scanned(&mut self, scanned: Scanned) {
        let Scanned {
            spec,
            number,
            walked,
        } = scanned;
        let request = spec.request();
        let merged = match &walked {
            Ok(walked) => self
                .subscription
                .merge(request, walked.tracks().iter().cloned()),
            Err(error) => {
                tracing::warn!(%error, "an alerts scan failed: its window yields nothing");
                self.subscription.merge(request, [])
            }
        };
        if !merged_in_order(merged) {
            return;
        }
        let before = self.census_dtos();
        // The latest scan's census decides each class's: an earlier scan's background walk still
        // merges its own window's tracks when it ends, but no longer settles its class.
        self.background.clear();
        match &walked {
            Ok(walked) => {
                for census in walked.census() {
                    if let Some(dto) = census_dto(census) {
                        self.census.insert(census.class(), dto);
                    }
                }
                for class in walked.background() {
                    self.launch_background(&spec, number, class);
                }
            }
            Err(_) => {
                for dto in self.census.values_mut() {
                    dto.status = ClassCensusStatusDto::Pending;
                }
            }
        }
        let census_changed = self.census_dtos() != before;
        let advance = self.subscription.advance(*self.subscription.observer());
        self.after(advance, census_changed).await;
    }

    /// Merges a class's background walk late, and makes what fell due: the class's census is
    /// complete if the walk is the latest scan's, and stays `pending` if the walk failed.
    async fn on_background(&mut self, walked: BackgroundWalked) {
        let BackgroundWalked {
            spec,
            number,
            class,
            walked,
        } = walked;
        let found = match walked {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!(%error, ?class, "an alerts background walk failed: it yields nothing");
                return;
            }
        };
        let merged = self
            .subscription
            .merge_late(spec.request(), found.tracks().iter().cloned());
        if !merged_in_order(merged) {
            return;
        }
        let mut census_changed = false;
        if self.background.get(&class) == Some(&number) {
            self.background.remove(&class);
            if let Some(dto) = self.census.get_mut(&class) {
                // A class whose cells are too many to walk yields nothing, as one over the host
                // limit does (lean, 2026-11-01).
                let settled = if found.is_oversized() {
                    ClassCensusStatusDto::OverLimit
                } else {
                    ClassCensusStatusDto::Complete
                };
                census_changed = dto.status != settled;
                dto.status = settled;
            }
        }
        let advance = self.subscription.advance(*self.subscription.observer());
        self.after(advance, census_changed).await;
    }

    /// Runs the scan `advance` asks for and makes its pushes: a restart drops the scans in
    /// flight, whose results the schedule would refuse, and the census starts again.
    async fn after(&mut self, advance: Advance, mut census_changed: bool) {
        let (due, rescan) = advance.into_parts();
        match rescan {
            Some(Rescan::Renew(request)) => self.launch(&request),
            Some(Rescan::Restart(request)) => {
                // Dropping a walk's last waiter cancels it (`AlertScans`).
                self.scans = FuturesOrdered::new();
                self.backgrounds = FuturesOrdered::new();
                self.background.clear();
                let opening = self.opening_census();
                census_changed |= opening != self.census;
                self.census = opening;
                self.launch(&request);
            }
            None => {}
        }
        let position = *self.subscription.schedule().position();
        self.crew.record(&position, due).await;
        if census_changed {
            self.crew.push(&[], self.census_dtos());
        }
        // This subscription's own changes go out with it, before its request is answered.
        self.push_changed();
    }

    /// Pushes the contacts the universe changed since this subscription last looked, each as it
    /// now stands.
    fn push_changed(&mut self) {
        let now = self.changes.borrow_and_update().clone();
        let changed: Vec<ContactId> = now.since(&self.told).collect();
        self.told = now;
        self.crew.push(&changed, Vec::new());
    }

    /// Starts the interactive walk of `request`, merged when it ends.
    fn launch(&mut self, request: &ScanRequest) {
        let spec = ScanSpec::new(self.key, &self.subscription, request)
            .expect("a scan its schedule asked for is at an observer's place, on the clock");
        self.launched += 1;
        let number = self.launched;
        let walk =
            self.state
                .alert_scans
                .interactive(&self.state, &self.galaxy, &spec, &self.local);
        self.scans.push_back(
            async move {
                let walked = walk.await;
                Scanned {
                    spec,
                    number,
                    walked,
                }
            }
            .boxed(),
        );
    }

    /// Starts the background walk of `class` for scan `number` of `spec`, merged late when it ends.
    fn launch_background(&mut self, spec: &ScanSpec, number: u64, class: ClassId) {
        self.background.insert(class, number);
        let walk = self
            .state
            .alert_scans
            .background(&self.state, &self.galaxy, spec, class);
        let spec = spec.clone();
        self.backgrounds.push_back(
            async move {
                let walked = walk.await;
                BackgroundWalked {
                    spec,
                    number,
                    class,
                    walked,
                }
            }
            .boxed(),
        );
    }

    /// Each class's census before a scan has run: `pending`, at the sensor's reach.
    #[must_use]
    fn opening_census(&self) -> BTreeMap<ClassId, ClassCensusDto> {
        self.subscription
            .classes()
            .iter()
            .filter_map(|class| Some((class, pending_census(class, self.subscription.sensor())?)))
            .collect()
    }

    /// The census as the wire carries it, in class order.
    #[must_use]
    fn census_dtos(&self) -> Vec<ClassCensusDto> {
        self.census.values().copied().collect()
    }
}

impl Crew {
    /// Records each push of `due` in order, at `position`: a detect or a resolve as a sighting at
    /// its level ([`Crossing::level`](super::Crossing::level)), and a clear, which is no sighting,
    /// as its contact's clear (Design note 11). The universe's `ChangedContacts` names the
    /// contacts they changed.
    async fn record(&self, position: &GalacticPosition, due: Vec<Push>) {
        for push in due {
            let event = push.detection().event();
            let recorded = match push.crossing().level() {
                Some(level) => {
                    let sighting = self.sighting(position, &push, level);
                    self.contacts
                        .record_sighting(event, sighting)
                        .await
                        .map(|_| ())
                }
                None => self
                    .contacts
                    .record_clear(event, push.at())
                    .await
                    .map(|_| ()),
            };
            if let Err(error) = recorded {
                tracing::error!(%error, crossing = ?push.crossing(), "an alert could not be recorded");
            }
        }
    }

    /// The sighting `push` makes at `level`: the observer at `position`, the schedule's place, at
    /// the push's time, and its host's light then, from the line the walk read it on (its onset's
    /// light, carried along the host's velocity then), with its stated error.
    #[must_use]
    fn sighting(
        &self,
        position: &GalacticPosition,
        push: &Push,
        level: KnowledgeLevel,
    ) -> Sighting {
        let observer = Observer::new(*position, push.at())
            .expect("a schedule's place is an observer's, and its pushes fall on the clock");
        let onset = push.detection().retardation();
        let line = Drift::new(
            extrapolate_to_present(onset, UniverseTime::EPOCH),
            onset.velocity_then(),
        );
        let light = retarded(&observer, &line);
        let error = curvature_error(&self.galaxy, observer.position(), &light);
        Sighting::new(
            &observer,
            &light,
            push.detection(),
            push.flux(),
            level,
            error.length(),
        )
    }

    /// Pushes the views of the contacts `changed`, in contact order, and `census` unless it is
    /// empty, unless both are.
    fn push(&self, changed: &[ContactId], census: Vec<ClassCensusDto>) {
        if changed.is_empty() && census.is_empty() {
            return;
        }
        let contacts = self.contacts.read(|store| {
            changed
                .iter()
                .filter_map(|&contact| store.view(contact))
                .map(|view| contact_dto(&view))
                .collect()
        });
        self.pusher
            .push(PendingPush::Alerts(AlertsPush { contacts, census }));
    }

    /// Acknowledges `contact`: if that changed it, the universe's `ChangedContacts` names it, and
    /// every subscription of the universe pushes it.
    async fn acknowledge(&self, contact: ContactId) -> Result<(), RequestError> {
        self.contacts
            .acknowledge(contact)
            .await
            .map(drop)
            .map_err(|error| acknowledge_refusal(&error))
    }
}

/// Whether a merge took its scan: a stale one is refused by a schedule that restarted since, and
/// one out of order would be a broken invariant of the topic's, which merges in the order asked.
#[must_use]
fn merged_in_order(merged: Result<(), MergeScanError>) -> bool {
    match merged {
        Ok(()) => true,
        Err(MergeScanError::Stale) => false,
        Err(error @ MergeScanError::OutOfOrder { .. }) => {
            tracing::error!(%error, "an alerts scan was merged out of order and dropped");
            false
        }
    }
}

/// Starts `topic`'s task and hands it to `pusher`'s subscription, which owns it and aborts it
/// when the subscription ends. A task that panics ends the subscription `internal`, as the scene's
/// does, so that the client is told and the subscription's place is freed.
fn spawn(topic: Topic, pusher: &Pusher) {
    let failing = pusher.clone();
    // Unwind-safe as asserted: a topic that panicked is dropped, and nothing of it is used again.
    let run = AssertUnwindSafe(topic.run()).catch_unwind();
    let task = tokio::spawn(async move {
        if let Err(payload) = run.await {
            tracing::error!(
                panic = %panic_message(payload.as_ref()),
                "an alerts subscription's task panicked"
            );
            failing.fail(request_error(
                ErrorCode::Internal,
                "the alerts subscription failed",
            ));
        }
    });
    pusher.attach(task);
}
