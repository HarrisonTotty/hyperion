//! A subscription's schedule of pushes, and the pure advance over it (plan 12, P12.T8.a; Design
//! notes 6, 8 and 9).
//!
//! # The crossing rule
//!
//! A detection's flux is its peak's for an onset and its light at the window's start for an event
//! in progress, and decides only whether the walk lists it. The schedule pushes a detection each
//! time its flux at the observer rises to the sensor's detection limit in its band, as a bearing
//! ([`Crossing::Detect`]), each time it rises to the resolve limit ([`Crossing::Resolve`]), and
//! each time it falls under the detection limit ([`Crossing::Clear`]): the condition clears, as
//! the UX guide's alerts and Design note 11 say. So a riser is pushed at its crossings, never at its
//! arrival: an FU Orionis outburst's peak arrives up to 25 years after its onset, and a Type Ia's
//! first light is days under the limit (research's lean of 2026-10-30 for T8, adopted). And a light
//! that dips under the limit and rises again is cleared and pushed again, as the two peaks of a
//! supernova whose envelope was partly stripped are, or a stripped supernova's breakout before its
//! peak (research's ruling of 2026-10-31, which overturned clearing at the last fall).
//!
//! Each walk's detection carries its light at the observer (P12.T8.b.1), which [`Tracked::of`]
//! takes: a function of the observer's time, monotone between the knots it names
//! ([`Tracked::of_light`], which takes any such light). Each crossing between knots is found by
//! bisection on the clock's grid of [`CROSSING_RESOLUTION`]: it is the first point of the grid
//! at which the light is past the limit, which depends on the light alone and not on the knots that
//! bracket it. A kind whose row has no light, a pulsar glitch timed in the radio with no flux, is
//! pushed at its arrival and cleared [`GLITCH_CLEAR_AFTER`] later ([`Tracked::of_unlit`]).
//!
//! # The schedule
//!
//! An [`AlertSchedule`] belongs to one place. It holds every push not yet made, by time, then
//! event, then crossing, and [`AlertSchedule::advance`] makes those before the next observer's
//! time. A merged scan schedules the crossings whose times lie in its own window, Design note 6's
//! rule for arrivals applied to crossings: each crossing is pushed by exactly one scan, however the
//! windows split the light, and a later scan that lists an event in progress adds only its
//! crossings in its own window. The first scan after the schedule starts also schedules the state
//! at the start: an event already past the detection limit is pushed then, resolved too if it is
//! past the resolve limit, and an event under it then is not, whatever it did before.
//!
//! For that, a scan lists every event whose light may reach the detection limit inside its window,
//! and gives each the same light whichever scan lists it (P12.T8.b's contract).
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

use std::collections::BTreeMap;
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

/// The clock's grid on which a crossing of a detection's light is found: 1 ms of the observer's
/// time, under the shortest event in the registry, a magnetar's short burst of at least 3.981 ms (plan 06,
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
    /// The flux rises to the sensor's detection limit: a bearing.
    Detect,
    /// The flux rises to the resolve limit: the host is resolved.
    Resolve,
    /// The flux falls under the detection limit, or a glitch's recovery time has passed: the
    /// condition clears, and the contact leaves the alert list (Design note 11).
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
}

/// A crossing of a detection's light: when, which limit, and the flux in the band then.
pub type CrossingAt = (UniverseTime, Crossing, WattsPerSquareMetre);

/// A detection with the times its light at the observer crosses its band's limits: what a
/// schedule holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Tracked {
    detection: Detection,
    /// Every crossing, in time order, then in [`Crossing`]'s.
    crossings: Vec<CrossingAt>,
    /// Each time the light falls under the resolve limit and stays at the detection limit: no
    /// push, but the host is no longer resolved there.
    unresolved: Vec<UniverseTime>,
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
    /// Every crossing is found, in time order: a [`Detect`](Crossing::Detect) at each rise to
    /// the sensor's detection limit, at the first knot if the light is already there; a
    /// [`Resolve`](Crossing::Resolve) at each rise to the resolve limit; and a
    /// [`Clear`](Crossing::Clear) at each fall under the detection limit (module documentation).
    /// Each crossing between knots is the first point of the clock's grid of
    /// [`CROSSING_RESOLUTION`] at which the light is past the limit, or the later knot if no point
    /// of the grid lies between them; `light` is called at each knot once, then about 35 times for
    /// each crossing between knots a year apart. Being monotone between knots, the light crosses
    /// each limit at most once between two of them.
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
        let (detect, resolve) = (limits.detect(), limits.resolve());
        let mut crossings = Vec::new();
        let mut unresolved = Vec::new();
        let (mut detected, mut resolved) = (false, false);
        let mut before: Option<UniverseTime> = None;
        for &knot in knots {
            let flux = light(knot);
            let (now_detected, now_resolved) = (reaches(flux, detect), reaches(flux, resolve));
            // The first point of the grid since the knot before at which `holds` does, or this knot.
            let mut crossed = |holds: &dyn Fn(WattsPerSquareMetre) -> bool| match before {
                None => (knot, flux),
                Some(earlier) => first_on_grid(earlier, (knot, flux), holds, &mut light),
            };
            if now_detected && !detected {
                let (at, seen) = crossed(&|flux| reaches(flux, detect));
                crossings.push((at, Crossing::Detect, seen));
            }
            if now_resolved && !resolved {
                let (at, seen) = crossed(&|flux| reaches(flux, resolve));
                // A rise through both in one piece resolves no earlier than it detects.
                let at = crossings
                    .last()
                    .map_or(at, |&(detected_at, ..)| at.max(detected_at));
                crossings.push((at, Crossing::Resolve, seen));
            }
            if detected && !now_detected {
                let (at, seen) = crossed(&|flux| !reaches(flux, detect));
                crossings.push((at, Crossing::Clear, seen));
            } else if resolved && !now_resolved {
                unresolved.push(crossed(&|flux| !reaches(flux, resolve)).0);
            }
            (detected, resolved, before) = (now_detected, now_resolved, Some(knot));
        }
        if crossings.is_empty() {
            return None;
        }
        crossings.sort_by_key(|&(at, crossing, _)| (at, crossing));
        Some(Self {
            detection,
            crossings,
            unresolved,
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
        let (arrival, flux) = (detection.arrival(), detection.flux());
        let clear = arrival
            .checked_add(GLITCH_CLEAR_AFTER)
            .expect("days after an arrival on the clock is a time");
        let mut crossings = vec![(arrival, Crossing::Detect, flux)];
        if detection.is_resolvable() {
            crossings.push((arrival, Crossing::Resolve, flux));
        }
        crossings.push((clear, Crossing::Clear, WattsPerSquareMetre::new(0.0)));
        Some(Self {
            detection,
            crossings,
            unresolved: Vec::new(),
        })
    }

    /// The detection.
    #[must_use]
    pub const fn detection(&self) -> &Detection {
        &self.detection
    }

    /// Every crossing of the light, in time order, then in [`Crossing`]'s order.
    #[must_use]
    pub fn crossings(&self) -> &[CrossingAt] {
        &self.crossings
    }

    /// Where the light stands at `t`, after every crossing at or before it: the latest
    /// [`Detect`](Crossing::Detect) if it is past the detection limit, and the latest
    /// [`Resolve`](Crossing::Resolve) too if it is past the resolve limit.
    #[must_use]
    fn state_at(&self, t: UniverseTime) -> (Option<CrossingAt>, Option<CrossingAt>) {
        let (mut detected, mut resolved) = (None, None);
        for &crossing in self.crossings.iter().take_while(|&&(at, ..)| at <= t) {
            match crossing.1 {
                Crossing::Detect => detected = Some(crossing),
                Crossing::Resolve => resolved = Some(crossing),
                Crossing::Clear => (detected, resolved) = (None, None),
            }
        }
        let last_unresolved = self
            .unresolved
            .iter()
            .copied()
            .take_while(|&at| at <= t)
            .last();
        if let (Some((resolved_at, ..)), Some(unresolved_at)) = (resolved, last_unresolved)
            && unresolved_at > resolved_at
        {
            resolved = None;
        }
        (detected, resolved.filter(|_| detected.is_some()))
    }
}

/// Whether `flux` reaches `limit`: false for a flux that is not a number.
#[must_use]
fn reaches(flux: WattsPerSquareMetre, limit: WattsPerSquareMetre) -> bool {
    flux >= limit
}

/// Nanoseconds in [`CROSSING_RESOLUTION`].
const GRID_NANOS: i128 = 1_000_000;

/// The first point of the clock's grid of [`CROSSING_RESOLUTION`] after `fails` at which the light
/// `holds`, with its flux then, or `holds_at` if none before it does: the light does not hold at
/// `fails`, does at `holds_at`, the later, and changes monotonically between them, so the point
/// depends on the light alone, not on the ends that bracket it.
#[must_use]
fn first_on_grid(
    fails: UniverseTime,
    holds_at: (UniverseTime, WattsPerSquareMetre),
    holds: &dyn Fn(WattsPerSquareMetre) -> bool,
    light: &mut impl FnMut(UniverseTime) -> WattsPerSquareMetre,
) -> (UniverseTime, WattsPerSquareMetre) {
    // The grid's points strictly between the ends, by index: [first, after).
    let first = nanos(fails).div_euclid(GRID_NANOS) + 1;
    let after = (nanos(holds_at.0) - 1).div_euclid(GRID_NANOS) + 1;
    let (mut low, mut high) = (first, after.max(first));
    let mut found = holds_at;
    while low < high {
        let middle = low + (high - low) / 2;
        let at = from_nanos(middle * GRID_NANOS);
        let flux = light(at);
        if holds(flux) {
            (high, found) = (middle, (at, flux));
        } else {
            low = middle + 1;
        }
    }
    found
}

/// `t` in nanoseconds since the epoch.
#[must_use]
fn nanos(t: UniverseTime) -> i128 {
    i128::from(t.seconds()) * NANOS_PER_SECOND + i128::from(t.subsec_nanos())
}

/// The time `nanos` nanoseconds after the epoch.
#[must_use]
fn from_nanos(nanos: i128) -> UniverseTime {
    let seconds = i64::try_from(nanos.div_euclid(NANOS_PER_SECOND))
        .expect("a time between two clock times fits its seconds");
    let nanos = u32::try_from(nanos.rem_euclid(NANOS_PER_SECOND)).expect("under a second");
    UniverseTime::new(seconds, nanos).expect("nanoseconds under a second")
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
    /// began for the state the light was in then.
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
    /// [`Clear`](Crossing::Clear). For the state when the schedule began it is the flux at the
    /// crossing that took the light there, not at [`at`](Self::at).
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
/// A scan lists every event whose light may reach the detection limit inside its window (P12.T8.b's
/// contract): those in progress at its start, those whose onset arrived before it and that cross a
/// limit in it, a slow riser's, for which its walk reaches back by the classes' longest rise to
/// peak (`class_longest_rise`; research's lean of 2026-10-30), and those under the limit at its
/// start that rise again in it. It gives each the same light whichever scan lists it, so that a
/// crossing near a window's edge falls in one window alone. Every scan asked for must be merged,
/// empty if it found nothing or failed, for the next to be accepted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScanRequest {
    generation: Generation,
    position: GalacticPosition,
    window: TimeWindow,
    first: bool,
}

impl ScanRequest {
    /// Whether it is the first scan after the schedule started or restarted, whose walk reaches
    /// back by the classes' longest rise and whose merge schedules the state at the start.
    #[must_use]
    pub const fn starts_schedule(&self) -> bool {
        self.first
    }

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
    /// The tracks with crossings past the merged scans' end, each as its event was first listed:
    /// a later scan's track of the same event gives way to it, so that each crossing has one
    /// owner whatever light the later scan gives (P12.T8.b; T5.d's two lights of a local event).
    carried: BTreeMap<EventId, Tracked>,
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
            carried: BTreeMap::new(),
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
    /// The crossings whose times lie in the scan's window are scheduled, and none other: a later
    /// scan that lists an event in progress adds only its own window's crossings. The first scan
    /// after the schedule starts also schedules each event's state at the start, a
    /// [`Detect`](Crossing::Detect) then if its light is past the detection limit, and a
    /// [`Resolve`](Crossing::Resolve) if it is past the resolve limit, with the flux of the
    /// crossings that took it there.
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
        let (from, to) = (scan.window.start(), scan.window.end());
        let owned = self.owned(found);
        let carried = std::mem::take(&mut self.carried).into_values();
        let tracks = carried.map(|track| (track, false));
        for (track, first) in tracks.chain(owned.into_iter().map(|track| (track, scan.first))) {
            let event = track.detection.event();
            if let Some(later) = self.insert(track, from, to, first) {
                self.carried.insert(event, later);
            }
        }
        self.covered = to;
        Ok(())
    }

    /// Merges what a scan already merged found late, the tracks of its classes walked in the
    /// background (Design note 7): their crossings in the scan's own window, and their state at the
    /// start for the first scan. Those already due are made at the next advance with their own
    /// times. None is carried: a later scan's own background walk lists what crosses in its
    /// window, on the same light, since a background class's light does not depend on the window
    /// (P12.T8.b), so each late merge keeps to its window and none pushes a crossing twice.
    ///
    /// # Errors
    ///
    /// [`MergeScanError::Stale`] if the schedule has been dropped since it asked for `scan`, and
    /// [`MergeScanError::OutOfOrder`] if `scan` is not merged yet; nothing is merged then.
    pub fn merge_late(
        &mut self,
        scan: &ScanRequest,
        found: impl IntoIterator<Item = Tracked>,
    ) -> Result<(), MergeScanError> {
        if scan.generation != self.generation {
            return Err(MergeScanError::Stale);
        }
        if scan.window.end() > self.covered {
            return Err(MergeScanError::OutOfOrder {
                expected: self.covered,
                found: scan.window.start(),
            });
        }
        for track in self.owned(found) {
            // Not carried: the module documentation's late merge keeps to its own window.
            let _later = self.insert(track, scan.window.start(), scan.window.end(), scan.first);
        }
        Ok(())
    }

    /// The tracks of `found` the schedule takes: one per event, the first listed, and none of an
    /// event it carries.
    fn owned(&self, found: impl IntoIterator<Item = Tracked>) -> Vec<Tracked> {
        let mut events = std::collections::BTreeSet::new();
        found
            .into_iter()
            .filter(|track| {
                let event = track.detection.event();
                !self.carried.contains_key(&event) && events.insert(event)
            })
            .collect()
    }

    /// Schedules `track`'s crossings in `[from, to)`, and its state at the start for the `first`
    /// scan: the track again if it crosses from `to` on, for a merge to carry.
    #[must_use]
    fn insert(
        &mut self,
        track: Tracked,
        from: UniverseTime,
        to: UniverseTime,
        first: bool,
    ) -> Option<Tracked> {
        let event = track.detection.event();
        let state = first.then(|| track.state_at(self.start));
        let in_window: Vec<CrossingAt> = track
            .crossings
            .iter()
            .copied()
            // The first scan's crossings at the start are its state there.
            .filter(|&(at, ..)| from <= at && at < to && !(first && at == self.start))
            .collect();
        let later = track
            .crossings
            .iter()
            .any(|&(at, ..)| at >= to)
            .then(|| track.clone());
        let detection = Arc::new(track.detection);
        let at_start = state.into_iter().flat_map(|(detected, resolved)| {
            [detected, resolved]
                .into_iter()
                .flatten()
                .map(|(_, crossing, flux)| (self.start, crossing, flux))
        });
        for (at, crossing, flux) in at_start.collect::<Vec<_>>().into_iter().chain(in_window) {
            let push = Push {
                at,
                crossing,
                flux,
                detection: Arc::clone(&detection),
            };
            self.pending.insert((at, event, crossing), push);
        }
        later
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
        self.carried.clear();
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
        let first = self.requested == self.start;
        self.requested = end;
        ScanRequest {
            generation: self.generation,
            position: self.position,
            window,
            first,
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

    /// Merges what a scan already merged found late ([`AlertSchedule::merge_late`]).
    ///
    /// # Errors
    ///
    /// As [`AlertSchedule::merge_late`].
    pub fn merge_late(
        &mut self,
        scan: &ScanRequest,
        found: impl IntoIterator<Item = Tracked>,
    ) -> Result<(), MergeScanError> {
        self.schedule.merge_late(scan, found)
    }
}

#[cfg(test)]
mod tests;
