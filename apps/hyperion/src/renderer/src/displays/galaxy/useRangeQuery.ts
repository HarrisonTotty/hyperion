import type { MassLayer, SystemsInRange, UniverseIdHex } from "@hyperion/protocol";
import { useState } from "react";

import type { CentreLy, ChartResult } from "../../lib/galaxy/model";
import { toChartResult, toRangeRequest } from "../../lib/galaxy/wire";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";

/** What a chart asks the server for. */
export interface RangeQueryInput {
  /** The open universe, or `null` when none is. */
  readonly universe: UniverseIdHex | null;
  /** The chart centre, or `null` before one is chosen. */
  readonly centreLy: CentreLy | null;
  readonly queryRadiusLy: number;
  /** The lightest mass layer wanted, the chart's mass floor. */
  readonly minLayer: MassLayer;
  /** The chart time, in years from the epoch. */
  readonly timeYr: number;
  /** Changing it asks the same query again, which is what `RETRY` does after a failure. */
  readonly generation: number;
}

/** Where a chart's range query stands, and the answer the chart is drawn from. */
export interface RangeQuery {
  readonly state: RequestState<"systems_in_range">;
  /**
   * The latest answer for the open universe, kept while a newer query is pending or has failed, so
   * that the chart is never emptied by a query in flight; `null` until the first answer.
   */
  readonly shown: ChartResult | null;
}

/** The answer on show, with what it was built from, so that it is built once per response. */
interface Shown {
  readonly universe: UniverseIdHex;
  readonly response: SystemsInRange;
  readonly result: ChartResult;
}

/**
 * Asks the server for the systems around a chart's centre, and keeps the answer the chart draws.
 *
 * @remarks
 * The body is built by `toRangeRequest` and compared by value, so a chart that re-renders asks
 * nothing again; a changed radius, floor, time or centre supersedes the query in flight and cancels
 * it on the server. The answer stays on show while a newer query is pending or has failed (plan 05,
 * design note D4: data already shown is a snapshot, not a live value), and is dropped when another
 * universe is opened. Without a centre nothing is asked and the state is `idle`.
 */
export function useRangeQuery({
  universe,
  centreLy,
  queryRadiusLy,
  minLayer,
  timeYr,
  generation,
}: RangeQueryInput): RangeQuery {
  const body =
    universe === null || centreLy === null
      ? null
      : toRangeRequest(universe, centreLy, queryRadiusLy, timeYr, minLayer);
  const state = useServerRequest<"systems_in_range">(body, REQUEST_TIMEOUT_MS, generation);
  const [shown, setShown] = useState<Shown | null>(null);

  // Adjusted during render, not in an effect: the chart draws the answer in the render it arrives
  // in, and the adapter, which sorts every system, runs once per response.
  let current = shown;
  if (state.kind === "ok" && universe !== null && shown?.response !== state.response) {
    current = { universe, response: state.response, result: toChartResult(state.response) };
    setShown(current);
  } else if (current !== null && current.universe !== universe) {
    current = null;
    setShown(null);
  }

  return { state, shown: current?.result ?? null };
}
