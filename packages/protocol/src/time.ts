import type { UniverseTime } from "./generated/UniverseTime";

/**
 * Seconds in one Julian year, 365.25 days of 86,400 s, exact by definition.
 *
 * @remarks
 * The value of `units::consts::SECONDS_PER_JULIAN_YEAR` in `hyperion-sim`, and the year of every
 * `yr`, `Myr` and `Gyr` on the wire.
 */
export const SECONDS_PER_JULIAN_YEAR = 31_557_600;

const NANOS_PER_SECOND = 1_000_000_000;

/**
 * The instant `years` Julian years after the epoch (before it when negative), on the universe clock.
 *
 * @remarks
 * Whole years and the fraction of a year are converted apart, so that the conversion itself loses
 * no more than a few nanoseconds; the precision of `years` is the limit.
 *
 * @throws RangeError if `years` is not finite, or so large that its seconds are not exact.
 */
export function universeTimeFromYears(years: number): UniverseTime {
  if (!Number.isFinite(years)) {
    throw new RangeError(`${years} years is not a time`);
  }
  const wholeYears = Math.trunc(years);
  // Below one year in magnitude, so resolved to a few nanoseconds.
  const fractionSeconds = (years - wholeYears) * SECONDS_PER_JULIAN_YEAR;
  const fractionWholeSeconds = Math.floor(fractionSeconds);
  let seconds = wholeYears * SECONDS_PER_JULIAN_YEAR + fractionWholeSeconds;
  let nanos = Math.round((fractionSeconds - fractionWholeSeconds) * NANOS_PER_SECOND);
  if (nanos === NANOS_PER_SECOND) {
    seconds += 1;
    nanos = 0;
  }
  if (!Number.isSafeInteger(seconds)) {
    throw new RangeError(`${years} years is too far from the epoch to hold exactly`);
  }
  // `-0` would otherwise reach the wire as a distinct value from `0`.
  return { seconds: seconds === 0 ? 0 : seconds, nanos };
}

/** The time in Julian years since the epoch, negative before it. */
export function universeTimeToYears(time: UniverseTime): number {
  return (
    time.seconds / SECONDS_PER_JULIAN_YEAR +
    time.nanos / (NANOS_PER_SECOND * SECONDS_PER_JULIAN_YEAR)
  );
}
