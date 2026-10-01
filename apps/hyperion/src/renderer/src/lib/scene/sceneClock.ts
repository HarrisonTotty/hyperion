/**
 * The scene time a client renders at (rendering plan R03, Design note 9).
 *
 * @remarks
 * Every push states the scene time and rate. The client keeps the latest with the
 * `performance.now()` at which it arrived and renders at t + (now − received) × rate, so that two
 * clients of one scene differ by the delivery of the latest push, the brainstorm's bound. The sum
 * is formed in whole seconds and nanoseconds: a `UniverseTime` a thousand years out keeps its
 * nanoseconds, where an `f64` of seconds from the epoch would resolve only about 4 µs.
 */
import type { UniverseTime } from "@hyperion/protocol";
import { SECONDS_PER_JULIAN_YEAR } from "@hyperion/protocol";

import { CLOCK_WINDOW_YR } from "../galaxy/model";
import type { SceneClock } from "./model";

const NANOS_PER_SECOND = 1_000_000_000;
const NANOS_PER_MILLISECOND = 1_000_000;
const MILLISECONDS_PER_SECOND = 1_000;

/** The clock window's edge, H, in whole seconds from the epoch: 1,000 Julian years. */
const CLOCK_WINDOW_S = CLOCK_WINDOW_YR * SECONDS_PER_JULIAN_YEAR;

/**
 * `time` advanced by `elapsedMs` of real time at `rate` scene seconds per real second.
 *
 * @remarks
 * The whole milliseconds times the rate are an exact integer of scene milliseconds, split into
 * seconds and nanoseconds exactly; only the sub-millisecond part, at most 10⁵ scene milliseconds,
 * is rounded, to the nanosecond.
 */
function advance(time: UniverseTime, elapsedMs: number, rate: number): UniverseTime {
  const wholeMs = Math.floor(elapsedMs);
  const scaledMs = wholeMs * rate;
  const scaledSeconds = Math.floor(scaledMs / MILLISECONDS_PER_SECOND);
  const nanos =
    time.nanos +
    (scaledMs - scaledSeconds * MILLISECONDS_PER_SECOND) * NANOS_PER_MILLISECOND +
    Math.round((elapsedMs - wholeMs) * rate * NANOS_PER_MILLISECOND);
  const carry = Math.floor(nanos / NANOS_PER_SECOND);
  return {
    seconds: time.seconds + scaledSeconds + carry,
    nanos: nanos - carry * NANOS_PER_SECOND,
  };
}

/** `time` held within the clock window, ±H inclusive, where the server's clock stops. */
function withinWindow(time: UniverseTime): UniverseTime {
  if (time.seconds >= CLOCK_WINDOW_S) {
    return { seconds: CLOCK_WINDOW_S, nanos: 0 };
  }
  if (time.seconds < -CLOCK_WINDOW_S) {
    return { seconds: -CLOCK_WINDOW_S, nanos: 0 };
  }
  return time;
}

/**
 * The scene time to render at: the clock's time plus the real time since its push arrived, times
 * its rate.
 *
 * @remarks
 * A paused clock, or one held at the window's limit, does not advance. A running one advances no
 * further than the clock window's edge, where the server's clock stops, and never backwards for a
 * `nowMs` before `receivedMs`.
 *
 * @param receivedMs - The `performance.now()` at which the push stating `clock` arrived, ms.
 * @param nowMs - The `performance.now()` of the frame being rendered, ms.
 */
export function renderTime(clock: SceneClock, receivedMs: number, nowMs: number): UniverseTime {
  if (clock.state !== "running" || clock.rate === 0) {
    return clock.time;
  }
  return withinWindow(advance(clock.time, Math.max(0, nowMs - receivedMs), clock.rate));
}
