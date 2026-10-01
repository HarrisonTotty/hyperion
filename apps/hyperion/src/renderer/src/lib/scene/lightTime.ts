/**
 * Light times to the nanosecond, as the simulation forms them (rendering plan R03, Design note 7):
 * the TypeScript mirror of `hyperion_sim::observe::light_time`, `time::Span` and its arithmetic.
 *
 * @remarks
 * The client's apparent positions are held to the simulation's golden vectors, and a light time a
 * nanosecond apart can change the iteration's stopping step and the emitted time, so every rounding
 * here is the simulation's: the light time is d ÷ c plus half a nanosecond, floored to the
 * nanosecond exactly (`Span::from_seconds_f64`, whose floor reads the exact rounding error of the
 * product by 10⁹ with a fused multiply-add, here by Dekker's exact product), and spans and times are
 * whole seconds and nanoseconds, never a float of seconds.
 */
import type { UniverseTime } from "@hyperion/protocol";

/** The speed of light, m/s, exact by the SI's definition (`units::consts::SPEED_OF_LIGHT`). */
export const SPEED_OF_LIGHT_M_PER_S = 299_792_458;

const NANOS_PER_SECOND = 1_000_000_000;

/**
 * A signed duration to the nanosecond: floored whole seconds and nanoseconds in `[0, 10⁹)`, as the
 * simulation's `Span` and the wire's `UniverseTime` hold an instant.
 */
export interface Span {
  readonly seconds: number;
  readonly nanos: number;
}

/** The zero span. */
export const ZERO_SPAN: Span = { seconds: 0, nanos: 0 };

/** Veltkamp's splitting constant for binary64, 2²⁷ + 1. */
const SPLIT = 134_217_729;

/** `a` split into a high half of 26 bits and the exact remainder (Veltkamp). */
function split(a: number): readonly [number, number] {
  const c = SPLIT * a;
  const high = c - (c - a);
  return [high, a - high];
}

/**
 * The rounding error of `a × b`, exactly: `a × b − fl(a × b)`, what `fma(a, b, −fl(a × b))` gives.
 *
 * @remarks
 * Dekker's (1971) two-product, exact where no partial product overflows or underflows, which holds
 * for a fraction of a second in `[0, 1)` above 10⁻²⁹⁰ times 10⁹.
 */
function productError(a: number, b: number, product: number): number {
  const [aHigh, aLow] = split(a);
  const [bHigh, bLow] = split(b);
  return aHigh * bHigh - product + aHigh * bLow + aLow * bHigh + aLow * bLow;
}

/**
 * ⌊`fraction` × 10⁹⌋ computed exactly, for a fraction of a second in `[0, 1)`: the simulation's
 * `floor_nanos`.
 */
function floorNanos(fraction: number): number {
  const product = fraction * NANOS_PER_SECOND;
  const residual = productError(fraction, NANOS_PER_SECOND, product);
  const floor = Math.floor(product);
  if (floor < product) {
    return floor;
  }
  return residual < 0 ? floor - 1 : floor;
}

/**
 * The span of `seconds`, not negative, floored to the nanosecond: the simulation's
 * `Span::from_seconds_f64` for a value at or above zero.
 */
function spanFromSeconds(seconds: number): Span {
  const whole = Math.floor(seconds);
  return { seconds: whole, nanos: floorNanos(seconds - whole) };
}

/**
 * The time light takes to cross `distanceM`, rounded to the nearest nanosecond: the simulation's
 * `light_time`, d ÷ c plus half a nanosecond, floored.
 *
 * @throws RangeError for a distance that is negative or not finite.
 */
export function lightTime(distanceM: number): Span {
  const seconds = distanceM / SPEED_OF_LIGHT_M_PER_S;
  if (!(Number.isFinite(seconds) && seconds >= 0)) {
    throw new RangeError(`a distance must be finite and not negative, got ${String(distanceM)} m`);
  }
  // Half a nanosecond before the floor rounds to the nearest nanosecond, as the simulation does.
  return spanFromSeconds(seconds + 5e-10);
}

/** `a − b` for two normalised pairs, borrowing a second for the nanoseconds. */
function subtract(
  a: { readonly seconds: number; readonly nanos: number },
  b: Span,
): { seconds: number; nanos: number } {
  return a.nanos >= b.nanos
    ? { seconds: a.seconds - b.seconds, nanos: a.nanos - b.nanos }
    : { seconds: a.seconds - b.seconds - 1, nanos: a.nanos + NANOS_PER_SECOND - b.nanos };
}

/** The instant `span` before `time`: the simulation's `UniverseTime::checked_sub`. */
export function timeBefore(time: UniverseTime, span: Span): UniverseTime {
  return subtract(time, span);
}

/** |`a` − `b`|, as the simulation's `checked_sub` then `checked_abs`. */
export function spanDistance(a: Span, b: Span): Span {
  const difference = subtract(a, b);
  if (difference.seconds >= 0) {
    return difference;
  }
  return difference.nanos === 0
    ? { seconds: -difference.seconds, nanos: 0 }
    : { seconds: -difference.seconds - 1, nanos: NANOS_PER_SECOND - difference.nanos };
}

/** Orders two spans by value: negative, zero or positive as `a` is less than, equal to or above `b`. */
export function compareSpans(a: Span, b: Span): number {
  return a.seconds === b.seconds ? a.nanos - b.nanos : a.seconds - b.seconds;
}

/** The span as a float of seconds: the simulation's `Span::as_seconds_f64`, lossy beyond 2⁵³ ns. */
export function spanSeconds(span: Span): number {
  return span.seconds + span.nanos * 1e-9;
}

/** `time` less `earlier`, s, as a float: for motion over a frame's elapsed time, not for clocks. */
export function secondsBetween(time: UniverseTime, earlier: UniverseTime): number {
  return time.seconds - earlier.seconds + (time.nanos - earlier.nanos) * 1e-9;
}
