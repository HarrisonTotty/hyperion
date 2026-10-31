//! The `alerts` topic (plan 12, P12.T9.a): an alerts subscription's opening, and the task that
//! answers the requests naming it.
//!
//! A subscription holds P12.T8.a's [`Subscription`] for its observer and sensor, and its
//! universe's contacts. Its state is the census and every contact the universe holds. Until
//! P12.T8.b's scans run, nothing is merged into its schedule: the census is empty, an advance
//! pushes nothing, and the scans the schedule asks for are not run. What it does push is a
//! contact the subscriber acknowledges.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use futures_util::FutureExt;
use hyperion_protocol::{
    AlertsSubscribeRequest, AlertsSubscribed, ContactDto, ErrorCode, RequestError,
    SubscriptionState, UniverseIdHex,
};
use tokio::sync::mpsc;

use super::Subscription;
use crate::AppState;
use crate::compute::{CancelToken, panic_message};
use crate::convert::{AlertsAsked, acknowledge_refusal, contact_dto, open_contacts_refusal};
use crate::knowledge::{ContactId, PersistedKnowledge};
use crate::requests::{openable_universe, request_error};
use crate::subscriptions::{
    AlertsPush, PendingPush, Pusher, SubscriptionCommand, not_of_this_topic,
};

/// Opens an alerts subscription: checks the request, opens the universe's contacts, and starts
/// the task that answers the requests naming the subscription.
///
/// # Errors
///
/// As [`openable_universe`]; `bad_request` naming the field at fault for a request no sensor or
/// observer has ([`AlertsAsked`]); and [`open_contacts_refusal`]'s for contacts that cannot be
/// read.
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
    let known = known_contacts(&contacts);
    // The schedule's first scan is P12.T8.b's to run; until then it is not asked for.
    let (subscription, _first) = Subscription::new(
        asked.observer,
        asked.sensor,
        asked.classes,
        asked.host_limit,
    );
    let topic = Topic {
        subscription,
        contacts,
        commands: pusher.take_commands(),
        pusher: pusher.clone(),
    };
    spawn(topic, &pusher);
    Ok(SubscriptionState::Alerts(AlertsSubscribed {
        census: Vec::new(),
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

/// One alerts subscription's task: its schedule and its universe's contacts.
struct Topic {
    subscription: Subscription,
    contacts: PersistedKnowledge,
    commands: Option<mpsc::Receiver<SubscriptionCommand>>,
    pusher: Pusher,
}

impl Topic {
    /// Answers the requests that name the subscription until it ends.
    async fn run(mut self) {
        loop {
            let commands = self.commands.as_mut();
            let command = tokio::select! {
                biased;
                () = self.pusher.ended() => return,
                Some(command) = async { match commands {
                    Some(commands) => commands.recv().await,
                    None => std::future::pending().await,
                } } => command,
            };
            self.on_command(command).await;
        }
    }

    /// Answers a request the connection routed here.
    async fn on_command(&mut self, command: SubscriptionCommand) {
        match command {
            SubscriptionCommand::AlertsObserver { observer, answer } => {
                // P12.T8.c records the due pushes and P12.T8.b runs the scans asked for; until
                // then nothing is merged, so nothing falls due.
                let (_due, _rescan) = self.subscription.advance(observer).into_parts();
                // A request cancelled meanwhile has nowhere to be answered, and needs nothing.
                let _ = answer.send(Ok(()));
            }
            SubscriptionCommand::AlertsAcknowledge { contact, answer } => {
                let acknowledged = self.acknowledge(contact).await;
                // As above: a cancelled request needs no answer.
                let _ = answer.send(acknowledged);
            }
            other @ SubscriptionCommand::SceneCameras { .. } => {
                let refusal = not_of_this_topic(other.kind(), "alerts");
                other.refuse(refusal);
            }
        }
    }

    /// Acknowledges `contact` and, if that changed it, pushes it as it now stands.
    async fn acknowledge(&self, contact: ContactId) -> Result<(), RequestError> {
        let changed = self
            .contacts
            .acknowledge(contact)
            .await
            .map_err(|error| acknowledge_refusal(&error))?;
        if changed && let Some(view) = self.contacts.read(|store| store.view(contact)) {
            self.pusher.push(PendingPush::Alerts(AlertsPush {
                contacts: vec![contact_dto(&view)],
                census: Vec::new(),
            }));
        }
        Ok(())
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
