import type { ResponseFor, SystemIdHex, UniverseIdHex, UniverseTime } from "@hyperion/protocol";
import { useState } from "react";

import {
  type SystemEventsResult,
  toEventsRequest,
  toSystemEvents,
} from "../../lib/system/eventsWire";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";
import { compareTimes } from "./requestTime";
import type { SummaryTarget } from "./useSystemSummary";

/** Where a system's events request stands, and the answer the events list shows. */
export interface BodyEventsQuery {
  readonly state: RequestState<"body_events">;
  /**
   * The latest answer for the target, kept while a newer request is pending or has failed; `null`
   * until the first.
   */
  readonly shown: SystemEventsResult | null;
  /**
   * Whether the server has answered that it does not serve `body_events`, as one built before
   * P14.T36.b does, and one of a generator version before 25 (P14.T31.c's hold), and has sent no
   * events for the target since.
   */
  readonly unserved: boolean;
}

/** What the hook holds for its target: the answer on show, the time it was centred on. */
interface Held {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  readonly centre: UniverseTime;
  readonly response: ResponseFor<"body_events"> | null;
  readonly result: SystemEventsResult | null;
  readonly unserved: boolean;
}

/**
 * Whether `time` lies outside the middle half of the span from `from` to `to`, by whole seconds:
 * the part an answer centred on a time holds on both sides of it.
 */
function outsideMiddleHalf(time: UniverseTime, from: UniverseTime, to: UniverseTime): boolean {
  const quarter = Math.floor((to.seconds - from.seconds) / 4);
  return time.seconds < from.seconds + quarter || time.seconds > to.seconds - quarter;
}

/**
 * Asks the server for the events of the century around the display time (`body_events`), and keeps
 * the answer the events list shows (plan 14, P14.T43.c).
 *
 * @remarks
 * The century is asked about the display time once, and again, about the display time then, once
 * the display time leaves the middle half of the span the answer on show lists: a quarter-century
 * either way for a century listed whole, less where the server lists a busier system's events only
 * about the window's middle. So a step of the display time costs no request, and the list always
 * holds the events either side of it. The answer stays on show while a newer request is pending or
 * has failed, and is dropped for another system or universe; it is read once, and one the display
 * cannot use is a fault, not an exception. An `unsupported` answer, which a server built before
 * P14.T36.b gives, and one of a generator version before 25 (P14.T31.c's hold), is not a fault:
 * the list says the events are not yet modelled.
 *
 * @param displayTime - The display's own time, which the operator steps.
 * @param generation - Changing it asks again, which is what `RETRY` does after a failure.
 */
export function useBodyEvents(
  target: SummaryTarget | null,
  displayTime: UniverseTime,
  generation: number,
): BodyEventsQuery {
  const [centre, setCentre] = useState(displayTime);
  const body = target === null ? null : toEventsRequest(target.universe, target.system, centre);
  const state = useServerRequest<"body_events">(body, REQUEST_TIMEOUT_MS, generation);
  const [held, setHeld] = useState<Held | null>(null);

  // Adjusted during render, as `useSystemBodies` adjusts its own, so that an answer shows in the
  // render it arrives in; each branch leaves the state it sets unchanged on the next render.
  let current = held;
  const same =
    target !== null && current?.universe === target.universe && current.system === target.system;
  if (current !== null && !same) {
    current = null;
    setHeld(null);
  }
  if (target !== null) {
    if (state.kind === "ok" && current?.response !== state.response) {
      current = {
        universe: target.universe,
        system: target.system,
        centre,
        response: state.response,
        result: toSystemEvents(state.response, target.system),
        unserved: false,
      };
      setHeld(current);
    } else if (
      state.kind === "rejected" &&
      state.code === "unsupported" &&
      (current?.result ?? null) === null &&
      current?.unserved !== true
    ) {
      current = {
        universe: target.universe,
        system: target.system,
        centre,
        response: null,
        result: null,
        unserved: true,
      };
      setHeld(current);
    }
  }

  // Until the system is known nothing is asked, so the century waits on the display time; once it
  // is, it is centred again only on an answer to the centre asked about, so that a request in
  // flight is not superseded by the next step of the display time.
  const answered = current?.result?.kind === "ok" ? current.result.events : null;
  if (target === null && compareTimes(displayTime, centre) !== 0) {
    setCentre(displayTime);
  } else if (
    answered !== null &&
    current !== null &&
    compareTimes(current.centre, centre) === 0 &&
    compareTimes(displayTime, centre) !== 0 &&
    outsideMiddleHalf(displayTime, answered.listedFrom, answered.listedTo)
  ) {
    setCentre(displayTime);
  }

  return { state, shown: current?.result ?? null, unserved: current?.unserved ?? false };
}
