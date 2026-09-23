import {
  type PendingRequest,
  RequestChannel,
  type RequestKind,
  type RequestOf,
  type RequestOutcome,
  type ResponseFor,
} from "@hyperion/protocol";
import { useEffect, useState } from "react";

import type { ConnectionStatus } from "./connection";
import { linkDownReason, useServerLink } from "./serverLink";

/**
 * Where a display's request stands, as the operator sees it (plan 05, design note D4).
 *
 * @remarks
 * `rejected` carries the server's error code and message, or the client's when the server
 * answered with a response of the wrong kind. `link_down` carries the link's state in the header
 * annunciator's words.
 */
export type RequestState<K extends RequestKind> =
  | { readonly kind: "idle" }
  | { readonly kind: "pending" }
  | { readonly kind: "ok"; readonly response: ResponseFor<K> }
  | { readonly kind: "rejected"; readonly code: string; readonly reason: string }
  | { readonly kind: "timed_out" }
  | { readonly kind: "link_down"; readonly reason: string };

/**
 * How long a request may take before it is cancelled and reported as timed out, in milliseconds.
 *
 * @remarks
 * Plan 04's request client has no timers, and the guide requires a timed-out state for every
 * command and request (plan 05, design note D4). A density map, which the server computes for
 * seconds and queues behind other work, is given longer by its caller.
 */
export const REQUEST_TIMEOUT_MS = 30_000;

/** A state a request ends in: every {@link RequestState} but `idle` and `pending`. */
export type SettledRequestState<K extends RequestKind> = Exclude<
  RequestState<K>,
  { readonly kind: "idle" } | { readonly kind: "pending" }
>;

/** The settled state of one request, and which request it was. */
interface Settled<K extends RequestKind> {
  readonly body: RequestOf<K>;
  readonly generation: number;
  readonly state: SettledRequestState<K>;
}

// The states without a payload are shared, so that an unchanged state keeps its identity from one
// render to the next and a memoised consumer does not re-render.
const IDLE = { kind: "idle" } as const;

/** The state of a request sent and not yet answered. */
export const PENDING = { kind: "pending" } as const;

const TIMED_OUT = { kind: "timed_out" } as const;

type DownStatus = Exclude<ConnectionStatus, "connected">;

function linkDownState(status: DownStatus): {
  readonly kind: "link_down";
  readonly reason: string;
} {
  const reason = linkDownReason(status);
  if (reason === null) {
    throw new Error(`the link state ${status} gives no reason for being down`);
  }
  return { kind: "link_down", reason };
}

const LINK_DOWN_STATES = {
  connecting: linkDownState("connecting"),
  disconnected: linkDownState("disconnected"),
  incompatible: linkDownState("incompatible"),
} as const satisfies Record<DownStatus, RequestState<never>>;

/** How a request lost to a dropped link reads: the socket closed, so there is no carrier. */
const LINK_LOST = LINK_DOWN_STATES.disconnected;

/**
 * The state an outcome settles a request in, or `null` for an outcome the operator never sees.
 *
 * @remarks
 * `aborted` and `superseded` requests were ended by the client itself (an unmount, a newer
 * request) and are dropped silently. A lost link reads as the link being down.
 */
function settledState<K extends RequestKind>(
  outcome: RequestOutcome<K>,
): SettledRequestState<K> | null {
  if (outcome.ok) {
    return { kind: "ok", response: outcome.response };
  }
  const { error } = outcome;
  let state: SettledRequestState<K> | null;
  switch (error.code) {
    case "aborted":
    case "superseded":
      state = null;
      break;
    case "link_lost":
      state = LINK_LOST;
      break;
    case "protocol_violation":
    case "bad_request":
    case "unsupported":
    case "hello_required":
    case "unknown_universe":
    case "unknown_system":
    case "unknown_body":
    case "generator_version_mismatch":
    case "unsupported_save_format":
    case "name_taken":
    case "universe_limit_reached":
    case "too_many_requests":
    case "queue_full":
    case "cancelled":
    case "storage_failed":
    case "internal":
      state = { kind: "rejected", code: error.code, reason: error.message };
      break;
  }
  return state;
}

/**
 * Follows a request in flight to the state it settles in, cancelling it if it is not answered in
 * time.
 *
 * @remarks
 * The one place where outcomes become states (plan 05, design note D4), shared by
 * {@link useServerRequest} and the universe commands. `onSettled` is called at most once: with the
 * outcome's state, or with `timed_out` after `timeoutMs`, when the request is cancelled. Outcomes
 * the client caused itself, `aborted` and `superseded`, are dropped.
 *
 * @param timeoutMs - Milliseconds before an unanswered request is cancelled.
 * @returns Stops following: the timer is cleared, `onSettled` is never called, and the request is
 *   cancelled if it is still in flight. Calling it again does nothing.
 */
export function followRequest<K extends RequestKind>(
  pending: PendingRequest<K>,
  timeoutMs: number,
  onSettled: (state: SettledRequestState<K>) => void,
): () => void {
  let following = true;
  const timer = setTimeout(() => {
    following = false;
    pending.cancel();
    onSettled(TIMED_OUT);
  }, timeoutMs);
  pending.outcome
    .then((outcome) => {
      if (following) {
        following = false;
        clearTimeout(timer);
        const state = settledState(outcome);
        if (state !== null) {
          onSettled(state);
        }
      }
      return undefined;
    })
    .catch((error: unknown) => {
      console.error("a request's outcome failed to settle:", error);
    });
  return () => {
    following = false;
    clearTimeout(timer);
    pending.cancel();
  };
}

/**
 * Whether two values of JSON data are equal, member by member.
 *
 * @remarks
 * Request bodies are plain JSON data, so this is how the hook tells a changed body from the same
 * body built again.
 */
function sameJson(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) {
    return true;
  }
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) {
    return false;
  }
  if (Array.isArray(a) || Array.isArray(b)) {
    return (
      Array.isArray(a) &&
      Array.isArray(b) &&
      a.length === b.length &&
      a.every((item, index) => sameJson(item, b[index]))
    );
  }
  const aEntries = Object.entries(a);
  const bRecord = new Map(Object.entries(b));
  return (
    aEntries.length === bRecord.size &&
    aEntries.every(([key, value]) => bRecord.has(key) && sameJson(value, bRecord.get(key)))
  );
}

/**
 * Makes one request on behalf of a display, and reports where it stands.
 *
 * @remarks
 * The hook owns one `RequestChannel` for its lifetime, so a newer body supersedes the older
 * request on the client and cancels it on the server (plan 05, design note D4). It sends when
 * `body` changes by value and the link is up, and shows `pending` until the outcome: a response
 * is `ok`; a server error, or a response of the wrong kind, is `rejected`; a lost link is
 * `link_down`. A request still unanswered after `timeoutMs` is cancelled and reported as
 * `timed_out`. With the link down nothing is sent and the state is `link_down`; a request that
 * never got its answer is sent again when the link returns. An `ok` answer stays through a link
 * loss and is not fetched again, since requests are stateless and it is a snapshot, not a live
 * value. A rejection or a timeout stays until `generation` changes: hiding and showing the display
 * that holds the hook sends nothing. Unmounting cancels the request in flight. States are shared
 * objects, so an unchanged state keeps its identity across renders.
 *
 * @param body - The request to make, or `null` for none (`idle`). Compared by value, so it may be
 *   built afresh on every render.
 * @param timeoutMs - Milliseconds before an unanswered request is cancelled.
 * @param generation - Changing it sends the same body again, as a refresh. The last `ok` answer
 *   stays on show until the new one arrives, and through a link loss that cuts the refresh off;
 *   a refresh that the server rejects, or that times out, replaces it with its failure.
 */
export function useServerRequest<K extends RequestKind>(
  body: RequestOf<K> | null,
  timeoutMs: number = REQUEST_TIMEOUT_MS,
  generation = 0,
): RequestState<K> {
  const { status, requests } = useServerLink();
  const connected = status === "connected";
  // The request client lives as long as the connection, so one channel serves for the hook's life.
  const [channel] = useState(() => new RequestChannel(requests));
  const [stableBody, setStableBody] = useState(body);
  const [settled, setSettled] = useState<Settled<K> | null>(null);
  const [wasConnected, setWasConnected] = useState(connected);

  let current = stableBody;
  if (body !== stableBody && !sameJson(body, stableBody)) {
    setStableBody(body);
    current = body;
  }
  let result = settled;
  if (wasConnected !== connected) {
    setWasConnected(connected);
    // A failure belongs to the link it happened on; only an answer outlives the link. Cleared as
    // the link comes up too, in case a lost request settled after the link was seen to drop.
    if (result !== null && result.state.kind !== "ok") {
      setSettled(null);
      result = null;
    }
  }

  // Any settled state of this body and generation is its answer, a failure included: a failure is
  // sent again only through `generation` (`RETRY`), never because the effect ran again, as it does
  // when a hidden display is shown. A link failure is cleared above, so that read is sent again.
  const answered = result !== null && result.body === current && result.generation === generation;
  const needsSending = current !== null && connected && !answered;

  useEffect(() => {
    if (!needsSending || current === null) {
      return undefined;
    }
    const sent = current;
    return followRequest(channel.request(sent), timeoutMs, (state) => {
      setSettled((previous) =>
        // A refresh cut off by the link leaves the last answer on show: answers outlive the link.
        state.kind === "link_down" &&
        previous !== null &&
        previous.body === sent &&
        previous.state.kind === "ok"
          ? previous
          : { body: sent, generation, state },
      );
    });
  }, [channel, current, generation, needsSending, timeoutMs]);

  if (current === null) {
    return IDLE;
  }
  if (result !== null && result.body === current) {
    if (result.state.kind === "ok") {
      return result.state;
    }
    if (connected && result.generation === generation) {
      return result.state;
    }
  }
  if (!connected) {
    return LINK_DOWN_STATES[status];
  }
  return PENDING;
}
