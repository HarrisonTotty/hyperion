//! A subscription's schedule of pushes, and the pure advance over it (plan 12, P12.T8.a; Design
//! notes 6, 8 and 9).
//!
//! # The crossing rule
//!
//! A detection's flux is its peak's for an onset and its light at the window's start for an event
//! in progress, and decides only whether the walk lists it. The schedule pushes a detection when
//! its flux at the observer first reaches the sensor's detection limit in its band, as a bearing
//! ([`Crossing::Detect`]), again when it first reaches the resolve limit ([`Crossing::Resolve`]),
//! and once more when it falls under the detection limit for the last time ([`Crossing::Clear`]):
//! the condition clears (Design note 11). So a riser is pushed at its crossings, never at its
//! arrival: an FU Orionis outburst's peak arrives up to 25 years after its onset, and a Type Ia's
//! first light is days under the limit (research's lean of 2026-10-30 for T8, adopted).
//!
//! Each walk's detection carries its light at the observer (P12.T8.b.1), which [`Tracked::of`]
//! takes: a function of the observer's time, monotone between the knots it names, on which the
//! crossings are found by bisection to [`CROSSING_RESOLUTION`] ([`Tracked::of_light`], which
//! takes any such light). A kind whose row has no light, a pulsar glitch timed in the radio with no
//! flux, is pushed at its arrival and cleared [`GLITCH_CLEAR_AFTER`] later
//! ([`Tracked::of_unlit`]).
//!
//! # The schedule
//!
//! An [`AlertSchedule`] belongs to one place. It holds every push not yet made, by time, then
//! event, then crossing, and [`AlertSchedule::advance`] makes those before the next observer's
//! time. Each crossing of an event is pushed at most once at a place, so a later scan that finds
//! the event in progress adds only the crossings no earlier scan gave (Design note 6). A crossing
//! before the schedule began is pushed when it began, since the event is then in progress, and an
//! event that cleared before it began is not pushed at all.
//!
//! The scans are asked for by [`ScanRequest`]s, each for a window of arrivals: the first over
//! [t, t + [`ALERT_LOOKAHEAD`]), then a renewal for the next look-ahead once the clock passes the
//! half-way point of what has been asked for (Design note 8), each from where the last ended and
//! merged in that order ([`AlertSchedule::merge`]). A merged scan's pushes that are already due are
//! made at the next advance with their own times, so the pushes are the same however the clock's
//! steps split the interval.
//!
//! A moved observer or a clock that runs backwards drops the schedule, with what it has pushed and
//! the scans it asked for, whose results it then refuses, and starts again at the new observer
//! ([`Rescan::Restart`]). Nothing is backfilled: light that passed the new place before the jump is
//! gone, and an event the crew outran is pushed again when its light catches up.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;
use std::sync::Arc;

use hyperion_sim::alerts::{
    AlertQuery, BuildAlertQueryError, ClassSet, Detection, DetectionLight, SensorHorizon,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::id::EventId;
use hyperion_sim::observe::Observer;
use hyperion_sim::time::{ClockWindow, Span, UniverseTime};
use hyperion_sim::units::WattsPerSquareMetre;

use crate::knowledge::KnowledgeLevel;

/// How far ahead a schedule's scans reach: one Julian year of the observer's time, 31,557,600 s
/// (Design note 8).
pub const ALERT_LOOKAHEAD: Span = Span::from_seconds(31_557_600);

/// Half of [`ALERT_LOOKAHEAD`]: past this much before the end of what has been asked for, the next
/// look-ahead is asked for.
const HALF_LOOKAHEAD: Span = Span::from_seconds(31_557_600 / 2);

/// How finely a crossing is found on a detection's light: 1 ms of the observer's time, under the
/// shortest event in the registry, a magnetar's short burst of at least 3.981 ms (plan 06,
/// P06.T28.c; [`MAGNETAR_BURST_PEAK`]).
///
/// [`MAGNETAR_BURST_PEAK`]: hyperion_sim::alerts::MAGNETAR_BURST_PEAK
pub const CROSSING_RESOLUTION: Span = match Span::new(0, 1_000_000) {
    Ok(span) => span,
    Err(_) => panic!("a millisecond is a span"),
};

/// How long after its arrival a detection whose row has no light clears: `τ_d`, the time of a
/// Crab-like glitch's exponential recovery, 3.1 days or 267,840 s (plan 06's
/// [`RECOVERY_TIME_DAYS`], `Glitch::recovery_time_seconds`), for Vela-like glitches too (lean,
/// 2026-10-31, on the owner's UX list). A glitch is timed in the radio with no flux, which would
/// otherwise clear at once (Design note 11).
///
/// [`RECOVERY_TIME_DAYS`]: hyperion_sim::stellar::events::glitches::RECOVERY_TIME_DAYS
pub const GLITCH_CLEAR_AFTER: Span = Span::from_seconds(267_840);

/// Nanoseconds in a second, for halving a span.
const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// A limit a detection's light crosses at the observer, in the order a rising light crosses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Crossing {
    /// The flux first reaches the sensor's detection limit: a bearing.
    Detect,
    /// The flux first reaches the resolve limit: the host is resolved.
    Resolve,
    /// The flux falls under the detection limit for the last time, or a glitch's recovery time has
    /// passed: the condition clears, and the contact leaves the alert list (Design note 11).
    Clear,
}

impl Crossing {
    /// Every crossing, in order.
    pub const ALL: [Self; 3] = [Self::Detect, Self::Resolve, Self::Clear];

    /// The Knowledge level a sighting at the crossing supports, or `None` for
    /// [`Clear`](Self::Clear), which is no sighting.
    #[must_use]
    pub const fn level(self) -> Option<KnowledgeLevel> {
        match self {
            Self::Detect => Some(KnowledgeLevel::Bearing),
            Self::Resolve => Some(KnowledgeLevel::Resolved),
            Self::Clear => None,
        }
    }

    /// The crossing's place in [`ALL`](Self::ALL).
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::Detect => 0,
            Self::Resolve => 1,
            Self::Clear => 2,
        }
    }
}

/// A detection with the times its light at the observer crosses its band's limits: what a
/// schedule holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Tracked {
    detection: Detection,
    /// Per crossing, in [`Crossing::ALL`]'s order: when, and the flux in the band then.
    crossings: [Option<(UniverseTime, WattsPerSquareMetre)>; 3],
}

impl Tracked {
    /// The track of `detection` on its own light (P12.T8.b.1): [`of_light`](Self::of_light) on
    /// its [`ArrivingLight`]'s knots and flux, or [`of_unlit`](Self::of_unlit) for a detection
    /// that gives none.
    ///
    /// `None` as those give it: for a band the sensor lacks, a light that reaches no limit at its
    /// knots, or an unlit detection of a kind whose row has light.
    ///
    /// [`ArrivingLight`]: hyperion_sim::alerts::ArrivingLight
    #[must_use]
    pub fn of(detection: Detection, sensor: &SensorHorizon) -> Option<Self> {
        match *detection.light() {
            DetectionLight::Lit(light) => {
                Self::of_light(detection, sensor, &light.knots(), |t| light.flux_at(t))
            }
            DetectionLight::Unlit => Self::of_unlit(detection),
        }
    }

    /// The track of `detection`, whose flux at the observer in its band is `light` (W m⁻², after
    /// extinction, at each observer time), monotone between consecutive `knots`.
    ///
    /// The detection is pushed when the light first reaches the sensor's detection limit, at the
    /// first knot if it is already there, and when it first reaches the resolve limit; it clears
    /// when the light falls under the detection limit after the last knot that reaches it, and
    /// never if the last knot does. Each crossing between knots is found by bisection, to
    /// [`CROSSING_RESOLUTION`], at a time when the light is past the limit. `light` is called at
    /// each knot once, then about 35 times for each crossing between knots a year apart.
    ///
    /// `None` if `sensor` lacks the detection's band, or if the light reaches its detection limit
    /// at no knot: the light the scan gives decides when a detection is seen, and the one it
    /// never shows is not listed.
    ///
    /// # Panics
    ///
    /// If `knots` is empty or not in strictly increasing order.
    #[must_use]
    pub fn of_light(
        detection: Detection,
        sensor: &SensorHorizon,
        knots: &[UniverseTime],
        mut light: impl FnMut(UniverseTime) -> WattsPerSquareMetre,
    ) -> Option<Self> {
        assert!(
            !knots.is_empty() && knots.windows(2).all(|pair| pair[0] < pair[1]),
            "a light's knots are not empty and strictly increasing"
        );
        let limits = sensor.limits(detection.band())?;
        let values: Vec<WattsPerSquareMetre> = knots.iter().map(|&t| light(t)).collect();
        let detect = first_reaching(knots, &values, limits.detect(), &mut light)?;
        let resolve = first_reaching(knots, &values, limits.resolve(), &mut light)
            .map(|(at, flux)| (at.max(detect.0), flux));
        let last = values
            .iter()
            .rposition(|&flux| reaches(flux, limits.detect()))
            .expect("the detection limit is reached at a knot");
        let clear = knots.get(last + 1).map(|&after| {
            bisect(
                knots[last],
                (after, values[last + 1]),
                |flux| !reaches(flux, limits.detect()),
                &mut light,
            )
        });
        Some(Self {
            detection,
            crossings: [Some(detect), resolve, clear],
        })
    }

    /// The track of `detection` whose kind's row has no light (a pulsar glitch, P12.T5.e): pushed
    /// at its arrival, resolved then if it is resolvable, which no glitch is, and cleared
    /// [`GLITCH_CLEAR_AFTER`] later with no flux.
    ///
    /// `None` for a kind whose row has a light curve, whose light [`of_light`](Self::of_light)
    /// takes.
    ///
    /// # Panics
    ///
    /// If the clearing falls off the clock, which days after an arrival on it cannot.
    #[must_use]
    pub fn of_unlit(detection: Detection) -> Option<Self> {
        if detection.kind().row().light_curve().is_some() {
            return None;
        }
        let seen = (detection.arrival(), detection.flux());
        let clear = detection
            .arrival()
            .checked_add(GLITCH_CLEAR_AFTER)
            .expect("days after an arrival on the clock is a time");
        Some(Self {
            crossings: [
                Some(seen),
                detection.is_resolvable().then_some(seen),
                Some((clear, WattsPerSquareMetre::new(0.0))),
            ],
            detection,
        })
    }

    /// The detection.
    #[must_use]
    pub const fn detection(&self) -> &Detection {
        &self.detection
    }

    /// When the light makes `crossing`, and the flux then, or `None` if it never does.
    #[must_use]
    pub const fn crossing(
        &self,
        crossing: Crossing,
    ) -> Option<(UniverseTime, WattsPerSquareMetre)> {
        self.crossings[crossing.index()]
    }
}

/// Whether `flux` reaches `limit`: false for a flux that is not a number.
#[must_use]
fn reaches(flux: WattsPerSquareMetre, limit: WattsPerSquareMetre) -> bool {
    flux >= limit
}

/// When the light whose values at `knots` are `values` first reaches `limit`, and its flux then:
/// the first knot if it is there, or by bisection before the first knot that reaches it.
#[must_use]
fn first_reaching(
    knots: &[UniverseTime],
    values: &[WattsPerSquareMetre],
    limit: WattsPerSquareMetre,
    light: &mut impl FnMut(UniverseTime) -> WattsPerSquareMetre,
) -> Option<(UniverseTime, WattsPerSquareMetre)> {
    let first = values.iter().position(|&flux| reaches(flux, limit))?;
    Some(match first.checked_sub(1) {
        None => (knots[0], values[0]),
        Some(before) => bisect(
            knots[before],
            (knots[first], values[first]),
            |flux| reaches(flux, limit),
            light,
        ),
    })
}

/// The first time after `fails`, to [`CROSSING_RESOLUTION`], at which the light `holds`, with its
/// flux then: it does not at `fails` and does at `holds_at`, the later, and changes monotonically
/// between them.
#[must_use]
fn bisect(
    mut fails: UniverseTime,
    mut holds_at: (UniverseTime, WattsPerSquareMetre),
    holds: impl Fn(WattsPerSquareMetre) -> bool,
    light: &mut impl FnMut(UniverseTime) -> WattsPerSquareMetre,
) -> (UniverseTime, WattsPerSquareMetre) {
    while holds_at
        .0
        .checked_since(fails)
        .expect("two clock times are a span apart")
        > CROSSING_RESOLUTION
    {
        let middle = midpoint(fails, holds_at.0);
        let flux = light(middle);
        if holds(flux) {
            holds_at = (middle, flux);
        } else {
            fails = middle;
        }
    }
    holds_at
}

/// The time half-way from `early` to `late`, floored to the nanosecond.
#[must_use]
fn midpoint(early: UniverseTime, late: UniverseTime) -> UniverseTime {
    let gap = late
        .checked_since(early)
        .expect("two clock times are a span apart");
    let total = i128::from(gap.seconds()) * NANOS_PER_SECOND + i128::from(gap.subsec_nanos());
    let half = total / 2;
    let seconds = i64::try_from(half.div_euclid(NANOS_PER_SECOND))
        .expect("half a span between clock times fits its seconds");
    let nanos = u32::try_from(half.rem_euclid(NANOS_PER_SECOND)).expect("under a second");
    early
        .checked_add(Span::new(seconds, nanos).expect("nanoseconds under a second"))
        .expect("a time between two clock times")
}

/// One push: a detection's light crossing a limit at the observer.
#[derive(Debug, Clone, PartialEq)]
pub struct Push {
    at: UniverseTime,
    crossing: Crossing,
    flux: WattsPerSquareMetre,
    detection: Arc<Detection>,
}

impl Push {
    /// When the push falls due, on the observer's clock: the crossing's time, or when the schedule
    /// began for a crossing before it.
    #[must_use]
    pub const fn at(&self) -> UniverseTime {
        self.at
    }

    /// The limit crossed.
    #[must_use]
    pub const fn crossing(&self) -> Crossing {
        self.crossing
    }

    /// The flux in the detection's band at the crossing itself, W m⁻², after extinction: at or
    /// past the limit it crosses for [`Detect`](Crossing::Detect) and
    /// [`Resolve`](Crossing::Resolve), and under the detection limit for
    /// [`Clear`](Crossing::Clear). For a crossing before the schedule began it is the flux then,
    /// not at [`at`](Self::at).
    #[must_use]
    pub const fn flux(&self) -> WattsPerSquareMetre {
        self.flux
    }

    /// The detection the scan found, its flux the peak's or the light at its window's start.
    #[must_use]
    pub fn detection(&self) -> &Detection {
        &self.detection
    }
}

/// Which of a schedule's lives a scan was asked for in: a moved observer or a clock that runs
/// backwards starts a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
struct Generation(u64);

/// A scan a schedule asks for: the detections whose light arrives at `position` in `window`, of
/// the subscription's sensor and classes ([`Subscription::query`]).
///
/// The first scan after a schedule starts must also list the onsets that arrived before the window
/// and cross a limit in it, a slow riser's, so its walk reaches back by the classes' longest rise
/// to peak (research's lean of 2026-10-30; P12.T8.b). Every scan asked for must be merged, empty
/// if it found nothing or failed, for the next to be accepted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScanRequest {
    generation: Generation,
    position: GalacticPosition,
    window: TimeWindow,
}

impl ScanRequest {
    /// Where the observer is.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// The arrivals to list, `[t₁, t₂)` of the observer's time, inside the clock window; empty for
    /// a schedule that starts at its end.
    #[must_use]
    pub const fn window(&self) -> TimeWindow {
        self.window
    }
}

/// A scan an advance asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rescan {
    /// The next look-ahead at the same place: the schedule keeps what it holds, and the scan
    /// extends it.
    Renew(ScanRequest),
    /// The observer moved or the clock ran backwards: the schedule was dropped and starts again
    /// with this scan.
    Restart(ScanRequest),
}

impl Rescan {
    /// The scan to run.
    #[must_use]
    pub const fn request(&self) -> &ScanRequest {
        match self {
            Self::Renew(request) | Self::Restart(request) => request,
        }
    }
}

/// What an advance from one observer to the next returns (Design note 9).
#[derive(Debug, Clone, PartialEq)]
#[must_use = "the pushes that fell due are made only once"]
pub struct Advance {
    due: Vec<Push>,
    rescan: Option<Rescan>,
}

impl Advance {
    /// The pushes that fell due, by time, then event, then crossing; any merged since the last
    /// advance with an earlier time come first.
    #[must_use]
    pub fn due(&self) -> &[Push] {
        &self.due
    }

    /// The scan the schedule needs, if it needs one.
    #[must_use]
    pub const fn rescan(&self) -> Option<&Rescan> {
        self.rescan.as_ref()
    }

    /// The pushes and the scan.
    #[must_use]
    pub fn into_parts(self) -> (Vec<Push>, Option<Rescan>) {
        (self.due, self.rescan)
    }
}

/// A scan's result could not be merged into a schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergeScanError {
    /// The scan was asked for before the schedule was dropped: it is of another place or time.
    Stale,
    /// The scan does not start where the schedule's merged scans end.
    OutOfOrder {
        /// Where the merged scans end.
        expected: UniverseTime,
        /// Where the scan starts.
        found: UniverseTime,
    },
}

impl fmt::Display for MergeScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale => f.write_str("the scan was asked for by a schedule since dropped"),
            Self::OutOfOrder { expected, found } => write!(
                f,
                "the scan starts at {found}, not where the merged scans end, {expected}"
            ),
        }
    }
}

impl Error for MergeScanError {}

/// The pushes to come at one place, from the detections of the scans merged so far (Design
/// note 8; module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct AlertSchedule {
    position: GalacticPosition,
    generation: Generation,
    /// When the schedule began at its place.
    start: UniverseTime,
    /// The merged scans' arrivals end here.
    covered: UniverseTime,
    /// The scans asked for end here.
    requested: UniverseTime,
    /// Every push not yet made.
    pending: BTreeMap<(UniverseTime, EventId, Crossing), Push>,
    /// Every crossing scheduled or pushed at this place, of the events a later scan could still
    /// find in progress.
    scheduled: BTreeSet<(EventId, Crossing)>,
    /// The clearing time of each event in `scheduled` that clears.
    clears: BTreeSet<(UniverseTime, EventId)>,
}

impl AlertSchedule {
    /// The empty schedule of `observer`'s place from its time, and the first scan it asks for,
    /// over the look-ahead.
    #[must_use]
    pub fn new(observer: &Observer) -> (Self, ScanRequest) {
        let mut schedule = Self {
            position: *observer.position(),
            generation: Generation::default(),
            start: observer.time(),
            covered: observer.time(),
            requested: observer.time(),
            pending: BTreeMap::new(),
            scheduled: BTreeSet::new(),
            clears: BTreeSet::new(),
        };
        let first = schedule.request(observer.time());
        (schedule, first)
    }

    /// The schedule's place.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// When the schedule began at its place.
    #[must_use]
    pub const fn start(&self) -> UniverseTime {
        self.start
    }

    /// The end of the arrivals its merged scans list: from the schedule's start to here.
    #[must_use]
    pub const fn covered(&self) -> UniverseTime {
        self.covered
    }

    /// How many pushes are still to be made.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Advances the observer from `previous`, the observer the last advance ended at (or the
    /// schedule's), to `next`: the pushes that fall due before `next`'s time, and the scan the
    /// schedule then needs (Design notes 8 and 9).
    ///
    /// If `next` is at another place, or its clock reads earlier than `previous`'s, the schedule
    /// is dropped and nothing of it is pushed: it starts again at `next` and asks for its first
    /// scan ([`Rescan::Restart`]). Otherwise every push before `next`'s time is due, and once
    /// `next` is past the half-way point of the last look-ahead asked for, the next is asked for
    /// ([`Rescan::Renew`]), reaching a look-ahead past `next`. An advance to the same time pushes
    /// only what a merge has made due since.
    pub fn advance(&mut self, previous: &Observer, next: &Observer) -> Advance {
        debug_assert_eq!(
            previous.position(),
            &self.position,
            "an advance starts at the schedule's place"
        );
        if next.position() != &self.position || next.time() < previous.time() {
            return Advance {
                due: Vec::new(),
                rescan: Some(Rescan::Restart(self.restart(next))),
            };
        }
        let mut due = Vec::new();
        while let Some(first) = self.pending.first_entry() {
            if first.key().0 >= next.time() {
                break;
            }
            due.push(first.remove());
        }
        Advance {
            due,
            rescan: self.renewal(next.time()).map(Rescan::Renew),
        }
    }

    /// Merges what `scan` found, the tracks of its detections, into the schedule.
    ///
    /// A crossing of an event the schedule has already scheduled or pushed at this place is not
    /// added again. A crossing before the schedule began is pushed when it began, unless the event
    /// cleared by then, when none of its crossings is.
    ///
    /// # Errors
    ///
    /// [`MergeScanError::Stale`] if the schedule has been dropped since it asked for `scan`, and
    /// [`MergeScanError::OutOfOrder`] if `scan` does not start where the merged scans end; nothing
    /// is merged then.
    pub fn merge(
        &mut self,
        scan: &ScanRequest,
        found: impl IntoIterator<Item = Tracked>,
    ) -> Result<(), MergeScanError> {
        if scan.generation != self.generation {
            return Err(MergeScanError::Stale);
        }
        if scan.window.start() != self.covered {
            return Err(MergeScanError::OutOfOrder {
                expected: self.covered,
                found: scan.window.start(),
            });
        }
        for track in found {
            self.insert(track);
        }
        self.covered = scan.window.end();
        self.forget_cleared();
        Ok(())
    }

    /// Schedules `track`'s crossings that are not yet scheduled.
    fn insert(&mut self, track: Tracked) {
        let event = track.detection.event();
        let clear = track.crossing(Crossing::Clear).map(|(at, _)| at);
        if clear.is_some_and(|at| at <= self.start) {
            return;
        }
        let detection = Arc::new(track.detection);
        for crossing in Crossing::ALL {
            let Some((at, flux)) = track.crossings[crossing.index()] else {
                continue;
            };
            if !self.scheduled.insert((event, crossing)) {
                continue;
            }
            let at = at.max(self.start);
            let push = Push {
                at,
                crossing,
                flux,
                detection: Arc::clone(&detection),
            };
            self.pending.insert((at, event, crossing), push);
        }
        if let Some(at) = clear {
            self.clears.insert((at, event));
        }
    }

    /// Forgets the crossings of the events that clear before the merged scans end.
    ///
    /// Every later scan starts at or after that end, and lists such an event in progress only if
    /// its light is still at the detection limit at its start, which it is not after its last fall
    /// under it. The pushes still to be made stay.
    fn forget_cleared(&mut self) {
        while let Some(&(at, event)) = self.clears.first() {
            if at >= self.covered {
                break;
            }
            self.clears.pop_first();
            for crossing in Crossing::ALL {
                self.scheduled.remove(&(event, crossing));
            }
        }
    }

    /// Drops the schedule and starts it again at `next`: its first scan.
    fn restart(&mut self, next: &Observer) -> ScanRequest {
        // Some 10¹⁹ restarts would wrap the count, far more than a server makes.
        self.generation = Generation(self.generation.0.wrapping_add(1));
        self.position = *next.position();
        self.start = next.time();
        self.covered = next.time();
        self.requested = next.time();
        self.pending.clear();
        self.scheduled.clear();
        self.clears.clear();
        self.request(next.time())
    }

    /// The renewal to ask for at `now`, if `now` is past the half-way point of the last look-ahead
    /// asked for and the clock window has more.
    fn renewal(&mut self, now: UniverseTime) -> Option<ScanRequest> {
        if self.requested >= ClockWindow::END {
            return None;
        }
        let half_way = self
            .requested
            .checked_sub(HALF_LOOKAHEAD)
            .expect("half a year before a clock time is a time");
        (now >= half_way).then(|| self.request(self.requested.max(now)))
    }

    /// Asks for the scan from where the last asked for ends to a look-ahead past `from`, inside the
    /// clock window.
    fn request(&mut self, from: UniverseTime) -> ScanRequest {
        let end = from
            .checked_add(ALERT_LOOKAHEAD)
            .expect("a year after a clock time is a time")
            .min(ClockWindow::END);
        let window = TimeWindow::new(self.requested, end)
            .expect("the scans asked for end inside the clock window");
        self.requested = end;
        ScanRequest {
            generation: self.generation,
            position: self.position,
            window,
        }
    }
}

/// One subscriber's alerts: its observer, its sensor, the classes it asks about with their host
/// limit, and its schedule (plan 12's Provides; Design note 9).
///
/// The observer is the client's until sessions exist, moved by `alerts_observer`; a session's clock
/// will call [`advance`](Self::advance) the same way.
#[derive(Debug, Clone, PartialEq)]
pub struct Subscription {
    observer: Observer,
    sensor: SensorHorizon,
    classes: ClassSet,
    host_limit: NonZeroU32,
    schedule: AlertSchedule,
}

impl Subscription {
    /// The subscription of `sensor` at `observer` to `classes`, each walked while it expects at
    /// most `host_limit` hosts in reach (Design note 7), and the first scan it asks for.
    #[must_use]
    pub fn new(
        observer: Observer,
        sensor: SensorHorizon,
        classes: ClassSet,
        host_limit: NonZeroU32,
    ) -> (Self, ScanRequest) {
        let (schedule, first) = AlertSchedule::new(&observer);
        let subscription = Self {
            observer,
            sensor,
            classes,
            host_limit,
            schedule,
        };
        (subscription, first)
    }

    /// Where and when the observer is, as the last advance left it.
    #[must_use]
    pub const fn observer(&self) -> &Observer {
        &self.observer
    }

    /// The sensor's limits.
    #[must_use]
    pub const fn sensor(&self) -> &SensorHorizon {
        &self.sensor
    }

    /// The classes asked about.
    #[must_use]
    pub const fn classes(&self) -> ClassSet {
        self.classes
    }

    /// The most hosts a class may expect in its reach for it to be walked.
    #[must_use]
    pub const fn host_limit(&self) -> NonZeroU32 {
        self.host_limit
    }

    /// The schedule.
    #[must_use]
    pub const fn schedule(&self) -> &AlertSchedule {
        &self.schedule
    }

    /// The sim's query for `scan`: its place and window, with the subscription's sensor, classes
    /// and host limit.
    ///
    /// # Errors
    ///
    /// As [`AlertQuery`]'s builder, which a scan the schedule asks for never trips: its place is
    /// an observer's and its window inside the clock window.
    pub fn query(&self, scan: &ScanRequest) -> Result<AlertQuery, BuildAlertQueryError> {
        AlertQuery::builder(scan.position, scan.window, self.sensor.clone())
            .classes(self.classes)
            .host_limit(self.host_limit)
            .build()
    }

    /// Moves the observer to `next`: [`AlertSchedule::advance`] from the observer the last
    /// advance left.
    pub fn advance(&mut self, next: Observer) -> Advance {
        let advance = self.schedule.advance(&self.observer, &next);
        self.observer = next;
        advance
    }

    /// Merges what `scan` found into the schedule ([`AlertSchedule::merge`]).
    ///
    /// # Errors
    ///
    /// As [`AlertSchedule::merge`].
    pub fn merge(
        &mut self,
        scan: &ScanRequest,
        found: impl IntoIterator<Item = Tracked>,
    ) -> Result<(), MergeScanError> {
        self.schedule.merge(scan, found)
    }
}

#[cfg(test)]
mod tests;
