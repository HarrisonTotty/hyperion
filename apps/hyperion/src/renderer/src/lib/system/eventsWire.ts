/**
 * The adapter between the `body_events` messages and the client's model of a system's events
 * (plan 14, P14.T31, T36.b and T43.c).
 *
 * @remarks
 * The only module outside `test/` that reads the events' wire fields. The wire is decoded but not
 * validated, so an answer the display could not read is reported as a fault in words rather than
 * thrown from a render. The events keep the server's order, by time and then by event ID.
 */
import {
  type BodyEventDto,
  type BodyIdHex,
  type RequestOf,
  type ResponseFor,
  SECONDS_PER_JULIAN_YEAR,
  type SystemIdHex,
  type UniverseIdHex,
  type UniverseTime,
} from "@hyperion/protocol";

import { CLOCK_WINDOW_YR } from "../galaxy/model";
import { usableTime } from "./bodiesWire";

/** Half the span the display asks about, in Julian years: the century around a time (P14.T43.c). */
export const EVENTS_HALF_SPAN_YR = 50;

/** The clock window's half-width, in seconds: every time a request names lies inside it. */
const CLOCK_WINDOW_S = CLOCK_WINDOW_YR * SECONDS_PER_JULIAN_YEAR;

/** The kinds of event, as the wire tags them. */
export type EventKind = BodyEventDto["type"];

/** One event as the display lists it. */
export interface SystemEvent {
  /** The event's ID, plan 01's text form: unique in the system. */
  readonly id: string;
  readonly kind: EventKind;
  /** The body it falls on: the halo, for a comet. */
  readonly body: BodyIdHex;
  /** An impact's instant, an eruption's or a storm's onset, or a comet's perihelion passage. */
  readonly time: UniverseTime;
}

/** A system's events in a window, as the display lists them. */
export interface SystemEvents {
  /** The window asked about, `from` inclusive and `to` exclusive. */
  readonly from: UniverseTime;
  readonly to: UniverseTime;
  /**
   * The span every event of which is listed: the window, or the part about its middle that an
   * answer holds where the window holds more events than one answer lists.
   */
  readonly listedFrom: UniverseTime;
  readonly listedTo: UniverseTime;
  /** The events of the listed span, by time and then by ID. */
  readonly events: ReadonlyArray<SystemEvent>;
}

/** A system's events as the display can use them, or why not. */
export type SystemEventsResult =
  | { readonly kind: "ok"; readonly events: SystemEvents }
  | { readonly kind: "fault"; readonly fault: string };

/** `time` moved by `seconds`, held inside the clock window. */
function heldInWindow(time: UniverseTime, seconds: number): UniverseTime {
  const moved = time.seconds + seconds;
  if (moved >= CLOCK_WINDOW_S) {
    return { seconds: CLOCK_WINDOW_S, nanos: 0 };
  }
  if (moved < -CLOCK_WINDOW_S) {
    return { seconds: -CLOCK_WINDOW_S, nanos: 0 };
  }
  return { seconds: moved, nanos: time.nanos };
}

/**
 * The `body_events` request for the century around `centre`: from 50 Julian years before it to 50
 * after, each end held inside the clock window, which every request's times lie in.
 */
export function toEventsRequest(
  universe: UniverseIdHex,
  system: SystemIdHex,
  centre: UniverseTime,
): RequestOf<"body_events"> {
  const half = EVENTS_HALF_SPAN_YR * SECONDS_PER_JULIAN_YEAR;
  return {
    kind: "body_events",
    universe,
    system,
    from: heldInWindow(centre, -half),
    to: heldInWindow(centre, half),
  };
}

/** The time at which `event` falls, by its kind's field. */
function timeOf(event: BodyEventDto): UniverseTime {
  let time: UniverseTime;
  switch (event.type) {
    case "impact":
      time = event.time;
      break;
    case "eruption":
    case "storm":
    case "dust_storm":
      time = event.onset;
      break;
    case "comet":
      time = event.perihelion_time;
      break;
  }
  return time;
}

/** The body `event` falls on: the halo, for a comet. */
function bodyOf(event: BodyEventDto): BodyIdHex {
  return event.type === "comet" ? event.halo : event.body;
}

/** Whether `a` is before `b`. */
function before(a: UniverseTime, b: UniverseTime): boolean {
  return a.seconds === b.seconds ? a.nanos < b.nanos : a.seconds < b.seconds;
}

/**
 * The answer of the system `system` as the display lists it, or a fault in words: an answer for
 * another system, a time the client cannot compute with, a listed span outside the window, or an
 * event outside the span or out of order.
 */
export function toSystemEvents(
  response: ResponseFor<"body_events">,
  system: SystemIdHex,
): SystemEventsResult {
  if (response.system !== system) {
    return { kind: "fault", fault: `answer for system ${response.system}` };
  }
  const { from, to, listed_from: listedFrom, listed_to: listedTo } = response;
  const events: SystemEvent[] = response.events.map((event) => ({
    id: event.id,
    kind: event.type,
    body: bodyOf(event),
    time: timeOf(event),
  }));
  if (![from, to, listedFrom, listedTo, ...events.map((event) => event.time)].every(usableTime)) {
    return { kind: "fault", fault: "event time unusable" };
  }
  if (before(listedFrom, from) || before(to, listedTo) || before(listedTo, listedFrom)) {
    return { kind: "fault", fault: "listed span outside the window" };
  }
  if (!events.every((event) => !before(event.time, listedFrom) && before(event.time, listedTo))) {
    return { kind: "fault", fault: "event outside the listed span" };
  }
  const ordered = events.every((event, index) => {
    const previous = events[index - 1];
    return previous === undefined || !before(event.time, previous.time);
  });
  if (!ordered) {
    return { kind: "fault", fault: "events out of order" };
  }
  return { kind: "ok", events: { from, to, listedFrom, listedTo, events } };
}
