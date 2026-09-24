import type { BodyIdHex, ResponseFor, UniverseTime } from "@hyperion/protocol";
import { useState } from "react";

import {
  type BodyDetailResult,
  toBodyDetail,
  toBodyDetailRequest,
} from "../../lib/system/bodiesWire";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";
import type { SummaryTarget } from "./useSystemSummary";

/** Where the selected body's record request stands, and the record on show. */
export interface BodyDetailQuery {
  readonly state: RequestState<"body_detail">;
  /**
   * The latest record of the body, kept while a newer request for it is pending or has failed;
   * `null` until the first, and again once another body is selected.
   */
  readonly shown: BodyDetailResult | null;
}

/** The record on show, with what it was built from, so that it is built once per response. */
interface Held {
  readonly body: BodyIdHex;
  readonly response: ResponseFor<"body_detail">;
  readonly result: BodyDetailResult;
}

/**
 * Asks the server for the whole record of the selected body (`body_detail`), for the readout (plan
 * 14, P14.T43.b).
 *
 * @remarks
 * Asked at the time the display last asked for its system, so that the record and the list are
 * of one instant and a step of the display time costs no request; the list's own entry for the body
 * is read until the record comes. A new selection supersedes the request in flight. With no body
 * selected, or a host, which has no record of its own, nothing is asked.
 *
 * @param bodyId - The selected body, or `null` for none.
 * @param time - The instant to ask about: the display's last request time.
 * @param generation - Changing it asks again, as `RETRY` does.
 */
export function useBodyDetail(
  target: SummaryTarget | null,
  bodyId: BodyIdHex | null,
  time: UniverseTime,
  generation: number,
): BodyDetailQuery {
  const request =
    target === null || bodyId === null ? null : toBodyDetailRequest(target.universe, bodyId, time);
  const state = useServerRequest<"body_detail">(request, REQUEST_TIMEOUT_MS, generation);
  const [held, setHeld] = useState<Held | null>(null);

  let current = held;
  if (current !== null && current.body !== bodyId) {
    current = null;
    setHeld(null);
  }
  if (
    target !== null &&
    bodyId !== null &&
    state.kind === "ok" &&
    current?.response !== state.response
  ) {
    current = {
      body: bodyId,
      response: state.response,
      result: toBodyDetail(state.response, target.system, target.designation),
    };
    setHeld(current);
  }

  return { state, shown: current?.result ?? null };
}
