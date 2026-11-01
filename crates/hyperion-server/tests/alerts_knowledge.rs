//! The alerts into Knowledge over a socket (plan 12, P12.T8.c): a contact shows as a bearing before
//! it resolves; a clear goes on the wire to every subscription of the universe, and a subscription
//! opened after it opens with the contact cleared; and the census per class rides a notification,
//! `over_limit` and `unwalked` among it. Each watches the pinned supernova (`common::alerts`).

mod common;

use common::TestServer;
use common::alerts::{
    DAY, FAR_LY, SEED, after, arrival, beside, contacts_until, fallen_under, light_at, move_to,
    notified_until, number_and_cleared, opened, opened_with, peak, pinned, standing_between,
    subscribe, subscribe_to, v_sensor,
};
use hyperion_protocol::{CatalogueClassWire, ClassCensusStatusDto, ContactDto, NotificationBody};
use hyperion_sim::Seed;
use hyperion_sim::alerts::class_reach;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::ClassId;

/// Subscribed 0.2 ly from the pinned supernova a day before its light arrives, through a V sensor
/// that detects it at a quarter of its peak there and resolves it at half: once the clock stands
/// where its light lies between the two, it is one contact at `bearing`; once it passes the peak,
/// the same contact, `resolved`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alerts_knowledge_show_a_contact_as_a_bearing_before_it_resolves() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let supernova = pinned(&galaxy);
    let far = beside(&supernova, FAR_LY);
    let light = light_at(&galaxy, &supernova, &far);
    let (peak_at, peak) = peak(&light);
    let (detect, resolve) = (peak / 4.0, peak / 2.0);
    let between = standing_between(&light, detect, resolve);
    assert!(
        light.flux_at(after(between, 1)).value() < resolve,
        "a second on, the light is still under the resolve limit"
    );

    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client
        .create_universe("Alerts: bearing first", SEED)
        .await
        .id;
    let start = after(arrival(&supernova, &far), -DAY);
    let (subscription, _) = opened(
        &mut client,
        subscribe(&universe, &far, start, (detect, resolve)),
    )
    .await;

    let id = move_to(&mut client, subscription, &far, after(between, 1)).await;
    let seen = contacts_until(&mut client, id, |_| true).await;
    let [ContactDto::Bearing(bearing)] = &seen[..] else {
        panic!("one contact, at a bearing: {seen:?}");
    };
    let id = move_to(&mut client, subscription, &far, after(peak_at, 1)).await;
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

/// Two subscriptions of one universe, on two connections: the first 0.2 ly from the pinned
/// supernova through a V sensor that detects it at half its peak, the second far off, seeing
/// nothing of its own. The second is told of the contact the first's push made, at `bearing`, and
/// then of its clear, once the first's clock passes the light's fall under the limit. A third
/// subscription, opened after, opens with the contact cleared.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alerts_knowledge_tell_every_subscription_of_the_universe_of_a_clear() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let supernova = pinned(&galaxy);
    let far = beside(&supernova, FAR_LY);
    let light = light_at(&galaxy, &supernova, &far);
    let (peak_at, peak) = peak(&light);
    let limits = (peak / 2.0, peak * 2.0);
    let start = after(arrival(&supernova, &far), -DAY);
    let fallen = fallen_under(&light, peak_at, limits.0);
    assert!(
        fallen < after(start, 365 * DAY),
        "the light falls under the limit within the first scan's year"
    );

    let server = TestServer::start().await;
    let mut watching = server.connected().await;
    let mut elsewhere = server.connected().await;
    let universe = watching.create_universe("Alerts: a clear", SEED).await.id;
    let (near, _) = opened(&mut watching, subscribe(&universe, &far, start, limits)).await;
    let away = beside(&supernova, 5_000.0);
    let (_, opened_contacts, _) = opened_with(
        &mut elsewhere,
        subscribe_to(&universe, &away, start, limits, &[], None),
    )
    .await;
    assert_eq!(opened_contacts, []);

    let id = move_to(&mut watching, near, &far, after(peak_at, 1)).await;
    let seen = contacts_until(&mut watching, id, |_| true).await;
    let [ContactDto::Bearing(bearing)] = &seen[..] else {
        panic!("one contact, at a bearing: {seen:?}");
    };
    let told = notified_until(&mut elsewhere, |_| true).await;
    assert_eq!(
        told.iter().map(number_and_cleared).collect::<Vec<_>>(),
        [(bearing.contact, false)],
        "the other subscription is told of the contact, lit"
    );

    let id = move_to(&mut watching, near, &far, after(fallen, 1)).await;
    let cleared = |contact: &ContactDto| number_and_cleared(contact).1;
    let seen = contacts_until(&mut watching, id, cleared).await;
    assert_eq!(
        seen.last().map(number_and_cleared),
        Some((bearing.contact, true))
    );
    let told = notified_until(&mut elsewhere, cleared).await;
    let Some(ContactDto::Bearing(clear)) = told.last() else {
        panic!("the contact, cleared at a bearing: {told:?}");
    };
    assert_eq!(clear.contact, bearing.contact);
    assert!(
        clear
            .cleared
            .is_some_and(|at| peak_at.seconds() < at.seconds && at.seconds <= fallen.seconds()),
        "it cleared as its light fell, after its peak and by the knot it was under: {clear:?}"
    );

    let mut late = server.connected().await;
    let (_, contacts, _) = opened_with(
        &mut late,
        subscribe_to(&universe, &away, start, limits, &[], None),
    )
    .await;
    assert_eq!(
        contacts.iter().map(number_and_cleared).collect::<Vec<_>>(),
        [(bearing.contact, true)],
        "a subscription opened after the clear opens with the contact cleared"
    );
    for client in [watching, elsewhere, late] {
        client.close().await;
    }
    server.stop().await;
}

/// The census per class rides a notification: subscribed to the core collapses and the luminous
/// blue variables at a host limit of one, through a V sensor that reaches 700 ly, both open
/// `pending`; once the first scan has run, a notification carries each class's census in class
/// order, the core collapses `over_limit` with their hosts counted, and the luminous blue
/// variables, whose walk is not built, `unwalked` with none.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alerts_knowledge_carry_the_census_per_class_over_the_limit_included() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let supernova = pinned(&galaxy);
    let far = beside(&supernova, FAR_LY);
    let probe = 1e-6;
    let reach = class_reach(ClassId::CORE_COLLAPSE, &v_sensor(probe, probe)).value();
    let ratio = reach / 700.0;
    let detect = probe * ratio * ratio;

    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Alerts: the census", SEED).await.id;
    let classes = [
        CatalogueClassWire::CoreCollapse,
        CatalogueClassWire::LuminousBlueVariable,
    ];
    let request = subscribe_to(
        &universe,
        &far,
        arrival(&supernova, &far),
        (detect, detect * 10.0),
        &classes,
        Some(1),
    );
    let (_, _, census) = opened_with(&mut client, request).await;
    assert_eq!(
        census
            .iter()
            .map(|line| (line.class, line.status))
            .collect::<Vec<_>>(),
        classes.map(|class| (class, ClassCensusStatusDto::Pending))
    );

    let census = loop {
        let (_, NotificationBody::Alerts(notification)) = client.next_notification().await else {
            panic!("an alerts notification");
        };
        if !notification.census.is_empty() {
            break notification.census;
        }
    };
    let [core_collapses, eruptions] = census[..] else {
        panic!("each class's census: {census:?}");
    };
    assert_eq!(
        (core_collapses.class, core_collapses.status),
        (
            CatalogueClassWire::CoreCollapse,
            ClassCensusStatusDto::OverLimit
        )
    );
    assert!(
        (core_collapses.reach_ly - 700.0).abs() < 1.0,
        "{core_collapses:?}"
    );
    assert!(
        core_collapses
            .expected_hosts
            .is_some_and(|hosts| hosts > 1.0)
    );
    assert_eq!(
        (eruptions.class, eruptions.status, eruptions.expected_hosts),
        (
            CatalogueClassWire::LuminousBlueVariable,
            ClassCensusStatusDto::Unwalked,
            None
        )
    );
    client.close().await;
    server.stop().await;
}
