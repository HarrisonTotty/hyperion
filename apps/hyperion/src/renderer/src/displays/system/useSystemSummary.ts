import type {
  SystemIdHex,
  SystemSummaryDto,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";
import { useState } from "react";

import { NOW_MODE, type QueryMode } from "../../lib/galaxy/model";
import { toQueryModeDto } from "../../lib/galaxy/wire";
import { type SystemModelResult, toSummaryRequest, toSystemModel } from "../../lib/system/wire";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";

/** The system a `SYSTEM` display asks about. */
export interface SummaryTarget {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  /** The system's designation of record, which its hosts' designations extend. */
  readonly designation: string;
}

/** Where a system's summary request stands, and the answer the display is drawn from. */
export interface SystemSummaryQuery {
  readonly state: RequestState<"system_summary">;
  /**
   * The latest answer for the target, kept while a newer request is pending or has failed, so that
   * a step of the display time never empties the display; `null` until the first answer.
   */
  readonly shown: SystemModelResult | null;
  /**
   * The mode the answer on show was answered in (plan 12, P12.T6.b): the one it was asked in where
   * it carries its observation, and `now` where it does not, as from a server that ignores the
   * mode; `null` with no answer on show.
   */
  readonly shownMode: QueryMode | null;
  /**
   * When the light the answer on show describes left the system, for an answer in observed mode,
   * whose stars are as they were then; `null` for one in `now`, and with no answer on show.
   */
  readonly shownEmitted: UniverseTime | null;
}

/** The answer on show, with what it was built from, so that it is built once per response. */
interface Shown {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  readonly response: SystemSummaryDto;
  readonly result: SystemModelResult;
  /** The mode the request it answers was asked in. */
  readonly asked: QueryMode;
}

/**
 * Asks the server for every star of one system at one time (`system_summary`), and keeps the
 * answer the `SYSTEM` display draws (plan 14, P14.T41.a; plan 06, P06.T36).
 *
 * @remarks
 * A thin layer over `useServerRequest`, whose `RequestChannel` makes the latest request win: a new
 * time, system or mode supersedes the request in flight and cancels it on the server, and the same
 * body built again asks nothing. The answer stays on show while a newer request is pending or has
 * failed, as the chart's does (plan 05, design note D4), and is dropped for another system or
 * universe. Each answer is turned into the display's model once, and a model the display cannot
 * use is a fault, not an exception. When the time moves is `useSystemData`'s to decide (D18); with
 * no target nothing is asked and the state is `idle`.
 *
 * The `GALAXY` display's readout asks in its chart's mode (plan 12, P12.T11), so that the stars it
 * shows are as the chart's observer sees them; the `SYSTEM` display asks in `now`. An answer is only
 * `ok` for the body asked now, so the mode it was asked in is kept with it, and it says which mode
 * it answers in: an answer without its observation is the present's, whatever was asked.
 *
 * @param time - The instant to ask about, within the clock window.
 * @param generation - Changing it asks again, which is what `RETRY` does after a failure.
 * @param mode - The mode to ask in; `now` when absent.
 */
export function useSystemSummary(
  target: SummaryTarget | null,
  time: UniverseTime,
  generation: number,
  mode: QueryMode = NOW_MODE,
): SystemSummaryQuery {
  const body =
    target === null
      ? null
      : toSummaryRequest(target.universe, target.system, time, toQueryModeDto(mode));
  const state = useServerRequest<"system_summary">(body, REQUEST_TIMEOUT_MS, generation);
  const [shown, setShown] = useState<Shown | null>(null);

  // Adjusted during render, not in an effect, so that the display draws an answer in the render it
  // arrives in, as the chart's range query does.
  let current = shown;
  const same =
    target !== null && current?.universe === target.universe && current.system === target.system;
  if (state.kind === "ok" && target !== null && current?.response !== state.response) {
    current = {
      universe: target.universe,
      system: target.system,
      response: state.response,
      result: toSystemModel(state.response, target.designation),
      asked: mode,
    };
    setShown(current);
  } else if (current !== null && !same) {
    current = null;
    setShown(null);
  }

  // Only the observation's presence and its emitted time are read here; its unit-suffixed fields
  // stay the wire adapters' (plan 05, design note D8).
  const observation = current?.response.observed;
  let shownMode: QueryMode | null = null;
  if (current !== null) {
    shownMode = observation === undefined ? NOW_MODE : current.asked;
  }
  return {
    state,
    shown: current?.result ?? null,
    shownMode,
    shownEmitted: observation?.emitted ?? null,
  };
}
