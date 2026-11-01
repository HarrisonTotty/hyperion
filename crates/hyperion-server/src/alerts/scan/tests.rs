//! The scans' walks (plan 12, P12.T8.b): how far a first scan walks back, what keys a walk, and a
//! background walk split into chunks, which gives the interactive walk's pushes, each once.

use std::collections::BTreeSet;

use hyperion_sim::alerts::{ClassSet, LocalEventsCache, LongestRise, SensorHorizon, TransientKind};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::catalogue_classes::ClassId;
use hyperion_sim::id::EventId;
use hyperion_sim::observe::Observer;
use hyperion_sim::time::{Span, UniverseTime};

use super::*;
use crate::alerts::testing::{
    DAY, after, arrival, beside, core_collapses, galaxy, key, pinned, reaching, server, v_sensor,
};
use crate::alerts::{ALERT_LOOKAHEAD, Rescan, ScanRequest};

/// How far a class's or a kind's rise reaches back.
fn bound(rise: Option<LongestRise>) -> Span {
    match rise {
        Some(LongestRise::AtMost(span)) => span,
        other => panic!("a bound rise, not {other:?}"),
    }
}

/// How far `query` reaches back from `request`'s window, which it ends with.
fn back(request: &ScanRequest, query: &AlertQuery) -> Span {
    assert_eq!(query.window().end(), request.window().end());
    request
        .window()
        .start()
        .checked_since(query.window().start())
        .unwrap()
}

fn sun() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap()
}

/// A first scan walks each class back by its own longest rise, 78 days for the core collapses and
/// 16 for the Type Ia, and the local events by an FU Orionis outburst's 25 years; the classes keep
/// their own: a renewal walks back by nothing.
#[test]
fn a_first_scan_walks_each_class_back_by_its_longest_rise_and_a_renewal_by_none() {
    let at = Observer::new(sun(), UniverseTime::EPOCH).unwrap();
    let classes = ClassSet::of(ClassId::CORE_COLLAPSE).with(ClassId::TYPE_IA);
    let (mut subscription, first) = Subscription::new(
        at,
        v_sensor(1e-12, 1e-10),
        classes,
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    );
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    for class in classes.iter() {
        let query = spec.class_query(class);
        assert_eq!(query.classes(), ClassSet::of(class));
        assert_eq!(back(&first, &query), bound(class_longest_rise(class)));
    }
    assert_eq!(
        back(&first, &spec.class_query(ClassId::CORE_COLLAPSE)),
        Span::from_seconds(78 * DAY)
    );
    let local = spec.local_query();
    assert_eq!(local.classes(), ClassSet::default());
    assert_eq!(
        back(&first, &local),
        bound(TransientKind::FuOrionisOutburst.row().longest_rise())
    );

    subscription.merge(&first, []).unwrap();
    let later = Observer::new(sun(), after(UniverseTime::EPOCH, 200 * DAY)).unwrap();
    let Some(Rescan::Renew(renewal)) = subscription.advance(later).rescan().copied() else {
        panic!("past half a look-ahead, a renewal");
    };
    let spec = ScanSpec::new(key(), &subscription, &renewal).unwrap();
    for query in [spec.class_query(ClassId::CORE_COLLAPSE), spec.local_query()] {
        assert_eq!(query.window(), renewal.window());
    }
}

/// Two subscriptions alike ask for one walk; a walk is keyed by every input it reads, the limits
/// and the place to the bit, and by whether its scan starts its schedule.
#[test]
fn a_scans_key_is_what_its_walk_reads() {
    let key_of = |at: GalacticPosition, sensor: SensorHorizon| {
        let (subscription, first) = core_collapses(at, UniverseTime::EPOCH, sensor);
        ScanSpec::new(key(), &subscription, &first).unwrap().key()
    };
    let sensor = || v_sensor(1e-12, 1e-10);
    let alike = key_of(sun(), sensor());
    assert_eq!(key_of(sun(), sensor()), alike);
    // Some 9,500 km off.
    let nearby = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0 + 1e-9]).unwrap();
    assert_ne!(nearby, sun());
    assert_ne!(key_of(nearby, sensor()), alike);
    assert_ne!(key_of(sun(), v_sensor(1e-12_f64.next_down(), 1e-10)), alike);
    assert_ne!(key_of(sun(), v_sensor(1e-12, 2e-10)), alike);

    // A schedule's first scan of a year, and another's renewal of the same year, walk the same
    // query from different starts.
    let (subscription, first) = core_collapses(sun(), UniverseTime::EPOCH, sensor());
    let year_before = UniverseTime::EPOCH.checked_sub(ALERT_LOOKAHEAD).unwrap();
    let (mut earlier, its_first) = core_collapses(sun(), year_before, sensor());
    earlier.merge(&its_first, []).unwrap();
    let on = Observer::new(sun(), after(year_before, 200 * DAY)).unwrap();
    let Some(Rescan::Renew(renewal)) = earlier.advance(on).rescan().copied() else {
        panic!("past half a look-ahead, a renewal");
    };
    assert_eq!(renewal.window(), first.window());
    let firsts = ScanSpec::new(key(), &subscription, &first).unwrap();
    let renewals = ScanSpec::new(key(), &earlier, &renewal).unwrap();
    assert_eq!(renewals.query, firsts.query);
    assert_ne!(renewals.key(), firsts.key());
}

/// The pushes of `tracks` merged as `request`'s, the scan of `subscription`, to the window's end:
/// each push's time, event and crossing.
fn pushes_of(
    subscription: &Subscription,
    request: &ScanRequest,
    tracks: Vec<Tracked>,
) -> Vec<(UniverseTime, EventId, Crossing)> {
    let mut subscription = subscription.clone();
    subscription.merge(request, tracks).unwrap();
    let end = Observer::new(*request.position(), request.window().end()).unwrap();
    subscription
        .advance(end)
        .due()
        .iter()
        .map(|push| (push.at(), push.detection().event(), push.crossing()))
        .collect()
}

/// A background walk of the core collapses split into chunks gives the whole's pushes, each once,
/// in chunks of one cell, of five and of all, and they are the interactive walk's: 0.2 ly from the
/// pinned supernova, through a V sensor that reaches some 700 ly, a few dozen of the class's cells,
/// over the first scan's year and its walk back.
#[tokio::test]
async fn a_background_walk_in_chunks_gives_the_interactive_walks_pushes_each_once() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let supernova = pinned();
    let far = beside(&supernova, 0.2);
    let start = after(arrival(&supernova, &far), -DAY);
    let (subscription, first) = core_collapses(far, start, reaching(700.0));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    let cells = background_cells(galaxy(), &spec, class, MAX_BACKGROUND_CELLS).unwrap();
    assert!(cells.len() > 10, "a few dozen cells: {}", cells.len());
    let token = CancelToken::new();
    let walk = |cells: &[BackgroundCell]| {
        walk_cells(state, galaxy(), &spec, class, cells, &token).expect("not cancelled")
    };
    let whole = walk(&cells);
    for per_job in [1, 5, cells.len()] {
        let chunked: Vec<_> = chunks(&cells, per_job)
            .iter()
            .flat_map(|chunk| walk(chunk))
            .collect();
        assert_eq!(chunked, whole, "in chunks of {per_job}");
    }
    let interactive = walk_interactive(state, galaxy(), &spec, &mut LocalEventsCache::default());
    assert_eq!(
        interactive.background().count(),
        0,
        "{:?}",
        interactive.census()
    );
    let interactive: Vec<_> = interactive
        .tracks()
        .iter()
        .filter(|track| track.detection().class() == Some(class))
        .cloned()
        .collect();

    let pushes = pushes_of(&subscription, &first, whole);
    assert_eq!(pushes, pushes_of(&subscription, &first, interactive));
    let once: BTreeSet<_> = pushes.iter().collect();
    assert_eq!(once.len(), pushes.len(), "each push once: {pushes:?}");
    assert!(
        pushes
            .iter()
            .any(|&(_, event, crossing)| event == supernova.event() && crossing == Crossing::Detect),
        "the supernova is detected: {pushes:?}"
    );
    server.shutdown().await.unwrap();
}

/// A class whose cells within its reach are more than a walk may read is not planned; with room,
/// it is.
#[test]
fn a_class_whose_cells_are_too_many_is_not_planned() {
    let supernova = pinned();
    let far = beside(&supernova, 0.2);
    let (subscription, first) = core_collapses(far, UniverseTime::EPOCH, reaching(700.0));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    let planned = background_cells(galaxy(), &spec, class, MAX_BACKGROUND_CELLS).unwrap();
    assert_eq!(
        background_cells(galaxy(), &spec, class, planned.len() / 2),
        None
    );
    assert_eq!(
        background_cells(galaxy(), &spec, class, planned.len()),
        Some(planned)
    );
}
