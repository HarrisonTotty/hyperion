//! Poisson bins: memoryless events, generated bin by bin (plan 06, design note 15).
//!
//! Time is cut into bins `[kΔ, (k + 1)Δ)` with Δ a whole number of seconds, so bin membership is
//! exact integer arithmetic on [`UniverseTime`]. Bin k's own stream, `key.bin_stream(k)`, gives in
//! a fixed order:
//!
//! 1. the number of candidates, a Poisson draw of mean `bound × Δ`, where the bound is the
//!    [`RateModel`]'s upper bound of the rate over the bin;
//! 2. for each candidate j in turn, two words: its time in the bin, from the word's top 53 bits as
//!    a fraction of Δ in integer nanoseconds, and its thinning decision, accepted with probability
//!    `rate(t) ÷ bound` through an integer [`Threshold`].
//!
//! An accepted candidate is event j of bin k, and its marks come from its own stream,
//! `key.event_stream(k, j)`, so drawing marks never moves a time. A rejected candidate leaves a gap
//! in the numbers. The event word's 8-bit number caps a bin at 255 events; a kind chooses Δ so that
//! `bound × Δ` is at most 64, which puts an overflow below 10⁻⁶⁰, and a larger count is clamped
//! with a debug assertion. With Δ of at least 16 s the 40-bit bin number covers the source horizon.

use std::error::Error;
use std::fmt;

use super::{
    EventBin, EventId, EventSeries, TimeWindow, from_nanos, from_nanos_saturating, to_nanos,
};
use crate::rng::{Stream, Threshold};
use crate::time::UniverseTime;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// Nanoseconds in one second, for bin arithmetic in `u128`.
const NANOS_U128: u128 = 1_000_000_000;

/// Seconds in one day.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// A rate of events, in events per second: what a [`RateModel`] returns.
///
/// Rates of astrophysical events are quoted per day or per year, so the constructors name the unit
/// they take: [`per_day`](Self::per_day) (86,400 s) and [`per_julian_year`](Self::per_julian_year)
/// (31,557,600 s).
///
/// # Examples
///
/// ```
/// use hyperion_sim::events::EventsPerSecond;
///
/// let fu_orionis = EventsPerSecond::per_julian_year(1.0 / 5_000.0);
/// assert!((fu_orionis.value() - 6.34e-12).abs() < 1e-14);
/// assert!(EventsPerSecond::per_day(3.0) > fu_orionis);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct EventsPerSecond(f64);

impl EventsPerSecond {
    /// No events.
    pub const ZERO: Self = Self(0.0);

    /// `per_second` events per second.
    #[must_use]
    pub const fn new(per_second: f64) -> Self {
        Self(per_second)
    }

    /// `per_day` events per day of 86,400 s.
    #[must_use]
    pub fn per_day(per_day: f64) -> Self {
        Self(per_day / SECONDS_PER_DAY)
    }

    /// `per_year` events per Julian year of 31,557,600 s.
    #[must_use]
    pub fn per_julian_year(per_year: f64) -> Self {
        Self(per_year / SECONDS_PER_JULIAN_YEAR)
    }

    /// The rate in events per second.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// The rate of one kind of event of one subject, and a bound on it over each bin.
///
/// The bound is what the bin's candidates are drawn at, and the rate thins them, so a tight bound
/// wastes few draws. Both must be pure functions of their arguments and the model's own fields.
pub trait RateModel {
    /// An upper bound on [`rate`](Self::rate) over the bin `bin`: finite and not negative, with
    /// `bound × Δ` at most [`PoissonBins::MAX_MEAN_PER_BIN`].
    #[must_use]
    fn bound(&self, bin: TimeWindow) -> EventsPerSecond;

    /// The rate at `t`: not negative and at most the bound of the bin that holds `t`.
    #[must_use]
    fn rate(&self, t: UniverseTime) -> EventsPerSecond;
}

/// One event of a [`PoissonBins`] series: its ID, its time and the stream of its marks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BinEvent {
    id: EventId,
    time: UniverseTime,
    marks: Stream,
}

impl BinEvent {
    /// The event's ID: subject, tag, bin and number in the bin.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The instant of the event, its onset for an event with a duration.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// The event's own stream, at word 0, from which a kind draws the event's marks.
    #[must_use]
    pub const fn marks(&self) -> &Stream {
        &self.marks
    }

    /// The event's own stream, consuming the event.
    #[must_use]
    pub fn into_marks(self) -> Stream {
        self.marks
    }
}

/// A [`PoissonBins`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildPoissonBinsError {
    /// The bin is shorter than [`PoissonBins::MIN_BIN_SECONDS`].
    BinTooShort {
        /// The requested length, in seconds.
        seconds: i64,
    },
    /// The bin is longer than [`PoissonBins::MAX_BIN_SECONDS`].
    BinTooLong {
        /// The requested length, in seconds.
        seconds: i64,
    },
}

impl fmt::Display for BuildPoissonBinsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinTooShort { seconds } => write!(
                f,
                "a poisson bin of {seconds} s is shorter than the {} s minimum",
                PoissonBins::MIN_BIN_SECONDS
            ),
            Self::BinTooLong { seconds } => write!(
                f,
                "a poisson bin of {seconds} s is longer than the {} s maximum",
                PoissonBins::MAX_BIN_SECONDS
            ),
        }
    }
}

impl Error for BuildPoissonBinsError {}

/// The bins of one kind of memoryless event: their length Δ and how far back an event can still
/// be active.
///
/// # Examples
///
/// Flares at a constant 3 a day, in bins of a day, over one year:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{
///     EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow, tags,
/// };
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::time::UniverseTime;
///
/// struct ThreeADay;
/// impl RateModel for ThreeADay {
///     fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
///         EventsPerSecond::per_day(3.0)
///     }
///     fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
///         EventsPerSecond::per_day(3.0)
///     }
/// }
///
/// let star = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let flares = EventSeries::new(Seed::new(7), tags::STAR_FLARE, star.into());
/// let bins = PoissonBins::new(86_400, 1)?;
/// let year = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(1).ok_or("")?)?;
///
/// let mut events = Vec::new();
/// bins.events_in(&flares, year, &ThreeADay, &mut events);
/// assert!((950..1_250).contains(&events.len()));
/// // Any one of them can be regenerated from its ID alone.
/// assert_eq!(bins.event(&flares, events[7].id(), &ThreeADay).as_ref(), Some(&events[7]));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PoissonBins {
    bin_seconds: i64,
    look_back_bins: u32,
}

impl PoissonBins {
    /// The shortest bin, 16 s: the shortest for which the event word's 40-bit bin number covers the
    /// source horizon (plan 06, design note 15).
    pub const MIN_BIN_SECONDS: i64 = 16;

    /// The longest bin, 2⁴⁴ s (about 557,000 years), which keeps a candidate's time exact in
    /// 128-bit integer arithmetic.
    pub const MAX_BIN_SECONDS: i64 = 1 << 44;

    /// The largest mean count a bin may have, 64: a kind chooses Δ so that its bound × Δ stays at
    /// or under it, and then a count above 255 has a probability below 10⁻⁶⁰.
    pub const MAX_MEAN_PER_BIN: f64 = 64.0;

    /// The most events a bin holds, 255, which the event word's 8-bit number holds.
    pub const MAX_EVENTS_PER_BIN: u8 = 255;

    /// Bins of `bin_seconds` seconds; [`active_at`](Self::active_at) looks back `look_back_bins`
    /// whole bins before the one holding its instant.
    ///
    /// # Errors
    ///
    /// [`BuildPoissonBinsError::BinTooShort`] below [`MIN_BIN_SECONDS`](Self::MIN_BIN_SECONDS);
    /// [`BuildPoissonBinsError::BinTooLong`] above [`MAX_BIN_SECONDS`](Self::MAX_BIN_SECONDS).
    pub fn new(bin_seconds: i64, look_back_bins: u32) -> Result<Self, BuildPoissonBinsError> {
        if bin_seconds < Self::MIN_BIN_SECONDS {
            return Err(BuildPoissonBinsError::BinTooShort {
                seconds: bin_seconds,
            });
        }
        if bin_seconds > Self::MAX_BIN_SECONDS {
            return Err(BuildPoissonBinsError::BinTooLong {
                seconds: bin_seconds,
            });
        }
        Ok(Self {
            bin_seconds,
            look_back_bins,
        })
    }

    /// The bin length Δ, in seconds.
    #[must_use]
    pub const fn bin_seconds(&self) -> i64 {
        self.bin_seconds
    }

    /// How many bins before its own [`active_at`](Self::active_at) looks back.
    #[must_use]
    pub const fn look_back_bins(&self) -> u32 {
        self.look_back_bins
    }

    /// The bin holding `t`, `floor(t ÷ Δ)`, or `None` beyond the 40-bit bin numbers.
    #[must_use]
    pub fn bin_of(&self, t: UniverseTime) -> Option<EventBin> {
        EventBin::new(t.seconds().div_euclid(self.bin_seconds)).ok()
    }

    /// Bin `bin`'s span, `[kΔ, (k + 1)Δ)`, or `None` where the universe clock cannot hold it.
    #[must_use]
    pub fn bin_window(&self, bin: EventBin) -> Option<TimeWindow> {
        let start = i128::from(bin.get()) * i128::from(self.bin_seconds);
        let start = UniverseTime::new(i64::try_from(start).ok()?, 0).ok()?;
        let end = bin
            .get()
            .checked_add(1)?
            .checked_mul(self.bin_seconds)
            .and_then(|end| UniverseTime::new(end, 0).ok())?;
        TimeWindow::new(start, end).ok()
    }

    /// Appends the events of `series` in `window` to `out`, sorted by time and then by ID.
    ///
    /// Every bin the window touches is generated in full and its events outside the window are
    /// dropped, so the listing over a window is exactly the union of the listings over any
    /// partition of it. The cost is proportional to the number of bins the window touches. Bins
    /// beyond the 40-bit bin numbers hold no events.
    ///
    /// # Panics
    ///
    /// If the model's bound is negative, NaN or so large that the bin's mean exceeds 2³¹. In debug
    /// builds also if the mean exceeds [`MAX_MEAN_PER_BIN`](Self::MAX_MEAN_PER_BIN), a count
    /// exceeds [`MAX_EVENTS_PER_BIN`](Self::MAX_EVENTS_PER_BIN), or a rate is negative, NaN or
    /// above its bound.
    pub fn events_in(
        &self,
        series: &EventSeries,
        window: TimeWindow,
        rate: &impl RateModel,
        out: &mut Vec<BinEvent>,
    ) {
        if window.is_empty() {
            return;
        }
        let last_instant = from_nanos_saturating(to_nanos(window.end()) - 1);
        let first = window.start().seconds().div_euclid(self.bin_seconds);
        let last = last_instant.seconds().div_euclid(self.bin_seconds);
        let first = first.max(EventBin::MIN.get());
        let last = last.min(EventBin::MAX.get());
        let begin = out.len();
        for k in first..=last {
            let bin = EventBin::new(k).expect("clamped to the 40-bit bin numbers");
            self.visit_bin(series, bin, rate, |event| {
                if window.contains(event.time) {
                    out.push(event);
                }
            });
        }
        out[begin..].sort_by(|a, b| a.time.cmp(&b.time).then(a.id.cmp(&b.id)));
    }

    /// Appends to `out` the events of `series` that may be active at `t`: those whose onset lies
    /// between the start of the bin [`look_back_bins`](Self::look_back_bins) before `t`'s and `t`
    /// itself, inclusive, sorted by time. The kind decides from each event's marks whether it is
    /// still running.
    ///
    /// # Panics
    ///
    /// As [`events_in`](Self::events_in).
    ///
    /// # Examples
    ///
    /// Outbursts at one per 5,000 years that each last a drawn 20 to 200 years, in bins of 500
    /// years looking back one, so that no outburst outlives the look-back; the marks give each
    /// event's duration, and only the running ones are kept:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::events::{
    ///     EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow, tags,
    /// };
    /// use hyperion_sim::id::{BodyId, SystemId};
    /// use hyperion_sim::time::{Span, UniverseTime};
    ///
    /// struct Outbursts;
    /// impl RateModel for Outbursts {
    ///     fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
    ///         EventsPerSecond::per_julian_year(1.0 / 5_000.0)
    ///     }
    ///     fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
    ///         EventsPerSecond::per_julian_year(1.0 / 5_000.0)
    ///     }
    /// }
    ///
    /// let star = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
    /// let series = EventSeries::new(Seed::new(7), tags::STAR_FU_ORIONIS, star.into());
    /// let bins = PoissonBins::new(500 * 31_557_600, 1)?;
    /// let mut running = 0;
    /// for century in -500..500 {
    ///     let t = UniverseTime::from_julian_years(100 * century).ok_or("")?;
    ///     let mut candidates = Vec::new();
    ///     bins.active_at(&series, t, &Outbursts, &mut candidates);
    ///     running += candidates
    ///         .into_iter()
    ///         .filter(|e| {
    ///             let years = e.marks().clone().uniform_in(20.0, 200.0);
    ///             let years = i64::try_from(years.round() as i32).unwrap_or(200);
    ///             let end = Span::from_julian_years(years).and_then(|d| e.time().checked_add(d));
    ///             end.is_some_and(|end| t < end)
    ///         })
    ///         .count();
    /// }
    /// // A duty cycle of 110 ÷ 5,000: about 22 of the 1,000 instants.
    /// assert!((5..60).contains(&running));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn active_at(
        &self,
        series: &EventSeries,
        t: UniverseTime,
        rate: &impl RateModel,
        out: &mut Vec<BinEvent>,
    ) {
        let k = t.seconds().div_euclid(self.bin_seconds);
        let first = k
            .saturating_sub(i64::from(self.look_back_bins))
            .max(EventBin::MIN.get());
        let start = i128::from(first) * i128::from(self.bin_seconds);
        let start = from_nanos_saturating(start.saturating_mul(1_000_000_000));
        let end = from_nanos_saturating(to_nanos(t) + 1);
        // The window is reversed only when every bin up to `t` lies below the 40-bit bin numbers,
        // whose bins hold no events, so there is nothing to append.
        if let Ok(window) = TimeWindow::new(start, end) {
            self.events_in(series, window, rate, out);
        }
    }

    /// The event `id` names, regenerated on its own; `None` if `id` is not of `series`, or names a
    /// candidate that thinning rejected or a number beyond its bin's count.
    ///
    /// # Panics
    ///
    /// As [`events_in`](Self::events_in).
    #[must_use]
    pub fn event(
        &self,
        series: &EventSeries,
        id: EventId,
        rate: &impl RateModel,
    ) -> Option<BinEvent> {
        if !series.names(id) {
            return None;
        }
        let bin = id.word().bin();
        let j = id.word().number();
        let mut open = self.open_bin(series, bin, rate)?;
        if j >= open.count {
            return None;
        }
        let first_candidate = open.stream.position();
        open.stream.seek(first_candidate + 2 * u64::from(j));
        open.candidate(series, bin, j, rate)
    }

    /// Emits every accepted event of bin `bin`, in candidate order.
    fn visit_bin(
        &self,
        series: &EventSeries,
        bin: EventBin,
        rate: &impl RateModel,
        mut emit: impl FnMut(BinEvent),
    ) {
        let Some(mut open) = self.open_bin(series, bin, rate) else {
            return;
        };
        for j in 0..open.count {
            if let Some(event) = open.candidate(series, bin, j, rate) {
                emit(event);
            }
        }
    }

    /// Bin `bin`'s stream after its count has been drawn, or `None` if the clock cannot hold the
    /// bin.
    #[must_use]
    fn open_bin(
        &self,
        series: &EventSeries,
        bin: EventBin,
        rate: &impl RateModel,
    ) -> Option<OpenBin> {
        let window = self.bin_window(bin)?;
        let bound = rate.bound(window).value();
        debug_assert!(
            bound.is_finite() && bound >= 0.0,
            "a rate bound must be finite and not negative, got {bound}"
        );
        let mean = bound * self.bin_seconds_f64();
        debug_assert!(
            mean <= Self::MAX_MEAN_PER_BIN,
            "a bin's mean count is {mean}, above {}: choose a shorter bin",
            Self::MAX_MEAN_PER_BIN
        );
        let mut stream = series.key().bin_stream(bin);
        let count = stream.poisson(mean);
        debug_assert!(
            count <= u64::from(Self::MAX_EVENTS_PER_BIN),
            "a bin drew {count} events, above the event word's {}",
            Self::MAX_EVENTS_PER_BIN
        );
        let count =
            u8::try_from(count.min(u64::from(Self::MAX_EVENTS_PER_BIN))).expect("clamped to 255");
        Some(OpenBin {
            stream,
            count,
            start_nanos: to_nanos(window.start()),
            bin_nanos: u128::from(self.bin_seconds.unsigned_abs()) * NANOS_U128,
            bound,
        })
    }

    /// Δ in seconds as an `f64`, exact: Δ is at most 2⁴⁴.
    #[must_use]
    fn bin_seconds_f64(&self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "the bin length is at most 2^44 s, exact in f64"
        )]
        let seconds = self.bin_seconds as f64;
        seconds
    }
}

/// A bin whose count has been drawn, its stream at the first candidate's words.
struct OpenBin {
    stream: Stream,
    count: u8,
    start_nanos: i128,
    bin_nanos: u128,
    bound: f64,
}

impl OpenBin {
    /// Candidate `j`, from the stream's next two words: its time, then its thinning decision.
    fn candidate(
        &mut self,
        series: &EventSeries,
        bin: EventBin,
        j: u8,
        rate: &impl RateModel,
    ) -> Option<BinEvent> {
        // The word's top 53 bits as a fraction of the bin, `uniform()` in exact integer arithmetic:
        // 53 + 75 bits at most, so the product fits in 128.
        let fraction = u128::from(self.stream.next_u64() >> 11);
        let offset = i128::try_from((fraction * self.bin_nanos) >> 53)
            .expect("an offset below the bin's length fits in i128");
        let time = from_nanos(self.start_nanos + offset).expect("inside a bin the clock holds");
        let accepted = self
            .stream
            .decide(Threshold::from_ratio(rate.rate(time).value(), self.bound));
        accepted.then(|| BinEvent {
            id: series.event_id(bin, j),
            time,
            marks: series.key().event_stream(bin, j),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{
        ALPHA, assert_p_value, assert_poisson_count, chi_square_gof, ks_one_sample, poisson_pmf,
    };

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::events::tags;
    use crate::events::testing::{ConstantRate, assert_partition_independent, partition};
    use crate::id::{BodyId, EventSubject, SystemId, event_tags};
    use crate::math;
    use crate::rng::Seed;

    const SEED: Seed = Seed::new(0x0e7e_0006_0027_0002);
    const DAY: i64 = 86_400;

    fn star() -> EventSubject {
        BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), 0).into()
    }

    fn flares() -> EventSeries {
        EventSeries::new(SEED, tags::STAR_FLARE, star())
    }

    fn at(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    fn days(from: i64, to: i64) -> TimeWindow {
        TimeWindow::new(at(from * DAY, 0), at(to * DAY, 0)).unwrap()
    }

    fn daily() -> PoissonBins {
        PoissonBins::new(DAY, 2).unwrap()
    }

    fn per_day(n: f64) -> ConstantRate {
        ConstantRate::new(EventsPerSecond::per_day(n))
    }

    fn list(
        bins: PoissonBins,
        series: &EventSeries,
        w: TimeWindow,
        r: &impl RateModel,
    ) -> Vec<BinEvent> {
        let mut out = Vec::new();
        bins.events_in(series, w, r, &mut out);
        out
    }

    /// A mean rate modulated by `1 + depth × sin(2π t ÷ period)`, bounded by its peak.
    struct Sinusoid {
        mean: EventsPerSecond,
        depth: f64,
        period_seconds: i64,
    }

    impl Sinusoid {
        /// The phase of `t` in the modulation, in `[0, 1)`, exactly from integer nanoseconds.
        fn phase(&self, t: UniverseTime) -> f64 {
            let period = i128::from(self.period_seconds) * 1_000_000_000;
            let rem = to_nanos(t).rem_euclid(period);
            #[expect(
                clippy::cast_precision_loss,
                reason = "a test's ratio of two nanosecond counts"
            )]
            let phase = rem as f64 / period as f64;
            phase
        }
    }

    impl RateModel for Sinusoid {
        fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
            EventsPerSecond::new(self.mean.value() * (1.0 + self.depth))
        }

        fn rate(&self, t: UniverseTime) -> EventsPerSecond {
            let modulation = 1.0 + self.depth * math::sin(TAU * self.phase(t));
            EventsPerSecond::new(self.mean.value() * modulation)
        }
    }

    /// A model that breaks its own bound, for the debug assertion.
    struct Overshoot;

    impl RateModel for Overshoot {
        fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
            EventsPerSecond::per_day(1.0)
        }

        fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
            EventsPerSecond::per_day(2.0)
        }
    }

    #[test]
    fn bins_are_between_16_seconds_and_2_to_the_44() {
        assert_eq!(
            PoissonBins::new(15, 0),
            Err(BuildPoissonBinsError::BinTooShort { seconds: 15 })
        );
        assert_eq!(
            PoissonBins::new(-DAY, 0),
            Err(BuildPoissonBinsError::BinTooShort { seconds: -DAY })
        );
        assert_eq!(
            PoissonBins::new((1 << 44) + 1, 0),
            Err(BuildPoissonBinsError::BinTooLong {
                seconds: (1 << 44) + 1
            })
        );
        assert!(PoissonBins::new(16, 0).is_ok());
        assert!(PoissonBins::new(1 << 44, 3).is_ok());
        assert_eq!(
            BuildPoissonBinsError::BinTooShort { seconds: 15 }.to_string(),
            "a poisson bin of 15 s is shorter than the 16 s minimum"
        );
        let bins = PoissonBins::new(DAY, 3).unwrap();
        assert_eq!((bins.bin_seconds(), bins.look_back_bins()), (DAY, 3));
    }

    /// Membership is floor division on whole seconds, so the nanosecond before the epoch is in
    /// bin −1 and the epoch starts bin 0.
    #[test]
    fn a_bin_holds_exactly_its_half_open_day() {
        let bins = daily();
        let before = at(-1, 999_999_999);
        assert_eq!(bins.bin_of(before), EventBin::new(-1).ok());
        assert_eq!(bins.bin_of(UniverseTime::EPOCH), EventBin::new(0).ok());
        let minus_one = bins.bin_window(EventBin::new(-1).unwrap()).unwrap();
        assert_eq!(minus_one, days(-1, 0));
        assert!(minus_one.contains(before));
        assert!(!minus_one.contains(UniverseTime::EPOCH));
        // Day-long bins run out of 40-bit numbers before the clock runs out of seconds; the
        // longest bins run out of seconds first.
        assert!(bins.bin_window(EventBin::MAX).is_some());
        assert_eq!(bins.bin_of(at(i64::MAX, 0)), None);
        let long = PoissonBins::new(1 << 44, 0).unwrap();
        assert_eq!(long.bin_window(EventBin::MAX), None);
        assert!(long.bin_of(at(i64::MAX, 0)).is_some());
    }

    #[test]
    fn a_listing_is_sorted_and_lies_in_its_window() {
        let w = TimeWindow::new(at(-3 * DAY + 5_000, 7), at(4 * DAY - 1, 0)).unwrap();
        let events = list(daily(), &flares(), w, &per_day(20.0));
        assert!(events.len() > 100, "{} events", events.len());
        assert!(events.iter().all(|e| w.contains(e.time())));
        assert!(events.windows(2).all(|p| p[0].time() <= p[1].time()));
        assert!(list(daily(), &flares(), days(0, 0), &per_day(20.0)).is_empty());
    }

    /// The window, rate and cut sets of the partition tests.
    fn partition_case() -> (Sinusoid, TimeWindow) {
        let rate = Sinusoid {
            mean: EventsPerSecond::per_day(12.0),
            depth: 0.7,
            period_seconds: 3 * DAY + 17,
        };
        let whole = TimeWindow::new(at(-9 * DAY - 123, 456), at(11 * DAY + 5, 0)).unwrap();
        (rate, whole)
    }

    /// Cuts on bin edges, inside bins, on an event's instant and a nanosecond after another.
    fn cut_sets(events: &[BinEvent]) -> Vec<Vec<UniverseTime>> {
        vec![
            vec![at(0, 0)],
            vec![at(-DAY, 0), at(-DAY, 0), at(DAY, 1), at(7 * DAY + 999, 3)],
            vec![
                events[40].time(),
                from_nanos(to_nanos(events[41].time()) + 1).unwrap(),
            ],
            (-8..11).map(|d| at(d * DAY + d * 1_000, 0)).collect(),
        ]
    }

    /// The brainstorm's time test for bins: the union over any partition is the whole, in any
    /// order of asking.
    #[test]
    fn the_listing_over_any_partition_is_the_listing_over_the_whole() {
        let series = flares();
        let (rate, whole) = partition_case();
        let listing = |w: TimeWindow| list(daily(), &series, w, &rate);
        let events = listing(whole);
        for cuts in &cut_sets(&events) {
            assert_partition_independent(whole, cuts, listing);
            assert_order_independent(&partition(whole, cuts), |w| listing(*w));
        }
    }

    /// The pieces listed on threads of their own give the same events. Not on wasm32-wasip1,
    /// which has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn the_listing_is_the_same_on_other_threads() {
        let series = flares();
        let (rate, whole) = partition_case();
        let listing = |w: TimeWindow| list(daily(), &series, w, &rate);
        let events = listing(whole);
        for cuts in &cut_sets(&events) {
            let pieces = partition(whole, cuts);
            let threaded: Vec<Vec<BinEvent>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&w| scope.spawn(move || listing(w)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), events);
        }
    }

    /// Every listed event comes back from its ID alone; every other number of its bins gives
    /// `None`, whether thinned away or beyond the count.
    #[test]
    fn an_id_regenerates_its_event_and_names_nothing_else() {
        let series = flares();
        let rate = Sinusoid {
            mean: EventsPerSecond::per_day(30.0),
            depth: 0.9,
            period_seconds: DAY / 3,
        };
        let bins = daily();
        let w = days(-4, 4);
        let events = list(bins, &series, w, &rate);
        let mut rejected = 0;
        for k in -4..4 {
            let bin = EventBin::new(k).unwrap();
            for j in 0..=u8::MAX {
                let id = series.event_id(bin, j);
                let listed = events.iter().find(|e| e.id() == id);
                let regenerated = bins.event(&series, id, &rate);
                assert_eq!(regenerated.as_ref(), listed, "bin {k}, number {j}");
                if listed.is_none() && j < 40 {
                    rejected += 1;
                }
            }
        }
        assert!(
            rejected > 0,
            "thinning should have left gaps in the numbers"
        );
        let other = EventSeries::new(SEED, event_tags::SELF_TEST, star());
        assert_eq!(bins.event(&other, events[0].id(), &rate), None);
    }

    #[test]
    fn marks_come_from_the_events_own_stream_and_the_same_seed_gives_the_same_events() {
        let series = flares();
        let events = list(daily(), &series, days(0, 3), &per_day(10.0));
        for e in &events {
            let word = e.id().word();
            assert_eq!(
                *e.marks(),
                series.key().event_stream(word.bin(), word.number())
            );
            assert_eq!(e.clone().into_marks(), *e.marks());
        }
        assert_eq!(list(daily(), &flares(), days(0, 3), &per_day(10.0)), events);
        let other = EventSeries::new(Seed::new(1), tags::STAR_FLARE, star());
        assert_ne!(list(daily(), &other, days(0, 3), &per_day(10.0)), events);
    }

    /// `active_at` returns the onsets from the start of the bin two before `t`'s up to `t`
    /// inclusive.
    #[test]
    fn active_at_looks_back_whole_bins() {
        let series = flares();
        let rate = per_day(8.0);
        let t = at(5 * DAY + 40_000, 5);
        let mut active = Vec::new();
        daily().active_at(&series, t, &rate, &mut active);
        let expected = list(
            daily(),
            &series,
            TimeWindow::new(at(3 * DAY, 0), from_nanos(to_nanos(t) + 1).unwrap()).unwrap(),
            &rate,
        );
        assert_eq!(active, expected);
        assert!(!active.is_empty());
        // An event exactly at t is active at t.
        let onset = active.last().unwrap().time();
        let mut at_onset = Vec::new();
        daily().active_at(&series, onset, &rate, &mut at_onset);
        assert_eq!(at_onset.last().map(BinEvent::time), Some(onset));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "exceeds its thinning bound")]
    fn a_rate_above_its_bound_fails_a_debug_assertion() {
        let _ = list(daily(), &flares(), days(0, 30), &Overshoot);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "choose a shorter bin")]
    fn a_mean_above_64_per_bin_fails_a_debug_assertion() {
        let _ = list(daily(), &flares(), days(0, 1), &per_day(65.0));
    }

    /// At a constant rate the counts per bin are Poisson: the total lies in its interval and the
    /// distribution of counts fits the Poisson law (chi-square).
    #[test]
    fn bin_counts_follow_the_poisson_law() {
        let mean = 7.5;
        let bins = daily();
        let series = flares();
        let n_bins = 6_000_i64;
        let events = list(bins, &series, days(-n_bins / 2, n_bins / 2), &per_day(mean));
        #[expect(clippy::cast_precision_loss, reason = "a small count")]
        let expected_total = mean * n_bins as f64;
        assert_poisson_count(
            "events in 6,000 bins",
            u64::try_from(events.len()).unwrap(),
            expected_total,
            ALPHA,
        );
        let mut per_bin = vec![0_u64; usize::try_from(n_bins).unwrap()];
        for e in &events {
            let k = e.id().word().bin().get() + n_bins / 2;
            per_bin[usize::try_from(k).unwrap()] += 1;
        }
        let top = 25_u64;
        let mut observed = vec![0_u64; usize::try_from(top).unwrap() + 1];
        for &c in &per_bin {
            observed[usize::try_from(c.min(top)).unwrap()] += 1;
        }
        #[expect(clippy::cast_precision_loss, reason = "a small count")]
        let n = n_bins as f64;
        let mut expected: Vec<f64> = (0..top).map(|k| n * poisson_pmf(k, mean)).collect();
        let below: f64 = expected.iter().sum();
        expected.push(n - below);
        let fit = chi_square_gof(&observed, &expected);
        assert_p_value("counts per bin against Poisson(7.5)", fit.p_value, ALPHA);
    }

    /// At a constant rate the waits between events, across bin edges, are exponential (K–S).
    #[test]
    fn waiting_times_are_exponential_at_a_constant_rate() {
        let per_second = EventsPerSecond::per_day(5.0).value();
        let events = list(daily(), &flares(), days(-2_000, 2_000), &per_day(5.0));
        let mut waits: Vec<f64> = events
            .windows(2)
            .map(|p| {
                p[1].time()
                    .checked_since(p[0].time())
                    .unwrap()
                    .as_seconds_f64()
            })
            .collect();
        let ks = ks_one_sample(&mut waits, |x| 1.0 - math::exp(-per_second * x.max(0.0)));
        assert_p_value("waiting times against the exponential", ks.p_value, ALPHA);
    }

    /// A thinned sinusoidal rate reproduces its profile: the events' phases in twenty bins against
    /// the integral of `1 + 0.8 sin 2πφ` over each (chi-square).
    #[test]
    fn a_thinned_sinusoidal_rate_reproduces_its_profile() {
        let rate = Sinusoid {
            mean: EventsPerSecond::per_day(6.0),
            depth: 0.8,
            period_seconds: 10 * DAY + 3_600,
        };
        let events = list(daily(), &flares(), days(-1_500, 1_500), &rate);
        let slots = 20_usize;
        let mut observed = vec![0_u64; slots];
        for e in &events {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "a phase in [0, 1) times twenty, floored"
            )]
            let slot = ((rate.phase(e.time()) * slots as f64) as usize).min(slots - 1);
            observed[slot] += 1;
        }
        #[expect(clippy::cast_precision_loss, reason = "small counts")]
        let (total, width) = (events.len() as f64, 1.0 / slots as f64);
        let expected: Vec<f64> = (0..slots)
            .map(|i| {
                #[expect(clippy::cast_precision_loss, reason = "a small index")]
                let (a, b) = (i as f64 * width, (i + 1) as f64 * width);
                let integral = width + 0.8 / TAU * (math::cos(TAU * a) - math::cos(TAU * b));
                total * integral
            })
            .collect();
        let fit = chi_square_gof(&observed, &expected);
        assert_p_value("thinned sinusoid's phase profile", fit.p_value, ALPHA);
    }

    /// Pinned keys: a star's flares and a system's self-test events, with each event's time and
    /// the first word of its marks, and a thinned rate whose gaps in the numbers pin each
    /// thinning decision.
    #[test]
    fn bin_events_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let system = SystemId::from_raw(0xF000_0007_0000_0000).unwrap();
        let cases: [(&str, EventSeries, PoissonBins, TimeWindow, f64); 3] = [
            ("star flares, 6 a day", flares(), daily(), days(-2, 2), 6.0),
            (
                "system self-test, 60 a day in hour bins",
                EventSeries::new(SEED, event_tags::SELF_TEST, system.into()),
                PoissonBins::new(3_600, 0).unwrap(),
                TimeWindow::new(at(-7_200, 0), at(7_200, 0)).unwrap(),
                60.0,
            ),
            (
                "body 3 flares, 40 a day",
                EventSeries::new(SEED, tags::STAR_FLARE, BodyId::new(system, 3).into()),
                daily(),
                days(100, 101),
                40.0,
            ),
        ];
        let write = |w: &mut GoldenWriter, events: Vec<BinEvent>| {
            for e in events {
                w.u64_hex(
                    &format!("{} at {}", e.id(), e.time()),
                    e.into_marks().next_u64(),
                );
            }
        };
        for (name, series, bins, window, per_day_count) in cases {
            w.line("");
            w.line(&format!("# {name}"));
            write(&mut w, list(bins, &series, window, &per_day(per_day_count)));
        }
        w.line("");
        w.line("# star flares thinned from 20 a day by 1 + 0.9 sin(2π t ÷ 1 d)");
        let thinned = Sinusoid {
            mean: EventsPerSecond::per_day(20.0),
            depth: 0.9,
            period_seconds: DAY,
        };
        write(&mut w, list(daily(), &flares(), days(-1, 1), &thinned));
        golden!("events/bins", w.as_str());
    }
}
