import type { FeaturesInRange, UniverseIdHex } from "@hyperion/protocol";
import { useState } from "react";

import type { CentreLy, ChartFeatures } from "../../lib/galaxy/model";
import { toChartFeatures, toFeaturesRequest } from "../../lib/galaxy/featureWire";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";

/** What a chart asks the server about the features around its centre. */
export interface FeaturesInRangeInput {
  /** The open universe, or `null` when none is. */
  readonly universe: UniverseIdHex | null;
  /** The chart centre, or `null` before one is chosen. */
  readonly centreLy: CentreLy | null;
  readonly queryRadiusLy: number;
  /** The chart time, in years from the epoch. */
  readonly timeYr: number;
  /** Changing it asks the same query again, which is what `RETRY` does after a failure. */
  readonly generation: number;
}

/** Where a chart's features query stands, and the answer kept for it. */
export interface FeaturesInRangeQuery {
  readonly state: RequestState<"features_in_range">;
  /**
   * The latest answer for the open universe, kept while a newer query is pending or has failed;
   * `null` until the first answer.
   */
  readonly shown: ChartFeatures | null;
}

/** The answer on show, with what it was built from, so that it is built once per response. */
interface Shown {
  readonly universe: UniverseIdHex;
  readonly response: FeaturesInRange;
  readonly features: ChartFeatures;
}

/**
 * Asks the server for the large features around a chart's centre (`features_in_range`, plan 09,
 * P09.T41.a), and keeps the answer.
 *
 * @remarks
 * The range query's counterpart ({@link useRangeQuery}), asked beside it for the same centre,
 * radius and time: the body is built by `toFeaturesRequest` and compared by value, so a chart that
 * re-renders asks nothing again, and a changed radius, time or centre supersedes the query in
 * flight. The answer stays while a newer one is pending or has failed, and is dropped when another
 * universe is opened; the chart draws it only while it answers the chart's own centre, radius and
 * time. The features are the present's in every mode, as the systems are (plan 12, Design note
 * 4), so the mode asks nothing of it. Without a centre nothing is asked and the state is `idle`.
 */
export function useFeaturesInRange({
  universe,
  centreLy,
  queryRadiusLy,
  timeYr,
  generation,
}: FeaturesInRangeInput): FeaturesInRangeQuery {
  const body =
    universe === null || centreLy === null
      ? null
      : toFeaturesRequest(universe, centreLy, queryRadiusLy, timeYr);
  const state = useServerRequest<"features_in_range">(body, REQUEST_TIMEOUT_MS, generation);
  const [shown, setShown] = useState<Shown | null>(null);

  // Adjusted during render, as the range query's answer is, so that the features are drawn in the
  // render their answer arrives in and sorted once per response.
  let current = shown;
  if (state.kind === "ok" && universe !== null && shown?.response !== state.response) {
    current = { universe, response: state.response, features: toChartFeatures(state.response) };
    setShown(current);
  } else if (current !== null && current.universe !== universe) {
    current = null;
    setShown(null);
  }

  return { state, shown: current?.features ?? null };
}
