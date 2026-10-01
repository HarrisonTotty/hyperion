import type { ClientMessage } from "./generated/ClientMessage";
import type { RequestBody } from "./generated/RequestBody";
import type { RequestError } from "./generated/RequestError";
import type { ResponseBody } from "./generated/ResponseBody";
import type { ServerMessage } from "./generated/ServerMessage";
import type { SubscriptionTopic } from "./generated/SubscriptionTopic";
import type { UniverseIdHex } from "./generated/UniverseIdHex";
import {
  type StateOf,
  type Subscription,
  SubscriptionTable,
  type TopicName,
} from "./subscriptions";

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

/** How a `subscribe` ended: the live subscription, or why there is none. */
export type SubscribeOutcome<T extends TopicName> =
  | { readonly ok: true; readonly subscription: Subscription<T> }
  | { readonly ok: false; readonly error: RequestFailure };

/** A `subscribe` in flight. */
export interface PendingSubscription<T extends TopicName> {
  /** Settles exactly once with the subscription or the failure, and never rejects. */
  readonly outcome: Promise<SubscribeOutcome<T>>;
  /**
   * Cancels the `subscribe`, as {@link PendingRequest.cancel} does. A subscription that has opened
   * is ended with its own `unsubscribe` instead.
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
  /** The live subscriptions, which notifications are routed to (rendering plan R03, R03.T5.c). */
  readonly #subscriptions = new SubscriptionTable();
  /**
   * Cancelled requests whose terminal message the server still owes, each with what to do if that
   * message is a response after all: a `subscribe` answered as it was cancelled must be ended.
   */
  readonly #cancelled = new Map<number, ((response: ResponseBody) => void) | undefined>();
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
   * Opens a subscription to `topic` in `universe`: a `subscribe` request whose answer opens a
   * {@link Subscription} that the server's notifications are routed to.
   *
   * @remarks
   * The subscription is registered the moment its answer arrives, so that no notification after
   * it is taken for one of an unknown subscription. When the link is down the outcome settles as
   * `link_lost` straight away.
   */
  subscribe<T extends TopicName>(
    universe: UniverseIdHex,
    topic: Extract<SubscriptionTopic, { topic: T }>,
  ): PendingSubscription<T> {
    let opened: Subscription<T> | undefined;
    const started = this[startRequest](
      { kind: "subscribe", universe, topic },
      (response) => {
        if (response.kind !== "subscribe") {
          return;
        }
        const id = response.subscription;
        if (isStateOf(topic.topic, response.state)) {
          opened = this.#subscriptions.open(id, response.state, () => {
            this.#unsubscribe(id);
          });
        } else {
          // Opened, but not as asked: the subscription is of no use and is ended at once.
          this.#unsubscribe(id);
        }
      },
      (late) => {
        // Answered as it was cancelled: the server holds a subscription nobody will read.
        if (late.kind === "subscribe") {
          this.#unsubscribe(late.subscription);
        }
      },
    );
    const outcome = started.outcome.then((settled): SubscribeOutcome<T> => {
      if (!settled.ok) {
        return settled;
      }
      if (opened === undefined) {
        return {
          ok: false,
          error: {
            code: "protocol_violation",
            message: `the server opened a subscription to another topic than ${topic.topic}`,
          },
        };
      }
      return { ok: true, subscription: opened };
    });
    return {
      outcome,
      cancel: () => {
        started.cancel("aborted");
      },
    };
  }

  /**
   * Settles the request a `response` or `request_error` ends, and routes a `notification` to its
   * subscription.
   *
   * @remarks
   * An answer to an unknown or cancelled ID is dropped: a cancelled request has already settled.
   * So is a notification for a subscription this client does not have, whether it has ended or
   * was never opened (rendering plan R03, Design note 1).
   *
   * @returns `true` when the message belonged to a request or a subscription and was consumed,
   *   `false` for every other message, which the caller handles itself.
   */
  handleServerMessage(message: ServerMessage): boolean {
    switch (message.type) {
      case "response": {
        const late = this.#cancelled.get(message.id);
        const request = this.#take(message.id);
        if (request === undefined) {
          late?.(message.body);
        } else {
          request.respond(message.body);
        }
        break;
      }
      case "request_error":
        this.#take(message.id)?.fail(message.error);
        break;
      case "notification":
        this.#subscriptions.route(message);
        break;
      case "welcome":
      case "pong":
      case "error":
        return false;
    }
    return true;
  }

  /**
   * Settles every request in flight as `link_lost` and ends every subscription, for when the
   * socket has closed: the server ends a connection's subscriptions with its socket.
   */
  linkLost(): void {
    const inFlight = [...this.#inFlight.values()];
    this.#inFlight.clear();
    this.#cancelled.clear();
    for (const request of inFlight) {
      request.fail(LINK_LOST);
    }
    this.#subscriptions.linkLost();
  }

  /**
   * Starts a request; see {@link startRequest}. `onAnswer`, if given, sees the response the moment
   * it arrives, before the outcome settles; `onLateAnswer` sees a response that arrives after the
   * request was cancelled.
   */
  [startRequest]<K extends RequestKind>(
    body: RequestOf<K>,
    onAnswer?: (response: ResponseBody) => void,
    onLateAnswer?: (response: ResponseBody) => void,
  ): StartedRequest<K> {
    const id = this.#allocateId();
    const outcome = new Promise<RequestOutcome<K>>((resolve) => {
      this.#inFlight.set(id, {
        respond: (response) => {
          onAnswer?.(response);
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
          this.#cancelled.set(id, onLateAnswer);
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

  /** Ends subscription `id` on the server; its answer changes nothing here. */
  #unsubscribe(id: number): void {
    // Discarded without a `.catch`: a request's outcome never rejects (it settles every failure,
    // `link_lost` included), and this package has no console to report on.
    void this.request({ kind: "unsubscribe", subscription: id }).outcome;
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

/** Whether `state` is the state of a subscription to `topic`, which makes it a {@link StateOf}. */
function isStateOf<T extends TopicName>(topic: T, state: StateOf<TopicName>): state is StateOf<T> {
  // With the scene the only topic the comparison is always true; plan 12's alerts make it real.
  // oxlint-disable-next-line typescript/no-unnecessary-condition
  return state.topic === topic;
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
