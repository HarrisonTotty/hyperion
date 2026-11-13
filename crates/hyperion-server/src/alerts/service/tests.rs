//! The service's walks (plan 12, P12.T8.b): two subscriptions with one key share one walk, the
//! walks are kept within their budget, and a background walk given up keeps nothing and runs no
//! more of its jobs.

use std::time::Duration;

use hyperion_sim::alerts::{AlertBand, BandLimits, ClassSet, SensorHorizon};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::observe::Observer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::WattsPerSquareMetre;
use tokio::time::timeout;

use super::*;
use crate::alerts::Subscription;
use crate::alerts::scan::{ScanSpec, background_plan};
use crate::alerts::testing::{
    DAY, after, arrival, beside, core_collapses, galaxy, key, pinned, reaching, server,
};
use crate::cache::ByteLru;
use crate::compute::BulkGate;
use crate::testing::WAIT;

fn sun() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap()
}

/// An X-ray sensor, which no local event lights: a walk of no class costs its range query alone.
fn xray(detect: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(detect * 100.0)).unwrap();
    SensorHorizon::new([(AlertBand::XRay, limits)]).unwrap()
}

/// The first scan of a subscription of `sensor` to no class at the Sun at `t`.
fn nothing_at(t: UniverseTime, sensor: SensorHorizon) -> ScanSpec {
    let (subscription, first) = Subscription::new(
        Observer::new(sun(), t).unwrap(),
        sensor,
        ClassSet::default(),
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    );
    ScanSpec::new(key(), &subscription, &first).unwrap()
}

/// Waits until `done`, looking every 5 ms, for up to [`WAIT`].
async fn until(what: &str, done: impl Fn() -> bool) {
    timeout(WAIT, async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
}

fn cache() -> Arc<Mutex<LocalEventsCache>> {
    Arc::new(Mutex::new(LocalEventsCache::default()))
}

/// Two subscriptions alike, each with its own local events cache, that ask at once share one walk,
/// and one that asks after it is served the walk kept; one with another sensor walks its own.
#[tokio::test]
async fn two_subscriptions_with_one_key_share_one_walk() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let scans = &state.alert_scans;
    let spec = || nothing_at(UniverseTime::EPOCH, xray(1e-12));
    let (one, other) = (spec(), spec());
    // Each joins at the call, before either is awaited.
    let first = scans.interactive(state, galaxy(), &one, &cache());
    let second = scans.interactive(state, galaxy(), &other, &cache());
    let (first, second) = futures_util::join!(first, second);
    let (first, second) = (first.unwrap(), second.unwrap());
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(scans.walked(), 1);
    let later = scans
        .interactive(state, galaxy(), &spec(), &cache())
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&later, &first));
    assert_eq!((scans.walked(), scans.counters().hits()), (1, 1));
    let fainter = scans
        .interactive(
            state,
            galaxy(),
            &nothing_at(UniverseTime::EPOCH, xray(1e-13)),
            &cache(),
        )
        .await
        .unwrap();
    assert!(!Arc::ptr_eq(&fainter, &first));
    assert_eq!(scans.walked(), 2);
    server.shutdown().await.unwrap();
}

/// Walks past the budget evict the least recently used: a budget of three empty walks holds
/// three of six, and is never passed.
#[tokio::test]
async fn the_walks_are_kept_within_their_budget() {
    let empty = ByteLru::<WalkKey, ScanResult>::charge(&ScanResult::of_pieces(
        &BackgroundWork::new(Vec::new()),
        [],
    ));
    let budget = 3 * empty;
    let (server, _data_dir) = server(|config| config.alert_cache_bytes(budget)).await;
    let state = server.state();
    let scans = &state.alert_scans;
    for days in 0..6 {
        let spec = nothing_at(after(UniverseTime::EPOCH, days * DAY), xray(1e-12));
        let walked = scans
            .interactive(state, galaxy(), &spec, &cache())
            .await
            .unwrap();
        assert_eq!(ByteLru::<WalkKey, ScanResult>::charge(&walked), empty);
        assert!(scans.counters().bytes() <= budget, "{:?}", scans.counters());
    }
    let counters = scans.counters();
    assert_eq!((counters.entries(), counters.evictions()), (3, 3));
    assert_eq!(scans.walked(), 6);
    server.shutdown().await.unwrap();
}

/// A background walk whose waiter gives up while its plan is held at the gate stores nothing and
/// runs nothing: let through, the plan is skipped as cancelled, and none of its slabs' jobs is
/// queued. Asked again, the walk runs whole and finds the pinned supernova, through a sensor that
/// reaches some 700 ly, in jobs of one slab.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_background_walk_given_up_keeps_nothing_and_runs_nothing() {
    let gate = BulkGate::default();
    gate.close();
    let (server, _data_dir) = server(|config| config.bulk_gate(gate.clone())).await;
    let state = server.state();
    let scans = AlertScans::with_slabs_per_job(1 << 20, 1);
    let supernova = pinned();
    let far = beside(&supernova, 0.2);
    let (subscription, first) =
        core_collapses(far, after(arrival(&supernova, &far), -DAY), reaching(700.0));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    let pool = || state.pool.counters();
    let given_up = tokio::spawn(scans.background(state, galaxy(), &spec, class));
    until("the plan to reach the gate", || gate.held() == 1).await;
    let before = pool();
    given_up.abort();
    assert!(given_up.await.unwrap_err().is_cancelled());
    gate.open();
    until("the plan to be skipped", || {
        pool().cancelled() > before.cancelled() && pool().running() == 0
    })
    .await;
    let skipped = pool();
    assert_eq!(skipped.cancelled(), before.cancelled() + 1, "{skipped:?}");
    assert_eq!(skipped.completed(), before.completed(), "{skipped:?}");
    assert_eq!(skipped.queued_bulk(), 0, "{skipped:?}");
    assert_eq!((scans.counters().entries(), scans.walked()), (0, 0));

    let walked = scans
        .background(state, galaxy(), &spec, class)
        .await
        .unwrap();
    assert_eq!(scans.walked(), 1);
    assert!(
        walked.drawn().is_some_and(|drawn| drawn > 0),
        "{:?}",
        walked.drawn()
    );
    assert!(
        walked
            .tracks()
            .iter()
            .any(|track| track.detection().event() == supernova.event()),
        "{} tracks",
        walked.tracks().len()
    );
    assert!(
        pool().completed() > skipped.completed() + 2,
        "a plan and its slabs' jobs: {:?}",
        pool()
    );
    server.shutdown().await.unwrap();
}

/// A background walk whose waiter gives up once its plan has run, its slabs' census jobs queued
/// and two of them held at the gate by the pool's two workers, stores nothing and counts no slab:
/// let through, every one of its jobs is skipped as cancelled, and only the plan has run.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_background_walk_given_up_among_its_jobs_walks_no_slab() {
    let gate = BulkGate::default();
    gate.close();
    let (server, _data_dir) = server(|config| config.bulk_gate(gate.clone())).await;
    let state = server.state();
    let scans = AlertScans::with_slabs_per_job(1 << 20, 1);
    let supernova = pinned();
    let far = beside(&supernova, 0.2);
    let (subscription, first) =
        core_collapses(far, after(arrival(&supernova, &far), -DAY), reaching(700.0));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let pool = || state.pool.counters();
    let start = pool();
    let given_up = tokio::spawn(scans.background(state, galaxy(), &spec, ClassId::CORE_COLLAPSE));
    until("the plan to reach the gate", || gate.held() == 1).await;
    gate.admit();
    until("two slabs' jobs to be held and more queued", || {
        gate.held() == 2 && pool().queued_bulk() > 0 && pool().completed() == start.completed() + 1
    })
    .await;
    given_up.abort();
    assert!(given_up.await.unwrap_err().is_cancelled());
    gate.open();
    until("the slabs' jobs to be skipped", || {
        pool().running() == 0 && pool().queued_bulk() == 0
    })
    .await;
    let after_all = pool();
    assert_eq!(
        after_all.completed(),
        start.completed() + 1,
        "the plan alone ran: {after_all:?}"
    );
    // The two held and every one queued behind them: the walk may queue a few more before its
    // waiter's departure cancels it.
    assert!(
        after_all.cancelled() > start.cancelled() + 2,
        "every slab's job was skipped: {after_all:?}"
    );
    assert_eq!(scans.counters().entries(), 0);
    server.shutdown().await.unwrap();
}

/// The background walk of a class no background walk reads in cells, the centre's victims, which
/// are walked through the global list, draws nothing and is kept as such: its census then reads
/// `unwalked`, never `complete`.
#[tokio::test]
async fn a_background_walk_of_a_class_read_in_no_cells_draws_nothing() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let spec = nothing_at(UniverseTime::EPOCH, xray(1e-12));
    let walked = state
        .alert_scans
        .background(state, galaxy(), &spec, ClassId::TIDAL_DISRUPTION_VICTIM)
        .await
        .unwrap();
    assert_eq!((walked.drawn(), walked.tracks().len()), (None, 0));
    server.shutdown().await.unwrap();
}

/// A background walk whose waiter gives up once its census has run and two of its walk's jobs are
/// held at the gate by the pool's two workers stores nothing and walks no piece: let through, the
/// two are skipped as cancelled, no more is queued, and only the plan and the census have run.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_background_walk_given_up_among_its_pieces_walks_none() {
    let gate = BulkGate::default();
    gate.close();
    let (server, _data_dir) = server(|config| config.bulk_gate(gate.clone())).await;
    let state = server.state();
    let scans = AlertScans::with_slabs_per_job(1 << 20, 1);
    let supernova = pinned();
    let far = beside(&supernova, 0.2);
    let (subscription, first) =
        core_collapses(far, after(arrival(&supernova, &far), -DAY), reaching(700.0));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    let census = background_plan(galaxy(), &spec, class, 1)
        .unwrap()
        .jobs()
        .len();
    let pool = || state.pool.counters();
    let start = pool();
    let given_up = tokio::spawn(scans.background(state, galaxy(), &spec, class));
    // The plan, then each census job: the walk's jobs are queued only once the census is whole.
    for admitted in 0..=census {
        until("the plan or a census job at the gate", || gate.held() > 0).await;
        gate.admit();
        until("it to run", || {
            pool().completed() == start.completed() + u64::try_from(admitted).unwrap() + 1
        })
        .await;
    }
    until("two of the walk's jobs to be held", || gate.held() == 2).await;
    given_up.abort();
    assert!(given_up.await.unwrap_err().is_cancelled());
    gate.open();
    until("the walk's jobs to be skipped", || {
        pool().running() == 0 && pool().queued_bulk() == 0
    })
    .await;
    let after_all = pool();
    assert_eq!(
        after_all.completed(),
        start.completed() + 1 + u64::try_from(census).unwrap(),
        "the plan and the census alone ran: {after_all:?}"
    );
    assert_eq!(
        after_all.cancelled(),
        start.cancelled() + 2,
        "the two held: {after_all:?}"
    );
    assert_eq!(scans.counters().entries(), 0);
    server.shutdown().await.unwrap();
}

/// A background walk is the same whatever its jobs' budget: the core collapses 0.2 ly from the
/// pinned supernova, through a sensor that reaches some 700 ly, walked by jobs that take every
/// piece they can, two at once on the pool's two workers; by jobs of one piece each, a job for
/// each piece; and by the service's jobs of about a second each. All find the supernova.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_background_walk_is_the_same_whatever_its_jobs_budget() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let supernova = pinned();
    let far = beside(&supernova, 0.2);
    let (subscription, first) =
        core_collapses(far, after(arrival(&supernova, &far), -DAY), reaching(700.0));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    let census = background_plan(galaxy(), &spec, class, 1)
        .unwrap()
        .jobs()
        .len();
    let completed = || state.pool.counters().completed();
    // The walk's jobs, less its plan and census.
    let walk = |budget| {
        let spec = spec.clone();
        async move {
            let before = completed();
            let scans = AlertScans::new(1 << 20).with_job_budget(budget);
            let walked = timeout(WAIT, scans.background(state, galaxy(), &spec, class))
                .await
                .expect("the walk finishes")
                .unwrap();
            let jobs = completed() - before - 1 - u64::try_from(census).unwrap();
            (walked, jobs)
        }
    };
    let (whole, two) = walk(Duration::MAX).await;
    assert_eq!(two, 2, "two jobs that take every piece");
    let (one_each, pieces) = walk(Duration::ZERO).await;
    assert_eq!(*one_each, *whole);
    assert!(pieces > 10, "a job a piece: {pieces} jobs");
    let (budgeted, _) = walk(JOB_BUDGET).await;
    assert_eq!(*budgeted, *whole);
    assert!(
        whole
            .tracks()
            .iter()
            .any(|track| track.detection().event() == supernova.event()),
        "{} tracks",
        whole.tracks().len()
    );
    server.shutdown().await.unwrap();
}
