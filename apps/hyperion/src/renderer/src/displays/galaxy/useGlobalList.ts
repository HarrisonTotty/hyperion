import type { GlobalFeatures, UniverseIdHex } from "@hyperion/protocol";
import { useEffect, useState, useSyncExternalStore } from "react";

import { toGlobalFeaturesRequest, toGlobalList } from "../../lib/galaxy/globalWire";
import type { GlobalDwarfCore, GlobalListModel, StreamTrack } from "../../lib/galaxy/model";
import { useServerLink } from "../../lib/serverLink";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";
import { NO_TRACKS, StreamTrackFetcher, type StreamTracksSnapshot } from "./streamTrackFetcher";

/**
 * How often the global list is asked for again while any of its tracks is not on hand, in
 * milliseconds: the server warms a galaxy's tables in a minute or two after an open, and says which
 * it holds in each answer.
 */
export const LIST_REFRESH_MS = 10_000;

/** Where the galaxy's global list and its tracks stand, for the map's stream overlay. */
export interface GlobalListQuery {
  readonly state: RequestState<"global_features">;
  /**
   * The latest answer's list for the open universe, kept while a newer request is pending or has
   * failed, without any dwarf core that cannot be drawn, or `null` until the first answer.
   */
  readonly list: GlobalListModel | null;
  /** The tracks on hand, by stream number. */
  readonly tracks: ReadonlyMap<number, StreamTrack>;
  /** Why a track answered cannot be drawn, in words, or `null` when nothing is wrong. */
  readonly trackFault: string | null;
  /**
   * Why a dwarf core of the list on hand cannot be drawn, in words, or `null` when nothing is wrong:
   * such a core is left out of `list`.
   */
  readonly coreFault: string | null;
  /** Asks for the list again, as `RETRY` does. */
  readonly retry: () => void;
}

/** The answer on hand, with what it was built from, so that it is built once per response. */
interface Kept {
  readonly universe: UniverseIdHex;
  readonly response: GlobalFeatures;
  readonly list: GlobalListModel;
}

/** Whether a dwarf core can be drawn: its place and its extent, where sent, finite. */
function usableCore({ positionLy, extentLy }: GlobalDwarfCore): boolean {
  return (
    (positionLy === null || [positionLy.x, positionLy.y, positionLy.z].every(Number.isFinite)) &&
    (extentLy === null || (Number.isFinite(extentLy) && extentLy >= 0))
  );
}

/** `list` without the dwarf cores that cannot be drawn, and why, or `null` when every core can be. */
function withUsableCores(list: GlobalListModel): {
  readonly list: GlobalListModel;
  readonly fault: string | null;
} {
  const cores = list.cores.filter(usableCore);
  return cores.length === list.cores.length
    ? { list, fault: null }
    : { list: { ...list, cores }, fault: "core unusable" };
}

/** A store that holds nothing and never changes, for while no universe is open. */
const NO_STORE = {
  subscribe: (): (() => void) => () => undefined,
  getSnapshot: (): StreamTracksSnapshot => NO_TRACKS,
};

/**
 * Asks the server for the galaxy's global list (`global_features`, plan 10, P10.T11), and for the
 * track of every stream it holds one for (`stream_track`), for the galaxy map's stream overlay.
 *
 * @remarks
 * Asked once per open universe, by the page, since both views draw the same tracks. Each answer
 * says which streams' tables the server holds, which it warms after an open, so while any
 * stream's track is not on hand the list is asked for again every {@link LIST_REFRESH_MS}, and the
 * tracks it holds are fetched as they become ready, a few at a time, never one the server would
 * have to build for the asking. The list's answer stays while a newer request is pending or has
 * failed, and goes, with the tracks, when another universe is opened; a track not answered is
 * asked again with the next answer of the list, or when the link returns.
 */
export function useGlobalList(universe: UniverseIdHex | null): GlobalListQuery {
  const [generation, setGeneration] = useState(0);
  const body = universe === null ? null : toGlobalFeaturesRequest(universe);
  const state = useServerRequest<"global_features">(body, REQUEST_TIMEOUT_MS, generation);
  const [kept, setKept] = useState<Kept | null>(null);
  const { status, requests } = useServerLink();
  const connected = status === "connected";

  // Adjusted during render, as the map's features are, so that the overlay is drawn in the render
  // its answer arrives in.
  let current = kept;
  if (state.kind === "ok" && universe !== null && kept?.response !== state.response) {
    current = { universe, response: state.response, list: toGlobalList(state.response) };
    setKept(current);
  } else if (current !== null && current.universe !== universe) {
    current = null;
    setKept(null);
  }
  const checked = current === null ? null : withUsableCores(current.list);
  const list = checked?.list ?? null;

  // One fetcher a universe and link: a new one starts with no tracks.
  const [fetcher, setFetcher] = useState<StreamTrackFetcher | null>(null);
  let store = fetcher;
  if (universe === null) {
    if (fetcher !== null) {
      store = null;
      setFetcher(null);
    }
  } else if (fetcher === null || fetcher.universe !== universe || fetcher.requests !== requests) {
    store = new StreamTrackFetcher(requests, universe);
    setFetcher(store);
  }
  const snapshot = useSyncExternalStore(
    store?.subscribe ?? NO_STORE.subscribe,
    store?.getSnapshot ?? NO_STORE.getSnapshot,
  );

  // The tracks the server holds, heaviest stream first, asked for as each answer names them.
  const ready = list?.streams
    .filter((stream) => stream.trackReady)
    .toSorted((a, b) => (a.massMsun === b.massMsun ? a.number - b.number : b.massMsun - a.massMsun))
    .map((stream) => stream.number);
  const readyKey = ready?.join(",") ?? "";
  // The requests in flight belong to one fetcher and one link: they are cancelled with either.
  useEffect(() => {
    if (store === null || !connected) {
      return undefined;
    }
    return () => {
      store.stop();
    };
  }, [store, connected]);
  // Each answer of the list asks again for the tracks not yet on hand, as does the link's return;
  // a newer answer replaces the queue and leaves the requests in flight to finish.
  const answer = current?.response ?? null;
  useEffect(() => {
    if (store === null || !connected || answer === null || readyKey.length === 0) {
      return undefined;
    }
    store.want(readyKey.split(",").map(Number));
    return () => {
      store.forget();
    };
  }, [store, connected, answer, readyKey]);

  // While any track is not on hand, the list is asked for again, to learn which the server holds.
  const missing =
    list !== null && list.streams.some((stream) => !snapshot.tracks.has(stream.number));
  useEffect(() => {
    if (!missing || !connected || state.kind !== "ok") {
      return undefined;
    }
    const timer = setTimeout(() => {
      setGeneration((count) => count + 1);
    }, LIST_REFRESH_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [missing, connected, state]);

  return {
    state,
    list,
    tracks: snapshot.tracks,
    trackFault: snapshot.fault,
    coreFault: checked?.fault ?? null,
    retry: () => {
      setGeneration((count) => count + 1);
    },
  };
}
