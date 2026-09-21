//! The universe clock: one coordinate time in the galaxy's rest frame.
//!
//! [`UniverseTime`] is an `i64` of seconds plus a `u32` of nanoseconds. An `f64` of seconds would
//! resolve only about a millisecond at the far end of the source horizon (the spacing of `f64`
//! values near 8.3 × 10¹² s is 2⁻¹⁰ s), which is the reason for the type. Zero is the epoch: the
//! instant the galaxy's fields describe. Everything that has a position or a state takes a time
//! and is symmetric in it.
//!
//! Both types are normalised like [`std::time::Duration`] but signed: the nanoseconds are always
//! in `0..1e9` and the value is `seconds + nanos × 10⁻⁹`, so −0.5 s is `(−1, 500_000_000)`. The
//! derived ordering is therefore chronological, and all arithmetic is checked.
//!
//! Two constants belong to the generator version, [`CLOCK_WINDOW_H`] and [`LIGHT_CROSSING_L`],
//! and together they give the [`SourceHorizon`].

use std::error::Error;
use std::fmt;

use crate::math;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// Nanoseconds in one second; the exclusive bound of every `nanos` field.
pub const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// Seconds in one Julian year, as an integer: 365.25 × 86,400.
const SECONDS_PER_JULIAN_YEAR_I64: i64 = 31_557_600;

/// **H**, the clock window: 1,000 Julian years = 31,557,600,000 s. Part of the generator version.
///
/// Brainstorm: "Within it a visit to any object is accurate to the guarantees of Orbits and time.
/// Beyond it nothing breaks locally, but the errors of straight-line drift grow with the square of
/// the time and distant events stop being findable."
pub const CLOCK_WINDOW_H: Span = Span::from_seconds(1_000 * SECONDS_PER_JULIAN_YEAR_I64);

/// **L**, the light-crossing bound: 2¹⁸ Julian years (262,144) = 8,272,635,494,400 s. Part of the
/// generator version.
///
/// Brainstorm: "The root cube's diagonal is 227,023 ly, so no observer inside it can receive light
/// that left a source more than L before."
pub const LIGHT_CROSSING_L: Span = Span::from_seconds((1 << 18) * SECONDS_PER_JULIAN_YEAR_I64);

/// The clock window, `[−H, +H]`: the times at which a visit to any object is accurate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClockWindow;

impl ClockWindow {
    /// `−H`.
    pub const START: UniverseTime = UniverseTime {
        seconds: -CLOCK_WINDOW_H.seconds,
        nanos: 0,
    };

    /// `+H`.
    pub const END: UniverseTime = UniverseTime {
        seconds: CLOCK_WINDOW_H.seconds,
        nanos: 0,
    };

    /// Whether `t` lies in the window, inclusive at both ends.
    #[must_use]
    pub fn contains(t: UniverseTime) -> bool {
        Self::START <= t && t <= Self::END
    }
}

/// The source horizon, `[−(H + L), +H]`: every emission time that any observer inside the root
/// cube can see or touch during play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceHorizon;

impl SourceHorizon {
    /// `−(H + L)`.
    pub const START: UniverseTime = UniverseTime {
        seconds: -(CLOCK_WINDOW_H.seconds + LIGHT_CROSSING_L.seconds),
        nanos: 0,
    };

    /// `+H`.
    pub const END: UniverseTime = ClockWindow::END;

    /// Whether `t` lies in the horizon, inclusive at both ends.
    #[must_use]
    pub fn contains(t: UniverseTime) -> bool {
        Self::START <= t && t <= Self::END
    }
}

/// A [`UniverseTime`] could not be built from its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildUniverseTimeError {
    /// The nanosecond field was not below one second.
    NanosOutOfRange {
        /// The offending value.
        nanos: u32,
    },
}

impl fmt::Display for BuildUniverseTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NanosOutOfRange { nanos } => {
                write!(f, "nanoseconds {nanos} must be below {NANOS_PER_SECOND}")
            }
        }
    }
}

impl Error for BuildUniverseTimeError {}

/// A [`Span`] could not be built from its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSpanError {
    /// The nanosecond field was not below one second.
    NanosOutOfRange {
        /// The offending value.
        nanos: u32,
    },
}

impl fmt::Display for BuildSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NanosOutOfRange { nanos } => {
                write!(f, "nanoseconds {nanos} must be below {NANOS_PER_SECOND}")
            }
        }
    }
}

impl Error for BuildSpanError {}

/// An instant of the universe clock: seconds since the epoch, to the nanosecond.
///
/// Field order gives the derived ordering: whole seconds first, then nanoseconds, which is
/// chronological because the nanoseconds are always non-negative.
///
/// # Examples
///
/// ```
/// use hyperion_sim::time::{Span, UniverseTime};
///
/// let t = UniverseTime::new(-1, 500_000_000)?; // −0.5 s
/// assert!(t < UniverseTime::EPOCH);
/// assert_eq!(t.checked_add(Span::new(1, 0)?), Some(UniverseTime::new(0, 500_000_000)?));
/// assert_eq!(t.to_string(), "T-0.500000000 s");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UniverseTime {
    seconds: i64,
    nanos: u32,
}

impl UniverseTime {
    /// The epoch: the instant the galaxy's fields describe. Also the [`Default`].
    pub const EPOCH: Self = Self {
        seconds: 0,
        nanos: 0,
    };

    /// Builds an instant from floored whole seconds and non-negative nanoseconds.
    ///
    /// # Errors
    ///
    /// [`BuildUniverseTimeError::NanosOutOfRange`] if `nanos ≥ 10⁹`.
    pub const fn new(seconds: i64, nanos: u32) -> Result<Self, BuildUniverseTimeError> {
        if nanos >= NANOS_PER_SECOND {
            return Err(BuildUniverseTimeError::NanosOutOfRange { nanos });
        }
        Ok(Self { seconds, nanos })
    }

    /// The instant a whole number of Julian years from the epoch, or `None` on overflow.
    #[must_use]
    pub fn from_julian_years(years: i64) -> Option<Self> {
        let seconds = years.checked_mul(SECONDS_PER_JULIAN_YEAR_I64)?;
        Some(Self { seconds, nanos: 0 })
    }

    /// The whole seconds, floored: the wire form's first field.
    #[must_use]
    pub const fn seconds(self) -> i64 {
        self.seconds
    }

    /// The nanoseconds above the whole seconds, in `0..1e9`: the wire form's second field.
    #[must_use]
    pub const fn subsec_nanos(self) -> u32 {
        self.nanos
    }

    /// This instant plus a span, or `None` on overflow.
    #[must_use]
    pub fn checked_add(self, span: Span) -> Option<Self> {
        let (seconds, nanos) = add_parts((self.seconds, self.nanos), (span.seconds, span.nanos))?;
        Some(Self { seconds, nanos })
    }

    /// This instant minus a span, or `None` on overflow.
    #[must_use]
    pub fn checked_sub(self, span: Span) -> Option<Self> {
        let (seconds, nanos) = sub_parts((self.seconds, self.nanos), (span.seconds, span.nanos))?;
        Some(Self { seconds, nanos })
    }

    /// The span from `earlier` to this instant, negative if `earlier` is later, or `None` on
    /// overflow.
    #[must_use]
    pub fn checked_since(self, earlier: Self) -> Option<Span> {
        let (seconds, nanos) =
            sub_parts((self.seconds, self.nanos), (earlier.seconds, earlier.nanos))?;
        Some(Span { seconds, nanos })
    }

    /// The span from the epoch to this instant. Cannot overflow.
    #[must_use]
    pub const fn since_epoch(self) -> Span {
        Span {
            seconds: self.seconds,
            nanos: self.nanos,
        }
    }
}

impl fmt::Display for UniverseTime {
    /// `T+<seconds>.<9 digits> s`, or `T-…` before the epoch.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("T")?;
        fmt_signed(f, self.seconds, self.nanos)
    }
}

/// A signed duration of the universe clock, to the nanosecond.
///
/// Represented like [`UniverseTime`]: floored whole seconds plus non-negative nanoseconds, so the
/// derived ordering is by value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    seconds: i64,
    nanos: u32,
}

impl Span {
    /// The zero span.
    pub const ZERO: Self = Self {
        seconds: 0,
        nanos: 0,
    };

    /// Builds a span from floored whole seconds and non-negative nanoseconds.
    ///
    /// # Errors
    ///
    /// [`BuildSpanError::NanosOutOfRange`] if `nanos ≥ 10⁹`.
    pub const fn new(seconds: i64, nanos: u32) -> Result<Self, BuildSpanError> {
        if nanos >= NANOS_PER_SECOND {
            return Err(BuildSpanError::NanosOutOfRange { nanos });
        }
        Ok(Self { seconds, nanos })
    }

    /// A span of whole seconds.
    #[must_use]
    pub const fn from_seconds(seconds: i64) -> Self {
        Self { seconds, nanos: 0 }
    }

    /// A span of whole Julian years, or `None` on overflow.
    #[must_use]
    pub fn from_julian_years(years: i64) -> Option<Self> {
        Some(Self::from_seconds(
            years.checked_mul(SECONDS_PER_JULIAN_YEAR_I64)?,
        ))
    }

    /// A span from a float of seconds, floored to the nanosecond.
    ///
    /// The result is the latest whole nanosecond not after the float's exact value, computed
    /// without rounding. The `f64` nearest a decimal is not the decimal itself, so this can
    /// differ by one nanosecond from the decimal's floor: the `f64` nearest `−10⁻⁹` lies just
    /// below it and floors to −2 ns.
    ///
    /// Returns `None` for a NaN, an infinity, or a value whose whole seconds do not fit in `i64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::time::Span;
    ///
    /// assert_eq!(Span::from_seconds_f64(-0.5), Some(Span::new(-1, 500_000_000)?));
    /// assert_eq!(Span::from_seconds_f64(f64::NAN), None);
    /// # Ok::<(), hyperion_sim::time::BuildSpanError>(())
    /// ```
    #[must_use]
    pub fn from_seconds_f64(seconds: f64) -> Option<Self> {
        if !seconds.is_finite() {
            return None;
        }
        let whole = seconds.floor();
        // 2^63 is exact in f64 and i64::MAX is not, so the check is against the exclusive bound.
        if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&whole) {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "whole is an integer inside the i64 range, checked above"
        )]
        let whole_seconds = whole as i64;
        // Work on the magnitude, whose fraction is exact: at 1 or above the magnitude and its floor
        // are within a factor of two (Sterbenz), and below 1 the floor is 0. For negative input
        // `seconds − whole` is not exact between −1 and 0.
        let magnitude = seconds.abs();
        let (floor, has_remainder) = floor_nanos(magnitude - magnitude.floor());
        let nanos = if seconds >= 0.0 || (floor == 0 && !has_remainder) {
            floor
        } else {
            // −(m + x ns) floored is (−⌊m⌋ − 1) s plus (10⁹ − ⌈x⌉) ns, and `whole` is already
            // −⌊m⌋ − 1. ⌈x⌉ is at least 1 because the fraction is not zero, and at most 10⁹.
            NANOS_PER_SECOND - (floor + u32::from(has_remainder))
        };
        Some(Self {
            seconds: whole_seconds,
            nanos,
        })
    }

    /// The whole seconds, floored: the wire form's first field.
    #[must_use]
    pub const fn seconds(self) -> i64 {
        self.seconds
    }

    /// The nanoseconds above the whole seconds, in `0..1e9`: the wire form's second field.
    #[must_use]
    pub const fn subsec_nanos(self) -> u32 {
        self.nanos
    }

    /// The span as a float of seconds, which closed forms take.
    ///
    /// Lossy beyond 2⁵³ ns (about 104 days): at the far end of the source horizon the result is
    /// only good to about a millisecond. A closed form that needs better takes the clock time in
    /// its own units, as `stellar` and `planetary` arrange.
    #[must_use]
    pub fn as_seconds_f64(self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "documented as lossy beyond 2^53 ns"
        )]
        let whole = self.seconds as f64;
        whole + f64::from(self.nanos) * 1e-9
    }

    /// The span as a float of Julian years, with the loss of [`Span::as_seconds_f64`].
    #[must_use]
    pub fn as_julian_years_f64(self) -> f64 {
        self.as_seconds_f64() / SECONDS_PER_JULIAN_YEAR
    }

    /// Whether the span is below zero.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.seconds < 0
    }

    /// This span plus another, or `None` on overflow.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let (seconds, nanos) = add_parts((self.seconds, self.nanos), (other.seconds, other.nanos))?;
        Some(Self { seconds, nanos })
    }

    /// This span minus another, or `None` on overflow.
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        let (seconds, nanos) = sub_parts((self.seconds, self.nanos), (other.seconds, other.nanos))?;
        Some(Self { seconds, nanos })
    }

    /// The negation, or `None` on overflow (only for whole `i64::MIN` seconds).
    #[must_use]
    pub fn checked_neg(self) -> Option<Self> {
        let (seconds, nanos) = neg_parts((self.seconds, self.nanos))?;
        Some(Self { seconds, nanos })
    }

    /// The absolute value, or `None` on overflow.
    #[must_use]
    pub fn checked_abs(self) -> Option<Self> {
        if self.is_negative() {
            self.checked_neg()
        } else {
            Some(self)
        }
    }

    /// This span times a whole number, or `None` on overflow.
    #[must_use]
    pub fn checked_mul(self, factor: i64) -> Option<Self> {
        let nanos =
            i128::from(self.seconds) * i128::from(NANOS_PER_SECOND) + i128::from(self.nanos);
        let product = nanos.checked_mul(i128::from(factor))?;
        // The quotient overflowing i64 is the overflow this returns `None` for.
        let seconds = i64::try_from(product.div_euclid(i128::from(NANOS_PER_SECOND))).ok()?;
        // Cannot fail: a Euclidean remainder by 10⁹ lies in 0..10⁹.
        let nanos = u32::try_from(product.rem_euclid(i128::from(NANOS_PER_SECOND))).ok()?;
        Some(Self { seconds, nanos })
    }
}

impl fmt::Display for Span {
    /// `+<seconds>.<9 digits> s` or `-…`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_signed(f, self.seconds, self.nanos)
    }
}

/// Writes `+<seconds>.<9 digits> s` or `-<seconds>.<9 digits> s` for a normalised pair.
fn fmt_signed(f: &mut fmt::Formatter<'_>, seconds: i64, nanos: u32) -> fmt::Result {
    if seconds >= 0 {
        write!(f, "+{}.{nanos:09} s", seconds.unsigned_abs())
    } else if nanos == 0 {
        write!(f, "-{}.000000000 s", seconds.unsigned_abs())
    } else {
        write!(
            f,
            "-{}.{:09} s",
            seconds.unsigned_abs() - 1,
            NANOS_PER_SECOND - nanos
        )
    }
}

/// For a fraction of a second in `[0, 1)`, `⌊fraction × 10⁹⌋` computed exactly, and whether the
/// product had anything after the point.
fn floor_nanos(fraction: f64) -> (u32, bool) {
    const NANOS: f64 = 1e9;
    let product = fraction * NANOS;
    // The rounding error of a product is itself a float, and the fused multiply-add returns it
    // exactly: `residual = fraction × 10⁹ − product`.
    let residual = math::mul_add(fraction, NANOS, -product);
    let floor = product.floor();
    let (floor, has_remainder) = if floor < product {
        // No whole number lies between the rounded product and the exact one: it would be a
        // float nearer the exact product than the rounded one is.
        (floor, true)
    } else if residual < 0.0 {
        (floor - 1.0, true)
    } else {
        (floor, residual > 0.0)
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "floor is a whole number in [0, 1e9), because the exact product is"
    )]
    let nanos = floor as u32;
    (nanos, has_remainder)
}

/// Adds two normalised pairs, carrying nanoseconds; `None` if the result's seconds overflow.
///
/// The seconds are summed in `i128` with the carry, so a sum that leaves the `i64` range only
/// before the carry brings it back is not mistaken for overflow.
fn add_parts((s1, n1): (i64, u32), (s2, n2): (i64, u32)) -> Option<(i64, u32)> {
    // Both are below 10⁹, so the sum is below 2 × 10⁹ and fits in u32.
    let nanos = n1 + n2;
    let (carry, nanos) = if nanos >= NANOS_PER_SECOND {
        (1, nanos - NANOS_PER_SECOND)
    } else {
        (0, nanos)
    };
    let seconds = i64::try_from(i128::from(s1) + i128::from(s2) + carry).ok()?;
    Some((seconds, nanos))
}

/// Subtracts two normalised pairs, borrowing nanoseconds; `None` if the result's seconds
/// overflow, with the same `i128` sum as [`add_parts`].
fn sub_parts((s1, n1): (i64, u32), (s2, n2): (i64, u32)) -> Option<(i64, u32)> {
    let (borrow, nanos) = if n1 >= n2 {
        (0, n1 - n2)
    } else {
        (1, n1 + NANOS_PER_SECOND - n2)
    };
    let seconds = i64::try_from(i128::from(s1) - i128::from(s2) - borrow).ok()?;
    Some((seconds, nanos))
}

/// Negates a normalised pair; `None` only for whole `i64::MIN` seconds.
fn neg_parts((s, n): (i64, u32)) -> Option<(i64, u32)> {
    if n == 0 {
        Some((s.checked_neg()?, 0))
    } else {
        // −(s + n × 10⁻⁹) = (−s − 1) + (1 − n × 10⁻⁹), and −s − 1 is `!s`, which never overflows.
        Some((!s, NANOS_PER_SECOND - n))
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::ROOT_HALF_WIDTH_LY;

    fn t(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    fn s(seconds: i64, nanos: u32) -> Span {
        Span::new(seconds, nanos).unwrap()
    }

    /// Edge values, negative ones included, that every representation test runs over.
    const EDGES: [(i64, u32); 16] = [
        (i64::MIN, 0),
        (i64::MIN, 999_999_999),
        (i64::MAX, 0),
        (i64::MAX, 999_999_999),
        (0, 0),
        (0, 1),
        (0, 999_999_999),
        (1, 0),
        (-1, 0),
        (-1, 999_999_999),
        (-1, 500_000_000),
        (31_557_600_000, 0),
        (-8_304_193_094_400, 123_456_789),
        (i64::MAX / 2, 999_999_999),
        (i64::MIN / 2, 1),
        (i64::MIN / 2, 0),
    ];

    #[test]
    fn ordering_is_chronological_across_zero() {
        assert!(t(-1, 999_999_999) < t(0, 0));
        assert!(t(0, 0) < t(0, 1));
        assert!(t(-2, 999_999_999) < t(-1, 0));
        assert!(s(-1, 999_999_999) < Span::ZERO);
    }

    #[test]
    fn nanoseconds_at_or_above_a_second_are_rejected() {
        assert_eq!(
            UniverseTime::new(0, NANOS_PER_SECOND),
            Err(BuildUniverseTimeError::NanosOutOfRange {
                nanos: NANOS_PER_SECOND
            })
        );
        assert_eq!(
            Span::new(0, u32::MAX),
            Err(BuildSpanError::NanosOutOfRange { nanos: u32::MAX })
        );
    }

    #[test]
    fn addition_and_subtraction_carry_and_borrow_in_both_signs() {
        assert_eq!(
            t(0, 600_000_000).checked_add(s(0, 600_000_000)),
            Some(t(1, 200_000_000))
        );
        assert_eq!(
            t(-1, 600_000_000).checked_add(s(0, 600_000_000)),
            Some(t(0, 200_000_000))
        );
        assert_eq!(
            t(1, 200_000_000).checked_sub(s(0, 600_000_000)),
            Some(t(0, 600_000_000))
        );
        assert_eq!(
            t(0, 200_000_000).checked_sub(s(0, 600_000_000)),
            Some(t(-1, 600_000_000))
        );
        assert_eq!(
            t(0, 0).checked_add(s(-1, 500_000_000)),
            Some(t(-1, 500_000_000))
        );
        assert_eq!(
            t(0, 0).checked_sub(s(-1, 500_000_000)),
            Some(t(0, 500_000_000))
        );
        assert_eq!(
            t(0, 500_000_000).checked_since(t(-1, 750_000_000)),
            Some(s(0, 750_000_000))
        );
        assert_eq!(
            t(-1, 750_000_000).checked_since(t(0, 500_000_000)),
            Some(s(-1, 250_000_000))
        );
    }

    #[test]
    fn overflow_returns_none() {
        assert_eq!(t(i64::MAX, 0).checked_add(s(1, 0)), None);
        assert_eq!(t(i64::MAX, 999_999_999).checked_add(s(0, 1)), None);
        assert_eq!(t(i64::MIN, 0).checked_sub(s(0, 1)), None);
        assert_eq!(t(i64::MIN, 0).checked_since(t(1, 0)), None);
        assert_eq!(UniverseTime::from_julian_years(i64::MAX), None);
        assert_eq!(s(i64::MIN, 0).checked_neg(), None);
        assert_eq!(s(i64::MIN, 1).checked_neg(), Some(s(i64::MAX, 999_999_999)));
        assert_eq!(s(i64::MAX, 0).checked_mul(2), None);
    }

    /// At the ends of the `i64` range the whole seconds overflow before the nanosecond carry or
    /// borrow brings them back: the result is representable and must not be `None`.
    #[test]
    fn a_carry_or_borrow_back_into_range_is_not_overflow() {
        assert_eq!(
            t(i64::MIN, 600_000_000).checked_add(s(-1, 600_000_000)),
            Some(t(i64::MIN, 200_000_000))
        );
        assert_eq!(
            t(i64::MAX, 200_000_000).checked_sub(s(-1, 600_000_000)),
            Some(t(i64::MAX, 600_000_000))
        );
        assert_eq!(
            t(i64::MAX, 500_000_000).checked_since(t(-1, 600_000_000)),
            Some(s(i64::MAX, 900_000_000))
        );
        assert_eq!(
            s(i64::MIN, 999_999_999).checked_add(s(-1, 1)),
            Some(s(i64::MIN, 0))
        );
        assert_eq!(
            s(i64::MAX, 0).checked_sub(s(-1, 1)),
            Some(s(i64::MAX, 999_999_999))
        );
        // And one nanosecond further is still overflow.
        assert_eq!(t(i64::MIN, 0).checked_add(s(-1, 999_999_999)), None);
        assert_eq!(
            t(i64::MAX, 999_999_999).checked_sub(s(-1, 999_999_999)),
            None
        );
        assert_eq!(s(i64::MIN, 999_999_999).checked_add(s(-1, 0)), None);
    }

    /// The value in nanoseconds, as an independent reference model.
    fn total_nanos((seconds, nanos): (i64, u32)) -> i128 {
        i128::from(seconds) * 1_000_000_000 + i128::from(nanos)
    }

    /// The normalised pair of a nanosecond count, or `None` outside the `i64` seconds range.
    fn from_total_nanos(total: i128) -> Option<(i64, u32)> {
        Some((
            i64::try_from(total.div_euclid(1_000_000_000)).ok()?,
            u32::try_from(total.rem_euclid(1_000_000_000)).ok()?,
        ))
    }

    /// Every operation agrees with plain `i128` arithmetic on nanoseconds, including where it
    /// overflows, over the edge table and LCG pairs whose seconds sit near the `i64` ends.
    #[test]
    fn arithmetic_matches_an_i128_nanosecond_model() {
        let mut g = hyperion_testkit::lcg::Lcg::new(0x71de);
        let mut pairs: Vec<(i64, u32)> = EDGES.to_vec();
        for _ in 0..200 {
            let near_end = i64::try_from(g.next_below(4)).unwrap();
            let seconds = match g.next_below(3) {
                0 => i64::MIN + near_end,
                1 => i64::MAX - near_end,
                _ => g.next_u64().cast_signed(),
            };
            let nanos = u32::try_from(g.next_below(u64::from(NANOS_PER_SECOND))).unwrap();
            pairs.push((seconds, nanos));
        }
        for &a in &pairs {
            for &b in &pairs {
                let sum = from_total_nanos(total_nanos(a) + total_nanos(b));
                let difference = from_total_nanos(total_nanos(a) - total_nanos(b));
                let parts = |x: Option<UniverseTime>| x.map(|x| (x.seconds(), x.subsec_nanos()));
                let span_parts = |x: Option<Span>| x.map(|x| (x.seconds(), x.subsec_nanos()));
                assert_eq!(
                    parts(t(a.0, a.1).checked_add(s(b.0, b.1))),
                    sum,
                    "{a:?} + {b:?}"
                );
                assert_eq!(parts(t(a.0, a.1).checked_sub(s(b.0, b.1))), difference);
                assert_eq!(
                    span_parts(t(a.0, a.1).checked_since(t(b.0, b.1))),
                    difference
                );
                assert_eq!(span_parts(s(a.0, a.1).checked_add(s(b.0, b.1))), sum);
                assert_eq!(span_parts(s(a.0, a.1).checked_sub(s(b.0, b.1))), difference);
            }
            let negated = from_total_nanos(-total_nanos(a));
            assert_eq!(
                s(a.0, a.1)
                    .checked_neg()
                    .map(|x| (x.seconds(), x.subsec_nanos())),
                negated
            );
            for factor in [-3, -1, 0, 2, 7] {
                assert_eq!(
                    s(a.0, a.1)
                        .checked_mul(factor)
                        .map(|x| (x.seconds(), x.subsec_nanos())),
                    from_total_nanos(total_nanos(a) * i128::from(factor)),
                    "{a:?} × {factor}"
                );
            }
        }
    }

    #[test]
    fn add_then_since_gives_the_span_back() {
        for (ts, tn) in EDGES {
            for (ss, sn) in EDGES {
                let time = t(ts, tn);
                let span = s(ss, sn);
                if let Some(later) = time.checked_add(span) {
                    assert_eq!(later.checked_since(time), Some(span), "{time} + {span}");
                    assert_eq!(
                        later.checked_sub(span),
                        Some(time),
                        "{time} + {span} - {span}"
                    );
                }
            }
        }
    }

    #[test]
    fn getters_round_trip_through_new_for_both_types() {
        for (seconds, nanos) in EDGES {
            let time = t(seconds, nanos);
            assert_eq!(
                UniverseTime::new(time.seconds(), time.subsec_nanos()),
                Ok(time)
            );
            let span = s(seconds, nanos);
            assert_eq!(Span::new(span.seconds(), span.subsec_nanos()), Ok(span));
            assert_eq!(time.since_epoch(), span);
        }
    }

    #[test]
    fn negation_and_absolute_value() {
        assert_eq!(s(-1, 500_000_000).checked_neg(), Some(s(0, 500_000_000)));
        assert_eq!(s(0, 500_000_000).checked_neg(), Some(s(-1, 500_000_000)));
        assert_eq!(s(-3, 0).checked_abs(), Some(s(3, 0)));
        assert_eq!(s(2, 1).checked_abs(), Some(s(2, 1)));
        assert_eq!(s(-1, 500_000_000).checked_mul(3), Some(s(-2, 500_000_000)));
        assert_eq!(s(1, 500_000_000).checked_mul(-2), Some(s(-3, 0)));
    }

    #[test]
    fn float_conversions_floor_to_the_nanosecond() {
        assert_eq!(Span::from_seconds_f64(1.5), Some(s(1, 500_000_000)));
        assert_eq!(Span::from_seconds_f64(-0.5), Some(s(-1, 500_000_000)));
        assert_eq!(Span::from_seconds_f64(-1.0), Some(s(-1, 0)));
        assert_eq!(
            Span::from_seconds_f64(0.999_999_999_9),
            Some(s(0, 999_999_999))
        );
        assert_eq!(Span::from_seconds_f64(f64::NAN), None);
        assert_eq!(Span::from_seconds_f64(f64::INFINITY), None);
        assert_eq!(Span::from_seconds_f64(1e19), None);
        assert_eq!(
            Span::from_seconds_f64(-9_223_372_036_854_775_808.0),
            Some(s(i64::MIN, 0))
        );
        assert_eq!(Span::from_seconds_f64(-0.0), Some(Span::ZERO));
        assert_eq!(Span::from_seconds_f64(-3.0), Some(s(-3, 0)));
        assert_same_bits(s(-1, 500_000_000).as_seconds_f64(), -0.5);
        assert_same_bits(CLOCK_WINDOW_H.as_julian_years_f64(), 1_000.0);
    }

    /// The floor is of the float's exact value. The `f64` nearest 0.1 is 0.1 + 5.6 × 10⁻¹⁸, so
    /// −0.1 floors to −0.100 000 001 s; the `f64` nearest 10⁻⁹ is 10⁻⁹ + 6.2 × 10⁻²⁶, so −10⁻⁹
    /// floors to −2 ns. A float of whole nanoseconds, such as 0.375, is exact in both signs.
    #[test]
    fn float_conversion_floors_the_exact_value() {
        assert_eq!(Span::from_seconds_f64(0.1), Some(s(0, 100_000_000)));
        assert_eq!(Span::from_seconds_f64(-0.1), Some(s(-1, 899_999_999)));
        assert_eq!(Span::from_seconds_f64(1e-9), Some(s(0, 1)));
        assert_eq!(Span::from_seconds_f64(-1e-9), Some(s(-1, 999_999_998)));
        assert_eq!(Span::from_seconds_f64(0.375), Some(s(0, 375_000_000)));
        assert_eq!(Span::from_seconds_f64(-0.375), Some(s(-1, 625_000_000)));
        assert_eq!(Span::from_seconds_f64(-2.375), Some(s(-3, 625_000_000)));
        // Just below a whole second, in both signs: the product rounds up to 10⁹ and the
        // residual brings it back.
        let below_one = 1.0_f64.next_down();
        assert_eq!(Span::from_seconds_f64(below_one), Some(s(0, 999_999_999)));
        assert_eq!(Span::from_seconds_f64(-below_one), Some(s(-1, 0)));
        // A value far below a nanosecond.
        assert_eq!(Span::from_seconds_f64(1e-300), Some(s(0, 0)));
        assert_eq!(Span::from_seconds_f64(-1e-300), Some(s(-1, 999_999_999)));
        // Whole nanoseconds that a naive product rounds across: 0.3 × 10⁹ is 299,999,999.99….
        for nanos in [1_u32, 7, 300_000_000, 999_999_999] {
            let whole = s(12, nanos);
            let float = 12.0 + f64::from(nanos) * 1e-9;
            let floored = Span::from_seconds_f64(float).unwrap();
            assert!(
                floored == whole || floored.checked_add(s(0, 1)) == Some(whole),
                "{float}"
            );
        }
    }

    #[test]
    fn display_prints_sign_and_nine_digits() {
        assert_eq!(t(0, 0).to_string(), "T+0.000000000 s");
        assert_eq!(t(12, 5).to_string(), "T+12.000000005 s");
        assert_eq!(t(-1, 500_000_000).to_string(), "T-0.500000000 s");
        assert_eq!(t(-2, 0).to_string(), "T-2.000000000 s");
        assert_eq!(
            t(i64::MIN, 1).to_string(),
            "T-9223372036854775807.999999999 s"
        );
        assert_eq!(s(-1, 999_999_999).to_string(), "-0.000000001 s");
    }

    /// The reason for the type: near 8.3 × 10¹² s an `f64` steps by 2⁻¹⁰ s, about a millisecond.
    #[test]
    fn f64_seconds_resolve_a_millisecond_at_the_horizon() {
        let far = 8.3e12_f64;
        assert_same_bits(far.next_up() - far, 1.0 / 1024.0);
        let ns = t(-8_300_000_000_000, 1)
            .checked_since(t(-8_300_000_000_000, 0))
            .unwrap();
        assert_eq!(ns, s(0, 1));
    }

    #[test]
    fn h_and_l_are_the_pinned_integers() {
        assert_eq!(CLOCK_WINDOW_H, s(31_557_600_000, 0));
        assert_eq!(LIGHT_CROSSING_L, s(8_272_635_494_400, 0));
        assert_eq!(Span::from_julian_years(1_000), Some(CLOCK_WINDOW_H));
        assert_eq!(Span::from_julian_years(262_144), Some(LIGHT_CROSSING_L));
    }

    #[test]
    fn the_horizon_runs_from_minus_h_plus_l_to_plus_h() {
        assert_eq!(SourceHorizon::START, t(-8_304_193_094_400, 0));
        assert_eq!(SourceHorizon::END, t(31_557_600_000, 0));
        assert_eq!(ClockWindow::START, t(-31_557_600_000, 0));
        assert_eq!(ClockWindow::END, t(31_557_600_000, 0));
        assert_eq!(
            UniverseTime::EPOCH
                .checked_sub(CLOCK_WINDOW_H)
                .unwrap()
                .checked_sub(LIGHT_CROSSING_L),
            Some(SourceHorizon::START)
        );
    }

    #[test]
    fn contains_is_inclusive_at_both_ends() {
        for (start, end, contains) in [
            (
                SourceHorizon::START,
                SourceHorizon::END,
                SourceHorizon::contains as fn(_) -> bool,
            ),
            (ClockWindow::START, ClockWindow::END, ClockWindow::contains),
        ] {
            assert!(contains(start));
            assert!(contains(end));
            assert!(contains(UniverseTime::EPOCH));
            assert!(!contains(start.checked_sub(s(0, 1)).unwrap()));
            assert!(!contains(end.checked_add(s(0, 1)).unwrap()));
        }
        assert!(!ClockWindow::contains(SourceHorizon::START));
    }

    /// L in light-years, 262,144, exceeds the root cube's diagonal, 131,072 × √3 = 227,023.4 ly,
    /// so no observer inside the cube can see light older than L.
    #[test]
    fn l_exceeds_the_root_cube_diagonal() {
        let l_ly = LIGHT_CROSSING_L.as_julian_years_f64();
        assert_same_bits(l_ly, 262_144.0);
        let diagonal = f64::from(2 * ROOT_HALF_WIDTH_LY) * 3.0_f64.sqrt();
        assert!((diagonal - 227_023.4).abs() < 0.05, "{diagonal}");
        assert!(l_ly > diagonal);
    }
}
