//! Events in time: the two constructions every event kind is built from.
//!
//! "A galaxy in which nothing happens during a campaign is not realistic" (brainstorm, "Events in
//! time"). A system's or body's own events are a pure function of seed, subject, tag and time,
//! without stored state, built by one of two constructions, both exact and open to random access:
//!
//! - [`PoissonBins`], for memoryless events. Time is cut into bins of a whole number of seconds,
//!   and the count and times of the events in bin k are a pure function of (seed, subject, tag,
//!   k). A varying rate is thinned within the bin, and durations are found by looking back a fixed
//!   number of bins.
//! - [`MonotonePhase`], for cycles of accumulation and release. An event falls where the clock's
//!   phase plus a bounded-slope noise crosses an integer, which keeps events in order while letting
//!   the phase wander, with an optional [`SkipMark`] for strongly irregular sources.
//!
//! Both key their streams through plan 01's two-step [`EventKey`]. An [`EventSeries`] holds the
//! key of one subject under one [`EventTag`]; each bin or cycle has its own stream, and each event
//! its own stream for its marks, so marks never shift times, and an [`EventId`] names one event
//! that can be regenerated on its own. Nothing here keeps state between calls: a listing is the same
//! whatever was asked before it, and no event leaves a mark that needs history replayed.

mod bins;
mod phase;
pub mod tags;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

use std::error::Error;
use std::fmt;

pub use bins::{BinEvent, BuildPoissonBinsError, EventsPerSecond, PoissonBins, RateModel};
pub use phase::{
    BuildLinearClockError, BuildMonotonePhaseError, CycleEvent, LinearClock, MonotonePhase, Phase,
    PhaseClock, SkipMark, octaves_to_span,
};

pub use crate::id::{EventBin, EventId, EventSubject, EventTag, EventWord};
pub use crate::rng::EventKey;
use crate::rng::Seed;
use crate::time::{NANOS_PER_SECOND, UniverseTime};

/// A [`TimeWindow`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildTimeWindowError {
    /// The end lies before the start.
    EndBeforeStart {
        /// The requested start.
        start: UniverseTime,
        /// The requested end.
        end: UniverseTime,
    },
}

impl fmt::Display for BuildTimeWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndBeforeStart { start, end } => {
                write!(f, "time window ends at {end}, before its start at {start}")
            }
        }
    }
}

impl Error for BuildTimeWindowError {}

/// A half-open interval of the universe clock, `[start, end)`.
///
/// Half-open so that consecutive windows partition time: an event at an instant belongs to exactly
/// one of them. A window whose start equals its end is empty. The [`Default`] is the empty window at
/// the epoch.
///
/// # Examples
///
/// ```
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::time::UniverseTime;
///
/// let start = UniverseTime::from_julian_years(-1).ok_or("in range")?;
/// let window = TimeWindow::new(start, UniverseTime::EPOCH)?;
/// assert!(window.contains(start));
/// assert!(!window.contains(UniverseTime::EPOCH));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeWindow {
    start: UniverseTime,
    end: UniverseTime,
}

impl TimeWindow {
    /// The window `[start, end)`.
    ///
    /// # Errors
    ///
    /// [`BuildTimeWindowError::EndBeforeStart`] if `end < start`.
    pub fn new(start: UniverseTime, end: UniverseTime) -> Result<Self, BuildTimeWindowError> {
        if end < start {
            return Err(BuildTimeWindowError::EndBeforeStart { start, end });
        }
        Ok(Self { start, end })
    }

    /// The first instant in the window.
    #[must_use]
    pub const fn start(self) -> UniverseTime {
        self.start
    }

    /// The first instant after the window.
    #[must_use]
    pub const fn end(self) -> UniverseTime {
        self.end
    }

    /// Whether `t` lies in `[start, end)`.
    #[must_use]
    pub fn contains(self, t: UniverseTime) -> bool {
        self.start <= t && t < self.end
    }

    /// Whether the window holds no instant.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// The events of one subject under one event tag: the subject, the tag and their [`EventKey`].
///
/// Built once from the seed so that the key always matches the subject and tag it names, and handed
/// to [`PoissonBins`] or [`MonotonePhase`], which need all three to build [`EventId`]s.
///
/// A tag's series goes to one construction only. The two share the key's slots with different
/// meanings: bin k's count stream is cycle k's skip stream, and a Poisson event j's marks are the
/// phase's lattice values of octave j − 1.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventBin, EventSeries, tags};
/// use hyperion_sim::id::{BodyId, SystemId};
///
/// let star = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let flares = EventSeries::new(Seed::new(7), tags::STAR_FLARE, star.into());
/// let id = flares.event_id(EventBin::new(12)?, 3);
/// assert!(flares.names(id));
/// assert_eq!(id.word().number(), 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventSeries {
    subject: EventSubject,
    tag: EventTag,
    key: EventKey,
}

impl EventSeries {
    /// The events of `subject` under `tag` in the universe of `seed`.
    #[must_use]
    pub fn new(seed: Seed, tag: EventTag, subject: EventSubject) -> Self {
        Self {
            subject,
            tag,
            key: EventKey::derive(seed, tag, subject),
        }
    }

    /// The system or body the events happen to.
    #[must_use]
    pub const fn subject(&self) -> EventSubject {
        self.subject
    }

    /// The kind of event.
    #[must_use]
    pub const fn tag(&self) -> EventTag {
        self.tag
    }

    /// The event key the streams are opened under.
    #[must_use]
    pub const fn key(&self) -> &EventKey {
        &self.key
    }

    /// The ID of event `number` of bin or cycle `bin` in this series.
    #[must_use]
    pub fn event_id(&self, bin: EventBin, number: u8) -> EventId {
        event_id(self.subject, self.tag, bin, number)
    }

    /// Whether `id` names an event of this series: its subject and its tag.
    #[must_use]
    pub fn names(&self, id: EventId) -> bool {
        id.subject() == self.subject && id.word().tag() == self.tag
    }
}

/// The ID of event `number` of bin or cycle `bin` of `subject` under `tag`:
/// `EventId::new(subject, EventWord::new(tag, bin, number))`.
#[must_use]
pub fn event_id(subject: EventSubject, tag: EventTag, bin: EventBin, number: u8) -> EventId {
    EventId::new(subject, EventWord::new(tag, bin, number))
}

/// Nanoseconds in one second, as the `i128` that time arithmetic here runs in.
const NANOS: i128 = 1_000_000_000;

/// `t` as nanoseconds since the epoch. Exact: an `i64` of seconds times 10⁹ fits in 94 bits.
#[must_use]
pub(crate) fn to_nanos(t: UniverseTime) -> i128 {
    i128::from(t.seconds()) * NANOS + i128::from(t.subsec_nanos())
}

/// The instant `nanos` nanoseconds from the epoch, or `None` beyond the clock's `i64` of seconds.
#[must_use]
pub(crate) fn from_nanos(nanos: i128) -> Option<UniverseTime> {
    // One 128-bit division, not two: the root finders call this on every step.
    let whole = nanos.div_euclid(NANOS);
    let seconds = i64::try_from(whole).ok()?;
    let subsec =
        u32::try_from(nanos - whole * NANOS).expect("a Euclidean remainder by 10⁹ lies in 0..10⁹");
    debug_assert!(subsec < NANOS_PER_SECOND);
    UniverseTime::new(seconds, subsec).ok()
}

/// The instant `nanos` nanoseconds from the epoch, clamped to the clock's range.
#[must_use]
pub(crate) fn from_nanos_saturating(nanos: i128) -> UniverseTime {
    let lo = to_nanos(UniverseTime::new(i64::MIN, 0).expect("0 ns is in range"));
    let hi = to_nanos(
        UniverseTime::new(i64::MAX, NANOS_PER_SECOND - 1).expect("10⁹ − 1 ns is in range"),
    );
    from_nanos(nanos.clamp(lo, hi)).expect("a clamped instant is representable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{BodyId, SystemId, event_tags};

    fn system() -> SystemId {
        SystemId::from_raw(0x0200_0800_2000_0000).unwrap()
    }

    #[test]
    fn a_window_is_half_open_and_rejects_a_reversed_pair() {
        let a = UniverseTime::new(-5, 0).unwrap();
        let b = UniverseTime::new(7, 250).unwrap();
        let window = TimeWindow::new(a, b).unwrap();
        assert!(window.contains(a));
        assert!(!window.contains(b));
        assert!(window.contains(UniverseTime::new(7, 249).unwrap()));
        assert!(!window.is_empty());
        assert!(TimeWindow::new(a, a).unwrap().is_empty());
        assert!(!TimeWindow::new(a, a).unwrap().contains(a));
        assert_eq!(
            TimeWindow::new(b, a),
            Err(BuildTimeWindowError::EndBeforeStart { start: b, end: a })
        );
        assert_eq!(
            BuildTimeWindowError::EndBeforeStart { start: b, end: a }.to_string(),
            "time window ends at T-5.000000000 s, before its start at T+7.000000250 s"
        );
    }

    #[test]
    fn a_series_names_only_its_own_events() {
        let seed = Seed::new(3);
        let body = BodyId::new(system(), 0);
        let series = EventSeries::new(seed, tags::STAR_FLARE, body.into());
        let bin = EventBin::new(-4).unwrap();
        let id = series.event_id(bin, 9);
        assert_eq!(id, event_id(body.into(), tags::STAR_FLARE, bin, 9));
        assert!(series.names(id));
        assert!(!series.names(event_id(system().into(), tags::STAR_FLARE, bin, 9)));
        assert!(!series.names(event_id(body.into(), event_tags::SELF_TEST, bin, 9)));
        assert_eq!(
            *series.key(),
            EventKey::derive(seed, tags::STAR_FLARE, body.into())
        );
        assert_eq!(
            (series.subject(), series.tag()),
            (body.into(), tags::STAR_FLARE)
        );
    }

    #[test]
    fn nanoseconds_round_trip_across_the_epoch_and_the_clocks_ends() {
        for (seconds, nanos) in [
            (0, 0),
            (-1, 999_999_999),
            (-1, 0),
            (12, 5),
            (i64::MIN, 0),
            (i64::MAX, 999_999_999),
        ] {
            let t = UniverseTime::new(seconds, nanos).unwrap();
            assert_eq!(from_nanos(to_nanos(t)), Some(t));
            assert_eq!(from_nanos_saturating(to_nanos(t)), t);
        }
        assert_eq!(to_nanos(UniverseTime::new(-1, 999_999_999).unwrap()), -1);
        let beyond = to_nanos(UniverseTime::new(i64::MAX, 999_999_999).unwrap()) + 1;
        assert_eq!(from_nanos(beyond), None);
        assert_eq!(
            from_nanos_saturating(beyond),
            UniverseTime::new(i64::MAX, 999_999_999).unwrap()
        );
        assert_eq!(
            from_nanos_saturating(-beyond - 5),
            UniverseTime::new(i64::MIN, 0).unwrap()
        );
    }
}
