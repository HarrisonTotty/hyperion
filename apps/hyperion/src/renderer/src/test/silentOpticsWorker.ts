import type { OpticsReply, OpticsRequest } from "../view/atmosphere/opticsMessages";
import type { OpticsWorkerLike } from "../view/atmosphere/opticsPool";

/**
 * An optics worker that takes every request and answers none (plan R08, R08.T10.b): `VIEW`'s
 * tests' worker, so that a body whose record is answered stands building, as a slow worker leaves
 * it, and no test's frame waits on the optics; a test of the atmosphere's own drawing gives a
 * working worker, `InProcessOpticsWorker`.
 */
export class SilentOpticsWorker implements OpticsWorkerLike {
  /** Every request posted to it, in order. */
  readonly posted: OpticsRequest[] = [];
  #terminated = false;

  /** Whether the pool terminated it. */
  get terminated(): boolean {
    return this.#terminated;
  }

  postMessage(message: OpticsRequest): void {
    this.posted.push(message);
  }

  terminate(): void {
    this.#terminated = true;
  }

  addEventListener(
    _type: "message" | "error",
    _listener: ((event: MessageEvent<OpticsReply>) => void) | ((event: Event) => void),
  ): void {}

  removeEventListener(
    _type: "message" | "error",
    _listener: ((event: MessageEvent<OpticsReply>) => void) | ((event: Event) => void),
  ): void {}
}
