import type { ResponseFor, SystemIdHex, UniverseIdHex, UniverseTime } from "@hyperion/protocol";
import { useState } from "react";

import {
  type SystemBodiesResult,
  toBodiesRequest,
  toSystemBodiesModel,
} from "../../lib/system/bodiesWire";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";
import type { SummaryTarget } from "./useSystemSummary";

/** Where a system's bodies request stands, and the answer the display is drawn from. */
export interface SystemBodiesQuery {
  readonly state: RequestState<"system_bodies">;
  /**
   * The latest answer for the target, kept while a newer request is pending or has failed, as the
   * summary's is; `null` until the first answer.
   */
  readonly shown: SystemBodiesResult | null;
  /**
   * Whether the server has answered that it does not serve `system_bodies`, and has sent no bodies
   * for the target since: the transitional state of the orchestrator's ruling 59.1, in which the
   * hosts show from `system_summary` alone.
   */
  readonly unserved: boolean;
}

/** What the hook holds for its target: the answer on show, and whether the kind is unserved. */
interface Held {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  readonly response: ResponseFor<"system_bodies"> | null;
  readonly result: SystemBodiesResult | null;
  readonly unserved: boolean;
}

/**
 * Asks the server for every body of one system at one time (`system_bodies`), and keeps the
 * answer the `SYSTEM` display draws (plan 14, P14.T41.a and design note D18).
 *
 * @remarks
 * A thin layer over `useServerRequest`, beside `useSystemSummary` and asked at the same time: the
 * latest request wins, and the same body built again asks nothing, so that `useSystemData` decides
 * when to ask again. The answer stays on show while a newer request is pending or has failed and is
 * dropped for another system or universe; it is turned into the display's model once, and one the
 * display cannot use is a fault, not an exception. An `unsupported` answer, which a server built
 * before P14.T36 gives, is not shown as a fault: the display carries on with the hosts alone and
 * says the bodies are not yet modelled, until the server sends some.
 *
 * @param time - The instant to ask about, within the clock window.
 * @param generation - Changing it asks again, which is what `RETRY` does after a failure.
 */
export function useSystemBodies(
  target: SummaryTarget | null,
  time: UniverseTime,
  generation: number,
): SystemBodiesQuery {
  const body = target === null ? null : toBodiesRequest(target.universe, target.system, time);
  const state = useServerRequest<"system_bodies">(body, REQUEST_TIMEOUT_MS, generation);
  const [held, setHeld] = useState<Held | null>(null);

  // Adjusted during render, not in an effect, so that the display draws an answer in the render it
  // arrives in; each branch leaves the state it sets unchanged on the next render.
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
        response: state.response,
        result: toSystemBodiesModel(state.response, target.designation),
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
        response: null,
        result: null,
        unserved: true,
      };
      setHeld(current);
    }
  }

  return { state, shown: current?.result ?? null, unserved: current?.unserved ?? false };
}
