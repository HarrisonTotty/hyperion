//! The alert service's scans over a socket (plan 12, P12.T8.b and P12.T9.b): a subscription near a
//! pinned supernova is told of it at `bearing` as its light rises past the sensor's detection
//! limit, and then, moved close, of the same contact `resolved` with its host; a class walked in
//! the background whose walk is given up stays `pending` and delivers nothing; and one walked across
//! the whole galaxy reads `complete` with the candidates it drew (P12.T8.b.2).
//!
//! P12.T9.b's test is to watch a pinned recurrent nova. Plan 11's novae are not walked yet, so a
//! supernova stands in, as the brief allows (`common::alerts`).

mod common;

use common::TestServer;
use common::alerts::{
    DAY, FAR_LY, SEED, after, arrival, beside, contacts_until, contacts_until_answered, light_at,
    move_to, opened, peak, pinned, subscribe, v_sensor,
};
use hyperion_protocol::{
    ClassCensusStatusDto, ContactDto, NotificationBody, RequestBody, ResponseBody,
    UnsubscribeRequest,
};
use hyperion_server::compute::BulkGate;
use hyperion_server::knowledge::PersistedKnowledge;
use hyperion_server::universe::{UniverseId, UniverseStore};
use hyperion_sim::Seed;
use hyperion_sim::alerts::class_reach;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::ClassId;

/// How close the observer moves, ly: ten times closer than [`FAR_LY`], a hundred times the flux.
const CLOSE_LY: f64 = 0.02;

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
    let (subscription, census) =
        opened(&mut client, subscribe(&universe, &far, start, limits)).await;
    assert_eq!(
        census.status,
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
    let (subscription, census) = opened(&mut client, request).await;
    assert_eq!(census.status, ClassCensusStatusDto::Pending);
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

/// A sensor that sees across the galaxy, a V limit of 10⁻¹² W m⁻², at the Sun-like point: its core
/// collapses, too many cells to count interactively, open `pending` with no count of hosts, are
/// walked in the background over every slab of the root cube, and read `complete` with the
/// candidates the walk drew, hundreds of thousands (P12.T8.b.2; research's 5.0 × 10⁵ for the Milky
/// Way fixture).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alerts_scans_walk_a_galaxy_wide_class_to_complete() {
    let sun =
        hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap();
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Alerts: the galaxy", SEED).await.id;
    let start = hyperion_sim::time::UniverseTime::EPOCH;
    let (_, census) = opened(
        &mut client,
        subscribe(&universe, &sun, start, (1e-12, 1e-10)),
    )
    .await;
    assert_eq!(
        (census.status, census.expected_hosts),
        (ClassCensusStatusDto::Pending, None)
    );
    let settled = loop {
        let (_, NotificationBody::Alerts(notification)) = client.next_notification().await else {
            panic!("an alerts notification");
        };
        if let [line] = notification.census[..]
            && line.status != ClassCensusStatusDto::Pending
        {
            break line;
        }
    };
    assert_eq!(
        settled.status,
        ClassCensusStatusDto::Complete,
        "{settled:?}"
    );
    let hosts = settled.expected_hosts.unwrap_or_default();
    assert!((1e5..1e7).contains(&hosts), "{settled:?}");
    client.close().await;
    server.stop().await;
}
