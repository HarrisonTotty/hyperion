//! The scans' walks (plan 12, P12.T8.b and P12.T8.b.2): how far a first scan walks back, what keys
//! a walk, a background walk in jobs of x-slabs, which gives the interactive walk's pushes, each
//! once, and a whole galaxy's plan, which lists no cell.

use std::collections::BTreeSet;

use hyperion_sim::alerts::{
    AlertBand, BandLimits, ClassSet, LocalEventsCache, LongestRise, SensorHorizon, TransientKind,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::{ClassId, cells_in_sphere, occupied_cells};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::id::EventId;
use hyperion_sim::observe::Observer;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::WattsPerSquareMetre;

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

/// A background walk of the core collapses in jobs gives the whole's pushes, each once, in jobs of
/// one slab, of three and of all, and they are the interactive walk's: 0.2 ly from the pinned
/// supernova, through a V sensor that reaches some 700 ly, over the first scan's year and its walk
/// back. The jobs draw every candidate of the sphere's cells.
#[tokio::test]
async fn a_background_walk_in_slab_jobs_gives_the_interactive_walks_pushes_each_once() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let supernova = pinned();
    let far = beside(&supernova, 0.2);
    let start = after(arrival(&supernova, &far), -DAY);
    let (subscription, first) = core_collapses(far, start, reaching(700.0));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    let token = CancelToken::new();
    let walk = |per_job| {
        let plan = background_plan(galaxy(), &spec, class, per_job).unwrap();
        let jobs = plan.jobs().to_vec();
        let walked = jobs.into_iter().map(|slabs| {
            walk_slabs(state, galaxy(), &spec, &plan, slabs, &token).expect("not cancelled")
        });
        (
            plan.jobs().len(),
            ScanResult::of_jobs(walked.collect::<Vec<_>>()),
        )
    };
    let (slabs, whole) = walk(usize::MAX);
    assert_eq!(slabs, 1, "one job of every slab");
    let plan = background_plan(galaxy(), &spec, class, 1).unwrap();
    assert!(
        plan.jobs().len() > 10,
        "a few dozen slabs: {:?}",
        plan.jobs()
    );
    let sphere = plan
        .sphere
        .as_ref()
        .expect("a sensor that sees the class has a sphere");
    let drawn: u64 = occupied_cells(
        galaxy(),
        &CoreCollapseProcess::new(galaxy()),
        cells_in_sphere(class, sphere),
    )
    .iter()
    .map(|&(_, count)| u64::from(count))
    .sum();
    assert_eq!(whole.drawn(), Some(drawn));
    for per_job in [1, 3] {
        let (_, jobs) = walk(per_job);
        assert_eq!(jobs, whole, "in jobs of {per_job} slabs");
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

    let pushes = pushes_of(&subscription, &first, whole.tracks().to_vec());
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

/// A sensor that sees across the galaxy, V 10⁻¹² W m⁻², plans its core collapses as the root
/// cube's 256 x-slabs of 512 ly cells, in order and without a gap: 64 jobs of 4 slabs, or at most
/// 256 jobs however few slabs a job is asked to take. The plan holds the slabs, never a list of the
/// cube's 2²⁴ cell keys. A class no background walk reads in cells has no plan.
#[test]
fn a_whole_galaxy_plan_is_the_cubes_slabs_and_no_list_of_its_keys() {
    let (subscription, first) = core_collapses(sun(), UniverseTime::EPOCH, v_sensor(1e-12, 1e-10));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    for (per_job, jobs) in [(SLABS_PER_JOB, 64), (1, 256), (0, 256)] {
        let plan = background_plan(galaxy(), &spec, class, per_job).unwrap();
        assert_eq!(plan.jobs().len(), jobs, "{per_job} slabs a job");
        assert_eq!(plan.jobs().first().map(|slabs| slabs.start), Some(-128));
        assert_eq!(plan.jobs().last().map(|slabs| slabs.end), Some(128));
        assert!(
            plan.jobs()
                .windows(2)
                .all(|pair| pair[0].end == pair[1].start),
            "{:?}",
            plan.jobs()
        );
    }
    assert!(background_plan(galaxy(), &spec, ClassId::LUMINOUS_BLUE_VARIABLE, 1).is_none());
}

/// A class the sensor cannot see, the core collapses through a radio sensor, in which no supernova
/// template gives light: its plan has no job, and its walk draws nothing.
#[test]
fn a_class_the_sensor_cannot_see_plans_no_job_and_draws_nothing() {
    let radio = SensorHorizon::new([(
        AlertBand::Photometric(Band::Radio),
        BandLimits::new(
            WattsPerSquareMetre::new(1e-20),
            WattsPerSquareMetre::new(1e-18),
        )
        .unwrap(),
    )])
    .unwrap();
    let (subscription, first) = core_collapses(sun(), UniverseTime::EPOCH, radio);
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let plan = background_plan(galaxy(), &spec, ClassId::CORE_COLLAPSE, SLABS_PER_JOB).unwrap();
    assert_eq!(plan.jobs(), []);
    assert_eq!(ScanResult::of_jobs([]).drawn(), Some(0));
}

/// A merger class, whose candidates read no time first, walked in jobs of one slab gives its
/// interactive walk's tracks: the stellar mergers through a V sensor reaching some 700 ly of the
/// core collapses' reach, from the Sun-like point over the first scan's year.
#[tokio::test]
async fn a_merger_class_in_slab_jobs_gives_its_interactive_walks_tracks() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let class = ClassId::STELLAR_MERGER;
    let (subscription, first) = Subscription::new(
        Observer::new(sun(), UniverseTime::EPOCH).unwrap(),
        reaching(700.0),
        ClassSet::of(class),
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    );
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let plan = background_plan(galaxy(), &spec, class, 1).unwrap();
    assert_ne!(plan.jobs(), []);
    let token = CancelToken::new();
    let jobs: Vec<_> = plan
        .jobs()
        .iter()
        .cloned()
        .map(|slabs| walk_slabs(state, galaxy(), &spec, &plan, slabs, &token).unwrap())
        .collect();
    let walked = ScanResult::of_jobs(jobs);
    assert!(
        walked.drawn().is_some_and(|drawn| drawn > 0),
        "{:?}",
        walked.drawn()
    );
    let interactive = walk_interactive(state, galaxy(), &spec, &mut LocalEventsCache::default());
    let interactive: Vec<_> = interactive
        .tracks()
        .iter()
        .filter(|track| track.detection().class() == Some(class))
        .cloned()
        .collect();
    let events = |tracks: &[Tracked]| {
        let mut events: Vec<_> = tracks
            .iter()
            .map(|track| track.detection().event())
            .collect();
        events.sort_unstable();
        events
    };
    assert_eq!(events(walked.tracks()), events(&interactive));
    server.shutdown().await.unwrap();
}
