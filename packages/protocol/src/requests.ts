import type { ClientMessage } from "./generated/ClientMessage";
import type { RequestBody } from "./generated/RequestBody";
import type { RequestError } from "./generated/RequestError";
import type { ResponseBody } from "./generated/ResponseBody";
import type { ServerMessage } from "./generated/ServerMessage";

/** The `kind` of every request the protocol defines. */
export type RequestKind = RequestBody["kind"];

/** The request body of one kind. */
export type RequestOf<K extends RequestKind> = Extract<RequestBody, { kind: K }>;

/** The response body that answers a request of kind `K`. */
export type ResponseFor<K extends RequestKind> = Extract<ResponseBody, { kind: K }>;

/**
 * Why a request ended without a response.
 *
 * @remarks
 * Either the server's own {@link RequestError}, or one of four endings the client decides:
 * `link_lost` when the request could not be sent or the link dropped before its answer, `aborted`
 * when it was cancelled, `superseded` when a {@link RequestChannel} replaced it with a newer one,
 * and `protocol_violation` when the server answered with a response of another kind.
 */
export type RequestFailure =
  | RequestError
  | {
      readonly code: "link_lost" | "aborted" | "superseded" | "protocol_violation";
      readonly message: string;
    };

/** How a request ended. */
export type RequestOutcome<K extends RequestKind> =
  | { readonly ok: true; readonly response: ResponseFor<K> }
  | { readonly ok: false; readonly error: RequestFailure };

/** A request in flight. */
export interface PendingRequest<K extends RequestKind> {
  /** Settles exactly once with the request's outcome, and never rejects. */
  readonly outcome: Promise<RequestOutcome<K>>;
  /**
   * Cancels the request: the server is told, and the outcome settles as `aborted` at once unless it
   * has already settled. Calling it again does nothing.
   */
  cancel(): void;
}

/** The largest request ID, 2³² − 1, after which IDs wrap to 1. */
export const MAX_REQUEST_ID = 0xffff_ffff;

/**
 * The first request ID after `previous` that is not in use, wrapping from 2³² − 1 to 1.
 *
 * @remarks
 * Module-internal; exported for its tests. The caller makes sure some ID is free.
 */
export function nextFreeRequestId(previous: number, inUse: (id: number) => boolean): number {
  let id = previous;
  do {
    id = id >= MAX_REQUEST_ID ? 1 : id + 1;
  } while (inUse(id));
  return id;
}

/** The ending of a request that was cancelled, by the caller or by a newer request. */
type CancelReason = "aborted" | "superseded";

/** A request in flight, as the client tracks it until its terminal message. */
interface InFlight {
  /** Settles the outcome with the server's response, checking its kind first. */
  readonly respond: (response: ResponseBody) => void;
  /** Settles the outcome with a failure. */
  readonly fail: (failure: RequestFailure) => void;
}

/** A started request whose cancellation can report either reason. */
interface StartedRequest<K extends RequestKind> {
  readonly outcome: Promise<RequestOutcome<K>>;
  readonly cancel: (reason: CancelReason) => void;
}

const LINK_LOST: RequestFailure = {
  code: "link_lost",
  message: "the link to the server is down",
};

const CANCEL_FAILURES = {
  aborted: { code: "aborted", message: "the request was cancelled" },
  superseded: { code: "superseded", message: "a newer request replaced this one" },
} as const satisfies Record<CancelReason, RequestFailure>;

/**
 * Whether `response` answers `request`, which is what makes it a {@link ResponseFor}.
 *
 * @remarks
 * The trust boundary of the request client. The generated `ResponseBody` union has exactly one
 * variant per `kind`, so a response whose `kind` equals the request's is the variant the request
 * expects. The payload itself is trusted as the server's, as in `decodeServerMessage`.
 */
function answers<K extends RequestKind>(
  request: RequestOf<K>,
  response: ResponseBody,
): response is ResponseFor<K> {
  return response.kind === request.kind;
}

/**
 * Starts a request whose cancellation may report `superseded` as well as `aborted`.
 *
 * @remarks
 * Keyed by a symbol this module does not export, so that only {@link RequestChannel} can call it.
 */
const startRequest = Symbol("startRequest");

/**
 * Keys {@link RequestClient}'s method that moves its ID counter.
 *
 * @remarks
 * Module-internal; exported for its tests, like {@link nextFreeRequestId}.
 */
export const setLastRequestId = Symbol("setLastRequestId");

/**
 * Makes requests over one server connection and matches each answer to its request.
 *
 * @remarks
 * Every request gets a fresh ID and settles exactly once. The connection feeds every server message
 * through {@link RequestClient.handleServerMessage} and calls {@link RequestClient.linkLost} when
 * the socket closes. IDs count up from 1, wrap after 2³² − 1, and skip any ID the server may still
 * answer: those pending, and those cancelled whose terminal message has not arrived yet.
 *
 * @example
 * ```ts
 * const pending = client.request({ kind: "list_universes" });
 * const outcome = await pending.outcome;
 * if (outcome.ok) {
 *   show(outcome.response.universes);
 * }
 * ```
 */
export class RequestClient {
  readonly #send: (message: ClientMessage) => boolean;
  readonly #inFlight = new Map<number, InFlight>();
  /** Cancelled requests whose terminal message the server still owes. */
  readonly #cancelled = new Set<number>();
  #lastId = 0;

  /**
   * @param send - Writes a message to the server, returning `false` when the link is down and
   *   nothing was sent.
   */
  constructor(send: (message: ClientMessage) => boolean) {
    this.#send = send;
  }

  /**
   * Sends a request at once.
   *
   * @remarks
   * When the link is down the outcome settles as `link_lost` straight away.
   */
  request<K extends RequestKind>(body: RequestOf<K>): PendingRequest<K> {
    const started = this[startRequest](body);
    return {
      outcome: started.outcome,
      cancel: () => {
        started.cancel("aborted");
      },
    };
  }

  /**
   * Settles the request a `response` or `request_error` ends.
   *
   * @remarks
   * An answer to an unknown or cancelled ID is dropped: a cancelled request has already settled.
   *
   * @returns `true` when the message belonged to a request and was consumed, `false` for every
   *   other message, which the caller handles itself.
   */
  handleServerMessage(message: ServerMessage): boolean {
    switch (message.type) {
      case "response":
        this.#take(message.id)?.respond(message.body);
        break;
      case "request_error":
        this.#take(message.id)?.fail(message.error);
        break;
      case "welcome":
      case "pong":
      case "error":
        return false;
    }
    return true;
  }

  /** Settles every request in flight as `link_lost`, for when the socket has closed. */
  linkLost(): void {
    const inFlight = [...this.#inFlight.values()];
    this.#inFlight.clear();
    this.#cancelled.clear();
    for (const request of inFlight) {
      request.fail(LINK_LOST);
    }
  }

  /** Starts a request; see {@link startRequest}. */
  [startRequest]<K extends RequestKind>(body: RequestOf<K>): StartedRequest<K> {
    const id = this.#allocateId();
    const outcome = new Promise<RequestOutcome<K>>((resolve) => {
      this.#inFlight.set(id, {
        respond: (response) => {
          resolve(
            answers<K>(body, response)
              ? { ok: true, response }
              : {
                  ok: false,
                  error: {
                    code: "protocol_violation",
                    message: `the server answered a ${body.kind} request with ${response.kind}`,
                  },
                },
          );
        },
        fail: (error) => {
          resolve({ ok: false, error });
        },
      });
    });
    if (!this.#send({ type: "request", id, body })) {
      this.#take(id)?.fail(LINK_LOST);
    }
    return {
      outcome,
      cancel: (reason) => {
        // Only the pending entry is removed: a request cancelled once already, or settled, must
        // stay in `#cancelled` until the server's terminal message for it arrives.
        const request = this.#inFlight.get(id);
        if (request === undefined) {
          return;
        }
        this.#inFlight.delete(id);
        // With the link down the server has forgotten the request, so it owes no answer.
        if (this.#send({ type: "cancel", id })) {
          this.#cancelled.add(id);
        }
        request.fail(CANCEL_FAILURES[reason]);
      },
    };
  }

  /**
   * Moves the ID counter as if `id` had just been allocated.
   *
   * @remarks
   * Module-internal; for tests that reach the wrap after 2³² − 1 without making that many requests.
   */
  [setLastRequestId](id: number): void {
    this.#lastId = id;
  }

  /** Removes and returns the request that a terminal message with this ID ends. */
  #take(id: number): InFlight | undefined {
    this.#cancelled.delete(id);
    const request = this.#inFlight.get(id);
    this.#inFlight.delete(id);
    return request;
  }

  #allocateId(): number {
    if (this.#inFlight.size + this.#cancelled.size >= MAX_REQUEST_ID) {
      throw new Error("every request ID is in use");
    }
    this.#lastId = nextFreeRequestId(
      this.#lastId,
      (id) => this.#inFlight.has(id) || this.#cancelled.has(id),
    );
    return this.#lastId;
  }
}

/**
 * A sequence of requests of which only the latest matters, such as the queries behind one panel.
 *
 * @remarks
 * Each request cancels the channel's previous one if it is still in flight, which then settles as
 * `superseded`, and the server is told to drop it. Displays get "latest wins" from this alone.
 */
export class RequestChannel {
  readonly #client: RequestClient;
  #supersedeLatest: (() => void) | undefined;

  constructor(client: RequestClient) {
    this.#client = client;
  }

  /** Sends a request, superseding the channel's previous one. */
  request<K extends RequestKind>(body: RequestOf<K>): PendingRequest<K> {
    this.#supersedeLatest?.();
    const started = this.#client[startRequest](body);
    this.#supersedeLatest = () => {
      started.cancel("superseded");
    };
    return {
      outcome: started.outcome,
      cancel: () => {
        started.cancel("aborted");
      },
    };
  }
}
