//! The exact reduction of a clock interval modulo a period.
//!
//! An orbit's phase is the fraction of a period elapsed since a reference instant. Computing it
//! as an `f64` of seconds divided by the period loses the phase for short orbits over long times
//! (module docs, "Precision"), so the interval's integer seconds are reduced modulo the period
//! first, by operations that are all exact but the last additions.

use crate::math;
use crate::time::{NANOS_PER_SECOND, UniverseTime};

/// 2⁵³: every integer of smaller magnitude is an exact `f64`.
const EXACT_INTEGER_BOUND: u128 = 1 << 53;

/// 2³², the weight of the high half when the whole seconds are split.
const TWO_POW_32: f64 = 4_294_967_296.0;

/// The whole seconds and nanoseconds from `from` to `to`, exact: `to − from` as `(seconds,
/// nanos)` with the seconds floored and the nanoseconds in `0..1e9`. Two `i64` seconds are less
/// than 2⁶⁴ apart, so the difference always fits.
pub(super) fn interval(from: UniverseTime, to: UniverseTime) -> (i128, u32) {
    let mut seconds = i128::from(to.seconds()) - i128::from(from.seconds());
    let (to_nanos, from_nanos) = (to.subsec_nanos(), from.subsec_nanos());
    let nanos = if to_nanos >= from_nanos {
        to_nanos - from_nanos
    } else {
        seconds -= 1;
        NANOS_PER_SECOND - (from_nanos - to_nanos)
    };
    (seconds, nanos)
}

/// The fraction of `period` (seconds, finite and positive) elapsed after `seconds` whole seconds
/// and `nanos` nanoseconds, centred: in `[−½, ½)`, the interval modulo the period, divided by it.
///
/// The whole seconds are reduced before anything rounds. Below 2⁵³ in magnitude they are an
/// exact `f64`, and above it they are split into `hi × 2³² + lo`, each part exact. Each part is
/// reduced by [`math::fmod`], the truncated remainder, which is exact and keeps the dividend's
/// sign, and brought into
/// `[−P/2, P/2)` by at most one addition or subtraction of the period, which Sterbenz's lemma
/// makes exact there. The nanoseconds, as a correctly rounded fraction of a second, are reduced
/// the same way, taken as a signed remainder about the nearer whole second so that it too keeps
/// its precision either side of one. The parts are then summed, which is the first rounding,
/// centred again, exactly, and divided by the period.
///
/// Centring is what keeps the phase's relative precision on both sides of each whole period: a
/// time just before one gives a small negative fraction, not one just below 1. The result is
/// within a few units in the last place of the true fraction however long the interval, and two
/// intervals a whole number of periods apart give the same bits unless the true fraction lies
/// within that error of a rounding boundary.
pub(super) fn fraction_of_period(seconds: i128, nanos: u32, period: f64) -> f64 {
    // The nearer whole second and a signed remainder of at most half a second, so that a time a
    // nanosecond before a whole second is −1 ns and not 999,999,999 ns less a second.
    let (seconds, nanos) = if nanos >= NANOS_PER_SECOND / 2 {
        (seconds + 1, i64::from(nanos) - i64::from(NANOS_PER_SECOND))
    } else {
        (seconds, i64::from(nanos))
    };
    let nanos = i32::try_from(nanos).expect("a signed remainder of at most half a second");
    let whole = whole_seconds_modulo(seconds, period);
    let fraction_of_second = centred(math::fmod(f64::from(nanos) / 1e9, period), period);
    let fraction = centred(whole + fraction_of_second, period) / period;
    // A remainder in [−P/2, P/2) divides into [−½, ½]; the guard keeps ½ itself out.
    if fraction < 0.5 {
        fraction
    } else {
        fraction - 1.0
    }
}

/// `seconds` modulo `period`, centred into `[−P/2, P/2)`.
fn whole_seconds_modulo(seconds: i128, period: f64) -> f64 {
    if seconds.unsigned_abs() < EXACT_INTEGER_BOUND {
        #[expect(
            clippy::cast_precision_loss,
            reason = "an integer below 2^53 in magnitude is exact in f64"
        )]
        let exact = seconds as f64;
        return centred(math::fmod(exact, period), period);
    }
    // Two i64 seconds are under 2^64 apart, so the high half is under 2^32 in magnitude, and the
    // low half is under 2^32: both exact, and the high half times 2^32 exact as well.
    let high = seconds >> 32;
    let low = seconds & 0xFFFF_FFFF;
    #[expect(
        clippy::cast_precision_loss,
        reason = "both halves are below 2^53 in magnitude, exact in f64"
    )]
    let (high, low) = (high as f64 * TWO_POW_32, low as f64);
    centred(
        centred(math::fmod(high, period), period) + centred(math::fmod(low, period), period),
        period,
    )
}

/// A value in `[−P, P]` brought into `[−P/2, P/2)` by at most one addition or subtraction of the
/// period, each exact there by Sterbenz's lemma.
fn centred(value: f64, period: f64) -> f64 {
    let half = 0.5 * period;
    if value >= half {
        value - period
    } else if value < -half {
        value + period
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::time::Span;

    fn at(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    #[test]
    fn intervals_are_exact_and_normalised() {
        assert_eq!(interval(at(5, 200), at(7, 100)), (1, 999_999_900));
        assert_eq!(interval(at(7, 100), at(5, 200)), (-2, 100));
        assert_eq!(
            interval(at(i64::MIN, 0), at(i64::MAX, 999_999_999)).0,
            (1_i128 << 64) - 1
        );
        assert_eq!(
            interval(UniverseTime::EPOCH, at(-1, 500_000_000)),
            (-1, 500_000_000)
        );
    }

    #[test]
    fn a_whole_number_of_periods_gives_the_same_bits() {
        // A period of an integer number of seconds reduces exactly, however far out.
        let period = 86_400.0;
        let base = fraction_of_period(12_345, 678_000_000, period);
        for periods in [1_i128, 365_250, 100_000_000, 10_000_000_000_000] {
            let later = fraction_of_period(12_345 + periods * 86_400, 678_000_000, period);
            let earlier = fraction_of_period(12_345 - periods * 86_400, 678_000_000, period);
            assert_same_bits(later, base);
            assert_same_bits(earlier, base);
        }
        assert!((base - 12_345.678 / 86_400.0).abs() < 1e-16);
    }

    #[test]
    fn a_fractional_period_keeps_its_phase_a_thousand_years_out() {
        // About 1.0336 days and not a whole number of seconds: 1,428,855 ÷ 16 s exactly.
        let period = 89_303.437_5;
        let thousand_years = i128::from(Span::from_julian_years(1_000).unwrap().seconds());
        let fraction = fraction_of_period(thousand_years, 0, period);
        let mut exact_remainder = (thousand_years * 16).rem_euclid(1_428_855);
        if 2 * exact_remainder >= 1_428_855 {
            exact_remainder -= 1_428_855;
        }
        #[expect(clippy::cast_precision_loss, reason = "small integers, exact in f64")]
        let expected = exact_remainder as f64 / 1_428_855.0;
        assert!(
            (fraction - expected).abs() < 3e-16,
            "{fraction:e} against {expected:e}"
        );
    }

    #[test]
    fn fractions_are_centred_and_keep_their_precision_either_side_of_a_period() {
        let period = 10.0;
        assert!((fraction_of_period(-3, 0, period) + 0.3).abs() < 1e-16);
        assert!((fraction_of_period(-1, 500_000_000, period) + 0.05).abs() < 1e-17);
        assert!((fraction_of_period(7, 0, period) + 0.3).abs() < 1e-16);
        assert_same_bits(fraction_of_period(5, 0, period), -0.5);
        assert!(fraction_of_period(-10, 0, period).abs() < 1e-300);
        // A nanosecond before the epoch, and before a whole period after it, is −10⁻¹⁰ of the
        // period to full precision, not 1 − 10⁻¹⁰ rounded.
        let before = fraction_of_period(-1, 999_999_999, period);
        assert!((before + 1e-10).abs() < 1e-25, "{before:e}");
        let before_later = fraction_of_period(9, 999_999_999, period);
        assert!((before_later + 1e-10).abs() < 1e-25, "{before_later:e}");
        // A long period: a second before the epoch is not lost in the period's last place.
        let long = 3.2e16;
        assert!((fraction_of_period(-1, 0, long) * long + 1.0).abs() < 1e-15);
    }

    #[test]
    fn huge_intervals_split_exactly() {
        let period = 3.0;
        let seconds = (1_i128 << 62) + 7;
        // 2^62 = (2^2)^31 ≡ 1 (mod 3), so the interval is ≡ 1 + 7 ≡ 2 ≡ −1 (mod 3).
        assert!((fraction_of_period(seconds, 0, period) + 1.0 / 3.0).abs() < 1e-16);
        assert!((fraction_of_period(-seconds, 0, period) - 1.0 / 3.0).abs() < 1e-16);
    }

    #[test]
    fn sub_second_periods_reduce_the_nanoseconds_too() {
        let period = 0.25;
        assert!((fraction_of_period(3, 300_000_000, period) - 0.2).abs() < 1e-15);
        assert!((fraction_of_period(-1, 900_000_000, period) + 0.4).abs() < 1e-14);
    }
}
