//! The schedule's tests, on detections made by hand (plan 12, P12.T8.a): a fixed set of sources
//! whose light at the observer is known, and a walk over them that lists each as the sim's would.
//! The walks' own detections feed `tests/alerts_schedule.rs`.

use hyperion_sim::alerts::testing::detection;
use hyperion_sim::alerts::{
    AlertBand, BandLimits, DEFAULT_HOST_LIMIT, DetectionPhase, TransientKind,
};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::id::{EventBin, EventSubject, EventWord, SystemId, event_tags};
use hyperion_sim::observe::light_time;
use hyperion_sim::stellar::events::glitches::RECOVERY_TIME_DAYS;
use hyperion_sim::units::consts::SECONDS_PER_DAY;

use super::*;

const DETECT: f64 = 1e-12;
const RESOLVE: f64 = 1e-10;

/// How far before its window a schedule's first scan walks, for the onsets that arrived before it
/// and cross a limit in it: two years, past the sources' longest rise here.
const REACH_BACK_DAYS: f64 = 730.0;

fn w(value: f64) -> WattsPerSquareMetre {
    WattsPerSquareMetre::new(value)
}

fn days(count: f64) -> Span {
    Span::from_seconds_f64(count * SECONDS_PER_DAY).unwrap()
}

/// The time `days` after the epoch.
fn day(count: f64) -> UniverseTime {
    let span = Span::from_seconds_f64(count.abs() * SECONDS_PER_DAY).unwrap();
    if count < 0.0 {
        UniverseTime::EPOCH.checked_sub(span).unwrap()
    } else {
        UniverseTime::EPOCH.checked_add(span).unwrap()
    }
}

fn seconds_between(early: UniverseTime, late: UniverseTime) -> f64 {
    late.checked_since(early).unwrap().as_seconds_f64()
}

fn sun() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap()
}

fn elsewhere() -> GalacticPosition {
    GalacticPosition::from_light_years([300.0, 26_000.0, 20.0]).unwrap()
}

/// Where every source here is.
fn site() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_100.0, 20.0]).unwrap()
}

fn observer(position: GalacticPosition, at: UniverseTime) -> Observer {
    Observer::new(position, at).unwrap()
}

fn sensor() -> SensorHorizon {
    let limits = BandLimits::new(w(DETECT), w(RESOLVE)).unwrap();
    SensorHorizon::new([
        (AlertBand::Photometric(Band::V), limits),
        (AlertBand::Photometric(Band::Radio), limits),
    ])
    .unwrap()
}

fn host() -> SystemId {
    "0200080020000000".parse().unwrap()
}

/// The source's event numbered `number`.
fn event(number: u8) -> EventId {
    EventId::new(
        EventSubject::System(host()),
        EventWord::new(event_tags::ALL[0], EventBin::new(0).unwrap(), number),
    )
}

/// A source the walk lists, with its light at the observer.
#[derive(Debug, Clone, Copy)]
enum Source {
    /// An outburst whose light rises linearly from nothing at its arrival to `peak` W m⁻² over
    /// `rise` days, then falls linearly to nothing over `fall` days.
    Ramp {
        number: u8,
        arrival: UniverseTime,
        rise: f64,
        fall: f64,
        peak: f64,
    },
    /// A glitch, timed in the radio with no flux at its arrival.
    Glitch { number: u8, arrival: UniverseTime },
}

impl Source {
    fn ramp(number: u8, arrival_day: f64, rise: f64, fall: f64, peak: f64) -> Self {
        Self::Ramp {
            number,
            arrival: day(arrival_day),
            rise,
            fall,
            peak,
        }
    }

    fn arrival(self) -> UniverseTime {
        match self {
            Self::Ramp { arrival, .. } | Self::Glitch { arrival, .. } => arrival,
        }
    }

    fn event(self) -> EventId {
        match self {
            Self::Ramp { number, .. } | Self::Glitch { number, .. } => event(number),
        }
    }

    /// The light at the observer at `t`.
    fn flux_at(self, t: UniverseTime) -> WattsPerSquareMetre {
        match self {
            Self::Ramp {
                arrival,
                rise,
                fall,
                peak,
                ..
            } => {
                let since = t.checked_since(arrival).unwrap().as_seconds_f64() / SECONDS_PER_DAY;
                let share = if since <= 0.0 {
                    0.0
                } else if since <= rise {
                    since / rise
                } else if since <= rise + fall {
                    1.0 - (since - rise) / fall
                } else {
                    0.0
                };
                w(peak * share)
            }
            Self::Glitch { .. } => w(0.0),
        }
    }

    /// The light's knots: the arrival, the peak and the end.
    fn knots(self) -> Vec<UniverseTime> {
        match self {
            Self::Ramp {
                arrival,
                rise,
                fall,
                ..
            } => vec![
                arrival,
                arrival.checked_add(days(rise)).unwrap(),
                arrival.checked_add(days(rise + fall)).unwrap(),
            ],
            Self::Glitch { arrival, .. } => vec![arrival],
        }
    }

    /// The detection by an observer at `at` of the source in `phase` with `flux`.
    fn detection(self, at: &GalacticPosition, phase: DetectionPhase, flux: f64) -> Detection {
        let (kind, band) = match self {
            Self::Ramp { .. } => (
                TransientKind::FuOrionisOutburst,
                AlertBand::Photometric(Band::V),
            ),
            Self::Glitch { .. } => (TransientKind::Glitch, AlertBand::Photometric(Band::Radio)),
        };
        let light = light_time(site().distance_to(at));
        detection(
            self.event(),
            host(),
            None,
            kind,
            phase,
            band,
            w(flux),
            matches!(self, Self::Ramp { .. }) && flux >= RESOLVE,
            site(),
            self.arrival().checked_sub(light).unwrap(),
            at,
        )
    }

    /// What a walk over `window` at `at` lists of the source, tracked: an onset whose light
    /// arrives in the window, seen by its peak; an earlier one in progress if its light at the
    /// window's start is at the limit; and nothing else. A glitch is listed only at its onset.
    fn walked(self, window: TimeWindow, at: &GalacticPosition) -> Option<Tracked> {
        match self {
            Self::Ramp { peak, .. } => {
                let found = if window.contains(self.arrival()) {
                    (peak >= DETECT).then(|| self.detection(at, DetectionPhase::Onset, peak))
                } else if self.arrival() < window.start() {
                    let flux = self.flux_at(window.start()).value();
                    (flux >= DETECT).then(|| self.detection(at, DetectionPhase::InProgress, flux))
                } else {
                    None
                };
                Tracked::of_light(found?, &sensor(), &self.knots(), |t| self.flux_at(t))
            }
            Self::Glitch { .. } => window
                .contains(self.arrival())
                .then(|| Tracked::of_unlit(self.detection(at, DetectionPhase::Onset, 0.0)))
                .flatten(),
        }
    }
}

/// The walk over `sources` that `scan` asks for. A schedule's first scan walks from
/// [`REACH_BACK_DAYS`] before its window, as P12.T8.b's must, for the slow risers.
fn scan(sources: &[Source], scan: &ScanRequest, first: bool) -> Vec<Tracked> {
    let window = scan.window();
    let start = if first {
        window.start().checked_sub(days(REACH_BACK_DAYS)).unwrap()
    } else {
        window.start()
    };
    let walked = TimeWindow::new(start, window.end()).unwrap();
    sources
        .iter()
        .filter_map(|source| source.walked(walked, scan.position()))
        .collect()
}

/// A push as the tests compare them.
type Made = (UniverseTime, EventId, Crossing);

fn made(push: &Push) -> Made {
    (push.at(), push.detection().event(), push.crossing())
}

/// The pushes a subscription at `position` from `start` makes of `sources` as its clock steps
/// through `steps`, merging each scan it asks for as soon as it asks, and once more at the last
/// step for what that merge made due.
fn run(
    sources: &[Source],
    position: GalacticPosition,
    start: UniverseTime,
    steps: &[UniverseTime],
) -> Vec<Made> {
    let (mut subscription, first) = Subscription::new(
        observer(position, start),
        sensor(),
        ClassSet::ALL,
        DEFAULT_HOST_LIMIT,
    );
    subscription
        .merge(&first, scan(sources, &first, true))
        .unwrap();
    let mut pushes = Vec::new();
    let last = *steps.last().unwrap();
    for &next in steps.iter().chain([&last]) {
        let (due, rescan) = subscription.advance(observer(position, next)).into_parts();
        pushes.extend(due.iter().map(made));
        if let Some(rescan) = rescan {
            let first = matches!(rescan, Rescan::Restart(_));
            let request = *rescan.request();
            subscription
                .merge(&request, scan(sources, &request, first))
                .unwrap();
        }
    }
    pushes
}

/// Sources over two years from the epoch: quick and slow risers, a bright one that resolves, one
/// that never reaches the limit, one in progress at the epoch, one over before it, a slow riser
/// that arrived before it and crosses after it, one that lasts across a renewal, and a glitch.
fn sources() -> Vec<Source> {
    vec![
        Source::ramp(1, 10.0, 1.0, 5.0, 1e-11),
        Source::ramp(2, 40.5, 30.0, 60.0, 1e-9),
        Source::ramp(3, 100.0, 2.0, 2.0, 1e-13),
        Source::ramp(4, -20.0, 10.0, 100.0, 1e-9),
        Source::ramp(5, -200.0, 10.0, 50.0, 1e-9),
        Source::ramp(6, -100.0, 2_000.0, 500.0, 1e-11),
        Source::ramp(7, 300.0, 10.0, 200.0, 1e-9),
        Source::Glitch {
            number: 8,
            arrival: day(150.25),
        },
        Source::ramp(9, 365.0, 0.01, 0.01, 1e-10),
        Source::ramp(10, 500.0, 100.0, 100.0, 1e-10),
        Source::ramp(11, 700.0, 3.0, 400.0, 1e-9),
    ]
}

/// The look-ahead, the glitch's clearing time and the crossings' resolution are what their docs
/// say.
#[test]
fn the_lookahead_is_a_julian_year_and_a_glitch_clears_after_its_recovery_time() {
    assert_eq!(Some(ALERT_LOOKAHEAD), Span::from_julian_years(1));
    assert_eq!(
        HALF_LOOKAHEAD.checked_mul(2),
        Some(ALERT_LOOKAHEAD),
        "half the look-ahead"
    );
    let recovery = RECOVERY_TIME_DAYS * SECONDS_PER_DAY;
    assert!(
        (GLITCH_CLEAR_AFTER.as_seconds_f64() - recovery).abs() < 1e-6,
        "{recovery} s"
    );
    assert_eq!(CROSSING_RESOLUTION, Span::new(0, 1_000_000).unwrap());
}

/// A riser that crosses the detection limit and then the resolve limit is pushed twice, a bearing
/// at the first crossing and the host at the second, each to the resolution, and cleared when it
/// fades; never at its arrival.
#[test]
fn a_riser_is_pushed_at_each_crossing_not_at_its_arrival() {
    let riser = Source::ramp(1, 10.0, 30.0, 60.0, 1e-9);
    let pushes = run(&[riser], sun(), UniverseTime::EPOCH, &[day(200.0)]);
    let expected = [
        (day(10.0 + 30.0 * DETECT / 1e-9), Crossing::Detect),
        (day(10.0 + 30.0 * RESOLVE / 1e-9), Crossing::Resolve),
        (
            day(10.0 + 30.0 + 60.0 * (1.0 - DETECT / 1e-9)),
            Crossing::Clear,
        ),
    ];
    assert_eq!(pushes.len(), 3, "{pushes:?}");
    for ((at, event, crossing), (want, wanted)) in pushes.iter().zip(expected) {
        assert_eq!((*event, *crossing), (riser.event(), wanted));
        let off = seconds_between(want.min(*at), want.max(*at));
        assert!(off <= 1e-3 + 1e-6, "{crossing:?} {off} s from {want}");
        assert!(*at > riser.arrival());
    }
    let sightings = pushes
        .iter()
        .filter(|push| push.2.level().is_some())
        .count();
    assert_eq!(sightings, 2, "pushed as a bearing, then resolved");
}

/// Each push's flux is past the limit it crosses, as the light gives it there.
#[test]
fn the_flux_of_each_push_is_the_light_at_its_crossing() {
    let riser = Source::ramp(1, 10.0, 30.0, 60.0, 1e-9);
    let (mut subscription, first) = Subscription::new(
        observer(sun(), UniverseTime::EPOCH),
        sensor(),
        ClassSet::ALL,
        DEFAULT_HOST_LIMIT,
    );
    subscription
        .merge(&first, scan(&[riser], &first, true))
        .unwrap();
    let advance = subscription.advance(observer(sun(), day(200.0)));
    for push in advance.due() {
        let flux = push.flux().value();
        assert_eq!(flux.to_bits(), riser.flux_at(push.at()).value().to_bits());
        match push.crossing() {
            Crossing::Detect => assert!((DETECT..DETECT * 1.001).contains(&flux), "{flux}"),
            Crossing::Resolve => assert!((RESOLVE..RESOLVE * 1.001).contains(&flux), "{flux}"),
            Crossing::Clear => assert!(flux < DETECT && flux > DETECT * 0.999, "{flux}"),
        }
        assert_eq!(push.detection().phase(), DetectionPhase::Onset);
        assert_eq!(
            push.detection().flux().value().to_bits(),
            1e-9_f64.to_bits()
        );
    }
}

/// An onset under the detection limit at its arrival is not pushed until its light reaches it.
#[test]
fn an_onset_under_the_limit_is_not_pushed_early() {
    let riser = Source::ramp(1, 10.0, 100.0, 100.0, 1e-11);
    // Its light reaches 10⁻¹² W m⁻² ten days after it arrives.
    let crossing = day(20.0);
    let (mut subscription, first) = Subscription::new(
        observer(sun(), UniverseTime::EPOCH),
        sensor(),
        ClassSet::ALL,
        DEFAULT_HOST_LIMIT,
    );
    subscription
        .merge(&first, scan(&[riser], &first, true))
        .unwrap();
    for at in [day(10.0), day(10.5), day(19.99)] {
        let advance = subscription.advance(observer(sun(), at));
        assert_eq!(advance.due(), [], "at {at}");
    }
    let advance = subscription.advance(observer(sun(), day(20.01)));
    assert_eq!(advance.due().len(), 1);
    let push = &advance.due()[0];
    assert_eq!(push.crossing(), Crossing::Detect);
    assert!(seconds_between(crossing.min(push.at()), crossing.max(push.at())) <= 1e-3 + 1e-6);
}

/// A slow riser whose onset arrived before the subscription and which crosses the limit after it
/// is pushed once, at the crossing: the first scan walks back for it.
#[test]
fn a_riser_arriving_before_the_subscription_and_crossing_after_it_is_pushed_once() {
    // It arrived 100 days before the epoch and reaches the limit 100 days after it.
    let riser = Source::ramp(6, -100.0, 2_000.0, 500.0, 1e-11);
    let pushes = run(&[riser], sun(), UniverseTime::EPOCH, &[day(400.0)]);
    let detects: Vec<_> = pushes
        .iter()
        .filter(|push| push.2 == Crossing::Detect)
        .collect();
    assert_eq!(detects.len(), 1, "{pushes:?}");
    let off = seconds_between(day(100.0).min(detects[0].0), day(100.0).max(detects[0].0));
    assert!(off <= 1e-3 + 1e-6, "{off} s");
    // A walk from the epoch alone would not list it: its light there is under the limit.
    let window = TimeWindow::new(UniverseTime::EPOCH, day(365.25)).unwrap();
    assert!(riser.walked(window, &sun()).is_none());
}

/// An event in progress when the schedule starts is pushed then, at both levels it has reached; one
/// that cleared before it started is not pushed.
#[test]
fn an_event_in_progress_at_the_start_is_pushed_then_and_one_over_before_is_not() {
    let in_progress = Source::ramp(4, -20.0, 10.0, 100.0, 1e-9);
    let over = Source::ramp(5, -200.0, 10.0, 50.0, 1e-9);
    let pushes = run(
        &[in_progress, over],
        sun(),
        UniverseTime::EPOCH,
        &[day(1.0)],
    );
    assert_eq!(
        pushes,
        [
            (UniverseTime::EPOCH, in_progress.event(), Crossing::Detect),
            (UniverseTime::EPOCH, in_progress.event(), Crossing::Resolve),
        ]
    );
}

/// The whole year in one step, or in ten, pushes each crossing of each event once, at the same
/// times.
#[test]
fn advancing_in_one_step_or_in_ten_pushes_each_onset_once() {
    let sources = sources();
    let end = day(365.25);
    let one = run(&sources, sun(), UniverseTime::EPOCH, &[end]);
    let tenths: Vec<_> = (1..=10).map(|k| day(36.525 * f64::from(k))).collect();
    let ten = run(&sources, sun(), UniverseTime::EPOCH, &tenths);
    assert_eq!(one, ten);
    let mut each: Vec<_> = one
        .iter()
        .map(|&(_, event, crossing)| (event, crossing))
        .collect();
    each.sort_unstable();
    let pushes = each.len();
    each.dedup();
    assert_eq!(each.len(), pushes, "each crossing once: {one:?}");
    // Every source seen in the year but the faint one and the one over before it.
    let seen: BTreeSet<_> = one.iter().map(|&(_, event, _)| event).collect();
    let expected: BTreeSet<_> = [1, 2, 4, 6, 7, 8, 9].map(event).into_iter().collect();
    assert_eq!(seen, expected);
    assert!(one.windows(2).all(|pair| pair[0] <= pair[1]), "in order");
}

/// The pushes over two years are the same however the steps split them: steps of a day and of
/// months, steps of no length, and a step longer than the look-ahead, which the late merge makes
/// up for.
#[test]
fn the_pushes_are_the_same_however_the_steps_split() {
    let sources = sources();
    let end = day(730.5);
    let mut whole = run(&sources, sun(), UniverseTime::EPOCH, &[end]);
    whole.sort_unstable();
    let splits: [Vec<UniverseTime>; 4] = [
        (1..=730).map(|d| day(f64::from(d))).chain([end]).collect(),
        [3.0, 3.0, 47.3, 180.0, 182.7, 182.7, 400.0, 401.0, 650.0]
            .map(day)
            .into_iter()
            .chain([end])
            .collect(),
        vec![day(600.0), end],
        vec![day(100.0), day(100.0), day(729.0), end],
    ];
    for steps in splits {
        let split = run(&sources, sun(), UniverseTime::EPOCH, &steps);
        let mut sorted = split.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, whole, "over {} steps", steps.len());
        assert_eq!(split.len(), whole.len());
    }
    assert!(
        whole.iter().any(|&(_, event, _)| event == self::event(11)),
        "the second year's sources are pushed"
    );
}

/// An event whose light lasts past the end of its first scan is pushed once: the renewal, which
/// lists it in progress after its bearing and its host were pushed, adds only the clearing its
/// first track lacked.
#[test]
fn an_event_a_later_scan_finds_in_progress_is_not_pushed_again() {
    // It arrives late in the first year, and the first scan's light stops at its peak, still
    // above the limit, so its first track has no clearing.
    let lasting = Source::ramp(7, 300.0, 10.0, 200.0, 1e-9);
    let (mut subscription, first) = Subscription::new(
        observer(sun(), UniverseTime::EPOCH),
        sensor(),
        ClassSet::ALL,
        DEFAULT_HOST_LIMIT,
    );
    let found = lasting
        .walked(first.window(), &sun())
        .map(|track| track.detection().clone())
        .unwrap();
    let knots = &lasting.knots()[..2];
    let short = Tracked::of_light(found, &sensor(), knots, |t| lasting.flux_at(t)).unwrap();
    assert_eq!(short.crossing(Crossing::Clear), None);
    subscription.merge(&first, [short]).unwrap();
    let advance = subscription.advance(observer(sun(), day(305.0)));
    let crossings: Vec<_> = advance.due().iter().map(Push::crossing).collect();
    assert_eq!(crossings, [Crossing::Detect, Crossing::Resolve]);
    let Some(Rescan::Renew(renewal)) = advance.rescan().copied() else {
        panic!("a renewal past half-way: {:?}", advance.rescan());
    };
    assert_eq!(renewal.window().start(), first.window().end());
    let again = lasting.walked(renewal.window(), &sun()).unwrap();
    assert_eq!(again.detection().phase(), DetectionPhase::InProgress);
    assert_eq!(
        again.crossing(Crossing::Detect),
        advance.due().first().map(|push| (push.at(), push.flux())),
        "the renewal finds the same bearing"
    );
    subscription.merge(&renewal, [again]).unwrap();
    let pushes: Vec<_> = subscription
        .advance(observer(sun(), day(700.0)))
        .due()
        .iter()
        .map(made)
        .collect();
    assert_eq!(pushes.len(), 1, "{pushes:?}");
    assert_eq!(pushes[0].2, Crossing::Clear);
}

/// A clock that runs backwards, or an observer moved elsewhere, drops the schedule and asks for a
/// fresh scan from the new observer; a clock that stays or runs on does not.
#[test]
fn a_backwards_clock_or_a_moved_observer_requests_a_rescan() {
    let (mut subscription, first) = Subscription::new(
        observer(sun(), day(10.0)),
        sensor(),
        ClassSet::ALL,
        DEFAULT_HOST_LIMIT,
    );
    subscription.merge(&first, []).unwrap();
    for at in [day(10.0), day(11.0), day(11.0)] {
        assert_eq!(subscription.advance(observer(sun(), at)).rescan(), None);
    }
    let back = subscription.advance(observer(sun(), day(5.0)));
    let Some(Rescan::Restart(request)) = back.rescan() else {
        panic!("a backwards clock restarts: {back:?}");
    };
    assert_eq!(request.window().start(), day(5.0));
    assert_eq!(request.position(), &sun());
    assert_eq!(subscription.schedule().start(), day(5.0));
    let moved = subscription.advance(observer(elsewhere(), day(6.0)));
    let Some(Rescan::Restart(request)) = moved.rescan() else {
        panic!("a moved observer restarts: {moved:?}");
    };
    assert_eq!(
        request.window(),
        TimeWindow::new(day(6.0), day(6.0).checked_add(ALERT_LOOKAHEAD).unwrap()).unwrap()
    );
    assert_eq!(request.position(), &elsewhere());
    assert_eq!(subscription.schedule().position(), &elsewhere());
}

/// After a jump, nothing of the old place's schedule is pushed: not its pushes to come, not its
/// scans, and not what it had pushed, so the same event found again at the new place is pushed
/// again there.
#[test]
fn nothing_from_the_old_place_survives_a_jump() {
    let here = Source::ramp(2, 40.0, 30.0, 60.0, 1e-9);
    let (mut subscription, first) = Subscription::new(
        observer(sun(), UniverseTime::EPOCH),
        sensor(),
        ClassSet::ALL,
        DEFAULT_HOST_LIMIT,
    );
    subscription
        .merge(&first, scan(&[here], &first, true))
        .unwrap();
    let advance = subscription.advance(observer(sun(), day(41.0)));
    assert_eq!(advance.due().len(), 1, "its bearing, at the old place");
    assert!(subscription.schedule().pending() > 0);

    // At the new place the event's light arrives 20 days later.
    let there = Source::ramp(2, 60.0, 30.0, 60.0, 1e-9);
    let jump = subscription.advance(observer(elsewhere(), day(46.0)));
    assert_eq!(jump.due(), []);
    let Some(Rescan::Restart(fresh)) = jump.rescan().copied() else {
        panic!("a jump restarts: {jump:?}");
    };
    assert_eq!(subscription.schedule().pending(), 0);
    // The old place's scans are refused: its first, and a renewal it would have asked for.
    assert_eq!(
        subscription.merge(&first, scan(&[here], &first, true)),
        Err(MergeScanError::Stale)
    );
    subscription
        .merge(&fresh, scan(&[there], &fresh, true))
        .unwrap();
    let pushes: Vec<_> = subscription
        .advance(observer(elsewhere(), day(200.0)))
        .due()
        .iter()
        .map(made)
        .collect();
    assert_eq!(pushes.len(), 3, "{pushes:?}");
    assert!(pushes.iter().all(|push| push.1 == here.event()));
    assert!(
        pushes.iter().all(|push| push.0 > there.arrival()),
        "each at the new place's crossings: {pushes:?}"
    );
}

/// A renewal is asked for once the clock passes half-way through what has been asked for, once,
/// from where the last scan ends; scans merge only in order, each once; and none reaches past the
/// clock window.
#[test]
fn a_renewal_is_asked_for_past_half_way_and_merged_in_order() {
    let (mut subscription, first) = Subscription::new(
        observer(sun(), UniverseTime::EPOCH),
        sensor(),
        ClassSet::ALL,
        DEFAULT_HOST_LIMIT,
    );
    assert_eq!(
        first.window(),
        TimeWindow::new(UniverseTime::EPOCH, day(365.25)).unwrap()
    );
    assert_eq!(
        subscription.advance(observer(sun(), day(182.0))).rescan(),
        None
    );
    let Some(Rescan::Renew(second)) = subscription
        .advance(observer(sun(), day(182.625)))
        .rescan()
        .copied()
    else {
        panic!("a renewal at half-way");
    };
    assert_eq!(
        second.window(),
        TimeWindow::new(day(365.25), day(730.5)).unwrap()
    );
    assert_eq!(
        subscription.advance(observer(sun(), day(300.0))).rescan(),
        None
    );
    // The clock runs past both: the next reaches a look-ahead past it.
    let Some(Rescan::Renew(third)) = subscription
        .advance(observer(sun(), day(800.0)))
        .rescan()
        .copied()
    else {
        panic!("a renewal past the end");
    };
    assert_eq!(
        third.window(),
        TimeWindow::new(day(730.5), day(800.0 + 365.25)).unwrap()
    );
    assert_eq!(
        subscription.merge(&second, []),
        Err(MergeScanError::OutOfOrder {
            expected: UniverseTime::EPOCH,
            found: day(365.25)
        })
    );
    subscription.merge(&first, []).unwrap();
    assert_eq!(
        subscription.merge(&first, []),
        Err(MergeScanError::OutOfOrder {
            expected: day(365.25),
            found: UniverseTime::EPOCH
        })
    );
    subscription.merge(&second, []).unwrap();
    subscription.merge(&third, []).unwrap();
    assert_eq!(subscription.schedule().covered(), day(800.0 + 365.25));

    // Near the clock window's end the scans stop at it.
    let late = ClockWindow::END.checked_sub(days(100.0)).unwrap();
    let (mut subscription, first) = Subscription::new(
        observer(sun(), late),
        sensor(),
        ClassSet::ALL,
        DEFAULT_HOST_LIMIT,
    );
    assert_eq!(first.window().end(), ClockWindow::END);
    subscription.merge(&first, []).unwrap();
    let at_end = subscription.advance(observer(sun(), ClockWindow::END));
    assert_eq!(at_end.rescan(), None);
    assert!(subscription.query(&first).is_ok());
}

/// A glitch, which has no light, is pushed at its arrival and clears its recovery time later; a
/// lit kind takes its light instead.
#[test]
fn a_glitch_is_pushed_at_its_arrival_and_cleared_after_its_recovery_time() {
    let glitch = Source::Glitch {
        number: 8,
        arrival: day(150.25),
    };
    let pushes = run(&[glitch], sun(), UniverseTime::EPOCH, &[day(200.0)]);
    let cleared = day(150.25).checked_add(GLITCH_CLEAR_AFTER).unwrap();
    assert_eq!(
        pushes,
        [
            (day(150.25), glitch.event(), Crossing::Detect),
            (cleared, glitch.event(), Crossing::Clear),
        ]
    );
    let flare =
        Source::ramp(1, 10.0, 1.0, 1.0, 1e-11).detection(&sun(), DetectionPhase::Onset, 1e-11);
    assert_eq!(Tracked::of_unlit(flare), None);
}

/// The crossings on a light that rises, spikes past the resolve limit, dips under the detection
/// limit and rises again: the bearing at the first rise, the host in the spike, and the clearing
/// after the last fall. A light already at the limit is seen at its first knot, one still at it at
/// its last never clears, one that never reaches it is not tracked, and nor is a band the sensor
/// lacks.
#[test]
fn the_crossings_are_found_on_any_light_monotone_between_its_knots() {
    let source = Source::ramp(1, 10.0, 1.0, 1.0, 1e-9);
    let found = source.detection(&sun(), DetectionPhase::Onset, 1e-9);
    let knots = [day(10.0), day(11.0), day(12.0), day(13.0), day(15.0)];
    let values = [0.0, 1e-9, 1e-13, 1e-11, 0.0];
    // Linear between the knots.
    let light = |t: UniverseTime| {
        let i = knots.iter().rposition(|&knot| knot <= t).unwrap_or(0);
        let Some(&next) = knots.get(i + 1) else {
            return w(values[i]);
        };
        let share = seconds_between(knots[i], t) / seconds_between(knots[i], next);
        w(values[i] + (values[i + 1] - values[i]) * share.clamp(0.0, 1.0))
    };
    let track = Tracked::of_light(found.clone(), &sensor(), &knots, light).unwrap();
    let near = |crossing, want: UniverseTime| {
        let (at, _) = track.crossing(crossing).unwrap();
        let off = seconds_between(want.min(at), want.max(at));
        assert!(off <= 1e-3 + 1e-6, "{crossing:?} {off} s from {want}");
    };
    near(Crossing::Detect, day(10.0 + DETECT / 1e-9));
    near(Crossing::Resolve, day(10.0 + RESOLVE / 1e-9));
    near(Crossing::Clear, day(13.0 + 2.0 * (1.0 - DETECT / 1e-11)));

    let bright = |_| w(1e-9);
    let track = Tracked::of_light(found.clone(), &sensor(), &knots, bright).unwrap();
    assert_eq!(track.crossing(Crossing::Detect), Some((day(10.0), w(1e-9))));
    assert_eq!(
        track.crossing(Crossing::Resolve),
        Some((day(10.0), w(1e-9)))
    );
    assert_eq!(track.crossing(Crossing::Clear), None);
    assert_eq!(
        Tracked::of_light(found.clone(), &sensor(), &knots, |_| w(1e-13)),
        None
    );
    assert_eq!(
        Tracked::of_light(found.clone(), &sensor(), &knots, |_| w(f64::NAN)),
        None
    );
    let x_ray = SensorHorizon::new([(
        AlertBand::XRay,
        BandLimits::new(w(DETECT), w(RESOLVE)).unwrap(),
    )])
    .unwrap();
    assert_eq!(Tracked::of_light(found, &x_ray, &knots, bright), None);
}

#[test]
#[should_panic(expected = "strictly increasing")]
fn a_lights_knots_must_increase() {
    let found =
        Source::ramp(1, 10.0, 1.0, 1.0, 1e-9).detection(&sun(), DetectionPhase::Onset, 1e-9);
    let _ = Tracked::of_light(found, &sensor(), &[day(2.0), day(1.0)], |_| w(1.0));
}

/// The crossings of an event that clears before the merged scans end are forgotten, since no
/// later scan can list it, and those of one still lit are kept.
#[test]
fn the_crossings_kept_are_of_the_events_a_later_scan_could_list() {
    let quick = Source::ramp(1, 10.0, 1.0, 5.0, 1e-11);
    let lasting = Source::ramp(7, 300.0, 10.0, 200.0, 1e-9);
    let (mut schedule, first) = AlertSchedule::new(&observer(sun(), UniverseTime::EPOCH));
    schedule
        .merge(&first, scan(&[quick, lasting], &first, true))
        .unwrap();
    let kept: BTreeSet<_> = schedule.scheduled.iter().map(|&(event, _)| event).collect();
    assert_eq!(kept, BTreeSet::from([lasting.event()]));
    assert_eq!(schedule.pending(), 5, "both events' pushes stay");
}

/// A subscription's query is its scan's place and window with its sensor, classes and host limit,
/// and an advance moves its observer.
#[test]
fn a_subscription_asks_the_sim_for_its_scans_window() {
    let classes = ClassSet::of(hyperion_sim::galaxy::catalogue_classes::ClassId::CORE_COLLAPSE);
    let limit = NonZeroU32::new(1_000).unwrap();
    let (mut subscription, first) =
        Subscription::new(observer(sun(), day(3.0)), sensor(), classes, limit);
    let query = subscription.query(&first).unwrap();
    assert_eq!(query.observer_position(), &sun());
    assert_eq!(query.window(), first.window());
    assert_eq!(query.sensor(), subscription.sensor());
    assert_eq!((query.classes(), query.host_limit()), (classes, limit));
    assert_eq!(
        (subscription.classes(), subscription.host_limit()),
        (classes, limit)
    );
    let _ = subscription.advance(observer(sun(), day(4.0)));
    assert_eq!(subscription.observer().time(), day(4.0));
}
