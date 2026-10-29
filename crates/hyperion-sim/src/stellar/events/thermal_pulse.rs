//! Thermal pulses: the helium-shell flashes of stars on the thermally pulsing asymptotic giant
//! branch, the sixth single-star event kind (plan 06, P06.T28.f).
//!
//! # Who pulses
//!
//! A star whose track holds a thermally pulsing AGB segment ([`Phase::ThermallyPulsingAgb`],
//! P06.T8.b) with its integrated pulse count (P06.T10.c): the stars of about 0.8–8 M☉ that reach
//! the AGB, once their track is built to it (to +H, or to the star's death). A star still short of
//! it at +H, one that never reaches it, and an object below 0.1 M☉ have no pulses here. The
//! segment is found once per call.
//!
//! # The clock
//!
//! The base phase φ(t) is the number of thermal pulses since the start of the phase: the integral
//! of 1 ÷ `τ_ip`(`M_c`, `M_env`) along the segment, Wagenhuber and Groenewegen's (1998, A&A 340,
//! 183, equation 11) interpulse period ([`ThermalPulseModel::interpulse_period_at`]), which the
//! track integrates with its rate at each of the segment's knots (P06.T10.c) and interpolates
//! between them by a monotone cubic. The age at t is read in its two parts, the age at the epoch
//! and the years since it, as the track's mass is for a planet's orbit (P14.T45.d), so that the
//! clock follows the clock time without forming an age at t (design note 23). Within ±H that is
//! the plan's form, the integral at the epoch plus the clock time since the epoch times the epoch's
//! pulse frequency, to under 10⁻³ cycles for most stars (a unit test measures 1–4 M☉), growing with
//! ḟ to some 10⁻² cycles for the short periods of the most massive (6–7 M☉, `τ_ip` of 100–300
//! years); across the source horizon it stays the track's pulse count, from which the first-order
//! form would drift by ½ ḟ Δt², tens of cycles for the shortest phases, which the source horizon
//! spans whole.
//!
//! Before the segment and after it φ continues linearly at the segment's mean rate, so that it is
//! strictly increasing on the whole clock, as a [`PhaseClock`] must be; pulses fall only inside
//! the segment. Inside it the count is held to [0, n] at its ends, and before it the continuation
//! is lowered by 10⁻⁹ cycles ([`START_STEP_CYCLES`]): a phase split into whole cycles and a fraction
//! cannot hold a value within 10⁻¹⁶ below a whole number, so without the step the continuation's
//! last instants would read as cycle 0 and the first pulse would fall a fraction of a millisecond
//! before the phase.
//!
//! # Pulses
//!
//! A [`MonotonePhase`] on that clock (design note 16; the brainstorm's construction for thermal
//! pulses): pulse n falls where Φ = ψ + N(ψ) crosses n, N the star's own bounded-slope noise of
//! [`NOISE_AMPLITUDE_CYCLES`] on a lattice of [`NOISE_LATTICE_CYCLES`] cycle, with as many octaves
//! as span the cycles of the source horizon at the phase's mean rate, so that the pulses' phase
//! wanders as the brainstorm asks. ψ is φ plus the series' anchor, the one offset c for which
//! c + N(c) = 0, so that pulse 0 falls on the phase's first instant exactly, where HPT and
//! Wagenhuber and Groenewegen put the first pulse, whatever the noise. The pulse number n(t),
//! ⌊Φ(t)⌋ on the phase ([`ThermalPulseModel::pulse_number_at`]), is the number of pulses since the
//! first, which P06.T24.b's C/O reads as a closed form; it wanders from the track's count by at
//! most the noise's bound.
//!
//! # The transient
//!
//! A pulse cycle's light, as a function of its fractional phase f and its pulse number
//! ([`cycle_luminosity_factor`]): a rapid peak after the flash, over the first
//! [`RAPID_PEAK_SHARE`] of the cycle, of [`RAPID_PEAK_DEX`] at most; then the slow dip, over the
//! next [`DIP_SHARE`], down by [`DIP_DEX`] at [`DIP_MINIMUM`] of its span and back; then the
//! quiescent luminosity of the interpulse phase, which is the track's (HPT's core mass–luminosity
//! relation), until the next pulse. Each piece is a smoothstep in f, so the factor is continuous,
//! with no step at a pulse. The first pulses are weak and grow: the excursion in dex is scaled by
//! 1 − e^(−(n + 1) ÷ 3) ([`PULSE_STRENGTH_PULSES`]), 0.28 at the first pulse and 0.96 by the
//! tenth. The shape follows the descriptions of a pulse cycle's surface luminosity in Boothroyd
//! and Sackmann's series on low-mass stars (1988, ApJ 328, 632–671) and in Wagenhuber and
//! Groenewegen's analytic fits (1998), and the growth of the first pulses Iben and Renzini's
//! review (1983, ARA&A 21, 271) and Herwig's (2005, ARA&A 43, 435), all recalled and not read at
//! source in this build; the six figures are this module's own, provisional, and the same for
//! every star. A cycle at full strength averages 0.920 of the track's luminosity (a unit test
//! measures it). The factor steps back to 1 where the phase ends during a transient.
//!
//! # Marks
//!
//! None: nothing here is drawn but the noise's lattice. A pulse's own stream,
//! [`CycleEvent::marks`], is left unread; scatter added later takes its words 0, 1, … in a fixed
//! order.
//!
//! An event may change what a console reads, never what the star's track is (design note 14): the
//! carbon the pulses dredge up is P06.T24.b's closed form in the pulse number, and nothing here
//! changes the star.
//!
//! [`Phase::ThermallyPulsingAgb`]: crate::stellar::Phase::ThermallyPulsingAgb

use crate::events::{
    CycleEvent, EventId, EventSeries, MonotonePhase, Phase, PhaseClock, TimeWindow,
    from_nanos_saturating, octaves_to_span, tags, to_nanos,
};
use crate::math;
use crate::stellar::sse::{self, Track, ZCoeffs};
use crate::stellar::system::StarModel;
use crate::time::{SourceHorizon, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{SolarMasses, Years};

/// The amplitude of the pulses' phase noise at its smallest octave, cycles: 0.03, ours.
///
/// The interpulse period is a closed form of the core mass and the envelope (Wagenhuber and
/// Groenewegen 1998, equation 11), and no source read gives a scatter about it, so the amplitude
/// is small, that of P06.T26.d's Miras; the brainstorm's monotone phase asks that the phase
/// wander, and the octaves let it, over the whole source horizon.
pub const NOISE_AMPLITUDE_CYCLES: f64 = 0.03;

/// The lattice spacing of the noise's smallest octave, cycles: one, so that each pulse has its own
/// value, as P06.T28.b's Vela-like glitches do.
pub const NOISE_LATTICE_CYCLES: u32 = 1;

/// The share of a pulse cycle over which the rapid luminosity peak after the flash rises and
/// falls: 1%, this module's provisional figure (see "The transient" in the
/// [module documentation](self)).
pub const RAPID_PEAK_SHARE: f64 = 0.01;

/// The rapid peak's height at full strength, dex in luminosity: +0.1, this module's provisional
/// figure.
pub const RAPID_PEAK_DEX: f64 = 0.1;

/// The share of a pulse cycle over which the slow dip that follows the rapid peak falls and
/// recovers: 30%, this module's provisional figure.
pub const DIP_SHARE: f64 = 0.3;

/// The slow dip's depth at full strength, dex in luminosity: 0.3, a factor of two at its minimum,
/// this module's provisional figure.
pub const DIP_DEX: f64 = 0.3;

/// Where in its span the slow dip reaches its minimum: a quarter of the way, falling faster than
/// it recovers; this module's provisional figure.
pub const DIP_MINIMUM: f64 = 0.25;

/// The share of a pulse cycle over which its light departs from the track's, the rapid peak and
/// the slow dip: 0.31.
pub const TRANSIENT_SHARE: f64 = RAPID_PEAK_SHARE + DIP_SHARE;

/// The pulses over which the first pulses grow to full strength, the e-folding of
/// [`pulse_strength`]: three, this module's provisional figure.
pub const PULSE_STRENGTH_PULSES: f64 = 3.0;

/// The step by which the clock's continuation before the thermally pulsing AGB is lowered, cycles:
/// 10⁻⁹, far above the 10⁻¹⁶ that a split phase loses just below a whole number and far below
/// anything a pulse's light or time reads (see "The clock" in the [module documentation](self)).
pub const START_STEP_CYCLES: f64 = 1e-9;

/// The margin added to the noise's anchor once it is found, cycles: 10⁻¹², far above the phase's
/// rounding at any noise bound (about 2 × 10⁻¹⁵ at a bound of nine cycles) and far below
/// [`START_STEP_CYCLES`], so that the phase is clearly at or above 0 from the thermally pulsing
/// AGB's first instant on, and still below it a nanosecond before.
pub const ANCHOR_MARGIN_CYCLES: f64 = 1e-12;

/// Nanoseconds in one second, for the clock's arithmetic.
const NANOS_PER_SECOND: f64 = 1e9;

/// The smoothstep 3s² − 2s³ of `s`, for `s` in [0, 1].
#[must_use]
fn smoothstep(s: f64) -> f64 {
    s * s * (3.0 - 2.0 * s)
}

/// The strength of pulse `number` (0 for the first) against a full-strength one: 1 −
/// e^(−(n + 1) ÷ 3), 0.28 for the first pulse, 0.74 for the fourth and 0.96 for the tenth, and 0
/// for a number below 0.
///
/// The first pulses of a thermally pulsing AGB star are weak and grow over several cycles (Iben and
/// Renzini 1983, ARA&A 21, 271; Herwig 2005, ARA&A 43, 435; recalled, not read at source in this
/// build); the e-folding of three pulses ([`PULSE_STRENGTH_PULSES`]) is this module's provisional
/// figure.
#[must_use]
pub fn pulse_strength(number: i64) -> f64 {
    if number < 0 {
        return 0.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a pulse number, exact in f64 below 2^53 and of no weight beyond a few dozen"
    )]
    let n = number as f64;
    -math::exp_m1(-(n + 1.0) / PULSE_STRENGTH_PULSES)
}

/// The excursion of a full-strength pulse cycle's light at its fractional phase `f` in [0, 1),
/// dex: the rapid peak's rise and fall, then the slow dip's, then none.
#[must_use]
fn cycle_dex(f: f64) -> f64 {
    if f < RAPID_PEAK_SHARE {
        let u = f / RAPID_PEAK_SHARE;
        let s = if u < 0.5 { 2.0 * u } else { 2.0 - 2.0 * u };
        RAPID_PEAK_DEX * smoothstep(s)
    } else {
        let u = (f - RAPID_PEAK_SHARE) / DIP_SHARE;
        let s = if u < DIP_MINIMUM {
            u / DIP_MINIMUM
        } else {
            (1.0 - u) / (1.0 - DIP_MINIMUM)
        };
        -DIP_DEX * smoothstep(s.clamp(0.0, 1.0))
    }
}

/// The luminosity of pulse `number`'s cycle at its fractional phase `fraction`, over the track's
/// quiescent luminosity.
///
/// It is 10^(s Δ(f)), with s the pulse's [`pulse_strength`] and Δ the rapid peak's rise and fall
/// over the first [`RAPID_PEAK_SHARE`] of the cycle, up to [`RAPID_PEAK_DEX`], the slow dip's fall
/// and recovery over the next [`DIP_SHARE`], down to −[`DIP_DEX`] at [`DIP_MINIMUM`] of its span,
/// and 0 for the rest of the cycle; each piece a smoothstep, so the factor is continuous in f, with
/// zero slope where the pieces meet, and 1 at the cycle's start and over its quiescent part. A
/// fraction outside [0, 1) is wrapped into it.
///
/// # Panics
///
/// If `fraction` is not finite, as [`Phase::new`] panics on one.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::events::thermal_pulse::{
///     DIP_MINIMUM, DIP_SHARE, RAPID_PEAK_SHARE, cycle_luminosity_factor,
/// };
///
/// // A pulse late in the phase, at full strength.
/// let late = |f: f64| cycle_luminosity_factor(60, f);
/// assert!((late(0.0) - 1.0).abs() < 1e-15);
/// // The rapid peak, 10^0.1 = 1.26, half-way through its share of the cycle.
/// assert!((late(RAPID_PEAK_SHARE / 2.0) - 1.2589).abs() < 1e-4);
/// // The slow dip, half the light, at its minimum.
/// let minimum = RAPID_PEAK_SHARE + DIP_MINIMUM * DIP_SHARE;
/// assert!((late(minimum) - 0.5012).abs() < 1e-4);
/// // Quiescent for the rest of the cycle.
/// assert!((late(0.5) - 1.0).abs() < 1e-15);
/// // The first pulse dips by a little over a quarter as much in dex: to 0.82.
/// assert!((cycle_luminosity_factor(0, minimum) - 0.8222).abs() < 1e-4);
/// ```
#[must_use]
pub fn cycle_luminosity_factor(number: i64, fraction: f64) -> f64 {
    assert!(fraction.is_finite(), "a cycle's fraction is {fraction}");
    let f = fraction - fraction.floor();
    if f >= TRANSIENT_SHARE {
        return 1.0;
    }
    math::exp10(pulse_strength(number) * cycle_dex(f))
}

/// The clock time, as nanoseconds since the epoch, at which a star whose age at the epoch is
/// `age_at_epoch` is `age` old: the epoch plus (age − age at the epoch) × 31,557,600 s, floored to
/// the nanosecond ([`Span::from_seconds_f64`]), as [`StarModel`]'s mass breaks place an age, or
/// `None` beyond the clock's range.
#[must_use]
fn clock_nanos_of_age(age_at_epoch: Years, age: Years) -> Option<i128> {
    let seconds = (age.value() - age_at_epoch.value()) * SECONDS_PER_JULIAN_YEAR;
    Span::from_seconds_f64(seconds)
        .and_then(|span| UniverseTime::EPOCH.checked_add(span))
        .map(to_nanos)
}

/// The span from `from` to `to` nanoseconds, in seconds, rounded.
#[must_use]
fn seconds_between(from: i128, to: i128) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a span of the clock in nanoseconds, rounded to 53 bits: a part in 10^16"
    )]
    let nanos = (to - from) as f64;
    nanos / NANOS_PER_SECOND
}

/// The pulse clock of a star on the thermally pulsing AGB: the base phase ψ(t), its thermal
/// pulses since the start of the phase plus an anchor (see "The clock" in the
/// [module documentation](self)).
///
/// It holds the segment's start and end as clock times, turned once from ages, the age at the
/// epoch for the two-part reading, the segment's pulses and mean frequency, by which the phase
/// continues outside it, and the anchor added to the count, zero for the track's own count.
///
/// Its inverse, [`time_at`](PhaseClock::time_at), brackets each pulse's crossing, so its form (the
/// nanosecond bisection inside the phase, its midpoint and its `>=`, and the lines outside it) is
/// part of the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalPulseClock<'a> {
    track: &'a Track,
    age_at_epoch: Years,
    start_age: Years,
    end_age: Years,
    /// The segment's first and last instant, nanoseconds since the epoch.
    start: i128,
    end: i128,
    /// The pulses at the segment's end.
    pulses: f64,
    /// The segment's mean frequency, pulses per second.
    frequency_hz: f64,
    /// The anchor added to the count, cycles.
    anchor: f64,
}

impl<'a> ThermalPulseClock<'a> {
    /// The clock of `star`, on the track's own count (no anchor), or `None` if its track holds no
    /// thermally pulsing AGB with pulses, or the phase lies beyond the clock's range.
    #[must_use]
    pub fn of(star: &'a StarModel) -> Option<Self> {
        let track = star.track()?;
        let span = track.pulsing_agb()?;
        let (start_age, end_age) = (span.start(), span.end());
        let pulses = track.pulse_phase_at(end_age.value(), 0.0)?;
        if !(pulses > 0.0 && pulses.is_finite()) {
            return None;
        }
        let age_at_epoch = star.age_at_epoch();
        let start = clock_nanos_of_age(age_at_epoch, start_age)?;
        let end = clock_nanos_of_age(age_at_epoch, end_age)?;
        if end <= start {
            return None;
        }
        Some(Self {
            track,
            age_at_epoch,
            start_age,
            end_age,
            start,
            end,
            pulses,
            frequency_hz: pulses / seconds_between(start, end),
            anchor: 0.0,
        })
    }

    /// This clock with the anchor `anchor` added to its count, cycles.
    #[must_use]
    pub const fn anchored(self, anchor: f64) -> Self {
        Self { anchor, ..self }
    }

    /// The start of the thermally pulsing AGB, its first pulse, as a clock time.
    #[must_use]
    pub fn start(&self) -> UniverseTime {
        from_nanos_saturating(self.start)
    }

    /// The end of the thermally pulsing AGB, as a clock time: the first instant after it.
    #[must_use]
    pub fn end(&self) -> UniverseTime {
        from_nanos_saturating(self.end)
    }

    /// The age at which the thermally pulsing AGB starts, Julian years since the onset of
    /// collapse.
    #[must_use]
    pub const fn start_age(&self) -> Years {
        self.start_age
    }

    /// The age at which the thermally pulsing AGB ends.
    #[must_use]
    pub const fn end_age(&self) -> Years {
        self.end_age
    }

    /// The pulses from the start of the phase to its end, the track's count there: not a whole
    /// number.
    #[must_use]
    pub const fn pulses_at_end(&self) -> f64 {
        self.pulses
    }

    /// The segment's mean pulse frequency, pulses per second, at which the phase continues outside
    /// it.
    #[must_use]
    pub const fn mean_frequency_hz(&self) -> f64 {
        self.frequency_hz
    }

    /// The anchor added to the track's count, cycles: 0 for [`of`](Self::of)'s clock.
    #[must_use]
    pub const fn anchor_cycles(&self) -> f64 {
        self.anchor
    }

    /// Whether `t` lies in the phase, `[start, end)`.
    #[must_use]
    pub fn holds(&self, t: UniverseTime) -> bool {
        let t = to_nanos(t);
        self.start <= t && t < self.end
    }

    /// The track's count at `t` nanoseconds since the epoch, held to [0, n] inside the phase and
    /// continued linearly outside it (the continuation before it lowered by the start's step), with
    /// no anchor.
    #[must_use]
    fn count_at_nanos(&self, t: i128) -> f64 {
        if t < self.start {
            return self.frequency_hz * seconds_between(self.start, t) - START_STEP_CYCLES;
        }
        if t >= self.end {
            return math::mul_add(self.frequency_hz, seconds_between(self.end, t), self.pulses);
        }
        let years = from_nanos_saturating(t).since_epoch().as_julian_years_f64();
        // The age formed from the two parts may round by half a unit in its last place outside the
        // segment at its ends, seconds at gigayears, where the count is its end's.
        self.track
            .pulse_phase_at(self.age_at_epoch.value(), years)
            .unwrap_or(if t - self.start < self.end - t {
                0.0
            } else {
                self.pulses
            })
            .clamp(0.0, self.pulses)
    }

    /// The first nanosecond at which the anchored count reaches `target`: inside the phase by
    /// bisection on the anchored count itself, before and after it by the continuations' lines.
    #[must_use]
    fn time_of_phase(&self, target: f64) -> UniverseTime {
        let nanos_after = |from: i128, seconds: f64| {
            let span = Span::from_seconds_f64(seconds).map_or(
                if seconds > 0.0 {
                    i128::MAX / 2
                } else {
                    i128::MIN / 2
                },
                |s| i128::from(s.seconds()) * 1_000_000_000 + i128::from(s.subsec_nanos()),
            );
            from_nanos_saturating(from.saturating_add(span))
        };
        let count = target - self.anchor;
        if count <= -START_STEP_CYCLES {
            return nanos_after(self.start, (count + START_STEP_CYCLES) / self.frequency_hz);
        }
        if count <= 0.0 {
            return self.start();
        }
        if count >= self.pulses {
            return nanos_after(self.end, (count - self.pulses) / self.frequency_hz);
        }
        // The anchored count is below the target at `lo` and at or above it at `hi`.
        let (mut lo, mut hi) = (self.start, self.end);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if self.count_at_nanos(mid) + self.anchor >= target {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        from_nanos_saturating(hi)
    }
}

impl PhaseClock for ThermalPulseClock<'_> {
    fn base_phase(&self, t: UniverseTime) -> Phase {
        Phase::new(0, self.count_at_nanos(to_nanos(t)) + self.anchor)
    }

    fn time_at(&self, phase: Phase) -> UniverseTime {
        self.time_of_phase(phase.to_f64())
    }
}

/// A clock that stands at one base phase, for reading a noise at it.
struct StandingClock(f64);

impl PhaseClock for StandingClock {
    fn base_phase(&self, _t: UniverseTime) -> Phase {
        Phase::new(0, self.0)
    }

    fn time_at(&self, _phase: Phase) -> UniverseTime {
        UniverseTime::EPOCH
    }
}

/// The anchor c of `noise` on `series`: where c + N(c), as the monotone phase forms it, reaches
/// cycle 0, found by bisection on `f64`, plus [`ANCHOR_MARGIN_CYCLES`]. Then the phase is at or
/// above 0 from the clock's start on, whatever the count's rounding just after it, and below 0 a
/// nanosecond before it.
#[must_use]
fn anchor_of(noise: &MonotonePhase, series: &EventSeries) -> f64 {
    let reaches = |c: f64| {
        noise
            .phase_at(series, &StandingClock(c), UniverseTime::EPOCH)
            .cycles()
            >= 0
    };
    let margin = noise.noise_bound_cycles() + 1.0;
    let (mut lo, mut hi) = (-margin, margin);
    // About 60 halvings of an interval of a few cycles; the cap is more than any needs.
    for _ in 0..1_100 {
        let mid = f64::midpoint(lo, hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if reaches(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi + ANCHOR_MARGIN_CYCLES
}

/// The thermal pulses of one star on one series: its pulse clock, anchored for the series, and
/// the monotone phase they fall on (plan 06, P06.T28.f; see the [module documentation](self)).
///
/// # Examples
///
/// A 2 M☉ star a third of the way through its thermally pulsing AGB at the epoch: its pulse number
/// and its interpulse period then, and its light through its next pulse's cycle.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventSeries, TimeWindow, tags};
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::thermal_pulse::{ThermalPulseClock, ThermalPulseModel};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::{Span, UniverseTime};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let (mass, solar) = (SolarMasses::new(2.0), Composition::SOLAR);
/// // Built old enough that its track holds the thermally pulsing AGB, and then again inside it.
/// let old = StarModel::new(mass, solar, StarDraws::median(), Years::new(2e9))?;
/// let clock = ThermalPulseClock::of(&old).ok_or("a 2 M☉ star pulses")?;
/// let (start, end) = (clock.start_age().value(), clock.end_age().value());
/// let star = StarModel::new(mass, solar, StarDraws::median(), Years::new(start + (end - start) / 3.0))?;
/// let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let series = EventSeries::new(Seed::new(7), tags::STAR_THERMAL_PULSE, body.into());
/// let model = ThermalPulseModel::of(&star, &series).ok_or("on the thermally pulsing AGB")?;
/// let n = model.pulse_number_at(UniverseTime::EPOCH).ok_or("pulsing")?;
/// assert!(n >= 1);
/// let period = model.interpulse_period_at(UniverseTime::EPOCH).ok_or("pulsing")?;
/// assert!((1e4..1e6).contains(&period.value()));
/// // The next pulse, and the star's light through the dip that follows it.
/// let mut pulses = Vec::new();
/// model.pulses_in(TimeWindow::new(UniverseTime::EPOCH, model.clock().end())?, &mut pulses);
/// let next = pulses.first().ok_or("more pulses to come")?;
/// let (onset, end) = (next.onset(), next.transient_end());
/// let span = end.checked_since(onset).ok_or("in order")?.as_seconds_f64();
/// let mut dimmest = 1.0_f64;
/// for k in 1..100 {
///     let offset = Span::from_seconds_f64(span * f64::from(k) / 100.0).ok_or("a span")?;
///     dimmest = dimmest.min(model.luminosity_factor_at(onset.checked_add(offset).ok_or("in range")?));
/// }
/// // Half the light at the dip's minimum, this many pulses in, and the track's after it.
/// assert!((0.5..0.6).contains(&dimmest));
/// assert!((model.luminosity_factor_at(end) - 1.0).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalPulseModel<'a> {
    star: &'a StarModel,
    series: EventSeries,
    clock: ThermalPulseClock<'a>,
    noise: MonotonePhase,
}

impl<'a> ThermalPulseModel<'a> {
    /// The thermal pulses of `star` on `series`, its `star.ev.thermal_pulse` series, or `None` for
    /// a star whose track holds no thermally pulsing AGB with pulses (see "Who pulses" in the
    /// [module documentation](self)).
    ///
    /// # Panics
    ///
    /// In debug builds, if `series` is not of [`tags::STAR_THERMAL_PULSE`]; otherwise only on a
    /// broken invariant: the noise's constants pass its checks for any number of octaves.
    #[must_use]
    pub fn of(star: &'a StarModel, series: &EventSeries) -> Option<Self> {
        debug_assert_eq!(
            series.tag(),
            tags::STAR_THERMAL_PULSE,
            "a star's thermal pulses are its star.ev.thermal_pulse series"
        );
        let clock = ThermalPulseClock::of(star)?;
        let horizon = SourceHorizon::END
            .checked_since(SourceHorizon::START)
            .expect("the source horizon is a span the clock holds");
        let noise = MonotonePhase::new(
            NOISE_AMPLITUDE_CYCLES,
            NOISE_LATTICE_CYCLES,
            octaves_to_span(
                NOISE_LATTICE_CYCLES,
                clock.mean_frequency_hz() * horizon.as_seconds_f64(),
            ),
        )
        .expect("a noise of 0.03 cycles on a one-cycle lattice has a slope bound of 0.31");
        Some(Self {
            star,
            series: *series,
            clock: clock.anchored(anchor_of(&noise, series)),
            noise,
        })
    }

    /// The star.
    #[must_use]
    pub const fn star(&self) -> &'a StarModel {
        self.star
    }

    /// The series.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The pulse clock, anchored for the series.
    #[must_use]
    pub const fn clock(&self) -> ThermalPulseClock<'a> {
        self.clock
    }

    /// The phase's noise.
    #[must_use]
    pub const fn noise(&self) -> MonotonePhase {
        self.noise
    }

    /// Whether the star is on the thermally pulsing AGB at `t`.
    #[must_use]
    pub fn is_pulsing_at(&self, t: UniverseTime) -> bool {
        self.clock.holds(t)
    }

    /// The pulse cycle the star is in at `t` and how far through it, in [0, 1), or `None` off the
    /// thermally pulsing AGB: Φ(t), split.
    #[must_use]
    pub fn cycle_at(&self, t: UniverseTime) -> Option<(i64, f64)> {
        if !self.is_pulsing_at(t) {
            return None;
        }
        let (cycle, fraction) = self.noise.cycle_at(&self.series, &self.clock, t);
        // The anchor puts Φ at or above 0 from the phase's first instant; held there against a
        // rounding below it.
        Some(if cycle < 0 {
            (0, 0.0)
        } else {
            (cycle, fraction)
        })
    }

    /// The pulse number n(t) at `t`, the pulses since the first, ⌊Φ(t)⌋, or `None` off the
    /// thermally pulsing AGB.
    ///
    /// It is 0 from the first pulse to the second, and non-decreasing in t.
    #[must_use]
    pub fn pulse_number_at(&self, t: UniverseTime) -> Option<i64> {
        self.cycle_at(t).map(|(cycle, _)| cycle)
    }

    /// `τ_ip` at `t`, Wagenhuber and Groenewegen's (1998, equation 11) interpulse period, or `None`
    /// off the thermally pulsing AGB.
    ///
    /// It is read at the star's core mass and envelope then, with its first pulse at the core mass
    /// where the phase starts: the reciprocal of the rate the track integrates at its knots.
    #[must_use]
    pub fn interpulse_period_at(&self, t: UniverseTime) -> Option<Years> {
        if !self.is_pulsing_at(t) {
            return None;
        }
        let track = self.clock.track;
        let age = Years::new(
            self.star
                .age_at(t)
                .value()
                .clamp(self.clock.start_age.value(), self.clock.end_age.value()),
        );
        let state = track.state_at(age);
        let first = track.state_at(self.clock.start_age).core_mass();
        let envelope = SolarMasses::new(state.envelope_mass().value().max(0.0));
        Some(sse::interpulse_period(
            state.core_mass(),
            first,
            envelope,
            &ZCoeffs::new(self.star.composition().z_fit()),
        ))
    }

    /// The star's bolometric luminosity at `t` over its track's, as its pulse cycle makes it.
    ///
    /// It is [`cycle_luminosity_factor`] of the cycle's number and fraction on the thermally
    /// pulsing AGB, and 1 off it, continuous in `t` within the phase.
    #[must_use]
    pub fn luminosity_factor_at(&self, t: UniverseTime) -> f64 {
        self.cycle_at(t).map_or(1.0, |(cycle, fraction)| {
            cycle_luminosity_factor(cycle, fraction)
        })
    }

    /// Appends to `out` the thermal pulses in `window`, in time order: those of the part of the
    /// window on the thermally pulsing AGB.
    pub fn pulses_in(&self, window: TimeWindow, out: &mut Vec<ThermalPulse>) {
        let start = window.start().max(self.clock.start());
        let end = window.end().min(self.clock.end());
        // A window that ends before the phase starts, or starts after it ends, holds no pulse.
        let Ok(window) = TimeWindow::new(start, end) else {
            return;
        };
        let mut events = Vec::new();
        self.noise
            .events_in(&self.series, &self.clock, window, &mut events);
        out.extend(events.into_iter().map(|event| self.pulse_of(&event)));
    }

    /// The thermal pulse `id` names, regenerated on its own; `None` if `id` is not of the series
    /// or names a pulse off the thermally pulsing AGB.
    #[must_use]
    pub fn pulse(&self, id: EventId) -> Option<ThermalPulse> {
        self.noise
            .event_by_id(&self.series, &self.clock, id)
            .filter(|event| self.is_pulsing_at(event.time()))
            .map(|event| self.pulse_of(&event))
    }

    /// The pulse whose transient runs at `t`, if one does: the pulse of the cycle at `t`, while its
    /// fraction is under [`TRANSIENT_SHARE`].
    #[must_use]
    pub fn pulse_active_at(&self, t: UniverseTime) -> Option<ThermalPulse> {
        let (cycle, _) = self.cycle_at(t)?;
        self.noise
            .event(&self.series, &self.clock, cycle)
            .filter(|event| self.is_pulsing_at(event.time()))
            .map(|event| self.pulse_of(&event))
            .filter(|pulse| pulse.is_active_at(t))
    }

    /// The pulse of `event`, a cycle of this star's series: its number, onset and the end of its
    /// transient, the first nanosecond at which Φ reaches the cycle plus [`TRANSIENT_SHARE`], held
    /// to the end of the phase. The search's form (its bracket from the onset to the phase's end,
    /// its midpoint and its `>=`) is part of the output, as the clock's inverse is.
    #[must_use]
    fn pulse_of(&self, event: &CycleEvent) -> ThermalPulse {
        let target = Phase::new(event.cycle(), TRANSIENT_SHARE);
        let (mut lo, mut hi) = (to_nanos(event.time()), self.clock.end);
        let transient_end = if self.phase_reaches(hi, target) {
            // Φ is below the target at `lo` and at or above it at `hi`.
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                if self.phase_reaches(mid, target) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            from_nanos_saturating(hi)
        } else {
            self.clock.end()
        };
        ThermalPulse {
            id: event.id(),
            number: event.cycle(),
            onset: event.time(),
            transient_end,
        }
    }

    /// Whether Φ at `t` nanoseconds since the epoch has reached `target`.
    #[must_use]
    fn phase_reaches(&self, t: i128, target: Phase) -> bool {
        self.noise
            .phase_at(&self.series, &self.clock, from_nanos_saturating(t))
            >= target
    }
}

/// One thermal pulse: its ID, its number since the first pulse, its onset and the end of its
/// luminosity transient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThermalPulse {
    id: EventId,
    number: i64,
    onset: UniverseTime,
    transient_end: UniverseTime,
}

impl ThermalPulse {
    /// The pulse's ID, from which [`thermal_pulse`] regenerates it: its cycle is its number.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The pulse's number, 0 for the first pulse of the thermally pulsing AGB.
    #[must_use]
    pub const fn number(&self) -> i64 {
        self.number
    }

    /// The helium-shell flash, the first nanosecond at which the phase reaches the pulse's number.
    #[must_use]
    pub const fn onset(&self) -> UniverseTime {
        self.onset
    }

    /// The first instant after the pulse's rapid peak and slow dip, where the phase reaches its
    /// number plus [`TRANSIENT_SHARE`] (held to the end of the thermally pulsing AGB).
    #[must_use]
    pub const fn transient_end(&self) -> UniverseTime {
        self.transient_end
    }

    /// Whether the pulse's luminosity transient runs at `t`: from its onset, inclusive, to its
    /// transient's end, exclusive.
    #[must_use]
    pub fn is_active_at(&self, t: UniverseTime) -> bool {
        self.onset <= t && t < self.transient_end
    }
}

/// Appends to `out` the thermal pulses of `star` in `window`, in time order: none for a star whose
/// track holds no thermally pulsing AGB ([`ThermalPulseModel::of`]).
///
/// `series` is the star's `star.ev.thermal_pulse` series,
/// `EventSeries::new(seed, tags::STAR_THERMAL_PULSE, body.into())`: [`StarModel`] holds no seed or
/// ID, so it comes in beside the star, as for the flares. Each pulse's time is a function of its
/// number alone, so a listing over a window is the union of the listings over any partition of it,
/// in any order of asking. `out` is only appended to.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_THERMAL_PULSE`].
///
/// # Examples
///
/// A 4 M☉ star's pulses over the whole of its thermally pulsing AGB, numbered from the first, and
/// one regenerated from its ID alone:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::events::{EventSeries, TimeWindow, tags};
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::events::thermal_pulse::{ThermalPulseClock, thermal_pulse, thermal_pulses_in};
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// // Long dead, so that its track holds the whole phase.
/// let star = StarModel::new(SolarMasses::new(4.0), Composition::SOLAR, StarDraws::median(), Years::new(1e9))?;
/// let clock = ThermalPulseClock::of(&star).ok_or("a 4 M☉ star pulses")?;
/// let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// let series = EventSeries::new(Seed::new(7), tags::STAR_THERMAL_PULSE, body.into());
/// let mut pulses = Vec::new();
/// thermal_pulses_in(&star, &series, TimeWindow::new(clock.start(), clock.end())?, &mut pulses);
/// // Some 84 pulses, the first on the phase's first instant.
/// assert!((70..100).contains(&pulses.len()));
/// assert_eq!((pulses[0].number(), pulses[0].onset()), (0, clock.start()));
/// assert_eq!(thermal_pulse(&star, &series, pulses[40].id()).as_ref(), Some(&pulses[40]));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn thermal_pulses_in(
    star: &StarModel,
    series: &EventSeries,
    window: TimeWindow,
    out: &mut Vec<ThermalPulse>,
) {
    if let Some(model) = ThermalPulseModel::of(star, series) {
        model.pulses_in(window, out);
    }
}

/// The thermal pulse `id` names, regenerated on its own: `None` if `id` is not of `series`, names
/// a pulse off the thermally pulsing AGB, or `star` has none.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_THERMAL_PULSE`].
#[must_use]
pub fn thermal_pulse(star: &StarModel, series: &EventSeries, id: EventId) -> Option<ThermalPulse> {
    ThermalPulseModel::of(star, series)?.pulse(id)
}

/// The pulse number of `star` at `t` ([`ThermalPulseModel::pulse_number_at`]), or `None` off the
/// thermally pulsing AGB: P06.T24.b's n.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_THERMAL_PULSE`].
#[must_use]
pub fn pulse_number_at(star: &StarModel, series: &EventSeries, t: UniverseTime) -> Option<i64> {
    ThermalPulseModel::of(star, series)?.pulse_number_at(t)
}

/// The star's luminosity at `t` over its track's as its pulse cycle makes it
/// ([`ThermalPulseModel::luminosity_factor_at`]), and 1 for a star off the thermally pulsing AGB:
/// what P06.T28.g composes into the star's summary.
///
/// # Panics
///
/// In debug builds, if `series` is not of [`tags::STAR_THERMAL_PULSE`].
#[must_use]
pub fn luminosity_factor_at(star: &StarModel, series: &EventSeries, t: UniverseTime) -> f64 {
    ThermalPulseModel::of(star, series).map_or(1.0, |model| model.luminosity_factor_at(t))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::events::testing::{assert_partition_independent, partition};
    use crate::events::{EventBin, event_id};
    use crate::galaxy::imf::MassBand;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::{Galaxy, Population};
    use crate::id::{BodyId, SystemId, event_tags};
    use crate::rng::Seed;
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::testing::{Sampling, reference_position, sample_inputs};
    use crate::time::ClockWindow;
    use crate::units::{Dex, HeliumExcess};

    const SEED: Seed = Seed::new(0x0e7e_0006_0028_000f);
    const YEAR: f64 = SECONDS_PER_JULIAN_YEAR;

    /// [Fe/H] = −1, Z = 0.002.
    fn low_z() -> Composition {
        Composition::from_fe_h(Dex::new(-1.0), HeliumExcess::ZERO)
    }

    /// The ages of the thermally pulsing AGB of a star of `mass` M☉ and `composition` at the
    /// median draws, from its full track.
    fn phase_ages(mass: f64, composition: &Composition) -> (f64, f64) {
        let span = Track::full(SolarMasses::new(mass), composition, &StarDraws::median())
            .pulsing_agb()
            .unwrap();
        (span.start().value(), span.end().value())
    }

    /// A star of `mass` M☉ and `composition` at the median draws whose age at the epoch is its
    /// thermally pulsing AGB's start plus `share` of the phase.
    fn star_in(mass: f64, composition: Composition, share: f64) -> StarModel {
        let (start, end) = phase_ages(mass, &composition);
        StarModel::new(
            SolarMasses::new(mass),
            composition,
            StarDraws::median(),
            Years::new(start + share * (end - start)),
        )
        .unwrap()
    }

    /// A solar star of `mass` M☉ at the median draws, `age` years old at the epoch.
    fn star(mass: f64, age: f64) -> StarModel {
        StarModel::new(
            SolarMasses::new(mass),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(age),
        )
        .unwrap()
    }

    /// Stars of 1, 2 and 4 M☉ of solar composition and 2 M☉ at [Fe/H] = −1, a third of the way
    /// through their thermally pulsing AGB at the epoch.
    fn cases() -> [StarModel; 4] {
        [
            star_in(1.0, Composition::SOLAR, 1.0 / 3.0),
            star_in(2.0, Composition::SOLAR, 1.0 / 3.0),
            star_in(4.0, Composition::SOLAR, 1.0 / 3.0),
            star_in(2.0, low_z(), 1.0 / 3.0),
        ]
    }

    fn body(index: u16) -> BodyId {
        BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), index)
    }

    fn series(index: u16) -> EventSeries {
        EventSeries::new(SEED, tags::STAR_THERMAL_PULSE, body(index).into())
    }

    fn model(s: &StarModel, index: u16) -> ThermalPulseModel<'_> {
        ThermalPulseModel::of(s, &series(index)).unwrap()
    }

    fn after(t: UniverseTime, seconds: f64) -> UniverseTime {
        t.checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    /// The instant `n` nanoseconds from `t`.
    fn shifted(t: UniverseTime, n: i64) -> UniverseTime {
        from_nanos_saturating(to_nanos(t) + i128::from(n))
    }

    /// The clock's base phase at `t`, cycles.
    fn phase(clock: &ThermalPulseClock<'_>, t: UniverseTime) -> f64 {
        clock.base_phase(t).to_f64()
    }

    /// Φ at `t`, the monotone phase of `m`'s series on its clock.
    fn big_phi(m: &ThermalPulseModel<'_>, t: UniverseTime) -> Phase {
        m.noise().phase_at(m.series(), &m.clock(), t)
    }

    /// The whole thermally pulsing AGB of `clock`'s star, and a day either side.
    fn whole(clock: &ThermalPulseClock<'_>) -> TimeWindow {
        TimeWindow::new(
            after(clock.start(), -86_400.0),
            after(clock.end(), 86_400.0),
        )
        .unwrap()
    }

    fn list(m: &ThermalPulseModel<'_>, w: TimeWindow) -> Vec<ThermalPulse> {
        let mut out = Vec::new();
        m.pulses_in(w, &mut out);
        out
    }

    fn relative(a: f64, b: f64) -> f64 {
        ((a - b) / b).abs()
    }

    /// The clock with no anchor is the track's pulse count, read at the age's two parts, bit for
    /// bit inside the phase; 0 at its start and the count at its end, to a nanosecond's rounding;
    /// non-decreasing; and linear at the mean rate outside it, with the start's step before it.
    #[test]
    fn the_clock_counts_the_tracks_pulses() {
        for s in cases() {
            let clock = ThermalPulseClock::of(&s).unwrap();
            let track = s.track().unwrap();
            let span = track.pulsing_agb().unwrap();
            assert_same_bits(clock.start_age().value(), span.start().value());
            assert_same_bits(clock.end_age().value(), span.end().value());
            assert_same_bits(clock.anchor_cycles(), 0.0);
            assert_same_bits(
                clock.pulses_at_end(),
                track.pulse_phase_at(span.end().value(), 0.0).unwrap(),
            );
            let length = clock
                .end()
                .checked_since(clock.start())
                .unwrap()
                .as_seconds_f64();
            assert!(relative(clock.mean_frequency_hz() * length, clock.pulses_at_end()) < 1e-12);
            let a0 = s.age_at_epoch().value();
            let mut last = f64::NEG_INFINITY;
            for i in 0..=1_000 {
                let t = if i == 1_000 {
                    shifted(clock.end(), -1)
                } else {
                    after(clock.start(), length * f64::from(i) / 1_000.0)
                };
                let years = t.since_epoch().as_julian_years_f64();
                let phi = phase(&clock, t);
                if let Some(count) = track.pulse_phase_at(a0, years) {
                    assert_same_bits(phi, count.clamp(0.0, clock.pulses_at_end()));
                }
                assert!(phi >= last, "{phi} after {last} at {t}");
                last = phi;
            }
            assert!((0.0..1e-12).contains(&phase(&clock, clock.start())));
            assert!(relative(last, clock.pulses_at_end()) < 1e-9);
            // Outside: linear at the mean rate, from 0 less the start's step before and from the
            // end's count after (to the split phase's 10⁻¹⁶ of a cycle).
            let day = 86_400.0;
            let rate = clock.mean_frequency_hz();
            let before = phase(&clock, after(clock.start(), -day));
            assert!(
                (before + rate * day + START_STEP_CYCLES).abs() < 1e-15,
                "{before}"
            );
            assert!(phase(&clock, shifted(clock.start(), -1)) < 0.0);
            let past = phase(&clock, after(clock.end(), day)) - clock.pulses_at_end();
            assert!((past - rate * day).abs() < 1e-12, "{past}");
            assert!(
                (phase(&clock, clock.end()) - phase(&clock, shifted(clock.end(), -1))).abs() < 1e-9
            );
            // The anchored clock is the same count plus its anchor.
            let anchored = clock.anchored(0.25);
            let t = after(clock.start(), length / 2.0);
            assert!((phase(&anchored, t) - phase(&clock, t) - 0.25).abs() < 1e-12);
        }
    }

    /// The clock's rate is 1 ÷ `τ_ip`, Wagenhuber and Groenewegen's interpulse period at the
    /// state's core mass and envelope, to the knots' interpolation (10%: 8.2% at [Fe/H] = −1, where
    /// the first pulses' turn-on bends the rate between the sparsest knots), at nine instants of
    /// each phase; and within ±H the clock is the plan's first-order form, its count at the epoch
    /// plus the time since times its rate at the epoch, to 10⁻³ cycles.
    #[test]
    fn the_rate_is_the_interpulse_periods_and_the_clock_is_first_order_within_the_window() {
        for s in cases() {
            let m = model(&s, 0);
            let clock = ThermalPulseClock::of(&s).unwrap();
            let length = clock
                .end()
                .checked_since(clock.start())
                .unwrap()
                .as_seconds_f64();
            let mut worst = 0.0_f64;
            for i in 1..10 {
                let t = after(clock.start(), length * f64::from(i) / 10.0);
                let rate =
                    (phase(&clock, after(t, YEAR)) - phase(&clock, after(t, -YEAR))) / (2.0 * YEAR);
                let period = m.interpulse_period_at(t).unwrap().value() * YEAR;
                worst = worst.max(relative(rate * period, 1.0));
            }
            let epoch = UniverseTime::EPOCH;
            let rate = (phase(&clock, after(epoch, YEAR)) - phase(&clock, after(epoch, -YEAR)))
                / (2.0 * YEAR);
            let mut first_order = 0.0_f64;
            for t in [ClockWindow::START, ClockWindow::END] {
                let seconds = t.since_epoch().as_seconds_f64();
                first_order = first_order
                    .max((phase(&clock, t) - (phase(&clock, epoch) + rate * seconds)).abs());
            }
            eprintln!(
                "{:?} M☉ {:?}: rate against 1 ÷ τ_ip within {worst:.2e}; first-order form within \
                 {first_order:.2e} cycles over ±H; {:.2} pulses",
                s.initial_mass(),
                s.composition().z_fit(),
                clock.pulses_at_end()
            );
            assert!(worst < 0.1, "{worst}");
            assert!(first_order < 1e-3, "{first_order}");
        }
    }

    /// The inverse returns the first nanosecond at which the anchored count reaches a value,
    /// inside the phase and on its linear continuations, anchored or not, and `time_at` that of a
    /// split phase's value.
    #[test]
    fn the_inverse_finds_the_first_instant_of_a_phase() {
        for s in cases() {
            let m = model(&s, 0);
            for clock in [ThermalPulseClock::of(&s).unwrap(), m.clock()] {
                let (n, c) = (clock.pulses_at_end(), clock.anchor_cycles());
                let anchored = |t: UniverseTime| clock.count_at_nanos(to_nanos(t)) + c;
                for count in [0.0, 0.37, 1.0, n / 2.0, n.floor(), n - 1e-3]
                    .into_iter()
                    .filter(|&count| count < n)
                {
                    let target = count + c;
                    let t = clock.time_of_phase(target);
                    assert!(anchored(t) >= target, "{target}");
                    assert!(anchored(shifted(t, -1)) < target, "{target}");
                    assert_eq!(
                        clock.time_at(Phase::new(0, target)),
                        clock.time_of_phase(Phase::new(0, target).to_f64())
                    );
                }
                for (count, side) in [(-2.5, clock.start()), (n + 2.5, clock.end())] {
                    let t = clock.time_at(Phase::new(0, count + c));
                    let seconds = t.checked_since(side).unwrap().as_seconds_f64();
                    let expected = if count < 0.0 {
                        (count + START_STEP_CYCLES) / clock.mean_frequency_hz()
                    } else {
                        (count - n) / clock.mean_frequency_hz()
                    };
                    assert!(
                        (seconds - expected).abs() < 1e-6 * expected.abs(),
                        "{count}"
                    );
                }
            }
        }
    }

    /// The anchor puts pulse 0 on the phase's first instant, Φ just above 0 there and below it a
    /// nanosecond before, for 200 series of each star and for 10 series each of the 2 M☉ star at
    /// 40 ages through its phase (whose first instants read counts of 0 or a few units in the last
    /// place above it); the anchor lies within the noise's bound.
    #[test]
    fn the_anchor_puts_the_first_pulse_on_the_phases_first_instant() {
        let check = |m: &ThermalPulseModel<'_>, what: &str| {
            let start = m.clock().start();
            let first = m.noise().event(m.series(), &m.clock(), 0).unwrap();
            assert_eq!(first.time(), start, "{what}");
            let at = big_phi(m, start);
            assert_eq!(at.cycles(), 0, "{what}");
            assert!(at.fraction() < 1e-11, "{what}: {}", at.fraction());
            assert!(big_phi(m, shifted(start, -1)).cycles() < 0, "{what}");
            assert!(m.clock().anchor_cycles().abs() <= m.noise().noise_bound_cycles() + 1e-9);
        };
        for s in cases() {
            for index in 0..200 {
                check(&model(&s, index), &format!("series {index}"));
            }
            let m = model(&s, 0);
            assert_same_bits(m.noise().amplitude_cycles(), NOISE_AMPLITUDE_CYCLES);
            assert_eq!(m.noise().lattice_cycles(), NOISE_LATTICE_CYCLES);
        }
        for k in 0..40 {
            let s = star_in(2.0, Composition::SOLAR, (f64::from(k) + 0.37) / 40.0);
            for index in 0..10 {
                check(&model(&s, index), &format!("age {k}, series {index}"));
            }
        }
    }

    /// Over the whole phase the pulses are 0, 1, …, the first on the phase's first instant, each at
    /// the first nanosecond at which Φ reaches its number, each regenerated from its ID; the pulse
    /// number between two pulses is the earlier's; the transient ends where Φ reaches the number
    /// plus 0.31, or at the end of the phase; and the count stays within the noise's bound of the
    /// track's.
    #[test]
    fn a_pulse_falls_where_the_phase_reaches_each_whole_number() {
        for s in cases() {
            let m = model(&s, 1);
            let clock = m.clock();
            let sr = series(1);
            let pulses = list(&m, whole(&clock));
            let last = pulses.last().unwrap().number();
            assert_eq!(
                pulses.iter().map(ThermalPulse::number).collect::<Vec<_>>(),
                (0..=last).collect::<Vec<_>>()
            );
            assert_eq!(pulses[0].onset(), clock.start());
            #[expect(clippy::cast_precision_loss, reason = "a few hundred pulses")]
            let wander = last as f64 - clock.pulses_at_end();
            assert!(
                wander.abs() <= m.noise().noise_bound_cycles() + 1.0,
                "{wander}"
            );
            for p in &pulses {
                let number = Phase::whole(p.number());
                assert!(big_phi(&m, p.onset()) >= number);
                assert!(big_phi(&m, shifted(p.onset(), -1)) < number);
                assert!(m.is_pulsing_at(p.onset()));
                assert_eq!(m.pulse(p.id()).as_ref(), Some(p));
                assert_eq!(thermal_pulse(&s, &sr, p.id()).as_ref(), Some(p));
                assert_eq!(m.pulse_number_at(p.onset()), Some(p.number()));
                // The first pulse is the phase's first instant, before which there is no number.
                assert_eq!(
                    m.pulse_number_at(shifted(p.onset(), -1)),
                    (p.number() > 0).then(|| p.number() - 1)
                );
                let transient = big_phi(&m, p.transient_end());
                assert!(
                    p.transient_end() == clock.end()
                        || transient >= Phase::new(p.number(), TRANSIENT_SHARE)
                );
                assert!(p.transient_end() > p.onset());
            }
            // A cycle off the phase names no pulse, and nor do other series' IDs.
            let off = sr.event_id(EventBin::new(last + 5).unwrap(), 0);
            assert_eq!(m.pulse(off), None);
            let word = pulses[0].id().word();
            for foreign in [
                event_id(body(9).into(), tags::STAR_THERMAL_PULSE, word.bin(), 0),
                event_id(body(1).into(), event_tags::SELF_TEST, word.bin(), 0),
                sr.event_id(word.bin(), 1),
            ] {
                assert_eq!(m.pulse(foreign), None);
            }
        }
    }

    /// The pulses of different series wander apart: over 40 series of the 4 M☉ star the onsets of
    /// pulse 40 spread over a good share of a cycle, and every series' first pulse is the phase's
    /// first instant.
    #[test]
    fn the_series_wander_apart() {
        let s = star_in(4.0, Composition::SOLAR, 1.0 / 3.0);
        let clock = ThermalPulseClock::of(&s).unwrap();
        let mut onsets = Vec::new();
        for index in 0..40 {
            let m = model(&s, index);
            let pulses = list(&m, whole(&clock));
            assert_eq!(pulses[0].onset(), clock.start());
            onsets.push(pulses[40].onset());
        }
        onsets.sort_unstable();
        let spread = onsets[39]
            .checked_since(onsets[0])
            .unwrap()
            .as_seconds_f64();
        let period = 1.0 / clock.mean_frequency_hz();
        eprintln!(
            "pulse 40 of 40 series spreads over {:.3} cycles",
            spread / period
        );
        assert!(spread > 0.05 * period, "{spread} s against {period} s");
    }

    /// Who pulses: a star whose track holds the phase, alive in it or long dead; not the Sun at
    /// 4.57 Gyr, whose track is built only to +H, a 20 or a 30 M☉ star, which never reach it, or a
    /// brown dwarf.
    #[test]
    fn only_a_star_whose_track_holds_the_phase_pulses() {
        assert!(ThermalPulseModel::of(&star(1.0, 1.3e10), &series(0)).is_some());
        let window = TimeWindow::new(crate::time::SourceHorizon::START, ClockWindow::END).unwrap();
        for s in [
            star(1.0, 4.57e9),
            star(20.0, 2e7),
            star(30.0, 2e7),
            star(0.05, 1e9),
        ] {
            assert!(
                ThermalPulseModel::of(&s, &series(0)).is_none(),
                "{:?}",
                s.initial_mass()
            );
            let mut out = Vec::new();
            thermal_pulses_in(&s, &series(0), window, &mut out);
            assert_eq!(out, []);
            assert_eq!(pulse_number_at(&s, &series(0), UniverseTime::EPOCH), None);
            assert_same_bits(
                luminosity_factor_at(&s, &series(0), UniverseTime::EPOCH),
                1.0,
            );
        }
        // Off the phase: no cycle, no period, a factor of 1.
        let s = star_in(2.0, Composition::SOLAR, 1.0 / 3.0);
        let m = model(&s, 0);
        let before = shifted(m.clock().start(), -1);
        assert_eq!(m.cycle_at(before), None);
        assert_eq!(m.interpulse_period_at(m.clock().end()), None);
        assert_same_bits(m.luminosity_factor_at(before), 1.0);
    }

    /// The cycle's light: 1 at its start and over its quiescent part, the rapid peak's 10^0.1 at
    /// its middle and the dip's 10^−0.3 at its minimum at full strength, continuous to 10⁻⁵ over
    /// 10⁻⁷ of a cycle, rising and falling where its pieces do, and 0.920 of the track's light
    /// over a full-strength cycle; the first pulses weaker, by 1 − e^(−(n + 1) ÷ 3) in dex. On the
    /// phase the star's factor is the profile at its cycle, continuous across a pulse.
    #[test]
    fn the_cycle_profile_dips_after_its_peak_and_returns_to_the_tracks_light() {
        let full = |f: f64| cycle_luminosity_factor(1_000, f);
        assert_same_bits(full(0.0), 1.0);
        assert_same_bits(full(TRANSIENT_SHARE), 1.0);
        assert_same_bits(full(0.9), 1.0);
        assert!(relative(full(RAPID_PEAK_SHARE / 2.0), math::exp10(RAPID_PEAK_DEX)) < 1e-15);
        let minimum = RAPID_PEAK_SHARE + DIP_MINIMUM * DIP_SHARE;
        assert!(relative(full(minimum), math::exp10(-DIP_DEX)) < 1e-15);
        assert_same_bits(full(1.25), full(0.25));
        for (n, strength) in [
            (-1, 0.0),
            (0, 0.283_468_689_426_018_6),
            (9, 0.964_326_006_652_748),
        ] {
            assert!((pulse_strength(n) - strength).abs() < 1e-12, "{n}");
        }
        assert!(
            relative(
                cycle_luminosity_factor(0, minimum),
                math::exp10(-DIP_DEX * pulse_strength(0))
            ) < 1e-15
        );
        let steps = 1_000_000_u32;
        let mut sum = 0.0;
        let (mut last, mut last_f) = (full(0.0), 0.0);
        // The pieces over which the factor rises and over which it falls.
        let rising = |f: f64| f < RAPID_PEAK_SHARE / 2.0 || (minimum..TRANSIENT_SHARE).contains(&f);
        let falling = |f: f64| (RAPID_PEAK_SHARE / 2.0..minimum).contains(&f);
        for i in 1..=steps {
            let f = f64::from(i) / f64::from(steps);
            let factor = full(f);
            assert!((factor - last).abs() < 1e-3, "a jump at {f}");
            sum += factor;
            if rising(f) && rising(last_f) {
                assert!(factor >= last, "{f}");
            }
            if falling(f) && falling(last_f) {
                assert!(factor <= last, "{f}");
            }
            (last, last_f) = (factor, f);
        }
        let mean = sum / f64::from(steps);
        eprintln!("a full-strength cycle's mean factor {mean:.4}");
        assert!((0.915..0.925).contains(&mean), "{mean}");
        for i in 0..1_000 {
            let f = f64::from(i) / 1_000.0;
            let step = (full(f + 1e-7) - full(f)).abs();
            assert!(step < 1e-5, "{step} at {f}");
        }
        let s = star_in(2.0, Composition::SOLAR, 1.0 / 3.0);
        let m = model(&s, 3);
        let sr = series(3);
        for p in list(&m, whole(&m.clock())).iter().skip(1).take(5) {
            let t = after(p.onset(), 1e3 * YEAR);
            let (cycle, fraction) = m.cycle_at(t).unwrap();
            assert_eq!(cycle, p.number());
            assert_same_bits(
                m.luminosity_factor_at(t),
                cycle_luminosity_factor(cycle, fraction),
            );
            let step = (m.luminosity_factor_at(p.onset())
                - m.luminosity_factor_at(shifted(p.onset(), -1)))
            .abs();
            assert!(step < 1e-6, "{step}");
            assert_same_bits(
                luminosity_factor_at(&s, &sr, p.onset()),
                m.luminosity_factor_at(p.onset()),
            );
        }
    }

    /// `pulse_active_at` gives the listed pulse whose transient runs at t, and nothing between
    /// transients or off the phase.
    #[test]
    fn the_active_pulse_is_the_listed_one_whose_transient_runs() {
        for s in cases() {
            let m = model(&s, 4);
            let w = whole(&m.clock());
            let listed = list(&m, w);
            let span = w.end().checked_since(w.start()).unwrap().as_seconds_f64();
            let mut seen = 0;
            for i in 0..2_000 {
                let t = after(w.start(), span * (f64::from(i) + 0.5) / 2_000.0);
                let expected = listed.iter().copied().find(|p| p.is_active_at(t));
                assert_eq!(m.pulse_active_at(t), expected, "{t}");
                seen += usize::from(expected.is_some());
            }
            assert!(seen > 400, "{seen}");
        }
    }

    /// The window and cut sets of the partition tests, on the 4 M☉ star.
    fn partition_case(
        clock: &ThermalPulseClock<'_>,
        listing: impl Fn(TimeWindow) -> Vec<ThermalPulse>,
    ) -> (TimeWindow, Vec<Vec<UniverseTime>>) {
        let whole = whole(clock);
        let pulses = listing(whole);
        assert!(pulses.len() > 10, "{} pulses", pulses.len());
        let cuts = vec![
            vec![
                clock.start(),
                shifted(clock.start(), 1),
                shifted(clock.end(), -1),
                clock.end(),
            ],
            vec![
                pulses[3].onset(),
                pulses[3].onset(),
                shifted(pulses[4].onset(), 1),
            ],
            vec![shifted(pulses[5].onset(), -1), pulses[7].transient_end()],
            pulses
                .iter()
                .skip(1)
                .step_by(3)
                .map(|p| after(p.onset(), 0.5))
                .collect(),
        ];
        (whole, cuts)
    }

    /// The brainstorm's time test: a listing over a window is the union of the listings over any
    /// partition of it, in any order of asking, and the free function lists the same.
    #[test]
    fn listings_are_the_same_over_any_partition_in_any_order() {
        let s = star_in(4.0, Composition::SOLAR, 1.0 / 3.0);
        let m = model(&s, 2);
        let listing = |w: TimeWindow| list(&m, w);
        let (whole, cut_sets) = partition_case(&m.clock(), listing);
        for cuts in &cut_sets {
            assert_partition_independent(whole, cuts, listing);
            assert_order_independent(&partition(whole, cuts), |w| listing(*w));
        }
        let mut free = Vec::new();
        thermal_pulses_in(&s, &series(2), whole, &mut free);
        assert_eq!(free, listing(whole));
    }

    /// The pieces listed on threads of their own give the same pulses. Not on wasm32-wasip1, which
    /// has no threads; the other CI architectures run it.
    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn listings_are_the_same_on_other_threads() {
        let s = star_in(4.0, Composition::SOLAR, 1.0 / 3.0);
        let m = model(&s, 2);
        let listing = |w: TimeWindow| list(&m, w);
        let (whole, cut_sets) = partition_case(&m.clock(), listing);
        let pulses = listing(whole);
        for cuts in &cut_sets {
            let pieces = partition(whole, cuts);
            let threaded: Vec<Vec<ThermalPulse>> = std::thread::scope(|scope| {
                let handles: Vec<_> = pieces
                    .iter()
                    .map(|&w| scope.spawn(move || listing(w)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert_eq!(threaded.concat(), pulses);
        }
    }

    /// The parent's no-history test for this kind: the pulse number, the factor and the active
    /// pulse at +900 years, asked cold, are the same after asking about 100 earlier times.
    #[test]
    fn the_pulse_at_900_years_needs_no_history() {
        let t = after(UniverseTime::EPOCH, 900.0 * YEAR);
        let sr = series(6);
        let ask = |s: &StarModel| {
            let m = ThermalPulseModel::of(s, &sr).unwrap();
            (
                pulse_number_at(s, &sr, t),
                m.pulse_active_at(t),
                luminosity_factor_at(s, &sr, t),
            )
        };
        let cold = ask(&star_in(4.0, Composition::SOLAR, 0.5));
        let s = star_in(4.0, Composition::SOLAR, 0.5);
        let m = model(&s, 6);
        let mut lcg = Lcg::new(0x0928_f900);
        for _ in 0..100 {
            let earlier = after(ClockWindow::START, lcg.next_f64() * 1.899e3 * YEAR);
            assert!(earlier < t);
            let _ = m.luminosity_factor_at(earlier);
            let _ = list(
                &m,
                TimeWindow::new(earlier, after(earlier, 50.0 * YEAR)).unwrap(),
            );
        }
        let warm = ask(&s);
        assert_eq!((warm.0, warm.1), (cold.0, cold.1));
        assert_same_bits(warm.2, cold.2);
        assert!(cold.0.is_some());
    }

    /// Pinned: four stars' pulse clocks (their phases' ages and clock times, pulses, mean rate, the
    /// anchor and the noise's octaves; the clock, Φ, the interpulse period and the factor at five
    /// instants) and their first 30 pulses' IDs, onsets and transients' ends.
    #[test]
    fn thermal_pulses_are_pinned() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let names = [
            "1 M☉, solar, a third of the way through its thermally pulsing AGB",
            "2 M☉, solar, a third of the way through",
            "4 M☉, solar, a third of the way through",
            "2 M☉, [Fe/H] = −1, a third of the way through",
        ];
        for (name, s) in names.iter().zip(cases()) {
            let m = model(&s, 0);
            let clock = m.clock();
            w.line("");
            w.line(&format!("# {name}"));
            w.f64("start age (yr)", clock.start_age().value());
            w.f64("end age (yr)", clock.end_age().value());
            w.line(&format!("from {} to {}", clock.start(), clock.end()));
            w.f64("pulses at the end", clock.pulses_at_end());
            w.f64("mean frequency (per s)", clock.mean_frequency_hz());
            w.f64("anchor (cycles)", clock.anchor_cycles());
            w.line(&format!("noise octaves {}", m.noise().octaves()));
            let middle = after(
                clock.start(),
                clock
                    .end()
                    .checked_since(clock.start())
                    .unwrap()
                    .as_seconds_f64()
                    / 2.0,
            );
            for (label, t) in [
                ("−H", ClockWindow::START),
                ("the epoch", UniverseTime::EPOCH),
                ("+H", ClockWindow::END),
                ("the middle of the phase", middle),
                ("a day before the end", after(clock.end(), -86_400.0)),
            ] {
                w.f64(&format!("clock at {label}"), phase(&clock, t));
                w.f64(&format!("phase at {label}"), big_phi(&m, t).to_f64());
                w.f64(
                    &format!("interpulse period at {label} (yr)"),
                    m.interpulse_period_at(t).unwrap().value(),
                );
                w.f64(&format!("factor at {label}"), m.luminosity_factor_at(t));
            }
            for p in list(&m, whole(&clock)).iter().take(30) {
                w.line(&format!(
                    "{} pulse {} at {}, transient to {}",
                    p.id(),
                    p.number(),
                    p.onset(),
                    p.transient_end()
                ));
            }
        }
        golden!("stellar/thermal_pulses", w.as_str());
    }

    /// The parent's statistical test for this kind, for a phase: 10⁴ stars of T31's sampler (bands
    /// C and D of the old thin disc at Milky Way values, at their own masses, compositions and
    /// draws) that reach the thermally pulsing AGB, each built at an age inside it from a
    /// test-local LCG, list over the source horizon exactly the pulses whose numbers the phase
    /// crosses there, ⌊Φ(b⁻)⌋ − ⌊Φ(a⁻)⌋ on the part of the horizon in the phase, and in all the
    /// clock's own integral, Σ (φ(b) − φ(a)), to within twice the noise's bound and one pulse a
    /// star.
    #[test]
    #[ignore = "slow: 10⁴ tracks of 0.75–8 M☉ to their deaths"]
    fn the_pulse_count_over_10_000_stars_matches_the_clocks_integral() {
        const STARS: u32 = 10_000;
        let seed = 0x0628_f000_0000_5eed_u64;
        let galaxy = Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like()).unwrap();
        let component = galaxy
            .fields()
            .components()
            .iter()
            .position(|c| c.population() == Population::OldThinDisc)
            .unwrap();
        let horizon = TimeWindow::new(crate::time::SourceHorizon::START, ClockWindow::END).unwrap();
        let mut lcg = Lcg::new(0x0628_f016);
        let (mut stars, mut drawn, mut counted) = (0_u32, 0_u64, 0_u64);
        let (mut integral, mut most, mut allowed, mut deviation) = (0.0, 0.0_f64, 0.0, 0.0);
        while stars < STARS {
            let band = if drawn % 2 == 0 {
                MassBand::C
            } else {
                MassBand::D
            };
            let sampling = Sampling {
                component,
                position: reference_position(Population::OldThinDisc),
                band: Some(band),
            };
            let inputs = sample_inputs(&galaxy, &sampling, seed, drawn);
            drawn += 1;
            let Some(span) =
                Track::full(inputs.initial_mass, &inputs.composition, &inputs.draws).pulsing_agb()
            else {
                continue;
            };
            let (first, last) = (span.start().value(), span.end().value());
            let age = first + lcg.next_f64() * (last - first);
            let s = StarModel::new(
                inputs.initial_mass,
                inputs.composition,
                inputs.draws,
                Years::new(age),
            )
            .unwrap();
            let sr = EventSeries::new(
                Seed::new(seed),
                tags::STAR_THERMAL_PULSE,
                inputs.body.into(),
            );
            let Some(m) = ThermalPulseModel::of(&s, &sr) else {
                continue;
            };
            let clock = m.clock();
            let a = horizon.start().max(clock.start());
            let b = horizon.end().min(clock.end());
            let count = u64::try_from(list(&m, horizon).len()).unwrap();
            if a < b {
                let crossed =
                    big_phi(&m, shifted(b, -1)).cycles() - big_phi(&m, shifted(a, -1)).cycles();
                assert_eq!(i64::try_from(count).unwrap(), crossed);
                let bare = ThermalPulseClock::of(&s).unwrap();
                let span = phase(&bare, b) - phase(&bare, a);
                #[expect(clippy::cast_precision_loss, reason = "a few thousand pulses")]
                let listed = count as f64;
                integral += span;
                deviation += listed - span;
                allowed += 2.0 * m.noise().noise_bound_cycles() + 1.0;
                most = most.max(span);
            } else {
                assert_eq!(count, 0);
            }
            counted += count;
            stars += 1;
        }
        #[expect(clippy::cast_precision_loss, reason = "counts of pulses")]
        let listed = counted as f64;
        eprintln!(
            "{STARS} stars on the thermally pulsing AGB of {drawn} drawn: {counted} pulses over the \
             source horizon against the clock's {integral:.1}, at most {most:.1} in one star; a mean \
             deviation of {:.3} a star, within the summed bound {allowed:.1}",
            deviation / f64::from(STARS)
        );
        assert!(
            (listed - integral).abs() <= allowed,
            "{listed} against {integral}"
        );
        assert!(integral > f64::from(STARS));
    }
}
