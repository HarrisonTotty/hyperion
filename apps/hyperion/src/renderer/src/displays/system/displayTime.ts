/**
 * The `SYSTEM` display's own time: its steps, and the clock window it is held to (plan 14, D24 and
 * P14.T44.a).
 *
 * @remarks
 * Universe time, and not the ship's clock, which does not exist yet. Every step is a whole number of
 * seconds added to the instant's whole seconds, so no float of years ever carries it.
 */
import { SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";

import { TIME_SYSTEM_LABEL } from "../../lib/format";
import { CLOCK_WINDOW_YR } from "../../lib/galaxy/model";

/**
 * The label of the display's own time, before its value: `DISPLAY TIME UT` (plan 14, D24; the
 * owner's draft of the guide's time formats).
 */
export const DISPLAY_TIME_LABEL = `DISPLAY TIME ${TIME_SYSTEM_LABEL}`;

/** Room for the longest display time, `-1000 yr 000/00:00:00`, in characters. */
export const DISPLAY_TIME_WIDTH_CH = 21;

/** The clock window H in seconds: a display time lies within this of the epoch. */
export const CLOCK_WINDOW_S = CLOCK_WINDOW_YR * SECONDS_PER_JULIAN_YEAR;

const SECONDS_PER_HOUR = 3_600;
const SECONDS_PER_DAY = 86_400;

/** An amount the display time can be stepped by. */
export interface TimeStep {
  /** Its label, which is also its value with its unit: `1 h`, `10 d`, `100 yr`. */
  readonly label: string;
  /** Its length in seconds; a year is the Julian year of every `yr` on the wire. */
  readonly seconds: number;
  /** The single key that chooses it, shown on its control. */
  readonly key: string;
}

/** The steps the display time can be moved by, shortest first (P14.T44.a). */
export const TIME_STEPS: ReadonlyArray<TimeStep> = [
  { label: "1 h", seconds: SECONDS_PER_HOUR, key: "1" },
  { label: "1 d", seconds: SECONDS_PER_DAY, key: "2" },
  { label: "10 d", seconds: 10 * SECONDS_PER_DAY, key: "3" },
  { label: "1 yr", seconds: SECONDS_PER_JULIAN_YEAR, key: "4" },
  { label: "10 yr", seconds: 10 * SECONDS_PER_JULIAN_YEAR, key: "5" },
  { label: "100 yr", seconds: 100 * SECONDS_PER_JULIAN_YEAR, key: "6" },
];

/** The step a display opens with: a day. */
export const DEFAULT_TIME_STEP = 1;

/** Which edge of the clock window a time stands at, if either. */
export type ClockLimit = "start" | "end" | null;

/** The edge of the clock window `time` stands at: `-H` is the start and `+H` the end. */
export function clockLimitAt(time: UniverseTime): ClockLimit {
  if (time.seconds >= CLOCK_WINDOW_S) {
    return "end";
  }
  return time.seconds <= -CLOCK_WINDOW_S && time.nanos === 0 ? "start" : null;
}

/**
 * The time `deltaS` seconds after `time`, held to the clock window: a step that would leave it
 * stops at its edge, `±H` exactly.
 *
 * @param deltaS - A whole number of seconds, negative to step back.
 * @throws RangeError for a step that is not a whole number of seconds.
 */
export function stepTime(time: UniverseTime, deltaS: number): UniverseTime {
  if (!Number.isSafeInteger(deltaS)) {
    throw new RangeError(`a step of ${String(deltaS)} s is not a whole number of seconds`);
  }
  const seconds = time.seconds + deltaS;
  if (seconds > CLOCK_WINDOW_S || (seconds === CLOCK_WINDOW_S && time.nanos > 0)) {
    return { seconds: CLOCK_WINDOW_S, nanos: 0 };
  }
  if (seconds < -CLOCK_WINDOW_S) {
    return { seconds: -CLOCK_WINDOW_S, nanos: 0 };
  }
  return { seconds, nanos: time.nanos };
}
