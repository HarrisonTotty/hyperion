//! The monotone phase: cycles of accumulation and release (plan 06, design note 16).
//!
//! An event falls where Φ(t) = φ(t) + N(φ(t)) crosses an integer n (brainstorm, "Events in time").
//! φ is the clock's base phase in cycles, t ÷ P for a fixed period ([`LinearClock`]) or any
//! strictly increasing closed form for a drifting one ([`PhaseClock`]). N is a sum of J octaves of
//! value noise under the subject's event key:
//!
//! ```text
//! N(x) = Σⱼ a × 2^(j/2) × vⱼ(x ÷ (ℓ × 2ʲ)),   j = 0 … J − 1
//! ```
//!
//! where each vⱼ interpolates hashed lattice values in [−1, 1] with the cubic 3s² − 2s³. The
//! amplitudes grow as a random walk's would, so the phase diffuses up to the largest octave, which
//! a kind sizes to span the source horizon ([`octaves_to_span`]); a plain jittered lattice is wrong
//! because its phase never diffuses. The slopes fall as 2^(−j/2), so |N′| is at most
//! 3 × a ÷ ℓ × Σⱼ 2^(−j/2) < 10.3 × a ÷ ℓ, and [`MonotonePhase::new`] rejects parameters for which
//! that exceeds 0.9. Φ is then strictly increasing, with dΦ/dφ between 0.1 and 1.9, so events cannot
//! change order. Event n is the root of Φ(t) = n, bracketed by the noise's amplitude bound and
//! found by bisection on whole nanoseconds, which is deterministic. It is the midpoint-displacement
//! idea of the research notes in a form that needs no recursion.
//!
//! Streams under the subject's event key: cycle n's own stream (slot 0) holds its [`SkipMark`]
//! decision, its event stream 0 (slot 1) the event's marks, and octave j's lattice value at index
//! i is the first word of event stream j + 1 of "bin" i (slot j + 2), with i reduced to the 40-bit
//! bin numbers. The slots never meet, so the lattice and the events draw from different streams.
//!
//! Phases are split into whole cycles and a fraction ([`Phase`]): at a period of 16 s the source
//! horizon holds 5 × 10¹¹ cycles, where an `f64` of cycles resolves only 6 × 10⁻⁵.

use std::error::Error;
use std::fmt;

use super::{EventBin, EventId, EventSeries, TimeWindow, from_nanos_saturating, to_nanos};
use crate::math;
use crate::rng::{EventKey, Stream, Threshold};
use crate::time::{Span, UniverseTime};

/// A phase in cycles, split into whole cycles and a fraction in `[0, 1)`.
///
/// The split keeps a phase exact to the fraction's precision however many cycles have passed.
/// The derived ordering is by value, since the fraction is normalised.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Phase {
    cycles: i64,
    fraction: f64,
}

impl Phase {
    /// The phase `cycles + fraction`, normalised so that the fraction lies in `[0, 1)`.
    ///
    /// Whole cycles saturate at the ends of `i64`.
    ///
    /// # Panics
    ///
    /// If `fraction` is not finite: a clock that computes a NaN or an infinite phase is broken, and
    /// continuing would give a NaN whose bits differ between architectures.
    #[must_use]
    pub fn new(cycles: i64, fraction: f64) -> Self {
        assert!(fraction.is_finite(), "a phase's fraction is {fraction}");
        let whole = fraction.floor();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "whole is an integer; the cast saturates, as the whole cycles do"
        )]
        let carried = whole as i64;
        // Exact: `fraction − floor(fraction)` needs no more bits than `fraction`, except for a
        // tiny negative fraction, where it rounds up to 1 and is carried below.
        let rest = fraction - whole;
        let cycles = cycles.saturating_add(carried);
        if rest >= 1.0 {
            Self {
                cycles: cycles.saturating_add(1),
                fraction: 0.0,
            }
        } else {
            Self {
                cycles,
                fraction: rest,
            }
        }
    }

    /// The phase of exactly `n` whole cycles.
    #[must_use]
    pub const fn whole(n: i64) -> Self {
        Self {
            cycles: n,
            fraction: 0.0,
        }
    }

    /// The whole cycles, floored.
    #[must_use]
    pub const fn cycles(self) -> i64 {
        self.cycles
    }

    /// The fraction of the current cycle, in `[0, 1)`.
    #[must_use]
    pub const fn fraction(self) -> f64 {
        self.fraction
    }

    /// This phase moved by `delta` cycles.
    #[must_use]
    pub fn offset(self, delta: f64) -> Self {
        Self::new(self.cycles, self.fraction + delta)
    }

    /// The phase as one `f64` of cycles, which loses the fraction's precision beyond 2⁵³ cycles and
    /// its resolution well before.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "documented as lossy; the split form is the exact one"
        )]
        let whole = self.cycles as f64;
        whole + self.fraction
    }
}

/// A clock whose phase advances through cycles: the base phase φ(t) of a [`MonotonePhase`].
///
/// This is the trait for clocks whose period drifts: an implementation gives φ in closed form,
/// continuous and strictly increasing in t, and its inverse. [`LinearClock`] is the fixed period.
/// A monotone phase needs the inverse only to bracket each crossing, one cycle wider than the
/// noise can reach, so the inverse may be off by a small fraction of a cycle.
///
/// # Examples
///
/// A clock whose frequency grows linearly from the epoch, f(t) = f₀ (1 + t ÷ τ), so that φ(t) =
/// f₀ (t + t² ÷ 2τ), with its inverse in closed form; a nova's recurrence speeding up as the
/// accretion rate grows could run on such a clock.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventSeries, MonotonePhase, Phase, PhaseClock, TimeWindow, tags};
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::time::{Span, UniverseTime};
///
/// struct Speeding {
///     cycles_per_second: f64,
///     tau_seconds: f64,
/// }
///
/// impl PhaseClock for Speeding {
///     fn base_phase(&self, t: UniverseTime) -> Phase {
///         let x = t.since_epoch().as_seconds_f64();
///         Phase::new(0, self.cycles_per_second * (x + x * x / (2.0 * self.tau_seconds)))
///     }
///
///     fn time_at(&self, phase: Phase) -> UniverseTime {
///         // The positive root of x² ÷ 2τ + x − φ ÷ f₀ = 0.
///         let tau = self.tau_seconds;
///         let c = phase.to_f64() / self.cycles_per_second;
///         let x = tau * ((1.0 + 2.0 * c / tau).max(0.0).sqrt() - 1.0);
///         let span = Span::from_seconds_f64(x).unwrap_or(Span::ZERO);
///         UniverseTime::EPOCH.checked_add(span).unwrap_or(UniverseTime::EPOCH)
///     }
/// }
///
/// let year = 31_557_600.0;
/// let clock = Speeding { cycles_per_second: 1.0 / (40.0 * year), tau_seconds: 1_000.0 * year };
/// let star = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let novae = EventSeries::new(Seed::new(7), tags::STAR_THERMAL_PULSE, star.into());
/// let phase = MonotonePhase::new(0.2, 4, 3)?;
/// let window = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(1_000).ok_or("")?)?;
/// let mut events = Vec::new();
/// phase.events_in(&novae, &clock, window, &mut events);
/// // 37.5 cycles in the first thousand years instead of 25 at the starting rate.
/// assert!((33..=42).contains(&events.len()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait PhaseClock {
    /// The base phase at `t`, in cycles.
    #[must_use]
    fn base_phase(&self, t: UniverseTime) -> Phase;

    /// The instant at which the base phase reaches `phase`, clamped to the universe clock's range.
    #[must_use]
    fn time_at(&self, phase: Phase) -> UniverseTime;
}

/// A [`LinearClock`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildLinearClockError {
    /// The period is shorter than [`LinearClock::MIN_PERIOD`].
    PeriodTooShort {
        /// The requested period.
        period: Span,
    },
}

impl fmt::Display for BuildLinearClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodTooShort { period } => write!(
                f,
                "a clock period of {period} is shorter than the {} minimum",
                LinearClock::MIN_PERIOD
            ),
        }
    }
}

impl Error for BuildLinearClockError {}

/// A fixed period: φ(t) = (t − origin) ÷ P.
///
/// The phase is computed in integer nanoseconds and split, whole cycles by Euclidean division and
/// the fraction as the remainder over the period in `f64`, so it is good to a few units in the last
/// place of the fraction, about 10⁻¹⁶ cycles, anywhere on the universe clock: well inside the
/// 10⁻⁶ cycles the plan asks for over the source horizon. The inverse is exact integer arithmetic
/// on the fraction's top 53 bits, so it returns an instant within max(1 ns, 2⁻⁵² P) of the one
/// whose phase that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinearClock {
    period: Span,
    origin: UniverseTime,
}

impl LinearClock {
    /// The shortest period, 16 s: the shortest for which the event word's 40-bit cycle number
    /// covers the source horizon (plan 06, design note 15).
    pub const MIN_PERIOD: Span = Span::from_seconds(16);

    /// A clock of period `period` whose phase is 0 at `origin`.
    ///
    /// # Errors
    ///
    /// [`BuildLinearClockError::PeriodTooShort`] below [`MIN_PERIOD`](Self::MIN_PERIOD).
    pub fn new(period: Span, origin: UniverseTime) -> Result<Self, BuildLinearClockError> {
        if period < Self::MIN_PERIOD {
            return Err(BuildLinearClockError::PeriodTooShort { period });
        }
        Ok(Self { period, origin })
    }

    /// The period.
    #[must_use]
    pub const fn period(&self) -> Span {
        self.period
    }

    /// The instant of phase 0.
    #[must_use]
    pub const fn origin(&self) -> UniverseTime {
        self.origin
    }

    /// How many cycles `span` holds, for sizing a [`MonotonePhase`]'s octaves.
    #[must_use]
    pub fn cycles_in(&self, span: Span) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a ratio of two nanosecond counts, for sizing"
        )]
        let ratio = span_nanos(span) as f64 / span_nanos(self.period) as f64;
        ratio
    }
}

impl PhaseClock for LinearClock {
    fn base_phase(&self, t: UniverseTime) -> Phase {
        let period = span_nanos(self.period);
        let since = to_nanos(t) - to_nanos(self.origin);
        // One 128-bit division, the remainder from the quotient: the root finder calls this on
        // every step.
        let whole = since.div_euclid(period);
        let cycles = i64::try_from(whole)
            .expect("with a period of at least 16 s the cycles of any two instants fit in i64");
        #[expect(
            clippy::cast_precision_loss,
            reason = "a ratio of two nanosecond counts, each rounded to 53 bits: the fraction is \
                      good to a few units in its last place, which is all an `f64` fraction holds"
        )]
        let fraction = (since - whole * period) as f64 / period as f64;
        Phase::new(cycles, fraction)
    }

    fn time_at(&self, phase: Phase) -> UniverseTime {
        let period = span_nanos(self.period);
        // The fraction's top 53 bits as an integer m, exact: scaling by 2⁵³ is exact, and the floor
        // of a value below 2⁵³ fits a u64. The part of the period is then m × P ÷ 2⁵³ in integers.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the floor of a fraction in [0, 1) times 2^53 is an integer in [0, 2^53)"
        )]
        let top_bits = (phase.fraction() * TWO_POW_53).floor() as u64;
        let part = i128::from(top_bits).saturating_mul(period) >> 53;
        let whole = i128::from(phase.cycles()).saturating_mul(period);
        from_nanos_saturating(
            to_nanos(self.origin)
                .saturating_add(whole)
                .saturating_add(part),
        )
    }
}

/// 2⁵³, exactly.
const TWO_POW_53: f64 = 9_007_199_254_740_992.0;

/// `span` in nanoseconds.
#[must_use]
fn span_nanos(span: Span) -> i128 {
    i128::from(span.seconds()) * 1_000_000_000 + i128::from(span.subsec_nanos())
}

/// The probability that a cycle passes without an event, for strongly irregular sources.
///
/// Decided by an integer [`Threshold`] against the first word of the cycle's own stream, so a
/// cycle's fate is a pure function of seed, subject, tag and cycle number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkipMark(Threshold);

impl SkipMark {
    /// Skips each cycle with probability `p`.
    ///
    /// # Panics
    ///
    /// In debug builds, unless `0 ≤ p ≤ 1`, as [`Threshold::from_probability`].
    #[must_use]
    pub fn from_probability(p: f64) -> Self {
        Self(Threshold::from_probability(p))
    }

    /// The threshold a cycle's mark must lie below for the cycle to be skipped.
    #[must_use]
    pub const fn threshold(self) -> Threshold {
        self.0
    }

    /// Whether cycle `cycle` of `key` is skipped.
    #[must_use]
    fn skips(self, key: &EventKey, cycle: EventBin) -> bool {
        key.bin_stream(cycle).decide(self.0)
    }
}

/// A [`MonotonePhase`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildMonotonePhaseError {
    /// The amplitude is negative or not finite.
    InvalidAmplitude {
        /// The requested amplitude, in cycles.
        amplitude: f64,
    },
    /// The lattice spacing is zero.
    ZeroLattice,
    /// There are no octaves, or more than [`MonotonePhase::MAX_OCTAVES`].
    OctavesOutOfRange {
        /// The requested number of octaves.
        octaves: u32,
    },
    /// The largest octave's lattice spacing, ℓ × 2^(J − 1), exceeds 2⁶² cycles.
    LatticeTooLong {
        /// The requested lattice spacing, in cycles.
        lattice_cycles: u32,
        /// The requested number of octaves.
        octaves: u32,
    },
    /// The noise's slope bound, 10.3 × a ÷ ℓ, exceeds [`MonotonePhase::MAX_SLOPE`], so events
    /// could change order.
    SlopeTooSteep {
        /// The slope bound.
        slope: f64,
    },
}

impl fmt::Display for BuildMonotonePhaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAmplitude { amplitude } => write!(
                f,
                "a phase noise amplitude must be finite and not negative, got {amplitude}"
            ),
            Self::ZeroLattice => f.write_str("a phase noise lattice needs at least one cycle"),
            Self::OctavesOutOfRange { octaves } => write!(
                f,
                "a phase noise needs 1 to {} octaves, got {octaves}",
                MonotonePhase::MAX_OCTAVES
            ),
            Self::LatticeTooLong {
                lattice_cycles,
                octaves,
            } => write!(
                f,
                "a lattice of {lattice_cycles} cycles over {octaves} octaves exceeds 2^62 cycles"
            ),
            Self::SlopeTooSteep { slope } => write!(
                f,
                "a phase noise slope bound of {slope} exceeds {}, so events could change order",
                MonotonePhase::MAX_SLOPE
            ),
        }
    }
}

impl Error for BuildMonotonePhaseError {}

/// One event of a [`MonotonePhase`] series: the crossing of whole cycle `cycle`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CycleEvent {
    id: EventId,
    cycle: i64,
    time: UniverseTime,
    marks: Stream,
}

impl CycleEvent {
    /// The event's ID: subject, tag, the cycle as its bin, and number 0.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The cycle whose start the event is.
    #[must_use]
    pub const fn cycle(&self) -> i64 {
        self.cycle
    }

    /// The instant the phase reaches the cycle: the first nanosecond of the bisection's bracket at
    /// which it has.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// The event's own stream, `event_stream(cycle, 0)` at word 0, from which a kind draws all of
    /// the event's marks. A phase kind opens no other event stream of the cycle: streams 1 and up
    /// hold the noise's lattice values.
    #[must_use]
    pub const fn marks(&self) -> &Stream {
        &self.marks
    }

    /// The event's own stream, consuming the event; as [`marks`](Self::marks).
    #[must_use]
    pub fn into_marks(self) -> Stream {
        self.marks
    }
}

/// The noise of a monotone phase: amplitude a, lattice spacing ℓ, J octaves and an optional skip.
///
/// # Examples
///
/// Thermal pulses every 10⁴ years, whose phase diffuses across the source horizon:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{
///     EventSeries, LinearClock, MonotonePhase, TimeWindow, octaves_to_span, tags,
/// };
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::time::{SourceHorizon, Span, UniverseTime};
///
/// let star = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let pulses = EventSeries::new(Seed::new(7), tags::STAR_THERMAL_PULSE, star.into());
/// let clock = LinearClock::new(Span::from_julian_years(10_000).ok_or("")?, UniverseTime::EPOCH)?;
/// let horizon = SourceHorizon::END.checked_since(SourceHorizon::START).ok_or("")?;
/// let phase = MonotonePhase::new(0.3, 4, octaves_to_span(4, clock.cycles_in(horizon)))?;
/// assert_eq!(phase.octaves(), 4);
///
/// let from = UniverseTime::from_julian_years(-200_000).ok_or("")?;
/// let window = TimeWindow::new(from, UniverseTime::EPOCH)?;
/// let mut events = Vec::new();
/// phase.events_in(&pulses, &clock, window, &mut events);
/// // Twenty periods, give or take twice the noise's bound of 2.2 cycles.
/// assert!((15..=25).contains(&events.len()));
/// assert!(events.windows(2).all(|p| p[1].cycle() == p[0].cycle() + 1));
/// // The cycle a star is in, and how far through it, at any instant.
/// let (cycle, _through) = phase.cycle_at(&pulses, &clock, events[3].time());
/// assert_eq!(cycle, events[3].cycle());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonotonePhase {
    amplitude: f64,
    lattice_cycles: u32,
    octaves: u32,
    skip: Option<SkipMark>,
}

impl MonotonePhase {
    /// The most octaves a noise may have, 48: the lattice values of octave j use event stream
    /// j + 1, and 48 octaves of a one-cycle lattice span 1.4 × 10¹⁴ cycles.
    pub const MAX_OCTAVES: u32 = 48;

    /// The factor of the noise's slope bound: |N′| < 10.3 × a ÷ ℓ (plan 06, design note 16; the
    /// exact sum is 3 ÷ (1 − 2^(−1/2)) = 10.24).
    pub const SLOPE_FACTOR: f64 = 10.3;

    /// The steepest slope bound allowed, 0.9, which keeps dΦ/dφ at 0.1 or more.
    pub const MAX_SLOPE: f64 = 0.9;

    /// The largest lattice spacing of the largest octave, 2⁶² cycles.
    const MAX_SPAN: i128 = 1 << 62;

    /// A noise of amplitude `amplitude` cycles at the smallest octave, lattice spacing
    /// `lattice_cycles` whole cycles at the smallest octave, and `octaves` octaves, with no skip.
    ///
    /// # Errors
    ///
    /// [`BuildMonotonePhaseError`] for a negative or non-finite amplitude, a zero lattice, a number
    /// of octaves outside 1 to [`MAX_OCTAVES`](Self::MAX_OCTAVES), a largest lattice spacing above
    /// 2⁶² cycles, or a slope bound above [`MAX_SLOPE`](Self::MAX_SLOPE).
    pub fn new(
        amplitude: f64,
        lattice_cycles: u32,
        octaves: u32,
    ) -> Result<Self, BuildMonotonePhaseError> {
        if !(amplitude.is_finite() && amplitude >= 0.0) {
            return Err(BuildMonotonePhaseError::InvalidAmplitude { amplitude });
        }
        if lattice_cycles == 0 {
            return Err(BuildMonotonePhaseError::ZeroLattice);
        }
        if octaves == 0 || octaves > Self::MAX_OCTAVES {
            return Err(BuildMonotonePhaseError::OctavesOutOfRange { octaves });
        }
        if i128::from(lattice_cycles) << (octaves - 1) > Self::MAX_SPAN {
            return Err(BuildMonotonePhaseError::LatticeTooLong {
                lattice_cycles,
                octaves,
            });
        }
        let slope = Self::SLOPE_FACTOR * amplitude / f64::from(lattice_cycles);
        if slope > Self::MAX_SLOPE {
            return Err(BuildMonotonePhaseError::SlopeTooSteep { slope });
        }
        Ok(Self {
            amplitude,
            lattice_cycles,
            octaves,
            skip: None,
        })
    }

    /// This noise, with each cycle's event skipped at `skip`'s probability.
    #[must_use]
    pub const fn with_skip(self, skip: SkipMark) -> Self {
        Self {
            skip: Some(skip),
            ..self
        }
    }

    /// The amplitude a of the smallest octave, in cycles.
    #[must_use]
    pub const fn amplitude_cycles(&self) -> f64 {
        self.amplitude
    }

    /// The lattice spacing of the smallest octave, in whole cycles.
    #[must_use]
    pub const fn lattice_cycles(&self) -> u32 {
        self.lattice_cycles
    }

    /// The number of octaves.
    #[must_use]
    pub const fn octaves(&self) -> u32 {
        self.octaves
    }

    /// The skip mark, if cycles can pass without an event.
    #[must_use]
    pub const fn skip(&self) -> Option<SkipMark> {
        self.skip
    }

    /// The largest the noise can be, Σⱼ a × 2^(j/2), in cycles: how far Φ can stray from φ.
    #[must_use]
    pub fn noise_bound_cycles(&self) -> f64 {
        (0..self.octaves)
            .map(|j| self.octave_amplitude(j))
            .fold(0.0, |sum, a| sum + a)
    }

    /// The phase Φ(t) = φ(t) + N(φ(t)) of `series` on `clock` at `t`.
    #[must_use]
    pub fn phase_at(
        &self,
        series: &EventSeries,
        clock: &impl PhaseClock,
        t: UniverseTime,
    ) -> Phase {
        self.phase_with(
            series.key(),
            clock,
            t,
            &mut Lattice::new(self, *series.key()),
        )
    }

    /// The cycle `series` is in at `t` and how far through it, in `[0, 1)`: Φ(t) split, for a
    /// continuous phase such as a light curve's.
    #[must_use]
    pub fn cycle_at(
        &self,
        series: &EventSeries,
        clock: &impl PhaseClock,
        t: UniverseTime,
    ) -> (i64, f64) {
        let phase = self.phase_at(series, clock, t);
        (phase.cycles(), phase.fraction())
    }

    /// Appends the events of `series` in `window` to `out`, in time order, which is cycle order.
    ///
    /// Each event's time is a function of its cycle alone, and membership is decided on that
    /// time, so the listing over a window is exactly the union of the listings over any partition
    /// of it. Cycles beyond the 40-bit cycle numbers have no event.
    ///
    /// # Panics
    ///
    /// In debug builds, if the events come out of order, which only a clock whose phase is not
    /// strictly increasing can cause.
    pub fn events_in(
        &self,
        series: &EventSeries,
        clock: &impl PhaseClock,
        window: TimeWindow,
        out: &mut Vec<CycleEvent>,
    ) {
        if window.is_empty() {
            return;
        }
        // One cycle of margin on each side: the crossings found by bisection and Φ at the window's
        // ends are computed separately, and a nanosecond's rounding must not drop an event.
        let first = self
            .phase_at(series, clock, window.start())
            .cycles()
            .saturating_sub(1)
            .max(EventBin::MIN.get());
        let last = self
            .phase_at(series, clock, window.end())
            .cycles()
            .saturating_add(1)
            .min(EventBin::MAX.get());
        let begin = out.len();
        for cycle in first..=last {
            if let Some(event) = self.event(series, clock, cycle)
                && window.contains(event.time)
            {
                out.push(event);
            }
        }
        debug_assert!(
            out[begin..].windows(2).all(|p| p[0].time < p[1].time),
            "a monotone phase's events came out of order"
        );
    }

    /// The event of cycle `cycle`, regenerated on its own; `None` if its skip mark removes it or
    /// the cycle lies beyond the 40-bit cycle numbers.
    #[must_use]
    pub fn event(
        &self,
        series: &EventSeries,
        clock: &impl PhaseClock,
        cycle: i64,
    ) -> Option<CycleEvent> {
        let bin = EventBin::new(cycle).ok()?;
        if self.skip.is_some_and(|skip| skip.skips(series.key(), bin)) {
            return None;
        }
        Some(CycleEvent {
            id: series.event_id(bin, 0),
            cycle,
            time: self.crossing(series.key(), clock, cycle),
            marks: series.key().event_stream(bin, 0),
        })
    }

    /// The event `id` names, regenerated on its own; `None` unless `id` is of `series` with number
    /// 0, or if its cycle is skipped.
    #[must_use]
    pub fn event_by_id(
        &self,
        series: &EventSeries,
        clock: &impl PhaseClock,
        id: EventId,
    ) -> Option<CycleEvent> {
        if !series.names(id) || id.word().number() != 0 {
            return None;
        }
        self.event(series, clock, id.word().bin().get())
    }

    /// The instant Φ reaches `n`: bisection on whole nanoseconds from a bracket of the noise
    /// bound plus one cycle on each side, returning the upper end, where Φ ≥ n.
    ///
    /// The bracket (its margin, [`noise_bound_cycles`](Self::noise_bound_cycles)'s form and the
    /// clock's inverse), the midpoint rule and the direction of the comparison are all output:
    /// Φ computed in `f64` is not monotone within a few units in the last place of n, over up to
    /// microseconds at periods of years, and there the path of the bisection decides which
    /// nanosecond it returns. Changing any of them moves event times and needs a version bump.
    #[must_use]
    fn crossing(&self, key: &EventKey, clock: &impl PhaseClock, n: i64) -> UniverseTime {
        // The lattice values of the coarse octaves repeat from one step to the next; the cache
        // only avoids recomputing them, so the result is the same bits as without it.
        let mut lattice = Lattice::new(self, *key);
        let (lo, hi) = self.bracket(clock, n);
        bisect(lo, hi, n, |t| self.phase_with(key, clock, t, &mut lattice))
    }

    /// The bracket of cycle `n`'s crossing, in nanoseconds: the clock's inverse at n minus and
    /// plus the noise bound and one cycle.
    #[must_use]
    fn bracket(&self, clock: &impl PhaseClock, n: i64) -> (i128, i128) {
        let margin = self.noise_bound_cycles() + 1.0;
        let target = Phase::whole(n);
        (
            to_nanos(clock.time_at(target.offset(-margin))),
            to_nanos(clock.time_at(target.offset(margin))),
        )
    }

    /// Φ(t), reading lattice values through `lattice`.
    #[must_use]
    fn phase_with(
        &self,
        key: &EventKey,
        clock: &impl PhaseClock,
        t: UniverseTime,
        lattice: &mut Lattice,
    ) -> Phase {
        let base = clock.base_phase(t);
        base.offset(self.noise(key, base, lattice))
    }

    /// N(φ): the octaves summed from the finest up, in that fixed order.
    #[must_use]
    fn noise(&self, key: &EventKey, base: Phase, lattice: &mut Lattice) -> f64 {
        let mut sum = 0.0;
        for j in 0..self.octaves {
            let span = i64::from(self.lattice_cycles) << j;
            let (index, s) = lattice_coordinate(base, span);
            let (h0, h1) = lattice.values(key, j, index);
            let smooth = s * s * (3.0 - 2.0 * s);
            sum += lattice.amplitude(j) * (h0 + (h1 - h0) * smooth);
        }
        sum
    }

    /// a × 2^(j/2): the square root of an exact power of two, correctly rounded.
    #[must_use]
    fn octave_amplitude(&self, j: u32) -> f64 {
        let power = math::powi(2.0, i32::try_from(j).expect("at most 48 octaves"));
        self.amplitude * power.sqrt()
    }
}

/// Bisection on whole nanoseconds between `lo` and `hi` for the first instant at which `phase`
/// reaches whole cycle `n`: the upper end once the two are a nanosecond apart.
#[must_use]
fn bisect(
    mut lo: i128,
    mut hi: i128,
    n: i64,
    mut phase: impl FnMut(UniverseTime) -> Phase,
) -> UniverseTime {
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if phase(from_nanos_saturating(mid)).cycles() >= n {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    from_nanos_saturating(hi)
}

/// The smallest J whose largest octave, ℓ × 2^(J − 1) cycles, spans `cycles`: the octaves a noise
/// needs for its phase to diffuse across that many cycles, at most
/// [`MonotonePhase::MAX_OCTAVES`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::events::octaves_to_span;
///
/// assert_eq!(octaves_to_span(4, 4.0), 1);
/// assert_eq!(octaves_to_span(4, 5.0), 2);
/// assert_eq!(octaves_to_span(4, 1e6), 19);
/// ```
#[must_use]
pub fn octaves_to_span(lattice_cycles: u32, cycles: f64) -> u32 {
    let mut octaves = 1;
    let mut span = f64::from(lattice_cycles.max(1));
    while span < cycles && octaves < MonotonePhase::MAX_OCTAVES {
        span *= 2.0;
        octaves += 1;
    }
    octaves
}

/// The lattice index and the fraction through it of `phase` on a lattice of `span` whole cycles:
/// the index by exact integer division, the fraction as `(remainder + fraction) ÷ span`.
#[must_use]
fn lattice_coordinate(phase: Phase, span: i64) -> (i64, f64) {
    let index = phase.cycles().div_euclid(span);
    // The Euclidean remainder from the quotient. It lies in [0, span), so wrapping arithmetic
    // gives it exactly even where the product itself would overflow, near `i64::MIN`.
    let remainder = phase.cycles().wrapping_sub(index.wrapping_mul(span));
    #[expect(
        clippy::cast_precision_loss,
        reason = "a remainder below the span, over the span: a fraction to the last place"
    )]
    let s = (remainder as f64 + phase.fraction()) / span as f64;
    // Rounding can reach 1 when the remainder is span − 1 and the fraction nearly 1.
    if s >= 1.0 {
        (index.saturating_add(1), 0.0)
    } else {
        (index, s)
    }
}

/// What one evaluation of the noise needs, kept for the length of one call: each octave's
/// amplitude, and its last lattice values.
///
/// Bisection repeats the coarse octaves' lattice points from step to step; this avoids recomputing
/// them, and the amplitudes, and nothing else. Every value is what a fresh computation gives, so
/// the result is the same bits with or without it. Measured on the `events/monotone_phase_root`
/// bench (16 octaves, about 65 steps): 55 µs a root without the lattice values kept, 30 µs with.
struct Lattice {
    key: EventKey,
    amplitudes: [f64; MonotonePhase::MAX_OCTAVES as usize],
    cached: [Option<(i64, f64, f64)>; MonotonePhase::MAX_OCTAVES as usize],
}

impl Lattice {
    /// The cache of one evaluation of `phase` under `key`.
    #[must_use]
    fn new(phase: &MonotonePhase, key: EventKey) -> Self {
        let mut amplitudes = [0.0; MonotonePhase::MAX_OCTAVES as usize];
        for (j, amplitude) in (0..phase.octaves).zip(amplitudes.iter_mut()) {
            *amplitude = phase.octave_amplitude(j);
        }
        Self {
            key,
            amplitudes,
            cached: [None; MonotonePhase::MAX_OCTAVES as usize],
        }
    }

    /// Octave `octave`'s amplitude, a × 2^(j/2).
    #[must_use]
    fn amplitude(&self, octave: u32) -> f64 {
        self.amplitudes[usize::try_from(octave).expect("at most 48 octaves")]
    }

    /// Octave `octave`'s values at `index` and `index + 1`.
    #[must_use]
    fn values(&mut self, key: &EventKey, octave: u32, index: i64) -> (f64, f64) {
        debug_assert_eq!(*key, self.key, "a lattice cache serves one event key");
        let slot = &mut self.cached[usize::try_from(octave).expect("at most 48 octaves")];
        match *slot {
            Some((cached, h0, h1)) if cached == index => (h0, h1),
            _ => {
                let values = (
                    lattice_value(key, octave, index),
                    lattice_value(key, octave, index.wrapping_add(1)),
                );
                *slot = Some((index, values.0, values.1));
                values
            }
        }
    }
}

/// Octave `octave`'s hashed value at lattice index `index`, in `[−1, 1)`: the first word of event
/// stream `octave + 1` of "bin" `index` reduced to 40 bits, as a uniform mapped onto `[−1, 1)`.
#[must_use]
fn lattice_value(key: &EventKey, octave: u32, index: i64) -> f64 {
    let bin = EventBin::from_field(index.cast_unsigned());
    let stream = u8::try_from(octave + 1).expect("at most 48 octaves");
    2.0 * key.event_stream(bin, stream).uniform() - 1.0
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::events::tags;
    use crate::events::testing::{assert_partition_independent, partition};
    use crate::events::{EventSubject, from_nanos};
    use crate::id::{BodyId, SystemId, event_tags};
    use crate::rng::Seed;
    use crate::time::SourceHorizon;

    const SEED: Seed = Seed::new(0x0e7e_0006_0027_0003);
    const DAY: i64 = 86_400;

    fn body(n: u64) -> EventSubject {
        let system = SystemId::from_raw(0x0200_0800_2000_0000 | (n << 13)).unwrap();
        BodyId::new(system, 0).into()
    }

    fn pulses(n: u64) -> EventSeries {
        EventSeries::new(SEED, tags::STAR_THERMAL_PULSE, body(n))
    }

    fn at(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    fn daily() -> LinearClock {
        LinearClock::new(Span::from_seconds(DAY), at(-3_600, 0)).unwrap()
    }

    /// Twelve octaves of a four-cycle lattice: the phase diffuses up to 8,192 cycles, with a noise
    /// bound of 45.6 cycles and a slope bound of 0.77.
    fn noise() -> MonotonePhase {
        MonotonePhase::new(0.3, 4, 12).unwrap()
    }

    fn list(
        phase: MonotonePhase,
        series: &EventSeries,
        clock: &impl PhaseClock,
        w: TimeWindow,
    ) -> Vec<CycleEvent> {
        let mut out = Vec::new();
        phase.events_in(series, clock, w, &mut out);
        out
    }

    fn days(from: i64, to: i64) -> TimeWindow {
        TimeWindow::new(at(from * DAY, 0), at(to * DAY, 0)).unwrap()
    }

    #[test]
    fn a_phase_keeps_its_fraction_in_zero_to_one() {
        let cases = [
            (Phase::new(3, 2.25), (5, 0.25)),
            (Phase::new(3, -0.25), (2, 0.75)),
            (Phase::new(-7, -3.5), (-11, 0.5)),
            (Phase::new(0, -1e-20), (0, 0.0)),
            (Phase::new(9, 0.0), (9, 0.0)),
            (Phase::new(i64::MAX, 5.5), (i64::MAX, 0.5)),
            (Phase::whole(-4).offset(0.125), (-4, 0.125)),
            (Phase::whole(2).offset(-2.5), (-1, 0.5)),
        ];
        for (phase, (cycles, fraction)) in cases {
            assert_eq!(phase.cycles(), cycles, "{phase:?}");
            assert_same_bits(phase.fraction(), fraction);
        }
        assert_same_bits(Phase::new(-2, 0.25).to_f64(), -1.75);
        assert!(Phase::new(1, 0.5) < Phase::new(2, 0.0));
        assert!(Phase::new(-1, 0.75) > Phase::new(-1, 0.5));
    }

    #[test]
    fn a_clock_period_is_at_least_16_seconds() {
        let short = Span::new(15, 999_999_999).unwrap();
        assert_eq!(
            LinearClock::new(short, UniverseTime::EPOCH),
            Err(BuildLinearClockError::PeriodTooShort { period: short })
        );
        assert_eq!(
            BuildLinearClockError::PeriodTooShort { period: short }.to_string(),
            "a clock period of +15.999999999 s is shorter than the +16.000000000 s minimum"
        );
        let clock = LinearClock::new(LinearClock::MIN_PERIOD, at(5, 0)).unwrap();
        assert_eq!(
            (clock.period(), clock.origin()),
            (LinearClock::MIN_PERIOD, at(5, 0))
        );
    }

    /// Over the whole source horizon, at the shortest period and an awkward one, the phase and its
    /// inverse agree to a nanosecond, which is 6 × 10⁻¹¹ cycles at 16 s: far inside the 10⁻⁶ cycles
    /// the plan asks for. An `f64` of cycles would resolve only 6 × 10⁻⁵ there.
    #[test]
    fn a_linear_clock_is_exact_to_a_nanosecond_over_the_source_horizon() {
        let origin = at(123_456, 789);
        for period in [
            Span::from_seconds(16),
            Span::new(16, 123).unwrap(),
            Span::from_seconds(DAY),
        ] {
            let clock = LinearClock::new(period, origin).unwrap();
            let start = to_nanos(SourceHorizon::START);
            let end = to_nanos(SourceHorizon::END);
            let step = (end - start) / 997;
            for k in 0..=997 {
                let t = from_nanos(start + k * step + k * 7).unwrap();
                let phase = clock.base_phase(t);
                assert!((0.0..1.0).contains(&phase.fraction()));
                let back = to_nanos(clock.time_at(phase));
                assert!(
                    (to_nanos(t) - back).abs() <= 1,
                    "period {period}: {t} came back as {back} ns"
                );
                // The next nanosecond is at or past the phase: the inverse floors.
                assert!(clock.base_phase(from_nanos(back + 1).unwrap()) >= phase);
            }
            let cycles = clock.cycles_in(
                SourceHorizon::END
                    .checked_since(SourceHorizon::START)
                    .unwrap(),
            );
            assert!(cycles > 1.0);
        }
        let clock = LinearClock::new(Span::from_seconds(16), origin).unwrap();
        assert_eq!(clock.base_phase(origin), Phase::whole(0));
        assert_eq!(
            clock
                .base_phase(from_nanos(to_nanos(origin) - 1).unwrap())
                .cycles(),
            -1
        );
    }

    #[test]
    fn the_constructor_rejects_a_steep_or_unbounded_noise() {
        assert_eq!(
            MonotonePhase::new(-0.1, 4, 3),
            Err(BuildMonotonePhaseError::InvalidAmplitude { amplitude: -0.1 })
        );
        assert!(matches!(
            MonotonePhase::new(f64::NAN, 4, 3),
            Err(BuildMonotonePhaseError::InvalidAmplitude { .. })
        ));
        assert_eq!(
            MonotonePhase::new(0.1, 0, 3),
            Err(BuildMonotonePhaseError::ZeroLattice)
        );
        assert_eq!(
            MonotonePhase::new(0.1, 4, 0),
            Err(BuildMonotonePhaseError::OctavesOutOfRange { octaves: 0 })
        );
        assert_eq!(
            MonotonePhase::new(0.1, 4, 49),
            Err(BuildMonotonePhaseError::OctavesOutOfRange { octaves: 49 })
        );
        assert_eq!(
            MonotonePhase::new(0.1, 1 << 20, 44),
            Err(BuildMonotonePhaseError::LatticeTooLong {
                lattice_cycles: 1 << 20,
                octaves: 44
            })
        );
        // 10.3 × 0.35 ÷ 4 = 0.90125 > 0.9, and 0.34 gives 0.8755.
        assert!(matches!(
            MonotonePhase::new(0.35, 4, 3),
            Err(BuildMonotonePhaseError::SlopeTooSteep { .. })
        ));
        assert!(MonotonePhase::new(0.34, 4, 3).is_ok());
        assert!(MonotonePhase::new(0.0, 1, 48).is_ok());
        assert_eq!(
            BuildMonotonePhaseError::SlopeTooSteep { slope: 1.5 }.to_string(),
            "a phase noise slope bound of 1.5 exceeds 0.9, so events could change order"
        );
        let skip = SkipMark::from_probability(0.25);
        let phase = noise().with_skip(skip);
        assert_eq!(
            (
                phase.amplitude_cycles(),
                phase.lattice_cycles(),
                phase.octaves(),
                phase.skip()
            ),
            (0.3, 4, 12, Some(skip))
        );
        assert_eq!(noise().skip(), None);
    }

    #[test]
    fn octaves_are_sized_to_span_their_cycles() {
        assert_eq!(octaves_to_span(4, 0.5), 1);
        assert_eq!(octaves_to_span(4, 8.0), 2);
        assert_eq!(octaves_to_span(4, 8.5), 3);
        assert_eq!(octaves_to_span(0, 1.0), 1);
        assert_eq!(octaves_to_span(1, 1e300), MonotonePhase::MAX_OCTAVES);
        let clock = LinearClock::new(Span::from_seconds(16), UniverseTime::EPOCH).unwrap();
        let horizon = SourceHorizon::END
            .checked_since(SourceHorizon::START)
            .unwrap();
        let octaves = octaves_to_span(4, clock.cycles_in(horizon));
        let top = |j: u32| 4.0 * math::powi(2.0, i32::try_from(j).unwrap());
        assert!(top(octaves - 1) >= clock.cycles_in(horizon));
        assert!(top(octaves - 2) < clock.cycles_in(horizon));
        assert_eq!(octaves, 38);
    }

    /// Sampled densely, the noise never exceeds its amplitude bound, and its finite differences
    /// never exceed the slope bound 10.3 × a ÷ ℓ.
    #[test]
    fn the_noise_keeps_to_its_amplitude_and_slope_bounds() {
        let phase = MonotonePhase::new(0.34, 4, 12).unwrap();
        let key = *pulses(1).key();
        let bound = phase.noise_bound_cycles();
        let slope_bound = MonotonePhase::SLOPE_FACTOR * 0.34 / 4.0;
        let step = 1.0 / 64.0;
        let mut lattice = Lattice::new(&phase, key);
        let mut previous = phase.noise(&key, Phase::whole(-3_000), &mut lattice);
        let mut largest = 0.0_f64;
        for k in 1..=(6_000 * 64) {
            let x = Phase::whole(-3_000).offset(f64::from(k) * step);
            let value = phase.noise(&key, x, &mut lattice);
            assert!(value.abs() <= bound, "N = {value} beyond {bound}");
            let slope = (value - previous).abs() / step;
            assert!(slope <= slope_bound, "slope {slope} beyond {slope_bound}");
            largest = largest.max(value.abs());
            previous = value;
        }
        assert!(
            largest > 0.2 * bound,
            "the noise barely moved: {largest} of {bound}"
        );
    }

    /// Each event is the crossing of its cycle to the nanosecond, evaluated cold: Φ has reached n
    /// at its time and had not one nanosecond earlier. That also shows the bisection's lattice
    /// cache changes no bit.
    #[test]
    fn each_event_is_the_crossing_of_its_cycle_to_the_nanosecond() {
        let clock = daily();
        for n in 0..4 {
            let series = pulses(n);
            for event in list(noise(), &series, &clock, days(-60, 60)) {
                let t = event.time();
                let before = from_nanos(to_nanos(t) - 1).unwrap();
                assert_eq!(noise().phase_at(&series, &clock, t).cycles(), event.cycle());
                assert_eq!(
                    noise().phase_at(&series, &clock, before).cycles(),
                    event.cycle() - 1
                );
                assert_eq!(noise().cycle_at(&series, &clock, t).0, event.cycle());
            }
        }
    }

    /// The clock, noise, series and window of the partition tests.
    fn partition_case() -> (LinearClock, MonotonePhase, EventSeries, TimeWindow) {
        let clock = LinearClock::new(Span::new(DAY / 3, 17).unwrap(), at(-1_000, 3)).unwrap();
        let phase = noise().with_skip(SkipMark::from_probability(0.2));
        let whole = TimeWindow::new(at(-40 * DAY - 77, 0), at(35 * DAY, 999)).unwrap();
        (clock, phase, pulses(7), whole)
    }

    /// Cuts on an event's instant, a nanosecond either side of one, and many inside cycles.
    fn cut_sets(events: &[CycleEvent]) -> Vec<Vec<UniverseTime>> {
        vec![
            vec![at(0, 0)],
            vec![
                events[10].time(),
                from_nanos(to_nanos(events[20].time()) + 1).unwrap(),
            ],
            vec![
                from_nanos(to_nanos(events[30].time()) - 1).unwrap(),
                events[30].time(),
            ],
            (-39..35).map(|d| at(d * DAY + 11 * d, 0)).collect(),
        ]
    }

    /// The brainstorm's time test for the phase: the union over any partition is the whole, in any
    /// order of asking, across the epoch.
    #[test]
    fn the_listing_over_any_partition_is_the_listing_over_the_whole() {
        let (clock, phase, series, whole) = partition_case();
        let listing = |w: TimeWindow| list(phase, &series, &clock, w);
        let events = listing(whole);
        assert!(events.len() > 150, "{} events", events.len());
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
        let (clock, phase, series, whole) = partition_case();
        let listing = |w: TimeWindow| list(phase, &series, &clock, w);
        let events = listing(whole);
        for cuts in &cut_sets(&events) {
            let pieces = partition(whole, cuts);
            let threaded: Vec<Vec<CycleEvent>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&w| scope.spawn(move || listing(w)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), events);
        }
    }

    /// The lattice cache changes no bit: every crossing equals a bisection that evaluates each
    /// step cold, and a warm evaluation of Φ equals a cold one, including at a ten-year period
    /// where Φ is not monotone within a few units in the last place of n.
    #[test]
    fn the_lattice_cache_gives_the_same_bits_as_a_cold_evaluation() {
        let decade = LinearClock::new(Span::from_julian_years(10).unwrap(), at(5, 0)).unwrap();
        let fine = MonotonePhase::new(0.2, 3, 16).unwrap();
        for (clock, phase) in [(daily(), noise()), (decade, fine)] {
            let series = pulses(21);
            let key = *series.key();
            let mut warm = Lattice::new(&phase, key);
            for n in -60..60 {
                let (lo, hi) = phase.bracket(&clock, n);
                let cold = bisect(lo, hi, n, |t| {
                    phase.phase_with(&key, &clock, t, &mut Lattice::new(&phase, key))
                });
                assert_eq!(phase.crossing(&key, &clock, n), cold, "cycle {n}");
                let t = from_nanos(to_nanos(cold) + 12_345).unwrap();
                let warm_phase = phase.phase_with(&key, &clock, t, &mut warm);
                let cold_phase = phase.phase_at(&series, &clock, t);
                assert_eq!(warm_phase.cycles(), cold_phase.cycles());
                assert_same_bits(warm_phase.fraction(), cold_phase.fraction());
            }
        }
    }

    /// Over periods of years the inverse stays within max(1 ns, 2⁻⁵² P) of the instant.
    #[test]
    fn a_long_period_clock_inverts_within_its_bound() {
        for years in [3, 10, 10_000] {
            let period = Span::from_julian_years(years).unwrap();
            let clock = LinearClock::new(period, at(-77, 5)).unwrap();
            let bound = (to_nanos(UniverseTime::EPOCH.checked_add(period).unwrap()) >> 52).max(1);
            let start = to_nanos(SourceHorizon::START);
            let step = (to_nanos(SourceHorizon::END) - start) / 499;
            for k in 0..=499 {
                let t = from_nanos(start + k * step + 13 * k).unwrap();
                let error = to_nanos(t) - to_nanos(clock.time_at(clock.base_phase(t)));
                assert!(
                    error.abs() <= bound,
                    "{years} yr: {t} came back {error} ns away, bound {bound}"
                );
            }
        }
    }

    /// An ID of the series with number 0 gives its cycle's event; any other ID gives `None`.
    #[test]
    fn an_id_regenerates_its_cycle_event() {
        let clock = daily();
        let series = pulses(4);
        let event = noise().event(&series, &clock, 17).unwrap();
        assert_eq!(
            noise().event_by_id(&series, &clock, event.id()),
            Some(event)
        );
        let bin = EventBin::new(17).unwrap();
        let numbered_one = series.event_id(bin, 1);
        assert_eq!(noise().event_by_id(&series, &clock, numbered_one), None);
        let other = EventSeries::new(SEED, event_tags::SELF_TEST, body(4));
        let foreign = series.event_id(bin, 0);
        assert_eq!(noise().event_by_id(&other, &clock, foreign), None);
    }

    /// `event(cycle)` gives each listed event and `None` for each skipped cycle, the skips come at
    /// their probability, and a skipped cycle still counts in `cycle_at`.
    #[test]
    fn a_cycle_regenerates_its_event_and_a_skip_mark_removes_cycles_at_its_rate() {
        let clock = daily();
        let series = pulses(3);
        let phase = noise().with_skip(SkipMark::from_probability(0.3));
        let events = list(phase, &series, &clock, days(-2_000, 2_000));
        let first = events[0].cycle();
        let last = events[events.len() - 1].cycle();
        let mut skipped = 0_u64;
        for cycle in first..=last {
            let listed = events.iter().find(|e| e.cycle() == cycle);
            let regenerated = phase.event(&series, &clock, cycle);
            assert_eq!(regenerated.as_ref(), listed, "cycle {cycle}");
            if let Some(event) = regenerated {
                assert_eq!(
                    event.id(),
                    series.event_id(EventBin::new(cycle).unwrap(), 0)
                );
                assert_eq!(
                    *event.marks(),
                    series.key().event_stream(EventBin::new(cycle).unwrap(), 0)
                );
            } else {
                skipped += 1;
                // The unskipped noise still crosses the cycle.
                let crossing = noise().event(&series, &clock, cycle).unwrap();
                assert_eq!(phase.cycle_at(&series, &clock, crossing.time()).0, cycle);
            }
        }
        let total = u64::try_from(last - first + 1).unwrap();
        #[expect(clippy::cast_precision_loss, reason = "small counts")]
        let n = total as f64;
        let fit = chi_square_gof(&[skipped, total - skipped], &[0.3 * n, 0.7 * n]);
        assert_p_value("skipped share against 0.3", fit.p_value, ALPHA);
        assert_eq!(phase.event(&series, &clock, EventBin::MAX.get() + 1), None);
    }

    #[test]
    fn different_subjects_and_tags_give_different_phases_and_the_same_seed_the_same() {
        let clock = daily();
        let a = list(noise(), &pulses(1), &clock, days(0, 30));
        assert_eq!(list(noise(), &pulses(1), &clock, days(0, 30)), a);
        let times = |events: &[CycleEvent]| events.iter().map(CycleEvent::time).collect::<Vec<_>>();
        let b = list(noise(), &pulses(2), &clock, days(0, 30));
        assert_ne!(times(&a), times(&b));
        let other_tag = EventSeries::new(SEED, event_tags::SELF_TEST, body(1));
        assert_ne!(
            times(&a),
            times(&list(noise(), &other_tag, &clock, days(0, 30)))
        );
    }

    /// The drift from cycle 0, tₙ − t₀ − nP, has a variance across subjects that grows with n
    /// up to the top octave (8,192 cycles): the phase diffuses. A plain jittered lattice, the
    /// control, keeps it flat and fails the same test.
    #[test]
    fn the_phase_diffuses_where_a_jittered_lattice_does_not() {
        let clock = LinearClock::new(Span::from_seconds(DAY), UniverseTime::EPOCH).unwrap();
        let lags = [4_i64, 16, 64, 256, 1_024, 4_096];
        let subjects = 400_u64;
        let seconds =
            |a: UniverseTime, b: UniverseTime| b.checked_since(a).unwrap().as_seconds_f64();
        #[expect(clippy::cast_precision_loss, reason = "lags of a few thousand days")]
        let drift = |t0: UniverseTime, tn: UniverseTime, n: i64| seconds(t0, tn) - (n * DAY) as f64;
        let mut phase_drifts = vec![Vec::new(); lags.len()];
        let mut jitter_drifts = vec![Vec::new(); lags.len()];
        for m in 0..subjects {
            let series = pulses(100 + m);
            let t0 = noise().event(&series, &clock, 0).unwrap().time();
            // The control: each crossing at nP plus a uniform jitter of ±0.3 P, from its own mark.
            let jittered = |n: i64| {
                let jitter = series
                    .key()
                    .event_stream(EventBin::new(n).unwrap(), 0)
                    .uniform_in(-0.3, 0.3);
                #[expect(clippy::cast_precision_loss, reason = "lags of a few thousand days")]
                let at = (n * DAY) as f64 + jitter * 86_400.0;
                at
            };
            for (i, &n) in lags.iter().enumerate() {
                let tn = noise().event(&series, &clock, n).unwrap().time();
                phase_drifts[i].push(drift(t0, tn, n));
                #[expect(clippy::cast_precision_loss, reason = "lags of a few thousand days")]
                let control = jittered(n) - jittered(0) - (n * DAY) as f64;
                jitter_drifts[i].push(control);
            }
        }
        let variance = |xs: &[f64]| {
            #[expect(clippy::cast_precision_loss, reason = "a sample of 400")]
            let n = xs.len() as f64;
            let mean = xs.iter().sum::<f64>() / n;
            xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0)
        };
        // Diffusion: each fourfold lag at least doubles the variance, and 1,024 times the lag
        // gives at least 64 times the variance (a random walk gives 1,024).
        let diffuses = |drifts: &[Vec<f64>]| {
            let v: Vec<f64> = drifts.iter().map(|d| variance(d)).collect();
            v.windows(2).all(|p| p[1] > 2.0 * p[0]) && v[v.len() - 1] > 64.0 * v[0]
        };
        assert!(
            diffuses(&phase_drifts),
            "the monotone phase does not diffuse"
        );
        assert!(
            !diffuses(&jitter_drifts),
            "the jittered control should not diffuse"
        );
    }

    /// Ordered, none lost, none doubled, over a million cycles on both sides of the epoch, and the
    /// cycles at the window's ends agree with `cycle_at`.
    #[test]
    #[ignore = "slow: a million monotone-phase roots"]
    fn a_million_cycles_are_ordered_with_none_lost_or_doubled() {
        let clock = LinearClock::new(Span::from_seconds(3_600), at(-7, 0)).unwrap();
        let horizon = SourceHorizon::END
            .checked_since(SourceHorizon::START)
            .unwrap();
        let phase =
            MonotonePhase::new(0.3, 4, octaves_to_span(4, clock.cycles_in(horizon))).unwrap();
        let series = pulses(11);
        let window = TimeWindow::new(at(-500_000 * 3_600, 0), at(500_000 * 3_600, 0)).unwrap();
        let events = list(phase, &series, &clock, window);
        let (first, _) = phase.cycle_at(&series, &clock, window.start());
        let (last, _) = phase.cycle_at(
            &series,
            &clock,
            from_nanos(to_nanos(window.end()) - 1).unwrap(),
        );
        assert!(events.windows(2).all(|p| p[1].cycle() == p[0].cycle() + 1));
        assert!(events.windows(2).all(|p| p[0].time() < p[1].time()));
        assert!(events[0].time() < UniverseTime::EPOCH);
        let count = i64::try_from(events.len()).unwrap();
        assert!((999_000..=1_001_000).contains(&count), "{count} events");
        // The first event is the first cycle begun inside the window, the last the one current at
        // its end.
        let first_event = events[0].cycle();
        assert!(
            first_event == first || first_event == first + 1,
            "the first event is cycle {first_event}, the window starts in cycle {first}"
        );
        assert_eq!(events[events.len() - 1].cycle(), last);
    }

    /// Pinned keys: the first events after the epoch of three series, and phases at fixed instants.
    #[test]
    fn phase_events_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let system = SystemId::from_raw(0xF000_0007_0000_0000).unwrap();
        let cases = [
            (
                "thermal pulses, daily clock, 12 octaves",
                pulses(0),
                noise(),
            ),
            (
                "self-test, daily clock, 12 octaves, skip 0.4",
                EventSeries::new(SEED, event_tags::SELF_TEST, system.into()),
                noise().with_skip(SkipMark::from_probability(0.4)),
            ),
            (
                "thermal pulses of body 2, 20 octaves",
                EventSeries::new(
                    SEED,
                    tags::STAR_THERMAL_PULSE,
                    BodyId::new(system, 2).into(),
                ),
                MonotonePhase::new(0.2, 3, 20).unwrap(),
            ),
        ];
        let clock = daily();
        // Where Φ is not monotone within a few ulps of n the bisection's path picks the
        // nanosecond; a ten-year period with 16 octaves pins such crossings.
        let decade =
            LinearClock::new(Span::from_julian_years(10).unwrap(), UniverseTime::EPOCH).unwrap();
        w.line("");
        w.line("# thermal pulses, ten-year clock, 16 octaves, cycles −40 to 40");
        let fine = MonotonePhase::new(0.2, 3, 16).unwrap();
        for cycle in -40..=40 {
            let event = fine.event(&pulses(0), &decade, cycle).unwrap();
            w.line(&format!(
                "{} cycle {} at {}",
                event.id(),
                event.cycle(),
                event.time()
            ));
        }
        for (name, series, phase) in cases {
            w.line("");
            w.line(&format!("# {name}"));
            for event in list(phase, &series, &clock, days(-3, 5)) {
                w.u64_hex(
                    &format!("{} cycle {} at {}", event.id(), event.cycle(), event.time()),
                    event.into_marks().next_u64(),
                );
            }
            for t in [at(-DAY, 0), UniverseTime::EPOCH, at(123_456_789, 42)] {
                let p = phase.phase_at(&series, &clock, t);
                w.f64(
                    &format!("phase at {t}: cycle {}, fraction", p.cycles()),
                    p.fraction(),
                );
            }
        }
        golden!("events/phase", w.as_str());
    }
}
