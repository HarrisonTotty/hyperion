/**
 * The rows of the `SYSTEM` display's events list (plan 14, P14.T43.c): each event's time in the
 * chart's time system, its countdown from the display time, its kind in words and its body.
 */
import type { UniverseTime } from "@hyperion/protocol";

import { formatCountdown, TIME_SYSTEM_LABEL } from "../../lib/format";
import type { EventKind, SystemEvent } from "../../lib/system/eventsWire";
import { formatEventTime } from "./displayTime";
import { compareTimes } from "./requestTime";

/** One row of the events list. */
export interface EventRow {
  /** The event's ID, unique in the system. */
  readonly id: string;
  /** Its time with its time system's label, in the display time's form: `UT +12 yr 183/14:08:33`. */
  readonly time: string;
  /** Its countdown from the display time: `T-` before it, `T+` after it. */
  readonly countdown: string;
  /** Its kind, in words. */
  readonly kind: string;
  /** That body's designation, or `null` where the list of bodies on show does not hold it. */
  readonly body: string | null;
}

/** An event's kind in words, as the list's `EVENT` column reads it. */
export function eventKindLabel(kind: EventKind): string {
  let label: string;
  switch (kind) {
    case "impact":
      label = "IMPACT";
      break;
    case "eruption":
      label = "ERUPTION";
      break;
    case "storm":
      label = "GREAT STORM";
      break;
    case "dust_storm":
      label = "DUST STORM";
      break;
    case "comet":
      label = "COMET";
      break;
  }
  return label;
}

/**
 * The row of `event` at the display time `now`, its body's designation read from `designations`
 * by its ID.
 */
export function eventRow(
  event: SystemEvent,
  designations: ReadonlyMap<string, string>,
  now: UniverseTime,
): EventRow {
  return {
    id: event.id,
    time: `${TIME_SYSTEM_LABEL} ${formatEventTime(event.time)}`,
    countdown: formatCountdown(now, event.time),
    kind: eventKindLabel(event.kind),
    body: designations.get(event.body) ?? null,
  };
}

/**
 * The index of the first event at or after `now`, the next to come, which the list opens on; the
 * last event's where every one has passed; `-1` for no events.
 */
export function nextEventIndex(events: ReadonlyArray<SystemEvent>, now: UniverseTime): number {
  const next = events.findIndex((event) => compareTimes(event.time, now) >= 0);
  return next >= 0 ? next : events.length - 1;
}

/**
 * A span of universe time as one reading in the guide's span form, each end in the display time's
 * form, the label once: `UT -50 yr 000/00:00:00 – +50 yr 000/00:00:00`. The display steps finer
 * than 0.01 yr, so its times take the long form, the span's ends as its rows' do.
 */
export function formatTimeSpan(from: UniverseTime, to: UniverseTime): string {
  return `${TIME_SYSTEM_LABEL} ${formatEventTime(from)} – ${formatEventTime(to)}`;
}
