//! The scene clock: a universe time that runs on the server's monotonic clock at a rate of 0 or a
//! power of ten (rendering plan R03, Design note 2).
//!
//! A [`SceneClock`] is an anchor, a scene time read at an [`Instant`], and a [`TimeRate`]. Any
//! later instant maps to the anchor's time plus the elapsed real time times the rate, exactly, in
//! whole nanoseconds; [`SceneClock::instant_of`] maps a scene time back to the first instant at
//! which the clock reads it, for a deadline. The clock stops at the clock window's end,
//! [`ClockWindow::END`], in state [`ClockState::WindowLimit`], and holds there. Rates are the
//! single-player brainstorm's (The clock): pause as a rate of zero, then 1× to 100,000× in powers
//! of ten.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use hyperion_protocol::{SceneClockDto, SceneClockStateDto};
use hyperion_sim::time::{ClockWindow, NANOS_PER_SECOND, Span, UniverseTime};
use tokio::time::Instant;

use super::Clock;

/// Nanoseconds in a second, as the wide integer the clock's arithmetic is done in. The `as` widens
/// a `u32` to a `u128`, which is lossless; `From` is not usable in a constant.
const NANOS: u128 = NANOS_PER_SECOND as u128;

/// The fastest rate: 100,000 scene seconds a real second (the single-player brainstorm's The
/// clock).
pub(crate) const MAX_TIME_RATE: u32 = 100_000;

/// Scene seconds per real second: 0 (paused) or a power of ten from 1 to [`MAX_TIME_RATE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct TimeRate(u32);

impl TimeRate {
    /// The paused clock's rate.
    pub(crate) const PAUSED: Self = Self(0);

    /// The rate `rate`.
    ///
    /// # Errors
    ///
    /// [`BuildTimeRateError`] unless `rate` is 0 or a power of ten from 1 to [`MAX_TIME_RATE`].
    pub(crate) fn new(rate: u32) -> Result<Self, BuildTimeRateError> {
        let mut allowed = 1;
        while allowed <= MAX_TIME_RATE {
            if rate == allowed {
                return Ok(Self(rate));
            }
            allowed *= 10;
        }
        if rate == 0 {
            Ok(Self::PAUSED)
        } else {
            Err(BuildTimeRateError { rate })
        }
    }

    /// Scene seconds per real second.
    #[must_use]
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

/// A rate that is neither 0 nor a power of ten from 1 to [`MAX_TIME_RATE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BuildTimeRateError {
    rate: u32,
}

impl fmt::Display for BuildTimeRateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a time rate of {} is not 0 or a power of ten from 1 to {MAX_TIME_RATE}",
            self.rate
        )
    }
}

impl Error for BuildTimeRateError {}

/// Whether a scene clock runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ClockState {
    /// It advances at its rate.
    Running,
    /// Its rate is zero.
    Paused,
    /// It has reached the clock window's end and holds there.
    WindowLimit,
}

/// What a scene clock reads at an instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ClockReading {
    time: UniverseTime,
    rate: TimeRate,
    state: ClockState,
}

impl ClockReading {
    /// The scene time, inside the clock window.
    #[must_use]
    pub(crate) const fn time(self) -> UniverseTime {
        self.time
    }

    /// The rate the clock runs at.
    #[must_use]
    pub(crate) const fn rate(self) -> TimeRate {
        self.rate
    }

    /// Whether the clock runs: worked out from the rate and the time, never stored.
    #[must_use]
    pub(crate) const fn state(self) -> ClockState {
        self.state
    }
}

impl From<ClockReading> for SceneClockDto {
    fn from(reading: ClockReading) -> Self {
        Self {
            time: hyperion_protocol::UniverseTime {
                seconds: reading.time().seconds(),
                nanos: reading.time().subsec_nanos(),
            },
            time_rate: reading.rate.get(),
            state: match reading.state() {
                ClockState::Running => SceneClockStateDto::Running,
                ClockState::Paused => SceneClockStateDto::Paused,
                ClockState::WindowLimit => SceneClockStateDto::WindowLimit,
            },
        }
    }
}

/// A scene clock: `time` at the instant `anchor`, running at `rate` from there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SceneClock {
    time: UniverseTime,
    anchor: Instant,
    rate: TimeRate,
}

impl SceneClock {
    /// A clock reading `time` at `anchor` and running at `rate`.
    ///
    /// # Errors
    ///
    /// [`BuildSceneClockError`] if `time` lies outside the [`ClockWindow`].
    pub(crate) fn new(
        time: UniverseTime,
        anchor: Instant,
        rate: TimeRate,
    ) -> Result<Self, BuildSceneClockError> {
        if !ClockWindow::contains(time) {
            return Err(BuildSceneClockError { time });
        }
        Ok(Self { time, anchor, rate })
    }

    /// What the clock reads now, on the runtime's clock (paused in tests).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn now(&self) -> ClockReading {
        self.at_instant(Instant::now())
    }

    /// What the clock reads at `at`: the anchor's time plus the real time elapsed since the anchor
    /// times the rate, to the nanosecond, held at the window's end. An instant before the anchor
    /// reads the anchor's time.
    #[must_use]
    pub(crate) fn at_instant(&self, at: Instant) -> ClockReading {
        let rate = self.rate.get();
        if rate == 0 {
            return self.reading(self.time);
        }
        let scene_nanos = at.saturating_duration_since(self.anchor).as_nanos() * u128::from(rate);
        let time = span_of_nanos(scene_nanos)
            .and_then(|elapsed| self.time.checked_add(elapsed))
            .filter(|&time| time <= ClockWindow::END)
            .unwrap_or(ClockWindow::END);
        self.reading(time)
    }

    /// The first instant at which the clock reads `time` or later: the deadline for something due
    /// at that scene time. `None` if the clock never reads it: paused, or `time` beyond the
    /// window's end. A time the clock has already passed at its anchor is due at the anchor.
    #[must_use]
    pub(crate) fn instant_of(&self, time: UniverseTime) -> Option<Instant> {
        let rate = u128::from(self.rate.get());
        if rate == 0 || time > ClockWindow::END {
            return None;
        }
        if time <= self.time {
            return Some(self.anchor);
        }
        let ahead = time
            .checked_since(self.time)
            .expect("two times inside the clock window are 2,000 years apart at most");
        let scene_nanos = u128::try_from(ahead.seconds())
            .expect("the time is later than the anchor's, so the span is positive")
            * NANOS
            + u128::from(ahead.subsec_nanos());
        // Rounded up, so that the clock reads at least `time` at the instant returned.
        let real_nanos = scene_nanos.div_ceil(rate);
        let real = Duration::new(
            u64::try_from(real_nanos / NANOS)
                .expect("2,000 years of seconds fit a u64 many times over"),
            u32::try_from(real_nanos % NANOS).expect("a remainder of 10⁹ fits a u32"),
        );
        self.anchor.checked_add(real)
    }

    /// The reading of `time`, with the state it implies.
    #[must_use]
    fn reading(&self, time: UniverseTime) -> ClockReading {
        let state = if self.rate == TimeRate::PAUSED {
            ClockState::Paused
        } else if time >= ClockWindow::END {
            ClockState::WindowLimit
        } else {
            ClockState::Running
        };
        ClockReading {
            time,
            rate: self.rate,
            state,
        }
    }
}

impl Clock for SceneClock {
    fn reading_at(&self, at: Instant) -> ClockReading {
        self.at_instant(at)
    }

    fn instant_of(&self, time: UniverseTime) -> Option<Instant> {
        SceneClock::instant_of(self, time)
    }
}

/// A span of `nanos` nanoseconds, or `None` if it does not fit a [`Span`].
#[must_use]
fn span_of_nanos(nanos: u128) -> Option<Span> {
    let seconds = i64::try_from(nanos / NANOS).ok()?;
    let subsec = u32::try_from(nanos % NANOS).expect("a remainder of 10⁹ fits a u32");
    Span::new(seconds, subsec).ok()
}

/// A scene time outside the clock window, ±1,000 Julian years about the epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BuildSceneClockError {
    time: UniverseTime,
}

impl fmt::Display for BuildSceneClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} lies outside the clock window of 1,000 Julian years either side of the epoch",
            self.time
        )
    }
}

impl Error for BuildSceneClockError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate(rate: u32) -> TimeRate {
        TimeRate::new(rate).unwrap()
    }

    fn time(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    #[test]
    fn rates_are_zero_or_powers_of_ten_to_a_hundred_thousand() {
        for good in [0, 1, 10, 100, 1_000, 10_000, 100_000] {
            assert_eq!(TimeRate::new(good).map(TimeRate::get), Ok(good));
        }
        for bad in [2, 5, 11, 99, 1_000_000, 200_000, u32::MAX] {
            assert_eq!(TimeRate::new(bad), Err(BuildTimeRateError { rate: bad }));
        }
    }

    #[test]
    fn a_time_outside_the_window_is_refused() {
        let past_end = ClockWindow::END
            .checked_add(Span::new(0, 1).unwrap())
            .unwrap();
        assert_eq!(
            SceneClock::new(past_end, Instant::now(), rate(1)),
            Err(BuildSceneClockError { time: past_end })
        );
        assert!(SceneClock::new(ClockWindow::START, Instant::now(), rate(1)).is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn the_clock_advances_at_its_rate_and_not_while_paused() {
        let start = time(1_000, 250);
        for r in [1, 10, 100_000] {
            let clock = SceneClock::new(start, Instant::now(), rate(r)).unwrap();
            tokio::time::advance(Duration::from_millis(1_500)).await;
            let reading = clock.now();
            let expected = start
                .checked_add(
                    Span::new(0, 500_000_000)
                        .unwrap()
                        .checked_mul(3 * i64::from(r))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(reading.time(), expected, "at {r}×");
            assert_eq!(reading.state(), ClockState::Running);
        }
        let paused = SceneClock::new(start, Instant::now(), TimeRate::PAUSED).unwrap();
        tokio::time::advance(Duration::from_secs(3_600)).await;
        let reading = paused.now();
        assert_eq!(
            (reading.time(), reading.state()),
            (start, ClockState::Paused)
        );
        assert_eq!(paused.instant_of(time(2_000, 0)), None);
    }

    #[tokio::test(start_paused = true)]
    async fn instant_of_inverts_at_instant_to_the_nanosecond() {
        let anchor = Instant::now();
        let start = time(-31_557_600 * 999, 123_456_789);
        for r in [1, 100_000] {
            let clock = SceneClock::new(start, anchor, rate(r)).unwrap();
            for real_nanos in [0_u64, 1, 7, 999_999_999, 1_000_000_001, 86_400_000_000_123] {
                let at = anchor + Duration::from_nanos(real_nanos);
                let read = clock.at_instant(at).time();
                assert_eq!(
                    clock.instant_of(read),
                    Some(at),
                    "at {r}× after {real_nanos} ns"
                );
            }
            // A scene time between two instants' readings is due at the later instant: at 1× every
            // nanosecond is an instant's, at 100,000× one is due a nanosecond of real time later.
            let between = start.checked_add(Span::new(0, 1).unwrap()).unwrap();
            let due = clock.instant_of(between).unwrap();
            assert_eq!(due, anchor + Duration::from_nanos(1));
            assert!(clock.at_instant(due).time() >= between);
            // Before the anchor's time is due at once.
            assert_eq!(clock.instant_of(time(-31_557_600 * 1_000, 0)), Some(anchor));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_clock_stops_at_the_window_s_end_and_holds_there() {
        let near_end = ClockWindow::END
            .checked_sub(Span::from_seconds(100_000))
            .unwrap();
        let clock = SceneClock::new(near_end, Instant::now(), rate(100_000)).unwrap();
        tokio::time::advance(Duration::from_millis(999)).await;
        assert_eq!(clock.now().state(), ClockState::Running);
        tokio::time::advance(Duration::from_millis(1)).await;
        let at_end = clock.now();
        assert_eq!(
            (at_end.time(), at_end.state()),
            (ClockWindow::END, ClockState::WindowLimit)
        );
        tokio::time::advance(Duration::from_secs(3_600)).await;
        let held = clock.now();
        assert_eq!(
            (held.time(), held.state()),
            (ClockWindow::END, ClockState::WindowLimit)
        );
        assert_eq!(
            SceneClockDto::from(held).state,
            SceneClockStateDto::WindowLimit
        );
        let beyond = ClockWindow::END.checked_add(Span::from_seconds(1)).unwrap();
        assert_eq!(clock.instant_of(beyond), None);
    }
}
