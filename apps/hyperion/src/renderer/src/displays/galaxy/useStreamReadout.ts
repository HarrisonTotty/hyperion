import type { UniverseIdHex } from "@hyperion/protocol";
import { useEffect, useState } from "react";

import {
  toGlobalFeaturesRequest,
  toGlobalList,
  toStreamTrack,
  toStreamTrackRequest,
} from "../../lib/galaxy/globalWire";
import type { GlobalListModel, StreamTrack } from "../../lib/galaxy/model";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";

/**
 * How long after a `pending` answer a stream's track is asked for again, in milliseconds: the
 * server builds an unheld table in about a second (plan 10, P10.T10.b), and holds it once built.
 */
export const TRACK_ASK_AGAIN_MS = 5_000;

/** What a stream member's readout reads of its stream, and where its requests stand. */
export interface StreamReadoutQuery {
  /** The global list, which gives the stream's mass and progenitor, or `null` until it answers. */
  readonly list: GlobalListModel | null;
  /** The stream's track, which gives its figures, or `null` until it is answered ready. */
  readonly track: StreamTrack | null;
  /**
   * How the requests stand, the list's first: the one to say beside the readout, `null` while
   * both have answered or neither was asked.
   */
  readonly state: RequestState<"global_features"> | RequestState<"stream_track"> | null;
}

/** Whether a request has not answered: pending, failed or held by the link. */
function unsettled(state: RequestState<"global_features"> | RequestState<"stream_track">): boolean {
  return state.kind !== "ok" && state.kind !== "idle";
}

/**
 * Asks for the stream `stream` of the open universe for its member's readout (plan 10, P10.T11):
 * the global list, for its mass and progenitor, and its track, for its length, width and
 * dispersion.
 *
 * @remarks
 * Asked while a stream's member is selected, and again when `generation` changes, as `RETRY` asks.
 * A track answered `pending`, which a server that has not built the stream's table may send, is
 * asked for again after {@link TRACK_ASK_AGAIN_MS} until it is ready. Each answer stays while a
 * newer request is pending or has failed, as `useServerRequest`'s do.
 */
export function useStreamReadout(
  universe: UniverseIdHex | null,
  stream: number | null,
  generation: number,
): StreamReadoutQuery {
  const [askedAgain, setAskedAgain] = useState(0);
  const asked = universe !== null && stream !== null;
  const listState = useServerRequest<"global_features">(
    asked ? toGlobalFeaturesRequest(universe) : null,
    REQUEST_TIMEOUT_MS,
    generation,
  );
  const trackState = useServerRequest<"stream_track">(
    asked ? toStreamTrackRequest(universe, stream) : null,
    REQUEST_TIMEOUT_MS,
    generation + askedAgain,
  );
  const answered = trackState.kind === "ok" ? trackState.response : null;
  const answer = answered === null ? null : toStreamTrack(answered);
  const pending = answer?.kind === "pending";
  // Each `pending` answer, a first or one asked again, schedules the next ask.
  useEffect(() => {
    if (!pending || answered === null) {
      return undefined;
    }
    const timer = setTimeout(() => {
      setAskedAgain((count) => count + 1);
    }, TRACK_ASK_AGAIN_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [pending, answered]);
  let state: StreamReadoutQuery["state"] = null;
  if (unsettled(listState)) {
    state = listState;
  } else if (unsettled(trackState)) {
    state = trackState;
  } else if (pending) {
    state = { kind: "pending" };
  }
  return {
    list: listState.kind === "ok" ? toGlobalList(listState.response) : null,
    track: answer?.kind === "ready" ? answer.track : null,
    state,
  };
}
