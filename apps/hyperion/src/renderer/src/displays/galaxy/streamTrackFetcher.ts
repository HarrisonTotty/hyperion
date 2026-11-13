import type { RequestClient, UniverseIdHex } from "@hyperion/protocol";

import { toStreamTrack, toStreamTrackRequest } from "../../lib/galaxy/globalWire";
import type { StreamTrack, TrackPoint } from "../../lib/galaxy/model";
import { followRequest, REQUEST_TIMEOUT_MS } from "../../lib/useServerRequest";

/**
 * The most `stream_track` requests one universe's tracks keep in flight, so that a warmed galaxy's
 * few hundred tracks arrive steadily without filling the server's interactive queue.
 */
export const MAX_TRACKS_IN_FLIGHT = 4;

/**
 * How many tracks the fetcher keeps before it tells its readers, unless its requests in flight run
 * out first: so that a warmed galaxy's few hundred tracks redraw the map some tens of times, not
 * once each.
 */
export const TRACKS_PER_PUBLISH = 16;

/** What the fetcher holds: the tracks on hand, and whether any answer could not be drawn. */
export interface StreamTracksSnapshot {
  /** The tracks fetched so far, by stream number. */
  readonly tracks: ReadonlyMap<number, StreamTrack>;
  /** Why an answer on hand cannot be drawn, in words, or `null` when nothing is wrong. */
  readonly fault: string | null;
}

/** No tracks, and no fault: the snapshot of a fetcher with nothing to fetch. */
export const NO_TRACKS: StreamTracksSnapshot = { tracks: new Map(), fault: null };

/** Whether a track point can be drawn and read: every number of it finite. */
function usablePoint(point: TrackPoint): boolean {
  const { x, y, z } = point.positionLy;
  return [x, y, z, point.normalWidthLy, point.binormalWidthLy, point.speedKmS].every((value) =>
    Number.isFinite(value),
  );
}

/** Whether a track can be drawn and read: every point of it, and its length where sent. */
function usableTrack(track: StreamTrack): boolean {
  return (
    track.trailing.every(usablePoint) &&
    track.leading.every(usablePoint) &&
    (track.lengthLy === null || Number.isFinite(track.lengthLy))
  );
}

/**
 * Fetches the tracks of one universe's streams, a few at a time (plan 10, P10.T11), for the galaxy
 * map's stream overlay.
 *
 * @remarks
 * An external store, read with `useSyncExternalStore`. {@link StreamTrackFetcher.want} names the
 * streams whose tracks the server holds; the fetcher asks for each it lacks and is not asking for
 * already, at most {@link MAX_TRACKS_IN_FLIGHT} at once, in the order given, and keeps every track
 * it is answered with, telling its readers every {@link TRACKS_PER_PUBLISH} answers and whenever
 * its requests run out. A track answered `pending`, or not answered (refused, timed out, or cut
 * off by the link), is asked again at the next `want` that names it, as the page asks when its
 * list is answered again. An answer that cannot be drawn is kept out and named as the snapshot's
 * fault until a usable answer for that stream replaces it. {@link StreamTrackFetcher.forget}
 * drops the queue, and {@link StreamTrackFetcher.stop} cancels the requests in flight too; the
 * tracks on hand stay.
 */
export class StreamTrackFetcher {
  /** The link's request client it asks with. */
  readonly requests: RequestClient;
  /** The universe whose tracks it fetches. */
  readonly universe: UniverseIdHex;
  readonly #tracks = new Map<number, StreamTrack>();
  /** The streams whose last answer could not be drawn. */
  readonly #faulty = new Set<number>();
  readonly #inFlight = new Map<number, () => void>();
  readonly #listeners = new Set<() => void>();
  #queue: number[] = [];
  /** The answers kept or refused since the readers were last told. */
  #unpublished = 0;
  #snapshot: StreamTracksSnapshot = NO_TRACKS;

  constructor(requests: RequestClient, universe: UniverseIdHex) {
    this.requests = requests;
    this.universe = universe;
  }

  /** Adds a listener for a change of the snapshot; returns its removal. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** The tracks on hand and the fault, one object while they are unchanged. */
  readonly getSnapshot = (): StreamTracksSnapshot => this.#snapshot;

  /** Asks for the tracks of `numbers` it lacks and is not asking for already, in that order. */
  want(numbers: ReadonlyArray<number>): void {
    this.#queue = numbers.filter(
      (number) => !this.#tracks.has(number) && !this.#inFlight.has(number),
    );
    this.#pump();
  }

  /** Forgets the queue; the requests in flight go on. */
  forget(): void {
    this.#queue = [];
  }

  /**
   * Forgets the queue and cancels the requests in flight; the tracks on hand stay, and those kept
   * since the readers were last told are told.
   */
  stop(): void {
    this.forget();
    const stops = [...this.#inFlight.values()];
    this.#inFlight.clear();
    for (const stopRequest of stops) {
      stopRequest();
    }
    this.#publish();
  }

  #pump(): void {
    while (this.#inFlight.size < MAX_TRACKS_IN_FLIGHT) {
      const number = this.#queue.shift();
      if (number === undefined) {
        return;
      }
      const pending = this.requests.request(toStreamTrackRequest(this.universe, number));
      const stopRequest = followRequest(pending, REQUEST_TIMEOUT_MS, (state) => {
        this.#inFlight.delete(number);
        if (state.kind === "ok") {
          const answer = toStreamTrack(state.response);
          if (answer.kind === "ready") {
            this.#keep(answer.track);
          }
        }
        // With the link down every request settles at once: the queue waits for its return.
        if (state.kind !== "link_down") {
          this.#pump();
        }
        if (this.#unpublished >= TRACKS_PER_PUBLISH || this.#inFlight.size === 0) {
          this.#publish();
        }
      });
      this.#inFlight.set(number, stopRequest);
    }
  }

  #keep(track: StreamTrack): void {
    if (usableTrack(track)) {
      this.#tracks.set(track.number, track);
      this.#faulty.delete(track.number);
    } else {
      this.#faulty.add(track.number);
    }
    this.#unpublished += 1;
  }

  #publish(): void {
    if (this.#unpublished === 0) {
      return;
    }
    this.#unpublished = 0;
    this.#snapshot = {
      tracks: new Map(this.#tracks),
      fault: this.#faulty.size === 0 ? null : "track unusable",
    };
    for (const listener of this.#listeners) {
      listener();
    }
  }
}
