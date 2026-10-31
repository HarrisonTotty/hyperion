//! The alert schedule on the alert walks' own detections (plan 12, P12.T8.a): the local events of
//! a range query's systems (P12.T5.e), and a production core collapse (P12.T5.b).
//!
//! The walks' detections carry no light curve, so each test gives the schedule a light of its own
//! for them: a local event's flux held from when its light is first counted, and the supernova's
//! template scaled to the detection's peak.

use std::collections::{BTreeMap, BTreeSet};

use hyperion_server::alerts::{Crossing, Push, Rescan, ScanRequest, Subscription, Tracked};
use hyperion_sim::Seed;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, BandLimits, ClassSet, DEFAULT_HOST_LIMIT, Detection, DetectionPhase,
    LocalEventsCache, OneShotCells, OneShotClass, OneShotEvent, SensorHorizon, local_events,
    one_shot_detections, supernova_luminosity, supernova_peak,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::NoCache;
use hyperion_sim::galaxy::query::{QueryMode, RangeQuery, range_query_observed};
use hyperion_sim::id::EventId;
use hyperion_sim::math::powf;
use hyperion_sim::observe::{NoStarsCache, Observer, light_time};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::consts::SECONDS_PER_DAY;
use hyperion_sim::units::{LightYears, WattsPerSquareMetre, Years};

fn w(value: f64) -> WattsPerSquareMetre {
    WattsPerSquareMetre::new(value)
}

fn days(count: f64) -> Span {
    Span::from_seconds_f64(count * SECONDS_PER_DAY).unwrap()
}

/// A push as the tests compare them.
type Made = (UniverseTime, EventId, Crossing);

fn made(push: &Push) -> Made {
    (push.at(), push.detection().event(), push.crossing())
}

/// The pushes `subscription` makes as its observer's clock steps through `steps`, with `scan`
/// listing what each scan it asks for finds, merged as soon as it asks, and once more at the
/// last step for what that merge made due.
fn run(
    subscription: &mut Subscription,
    first: &ScanRequest,
    steps: &[UniverseTime],
    mut scan: impl FnMut(&Subscription, &ScanRequest) -> Vec<Tracked>,
) -> Vec<Made> {
    let found = scan(subscription, first);
    subscription.merge(first, found).unwrap();
    let position = *subscription.observer().position();
    let last = *steps.last().unwrap();
    let mut pushes = Vec::new();
    for &next in steps.iter().chain([&last]) {
        let (due, rescan) = subscription
            .advance(Observer::new(position, next).unwrap())
            .into_parts();
        pushes.extend(due.iter().map(made));
        if let Some(rescan) = rescan {
            assert!(matches!(rescan, Rescan::Renew(_)), "{rescan:?}");
            let found = scan(subscription, rescan.request());
            subscription.merge(rescan.request(), found).unwrap();
        }
    }
    pushes
}

/// `detection`'s track for a light held at its flux from when it is first counted in the scan over
/// `window`, or its glitch's track.
///
/// The light never fades, so no event is forgotten: a later scan that lists one in progress, by
/// the event's own light, is always one whose crossings the schedule still holds.
fn held(detection: Detection, window: TimeWindow, sensor: &SensorHorizon) -> Tracked {
    if detection.kind().row().light_curve().is_none() {
        return Tracked::of_unlit(detection).unwrap();
    }
    let from = match detection.phase() {
        DetectionPhase::Onset => detection.arrival(),
        DetectionPhase::InProgress => window.start(),
    };
    let flux = detection.flux();
    Tracked::of_light(detection, sensor, &[from], |_| flux)
        .expect("a detection's flux reaches its band's limit")
}

/// The local events of the systems within 10 ly of the Sun, a Sun-like seed's, over the year a
/// subscription there asks for and the next, pushed the same whether its clock steps once or
/// daily, each crossing of each event once, every onset at its arrival.
#[test]
fn alerts_schedule_pushes_a_range_querys_local_events_once_however_the_clock_steps() {
    let galaxy = Galaxy::new(Seed::new(12));
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
    let found = range_query_observed(
        &galaxy,
        &mut NoCache::new(),
        &mut NoStarsCache,
        &[],
        &RangeQuery::builder(sun, LightYears::new(10.0))
            .mode(QueryMode::ObservedFrom(sun))
            .build()
            .unwrap(),
    )
    .unwrap();
    let rows = found.observed();
    let visual = BandLimits::new(w(1e-16), w(1e-14)).unwrap();
    let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), visual)]).unwrap();
    let mut noise = NoiseCache::with_capacity(1_024);
    let mut cache = LocalEventsCache::with_capacity(1_024);
    let mut onsets = BTreeMap::new();
    let mut scan = |subscription: &Subscription, request: &ScanRequest| {
        let query = subscription.query(request).unwrap();
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
        let mut seen = Vec::new();
        local_events(
            &galaxy,
            &mut ctx,
            &query,
            rows,
            &mut NoStarsCache,
            &mut cache,
            &mut seen,
        );
        for detection in &seen {
            if detection.phase() == DetectionPhase::Onset {
                onsets.insert(detection.event(), detection.arrival());
            }
        }
        seen.into_iter()
            .map(|detection| held(detection, request.window(), subscription.sensor()))
            .collect::<Vec<_>>()
    };

    let start = UniverseTime::EPOCH;
    let end = start.checked_add(days(400.0)).unwrap();
    let subscribe = || {
        Subscription::new(
            Observer::new(sun, start).unwrap(),
            sensor.clone(),
            ClassSet::ALL,
            DEFAULT_HOST_LIMIT,
        )
    };
    let (mut once, first) = subscribe();
    let mut whole = run(&mut once, &first, &[end], &mut scan);
    let daily: Vec<_> = (1..=400)
        .map(|d| start.checked_add(days(f64::from(d))).unwrap())
        .collect();
    let (mut stepped, first) = subscribe();
    let steps = run(&mut stepped, &first, &daily, &mut scan);
    assert!(steps.windows(2).all(|pair| pair[0] <= pair[1]), "on time");
    whole.sort_unstable();
    assert_eq!(whole, steps);

    let detects: Vec<_> = steps
        .iter()
        .filter(|push| push.2 == Crossing::Detect)
        .collect();
    let events: BTreeSet<_> = detects.iter().map(|push| push.1).collect();
    assert_eq!(events.len(), detects.len(), "each event's bearing once");
    assert!(detects.len() > 10, "{} events seen", detects.len());
    for (event, arrival) in onsets.iter().filter(|&(_, arrival)| *arrival < end) {
        let pushed = detects.iter().find(|push| push.1 == *event);
        assert_eq!(pushed.map(|push| push.0), Some(*arrival), "{event:?}");
    }
    eprintln!(
        "{} pushes of {} events, {} onsets",
        steps.len(),
        events.len(),
        onsets.len()
    );
}

/// Keeps every class cell it is asked for, for one galaxy.
#[derive(Debug, Default)]
struct KeepCells(BTreeMap<CatalogueCellKey, Vec<OneShotEvent>>);

impl OneShotCells for KeepCells {
    fn with_events(
        &mut self,
        key: CatalogueCellKey,
        generate: &mut dyn FnMut() -> Vec<OneShotEvent>,
        visit: &mut dyn FnMut(&[OneShotEvent]),
    ) {
        visit(self.0.entry(key).or_insert_with(generate));
    }
}

/// The last core collapse before +990 years by the solar circle in `galaxy`, as P12.T5.b's slow
/// test finds it.
fn latest_death(galaxy: &Galaxy, process: &CoreCollapseProcess) -> OneShotEvent {
    let mut death: Option<OneShotEvent> = None;
    for y in 44..56 {
        for x in -6..6 {
            for z in [-1, 0] {
                let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [x, y, z]).unwrap();
                for entry in CatalogueClassCell::generate(galaxy, process, key).entries() {
                    let event = process.event(entry.value());
                    let t = event.emitted().since_epoch().as_julian_years_f64();
                    if (400.0..990.0).contains(&t)
                        && death.is_none_or(|last| event.emitted() > last.emitted())
                    {
                        death = Some(event);
                    }
                }
            }
        }
    }
    death.expect("a death to come by the solar circle")
}

/// `death`'s light at the observer in `onset`'s band: its template scaled to the detection's
/// peak, from the arrival, and the light's knots out to 274 years, ten a decade of age.
fn template_light(
    death: &OneShotEvent,
    onset: &Detection,
) -> (
    impl Fn(UniverseTime) -> WattsPerSquareMetre + Copy + use<>,
    Vec<UniverseTime>,
) {
    let (kind, band, arrival) = (death.supernova(), onset.band(), onset.arrival());
    let scale = onset.flux().value() / supernova_peak(kind, band).value();
    let light = move |t: UniverseTime| {
        let age = t.checked_since(arrival).unwrap().as_julian_years_f64();
        w(scale * supernova_luminosity(kind, Years::new(age), band).value())
    };
    let knots = [0.0]
        .into_iter()
        .chain((-40..=50).map(|k| powf(10.0, f64::from(k) / 10.0)))
        .map(|age| arrival.checked_add(days(age)).unwrap())
        .collect();
    (light, knots)
}

/// A core collapse to come in the galaxy of seed 9, by the solar circle, seen from 50 ly: the
/// subscription half a year before its light arrives pushes it once as a bearing and once as a
/// host, each when its template's light crosses the limit after the arrival and before the peak,
/// and the renewal that lists it in progress adds neither again.
#[test]
#[ignore = "slow: generates the core-collapse class's cells by the solar circle and about the observer"]
fn alerts_schedule_pushes_a_core_collapse_at_its_crossings() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = CoreCollapseProcess::new(&galaxy);
    let death = latest_death(&galaxy, &process);
    let site = death.site().to_light_years_f64();
    let at = GalacticPosition::from_light_years([site[0] + 50.0, site[1], site[2]]).unwrap();
    let arrival = death
        .emitted()
        .checked_add(light_time(death.site().distance_to(&at)))
        .unwrap();
    let half = Span::from_seconds(31_557_600 / 2);
    let start = arrival.checked_sub(half).unwrap();
    let limits = |detect: f64| BandLimits::new(w(detect), w(detect * 100.0)).unwrap();
    let sensor = SensorHorizon::new([
        (AlertBand::Photometric(Band::V), limits(7.6e-6)),
        (AlertBand::Photometric(Band::K), limits(1.4e-6)),
    ])
    .unwrap();
    let (mut subscription, first) = Subscription::new(
        Observer::new(at, start).unwrap(),
        sensor.clone(),
        ClassSet::of(ClassId::CORE_COLLAPSE),
        DEFAULT_HOST_LIMIT,
    );
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut cells = KeepCells::default();
    let mut walk = |subscription: &Subscription, request: &ScanRequest| {
        let query = subscription.query(request).unwrap();
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers).with_one_shot_cells(&mut cells);
        let mut seen = Vec::new();
        one_shot_detections(&galaxy, &mut ctx, &query, &process, &mut seen);
        seen
    };

    let seen = walk(&subscription, &first);
    assert_eq!(seen.len(), 1, "{seen:?}");
    let onset = seen[0].clone();
    assert_eq!(onset.event(), death.event());
    assert_eq!(onset.arrival(), arrival);
    assert_eq!(onset.phase(), DetectionPhase::Onset);
    let band = onset.band();
    let (light, knots) = template_light(&death, &onset);
    let peak = knots
        .iter()
        .copied()
        .max_by(|a, b| light(*a).value().total_cmp(&light(*b).value()))
        .unwrap();
    let tracked = Tracked::of_light(onset, &sensor, &knots, light).unwrap();
    let (detect, _) = tracked.crossing(Crossing::Detect).unwrap();
    let (resolve, _) = tracked.crossing(Crossing::Resolve).unwrap();
    assert!(arrival <= detect && detect <= resolve && resolve <= peak);
    eprintln!(
        "{:?} in {band:?}: detected {} s and resolved {} s after its arrival, peak {} days after",
        death.supernova(),
        detect.checked_since(arrival).unwrap().as_seconds_f64(),
        resolve.checked_since(arrival).unwrap().as_seconds_f64(),
        peak.checked_since(arrival).unwrap().as_seconds_f64() / SECONDS_PER_DAY,
    );
    subscription.merge(&first, [tracked]).unwrap();

    let end = arrival.checked_add(half).unwrap();
    let first_year = subscription.advance(Observer::new(at, end).unwrap());
    let pushes: Vec<_> = first_year.due().iter().map(made).collect();
    assert_eq!(
        pushes,
        [
            (detect, death.event(), Crossing::Detect),
            (resolve, death.event(), Crossing::Resolve),
        ]
    );
    let Some(Rescan::Renew(renewal)) = first_year.rescan().copied() else {
        panic!(
            "a renewal at the first scan's end: {:?}",
            first_year.rescan()
        );
    };
    let again = walk(&subscription, &renewal);
    assert_eq!(again.len(), 1, "{again:?}");
    assert_eq!(again[0].phase(), DetectionPhase::InProgress);
    let in_progress = Tracked::of_light(again[0].clone(), &sensor, &knots, light).unwrap();
    assert_eq!(
        in_progress.crossing(Crossing::Detect).map(|(at, _)| at),
        Some(detect)
    );
    subscription.merge(&renewal, [in_progress]).unwrap();
    let later = subscription.advance(Observer::new(at, renewal.window().end()).unwrap());
    assert!(
        later
            .due()
            .iter()
            .all(|push| push.crossing() == Crossing::Clear),
        "{:?}",
        later.due()
    );
}
