//! The alert service's scans over a socket (plan 12, P12.T8.b and P12.T9.b): a subscription near a
//! pinned supernova is told of it at `bearing` as its light rises past the sensor's detection
//! limit, and then, moved close, of the same contact `resolved` with its host; and a class walked
//! in the background whose walk is given up stays `pending` and delivers nothing.
//!
//! P12.T9.b's test is to watch a pinned recurrent nova. Plan 11's novae are not walked yet (their
//! registry row waits on P11.T9), so a supernova stands in, as the brief allows: seed 9's one core
//! collapse of the 288 cells about the solar circle that goes off in the clock window's future, a
//! Type IIP 874 years after the epoch, in core-collapse cell [-3, 48, 0].

mod common;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    AlertBandDto, AlertsObserverRequest, AlertsSubscribeRequest, BandLimitsDto, CatalogueClassWire,
    ClassCensusStatusDto, ContactDto, GalacticPosition, NotificationBody, RequestBody, RequestId,
    ResponseBody, SensorDto, ServerMessage, SubscribeRequest, SubscriptionState, SubscriptionTopic,
    UniverseIdHex, UniverseTime, UnsubscribeRequest,
};
use hyperion_server::compute::BulkGate;
use hyperion_server::knowledge::PersistedKnowledge;
use hyperion_server::universe::{UniverseId, UniverseStore};
use hyperion_sim::Seed;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, ArrivingLight, BandLimits, ClassSet, DetectionLight,
    OneShotClass, OneShotEvent, SensorHorizon, class_reach, one_shot_scan_cell,
};
use hyperion_sim::coords::GalacticPosition as SimPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::grid::CatalogueClassCell;
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::observe::light_time;
use hyperion_sim::time::{Span, UniverseTime as SimTime};
use hyperion_sim::units::WattsPerSquareMetre;
use serde_json::Value;

/// The galaxy of the pinned supernova.
const SEED: u64 = 9;

/// The core-collapse cell that holds it.
const PINNED_CELL: [i32; 3] = [-3, 48, 0];

/// How far from its site the observer first watches it, and then how close it moves, ly: ten times
/// closer, a hundred times the flux.
const FAR_LY: f64 = 0.2;
const CLOSE_LY: f64 = 0.02;

/// A day, s.
const DAY: i64 = 86_400;

/// The pinned supernova: the latest death of [`PINNED_CELL`] in the clock window's future.
fn pinned(galaxy: &Galaxy) -> OneShotEvent {
    let process = CoreCollapseProcess::new(galaxy);
    let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, PINNED_CELL).unwrap();
    CatalogueClassCell::generate(galaxy, &process, key)
        .entries()
        .iter()
        .map(|entry| process.event(entry.value()))
        .filter(|event| {
            let years = event.emitted().since_epoch().as_julian_years_f64();
            (0.0..990.0).contains(&years)
        })
        .max_by_key(OneShotEvent::emitted)
        .expect("seed 9's core collapse goes off 874 years after the epoch")
}

/// The place `away` light-years from `event`'s site, spinward along x.
fn beside(event: &OneShotEvent, away: f64) -> SimPosition {
    let [x, y, z] = event.site().to_light_years_f64();
    SimPosition::from_light_years([x + away, y, z]).unwrap()
}

/// When `event`'s light reaches `at`.
fn arrival(event: &OneShotEvent, at: &SimPosition) -> SimTime {
    event
        .emitted()
        .checked_add(light_time(event.site().distance_to(at)))
        .unwrap()
}

/// `event`'s light in V as the sim's one-shot walk of its cell gives it to an observer at `at`,
/// through the smooth gas the server's scans march while the feature members are held.
fn light_at(galaxy: &Galaxy, event: &OneShotEvent, at: &SimPosition) -> ArrivingLight {
    let seen = arrival(event, at);
    let window = TimeWindow::new(
        seen.checked_sub(Span::from_seconds(DAY)).unwrap(),
        seen.checked_add(Span::from_seconds(DAY)).unwrap(),
    )
    .unwrap();
    let query = AlertQuery::builder(*at, window, v_sensor(1e-30, 1e-29))
        .classes(ClassSet::of(ClassId::CORE_COLLAPSE))
        .build()
        .unwrap();
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let process = CoreCollapseProcess::new(galaxy);
    let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, PINNED_CELL).unwrap();
    let mut seen = Vec::new();
    one_shot_scan_cell(galaxy, &mut ctx, &query, &process, key, &mut seen);
    let detection = seen
        .iter()
        .find(|detection| detection.event() == event.event())
        .expect("the walk of its cell sees it go off");
    match *detection.light() {
        DetectionLight::Lit(light) => light,
        DetectionLight::Unlit => panic!("a supernova has light"),
    }
}

/// A V sensor of these limits, W m⁻².
fn v_sensor(detect: f64, resolve: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(resolve)).unwrap();
    SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap()
}

/// The light's peak and when it arrives, from its knots.
fn peak(light: &ArrivingLight) -> (SimTime, f64) {
    light
        .knots()
        .into_iter()
        .map(|t| (t, light.flux_at(t).value()))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
}

fn wire_position(position: &SimPosition) -> GalacticPosition {
    GalacticPosition {
        cell_ly: position.cell().to_array(),
        offset_m: position.offset_metres(),
    }
}

fn wire_time(t: SimTime) -> UniverseTime {
    UniverseTime {
        seconds: t.seconds(),
        nanos: t.subsec_nanos(),
    }
}

/// `t` and `seconds` later.
fn after(t: SimTime, seconds: i64) -> SimTime {
    t.checked_add(Span::from_seconds(seconds)).unwrap()
}

/// An alerts subscription to the core collapses at `at` and `t`, through V limits of `detect` and
/// `resolve`, W m⁻².
fn subscribe(
    universe: &UniverseIdHex,
    at: &SimPosition,
    t: SimTime,
    (detect, resolve): (f64, f64),
) -> RequestBody {
    RequestBody::Subscribe(SubscribeRequest {
        universe: universe.clone(),
        topic: SubscriptionTopic::Alerts(AlertsSubscribeRequest {
            observer: wire_position(at),
            time: wire_time(t),
            sensor: SensorDto {
                bands: vec![BandLimitsDto {
                    band: AlertBandDto::V,
                    detect_w_m2: detect,
                    resolve_w_m2: resolve,
                }],
            },
            classes: Some(vec![CatalogueClassWire::CoreCollapse]),
            host_limit: None,
        }),
    })
}

/// The subscription a `subscribe` opened, and its census's one class's status.
async fn opened(client: &mut TestClient, request: RequestBody) -> (u32, ClassCensusStatusDto) {
    let ResponseBody::Subscribe(subscribed) = client.request(request).await.unwrap() else {
        panic!("expected `subscribed`");
    };
    let SubscriptionState::Alerts(state) = subscribed.state else {
        panic!("an alerts subscription opens with the alerts' state");
    };
    assert_eq!(state.contacts, []);
    let [census] = state.census[..] else {
        panic!("one class's census: {:?}", state.census);
    };
    assert_eq!(census.class, CatalogueClassWire::CoreCollapse);
    (subscribed.subscription, census.status)
}

/// Moves `subscription`'s observer to `at` and `t`, without waiting for the answer.
async fn move_to(
    client: &mut TestClient,
    subscription: u32,
    at: &SimPosition,
    t: SimTime,
) -> RequestId {
    client
        .send_request(RequestBody::AlertsObserver(AlertsObserverRequest {
            subscription,
            observer: wire_position(at),
            time: wire_time(t),
        }))
        .await
}

/// Reads messages until request `id` is answered and a notification has brought a contact
/// `until` accepts: every contact the notifications carry, in order. Every bearing among them is
/// checked, in the JSON as sent, to hold no host.
async fn contacts_until(
    client: &mut TestClient,
    id: RequestId,
    until: impl Fn(&ContactDto) -> bool,
) -> Vec<ContactDto> {
    let (mut answered, mut contacts) = (false, Vec::new());
    while !answered || !contacts.iter().any(&until) {
        let text = client.next_text().await;
        let json: Value = serde_json::from_str(&text).unwrap();
        for contact in json["body"]["contacts"].as_array().into_iter().flatten() {
            if contact["level"] == "bearing" {
                for key in ["host", "designation", "light_age_yr", "apparent_position"] {
                    assert!(
                        contact.get(key).is_none(),
                        "a bearing holds `{key}`: {json}"
                    );
                }
            }
        }
        match serde_json::from_str(&text).unwrap() {
            ServerMessage::Response { id: answer, .. } if answer == id => answered = true,
            ServerMessage::Notification {
                body: NotificationBody::Alerts(notification),
                ..
            } => contacts.extend(notification.contacts),
            other => panic!("expected the move's answer or an alerts notification: {other:?}"),
        }
    }
    contacts
}

/// P12.T9.b, on the pinned supernova (module documentation): subscribed 0.2 ly from it a day
/// before its light arrives, through a V sensor that detects it at half its peak there and resolves
/// it at no point of its light there, the subscriber is told of one contact at `bearing` once its
/// clock passes the peak; moved to 0.02 ly a moment later, where the light arrived 66 days earlier
/// and stands over the resolve limit, it is told of the same contact `resolved`, with the
/// supernova's host. No bearing's JSON holds a host.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alerts_scans_bear_then_resolve_a_pinned_supernova() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let supernova = pinned(&galaxy);
    let (far, close) = (beside(&supernova, FAR_LY), beside(&supernova, CLOSE_LY));
    let (far_peak_at, far_peak) = peak(&light_at(&galaxy, &supernova, &far));
    let at_close = after(far_peak_at, 2);
    let close_then = light_at(&galaxy, &supernova, &close)
        .flux_at(at_close)
        .value();
    assert!(
        close_then > 4.0 * far_peak,
        "moved close, the light stands well over its peak from afar: {close_then} against \
         {far_peak} W m⁻²"
    );
    let limits = (far_peak / 2.0, (far_peak * close_then).sqrt());

    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Alerts: a supernova", SEED).await.id;
    let start = after(arrival(&supernova, &far), -DAY);
    let (subscription, status) =
        opened(&mut client, subscribe(&universe, &far, start, limits)).await;
    assert_eq!(
        status,
        ClassCensusStatusDto::Pending,
        "before its first scan"
    );

    // Past the peak, one contact: the supernova, at a bearing.
    let id = move_to(&mut client, subscription, &far, after(far_peak_at, 1)).await;
    let seen = contacts_until(&mut client, id, |_| true).await;
    let [ContactDto::Bearing(bearing)] = &seen[..] else {
        panic!("one contact, at a bearing: {seen:?}");
    };
    assert_eq!(bearing.phase, hyperion_protocol::DetectionPhaseDto::Onset);

    // Moved close, the same contact, resolved with its host.
    let id = move_to(&mut client, subscription, &close, at_close).await;
    let pushed = contacts_until_answered(&mut client, id).await;
    assert_eq!(pushed, [], "nothing is pushed before the clock moves on");
    let id = move_to(&mut client, subscription, &close, after(at_close, 1)).await;
    let seen = contacts_until(&mut client, id, |contact| {
        matches!(contact, ContactDto::Resolved(_))
    })
    .await;
    let Some(ContactDto::Resolved(resolved)) = seen.last() else {
        panic!("the contact, resolved: {seen:?}");
    };
    assert_eq!(resolved.seen.contact, bearing.contact);
    assert_eq!(resolved.host.to_u64(), supernova.host().raw());
    client.close().await;
    server.stop().await;
}

/// Reads messages until request `id` is answered: the contacts the notifications before it carry.
async fn contacts_until_answered(client: &mut TestClient, id: RequestId) -> Vec<ContactDto> {
    let mut contacts = Vec::new();
    loop {
        match client.next_message().await {
            ServerMessage::Response { id: answer, .. } if answer == id => return contacts,
            ServerMessage::Notification {
                body: NotificationBody::Alerts(notification),
                ..
            } => contacts.extend(notification.contacts),
            other => panic!("expected the move's answer or an alerts notification: {other:?}"),
        }
    }
}

/// A class the census sends to the background whose walk is given up stays `pending` and delivers
/// nothing (P12.T8.b): subscribed 0.2 ly from the pinned supernova through a V sensor that reaches
/// 16,000 ly, more cells than an interactive walk takes, the core collapses wait for the
/// background, whose plan the pool holds at its gate; the clock passes the supernova's arrival,
/// and the observer moves away, which gives the walk up and starts the new place's, held too; and
/// the subscription ends. Let through, both plans are skipped as cancelled: the subscriber was told
/// nothing, and the universe's Knowledge holds no contact.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alerts_scans_give_up_a_background_walk_and_deliver_nothing() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let supernova = pinned(&galaxy);
    let far = beside(&supernova, FAR_LY);
    // A reach of 16,000 ly: the class's reach goes as the limit's inverse square root.
    let probe = 1e-6;
    let reach = class_reach(ClassId::CORE_COLLAPSE, &v_sensor(probe, probe)).value();
    let ratio = reach / 16_000.0;
    let detect = probe * ratio * ratio;
    assert!(
        (class_reach(ClassId::CORE_COLLAPSE, &v_sensor(detect, detect)).value() - 16_000.0).abs()
            < 1.0
    );

    let gate = BulkGate::default();
    gate.close();
    let data_dir = tempfile::tempdir().unwrap();
    let config = TestServer::config(data_dir.path()).bulk_gate(gate.clone());
    let server = TestServer::start_with(config.build()).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Alerts: given up", SEED).await.id;
    let start = after(arrival(&supernova, &far), -DAY);
    let request = subscribe(&universe, &far, start, (detect, detect * 10.0));
    let (subscription, status) = opened(&mut client, request).await;
    assert_eq!(status, ClassCensusStatusDto::Pending);
    server
        .stats_until("the background walk's plan is held at the gate", |_| {
            gate.held() == 1
        })
        .await;

    // The supernova's light arrives and peaks: the class is walked in the background, so nothing.
    let id = move_to(&mut client, subscription, &far, after(start, 60 * DAY)).await;
    assert_eq!(contacts_until_answered(&mut client, id).await, []);
    // Moved away, the walk is given up, and the new place's is held.
    let away = beside(&supernova, 50.0);
    let id = move_to(&mut client, subscription, &away, after(start, 61 * DAY)).await;
    assert_eq!(contacts_until_answered(&mut client, id).await, []);
    let before = server
        .stats_until("the new place's plan is held at the gate", |_| {
            gate.held() == 2
        })
        .await
        .pool();
    let (answer, pushed) = client
        .request_among_notifications(RequestBody::Unsubscribe(UnsubscribeRequest {
            subscription,
        }))
        .await;
    assert_eq!(answer, Ok(ResponseBody::Unsubscribe));
    assert_eq!(
        pushed,
        [],
        "the census stays pending and no contact is pushed"
    );
    gate.open();
    let skipped = server
        .stats_until("both plans are skipped", |stats| {
            stats.pool().cancelled() == before.cancelled() + 2 && stats.pool().running() == 0
        })
        .await
        .pool();
    assert_eq!(skipped.completed(), before.completed(), "{skipped:?}");
    assert_eq!(skipped.queued_bulk(), 0, "{skipped:?}");
    client.close().await;
    server.stop().await;

    let store = UniverseStore::new(data_dir.path());
    let knowledge = PersistedKnowledge::open(&store, UniverseId::from(&universe))
        .await
        .unwrap();
    assert_eq!(knowledge.read(|store| store.contacts().count()), 0);
}
