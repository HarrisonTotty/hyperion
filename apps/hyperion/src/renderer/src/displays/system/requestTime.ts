/**
 * When the `SYSTEM` display asks the server again as its time moves (plan 14, design note D18).
 *
 * @remarks
 * The display draws by propagating the orbits it was sent, but every number in a readout is the
 * server's, and the server's answer holds only near the time it was asked for. So the display asks
 * again, at its own time, once that time has moved more than a year from the time it last asked
 * for, or past an instant at which something the answer describes changes: a star's death, or, for
 * a system not yet formed, its birth, or the first instant past a body's orbit's
 * `valid_until`, the last at which its elements hold.
 */
import { SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";

/**
 * How far the display time may move from the time last asked for before it is asked again, in
 * seconds: one Julian year (D18).
 */
export const REQUEST_AGAIN_AFTER_S = SECONDS_PER_JULIAN_YEAR;

/** Whether `a` is before, at or after `b`: negative, zero or positive. */
export function compareTimes(a: UniverseTime, b: UniverseTime): number {
  return a.seconds === b.seconds ? a.nanos - b.nanos : a.seconds - b.seconds;
}

/**
 * Whether `to` lies more than {@link REQUEST_AGAIN_AFTER_S} from `from`, either way: decided on the
 * whole seconds and then the nanoseconds, with no float of seconds.
 */
function movedTooFar(from: UniverseTime, to: UniverseTime): boolean {
  const seconds = to.seconds - from.seconds;
  const nanos = to.nanos - from.nanos;
  const limit = REQUEST_AGAIN_AFTER_S;
  return (
    seconds > limit ||
    (seconds === limit && nanos > 0) ||
    seconds < -limit ||
    (seconds === -limit && nanos < 0)
  );
}

/** Whether `change` lies between `from` and `to`: one is before it and the other at or after it. */
function crosses(from: UniverseTime, to: UniverseTime, change: UniverseTime): boolean {
  return compareTimes(from, change) < 0 !== compareTimes(to, change) < 0;
}

/**
 * The time to ask the server for, given the time last asked for and the display's time now.
 *
 * @param changesAt - Instants at which what the last answer describes changes, as a star's death.
 * @returns `displayTime` when it has moved more than a year from `requested` or across one of
 *   `changesAt`; `requested` itself otherwise, so that the request is the same by value and is
 *   not sent again.
 */
export function nextRequestTime(
  requested: UniverseTime,
  displayTime: UniverseTime,
  changesAt: ReadonlyArray<UniverseTime>,
): UniverseTime {
  const again =
    movedTooFar(requested, displayTime) ||
    changesAt.some((change) => crosses(requested, displayTime, change));
  return again ? displayTime : requested;
}
