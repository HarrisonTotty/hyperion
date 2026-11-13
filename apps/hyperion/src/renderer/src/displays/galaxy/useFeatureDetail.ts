import type { FeatureDetail, FeatureIdHex, UniverseIdHex } from "@hyperion/protocol";
import { useState } from "react";

import { toFeatureDetail, toFeatureDetailRequest } from "../../lib/galaxy/featureWire";
import type { FeatureDetailModel } from "../../lib/galaxy/model";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";

/** What the selected feature's detail is asked for. */
export interface FeatureDetailInput {
  /** The open universe, or `null` when none is. */
  readonly universe: UniverseIdHex | null;
  /** The selected feature, or `null` while none is. */
  readonly feature: FeatureIdHex | null;
  /** The time of the chart on show, in years from the epoch. */
  readonly timeYr: number;
  /** Changing it asks the same question again, which is what `RETRY` does. */
  readonly generation: number;
}

/** Where the selected feature's detail stands, and the answer kept for it. */
export interface FeatureDetailQuery {
  readonly state: RequestState<"feature_detail">;
  /**
   * The latest answer for the selected feature, kept while a newer one for it, at another time, is
   * pending or has failed; `null` before the first answer and for another feature.
   */
  readonly shown: FeatureDetailModel | null;
}

/** The answer kept, with the universe and the response it was built from. */
interface Kept {
  readonly universe: UniverseIdHex;
  readonly response: FeatureDetail;
  readonly detail: FeatureDetailModel;
}

/**
 * Asks for the selected feature's structure at the chart's time (`feature_detail`, plan 09,
 * P09.T41.c), as `useSystemSummary` asks for a selected system's stars.
 *
 * @remarks
 * A new selection or time supersedes the question in flight. The answer stays while a newer one
 * for the same feature is awaited, and goes when another feature is selected or another universe
 * opened, so that one feature's structure is never read beside another's summary.
 */
export function useFeatureDetail({
  universe,
  feature,
  timeYr,
  generation,
}: FeatureDetailInput): FeatureDetailQuery {
  const body =
    universe === null || feature === null
      ? null
      : toFeatureDetailRequest(universe, feature, timeYr);
  const state = useServerRequest<"feature_detail">(body, REQUEST_TIMEOUT_MS, generation);
  const [kept, setKept] = useState<Kept | null>(null);

  // Adjusted during render, as the chart's answers are.
  let current = kept;
  if (state.kind === "ok" && universe !== null && kept?.response !== state.response) {
    current = { universe, response: state.response, detail: toFeatureDetail(state.response) };
    setKept(current);
  } else if (current !== null && (current.universe !== universe || current.detail.id !== feature)) {
    current = null;
    setKept(null);
  }
  return { state, shown: current?.detail ?? null };
}
