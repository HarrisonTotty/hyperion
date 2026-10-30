import type { BlackHoleState, UniverseIdHex } from "@hyperion/protocol";
import { useState } from "react";

import { toBlackHole, toBlackHoleRequest } from "../../lib/galaxy/featureWire";
import type { BlackHoleModel } from "../../lib/galaxy/model";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";

/** Where the central black hole's state stands, and the answer kept for it. */
export interface BlackHoleQuery {
  readonly state: RequestState<"black_hole_state">;
  /** The latest answer for the open universe, or `null` before it, while not asked, or for another. */
  readonly shown: BlackHoleModel | null;
}

/** The answer kept, with the universe and the response it was built from. */
interface Kept {
  readonly universe: UniverseIdHex;
  readonly response: BlackHoleState;
  readonly blackHole: BlackHoleModel;
}

/**
 * Asks for the central black hole's state at the chart's time (`black_hole_state`, plan 09,
 * P09.T41.c) while the galactic centre is selected, for its readout's `BLACK HOLE` readings.
 *
 * @param universe - The open universe, or `null` while nothing is asked: none open, or the centre
 *   not selected.
 * @param timeYr - The time of the chart on show, in years from the epoch.
 * @param generation - Changing it asks again, which is what `RETRY` does.
 */
export function useBlackHoleState(
  universe: UniverseIdHex | null,
  timeYr: number,
  generation: number,
): BlackHoleQuery {
  const body = universe === null ? null : toBlackHoleRequest(universe, timeYr);
  const state = useServerRequest<"black_hole_state">(body, REQUEST_TIMEOUT_MS, generation);
  const [kept, setKept] = useState<Kept | null>(null);

  // Adjusted during render, as the chart's answers are; asked for nothing, nothing is kept.
  let current = kept;
  if (state.kind === "ok" && universe !== null && kept?.response !== state.response) {
    current = { universe, response: state.response, blackHole: toBlackHole(state.response) };
    setKept(current);
  } else if (current !== null && current.universe !== universe) {
    current = null;
    setKept(null);
  }
  return { state, shown: current?.blackHole ?? null };
}
