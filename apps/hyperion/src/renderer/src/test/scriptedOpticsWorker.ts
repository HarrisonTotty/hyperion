import type { OpticsReply, OpticsRequest } from "../view/atmosphere/opticsMessages";
import type { OpticsWorkerLike } from "../view/atmosphere/opticsPool";

/**
 * An optics worker driven by the test (plan R08, R08.T10.a): it records what it is posted and
 * answers, or fails as a worker raises its error, when told.
 */
export class ScriptedOpticsWorker implements OpticsWorkerLike {
  readonly #events = new EventTarget();
  readonly #wrappers = new Map<unknown, EventListenerObject>();
  readonly #answered = new Set<number>();
  /** Every request posted, in order. */
  readonly received: OpticsRequest[] = [];
  terminated = false;

  /** The requests posted and not yet answered, oldest first. */
  get pending(): OpticsRequest[] {
    return this.received.filter((request) => !this.#answered.has(request.id));
  }

  postMessage(message: OpticsRequest): void {
    this.received.push(message);
  }

  terminate(): void {
    this.terminated = true;
  }

  /** Answers a request. */
  reply(reply: OpticsReply): void {
    this.#answered.add(reply.id);
    this.#events.dispatchEvent(new MessageEvent("message", { data: reply }));
  }

  /** Raises the worker's error, as a worker whose script threw. */
  fail(): void {
    this.#events.dispatchEvent(new Event("error"));
  }

  addEventListener(type: "message", listener: (event: MessageEvent<OpticsReply>) => void): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<OpticsReply>) => void) | ((event: Event) => void),
  ): void {
    const wrapper = { handleEvent: listener };
    this.#wrappers.set(listener, wrapper);
    this.#events.addEventListener(type, wrapper);
  }

  removeEventListener(type: "message", listener: (event: MessageEvent<OpticsReply>) => void): void;
  removeEventListener(type: "error", listener: (event: Event) => void): void;
  removeEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<OpticsReply>) => void) | ((event: Event) => void),
  ): void {
    const wrapper = this.#wrappers.get(listener);
    if (wrapper !== undefined) {
      this.#wrappers.delete(listener);
      this.#events.removeEventListener(type, wrapper);
    }
  }
}
