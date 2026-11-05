//! The scans' walks (plan 12, P12.T8.b and P12.T8.b.2): how far a first scan walks back, what keys
//! a walk, a background walk counted in jobs of x-slabs and walked in pieces, which gives the
//! interactive walk's pushes, each once, a merger class's cells split by candidate index, which
//! give the whole cells' tracks in their order, and a whole galaxy's plan, which lists no cell.

use std::collections::BTreeSet;
use std::time::Duration;

use hyperion_sim::alerts::{
    AlertBand, BandLimits, ClassSet, LocalEventsCache, LongestRise, OneShotClass, OneShotEvent,
    SensorHorizon, TransientKind,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::catalogue_classes::binary::StellarMergers;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, ClassId, cells_in_sphere, occupied_cells,
};
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

/// The background walk of `plan` for `spec` in `galaxy`, its jobs run here in turn: its census
/// jobs' slabs, a merger class's cells in `parts`, and its pieces by jobs of `budget` each.
fn walked(
    state: &AppState,
    galaxy: &Galaxy,
    spec: &ScanSpec,
    plan: &BackgroundPlan,
    parts: CellParts,
    budget: Duration,
) -> ScanResult {
    let token = CancelToken::new();
    let census: Vec<_> = plan
        .jobs()
        .iter()
        .cloned()
        .map(|slabs| census_slabs(galaxy, plan, slabs, parts, &token).expect("not cancelled"))
        .collect();
    let work = BackgroundWork::new(census);
    let mut jobs = Vec::new();
    while !work.is_taken() {
        let job = walk_pieces(state, galaxy, spec, plan, &work, budget, &token);
        jobs.push(job.expect("not cancelled"));
    }
    ScanResult::of_pieces(&work, jobs)
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

/// A background walk of the core collapses in jobs gives the whole's pushes, each once, counted in
/// jobs of one slab, of three and of all, and walked a piece a job or in the service's budget, and
/// they are the interactive walk's: 0.2 ly from the pinned supernova, through a V sensor that
/// reaches some 700 ly, over the first scan's year and its walk back. The jobs draw every candidate
/// of the sphere's cells.
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
    let walk = |per_job, budget| {
        let plan = background_plan(galaxy(), &spec, class, per_job).unwrap();
        let walked = walked(state, galaxy(), &spec, &plan, CellParts::SERVICE, budget);
        (plan.jobs().len(), walked)
    };
    let (slabs, whole) = walk(usize::MAX, JOB_BUDGET);
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
    for (per_job, budget) in [
        (1, JOB_BUDGET),
        (3, Duration::ZERO),
        (usize::MAX, Duration::ZERO),
    ] {
        let (_, jobs) = walk(per_job, budget);
        assert_eq!(jobs, whole, "in jobs of {per_job} slabs, {budget:?}");
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

/// Seed 9's galaxy at the generator version, whose core collapses' walks read the features' lists
/// (the sim's `listed`), with its kinematic tables, as the server's galaxies are built.
fn seed_9_galaxy() -> &'static Galaxy {
    static GALAXY: std::sync::OnceLock<Galaxy> = std::sync::OnceLock::new();
    GALAXY.get_or_init(|| {
        let seed = hyperion_sim::Seed::new(crate::alerts::testing::SEED);
        Galaxy::from_params_at_version(
            seed,
            hyperion_sim::galaxy::params::GalaxyParams::from_seed(
                seed,
                hyperion_sim::galaxy::imf::MassFunctionKind::default(),
            ),
            hyperion_sim::GENERATOR_VERSION,
        )
        .unwrap()
        .with_full_potential()
    })
}

/// From generator version 29 a background walk of the core collapses in slab jobs gives the
/// interactive walk's pushes with the features' lists' (the sim's `listed`), each once: 0.2 ly
/// spinward of a nursery's supernova that goes off 234.8 years after the epoch at
/// [−12,727.7, 26,883.1, 204.0] ly in seed 9's galaxy, through a V sensor that reaches some
/// 700 ly, over the first scan's year and its walk back. Its member's onset is detected, and the
/// jobs draw the lists' candidates beside the cells'.
#[tokio::test]
async fn from_29_a_background_walk_gives_the_interactive_walks_listed_pushes() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let galaxy = seed_9_galaxy();
    let key = GalaxyKey::new(
        crate::alerts::testing::SEED,
        hyperion_sim::GENERATOR_VERSION,
    );
    let host = hyperion_sim::id::SystemId::from_raw(0xe65a_0018_f000_0003).unwrap();
    let far = GalacticPosition::from_light_years([-12_727.473, 26_883.128, 204.046]).unwrap();
    // Its light arrives 234.997 years after the epoch, inside the first scan's year.
    let start = UniverseTime::from_julian_years(234).unwrap();
    let (subscription, first) = core_collapses(far, start, reaching(700.0));
    let spec = ScanSpec::new(key, &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    let plan = background_plan(galaxy, &spec, class, 1).unwrap();
    let whole = walked(state, galaxy, &spec, &plan, CellParts::SERVICE, JOB_BUDGET);
    let sphere = plan.sphere.as_ref().expect("the sensor sees the class");
    let cells: u64 = occupied_cells(
        galaxy,
        &CoreCollapseProcess::new(galaxy),
        cells_in_sphere(class, sphere),
    )
    .iter()
    .map(|&(_, count)| u64::from(count))
    .sum();
    assert!(
        whole.drawn().is_some_and(|drawn| drawn > cells),
        "the lists' candidates beside the cells' {cells}: {:?}",
        whole.drawn()
    );
    let interactive = walk_interactive(state, galaxy, &spec, &mut LocalEventsCache::default());
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
    let listed = whole
        .tracks()
        .iter()
        .find(|track| track.detection().host() == host)
        .expect("the listed supernova is tracked");
    let event = listed.detection().event();
    assert!(
        pushes
            .iter()
            .any(|&(_, e, crossing)| e == event && crossing == Crossing::Detect),
        "the listed supernova is detected: {pushes:?}"
    );
    server.shutdown().await.unwrap();
}

/// From generator version 29 a background walk of the slow accreting white dwarfs in slab jobs gives
/// the interactive walk's pushes with the features' lists' (the sim's `listed`), each once, and
/// reads its lists from the server's listed hosts, which a second walk finds them in: 1 ly from a
/// globular's slow nova that begins 764.6 years after the epoch at [−9,792.8, −5,763.1,
/// −17,139.2] ly in seed 9's galaxy, through a V sensor through which the novae reach 50 ly,
/// over the first scan's year. Its member's onset is detected.
#[tokio::test]
async fn from_29_a_background_walk_of_the_slow_novae_gives_the_interactive_walks_listed_pushes() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let galaxy = seed_9_galaxy();
    let key = GalaxyKey::new(
        crate::alerts::testing::SEED,
        hyperion_sim::GENERATOR_VERSION,
    );
    let host = hyperion_sim::id::SystemId::from_raw(0xe6b9_6000_7000_0057).unwrap();
    let at = GalacticPosition::from_light_years([-9_792.817, -5_763.148, -17_138.163]).unwrap();
    // Its light arrives 765.574 years after the epoch, inside the first scan's year.
    let start = UniverseTime::from_julian_years(765).unwrap();
    let probe = 1e-6;
    let reach = kind_reach(TransientKind::Nova, &v_sensor(probe, probe)).value();
    let detect = probe * (reach / 50.0) * (reach / 50.0);
    let class = ClassId::ACCRETING_WHITE_DWARF_SLOW;
    let (subscription, first) = Subscription::new(
        Observer::new(at, start).unwrap(),
        v_sensor(detect, detect * 10.0),
        ClassSet::of(class),
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    );
    let spec = ScanSpec::new(key, &subscription, &first).unwrap();
    let plan = background_plan(galaxy, &spec, class, 1).unwrap();
    let walk = || walked(state, galaxy, &spec, &plan, CellParts::SERVICE, JOB_BUDGET);
    let whole = walk();
    let listed = state.listed_hosts.counters();
    assert!(listed.entries() > 0, "the walk keeps its lists: {listed:?}");
    assert_eq!(walk(), whole, "a second walk of the kept lists");
    assert_eq!(
        state.listed_hosts.counters().misses(),
        listed.misses(),
        "the second walk builds no list"
    );
    let interactive = walk_interactive(state, galaxy, &spec, &mut LocalEventsCache::default());
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
    let nova = whole
        .tracks()
        .iter()
        .find(|track| track.detection().host() == host)
        .expect("the listed nova is tracked");
    let event = nova.detection().event();
    assert!(
        pushes
            .iter()
            .any(|&(_, e, crossing)| e == event && crossing == Crossing::Detect),
        "the listed nova is detected: {pushes:?}"
    );
    server.shutdown().await.unwrap();
}

/// A sensor that sees across the galaxy, V 10⁻¹² W m⁻², plans its core collapses as the root
/// cube's 256 x-slabs of 512 ly cells, in order and without a gap: 256 jobs of the service's one
/// slab, 64 of four, and at most 256 however few slabs a job is asked to take. The plan holds the slabs, never a list of the
/// cube's 2²⁴ cell keys. A class no background walk reads in cells has no plan.
#[test]
fn a_whole_galaxy_plan_is_the_cubes_slabs_and_no_list_of_its_keys() {
    let (subscription, first) = core_collapses(sun(), UniverseTime::EPOCH, v_sensor(1e-12, 1e-10));
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let class = ClassId::CORE_COLLAPSE;
    for (per_job, jobs) in [(SLABS_PER_JOB, 256), (4, 64), (0, 256)] {
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
    assert!(background_plan(galaxy(), &spec, ClassId::TIDAL_DISRUPTION_VICTIM, 1).is_none());
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
    let work = BackgroundWork::new(Vec::new());
    assert!(work.is_taken(), "no piece to take");
    assert_eq!(ScanResult::of_pieces(&work, []).drawn(), Some(0));
}

/// Seed 9's red nova of stellar merger cell [−2, 7, −1], one of its 231 candidates, which goes off
/// 727.1 years after the epoch at [−6,820.3, 29,961.6, −858.1] ly, out by the solar circle.
fn red_nova() -> OneShotEvent {
    let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [-2, 7, -1]).unwrap();
    let host = hyperion_sim::id::SystemId::from_raw(0xfc27_0b87_8000_0040).unwrap();
    StellarMergers::new(galaxy())
        .cell_events(galaxy(), key)
        .into_iter()
        .find(|event| event.host() == host)
        .expect("the cell's merger makes its red nova")
}

/// A merger class, whose hosts move from their cells to their mergers and whose merger times are
/// read first and cut on that drift (perf-alert-walks-cpu), walked in pieces gives its interactive
/// walk's tracks, and its cells split by candidate index among one-piece jobs give the whole
/// cells' tracks in their order, each to the bit (plan 12, the mergers' balance): the stellar
/// mergers 1 ly from the pinned red nova, through a V sensor through which red novae reach some
/// 400 ly, over the first scan's year. The red nova is detected.
#[tokio::test]
async fn a_merger_class_in_pieces_gives_its_interactive_walks_tracks_and_its_whole_cells() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let class = ClassId::STELLAR_MERGER;
    let nova = red_nova();
    let far = beside(&nova, 1.0);
    let (subscription, first) = Subscription::new(
        Observer::new(far, after(arrival(&nova, &far), -DAY)).unwrap(),
        reaching(700.0),
        ClassSet::of(class),
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    );
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let plan = background_plan(galaxy(), &spec, class, 1).unwrap();
    assert_ne!(plan.jobs(), []);
    let whole = walked(
        state,
        galaxy(),
        &spec,
        &plan,
        CellParts::new(u32::MAX, 1),
        JOB_BUDGET,
    );
    let split = walked(
        state,
        galaxy(),
        &spec,
        &plan,
        CellParts::new(1, 3),
        Duration::ZERO,
    );
    assert_eq!(split, whole, "split three ways, a piece a job");
    assert!(
        whole.drawn().is_some_and(|drawn| drawn > 0),
        "{:?}",
        whole.drawn()
    );
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
    let events = |tracks: &[Tracked]| {
        let mut events: Vec<_> = tracks
            .iter()
            .map(|track| track.detection().event())
            .collect();
        events.sort_unstable();
        events
    };
    assert_eq!(events(whole.tracks()), events(&interactive));
    assert!(
        events(whole.tracks()).contains(&nova.event()),
        "the red nova is detected: {:?}",
        events(whole.tracks())
    );
    server.shutdown().await.unwrap();
}

/// A cell's parts cover its candidates in order, each of at least the fewest a part but in a cell
/// of fewer, and at most the most: the service's split one of the eight central cells' 12.15
/// million candidates into 64, an inner disc cell's 10,377 into 20, and keep 1,023 whole.
#[test]
fn a_cells_parts_cover_its_candidates_in_order() {
    for (parts, count, expected) in [
        (CellParts::SERVICE, 12_153_859, 64),
        (CellParts::SERVICE, 10_377, 20),
        (CellParts::SERVICE, 2_028, 3),
        (CellParts::SERVICE, 1_024, 2),
        (CellParts::SERVICE, 1_023, 1),
        (CellParts::SERVICE, 1, 1),
        (CellParts::new(1, 3), 2, 2),
        (CellParts::new(1, 3), 231, 3),
        (CellParts::new(u32::MAX, 1), 12_153_859, 1),
        (CellParts::new(0, 0), 5, 1),
    ] {
        let of: Vec<_> = parts.of(count).collect();
        assert_eq!(of.len(), expected, "{parts:?} of {count}: {of:?}");
        assert_eq!(of.first().map(|part| part.start), Some(0), "{of:?}");
        assert_eq!(of.last().map(|part| part.end), Some(count), "{of:?}");
        assert!(
            of.windows(2).all(|pair| pair[0].end == pair[1].start),
            "{of:?}"
        );
        let least = if expected > 1 { parts.fewest } else { 1 };
        assert!(
            of.iter().all(|part| part.end - part.start >= least),
            "{of:?}"
        );
    }
}

/// The pieces' items in the walk's order: each piece's in turn, the parts of each split cell
/// merged by their key with a stable sort, ties in the parts' order, and a cell walked whole or a
/// slab's lists kept in their own order, as the core collapses' supernovae and then their
/// magnetars.
#[test]
fn the_parts_of_a_split_cell_are_merged_by_arrival_and_event_and_the_rest_kept() {
    let cell = |x| CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, 0, 0]).unwrap();
    let part = |x, indices| Piece::Part {
        cell: cell(x),
        indices,
    };
    let pieces = [
        part(0, 0..2),
        part(0, 2..5),
        part(0, 5..6),
        Piece::Listed(0),
        Piece::Cell(cell(1)),
        part(2, 0..1),
        part(2, 1..2),
        Piece::Listed(1),
        Piece::Cell(cell(3)),
    ];
    // Each part sorted by its key, as a part's walk sorts its detections.
    let walked = [
        vec![(2, 'a'), (5, 'b')],
        vec![(1, 'c'), (5, 'd'), (7, 'e')],
        vec![(0, 'f')],
        vec![(9, 'g'), (3, 'h')],
        vec![(8, 'i'), (2, 'j')],
        vec![(4, 'k')],
        vec![(1, 'l'), (4, 'm')],
        vec![],
        vec![(6, 'n'), (0, 'o')],
    ];
    let merged = in_walk_order(pieces.iter().zip(walked.clone()), |&(key, _)| key);
    let labels: String = merged.iter().map(|&(_, label)| label).collect();
    assert_eq!(labels, "fcabdeghijlkmno");
    // The same with the pieces that found nothing left out.
    let found = pieces
        .iter()
        .zip(walked)
        .filter(|(_, items)| !items.is_empty());
    let merged = in_walk_order(found, |&(key, _)| key);
    assert_eq!(
        merged.iter().map(|&(_, label)| label).collect::<String>(),
        labels
    );
}

/// Every class `ProductionAlertClasses` walks has a background plan, so that none its census sends
/// to the background walks nothing and reads `complete` (P12.T8.b.2, after P12.T5.c): the one-shot
/// classes and the recurrent alike, all but the centre's victims, which are walked through the
/// global list. And the X-ray binaries through an X-ray sensor reaching 30,000 ly, the socket
/// test's, are sent there.
#[tokio::test]
async fn every_class_the_census_walks_has_a_background_plan() {
    let (server, _data_dir) = server(|config| config).await;
    let state = server.state();
    let w = WattsPerSquareMetre::new;
    let sensor = SensorHorizon::new([
        (
            AlertBand::Photometric(Band::V),
            BandLimits::new(w(1e-12), w(1e-10)).unwrap(),
        ),
        (
            AlertBand::XRay,
            BandLimits::new(w(1e-12), w(1e-10)).unwrap(),
        ),
    ])
    .unwrap();
    let (subscription, first) = Subscription::new(
        Observer::new(sun(), UniverseTime::EPOCH).unwrap(),
        sensor,
        ClassSet::ALL,
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    );
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    for class in ClassSet::ALL.iter() {
        let plan = background_plan(galaxy(), &spec, class, SLABS_PER_JOB);
        assert_eq!(
            plan.is_some(),
            class != ClassId::TIDAL_DISRUPTION_VICTIM,
            "{class}'s background plan"
        );
    }

    let probe = SensorHorizon::new([(AlertBand::XRay, BandLimits::new(w(1e-6), w(1e-6)).unwrap())])
        .unwrap();
    let ratio = class_reach(ClassId::XRAY_BINARY, &probe).value() / 30_000.0;
    let detect = 1e-6 * ratio * ratio;
    let xray = SensorHorizon::new([(
        AlertBand::XRay,
        BandLimits::new(w(detect), w(detect * 100.0)).unwrap(),
    )])
    .unwrap();
    let (subscription, first) = Subscription::new(
        Observer::new(sun(), UniverseTime::EPOCH).unwrap(),
        xray,
        ClassSet::of(ClassId::XRAY_BINARY),
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    );
    let spec = ScanSpec::new(key(), &subscription, &first).unwrap();
    let interactive = walk_interactive(state, galaxy(), &spec, &mut LocalEventsCache::default());
    assert_eq!(
        interactive.background().collect::<Vec<_>>(),
        [ClassId::XRAY_BINARY],
        "{:?}",
        interactive.census()
    );
    server.shutdown().await.unwrap();
}

/// The process's CPU time so far, s: user and system, from `/proc/self/stat` at Linux's 100 clock
/// ticks a second; `None` where there is none.
fn cpu_seconds() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // The fields after the command's closing parenthesis, from the state (field 3): utime and
    // stime are fields 14 and 15.
    let fields: Vec<&str> = stat.get(stat.rfind(')')? + 2..)?.split(' ').collect();
    let ticks = |field: usize| fields.get(field - 3)?.parse::<f64>().ok();
    Some((ticks(14)? + ticks(15)?) / 100.0)
}

/// FNV-1a of `bytes`, 64-bit: a digest the same on any toolchain, to compare two trees' walks.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Times a class's background walk of the whole galaxy through the service, cold and then warm:
/// V 10⁻¹² W m⁻² over the first 1,000 years from the Sun-like point, in the Milky Way-like galaxy
/// of seed `0x0926_0000`, on `HYPERION_WALK_WORKERS` workers (16 if unset), of the class
/// `HYPERION_WALK_CLASS` names (`STELLAR_MERGER` if unset, or `CORE_COLLAPSE`, ...) at generator
/// version `HYPERION_WALK_VERSION` (the tree's if unset; 29 reads the features' lists). The cold
/// walk is a fresh server's; the warm one another service's on the same server, its feature cells
/// and lists kept. Each prints its wall and CPU seconds, the tracks, the candidates drawn and the
/// tracks' digest, which two trees' walks share to the bit.
///
/// Measured (release, 2026-11-05):
///
/// - The stellar mergers at 28, cold, whole x-slab jobs before and pieces after (the mergers'
///   balance; pod, 48 vCPU): on 16 workers 120.4 s and 442 CPU-s before, 27.2 s and 433 after;
///   on 31, 120.5 s and 444 before, 16.3 s and 498 after (31 threads on 24 cores). 69 tracks of
///   97,706,200 candidates, digest `c0e74d1a42e93c81`.
/// - Then their merger times read first (perf-alert-walks-cpu; pod, 32 vCPU, 16 workers): at 28
///   27.8 s and 443 CPU-s cold before, 3.0 s and 44 after, warm alike; at 29 (10 tracks, digest
///   `75210f0c25cf4d75`) 29.6 s and 471 cold, 28.6 s and 453 warm before, 6.2 s and 79, 3.8 s
///   and 57 after.
/// - The core collapses at 29 (4 tracks, digest `5cbd71df19256327`), the told deaths' entries
///   kept: 11.3 s and 149 CPU-s cold, 7.2 s and 108 warm before; 11.3 s and 150, 7.3 s and 106
///   after. At 28 (7 tracks, digest `487c894e15784ba3`) some 5.8 s and 80 CPU-s either way.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "measurement, run by hand: HYPERION_WALK_WORKERS=16 HYPERION_WALK_CLASS=STELLAR_MERGER cargo test --release -p hyperion-server --lib alerts::scan::tests::whole_galaxy_background_walk_wall_and_cpu -- --ignored --nocapture"]
async fn whole_galaxy_background_walk_wall_and_cpu() {
    let workers = std::env::var("HYPERION_WALK_WORKERS").map_or(16, |n| n.parse().unwrap());
    let class = std::env::var("HYPERION_WALK_CLASS").map_or(ClassId::STELLAR_MERGER, |name| {
        ClassSet::ALL
            .iter()
            .find(|class| class.name() == name)
            .unwrap_or_else(|| panic!("no class {name}"))
    });
    let version = std::env::var("HYPERION_WALK_VERSION")
        .map_or(hyperion_sim::GENERATOR_VERSION, |version| {
            hyperion_sim::GeneratorVersion::new(version.parse().unwrap())
        });
    let (server, _data_dir) =
        server(|config| config.workers(std::num::NonZeroUsize::new(workers).unwrap())).await;
    let state = server.state();
    let seed = 0x0926_0000;
    let galaxy = Arc::new(
        Galaxy::from_params_at_version(
            hyperion_sim::Seed::new(seed),
            hyperion_sim::galaxy::params::GalaxyParams::milky_way_like(),
            version,
        )
        .unwrap()
        .with_full_potential(),
    );
    let (subscription, first) = Subscription::new(
        Observer::new(sun(), UniverseTime::EPOCH).unwrap(),
        v_sensor(1e-12, 1e-10),
        ClassSet::of(class),
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    );
    let mut spec = ScanSpec::new(GalaxyKey::new(seed, version), &subscription, &first).unwrap();
    let millennium = hyperion_sim::events::TimeWindow::new(
        UniverseTime::EPOCH,
        UniverseTime::from_julian_years(1_000).unwrap(),
    )
    .unwrap();
    spec.query = AlertQuery::builder(sun(), millennium, v_sensor(1e-12, 1e-10))
        .classes(ClassSet::of(class))
        .build()
        .unwrap();
    let mut walks = Vec::new();
    for walk in ["cold", "warm"] {
        let scans = crate::alerts::service::AlertScans::new(1 << 30);
        let (cpu, wall) = (cpu_seconds(), std::time::Instant::now());
        let walked = scans
            .background(state, &galaxy, &spec, class)
            .await
            .unwrap();
        let wall = wall.elapsed().as_secs_f64();
        let cpu = cpu.zip(cpu_seconds()).map(|(before, after)| after - before);
        eprintln!(
            "{class} at {version}, {walk}, on {workers} workers: {wall:.2} s wall, {cpu:.1?} \
             CPU-s, {} tracks of {:?} candidates, digest {:016x}",
            walked.tracks().len(),
            walked.drawn(),
            fnv1a(format!("{:?}", walked.tracks()).as_bytes())
        );
        eprintln!("  the lists kept: {:?}", state.listed_hosts.counters());
        walks.push(walked);
    }
    assert_eq!(walks[0], walks[1], "the warm walk is the cold one's");
    server.shutdown().await.unwrap();
}
