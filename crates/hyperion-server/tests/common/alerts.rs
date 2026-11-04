//! The alerts' helpers, shared by the alerts' integration tests (plan 12, P12.T8.b and P12.T8.c):
//! a pinned supernova, its light as an observer receives it, and subscribing, moving and reading
//! contacts over a socket.
//!
//! P12.T9.b's test is to watch a pinned recurrent nova. Plan 11's novae are not walked yet (their
//! registry row waits on P11.T9), so a supernova stands in: seed 9's core collapse that goes off in
//! the clock window's future 179 years after the epoch, in core-collapse cell [-2, 30, 0] (from
//! generator version 27, whose young field gives most of its young stars to the features, P09.T2.c;
//! at 26 a Type IIP 874 years after the epoch in cell [-3, 48, 0], which holds none at 27).

use hyperion_protocol::{
    AlertBandDto, AlertsObserverRequest, AlertsSubscribeRequest, BandLimitsDto, CatalogueClassWire,
    ClassCensusDto, ContactDto, GalacticPosition, NotificationBody, RequestBody, RequestId,
    ResponseBody, SensorDto, ServerMessage, SubscribeRequest, SubscriptionState, SubscriptionTopic,
    UniverseIdHex, UniverseTime,
};
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, ArrivingLight, BandLimits, ClassSet, DetectionLight,
    OneShotClass, OneShotEvent, SensorHorizon, one_shot_scan_cell,
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

use super::TestClient;

/// The galaxy of the pinned supernova.
pub const SEED: u64 = 9;

/// The core-collapse cell that holds it.
pub const PINNED_CELL: [i32; 3] = [-2, 30, 0];

/// How far from its site an observer first watches it, ly.
pub const FAR_LY: f64 = 0.2;

/// A day, s.
pub const DAY: i64 = 86_400;

/// The pinned supernova: the latest death of [`PINNED_CELL`] in the clock window's future.
pub fn pinned(galaxy: &Galaxy) -> OneShotEvent {
    let process = CoreCollapseProcess::new(galaxy);
    let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, PINNED_CELL).unwrap();
    CatalogueClassCell::generate(galaxy, &process, key)
        .entries()
        .iter()
        .filter_map(|entry| process.event(galaxy, entry.value()))
        .filter(|event| {
            let years = event.emitted().since_epoch().as_julian_years_f64();
            (0.0..990.0).contains(&years)
        })
        .max_by_key(OneShotEvent::emitted)
        .expect("seed 9's core collapse goes off 179 years after the epoch")
}

/// The place `away` light-years from `event`'s site, spinward along x.
pub fn beside(event: &OneShotEvent, away: f64) -> SimPosition {
    let [x, y, z] = event.site().to_light_years_f64();
    SimPosition::from_light_years([x + away, y, z]).unwrap()
}

/// When `event`'s light reaches `at`.
pub fn arrival(event: &OneShotEvent, at: &SimPosition) -> SimTime {
    event
        .emitted()
        .checked_add(light_time(event.site().distance_to(at)))
        .unwrap()
}

/// `event`'s light in V as the sim's one-shot walk of its cell gives it to an observer at `at`,
/// through the smooth gas the server's scans march while the feature members are held.
pub fn light_at(galaxy: &Galaxy, event: &OneShotEvent, at: &SimPosition) -> ArrivingLight {
    let seen = arrival(event, at);
    let window = TimeWindow::new(after(seen, -DAY), after(seen, DAY)).unwrap();
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
pub fn v_sensor(detect: f64, resolve: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(resolve)).unwrap();
    SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap()
}

/// The light's peak and when it arrives, from its knots.
pub fn peak(light: &ArrivingLight) -> (SimTime, f64) {
    light
        .knots()
        .into_iter()
        .map(|t| (t, light.flux_at(t).value()))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
}

/// The first of the light's knots after `from` at which its flux is `under` W m⁻² or less: past
/// the time it fell under it.
pub fn fallen_under(light: &ArrivingLight, from: SimTime, under: f64) -> SimTime {
    light
        .knots()
        .into_iter()
        .find(|&t| t > from && light.flux_at(t).value() <= under)
        .expect("a supernova's light falls")
}

/// The first of the light's knots at which its flux lies in `[low, high)` W m⁻².
pub fn standing_between(light: &ArrivingLight, low: f64, high: f64) -> SimTime {
    light
        .knots()
        .into_iter()
        .find(|&t| (low..high).contains(&light.flux_at(t).value()))
        .expect("the light rises through the band between the limits at a knot")
}

pub fn wire_position(position: &SimPosition) -> GalacticPosition {
    GalacticPosition {
        cell_ly: position.cell().to_array(),
        offset_m: position.offset_metres(),
    }
}

pub fn wire_time(t: SimTime) -> UniverseTime {
    UniverseTime {
        seconds: t.seconds(),
        nanos: t.subsec_nanos(),
    }
}

/// `t` and `seconds` later, or earlier for a negative count.
pub fn after(t: SimTime, seconds: i64) -> SimTime {
    if seconds < 0 {
        t.checked_sub(Span::from_seconds(-seconds)).unwrap()
    } else {
        t.checked_add(Span::from_seconds(seconds)).unwrap()
    }
}

/// An alerts subscription to the core collapses at `at` and `t`, through V limits of `detect` and
/// `resolve`, W m⁻².
pub fn subscribe(
    universe: &UniverseIdHex,
    at: &SimPosition,
    t: SimTime,
    limits: (f64, f64),
) -> RequestBody {
    subscribe_to(
        universe,
        at,
        t,
        limits,
        &[CatalogueClassWire::CoreCollapse],
        None,
    )
}

/// An alerts subscription to `classes` at `at` and `t`, through V limits of `detect` and
/// `resolve`, W m⁻², at the host limit `host_limit` or the server's.
pub fn subscribe_to(
    universe: &UniverseIdHex,
    at: &SimPosition,
    t: SimTime,
    limits: (f64, f64),
    classes: &[CatalogueClassWire],
    host_limit: Option<u32>,
) -> RequestBody {
    subscribe_in(
        universe,
        (at, t),
        AlertBandDto::V,
        limits,
        classes,
        host_limit,
    )
}

/// An alerts subscription to `classes` at `at` and `t`, through limits of `detect` and `resolve`,
/// W m⁻², in `band`, at the host limit `host_limit` or the server's.
pub fn subscribe_in(
    universe: &UniverseIdHex,
    (at, t): (&SimPosition, SimTime),
    band: AlertBandDto,
    (detect, resolve): (f64, f64),
    classes: &[CatalogueClassWire],
    host_limit: Option<u32>,
) -> RequestBody {
    RequestBody::Subscribe(SubscribeRequest {
        universe: universe.clone(),
        topic: SubscriptionTopic::Alerts(AlertsSubscribeRequest {
            observer: wire_position(at),
            time: wire_time(t),
            sensor: SensorDto {
                bands: vec![BandLimitsDto {
                    band,
                    detect_w_m2: detect,
                    resolve_w_m2: resolve,
                }],
            },
            classes: Some(classes.to_vec()),
            host_limit,
        }),
    })
}

/// The subscription a `subscribe` opened, the contacts it opened with, and its census.
pub async fn opened_with(
    client: &mut TestClient,
    request: RequestBody,
) -> (u32, Vec<ContactDto>, Vec<ClassCensusDto>) {
    let ResponseBody::Subscribe(subscribed) = client.request(request).await.unwrap() else {
        panic!("expected `subscribed`");
    };
    let SubscriptionState::Alerts(state) = subscribed.state else {
        panic!("an alerts subscription opens with the alerts' state");
    };
    (subscribed.subscription, state.contacts, state.census)
}

/// The subscription a `subscribe` to the core collapses opened with no contact, and its census's
/// one class's.
pub async fn opened(client: &mut TestClient, request: RequestBody) -> (u32, ClassCensusDto) {
    let (subscription, contacts, census) = opened_with(client, request).await;
    assert_eq!(contacts, []);
    let [census] = census[..] else {
        panic!("one class's census: {census:?}");
    };
    assert_eq!(census.class, CatalogueClassWire::CoreCollapse);
    (subscription, census)
}

/// Moves `subscription`'s observer to `at` and `t`, without waiting for the answer.
pub async fn move_to(
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

/// Checks that no bearing in a message's JSON as sent holds a host.
fn no_bearing_holds_a_host(json: &Value) {
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
}

/// Reads messages until request `id` is answered and a notification has brought a contact
/// `until` accepts: every contact the notifications carry, in order. Every bearing among them is
/// checked, in the JSON as sent, to hold no host.
pub async fn contacts_until(
    client: &mut TestClient,
    id: RequestId,
    until: impl Fn(&ContactDto) -> bool,
) -> Vec<ContactDto> {
    let (mut answered, mut contacts) = (false, Vec::new());
    while !answered || !contacts.iter().any(&until) {
        let text = client.next_text().await;
        no_bearing_holds_a_host(&serde_json::from_str(&text).unwrap());
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

/// Reads notifications, and nothing else, until one brings a contact `until` accepts: every
/// contact they carry, in order, each bearing checked to hold no host. For a client that has asked
/// nothing, told of what another subscription changed.
pub async fn notified_until(
    client: &mut TestClient,
    until: impl Fn(&ContactDto) -> bool,
) -> Vec<ContactDto> {
    let mut contacts: Vec<ContactDto> = Vec::new();
    while !contacts.iter().any(&until) {
        let text = client.next_text().await;
        no_bearing_holds_a_host(&serde_json::from_str(&text).unwrap());
        match serde_json::from_str(&text).unwrap() {
            ServerMessage::Notification {
                body: NotificationBody::Alerts(notification),
                ..
            } => contacts.extend(notification.contacts),
            other => panic!("expected an alerts notification: {other:?}"),
        }
    }
    contacts
}

/// Reads messages until request `id` is answered: the contacts the notifications before it carry.
pub async fn contacts_until_answered(client: &mut TestClient, id: RequestId) -> Vec<ContactDto> {
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

/// The contact's number and whether it has cleared, at either level.
pub fn number_and_cleared(contact: &ContactDto) -> (u32, bool) {
    match contact {
        ContactDto::Bearing(seen) => (seen.contact, seen.cleared.is_some()),
        ContactDto::Resolved(resolved) => (resolved.seen.contact, resolved.seen.cleared.is_some()),
    }
}
