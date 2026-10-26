import { OpticsJobs, type OpticsJobsDeps } from "../view/atmosphere/opticsJobs";
import type { OpticsReply, OpticsRequest } from "../view/atmosphere/opticsMessages";
import type { OpticsWorkerLike } from "../view/atmosphere/opticsPool";

/**
 * An optics worker run on the test's own thread (plan R08, R08.T10.a): each request answered on a
 * later task by the real worker's own logic (`opticsJobs.ts`' {@link OpticsJobs}), as
 * `optics.worker.ts` answers in its module worker.
 *
 * @remarks
 * Both ways the messages cross the boundary as a worker's do, through `structuredClone` with
 * their transfer lists, so that a buffer transferred is detached on the side that sent it: the
 * render thread's after a join's sums are posted, the worker's after a reply. A k-table that
 * cannot be read rejects the answer, which becomes the worker's `error` event, as
 * `optics.worker.ts` raises it.
 */
export class InProcessOpticsWorker implements OpticsWorkerLike {
  readonly #jobs: OpticsJobs;
  readonly #events = new EventTarget();
  readonly #wrappers = new Map<unknown, EventListenerObject>();
  readonly #timers = new Set<ReturnType<typeof setTimeout>>();
  /** Every request it was posted, as it received them, shared with the log the test gave. */
  readonly #log: OpticsRequest[];
  #terminated = false;

  /**
   * @param deps - The worker's k-table reader.
   * @param log - Where every request posted is recorded, in the order posted: one log over a
   *   pool's workers shows the pool's order.
   */
  constructor(deps: OpticsJobsDeps, log: OpticsRequest[] = []) {
    this.#jobs = new OpticsJobs(deps);
    this.#log = log;
  }

  /** Whether the pool terminated it. */
  get terminated(): boolean {
    return this.#terminated;
  }

  /** The species whose tables this worker loaded. */
  get loadedSpecies(): ReadonlyArray<string> {
    return this.#jobs.loadedSpecies;
  }

  postMessage(message: OpticsRequest, transfer: Transferable[]): void {
    if (this.#terminated) {
      return;
    }
    const request = structuredClone(message, { transfer });
    this.#log.push(request);
    const timer = setTimeout(() => {
      this.#timers.delete(timer);
      void this.#answer(request).catch((error: unknown) => {
        console.error("the in-process optics worker could not answer:", error);
      });
    }, 0);
    this.#timers.add(timer);
  }

  /** Ends the worker: its answers still to come are dropped. */
  terminate(): void {
    this.#terminated = true;
    for (const timer of this.#timers) {
      clearTimeout(timer);
    }
    this.#timers.clear();
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

  async #answer(request: OpticsRequest): Promise<void> {
    let event: Event;
    try {
      const { reply, transfer } = await this.#jobs.answer(request);
      event = new MessageEvent("message", { data: structuredClone(reply, { transfer }) });
    } catch {
      event = new Event("error");
    }
    if (!this.#terminated) {
      this.#events.dispatchEvent(event);
    }
  }
}
