import type { ErrorCode } from "./generated/ErrorCode";
import type { NotificationBody } from "./generated/NotificationBody";
import type { RequestError } from "./generated/RequestError";
import type { ServerMessage } from "./generated/ServerMessage";
import type { SubscriptionState } from "./generated/SubscriptionState";
import type { SubscriptionTopic } from "./generated/SubscriptionTopic";

/** The name of every topic a subscription can watch. */
export type TopicName = SubscriptionTopic["topic"];

/** The whole state a subscription to topic `T` opens with. */
export type StateOf<T extends TopicName> = Extract<SubscriptionState, { topic: T }>;

/** A notification's body on a subscription to topic `T`. */
export type NotificationOf<T extends TopicName> = Extract<NotificationBody, { topic: T }>;

/**
 * Why a subscription ended: the client unsubscribed; the link to the server dropped, which ends
 * every subscription (the server ends them with the socket); or the server could no longer serve
 * it and ended it with `subscription_ended`, saying why.
 */
export type SubscriptionEnd =
  | { readonly kind: "unsubscribed" }
  | { readonly kind: "link_lost" }
  | { readonly kind: "ended"; readonly error: RequestError };

const UNSUBSCRIBED: SubscriptionEnd = { kind: "unsubscribed" };
const LINK_LOST: SubscriptionEnd = { kind: "link_lost" };

/** The reason given for a `subscription_ended` whose own the client cannot read. */
const UNREADABLE_END: RequestError = {
  code: "internal",
  message: "the server ended the subscription without a reason the client can read",
  field: null,
};

/**
 * Every code a request error may carry, as this client knows them; a code added to `ErrorCode` must
 * be added here, or this does not compile.
 */
const ERROR_CODES: Readonly<Record<ErrorCode, true>> = {
  bad_request: true,
  unsupported: true,
  hello_required: true,
  unknown_universe: true,
  unknown_system: true,
  unknown_body: true,
  generator_version_mismatch: true,
  unsupported_save_format: true,
  name_taken: true,
  universe_limit_reached: true,
  too_many_requests: true,
  queue_full: true,
  cancelled: true,
  storage_failed: true,
  internal: true,
};

/**
 * Whether `error`, as parsed from the wire, is a request error this client can read: a code it
 * knows, a message, and a field as a string or `null`.
 */
function isRequestError(error: unknown): error is RequestError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    Object.hasOwn(ERROR_CODES, error.code) &&
    "message" in error &&
    typeof error.message === "string" &&
    "field" in error &&
    (error.field === null || typeof error.field === "string")
  );
}

/**
 * A live subscription to topic `T`, which the server pushes notifications on until it ends.
 *
 * @remarks
 * Made by `RequestClient.subscribe` once the server has answered `subscribed`. Notifications arrive
 * in the order the server sent them; checking their `sequence` is the topic's affair.
 */
export interface Subscription<T extends TopicName> {
  /** The subscription's number on this connection, as the server gave it. */
  readonly id: number;
  /** The topic's whole state when the subscription opened. */
  readonly state: StateOf<T>;
  /**
   * Calls `listener` with every notification from now on.
   *
   * @returns A function that removes the listener.
   */
  onNotification(listener: (notification: NotificationOf<T>) => void): () => void;
  /**
   * Calls `listener` once when the subscription ends, or at once if it has ended already.
   *
   * @returns A function that removes the listener.
   */
  onEnd(listener: (end: SubscriptionEnd) => void): () => void;
  /**
   * Ends the subscription: no notification reaches a listener after this, and the server is told
   * when the link is up. Calling it again does nothing.
   */
  unsubscribe(): void;
  /** Whether the subscription has ended. */
  readonly ended: boolean;
}

/**
 * Whether `body` is a notification of `topic`, which makes it a {@link NotificationOf}.
 *
 * @remarks
 * The server numbers subscriptions per connection and never reuses a number, so a body on a
 * subscription is of its topic; this is the check that says so to the compiler.
 */
function isNotificationOf<T extends TopicName>(
  topic: T,
  body: NotificationBody,
): body is NotificationOf<T> {
  // With the scene the only topic the comparison is always true; plan 12's alerts make it real.
  // oxlint-disable-next-line typescript/no-unnecessary-condition
  return body.topic === topic;
}

/** One live subscription, as the table tracks it. */
interface Entry {
  readonly route: (body: NotificationBody) => void;
  readonly end: (end: SubscriptionEnd) => void;
}

/**
 * A connection's live subscriptions, which route each notification to its own.
 *
 * @remarks
 * Module-internal to `@hyperion/protocol`: `RequestClient` owns one.
 */
export class SubscriptionTable {
  readonly #live = new Map<number, Entry>();

  /**
   * Registers subscription `id` of topic `T`, which opened with `state`; `unsubscribe` tells the
   * server.
   */
  open<T extends TopicName>(
    id: number,
    state: StateOf<T>,
    unsubscribe: () => void,
  ): Subscription<T> {
    const listeners = new Set<(notification: NotificationOf<T>) => void>();
    const endListeners = new Set<(end: SubscriptionEnd) => void>();
    let ended: SubscriptionEnd | undefined;
    const end = (reason: SubscriptionEnd): void => {
      if (ended !== undefined) {
        return;
      }
      ended = reason;
      this.#live.delete(id);
      listeners.clear();
      for (const listener of endListeners) {
        listener(reason);
      }
      endListeners.clear();
    };
    this.#live.set(id, {
      route: (body) => {
        if (isNotificationOf(state.topic, body)) {
          for (const listener of listeners) {
            listener(body);
          }
        }
      },
      end,
    });
    return {
      id,
      state,
      onNotification: (listener) => {
        if (ended === undefined) {
          listeners.add(listener);
        }
        return () => {
          listeners.delete(listener);
        };
      },
      onEnd: (listener) => {
        if (ended !== undefined) {
          listener(ended);
          return () => undefined;
        }
        endListeners.add(listener);
        return () => {
          endListeners.delete(listener);
        };
      },
      unsubscribe: () => {
        if (ended !== undefined) {
          return;
        }
        end(UNSUBSCRIBED);
        unsubscribe();
      },
      get ended() {
        return ended !== undefined;
      },
    };
  }

  /**
   * Hands a notification to its subscription; one for a subscription this connection does not
   * have (ended, or never opened) is dropped.
   */
  route(message: Extract<ServerMessage, { type: "notification" }>): void {
    this.#live.get(message.subscription)?.route(message.body);
  }

  /**
   * Ends the subscription a `subscription_ended` names, with the server's reason; one this
   * connection does not have is ignored. A reason the client cannot read still ends it, as
   * `internal`, so that the topic's owner is told and does not wait on it.
   */
  endedByServer(message: Extract<ServerMessage, { type: "subscription_ended" }>): void {
    const error = isRequestError(message.error) ? message.error : UNREADABLE_END;
    this.#live.get(message.subscription)?.end({ kind: "ended", error });
  }

  /** Ends every subscription as `link_lost`, for when the socket has closed. */
  linkLost(): void {
    for (const entry of this.#live.values()) {
      entry.end(LINK_LOST);
    }
  }
}
